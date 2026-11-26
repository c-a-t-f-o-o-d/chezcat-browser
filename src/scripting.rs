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
use crate::dom_string::DomString;

#[cfg(test)]
#[path="script_native_dom_string_tests.rs"]
mod native_dom_string_tests;
use crate::javascript::adjacent_interface;
use crate::javascript::anchor_url_interface;
use crate::javascript::html_reflection_interface::{meta_attribute, meta_operation, charset_operation};
use crate::javascript::{Host, Runtime, Value,
    CREATE_CDATA_SECTION, CREATE_PROCESSING_INSTRUCTION, DOCUMENT_ACTIVE_ELEMENT_GET, DOCUMENT_CURRENT_SCRIPT_GET, DOCUMENT_IMPORT_NODE, DOCUMENT_ADOPT_NODE,
    TITLE_TEXT_GET, TITLE_TEXT_SET, HTML_SLOT_NAME_GET, HTML_SLOT_NAME_SET, HTML_TEMPLATE_CONTENT_GET, PROCESSING_INSTRUCTION_TARGET_GET,
    ELEMENT_INNER_HTML_GET, ELEMENT_INNER_HTML_SET, ELEMENT_STYLE_GET, ELEMENT_STYLE_SET, SVG_STYLE_GET,SVG_STYLE_SET,
    NODE_TEXT_CONTENT_GET, NODE_TEXT_CONTENT_SET, NODE_VALUE_GET, NODE_VALUE_SET, NODE_HAS_CHILDREN, NODE_TRAVERSAL_GETTERS};

#[path = "script_promises.rs"]
mod script_promises;
use crate::loader::Script;

#[path = "script_dom_compat.rs"]
mod dom_compat;
use dom_compat::{dataset_attribute, navigator_readonly};
#[path = "script_queries.rs"]
mod queries;
use queries::node_list_index;
#[cfg(test)]
#[path = "script_parent_children_tests.rs"]
mod parent_children_tests;
#[path = "script_element_attribute_list.rs"]
mod script_element_attribute_list;
#[path = "script_parent_children.rs"]
mod parent_children;
#[path = "script_collections.rs"]
mod dom_collections;
use dom_collections::collection_handle_index;
#[path = "script_tree_order.rs"]
mod tree_order;
#[path = "script_node_sets.rs"]
mod node_sets;
#[path = "script_node_state.rs"]
mod node_state;
#[path = "script_dom_tree.rs"]
mod dom_tree;
#[path = "script_position.rs"]
mod script_position;
#[path = "script_node_root.rs"]
mod script_node_root;
#[path = "script_clone_node.rs"]
mod script_clone_node;
#[cfg(test)]
#[path = "script_clone_node_tests.rs"]
mod script_clone_node_tests;
#[path = "script_node_contains.rs"]
mod script_node_contains;
#[cfg(test)]
#[path = "script_node_contains_tests.rs"]
mod node_contains_tests;
#[path = "script_document_fragments.rs"]
mod script_document_fragments;
#[path = "script_timezone.rs"]
mod script_timezone;
#[path = "script_computed_style.rs"]
mod script_computed_style;
#[path = "script_inline_style.rs"]
mod script_inline_style;
#[cfg(test)]
#[path = "script_inline_easing_tests.rs"]
mod script_inline_easing_tests;
use dom_tree::DomState;
#[path = "script_attr.rs"]
mod script_attr;
#[path = "script_hidden_inputs.rs"]
mod script_hidden_inputs;
#[path = "script_text_inputs.rs"]
mod script_text_inputs;
#[path="script_iframe_sandbox.rs"]
mod script_iframe_sandbox;
#[path = "script_textarea_interface.rs"]
mod script_textarea_interface;
#[path = "script_input_accessors.rs"]
mod script_input_accessors;
#[path = "script_content_editable.rs"]
mod script_content_editable;
#[path = "script_canvas.rs"]
mod script_canvas;
#[path = "script_canvas_context.rs"]
mod script_canvas_context;
use script_canvas_context::Operation as CanvasOperation;
#[cfg(test)]
#[path = "script_input_accessor_tests.rs"]
mod script_input_accessor_tests;
#[cfg(test)]
#[path = "script_input_checkedness_tests.rs"]
mod script_input_checkedness_tests;
#[path = "script_buttons.rs"]
mod script_buttons;
#[path = "script_attribute_names.rs"]
mod script_attribute_names;
#[path = "script_class_list.rs"]
mod script_class_list;
#[path = "script_named_map.rs"]
mod script_named_map;
#[path = "script_normalize.rs"]
mod script_normalize;
#[cfg(test)]
#[path = "script_node_sets_tests.rs"]
mod script_node_sets_tests;
#[cfg(test)]
#[path = "script_tree_order_tests.rs"]
mod script_tree_order_tests;
#[cfg(test)]
#[path = "script_sibling_order_storage_tests.rs"]
mod script_sibling_order_storage_tests;
#[cfg(test)]
#[path = "script_node_state_tests.rs"]
mod script_node_state_tests;
#[cfg(test)]
#[path = "script_id_index_tests.rs"]
mod script_id_index_tests;
#[cfg(test)]
#[path = "script_node_handle_tests.rs"]
mod script_node_handle_tests;
#[cfg(test)]
#[path = "script_character_storage_tests.rs"]
mod script_character_storage_tests;
#[cfg(test)]
#[path = "script_main_child_storage_tests.rs"]
mod script_main_child_storage_tests;
#[path = "script_element_properties.rs"]
mod script_element_properties;
#[path = "script_created_events.rs"]
mod script_created_events;
#[path = "script_anchor_mouse.rs"]
mod script_anchor_mouse;
#[path = "script_focus_events.rs"]
mod script_focus_events;
#[path = "script_anchor_url.rs"]
mod script_anchor_url;
pub use script_anchor_mouse::AnchorMouseInput;
#[path = "script_dispatch.rs"]
mod script_dispatch;
#[path = "script_ranges.rs"]
mod script_ranges;
#[path = "script_abort.rs"]
mod script_abort;
#[path = "script_message_channel.rs"]
mod script_message_channel;
#[path = "script_media_query_list.rs"]
mod script_media_query_list;
use crate::javascript::{MEDIA_QUERY_LIST, MEDIA_QUERY_MATCHES, MEDIA_QUERY_MEDIA};
#[cfg(test)]
#[path = "script_listener_object_tests.rs"]
mod script_listener_object_tests;
#[cfg(test)]
#[path = "script_listener_object_realm_tests.rs"]
mod script_listener_object_realm_tests;
#[path = "script_navigation.rs"]
mod page_navigation;
pub use page_navigation::Navigation;
#[path = "script_forms.rs"]
mod script_forms;
#[path="script_form_reset.rs"]
mod script_form_reset;
#[path="script_form_validation.rs"]
mod script_form_validation;
#[path="script_form_request.rs"]
mod script_form_request;
#[path="script_control_eligibility.rs"]
mod script_control_eligibility;
#[path = "script_resources.rs"]
mod script_resources;
#[path="script_xhr.rs"]
mod script_xhr;
pub use script_resources::DynamicScript;
#[cfg(test)]
pub use script_resources::ScriptRequest;
use script_resources::DynamicState;
#[path = "script_blank_frames.rs"]
mod script_blank_frames;
#[path = "script_inert_documents.rs"]
mod script_inert_documents;
use script_inert_documents::character_substring;
use script_inert_documents::text_whole;
use crate::javascript::text_split_interface;
use crate::javascript::node_normalize_interface;
use crate::javascript::character_data_interface;

#[path = "script_mutation_child_list.rs"]
mod script_mutation_child_list;
#[path = "script_stylesheets.rs"]
mod script_stylesheets;
#[path = "script_charset.rs"]
mod script_charset;
pub use script_stylesheets::DynamicStylesheet;
use script_stylesheets::StylesheetState;
#[path = "script_images.rs"]
mod script_images;
#[path = "script_media.rs"]
mod script_media;
#[path = "script_media_play.rs"]
mod script_media_play;
pub(crate) use script_media::event_name as media_event_name;
pub use script_images::DynamicImage;
pub(crate) use script_images::MAX_IMAGE_REQUESTS;
use script_images::ImagesState;
#[path = "script_select_fragments.rs"]
mod script_select_fragments;
#[cfg(test)]
#[path = "script_textarea_fragment_tests.rs"]
mod script_textarea_fragment_tests;
#[cfg(test)]
#[path = "script_textarea_clone_tests.rs"]
mod script_textarea_clone_tests;
#[cfg(test)]
#[path = "script_created_radio_tests.rs"]
mod script_created_radio_tests;
#[path = "script_element_siblings.rs"]
mod script_element_siblings;
#[path = "script_slottable.rs"]
mod script_slottable;
#[path = "script_offset_parent.rs"]
mod script_offset_parent;
use script_offset_parent::Offset;
#[path = "script_scroll_offsets.rs"]
mod script_scroll_offsets;
#[path = "script_transform.rs"]
mod script_transform;
#[cfg(test)]
#[path = "script_translate_tests.rs"]
mod translate_tests;
use script_slottable::Slottable;

#[path = "script_element_selectors.rs"]
mod script_element_selectors;
#[path = "script_document_properties.rs"]
mod script_document_properties;

#[path = "script_inner_html.rs"]
mod script_inner_html;
#[path = "script_meta_element.rs"]
mod script_meta_element;
#[path="script_title_element.rs"]
mod script_title_element;
#[path = "script_history.rs"]
mod script_history;
#[cfg(test)]
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
// Disjoint from nodes, events, class lists, Attrs, standalone targets and maps.
// The range is representational; policy and actual use bound allocation.
const NODE_LIST_BASE: u32 = 0x8300_0000;
const NODE_LIST_HANDLES: usize = 0x0100_0000;
#[cfg(test)]
const MAX_NODE_LISTS: usize = 128;
#[cfg(test)]
const MAX_NODE_LIST_REFERENCES: usize = 16_384;
// A typed byte and 24-bit stable arena index. These are representation domains,
// not allocation/count policies; all producers still admit actual storage.
const NODE_HANDLES: u32 = 0x0100_0000;
const NODE_BASE: u32 = 0x0100_0000;
const STYLE_BASE: u32 = 0x0200_0000;
const DATASET_BASE: u32 = 0x0300_0000;
const EVENT_BASE: u32 = 0x0400_0000;
const _: () = assert!(
    crate::html::MAX_LOADED_DOCUMENT_NODES + crate::javascript::limits::BROWSER.host.extra_nodes
        <= (DATASET_BASE - STYLE_BASE) as usize
);
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
    pub caps_lock: bool,
    pub num_lock: bool,
    pub scroll_lock: bool,
    pub alt_graph: bool,
}

#[derive(Default, Debug)]
pub struct Outcome {
    pub prevented: bool,
    pub layout_changed: bool,
    pub errors: usize,
}

#[path = "script_identity_map.rs"]
mod identity_map;
#[path = "script_retained_storage.rs"]
mod retained_storage;

#[path = "script_listeners.rs"]
mod listener_store;
#[cfg(test)]
#[path = "script_listener_storage_tests.rs"]
mod listener_storage_tests;
#[cfg(test)]
#[path = "script_event_target_storage_tests.rs"]
mod event_target_storage_tests;

#[derive(Clone)]
struct Listener {
    serial: usize,
    target: u32,
    event: String,
    callback: Value,
    property: bool,
    options: ListenerOptions,
}

#[derive(Clone, Copy, Default)]
struct ListenerOptions {
    capture: bool,
    once: bool,
    passive: bool,
}

#[path = "script_timer_queue.rs"]
mod timer_queue;

struct Timer {
    id: u32,
    order: usize,
    due: Instant,
    callback: Value,
    arguments: Vec<Value>,
    bytes: usize,
    nesting: u32,
}

#[derive(Clone,Copy)]
enum CallbackQueue { Timer, AnimationFrame }

struct AnimationFrame {
    id: u32,
    in_batch: bool,
    callback: Value,
}

struct Event {
    composed: bool,
    path: Vec<u32>,
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
    mouse: Option<AnchorMouseInput>,
    focus: Option<Option<u32>>,
    submitter: Option<Option<u32>>,
    timestamp: f64,
}

#[derive(Default)]
struct Tree {
    active: Vec<bool>,
    parents: Vec<Option<usize>>,
    sibling_slots: Vec<usize>,
    order: Vec<usize>,
    ids: Vec<(usize, usize)>,
    body: Option<usize>,
    head: Option<usize>,
    title: Option<usize>,
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
    background_pending: bool,
    background_tracking: bool,
    blank_frames: Vec<script_blank_frames::BlankFrame>,
    inert_documents: script_inert_documents::Arena,
    text_split_plan:Option<script_inert_documents::text_split::Plan>,
    normalize_plan:Option<script_inert_documents::shared_normalize::Plan>,
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
    listeners: listener_store::Store,
    native_functions: HashMap<&'static str, Value>,
    native_methods: Vec<(u32, &'static str, Value)>,
    detached_images: node_sets::NodeSet,
    detached_nodes: node_sets::NodeSet,
    output_defaults: identity_map::IdentityMap<u32,DomString>,
    output_custom_errors: identity_map::IdentityMap<u32,DomString>,
    node_lists: Vec<queries::NodeList>,
    computed_styles: Vec<script_computed_style::Declaration>,
    canvas_contexts: script_canvas_context::Contexts,
    ranges: Vec<script_ranges::Record>,
    node_list_references: usize,
    attributes: Vec<script_attr::Attr>,
    attached_attributes: Vec<usize>,
    declined_images: usize,
    declined_beacons: usize,
    eval_allowed: bool,
    timers: timer_queue::Queue,
    animation_frames: VecDeque<AnimationFrame>,
    next_animation_frame: u32,
    animation_ids_reused: bool,
    active_animation_frame: Option<u32>,
    animation_due: Option<Instant>,
    animation_callbacks: u64,
    clock_origin: Instant,
    navigation_timing:Option<crate::network_trace::DocumentTiming>,
    embedded_video:Option<crate::embedded_video::Snapshot>,
    media_changes:script_media::Changes,
    media_play:script_media_play::State,
    media_play_notify:bool,
    media_play_fault:Option<String>,
    pending_keyboard: Option<KeyboardInput>,
    next_timer: u32,
    timer_ids_reused: bool,
    active_timer: Option<u32>,
    timer_bytes: usize,
    timer_callbacks: usize,
    timer_nesting: u32,
    next_event: u32,
    event: Option<Event>,
    created_events: Vec<script_created_events::CreatedEvent>,
    standalone_targets: Vec<u32>,
    message_channels: script_message_channel::State,
    xhr:script_xhr::State,
    media_queries: Vec<String>,
    abort_signals: Vec<script_abort::Record>,
    synthetic_dispatches: Vec<script_dispatch::Dispatch>,
    form_reset_pending: Option<script_form_reset::PendingReset>,
    form_validations:Vec<script_form_validation::Pending>,
    form_requests:Vec<script_form_request::Pending>,
    native_submission:Option<usize>,
    validation_report:Option<Option<usize>>,
    #[cfg(test)]
    disable_event_factory: bool,
    tree: Tree,
    tree_stack: Vec<tree_order::Visit>,
    dirty: bool,
    // Committed DOM writes, including detached nodes; not native form values,
    // allocations, ordinary JS properties, layout work or resource completion.
    dom_commits: Option<u64>,
    limits: crate::javascript::limits::Limits,
    host_work: usize,
    execution_deadline: Option<Instant>,
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
    current_script: Option<usize>,
    // readyState becomes complete BEFORE the load callbacks/checkpoints.
    document_complete: bool,
    last_activation: Option<Instant>,
    navigation_incumbent: Option<u32>,
}

impl State {
    fn new() -> Self {
        Self {
            blank_frames: Vec::new(),
            inert_documents: script_inert_documents::Arena::default(),
            text_split_plan:None,
            normalize_plan:None,
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
            listeners: listener_store::Store::default(),
            native_functions: HashMap::new(),
            native_methods: Vec::new(),
            detached_images: node_sets::NodeSet::default(),
            detached_nodes: node_sets::NodeSet::default(),
            output_defaults: identity_map::IdentityMap::default(),
            output_custom_errors: identity_map::IdentityMap::default(),
            node_lists: Vec::new(),
            computed_styles: Vec::new(),
            canvas_contexts: script_canvas_context::Contexts::default(),
            ranges: Vec::new(),
            node_list_references: 0,
            attributes: Vec::new(),
            attached_attributes: Vec::new(),
            declined_images: 0,
            declined_beacons: 0,
            eval_allowed: false,
            timers: timer_queue::Queue::default(),
            animation_frames: VecDeque::new(),
            next_animation_frame: 0,
            animation_ids_reused: false,
            active_animation_frame: None,
            animation_due: None,
            animation_callbacks: 0,
            clock_origin: Instant::now(),
            navigation_timing:None,
            embedded_video:None,
            media_changes:script_media::Changes::default(),
            media_play:script_media_play::State::default(),
            media_play_notify:false,
            media_play_fault:None,
            pending_keyboard: None,
            next_timer: 0,
            timer_ids_reused: false,
            active_timer: None,
            timer_bytes: 0,
            timer_callbacks: 0,
            timer_nesting: 0,
            next_event: EVENT_BASE,
            event: None,
            created_events: Vec::new(),
            standalone_targets: Vec::new(),
            message_channels: script_message_channel::State::default(),
            xhr:script_xhr::State::default(),
            media_queries: Vec::new(),
            abort_signals: Vec::new(),
            synthetic_dispatches: Vec::new(),
            form_reset_pending: None,
            form_validations:Vec::new(),
            form_requests:Vec::new(),
            native_submission:None,
            validation_report:None,
            #[cfg(test)]
            disable_event_factory: false,
            tree: Tree::default(),
            tree_stack: Vec::new(),
            dirty: true,
            dom_commits: Some(0),
            limits: crate::javascript::limits::BROWSER,
            host_work: crate::javascript::limits::BROWSER.host_work,
            execution_deadline: None,
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
            current_script: None,
            background_pending: false,
            background_tracking: false,
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
        let remaining = self.host_work;
        let Some(left) = remaining.checked_sub(count) else {
            self.host_work = 0;
            return Err(format!("DOM work limit reached ({} node/host operations per job).", self.limits.host_work));
        };
        self.host_work = left;
        if remaining / 4096 != left / 4096
            && self.execution_deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err("DOM computation time limit reached.".into());
        }
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

    fn prepare_callback_id(&mut self,queue:CallbackQueue)->Result<(u32,bool),String> {
        let (next,reused,maximum,count,active)=match queue {
            CallbackQueue::Timer=>(self.next_timer,self.timer_ids_reused,i32::MAX as u32,self.timers.len(),self.active_timer),
            CallbackQueue::AnimationFrame=>(self.next_animation_frame,self.animation_ids_reused,u32::MAX,self.animation_frames.len(),self.active_animation_frame),
        };
        if !reused {
            if let Some(id)=next.checked_add(1).filter(|id|*id<=maximum) {
                return Ok((id,false));
            }
        }
        // After the native positive-ID range wraps, the pigeonhole bound gives
        // a free ID among live-count + 1 candidates. This scratch bitmap avoids
        // probing the entire ID domain or repeatedly scanning the timer heap.
        let candidates=count.checked_add(usize::from(active.is_some()))
            .and_then(|count|count.checked_add(1)).filter(|count|*count<=maximum as usize)
            .ok_or("Native callback positive identifier space exhausted.")?;
        let words=candidates.div_ceil(64);
        let available=self.retained_limit.saturating_sub(self.retained);
        if words*8>available{return Err("DOM retained callback identity scratch allowance exhausted.".into());}
        self.work(count.saturating_add(words.saturating_mul(2)).saturating_add(1))?;
        let mut used=Vec::<u64>::new();
        used.try_reserve_exact(words).map_err(|_|"Could not reserve callback identity scratch.")?;
        let bytes=used.capacity()*8;
        if bytes>available{return Err("DOM retained callback identity scratch allocation exceeds allowance.".into());}
        self.retained+=bytes;
        used.resize(words,0);
        let mut mark=|id:u32| {
            if id>0&&(id as usize)<=candidates {let bit=id as usize-1;used[bit/64]|=1u64<<(bit%64);}
        };
        match queue {
            CallbackQueue::Timer=>self.timers.iter().for_each(|timer|mark(timer.id)),
            CallbackQueue::AnimationFrame=>self.animation_frames.iter().for_each(|frame|mark(frame.id)),
        }
        if let Some(id)=active {mark(id);}
        let id=used.iter().enumerate().find_map(|(word,bits)| {
            let bit=word*64+(!bits).trailing_zeros() as usize;
            (bits!=&u64::MAX&&bit<candidates).then_some((bit+1) as u32)
        });
        drop(used);
        self.retained-=bytes;
        id.map(|id|(id,true)).ok_or_else(||"Native callback positive identifier space exhausted.".into())
    }

    fn set_timeout(&mut self, arguments: &[Value]) -> Result<Value, String> {
        let Some(callback @ Value::Function(_)) = arguments.first() else {
            return Err(
                "setTimeout requires a function; string/eval timers are not supported.".into(),
            );
        };
        #[cfg(test)]
        if self.limits.test_entry_arenas && self.timers.len() >= self.limits.host.timers {
            return Err("Timer limit reached (64 pending callbacks per document).".into());
        }
        #[cfg(test)]
        if self.limits.test_entry_arenas && self.timer_callbacks >= self.limits.host.timer_callbacks {
            return Err(
                "Timer callback limit reached (4096 per document). Reload starts a new allowance."
                    .into(),
            );
        }
        let extra = arguments.get(2..).unwrap_or(&[]);
        #[cfg(test)]
        if self.limits.test_entry_arenas && extra.len() > self.limits.host.timer_arguments {
            return Err("Timer argument limit reached (30).".into());
        }
        let slots = extra.len().checked_mul(std::mem::size_of::<Value>())
            .ok_or("Timer argument size overflow.")?;
        let bytes = extra.iter().try_fold(slots, |total, value| total.checked_add(value.bytes()))
            .ok_or("Timer argument size overflow.")?;
        let retained = self.timer_bytes.checked_add(bytes).ok_or("Timer argument size overflow.")?;
        #[cfg(test)]
        if self.limits.test_entry_arenas && retained > self.limits.host.timer_bytes {
            return Err("Timer argument memory limit reached (256 KiB per document).".into());
        }
        if bytes > self.retained_limit.saturating_sub(self.retained) {
            return Err("DOM retained timer payload allowance exhausted.".into());
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
        let (id,reused_id) = self.prepare_callback_id(CallbackQueue::Timer)?;
        let mut copied = Vec::new();
        copied.try_reserve_exact(extra.len()).map_err(|_| "Could not reserve timer argument slots.")?;
        for value in extra {
            let value = match value {
                Value::String(text) => {
                    let mut copy = String::new();
                    copy.try_reserve_exact(text.len()).map_err(|_| "Could not reserve timer argument text.")?;
                    copy.push_str(text);
                    Value::String(copy)
                }
                Value::StringUnits(units) => {
                    let mut copy = Vec::new();
                    copy.try_reserve_exact(units.len()).map_err(|_| "Could not reserve timer argument code units.")?;
                    copy.extend_from_slice(units);
                    Value::StringUnits(copy)
                }
                // All other values retain fixed-size primitive or genuine
                // object/function/native identities, not another payload.
                _ => value.clone(),
            };
            copied.push(value);
        }
        let actual=copied.iter().try_fold(copied.capacity()*std::mem::size_of::<Value>(),|total,value| {
            total.checked_add(retained_storage::value_bytes(value))
        }).ok_or("Timer payload storage overflow.")?;
        let retained=retained.checked_add(actual-bytes).ok_or("Timer payload accounting overflow.")?;
        #[cfg(test)]
        if self.limits.test_entry_arenas && retained>self.limits.host.timer_bytes {
            return Err("Timer argument memory limit reached (256 KiB per document).".into());
        }
        let available=self.retained_limit.saturating_sub(self.retained).checked_sub(actual)
            .ok_or("DOM retained timer payload allowance exhausted.")?;
        let (growth,added)=self.timers.prepare_growth(available)?;
        self.work(self.timers.maintenance_work().saturating_add(self.timers.order_work()))?;
        if self.timers.order_work()>0 {self.copies(self.timers.len()*std::mem::size_of::<usize>())?;}
        if growth.is_some(){self.copies(self.timers.len()*std::mem::size_of::<Timer>())?;}
        self.copies(actual.saturating_sub(bytes))?;
        let order=self.timers.prepare_order(available-added)?;
        self.timers.commit_order(order);
        self.timers.commit_growth(growth);
        self.next_timer = id;
        self.timer_ids_reused = reused_id;
        self.timer_bytes = retained;
        self.retained += added+actual;
        self.timers.push_reserved(Timer {
            id,
            order: 0,
            due,
            callback: callback.clone(),
            arguments: copied,
            bytes: actual,
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
        self.work(self.timers.len().saturating_add(self.timers.maintenance_work()))?;
        if let Some(index) = self
            .timers
            .iter()
            .position(|timer| id > 0 && timer.id == id as u32)
        {
            let timer = self.timers.remove(index);
            self.timer_bytes -= timer.bytes;
            self.retained -= timer.bytes;
        }
        Ok(Value::Undefined)
    }

    fn clear_timers(&mut self) {
        self.retained -= self.timer_bytes+self.timers.storage_bytes();
        self.timer_bytes=0;
        self.timers=timer_queue::Queue::default();
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
        #[cfg(test)]
        if self.limits.test_entry_arenas && self.animation_frames.len() >= self.limits.host.animation_frames {
            return Err(format!("Animation frame limit reached ({} pending callbacks per document).",self.limits.host.animation_frames));
        }
        let (id,reused)=self.prepare_callback_id(CallbackQueue::AnimationFrame)?;
        self.work(1)?;
        let before=self.animation_frames.capacity();
        let mut added=0;
        let mut replacement=None;
        if self.animation_frames.len()==before {
            let slot=std::mem::size_of::<AnimationFrame>();
            let available=self.retained_limit.saturating_sub(self.retained);
            let growth=before.max(4).min(available/slot);
            if growth==0{return Err("DOM retained animation queue allowance exhausted.".into());}
            let capacity=before.checked_add(growth).ok_or("Animation queue capacity overflow.")?;
            let mut next=VecDeque::new();
            next.try_reserve_exact(capacity).map_err(|_|"Could not reserve animation queue storage.")?;
            added=next.capacity().checked_sub(before).and_then(|slots|slots.checked_mul(slot))
                .filter(|bytes|*bytes<=available).ok_or("DOM retained animation queue allocation exceeds allowance.")?;
            self.copies(self.animation_frames.len()*slot)?;
            replacement=Some(next);
        }
        if let Some(mut next)=replacement {next.append(&mut self.animation_frames);self.animation_frames=next;}
        self.retained+=added;
        self.animation_frames.push_back(AnimationFrame {
            id,
            in_batch: false,
            callback: callback.clone(),
        });
        self.next_animation_frame = id;
        self.animation_ids_reused = reused;
        self.animation_due
            .get_or_insert_with(|| Instant::now() + FRAME_INTERVAL);
        Ok(Value::Number(f64::from(id)))
    }

    fn clear_animation_frames(&mut self) {
        self.retained-=self.animation_frames.capacity()*std::mem::size_of::<AnimationFrame>();
        self.animation_frames=VecDeque::new();
        self.animation_due=None;
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
        self.replace_storage(old,new,new)
    }
    fn replace_storage(&mut self,old:usize,new:usize,work:usize)->Result<(),String>{
        self.copies(work)?;
        let retained=self.retained.saturating_sub(old).checked_add(new).ok_or("DOM retained storage accounting overflow.")?;
        if retained>self.retained_limit{return Err(format!("DOM retained text/attribute growth limit reached ({} bytes total).",self.retained_limit));}
        self.retained=retained;Ok(())
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

    fn reserve_dispatch_temporary<T>(&mut self,values:&mut Vec<T>,capacity:usize,other:usize)->Result<(),String> {
        let slot=std::mem::size_of::<T>();
        let old=values.capacity().checked_mul(slot).ok_or("Dispatch metadata size overflow.")?;
        let available=self.retained_limit.saturating_sub(self.retained).checked_sub(other)
            .and_then(|available|available.checked_sub(old)).ok_or("DOM retained dispatch metadata allowance exhausted.")?;
        let (next,_)=retained_storage::capacity(values,capacity,available)?;
        if let Some(mut next)=next {
            self.copies(values.len().saturating_mul(slot))?;
            next.append(values);*values=next;
        }
        Ok(())
    }

    fn push_dispatch_temporary<T>(&mut self,values:&mut Vec<T>,value:T,other:usize)->Result<(),String> {
        let slot=std::mem::size_of::<T>();
        if values.len()==values.capacity() {
            let available=self.retained_limit.saturating_sub(self.retained).saturating_sub(other)
                .saturating_sub(values.capacity().saturating_mul(slot));
            let growth=values.capacity().max(4).min(available/slot);
            if growth==0{return Err("DOM retained dispatch metadata allowance exhausted.".into());}
            let capacity=values.capacity().checked_add(growth).ok_or("Dispatch metadata capacity overflow.")?;
            self.reserve_dispatch_temporary(values,capacity,other)?;
        }
        self.copies(slot)?;
        values.push(value);
        Ok(())
    }

    fn listener_snapshot(&mut self,target:u32,event:&str,capture:bool)->Result<Vec<usize>,String> {
        self.work(self.listeners.len().saturating_mul(2))?;
        let count=self.listeners.iter().filter(|listener|listener.target==target&&listener.event==event&&listener.options.capture==capture).count();
        let mut snapshot=Vec::new();
        self.reserve_dispatch_temporary(&mut snapshot,count,0)?;
        self.copies(count.saturating_mul(std::mem::size_of::<usize>()))?;
        snapshot.extend(self.listeners.iter().filter(|listener|listener.target==target&&listener.event==event&&listener.options.capture==capture).map(|listener|listener.serial));
        self.retained+=snapshot.capacity()*std::mem::size_of::<usize>();
        Ok(snapshot)
    }

    fn event_type_allowance(&self)->usize {
        #[cfg(test)]
        if self.limits.test_entry_arenas{return self.limits.host.string_bytes.min(self.limits.host.event_type_bytes);}
        self.limits.host.string_bytes
    }

    fn event_type_refused(&self,bytes:usize)->bool {bytes>self.event_type_allowance()}

    fn created_event_limit_reached(&self)->bool {
        #[cfg(test)]
        if self.limits.test_entry_arenas&&self.created_events.len()>=self.limits.host.created_events{return true;}
        false
    }

    fn event_lookup_work(&self,lookups:usize)->usize {
        ((usize::BITS-self.created_events.len().leading_zeros()) as usize+1).saturating_mul(lookups)
    }

    fn prepare_created_event_storage(&mut self,payload:usize)->Result<(Option<Vec<script_created_events::CreatedEvent>>,usize),String> {
        let available=self.retained_limit.saturating_sub(self.retained).checked_sub(payload)
            .ok_or("Event retained payload allowance exceeded.")?;
        let before=self.created_events.capacity();
        if self.created_events.len()<before{return Ok((None,0));}
        let slot=std::mem::size_of::<script_created_events::CreatedEvent>();
        let growth=before.max(4).min(available/slot);
        if growth==0{return Err("DOM retained Event record allowance exhausted.".into());}
        let capacity=before.checked_add(growth).ok_or("Event record capacity overflow.")?;
        let reservation=retained_storage::capacity(&self.created_events,capacity,available)?;
        self.copies(self.created_events.len().saturating_mul(slot))?;
        Ok(reservation)
    }

    fn listener_limit_reached(&self)->bool {
        #[cfg(test)]
        if self.limits.test_entry_arenas && self.listeners.len()>=self.limits.host.listeners {return true;}
        false
    }

    fn add_listener(
        &mut self,
        target: u32,
        event: &str,
        callback: Value,
        property: bool,
        options: ListenerOptions,
    ) -> Result<(), String> {
        self.work(self.listeners.len())?;
        if property {
            if let Some(old) = self
                .listeners
                .iter_mut()
                .find(|l| l.target == target && l.event == event && l.property)
            {
                old.callback = callback;
                return Ok(());
            }
        } else if self.listeners.iter().any(|l| {
            l.target == target
                && l.event == event
                && !l.property
                && l.options.capture == options.capture
                && same_event_listener(&l.callback, &callback)
        }) {
            return Ok(());
        }
        if self.listener_limit_reached() {
            return Err("Event listener limit reached (64 per document).".into());
        }
        let available=self.retained_limit.saturating_sub(self.retained);
        if event.len()>available{return Err("DOM retained listener name allowance exhausted.".into());}
        let mut name=String::new();
        name.try_reserve_exact(event.len()).map_err(|_|"Could not reserve event listener name.")?;
        name.push_str(event);
        let available=available.checked_sub(name.capacity()).ok_or("DOM retained listener name allocation exceeds allowance.")?;
        let growth=self.listeners.prepare(available,self.event.is_none()&&self.synthetic_dispatches.is_empty())?;
        self.copies(name.capacity().saturating_add(growth.copies))?;
        let added=growth.added+name.capacity();
        growth.commit(&mut self.listeners,Listener {
            serial:0,target,event:name,callback,property,options,
        });
        self.retained+=added;
        Ok(())
    }
    fn retain_listeners(&mut self,keep:impl FnMut(&Listener)->bool)->Result<(),String> {
        self.work(self.listeners.len())?;
        self.retained-=self.listeners.retain(keep);
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

    #[cfg(test)]
    pub(crate) fn native_node_limit_for_test(&mut self,limit:usize)->usize{std::mem::replace(&mut self.state.node_limit,limit)}

    #[cfg(test)]
    pub(crate) fn loading_state_for_test(&self) -> (&'static str, bool) {
        (self.state.ready, self.state.dynamic.load_ready)
    }

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
            .bind_top_level_browser_global(WINDOW)
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
            "getComputedStyle",
            "matchMedia",
            "addEventListener",
            "removeEventListener",
            "dispatchEvent",
        ] {
            if matches!(name, "setTimeout" | "clearTimeout" | "requestAnimationFrame" | "cancelAnimationFrame" | "getComputedStyle" | "matchMedia") {
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
            (HISTORY, "pushState"),
            (PERFORMANCE, "now"),
            (DOCUMENT, "getElementsByTagName"),
            (DOCUMENT, "querySelector"),
            (DOCUMENT, "querySelectorAll"),
            (DOCUMENT, "createElement"),
            (DOCUMENT, "createElementNS"),
            (DOCUMENT, "createTextNode"),
            (DOCUMENT, "createComment"),
            (DOCUMENT, "createDocumentFragment"),
            (DOCUMENT, "addEventListener"),
            (DOCUMENT, "removeEventListener"),
            (DOCUMENT, "dispatchEvent"),
        ] {
            let function = if object == HISTORY || object==DOCUMENT&&method=="createElementNS" { runtime.host_method_with_length(object, method, 2) }
                else if object==DOCUMENT&&method=="createComment" {runtime.host_method_with_length(object,method,1)}
                else if object==DOCUMENT&&method=="createDocumentFragment" {runtime.host_method_with_length(object,method,0)}
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
        runtime.define_media_query_interface(WINDOW).expect("fixed MediaQueryList interface");
        runtime.define_abort_interfaces(WINDOW).expect("fixed abort interfaces");
        runtime.define_dom_parser_interface(WINDOW, script_inert_documents::parser::PARSE).expect("fixed DOMParser interface");
        runtime.define_event_interface(WINDOW).expect("fixed Event interface");
        runtime.define_message_channel_interfaces(WINDOW).expect("fixed channel interfaces");
        runtime.define_custom_event_interface(WINDOW).expect("fixed CustomEvent interface");
        runtime.define_ui_event_interface(WINDOW).expect("fixed UIEvent interface");
        runtime.define_mouse_event_interface(WINDOW).expect("fixed MouseEvent interface");
        runtime.define_wheel_event_interface(WINDOW).expect("fixed WheelEvent interface");
        runtime.define_keyboard_event_interface(WINDOW).expect("fixed KeyboardEvent interface");
        runtime.define_focus_event_interface(WINDOW).expect("fixed FocusEvent interface");
        runtime.define_submit_event_interface(WINDOW).expect("fixed SubmitEvent interface");
        runtime.define_progress_event_interface(WINDOW).expect("fixed ProgressEvent interface");
        runtime.define_animation_event_interface(WINDOW).expect("fixed AnimationEvent interface");
        runtime.define_error_event_interface(WINDOW).expect("fixed ErrorEvent interface");
        runtime.define_xhr_interfaces(WINDOW).expect("fixed XMLHttpRequest interfaces");
        runtime.define_node_interfaces(WINDOW,dom_tree::NODE_CONSTANTS,Some((IMAGE_FACTORY,"Image"))).expect("fixed Node interfaces");
        runtime.define_range_interfaces(WINDOW).expect("fixed range interfaces");
        runtime.define_mutation_observers().expect("fixed mutation observer interfaces");
        runtime.define_tree_walker(WINDOW).expect("fixed tree traversal interfaces");
        runtime.define_node_iterator(WINDOW).expect("fixed node iterator interface");
        runtime.define_inert_document_interfaces(WINDOW)
            .expect("fixed inert document interfaces");
        runtime.define_media_interfaces(WINDOW).expect("fixed media interfaces");
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
    pub fn set_embedded_video(&mut self,snapshot:Option<crate::embedded_video::Snapshot>){self.observe_media_snapshot(snapshot);}

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

    pub fn take_viewport_scroll_request(&mut self) -> Option<i32> {
        self.state.geometry.as_mut()?.take_scroll_request()
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
        let deadline = self.runtime.execution_deadline();
        self.state.execution_deadline = Some(deadline);
        self.state.host_work = self.state.limits.host_work;
        self.state.copy_work = self.state.limits.copy_bytes;
        self.state.timer_nesting = 0;
        if let Some(geometry) = &mut self.state.geometry {
            geometry.begin_job();
            geometry.set_execution_deadline(deadline);
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
            .deadline()
            .into_iter()
            .chain(self.state.animation_due)
            .chain(self.state.message_channels.next_deadline())
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
        let timer_due = self.state.timers.deadline();
        if self.state.message_channels.next_deadline().is_some_and(|due|
            due <= now && timer_due.is_none_or(|timer| due <= timer)
                && self.state.animation_due.is_none_or(|frame| due <= frame))
        {
            return self.poll_message_task(doc, forms, now);
        }
        if self
            .state
            .animation_due
            .is_some_and(|due| due <= now && timer_due.is_none_or(|timer| due <= timer))
        {
            return Some(self.poll_animation_frames(doc, forms, now));
        }
        if !timer_due.is_some_and(|due|due<=now){return None;}
        #[cfg(test)]
        if self.state.limits.test_entry_arenas && self.state.timer_callbacks >= self.state.limits.host.timer_callbacks {
            self.state.clear_timers();
            self.state.error(
                "setTimeout",
                "Timer callback limit reached (4096 per document). Reload starts a new allowance.",
            );
            return Some(Outcome {
                errors: 1,
                ..Outcome::default()
            });
        }
        self.begin_job();
        if let Err(error)=self.state.work(self.state.timers.maintenance_work()) {
            self.state.clear_timers();
            self.runtime.cancel_microtasks();
            self.cancel_media_selection();
            self.state.error("setTimeout",&error);
            return Some(Outcome {errors:1,..Outcome::default()});
        }
        let timer = self.state.timers.remove(0);
        self.state.active_timer = Some(timer.id);
        self.state.timer_nesting = timer.nesting;
        self.state.timer_callbacks = self.state.timer_callbacks.saturating_add(1);
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
        drop(timer.arguments);
        self.state.active_timer = None;
        self.state.retained -= timer.bytes;
        self.state.timer_bytes -= timer.bytes;
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
        self.clear_message_tasks();
        self.state.clear_timers();
        self.state.clear_animation_frames();
        self.state.animation_due = None;
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
        // Mark the current FIFO prefix after admitting its scan. New callbacks
        // append unmarked entries, independently of recycled native IDs.
        let batch_work = self.state.animation_frames.len().checked_add(1);
        self.state.animation_due = None;
        if let Err(error) = batch_work.ok_or_else(|| "Animation frame work overflow.".to_owned()).and_then(|work| self.state.work(work)) {
            self.state.clear_animation_frames();
            self.runtime.cancel_microtasks();
            self.cancel_media_selection();
            self.state.error("requestAnimationFrame", &error);
            return Outcome { errors: self.state.errors.saturating_sub(before), ..Outcome::default() };
        }
        for frame in &mut self.state.animation_frames {frame.in_batch=true;}
        while self.state.animation_frames.front().is_some_and(|frame|frame.in_batch) {
            let frame = self.state.animation_frames.pop_front().unwrap();
            self.state.active_animation_frame=Some(frame.id);
            let id = frame.id;
            self.state.animation_callbacks = self.state.animation_callbacks.saturating_add(1);
            let result = self.runtime.invoke(
                frame.callback,
                &[Value::Number(timestamp)],
                Value::Undefined,
                &mut Bridge {
                    doc,
                    forms,
                    state: &mut self.state,
                },
            );
            self.state.active_animation_frame=None;
            if let Err(error) = result {
                self.state
                    .error(&format!("requestAnimationFrame callback {id}"), &error);
                if self.runtime.invocation_failed_fatally() {
                    self.state.clear_animation_frames();
                    self.cancel_media_selection();
                    break;
                }
            }
            if !self.microtask_checkpoint(doc, forms) {
                self.state.clear_animation_frames();
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
        let stylesheet_sources = std::mem::take(&mut self.state.stylesheets.sources);
        let stylesheet_source_capacity = self.state.stylesheets.source_capacity;
        let stylesheet_withheld_capacity=self.state.stylesheets.withheld_capacity;
        let stylesheet_sources_prepared = self.state.stylesheets.sources_prepared;
        let stylesheet_source_error = self.state.stylesheets.source_setup_error.take();
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
        self.state.stylesheets.sources = stylesheet_sources;
        self.state.stylesheets.source_capacity = stylesheet_source_capacity;
        self.state.stylesheets.withheld_capacity=stylesheet_withheld_capacity;
        self.state.stylesheets.sources_prepared = stylesheet_sources_prepared;
        self.state.stylesheets.source_setup_error = stylesheet_source_error;
        self.state.images = images;
        self.state.history = history;
        self.state.dynamic.policy = policy.clone();
        self.state.dynamic.script_count = sources.len();
        self.state.dynamic.source_bytes = sources.iter().map(|script| script.source.len()).sum();
        self.begin_job();
        self.state.enabled = doc.scripting_enabled;
        self.state.eval_allowed = policy.allows_eval().is_ok();
        if !doc.scripting_enabled {
            self.state.stylesheets.sources_prepared = false;
            return Outcome::default();
        }
        if !self.state.stylesheets.sources_prepared {
            let geometry = self.state.geometry.take();
            self.set_stylesheet_sources(doc, geometry.as_ref().map(|geometry| geometry.stylesheet_sources()).unwrap_or(&HashMap::new()));
            self.state.geometry = geometry;
        }
        self.state.stylesheets.sources_prepared = false;
        if let Some(error) = self.state.stylesheets.source_setup_error.take() {
            self.state.error("Stylesheet source ownership", &error);
            return Outcome { errors: 1, ..Outcome::default() };
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
        self.state.retained += self.state.stylesheets.source_metadata_bytes();
        if let Err(error) = self.state.rebuild(doc) {
            self.state.error("JavaScript startup", &error);
            return Outcome {
                errors: 1,
                ..Outcome::default()
            };
        }
        // Compile each attribute as an ordinary function in this small global
        // environment. Legacy implicit form/element name scope is unsupported.
        // Handler compilation invokes no author code and cannot mutate tree order.
        'handlers: for position in 0..self.state.tree.order.len() {
            let node=self.state.tree.order[position];
            if let Err(error) = self.state.work(1) {
                self.state.error("inline handler scan", &error);
                break;
            }
            if doc.nodes[node].namespace != crate::html::Namespace::Html {
                continue;
            }
            for name in [
                "onclick",
                "onauxclick",
                "oninput",
                "onchange",
                "onsubmit",
                "onreset",
                "oninvalid",
                "onload",
                "onkeydown",
                "onkeyup",
                "onfocus", "onblur",
                "onerror","onloadstart","onabort","onemptied","ondurationchange","onloadedmetadata","onloadeddata",
                "oncanplay","oncanplaythrough","onplay","onplaying","onwaiting","onpause","onseeking","onseeked","ontimeupdate","onended",
            ] {
                if !matches!(name,"onclick"|"onauxclick"|"oninput"|"onchange"|"onsubmit"|"onreset"|"oninvalid"|"onload"|"onkeydown"|"onkeyup"|"onfocus"|"onblur")
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
                if self.state.listener_limit_reached() {
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
            let deadline = self.runtime.execution_deadline();
            self.state.execution_deadline = Some(deadline);
            if let Some(geometry) = &mut self.state.geometry {
                geometry.set_execution_deadline(deadline);
            }
            // Reactions run before the next classic script and consume this
            // script's allowance, including the aggregate startup budget.
            self.run_classic_script(doc, forms, source, None);
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

    pub(crate) fn set_background_pending(&mut self, pending: bool) {
        self.state.background_tracking = true;
        self.state.background_pending = pending;
    }

    // Real App resource callbacks can change the cascade before window load.
    // Reserve its rescan; standalone Page callers retain their existing contract.
    pub(super) fn defer_backgrounds_for_layout(&mut self, changed: bool) {
        if self.state.background_tracking && changed { self.state.background_pending = true; }
    }

    pub(crate) fn finish_background_loading(&mut self, doc: &mut Document, forms: &mut Forms) -> Outcome {
        if !self.state.background_pending && self.state.dynamic.load_ready {
            self.finish_loading(doc, forms)
        } else { Outcome::default() }
    }

    /// The later resource-completion task fires load at most once. Stopping or
    /// replacing a load does not call this method for the abandoned document.
    pub fn finish_loading(&mut self, doc: &mut Document, forms: &mut Forms) -> Outcome {
        if !self.state.enabled || !doc.scripting_enabled || self.state.ready != "interactive" {
            return Outcome::default();
        }
        self.state.dynamic.load_ready = true;
        if self.state.background_pending || self.state.dynamic.in_flight != 0 || self.state.stylesheets.in_flight != 0
            || self.state.embedded_video.as_ref().is_some_and(|video|video.load_pending)
            || self.state.images.entries.values().any(|entry| entry.pending) {
            return Outcome::default();
        }
        self.begin_job();
        let before = self.state.errors;
        self.state.ready = "complete";
        self.dispatch_to(doc, forms, DOCUMENT, "readystatechange", false, false);
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
        let prevented = self.dispatch_composed_to(doc, forms, target, event, true, true, true);
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
                    || tag(doc, target) == "script" && self.state.dom.created.contains(&target)
                    || tag(doc, target) == "link" && doc.nodes[target].namespace == crate::html::Namespace::Html
            }
            _ => false,
        };
        if !supported {
            return Outcome::default();
        }
        let prevented = self.dispatch_composed_to(
            doc,
            forms,
            node_handle(target),
            event,
            cancelable,
            !matches!(event, "load" | "error"),
            matches!(event,"click"|"input"),
        );
        Outcome {
            prevented,
            layout_changed: std::mem::take(&mut self.state.changed),
            errors: self.state.errors.saturating_sub(before),
        }
    }

    /// Snapshot invalid candidates before callbacks. The backing remains charged
    /// across every dispatch, including listeners that remove later candidates.
    pub(crate) fn validate_native_submission(&mut self,doc:&mut Document,forms:&mut Forms,owner:usize,submitter:Option<usize>)->Result<(Outcome,bool,Option<usize>),String>{
        let before=self.state.errors;
        let enabled=self.state.enabled&&doc.scripting_enabled;
        if enabled {
            if self.runtime.invocation_failed_fatally(){return Err("The script job failed; nothing was submitted.".into());}
            let bridge=Bridge{doc,forms,state:&mut self.state};
            bridge.native_node_access(node_handle(owner)).map_err(|(_,e)|e)?;
        }
        let work=doc.nodes.len().checked_mul(2).and_then(|n|forms.controls.len().checked_mul(forms.controls.len()).and_then(|m|n.checked_add(m))).ok_or("Validation work overflow.")?;
        let work=work.checked_add(forms.validation_metadata_work(doc)?).ok_or("Validation metadata work overflow.")?;
        let work=work.checked_add(script_control_custom_validity::snapshot_work(&self.state,forms.controls.len())?).ok_or("Custom validation work overflow.")?;
        if enabled{self.state.work(work)?;script_text_content::deadline(&self.state)?;}
        let available=if enabled{self.state.retained_limit.saturating_sub(self.state.retained)}else{usize::MAX};
        let mut pattern_work=if enabled{self.state.host_work}else{crate::javascript::limits::BROWSER.regexp_work};
        let mut pattern_copies=if enabled{self.state.copy_work}else{usize::MAX};
        let snapshot=forms.validation_snapshot_with_budget(doc,owner,submitter,available,&mut |node|self.state.output_custom_errors.get(&node_handle(node)).is_some(),&mut pattern_work,&mut pattern_copies);
        if enabled{self.state.host_work=pattern_work;self.state.copy_work=pattern_copies;script_text_content::deadline(&self.state)?;}
        let snapshot=snapshot?;
        let storage=snapshot.storage_bytes();
        if enabled{self.state.copies(snapshot.peak_bytes)?;script_text_content::deadline(&self.state)?;self.state.replace_storage(0,storage,0)?;}
        let invalid=!snapshot.nodes.is_empty();let mut unhandled=None;
        let result:Result<(),String>=(||{
            for &node in &snapshot.nodes{
                let (canceled,complete)=if enabled{
                    Bridge{doc,forms,state:&mut self.state}.native_node_access(node_handle(node)).map_err(|(_,e)|e)?;
                    self.dispatch_composed_status(doc,forms,node_handle(node),"invalid",true,false,false,None,None,None)
                }else{(false,true)};
                if !complete{return Err("Required-field invalid event delivery did not complete; nothing was submitted.".into());}
                if !canceled&&unhandled.is_none(){unhandled=Some(node);}
            }
            Ok(())
        })();
        if enabled{self.state.retained-=storage;}
        result?;
        Ok((Outcome{prevented:false,layout_changed:std::mem::take(&mut self.state.changed),errors:self.state.errors.saturating_sub(before)},invalid,unhandled))
    }

    pub(crate) fn dispatch_submit(&mut self,doc:&mut Document,forms:&mut Forms,target:usize,submitter:Option<usize>)->Outcome{
        if !self.state.enabled||!doc.scripting_enabled{return Outcome::default();}
        let before=self.state.errors;
        let checked=(||{
            let bridge=Bridge{doc,forms,state:&mut self.state};
            if !bridge.form_length_receiver(node_handle(target)){return Err("Submit requires a genuine native form.".into());}
            bridge.native_node_access(node_handle(target)).map_err(|(_,e)|e)?;
            bridge.submit_event_argument(WINDOW,submitter.map(node_handle).map(Value::Host).as_ref()).map_err(|(_,e)|e)?;
            Ok::<(),String>(())
        })();
        if let Err(error)=checked{self.state.error("submit",&error);return Outcome{prevented:true,errors:self.state.errors-before,..Outcome::default()};}
        let(prevented,complete)=self.dispatch_composed_status(doc,forms,node_handle(target),"submit",true,true,false,None,None,submitter.map(node_handle));
        Outcome{prevented:prevented||!complete,layout_changed:std::mem::take(&mut self.state.changed),errors:self.state.errors.saturating_sub(before)}
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
        self.dispatch_composed_to(doc,forms,target,event,cancelable,bubbles,false)
    }

    fn dispatch_composed_to(
        &mut self,
        doc: &mut Document,
        forms: &mut Forms,
        target: u32,
        event: &str,
        cancelable: bool,
        bubbles: bool,
        composed: bool,
    ) -> bool {
        self.dispatch_composed_status(doc, forms, target, event, cancelable, bubbles, composed, None, None, None).0
    }

    // Existing callers retain their cancellation result; native anchor input
    // additionally requires a completed path and callback/checkpoint traversal.
    fn dispatch_composed_status(
        &mut self, doc: &mut Document, forms: &mut Forms, target: u32,
        event: &str, cancelable: bool, bubbles: bool, composed: bool,
        mouse: Option<AnchorMouseInput>,
        focus: Option<Option<u32>>,
        submitter: Option<u32>,
    ) -> (bool, bool) {
        // Resource completion can reach a native event tail in the same job
        // as a fatal script. It must not revive that job or queue later work.
        if self.runtime.invocation_failed_fatally() {
            self.runtime.cancel_microtasks();
            self.cancel_media_selection();
            return (false, false);
        }
        let result = if event=="invalid"{self.event_path_with_detached(doc,target,true)}else{self.event_path(doc, target)};
        let path = match result {
            Ok(path) => path,
            Err(error) => {
                self.state.error(event, &error);
                return (false, false);
            }
        };
        let Some(next) = self.state.next_event.checked_add(1).filter(|next|*next <= script_class_list::BASE) else {
            self.state.error(event, "Event identity limit reached.");
            return (false, false);
        };
        let prepared=(||->Result<(String,Vec<(u32,bool,u8)>,usize),String> {
            let path_bytes=path.capacity()*std::mem::size_of::<u32>();
            let keyboard_bytes=self.state.pending_keyboard.as_ref().map_or(0,|input|input.key.capacity()+input.code.capacity());
            let available=self.state.retained_limit.saturating_sub(self.state.retained)
                .checked_sub(path_bytes).and_then(|bytes|bytes.checked_sub(keyboard_bytes))
                .filter(|bytes|*bytes>=event.len()).ok_or("DOM retained native event payload allowance exhausted.")?;
            let mut name=String::new();
            name.try_reserve_exact(event.len()).map_err(|_|"Could not reserve native event name.")?;
            if name.capacity()>available{return Err("DOM retained native event name allocation exceeds allowance.".into());}
            self.state.copies(name.capacity())?;name.push_str(event);
            let visit_count=path.len().checked_add(1).and_then(|count|count.checked_add(if bubbles {path.len()-1}else{0})).ok_or("Native event phase count overflow.")?;
            self.state.work(visit_count)?;
            let mut visits=Vec::new();
            self.state.reserve_dispatch_temporary(&mut visits,visit_count,path_bytes+keyboard_bytes+name.capacity())?;
            self.state.copies(visit_count.saturating_mul(std::mem::size_of::<(u32,bool,u8)>()))?;
            visits.extend(path.iter().skip(1).rev().map(|&node|(node,true,1)));
            visits.extend([(target,true,2),(target,false,2)]);
            if bubbles {visits.extend(path.iter().skip(1).map(|&node|(node,false,3)));}
            let bytes=path_bytes+keyboard_bytes+name.capacity()+visits.capacity()*std::mem::size_of::<(u32,bool,u8)>();
            Ok((name,visits,bytes))
        })();
        let (name,visits,native_storage)=match prepared {
            Ok(prepared)=>prepared,
            Err(error)=>{self.state.error(event,&error);self.runtime.cancel_microtasks();self.cancel_media_selection();return (false,false);}
        };
        self.state.retained+=native_storage;
        let handle=self.state.next_event;
        self.state.next_event=next;
        self.state.event=Some(Event {
            composed,path,handle,
            target:if target==WINDOW&&event=="load"{DOCUMENT}else{target},
            current:target,name,cancelable,bubbles,prevented:false,stopped:false,immediate:false,
            phase:0,passive:false,keyboard:self.state.pending_keyboard.take(),mouse,focus,
            submitter:if event=="submit"{Some(submitter)}else{None},
            timestamp:self.state.clock_ms(Instant::now()),
        });
        let mut complete = true;
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
            let snapshot=match self.state.listener_snapshot(current,event,capture) {
                Ok(snapshot)=>snapshot,
                Err(error)=>{self.state.error(event,&error);self.runtime.cancel_microtasks();self.cancel_media_selection();complete=false;break 'visits;}
            };
            for serial in &snapshot {
                if let Err(error)=self.state.work(1) {
                    self.state.error(event,&error);self.runtime.cancel_microtasks();self.cancel_media_selection();complete=false;break;
                }
                let Some((callback,options,property)) = self
                    .state
                    .listeners
                    .get(*serial)
                    .filter(|l| l.target == current)
                    .map(|l|(l.callback.clone(),l.options,l.property))
                else {
                    continue;
                };
                if options.once {
                    if let Err(error)=self.state.retain_listeners(|l|l.serial!=*serial) {
                        self.state.error(event,&error);
                        self.runtime.cancel_microtasks();self.cancel_media_selection();
                        complete=false;break;
                    }
                }
                if let Some(active) = &mut self.state.event {
                    active.passive = options.passive;
                }
                let result = self.runtime.invoke_event_listener(
                    callback,
                    &[Value::Host(handle)],
                    Value::Host(current),
                    &mut Bridge {
                        doc,
                        forms,
                        state: &mut self.state,
                    },
                );
                match result {
                    Ok(Value::Bool(false)) if property && cancelable => {
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
                    complete = false;
                    break;
                }
                if self.state.event.as_ref().is_some_and(|e| e.immediate) {
                    break;
                }
            }
            self.state.retained-=snapshot.capacity()*std::mem::size_of::<usize>();
            drop(snapshot);
            if !complete {break 'visits;}
        }
        // Handles expire instead of being reused: retained JS event references
        // can never mutate a later event's cancellation state.
        let prevented = self.state.event.take().is_some_and(|event| event.prevented);
        self.state.retained-=native_storage;
        (prevented, complete && !self.runtime.invocation_failed_fatally())
    }

    fn event_path(&mut self, doc: &Document, target: u32) -> Result<Vec<u32>, String> {
        self.event_path_with_detached(doc,target,false)
    }
    fn event_path_with_detached(&mut self,doc:&Document,target:u32,allow_detached:bool)->Result<Vec<u32>,String>{
        self.state.work(1)?;
        self.state.rebuild(doc)?;
        if target == WINDOW {
            let mut path=Vec::new();self.state.push_dispatch_temporary(&mut path,WINDOW,0)?;return Ok(path);
        }
        if target == DOCUMENT {
            let mut path=Vec::new();self.state.push_dispatch_temporary(&mut path,DOCUMENT,0)?;self.state.push_dispatch_temporary(&mut path,WINDOW,0)?;return Ok(path);
        }
        let node = node_index(target).ok_or("Unknown event target.")?;
        if !(allow_detached && self.state.detached_nodes.contains(&node))
            && !self.state.tree.active.get(node).copied().unwrap_or(false)
            && !(tag(doc, node) == "script" && self.state.dom.created.contains(&node))
            && !(tag(doc, node) == "img" && self.state.images.entries.contains_key(&node))
        {
            return Err("Event target is no longer connected.".into());
        }
        let mut path=Vec::new();self.state.push_dispatch_temporary(&mut path,target,0)?;
        {
            let mut parent = self.state.tree.parents[node];
            while let Some(node) = parent {
                self.state.work(1)?;
                if node != 0 {
                    self.state.push_dispatch_temporary(&mut path,node_handle(node),0)?;
                }
                parent = self.state.tree.parents[node];
            }
            if self.state.dom.connected.get(node).copied().unwrap_or(false) {
                self.state.push_dispatch_temporary(&mut path,DOCUMENT,0)?;self.state.push_dispatch_temporary(&mut path,WINDOW,0)?;
            }
        }
        Ok(path)
    }

    pub fn report(&self) -> String {
        if !self.state.enabled {
            return "JavaScript did not run for this document (disabled or blocked by document policy).\n".into();
        }
        let mut text = format!("ChezCat JavaScript subset\n{} script(s), {} active listener(s), {} error(s).\n{} pending timer(s), {} timer callback(s) executed.\nClassic scripts run after parsing. Events come from supported controls/forms and completed dynamic scripts/stylesheets.\nFunction timers use the document queue. Bounded eval follows the document CSP.\nLocation inspection reflects the displayed document; same-origin assign/replace/reload and href/search/hash writes queue navigation.\nSession cookies are memory-only with host-only/Domain scope and bundled public-suffix boundaries; document.cookie excludes HttpOnly entries.\nDynamic external classic scripts use CSP, MIME, request-blocking and bounded source checks; cross-origin HTTPS scripts omit credentials. Crossorigin-marked scripts require same-origin at every hop. Unmarked HTTP(S) dynamic LINK stylesheets use guarded cross-origin loading; marked stylesheets retain exact-origin checks. Checked innerHTML replaces ordinary HTML fragments with inert injected scripts and real image queues; unsupported parser recovery and native form/resource changes reject before replacement. Modules, persistent storage and Fetch are not implemented. XMLHttpRequest has a finite original asynchronous main-document HTTP(S) text-response adapter, buffered completion, native progress and explicit unsupported preflight/response/auth/child cases.\n\n", self.state.executed, self.state.listeners.len(), self.state.errors, self.state.timers.len(), self.state.timer_callbacks);
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
text.push_str(&format!("Classic scripts have independent execution jobs ({} steps / {} seconds / {} MiB string-copy work). Aggregate startup: {} steps / {} seconds / {} MiB string-copy work. Native-stack and allocation guards are separate.\nFresh detached image requests declined: {}. Ordinary connected HTML img source changes use cancelable real resource requests; current/pending/broken state and completion events follow actual decoding. Detached never-connected Image nodes only reflect attributes; no request or load event was fabricated. Scripted responsive source-set changes remain unsupported.\nBeacon requests declined: {}. sendBeacon returns false; no data was queued or transmitted.\n", self.state.limits.steps, self.state.limits.job_seconds, self.state.limits.copy_bytes / (1024 * 1024), self.state.limits.startup_steps, self.state.limits.startup_seconds, self.state.limits.startup_copy / (1024 * 1024), self.state.declined_images, self.state.declined_beacons));
        text.push_str(&format!(
            "Native work allowance: {} units per job, {} units during startup.\n",
            self.state.limits.host_work,
            self.state.limits.startup_host_work
        ));
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

    fn image_named(&mut self, name: crate::dom_string::DomStringRef<'_>) -> Result<Option<usize>, String> {
        if name.logical_bytes()>self.state.limits.host.string_bytes{return Err("Image collection key exceeds the host string allowance.".into());}
        if name.is_empty() {
            return Ok(None);
        }
        self.state.rebuild(self.doc)?;
        for position in 0..self.state.tree.order.len() {
            let node = self.state.tree.order[position];
            self.state.work(1)?;
            if tag(self.doc, node) == "img" {
                if self.collection_element_named_match(node_handle(node),name)? {
                    return Ok(Some(node));
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
            self.image_named(crate::dom_string::DomStringRef::Scalar(property))?
        };
        Ok(node.map_or(Value::Undefined, |node| Value::Host(node_handle(node))))
    }

    fn element_parent(&mut self, node: usize, document: bool) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        Ok(match self.state.tree.parents[node] {
            Some(0) if document => Value::Host(DOCUMENT),
            Some(parent) if parent != 0 && (document || matches!(self.doc.nodes[parent].kind,NodeKind::Element {..})) => Value::Host(node_handle(parent)),
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
        let text = value.scalar_text()?;
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

    fn dom_text(&mut self,value:&Value)->Result<DomString,String>{
        if let Value::StringUnits(units)=value {
            let logical=units.len().checked_mul(2).ok_or("DOMString input byte count overflow.")?;
            if logical>self.state.limits.host.string_bytes{return Err("DOMString input exceeds the host string allowance.".into());}
            let (projection,unpaired)=DomString::projected_size(units)?;
            let copies=projection.checked_add(if unpaired{logical}else{0}).ok_or("DOMString input copy count overflow.")?;
            self.state.copies(copies)?;
            DomString::from_units(units,self.state.retained_limit)
        }else{self.text(value).map(DomString::from)}
    }
    fn dom_string_value(state:&mut State,text:&DomString)->Result<Value,String>{
        if text.logical_bytes()>state.limits.host.string_bytes{return Err("DOMString result exceeds the host string allowance.".into());}
        state.copies(text.logical_bytes())?;
        Self::dom_string_copy(text)
    }
    fn dom_string_copy(text:&DomString)->Result<Value,String>{
        if let Some(units)=text.raw_units() {
            let mut copy=Vec::new();copy.try_reserve_exact(units.len()).map_err(|_|"Cannot reserve DOMString result code units.")?;
            copy.extend_from_slice(units);Ok(Value::StringUnits(copy))
        }else{Ok(Value::String(script_clone_node::copy_text(text.scalar()?)?))}
    }
    fn dom_string_owned(text:DomString)->Value{
        let (scalar,units)=text.into_parts();
        match units {Some(units)=>Value::StringUnits(units),None=>Value::String(scalar)}
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
        self.attribute_named_prepared(node,name,value,script_attr::InputPreparation::Pending)
    }
    fn attribute_named_prepared(&mut self,node:usize,name:crate::html::AttributeName,value:Option<String>,input:script_attr::InputPreparation)->Result<(),String>{
        self.attribute_dom_named_prepared(node,name,value.map(DomString::from),input)
    }
    fn attribute_dom_named_prepared(&mut self,node:usize,name:crate::html::AttributeName,value:Option<DomString>,input:script_attr::InputPreparation)->Result<(),String>{
        let meta = self.meta_content_attribute(node, &name);
        if meta { self.check_meta_change(node, &name, value.as_ref())?; }
        if value.as_ref().is_some_and(|v|v.raw_units().is_some()) && !self.html_id_attribute(node,&name) && !self.html_passive_name_attribute(node,&name) && !meta && !self.resource_charset_attribute(node,&name) && !self.html_class_attribute(node,&name) && !self.html_text_reflection_attribute(node,&name) && !script_attr::html_aria_attribute(self.doc.nodes[node].namespace,&name) && !script_attr::html_label_or_output_for_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_output_form_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_time_datetime_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(script_attr::html_area_alt_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_optgroup_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_option_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_option_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(((script_attr::html_select_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_button_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_textarea_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))))) && !script_attr::html_area_coords_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_shape_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_download_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_hreflang_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_rel_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_href_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_referrer_policy_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_area_ping_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(script_attr::html_map_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_param_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_param_metadata_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))) && !script_attr::html_mod_datetime_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_data_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_heading_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_ul_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_li_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_ol_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_li_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(script_attr::html_pre_width_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_textarea_dimensions_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((script_attr::html_textarea_max_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_input_max_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||((script_attr::html_textarea_min_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_input_min_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||(((((((script_attr::html_textarea_wrap_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_button_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_button_type_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_step_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_max_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_min_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_pattern_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||(script_attr::html_textarea_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_select_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((script_attr::html_input_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((script_attr::html_input_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_input_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_input_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||(script_attr::html_button_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(script_attr::html_button_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_button_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))))||(((script_attr::html_form_autocomplete_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((script_attr::html_form_method_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_form_action_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_form_enctype_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||((script_attr::html_form_accept_charset_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_form_rel_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||((script_attr::html_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_button_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_form_target_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))||((script_attr::html_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_button_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_form_no_validate_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))))))))))) && !script_attr::html_ol_start_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_ul_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_dir_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_menu_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_ol_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_hr_no_shade_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_ol_reversed_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !(script_attr::html_area_no_href_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||(((script_attr::html_option_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((((script_attr::html_select_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_button_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_input_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||(script_attr::html_textarea_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||((script_attr::html_textarea_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_input_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||(script_attr::html_textarea_read_only_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_input_read_only_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))))||script_attr::html_select_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)))||script_attr::html_option_default_selected_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))||script_attr::html_optgroup_disabled_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name))) && !script_attr::html_dl_compact_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_paragraph_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_hr_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_hr_color_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_font_size_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_font_face_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_font_color_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_br_clear_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_hr_size_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_hr_width_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_legend_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_div_align_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_meter_number_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !script_attr::html_progress_number_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name) && !name.namespace_uri().is_some_and(|uri|uri=="http://www.w3.org/2000/xmlns/") && !name.qualified.starts_with("data-"){return Err("Unpaired attribute DOMString integration currently requires supported HTML ID or passive metadata.".into());}
        // HTML style sheet media is a live reference to the element's media
        // attribute. Type and nonce are checked when updating the style block
        // on connection; changing either attribute alone does not update it.
        let sheet_media = self.created_html_style(node) && name.html_is("media");
        if sheet_media {
            script_stylesheets::validate_inline_sheet_media(value.as_deref())?;
        }
        let repaint_sheet = sheet_media && self.state.dom.connected[node]
            && self.doc.attr(node, "media") != value.as_deref();
        let input = match input{script_attr::InputPreparation::Pending=>self.prepare_input_attribute(node,&name,value.as_deref())?,script_attr::InputPreparation::Ready(input)=>input};
        let button = self.prepare_button_attribute(node, &name, value.as_deref())?;
        let group_label = self.prepare_optgroup_label(node,&name,value.as_ref())?;
        let option_label = self.prepare_option_label(node,&name,value.as_ref())?;
        let option_value = self.prepare_option_value(node,&name,value.as_ref())?;
        let select_name = self.prepare_select_name(node,&name,value.as_ref())?.or(self.prepare_input_control_name(node,&name,value.as_ref())?);
        let textarea_name = self.prepare_textarea_name(node,&name,value.as_ref())?;
        let option_default = self.prepare_option_default_selected(node,&name,value.as_ref())?;
        let select_required=script_attr::html_select_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name);
        let select_required_present=value.is_some();
        let textarea_required=script_attr::html_textarea_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_input_required_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name);
        let textarea_required_present=value.is_some();
        let textarea_read_only=script_attr::html_textarea_read_only_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name)||script_attr::html_input_read_only_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name);
        let textarea_read_only_present=value.is_some();
        let textarea_max_length=script_attr::html_textarea_max_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name);
        let input_max_length=script_attr::html_input_max_length_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),&name);
        let textarea_max_length_value=if textarea_max_length{self.state.work(self.forms.controls.len().saturating_add(value.as_ref().map_or(0,DomString::used_bytes)).saturating_add(1))?;Some(forms::textarea_max_length(value.as_ref().map(DomString::as_str)))}else{None};
        let input_max_length_value=if input_max_length{self.state.work(self.forms.controls.len().saturating_add(value.as_ref().map_or(0,DomString::used_bytes)).saturating_add(1))?;Some(forms::input_max_length(value.as_ref().map(DomString::as_str)))}else{None};
        if textarea_read_only{self.state.work(self.forms.controls.len().saturating_add(1))?;}
        if textarea_required{self.state.work(self.forms.controls.len().saturating_add(1))?;}
        if select_required{self.state.work(self.forms.controls.len().saturating_add(1))?;}
        let media_source=name.html_is("src")&&self.media_element_receiver(node_handle(node));let media_change=media_source&&value.is_some();
        if media_change&&!self.media_source_prepared(){return Err("Media source change requires its reserved stable-state task.".into());}
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html && tag(self.doc,node)=="video"
            && !media_source && ["src","width","height","poster","autoplay","loop","muted","preload","crossorigin"].iter().any(|key|name.html_is(key)) {
            return Err("Changing parser-authored video source or native media attributes is not implemented; no attribute was changed.".into());
        }
        if name.namespace.is_empty() { self.frame_attribute_guard(node, &name.qualified)?; }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else {
            return Err("Not an element.".into());
        };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|attr| attr.name.bytes()).sum())?;
        let slot = attrs.iter().position(|attr| attr.name.same_expanded_name(&name));
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&attrs.len()>=self.state.limits.host.element_attributes&&value.is_some()&&slot.is_none(){
            return Err(format!("Element attribute limit reached ({}).",self.state.limits.host.element_attributes));
        }
        if name.html_is("id") {
            let old=self.doc.dom_attr(node,"id");
            self.state.work(old.map_or(0,DomString::logical_bytes).min(value.as_ref().map_or(0,DomString::logical_bytes)).saturating_add(1))?;
            if old!=value.as_ref() && form_id_guards::used(self.doc,self.forms,self.state,[old,value.as_ref()])? {
                return Err("Changing an id used by an explicit form association is not implemented.".into());
            }
        }
        if name.html_is("disabled") {
            self.charge_disabled_refresh(node, value.is_some())?;
        }
        let canvas_resize = if slot.is_some() || value.is_some() {
            self.prepare_canvas_resize(node, &name, value.as_deref())?
        } else { None };
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        let old_id=if name.html_is("id"){let NodeKind::Element{id,..}=&self.doc.nodes[node].kind else{unreachable!()};id.as_ref().map_or(0,String::capacity)}else{0};
        let old = slot.map_or(0, |slot| attrs[slot].owned_bytes()).checked_add(old_id).ok_or("Attribute old payload overflow.")?;
        // Existing expanded names retain the original QName/prefix and order.
        let new = value.as_ref().map_or(0, |value| value.capacity() + slot.map_or_else(|| name.owned_bytes(), |slot| attrs[slot].name.owned_bytes()));
        let detached_attr = if value.is_none() {
            self.state.attribute_lookup_work()?;
            let position=self.state.attribute_position(node,&name).ok();
            if let Some(position)=position{self.state.copies((self.state.attached_attributes.len()-position-1)*size_of::<usize>())?;}
            position.map(|position|(position,self.state.attached_attributes[position]))
        } else { None };
        let retained_value = if detached_attr.is_some() { slot.map_or(0, |slot| attrs[slot].value.capacity()) } else { 0 };
        let copied_id=if name.html_is("id") {
            self.state.copies(value.as_ref().map_or(0,DomString::used_bytes))?;
            value.as_deref().map(script_clone_node::copy_text).transpose()?
        } else {None};
        let native_bytes = input.as_ref().map_or(0, forms::InputPlan::retained_bytes);
        let selection_scratch=option_default.as_ref().map_or(0,forms::OptionDefaultPlan::bytes).checked_add(select_name.as_ref().map_or(0,forms::ControlNamePlan::bytes).checked_add(textarea_name.as_ref().map_or(0,forms::ControlNamePlan::bytes)).ok_or("Native name scratch overflow.")?).ok_or("Native attribute scratch overflow.")?;
        if selection_scratch>self.state.retained_limit.saturating_sub(self.state.retained).saturating_sub(new){return Err("Native OPTION selection and attribute retained allowance exhausted.".into());}
        let new_bytes = new.checked_add(copied_id.as_ref().map_or(0,String::capacity)).and_then(|bytes|bytes.checked_add(retained_value)).and_then(|bytes|bytes.checked_add(native_bytes))
            .ok_or("Attribute/native INPUT retained payload overflow.")?;
        let occupied=self.state.retained.saturating_sub(old).checked_add(new_bytes).ok_or("Attribute retained payload overflow.")?;
        let available=self.state.retained_limit.checked_sub(occupied).and_then(|n|n.checked_sub(selection_scratch)).ok_or("DOM retained attribute backing allowance exhausted.")?;
        let (replacement,backing)=if slot.is_none()&&value.is_some(){retained_storage::grow(attrs,1,available)?}else{(None,0)};
        if replacement.is_some(){self.state.copies(attrs.len()*size_of::<crate::html::Attribute>())?;}
        // Spare capacity is retained memory, not copied text. Keep payload
        // work and newly initialized records separate from allocation bytes.
        let payload_work=value.as_ref().map_or(0,|value|value.used_bytes()+slot.map_or_else(||name.bytes(),|slot|attrs[slot].name.bytes()));
        let detached_work=if detached_attr.is_some(){slot.map_or(0,|slot|attrs[slot].value.used_bytes())}else{0};
        let record_work=if slot.is_none()&&value.is_some(){size_of::<crate::html::Attribute>()}else{0};
        let work=payload_work.checked_add(copied_id.as_ref().map_or(0,String::len)).and_then(|bytes|bytes.checked_add(detached_work)).and_then(|bytes|bytes.checked_add(native_bytes)).and_then(|bytes|bytes.checked_add(record_work)).ok_or("Attribute payload work overflow.")?;
        self.state.replace_storage(old,new_bytes.checked_add(backing).ok_or("Attribute retained backing overflow.")?,work)?;
        let NodeKind::Element { attrs, id, .. } = &mut self.doc.nodes[node].kind else {
            return Err("Not an element.".into());
        };
        if let Some(mut next)=replacement{next.append(attrs);*attrs=next;}
        if name.html_is("id") {
            *id = copied_id;
            self.state.dirty = true;
        }
        let disabled = name.html_is("disabled");
        let tab_index=name.html_is("tabindex");
        if let Some(index) = slot {
            if let Some(value) = value {
                attrs[index].value = value;
            } else {
                let removed = attrs.remove(index).value;
                if let Some((position,attr)) = detached_attr {
                    self.state.attached_attributes.remove(position);
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
        if let Some(plan) = input { plan.commit(self.forms); }
        if let Some(plan) = button { plan.commit(self.forms); }
        if let Some(plan) = group_label { plan.commit(self.forms); }
        if let Some(plan) = option_label { plan.commit(self.forms); }
        if let Some(plan) = option_value { plan.commit(self.forms); }
        if let Some(plan) = select_name { plan.commit(self.forms); }
        if let Some(plan) = textarea_name { plan.commit(self.forms); }
        if let Some(plan) = option_default { plan.commit(self.forms); }
        if select_required{if let Some(control)=self.forms.controls.get_mut(&node){control.required=select_required_present;}}
        if textarea_required{if let Some(control)=self.forms.controls.get_mut(&node){control.required=textarea_required_present;}}
        if textarea_read_only{if let Some(control)=self.forms.controls.get_mut(&node){control.readonly=textarea_read_only_present;}}
        if let Some(maximum)=textarea_max_length_value{if let Some(control)=self.forms.controls.get_mut(&node){control.max_length=maximum;}}
        if let Some(maximum)=input_max_length_value{if let Some(control)=self.forms.controls.get_mut(&node){control.max_length=maximum;}}
        if tab_index {
            let value=self.doc.attr(node,"tabindex").and_then(forms::parse_tab_index).unwrap_or(0);
            if let Some(control)=self.forms.controls.get_mut(&node) {control.tab_index=value;}
        }
        if slot.is_some() || new != 0 {
            self.state.canvas_contexts.commit_resize(node, canvas_resize);
            self.state.dom_committed();
        }
        if repaint_sheet || self.state.tree.active.get(node).copied().unwrap_or(false) {
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

    fn text_content(&mut self, node: usize) -> Result<DomString, String> {
        self.state.rebuild(self.doc)?;
        script_text_content::deadline(self.state)?;
        let mut parts = Vec::new();
        let mut stack = Vec::new();
        script_text_replacement::push_values(self.state,&mut stack,node,0)?;
        while let Some(node) = stack.pop() {
            self.state.work(1)?;
            // Template contents belong to their separate fragment, even when
            // the immutable parser input still parks source children here.
            if self.doc.nodes[node].namespace==crate::html::Namespace::Html && tag(self.doc,node)=="template" {
                continue;
            }
            self.check_iframe_child_topology(node)?;
            if let NodeKind::Text(text) | NodeKind::Cdata(text) = &self.doc.nodes[node].kind {
                let other=script_text_replacement::bytes(&[&stack],0)?;
                script_text_replacement::push_values(self.state,&mut parts,text,other)?;
            }
            for &child in self.doc.nodes[node].children.iter().rev() {
                let other=script_text_replacement::bytes(&[&parts],0)?;
                script_text_replacement::push_values(self.state,&mut stack,child,other)?;
            }
        }
        drop(stack);
        self.state.work(parts.len().checked_mul(6).ok_or("textContent part work overflow.")?)?;
        // Admit inspection of the source payload before UTF-16 sizing scans.
        let inspected=parts.iter().try_fold(0usize,|bytes,part|bytes.checked_add(part.used_bytes()).ok_or("textContent source work overflow."))?;
        self.state.copies(inspected)?;
        let available=script_text_content::available(self.state,script_text_replacement::bytes(&[&parts],0)?)?;
        let (copies,budget,raw)=script_text_content::plan(&parts,self.state.limits.host.string_bytes,available)?;
        self.state.copies(copies.saturating_sub(inspected))?;
        script_text_content::deadline(self.state)?;
        let result=DomString::concat_with_scratch(&parts,budget,raw)?;
        script_text_content::deadline(self.state)?;
        Ok(result)
    }

    fn text_content_container(&self, node: usize) -> bool {
        text_container(tag(self.doc,node))
            || self.doc.nodes.get(node).is_some_and(|item| item.namespace==crate::html::Namespace::Html
                && matches!(item.kind,NodeKind::Element{..}) && (matches!(tag(self.doc,node),"output"|"optgroup"|"option") || script_clone_node::ordinary_clone_element(tag(self.doc,node))))
    }

    fn set_text_content(&mut self, node: usize, text: DomString) -> Result<(), String> {
        if !self.title_receiver(node_handle(node)) {return self.replace_main_text_content(node,text);}
        self.state.rebuild(self.doc)?;
        if self.state.tree.title!=Some(node) {return self.replace_main_text_content(node,text);}
        let display=self.normalized_title(&text)?.into_parts().0;
        let old=self.doc.title.capacity().min(self.state.retained);
        let new=display.capacity();
        self.state.replace_storage(old,new,0)?;
        if let Err(message)=self.replace_main_text_content(node,text) {
            self.state.retained=self.state.retained-new+old;
            return Err(message);
        }
        self.doc.title=display;
        Ok(())
    }

    fn replace_main_text_content(&mut self, node: usize, text: DomString) -> Result<(), String> {
        self.state.work(1)?;
        if self.state.execution_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err("DOM computation time limit reached.".into());
        }
        if self.doc.nodes[node].namespace == crate::html::Namespace::Html && tag(self.doc,node) == "optgroup" {
            self.state.rebuild(self.doc)?;
            self.mutation_ancestors(node)?;
        }
        if self.doc.nodes[node].namespace == crate::html::Namespace::Html && tag(self.doc,node) == "option" { self.state.rebuild(self.doc)?; }
        let textarea_default = self.prepare_textarea_default_content(node,&text)?;
        let option_content = self.prepare_option_content(node,&text)?;
        let option_bytes = option_content.as_ref().map_or(0,|plan|plan.bytes).checked_add(textarea_default.as_ref().map_or(0,|plan|plan.bytes)).ok_or("Content native scratch overflow.")?;
        let detached_style = self.created_html_style(node)
            && self.state.detached_nodes.contains(&node)
            && self.state.dom.connected.get(node) == Some(&false);
        if !self.text_content_container(node) && textarea_default.is_none() && !self.title_receiver(node_handle(node)) && !detached_style && !matches!(self.doc.nodes[node].kind,NodeKind::DocumentFragment) {
            return Err(
                "textContent replacement requires an ordinary text container, button or detached created HTML style element.".into(),
            );
        }
        // Complete validation before touching any node. Control models and
        // already-loaded script/style/image resources must never become stale.
        let mut descendants = Vec::new();
        let mut stack = Vec::new();
        for &child in &self.doc.nodes[node].children {
            script_text_replacement::push(self.state,&mut stack,child,text.capacity().checked_add(option_bytes).ok_or("OPTION content traversal overflow.")?)?;
        }
        while let Some(child) = stack.pop() {
            self.state.work(1)?;
            if let NodeKind::Element { tag, .. } = &self.doc.nodes[child].kind {
                if (!self.text_content_container(child) || matches!(tag.as_str(),"button"|"option"))&&!self.movable_template_host(child) {
                    return Err("textContent cannot remove controls, resources, scripts or structural elements.".into());
                }
            }
            let other=script_text_replacement::bytes(&[&stack],text.capacity().checked_add(option_bytes).ok_or("OPTION content traversal overflow.")?)?;
            script_text_replacement::push(self.state,&mut descendants,child,other)?;
            for &nested in &self.doc.nodes[child].children {
                let other=script_text_replacement::bytes(&[&descendants],text.capacity().checked_add(option_bytes).ok_or("OPTION content traversal overflow.")?)?;
                script_text_replacement::push(self.state,&mut stack,nested,other)?;
            }
        }
        drop(stack);
        if !text.is_empty() && self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes) >= self.state.node_limit {
            return Err("DOM appended-node limit reached (256 per document).".into());
        }
        let mut captions = Vec::new();
        let mut current = Some(node);
        while let Some(index) = current {
            self.state.work(1)?;
            if tag(self.doc, index) == "button" {
                self.state.work(self.doc.nodes.len())?;
                let other=script_text_replacement::bytes(&[&descendants],text.capacity().checked_add(option_bytes).ok_or("OPTION content traversal overflow.")?)?;
                script_text_replacement::push(self.state,&mut captions,index,other)?;
            }
            current = self.state.tree.parents.get(index).copied().flatten();
        }
        // Detached nodes remain real objects for saved handles/NodeLists. Their
        // contents remain charged to the document budget until realm teardown.
        // Element textContent replaces all children. Saved Text objects keep
        // their old data and identity; a nonempty replacement creates a new
        // Text, while the empty string creates no child at all.
        let detached_parent = self.state.detached_nodes.contains(&node);
        let text_bytes = if text.is_empty() { 0 } else { text.capacity() };
        let scratch=script_text_replacement::bytes(&[&descendants,&captions],if text.is_empty(){text.capacity()}else{0})?.checked_add(option_bytes).ok_or("OPTION content scratch overflow.")?;
        let count = usize::from(!text.is_empty());
        let minimum = self.main_node_minimum(count)?;
        let (child_growth, child_bytes) = if text.is_empty() { (None, 0) } else {
            let available = self.state.retained_limit.saturating_sub(self.state.retained)
                .checked_sub(scratch).and_then(|bytes|bytes.checked_sub(text_bytes)).and_then(|bytes|bytes.checked_sub(minimum))
                .ok_or("textContent child/text/node storage allowance exhausted.")?;
            retained_storage::capacity(&self.doc.nodes[node].children, 1, available)?
        };
        let available = self.state.retained_limit.saturating_sub(self.state.retained)
            .checked_sub(scratch).and_then(|bytes|bytes.checked_sub(text_bytes)).and_then(|bytes|bytes.checked_sub(child_bytes)).ok_or("textContent node backing allowance exhausted.")?;
        let (node_growth, node_bytes) = self.prepare_main_nodes(count, available)?;
        // An empty replacement creates no retained Text, even if its incoming
        // buffer has spare capacity. Old detached children remain charged.
        let retained = text_bytes.checked_add(child_bytes).and_then(|bytes|bytes.checked_add(node_bytes)).ok_or("textContent retained storage overflow.")?;
        let work = text.len().checked_add(if text.is_empty(){0}else{size_of::<usize>()}).ok_or("textContent copy work overflow.")?;
        self.state.prepare_range_changes(1,descendants.len())?;
        if option_bytes!=0 && self.state.retained.checked_add(retained).and_then(|n|n.checked_add(scratch)).is_none_or(|n|n>self.state.retained_limit){return Err("textContent combined scratch allowance exhausted.".into());}
        self.state.replace_storage(0, retained, work)?;
        self.state.ranges_remove(node,0,self.doc.nodes[node].children.len(),&descendants);
        self.commit_main_nodes(node_growth);
        self.state.detached_nodes.extend(descendants);
        if let Some(children) = child_growth { self.doc.nodes[node].children = children; }
        self.doc.nodes[node].children.clear();
        if !text.is_empty() {
            let child = self.doc.nodes.len();
            self.doc.nodes.push(Node {
                kind: NodeKind::Text(text),
                children: Vec::new(),
                namespace: crate::html::Namespace::None,
            });
            self.doc.nodes[node].children.push(child);
            if detached_parent { self.state.detached_nodes.insert(child); }
        }
        // Updating a span inside a button must update the native caption too.
        for index in captions {
            if let Some(control) = self.forms.controls.get_mut(&index) {
                control.caption = forms::caption(self.doc, index, control.kind);
            }
        }
        if let Some(plan) = option_content { plan.commit(self.forms); }
        if let Some(plan) = textarea_default { plan.commit(self.forms); }
        self.state.dirty = true;
        self.state.dom_committed();
        if self.state.tree.active.get(node).copied().unwrap_or(false) {
            self.state.layout_changed();
        }
        Ok(())
    }

    fn listener_target(&mut self, handle: u32, event: &str) -> Result<(), String> {
        // Event types are case-sensitive DOM strings, including custom and
        // empty names. Registering one does not create a native event source.
        if self.state.event_type_refused(event.len()) {
            return Err(format!("Event type exceeds the {}-byte limit.",self.state.event_type_allowance()));
        }
        if !self.dispatch_receiver(handle) {return Err("Listener receiver is not a supported EventTarget.".into());}
        Ok(())
    }

    fn handler_event_target(&mut self, handle: u32, event: &str) -> Result<(), String> {
        // Keep native on* property availability unchanged. Custom names are
        // accepted through add/removeEventListener on existing EventTargets.
        let allowed = self.xhr_index(handle).is_some() && matches!(event,"readystatechange"|"loadstart"|"progress"|"abort"|"error"|"load"|"timeout"|"loadend") || event == "abort" && self.abort_signal_owner(handle).is_some()
            || event == "message" && self.message_port_owner(handle).is_some()
            || self.media_element_receiver(handle) && script_media::event_name(event) || matches!(
            event,
            "click" | "auxclick" | "input" | "change" | "submit" | "reset" | "invalid" | "keydown" | "keyup" | "focus" | "blur" | "focusin" | "focusout"
        ) || event == "DOMContentLoaded" && matches!(handle, DOCUMENT | WINDOW)
            || event == "load" && handle == WINDOW
            || matches!(event, "load" | "error")
                && node_index(handle).is_some_and(|node| {
                    tag(self.doc, node) == "img"
                        || self.dom_script(node) || self.dom_stylesheet(node)
                });
        if !allowed {
            return Err(format!("Event {event:?} is not supported on this target."));
        }
        self.listener_target(handle, event)
    }

    fn handler(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        let Some(event) = (property == "onabort" && self.abort_signal_owner(object).is_some())
            .then_some("abort").or_else(||self.xhr_handler_event(object,property)).or_else(|| self.media_handler_event(object,property))
            .or_else(|| self.message_handler_event(object, property)) else {
            return Ok(None);
        };
        let object = self.handler_target(object, event);
        self.handler_event_target(object, event)?;
        self.state.work(self.state.listeners.len())?;
        Ok(Some(
            self.state
                .listeners
                .iter()
                .find(|l| l.target == object && l.event == event && l.property)
                .map_or(Value::Null, |l| l.callback.clone()),
        ))
    }

    fn set_handler(&mut self, object: u32, property: &str, value: Value) -> Result<bool, String> {
        let Some(event) = (property == "onabort" && self.abort_signal_owner(object).is_some())
            .then_some("abort").or_else(||self.xhr_handler_event(object,property)).or_else(|| self.media_handler_event(object,property))
            .or_else(|| self.message_handler_event(object, property)) else {
            return Ok(false);
        };
        let object = self.handler_target(object, event);
        self.handler_event_target(object, event)?;
        match value {
            Value::Function(_) => {
                self.state
                    .add_listener(object, event, value, true, ListenerOptions::default())?
            }
            Value::Null | Value::Undefined => self.state.retain_listeners(|l| !(l.target == object && l.event == event && l.property))?,
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
        if !matches!(args[1], Value::Function(_) | Value::Object(_) | Value::Host(_)) {
            return Err("An EventListener callback must be an object or null.".into());
        }
        if method == "addEventListener" {
            self.state
                .add_listener(object, &event, args[1].clone(), false, options)?;
        } else {
            self.state.retain_listeners(|l| {
                !(l.target == object
                    && l.event == event
                    && !l.property
                    && l.options.capture == options.capture
                    && same_event_listener(&l.callback, &args[1]))
            })?;
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
        if name == "transform" {
            let value = script_transform::specified(style, self.state)?;
            return self.string(&value);
        }
        if name == "animation-timing-function" {
            let value = script_inline_style::easing_declaration(style, self.state, script_inline_style::EasingText::Cssom)?.map_or_else(String::new, |(value, _)| value);
            return self.string(&value);
        }
        let Some((_, raw)) = script_inline_style::winner(style, name, self.state)? else {
            return self.string("");
        };
        if name == "order" { script_inline_style::order_source(raw, self.state)?; }
        let clean;
        let raw = if raw.contains("/*") { clean = crate::css::without_comments(raw); &clean } else { raw };
        let specified = crate::css::declaration_priority(raw).0;
        let value = if name=="z-index"{self.state.work(specified.len().saturating_mul(2))?;self.state.copies(specified.len().saturating_mul(2))?;crate::css::z_index::specified(specified).ok_or("Invalid selected z-index declaration.")?}
        else if name=="order"{crate::css::order::specified(specified).ok_or("Invalid selected order declaration.")?}
        else if matches!(name,"visibility"|"pointer-events"){
            self.state.work(specified.len().saturating_mul(2))?;self.state.copies(specified.len().saturating_mul(2))?;
            crate::css::specified_interaction_value(name,specified).ok_or("Invalid selected interaction declaration.")?
        } else if name == "touch-action" {
            self.state.work(specified.len())?;
            crate::css::specified_touch_action(specified).ok_or("Invalid selected touch-action.")?.to_owned()
        } else if name == "opacity" {
            let parsed = metered_opacity(specified, self.state)?
                .ok_or("The selected inline opacity is invalid.")?;
            parsed.serialize()?
        } else {
            specified.to_owned()
        };
        self.string(&value)
    }

    fn set_style(
        &mut self,
        node: usize,
        name: &str,
        value: &str,
        priority: &str,
    ) -> Result<(), String> {
        let normalized_order;
        let value = if name == "order" && !value.is_empty() {
            if !priority.is_empty() && !priority.eq_ignore_ascii_case("important") { return Ok(()); }
            normalized_order = match script_inline_style::normalize_order(value, self.state)? {
                Some(value) => value,
                None => return Ok(()),
            };
            normalized_order.as_str()
        } else { value };
        validate_css_value(name, value, self.state.limits.host.string_bytes)?;

        if matches!(name, "color" | "background-color") && !value.is_empty() && !crate::css::inline_color_value(value) {
            return Ok(());
        }
        let specified;
        let checked = matches!(name, "transform" | "opacity" | "touch-action");
        let value = if checked && !value.is_empty() {
            if value.trim().is_empty() || !priority.is_empty() && !priority.eq_ignore_ascii_case("important") {
                return Ok(());
            }
            specified = if name == "touch-action" {
                self.state.work(value.len())?;
                let Some(parsed)=crate::css::specified_touch_action(value) else {return Ok(());};
                self.state.copies(parsed.len())?;
                parsed.to_owned()
            } else if name == "opacity" {
                let Some(parsed) = metered_opacity(value, self.state)? else { return Ok(()); };
                parsed.serialize()?
            } else {
                match script_transform::normalize(value, self.state)? {
                    Ok(value) => value,
                    Err(crate::css::TransformSyntax::Invalid) => return Ok(()),
                    Err(crate::css::TransformSyntax::Unsupported) => return Err("This transform exceeds the supported translation grammar.".into()),
                }
            };
            specified.as_str()
        } else if name=="z-index"&&!value.is_empty(){
            if !priority.is_empty()&&!priority.eq_ignore_ascii_case("important"){return Ok(());}
            self.state.work(value.len().saturating_mul(2))?;self.state.copies(value.len().saturating_mul(2))?;specified=match crate::css::z_index::specified(value){Some(value)=>value,None=>return Ok(())};specified.as_str()
        } else if matches!(name,"visibility"|"pointer-events")&&!value.is_empty(){
            if !priority.is_empty()&&!priority.eq_ignore_ascii_case("important"){return Ok(());}
            self.state.work(value.len().saturating_mul(2))?;self.state.copies(value.len().saturating_mul(2))?;
            specified=match crate::css::specified_interaction_value(name,value){Some(value)=>value,None=>return Ok(())};specified.as_str()
        } else if name == "animation-timing-function" && !value.is_empty() {
            if !priority.is_empty() && !priority.eq_ignore_ascii_case("important") { return Ok(()); }
            specified = match script_inline_style::normalize_easing(value, self.state, script_inline_style::EasingText::Specified)? { Some(value) => value, None => return Ok(()) };
            specified.as_str()
        } else {value};
        let priority = if value.is_empty() {""} else {priority};
        if !priority.is_empty() && !priority.eq_ignore_ascii_case("important") {
            return Err("CSS priority must be empty or important.".into());
        }
        let existing = self.doc.attr(node, "style").unwrap_or("");
        if existing.len() > self.state.limits.host.string_bytes {
            return Err(format!("Inline style exceeds {} KiB.", self.state.limits.host.string_bytes / 1024).into());
        }
        self.state.copies(existing.len())?;
        if name == "animation-timing-function" && !value.is_empty() {
            if let Some((prior, important)) = script_inline_style::easing_declaration(existing, self.state, script_inline_style::EasingText::Specified)? {
                if prior == value && important == priority.eq_ignore_ascii_case("important") { return Ok(()); }
            }
        }
        if name == "transform" && value.is_empty()
            && matches!(script_transform::declaration(existing, self.state)?, script_transform::Declaration::Absent)
        {
            return Ok(());
        }
        self.state.work(1 + existing.len().saturating_mul(2))?;
        self.state.copies(existing.len().saturating_add(name.len()).saturating_add(value.len()).saturating_add(14))?;
        let limit = self.state.limits.host.string_bytes;
        let Some(style) = script_inline_style::replace(existing, name, value.trim(), priority, limit, self.state)? else { return Ok(()); };
        self.attribute(node, "style", Some(style))
    }
}

fn node_traversal_property(operation: &str) -> Option<&'static str> {
    operation.strip_prefix("\0Node.get.")?;
    NODE_TRAVERSAL_GETTERS.iter()
        .find_map(|&(property, getter)| (operation == getter).then_some(property))
}

impl Bridge<'_> {
    /// Author data uses VM backing only for represented CharacterData owners.
    /// Native content/resource mutation eligibility remains independently checked.
    fn character_data_author_receiver(&self, object: u32) -> bool {
        if matches!(self.inert_node_type(object), Some(3 | 4 | 7 | 8)) {
            return true;
        }
        node_index(object).is_some_and(|node| {
            self.dom_accessible(node)
                && self.doc.nodes.get(node).is_some_and(|node| node.kind.character_data().is_some())
        })
    }
    fn text_or_comment_receiver(&self, object: u32) -> bool {
        self.comment_receiver(object) || self.slottable(object) == Some(Slottable::Text)
    }

    fn node_traversal_receiver(&self, object: u32) -> bool {
        object == DOCUMENT || self.attr_receiver(object) || node_index(object).is_some_and(|node| self.dom_accessible(node))
            || self.inert_children(object).is_some()
    }

    fn node_metadata_property(&self, object:u32, property:&str)->bool {
        matches!(property,"ownerDocument"|"isConnected"|"baseURI") && self.text_content_receiver(object)
            && (property!="baseURI" || !self.form_named_control(object,property))
    }

    /// Properties handled by ordinary lookup on the node's interface prototypes.
    fn inherited_node_property(&self, object: u32, property: &str) -> bool {
        if property=="valueOf"&&(self.area_href_receiver(object)||self.native_anchor(object)){return true;}
        if property=="baseURI" && self.form_named_control(object,property) {return false;}
        if matches!(property, "nodeValue" | "hasChildNodes") && self.text_content_receiver(object) { return !self.form_named_control(object, property); }
        if matches!(property,"insertAdjacentText"|"insertAdjacentElement") && self.collection_interface_receiver(object)==Some(false) {
            return !self.form_named_control(object,property);
        }
        if property=="text" && self.title_receiver(object) {return true;}
        if script_anchor_url::property(property) && self.native_anchor(object) {return true;}
        if property=="remove" && (self.collection_interface_receiver(object)==Some(false)||self.character_data_receiver(object)||self.doctype_receiver(object)) {
            return !self.form_named_control(object,property);
        }
        if self.canonical_collection_member(object, property)||self.canonical_name_list_member(object,property) { return true; }
        if self.node_metadata_property(object,property) {return true;}
        if self.charset_member(object, property) { return true; }
        if self.meta_member(object, property) { return true; }
        if self.aria_reflection_member(object,property){return true;}
        if self.element_id_member(object,property){return true;}
        if self.element_class_name_member(object,property){return true;}
        if self.control_labels_member(object,property){return true;}
        if self.html_label_form_member(object,property){return true;}
        if self.output_form_member(object,property){return true;}
        if self.control_type_member(object,property){return true;}
        if self.form_length_member(object,property){return true;}
        if self.meter_numeric_member(object,property){return true;}
        if self.progress_numeric_member(object,property){return true;}
        if self.legend_form_member(object,property){return true;}
        if self.option_form_member(object,property){return true;}
        if self.option_index_member(object,property){return true;}
        if self.textarea_text_length_member(object,property){return true;}
        if self.map_areas_member(object,property){return true;}
        if self.datalist_options_member(object,property){return true;}
        if self.time_datetime_member(object,property){return true;}
        if self.area_alt_member(object,property){return true;}
        if self.optgroup_label_member(object,property){return true;}
        if self.option_label_member(object,property){return true;}
        if self.option_text_member(object,property){return true;}
        if self.textarea_default_value_member(object,property){return true;}
        if self.option_value_member(object,property){return true;}
        if self.select_name_member(object,property){return true;}
        if self.textarea_name_member(object,property){return true;}
        if self.area_coords_member(object,property){return true;}
        if self.area_shape_member(object,property){return true;}
        if self.area_download_member(object,property){return true;}
        if self.area_type_member(object,property){return true;}
        if self.area_hreflang_member(object,property){return true;}
        if self.area_target_member(object,property){return true;}
        if self.area_rel_member(object,property){return true;}
        if self.area_href_member(object,property){return true;}
        if self.area_url_member(object,property){return true;}
        if self.area_referrer_policy_member(object,property){return true;}
        if self.area_ping_member(object,property){return true;}
        if self.map_name_member(object,property){return true;}
        if self.param_name_member(object,property){return true;}
        if self.param_metadata_member(object,property){return true;}
        if self.mod_datetime_member(object,property){return true;}
        if self.data_value_member(object,property){return true;}
        if self.heading_align_member(object,property){return true;}
        if self.ul_type_member(object,property){return true;}
        if self.li_type_member(object,property){return true;}
        if self.ol_type_member(object,property){return true;}
        if self.input_autocomplete_member(object,property){return true;}
        if self.input_form_action_member(object,property){return true;}
        if self.input_form_method_member(object,property){return true;}
        if self.input_form_enctype_member(object,property){return true;}
        if self.button_form_action_member(object,property){return true;}
        if self.button_form_method_member(object,property){return true;}
        if self.button_form_enctype_member(object,property){return true;}
        if self.form_autocomplete_member(object,property){return true;}
        if self.form_method_member(object,property){return true;}
        if self.form_action_member(object,property){return true;}
        if self.form_enctype_member(object,property){return true;}
        if self.form_name_member(object,property){return true;}
        if self.button_name_member(object,property){return true;}
        if self.input_name_member(object,property){return true;}
        if self.form_accept_charset_member(object,property){return true;}
        if self.form_rel_member(object,property){return true;}
        if self.form_target_member(object,property){return true;}
        if self.button_form_target_member(object,property){return true;}
        if self.input_form_target_member(object,property){return true;}
        if self.form_no_validate_member(object,property){return true;}
        if self.button_form_no_validate_member(object,property){return true;}
        if self.input_form_no_validate_member(object,property){return true;}
        if self.select_autocomplete_member(object,property){return true;}
        if self.textarea_autocomplete_member(object,property){return true;}
        if self.textarea_wrap_member(object,property){return true;}
        if self.button_value_member(object,property){return true;}
        if self.button_type_member(object,property){return true;}
        if self.input_step_member(object,property){return true;}
        if self.input_max_member(object,property){return true;}
        if self.input_min_member(object,property){return true;}
        if self.input_pattern_member(object,property){return true;}
        if self.li_value_member(object,property){return true;}
        if self.pre_width_member(object,property){return true;}
        if self.textarea_dimensions_member(object,property){return true;}
        if self.ol_start_member(object,property){return true;}
        if self.textarea_min_length_member(object,property){return true;}
        if self.input_min_length_member(object,property){return true;}
        if self.textarea_max_length_member(object,property){return true;}
        if self.input_max_length_member(object,property){return true;}
        if self.ul_compact_member(object,property){return true;}
        if self.dir_compact_member(object,property){return true;}
        if self.menu_compact_member(object,property){return true;}
        if self.ol_compact_member(object,property){return true;}
        if self.hr_no_shade_member(object,property){return true;}
        if self.area_no_href_member(object,property){return true;}
        if self.option_disabled_member(object,property){return true;}
        if self.select_disabled_member(object,property){return true;}
        if self.button_disabled_member(object,property){return true;}
        if self.input_disabled_member(object,property){return true;}
        if self.textarea_disabled_member(object,property){return true;}
        if self.textarea_required_member(object,property){return true;}
        if self.input_required_member(object,property){return true;}
        if self.textarea_read_only_member(object,property){return true;}
        if self.input_read_only_member(object,property){return true;}
        if self.select_required_member(object,property){return true;}
        if self.option_default_selected_member(object,property){return true;}
        if self.optgroup_disabled_member(object,property){return true;}
        if self.ol_reversed_member(object,property){return true;}
        if self.dl_compact_member(object,property){return true;}
        if self.paragraph_align_member(object,property){return true;}
        if self.hr_align_member(object,property){return true;}
        if self.hr_color_member(object,property){return true;}
        if self.font_size_member(object,property){return true;}
        if self.font_face_member(object,property){return true;}
        if self.font_color_member(object,property){return true;}
        if self.br_clear_member(object,property){return true;}
        if self.hr_size_member(object,property){return true;}
        if self.hr_width_member(object,property){return true;}
        if self.legend_align_member(object,property){return true;}
        if self.div_align_member(object,property){return true;}
        if self.fieldset_member(object,property){return true;}
        if self.control_form_member(object,property){return true;}
        if self.output_metadata_member(object,property){return true;}
        if self.control_custom_validity_member(object,property)||self.control_validation_member(object,property){return true;}
        if self.control_eligibility_member(object,property){return true;}
        if self.fieldset_validation_member(object,property){return true;}
        if self.output_validation_member(object,property){return true;}
        if self.output_validity_member(object,property)||self.validity_member(object,property){return true;}
        if self.output_value_member(object,property){return true;}
        if self.html_label_control_member(object,property){return true;}
        if self.html_label_for_member(object,property){return true;}
        if self.element_class_list_member(object,property){return true;}
        if self.output_html_for_member(object,property){return true;}
        if self.area_rel_list_member(object,property){return true;}
        if self.form_rel_list_member(object,property){return true;}
        if self.html_reflection_member(object,property){return true;}
        if self.xhr_index(object).is_some(){return self.xhr_member(object,property);}

        if script_element_properties::ordinary_interface_member(property) && self.form_named_control(object,property) {return false;}
        self.placeholder_member(object,property) || self.ordinary_attribute_method(object,property)
            || parent_children::member(property) && self.parent_node_receiver(object)
            || property == "style" && self.html_element_expando_receiver(object)
            || property == "innerHTML" && (self.attr_element_receiver(object) || self.template_receiver(object))
            || property == "textContent" && self.text_content_receiver(object)
            || NODE_TRAVERSAL_GETTERS.iter().any(|&(name, _)| property == name)
                && self.node_traversal_receiver(object)
            || property == "target" && self.processing_instruction_receiver(object)
            || property == "content" && self.template_receiver(object)
            || property == "name" && self.html_slot_receiver(object)
            || self.content_editable_member(object, property)
            || self.element_sibling_member(object, property)
            || self.document_inherited_property(object,property)
            || self.assigned_slot_lookup(object, property)
            || self.html_offset_member(object, property)
            || self.scroll_offset_member(object, property)
            || self.input_accessor_member(object, property)
            || self.textarea_value_member(object, property)
            || self.iframe_sandbox_member(object, property)
            || self.canvas_member(object, property)
            || property == "getContext" && self.canvas_receiver(object)
            || self.text_prototype_property(object, property)
            || script_element_properties::ordinary_interface_member(property)
                && self.concrete_html_interface(object).is_some()
    }
}

impl Host for Bridge<'_> {
    fn named_units_property(&mut self,object:u32,units:&[u16])->Result<Option<Value>,String>{
        if self.concrete_html_interface(object)!=Some("HTMLFormElement"){return Ok(None);}
        self.native_node_access(object).map_err(|(_,message)|message)?;
        if self.is_frame_handle(object){return Err("Initial child FORM raw named lookup requires lifecycle integration.".into());}
        script_text_content::deadline(self.state)?;let node=self.node(object)?;
        let value=self.named_form_property_dom(node,crate::dom_string::DomStringRef::Raw(units))?;
        script_text_content::deadline(self.state)?;Ok(Some(value))
    }
    fn native_leaf_getter(&self,object:u32,method:&str)->bool {
        self.event_target_interface(object) && (matches!(method,"\0Node.get.nodeType"|"\0Node.get.nodeName")
            || node_traversal_property(method).is_some() && method!="\0Node.get.baseURI")
    }
    fn console_message_limit(&self,object:u32,method:&str)->Option<usize>{
        (object==CONSOLE&&matches!(method,"log"|"info"|"warn"|"error")).then_some(MAX_LOG_LINE)
    }
    fn print_console(&mut self,object:u32,method:&str,message:&str)->Result<Value,String>{
        if self.console_message_limit(object,method).is_none(){return Err("Native console receiver or operation is invalid.".into());}
        self.state.copies(message.len())?;
        self.state.log(&format!("console.{method}: {message}"));
        Ok(Value::Undefined)
    }
    fn prepare_request_call(&mut self,object:u32,method:&str,args:&[Value],binary:Option<Vec<u8>>)->Result<crate::javascript::xhr_interface::Call,crate::javascript::xhr_interface::Error>{self.xhr_call(object,method,args,binary)}
    fn begin_request_notification(&mut self,notification:crate::javascript::xhr_interface::Notification)->Result<Option<usize>,String>{self.xhr_notification(notification)}
    fn finish_request_abort(&mut self,target:u32,generation:u64){self.xhr_finish_abort(target,generation);}
    fn cancel_request_work(&mut self){self.xhr_cancel_all();}
    fn prepare_range_operation(&mut self,receiver:u32,method:&str,args:&[Value])->Result<(),crate::javascript::range_interface::Error> { self.validate_range_call(receiver,method,args) }
    fn tree_walker_document(&self, document: u32) -> bool {
        document == DOCUMENT || self.inert_node_type(document) == Some(9)
    }

    fn tree_walker_node_type(&self, node: u32) -> Option<u32> {
        if node == DOCUMENT { return Some(9); }
        if self.attr_receiver(node) { return Some(2); }
        if let Some(kind) = self.inert_node_type(node) { return Some(kind); }
        let index = node_index(node).filter(|&index| self.dom_accessible(index))?;
        Some(match self.doc.nodes[index].kind {
            NodeKind::Element { .. } => 1,
            NodeKind::Text(_) => 3,
            NodeKind::Cdata(_)=>4,
            NodeKind::Comment(_) => 8,
            NodeKind::ProcessingInstruction { .. } => 7,
            NodeKind::DocumentFragment => 11,
            NodeKind::DocumentType(_)=>10,
        })
    }
    fn mutation_observation_target(&self, target: u32) -> Option<bool> {
        if let Some(kind)=self.inert_node_type(target){return Some(matches!(kind,3|4|7|8));}
        self.text_content_receiver(target).then(|| node_index(target).is_some_and(|node|
            self.dom_accessible(node) && self.doc.nodes[node].kind.character_data().is_some()))
    }

    fn character_data_write_generation(&self, target: u32, property: &str) -> Option<u64> {
        if matches!(property, "data" | "nodeValue" | "textContent") && self.mutation_observation_target(target) == Some(true) {
            self.state.dom_commits
        } else {
            None
        }
    }

    fn mutation_child_list_target(&self, target: u32) -> bool {
        self.inert_node_type(target).is_some() || target == DOCUMENT || node_index(target).is_some_and(|node| self.dom_accessible(node))
    }

    fn mutation_observation_parent(&self, target: u32) -> Option<u32> {
        if self.inert_node_type(target).is_some(){return self.inert_observation_parent(target);}
        let node = node_index(target).filter(|&node| self.dom_accessible(node))?;
        let parent = self.state.tree.parents.get(node).copied().flatten()?;
        Some(if parent == 0 { DOCUMENT } else { node_handle(parent) })
    }

    fn mutation_tree_generation(&self) -> Option<u64> { self.state.dom_commits }

    fn mutation_child_list_changes(&mut self, target: u32, method: &str, args: &[Value])
        -> Result<Vec<crate::javascript::NativeChildListChange>, String>
    {
        self.child_list_change_plan(target, method, args)
    }

    fn mutation_content_target(&mut self,target:u32,property:&str)->Result<Option<u32>,String>{self.content_observation_target(target,property)}
    fn mutation_content_changes(&mut self,target:u32,property:&str,value:&Value)->Result<Vec<crate::javascript::NativeChildListChange>,String>{self.content_observation_changes(target,property,value)}

    fn mutation_attribute_setter_change(&mut self,target:u32,property:&str)->Result<Option<(u32,crate::javascript::NativeAttributeChange)>,String>{self.attribute_setter_change(target,property)}
    fn mutation_attribute_operation_change(&mut self,target:u32,method:&str)->Result<Option<(u32,crate::javascript::NativeAttributeChange)>,String>{let method=script_class_list::operation(method).unwrap_or(method);if matches!(method,"add"|"remove"|"toggle"|"replace"){self.token_attribute_change(target)}else if matches!(method,"setProperty"|"removeProperty"){self.style_attribute_change(target)}else{Ok(None)}}

    fn mutation_attribute_change(&mut self, target: u32, method: &str, args: &[Value])
        -> Result<Option<crate::javascript::NativeAttributeChange>, String>
    {
        self.attribute_observation_plan(target, method, args)
    }

    fn observed_attribute_setter(&self, target: u32, property: &str) -> Option<&'static str> {
        if property=="id"&&self.canonical_selector_receiver(target){return Some("id");}
        if property=="className"&&self.canonical_selector_receiver(target){return Some("class");}
        if self.html_label_for_member(target,property){return Some("for");}
        if self.fieldset_disabled_member(target,property){return Some("disabled");}
        if let Some(name)=self.meter_numeric_attribute(target,property){return Some(name);}
        if let Some(name)=self.progress_numeric_attribute(target,property){return Some(name);}
        if self.time_datetime_member(target,property){return Some("datetime");}
        if self.area_alt_member(target,property){return Some("alt");}
        if self.optgroup_label_member(target,property){return Some("label");}
        if self.option_label_member(target,property){return Some("label");}
        if self.option_text_member(target,property){return None;}
        if self.textarea_default_value_member(target,property){return None;}
        if self.option_value_member(target,property){return Some("value");}
        if self.select_name_member(target,property){return Some("name");}
        if self.textarea_name_member(target,property){return Some("name");}
        if self.area_coords_member(target,property){return Some("coords");}
        if self.area_shape_member(target,property){return Some("shape");}
        if self.area_download_member(target,property){return Some("download");}
        if self.area_type_member(target,property){return Some("type");}
        if self.area_hreflang_member(target,property){return Some("hreflang");}
        if self.area_target_member(target,property){return Some("target");}
        if self.area_rel_member(target,property){return Some("rel");}
        if self.area_href_member(target,property){return Some("href");}
        if property!="origin"&&self.area_url_member(target,property){return Some("href");}
        if self.area_referrer_policy_member(target,property){return Some("referrerpolicy");}
        if self.area_ping_member(target,property){return Some("ping");}
        if self.map_name_member(target,property){return Some("name");}
        if self.param_name_member(target,property){return Some("name");}
        if self.param_metadata_member(target,property){return crate::javascript::param_metadata_interface::attribute(property);}
        if self.mod_datetime_member(target,property){return Some("datetime");}
        if self.data_value_member(target,property){return Some("value");}
        if self.heading_align_member(target,property){return Some("align");}
        if self.ul_type_member(target,property){return Some("type");}
        if self.li_type_member(target,property){return Some("type");}
        if self.ol_type_member(target,property){return Some("type");}
        if self.input_autocomplete_member(target,property){return Some("autocomplete");}
        if self.input_form_action_member(target,property){return Some("formAction");}
        if self.input_form_method_member(target,property){return Some("formMethod");}
        if self.input_form_enctype_member(target,property){return Some("formEnctype");}
        if self.button_form_action_member(target,property){return Some("formAction");}
        if self.button_form_method_member(target,property){return Some("formMethod");}
        if self.button_form_enctype_member(target,property){return Some("formEnctype");}
        if property=="autocomplete"&&self.form_autocomplete_receiver(target){return Some("autocomplete");}
        if property=="method"&&self.form_method_receiver(target){return Some("method");}
        if property=="action"&&self.form_action_receiver(target){return Some("action");}
        if matches!(property,"enctype"|"encoding")&&self.form_enctype_receiver(target){return Some("enctype");}
        if property=="name"&&self.form_name_receiver(target){return Some("name");}
        if property=="name"&&self.button_name_receiver(target){return Some("name");}
        if property=="name"&&self.input_name_receiver(target){return Some("name");}
        if property=="acceptCharset"&&self.form_accept_charset_receiver(target){return Some("accept-charset");}
        if property=="rel"&&self.form_rel_receiver(target){return Some("rel");}
        if property=="target"&&self.form_target_receiver(target){return Some("target");}
        if property=="formTarget"&&self.button_form_target_receiver(target){return Some("formtarget");}
        if property=="formTarget"&&self.input_form_target_receiver(target){return Some("formtarget");}
        if property=="noValidate"&&self.form_no_validate_receiver(target){return Some("novalidate");}
        if property=="formNoValidate"&&self.button_form_no_validate_receiver(target){return Some("formnovalidate");}
        if property=="formNoValidate"&&self.input_form_no_validate_receiver(target){return Some("formnovalidate");}
        if self.select_autocomplete_member(target,property){return Some("autocomplete");}
        if self.textarea_autocomplete_member(target,property){return Some("autocomplete");}
        if self.textarea_wrap_member(target,property){return Some("wrap");}
        if self.button_value_member(target,property){return Some("value");}
        if self.button_type_member(target,property){return Some("type");}
        if self.input_step_member(target,property){return Some("step");}
        if self.input_max_member(target,property){return Some("max");}
        if self.input_min_member(target,property){return Some("min");}
        if self.input_pattern_member(target,property){return Some("pattern");}
        if self.li_value_member(target,property){return Some("value");}
        if self.pre_width_member(target,property){return Some("width");}
        if self.textarea_dimensions_member(target,property){return Some(if property=="cols"{"cols"}else{"rows"});}
        if self.ol_start_member(target,property){return Some("start");}
        if self.textarea_min_length_member(target,property){return Some("minlength");}
        if self.input_min_length_member(target,property){return Some("minlength");}
        if self.textarea_max_length_member(target,property){return Some("maxlength");}
        if self.input_max_length_member(target,property){return Some("maxlength");}
        if self.ul_compact_member(target,property){return Some("compact");}
        if self.dir_compact_member(target,property){return Some("compact");}
        if self.menu_compact_member(target,property){return Some("compact");}
        if self.ol_compact_member(target,property){return Some("compact");}
        if self.hr_no_shade_member(target,property){return Some("noshade");}
        if self.area_no_href_member(target,property){return Some("nohref");}
        if self.option_disabled_member(target,property){return Some("disabled");}
        if self.select_disabled_member(target,property){return Some("disabled");}
        if self.button_disabled_member(target,property){return Some("disabled");}
        if self.input_disabled_member(target,property){return Some("disabled");}
        if self.textarea_disabled_member(target,property){return Some("disabled");}
        if self.textarea_required_member(target,property){return Some("required");}
        if self.input_required_member(target,property){return Some("required");}
        if self.textarea_read_only_member(target,property){return Some("readonly");}
        if self.input_read_only_member(target,property){return Some("readonly");}
        if self.select_required_member(target,property){return Some("required");}
        if self.option_default_selected_member(target,property){return Some("selected");}
        if self.optgroup_disabled_member(target,property){return Some("disabled");}
        if self.ol_reversed_member(target,property){return Some("reversed");}
        if self.dl_compact_member(target,property){return Some("compact");}
        if self.paragraph_align_member(target,property){return Some("align");}
        if self.hr_align_member(target,property){return Some("align");}
        if self.hr_color_member(target,property){return Some("color");}
        if self.font_size_member(target,property){return Some("size");}
        if self.font_face_member(target,property){return Some("face");}
        if self.font_color_member(target,property){return Some("color");}
        if self.br_clear_member(target,property){return Some("clear");}
        if self.hr_size_member(target,property){return Some("size");}
        if self.hr_width_member(target,property){return Some("width");}
        if self.legend_align_member(target,property){return Some("align");}
        if self.div_align_member(target,property){return Some("align");}
        if self.fieldset_name_member(target,property){return Some("name");}
        if self.output_name_member(target,property){return Some("name");}
        if property!="origin" && script_anchor_url::property(property) && self.native_anchor(target) {return Some("href");}
        if self.charset_member(target, property) { return Some("charset"); }
        if self.meta_member(target, property) { return meta_attribute(property); }
        if self.aria_reflection_attribute(target,property){return crate::javascript::aria_reflection_interface::property(property);}
        if self.html_reflection_receiver(target){if let Some(name)=crate::javascript::html_reflection_interface::attribute(property){return Some(name);}}
        if self.placeholder_member(target,property){return Some("placeholder");}
        (property == "name" && self.html_slot_receiver(target)).then_some("name")
    }

    fn mutation_node_list(&mut self, nodes: &[u32]) -> Result<u32, String> {
        self.snapshot_mutation_node_list(nodes)
    }

    fn mutation_callback_error(&mut self, message: &str) {
        self.state.error("MutationObserver callback", message);
    }
    fn native_descriptor_change_allowed(&self,object:u32,property:&str)->bool{
        !(self.control_form_member(object,property)&&node_index(object).is_some_and(|node|self.forms.controls.contains_key(&node)))
    }
    fn expando_owner(&self,object:u32)->Option<u32> {
        if self.validity_receiver(object){return Some(WINDOW);}
        if self.class_list_receiver(object){return Some(WINDOW);}
        if self.attr_receiver(object){return Some(WINDOW);}
        if self.character_data_author_receiver(object) {return Some(WINDOW);}
        if self.inert_node_type(object)==Some(11) {return Some(WINDOW);}
        if self.inert_node_type(object)==Some(1){return Some(WINDOW);}
        if self.named_map_receiver(object){return Some(WINDOW);}
        if let Some(window)=self.collection_owner_window(object){return Some(window);}
        if self.inert_adopted_node(object) && self.inert_interface(object)
            .is_some_and(|interface| interface.starts_with("HTML") || interface == "SVGElement")
        {
            return Some(WINDOW);
        }
        if self.message_instance(object).is_some() { return Some(WINDOW); }
        self.range_owner(object).or_else(||self.ordinary_event_owner(object)).or_else(||self.abort_instance(object).map(|(owner, _)| owner)).or_else(||self.standalone_target_owner(object)).or_else(||self.node_list_owner(object)).or_else(||(object==DOCUMENT||self.element_expando_receiver(object)||self.fragment_receiver(object)||self.collection_receiver(object)||self.canvas_context_receiver(object)).then_some(WINDOW))}
    fn ordinary_native_fallback(&self,object:u32,property:&str)->bool {
        if self.canonical_attribute_node_member(object,property){return false;}
        if self.canonical_attr_member(object,property){return false;}
        if self.canonical_attribute_factory_member(object,property){return false;}
        if self.canonical_collection_member(object,property)||self.canonical_name_list_member(object,property){return false;}
        self.inert_adopted_node(object) && self.inert_owns_property(object, property)
            || matches!(self.inert_node_type(object),Some(1|9))&&matches!(property,"getElementsByTagName"|"getElementsByTagNameNS"|"getElementsByClassName")
            || object==DOCUMENT && (matches!(property,"constructor"|"__proto__") || !self.document_native_property(property))
    }
    fn exposed_native_prototype(&self,object:u32)->bool {
        if self.validity_receiver(object){return true;}
        self.attr_receiver(object) || self.range_owner(object).is_some() || self.event_target_interface(object) || self.inert_interface(object).is_some() || self.document_interface_receiver(object)
            || self.ordinary_event_owner(object).is_some() || self.standalone_target_owner(object).is_some()
            || self.abort_instance(object).is_some() || self.message_instance(object).is_some()
            || self.canvas_context_receiver(object) || self.class_list_receiver(object)
            || self.video_interface_receiver(object) || self.fragment_receiver(object)
            || self.node_list_receiver(object) || self.collection_receiver(object) || self.named_map_receiver(object) || self.concrete_html_interface(object).is_some()
            || self.style_interface_receiver(object, true)
            || self.slottable(object) == Some(Slottable::Text)
            || self.comment_receiver(object)
            || self.processing_instruction_receiver(object) || self.doctype_receiver(object)
    }
    fn mutable_native_wrapper_prototype(&self, object: u32) -> bool {
        self.fragment_receiver(object) || self.inert_node_type(object)==Some(11)
            || self.element_expando_receiver(object) && self.concrete_html_interface(object) == Some("HTMLElement")
            || self.ordinary_event_owner(object).is_some()
    }
    fn native_ancestry_inspection(&self,object:u32)->bool {self.event_receiver(object)||self.exposed_native_prototype(object)||self.namespace_receiver(object)}
    fn ordinary_inherited_enumeration(&self, object: u32) -> bool {
        if self.document_interface_receiver(object) {return false;}
        // Container prototypes identify the objects, but their enumerable
        // native attributes still live outside the ordinary descriptor store.
        if let Some(interface) = self.concrete_html_interface(object) {
            return matches!(interface, "HTMLLegendElement" | "HTMLFieldSetElement" | "HTMLMeterElement" | "HTMLOutputElement" | "HTMLProgressElement" | "HTMLSelectElement" | "HTMLLabelElement" | "HTMLButtonElement" | "HTMLUListElement" | "HTMLInputElement" | "HTMLCanvasElement" | "HTMLTextAreaElement");
        }
        self.exposed_native_prototype(object)
    }
    fn event_target_interface(&self,window:u32)->bool {window==WINDOW || self.frame_context(window).is_some()}
    fn node_interface_constants(&self,window:u32)->Option<&'static [(&'static str,u16)]> {self.event_target_interface(window).then_some(dom_tree::NODE_CONSTANTS)}
    fn instance_ancestry(&self, object:u32) -> Option<crate::javascript::NativeAncestry> {
        if self.validity_receiver(object){return Some(crate::javascript::NativeAncestry{window:WINDOW,constructor:Some((WINDOW,"ValidityState"))});}
        if self.class_list_receiver(object){return Some(crate::javascript::NativeAncestry{window:WINDOW,constructor:Some((WINDOW,"DOMTokenList"))});}
        if self.attr_receiver(object){return Some(crate::javascript::NativeAncestry{window:WINDOW,constructor:Some((WINDOW,"Attr"))});}
        if let Some((window,interface))=self.xhr_interface(object){return Some(crate::javascript::NativeAncestry {window,constructor:Some((window,interface))});}

        if let Some(record)=self.range_record(object) { let window=record.owner;return Some(crate::javascript::NativeAncestry {window,constructor:Some((window,if record.fixed {"StaticRange"}else{"Range"}))}); }
        if self.event_target_interface(object) {
            return Some(crate::javascript::NativeAncestry { window: object, constructor: Some((object, "Window")) });
        }
        if let Some(interface) = self.inert_interface(object) {
            return Some(crate::javascript::NativeAncestry { window: WINDOW, constructor: Some((WINDOW, interface)) });
        }
        if self.canvas_context_receiver(object) {
            return Some(crate::javascript::NativeAncestry { window: WINDOW, constructor: Some((WINDOW, "CanvasRenderingContext2D")) });
        }
        if let Some(window)=self.computed_style_owner(object) {return Some(crate::javascript::NativeAncestry {window,constructor:None});}
        if self.named_map_receiver(object){return Some(crate::javascript::NativeAncestry{window:WINDOW,constructor:Some((WINDOW,"NamedNodeMap"))});}
        if let Some(window)=self.collection_owner_window(object){return Some(crate::javascript::NativeAncestry{window,constructor:Some((window,"HTMLCollection"))});}
        if let Some(window) = self.node_list_owner(object) { return Some(crate::javascript::NativeAncestry { window, constructor:Some((window,"NodeList")) }); }
        if let Some(window)=self.event_owner(object) {
            // Subclass immediate prototypes stay opaque; this is their genuine
            // implemented Event ancestry, as with the existing Node suffixes.
            let interface = if self.message_event_receiver(object) {"MessageEvent"}
                else if self.custom_receiver(object) {"CustomEvent"}
                else if self.wheel_receiver(object) {"WheelEvent"}
                else if self.mouse_receiver(object) {"MouseEvent"}
                else if self.keyboard_receiver(object) {"KeyboardEvent"}
                else if self.focus_receiver(object) {"FocusEvent"}
                else if self.submit_event_receiver(object) {"SubmitEvent"}
                else if self.error_event_receiver(object) {"ErrorEvent"}
                else if self.animation_receiver(object) {"AnimationEvent"}
                else if self.progress_receiver(object) {"ProgressEvent"}
                else if self.ui_receiver(object) {"UIEvent"}
                else {"Event"};
            return Some(crate::javascript::NativeAncestry {window,constructor:Some((window,interface))});
        }
        if let Some(interface) = self.message_instance(object) {
            return Some(crate::javascript::NativeAncestry {window:WINDOW,constructor:Some((WINDOW,interface))});
        }
        if self.media_query_owner(object).is_some() {
            return Some(crate::javascript::NativeAncestry {window:WINDOW,constructor:Some((WINDOW,MEDIA_QUERY_LIST))});
        }
        if let Some((window, interface)) = self.abort_instance(object) {
            return Some(crate::javascript::NativeAncestry { window, constructor: Some((window, interface)) });
        }
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
                    if let Some(interface) = self.concrete_html_interface(object) {(WINDOW, interface)}
                    else if self.video_interface_receiver(object) {(WINDOW,"HTMLVideoElement")}
                    else if self.media_interface_receiver(object) {(WINDOW,"HTMLMediaElement")}
                    else if tag(self.doc,node)=="img" {(IMAGE_FACTORY,"Image")} else {(WINDOW,"HTMLElement")}
                } else if self.doc.nodes[node].namespace==crate::html::Namespace::Svg {
                    (WINDOW,"SVGElement")
                } else {(WINDOW,"Element")}
            } else if self.fragment_receiver(object) {(WINDOW,"DocumentFragment")}
            else if matches!(self.doc.nodes[node].kind,NodeKind::Text(_)) {(WINDOW,"Text")}
            else if matches!(self.doc.nodes[node].kind,NodeKind::Cdata(_)){(WINDOW,"CDATASection")}
            else if matches!(self.doc.nodes[node].kind,NodeKind::Comment(_)) {(WINDOW,"Comment")}
            else if matches!(self.doc.nodes[node].kind, NodeKind::ProcessingInstruction { .. }) {(WINDOW,"ProcessingInstruction")}
            else if matches!(self.doc.nodes[node].kind,NodeKind::DocumentType(_)){(WINDOW,"DocumentType")}
            else {(WINDOW,"Node")})
        } else if object==DOCUMENT {Some((WINDOW,"Document"))}
        else if self.attr_receiver(object) {Some((WINDOW,"Node"))} else {None};
        let known=node.is_some() || matches!(object,DOCUMENT|CONSOLE|NAVIGATOR|PLUGINS|MIME_TYPES|DOCUMENT_IMAGES|LOCATION|PERFORMANCE|PERFORMANCE_TIMING|HISTORY)
            || self.attr_receiver(object) || self.event_receiver(object) || self.class_list_receiver(object) || self.named_map_receiver(object)
            || node_list_index(object).is_some_and(|index|index<self.state.node_lists.len())
            || collection_handle_index(object).is_some_and(|index|index<self.state.dom.collections.len())
            || (STYLE_BASE..EVENT_BASE).contains(&object) && self.attr_element_receiver(node_handle(((object-STYLE_BASE)%NODE_HANDLES) as usize));
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
        if self.area_href_receiver(object)||self.native_anchor(object){return true;}
        if self.validity_receiver(object){return true;}
        if self.form_receiver(object) && self.class_list_element_receiver(object) {return true;}
        self.range_owner(object).is_some() || object == LOCATION || self.class_list_receiver(object) || self.standalone_target_owner(object).is_some()
            || self.abort_instance(object).is_some()
    }

    fn owns_property(&self, object: u32, property: &str) -> bool {
        if self.class_list_receiver(object){return collection_index(property).is_some();}
        if self.canonical_attribute_node_member(object,property){return false;}
        if self.canonical_attr_member(object,property){return false;}
        if self.canonical_attribute_factory_member(object,property){return false;}
        if self.canonical_collection_member(object,property)||self.canonical_name_list_member(object,property){return false;}
        if property=="baseURI" && self.form_named_control(object,property) {return true;}
        if self.node_metadata_property(object,property) {return false;}
        if self.charset_member(object, property) { return false; }
        if self.meta_member(object, property) { return false; }
        if self.aria_reflection_member(object,property){return false;}
        if self.element_id_member(object,property){return false;}
        if self.element_class_name_member(object,property){return false;}
        if self.control_labels_member(object,property){return false;}
        if self.html_label_form_member(object,property){return false;}
        if self.output_form_member(object,property){return false;}
        if self.control_type_member(object,property){return false;}
        if self.form_length_member(object,property){return false;}
        if self.meter_numeric_member(object,property){return false;}
        if self.progress_numeric_member(object,property){return false;}
        if self.legend_form_member(object,property){return false;}
        if self.option_form_member(object,property){return false;}
        if self.option_index_member(object,property){return false;}
        if self.textarea_text_length_member(object,property){return false;}
        if self.map_areas_member(object,property){return false;}
        if self.datalist_options_member(object,property){return false;}
        if self.time_datetime_member(object,property){return false;}
        if self.area_alt_member(object,property){return false;}
        if self.optgroup_label_member(object,property){return false;}
        if self.option_label_member(object,property){return false;}
        if self.option_text_member(object,property){return false;}
        if self.textarea_default_value_member(object,property){return false;}
        if self.option_value_member(object,property){return false;}
        if self.select_name_member(object,property){return false;}
        if self.textarea_name_member(object,property){return false;}
        if self.area_coords_member(object,property){return false;}
        if self.area_shape_member(object,property){return false;}
        if self.area_download_member(object,property){return false;}
        if self.area_type_member(object,property){return false;}
        if self.area_hreflang_member(object,property){return false;}
        if self.area_target_member(object,property){return false;}
        if self.area_rel_member(object,property){return false;}
        if self.area_href_member(object,property){return false;}
        if self.area_url_member(object,property){return false;}
        if self.area_referrer_policy_member(object,property){return false;}
        if self.area_ping_member(object,property){return false;}
        if self.map_name_member(object,property){return false;}
        if self.param_name_member(object,property){return false;}
        if self.param_metadata_member(object,property){return false;}
        if self.mod_datetime_member(object,property){return false;}
        if self.data_value_member(object,property){return false;}
        if self.heading_align_member(object,property){return false;}
        if self.ul_type_member(object,property){return false;}
        if self.li_type_member(object,property){return false;}
        if self.ol_type_member(object,property){return false;}
        if self.input_autocomplete_member(object,property){return false;}
        if self.input_form_action_member(object,property){return false;}
        if self.input_form_method_member(object,property){return false;}
        if self.input_form_enctype_member(object,property){return false;}
        if self.button_form_action_member(object,property){return false;}
        if self.button_form_method_member(object,property){return false;}
        if self.button_form_enctype_member(object,property){return false;}
        if self.form_autocomplete_member(object,property){return false;}
        if self.form_method_member(object,property){return false;}
        if self.form_action_member(object,property){return false;}
        if self.form_enctype_member(object,property){return false;}
        if self.form_name_member(object,property){return false;}
        if self.button_name_member(object,property){return false;}
        if self.input_name_member(object,property){return false;}
        if self.form_accept_charset_member(object,property){return false;}
        if self.form_rel_member(object,property){return false;}
        if self.form_target_member(object,property){return false;}
        if self.button_form_target_member(object,property){return false;}
        if self.input_form_target_member(object,property){return false;}
        if self.form_no_validate_member(object,property){return false;}
        if self.button_form_no_validate_member(object,property){return false;}
        if self.input_form_no_validate_member(object,property){return false;}
        if self.select_autocomplete_member(object,property){return false;}
        if self.textarea_autocomplete_member(object,property){return false;}
        if self.textarea_wrap_member(object,property){return false;}
        if self.button_value_member(object,property){return false;}
        if self.button_type_member(object,property){return false;}
        if self.input_step_member(object,property){return false;}
        if self.input_max_member(object,property){return false;}
        if self.input_min_member(object,property){return false;}
        if self.input_pattern_member(object,property){return false;}
        if self.li_value_member(object,property){return false;}
        if self.pre_width_member(object,property){return false;}
        if self.textarea_dimensions_member(object,property){return false;}
        if self.ol_start_member(object,property){return false;}
        if self.textarea_min_length_member(object,property){return false;}
        if self.input_min_length_member(object,property){return false;}
        if self.textarea_max_length_member(object,property){return false;}
        if self.input_max_length_member(object,property){return false;}
        if self.ul_compact_member(object,property){return false;}
        if self.dir_compact_member(object,property){return false;}
        if self.menu_compact_member(object,property){return false;}
        if self.ol_compact_member(object,property){return false;}
        if self.hr_no_shade_member(object,property){return false;}
        if self.area_no_href_member(object,property){return false;}
        if self.option_disabled_member(object,property){return false;}
        if self.select_disabled_member(object,property){return false;}
        if self.button_disabled_member(object,property){return false;}
        if self.input_disabled_member(object,property){return false;}
        if self.textarea_disabled_member(object,property){return false;}
        if self.textarea_required_member(object,property){return false;}
        if self.input_required_member(object,property){return false;}
        if self.textarea_read_only_member(object,property){return false;}
        if self.input_read_only_member(object,property){return false;}
        if self.select_required_member(object,property){return false;}
        if self.option_default_selected_member(object,property){return false;}
        if self.optgroup_disabled_member(object,property){return false;}
        if self.ol_reversed_member(object,property){return false;}
        if self.dl_compact_member(object,property){return false;}
        if self.paragraph_align_member(object,property){return false;}
        if self.hr_align_member(object,property){return false;}
        if self.hr_color_member(object,property){return false;}
        if self.font_size_member(object,property){return false;}
        if self.font_face_member(object,property){return false;}
        if self.font_color_member(object,property){return false;}
        if self.br_clear_member(object,property){return false;}
        if self.hr_size_member(object,property){return false;}
        if self.hr_width_member(object,property){return false;}
        if self.legend_align_member(object,property){return false;}
        if self.div_align_member(object,property){return false;}
        if self.fieldset_member(object,property){return false;}
        if self.control_form_member(object,property){return false;}
        if self.output_metadata_member(object,property){return false;}
        if self.control_custom_validity_member(object,property)||self.control_validation_member(object,property){return false;}
        if self.control_eligibility_member(object,property){return false;}
        if self.fieldset_validation_member(object,property){return false;}
        if self.output_validation_member(object,property){return false;}
        if self.output_validity_member(object,property)||self.validity_receiver(object){return false;}
        if self.output_value_member(object,property){return false;}
        if self.html_label_control_member(object,property){return false;}
        if self.html_label_for_member(object,property){return false;}
        if self.element_class_list_member(object,property){return false;}
        if self.output_html_for_member(object,property){return false;}
        if self.area_rel_list_member(object,property){return false;}
        if self.form_rel_list_member(object,property){return false;}
        if self.html_reflection_member(object,property){return false;}
        if self.xhr_index(object).is_some(){return false;}

        if self.form_request_member(object,property){return false;}
        if self.control_custom_validity_member(object,property)||self.control_validation_member(object,property)||self.form_validation_member(object,property){return false;}
        if property=="reset" && self.form_length_receiver(object) && !self.form_named_control(object,property) {return false;}
        if property=="submit" && self.form_submit_method(object).is_some() {return false;}
        if self.range_owner(object).is_some() {return false;}
        if self.inert_interface(object).is_some() { return self.inert_owns_property(object, property); }
        if self.canvas_context_receiver(object) { return script_canvas_context::unsupported_state(property); }
        if self.frame_document_prototype_lookup(object,property) {return false;}
        if self.named_map_receiver(object){return !script_named_map::prototype_member(property);}
        if self.collection_receiver(object){return !self.collection_prototype_member(property);}
        if self.node_list_receiver(object) {
            // Only native indices are protected. Methods and length use the
            // ordinary backing/prototype, including author overrides.
            return collection_index(property).is_some();
        }
        if self.element_selector_member(object,property) {return false;}
        if self.inherited_node_property(object,property) {return false;}
        if self.node_tree_method_member(object,property) {return false;}
        if matches!(property,"nodeType"|"nodeName") && self.node_identity_receiver(object) {return false;}
        if self.video_interface_receiver(object)&&script_media::interface_member(property) {return false;}
        if self.event_receiver(object)&&property=="composedPath" {return false;}
        if self.ordinary_event_owner(object).is_some() {return false;}
        if self.node_event_method(object,property) {return false;}
        if self.fragment_receiver(object) {return matches!(property,"childNodes"|"nodeValue"|"textContent"|"parentNode"|"parentElement"|"ownerDocument"|"isConnected"|"firstChild"|"lastChild"|"previousSibling"|"nextSibling") || self.dom_method(object,property).is_some();}
        if self.standalone_target_owner(object).is_some() || self.abort_instance(object).is_some()
            || self.message_instance(object).is_some() {return false;}
        if property=="prototype"&&self.location_instance_owner(object).is_some(){return false;}
        if object==DOCUMENT {return self.document_native_property(property);}
        if self.element_expando_receiver(object) {return self.element_native_property(object,property);}
        if self.attr_receiver(object) { return self.attr_native_member(property); }
        if self.character_data_author_receiver(object) {
            return matches!(property, "data" | "length" | "nodeValue" | "ownerDocument" | "isConnected" | "baseURI")
                || matches!(property, "normalize" | "isDefaultNamespace" | "compareDocumentPosition")
                || dom_tree::node_constant(property).is_some();
        }
        if self.constructed_event_receiver(object) { return self.event_native_member(object,property); }
        object != NAVIGATOR || navigator_readonly(property)
    }

    fn ordinary_lookup_tail(&self, object:u32) -> bool {
        if self.xhr_index(object).is_some(){return true;}

        if self.form_receiver(object) && self.class_list_element_receiver(object) {return true;}
        self.location_instance_owner(object).is_some() || object == HISTORY || self.attr_receiver(object)
            || self.text_or_comment_receiver(object) || self.ordinary_event_owner(object).is_some() || self.named_map_receiver(object)
    }
    fn ordinary_lookup_tail_property(&self,object:u32,property:&str)->bool {
        if self.validity_receiver(object){return true;}
        if self.class_list_receiver(object){return true;}
        if self.inert_interface(object).is_some() { !self.inert_owns_property(object, property) }
        else if self.canvas_context_receiver(object) { !script_canvas_context::unsupported_state(property) }
        else if self.collection_receiver(object) || self.node_list_receiver(object) || self.frame_document_prototype_lookup(object,property) {true}
        else if self.element_selector_member(object,property) {true}
        else if self.inherited_node_property(object,property) {true}
        else if self.node_tree_method_member(object,property) {true}
        else if matches!(property,"nodeType"|"nodeName") && self.node_identity_receiver(object) {true}
        else if self.event_receiver(object)&&property=="composedPath" {true}
        else if self.node_event_method(object,property) {true}
        else if self.location_instance_owner(object).is_some() {property=="prototype"}
        else if property=="animate" && self.html_element_expando_receiver(object) {true}
        else {self.ordinary_lookup_tail(object)}
    }
    fn native_access_owner(&self, object: u32) -> Option<u32> {
        if self.validity_receiver(object){return Some(WINDOW);}
        if self.class_list_receiver(object){return Some(WINDOW);}
        if self.inert_interface(object).is_some() { return Some(WINDOW); }
        if self.message_instance(object).is_some() { return Some(WINDOW); }
        if script_inline_style::handle(object) {return Some(WINDOW);}
        if self.canvas_context_receiver(object) {return Some(WINDOW);}
        if let Some(owner)=self.computed_style_owner(object) {return Some(owner);}
        if let Some(owner)=self.event_owner(object) {return Some(owner);}
        if self.event_target_interface(object) {return Some(object);}
        if let Some(owner)=self.standalone_target_owner(object) {return Some(owner);}
        if let Some((owner, _))=self.abort_instance(object) {return Some(owner);}
        if let Some(window)=self.frame_instance_window(object) {return Some(window);}
        if let Some(window)=self.collection_owner_window(object){return Some(window);}
        if let Some(window) = self.node_list_owner(object) { return Some(window); }
        if self.namespace_receiver(object) {return Some(WINDOW);}
        self.location_instance_owner(object).or_else(||(matches!(object,DOCUMENT|CONSOLE|PERFORMANCE|PERFORMANCE_TIMING|HISTORY) || self.named_map_receiver(object) || self.attr_receiver(object) || self.media_element_receiver(object)).then_some(WINDOW))
    }
    fn bounding_rect_snapshot(&mut self, object: u32) -> Option<Result<[f64; 4], String>> {
        if !self.class_list_element_receiver(object) { return None; }
        Some((|| {
            let node = self.node(object)?;
            if !self.state.dom.connected.get(node).copied().unwrap_or(false) { return Ok([0.0; 4]); }
            self.state.geometry.as_mut().ok_or("Bounding rectangle requires the actual document renderer.")?
                .bounding_rect(self.doc, self.forms, node, &mut self.state.host_work, &mut self.state.copy_work)
        })())
    }
    fn canvas_readback_source(
        &mut self,
        object: u32,
        output_bytes: usize,
    ) -> Result<&crate::canvas_2d_bitmap::CanvasBitmap, String> {
        self.canvas_pixels_for_readback(object, output_bytes)
    }

    fn inherited_assignment(&self, object: u32, property: &str) -> bool {
        if self.form_rel_list_member(object,property){return true;}
        if property=="classList"&&self.canonical_selector_receiver(object){return true;}
        if script_anchor_url::property(property) && self.native_anchor(object) {return true;}
        if property=="text" && self.title_receiver(object) {return true;}
        if property == "nodeValue" && self.text_content_receiver(object) { return !self.form_named_control(object, property); }
        if self.charset_member(object, property) { return true; }
        if self.meta_member(object, property) { return true; }
        if self.aria_reflection_attribute(object,property){return true;}
        if self.placeholder_member(object,property){return true;}
        script_scroll_offsets::axis(property).is_some() && self.element_selector_receiver(object)
            || property == "innerHTML" && (self.attr_element_receiver(object) || self.template_receiver(object))
            || property == "textContent" && self.text_content_receiver(object)
            || property == "content" && self.template_receiver(object)
            || self.content_editable_attribute(object, property)
            || self.html_offset_attribute(object, property)
            || script_element_properties::ordinary_interface_member(property)
            && self.frame_concrete_html_interface(object).is_some()
    }

    fn readonly_property(&self, object: u32, property: &str) -> bool {
        if self.control_labels_member(object,property){return true;}
        if self.html_label_form_member(object,property){return true;}
        if self.output_form_member(object,property){return true;}
        if self.control_type_member(object,property){return true;}
        if self.form_length_member(object,property){return true;}
        if self.progress_numeric_member(object,property){return property=="position";}
        if self.legend_form_member(object,property){return true;}
        if self.option_form_member(object,property){return true;}
        if self.option_index_member(object,property){return true;}
        if self.textarea_text_length_member(object,property){return true;}
        if self.map_areas_member(object,property){return true;}
        if self.datalist_options_member(object,property){return true;}
        if self.time_datetime_member(object,property){return false;}
        if self.area_alt_member(object,property){return false;}
        if self.optgroup_label_member(object,property){return false;}
        if self.option_label_member(object,property){return false;}
        if self.option_text_member(object,property){return false;}
        if self.textarea_default_value_member(object,property){return false;}
        if self.option_value_member(object,property){return false;}
        if self.select_name_member(object,property){return false;}
        if self.textarea_name_member(object,property){return false;}
        if self.area_coords_member(object,property){return false;}
        if self.area_shape_member(object,property){return false;}
        if self.area_download_member(object,property){return false;}
        if self.area_type_member(object,property){return false;}
        if self.area_hreflang_member(object,property){return false;}
        if self.area_target_member(object,property){return false;}
        if self.area_rel_member(object,property){return false;}
        if self.area_href_member(object,property){return false;}
        if self.area_url_member(object,property){return property=="origin";}
        if self.area_referrer_policy_member(object,property){return false;}
        if self.area_ping_member(object,property){return false;}
        if self.map_name_member(object,property){return false;}
        if self.param_name_member(object,property){return false;}
        if self.param_metadata_member(object,property){return false;}
        if self.mod_datetime_member(object,property){return false;}
        if self.data_value_member(object,property){return false;}
        if self.heading_align_member(object,property){return false;}
        if self.ul_type_member(object,property){return false;}
        if self.li_type_member(object,property){return false;}
        if self.ol_type_member(object,property){return false;}
        if self.input_autocomplete_member(object,property){return false;}
        if self.input_form_action_member(object,property){return false;}
        if self.input_form_method_member(object,property){return false;}
        if self.input_form_enctype_member(object,property){return false;}
        if self.button_form_action_member(object,property){return false;}
        if self.button_form_method_member(object,property){return false;}
        if self.button_form_enctype_member(object,property){return false;}
        if self.form_autocomplete_member(object,property){return false;}
        if self.form_method_member(object,property){return false;}
        if self.form_action_member(object,property){return false;}
        if self.form_enctype_member(object,property){return false;}
        if self.form_name_member(object,property){return false;}
        if self.button_name_member(object,property){return false;}
        if self.input_name_member(object,property){return false;}
        if self.form_accept_charset_member(object,property){return false;}
        if self.form_rel_member(object,property){return false;}
        if self.form_target_member(object,property){return false;}
        if self.button_form_target_member(object,property){return false;}
        if self.input_form_target_member(object,property){return false;}
        if self.form_no_validate_member(object,property){return false;}
        if self.button_form_no_validate_member(object,property){return false;}
        if self.input_form_no_validate_member(object,property){return false;}
        if self.select_autocomplete_member(object,property){return false;}
        if self.textarea_autocomplete_member(object,property){return false;}
        if self.textarea_wrap_member(object,property){return false;}
        if self.button_value_member(object,property){return false;}
        if self.button_type_member(object,property){return false;}
        if self.input_step_member(object,property){return false;}
        if self.input_max_member(object,property){return false;}
        if self.input_min_member(object,property){return false;}
        if self.input_pattern_member(object,property){return false;}
        if self.li_value_member(object,property){return false;}
        if self.pre_width_member(object,property){return false;}
        if self.textarea_dimensions_member(object,property){return false;}
        if self.ol_start_member(object,property){return false;}
        if self.textarea_min_length_member(object,property){return false;}
        if self.input_min_length_member(object,property){return false;}
        if self.textarea_max_length_member(object,property){return false;}
        if self.input_max_length_member(object,property){return false;}
        if self.ul_compact_member(object,property){return false;}
        if self.dir_compact_member(object,property){return false;}
        if self.menu_compact_member(object,property){return false;}
        if self.ol_compact_member(object,property){return false;}
        if self.hr_no_shade_member(object,property){return false;}
        if self.area_no_href_member(object,property){return false;}
        if self.option_disabled_member(object,property){return false;}
        if self.select_disabled_member(object,property){return false;}
        if self.button_disabled_member(object,property){return false;}
        if self.input_disabled_member(object,property){return false;}
        if self.textarea_disabled_member(object,property){return false;}
        if self.textarea_required_member(object,property){return false;}
        if self.input_required_member(object,property){return false;}
        if self.textarea_read_only_member(object,property){return false;}
        if self.input_read_only_member(object,property){return false;}
        if self.select_required_member(object,property){return false;}
        if self.option_default_selected_member(object,property){return false;}
        if self.optgroup_disabled_member(object,property){return false;}
        if self.ol_reversed_member(object,property){return false;}
        if self.dl_compact_member(object,property){return false;}
        if self.paragraph_align_member(object,property){return false;}
        if self.hr_align_member(object,property){return false;}
        if self.hr_color_member(object,property){return false;}
        if self.font_size_member(object,property){return false;}
        if self.font_face_member(object,property){return false;}
        if self.font_color_member(object,property){return false;}
        if self.br_clear_member(object,property){return false;}
        if self.hr_size_member(object,property){return false;}
        if self.hr_width_member(object,property){return false;}
        if self.legend_align_member(object,property){return false;}
        if self.div_align_member(object,property){return false;}
        if self.fieldset_member(object,property){return matches!(property,"form"|"type");}
        if self.control_form_member(object,property){return true;}
        if property=="type"&&self.output_metadata_member(object,property){return true;}
        if property=="validationMessage"&&self.control_validation_member(object,property){return true;}
        if self.control_eligibility_member(object,property){return true;}
        if self.fieldset_validation_readonly(object,property){return true;}
        if self.output_validation_readonly(object,property){return true;}
        if property=="validity"&&(self.html_output_receiver(object)||self.fieldset_receiver(object)||self.control_eligibility_class(object).is_some())||self.validity_member(object,property){return true;}
        if self.html_label_control_member(object,property){return true;}
        if self.class_list_receiver(object){return collection_index(property).is_some();}
        if property=="activeElement"&&self.document_interface_receiver(object){return true;}
        if self.doctype_receiver(object)&&matches!(property,"name"|"publicId"|"systemId"){return true;}
        if property=="attributes"&&self.inert_node_type(object)==Some(1){return true;}
        if self.named_map_receiver(object)&&script_named_map::prototype_member(property){return false;}
        if self.collection_receiver(object)&&self.collection_prototype_member(property){return false;}
        if parent_children::member(property) && self.parent_node_receiver(object) {return true;}
        if script_inline_style::handle(object) && (property == "length" || collection_index(property).is_some()) { return true; }
        if self.html_offset_member(object, property) {return true;}
        if self.node_list_iteration_member(object,property) {return false;}
        if self.element_sibling_member(object,property) || self.assigned_slot_lookup(object,property) && self.slottable(object).is_some() {return true;}
        if self.ordinary_event_owner(object).is_some() {return false;}
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

    fn nullable_scalar_setter(&self,object:u32,property:&str)->bool{property=="hidden"&&self.html_reflection_receiver(object)}
    fn nullable_string_setter(&self,object:u32,property:&str)->bool{self.aria_reflection_attribute(object,property)}
    fn string_setter(&self, object: u32, property: &str) -> bool {
        if self.html_label_for_member(object,property){return true;}
        if self.time_datetime_member(object,property){return true;}
        if self.area_alt_member(object,property){return true;}
        if self.optgroup_label_member(object,property){return true;}
        if self.option_label_member(object,property){return true;}
        if self.option_text_member(object,property){return true;}
        if self.textarea_default_value_member(object,property){return self.validate_textarea_default_value(object).is_ok();}
        if self.option_value_member(object,property){return true;}
        if self.select_name_member(object,property){return true;}
        if self.textarea_name_member(object,property){return true;}
        if self.area_coords_member(object,property){return true;}
        if self.area_shape_member(object,property){return true;}
        if self.area_download_member(object,property){return true;}
        if self.area_type_member(object,property){return true;}
        if self.area_hreflang_member(object,property){return true;}
        if self.area_target_member(object,property){return true;}
        if self.area_rel_member(object,property){return true;}
        if self.area_href_member(object,property){return true;}
        if self.area_url_member(object,property){return property!="origin";}
        if self.area_referrer_policy_member(object,property){return true;}
        if self.area_ping_member(object,property){return true;}
        if self.map_name_member(object,property){return true;}
        if self.param_name_member(object,property){return true;}
        if self.param_metadata_member(object,property){return true;}
        if self.mod_datetime_member(object,property){return true;}
        if self.data_value_member(object,property){return true;}
        if self.heading_align_member(object,property){return true;}
        if self.ul_type_member(object,property){return true;}
        if self.li_type_member(object,property){return true;}
        if self.ol_type_member(object,property){return true;}
        if self.input_autocomplete_member(object,property){return true;}
        if self.input_form_action_member(object,property){return true;}
        if self.input_form_method_member(object,property){return true;}
        if self.input_form_enctype_member(object,property){return true;}
        if self.button_form_action_member(object,property){return true;}
        if self.button_form_method_member(object,property){return true;}
        if self.button_form_enctype_member(object,property){return true;}
        if property=="autocomplete"&&self.form_autocomplete_receiver(object){return true;}
        if property=="method"&&self.form_method_receiver(object){return true;}
        if property=="action"&&self.form_action_receiver(object){return true;}
        if matches!(property,"enctype"|"encoding")&&self.form_enctype_receiver(object){return true;}
        if property=="name"&&self.form_name_receiver(object){return true;}
        if property=="name"&&self.button_name_receiver(object){return true;}
        if property=="name"&&self.input_name_receiver(object){return true;}
        if property=="acceptCharset"&&self.form_accept_charset_receiver(object){return true;}
        if property=="rel"&&self.form_rel_receiver(object){return true;}
        if property=="target"&&self.form_target_receiver(object){return true;}
        if property=="formTarget"&&self.button_form_target_receiver(object){return true;}
        if property=="formTarget"&&self.input_form_target_receiver(object){return true;}
        if property=="noValidate"&&self.form_no_validate_receiver(object){return false;}
        if property=="formNoValidate"&&self.button_form_no_validate_receiver(object){return false;}
        if property=="formNoValidate"&&self.input_form_no_validate_receiver(object){return false;}
        if self.select_autocomplete_member(object,property){return true;}
        if self.textarea_autocomplete_member(object,property){return true;}
        if self.textarea_wrap_member(object,property){return true;}
        if self.button_value_member(object,property){return true;}
        if self.button_type_member(object,property){return true;}
        if self.input_step_member(object,property){return true;}
        if self.input_max_member(object,property){return true;}
        if self.input_min_member(object,property){return true;}
        if self.input_pattern_member(object,property){return true;}
        if self.li_value_member(object,property){return false;}
        if self.pre_width_member(object,property){return false;}
        if self.textarea_dimensions_member(object,property){return false;}
        if self.ol_start_member(object,property){return false;}
        if self.textarea_min_length_member(object,property){return false;}
        if self.input_min_length_member(object,property){return false;}
        if self.textarea_max_length_member(object,property){return false;}
        if self.input_max_length_member(object,property){return false;}
        if self.ul_compact_member(object,property){return false;}
        if self.dir_compact_member(object,property){return false;}
        if self.menu_compact_member(object,property){return false;}
        if self.ol_compact_member(object,property){return false;}
        if self.hr_no_shade_member(object,property){return false;}
        if self.area_no_href_member(object,property){return false;}
        if self.option_disabled_member(object,property){return false;}
        if self.select_disabled_member(object,property){return false;}
        if self.button_disabled_member(object,property){return false;}
        if self.input_disabled_member(object,property){return false;}
        if self.textarea_disabled_member(object,property){return false;}
        if self.textarea_required_member(object,property){return false;}
        if self.input_required_member(object,property){return false;}
        if self.textarea_read_only_member(object,property){return false;}
        if self.input_read_only_member(object,property){return false;}
        if self.select_required_member(object,property){return false;}
        if self.option_default_selected_member(object,property){return false;}
        if self.optgroup_disabled_member(object,property){return false;}
        if self.ol_reversed_member(object,property){return false;}
        if self.dl_compact_member(object,property){return false;}
        if self.paragraph_align_member(object,property){return true;}
        if self.hr_align_member(object,property){return true;}
        if self.hr_color_member(object,property){return true;}
        if self.font_size_member(object,property){return true;}
        if self.font_face_member(object,property){return true;}
        if self.font_color_member(object,property){return true;}
        if self.br_clear_member(object,property){return true;}
        if self.hr_size_member(object,property){return true;}
        if self.hr_width_member(object,property){return true;}
        if self.legend_align_member(object,property){return true;}
        if self.div_align_member(object,property){return true;}
        if self.fieldset_name_member(object,property){return true;}
        if self.output_name_member(object,property){return true;}
        if self.output_value_member(object,property){return true;}
        if matches!(property,"id"|"className")&&self.canonical_selector_receiver(object){return true;}
        if property=="className"&&self.attr_element_receiver(object)&&node_index(object).is_some_and(|node|self.html_class_element(node)){return true;}
        if property!="origin" && script_anchor_url::property(property) && self.native_anchor(object) {return true;}
        if property=="text" && self.title_receiver(object) || object==DOCUMENT && property=="title" {return true;}
        if self.aria_reflection_attribute(object,property){return true;}
        if self.charset_member(object, property) { return true; }
        if self.meta_member(object, property) { return true; }
        if !matches!(property,"hidden"|"tabIndex")&&crate::javascript::html_reflection_interface::property(property).is_some()&&self.html_reflection_receiver(object){return true;}
        if property=="style"&&self.html_element_receiver(object){return true;}
        if self.placeholder_member(object,property){return true;}
        if self.xhr_index(object).is_some(){return property=="responseType";}
        if matches!(property, "textContent" | "nodeValue") && self.text_content_receiver(object) { return true; }
        if property=="name"&&self.html_slot_receiver(object){return true;}
        if self.mutation_observation_target(object)==Some(true)&&matches!(property,"data"|"nodeValue"){return true;}
        if matches!(self.inert_node_type(object),Some(3|4|7|8)) && matches!(property,"data"|"nodeValue"|"textContent"){return true;}

        if self.iframe_sandbox_member(object,property) {return true;}
        if self.textarea_value_member(object,property) {return true;}
        if property == "innerHTML" && self.template_receiver(object) { return true; }
        if object == DOCUMENT && property == "textContent" {return true;}
        if self.content_editable_attribute(object,property) {return true;}
        if property == "value" && self.input_accessor_member(object, property) {return true;}
        if matches!(property, "order" | "zIndex" | "z-index" | "visibility" | "pointerEvents" | "pointer-events" | "cssText" | "touchAction" | "touch-action" | "transform" | "opacity" | "animationTimingFunction" | "animation-timing-function") && (STYLE_BASE..DATASET_BASE).contains(&object) {return true;}
        if self.button_string_property(object,property) {return true;}
        if self.fragment_receiver(object) && matches!(property,"nodeValue"|"textContent") {return true;}
        if self.doctype_receiver(object)&&matches!(property,"nodeValue"|"textContent"){return true;}
        if self.hidden_input_property(object,property) {return true;}
        if self.text_input_default_property(object,property) {return true;}
        if self.media_interface_receiver(object)&&property=="src" {return true;}
        if self.style_text_attribute(object, property) { return true; }
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
    fn prepare_native_microtask(&mut self,object:u32,member:&str,args:&[Value],setter:bool)->Result<Option<u64>,String>{
        self.prepare_media_microtask(object,script_media::interface_operation(member).unwrap_or(member),args,setter)
    }
    fn native_setter_property(&self,object:u32,method:&str)->Option<&'static str>{
        if method==script_element_id::SET&&self.canonical_selector_receiver(object){return Some("id");}
        if method==script_element_class_name::SET&&self.canonical_selector_receiver(object){return Some("className");}
        if method==script_label_html_for::SET&&self.html_label_receiver(object){return Some("htmlFor");}
        if self.meter_numeric_receiver(object){match method{script_meter_numeric::VALUE_SET=>return Some("value"),script_meter_numeric::MIN_SET=>return Some("min"),script_meter_numeric::MAX_SET=>return Some("max"),script_meter_numeric::LOW_SET=>return Some("low"),script_meter_numeric::HIGH_SET=>return Some("high"),script_meter_numeric::OPTIMUM_SET=>return Some("optimum"),_=>{}}}
        if self.progress_numeric_receiver(object){match method{script_progress_numeric::VALUE_SET=>return Some("value"),script_progress_numeric::MAX_SET=>return Some("max"),_=>{}}}
        if method==script_fieldset::DISABLED_SET&&self.fieldset_receiver(object){return Some("disabled");}
        if method==script_time_datetime::SET&&self.time_datetime_receiver(object){return Some("dateTime");}
        if method==script_area_alt::SET&&self.area_alt_receiver(object){return Some("alt");}
        if method==script_optgroup_label::SET&&self.optgroup_label_receiver(object){return Some("label");}
        if method==script_option_label::SET&&self.option_label_receiver(object){return Some("label");}
        if method==script_option_text::SET&&self.option_text_receiver(object){return Some("text");}
        if method==script_textarea_default_value::SET&&self.textarea_default_value_receiver(object){return Some("defaultValue");}
        if method==script_option_value::SET&&self.option_value_receiver(object){return Some("value");}
        if method==script_select_name::SET&&self.select_name_receiver(object){return Some("name");}
        if method==script_textarea_name::SET&&self.textarea_name_receiver(object){return Some("name");}
        if method==script_area_coords::SET&&self.area_coords_receiver(object){return Some("coords");}
        if method==script_area_shape::SET&&self.area_shape_receiver(object){return Some("shape");}
        if method==script_area_download::SET&&self.area_download_receiver(object){return Some("download");}
        if method==script_area_type::SET&&self.area_type_receiver(object){return Some("type");}
        if method==script_area_hreflang::SET&&self.area_hreflang_receiver(object){return Some("hreflang");}
        if method==script_area_target::SET&&self.area_target_receiver(object){return Some("target");}
        if method==script_area_rel::SET&&self.area_rel_receiver(object){return Some("rel");}
        if method==script_area_href::SET&&self.area_href_receiver(object){return Some("href");}
        if let Some((name,true))=script_area_url::operation(method){return self.area_href_receiver(object).then_some(name);}
        if method==script_area_referrer_policy::SET&&self.area_referrer_policy_receiver(object){return Some("referrerPolicy");}
        if method==script_area_ping::SET&&self.area_ping_receiver(object){return Some("ping");}
        if method==script_map_name::SET&&self.map_name_receiver(object){return Some("name");}
        if method==script_param_name::SET&&self.param_name_receiver(object){return Some("name");}
        if let Some((property,true))=crate::javascript::param_metadata_interface::operation(method){if self.param_name_receiver(object){return Some(property);}}
        if method==script_mod_datetime::SET&&self.mod_datetime_receiver(object){return Some("dateTime");}
        if method==script_data_value::SET&&self.data_value_receiver(object){return Some("value");}
        if method==script_heading_align::SET&&self.heading_align_receiver(object){return Some("align");}
        if method==script_ul_type::SET&&self.ul_type_receiver(object){return Some("type");}
        if method==script_li_type::SET&&self.li_type_receiver(object){return Some("type");}
        if method==script_ol_type::SET&&self.ol_type_receiver(object){return Some("type");}
        if method==script_input_autocomplete::SET&&self.input_autocomplete_receiver(object){return Some("autocomplete");}
        if method==script_input_form_action::SET&&self.input_form_action_receiver(object){return Some("formAction");}
        if method==script_input_form_method::SET&&self.input_form_method_receiver(object){return Some("formMethod");}
        if method==script_input_form_enctype::SET&&self.input_form_enctype_receiver(object){return Some("formEnctype");}
        if method==script_button_form_action::SET&&self.button_form_action_receiver(object){return Some("formAction");}
        if method==script_button_form_method::SET&&self.button_form_method_receiver(object){return Some("formMethod");}
        if method==script_button_form_enctype::SET&&self.button_form_enctype_receiver(object){return Some("formEnctype");}
        if method==script_form_autocomplete::SET&&self.form_autocomplete_receiver(object){return Some("autocomplete");}
        if method==script_form_method::SET&&self.form_method_receiver(object){return Some("method");}
        if method==script_form_action::SET&&self.form_action_receiver(object){return Some("action");}
        if method==script_form_enctype::SET&&self.form_enctype_receiver(object){return Some("enctype");}
        if method==script_form_enctype::SET_ENCODING&&self.form_enctype_receiver(object){return Some("encoding");}
        if method==script_form_name::SET&&self.form_name_receiver(object){return Some("name");}
        if method==script_button_name::SET&&self.button_name_receiver(object){return Some("name");}
        if method==script_input_name::SET&&self.input_name_receiver(object){return Some("name");}
        if method==script_form_accept_charset::SET&&self.form_accept_charset_receiver(object){return Some("acceptCharset");}
        if method==script_form_rel::SET&&self.form_rel_receiver(object){return Some("rel");}
        if method==script_form_target::SET&&self.form_target_receiver(object){return Some("target");}
        if method==script_button_form_target::SET&&self.button_form_target_receiver(object){return Some("formTarget");}
        if method==script_input_form_target::SET&&self.input_form_target_receiver(object){return Some("formTarget");}
        if method==script_form_no_validate::SET&&self.form_no_validate_receiver(object){return Some("noValidate");}
        if method==script_button_form_no_validate::SET&&self.button_form_no_validate_receiver(object){return Some("formNoValidate");}
        if method==script_input_form_no_validate::SET&&self.input_form_no_validate_receiver(object){return Some("formNoValidate");}
        if method==script_select_autocomplete::SET&&self.select_autocomplete_receiver(object){return Some("autocomplete");}
        if method==script_textarea_autocomplete::SET&&self.textarea_autocomplete_receiver(object){return Some("autocomplete");}
        if method==script_textarea_wrap::SET&&self.textarea_wrap_receiver(object){return Some("wrap");}
        if method==script_button_value::SET&&self.button_value_receiver(object){return Some("value");}
        if method==script_button_type::SET&&self.button_type_receiver(object){return Some("type");}
        if method==script_input_step::SET&&self.input_step_receiver(object){return Some("step");}
        if method==script_input_max::SET&&self.input_max_receiver(object){return Some("max");}
        if method==script_input_min::SET&&self.input_min_receiver(object){return Some("min");}
        if method==script_input_pattern::SET&&self.input_pattern_receiver(object){return Some("pattern");}
        if method==script_li_value::SET&&self.li_value_receiver(object){return Some("value");}
        if method==script_pre_width::SET&&self.pre_width_receiver(object){return Some("width");}
        if let Some((property,true))=script_textarea_dimensions::operation(method){if self.textarea_dimensions_receiver(object){return Some(property);}}
        if method==script_ol_start::SET&&self.ol_start_receiver(object){return Some("start");}
        if method==script_textarea_min_length::SET&&self.textarea_min_length_receiver(object){return Some("minLength");}
        if method==script_input_min_length::SET&&self.input_min_length_receiver(object){return Some("minLength");}
        if method==script_textarea_max_length::SET&&self.textarea_max_length_receiver(object){return Some("maxLength");}
        if method==script_input_max_length::SET&&self.input_max_length_receiver(object){return Some("maxLength");}
        if method==script_ul_compact::SET&&self.ul_compact_receiver(object){return Some("compact");}
        if method==script_dir_compact::SET&&self.dir_compact_receiver(object){return Some("compact");}
        if method==script_menu_compact::SET&&self.menu_compact_receiver(object){return Some("compact");}
        if method==script_ol_compact::SET&&self.ol_compact_receiver(object){return Some("compact");}
        if method==script_hr_no_shade::SET&&self.hr_no_shade_receiver(object){return Some("noShade");}
        if method==script_area_no_href::SET&&self.area_no_href_receiver(object){return Some("noHref");}
        if method==script_option_disabled::SET&&self.option_disabled_receiver(object){return Some("disabled");}
        if method==script_select_disabled::SET&&self.select_disabled_receiver(object){return Some("disabled");}
        if method==script_button_disabled::SET&&self.button_disabled_receiver(object){return Some("disabled");}
        if method==script_input_disabled::SET&&self.input_disabled_receiver(object){return Some("disabled");}
        if method==script_textarea_disabled::SET&&self.textarea_disabled_receiver(object){return Some("disabled");}
        if method==script_textarea_required::SET&&self.textarea_required_receiver(object){return Some("required");}
        if method==script_input_required::SET&&self.input_required_receiver(object){return Some("required");}
        if method==script_textarea_read_only::SET&&self.textarea_read_only_receiver(object){return Some("readOnly");}
        if method==script_input_read_only::SET&&self.input_read_only_receiver(object){return Some("readOnly");}
        if method==script_select_required::SET&&self.select_required_receiver(object){return Some("required");}
        if method==script_option_default_selected::SET&&self.option_default_selected_receiver(object){return Some("defaultSelected");}
        if method==script_optgroup_disabled::SET&&self.optgroup_disabled_receiver(object){return Some("disabled");}
        if method==script_ol_reversed::SET&&self.ol_reversed_receiver(object){return Some("reversed");}
        if method==script_dl_compact::SET&&self.dl_compact_receiver(object){return Some("compact");}
        if method==script_paragraph_align::SET&&self.paragraph_align_receiver(object){return Some("align");}
        if method==script_hr_align::SET&&self.hr_align_receiver(object){return Some("align");}
        if method==script_hr_color::SET&&self.hr_color_receiver(object){return Some("color");}
        if method==script_font_size::SET&&self.font_size_receiver(object){return Some("size");}
        if method==script_font_face::SET&&self.font_face_receiver(object){return Some("face");}
        if method==script_font_color::SET&&self.font_color_receiver(object){return Some("color");}
        if method==script_br_clear::SET&&self.br_clear_receiver(object){return Some("clear");}
        if method==script_hr_size::SET&&self.hr_size_receiver(object){return Some("size");}
        if method==script_hr_width::SET&&self.hr_width_receiver(object){return Some("width");}
        if method==script_legend_align::SET&&self.legend_align_receiver(object){return Some("align");}
        if method==script_div_align::SET&&self.div_align_receiver(object){return Some("align");}
        if method==script_fieldset::NAME_SET&&self.fieldset_receiver(object){return Some("name");}
        if method==script_output_metadata::NAME_SET&&self.html_output_receiver(object){return Some("name");}
        if let Some((property,true))=script_output_value::operation(method){return self.html_output_receiver(object).then_some(property);}
        if script_class_list::operation(method)==Some("set value"){return self.class_list_receiver(object).then_some("value");}
        if let Some((interface,true)) = charset_operation(method) { return (self.charset_interface(object)==Some(interface) && self.charset_member(object,"charset")).then_some("charset"); }
        if let Some((property, _, true)) = meta_operation(method) { return self.meta_receiver(object).then_some(property); }
        if let Some((name,_,true))=crate::javascript::aria_reflection_interface::operation(method){return (self.aria_reflection_receiver(object)&&!self.is_frame_handle(object)).then_some(name);}
        if let Some((name,true))=crate::javascript::html_reflection_interface::operation(method){return (self.html_reflection_receiver(object)&&!self.is_frame_handle(object)).then_some(name);}
        if matches!(method,ELEMENT_STYLE_SET|SVG_STYLE_SET)&&self.style_interface_receiver(object,method==SVG_STYLE_SET)&&!self.is_frame_handle(object)&&self.inert_node_type(object).is_none(){return Some("style");}
        if matches!(placeholder::operation(method),Some((interface,true)) if self.placeholder_interface(object)==Some(interface)){return Some("placeholder");}
        if self.xhr_index(object).is_some(){return crate::javascript::xhr_interface::setter(method).map(|name|match name {"timeout"=>"timeout","withCredentials"=>"withCredentials","responseType"=>"responseType","onreadystatechange"=>"onreadystatechange","onloadstart"=>"onloadstart","onprogress"=>"onprogress","onabort"=>"onabort","onerror"=>"onerror","onload"=>"onload","ontimeout"=>"ontimeout","onloadend"=>"onloadend",_=>unreachable!()});}

        if method == ELEMENT_INNER_HTML_SET && (self.attr_element_receiver(object) || self.template_receiver(object)) {
            return Some("innerHTML");
        }
        if method == NODE_VALUE_SET && self.text_content_receiver(object) { return Some("nodeValue"); }
        if method == NODE_TEXT_CONTENT_SET && self.text_content_receiver(object) {return Some("textContent");}
        if let Some((name,true))=anchor_url_interface::operation(method){return self.native_anchor(object).then_some(name);}
        if method==TITLE_TEXT_SET && self.title_receiver(object) {return Some("text");}
        if method == HTML_SLOT_NAME_SET && self.html_slot_receiver(object) {return Some("name");}
        if method == script_content_editable::SET && self.content_editable_receiver(object) {return Some("contentEditable");}
        if let Some((name, true)) = script_canvas::accessor(method) {
            return self.canvas_receiver(object).then_some(name);
        }
        if script_iframe_sandbox::accessor(method)==Some(true) {return self.iframe_sandbox_receiver(object).then_some("sandbox");}
        if script_textarea_interface::accessor(method)==Some(true) {return self.textarea_value_receiver(object).then_some("value");}
        if let Some((name, true)) = script_input_accessors::accessor(method) {
            return self.input_receiver(object).then_some(name);
        }
        if let Some((name, true)) = script_scroll_offsets::accessor(method) {
            return (self.element_selector_receiver(object) && !self.is_frame_handle(object)).then_some(name);
        }
        self.media_interface_receiver(object).then(||script_media::interface_setter(method)).flatten()
    }
    fn finish_native_microtask(&mut self,ticket:u64)->bool{self.finish_media_microtask(ticket)}
    fn cancel_native_microtask(&mut self,ticket:u64){self.cancel_media_microtask(ticket)}
    fn run_native_microtask(&mut self,ticket:u64)->Result<(),String>{self.run_media_microtask(ticket)}

    fn number_setter(&self, object: u32, property: &str) -> bool {
        if self.li_value_member(object,property){return true;}
        if self.pre_width_member(object,property){return true;}
        if self.textarea_dimensions_member(object,property){return true;}
        if self.ol_start_member(object,property){return true;}
        if self.textarea_min_length_member(object,property){return true;}
        if self.input_min_length_member(object,property){return true;}
        if self.textarea_max_length_member(object,property){return true;}
        if self.input_max_length_member(object,property){return true;}
        if self.meter_numeric_attribute(object,property).is_some(){return true;}
        if self.progress_numeric_attribute(object,property).is_some(){return true;}
        if property=="tabIndex"&&self.html_reflection_receiver(object){return true;}
        if self.xhr_index(object).is_some(){return property=="timeout";}

        if self.canvas_member(object, property) {return true;}
        if script_scroll_offsets::axis(property).is_some() && self.element_selector_receiver(object) {return true;}
        if property=="currentTime" && self.media_interface_receiver(object) {return true;}
        node_index(object).is_some_and(|node| tag(self.doc, node) == "img")
            && matches!(property, "width" | "height")
            || node_index(object).is_some_and(|node| self.html_spacing_property(node, property))
    }

    fn preserves_dom_string_argument(&self,object:u32,method:&str,index:usize)->bool{
        if index==0&&(crate::javascript::control_custom_validity_interface(method).is_some()||matches!(method,script_output_validity_state::SET|script_output_validity_state::FIELD_SET)){return true;}
        if index==0&&method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME{return true;}
        if index==0&&self.collection_receiver(object)&&matches!(method,"\0HTMLCollection.namedItem"|"namedItem"){return true;}
        let method=script_class_list::operation(method).unwrap_or(method);
        if index==0 && self.class_list_receiver(object) && matches!(method,"contains"|"supports"){return true;}
        if matches!(method,"add"|"remove")&&self.exact_class_token_receiver(object){return true;}
        if index==0&&method=="toggle"&&self.exact_class_token_receiver(object){return true;}
        if index<2&&method=="replace"&&self.exact_class_token_receiver(object){return true;}
        if index==0&&(script_inert_documents::identifier_lookup::operation(method).is_some()||object==DOCUMENT&&method=="getElementById"){return true;}
        if let Some(edit)=character_data_interface::operation(method){return edit.data_index()==Some(index);}
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) {return index==0 || method==adjacent_interface::TEXT && index==1;}
        if let Some((query, _)) = dom_collections::operation(method) { return index < query.arity(); }
        index==1&&matches!(method,"createElementNS"|xml_documents::CREATE)||index==0&&method=="createElement"
            ||(script_attr::factory_operation(method).is_some_and(|ns|index==usize::from(ns)))
            ||index==0&&matches!(method,"getAttribute"|"getAttributeNode"|"setAttribute"|"removeAttribute"|"hasAttribute")
            ||index==1&&matches!(method,"getAttributeNS"|"getAttributeNodeNS"|"setAttributeNS"|"removeAttributeNS")
            ||script_named_map::operation(method).is_some_and(|name|name=="getNamedItem"&&index==0||name=="getNamedItemNS"&&index==1)
            ||index==0&&(script_attr::factory_operation(method)==Some(true)||matches!(method,"createAttributeNS"|"setAttributeNS"|"getAttributeNS"|"getAttributeNodeNS"|"removeAttributeNS")||script_named_map::operation(method)==Some("getNamedItemNS"))
            ||index==0&&method==CREATE_CDATA_SECTION
            ||index==0&&method==xml_documents::CREATE
            ||index==0&&(element_names::namespace_method(method)||method=="isDefaultNamespace"&&self.namespace_receiver(object))
            ||index==0&&method=="createElementNS"&&(object==DOCUMENT||self.inert_document(object))
            ||index<3&&method==script_inert_documents::CREATE_DOCTYPE&&self.inert_implementation(object)
            ||index==1&&method=="setAttribute"&&(self.attr_element_receiver(object)||self.inert_node_type(object)==Some(1))
            ||index==2&&method=="setAttributeNS"&&(self.attr_element_receiver(object)||self.inert_node_type(object)==Some(1))
            ||index==0&&(object==DOCUMENT&&matches!(method,"createTextNode"|"createComment")
            ||matches!(self.inert_factory(object,method),Some("createTextNode"|"createComment"))
            ||self.event_target_interface(object)&&matches!(method,"Text"|"Comment"))
            ||index<2&&(method==CREATE_PROCESSING_INSTRUCTION||method=="createProcessingInstruction"
                ||self.event_target_interface(object)&&method=="ProcessingInstruction")
    }
    fn preserves_dom_string_setter(&self,object:u32,property:&str)->bool{
        if self.html_label_for_member(object,property){return true;}
        if self.time_datetime_member(object,property){return true;}
        if self.area_alt_member(object,property){return true;}
        if self.optgroup_label_member(object,property){return true;}
        if self.option_label_member(object,property){return true;}
        if self.option_text_member(object,property){return true;}
        if self.textarea_default_value_member(object,property){return true;}
        if self.option_value_member(object,property){return true;}
        if self.select_name_member(object,property){return true;}
        if self.textarea_name_member(object,property){return true;}
        if self.area_coords_member(object,property){return true;}
        if self.area_shape_member(object,property){return true;}
        if self.area_download_member(object,property){return true;}
        if self.area_type_member(object,property){return true;}
        if self.area_hreflang_member(object,property){return true;}
        if self.area_target_member(object,property){return true;}
        if self.area_rel_member(object,property){return true;}
        if self.area_href_member(object,property){return true;}
        if self.area_url_member(object,property){return property!="origin";}
        if self.area_referrer_policy_member(object,property){return true;}
        if self.area_ping_member(object,property){return true;}
        if self.map_name_member(object,property){return true;}
        if self.param_name_member(object,property){return true;}
        if self.param_metadata_member(object,property){return true;}
        if self.mod_datetime_member(object,property){return true;}
        if self.data_value_member(object,property){return true;}
        if self.heading_align_member(object,property){return true;}
        if self.ul_type_member(object,property){return true;}
        if self.li_type_member(object,property){return true;}
        if self.ol_type_member(object,property){return true;}
        if self.input_autocomplete_member(object,property){return true;}
        if self.input_form_action_member(object,property){return true;}
        if self.input_form_method_member(object,property){return true;}
        if self.input_form_enctype_member(object,property){return true;}
        if self.button_form_action_member(object,property){return true;}
        if self.button_form_method_member(object,property){return true;}
        if self.button_form_enctype_member(object,property){return true;}
        if property=="autocomplete"&&self.form_autocomplete_receiver(object){return true;}
        if property=="method"&&self.form_method_receiver(object){return true;}
        if property=="action"&&self.form_action_receiver(object){return true;}
        if matches!(property,"enctype"|"encoding")&&self.form_enctype_receiver(object){return true;}
        if property=="name"&&self.form_name_receiver(object){return true;}
        if property=="name"&&self.button_name_receiver(object){return true;}
        if property=="name"&&self.input_name_receiver(object){return true;}
        if property=="acceptCharset"&&self.form_accept_charset_receiver(object){return true;}
        if property=="rel"&&self.form_rel_receiver(object){return true;}
        if property=="target"&&self.form_target_receiver(object){return true;}
        if property=="formTarget"&&self.button_form_target_receiver(object){return true;}
        if property=="formTarget"&&self.input_form_target_receiver(object){return true;}
        if property=="noValidate"&&self.form_no_validate_receiver(object){return false;}
        if property=="formNoValidate"&&self.button_form_no_validate_receiver(object){return false;}
        if property=="formNoValidate"&&self.input_form_no_validate_receiver(object){return false;}
        if self.select_autocomplete_member(object,property){return true;}
        if self.textarea_autocomplete_member(object,property){return true;}
        if self.textarea_wrap_member(object,property){return true;}
        if self.button_value_member(object,property){return true;}
        if self.button_type_member(object,property){return true;}
        if self.input_step_member(object,property){return true;}
        if self.input_max_member(object,property){return true;}
        if self.input_min_member(object,property){return true;}
        if self.input_pattern_member(object,property){return true;}
        if self.li_value_member(object,property){return false;}
        if self.pre_width_member(object,property){return false;}
        if self.textarea_dimensions_member(object,property){return false;}
        if self.ol_start_member(object,property){return false;}
        if self.textarea_min_length_member(object,property){return false;}
        if self.input_min_length_member(object,property){return false;}
        if self.textarea_max_length_member(object,property){return false;}
        if self.input_max_length_member(object,property){return false;}
        if self.ul_compact_member(object,property){return false;}
        if self.dir_compact_member(object,property){return false;}
        if self.menu_compact_member(object,property){return false;}
        if self.ol_compact_member(object,property){return false;}
        if self.hr_no_shade_member(object,property){return false;}
        if self.area_no_href_member(object,property){return false;}
        if self.option_disabled_member(object,property){return false;}
        if self.select_disabled_member(object,property){return false;}
        if self.button_disabled_member(object,property){return false;}
        if self.input_disabled_member(object,property){return false;}
        if self.textarea_disabled_member(object,property){return false;}
        if self.textarea_required_member(object,property){return false;}
        if self.input_required_member(object,property){return false;}
        if self.textarea_read_only_member(object,property){return false;}
        if self.input_read_only_member(object,property){return false;}
        if self.select_required_member(object,property){return false;}
        if self.option_default_selected_member(object,property){return false;}
        if self.optgroup_disabled_member(object,property){return false;}
        if self.ol_reversed_member(object,property){return false;}
        if self.dl_compact_member(object,property){return false;}
        if self.paragraph_align_member(object,property){return true;}
        if self.hr_align_member(object,property){return true;}
        if self.hr_color_member(object,property){return true;}
        if self.font_size_member(object,property){return true;}
        if self.font_face_member(object,property){return true;}
        if self.font_color_member(object,property){return true;}
        if self.br_clear_member(object,property){return true;}
        if self.hr_size_member(object,property){return true;}
        if self.hr_width_member(object,property){return true;}
        if self.legend_align_member(object,property){return true;}
        if self.div_align_member(object,property){return true;}
        if self.fieldset_name_member(object,property){return true;}
        if self.output_name_member(object,property){return true;}
        if self.output_value_member(object,property){return true;}
        if self.aria_reflection_attribute(object,property){return true;}
        if matches!(property,"id"|"className")&&self.canonical_selector_receiver(object){return true;}
        if matches!(property,"title"|"lang"|"dir")&&self.html_reflection_receiver(object){return true;}
        if matches!(property,"className"|"classList")&&self.attr_element_receiver(object)&&node_index(object).is_some_and(|node|self.html_class_element(node)){return true;}
        if property=="value"&&self.exact_class_token_receiver(object){return true;}
        if property=="charset" && node_index(object).is_some_and(|node|self.resource_charset_element(node)) {return true;}
        if property!="origin" && script_anchor_url::property(property) && self.native_anchor(object) {return true;}
        if property=="text" && self.title_receiver(object) || object==DOCUMENT && property=="title" {return true;}
        if property == "nodeValue" && self.text_content_receiver(object) { return true; }
        if self.charset_member(object, property) { return true; }
        if self.meta_member(object, property) { return true; }
        self.doctype_receiver(object)&&matches!(property,"nodeValue"|"textContent")
            ||self.attr_receiver(object)&&matches!(property,"value"|"nodeValue"|"textContent")
            ||matches!(property,"data"|"nodeValue")&&self.mutation_observation_target(object)==Some(true)
            ||property=="textContent"&&!self.attr_receiver(object)&&self.text_content_receiver(object)
    }
    fn argument_conversions(&self, object: u32, method: &str, args: &[Value]) -> &'static [(usize, bool)] {
        if crate::javascript::control_custom_validity_interface(method).is_some()||matches!(method,script_output_validity_state::SET|script_output_validity_state::FIELD_SET){return &[(0,true)];}
        let method=script_class_list::operation(method).unwrap_or(method);
        if self.class_list_receiver(object)&&method=="set value"{return &[(0,true)];}
        if method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME||script_inert_documents::identifier_lookup::operation(method).is_some(){return &[(0,true)];}
        if method==text_split_interface::METHOD{return &[(0,false)];}
        if let Some(edit)=character_data_interface::operation(method){return edit.conversions();}
        if method==character_substring::METHOD{return &[(0,false),(1,false)];}
        if method=="getAttributeNode"{return &[(0,true)];}if method=="getAttributeNodeNS"{return &[(0,true),(1,true)];}
        if let Some(ns)=script_attr::factory_operation(method){return if ns{&[(0,true),(1,true)]}else{&[(0,true)]};}
        if method==adjacent_interface::TEXT {return &[(0,true),(1,true)];}
        if method==adjacent_interface::ELEMENT {return &[(0,true)];}
        if let Some((query, _)) = dom_collections::operation(method) {
            return if query == dom_collections::Query::Namespace { &[(0,true),(1,true)] } else { &[(0,true)] };
        }
        if method==xml_documents::CREATE{return &[(0,true),(1,true)];}
        if element_names::namespace_method(method){return &[(0,true)];}
        if method==script_inert_documents::CREATE_DOCTYPE{return &[(0,true),(1,true),(2,true)];}
        if crate::javascript::xhr_interface::operation(method).is_some(){return &[];}

        if method==crate::javascript::range_interface::COMPARE_BOUNDARIES {return &[(0,false)];}
        if matches!(method,crate::javascript::range_interface::SET_START|crate::javascript::range_interface::SET_END|crate::javascript::range_interface::COMPARE_POINT|crate::javascript::range_interface::POINT_IN_RANGE) { return &[(1,false)]; }
        if method == script_inert_documents::parser::PARSE { return &[(0, true), (1, true)]; }
        if method == CREATE_CDATA_SECTION { return &[(0, true)]; }
        if method == CREATE_PROCESSING_INSTRUCTION { return &[(0, true), (1, true)]; }
        if object == WINDOW && method == "ProcessingInstruction" {
            return if matches!(args.get(1), None | Some(Value::Undefined)) {
                &[(0, true)]
            } else {
                &[(0, true), (1, true)]
            };
        }
        if self.inert_implementation(object) && method == script_inert_documents::CREATE {
            return if matches!(args.first(), None | Some(Value::Undefined)) { &[] } else { &[(0, true)] };
        }
        if self.inert_factory(object, method).is_some() {
            return if method == "createDocumentFragment" { &[] } else if method == "createElementNS" { &[(0, true), (1, true)] } else { &[(0, true)] };
        }
        if method=="setAttribute"&&self.inert_node_type(object)==Some(1){return &[(0,true),(1,true)];}
        if matches!(method,"getAttribute"|"getAttributeNode"|"hasAttribute"|"removeAttribute") && self.inert_node_type(object) == Some(1) {
            return &[(0, true)];
        }
        if let Some(operation) = CanvasOperation::parse(method) {
            return if operation.accepts(self, object) { operation.conversions() } else { &[] };
        }
        if matches!(method, "addEventListener" | "removeEventListener") { return &[(0, true)]; }
        if object == WINDOW && matches!(method, "Text" | "Comment") {
            return if matches!(args.first(), None | Some(Value::Undefined)) { &[] } else { &[(0, true)] };
        }
        if object==WINDOW && method=="getComputedStyle" {return &[(1,true)];}
        if object==WINDOW && method=="matchMedia" {return &[(0,true)];}
        if (STYLE_BASE..DATASET_BASE).contains(&object) {
            return match method {
                "item" => &[(0, false)],
                "setProperty" if matches!(args.get(1), Some(Value::Null)) => &[(0, true), (2, true)],
                "setProperty" => &[(0, true), (1, true), (2, true)],
                "getPropertyValue" | "removeProperty" => &[(0, true)],
                _ => &[],
            };
        }
        if self.computed_style_receiver(object) {return match method {"getPropertyValue"|"removeProperty"=>&[(0,true)],"setProperty"=>&[(0,true),(1,true),(2,true)],_=>&[]};}
        if script_element_selectors::operation(method).is_some() && self.selector_operation_receiver(object,method) {
            return if self.is_frame_handle(object) {&[]} else {&[(0,true)]};
        }
        if matches!(method,"Event"|"UIEvent"|"MouseEvent"|"WheelEvent"|"KeyboardEvent"|"FocusEvent"|"SubmitEvent"|"ProgressEvent"|"AnimationEvent"|"ErrorEvent"|"CustomEvent")&&self.event_target_interface(object) {return &[(0,true)];}
        if method=="getModifierState"&&self.mouse_receiver(object) || method=="\0KeyboardEvent.getModifierState"&&self.keyboard_receiver(object) {return &[(0,true)];}
        if object==WINDOW && method=="cancelAnimationFrame" {return &[(0,false)];}
        if object == HISTORY && matches!(method,"replaceState"|"pushState") { return &[(1, true), (2, true)]; }
        if self.named_map_receiver(object) {
            let method=script_named_map::operation(method).unwrap_or(method);
            return match method { "item" => &[(0, false)], "getNamedItem" => &[(0, true)], "getNamedItemNS" => &[(0, true), (1, true)], _ => &[] };
        }
        if self.class_list_receiver(object) {
            return match method { "item"=>&[(0,false)], "contains"|"supports"|"toggle"=>&[(0,true)], "replace"=>&[(0,true),(1,true)], _=>&[] };
        }
        if object == DOCUMENT && method == "createEvent" && self.event_factory_enabled()
            || self.event_receiver(object) && method == "initEvent"
            || self.custom_receiver(object) && method == "initCustomEvent"
        { return &[(0, true)]; }
        if self.mouse_receiver(object) && method == "initMouseEvent" {
            return &[(0, true), (4, false), (5, false), (6, false), (7, false), (8, false), (13, false)];
        }
        if self.ui_receiver(object) && method=="initUIEvent" {return &[(0,true),(4,false)];}
        if method == "isDefaultNamespace" && self.namespace_receiver(object) { return &[(0, true)]; }
        if object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS") {return &[(0,true),(1,true)];}
        if self.attr_element_receiver(object) && method == "getAttributeNode" { return &[(0, true)]; }
        if self.attribute_names_receiver(object) && method == "setAttributeNS" { return &[(0, true), (1, true), (2, true)]; }
        if self.attribute_names_receiver(object) && matches!(method,"getAttributeNS" | "getAttributeNodeNS" | "removeAttributeNS") { return &[(0, true), (1, true)]; }
        if self.collection_receiver(object)&&method=="\0HTMLCollection.item"{return &[(0,false)];}
        if self.collection_receiver(object)&&method=="\0HTMLCollection.namedItem"{return &[(0,true)];}
        if collection_handle_index(object).is_some() {
            return match method {
                "item" => &[(0, false)],
                "namedItem" => &[(0, true)],
                _ => &[],
            };
        }
        if (object == DOCUMENT || node_index(object).is_some() || matches!(self.inert_node_type(object),Some(1|9))) && method=="getElementsByTagName"
            || (object==DOCUMENT || self.attr_element_receiver(object) || matches!(self.inert_node_type(object),Some(1|9))) && method=="getElementsByClassName"
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
            || object == DOCUMENT && matches!(method, "createElement" | "createTextNode" | "createComment" | "createAttribute")
        {
            return &[(0, true)];
        }
        if self.node_list_receiver(object) && method == "\0NodeList.item" {
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
        let method=script_class_list::operation(method).unwrap_or(method);
        self.class_list_receiver(object) && matches!(method,"add"|"remove")
    }

    fn method_length(&self, object:u32, method:&str) -> Option<usize> {
        if method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME||script_inert_documents::identifier_lookup::operation(method).is_some(){return Some(1);}
        if script_attr::node_method(method){return Some(if method=="getAttributeNodeNS"{2}else{1});}
        if let Some(ns)=script_attr::factory_operation(method){return Some(if ns{2}else{1});}
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) {return Some(2);}
        if let Some((query, _)) = dom_collections::operation(method) { return Some(query.arity()); }
        if method==xml_documents::CREATE{return Some(2);}
        if method==script_inert_documents::CREATE_DOCTYPE{return Some(3);}
        if self.class_list_receiver(object)&&method=="supports" {return Some(1);}
        if self.inert_factory(object, method).is_some() {
            return Some(if method == "createDocumentFragment" { 0 } else if method == "createElementNS" { 2 } else { 1 });
        }
        if script_inline_style::handle(object) && method == "item" { return Some(1); }
        if let Some(operation) = CanvasOperation::parse(method) { return Some(operation.arity()); }
        if object==WINDOW && method=="getComputedStyle" {return Some(1);}
        if self.computed_style_receiver(object) || (STYLE_BASE..DATASET_BASE).contains(&object) {
            return match method { "getPropertyValue" | "removeProperty" => Some(1), "setProperty" => Some(2), _ => None };
        }
        if method=="createDocumentFragment" && (object==DOCUMENT || self.inert_document(object)) {return Some(0);}
        if method==script_node_root::ROOT_METHOD && self.get_root_receiver(object) {return Some(0);}
        if method == script_clone_node::CLONE_METHOD { return Some(0); }
        if script_inert_documents::node_equality::operation(method){return Some(1);}
        if method == script_node_contains::CONTAINS_METHOD || method==script_position::METHOD { return Some(1); }
        if object==DOCUMENT && method=="createComment" {return Some(1);}
        if matches!(method,"load"|"pause"|"play") && self.media_element_receiver(object) {return Some(0);}
        if method=="dispatchEvent"&&self.dispatch_receiver(object){return Some(1);}
        if object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS") {return Some(2);}
        if object==DOCUMENT && method=="createAttribute" || self.attr_element_receiver(object)&&matches!(method,"setAttributeNode"|"setAttributeNodeNS") {return Some(1);}
        if node_index(object).is_some() && method == "replaceChild" { return Some(2); }
        if object == HISTORY && matches!(method,"replaceState"|"pushState") { return Some(2); }
        if method == "getBoundingClientRect" && self.class_list_element_receiver(object) { return Some(0); }
        if self.named_map_receiver(object) { return match method { "item" | "getNamedItem" => Some(1), "getNamedItemNS" => Some(2), _ => None }; }
        if self.attribute_names_receiver(object) && method == "setAttributeNS" { return Some(3); }
        if self.event_receiver(object) && method=="initEvent"
            || self.custom_receiver(object) && method=="initCustomEvent"
            || self.ui_receiver(object) && method=="initUIEvent"
            || self.mouse_receiver(object) && method=="initMouseEvent" {return Some(1);}
        if method=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object) || matches!(self.inert_node_type(object),Some(1|9))) { return Some(1); }
        if self.class_list_receiver(object) {
            return match method {"contains"|"item"|"toggle"=>Some(1),"replace"=>Some(2),"add"|"remove"|"toString"=>Some(0),_=>None};
        }
        (self.attribute_names_receiver(object) && matches!(method,"getAttributeNS"|"getAttributeNodeNS"|"removeAttributeNS")).then_some(2)
    }

    fn native_method_owner(&self, object:u32, method:&str) -> Option<u32> {
        if script_class_list::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if self.event_target_interface(object)&&(method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME||script_inert_documents::identifier_lookup::operation(method).is_some()){return Some(object);}
        if self.event_target_interface(object)&&method==script_position::METHOD{return Some(object);}
        if self.event_target_interface(object)&&script_inert_documents::node_equality::operation(method){return Some(object);}
        if script_attr::node_method(method)&&self.event_target_interface(object){return Some(object);}
        if script_attr::member_operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if script_attr::factory_operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if dom_collections::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if self.console_message_limit(object,method).is_some(){return Some(WINDOW);}
        if placeholder::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0XHR."){return Some(object);}

        if script_iframe_sandbox::accessor(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if script_textarea_interface::accessor(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if crate::javascript::control_custom_validity_interface(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if crate::javascript::control_validation_operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if matches!(method,crate::javascript::HTML_FORM_SUBMIT|crate::javascript::HTML_FORM_RESET|crate::javascript::HTML_FORM_CHECK|crate::javascript::HTML_FORM_REPORT|crate::javascript::HTML_FORM_REQUEST) && self.event_target_interface(object) {return Some(object);}
        if self.event_target_interface(object) && crate::javascript::range_interface::operation(method) { return Some(object); }
        if self.event_target_interface(object)&&script_named_map::operation(method).is_some(){return Some(object);}
        if object==WINDOW && matches!(method,"matchMedia"|MEDIA_QUERY_MATCHES|MEDIA_QUERY_MEDIA) {return Some(WINDOW);}
        if object == WINDOW && crate::javascript::message_channel_interface::operation(method) { return Some(WINDOW); }
        if object == WINDOW && matches!(method, script_inert_documents::parser::CONSTRUCTOR | script_inert_documents::parser::PARSE) {
            return Some(WINDOW);
        }
        if self.event_target_interface(object) && crate::javascript::abort_interface::operation(method) {
            return Some(object);
        }
        if self.event_target_interface(object)&&crate::javascript::DOCUMENT_TYPE_GETTERS.iter().any(|(_,operation)|*operation==method){return Some(object);}
        if self.event_target_interface(object)
            && matches!(method, CREATE_CDATA_SECTION | CREATE_PROCESSING_INSTRUCTION | DOCUMENT_IMPORT_NODE | DOCUMENT_ADOPT_NODE
                | PROCESSING_INSTRUCTION_TARGET_GET | HTML_TEMPLATE_CONTENT_GET | "ProcessingInstruction"
                | script_inert_documents::GET | script_inert_documents::CREATE | script_inert_documents::CREATE_DOCTYPE | xml_documents::CREATE | script_inert_documents::HAS_FEATURE)
        {
            return Some(object);
        }
        if self.inert_factory(object, method).is_some() { return Some(WINDOW); }
        if matches!(method,"getAttributeNS"|"getAttributeNodeNS"|"setAttributeNS"|"removeAttributeNS"|"getAttribute"|"getAttributeNode"|"hasAttribute"|"removeAttribute"|"setAttribute"|"\0Element.getAttributeNames") && self.event_target_interface(object) { return Some(object); }
        if script_content_editable::accessor(method) && self.event_target_interface(object) {return Some(object);}
        if matches!(method, ELEMENT_INNER_HTML_GET | ELEMENT_INNER_HTML_SET)
            && self.event_target_interface(object) { return Some(object); }
        if crate::javascript::window_relationships::operation(method).is_some() && self.event_target_interface(object) { return Some(object); }
        if matches!(method, NODE_VALUE_GET | NODE_VALUE_SET | NODE_HAS_CHILDREN) && self.event_target_interface(object) { return Some(object); }
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) && self.event_target_interface(object) {return Some(object);}
        if matches!(method,TITLE_TEXT_GET|TITLE_TEXT_SET) && self.event_target_interface(object){return Some(object);}
        if matches!(method,crate::javascript::document_location_interface::GET|crate::javascript::document_location_interface::SET) && self.event_target_interface(object) {return Some(object);}
        if script_element_id::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_element_class_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_labels::interface(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if method==script_label_form::GET&&self.event_target_interface(object){return Some(object);}
        if method==script_output_form::GET&&self.event_target_interface(object){return Some(object);}
        if script_control_type::interface(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if method==script_form_length::GET&&self.event_target_interface(object){return Some(object);}
        if script_meter_numeric::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_progress_numeric::operation(method)&&self.event_target_interface(object){return Some(object);}
        if method==script_legend_form::GET&&self.event_target_interface(object){return Some(object);}
        if method==script_option_form::GET&&self.event_target_interface(object){return Some(object);}
        if method==script_option_index::GET&&self.event_target_interface(object){return Some(object);}
        if method==script_textarea_text_length::GET&&self.event_target_interface(object){return Some(object);}
        if method==script_map_areas::GET&&self.event_target_interface(object){return Some(object);}
        if method==script_datalist_options::GET&&self.event_target_interface(object){return Some(object);}
        if script_time_datetime::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_alt::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_optgroup_label::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_option_label::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_option_text::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_default_value::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_option_value::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_select_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_coords::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_shape::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_download::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_type::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_hreflang::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_target::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_rel::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_href::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_hyperlink_stringifier::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_url::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if script_area_referrer_policy::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_ping::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_map_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_param_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_param_metadata::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_mod_datetime::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_data_value::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_heading_align::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_ul_type::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_li_type::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_ol_type::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_autocomplete::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_form_action::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_form_method::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_form_enctype::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_form_action::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_form_method::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_form_enctype::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_autocomplete::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_method::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_action::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_enctype::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_name::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_accept_charset::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_rel::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_target::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_form_target::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_form_target::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_no_validate::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_form_no_validate::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_form_no_validate::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_select_autocomplete::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_autocomplete::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_wrap::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_value::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_type::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_step::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_max::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_min::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_pattern::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_li_value::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_pre_width::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_dimensions::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if script_ol_start::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_min_length::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_min_length::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_max_length::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_max_length::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_ul_compact::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_dir_compact::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_menu_compact::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_ol_compact::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_hr_no_shade::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_no_href::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_option_disabled::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_select_disabled::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_button_disabled::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_disabled::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_disabled::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_required::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_required::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_textarea_read_only::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_input_read_only::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_select_required::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_option_default_selected::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_optgroup_disabled::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_ol_reversed::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_dl_compact::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_paragraph_align::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_hr_align::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_hr_color::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_font_size::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_font_face::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_font_color::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_br_clear::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_hr_size::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_hr_width::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_legend_align::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_div_align::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_fieldset::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_control_form::interface(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if script_output_metadata::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_control_eligibility::interface(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if script_fieldset_validation::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_output_validation::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_output_validity_state::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_output_value::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if method==script_label_control::GET&&self.event_target_interface(object){return Some(object);}
        if script_label_html_for::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_element_class_list::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_output_html_for::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_area_rel_list::operation(method)&&self.event_target_interface(object){return Some(object);}
        if script_form_rel_list::operation(method)&&self.event_target_interface(object){return Some(object);}
        if anchor_url_interface::operation(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if crate::javascript::child_remove_operation(method) && self.event_target_interface(object) {return Some(object);}
        if charset_operation(method).is_some() && self.event_target_interface(object) { return Some(object); }
        if meta_operation(method).is_some() && self.event_target_interface(object) { return Some(object); }
        if crate::javascript::aria_reflection_interface::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if crate::javascript::html_reflection_interface::operation(method).is_some()&&self.event_target_interface(object){return Some(object);}
        if matches!(method, ELEMENT_STYLE_GET | ELEMENT_STYLE_SET |SVG_STYLE_GET|SVG_STYLE_SET)
            && self.event_target_interface(object) { return Some(object); }
        if script_inline_style::handle(object) && script_inline_style::method(method).is_some() {return Some(WINDOW);}
        if CanvasOperation::parse(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if script_canvas::accessor(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if script_input_accessors::accessor(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if script_scroll_offsets::accessor(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if Offset::getter(method).is_some() && self.event_target_interface(object) {return Some(object);}
        if object==WINDOW && method=="getComputedStyle" {return Some(WINDOW);}
        if self.computed_style_receiver(object) {return self.computed_style_owner(object);}
        if self.event_target_interface(object)&&matches!(method,"\0HTMLCollection.get.length"|"\0HTMLCollection.item"|"\0HTMLCollection.namedItem"){return Some(object);}
        if self.event_target_interface(object)&&matches!(method,"\0NodeList.get.length"|"\0NodeList.item") {return Some(object);}
        if self.event_target_interface(object)&&script_element_selectors::operation(method).is_some() {return Some(object);}
        if self.event_target_interface(object)&&script_element_siblings::operation(method).is_some() {return Some(object);}
        if self.event_target_interface(object) && (matches!(method, "Document" | "\0Document.get.documentElement" | crate::javascript::DOCUMENT_DOCTYPE_GET | DOCUMENT_CURRENT_SCRIPT_GET | DOCUMENT_ACTIVE_ELEMENT_GET | "DocumentFragment" | "Text" | "Comment") || Slottable::getter(method).is_some() || crate::javascript::document_encoding_getter(method)) {return Some(object);}
        if self.event_target_interface(object) && (matches!(method, script_node_root::ROOT_METHOD | script_node_contains::CONTAINS_METHOD | script_clone_node::CLONE_METHOD)||element_names::namespace_method(method)||method==character_substring::METHOD||character_data_interface::operation(method).is_some()||method==text_whole::GETTER||method==text_split_interface::METHOD||method==node_normalize_interface::METHOD) {return Some(object);}
        if self.event_target_interface(object) && dom_tree::node_mutation_method(method) {return Some(object);}
        if self.event_target_interface(object) && matches!(method, HTML_SLOT_NAME_GET | HTML_SLOT_NAME_SET) {
            return Some(object);
        }
        if self.event_target_interface(object) && (node_traversal_property(method).is_some() || parent_children::operation(method).is_some()
            || matches!(method, "\0Node.get.nodeType" | "\0Node.get.nodeName" | NODE_TEXT_CONTENT_GET | NODE_TEXT_CONTENT_SET))
        {
            return Some(object);
        }
        if self.event_target_interface(object)&&(method.starts_with("\0CustomEvent.")||method=="initCustomEvent") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0KeyboardEvent.") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0FocusEvent.") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0SubmitEvent.") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0WheelEvent.") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0ErrorEvent.") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0AnimationEvent.") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0ProgressEvent.") {return Some(object);}
        if self.event_target_interface(object)&&method.starts_with("\0Media.")
            || self.event_target_interface(object)&&method.starts_with("\0Video.") {return Some(object);}
        if self.media_element_receiver(object)&&matches!(method,"load"|"pause"|"play") {return Some(WINDOW);}
        self.created_event_method(object,method).and_then(|_|self.event_owner(object))
    }

    fn unpublished_base_event_owner(&self,object:u32,method:&str,result:u32)->Option<u32> {
        if !(object==DOCUMENT&&method=="createEvent"||self.event_target_interface(object)&&matches!(method,"Event"|"UIEvent"|"MouseEvent"|"WheelEvent"|"KeyboardEvent"|"FocusEvent"|"SubmitEvent"|"ProgressEvent"|"AnimationEvent"|"ErrorEvent"|"CustomEvent")) {
            return None;
        }
        if self.state.created_events.last().is_none_or(|event|event.handle!=result) {return None;}
        self.ordinary_event_owner(result)
    }
    fn rollback_unpublished_base_event(&mut self,event:u32)->Result<(),String>{self.rollback_base_event(event)}
    fn attribute_name_snapshot(&mut self,object:u32)->Result<Vec<DomString>,String>{self.attribute_names(object)}
    fn event_path_snapshot(&mut self,event:u32)->Result<Vec<u32>,String>{self.copy_event_path(event)}

    fn nullable_string_argument(&self, object: u32, method: &str, index: usize) -> bool {
        if index == 0 && matches!(dom_collections::operation(method), Some((dom_collections::Query::Namespace, _))) { return true; }
        if method=="getAttributeNodeNS"&&index==0{return true;}
        if script_attr::factory_operation(method)==Some(true)&&index==0{return true;}
        if method==xml_documents::CREATE&&index==0{return true;}
        if (STYLE_BASE..DATASET_BASE).contains(&object) && method == "setProperty" && index == 2 { return true; }
        if object==WINDOW && method=="getComputedStyle" && index==1 {return true;}
        if (object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS")
            || self.inert_document(object) && method=="createElementNS") && index==0 {return true;}
        if object == HISTORY && matches!(method,"replaceState"|"pushState") && index == 2 { return true; }
        if self.named_map_receiver(object) { return script_named_map::operation(method).unwrap_or(method) == "getNamedItemNS" && index == 0; }
        element_names::namespace_method(method)&&index==0
            ||method == "isDefaultNamespace" && index == 0 && self.namespace_receiver(object)
            || matches!(method,"getAttributeNS" | "getAttributeNodeNS" | "removeAttributeNS" | "setAttributeNS") && index == 0 && self.attribute_names_receiver(object)
    }

    fn validate_argument_prefix(&self, object: u32, method: &str, next_index: usize, args: &[Value])
        -> Result<(), (&'static str, String)> {
        if method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME{return self.validate_name_list(object);}
        if object==WINDOW && method=="cancelAnimationFrame" && (!self.state.enabled || !self.doc.scripting_enabled) {return Err(("SecurityError","Animation capability is unavailable while scripting is disabled.".into()));}
        if object==WINDOW && method=="getComputedStyle" && next_index==1 {self.validate_computed_target(args.first())?;}
        if crate::javascript::range_interface::node_argument(method) && next_index==1 {
            if !matches!(args.first(),Some(Value::Host(handle)) if *handle==DOCUMENT || node_index(*handle).is_some_and(|node|self.dom_accessible(node)) || self.inert_node_type(*handle).is_some() || self.attr_receiver(*handle) || self.frame_node_interface(*handle).is_some()) {
                return Err(("TypeError","A genuine Node is required for a Range boundary.".into()));
            }
        }
        self.validate_event_prefix(object, method, next_index, args)
    }
    fn native_reference_arguments(&self,object:u32,method:&str)->&'static [usize] {
        if method==adjacent_interface::ELEMENT {return &[1];}
        if method==script_position::METHOD{return &[0];}
        if script_inert_documents::node_equality::operation(method){return &[0];}
        if method=="replaceChild"&&self.inert_replace_receiver(object){return &[0,1];}
        if method==crate::javascript::range_interface::COMPARE_BOUNDARIES {return &[1];}
        if crate::javascript::range_interface::node_argument(method) { return &[0]; }
        if matches!(method,"appendChild"|"insertBefore"|"removeChild") && self.inert_mutation_receiver(object) { return if method=="insertBefore"{&[0,1]}else{&[0]}; }
        if matches!(method, "addEventListener" | "removeEventListener") { return &[1]; }
        if matches!(method, script_node_contains::CONTAINS_METHOD | DOCUMENT_IMPORT_NODE | DOCUMENT_ADOPT_NODE) { return &[0]; }
        if method=="initMouseEvent" && self.mouse_receiver(object) {&[3,14]}
        else if method=="initUIEvent" && self.ui_receiver(object) {&[3]}
        else {&[]}
    }

    fn nullish_string_setter(&self, object: u32, property: &str) -> bool {
        (self.attr_receiver(object) || self.comment_receiver(object) || self.fragment_receiver(object) || self.text_content_receiver(object)) && matches!(property, "nodeValue" | "textContent")
    }

    fn null_string_setter(&self, object: u32, property: &str) -> bool {
        if self.font_color_member(object,property){return true;}
        if property=="style"&&self.html_element_receiver(object){return true;}
        if self.textarea_value_member(object,property) {return true;}
        property == "innerHTML" && self.template_receiver(object)
            || property == "value" && self.input_accessor_member(object, property)
            || matches!(property, "order" | "zIndex" | "z-index" | "visibility" | "pointerEvents" | "pointer-events" | "cssText" | "touchAction" | "touch-action" | "transform" | "opacity" | "animationTimingFunction" | "animation-timing-function") && (STYLE_BASE..DATASET_BASE).contains(&object)
            || property == "value" && self.hidden_input_property(object,property)
            || property == "value" && self.text_input_default_property(object,property)
            || property == "innerHTML" && (self.attr_element_receiver(object) || self.template_receiver(object))
            || property == "border" && node_index(object).is_some_and(|node|
            self.html_border_property(node) && matches!(tag(self.doc, node), "img" | "object"))
    }

    fn required_arguments(&self, object: u32, method: &str) -> usize {
        if crate::javascript::control_custom_validity_interface(method).is_some()||matches!(method,script_output_validity_state::SET|script_output_validity_state::FIELD_SET){return 1;}
        let method=script_class_list::operation(method).unwrap_or(method);
        if self.class_list_receiver(object)&&method=="set value"{return 1;}
        if method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME||script_inert_documents::identifier_lookup::operation(method).is_some(){return 1;}
        if method==script_position::METHOD{return 1;}
        if script_inert_documents::node_equality::operation(method){return 1;}
        if script_attr::node_method(method){return if method=="getAttributeNodeNS"{2}else{1};}
        if let Some((_,setter))=script_attr::member_operation(method){return usize::from(setter);}
        if let Some(ns)=script_attr::factory_operation(method){return if ns{2}else{1};}
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) {return 2;}
        if let Some((query, _)) = dom_collections::operation(method) { return query.arity(); }
        if method==xml_documents::CREATE{return 2;}
        if element_names::namespace_method(method){return 1;}
        if method==character_substring::METHOD{return 2;}
        if method==text_split_interface::METHOD{return 1;}
        if let Some(edit)=character_data_interface::operation(method){return edit.arity();}
        if self.doctype_receiver(object)&&dom_tree::node_mutation_method(method){return if matches!(method,"insertBefore"|"replaceChild"){2}else{1};}
        if method==script_inert_documents::CREATE_DOCTYPE{return 3;}
        if let Some(operation)=crate::javascript::xhr_interface::operation(method){return match operation {"open"|"setRequestHeader"=>2,"getResponseHeader"|"overrideMimeType"=>1,_=>0};}

        if crate::javascript::range_interface::operation(method) { return crate::javascript::range_interface::arity(method); }
        if let Some(method)=script_named_map::operation(method){return match method{"item"|"getNamedItem"=>1,"getNamedItemNS"=>2,_=>0};}
        if method == script_inert_documents::parser::PARSE { return 2; }
        if method=="replaceChild"&&self.inert_replace_receiver(object){return 2;}
        if matches!(method,"appendChild"|"insertBefore"|"removeChild") && self.inert_mutation_receiver(object) { return if method=="insertBefore"{2}else{1}; }
        if matches!(method,DOCUMENT_IMPORT_NODE|DOCUMENT_ADOPT_NODE) { return 1; }
        if method == CREATE_PROCESSING_INSTRUCTION { return 2; }
        if method == CREATE_CDATA_SECTION || object == WINDOW && method == "ProcessingInstruction" { return 1; }
        if self.inert_factory(object, method).is_some() {
            return if method == "createDocumentFragment" { 0 } else if method == "createElementNS" { 2 } else { 1 };
        }
        if method=="setAttribute"&&self.inert_node_type(object)==Some(1){return 2;}
        if matches!(method,"getAttribute"|"getAttributeNode"|"hasAttribute"|"removeAttribute") && self.inert_node_type(object) == Some(1) { return 1; }
        if script_inline_style::handle(object) && method == "item" { return 1; }
        if let Some(operation) = CanvasOperation::parse(method) { return operation.arity(); }
        if matches!(method, "addEventListener" | "removeEventListener") { return 2; }
        if method == script_node_contains::CONTAINS_METHOD { return 1; }
        if object==WINDOW && method=="getComputedStyle" {return 1;}
        if object==WINDOW && method=="matchMedia" {return 1;}
        if self.computed_style_receiver(object) || (STYLE_BASE..DATASET_BASE).contains(&object) {
            return match method { "setProperty" => 2, "getPropertyValue" | "removeProperty" => 1, _ => 0 };
        }
        if script_element_selectors::operation(method).is_some() && self.selector_operation_receiver(object,method) {return 1;}
        if matches!(method,"Event"|"UIEvent"|"MouseEvent"|"WheelEvent"|"KeyboardEvent"|"FocusEvent"|"SubmitEvent"|"ProgressEvent"|"AnimationEvent"|"ErrorEvent"|"CustomEvent")&&self.event_target_interface(object) {return 1;}
        if method=="getModifierState"&&self.mouse_receiver(object) || method=="\0KeyboardEvent.getModifierState"&&self.keyboard_receiver(object) {return 1;}
        if object==WINDOW && matches!(method,"requestAnimationFrame"|"cancelAnimationFrame") {return 1;}
        if method=="dispatchEvent"&&self.dispatch_receiver(object){return 1;}
        if object==DOCUMENT && matches!(method,"createAttributeNS"|"createElementNS") {return 2;}
        if object==DOCUMENT && method=="createAttribute" || self.attr_element_receiver(object)&&matches!(method,"setAttributeNode"|"setAttributeNodeNS") {return 1;}
        if object == HISTORY && matches!(method,"replaceState"|"pushState") { return 2; }
        if self.named_map_receiver(object) { return match method { "item" | "getNamedItem" => 1, "getNamedItemNS" => 2, _ => 0 }; }
        if self.attribute_names_receiver(object) && method == "setAttributeNS" { return 3; }
        if self.class_list_receiver(object)&&method=="replace"{return 2;}
        if self.class_list_receiver(object) && matches!(method,"item"|"contains"|"supports"|"toggle") { return 1; }
        if self.attribute_names_receiver(object) && matches!(method,"getAttributeNS" | "getAttributeNodeNS" | "removeAttributeNS") { return 2; }
        if object == DOCUMENT && method == "createEvent" && self.event_factory_enabled()
            || self.event_receiver(object) && method == "initEvent"
            || self.custom_receiver(object) && method == "initCustomEvent"
            || self.ui_receiver(object) && method=="initUIEvent"
            || self.mouse_receiver(object) && method == "initMouseEvent"
        { return 1; }
        if matches!(method, "isDefaultNamespace" | "compareDocumentPosition") && self.namespace_receiver(object) { return 1; }
        if self.attr_element_receiver(object) && matches!(method, "getAttributeNode" | "removeAttributeNode") { return 1; }
        if collection_handle_index(object).is_some() && matches!(method, "item" | "namedItem")
            || (object == DOCUMENT || node_index(object).is_some() || matches!(self.inert_node_type(object),Some(1|9)))
                && matches!(method,"getElementsByTagName"|"getElementsByClassName")
        {
            return 1;
        }
        if object == LOCATION && matches!(method, "assign" | "replace") {
            return 1;
        }
        if node_index(object).is_some() && matches!(method, "setAttribute" | "insertBefore" | "replaceChild") {
            2
        } else if (object == DOCUMENT || node_index(object).is_some() || matches!(self.inert_node_type(object),Some(1|9|11)))
            && matches!(
                method,
                "querySelector" | "querySelectorAll" | "matches" | "closest"
            )
            || object == DOCUMENT
                && matches!(
                    method,
                    "getElementById" | "createElement" | "createTextNode" | "createComment"
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
            || self.node_list_receiver(object) && method=="\0NodeList.item"
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
        if let Some(operation)=script_class_list::operation(method){return self.class_list_validate(object,operation,args);}
        if script_attr::node_method(method){if !(self.attr_element_receiver(object)||self.inert_node_type(object)==Some(1)){return Err(("TypeError","Attribute-node method requires a genuine Element.".into()));}return match method{"setAttributeNode"|"setAttributeNodeNS"=>self.validate_attr_attachment(object,args),"removeAttributeNode"=>self.validate_attr_removal(object,args),_=>Ok(())};}
        if script_attr::member_operation(method).is_some(){return if self.attr_receiver(object){Ok(())}else{Err(("TypeError","Attr operation requires a genuine Attr.".into()))};}
        if self.attr_receiver(object)&&dom_tree::node_mutation_method(method){return Err(if method=="removeChild"{("NotFoundError","An Attr has no children.".into())}else{("HierarchyRequestError","An Attr cannot contain children.".into())});}
        if let Some(ns)=script_attr::factory_operation(method){return self.validate_attribute_factory(object,args,ns);}
        if let Some((_, document)) = dom_collections::operation(method) {
            if self.collection_interface_receiver(object) != Some(document) {
                return Err(("TypeError", "Live collection method requires its genuine Document or Element receiver.".into()));
            }
            return self.native_node_access(object);
        }
        if method==xml_documents::CREATE{return self.validate_xml_document_factory(args);}
        if method=="createElement"&&self.inert_document(object){if let Some(name)=args.first(){if !script_attribute_names::valid_element_local_name_dom(script_attribute_names::converted_name(name)?){return Err(("InvalidCharacterError","Invalid element local name.".into()));}}}

        if self.event_target_interface(object)&&matches!(method,"XMLHttpRequestEventTarget"|"XMLHttpRequestUpload"){return Err(("TypeError","Illegal request interface constructor.".into()));}
        if method.starts_with("\0XHR.") {
            if let Some(name)=crate::javascript::xhr_interface::getter(method).or_else(||crate::javascript::xhr_interface::setter(method)){return self.xhr_validate_member(object,name);}
            if !self.xhr_index(object).is_some_and(|(_,upload)|!upload){return Err(("TypeError","Request operations require a genuine XMLHttpRequest.".into()));}
            if !self.state.enabled||!self.doc.scripting_enabled{return Err(("InvalidStateError","Script requests are disabled.".into()));}
            return Ok(());
        }

        if let Some(class)=crate::javascript::control_custom_validity_interface(method){return self.validate_control_custom_validity(object,class);}
        if let Some((class,_))=crate::javascript::control_validation_operation(method){return self.validate_control_validation(object,class);}
        if method==crate::javascript::HTML_FORM_REQUEST{return self.validate_form_request(object,args).map(|_|());}
        if matches!(method,crate::javascript::HTML_FORM_CHECK|crate::javascript::HTML_FORM_REPORT){return self.validate_form_validation(object);}
        if method==crate::javascript::HTML_FORM_RESET {return self.validate_form_reset(object);}
        if method==crate::javascript::HTML_FORM_SUBMIT && self.inert_interface(object)==Some("HTMLFormElement") {
            return Err(("NotSupportedError","Inert form submission has no native form backend.".into()));
        }
        let method=if method==crate::javascript::HTML_FORM_SUBMIT {"submit"}else{method};
        if crate::javascript::range_interface::operation(method) { return Ok(()); }
        if method == MEDIA_QUERY_LIST && object == WINDOW {
            return Err(("TypeError", "Illegal MediaQueryList constructor.".into()));
        }
        if method=="matchMedia" {
            if object==WINDOW && matches!(args.first(), Some(Value::String(_))) {return Ok(());}
            return Err(("TypeError","matchMedia requires a CSS media query string.".into()));
        }
        if crate::javascript::message_channel_interface::operation(method) {
            return self.validate_message_call(object, method, args);
        }
        if method=="replaceChild"&&self.inert_replace_receiver(object){return self.validate_inert_replace(object,args);}
        if matches!(method,"appendChild"|"insertBefore"|"removeChild") && self.inert_mutation_receiver(object) {
            return self.validate_inert_mutation(object,method,args);
        }
        if method == script_inert_documents::parser::PARSE {
            return match args.get(1) {
                Some(Value::String(mime)) if mime == "text/html" => Ok(()),
                Some(Value::String(mime)) if matches!(mime.as_str(), "text/xml" | "application/xml" | "application/xhtml+xml" | "image/svg+xml") =>
                    Err(("NotSupportedError", "XML DOMParser parsing is outside the supported subset.".into())),
                _ => Err(("TypeError", "Invalid DOMParser supported type.".into())),
            };
        }
        if method == crate::javascript::abort_interface::SIGNAL_INTERFACE && self.event_target_interface(object) {
            return Err(("TypeError", "Illegal AbortSignal constructor.".into()));
        }
        if self.event_target_interface(object) && matches!(method, "DOMImplementation" | "DocumentType" | "XMLDocument") {
            return Err(("TypeError", "Illegal inert DOM interface constructor.".into()));
        }
        if matches!(method, NODE_VALUE_GET | NODE_VALUE_SET | NODE_HAS_CHILDREN) { return self.native_node_access(object); }
        if matches!(method, NODE_TEXT_CONTENT_GET | NODE_TEXT_CONTENT_SET) {
            if !self.text_content_receiver(object) {
                return Err(("TypeError", "The textContent accessor requires a Node.".into()));
            }
            if method == NODE_TEXT_CONTENT_SET && self.is_frame_handle(object) {
                return Err(("NotSupportedError", "Blank child native writes are not implemented.".into()));
            }
            return Ok(());
        }
        if let Some((_, setter)) = crate::javascript::window_relationships::operation(method) {
            if !self.event_target_interface(object) { return Err(("TypeError", "Expected a Window receiver.".into())); }
            self.window_relationship_access(object).map_err(|e| ("SecurityError", e))?;
            if setter && object != WINDOW { return Err(("NotSupportedError", "Child Window relationship writes are unavailable.".into())); }
            return Ok(());
        }
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) {
            self.adjacent_access(object)?;
            if args.len()<2 {return Err(("TypeError","Adjacent insertion requires two arguments.".into()));}
            if method==adjacent_interface::ELEMENT {
                let Some(Value::Host(child))=args.get(1) else{return Err(("TypeError","Expected an Element to insert.".into()))};
                if self.collection_interface_receiver(*child)!=Some(false) {return Err(("TypeError","Expected an Element to insert.".into()));}
                self.native_node_access(*child)?;
            }
            return Ok(());
        }
        if matches!(method,TITLE_TEXT_GET|TITLE_TEXT_SET) {return self.native_node_access(object);}
        if script_element_id::operation(method){return self.validate_element_id(object,method==script_element_id::SET);}
        if script_element_class_name::operation(method){return self.validate_element_class_name(object,method==script_element_class_name::SET);}
        if script_input_labels::interface(method).is_some(){return self.validate_control_labels(object,method);}
        if method==script_label_form::GET{return self.validate_html_label_form(object);}
        if method==script_output_form::GET{return self.validate_output_form(object);}
        if script_control_type::interface(method).is_some(){return self.validate_control_type(object,method);}
        if method==script_form_length::GET{return self.validate_form_length(object);}
        if script_meter_numeric::operation(method){return self.validate_meter_numeric(object);}
        if script_progress_numeric::operation(method){return self.validate_progress_numeric(object);}
        if method==script_legend_form::GET{return self.validate_legend_form(object);}
        if method==script_option_form::GET{return self.validate_option_form(object);}
        if method==script_option_index::GET{return self.validate_option_index(object);}
        if method==script_textarea_text_length::GET{return self.validate_textarea_text_length(object);}
        if method==script_map_areas::GET{return self.validate_map_areas(object);}
        if method==script_datalist_options::GET{return self.validate_datalist_options(object);}
        if script_time_datetime::operation(method){return self.validate_time_datetime(object);}
        if script_area_alt::operation(method){return self.validate_area_alt(object);}
        if script_optgroup_label::operation(method){return self.validate_optgroup_label(object);}
        if script_option_label::operation(method){return self.validate_option_label(object);}
        if script_option_text::operation(method){return self.validate_option_text(object);}
        if script_textarea_default_value::operation(method){return self.validate_textarea_default_value(object);}
        if script_option_value::operation(method){return self.validate_option_value(object);}
        if script_select_name::operation(method){return self.validate_select_name(object);}
        if script_textarea_name::operation(method){return self.validate_textarea_name(object);}
        if script_area_coords::operation(method){return self.validate_area_coords(object);}
        if script_area_shape::operation(method){return self.validate_area_shape(object);}
        if script_area_download::operation(method){return self.validate_area_download(object);}
        if script_area_type::operation(method){return self.validate_area_type(object);}
        if script_area_hreflang::operation(method){return self.validate_area_hreflang(object);}
        if script_area_target::operation(method){return self.validate_area_target(object);}
        if script_area_rel::operation(method){return self.validate_area_rel(object);}
        if script_area_href::operation(method){return self.validate_area_href(object);}
        if script_hyperlink_stringifier::operation(method){return self.validate_hyperlink_stringifier(object,method);}
        if script_area_url::operation(method).is_some(){return self.validate_area_href(object);}
        if script_area_referrer_policy::operation(method){return self.validate_area_referrer_policy(object);}
        if script_area_ping::operation(method){return self.validate_area_ping(object);}
        if script_map_name::operation(method){return self.validate_map_name(object);}
        if script_param_name::operation(method){return self.validate_param_name(object);}
        if script_param_metadata::operation(method){return self.validate_param_name(object);}
        if script_mod_datetime::operation(method){return self.validate_mod_datetime(object);}
        if script_data_value::operation(method){return self.validate_data_value(object);}
        if script_heading_align::operation(method){return self.validate_heading_align(object);}
        if script_ul_type::operation(method){return self.validate_ul_type(object);}
        if script_li_type::operation(method){return self.validate_li_type(object);}
        if script_ol_type::operation(method){return self.validate_ol_type(object);}
        if script_input_autocomplete::operation(method){return self.validate_input_autocomplete(object);}
        if script_input_form_action::operation(method){return self.validate_input_form_action(object);}
        if script_input_form_method::operation(method){return self.validate_input_form_method(object);}
        if script_input_form_enctype::operation(method){return self.validate_input_form_enctype(object);}
        if script_button_form_action::operation(method){return self.validate_button_form_action(object);}
        if script_button_form_method::operation(method){return self.validate_button_form_method(object);}
        if script_button_form_enctype::operation(method){return self.validate_button_form_enctype(object);}
        if script_form_autocomplete::operation(method){return self.validate_form_autocomplete(object);}
        if script_form_method::operation(method){return self.validate_form_method(object);}
        if script_form_action::operation(method){return self.validate_form_action(object);}
        if script_form_enctype::operation(method){return self.validate_form_enctype(object);}
        if script_form_name::operation(method){return self.validate_form_name(object);}
        if script_button_name::operation(method){return self.validate_button_name(object);}
        if script_input_name::operation(method){return self.validate_input_name(object);}
        if script_form_accept_charset::operation(method){return self.validate_form_accept_charset(object);}
        if script_form_rel::operation(method){return self.validate_form_rel(object);}
        if script_form_target::operation(method){return self.validate_form_target(object);}
        if script_button_form_target::operation(method){return self.validate_button_form_target(object);}
        if script_input_form_target::operation(method){return self.validate_input_form_target(object);}
        if script_form_no_validate::operation(method){return self.validate_form_no_validate(object);}
        if script_button_form_no_validate::operation(method){return self.validate_button_form_no_validate(object);}
        if script_input_form_no_validate::operation(method){return self.validate_input_form_no_validate(object);}
        if script_select_autocomplete::operation(method){return self.validate_select_autocomplete(object);}
        if script_textarea_autocomplete::operation(method){return self.validate_textarea_autocomplete(object);}
        if script_textarea_wrap::operation(method){return self.validate_textarea_wrap(object);}
        if script_button_value::operation(method){return self.validate_button_value(object);}
        if script_button_type::operation(method){return self.validate_button_type(object);}
        if script_input_step::operation(method){return self.validate_input_step(object);}
        if script_input_max::operation(method){return self.validate_input_max(object);}
        if script_input_min::operation(method){return self.validate_input_min(object);}
        if script_input_pattern::operation(method){return self.validate_input_pattern(object);}
        if script_li_value::operation(method){return self.validate_li_value(object);}
        if script_pre_width::operation(method){return self.validate_pre_width(object);}
        if script_textarea_dimensions::operation(method).is_some(){return self.validate_textarea_dimensions(object);}
        if script_ol_start::operation(method){return self.validate_ol_start(object);}
        if script_textarea_min_length::operation(method){return self.validate_textarea_min_length(object);}
        if script_input_min_length::operation(method){return self.validate_input_min_length(object);}
        if script_textarea_max_length::operation(method){return self.validate_textarea_max_length(object);}
        if script_input_max_length::operation(method){return self.validate_input_max_length(object);}
        if script_ul_compact::operation(method){return self.validate_ul_compact(object);}
        if script_dir_compact::operation(method){return self.validate_dir_compact(object);}
        if script_menu_compact::operation(method){return self.validate_menu_compact(object);}
        if script_ol_compact::operation(method){return self.validate_ol_compact(object);}
        if script_hr_no_shade::operation(method){return self.validate_hr_no_shade(object);}
        if script_area_no_href::operation(method){return self.validate_area_no_href(object);}
        if script_option_disabled::operation(method){return self.validate_option_disabled(object);}
        if script_select_disabled::operation(method){return self.validate_select_disabled(object);}
        if script_button_disabled::operation(method){return self.validate_button_disabled(object);}
        if script_input_disabled::operation(method){return self.validate_input_disabled(object);}
        if script_textarea_disabled::operation(method){return self.validate_textarea_disabled(object);}
        if script_textarea_required::operation(method){return self.validate_textarea_required(object);}
        if script_input_required::operation(method){return self.validate_input_required(object);}
        if script_textarea_read_only::operation(method){return self.validate_textarea_read_only(object);}
        if script_input_read_only::operation(method){return self.validate_input_read_only(object);}
        if script_select_required::operation(method){return self.validate_select_required(object);}
        if script_option_default_selected::operation(method){return self.validate_option_default_selected(object);}
        if script_optgroup_disabled::operation(method){return self.validate_optgroup_disabled(object);}
        if script_ol_reversed::operation(method){return self.validate_ol_reversed(object);}
        if script_dl_compact::operation(method){return self.validate_dl_compact(object);}
        if script_paragraph_align::operation(method){return self.validate_paragraph_align(object);}
        if script_hr_align::operation(method){return self.validate_hr_align(object);}
        if script_hr_color::operation(method){return self.validate_hr_color(object);}
        if script_font_size::operation(method){return self.validate_font_size(object);}
        if script_font_face::operation(method){return self.validate_font_face(object);}
        if script_font_color::operation(method){return self.validate_font_color(object);}
        if script_br_clear::operation(method){return self.validate_br_clear(object);}
        if script_hr_size::operation(method){return self.validate_hr_size(object);}
        if script_hr_width::operation(method){return self.validate_hr_width(object);}
        if script_legend_align::operation(method){return self.validate_legend_align(object);}
        if script_div_align::operation(method){return self.validate_div_align(object);}
        if script_fieldset::operation(method){return self.validate_fieldset(object);}
        if script_control_form::interface(method).is_some(){return self.validate_control_form(object,method);}
        if script_output_metadata::operation(method){return self.validate_output_metadata(object);}
        if let Some(class)=script_control_eligibility::interface(method){return self.validate_control_eligibility(object,class);}
        if script_fieldset_validation::operation(method){return self.validate_fieldset(object);}
        if script_output_validation::operation(method){return self.validate_output_metadata(object);}
        if script_output_validity_state::operation(method){return self.validate_output_validity(object,method);}
        if script_output_value::operation(method).is_some(){return self.validate_output_value(object);}
        if method==script_label_control::GET{return self.validate_html_label_control(object);}
        if script_label_html_for::operation(method){return self.validate_html_label_for(object,method==script_label_html_for::SET);}
        if script_element_class_list::operation(method){return self.validate_element_class_list(object);}
        if script_output_html_for::operation(method){return self.validate_output_html_for(object);}
        if script_area_rel_list::operation(method){return self.validate_area_rel_list(object);}
        if script_form_rel_list::operation(method){return self.validate_form_rel_list(object);}
        if anchor_url_interface::operation(method).is_some() {return self.native_node_access(object);}
        if crate::javascript::child_remove_operation(method) {
            self.native_node_access(object)?;
            if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child document mutation requires its lifecycle hooks.".into()));}
            return Ok(());
        }
        if let Some((interface,_)) = charset_operation(method) { return self.validate_charset(object,interface); }
        if meta_operation(method).is_some() { return self.validate_meta(object); }
        if let Some((_,_,setter))=crate::javascript::aria_reflection_interface::operation(method){return self.validate_aria_reflection(object,setter);}
        if let Some((_,setter))=crate::javascript::html_reflection_interface::operation(method){return self.validate_html_reflection(object,setter);}
        if script_content_editable::accessor(method) {return self.validate_content_editable(object);}
        if matches!(method, ELEMENT_STYLE_GET | ELEMENT_STYLE_SET |SVG_STYLE_GET|SVG_STYLE_SET) {
            if !self.style_interface_receiver(object,matches!(method,SVG_STYLE_GET|SVG_STYLE_SET)) {
                return Err(("TypeError", "The style accessor requires its genuine HTML or SVG interface receiver.".into()));
            }
            if self.is_frame_handle(object)||self.inert_node_type(object).is_some() {
                return Err(("NotSupportedError", "Inline CSS declarations on child document nodes are unavailable.".into()));
            }
            return Ok(());
        }
        if let Some(operation) = CanvasOperation::parse(method) {
            return if operation.accepts(self, object) { Ok(()) }
                else { Err(("TypeError", "Canvas operation requires its genuine native receiver.".into())) };
        }
        if object==WINDOW && method=="getComputedStyle" || script_computed_style::index(object).is_some() {return self.validate_computed_call(object,method,args);}
        if script_iframe_sandbox::accessor(method).is_some() {return self.validate_iframe_sandbox(object);}
        if script_textarea_interface::accessor(method).is_some() {return self.validate_textarea_value(object);}
        if script_input_accessors::accessor(method).is_some() {return self.validate_input_accessor(object);}
        if script_canvas::accessor(method).is_some() {return self.validate_canvas_accessor(object);}
        if let Some(offset) = Offset::getter(method) {return self.validate_html_offset(object, offset);}
        if script_scroll_offsets::accessor(method).is_some() {return self.validate_scroll_element(object);}
        if matches!(method,"ValidityState"|"NodeList"|"HTMLCollection"|"NamedNodeMap"|"DOMTokenList"|"CanvasRenderingContext2D") && self.event_target_interface(object) {return Err(("TypeError","Illegal constructor for this native interface.".into()));}
        if matches!(method,"\0HTMLCollection.get.length"|"\0HTMLCollection.item"|"\0HTMLCollection.namedItem")&&!self.collection_receiver(object){return Err(("TypeError","HTMLCollection operation requires a genuine native collection.".into()));}
        if matches!(method,"\0HTMLCollection.item"|"\0HTMLCollection.namedItem")&&args.is_empty(){return Err(("TypeError","HTMLCollection method requires one argument.".into()));}
        if script_named_map::operation(method).is_some()&&!self.named_map_receiver(object){return Err(("TypeError","NamedNodeMap operation requires a genuine map.".into()));}
        if matches!(method,"\0NodeList.get.length"|"\0NodeList.item")&&!self.node_list_receiver(object) {return Err(("TypeError","NodeList operation requires a genuine accessible NodeList.".into()));}
        if method=="\0NodeList.item"&&args.is_empty(){return Err(("TypeError","NodeList item requires one argument.".into()));}
        if method=="setAttribute"&&self.inert_node_type(object)==Some(1){if let Some(name)=args.first(){if !script_attribute_names::valid_attribute_name_dom(script_attribute_names::converted_name(name)?){return Err(("InvalidCharacterError","Invalid attribute local name.".into()));}}}
        if parent_children::operation(method).is_some() && !self.parent_node_receiver(object) {
            return Err(("TypeError","ParentNode getter requires a genuine accessible parent node.".into()));
        }
        let method=if let Some(operation)=script_element_selectors::operation(method) {
            self.validate_selector_operation_receiver(object,method)?;
            operation
        } else {method};
        if self.event_target_interface(object) && matches!(method, "Text" | "Comment") {
            return if object == WINDOW { Ok(()) } else { Err(("NotSupportedError", format!("Child {method} allocation is not implemented."))) };
        }
        if self.event_target_interface(object) && method=="DocumentFragment" {
            return if object==WINDOW {Ok(())} else {Err(("NotSupportedError","Child DocumentFragment allocation is not implemented.".into()))};
        }
        if self.event_target_interface(object)&&method=="Document"{return if object==WINDOW{Ok(())}else{Err(("NotSupportedError","Child-realm Document construction requires its actual realm ownership; no object was created.".into()))};}
        if method == DOCUMENT_ADOPT_NODE{return self.validate_node_adoption(object,args);}
        if method==DOCUMENT_IMPORT_NODE{return self.validate_import_source(args);}
        if method==CREATE_CDATA_SECTION{return self.validate_cdata_factory(object,args);}
        if method == CREATE_PROCESSING_INSTRUCTION || object == WINDOW && method == "ProcessingInstruction" {
            let valid_target=args.first().and_then(|target|script_attribute_names::converted_name(target).ok()).is_some_and(script_attribute_names::valid_xml_name);
            let invalid_data=match args.get(1){Some(Value::String(data))=>data.contains("?>"),Some(Value::StringUnits(data))=>data.windows(2).any(|units|units==[0x3f,0x3e]),_=>false};
            return if valid_target && !invalid_data { Ok(()) } else {
                Err(("InvalidCharacterError", "ProcessingInstruction requires a valid XML name and data without ?>.".into()))
            };
        }
        if method==character_substring::METHOD{return self.validate_substring_data(object,args);}
        if method==text_whole::GETTER{return if self.text_node_receiver(object){Ok(())}else{Err(("TypeError","wholeText requires a genuine Text or CDATA node.".into()))};}
        if method==text_split_interface::METHOD{return self.validate_text_split(object,args);}
        if method==node_normalize_interface::METHOD{return if self.normalize_receiver(object){Ok(())}else{Err(("TypeError","normalize requires a genuine supported Node.".into()))};}
        if character_data_interface::operation(method).is_some(){return self.validate_character_edit(object,method,args);}
        if method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME{return self.validate_name_list(object);}
        if let Some(document)=script_inert_documents::identifier_lookup::operation(method){return self.validate_identifier_lookup(object,document);}
        if method==script_position::METHOD{return self.validate_node_position(object,args);}
        if script_inert_documents::node_equality::operation(method){return self.validate_node_equality(object,args);}
        if method==script_node_root::ROOT_METHOD {return self.validate_root_node_options(object,args);}
        if method == script_clone_node::CLONE_METHOD { self.validate_clone_receiver(object)?;return self.validate_document_clone(object,args.first().is_some_and(Value::truthy)); }
        if method == script_node_contains::CONTAINS_METHOD { return self.validate_node_contains(object, args); }
        let method=script_media::interface_operation(method).unwrap_or(method);
        if matches!(method,"load"|"pause")&&self.media_interface_receiver(object)&&!self.media_element_receiver(object) {
            return Err(("NotSupportedError","This media element has no implemented player binding.".into()));
        }
        if matches!(method,"Event"|"UIEvent"|"MouseEvent"|"WheelEvent"|"KeyboardEvent"|"FocusEvent"|"SubmitEvent"|"ProgressEvent"|"AnimationEvent"|"ErrorEvent"|"CustomEvent")&&self.event_target_interface(object) {
            if !self.state.enabled||!self.doc.scripting_enabled {return Err(("InvalidStateError","Event construction is disabled.".into()));}
            if args.get(1).is_some_and(|value|!matches!(value,Value::Undefined|Value::Null|Value::Object(_)|Value::Function(_)|Value::Host(_))) {
                return Err(("TypeError",format!("{method}Init requires an object, null or undefined.")));
            }
            return Ok(());
        }
        if method.starts_with("\0Event.")&&!self.event_receiver(object) {
            return Err(("TypeError","Event operation requires a genuine Event.".into()));
        }
        if method.starts_with("\0CustomEvent.")&&!self.custom_receiver(object) {
            return Err(("TypeError","CustomEvent operation requires a genuine CustomEvent.".into()));
        }
        if method.starts_with("\0UIEvent.")&&!self.ui_receiver(object) {
            return Err(("TypeError","UIEvent operation requires a genuine UIEvent.".into()));
        }
        if method.starts_with("\0WheelEvent.")&&!self.wheel_receiver(object) {return Err(("TypeError","WheelEvent operation requires a genuine WheelEvent.".into()));}
        if method.starts_with("\0ErrorEvent.")&&!self.error_event_receiver(object) {return Err(("TypeError","ErrorEvent operation requires a genuine ErrorEvent.".into()));}
        if method.starts_with("\0AnimationEvent.")&&!self.animation_receiver(object) {return Err(("TypeError","AnimationEvent operation requires a genuine AnimationEvent.".into()));}
        if method.starts_with("\0ProgressEvent.")&&!self.progress_receiver(object) {return Err(("TypeError","ProgressEvent operation requires a genuine ProgressEvent.".into()));}
        if method.starts_with("\0FocusEvent.")&&!self.focus_receiver(object) {return Err(("TypeError","FocusEvent operation requires a genuine FocusEvent.".into()));}
        if method.starts_with("\0SubmitEvent.")&&!self.submit_event_receiver(object) {return Err(("TypeError","SubmitEvent operation requires a genuine SubmitEvent.".into()));}
        if method.starts_with("\0KeyboardEvent.")&&!self.keyboard_receiver(object) {
            return Err(("TypeError","KeyboardEvent operation requires a genuine KeyboardEvent.".into()));
        }
        if method.starts_with("\0MouseEvent.")&&!self.mouse_receiver(object) {
            return Err(("TypeError","MouseEvent operation requires a genuine MouseEvent.".into()));
        }
        if method=="pause" && self.media_element_receiver(object) && (!self.state.enabled || !self.doc.scripting_enabled) {return Err(("InvalidStateError","Scripting is disabled.".into()));}
        if object==WINDOW && method=="requestAnimationFrame" && !matches!(args.first(),Some(Value::Function(_))) {return Err(("TypeError","requestAnimationFrame requires a callable callback.".into()));}
        if self.event_target_interface(object)
            && (matches!(method,"Window"|"Node"|"Attr"|"Element"|"HTMLElement"|"SVGElement"|"CharacterData"|"CDATASection"|"HTMLMediaElement"|"HTMLVideoElement")
                || crate::javascript::ORDINARY_HTML_SUBCLASSES.iter().any(|&(_, name)| name == method))
        {
            return Err(("TypeError","Illegal constructor for this native interface.".into()));
        }
        if method=="dispatchEvent"{return self.validate_dispatch(object,args);}
        if matches!(method, "addEventListener" | "removeEventListener") {
            if !self.dispatch_receiver(object) {
                return Err(("TypeError", "Listener receiver must be a supported EventTarget.".into()));
            }
            if !matches!(args.get(1), Some(Value::Object(_) | Value::Function(_) | Value::Host(_) | Value::Null | Value::Undefined)) {
                return Err(("TypeError", "An EventListener callback must be an object or null.".into()));
            }
        }
        if object == HISTORY && matches!(method,"replaceState"|"pushState") {
            self.validate_history_active()?;
            if matches!(args.first(), Some(Value::Function(_) | Value::Symbol(_))) { return Err(("DataCloneError", "This History value cannot be serialized for storage.".into())); }
            if matches!(args.first(), Some(Value::Object(_) | Value::Host(_))) { return Ok(()); }
            self.history_target(args)?;
            return Ok(());
        }
        if method=="getElementsByClassName" && !(object==DOCUMENT || self.attr_element_receiver(object) || matches!(self.inert_node_type(object),Some(1|9))) {
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
        if (object == DOCUMENT || node_index(object).is_some() || matches!(self.inert_node_type(object),Some(1|9|11)))
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
        if (object == DOCUMENT && method == "getElementById"
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
        if self.time_datetime_member(object,property){return self.validate_time_datetime(object);}
        if self.area_alt_member(object,property){return self.validate_area_alt(object);}
        if self.optgroup_label_member(object,property){return self.validate_optgroup_label(object);}
        if self.option_label_member(object,property){return self.validate_option_label(object);}
        if self.option_text_member(object,property){return self.validate_option_text(object);}
        if self.textarea_default_value_member(object,property){return self.validate_textarea_default_value(object);}
        if self.option_value_member(object,property){return self.validate_option_value(object);}
        if self.select_name_member(object,property){return self.validate_select_name(object);}
        if self.textarea_name_member(object,property){return self.validate_textarea_name(object);}
        if self.area_coords_member(object,property){return self.validate_area_coords(object);}
        if self.area_shape_member(object,property){return self.validate_area_shape(object);}
        if self.area_download_member(object,property){return self.validate_area_download(object);}
        if self.area_type_member(object,property){return self.validate_area_type(object);}
        if self.area_hreflang_member(object,property){return self.validate_area_hreflang(object);}
        if self.area_target_member(object,property){return self.validate_area_target(object);}
        if self.area_rel_member(object,property){return self.validate_area_rel(object);}
        if self.area_href_member(object,property){return self.validate_area_href(object);}
        if self.area_url_member(object,property){return self.validate_area_href(object);}
        if self.area_referrer_policy_member(object,property){return self.validate_area_referrer_policy(object);}
        if self.area_ping_member(object,property){return self.validate_area_ping(object);}
        if self.map_name_member(object,property){return self.validate_map_name(object);}
        if self.param_name_member(object,property){return self.validate_param_name(object);}
        if self.param_metadata_member(object,property){return self.validate_param_name(object);}
        if self.mod_datetime_member(object,property){return self.validate_mod_datetime(object);}
        if self.data_value_member(object,property){return self.validate_data_value(object);}
        if self.heading_align_member(object,property){return self.validate_heading_align(object);}
        if self.ul_type_member(object,property){return self.validate_ul_type(object);}
        if self.li_type_member(object,property){return self.validate_li_type(object);}
        if self.ol_type_member(object,property){return self.validate_ol_type(object);}
        if self.input_autocomplete_member(object,property){return self.validate_input_autocomplete(object);}
        if self.input_form_action_member(object,property){return self.validate_input_form_action(object);}
        if self.input_form_method_member(object,property){return self.validate_input_form_method(object);}
        if self.input_form_enctype_member(object,property){return self.validate_input_form_enctype(object);}
        if self.button_form_action_member(object,property){return self.validate_button_form_action(object);}
        if self.button_form_method_member(object,property){return self.validate_button_form_method(object);}
        if self.button_form_enctype_member(object,property){return self.validate_button_form_enctype(object);}
        if property=="autocomplete"&&self.form_autocomplete_receiver(object){return self.validate_form_autocomplete(object);}
        if property=="method"&&self.form_method_receiver(object){return self.validate_form_method(object);}
        if property=="action"&&self.form_action_receiver(object){return self.validate_form_action(object);}
        if matches!(property,"enctype"|"encoding")&&self.form_enctype_receiver(object){return self.validate_form_enctype(object);}
        if property=="name"&&self.form_name_receiver(object){return self.validate_form_name(object);}
        if property=="name"&&self.button_name_receiver(object){return self.validate_button_name(object);}
        if property=="name"&&self.input_name_receiver(object){return self.validate_input_name(object);}
        if property=="acceptCharset"&&self.form_accept_charset_receiver(object){return self.validate_form_accept_charset(object);}
        if property=="rel"&&self.form_rel_receiver(object){return self.validate_form_rel(object);}
        if property=="target"&&self.form_target_receiver(object){return self.validate_form_target(object);}
        if property=="formTarget"&&self.button_form_target_receiver(object){return self.validate_button_form_target(object);}
        if property=="formTarget"&&self.input_form_target_receiver(object){return self.validate_input_form_target(object);}
        if property=="noValidate"&&self.form_no_validate_receiver(object){return self.validate_form_no_validate(object);}
        if property=="formNoValidate"&&self.button_form_no_validate_receiver(object){return self.validate_button_form_no_validate(object);}
        if property=="formNoValidate"&&self.input_form_no_validate_receiver(object){return self.validate_input_form_no_validate(object);}
        if self.select_autocomplete_member(object,property){return self.validate_select_autocomplete(object);}
        if self.textarea_autocomplete_member(object,property){return self.validate_textarea_autocomplete(object);}
        if self.textarea_wrap_member(object,property){return self.validate_textarea_wrap(object);}
        if self.button_value_member(object,property){return self.validate_button_value(object);}
        if self.button_type_member(object,property){return self.validate_button_type(object);}
        if self.input_step_member(object,property){return self.validate_input_step(object);}
        if self.input_max_member(object,property){return self.validate_input_max(object);}
        if self.input_min_member(object,property){return self.validate_input_min(object);}
        if self.input_pattern_member(object,property){return self.validate_input_pattern(object);}
        if self.li_value_member(object,property){return self.validate_li_value(object);}
        if self.pre_width_member(object,property){return self.validate_pre_width(object);}
        if self.textarea_dimensions_member(object,property){return self.validate_textarea_dimensions(object);}
        if self.ol_start_member(object,property){return self.validate_ol_start(object);}
        if self.textarea_min_length_member(object,property){return self.validate_textarea_min_length_assignment(object,value);}
        if self.input_min_length_member(object,property){return self.validate_input_min_length_assignment(object,value);}
        if self.textarea_max_length_member(object,property){return self.validate_textarea_max_length_assignment(object,value);}
        if self.input_max_length_member(object,property){return self.validate_input_max_length_assignment(object,value);}
        if self.ul_compact_member(object,property){return self.validate_ul_compact(object);}
        if self.dir_compact_member(object,property){return self.validate_dir_compact(object);}
        if self.menu_compact_member(object,property){return self.validate_menu_compact(object);}
        if self.ol_compact_member(object,property){return self.validate_ol_compact(object);}
        if self.hr_no_shade_member(object,property){return self.validate_hr_no_shade(object);}
        if self.area_no_href_member(object,property){return self.validate_area_no_href(object);}
        if self.option_disabled_member(object,property){return self.validate_option_disabled(object);}
        if self.select_disabled_member(object,property){return self.validate_select_disabled(object);}
        if self.button_disabled_member(object,property){return self.validate_button_disabled(object);}
        if self.input_disabled_member(object,property){return self.validate_input_disabled(object);}
        if self.textarea_disabled_member(object,property){return self.validate_textarea_disabled(object);}
        if self.textarea_required_member(object,property){return self.validate_textarea_required(object);}
        if self.input_required_member(object,property){return self.validate_input_required(object);}
        if self.textarea_read_only_member(object,property){return self.validate_textarea_read_only(object);}
        if self.input_read_only_member(object,property){return self.validate_input_read_only(object);}
        if self.select_required_member(object,property){return self.validate_select_required(object);}
        if self.option_default_selected_member(object,property){return self.validate_option_default_selected(object);}
        if self.optgroup_disabled_member(object,property){return self.validate_optgroup_disabled(object);}
        if self.ol_reversed_member(object,property){return self.validate_ol_reversed(object);}
        if self.dl_compact_member(object,property){return self.validate_dl_compact(object);}
        if self.paragraph_align_member(object,property){return self.validate_paragraph_align(object);}
        if self.hr_align_member(object,property){return self.validate_hr_align(object);}
        if self.hr_color_member(object,property){return self.validate_hr_color(object);}
        if self.font_size_member(object,property){return self.validate_font_size(object);}
        if self.font_face_member(object,property){return self.validate_font_face(object);}
        if self.font_color_member(object,property){return self.validate_font_color(object);}
        if self.br_clear_member(object,property){return self.validate_br_clear(object);}
        if self.hr_size_member(object,property){return self.validate_hr_size(object);}
        if self.hr_width_member(object,property){return self.validate_hr_width(object);}
        if self.legend_align_member(object,property){return self.validate_legend_align(object);}
        if self.div_align_member(object,property){return self.validate_div_align(object);}
        if self.fieldset_name_member(object,property)||self.fieldset_disabled_member(object,property){return self.validate_fieldset(object);}
        if self.xhr_index(object).is_some(){return self.xhr_validate_set(object,property,value);}

        if property == "nodeValue" && self.text_content_receiver(object) { self.native_node_access(object)?; }
        if self.charset_member(object, property) { return self.validate_charset(object,self.charset_interface(object).unwrap()); }
        if self.meta_member(object, property) { return self.validate_meta(object); }
        if property=="id"&&self.canonical_selector_receiver(object){return self.validate_element_id(object,true);}
        if property=="className"&&self.canonical_selector_receiver(object){return self.validate_element_class_name(object,true);}
        if self.aria_reflection_attribute(object,property){return self.validate_aria_reflection(object,true);}
        if crate::javascript::html_reflection_interface::property(property).is_some() && self.html_reflection_receiver(object){return self.validate_html_reflection(object,true);}
        if self.content_editable_attribute(object,property) {return self.validate_content_editable_value(object,value);}
        if self.canvas_context_receiver(object) && script_canvas_context::unsupported_state(property) {
            return Err(("NotSupportedError", format!("Canvas 2D drawing state {property:?} is not implemented.")));
        }
        if self.meter_numeric_attribute(object,property).is_some(){self.validate_meter_numeric(object)?;if !matches!(value,Value::Number(n)if n.is_finite()){return Err(("TypeError","Meter numeric metadata requires finite doubles.".into()));}return Ok(());}
        if self.progress_numeric_attribute(object,property).is_some(){self.validate_progress_numeric(object)?;if !matches!(value,Value::Number(n)if n.is_finite()){return Err(("TypeError","Progress numeric metadata requires finite doubles.".into()));}return Ok(());}
        if property=="currentTime" && self.media_interface_receiver(object) {
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
                        node.kind.character_data().is_some() && matches!(property, "data" | "nodeValue")
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

    fn setter_dom_exception(&self,object:u32,property:&str,kind:&str)->bool {
        if self.textarea_min_length_member(object,property)&&kind=="IndexSizeError"{return true;}
        if self.input_min_length_member(object,property)&&kind=="IndexSizeError"{return true;}
        if self.textarea_max_length_member(object,property)&&kind=="IndexSizeError"{return true;}
        if self.input_max_length_member(object,property)&&kind=="IndexSizeError"{return true;}
        self.xhr_index(object).is_some()&&!matches!(kind,"TypeError"|"RangeError") || kind=="SyntaxError" && self.content_editable_attribute(object,property)
    }

    fn resolve_member_before_call(&self, object: u32) -> bool {
        if self.collection_receiver(object){return true;}
        if self.canvas_context_receiver(object) {return true;}
        if self.event_receiver(object) {return true;}
        if self.standalone_target_owner(object).is_some() || self.abort_instance(object).is_some() {return true;}
        if object == HISTORY || self.attr_receiver(object) || self.constructed_event_receiver(object) || self.named_map_receiver(object) || self.media_element_receiver(object) { return true; }
        self.form_receiver(object)
    }

    fn ordinary_prototype(&self, object: u32) -> bool {
        // NodeLists now have a genuine ordinary backing/canonical prototype;
        // the legacy dataset-style descriptor route must not classify them.
        (DATASET_BASE..EVENT_BASE).contains(&object)
    }

    fn promise_resolution_receiver(&self, object: u32) -> bool {
        if self.validity_receiver(object){return true;}
        // Promise resolution performs only the normal `then` property lookup.
        // Authorize this protocol explicitly for this realm's Window, Document
        // and real DOM nodes; other native capabilities keep the default deny.
        matches!(object, WINDOW | DOCUMENT)
            || self.canvas_context_receiver(object)
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
        if self.canonical_attribute_node_member(object,property)||script_attr::node_method(property)&&self.form_named_control(object,property){return None;}
        if self.canonical_attribute_factory_member(object,property){return None;}
        if matches!(property,"getElementsByTagName"|"getElementsByTagNameNS"|"getElementsByClassName")&&self.form_named_control(object,property){return None;}
        if self.canonical_collection_member(object,property)||self.canonical_name_list_member(object,property){return None;}
        if object==CONSOLE {return match property {"log"=>Some("log"),"info"=>Some("info"),"warn"=>Some("warn"),"error"=>Some("error"),_=>None};}
        if self.xhr_index(object).is_some(){return None;}

        if self.attribute_method_member(object,property){return None;}
        if self.inert_interface(object).is_some() { return self.inert_factory(object, property); }
        if script_inline_style::handle(object) {
            return self.attr_element_receiver(node_handle((object - STYLE_BASE) as usize))
                .then(|| script_inline_style::method(property)).flatten();
        }
        if self.computed_style_receiver(object) {return match property {"getPropertyValue"=>Some("getPropertyValue"),"setProperty"=>Some("setProperty"),"removeProperty"=>Some("removeProperty"),_=>None};}
        if self.node_list_iteration_member(object,property) {return None;}
        // Named form controls use the native field path; other genuine
        // Elements use the canonical prototype. Neither may fall through to
        // the old per-instance selector method factory below.
        if script_element_selectors::member(property) && self.canonical_selector_receiver(object) {return None;}
        if object==DOCUMENT && property=="createDocumentFragment" {return Some("createDocumentFragment");}
        if self.node_tree_method_member(object,property) {return None;}
        if self.media_element_receiver(object) {match property {"load"=>return Some("load"),"pause"=>return Some("pause"),"play"=>return Some("play"),_=>{}}}
        if matches!(property,"addEventListener"|"removeEventListener"|"dispatchEvent") && self.form_named_control(object,property) {return None;}
        if self.node_event_method(object,property) {return None;}
        if property=="dispatchEvent"&&self.dispatch_receiver(object)&&!self.form_named_control(object,property){return Some("dispatchEvent");}
        if self.named_map_receiver(object){return None;}
        if property=="getElementsByTagName"&&matches!(self.inert_node_type(object),Some(1|9)){return Some("getElementsByTagName");}
        if property=="normalize" && !self.is_frame_handle(object) && self.namespace_receiver(object) {return Some("normalize");}
        if property=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object) || matches!(self.inert_node_type(object),Some(1|9)))
            && !self.form_named_control(object,property) { return Some("getElementsByClassName"); }
        if script_class_list::element(object).is_some() {return None;}
        if object == DOCUMENT && property == "createEvent" && self.event_factory_enabled() { return Some("createEvent"); }
        if let Some(method) = self.created_event_method(object, property) { return Some(method); }
        // Form submission is a canonical prototype operation; native named
        // controls still resolve before the prototype through form lookup.
        if property == "submit" && self.form_receiver(object) { return None; }
        if self.collection_receiver(object){return None;}
        if let Some(method) = self.dom_method(object, property) {
            return Some(method);
        }
        if node_list_index(object).is_some() {return None;}
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
        if script_class_list::operation(method).is_some(){return self.event_target_interface(object)&&self.class_list_receiver(receiver);}
        if script_attr::node_method(method){return self.event_target_interface(object)&&(self.attr_element_receiver(receiver)||self.inert_node_type(receiver)==Some(1));}
        if script_attr::member_operation(method).is_some(){return self.event_target_interface(object)&&self.attr_receiver(receiver);}
        if script_attr::factory_operation(method).is_some(){return self.event_target_interface(object)&&(receiver==DOCUMENT||self.inert_document(receiver));}
        if method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME{return self.event_target_interface(object)&&self.identifier_lookup_receiver(receiver,Some(true));}
        if let Some((_,document))=dom_collections::operation(method){return self.event_target_interface(object)&&self.collection_interface_receiver(receiver)==Some(document);}
        if element_names::namespace_method(method){return self.event_target_interface(object)&&self.namespace_receiver(receiver);}
        if method==character_substring::METHOD{return self.event_target_interface(object)&&self.character_data_receiver(receiver);}
        if method==text_whole::GETTER||method==text_split_interface::METHOD{return self.event_target_interface(object)&&self.text_node_receiver(receiver);}
        if method==node_normalize_interface::METHOD{return self.event_target_interface(object)&&self.normalize_receiver(receiver);}
        if character_data_interface::operation(method).is_some(){return self.event_target_interface(object)&&self.character_data_receiver(receiver);}
        if let Some((interface,_))=placeholder::operation(method){if self.event_target_interface(object){return self.placeholder_interface(receiver)==Some(interface);}}
        if self.event_target_interface(object)&&method.starts_with("\0XHR.") {
            let Some((_,upload))=self.xhr_index(receiver) else{return false};
            let name=crate::javascript::xhr_interface::getter(method).or_else(||crate::javascript::xhr_interface::setter(method));
            return !upload||name.is_some_and(|name|self.xhr_handler_event(receiver,name).is_some());
        }

        if script_iframe_sandbox::accessor(method).is_some() && self.event_target_interface(object) {return self.iframe_sandbox_receiver(receiver)||self.inert_interface(receiver)==Some("HTMLIFrameElement");}
        if script_textarea_interface::accessor(method).is_some() && self.event_target_interface(object) {return self.textarea_value_receiver(receiver) || self.inert_interface(receiver)==Some("HTMLTextAreaElement");}
        if matches!(method,crate::javascript::HTML_FORM_RESET|crate::javascript::HTML_FORM_CHECK|crate::javascript::HTML_FORM_REPORT|crate::javascript::HTML_FORM_REQUEST){return self.form_length_receiver(receiver);}
        if let Some(class)=crate::javascript::control_custom_validity_interface(method){return self.event_target_interface(object)&&self.control_eligibility_class(receiver)==Some(class);}
        if let Some((class,_))=crate::javascript::control_validation_operation(method){return self.event_target_interface(object)&&self.control_eligibility_class(receiver)==Some(class);}
        if method==crate::javascript::HTML_FORM_SUBMIT {return self.form_receiver(receiver)&&self.class_list_element_receiver(receiver) || self.inert_interface(receiver)==Some("HTMLFormElement");}
        if crate::javascript::range_interface::operation(method) { return if method==crate::javascript::range_interface::CREATE { self.document_interface_receiver(receiver) } else { self.range_record(receiver).is_some_and(|record|!record.fixed||crate::javascript::range_interface::snapshot_getter(method)) }; }
        if matches!(method, MEDIA_QUERY_MATCHES | MEDIA_QUERY_MEDIA) {
            return object==WINDOW && self.media_query_owner(receiver).is_some();
        }
        if crate::javascript::message_channel_interface::operation(method) {
            return object == WINDOW && self.matches_message_receiver(receiver, method);
        }
        if self.event_target_interface(object) {
            if matches!(method, CREATE_CDATA_SECTION | CREATE_PROCESSING_INSTRUCTION | script_inert_documents::GET) {
                return receiver == DOCUMENT || self.inert_document(receiver);
            }
            if crate::javascript::DOCUMENT_TYPE_GETTERS.iter().any(|(_,operation)|*operation==method){return self.doctype_receiver(receiver);}
            if method == PROCESSING_INSTRUCTION_TARGET_GET {
                return self.processing_instruction_receiver(receiver);
            }
            if method == HTML_TEMPLATE_CONTENT_GET { return self.template_receiver(receiver); }
            if matches!(method, HTML_SLOT_NAME_GET | HTML_SLOT_NAME_SET) {
                return self.html_slot_receiver(receiver);
            }
            if method==DOCUMENT_ADOPT_NODE{return receiver==DOCUMENT||self.inert_document(receiver);}
            if method == DOCUMENT_IMPORT_NODE { return receiver == DOCUMENT || self.inert_document(receiver); }
            if matches!(method, script_inert_documents::CREATE | script_inert_documents::CREATE_DOCTYPE | xml_documents::CREATE | script_inert_documents::HAS_FEATURE) {
                return self.inert_implementation(receiver);
            }
        }
        if self.inert_factory(object, method).is_some() { return self.inert_document(receiver) || method=="createDocumentFragment" && receiver==DOCUMENT; }
        if matches!(method,"getAttributeNS"|"getAttributeNodeNS"|"setAttributeNS"|"removeAttributeNS"|"getAttribute"|"getAttributeNode"|"hasAttribute"|"removeAttribute"|"setAttribute"|"\0Element.getAttributeNames") {
            return (self.event_target_interface(object) || self.attr_element_receiver(object))
                && (self.attr_element_receiver(receiver) || self.inert_node_type(receiver) == Some(1));
        }
        if script_content_editable::accessor(method) && self.event_target_interface(object) {return self.content_editable_receiver(receiver);}
        if matches!(method, ELEMENT_INNER_HTML_GET | ELEMENT_INNER_HTML_SET) {
            return self.event_target_interface(object) && (self.attr_element_receiver(receiver) || self.template_receiver(receiver));
        }
        if crate::javascript::window_relationships::operation(method).is_some() { return self.event_target_interface(object) && self.event_target_interface(receiver); }
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) {return self.event_target_interface(object) && self.collection_interface_receiver(receiver)==Some(false);}
        if matches!(method,TITLE_TEXT_GET|TITLE_TEXT_SET) {return self.event_target_interface(object) && self.title_receiver(receiver);}
        if matches!(method,crate::javascript::document_location_interface::GET|crate::javascript::document_location_interface::SET) {return self.event_target_interface(object)&&self.document_location_owner(receiver).is_some();}
        if script_element_id::operation(method){return self.event_target_interface(object)&&self.canonical_selector_receiver(receiver);}
        if script_element_class_name::operation(method){return self.event_target_interface(object)&&self.canonical_selector_receiver(receiver);}
        if script_input_labels::interface(method).is_some(){return self.event_target_interface(object)&&self.control_labels_receiver(receiver,method);}
        if method==script_label_form::GET{return self.event_target_interface(object)&&self.html_label_receiver(receiver);}
        if method==script_output_form::GET{return self.event_target_interface(object)&&self.html_output_receiver(receiver);}
        if script_control_type::interface(method).is_some(){return self.event_target_interface(object)&&self.control_type_matches(receiver,method);}
        if method==script_form_length::GET{return self.event_target_interface(object)&&self.form_length_receiver(receiver);}
        if script_meter_numeric::operation(method){return self.event_target_interface(object)&&self.meter_numeric_receiver(receiver);}
        if script_progress_numeric::operation(method){return self.event_target_interface(object)&&self.progress_numeric_receiver(receiver);}
        if method==script_legend_form::GET{return self.event_target_interface(object)&&self.legend_receiver(receiver);}
        if method==script_option_form::GET{return self.event_target_interface(object)&&self.option_disabled_receiver(receiver);}
        if method==script_option_index::GET{return self.event_target_interface(object)&&self.option_disabled_receiver(receiver);}
        if method==script_textarea_text_length::GET{return self.event_target_interface(object)&&self.textarea_text_length_receiver(receiver);}
        if method==script_map_areas::GET{return self.event_target_interface(object)&&self.map_name_receiver(receiver);}
        if method==script_datalist_options::GET{return self.event_target_interface(object)&&self.datalist_options_receiver(receiver);}
        if script_time_datetime::operation(method){return self.event_target_interface(object)&&self.time_datetime_receiver(receiver);}
        if script_area_alt::operation(method){return self.event_target_interface(object)&&self.area_alt_receiver(receiver);}
        if script_optgroup_label::operation(method){return self.event_target_interface(object)&&self.optgroup_label_receiver(receiver);}
        if script_option_label::operation(method){return self.event_target_interface(object)&&self.option_label_receiver(receiver);}
        if script_option_text::operation(method){return self.event_target_interface(object)&&self.option_text_receiver(receiver);}
        if script_textarea_default_value::operation(method){return self.event_target_interface(object)&&self.textarea_default_value_receiver(receiver);}
        if script_option_value::operation(method){return self.event_target_interface(object)&&self.option_value_receiver(receiver);}
        if script_select_name::operation(method){return self.event_target_interface(object)&&self.select_name_receiver(receiver);}
        if script_textarea_name::operation(method){return self.event_target_interface(object)&&self.textarea_name_receiver(receiver);}
        if script_area_coords::operation(method){return self.event_target_interface(object)&&self.area_coords_receiver(receiver);}
        if script_area_shape::operation(method){return self.event_target_interface(object)&&self.area_shape_receiver(receiver);}
        if script_area_download::operation(method){return self.event_target_interface(object)&&self.area_download_receiver(receiver);}
        if script_area_type::operation(method){return self.event_target_interface(object)&&self.area_type_receiver(receiver);}
        if script_area_hreflang::operation(method){return self.event_target_interface(object)&&self.area_hreflang_receiver(receiver);}
        if script_area_target::operation(method){return self.event_target_interface(object)&&self.area_target_receiver(receiver);}
        if script_area_rel::operation(method){return self.event_target_interface(object)&&self.area_rel_receiver(receiver);}
        if script_area_href::operation(method){return self.event_target_interface(object)&&self.area_href_receiver(receiver);}
        if script_hyperlink_stringifier::operation(method){return self.event_target_interface(object)&&self.hyperlink_stringifier_receiver(receiver,method);}
        if script_area_url::operation(method).is_some(){return self.event_target_interface(object)&&self.area_href_receiver(receiver);}
        if script_area_referrer_policy::operation(method){return self.event_target_interface(object)&&self.area_referrer_policy_receiver(receiver);}
        if script_area_ping::operation(method){return self.event_target_interface(object)&&self.area_ping_receiver(receiver);}
        if script_map_name::operation(method){return self.event_target_interface(object)&&self.map_name_receiver(receiver);}
        if script_param_name::operation(method){return self.event_target_interface(object)&&self.param_name_receiver(receiver);}
        if script_param_metadata::operation(method){return self.event_target_interface(object)&&self.param_name_receiver(receiver);}
        if script_mod_datetime::operation(method){return self.event_target_interface(object)&&self.mod_datetime_receiver(receiver);}
        if script_data_value::operation(method){return self.event_target_interface(object)&&self.data_value_receiver(receiver);}
        if script_heading_align::operation(method){return self.event_target_interface(object)&&self.heading_align_receiver(receiver);}
        if script_ul_type::operation(method){return self.event_target_interface(object)&&self.ul_type_receiver(receiver);}
        if script_li_type::operation(method){return self.event_target_interface(object)&&self.li_type_receiver(receiver);}
        if script_ol_type::operation(method){return self.event_target_interface(object)&&self.ol_type_receiver(receiver);}
        if script_input_autocomplete::operation(method){return self.event_target_interface(object)&&self.input_autocomplete_receiver(receiver);}
        if script_input_form_action::operation(method){return self.event_target_interface(object)&&self.input_form_action_receiver(receiver);}
        if script_input_form_method::operation(method){return self.event_target_interface(object)&&self.input_form_method_receiver(receiver);}
        if script_input_form_enctype::operation(method){return self.event_target_interface(object)&&self.input_form_enctype_receiver(receiver);}
        if script_button_form_action::operation(method){return self.event_target_interface(object)&&self.button_form_action_receiver(receiver);}
        if script_button_form_method::operation(method){return self.event_target_interface(object)&&self.button_form_method_receiver(receiver);}
        if script_button_form_enctype::operation(method){return self.event_target_interface(object)&&self.button_form_enctype_receiver(receiver);}
        if script_form_autocomplete::operation(method){return self.event_target_interface(object)&&self.form_autocomplete_receiver(receiver);}
        if script_form_method::operation(method){return self.event_target_interface(object)&&self.form_method_receiver(receiver);}
        if script_form_action::operation(method){return self.event_target_interface(object)&&self.form_action_receiver(receiver);}
        if script_form_enctype::operation(method){return self.event_target_interface(object)&&self.form_enctype_receiver(receiver);}
        if script_form_name::operation(method){return self.event_target_interface(object)&&self.form_name_receiver(receiver);}
        if script_button_name::operation(method){return self.event_target_interface(object)&&self.button_name_receiver(receiver);}
        if script_input_name::operation(method){return self.event_target_interface(object)&&self.input_name_receiver(receiver);}
        if script_form_accept_charset::operation(method){return self.event_target_interface(object)&&self.form_accept_charset_receiver(receiver);}
        if script_form_rel::operation(method){return self.event_target_interface(object)&&self.form_rel_receiver(receiver);}
        if script_form_target::operation(method){return self.event_target_interface(object)&&self.form_target_receiver(receiver);}
        if script_button_form_target::operation(method){return self.event_target_interface(object)&&self.button_form_target_receiver(receiver);}
        if script_input_form_target::operation(method){return self.event_target_interface(object)&&self.input_form_target_receiver(receiver);}
        if script_form_no_validate::operation(method){return self.event_target_interface(object)&&self.form_no_validate_receiver(receiver);}
        if script_button_form_no_validate::operation(method){return self.event_target_interface(object)&&self.button_form_no_validate_receiver(receiver);}
        if script_input_form_no_validate::operation(method){return self.event_target_interface(object)&&self.input_form_no_validate_receiver(receiver);}
        if script_select_autocomplete::operation(method){return self.event_target_interface(object)&&self.select_autocomplete_receiver(receiver);}
        if script_textarea_autocomplete::operation(method){return self.event_target_interface(object)&&self.textarea_autocomplete_receiver(receiver);}
        if script_textarea_wrap::operation(method){return self.event_target_interface(object)&&self.textarea_wrap_receiver(receiver);}
        if script_button_value::operation(method){return self.event_target_interface(object)&&self.button_value_receiver(receiver);}
        if script_button_type::operation(method){return self.event_target_interface(object)&&self.button_type_receiver(receiver);}
        if script_input_step::operation(method){return self.event_target_interface(object)&&self.input_step_receiver(receiver);}
        if script_input_max::operation(method){return self.event_target_interface(object)&&self.input_max_receiver(receiver);}
        if script_input_min::operation(method){return self.event_target_interface(object)&&self.input_min_receiver(receiver);}
        if script_input_pattern::operation(method){return self.event_target_interface(object)&&self.input_pattern_receiver(receiver);}
        if script_li_value::operation(method){return self.event_target_interface(object)&&self.li_value_receiver(receiver);}
        if script_pre_width::operation(method){return self.event_target_interface(object)&&self.pre_width_receiver(receiver);}
        if script_textarea_dimensions::operation(method).is_some(){return self.event_target_interface(object)&&self.textarea_dimensions_receiver(receiver);}
        if script_ol_start::operation(method){return self.event_target_interface(object)&&self.ol_start_receiver(receiver);}
        if script_textarea_min_length::operation(method){return self.event_target_interface(object)&&self.textarea_min_length_receiver(receiver);}
        if script_input_min_length::operation(method){return self.event_target_interface(object)&&self.input_min_length_receiver(receiver);}
        if script_textarea_max_length::operation(method){return self.event_target_interface(object)&&self.textarea_max_length_receiver(receiver);}
        if script_input_max_length::operation(method){return self.event_target_interface(object)&&self.input_max_length_receiver(receiver);}
        if script_ul_compact::operation(method){return self.event_target_interface(object)&&self.ul_compact_receiver(receiver);}
        if script_dir_compact::operation(method){return self.event_target_interface(object)&&self.dir_compact_receiver(receiver);}
        if script_menu_compact::operation(method){return self.event_target_interface(object)&&self.menu_compact_receiver(receiver);}
        if script_ol_compact::operation(method){return self.event_target_interface(object)&&self.ol_compact_receiver(receiver);}
        if script_hr_no_shade::operation(method){return self.event_target_interface(object)&&self.hr_no_shade_receiver(receiver);}
        if script_area_no_href::operation(method){return self.event_target_interface(object)&&self.area_no_href_receiver(receiver);}
        if script_option_disabled::operation(method){return self.event_target_interface(object)&&self.option_disabled_receiver(receiver);}
        if script_select_disabled::operation(method){return self.event_target_interface(object)&&self.select_disabled_receiver(receiver);}
        if script_button_disabled::operation(method){return self.event_target_interface(object)&&self.button_disabled_receiver(receiver);}
        if script_input_disabled::operation(method){return self.event_target_interface(object)&&self.input_disabled_receiver(receiver);}
        if script_textarea_disabled::operation(method){return self.event_target_interface(object)&&self.textarea_disabled_receiver(receiver);}
        if script_textarea_required::operation(method){return self.event_target_interface(object)&&self.textarea_required_receiver(receiver);}
        if script_input_required::operation(method){return self.event_target_interface(object)&&self.input_required_receiver(receiver);}
        if script_textarea_read_only::operation(method){return self.event_target_interface(object)&&self.textarea_read_only_receiver(receiver);}
        if script_input_read_only::operation(method){return self.event_target_interface(object)&&self.input_read_only_receiver(receiver);}
        if script_select_required::operation(method){return self.event_target_interface(object)&&self.select_required_receiver(receiver);}
        if script_option_default_selected::operation(method){return self.event_target_interface(object)&&self.option_default_selected_receiver(receiver);}
        if script_optgroup_disabled::operation(method){return self.event_target_interface(object)&&self.optgroup_disabled_receiver(receiver);}
        if script_ol_reversed::operation(method){return self.event_target_interface(object)&&self.ol_reversed_receiver(receiver);}
        if script_dl_compact::operation(method){return self.event_target_interface(object)&&self.dl_compact_receiver(receiver);}
        if script_paragraph_align::operation(method){return self.event_target_interface(object)&&self.paragraph_align_receiver(receiver);}
        if script_hr_align::operation(method){return self.event_target_interface(object)&&self.hr_align_receiver(receiver);}
        if script_hr_color::operation(method){return self.event_target_interface(object)&&self.hr_color_receiver(receiver);}
        if script_font_size::operation(method){return self.event_target_interface(object)&&self.font_size_receiver(receiver);}
        if script_font_face::operation(method){return self.event_target_interface(object)&&self.font_face_receiver(receiver);}
        if script_font_color::operation(method){return self.event_target_interface(object)&&self.font_color_receiver(receiver);}
        if script_br_clear::operation(method){return self.event_target_interface(object)&&self.br_clear_receiver(receiver);}
        if script_hr_size::operation(method){return self.event_target_interface(object)&&self.hr_size_receiver(receiver);}
        if script_hr_width::operation(method){return self.event_target_interface(object)&&self.hr_width_receiver(receiver);}
        if script_legend_align::operation(method){return self.event_target_interface(object)&&self.legend_align_receiver(receiver);}
        if script_div_align::operation(method){return self.event_target_interface(object)&&self.div_align_receiver(receiver);}
        if script_fieldset::operation(method){return self.event_target_interface(object)&&self.fieldset_receiver(receiver);}
        if script_control_form::interface(method).is_some(){return self.event_target_interface(object)&&self.control_form_matches(receiver,method);}
        if script_output_metadata::operation(method){return self.event_target_interface(object)&&self.html_output_receiver(receiver);}
        if let Some(class)=script_control_eligibility::interface(method){return self.event_target_interface(object)&&self.control_eligibility_class(receiver)==Some(class);}
        if script_fieldset_validation::operation(method){return self.event_target_interface(object)&&self.fieldset_receiver(receiver);}
        if script_output_validation::operation(method){return self.event_target_interface(object)&&self.html_output_receiver(receiver);}
        if script_output_validity_state::operation(method){return self.event_target_interface(object)&&if script_output_validity_state::flag(method).is_some(){self.validity_receiver(receiver)}else if matches!(method,script_output_validity_state::FIELD_GET|script_output_validity_state::FIELD_SET){self.fieldset_receiver(receiver)}else if let Some(class)=crate::javascript::control_validity_interface(method){self.control_eligibility_class(receiver)==Some(class)}else{self.html_output_receiver(receiver)};}
        if script_output_value::operation(method).is_some(){return self.event_target_interface(object)&&self.html_output_receiver(receiver);}
        if method==script_label_control::GET{return self.event_target_interface(object)&&self.html_label_receiver(receiver);}
        if script_label_html_for::operation(method){return self.event_target_interface(object)&&self.html_label_receiver(receiver);}
        if script_element_class_list::operation(method){return self.event_target_interface(object)&&self.canonical_selector_receiver(receiver);}
        if script_output_html_for::operation(method){return self.event_target_interface(object)&&self.html_output_receiver(receiver);}
        if script_area_rel_list::operation(method){return self.event_target_interface(object)&&self.area_rel_receiver(receiver);}
        if script_form_rel_list::operation(method){return self.event_target_interface(object)&&self.form_rel_receiver(receiver);}
        if anchor_url_interface::operation(method).is_some() {return self.event_target_interface(object)&&self.native_anchor(receiver);}
        if crate::javascript::child_remove_operation(method) {
            return self.event_target_interface(object) && match method {
                crate::javascript::ELEMENT_REMOVE=>self.collection_interface_receiver(receiver)==Some(false),
                crate::javascript::CHARACTER_REMOVE=>self.character_data_receiver(receiver),
                _=>self.doctype_receiver(receiver),
            };
        }
        if let Some((interface,_)) = charset_operation(method) { return self.event_target_interface(object) && self.charset_interface(receiver)==Some(interface); }
        if meta_operation(method).is_some() { return self.event_target_interface(object) && self.meta_receiver(receiver); }
        if crate::javascript::aria_reflection_interface::operation(method).is_some(){return self.event_target_interface(object)&&self.aria_reflection_receiver(receiver);}
        if crate::javascript::html_reflection_interface::operation(method).is_some(){return self.event_target_interface(object)&&self.html_reflection_receiver(receiver);}
        if matches!(method, ELEMENT_STYLE_GET | ELEMENT_STYLE_SET |SVG_STYLE_GET|SVG_STYLE_SET) {
            return self.event_target_interface(object) && self.style_interface_receiver(receiver,matches!(method,SVG_STYLE_GET|SVG_STYLE_SET));
        }
        if script_inline_style::handle(object) && script_inline_style::method(method).is_some() {
            return script_inline_style::handle(receiver) && self.attr_element_receiver(node_handle((receiver - STYLE_BASE) as usize));
        }
        if let Some(operation) = CanvasOperation::parse(method) {
            return self.event_target_interface(object) && operation.accepts(self, receiver);
        }
        if (matches!(method, "\0Document.get.documentElement" | crate::javascript::DOCUMENT_DOCTYPE_GET | DOCUMENT_CURRENT_SCRIPT_GET | DOCUMENT_ACTIVE_ELEMENT_GET) || crate::javascript::document_encoding_getter(method))
            && self.event_target_interface(object)
        {
            return self.document_interface_receiver(receiver);
        }
        if script_canvas::accessor(method).is_some() && self.event_target_interface(object) {
            return self.canvas_receiver(receiver);
        }
        if script_input_accessors::accessor(method).is_some() && self.event_target_interface(object) {
            return self.input_receiver(receiver);
        }
        if script_scroll_offsets::accessor(method).is_some() && self.event_target_interface(object) {
            return self.element_selector_receiver(receiver);
        }
        if Offset::getter(method).is_some() && self.event_target_interface(object) {
            return self.html_element_receiver(receiver);
        }
        if self.computed_style_receiver(object) {return self.computed_style_receiver(receiver);}
        if self.event_target_interface(object)&&matches!(method,"\0HTMLCollection.get.length"|"\0HTMLCollection.item"|"\0HTMLCollection.namedItem"){return self.collection_receiver(receiver);}
        if self.event_target_interface(object)&&script_named_map::operation(method).is_some(){return self.named_map_receiver(receiver);}
        if self.event_target_interface(object)&&matches!(method,"\0NodeList.get.length"|"\0NodeList.item") {return self.node_list_receiver(receiver);}
        if self.event_target_interface(object)&&script_element_selectors::operation(method).is_some() {return self.selector_operation_receiver(receiver,method);}
        if self.event_target_interface(object) {
            if let Some(interface) = Slottable::getter(method) {return self.slottable(receiver) == Some(interface);}
            if let Some((family,_))=script_element_siblings::operation(method) {return self.element_sibling_family(receiver)==Some(family);}
        }
        if object==DOCUMENT && method=="createDocumentFragment" {return receiver==DOCUMENT || self.inert_document(receiver);}
        if dom_tree::node_mutation_method(method) {
            if self.event_target_interface(object) {
                return self.node_mutation_receiver(receiver)
                    || method=="replaceChild" && self.inert_replace_receiver(receiver)
                    || matches!(method,"appendChild"|"insertBefore"|"removeChild") && self.inert_mutation_receiver(receiver);
            }
            let parent=|handle|node_index(handle).is_some_and(|node|self.dom_accessible(node)
                && matches!(self.doc.nodes[node].kind,NodeKind::Element {..}|NodeKind::DocumentFragment));
            if parent(object) {return parent(receiver);}
        }
        if self.event_target_interface(object)&&method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME{return self.identifier_lookup_receiver(receiver,Some(true));}
        if self.event_target_interface(object){if let Some(document)=script_inert_documents::identifier_lookup::operation(method){return self.identifier_lookup_receiver(receiver,Some(document));}}
        if self.event_target_interface(object)&&(script_inert_documents::node_equality::operation(method)||method==script_position::METHOD){return self.namespace_receiver(receiver);}
        if self.event_target_interface(object) && method==script_node_root::ROOT_METHOD {return self.get_root_receiver(receiver);}
        if self.event_target_interface(object) && method == script_clone_node::CLONE_METHOD { return self.clone_node_receiver(receiver); }
        if self.event_target_interface(object) && method == script_node_contains::CONTAINS_METHOD {
            return self.root_node_receiver(receiver) || self.frame_contains_node(receiver).is_some();
        }
        if object == WINDOW && method == script_inert_documents::parser::PARSE {
            return self.inert_parser(receiver);
        }
        if self.event_target_interface(object) && crate::javascript::abort_interface::operation(method) {
            return self.matches_abort_receiver(receiver, method);
        }
        if self.event_target_interface(object)&&(method.starts_with("\0CustomEvent.")||method=="initCustomEvent") {return self.custom_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0Video.") {return self.video_interface_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0Media.") {return self.media_interface_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0KeyboardEvent.") {return self.keyboard_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0FocusEvent.") {return self.focus_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0SubmitEvent.") {return self.submit_event_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0WheelEvent.") {return self.wheel_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0ErrorEvent.") {return self.error_event_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0AnimationEvent.") {return self.animation_receiver(receiver);}
        if self.event_target_interface(object)&&method.starts_with("\0ProgressEvent.") {return self.progress_receiver(receiver);}
        if self.event_target_interface(object)&&(method.starts_with("\0MouseEvent.")||matches!(method,"initMouseEvent"|"getModifierState")) {
            return self.mouse_receiver(receiver);
        }
        if self.event_target_interface(object)&&(method.starts_with("\0UIEvent.")||method=="initUIEvent") {
            return self.ui_receiver(receiver);
        }
        if self.event_target_interface(object) && parent_children::operation(method).is_some() {
            return self.parent_node_receiver(receiver);
        }
        if self.event_target_interface(object) && matches!(method,"\0Node.get.ownerDocument"|"\0Node.get.isConnected"|"\0Node.get.baseURI") {
            return self.text_content_receiver(receiver);
        }
        if self.event_target_interface(object) && node_traversal_property(method).is_some() {
            return self.node_traversal_receiver(receiver);
        }
        if self.event_target_interface(object) && matches!(method, NODE_TEXT_CONTENT_GET | NODE_TEXT_CONTENT_SET | NODE_VALUE_GET | NODE_VALUE_SET | NODE_HAS_CHILDREN) {
            return self.text_content_receiver(receiver);
        }
        if self.event_target_interface(object) && matches!(method,"\0Node.get.nodeType"|"\0Node.get.nodeName") {return self.node_identity_receiver(receiver);}
        if self.event_target_interface(object)&&(method.starts_with("\0Event.")||matches!(method,"initEvent"|"preventDefault"|"stopPropagation"|"stopImmediatePropagation")) {
            return self.event_receiver(receiver);
        }
        if matches!(method,"load"|"pause"|"play") && self.media_element_receiver(object) {return self.media_element_receiver(receiver);}
        if matches!(method,"dispatchEvent"|"addEventListener"|"removeEventListener") {
            return (self.event_target_interface(object)||self.dispatch_receiver(object)) && self.dispatch_receiver(receiver);
        }
        if object==DOCUMENT && matches!(method,"createAttribute"|"createAttributeNS"|"createElementNS") {return receiver==DOCUMENT;}
        if object == HISTORY { return receiver == HISTORY && matches!(method,"replaceState"|"pushState"); }
        if method == "getBoundingClientRect" { return self.class_list_element_receiver(object) && self.class_list_element_receiver(receiver); }
        if script_named_map::element(object).is_some() { return self.named_map_receiver(object) && self.named_map_receiver(receiver); }
        if method=="normalize" {return !self.is_frame_handle(object) && !self.is_frame_handle(receiver)
            && self.namespace_receiver(object) && self.namespace_receiver(receiver);}
        if method=="getElementsByClassName" {
            return if object==DOCUMENT||self.inert_node_type(object)==Some(9){receiver==DOCUMENT||self.inert_node_type(receiver)==Some(9)}
                else {(self.attr_element_receiver(object)||self.inert_node_type(object)==Some(1))&&(self.attr_element_receiver(receiver)||self.inert_node_type(receiver)==Some(1))};
        }
        if script_class_list::element(object).is_some() { return self.class_list_receiver(object) && self.class_list_receiver(receiver); }
        if method == "initMouseEvent" { return self.mouse_receiver(object) && self.mouse_receiver(receiver); }
        if method == "initCustomEvent" { return self.custom_receiver(object) && self.custom_receiver(receiver); }
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
            return self.node_list_receiver(object) && self.node_list_receiver(receiver);
        }
        object == receiver
    }

    fn global_operation_receiver(&self, object: u32, method: &str, receiver: &Value)
        -> Option<Result<u32, (&'static str, String)>> {
        if object==WINDOW && method=="matchMedia" {return Some(match receiver {
            Value::Null|Value::Undefined|Value::Host(WINDOW)=>Ok(WINDOW),
            Value::Host(handle) if self.frame_context(*handle).is_some()=>Err(("NotSupportedError","Child Window media query evaluation is unavailable.".into())),
            _=>Err(("TypeError","matchMedia requires a Window receiver.".into())),
        });}
        if object==WINDOW && method=="getComputedStyle" {return Some(match receiver {
            Value::Null|Value::Undefined|Value::Host(WINDOW)=>Ok(WINDOW),
            Value::Host(handle) if self.frame_context(*handle).is_some()=>Err(("NotSupportedError","Computed style for a child Window has no renderer binding.".into())),
            _=>Err(("TypeError","getComputedStyle requires a Window receiver.".into())),
        });}
        if object != WINDOW || !matches!(method, "requestAnimationFrame" | "cancelAnimationFrame") { return None; }
        Some(match receiver {
            Value::Null | Value::Undefined | Value::Host(WINDOW) => Ok(WINDOW),
            Value::Host(handle) if self.frame_context(*handle).is_some() =>
                Err(("NotSupportedError", "Animation frame scheduling for a child Window is not implemented.".into())),
            _ => Err(("TypeError", "Animation frame operation requires a Window receiver.".into())),
        })
    }

    fn indexed_iteration_owner(&self, object:u32)->Option<u32> {
        self.collection_owner_window(object).or_else(|| self.node_list_owner(object)).or_else(||self.named_map_receiver(object).then_some(WINDOW)).or_else(||self.class_list_receiver(object).then_some(WINDOW))
    }

    fn own_keys(&mut self, object: u32) -> Result<Vec<String>, String> {
        if self.attr_receiver(object){self.state.work(1)?;return Ok(Vec::new());}
        if script_inline_style::handle(object) {return self.inline_keys(object, false);}
        if self.canvas_context_receiver(object) {self.state.work(1)?;return Ok(Vec::new());}
        if object==DOCUMENT {self.state.work(1)?;return Ok(Vec::new());}
        if self.range_owner(object).is_some() || self.standalone_target_owner(object).is_some() || self.abort_instance(object).is_some() {self.state.work(1)?;return Ok(Vec::new());}
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
            return self.dataset_keys(object);
        }
        Err("Enumerating this native host object is not implemented.".into())
    }

    fn inherited_keys(&mut self,object:u32) -> Result<Vec<String>,String> {
        if script_inline_style::handle(object) {return self.inline_keys(object, true);}
        if self.element_expando_receiver(object) {
            return Err("Inherited native Element property enumeration is not implemented.".into());
        }
        self.own_keys(object)
    }

    fn enumeration_member(&mut self, object: u32, name: &str) -> Result<bool, String> {
        if script_inline_style::handle(object) {
            // Supported inline CSS properties, IDL members and live own indices
            // are enumerable. Presence validates the receiver and current list.
            return self.has(object, name);
        }
        self.own_data(object, name).map(|slot| slot.is_some())
    }

    fn own_data(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if self.form_request_member(object,property){self.state.work(1)?;return Ok(None);}
        if self.control_custom_validity_member(object,property)||self.control_validation_member(object,property)||self.form_validation_member(object,property){self.state.work(1)?;return Ok(None);}
        if property=="reset" && self.form_length_receiver(object) && !self.form_named_control(object,property) {self.state.work(1)?;return Ok(None);}
        if script_inline_style::handle(object) {
            let node = self.style_node(object)?;
            return match collection_index(property) { Some(index) => self.inline_index(node, index), None => Ok(None) };
        }
        if self.canvas_context_receiver(object) {self.state.work(1)?;return Ok(None);}
        if self.node_list_iteration_member(object,property) {self.state.work(1)?;return Ok(None);}
        if self.element_selector_member(object,property) {self.state.work(1)?;return Ok(None);}
        if self.inherited_node_property(object,property) {self.state.work(1)?;return Ok(None);}
        if self.node_tree_method_member(object,property) {self.state.work(1)?;return Ok(None);}
        if object==DOCUMENT && dom_tree::document_node_property(property) {self.state.work(1)?;return Ok(None);}
        if self.range_owner(object).is_some() || self.standalone_target_owner(object).is_some() || self.abort_instance(object).is_some() {self.state.work(1)?;return Ok(None);}
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
        if self.form_request_member(object,property){self.state.work(1)?;return Ok(true);}
        if self.control_custom_validity_member(object,property)||self.control_validation_member(object,property)||self.form_validation_member(object,property){self.state.work(1)?;return Ok(true);}
        if property=="reset" && self.form_length_receiver(object) && !self.form_named_control(object,property) {self.state.work(1)?;return Ok(true);}
        if script_inline_style::handle(object) { return Ok(self.own_data(object, property)?.is_none()); }
        if self.canvas_context_receiver(object) {self.state.work(1)?;return Ok(true);}
        if self.node_list_iteration_member(object,property) {self.state.work(1)?;return Ok(true);}
        if self.element_selector_member(object,property) {self.state.work(1)?;return Ok(true);}
        if self.inherited_node_property(object,property) {self.state.work(1)?;return Ok(true);}
        if self.node_tree_method_member(object,property) {self.state.work(1)?;return Ok(true);}
        if object==DOCUMENT && dom_tree::document_node_property(property) {self.state.work(1)?;return Ok(true);}
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
    fn begin_form_request(&mut self,object:u32,args:&[Value])->Result<Option<usize>,String>{self.begin_native_form_request(object,args)}
    fn form_request_validation(&mut self,token:usize)->Result<Option<usize>,(&'static str,String)>{self.native_form_request_validation(token)}
    fn form_request_event(&mut self,token:usize)->Result<usize,String>{self.native_form_request_event(token)}
    fn complete_form_request(&mut self,token:usize)->Result<(),String>{self.complete_native_form_request(token)}
    fn finish_form_request(&mut self,token:usize){self.finish_native_form_request(token);}
    fn begin_control_validation(&mut self,object:u32,report:Option<bool>)->Result<crate::javascript::NativeControlValidation,(&'static str,String)>{self.prepare_native_control_validation(object,report)}
    fn begin_form_validation(&mut self,object:u32,report:bool)->Result<usize,String>{self.begin_native_form_validation(object,report)}
    fn next_form_validation_event(&mut self,token:usize)->Result<Option<usize>,String>{self.next_native_form_validation_event(token)}
    fn form_validation_event_result(&mut self,token:usize,accepted:bool)->Result<(),String>{self.native_form_validation_event_result(token,accepted)}
    fn complete_form_validation(&mut self,token:usize)->Result<bool,String>{self.complete_native_form_validation(token)}
    fn finish_form_validation(&mut self,token:usize){self.finish_native_form_validation(token);}
    fn begin_form_reset(&mut self,object:u32)->Result<Option<usize>,String>{self.begin_native_form_reset(object)}
    fn prepare_form_reset(&mut self,object:u32)->Result<(),String>{self.prepare_native_form_reset(object)}
    fn next_form_reset_output(&mut self,object:u32)->Result<Option<u32>,String>{self.next_native_form_reset_output(object)}
    fn complete_form_reset(&mut self,object:u32)->Result<(),String>{self.complete_native_form_reset(object)}
    fn finish_form_reset(&mut self,object:u32){self.finish_native_form_reset(object);}
    fn synchronous_event_dispatch(&self,object:u32,method:&str)->bool {method=="dispatchEvent"&&self.dispatch_receiver(object)}
    fn begin_abort_dispatch(&mut self, controller: u32, reason: Value) -> Result<usize, String> {
        self.commit_abort_dispatch(controller, reason)
    }
    fn new_aborted_signal(&mut self, owner: u32, reason: Value) -> Result<u32, String> {
        self.create_abort_record(owner, false, reason)
    }
    fn prepare_event_dispatch(&mut self)->Result<(),String>{self.state.work(self.state.event_lookup_work(5))}
    fn begin_event_dispatch(&mut self,object:u32,args:&[Value])->Result<usize,String>{self.begin_synthetic_dispatch(object,args)}
    fn next_event_callback(&mut self,token:usize)->Result<Option<crate::javascript::EventCallback>,String>{self.next_synthetic_callback(token)}
    fn event_callback_result(&mut self,token:usize,returned_false:bool){self.synthetic_callback_result(token,returned_false);}
    fn event_callback_error(&mut self,token:usize,message:&str){self.synthetic_callback_error(token,message);}
    fn finish_event_dispatch(&mut self,token:usize)->bool{self.finish_synthetic_dispatch(token)}
    fn dictionary_argument(
        &self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Option<(usize, &'static [&'static str])> {
        if CanvasOperation::parse(method) == Some(CanvasOperation::GetContext) {
            return self.canvas_context_dictionary(object, args);
        }
        if method == crate::javascript::canvas_readback::METHOD {
            return Some((4, &["colorSpace", "pixelFormat"]));
        }
        if method==crate::javascript::range_interface::STATIC && self.event_target_interface(object) {return Some((0,&["endContainer","endOffset","startContainer","startOffset"]));}
        if method == DOCUMENT_IMPORT_NODE { return Some((1, &["customElementRegistry", "selfOnly"])); }
        if method==script_node_root::ROOT_METHOD && self.root_node_receiver(object) {return Some((0,&["composed"]));}
        if method=="Event"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed"]));}
        if method=="CustomEvent"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed","detail"]));}
        if method=="UIEvent"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed","detail","view"]));}
        if method=="FocusEvent"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed","detail","view","relatedTarget"]));}
        if method=="SubmitEvent"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed","submitter"]));}
        if method=="ErrorEvent"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed","colno","error","filename","lineno","message"]));}
        if method=="AnimationEvent"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed","animationName","elapsedTime","pseudoElement"]));}
        if method=="ProgressEvent"&&self.event_target_interface(object) {return Some((1,&["bubbles","cancelable","composed","lengthComputable","loaded","total"]));}
        if method=="KeyboardEvent"&&self.event_target_interface(object) {return Some((1,&[
            "bubbles","cancelable","composed","detail","view",
            "altKey","ctrlKey","metaKey","modifierAltGraph","modifierCapsLock","modifierFn","modifierFnLock",
            "modifierHyper","modifierNumLock","modifierScrollLock","modifierSuper","modifierSymbol","modifierSymbolLock","shiftKey",
            "charCode","code","isComposing","key","keyCode","location","repeat",
        ]));}
        if method=="WheelEvent"&&self.event_target_interface(object) {return Some((1,&[
            "bubbles","cancelable","composed","detail","view",
            "altKey","ctrlKey","metaKey","modifierAltGraph","modifierCapsLock","modifierFn","modifierFnLock",
            "modifierHyper","modifierNumLock","modifierScrollLock","modifierSuper","modifierSymbol","modifierSymbolLock","shiftKey",
            "button","buttons","clientX","clientY","relatedTarget","screenX","screenY",
            "deltaMode","deltaX","deltaY","deltaZ","momentum",
        ]));}
        if method=="MouseEvent"&&self.event_target_interface(object) {return Some((1,&[
            "bubbles","cancelable","composed","detail","view",
            "altKey","ctrlKey","metaKey","modifierAltGraph","modifierCapsLock","modifierFn","modifierFnLock",
            "modifierHyper","modifierNumLock","modifierScrollLock","modifierSuper","modifierSymbol","modifierSymbolLock","shiftKey",
            "button","buttons","clientX","clientY","relatedTarget","screenX","screenY",
        ]));}
        match method {
            "addEventListener" => Some((2, &["capture", "once", "passive", "signal"])),
            "removeEventListener" => Some((2, &["capture"])),
            _ => None,
        }
    }

    fn dictionary_member_conversion(&self,object:u32,method:&str,member:&str)->Option<bool>{
        if method==crate::javascript::range_interface::STATIC {return matches!(member,"endOffset"|"startOffset").then_some(false);}
        if method == crate::javascript::canvas_readback::METHOD { return Some(true); }
        if CanvasOperation::parse(method) == Some(CanvasOperation::GetContext) {
            return matches!(member, "colorSpace" | "colorType").then_some(true);
        }
        if method=="WheelEvent"&&self.event_target_interface(object)&&matches!(member,"deltaMode"|"deltaX"|"deltaY"|"deltaZ"){return Some(false);}
        if method=="ErrorEvent"&&self.event_target_interface(object) {return match member{"message"|"filename"=>Some(true),"lineno"|"colno"=>Some(false),_=>None};}
        if method=="AnimationEvent"&&self.event_target_interface(object) {return match member {"animationName"|"pseudoElement"=>Some(true),"elapsedTime"=>Some(false),_=>None};}
        if method=="ProgressEvent"&&self.event_target_interface(object)&&matches!(member,"loaded"|"total"){return Some(false);}
        if method=="KeyboardEvent"&&self.event_target_interface(object) {
            if matches!(member,"key"|"code") {return Some(true);}
            if matches!(member,"detail"|"location"|"keyCode"|"charCode") {return Some(false);}
        }
        (self.event_target_interface(object)&&(matches!(method,"UIEvent"|"MouseEvent"|"WheelEvent"|"FocusEvent")&&member=="detail"
            || matches!(method,"MouseEvent"|"WheelEvent")&&matches!(member,"button"|"buttons"|"clientX"|"clientY"|"screenX"|"screenY"))).then_some(false)
    }
    fn validate_dictionary_member(&self,object:u32,method:&str,member:&str,value:&Value)->Result<(),(&'static str,String)>{
        if method==crate::javascript::range_interface::STATIC {return self.validate_static_range_member(member,value);}
        if method=="WheelEvent"&&self.event_target_interface(object)&&matches!(member,"deltaX"|"deltaY"|"deltaZ")&&!matches!(value,Value::Undefined)&&!matches!(value,Value::Number(n) if n.is_finite()){return Err(("TypeError","WheelEvent deltas require finite doubles.".into()));}
        if method=="AnimationEvent"&&self.event_target_interface(object)&&member=="elapsedTime"&&!matches!(value,Value::Undefined)&&!matches!(value,Value::Number(number) if number.is_finite()) {
            return Err(("TypeError","AnimationEvent elapsedTime requires a finite double.".into()));
        }
        if method == DOCUMENT_IMPORT_NODE && member == "customElementRegistry" && !matches!(value, Value::Undefined) {
            return Err(("TypeError", "importNode requires a genuine custom element registry.".into()));
        }
        if method == crate::javascript::canvas_readback::METHOD {
            return crate::javascript::canvas_readback::validate_setting(member, value);
        }
        if CanvasOperation::parse(method) == Some(CanvasOperation::GetContext) {
            return script_canvas_context::option_validation(member,value);
        }
        if matches!(method,"UIEvent"|"MouseEvent"|"WheelEvent"|"KeyboardEvent"|"FocusEvent")&&member=="view"&&self.event_target_interface(object) {
            self.ui_view_argument(object,Some(value))?;
        }
        if method=="FocusEvent"&&self.event_target_interface(object)&&member=="relatedTarget" {self.mouse_target_argument(object,Some(value))?;}
        if method=="SubmitEvent"&&self.event_target_interface(object)&&member=="submitter" {self.submit_event_argument(object,Some(value))?;}
        if matches!(method,"MouseEvent"|"WheelEvent")&&self.event_target_interface(object) {
            if member=="relatedTarget" {self.mouse_target_argument(object,Some(value))?;}
            if matches!(member,"screenX"|"screenY"|"clientX"|"clientY")
                && !matches!(value,Value::Undefined)
                && !matches!(value,Value::Number(number) if number.is_finite()) {
                return Err(("TypeError","MouseEvent coordinates require finite doubles.".into()));
            }
        }
        Ok(())
    }
    fn validate_dictionary_call(&mut self,object:u32,method:&str,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<(),(&'static str,String)>{if method==crate::javascript::range_interface::STATIC{return self.validate_static_range_fields(fields);}if method==DOCUMENT_IMPORT_NODE&&object!=DOCUMENT{let Some(Value::Host(source))=args.first()else{return Err(("TypeError","importNode requires a genuine Node.".into()));};return self.validate_inert_document_import(object,*source,script_clone_node::import_subtree(args,fields)).map(|_|());}Ok(())}
    fn call_with_dictionary(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        fields: Option<&[(&str, Value)]>,
    ) -> Result<Value, String> {
        if let Some(operation) = CanvasOperation::parse(method) {
            return self.canvas_context_call(object, operation, args, fields);
        }
        if method==crate::javascript::range_interface::STATIC {return self.create_static_range(object,fields);}
        if method==DOCUMENT_IMPORT_NODE{if object!=DOCUMENT{let Some(Value::Host(source))=args.first()else{return Err("importNode requires a genuine Node source.".into());};return self.import_into_inert_document(object,*source,script_clone_node::import_subtree(args,fields));}return self.import_node(args,fields);}
        if method==script_node_root::ROOT_METHOD {return self.get_root_node(object,args,fields);}
        if method=="Event"&&self.event_target_interface(object) {return self.construct_base_event(object,args,fields);}
        if method=="CustomEvent"&&self.event_target_interface(object) {return self.construct_custom_event(object,args,fields);}
        if method=="UIEvent"&&self.event_target_interface(object) {return self.construct_ui_event(object,args,fields);}
        if method=="WheelEvent"&&self.event_target_interface(object) {return self.construct_wheel_event(object,args,fields);}
        if method=="MouseEvent"&&self.event_target_interface(object) {return self.construct_mouse_event(object,args,fields);}
        if method=="KeyboardEvent"&&self.event_target_interface(object) {return self.construct_keyboard_event(object,args,fields);}
        if method=="FocusEvent"&&self.event_target_interface(object) {return self.construct_focus_event(object,args,fields);}
        if method=="SubmitEvent"&&self.event_target_interface(object) {return self.construct_submit_event(object,args,fields);}
        if method=="ErrorEvent"&&self.event_target_interface(object) {return self.construct_error_event(object,args,fields);}
        if method=="AnimationEvent"&&self.event_target_interface(object) {return self.construct_animation_event(object,args,fields);}
        if method=="ProgressEvent"&&self.event_target_interface(object) {return self.construct_progress_event(object,args,fields);}
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
                ("onclick", true), ("onauxclick", true), ("oninput", true), ("onchange", true), ("onsubmit", true),
                ("onreset", true), ("oninvalid", true), ("onload", true), ("onkeydown", true), ("onkeyup", true),
                ("onfocus",true),("onblur",true)]);
        }
        if self.frame_context(window).is_none() { return Err("Unknown Window key receiver.".into()); }
        self.frame_window_access(Some(WINDOW), window)?;
        Ok(&[("window", true), ("self", true), ("globalThis", false),
            ("document", true), ("location", true), ("parent", true), ("top", true),
            ("frameElement", true), ("closed", true), ("length", true), ("origin", true)])
    }

    fn has(&mut self, object: u32, property: &str) -> Result<bool, String> {
        if self.canonical_attribute_node_member(object,property){return Ok(false);}
        if self.canonical_attr_member(object,property){return Ok(false);}
        if self.canonical_attribute_factory_member(object,property){return Ok(false);}
        if self.canonical_collection_member(object,property)||self.canonical_name_list_member(object,property){return Ok(false);}
        if self.xhr_index(object).is_some(){return Ok(self.xhr_member(object,property));}

        if self.range_owner(object).is_some() {self.state.work(1)?;return Ok(false);}
        if self.named_map_receiver(object)&&script_named_map::prototype_member(property){return Ok(false);}
        if self.collection_receiver(object)&&self.collection_prototype_member(property){return Ok(false);}
        if parent_children::member(property) && self.parent_node_receiver(object) {return Ok(true);}

        if self.inert_interface(object).is_some() {
            self.state.work(1)?;
            return Ok(self.inert_owns_property(object, property));
        }
        if self.canvas_context_receiver(object) {self.state.work(1)?;return Ok(false);}
        if script_computed_style::index(object).is_some() {self.state.work(1)?;return if self.computed_style_receiver(object) {Ok(script_computed_style::attribute_member(property)||self.method(object,property).is_some())} else {Err("Invalid computed declaration receiver.".into())};}
        if self.node_list_iteration_member(object,property) {self.state.work(1)?;return Ok(false);}
        if self.element_selector_member(object,property) {self.state.work(1)?;return Ok(false);}
        if self.inherited_node_property(object,property) {self.state.work(1)?;return Ok(false);}
        if object==DOCUMENT && property=="createDocumentFragment" {self.state.work(1)?;return Ok(true);}
        if self.node_tree_method_member(object,property) {self.state.work(1)?;return Ok(false);}
        if property=="dispatchEvent"&&self.dispatch_receiver(object){return Ok(true);}
        self.state.work(1)?;
        if script_named_map::element(object).is_some() { return self.named_map_has(object, property); }
        if property=="normalize" && !self.is_frame_handle(object) && self.namespace_receiver(object) {return Ok(true);}
        if property=="getElementsByTagName"&&matches!(self.inert_node_type(object),Some(1|9)){return Ok(true);}
        if property=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object) || matches!(self.inert_node_type(object),Some(1|9))) { return Ok(true); }
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

        if collection_handle_index(object).is_some() {
            return Ok(matches!(property, "length" | "item" | "namedItem")
                || !matches!(self.tag_collection_get(object, property)?, Value::Undefined));
        }
        if node_list_index(object).is_some() {
            return Ok(property=="length"
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
                "state" | "length" | "replaceState" | "pushState" => Ok(true),
                "constructor" | "__proto__" | "scrollRestoration" | "go" | "back" | "forward" => Err(format!("History property {property:?} is not implemented.")),
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
            let mouse = self.state.event.as_ref().unwrap().mouse.is_some();
            let keyboard = self.state.event.as_ref().unwrap().keyboard.is_some();
            return Ok((mouse || self.focus_receiver(object) || self.submit_event_receiver(object)) && self.event_native_member(object, property) || keyboard && keyboard_property(property)
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
            let node = self.style_node(object)?;
            if let Some(index) = collection_index(property) { return Ok(index < self.inline_names(node)?.len()); }
            return Ok(matches!(property,"length"|"cssText") || script_inline_style::method(property).is_some() || supported_css_name(property).is_some());
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
                            || self.dom_script(node) || self.dom_stylesheet(node)
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
            if dom_tree::document_node_property(property) {return Ok(true);}
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
                    | "childNodes"
                    | "body"
                    | "head"
                    | "images"
                    | "parentElement"
                    | "parentNode"
                    | "ownerDocument"
                    | "documentElement"
                    | "activeElement"
                    | "readyState"
                    | "referrer"
                    | "defaultView"
                    | "compatMode"
                    | "doctype"
                    | "prerendering"
                    | "getElementById"
                    | "getElementsByTagName"
                    | "querySelector"
                    | "querySelectorAll"
                    | "createElement"
                    | "createElementNS"
                    | "createTextNode"
                    | "createComment"
                    | "createAttribute"
                    | "createAttributeNS"
                    | "addEventListener"
                    | "removeEventListener"
            ) || !matches!(self.named_document_property(property)?, Value::Undefined));
        }
        if self.hidden_input_property(object,property) { return Ok(true); }
        if self.button_property(object,property) { return Ok(true); }
        if self.text_input_default_property(object,property) || self.button_type_property(object,property) { return Ok(true); }
        if let Some(has) = self.dom_has(object, property)? {
            return Ok(has);
        }
        let node = self.node(object)?;
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html && tag(self.doc,node)!="form"
            && matches!(property,"prototype"|"setCapture"|"releaseCapture")
            || property=="animate" && self.html_element_expando_receiver(object) {
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
            "value" => self.forms.controls.contains_key(&node),
            "form" => self.forms.controls.contains_key(&node) || self.unadmitted_parentless_input_form(object),
            "selectedIndex" => self
                .forms
                .controls
                .get(&node)
                .is_some_and(|control| control.kind == Kind::Select),
            "checked" | "defaultChecked" => self.input_receiver(object),
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

    fn get_with_caller(&mut self, caller:Option<u32>, object:u32, property:&str)->Result<Value,String> {
        if self.is_frame_handle(object) {
            self.state.work(1)?;
            return self.frame_get_with_caller(caller,object,property)
                .ok_or_else(||"Invalid blank-frame handle.".to_string())?;
        }
        self.get(object,property)
    }

    fn get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        if matches!(property,"id"|"className"|"classList"|"baseURI") && self.form_named_control(object,property) {return self.named_form_property(node_index(object).unwrap(),property);}
        if property=="baseURI" && self.text_content_receiver(object) {return self.node_base_uri_get(object);}
        if self.aria_reflection_member(object,property){return self.aria_reflection_get(object,property);}
        if self.charset_member(object, property) { return self.charset_reflection_get(object); }
        if self.meta_member(object, property) { return self.meta_get(object, meta_attribute(property).unwrap()); }
        if self.element_id_member(object,property){return self.element_id_get(object);}
        if self.element_class_name_member(object,property){return self.element_class_name_get(object);}
        if property=="labels"{if let Some(method)=self.control_labels_method(object){return self.html_control_labels_get(object,method);}}
        if self.html_label_form_member(object,property){return self.html_label_form_get(object);}
        if self.output_form_member(object,property){return self.output_form_get(object);}
        if self.control_type_member(object,property){return self.control_type_get(object,self.control_type_method(object).unwrap());}
        if self.form_length_member(object,property){return self.form_length_get(object);}
        if self.meter_numeric_member(object,property){return self.meter_numeric_get(object,match property{"value"=>script_meter_numeric::VALUE_GET,"min"=>script_meter_numeric::MIN_GET,"max"=>script_meter_numeric::MAX_GET,"low"=>script_meter_numeric::LOW_GET,"high"=>script_meter_numeric::HIGH_GET,"optimum"=>script_meter_numeric::OPTIMUM_GET,_=>unreachable!()});}
        if self.progress_numeric_member(object,property){return self.progress_numeric_get(object,match property{"value"=>script_progress_numeric::VALUE_GET,"max"=>script_progress_numeric::MAX_GET,_=>script_progress_numeric::POSITION_GET});}
        if self.legend_form_member(object,property){return self.legend_form_get(object);}
        if self.option_form_member(object,property){return self.option_form_get(object);}
        if self.option_index_member(object,property){return self.option_index_get(object);}
        if self.textarea_text_length_member(object,property){return self.textarea_text_length_get(object);}
        if self.map_areas_member(object,property){return self.map_areas_get(object);}
        if self.datalist_options_member(object,property){return self.datalist_options_get(object);}
        if self.time_datetime_member(object,property){return self.time_datetime_get(object);}
        if self.area_alt_member(object,property){return self.area_alt_get(object);}
        if self.optgroup_label_member(object,property){return self.optgroup_label_get(object);}
        if self.option_label_member(object,property){return self.option_label_get(object);}
        if self.option_text_member(object,property){return self.option_text_get(object);}
        if self.textarea_default_value_member(object,property){return self.textarea_default_value_get(object);}
        if self.option_value_member(object,property){return self.option_value_get(object);}
        if self.select_name_member(object,property){return self.select_name_get(object);}
        if self.textarea_name_member(object,property){return self.textarea_name_get(object);}
        if self.area_coords_member(object,property){return self.area_coords_get(object);}
        if self.area_shape_member(object,property){return self.area_shape_get(object);}
        if self.area_download_member(object,property){return self.area_download_get(object);}
        if self.area_type_member(object,property){return self.area_type_get(object);}
        if self.area_hreflang_member(object,property){return self.area_hreflang_get(object);}
        if self.area_target_member(object,property){return self.area_target_get(object);}
        if self.area_rel_member(object,property){return self.area_rel_get(object);}
        if self.area_href_member(object,property){return self.area_href_get(object);}
        if self.area_url_member(object,property){return self.area_url_get(object,property);}
        if self.area_referrer_policy_member(object,property){return self.area_referrer_policy_get(object);}
        if self.area_ping_member(object,property){return self.area_ping_get(object);}
        if self.map_name_member(object,property){return self.map_name_get(object);}
        if self.param_name_member(object,property){return self.param_name_get(object);}
        if self.param_metadata_member(object,property){return self.param_metadata_get(object,property);}
        if self.mod_datetime_member(object,property){return self.mod_datetime_get(object);}
        if self.data_value_member(object,property){return self.data_value_get(object);}
        if self.heading_align_member(object,property){return self.heading_align_get(object);}
        if self.ul_type_member(object,property){return self.ul_type_get(object);}
        if self.li_type_member(object,property){return self.li_type_get(object);}
        if self.ol_type_member(object,property){return self.ol_type_get(object);}
        if self.input_autocomplete_member(object,property){return self.input_autocomplete_get(object);}
        if self.input_form_action_member(object,property){return self.input_form_action_get(object);}
        if self.input_form_method_member(object,property){return self.input_form_method_get(object);}
        if self.input_form_enctype_member(object,property){return self.input_form_enctype_get(object);}
        if self.button_form_action_member(object,property){return self.button_form_action_get(object);}
        if self.button_form_method_member(object,property){return self.button_form_method_get(object);}
        if self.button_form_enctype_member(object,property){return self.button_form_enctype_get(object);}
        if self.form_autocomplete_member(object,property){return self.form_autocomplete_get(object);}
        if self.form_method_member(object,property){return self.form_method_get(object);}
        if self.form_action_member(object,property){return self.form_action_get(object);}
        if self.form_enctype_member(object,property){return self.form_enctype_get(object);}
        if self.form_name_member(object,property){return self.form_name_get(object);}
        if self.button_name_member(object,property){return self.button_name_get(object);}
        if self.input_name_member(object,property){return self.input_name_get(object);}
        if self.form_accept_charset_member(object,property){return self.form_accept_charset_get(object);}
        if self.form_rel_member(object,property){return self.form_rel_get(object);}
        if self.form_target_member(object,property){return self.form_target_get(object);}
        if self.button_form_target_member(object,property){return self.button_form_target_get(object);}
        if self.input_form_target_member(object,property){return self.input_form_target_get(object);}
        if self.form_no_validate_member(object,property){return self.form_no_validate_get(object);}
        if self.button_form_no_validate_member(object,property){return self.button_form_no_validate_get(object);}
        if self.input_form_no_validate_member(object,property){return self.input_form_no_validate_get(object);}
        if self.select_autocomplete_member(object,property){return self.select_autocomplete_get(object);}
        if self.textarea_autocomplete_member(object,property){return self.textarea_autocomplete_get(object);}
        if self.textarea_wrap_member(object,property){return self.textarea_wrap_get(object);}
        if self.button_value_member(object,property){return self.button_value_get(object);}
        if self.button_type_member(object,property){return self.button_type_get(object);}
        if self.input_step_member(object,property){return self.input_step_get(object);}
        if self.input_max_member(object,property){return self.input_max_get(object);}
        if self.input_min_member(object,property){return self.input_min_get(object);}
        if self.input_pattern_member(object,property){return self.input_pattern_get(object);}
        if self.li_value_member(object,property){return self.li_value_get(object);}
        if self.pre_width_member(object,property){return self.pre_width_get(object);}
        if self.textarea_dimensions_member(object,property){return self.textarea_dimensions_get(object,property);}
        if self.ol_start_member(object,property){return self.ol_start_get(object);}
        if self.textarea_min_length_member(object,property){return self.textarea_min_length_get(object);}
        if self.input_min_length_member(object,property){return self.input_min_length_get(object);}
        if self.textarea_max_length_member(object,property){return self.textarea_max_length_get(object);}
        if self.input_max_length_member(object,property){return self.input_max_length_get(object);}
        if self.ul_compact_member(object,property){return self.ul_compact_get(object);}
        if self.dir_compact_member(object,property){return self.dir_compact_get(object);}
        if self.menu_compact_member(object,property){return self.menu_compact_get(object);}
        if self.ol_compact_member(object,property){return self.ol_compact_get(object);}
        if self.hr_no_shade_member(object,property){return self.hr_no_shade_get(object);}
        if self.area_no_href_member(object,property){return self.area_no_href_get(object);}
        if self.option_disabled_member(object,property){return self.option_disabled_get(object);}
        if self.select_disabled_member(object,property){return self.select_disabled_get(object);}
        if self.button_disabled_member(object,property){return self.button_disabled_get(object);}
        if self.input_disabled_member(object,property){return self.input_disabled_get(object);}
        if self.textarea_disabled_member(object,property){return self.textarea_disabled_get(object);}
        if self.textarea_required_member(object,property){return self.textarea_required_get(object);}
        if self.input_required_member(object,property){return self.input_required_get(object);}
        if self.textarea_read_only_member(object,property){return self.textarea_read_only_get(object);}
        if self.input_read_only_member(object,property){return self.input_read_only_get(object);}
        if self.select_required_member(object,property){return self.select_required_get(object);}
        if self.option_default_selected_member(object,property){return self.option_default_selected_get(object);}
        if self.optgroup_disabled_member(object,property){return self.optgroup_disabled_get(object);}
        if self.ol_reversed_member(object,property){return self.ol_reversed_get(object);}
        if self.dl_compact_member(object,property){return self.dl_compact_get(object);}
        if self.paragraph_align_member(object,property){return self.paragraph_align_get(object);}
        if self.hr_align_member(object,property){return self.hr_align_get(object);}
        if self.hr_color_member(object,property){return self.hr_color_get(object);}
        if self.font_size_member(object,property){return self.font_size_get(object);}
        if self.font_face_member(object,property){return self.font_face_get(object);}
        if self.font_color_member(object,property){return self.font_color_get(object);}
        if self.br_clear_member(object,property){return self.br_clear_get(object);}
        if self.hr_size_member(object,property){return self.hr_size_get(object);}
        if self.hr_width_member(object,property){return self.hr_width_get(object);}
        if self.legend_align_member(object,property){return self.legend_align_get(object);}
        if self.div_align_member(object,property){return self.div_align_get(object);}
        if self.fieldset_member(object,property){return self.fieldset_get(object,match property{"form"=>script_fieldset::FORM,"name"=>script_fieldset::NAME_GET,"disabled"=>script_fieldset::DISABLED_GET,_=>script_fieldset::TYPE});}
        if self.control_form_member(object,property){return self.control_form_get(object,self.control_form_method(object).unwrap());}
        if self.output_metadata_member(object,property){return self.output_metadata_get(object,property);}
        if self.control_validation_member(object,property)&&property=="validationMessage"{return self.control_validation_message(object);}
        if self.control_eligibility_member(object,property){return self.control_eligibility_get(object);}
        if self.fieldset_validation_readonly(object,property){return self.fieldset_validation(object,if property=="willValidate"{script_fieldset_validation::WILL_GET}else{script_fieldset_validation::MESSAGE_GET});}
        if self.output_validation_readonly(object,property){return self.output_validation(object,if property=="willValidate"{script_output_validation::WILL_GET}else{script_output_validation::MESSAGE_GET});}
        if property=="validity"&&self.control_eligibility_class(object).is_some(){let class=self.control_eligibility_class(object).unwrap();let op=crate::javascript::CONTROL_VALIDITY_MEMBERS.iter().find(|&&(c,_)|c==class).unwrap().1;return self.output_validity_call(object,op,&[]);}
        if property=="validity"&&(self.html_output_receiver(object)||self.fieldset_receiver(object)){return self.output_validity_call(object,if self.fieldset_receiver(object){script_output_validity_state::FIELD_GET}else{script_output_validity_state::GET},&[]);}
        if self.validity_member(object,property){let method=script_output_validity_state::FLAGS.iter().find(|&&(key,_)|key==property).unwrap().1;return self.output_validity_call(object,method,&[]);}
        if self.output_value_member(object,property){return self.output_value_get(object,property);}
        if self.html_label_control_member(object,property){return self.html_label_control_get(object);}
        if self.html_label_for_member(object,property){return self.html_label_for_get(object);}
        if self.element_class_list_member(object,property){return self.element_class_list_get(object);}
        if self.output_html_for_member(object,property){return self.output_html_for_get(object);}
        if self.area_rel_list_member(object,property){return self.area_rel_list_get(object);}
        if self.form_rel_list_member(object,property){return self.form_rel_list_get(object);}
        if self.html_reflection_member(object,property){return self.html_reflection_get(object,property);}
        if let Some(value)=self.xhr_get(object,property){return value;}
        if self.placeholder_member(object,property){return self.placeholder_get(object);}

        if self.range_owner(object).is_some() { self.state.work(1)?;return Ok(Value::Undefined); }
        if self.named_map_receiver(object)&&script_named_map::prototype_member(property){return Ok(Value::Undefined);}
        if self.collection_receiver(object)&&self.collection_prototype_member(property){return Ok(Value::Undefined);}
        if parent_children::member(property) && self.parent_node_receiver(object) {return self.parent_node_get(object,property);}

        if property == "innerHTML" && self.template_receiver(object) { return self.template_html(object); }
        if self.inert_interface(object).is_some() { return self.inert_get(object, property); }
        if object == DOCUMENT && property == "implementation" {
            self.state.work(1)?;
            return Ok(Value::Host(script_inert_documents::MAIN_IMPLEMENTATION));
        }
        if self.canvas_context_receiver(object) {
            self.state.work(1)?;
            if script_canvas_context::unsupported_state(property) {
                return Err(format!("Canvas 2D drawing state {property:?} is not implemented."));
            }
            return Ok(Value::Undefined);
        }
        self.state.work(1)?;
        if self.content_editable_member(object,property) {return Ok(Value::Undefined);}
        if script_computed_style::index(object).is_some() {return self.computed_style_get(object,property);}
        if self.node_list_iteration_member(object,property)&&property!="length" {return Ok(Value::Undefined);}
        if self.element_selector_member(object,property) {return Ok(Value::Undefined);}
        if self.assigned_slot_lookup(object,property) || self.text_prototype_property(object,property) {return Ok(Value::Undefined);}
        if self.node_tree_method_member(object,property) {return Ok(Value::Undefined);}
        if script_named_map::element(object).is_some() { return self.named_map_get(object, property); }
        if script_class_list::element(object).is_some() { return self.class_list_get(object,property); }
        if property=="submitter"&&self.submit_event_receiver(object){return self.submit_event_value(object);}
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
        if let Some(value) = self.trusted_mouse_get(object, property) { return value; }
        if (EVENT_BASE..script_class_list::BASE).contains(&object) {
            let event = self
                .state
                .event
                .as_ref()
                .filter(|e| e.handle == object)
                .ok_or("Event handle has expired.")?;
            return Ok(match property {
                "type" => Value::String(event.name.clone()),
                "target" | "srcElement" => Value::Host(event.target),
                "currentTarget" => Value::Host(event.current),
                "cancelable" => Value::Bool(event.cancelable),
                "bubbles" => Value::Bool(event.bubbles),
                "eventPhase" => Value::Number(f64::from(event.phase)),
                "defaultPrevented" => Value::Bool(event.prevented),
                "returnValue" => Value::Bool(!event.prevented),
                "cancelBubble" => Value::Bool(event.stopped),
                "timeStamp" => Value::Number(event.timestamp),
                "relatedTarget" if event.focus.is_some() => event.focus.unwrap().map_or(Value::Null,Value::Host),
                "which" if event.focus.is_some() => Value::Number(0.0),
                "isTrusted" => Value::Bool(true),
                "composed" => Value::Bool(event.composed),
                "view" if event.keyboard.is_some() || event.focus.is_some() => Value::Host(WINDOW),
                "detail" if event.keyboard.is_some() || event.focus.is_some() => Value::Number(0.0),
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
            if property=="cssText"{return self.get_css_text(node);}
            if property == "length" { return Ok(Value::Number(self.inline_names(node)?.len() as f64)); }
            if let Some(index) = collection_index(property) { return Ok(self.inline_index(node, index)?.unwrap_or(Value::Undefined)); }
            // Feature detection reads the actual names exposed by this declaration.
            let Some(name) = supported_css_name(property) else { return Ok(Value::Undefined); };
            return self.get_style(node, &name);
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
            if let Some(value)=self.document_node_get(property)? {return Ok(value);}
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
                    self.state.rebuild(self.doc)?;
                    if let Some(node)=self.state.tree.title {
                        return self.title_child_text(node_handle(node),true).map(Self::dom_string_owned);
                    }
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
                "activeElement"=>self.document_active_element(object),
                "readyState" => Ok(Value::String(self.state.ready.into())),
                "defaultView" => Ok(Value::Host(WINDOW)),
                // This browser does not send Referer or populate a document
                // referrer. Expose the actual empty value of that policy.
                "referrer" => Ok(Value::String(String::new())),
                // Report the actual native document mode. Full quirks layout
                // consumers remain a separate integration contract.
                "compatMode" => Ok(Value::String(self.doc.mode.compat_mode().into())),
                "doctype"=>{self.state.work(self.doc.nodes[0].children.len())?;Ok(self.doc.nodes[0].children.iter().copied().find(|&node|matches!(self.doc.nodes[node].kind,NodeKind::DocumentType(_))).map_or(Value::Null,|node|Value::Host(node_handle(node))))},
                "prerendering" => Ok(Value::Bool(false)),
                "images" => Ok(Value::Host(DOCUMENT_IMAGES)),
                "childNodes" => self.create_child_list(DOCUMENT),
                _ => self.named_document_property(property),
            };
        }
        if let Some(value) = self.hidden_input_get(object,property)? {return Ok(value);}
        if let Some(value) = self.button_get(object,property)? {return Ok(value);}
        if let Some(value) = self.text_input_metadata_get(object,property)? {return Ok(value);}
        if let Some(value) = self.dom_get(object, property)? {
            return Ok(value);
        }
        let node = self.node(object)?;
        if self.doc.nodes[node].namespace==crate::html::Namespace::Html && tag(self.doc,node)!="form"
            && matches!(property,"prototype"|"setCapture"|"releaseCapture")
            || property=="animate" && self.html_element_expando_receiver(object) {
            return Ok(Value::Undefined);
        }
        if let Some(value) = self.image_get(node, property)? {
            return Ok(value);
        }
        match property {
            "tagName" | "nodeName" => self.string(&dom_tree::element_node_name(self.doc, node)),
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
                        &mut self.state.copy_work,
                    )?
                } else if offset {
                    return Err("Offset dimensions require an attached renderer for this connected Element.".into());
                } else {
                    // A pure script test has no attached viewport/layout box.
                    0.0
                };
                Ok(Value::Number(value))
            }
            "textContent" => Ok(Self::dom_string_owned(self.text_content(node)?)),
            "style" => Ok(Value::Host(STYLE_BASE + node as u32)),
            "dataset" => Ok(Value::Host(DATASET_BASE + node as u32)),
            "id" | "className" | "title" => {
                if property=="className"&&self.html_class_element(node){return self.class_value(node,"class");}
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
            "defaultChecked" if self.input_receiver(object) => {
                Ok(Value::Bool(self.doc.attr(node, "checked").is_some()))
            }
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
                        if self.input_receiver(object) {
                            Ok(Value::Bool(control.checked))
                        } else {
                            Err("checked requires an input element.".into())
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
            _ => {
                let property = prefix(property, 160);
                let name = prefix(tag(self.doc, node), 80);
                let namespace = self.doc.nodes[node].namespace.element_uri().unwrap_or("unknown");
                Err(format!("Element property {property:?} is not supported. Receiver tag={name}, namespace={namespace}, owner=main document."))
            }
        }
    }

    fn promise_returning_operation(&self,object:u32,method:&str)->bool {
        method=="\0Media.play"&&self.event_target_interface(object)
            || method=="play"&&self.media_element_receiver(object)
    }
    fn validate_promise_operation(&self,object:u32,method:&str)->Result<(),crate::javascript::NativePromiseFailure> {
        use crate::javascript::NativePromiseFailure as Failure;
        let method=script_media::interface_operation(method).unwrap_or(method);
        if method=="play"&&self.media_interface_receiver(object)&&!self.media_element_receiver(object) {
            return Err(Failure::Dom("NotSupportedError","This media element has no implemented player binding."));
        }
        if method!="play"||!self.media_element_receiver(object) {return Err(Failure::Fatal("Media Promise operation is unavailable.".into()));}
        if !self.state.enabled||!self.doc.scripting_enabled {return Err(Failure::Dom("InvalidStateError","Scripting is disabled for this document."));}
        if !self.state.has_transient_activation(Instant::now()) {return Err(Failure::Dom("NotAllowedError","Playback requires trusted input in this media element's Window."));}
        let snapshot=self.state.embedded_video.as_ref().unwrap();
        if snapshot.error_code==Some(4) {return Err(Failure::Dom("NotSupportedError","The selected media source is not supported."));}
        if snapshot.error_code==Some(1) {return Err(Failure::Fatal("The media controller is stopped after a resource failure.".into()));}
        Ok(())
    }
    fn call_promise_operation(&mut self,object:u32,method:&str,promise:crate::javascript::NativePromise)->Result<(),crate::javascript::NativePromiseFailure> {
        let method=script_media::interface_operation(method).unwrap_or(method);
        if method!="play" {return Err(crate::javascript::NativePromiseFailure::Fatal("Unknown media Promise operation.".into()));}
        self.play_media(node_index(object).ok_or_else(||crate::javascript::NativePromiseFailure::Fatal("Invalid media element.".into()))?,promise)
    }
    fn peek_native_promise_effect(&self)->Option<(crate::javascript::NativePromise,Option<(&'static str,&'static str)>)> {
        let effect=self.state.media_play.peek_effect()?;
        let reason=effect.rejection.map(|reason|match reason {
            script_media_play::Rejection::Abort=>("AbortError","The play request was interrupted."),
            script_media_play::Rejection::NotSupported=>("NotSupportedError","The selected media source is not supported."),
        });
        Some((effect.token,reason))
    }
    fn finish_native_promise_effect(&mut self) {self.state.media_play.commit_effect();}

    fn native_operation_caller(&mut self, window: Option<u32>) {
        self.state.navigation_incumbent = window;
    }

    fn reset_output_text(&mut self,target:u32,value:Value)->Result<(),String>{self.reset_output_value(target,value)}
    fn prepare_node_normalize(&mut self,target:u32)->Result<crate::javascript::NativeNormalizeInfo,String>{self.prepare_shared_normalize(target)}
    fn commit_node_normalize(&mut self,target:u32)->Result<(),String>{self.commit_shared_normalize(target)}
    fn abort_node_normalize(&mut self,target:u32){self.abort_shared_normalize(target);}
    fn document_location_owner(&self, target:u32)->Option<u32>{
        if target==DOCUMENT || self.inert_document(target) {return Some(WINDOW);}
        self.frame_contains_node(target).and_then(|(owner,node)|(node==0).then_some(owner))
    }
    fn document_location_access(&self,caller:Option<u32>,target:u32)->Result<(),String>{
        let owner=self.document_location_owner(target).ok_or("Document location requires a genuine Document.")?;
        if caller.or(Some(WINDOW))!=Some(owner){self.frame_window_access(caller.or(Some(WINDOW)),owner)?;}
        if owner!=WINDOW {self.contains_child_document(owner)?;}
        Ok(())
    }
    fn document_location_value(&mut self,caller:Option<u32>,target:u32)->Result<Value,String>{
        self.document_location_access(caller,target)?;
        if target==DOCUMENT {return self.location_handle();}
        if self.inert_document(target){return Ok(Value::Null);}
        let owner=self.document_location_owner(target).unwrap();
        self.frame_context(owner).map(|context|Value::Host(context.location)).ok_or("Child Document location is unavailable.".into())
    }
    fn child_removal_parent(&mut self,object:u32)->Result<Option<u32>,String>{
        self.native_node_access(object).map_err(|(_,message)|message)?;
        if self.inert_node_type(object).is_some(){return Ok(self.inert_observation_parent(object));}
        let node=self.dom_node(object)?;
        Ok(self.state.tree.parents[node].map(|parent|if parent==0{DOCUMENT}else{node_handle(parent)}))
    }
    fn adjacent_receiver_access(&self,target:u32)->Result<(),(&'static str,String)>{self.adjacent_access(target)}
    fn adjacent_destination(&mut self,target:u32,pos:adjacent_interface::Position,text:bool)->Result<Option<adjacent_interface::Destination>,(&'static str,String)>{self.resolve_adjacent(target,pos,text)}
    fn commit_adjacent_text(&mut self,target:u32,pos:adjacent_interface::Position,dest:adjacent_interface::Destination,text:&Value)->Result<(),(&'static str,String)>{self.publish_adjacent_text(target,pos,dest,text)}
    fn prepare_text_split(&mut self,target:u32,args:&[Value])->Result<crate::javascript::NativeTextSplitInfo,(&'static str,String)>{self.prepare_native_text_split(target,args)}
    fn commit_text_split(&mut self,target:u32)->Result<u32,String>{self.commit_native_text_split(target)}
    fn abort_text_split(&mut self,target:u32){self.abort_native_text_split(target);}
    fn prepare_character_data_edit(&mut self,target:u32,method:&str,args:&[Value])->Result<crate::javascript::NativeCharacterDataEdit,String>{self.native_character_edit(target,method,args)}
    fn set_character_data_splice(&mut self,target:u32,value:Value,offset:usize,removed:usize,inserted:usize)->Result<(),String>{self.write_character_splice(target,value,offset,removed,inserted)}
    fn set(&mut self, object: u32, property: &str, value: Value) -> Result<(), String> {
        if property == "nodeValue" && self.text_content_receiver(object) {
            self.native_node_access(object).map_err(|(_, message)| message)?;
            if !self.attr_receiver(object) && !self.character_data_receiver(object) { return Ok(()); }
        }
        if self.doctype_receiver(object)&&matches!(property,"nodeValue"|"textContent"){self.state.work(1)?;return Ok(());}
        if property=="text" && self.title_receiver(object) {return self.set_title_text(object,&value);}
        if property!="origin" && script_anchor_url::property(property) && self.native_anchor(object){return self.anchor_reflection_set(object,property,&value);}
        if self.charset_member(object, property) { return self.charset_reflection_set(object, &value); }
        if self.meta_member(object, property) { return self.meta_set(object, meta_attribute(property).unwrap(), &value); }
        if property=="id"&&self.canonical_selector_receiver(object){return self.element_id_set(object,&value);}
        if property=="className"&&self.canonical_selector_receiver(object){return self.element_class_name_set(object,&value);}
        if self.html_label_for_member(object,property){return self.html_label_for_set(object,&value);}
        if self.meter_numeric_attribute(object,property).is_some(){return self.meter_numeric_set(object,property,&value);}
        if self.progress_numeric_attribute(object,property).is_some(){return self.progress_numeric_set(object,property,&value);}
        if self.fieldset_disabled_member(object,property){return self.fieldset_disabled_set(object,&value);}
        if self.time_datetime_member(object,property){return self.time_datetime_set(object,&value);}
        if self.area_alt_member(object,property){return self.area_alt_set(object,&value);}
        if self.optgroup_label_member(object,property){return self.optgroup_label_set(object,&value);}
        if self.option_label_member(object,property){return self.option_label_set(object,&value);}
        if self.option_text_member(object,property){return self.option_text_set(object,&value);}
        if self.textarea_default_value_member(object,property){return self.textarea_default_value_set(object,&value);}
        if self.option_value_member(object,property){return self.option_value_set(object,&value);}
        if self.select_name_member(object,property){return self.select_name_set(object,&value);}
        if self.textarea_name_member(object,property){return self.textarea_name_set(object,&value);}
        if self.area_coords_member(object,property){return self.area_coords_set(object,&value);}
        if self.area_shape_member(object,property){return self.area_shape_set(object,&value);}
        if self.area_download_member(object,property){return self.area_download_set(object,&value);}
        if self.area_type_member(object,property){return self.area_type_set(object,&value);}
        if self.area_hreflang_member(object,property){return self.area_hreflang_set(object,&value);}
        if self.area_target_member(object,property){return self.area_target_set(object,&value);}
        if self.area_rel_member(object,property){return self.area_rel_set(object,&value);}
        if self.area_href_member(object,property){return self.area_href_set(object,&value);}
        if self.area_url_member(object,property){return self.area_url_set(object,property,&value);}
        if self.area_referrer_policy_member(object,property){return self.area_referrer_policy_set(object,&value);}
        if self.area_ping_member(object,property){return self.area_ping_set(object,&value);}
        if self.map_name_member(object,property){return self.map_name_set(object,&value);}
        if self.param_name_member(object,property){return self.param_name_set(object,&value);}
        if self.param_metadata_member(object,property){return self.param_metadata_set(object,property,&value);}
        if self.mod_datetime_member(object,property){return self.mod_datetime_set(object,&value);}
        if self.data_value_member(object,property){return self.data_value_set(object,&value);}
        if self.heading_align_member(object,property){return self.heading_align_set(object,&value);}
        if self.ul_type_member(object,property){return self.ul_type_set(object,&value);}
        if self.li_type_member(object,property){return self.li_type_set(object,&value);}
        if self.ol_type_member(object,property){return self.ol_type_set(object,&value);}
        if self.input_autocomplete_member(object,property){return self.input_autocomplete_set(object,&value);}
        if self.input_form_action_member(object,property){return self.input_form_action_set(object,&value);}
        if self.input_form_method_member(object,property){return self.input_form_method_set(object,&value);}
        if self.input_form_enctype_member(object,property){return self.input_form_enctype_set(object,&value);}
        if self.button_form_action_member(object,property){return self.button_form_action_set(object,&value);}
        if self.button_form_method_member(object,property){return self.button_form_method_set(object,&value);}
        if self.button_form_enctype_member(object,property){return self.button_form_enctype_set(object,&value);}
        if property=="autocomplete"&&self.form_autocomplete_receiver(object){return self.form_autocomplete_set(object,&value);}
        if property=="method"&&self.form_method_receiver(object){return self.form_method_set(object,&value);}
        if property=="action"&&self.form_action_receiver(object){return self.form_action_set(object,&value);}
        if matches!(property,"enctype"|"encoding")&&self.form_enctype_receiver(object){return self.form_enctype_set(object,&value);}
        if property=="name"&&self.form_name_receiver(object){return self.form_name_set(object,&value);}
        if property=="name"&&self.button_name_receiver(object){return self.button_name_set(object,&value);}
        if property=="name"&&self.input_name_receiver(object){return self.input_name_set(object,&value);}
        if property=="acceptCharset"&&self.form_accept_charset_receiver(object){return self.form_accept_charset_set(object,&value);}
        if property=="rel"&&self.form_rel_receiver(object){return self.form_rel_set(object,&value);}
        if property=="target"&&self.form_target_receiver(object){return self.form_target_set(object,&value);}
        if property=="formTarget"&&self.button_form_target_receiver(object){return self.button_form_target_set(object,&value);}
        if property=="formTarget"&&self.input_form_target_receiver(object){return self.input_form_target_set(object,&value);}
        if property=="noValidate"&&self.form_no_validate_receiver(object){return self.form_no_validate_set(object,&value);}
        if property=="formNoValidate"&&self.button_form_no_validate_receiver(object){return self.button_form_no_validate_set(object,&value);}
        if property=="formNoValidate"&&self.input_form_no_validate_receiver(object){return self.input_form_no_validate_set(object,&value);}
        if self.select_autocomplete_member(object,property){return self.select_autocomplete_set(object,&value);}
        if self.textarea_autocomplete_member(object,property){return self.textarea_autocomplete_set(object,&value);}
        if self.textarea_wrap_member(object,property){return self.textarea_wrap_set(object,&value);}
        if self.button_value_member(object,property){return self.button_value_set(object,&value);}
        if self.button_type_member(object,property){return self.button_type_set(object,&value);}
        if self.input_step_member(object,property){return self.input_step_set(object,&value);}
        if self.input_max_member(object,property){return self.input_max_set(object,&value);}
        if self.input_min_member(object,property){return self.input_min_set(object,&value);}
        if self.input_pattern_member(object,property){return self.input_pattern_set(object,&value);}
        if self.li_value_member(object,property){return self.li_value_set(object,&value);}
        if self.pre_width_member(object,property){return self.pre_width_set(object,&value);}
        if self.textarea_dimensions_member(object,property){return self.textarea_dimensions_set(object,property,&value);}
        if self.ol_start_member(object,property){return self.ol_start_set(object,&value);}
        if self.textarea_min_length_member(object,property){return self.textarea_min_length_set(object,&value);}
        if self.input_min_length_member(object,property){return self.input_min_length_set(object,&value);}
        if self.textarea_max_length_member(object,property){return self.textarea_max_length_set(object,&value);}
        if self.input_max_length_member(object,property){return self.input_max_length_set(object,&value);}
        if self.ul_compact_member(object,property){return self.ul_compact_set(object,&value);}
        if self.dir_compact_member(object,property){return self.dir_compact_set(object,&value);}
        if self.menu_compact_member(object,property){return self.menu_compact_set(object,&value);}
        if self.ol_compact_member(object,property){return self.ol_compact_set(object,&value);}
        if self.hr_no_shade_member(object,property){return self.hr_no_shade_set(object,&value);}
        if self.area_no_href_member(object,property){return self.area_no_href_set(object,&value);}
        if self.option_disabled_member(object,property){return self.option_disabled_set(object,&value);}
        if self.select_disabled_member(object,property){return self.select_disabled_set(object,&value);}
        if self.button_disabled_member(object,property){return self.button_disabled_set(object,&value);}
        if self.input_disabled_member(object,property){return self.input_disabled_set(object,&value);}
        if self.textarea_disabled_member(object,property){return self.textarea_disabled_set(object,&value);}
        if self.textarea_required_member(object,property){return self.textarea_required_set(object,&value);}
        if self.input_required_member(object,property){return self.input_required_set(object,&value);}
        if self.textarea_read_only_member(object,property){return self.textarea_read_only_set(object,&value);}
        if self.input_read_only_member(object,property){return self.input_read_only_set(object,&value);}
        if self.select_required_member(object,property){return self.select_required_set(object,&value);}
        if self.option_default_selected_member(object,property){return self.option_default_selected_set(object,&value);}
        if self.optgroup_disabled_member(object,property){return self.optgroup_disabled_set(object,&value);}
        if self.ol_reversed_member(object,property){return self.ol_reversed_set(object,&value);}
        if self.dl_compact_member(object,property){return self.dl_compact_set(object,&value);}
        if self.paragraph_align_member(object,property){return self.paragraph_align_set(object,&value);}
        if self.hr_align_member(object,property){return self.hr_align_set(object,&value);}
        if self.hr_color_member(object,property){return self.hr_color_set(object,&value);}
        if self.font_size_member(object,property){return self.font_size_set(object,&value);}
        if self.font_face_member(object,property){return self.font_face_set(object,&value);}
        if self.font_color_member(object,property){return self.font_color_set(object,&value);}
        if self.br_clear_member(object,property){return self.br_clear_set(object,&value);}
        if self.hr_size_member(object,property){return self.hr_size_set(object,&value);}
        if self.hr_width_member(object,property){return self.hr_width_set(object,&value);}
        if self.legend_align_member(object,property){return self.legend_align_set(object,&value);}
        if self.div_align_member(object,property){return self.div_align_set(object,&value);}
        if self.fieldset_name_member(object,property){return self.fieldset_name_set(object,&value);}
        if self.output_name_member(object,property){return self.output_name_set(object,&value);}
        if self.output_value_member(object,property){return self.output_value_set(object,property,&value);}
        if self.aria_reflection_attribute(object,property){return self.aria_reflection_set(object,property,&value);}
        if crate::javascript::html_reflection_interface::property(property).is_some()&&self.html_reflection_receiver(object){return self.html_reflection_set(object,property,&value);}
        if property=="style"&&self.html_element_receiver(object){let node=self.node(object)?;let source=self.text(&value)?;return self.set_css_text(node,&source);}
        if self.xhr_index(object).is_some(){return self.xhr_set(object,property,value).unwrap();}
        if self.placeholder_member(object,property){return self.placeholder_set(object,&value);}

        if property == "innerHTML" && self.template_receiver(object) {
            let source = if matches!(value, Value::Null) { String::new() } else { self.text(&value)? };
            return self.set_template_html(object, &source);
        }
        if self.inert_interface(object).is_some() {
            if property=="name"&&self.html_slot_receiver(object){return self.set_inert_attribute(object,&[Value::String("name".into()),value]).map(|_|());}
            if property=="textContent"&&self.inert_text_replacement_receiver(object){let text=if matches!(value,Value::Null|Value::Undefined){DomString::new()}else{self.dom_text(&value)?};return self.replace_inert_text_content(object,text);}
            if matches!(self.inert_node_type(object),Some(3|4|7|8)) && matches!(property,"data"|"nodeValue"|"textContent"){let text=if matches!(value,Value::Null)&&property!="data"{DomString::new()}else{self.dom_text(&value)?};return self.write_inert_character_data(object,text);}
            if property == "textContent" && matches!(self.inert_node_type(object), Some(9 | 10)) { return Ok(()); }
            return Err(format!("Writing inert DOM property {property:?} is unavailable."));
        }
        if object == DOCUMENT && property == "textContent" {return Ok(());}
        if self.content_editable_attribute(object,property) {return self.content_editable_set(object,&value);}
        if self.canvas_context_receiver(object) {
            return Err(format!("Setting native Canvas 2D property {property:?} is not implemented."));
        }
        if let Some(axis) = script_scroll_offsets::axis(property).filter(|_| self.element_selector_receiver(object)) {
            let Value::Number(number) = value else {return Err("Scroll offset requires numeric conversion.".into());};
            self.element_scroll_offset(object, axis, Some(number))?;
            return Ok(());
        }
        if script_computed_style::index(object).is_some() {return Err("Computed style declarations are read-only.".into());}
        if self.button_set(object,property,&value)? {return Ok(());}
        if self.text_input_set(object,property,&value)? {return Ok(());}
        if self.hidden_input_set(object,property,&value)? {return Ok(());}
        if self.media_interface_receiver(object)&&matches!(property,"src"|"currentTime") {self.require_bound_media(object)?;}
        if property=="controls"&&self.media_interface_receiver(object) {
            self.state.work(1)?;
            let node=self.node(object)?;
            // Boolean reflection does not perform primitive coercion and does
            // not select a source or alter the existing player's intent.
            return self.attribute(node,"controls",value.truthy().then(String::new));
        }
        if property=="returnValue"&&self.event_receiver(object) {return self.set_event_return_value(object,&value);}
        if property=="currentTime" && self.media_element_receiver(object) {
            let Value::Number(seconds)=value else {return Err("Media time requires numeric conversion.".into());};
            return self.set_media_time(node_index(object).unwrap(),seconds);
        }
        if self.style_text_attribute(object, property) {
            let node = self.node(object)?;
            let text = self.text(&value)?;
            return self.attribute(node, property, Some(text));
        }
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
            if property=="cssText"{return self.set_css_text(node,&value);}
            return self.set_style(node, &css_name(property)?, &value, "");
        }
        if self.set_handler(object, property, value.clone())? {
            return Ok(());
        }
        if object == DOCUMENT && property == "title" {
            // A tree rebuild can refund old index backing. When even all that
            // credit and the old display buffer cannot fit the incoming value,
            // refuse before touching the currently charged topology cache.
            let mut credit=self.doc.title.capacity();
            if self.state.dirty {
                credit=credit.saturating_add(self.state.node_state_bytes())
                    .saturating_add(self.state.order_bytes())
                    .saturating_add(self.state.tree.ids.capacity().saturating_mul(size_of::<(usize,usize)>()))
                    .saturating_add(self.state.tree_stack.capacity().saturating_mul(size_of::<tree_order::Visit>()));
            }
            if value.bytes()>self.state.retained_limit.saturating_sub(self.state.retained.saturating_sub(credit)) {
                return Err("Document title exceeds the retained text allowance.".into());
            }
            self.state.rebuild(self.doc)?;
            if let Some(node)=self.state.tree.title {return self.set_title_text(node_handle(node),&value);}
            let value = self.text(&value)?;
            self.state
                .replace_storage(self.doc.title.capacity(), value.capacity(), value.len())?;
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
        if self.iframe_sandbox_member(object,property){return self.iframe_sandbox_set(object,value);}
        if property == "classList" {
            if !matches!(self.doc.nodes[node].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg) { return Err("Class token list requires known namespace provenance.".into()); }
            if self.html_class_element(node){return self.html_class_set(node,&value);}
            let value = self.text(&value)?;
            return self.attribute(node,"class",Some(value));
        }
        if self.canvas_member(object, property) {
            return self.canvas_dimension_set(object, property, &value);
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
                    DomString::new()
                } else {
                    self.dom_text(&value)?
                };
                self.set_text_content(node, text)
            }
            "id" | "className" | "title" => {
                if property=="className"&&self.html_class_element(node){return self.html_class_set(node,&value);}
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
            "defaultChecked" if self.input_receiver(object) => {
                self.attribute(node, "checked", value.truthy().then(String::new))
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
        if let Some(operation)=script_class_list::operation(method){
            return match operation {
                "get length"=>self.class_list_get(object,"length"),
                "get value"=>self.class_list_get(object,"value"),
                "set value"=>{self.class_list_set(object,"value",args.first().cloned().ok_or("Token value setter requires an argument.")?)?;Ok(Value::Undefined)},
                _=>self.class_list_call(object,operation,args),
            };
        }
        if method==text_whole::GETTER{return self.native_whole_text(object);}
        if method==character_substring::METHOD{return self.native_substring_data(object,args);}
        if object==WINDOW&&method=="Document"{return self.create_generic_document();}
        if script_attr::node_method(method){return match method{"getAttributeNode"=>self.get_attribute_node(object,args),"getAttributeNodeNS"=>self.get_attribute_node_ns(object,args),"removeAttributeNode"=>self.remove_attribute_node(object,args),_=>self.set_attribute_node(object,args)};}
        if let Some((name,setter))=script_attr::member_operation(method){return if setter{Err("Attr setters require the native conversion transaction.".into())}else{self.attr_get(object,name)};}
        if let Some(ns)=script_attr::factory_operation(method){return self.create_attribute_for_document(object,args,ns);}
        if let Some((query, _)) = dom_collections::operation(method) {
            self.native_node_access(object).map_err(|(_, message)| message)?;
            let value = args.first().ok_or("A live collection operation requires an argument.")?;
            return match query {
                dom_collections::Query::Tag => self.create_tag_collection(object, value),
                dom_collections::Query::Class => self.create_class_collection(object, value),
                dom_collections::Query::Namespace => self.create_namespace_collection(object, args),
            };
        }
        if let Some((name, false)) = crate::javascript::window_relationships::operation(method) {
            self.window_relationship_access(object)?;
            if object == WINDOW {
                return Ok(if matches!(name, "parent" | "top") { Value::Host(WINDOW) } else { Value::Null });
            }
            if name == "opener" && self.frame_context(object).is_some() { return Ok(Value::Null); }
            return self.frame_get_with_caller(self.state.navigation_incumbent.or(Some(WINDOW)), object, name)
                .ok_or("Unknown Window relationship receiver.")?;
        }
        if let Some((interface,setter)) = charset_operation(method) {
            self.validate_charset(object,interface).map_err(|(_,message)|message)?;
            return if setter { Err("Charset setters require the native conversion transaction.".into()) } else { self.charset_reflection_get(object) };
        }
        if let Some((_, attr, setter)) = meta_operation(method) { return if setter { Err("Meta setters require the native conversion transaction.".into()) } else { self.meta_get(object, attr) }; }
        if let Some((name,_,setter))=crate::javascript::aria_reflection_interface::operation(method){return if setter{Err("ARIA setters require the native conversion transaction.".into())}else{self.aria_reflection_get(object,name)};}
        if let Some((name,setter))=crate::javascript::html_reflection_interface::operation(method){return if setter{Err("HTML reflection setters require the native conversion transaction.".into())}else{self.html_reflection_get(object,name)};}
        if method=="XMLHttpRequest"&&self.event_target_interface(object){return self.xhr_new(object);}
        if let Some(property)=crate::javascript::xhr_interface::getter(method){return self.xhr_get(object,property).unwrap_or_else(||Err("XHR receiver is invalid.".into()));}

        let method=if method==crate::javascript::HTML_FORM_SUBMIT {"submit"}else{method};
        if crate::javascript::range_interface::operation(method) { return self.range_call(object,method,args); }
        if object==WINDOW && method=="matchMedia" {return self.create_media_query(args);}
        if matches!(method, MEDIA_QUERY_MATCHES | MEDIA_QUERY_MEDIA) {return self.media_query_get(object,method);}
        if crate::javascript::message_channel_interface::operation(method) {
            return self.message_call(object, method, args);
        }
        if method=="replaceChild"&&self.inert_replace_receiver(object){return self.replace_inert_child(object,args);}
        if matches!(method,"appendChild"|"insertBefore"|"removeChild") && self.inert_mutation_receiver(object) {
            return self.mutate_inert_children(object,method,args);
        }
        if crate::javascript::abort_interface::operation(method) {
            return self.abort_call(object, method, args);
        }
        if let Some(&(property,_))=crate::javascript::DOCUMENT_TYPE_GETTERS.iter().find(|(_,operation)|*operation==method){
            if !self.doctype_receiver(object){return Err("DocumentType getter requires a genuine accessible DocumentType.".into());}
            return self.get_with_caller(self.state.navigation_incumbent,object,property);
        }
        if method==DOCUMENT_ADOPT_NODE{return self.adopt_node(object,args);}
        if method==CREATE_CDATA_SECTION{return self.create_cdata_section(object,args);}
        if method == PROCESSING_INSTRUCTION_TARGET_GET { return self.processing_instruction_target(object); }
        if method == HTML_TEMPLATE_CONTENT_GET { return self.template_content(object); }
        if method == CREATE_PROCESSING_INSTRUCTION {
            return self.create_processing_instruction(object, args);
        }
        if object == WINDOW && method == "ProcessingInstruction" {
            return self.create_processing_instruction(DOCUMENT, args);
        }
        if method == script_inert_documents::GET { return self.get(object, "implementation"); }
        if object == WINDOW && method == script_inert_documents::parser::CONSTRUCTOR { return self.create_dom_parser(); }
        if method == script_inert_documents::parser::PARSE && self.inert_parser(object) { return self.parse_inert_document(args); }
        if method==xml_documents::CREATE{return self.create_xml_document(object,args);}
        if method==script_inert_documents::CREATE_DOCTYPE{return self.create_document_type(object,args);}
        if method == script_inert_documents::CREATE { return self.create_inert_document(args); }
        if matches!(placeholder::operation(method),Some((_,false))){return self.placeholder_get(object);}
        if method == script_inert_documents::HAS_FEATURE { return Ok(Value::Bool(true)); }
        if self.inert_factory(object, method).is_some() { return self.inert_create_node(object, method, args); }
        if self.inert_node_type(object)==Some(1)&&matches!(method,"getAttributeNS"|"getAttributeNodeNS"|"setAttributeNS"|"removeAttributeNS"){return match method{"getAttributeNS"=>self.get_attribute_ns(object,args),"getAttributeNodeNS"=>self.get_attribute_node_ns(object,args),"setAttributeNS"=>self.set_attribute_ns(object,args),_=>self.remove_attribute_ns(object,args)};}
        if method=="setAttribute"&&self.inert_node_type(object)==Some(1){return self.set_inert_attribute(object,args);}
        if matches!(method,"getAttribute"|"getAttributeNode"|"hasAttribute"|"removeAttribute") && self.inert_node_type(object) == Some(1) {
            let argument=args.first().ok_or("Attribute operation requires one argument.")?;
            if method=="getAttributeNode"{return self.get_attribute_node(object,args);}
            if method=="removeAttribute"{return self.remove_inert_attribute(object,argument);}
            let value=self.inert_element_attribute(object,argument)?;
            return Ok(if method=="hasAttribute"{Value::Bool(!matches!(value,Value::Null))}else{value});
        }
        if method == ELEMENT_INNER_HTML_GET {
            return self.get(object, "innerHTML");
        }
        if method == HTML_SLOT_NAME_GET {
            if self.inert_node_type(object).is_some(){return self.inert_get(object,"name");}
            let node = self.dom_node(object)?;
            return self.html_text_reflection_get(node, "name");
        }
        if let Some(property)=parent_children::operation(method) {return self.parent_node_get(object,property);}
        if method=="\0Node.get.baseURI" {return self.node_base_uri_get(object);}
        if let Some(property) = node_traversal_property(method) {
            return self.get_with_caller(self.state.navigation_incumbent, object, property);
        }
        if method == NODE_HAS_CHILDREN { return self.has_child_nodes(object); }
        if method==TITLE_TEXT_GET {return self.title_child_text(object,false).map(Self::dom_string_owned);}
        if method==script_element_id::GET{return self.element_id_get(object);}
        if method==script_element_class_name::GET{return self.element_class_name_get(object);}
        if script_input_labels::interface(method).is_some(){return self.html_control_labels_get(object,method);}
        if method==script_label_form::GET{return self.html_label_form_get(object);}
        if method==script_output_form::GET{return self.output_form_get(object);}
        if script_control_type::interface(method).is_some(){return self.control_type_get(object,method);}
        if method==script_form_length::GET{return self.form_length_get(object);}
        if method==script_time_datetime::SET{return Err("Time datetime setters require the native conversion transaction.".into());}
        if method==script_area_alt::SET{return Err("Area alt setters require the native conversion transaction.".into());}
        if method==script_optgroup_label::SET{return Err("OptGroup label setters require the native conversion transaction.".into());}
        if method==script_option_label::SET{return Err("OptGroup label setters require the native conversion transaction.".into());}
        if method==script_option_text::SET{return Err("Option text setters require the native conversion transaction.".into());}
        if method==script_textarea_default_value::SET{return Err("Textarea defaultValue setters require the native conversion transaction.".into());}
        if method==script_option_value::SET{return Err("OptGroup label setters require the native conversion transaction.".into());}
        if method==script_select_name::SET{return Err("OptGroup label setters require the native conversion transaction.".into());}
        if method==script_textarea_name::SET{return Err("Textarea name setters require the native conversion transaction.".into());}
        if method==script_area_coords::SET{return Err("Area coords setters require the native conversion transaction.".into());}
        if method==script_area_shape::SET{return Err("Area shape setters require the native conversion transaction.".into());}
        if method==script_area_download::SET{return Err("Area download setters require the native conversion transaction.".into());}
        if method==script_area_type::SET{return Err("Area type setters require the native conversion transaction.".into());}
        if method==script_area_hreflang::SET{return Err("Area hreflang setters require the native conversion transaction.".into());}
        if method==script_area_target::SET{return Err("Area target setters require the native conversion transaction.".into());}
        if method==script_area_rel::SET{return Err("Area rel setters require the native conversion transaction.".into());}
        if method==script_area_href::SET{return Err("Area href setters require the native conversion transaction.".into());}
        if script_area_url::operation(method).is_some_and(|(_,setter)|setter){return Err("Area URL setters require the native conversion transaction.".into());}
        if method==script_area_referrer_policy::SET{return Err("Area referrerPolicy setters require the native conversion transaction.".into());}
        if method==script_area_ping::SET{return Err("Area ping setters require the native conversion transaction.".into());}
        if method==script_map_name::SET{return Err("Map name setters require the native conversion transaction.".into());}
        if method==script_param_name::SET{return Err("Param name setters require the native conversion transaction.".into());}
        if matches!(crate::javascript::param_metadata_interface::operation(method),Some((_,true))){return Err("Param metadata setters require the native conversion transaction.".into());}
        if method==script_mod_datetime::SET{return Err("Mod datetime setters require the native conversion transaction.".into());}
        if method==script_time_datetime::GET{return self.time_datetime_get(object);}
        if method==script_area_alt::GET{return self.area_alt_get(object);}
        if method==script_optgroup_label::GET{return self.optgroup_label_get(object);}
        if method==script_option_label::GET{return self.option_label_get(object);}
        if method==script_option_text::GET{return self.option_text_get(object);}
        if method==script_textarea_default_value::GET{return self.textarea_default_value_get(object);}
        if method==script_option_value::GET{return self.option_value_get(object);}
        if method==script_select_name::GET{return self.select_name_get(object);}
        if method==script_textarea_name::GET{return self.textarea_name_get(object);}
        if method==script_area_coords::GET{return self.area_coords_get(object);}
        if method==script_area_shape::GET{return self.area_shape_get(object);}
        if method==script_area_download::GET{return self.area_download_get(object);}
        if method==script_area_type::GET{return self.area_type_get(object);}
        if method==script_area_hreflang::GET{return self.area_hreflang_get(object);}
        if method==script_area_target::GET{return self.area_target_get(object);}
        if method==script_area_rel::GET{return self.area_rel_get(object);}
        if method==script_area_href::GET{return self.area_href_get(object);}
        if script_hyperlink_stringifier::operation(method){return self.hyperlink_stringify(object,method);}
        if let Some((name,false))=script_area_url::operation(method){return self.area_url_get(object,name);}
        if method==script_area_referrer_policy::GET{return self.area_referrer_policy_get(object);}
        if method==script_area_ping::GET{return self.area_ping_get(object);}
        if method==script_map_name::GET{return self.map_name_get(object);}
        if method==script_param_name::GET{return self.param_name_get(object);}
        if let Some((property,false))=crate::javascript::param_metadata_interface::operation(method){return self.param_metadata_get(object,property);}
        if method==script_mod_datetime::GET{return self.mod_datetime_get(object);}
        if method==script_data_value::SET{return Err("Data value setters require the native conversion transaction.".into());}
        if method==script_heading_align::SET{return Err("Heading align setters require the native conversion transaction.".into());}
        if method==script_ul_type::SET{return Err("UL type setters require the native conversion transaction.".into());}
        if method==script_li_type::SET{return Err("LI type setters require the native conversion transaction.".into());}
        if method==script_ol_type::SET{return Err("OL type setters require the native conversion transaction.".into());}
        if method==script_input_autocomplete::SET{return Err("Input autocomplete setters require the native conversion transaction.".into());}
        if method==script_input_form_action::SET{return Err("Input formAction setters require the native conversion transaction.".into());}
        if method==script_input_form_method::SET{return Err("Input formMethod setters require the native conversion transaction.".into());}
        if method==script_input_form_enctype::SET{return Err("Input formEnctype setters require the native conversion transaction.".into());}
        if method==script_button_form_action::SET{return Err("Button formAction setters require the native conversion transaction.".into());}
        if method==script_button_form_method::SET{return Err("Button formMethod setters require the native conversion transaction.".into());}
        if method==script_button_form_enctype::SET{return Err("Button formEnctype setters require the native conversion transaction.".into());}
        if method==script_form_autocomplete::SET{return Err("Form autocomplete setters require the native conversion transaction.".into());}
        if method==script_form_method::SET{return Err("Form method setters require the native conversion transaction.".into());}
        if method==script_form_action::SET{return Err("Form action setters require the native conversion transaction.".into());}
        if matches!(method,script_form_enctype::SET|script_form_enctype::SET_ENCODING){return Err("Form enctype setters require the native conversion transaction.".into());}
        if method==script_form_name::SET{return Err("Form name setters require the native conversion transaction.".into());}
        if method==script_button_name::SET{return Err("Button name setters require the native conversion transaction.".into());}
        if method==script_input_name::SET{return Err("Input name setters require the native conversion transaction.".into());}
        if method==script_form_accept_charset::SET{return Err("Form acceptCharset setters require the native conversion transaction.".into());}
        if method==script_form_rel::SET{return Err("Form rel setters require the native conversion transaction.".into());}
        if method==script_form_target::SET{return Err("Form target setters require the native conversion transaction.".into());}
        if method==script_button_form_target::SET{return Err("Form target setters require the native conversion transaction.".into());}
        if method==script_input_form_target::SET{return Err("Form target setters require the native conversion transaction.".into());}
        if method==script_form_no_validate::SET{return Err("Form noValidate setters require the native conversion transaction.".into());}
        if method==script_button_form_no_validate::SET{return Err("Form formNoValidate setters require the native conversion transaction.".into());}
        if method==script_input_form_no_validate::SET{return Err("Form formNoValidate setters require the native conversion transaction.".into());}
        if method==script_select_autocomplete::SET{return Err("Select autocomplete setters require the native conversion transaction.".into());}
        if method==script_textarea_autocomplete::SET{return Err("Textarea autocomplete setters require the native conversion transaction.".into());}
        if method==script_textarea_wrap::SET{return Err("Textarea wrap setters require the native conversion transaction.".into());}
        if method==script_button_value::SET{return Err("Button value setters require the native conversion transaction.".into());}
        if method==script_button_type::SET{return Err("Button type setters require the native conversion transaction.".into());}
        if method==script_input_step::SET{return Err("Input step setters require the native conversion transaction.".into());}
        if method==script_input_max::SET{return Err("Input max setters require the native conversion transaction.".into());}
        if method==script_input_min::SET{return Err("Input min setters require the native conversion transaction.".into());}
        if method==script_input_pattern::SET{return Err("Input pattern setters require the native conversion transaction.".into());}
        if method==script_li_value::SET{return Err("LI value setters require the native conversion transaction.".into());}
        if method==script_pre_width::SET{return Err("PRE width setters require the native conversion transaction.".into());}
        if matches!(script_textarea_dimensions::operation(method),Some((_,true))){return Err("Textarea dimension setters require the native conversion transaction.".into());}
        if method==script_ol_start::SET{return Err("OL start setters require the native conversion transaction.".into());}
        if method==script_textarea_min_length::SET{return Err("Textarea minLength setters require the native conversion transaction.".into());}
        if method==script_input_min_length::SET{return Err("Input minLength setters require the native conversion transaction.".into());}
        if method==script_textarea_max_length::SET{return Err("Textarea maxLength setters require the native conversion transaction.".into());}
        if method==script_input_max_length::SET{return Err("Input maxLength setters require the native conversion transaction.".into());}
        if method==script_ul_compact::SET{return Err("UL compact setters require the native conversion transaction.".into());}
        if method==script_dir_compact::SET{return Err("Directory compact setters require the native conversion transaction.".into());}
        if method==script_menu_compact::SET{return Err("Menu compact setters require the native conversion transaction.".into());}
        if method==script_ol_compact::SET{return Err("OL compact setters require the native conversion transaction.".into());}
        if method==script_hr_no_shade::SET{return Err("HR noShade setters require the native conversion transaction.".into());}
        if method==script_area_no_href::SET{return Err("Area noHref setters require the native conversion transaction.".into());}
        if method==script_option_disabled::SET{return Err("Option disabled setters require the native conversion transaction.".into());}
        if method==script_select_disabled::SET{return Err("Select disabled setters require the native conversion transaction.".into());}
        if method==script_button_disabled::SET{return Err("Button disabled setters require the native conversion transaction.".into());}
        if method==script_input_disabled::SET{return Err("Input disabled setters require the native conversion transaction.".into());}
        if method==script_textarea_disabled::SET{return Err("Textarea disabled setters require the native conversion transaction.".into());}
        if method==script_textarea_required::SET{return Err("Textarea required setters require the native conversion transaction.".into());}
        if method==script_input_required::SET{return Err("Input required setters require the native conversion transaction.".into());}
        if method==script_textarea_read_only::SET{return Err("Textarea readOnly setters require the native conversion transaction.".into());}
        if method==script_input_read_only::SET{return Err("Input readOnly setters require the native conversion transaction.".into());}
        if method==script_select_required::SET{return Err("Select required setters require the native conversion transaction.".into());}
        if method==script_option_default_selected::SET{return Err("Option defaultSelected setters require the native conversion transaction.".into());}
        if method==script_optgroup_disabled::SET{return Err("OptGroup disabled setters require the native conversion transaction.".into());}
        if method==script_ol_reversed::SET{return Err("OL reversed setters require the native conversion transaction.".into());}
        if method==script_dl_compact::SET{return Err("DL compact setters require the native conversion transaction.".into());}
        if method==script_paragraph_align::SET{return Err("Paragraph align setters require the native conversion transaction.".into());}
        if method==script_hr_align::SET{return Err("HR align setters require the native conversion transaction.".into());}
        if method==script_hr_color::SET{return Err("HR color setters require the native conversion transaction.".into());}
        if method==script_font_size::SET{return Err("Font size setters require the native conversion transaction.".into());}
        if method==script_font_face::SET{return Err("Font face setters require the native conversion transaction.".into());}
        if method==script_font_color::SET{return Err("Font color setters require the native conversion transaction.".into());}
        if method==script_br_clear::SET{return Err("BR clear setters require the native conversion transaction.".into());}
        if method==script_hr_size::SET{return Err("HR size setters require the native conversion transaction.".into());}
        if method==script_hr_width::SET{return Err("HR width setters require the native conversion transaction.".into());}
        if method==script_legend_align::SET{return Err("Legend align setters require the native conversion transaction.".into());}
        if method==script_div_align::SET{return Err("DIV align setters require the native conversion transaction.".into());}
        if method==script_data_value::GET{return self.data_value_get(object);}
        if method==script_heading_align::GET{return self.heading_align_get(object);}
        if method==script_ul_type::GET{return self.ul_type_get(object);}
        if method==script_li_type::GET{return self.li_type_get(object);}
        if method==script_ol_type::GET{return self.ol_type_get(object);}
        if method==script_input_autocomplete::GET{return self.input_autocomplete_get(object);}
        if method==script_input_form_action::GET{return self.input_form_action_get(object);}
        if method==script_input_form_method::GET{return self.input_form_method_get(object);}
        if method==script_input_form_enctype::GET{return self.input_form_enctype_get(object);}
        if method==script_button_form_action::GET{return self.button_form_action_get(object);}
        if method==script_button_form_method::GET{return self.button_form_method_get(object);}
        if method==script_button_form_enctype::GET{return self.button_form_enctype_get(object);}
        if method==script_form_autocomplete::GET{return self.form_autocomplete_get(object);}
        if method==script_form_method::GET{return self.form_method_get(object);}
        if method==script_form_action::GET{return self.form_action_get(object);}
        if matches!(method,script_form_enctype::GET|script_form_enctype::GET_ENCODING){return self.form_enctype_get(object);}
        if method==script_form_name::GET{return self.form_name_get(object);}
        if method==script_button_name::GET{return self.button_name_get(object);}
        if method==script_input_name::GET{return self.input_name_get(object);}
        if method==script_form_accept_charset::GET{return self.form_accept_charset_get(object);}
        if method==script_form_rel::GET{return self.form_rel_get(object);}
        if method==script_form_target::GET{return self.form_target_get(object);}
        if method==script_button_form_target::GET{return self.button_form_target_get(object);}
        if method==script_input_form_target::GET{return self.input_form_target_get(object);}
        if method==script_form_no_validate::GET{return self.form_no_validate_get(object);}
        if method==script_button_form_no_validate::GET{return self.button_form_no_validate_get(object);}
        if method==script_input_form_no_validate::GET{return self.input_form_no_validate_get(object);}
        if method==script_select_autocomplete::GET{return self.select_autocomplete_get(object);}
        if method==script_textarea_autocomplete::GET{return self.textarea_autocomplete_get(object);}
        if method==script_textarea_wrap::GET{return self.textarea_wrap_get(object);}
        if method==script_button_value::GET{return self.button_value_get(object);}
        if method==script_button_type::GET{return self.button_type_get(object);}
        if method==script_input_step::GET{return self.input_step_get(object);}
        if method==script_input_max::GET{return self.input_max_get(object);}
        if method==script_input_min::GET{return self.input_min_get(object);}
        if method==script_input_pattern::GET{return self.input_pattern_get(object);}
        if method==script_li_value::GET{return self.li_value_get(object);}
        if method==script_pre_width::GET{return self.pre_width_get(object);}
        if let Some((property,false))=script_textarea_dimensions::operation(method){return self.textarea_dimensions_get(object,property);}
        if method==script_ol_start::GET{return self.ol_start_get(object);}
        if method==script_textarea_min_length::GET{return self.textarea_min_length_get(object);}
        if method==script_input_min_length::GET{return self.input_min_length_get(object);}
        if method==script_textarea_max_length::GET{return self.textarea_max_length_get(object);}
        if method==script_input_max_length::GET{return self.input_max_length_get(object);}
        if method==script_ul_compact::GET{return self.ul_compact_get(object);}
        if method==script_dir_compact::GET{return self.dir_compact_get(object);}
        if method==script_menu_compact::GET{return self.menu_compact_get(object);}
        if method==script_ol_compact::GET{return self.ol_compact_get(object);}
        if method==script_hr_no_shade::GET{return self.hr_no_shade_get(object);}
        if method==script_area_no_href::GET{return self.area_no_href_get(object);}
        if method==script_option_disabled::GET{return self.option_disabled_get(object);}
        if method==script_select_disabled::GET{return self.select_disabled_get(object);}
        if method==script_button_disabled::GET{return self.button_disabled_get(object);}
        if method==script_input_disabled::GET{return self.input_disabled_get(object);}
        if method==script_textarea_disabled::GET{return self.textarea_disabled_get(object);}
        if method==script_textarea_required::GET{return self.textarea_required_get(object);}
        if method==script_input_required::GET{return self.input_required_get(object);}
        if method==script_textarea_read_only::GET{return self.textarea_read_only_get(object);}
        if method==script_input_read_only::GET{return self.input_read_only_get(object);}
        if method==script_select_required::GET{return self.select_required_get(object);}
        if method==script_option_default_selected::GET{return self.option_default_selected_get(object);}
        if method==script_optgroup_disabled::GET{return self.optgroup_disabled_get(object);}
        if method==script_ol_reversed::GET{return self.ol_reversed_get(object);}
        if method==script_dl_compact::GET{return self.dl_compact_get(object);}
        if method==script_paragraph_align::GET{return self.paragraph_align_get(object);}
        if method==script_hr_align::GET{return self.hr_align_get(object);}
        if method==script_hr_color::GET{return self.hr_color_get(object);}
        if method==script_font_size::GET{return self.font_size_get(object);}
        if method==script_font_face::GET{return self.font_face_get(object);}
        if method==script_font_color::GET{return self.font_color_get(object);}
        if method==script_br_clear::GET{return self.br_clear_get(object);}
        if method==script_hr_size::GET{return self.hr_size_get(object);}
        if method==script_hr_width::GET{return self.hr_width_get(object);}
        if method==script_legend_align::GET{return self.legend_align_get(object);}
        if method==script_div_align::GET{return self.div_align_get(object);}
        if matches!(method,script_fieldset::NAME_SET|script_fieldset::DISABLED_SET){return Err("Fieldset setters require the native conversion transaction.".into());}
        if matches!(method,script_meter_numeric::VALUE_SET|script_meter_numeric::MIN_SET|script_meter_numeric::MAX_SET|script_meter_numeric::LOW_SET|script_meter_numeric::HIGH_SET|script_meter_numeric::OPTIMUM_SET){return Err("Meter setters require the native conversion transaction.".into());}
        if matches!(method,script_progress_numeric::VALUE_SET|script_progress_numeric::MAX_SET){return Err("Progress setters require the native conversion transaction.".into());}
        if script_meter_numeric::operation(method){return self.meter_numeric_get(object,method);}
        if script_progress_numeric::operation(method){return self.progress_numeric_get(object,method);}
        if method==script_legend_form::GET{return self.legend_form_get(object);}
        if method==script_option_form::GET{return self.option_form_get(object);}
        if method==script_option_index::GET{return self.option_index_get(object);}
        if method==script_textarea_text_length::GET{return self.textarea_text_length_get(object);}
        if method==script_map_areas::GET{return self.map_areas_get(object);}
        if method==script_datalist_options::GET{return self.datalist_options_get(object);}
        if script_fieldset::operation(method){return self.fieldset_get(object,method);}
        if script_control_form::interface(method).is_some(){return self.control_form_get(object,method);}
        if let Some((property,setter))=script_output_value::operation(method){return if setter{Err("OUTPUT value setters require the native conversion transaction.".into())}else{self.output_value_get(object,property)};}
        if method==script_output_metadata::NAME_GET{return self.output_metadata_get(object,"name");}
        if script_control_eligibility::interface(method).is_some(){return self.control_eligibility_get(object);}
        if script_fieldset_validation::operation(method){return self.fieldset_validation(object,method);}
        if script_output_validation::operation(method){return self.output_validation(object,method);}
        if crate::javascript::control_custom_validity_interface(method).is_some(){return self.control_custom_validity_call(object,args);}
        if crate::javascript::control_validation_operation(method).is_some(){return self.control_validation_message(object);}
        if script_output_validity_state::operation(method){return self.output_validity_call(object,method,args);}
        if method==script_output_metadata::TYPE_GET{return self.output_metadata_get(object,"type");}
        if method==script_output_metadata::NAME_SET{return Err("OUTPUT name setter requires the native conversion transaction.".into());}
        if method==script_label_control::GET{return self.html_label_control_get(object);}
        if method==script_label_html_for::GET{return self.html_label_for_get(object);}
        if method==script_element_class_list::GET{return self.element_class_list_get(object);}
        if method==script_output_html_for::GET{return self.output_html_for_get(object);}
        if method==script_area_rel_list::GET{return self.area_rel_list_get(object);}
        if method==script_form_rel_list::GET{return self.form_rel_list_get(object);}
        if method==script_output_html_for::SET{return Err("HTMLOutputElement.htmlFor setter requires the ordinary forwarding transaction.".into());}
        if method==script_area_rel_list::SET{return Err("HTMLAreaElement.relList setter requires the ordinary forwarding transaction.".into());}
        if method==script_form_rel_list::SET{return Err("HTMLFormElement.relList setter requires the ordinary forwarding transaction.".into());}
        if method==script_element_class_list::SET{return Err("Element.classList setter requires the ordinary forwarding transaction.".into());}
        if method==script_element_id::SET{return Err("Element.id setter requires the native conversion transaction.".into());}
        if method==script_element_class_name::SET{return Err("Element.className setter requires the native conversion transaction.".into());}
        if method==script_label_html_for::SET{return Err("HTMLLabelElement.htmlFor setter requires the native conversion transaction.".into());}
        if let Some((name,false))=anchor_url_interface::operation(method){return self.anchor_reflection_get(object,name);}
        if method == NODE_VALUE_GET { return self.canonical_node_value(object); }
        if method == NODE_TEXT_CONTENT_GET {
            return if object == DOCUMENT { Ok(Value::Null) } else { self.get(object, "textContent") };
        }
        if method == script_content_editable::GET {return self.content_editable_get(object);}
        if matches!(method, ELEMENT_STYLE_GET | ELEMENT_STYLE_SET |SVG_STYLE_GET|SVG_STYLE_SET) {
            self.state.work(1)?;
            let node = self.node(object)?;
            if matches!(method,ELEMENT_STYLE_GET|SVG_STYLE_GET) {
                return Ok(Value::Host(STYLE_BASE + node as u32));
            }
            return Err("Inline style setters must use the native conversion transaction.".into());
        }
        if let Some(operation) = CanvasOperation::parse(method) {
            return self.canvas_context_call(object, operation, args, None);
        }
        if let Some((name, false)) = script_canvas::accessor(method) {
            return self.canvas_dimension_get(object, name);
        }
        if script_iframe_sandbox::accessor(method)==Some(false) {return self.iframe_sandbox_get(object);}
        if script_textarea_interface::accessor(method)==Some(false) {return self.get(object,"value");}
        if let Some((name, false)) = script_input_accessors::accessor(method) {
            self.validate_input_accessor(object).map_err(|(_, reason)| reason)?;
            return self.get(object, name);
        }
        if object==WINDOW && method=="getComputedStyle" {return self.create_computed_style(args);}
        if script_computed_style::index(object).is_some() {return if method=="getPropertyValue" {self.computed_property_value(object,args)} else {Err("Computed style declarations are read-only.".into())};}
        if let Some(operation)=script_named_map::operation(method){return if operation=="length"{self.named_map_get(object,"length")}else{self.named_map_call(object,operation,args)};}
        if method=="\0HTMLCollection.get.length"{return self.tag_collection_get(object,"length");}
        if method=="\0HTMLCollection.item"{return self.tag_collection_call(object,"item",args);}
        if method=="\0HTMLCollection.namedItem"{return self.tag_collection_call(object,"namedItem",args);}
        if method=="\0NodeList.get.length" {self.state.work(1)?;return self.node_list_get(object,"length");}
        if method=="\0NodeList.item" {
            let index=timer_integer(args.first().ok_or("NodeList item requires an index.")?)? as u32;
            let value=self.node_list_get(object,&index.to_string())?;
            return Ok(if matches!(value,Value::Undefined){Value::Null}else{value});
        }
        let method=if let Some(operation)=script_element_selectors::operation(method) {
            self.validate_selector_operation_receiver(object,method).map_err(|(_,message)|message)?;
            operation
        } else {method};
        if let Some((family,next))=script_element_siblings::operation(method) {
            if self.element_sibling_family(object)!=Some(family) {return Err("Element sibling getter has an incompatible native receiver.".into());}
            return self.element_sibling(object,next);
        }
        if let Some(interface) = Slottable::getter(method) {return self.assigned_slot(object, interface);}
        if let Some(offset) = Offset::getter(method) {return self.html_offset(object, offset);}
        if let Some((name, false)) = script_scroll_offsets::accessor(method) {
            return self.element_scroll_offset(object, script_scroll_offsets::axis(name).unwrap(), None);
        }
        if object == WINDOW && matches!(method, "Text" | "Comment") {
            let data = match args.first() {
                None | Some(Value::Undefined) => DomString::new(),
                Some(value) => self.dom_text(value)?,
            };
            let kind = if method == "Comment" { NodeKind::Comment(data) } else { NodeKind::Text(data) };
            return self.create_dom_node(kind);
        }
        if object==WINDOW && method=="DocumentFragment" {return self.create_dom_node(NodeKind::DocumentFragment);}
        if method==crate::javascript::DOCUMENT_DOCTYPE_GET {
            if !self.document_interface_receiver(object){return Err("Document getter requires a genuine accessible Document.".into());}
            return self.get_with_caller(self.state.navigation_incumbent,object,"doctype");
        }
        if method=="\0Document.get.documentElement" {
            if !self.document_interface_receiver(object) {return Err("Document getter requires a genuine accessible Document.".into());}
            return self.get_with_caller(self.state.navigation_incumbent,object,"documentElement");
        }
        if crate::javascript::document_encoding_getter(method) {
            self.native_node_access(object).map_err(|(_, message)| message)?;
            let encoding = if object == DOCUMENT {
                self.doc.encoding
            } else if self.inert_document(object) {
                crate::text_codec::Encoding::UTF8
            } else if let Some((owner, 0)) = self.frame_contains_node(object) {
                self.contains_child_document(owner)?.encoding
            } else {
                return Err("Encoding getter requires a Document.".into());
            };
            return self.string(encoding.document_name());
        }
        if method==DOCUMENT_ACTIVE_ELEMENT_GET {return self.document_active_element(object);}
        if method == DOCUMENT_CURRENT_SCRIPT_GET {
            self.state.work(1)?;
            return Ok(if object == DOCUMENT {
                self.state.current_script.map_or(Value::Null, |node| Value::Host(node_handle(node)))
            } else {
                Value::Null
            });
        }
        if method==crate::javascript::DOCUMENT_GET_ELEMENTS_NAME{self.validate_name_list(object).map_err(|(_,e)|e)?;return self.create_document_name_list(object,args.first().ok_or("getElementsByName requires one argument.")?);}
        if let Some(document)=script_inert_documents::identifier_lookup::operation(method){return self.native_identifier_lookup(object,args,document);}
        if method==script_position::METHOD{return self.compare_document_position(object,args);}
        if script_inert_documents::node_equality::operation(method){return self.node_equality(object,args,method==script_inert_documents::node_equality::SAME);}
        if method==script_node_root::ROOT_METHOD {return self.get_root_node(object,args,None);}
        if method == script_clone_node::CLONE_METHOD { return self.clone_node(object, args.first().is_some_and(Value::truthy)); }
        if method == script_node_contains::CONTAINS_METHOD { return self.node_contains(object, args); }
        if let Some(property)=script_media::interface_getter(method) {
            self.state.work(1)?;
            if property!="controls" {self.require_bound_media(object)?;}
            return self.media_element_get(object,property)?.ok_or_else(||"Media interface getter is not implemented.".into());
        }
        let method=if let Some(operation)=script_media::interface_operation(method) {self.require_bound_media(object)?;operation} else {method};
        if let Some(property)=method.strip_prefix("\0Node.get.") {return self.get(object,property);}
        if method=="\0KeyboardEvent.getModifierState" {return self.keyboard_modifier_state(object,args);}
        if let Some(property)=method.strip_prefix("\0KeyboardEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0FocusEvent.get.") {return self.get(object,property);}
        if method=="\0SubmitEvent.get.submitter" {return self.submit_event_value(object);}
        if let Some(property)=method.strip_prefix("\0WheelEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0ErrorEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0AnimationEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0ProgressEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0MouseEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0CustomEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0UIEvent.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0Event.get.") {return self.get(object,property);}
        if let Some(property)=method.strip_prefix("\0Event.set.") {
            self.set(object,property,args.first().cloned().unwrap_or(Value::Undefined))?;
            return Ok(Value::Undefined);
        }
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
        if matches!(method,element_names::LOOKUP_URI|element_names::LOOKUP_PREFIX){return self.native_namespace_lookup(object,args,method==element_names::LOOKUP_PREFIX);}
        if method==element_names::DEFAULT_METHOD || method == "isDefaultNamespace" {
            self.state.work(1)?;
            return self.is_default_namespace(object, args);
        }
        if self.is_frame_handle(object) { return Err("Blank child native methods are not implemented.".into()); }
        self.state.work(1)?;
        if object == DOCUMENT && method == "createEvent" {
            return if self.event_factory_enabled() { self.create_event(args) }
                else { Err(unsupported_event_interface(args.first())) };
        }
        if method=="getElementsByClassName" && (object==DOCUMENT || self.attr_element_receiver(object) || matches!(self.inert_node_type(object),Some(1|9))) {
            return self.create_class_collection(object,args.first().ok_or("getElementsByClassName requires one argument.")?);
        }
        if let Some(result) = self.trusted_mouse_call(object, method, args) { return result; }
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
        if (object == DOCUMENT || node_index(object).is_some() || matches!(self.inert_node_type(object),Some(1|9))) && method == "getElementsByTagName"
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
        if object == HISTORY && matches!(method,"replaceState"|"pushState") { return if method=="pushState"{self.history_push(args)}else{self.history_replace(args)}; }
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
                    let name = match &args[0] {
                        Value::String(s)=>crate::dom_string::DomStringRef::Scalar(s),
                        Value::StringUnits(u)=>crate::dom_string::DomStringRef::Raw(u),
                        _=>return Err("Image namedItem requires its converted DOMString key.".into()),
                    };
                    self.image_named(name)?
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
        if matches!(method,"querySelector"|"querySelectorAll"|"matches"|"closest")&&self.inert_node_type(object).is_some(){
            let Some(Value::String(source))=args.first() else{return Err("Inert selector requires a converted selector string.".into());};
            return self.inert_selector_query(object,method,source);
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
            return self.native_identifier_lookup(object,args,true);
        }
        if object >= STYLE_BASE {
            let node = self.style_node(object)?;
            if method == "item" {
                let index = timer_integer(args.first().ok_or("Style item requires an index.")?)? as u32 as usize;
                return Ok(self.inline_index(node, index)?.unwrap_or_else(|| Value::String(String::new())));
            }
            if method == "setProperty" && (2..=3).contains(&args.len()) {
                let name = self.text(&args[0])?;
                let name = css_declaration_name(&name)?;
                let value = if matches!(args[1], Value::Null) {String::new()} else {self.text(&args[1])?};
                let priority = args
                    .get(2)
                    .filter(|v| !matches!(v, Value::Null | Value::Undefined))
                    .map(|v| self.text(v))
                    .transpose()?
                    .unwrap_or_default();
                self.set_style(node, &name, &value, &priority)?;
                return Ok(Value::Undefined);
            }
            if matches!(method, "getPropertyValue" | "removeProperty") && args.len() == 1 {
                let name = css_declaration_name(&self.text(&args[0])?)?;
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
            let text = self.dom_text(&args[0])?;
            let namespace = self.doc.nodes[node].namespace;
            let qualified = self.method_qualified_attribute_name(node, method, text)?;
            let slot = self.raw_attribute_slot(node, &qualified)?;
            // Uncertain foreign parser contexts retain their existing raw
            // mutation policy; only known element namespaces prove absence.
            if method == "removeAttribute" && namespace.element_uri().is_some() {
                return self.remove_attribute_at(node, slot);
            }
            if matches!(method, "getAttribute" | "hasAttribute") {
                if method == "hasAttribute" { return Ok(Value::Bool(slot.is_some())); }
                let Some(slot) = slot else { return Ok(Value::Null); };
                let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
                let value = &attrs[slot].value;
                if value.logical_bytes() > self.state.limits.host.string_bytes { return Err("Attribute value exceeds the host string bound.".into()); }
                self.state.copies(value.logical_bytes())?;
                return Self::dom_string_copy(value);
            }
            let name = if let Some(slot) = slot { self.copy_attribute_name(node, slot)? }
                else { crate::html::AttributeName::unqualified_dom(qualified) };
            let value = if method == "setAttribute" { Some(self.dom_text(&args[1])?) } else { None };
            self.public_dom_attribute(node,name,value,script_attr::InputPreparation::Pending)?;
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
fn same_event_listener(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Function(a), Value::Function(b)) | (Value::Object(a), Value::Object(b)) => a == b,
        (Value::Host(a), Value::Host(b)) => a == b,
        _ => false,
    }
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
        "onclick" | "onauxclick" | "oninput" | "onchange" | "onsubmit" | "onreset" | "oninvalid" | "onload" | "onkeydown"
        | "onkeyup" | "onerror" | "onfocus" | "onblur" => Some(&property[2..]),
        _ => None,
    }
}
const TEXT_CONTAINERS:&[&str]=&["div","span","map","ins","del","font","p","a","b","strong","i","em","u","s","small","mark","sub","sup","code","pre","listing","xmp","kbd","samp","var","label","button","h1","h2","h3","h4","h5","h6","li","dt","dd","legend","blockquote","br","wbr"];
fn text_container(tag:&str)->bool{TEXT_CONTAINERS.contains(&tag)}

fn node_bytes(node: &Node) -> usize {
    match &node.kind {
        NodeKind::DocumentFragment => 0,
        NodeKind::DocumentType(data)=>data.capacity(),
        NodeKind::Text(text) | NodeKind::Cdata(text) | NodeKind::Comment(text) => text.capacity(),
        NodeKind::ProcessingInstruction { target, data } => target.capacity().saturating_add(data.capacity()),
        NodeKind::Element { id, attrs, expanded_name, .. } => {
            id.as_ref().map_or(0, String::capacity)
                + crate::html::element_names::owned_bytes(expanded_name)
                + attrs.capacity()*size_of::<crate::html::Attribute>()
                + attrs.iter().map(crate::html::Attribute::owned_bytes).sum::<usize>()
        }
    }
}
// Compact copies need only the used payload. Reconcile any capacity returned
// by the allocator after the fallible copy, before publishing its new owner.
fn copied_node_bytes(node: &Node) -> usize {
    match &node.kind {
        NodeKind::Element { attrs, id, expanded_name, .. } => {
            id.as_ref().map_or(0, String::len)
                + crate::html::element_names::copied_bytes(expanded_name)
                + attrs.len() * size_of::<crate::html::Attribute>()
                + attrs.iter().map(crate::html::Attribute::bytes).sum::<usize>()
        }
        NodeKind::Text(text) | NodeKind::Cdata(text) | NodeKind::Comment(text) => text.used_bytes(),
        NodeKind::ProcessingInstruction { target, data } => target.len().saturating_add(data.used_bytes()),
        NodeKind::DocumentFragment => 0,
        NodeKind::DocumentType(data)=>data.used_bytes(),
    }
}
fn copied_payload_extra(kind: &NodeKind) -> usize {
    match kind {
        NodeKind::Element { attrs, id, expanded_name, .. } => {
            crate::html::element_names::owned_bytes(expanded_name)-crate::html::element_names::copied_bytes(expanded_name)
                + (attrs.capacity() - attrs.len()) * size_of::<crate::html::Attribute>()
                + attrs.iter().map(|attr| attr.owned_bytes() - attr.bytes()).sum::<usize>()
                + id.as_ref().map_or(0, |text| text.capacity() - text.len())
        }
        NodeKind::Text(text) | NodeKind::Cdata(text) | NodeKind::Comment(text) => text.capacity() - text.used_bytes(),
        NodeKind::ProcessingInstruction { target, data } => {
            target.capacity() - target.len() + data.capacity() - data.used_bytes()
        }
        NodeKind::DocumentFragment => 0,
        NodeKind::DocumentType(data)=>data.capacity()-data.used_bytes(),
    }
}
fn document_bytes(doc: &Document) -> usize {
    doc.nodes.capacity()*size_of::<Node>() + doc.title.capacity()
        + doc.nodes.iter().map(|node| node_bytes(node) + child_link_bytes(&node.children)).sum::<usize>()
}
fn child_link_bytes(children: &Vec<usize>) -> usize {
    children.capacity() * size_of::<usize>()
}
fn prefix(text: &str, maximum: usize) -> &str {
    let mut end = text.len().min(maximum);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
fn css_declaration_name(name: &str) -> Result<String, String> {
    if crate::css::computed_custom_property_name(name) { return Ok(name.to_owned()); }
    if name.len() <= 64 {
        let lower = name.to_ascii_lowercase();
        if script_inline_style::PROPERTIES.binary_search(&lower.as_str()).is_ok() { return Ok(lower); }
    }
    css_name(name)
}
fn metered_opacity<'a>(value: &'a str, state: &mut State) -> Result<Option<crate::css::SpecifiedOpacity<'a>>, String> {
    let (work, copies) = crate::css::transform_source_cost(value.len());
    state.work(work)?;
    state.copies(copies)?;
    Ok(crate::css::specified_opacity(value))
}
fn css_name(name: &str) -> Result<String, String> {
    if name.len() > 64 {
        return Err("CSS property name exceeds 64 bytes.".into());
    }
    supported_css_name(name).ok_or_else(|| format!("CSS property {name:?} is not supported by the script bridge."))
}
fn supported_css_name(name: &str) -> Option<String> {
    if name.len() > 64 { return None; }
    let mut css = String::new();
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            css.push('-');
            css.push(ch.to_ascii_lowercase());
        } else {
            css.push(ch);
        }
    }
    script_inline_style::PROPERTIES.binary_search(&css.as_str()).is_ok().then_some(css)
}
fn validate_css_value(name: &str, value: &str, string_limit: usize) -> Result<(), String> {
    if name.starts_with("--") && !crate::css::inline_custom_value(value) {
        return Err("The custom property exceeds the supported inline value syntax.".into());
    }
    if name == "background-image" {
        // Reuse the genuine single-image parser. Shared declaration boundaries
        // preserve semicolons and bangs inside URL payloads. Escaped mutation
        // values retain the bridge's explicit finite-subset refusal.
        if value.len() > string_limit || value.chars().any(|ch| {
            ch.is_control() && !ch.is_ascii_whitespace()
                || ch == '\\'
        }) {
            return Err("Background image mutation exceeds the string bound or contains unsupported controls/escapes.".into());
        }
        let keyword = value.trim();
        if keyword.is_empty() || ["none", "inherit", "initial", "unset"].iter().any(|word| keyword.eq_ignore_ascii_case(word))
            || crate::css::background_image_url(value).is_some() {
            return Ok(());
        }
        return Err("Background image mutation requires one supported URL or background-image keyword.".into());
    }
    let allow_font_quotes = matches!(name, "font" | "font-family");
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
    let mut items = crate::css::declaration_items(style);
    for item in items.by_ref().filter(|item| !item.trim().is_empty()) {
        let (name, value) = item
            .split_once(':')
            .ok_or("Style declaration needs a colon.")?;
        let raw_name = name;
        let name = if crate::css::easing_trim(raw_name).eq_ignore_ascii_case("animation-timing-function") {
            "animation-timing-function".to_owned()
        } else if name.trim().eq_ignore_ascii_case("transform") || name.trim().eq_ignore_ascii_case("opacity") {
            name.trim().to_ascii_lowercase()
        } else {
            let name = css_name(name.trim())?;
            if name == "animation-timing-function" {
                return Err("Raw easing declarations require the CSS animation-timing-function property name.".into());
            }
            name
        };
        // Priority is separate grammar. Keep the ordinary value guard on
        // everything before a genuine trailing priority marker.
        let value = if name == "animation-timing-function" { crate::css::easing_priority(value).0 }
            else if matches!(name.as_str(), "transform" | "opacity" | "touch-action") { crate::css::declaration_priority(value).0 } else { value.trim() };
        validate_css_value(&name, value, string_limit)?;
    }
    if !items.complete() { return Err("Inline style has an unterminated string or function.".into()); }
    Ok(())
}

#[cfg(test)]
#[path = "script_compat_tests.rs"]
mod compatibility_tests;

#[cfg(test)]
#[path = "script_child_nodes_tests.rs"]
mod script_child_nodes_tests;

#[cfg(test)]
#[path = "script_assigned_slot_tests.rs"]
mod script_assigned_slot_tests;

#[cfg(test)]
#[path = "script_assigned_slot_realm_tests.rs"]
mod script_assigned_slot_realm_tests;

#[cfg(test)]
#[path = "script_text_construction_tests.rs"]
mod script_text_construction_tests;

#[cfg(test)]
#[path = "script_computed_style_tests.rs"]
mod script_computed_style_tests;

#[cfg(test)]
#[path = "script_computed_style_protocol_tests.rs"]
mod script_computed_style_protocol_tests;

#[cfg(test)]
#[path = "script_computed_overflow_tests.rs"]
mod script_computed_overflow_tests;

#[cfg(test)]
#[path = "script_computed_position_tests.rs"]
mod script_computed_position_tests;
#[cfg(test)]
#[path = "script_computed_opacity_tests.rs"]
mod script_computed_opacity_tests;

#[cfg(test)]
#[path = "script_selector_negation_tests.rs"]
mod script_selector_negation_tests;

#[cfg(test)]
#[path = "script_selector_negation_protocol_tests.rs"]
mod script_selector_negation_protocol_tests;

#[cfg(test)]
#[path = "script_dom_object_string_tests.rs"]
mod script_dom_object_string_tests;

#[cfg(test)]
#[path = "script_dom_object_string_protocol_tests.rs"]
mod script_dom_object_string_protocol_tests;

#[cfg(test)]
#[path = "script_container_object_string_tests.rs"]
mod script_container_object_string_tests;

#[cfg(test)]
#[path = "script_container_object_realm_tests.rs"]
mod script_container_object_realm_tests;

#[cfg(test)]
#[path = "script_svg_element_interface_tests.rs"]
mod script_svg_element_interface_tests;

#[cfg(test)]
#[path = "script_offset_parent_tests.rs"]
mod script_offset_parent_tests;

#[cfg(test)]
#[path = "script_offset_coordinate_tests.rs"]
mod script_offset_coordinate_tests;

#[cfg(test)]
#[path = "script_scroll_offset_tests.rs"]
mod script_scroll_offset_tests;

#[cfg(test)]
#[path = "script_node_list_iteration_tests.rs"]
mod script_node_list_iteration_tests;

#[cfg(test)]
#[path = "script_node_list_iteration_protocol_tests.rs"]
mod script_node_list_iteration_protocol_tests;

#[cfg(test)]
#[path = "script_document_node_tests.rs"]
mod script_document_node_tests;

#[cfg(test)]
#[path = "script_element_sibling_tests.rs"]
mod script_element_sibling_tests;

#[cfg(test)]
#[path = "script_element_sibling_protocol_tests.rs"]
mod script_element_sibling_protocol_tests;

#[cfg(test)]
#[path = "script_element_sibling_security_tests.rs"]
mod script_element_sibling_security_tests;

#[cfg(test)]
#[path = "script_element_selector_prototype_tests.rs"]
mod script_element_selector_prototype_tests;

#[cfg(test)]
#[path = "script_element_selector_protocol_tests.rs"]
mod script_element_selector_protocol_tests;

#[cfg(test)]
#[path = "script_element_selector_security_tests.rs"]
mod script_element_selector_security_tests;

#[cfg(test)]
#[path = "script_document_own_properties_tests.rs"]
mod script_document_own_properties_tests;

#[cfg(test)]
#[path = "script_document_own_descriptor_tests.rs"]
mod script_document_own_descriptor_tests;

#[cfg(test)]
#[path = "script_document_prebound_methods_tests.rs"]
mod script_document_prebound_methods_tests;

#[cfg(test)]
#[path = "script_document_dispatch_tests.rs"]
mod script_document_dispatch_tests;

#[cfg(test)]
#[path = "script_comment_tests.rs"]
mod script_comment_tests;

#[cfg(test)]
#[path = "script_background_style_tests.rs"]
mod background_style_tests;

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
        setup_with_limits(source,scripts,location,crate::javascript::limits::CAPACITY_REGRESSION)
    }

    fn setup_with_limits(source:&str,scripts:&[&str],location:Option<&crate::navigation::Location>,limits:crate::javascript::limits::Limits) -> (Document,Forms,Page,Outcome) {
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
        let mut page = Page::with_limits(limits);
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
var rejected=false;try{p.hidden=symbol;}catch(error){rejected=error instanceof TypeError;}
if(!rejected||p.hidden||p.hasAttribute('hidden'))throw Error('hidden union Symbol refusal');
p.hidden=true; field.disabled=symbol; check.checked=symbol;
if(!p.hidden||!field.disabled||!check.checked)throw Error('boolean Symbol');
document.title=null; p.title=null; p.textContent=null; field.value=null; area.value=null;
if(document.title!=='null'||p.title!=='null'||p.textContent!==''||field.value!==''||area.value!=='')throw Error('null conversion');
p.textContent=undefined; field.value=undefined; choice.selectedIndex=null;
var text=document.createTextNode('old'); text.nodeValue=null;
if(text.data!==''||p.textContent!==''||field.value!=='undefined'||choice.value!=='a')throw Error('existing defaults');
text.data=null; if(text.data!=='null')throw Error('data null');
text.textContent=undefined; if(text.data!=='')throw Error('nullable textContent');
text.data=undefined; if(text.data!=='undefined')throw Error('non-nullable CharacterData');
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
        assert!(page.report().contains("expired"), "{}", page.report());
        assert!(page.state.event.is_none());
        assert_eq!(doc.title, "widu");
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
    fn detached_style_text_content_replaces_real_text_without_activating_a_sheet() {
        let (mut doc, mut forms, mut page, startup) = setup(
            "<title>Original</title><form><input id=neighbor value=seed></form>",
            &[],
        );
        assert_eq!(startup.errors, 0);
        page.set_geometry(crate::script_geometry::Geometry::new(
            640, 360, 1.0, &HashMap::new(), HashMap::new(), &crate::fonts::Book::empty(),
        ));
        let outcome = page.console_eval(&mut doc, &mut forms, r#"
var styleNode=document.createElement('style'), nodes=styleNode.childNodes;
var neighbor=document.getElementById('neighbor');
neighbor.value='edited';
var css='#neighbor { opacity: 0 } /* <script>plain text</script> */';
styleNode.textContent=css;
var oldText=styleNode.firstChild;
if(nodes.length!==1 || nodes[0]!==oldText || oldText.nodeType!==3 ||
   oldText.parentNode!==styleNode || oldText.ownerDocument!==document ||
   styleNode.textContent!==css || oldText.data!==css ||
   styleNode.isConnected || oldText.isConnected ||
   getComputedStyle(neighbor).opacity!=='1')throw 'first text or detached stylesheet';
styleNode.textContent='second';
if(nodes!==styleNode.childNodes || nodes.length!==1 || oldText.parentNode!==null ||
   oldText.data!==css || nodes[0]===oldText || nodes[0].data!=='second')throw 'replacement';
var second=nodes[0];
styleNode.textContent=null;
if(styleNode.textContent!=='' || nodes.length!==0 || second.parentNode!==null ||
   second.data!=='second' || neighbor.value!=='edited' || neighbor.defaultValue!=='seed')
    throw 'removal or neighboring input';
document.title='Detached text verified';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "Detached text verified");
        assert!(page.take_dynamic_stylesheets().is_empty());
        assert!(page.take_dynamic_scripts().is_empty());
        let saved = page.console_eval(&mut doc, &mut forms, "styleNode.textContent='retained';var held=styleNode.firstChild;");
        assert_eq!(saved.errors, 0, "{}", page.report());
        page.state.node_limit = doc.nodes.len();
        let before = (doc.nodes.len(), page.state.dom_commits);
        let failed = page.console_eval(&mut doc, &mut forms, "styleNode.textContent='too many nodes';");
        assert_eq!(failed.errors, 1);
        assert!(page.report().contains("DOM appended-node limit reached"));
        assert_eq!((doc.nodes.len(), page.state.dom_commits), before);
        let check = page.console_eval(&mut doc, &mut forms, "if(styleNode.firstChild!==held || held.data!=='retained' || held.parentNode!==styleNode)throw 'partial replacement';");
        assert_eq!(check.errors, 0, "{}", page.report());
    }

    #[test]
    fn location_reads_serialized_components_and_stable_native_identity() {
        let script = r#"
var l=location;
if(l!==window.location || l!==document.location || l!==self.location || l!==globalThis.location)throw Error('location identity');
if(document.URL!==l.href || document.documentURI!==l.href)throw Error('document URL');
if(!('location' in window) || !('location' in document) || !('URL' in document) || !('documentURI' in document))throw Error('URL presence');
if(!('baseURI' in document)||document.baseURI!==l.href||!('assign' in l)||!('replace' in l)||!('reload' in l))throw Error('Location capability mismatch');
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
            "<!doctype html><html><body><p id='out'>Hello</p></body></html>",
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
"#, "document.images.length=0;", "var lengthGetter=Object.getOwnPropertyDescriptor(HTMLCollection.prototype,'length').get;if(document.images.length!==6||lengthGetter.call(document.images)!==6)throw Error('mutable collection');"],
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
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
 'checked' in box, 'checked' in name, 'querySelector' in p,
 !('checked' in p), Object.getOwnPropertyDescriptor(name,'checked')===undefined,
 typeof Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'checked').get==='function',
 name.checked===false, box.checked===false,
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
    fn text_replacement_is_atomic_reachable_and_preserves_superseded_text_identities() {
        let (doc, forms, page, result) = setup_with_limits(
            "<body><div id=unsafe><span>Keep</span><input id=edit value=original></div><div id=safe><span id=removed>Old</span></div><p id=result></p><p id=loop>0</p></body>",
            &[
                "document.getElementById('unsafe').textContent='must not replace';",
                "var safe=document.getElementById('safe'),removed=document.getElementById('removed');safe.textContent='new'; document.getElementById('result').textContent=String(document.getElementById('removed') === null);if(removed.parentNode!==null||removed.textContent!=='Old')throw 'saved removed subtree';var loop=document.getElementById('loop'),children=loop.childNodes,original=children[0],previous=original;var i=0;while(i<1000){var oldData=previous.data;loop.textContent=String(i);var current=children[0];if(current===previous||previous.data!==oldData||previous.parentNode!==null||previous.isConnected)throw 'saved replaced Text';previous=current;i++;}if(original.data!=='0'||original.parentNode!==null||original.isConnected||children.length!==1)throw 'original Text retention';",
            ],
            None,
            crate::javascript::limits::BROWSER,
        );
        assert_eq!(result.errors, 1, "{}", page.report());
        assert_eq!(text(&doc, id(&doc, "unsafe")), "Keep");
        assert!(forms.controls.contains_key(&id(&doc, "edit")));
        assert_eq!(text(&doc, id(&doc, "safe")), "new");
        assert_eq!(text(&doc, id(&doc, "result")), "true");
        assert_eq!(text(&doc, id(&doc, "loop")), "999");
        let before = html::parse("<body><div id=unsafe><span>Keep</span><input id=edit value=original></div><div id=safe><span id=removed>Old</span></div><p id=result></p><p id=loop>0</p></body>").nodes.len();
        assert_eq!(doc.nodes.len(), before + 1002, "Safe/result replacements and all 1000 loop replacements create fresh Text");
        assert_eq!(page.state.detached_nodes.len(),1002,"Removed safe subtree and superseded loop Texts remain retained");
        assert!(page.state.retained>=2897,"Every new payload remains charged after old Text becomes detached");
        assert!(page.state.retained<=page.state.retained_limit);
        assert!(doc.nodes.len()<=page.state.node_limit);
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
            "<body><p id=first>First</p><form id=f></form><input id=input type=checkbox form=f value=keep><p id=out></p></body>",
            &[
                "document.getElementById('f').id='other';",
                "document.getElementById('first').setAttribute('id','f');",
                "document.getElementById('input').setAttribute('form','other');",
                "document.getElementById('first').setAttribute('onclick','alert(1)');",
                "document.getElementById('first').style.background='url(https://example.test/)';",
                "var box=document.getElementById('input');if(box.value!=='keep'||box.form!==document.getElementById('f'))throw 'refused mutation changed control';box.setAttribute('value','changed');document.getElementById('first').setAttribute('class','ready');document.getElementById('out').textContent=box.value;",
            ],
        );
        assert_eq!(result.errors, 5, "{}", page.report());
        assert_eq!(
            forms.controls[&id(&doc, "input")].owner,
            Some(id(&doc, "f"))
        );
        assert_eq!(doc.attr(id(&doc, "input"), "form"), Some("f"));
        assert_eq!(doc.attr(id(&doc, "input"), "value"), Some("changed"));
        assert_eq!(doc.attr(id(&doc, "first"), "class"), Some("ready"));
        assert_eq!(text(&doc, id(&doc, "out")), "changed");
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
        assert_eq!(page.state.host_work, 768 * 1024 * 1024);
        assert_eq!(page.state.limits.startup_host_work, 4 * page.state.host_work);
        assert_eq!(page.state.copy_work, 16 * 1024 * 1024 * 1024);
        let report = page.report();
        assert!(report.contains("1000000000 steps / 30 seconds / 16384 MiB"));
        assert!(report.contains("4000000000 steps / 120 seconds / 65536 MiB"));
        assert!(report.contains("Native work allowance: 805306368 units per job, 3221225472 units during startup."));
        assert!(report.contains("1048576 properties per object / 16777216 per document"));
        assert!(report.contains("1048576 object identities; array length 16777216; 512 MiB"));
    }
}

#[cfg(test)]
#[path = "script_timer_global_tests.rs"]
mod timer_global_tests;

#[cfg(test)]
#[path = "script_canvas_dimension_tests.rs"]
mod canvas_dimension_tests;

#[cfg(test)]
#[path = "script_canvas_protocol_tests.rs"]
mod canvas_protocol_tests;

#[cfg(test)]
#[path="script_element_attribute_storage_tests.rs"]
mod element_attribute_storage_tests;

#[cfg(test)]
#[path="script_attribute_payload_tests.rs"]
mod attribute_payload_tests;

#[cfg(test)]
#[path="script_focus_event_tests.rs"]
mod script_focus_event_tests;
#[cfg(test)]
#[path="script_keyboard_event_tests.rs"]
mod script_keyboard_event_tests;

#[cfg(test)]
#[path = "script_range_tests.rs"]
mod range_tests;

#[cfg(test)]
#[path="script_xhr_tests.rs"]
mod script_xhr_tests;

#[cfg(test)]
#[path="script_animation_event_tests.rs"]
mod animation_event_tests;

#[cfg(test)]
#[path="script_wheel_event_tests.rs"]
mod wheel_event_tests;

#[cfg(test)]
#[path="script_nested_template_tests.rs"]
mod nested_template_tests;

#[cfg(test)]
#[path="script_error_event_tests.rs"]
mod error_event_tests;

#[cfg(test)]
#[path="script_inert_selector_tests.rs"]
mod inert_selector_tests;

#[cfg(test)]
#[path="script_passive_html_import_tests.rs"]
mod passive_html_import_tests;

#[cfg(test)]
#[path="script_native_control_import_tests.rs"]
mod native_control_import_tests;

#[path="script_svg_dom.rs"]
mod svg_dom;

#[cfg(test)]
#[path="script_dynamic_svg_tests.rs"]
mod dynamic_svg_tests;

#[cfg(test)]
#[path="script_inert_observation_tests.rs"]
mod inert_observation_tests;

#[cfg(test)]
#[path="script_content_observation_tests.rs"]
mod content_observation_tests;

#[path="script_content_observation.rs"]
mod content_observation;

#[cfg(test)]
#[path="script_attribute_notification_tests.rs"]
mod attribute_notification_tests;

#[path="script_attribute_notifications.rs"]
mod attribute_notifications;

#[cfg(test)]
#[path="script_style_slot_notification_tests.rs"]
mod style_slot_notification_tests;

#[cfg(test)]
#[path="script_transient_observation_tests.rs"]
mod transient_observation_tests;

#[cfg(test)]
#[path="script_placeholder_tests.rs"]
mod placeholder_tests;

#[path="script_placeholder.rs"]
mod placeholder;

#[cfg(test)]
#[path="script_computed_presentation_tests.rs"]
mod computed_presentation_tests;

#[cfg(test)]
#[path="script_css_text_tests.rs"]
mod css_text_tests;

#[path="script_css_text.rs"]
mod css_text;

#[cfg(test)]
#[path="script_structural_selector_tests.rs"]
mod structural_selector_tests;

#[cfg(test)]
#[path="script_native_focus_selector_tests.rs"]
mod native_focus_selector_tests;

#[cfg(test)]
#[path="script_visibility_hit_tests.rs"]
mod visibility_hit_tests;

#[cfg(test)]
#[path="script_stacking_tests.rs"]
mod stacking_tests;

#[cfg(test)]
#[path="script_html_reflector_tests.rs"]
mod html_reflector_tests;

#[path="script_html_reflections.rs"]
mod html_reflections;

#[cfg(test)]
#[path="script_tab_index_reflection_tests.rs"]
mod tab_index_reflection_tests;

#[path="script_aria_reflections.rs"]
mod aria_reflections;

#[cfg(test)]
#[path="script_aria_reflection_tests.rs"]
mod aria_reflection_tests;

#[cfg(test)]
#[path="script_replace_self_tests.rs"]
mod replacement_self_tests;

#[cfg(test)]
#[path="script_svg_style_geometry_tests.rs"]
mod svg_style_geometry_tests;

#[cfg(test)]
#[path="script_console_diagnostics_tests.rs"]
mod console_diagnostics_tests;

#[path="script_document_active_element.rs"]
mod document_active_element;

#[cfg(test)]
#[path="script_document_active_element_tests.rs"]
mod document_active_element_tests;

#[cfg(test)]
#[path="script_character_data_properties_tests.rs"]
mod character_data_properties_tests;

#[cfg(test)]
#[path="script_svg_author_properties_tests.rs"]
mod svg_author_properties_tests;

#[cfg(test)]
#[path="script_fragment_author_prototype_tests.rs"]
mod fragment_author_prototype_tests;

#[cfg(test)]
#[path="script_event_author_prototype_tests.rs"]
mod event_author_prototype_tests;

#[cfg(test)]
#[path="script_node_base_uri_tests.rs"]
mod node_base_uri_tests;

#[path="script_node_base_uri.rs"]
mod node_base_uri;

#[cfg(test)]
#[path="script_custom_host_content_tests.rs"]
mod custom_host_content_tests;

#[cfg(test)]
#[path="script_native_document_records_tests.rs"]
mod native_document_records_tests;

use script_inert_documents::element_names;

#[cfg(test)]
#[path="script_native_expanded_names_tests.rs"]
mod native_expanded_names_tests;

use script_inert_documents::xml_documents;

#[cfg(test)]
#[path="script_native_xml_document_tests.rs"]
mod native_xml_document_tests;
#[cfg(test)]
#[path="script_native_cdata_tests.rs"]
mod native_cdata_tests;
#[cfg(test)]
#[path="script_xml_doctype_adoption_tests.rs"]
mod xml_doctype_adoption_tests;
#[cfg(test)]
#[path="script_document_children_tests.rs"]
mod document_children_tests;
#[cfg(test)]
#[path="script_document_clone_tests.rs"]
mod document_clone_tests;
#[cfg(test)]
#[path="script_node_adoption_tests.rs"]
mod node_adoption_tests;
#[cfg(test)]
#[path="script_inert_direct_insertion_tests.rs"]
mod inert_direct_insertion_tests;
#[cfg(test)]
#[path="script_xml_factory_root_identity_tests.rs"]
mod xml_factory_root_identity_tests;
#[cfg(test)]
#[path="script_domparser_node_identity_tests.rs"]
mod domparser_node_identity_tests;
#[cfg(test)]
#[path="script_inert_attribute_factory_tests.rs"]
mod inert_attribute_factory_tests;
#[cfg(test)]
#[path="script_inert_attribute_attachment_tests.rs"]
mod inert_attribute_attachment_tests;

#[cfg(test)]
#[path="script_inert_foreign_attribute_tests.rs"]
mod inert_foreign_attribute_tests;

#[cfg(test)]
#[path="script_attribute_namespace_tests.rs"]
mod attribute_namespace_tests;

#[cfg(test)]
#[path="script_attribute_name_tests.rs"]
mod attribute_name_tests;

#[cfg(test)]
#[path="script_attribute_clone_tests.rs"]
mod attribute_clone_tests;

#[cfg(test)]
#[path="script_element_name_units_tests.rs"]
mod element_name_units_tests;

#[cfg(test)]
#[path="script_attribute_filter_units_tests.rs"]
mod attribute_filter_units_tests;

#[cfg(test)]
#[path="script_inert_document_import_tests.rs"]
mod inert_document_import_tests;

#[cfg(test)]
#[path="script_template_document_owner_tests.rs"]
mod template_document_owner_tests;

#[cfg(test)]
#[path="script_inert_template_import_tests.rs"]
mod inert_template_import_tests;

#[cfg(test)]
#[path="script_generic_document_tests.rs"]
mod generic_document_tests;

#[cfg(test)]
#[path="script_document_template_clone_tests.rs"]
mod document_template_clone_tests;

#[cfg(test)]
#[path="script_template_document_children_tests.rs"]
mod template_document_children_tests;

#[cfg(test)]
#[path="script_node_namespace_lookup_tests.rs"]
mod node_namespace_lookup_tests;

#[cfg(test)]
#[path="script_pi_domstring_validation_tests.rs"]
mod pi_domstring_validation_tests;

#[cfg(test)]
#[path="script_character_substring_tests.rs"]
mod character_substring_tests;

#[cfg(test)]
#[path="script_character_splice_tests.rs"]
mod character_splice_tests;

#[cfg(test)]
#[path="script_text_whole_tests.rs"]
mod text_whole_tests;

#[cfg(test)]
#[path="script_text_split_tests.rs"]
mod text_split_tests;

#[cfg(test)]
#[path="script_shared_normalize_tests.rs"]
mod shared_normalize_tests;

#[cfg(test)]
#[path="script_node_equality_tests.rs"]
mod node_equality_tests;

#[cfg(test)]
#[path="script_inert_position_tests.rs"]
mod inert_position_tests;

#[cfg(test)]
#[path="script_inert_attribute_units_tests.rs"]
mod inert_attribute_units_tests;

#[cfg(test)]
#[path="script_identifier_lookup_tests.rs"]
mod identifier_lookup_tests;

#[cfg(test)]
#[path = "script_token_list_exact_reads_tests.rs"]
mod token_list_exact_reads_tests;

#[path = "script_token_list_reads.rs"]
mod token_list_reads;

#[cfg(test)]
#[path = "script_token_list_interface_tests.rs"]
mod token_list_interface_tests;

#[cfg(test)]
#[path="script_html_class_domstring_tests.rs"]
mod html_class_domstring_tests;

#[cfg(test)]
#[path="script_token_list_exact_mutation_tests.rs"]
mod token_list_exact_mutation_tests;

#[path="script_token_list_mutation.rs"]
mod token_list_mutation;

#[cfg(test)]
#[path="script_token_list_toggle_tests.rs"]
mod token_list_toggle_tests;

#[cfg(test)]
#[path="script_token_list_replace_tests.rs"]
mod token_list_replace_tests;

#[cfg(test)]
#[path="script_token_list_for_each_tests.rs"]
mod token_list_for_each_tests;

#[cfg(test)]
#[path="script_node_list_for_each_tests.rs"]
mod node_list_for_each_tests;

#[cfg(test)]
#[path="script_node_list_item_tests.rs"]
mod node_list_item_tests;

#[cfg(test)]
#[path="script_html_title_domstring_tests.rs"]
mod html_title_domstring_tests;

#[cfg(test)]
#[path="script_html_language_domstring_tests.rs"]
mod html_language_domstring_tests;

#[cfg(test)]
#[path="script_css_id_exact_tests.rs"]
mod css_id_exact_tests;

#[cfg(test)]
#[path="script_collection_named_exact_tests.rs"]
mod collection_named_exact_tests;

#[cfg(test)]
#[path="script_form_named_exact_tests.rs"]
mod form_named_exact_tests;

#[cfg(test)]
#[path="script_element_id_accessor_tests.rs"]
mod element_id_accessor_tests;

#[path="script_element_id.rs"]
mod script_element_id;

#[cfg(test)]
#[path="script_form_owner_id_exact_tests.rs"]
mod form_owner_id_exact_tests;
#[cfg(test)]
#[path="script_html_id_domstring_tests.rs"]
mod html_id_domstring_tests;
#[cfg(test)]
#[path="script_html_id_materialization_tests.rs"]
mod html_id_materialization_tests;
#[cfg(test)]
#[path="script_live_name_domstring_tests.rs"]
mod live_name_domstring_tests;

#[path="script_form_id_guards.rs"]
mod form_id_guards;

#[cfg(test)]
#[path="script_document_name_method_tests.rs"]
mod document_name_method_tests;

#[cfg(test)]
#[path="script_owned_name_list_tests.rs"]
mod owned_name_list_tests;

#[cfg(test)]
#[path="script_passive_name_domstring_tests.rs"]
mod passive_name_domstring_tests;

#[cfg(test)]
#[path="script_passive_name_materialization_tests.rs"]
mod passive_name_materialization_tests;

#[cfg(test)]
#[path="script_html_metadata_materialization_tests.rs"]
mod html_metadata_materialization_tests;

#[cfg(test)]
#[path="script_element_class_name_accessor_tests.rs"]
mod element_class_name_accessor_tests;

#[path="script_element_class_name.rs"]
mod script_element_class_name;
#[path="script_element_class_list.rs"]
mod script_element_class_list;
#[path="script_output_html_for.rs"]
mod script_output_html_for;
#[path="script_output_form.rs"]
mod script_output_form;
#[path="script_output_metadata.rs"]
mod script_output_metadata;
#[cfg(test)]
#[path="script_element_class_list_accessor_tests.rs"]
mod element_class_list_accessor_tests;
#[cfg(test)]
#[path="script_inert_html_class_list_tests.rs"]
mod inert_html_class_list_tests;
#[cfg(test)]
#[path="script_inert_element_properties_tests.rs"]
mod inert_element_properties_tests;

#[cfg(test)]
#[path="script_inert_foreign_class_list_tests.rs"]
mod inert_foreign_class_list_tests;

#[cfg(test)]
#[path="script_inert_reflection_namespace_tests.rs"]
mod inert_reflection_namespace_tests;

#[cfg(test)]
#[path="script_aria_domstring_reflection_tests.rs"]
mod aria_domstring_reflection_tests;

#[cfg(test)]
#[path="script_html_aria_domstring_tests.rs"]
mod html_aria_domstring_tests;

#[cfg(test)]
#[path="script_html_label_for_domstring_tests.rs"]
mod html_label_for_domstring_tests;

#[cfg(test)]
#[path="script_html_label_for_accessor_tests.rs"]
mod html_label_for_accessor_tests;

#[path="script_label_html_for.rs"]
mod script_label_html_for;

#[cfg(test)]
#[path="script_html_label_control_tests.rs"]
mod script_html_label_control_tests;


use script_inert_documents::label_control as script_label_control;

#[cfg(test)]
#[path="script_html_label_form_tests.rs"]
mod html_label_form_tests;

use script_inert_documents::label_control::label_form as script_label_form;

#[cfg(test)]
#[path="script_html_input_labels_tests.rs"]
mod html_input_labels_tests;

use script_inert_documents::label_control::input_labels as script_input_labels;

#[cfg(test)]
#[path="script_button_textarea_labels_tests.rs"]
mod button_textarea_labels_tests;

#[cfg(test)]
#[path="script_remaining_builtin_labels_tests.rs"]
mod remaining_builtin_labels_tests;

#[cfg(test)]
#[path="script_output_for_domstring_tests.rs"]
mod output_for_domstring_tests;

#[cfg(test)]
#[path="script_output_html_for_tests.rs"]
mod output_html_for_tests;

#[cfg(test)]
#[path="script_output_form_tests.rs"]
mod output_form_tests;

#[cfg(test)]
#[path="script_output_name_type_tests.rs"]
mod output_name_type_tests;

#[cfg(test)]
#[path="script_output_text_replacement_tests.rs"]
mod output_text_replacement_tests;

#[path="script_text_replacement.rs"]
mod script_text_replacement;

#[cfg(test)]
#[path="script_descendant_text_tests.rs"]
mod descendant_text_tests;

#[path="script_text_content.rs"]
mod script_text_content;

#[cfg(test)]
#[path="script_output_value_default_tests.rs"]
mod output_value_default_tests;

#[path="script_output_value.rs"]
mod script_output_value;

#[path="script_output_reset.rs"]
mod script_output_reset;
pub(crate) use script_output_reset::reset_form;

#[cfg(test)]
#[path="script_output_reset_tests.rs"]
mod output_reset_tests;

#[cfg(test)]
#[path="script_output_form_attribute_tests.rs"]
mod output_form_attribute_tests;

#[cfg(test)]
#[path="script_output_validation_tests.rs"]
mod output_validation_tests;

#[path="script_output_validation.rs"]
mod script_output_validation;

#[cfg(test)]
#[path="script_output_validity_state_tests.rs"]
mod output_validity_state_tests;

#[path="script_output_validity_state.rs"]
mod script_output_validity_state;

#[cfg(test)]
#[path="script_control_form_accessor_tests.rs"]
mod control_form_accessor_tests;

#[path="script_control_form.rs"]
mod script_control_form;

#[cfg(test)]
#[path="script_control_type_tests.rs"]
mod control_type_tests;

#[path="script_control_type.rs"]
mod script_control_type;

#[cfg(test)]
#[path="script_form_length_tests.rs"]
mod form_length_tests;

use script_inert_documents::label_control::form_length as script_form_length;

#[cfg(test)]
#[path="script_fieldset_metadata_tests.rs"]
mod fieldset_metadata_tests;

#[path="script_fieldset.rs"]
mod script_fieldset;

#[cfg(test)]
#[path="script_fieldset_name_tests.rs"]
mod fieldset_name_tests;

#[cfg(test)]
#[path="script_fieldset_disabled_tests.rs"]
mod fieldset_disabled_tests;

#[cfg(test)]
#[path="script_fieldset_validation_tests.rs"]
mod fieldset_validation_tests;

#[path="script_fieldset_validation.rs"]
mod script_fieldset_validation;

#[cfg(test)]
#[path="script_fieldset_validity_tests.rs"]
mod fieldset_validity_tests;

#[cfg(test)]
#[path="script_fieldset_form_attribute_tests.rs"]
mod fieldset_form_attribute_tests;

#[cfg(test)]
#[path="script_legend_form_tests.rs"]
mod legend_form_tests;

#[path="script_legend_form.rs"]
mod script_legend_form;

#[cfg(test)]
#[path="script_progress_numeric_tests.rs"]
mod progress_numeric_tests;

#[path="script_progress_numeric.rs"]
mod script_progress_numeric;

#[cfg(test)]
#[path="script_meter_numeric_tests.rs"]
mod meter_numeric_tests;

#[path="script_meter_numeric.rs"]
mod script_meter_numeric;

#[cfg(test)]
#[path="script_gauge_lifecycle_tests.rs"]
mod gauge_lifecycle_tests;

#[cfg(test)]
#[path="script_time_datetime_tests.rs"]
mod time_datetime_tests;

#[path="script_time_datetime.rs"]
mod script_time_datetime;

#[cfg(test)]
#[path="script_data_value_tests.rs"]
mod data_value_tests;

#[path="script_data_value.rs"]
mod script_data_value;

#[cfg(test)]
#[path="script_quote_interface_tests.rs"]
mod quote_interface_tests;

#[cfg(test)]
#[path="script_heading_interface_tests.rs"]
mod heading_interface_tests;

#[cfg(test)]
#[path="script_heading_align_tests.rs"]
mod heading_align_tests;

#[path="script_heading_align.rs"]
mod script_heading_align;

#[cfg(test)]
#[path="script_div_align_tests.rs"]
mod div_align_tests;

#[path="script_div_align.rs"]
mod script_div_align;

#[cfg(test)]
#[path="script_legend_align_tests.rs"]
mod legend_align_tests;

#[path="script_legend_align.rs"]
mod script_legend_align;

#[cfg(test)]
#[path="script_legend_inert_import_tests.rs"]
mod legend_inert_import_tests;

#[cfg(test)]
#[path="script_legend_native_copy_tests.rs"]
mod legend_native_copy_tests;

#[cfg(test)]
#[path="script_paragraph_align_tests.rs"]
mod paragraph_align_tests;

#[path="script_paragraph_align.rs"]
mod script_paragraph_align;

#[cfg(test)]
#[path="script_ul_type_tests.rs"]
mod ul_type_tests;

#[path="script_ul_type.rs"]
mod script_ul_type;

#[cfg(test)]
#[path="script_ul_compact_tests.rs"]
mod ul_compact_tests;

#[path="script_ul_compact.rs"]
mod script_ul_compact;

#[cfg(test)]
#[path="script_span_interface_tests.rs"]
mod span_interface_tests;

#[cfg(test)]
#[path="script_dl_compact_tests.rs"]
mod dl_compact_tests;

#[path="script_dl_compact.rs"]
mod script_dl_compact;

#[cfg(test)]
#[path="script_li_type_tests.rs"]
mod li_type_tests;

#[path="script_li_type.rs"]
mod script_li_type;

#[cfg(test)]
#[path="script_li_value_tests.rs"]
mod li_value_tests;

#[path="script_li_value.rs"]
mod script_li_value;

#[cfg(test)]
#[path="script_ol_type_tests.rs"]
mod ol_type_tests;

#[path="script_ol_type.rs"]
mod script_ol_type;

#[cfg(test)]
#[path="script_ol_reversed_tests.rs"]
mod ol_reversed_tests;

#[path="script_ol_reversed.rs"]
mod script_ol_reversed;

#[cfg(test)]
#[path="script_ol_start_tests.rs"]
mod ol_start_tests;

#[path="script_ol_start.rs"]
mod script_ol_start;

#[cfg(test)]
#[path="script_ol_compact_tests.rs"]
mod ol_compact_tests;

#[path="script_ol_compact.rs"]
mod script_ol_compact;

#[cfg(test)]
#[path="script_pre_width_tests.rs"]
mod pre_width_tests;

#[path="script_pre_width.rs"]
mod script_pre_width;

#[cfg(test)]
#[path="script_hr_align_tests.rs"]
mod hr_align_tests;

#[path="script_hr_align.rs"]
mod script_hr_align;

#[cfg(test)]
#[path="script_hr_color_tests.rs"]
mod hr_color_tests;

#[path="script_hr_color.rs"]
mod script_hr_color;

#[cfg(test)]
#[path="script_hr_no_shade_tests.rs"]
mod hr_no_shade_tests;

#[path="script_hr_no_shade.rs"]
mod script_hr_no_shade;

#[cfg(test)]
#[path="script_hr_size_tests.rs"]
mod hr_size_tests;

#[path="script_hr_size.rs"]
mod script_hr_size;

#[cfg(test)]
#[path="script_hr_width_tests.rs"]
mod hr_width_tests;

#[path="script_hr_width.rs"]
mod script_hr_width;

#[cfg(test)]
#[path="script_br_clear_tests.rs"]
mod br_clear_tests;

#[path="script_br_clear.rs"]
mod script_br_clear;

#[cfg(test)]
#[path="script_menu_compact_tests.rs"]
mod menu_compact_tests;

#[path="script_menu_compact.rs"]
mod script_menu_compact;

#[cfg(test)]
#[path="script_dir_compact_tests.rs"]
mod dir_compact_tests;

#[path="script_dir_compact.rs"]
mod script_dir_compact;

#[cfg(test)]
#[path="script_font_color_tests.rs"]
mod font_color_tests;

#[path="script_font_color.rs"]
mod script_font_color;

#[cfg(test)]
#[path="script_font_face_tests.rs"]
mod font_face_tests;

#[path="script_font_face.rs"]
mod script_font_face;

#[cfg(test)]
#[path="script_font_size_tests.rs"]
mod font_size_tests;

#[path="script_font_size.rs"]
mod script_font_size;

#[cfg(test)]
#[path="script_pre_alias_tests.rs"]
mod pre_alias_tests;

#[cfg(test)]
#[path="script_unknown_interface_tests.rs"]
mod unknown_interface_tests;

#[cfg(test)]
#[path="script_historical_html_tests.rs"]
mod historical_html_tests;

#[cfg(test)]
#[path="script_semantic_html_tests.rs"]
mod semantic_html_tests;

#[cfg(test)]
#[path="script_unknown_fallback_tests.rs"]
mod unknown_fallback_tests;

#[cfg(test)]
#[path="script_mod_datetime_tests.rs"]
mod mod_datetime_tests;

#[path="script_mod_datetime.rs"]
mod script_mod_datetime;

#[cfg(test)]
#[path="script_map_name_tests.rs"]
mod map_name_tests;

#[path="script_map_name.rs"]
mod script_map_name;

#[cfg(test)]
#[path="script_map_areas_tests.rs"]
mod map_areas_tests;

#[path="script_map_areas.rs"]
mod script_map_areas;

#[cfg(test)]
#[path="script_area_alt_tests.rs"]
mod area_alt_tests;

#[path="script_area_alt.rs"]
mod script_area_alt;

#[cfg(test)]
#[path="script_area_coords_tests.rs"]
mod area_coords_tests;

#[path="script_area_coords.rs"]
mod script_area_coords;

#[cfg(test)]
#[path="script_area_shape_tests.rs"]
mod area_shape_tests;

#[path="script_area_shape.rs"]
mod script_area_shape;

#[cfg(test)]
#[path="script_area_download_tests.rs"]
mod area_download_tests;

#[path="script_area_download.rs"]
mod script_area_download;

#[cfg(test)]
#[path="script_area_rel_tests.rs"]
mod area_rel_tests;

#[path="script_area_rel.rs"]
mod script_area_rel;

#[cfg(test)]
#[path="script_area_ping_tests.rs"]
mod area_ping_tests;

#[path="script_area_ping.rs"]
mod script_area_ping;

#[cfg(test)]
#[path="script_area_rel_list_tests.rs"]
mod area_rel_list_tests;

#[path="script_area_rel_list.rs"]
mod script_area_rel_list;

#[cfg(test)]
#[path="script_area_target_tests.rs"]
mod area_target_tests;

#[path="script_area_target.rs"]
mod script_area_target;

#[cfg(test)]
#[path="script_area_hreflang_tests.rs"]
mod area_hreflang_tests;

#[path="script_area_hreflang.rs"]
mod script_area_hreflang;

#[cfg(test)]
#[path="script_area_type_tests.rs"]
mod area_type_tests;

#[path="script_area_type.rs"]
mod script_area_type;

#[cfg(test)]
#[path="script_area_referrer_policy_tests.rs"]
mod area_referrer_policy_tests;

#[path="script_area_referrer_policy.rs"]
mod script_area_referrer_policy;

#[cfg(test)]
#[path="script_area_href_tests.rs"]
mod area_href_tests;

#[path="script_area_href.rs"]
mod script_area_href;

#[cfg(test)]
#[path="script_area_url_tests.rs"]
mod area_url_tests;

#[path="script_area_url.rs"]
mod script_area_url;

#[cfg(test)]
#[path="script_hyperlink_stringifier_tests.rs"]
mod hyperlink_stringifier_tests;

#[path="script_hyperlink_stringifier.rs"]
mod script_hyperlink_stringifier;

#[cfg(test)]
#[path="script_area_no_href_tests.rs"]
mod area_no_href_tests;

#[path="script_area_no_href.rs"]
mod script_area_no_href;

#[cfg(test)]
#[path="script_area_detached_copy_tests.rs"]
mod area_detached_copy_tests;

#[cfg(test)]
#[path="script_area_connection_tests.rs"]
mod area_connection_tests;

#[cfg(test)]
#[path="script_param_name_tests.rs"]
mod param_name_tests;

#[path="script_param_name.rs"]
mod script_param_name;

#[cfg(test)]
#[path="script_param_metadata_tests.rs"]
mod param_metadata_tests;

#[path="script_param_metadata.rs"]
mod script_param_metadata;

#[cfg(test)]
#[path="script_param_detached_copy_tests.rs"]
mod param_detached_copy_tests;

#[cfg(test)]
#[path="script_param_connection_tests.rs"]
mod param_connection_tests;

#[cfg(test)]
#[path="script_datalist_interface_tests.rs"]
mod datalist_interface_tests;

#[cfg(test)]
#[path="script_datalist_options_tests.rs"]
mod datalist_options_tests;

#[path="script_datalist_options.rs"]
mod script_datalist_options;

#[cfg(test)]
#[path="script_datalist_detached_copy_tests.rs"]
mod datalist_detached_copy_tests;

#[cfg(test)]
#[path="script_datalist_connection_tests.rs"]
mod datalist_connection_tests;

#[cfg(test)]
#[path="script_option_interface_tests.rs"]
mod option_interface_tests;

#[cfg(test)]
#[path="script_option_disabled_tests.rs"]
mod option_disabled_tests;

#[path="script_option_disabled.rs"]
mod script_option_disabled;

#[cfg(test)]
#[path="script_option_form_tests.rs"]
mod option_form_tests;

#[path="script_option_form.rs"]
mod script_option_form;

#[cfg(test)]
#[path="script_option_index_tests.rs"]
mod option_index_tests;

#[path="script_option_index.rs"]
mod script_option_index;

#[cfg(test)]
#[path="script_optgroup_interface_tests.rs"]
mod optgroup_interface_tests;

#[cfg(test)]
#[path="script_optgroup_disabled_tests.rs"]
mod optgroup_disabled_tests;

#[path="script_optgroup_disabled.rs"]
mod script_optgroup_disabled;

#[cfg(test)]
#[path="script_optgroup_label_tests.rs"]
mod optgroup_label_tests;

#[path="script_optgroup_label.rs"]
mod script_optgroup_label;

#[cfg(test)]
#[path="script_optgroup_detached_copy_tests.rs"]
mod optgroup_detached_copy_tests;

#[cfg(test)]
#[path="script_optgroup_connection_tests.rs"]
mod optgroup_connection_tests;

#[cfg(test)]
#[path="script_optgroup_fragment_tests.rs"]
mod optgroup_fragment_tests;

#[cfg(test)]
#[path="script_optgroup_adoption_tests.rs"]
mod optgroup_adoption_tests;

#[cfg(test)]
#[path="script_optgroup_content_tests.rs"]
mod optgroup_content_tests;

#[cfg(test)]
#[path="script_option_label_tests.rs"]
mod option_label_tests;

#[path="script_option_label.rs"]
mod script_option_label;
#[path="script_html_aware_text.rs"]
mod script_html_aware_text;

#[cfg(test)]
#[path="script_option_value_tests.rs"]
mod option_value_tests;

#[path="script_option_value.rs"]
mod script_option_value;

#[cfg(test)]
#[path="script_option_text_tests.rs"]
mod option_text_tests;

#[path="script_option_text.rs"]
mod script_option_text;

#[cfg(test)]
#[path="script_option_default_selected_tests.rs"]
mod option_default_selected_tests;

#[path="script_option_default_selected.rs"]
mod script_option_default_selected;

#[cfg(test)]
#[path="script_select_disabled_tests.rs"]
mod select_disabled_tests;

#[path="script_select_disabled.rs"]
mod script_select_disabled;

#[cfg(test)]
#[path="script_select_required_tests.rs"]
mod select_required_tests;

#[path="script_select_required.rs"]
mod script_select_required;

#[cfg(test)]
#[path="script_select_name_tests.rs"]
mod select_name_tests;

#[path="script_select_name.rs"]
mod script_select_name;

#[cfg(test)]
#[path="script_textarea_name_tests.rs"]
mod textarea_name_tests;

#[path="script_textarea_name.rs"]
mod script_textarea_name;

#[cfg(test)]
#[path="script_textarea_disabled_tests.rs"]
mod textarea_disabled_tests;

#[path="script_textarea_disabled.rs"]
mod script_textarea_disabled;

#[cfg(test)]
#[path="script_textarea_required_tests.rs"]
mod textarea_required_tests;

#[path="script_textarea_required.rs"]
mod script_textarea_required;

#[cfg(test)]
#[path="script_textarea_read_only_tests.rs"]
mod textarea_read_only_tests;

#[path="script_textarea_read_only.rs"]
mod script_textarea_read_only;

#[cfg(test)]
#[path="script_textarea_dimensions_tests.rs"]
mod textarea_dimensions_tests;

#[path="script_textarea_dimensions.rs"]
mod script_textarea_dimensions;

#[cfg(test)]
#[path="script_textarea_text_length_tests.rs"]
mod textarea_text_length_tests;

#[path="script_textarea_text_length.rs"]
mod script_textarea_text_length;

#[cfg(test)]
#[path="script_textarea_default_value_tests.rs"]
mod textarea_default_value_tests;

#[path="script_textarea_default_value.rs"]
mod script_textarea_default_value;

#[cfg(test)]
#[path="script_textarea_max_length_tests.rs"]
mod textarea_max_length_tests;

#[path="script_textarea_max_length.rs"]
mod script_textarea_max_length;

#[cfg(test)]
#[path="script_textarea_min_length_tests.rs"]
mod textarea_min_length_tests;

#[path="script_textarea_min_length.rs"]
mod script_textarea_min_length;

#[cfg(test)]
#[path="script_textarea_wrap_tests.rs"]
mod textarea_wrap_tests;

#[path="script_textarea_wrap.rs"]
mod script_textarea_wrap;

#[cfg(test)]
#[path="script_textarea_autocomplete_tests.rs"]
mod textarea_autocomplete_tests;

#[path="script_textarea_autocomplete.rs"]
mod script_textarea_autocomplete;

#[cfg(test)]
#[path="script_select_autocomplete_tests.rs"]
mod select_autocomplete_tests;

#[path="script_autofill.rs"]
mod script_autofill;
#[path="script_select_autocomplete.rs"]
mod script_select_autocomplete;

#[cfg(test)]
#[path="script_input_autocomplete_tests.rs"]
mod input_autocomplete_tests;

#[path="script_input_autocomplete.rs"]
mod script_input_autocomplete;

#[cfg(test)]
#[path="script_form_autocomplete_tests.rs"]
mod form_autocomplete_tests;

#[path="script_form_autocomplete.rs"]
mod script_form_autocomplete;

#[cfg(test)]
#[path="script_form_name_tests.rs"]
mod form_name_tests;

#[path="script_form_name.rs"]
mod script_form_name;

#[cfg(test)]
#[path="script_form_no_validate_tests.rs"]
mod form_no_validate_tests;

#[path="script_form_no_validate.rs"]
mod script_form_no_validate;

#[cfg(test)]
#[path="script_form_accept_charset_tests.rs"]
mod form_accept_charset_tests;

#[path="script_form_accept_charset.rs"]
mod script_form_accept_charset;

#[cfg(test)]
#[path="script_form_target_tests.rs"]
mod form_target_tests;

#[path="script_form_target.rs"]
mod script_form_target;

#[cfg(test)]
#[path="script_form_rel_tests.rs"]
mod form_rel_tests;

#[path="script_form_rel.rs"]
mod script_form_rel;

#[cfg(test)]
#[path="script_form_rel_list_tests.rs"]
mod form_rel_list_tests;

#[path="script_form_rel_list.rs"]
mod script_form_rel_list;

#[cfg(test)]
#[path="script_form_method_tests.rs"]
mod form_method_tests;

#[path="script_form_method.rs"]
mod script_form_method;

#[cfg(test)]
#[path="script_form_enctype_tests.rs"]
mod form_enctype_tests;

#[path="script_form_enctype.rs"]
mod script_form_enctype;

#[cfg(test)]
#[path="script_form_action_tests.rs"]
mod form_action_tests;

#[path="script_form_action.rs"]
mod script_form_action;

#[cfg(test)]
#[path="script_input_form_action_tests.rs"]
mod input_form_action_tests;

#[path="script_input_form_action.rs"]
mod script_input_form_action;

#[cfg(test)]
#[path="script_button_form_action_tests.rs"]
mod button_form_action_tests;

#[path="script_button_form_action.rs"]
mod script_button_form_action;

#[cfg(test)]
#[path="script_submitter_form_method_tests.rs"]
mod submitter_form_method_tests;

#[path="script_input_form_method.rs"]
mod script_input_form_method;

#[path="script_button_form_method.rs"]
mod script_button_form_method;

#[cfg(test)]
#[path="script_submitter_form_enctype_tests.rs"]
mod submitter_form_enctype_tests;

#[path="script_input_form_enctype.rs"]
mod script_input_form_enctype;

#[path="script_button_form_enctype.rs"]
mod script_button_form_enctype;

#[cfg(test)]
#[path="script_submitter_form_no_validate_tests.rs"]
mod submitter_form_no_validate_tests;

#[path="script_input_form_no_validate.rs"]
mod script_input_form_no_validate;

#[path="script_button_form_no_validate.rs"]
mod script_button_form_no_validate;

#[cfg(test)]
#[path="script_submitter_form_target_input_tests.rs"]
mod submitter_form_target_input_tests;

#[cfg(test)]
#[path="script_submitter_form_target_button_tests.rs"]
mod submitter_form_target_button_tests;

#[path="script_input_form_target.rs"]
mod script_input_form_target;

#[path="script_button_form_target.rs"]
mod script_button_form_target;

#[cfg(test)]
#[path="script_control_names_input_tests.rs"]
mod control_names_input_tests;

#[cfg(test)]
#[path="script_control_names_button_tests.rs"]
mod control_names_button_tests;

#[path="script_input_name.rs"]
mod script_input_name;

#[path="script_button_name.rs"]
mod script_button_name;

#[cfg(test)]
#[path="script_control_names_raw_form_tests.rs"]
mod control_names_raw_form_tests;

#[cfg(test)]
#[path="script_control_disabled_tests.rs"]
mod control_disabled_tests;

#[path="script_input_disabled.rs"]
mod script_input_disabled;

#[path="script_button_disabled.rs"]
mod script_button_disabled;

#[cfg(test)]
#[path="script_input_constraint_flags_tests.rs"]
mod input_constraint_flags_tests;

#[path="script_input_required.rs"]
mod script_input_required;

#[path="script_input_read_only.rs"]
mod script_input_read_only;

#[cfg(test)]
#[path="script_input_max_length_tests.rs"]
mod input_max_length_tests;

#[cfg(test)]
#[path="script_input_min_length_tests.rs"]
mod input_min_length_tests;

#[path="script_input_max_length.rs"]
mod script_input_max_length;

#[path="script_input_min_length.rs"]
mod script_input_min_length;

#[cfg(test)]
#[path="script_input_constraint_metadata_tests.rs"]
mod input_constraint_metadata_tests;

#[path="script_input_pattern.rs"]
mod script_input_pattern;

#[path="script_input_min.rs"]
mod script_input_min;

#[path="script_input_max.rs"]
mod script_input_max;

#[path="script_input_step.rs"]
mod script_input_step;

#[cfg(test)]
#[path="script_button_type_value_tests.rs"]
mod button_type_value_tests;

#[path="script_button_type.rs"]
mod script_button_type;

#[path="script_button_value.rs"]
mod script_button_value;

#[cfg(test)]
#[path="script_form_reset_tests.rs"]
mod form_reset_tests;

#[cfg(test)]
#[path="script_form_reset_output_tests.rs"]
mod form_reset_output_tests;

#[cfg(test)]
#[path="script_submit_event_tests.rs"]
mod submit_event_tests;

#[cfg(test)]
#[path="script_invalid_event_tests.rs"]
mod invalid_event_tests;

#[cfg(test)]
#[path="script_form_validation_tests.rs"]
mod form_validation_tests;

#[cfg(test)]
#[path="script_form_request_tests.rs"]
mod form_request_tests;

#[cfg(test)]
#[path="script_control_eligibility_tests.rs"]
mod control_eligibility_tests;

#[cfg(test)]
#[path="script_inert_control_eligibility_tests.rs"]
mod inert_control_eligibility_tests;

#[cfg(test)]
#[path="script_control_validation_tests.rs"]
mod control_validation_tests;

#[path="script_control_validation.rs"]
mod script_control_validation;

#[cfg(test)]
#[path="script_control_custom_validity_tests.rs"]
mod control_custom_validity_tests;

#[path="script_control_custom_validity.rs"]
mod script_control_custom_validity;

#[cfg(test)]
#[path="script_candidate_validity_tests.rs"]
mod candidate_validity_tests;

#[path="script_candidate_validity.rs"]
mod script_candidate_validity;

#[cfg(test)]
#[path="script_text_length_validation_tests.rs"]
mod text_length_validation_tests;

#[cfg(test)]
#[path="script_pattern_validation_tests.rs"]
mod pattern_validation_tests;

#[cfg(test)]
#[path="script_password_input_tests.rs"]
mod password_input_tests;

#[cfg(test)]
#[path="script_text_type_transition_tests.rs"]
mod text_type_transition_tests;

#[cfg(test)]
#[path="script_text_family_admission_tests.rs"]
mod text_family_admission_tests;

#[cfg(test)]
#[path="script_tree_walker_filter_tests.rs"]
mod tree_walker_filter_tests;

#[cfg(test)]
#[path="script_range_clone_contents_tests.rs"]
mod range_clone_contents_tests;

#[cfg(test)]
#[path="script_range_comparison_tests.rs"]
mod range_comparison_tests;

#[cfg(test)]
#[path="script_static_range_tests.rs"]
mod static_range_tests;

#[cfg(test)]
#[path="script_static_range_inert_tests.rs"]
mod static_range_inert_tests;

#[cfg(test)]
#[path="script_node_iterator_state_tests.rs"]
mod node_iterator_state_tests;

#[cfg(test)]
#[path="script_node_iterator_live_tests.rs"]
mod node_iterator_live_tests;

#[cfg(test)]
#[path="script_inert_traversal_factory_tests.rs"]
mod inert_traversal_factory_tests;
