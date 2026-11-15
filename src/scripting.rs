//! The deliberately small DOM boundary of the original JavaScript interpreter.
//! A Page is one document realm. It never opens files or performs network I/O.
//! Bounded navigation/resource requests are handed to the App after each job.
//! Its document-owned timer queue is pumped by the App;
//! callbacks never execute on a background thread.
//!
//! Classic scripts run once, in source order, after parsing. Event jobs share
//! both interpreter and host-work limits; dispatch never resets those limits.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::forms::{self, Forms, Kind};
use crate::html::{Document, Node, NodeKind};
use crate::javascript::{Host, Runtime, Value};

#[path = "script_promises.rs"]
mod script_promises;
use crate::loader::Script;

#[path = "script_dom_compat.rs"]
mod dom_compat;
use dom_compat::{dataset_attribute, dataset_key, navigator_readonly};
#[path = "script_queries.rs"]
mod queries;
use queries::node_list_index;
#[path = "script_collections.rs"]
mod dom_collections;
use dom_collections::collection_handle_index;
#[path = "script_dom_tree.rs"]
mod dom_tree;
#[path = "script_position.rs"]
mod script_position;
#[path = "script_timezone.rs"]
mod script_timezone;
use dom_tree::DomState;
#[path = "script_attr.rs"]
mod script_attr;
#[path = "script_attribute_names.rs"]
mod script_attribute_names;
#[path = "script_class_list.rs"]
mod script_class_list;
#[path = "script_named_map.rs"]
mod script_named_map;
#[path = "script_normalize.rs"]
mod script_normalize;
#[path = "script_element_properties.rs"]
mod script_element_properties;
#[path = "script_created_events.rs"]
mod script_created_events;
#[path = "script_dispatch.rs"]
mod script_dispatch;
#[path = "script_navigation.rs"]
mod page_navigation;
pub use page_navigation::Navigation;
#[path = "script_forms.rs"]
mod script_forms;
#[path = "script_resources.rs"]
mod script_resources;
pub use script_resources::DynamicScript;
use script_resources::DynamicState;
#[path = "script_blank_frames.rs"]
mod script_blank_frames;
#[path = "script_stylesheets.rs"]
mod script_stylesheets;
pub use script_stylesheets::DynamicStylesheet;
use script_stylesheets::StylesheetState;
#[path = "script_images.rs"]
mod script_images;
#[path = "script_media.rs"]
mod script_media;
pub(crate) use script_media::event_name as media_event_name;
pub use script_images::DynamicImage;
pub(crate) use script_images::MAX_IMAGE_REQUESTS;
use script_images::ImagesState;
#[path = "script_inner_html.rs"]
mod script_inner_html;
#[path = "script_history.rs"]
mod script_history;
pub use script_history::HistoryData;
use script_history::HistoryState;

const WINDOW: u32 = 1;
const DOCUMENT: u32 = 2;
const CONSOLE: u32 = 3;
const NAVIGATOR: u32 = 4;
const PLUGINS: u32 = 5;
const MIME_TYPES: u32 = 6;
const DOCUMENT_IMAGES: u32 = 7;
const LOCATION: u32 = 8;
const PERFORMANCE: u32 = 9;
const IMAGE_FACTORY: u32 = 10;
const HISTORY: u32 = 11;
const PERFORMANCE_TIMING:u32=12;
const NODE_LIST_BASE: u32 = 32;
const MAX_NODE_LISTS: usize = 128;
const MAX_NODE_LIST_REFERENCES: usize = 16_384;
const NODE_BASE: u32 = 1024;
const STYLE_BASE: u32 = 1_000_000;
const DATASET_BASE: u32 = 1_500_000;
const EVENT_BASE: u32 = 2_000_000;
#[cfg(test)]
const MAX_LISTENERS: usize = 64;
#[cfg(test)]
const MAX_EVENT_TYPE_BYTES: usize = 256;
#[cfg(test)]
const MAX_STRING: usize = 32 * 1024;
#[cfg(test)]
const MAX_EXTRA_NODES: usize = 256;
#[cfg(test)]
const MAX_EXTRA_BYTES: usize = 1024 * 1024;
const MAX_LOG_LINES: usize = 128;
const MAX_LOG_BYTES: usize = 32 * 1024;
const MAX_LOG_LINE: usize = 2048;
const MAX_HANDLER_BYTES: usize = 8192;
#[cfg(test)]
const MAX_TIMERS: usize = 64;
#[cfg(test)]
const MAX_TIMER_CALLBACKS: usize = 4096;
#[cfg(test)]
const MAX_ANIMATION_FRAMES: usize = 64;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
// A bounded UA policy, measured from actual qualifying native input. Script
// dispatch and resource completion cannot create or renew transient activation.
const TRANSIENT_ACTIVATION: Duration = Duration::from_secs(5);
pub const MAX_CONSOLE_SOURCE: usize = 8192;

/// Values supplied by the native input path, never by document code.
#[derive(Clone, Debug, Default)]
pub struct KeyboardInput {
    pub down: bool,
    pub target: Option<usize>,
    pub key: String,
    pub code: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    pub repeat: bool,
    pub location: u8,
    pub key_code: u32,
}

#[derive(Default, Debug)]
pub struct Outcome {
    pub prevented: bool,
    pub layout_changed: bool,
    pub errors: usize,
}

#[derive(Clone)]
struct Listener {
    serial: u32,
    target: u32,
    event: String,
    function: Value,
    property: bool,
    options: ListenerOptions,
}

#[derive(Clone, Copy, Default)]
struct ListenerOptions {
    capture: bool,
    once: bool,
    passive: bool,
}

struct Timer {
    id: u32,
    due: Instant,
    callback: Value,
    arguments: Vec<Value>,
    bytes: usize,
    nesting: u32,
}

struct AnimationFrame {
    id: u32,
    callback: Value,
}

struct Event {
    handle: u32,
    target: u32,
    current: u32,
    name: String,
    cancelable: bool,
    bubbles: bool,
    prevented: bool,
    stopped: bool,
    immediate: bool,
    phase: u8,
    passive: bool,
    keyboard: Option<KeyboardInput>,
    timestamp: f64,
}

#[derive(Default)]
struct Tree {
    active: Vec<bool>,
    parents: Vec<Option<usize>>,
    order: Vec<usize>,
    ids: HashMap<String, usize>,
    body: Option<usize>,
    head: Option<usize>,
    document_element: Option<usize>,
}

/// The App supplies the displayed document's committed location. These values
/// reflect its existing URL serialization; the content root and HTML base URL
/// are deliberately not part of this script-visible snapshot.
struct LocationView {
    source: crate::navigation::Location,
    href: String,
    protocol: String,
    host: String,
    hostname: String,
    port: String,
    pathname: String,
    search: String,
    hash: String,
    origin: String,
}

impl LocationView {
    fn new(location: &crate::navigation::Location) -> Self {
        let href = location.display();
        // Location::display encodes literal '#' characters in path/query data
        // and serializes the separately stored, decoded fragment exactly once.
        let fragment_start = href.find('#').unwrap_or(href.len());
        let hash = href[fragment_start..].to_owned();
        let (protocol, host, hostname, port, pathname, search, origin) =
            if let Some(url) = &location.url {
                let protocol = if url.secure { "https:" } else { "http:" }.to_owned();
                let host = url.host_header();
                let hostname = if url.host.contains(':') {
                    format!("[{}]", url.host)
                } else {
                    url.host.clone()
                };
                let port = if url.port == if url.secure { 443 } else { 80 } {
                    String::new()
                } else {
                    url.port.to_string()
                };
                let search = url
                    .query
                    .as_ref()
                    .filter(|query| !query.is_empty())
                    .map_or_else(String::new, |query| format!("?{query}"));
                let origin = format!("{protocol}//{host}");
                (
                    protocol,
                    host,
                    hostname,
                    port,
                    url.path.clone(),
                    search,
                    origin,
                )
            } else {
                let file = location.path.is_some();
                let pathname = if file {
                    // File locations have no network authority or query. Use
                    // the path spelling from the same serializer as href.
                    href["file://".len()..fragment_start].to_owned()
                } else {
                    "home".to_owned()
                };
                (
                    if file { "file:" } else { "about:" }.to_owned(),
                    String::new(),
                    String::new(),
                    String::new(),
                    pathname,
                    String::new(),
                    "null".to_owned(),
                )
            };
        Self {
            source: location.clone(),
            href,
            protocol,
            host,
            hostname,
            port,
            pathname,
            search,
            hash,
            origin,
        }
    }

    fn property(&self, property: &str) -> Option<&str> {
        Some(match property {
            "href" => &self.href,
            "protocol" => &self.protocol,
            "host" => &self.host,
            "hostname" => &self.hostname,
            "port" => &self.port,
            "pathname" => &self.pathname,
            "search" => &self.search,
            "hash" => &self.hash,
            "origin" => &self.origin,
            _ => return None,
        })
    }
}

struct State {
    blank_frames: Vec<script_blank_frames::BlankFrame>,
    blank_frame_nodes: usize,
    geometry: Option<crate::script_geometry::Geometry>,
    location: Option<LocationView>,
    navigation: Option<Navigation>,
    navigation_requests: usize,
    cookies: std::sync::Arc<crate::cookies::Session>,
    dom: DomState,
    dynamic: DynamicState,
    stylesheets: StylesheetState,
    images: ImagesState,
    history: HistoryState,
    listeners: Vec<Listener>,
    native_functions: HashMap<&'static str, Value>,
    native_methods: Vec<(u32, &'static str, Value)>,
    detached_images: HashSet<usize>,
    detached_nodes: HashSet<usize>,
    node_lists: Vec<queries::NodeList>,
    node_list_references: usize,
    attributes: Vec<script_attr::Attr>,
    declined_images: usize,
    declined_beacons: usize,
    eval_allowed: bool,
    timers: Vec<Timer>,
    animation_frames: VecDeque<AnimationFrame>,
    next_animation_frame: u32,
    animation_due: Option<Instant>,
    animation_callbacks: u64,
    clock_origin: Instant,
    navigation_timing:Option<crate::network_trace::DocumentTiming>,
    embedded_video:Option<crate::embedded_video::Snapshot>,
    media_changes:script_media::Changes,
    pending_keyboard: Option<KeyboardInput>,
    next_timer: u32,
    timer_bytes: usize,
    timer_callbacks: usize,
    timer_nesting: u32,
    next_listener: u32,
    next_event: u32,
    event: Option<Event>,
    created_events: Vec<script_created_events::CreatedEvent>,
    standalone_targets: Vec<u32>,
    synthetic_dispatches: Vec<script_dispatch::Dispatch>,
    #[cfg(test)]
    disable_event_factory: bool,
    tree: Tree,
    dirty: bool,
    // Committed DOM writes, including detached nodes; not native form values,
    // allocations, ordinary JS properties, layout work or resource completion.
    dom_commits: Option<u64>,
    limits: crate::javascript::limits::Limits,
    host_work: usize,
    copy_work: usize,
    node_limit: usize,
    retained: usize,
    retained_limit: usize,
    changed: bool,
    errors: usize,
    logs: Vec<String>,
    log_bytes: usize,
    omitted: usize,
    executed: usize,
    enabled: bool,
    ready: &'static str,
    // readyState becomes complete BEFORE the load callbacks/checkpoints.
    document_complete: bool,
    last_activation: Option<Instant>,
    navigation_incumbent: Option<u32>,
}

impl State {
    fn new() -> Self {
        Self {
            blank_frames: Vec::new(),
            blank_frame_nodes: 0,
            geometry: None,
            location: None,
            navigation: None,
            navigation_requests: 0,
            cookies: std::sync::Arc::new(crate::cookies::Session::default()),
            dom: DomState::default(),
            dynamic: DynamicState::default(),
            stylesheets: StylesheetState::default(),
            images: ImagesState::default(),
            history: HistoryState::default(),
            listeners: Vec::new(),
            native_functions: HashMap::new(),
            native_methods: Vec::new(),
            detached_images: HashSet::new(),
            detached_nodes: HashSet::new(),
            node_lists: Vec::new(),
            node_list_references: 0,
            attributes: Vec::new(),
            declined_images: 0,
            declined_beacons: 0,
            eval_allowed: false,
            timers: Vec::new(),
            animation_frames: VecDeque::new(),
            next_animation_frame: 0,
            animation_due: None,
            animation_callbacks: 0,
            clock_origin: Instant::now(),
            navigation_timing:None,
            embedded_video:None,
            media_changes:script_media::Changes::default(),
            pending_keyboard: None,
            next_timer: 0,
            timer_bytes: 0,
            timer_callbacks: 0,
            timer_nesting: 0,
            next_listener: 0,
            next_event: EVENT_BASE,
            event: None,
            created_events: Vec::new(),
            standalone_targets: Vec::new(),
            synthetic_dispatches: Vec::new(),
            #[cfg(test)]
            disable_event_factory: false,
            tree: Tree::default(),
            dirty: true,
            dom_commits: Some(0),
            limits: crate::javascript::limits::BROWSER,
            host_work: crate::javascript::limits::BROWSER.host_work,
            copy_work: crate::javascript::limits::BROWSER.copy_bytes,
            node_limit: 0,
            retained: 0,
            retained_limit: crate::javascript::limits::BROWSER.host.retained_bytes,
            changed: false,
            errors: 0,
            logs: Vec::new(),
            log_bytes: 0,
            omitted: 0,
            executed: 0,
            enabled: false,
            ready: "loading",
            document_complete: false,
            last_activation: None,
            navigation_incumbent: Some(WINDOW),
        }
    }

    fn activation_is_live(last: Option<Instant>, now: Instant) -> bool {
        last.is_some_and(|last| {
            now.checked_duration_since(last).is_some_and(|age| age < TRANSIENT_ACTIVATION)
        })
    }

    fn has_transient_activation(&self, now: Instant) -> bool {
        Self::activation_is_live(self.last_activation, now)
    }

    fn dom_committed(&mut self) {
        self.dom_commits = self.dom_commits.and_then(|value| value.checked_add(1));
    }

    fn layout_changed(&mut self) {
        self.changed = true;
        if let Some(geometry) = &mut self.geometry {
            geometry.invalidate();
        }
    }

    fn work(&mut self, count: usize) -> Result<(), String> {
        if count > self.host_work {
            self.host_work = 0;
            return Err(format!("DOM work limit reached ({} node/host operations per job).", self.limits.host_work));
        }
        self.host_work -= count;
        Ok(())
    }

    fn copies(&mut self, bytes: usize) -> Result<(), String> {
        if bytes > self.copy_work {
            self.copy_work = 0;
            return Err(format!("DOM string work limit reached ({} MiB per job).", self.limits.copy_bytes / (1024 * 1024)));
        }
        self.copy_work -= bytes;
        Ok(())
    }

    fn set_timeout(&mut self, arguments: &[Value]) -> Result<Value, String> {
        let Some(callback @ Value::Function(_)) = arguments.first() else {
            return Err(
                "setTimeout requires a function; string/eval timers are not supported.".into(),
            );
        };
        if self.timers.len() >= self.limits.host.timers {
            return Err("Timer limit reached (64 pending callbacks per document).".into());
        }
        if self.timer_callbacks >= self.limits.host.timer_callbacks {
            return Err(
                "Timer callback limit reached (4096 per document). Reload starts a new allowance."
                    .into(),
            );
        }
        let extra = arguments.get(2..).unwrap_or(&[]);
        if extra.len() > self.limits.host.timer_arguments {
            return Err("Timer argument limit reached (30).".into());
        }
        let bytes = extra
            .iter()
            .try_fold(std::mem::size_of_val(extra), |total, value| {
                total.checked_add(match value {
                    Value::String(text) => text.len(),
                    _ => 0,
                })
            })
            .ok_or("Timer argument size overflow.")?;
        if bytes > self.limits.host.timer_bytes.saturating_sub(self.timer_bytes) {
            return Err("Timer argument memory limit reached (256 KiB per document).".into());
        }
        self.copies(bytes)?;
        let delay_value = arguments.get(1).unwrap_or(&Value::Undefined);
        if let Value::String(text) = delay_value {
            self.copies(text.len().saturating_mul(3))?;
        }
        let delay = timer_integer(delay_value)?.max(0) as u64;
        let delay = if self.timer_nesting > 5 {
            delay.max(4)
        } else {
            delay
        };
        let due = Instant::now()
            .checked_add(Duration::from_millis(delay))
            .ok_or("Timer deadline overflow.")?;
        let id = self
            .next_timer
            .checked_add(1)
            .filter(|id| *id <= i32::MAX as u32)
            .ok_or("Timer identifier limit reached.")?;
        self.next_timer = id;
        self.timer_bytes += bytes;
        self.timers.push(Timer {
            id,
            due,
            callback: callback.clone(),
            arguments: extra.to_vec(),
            bytes,
            nesting: self.timer_nesting.saturating_add(1),
        });
        Ok(Value::Number(f64::from(id)))
    }

    fn clear_timeout(&mut self, arguments: &[Value]) -> Result<Value, String> {
        let id_value = arguments.first().unwrap_or(&Value::Undefined);
        if let Value::String(text) = id_value {
            self.copies(text.len().saturating_mul(3))?;
        }
        let id = timer_integer(id_value)?;
        if let Some(index) = self
            .timers
            .iter()
            .position(|timer| id > 0 && timer.id == id as u32)
        {
            let timer = self.timers.remove(index);
            self.timer_bytes -= timer.bytes;
        }
        Ok(Value::Undefined)
    }

    fn clock_ms(&self, now: Instant) -> f64 {
        // A real monotonic realm clock, deliberately rounded down to whole
        // milliseconds. No epoch time or high-resolution device clock leaks.
        now.saturating_duration_since(self.clock_origin).as_millis() as f64
    }

    fn request_animation_frame(&mut self, arguments: &[Value]) -> Result<Value, String> {
        let Some(callback @ Value::Function(_)) = arguments.first() else {
            return Err("requestAnimationFrame requires a function callback.".into());
        };
        if self.animation_frames.len() >= self.limits.host.animation_frames {
            return Err(format!("Animation frame limit reached ({} pending callbacks per document).",self.limits.host.animation_frames));
        }
        let id = self
            .next_animation_frame
            .checked_add(1)
            .ok_or("Animation frame identifier limit reached.")?;
        self.work(1)?;
        self.animation_frames.try_reserve(1).map_err(|_|"Could not reserve animation frame callback storage.")?;
        self.animation_frames.push_back(AnimationFrame {
            id,
            callback: callback.clone(),
        });
        self.next_animation_frame = id;
        self.animation_due
            .get_or_insert_with(|| Instant::now() + FRAME_INTERVAL);
        Ok(Value::Number(f64::from(id)))
    }

    fn cancel_animation_frame(&mut self, arguments: &[Value]) -> Result<Value, String> {
        let number=arguments.first().ok_or("cancelAnimationFrame requires an identifier.")?.to_number();
        let id=if number.is_finite() {number.trunc().rem_euclid(4_294_967_296.0) as u32} else {0};
        self.work(self.animation_frames.len().checked_add(1).ok_or("Animation cancellation work overflow.")?)?;
        self.animation_frames
            .retain(|frame| frame.id != id);
        if self.animation_frames.is_empty() {
            self.animation_due = None;
        }
        Ok(Value::Undefined)
    }

    fn replace_bytes(&mut self, old: usize, new: usize) -> Result<(), String> {
        self.copies(new)?;
        let retained = self.retained.saturating_sub(old).saturating_add(new);
        if retained > self.retained_limit {
            return Err("DOM retained text/attribute growth limit reached (1 MiB).".into());
        }
        self.retained = retained;
        Ok(())
    }

    fn log(&mut self, message: &str) {
        let message = prefix(message, MAX_LOG_LINE);
        if self.logs.len() >= MAX_LOG_LINES
            || self.log_bytes.saturating_add(message.len() + 1) > MAX_LOG_BYTES
        {
            self.omitted = self.omitted.saturating_add(1);
        } else {
            self.log_bytes += message.len() + 1;
            self.logs.push(message.to_owned());
        }
    }

    fn error(&mut self, label: &str, error: &str) {
        self.errors = self.errors.saturating_add(1);
        self.log(&format!(
            "{}: {}",
            prefix(label, 512),
            prefix(error, MAX_LOG_LINE - 520)
        ));
    }

    fn rebuild(&mut self, doc: &Document) -> Result<(), String> {
        self.rebuild_dom_tree(doc)
    }

    fn add_listener(
        &mut self,
        target: u32,
        event: &str,
        function: Value,
        property: bool,
        options: ListenerOptions,
    ) -> Result<(), String> {
        if property {
            if let Some(old) = self
                .listeners
                .iter_mut()
                .find(|l| l.target == target && l.event == event && l.property)
            {
                old.function = function;
                return Ok(());
            }
        } else if self.listeners.iter().any(|l| {
            l.target == target
                && l.event == event
                && !l.property
                && l.options.capture == options.capture
                && same_function(&l.function, &function)
        }) {
            return Ok(());
        }
        if self.listeners.len() >= self.limits.host.listeners {
            return Err("Event listener limit reached (64 per document).".into());
        }
        let serial = self
            .next_listener
            .checked_add(1)
            .ok_or("Event listener identity limit reached.")?;
        let mut name=String::new();
        name.try_reserve_exact(event.len()).map_err(|_|"Could not reserve event listener name.")?;
        name.push_str(event);
        self.listeners.try_reserve(1).map_err(|_|"Could not reserve event listener storage.")?;
        self.listeners.push(Listener {
            serial,
            target,
            event: name,
            function,
            property,
            options,
        });
        self.next_listener=serial;
        Ok(())
    }
}

pub struct Page {
    runtime: Runtime,
    state: State,
}

impl Default for Page {
    fn default() -> Self {
        Self::new()
    }
}

impl Page {
    pub(crate) fn dom_commit_generation(&self) -> Option<u64> { self.state.dom_commits }

    pub fn new() -> Self {
        Self::with_limits(crate::javascript::limits::BROWSER)
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::with_limits(crate::javascript::limits::CAPACITY_REGRESSION)
    }

    fn with_limits(limits: crate::javascript::limits::Limits) -> Self {
        let mut runtime = Runtime::with_limits(limits);
        for (name, handle) in [
            ("window", WINDOW),
            ("self", WINDOW),
            ("globalThis", WINDOW),
            ("document", DOCUMENT),
            ("console", CONSOLE),
            ("navigator", NAVIGATOR),
            ("clientInformation", NAVIGATOR),
            ("location", LOCATION),
            ("history", HISTORY),
            ("performance", PERFORMANCE),
        ] {
            runtime
                .define_global(name, Value::Host(handle))
                .expect("fixed host global");
        }
        runtime
            .bind_browser_global(WINDOW)
            .expect("fixed browser global");
        let mut state = State::new();
        state.limits = limits;
        state.host_work = limits.host_work;
        state.copy_work = limits.copy_bytes;
        state.retained_limit = limits.host.retained_bytes;
        for name in [
            "setTimeout",
            "clearTimeout",
            "requestAnimationFrame",
            "cancelAnimationFrame",
            "addEventListener",
            "removeEventListener",
            "dispatchEvent",
        ] {
            if matches!(name, "setTimeout" | "clearTimeout" | "requestAnimationFrame" | "cancelAnimationFrame") {
                runtime
                    .define_global_operation(
                        name,
                        WINDOW,
                        name,
                        usize::from(name != "clearTimeout"),
                    )
                    .expect("fixed replaceable browser operation");
            } else {
                let function = runtime
                    .define_host_function(name, WINDOW, name)
                    .expect("fixed native global");
                state.native_functions.insert(name, function);
            }
        }
        for (object, method) in [
            (NAVIGATOR, "javaEnabled"),
            (NAVIGATOR, "sendBeacon"),
            (PLUGINS, "item"),
            (PLUGINS, "namedItem"),
            (PLUGINS, "refresh"),
            (MIME_TYPES, "item"),
            (MIME_TYPES, "namedItem"),
            (DOCUMENT_IMAGES, "item"),
            (DOCUMENT_IMAGES, "namedItem"),
            (LOCATION, "toString"),
            (LOCATION, "assign"),
            (LOCATION, "replace"),
            (LOCATION, "reload"),
            (HISTORY, "replaceState"),
            (PERFORMANCE, "now"),
            (DOCUMENT, "getElementById"),
            (DOCUMENT, "getElementsByTagName"),
            (DOCUMENT, "querySelector"),
            (DOCUMENT, "querySelectorAll"),
            (DOCUMENT, "createElement"),
            (DOCUMENT, "createElementNS"),
            (DOCUMENT, "createTextNode"),
            (DOCUMENT, "addEventListener"),
            (DOCUMENT, "removeEventListener"),
            (DOCUMENT, "dispatchEvent"),
        ] {
            let function = if object == HISTORY || object==DOCUMENT&&method=="createElementNS" { runtime.host_method_with_length(object, method, 2) }
                else { runtime.host_method(object, method) }.expect("fixed native method");
            state.native_methods.push((object, method, function));
        }
        let navigator_methods: Vec<_> = state
            .native_methods
            .iter()
            .filter(|(target, _, _)| *target == NAVIGATOR)
            .map(|(_, method, function)| (*method, function.clone()))
            .collect();
        runtime
            .bind_host_object(NAVIGATOR, &navigator_methods)
            .expect("fixed navigator object");
        let image = runtime
            .define_host_constructor("Image", IMAGE_FACTORY, "Image")
            .expect("fixed image constructor");
        state.native_functions.insert("Image", image);
        runtime.define_event_target_interface(WINDOW).expect("fixed EventTarget interface");
        runtime.define_node_interfaces(WINDOW,dom_tree::NODE_CONSTANTS,Some((IMAGE_FACTORY,"Image"))).expect("fixed Node interfaces");
        Self { runtime, state }
    }

    /// Only trusted App navigation updates this context. Failed and pending
    /// navigations must keep the displayed document's existing location.
    pub fn set_location(&mut self, location: &crate::navigation::Location) {
        self.state.location = Some(LocationView::new(location));
    }

    /// Set only on a newly committed document, before any startup script. A
    /// failed/pending load and same-document history never call this setter.
    pub fn set_navigation_timing(&mut self,timing:Option<crate::network_trace::DocumentTiming>) {
        if let Some(t)=timing {self.state.clock_origin=t.origin();}
        self.state.navigation_timing=timing;
    }
    pub fn set_embedded_video(&mut self,snapshot:Option<crate::embedded_video::Snapshot>){self.state.embedded_video=snapshot;}

    pub fn set_geometry(&mut self, geometry: crate::script_geometry::Geometry) {
        self.state.geometry = Some(geometry);
    }

    pub fn invalidate_geometry(&mut self) {
        if let Some(geometry) = &mut self.state.geometry {
            geometry.invalidate();
        }
    }

    pub fn set_geometry_scroll(&mut self, scroll: i32) {
        if let Some(geometry) = &mut self.state.geometry { geometry.set_scroll(scroll); }
    }

    pub fn update_geometry(
        &mut self,
        width: i32,
        height: i32,
        zoom: f32,
        images: HashMap<usize, (usize, usize)>,
    ) {
        if let Some(geometry) = &mut self.state.geometry {
            geometry.update(width, height, zoom, images);
        }
    }

    pub fn sync_geometry(
        &mut self,
        width: i32,
        height: i32,
        zoom: f32,
        images: HashMap<usize, (usize, usize)>,
        layout: &crate::layout::Layout,
    ) {
        if let Some(geometry) = &mut self.state.geometry {
            geometry.sync(width, height, zoom, images, layout);
        }
    }

    /// One user action may cause click, input/change and submit/reset. App calls
    /// this once for that entire action, never once per listener or dispatch.
    pub fn begin_job(&mut self) {
        self.refresh_media_official_position();
        self.runtime.begin_job();
        self.state.host_work = self.state.limits.host_work;
        self.state.copy_work = self.state.limits.copy_bytes;
        self.state.timer_nesting = 0;
        if let Some(geometry) = &mut self.state.geometry {
            geometry.begin_job();
        }
    }

    pub fn next_timer_deadline(&self) -> Option<Instant> {
        if !self.state.enabled {
            return None;
        }
        if self.has_host_tasks() {
            return Some(Instant::now());
        }
        self.state
            .timers
            .iter()
            .map(|timer| timer.due)
            .chain(self.state.animation_due)
            .min()
    }

    /// Executes one due timer task or one bounded animation-frame batch.
    /// Work scheduled by callbacks waits until a later event-loop turn.
    pub fn poll_timer(&mut self, doc: &mut Document, forms: &mut Forms) -> Option<Outcome> {
        self.poll_timer_at(doc, forms, Instant::now())
    }

    fn poll_timer_at(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        now: Instant,
    ) -> Option<Outcome> {
        if !self.state.enabled || !doc.scripting_enabled {
            return None;
        }
        let timer_due = self.state.timers.iter().map(|timer| timer.due).min();
        if self
            .state
            .animation_due
            .is_some_and(|due| due <= now && timer_due.is_none_or(|timer| due <= timer))
        {
            return Some(self.poll_animation_frames(doc, forms, now));
        }
        let (index, _) = self
            .state
            .timers
            .iter()
            .enumerate()
            .filter(|(_, timer)| timer.due <= now)
            .min_by_key(|(_, timer)| (timer.due, timer.id))?;
        if self.state.timer_callbacks >= self.state.limits.host.timer_callbacks {
            self.state.timers.clear();
            self.state.timer_bytes = 0;
            self.state.error(
                "setTimeout",
                "Timer callback limit reached (4096 per document). Reload starts a new allowance.",
            );
            return Some(Outcome {
                errors: 1,
                ..Outcome::default()
            });
        }
        let timer = self.state.timers.remove(index);
        self.state.timer_bytes -= timer.bytes;
        self.begin_job();
        self.state.timer_nesting = timer.nesting;
        self.state.timer_callbacks += 1;
        let before = self.state.errors;
        if let Err(error) = self.runtime.invoke(
            timer.callback,
            &timer.arguments,
            Value::Host(WINDOW),
            &mut Bridge {
                doc,
                forms,
                state: &mut self.state,
            },
        ) {
            self.state
                .error(&format!("setTimeout callback {}", timer.id), &error);
        }
        self.state.timer_nesting = 0;
        self.microtask_checkpoint(doc, forms);
        Some(Outcome {
            prevented: false,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        })
    }

    pub fn stop_timers(&mut self, reason: &str) {
        self.runtime.cancel_microtasks();
        self.cancel_media_selection();
        self.state.timers.clear();
        self.state.animation_frames.clear();
        self.state.animation_due = None;
        self.state.timer_bytes = 0;
        self.state.error("setTimeout", reason);
    }

    fn poll_animation_frames(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        now: Instant,
    ) -> Outcome {
        self.begin_job();
        let before = self.state.errors;
        let timestamp = self.state.clock_ms(now);
        // IDs are monotonic and the queue preserves registration order. The
        // cutoff is a snapshot without allocation: cancellation still removes
        // live entries, and newer callbacks always wait for the next frame.
        let cutoff = self.state.animation_frames.back().map(|frame| frame.id);
        let batch_work = self.state.animation_frames.len().checked_add(1);
        self.state.animation_due = None;
        if let Err(error) = batch_work.ok_or_else(|| "Animation frame work overflow.".to_owned()).and_then(|work| self.state.work(work)) {
            self.state.animation_frames.clear();
            self.runtime.cancel_microtasks();
            self.cancel_media_selection();
            self.state.error("requestAnimationFrame", &error);
            return Outcome { errors: self.state.errors.saturating_sub(before), ..Outcome::default() };
        }
        while self.state.animation_frames.front().is_some_and(|frame| cutoff.is_some_and(|last| frame.id <= last)) {
            let frame = self.state.animation_frames.pop_front().unwrap();
            let id = frame.id;
            self.state.animation_callbacks = self.state.animation_callbacks.saturating_add(1);
            if let Err(error) = self.runtime.invoke(
                frame.callback,
                &[Value::Number(timestamp)],
                Value::Undefined,
                &mut Bridge {
                    doc,
                    forms,
                    state: &mut self.state,
                },
            ) {
                self.state
                    .error(&format!("requestAnimationFrame callback {id}"), &error);
                if self.runtime.invocation_failed_fatally() {
                    self.state.animation_frames.clear();
                    self.cancel_media_selection();
                    break;
                }
            }
            if !self.microtask_checkpoint(doc, forms) {
                self.state.animation_frames.clear();
                break;
            }
        }
        if self.state.animation_frames.is_empty() {
            self.state.animation_due = None;
        } else {
            self.state.animation_due = Some(now.max(Instant::now()) + FRAME_INTERVAL);
        }
        Outcome {
            prevented: false,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }

    /// Explicit browser-inspector command, evaluated in this document's real
    /// realm. It has the same host capabilities, CSP eval permission and resource
    /// limits as page code, and never invokes object conversion to print a value.
    pub fn console_eval(&mut self, doc: &mut Document, forms: &mut Forms, source: &str) -> Outcome {
        let before = self.state.errors;
        self.begin_job();
        self.state
            .log(&format!("> {}", prefix(source, MAX_LOG_LINE - 4)));
        let denied = if !self.state.enabled || !doc.scripting_enabled {
            Some("Console execution requires JavaScript to be enabled for this document.")
        } else if !self.state.eval_allowed {
            Some("Console execution is blocked by this document's Content Security Policy (eval is not allowed).")
        } else if source.len() > MAX_CONSOLE_SOURCE {
            Some("Console commands are limited to 8 KiB.")
        } else {
            None
        };
        if let Some(error) = denied {
            self.state.error("Inspector console", error);
        } else {
            match self.runtime.execute(
                source,
                &mut Bridge {
                    doc,
                    forms,
                    state: &mut self.state,
                },
            ) {
                Ok(value) => {
                    let result = match &value {
                        Value::String(text) => format!("{:?}", prefix(text, MAX_LOG_LINE / 2)),
                        _ => value.to_text(),
                    };
                    self.state.log(&format!("=> {result}"));
                }
                Err(error) => self.state.error("Inspector console", &error),
            }
            self.microtask_checkpoint(doc, forms);
        }
        Outcome {
            prevented: false,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }

    /// Clearing diagnostic text does not erase page errors or change its realm.
    pub fn clear_console(&mut self) {
        self.state.logs.clear();
        self.state.log_bytes = 0;
        self.state.omitted = 0;
    }

    #[cfg(test)]
    pub fn start(&mut self, doc: &mut Document, forms: &mut Forms, sources: &[Script]) -> Outcome {
        self.start_with_policy(doc, forms, sources, &crate::csp::PolicySet::default())
    }

    pub fn start_with_policy(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        sources: &[Script],
        policy: &crate::csp::PolicySet,
    ) -> Outcome {
        self.start_document(doc, forms, sources, policy, false)
    }

    /// A network document becomes interactive while its images are loading.
    /// Its realm and listeners survive subsequent resource updates.
    pub fn start_before_load(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        sources: &[Script],
        policy: &crate::csp::PolicySet,
    ) -> Outcome {
        self.start_document(doc, forms, sources, policy, true)
    }

    fn start_document(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        sources: &[Script],
        policy: &crate::csp::PolicySet,
        defer_load: bool,
    ) -> Outcome {
        let geometry = self.state.geometry.take();
        let location = self.state.location.take();
        let cookies = self.state.cookies.clone();
        let resource_base = self.state.dynamic.base.take();
        let stylesheet_bytes = self.state.stylesheets.source_bytes;
        let stylesheet_rules = self.state.stylesheets.source_rules;
        let images = std::mem::take(&mut self.state.images);
        let history = std::mem::take(&mut self.state.history);
        let navigation_timing=self.state.navigation_timing;
        let embedded_video=self.state.embedded_video.take();
        let limits = self.state.limits;
        *self = Self::with_limits(limits);
        self.set_navigation_timing(navigation_timing);
        self.set_embedded_video(embedded_video);
        self.state.geometry = geometry;
        self.state.location = location;
        self.state.cookies = cookies;
        self.state.dynamic.base = resource_base;
        self.state.stylesheets.source_bytes = stylesheet_bytes;
        self.state.stylesheets.source_rules = stylesheet_rules;
        self.state.images = images;
        self.state.history = history;
        self.state.dynamic.policy = policy.clone();
        self.state.dynamic.script_count = sources.len();
        self.state.dynamic.source_bytes = sources.iter().map(|script| script.source.len()).sum();
        self.begin_job();
        self.state.enabled = doc.scripting_enabled;
        self.state.eval_allowed = policy.allows_eval().is_ok();
        if !doc.scripting_enabled {
            return Outcome::default();
        }
        self.state.ready = "interactive";
        self.state.node_limit = doc.nodes.len().saturating_add(self.state.limits.host.extra_nodes);
        if let Err(error) = self.state.work(doc.nodes.len()) {
            self.state.error("JavaScript startup", &error);
            return Outcome {
                errors: 1,
                ..Outcome::default()
            };
        }
        self.state.retained = document_bytes(doc);
        self.state.retained_limit = self.state.retained.saturating_add(self.state.limits.host.retained_bytes);
        if let Err(error) = self.state.rebuild(doc) {
            self.state.error("JavaScript startup", &error);
            return Outcome {
                errors: 1,
                ..Outcome::default()
            };
        }
        // Compile each attribute as an ordinary function in this small global
        // environment. Legacy implicit form/element name scope is unsupported.
        let nodes = self.state.tree.order.clone();
        'handlers: for node in nodes {
            if let Err(error) = self.state.work(1) {
                self.state.error("inline handler scan", &error);
                break;
            }
            for name in [
                "onclick",
                "oninput",
                "onchange",
                "onsubmit",
                "onreset",
                "onload",
                "onkeydown",
                "onkeyup",
                "onerror","onloadstart","onabort","onemptied","ondurationchange","onloadedmetadata","onloadeddata",
                "oncanplay","oncanplaythrough","onplay","onplaying","onpause","onseeking","onseeked","ontimeupdate","onended",
            ] {
                if !matches!(name,"onclick"|"oninput"|"onchange"|"onsubmit"|"onreset"|"onload"|"onkeydown"|"onkeyup")
                    && !self.state.embedded_video.as_ref().is_some_and(|video|video.node==node) {continue;}
                let Some(source) = doc.attr(node, name) else {
                    continue;
                };
                let target = if name == "onload" && tag(doc, node) == "body" {
                    WINDOW
                } else {
                    node_handle(node)
                };
                if name == "onload" && target != WINDOW {
                    continue;
                }
                let label = format!("inline {name} on node {node}");
                if source.len() > MAX_HANDLER_BYTES {
                    self.state
                        .error(&label, "Inline event handler exceeds 8 KiB.");
                    continue;
                }
                if let Err(error) = policy.allows_attribute(source) {
                    self.state.error(&label, &error);
                    continue;
                }
                if self.state.listeners.len() >= self.state.limits.host.listeners {
                    self.state
                        .error(&label, "Event listener limit reached (64 per document).");
                    break 'handlers;
                }
                let result = self.runtime.compile_handler(source);
                match result {
                    Ok(function @ Value::Function(_)) => {
                        if let Err(error) = self.state.add_listener(
                            target,
                            &name[2..],
                            function,
                            true,
                            ListenerOptions::default(),
                        ) {
                            self.state.error(&label, &error);
                        }
                    }
                    Ok(_) => self
                        .state
                        .error(&label, "Handler did not compile to a function."),
                    Err(error) => self.state.error(&label, &error),
                }
            }
        }
        let started = Instant::now();
        let mut startup_steps = 0usize;
        let mut startup_copy = 0usize;
        let mut startup_regexp = 0usize;
        let mut startup_host = 0usize;
        for source in sources {
            if startup_steps >= self.state.limits.startup_steps
                || startup_copy >= self.state.limits.startup_copy
                || startup_regexp >= self.state.limits.startup_regexp
                || startup_host >= self.state.limits.startup_host_work
                || started.elapsed() >= Duration::from_secs(self.state.limits.startup_seconds)
            {
                self.state.error("JavaScript startup", "Aggregate startup work/time limit reached; remaining classic scripts were skipped.");
                break;
            }
            // Each classic script is an independent execution job. An earlier
            // exception or exhausted allowance must not poison the next one.
            // Retained AST/object/binding caps still cover the entire realm.
            self.begin_job();
            let remaining_copy = self.state.limits.startup_copy - startup_copy;
            let runtime_copy = (remaining_copy / 2).min(self.state.limits.copy_bytes);
            self.state.copy_work = (remaining_copy - runtime_copy).min(self.state.limits.copy_bytes);
            self.state.host_work = (self.state.limits.startup_host_work - startup_host).min(self.state.limits.host_work);
            let host_copy = self.state.copy_work;
            let host_work = self.state.host_work;
            self.runtime.constrain_job(
                self.state.limits.startup_steps - startup_steps,
                runtime_copy,
                self.state.limits.startup_regexp - startup_regexp,
                started + Duration::from_secs(self.state.limits.startup_seconds),
            );
            self.state.executed += 1;
            if let Err(error) = self.runtime.execute(
                &source.source,
                &mut Bridge {
                    doc,
                    forms,
                    state: &mut self.state,
                },
            ) {
                self.state.error(
                    &format!("{} (script node {})", source.label, source.node),
                    &error,
                );
            }
            // Reactions run before the next classic script and consume this
            // script's allowance, including the aggregate startup budget.
            self.microtask_checkpoint(doc, forms);
            let (steps, copies, regexp) = self.runtime.job_usage();
            startup_steps = startup_steps.saturating_add(steps);
            startup_copy = startup_copy
                .saturating_add(copies)
                .saturating_add(host_copy - self.state.copy_work);
            startup_regexp = startup_regexp.saturating_add(regexp);
            startup_host = startup_host.saturating_add(host_work - self.state.host_work);
        }
        self.begin_job();
        self.dispatch_to(doc, forms, DOCUMENT, "DOMContentLoaded", false, true);
        let loaded = if defer_load {
            Outcome::default()
        } else {
            self.finish_loading(doc, forms)
        };
        let changed = std::mem::take(&mut self.state.changed) | loaded.layout_changed;
        Outcome {
            prevented: false,
            layout_changed: changed,
            errors: self.state.errors,
        }
    }

    /// The later resource-completion task fires load at most once. Stopping or
    /// replacing a load does not call this method for the abandoned document.
    pub fn finish_loading(&mut self, doc: &mut Document, forms: &mut Forms) -> Outcome {
        if !self.state.enabled || !doc.scripting_enabled || self.state.ready != "interactive" {
            return Outcome::default();
        }
        self.state.dynamic.load_ready = true;
        if self.state.dynamic.in_flight != 0 || self.state.stylesheets.in_flight != 0
            || self.state.embedded_video.as_ref().is_some_and(|video|video.load_pending)
            || self.state.images.entries.values().any(|entry| entry.pending) {
            return Outcome::default();
        }
        self.begin_job();
        let before = self.state.errors;
        self.state.ready = "complete";
        self.dispatch_to(doc, forms, WINDOW, "load", false, false);
        // dispatch_to includes each callback's microtask checkpoint. A queued
        // Location request already captured its history handling before this.
        self.state.document_complete = true;
        Outcome {
            prevented: false,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors - before,
        }
    }

    /// Native document input only; browser chrome and JS dispatch never call it.
    pub fn notify_user_activation(&mut self) {
        let now = Instant::now();
        self.state.last_activation = Some(now);
        self.state.notify_blank_frame_activation(now);
    }

    pub fn error_count(&self) -> usize {
        self.state.errors
    }

    /// Called by App after beginning the native input job. No script can
    /// manufacture this trusted input or observe browser/inspector editors.
    pub fn keyboard(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        input: KeyboardInput,
    ) -> Outcome {
        if !self.state.enabled || !doc.scripting_enabled {
            return Outcome::default();
        }
        let before = self.state.errors;
        if input.key.len() > 64 || input.code.len() > 64 || input.location > 3 {
            self.state.error(
                "Keyboard input",
                "Native keyboard input exceeds its bounds.",
            );
            return Outcome {
                errors: 1,
                ..Outcome::default()
            };
        }
        if let Err(error) = self.state.rebuild(doc) {
            self.state.error("Keyboard input", &error);
            return Outcome {
                errors: 1,
                ..Outcome::default()
            };
        }
        if input.down && input.key != "Escape" {
            self.notify_user_activation();
        }
        let node = input.target.or(self.state.tree.body);
        let target = node.map_or(DOCUMENT, node_handle);
        let event = if input.down { "keydown" } else { "keyup" };
        self.state.pending_keyboard = Some(input);
        let prevented = self.dispatch_to(doc, forms, target, event, true, true);
        self.state.pending_keyboard = None;
        Outcome {
            prevented,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }

    pub fn dispatch(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        target: usize,
        event: &str,
        cancelable: bool,
    ) -> Outcome {
        if !self.state.enabled || !doc.scripting_enabled {
            return Outcome::default();
        }
        let before = self.state.errors;
        // Direct activation is currently wired only for supported form controls
        // and forms; ancestors can still observe these bubbling events.
        let supported = match event {
            "click" | "input" | "change" => forms
                .controls
                .get(&target)
                .is_some_and(|c| !matches!(c.kind, Kind::Hidden | Kind::Unsupported)),
            "submit" | "reset" => tag(doc, target) == "form",
            "load" | "error" => {
                tag(doc, target) == "img" && self.state.images.entries.contains_key(&target)
                    || matches!(tag(doc, target), "script" | "link")
                    && self.state.dom.created.contains(&target)
            }
            _ => false,
        };
        if !supported {
            return Outcome::default();
        }
        let prevented = self.dispatch_to(
            doc,
            forms,
            node_handle(target),
            event,
            cancelable,
            !matches!(event, "load" | "error"),
        );
        Outcome {
            prevented,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }

    fn dispatch_to(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        target: u32,
        event: &str,
        cancelable: bool,
        bubbles: bool,
    ) -> bool {
        // Resource completion can reach a native event tail in the same job
        // as a fatal script. It must not revive that job or queue later work.
        if self.runtime.invocation_failed_fatally() {
            self.runtime.cancel_microtasks();
            self.cancel_media_selection();
            return false;
        }
        let result = self.event_path(doc, target);
        let path = match result {
            Ok(path) => path,
            Err(error) => {
                self.state.error(event, &error);
                return false;
            }
        };
        let Some(next) = self.state.next_event.checked_add(1).filter(|next|*next <= script_class_list::BASE) else {
            self.state.error(event, "Event identity limit reached.");
            return false;
        };
        let handle = self.state.next_event;
        self.state.next_event = next;
        self.state.event = Some(Event {
            handle,
            // HTML's Window load event uses the legacy target override:
            // target is Document while currentTarget and callback this are Window.
            target: if target == WINDOW && event == "load" {
                DOCUMENT
            } else {
                target
            },
            current: target,
            name: event.into(),
            cancelable,
            bubbles,
            prevented: false,
            stopped: false,
            immediate: false,
            phase: 0,
            passive: false,
            keyboard: self.state.pending_keyboard.take(),
            timestamp: self.state.clock_ms(Instant::now()),
        });
        let mut visits: Vec<_> = path
            .iter()
            .skip(1)
            .rev()
            .map(|&node| (node, true, 1))
            .collect();
        visits.extend([(target, true, 2), (target, false, 2)]);
        if bubbles {
            visits.extend(path.iter().skip(1).map(|&node| (node, false, 3)));
        }
        'visits: for (current, capture, phase) in visits {
            if self
                .state
                .event
                .as_ref()
                .is_some_and(|e| e.immediate || e.stopped)
            {
                break;
            }
            if let Some(active) = &mut self.state.event {
                active.current = current;
                active.phase = phase;
            }
            // DOM snapshots each current target/phase separately. A removed
            // listener stays removed, and a listener added during this phase
            // waits for a subsequent phase or dispatch.
            let snapshot: Vec<_> = self
                .state
                .listeners
                .iter()
                .filter(|l| l.event == event && l.target == current && l.options.capture == capture)
                .map(|l| l.serial)
                .collect();
            for serial in &snapshot {
                let Some(listener) = self
                    .state
                    .listeners
                    .iter()
                    .find(|l| l.serial == *serial && l.target == current)
                    .cloned()
                else {
                    continue;
                };
                if listener.options.once {
                    self.state.listeners.retain(|l| l.serial != *serial);
                }
                if let Some(active) = &mut self.state.event {
                    active.passive = listener.options.passive;
                }
                let result = self.runtime.invoke(
                    listener.function,
                    &[Value::Host(handle)],
                    Value::Host(current),
                    &mut Bridge {
                        doc,
                        forms,
                        state: &mut self.state,
                    },
                );
                match result {
                    Ok(Value::Bool(false)) if listener.property && cancelable => {
                        if let Some(active) = &mut self.state.event {
                            active.prevented = true;
                        }
                    }
                    Err(error) => self.state.error(&format!("{event} listener"), &error),
                    _ => {}
                }
                // This dispatch is entered by the native host, with no outer
                // JavaScript stack. Each completed callback reaches a checkpoint
                // while this event's cancellation/passive state is still live.
                let checkpoint_complete = !self.runtime.invocation_failed_fatally()
                    && self.microtask_checkpoint(doc, forms);
                if let Some(active) = &mut self.state.event {
                    active.passive = false;
                }
                if !checkpoint_complete {
                    self.runtime.cancel_microtasks();
                    self.cancel_media_selection();
                    break 'visits;
                }
                if self.state.event.as_ref().is_some_and(|e| e.immediate) {
                    break;
                }
            }
        }
        // Handles expire instead of being reused: retained JS event references
        // can never mutate a later event's cancellation state.
        self.state.event.take().is_some_and(|event| event.prevented)
    }

    fn event_path(&mut self, doc: &Document, target: u32) -> Result<Vec<u32>, String> {
        self.state.work(1)?;
        self.state.rebuild(doc)?;
        if target == WINDOW {
            return Ok(vec![WINDOW]);
        }
        if target == DOCUMENT {
            return Ok(vec![DOCUMENT, WINDOW]);
        }
        let node = node_index(target).ok_or("Unknown event target.")?;
        if !self.state.tree.active.get(node).copied().unwrap_or(false)
            && !(tag(doc, node) == "script" && self.state.dom.created.contains(&node))
            && !(tag(doc, node) == "img" && self.state.images.entries.contains_key(&node))
        {
            return Err("Event target is no longer connected.".into());
        }
        let mut path = vec![target];
        {
            let mut parent = self.state.tree.parents[node];
            while let Some(node) = parent {
                self.state.work(1)?;
                if node != 0 {
                    path.push(node_handle(node));
                }
                parent = self.state.tree.parents[node];
            }
            if self.state.dom.connected.get(node).copied().unwrap_or(false) {
                path.extend([DOCUMENT, WINDOW]);
            }
        }
        Ok(path)
    }

    pub fn report(&self) -> String {
        if !self.state.enabled {
            return "JavaScript did not run for this document (disabled or blocked by document policy).\n".into();
        }
        let mut text = format!("ChezCat JavaScript subset\n{} script(s), {} active listener(s), {} error(s).\n{} pending timer(s), {} timer callback(s) executed.\nClassic scripts run after parsing. Events come from supported controls/forms and completed dynamic scripts/stylesheets.\nFunction timers use the document queue. Bounded eval follows the document CSP.\nLocation inspection reflects the displayed document; same-origin assign/replace/reload and href/search/hash writes queue navigation.\nSession cookies are memory-only with host-only/Domain scope and bundled public-suffix boundaries; document.cookie excludes HttpOnly entries.\nDynamic external classic scripts use CSP, MIME, request-blocking and bounded source checks; cross-origin HTTPS scripts omit credentials. Crossorigin-marked scripts require same-origin at every hop. Dynamic LINK stylesheets retain exact-origin checks. Checked innerHTML replaces ordinary HTML fragments with inert injected scripts and real image queues; unsupported parser recovery and native form/resource changes reject before replacement. Modules, persistent storage, fetch and XHR are not implemented.\n\n", self.state.executed, self.state.listeners.len(), self.state.errors, self.state.timers.len(), self.state.timer_callbacks);
        let attribution = self.runtime.string_work_failure_report();
        let reachability = self.runtime.persistent_allocation_report();
        let promises = self.runtime.promise_state_report();
        let caught = self.runtime.caught_error_report();
        // Reserve the fixed snapshot inside the existing log byte allowance.
        // Rendering never mutates the stored log, counters, or error count.
        let log_budget = MAX_LOG_BYTES
            .saturating_sub(text.len())
            .saturating_sub(attribution.as_ref().map_or(0, String::len))
            .saturating_sub(reachability.len())
            .saturating_sub(promises.len())
            .saturating_sub(caught.len())
            .saturating_sub(1536); // Fixed trailers, heap profile and omission count.
        let mut rendered_log_bytes = 0;
        let mut snapshot_omissions = 0usize;
        for line in &self.state.logs {
            let bytes = line.len().saturating_add(1);
            if bytes > log_budget.saturating_sub(rendered_log_bytes) {
                snapshot_omissions = snapshot_omissions.saturating_add(1);
                continue;
            }
            text.push_str(line);
            text.push('\n');
            rendered_log_bytes += bytes;
        }
        if let Some(attribution) = attribution {
            text.push_str(&attribution);
        }
        text.push_str(&reachability);
        text.push_str(&promises);
        text.push_str(&caught);
        text.push_str(&format!("Animation frames: {} pending callback(s), {} executed. Whole-millisecond monotonic timestamps; 16 ms minimum frame interval.\n", self.state.animation_frames.len(), self.state.animation_callbacks));
        text.push_str(&format!("Accepted script navigation requests: {}.\n", self.state.navigation_requests));
        if let Some(t)=self.state.navigation_timing {text.push_str(&t.report());}
text.push_str(&format!("Classic scripts have independent execution jobs ({} steps / {} seconds / {} MiB string-copy work). Aggregate startup: {} steps / {} seconds / {} MiB string-copy work. Native-stack and allocation guards are separate.\nFresh detached image requests declined: {}. Ordinary connected HTML img source changes use cancelable real resource requests; current/pending/broken state and completion events follow actual decoding. Detached never-connected Image nodes only reflect attributes; no request or load event was fabricated. Responsive dynamic selection remains unsupported.\nBeacon requests declined: {}. sendBeacon returns false; no data was queued or transmitted.\n", self.state.limits.steps, self.state.limits.job_seconds, self.state.limits.copy_bytes / (1024 * 1024), self.state.limits.startup_steps, self.state.limits.startup_seconds, self.state.limits.startup_copy / (1024 * 1024), self.state.declined_images, self.state.declined_beacons));
        let heap = self.state.limits.heap;
        text.push_str(&format!(
            "Script heap caps: {} properties per object / {} per document; {} object identities; array length {}; {} MiB retained-value payload accounting (not process memory). No up-front reservation.\n",
            heap.object_properties, heap.properties, heap.objects, heap.array_length,
            heap.retained_values / (1024 * 1024),
        ));
        let omitted = self.state.omitted.saturating_add(snapshot_omissions);
        if omitted > 0 {
            text.push_str(&format!(
                "{} further log entries omitted by the report limit.\n",
                omitted
            ));
        }
        text
    }
}

struct Bridge<'a> {
    doc: &'a mut Document,
    forms: &'a mut Forms,
    state: &'a mut State,
}

// Web IDL long conversion used by timer delays/identifiers. Object coercion is
// deliberately outside this bridge; primitive conversion matches the runtime.
fn timer_integer(value: &Value) -> Result<i32, String> {
    if matches!(value, Value::Symbol(_)) {
        return Err("Cannot convert a Symbol value to a timer delay or identifier.".into());
    }
    if matches!(
        value,
        Value::Function(_) | Value::Object(_) | Value::Host(_)
    ) {
        return Err("Timer delay/identifier requires a primitive value; object coercion is not implemented.".into());
    }
    let number = value.to_number();
    if !number.is_finite() || number == 0.0 {
        return Ok(0);
    }
    Ok(number.trunc().rem_euclid(4_294_967_296.0) as u32 as i32)
}

fn navigator_platform() -> String {
    if cfg!(target_os = "windows") {
        "Win32".into()
    } else if cfg!(target_os = "macos") {
        "MacIntel".into()
    } else if cfg!(target_os = "linux") {
        format!("Linux {}", std::env::consts::ARCH)
    } else {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

impl Bridge<'_> {
    // A stable live HTMLCollection, limited to this realm's connected DOM.
    // https://dom.spec.whatwg.org/#interface-htmlcollection
    fn image_at(&mut self, index: usize) -> Result<Option<usize>, String> {
        self.state.rebuild(self.doc)?;
        let mut found = 0usize;
        for position in 0..self.state.tree.order.len() {
            let node = self.state.tree.order[position];
            self.state.work(1)?;
            if tag(self.doc, node) == "img" {
                if found == index {
                    return Ok(Some(node));
                }
                found += 1;
            }
        }
        Ok(None)
    }

    fn image_named(&mut self, name: &str) -> Result<Option<usize>, String> {
        if name.is_empty() {
            return Ok(None);
        }
        if name.len() > self.state.limits.host.string_bytes {
            return Err(format!("Image collection name exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
        }
        self.state.copies(name.len())?;
        self.state.rebuild(self.doc)?;
        for position in 0..self.state.tree.order.len() {
            let node = self.state.tree.order[position];
            self.state.work(1)?;
            if tag(self.doc, node) == "img" {
                for attribute in ["id", "name"] {
                    let value = self.doc.attr(node, attribute);
                    self.state.copies(value.map_or(0, str::len))?;
                    if value == Some(name) {
                        return Ok(Some(node));
                    }
                }
            }
        }
        Ok(None)
    }

    fn image_collection_get(&mut self, property: &str) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        if property == "length" {
            self.state.work(self.state.tree.order.len())?;
            let length = self
                .state
                .tree
                .order
                .iter()
                .filter(|&&node| tag(self.doc, node) == "img")
                .count();
            return Ok(Value::Number(length as f64));
        }
        let node = if let Some(index) = collection_index(property) {
            self.image_at(index)?
        } else {
            self.image_named(property)?
        };
        Ok(node.map_or(Value::Undefined, |node| Value::Host(node_handle(node))))
    }

    fn element_parent(&mut self, node: usize, document: bool) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        Ok(match self.state.tree.parents[node] {
            Some(0) if document => Value::Host(DOCUMENT),
            Some(parent) if parent != 0 => Value::Host(node_handle(parent)),
            _ => Value::Null,
        })
    }

    fn node(&mut self, handle: u32) -> Result<usize, String> {
        self.state.rebuild(self.doc)?;
        let node = node_index(handle).ok_or("This is not an element handle.")?;
        if (!self.state.tree.active.get(node).copied().unwrap_or(false)
            && !self.state.detached_images.contains(&node)
            && !self.state.detached_nodes.contains(&node)
            && !self.state.dom.created.contains(&node))
            || !self
                .doc
                .nodes
                .get(node)
                .is_some_and(|node| matches!(node.kind, NodeKind::Element { .. }))
            || node == 0
        {
            return Err("Element is inert or unavailable in this subset.".into());
        }
        Ok(node)
    }

    fn text(&mut self, value: &Value) -> Result<String, String> {
        if matches!(value, Value::Symbol(_)) {
            return Err("Cannot convert a Symbol value to a DOM string.".into());
        }
        if matches!(
            value,
            Value::Function(_) | Value::Host(_) | Value::Object(_)
        ) {
            return Err("DOM values must be primitive strings, numbers, booleans or null.".into());
        }
        let text = value.to_text();
        if text.len() > self.state.limits.host.string_bytes {
            return Err(format!("DOM string limit reached ({} KiB).", self.state.limits.host.string_bytes / 1024).into());
        }
        self.state.copies(text.len())?;
        Ok(text)
    }

    fn string(&mut self, text: &str) -> Result<Value, String> {
        if text.len() > self.state.limits.host.string_bytes {
            return Err(format!("DOM result exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
        }
        self.state.copies(text.len())?;
        Ok(Value::String(text.into()))
    }

    fn location_handle(&self) -> Result<Value, String> {
        if self.state.location.is_some() {
            Ok(Value::Host(LOCATION))
        } else {
            Err("The document Location is unavailable without a committed URL context.".into())
        }
    }

    fn location_property(&mut self, property: &str) -> Result<Value, String> {
        let location = self
            .state
            .location
            .as_ref()
            .ok_or("The document Location is unavailable without a committed URL context.")?;
        let value = location.property(property).ok_or_else(|| {
            format!("Location property {property:?} is not supported by this subset.")
        })?;
        if value.len() > self.state.limits.host.string_bytes {
            return Err(format!("Location result exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
        }
        let size = value.len();
        self.state.copies(size)?;
        // Reborrow after charging the job's copy budget, before allocating.
        Ok(Value::String(
            self.state
                .location
                .as_ref()
                .unwrap()
                .property(property)
                .unwrap()
                .to_owned(),
        ))
    }

    fn attribute(&mut self, node: usize, name: &str, value: Option<String>) -> Result<(), String> {
        self.state.copies(name.len())?;
        self.attribute_named(node, crate::html::AttributeName::unqualified(name.into()), value)
    }

    fn attribute_named(&mut self, node: usize, name: crate::html::AttributeName, value: Option<String>) -> Result<(), String> {
        let media_source=name.html_is("src")&&self.media_element_receiver(node_handle(node));let media_change=media_source&&value.is_some();
        if media_change&&!self.media_source_prepared(){return Err("Media source change requires its reserved stable-state task.".into());}
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html && tag(self.doc,node)=="video"
            && !media_source && ["src","controls","width","height","poster","autoplay","loop","muted","preload","crossorigin"].iter().any(|key|name.html_is(key)) {
            return Err("Changing parser-authored video source or native media attributes is not implemented; no attribute was changed.".into());
        }
        if name.namespace.is_none() { self.frame_attribute_guard(node, &name.qualified)?; }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else {
            return Err("Not an element.".into());
        };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|attr| attr.name.bytes()).sum())?;
        let slot = attrs.iter().position(|attr| attr.name.same_expanded_name(&name));
        if attrs.len() >= 4096 && value.is_some() && slot.is_none() {
            return Err("Element attribute limit reached (4096).".into());
        }
        if name.html_is("id") && self.doc.attr(node, "id") != value.as_deref() {
            self.state.work(self.forms.controls.len())?;
            self.state.copies(
                self.forms
                    .controls
                    .keys()
                    .map(|&control| self.doc.attr(control, "form").map_or(0, str::len))
                    .sum(),
            )?;
            let old = self.doc.attr(node, "id");
            if self.forms.controls.keys().any(|&control| {
                self.doc.attr(control, "form").is_some_and(|owner| {
                    !owner.is_empty() && (Some(owner) == old || Some(owner) == value.as_deref())
                })
            }) {
                return Err(
                    "Changing an id used by an explicit form association is not implemented."
                        .into(),
                );
            }
        }
        if name.html_is("disabled") {
            self.charge_disabled_refresh(node, value.is_some())?;
        }
        let NodeKind::Element { attrs, .. } = &mut self.doc.nodes[node].kind else { unreachable!() };
        if slot.is_none() && value.is_some() { attrs.try_reserve(1).map_err(|_| "Attribute storage reservation failed.")?; }
        let factor = if name.html_is("id") { 2 } else { 1 };
        let old = slot.map_or(0, |slot| attrs[slot].name.bytes() + attrs[slot].value.len() * factor);
        // Existing expanded names retain the original QName/prefix and order.
        let new = value.as_ref().map_or(0, |value| value.len() * factor + slot.map_or_else(|| name.bytes(), |slot| attrs[slot].name.bytes()));
        let detached_attr = if value.is_none() {
            self.state.work(self.state.attributes.len())?;
            self.state.copies(self.state.attributes.iter().filter(|attr| attr.owner == Some(node)).map(|attr| attr.name.bytes()).sum())?;
            self.state.attributes.iter().position(|attr| attr.owner == Some(node) && attr.name.same_expanded_name(&name))
        } else { None };
        let retained_value = if detached_attr.is_some() { slot.map_or(0, |slot| attrs[slot].value.len()) } else { 0 };
        if name.html_is("id") { self.state.copies(value.as_ref().map_or(0, String::len))?; }
        self.state.replace_bytes(old, new + retained_value)?;
        let NodeKind::Element { attrs, id, .. } = &mut self.doc.nodes[node].kind else {
            return Err("Not an element.".into());
        };
        if name.html_is("id") {
            *id = value.clone();
            self.state.dirty = true;
        }
        let disabled = name.html_is("disabled");
        if let Some(index) = slot {
            if let Some(value) = value {
                attrs[index].value = value;
            } else {
                let removed = attrs.remove(index).value;
                if let Some(attr) = detached_attr {
                    self.state.attributes[attr].owner = None;
                    self.state.attributes[attr].value = removed;
                }
            }
        } else if let Some(value) = value {
            attrs.push(crate::html::Attribute { name, value });
        }
        if disabled {
            self.forms.refresh_disabled(self.doc);
        }
        if slot.is_some() || new != 0 { self.state.dom_committed(); }
        if self.state.tree.active.get(node).copied().unwrap_or(false) {
            self.state.layout_changed();
        }
        if media_change{self.commit_media_source_change(node)?;}
        Ok(())
    }

    fn charge_disabled_refresh(&mut self, changed: usize, present: bool) -> Result<(), String> {
        // Forms rebuilds parent links and checks fieldset/first-legend ancestry.
        // Account for those traversals before changing the attribute. The small
        // estimation walk is charged too, including possible legend searches.
        self.state.work(
            self.doc
                .nodes
                .len()
                .saturating_mul(2)
                .saturating_add(self.forms.controls.len()),
        )?;
        self.state.work(
            self.forms
                .controls
                .values()
                .filter_map(|control| control.select.as_ref())
                .map(|select| select.options.len().saturating_mul(3))
                .sum(),
        )?;
        for &control in self.forms.controls.keys() {
            let mut depth = 1usize;
            let mut parent = self.state.tree.parents[control];
            while let Some(node) = parent {
                self.state.work(2)?;
                let disabled = if node == changed {
                    present
                } else {
                    self.doc.attr(node, "disabled").is_some()
                };
                if tag(self.doc, node) == "fieldset" && disabled {
                    self.state
                        .work(self.doc.nodes[node].children.len().saturating_add(depth))?;
                }
                parent = self.state.tree.parents[node];
                depth += 1;
            }
        }
        Ok(())
    }

    fn text_content(&mut self, node: usize) -> Result<String, String> {
        self.state.rebuild(self.doc)?;
        let mut result = String::new();
        let root = node;
        let mut stack = vec![node];
        while let Some(node) = stack.pop() {
            self.state.work(1)?;
            if node != root
                && dom_tree::inert_dom_tag(tag(self.doc, node), self.doc.scripting_enabled)
            {
                continue;
            }
            if !self.state.tree.active[node] && !self.state.detached_nodes.contains(&node) {
                continue;
            }
            if let NodeKind::Text(text) = &self.doc.nodes[node].kind {
                if result.len().saturating_add(text.len()) > self.state.limits.host.string_bytes {
                    return Err(format!("textContent result exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
                }
                self.state.copies(text.len())?;
                result.push_str(text);
            }
            stack.extend(self.doc.nodes[node].children.iter().rev().copied());
        }
        Ok(result)
    }

    fn set_text_content(&mut self, node: usize, text: String) -> Result<(), String> {
        if !text_container(tag(self.doc, node)) {
            return Err(
                "textContent replacement is limited to plain text containers and buttons.".into(),
            );
        }
        // Complete validation before touching any node. Control models and
        // already-loaded script/style/image resources must never become stale.
        let mut descendants = Vec::new();
        let mut stack = self.doc.nodes[node].children.clone();
        let mut old_bytes = 0usize;
        while let Some(child) = stack.pop() {
            self.state.work(1)?;
            if let NodeKind::Element { tag, .. } = &self.doc.nodes[child].kind {
                if !text_container(tag) || tag == "button" {
                    return Err("textContent cannot remove controls, resources, scripts or structural elements.".into());
                }
            }
            old_bytes = old_bytes.saturating_add(node_bytes(&self.doc.nodes[child]));
            descendants.push(child);
            stack.extend(self.doc.nodes[child].children.iter().copied());
        }
        let reuse = if let [child] = self.doc.nodes[node].children.as_slice() {
            matches!(self.doc.nodes[*child].kind, NodeKind::Text(_)).then_some(*child)
        } else {
            None
        };
        if reuse.is_none() && !text.is_empty() && self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes) >= self.state.node_limit {
            return Err("DOM appended-node limit reached (256 per document).".into());
        }
        let mut captions = Vec::new();
        let mut current = Some(node);
        while let Some(index) = current {
            self.state.work(1)?;
            if tag(self.doc, index) == "button" {
                self.state.work(self.doc.nodes.len())?;
                captions.push(index);
            }
            current = self.state.tree.parents.get(index).copied().flatten();
        }
        // Detached nodes remain real objects for saved handles/NodeLists. Their
        // contents remain charged to the document budget until realm teardown.
        self.state
            .replace_bytes(if reuse.is_some() { old_bytes } else { 0 }, text.len())?;
        if let Some(child) = reuse {
            self.doc.nodes[child].kind = NodeKind::Text(text);
        } else {
            self.state.detached_nodes.extend(descendants);
            self.doc.nodes[node].children.clear();
            if !text.is_empty() {
                let child = self.doc.nodes.len();
                self.doc.nodes.push(Node {
                    kind: NodeKind::Text(text),
                    children: Vec::new(),
                    namespace: crate::html::Namespace::None,
                });
                self.doc.nodes[node].children.push(child);
                if self.state.detached_nodes.contains(&node) {
                    self.state.detached_nodes.insert(child);
                }
            }
        }
        // Updating a span inside a button must update the native caption too.
        for index in captions {
            if let Some(control) = self.forms.controls.get_mut(&index) {
                control.caption = forms::caption(self.doc, index, control.kind);
            }
        }
        self.state.dirty |= reuse.is_none();
        self.state.dom_committed();
        if self.state.tree.active.get(node).copied().unwrap_or(false) {
            self.state.layout_changed();
        }
        Ok(())
    }

    fn listener_target(&mut self, handle: u32, event: &str) -> Result<(), String> {
        // Event types are case-sensitive DOM strings, including custom and
        // empty names. Registering one does not create a native event source.
        if event.len() > self.state.limits.host.event_type_bytes {
            return Err("Event type exceeds the 256-byte limit.".into());
        }
        if !self.dispatch_receiver(handle) {return Err("Listener receiver is not a supported EventTarget.".into());}
        Ok(())
    }

    fn handler_event_target(&mut self, handle: u32, event: &str) -> Result<(), String> {
        // Keep native on* property availability unchanged. Custom names are
        // accepted through add/removeEventListener on existing EventTargets.
        let allowed = self.media_element_receiver(handle) && script_media::event_name(event) || matches!(
            event,
            "click" | "input" | "change" | "submit" | "reset" | "keydown" | "keyup"
        ) || event == "DOMContentLoaded" && matches!(handle, DOCUMENT | WINDOW)
            || event == "load" && handle == WINDOW
            || matches!(event, "load" | "error")
                && node_index(handle).is_some_and(|node| {
                    tag(self.doc, node) == "img"
                        || matches!(tag(self.doc, node), "script" | "link")
                            && self.state.dom.created.contains(&node)
                });
        if !allowed {
            return Err(format!("Event {event:?} is not supported on this target."));
        }
        self.listener_target(handle, event)
    }

    fn handler(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        let Some(event) = self.media_handler_event(object,property) else {
            return Ok(None);
        };
        let object = self.handler_target(object, event);
        self.handler_event_target(object, event)?;
        Ok(Some(
            self.state
                .listeners
                .iter()
                .find(|l| l.target == object && l.event == event && l.property)
                .map_or(Value::Null, |l| l.function.clone()),
        ))
    }

    fn set_handler(&mut self, object: u32, property: &str, value: Value) -> Result<bool, String> {
        let Some(event) = self.media_handler_event(object,property) else {
            return Ok(false);
        };
        let object = self.handler_target(object, event);
        self.handler_event_target(object, event)?;
        match value {
            Value::Function(_) => {
                self.state
                    .add_listener(object, event, value, true, ListenerOptions::default())?
            }
            Value::Null | Value::Undefined => self
                .state
                .listeners
                .retain(|l| !(l.target == object && l.event == event && l.property)),
            _ => return Err("Event handler properties accept a function or null.".into()),
        }
        Ok(true)
    }

    fn handler_target(&self, object: u32, event: &str) -> u32 {
        if event == "load" && node_index(object).is_some_and(|node| tag(self.doc, node) == "body") {
            WINDOW
        } else {
            object
        }
    }

    fn event_listener(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        fields: Option<&[(&str, Value)]>,
    ) -> Result<Value, String> {
        if args.len() < 2 {
            return Err("Event listeners need an event name and callback.".into());
        }
        let mut options = ListenerOptions::default();
        if let Some(fields) = fields {
            for (name, value) in fields {
                match *name {
                    "capture" => options.capture = value.truthy(),
                    "once" => options.once = value.truthy(),
                    "passive" => options.passive = value.truthy(),
                    "signal" if !matches!(value, Value::Null | Value::Undefined) => {
                        return Err("AbortSignal listener options are not implemented.".into())
                    }
                    _ => {}
                }
            }
        } else if let Some(value) = args.get(2) {
            if matches!(
                value,
                Value::Object(_) | Value::Host(_) | Value::Function(_)
            ) {
                return Err("Listener options require the runtime dictionary conversion.".into());
            }
            options.capture = value.truthy();
        }
        let event = self.text(&args[0])?;
        self.listener_target(object, &event)?;
        if matches!(args[1], Value::Null | Value::Undefined) {
            return Ok(Value::Undefined);
        }
        if !matches!(args[1], Value::Function(_)) {
            return Err(
                "Event listener objects with handleEvent are not implemented; use a function."
                    .into(),
            );
        }
        if method == "addEventListener" {
            self.state
                .add_listener(object, &event, args[1].clone(), false, options)?;
        } else {
            self.state.listeners.retain(|l| {
                !(l.target == object
                    && l.event == event
                    && !l.property
                    && l.options.capture == options.capture
                    && same_function(&l.function, &args[1]))
            });
        }
        Ok(Value::Undefined)
    }

    fn style_node(&mut self, handle: u32) -> Result<usize, String> {
        if !(STYLE_BASE..DATASET_BASE).contains(&handle) {
            return Err("Unknown style handle.".into());
        }
        self.node(node_handle((handle - STYLE_BASE) as usize))
    }

    fn get_style(&mut self, node: usize, name: &str) -> Result<Value, String> {
        let style = self.doc.attr(node, "style").unwrap_or("");
        if style.len() > self.state.limits.host.string_bytes {
            return Err(format!("Inline style exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
        }
        self.state.copies(style.len())?;
        let value = style
            .split(';')
            .filter_map(|item| item.split_once(':'))
            .filter(|(key, _)| key.trim().eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().split('!').next().unwrap_or("").trim())
            .next_back()
            .unwrap_or("")
            .to_owned();
        self.string(&value)
    }

    fn set_style(
        &mut self,
        node: usize,
        name: &str,
        value: &str,
        priority: &str,
    ) -> Result<(), String> {
        validate_css_value(value, matches!(name, "font" | "font-family"), self.state.limits.host.string_bytes)?;
        if !priority.is_empty() && !priority.eq_ignore_ascii_case("important") {
            return Err("CSS priority must be empty or important.".into());
        }
        let existing = self.doc.attr(node, "style").unwrap_or("");
        if existing.len() > self.state.limits.host.string_bytes {
            return Err(format!("Inline style exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
        }
        self.state.copies(existing.len())?;
        let mut style = String::new();
        for item in existing.split(';').filter(|item| !item.trim().is_empty()) {
            if item
                .split_once(':')
                .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case(name))
            {
                continue;
            }
            style.push_str(item.trim());
            style.push(';');
        }
        if !value.trim().is_empty() {
            style.push_str(name);
            style.push(':');
            style.push_str(value.trim());
            if !priority.is_empty() {
                style.push_str(" !important");
            }
            style.push(';');
        }
        if style.len() > self.state.limits.host.string_bytes {
            return Err(format!("Inline style exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
        }
        self.attribute(node, "style", Some(style))
    }
}

impl Host for Bridge<'_> {
    fn expando_owner(&self,object:u32)->Option<u32> {self.standalone_target_owner(object).or_else(||self.element_expando_receiver(object).then_some(WINDOW))}
    fn exposed_native_prototype(&self,object:u32)->bool {self.standalone_target_owner(object).is_some()}
    fn native_ancestry_inspection(&self,object:u32)->bool {self.exposed_native_prototype(object)||self.namespace_receiver(object)}
    fn event_target_interface(&self,window:u32)->bool {window==WINDOW || self.frame_context(window).is_some()}
    fn node_interface_constants(&self,window:u32)->Option<&'static [(&'static str,u16)]> {self.event_target_interface(window).then_some(dom_tree::NODE_CONSTANTS)}
    fn instance_ancestry(&self, object:u32) -> Option<crate::javascript::NativeAncestry> {
        if let Some(window)=self.standalone_target_owner(object) {
            return Some(crate::javascript::NativeAncestry {window,constructor:Some((window,"EventTarget"))});
        }
        if let Some(window)=self.frame_instance_window(object) {
            let constructor=self.frame_node_interface(object).map(|name|(window,name));
            return Some(crate::javascript::NativeAncestry {window,constructor});
        }
        let node=node_index(object).filter(|_|self.namespace_receiver(object));
        let constructor=if let Some(node)=node {
            Some(if matches!(self.doc.nodes[node].kind,NodeKind::Element {..}) {
                if self.doc.nodes[node].namespace==crate::html::Namespace::Html {
                    if tag(self.doc,node)=="img" {(IMAGE_FACTORY,"Image")} else {(WINDOW,"HTMLElement")}
                } else {(WINDOW,"Element")}
            } else {(WINDOW,"Node")})
        } else if object==DOCUMENT || self.attr_receiver(object) {Some((WINDOW,"Node"))} else {None};
        let known=node.is_some() || matches!(object,DOCUMENT|CONSOLE|NAVIGATOR|PLUGINS|MIME_TYPES|DOCUMENT_IMAGES|LOCATION|PERFORMANCE|PERFORMANCE_TIMING|HISTORY)
            || self.attr_receiver(object) || self.event_receiver(object) || self.class_list_receiver(object) || self.named_map_receiver(object)
            || node_list_index(object).is_some_and(|index|index<self.state.node_lists.len())
            || collection_handle_index(object).is_some_and(|index|index<self.state.dom.collections.len())
            || (STYLE_BASE..EVENT_BASE).contains(&object) && self.attr_element_receiver(node_handle(((object-STYLE_BASE)%500_000) as usize));
        known.then_some(crate::javascript::NativeAncestry {window:WINDOW,constructor})
    }

    fn window_context(&self, window: u32) -> Option<crate::javascript::WindowContext> {
        self.frame_context(window)
    }

    fn validate_window_access(&self, caller: Option<u32>, target: u32) -> Result<(), String> {
        self.frame_window_access(caller, target)
    }

    fn allow_eval_for_window(&mut self, window: Option<u32>) -> Result<bool, String> {
        self.state.work(1)?;
        if !self.state.enabled { return Err("String compilation is unavailable while scripting is disabled.".into()); }
        if let Some(result) = window.and_then(|handle| self.frame_eval(handle)) { return result; }
        self.allow_eval()
    }

    fn owns_window_property(&self, window: Option<u32>, property: &str) -> bool {
        if let Some(window) = window.filter(|&window| self.frame_context(window).is_some()) {
            self.frame_native_property(window, property)
        } else { self.owns_global_property(property) }
    }
    fn primitive_conversion(&self, object: u32) -> bool {
        object == LOCATION || self.class_list_receiver(object) || self.standalone_target_owner(object).is_some()
    }

    fn owns_property(&self, object: u32, property: &str) -> bool {
        if self.node_event_method(object,property) {return false;}
        if self.standalone_target_owner(object).is_some() {return false;}
        if property=="prototype"&&self.location_instance_owner(object).is_some(){return false;}
        if self.element_expando_receiver(object) {return self.element_native_property(object,property);}
        if self.attr_receiver(object) { return self.attr_native_member(property); }
        if self.constructed_event_receiver(object) { return self.event_native_member(object,property); }
        object != NAVIGATOR || navigator_readonly(property)
    }

    fn ordinary_lookup_tail(&self, object:u32) -> bool {
        self.location_instance_owner(object).is_some() || object == HISTORY || self.attr_receiver(object) || self.constructed_event_receiver(object) || self.named_map_receiver(object)
    }
    fn ordinary_lookup_tail_property(&self,object:u32,property:&str)->bool {
        if self.node_event_method(object,property) {true}
        else if self.location_instance_owner(object).is_some() {property=="prototype"}
        else if property=="animate" && self.element_expando_receiver(object) {true}
        else {self.ordinary_lookup_tail(object)}
    }
    fn native_access_owner(&self, object: u32) -> Option<u32> {
        if self.event_target_interface(object) {return Some(object);}
        if let Some(owner)=self.standalone_target_owner(object) {return Some(owner);}
        if let Some(window)=self.frame_instance_window(object) {return Some(window);}
        if self.namespace_receiver(object) {return Some(WINDOW);}
        self.location_instance_owner(object).or_else(||(matches!(object,DOCUMENT|PERFORMANCE|PERFORMANCE_TIMING|HISTORY) || self.named_map_receiver(object) || self.attr_receiver(object) || self.media_element_receiver(object)).then_some(WINDOW))
    }
    fn bounding_rect_snapshot(&mut self, object: u32) -> Option<Result<[f64; 4], String>> {
        if !self.class_list_element_receiver(object) { return None; }
        Some((|| {
            let node = self.node(object)?;
            if !self.state.dom.connected.get(node).copied().unwrap_or(false) { return Ok([0.0; 4]); }
            self.state.geometry.as_mut().ok_or("Bounding rectangle requires the actual document renderer.")?
                .bounding_rect(self.doc, self.forms, node, &mut self.state.host_work)
        })())
    }

    fn readonly_property(&self, object: u32, property: &str) -> bool {
        if self.media_element_receiver(object) && matches!(property,"currentSrc"|"videoWidth"|"videoHeight"|"duration"|"paused"|"ended"|"seeking"|"readyState"|"networkState") {return true;}
        if object==PERFORMANCE&&property=="timing"||object==PERFORMANCE_TIMING&&self.state.navigation_timing.is_some_and(|t|t.value(property).is_some()) {return true;}
        object == WINDOW && property == "history" || object == HISTORY && matches!(property, "state" | "length")
            || self.created_event_readonly(object, property) || self.attr_readonly(object, property) || self.dom_readonly(object, property)
            || collection_handle_index(object).is_some()
            || self.class_list_receiver(object) && (property == "length" || collection_index(property).is_some())
            || self.named_map_receiver(object) && (property == "length" || collection_index(property).is_some())
            || node_list_index(object).is_some()
                && (property == "length" || collection_index(property).is_some())
            || object == NAVIGATOR && navigator_readonly(property)
            || node_index(object).is_some() && property == "dataset"
            || node_index(object).is_some_and(|node| tag(self.doc, node) == "img")
                && matches!(
                    property,
                    "complete" | "naturalWidth" | "naturalHeight" | "currentSrc"
                )
    }

    fn string_setter(&self, object: u32, property: &str) -> bool {
        if property=="type"&&self.style_type_receiver(object){return true;}
        if self.media_element_receiver(object)&&property=="src"{return true;}
        property=="srcdoc" && self.class_list_element_receiver(object) && node_index(object).is_some_and(|node|self.iframe_srcdoc_property(node))
            || self.class_list_receiver(object) && property == "value"
            || self.class_list_element_receiver(object) && property == "classList"
            || self.attr_receiver(object) && matches!(property, "value" | "nodeValue" | "textContent")
            || self.dom_string_setter(object, property)
            || Self::location_string_setter(object, property)
            || object == DOCUMENT && property == "cookie"
            || (DATASET_BASE..EVENT_BASE).contains(&object)
            || node_index(object).is_some_and(|node| tag(self.doc, node) == "img")
                && matches!(property, "src" | "alt")
            || property == "border" && node_index(object).is_some_and(|node| self.html_border_property(node))
    }
    fn prepare_native_microtask(&mut self,object:u32,member:&str,args:&[Value],setter:bool)->Result<Option<u64>,String>{self.prepare_media_microtask(object,member,args,setter)}
    fn finish_native_microtask(&mut self,ticket:u64)->bool{self.finish_media_microtask(ticket)}
    fn run_native_microtask(&mut self,ticket:u64)->Result<(),String>{self.run_media_microtask(ticket)}

    fn number_setter(&self, object: u32, property: &str) -> bool {
        if property=="currentTime" && self.media_element_receiver(object) {return true;}
        node_index(object).is_some_and(|node| tag(self.doc, node) == "img")
            && matches!(property, "width" | "height")
            || node_index(object).is_some_and(|node| self.html_spacing_property(node, property))
    }

    fn argument_conversions(&self, object: u32, method: &str) -> &'static [(usize, bool)] {
        if object==WINDOW && method=="cancelAnimationFrame" {return &[(0,false)];}
        if object == HISTORY && method == "replaceState" { return &[(1, true), (2, true)]; }
        if self.named_map_receiver(object) {
            return match method { "item" => &[(0, false)], "getNamedItem" => &[(0, true)], "getNamedItemNS" => &[(0, true), (1, true)], _ => &[] };
        }
        if self.class_list_receiver(object) {
            return match method { "item"=>&[(0,false)], "contains"=>&[(0,true)], _=>&[] };
        }
        if object == DOCUMENT && method == "createEvent" && self.event_factory_enabled()
            || self.event_receiver(object) && method == "initEvent"
        { return &[(0, true)]; }
        if self.mouse_receiver(object) && method == "initMouseEvent" {
            return &[(0, true), (4, false), (5, false), (6, false), (7, false), (8, false), (13, false)];
        }
        if self.ui_receiver(object) && method=="initUIEvent" {return &[(0,true),(4,false)];}
        if method == "isDefaultNamespace" && self.namespace_receiver(object) { return &[(0, true)]; }
        if object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS") {return &[(0,true),(1,true)];}
        if self.attr_element_receiver(object) && method == "getAttributeNode" { return &[(0, true)]; }
        if self.attr_element_receiver(object) && method == "setAttributeNS" { return &[(0, true), (1, true), (2, true)]; }
        if self.attr_element_receiver(object) && matches!(method,"getAttributeNS" | "getAttributeNodeNS" | "removeAttributeNS") { return &[(0, true), (1, true)]; }
        if collection_handle_index(object).is_some() {
            return match method {
                "item" => &[(0, false)],
                "namedItem" => &[(0, true)],
                _ => &[],
            };
        }
        if (object == DOCUMENT || node_index(object).is_some()) && method=="getElementsByTagName"
            || (object==DOCUMENT || self.attr_element_receiver(object)) && method=="getElementsByClassName"
            || object == DOCUMENT && method == "getElementsByName"
        {
            return &[(0, true)];
        }
        if object == LOCATION && matches!(method, "assign" | "replace") {
            return &[(0, true)];
        }
        if object == IMAGE_FACTORY && method == "Image" {
            return &[(0, false), (1, false)];
        }
        if object == NAVIGATOR && method == "sendBeacon"
            || object == DOCUMENT && matches!(method, "createElement" | "createTextNode" | "createAttribute")
        {
            return &[(0, true)];
        }
        if node_list_index(object).is_some() && method == "item" {
            return &[(0, false)];
        }
        if object == DOCUMENT && method == "getElementById"
            || (object == DOCUMENT || node_index(object).is_some())
                && matches!(
                    method,
                    "querySelector" | "querySelectorAll" | "matches" | "closest"
                )
            || node_index(object).is_some()
                && matches!(method, "getAttribute" | "hasAttribute" | "removeAttribute")
        {
            return &[(0, true)];
        }
        if node_index(object).is_some() && method == "setAttribute" {
            return &[(0, true), (1, true)];
        }
        &[]
    }

    fn variadic_string_arguments(&self, object:u32, method:&str) -> bool {
        self.class_list_receiver(object) && matches!(method,"add"|"remove")
    }

    fn method_length(&self, object:u32, method:&str) -> Option<usize> {
        if matches!(method,"load"|"pause") && self.media_element_receiver(object) {return Some(0);}
        if method=="dispatchEvent"&&self.dispatch_receiver(object){return Some(1);}
        if object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS") {return Some(2);}
        if object==DOCUMENT && method=="createAttribute" || self.attr_element_receiver(object)&&matches!(method,"setAttributeNode"|"setAttributeNodeNS") {return Some(1);}
        if node_index(object).is_some() && method == "replaceChild" { return Some(2); }
        if object == HISTORY && method == "replaceState" { return Some(2); }
        if method == "getBoundingClientRect" && self.class_list_element_receiver(object) { return Some(0); }
        if self.named_map_receiver(object) { return match method { "item" | "getNamedItem" => Some(1), "getNamedItemNS" => Some(2), _ => None }; }
        if self.attr_element_receiver(object) && method == "setAttributeNS" { return Some(3); }
        if self.event_receiver(object) && method=="initEvent"
            || self.ui_receiver(object) && method=="initUIEvent"
            || self.mouse_receiver(object) && method=="initMouseEvent" {return Some(1);}
        if object == DOCUMENT && method == "getElementsByName" { return Some(1); }
        if method=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object)) { return Some(1); }
        if self.class_list_receiver(object) {
            return match method {"contains"|"item"=>Some(1),"add"|"remove"|"toString"=>Some(0),_=>None};
        }
        (self.attr_element_receiver(object) && matches!(method,"getAttributeNS"|"getAttributeNodeNS"|"removeAttributeNS")).then_some(2)
    }

    fn nullable_string_argument(&self, object: u32, method: &str, index: usize) -> bool {
        if object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS") && index==0 {return true;}
        if object == HISTORY && method == "replaceState" && index == 2 { return true; }
        if self.named_map_receiver(object) { return method == "getNamedItemNS" && index == 0; }
        method == "isDefaultNamespace" && index == 0 && self.namespace_receiver(object)
            || matches!(method,"getAttributeNS" | "getAttributeNodeNS" | "removeAttributeNS" | "setAttributeNS") && index == 0 && self.attr_element_receiver(object)
    }

    fn validate_argument_prefix(&self, object: u32, method: &str, next_index: usize, args: &[Value])
        -> Result<(), (&'static str, String)> {
        if object==WINDOW && method=="cancelAnimationFrame" && (!self.state.enabled || !self.doc.scripting_enabled) {return Err(("SecurityError","Animation capability is unavailable while scripting is disabled.".into()));}
        self.validate_event_prefix(object, method, next_index, args)
    }
    fn native_reference_arguments(&self,object:u32,method:&str)->&'static [usize] {
        if method=="initMouseEvent" && self.mouse_receiver(object) {&[3,14]}
        else if method=="initUIEvent" && self.ui_receiver(object) {&[3]}
        else {&[]}
    }

    fn nullish_string_setter(&self, object: u32, property: &str) -> bool {
        self.attr_receiver(object) && matches!(property, "nodeValue" | "textContent")
    }

    fn null_string_setter(&self, object: u32, property: &str) -> bool {
        property == "innerHTML" && self.attr_element_receiver(object)
            || property == "border" && node_index(object).is_some_and(|node|
            self.html_border_property(node) && matches!(tag(self.doc, node), "img" | "object"))
    }

    fn required_arguments(&self, object: u32, method: &str) -> usize {
        if object==WINDOW && matches!(method,"requestAnimationFrame"|"cancelAnimationFrame") {return 1;}
        if method=="dispatchEvent"&&self.dispatch_receiver(object){return 1;}
        if object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS") {return 2;}
        if object==DOCUMENT && method=="createAttribute" || self.attr_element_receiver(object)&&matches!(method,"setAttributeNode"|"setAttributeNodeNS") {return 1;}
        if object == HISTORY && method == "replaceState" { return 2; }
        if self.named_map_receiver(object) { return match method { "item" | "getNamedItem" => 1, "getNamedItemNS" => 2, _ => 0 }; }
        if self.attr_element_receiver(object) && method == "setAttributeNS" { return 3; }
        if self.class_list_receiver(object) && matches!(method,"item"|"contains") { return 1; }
        if self.attr_element_receiver(object) && matches!(method,"getAttributeNS" | "getAttributeNodeNS" | "removeAttributeNS") { return 2; }
        if object == DOCUMENT && method == "getElementsByName" { return 1; }
        if object == DOCUMENT && method == "createEvent" && self.event_factory_enabled()
            || self.event_receiver(object) && method == "initEvent"
            || self.ui_receiver(object) && method=="initUIEvent"
            || self.mouse_receiver(object) && method == "initMouseEvent"
        { return 1; }
        if matches!(method, "isDefaultNamespace" | "compareDocumentPosition") && self.namespace_receiver(object) { return 1; }
        if self.attr_element_receiver(object) && matches!(method, "getAttributeNode" | "removeAttributeNode") { return 1; }
        if collection_handle_index(object).is_some() && matches!(method, "item" | "namedItem")
            || (object == DOCUMENT || node_index(object).is_some())
                && matches!(method,"getElementsByTagName"|"getElementsByClassName")
        {
            return 1;
        }
        if object == LOCATION && matches!(method, "assign" | "replace") {
            return 1;
        }
        if node_index(object).is_some() && matches!(method, "setAttribute" | "insertBefore" | "replaceChild") {
            2
        } else if (object == DOCUMENT || node_index(object).is_some())
            && matches!(
                method,
                "querySelector" | "querySelectorAll" | "matches" | "closest"
            )
            || object == DOCUMENT
                && matches!(
                    method,
                    "getElementById" | "createElement" | "createTextNode"
                )
            || node_index(object).is_some()
                && matches!(
                    method,
                    "getAttribute"
                        | "hasAttribute"
                        | "removeAttribute"
                        | "appendChild"
                        | "removeChild"
                )
            || node_list_index(object).is_some() && matches!(method, "item" | "forEach")
            || object == NAVIGATOR && method == "sendBeacon"
        {
            1
        } else {
            0
        }
    }

    fn validate_call(
        &self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if method=="pause" && self.media_element_receiver(object) && (!self.state.enabled || !self.doc.scripting_enabled) {return Err(("InvalidStateError","Scripting is disabled.".into()));}
        if object==WINDOW && method=="requestAnimationFrame" && !matches!(args.first(),Some(Value::Function(_))) {return Err(("TypeError","requestAnimationFrame requires a callable callback.".into()));}
        if self.event_target_interface(object) && matches!(method,"Node"|"Element"|"HTMLElement") {return Err(("TypeError","Illegal constructor for this native interface.".into()));}
        if method=="dispatchEvent"{return self.validate_dispatch(object,args);}
        if object == HISTORY && method == "replaceState" {
            if self.state.history.current.is_none() { self.history_target(args)?; }
            if matches!(args.first(), Some(Value::Function(_) | Value::Symbol(_))) { return Err(("DataCloneError", "This History value cannot be serialized for storage.".into())); }
            if matches!(args.first(), Some(Value::Object(_) | Value::Host(_))) { return Ok(()); }
            self.history_target(args)?;
            return Ok(());
        }
        if method=="getElementsByClassName" && !(object==DOCUMENT || self.attr_element_receiver(object)) {
            return Err(("TypeError","getElementsByClassName requires a Document or Element.".into()));
        }
        self.validate_created_event_call(object, method, args)?;
        // These existing methods still convert primitive arguments inside the
        // bridge. Reject Symbol before that boundary so the error is catchable
        // and no timer, listener or style mutation has begun. Timer callback
        // arguments and boolean listener options are deliberately untouched.
        let symbols_rejected = if object == WINDOW {
            match method {
                "setTimeout" => args
                    .iter()
                    .take(2)
                    .any(|value| matches!(value, Value::Symbol(_))),
                "clearTimeout" | "requestAnimationFrame" | "cancelAnimationFrame" => {
                    matches!(args.first(), Some(Value::Symbol(_)))
                }
                _ => false,
            }
        } else {
            false
        } || matches!(method, "addEventListener" | "removeEventListener")
            && args.len() >= 2
            && args
                .iter()
                .take(2)
                .any(|value| matches!(value, Value::Symbol(_)))
            || (STYLE_BASE..DATASET_BASE).contains(&object)
                && (method == "setProperty" && (2..=3).contains(&args.len())
                    || matches!(method, "getPropertyValue" | "removeProperty") && args.len() == 1)
                && args.iter().any(|value| matches!(value, Value::Symbol(_)))
            || matches!(object, PLUGINS | MIME_TYPES | DOCUMENT_IMAGES)
                && matches!(method, "item" | "namedItem")
                && matches!(args.first(), Some(Value::Symbol(_)));
        if symbols_rejected {
            return Err((
                "TypeError",
                format!("{method} cannot convert a Symbol argument to its required type."),
            ));
        }
        if script_class_list::element(object).is_some() { return self.class_list_validate(object,method,args); }
        self.dom_validate_call(object, method, args)?;
        if (object == DOCUMENT || node_index(object).is_some())
            && matches!(
                method,
                "querySelector" | "querySelectorAll" | "matches" | "closest"
            )
        {
            let Some(Value::String(source)) = args.first() else {
                return Err((
                    "TypeError",
                    "Selector methods require a selector string.".into(),
                ));
            };
            if !crate::css::valid_query_selector(source) {
                let excerpt: String = source.chars().take(160).collect();
                return Err((
                    "SyntaxError",
                    format!("Invalid selector or selector syntax outside the supported subset: {excerpt:?} ({} UTF-8 bytes; excerpt at most 160 characters).",source.len()),
                ));
            }
        }
        if (node_list_index(object).is_some() && method == "item"
            || object == DOCUMENT && method == "getElementById"
            || node_index(object).is_some()
                && matches!(method, "getAttribute" | "hasAttribute" | "removeAttribute"))
            && args.is_empty()
        {
            return Err(("TypeError", format!("{method} requires one argument.")));
        }
        if node_index(object).is_some() && method == "setAttribute" && args.len() < 2 {
            return Err(("TypeError", "setAttribute requires two arguments.".into()));
        }
        if object == NAVIGATOR && method == "sendBeacon" {
            let Some(Value::String(source)) = args.first() else {
                return Err(("TypeError", "sendBeacon requires a URL.".into()));
            };
            if !self.beacon_url_valid(source) {
                return Err((
                    "TypeError",
                    "sendBeacon requires a valid HTTP or HTTPS URL.".into(),
                ));
            }
        }
        Ok(())
    }

    fn validate_set(
        &self,
        object: u32,
        property: &str,
        value: &Value,
    ) -> Result<(), (&'static str, String)> {
        if property=="currentTime" && self.media_element_receiver(object) {
            if !self.state.enabled || !self.doc.scripting_enabled {
                return Err(("InvalidStateError","Scripting is disabled.".into()));
            }
            if !matches!(value,Value::Number(seconds) if seconds.is_finite()) {
                return Err(("TypeError","Media time must be a finite double.".into()));
            }
        }
        if matches!(value, Value::Symbol(_)) {
            let text_or_index = object == DOCUMENT && property == "title"
                || (STYLE_BASE..DATASET_BASE).contains(&object) && css_name(property).is_ok()
                || node_index(object).is_some_and(|node| {
                    matches!(
                        property,
                        "textContent" | "id" | "className" | "title" | "value" | "selectedIndex"
                    ) || self.doc.nodes.get(node).is_some_and(|node| {
                        matches!(node.kind, NodeKind::Text(_))
                            && matches!(property, "data" | "nodeValue")
                    }) || property == "text"
                        && tag(self.doc, node) == "script"
                        && self.state.dom.created.contains(&node)
                });
            if text_or_index {
                return Err((
                    "TypeError",
                    format!("{property} cannot convert a Symbol value to its required type."),
                ));
            }
        }
        if (DATASET_BASE..EVENT_BASE).contains(&object) {
            if property
                .as_bytes()
                .windows(2)
                .any(|pair| pair[0] == b'-' && pair[1].is_ascii_lowercase())
            {
                return Err((
                    "SyntaxError",
                    "Dataset keys cannot contain a hyphen followed by a lowercase ASCII letter."
                        .into(),
                ));
            }
            if property.chars().any(|ch| {
                ch == '\0'
                    || ch.is_ascii_whitespace()
                    || matches!(ch, '/' | '>' | '=' | '\'' | '"' | '<')
            }) {
                return Err((
                    "InvalidCharacterError",
                    "Dataset key is not a valid attribute name.".into(),
                ));
            }
        }
        Ok(())
    }

    fn resolve_member_before_call(&self, object: u32) -> bool {
        if self.standalone_target_owner(object).is_some() {return true;}
        if object == HISTORY || self.attr_receiver(object) || self.constructed_event_receiver(object) || self.named_map_receiver(object) || self.media_element_receiver(object) { return true; }
        self.form_receiver(object)
    }

    fn ordinary_prototype(&self, object: u32) -> bool {
        self.class_list_receiver(object) || node_list_index(object).is_some()
            || collection_handle_index(object).is_some()
            || (DATASET_BASE..EVENT_BASE).contains(&object)
    }

    fn promise_resolution_receiver(&self, object: u32) -> bool {
        // Promise resolution performs only the normal `then` property lookup.
        // Authorize this protocol explicitly for this realm's Window, Document
        // and real DOM nodes; other native capabilities keep the default deny.
        matches!(object, WINDOW | DOCUMENT)
            || self.class_list_receiver(object)
            || self.attr_receiver(object)
            || self.constructed_event_receiver(object)
            || node_index(object).is_some_and(|node| self.doc.nodes.get(node).is_some())
    }

    fn is_instance(&self, object: u32, constructor: u32, method: &str) -> Option<bool> {
        (constructor == IMAGE_FACTORY && method == "Image")
            .then(|| node_index(object).is_some_and(|node| tag(self.doc, node) == "img"))
    }

    fn method(&self, object: u32, property: &str) -> Option<&'static str> {
        if self.media_element_receiver(object) {match property {"load"=>return Some("load"),"pause"=>return Some("pause"),_=>{}}}
        if matches!(property,"addEventListener"|"removeEventListener"|"dispatchEvent") && self.form_named_control(object,property) {return None;}
        if self.node_event_method(object,property) {return None;}
        if property=="dispatchEvent"&&self.dispatch_receiver(object)&&!self.form_named_control(object,property){return Some("dispatchEvent");}
        if script_named_map::element(object).is_some() { return self.named_map_receiver(object).then(|| script_named_map::method(property)).flatten(); }
        if object == DOCUMENT && property == "getElementsByName" { return Some("getElementsByName"); }
        if property=="normalize" && !self.is_frame_handle(object) && self.namespace_receiver(object) {return Some("normalize");}
        if property=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object))
            && !self.form_named_control(object,property) { return Some("getElementsByClassName"); }
        if script_class_list::element(object).is_some() {
            return self.class_list_receiver(object).then(||script_class_list::method(property)).flatten();
        }
        if object == DOCUMENT && property == "createEvent" && self.event_factory_enabled() { return Some("createEvent"); }
        if let Some(method) = self.created_event_method(object, property) { return Some(method); }
        if property == "submit" {
            return self.form_submit_method(object);
        }
        if collection_handle_index(object).is_some() {
            return match property {
                "item" => Some("item"),
                "namedItem" => Some("namedItem"),
                _ => None,
            };
        }
        if let Some(method) = self.dom_method(object, property) {
            return Some(method);
        }
        if node_list_index(object).is_some() {
            return match property {
                "item" => Some("item"),
                "forEach" => Some("forEach"),
                _ => None,
            };
        }
        if node_index(object).is_some_and(|node| {
            self.doc
                .nodes
                .get(node)
                .is_some_and(|node| matches!(node.kind, NodeKind::Element { .. }))
        }) {
            return match property {
                "getAttribute" => Some("getAttribute"),
                "getElementsByTagName" => Some("getElementsByTagName"),
                "hasAttribute" => Some("hasAttribute"),
                "setAttribute" => Some("setAttribute"),
                "removeAttribute" => Some("removeAttribute"),
                "querySelector" => Some("querySelector"),
                "querySelectorAll" => Some("querySelectorAll"),
                "matches" => Some("matches"),
                "closest" => Some("closest"),
                "addEventListener" => Some("addEventListener"),
                "removeEventListener" => Some("removeEventListener"),
                _ => None,
            };
        }
        None
    }

    fn accepts_receiver(&self, object: u32, method: &str, receiver: u32) -> bool {
        if matches!(method,"load"|"pause") && self.media_element_receiver(object) {return self.media_element_receiver(receiver);}
        if matches!(method,"dispatchEvent"|"addEventListener"|"removeEventListener") {
            return (self.event_target_interface(object)||self.dispatch_receiver(object)) && self.dispatch_receiver(receiver);
        }
        if object==DOCUMENT && matches!(method,"createAttribute"|"createAttributeNS"|"createElementNS") {return receiver==DOCUMENT;}
        if object == HISTORY { return receiver == HISTORY && method == "replaceState"; }
        if method == "getBoundingClientRect" { return self.class_list_element_receiver(object) && self.class_list_element_receiver(receiver); }
        if script_named_map::element(object).is_some() { return self.named_map_receiver(object) && self.named_map_receiver(receiver); }
        if method == "getElementsByName" { return object == DOCUMENT && receiver == DOCUMENT; }
        if method=="normalize" {return !self.is_frame_handle(object) && !self.is_frame_handle(receiver)
            && self.namespace_receiver(object) && self.namespace_receiver(receiver);}
        if method=="getElementsByClassName" {
            return if object==DOCUMENT {receiver==DOCUMENT}
                else {self.attr_element_receiver(object) && self.attr_element_receiver(receiver)};
        }
        if script_class_list::element(object).is_some() { return self.class_list_receiver(object) && self.class_list_receiver(receiver); }
        if method == "initMouseEvent" { return self.mouse_receiver(object) && self.mouse_receiver(receiver); }
        if method=="initUIEvent" {
            return (EVENT_BASE..self.state.next_event).contains(&object) && self.ui_receiver(receiver);
        }
        if matches!(method, "initEvent" | "preventDefault" | "stopPropagation" | "stopImmediatePropagation") {
            // First-class methods are created only from genuine Event handles.
            // Issued IDs are monotonic and never reused. Expiry of the source
            // event does not revoke borrowing; the actual receiver must be live.
            return (EVENT_BASE..self.state.next_event).contains(&object) && self.event_receiver(receiver);
        }
        if matches!(method, "getAttributeNS" | "getAttributeNode" | "getAttributeNodeNS" | "removeAttributeNS" | "setAttributeNS" | "removeAttributeNode" | "setAttributeNode" | "setAttributeNodeNS" | "hasAttributes") {
            return self.attr_element_receiver(object) && self.attr_element_receiver(receiver);
        }
        if matches!(method, "isDefaultNamespace" | "compareDocumentPosition") {
            return self.namespace_receiver(object) && self.namespace_receiver(receiver);
        }
        if method == "submit" {
            return self.form_receiver(object) && self.form_receiver(receiver);
        }
        if matches!(method, "item" | "namedItem")
            && (collection_handle_index(object).is_some() || object == DOCUMENT_IMAGES)
        {
            return collection_handle_index(receiver).is_some() || receiver == DOCUMENT_IMAGES;
        }
        if matches!(method, "appendChild" | "insertBefore" | "replaceChild" | "removeChild") {
            let node = |handle| {
                node_index(handle).is_some_and(|node| node > 0 && node < self.doc.nodes.len())
            };
            return node(object) && node(receiver);
        }
        let element = |handle| {
            node_index(handle).is_some_and(|node| {
                self.doc
                    .nodes
                    .get(node)
                    .is_some_and(|node| matches!(node.kind, NodeKind::Element { .. }))
            })
        };
        if element(object) && self.method(object, method).is_some() {
            return element(receiver);
        }
        if node_list_index(object).is_some() {
            return node_list_index(receiver).is_some();
        }
        object == receiver
    }

    fn global_operation_receiver(&self, object: u32, method: &str, receiver: &Value)
        -> Option<Result<u32, (&'static str, String)>> {
        if object != WINDOW || !matches!(method, "requestAnimationFrame" | "cancelAnimationFrame") { return None; }
        Some(match receiver {
            Value::Null | Value::Undefined | Value::Host(WINDOW) => Ok(WINDOW),
            Value::Host(handle) if self.frame_context(*handle).is_some() =>
                Err(("NotSupportedError", "Animation frame scheduling for a child Window is not implemented.".into())),
            _ => Err(("TypeError", "Animation frame operation requires a Window receiver.".into())),
        })
    }

    fn indexed_for_each(&self, object: u32, method: &str) -> bool {
        node_list_index(object).is_some() && method == "forEach"
    }

    fn own_keys(&mut self, object: u32) -> Result<Vec<String>, String> {
        if self.standalone_target_owner(object).is_some() {self.state.work(1)?;return Ok(Vec::new());}
        if script_named_map::element(object).is_some() {
            self.named_map_get(object, "length")?;
            return Err("NamedNodeMap complete indexed/named enumeration is not implemented; use length and item.".into());
        }
        if self.element_expando_receiver(object) {
            self.state.work(1)?;
            return Ok(Vec::new());
        }
        if script_class_list::element(object).is_some() { return self.class_list_keys(object); }
        if self.is_frame_handle(object) { return Err(format!("Blank child native property enumeration is not implemented (handle {object}).")); }
        self.state.work(1)?;
        if collection_handle_index(object).is_some() {
            return self.tag_collection_keys(object);
        }
        if node_list_index(object).is_some() {
            return self.node_list_keys(object);
        }
        if (DATASET_BASE..EVENT_BASE).contains(&object) {
            let node = self.dataset_node(object)?;
            let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else {
                unreachable!();
            };
            if attrs.len() > 4096 {
                return Err("Dataset enumeration limit reached (4096 attributes).".into());
            }
            self.state.work(attrs.len())?;
            self.state
                .copies(attrs.iter().map(|attr| attr.name.bytes()).sum())?;
            return Ok(attrs
                .iter()
                .filter(|attr| attr.name.namespace.is_none())
                .filter_map(|attr| dataset_key(&attr.name.qualified))
                .collect());
        }
        Err("Enumerating this native host object is not implemented.".into())
    }

    fn inherited_keys(&mut self,object:u32) -> Result<Vec<String>,String> {
        if self.element_expando_receiver(object) {
            return Err("Inherited native Element property enumeration is not implemented.".into());
        }
        self.own_keys(object)
    }

    fn own_data(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if self.standalone_target_owner(object).is_some() {self.state.work(1)?;return Ok(None);}
        if script_named_map::element(object).is_some() { return self.named_map_own_data(object, property); }
        if self.attr_receiver(object) { self.state.work(1)?; return Ok(None); }
        if self.event_receiver(object) {
            self.state.work(1)?;
            if property=="isTrusted" { return Err("The unforgeable native Event accessor descriptor is not implemented.".into()); }
            return Ok(None);
        }
        if self.element_expando_receiver(object) {
            // VM-backed expandos are inspected before this host path. These
            // Elements have no indexed/named native own properties; IDL
            // accessors live on their prototype and must not run here.
            self.state.work(1)?;
            return Ok(None);
        }
        if script_class_list::element(object).is_some() {
            if collection_index(property).is_none() { self.class_list_get(object,"length")?; return Ok(None); }
            return self.class_list_get(object,property).map(|value|(!matches!(value,Value::Undefined)).then_some(value));
        }
        if self.is_frame_handle(object) { return Err("Blank child native descriptors are not implemented.".into()); }
        self.state.work(1)?;
        if collection_handle_index(object).is_some() {
            return self.tag_collection_own_data(object, property);
        }
        if node_list_index(object).is_some() {
            let value = self.node_list_get(object, property)?;
            return Ok((collection_index(property).is_some()
                && !matches!(value, Value::Undefined))
            .then_some(value));
        }
        if (DATASET_BASE..EVENT_BASE).contains(&object) {
            return self.dataset_get(object, property).map(|value| {
                if matches!(value, Value::Undefined) {
                    None
                } else {
                    Some(value)
                }
            });
        }
        if object == NAVIGATOR {
            return Ok(None);
        }
        Err(unsupported_own_property(object,property))
    }

    fn delete(&mut self, object: u32, property: &str) -> Result<bool, String> {
        if script_named_map::element(object).is_some() {
            return if collection_index(property).is_some() { self.named_map_has(object, property).map(|present| !present) }
                else { self.named_map_own_data(object, property)?; Ok(true) };
        }
        if script_class_list::element(object).is_some() {
            return if collection_index(property).is_some() { self.class_list_get(object,property).map(|value|matches!(value,Value::Undefined)) }
                else { self.class_list_get(object,"length")?; Ok(true) };
        }
        if self.is_frame_handle(object) { return Err("Blank child native property deletion is not implemented.".into()); }
        self.state.work(1)?;
        if collection_handle_index(object).is_some() {
            return if matches!(property, "length" | "item" | "namedItem") {
                Ok(true)
            } else {
                self.tag_collection_get(object, property)
                    .map(|value| matches!(value, Value::Undefined))
            };
        }
        if node_list_index(object).is_some() {
            return Ok(collection_index(property).is_none()
                || matches!(self.node_list_get(object, property)?, Value::Undefined));
        }
        if (DATASET_BASE..EVENT_BASE).contains(&object) {
            if !matches!(self.dataset_get(object, property)?, Value::Undefined) {
                let node = self.dataset_node(object)?;
                self.attribute(node, &dataset_attribute(property), None)?;
            }
            return Ok(true);
        }
        // Native Navigator attributes live on its fixed prototype. Deleting an
        // absent own property does not remove or alter that native information.
        if object == NAVIGATOR {
            return Ok(true);
        }
        Err("Deleting this native host property is not implemented.".into())
    }
    fn synchronous_event_dispatch(&self,object:u32,method:&str)->bool {method=="dispatchEvent"&&self.dispatch_receiver(object)}
    fn prepare_event_dispatch(&mut self)->Result<(),String>{self.state.work(self.state.created_events.len().checked_mul(5).ok_or("Event validation work overflow.")?)}
    fn begin_event_dispatch(&mut self,object:u32,args:&[Value])->Result<usize,String>{self.begin_synthetic_dispatch(object,args)}
    fn next_event_callback(&mut self,token:usize)->Result<Option<crate::javascript::EventCallback>,String>{self.next_synthetic_callback(token)}
    fn event_callback_result(&mut self,token:usize,returned_false:bool){self.synthetic_callback_result(token,returned_false);}
    fn event_callback_error(&mut self,token:usize,message:&str){self.synthetic_callback_error(token,message);}
    fn finish_event_dispatch(&mut self,token:usize)->bool{self.finish_synthetic_dispatch(token)}
    fn dictionary_argument(
        &self,
        _object: u32,
        method: &str,
    ) -> Option<(usize, &'static [&'static str])> {
        match method {
            "addEventListener" => Some((2, &["capture", "once", "passive", "signal"])),
            "removeEventListener" => Some((2, &["capture"])),
            _ => None,
        }
    }

    fn call_with_dictionary(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        fields: Option<&[(&str, Value)]>,
    ) -> Result<Value, String> {
        if matches!(method, "addEventListener" | "removeEventListener") {
            self.state.work(1)?;
            self.event_listener(object, method, args, fields)
        } else {
            self.call(object, method, args)
        }
    }

    fn date_now(&mut self) -> Result<f64, String> {
        self.state.work(1)?;
        if !self.state.enabled || !self.doc.scripting_enabled {
            return Err("Date clock access is unavailable while scripting is disabled.".into());
        }
        // The page host explicitly grants UTC wall time at whole-millisecond
        // resolution. The language runtime remains default-deny; this does not
        // reveal the OS timezone or change performance.now's monotonic origin.
        Ok(
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(duration) => duration.as_millis() as f64,
                Err(error) => -(error.duration().as_millis() as f64),
            },
        )
    }

    fn date_to_local(&mut self, utc: f64) -> Result<f64, String> {
        self.state.work(256)?;
        if !self.state.enabled || !self.doc.scripting_enabled {
            return Err("Date timezone access is unavailable while scripting is disabled.".into());
        }
        script_timezone::to_local(utc)
    }

    fn date_to_utc(&mut self, local: f64) -> Result<f64, String> {
        self.state.work(256)?;
        if !self.state.enabled || !self.doc.scripting_enabled {
            return Err("Date timezone access is unavailable while scripting is disabled.".into());
        }
        script_timezone::to_utc(local)
    }

    fn random(&mut self) -> Result<f64, String> {
        self.state.work(1)?;
        let mut bytes = [0u8; 8];
        crate::random::fill(&mut bytes)?;
        Ok((u64::from_le_bytes(bytes) >> 11) as f64 / 9_007_199_254_740_992.0)
    }
    fn allow_eval(&mut self) -> Result<bool, String> {
        self.state.work(1)?;
        if !self.state.enabled {
            return Err("String compilation is unavailable while scripting is disabled.".into());
        }
        Ok(self.state.eval_allowed)
    }

    fn owns_global_property(&self, property: &str) -> bool {
        if property=="dispatchEvent"{return true;}
        matches!(
            property,
            "window"
                | "self"
                | "globalThis"
                | "document"
                | "console"
                | "navigator"
                | "clientInformation"
                | "location"
                | "history"
                | "addEventListener"
                | "removeEventListener"
                | "performance"
                | "Image"
        ) || handler_event(property).is_some_and(|event| event != "error")
    }

    fn window_properties(&mut self, window: u32) -> Result<&'static [(&'static str, bool)], String> {
        self.state.work(1)?;
        if window == WINDOW {
            return Ok(&[("window", true), ("self", true), ("globalThis", false),
                ("document", true), ("console", true), ("navigator", true),
                ("clientInformation", true), ("location", true), ("performance", true),
                ("onclick", true), ("oninput", true), ("onchange", true), ("onsubmit", true),
                ("onreset", true), ("onload", true), ("onkeydown", true), ("onkeyup", true)]);
        }
        if self.frame_context(window).is_none() { return Err("Unknown Window key receiver.".into()); }
        self.frame_window_access(Some(WINDOW), window)?;
        Ok(&[("window", true), ("self", true), ("globalThis", false),
            ("document", true), ("location", true), ("parent", true), ("top", true),
            ("frameElement", true), ("closed", true), ("length", true), ("origin", true)])
    }

    fn has(&mut self, object: u32, property: &str) -> Result<bool, String> {
        if property=="dispatchEvent"&&self.dispatch_receiver(object){return Ok(true);}
        self.state.work(1)?;
        if script_named_map::element(object).is_some() { return self.named_map_has(object, property); }
        if property=="normalize" && !self.is_frame_handle(object) && self.namespace_receiver(object) {return Ok(true);}
        if property=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object)) { return Ok(true); }
        if script_class_list::element(object).is_some() {
            if matches!(property,"length"|"value") || script_class_list::method(property).is_some() { self.class_list_get(object,"length")?; return Ok(true); }
            return if collection_index(property).is_some() { self.class_list_get(object,property).map(|value|!matches!(value,Value::Undefined)) } else { Ok(false) };
        }
        if let Some(result) = self.created_event_has(object, property) { return result; }
        if script_attr::index(object).is_some() { return self.attr_has(object, property); }
        if self.is_frame_handle(object) {
            if property=="prototype"&&self.frame_location_owner(object).is_some(){return self.frame_get(object,property).unwrap().map(|value|!matches!(value,Value::Undefined));}
            if matches!(property,"localName"|"prefix"|"namespaceURI") {return self.frame_get(object,property).unwrap().map(|value|!matches!(value,Value::Undefined));}
            if matches!(property, "isDefaultNamespace" | "compareDocumentPosition") && self.namespace_receiver(object) { return Ok(true); }
            return self.frame_get(object, property).unwrap().map(|_| true);
        }
        if property == "submit" && self.form_submit_method(object).is_some() {
            return Ok(true);
        }
        if collection_handle_index(object).is_some() {
            return Ok(matches!(property, "length" | "item" | "namedItem")
                || !matches!(self.tag_collection_get(object, property)?, Value::Undefined));
        }
        if node_list_index(object).is_some() {
            return Ok(matches!(property, "length" | "item" | "forEach")
                || !matches!(self.node_list_get(object, property)?, Value::Undefined));
        }
        if self.ui_receiver(object) && matches!(property,"view"|"detail") {return Ok(true);}
        if (DATASET_BASE..EVENT_BASE).contains(&object) {
            return self
                .dataset_get(object, property)
                .map(|value| !matches!(value, Value::Undefined));
        }
        if object == PERFORMANCE {
            return Ok(property == "now"||property=="timing"&&self.state.navigation_timing.is_some());
        }
        if object==PERFORMANCE_TIMING {return Ok(self.state.navigation_timing.is_some_and(|t|t.value(property).is_some()));}
        if object == HISTORY {
            return match property {
                "state" | "length" | "replaceState" => Ok(true),
                "constructor" | "__proto__" | "scrollRestoration" | "go" | "back" | "forward" | "pushState" => Err(format!("History property {property:?} is not implemented.")),
                _ => Ok(false),
            };
        }
        if object == LOCATION {
            return Ok(self.state.location.as_ref().is_some_and(|location| {
                matches!(property, "toString" | "assign" | "replace" | "reload")
                    || location.property(property).is_some()
            }));
        }
        if object == NAVIGATOR {
            return Ok(matches!(
                property,
                "userAgent"
                    | "appVersion"
                    | "appCodeName"
                    | "appName"
                    | "product"
                    | "vendor"
                    | "platform"
                    | "language"
                    | "cookieEnabled"
                    | "webdriver"
                    | "pdfViewerEnabled"
                    | "javaEnabled"
                    | "plugins"
                    | "mimeTypes"
            ));
        }
        if matches!(object, PLUGINS | MIME_TYPES) {
            return Ok(matches!(property, "length" | "item" | "namedItem")
                || object == PLUGINS && property == "refresh");
        }
        if object == DOCUMENT_IMAGES {
            return if matches!(property, "length" | "item" | "namedItem") {
                Ok(true)
            } else {
                self.image_collection_get(property)
                    .map(|value| !matches!(value, Value::Undefined))
            };
        }
        if (EVENT_BASE..script_class_list::BASE).contains(&object) {
            if !self
                .state
                .event
                .as_ref()
                .is_some_and(|event| event.handle == object)
            {
                return Err("Event handle has expired.".into());
            }
            if self.created_event_method(object, property).is_some() { return Ok(true); }
            let keyboard = self.state.event.as_ref().unwrap().keyboard.is_some();
            return Ok(keyboard && keyboard_property(property)
                || matches!(
                    property,
                    "type"
                        | "target"
                        | "currentTarget"
                        | "cancelable"
                        | "bubbles"
                        | "eventPhase"
                        | "defaultPrevented"
                        | "cancelBubble"
                        | "preventDefault"
                        | "stopPropagation"
                        | "stopImmediatePropagation"
                        | "timeStamp"
                        | "isTrusted"
                ));
        }
        if object >= STYLE_BASE {
            self.style_node(object)?;
            return Ok(matches!(
                property,
                "setProperty" | "getPropertyValue" | "removeProperty"
            ) || css_name(property).is_ok());
        }
        if object == CONSOLE {
            return Ok(matches!(property, "log" | "info" | "warn" | "error"));
        }
        if let Some(event) = self.media_handler_event(object,property) {
            if self.media_element_receiver(object)&&script_media::event_name(event){return Ok(true);}
            return if matches!(event, "load" | "error") {
                Ok(self.handler_target(object, event) == WINDOW
                    || node_index(object).is_some_and(|node| {
                        tag(self.doc, node) == "img"
                            || matches!(tag(self.doc, node), "script" | "link")
                                && self.state.dom.created.contains(&node)
                    }))
            } else {
                self.handler_event_target(object, event)?;
                Ok(true)
            };
        }
        if object == WINDOW {
            if property == "location" {
                return Ok(self.state.location.is_some());
            }
            // Reserved but unavailable APIs remain absent.
            // The interpreter answers ordinary globals and expandos separately.
            return Ok(matches!(
                property,
                "window"
                    | "self"
                    | "globalThis"
                    | "document"
                    | "console"
                    | "navigator"
                    | "clientInformation"
                    | "history"
                    | "addEventListener"
                    | "removeEventListener"
                    | "performance"
                    | "Image"
            ));
        }
        if object == DOCUMENT {
            if dom_tree::node_constant(property).is_some() { return Ok(true); }
            if property == "createEvent" && self.event_factory_enabled() { return Ok(true); }
            if matches!(property, "isDefaultNamespace" | "compareDocumentPosition") { return Ok(true); }
            if matches!(property, "location" | "URL" | "documentURI") {
                return Ok(self.state.location.is_some());
            }
            return Ok(matches!(
                property,
                "title"
                    | "cookie"
                    | "contentType"
                    | "body"
                    | "head"
                    | "images"
                    | "parentElement"
                    | "parentNode"
                    | "ownerDocument"
                    | "documentElement"
                    | "readyState"
                    | "referrer"
                    | "compatMode"
                    | "prerendering"
                    | "getElementById"
                    | "getElementsByName"
                    | "getElementsByTagName"
                    | "querySelector"
                    | "querySelectorAll"
                    | "createElement"
                    | "createElementNS"
                    | "createTextNode"
                    | "createAttribute"
                    | "createAttributeNS"
                    | "addEventListener"
                    | "removeEventListener"
            ) || !matches!(self.named_document_property(property)?, Value::Undefined));
        }
        if let Some(has) = self.dom_has(object, property)? {
            return Ok(has);
        }
        let node = self.node(object)?;
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html && tag(self.doc,node)!="form"
            && matches!(property,"prototype"|"setCapture"|"releaseCapture")
            || property=="animate" && self.element_expando_receiver(object) {
            // Instance prototype/legacy capture and unimplemented animation
            // reads are absent. No capture/animation operation is advertised.
            return Ok(false);
        }
        if tag(self.doc, node) == "img"
            && matches!(
                property,
                "src"
                    | "alt"
                    | "width"
                    | "height"
                    | "naturalWidth"
                    | "naturalHeight"
                    | "complete"
                    | "currentSrc"
            )
        {
            return Ok(true);
        }
        Ok(match property {
            "value" | "form" => self.forms.controls.contains_key(&node),
            "selectedIndex" => self
                .forms
                .controls
                .get(&node)
                .is_some_and(|control| control.kind == Kind::Select),
            "checked" => self
                .forms
                .controls
                .get(&node)
                .is_some_and(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio)),
            _ => {
                matches!(
                    property,
                    "tagName"
                        | "nodeName"
                        | "nodeType"
                        | "parentElement"
                        | "parentNode"
                        | "ownerDocument"
                        | "clientWidth"
                        | "clientHeight"
                        | "clientLeft"
                        | "clientTop"
                        | "textContent"
                        | "style"
                        | "dataset"
                        | "isConnected"
                        | "id"
                        | "className"
                        | "title"
                        | "hidden"
                        | "disabled"
                        | "querySelector"
                        | "querySelectorAll"
                        | "matches"
                        | "closest"
                        | "getAttribute"
                        | "getElementsByTagName"
                        | "hasAttribute"
                        | "setAttribute"
                        | "removeAttribute"
                        | "addEventListener"
                        | "removeEventListener"
                ) || tag(self.doc, node) == "form"
                    && !matches!(self.named_form_property(node, property)?, Value::Undefined)
            }
        })
    }

    fn get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        self.state.work(1)?;
        if script_named_map::element(object).is_some() { return self.named_map_get(object, property); }
        if script_class_list::element(object).is_some() { return self.class_list_get(object,property); }
        if let Some(result) = self.created_event_get(object, property) { return result; }
        if script_attr::index(object).is_some() { return self.attr_get(object, property); }
        if let Some(value) = self.frame_get(object, property) { return value; }
        if collection_handle_index(object).is_some() {
            return self.tag_collection_get(object, property);
        }
        if node_list_index(object).is_some() {
            return self.node_list_get(object, property);
        }
        if (DATASET_BASE..EVENT_BASE).contains(&object) {
            return self.dataset_get(object, property);
        }
        if let Some((_, _, function)) = self
            .state
            .native_methods
            .iter()
            .find(|(target, method, _)| *target == object && *method == property)
        {
            return Ok(function.clone());
        }
        if object == PERFORMANCE {
            return Ok(if property=="timing"&&self.state.navigation_timing.is_some(){Value::Host(PERFORMANCE_TIMING)}else{Value::Undefined});
        }
        if object==PERFORMANCE_TIMING {return self.state.navigation_timing.and_then(|t|t.value(property)).map(|n|Value::Number(n as f64)).ok_or_else(||format!("Unmeasured or unsupported PerformanceTiming member {property:?}."));}
        if object == NAVIGATOR {
            return Ok(match property {
                "userAgent" => Value::String(crate::http::USER_AGENT.into()),
                "appVersion" => Value::String(
                    crate::http::USER_AGENT
                        .split_once('/')
                        .map_or("", |(_, version)| version)
                        .into(),
                ),
                // These three strings are HTML's legacy compatibility constants,
                // not claims that the browser embeds Mozilla or Gecko code.
                "appCodeName" => Value::String("Mozilla".into()),
                "appName" => Value::String("Netscape".into()),
                "product" => Value::String("Gecko".into()),
                "vendor" => Value::String(String::new()),
                "platform" => Value::String(navigator_platform()),
                // The browser UI currently has one fixed English language.
                // No system locale, installed fonts or hardware are consulted.
                "language" => Value::String("en".into()),
                "cookieEnabled" => Value::Bool(true),
                "webdriver" | "pdfViewerEnabled" => Value::Bool(false),
                "plugins" => Value::Host(PLUGINS),
                "mimeTypes" => Value::Host(MIME_TYPES),
                _ => Value::Undefined,
            });
        }
        if matches!(object, PLUGINS | MIME_TYPES) {
            return Ok(if property == "length" {
                Value::Number(0.0)
            } else {
                Value::Undefined
            });
        }
        if object == DOCUMENT_IMAGES {
            return self.image_collection_get(property);
        }
        if object == HISTORY { return self.history_get(property); }
        if object == LOCATION {
            if property=="prototype"{self.location_handle()?;return Ok(Value::Undefined);}
            if property == "valueOf" {
                return Ok(Value::Undefined);
            }
            return self.location_property(property);
        }
        if (EVENT_BASE..script_class_list::BASE).contains(&object) {
            let event = self
                .state
                .event
                .as_ref()
                .filter(|e| e.handle == object)
                .ok_or("Event handle has expired.")?;
            return Ok(match property {
                "type" => Value::String(event.name.clone()),
                "target" => Value::Host(event.target),
                "currentTarget" => Value::Host(event.current),
                "cancelable" => Value::Bool(event.cancelable),
                "bubbles" => Value::Bool(event.bubbles),
                "eventPhase" => Value::Number(f64::from(event.phase)),
                "defaultPrevented" => Value::Bool(event.prevented),
                "cancelBubble" => Value::Bool(event.stopped),
                "timeStamp" => Value::Number(event.timestamp),
                "isTrusted" => Value::Bool(true),
                "view" if event.keyboard.is_some() => Value::Host(WINDOW),
                "detail" if event.keyboard.is_some() => Value::Number(0.0),
                name if event.keyboard.is_some() && keyboard_property(name) => {
                    let key = event.keyboard.as_ref().unwrap();
                    match name {
                        "key" => Value::String(key.key.clone()),
                        "code" => Value::String(key.code.clone()),
                        "location" => Value::Number(f64::from(key.location)),
                        "ctrlKey" => Value::Bool(key.ctrl),
                        "shiftKey" => Value::Bool(key.shift),
                        "altKey" => Value::Bool(key.alt),
                        "metaKey" => Value::Bool(key.meta),
                        "repeat" => Value::Bool(key.repeat),
                        "isComposing" => Value::Bool(false),
                        "keyCode" | "which" => Value::Number(f64::from(key.key_code)),
                        "charCode" => Value::Number(0.0),
                        _ => Value::Undefined,
                    }
                }
                _ => return Err(format!("Event property {property:?} is not supported.")),
            });
        }
        if object >= STYLE_BASE {
            let node = self.style_node(object)?;
            // An empty direct key is an absent ordinary property, not a CSS
            // declaration name. Validate the handle before reporting absence.
            if property.is_empty() { return Ok(Value::Undefined); }
            return self.get_style(node, &css_name(property)?);
        }
        if let Some(value) = self.handler(object, property)? {
            return Ok(value);
        }
        if object == WINDOW {
            if let Some(function) = self.state.native_functions.get(property) {
                return Ok(function.clone());
            }
            return match property {
                "window" | "self" | "globalThis" => Ok(Value::Host(WINDOW)),
                "history" => Ok(Value::Host(HISTORY)),
                "document" => Ok(Value::Host(DOCUMENT)),
                "console" => Ok(Value::Host(CONSOLE)),
                "performance" => Ok(Value::Host(PERFORMANCE)),
                "navigator" | "clientInformation" => Ok(Value::Host(NAVIGATOR)),
                "location" => self.location_handle(),
                "addEventListener" | "removeEventListener" => Err("Reading native method values is not implemented yet; direct calls are supported.".into()),
                _ => Ok(Value::Undefined),
            };
        }
        if object == DOCUMENT {
            if let Some(number) = dom_tree::node_constant(property) {
                return Ok(Value::Number(f64::from(number)));
            }
            return match property {
                "location" => self.location_handle(),
                "cookie" => self.document_cookie(),
                "URL" | "documentURI" => self.location_property("href"),
                "contentType" => {
                    let content_type = self.doc.content_type.clone();
                    self.string(&content_type)
                }
                "title" => {
                    if self.doc.title.len() > self.state.limits.host.string_bytes {
                        return Err(format!("Document title exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
                    }
                    let title = self.doc.title.clone();
                    self.string(&title)
                }
                "body" | "head" | "documentElement" => {
                    self.state.rebuild(self.doc)?;
                    let node = match property {
                        "body" => self.state.tree.body,
                        "head" => self.state.tree.head,
                        _ => self.state.tree.document_element,
                    };
                    Ok(node.map_or(Value::Null, |node| Value::Host(node_handle(node))))
                }
                "readyState" => Ok(Value::String(self.state.ready.into())),
                // This browser does not send Referer or populate a document
                // referrer. Expose the actual empty value of that policy.
                "referrer" => Ok(Value::String(String::new())),
                // ChezCat currently has one standards layout mode and never
                // prerenders a hidden speculative document. Report that actual
                // behavior; quirks-mode layout is not implemented.
                "compatMode" => Ok(Value::String("CSS1Compat".into())),
                "prerendering" => Ok(Value::Bool(false)),
                "images" => Ok(Value::Host(DOCUMENT_IMAGES)),
                "parentElement" | "parentNode" | "ownerDocument" => Ok(Value::Null),
                _ => self.named_document_property(property),
            };
        }
        if let Some(value) = self.dom_get(object, property)? {
            return Ok(value);
        }
        let node = self.node(object)?;
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html && tag(self.doc,node)!="form"
            && matches!(property,"prototype"|"setCapture"|"releaseCapture")
            || property=="animate" && self.element_expando_receiver(object) {
            return Ok(Value::Undefined);
        }
        if let Some(value) = self.image_get(node, property)? {
            return Ok(value);
        }
        match property {
            "tagName" | "nodeName" => self.string(&tag(self.doc, node).to_ascii_uppercase()),
            "nodeType" => Ok(Value::Number(1.0)),
            "isConnected" => Ok(Value::Bool(self.state.tree.active[node])),
            "parentElement" => self.element_parent(node, false),
            "parentNode" => self.element_parent(node, true),
            "ownerDocument" => Ok(Value::Host(DOCUMENT)),
            "clientWidth" | "clientHeight" | "clientLeft" | "clientTop" | "offsetWidth" | "offsetHeight" => {
                let is_root = self.state.tree.document_element == Some(node);
                let offset=matches!(property,"offsetWidth"|"offsetHeight");
                let value = if offset && !self.state.dom.connected[node] {0.0}
                else if let Some(geometry) = &mut self.state.geometry {
                    geometry.read(
                        self.doc,
                        self.forms,
                        node,
                        is_root,
                        property,
                        &mut self.state.host_work,
                    )?
                } else if offset {
                    return Err("Offset dimensions require an attached renderer for this connected Element.".into());
                } else {
                    // A pure script test has no attached viewport/layout box.
                    0.0
                };
                Ok(Value::Number(value))
            }
            "textContent" => Ok(Value::String(self.text_content(node)?)),
            "style" => Ok(Value::Host(STYLE_BASE + node as u32)),
            "dataset" => Ok(Value::Host(DATASET_BASE + node as u32)),
            "id" | "className" | "title" => {
                let name = if property == "className" {
                    "class"
                } else {
                    property
                };
                let value = self.doc.attr(node, name).unwrap_or("");
                if value.len() > self.state.limits.host.string_bytes {
                    return Err(format!("Attribute value exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
                }
                let value = value.to_owned();
                self.string(&value)
            }
            "hidden" | "disabled" => Ok(Value::Bool(self.doc.attr(node, property).is_some())),
            "form" if self.forms.controls.contains_key(&node) => Ok(self.forms.controls[&node]
                .owner
                .map_or(Value::Null, |owner| Value::Host(node_handle(owner)))),
            "checked" | "value" | "selectedIndex" => {
                let control = self
                    .forms
                    .controls
                    .get(&node)
                    .ok_or("This element is not a supported form control.")?;
                match property {
                    "selectedIndex" => {
                        let select = control
                            .select
                            .as_ref()
                            .ok_or("selectedIndex requires a supported select control.")?;
                        Ok(Value::Number(
                            select.selected.map_or(-1.0, |index| index as f64),
                        ))
                    }
                    "checked" => {
                        if matches!(control.kind, Kind::Checkbox | Kind::Radio) {
                            Ok(Value::Bool(control.checked))
                        } else {
                            Err("checked is supported on checkbox/radio controls.".into())
                        }
                    }
                    _ => {
                        let value = control.value.text();
                        self.string(&value)
                    }
                }
            }
            _ if tag(self.doc, node) == "form" => self.named_form_property(node, property),
            // Ordinary DOM nodes have no built-in then method. Recognizing
            // this absence permits the explicitly authorized Promise protocol;
            // form named properties above retain their normal lookup behavior.
            "then" | "" => Ok(Value::Undefined),
            _ => Err(format!("Element property {property:?} is not supported.")),
        }
    }

    fn native_operation_caller(&mut self, window: Option<u32>) {
        self.state.navigation_incumbent = window;
    }

    fn set(&mut self, object: u32, property: &str, value: Value) -> Result<(), String> {
        if property=="currentTime" && self.media_element_receiver(object) {
            let Value::Number(seconds)=value else {return Err("Media time requires numeric conversion.".into());};
            return self.set_media_time(node_index(object).unwrap(),seconds);
        }
        if property=="type"&&self.style_type_receiver(object){let node=self.node(object)?;let text=self.text(&value)?;return self.attribute(node,"type",Some(text));}
        if self.media_element_receiver(object)&&property=="src" {let node=self.node(object)?;let text=self.text(&value)?;return self.attribute(node,"src",Some(text));}
        if script_named_map::element(object).is_some() { self.named_map_get(object, "length")?; return Err("NamedNodeMap property writes are not implemented.".into()); }
        if script_class_list::element(object).is_some() {
            self.state.work(1)?;
            return self.class_list_set(object,property,value);
        }
        if script_attr::index(object).is_some() {
            self.state.work(1)?;
            return self.attr_set(object, property, value);
        }
        if self.is_frame_handle(object) { return Err("Blank child native writes and navigation are not implemented.".into()); }
        self.state.work(1)?;
        if collection_handle_index(object).is_some() {
            return Err("Native HTMLCollection properties are read-only in this subset.".into());
        }
        if (EVENT_BASE..script_class_list::BASE).contains(&object) && property == "cancelBubble" {
            return self.set_cancel_bubble(object, &value);
        }
        if Self::location_string_setter(object, property) {
            return self.set_location_value(object, property, &value);
        }
        if object == DOCUMENT && property == "cookie" {
            return self.set_document_cookie(&value);
        }
        if node_list_index(object).is_some() {
            return Err(
                "Adding arbitrary properties to a native NodeList is not implemented.".into(),
            );
        }
        if (DATASET_BASE..EVENT_BASE).contains(&object) {
            let node = self.dataset_node(object)?;
            let value = self.text(&value)?;
            return self.attribute(node, &dataset_attribute(property), Some(value));
        }
        if matches!(object,PERFORMANCE|PERFORMANCE_TIMING) {
            return Err("Performance is read-only in this subset.".into());
        }
        if object == LOCATION {
            return Err("This Location property is read-only or unsupported.".into());
        }
        if object == DOCUMENT_IMAGES {
            return Err("The document image collection is read-only.".into());
        }
        if matches!(object, NAVIGATOR | PLUGINS | MIME_TYPES) {
            return Err("Navigator and plugin information are read-only in this subset.".into());
        }
        if (EVENT_BASE..script_class_list::BASE).contains(&object) {
            return Err("Event properties are read-only; use preventDefault().".into());
        }
        if object >= STYLE_BASE {
            let node = self.style_node(object)?;
            let value = self.text(&value)?;
            return self.set_style(node, &css_name(property)?, &value, "");
        }
        if self.set_handler(object, property, value.clone())? {
            return Ok(());
        }
        if object == DOCUMENT && property == "title" {
            let value = self.text(&value)?;
            self.state
                .replace_bytes(self.doc.title.len(), value.len())?;
            self.doc.title = value;
            self.state.dom_committed();
            self.state.layout_changed();
            return Ok(());
        }
        if matches!(object, DOCUMENT | WINDOW | CONSOLE) {
            return Err(format!(
                "{} property {property:?} is not writable in this subset.",
                if object == DOCUMENT {
                    "Document"
                } else if object == WINDOW {
                    "Window"
                } else {
                    "Console"
                }
            ));
        }
        if self.dom_set(object, property, &value)? {
            return Ok(());
        }
        let node = self.node(object)?;
        if property == "classList" {
            if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("Class token list requires known namespace provenance.".into()); }
            let value = self.text(&value)?;
            return self.attribute(node,"class",Some(value));
        }
        if property=="srcdoc" && self.iframe_srcdoc_property(node) {
            let text=self.text(&value)?;
            let bytes=text.len();
            return self.attribute(node,"srcdoc",Some(text)).map_err(|error|format!("{error} Converted srcdoc bytes={bytes}."));
        }
        if property == "isMap" && self.html_image_map_property(node) {
            return self.attribute(node, "ismap", value.truthy().then(String::new));
        }
        if property == "border" && self.html_border_property(node) {
            let value = self.text(&value)?;
            return self.attribute(node, "border", Some(value));
        }
        if self.html_spacing_property(node, property) {
            return self.html_spacing_set(node, property, &value);
        }
        if self.image_set(node, property, &value)? {
            return Ok(());
        }
        match property {
            "textContent" => {
                let text = if matches!(value, Value::Null | Value::Undefined) {
                    String::new()
                } else {
                    self.text(&value)?
                };
                self.set_text_content(node, text)
            }
            "id" | "className" | "title" => {
                let value = self.text(&value)?;
                self.attribute(
                    node,
                    if property == "className" {
                        "class"
                    } else {
                        property
                    },
                    Some(value),
                )
            }
            "hidden" | "disabled" => {
                self.attribute(node, property, value.truthy().then(String::new))
            }
            "checked" => {
                self.state.work(self.forms.controls.len())?;
                self.state
                    .copies(self.forms.controls.values().map(|c| c.name.len()).sum())?;
                self.forms.script_set_checked(node, value.truthy())?;
                self.state.layout_changed();
                Ok(())
            }
            "selectedIndex" => {
                if let Value::String(text) = &value {
                    self.state.copies(text.len().saturating_mul(3))?;
                }
                let index = timer_integer(&value)?;
                let control = self
                    .forms
                    .controls
                    .get(&node)
                    .ok_or("This element is not a supported form control.")?;
                let select = control
                    .select
                    .as_ref()
                    .ok_or("selectedIndex requires a supported select control.")?;
                if let Some(option) = usize::try_from(index)
                    .ok()
                    .and_then(|index| select.options.get(index))
                {
                    self.state
                        .copies(option.label.len().saturating_add(option.value.len()))?;
                }
                self.forms.script_set_selected_index(node, index)?;
                self.state.layout_changed();
                Ok(())
            }
            "value" => {
                let value = if matches!(value, Value::Null) {
                    String::new()
                } else {
                    self.text(&value)?
                };
                self.state.work(self.forms.controls.len())?;
                let existing_bytes = self.forms.controls.values().fold(0usize, |bytes, c| {
                    bytes
                        .saturating_add(c.name.len())
                        .saturating_add(c.value.chars.len().saturating_mul(4))
                });
                self.state.copies(existing_bytes)?;
                let kind = self
                    .forms
                    .controls
                    .get(&node)
                    .ok_or("This element is not a supported form control.")?
                    .kind;
                if let Some(select) = &self.forms.controls[&node].select {
                    self.state.work(select.options.len())?;
                    self.state.copies(
                        select
                            .options
                            .iter()
                            .map(|option| option.value.len().saturating_add(option.label.len()))
                            .sum(),
                    )?;
                }
                if !matches!(kind, Kind::Text | Kind::TextArea | Kind::Select) {
                    let old = self.doc.attr(node, "value").map_or(0, str::len);
                    if self
                        .state
                        .retained
                        .saturating_sub(old)
                        .saturating_add(value.len())
                        > self.state.retained_limit
                    {
                        return Err(
                            "DOM retained text/attribute growth limit reached (1 MiB).".into()
                        );
                    }
                    if value.len() > self.state.copy_work {
                        return Err(format!("DOM string work limit reached ({} MiB per job).", self.state.limits.copy_bytes / (1024 * 1024)));
                    }
                }
                self.forms.script_set_value(node, &value)?;
                let control = &self.forms.controls[&node];
                if !matches!(control.kind, Kind::Text | Kind::TextArea | Kind::Select) {
                    self.attribute(node, "value", Some(value))?;
                    if tag(self.doc, node) == "input" {
                        let control = self.forms.controls.get_mut(&node).unwrap();
                        control.caption = forms::caption(self.doc, node, control.kind);
                    }
                }
                self.state.layout_changed();
                Ok(())
            }
            _ => Err(format!(
                "Element property {property:?} is not writable in this subset."
            )),
        }
    }

    fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        if method=="pause" && self.media_element_receiver(object) {return self.pause_media(node_index(object).unwrap());}
        if method=="load" && self.media_element_receiver(object) {
            self.state.work(1)?;
            self.commit_media_source_change(node_index(object).unwrap())?;
            return Ok(Value::Undefined);
        }
        if method=="EventTarget" && self.event_target_interface(object) {return self.create_standalone_target(object);}
        if script_named_map::element(object).is_some() { return self.named_map_call(object, method, args); }
        if method=="normalize" {self.state.work(1)?;return self.normalize_node(object);}
        if script_class_list::element(object).is_some() {
            self.state.work(1)?;
            return self.class_list_call(object,method,args);
        }
        if method == "compareDocumentPosition" {
            self.state.work(1)?;
            return self.compare_document_position(object, args);
        }
        if method == "isDefaultNamespace" {
            self.state.work(1)?;
            return self.is_default_namespace(object, args);
        }
        if self.is_frame_handle(object) { return Err("Blank child native methods are not implemented.".into()); }
        self.state.work(1)?;
        if object == DOCUMENT && method == "createEvent" {
            return if self.event_factory_enabled() { self.create_event(args) }
                else { Err(unsupported_event_interface(args.first())) };
        }
        if object == DOCUMENT && method == "getElementsByName" {
            return self.create_name_list(args.first().ok_or("getElementsByName requires one argument.")?);
        }
        if method=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object)) {
            return self.create_class_collection(object,args.first().ok_or("getElementsByClassName requires one argument.")?);
        }
        if let Some(result) = self.created_event_call(object, method, args) { return result; }
        if (method == "initEvent" || method=="initUIEvent" && self.ui_receiver(object)) && self.state.event.as_ref().is_some_and(|event| event.handle == object) {
            // Ordinary WebIDL conversion has completed. The trusted dispatch
            // flag prevents reinitialization; callback side effects remain.
            return Ok(Value::Undefined);
        }
        if method == "submit" {
            return self.submit_form(object);
        }
        if collection_handle_index(object).is_some() {
            return self.tag_collection_call(object, method, args);
        }
        if (object == DOCUMENT || node_index(object).is_some()) && method == "getElementsByTagName"
        {
            return self.create_tag_collection(
                object,
                args.first()
                    .ok_or("getElementsByTagName requires a tag name.")?,
            );
        }
        if let Some(value) = self.dom_call(object, method, args)? {
            return Ok(value);
        }
        if node_list_index(object).is_some() {
            if method == "item" {
                let index = timer_integer(args.first().ok_or("item requires an index.")?)? as u32;
                let value = self.node_list_get(object, &index.to_string())?;
                return Ok(if matches!(value, Value::Undefined) {
                    Value::Null
                } else {
                    value
                });
            }
            return Err("This NodeList method is not implemented.".into());
        }
        if object == IMAGE_FACTORY && method == "Image" {
            return self.create_image(args);
        }
        if object == NAVIGATOR && method == "sendBeacon" {
            return self.send_beacon(args);
        }
        if object == PERFORMANCE {
            return if method == "now" {
                Ok(Value::Number(self.state.clock_ms(Instant::now())))
            } else {
                Err(format!("Performance method {method:?} is not implemented."))
            };
        }
        if object == LOCATION {
            return self.location_call(method, args);
        }
        if object == HISTORY && method == "replaceState" { return self.history_replace(args); }
        if object == DOCUMENT_IMAGES {
            if args.is_empty() {
                return Err("Image collection methods require one argument.".into());
            }
            let node = match method {
                "item" => {
                    if let Value::String(text) = &args[0] {
                        self.state.copies(text.len())?;
                    }
                    self.image_at(timer_integer(&args[0])? as u32 as usize)?
                }
                "namedItem" => {
                    let name = self.text(&args[0])?;
                    self.image_named(&name)?
                }
                _ => {
                    return Err(format!(
                        "Image collection method {method:?} is not supported."
                    ))
                }
            };
            return Ok(node.map_or(Value::Null, |node| Value::Host(node_handle(node))));
        }
        if object == NAVIGATOR && method == "javaEnabled" {
            return Ok(Value::Bool(false));
        }
        if matches!(object, PLUGINS | MIME_TYPES) {
            return match method {
                "item" => {
                    let value = args.first().ok_or("Plugin item needs an index.")?;
                    if let Value::String(text) = value {
                        self.state.copies(text.len())?;
                    }
                    timer_integer(value)?;
                    Ok(Value::Null)
                }
                "namedItem" => {
                    let value = args.first().ok_or("Plugin namedItem needs a name.")?;
                    self.text(value)?;
                    Ok(Value::Null)
                }
                "refresh" if object == PLUGINS => Ok(Value::Undefined),
                _ => Err(format!(
                    "Plugin collection method {method:?} is not supported."
                )),
            };
        }
        if object == WINDOW {
            if matches!(method, "setTimeout" | "clearTimeout" | "requestAnimationFrame" | "cancelAnimationFrame")
                && (!self.state.enabled || !self.doc.scripting_enabled)
            {
                return Err("Timer capability is unavailable while scripting is disabled.".into());
            }
            match method {
                "setTimeout" => return self.state.set_timeout(args),
                "clearTimeout" => return self.state.clear_timeout(args),
                "requestAnimationFrame" => return self.state.request_animation_frame(args),
                "cancelAnimationFrame" => return self.state.cancel_animation_frame(args),
                _ => {}
            }
        }
        if matches!(method, "querySelector" | "querySelectorAll")
            && (object == DOCUMENT || node_index(object).is_some())
            || matches!(method, "matches" | "closest") && node_index(object).is_some()
        {
            let selector = self.text(
                args.first()
                    .ok_or("Selector methods require a selector string.")?,
            )?;
            return self.selector_query(object, method, &selector);
        }
        if (EVENT_BASE..script_class_list::BASE).contains(&object) {
            if !args.is_empty() {
                return Err("Event methods take no arguments in this subset.".into());
            }
            let event = self
                .state
                .event
                .as_mut()
                .filter(|e| e.handle == object)
                .ok_or("Event handle has expired.")?;
            match method {
                "preventDefault" => {
                    if event.cancelable && !event.passive {
                        event.prevented = true;
                    }
                }
                "stopPropagation" => event.stopped = true,
                "stopImmediatePropagation" => {
                    event.stopped = true;
                    event.immediate = true;
                }
                _ => return Err(format!("Event method {method:?} is not supported.")),
            }
            return Ok(Value::Undefined);
        }
        if object == CONSOLE && matches!(method, "log" | "info" | "warn" | "error") {
            let mut message = String::new();
            for value in args {
                let text = value.to_text();
                self.state.copies(text.len())?;
                if !message.is_empty() {
                    message.push(' ');
                }
                message.push_str(prefix(&text, MAX_LOG_LINE.saturating_sub(message.len())));
            }
            self.state.log(&format!("console.{method}: {message}"));
            return Ok(Value::Undefined);
        }
        if matches!(method, "addEventListener" | "removeEventListener") {
            return self.event_listener(object, method, args, None);
        }
        if object == DOCUMENT && method == "getElementById" {
            if args.is_empty() {
                return Err("getElementById needs one id.".into());
            }
            let id = self.text(&args[0])?;
            self.state.rebuild(self.doc)?;
            return Ok(self
                .state
                .tree
                .ids
                .get(&id)
                .copied()
                .map_or(Value::Null, |node| Value::Host(node_handle(node))));
        }
        if object >= STYLE_BASE {
            let node = self.style_node(object)?;
            if method == "setProperty" && (2..=3).contains(&args.len()) {
                let name = self.text(&args[0])?;
                let name = css_name(&name)?;
                let value = self.text(&args[1])?;
                let priority = args
                    .get(2)
                    .map(|v| self.text(v))
                    .transpose()?
                    .unwrap_or_default();
                self.set_style(node, &name, &value, &priority)?;
                return Ok(Value::Undefined);
            }
            if matches!(method, "getPropertyValue" | "removeProperty") && args.len() == 1 {
                let name = css_name(&self.text(&args[0])?)?;
                let value = self.get_style(node, &name)?;
                if method == "removeProperty" {
                    self.set_style(node, &name, "", "")?;
                }
                return Ok(value);
            }
            return Err(format!(
                "Style method {method:?} is not supported with these arguments."
            ));
        }
        if matches!(object, DOCUMENT | WINDOW | CONSOLE | NAVIGATOR) {
            return Err(format!(
                "{} method {method:?} is not supported in this subset.",
                if object == DOCUMENT {
                    "Document"
                } else if object == WINDOW {
                    "Window"
                } else if object == NAVIGATOR {
                    "Navigator"
                } else {
                    "Console"
                }
            ));
        }
        let node = self.node(object)?;
        if matches!(method, "getAttribute" | "hasAttribute" | "setAttribute" | "removeAttribute") {
            let count = if method == "setAttribute" { 2 } else { 1 };
            if args.len() < count { return Err("Wrong number of attribute arguments.".into()); }
            let text = self.text(&args[0])?;
            self.state.copies(text.len())?;
            let qualified = text.to_ascii_lowercase();
            let slot = self.raw_attribute_slot(node, &qualified)?;
            if matches!(method, "getAttribute" | "hasAttribute") {
                if method == "hasAttribute" { return Ok(Value::Bool(slot.is_some())); }
                let Some(slot) = slot else { return Ok(Value::Null); };
                let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
                let value = &attrs[slot].value;
                if value.len() > self.state.limits.host.string_bytes { return Err("Attribute value exceeds the host string bound.".into()); }
                self.state.copies(value.len())?;
                return Ok(Value::String(value.to_owned()));
            }
            let name = if let Some(slot) = slot { self.copy_attribute_name(node, slot)? }
                else { crate::html::AttributeName::unqualified(qualified) };
            let value = if method == "setAttribute" { Some(self.text(&args[1])?) } else { None };
            self.public_attribute(node, name, value)?;
            return Ok(Value::Undefined);
        }
        Err(format!("DOM method {method:?} is not supported."))
    }
}

fn node_handle(node: usize) -> u32 {
    NODE_BASE + node as u32
}

// Diagnostics only: no event object, permission, coercion or dispatch is added.
#[cold]
#[inline(never)]
fn unsupported_event_interface(value: Option<&Value>) -> String {
    let (class, bytes) = match value {
        Some(Value::String(name)) => {
            let class = [
                ("Event", "base-event"), ("Events", "base-event"),
                ("HTMLEvents", "base-event"), ("SVGEvents", "base-event"),
                ("UIEvent", "ui-event"), ("UIEvents", "ui-event"),
                ("MouseEvent", "mouse-event"), ("MouseEvents", "mouse-event"),
                ("KeyboardEvent", "keyboard-event"), ("KeyboardEvents", "keyboard-event"),
                ("CustomEvent", "custom-event"), ("TouchEvent", "touch-event"),
                ("PointerEvent", "pointer-event"), ("MutationEvent", "mutation-event"),
                ("MutationEvents", "mutation-event"),
            ].into_iter().find_map(|(known, class)| name.eq_ignore_ascii_case(known).then_some(class)).unwrap_or("other");
            (class, Some(name.len()))
        }
        Some(_) => ("non-string", None),
        None => ("missing", None),
    };
    format!("Document method \"createEvent\" is not supported in this subset. Requested interface class={class}; UTF-8 bytes={bytes:?}. Diagnostic only; no event was created.")
}
fn inert_attribute_name(name: &str) -> bool {
    (name.starts_with("data-") || name.starts_with("aria-") && name.len() > 5)
        && name.len() <= 256
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b':' | b'.')
        })
}

#[cold]
#[inline(never)]
fn unsupported_own_property(object:u32, property:&str) -> String {
    format!("Own-property inspection is not implemented for this host object. Native handle={object}; property={property:?}.")
}
fn collection_index(property: &str) -> Option<usize> {
    if property.is_empty() || property.len() > 1 && property.starts_with('0') {
        return None;
    }
    let index = property.parse::<u32>().ok()?;
    (index != u32::MAX && index.to_string() == property).then_some(index as usize)
}

fn node_index(handle: u32) -> Option<usize> {
    (NODE_BASE..STYLE_BASE)
        .contains(&handle)
        .then(|| (handle - NODE_BASE) as usize)
}
fn tag(doc: &Document, node: usize) -> &str {
    match doc.nodes.get(node).map(|n| &n.kind) {
        Some(NodeKind::Element { tag, .. }) => tag,
        _ => "",
    }
}
fn same_function(a: &Value, b: &Value) -> bool {
    matches!((a,b), (Value::Function(a), Value::Function(b)) if a == b)
}
fn keyboard_property(property: &str) -> bool {
    matches!(
        property,
        "key"
            | "code"
            | "location"
            | "ctrlKey"
            | "shiftKey"
            | "altKey"
            | "metaKey"
            | "repeat"
            | "isComposing"
            | "keyCode"
            | "which"
            | "charCode"
    )
}

fn handler_event(property: &str) -> Option<&str> {
    match property {
        "onclick" | "oninput" | "onchange" | "onsubmit" | "onreset" | "onload" | "onkeydown"
        | "onkeyup" | "onerror" => Some(&property[2..]),
        _ => None,
    }
}
fn text_container(tag: &str) -> bool {
    matches!(
        tag,
        "div"
            | "span"
            | "p"
            | "a"
            | "b"
            | "strong"
            | "i"
            | "em"
            | "u"
            | "s"
            | "small"
            | "mark"
            | "sub"
            | "sup"
            | "code"
            | "pre"
            | "kbd"
            | "samp"
            | "var"
            | "label"
            | "button"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "li"
            | "dt"
            | "dd"
            | "legend"
            | "blockquote"
            | "br"
            | "wbr"
    )
}
fn node_bytes(node: &Node) -> usize {
    match &node.kind {
        NodeKind::Text(text) => text.len(),
        NodeKind::Element { id, attrs, .. } => {
            id.as_ref().map_or(0, String::len)
                + attrs.iter().map(crate::html::Attribute::bytes).sum::<usize>()
        }
    }
}
fn document_bytes(doc: &Document) -> usize {
    doc.title.len() + doc.nodes.iter().map(node_bytes).sum::<usize>()
}
fn prefix(text: &str, maximum: usize) -> &str {
    let mut end = text.len().min(maximum);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
fn css_name(name: &str) -> Result<String, String> {
    if name.len() > 64 {
        return Err("CSS property name exceeds 64 bytes.".into());
    }
    let mut css = String::new();
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            css.push('-');
            css.push(ch.to_ascii_lowercase());
        } else {
            css.push(ch);
        }
    }
    if !matches!(
        css.as_str(),
        "display"
            | "overflow"
            | "position"
            | "inset"
            | "top"
            | "right"
            | "bottom"
            | "left"
            | "color"
            | "background"
            | "background-color"
            | "font"
            | "font-family"
            | "font-size"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-decoration-line"
            | "text-align"
            | "line-height"
            | "white-space"
            | "width"
            | "height"
            | "min-width"
            | "max-width"
            | "min-height"
            | "max-height"
            | "margin"
            | "margin-top"
            | "margin-right"
            | "margin-bottom"
            | "margin-left"
            | "padding"
            | "padding-top"
            | "padding-right"
            | "padding-bottom"
            | "padding-left"
            | "border"
            | "border-width"
            | "border-color"
            | "border-style"
            | "box-sizing"
            | "vertical-align"
            | "flex"
            | "flex-grow"
            | "flex-shrink"
            | "flex-basis"
            | "flex-direction"
            | "flex-wrap"
            | "justify-content"
            | "align-items"
            | "align-self"
            | "gap"
            | "row-gap"
            | "column-gap"
            | "order"
    ) {
        return Err(format!(
            "CSS property {name:?} is not supported by the script bridge."
        ));
    }
    Ok(css)
}
fn validate_css_value(value: &str, allow_font_quotes: bool, string_limit: usize) -> Result<(), String> {
    if value.len() > string_limit
        || value.chars().any(|ch| {
            ch.is_control() && !ch.is_ascii_whitespace()
                || matches!(ch, ';' | '{' | '}' | '\\' | '@' | '!')
                || !allow_font_quotes && matches!(ch, '\'' | '"')
        })
        || value.to_ascii_lowercase().contains("url(")
    {
        return Err(
            "CSS mutation accepts simple values without URLs, escapes or extra declarations."
                .into(),
        );
    }
    if allow_font_quotes {
        let mut quote = None;
        for ch in value.chars() {
            if let Some(open) = quote {
                if ch == open {
                    quote = None;
                } else if matches!(ch, '\n' | '\r' | '\u{c}') {
                    return Err("A quoted font family cannot contain a line ending.".into());
                }
            } else if matches!(ch, '\'' | '"') {
                quote = Some(ch);
            }
        }
        if quote.is_some() {
            return Err("A quoted font family must have a matching closing quote.".into());
        }
    }
    Ok(())
}
fn validate_style(style: &str, string_limit: usize) -> Result<(), String> {
    for item in style.split(';').filter(|item| !item.trim().is_empty()) {
        let (name, value) = item
            .split_once(':')
            .ok_or("Style declaration needs a colon.")?;
        let name = css_name(name.trim())?;
        validate_css_value(
            value.trim(),
            matches!(name.as_str(), "font" | "font-family"),
            string_limit,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "script_compat_tests.rs"]
mod compatibility_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    fn setup(source: &str, scripts: &[&str]) -> (Document, Forms, Page, Outcome) {
        setup_at(source, scripts, None)
    }

    fn setup_at(
        source: &str,
        scripts: &[&str],
        location: Option<&crate::navigation::Location>,
    ) -> (Document, Forms, Page, Outcome) {
        let mut doc = html::parse(source);
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let scripts: Vec<_> = scripts
            .iter()
            .enumerate()
            .map(|(node, source)| Script {
                node,
                source: (*source).into(),
                label: format!("test script {node}"),
            })
            .collect();
        let mut page = Page::for_test();
        if let Some(location) = location {
            page.set_location(location);
        }
        let result = page.start(&mut doc, &mut forms, &scripts);
        (doc, forms, page, result)
    }

    fn id(doc: &Document, id: &str) -> usize {
        doc.nodes
            .iter()
            .enumerate()
            .find(|(node, _)| doc.attr(*node, "id") == Some(id))
            .unwrap()
            .0
    }

    fn text(doc: &Document, node: usize) -> String {
        let mut text = String::new();
        let mut stack = vec![node];
        while let Some(node) = stack.pop() {
            if let NodeKind::Text(value) = &doc.nodes[node].kind {
                text.push_str(value);
            }
            stack.extend(doc.nodes[node].children.iter().rev().copied());
        }
        text
    }

    fn click(page: &mut Page, doc: &mut Document, forms: &mut Forms, node: usize) -> Outcome {
        page.begin_job();
        page.dispatch(doc, forms, node, "click", true)
    }

    #[test]
    fn symbol_dom_setters_throw_before_mutating_text_attributes_or_controls() {
        let (doc, forms, page, start) = setup(
            "<head><title>Stable</title></head><body><p id=p class=before title=caption style='color: blue'>Before</p><input id=field value=original><textarea id=area>seed</textarea><select id=choice><option value=a>A</option><option value=b>B</option></select></body>",
            &[r#"
var symbol=Symbol('blocked'), caught=0;
function reject(action){try{action();}catch(e){if(e.name==='TypeError'){caught++;return;}}throw Error('expected TypeError');}
var p=document.getElementById('p'), field=document.getElementById('field');
var area=document.getElementById('area'), choice=document.getElementById('choice');
var text=document.createTextNode('Text'), script=document.createElement('script');
var checks=[
 function(){document.title=symbol;}, function(){p.textContent=symbol;},
 function(){p.id=symbol;}, function(){p.className=symbol;}, function(){p.title=symbol;},
 function(){p.style.color=symbol;}, function(){field.value=symbol;},
 function(){area.value=symbol;}, function(){choice.value=symbol;}, function(){choice.selectedIndex=symbol;},
 function(){text.data=symbol;}, function(){text.nodeValue=symbol;}, function(){text.textContent=symbol;},
 function(){script.text=symbol;}, function(){script.textContent=symbol;},
 function(){p.setAttribute('title',symbol);}, function(){p.dataset.note=symbol;},
 function(){script.src=symbol;}
];
checks.forEach(reject);
if(caught!==checks.length||document.title!=='Stable'||p.textContent!=='Before')throw Error('text changed');
if(p.id!=='p'||p.className!=='before'||p.title!=='caption'||p.style.color!=='blue')throw Error('attribute changed');
if(field.value!=='original'||area.value!=='seed'||choice.value!=='a'||choice.selectedIndex!==0)throw Error('control changed');
if(text.data!=='Text'||script.text!==''||script.src!==''||p.getAttribute('data-note')!==null)throw Error('detached value changed');
"#],
        );
        assert_eq!(start.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Stable");
        assert_eq!(text(&doc, id(&doc, "p")), "Before");
        assert_eq!(forms.controls[&id(&doc, "field")].value.text(), "original");
        assert!(!page.has_host_tasks());
        assert!(page.state.timers.is_empty());
    }

    #[test]
    fn symbol_host_call_arguments_throw_without_timer_listener_or_style_side_effects() {
        let (doc, _, page, start) = setup(
            "<head><title>Stable</title></head><body><p id=p style='color: blue'>Calls</p><img id=picture></body>",
            &[r#"
var symbol=Symbol('blocked'), caught=0, p=document.getElementById('p');
function reject(action){try{action();}catch(e){if(e.name==='TypeError'){caught++;return;}}throw Error('expected TypeError');}
function callback(){};
setTimeout(callback,60000); requestAnimationFrame(callback); p.addEventListener('click',callback);
var checks=[
 function(){setTimeout(callback,symbol);}, function(){setTimeout(symbol,0);},
 function(){clearTimeout(symbol);}, function(){requestAnimationFrame(symbol);}, function(){cancelAnimationFrame(symbol);},
 function(){p.addEventListener(symbol,callback);}, function(){p.removeEventListener(symbol,callback);},
 function(){p.addEventListener('click',symbol);}, function(){p.removeEventListener('click',symbol);},
 function(){p.style.setProperty(symbol,'red');}, function(){p.style.setProperty('color',symbol);},
 function(){p.style.setProperty('color','red',symbol);}, function(){p.style.getPropertyValue(symbol);}, function(){p.style.removeProperty(symbol);},
 function(){navigator.plugins.item(symbol);}, function(){navigator.plugins.namedItem(symbol);},
 function(){navigator.mimeTypes.item(symbol);}, function(){navigator.mimeTypes.namedItem(symbol);},
 function(){document.images.item(symbol);}, function(){document.images.namedItem(symbol);}
];
checks.forEach(reject);
if(caught!==checks.length||p.style.color!=='blue')throw Error('call changed state');
"#],
        );
        assert_eq!(start.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Stable");
        assert_eq!(doc.attr(id(&doc, "p"), "style"), Some("color: blue"));
        assert_eq!(page.state.timers.len(), 1);
        assert_eq!(page.state.next_timer, 1);
        assert_eq!(page.state.animation_frames.len(), 1);
        assert_eq!(page.state.next_animation_frame, 1);
        assert_eq!(page.state.listeners.len(), 1);
        assert!(!page.has_host_tasks());
    }

    #[test]
    fn symbol_navigation_and_resource_conversions_do_not_queue_requests() {
        let location = crate::navigation::address(
            "https://example.test/search?q=first",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        let (doc, _, mut page, start) = setup_at(
            "<head><title>Stable</title></head><body><p>Navigation</p></body>",
            &[r#"
var symbol=Symbol('blocked'), caught=0;
function reject(action){try{action();}catch(e){if(e.name==='TypeError'){caught++;return;}}throw Error('expected TypeError');}
var image=new Image(), link=document.createElement('link'), script=document.createElement('script');
var checks=[
 function(){location.href=symbol;}, function(){location.search=symbol;}, function(){location.hash=symbol;},
 function(){location.assign(symbol);}, function(){location.replace(symbol);},
 function(){location=symbol;}, function(){window.location=symbol;}, function(){document.location=symbol;}, function(){document.cookie=symbol;},
 function(){image.src=symbol;}, function(){image.alt=symbol;}, function(){image.width=symbol;}, function(){image.height=symbol;},
 function(){new Image(symbol);}, function(){new Image(1,symbol);},
 function(){link.href=symbol;}, function(){link.rel=symbol;}, function(){script.src=symbol;}
];
checks.forEach(reject);
if(caught!==checks.length||location.href!=='https://example.test/search?q=first')throw Error('navigation changed');
if(image.src!==''||image.alt!==''||image.width!==0||image.height!==0||link.href!==''||link.rel!==''||script.src!=='')throw Error('resource changed');
"#],
            Some(&location),
        );
        assert_eq!(start.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Stable");
        assert!(page.take_navigation().is_none());
        assert_eq!(page.state.navigation_requests, 0);
        assert_eq!(page.state.declined_images, 0);
        assert!(!page.has_host_tasks());
    }

    #[test]
    fn symbol_rejection_preserves_null_boolean_and_timer_argument_behavior() {
        let (mut doc, mut forms, mut page, start) = setup(
            "<body><p id=p>Before</p><input id=field value=original><textarea id=area>seed</textarea><input id=check type=checkbox><select id=choice><option value=a>A</option><option value=b selected>B</option></select></body>",
            &[r#"
var symbol=Symbol('timer token'), p=document.getElementById('p'), field=document.getElementById('field');
var area=document.getElementById('area'), check=document.getElementById('check'), choice=document.getElementById('choice');
p.hidden=symbol; field.disabled=symbol; check.checked=symbol;
if(!p.hidden||!field.disabled||!check.checked)throw Error('boolean Symbol');
document.title=null; p.title=null; p.textContent=null; field.value=null; area.value=null;
if(document.title!=='null'||p.title!=='null'||p.textContent!==''||field.value!==''||area.value!=='')throw Error('null conversion');
p.textContent=undefined; field.value=undefined; choice.selectedIndex=null;
var text=document.createTextNode('old'); text.nodeValue=null;
if(text.data!==''||p.textContent!==''||field.value!=='undefined'||choice.value!=='a')throw Error('existing defaults');
text.data=null; if(text.data!=='null')throw Error('data null');
text.textContent=undefined; if(text.data!=='undefined')throw Error('text undefined');
p.addEventListener('click',function(){},symbol);
setTimeout(function(value){if(value!==symbol||typeof value!=='symbol')throw Error('argument identity');document.title=value.description;},0,symbol);
"#],
        );
        assert_eq!(start.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "null");
        assert_eq!(page.state.listeners.len(), 1);
        assert!(page.state.listeners[0].options.capture);
        assert_eq!(page.state.timers.len(), 1);
        assert!(matches!(
            page.state.timers[0].arguments.as_slice(),
            [Value::Symbol(_)]
        ));
        let outcome = page
            .poll_timer_at(
                &mut doc,
                &mut forms,
                Instant::now() + Duration::from_millis(1),
            )
            .unwrap();
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "timer token");
        assert!(page.state.timers.is_empty());
        assert_eq!(page.state.timer_bytes, 0);

        // The raw bridge helpers are also used by native tests/callers. They
        // must fail rather than serialize the runtime's diagnostic label.
        assert!(timer_integer(&Value::Symbol(0))
            .unwrap_err()
            .contains("Symbol"));
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        assert!(bridge
            .text(&Value::Symbol(0))
            .unwrap_err()
            .contains("Symbol"));
        assert!(bridge.set(DOCUMENT, "title", Value::Symbol(0)).is_err());
        assert_eq!(bridge.doc.title, "timer token");
    }

    #[test]
    fn animation_frames_snapshot_cancel_share_timestamps_and_wait_for_next_frame() {
        let (mut doc, mut forms, mut page, start) = setup(
            "<body><p>Frames</p></body>",
            &[r#"
var order='', stamp=-1, last=-1;
if(performance!==window.performance || typeof performance.now!=='function')throw Error('clock');
var p=performance.now(); if(performance.now()<p)throw Error('nonmonotonic');
requestAnimationFrame(function(t){order+='a';stamp=t;cancelAnimationFrame(cancelled);
 requestAnimationFrame(function(next){if(next<stamp)throw Error('time reversed');order+='r';document.title=order;});
});
var cancelled=requestAnimationFrame(function(){throw Error('cancelled callback ran');});
requestAnimationFrame(function(t){if(t!==stamp)throw Error('frame timestamps differ');order+='d';document.title=order;});
setTimeout(function(){order+='t';},0);
"#],
        );
        assert_eq!(start.errors, 0, "{}", page.report());
        let due = page.state.animation_due.unwrap();
        page.poll_timer_at(&mut doc, &mut forms, due).unwrap();
        assert_eq!(page.state.timer_callbacks, 1);
        let outcome = page.poll_timer_at(&mut doc, &mut forms, due).unwrap();
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "tad");
        assert_eq!(page.state.animation_callbacks, 2);
        assert_eq!(page.state.animation_frames.len(), 1);
        assert!(page.poll_timer_at(&mut doc, &mut forms, due).is_none());
        let next = page.state.animation_due.unwrap();
        assert!(next >= due + FRAME_INTERVAL);
        // Animation frames use a per-frame budget and don't inherit the
        // document's separate finite timeout-callback allowance.
        page.state.timer_callbacks = MAX_TIMER_CALLBACKS;
        let outcome = page.poll_timer_at(&mut doc, &mut forms, next).unwrap();
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "tadr");
        assert!(page.next_timer_deadline().is_none());
    }

    #[test]
    fn animation_frames_bound_queue_and_work_and_stop_with_the_document() {
        let (_, _, mut page, start) = setup(
            "<body></body>",
            &["for(var i=0;i<65;i++)requestAnimationFrame(function(){});"],
        );
        assert_eq!(start.errors, 1);
        assert_eq!(page.state.animation_frames.len(), MAX_ANIMATION_FRAMES);
        page.stop_timers("Stopped by user.");
        assert!(page.next_timer_deadline().is_none());
        let (_, _, _, start) = setup("<body></body>", &["requestAnimationFrame('bad');"]);
        assert_eq!(start.errors, 1);
        let (mut doc, mut forms, mut page, start) = setup(
            "<body></body>",
            &["requestAnimationFrame(function(){while(true){}});"],
        );
        assert_eq!(start.errors, 0);
        let due = page.state.animation_due.unwrap();
        let outcome = page.poll_timer_at(&mut doc, &mut forms, due).unwrap();
        assert_eq!(outcome.errors, 1);
        assert!(page.report().contains("limit"));
        assert!(page.next_timer_deadline().is_none());
        // A new explicit inspector job still has a fresh work allowance.
        let result = page.console_eval(&mut doc, &mut forms, "document.title='Recovered'");
        assert_eq!(result.errors, 0);
        assert_eq!(doc.title, "Recovered");
    }

    #[test]
    fn native_keyboard_events_bubble_cancel_and_expose_actual_input_values() {
        let (mut doc, mut forms, mut page, start) = setup(
            "<html><body><input id=field></body></html>",
            &[r#"
var order='', saved;
window.addEventListener('keydown',function(e){order+='w';if(e.eventPhase!==1)throw Error('capture');},true);
document.getElementById('field').onkeydown=function(e){
 if(e.key!=='A' || e.code!=='KeyA' || !e.shiftKey || e.ctrlKey || e.altKey || e.metaKey || !e.repeat || e.location!==0)throw Error('key fields');
 if(e.keyCode!==65 || e.which!==65 || e.charCode!==0 || !e.isTrusted || e.isComposing || e.timeStamp<0)throw Error('event fields');
 if(!('key' in e) || !('timeStamp' in e))throw Error('presence');
 saved=e;order+='i';return false;
};
document.addEventListener('keydown',function(e){if(!e.defaultPrevented)throw Error('cancellation');order+='d';document.title=order;});
document.onkeyup=function(e){if(e.key!=='A' || e.type!=='keyup')throw Error('release');
 document.title+='u';};
"#],
        );
        assert_eq!(start.errors, 0, "{}", page.report());
        let input = KeyboardInput {
            down: true,
            target: Some(id(&doc, "field")),
            key: "A".into(),
            code: "KeyA".into(),
            shift: true,
            repeat: true,
            key_code: 65,
            ..KeyboardInput::default()
        };
        page.begin_job();
        let outcome = page.keyboard(&mut doc, &mut forms, input.clone());
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(outcome.prevented);
        assert_eq!(doc.title, "wid");
        page.begin_job();
        let outcome = page.keyboard(
            &mut doc,
            &mut forms,
            KeyboardInput {
                down: false,
                repeat: false,
                ..input
            },
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "widu");
        let expired = page.console_eval(&mut doc, &mut forms, "saved.preventDefault()");
        assert_eq!(expired.errors, 1);
        assert!(page.report().contains("Event handle has expired"));
        doc.scripting_enabled = false;
        let outcome = page.keyboard(&mut doc, &mut forms, KeyboardInput::default());
        assert_eq!(outcome.errors, 0);
        assert_eq!(doc.title, "widu");
    }

    #[test]
    fn inspector_console_uses_the_live_realm_and_preserves_job_bounds() {
        let (mut doc, mut forms, mut page, start) =
            setup("<body><p id=out>old</p></body>", &["var count=1;"]);
        assert_eq!(start.errors, 0);
        let result = page.console_eval(
            &mut doc,
            &mut forms,
            "count+=2;document.getElementById('out').textContent=count;count",
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        assert!(result.layout_changed);
        assert_eq!(text(&doc, id(&doc, "out")), "3");
        assert!(page.report().contains("=> 3"));
        let result = page.console_eval(&mut doc, &mut forms, "while(true){}");
        assert_eq!(result.errors, 1);
        assert!(page.report().contains("limit"));
        page.clear_console();
        assert_eq!(page.error_count(), 1);
        let result = page.console_eval(&mut doc, &mut forms, "count+4");
        assert_eq!(result.errors, 0, "{}", page.report());
        assert!(page.report().contains("=> 7"));
        page.set_location(&crate::navigation::Location::home());
        let result = page.console_eval(
            &mut doc,
            &mut forms,
            "location.href='https://example.test/'",
        );
        assert_eq!(result.errors, 1);
        let result = page.console_eval(&mut doc, &mut forms, "location.href");
        assert_eq!(result.errors, 0);
        assert!(page.report().contains("about:home"));
    }

    #[test]
    fn inspector_console_obeys_script_and_csp_settings_and_source_limit() {
        let mut doc = html::parse("<body><p id=out>unchanged</p></body>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let mut policy = crate::csp::PolicySet::default();
        policy.add_header("script-src 'self'");
        page.start_with_policy(&mut doc, &mut forms, &[], &policy);
        assert_eq!(
            page.console_eval(&mut doc, &mut forms, "document.title='denied'")
                .errors,
            1
        );
        assert_ne!(doc.title, "denied");
        assert!(page.report().contains("Content Security Policy"));
        page.start(&mut doc, &mut forms, &[]);
        assert_eq!(
            page.console_eval(&mut doc, &mut forms, &";".repeat(MAX_CONSOLE_SOURCE + 1))
                .errors,
            1
        );
        doc.scripting_enabled = false;
        assert_eq!(
            page.console_eval(&mut doc, &mut forms, "document.title='denied'")
                .errors,
            1
        );
        assert_ne!(doc.title, "denied");
    }

    #[test]
    fn location_reads_serialized_components_and_stable_native_identity() {
        let script = r#"
var l=location;
if(l!==window.location || l!==document.location || l!==self.location || l!==globalThis.location)throw Error('location identity');
if(document.URL!==l.href || document.documentURI!==l.href)throw Error('document URL');
if(!('location' in window) || !('location' in document) || !('URL' in document) || !('documentURI' in document))throw Error('URL presence');
if('baseURI' in document || !('assign' in l) || !('replace' in l) || !('reload' in l))throw Error('Location capability mismatch');
var stringify=l.toString;
if(typeof stringify!=='function' || stringify!==l.toString || stringify.call(l)!==l.href)throw Error('native string method');
var rejected=false;try{stringify.call(window);}catch(e){rejected=e.name==='TypeError';}
if(!rejected)throw Error('incorrect receiver');
rejected=false;try{stringify();}catch(e){rejected=e.name==='TypeError';}
if(!rejected)throw Error('detached receiver');
document.title=[l.href,l.protocol,l.host,l.hostname,l.port,l.pathname,l.search,l.hash,l.origin].join('|');
"#;
        for (url, expected) in [
            (
                "http://EXAMPLE.com:80/a%2Fb/c?q=a%20b&x=%23#snow%20%E2%98%83",
                "http://example.com/a%2Fb/c?q=a%20b&x=%23#snow%20%E2%98%83|http:|example.com|example.com||/a%2Fb/c|?q=a%20b&x=%23|#snow%20%E2%98%83|http://example.com",
            ),
            (
                "https://[2001:0db8::1]:8443/a?value=%2F#x%2Fy",
                "https://[2001:db8::1]:8443/a?value=%2F#x%2Fy|https:|[2001:db8::1]:8443|[2001:db8::1]|8443|/a|?value=%2F|#x%2Fy|https://[2001:db8::1]:8443",
            ),
            (
                "https://[::1]:443/?#",
                "https://[::1]/?|https:|[::1]|[::1]||/|||https://[::1]",
            ),
            (
                "about:home#section%20two",
                "about:home#section%20two|about:||||home||#section%20two|null",
            ),
        ] {
            let location = crate::navigation::address(url, &crate::navigation::Location::home()).unwrap();
            let (doc, _, page, result) = setup_at("<html><body></body></html>", &[script], Some(&location));
            assert_eq!(result.errors, 0, "{url}: {}", page.report());
            assert_eq!(doc.title, expected, "{url}");
        }
        for (path, expected_path) in [
            ("/site/a b#%.html", "/site/a%20b%23%25.html"),
            ("C:/site/a b.html", "/C:/site/a%20b.html"),
        ] {
            let location = crate::navigation::Location {
                path: Some(std::path::PathBuf::from(path)),
                fragment: "part #one".into(),
                root: Some(std::path::PathBuf::from("/private/content-root")),
                url: None,
            };
            let (doc, _, page, result) =
                setup_at("<html><body></body></html>", &[script], Some(&location));
            assert_eq!(result.errors, 0, "{}", page.report());
            assert_eq!(doc.title, format!("file://{expected_path}#part%20%23one|file:||||{expected_path}||#part%20%23one|null"));
            assert!(!doc.title.contains("private"));
        }
    }

    #[test]
    fn location_native_handle_stays_live_after_trusted_context_updates() {
        let first = crate::navigation::address(
            "https://example.test/path?q=one#first",
            &crate::navigation::Location::home(),
        )
        .unwrap();
        let (mut doc, mut forms, mut page, result) = setup_at(
            "<html><body><button id=read type=button>Read</button></body></html>",
            &[r#"var saved=location, stringify=location.toString;
document.getElementById('read').onclick=function(){
if(saved!==location || stringify!==location.toString)throw Error('identity changed');
document.title=stringify.call(saved)+'|'+document.URL+'|'+document.documentURI;
};"#],
            Some(&first),
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        let next = crate::navigation::address("#next%20part", &first).unwrap();
        page.set_location(&next);
        let button = id(&doc, "read");
        assert_eq!(
            click(&mut page, &mut doc, &mut forms, button).errors,
            0,
            "{}",
            page.report()
        );
        assert_eq!(doc.title, vec![next.display(); 3].join("|"));
    }

    #[test]
    fn location_requires_real_context_and_rejects_navigation_and_host_redefinition() {
        for source in [
            "location.href;",
            "window.location;",
            "document.location;",
            "document.URL;",
            "document.documentURI;",
        ] {
            let (_, _, page, result) = setup("<body></body>", &[source]);
            assert_eq!(result.errors, 1, "{source}: {}", page.report());
            assert!(
                page.report()
                    .contains("unavailable without a committed URL context"),
                "{}",
                page.report()
            );
        }
        let location = crate::navigation::Location::home();
        for source in [
            "location='https://else.invalid/';",
            "window.location='https://else.invalid/';",
            "document.location='https://else.invalid/';",
            "location.href='https://else.invalid/';",
            "location.hash='#changed';",
            "location.toString=function(){return 'spoof';};",
            "document.URL='spoof';",
            "document.documentURI='spoof';",
            "location.assign('https://else.invalid/');",
            "location.replace('https://else.invalid/');",
            "location.reload();",
            "Object.defineProperty(window,'location',{value:'spoof'});",
            "Object.defineProperty(location,'href',{value:'spoof'});",
            "Object.setPrototypeOf(location,{href:'spoof'});",
            "delete location.href;",
        ] {
            let (doc, _, page, result) = setup_at("<body></body>", &[
                source,
                "if(location!==window.location || location!==document.location || location.href!=='about:home' || location.toString()!==document.URL)throw Error('Location changed'); document.title=location.href;",
            ], Some(&location));
            assert_eq!(result.errors, 1, "{source}: {}", page.report());
            assert_eq!(doc.title, "about:home", "{source}: {}", page.report());
        }
    }

    #[test]
    fn location_results_obey_dom_size_and_copy_work_limits() {
        let location = crate::navigation::Location::home();
        let (mut doc, mut forms, mut page, result) =
            setup_at("<body></body>", &[], Some(&location));
        assert_eq!(result.errors, 0);
        page.state.copy_work = "about:home".len();
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        assert_eq!(
            bridge.get(LOCATION, "href").unwrap().to_text(),
            "about:home"
        );
        assert!(bridge
            .call(LOCATION, "toString", &[])
            .unwrap_err()
            .contains("string work limit"));
        page.set_location(&crate::navigation::Location {
            path: Some(std::path::PathBuf::from(format!(
                "/{}",
                "x".repeat(MAX_STRING)
            ))),
            ..location
        });
        page.begin_job();
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        assert!(bridge.get(DOCUMENT, "URL").unwrap_err().contains("32 KiB"));
        assert!(bridge
            .get(LOCATION, "pathname")
            .unwrap_err()
            .contains("32 KiB"));
    }

    #[test]
    fn native_document_methods_metadata_body_load_and_random_match_actual_capabilities() {
        let (doc, _, page, outcome) = setup(
            "<html><body><p id='out'>Hello</p></body></html>",
            &[r#"
if(typeof document.getElementById!=='function' || document.getElementById!==document.getElementById)throw Error('method identity');
var get=document.getElementById;
if(get.call(document,'out').textContent!=='Hello')throw Error('receiver');
var failed=false;try{get('out');}catch(e){failed=e.name==='TypeError';}
if(!failed)throw Error('detached receiver accepted');
if(document.prerendering!==false || document.compatMode!=='CSS1Compat')throw Error('metadata');
if(document.contentType!=='text/html')throw Error('content type');
if(!('prerendering' in document) || !('compatMode' in document) || !('contentType' in document))throw Error('presence');
var random=Math.random();if(random<0 || random>=1)throw Error('random');
document.body.onload=function(e){if(this!==window || e.target!==document)throw Error('load');document.title+='B';};
if(window.onload!==document.body.onload)throw Error('body alias');
addEventListener('load',function(){document.title+='W';},{once:true});
document.title='';
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "BW");
    }

    #[test]
    fn document_and_element_ancestry_follow_the_connected_tree() {
        let (mut doc, mut forms, mut page, outcome) = setup(
            "<html><head><title>Initial</title></head><body><div id=box><form id=owner><button type=button id=label>Follow</button></form></div></body></html>",
            &[r#"
var html=document.documentElement, body=document.body, head=document.head;
var box=document.getElementById('box'), owner=document.getElementById('owner'), label=document.getElementById('label');
if(head.tagName!=='HEAD' || head.parentElement!==html || body.parentNode!==html)throw Error('document elements');
if(html.parentElement!==null || html.parentNode!==document)throw Error('document root');
if(document.parentNode!==null || document.parentElement!==null || document.ownerDocument!==null)throw Error('root ancestry');
if(label.parentElement!==owner || label.parentNode!==owner || owner.parentElement!==box)throw Error('nested ancestry');
if(label.ownerDocument!==document || head.ownerDocument!==document)throw Error('ownership');
if(!('parentElement' in label) || !('parentNode' in document) || !('head' in document))throw Error('presence');
document.addEventListener('click',function(event){
    for(var node=event.target;node && node!==document.documentElement;node=node.parentElement){
        if(node.tagName==='FORM'){document.title=node.id;event.preventDefault();return;}
    }
});
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let label = id(&doc, "label");
        let outcome = click(&mut page, &mut doc, &mut forms, label);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(outcome.prevented);
        assert_eq!(doc.title, "owner");
    }

    #[test]
    fn document_images_is_live_ordered_and_preserves_native_members() {
        let (doc, _, page, outcome) = setup(
            "<html><body><img id=first name=logo><div><img id=second name=shared></div><img id=shared><img id=length><img id=item><img id=01><template><img id=inert></template></body></html>",
            &[r#"
var images=document.images;
var first=document.getElementById('first'), second=document.getElementById('second');
if(images!==document.images || images.length!==6)throw Error('identity or length');
if(images[0]!==first || images.item(0)!==first || images.item(1.9)!==second)throw Error('tree order');
if(images.item(4294967296)!==first || images.item(-1)!==null)throw Error('unsigned index');
if(images.logo!==first || images.namedItem('shared')!==second || images['01']!==images[5])throw Error('names');
if(images[6]!==undefined || images.item(6)!==null || images.namedItem('missing')!==null || images.namedItem('')!==null)throw Error('missing');
if(!('0' in images) || '6' in images || !('logo' in images) || 'inert' in images)throw Error('presence');
if(images.length!==6 || typeof images.item!=='function' || images.namedItem('length')!==images[3] || images.namedItem('item')!==images[4])throw Error('member collision');
first.id='renamed';
if(images.first!==undefined || images.renamed!==first || images.logo!==first || images.length!==6)throw Error('live names');
var item=images.item;
if(item!==images.item || item.call(images,0)!==first)throw Error('method identity');
var rejected=false;try{item(0);}catch(error){rejected=error.name==='TypeError';}
if(!rejected)throw Error('unbound method accepted');
document.title='Images inspected';
"#, "document.images.length=0;", "if(document.images.length!==6)throw Error('mutable collection');"],
        );
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert!(page.report().contains("image collection is read-only"));
        assert_eq!(doc.title, "Images inspected");
        assert_eq!(doc.attr(id(&doc, "renamed"), "name"), Some("logo"));
    }

    #[test]
    fn custom_event_probe_reads_accessor_options_and_removes_the_listener() {
        let (doc, _, page, outcome) = setup(
            "<html><body><p>Probe</p></body></html>",
            &[r#"
var reads='', detected=false, options={};
Object.defineProperty(options,'capture',{get:function(){reads+='c';return false;}});
Object.defineProperty(options,'once',{get:function(){reads+='o';return false;}});
Object.defineProperty(options,'passive',{get:function(){reads+='p';detected=true;}});
Object.defineProperty(options,'signal',{get:function(){reads+='s';}});
function callback(){throw Error('custom probe event was invented');}
window.addEventListener('test',callback,options);
window.removeEventListener('test',callback,options);
if(!detected || reads!=='copsc')throw Error('listener option conversion');
document.title='Custom probe complete';
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Custom probe complete");
        assert!(page.state.listeners.is_empty());
    }

    #[test]
    fn custom_event_names_preserve_case_identity_and_native_dispatch() {
        let (mut doc, mut forms, mut page, outcome) = setup(
            "<html><body><button id=b type=button>Go</button></body></html>",
            &[r#"
var b=document.getElementById('b');document.title='';
function custom(){throw Error('custom event was invented');}
b.addEventListener('Click',custom);
b.addEventListener('click',function(){document.title+='click';});
document.addEventListener('not a native event',custom);
window.addEventListener('',custom);
window.addEventListener('Test',custom,{capture:true,once:true,passive:true});
window.addEventListener('Test',custom,{capture:true,once:false,passive:false});
window.removeEventListener('test',custom,true);
window.removeEventListener('Test',custom,false);
window.addEventListener('test',custom);
window.removeEventListener('test',custom);
if(typeof b.dispatchEvent!=='function'||typeof window.dispatchEvent!=='function')throw Error('real dispatch capability');
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "");
        assert_eq!(page.state.listeners.len(), 5);
        let custom = page
            .state
            .listeners
            .iter()
            .find(|listener| listener.event == "Test")
            .unwrap();
        assert!(custom.options.capture && custom.options.once && custom.options.passive);
        assert!(page
            .state
            .listeners
            .iter()
            .any(|listener| listener.event.is_empty()));
        let button = id(&doc, "b");
        assert_eq!(click(&mut page, &mut doc, &mut forms, button).errors, 0);
        assert_eq!(doc.title, "click");
        assert_eq!(page.state.listeners.len(), 5);
    }

    #[test]
    fn custom_event_names_and_listener_storage_remain_bounded() {
        let boundary = "é".repeat(128);
        let ascii_overflow = "x".repeat(257);
        let unicode_overflow = "é".repeat(129);
        let scripts = [
            format!("function callback(){{}} window.addEventListener({boundary:?},callback);"),
            format!("window.addEventListener({ascii_overflow:?},callback);"),
            format!("window.addEventListener({unicode_overflow:?},callback);"),
            format!("window.removeEventListener({ascii_overflow:?},callback);"),
        ];
        let scripts: Vec<_> = scripts.iter().map(String::as_str).collect();
        let (_, _, page, outcome) = setup("<body><p>Bounds</p></body>", &scripts);
        assert_eq!(outcome.errors, 3, "{}", page.report());
        assert!(page
            .report()
            .contains("Event type exceeds the 256-byte limit"));
        assert_eq!(page.state.listeners.len(), 1);
        assert_eq!(page.state.listeners[0].event.len(), MAX_EVENT_TYPE_BYTES);
        assert_eq!(page.state.listeners[0].event.chars().count(), 128);

        let (_, _, page, outcome) = setup(
            "<body><p>Capacity</p></body>",
            &[
                "function callback(){} for(var i=0;i<64;i++){window.addEventListener('custom-'+i,callback);} window.addEventListener('overflow',callback);",
                "window.addEventListener('custom-0',callback,{once:true}); window.removeEventListener('custom-0',callback); window.addEventListener('replacement',callback);",
            ],
        );
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert!(page.report().contains("Event listener limit reached"));
        assert_eq!(page.state.listeners.len(), MAX_LISTENERS);
        assert!(page
            .state
            .listeners
            .iter()
            .any(|listener| listener.event == "replacement"));
        assert!(!page
            .state
            .listeners
            .iter()
            .any(|listener| matches!(listener.event.as_str(), "overflow" | "custom-0")));
    }

    #[test]
    fn custom_event_options_are_read_before_null_listener_noops_and_mutation() {
        let (doc, _, page, outcome) = setup(
            "<body><p>Options</p></body>",
            &[r#"
var reads='', options={};
Object.defineProperty(options,'capture',{get:function(){reads+='c';return false;}});
Object.defineProperty(options,'once',{get:function(){reads+='o';return false;}});
Object.defineProperty(options,'passive',{get:function(){reads+='p';return false;}});
Object.defineProperty(options,'signal',{get:function(){reads+='s';}});
window.addEventListener('custom-null',null,options);
window.addEventListener('custom-undefined',undefined,options);
window.removeEventListener('custom-null',null,options);
var rejected=0, bad={};
Object.defineProperty(bad,'capture',{get:function(){throw Error('options fail');}});
try{window.addEventListener('custom-null',null,bad);}catch(error){if(error.message==='options fail')rejected++;}
try{window.addEventListener('custom-live',function(){},bad);}catch(error){if(error.message==='options fail')rejected++;}
document.title=reads+'|'+rejected;
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "copscopsc|2");
        assert!(page.state.listeners.is_empty());
    }

    #[test]
    fn custom_event_methods_retain_native_receiver_checks() {
        let (doc, _, page, outcome) = setup(
            "<body><p>Receivers</p></body>",
            &[r#"
var add=document.addEventListener, remove=document.removeEventListener;
var reads=0, options={}, rejected=0;
Object.defineProperty(options,'capture',{get:function(){reads++;return false;}});
var receivers=[navigator,console,document.body.style,document.querySelectorAll('p'),{}];
for(var i=0;i<receivers.length;i++){
    try{add.call(receivers[i],'test',function(){},options);}catch(error){if(error.name==='TypeError')rejected++;}
    try{remove.call(receivers[i],'test',null,options);}catch(error){if(error.name==='TypeError')rejected++;}
}
document.title=rejected+'|'+reads;
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "10|0");
        assert!(page.state.listeners.is_empty());
    }

    #[test]
    fn capture_once_passive_and_listener_removal_follow_event_phases() {
        let (mut doc, mut forms, mut page, outcome)=setup("<html><body><form id='f'><button id='b' type='button'>Go</button></form></body></html>", &[r#"
var b=document.getElementById('b');var f=document.getElementById('f');
document.title='';
window.addEventListener('click',function(e){if(e.eventPhase!==1)throw Error('window capture');document.title+='W';},true);
document.addEventListener('click',function(e){document.title+='D';},{capture:true,once:true});
f.addEventListener('click',function(e){document.title+='F';},{capture:true});
function removed(){document.title+='X';}
b.addEventListener('click',removed,true);
b.removeEventListener('click',removed,{capture:true,signal:1});
b.addEventListener('click',function(e){if(e.eventPhase!==2)throw Error('target capture');document.title+='C';},{capture:true});
b.addEventListener('click',function(e){if(e.eventPhase!==2)throw Error('target bubble');e.preventDefault();if(e.defaultPrevented)throw Error('passive cancellation');document.title+='P';},{passive:true,once:true});
b.addEventListener('click',function(e){document.title+='B';});
f.addEventListener('click',function(e){if(e.eventPhase!==3)throw Error('ancestor bubble');document.title+='f';});
"#]);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let button = id(&doc, "b");
        let outcome = click(&mut page, &mut doc, &mut forms, button);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(!outcome.prevented);
        assert_eq!(doc.title, "WDFCPBf");
        let outcome = click(&mut page, &mut doc, &mut forms, button);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(!outcome.prevented);
        assert_eq!(doc.title, "WDFCPBfWFCBf");
    }

    #[test]
    fn capture_stop_propagation_preserves_same_phase_and_blocks_later_visits() {
        let (mut doc, mut forms, mut page, outcome) = setup(
            "<body><button id=b>Go</button></body>",
            &[r#"
var b=document.getElementById('b');document.title='';
b.addEventListener('click',function(e){e.stopPropagation();document.title+='C';},true);
b.addEventListener('click',function(e){document.title+='D';},true);
b.addEventListener('click',function(e){document.title+='B';e.preventDefault();});
document.addEventListener('click',function(){document.title+='bad';});
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let button = id(&doc, "b");
        let outcome = click(&mut page, &mut doc, &mut forms, button);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(!outcome.prevented);
        assert_eq!(doc.title, "CD");
    }

    #[test]
    fn inert_attribute_metadata_is_mutable_but_active_capabilities_remain_blocked() {
        let (doc, _, page, outcome) = setup(
            "<html><body><p id=p>Text</p></body></html>",
            &[
                r#"
var p=document.getElementById('p');
p.setAttribute('data-mode','night');p.setAttribute('aria-label','Night');p.setAttribute('role','status');
if(p.getAttribute('data-mode')!=='night' || !p.hasAttribute('aria-label'))throw Error('metadata');
p.removeAttribute('aria-label');if(p.hasAttribute('aria-label'))throw Error('removed');
"#,
                "document.getElementById('p').setAttribute('onclick','attack()');",
                "document.getElementById('p').setAttribute('src','https://example.org/pixel');",
            ],
        );
        assert_eq!(outcome.errors, 2, "{}", page.report());
        let p = id(&doc, "p");
        assert_eq!(doc.attr(p, "data-mode"), Some("night"));
        assert_eq!(doc.attr(p, "onclick"), None);
        assert_eq!(doc.attr(p, "src"), None);
    }

    #[test]
    fn script_font_properties_keep_the_declared_family_and_shorthand() {
        let (doc, _, page, outcome) = setup(
            "<body><p id=p>Text</p></body>",
            &[r#"
var p=document.getElementById('p');
p.style.fontFamily='"Page Face", serif';
if(p.style.fontFamily!=='"Page Face", serif')throw 'family';
p.style.setProperty('font','italic 700 20px/1.5 "Page Face", sans-serif');
if(p.style.getPropertyValue('font')!=='italic 700 20px/1.5 "Page Face", sans-serif')throw 'shorthand';
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let style = doc.attr(id(&doc, "p"), "style").unwrap();
        assert!(
            style.contains("font-family:") && style.contains("font:italic 700 20px/1.5"),
            "{style}"
        );
    }

    #[test]
    fn native_property_probes_report_only_supported_capabilities() {
        let (doc, _, page, result) = setup(
            "<html><body><p id='p'>Text</p><input id='box' type='checkbox'><input id='name'><button id='button'>Go</button></body></html>",
            &[r#"
var ordinary=undefined; let lexical=1; window.extra=7;
var p=document.getElementById('p');
var box=document.getElementById('box');
var name=document.getElementById('name');
var checks=[
 'ordinary' in window, !('lexical' in window), 'extra' in window,
 'setTimeout' in window, 'clearTimeout' in self, 'document' in globalThis,
 !('location' in window), !('fetch' in window), 'navigator' in window,
 'querySelector' in document, 'getElementById' in document, 'readyState' in document,
 'referrer' in document, document.referrer==='', 'cookie' in document, document.cookie==='',
 !('onload' in document), 'onload' in window, 'onclick' in p,
 'textContent' in p, !('value' in p), 'value' in name,
 'checked' in box, !('checked' in name), 'querySelector' in p,
 'color' in p.style, 'getPropertyValue' in p.style, !('madeUpProperty' in p.style),
 'log' in console, !('table' in console)
];
for(var i=0;i<checks.length;i+=1){if(!checks[i]){throw Error('probe '+i);}}
document.title='Supported capabilities only';
"#],
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Supported capabilities only");
    }

    #[test]
    fn navigator_reports_our_identity_and_actual_absent_capabilities_without_device_queries() {
        let script = format!(
            r#"
if(navigator !== window.navigator || navigator !== clientInformation || navigator !== self.clientInformation) throw Error('identity');
if(navigator.userAgent !== '{}' || navigator.platform !== '{}') throw Error('browser');
if(!navigator.cookieEnabled || navigator.pdfViewerEnabled || navigator.javaEnabled()) throw Error('capabilities');
if(navigator.webdriver !== false || navigator.language !== 'en') throw Error('state');
if(navigator.plugins.length !== 0 || navigator.mimeTypes.length !== 0) throw Error('plugins');
if(navigator.plugins.item(0) !== null || navigator.plugins.namedItem('PDF') !== null || navigator.mimeTypes.item(-1) !== null) throw Error('empty');
if(navigator.plugins[0] !== undefined || navigator.userAgentData !== undefined || navigator.hardwareConcurrency !== undefined) throw Error('fabricated');
if('hardwareConcurrency' in navigator || 'geolocation' in navigator || '0' in navigator.plugins) throw Error('probes');
if(typeof navigator.javaEnabled !== 'function' || typeof navigator.plugins.item !== 'function') throw Error('callable');
navigator.plugins.refresh();
document.title='Navigator matches browser';
"#,
            crate::http::USER_AGENT,
            navigator_platform()
        );
        let (doc, _, page, outcome) = setup("<body></body>", &[&script]);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Navigator matches browser");

        let (doc, _, page, outcome) = setup(
            "<body></body>",
            &[
                "navigator.cookieEnabled=false;",
                "document.title=String(navigator.cookieEnabled);",
            ],
        );
        // A readonly Web IDL attribute ignores an ordinary sloppy assignment;
        // it still reports the browser's actual capability. Strict assignment
        // is covered by the native-property regression tests.
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "true");
    }

    #[test]
    fn select_script_values_and_indices_preserve_defaults_and_do_not_emit_user_events() {
        let (mut doc, mut forms, mut page, outcome) = setup(
            "<body><form id=f><select id=s name=choice><option value=a>Alpha<option value=b selected>Beta<optgroup label=Hidden disabled><option value=c>Gamma</optgroup></select><p id=result>Initial</p></form></body>",
            &[r#"
var s=document.getElementById('s');var events=0;
s.oninput=function(){events+=1;};s.onchange=function(){events+=1;};
if(!('selectedIndex' in s) || s.selectedIndex!==1 || s.value!=='b') throw Error('initial');
s.value='c';if(s.selectedIndex!==2 || s.value!=='c') throw Error('disabled script choice');
s.value='missing';if(s.selectedIndex!==-1 || s.value!=='') throw Error('missing');
s.selectedIndex=0;if(s.value!=='a') throw Error('index');
if(events!==0) throw Error('unexpected input event');
document.title='Select setters';
"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Select setters");
        let select = id(&doc, "s");
        let form = id(&doc, "f");
        assert_eq!(doc.attr(select, "value"), None);
        assert_eq!(forms.controls[&select].caption, "Alpha");
        forms.reset(form);
        assert_eq!(forms.controls[&select].value.text(), "b");
        assert_eq!(
            forms.controls[&select].select.as_ref().unwrap().selected,
            Some(1)
        );
        page.begin_job();
        let mut bridge = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        };
        bridge
            .set(node_handle(select), "selectedIndex", Value::Number(99.0))
            .unwrap();
        assert_eq!(
            bridge.get(node_handle(select), "value").unwrap().to_text(),
            ""
        );
        assert!(bridge
            .set(node_handle(select), "selectedIndex", Value::Host(WINDOW))
            .is_err());
    }

    #[test]
    fn eval_csp_is_enforced_for_startup_timers_and_user_events() {
        let source = r#"
var code="document.getElementById('out').textContent='changed'; 7;";
function attempt(label){
 try {
  var compile=eval;
  var result=label==='timer' ? compile(code) : eval(code);
  if(result!==7) throw Error('eval completion');
  document.title+=label+':allowed;';
 } catch(error){ document.title+=label+':'+error.name+';'; }
}
document.title='';attempt('start');
setTimeout(function(){attempt('timer');},0);
document.getElementById('button').addEventListener('click',function(){attempt('click');});
"#;
        for (header, meta, allowed) in [
            ("", "", true),
            ("script-src 'unsafe-inline'", "", false),
            ("script-src 'unsafe-inline' 'unsafe-eval'", "", true),
            (
                "default-src 'none'; script-src-elem 'unsafe-inline' 'unsafe-eval'",
                "",
                false,
            ),
            ("script-src 'nonce-YWJj'", "", false),
            ("script-src 'nonce-YWJj' 'unsafe-eval'", "", true),
            (
                "script-src 'unsafe-inline' 'unsafe-eval', script-src 'unsafe-inline'",
                "",
                false,
            ),
            (
                "script-src 'unsafe-inline' 'unsafe-eval'",
                "script-src 'unsafe-inline'",
                false,
            ),
        ] {
            let mut policy = crate::csp::PolicySet::default();
            policy.add_header(header);
            if !meta.is_empty() {
                policy.add_meta(meta, 1);
            }
            // The loader filters inline sources before start_with_policy.
            // Exercise that same gate so a nonce-authorized source must still
            // pass the independent string-compilation decision at execution.
            assert!(policy.allows_inline(10, Some("YWJj"), source).is_ok());
            let mut doc =
                html::parse("<body><p id=out>untouched</p><button id=button>Run</button></body>");
            doc.scripting_enabled = true;
            let button = id(&doc, "button");
            let out = id(&doc, "out");
            let mut forms = Forms::new(&doc);
            let mut page = Page::for_test();
            let scripts = [Script {
                node: 10,
                label: "eval CSP fixture".into(),
                source: source.into(),
            }];
            assert_eq!(
                page.start_with_policy(&mut doc, &mut forms, &scripts, &policy)
                    .errors,
                0,
                "{}",
                page.report()
            );
            assert_eq!(
                page.poll_timer_at(
                    &mut doc,
                    &mut forms,
                    Instant::now() + Duration::from_secs(1)
                )
                .unwrap()
                .errors,
                0,
                "{}",
                page.report()
            );
            assert_eq!(
                click(&mut page, &mut doc, &mut forms, button).errors,
                0,
                "{}",
                page.report()
            );
            let expected = if allowed { "allowed" } else { "EvalError" };
            assert_eq!(
                doc.title,
                format!("start:{expected};timer:{expected};click:{expected};"),
                "header={header}; meta={meta}"
            );
            assert_eq!(
                text(&doc, out),
                if allowed { "changed" } else { "untouched" }
            );
        }
    }

    #[test]
    fn deferred_load_uses_the_existing_realm_and_fires_once_after_ready() {
        let mut doc = html::parse("<html><body><p id='result'>Initial</p></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let scripts = [Script { node: 0, label: "deferred fixture".into(), source: r#"
var order='script';
document.addEventListener('DOMContentLoaded', function(){order+='-dom';document.title=document.readyState;});
setTimeout(function(){order+='-timer';},0);
window.addEventListener('load',function(){order+='-load';document.getElementById('result').textContent=order;document.title=document.readyState;});
"#.into() }];
        assert_eq!(
            page.start_before_load(
                &mut doc,
                &mut forms,
                &scripts,
                &crate::csp::PolicySet::default()
            )
            .errors,
            0
        );
        assert_eq!(doc.title, "interactive");
        assert_eq!(text(&doc, id(&doc, "result")), "Initial");
        assert_eq!(page.poll_timer(&mut doc, &mut forms).unwrap().errors, 0);
        let finished = page.finish_loading(&mut doc, &mut forms);
        assert!(finished.layout_changed && finished.errors == 0);
        assert_eq!(doc.title, "complete");
        assert_eq!(text(&doc, id(&doc, "result")), "script-dom-timer-load");
        assert!(!page.finish_loading(&mut doc, &mut forms).layout_changed);
        assert_eq!(text(&doc, id(&doc, "result")), "script-dom-timer-load");
    }

    #[test]
    fn timers_are_deferred_ordered_cancellable_and_keep_argument_identity() {
        let (mut doc, mut forms, mut page, outcome) = setup(
            "<html><body><p id=out>Waiting</p></body></html>",
            &[r#"var order='startup'; var shared={count:1};
var cancelled=setTimeout(()=>{missing();},0); clearTimeout(cancelled);
var schedule=window.setTimeout;
if(typeof setTimeout!=='function' || schedule!==setTimeout || self.clearTimeout!==clearTimeout){missing();}
schedule(function(object,label){
 if(this!==window){missing();} object.count++;
 document.getElementById('out').textContent=label+':'+object.count+':'+order;
 setTimeout(()=>{document.title='nested';},0);
},0,shared,'first');
setTimeout(()=>{document.title='second:'+shared.count;},0);
document.addEventListener('DOMContentLoaded',()=>{order+='-dom';});
window.addEventListener('load',()=>{order+='-load';});"#],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(text(&doc, id(&doc, "out")), "Waiting");
        assert_eq!(page.state.timers.len(), 2);
        let due = Instant::now() + Duration::from_secs(1);
        assert_eq!(
            page.poll_timer_at(&mut doc, &mut forms, due)
                .unwrap()
                .errors,
            0
        );
        assert_eq!(text(&doc, id(&doc, "out")), "first:2:startup-dom-load");
        assert_eq!(page.state.timers.len(), 2);
        page.poll_timer_at(&mut doc, &mut forms, due).unwrap();
        assert_eq!(doc.title, "second:2");
        page.poll_timer_at(&mut doc, &mut forms, due).unwrap();
        assert_eq!(doc.title, "nested");
        assert!(page.next_timer_deadline().is_none());
        assert_eq!(page.state.timer_bytes, 0);
    }

    #[test]
    fn timer_delay_conversion_limits_and_callback_failure_are_bounded() {
        assert_eq!(timer_integer(&Value::Number(4_294_967_297.0)).unwrap(), 1);
        assert_eq!(timer_integer(&Value::String("-1.7".into())).unwrap(), -1);
        assert_eq!(timer_integer(&Value::Number(f64::INFINITY)).unwrap(), 0);
        let (mut doc, mut forms, mut page, outcome) = setup("<p id=out>Waiting</p>", &[
            "setTimeout(()=>{while(true){}},0); setTimeout(()=>{document.title='Recovered';},0); setTimeout(()=>{document.title='Too early';},60000);"
        ]);
        assert_eq!(outcome.errors, 0);
        let now = Instant::now();
        assert_eq!(
            page.poll_timer_at(&mut doc, &mut forms, now)
                .unwrap()
                .errors,
            1
        );
        assert_eq!(
            page.poll_timer_at(&mut doc, &mut forms, now)
                .unwrap()
                .errors,
            0
        );
        assert_eq!(doc.title, "Recovered");
        assert!(page.poll_timer_at(&mut doc, &mut forms, now).is_none());
        let (_, _, page, outcome) = setup(
            "<p>Timers</p>",
            &["try{setTimeout('document.title=1',0);}catch(e){document.title='Wrong';}"],
        );
        assert_eq!(outcome.errors, 1);
        assert!(page.state.timers.is_empty());
        let (_, _, page, outcome) = setup(
            "<p>Timers</p>",
            &["for(var i=0;i<65;i++){setTimeout(()=>{},60000);}"],
        );
        assert_eq!(outcome.errors, 1);
        assert_eq!(page.state.timers.len(), MAX_TIMERS);
        let source = format!("var large='{}';setTimeout(()=>{{}},60000,large,large,large);setTimeout(()=>{{}},60000,large,large);", "x".repeat(60 * 1024));
        let (_, _, page, outcome) = setup("<p>Timers</p>", &[&source]);
        assert_eq!(outcome.errors, 1);
        assert_eq!(page.state.timers.len(), 1);
        assert!(page.report().contains("Timer argument memory limit"));
    }

    #[test]
    fn nested_timers_are_clamped_and_cannot_run_forever() {
        let (mut doc, mut forms, mut page, outcome) = setup(
            "<p>Timers</p>",
            &["function again(){setTimeout(again,0);} setTimeout(again,0);"],
        );
        assert_eq!(outcome.errors, 0);
        let future = Instant::now() + Duration::from_secs(3600);
        for _ in 0..6 {
            page.poll_timer_at(&mut doc, &mut forms, future).unwrap();
        }
        let before = Instant::now();
        page.poll_timer_at(&mut doc, &mut forms, future).unwrap();
        assert!(page.state.timers[0].due >= before + Duration::from_millis(4));
        for _ in 7..MAX_TIMER_CALLBACKS {
            page.poll_timer_at(&mut doc, &mut forms, future).unwrap();
        }
        assert!(page.state.timers.is_empty());
        assert_eq!(page.state.timer_callbacks, MAX_TIMER_CALLBACKS);
        assert!(page.report().contains("Timer callback limit reached"));
    }

    #[test]
    fn startup_order_live_dom_forms_and_ready_events() {
        let (doc, forms, page, result) = setup(
            "<html><head><title>Initial</title></head><body><form id=f><input id=name value=original><input id=check type=checkbox><button id=button type=button><span id=caption>Old</span></button></form><p id=result>Waiting</p></body></html>",
            &[
                "var order = 'one'; document.getElementById('name').value = 'edited'; document.getElementById('check').checked = true; document.getElementById('caption').textContent = 'Changed';",
                "order += '-two'; document.addEventListener('DOMContentLoaded', function(event) { order += '-dom'; if (event.target !== document || this !== document || document.readyState !== 'interactive') { missing(); } }); window.addEventListener('load', function(event) { if(event.target !== document || event.currentTarget !== window || this !== window || document.readyState !== 'complete'){missing();} document.title = order + '-load'; document.getElementById('result').textContent = document.getElementById('name').value; document.getElementById('result').style.color = 'red'; document.getElementById('result').style.setProperty('font-weight', 'bold'); });",
            ],
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        assert!(result.layout_changed);
        assert_eq!(doc.title, "one-two-dom-load");
        assert_eq!(text(&doc, id(&doc, "result")), "edited");
        assert_eq!(forms.controls[&id(&doc, "name")].value.text(), "edited");
        assert!(forms.controls[&id(&doc, "check")].checked);
        assert_eq!(forms.controls[&id(&doc, "button")].caption, "Changed");
        assert_eq!(
            doc.attr(id(&doc, "result"), "style"),
            Some("color:red;font-weight:bold;")
        );
    }

    #[test]
    fn event_bubbling_context_return_false_and_non_cancelable_events() {
        let (mut doc, mut forms, mut page, result) = setup(
            "<body><form id=f onsubmit='return false'><button id=b type=button onclick=\"document.getElementById('out').textContent += 'p'; return false;\">Go</button></form><p id=out></p></body>",
            &["var b=document.getElementById('b'); var out=document.getElementById('out'); b.addEventListener('click', function(e){ if(this !== b || e.target !== b || e.currentTarget !== b) { missing(); } out.textContent += 'b'; return false; }); document.getElementById('f').addEventListener('click',function(e){ if(e.target !== b || e.currentTarget !== this) { missing(); } out.textContent += 'f'; }); document.addEventListener('click',function(e){out.textContent += 'd';}); window.addEventListener('click',function(e){out.textContent += 'w';}); b.oninput=function(e){e.preventDefault(); return false;};"],
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        let button = id(&doc, "b");
        let result = click(&mut page, &mut doc, &mut forms, button);
        assert_eq!(result.errors, 0, "{}", page.report());
        assert!(result.prevented);
        assert_eq!(text(&doc, id(&doc, "out")), "pbfdw");
        page.begin_job();
        assert!(
            !page
                .dispatch(&mut doc, &mut forms, button, "input", false)
                .prevented
        );
        let form = id(&doc, "f");
        assert!(
            page.dispatch(&mut doc, &mut forms, form, "submit", true)
                .prevented
        );

        let (mut doc, mut forms, mut page, result) = setup("<body><button id=b type=button>Go</button></body>", &["document.getElementById('b').addEventListener('click', function(e){ return false; });"]);
        assert_eq!(result.errors, 0);
        let button = id(&doc, "b");
        assert!(!click(&mut page, &mut doc, &mut forms, button).prevented);
    }

    #[test]
    fn listener_snapshot_deduplication_removal_and_expired_events() {
        let (mut doc, mut forms, mut page, result) = setup(
            "<body><button id=b type=button>Go</button><p id=out></p></body>",
            &["var b=document.getElementById('b'); var out=document.getElementById('out'); function removed(e){out.textContent += 'R';} function late(e){out.textContent += 'L';} function first(e){out.textContent += 'F'; b.removeEventListener('click',removed); b.addEventListener('click',late);} b.addEventListener('click',first); b.addEventListener('click',first); b.addEventListener('click',removed);"],
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        let button = id(&doc, "b");
        assert_eq!(click(&mut page, &mut doc, &mut forms, button).errors, 0);
        assert_eq!(text(&doc, id(&doc, "out")), "F");
        assert_eq!(click(&mut page, &mut doc, &mut forms, button).errors, 0);
        assert_eq!(text(&doc, id(&doc, "out")), "FFL");

        let (mut doc, mut forms, mut page, result) = setup("<body><button id=b type=button>Go</button></body>", &["var saved=null; document.getElementById('b').onclick=function(e){if(saved !== null){saved.preventDefault();} saved=e;};"]);
        assert_eq!(result.errors, 0);
        let button = id(&doc, "b");
        assert!(!click(&mut page, &mut doc, &mut forms, button).prevented);
        let result = click(&mut page, &mut doc, &mut forms, button);
        assert!(!result.prevented);
        assert_eq!(result.errors, 1);
        assert!(page.report().contains("Event handle has expired."));
    }

    #[test]
    fn text_replacement_is_atomic_reachable_and_reuses_stable_nodes() {
        let (doc, forms, page, result) = setup(
            "<body><div id=unsafe><span>Keep</span><input id=edit value=original></div><div id=safe><span id=removed>Old</span></div><p id=result></p><p id=loop>0</p></body>",
            &[
                "document.getElementById('unsafe').textContent='must not replace';",
                "document.getElementById('safe').textContent='new'; document.getElementById('result').textContent=String(document.getElementById('removed') === null); var i=0; while(i<1000){document.getElementById('loop').textContent=String(i); i++;}",
            ],
        );
        assert_eq!(result.errors, 1, "{}", page.report());
        assert_eq!(text(&doc, id(&doc, "unsafe")), "Keep");
        assert!(forms.controls.contains_key(&id(&doc, "edit")));
        assert_eq!(text(&doc, id(&doc, "safe")), "new");
        assert_eq!(text(&doc, id(&doc, "result")), "true");
        assert_eq!(text(&doc, id(&doc, "loop")), "999");
        let before = html::parse("<body><div id=unsafe><span>Keep</span><input id=edit value=original></div><div id=safe><span id=removed>Old</span></div><p id=result></p><p id=loop>0</p></body>").nodes.len();
        assert_eq!(doc.nodes.len(), before + 2);
    }

    #[test]
    fn inert_and_blocked_handlers_never_execute_and_inline_breakout_is_rejected() {
        let source = "<body><template><button id=inactive onclick=\"document.title='bad'\">Hidden</button></template><noscript><button id=fallback onclick=\"document.title='bad'\">Fallback</button></noscript><button id=b onclick=\"});document.title='escaped';(function(){\">Go</button><p id=out></p></body>";
        let (doc, _, page, result) = setup(source, &["document.getElementById('out').textContent=String(document.getElementById('inactive') === null && document.getElementById('fallback') === null);"]);
        assert_eq!(result.errors, 1, "{}", page.report());
        assert_eq!(doc.title, "");
        assert_eq!(text(&doc, id(&doc, "out")), "true");
        let mut doc = html::parse("<body onload=\"document.title='ran'\"><button id=b onclick=\"document.title='ran'\">Go</button></body>");
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let result = page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                source: "document.title='ran'".into(),
                label: "blocked".into(),
            }],
        );
        assert_eq!(result.errors, 0);
        let button = id(&doc, "b");
        assert_eq!(click(&mut page, &mut doc, &mut forms, button).errors, 0);
        assert_eq!(doc.title, "");
    }

    #[test]
    fn form_mutations_keep_defaults_groups_and_do_not_dispatch_input() {
        let (mut doc, mut forms, mut page, result) = setup(
            "<body><form id=f><fieldset disabled><input id=blocked value=x></fieldset><input id=text value=original><input id=r1 type=radio name=pick checked><input id=r2 type=radio name=pick><input id=submit type=submit value=Before><button id=button type=button value=before>Caption</button></form><p id=out>quiet</p></body>",
            &["var t=document.getElementById('text'); t.oninput=function(){document.getElementById('out').textContent='unexpected';}; t.value='edited'; document.getElementById('r2').checked=true; document.getElementById('blocked').disabled=false; if(document.getElementById('blocked').disabled !== false){missing();} document.getElementById('submit').value='After'; document.getElementById('button').value='after';"],
        );
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(text(&doc, id(&doc, "out")), "quiet");
        assert!(forms.controls[&id(&doc, "blocked")].disabled);
        assert!(!forms.controls[&id(&doc, "r1")].checked);
        assert!(forms.controls[&id(&doc, "r2")].checked);
        assert_eq!(forms.controls[&id(&doc, "submit")].caption, "After");
        assert_eq!(forms.controls[&id(&doc, "button")].caption, "Caption");
        forms.reset(id(&doc, "f"));
        assert_eq!(forms.controls[&id(&doc, "text")].value.text(), "original");
        assert!(forms.controls[&id(&doc, "r1")].checked);
        assert!(!forms.controls[&id(&doc, "r2")].checked);
        let input = id(&doc, "text");
        page.begin_job();
        assert_eq!(
            page.dispatch(&mut doc, &mut forms, input, "input", false)
                .errors,
            0
        );
        assert_eq!(text(&doc, id(&doc, "out")), "unexpected");
    }

    #[test]
    fn attribute_changes_cannot_reassociate_controls_or_add_active_capabilities() {
        let (doc, forms, page, result) = setup(
            "<body><p id=first>First</p><form id=f></form><input id=input form=f value=keep><p id=out></p></body>",
            &[
                "document.getElementById('f').id='other';",
                "document.getElementById('first').setAttribute('id','f');",
                "document.getElementById('input').setAttribute('value','changed');",
                "document.getElementById('first').setAttribute('onclick','alert(1)');",
                "document.getElementById('first').style.background='url(https://example.test/)';",
                "document.getElementById('first').setAttribute('class','ready'); document.getElementById('out').textContent=document.getElementById('input').value;",
            ],
        );
        assert_eq!(result.errors, 5, "{}", page.report());
        assert_eq!(
            forms.controls[&id(&doc, "input")].owner,
            Some(id(&doc, "f"))
        );
        assert_eq!(doc.attr(id(&doc, "input"), "value"), Some("keep"));
        assert_eq!(doc.attr(id(&doc, "first"), "class"), Some("ready"));
        assert_eq!(text(&doc, id(&doc, "out")), "keep");
    }

    #[test]
    fn host_limits_listener_limits_and_shared_jobs_are_visible_diagnostics() {
        let html = format!(
            "<body><button id=b type=button>Go</button>{}</body>",
            "<p>filler</p>".repeat(3000)
        );
        let (doc, _, page, result) = setup(
            &html,
            &["var b=document.getElementById('b'); while(true){b.disabled = false;}"],
        );
        assert!(result.errors >= 1);
        assert!(page.report().contains("DOM work limit reached"));
        assert!(!doc.nodes.is_empty());

        let (mut doc, mut forms, mut page, result) = setup("<body><button id=b type=button>Go</button></body>", &["var b=document.getElementById('b'); var i=0; while(i<65){b.addEventListener('click',function(e){console.log('ok');});i++;}"]);
        assert_eq!(result.errors, 1);
        assert_eq!(page.state.listeners.len(), MAX_LISTENERS);
        let button = id(&doc, "b");
        page.begin_job();
        page.state.host_work = 1;
        let result = page.dispatch(&mut doc, &mut forms, button, "click", true);
        assert!(result.errors >= 1);
        // dispatch cannot reset the exhausted shared action allowance.
        assert_eq!(page.state.host_work, 0);
        assert!(
            page.dispatch(&mut doc, &mut forms, button, "click", true)
                .errors
                >= 1
        );
        for _ in 0..200 {
            page.state.log(&"x".repeat(3000));
        }
        assert!(page.report().len() <= MAX_LOG_BYTES + 1024);
        assert!(page.state.omitted > 0);
    }
}


#[cfg(test)]
mod browser_budget_tests {
    use super::*;

    #[test]
    fn browser_host_strings_cross_old_limit_and_explicit_profiles_survive_start() {
        for small in [false, true] {
            let mut page = if small { Page::for_test() } else { Page::new() };
            let selected = page.state.limits;
            let mut doc = crate::html::parse("<title>pending</title><body></body>");
            doc.scripting_enabled = true;
            let mut forms = Forms::new(&doc);
            let payload = "x".repeat(70 * 1024);
            let script = Script { node: 0,
                source: format!("var payload='{payload}';document.title=payload;var node=document.createElement('div');node.setAttribute('data-large',payload);document.body.appendChild(node);"),
                label: "host conversion beyond old32KiB".into() };
            let result = page.start(&mut doc, &mut forms, &[script]);
            assert_eq!(page.state.limits, selected);
            if small {
                assert!(result.errors > 0);
                assert_eq!(doc.title, "pending");
            } else {
                assert_eq!(result.errors, 0, "{}", page.report());
                assert_eq!(doc.title, payload);
                assert!(doc.nodes.iter().enumerate().any(|(id, _)| doc.attr(id, "data-large") == Some(payload.as_str())));
            }
        }
    }

    #[test]
    fn production_page_uses_enlarged_limits_for_startup_and_later_jobs() {
        let mut page = Page::new();
        assert_eq!(page.state.limits, crate::javascript::limits::BROWSER);
        let mut doc = crate::html::parse("<title>pending</title>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let script = Script {
            node: 0,
            source: format!(
                "let payload={:?};Promise.resolve().then(function(){{let n=0;for(let i=0;i<1024;i++)n+=payload.length;document.title=String(n);}});",
                "x".repeat(16 * 1024),
            ),
            label: "original enlarged-budget fixture".into(),
        };
        let result = page.start(&mut doc, &mut forms, &[script]);
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "16777216");
        page.begin_job();
        assert_eq!(page.state.host_work, 64_000_000);
        assert_eq!(page.state.copy_work, 16 * 1024 * 1024 * 1024);
        let report = page.report();
        assert!(report.contains("1000000000 steps / 30 seconds / 16384 MiB"));
        assert!(report.contains("4000000000 steps / 120 seconds / 65536 MiB"));
        assert!(report.contains("1048576 properties per object / 16777216 per document"));
        assert!(report.contains("1048576 object identities; array length 16777216; 512 MiB"));
    }
}

#[cfg(test)]
#[path = "script_timer_global_tests.rs"]
mod timer_global_tests;
