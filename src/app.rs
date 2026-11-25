use crate::editor::Editor;
use crate::forms::{Forms, Kind, SelectRow};
use crate::html::{self, Document};
use crate::image::Bitmap;
use crate::layout::{self, Layout, PaintItem};
use crate::loader::{self, Loaded};
use crate::navigation::{self, Location};
use crate::paint::{Canvas, GLYPH_ADVANCE, GLYPH_HEIGHT};
use crate::ui;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[path = "app_animation.rs"]
mod animation;
#[path = "app_media.rs"]
mod media_ui;
#[path = "app_audio.rs"]
mod audio_ui;
#[path = "app_browser_ui.rs"]
mod browser_ui;
use browser_ui::BrowserPanel;
#[path = "app_appearance.rs"]
mod appearance;
#[path = "app_bookmarks_bar.rs"]
mod bookmarks_bar;
#[path = "app_history.rs"]
mod browsing_history;
#[path = "app_chrome.rs"]
mod chrome;
#[path = "app_downloads.rs"]
mod download_ui;
#[path = "app_inspector.rs"]
mod inspector;
#[path = "app_input_harness.rs"]
mod input_harness;
#[path = "app_observations.rs"]
mod observations;
#[path = "app_keyboard.rs"]
pub(crate) mod keyboard;
#[path="app_script_requests.rs"]
mod script_requests;
#[path = "app_scripts.rs"]
mod script_jobs;
#[cfg(test)]
#[path = "app_search_tests.rs"]
mod search_tests;
#[path = "app_stylesheets.rs"]
mod style_jobs;
#[path = "app_session_history.rs"]
mod session_history;
#[path = "app_session.rs"]
pub(crate) mod session;
#[path = "app_platform_continuation.rs"]
mod platform_continuation;
pub use platform_continuation::{PlatformCompletion, PlatformRequest};
#[path = "app_anchor.rs"]
pub(crate) mod anchor;
#[path = "app_inline_svg.rs"]
mod inline_svg;
#[path = "app_images.rs"]
mod image_jobs;
#[path = "app_responsive_images.rs"]
mod responsive_images;
#[path = "app_backgrounds.rs"]
mod backgrounds;
#[path = "app_textarea.rs"]
mod textarea;

pub const BAR_HEIGHT: i32 = 68;
pub const FOOT_HEIGHT: i32 = 32;
const FIND_HEIGHT: i32 = 42;
const START: &str = include_str!("../pages/start.html");

pub enum PlatformAction {
    None,
    OpenFile,
    OpenFolder(PathBuf),
    Copy(String),
    /// Fixed safe build metadata from the native About panel only.
    CopyBuildInformation(String),
    Cut(String),
    Paste,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Page,
    Address,
    Find,
    Control(usize),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Page,
    Source,
    Resources,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TextPoint {
    run: usize,
    offset: usize,
}
#[derive(Clone, Copy)]
struct Selection {
    start: TextPoint,
    end: TextPoint,
}
impl Selection {
    fn ordered(self) -> (TextPoint, TextPoint) {
        (self.start.min(self.end), self.start.max(self.end))
    }
    fn slice(self, run: usize, length: usize) -> Option<(usize, usize)> {
        let (start, end) = self.ordered();
        if run < start.run || run > end.run {
            return None;
        }
        let a = if run == start.run {
            start.offset.min(length)
        } else {
            0
        };
        let b = if run == end.run {
            end.offset.min(length)
        } else {
            length
        };
        (a < b).then_some((a, b))
    }
}
enum Drag {
    None,
    Text {
        x: i32,
        y: i32,
        href: Option<String>,
        moved: bool,
    },
    Scrollbar {
        offset: i32,
    },
    Field,
    Control {
        node: usize,
    },
    SelectOption {
        node: usize,
        index: usize,
    },
}
#[derive(Clone, Copy)]
struct SelectPopup {
    node: usize,
    highlighted: Option<usize>, // Row index; group headings cannot be chosen.
    first: usize,
    wheel_remainder: i32,
}
#[derive(Clone, Copy)]
struct SelectPopupBox {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    row_height: i32,
    visible: usize,
    scale: i32,
}
use crate::session_history::{Entry as HistoryEntry,Stack as SessionHistory};
#[derive(Clone, Copy)]
enum NavigationKind {
    New,
    ScriptNew,
    Reload(i32),
    ScriptReplace(Option<i32>),
    History(usize, i32),
}
#[derive(Clone, Copy)]
enum LoadingScroll {
    Offset(i32),
    Fragment,
}
struct Pending {
    id: u64,
    location: Location,
    kind: NavigationKind,
    mode: crate::http::FetchMode,
    request: crate::http::Request,
    committed: bool,
    scroll_target: Option<LoadingScroll>,
}
struct Resubmission {
    token: u64,
    location: Location,
    request: crate::http::Request,
    kind: NavigationKind,
    mode: crate::http::FetchMode,
    offered: bool,
}

pub struct App {
    pub document: Document,
    pub page: Layout,
    pub width: i32,
    pub height: i32,
    pub scroll: i32,
    pub source: Option<PathBuf>,
    pub status: String,
    pub failed: bool,
    location: Location,
    current_request: crate::http::Request,
    base_location: Location,
    base_is_document_url: bool,
    home_location: Location,
    raw: String,
    styles: HashMap<usize, String>,
    stylesheet_cache: Option<crate::css::StylesheetCache>,
    style_bases: HashMap<usize, Location>,
    backgrounds: HashMap<usize, std::sync::Arc<Bitmap>>,
    background_requests: HashMap<usize, backgrounds::Request>,
    pending_backgrounds: HashMap<usize, u64>,
    background_generation: u64,
    background_request_limit: u64,
    base_fonts: crate::fonts::Book,
    fonts: crate::fonts::Book,
    images: HashMap<usize, std::sync::Arc<Bitmap>>,
    inline_svg_sources: HashMap<usize, inline_svg::Source>,
    image_densities: HashMap<usize, f64>,
    responsive_images: responsive_images::State,
    animations: HashMap<usize, animation::Playback>,
    media: Option<media_ui::Player>,
    media_worker: Option<crate::media::Worker>,
    embedded_video: Option<media_ui::Embedded>,
    embedded_video_worker: Option<crate::embedded_video::Worker>,
    media_limits: crate::media::Limits,
    geometry_limits: crate::script_geometry::Limits,
    audio: Option<audio_ui::Player>,
    audio_worker: Option<crate::audio_output::Worker>,
    audio_limits: crate::audio::Limits,
    audio_notice: Option<String>,
    events: Vec<String>,
    last_failed_navigation: Option<String>,
    scripts_enabled: bool,
    images_enabled: bool,
    bookmark_bar_visible: bool,
    profile: crate::profile::View,
    input_search_provider: Option<String>,
    stylesheet_limits: crate::css::StylesheetLimits,
    downloads: crate::downloads::View,
    // Legacy single-window ownership. Session-attached Apps retain only a Weak
    // View; their browser coordinator owns Shared past every App/View drop.
    owned_downloads: Option<crate::downloads::Shared>,
    downloads_ui: download_ui::DownloadsUi,
    bookmarks_bar: bookmarks_bar::BookmarkBar,
    visited: std::collections::HashSet<String>,
    link_paint: HashMap<usize, u32>,
    network: crate::network_trace::Trace,
    last_network_revision: u64,
    browsing_history: crate::history::View,
    history_commands: crate::history::Commands,
    owned_history: Option<crate::history::Shared>,
    browsing_history_error: Option<String>,
    history_maintenance: Option<std::time::Instant>,
    history_closed: bool,
    browser_panel: Option<BrowserPanel>,
    scripting: Option<crate::scripting::Page>,
    scripting_blocked: Option<String>,
    document_policy: crate::csp::PolicySet,
    cookies: std::sync::Arc<crate::cookies::Session>,
    cookie_same_site: bool,
    dynamic_scripts: Option<script_jobs::Worker>,
    script_requests:Option<crate::script_request_core::Worker>,
    dynamic_pending: usize,
    dynamic_stylesheets: Option<style_jobs::Worker>,
    pending_stylesheets: usize,
    dynamic_images: Option<image_jobs::Worker>,
    pending_images: HashMap<usize, u64>,
    script_epoch: u64,
    script_navigation_count: usize,
    script_navigation_since: std::time::Instant,
    history: crate::session_history::Shared,
    history_index: usize,
    zoom: f32,
    focus: Focus,
    address: Editor,
    find: Editor,
    forms: Forms,
    find_open: bool,
    matches: Vec<Selection>,
    match_index: usize,
    selection: Option<Selection>,
    drag: Drag,
    select_popup: Option<SelectPopup>,
    hover: Option<String>,
    hover_point:Option<(i32,i32)>,
    hover_reflow_active:bool,
    menu: bool,
    chrome: chrome::Chrome,
    inspector: inspector::Inspector,
    view: View,
    alternate: Option<Document>,
    page_scroll: i32,
    wheel_remainder: i32,
    pending_surrogate: Option<u16>,
    worker: Option<loader::Worker>,
    trust: std::sync::Arc<crate::trust::Store>,
    wake: loader::Wake,
    platform_request_scope: std::rc::Rc<()>,
    anchor_geometry_scope: std::rc::Rc<()>,
    pending: Option<Pending>,
    next_request: u64,
    committed_request: Option<u64>,
    observations: observations::Data,
    resubmission: Option<Resubmission>,
    next_confirmation: u64,
}

impl App {
    pub fn new() -> Self {
        let location = Location::home();
        let mut document = html::parse(START);
        document.media_environment=Some(crate::css::MediaEnvironment::desktop_viewport(1080,760-BAR_HEIGHT-bookmarks_bar::HEIGHT-FOOT_HEIGHT,1.0));
        let page = layout::build_viewport(
            &document,
            1068,
            760 - BAR_HEIGHT - bookmarks_bar::HEIGHT - FOOT_HEIGHT,
            1.0,
            &HashMap::new(),
            &HashMap::new(),
        );
        let forms = Forms::new(&document);
        let mut address = Editor::default();
        address.set(&location.display());
        let owned_downloads = crate::downloads::Shared::new();
        // Fresh and not escaped: no borrow or worker can conflict with view().
        let downloads = owned_downloads.view().expect("A fresh download authority is available.");
        let owned_history = crate::history::Shared::new(crate::history::Store::memory());
        let browsing_history = owned_history.view().expect("A fresh history authority is available.");
        let history_commands = owned_history.commands();
        Self {
            document,
            page,
            width: 1080,
            height: 760,
            scroll: 0,
            source: None,
            status: String::new(),
            failed: false,
            location: location.clone(),
            current_request: crate::http::Request::Get,
            base_location: location.clone(),
            base_is_document_url: true,
            home_location: location.clone(),
            raw: START.to_owned(),
            styles: HashMap::new(),
            stylesheet_cache: None,
            style_bases: HashMap::new(),
            backgrounds: HashMap::new(),
            background_requests: HashMap::new(),
            pending_backgrounds: HashMap::new(),
            background_generation: 0,
            background_request_limit: 65_536,
            base_fonts: crate::fonts::Book::empty(),
            fonts: crate::fonts::Book::empty(),
            images: HashMap::new(),
            inline_svg_sources: HashMap::new(),
            image_densities: HashMap::new(),
            responsive_images: responsive_images::State::default(),
            animations: HashMap::new(),
            media: None,
            media_worker: None,
            embedded_video: None,
            embedded_video_worker: None,
            media_limits: crate::media::Limits::default(),
            geometry_limits: crate::script_geometry::Limits::default(),
            audio: None,
            audio_worker: None,
            audio_limits: crate::audio::Limits::default(),
            audio_notice: None,
            events: vec!["Loaded bundled start page (no external resources).".to_owned()],
            last_failed_navigation: None,
            scripts_enabled: true,
            images_enabled: true,
            bookmark_bar_visible: true,
            profile: crate::profile::View::default(),
            input_search_provider: None,
            stylesheet_limits: crate::css::StylesheetLimits::default(),
            downloads,
            owned_downloads: Some(owned_downloads),
            downloads_ui: download_ui::DownloadsUi::default(),
            bookmarks_bar: bookmarks_bar::BookmarkBar::default(),
            visited: std::collections::HashSet::new(),
            link_paint: HashMap::new(),
            network: crate::network_trace::Trace::default(),
            last_network_revision: 0,
            browsing_history,
            history_commands,
            owned_history: Some(owned_history),
            browsing_history_error: None,
            history_maintenance: None,
            history_closed: false,
            browser_panel: None,
            scripting: None,
            scripting_blocked: None,
            document_policy: crate::csp::PolicySet::default(),
            cookies: std::sync::Arc::new(crate::cookies::Session::default()),
            cookie_same_site: false,
            dynamic_scripts: None,
            script_requests:None,
            dynamic_pending: 0,
            dynamic_stylesheets: None,
            pending_stylesheets: 0,
            dynamic_images: None,
            pending_images: HashMap::new(),
            script_epoch: 0,
            script_navigation_count: 0,
            script_navigation_since: std::time::Instant::now(),
            history: SessionHistory::shared(HistoryEntry {
                location,
                scroll: 0,
                request: crate::http::Request::Get,
                document: 0,
                data: Default::default(),
            }).expect("fixed initial session entry"),
            history_index: 0,
            zoom: 1.0,
            focus: Focus::Page,
            address,
            find: Editor::default(),
            forms,
            find_open: false,
            matches: Vec::new(),
            match_index: 0,
            selection: None,
            drag: Drag::None,
            select_popup: None,
            hover: None,
            hover_point:None,hover_reflow_active:false,
            menu: false,
            chrome: chrome::Chrome::default(),
            inspector: inspector::Inspector::default(),
            view: View::Page,
            alternate: None,
            page_scroll: 0,
            wheel_remainder: 0,
            pending_surrogate: None,
            worker: None,
            trust: std::sync::Arc::new(crate::trust::Store::unconfigured()),
            wake: std::sync::Arc::new(|| {}),
            platform_request_scope: std::rc::Rc::new(()),
            anchor_geometry_scope: std::rc::Rc::new(()),
            pending: None,
            next_request: 0,
            committed_request: None,
            observations: observations::Data::new(),
            resubmission: None,
            next_confirmation: 0,
        }
    }
    pub fn use_system_fonts(&mut self) {
        self.base_fonts = crate::fonts::Book::system();
        self.fonts = self.base_fonts.clone();
        self.reflow();
    }
    pub fn use_home_file(&mut self, path: PathBuf) {
        if !path.is_file() {
            return;
        }
        if let Ok(location) = navigation::explicit(&path) {
            if let Ok(loaded) = loader::local_with_options(&location, self.load_options()) {
                self.home_location = location.clone();
                let entry = HistoryEntry {
                    location: location.clone(),
                    scroll: 0,
                    request: crate::http::Request::Get,
                    document: self.script_epoch.wrapping_add(1),
                    data: Default::default(),
                };
                let history = match SessionHistory::shared(entry) {
                    Ok(history) => history,
                    Err(error) => {
                        self.error(error);
                        return;
                    }
                };
                self.history = history;
                self.history_index = 0;
                self.commit(location, loaded, false);
                self.apply_history_replacement();
            }
        }
    }
    fn error(&mut self, error: String) {
        self.status = error;
        self.failed = true;
    }
    fn navigation_error(&mut self, attempted: &str, error: String) {
        self.last_failed_navigation = Some(format!(
            "{attempted}\n{error}\nThe previous page is still displayed."
        ));
        self.error(format!("{}: {error}", fit_text(attempted, 100)));
    }
    pub fn open(&mut self, path: &Path) {
        self.invalidate_platform_requests();
        self.cancel_pending();
        match navigation::explicit(path) {
            Ok(location) => self.navigate(location),
            Err(e) => self.navigation_error(&path.to_string_lossy(), e),
        }
    }
    fn submit_address_bar(&mut self, input: &str) {
        let engine=self.profile.data().settings.search_engine;
        if engine==crate::search::SearchEngine::None { self.open_address(input); return; }
        let query=match navigation::address_intent(input,&self.location) {
            Ok(navigation::AddressIntent::Navigate)=>{self.open_address(input);return;},
            Ok(navigation::AddressIntent::Search)=>engine.query_url(input.trim(),self.input_search_provider.as_deref()),
            Err(error)=>Err(error),
        };
        match query {Ok(address)=>self.open_address(&address),Err(error)=>{self.cancel_pending();self.navigation_error(input,error);}}
    }
    pub fn open_address(&mut self, address: &str) {
        self.invalidate_platform_requests();
        match navigation::address(address, &self.location) {
            Ok(location) => self.navigate_location(location, address.contains('#')),
            Err(error) => {
                self.cancel_pending();
                self.navigation_error(address, error);
            }
        }
    }
    /// Command-line relative paths start in the process working directory,
    /// while address-bar paths start beside the current local document.
    pub fn open_argument(&mut self, address: &std::ffi::OsStr) {
        self.cancel_pending();
        // Preserve literal '#'/'%' filename characters from the shell or a
        // file association. `explicit` rejects network/device paths before I/O.
        match navigation::explicit(Path::new(address)) {
            Ok(location) => self.navigate(location),
            Err(file_error) => {
                if let Some(text) = address.to_str() {
                    match navigation::address(text, &Location::home()) {
                        Ok(location) => self.navigate(location),
                        Err(error) => self.navigation_error(text, error),
                    }
                } else {
                    self.navigation_error(&address.to_string_lossy(), file_error);
                }
            }
        }
    }
    pub fn set_load_wake(&mut self, wake: loader::Wake) {
        self.set_owned_download_wake(wake.clone());
        if let Some(worker)=&self.media_worker {worker.set_wake(wake.clone());}
        if let Some(worker)=&self.embedded_video_worker {worker.set_wake(wake.clone());}
        if let Some(worker)=&self.audio_worker {worker.set_wake(wake.clone());}
        if let Some(worker) = &self.worker {
            worker.set_wake(wake.clone());
        }
        if let Some(worker) = &self.dynamic_scripts {
            worker.set_wake(wake.clone());
        }
        if let Some(worker) = &self.dynamic_stylesheets {
            worker.set_wake(wake.clone());
        }
        if let Some(worker) = &self.dynamic_images { worker.set_wake(wake.clone()); }
        if let Some(worker)=&self.script_requests{worker.set_wake(wake.clone());}
        self.wake = wake;
    }
    /// Called at startup; trust is immutable for all requests in this worker.
    pub fn use_trust_store(&mut self, trust: std::sync::Arc<crate::trust::Store>) {
        self.cancel_pending();
        self.cancel_script_requests();
        self.dynamic_scripts = None;
        self.dynamic_stylesheets = None;
        self.dynamic_images = None;
        self.worker = None;
        self.trust = trust;
    }
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        if self.scripts_enabled != enabled {
            self.cancel_script_requests();
            self.scripts_enabled = enabled;
            self.scripting = None;
            if !enabled {
                self.document.scripting_enabled = false;
            }
            self.reload();
        }
    }
    pub fn set_images_enabled(&mut self, enabled: bool) {
        if self.images_enabled != enabled {
            self.images_enabled = enabled;
            if !enabled {
                self.images.retain(|node,_|self.media.as_ref().is_some_and(|player|player.node==*node));
                self.image_densities.clear();
                self.animations.clear();
                self.reflow();
            }
            self.reload();
        }
    }
    fn css_media_environment(&self)->crate::css::MediaEnvironment {
        crate::css::MediaEnvironment::desktop_viewport(self.width,self.viewport_height(),self.zoom)
    }
    fn load_options(&self) -> loader::LoadOptions {
        let image_environment = self.image_selection_environment();
        loader::LoadOptions {
            media_limits: self.media_limits,
            audio_limits: self.audio_limits,
            stylesheet_limits: self.stylesheet_limits,
            script_capacity: crate::javascript::limits::BROWSER_CAPACITIES,
            javascript: self.scripts_enabled,
            images: self.images_enabled,
            blocking: std::sync::Arc::new(crate::blocking::Policy::new(
                &self.profile.data().settings.blocking,
            )),
            viewport_width: image_environment.viewport_width as u32,
            viewport_height: image_environment.viewport_height as u32,
            density_milli: (image_environment.density * 1000.0).round() as u32,
            media_environment: Some(self.css_media_environment()),
            cookie_origin: self.location.url.clone(),
            cookie_same_site: None,
            script_origin: None,
            form_policy: None,
        }
    }
    fn toggle_javascript(&mut self) {
        let mut data = self.profile.data().clone();
        data.settings.javascript = !self.scripts_enabled;
        if let Err(error) = self.profile.save(data) {
            self.profile_error(error);
            return;
        }
        self.set_javascript_enabled(!self.scripts_enabled);
    }
    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
            || self.embedded_video_loading()
            || self.dynamic_pending != 0
            || self.pending_stylesheets != 0
            || !self.pending_images.is_empty()
            || self.responsive_images.pending()
            || !self.pending_backgrounds.is_empty()
            || self
                .scripting
                .as_ref()
                .is_some_and(|page| page.has_host_tasks())
    }
    fn cancel_pending(&mut self) {
        self.suspend_media();
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.pending = None;
        self.resubmission = None;
    }
    pub fn stop_loading(&mut self) {
        let was_loading=self.is_loading();
        self.stop_media();
        if !was_loading {
            return;
        }
        self.cancel_script_requests();
        self.cancel_pending();
        self.stop_audio();
        self.address.set(&self.location.display());
        self.status = "Loading stopped.".to_owned();
        self.failed = false;
    }
    fn finish_ready_load_before_replacement(&mut self) -> bool {
        let Some(id) = self.pending.as_ref().filter(|pending|
            pending.committed && self.committed_request == Some(pending.id)).map(|pending| pending.id) else {
            return false;
        };
        let Some(poll) = self.worker.as_ref().and_then(|worker| worker.take_completed_document(id)) else {
            return false;
        };
        self.apply_loader_poll(poll)
    }

    fn start_network(
        &mut self,
        location: Location,
        kind: NavigationKind,
        mode: crate::http::FetchMode,
    ) {
        self.start_network_request(location, kind, mode, crate::http::Request::Get);
    }
    fn start_network_request(
        &mut self,
        location: Location,
        kind: NavigationKind,
        mode: crate::http::FetchMode,
        request: crate::http::Request,
    ) {
        self.start_network_request_with_policy(location, kind, mode, request, None);
    }
    fn start_network_request_with_policy(
        &mut self,
        location: Location,
        kind: NavigationKind,
        mode: crate::http::FetchMode,
        request: crate::http::Request,
        form_policy: Option<crate::csp::PolicySet>,
    ) {
        self.start_network_request_with_link_origin(location, kind, mode, request, form_policy, None);
    }
    fn start_network_request_with_link_origin(
        &mut self,
        location: Location,
        kind: NavigationKind,
        mode: crate::http::FetchMode,
        request: crate::http::Request,
        form_policy: Option<crate::csp::PolicySet>,
        link_origin: Option<Option<crate::url::HttpUrl>>,
    ) {
        // Complete only an already-finished owned document before replacement
        // clears its worker slot and retires the old script/resource queues.
        self.finish_ready_load_before_replacement();
        self.invalidate_platform_requests();
        self.suspend_media();
        self.cancel_script_requests();
        self.select_popup = None;
        self.resubmission = None;
        if self.worker.is_none() {
            match loader::Worker::with_cookie_session(
                self.wake.clone(),
                self.trust.clone(),
                self.cookies.clone(),
            ) {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    self.error(error);
                    return;
                }
            }
        }
        self.next_request = self.next_request.wrapping_add(1);
        let id = self.next_request;
        self.address.set(&location.display());
        self.focus = Focus::Page;
        self.status = format!(
            "{} {} | Esc stops",
            if mode == crate::http::FetchMode::FullReload {
                "Full reload:"
            } else {
                "Loading"
            },
            location.display()
        );
        self.failed = false;
        self.menu = false;
        self.pending = Some(Pending {
            id,
            location: location.clone(),
            kind,
            mode,
            request: request.clone(),
            committed: false,
            scroll_target: None,
        });
        let mut options = self.load_options();
        // A resolved user link carries its source top URL only for this new
        // chain. It never copies a reload SameSite bit or script/form authority.
        if let Some(origin) = link_origin { options.cookie_origin = origin; }
        options.form_policy = form_policy.map(std::sync::Arc::new);
        if matches!(kind, NavigationKind::Reload(_)) {
            // A UI reload preserves how the committed document was reached;
            // it cannot upgrade a cross-site POST into a same-site request.
            options.cookie_same_site = Some(self.cookie_same_site);
        }
        if matches!(
            kind,
            NavigationKind::ScriptNew | NavigationKind::ScriptReplace(_)
        ) {
            options.script_origin = self.location.url.clone();
        }
        self.worker
            .as_ref()
            .unwrap()
            .submit_request_with_options(id, location, mode, options, request);
        if let Some((_, trace)) = self.worker.as_ref().and_then(loader::Worker::network_trace) {
            self.network = trace;
        }
    }
    pub fn network_needs_tick(&self) -> bool {
        (self.pending.is_some() || self.dynamic_pending != 0 || self.pending_stylesheets != 0 || !self.pending_images.is_empty() || self.responsive_images.pending() || !self.pending_backgrounds.is_empty())
            && self.inspector.network_visible()
    }
    pub fn poll_load(&mut self) -> bool {
        let changed = self.poll_load_once();
        self.apply_script_scroll() || changed
    }
    fn poll_load_once(&mut self) -> bool {
        let pause_changed=if self.scripting.as_ref().is_some_and(|p|p.has_media_tasks()) {self.flush_embedded_video()} else {false};
        self.inspector.invalidate();
        let responsive_changed = self.process_responsive_images() | self.poll_responsive_image_events();
        let script_changed = self.poll_native_script_requests() | self.process_native_script_requests() | self.poll_dynamic_scripts()
            | self.process_script_requests()
            | self.poll_dynamic_stylesheets()
            | self.process_stylesheet_requests()
            | self.poll_dynamic_images()
            | self.process_image_requests();
        let script_changed = script_changed | self.resume_background_loading();
        let revision = self.network.revision();
        let mut changed = pause_changed | script_changed | responsive_changed | self.process_media_selection() | self.poll_embedded_video() | self.poll_media() | self.poll_audio() | self.poll_media_sync() | self.poll_downloads()
            || revision != self.last_network_revision
            || self.network_needs_tick();
        changed |= self.flush_embedded_video();
        self.last_network_revision = revision;
        let Some(worker) = &self.worker else {
            return changed;
        };
        let poll = worker.poll();
        changed |= self.apply_loader_poll(poll);
        if changed && self.view == View::Resources {
            self.refresh_resource_view();
        }
        changed | self.process_responsive_images()
    }

    // Shared delivery path: handoff completion retains the ordinary resource,
    // source-ID and load-event ordering without polling unrelated script work.
    fn apply_loader_poll(&mut self, poll: loader::Poll) -> bool {
        let mut changed = false;
        if let Some((id, text)) = poll.progress {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == id)
            {
                self.status = format!("{} | Esc stops", text);
                changed = true;
            }
        }
        if let Some((id, response)) = poll.download {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == id)
            {
                self.download_received(response);
                self.address.set(&self.location.display());
                changed = true;
            }
        }
        if let Some((id, loaded)) = poll.document {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == id && !pending.committed)
            {
                let pending = self.pending.as_mut().unwrap();
                pending.committed = true;
                pending.location = loaded.location.clone();
                pending.request = loaded.request.clone();
                let kind = pending.kind;
                pending.scroll_target = match kind {
                    NavigationKind::Reload(scroll)
                    | NavigationKind::History(_, scroll)
                    | NavigationKind::ScriptReplace(Some(scroll)) => {
                        Some(LoadingScroll::Offset(scroll))
                    }
                    NavigationKind::New
                    | NavigationKind::ScriptNew
                    | NavigationKind::ScriptReplace(None)
                        if !loaded.location.fragment.is_empty() =>
                    {
                        Some(LoadingScroll::Fragment)
                    }
                    _ => None,
                };
                let destination = pending.location.display();
                if self.finish_navigation(kind, loaded, true) {
                    self.committed_request = Some(id);
                    self.status.push_str(" | Loading images; Esc stops");
                } else {
                    if let Some(pending) = &mut self.pending {
                        pending.committed = false;
                    }
                    self.cancel_pending();
                    self.address.set(&self.location.display());
                    self.last_failed_navigation = Some(format!(
                        "{destination}\n{}\nThe previous page is still displayed.",
                        self.status
                    ));
                }
                changed = true;
            }
        }
        if let Some((id, mut update)) = poll.images {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == id && pending.committed)
            {
                update.images.retain(|node, _| self.scripting.as_ref().is_none_or(|page| page.initial_image_current(*node)));
                update.animations.retain(|node, _| update.images.contains_key(node));
                update.outcomes.retain(|(node, _)| self.scripting.as_ref().is_none_or(|page| page.initial_image_current(*node)));
                // Dynamic and parser assets share the retained document bound.
                let nodes: Vec<_> = update.outcomes.iter().map(|(node, _)| *node).filter(|node| update.images.contains_key(node)).collect();
                for node in nodes {
                    let mut asset = crate::image::DecodedImage { bitmap: update.images.remove(&node).unwrap(), animation: update.animations.remove(&node), notice: None };
                    let result = self.fit_document_image(node, &mut asset);
                    if let Err(error) = result {
                        if let Some((_, success)) = update.outcomes.iter_mut().find(|(candidate, _)| *candidate == node) { *success = false; }
                        update.events.push(format!("IMAGE node {node}: {error}"));
                    } else {
                        self.animations.remove(&node);
                        if let Some(notice) = asset.notice { update.events.push(format!("IMAGE node {node}: {notice}")); }
                        self.images.insert(node, asset.bitmap);
                        if let Some(animation) = asset.animation { self.install_animations(HashMap::from([(node, animation)])); }
                    }
                }
                let has_images = update.outcomes.iter().any(|(_, success)| *success);
                self.events.extend(update.events);
                if let Some(page) = &mut self.scripting {
                    for &(node, success) in &update.outcomes { page.settle_initial_image(node, success); }
                }
                if has_images {
                    self.update_script_geometry();
                }
                if has_images && self.view == View::Page {
                    // Node IDs and the document realm remain stable. Reflow
                    // never rebuilds Forms or executes startup scripts again.
                    self.reflow();
                    self.restore_loading_scroll();
                    self.sync_editors();
                }
                for (node, success) in update.outcomes {
                    if let Some(page) = &mut self.scripting {
                        if page.initial_image_current(node) {
                            let outcome = page.finish_image_event(&mut self.document, &mut self.forms, node, success);
                            if outcome.layout_changed { self.reflow(); }
                            self.resume_background_loading();
                        }
                    }
                }
                self.show_script_errors();
                changed = true;
            }
        }
        if let Some(completion) = poll.completed {
            if self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == completion.id)
            {
                let pending = self.pending.take().unwrap();
                changed = true;
                match completion.result {
                    Ok(end) if end.download => {
                        self.address.set(&self.location.display());
                    }
                    Ok(end) if pending.committed => {
                        self.events.extend(end.events);
                        self.set_load_status(&end.notice, end.skipped);
                        let outcome = self
                            .scripting
                            .as_mut()
                            .map(|page| page.finish_loading(&mut self.document, &mut self.forms));
                        if outcome.is_some_and(|outcome| outcome.layout_changed) {
                            self.reflow();
                            if self.view == View::Page {
                                self.apply_loading_scroll(pending.scroll_target);
                            }
                        }
                        self.validate_control_focus();
                        self.sync_editors();
                        self.show_script_errors();
                        self.failed = false;
                    }
                    Ok(_) => self.error("The loader finished without a document.".into()),
                    Err(error) => {
                        self.address.set(&self.location.display());
                        self.last_failed_navigation = Some(format!(
                            "{}\n{}\n{}",
                            pending.location.display(),
                            error,
                            if pending.committed {
                                "The new document is displayed; its remaining resources did not finish."
                            } else {
                                "The previous page is still displayed."
                            }
                        ));
                        self.error(format!(
                            "{}: {}",
                            fit_text(&pending.location.display(), 100),
                            error
                        ));
                    }
                }
            }
        }
        changed
    }

    fn restore_loading_scroll(&mut self) {
        let target = self
            .pending
            .as_ref()
            .and_then(|pending| pending.scroll_target);
        self.apply_loading_scroll(target);
    }

    fn apply_loading_scroll(&mut self, target: Option<LoadingScroll>) {
        match target {
            Some(LoadingScroll::Offset(scroll)) => {
                self.set_scroll(scroll.clamp(0, self.max_scroll()));
            }
            Some(LoadingScroll::Fragment) => self.jump_fragment(),
            None => {}
        }
    }

    fn user_changed_loading_position(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.scroll_target = None;
        }
    }

    fn refresh_resource_view(&mut self) {
        let scroll = self.scroll;
        let focus = self.focus;
        let menu = self.menu;
        self.view = View::Source;
        self.toggle_view(View::Resources);
        self.set_scroll(scroll.clamp(0, self.max_scroll()));
        self.focus = focus;
        self.menu = menu;
    }
    pub fn wait_for_load(&mut self) {
        let started = std::time::Instant::now();
        while self.is_loading() {
            self.poll_load();
            // While already waiting for image I/O, service the displayed
            // document's queued tasks as the window event loop does. This
            // adds no post-load waiting; --wait-ms controls that separately.
            if self.is_loading() {
                self.poll_page_timer();
            }
            if started.elapsed() > std::time::Duration::from_secs(90) {
                self.stop_loading();
                self.error("Page loading exceeded the headless 90-second limit.".to_owned());
                break;
            }
            if self.is_loading() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    pub fn next_script_timer(&self) -> Option<std::time::Instant> {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.committed)
            || self.resubmission.is_some()
            || !self.scripts_enabled
            || !self.document.scripting_enabled
        {
            return None;
        }
        self.scripting.as_ref()?.next_timer_deadline()
    }

    pub fn poll_script_timer(&mut self) -> bool {
        self.inspector.invalidate();
        if self.process_script_requests() {
            return true;
        }
        if self.next_script_timer().is_none() {
            return false;
        }
        let Some(outcome) = self
            .scripting
            .as_mut()
            .and_then(|page| page.poll_timer(&mut self.document, &mut self.forms))
        else {
            return false;
        };
        if outcome.layout_changed {
            self.reflow();
        }
        self.validate_control_focus();
        self.sync_editors();
        if outcome.errors > 0 {
            self.status = format!(
                "JavaScript timer: {} error(s). Ctrl+I shows details.",
                outcome.errors
            );
            self.failed = false;
        }
        if self.view == View::Resources {
            // Refresh a diagnostic snapshot after asynchronous work, retaining
            // the reader's scroll position and the underlying page position.
            self.refresh_resource_view();
        }
        self.process_script_requests();
        true
    }

    fn validate_control_focus(&mut self) {
        if let Focus::Control(node) = self.focus {
            if self
                .forms
                .controls
                .get(&node)
                .is_none_or(|control| !control.focusable())
                || !self
                    .page
                    .controls
                    .iter()
                    .any(|control| control.node == node)
            {
                self.focus = Focus::Page;
            }
        }
        if self.select_popup.is_some_and(|popup| {
            self.focus != Focus::Control(popup.node)
                || self
                    .forms
                    .controls
                    .get(&popup.node)
                    .is_none_or(|control| control.disabled || control.kind != Kind::Select)
        }) {
            self.select_popup = None;
            self.drag = Drag::None;
        }
    }

    pub fn script_timer_unavailable(&mut self) {
        self.suspend_media();
        if let Some(page) = &mut self.scripting {
            page.stop_timers("The operating system could not schedule the timer wake-up.");
        }
        self.animations.clear();
        self.events.push(
            "Page timer could not be scheduled; image animations stopped at their current frame."
                .into(),
        );
        self.status = "Page timer could not be scheduled. Ctrl+I shows details.".into();
    }

    /// Optional headless settling time. It returns early when the queue empties;
    /// pending timers never extend the caller's fixed deadline.
    pub fn wait_for_script_timers(&mut self, duration: std::time::Duration) {
        let deadline = std::time::Instant::now() + duration;
        while std::time::Instant::now() < deadline {
            if self.poll_load() | self.poll_page_timer() {
                continue;
            }
            let due = match self.next_page_timer() {
                Some(due) => due,
                None if self.is_loading() || self.scripting.as_ref().is_some_and(|page|page.has_native_script_requests()) => {
                    std::time::Instant::now() + std::time::Duration::from_millis(10)
                }
                None => break,
            };
            let wait = due
                .min(deadline)
                .saturating_duration_since(std::time::Instant::now());
            if !wait.is_zero() {
                std::thread::sleep(wait.min(std::time::Duration::from_millis(10)));
            }
        }
    }
    fn finish_navigation(
        &mut self,
        kind: NavigationKind,
        loaded: Loaded,
        images_pending: bool,
    ) -> bool {
        self.apply_history_replacement();
        self.save_scroll();
        let location = loaded.location.clone();
        let same = self.location == location && self.current_request == loaded.request;
        let index = match kind {
            NavigationKind::History(index, _) => index,
            _ => self.history_index,
        };
        let restore = !loaded.document_redirected
            && matches!(
                kind,
                NavigationKind::Reload(_) | NavigationKind::History(_, _)
            );
        let previous_document = if restore {
            self.history
                .borrow()
                .entry(index)
                .map(|entry| entry.document)
        } else {
            None
        };
        let data = if restore {
            let copied = self
                .history
                .borrow()
                .entry(index)
                .map(|entry| entry.data.try_copy())
                .transpose();
            match copied {
                Ok(data) => data.unwrap_or_default(),
                Err(error) => {
                    self.error(error);
                    return false;
                }
            }
        } else {
            Default::default()
        };
        let request = match crate::session_history::copy_request(&loaded.request) {
            Ok(request) => request,
            Err(error) => {
                self.error(error);
                return false;
            }
        };
        let append = !same && matches!(kind, NavigationKind::New | NavigationKind::ScriptNew);
        let entry = HistoryEntry {
            location: location.clone(),
            scroll: 0,
            request,
            document: self.script_epoch.wrapping_add(1),
            data,
        };
        let result = {
            let mut history = self.history.borrow_mut();
            history
                .prepare_at(entry, !append, index)
                .and_then(|change| history.commit(change))
        };
        if let Err(error) = result {
            self.error(error);
            return false;
        }
        self.history_index = self.history.borrow().index();
        self.commit(location, loaded, images_pending);
        self.history_index = self.history.borrow().index();
        match kind {
            NavigationKind::Reload(scroll) | NavigationKind::History(_, scroll) => {
                self.set_scroll(scroll.clamp(0, self.max_scroll()))
            }
            NavigationKind::ScriptReplace(Some(scroll)) => {
                self.set_scroll(scroll.clamp(0, self.max_scroll()))
            }
            _ => {}
        }
        if let Some(previous) = previous_document {
            let result = self.history.borrow_mut().retag_document(
                previous,
                self.script_epoch,
                &self.location,
            );
            if let Err(error) = result {
                self.error(error);
                return true;
            }
        }
        self.save_scroll();
        self.apply_history_replacement();
        self.record_browsing_visit();
        true
    }
    pub fn home(&mut self) {
        self.browser_panel = None;
        if let Some(home) = self.profile.data().settings.home.clone() {
            match navigation::address(&home, &Location::home()) {
                Ok(location) => self.navigate(location),
                Err(error) => self.error(error),
            }
        } else {
            self.navigate(self.home_location.clone());
        }
    }
    pub fn reload(&mut self) {
        self.reload_with_mode(crate::http::FetchMode::Normal);
    }
    fn reload_with_mode(&mut self, mode: crate::http::FetchMode) {
        self.apply_history_replacement();
        if let Some(pending) = self.pending.as_ref().filter(|pending| !pending.committed) {
            let (location, kind) = (pending.location.clone(), pending.kind);
            let request = pending.request.clone();
            let mode = if pending.mode == crate::http::FetchMode::FullReload {
                pending.mode
            } else {
                mode
            };
            if request.is_post() {
                self.ask_resubmission(location, request, kind, mode);
            } else {
                self.start_network(location, kind, mode);
            }
            return;
        }
        let old = if self.view == View::Page {
            self.scroll
        } else {
            self.page_scroll
        };
        if self.location.url.is_some() || crate::media::is_local_media(&self.location) {
            if self.current_request.is_post() {
                self.ask_resubmission(
                    self.location.clone(),
                    self.current_request.clone(),
                    NavigationKind::Reload(old),
                    mode,
                );
            } else {
                self.start_network(self.location.clone(), NavigationKind::Reload(old), mode);
            }
            return;
        }
        match loader::local_with_options(&self.location, self.load_options()) {
            Ok(loaded) => {self.finish_navigation(NavigationKind::Reload(old), loaded, false);},
            Err(e) => self.error(e),
        }
    }
    fn save_scroll(&mut self) {
        let scroll = if self.view == View::Page {
            self.scroll
        } else {
            self.page_scroll
        };
        let result = self.history.borrow_mut().set_scroll(scroll);
        if let Err(error) = result {
            self.error(error);
        }
    }
    fn ask_resubmission(
        &mut self,
        location: Location,
        request: crate::http::Request,
        kind: NavigationKind,
        mode: crate::http::FetchMode,
    ) {
        self.cancel_pending();
        self.address.set(&self.location.display());
        self.next_confirmation = self.next_confirmation.wrapping_add(1);
        self.resubmission = Some(Resubmission {
            token: self.next_confirmation,
            location,
            request,
            kind,
            mode,
            offered: false,
        });
        self.status =
            "This page needs the previous POST form data. Confirm resending to continue.".into();
        self.failed = false;
        self.menu = false;
    }
    /// The platform owns the native dialog; no App reference crosses its modal
    /// message loop. Navigation invalidates the token before a delayed answer.
    pub fn take_resubmission_prompt(&mut self) -> Option<(u64, String)> {
        let question = self.resubmission.as_mut()?;
        if question.offered {
            return None;
        }
        question.offered = true;
        Some((question.token, question.location.display()))
    }
    pub fn confirm_resubmission(&mut self, token: u64, accepted: bool) {
        if !self
            .resubmission
            .as_ref()
            .is_some_and(|question| question.token == token && question.offered)
        {
            return;
        }
        let question = self.resubmission.take().unwrap();
        if accepted {
            self.start_network_request(
                question.location,
                question.kind,
                question.mode,
                question.request,
            );
        } else {
            self.status = "Form data was not resent.".into();
            self.failed = false;
        }
    }
    fn navigate(&mut self, location: Location) {
        self.navigate_location(location, false);
    }
    fn navigate_location(&mut self, location: Location, fragment_navigation: bool) {
        self.navigate_location_mode(location, fragment_navigation, false);
    }
    fn navigate_location_mode(
        &mut self,
        location: Location,
        fragment_navigation: bool,
        replace: bool,
    ) {
        self.navigate_location_mode_with_link_origin(location, fragment_navigation, replace, None);
    }
    fn navigate_location_mode_with_link_origin(
        &mut self,
        location: Location,
        fragment_navigation: bool,
        replace: bool,
        link_origin: Option<Option<crate::url::HttpUrl>>,
    ) {
        self.invalidate_platform_requests();
        self.apply_history_replacement();
        let location = if location.url.is_none()
            && location.path.is_none()
            && self.home_location.path.is_some()
        {
            Location {
                fragment: location.fragment,
                ..self.home_location.clone()
            }
        } else {
            location
        };
        let same_document = location.url == self.location.url
            && location.path == self.location.path
            && location.root == self.location.root
            && (!self.current_request.is_post() || fragment_navigation);
        let keep_images = same_document
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.committed);
        if !keep_images {
            self.cancel_pending();
        } else if let Some(pending) = &mut self.pending {
            pending.location = location.clone();
            pending.scroll_target = if location.fragment.is_empty() {
                None
            } else {
                Some(LoadingScroll::Fragment)
            };
        }
        if !same_document && (location.url.is_some() || crate::media::is_local_media(&location)) {
            self.start_network_request_with_link_origin(
                location,
                if replace {
                    NavigationKind::ScriptReplace(None)
                } else {
                    NavigationKind::New
                },
                crate::http::FetchMode::Normal,
                crate::http::Request::Get,
                None,
                link_origin,
            );
            return;
        }
        if !same_document {
            match loader::local_with_options(&location, self.load_options()) {
                Ok(loaded) => {
                    self.finish_navigation(
                        if replace {
                            NavigationKind::ScriptReplace(None)
                        } else {
                            NavigationKind::New
                        },
                        loaded,
                        false,
                    );
                }
                Err(error) => self.navigation_error(&location.display(), error),
            }
            return;
        }
        self.last_failed_navigation = None;
        self.save_scroll();
        let same = self.location == location;
        if !same {
            let entry = HistoryEntry {
                location: location.clone(),
                scroll: self.scroll,
                request: self.current_request.clone(),
                document: self.script_epoch,
                data: Default::default(),
            };
            let result = self.history.borrow_mut().update(entry, replace);
            if let Err(error) = result {
                self.error(error);
                return;
            }
            self.history_index = self.history.borrow().index();
        }
        self.location = location;
        if let Some(page) = &mut self.scripting {
            page.set_location(&self.location);
        }
        self.address.set(&self.location.display());
        if self.view != View::Page {
            self.view = View::Page;
            self.alternate = None;
            self.reflow();
        }
        if self.location.fragment.is_empty() {
            self.set_scroll(0);
        }
        self.focus = Focus::Page;
        self.selection = None;
        self.drag = Drag::None;
        self.hover = None;
        self.menu = false;
        self.status.clear();
        self.failed = false;
        self.jump_fragment();
        self.apply_history_replacement();
    }
    fn navigation_history_index(&self) -> usize {
        match self
            .pending
            .as_ref()
            .filter(|pending| !pending.committed)
            .map(|pending| pending.kind)
        {
            Some(NavigationKind::History(index, _)) => index,
            _ => self.history_index,
        }
    }
    fn history_move(&mut self, delta: isize) {
        self.invalidate_platform_requests();
        self.apply_history_replacement();
        let index = self.navigation_history_index() as isize + delta;
        if index < 0 || index >= self.history.borrow().len() as isize {
            return;
        }
        let index = index as usize;
        let (location, scroll, document, request) = {
            let history = self.history.borrow();
            let entry = history.entry(index).unwrap();
            (
                entry.location.clone(),
                entry.scroll,
                entry.document,
                crate::session_history::copy_request(&entry.request),
            )
        };
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        let compatible = match (&location.url, &self.location.url) {
            (Some(old), Some(current)) => old.same_origin(current),
            (None, None) => {
                location.path == self.location.path && location.root == self.location.root
            }
            _ => false,
        };
        let same_document = document == self.script_epoch && compatible;
        let keep_images = same_document
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.committed);
        if !keep_images {
            self.cancel_pending();
        } else if let Some(pending) = &mut self.pending {
            pending.location = location.clone();
            pending.scroll_target = Some(LoadingScroll::Offset(scroll));
        }
        if same_document {
            self.save_scroll();
            let result = self.history.borrow_mut().traverse(index);
            if let Err(error) = result {
                self.error(error);
                return;
            }
            self.location = location;
            if let Some(page) = &mut self.scripting {
                page.set_location(&self.location);
            }
            if self.base_is_document_url {
                self.base_location = self.location.clone();
            }
            if let Some(page) = &mut self.scripting {
                page.set_resource_base(&self.base_location);
            }
            self.address.set(&self.location.display());
            if self.view != View::Page {
                self.view = View::Page;
                self.alternate = None;
                self.reflow();
            }
            self.history_index = index;
            self.set_scroll(scroll.clamp(0, self.max_scroll()));
            self.selection = None;
            self.focus = Focus::Page;
            self.status.clear();
            self.failed = false;
            return;
        }
        if location.url.is_some() || crate::media::is_local_media(&location) {
            if request.is_post() {
                self.ask_resubmission(
                    location,
                    request,
                    NavigationKind::History(index, scroll),
                    crate::http::FetchMode::Normal,
                );
            } else {
                self.start_network(
                    location,
                    NavigationKind::History(index, scroll),
                    crate::http::FetchMode::Normal,
                );
            }
            return;
        }
        match loader::local_with_options(&location, self.load_options()) {
            Ok(loaded) => {
                self.finish_navigation(NavigationKind::History(index, scroll), loaded, false);
            }
            Err(e) => self.error(e),
        }
    }
    fn follow(&mut self, href: &str) {
        self.apply_history_replacement();
        match navigation::resolve(href, &self.base_location) {
            Ok(location) => self.navigate_location(location, href.contains('#')),
            Err(e) => self.error(e),
        }
    }
    fn commit(&mut self,location:Location,loaded:Loaded,images_pending:bool){
        self.invalidate_platform_requests();
        self.cancel_embedded_video();
        self.embedded_video=None;
        self.cancel_script_requests();
        self.script_epoch = self.script_epoch.wrapping_add(1);
        self.committed_request = None;
        self.network = loaded.network.clone();
        self.cookie_same_site = loaded.cookie_same_site;
        self.current_request = loaded.request;
        self.resubmission = None;
        let scripts = loaded.scripts;
        self.inspector.navigation(&scripts);
        self.scripting_blocked = loaded.scripting_blocked;
        self.document_policy = loaded.csp.clone();
        self.scripting = None;
        self.base_location = loaded.base;
        self.base_is_document_url = loaded.base_is_document_url;
        self.inline_svg_sources.clear();
        self.document = loaded.document;
        self.stylesheet_cache = None;
        // The viewport may have changed since loading began. Startup scripts
        // can synchronously request styles before the first reflow.
        self.document.media_environment = Some(self.css_media_environment());
        self.document.scripting_enabled = self.scripts_enabled && self.scripting_blocked.is_none();
        self.forms = Forms::new(&self.document);
        self.raw = loaded.raw;
        self.styles = loaded.styles;
        self.style_bases = loaded.style_bases;
        self.backgrounds.clear();
        self.background_requests.clear();
        self.pending_backgrounds.clear();
        self.background_generation = 0;
        self.fonts = self.base_fonts.clone();
        self.fonts.extend(&loaded.fonts);
        self.images = loaded.images;
        self.image_densities = loaded.image_densities;
        self.responsive_images = responsive_images::State::new(
            loaded.responsive_images,
            loaded.image_environment,
            &loaded.image_sources,
            &self.image_densities,
        );
        self.animations.clear();
        self.install_animations(loaded.animations);
        self.events = loaded.events;
        self.install_media(loaded.media,loaded.media_node,false);
        self.install_audio(loaded.audio);
        self.install_media_sync();
        self.last_failed_navigation = None;
        self.source = location.path.clone();
        self.location = location;
        self.prepare_embedded_video();
        self.remember_visit();
        self.close_bookmark_popup();
        self.view = View::Page;
        self.alternate = None;
        self.set_scroll(0);
        self.wheel_remainder = 0;
        self.pending_surrogate = None;
        self.selection = None;
        self.drag = Drag::None;
        self.hover = None;
        self.menu = false;
        self.focus = Focus::Page;
        self.address.set(&self.location.display());
        self.set_load_status(&loaded.notice, loaded.skipped);
        self.failed = false;
        if self.document.scripting_enabled {
            let mut page = crate::scripting::Page::new();
            page.set_background_pending(false);
            page.set_navigation_timing(loaded.document_timing);
            page.set_embedded_video(self.embedded_video_snapshot());
            page.set_location(&self.location);
            page.set_cookie_session(self.cookies.clone());
            page.set_resource_base(&self.base_location);
            page.attach_session_history(self.history.clone(),self.script_epoch,self.base_is_document_url);
            page.set_stylesheet_sources(&self.document,&self.styles);
            page.set_initial_images(loaded.image_sources, &self.images, images_pending && self.images_enabled, self.images_enabled);
            page.set_geometry(crate::script_geometry::Geometry::new(
                self.width - 12,
                self.viewport_height(),
                self.zoom,
                &self.styles,
                inline_svg::natural_sizes(&self.images,&self.image_densities,&self.inline_svg_sources),
                &self.fonts,
            ).with_limits(self.geometry_limits));
            // Startup scripts and DOMContentLoaded can select CSS images too.
            // Commit their real resource plan before the window-load checkpoint.
            page.start_before_load(&mut self.document, &mut self.forms, &scripts, &loaded.csp);
            self.scripting = Some(page);
        }
        self.reflow();
        if !images_pending {
            let outcome = self.scripting.as_mut().map(|page| page.finish_loading(&mut self.document, &mut self.forms));
            if outcome.is_some_and(|outcome| outcome.layout_changed) { self.reflow(); }
        }
        self.jump_fragment();
        self.show_script_errors();
        self.flush_embedded_video();
    }
    fn set_load_status(&mut self, notice: &str, skipped: usize) {
        self.status = if skipped == 0 {
            notice.to_owned()
        } else {
            format!(
                "{}{}{} resource(s) skipped. Ctrl+I shows details.",
                notice,
                if notice.is_empty() { "" } else { " | " },
                skipped
            )
        };
    }
    fn show_script_errors(&mut self) {
        self.apply_script_scroll();
        // Committed media effects survive a producer error. Apply their issue
        // time before selection or a fresh snapshot can replace the projection.
        self.apply_media_pause();
        self.process_media_selection();
        self.apply_history_replacement();
        let script_errors = self.scripting.as_ref().map_or(0, |page| page.error_count());
        if script_errors != 0 {
            self.status = format!("JavaScript subset: {script_errors} script error(s). Ctrl+I shows details; Ctrl+J toggles scripting.");
        }
    }
    fn jump_fragment(&mut self) {
        let fragment = &self.location.fragment;
        if fragment.is_empty() || self.page.anchor_is_fixed(fragment) {
            return;
        }
        if let Some((_, y)) = self.page.anchors.iter().find(|(id, _)| id == fragment) {
            self.set_scroll((*y - 12).clamp(0, self.max_scroll()));
        } else if fragment.eq_ignore_ascii_case("top") {
            self.set_scroll(0);
        } else {
            self.status = format!("No anchor named {}", fragment);
        }
    }
    fn reflow(&mut self) {
        self.sync_native_focus_css();
        let svg_started=std::time::Instant::now();
        let environment=Some(self.css_media_environment());
        self.document.media_environment=environment;
        let previous_svg_error=self.status.starts_with("Inline SVG");
        let svg_complete=self.sync_inline_svg(svg_started);
        self.anchor_geometry_scope = std::rc::Rc::new(());
        if let Some(report)=&mut self.alternate {report.media_environment=environment;}
        self.inspector.invalidate();
        self.select_popup = None;
        self.page = if let Some(doc) = &self.alternate {
            layout::build_viewport_with_fonts(
                doc,
                self.width - 12,
                self.viewport_height(),
                self.zoom,
                &HashMap::new(),
                &HashMap::new(),
                None,
                &self.base_fonts,
            )
        } else {
            let sizes = inline_svg::natural_sizes(&self.images,&self.image_densities,&self.inline_svg_sources);
            let checked = |node| {
                self.forms
                    .controls
                    .get(&node)
                    .filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
                    .map(|control| control.checked)
            };
            layout::build_viewport_with_fonts_cached(
                &self.document,
                self.width - 12,
                self.viewport_height(),
                self.zoom,
                &self.styles,
                &sizes,
                Some(&checked),
                &self.fonts,
                self.stylesheet_cache.as_ref(),
            )
        };
        if self.alternate.is_none() {
            self.stylesheet_cache = self.page.stylesheet_cache.clone();
            let viewports_complete=self.sync_inline_svg_viewports(svg_started);
            if previous_svg_error && svg_complete && viewports_complete && self.status.starts_with("Inline SVG") {self.status="Ready".into();}
        }
        self.sync_background_images();
        self.sync_media_controls();
        self.observe_layout();
        self.rebuild_link_colors();
        if self.alternate.is_none() {
            let viewport_height = self.viewport_height();
            if let Some(scripting) = &mut self.scripting {
                scripting.sync_geometry(
                    self.width - 12,
                    viewport_height,
                    self.zoom,
                    inline_svg::natural_sizes(&self.images,&self.image_densities,&self.inline_svg_sources),
                    &self.page,
                );
            }
        } else {
            self.update_script_geometry();
        }
        self.selection = None;
        self.drag = Drag::None;
        if !self.apply_script_scroll() {
            self.set_scroll(self.scroll.clamp(0, self.max_scroll()));
        }
        self.update_find(false);
        self.validate_control_focus();
        if self.sync_native_focus_css(){self.reflow();}
        self.refresh_native_hover();
    }
    pub fn resize(&mut self, width: i32, height: i32) {
        let width = width.clamp(1, 8192);
        let height = height.clamp(1, 8192);
        let changed = self.width != width || self.height != height;
        self.width = width;
        self.height = height;
        self.keep_menu_selection_visible();
        if changed {
            self.chrome.hover = None;
            self.chrome.pressed = None;
            self.reflow();
            self.process_responsive_images();
        }
        self.set_scroll(self.scroll.clamp(0, self.max_scroll()));
        self.sync_editors();
        self.inspector_sync_editor();
    }
    pub fn set_zoom(&mut self, zoom: f32) {
        if !zoom.is_finite() {
            return;
        }
        let fraction = self.scroll as f32 / self.page.height.max(1) as f32;
        self.zoom = zoom.clamp(0.5, 3.0);
        self.reflow();
        self.process_responsive_images();
        self.set_scroll(((fraction * self.page.height as f32) as i32).clamp(0, self.max_scroll()));
        self.sync_editors();
    }
    pub fn content_top(&self) -> i32 {
        self.chrome_bottom() + if self.find_open { FIND_HEIGHT } else { 0 }
    }
    pub fn viewport_height(&self) -> i32 {
        (self.content_bottom() - self.content_top()).max(1)
    }
    pub fn max_scroll(&self) -> i32 {
        (self.page.height - self.viewport_height()).max(0)
    }
    fn apply_script_scroll(&mut self) -> bool {
        let Some(target) = self.scripting.as_mut().and_then(|page| page.take_viewport_scroll_request()) else {
            return false;
        };
        if self.view == View::Page {
            self.set_scroll(target);
        } else {
            // Source and Resources have their own viewport. Update the saved
            // page position for restoration when the document is shown again.
            self.page_scroll = target;
        }
        true
    }
    fn set_scroll(&mut self, scroll: i32) {
        let scroll = scroll.clamp(0, self.max_scroll());
        let moved=scroll!=self.scroll;
        if moved {
            self.anchor_geometry_scope = std::rc::Rc::new(());
            self.select_popup = None;
        }
        self.scroll = scroll;
        self.page.set_scroll(self.scroll);
        if let Some(page) = &mut self.scripting {
            page.set_geometry_scroll(if self.view == View::Page { self.scroll } else { self.page_scroll });
        }
        if moved{self.refresh_native_hover();}
    }
    pub fn scroll_by(&mut self, delta: i32) {
        self.user_changed_loading_position();
        self.set_scroll(
            self.scroll
                .saturating_add(delta)
                .clamp(0, self.max_scroll()),
        );
    }
    pub fn wheel(&mut self, delta: i32, ctrl: bool) {
        if self.downloads_wheel(delta) || self.bookmark_bar_wheel(delta) {
            return;
        }
        if self.menu {
            self.chrome_wheel(delta);
            return;
        }
        if self.browser_panel.is_some() {
            self.browser_wheel(delta);
            return;
        }
        self.user_changed_loading_position();
        if ctrl {
            self.set_zoom(self.zoom + delta as f32 / 1200.0);
            return;
        }
        if let (Some(popup), Some(bounds)) = (self.select_popup, self.select_popup_box()) {
            let rows = self.forms.controls[&popup.node]
                .select
                .as_ref()
                .unwrap()
                .rows
                .len();
            let popup = self.select_popup.as_mut().unwrap();
            popup.wheel_remainder = popup.wheel_remainder.saturating_add(delta);
            let steps = popup.wheel_remainder / 120;
            popup.wheel_remainder %= 120;
            popup.first = (popup.first as i32)
                .saturating_sub(steps.saturating_mul(3))
                .clamp(0, rows.saturating_sub(bounds.visible) as i32)
                as usize;
            return;
        }
        self.wheel_remainder = self
            .wheel_remainder
            .saturating_add(delta.saturating_mul(72));
        let pixels = self.wheel_remainder / 120;
        self.wheel_remainder %= 120;
        self.scroll_by(-pixels);
    }
    pub fn title(&self) -> String {
        let title = if self.document.title.is_empty() {
            "Untitled"
        } else {
            &self.document.title
        };
        let view = match self.view {
            View::Page => "",
            View::Source => "Source: ",
            View::Resources => "Resources: ",
        };
        format!(
            "{}{} - ChezCat",
            view,
            title.chars().take(160).collect::<String>()
        )
    }
    pub fn show_source(&mut self) {
        self.toggle_view(View::Source);
    }
    pub fn show_resources(&mut self) {
        self.toggle_view(View::Resources);
    }
    fn script_activity(&self) -> String {
        if !self.scripts_enabled {
            "JavaScript is OFF. Ctrl+J enables it and reloads the page.".to_owned()
        } else if let Some(reason) = &self.scripting_blocked {
            format!("JavaScript blocked on this page: {reason}")
        } else {
            self.scripting
                .as_ref()
                .map(|page| page.report())
                .unwrap_or_else(|| "No active page runtime.".into())
        }
    }
    /// Original decoded source of the displayed document, before DOM mutations.
    pub fn document_source(&self) -> &str {
        &self.raw
    }
    /// Export the current activity without changing the view or executing code.
    pub fn activity_report(&self) -> String {
        let mut report = format!(
            "{} activity report\nStatus: {}\nPage title: {}\nJavaScript enabled: {}\nImages enabled: {}\n\nTRUST FILE: {}\n\nLAST FAILED NAVIGATION: {}\n\nDOCUMENT: {}\nCONTENT ROOT / ORIGIN: {}\n\n{}\n\nJavaScript\n{}\n",
            crate::http::USER_AGENT,
            self.status,
            self.document.title,
            self.scripts_enabled,
            self.images_enabled,
            self.trust.description(),
            self.last_failed_navigation.as_deref().unwrap_or("None"),
            self.location.display(),
            self.location.root.as_ref().map(|p| p.display().to_string())
                .or_else(|| self.location.url.as_ref().map(|u| format!("{}://{}", if u.secure { "https" } else { "http" }, u.host_header())))
                .unwrap_or_else(|| "bundled".to_owned()),
            self.events.join("\n\n"),
            self.script_activity(),
        );
        for error in crate::css::stylesheet_diagnostics(&self.document,&self.styles) {report.push_str(&format!("CSS: {error}\n"));}
        report.push_str(&self.opacity_paint_report());
        report.push_str(&self.stage_report());
        report.push_str(&self.media_report());
        report.push_str(&self.audio_report());
        report
    }
    fn toggle_view(&mut self, requested: View) {
        self.apply_script_scroll();
        self.invalidate_platform_requests();
        let view = if self.view == requested {
            View::Page
        } else {
            requested
        };
        if self.view == View::Page {
            self.page_scroll = self.scroll;
        }
        self.view = view;
        self.set_scroll(0);
        self.focus = Focus::Page;
        self.menu = false;
        self.alternate = match view {
            View::Page => None,
            View::Source => Some(html::parse(&format!("<title>Page source</title><style>body {{ margin: 24px; }} pre {{ font-size: 14px; white-space: pre-wrap; }}</style><body><h1>Page source</h1><p>Ctrl+U returns to the page. Ctrl+F searches this view.</p><pre>{}</pre></body>", escape(&self.raw)))),
            View::Resources => {
                let scripting = self.script_activity();
                Some(html::parse(&format!(
                    "<title>Resource activity</title><style>body {{ margin: 24px; }} pre {{ font-size: 14px; white-space: pre-wrap; }}</style><body><h1>Resource activity</h1><p>Ctrl+I returns to the page. The document list belongs to the page still displayed.</p><pre>TRUST FILE: {}\n\nLAST FAILED NAVIGATION: {}\n\nDOCUMENT: {}\nCONTENT ROOT / ORIGIN: {}\n\n{}</pre><h2>JavaScript</h2><pre>{}</pre><p>Ctrl+J toggles scripting and reloads the page. The first language and DOM subset is limited; unsupported syntax and APIs are reported above.</p></body>",
                    escape(self.trust.description()),
                    escape(self.last_failed_navigation.as_deref().unwrap_or("None")),
                    escape(&self.location.display()),
                    escape(&self.location.root.as_ref().map(|p| p.display().to_string()).or_else(|| self.location.url.as_ref().map(|u| format!("{}://{}", if u.secure { "https" } else { "http" }, u.host_header()))).unwrap_or_else(|| "bundled".to_owned())),
                    escape(&format!("{}\n{}", self.events.join("\n\n"), self.opacity_paint_report())),
                    escape(&scripting),
                )))
            },
        };
        self.reflow();
        if view == View::Page {
            self.set_scroll(self.page_scroll.clamp(0, self.max_scroll()));
        }
    }
}

impl App {
    fn address_rect(&self) -> (i32, i32) {
        if self.width < 680 {
            (160, (self.width - 282).max(20))
        } else {
            (202, (self.width - 356).max(20))
        }
    }
    fn sync_native_focus_css(&mut self)->bool{
        if self.scripting.as_mut().is_some_and(|p|p.take_native_focus_loss()) && matches!(self.focus,Focus::Control(_)){self.focus=Focus::Page;}
        let next=if self.view==View::Page && self.alternate.is_none(){
            if let Focus::Control(node)=self.focus{self.forms.controls.get(&node).filter(|c|c.focusable()).and_then(|_|self.page.controls.iter().any(|c|c.node==node).then_some(node))}else{None}
        }else{None};
        if self.document.native_focus.get()==next{return false;}
        self.document.native_focus.set(next);self.invalidate_script_geometry();true
    }
    fn apply_native_focus_css(&mut self){
        if !self.sync_native_focus_css(){return;}
        self.reflow_preserving_drag();
    }
    pub(super) fn reflow_preserving_drag(&mut self){
        let selection=self.selection;
        let old_runs=selection.map(|_|std::mem::take(&mut self.page.runs));
        // CSS state changes must not discard the caret/drag committed by the
        // native press. A vanished control still cancels its old drag owner.
        let drag=std::mem::replace(&mut self.drag,Drag::None);
        self.reflow();
        if let Some(old)=old_runs{if old.len()==self.page.runs.len()&&old.iter().zip(&self.page.runs).all(|(a,b)|a.node==b.node&&a.text==b.text){self.selection=selection;}}
        let keep=match &drag{Drag::Control{node}|Drag::SelectOption{node,..}=>self.page.controls.iter().any(|c|c.node==*node),Drag::Field=>self.focus!=Focus::Page,_=>true};
        if keep{self.drag=drag;}
    }
    fn sync_editors(&mut self) {
        self.apply_native_focus_css();
        self.sync_browser_panel();
        self.address
            .ensure_visible(((self.address_rect().1 - 16) / 12).max(1) as usize);
        self.find
            .ensure_visible(((self.width - 320) / 12).max(1) as usize);
        if let Focus::Control(node) = self.focus {
            if let Some(rect) = self.page.controls.iter().find(|c| c.node == node) {
                if let Some(control) = self.forms.controls.get_mut(&node) {
                    if control.kind == Kind::TextArea {
                        textarea::sync(rect, &mut control.value);
                    } else if let Some(font) = &rect.text_font {
                        let shape = font.shape(&control.value.text());
                        let value = &mut control.value;
                        value.offset = value.offset.min(value.cursor).min(value.chars.len());
                        let available = (rect.width - 14).max(0);
                        while value.offset < value.cursor
                            && shape.x(value.cursor) - shape.x(value.offset) > available
                        {
                            value.offset += 1;
                        }
                    } else {
                        control.value.ensure_visible(
                            ((rect.width - 12) / (GLYPH_ADVANCE * rect.scale)).max(1) as usize,
                        );
                    }
                }
            }
        }
    }
    fn has_editor(&self) -> bool {
        match self.focus {
            Focus::Address | Focus::Find => true,
            Focus::Control(node) => self
                .forms
                .controls
                .get(&node)
                .is_some_and(|c| matches!(c.kind, Kind::Text | Kind::TextArea) && !c.disabled),
            Focus::Page => false,
        }
    }
    fn editor_editable(&self) -> bool {
        match self.focus {
            Focus::Control(node) => self.forms.controls.get(&node).is_some_and(|c| c.editable()),
            _ => self.has_editor(),
        }
    }
    fn editor(&mut self) -> &mut Editor {
        match self.focus {
            Focus::Find => &mut self.find,
            Focus::Control(node) => {
                &mut self
                    .forms
                    .controls
                    .get_mut(&node)
                    .expect("focused control exists")
                    .value
            }
            _ => &mut self.address,
        }
    }
    fn focus_control(&mut self, node: usize) {
        self.focus_control_with_scroll(node, true);
    }
    fn focus_control_with_scroll(&mut self, node: usize, scroll_into_view: bool) {
        if self.view != View::Page
            || !self
                .forms
                .controls
                .get(&node)
                .is_some_and(|c| c.focusable())
            || !self.page.controls.iter().any(|rect| rect.node == node)
        {
            return;
        }
        self.invalidate_platform_requests();
        self.select_popup = None;
        self.chrome.focus = None;
        let lost=self.scripting.as_mut().is_some_and(|page|page.take_native_focus_loss());
        let previous=if !lost{if let Focus::Control(previous)=self.focus {Some(previous)}else{None}}else{None};
        self.focus = Focus::Control(node);
        self.selection = None;
        self.pending_surrogate = None;
        self.menu = false;
        if scroll_into_view && !self.page.control_is_fixed(node) {
            if let Some(rect) = self.page.controls.iter().find(|r| r.node == node) {
                let (top, height) = (rect.y, rect.height);
                if top < self.scroll {
                    self.set_scroll(top);
                } else if top + height > self.scroll + self.viewport_height() {
                    self.set_scroll(top + height - self.viewport_height());
                }
            }
        }
        // Pointer activation still needs the horizontal offset that was on
        // screen when the click occurred. Its caret is assigned before syncing.
        if scroll_into_view {
            self.sync_editors();
        }
        if scroll_into_view {self.script_focus_transition(previous,Some(node));}
    }
    fn script_focus_transition(&mut self,previous:Option<usize>,_next:Option<usize>) {
        self.apply_native_focus_css();
        let next=if let Focus::Control(node)=self.focus{Some(node)}else{None};
        if previous==next || self.script_jobs_paused() {return;}
        if let Some(page)=&mut self.scripting {
            let outcome=page.focus_transition(&mut self.document,&mut self.forms,previous,next);
            if outcome.layout_changed {self.reflow();}
            self.apply_history_replacement();
            self.apply_script_scroll();
            self.validate_control_focus();
            self.sync_editors();
            if outcome.errors>0 {self.show_script_errors();}
        }
    }
    fn tab_focus(&mut self, previous: bool) {
        let mut candidates: Vec<_> = if self.view == View::Page {
            self.page
                .controls
                .iter()
                .filter_map(|rect| {
                    self.forms
                        .controls
                        .get(&rect.node)
                        .filter(|c| c.focusable() && c.tab_index >= 0)
                        .map(|c| {
                            (
                                if c.tab_index > 0 {
                                    (0, c.tab_index)
                                } else {
                                    (1, 0)
                                },
                                rect.node,
                            )
                        })
                })
                .collect()
        } else {
            Vec::new()
        };
        candidates.sort_unstable();
        let index = if let Focus::Control(node) = self.focus {
            candidates.iter().position(|(_, id)| *id == node)
        } else {
            None
        };
        let next = match index {
            Some(index) if previous => index.checked_sub(1),
            Some(index) => (index + 1 < candidates.len()).then_some(index + 1),
            None if previous => candidates.len().checked_sub(1),
            None => (!candidates.is_empty()).then_some(0),
        };
        if let Some(index) = next {
            self.focus_control(candidates[index].1);
        } else {
            self.focus_address();
        }
    }
    fn submit_form(&mut self, owner: usize, submitter: Option<usize>) {
        self.script_begin_job();
        self.submit_form_in_job(owner, submitter);
    }
    fn submit_form_in_job(&mut self, owner: usize, submitter: Option<usize>) {
        if let Err(error) = self
            .forms
            .validate_submission(&self.document, owner, submitter)
        {
            self.error(error);
            return;
        }
        if self.script_event(owner, "submit", true) {
            return;
        }
        match self.forms.submit_request(
            &self.document,
            owner,
            submitter,
            &self.location,
            &self.base_location,
        ) {
            Ok(submission) => {
                if let Err(error) = self
                    .document_policy
                    .allows_form_action(&submission.location)
                {
                    self.error(error);
                    return;
                }
                self.drag = Drag::None;
                self.start_network_request_with_policy(
                    submission.location,
                    NavigationKind::New,
                    crate::http::FetchMode::Normal,
                    submission.request,
                    Some(self.document_policy.clone()),
                );
            }
            Err(error) => self.error(error),
        }
    }
    fn activate_control(&mut self, node: usize) {
        self.activate_control_at(node, None);
    }
    fn activate_control_at(&mut self, node: usize, image_point: Option<(i32, i32)>) {
        let Some(control) = self.forms.controls.get(&node) else {
            return;
        };
        if control.disabled {
            return;
        }
        let owner = control.owner;
        let kind = control.kind;
        let old_checked = control.checked;
        let previous_radio = if kind == Kind::Radio {
            self.forms
                .controls
                .iter()
                .find(|(id, c)| {
                    c.kind == Kind::Radio
                        && c.checked
                        && c.owner == owner
                        && c.group_root == control.group_root
                        && c.name == control.name
                        && (!control.name.is_empty() || **id == node)
                })
                .map(|(&id, _)| id)
        } else {
            None
        };
        self.script_begin_job();
        if matches!(kind, Kind::Checkbox | Kind::Radio) {
            self.forms.toggle(node);
            self.invalidate_script_geometry();
        }
        let changed = self.forms.controls[&node].checked != old_checked;
        if self.script_event(node, "click", true) {
            match kind {
                Kind::Checkbox => self.forms.controls.get_mut(&node).unwrap().checked = old_checked,
                Kind::Radio => {
                    if let Some(previous) = previous_radio.filter(|&previous|self.forms.same_radio_group(previous,node)) {
                        // Restore this group's former selection. Script changes
                        // to unrelated controls are not part of default rollback.
                        self.forms.check_radio(previous);
                    } else {
                        self.forms.controls.get_mut(&node).unwrap().checked = false;
                    }
                }
                _ => {}
            }
            if matches!(kind, Kind::Checkbox | Kind::Radio) {
                self.reflow();
            }
            return;
        }
        if self.forms.controls.get(&node).is_none_or(|c| c.disabled) {
            return;
        }
        match kind {
            Kind::Checkbox | Kind::Radio => {
                if changed {
                    self.script_event(node, "input", false);
                    self.script_event(node, "change", false);
                }
                self.reflow();
            }
            Kind::Submit | Kind::Image => {
                if let Some(owner) = owner {
                    if kind == Kind::Image {
                        if let Some((x, y)) = image_point {
                            if let Err(error) = self.forms.select_image_point(node, x, y) {
                                self.error(error);
                                return;
                            }
                        }
                    }
                    self.submit_form_in_job(owner, Some(node));
                } else {
                    self.error("This button has no form.".into());
                }
            }
            Kind::Reset => {
                if let Some(owner) = owner {
                    if self.script_event(owner, "reset", true) {
                        return;
                    }
                    let outcome=match crate::scripting::reset_form(&mut self.document,&mut self.forms,self.scripting.as_mut(),owner){Ok(outcome)=>outcome,Err(error)=>{self.error(error);return;}};
                    self.apply_history_replacement();
                    self.apply_script_scroll();
                    self.apply_script_focus();
                    self.validate_control_focus();
                    self.invalidate_script_geometry();
                    self.reflow();
                    self.sync_editors();
                    self.status = if outcome.errors>0{format!("JavaScript subset: {} error(s). Ctrl+I shows details.",outcome.errors)}else{"Form reset.".into()};
                    self.failed = false;
                }
            }
            Kind::Button => {}
            Kind::Select => self.open_select(node),
            Kind::Unsupported => self.error(format!(
                "{} in this build.",
                self.forms.controls[&node].caption
            )),
            _ => {}
        }
    }
    fn image_point_at(&self, node: usize, x: i32, y: i32) -> Option<(i32, i32)> {
        if self.forms.controls.get(&node)?.kind != Kind::Image || !self.images.contains_key(&node) {
            return None;
        }
        let rect = self.page.controls.iter().find(|rect| rect.node == node)?;
        let image = rect.image.as_ref()?;
        let document_y = y
            .saturating_sub(self.content_top())
            .saturating_add(self.scroll);
        // A label can activate this control from outside its own border box.
        // Only a direct click on the rendered image selects a new coordinate.
        if !crate::paint::ClipRect::new(rect.x, rect.y, rect.width, rect.height)
            .contains(x, document_y)
        {
            return None;
        }
        // The coordinate origin is the rendered image's content edge, not its
        // intrinsic pixel grid. Keep signed padding/border coordinates and undo
        // browser zoom, truncating fractional CSS pixels toward zero.
        let css_x = (f64::from(x) - f64::from(rect.x) - f64::from(image.x)) / f64::from(self.zoom);
        let css_y =
            (f64::from(document_y) - f64::from(rect.y) - f64::from(image.y)) / f64::from(self.zoom);
        Some((css_x as i32, css_y as i32))
    }
    fn select_popup_box(&self) -> Option<SelectPopupBox> {
        let popup = self.select_popup?;
        if self.view != View::Page || self.focus != Focus::Control(popup.node) {
            return None;
        }
        let control = self.forms.controls.get(&popup.node)?;
        if control.disabled {
            return None;
        }
        let state = control.select.as_ref()?;
        if state.rows.is_empty() {
            return None;
        }
        let rect = self
            .page
            .controls
            .iter()
            .find(|rect| rect.node == popup.node)?;
        let row_height = rect
            .text_font
            .as_ref()
            .map_or((GLYPH_HEIGHT + 6) * rect.scale, |font| font.height() + 12);
        let top = self.content_top();
        let bottom = self.content_bottom();
        if bottom <= top {
            return None;
        }
        let raw_top = rect.y + top - self.scroll;
        let control_top = raw_top.clamp(top, bottom);
        let control_bottom = raw_top.saturating_add(rect.height).clamp(top, bottom);
        let above = (control_top - top).max(0).min(self.viewport_height());
        let below = (bottom - control_bottom).max(0).min(self.viewport_height());
        let wanted = (state.rows.len().min(12) as i32 * row_height) + 2;
        let upward = below < wanted && above > below;
        let space = if upward { above } else { below };
        let visible = ((space - 2).max(0) / row_height).min(12) as usize;
        let visible = visible.min(state.rows.len());
        if visible == 0 {
            return None;
        }
        let widest = state
            .rows
            .iter()
            .map(|row| match row {
                SelectRow::Group { label, .. } => label.chars().count(),
                SelectRow::Option(index) => {
                    state.options[*index].label.chars().count()
                        + usize::from(state.options[*index].group.is_some()) * 2
                }
            })
            .max()
            .unwrap_or(0)
            .min(256) as i32;
        let widest = rect
            .text_font
            .as_ref()
            .map_or(widest * GLYPH_ADVANCE * rect.scale, |font| {
                state
                    .rows
                    .iter()
                    .map(|row| match row {
                        SelectRow::Group { label, .. } => {
                            font.measure(&label.chars().take(256).collect::<String>())
                        }
                        SelectRow::Option(index) => {
                            let option = &state.options[*index];
                            font.measure(&option.label.chars().take(256).collect::<String>())
                                + if option.group.is_some() {
                                    font.measure("  ")
                                } else {
                                    0
                                }
                        }
                    })
                    .max()
                    .unwrap_or(0)
            });
        let width = (widest + 28).max(rect.width).min((self.width - 8).max(1));
        let height = visible as i32 * row_height + 2;
        let x = rect.x.clamp(0, (self.width - width).max(0));
        let y = if upward {
            control_top - height
        } else {
            control_bottom
        };
        Some(SelectPopupBox {
            x,
            y,
            width,
            height,
            row_height,
            visible,
            scale: rect.scale,
        })
    }

    fn select_row_option(&self, node: usize, row: usize) -> Option<usize> {
        let state = self.forms.controls.get(&node)?.select.as_ref()?;
        let SelectRow::Option(index) = *state.rows.get(row)? else {
            return None;
        };
        (!state.options[index].disabled).then_some(index)
    }

    fn select_row_at(&self, x: i32, y: i32) -> Option<usize> {
        let popup = self.select_popup?;
        let bounds = self.select_popup_box()?;
        if x <= bounds.x
            || x >= bounds.x + bounds.width - 1
            || y <= bounds.y
            || y >= bounds.y + bounds.height - 1
        {
            return None;
        }
        Some(popup.first + ((y - bounds.y - 1) / bounds.row_height) as usize)
    }

    fn open_select(&mut self, node: usize) {
        self.focus_control(node);
        let Some(state) = self
            .forms
            .controls
            .get(&node)
            .and_then(|control| control.select.as_ref())
        else {
            return;
        };
        let highlighted = state.selected.and_then(|selected| state.rows.iter()
            .position(|row| matches!(row, SelectRow::Option(index) if *index == selected && !state.options[*index].disabled)))
            .or_else(|| state.rows.iter().position(|row| matches!(row, SelectRow::Option(index) if !state.options[*index].disabled)));
        self.select_popup = Some(SelectPopup {
            node,
            highlighted,
            first: 0,
            wheel_remainder: 0,
        });
        self.drag = Drag::None;
        if self.select_popup_box().is_none() {
            self.select_popup = None;
        } else {
            self.keep_select_highlight_visible();
        }
    }

    fn keep_select_highlight_visible(&mut self) {
        let Some(bounds) = self.select_popup_box() else {
            return;
        };
        let Some(popup) = &mut self.select_popup else {
            return;
        };
        if let Some(row) = popup.highlighted {
            if row < popup.first {
                popup.first = row;
            }
            if row >= popup.first + bounds.visible {
                popup.first = row + 1 - bounds.visible;
            }
        }
        let count = self.forms.controls[&popup.node]
            .select
            .as_ref()
            .unwrap()
            .rows
            .len();
        popup.first = popup.first.min(count.saturating_sub(bounds.visible));
    }

    fn move_select_highlight(&mut self, forward: bool, edge: bool) {
        let Some(popup) = self.select_popup else {
            return;
        };
        let state = self.forms.controls[&popup.node].select.as_ref().unwrap();
        let rows = state.rows.len();
        let candidate = if forward {
            let start = if edge {
                0
            } else {
                popup.highlighted.map_or(0, |row| row + 1)
            };
            (start..rows).find(|&row| self.select_row_option(popup.node, row).is_some())
        } else {
            let end = if edge {
                rows
            } else {
                popup.highlighted.unwrap_or(rows)
            };
            (0..end)
                .rev()
                .find(|&row| self.select_row_option(popup.node, row).is_some())
        };
        if let Some(row) = candidate {
            self.select_popup.as_mut().unwrap().highlighted = Some(row);
            self.keep_select_highlight_visible();
        }
    }

    fn choose_select(&mut self, node: usize, index: usize) {
        self.select_popup = None;
        self.drag = Drag::None;
        if self.forms.select_option(node, index).unwrap_or(false) {
            self.script_begin_job();
            self.invalidate_script_geometry();
            self.script_event(node, "input", false);
            self.script_event(node, "change", false);
            self.sync_editors();
        }
    }

    fn step_select(&mut self, node: usize, forward: bool, edge: bool) {
        let Some(state) = self
            .forms
            .controls
            .get(&node)
            .and_then(|control| control.select.as_ref())
        else {
            return;
        };
        let candidate = if forward {
            let start = if edge {
                0
            } else {
                state.selected.map_or(0, |index| index + 1)
            };
            (start..state.options.len()).find(|&index| !state.options[index].disabled)
        } else {
            let end = if edge {
                state.options.len()
            } else {
                state.selected.unwrap_or(state.options.len())
            };
            (0..end).rev().find(|&index| !state.options[index].disabled)
        };
        if let Some(index) = candidate {
            self.choose_select(node, index);
        }
    }

    fn draw_select_popup(&self, canvas: &mut Canvas) {
        let (Some(popup), Some(bounds)) = (self.select_popup, self.select_popup_box()) else {
            return;
        };
        let state = self.forms.controls[&popup.node].select.as_ref().unwrap();
        let font = self
            .page
            .controls
            .iter()
            .find(|rect| rect.node == popup.node)
            .and_then(|rect| rect.text_font.as_ref());
        canvas.set_clip(Some(crate::paint::ClipRect::new(
            0,
            self.content_top(),
            self.width,
            self.viewport_height(),
        )));
        canvas.fill_rect(bounds.x, bounds.y, bounds.width, bounds.height, 0x526d80);
        canvas.fill_rect(
            bounds.x + 1,
            bounds.y + 1,
            bounds.width - 2,
            bounds.height - 2,
            0xffffff,
        );
        for (visible, row) in state
            .rows
            .iter()
            .enumerate()
            .skip(popup.first)
            .take(bounds.visible)
        {
            let y = bounds.y + 1 + (visible - popup.first) as i32 * bounds.row_height;
            let (label, disabled, grouped) = match row {
                SelectRow::Group { node, label } => {
                    canvas.fill_rect(
                        bounds.x + 1,
                        y,
                        bounds.width - 2,
                        bounds.row_height,
                        0xe7edf1,
                    );
                    (
                        label.as_str(),
                        self.document.attr(*node, "disabled").is_some(),
                        false,
                    )
                }
                SelectRow::Option(index) => {
                    let option = &state.options[*index];
                    if popup.highlighted == Some(visible) && !option.disabled {
                        canvas.fill_rect(
                            bounds.x + 1,
                            y,
                            bounds.width - 2,
                            bounds.row_height,
                            0xc4e2f5,
                        );
                    }
                    (
                        option.label.as_str(),
                        option.disabled,
                        option.group.is_some(),
                    )
                }
            };
            if let Some(font) = font {
                let indent = if grouped { font.measure("  ") } else { 0 };
                let label = font.fit(label, (bounds.width - 20 - indent).max(0));
                let shape = font.shape(&label);
                shape.draw(
                    canvas,
                    bounds.x + 7 + indent,
                    y + (bounds.row_height - shape.height()) / 2,
                    if disabled { 0x89929a } else { 0x26364a },
                );
                continue;
            }
            let indent = if grouped {
                2 * GLYPH_ADVANCE * bounds.scale
            } else {
                0
            };
            let cells =
                ((bounds.width - 20 - indent) / (GLYPH_ADVANCE * bounds.scale)).max(0) as usize;
            canvas.text(
                bounds.x + 7 + indent,
                y + (bounds.row_height - GLYPH_HEIGHT * bounds.scale) / 2,
                &fit_text(label, cells),
                bounds.scale,
                if disabled { 0x89929a } else { 0x26364a },
            );
        }
        if state.rows.len() > bounds.visible && bounds.width >= 12 {
            let track = bounds.height - 4;
            let thumb = (track * bounds.visible as i32 / state.rows.len() as i32).max(4);
            let offset = (track - thumb) * popup.first as i32
                / state.rows.len().saturating_sub(bounds.visible).max(1) as i32;
            canvas.fill_rect(
                bounds.x + bounds.width - 6,
                bounds.y + 2,
                4,
                track,
                0xe7edf1,
            );
            canvas.fill_rect(
                bounds.x + bounds.width - 6,
                bounds.y + 2 + offset,
                4,
                thumb,
                0x849aaa,
            );
        }
        canvas.set_clip(None);
    }
    fn script_begin_job(&mut self) {
        self.inspector.invalidate();
        if let Some(page) = &mut self.scripting {
            page.begin_job();
        }
    }
    fn update_script_geometry(&mut self) {
        let height = self.viewport_height();
        if let Some(page) = &mut self.scripting {
            page.update_geometry(
                self.width - 12,
                height,
                self.zoom,
                inline_svg::natural_sizes(&self.images,&self.image_densities,&self.inline_svg_sources),
            );
        }
    }
    fn invalidate_script_geometry(&mut self) {
        if let Some(page) = &mut self.scripting {
            page.invalidate_geometry();
        }
    }
    fn script_event(&mut self, target: usize, event: &str, cancelable: bool) -> bool {
        self.inspector.invalidate();
        let Some(page) = &mut self.scripting else {
            return false;
        };
        let outcome = page.dispatch(
            &mut self.document,
            &mut self.forms,
            target,
            event,
            cancelable,
        );
        self.apply_history_replacement();
        if outcome.layout_changed {
            self.reflow();
        }
        self.apply_script_scroll();
        self.apply_script_focus();
        self.validate_control_focus();
        self.sync_editors();
        if outcome.errors > 0 {
            self.status = format!(
                "JavaScript subset: {} error(s). Ctrl+I shows details.",
                outcome.errors
            );
            self.failed = false;
        }
        outcome.prevented
    }
    fn edited_control(&mut self, previous: Option<(usize, String)>) {
        if let Some((node, previous)) = previous {
            if self
                .forms
                .controls
                .get(&node)
                .is_some_and(|c| c.value.text() != previous)
            {
                if let Some(control) = self.forms.controls.get_mut(&node) {
                    control.mark_value_dirty();
                }
                self.script_begin_job();
                self.invalidate_script_geometry();
                self.script_event(node, "input", false);
            }
        }
    }
    fn current_control_value(&self) -> Option<(usize, String)> {
        if let Focus::Control(node) = self.focus {
            self.forms
                .controls
                .get(&node)
                .filter(|c| c.editable())
                .map(|c| (node, c.value.text()))
        } else {
            None
        }
    }
    fn focus_address(&mut self) {
        self.invalidate_platform_requests();
        self.reset_chrome();
        self.select_popup = None;
        self.focus = Focus::Address;
        self.address.set(
            &self
                .pending
                .as_ref()
                .map(|pending| pending.location.display())
                .unwrap_or_else(|| self.location.display()),
        );
        self.address.select_all();
        self.menu = false;
        self.pending_surrogate = None;
        self.sync_editors();
    }
    fn open_find(&mut self) {
        self.invalidate_platform_requests();
        self.reset_chrome();
        self.select_popup = None;
        let changed = !self.find_open;
        self.find_open = true;
        if changed {
            self.reflow();
        }
        self.focus = Focus::Find;
        self.find.select_all();
        self.menu = false;
        self.set_scroll(self.scroll.clamp(0, self.max_scroll()));
        self.update_find(false);
        self.sync_editors();
    }
    pub fn search(&mut self, query: &str) {
        self.user_changed_loading_position();
        self.open_find();
        self.find.set(query);
        self.update_find(true);
        self.sync_editors();
    }
    fn key_default(&mut self, key: u32, ctrl: bool, shift: bool, alt: bool) -> PlatformAction {
        self.invalidate_platform_requests();
        // AltGr belongs to text input. Page dispatch has already observed the
        // key; it must not dismiss a browser menu or steal an inspector editor.
        if ctrl && alt {
            return PlatformAction::None;
        }
        if let Some(action) = self.downloads_key(key, ctrl, shift, alt) {
            return action;
        }
        if self.bookmark_bar_key(key, ctrl, shift, alt) {
            return PlatformAction::None;
        }
        if !alt && (key == 0x7b || ctrl && shift && key == 0x49) {
            self.toggle_inspector();
            return PlatformAction::None;
        }
        if self.inspector.focused && self.inspector.mode != inspector::Mode::Closed {
            if alt
                || ctrl
                    && matches!(
                        key,
                        0x4c | 0x52 | 0x74 | 0x4f | 0xbc | 0x48 | 0x44 | 0x4a | 0x42 | 0x53
                    )
            {
                self.inspector.focused = false;
            } else {
                return self.inspector_key(key, ctrl, shift, alt);
            }
        }
        if let Some(action) = self.chrome_key(key, ctrl, shift, alt) {
            return action;
        }
        if self.browser_panel.is_some() {
            return self.browser_key(key, ctrl, shift, alt);
        }
        self.user_changed_loading_position();
        if let Some(popup) = self.select_popup {
            if !ctrl && !alt {
                match key {
                    0x1b => {
                        self.select_popup = None;
                        self.drag = Drag::None;
                        return PlatformAction::None;
                    }
                    0x26 | 0x28 | 0x24 | 0x23 => {
                        self.move_select_highlight(
                            matches!(key, 0x28 | 0x24),
                            matches!(key, 0x24 | 0x23),
                        );
                        return PlatformAction::None;
                    }
                    0x0d | 0x20 => {
                        if let Some(index) = popup
                            .highlighted
                            .and_then(|row| self.select_row_option(popup.node, row))
                        {
                            self.choose_select(popup.node, index);
                        } else {
                            self.select_popup = None;
                        }
                        return PlatformAction::None;
                    }
                    _ => {}
                }
            }
            self.select_popup = None;
            self.drag = Drag::None;
        }
        if !ctrl && !alt && self.media_key(key) {return PlatformAction::None;}
        if alt {
            match key {
                0x25 => self.history_move(-1),
                0x27 => self.history_move(1),
                0x24 => self.home(),
                0x44 => self.focus_address(),
                _ => {}
            }
            return PlatformAction::None;
        }
        if ctrl {
            match key {
                0x44 => self.add_current_bookmark(),
                0x42 => self.show_bookmarks(),
                0x48 => self.show_history(),
                0xbc => self.show_settings(),
                0x4c => self.focus_address(),
                0x4f => return PlatformAction::OpenFile,
                0x53 => self.download_current(),
                0x52 if !shift => self.reload(),
                0x52 | 0x74 => self.reload_with_mode(crate::http::FetchMode::FullReload),
                0x46 => self.open_find(),
                0x55 => self.show_source(),
                0x49 => self.show_resources(),
                0x4a => self.toggle_javascript(),
                0xbb | 0x6b => self.set_zoom(self.zoom + 0.25),
                0xbd | 0x6d => self.set_zoom(self.zoom - 0.25),
                0x30 | 0x60 => {
                    self.set_zoom(self.profile.data().settings.default_zoom as f32 / 100.0)
                }
                0x41 => {
                    if self.has_editor() {
                        self.editor().select_all();
                    } else if let Some(last) = self.page.runs.last() {
                        self.selection = Some(Selection {
                            start: TextPoint { run: 0, offset: 0 },
                            end: TextPoint {
                                run: self.page.runs.len() - 1,
                                offset: last.text.chars().count(),
                            },
                        });
                    }
                }
                0x43 | 0x58 => {
                    let text = if self.has_editor() {
                        self.editor().selected()
                    } else {
                        self.selected_text()
                    };
                    self.sync_editors();
                    return if text.is_empty() {
                        PlatformAction::None
                    } else if key == 0x58 && self.editor_editable() {
                        PlatformAction::Cut(text)
                    } else {
                        PlatformAction::Copy(text)
                    };
                }
                0x56 if self.editor_editable() => return PlatformAction::Paste,
                0x24 => {
                    if !self.has_editor() {
                        self.set_scroll(0);
                    } else {
                        self.editor().move_to(0, shift);
                    }
                }
                0x23 => {
                    if !self.has_editor() {
                        self.set_scroll(self.max_scroll());
                    } else {
                        let n = self.editor().chars.len();
                        self.editor().move_to(n, shift);
                    }
                }
                _ => {}
            }
            self.sync_editors();
            return PlatformAction::None;
        }
        match key {
            0x74 => {
                self.reload();
                return PlatformAction::None;
            }
            0x75 => {
                self.focus_address();
                return PlatformAction::None;
            }
            0x72 => {
                if !self.find_open {
                    self.open_find();
                }
                self.find_next(shift);
                return PlatformAction::None;
            }
            0x1b => {
                if self.is_loading() {
                    self.stop_loading();
                    return PlatformAction::None;
                }
                if self.menu {
                    self.menu = false;
                } else if self.focus == Focus::Address {
                    self.focus = Focus::Page;
                    self.address.set(&self.location.display());
                } else if matches!(self.focus, Focus::Control(_)) {
                    self.focus = Focus::Page;
                } else if self.find_open {
                    self.find_open = false;
                    self.reflow();
                    self.focus = Focus::Page;
                    self.matches.clear();
                } else if self.view != View::Page {
                    self.toggle_view(self.view);
                }
                self.set_scroll(self.scroll.clamp(0, self.max_scroll()));
                return PlatformAction::None;
            }
            _ => {}
        }
        if key == 0x09 {
            self.tab_focus(shift);
            return PlatformAction::None;
        }
        if let Focus::Control(node) = self.focus {
            let kind = self.forms.controls[&node].kind;
            if kind == Kind::Select && matches!(key, 0x26 | 0x28 | 0x24 | 0x23) {
                self.step_select(node, matches!(key, 0x28 | 0x24), matches!(key, 0x24 | 0x23));
                return PlatformAction::None;
            }
            if key == 0x0d && kind == Kind::Text {
                match self.forms.implicit_submitter(node) {
                    Ok((_, Some(submitter))) => self.activate_control(submitter),
                    Ok((owner, None)) => self.submit_form(owner, None),
                    Err(error) => self.error(error),
                }
                return PlatformAction::None;
            }
            if kind == Kind::TextArea && self.textarea_key(node, key, shift) {
                return PlatformAction::None;
            }
            if !matches!(kind, Kind::Text | Kind::TextArea) {
                if matches!(key, 0x0d | 0x20) {
                    self.activate_control(node);
                }
                return PlatformAction::None;
            }
        }
        if self.has_editor() {
            let previous = self.current_control_value();
            match key {
                0x0d => {
                    if self.focus == Focus::Find {
                        self.find_next(shift);
                    } else {
                        let address = self.address.text();
                        self.submit_address_bar(&address);
                    }
                }
                0x08 if self.editor_editable() => self.editor().backspace(),
                0x2e if self.editor_editable() => self.editor().delete(),
                0x25 => self.editor().left(shift),
                0x27 => self.editor().right(shift),
                0x24 => self.editor().move_to(0, shift),
                0x23 => {
                    let n = self.editor().chars.len();
                    self.editor().move_to(n, shift);
                }
                _ => {}
            }
            self.edited_control(previous);
            if self.focus == Focus::Find && matches!(key, 0x08 | 0x2e) {
                self.update_find(true);
            }
        } else {
            match key {
                0x26 => self.scroll_by(-24),
                0x28 => self.scroll_by(24),
                0x21 => self.scroll_by(-self.viewport_height() + 24),
                0x22 => self.scroll_by(self.viewport_height() - 24),
                0x20 => self.scroll_by((self.viewport_height() - 24) * if shift { -1 } else { 1 }),
                0x24 => self.set_scroll(0),
                0x23 => self.set_scroll(self.max_scroll()),
                _ => {}
            }
        }
        self.sync_editors();
        PlatformAction::None
    }
    pub fn text_input(&mut self, ch: char) {
        self.invalidate_platform_requests();
        if self.downloads_open() || self.bookmark_popup_open() {
            return;
        }
        if self.inspector.focused && self.inspector.mode != inspector::Mode::Closed {
            if !ch.is_control() {
                let mut buffer = [0u8; 4];
                self.inspector_paste(ch.encode_utf8(&mut buffer));
            }
            return;
        }
        if self.menu || self.chrome.focus.is_some() {
            return;
        }
        if self.browser_panel.is_some() {
            if !ch.is_control() {
                let mut buffer = [0u8; 4];
                self.browser_paste(ch.encode_utf8(&mut buffer));
            }
            return;
        }
        if !self.editor_editable() || ch.is_control() {
            return;
        }
        let mut buffer = [0u8; 4];
        self.paste(ch.encode_utf8(&mut buffer));
    }
    pub fn utf16_input(&mut self, unit: u16) {
        self.invalidate_platform_requests();
        if (0xd800..=0xdbff).contains(&unit) {
            self.pending_surrogate = Some(unit);
            return;
        }
        let code = if (0xdc00..=0xdfff).contains(&unit) {
            match self.pending_surrogate.take() {
                Some(high) => 0x10000 + (((high as u32 - 0xd800) << 10) | (unit as u32 - 0xdc00)),
                None => 0xfffd,
            }
        } else {
            self.pending_surrogate = None;
            unit as u32
        };
        if let Some(ch) = char::from_u32(code) {
            self.text_input(ch);
        }
    }
    pub fn paste(&mut self, text: &str) {
        self.invalidate_platform_requests();
        if self.downloads_open() || self.bookmark_popup_open() {
            return;
        }
        if self.inspector.focused && self.inspector.mode != inspector::Mode::Closed {
            self.inspector_paste(text);
            return;
        }
        if self.menu || self.chrome.focus.is_some() {
            return;
        }
        if self.browser_panel.is_some() {
            self.browser_paste(text);
            return;
        }
        if !self.editor_editable() {
            return;
        }
        if let Focus::Control(node) = self.focus {
            let previous = self.current_control_value();
            self.forms.controls.get_mut(&node).unwrap().insert(text);
            self.edited_control(previous);
            self.sync_editors();
            return;
        }
        let limit = if self.focus == Focus::Find {
            256
        } else {
            32_768
        };
        self.editor().insert(text, limit);
        if self.focus == Focus::Find {
            self.update_find(true);
        }
        self.sync_editors();
    }
    pub fn cut_copied(&mut self, text: &str) {
        self.invalidate_platform_requests();
        if self.inspector.focused && self.inspector.mode != inspector::Mode::Closed {
            self.inspector_cut_copied(text);
            return;
        }
        if self.browser_panel.is_some() {
            self.browser_cut_copied(text);
            return;
        }
        // The platform acknowledges only a successful clipboard write. If a
        // reentrant native message changed the selection, leave the editor alone.
        if self.editor_editable() && self.editor().selected() == text {
            let previous = self.current_control_value();
            self.editor().delete_selection();
            self.edited_control(previous);
            if self.focus == Focus::Find {
                self.update_find(true);
            }
            self.sync_editors();
        }
    }
    fn update_find(&mut self, reveal: bool) {
        self.matches.clear();
        self.match_index = 0;
        let needle = self.find.text().to_ascii_lowercase();
        if !self.find_open || needle.is_empty() {
            return;
        }
        let mut text = String::new();
        let mut spans = Vec::new();
        for (i, run) in self.page.runs.iter().enumerate() {
            if i > 0 && !run.separator_before.is_empty() {
                text.push(' ');
            }
            let start = text.len();
            text.push_str(&run.text);
            spans.push((start, text.len(), i));
            if text.len() > 2 * 1024 * 1024 {
                break;
            }
        }
        let lower = text.to_ascii_lowercase();
        for (start, value) in lower.match_indices(&needle).take(2000) {
            let end = start + value.len();
            let a = spans.partition_point(|(_, b, _)| *b <= start);
            let b = spans
                .partition_point(|(a, _, _)| *a < end)
                .saturating_sub(1);
            if a >= spans.len() || b < a {
                continue;
            }
            let (as_, ae, arun) = spans[a];
            let (bs, be, brun) = spans[b];
            let aoffset = text[as_..start.clamp(as_, ae)].chars().count();
            let boffset = text[bs..end.clamp(bs, be)].chars().count();
            self.matches.push(Selection {
                start: TextPoint {
                    run: arun,
                    offset: aoffset,
                },
                end: TextPoint {
                    run: brun,
                    offset: boffset,
                },
            });
        }
        if reveal {
            self.reveal_match();
        }
    }
    fn reveal_match(&mut self) {
        if let Some(found) = self.matches.get(self.match_index) {
            if self.page.run_is_fixed(found.start.run) {
                return;
            }
            let run = &self.page.runs[found.start.run];
            self.set_scroll((run.y - self.viewport_height() / 3).clamp(0, self.max_scroll()));
        }
    }
    fn find_next(&mut self, previous: bool) {
        if self.matches.is_empty() {
            self.update_find(true);
            return;
        }
        self.match_index = if previous {
            (self.match_index + self.matches.len() - 1) % self.matches.len()
        } else {
            (self.match_index + 1) % self.matches.len()
        };
        self.reveal_match();
    }
    fn selected_text(&self) -> String {
        let Some(selection) = self.selection else {
            return String::new();
        };
        let mut out = String::new();
        let mut previous: Option<usize> = None;
        for (i, run) in self.page.runs.iter().enumerate() {
            if let Some((a, b)) = selection.slice(i, run.text.chars().count()) {
                if previous.is_some() {
                    out.push_str(run.separator_before);
                }
                out.extend(run.text.chars().skip(a).take(b - a));
                previous = Some(i);
            }
        }
        out
    }
    fn point_at(&self, x: i32, y: i32) -> Option<TextPoint> {
        let y = y - self.content_top() + self.scroll;
        // Runs remain in source order for copy/find. Pick overlapping text in
        // paint order, so an overlay cannot select the hidden text below it.
        let mut exact = None;
        for (item_index, item) in self.page.items.iter().enumerate().rev() {
            if self
                .page
                .paint_clip(item_index)
                .is_some_and(|clip| !clip.contains(x, y))
            {
                continue;
            }
            let inside = |left: i32, top: i32, width: i32, height: i32| {
                x >= left
                    && x < left.saturating_add(width)
                    && y >= top
                    && y < top.saturating_add(height)
            };
            match *item {
                PaintItem::Text(index) => {
                    let run = &self.page.runs[index];
                    if inside(run.x, run.y, run.width(), run.height()) {
                        exact = Some(index);
                        break;
                    }
                }
                PaintItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    ..
                } | PaintItem::RoundedRect { x, y, width, height, .. } if inside(x, y, width, height) => {
                    if self.page.paint_is_positioned(item_index) {
                        return None;
                    }
                    break;
                }
                PaintItem::Image {
                    x,
                    y,
                    width,
                    height,
                    ..
                } if inside(x, y, width, height) => return None,
                PaintItem::Control(index) => {
                    let control = &self.page.controls[index];
                    if inside(control.x, control.y, control.width, control.height) {
                        return None;
                    }
                }
                _ => {}
            }
        }
        let nearest = || {
            self.page
                .runs
                .iter()
                .enumerate()
                .filter(|(index, run)| {
                    self.page.run_clip(*index).is_none_or(|clip| {
                        let area =
                            crate::paint::ClipRect::new(run.x, run.y, run.width(), run.height())
                                .intersect(clip);
                        area.left < area.right && area.top < area.bottom
                    })
                })
                .min_by_key(|(_, run)| {
                    let dy = if y < run.y {
                        run.y - y
                    } else {
                        (y - run.y - run.height()).max(0)
                    };
                    let dx = if x < run.x {
                        run.x - x
                    } else {
                        (x - run.x - run.width()).max(0)
                    };
                    i64::from(dy) * 1_000_000 + i64::from(dx)
                })
        };
        let (i, run) = exact
            .map(|index| (index, &self.page.runs[index]))
            .or_else(nearest)?;
        let offset = run.character_at(x - run.x);
        Some(TextPoint {
            run: i,
            offset: offset.min(run.text.chars().count()),
        })
    }
    fn scrollbar(&self) -> (i32, i32, i32) {
        let track = self.viewport_height();
        let thumb = ((i64::from(track) * i64::from(track) / i64::from(self.page.height.max(1)))
            as i32)
            .clamp(24.min(track), track);
        let y = self.content_top()
            + ((i64::from(track - thumb) * i64::from(self.scroll))
                / i64::from(self.max_scroll().max(1))) as i32;
        (y, thumb, track)
    }
    fn pointer_down_impl(&mut self, x: i32, y: i32, shift: bool) -> PlatformAction {
        self.invalidate_platform_requests();
        if self.downloads_pointer_down(x, y) {
            return PlatformAction::None;
        }
        if !self.menu && self.bookmark_bar_pointer_down(x, y) {
            return PlatformAction::None;
        }
        if !self.menu && self.inspector_main_pointer_down(x, y) {
            return PlatformAction::None;
        }
        if self.browser_panel.is_some() && self.browser_pointer_down(x, y, shift) {
            return PlatformAction::None;
        }
        self.user_changed_loading_position();
        self.pending_surrogate = None;
        if let Some(popup) = self.select_popup {
            if let Some(row) = self.select_row_at(x, y) {
                if let Some(index) = self.select_row_option(popup.node, row) {
                    self.select_popup.as_mut().unwrap().highlighted = Some(row);
                    self.drag = Drag::SelectOption {
                        node: popup.node,
                        index,
                    };
                } else {
                    self.drag = Drag::None;
                }
            } else {
                self.select_popup = None;
                self.drag = Drag::None;
            }
            return PlatformAction::None;
        }
        if self.chrome_pointer_down(x, y, shift) {
            return PlatformAction::None;
        }
        if self.media_pointer_down(x,y) {return PlatformAction::None;}
        if y < self.content_top() || y >= self.content_bottom() {
            return PlatformAction::None;
        }
        if x >= self.width - 12 && self.max_scroll() > 0 {
            self.focus = Focus::Page;
            let (top, thumb, _) = self.scrollbar();
            if y < top {
                self.scroll_by(-self.viewport_height());
            } else if y >= top + thumb {
                self.scroll_by(self.viewport_height());
            } else {
                self.drag = Drag::Scrollbar { offset: y - top };
            }
            return PlatformAction::None;
        }
        if self.view == View::Page {
            if let Some(page) = self.scripting.as_mut() {
                page.notify_user_activation();
            }
            if let Some(node) = self
                .page
                .control_at(x, y - self.content_top() + self.scroll)
            {
                self.selection = None;
                if !self.page.controls.iter().any(|rect| rect.node == node) {
                    return PlatformAction::None;
                }
                if let Some(control) = self.forms.controls.get(&node) {
                    if control.disabled {
                        self.drag = Drag::None;
                        return PlatformAction::None;
                    }
                    let kind = control.kind;
                    // Pointer focus must not move the target between press and
                    // release. Keyboard focus still brings controls into view.
                    let previous=if let Focus::Control(previous)=self.focus {Some(previous)}else{None};
                    self.focus_control_with_scroll(node, false);
                    if matches!(kind, Kind::Text | Kind::TextArea) {
                        if let Some(rect) = self.page.controls.iter().find(|c| c.node == node) {
                            let pos = if kind == Kind::TextArea {
                                textarea::character_at(
                                    rect,
                                    &self.forms.controls[&node].value,
                                    x,
                                    y - self.content_top() + self.scroll,
                                )
                            } else {
                                control_character_at(rect, &self.forms.controls[&node].value, x)
                            };
                            self.forms
                                .controls
                                .get_mut(&node)
                                .unwrap()
                                .value
                                .move_to(pos, shift);
                        }
                        self.drag = Drag::Field;
                        self.sync_editors();
                    } else {
                        self.drag = Drag::Control { node };
                    }
                    self.script_begin_job();
                    self.update_script_geometry();
                    self.script_focus_transition(previous,Some(node));
                }
                return PlatformAction::None;
            }
        }
        self.focus = Focus::Page;
        if let Some(point) = self.point_at(x, y) {
            let start = if shift {
                self.selection.map(|s| s.start).unwrap_or(point)
            } else {
                point
            };
            self.selection = Some(Selection { start, end: point });
        }
        let href = self
            .page
            .link_at(x, y - self.content_top() + self.scroll)
            .map(str::to_owned);
        if !shift {
            if let Some(href) = &href {
                self.arm_page_bookmark_drag(href, x, y);
            }
        }
        self.drag = Drag::Text {
            x,
            y,
            href,
            moved: shift,
        };
        PlatformAction::None
    }
    fn pointer_move_impl(&mut self, x: i32, y: i32, dragging: bool) -> bool {
        if dragging { self.invalidate_platform_requests(); }
        if self.downloads_open() {
            return self.downloads_pointer_move(x, y);
        }
        let bookmark_changed = self.bookmark_bar_pointer_move(x, y, dragging);
        if self.bookmark_drag_active()
            || self.bookmark_popup_open()
            || (BAR_HEIGHT..self.chrome_bottom()).contains(&y)
        {
            return bookmark_changed;
        }
        if self.inspector.mode == inspector::Mode::Docked
            && self.inspector.focused
            && self.inspector_pointer_move(x, y, dragging)
        {
            return true;
        }
        let hover = self.chrome_hit(x, y);
        let chrome_changed = self.chrome.hover != hover;
        self.chrome.hover = hover;
        if self.menu || self.chrome.pressed.is_some() {
            self.hover = None;
            if let Some(chrome::ChromeAction::MenuItem(index)) = hover {
                self.chrome.menu_selected = Some(index);
            }
            return chrome_changed;
        }
        if self.browser_panel.is_some() {
            return self.browser_pointer_move(x, y, dragging) || chrome_changed;
        }
        if let Some(popup) = self.select_popup {
            if let Some(row) = self.select_row_at(x, y) {
                if self.select_row_option(popup.node, row).is_some()
                    && popup.highlighted != Some(row)
                {
                    self.select_popup.as_mut().unwrap().highlighted = Some(row);
                    return true;
                }
            }
            return chrome_changed;
        }
        let hover = if y >= self.content_top() && y < self.content_bottom() {
            self.page
                .link_at(x, y - self.content_top() + self.scroll)
                .map(str::to_owned)
        } else {
            None
        };
        let changed = self.hover != hover || chrome_changed;
        self.hover = hover;
        if !dragging {
            return changed;
        }
        match &mut self.drag {
            Drag::Text {
                x: start_x,
                y: start_y,
                moved,
                ..
            } => {
                *moved |= (x - *start_x).abs() > 4 || (y - *start_y).abs() > 4;
                if !*moved {
                    return changed;
                }
                if y < self.content_top() {
                    self.scroll_by(-24);
                } else if y > self.content_bottom() {
                    self.scroll_by(24);
                }
                if let Some(point) = self.point_at(x, y) {
                    if let Some(selection) = &mut self.selection {
                        selection.end = point;
                    }
                }
            }
            Drag::Scrollbar { offset } => {
                let offset = *offset;
                let (_, thumb, track) = self.scrollbar();
                let position = (y - offset - self.content_top()).clamp(0, (track - thumb).max(0));
                self.set_scroll(
                    ((i64::from(position) * i64::from(self.max_scroll()))
                        / i64::from((track - thumb).max(1))) as i32,
                );
            }
            Drag::Field => {
                if let Focus::Control(node) = self.focus {
                    if let Some(rect) = self.page.controls.iter().find(|rect| rect.node == node) {
                        let control = &self.forms.controls[&node];
                        let pos = if control.kind == Kind::TextArea {
                            textarea::character_at(
                                rect,
                                &control.value,
                                x,
                                y - self.content_top() + self.scroll,
                            )
                        } else {
                            control_character_at(rect, &control.value, x)
                        };
                        self.editor().move_to(pos, true);
                        self.sync_editors();
                        return true;
                    }
                }
                let (left, padding, advance) = match self.focus {
                    Focus::Find => (72, 8, 12),
                    Focus::Control(node) => self
                        .page
                        .controls
                        .iter()
                        .find(|c| c.node == node)
                        .map(|c| (c.x, 6, GLYPH_ADVANCE * c.scale))
                        .unwrap_or((0, 6, 12)),
                    _ => (self.address_rect().0, 8, 12),
                };
                let pos = self.editor().offset + ((x - left - padding).max(0) / advance) as usize;
                self.editor().move_to(pos, true);
                self.sync_editors();
            }
            Drag::None | Drag::Control { .. } | Drag::SelectOption { .. } => {}
        }
        changed || !matches!(self.drag, Drag::None)
    }
    fn pointer_up_impl(&mut self, x: i32, y: i32) -> PlatformAction {
        self.pointer_up_mode(x, y, true)
    }
    /// Native link activation owns its guarded author/default path separately.
    /// Keep every other existing release behavior, without a hidden link fallback.
    pub(crate) fn pointer_up_without_link_default(&mut self, x: i32, y: i32) -> PlatformAction {
        self.pointer_up_mode(x, y, false)
    }
    fn pointer_up_mode(&mut self, x: i32, y: i32, allow_link_default: bool) -> PlatformAction {
        self.invalidate_platform_requests();
        if let Some(action) = self.downloads_pointer_up(x, y) {
            return action;
        }
        if self.bookmark_bar_pointer_up(x, y) {
            return PlatformAction::None;
        }
        if self.inspector.mode == inspector::Mode::Docked && self.inspector.focused && !self.menu {
            return self.inspector_pointer_up(x, y);
        }
        if let Some(action) = self.chrome_pointer_up(x, y) {
            return action;
        }
        if self.browser_panel.is_some() {
            return self.browser_pointer_up(x, y);
        }
        if self.media_pointer_up(x,y) {return PlatformAction::None;}
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        if let Drag::SelectOption { node, index } = drag {
            if self.select_popup.is_some_and(|popup| popup.node == node)
                && self
                    .select_row_at(x, y)
                    .and_then(|row| self.select_row_option(node, row))
                    == Some(index)
            {
                self.choose_select(node, index);
            }
            return PlatformAction::None;
        }
        if let Drag::Field = drag {
            if let Focus::Control(node) = self.focus {
                if self.view == View::Page
                    && y >= self.content_top()
                    && y < self.content_bottom()
                    && self
                        .page
                        .control_at(x, y - self.content_top() + self.scroll)
                        == Some(node)
                {
                    self.activate_control(node);
                }
            }
            return PlatformAction::None;
        }
        if let Drag::Control { node } = drag {
            if self.view == View::Page
                && y >= self.content_top()
                && y < self.content_bottom()
                && self
                    .page
                    .control_at(x, y - self.content_top() + self.scroll)
                    == Some(node)
            {
                let point = self.image_point_at(node, x, y);
                self.activate_control_at(node, point);
            }
            return PlatformAction::None;
        }
        if allow_link_default {
        if let Drag::Text {
            href: Some(href),
            moved: false,
            x: start_x,
            y: start_y,
        } = drag
        {
            if (x - start_x).abs() <= 4
                && (y - start_y).abs() <= 4
                && y >= self.content_top()
                && y < self.content_bottom()
            {
                let page_y = y - self.content_top() + self.scroll;
                let matched = self.page.link_at(x, page_y) == Some(href.as_str());
                let node = self.page.link_node_at(x, page_y);
                self.observe_link_hit(x, y, page_y, node, matched);
                if !matched { return PlatformAction::None; }
                let download = self
                    .page
                    .link_node_at(x, y - self.content_top() + self.scroll)
                    .and_then(|node| self.document.attr(node, "download"))
                    .map(str::to_owned);
                if let Some(name) = download {
                    self.download_link(&href, (!name.is_empty()).then_some(name));
                } else {
                    self.follow(&href);
                }
            }
        }
        }
        PlatformAction::None
    }
    pub fn pointer_cancel(&mut self) {
        self.clear_native_hover();
        self.media_pointer_cancel();
        self.downloads_pointer_cancel();
        self.cancel_bookmark_pointer();
        self.inspector.cancel_pointer();
        self.drag = Drag::None;
        self.chrome.hover = None;
        self.chrome.pressed = None;
        self.hover = None;
        if let Some(panel) = &mut self.browser_panel {
            panel.cancel_pointer();
        }
    }
    pub fn wants_hand(&self, x: i32, y: i32) -> bool {
        if self.downloads_open() {
            return self.downloads_wants_hand(x, y);
        }
        if self.bookmark_bar_wants_hand(x, y) {
            return true;
        }
        if let Some(action) = self.chrome_hit(x, y) {
            return self.chrome_enabled(action);
        }
        if self.menu || y < self.content_top() {
            return false;
        }
        if self.inspector.mode == inspector::Mode::Docked && y >= self.content_bottom() {
            return self.inspector_wants_hand(x, y);
        }
        if self.browser_panel.is_some() {
            return self.browser_wants_hand(x, y);
        }
        if let Some(popup) = self.select_popup {
            return self
                .select_row_at(x, y)
                .and_then(|row| self.select_row_option(popup.node, row))
                .is_some();
        }
        if self.media_hit(x,y).is_some(){return true;}
        if self.view == View::Page && y >= self.content_top() && y < self.content_bottom() {
            if let Some(node) = self
                .page
                .control_at(x, y - self.content_top() + self.scroll)
            {
                return self.forms.controls.get(&node).is_some_and(|c| {
                    c.focusable() && !matches!(c.kind, Kind::Text | Kind::TextArea)
                });
            }
        }
        y >= self.content_top()
            && y < self.content_bottom()
            && self
                .page
                .link_at(x, y - self.content_top() + self.scroll)
                .is_some()
    }
}

#[path = "app_opacity.rs"]
mod opacity_paint;

impl App {
    pub fn draw(&self) -> Canvas {
        let mut canvas = Canvas::new(self.width as usize, self.height as usize);
        let top = self.content_top();
        let bottom = self.content_bottom();
        let viewport = crate::paint::ClipRect::new(0, top, self.width, bottom - top);
        // Refuse document paint atomically before any authored pixels, while
        // preserving the independently allocated browser/inspector surface.
        let preflight = if self.page.opacity_unavailable { Err(()) }
        else if self.page.opacity_groups.is_empty() { Ok(None) }
        else {
            self.opacity_paint_bounds(viewport).ok_or(()).and_then(|(regions, work)| {
                crate::paint::OpacityPainter::new(
                    &canvas, &self.page.opacity_groups, &regions, viewport, self.page.items.len(), work,
                ).map(Some).ok_or(())
            })
        };
        let paint_refused = preflight.is_err();
        self.page.opacity_draw_refused.set(paint_refused);
        let mut opacity = preflight.unwrap_or(None);
        canvas.fill_rect(
            0,
            0,
            self.width,
            self.height,
            self.page_background_color(0xffffff),
        );
        for (item_index, item) in self.page.items.iter().enumerate() {
            if paint_refused { break; }
            if let Some(painter) = &mut opacity {
                painter.advance(&mut canvas, item_index);
                if painter.hidden() { continue; }
            }
            let clip = self.page.paint_clip(item_index).map_or(viewport, |clip| {
                viewport.intersect(clip.translate_y(top - self.scroll))
            });
            canvas.set_clip(Some(clip));
            match *item {
                PaintItem::HitBox { .. } => {}
                PaintItem::Control(index) => {
                    if self.view != View::Page {
                        continue;
                    }
                    let rect = &self.page.controls[index];
                    let y = rect.y + top - self.scroll;
                    if y >= bottom || y.saturating_add(rect.height) <= top {
                        continue;
                    }
                    if let Some(control) = self.forms.controls.get(&rect.node) {
                        draw_control(
                            &mut canvas,
                            rect,
                            y,
                            control,
                            self.document.attr(rect.node, "placeholder").unwrap_or(""),
                            self.focus == Focus::Control(rect.node),
                            self.images.get(&rect.node).map(|bitmap| bitmap.as_ref()),
                        );
                    }
                }
                PaintItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    color,
                    alpha, .. } => {
                    let y = y + top - self.scroll;
                    if y < bottom && y.saturating_add(height) > top {
                        canvas.fill_rect_alpha(x, y, width, height, self.page_background_color(color), alpha);
                    }
                }
                PaintItem::RoundedRect { x, y, width, height, color, alpha, radius, border, background_insets,.. } => {
                    let y = y + top - self.scroll;
                    if y < bottom && y.saturating_add(height) > top {
                        canvas.fill_color_box(x, y, width, height, radius,
                            self.page_background_color(color), alpha, background_insets,
                            border.map(|border| crate::paint::UniformBorder {
                                    color: self.page_background_color(border.color), ..border
                            }));
                    }
                }
                PaintItem::Background { node, x, y, width, height, border, presentation, .. } => {
                    if self.view == View::Page {
                        if let Some(bitmap) = self.backgrounds.get(&node) {
                            let rect = crate::paint::ClipRect::new(x, y + top - self.scroll, width, height);
                            let origin = crate::paint::ClipRect::new(rect.left.saturating_add(border[3]),
                                rect.top.saturating_add(border[0]), width.saturating_sub(border[1]).saturating_sub(border[3]),
                                height.saturating_sub(border[0]).saturating_sub(border[2]));
                            canvas.background_image(rect, origin, bitmap, presentation);
                        }
                    }
                }
                PaintItem::Image {
                    node,
                    x,
                    y,
                    width,
                    height,
                    presentation,
                } => {
                    let y = y + top - self.scroll;
                    if y >= bottom || y.saturating_add(height) <= top {
                        continue;
                    }
                    if crate::html_canvas::element(&self.document, node) {
                        if self.view == View::Page {
                            if let Some(bitmap) = self.scripting.as_ref().and_then(|page| page.canvas_bitmap(node)) {
                                canvas.fitted_image(crate::paint::ClipRect::new(x, y, width, height), bitmap, presentation);
                            }
                        }
                        continue;
                    }
                    if self.view == View::Page {
                        let video=self.document.nodes.get(node).is_some_and(|n|n.namespace==crate::html::Namespace::Html&&matches!(&n.kind,crate::html::NodeKind::Element {tag,..} if tag=="video"));
                        if video {
                            let rect=crate::paint::ClipRect::new(x,y,width,height);canvas.fill_rect(x,y,width,height,0xff000000);
                            if !self.embedded_video_retired(){if let Some(bitmap)=self.images.get(&node){canvas.fitted_image(rect,bitmap,presentation);}}
                            self.draw_embedded_video_controls(&mut canvas,node,rect);
                            continue;
                        }
                        if let Some(bitmap) = self.images.get(&node) {
                            canvas.fitted_image(
                                crate::paint::ClipRect::new(x, y, width, height),
                                bitmap,
                                presentation,
                            );
                            continue;
                        }
                    }
                    canvas.fill_rect(x, y, width, height, 0xe9edf1);
                    canvas.fill_rect(x, y, width, 1, 0x8a98a6);
                    canvas.fill_rect(x, y, 1, height, 0x8a98a6);
                    canvas.fill_rect(x, y + height - 1, width, 1, 0x8a98a6);
                    canvas.fill_rect(x + width - 1, y, 1, height, 0x8a98a6);
                    if width >= 12 && height >= 14 {
                        let alt = self
                            .document
                            .attr(node, "alt")
                            .unwrap_or("Image unavailable");
                        canvas.text(
                            x + 5,
                            y + 5,
                            &fit_text(alt, ((width - 10) / 6).max(0) as usize),
                            1,
                            0x5d6770,
                        );
                    }
                }
                PaintItem::Text(index) => {
                    let run = &self.page.runs[index];
                    let color = self.page_text_color(run);
                    let y = run.y + top - self.scroll;
                    let h = run.height();
                    if y + h <= top || y >= bottom {
                        continue;
                    }
                    let length = run.text.chars().count();
                    let first_match = self.matches.partition_point(|found| found.end.run < index);
                    for (i, found) in self
                        .matches
                        .iter()
                        .enumerate()
                        .skip(first_match)
                        .take_while(|(_, found)| found.start.run <= index)
                    {
                        if let Some((a, b)) = found.slice(index, length) {
                            canvas.fill_rect(
                                run.x + run.character_x(a),
                                y - 1,
                                run.character_x(b) - run.character_x(a),
                                h,
                                if i == self.match_index {
                                    0xffb95c
                                } else {
                                    0xffed96
                                },
                            );
                        }
                    }
                    if let Some((a, b)) = self.selection.and_then(|s| s.slice(index, length)) {
                        canvas.fill_rect(
                            run.x + run.character_x(a),
                            y - 1,
                            run.character_x(b) - run.character_x(a),
                            h,
                            0xb4d8fa,
                        );
                    }
                    if let Some(shape) = &run.shaped {
                        shape.draw(&mut canvas, run.x, y, color);
                    } else {
                        let text = run.text.replace('\u{a0}', " ");
                        canvas.styled_text(run.x, y, &text, run.scale, color, run.italic);
                        if run.bold {
                            canvas.styled_text(run.x + 1, y, &text, run.scale, color, run.italic);
                        }
                    }
                    if run.underline {
                        canvas.fill_rect(run.x, y + run.ascent() + 2, run.width(), 1, color);
                    }
                    // Keep search/selection readable on dark pages with light text.
                    for found in self
                        .matches
                        .iter()
                        .skip(first_match)
                        .take_while(|found| found.start.run <= index)
                    {
                        if let Some((a, b)) = found.slice(index, length) {
                            highlight_text(&mut canvas, run, y, a, b);
                        }
                    }
                    if let Some((a, b)) = self.selection.and_then(|s| s.slice(index, length)) {
                        highlight_text(&mut canvas, run, y, a, b);
                    }
                }
            }
        }
        if let Some(painter) = &mut opacity { painter.finish(&mut canvas); }
        canvas.set_clip(None);
        if !paint_refused && self.max_scroll() > 0 {
            let (y, thumb, track) = self.scrollbar();
            canvas.fill_rect(self.width - 12, top, 12, track, ui::SURFACE);
            ui::rounded_rect(&mut canvas, (self.width - 9, y, 6, thumb), 3.0, ui::MUTED);
        }
        if !paint_refused { self.draw_select_popup(&mut canvas); }
        else {
            canvas.set_clip(Some(viewport));
            canvas.text(16, top + 16, "Page painting unavailable.", 1, ui::ERROR);
            canvas.text(16, top + 34, "Rendering work or memory limit. Ctrl+I shows details.", 1, ui::INK);
            canvas.set_clip(None);
        }
        if self.inspector.mode == inspector::Mode::Docked {
            self.draw_inspector(&mut canvas);
        }
        let bottom = self.height - FOOT_HEIGHT;
        // Chrome is painted last so document content cannot cover the controls.
        self.draw_toolbar(&mut canvas);
        self.draw_bookmarks_bar(&mut canvas);
        self.draw_find_bar(&mut canvas);
        let profile_warning = self.profile.problem().is_some() && !self.failed;
        let history_warning = self.browsing_history_error.is_some() && !self.failed;
        canvas.fill_rect(
            0,
            bottom,
            self.width,
            FOOT_HEIGHT,
            if self.failed || paint_refused {
                ui::ERROR
            } else if profile_warning || history_warning {
                ui::CREAM
            } else {
                ui::NAVY
            },
        );
        let status = if paint_refused {
            "Page painting unavailable. Ctrl+I shows limits."
        } else if profile_warning {
            "Preferences unavailable; Ctrl+, for details."
        } else if history_warning {
            "History needs attention; Ctrl+H for details."
        } else if !self.status.is_empty() {
            self.status.as_str()
        } else if let Some(href) = &self.hover {
            href.as_str()
        } else {
            match self.view {
                View::Page if self.location.url.as_ref().is_some_and(|url| url.secure) => {
                    "HTTPS / CERTIFICATE VERIFIED  |  Ctrl+I resources"
                }
                View::Page if self.location.url.is_some() => {
                    "HTTP / UNENCRYPTED  |  Ctrl+L address  |  Ctrl+I resources"
                }
                View::Page => "LOCAL FILES  |  Ctrl+O open  |  Ctrl+F find  |  Ctrl+I resources",
                View::Source => "PAGE SOURCE  |  Ctrl+U returns to page",
                View::Resources => "RESOURCE ACTIVITY  |  Ctrl+I returns to page",
            }
        };
        canvas.text(
            12,
            bottom + 12,
            &fit_text(status, ((self.width - 108) / 6).max(0) as usize),
            1,
            if profile_warning || history_warning {
                ui::INK
            } else {
                ui::CREAM
            },
        );
        ui::rounded_rect(
            &mut canvas,
            (self.width - 76, bottom + 6, 64, 20),
            5.0,
            if self.failed {
                ui::ERROR
            } else if profile_warning || history_warning {
                ui::CREAM
            } else {
                ui::NAVY_LIGHT
            },
        );
        canvas.text(
            self.width - 64,
            bottom + 12,
            &format!("{:.0}%", self.zoom * 100.0),
            1,
            if self.failed {
                ui::CREAM
            } else if profile_warning || history_warning {
                ui::TEAL_DARK
            } else {
                ui::TEAL
            },
        );
        self.draw_media_controls(&mut canvas);
        self.draw_browser_panel(&mut canvas);
        self.draw_bookmark_popup(&mut canvas);
        self.draw_downloads(&mut canvas);
        self.draw_menu(&mut canvas);
        self.draw_chrome_tooltip(&mut canvas);
        self.observe_canvas(&canvas);
        canvas
    }
}

fn highlight_text(
    canvas: &mut Canvas,
    run: &crate::layout::TextRun,
    y: i32,
    start: usize,
    end: usize,
) {
    if let Some(shape) = &run.shaped {
        shape.draw_range(canvas, run.x, y, 0x182b3b, start, end);
        if run.underline {
            canvas.fill_rect(
                run.x + shape.x(start),
                y + shape.ascent + 2,
                shape.x(end) - shape.x(start),
                1,
                0x182b3b,
            );
        }
        return;
    }
    let text: String = run
        .text
        .chars()
        .skip(start)
        .take(end - start)
        .map(|c| if c == '\u{a0}' { ' ' } else { c })
        .collect();
    let x = run.x + start as i32 * GLYPH_ADVANCE * run.scale;
    canvas.styled_text(x, y, &text, run.scale, 0x182b3b, run.italic);
    if run.bold {
        canvas.styled_text(x + 1, y, &text, run.scale, 0x182b3b, run.italic);
    }
    if run.underline {
        canvas.fill_rect(
            x,
            y + GLYPH_HEIGHT * run.scale + 2,
            (end - start) as i32 * GLYPH_ADVANCE * run.scale,
            1,
            0x182b3b,
        );
    }
}

fn control_character_at(rect: &layout::ControlBox, value: &Editor, x: i32) -> usize {
    if let Some(font) = &rect.text_font {
        let shape = font.shape(&value.text());
        shape.hit(x - rect.x - 6 + shape.x(value.offset))
    } else {
        value.offset + ((x - rect.x - 6).max(0) / (GLYPH_ADVANCE * rect.scale)) as usize
    }
}

fn draw_control_text(
    canvas: &mut Canvas,
    rect: &layout::ControlBox,
    y: i32,
    control: &crate::forms::Control,
    placeholder: &str,
    focused: bool,
    color: u32,
) {
    let font = rect.text_font.as_ref().unwrap();
    let padding = rect.padding;
    let (x,y,width,height) = (
        rect.x.saturating_add(padding[3]),y.saturating_add(padding[0]),
        (rect.width-padding[1]-padding[3]).max(0),
        (rect.height-padding[0]-padding[2]).max(0),
    );
    let clip = canvas.clip();
    canvas.set_clip(Some(clip.intersect(crate::paint::ClipRect::new(
        x + 2,
        y + 2,
        (width - 4).max(0),
        (height - 4).max(0),
    ))));
    if control.kind == Kind::Text {
        let value = &control.value;
        let shape = font.shape(&value.text());
        let origin = x + 6 - shape.x(value.offset);
        let text_y = y + (height - shape.height()) / 2;
        if value.chars.is_empty() && !focused {
            let hint_text=textarea::single_line_hint(placeholder);
            let hint = font.shape(&font.fit(&hint_text, (width - 12).max(0)));
            hint.draw(canvas, x + 6, y + (height - hint.height()) / 2, 0x75818b);
        }
        let selected = value.range();
        if focused && !selected.is_empty() {
            canvas.fill_rect(
                origin + shape.x(selected.start),
                y + 2,
                shape.x(selected.end) - shape.x(selected.start),
                height - 4,
                0xb4d8fa,
            );
        }
        shape.draw(canvas, origin, text_y, color);
        if focused && !selected.is_empty() {
            shape.draw_range(
                canvas,
                origin,
                text_y,
                0x203847,
                selected.start,
                selected.end,
            );
        }
        if focused {
            canvas.fill_rect(origin + shape.x(value.cursor), y + 3, 1, height - 6, color);
        }
    } else {
        let select = control.kind == Kind::Select;
        let caption = font.fit(
            &control.caption,
            (width - if select { 32 } else { 12 }).max(0),
        );
        let shape = font.shape(&caption);
        let text_x = if select {
            x + 6
        } else {
            x + (width - shape.width()) / 2
        };
        shape.draw(canvas, text_x, y + (height - shape.height()) / 2, color);
        if select && width >= 18 {
            for row in 0..4 {
                canvas.fill_rect(
                    x + width - 16 + row,
                    y + height / 2 - 2 + row,
                    7 - 2 * row,
                    1,
                    color,
                );
            }
        }
    }
    canvas.set_clip(Some(clip));
}

/// The layout engine uses integer CSS pixels. Correct responsive image density
/// before page zoom, preserving the bitmap's aspect when layout bounds apply.
fn natural_image_sizes(
    images: &HashMap<usize, std::sync::Arc<Bitmap>>,
    densities: &HashMap<usize, f64>,
) -> HashMap<usize, (usize, usize)> {
    images
        .iter()
        .map(|(&node, bitmap)| {
            let density = densities.get(&node).copied().unwrap_or(1.0);
            let size = if density == f64::INFINITY {
                (0, 0)
            } else {
                let largest = bitmap.width.max(bitmap.height).max(1) as f64;
                // Explicit 0x is valid HTML. Its unbounded intrinsic dimensions are
                // capped here, before integer conversion or layout multiplication.
                let density = if density.is_finite() && density >= 0.0 {
                    density
                } else {
                    1.0
                };
                let scale = if density == 0.0 {
                    32_768.0 / largest
                } else {
                    (1.0 / density).min(32_768.0 / largest)
                };
                (
                    (bitmap.width as f64 * scale).round().clamp(0.0, 32_768.0) as usize,
                    (bitmap.height as f64 * scale).round().clamp(0.0, 32_768.0) as usize,
                )
            };
            (node, size)
        })
        .collect()
}

fn draw_control(
    canvas: &mut Canvas,
    rect: &layout::ControlBox,
    y: i32,
    control: &crate::forms::Control,
    placeholder: &str,
    focused: bool,
    bitmap: Option<&Bitmap>,
) {
    let (x, width, height, scale) = (rect.x, rect.width, rect.height, rect.scale);
    if control.kind == Kind::Image {
        if let (Some(image), Some(bitmap)) = (&rect.image, bitmap) {
            if let Some(background) = rect.background {
                canvas.fill_rect(x, y, width, height, background);
            }
            canvas.fitted_image(
                crate::paint::ClipRect::new(
                    x.saturating_add(image.x),
                    y.saturating_add(image.y),
                    image.width,
                    image.height,
                ),
                bitmap,
                image.presentation,
            );
            for (side,(x,y,width,height)) in crate::paint::border_segments(x,y,width,height,image.border,image.border_alpha).into_iter().enumerate() {
                canvas.fill_rect_alpha(x,y,width,height,image.border_color[side],image.border_alpha[side]);
            }
            if focused {
                let color = 0x178b78;
                canvas.fill_rect(x, y, width, 1, color);
                canvas.fill_rect(x, y, 1, height, color);
                canvas.fill_rect(
                    x,
                    y.saturating_add(height).saturating_sub(1),
                    width,
                    1,
                    color,
                );
                canvas.fill_rect(
                    x.saturating_add(width).saturating_sub(1),
                    y,
                    1,
                    height,
                    color,
                );
            }
            return;
        }
    }
    let inactive = control.disabled || control.kind == Kind::Unsupported;
    let color = if inactive { 0x78858e } else { rect.color };
    let background = if inactive {
        0xe8edf0
    } else {
        rect.background.unwrap_or(
            if matches!(
                control.kind,
                Kind::Submit | Kind::Image | Kind::Reset | Kind::Button
            ) {
                0xe3edf2
            } else {
                0xffffff
            },
        )
    };
    let border = if focused { 0x178b78 } else { 0x7c8b96 };
    canvas.fill_rect(x, y, width, height, border);
    canvas.fill_rect(
        x + 1,
        y + 1,
        (width - 2).max(0),
        (height - 2).max(0),
        background,
    );
    if focused {
        canvas.fill_rect(x, y + height - 2.min(height), width, 2.min(height), border);
    }
    if matches!(control.kind, Kind::Checkbox | Kind::Radio) {
        let size = (GLYPH_HEIGHT * scale).min(width - 4).min(height - 4).max(0);
        let sx = x + (width - size) / 2;
        let sy = y + (height - size) / 2;
        if control.kind == Kind::Radio {
            let inset = (size / 4).max(1);
            canvas.fill_rect(sx + inset, sy, size - 2 * inset, size, border);
            canvas.fill_rect(sx, sy + inset, size, size - 2 * inset, border);
            canvas.fill_rect(
                sx + inset,
                sy + inset,
                (size - 2 * inset).max(0),
                (size - 2 * inset).max(0),
                if control.checked { color } else { background },
            );
        } else if control.checked && size >= 5 {
            for row in 0..size {
                let dx = row.min(size - row - 1);
                canvas.fill_rect(sx + dx, sy + row, scale.min(size), scale.min(size), color);
                canvas.fill_rect(
                    sx + size - dx - scale.min(size),
                    sy + row,
                    scale.min(size),
                    scale.min(size),
                    color,
                );
            }
        }
        return;
    }
    if control.kind == Kind::TextArea {
        textarea::draw(canvas, rect, y, control, placeholder, focused, color);
        return;
    }
    if rect.text_font.is_some() {
        draw_control_text(canvas, rect, y, control, placeholder, focused, color);
        return;
    }
    let padding = rect.padding;
    let (x,y,width,height) = (
        x.saturating_add(padding[3]),y.saturating_add(padding[0]),
        (width-padding[1]-padding[3]).max(0),
        (height-padding[0]-padding[2]).max(0),
    );
    if height < GLYPH_HEIGHT * scale + 2 {
        return;
    }
    let clip = canvas.clip();
    if padding != [0;4] {
        canvas.set_clip(Some(clip.intersect(crate::paint::ClipRect::new(
            x+2,y+2,(width-4).max(0),(height-4).max(0),
        ))));
    }
    let advance = GLYPH_ADVANCE * scale;
    let cells = ((width - 12) / advance).max(0) as usize;
    let text_y = y + (height - GLYPH_HEIGHT * scale) / 2;
    if control.kind == Kind::Text {
        let value = &control.value;
        if value.chars.is_empty() && !focused {
            canvas.text(
                x + 6,
                text_y,
                &fit_text(&textarea::single_line_hint(placeholder), cells),
                scale,
                0x75818b,
            );
        }
        let selected = value.range();
        for (i, &ch) in value
            .chars
            .iter()
            .enumerate()
            .skip(value.offset)
            .take(cells)
        {
            let px = x + 6 + (i - value.offset) as i32 * advance;
            if focused && selected.contains(&i) {
                canvas.fill_rect(px, y + 2, advance, height - 4, 0xb4d8fa);
            }
            let mut bytes = [0u8; 4];
            canvas.text(
                px,
                text_y,
                ch.encode_utf8(&mut bytes),
                scale,
                if focused && selected.contains(&i) {
                    0x203847
                } else {
                    color
                },
            );
        }
        if focused && value.cursor >= value.offset && value.cursor - value.offset <= cells {
            canvas.fill_rect(
                x + 5 + (value.cursor - value.offset) as i32 * advance,
                y + 3,
                1,
                height - 6,
                color,
            );
        }
    } else if control.kind == Kind::Select {
        let cells = ((width - 32) / advance).max(0) as usize;
        canvas.text(
            x + 6,
            text_y,
            &fit_text(&control.caption, cells),
            scale,
            color,
        );
        if width >= 18 {
            let arrow_x = x + width - 16;
            let arrow_y = y + height / 2 - 2;
            for row in 0..4 {
                canvas.fill_rect(arrow_x + row, arrow_y + row, 7 - 2 * row, 1, color);
            }
        }
    } else {
        let caption = fit_text(&control.caption, cells);
        let text_x = x + (width - caption.chars().count() as i32 * advance) / 2;
        canvas.text(text_x, text_y, &caption, scale, color);
    }
    canvas.set_clip(Some(clip));
}
fn field_themed(
    canvas: &mut Canvas,
    editor: &Editor,
    rect: (i32, i32, i32, i32),
    focused: bool,
    palette: ui::Palette,
) {
    field_at_offset_themed(canvas, editor, rect, focused, editor.offset, palette);
}
// The idle address field starts at the origin; focused editing still follows
// the caret. Other native fields keep their existing editing offset.
fn field_at_offset_themed(
    canvas: &mut Canvas,
    editor: &Editor,
    rect: (i32, i32, i32, i32),
    focused: bool,
    offset: usize,
    palette: ui::Palette,
) {
    let (x, y, w, h) = rect;
    ui::rounded_rect(
        canvas,
        rect,
        7.0,
        if focused {
            palette.panel
        } else {
            palette.panel_alt
        },
    );
    ui::rounded_border(
        canvas,
        rect,
        7.0,
        if focused { 2.0 } else { 1.0 },
        if focused {
            palette.accent
        } else {
            palette.border
        },
    );
    let cells = ((w - 16) / 12).max(0) as usize;
    let range = editor.range();
    for (i, ch) in editor.chars.iter().enumerate().skip(offset).take(cells) {
        let px = x + 8 + (i - offset) as i32 * 12;
        if focused && range.contains(&i) {
            canvas.fill_rect(px, y + 5, 12, h - 10, palette.selected);
        }
        let mut bytes = [0u8; 4];
        canvas.text(
            px,
            y + (h - 14) / 2,
            ch.encode_utf8(&mut bytes),
            2,
            palette.foreground,
        );
    }
    if focused && editor.cursor >= offset && editor.cursor - offset <= cells {
        canvas.fill_rect(
            x + 7 + (editor.cursor - offset) as i32 * 12,
            y + 6,
            1,
            h - 12,
            palette.foreground,
        );
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn fit_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() && max_chars >= 3 {
        result = result.chars().take(max_chars - 3).collect();
        result.push_str("...");
    }
    result
}

#[cfg(test)]
#[path = "app_chrome_tests.rs"]
mod chrome_tests;

#[cfg(test)]
#[path = "app_font_tests.rs"]
mod font_tests;

#[cfg(test)]
#[path = "app_geometry_tests.rs"]
mod geometry_tests;

#[cfg(test)]
#[path = "app_history_tests.rs"]
mod saved_history_tests;

#[cfg(test)]
#[path = "app_post_tests.rs"]
mod post_tests;

#[cfg(test)]
#[path = "app_script_tests.rs"]
mod script_tests;

#[cfg(test)]
#[path = "app_loading_tests.rs"]
mod loading_tests;

#[cfg(test)]
#[path = "app_script_navigation_tests.rs"]
mod script_navigation_tests;

#[cfg(test)]
#[path = "app_select_tests.rs"]
mod select_tests;

#[cfg(test)]
mod tests {
    mod autonomous_element_identity;
    mod inline_easing;
    mod attribute_substrings;
    mod canvas_2d;
    mod canvas_colors;
    mod canvas_readback;
    mod inline_style_keys;

    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    mod responsive_picture;
    static SERIAL: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn calculated_box_gutters_match_literal_geometry_paint_and_link_hits() {
        let files = Fixture::new();
        let path = files.write("gutters.html", r##"<!doctype html><html><head><style>
            html,body{margin:0;padding:0;background:white}
            .row{width:200px;display:flex;--gutter:1.5rem}
            .column{width:100px;box-sizing:border-box}
            .literal .column{padding-left:12px;padding-right:12px}
            .calculated .column{padding-left:calc(var(--gutter) * .5);padding-right:calc(.5 * var(--gutter))}
            a{display:block;height:20px;background:#336699}
            </style></head><body>
            <div class="literal row"><div class=column><a id=a href="#a"></a></div><div class=column><a id=b href="#b"></a></div></div>
            <div class="calculated row"><div class=column><a id=c href="#c"></a></div><div class=column><a id=d href="#d"></a></div></div>
            </body></html>"##);
        let mut app = App::new();
        app.resize(640, 480);
        app.open(&path);
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        let canvas = app.draw();
        for (ids, top) in [(["a", "b"], 0), (["c", "d"], 20)] {
            let nodes = ids.map(|id| (0..app.document.nodes.len())
                .find(|&n| app.document.attr(n, "id") == Some(id)).unwrap());
            for (&node, x) in nodes.iter().zip([12, 112]) {
                let r = app.page.border_rects[node].unwrap();
                assert_eq!((r.x, r.y, r.width, r.height), (x, top, 76, 20));
            }
            let y = top + 10;
            for x in 0..200 {
                let expected_link = match x {
                    12..=87 => Some(nodes[0]),
                    112..=187 => Some(nodes[1]),
                    _ => None,
                };
                let pixel = canvas.pixels[(app.content_top() + y) as usize * canvas.width + x as usize];
                assert_eq!(pixel, if expected_link.is_some() { 0x336699 } else { 0xffffff }, "x={x}, y={y}");
                assert_eq!(app.page.link_node_at(x, y), expected_link, "x={x}, y={y}");
            }
        }
    }

    #[test]
    fn responsive_density_and_object_fit_reach_layout_paint_links_and_image_controls() {
        let files = Fixture::new();
        let picture="data:image/x-portable-pixmap,P3%204%202%20255%20255%200%200%200%20255%200%200%200%20255%20255%20255%20255%200%200%200%20255%20255%200%20255%200%20255%200%20255%20255";
        let path=files.write("presentation.html",&format!("<title>Image presentation</title><body style='margin:0;background:#123456;image-rendering:crisp-edges'><a href=next.html><img id=contained src='{picture}' style='display:block;width:4px;height:4px;object-fit:contain;object-position:left bottom'></a><img id=density srcset='{picture} 2x' style='display:block'><form action=next.html><input id=button type=image src='{picture}' style='display:block;width:4px;height:4px;border:0;padding:0;object-fit:cover;object-position:left top'></form>"));
        files.write("next.html", "<title>Image link reached</title>");
        let mut app = App::new();
        app.open(&path);
        assert!(!app.failed, "{}", app.status);
        let node = |id| {
            app.document
                .nodes
                .iter()
                .enumerate()
                .find(|(n, _)| app.document.attr(*n, "id") == Some(id))
                .unwrap()
                .0
        };
        let (contained, density, button) = (node("contained"), node("density"), node("button"));
        let rect = |node| {
            app.page
                .items
                .iter()
                .find_map(|item| match item {
                    PaintItem::Image {
                        node: n,
                        x,
                        y,
                        width,
                        height,
                        ..
                    } if *n == node => Some((*x, *y, *width, *height)),
                    _ => None,
                })
                .unwrap()
        };
        let (x, y, w, h) = rect(contained);
        assert_eq!((w, h), (4, 4));
        let (_, _, w, h) = rect(density);
        assert_eq!((w, h), (2, 1));
        assert_eq!(app.images[&density].width, 4);
        let canvas = app.draw();
        let top = app.content_top();
        let pixel = |x: i32, y: i32| canvas.pixels[(y + top) as usize * canvas.width + x as usize];
        assert_eq!(pixel(x, y), 0x123456);
        assert_eq!(pixel(x, y + 2), 0xff0000);
        assert_eq!(pixel(x + 3, y + 3), 0x00ffff);
        let control = app.page.controls.iter().find(|c| c.node == button).unwrap();
        let image = control.image.as_ref().unwrap();
        assert_eq!(pixel(control.x + image.x, control.y + image.y), 0xff0000);
        assert_eq!(
            pixel(control.x + image.x + 3, control.y + image.y),
            0x00ff00
        );
        // Transparent padding inside the image box remains part of its link.
        assert!(app.wants_hand(x, y + top));
        app.pointer_down(x, y + top, false);
        app.pointer_up(x, y + top);
        assert_eq!(app.document.title, "Image link reached");
    }

    #[test]
    fn extreme_valid_image_densities_are_bounded_before_layout() {
        let bitmap = Arc::new(Bitmap {
            width: 4,
            height: 2,
            pixels: vec![0; 8],
        });
        let images = HashMap::from([
            (1, bitmap.clone()),
            (2, bitmap.clone()),
            (3, bitmap.clone()),
            (4, bitmap),
        ]);
        let sizes = natural_image_sizes(
            &images,
            &HashMap::from([
                (1, 0.0),
                (2, f64::INFINITY),
                (3, 2.0),
                (4, f64::MIN_POSITIVE),
            ]),
        );
        assert_eq!(sizes[&1], (32768, 16384));
        assert_eq!(sizes[&2], (0, 0));
        assert_eq!(sizes[&3], (2, 1));
        assert_eq!(sizes[&4], (32768, 16384));
    }

    #[test]
    fn overflow_clipping_matches_painted_pixels_and_pointer_navigation() {
        let files = Fixture::new();
        let path = files.write("clipped.html", "<title>Clipping</title><body style='margin:0'><div style='width:30px;height:20px;overflow:hidden'><a href=next.html style='display:block;width:100px;height:100px;background:red'></a></div></body>");
        files.write("next.html", "<title>Visible link</title>");
        let mut app = App::new();
        app.open(&path);
        let canvas = app.draw();
        let top = app.content_top();
        let pixel = |x: i32, y: i32| canvas.pixels[y as usize * canvas.width + x as usize];
        assert_eq!(pixel(2, top + 2), 0xff0000);
        assert_eq!(pixel(29, top + 19), 0xff0000);
        assert_eq!(pixel(30, top + 2), 0xffffff);
        assert_eq!(pixel(2, top + 20), 0xffffff);
        assert_eq!(
            pixel(0, 0),
            ui::NAVY,
            "document clip must not affect browser chrome"
        );
        assert!(app.wants_hand(2, top + 2));
        assert!(!app.wants_hand(35, top + 2));
        app.pointer_down(35, top + 2, false);
        app.pointer_up(35, top + 2);
        assert_eq!(app.document.title, "Clipping");
        app.pointer_down(2, top + 2, false);
        app.pointer_up(2, top + 2);
        assert_eq!(app.document.title, "Visible link");
    }
    #[test]
    fn scripted_overflow_clips_pixels_and_hit_targets_and_can_be_removed() {
        let files=Fixture::new();
        let path=files.write("script-clip.html","<body style='margin:0'><div id=p style='width:30px;height:20px'><a href=next.html style='display:block;width:100px;height:100px;background:red'></a></div><script>var p=document.getElementById('p');p.style.overflow='hidden';if(p.style.getPropertyValue('overflow')!=='hidden')throw 'declaration';</script>");
        files.write("next.html","<title>Next</title>");
        let mut app=App::new();app.set_javascript_enabled(true);app.open(&path);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
        let top=app.content_top();let canvas=app.draw();
        assert_eq!(canvas.pixels[(top+2) as usize*canvas.width+35],0xffffff);
        assert!(app.wants_hand(2,top+2));assert!(!app.wants_hand(35,top+2));
        let result=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,"if(p.style.removeProperty('overflow')!=='hidden')throw 'old value';");
        assert_eq!(result.errors,0);app.reflow();
        let canvas=app.draw();assert_eq!(canvas.pixels[(top+2) as usize*canvas.width+35],0xff0000);assert!(app.wants_hand(35,top+2));
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "chezcat-app-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn write(&self, name: &str, content: &str) -> PathBuf {
            let p = self.0.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
            p
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct LiveSite {
        url: String,
        requests: Arc<Mutex<Vec<String>>>,
        headers: Arc<Mutex<Vec<String>>>,
        stop: Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl LiveSite {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let seen = requests.clone();
            let headers = Arc::new(Mutex::new(Vec::new()));
            let seen_headers = headers.clone();
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let ending = stop.clone();
            let thread = std::thread::spawn(move || {
                let mut clients = Vec::new();
                while !ending.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            // Accepted Winsock sockets inherit the polling listener mode.
                            stream.set_nonblocking(false).unwrap();
                            let seen = seen.clone();
                            let seen_headers = seen_headers.clone();
                            clients.push(std::thread::spawn(move || {
                                stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
                                let mut request = Vec::new(); let mut byte = [0u8; 1];
                                while request.len() < 8192 && !request.ends_with(b"\r\n\r\n") {
                                    if stream.read(&mut byte).unwrap_or(0) == 0 { return; } request.push(byte[0]);
                                }
                                let text = String::from_utf8(request).unwrap();
                                let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
                                seen.lock().unwrap().push(path.clone());
                                seen_headers.lock().unwrap().push(text);
                                let mut extra = String::new(); let mut mime = "text/html;charset=utf-8"; let mut status = 200;
                                let body = match path.as_str() {
                                    "/redirect" => { status = 302; extra = "Location: /doc#target\r\n".to_owned(); String::new() }
                                    "/location-redirect" => { status = 302; extra = "Location: /location-page?q=%23#target\r\n".to_owned(); String::new() }
                                    path if path.starts_with("/location-page") => "<html><head><base href='/assets/'></head><body><button id=read type=button>Read</button><p id=target>Target</p><script>var saved=location,stringify=location.toString;function readURL(){if(saved!==window.location || saved!==document.location || stringify!==location.toString)throw Error('location identity');if(document.URL!==saved.href || document.documentURI!==saved.href)throw Error('document URL');document.title=stringify.call(saved);}readURL();document.getElementById('read').onclick=readURL;</script></body></html>".to_owned(),
                                    "/doc" => format!("<title>Network page</title><base href='/assets/'><link rel=stylesheet href=style.css><img src=pixel.ppm><img src='http://localhost:{port}/blocked'><img src='file:///blocked'><p>Start</p>{}<p id=target>Target</p>{}", "<p>Before</p>".repeat(20), "<p>After</p>".repeat(20)),
                                    "/assets/style.css" => { mime = "text/css"; "p{color:#123456}".to_owned() }
                                    "/assets/pixel.ppm" => { mime = "image/x-portable-pixmap"; "P3\n1 1\n255\n255 0 0\n".to_owned() }
                                    "/slow" | "/slow2" => { std::thread::sleep(std::time::Duration::from_millis(600)); "<title>Stale slow page</title><p>Slow</p>".to_owned() }
                                    "/empty" => { status = 204; String::new() }
                                    "/compat" => "<title>Compatibility controls</title><style>body{margin:0} .atomic{display:inline-block} .atomic button{display:block} .padded{display:inline-block;padding:12px} input{width:200px}</style><center><h1>Find something</h1><form action='/form'><table cellpadding=0 cellspacing=0><tr><td width='25%'></td><td align=center><input name=q value=seed></td><td width='25%'><a class=padded href='/fast'>Open</a></td></tr><tr><td></td><td align=center><span class=atomic><button name=go value=send>Search</button></span> <span class=atomic><button type=reset>Reset</button></span></td><td></td></tr></table></form></center>".to_owned(),
                                    path if path == "/form" || path.starts_with("/form?") => "<title>Live form</title><form action='/form' method=get><label for=q>Query</label><input id=q name=q value=hello required><input type=hidden name=source value=fixture><label><input type=checkbox name=details value=yes checked>Details</label><button name=go value=send>Submit</button><button type=reset>Reset</button></form>".to_owned(),
                                    _ => "<title>Next page</title><p>Fast</p>".to_owned(),
                                };
                                let response = format!("HTTP/1.1 {status} Test\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}", body.len());
                                let _ = stream.write_all(response.as_bytes());
                            }));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(2))
                        }
                        Err(e) => panic!("Fixture accept failed: {e}"),
                    }
                }
                for client in clients {
                    client.join().unwrap();
                }
            });
            Self {
                url: format!("http://127.0.0.1:{port}"),
                requests,
                headers,
                stop,
                thread: Some(thread),
            }
        }
        fn wait_for_request(&self, path: &str) {
            let started = std::time::Instant::now();
            while !self.requests.lock().unwrap().iter().any(|r| r == path) {
                assert!(started.elapsed() < std::time::Duration::from_secs(3));
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }
    impl Drop for LiveSite {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    #[test]
    fn command_line_files_preserve_literal_characters_and_use_the_working_directory() {
        let files = Fixture::new();
        let path = files.write(
            "folder # 100%/page.html",
            "<title>Literal filename</title><p>Local</p>",
        );
        let mut app = App::new();
        app.open_argument(path.as_os_str());
        assert!(!app.failed, "{}", app.status);
        assert_eq!(app.document.title, "Literal filename");
        app.use_home_file(path.clone());
        app.open_argument(std::ffi::OsStr::new("pages/second.html"));
        assert!(!app.failed, "{}", app.status);
        assert_eq!(
            app.location,
            navigation::explicit(Path::new("pages/second.html")).unwrap()
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let name = std::ffi::OsString::from_vec(b"native-\xff.html".to_vec());
            let path = files.0.join(name);
            std::fs::write(&path, b"<title>Native filename</title>").unwrap();
            app.open_argument(path.as_os_str());
            assert!(!app.failed, "{}", app.status);
            assert_eq!(app.document.title, "Native filename");
        }
    }

    #[test]
    fn failed_navigation_keeps_the_page_and_exposes_the_attempt_in_resources() {
        let files = Fixture::new();
        let path = files.write(
            "page.html",
            "<title>Current page</title><p>Keep this page</p>",
        );
        let mut app = App::new();
        app.open(&path);
        let before = app.location.clone();
        app.open_address("https://127.0.0.1:9/");
        app.wait_for_load();
        assert!(app.failed);
        assert_eq!(app.location, before);
        assert_eq!(app.document.title, "Current page");
        assert_eq!(app.address.text(), before.display());
        let report = app.activity_report();
        assert!(report.starts_with(crate::http::USER_AGENT));
        assert!(report.contains("LAST FAILED NAVIGATION:"));
        assert!(report.contains("https://127.0.0.1:9/"));
        assert!(report.contains("Page title: Current page"));
        assert_eq!(app.location, before);
        assert!(matches!(app.view, View::Page));
        app.show_resources();
        let text = app
            .page
            .runs
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("LAST FAILED NAVIGATION:"), "{text}");
        assert!(text.contains("https://127.0.0.1:9/"), "{text}");
        assert!(text.contains("TRUST FILE:"), "{text}");
        app.show_resources();
        let missing = files.0.join("missing-page.html");
        app.open_argument(missing.as_os_str());
        assert!(app.failed);
        assert!(app.activity_report().contains("missing-page.html"));
        assert_eq!(app.location, before);
        assert_eq!(app.document.title, "Current page");
        app.open(&path);
        assert!(app.last_failed_navigation.is_none());
    }

    #[test]
    fn full_reload_shortcuts_refetch_assets_and_do_not_leak_to_normal_navigation() {
        let site = LiveSite::new();
        let mut app = App::new();
        app.open_address(&format!("{}/doc", site.url));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        app.scroll_by(160);
        let scroll = app.scroll;
        let history = (app.history.len(), app.history_index);
        for (key, ctrl, shift, force) in [
            (0x74, true, false, true),
            (0x74, false, false, false),
            (0x52, true, true, true),
            (0x52, true, false, false),
        ] {
            let start = site.headers.lock().unwrap().len();
            app.key(key, ctrl, shift, false);
            assert!(app.is_loading());
            app.wait_for_load();
            assert!(!app.failed, "{}", app.status);
            assert_eq!((app.history.len(), app.history_index), history);
            assert_eq!(app.scroll, scroll);
            assert_eq!(
                app.events.iter().any(|e| e.starts_with("FULL RELOAD:")),
                force
            );
            let headers = site.headers.lock().unwrap();
            let requests = &headers[start..];
            assert_eq!(requests.len(), 3);
            let paths: Vec<_> = requests
                .iter()
                .map(|r| r.split_whitespace().nth(1).unwrap())
                .collect();
            assert_eq!(paths, ["/doc", "/assets/style.css", "/assets/pixel.ppm"]);
            for request in requests {
                assert_eq!(request.contains("\r\nCache-Control: no-cache\r\n"), force);
                assert_eq!(request.contains("\r\nPragma: no-cache\r\n"), force);
                assert!(
                    !request.contains("If-None-Match:") && !request.contains("If-Modified-Since:")
                );
            }
        }
        let start = site.headers.lock().unwrap().len();
        app.open_address(&format!("{}/fast", site.url));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert!(!site.headers.lock().unwrap()[start].contains("Cache-Control:"));
    }

    #[test]
    fn full_reload_restarts_pending_redirects_and_preserves_history_semantics() {
        let site = LiveSite::new();
        let mut app = App::new();
        app.open_address(&format!("{}/redirect", site.url));
        app.key(0x74, true, false, false);
        assert_eq!(
            app.pending.as_ref().unwrap().mode,
            crate::http::FetchMode::FullReload
        );
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert_eq!(app.history.len(), 2);
        assert_eq!(app.history_index, 1);
        assert_eq!(app.location.fragment, "target");
        let headers = site.headers.lock().unwrap();
        let full_paths: Vec<_> = headers
            .iter()
            .filter(|r| r.contains("\r\nCache-Control: no-cache\r\n"))
            .map(|r| r.split_whitespace().nth(1).unwrap())
            .collect();
        assert_eq!(
            full_paths,
            [
                "/redirect",
                "/doc",
                "/assets/style.css",
                "/assets/pixel.ppm"
            ]
        );
    }

    #[test]
    fn centered_table_controls_remain_clickable_after_resize_zoom_and_find() {
        let site = LiveSite::new();
        let mut app = App::new();
        for (width, zoom) in [(920, 1.0), (480, 1.25)] {
            app.open_address(&format!("{}/compat", site.url));
            app.wait_for_load();
            assert!(!app.failed, "{}", app.status);
            app.resize(width, 650);
            app.set_zoom(zoom);
            app.search("Find something");
            assert_eq!(app.matches.len(), 1);
            app.key(0x1b, false, false, false);
            let query = *app
                .forms
                .controls
                .iter()
                .find(|(_, c)| c.name == "q")
                .unwrap()
                .0;
            let reset = *app
                .forms
                .controls
                .iter()
                .find(|(_, c)| c.kind == Kind::Reset)
                .unwrap()
                .0;
            let submit = *app
                .forms
                .controls
                .iter()
                .find(|(_, c)| c.name == "go")
                .unwrap()
                .0;
            for (node, text) in [
                (query, Some("edited")),
                (reset, None),
                (query, Some("layout + café")),
                (submit, None),
            ] {
                let field = app
                    .page
                    .controls
                    .iter()
                    .find(|c| c.node == node)
                    .unwrap()
                    .clone();
                let (x, y) = (field.x + 6, field.y + app.content_top() - app.scroll + 6);
                assert!(
                    (0..app.width).contains(&x)
                        && (app.content_top()..app.height - FOOT_HEIGHT).contains(&y)
                );
                app.pointer_down(x, y, false);
                app.pointer_up(x, y);
                if let Some(text) = text {
                    assert!(app.focus == Focus::Control(query));
                    app.key(0x41, true, false, false);
                    app.paste(text);
                    assert_eq!(app.forms.controls[&query].value.text(), text);
                } else if node == reset {
                    assert_eq!(app.forms.controls[&query].value.text(), "seed");
                }
            }
            app.wait_for_load();
            assert!(!app.failed, "{}", app.status);
            assert_eq!(app.document.title, "Live form");
            assert!(site
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r == "/form?q=layout+%2B+caf%C3%A9&go=send"));
        }
    }

    #[test]
    fn live_form_edit_submit_and_same_url_resubmit_reach_the_server() {
        let site = LiveSite::new();
        let mut app = App::new();
        app.open_address(&format!("{}/form", site.url));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        let node = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.name == "q")
            .unwrap()
            .0;
        let field = app
            .page
            .controls
            .iter()
            .find(|c| c.node == node)
            .unwrap()
            .clone();
        let (x, y) = (field.x + 10, field.y + app.content_top() + 8);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        app.key(0x41, true, false, false);
        app.paste("café +&");
        app.key(0x09, false, false, false);
        let checkbox = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Checkbox)
            .unwrap()
            .0;
        assert!(app.focus == Focus::Control(checkbox));
        app.key(0x20, false, false, false);
        assert!(!app.forms.controls[&checkbox].checked);
        app.key(0x09, false, true, false);
        assert!(app.focus == Focus::Control(node));
        app.key(0x0d, false, false, false);
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert!(site
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r == "/form?q=caf%C3%A9+%2B%26&source=fixture&go=send"));
        assert_eq!(app.document.title, "Live form");
        // Recreate the same values. Explicit GET submission must fetch even if
        // the full URL equals the current document (fragment navigation does not).
        app.focus_control(node);
        app.key(0x41, true, false, false);
        app.paste("café +&");
        app.forms.toggle(checkbox);
        let before = site.requests.lock().unwrap().len();
        app.key(0x0d, false, false, false);
        app.wait_for_load();
        assert_eq!(site.requests.lock().unwrap().len(), before + 1);
    }

    #[test]
    fn form_labels_reset_readonly_clipboard_and_mouse_cancellation() {
        let fixture = Fixture::new();
        let file = fixture.write("form.html", "<form><input id=edit name=q value=seed maxlength=6><input id=ro name=r value=kept readonly><input name=skip disabled><label for=c>Choice</label><input id=c type=checkbox name=c><button type=reset>Reset</button></form>");
        let mut app = App::new();
        app.resize(900, 500);
        app.open(&file);
        let node = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.name == "q")
            .unwrap()
            .0;
        app.focus_control(node);
        app.key(0x41, true, false, false);
        let PlatformAction::Cut(text) = app.key(0x58, true, false, false) else {
            panic!("Cut action");
        };
        assert_eq!(app.forms.controls[&node].value.text(), "seed");
        app.cut_copied(&text);
        app.paste("é🦀abcd");
        assert_eq!(app.forms.controls[&node].value.text(), "é🦀abc");
        app.key(0x09, false, false, false);
        let readonly = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.name == "r")
            .unwrap()
            .0;
        assert!(app.focus == Focus::Control(readonly));
        app.key(0x41, true, false, false);
        assert!(matches!(
            app.key(0x58, true, false, false),
            PlatformAction::Copy(_)
        ));
        app.paste("overwrite");
        app.key(0x08, false, false, false);
        assert_eq!(app.forms.controls[&readonly].value.text(), "kept");
        let checkbox = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Checkbox)
            .unwrap()
            .0;
        let label = app.page.runs.iter().find(|r| r.text == "Choice").unwrap();
        let (x, y) = (label.x + 2, label.y + app.content_top() + 2);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        assert!(app.forms.controls[&checkbox].checked);
        let rect = app
            .page
            .controls
            .iter()
            .find(|c| c.node == checkbox)
            .unwrap()
            .clone();
        let (x, y) = (rect.x + 2, rect.y + app.content_top() + 2);
        app.pointer_down(x, y, false);
        app.pointer_up(800, 450);
        assert!(app.forms.controls[&checkbox].checked);
        let reset = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Reset)
            .unwrap()
            .0;
        app.activate_control(reset);
        assert_eq!(app.forms.controls[&node].value.text(), "seed");
        assert!(!app.forms.controls[&checkbox].checked);
        app.draw(); // Controls paint using the same live value/checked state.
    }

    #[test]
    fn live_http_redirect_base_assets_and_fragment_history() {
        let site = LiveSite::new();
        let mut app = App::new();
        app.resize(640, 420);
        app.open_address(&format!("{}/redirect", site.url));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert_eq!(app.document.title, "Network page");
        assert_eq!(app.location.url.as_ref().unwrap().path, "/doc");
        assert_eq!(app.location.fragment, "target");
        assert_eq!(app.base_location.url.as_ref().unwrap().path, "/assets/");
        assert_eq!(app.styles.len(), 1);
        assert_eq!(app.images.len(), 1);
        assert!(app.scroll > 0);
        assert_eq!(site.requests.lock().unwrap().len(), 4);
        assert!(app.events.iter().any(|e| e.contains("document's scheme")));
        app.open_address(&format!("{}/doc#top", site.url));
        let request_count = site.requests.lock().unwrap().len();
        app.history_move(-1);
        assert!(!app.is_loading());
        assert_eq!(app.location.fragment, "target");
        assert!(app.scroll > 0);
        assert_eq!(site.requests.lock().unwrap().len(), request_count);
        app.follow("../next");
        app.wait_for_load();
        assert_eq!(app.location.url.as_ref().unwrap().path, "/next");
        assert_eq!(app.document.title, "Next page");
    }

    #[test]
    fn script_location_uses_final_redirect_and_ignores_pending_or_failed_targets() {
        let site = LiveSite::new();
        let mut app = App::new();
        app.open_address(&format!("{}/location-redirect", site.url));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        let original = format!("{}/location-page?q=%23#target", site.url);
        assert_eq!(app.location.display(), original);
        assert_eq!(
            app.document.title,
            original,
            "{}",
            app.scripting.as_ref().unwrap().report()
        );
        assert_eq!(app.base_location.url.as_ref().unwrap().path, "/assets/");
        let read = app
            .document
            .nodes
            .iter()
            .enumerate()
            .find_map(|(node, _)| (app.document.attr(node, "id") == Some("read")).then_some(node))
            .unwrap();
        let next = format!("{}/location-page?q=%23#next%20part", site.url);
        let requests = site.requests.lock().unwrap().len();
        app.open_address(&next);
        assert!(!app.is_loading());
        app.activate_control(read);
        assert_eq!(app.document.title, next);
        app.history_move(-1);
        app.activate_control(read);
        assert_eq!(app.document.title, original);
        app.history_move(1);
        app.activate_control(read);
        assert_eq!(app.document.title, next);
        assert_eq!(site.requests.lock().unwrap().len(), requests);
        app.open_address(&format!("{}/slow", site.url));
        assert!(app.is_loading());
        assert_ne!(app.address.text(), next);
        app.activate_control(read);
        assert_eq!(app.document.title, next);
        app.stop_loading();
        app.open_address(&format!("{}/empty", site.url));
        app.wait_for_load();
        assert!(app.failed);
        app.activate_control(read);
        assert_eq!(app.document.title, next);
        assert_eq!(app.location.display(), next);
        assert_eq!(
            app.scripting.as_ref().unwrap().error_count(),
            0,
            "{}",
            app.scripting.as_ref().unwrap().report()
        );
    }

    #[test]
    fn replacing_and_stopping_a_live_request_keep_stale_pages_out() {
        let site = LiveSite::new();
        let mut app = App::new();
        app.open_address(&format!("{}/slow", site.url));
        site.wait_for_request("/slow");
        app.open_address(&format!("{}/fast", site.url));
        app.wait_for_load();
        assert!(!app.failed, "{}", app.status);
        assert_eq!(app.document.title, "Next page");
        std::thread::sleep(std::time::Duration::from_millis(650));
        app.poll_load();
        assert_eq!(app.document.title, "Next page");
        app.open_address(&format!("{}/slow2", site.url));
        site.wait_for_request("/slow2");
        app.stop_loading();
        std::thread::sleep(std::time::Duration::from_millis(250));
        app.poll_load();
        assert!(!app.is_loading());
        assert_eq!(app.document.title, "Next page");
        assert_eq!(app.address.text(), app.location.display());
    }

    #[test]
    fn an_http_no_content_response_keeps_the_current_document() {
        let site = LiveSite::new();
        let mut app = App::new();
        let title = app.document.title.clone();
        app.open_address(&format!("{}/empty", site.url));
        app.wait_for_load();
        assert!(app.failed);
        assert!(app.status.contains("204"));
        assert_eq!(app.document.title, title);
    }

    #[test]
    fn navigation_history_scroll_reload_and_failed_load() {
        let f = Fixture::new();
        let a = f.write(
            "a.html",
            &format!(
                "<title>A</title><a href='b.html'>Next</a>{}<p id='target'>Target</p>{}",
                "<p>Before</p>".repeat(20),
                "<p>After</p>".repeat(20)
            ),
        );
        f.write("b.html", "<title>B</title><p>Second page</p>");
        let mut app = App::new();
        app.resize(640, 420);
        app.open(&a);
        app.scroll_by(120);
        app.follow("b.html");
        assert_eq!(app.document.title, "B");
        app.history_move(-1);
        assert_eq!(app.document.title, "A");
        assert_eq!(app.scroll, 120);
        app.follow("#target");
        assert!(app.scroll > 120);
        assert_eq!(app.history_index + 1, app.history.len());
        let title = app.document.title.clone();
        app.follow("https://example.com/");
        app.wait_for_load();
        assert!(app.failed);
        assert_eq!(app.document.title, title);
        f.write("a.html", "<title>Edited</title><p>Saved change</p>");
        app.reload();
        assert_eq!(app.document.title, "Edited");
        assert!(!app.failed);
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn local_assets_and_inert_content_use_the_selected_root() {
        let f = Fixture::new();
        f.write("outside.css", "p { color:red; }");
        f.write("site/style.css", "p { color:#123456; }");
        f.write("site/pixel.ppm", "P3\n1 1\n255\n255 40 20\n");
        let file = f.write("site/index.html", "<link rel='stylesheet' href='style.css'><link rel='stylesheet' href='../outside.css'><template><img src='absent.png'></template><p>Styled</p><img src='pixel.ppm'>");
        let mut app = App::new();
        app.open(&file);
        assert!(!app.failed);
        assert_eq!(app.styles.len(), 1);
        assert_eq!(app.images.len(), 1);
        assert_eq!(
            app.page
                .runs
                .iter()
                .find(|r| r.text == "Styled")
                .unwrap()
                .color,
            0x123456
        );
        assert!(app
            .events
            .iter()
            .any(|e| e.contains("outside.css") && e.contains("SKIPPED")));
        assert!(!app.events.iter().any(|e| e.contains("absent.png")));
    }

    #[test]
    fn address_editing_and_unicode_input_preserve_text() {
        let f = Fixture::new();
        let file = f.write("space page.html", "<title>Address</title><p>Opened</p>");
        let mut app = App::new();
        app.key(0x4c, true, false, false);
        app.paste(&file.display().to_string());
        app.key(0x0d, false, false, false);
        assert_eq!(app.document.title, "Address");
        app.key(0x4c, true, false, false);
        app.utf16_input(0xd83e);
        app.utf16_input(0xdd80);
        assert_eq!(app.address.text(), "🦀");
        app.text_input('é');
        assert_eq!(app.address.text(), "🦀é");
        app.key(0x41, true, false, false);
        match app.key(0x43, true, false, false) {
            PlatformAction::Copy(text) => assert_eq!(text, "🦀é"),
            _ => panic!("No clipboard action"),
        }
        app.key(0x1b, false, false, false);
        assert_eq!(app.address.text(), app.location.display());
    }

    #[test]
    fn find_reopens_and_closing_it_clamps_scroll() {
        let mut app = App::new();
        app.resize(640, 400);
        app.search("page");
        assert!(!app.matches.is_empty());
        let count = app.matches.len();
        app.key(0x1b, false, false, false);
        assert!(app.matches.is_empty());
        app.key(0x46, true, false, false);
        assert_eq!(app.matches.len(), count);
        app.scroll = app.max_scroll();
        app.pointer_down(app.width - 20, app.chrome_bottom() + 20, false);
        app.pointer_up(app.width - 20, app.chrome_bottom() + 20);
        assert!(!app.find_open);
        assert_eq!(app.scroll, app.max_scroll());
        app.show_source();
        assert!(app.page.runs.iter().any(|r| r.text.contains("doctype")));
        app.show_source();
        assert!(app.view == View::Page);
    }

    #[test]
    fn link_drag_and_capture_cancel_do_not_navigate() {
        let f = Fixture::new();
        let file = f.write(
            "a.html",
            "<title>A</title><a href='b.html'>Open next page</a>",
        );
        f.write("b.html", "<title>B</title><p>Second</p>");
        let mut app = App::new();
        app.open(&file);
        let run = &app.page.runs[0];
        let (x, y) = (run.x + 1, run.y + app.content_top() + 1);
        app.pointer_down(x, y, false);
        app.pointer_move(x + 20, y, true);
        app.pointer_up(x + 20, y);
        assert_eq!(app.document.title, "A");
        assert!(!app.selected_text().is_empty());
        app.pointer_down(x, y, false);
        app.pointer_cancel();
        app.pointer_up(x, y);
        assert_eq!(app.document.title, "A");
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        assert_eq!(app.document.title, "B");
    }

    #[test]
    fn flex_reflow_keeps_editing_links_find_and_anchor_navigation_working() {
        let files = Fixture::new();
        let file = files.write("flex.html", &format!(
            "<title>Flex interactions</title><style>body{{margin:8px}}.row{{display:flex;flex-wrap:wrap;align-items:center;gap:16px}}.reverse{{flex-direction:row-reverse}}</style>\
            <div class='row reverse'><a href='next.html'>Open</a><span>Alpha</span><a href='#target'>Jump</a></div>\
            <form><div class=row><label for=q>Query</label><input id=q name=q value=seed size=10><button type=reset>Reset</button></div></form>\
            {}<div class='row reverse'><p id=target>Target</p><p>Neighbor</p></div>{}",
            "<p>Before</p>".repeat(20), "<p>After</p>".repeat(20)));
        files.write("next.html", "<title>Next flex page</title><p>Arrived</p>");
        let mut app = App::new();
        app.open(&file);
        let node = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.name == "q")
            .unwrap()
            .0;
        for (width, zoom) in [(900, 1.0), (460, 1.5)] {
            app.resize(width, 650);
            app.set_zoom(zoom);
            app.scroll = 0;
            let field = app
                .page
                .controls
                .iter()
                .find(|c| c.node == node)
                .unwrap()
                .clone();
            let (x, y) = (field.x + 8, field.y + app.content_top() + 8);
            app.pointer_down(x, y, false);
            app.pointer_up(x, y);
            assert!(app.focus == Focus::Control(node));
            app.key(0x41, true, false, false);
            app.paste("updated");
            assert_eq!(app.forms.controls[&node].value.text(), "updated");
            app.key(0x09, false, false, false);
            let reset = *app
                .forms
                .controls
                .iter()
                .find(|(_, c)| c.kind == Kind::Reset)
                .unwrap()
                .0;
            assert!(app.focus == Focus::Control(reset));
            let rect = app
                .page
                .controls
                .iter()
                .find(|c| c.node == reset)
                .unwrap()
                .clone();
            let (x, y) = (rect.x + 5, rect.y + app.content_top() - app.scroll + 5);
            app.pointer_down(x, y, false);
            app.pointer_up(x, y);
            assert_eq!(app.forms.controls[&node].value.text(), "seed");
            app.search("Alpha");
            assert_eq!(app.matches.len(), 1);
            app.selection = app.matches.first().copied();
            assert_eq!(app.selected_text(), "Alpha");
            app.key(0x1b, false, false, false);
        }
        app.scroll = 0;
        let jump = app.page.runs.iter().find(|r| r.text == "Jump").unwrap();
        let (x, y) = (jump.x + 2, jump.y + app.content_top() + 2);
        assert!(app.wants_hand(x, y));
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        let target = app
            .page
            .anchors
            .iter()
            .find(|(name, _)| name == "target")
            .unwrap()
            .1;
        assert!(app.scroll > 0);
        assert!(
            (0..=16).contains(&(target - app.scroll)),
            "anchor should remain just below the viewport top"
        );
        let text = app.page.runs.iter().find(|r| r.text == "Target").unwrap();
        assert!(
            (text.y - target).abs() < 64,
            "moved anchor must follow its text"
        );
        app.scroll = 0;
        let link = app.page.runs.iter().find(|r| r.text == "Open").unwrap();
        let (x, y) = (link.x + 2, link.y + app.content_top() + 2);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        assert_eq!(app.document.title, "Next flex page");
    }

    #[test]
    fn positioned_controls_links_find_and_history_follow_the_visible_geometry() {
        let files = Fixture::new();
        let markup = format!(
            "<title>Positioned</title><style>body{{margin:16px}} .stage{{position:relative;height:160px}} .absolute{{position:absolute;left:30px;top:24px;width:180px;background:#ccefe0}} .pin{{position:fixed;right:8px;bottom:8px;width:240px;height:160px;background:#17394a;color:white}} .pin p{{margin:0}} </style><div class=stage><div class=absolute><a href='next.html'>Absolute</a></div></div><p>Before</p><div style='position:absolute;left:20px;top:200px'>Middle</div><p>After</p>{}<h2 id=target>Destination</h2>{}<div class=pin id=pin><p><a href='#target'>Pinned</a></p><form><input name=q value=original><button type=reset>Reset</button></form></div>",
            "<p>Normal document flow.</p>".repeat(25),
            "<p>More normal document flow.</p>".repeat(20),
        );
        let file = files.write("position.html", &markup);
        files.write(
            "next.html",
            "<title>Next positioned page</title><p>Arrived</p>",
        );
        let mut app = App::new();
        app.open(&file);
        app.resize(780, 620);
        let node = *app
            .forms
            .controls
            .iter()
            .find(|(_, c)| c.name == "q")
            .unwrap()
            .0;
        assert!(app.page.control_is_fixed(node));
        let pin_y =
            |app: &App| app.page.runs.iter().find(|r| r.text == "Pinned").unwrap().y - app.scroll;
        let initial_y = pin_y(&app);
        assert_eq!(initial_y, app.viewport_height() - 8 - 160);
        let document_height = app.page.height;
        for scroll in [300, 700, 0, 400] {
            app.set_scroll(scroll);
            assert_eq!(pin_y(&app), initial_y);
            assert_eq!(app.page.height, document_height);
            let rect = app.page.controls.iter().find(|r| r.node == node).unwrap();
            let (x, y) = (rect.x + 8, rect.y - app.scroll + app.content_top() + 8);
            app.pointer_down(x, y, false);
            app.pointer_up(x, y);
            assert!(app.focus == Focus::Control(node));
            assert_eq!(app.scroll, scroll);
            app.key(0x41, true, false, false);
            app.paste("updated");
            assert_eq!(app.forms.controls[&node].value.text(), "updated");
            let reset = app
                .forms
                .controls
                .iter()
                .find(|(_, c)| c.kind == Kind::Reset)
                .unwrap()
                .0;
            let rect = app.page.controls.iter().find(|r| r.node == *reset).unwrap();
            let (x, y) = (rect.x + 5, rect.y - app.scroll + app.content_top() + 5);
            app.pointer_down(x, y, false);
            app.pointer_up(x, y);
            assert_eq!(app.forms.controls[&node].value.text(), "original");
        }
        app.focus = Focus::Page;
        app.search("Pinned");
        assert_eq!(app.scroll, 400);
        assert_eq!(pin_y(&app), app.viewport_height() - 8 - 160);
        app.selection = app.matches.first().copied();
        assert_eq!(app.selected_text(), "Pinned");
        app.key(0x1b, false, false, false);
        assert_eq!(pin_y(&app), initial_y);
        app.follow("#pin");
        assert_eq!(
            app.scroll, 400,
            "an already visible fixed anchor must not scroll the document"
        );
        app.resize(780, 480);
        assert_eq!(pin_y(&app), app.viewport_height() - 8 - 160);
        app.set_zoom(1.5);
        assert_eq!(pin_y(&app), app.viewport_height() - 12 - 240);
        let link = app.page.runs.iter().find(|r| r.text == "Pinned").unwrap();
        let (x, y) = (link.x + 2, link.y - app.scroll + app.content_top() + 2);
        assert!(app.wants_hand(x, y));
        let point = app.point_at(x, y).unwrap();
        assert_eq!(app.page.runs[point.run].text, "Pinned");
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        let target = app
            .page
            .anchors
            .iter()
            .find(|(id, _)| id == "target")
            .unwrap()
            .1;
        assert!((0..=16).contains(&(target - app.scroll)));
        app.set_scroll(0);
        let absolute = app.page.runs.iter().find(|r| r.text == "Absolute").unwrap();
        let (x, y) = (absolute.x + 2, absolute.y + app.content_top() + 2);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        assert_eq!(app.document.title, "Next positioned page");
        app.history_move(-1);
        assert_eq!(app.document.title, "Positioned");
        assert_eq!(app.scroll, 0);
        assert_eq!(pin_y(&app), app.viewport_height() - 12 - 240);
        let words: Vec<_> = app.page.runs.iter().map(|r| r.text.as_str()).collect();
        let before = words.iter().position(|&word| word == "Before").unwrap();
        let middle = words.iter().position(|&word| word == "Middle").unwrap();
        let after = words.iter().position(|&word| word == "After").unwrap();
        assert!(
            before < middle && middle < after,
            "copy/find order must remain source order"
        );
    }

    #[test]
    fn jpeg_assets_load_through_the_resource_loader_and_keep_image_links() {
        let files = Fixture::new();
        std::fs::write(
            files.0.join("baseline.jpg"),
            include_bytes!("../pages/images/jpeg-landscape.jpg"),
        )
        .unwrap();
        std::fs::write(
            files.0.join("progressive.jpg"),
            include_bytes!("../pages/images/jpeg-progressive.jpg"),
        )
        .unwrap();
        std::fs::write(
            files.0.join("oriented.jpg"),
            include_bytes!("../pages/images/jpeg-oriented.jpg"),
        )
        .unwrap();
        let file = files.write("jpeg.html", "<title>JPEGs</title><a href=next.html><img src=baseline.jpg width=350></a><img src=progressive.jpg width=350><img src=oriented.jpg>");
        files.write("next.html", "<title>JPEG link</title><p>Arrived</p>");
        let mut app = App::new();
        app.open(&file);
        assert_eq!(app.images.len(), 3);
        assert!(app
            .events
            .iter()
            .filter(|e| e.starts_with("IMAGE "))
            .all(|e| !e.contains("SKIPPED")));
        let oriented = app
            .document
            .nodes
            .iter()
            .enumerate()
            .find(|(i, _)| app.document.attr(*i, "src") == Some("oriented.jpg"))
            .unwrap()
            .0;
        assert_eq!(
            (app.images[&oriented].width, app.images[&oriented].height),
            (320, 180)
        );
        let (x, y) = app
            .page
            .items
            .iter()
            .find_map(|item| match item {
                PaintItem::Image { x, y, .. } => Some((*x + 10, *y + 10 + app.content_top())),
                _ => None,
            })
            .unwrap();
        assert!(app.wants_hand(x, y));
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
        assert_eq!(app.document.title, "JPEG link");
    }

    #[test]
    fn fine_wheel_zoom_and_resize_keep_scroll_bounded() {
        let mut app = App::new();
        app.resize(640, 400);
        for _ in 0..120 {
            app.wheel(-1, false);
        }
        assert_eq!(app.scroll, 72);
        app.scroll_by(i32::MAX);
        assert_eq!(app.scroll, app.max_scroll());
        app.set_zoom(3.0);
        assert!(app.scroll <= app.max_scroll());
        app.resize(1200, 800);
        assert!(app.scroll <= app.max_scroll());
        app.scroll_by(i32::MIN);
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn cut_waits_for_clipboard_success_and_empty_copy_is_a_noop() {
        let mut app = App::new();
        assert!(matches!(
            app.key(0x43, true, false, false),
            PlatformAction::None
        ));
        app.focus_address();
        app.address.set("Keep this");
        app.address.select_all();
        match app.key(0x58, true, false, false) {
            PlatformAction::Cut(text) => {
                assert_eq!(app.address.text(), "Keep this");
                app.cut_copied(&text);
                assert_eq!(app.address.text(), "");
            }
            _ => panic!("Expected deferred cut"),
        }
    }

    #[test]
    fn find_and_copy_reconstruct_wrapped_words_and_inline_styles() {
        let f = Fixture::new();
        let long = "abcdefghijklmnopqrstuvxyz".repeat(5);
        let path = f.write(
            "index.html",
            &format!(
                "<p>{}</p><p>to<strong style='font-size:32px'>get</strong>her again</p>",
                long
            ),
        );
        let mut app = App::new();
        app.open(&path);
        app.resize(280, 400);
        app.search(&long);
        assert_eq!(app.matches.len(), 1);
        app.selection = app.matches.first().copied();
        assert_eq!(app.selected_text(), long);
        app.search("together");
        assert_eq!(app.matches.len(), 1);
        app.selection = app.matches.first().copied();
        assert_eq!(app.selected_text(), "together");
    }
}

#[cfg(test)]
#[path="app_media_query_tests.rs"]
mod media_query_tests;

#[cfg(test)]
#[path="app_child_nodes_tests.rs"]
mod child_nodes_tests;

#[cfg(test)]
#[path="app_comment_tests.rs"]
mod comment_tests;

#[cfg(test)]
#[path = "app_select_fragment_tests.rs"]
mod select_fragment_tests;

#[cfg(test)]
#[path="app_rgba_border_tests.rs"]
mod rgba_border_tests;

#[cfg(test)]
#[path="app_css_background_tests.rs"]
mod css_background_tests;

#[cfg(test)]
#[path="app_css_color_tests.rs"]
mod css_color_tests;

#[cfg(test)]
#[path="app_css_font_size_tests.rs"]
mod css_font_size_tests;

#[cfg(test)]
#[path="app_native_padding_tests.rs"]
mod native_padding_tests;

#[cfg(test)]
#[path="app_native_padding_detail_tests.rs"]
mod native_padding_detail_tests;

#[cfg(test)]
#[path = "app_computed_custom_style_tests.rs"]
mod computed_custom_style_tests;

#[cfg(test)]
#[path = "app_overflow_axes_tests.rs"]
mod overflow_axes_tests;

#[cfg(test)]
#[path = "app_offset_parent_tests.rs"]
mod offset_parent_tests;

#[cfg(test)]
#[path = "app_offset_coordinate_tests.rs"]
mod offset_coordinate_tests;

#[cfg(test)]
#[path = "app_translate_transform_tests.rs"]
mod translate_transform_tests;

#[cfg(test)]
#[path = "app_scroll_offset_tests.rs"]
mod scroll_offset_tests;

#[cfg(test)]
#[path = "app_computed_position_tests.rs"]
mod computed_position_tests;
#[cfg(test)]
#[path = "app_computed_opacity_tests.rs"]
mod computed_opacity_tests;
#[cfg(test)]
#[path = "app_opacity_paint_tests.rs"]
mod opacity_paint_tests;
#[cfg(test)]
#[path="app_stylesheet_cache_tests.rs"]
mod stylesheet_cache_tests;

#[cfg(test)]
#[path="app_element_sibling_tests.rs"]
mod element_sibling_tests;

#[cfg(test)]
#[path="app_document_fragment_tests.rs"]
mod document_fragment_tests;

#[cfg(test)]
#[path = "app_literal_prototype_tests.rs"]
mod literal_prototype_tests;

#[cfg(test)]
#[path = "app_string_concat_tests.rs"]
mod string_concat_tests;
#[cfg(test)]
#[path = "app_string_last_index_utf16_tests.rs"]
mod string_last_index_utf16_tests;

#[cfg(test)]
#[path = "app_hidden_input_tests.rs"]
mod hidden_input_tests;

#[cfg(test)]
#[path = "app_css_variable_tests.rs"]
mod css_variable_tests;

#[cfg(test)]
#[path = "app_css_radius_tests.rs"]
mod css_radius_tests;

#[cfg(test)]
#[path = "app_css_line_height_tests.rs"]
mod css_line_height_tests;

#[cfg(test)]
#[path = "app_css_root_selector_tests.rs"]
mod css_root_selector_tests;

#[cfg(test)]
#[path = "app_rounded_border_tests.rs"]
mod rounded_border_tests;

#[cfg(test)]
#[path = "app_background_clip_tests.rs"]
mod background_clip_tests;

#[cfg(test)]
#[path = "app_text_input_tests.rs"]
mod text_input_tests;

#[cfg(test)]
#[path = "app_created_button_tests.rs"]
mod created_button_tests;

#[cfg(test)]
#[path="app_checkbox_move_tests.rs"]
mod checkbox_move_tests;
#[cfg(test)]
#[path = "app_input_checkedness_tests.rs"]
mod input_checkedness_tests;

#[cfg(test)]
#[path = "app_base_class_tests.rs"]
mod base_class_tests;
#[cfg(test)]
#[path = "app_derived_class_tests.rs"]
mod derived_class_tests;
#[cfg(test)]
#[path = "app_document_properties_tests.rs"]
mod document_properties_tests;

#[cfg(test)]
#[path = "app_range_tests.rs"]
mod range_tests;

#[cfg(test)]
#[path="app_xhr_tests.rs"]
mod xhr_tests;

#[cfg(test)]
#[path="app_animation_event_tests.rs"]
mod animation_event_tests;

#[cfg(test)]
#[path="app_iframe_sandbox_tests.rs"]
mod iframe_sandbox_tests;

#[cfg(test)]
#[path="app_wheel_event_tests.rs"]
mod wheel_event_tests;

#[cfg(test)]
#[path="app_nested_template_tests.rs"]
mod nested_template_tests;

#[cfg(test)]
#[path="app_error_event_tests.rs"]
mod error_event_tests;

#[cfg(test)]
#[path="app_inert_selector_tests.rs"]
mod inert_selector_tests;

#[cfg(test)]
#[path="app_passive_html_import_tests.rs"]
mod passive_html_import_tests;

#[cfg(test)]
#[path="app_native_control_import_tests.rs"]
mod native_control_import_tests;

#[cfg(test)]
#[path="app_style_import_tests.rs"]
mod style_import_tests;

#[cfg(test)]
#[path="app_template_main_import_tests.rs"]
mod template_main_import_tests;

#[cfg(test)]
#[path="app_dynamic_svg_tests.rs"]
mod dynamic_svg_tests;

#[cfg(test)]
#[path="app_inert_observation_tests.rs"]
mod inert_observation_tests;

#[cfg(test)]
#[path="app_content_observation_tests.rs"]
mod content_observation_tests;

#[cfg(test)]
#[path="app_attribute_notification_tests.rs"]
mod attribute_notification_tests;

#[cfg(test)]
#[path="app_style_slot_notification_tests.rs"]
mod style_slot_notification_tests;

#[cfg(test)]
#[path="app_transient_observation_tests.rs"]
mod transient_observation_tests;

#[cfg(test)]
#[path="app_placeholder_tests.rs"]
mod placeholder_tests;

#[cfg(test)]
#[path="app_layout_variable_tests.rs"]
mod layout_variable_tests;

#[cfg(test)]
#[path="app_computed_presentation_tests.rs"]
mod computed_presentation_tests;

#[cfg(test)]
#[path="app_sizing_math_tests.rs"]
mod sizing_math_tests;

#[cfg(test)]
#[path="app_viewport_sizing_tests.rs"]
mod viewport_sizing_tests;

#[cfg(test)]
#[path="app_css_text_tests.rs"]
mod css_text_tests;

#[cfg(test)]
#[path="app_inline_flex_tests.rs"]
mod inline_flex_tests;

#[cfg(test)]
#[path="app_structural_selector_tests.rs"]
mod structural_selector_tests;

#[cfg(test)]
#[path="app_native_focus_selector_tests.rs"]
mod native_focus_selector_tests;

#[cfg(test)]
#[path="app_native_hover_tests.rs"]
mod native_hover_tests;

#[path="app_native_hover.rs"]
mod native_hover;

#[cfg(test)]
#[path="app_visibility_hit_tests.rs"]
mod visibility_hit_tests;

#[cfg(test)]
#[path="app_stacking_tests.rs"]
mod stacking_tests;

#[cfg(test)]
#[path="app_p2_shell_package_tests.rs"]
mod p2_shell_package_tests;

#[cfg(test)]
#[path="app_html_reflector_tests.rs"]
mod html_reflector_tests;

#[cfg(test)]
#[path="app_aria_reflection_tests.rs"]
mod aria_reflection_tests;

#[cfg(test)]
#[path="app_inert_replace_tests.rs"]
mod inert_replace_tests;

#[cfg(test)]
#[path="app_replace_self_tests.rs"]
mod replacement_self_tests;

#[cfg(test)]
#[path="app_svg_geometry_tests.rs"]
mod svg_geometry_tests;

#[cfg(test)]
#[path="app_svg_cascade_paint_tests.rs"]
mod svg_cascade_paint_tests;

#[cfg(test)]
#[path="app_console_diagnostics_tests.rs"]
mod console_diagnostics_tests;

#[cfg(test)]
#[path="app_svg_shared_paint_tests.rs"]
mod svg_shared_paint_tests;

#[cfg(test)]
#[path="app_selected_component_tests.rs"]
mod selected_component_tests;

#[cfg(test)]
#[path="app_flat_behavior_tests.rs"]
mod flat_behavior_tests;

#[cfg(test)]
#[path="app_character_data_properties_tests.rs"]
mod character_data_properties_tests;

#[cfg(test)]
#[path="app_bigint_width_tests.rs"]
mod bigint_width_tests;

#[cfg(test)]
#[path="app_svg_author_properties_tests.rs"]
mod svg_author_properties_tests;

#[cfg(test)]
#[path="app_fragment_author_prototype_tests.rs"]
mod fragment_author_prototype_tests;

#[cfg(test)]
#[path="app_custom_host_content_tests.rs"]
mod custom_host_content_tests;

#[cfg(test)]
#[path="app_native_dom_string_tests.rs"]
mod native_dom_string_tests;

#[cfg(test)]
#[path="app_native_document_records_tests.rs"]
mod native_document_records_tests;

#[cfg(test)]
#[path="app_native_expanded_names_tests.rs"]
mod native_expanded_names_tests;

#[cfg(test)]
#[path="app_native_xml_document_tests.rs"]
mod native_xml_document_tests;

#[cfg(test)]
#[path="app_native_cdata_tests.rs"]
mod native_cdata_tests;

#[cfg(test)]
#[path="app_xml_doctype_adoption_tests.rs"]
mod xml_doctype_adoption_tests;

#[cfg(test)]
#[path="app_document_children_tests.rs"]
mod document_children_tests;

#[cfg(test)]
#[path="app_document_clone_tests.rs"]
mod document_clone_tests;

#[cfg(test)]
#[path="app_node_adoption_tests.rs"]
mod node_adoption_tests;

#[cfg(test)]
#[path="app_inert_direct_insertion_tests.rs"]
mod inert_direct_insertion_tests;

#[cfg(test)]
#[path="app_xml_factory_root_identity_tests.rs"]
mod xml_factory_root_identity_tests;

#[cfg(test)]
#[path="app_domparser_node_identity_tests.rs"]
mod domparser_node_identity_tests;

#[cfg(test)]
#[path="app_inert_attribute_factory_tests.rs"]
mod inert_attribute_factory_tests;

#[cfg(test)]
#[path="app_inert_attribute_attachment_tests.rs"]
mod inert_attribute_attachment_tests;

#[cfg(test)]
#[path="app_inert_foreign_attribute_tests.rs"]
mod inert_foreign_attribute_tests;

#[cfg(test)]
#[path="app_attribute_namespace_tests.rs"]
mod attribute_namespace_tests;

#[cfg(test)]
#[path="app_attribute_name_tests.rs"]
mod attribute_name_tests;

#[cfg(test)]
#[path="app_attribute_clone_tests.rs"]
mod attribute_clone_tests;

#[cfg(test)]
#[path="app_element_name_units_tests.rs"]
mod element_name_units_tests;

#[cfg(test)]
#[path="app_attribute_filter_units_tests.rs"]
mod attribute_filter_units_tests;

#[cfg(test)]
#[path="app_loaded_clone_snapshot_tests.rs"]
mod loaded_clone_snapshot_tests;

#[cfg(test)]
#[path="app_inert_document_import_tests.rs"]
mod inert_document_import_tests;

#[cfg(test)]
#[path="app_template_document_owner_tests.rs"]
mod template_document_owner_tests;

#[cfg(test)]
#[path="app_inert_template_import_tests.rs"]
mod inert_template_import_tests;

#[cfg(test)]
#[path="app_generic_document_tests.rs"]
mod generic_document_tests;

#[cfg(test)]
#[path="app_document_template_clone_tests.rs"]
mod document_template_clone_tests;

#[cfg(test)]
#[path="app_template_document_children_tests.rs"]
mod template_document_children_tests;

#[cfg(test)]
#[path="app_node_namespace_lookup_tests.rs"]
mod node_namespace_lookup_tests;

#[cfg(test)]
#[path="app_node_equality_tests.rs"]
mod node_equality_tests;

#[cfg(test)]
#[path="app_inert_position_tests.rs"]
mod inert_position_tests;

#[cfg(test)]
#[path="app_inert_attribute_units_tests.rs"]
mod inert_attribute_units_tests;

#[cfg(test)]
#[path="app_identifier_lookup_tests.rs"]
mod identifier_lookup_tests;

#[cfg(test)]
#[path="app_pi_domstring_validation_tests.rs"]
mod pi_domstring_validation_tests;

#[cfg(test)]
#[path="app_character_substring_tests.rs"]
mod character_substring_tests;

#[cfg(test)]
#[path="app_character_splice_tests.rs"]
mod character_splice_tests;

#[cfg(test)]
#[path="app_text_whole_tests.rs"]
mod text_whole_tests;

#[cfg(test)]
#[path="app_text_split_tests.rs"]
mod text_split_tests;

#[cfg(test)]
#[path="app_shared_normalize_tests.rs"]
mod shared_normalize_tests;

#[cfg(test)]
#[path = "app_string_index_utf16_tests.rs"]
mod string_index_utf16_tests;

#[cfg(test)]
#[path = "app_string_box_utf16_tests.rs"]
mod string_box_utf16_tests;

#[cfg(test)]
#[path = "app_reflect_own_keys_tests.rs"]
mod reflect_own_keys_tests;

#[cfg(test)]
#[path = "app_from_char_code_utf16_tests.rs"]
mod from_char_code_utf16_tests;

#[cfg(test)]
#[path = "app_character_position_tests.rs"]
mod character_position_tests;

#[cfg(test)]
#[path = "app_trim_utf16_tests.rs"]
mod trim_utf16_tests;

#[cfg(test)]
#[path = "app_token_list_exact_reads_tests.rs"]
mod token_list_exact_reads_tests;

#[cfg(test)]
#[path = "app_token_list_interface_tests.rs"]
mod token_list_interface_tests;

#[cfg(test)]
#[path="app_css_class_exact_tests.rs"]
mod css_class_exact_tests;

#[cfg(test)]
#[path="app_selector_ident_start_tests.rs"]
mod selector_ident_start_tests;

#[cfg(test)]
#[path="app_html_class_domstring_tests.rs"]
mod html_class_domstring_tests;

#[cfg(test)]
#[path="app_token_list_exact_mutation_tests.rs"]
mod token_list_exact_mutation_tests;

#[cfg(test)]
#[path="app_token_list_toggle_tests.rs"]
mod token_list_toggle_tests;

#[cfg(test)]
#[path="app_token_list_replace_tests.rs"]
mod token_list_replace_tests;

#[cfg(test)]
#[path="app_token_list_for_each_tests.rs"]
mod token_list_for_each_tests;

#[cfg(test)]
#[path="app_node_list_for_each_tests.rs"]
mod node_list_for_each_tests;

#[cfg(test)]
#[path="app_node_list_item_tests.rs"]
mod node_list_item_tests;

#[cfg(test)]
#[path="app_html_title_domstring_tests.rs"]
mod html_title_domstring_tests;

#[cfg(test)]
#[path="app_html_language_domstring_tests.rs"]
mod html_language_domstring_tests;

#[cfg(test)]
#[path="app_css_id_exact_tests.rs"]
mod css_id_exact_tests;

#[cfg(test)]
#[path="app_collection_named_exact_tests.rs"]
mod collection_named_exact_tests;

#[cfg(test)]
#[path="app_visual_id_exact_tests.rs"]
mod visual_id_exact_tests;

#[cfg(test)]
#[path="app_form_named_exact_tests.rs"]
mod form_named_exact_tests;

#[cfg(test)]
#[path="app_element_id_accessor_tests.rs"]
mod element_id_accessor_tests;

#[cfg(test)]
#[path="app_form_owner_id_exact_tests.rs"]
mod form_owner_id_exact_tests;

#[cfg(test)]
#[path="app_html_id_domstring_tests.rs"]
mod html_id_domstring_tests;

#[cfg(test)]
#[path="app_html_id_materialization_tests.rs"]
mod html_id_materialization_tests;

#[cfg(test)]
#[path="app_live_name_domstring_tests.rs"]
mod live_name_domstring_tests;

#[cfg(test)]
#[path="app_document_name_method_tests.rs"]
mod document_name_method_tests;

#[cfg(test)]
#[path="app_owned_name_list_tests.rs"]
mod owned_name_list_tests;

#[cfg(test)]
#[path="app_passive_name_domstring_tests.rs"]
mod passive_name_domstring_tests;

#[cfg(test)]
#[path="app_passive_name_materialization_tests.rs"]
mod passive_name_materialization_tests;

#[cfg(test)]
#[path="app_html_metadata_materialization_tests.rs"]
mod html_metadata_materialization_tests;

#[cfg(test)]
#[path="app_element_class_name_accessor_tests.rs"]
mod element_class_name_accessor_tests;

#[cfg(test)]
#[path="app_element_class_list_accessor_tests.rs"]
mod element_class_list_accessor_tests;

#[cfg(test)]
#[path="app_inert_html_class_list_tests.rs"]
mod inert_html_class_list_tests;

#[cfg(test)]
#[path="app_inert_element_properties_tests.rs"]
mod inert_element_properties_tests;

#[cfg(test)]
#[path="app_inert_foreign_class_list_tests.rs"]
mod inert_foreign_class_list_tests;

#[cfg(test)]
#[path="app_inert_reflection_namespace_tests.rs"]
mod inert_reflection_namespace_tests;

#[cfg(test)]
#[path="app_aria_domstring_reflection_tests.rs"]
mod aria_domstring_reflection_tests;

#[cfg(test)]
#[path="app_html_aria_domstring_tests.rs"]
mod html_aria_domstring_tests;

#[cfg(test)]
#[path="app_html_label_for_domstring_tests.rs"]
mod html_label_for_domstring_tests;

#[cfg(test)]
#[path="app_html_label_for_accessor_tests.rs"]
mod html_label_for_accessor_tests;

#[cfg(test)]
#[path="app_html_label_control_tests.rs"]
mod html_label_control_tests;

#[cfg(test)]
#[path="app_html_label_form_tests.rs"]
mod html_label_form_tests;

#[cfg(test)]
#[path="app_html_input_labels_tests.rs"]
mod html_input_labels_tests;

#[cfg(test)]
#[path="app_button_textarea_labels_tests.rs"]
mod button_textarea_labels_tests;

#[cfg(test)]
#[path="app_remaining_builtin_labels_tests.rs"]
mod remaining_builtin_labels_tests;

#[cfg(test)]
#[path="app_output_for_domstring_tests.rs"]
mod output_for_domstring_tests;

#[cfg(test)]
#[path="app_output_html_for_tests.rs"]
mod output_html_for_tests;

#[cfg(test)]
#[path="app_output_form_tests.rs"]
mod output_form_tests;

#[cfg(test)]
#[path="app_output_name_type_tests.rs"]
mod output_name_type_tests;

#[cfg(test)]
#[path="app_output_text_replacement_tests.rs"]
mod output_text_replacement_tests;

#[cfg(test)]
#[path="app_descendant_text_tests.rs"]
mod descendant_text_tests;

#[cfg(test)]
#[path="app_output_value_default_tests.rs"]
mod output_value_default_tests;

#[cfg(test)]
#[path="app_output_reset_tests.rs"]
mod output_reset_tests;

#[cfg(test)]
#[path="app_output_form_attribute_tests.rs"]
mod output_form_attribute_tests;

#[cfg(test)]
#[path="app_output_validation_tests.rs"]
mod output_validation_tests;

#[cfg(test)]
#[path="app_output_validity_state_tests.rs"]
mod output_validity_state_tests;

#[cfg(test)]
#[path="app_control_form_accessor_tests.rs"]
mod control_form_accessor_tests;

#[cfg(test)]
#[path="app_control_type_tests.rs"]
mod control_type_tests;

#[cfg(test)]
#[path="app_form_length_tests.rs"]
mod form_length_tests;

#[cfg(test)]
#[path="app_fieldset_metadata_tests.rs"]
mod fieldset_metadata_tests;

#[cfg(test)]
#[path="app_fieldset_name_tests.rs"]
mod fieldset_name_tests;

#[cfg(test)]
#[path="app_fieldset_disabled_tests.rs"]
mod fieldset_disabled_tests;

#[cfg(test)]
#[path="app_fieldset_validation_tests.rs"]
mod fieldset_validation_tests;

#[cfg(test)]
#[path="app_fieldset_validity_tests.rs"]
mod fieldset_validity_tests;

#[cfg(test)]
#[path="app_fieldset_form_attribute_tests.rs"]
mod fieldset_form_attribute_tests;

#[cfg(test)]
#[path="app_legend_form_tests.rs"]
mod legend_form_tests;

#[cfg(test)]
#[path="app_progress_numeric_tests.rs"]
mod progress_numeric_tests;

#[cfg(test)]
#[path="app_meter_numeric_tests.rs"]
mod meter_numeric_tests;

#[cfg(test)]
#[path="app_gauge_lifecycle_tests.rs"]
mod gauge_lifecycle_tests;

#[cfg(test)]
#[path="app_time_datetime_tests.rs"]
mod time_datetime_tests;

#[cfg(test)]
#[path="app_data_value_tests.rs"]
mod data_value_tests;

#[cfg(test)]
#[path="app_quote_interface_tests.rs"]
mod quote_interface_tests;

#[cfg(test)]
#[path="app_heading_interface_tests.rs"]
mod heading_interface_tests;

#[cfg(test)]
#[path="app_heading_align_tests.rs"]
mod heading_align_tests;

#[cfg(test)]
#[path="app_div_align_tests.rs"]
mod div_align_tests;

#[cfg(test)]
#[path="app_legend_align_tests.rs"]
mod legend_align_tests;

#[cfg(test)]
#[path="app_legend_inert_import_tests.rs"]
mod legend_inert_import_tests;

#[cfg(test)]
#[path="app_legend_native_copy_tests.rs"]
mod legend_native_copy_tests;

#[cfg(test)]
#[path="app_paragraph_align_tests.rs"]
mod paragraph_align_tests;

#[cfg(test)]
#[path="app_ul_type_tests.rs"]
mod ul_type_tests;

#[cfg(test)]
#[path="app_ul_compact_tests.rs"]
mod ul_compact_tests;

#[cfg(test)]
#[path="app_span_interface_tests.rs"]
mod span_interface_tests;

#[cfg(test)]
#[path="app_dl_compact_tests.rs"]
mod dl_compact_tests;

#[cfg(test)]
#[path="app_li_type_tests.rs"]
mod li_type_tests;

#[cfg(test)]
#[path="app_li_value_tests.rs"]
mod li_value_tests;

#[cfg(test)]
#[path="app_ol_type_tests.rs"]
mod ol_type_tests;

#[cfg(test)]
#[path="app_ol_reversed_tests.rs"]
mod ol_reversed_tests;

#[cfg(test)]
#[path="app_ol_start_tests.rs"]
mod ol_start_tests;

#[cfg(test)]
#[path="app_ol_compact_tests.rs"]
mod ol_compact_tests;

#[cfg(test)]
#[path="app_pre_width_tests.rs"]
mod pre_width_tests;

#[cfg(test)]
#[path="app_hr_align_tests.rs"]
mod hr_align_tests;

#[cfg(test)]
#[path="app_hr_color_tests.rs"]
mod hr_color_tests;

#[cfg(test)]
#[path="app_hr_no_shade_tests.rs"]
mod hr_no_shade_tests;

#[cfg(test)]
#[path="app_hr_size_tests.rs"]
mod hr_size_tests;

#[cfg(test)]
#[path="app_hr_width_tests.rs"]
mod hr_width_tests;

#[cfg(test)]
#[path="app_br_clear_tests.rs"]
mod br_clear_tests;

#[cfg(test)]
#[path="app_menu_compact_tests.rs"]
mod menu_compact_tests;

#[cfg(test)]
#[path="app_dir_compact_tests.rs"]
mod dir_compact_tests;

#[cfg(test)]
#[path="app_font_color_tests.rs"]
mod font_color_tests;

#[cfg(test)]
#[path="app_font_face_tests.rs"]
mod font_face_tests;

#[cfg(test)]
#[path="app_font_size_tests.rs"]
mod font_size_tests;

#[cfg(test)]
#[path="app_pre_alias_tests.rs"]
mod pre_alias_tests;

#[cfg(test)]
#[path="app_unknown_interface_tests.rs"]
mod unknown_interface_tests;

#[cfg(test)]
#[path="app_historical_html_tests.rs"]
mod historical_html_tests;

#[cfg(test)]
#[path="app_semantic_html_tests.rs"]
mod semantic_html_tests;
