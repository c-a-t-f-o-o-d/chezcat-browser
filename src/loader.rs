//! One replaceable page-loading worker, with small scoped image-fetch batches.
//! Network work stays off the window thread; navigation owns all resource reuse.
use crate::html::{self, Document, NodeKind};
use crate::http;
use crate::image::{self, Animation, Bitmap};
use crate::navigation::{self, Location};
use crate::trust;
use crate::{csp, css, font, fonts};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// User-controlled capabilities captured once for each document load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadOptions {
    /// Host-selected script policy; page content cannot change it.
    pub script_capacity: crate::javascript::limits::Capacities,
    pub javascript: bool,
    pub images: bool,
    pub blocking: Arc<crate::blocking::Policy>,
    /// Captured CSS viewport and device/page-zoom density for image selection.
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub density_milli: u32,
    /// Previous top-level document, for conservative SameSite cookie checks.
    pub cookie_origin: Option<crate::url::HttpUrl>,
    pub cookie_same_site: Option<bool>,
    /// Script-triggered document loads must retain this origin at every hop.
    pub script_origin: Option<crate::url::HttpUrl>,
    /// The initiating document's CSP for a form submission.
    pub form_policy: Option<Arc<csp::PolicySet>>,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            script_capacity: crate::javascript::limits::BROWSER_CAPACITIES,
            javascript: true,
            images: true,
            blocking: Arc::new(crate::blocking::Policy::default()),
            viewport_width: 1068,
            viewport_height: 660,
            density_milli: 1000,
            cookie_origin: None,
            cookie_same_site: None,
            script_origin: None,
            form_policy: None,
        }
    }
}

pub(crate) const MAX_SCRIPTS: usize = crate::javascript::limits::BROWSER_CAPACITIES.programs;
pub(crate) const MAX_STYLESHEETS: usize = 256;
const MAX_RESOURCE_ATTEMPTS: usize = 8192;
pub(crate) const MAX_SCRIPT_BYTES: usize = crate::javascript::limits::BROWSER_CAPACITIES.source_bytes;
// Decoded-image counts stay independent from CSS/script request capacities.
const MAX_UNIQUE_IMAGES: usize = 64;
const MAX_DECODED_IMAGE_BYTES: usize = 64 * 1024 * 1024;

pub struct Script {
    pub node: usize,
    pub source: String,
    pub label: String,
}

pub struct Loaded {
    pub network: crate::network_trace::Trace,
    pub location: Location,
    /// The final document exchange, after any redirects. Assets never inherit it.
    pub request: http::Request,
    pub cookie_same_site: bool,
    pub base: Location,
    pub document: Document,
    pub raw: String,
    pub styles: HashMap<usize, String>,
    pub fonts: fonts::Book,
    pub images: HashMap<usize, Arc<Bitmap>>,
    pub image_densities: HashMap<usize, f64>,
    pub animations: HashMap<usize, Arc<Animation>>,
    pub scripts: Vec<Script>,
    pub csp: csp::PolicySet,
    pub scripting_blocked: Option<String>,
    pub events: Vec<String>,
    pub skipped: usize,
    pub notice: String,
}
// Reuse belongs to one document load. It never survives reload or navigation.
// Images share their immutable pixels; CSS is still attached in document order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ResourceAddress {
    Web(crate::url::HttpUrl),
    File(std::path::PathBuf),
    Data([u8; 32]),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ResourceKey {
    kind: &'static str,
    address: ResourceAddress,
}
impl ResourceKey {
    fn new(kind: &'static str, location: &Location) -> Option<Self> {
        let address = if let Some(url) = &location.url {
            ResourceAddress::Web(url.clone())
        } else {
            ResourceAddress::File(location.path.clone()?)
        };
        Some(Self { kind, address })
    }
}
#[derive(Clone)]
enum ResourceOutcome {
    Css(String, Location),
    Image(Arc<Bitmap>, Option<Arc<Animation>>),
    Failed(String),
}

pub struct Completion {
    pub id: u64,
    pub result: Result<LoadEnd, String>,
}
pub struct ImageUpdate {
    pub images: HashMap<usize, Arc<Bitmap>>,
    pub animations: HashMap<usize, Arc<Animation>>,
    pub events: Vec<String>,
    /// New skipped references since the preceding delivery.
    pub skipped: usize,
}
pub struct LoadEnd {
    /// An attachment was delivered without committing a document.
    pub download: bool,
    pub events: Vec<String>,
    /// Total skipped references in the complete document load.
    pub skipped: usize,
    pub notice: String,
}
pub struct Poll {
    pub download: Option<(u64, http::Response)>,
    pub progress: Option<(u64, String)>,
    pub document: Option<(u64, Loaded)>,
    pub images: Option<(u64, ImageUpdate)>,
    pub completed: Option<Completion>,
}
enum Delivery {
    Download(Box<http::Response>),
    Document(Box<Loaded>),
    Images(ImageUpdate),
}
struct Request {
    network: crate::network_trace::Trace,
    id: u64,
    location: Location,
    cancel: Arc<AtomicBool>,
    mode: http::FetchMode,
    options: LoadOptions,
    http_request: http::Request,
}
struct WorkerState {
    network: Option<(u64, crate::network_trace::Trace)>,
    download: Option<(u64, http::Response)>,
    request: Option<Request>,
    active: Option<Arc<AtomicBool>>,
    document: Option<(u64, Loaded)>,
    images: Option<(u64, ImageUpdate)>,
    completed: Option<Completion>,
    progress: Option<(u64, String)>,
    stopping: bool,
    wake: Wake,
}
pub struct Worker {
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
}
impl Worker {
    #[allow(dead_code)] // Retained isolated loader API for independent callers.
    pub fn new(wake: Wake, trust: Arc<trust::Store>) -> Result<Self, String> {
        Self::with_cookie_session(wake, trust, Arc::new(crate::cookies::Session::default()))
    }

    pub fn with_cookie_session(
        wake: Wake,
        trust: Arc<trust::Store>,
        cookies: Arc<crate::cookies::Session>,
    ) -> Result<Self, String> {
        let shared = Arc::new((
            Mutex::new(WorkerState {
                network: None,
                download: None,
                request: None,
                active: None,
                document: None,
                images: None,
                completed: None,
                progress: None,
                stopping: false,
                wake,
            }),
            Condvar::new(),
        ));
        let state = shared.clone();
        std::thread::Builder::new()
            .name("page-loader".to_owned())
            .spawn(move || loop {
                let request = {
                    let mut guard = state.0.lock().unwrap();
                    while guard.request.is_none() && !guard.stopping {
                        guard = state.1.wait(guard).unwrap();
                    }
                    if guard.stopping {
                        return;
                    }
                    let request = guard.request.take().unwrap();
                    guard.active = Some(request.cancel.clone());
                    request
                };
                let progress = |message: String| {
                    if request.cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let wake = {
                        let mut guard = state.0.lock().unwrap();
                        if guard.stopping || request.cancel.load(Ordering::Relaxed) {
                            return;
                        }
                        guard.progress = Some((request.id, message));
                        guard.wake.clone()
                    };
                    wake();
                };
                let mut deliver = |delivery| {
                    let wake = {
                        let mut guard = state.0.lock().unwrap();
                        if guard.stopping || request.cancel.load(Ordering::Relaxed) {
                            return Err("Navigation canceled.".to_owned());
                        }
                        match delivery {
                            Delivery::Download(response) => {
                                guard.download = Some((request.id, *response));
                            }
                            Delivery::Document(loaded) => {
                                guard.document = Some((request.id, *loaded));
                            }
                            Delivery::Images(update) => {
                                if let Some((id, queued)) = &mut guard.images {
                                    debug_assert_eq!(*id, request.id);
                                    queued.images.extend(update.images);
                                    queued.animations.extend(update.animations);
                                    queued.events.extend(update.events);
                                    queued.skipped += update.skipped;
                                } else {
                                    guard.images = Some((request.id, update));
                                }
                            }
                        }
                        guard.wake.clone()
                    };
                    wake();
                    Ok(())
                };
                let result = load_request_progressively(
                    &request.location,
                    &request.http_request,
                    &request.cancel,
                    &progress,
                    &trust,
                    request.mode,
                    request.options.clone(),
                    request.network.clone(),
                    cookies.clone(),
                    &mut deliver,
                );
                let wake = {
                    let mut guard = state.0.lock().unwrap();
                    guard.active = None;
                    if guard.stopping {
                        return;
                    }
                    if !request.cancel.load(Ordering::Relaxed) {
                        guard.completed = Some(Completion {
                            id: request.id,
                            result,
                        });
                    }
                    // Cancellation still finishes measured transport phases.
                    // Wake once so a visible Network view can show that final
                    // state after Stop has disabled its periodic activity tick.
                    // No canceled document/completion is published.
                    guard.wake.clone()
                };
                wake();
            })
            .map_err(|e| format!("Cannot start page loader: {e}"))?;
        Ok(Self { shared })
    }
    pub fn set_wake(&self, wake: Wake) {
        self.shared.0.lock().unwrap().wake = wake;
    }
    #[allow(dead_code)] // Retained GET convenience entry point for callers without form state.
    pub fn submit(
        &self,
        id: u64,
        location: Location,
        mode: http::FetchMode,
        scripts_enabled: bool,
    ) {
        self.submit_request(id, location, mode, scripts_enabled, http::Request::Get);
    }
    #[allow(dead_code)] // Existing command-line development probes use this wrapper.
    pub fn submit_request(
        &self,
        id: u64,
        location: Location,
        mode: http::FetchMode,
        scripts_enabled: bool,
        http_request: http::Request,
    ) {
        self.submit_request_with_options(
            id,
            location,
            mode,
            LoadOptions {
                javascript: scripts_enabled,
                ..LoadOptions::default()
            },
            http_request,
        );
    }
    pub fn submit_request_with_options(
        &self,
        id: u64,
        location: Location,
        mode: http::FetchMode,
        options: LoadOptions,
        http_request: http::Request,
    ) {
        let mut state = self.shared.0.lock().unwrap();
        if let Some(active) = &state.active {
            active.store(true, Ordering::Relaxed);
        }
        if let Some(queued) = &state.request {
            queued.cancel.store(true, Ordering::Relaxed);
        }
        let network = crate::network_trace::Trace::default();
        state.network = Some((id, network.clone()));
        state.request = Some(Request {
            network,
            id,
            location,
            cancel: Arc::new(AtomicBool::new(false)),
            mode,
            options,
            http_request,
        });
        state.completed = None;
        state.download = None;
        state.document = None;
        state.images = None;
        state.progress = None;
        self.shared.1.notify_one();
    }
    pub fn cancel(&self) {
        let mut state = self.shared.0.lock().unwrap();
        if let Some(active) = &state.active {
            active.store(true, Ordering::Relaxed);
        }
        state.request = None;
        state.completed = None;
        state.download = None;
        state.document = None;
        state.images = None;
        state.progress = None;
    }
    pub fn poll(&self) -> Poll {
        let mut state = self.shared.0.lock().unwrap();
        Poll {
            download: state.download.take(),
            progress: state.progress.take(),
            document: state.document.take(),
            images: state.images.take(),
            completed: state.completed.take(),
        }
    }
    /// The latest requested navigation, including an in-flight or failed load.
    /// Clones share measurements; a later submit installs a fresh recorder.
    pub fn network_trace(&self) -> Option<(u64, crate::network_trace::Trace)> {
        self.shared.0.lock().unwrap().network.clone()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        let mut state = self.shared.0.lock().unwrap();
        state.stopping = true;
        self.shared.1.notify_one();
        // A blocking OS DNS call cannot be cancelled by std::net. Do not hold up
        // the UI or process shutdown waiting for it; this single worker exits
        // when the OS call returns (or the process ends).
    }
}

#[cfg(test)]
pub fn local(location: &Location) -> Result<Loaded, String> {
    local_with_scripts(location, true)
}

#[allow(dead_code)] // Kept for existing engine callers with default image policy.
pub fn local_with_scripts(location: &Location, scripts_enabled: bool) -> Result<Loaded, String> {
    local_with_options(
        location,
        LoadOptions {
            javascript: scripts_enabled,
            ..LoadOptions::default()
        },
    )
}

pub fn local_with_options(location: &Location, options: LoadOptions) -> Result<Loaded, String> {
    load_with_options(
        location,
        &AtomicBool::new(false),
        &|_| {},
        &trust::Store::unconfigured(),
        http::FetchMode::Normal,
        options,
    )
}

#[allow(dead_code)] // Existing independent transport probes use the bool-only API.
pub fn load(
    initial: &Location,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    trust: &trust::Store,
    mode: http::FetchMode,
    scripts_enabled: bool,
) -> Result<Loaded, String> {
    load_request(
        initial,
        &http::Request::Get,
        cancel,
        progress,
        trust,
        mode,
        scripts_enabled,
    )
}

pub fn load_with_options(
    initial: &Location,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    trust: &trust::Store,
    mode: http::FetchMode,
    options: LoadOptions,
) -> Result<Loaded, String> {
    load_request_with_options(
        initial,
        &http::Request::Get,
        cancel,
        progress,
        trust,
        mode,
        options,
    )
}

#[allow(dead_code)] // Existing independent form/transport probes use this wrapper.
pub fn load_request(
    initial: &Location,
    request: &http::Request,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    trust: &trust::Store,
    mode: http::FetchMode,
    scripts_enabled: bool,
) -> Result<Loaded, String> {
    load_request_with_options(
        initial,
        request,
        cancel,
        progress,
        trust,
        mode,
        LoadOptions {
            javascript: scripts_enabled,
            ..LoadOptions::default()
        },
    )
}

pub fn load_request_with_options(
    initial: &Location,
    request: &http::Request,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    trust: &trust::Store,
    mode: http::FetchMode,
    options: LoadOptions,
) -> Result<Loaded, String> {
    let mut assembled: Option<Loaded> = None;
    let end =
        load_request_progressively(
            initial,
            request,
            cancel,
            progress,
            trust,
            mode,
            options,
            crate::network_trace::Trace::default(),
            Arc::new(crate::cookies::Session::default()),
            &mut |delivery| {
                match delivery {
                    Delivery::Download(_) => return Err(
                        "This response is an attachment; a browser download handler is required."
                            .into(),
                    ),
                    Delivery::Document(loaded) => assembled = Some(*loaded),
                    Delivery::Images(update) => {
                        let loaded = assembled.as_mut().expect("document precedes image results");
                        loaded.images.extend(update.images);
                        loaded.animations.extend(update.animations);
                        loaded.events.extend(update.events);
                        loaded.skipped += update.skipped;
                    }
                }
                Ok(())
            },
        )?;
    let mut loaded = assembled.expect("successful load supplies a document");
    loaded.events.extend(end.events);
    loaded.skipped = end.skipped;
    loaded.notice = end.notice;
    Ok(loaded)
}

#[allow(clippy::too_many_arguments)]
fn load_request_progressively(
    initial: &Location,
    request: &http::Request,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    trust: &trust::Store,
    mode: http::FetchMode,
    options: LoadOptions,
    network: crate::network_trace::Trace,
    cookies: Arc<crate::cookies::Session>,
    deliver: &mut dyn FnMut(Delivery) -> Result<(), String>,
) -> Result<LoadEnd, String> {
    check_cancel(cancel)?;
    if request.is_post() && initial.url.is_none() {
        return Err("POST forms require an HTTP or HTTPS destination; local-file submissions are not supported.".into());
    }
    let started = Instant::now();
    let resource_filter = |url: &crate::url::HttpUrl| options.blocking.check(url);
    let document_filter = |url: &crate::url::HttpUrl| {
        if let Some(policy) = &options.form_policy {
            policy.allows_form_action(&Location {
                url: Some(url.clone()),
                path: None,
                root: None,
                fragment: String::new(),
            })
        } else {
            Ok(())
        }
    };
    let pool = http::Pool::new(cancel, trust)
        .with_resource_filter(&resource_filter)
        .with_network_trace(network.clone())
        .with_cookie_session(cookies, options.cookie_origin.clone())
        .with_cookie_context(options.cookie_same_site)
        .with_document_origin(options.script_origin.clone())
        .with_document_filter(&document_filter);
    let mut location = initial.clone();
    let mut final_request = http::Request::Get;
    let mut final_cookie_same_site = false;
    let mut events = Vec::new();
    events.push(options.blocking.description());
    if mode == http::FetchMode::FullReload {
        events.push("FULL RELOAD: re-fetch the document and all supported resources; Cache-Control: no-cache; Pragma: no-cache. No browser cache is implemented yet.".to_owned());
    }
    let mut notice = String::new();
    let mut plain = false;
    let mut document_image = None;
    let mut image_document = false;
    let mut document_type = "text/html".to_owned();
    let scripting_blocked =
        (!options.javascript).then(|| "JavaScript disabled by user.".to_owned());
    let mut csp_headers = Vec::new();
    let mut embedder_headers = Vec::new();
    let raw = if let Some(url) = &initial.url {
        progress(format!("Loading {}", url.display()));
        let response = pool.fetch_document(url, request, mode)?;
        final_cookie_same_site = response.cookie_same_site;
        if response
            .header("content-disposition")
            .is_some_and(crate::downloads::is_attachment)
        {
            let events = response.events.clone();
            deliver(Delivery::Download(Box::new(response)))?;
            return Ok(LoadEnd {
                download: true,
                events,
                skipped: 0,
                notice: "Download received.".into(),
            });
        }
        if response.request_is_post {
            final_request = request.clone();
        }
        if matches!(response.status, 204 | 205) {
            return Err(format!(
                "HTTP {} supplied no new document; the current page is retained.",
                response.status
            ));
        }
        if response.status == 304 {
            return Err("HTTP 304 requires a cached response, and this build has no HTTP cache; the current page is retained.".to_owned());
        }
        let content_type = response.header("content-type").unwrap_or("");
        csp_headers.extend(
            response
                .headers
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-security-policy"))
                .map(|(_, value)| value.clone()),
        );
        embedder_headers.extend(
            response
                .headers
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case("cross-origin-embedder-policy"))
                .map(|(_, value)| value.clone()),
        );
        if response
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-security-policy-report-only"))
        {
            events.push("Content-Security-Policy-Report-Only is present; it does not block scripts. Report-only violation evaluation and reporting are not implemented.".to_owned());
        }
        let mime = media_type(content_type);
        image_document = mime.starts_with("image/");
        if !mime.is_empty() && mime != "text/html" && mime != "text/plain" && !image_document {
            return Err(format!("This build cannot display document type {mime}. HTML, plain text and supported image formats can be opened."));
        }
        plain = mime == "text/plain";
        if !mime.is_empty() {
            document_type = mime;
        }
        let raw = if image_document {
            if options.images {
                document_image = Some(image::decode_asset_checked(&response.body, &|| {
                    check_cancel(cancel)
                })?);
            }
            image_document_markup(
                &response.url.display(),
                response.body.len(),
                document_image.as_ref().map(|asset| asset.bitmap.as_ref()),
            )
        } else {
            decode_text(&response.body, content_type, !plain)?
        };
        notice = if let Some(protocol) = response.protocol {
            format!(
                "HTTP {} | {protocol} | Certificate verified",
                response.status
            )
        } else {
            format!("HTTP {} | Unencrypted connection", response.status)
        };
        events.extend(response.events);
        location.url = Some(response.url);
        if let Some(fragment) = response.fragment {
            location.fragment = fragment;
        }
        events.push(format!(
            "DOCUMENT: HTTP {}, {} bytes, {}",
            response.status,
            response.body.len(),
            location.display()
        ));
        raw
    } else if let Some(path) = &location.path {
        let local_type = image_file_type(path);
        image_document = local_type.is_some();
        if let Some(mime) = local_type {
            document_type = mime.to_owned();
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
        {
            plain = true;
            document_type = "text/plain".into();
        }
        let bytes = navigation::read_limited(
            path,
            if image_document {
                image::MAX_INPUT_BYTES
            } else {
                1_048_576
            },
        )?;
        if image_document {
            if options.images {
                document_image = Some(image::decode_asset_checked(&bytes, &|| {
                    check_cancel(cancel)
                })?);
            }
            image_document_markup(
                &location.display(),
                bytes.len(),
                document_image.as_ref().map(|asset| asset.bitmap.as_ref()),
            )
        } else {
            String::from_utf8(bytes)
                .map_err(|_| "Local HTML/plain-text documents must contain UTF-8 text.".to_owned())?
                .trim_start_matches('\u{feff}')
                .to_owned()
        }
    } else {
        include_str!("../pages/start.html").to_owned()
    };
    check_cancel(cancel)?;
    let markup = if plain {
        format!(
            "<title>Plain text</title><body><pre style='white-space:pre-wrap'>{}</pre></body>",
            escape(&raw)
        )
    } else {
        raw.clone()
    };
    let mut document = html::parse(&markup);
    document.content_type = document_type;
    let mut csp = csp::PolicySet::new(&location);
    for value in csp_headers {
        csp.add_header(&value);
    }
    for value in embedder_headers {
        csp.add_embedder_header(&value);
    }
    events.push(format!("HTML/text loaded: {}", location.display()));
    let mut loaded = Loaded {
        network,
        base: location.clone(),
        location,
        request: final_request,
        cookie_same_site: final_cookie_same_site,
        document,
        raw,
        styles: HashMap::new(),
        fonts: fonts::Book::empty(),
        images: HashMap::new(),
        image_densities: HashMap::new(),
        animations: HashMap::new(),
        scripts: Vec::new(),
        csp,
        scripting_blocked,
        events,
        skipped: 0,
        notice,
    };
    if image_document {
        loaded.events.push("IMAGE DOCUMENT: generated viewer for a binary image; Sources shows this viewer's markup. No second image request is made. HTTP image documents currently share the 2 MiB document-transfer limit.".into());
        if let Some(asset) = document_image {
            let node = loaded
                .document
                .nodes
                .iter()
                .position(
                    |node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img"),
                )
                .ok_or("Image viewer is missing its image element.")?;
            loaded.events.push(format!(
                "IMAGE DOCUMENT: decoded {} x {} pixels.",
                asset.bitmap.width, asset.bitmap.height
            ));
            if let Some(animation) = asset.animation {
                loaded.events.push(animation_description(&animation));
                loaded.animations.insert(node, animation);
            }
            if let Some(notice) = asset.notice {
                loaded.events.push(format!("IMAGE DOCUMENT: {notice}"));
            }
            loaded.images.insert(node, asset.bitmap);
        } else {
            loaded.events.push(
                "IMAGE DOCUMENT: decoding and display disabled by the image preference.".into(),
            );
        }
    }
    let inert = inert_nodes(&loaded.document, true);
    for (node, item) in loaded.document.nodes.iter().enumerate() {
        if !inert[node]
            && matches!(&item.kind, NodeKind::Element { tag, .. } if tag == "meta")
            && loaded
                .document
                .attr(node, "http-equiv")
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("content-security-policy"))
            && loaded.document.attr(node, "content").is_some()
        {
            loaded
                .csp
                .add_meta(loaded.document.attr(node, "content").unwrap(), node);
        }
    }
    loaded
        .events
        .extend(loaded.csp.diagnostics().iter().cloned());
    if let Some(reason) = &loaded.scripting_blocked {
        loaded.events.push(reason.clone());
    }
    for (i, node) in loaded.document.nodes.iter().enumerate() {
        if inert[i] {
            continue;
        }
        if matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "base") {
            if let Some(href) = loaded.document.attr(i, "href") {
                match navigation::resolve(href, &loaded.location) {
                    Ok(base) => match loaded.csp.allows_base(i, &base) {
                        Ok(()) => {
                            loaded.events.push(format!("BASE: {}", base.display()));
                            loaded.base = base;
                        }
                        Err(error) => loaded.events.push(format!("BASE skipped: {error}")),
                    },
                    Err(error) => loaded.events.push(format!("BASE skipped: {error}")),
                }
                break;
            }
        }
    }
    let mut stats = LoadStats::new(started);
    let environment = crate::image_selection::Environment {
        viewport_width: f64::from(options.viewport_width),
        viewport_height: f64::from(options.viewport_height),
        density: f64::from(options.density_milli) / 1000.0,
    };
    let mut image_plan = image_plan(
        &loaded.document,
        loaded.scripting_blocked.is_none(),
        environment,
        &mut loaded.events,
    );
    loaded.image_densities = image_plan
        .iter()
        .map(|image| (image.node, image.density))
        .collect();
    if !options.images {
        for image in &image_plan {
            if let Ok(resolved) = navigation::resolve(&image.target, &loaded.base) {
                if let Some(url) = resolved.url {
                    pool.network_trace().blocked(
                        &url.display(),
                        "image",
                        "Images disabled by user.",
                    );
                }
            }
        }
        stats.skipped += image_plan.len();
        loaded.events.push(format!(
            "Images disabled by user: {} image reference(s) skipped before fetching or decoding.",
            image_plan.len()
        ));
        image_plan.clear();
    }
    load_critical_resources(&mut loaded, &mut stats, cancel, progress, &pool, mode, options.script_capacity)?;
    check_cancel(cancel)?;
    loaded.events.push(format!(
        "DISPLAY READY: {} ms after document request; CSS, fonts and classic script sources ready; images load afterward.",
        started.elapsed().as_millis()
    ));
    let base = loaded.base.clone();
    let origin = loaded.location.url.clone();
    let notice = loaded.notice.clone();
    loaded.skipped = stats.skipped;
    deliver(Delivery::Document(Box::new(loaded)))?;
    load_images(
        &image_plan,
        &base,
        origin.as_ref(),
        &mut stats,
        cancel,
        progress,
        &pool,
        mode,
        deliver,
    )?;
    check_cancel(cancel)?;
    let mut events = Vec::new();
    if stats.attempts >= MAX_RESOURCE_ATTEMPTS {
        events.push(
            format!("Resource attempt limit reached ({MAX_RESOURCE_ATTEMPTS}). Additional references were skipped."),
        );
    }
    if stats.omitted_messages > 0 {
        events.push(format!(
            "{} repeated/additional resource messages omitted.",
            stats.omitted_messages
        ));
    }
    let total = started.elapsed();
    events.push(format!(
        "LOAD TIMING: document/parse {} ms; resources {} ms; total {} ms. {} resource attempts; {} reused references ({} earlier failures). Reuse ends with this document load.",
        stats.document_ready.as_millis(), total.saturating_sub(stats.document_ready).as_millis(), total.as_millis(), stats.attempts, stats.reused, stats.reused_failures
    ));
    events.push("Classic JavaScript uses a bounded subset after document parsing. Session cookies stay in memory with host-only/Domain scope and bundled public-suffix boundaries. No HTTP authentication, referrer header, frames, CSS imports, speculative requests or telemetry. Stylesheets, images, fonts and script navigation retain the document origin. Classic scripts without crossorigin may also include HTTPS sources from HTTPS documents, with CSP and request checks at each redirect and no cross-origin credentials. Crossorigin-marked scripts currently require the document origin at every hop.".to_owned());
    Ok(LoadEnd {
        download: false,
        events,
        skipped: stats.skipped,
        notice,
    })
}

struct LoadStats {
    started: Instant,
    document_ready: Duration,
    reused: usize,
    reused_failures: usize,
    resource_messages: usize,
    omitted_messages: usize,
    network_bytes: usize,
    attempts: usize,
    skipped: usize,
}
impl LoadStats {
    fn new(started: Instant) -> Self {
        Self {
            started,
            document_ready: started.elapsed(),
            reused: 0,
            reused_failures: 0,
            resource_messages: 0,
            omitted_messages: 0,
            network_bytes: 0,
            attempts: 0,
            skipped: 0,
        }
    }
    fn begin_attempt(&mut self, network: bool) -> Result<(), String> {
        if self.attempts >= MAX_RESOURCE_ATTEMPTS {
            return Err(format!("Resource attempt limit reached ({MAX_RESOURCE_ATTEMPTS})."));
        }
        if network && self.started.elapsed() > Duration::from_secs(45) {
            return Err("Page resource time budget reached (45 seconds).".to_owned());
        }
        self.attempts += 1;
        Ok(())
    }
    fn remaining_bytes(&self) -> usize {
        (256 * 1024 * 1024usize).saturating_sub(self.network_bytes)
    }
    fn record(
        &mut self,
        kind: &str,
        label: &str,
        reused: bool,
        result: Result<String, String>,
        events: &mut Vec<String>,
    ) {
        if result.is_err() {
            self.skipped += 1;
        }
        // Bounds are shared by critical resources and later image deliveries.
        if self.resource_messages < 128 && (!reused || self.reused <= 24) {
            self.resource_messages += 1;
            events.push(match result {
                Ok(message) => format!("{kind} {label}: {message}"),
                Err(error) => format!("{kind} {label}: SKIPPED - {error}"),
            });
        } else {
            self.omitted_messages += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn load_critical_resources(
    loaded: &mut Loaded,
    stats: &mut LoadStats,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    pool: &http::Pool<'_>,
    mode: http::FetchMode,
    capacity: crate::javascript::limits::Capacities,
) -> Result<(), String> {
    let inert = inert_nodes(&loaded.document, loaded.scripting_blocked.is_none());
    let script_inert = inert_nodes(&loaded.document, true);
    let mut cache = HashMap::<ResourceKey, ResourceOutcome>::new();
    let mut style_bases = HashMap::<usize, Location>::new();
    let mut css_bytes = 0usize;
    let mut script_bytes = 0usize;
    for (node, item) in loaded.document.nodes.iter().enumerate() {
        check_cancel(cancel)?;
        if inert[node] {
            continue;
        }
        let NodeKind::Element { tag, .. } = &item.kind else {
            continue;
        };
        let kind = if tag == "link"
            && loaded.document.attr(node, "rel").is_some_and(|value| {
                value
                    .split_ascii_whitespace()
                    .any(|rel| rel.eq_ignore_ascii_case("stylesheet"))
            }) {
            "CSS"
        } else if tag == "script" && !script_inert[node] {
            "SCRIPT"
        } else {
            continue;
        };
        let reference = loaded
            .document
            .attr(node, if kind == "CSS" { "href" } else { "src" });
        if kind == "CSS" && reference.is_none() {
            continue;
        }
        let target = reference.unwrap_or("");
        let label: String = if kind == "SCRIPT" && reference.is_none() {
            format!("inline node {node}")
        } else {
            target.chars().take(220).collect()
        };
        let mut attempted_key = None;
        let mut was_reused = false;
        let trace_address = reference
            .and_then(|target| navigation::resolve(target, &loaded.base).ok())
            .and_then(|location| location.url)
            .map(|url| url.display());
        let mut trace_attempted = false;
        let result = (|| -> Result<String, String> {
            if kind == "SCRIPT" {
                if let Some(reason) = &loaded.scripting_blocked {
                    return Err(reason.clone());
                }
                check_script(&loaded.document, node)?;
                if loaded.scripts.len() >= capacity.programs {
                    return Err(format!("Script count limit reached ({}).", capacity.programs));
                }
                if script_bytes >= capacity.retained_source {
                    return Err(format!("Script source budget is full ({} MiB).", capacity.retained_source / (1024 * 1024)));
                }
                if reference.is_none() {
                    stats.begin_attempt(loaded.location.url.is_some())?;
                    let source = script_text(&loaded.document, node, capacity)?;
                    loaded
                        .csp
                        .allows_inline(node, csp::nonce(&loaded.document, node), &source)?;
                    check_script_size(source.len(), script_bytes, capacity)?;
                    script_bytes += source.len();
                    loaded.scripts.push(Script {
                        node,
                        source,
                        label: label.clone(),
                    });
                    return Ok("queued classic script after document parsing".to_owned());
                }
            }
            if target.trim().is_empty() {
                return Err("Empty resource reference.".to_owned());
            }
            if kind == "CSS" {
                if loaded.styles.len() >= MAX_STYLESHEETS {
                    return Err(format!("Stylesheet count limit reached ({MAX_STYLESHEETS})."));
                }
                let media = loaded.document.attr(node, "media").unwrap_or("").trim();
                if !media.is_empty()
                    && !media.eq_ignore_ascii_case("all")
                    && !media.eq_ignore_ascii_case("screen")
                {
                    return Err("Only screen/all media is supported.".to_owned());
                }
                if loaded.document.attr(node, "rel").is_some_and(|value| {
                    value
                        .split_ascii_whitespace()
                        .any(|rel| rel.eq_ignore_ascii_case("alternate"))
                }) {
                    return Err("Alternate stylesheets are not enabled.".to_owned());
                }
                if css_bytes >= 512 * 1024 {
                    return Err("Stylesheet budget is full (512 KiB).".to_owned());
                }
            }
            let resolved = navigation::resolve(target, &loaded.base)?;
            let same_origin_only = kind == "SCRIPT"
                && loaded.document.attr(node, "crossorigin").is_some();
            if same_origin_only {
                let origin = loaded.location.url.as_ref()
                    .ok_or("CORS-mode scripts require an HTTP(S) document origin.")?;
                let target = resolved.url.as_ref()
                    .ok_or("CORS-mode scripts require an HTTP(S) resource URL.")?;
                http::check_classic_script_mode(origin, target, true)?;
            }
            // Per-element CSP must precede reuse too: a previous nonce does
            // not authorize another LINK node referencing the same URL.
            if kind == "CSS" {
                loaded.csp.allows_external_style(
                    node,
                    csp::nonce(&loaded.document, node),
                    &resolved,
                )?;
            }
            if let Some(url) = &resolved.url {
                pool.check_resource(url)?;
            }
            let key = (kind == "CSS")
                .then(|| ResourceKey::new(kind, &resolved))
                .flatten();
            if let Some(outcome) = key.as_ref().and_then(|key| cache.get(key)).cloned() {
                was_reused = true;
                if let Some(address) = &trace_address {
                    pool.network_trace().reused(
                        address,
                        kind,
                        matches!(outcome, ResourceOutcome::Failed(_)),
                    );
                }
                stats.reused += 1;
                return match outcome {
                    ResourceOutcome::Css(source, base) => {
                        if css_bytes + source.len() > 512 * 1024 {
                            return Err("Stylesheet budget exceeded (512 KiB).".to_owned());
                        }
                        css_bytes += source.len();
                        loaded.styles.insert(node, source);
                        style_bases.insert(node, base);
                        Ok("reused stylesheet from this document load".to_owned())
                    }
                    ResourceOutcome::Failed(error) => {
                        stats.reused_failures += 1;
                        Err(format!(
                            "earlier attempt in this document load failed: {error}"
                        ))
                    }
                    ResourceOutcome::Image(..) => unreachable!("CSS cache only"),
                };
            }
            stats.begin_attempt(loaded.location.url.is_some())?;
            attempted_key = key;
            if kind == "SCRIPT" {
                loaded
                    .csp
                    .allows_external(node, csp::nonce(&loaded.document, node), &resolved)?;
            }
            let (bytes, encoding, resolved_location) = if let Some(url) = &resolved.url {
                let origin = loaded.location.url.as_ref().ok_or("Local pages do not automatically fetch network resources. Follow a web link explicitly first.")?;
                if kind == "SCRIPT" {
                    http::check_classic_script_origin(origin, url)?;
                } else if !origin.same_origin(url) {
                    return Err("Automatic resources must have the document's scheme, host and port in this build.".to_owned());
                }
                let remaining = stats.remaining_bytes();
                if remaining == 0 {
                    return Err("Page resource download budget is full (256 MiB).".to_owned());
                }
                progress(format!("Loading {kind} {label}"));
                let limit = if kind == "CSS" {
                    128 * 1024
                } else {
                    capacity.source_bytes
                };
                trace_attempted = true;
                let guard = |target: &crate::url::HttpUrl| {
                    let target = Location {
                        url: Some(target.clone()),
                        path: None,
                        root: None,
                        fragment: String::new(),
                    };
                    let nonce = csp::nonce(&loaded.document, node);
                    if kind == "SCRIPT" {
                        loaded.csp.allows_external(node, nonce, &target)
                    } else {
                        loaded.csp.allows_external_style(node, nonce, &target)
                    }
                };
                let response = if kind == "SCRIPT" {
                    pool.fetch_classic_script(url, limit.min(remaining), origin, mode, same_origin_only, &guard)?
                } else {
                    pool.fetch_kind_checked(url, limit.min(remaining), origin, mode, kind, &guard)?
                };
                stats.network_bytes += response.body.len();
                let final_location = Location {
                    url: Some(response.url.clone()),
                    path: None,
                    root: None,
                    fragment: String::new(),
                };
                if kind == "SCRIPT" {
                    loaded.csp.allows_external(
                        node,
                        csp::nonce(&loaded.document, node),
                        &final_location,
                    )?;
                }
                let encoding = response.header("content-type").unwrap_or("").to_owned();
                loaded.events.extend(response.events);
                if response.status < 200 || response.status >= 300 {
                    return Err(format!("HTTP {}", response.status));
                }
                if kind == "CSS"
                    && !media_type(&encoding).is_empty()
                    && media_type(&encoding) != "text/css"
                {
                    return Err(format!(
                        "Stylesheet has unsupported media type {}",
                        media_type(&encoding)
                    ));
                }
                if kind == "SCRIPT" && !javascript_mime(&media_type(&encoding)) {
                    return Err(format!(
                        "External scripts require a JavaScript Content-Type, received {}.",
                        if encoding.is_empty() {
                            "no Content-Type"
                        } else {
                            &encoding
                        }
                    ));
                }
                (response.body, encoding, final_location)
            } else {
                if loaded.location.url.is_some() {
                    return Err("A web document cannot load local files.".to_owned());
                }
                let path = resolved
                    .path
                    .as_ref()
                    .ok_or("This resource has no local file path.")?;
                let limit = if kind == "CSS" {
                    128 * 1024
                } else {
                    capacity.source_bytes
                };
                (
                    navigation::read_limited(path, limit)?,
                    "charset=utf-8".to_owned(),
                    resolved.clone(),
                )
            };
            check_cancel(cancel)?;
            let resolved_name = resolved_location.display();
            let source = decode_text(&bytes, &encoding, false)?;
            if kind == "CSS" {
                if css_bytes + source.len() > 512 * 1024 {
                    return Err("Stylesheet budget exceeded (512 KiB).".to_owned());
                }
                css_bytes += source.len();
                loaded.styles.insert(node, source);
                style_bases.insert(node, resolved_location);
                Ok(format!("loaded {resolved_name}"))
            } else {
                check_script_size(source.len(), script_bytes, capacity)?;
                script_bytes += source.len();
                loaded.scripts.push(Script {
                    node,
                    source,
                    label: resolved_name.clone(),
                });
                Ok(format!(
                    "queued classic script after document parsing: {resolved_name}"
                ))
            }
        })();
        if !trace_attempted && !was_reused {
            if let (Some(address), Err(error)) = (&trace_address, &result) {
                pool.network_trace().blocked(address, kind, error);
            }
        }
        if let Some(key) = attempted_key {
            let outcome = match &result {
                Err(error) => ResourceOutcome::Failed(error.clone()),
                Ok(_) => {
                    ResourceOutcome::Css(loaded.styles[&node].clone(), style_bases[&node].clone())
                }
            };
            cache.insert(key, outcome);
        }
        stats.record(kind, &label, was_reused, result, &mut loaded.events);
    }
    load_fonts(loaded, &style_bases, stats, cancel, progress, pool, mode)?;
    Ok(())
}

fn used_font_characters(loaded: &mut Loaded) -> HashMap<String, BTreeSet<u32>> {
    // CSS's noscript handling follows the actual load option, even before the
    // document is committed to the UI. Restore the flag expected at commit.
    let old_scripting = loaded.document.scripting_enabled;
    loaded.document.scripting_enabled = loaded.scripting_blocked.is_none();
    let styles = css::compute(&loaded.document, &loaded.styles);
    loaded.document.scripting_enabled = old_scripting;
    let mut hidden = vec![false; loaded.document.nodes.len()];
    let mut used = HashMap::<String, BTreeSet<u32>>::new();
    for (node, item) in loaded.document.nodes.iter().enumerate() {
        hidden[node] |= styles[node].display == css::Display::None;
        if hidden[node] {
            for &child in &item.children {
                hidden[child] = true;
            }
            continue;
        }
        let text = match &item.kind {
            NodeKind::Text(text) => text.as_str(),
            NodeKind::Element { tag, .. }
                if matches!(tag.as_str(), "input" | "textarea" | "select") =>
            {
                // Empty editable controls need their Latin font before typing.
                for family in &styles[node].font_families {
                    used.entry(family.to_ascii_lowercase())
                        .or_default()
                        .extend(32..=126);
                }
                loaded
                    .document
                    .attr(node, "value")
                    .or_else(|| loaded.document.attr(node, "placeholder"))
                    .unwrap_or("")
            }
            _ => continue,
        };
        if text.is_empty() {
            continue;
        }
        for family in &styles[node].font_families {
            used.entry(family.to_ascii_lowercase())
                .or_default()
                .extend(text.chars().map(u32::from));
        }
    }
    used
}

#[allow(clippy::too_many_arguments)]
fn load_fonts(
    loaded: &mut Loaded,
    style_bases: &HashMap<usize, Location>,
    stats: &mut LoadStats,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    pool: &http::Pool<'_>,
    mode: http::FetchMode,
) -> Result<(), String> {
    let inert = inert_nodes(&loaded.document, loaded.scripting_blocked.is_none());
    let mut plan = Vec::new();
    for (node, item) in loaded.document.nodes.iter().enumerate() {
        if inert[node] {
            continue;
        }
        let NodeKind::Element { tag, .. } = &item.kind else {
            continue;
        };
        if tag != "style" && tag != "link" {
            continue;
        }
        let media = loaded.document.attr(node, "media").unwrap_or("").trim();
        if !media.is_empty()
            && !media.eq_ignore_ascii_case("screen")
            && !media.eq_ignore_ascii_case("all")
        {
            continue;
        }
        let inline;
        let (source, base) = if tag == "style" {
            inline = item
                .children
                .iter()
                .filter_map(|&child| match &loaded.document.nodes[child].kind {
                    NodeKind::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>();
            (inline.as_str(), &loaded.base)
        } else if let (Some(source), Some(base)) =
            (loaded.styles.get(&node), style_bases.get(&node))
        {
            (source.as_str(), base)
        } else {
            continue;
        };
        for face in css::font_faces(source) {
            if plan.len() == 128 {
                break;
            }
            plan.push((base.clone(), face));
        }
        if plan.len() == 128 {
            break;
        }
    }
    if plan.is_empty() {
        return Ok(());
    }
    let used = used_font_characters(loaded);
    let mut cache = HashMap::<ResourceKey, Result<Arc<font::Font>, String>>::new();
    let mut font_bytes = 0usize;
    let mut unused = 0usize;
    for (base, face) in plan {
        check_cancel(cancel)?;
        let characters = used.get(&face.family.to_ascii_lowercase());
        if !face.family.is_empty()
            && (characters.is_none()
                || !face.ranges.is_empty()
                    && !characters.unwrap().iter().any(|code| {
                        face.ranges
                            .iter()
                            .any(|&(start, end)| (start..=end).contains(code))
                    }))
        {
            unused += 1;
            continue;
        }
        let label = format!(
            "{} {}{}",
            face.family,
            face.weight,
            if face.italic { " italic" } else { "" }
        );
        let mut reused = false;
        let result = (|| -> Result<String, String> {
            if let Some(error) = &face.error {
                return Err(error.clone());
            }
            if loaded.fonts.len() >= 24 {
                return Err("Page font-face limit reached (24).".into());
            }
            let mut failures = Vec::new();
            for source in &face.sources {
                check_cancel(cancel)?;
                let (font, description) = match source {
                    css::FontSource::Local(name) => {
                        let Some(local) =
                            fonts::Book::system().local(name, face.weight, face.italic)
                        else {
                            failures.push(format!(
                                "local({name}) is not in the available named font set"
                            ));
                            continue;
                        };
                        (local.font.clone(), format!("local({name})"))
                    }
                    css::FontSource::Url {
                        url,
                        format,
                        unsupported_technology,
                    } => {
                        if *unsupported_technology {
                            failures.push("src tech() requirements are not implemented".into());
                            continue;
                        }
                        if let Some(format) = format {
                            if !matches!(format.as_str(), "woff" | "truetype" | "opentype") {
                                failures.push(format!("src format({format}) is not implemented"));
                                continue;
                            }
                        }
                        let trace_address = navigation::resolve(url, &base)
                            .ok()
                            .and_then(|location| location.url)
                            .map(|url| url.display());
                        let mut trace_attempted = false;
                        let mut trace_reused = false;
                        let fetched = (|| -> Result<Arc<font::Font>, String> {
                            let resolved = navigation::resolve(url, &base)?;
                            if let Some(url) = &resolved.url {
                                pool.check_resource(url)?;
                            }
                            // Fonts are requested after parsing; all installed
                            // policies apply, including later meta policies.
                            loaded.csp.allows_font(usize::MAX, &resolved)?;
                            let key = ResourceKey::new("FONT", &resolved)
                                .ok_or("Font source has no fetchable address.")?;
                            if let Some(previous) = cache.get(&key) {
                                trace_reused = true;
                                if let Some(address) = &trace_address {
                                    pool.network_trace()
                                        .reused(address, "font", previous.is_err());
                                }
                                reused = true;
                                stats.reused += 1;
                                if previous.is_err() {
                                    stats.reused_failures += 1;
                                }
                                return previous.clone();
                            }
                            stats.begin_attempt(loaded.location.url.is_some())?;
                            let attempt = (|| -> Result<Arc<font::Font>, String> {
                                let remaining = (16 * 1024 * 1024usize).saturating_sub(font_bytes);
                                if remaining == 0 {
                                    return Err("Page font source budget is full (16 MiB).".into());
                                }
                                let limit = remaining.min(8 * 1024 * 1024);
                                let bytes = if let Some(url) = &resolved.url {
                                    let origin = loaded.location.url.as_ref().ok_or(
                                        "Local pages do not automatically fetch network fonts.",
                                    )?;
                                    if !origin.same_origin(url) {
                                        return Err("Automatic fonts must have the document's scheme, host and port in this build; cross-origin font loading/CORS is not implemented.".into());
                                    }
                                    let remaining = stats.remaining_bytes();
                                    if remaining == 0 {
                                        return Err(
                                            "Page resource download budget is full (256 MiB)."
                                                .into(),
                                        );
                                    }
                                    progress(format!("Loading font {label}"));
                                    trace_attempted = true;
                                    let response = pool.fetch_kind(
                                        url,
                                        limit.min(remaining),
                                        Some(origin),
                                        mode,
                                        "font",
                                    )?;
                                    stats.network_bytes += response.body.len();
                                    let final_location = Location {
                                        url: Some(response.url.clone()),
                                        path: None,
                                        root: None,
                                        fragment: String::new(),
                                    };
                                    loaded.csp.allows_font(usize::MAX, &final_location)?;
                                    if !(200..300).contains(&response.status) {
                                        return Err(format!("HTTP {}", response.status));
                                    }
                                    let mime =
                                        media_type(response.header("content-type").unwrap_or(""));
                                    if !mime.is_empty()
                                        && !mime.starts_with("font/")
                                        && !matches!(
                                            mime.as_str(),
                                            "application/octet-stream"
                                                | "application/font-woff"
                                                | "application/x-font-woff"
                                                | "application/x-font-ttf"
                                                | "application/x-font-opentype"
                                                | "application/vnd.ms-opentype"
                                        )
                                    {
                                        return Err(format!(
                                            "Font source has unsupported media type {mime}."
                                        ));
                                    }
                                    loaded.events.extend(response.events);
                                    response.body
                                } else {
                                    if loaded.location.url.is_some() {
                                        return Err(
                                            "A web document cannot load local font files.".into()
                                        );
                                    }
                                    navigation::read_limited(
                                        resolved
                                            .path
                                            .as_ref()
                                            .ok_or("Font source has no local file path.")?,
                                        limit,
                                    )?
                                };
                                font_bytes += bytes.len();
                                check_cancel(cancel)?;
                                let parsed = font::Font::parse(&bytes).map(Arc::new);
                                check_cancel(cancel)?;
                                parsed
                            })();
                            cache.insert(key, attempt.clone());
                            attempt
                        })();
                        if !trace_attempted && !trace_reused {
                            if let (Some(address), Err(error)) = (&trace_address, &fetched) {
                                pool.network_trace().blocked(address, "font", error);
                            }
                        }
                        match fetched {
                            Ok(font) => (font, url.chars().take(220).collect::<String>()),
                            Err(error) => {
                                failures.push(format!(
                                    "{}: {error}",
                                    url.chars().take(100).collect::<String>()
                                ));
                                continue;
                            }
                        }
                    }
                };
                loaded.fonts.add_with_ranges(
                    face.family.clone(),
                    face.weight,
                    face.italic,
                    font,
                    face.ranges.clone(),
                )?;
                return Ok(format!(
                    "loaded {description}{}",
                    if failures.is_empty() {
                        String::new()
                    } else {
                        format!("; earlier alternatives: {}", failures.join("; "))
                    }
                ));
            }
            Err(if failures.is_empty() {
                "No supported font source.".into()
            } else {
                failures.join("; ")
            })
        })();
        stats.record("FONT", &label, reused, result, &mut loaded.events);
    }
    if unused > 0 {
        loaded.events.push(format!("FONTS: {unused} face(s) not requested: family or Unicode range is unused by initial visible text."));
    }
    loaded.events.push(format!("FONTS: {} page face(s), {font_bytes} source bytes. TrueType/WOFF1; font-family, weight/style and Unicode-range selection. WOFF2, CFF, variable fonts, shaping and cross-origin font fetches are not implemented; unavailable glyphs fall back.",loaded.fonts.len()));
    Ok(())
}

struct ImageReference {
    node: usize,
    target: String,
    group: usize,
    density: f64,
}
fn image_plan(
    doc: &Document,
    scripting: bool,
    environment: crate::image_selection::Environment,
    events: &mut Vec<String>,
) -> Vec<ImageReference> {
    let inert = inert_nodes(doc, scripting);
    let selector = crate::image_selection::Selector::new(doc, environment);
    let mut plan = Vec::new();
    let mut group = 0;
    for (node, item) in doc.nodes.iter().enumerate() {
        if inert[node] {
            continue;
        }
        let NodeKind::Element { tag, .. } = &item.kind else {
            continue;
        };
        if tag == "script"
            || tag == "link"
                && doc.attr(node, "rel").is_some_and(|value| {
                    value
                        .split_ascii_whitespace()
                        .any(|rel| rel.eq_ignore_ascii_case("stylesheet"))
                })
        {
            group += 1;
        }
        if tag == "img"
            || tag == "input"
                && doc
                    .attr(node, "type")
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("image"))
        {
            let selection = selector.select(node);
            for diagnostic in selection.diagnostics {
                events.push(format!("IMAGE node {node}: {diagnostic}"));
            }
            if let Some(selected) = selection.image {
                if selected.responsive {
                    events.push(format!("IMAGE node {node}: responsive source node {}, density {:.3}x; selected for {:.0} x {:.0} CSS-pixel viewport at {:.3}x. Resize/zoom takes effect on the next load.", selected.source_node, selected.density, environment.viewport_width, environment.viewport_height, environment.density));
                }
                plan.push(ImageReference {
                    node,
                    target: selected.url,
                    group,
                    density: selected.density,
                });
            }
        }
    }
    plan
}

#[allow(clippy::too_many_arguments)]
fn load_images(
    plan: &[ImageReference],
    base: &Location,
    origin: Option<&crate::url::HttpUrl>,
    stats: &mut LoadStats,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
    pool: &http::Pool<'_>,
    mode: http::FetchMode,
    deliver: &mut dyn FnMut(Delivery) -> Result<(), String>,
) -> Result<(), String> {
    let mut cache = HashMap::<ResourceKey, ResourceOutcome>::new();
    let mut prefetched = HashMap::<ResourceKey, Result<http::Response, String>>::new();
    let mut unique_images = 0usize;
    let mut image_bytes = 0usize;
    let mut update = ImageUpdate {
        images: HashMap::new(),
        animations: HashMap::new(),
        events: Vec::new(),
        skipped: 0,
    };
    for (index, image) in plan.iter().enumerate() {
        check_cancel(cancel)?;
        let mut attempted_key = None;
        let mut was_reused = false;
        let mut attempted = false;
        let trace_address = navigation::resolve(&image.target, base)
            .ok()
            .and_then(|location| location.url)
            .map(|url| url.display());
        let mut trace_attempted = false;
        let embedded = if crate::image_data::is_data(&image.target) {
            Some(crate::image_data::prepare(&image.target))
        } else {
            None
        };
        let label: String = match &embedded {
            Some(Ok(image)) => image.label(),
            Some(Err(_)) => "data: (embedded image; payload omitted)".into(),
            None => image.target.chars().take(220).collect(),
        };
        let before_skipped = stats.skipped;
        let result = (|| -> Result<String, String> {
            if image.target.trim().is_empty() {
                return Err("Empty resource reference.".to_owned());
            }
            let embedded = embedded
                .as_ref()
                .map(|r| r.as_ref().map_err(Clone::clone))
                .transpose()?;
            let resolved = if embedded.is_some() {
                None
            } else {
                Some(navigation::resolve(&image.target, base)?)
            };
            if let Some(url) = resolved.as_ref().and_then(|r| r.url.as_ref()) {
                pool.check_resource(url)?;
            }
            let key = if let Some(embedded) = embedded {
                Some(ResourceKey {
                    kind: "IMAGE",
                    address: ResourceAddress::Data(embedded.key()),
                })
            } else {
                resolved.as_ref().and_then(|r| ResourceKey::new("IMAGE", r))
            };
            if let Some(outcome) = key.as_ref().and_then(|key| cache.get(key)).cloned() {
                was_reused = true;
                if let Some(address) = &trace_address {
                    pool.network_trace().reused(
                        address,
                        "image",
                        matches!(outcome, ResourceOutcome::Failed(_)),
                    );
                }
                stats.reused += 1;
                return match outcome {
                    ResourceOutcome::Image(bitmap, animation) => {
                        update.images.insert(image.node, bitmap);
                        if let Some(animation) = animation {
                            update.animations.insert(image.node, animation);
                        }
                        Ok("reused decoded image from this document load".to_owned())
                    }
                    ResourceOutcome::Failed(error) => {
                        stats.reused_failures += 1;
                        Err(format!(
                            "earlier attempt in this document load failed: {error}"
                        ))
                    }
                    ResourceOutcome::Css(..) => unreachable!("image cache only"),
                };
            }
            if unique_images >= MAX_UNIQUE_IMAGES {
                return Err(format!(
                    "Unique image count limit reached ({MAX_UNIQUE_IMAGES})."
                ));
            }
            if image_bytes >= MAX_DECODED_IMAGE_BYTES {
                return Err("Decoded image budget is full (64 MiB).".to_owned());
            }
            let has_prefetched = key.as_ref().is_some_and(|key| prefetched.contains_key(key));
            if !has_prefetched {
                stats.begin_attempt(origin.is_some())?;
            }
            attempted_key = key.clone();
            attempted = true;
            let (bytes, resolved_name) = if let Some(embedded) = embedded {
                (embedded.bytes(&|| check_cancel(cancel))?, embedded.label())
            } else if let Some(url) = resolved.as_ref().and_then(|r| r.url.as_ref()) {
                let origin = origin.ok_or("Local pages do not automatically fetch network resources. Follow a web link explicitly first.")?;
                if !origin.same_origin(url) {
                    return Err("Automatic resources must have the document's scheme, host and port in this build.".to_owned());
                }
                let remaining = stats.remaining_bytes();
                if remaining == 0 && !has_prefetched {
                    return Err("Page resource download budget is full (256 MiB).".to_owned());
                }
                progress(format!("Loading IMAGE {label}"));
                trace_attempted = true;
                let response = if let Some(response) =
                    key.as_ref().and_then(|key| prefetched.remove(key))
                {
                    response?
                } else {
                    let slots = 3usize
                        .saturating_sub(prefetched.len())
                        .max(1)
                        .min(49 - stats.attempts)
                        .min(MAX_UNIQUE_IMAGES - unique_images)
                        .min((remaining / (8 * 1024 * 1024)).max(1));
                    let urls = image_batch_urls(
                        plan,
                        index,
                        base,
                        origin,
                        url,
                        slots,
                        &cache,
                        &prefetched,
                        pool,
                    );
                    stats.attempts += urls.len() - 1;
                    let responses = fetch_image_batch(
                        &urls,
                        (8 * 1024 * 1024).min(remaining),
                        origin,
                        pool,
                        mode,
                    );
                    let mut current = None;
                    for (index, (address, response)) in urls.into_iter().zip(responses).enumerate()
                    {
                        if let Ok(response) = &response {
                            stats.network_bytes += response.body.len();
                        }
                        if index == 0 {
                            current = Some(response);
                        } else {
                            prefetched.insert(
                                ResourceKey {
                                    kind: "IMAGE",
                                    address: ResourceAddress::Web(address),
                                },
                                response,
                            );
                        }
                    }
                    current.expect("image batch includes its current URL")?
                };
                update.events.extend(response.events);
                if response.status < 200 || response.status >= 300 {
                    return Err(format!("HTTP {}", response.status));
                }
                (response.body, response.url.display())
            } else {
                if origin.is_some() {
                    return Err("A web document cannot load local files.".to_owned());
                }
                let path = resolved
                    .as_ref()
                    .ok_or("Missing image resource location.")?
                    .path
                    .as_ref()
                    .ok_or("This resource has no local file path.")?;
                (
                    navigation::read_limited(path, 32 * 1024 * 1024)?,
                    path.display().to_string(),
                )
            };
            check_cancel(cancel)?;
            let check_image = || {
                check_cancel(cancel)?;
                if origin.is_some() && stats.started.elapsed() > Duration::from_secs(45) {
                    return Err("Page resource time budget reached (45 seconds).".to_owned());
                }
                Ok(())
            };
            let mut asset = image::decode_asset_checked(&bytes, &check_image)?;
            let decoded = fit_image_budget(&mut asset, image_bytes)?;
            image_bytes += decoded;
            unique_images += 1;
            let mut message = format!(
                "loaded {} x {} pixels, {}",
                asset.bitmap.width, asset.bitmap.height, resolved_name
            );
            if let Some(animation) = asset.animation {
                message.push_str(&format!("; {}", animation_description(&animation)));
                update.animations.insert(image.node, animation);
            }
            if let Some(notice) = asset.notice {
                message.push_str(&format!("; {notice}"));
            }
            update.images.insert(image.node, asset.bitmap);
            Ok(message)
        })();
        if let Some(key) = attempted_key {
            let outcome = match &result {
                Err(error) => ResourceOutcome::Failed(error.clone()),
                Ok(_) => ResourceOutcome::Image(
                    update.images[&image.node].clone(),
                    update.animations.get(&image.node).cloned(),
                ),
            };
            cache.insert(key, outcome);
        }
        if !trace_attempted && !was_reused {
            if let (Some(address), Err(error)) = (&trace_address, &result) {
                pool.network_trace().blocked(address, "image", error);
            }
        }
        stats.record("IMAGE", &label, was_reused, result, &mut update.events);
        update.skipped += stats.skipped - before_skipped;
        // At most one delivery per physical batch, plus the final repeated refs.
        // Worker::poll coalesces outstanding deltas while the UI is busy.
        if attempted && prefetched.is_empty() {
            publish_images(&mut update, deliver)?;
        }
    }
    publish_images(&mut update, deliver)
}

fn publish_images(
    update: &mut ImageUpdate,
    deliver: &mut dyn FnMut(Delivery) -> Result<(), String>,
) -> Result<(), String> {
    if !update.images.is_empty() || !update.events.is_empty() || update.skipped != 0 {
        let ready = std::mem::replace(
            update,
            ImageUpdate {
                images: HashMap::new(),
                animations: HashMap::new(),
                events: Vec::new(),
                skipped: 0,
            },
        );
        deliver(Delivery::Images(ready))?;
    }
    Ok(())
}

fn animation_description(animation: &Animation) -> String {
    let plays = animation
        .plays
        .map_or_else(|| "repeating".to_owned(), |n| format!("{n} total play(s)"));
    format!(
        "ANIMATION: {} frame(s), {plays}; file delays with a 20 ms minimum",
        animation.frames.len()
    )
}

fn fit_image_budget(asset: &mut image::DecodedImage, used: usize) -> Result<usize, String> {
    let available = MAX_DECODED_IMAGE_BYTES.saturating_sub(used);
    let mut bytes = asset.retained_bytes()?;
    if bytes > available && asset.animation.is_some() {
        asset.animation = None;
        asset.notice = Some(
            "Animation kept static to stay within the document's 64 MiB decoded image budget."
                .into(),
        );
        bytes = asset.retained_bytes()?;
    }
    if bytes > available {
        return Err("Decoded image budget exceeded (64 MiB).".into());
    }
    Ok(bytes)
}

/// Look ahead through the original image run only, keeping CSS/script barriers.
#[allow(clippy::too_many_arguments)]
fn image_batch_urls(
    plan: &[ImageReference],
    current_index: usize,
    base: &Location,
    origin: &crate::url::HttpUrl,
    current: &crate::url::HttpUrl,
    slots: usize,
    cache: &HashMap<ResourceKey, ResourceOutcome>,
    pending: &HashMap<ResourceKey, Result<http::Response, String>>,
    pool: &http::Pool<'_>,
) -> Vec<crate::url::HttpUrl> {
    let mut urls = vec![current.clone()];
    if slots < 2 {
        return urls;
    }
    for image in &plan[current_index + 1..] {
        if image.group != plan[current_index].group {
            break;
        }
        if image.target.trim().is_empty() {
            continue;
        }
        let Ok(location) = navigation::resolve(&image.target, base) else {
            continue;
        };
        let Some(url) = location.url else {
            continue;
        };
        if !origin.same_origin(&url) || urls.contains(&url) || pool.check_resource(&url).is_err() {
            continue;
        }
        let key = ResourceKey {
            kind: "IMAGE",
            address: ResourceAddress::Web(url.clone()),
        };
        if cache.contains_key(&key) || pending.contains_key(&key) {
            continue;
        }
        urls.push(url);
        if urls.len() == slots {
            break;
        }
    }
    urls
}

fn fetch_image_batch(
    urls: &[crate::url::HttpUrl],
    limit: usize,
    origin: &crate::url::HttpUrl,
    pool: &http::Pool<'_>,
    mode: http::FetchMode,
) -> Vec<Result<http::Response, String>> {
    if urls.len() == 1 {
        return vec![pool.fetch_kind(&urls[0], limit, Some(origin), mode, "image")];
    }
    std::thread::scope(|scope| {
        let workers: Vec<_> = urls
            .iter()
            .map(|url| {
                std::thread::Builder::new()
                    .name("image-fetch".to_owned())
                    .spawn_scoped(scope, move || {
                        pool.fetch_kind(url, limit, Some(origin), mode, "image")
                    })
            })
            .collect();
        // Completion is merged in document order regardless of wire arrival order.
        workers
            .into_iter()
            .map(|worker| match worker {
                Ok(worker) => worker
                    .join()
                    .unwrap_or_else(|_| Err("Image loading worker failed.".to_owned())),
                Err(error) => Err(format!("Cannot start image loading worker: {error}")),
            })
            .collect()
    })
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Loading stopped.".to_owned())
    } else {
        Ok(())
    }
}
fn inert_nodes(doc: &Document, noscript: bool) -> Vec<bool> {
    let mut inert = vec![false; doc.nodes.len()];
    for (i, node) in doc.nodes.iter().enumerate() {
        if inert[i]
            || matches!(&node.kind, NodeKind::Element { tag, .. } if matches!(tag.as_str(), "template" | "iframe" | "object") || noscript && tag == "noscript")
        {
            inert[i] = true;
            for child in &node.children {
                inert[*child] = true;
            }
        }
    }
    inert
}

fn check_script(doc: &Document, node: usize) -> Result<(), String> {
    let kind = if let Some(kind) = doc.attr(node, "type") {
        kind.trim().to_ascii_lowercase()
    } else if let Some(language) = doc.attr(node, "language").filter(|v| !v.trim().is_empty()) {
        format!("text/{}", language.trim().to_ascii_lowercase())
    } else {
        String::new()
    };
    if !kind.is_empty() && !javascript_mime(&kind) {
        return Err(if kind == "module" {
            "JavaScript modules are not implemented.".to_owned()
        } else {
            format!(
                "Non-classic script type {} is inert.",
                kind.chars().take(80).collect::<String>()
            )
        });
    }
    if doc.attr(node, "for").is_some() || doc.attr(node, "event").is_some() {
        return Err("Legacy script for/event attributes are not implemented.".to_owned());
    }
    if doc.attr(node, "src").is_some() {
        if doc.attr(node, "async").is_some() {
            return Err("Async external scripts are not implemented.".to_owned());
        }
        if doc
            .attr(node, "integrity")
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err("Subresource Integrity is not implemented; the declared-integrity script was not fetched.".to_owned());
        }
    }
    Ok(())
}

fn script_text(doc: &Document, node: usize, capacity: crate::javascript::limits::Capacities) -> Result<String, String> {
    let mut source = String::new();
    for &child in &doc.nodes[node].children {
        if let NodeKind::Text(text) = &doc.nodes[child].kind {
            check_script_size(text.len(), source.len(), capacity)?;
            if text.len() > capacity.source_bytes.saturating_sub(source.len()) {
                return Err(format!("A script source exceeds the {} KiB limit.", capacity.source_bytes / 1024));
            }
            source.push_str(text);
        }
    }
    Ok(source)
}

fn check_script_size(bytes: usize, total: usize, capacity: crate::javascript::limits::Capacities) -> Result<(), String> {
    if bytes > capacity.source_bytes {
        return Err(format!("A script source exceeds the {} KiB limit.", capacity.source_bytes / 1024));
    }
    if bytes > capacity.retained_source.saturating_sub(total) {
        return Err(format!("Script source budget exceeded ({} MiB).", capacity.retained_source / (1024 * 1024)));
    }
    Ok(())
}

/// The HTML type attribute uses an essence match, so parameters there reject.
/// HTTP Content-Type is parsed separately before calling this helper.
pub(crate) fn javascript_mime(mime: &str) -> bool {
    matches!(
        mime,
        "application/ecmascript"
            | "application/javascript"
            | "application/x-ecmascript"
            | "application/x-javascript"
            | "text/ecmascript"
            | "text/javascript"
            | "text/javascript1.0"
            | "text/javascript1.1"
            | "text/javascript1.2"
            | "text/javascript1.3"
            | "text/javascript1.4"
            | "text/javascript1.5"
            | "text/jscript"
            | "text/livescript"
            | "text/x-ecmascript"
            | "text/x-javascript"
    )
}

pub(crate) fn media_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn image_file_type(path: &std::path::Path) -> Option<&'static str> {
    // Recognition routes unsupported formats to a decoder error, never HTML.
    Some(
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "apng" => "image/apng",
            "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
            "gif" => "image/gif",
            "bmp" | "dib" => "image/bmp",
            "ico" | "cur" => "image/vnd.microsoft.icon",
            "svg" => "image/svg+xml",
            "ppm" => "image/x-portable-pixmap",
            "pbm" => "image/x-portable-bitmap",
            "pgm" => "image/x-portable-graymap",
            "pnm" => "image/x-portable-anymap",
            "pam" => "image/x-portable-arbitrarymap",
            "qoi" => "image/qoi",
            "webp" => "image/webp",
            "avif" => "image/avif",
            "jxl" => "image/jxl",
            "tif" | "tiff" => "image/tiff",
            "tga" => "image/x-tga",
            "pcx" => "image/x-pcx",
            "heic" => "image/heic",
            "heif" => "image/heif",
            _ => return None,
        },
    )
}

fn image_document_markup(address: &str, bytes: usize, bitmap: Option<&Bitmap>) -> String {
    // This image node deliberately has no src: its already-decoded pixels are
    // attached directly. Opening an image must never refetch it as an asset.
    let name = address
        .split(['?', '#'])
        .next()
        .unwrap_or(address)
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("Image");
    let name = escape(&name.chars().take(160).collect::<String>());
    let detail = bitmap.map_or_else(
        || "Image display is disabled in Settings.".to_owned(),
        |image| {
            format!(
                "{} × {} pixels · {} bytes · Ctrl+plus/minus zooms",
                image.width, image.height, bytes
            )
        },
    );
    format!("<!doctype html><html><head><title>{name} — Image</title><style>body{{margin:0;padding:24px;background:#eef2f3;color:#183444;font-family:system-ui,sans-serif}}h1{{font-size:20px;margin:0 0 8px}}p{{font-size:14px;margin:0 0 24px}}img{{display:block;margin:auto;max-width:100%;height:auto;background:#ffffff}}</style></head><body><h1>{name}</h1><p>{detail}</p><img alt='Image display disabled'></body></html>")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn charset(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['\'', '"']).to_ascii_lowercase())
    })
}
pub(crate) fn decode_text(
    bytes: &[u8],
    content_type: &str,
    sniff_html: bool,
) -> Result<String, String> {
    if let Some(bytes) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return Ok(String::from_utf8_lossy(bytes).into_owned());
    }
    let mut encoding = charset(content_type);
    if encoding.is_none() && sniff_html {
        let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
        let preview = html::parse(&prefix);
        for (index, node) in preview.nodes.iter().enumerate() {
            if matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "meta") {
                if let Some(value) = preview.attr(index, "charset") {
                    encoding = Some(value.trim().to_ascii_lowercase());
                    break;
                }
                if preview
                    .attr(index, "http-equiv")
                    .is_some_and(|v| v.eq_ignore_ascii_case("content-type"))
                {
                    encoding = preview.attr(index, "content").and_then(charset);
                    if encoding.is_some() {
                        break;
                    }
                }
            }
        }
    }
    let encoding = encoding.unwrap_or_else(|| {
        if std::str::from_utf8(bytes).is_ok() {
            "utf-8".to_owned()
        } else {
            "windows-1252".to_owned()
        }
    });
    match encoding.as_str() {
        "utf-8" | "utf8" => Ok(String::from_utf8_lossy(bytes).into_owned()),
        "windows-1252" | "cp1252" | "iso-8859-1" | "iso8859-1" | "latin1" | "latin-1"
        | "us-ascii" | "ascii" => {
            const HIGH: [char; 32] = [
                '€', '\u{fffd}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{fffd}',
                'Ž', '\u{fffd}', '\u{fffd}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›',
                'œ', '\u{fffd}', 'ž', 'Ÿ',
            ];
            Ok(bytes
                .iter()
                .map(|b| {
                    if (0x80..0xa0).contains(b) {
                        HIGH[(b - 0x80) as usize]
                    } else {
                        char::from(*b)
                    }
                })
                .collect())
        }
        _ => Err(format!("Text encoding {encoding} is not supported yet.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn responsive_sources_data_urls_density_and_image_preferences_share_the_loader() {
        let folder = Folder::new();
        folder.file("small.ppm", "P3 2 1 255 255 0 0 255 0 0");
        folder.file(
            "large.ppm",
            "P3 4 2 255 0 255 0 0 255 0 0 255 0 0 255 0 0 255 0 0 255 0 0 255 0 0 255 0",
        );
        let page=folder.page("<picture><source type='image/avif' srcset='unsupported.avif'><source type='image/x-portable-pixmap' media='(min-width:700px)' srcset='large.ppm 2x'><img id=p src='fallback-must-not-load.ppm' srcset='small.ppm 1x'></picture><img id=d src='data:image/x-portable-pixmap,P3%202%201%20255%20255%200%200%20255%200%200'><img id=r src='data:image/x-portable-pixmap,P3%202%201%20255%20255%200%200%20255%200%200#same'><img id=bad src='data:text/html,%3Cscript%3Epayload-must-be-omitted'><img id=x src='data:image/x-portable-pixmap,P3%202%201%20255%200%200%20255%200%200%20255' srcset='data:image/x-portable-pixmap,P3%202%201%20255%20255%200%200%20255%200%200 1x'>");
        let loaded = local_with_options(
            &page,
            LoadOptions {
                viewport_width: 800,
                ..Default::default()
            },
        )
        .unwrap();
        let node = |id| {
            loaded
                .document
                .nodes
                .iter()
                .enumerate()
                .find(|(n, _)| loaded.document.attr(*n, "id") == Some(id))
                .unwrap()
                .0
        };
        assert_eq!(loaded.images.len(), 4);
        assert_eq!(loaded.image_densities[&node("p")], 2.0);
        assert_eq!(
            (
                loaded.images[&node("p")].width,
                loaded.images[&node("p")].height
            ),
            (4, 2)
        );
        assert!(Arc::ptr_eq(
            &loaded.images[&node("d")],
            &loaded.images[&node("r")]
        ));
        assert!(Arc::ptr_eq(
            &loaded.images[&node("d")],
            &loaded.images[&node("x")]
        ));
        assert_eq!(loaded.skipped, 1);
        assert!(!loaded
            .events
            .iter()
            .any(|event| event.contains("payload-must-be-omitted")
                || event.contains("P3%20")
                || event.contains("fallback-must")
                || event.contains("unsupported.avif")));
        let narrow = local_with_options(
            &page,
            LoadOptions {
                viewport_width: 500,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(narrow.images[&node("p")].width, 2);
        assert_eq!(narrow.image_densities[&node("p")], 1.0);
        let disabled = local_with_options(
            &page,
            LoadOptions {
                images: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(disabled.images.is_empty());
        assert!(!disabled
            .events
            .iter()
            .any(|event| event.contains("decoded image") || event.contains("not enough")));
    }

    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "chezcat-scripts-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn file(&self, name: &str, value: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, value).unwrap();
            path
        }
        fn page(&self, value: &str) -> Location {
            navigation::explicit(&self.file("page.html", value)).unwrap()
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn animation_budget_counts_shared_frames_once_and_keeps_a_valid_poster() {
        let poster = Arc::new(Bitmap {
            width: 2,
            height: 2,
            pixels: vec![0xff123456; 4],
        });
        let other = Arc::new(Bitmap {
            width: 2,
            height: 2,
            pixels: vec![0xffabcdef; 4],
        });
        let animation = Arc::new(Animation {
            frames: vec![
                image::ImageFrame {
                    bitmap: poster.clone(),
                    delay_ms: 50,
                },
                image::ImageFrame {
                    bitmap: other,
                    delay_ms: 50,
                },
                image::ImageFrame {
                    bitmap: poster.clone(),
                    delay_ms: 50,
                },
            ],
            plays: None,
        });
        let mut asset = image::DecodedImage {
            bitmap: poster,
            animation: Some(animation),
            notice: None,
        };
        assert_eq!(
            fit_image_budget(&mut asset, MAX_DECODED_IMAGE_BYTES - 32).unwrap(),
            32
        );
        assert!(asset.animation.is_some() && asset.notice.is_none());
        assert_eq!(
            fit_image_budget(&mut asset, MAX_DECODED_IMAGE_BYTES - 16).unwrap(),
            16
        );
        assert!(asset.animation.is_none());
        assert!(asset.notice.as_ref().unwrap().contains("kept static"));
        assert_eq!(asset.bitmap.pixels, [0xff123456; 4]);
        assert!(fit_image_budget(&mut asset, MAX_DECODED_IMAGE_BYTES - 15).is_err());
    }

    #[test]
    fn small_local_icons_no_longer_hit_the_old_attempt_limit() {
        let folder = Folder::new();
        let mut source = String::new();
        for index in 0..50 {
            let name = format!("icon-{index}.ppm");
            folder.file(&name, &format!("P3\n1 1\n255\n{index} 60 100\n"));
            source.push_str(&format!("<img src={name}>"));
        }
        let loaded = local(&folder.page(&source)).unwrap();
        assert_eq!(loaded.images.len(), 50);
        assert_eq!(loaded.skipped, 0);
        assert_eq!(
            loaded
                .images
                .values()
                .map(|bitmap| bitmap.pixels.len() * 4)
                .sum::<usize>(),
            200
        );
        assert!(!loaded
            .events
            .iter()
            .any(|event| event.contains("Resource attempt limit reached")));
        assert!(!loaded
            .events
            .iter()
            .any(|event| event.contains("Unique image count limit")));
    }

    #[test]
    fn image_inputs_share_image_resources_and_inert_rules() {
        let folder = Folder::new();
        folder.file("pixel.ppm", "P3\n1 1\n255\n20 60 100\n");
        let page = folder.page(
            "<input type=ImAgE src=pixel.ppm><img src='pixel.ppm#same'>\
             <input src=missing.ppm><input type='' src=missing.ppm>\
             <input type=text src=missing.ppm><input type='image ' src=missing.ppm>\
             <template><input type=image src=missing.ppm></template>\
             <noscript><input type=image src=pixel.ppm></noscript>",
        );
        for scripts in [true, false] {
            let loaded = local_with_scripts(&page, scripts).unwrap();
            assert_eq!(loaded.images.len(), if scripts { 2 } else { 3 });
            assert_eq!(loaded.skipped, 0);
            let first = loaded.images.values().next().unwrap();
            assert!(loaded
                .images
                .values()
                .all(|bitmap| Arc::ptr_eq(first, bitmap)));
        }
    }

    #[test]
    fn classic_script_sources_keep_order_raw_text_and_local_boundaries() {
        let folder = Folder::new();
        folder.file("app.js", "var external = 2;");
        let page = folder.page("<script>var first = '<b>&amp;</b>';</script><script defer src=app.js></script><script async>var inlineAsync = 3;</script><script nomodule>var fallback = 4;</script><script type=application/json>{\"data\":1}</script><script type='text/javascript;charset=utf-8'>bad()</script><script async src=missing.js></script><script integrity=sha256-missing src=missing.js></script><template><script src=missing.js></script><meta http-equiv=Content-Security-Policy content=none></template><noscript><script src=missing.js></script></noscript><script src=../private.js></script>");
        let loaded = local(&page).unwrap();
        assert!(loaded.scripting_blocked.is_none());
        let sources: Vec<_> = loaded.scripts.iter().map(|s| s.source.as_str()).collect();
        assert_eq!(
            sources,
            [
                "var first = '<b>&amp;</b>';",
                "var external = 2;",
                "var inlineAsync = 3;",
                "var fallback = 4;"
            ]
        );
        assert!(loaded.scripts.windows(2).all(|s| s[0].node < s[1].node));
        assert!(loaded.events.iter().any(|e| e.contains("Async external")));
        assert!(loaded
            .events
            .iter()
            .any(|e| e.contains("Subresource Integrity")));
        assert!(loaded
            .events
            .iter()
            .any(|e| e.contains("text/javascript;charset=utf-8")));
        assert_eq!(loaded.skipped, 5);
    }

    #[test]
    fn page_fonts_resolve_from_css_reuse_sources_skip_unused_ranges_and_obey_csp() {
        let folder = Folder::new();
        std::fs::create_dir(folder.0.join("css")).unwrap();
        std::fs::create_dir(folder.0.join("fonts")).unwrap();
        std::fs::write(
            folder.0.join("fonts/original.ttf"),
            crate::font::tests::fixture_font(),
        )
        .unwrap();
        folder.file("css/site.css", "@font-face{font-family:'Page Face';src:url(ignored.woff2) format(woff2),url('../fonts/original.ttf') format(truetype);unicode-range:U+0-FF}@font-face{font-family:'Page Face';font-weight:700;src:url('../fonts/original.ttf')}@font-face{font-family:'Page Face';src:url(unused.ttf);unicode-range:U+400-4FF}@font-face{font-family:Invisible;src:url(not-requested.ttf)}p{font-family:'Page Face'}");
        let source =
            "<link rel=stylesheet href=css/site.css><p>AV</p><p style='font-weight:700'>AV</p>";
        let loaded = local(&folder.page(source)).unwrap();
        assert_eq!(loaded.fonts.len(), 2);
        assert_eq!(loaded.skipped, 0);
        let plain = loaded
            .fonts
            .select(&["Page Face".into()], 400, false)
            .unwrap();
        let bold = loaded
            .fonts
            .select(&["Page Face".into()], 700, false)
            .unwrap();
        assert!(Arc::ptr_eq(&plain.font, &bold.font));
        assert!(loaded
            .events
            .iter()
            .any(|event| event.contains("format(woff2) is not implemented")));
        assert!(loaded
            .events
            .iter()
            .any(|event| event.contains("2 face(s) not requested")));
        let blocked = local(&folder.page(&format!(
            "{source}<meta http-equiv=Content-Security-Policy content=\"font-src 'none'\">"
        )))
        .unwrap();
        assert_eq!(blocked.fonts.len(), 0);
        assert!(blocked
            .events
            .iter()
            .any(|event| event.contains("font-src/default-src blocked")));
        folder.file(
            "css/escape.css",
            "@font-face{font-family:Escape;src:url(../../outside.ttf)}p{font-family:Escape}",
        );
        let escaped =
            local(&folder.page("<link rel=stylesheet href=css/escape.css><p>A</p>")).unwrap();
        assert_eq!(escaped.fonts.len(), 0);
        assert!(escaped
            .events
            .iter()
            .any(|event| event.starts_with("FONT Escape") && event.contains("SKIPPED")));
    }

    #[test]
    fn csp_filters_scripts_without_changing_the_user_scripting_flag() {
        let folder = Folder::new();
        folder.file("pixel.ppm", "P3\n1 1\n255\n255 0 0\n");
        folder.file("blocked.js", "throw 'CSP must block this source';");
        let source = "<script>var okay = 1;</script><noscript><img src=pixel.ppm><script>never()</script></noscript>";
        let page = folder.page(source);
        let enabled = local(&page).unwrap();
        assert_eq!(enabled.scripts.len(), 1);
        assert!(enabled.images.is_empty());
        let disabled = local_with_scripts(&page, false).unwrap();
        assert!(disabled.scripts.is_empty());
        assert!(disabled
            .scripting_blocked
            .as_deref()
            .unwrap()
            .contains("by user"));
        assert_eq!(disabled.images.len(), 1);
        let page = folder.page(&format!("{source}<meta HTTP-EQUIV=' Content-Security-Policy ' content=\"script-src 'none'\"><script src=blocked.js></script><script>blocked()</script>"));
        let csp = local(&page).unwrap();
        assert_eq!(csp.scripts.len(), 1);
        assert!(csp.scripting_blocked.is_none());
        assert!(csp.images.is_empty());
        assert!(csp
            .events
            .iter()
            .any(|event| event.contains("Content Security Policy blocked")));
        assert!(!csp.events.iter().any(|e| e.contains("Cannot resolve file")));
        let page = folder.page("<meta http-equiv=Content-Security-Policy-Report-Only content=none><noscript><meta http-equiv=Content-Security-Policy content=none></noscript><script>okay()</script>");
        let report_only = local(&page).unwrap();
        assert!(report_only.scripting_blocked.is_none());
        assert_eq!(report_only.scripts.len(), 1);
    }

    #[test]
    fn linked_style_csp_precedes_reuse_and_cors_scripts_are_not_fetched() {
        let folder = Folder::new();
        folder.file("one.css", "body { color: red; }");
        folder.file("cors.js", "throw 'unsupported CORS must not execute';");
        let page = folder.page("<meta http-equiv=Content-Security-Policy content=\"style-src 'nonce-One'\"><link rel=stylesheet nonce=One href=one.css><link rel=stylesheet href=one.css><link rel=stylesheet nonce=One nonce=Two href=one.css><script crossorigin src=cors.js></script>");
        let loaded = local(&page).unwrap();
        assert_eq!(loaded.styles.len(), 1);
        assert!(loaded.scripts.is_empty());
        assert!(loaded
            .events
            .iter()
            .any(|e| e.contains("blocked this stylesheet")));
        assert!(loaded.events.iter().any(|e| e.contains("CORS-mode")));
        assert!(!loaded
            .events
            .iter()
            .any(|e| e.contains("reused stylesheet")));
    }

    #[test]
    fn browser_script_loading_crosses_old_count_and_source_limits() {
        let folder = Folder::new();
        let source = format!("/*{}*/42;", "x".repeat(513 * 1024));
        folder.file("large.js", &source);
        let page = folder.page(&format!("<script src=large.js></script>{}", "<script>1;</script>".repeat(140)));
        let loaded = local(&page).unwrap();
        assert_eq!(loaded.scripts.len(), 141);
        assert_eq!(loaded.scripts[0].source, source);
        assert_eq!(loaded.skipped, 0, "{:?}", loaded.events);
    }

    #[test]
    fn script_source_count_and_byte_budgets_are_independent() {
        let folder = Folder::new();
        let capacity = crate::javascript::limits::REGRESSION_CAPACITIES;
        let local = |location: &Location| local_with_options(location, LoadOptions {
            script_capacity: capacity, ..LoadOptions::default()
        });
        let page = folder.page(&"<script>1;</script>".repeat(capacity.programs + 1));
        let loaded = local(&page).unwrap();
        assert_eq!(loaded.scripts.len(), capacity.programs);
        assert_eq!(loaded.skipped, 1);
        let large = "x".repeat(capacity.source_bytes);
        let page = folder.page(&format!("<script>{large}x</script><script>1;</script>"));
        let loaded = local(&page).unwrap();
        assert_eq!(loaded.scripts.len(), 1);
        assert!(loaded.events.iter().any(|e| e.contains("512 KiB")));
        folder.file("large.js", &large);
        let page = folder.page(&"<script src=large.js></script>".repeat(5));
        let loaded = local(&page).unwrap();
        assert_eq!(loaded.scripts.len(), 4);
        assert_eq!(
            loaded.scripts.iter().map(|s| s.source.len()).sum::<usize>(),
            capacity.retained_source
        );
        assert!(loaded.events.iter().any(|e| e.contains("2 MiB")));
    }

    struct Site {
        location: Location,
        requests: Arc<Mutex<Vec<String>>>,
        image_gate: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Site {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let seen = requests.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let ending = stop.clone();
            let image_gate = Arc::new(AtomicBool::new(false));
            let release_image = image_gate.clone();
            let thread = std::thread::spawn(move || {
                while !ending.load(Ordering::Relaxed) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(pair) => pair,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2));
                            continue;
                        }
                        Err(error) => panic!("{error}"),
                    };
                    // Accepted Winsock sockets inherit the polling listener mode.
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while request.len() < 16 * 1024 && !request.ends_with(b"\r\n\r\n") {
                        if stream.read(&mut byte).unwrap_or(0) == 0 {
                            break;
                        }
                        request.push(byte[0]);
                    }
                    // A cancelled connection has no routable HTTP request.
                    if !request.ends_with(b"\r\n\r\n") {
                        continue;
                    }
                    let mut text = String::from_utf8(request).unwrap();
                    let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    let body_length = text
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    assert!(body_length <= http::MAX_FORM_BYTES);
                    let mut body = vec![0; body_length];
                    stream.read_exact(&mut body).unwrap();
                    text.push_str(std::str::from_utf8(&body).unwrap());
                    seen.lock().unwrap().push(text);
                    if path == "/assets/page-font.ttf" {
                        let bytes = crate::font::tests::fixture_font();
                        let _ = write!(stream,"HTTP/1.1 200 Test\r\nContent-Type: font/ttf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len());
                        let _ = stream.write_all(&bytes);
                        continue;
                    }
                    if path == "/held.ppm" {
                        while !release_image.load(Ordering::Relaxed)
                            && !ending.load(Ordering::Relaxed)
                        {
                            std::thread::sleep(Duration::from_millis(2));
                        }
                    }
                    let (mut status, mut mime, mut extra) =
                        (200, "text/html;charset=utf-8", String::new());
                    let body = match path.as_str() {
                        "/font-page" => "<link rel=stylesheet href=/font-redirect.css><link rel=stylesheet href=/font-redirect.css><p style='font-family:Page'>AV</p><p style='font-family:Other'>A</p>".into(),
                        "/font-blocked" => { extra="Content-Security-Policy: default-src 'self'; font-src 'none'\r\n".into(); "<link rel=stylesheet href=/font-redirect.css><p style='font-family:Page'>AV</p>".into() },
                        "/font-redirect.css" => { status=302; extra="Location: /assets/font.css\r\n".into(); String::new() },
                        "/assets/font.css" => { mime="text/css"; format!("@font-face{{font-family:Page;src:url(page-font.ttf) format(truetype)}}@font-face{{font-family:Other;src:url(http://localhost:{port}/must-not-fetch.ttf)}}") },
                        "/cors-page" => format!("<script crossorigin=anonymous src=/ok.js></script><script crossorigin=use-credentials src=/redirect.js></script><script crossorigin=invalid src=/ok.js></script><script crossorigin src=/bad.js></script><script crossorigin src='http://localhost:{port}/unfetched.js'></script><script crossorigin src=/foreign-redirect.js></script><script crossorigin integrity=declared src=/unfetched.js></script><script type=text/rms src=/unfetched.js></script>"),
                        "/cors-blocked" => { extra="Content-Security-Policy: script-src 'none'\r\n".into(); "<script crossorigin src=/ok.js></script>".into() },
                        "/page" => format!("<script>var a=1;</script><script src=/ok.js></script><script src=/redirect.js></script><script src=/bad.js></script><script src=/no-type.js></script><script src='http://localhost:{port}/foreign.js'></script><script src=/foreign-redirect.js></script><script integrity=declared src=/unfetched.js></script><script async src=/async.js></script><script type=module src=/module.js></script>"),
                        "/ok.js" | "/last.js" => { mime = "text/javascript;charset=UTF-8"; "var external=2;".to_owned() },
                        "/redirect.js" => { status=302; extra="Location: /last.js\r\n".into(); String::new() },
                        "/foreign-redirect.js" => { status=302; extra=format!("Location: http://localhost:{port}/unfetched.js\r\n"); String::new() },
                        "/bad.js" => "<html>This is not JavaScript</html>".to_owned(),
                        "/no-type.js" => { mime=""; "var missingType=1;".to_owned() },
                        "/csp" => { extra="Content-Security-Policy: script-src 'self'\r\n".into(); "<script src=/ok.js></script><script>blocked()</script>".into() },
                        "/csp-nonce" => { extra="Content-Security-Policy: script-src 'nonce-One' 'strict-dynamic' 'self' 'unsafe-inline'\r\n".into(); "<script nonce=One src=/redirect.js></script><script nonce=Wrong src=/unfetched.js></script><script nonce=One>var nonceOkay=1;</script><script>blocked()</script>".into() },
                        "/csp-intersection" => { extra="Content-Security-Policy: script-src 'nonce-One'\r\nContent-Security-Policy: script-src 'none'\r\n".into(); "<script nonce=One src=/unfetched.js></script><script nonce=One>blocked()</script>".into() },
                        "/csp-base" => { extra="Content-Security-Policy: script-src 'nonce-One'\r\n".into(); "<head><meta http-equiv=Content-Security-Policy content=\"base-uri 'none'\"><base href=/unapproved/><script nonce=One src=ok.js></script></head>".into() },
                        "/csp-duplicate" => { extra="Content-Security-Policy: script-src 'nonce-One' 'strict-dynamic'\r\n".into(); "<script nonce=One nonce=Two src=/unfetched.js></script><script nonce=One src=/unfetched.js SRC=/ok.js></script><script nonce=One x='&lt;SCRIPT' src=/unfetched.js></script>".into() },
                        "/csp-sandbox" => { extra="Content-Security-Policy: sandbox allow-scripts; script-src * 'unsafe-inline'\r\n".into(); "<script src=/unfetched.js></script><script>blocked()</script><noscript><img src=/unfetched.ppm></noscript>".into() },
                        "/report" => { extra="Content-Security-Policy-Report-Only: script-src 'none'\r\n".into(); "<script src=/ok.js></script>".into() },
                        "/post?kept=1" | "/post-result" => "<title>Form result</title><link rel=stylesheet href=/post-style.css><img src=/post-image.ppm><script src=/ok.js></script><p>Result</p>".into(),
                        "/settings" => "<title>Load settings</title><link rel=stylesheet href=/post-style.css><img src=/post-image.ppm><input type=image src=/post-image.ppm><script src=/ok.js></script><noscript><img src=/post-image.ppm></noscript>".into(),
                        "/post-303" => { status=303; extra="Location: /post-result#finished\r\n".into(); String::new() },
                        "/post-307" => { status=307; extra="Location: /post?kept=1\r\n".into(); String::new() },
                        "/post-style.css" => { mime="text/css"; "p{color:#123456}".into() },
                        "/staged" => "<title>Staged page</title><img src=/held.ppm><link rel=stylesheet href=/post-style.css><script src=/ok.js></script><p>Visible before its image</p>".into(),
                        "/fresh" => "<title>Fresh page</title><p>Replacement</p>".into(),
                        "/script-nav-redirect" => { status=302; extra="Location: /fresh\r\n".into(); String::new() },
                        "/script-nav-cross" => { status=302; extra=format!("Location: http://localhost:{port}/fresh\r\n"); String::new() },
                        "/repeat-resources" => format!("<link rel=stylesheet href=/post-style.css><style>p{{color:#abcdef}}</style><link rel=stylesheet href='./post-style.css#again'><p id=result>Repeat</p>{}{}<img src='/post-image.ppm#after-many-references'><script src=/ok.js></script><script src=/ok.js></script>", "<img src=/post-image.ppm>".repeat(40), "<img src=/unsupported.gif>".repeat(60)),
                        "/unsupported.gif" => { mime="image/gif"; "GIF89a-not-an-implemented-format".into() },
                        "/post-image.ppm" | "/held.ppm" => { mime="image/x-portable-pixmap"; "P3\n1 1\n255\n255 0 0\n".into() },
                        _ => panic!("Unexpected resource request {path}"),
                    };
                    let content_type = if mime.is_empty() {
                        String::new()
                    } else {
                        format!("Content-Type: {mime}\r\n")
                    };
                    let response = format!("HTTP/1.1 {status} Test\r\n{content_type}{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            let location =
                navigation::address(&format!("http://127.0.0.1:{port}/page"), &Location::home())
                    .unwrap();
            Self {
                location,
                requests,
                image_gate,
                stop,
                thread: Some(thread),
            }
        }
        fn load(&self, path: &str, mode: http::FetchMode, enabled: bool) -> Loaded {
            let location = navigation::resolve(path, &self.location).unwrap();
            load(
                &location,
                &AtomicBool::new(false),
                &|_| {},
                &trust::Store::unconfigured(),
                mode,
                enabled,
            )
            .unwrap()
        }
    }
    impl Drop for Site {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
            if let Err(failure) = thread.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(failure);
                }
            }
        }
        }
    }

    #[test]
    fn loader_fixture_waits_for_fragmented_requests_and_ignores_cancelled_connections() {
        let site = Site::new();
        let url = site.location.url.as_ref().expect("loopback fixture URL");
        let address = format!("{}:{}", url.host, url.port);
        drop(std::net::TcpStream::connect(&address).unwrap());
        let mut client = std::net::TcpStream::connect(&address).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        client.write_all(b"GET /fre").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        client.write_all(b"sh HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 Test\r\n"), "{response}");
        assert!(response.contains("Fresh page"), "{response}");
        assert_eq!(site.requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn script_network_loading_enforces_mime_origin_integrity_csp_and_full_reload() {
        let site = Site::new();
        let loaded = site.load("/page", http::FetchMode::FullReload, true);
        assert_eq!(
            loaded
                .scripts
                .iter()
                .map(|s| s.source.as_str())
                .collect::<Vec<_>>(),
            ["var a=1;", "var external=2;", "var external=2;"]
        );
        let requests = site.requests.lock().unwrap().clone();
        assert_eq!(
            requests
                .iter()
                .map(|r| r.split_whitespace().nth(1).unwrap())
                .collect::<Vec<_>>(),
            [
                "/page",
                "/ok.js",
                "/redirect.js",
                "/last.js",
                "/bad.js",
                "/no-type.js",
                "/foreign-redirect.js"
            ]
        );
        assert!(requests
            .iter()
            .all(|r| r.contains("\r\nCache-Control: no-cache\r\n")
                && r.contains("\r\nPragma: no-cache\r\n")));
        assert!(loaded.events.iter().any(|e| e.contains("Content-Type")));
        let before = site.requests.lock().unwrap().len();
        let blocked = site.load("/csp", http::FetchMode::Normal, true);
        assert!(blocked.scripting_blocked.is_none());
        assert_eq!(blocked.scripts.len(), 1);
        assert!(blocked
            .events
            .iter()
            .any(|event| event.contains("Content Security Policy blocked")));
        assert_eq!(site.requests.lock().unwrap().len(), before + 2);
        let before = site.requests.lock().unwrap().len();
        let nonce = site.load("/csp-nonce", http::FetchMode::Normal, true);
        assert_eq!(nonce.scripts.len(), 2);
        assert_eq!(site.requests.lock().unwrap().len(), before + 3);
        let before = site.requests.lock().unwrap().len();
        let intersection = site.load("/csp-intersection", http::FetchMode::Normal, true);
        assert!(intersection.scripts.is_empty());
        assert!(intersection.scripting_blocked.is_none());
        assert_eq!(site.requests.lock().unwrap().len(), before + 1);
        let before = site.requests.lock().unwrap().len();
        let off = site.load("/page", http::FetchMode::Normal, false);
        assert!(off.scripting_blocked.is_some() && off.scripts.is_empty());
        assert_eq!(site.requests.lock().unwrap().len(), before + 1);
        let report = site.load("/report", http::FetchMode::Normal, true);
        assert!(report.scripting_blocked.is_none());
        assert_eq!(report.scripts.len(), 1);
    }

    #[test]
    fn font_network_loading_uses_redirected_stylesheet_base_and_origin_policy() {
        let site = Site::new();
        let loaded = site.load("/font-page", http::FetchMode::FullReload, true);
        assert_eq!(loaded.fonts.len(), 2);
        assert!(loaded
            .fonts
            .resolve(&["Page".into()], 400, false, 'A')
            .is_some());
        assert!(loaded
            .events
            .iter()
            .any(|event| event.contains("cross-origin font loading/CORS is not implemented")));
        let requests = site.requests.lock().unwrap().clone();
        assert_eq!(
            requests
                .iter()
                .map(|request| request.split_whitespace().nth(1).unwrap())
                .collect::<Vec<_>>(),
            [
                "/font-page",
                "/font-redirect.css",
                "/assets/font.css",
                "/assets/page-font.ttf"
            ]
        );
        assert!(requests
            .iter()
            .all(|request| request.contains("\r\nCache-Control: no-cache\r\n")));
        let before = requests.len();
        let blocked = site.load("/font-blocked", http::FetchMode::Normal, true);
        assert_eq!(blocked.fonts.len(), 0);
        assert_eq!(site.requests.lock().unwrap().len(), before + 3);
        assert!(blocked
            .events
            .iter()
            .any(|event| event.contains("font-src/default-src blocked")));
    }

    #[test]
    fn standalone_images_use_existing_pixels_without_a_second_request() {
        let site = Site::new();
        let loaded = site.load("/post-image.ppm", http::FetchMode::Normal, true);
        assert_eq!(loaded.images.len(), 1);
        assert_eq!(loaded.document.content_type, "image/x-portable-pixmap");
        assert_eq!(loaded.images.values().next().unwrap().pixels, [0xffff0000]);
        assert!(loaded.document.title.contains("post-image.ppm"));
        assert!(loaded
            .events
            .iter()
            .any(|event| event.contains("IMAGE DOCUMENT: decoded 1 x 1")));
        let paths: Vec<String> = site
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.split_whitespace().nth(1).unwrap().to_owned())
            .collect();
        assert_eq!(paths, ["/post-image.ppm"]);
        assert!(image_plan(&loaded.document, true, Default::default(), &mut Vec::new()).is_empty());
        let folder = Folder::new();
        let path = folder.file("pixel.ppm", "P3\n2 1\n255\n255 0 0 0 255 0\n");
        let local = local(&navigation::explicit(&path).unwrap()).unwrap();
        assert_eq!(local.images.values().next().unwrap().width, 2);
        assert_eq!(local.document.content_type, "image/x-portable-pixmap");
        assert!(local.scripts.is_empty());
        // An image document's text never enters the HTML/script execution path.
        let path = folder.file("invalid.svg", "<script>document.title='injected'</script>");
        let disabled = local_with_options(
            &navigation::explicit(&path).unwrap(),
            LoadOptions {
                images: false,
                ..LoadOptions::default()
            },
        )
        .unwrap();
        assert!(disabled.images.is_empty() && disabled.scripts.is_empty());
        assert!(disabled.raw.contains("display is disabled"));
        assert_ne!(disabled.document.title, "injected");
        assert_eq!(disabled.document.content_type, "image/svg+xml");
        let path = folder.file(
            "notes.txt",
            "<script>document.title='injected'</script>\nplain text",
        );
        let text = local_with_scripts(&navigation::explicit(&path).unwrap(), true).unwrap();
        assert_eq!(text.document.content_type, "text/plain");
        assert!(text.scripts.is_empty() && text.images.is_empty());
        assert!(text.raw.starts_with("<script>"));
    }

    #[test]
    fn script_document_redirects_check_origin_and_request_rules_before_fetch() {
        let site = Site::new();
        let options = |script: bool, rules: &[&str]| LoadOptions {
            javascript: false,
            images: false,
            script_origin: script.then(|| site.location.url.clone().unwrap()),
            blocking: Arc::new(crate::blocking::Policy::new(&crate::blocking::Config {
                enabled: true,
                builtins: false,
                rules: rules.iter().map(|rule| (*rule).to_owned()).collect(),
            })),
            ..LoadOptions::default()
        };
        let load_path = |reference: &str, options| {
            load_with_options(
                &navigation::resolve(reference, &site.location).unwrap(),
                &AtomicBool::new(false),
                &|_| {},
                &trust::Store::unconfigured(),
                http::FetchMode::Normal,
                options,
            )
        };
        let take_paths = || {
            std::mem::take(&mut *site.requests.lock().unwrap())
                .iter()
                .map(|r| r.split_whitespace().nth(1).unwrap().to_owned())
                .collect::<Vec<_>>()
        };

        let loaded = load_path("/script-nav-redirect", options(true, &[])).unwrap();
        assert_eq!(loaded.document.title, "Fresh page");
        assert_eq!(take_paths(), ["/script-nav-redirect", "/fresh"]);

        let error = load_path("/script-nav-cross", options(true, &[]))
            .err()
            .expect("the cross-origin redirected destination must be blocked");
        assert!(error.contains("Cross-origin script navigation"), "{error}");
        assert_eq!(take_paths(), ["/script-nav-cross"]);

        let origin = site.location.url.as_ref().unwrap();
        let cross = format!("http://localhost:{}/fresh", origin.port);
        let error = load_path(&cross, options(true, &[]))
            .err()
            .expect("the initial destination must also keep the origin");
        assert!(error.contains("Cross-origin script navigation"), "{error}");
        assert!(take_paths().is_empty());

        let error = load_path("/fresh", options(true, &["path:/fresh"]))
            .err()
            .expect("initial script navigation must honor request rules");
        assert!(error.contains("Blocked by request rule"), "{error}");
        assert!(take_paths().is_empty());
        let error = load_path("/script-nav-redirect", options(true, &["path:/fresh"]))
            .err()
            .expect("redirected script navigation must honor request rules");
        assert!(error.contains("Blocked by request rule"), "{error}");
        assert_eq!(take_paths(), ["/script-nav-redirect"]);

        // The same endpoint remains reachable through an ordinary user
        // document navigation, including its cross-origin redirect.
        let loaded = load_path("/script-nav-cross", options(false, &["path:/fresh"])).unwrap();
        assert_eq!(loaded.document.title, "Fresh page");
        assert_eq!(loaded.location.url.unwrap().host, "localhost");
        assert_eq!(take_paths(), ["/script-nav-cross", "/fresh"]);
    }

    #[test]
    fn request_rules_stop_initial_and_redirected_subresources_before_network_work() {
        let site = Site::new();
        let options = |rules: &[&str]| LoadOptions {
            blocking: Arc::new(crate::blocking::Policy::new(&crate::blocking::Config {
                enabled: true,
                builtins: false,
                rules: rules.iter().map(|rule| (*rule).to_owned()).collect(),
            })),
            ..LoadOptions::default()
        };
        let load_path = |path: &str, options| {
            load_with_options(
                &navigation::resolve(path, &site.location).unwrap(),
                &AtomicBool::new(false),
                &|_| {},
                &trust::Store::unconfigured(),
                http::FetchMode::Normal,
                options,
            )
            .unwrap()
        };
        let paths = || {
            site.requests
                .lock()
                .unwrap()
                .iter()
                .map(|r| r.split_whitespace().nth(1).unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        // The top-level document stays reachable, even when a host rule blocks
        // all of its automatic resources. No blocked resource uses an attempt.
        let loaded = load_path("/settings", options(&["127.0.0.1"]));
        assert_eq!(paths(), ["/settings"]);
        assert!(loaded.images.is_empty() && loaded.styles.is_empty() && loaded.scripts.is_empty());
        assert!(loaded
            .events
            .iter()
            .any(|event| event.contains("Blocked by request rule")));
        site.requests.lock().unwrap().clear();
        let loaded = load_path("/settings", options(&["path:/", "@@path:/ok.js"]));
        assert_eq!(paths(), ["/settings", "/ok.js"]);
        assert_eq!(loaded.scripts.len(), 1);
        assert!(loaded.images.is_empty());
        site.requests.lock().unwrap().clear();
        let loaded = load_path("/font-page", options(&["path:/assets/font.css"]));
        assert_eq!(paths(), ["/font-page", "/font-redirect.css"]);
        assert!(loaded.styles.is_empty() && loaded.fonts.len() == 0);
        assert!(loaded
            .events
            .iter()
            .any(|event| event.contains("path:/assets/font.css")));
        site.requests.lock().unwrap().clear();
        let loaded = load_path("/font-page", options(&["path:/assets/page-font.ttf"]));
        assert_eq!(
            paths(),
            ["/font-page", "/font-redirect.css", "/assets/font.css"]
        );
        assert_eq!(loaded.fonts.len(), 0);
        assert!(loaded
            .events
            .iter()
            .any(|event| event.contains("path:/assets/page-font.ttf")));
        // Disable the policy and the same image/script/CSS now load normally.
        site.requests.lock().unwrap().clear();
        let loaded = load_path("/settings", LoadOptions::default());
        assert_eq!(loaded.images.len(), 2);
        assert_eq!(loaded.scripts.len(), 1);
        assert!(paths().contains(&"/post-image.ppm".to_owned()));
    }

    #[test]
    fn image_and_script_preferences_skip_network_work_independently() {
        let site = Site::new();
        let location = navigation::resolve("/settings", &site.location).unwrap();
        for javascript in [false, true] {
            for images in [false, true] {
                site.requests.lock().unwrap().clear();
                let loaded = load_with_options(
                    &location,
                    &AtomicBool::new(false),
                    &|_| {},
                    &trust::Store::unconfigured(),
                    http::FetchMode::FullReload,
                    LoadOptions {
                        javascript,
                        images,
                        ..LoadOptions::default()
                    },
                )
                .unwrap();
                let requests = site.requests.lock().unwrap().clone();
                let paths: Vec<_> = requests
                    .iter()
                    .map(|request| request.split_whitespace().nth(1).unwrap())
                    .collect();
                assert_eq!(
                    paths.iter().filter(|path| **path == "/ok.js").count(),
                    usize::from(javascript)
                );
                assert_eq!(
                    paths
                        .iter()
                        .filter(|path| **path == "/post-image.ppm")
                        .count(),
                    usize::from(images)
                );
                assert_eq!(loaded.styles.len(), 1);
                assert_eq!(loaded.scripts.len(), usize::from(javascript));
                assert_eq!(
                    loaded.images.len(),
                    if images {
                        if javascript {
                            2
                        } else {
                            3
                        }
                    } else {
                        0
                    }
                );
                assert!(requests
                    .iter()
                    .all(|request| request.contains("\r\nCache-Control: no-cache\r\n")));
                assert_eq!(
                    loaded
                        .events
                        .iter()
                        .any(|event| event.starts_with("Images disabled by user:")),
                    !images
                );
            }
        }
    }

    #[test]
    fn disabled_images_do_not_read_or_decode_local_images() {
        let folder = Folder::new();
        folder.file("bad.ppm", "not an image");
        let location = folder.page(
            "<img src=bad.ppm><input type=image src=missing.ppm><script>var stillRuns=1;</script>",
        );
        let off = local_with_options(
            &location,
            LoadOptions {
                javascript: true,
                images: false,
                ..LoadOptions::default()
            },
        )
        .unwrap();
        assert!(off.images.is_empty());
        assert_eq!(off.scripts.len(), 1);
        assert_eq!(off.skipped, 2);
        assert!(!off.events.iter().any(|event| event.starts_with("IMAGE ")));
        assert!(off
            .events
            .iter()
            .any(|event| event.contains("1 resource attempts")));
        let on = local_with_options(&location, LoadOptions::default()).unwrap();
        assert!(on.images.is_empty());
        assert!(on
            .events
            .iter()
            .any(|event| event.starts_with("IMAGE bad.ppm:")));
    }

    #[test]
    fn repeated_assets_share_one_attempt_and_decode_but_reload_fetches_again() {
        let site = Site::new();
        for mode in [
            http::FetchMode::Normal,
            http::FetchMode::Normal,
            http::FetchMode::FullReload,
        ] {
            site.requests.lock().unwrap().clear();
            let loaded = site.load("/repeat-resources", mode, true);
            let requests = site.requests.lock().unwrap().clone();
            let paths: Vec<_> = requests
                .iter()
                .map(|request| request.split_whitespace().nth(1).unwrap())
                .collect();
            assert_eq!(paths.len(), 6);
            assert_eq!(&paths[..2], ["/repeat-resources", "/post-style.css"]);
            assert_eq!(&paths[2..4], ["/ok.js", "/ok.js"]);
            assert!(paths[4..].contains(&"/post-image.ppm"));
            assert!(paths[4..].contains(&"/unsupported.gif"));
            assert_eq!(loaded.styles.len(), 2);
            assert_eq!(loaded.images.len(), 41);
            assert_eq!(loaded.scripts.len(), 2);
            assert_eq!(loaded.skipped, 60);
            let first = loaded.images.values().next().unwrap();
            assert!(loaded
                .images
                .values()
                .all(|image| Arc::ptr_eq(first, image)));
            let styles = crate::css::compute(&loaded.document, &loaded.styles);
            let result = loaded.document.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { id, .. } if id.as_deref() == Some("result"))).unwrap();
            assert_eq!(styles[result].color, 0x123456);
            assert!(loaded
                .events
                .iter()
                .any(|event| event.contains("100 reused references (59 earlier failures)")));
            assert!(!loaded
                .events
                .iter()
                .any(|event| event.contains("Resource attempt limit reached")));
            assert!(loaded
                .events
                .iter()
                .any(|event| event.contains("resource messages omitted")));
            assert!(loaded.events.len() < 128);
            if mode == http::FetchMode::FullReload {
                assert!(requests.iter().all(|request| request
                    .contains("\r\nCache-Control: no-cache\r\n")
                    && request.contains("\r\nPragma: no-cache\r\n")));
            } else {
                assert!(requests
                    .iter()
                    .all(|request| !request.contains("Cache-Control:")));
            }
        }
    }

    #[test]
    fn csp_base_nonceability_and_sandbox_are_checked_before_script_fetch() {
        let site = Site::new();
        let base = site.load("/csp-base", http::FetchMode::Normal, true);
        assert_eq!(base.scripts.len(), 1);
        assert_eq!(base.base.display(), base.location.display());
        assert!(base
            .events
            .iter()
            .any(|event| event.contains("base-uri blocked")));
        assert_eq!(site.requests.lock().unwrap().len(), 2);

        let duplicate = site.load("/csp-duplicate", http::FetchMode::Normal, true);
        assert!(duplicate.scripts.is_empty());
        assert_eq!(site.requests.lock().unwrap().len(), 3);

        let sandbox = site.load("/csp-sandbox", http::FetchMode::Normal, true);
        assert!(sandbox.scripts.is_empty());
        assert!(sandbox.images.is_empty());
        assert!(sandbox.scripting_blocked.is_none());
        assert!(sandbox.events.iter().any(|event| event.contains("sandbox")));
        assert_eq!(site.requests.lock().unwrap().len(), 4);
    }

    #[test]
    fn post_document_metadata_tracks_redirects_while_assets_always_use_get() {
        let site = Site::new();
        let request = http::Request::FormPost {
            body: b"1=private-token&12=++++Proceed++++".to_vec(),
            origin: site.location.url.clone(),
        };
        for (path, final_post, expected_document_count) in [
            ("/post?kept=1", true, 1),
            ("/post-303", false, 2),
            ("/post-307", true, 2),
        ] {
            site.requests.lock().unwrap().clear();
            let loaded = load_request(
                &navigation::resolve(path, &site.location).unwrap(),
                &request,
                &AtomicBool::new(false),
                &|_| {},
                &trust::Store::unconfigured(),
                http::FetchMode::FullReload,
                true,
            )
            .unwrap();
            assert_eq!(loaded.request.is_post(), final_post);
            if let http::Request::FormPost { body, origin } = &loaded.request {
                assert_eq!(body, b"1=private-token&12=++++Proceed++++");
                assert!(origin
                    .as_ref()
                    .unwrap()
                    .same_origin(site.location.url.as_ref().unwrap()));
            }
            assert_eq!(loaded.document.title, "Form result");
            assert_eq!(
                (
                    loaded.styles.len(),
                    loaded.images.len(),
                    loaded.scripts.len()
                ),
                (1, 1, 1)
            );
            assert!(!loaded
                .events
                .iter()
                .any(|line| line.contains("private-token")));
            let requests = site.requests.lock().unwrap();
            assert_eq!(requests.len(), expected_document_count + 3);
            assert!(requests[0].starts_with(&format!("POST {path} HTTP/1.1\r\n")));
            assert!(requests[0].ends_with("1=private-token&12=++++Proceed++++"));
            if expected_document_count == 2 {
                if final_post {
                    assert!(requests[1].starts_with("POST /post?kept=1 HTTP/1.1\r\n"));
                    assert!(requests[1].ends_with("1=private-token&12=++++Proceed++++"));
                } else {
                    assert!(requests[1].starts_with("GET /post-result HTTP/1.1\r\n"));
                    assert!(requests[1].ends_with("\r\n\r\n"));
                    assert_eq!(loaded.location.fragment, "finished");
                }
            }
            for asset in &requests[expected_document_count..] {
                assert!(asset.starts_with("GET ") && asset.ends_with("\r\n\r\n"));
                assert!(
                    !asset.contains("\r\nOrigin:")
                        && !asset.contains("\r\nContent-Type:")
                        && !asset.contains("\r\nContent-Length:")
                );
            }
            assert!(requests
                .iter()
                .all(|r| r.contains("\r\nCache-Control: no-cache\r\n")
                    && r.contains("\r\nPragma: no-cache\r\n")));
        }
    }

    #[test]
    fn local_post_rejects_and_get_wrapper_keeps_local_documents_available() {
        let folder = Folder::new();
        let page = folder.page("<title>Local</title><p>Read me</p>");
        let request = http::Request::FormPost {
            body: b"field=value".to_vec(),
            origin: None,
        };
        for location in [&page, &Location::home()] {
            let error = load_request(
                location,
                &request,
                &AtomicBool::new(false),
                &|_| {},
                &trust::Store::unconfigured(),
                http::FetchMode::Normal,
                true,
            )
            .err()
            .unwrap();
            assert!(error.contains("POST") && error.contains("local-file"));
        }
        let loaded = local(&page).unwrap();
        assert!(!loaded.request.is_post());
        assert_eq!(loaded.document.title, "Local");
    }

    #[test]
    fn worker_delivers_the_explicit_post_payload_and_final_document_method() {
        let site = Site::new();
        let worker = Worker::new(Arc::new(|| {}), Arc::new(trust::Store::unconfigured())).unwrap();
        worker.submit_request(
            7,
            navigation::resolve("/post?kept=1", &site.location).unwrap(),
            http::FetchMode::Normal,
            false,
            http::Request::FormPost {
                body: b"name=worker".to_vec(),
                origin: None,
            },
        );
        let started = Instant::now();
        let mut loaded = None;
        loop {
            let poll = worker.poll();
            if let Some((id, document)) = poll.document {
                assert_eq!(id, 7);
                assert!(loaded.is_none());
                loaded = Some(document);
            }
            if let Some(completion) = poll.completed {
                assert_eq!(completion.id, 7);
                completion.result.unwrap();
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        let loaded = loaded.unwrap();
        assert!(loaded.request.is_post());
        assert!(loaded.scripts.is_empty());
        let requests = site.requests.lock().unwrap();
        assert!(
            requests[0].starts_with("POST /post?kept=1 HTTP/1.1\r\n")
                && requests[0].ends_with("name=worker")
        );
        assert_eq!(requests.len(), 3); // document, CSS and image; scripting disabled.
    }

    #[test]
    fn worker_delivers_styled_document_before_a_blocked_image() {
        let site = Site::new();
        let worker = Worker::new(Arc::new(|| {}), Arc::new(trust::Store::unconfigured())).unwrap();
        worker.submit(
            1,
            navigation::resolve("/staged", &site.location).unwrap(),
            http::FetchMode::Normal,
            true,
        );
        let started = Instant::now();
        let document = loop {
            let poll = worker.poll();
            assert!(poll.images.is_none() && poll.completed.is_none());
            if let Some((id, document)) = poll.document {
                assert_eq!(id, 1);
                break document;
            }
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(document.document.title, "Staged page");
        assert_eq!(document.styles.len(), 1);
        assert_eq!(document.scripts.len(), 1);
        assert!(document.images.is_empty());
        assert!(document
            .events
            .iter()
            .any(|line| line.starts_with("DISPLAY READY:")));
        while site.requests.lock().unwrap().len() < 4 {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        let paths: Vec<String> = site
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.split_whitespace().nth(1).unwrap().to_owned())
            .collect();
        assert_eq!(paths, ["/staged", "/post-style.css", "/ok.js", "/held.ppm"]);
        let poll = worker.poll();
        assert!(poll.document.is_none() && poll.images.is_none() && poll.completed.is_none());
        site.image_gate.store(true, Ordering::Relaxed);
        let mut images = 0;
        loop {
            let poll = worker.poll();
            assert!(
                poll.document.is_none(),
                "document must only be delivered once"
            );
            if let Some((id, update)) = poll.images {
                assert_eq!(id, 1);
                images += update.images.len();
                assert_eq!(update.skipped, 0);
            }
            if let Some(completion) = poll.completed {
                assert_eq!(completion.id, 1);
                let end = completion.result.unwrap();
                assert_eq!(end.skipped, 0);
                assert!(end
                    .events
                    .iter()
                    .any(|line| line.starts_with("LOAD TIMING:")));
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(images, 1);
    }

    #[test]
    fn worker_coalesces_late_polls_and_discards_replaced_image_deliveries() {
        let site = Site::new();
        let worker = Worker::new(Arc::new(|| {}), Arc::new(trust::Store::unconfigured())).unwrap();
        worker.submit(
            1,
            navigation::resolve("/repeat-resources", &site.location).unwrap(),
            http::FetchMode::Normal,
            true,
        );
        let started = Instant::now();
        while worker.shared.0.lock().unwrap().completed.is_none() {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        let poll = worker.poll();
        let (id, document) = poll.document.unwrap();
        assert_eq!(id, 1);
        assert!(document.images.is_empty());
        assert_eq!(document.scripts.len(), 2);
        let (id, update) = poll.images.unwrap();
        assert_eq!(id, 1);
        assert_eq!(update.images.len(), 41);
        assert_eq!(update.skipped, 60);
        assert!(update.events.len() < 128);
        let first = update.images.values().next().unwrap();
        assert!(update
            .images
            .values()
            .all(|image| Arc::ptr_eq(first, image)));
        assert_eq!(poll.completed.unwrap().result.unwrap().skipped, 60);
        let poll = worker.poll();
        assert!(poll.document.is_none() && poll.images.is_none() && poll.completed.is_none());

        worker.submit(
            2,
            navigation::resolve("/staged", &site.location).unwrap(),
            http::FetchMode::Normal,
            true,
        );
        let started = Instant::now();
        loop {
            if let Some((id, document)) = worker.poll().document {
                assert_eq!(id, 2);
                assert_eq!(document.document.title, "Staged page");
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        worker.submit(
            3,
            navigation::resolve("/fresh", &site.location).unwrap(),
            http::FetchMode::Normal,
            false,
        );
        site.image_gate.store(true, Ordering::Relaxed);
        let mut replacement = false;
        loop {
            let poll = worker.poll();
            assert!(
                poll.images.is_none(),
                "old image results must not reach the new document"
            );
            if let Some((id, document)) = poll.document {
                assert_eq!(id, 3);
                assert_eq!(document.document.title, "Fresh page");
                replacement = true;
            }
            if let Some(completion) = poll.completed {
                assert_eq!(completion.id, 3);
                completion.result.unwrap();
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(replacement);
    }

    #[test]
    fn text_decoding_respects_bom_headers_and_meta() {
        assert_eq!(
            decode_text(b"\xef\xbb\xbfhello", "text/html;charset=nope", true).unwrap(),
            "hello"
        );
        assert_eq!(
            decode_text(&[b'A', 0x80, 0xe9], "text/html; charset=windows-1252", true).unwrap(),
            "A€é"
        );
        assert!(decode_text(b"abc", "text/html;charset=shift_jis", true).is_err());
        assert!(decode_text(b"<meta charset=shift_jis>abc", "text/html", true).is_err());
        assert_eq!(
            decode_text(b"a\xff", "text/html;charset=utf-8", true).unwrap(),
            "a\u{fffd}"
        );
    }

    #[test]
    fn stylesheet_fragments_beyond_sixteen_keep_sources_and_cap() {
        let folder = Folder::new();
        let mut markup = String::new();
        for i in 0..MAX_STYLESHEETS + 1 {
            let name = format!("fragment-{i}.css");
            folder.file(&name, &format!("p{{color:#{:06x}}}", i));
            markup.push_str(&format!("<link rel=stylesheet href={name}>"));
        }
        markup.push_str("<p>fragments</p>");
        let loaded = local(&folder.page(&markup)).unwrap();
        assert_eq!(loaded.styles.len(), MAX_STYLESHEETS);
        assert_eq!(loaded.skipped, 1);
        // The bounded event report may omit late repeated resource messages.
        assert!(loaded.styles.values().any(|s| s.contains("#0000ff")));
        assert!(!loaded.styles.values().any(|s| s.contains("#000100")));
    }

    #[test]
    fn resource_attempts_have_a_finite_shared_boundary() {
        let mut stats = LoadStats::new(Instant::now());
        for _ in 0..MAX_RESOURCE_ATTEMPTS { stats.begin_attempt(false).unwrap(); }
        assert!(stats.begin_attempt(false).unwrap_err().contains(&MAX_RESOURCE_ATTEMPTS.to_string()));
        assert_eq!(stats.attempts, MAX_RESOURCE_ATTEMPTS);
        // Time and bytes are independent; higher counts do not disable either.
        stats.attempts = 0;
        stats.started = Instant::now() - Duration::from_secs(46);
        assert!(stats.begin_attempt(true).unwrap_err().contains("45 seconds"));
        stats.network_bytes = 256 * 1024 * 1024;
        assert_eq!(stats.remaining_bytes(), 0);
    }

    #[test]
    fn same_origin_crossorigin_scripts_load_but_do_not_cross_redirect_origins() {
        let site = Site::new();
        let loaded = site.load("/cors-page", http::FetchMode::Normal, true);
        assert_eq!(loaded.scripts.len(), 3);
        assert!(loaded.scripts.iter().all(|s| s.source == "var external=2;"));
        let paths: Vec<_> = site.requests.lock().unwrap().iter()
            .map(|r| r.split_whitespace().nth(1).unwrap().to_owned()).collect();
        assert_eq!(paths, ["/cors-page", "/ok.js", "/redirect.js", "/last.js",
            "/ok.js", "/bad.js", "/foreign-redirect.js"]);
        assert!(loaded.events.iter().any(|e| e.contains("CORS-mode")));
        assert!(loaded.events.iter().any(|e| e.contains("Content-Type")));
        assert!(loaded.events.iter().any(|e| e.contains("Subresource Integrity")));
        assert!(!paths.iter().any(|p| p == "/unfetched.js"));
        site.requests.lock().unwrap().clear();
        let blocked = site.load("/cors-blocked", http::FetchMode::Normal, true);
        assert!(blocked.scripts.is_empty());
        assert_eq!(site.requests.lock().unwrap().len(), 1);
    }

}
