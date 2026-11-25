//! A small, original JavaScript interpreter. No evaluation escapes this module
//! except through the explicitly supplied Host interface. This is a bounded
//! language subset, not an ECMAScript conformance claim or a process sandbox.
//!
//! ASTs, environments and objects use arenas, so hostile expression chains and
//! circular objects cannot make recursive Rust values or reference-counted
//! cycles. Object identities remain allocated until document teardown, under
//! fixed object/property/string caps. Jobs share instruction and string-copy
//! budgets across scripts/callbacks until the host calls begin_job.

use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};
use work_attribution::Kind as WorkKind;
#[path = "string_property_map.rs"]
mod string_property_map;
use string_property_map::{StringKey, StringProperties};
#[path = "property_key_utf16.rs"]
mod property_key_utf16;
#[cfg(test)]
#[path = "property_key_utf16_tests.rs"]
mod property_key_utf16_tests;
#[cfg(test)]
#[path = "property_key_proxy_tests.rs"]
mod property_key_proxy_tests;
#[cfg(test)]
#[path = "property_key_native_tests.rs"]
mod property_key_native_tests;
#[path="character_data_interface.rs"]
pub(crate) mod character_data_interface;
#[path="anchor_url_interface.rs"]
pub(crate) mod anchor_url_interface;
#[path="area_url_interface.rs"]
pub(crate) mod area_url_interface;
#[path="document_location_interface.rs"]
pub(crate) mod document_location_interface;
#[path="adjacent_interface.rs"]
pub(crate) mod adjacent_interface;
#[path="text_split_interface.rs"]
pub(crate) mod text_split_interface;
#[path="node_normalize_interface.rs"]
pub(crate) mod node_normalize_interface;

#[path = "arguments.rs"]
mod arguments;
#[path = "array_descriptors.rs"]
mod array_descriptors;
#[path = "array_ops.rs"]
mod array_ops;
#[path = "javascript_base64.rs"]
mod base64;
#[path = "binary_data.rs"]
mod binary_data;
#[path = "canvas_readback.rs"]
pub(crate) mod canvas_readback;
#[path = "binding_length.rs"]
mod binding_length;
#[path = "binding_character.rs"]
mod binding_character;
#[path = "intrinsic_realms.rs"]
mod intrinsic_realms;
#[path = "reflect.rs"]
mod reflect;
#[path = "proxy_objects.rs"]
mod proxy_objects;
#[path = "proxy_operations.rs"]
mod proxy_operations;
#[path = "proxy_dispatch.rs"]
mod proxy_dispatch;
#[path = "proxy_traps.rs"]
mod proxy_traps;
#[path = "function_allocation.rs"]
mod function_allocation;
#[path = "function_arena.rs"]
mod function_arena;
#[path = "number_radix.rs"]
mod number_radix;
#[path = "range_interface.rs"]
pub(crate) mod range_interface;
#[path = "abort_interface.rs"]
pub(crate) mod abort_interface;
#[path = "message_channel_interface.rs"]
pub(crate) mod message_channel_interface;
#[path = "bigint.rs"]
mod bigint;
#[path = "number_to_fixed.rs"]
mod number_to_fixed;
#[cfg(test)]
#[path = "number_to_fixed_tests.rs"]
mod number_to_fixed_tests;
#[path = "program_allocation.rs"]
mod program_allocation;
#[path="program_storage.rs"]
mod program_storage;
#[path = "function_reachability.rs"]
mod function_reachability;
#[cfg(test)]
#[path = "function_collection_tests.rs"]
mod function_collection_tests;
#[cfg(test)]
#[path = "function_demand_tests.rs"]
mod function_demand_tests;
#[cfg(test)]
#[path = "new_target_tests.rs"]
mod new_target_tests;
#[cfg(test)]
#[path = "script_preflight_tests.rs"]
mod script_preflight_tests;
#[path = "selected_if_statement.rs"]
mod selected_if_statement;
#[path = "transient_roots.rs"]
mod transient_roots;
#[path = "deferred_native_methods.rs"]
mod deferred_native_methods;
#[path = "function_source.rs"]
mod function_source;
#[cfg(test)]
#[path = "regexp_input_conversion_tests.rs"]
mod regexp_input_conversion_tests;
#[cfg(test)]
#[path = "statement_expression_tests.rs"]
mod statement_expression_tests;
#[path = "binding_patterns.rs"]
mod binding_patterns;
#[path = "bound_functions.rs"]
mod bound_functions;
#[path = "class_parser.rs"]
mod class_parser;
#[path = "class_runtime.rs"]
mod class_runtime;
#[path = "derived_context.rs"]
mod derived_context;
#[path = "super_access.rs"]
mod super_access;
#[cfg(test)]
#[path = "base_class_tests.rs"]
mod base_class_tests;
#[cfg(test)]
#[path = "derived_class_tests.rs"]
mod derived_class_tests;
#[cfg(test)]
#[path = "derived_class_lifetime_tests.rs"]
mod derived_class_lifetime_tests;
#[cfg(test)]
#[path = "super_home_object_tests.rs"]
mod super_home_object_tests;
#[cfg(test)]
#[path = "strict_missing_assignment_tests.rs"]
mod strict_missing_assignment_tests;
#[path = "collections.rs"]
mod collections;
#[path = "date.rs"]
mod date;
#[path = "rect_api.rs"]
mod rect_api;
#[path = "image_data_api.rs"]
mod image_data_api;
#[path = "dom_exception.rs"]
mod dom_exception;
#[path = "mutation_observers.rs"]
mod mutation_observers;
#[path = "tree_walker.rs"]
mod tree_walker;
#[path = "expression_parser.rs"]
mod expression_parser;
#[path = "identifier_names.rs"]
mod identifier_names;
#[path = "expression_evaluator.rs"]
mod expression_evaluator;
#[cfg(test)]
#[path = "expression_evaluator_tests.rs"]
mod expression_evaluator_tests;
#[cfg(test)]
#[path = "expression_parser_tests.rs"]
mod expression_parser_tests;
#[cfg(test)]
#[path = "environment_lifetime_tests.rs"]
mod environment_lifetime_tests;
#[path = "function_constructor.rs"]
mod function_constructor;
#[path = "async_function_intrinsics.rs"]
mod async_function_intrinsics;
#[path = "generator_functions.rs"]
mod generator_functions;
#[path = "host_objects.rs"]
mod host_objects;
#[path = "event_interface.rs"]
mod event_interface;
#[path="submit_event_interface.rs"]
mod submit_event_interface;
#[path="xhr_interface.rs"]
pub(crate) mod xhr_interface;
#[path = "event_listener.rs"]
mod event_listener;
#[path = "media_interface.rs"]
mod media_interface;
#[path = "runtime_diagnostics.rs"]
mod runtime_diagnostics;
#[path = "string_substr.rs"]
mod string_substr;
#[cfg(test)]
#[path = "string_slicing_utf16_tests.rs"]
mod string_slicing_utf16_tests;
#[cfg(test)]
#[path = "string_slicing_utf16_boundary_tests.rs"]
mod string_slicing_utf16_boundary_tests;
#[path = "string_concat.rs"]
mod string_concat;
#[path = "window_relationships.rs"]
pub(crate) mod window_relationships;
#[path = "string_units.rs"]
mod string_units;
#[cfg(test)]
#[path = "string_unit_tests.rs"]
mod string_unit_tests;
#[path = "string_last_index_of.rs"]
mod string_last_index_of;
#[cfg(test)]
#[path = "string_last_index_of_tests.rs"]
mod string_last_index_of_tests;
#[path = "global_operations.rs"]
mod global_operations;
#[path = "iterable_builtins.rs"]
mod iterable_builtins;
#[path = "iterator_protocol.rs"]
mod iterator_protocol;
#[path = "json.rs"]
mod json;
#[path = "named_regexp.rs"]
mod named_regexp;
#[path = "object_inspection.rs"]
mod object_inspection;
#[path = "object_literals.rs"]
mod object_literals;
#[path = "operators.rs"]
mod operators;
#[path = "optional_chaining.rs"]
mod optional_chaining;
#[path = "parameters.rs"]
mod parameters;
#[path = "primitive_conversion.rs"]
mod primitive_conversion;
#[path = "primitive_wrappers.rs"]
mod primitive_wrappers;
#[path = "promises.rs"]
mod promises;
#[cfg(test)]
#[path = "async_function_execution_tests.rs"]
mod async_function_execution_tests;
#[cfg(test)]
#[path = "generator_function_tests.rs"]
mod generator_function_tests;
#[path = "async_plan.rs"]
mod async_plan;
#[path = "async_execution.rs"]
mod async_execution;
#[cfg(test)]
#[path = "await_execution_tests.rs"]
mod await_execution_tests;
pub(crate) use promises::{NativePromise, NativePromiseOutcome};
#[path = "host_promise.rs"]
mod host_promise;
#[path = "prototypes.rs"]
mod prototypes;
#[cfg(test)]
#[path = "legacy_object_prototype_tests.rs"]
mod legacy_object_prototype_tests;
#[path = "regexp.rs"]
mod regexp;
#[path = "regexp_utf16_values.rs"]
mod regexp_utf16_values;
#[cfg(test)]
#[path = "rest_parameters.rs"]
mod rest_parameters;
#[path = "spread.rs"]
mod spread;
#[path = "string_objects.rs"]
mod string_objects;
#[path = "string_replacement.rs"]
mod string_replacement;
#[path = "symbols.rs"]
mod symbols;
#[path = "template_literals.rs"]
mod template_literals;
#[path = "text_encoding.rs"]
mod text_encoding;
#[path = "url_api.rs"]
mod url_api;
#[path = "headers_api.rs"]
mod headers_api;
#[path = "response_api.rs"]
mod response_api;
pub(crate) use url_api::{project as anchor_url_project, mutate as anchor_url_mutate};
#[path = "work_attribution.rs"]
mod work_attribution;
#[path = "execution_limits.rs"]
pub mod limits;
#[cfg(test)]
#[path = "execution_limits_tests.rs"]
mod execution_limits_tests;
#[path = "window_keys.rs"]
mod window_keys;
#[cfg(test)]
#[path = "browser_capacity_tests.rs"]
mod browser_capacity_tests;
#[cfg(test)]
#[path = "heap_limits_tests.rs"]
mod heap_limits_tests;

// Source/parse storage is separate from execution, strings and host work.
// Historical aliases below are only for explicit regression-profile fixtures.
// Normal source/parse/storage paths use the selected capacity profile.
#[cfg(test)]
pub const MAX_SOURCE_BYTES: usize = 512 * 1024;
#[cfg(test)]
pub const MAX_STRING_BYTES: usize = 128 * 1024;
#[cfg(test)]
const MAX_REGEXP_WORK: usize = limits::REGRESSION.regexp_work;
#[cfg(test)]
const MAX_TOKENS: usize = 131_072;
#[cfg(test)]
const MAX_AST: usize = 131_072;
const MAX_PARSE_DEPTH: usize = 64;
// Explicit count boundary for parser regression tests. Production charges
// allocated continuation storage and retains the separate native depth guard.
#[cfg(test)]
const MAX_EXPRESSION_FRAMES: usize = 4096;
// Flat else-if dispatchers do not consume native parser/evaluator frames.
// Pending branches across nested statements still share a finite allowance.
#[cfg(test)]
const MAX_IF_BRANCHES: usize = 4096;
// Native reentry (statements, callbacks and compound helpers), not the
// expression continuation stack. Never raise this to enlarge heap capacity.
const MAX_EVAL_DEPTH: usize = 128;
#[cfg(test)]
const MAX_EVALUATION_FRAMES: usize = 4096;
#[cfg(test)]
const MAX_EVALUATION_FRAME_SLOTS: usize = 2 * MAX_EVALUATION_FRAMES;
#[cfg(test)]
const MAX_EVALUATION_ARGUMENT_SLOTS: usize = 4 * MAX_EVALUATION_FRAMES;
#[cfg(any(test, not(windows)))]
const MAX_CALL_DEPTH: usize = 32;
#[path = "native_call_stack.rs"]
mod native_call_stack;
#[cfg(test)]
const MAX_ARGUMENTS: usize = 256;
#[cfg(test)]
const MAX_STEPS: usize = limits::REGRESSION.steps;
#[cfg(test)]
const MAX_COPY_BYTES: usize = limits::REGRESSION.copy_bytes;
#[cfg(test)]
const MAX_PROGRAMS: usize = 128;
#[cfg(test)]
const MAX_RETAINED_SOURCE: usize = 2 * 1024 * 1024;
#[cfg(test)]
const MAX_RETAINED_AST: usize = 262_144;
#[cfg(test)]
const MAX_FUNCTIONS: usize = 1024;
#[cfg(test)]
const MAX_ENVIRONMENTS: usize = 4096;
#[cfg(test)]
const MAX_RETAINED_VALUES: usize = limits::REGRESSION_HEAP.retained_values;
#[cfg(test)]
const MAX_OBJECTS: usize = limits::REGRESSION_HEAP.objects;
#[cfg(test)]
const MAX_PROPERTIES: usize = limits::REGRESSION_HEAP.properties;
// Source literal/pattern bounds remain separate from dynamic heap capacity.
// Historical regression-profile tests use these same small boundary witnesses.
#[cfg(test)]
const MAX_OBJECT_PROPERTIES: usize = limits::REGRESSION_HEAP.object_properties;
#[cfg(test)]
const MAX_ARRAY_LENGTH: usize = limits::REGRESSION_HEAP.array_length;
#[cfg(test)]
const MAX_PROPERTY_BYTES: usize = 256;

#[derive(Clone, Debug)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    BigInt(std::sync::Arc<str>),
    String(String),
    /// Exact UTF-16 when at least one code unit is unpaired.
    StringUnits(Vec<u16>),
    /// Unique primitive identity in this document's bounded symbol arena.
    Symbol(usize),
    Function(usize),
    /// Stable identity in this document's bounded object arena.
    Object(usize),
    Host(u32),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Bool(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            Self::BigInt(value) => value.as_ref() != "0",
            Self::String(value) => !value.is_empty(),
            Self::StringUnits(_) => true,
            Self::Symbol(_) | Self::Function(_) | Self::Object(_) | Self::Host(_) => true,
        }
    }

    /// Text for diagnostics and supported primitive conversions. Object handles
    /// are labelled; script-visible conversion requires the runtime's checks.
    pub fn to_text(&self) -> String {
        match self {
            Self::Undefined => "undefined".into(),
            Self::Null => "null".into(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => number_text(*value),
            Self::BigInt(value) => value.to_string(),
            Self::String(value) => value.clone(),
            Self::StringUnits(_) => "[string containing unpaired UTF-16]".into(),
            Self::Symbol(_) => "[symbol]".into(),
            Self::Function(_) => "[function]".into(),
            Self::Object(_) => "[script object]".into(),
            Self::Host(_) => "[host object]".into(),
        }
    }

    pub fn to_number(&self) -> f64 {
        match self {
            Self::Undefined
            | Self::BigInt(_)
            | Self::Symbol(_)
            | Self::Function(_)
            | Self::Object(_)
            | Self::Host(_) => f64::NAN,
            Self::Null => 0.0,
            Self::Bool(value) => f64::from(*value),
            Self::Number(value) => *value,
            Self::String(value) => string_number(value),
            Self::StringUnits(_) => f64::NAN,
        }
    }

    pub(crate) fn bytes(&self) -> usize {
        match self {
            Self::String(value) => value.len(),
            Self::StringUnits(value) => value.len().saturating_mul(2),
            Self::BigInt(value) => value.len(),
            _ => 0,
        }
    }

    fn diagnostic_type(&self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::BigInt(_) => "bigint",
            Self::String(_) | Self::StringUnits(_) => "string",
            Self::Symbol(_) => "symbol",
            Self::Function(_) => "function",
            Self::Object(_) => "object",
            Self::Host(_) => "native object",
        }
    }

    fn primitive(&self) -> Result<(), Fault> {
        if matches!(self, Self::Symbol(_)) {
            Err(Fault::exception(
                "TypeError",
                "Cannot convert a Symbol to a string or number.",
            ))
        } else if matches!(self, Self::Function(_) | Self::Object(_) | Self::Host(_)) {
            Err(Fault::new("Implicit object conversion is not implemented."))
        } else {
            Ok(())
        }
    }

    /// Checked implicit conversion, after the runtime has applied ToPrimitive.
    /// Diagnostic labels must never become script-visible coercion results.
    fn script_text(&self) -> Result<String, Fault> {
        self.primitive()?;
        self.scalar_text().map_err(Fault::new)
    }

    fn script_number(&self) -> Result<f64, Fault> {
        self.primitive()?;
        if matches!(self, Self::BigInt(_)) {
            return Err(Fault::exception("TypeError", "Cannot implicitly convert BigInt to Number."));
        }
        Ok(self.to_number())
    }
}

#[derive(Clone, Copy)]
pub struct WindowContext {
    pub document: u32,
    pub location: u32,
}

/// Proven ordinary tail of an opaque native interface chain, or the actual
/// exposed prototype anchored by an original native constructor.
pub struct NativeAncestry {
    pub window:u32,
    pub constructor:Option<(u32,&'static str)>,
}

/// Ordinary element interfaces installed directly on each realm's HTMLElement chain.
pub(crate) const ORDINARY_HTML_SUBCLASSES: &[(&str, &str)] = &[
    ("a", "HTMLAnchorElement"),
    ("body", "HTMLBodyElement"),
    ("blockquote", "HTMLQuoteElement"),
    ("button", "HTMLButtonElement"),
    ("canvas", "HTMLCanvasElement"),
    ("data", "HTMLDataElement"),
    ("datalist", "HTMLDataListElement"),
    ("option", "HTMLOptionElement"),
    ("optgroup", "HTMLOptGroupElement"),
    ("div", "HTMLDivElement"),
    ("dl", "HTMLDListElement"),
    ("applet", "HTMLUnknownElement"),
    ("bgsound", "HTMLUnknownElement"),
    ("blink", "HTMLUnknownElement"),
    ("isindex", "HTMLUnknownElement"),
    ("keygen", "HTMLUnknownElement"),
    ("multicol", "HTMLUnknownElement"),
    ("nextid", "HTMLUnknownElement"),
    ("spacer", "HTMLUnknownElement"),
    ("fieldset", "HTMLFieldSetElement"),
    ("form", "HTMLFormElement"),
    ("h1", "HTMLHeadingElement"),
    ("h2", "HTMLHeadingElement"),
    ("h3", "HTMLHeadingElement"),
    ("h4", "HTMLHeadingElement"),
    ("h5", "HTMLHeadingElement"),
    ("h6", "HTMLHeadingElement"),
    ("head", "HTMLHeadElement"),
    ("html", "HTMLHtmlElement"),
    ("iframe", "HTMLIFrameElement"),
    ("img", "HTMLImageElement"),
    ("input", "HTMLInputElement"),
    ("label", "HTMLLabelElement"),
    ("legend", "HTMLLegendElement"),
    ("ol", "HTMLOListElement"),
    ("li", "HTMLLIElement"),
    ("meta", "HTMLMetaElement"),
    ("meter", "HTMLMeterElement"),
    ("output", "HTMLOutputElement"),
    ("p", "HTMLParagraphElement"),
    ("pre", "HTMLPreElement"),
    ("listing", "HTMLPreElement"),
    ("xmp", "HTMLPreElement"),
    ("hr", "HTMLHRElement"),
    ("br", "HTMLBRElement"),
    ("dir", "HTMLDirectoryElement"),
    ("font", "HTMLFontElement"),
    ("menu", "HTMLMenuElement"),
    ("progress", "HTMLProgressElement"),
    ("q", "HTMLQuoteElement"),
    ("select", "HTMLSelectElement"),
    ("script", "HTMLScriptElement"),
    ("link", "HTMLLinkElement"),
    ("slot", "HTMLSlotElement"),
    ("span", "HTMLSpanElement"),
    ("template", "HTMLTemplateElement"),
    ("textarea", "HTMLTextAreaElement"),
    ("time", "HTMLTimeElement"),
    ("area", "HTMLAreaElement"),
    ("map", "HTMLMapElement"),
    ("param", "HTMLParamElement"),
    ("ins", "HTMLModElement"),
    ("del", "HTMLModElement"),
    ("title", "HTMLTitleElement"),
    ("ul", "HTMLUListElement"),
];

/// Canonical readonly labels operations for ordinary built-in labelable controls.
pub(crate) const CONTROL_LABEL_GETTERS:&[(&str,&str)]=&[
    ("HTMLInputElement","\0HTMLInputElement.get.labels"),
    ("HTMLButtonElement","\0HTMLButtonElement.get.labels"),
    ("HTMLTextAreaElement","\0HTMLTextAreaElement.get.labels"),
    ("HTMLMeterElement","\0HTMLMeterElement.get.labels"),
    ("HTMLOutputElement","\0HTMLOutputElement.get.labels"),
    ("HTMLProgressElement","\0HTMLProgressElement.get.labels"),
    ("HTMLSelectElement","\0HTMLSelectElement.get.labels"),
];

/// Exact ordinary interfaces for supported HTML nodes, including elements
/// whose immediate interface is HTMLElement itself.
pub(crate) fn ordinary_html_interface(tag: &str) -> Option<&'static str> {
    if matches!(tag, "main" | "abbr" | "address" | "article" | "aside" | "b" | "bdi" | "bdo" | "cite" | "code" | "dd" | "dfn" | "dt" | "em" | "figcaption" | "figure" | "footer" | "header" | "hgroup" | "i" | "kbd" | "mark" | "nav" | "noscript" | "rp" | "rt" | "ruby" | "s" | "samp" | "search" | "section" | "small" | "strong" | "sub" | "summary" | "sup" | "u" | "var" | "wbr" | "acronym" | "basefont" | "big" | "center" | "nobr" | "noembed" | "noframes" | "plaintext" | "rb" | "rtc" | "strike" | "tt") || crate::custom_element_name::valid(tag) {
        return Some("HTMLElement");
    }
    if let Some(&(_, name)) = ORDINARY_HTML_SUBCLASSES.iter().find(|(local, _)| *local == tag) {
        return Some(name);
    }
    // Known HTML interfaces with partial implementation keep their native routing.
    if matches!(tag, "audio" | "base" | "caption" | "col" | "colgroup" | "datalist" | "details" | "dialog" | "embed" | "object" | "optgroup" | "option" | "picture" | "selectedcontent" | "source" | "style" | "table" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr" | "track" | "video" | "frame" | "frameset" | "marquee") {
        return None;
    }
    Some("HTMLUnknownElement")
}

pub(crate) const ELEMENT_STYLE_GET: &str = "\0HTMLElement.get.style";
pub(crate) const SVG_STYLE_GET:&str="\0SVGElement.get.style";
pub(crate) const SVG_STYLE_SET:&str="\0SVGElement.set.style";
pub(crate) const MEDIA_QUERY_LIST: &str = "MediaQueryList";
pub(crate) const MEDIA_QUERY_MATCHES: &str = "\0MediaQueryList.get.matches";
pub(crate) const MEDIA_QUERY_MEDIA: &str = "\0MediaQueryList.get.media";
pub(crate) const DOCUMENT_ENCODING_GETTERS: [(&str, &str); 3] = [
    ("characterSet", "\0Document.get.characterSet"),
    ("charset", "\0Document.get.charset"),
    ("inputEncoding", "\0Document.get.inputEncoding"),
];

pub(crate) fn document_encoding_getter(method: &str) -> bool {
    DOCUMENT_ENCODING_GETTERS.iter().any(|&(_, getter)| getter == method)
}

pub(crate) const DOCUMENT_CURRENT_SCRIPT_GET: &str = "\0Document.get.currentScript";
pub(crate) const DOCUMENT_ACTIVE_ELEMENT_GET:&str="\0Document.get.activeElement";
pub(crate) const TITLE_TEXT_GET: &str = "\0HTMLTitleElement.get.text";
pub(crate) const TITLE_TEXT_SET: &str = "\0HTMLTitleElement.set.text";
pub(crate) const HTML_SLOT_NAME_GET: &str = "\0HTMLSlotElement.get.name";
pub(crate) const HTML_SLOT_NAME_SET: &str = "\0HTMLSlotElement.set.name";
pub(crate) const NODE_MUTATION_METHODS: [(&str, usize); 4] = [
    ("appendChild", 1),
    ("insertBefore", 2),
    ("removeChild", 1),
    ("replaceChild", 2),
];
pub(crate) const NODE_TRAVERSAL_GETTERS: [(&str, &str); 10] = [
    ("baseURI", "\0Node.get.baseURI"),
    ("ownerDocument", "\0Node.get.ownerDocument"),
    ("isConnected", "\0Node.get.isConnected"),
    ("childNodes", "\0Node.get.childNodes"),
    ("firstChild", "\0Node.get.firstChild"),
    ("lastChild", "\0Node.get.lastChild"),
    ("previousSibling", "\0Node.get.previousSibling"),
    ("nextSibling", "\0Node.get.nextSibling"),
    ("parentNode", "\0Node.get.parentNode"),
    ("parentElement", "\0Node.get.parentElement"),
];
pub(crate) const PARENT_NODE_GETTERS: [(&str, &str); 4] = [
    ("children","\0ParentNode.get.children"),
    ("firstElementChild", "\0ParentNode.get.firstElementChild"),
    ("lastElementChild", "\0ParentNode.get.lastElementChild"),
    ("childElementCount", "\0ParentNode.get.childElementCount"),
];
pub(crate) const DOCUMENT_IMPLEMENTATION_GET: &str = "\0Document.get.implementation";
pub(crate) const DOM_IMPLEMENTATION_CREATE_HTML_DOCUMENT: &str = "\0DOMImplementation.createHTMLDocument";
pub(crate) const DOM_IMPLEMENTATION_HAS_FEATURE: &str = "\0DOMImplementation.hasFeature";
pub(crate) const DOM_IMPLEMENTATION_CREATE_DOCUMENT: &str = "\0DOMImplementation.createDocument";
pub(crate) const DOM_IMPLEMENTATION_CREATE_DOCTYPE: &str = "\0DOMImplementation.createDocumentType";
pub(crate) const ELEMENT_REMOVE: &str = "\0Element.remove";
pub(crate) const CHARACTER_REMOVE: &str = "\0CharacterData.remove";
pub(crate) const DOCTYPE_REMOVE: &str = "\0DocumentType.remove";
pub(crate) fn child_remove_operation(method:&str)->bool {matches!(method,ELEMENT_REMOVE|CHARACTER_REMOVE|DOCTYPE_REMOVE)}
pub(crate) const NODE_HAS_CHILDREN: &str = "\0Node.hasChildNodes";
pub(crate) const NODE_VALUE_GET: &str = "\0Node.get.nodeValue";
pub(crate) const NODE_VALUE_SET: &str = "\0Node.set.nodeValue";
pub(crate) const NODE_TEXT_CONTENT_GET: &str = "\0Node.get.textContent";
pub(crate) const NODE_TEXT_CONTENT_SET: &str = "\0Node.set.textContent";
pub(crate) const DOCUMENT_IMPORT_NODE: &str = "\0Document.importNode";
pub(crate) const DOCUMENT_ADOPT_NODE:&str="\0Document.adoptNode";
pub(crate) const DOCUMENT_CREATE_ATTRIBUTE:&str="\0Document.createAttribute";
pub(crate) const DOCUMENT_CREATE_ATTRIBUTE_NS:&str="\0Document.createAttributeNS";
pub(crate) const DOCUMENT_GET_ELEMENTS_NAME:&str="\0Document.getElementsByName";
pub(crate) const DOCUMENT_GET_ELEMENTS_TAG:&str="\0Document.getElementsByTagName";
pub(crate) const DOCUMENT_GET_ELEMENTS_TAG_NS: &str = "\0Document.getElementsByTagNameNS";
pub(crate) const DOCUMENT_GET_ELEMENTS_CLASS:&str="\0Document.getElementsByClassName";
pub(crate) const ELEMENT_GET_ELEMENTS_TAG:&str="\0Element.getElementsByTagName";
pub(crate) const ELEMENT_GET_ELEMENTS_TAG_NS: &str = "\0Element.getElementsByTagNameNS";
pub(crate) const ELEMENT_GET_ELEMENTS_CLASS:&str="\0Element.getElementsByClassName";
pub(crate) const HTML_TEMPLATE_CONTENT_GET: &str = "\0HTMLTemplateElement.get.content";
pub(crate) const CREATE_CDATA_SECTION: &str = "\0Document.createCDATASection";
pub(crate) const CREATE_PROCESSING_INSTRUCTION: &str = "\0Document.createProcessingInstruction";
pub(crate) const DOCUMENT_DOCTYPE_GET:&str="\0Document.get.doctype";
pub(crate) const DOCUMENT_TYPE_GETTERS:&[(&str,&str)]=&[("name","\0DocumentType.get.name"),("publicId","\0DocumentType.get.publicId"),("systemId","\0DocumentType.get.systemId")];
pub(crate) const PROCESSING_INSTRUCTION_TARGET_GET: &str = "\0ProcessingInstruction.get.target";
pub(crate) const ELEMENT_INNER_HTML_GET: &str = "\0Element.get.innerHTML";
pub(crate) const IFRAME_SANDBOX_GET:&str="\0HTMLIFrameElement.get.sandbox";
pub(crate) const IFRAME_SANDBOX_SET:&str="\0HTMLIFrameElement.set.sandbox";
pub(crate) const TEXTAREA_VALUE_GET: &str = "\0HTMLTextAreaElement.get.value";
pub(crate) const TEXTAREA_VALUE_SET: &str = "\0HTMLTextAreaElement.set.value";
pub(crate) const HTML_FORM_SUBMIT: &str = "\0HTMLFormElement.submit";
pub(crate) const HTML_FORM_RESET: &str = "\0HTMLFormElement.reset";
pub(crate) const ELEMENT_INNER_HTML_SET: &str = "\0Element.set.innerHTML";
pub(crate) const ELEMENT_STYLE_SET: &str = "\0HTMLElement.set.style";

/// One callback from a host-owned synchronous event propagation plan. The host
/// never receives interpreter access; the runtime keeps the current job/realm.
pub struct EventCallback {pub callback:Value,pub target:u32,pub event:u32}

pub(crate) enum NativePromiseFailure {
    Dom(&'static str, &'static str),
    Fatal(String),
}

pub struct NativeChildListChange {
    pub target: u32,
    pub added: Vec<u32>,
    pub removed: Vec<u32>,
    pub previous: Option<u32>,
    pub next: Option<u32>,
}

pub enum NativeNormalizeEvent{Data(u32),Child(NativeChildListChange)}
pub struct NativeNormalizeInfo{pub events:Vec<NativeNormalizeEvent>}

pub struct NativeTextSplitInfo {pub node:u32,pub changes:Vec<NativeChildListChange>}

/// A native string proposal; no author conversion occurs after this plan.
pub struct NativeCharacterDataEdit {
    pub value:Value,
    pub offset:usize,
    pub removed:usize,
    pub inserted:usize,
}

pub struct NativeAttributeChange {
    pub local_name: crate::dom_string::DomString,
    pub namespace: Option<crate::dom_string::DomString>,
    pub old_value: Option<Value>,
}

pub trait Host {
    /// Opt-in exact native named-property reads; None preserves unsupported receivers.
    fn named_units_property(&mut self,_object:u32,_units:&[u16])->Result<Option<Value>,String>{Ok(None)}
    /// Opt-in, native console printer. The runtime may describe genuine Error
    /// identities without invoking author getters or coercions. No object arena
    /// access is granted to the host; unrelated native calls keep original args.
    fn console_message_limit(&self,_object:u32,_method:&str)->Option<usize>{None}
    fn print_console(&mut self,_object:u32,_method:&str,_message:&str)->Result<Value,String>{Err("Native console printing is unavailable.".into())}
    fn prepare_request_call(&mut self,_object:u32,_method:&str,_args:&[Value],_binary:Option<Vec<u8>>)->Result<xhr_interface::Call,xhr_interface::Error>{Err(xhr_interface::Error::Exception("NotSupportedError","Script requests are unavailable.".into()))}
    fn begin_request_notification(&mut self,_notification:xhr_interface::Notification)->Result<Option<usize>,String>{Ok(None)}
    fn finish_request_abort(&mut self,_target:u32,_generation:u64){}
    fn cancel_request_work(&mut self){}
    fn prepare_range_operation(&mut self,_receiver:u32,_method:&str,_args:&[Value]) -> Result<(),range_interface::Error> { Err(range_interface::Error::Exception("NotSupportedError","DOM ranges are unavailable.".into())) }
    fn tree_walker_document(&self, _document: u32) -> bool { false }
    fn tree_walker_node_type(&self, _node: u32) -> Option<u32> { None }
    fn mutation_observation_target(&self, _target: u32) -> Option<bool> { None }
    fn character_data_write_generation(&self, _target: u32, _property: &str) -> Option<u64> { None }
    fn mutation_child_list_target(&self, _target: u32) -> bool { false }
    fn mutation_observation_parent(&self, _target: u32) -> Option<u32> { None }
    fn mutation_tree_generation(&self) -> Option<u64> { None }
    fn observed_attribute_setter(&self, _target: u32, _property: &str) -> Option<&'static str> { None }
    fn mutation_child_list_changes(&mut self, _target: u32, _method: &str, _args: &[Value])
        -> Result<Vec<NativeChildListChange>, String> { Ok(Vec::new()) }
    fn mutation_content_target(&mut self,_target:u32,_property:&str)->Result<Option<u32>,String>{Ok(None)}
    fn mutation_content_changes(&mut self,_receiver:u32,_property:&str,_value:&Value)->Result<Vec<NativeChildListChange>,String>{Ok(Vec::new())}
    fn mutation_attribute_setter_change(&mut self,_target:u32,_property:&str)->Result<Option<(u32,NativeAttributeChange)>,String>{Ok(None)}
    fn mutation_attribute_operation_change(&mut self,_target:u32,_method:&str)->Result<Option<(u32,NativeAttributeChange)>,String>{Ok(None)}
    fn mutation_attribute_change(&mut self, _target: u32, _method: &str, _args: &[Value])
        -> Result<Option<NativeAttributeChange>, String> { Ok(None) }
    fn mutation_node_list(&mut self, _nodes: &[u32]) -> Result<u32, String> {
        Err("MutationRecord nodes are unavailable.".into())
    }
    fn mutation_callback_error(&mut self, _message: &str) {}
    fn promise_returning_operation(&self,_object:u32,_method:&str)->bool {false}
    fn validate_promise_operation(&self,_object:u32,_method:&str)->Result<(),NativePromiseFailure> {Ok(())}
    fn call_promise_operation(&mut self,_object:u32,_method:&str,_promise:NativePromise)->Result<(),NativePromiseFailure> {
        Err(NativePromiseFailure::Fatal("Native Promise operation is unavailable.".into()))
    }
    fn peek_native_promise_effect(&self)->Option<(NativePromise,Option<(&'static str,&'static str)>)> {None}
    fn finish_native_promise_effect(&mut self) {}
    fn begin_form_reset(&mut self,_object:u32)->Result<Option<usize>,String>{Err("Form reset is not authorized by this host.".into())}
    fn prepare_form_reset(&mut self,_object:u32)->Result<(),String>{Err("Form reset is not authorized by this host.".into())}
    fn next_form_reset_output(&mut self,_object:u32)->Result<Option<u32>,String>{Err("Form reset is not authorized by this host.".into())}
    fn complete_form_reset(&mut self,_object:u32)->Result<(),String>{Err("Form reset is not authorized by this host.".into())}
    /// Unconditional pending-plan cleanup, including exhausted job allowances.
    fn finish_form_reset(&mut self,_object:u32){}
    fn synchronous_event_dispatch(&self,_object:u32,_method:&str)->bool {false}
    fn begin_abort_dispatch(&mut self, _controller: u32, _reason: Value) -> Result<usize, String> {
        Err("Abort dispatch is not authorized by this host.".into())
    }
    fn new_aborted_signal(&mut self, _owner: u32, _reason: Value) -> Result<u32, String> {
        Err("AbortSignal creation is not authorized by this host.".into())
    }
    fn prepare_event_dispatch(&mut self)->Result<(),String> {Err("Synchronous event dispatch is not authorized by this host.".into())}
    fn begin_event_dispatch(&mut self,_object:u32,_args:&[Value])->Result<usize,String> {Err("Synchronous event dispatch is not authorized by this host.".into())}
    fn next_event_callback(&mut self,_token:usize)->Result<Option<EventCallback>,String> {Err("Synchronous event dispatch is not authorized by this host.".into())}
    fn event_callback_result(&mut self,_token:usize,_returned_false:bool) {}
    fn event_callback_error(&mut self,_token:usize,_message:&str) {}
    /// Always called after successful begin, including a fatal interpreter fault.
    /// Cleanup must not depend on an unexhausted work/allocation allowance.
    fn finish_event_dispatch(&mut self,_token:usize)->bool {false}
    /// Explicit read/presence fallback to a proven native owner's Object tail.
    /// This does not grant expando, mutation or descriptor authority.
    fn ordinary_lookup_tail(&self, _object:u32) -> bool { false }
    /// A host may restrict the read-only tail to particular absent keys.
    fn ordinary_lookup_tail_property(&self, object:u32, _property:&str) -> bool {
        self.ordinary_lookup_tail(object)
    }
    /// Owner validation without granting an ordinary expando or definition store.
    fn native_access_owner(&self, _object: u32) -> Option<u32> { None }
    /// Real renderer border-box snapshot, when the host implements this method.
    fn bounding_rect_snapshot(&mut self, _object: u32) -> Option<Result<[f64; 4], String>> { None }
    fn canvas_readback_source(
        &mut self,
        _object: u32,
        _output_bytes: usize,
    ) -> Result<&crate::canvas_2d_bitmap::CanvasBitmap, String> {
        Err("Canvas pixels are unavailable.".into())
    }
    /// Real ordinary expando storage for an explicitly proven native family.
    fn expando_owner(&self, _object:u32) -> Option<u32> { None }
    /// Preserve a native named-read/presence fallback after ordinary own keys.
    /// Only explicitly opted-in native receivers use this; other hosts do not.
    fn ordinary_native_fallback(&self, _object:u32, _property:&str) -> bool { false }
    /// Preserve a native lifecycle property while exposing its canonical getter.
    fn native_descriptor_change_allowed(&self,_object:u32,_property:&str)->bool{true}

    fn instance_ancestry(&self, _object:u32) -> Option<NativeAncestry> { None }
    /// Actual exposed immediate prototype, distinct from a read-only Object tail.
    fn exposed_native_prototype(&self, _object:u32) -> bool { false }
    fn mutable_native_wrapper_prototype(&self, _object: u32) -> bool { false }
    /// Whether the ordinary chain can supply inherited enumerable keys.
    fn ordinary_inherited_enumeration(&self, object: u32) -> bool {
        self.exposed_native_prototype(object)
    }
    /// Proven ancestry suffix; does not claim an immediate prototype identity.
    fn native_ancestry_inspection(&self,object:u32)->bool {self.exposed_native_prototype(object)}
    /// Only real browser Window contexts may publish this native interface.
    fn event_target_interface(&self, _window:u32) -> bool { false }
    fn node_interface_constants(&self,_window:u32)->Option<&'static [(&'static str,u16)]> {None}
    /// Only a genuine, already-created child context may expose these handles.
    /// The default grants no Window capability to an arbitrary native handle.
    fn window_context(&self, _window: u32) -> Option<WindowContext> { None }
    fn validate_window_access(&self, _caller: Option<u32>, _target: u32) -> Result<(), String> {
        Err("Foreign Window access is not authorized by this host.".into())
    }
    fn allow_eval_for_window(&mut self, _window: Option<u32>) -> Result<bool, String> {
        self.allow_eval()
    }
    /// Explicit wall-clock capability for Date.now and zero-argument construction.
    /// Return a finite integral UTC Unix timestamp in milliseconds. No implicit
    /// OS clock read or local timezone access is granted by the language runtime.
    fn date_now(&mut self) -> Result<f64, String> {
        Err("Date clock access is not available in this host.".into())
    }
    /// Integral local calendar milliseconds for an integral UTC timestamp.
    fn date_to_local(&mut self, _utc: f64) -> Result<f64, String> {
        Err("Date local-time operations are not implemented in this host.".into())
    }
    /// Inverse local calendar mapping: earlier overlap occurrence, pre-gap bias.
    /// NaN may represent a result proven outside the TimeClip range.
    fn date_to_utc(&mut self, _local: f64) -> Result<f64, String> {
        Err("Date local-time operations are not implemented in this host.".into())
    }
    /// Opt a current-realm native receiver into Promise then-property lookup.
    /// Unregistered/opaque handles remain denied; this grants no new methods.
    fn promise_resolution_receiver(&self, _object: u32) -> bool {
        false
    }
    /// Native attributes take precedence over a realm-owned expando store.
    fn owns_property(&self, _object: u32, _property: &str) -> bool {
        true
    }
    fn readonly_property(&self, _object: u32, _property: &str) -> bool {
        false
    }
    /// Support assignment through an inherited ordinary descriptor on a
    /// native receiver that cannot retain new own properties.
    fn inherited_assignment(&self, _object: u32, _property: &str) -> bool {
        false
    }
    /// Named DOMStringMap setters use ordinary ECMAScript ToString before the
    /// host applies the attribute mutation. This never grants network access.
    fn string_setter(&self, _object: u32, _property: &str) -> bool {
        false
    }
    fn validate_set(
        &self,
        _object: u32,
        _property: &str,
        _value: &Value,
    ) -> Result<(), (&'static str, String)> {
        Ok(())
    }
    /// A native attribute can require a genuine DOMException for a specific
    /// validation result. Existing non-DOM setter failures keep their brands.
    fn setter_dom_exception(&self, _object:u32, _property:&str, _kind:&str)->bool {false}
    fn own_keys(&mut self, _object: u32) -> Result<Vec<String>, String> {
        Err("Enumerating this native host object is not implemented.".into())
    }
    fn inherited_keys(&mut self, object:u32) -> Result<Vec<String>,String> { self.own_keys(object) }
    /// A for-in snapshot can outlive a mutation. Most native families expose
    /// only enumerable own data; others can recheck an inherited member here.
    fn enumeration_member(&mut self, object: u32, name: &str) -> Result<bool, String> {
        self.own_data(object, name).map(|slot| slot.is_some())
    }

    /// Implemented native Window own names and their enumerable attributes.
    /// ES bindings and ordinary expandos are supplied by the active realm.
    fn window_properties(&mut self, _window: u32) -> Result<&'static [(&'static str, bool)], String> {
        Ok(&[])
    }
    /// Present named data properties have writable/enumerable/configurable
    /// descriptors. Native accessor/prototype attributes are not own data.
    fn own_data(&mut self, _object: u32, _property: &str) -> Result<Option<Value>, String> {
        Err("Own-property inspection is not implemented for this host object.".into())
    }
    fn delete(&mut self, _object: u32, _property: &str) -> Result<bool, String> {
        Err("Deleting this native host property is not implemented.".into())
    }
    fn ordinary_prototype(&self, _object: u32) -> bool {
        false
    }
    /// Native named properties can shadow a method. Opt those interfaces
    /// into normal member lookup before arguments, without granting expandos
    /// or an ordinary prototype to otherwise protected host objects.
    fn resolve_member_before_call(&self, _object: u32) -> bool {
        false
    }
    /// Explicitly opt a native value into ordinary toString/valueOf conversion.
    /// The runtime keeps the native receiver and all calls remain host-owned.
    fn primitive_conversion(&self, _object: u32) -> bool {
        false
    }
    fn is_instance(&self, _object: u32, _constructor: u32, _method: &str) -> Option<bool> {
        None
    }
    fn method(&self, _object: u32, _property: &str) -> Option<&'static str> {
        None
    }
    /// Native methods may be borrowed by another object implementing the same
    /// supported interface. The default keeps an individual host capability
    /// bound to its original receiver.
    fn accepts_receiver(&self, object: u32, _method: &str, receiver: u32) -> bool {
        object == receiver
    }

    /// Selected Web IDL global operations resolve null/undefined to their
    /// native realm's global, without accepting arbitrary receivers. None
    /// preserves the existing receiver policy of every other host operation.
    fn global_operation_receiver(&self, _object: u32, _method: &str, _receiver: &Value)
        -> Option<Result<u32, (&'static str, String)>> { None }
    /// A bounded, read-only indexed host collection can request the ordinary
    /// callback loop without gaining access to the runtime or resetting jobs.
    fn indexed_for_each(&self, _object: u32, _method: &str) -> bool {
        false
    }
    /// Only genuine, bounded indexed native collections may enter the lazy
    /// Array iterator path. The returned owner must also match native access.
    fn indexed_iteration_owner(&self, _object: u32) -> Option<u32> { None }
    fn argument_conversions(&self, _object: u32, _method: &str, _args: &[Value]) -> &'static [(usize, bool)] {
        &[]
    }
    fn preserves_dom_string_argument(&self,_object:u32,_method:&str,_index:usize)->bool{false}
    fn preserves_dom_string_setter(&self,_object:u32,_property:&str)->bool{false}
    /// Native interface references checked after ordered primitive conversion.
    fn native_reference_arguments(&self,_object:u32,_method:&str)->&'static [usize] {&[]}
    /// Explicit WebIDL variadic DOMString conversion, in argument order before
    /// method validation. It shares the caller's argument/copy/work budgets.
    fn variadic_string_arguments(&self, _object: u32, _method: &str) -> bool { false }
    fn method_length(&self, _object: u32, _method: &str) -> Option<usize> { None }
    /// Optional owner for a real native method whose first lookup may occur in
    /// another supported realm. This grants no constructor/prototype capability.
    fn native_method_owner(&self, _object:u32, _method:&str) -> Option<u32> { None }
    /// Trusted zero-argument getter with a native receiver: no author calls,
    /// VM-arena results, argument conversion or collection-inhibition escape.
    /// The existing native dispatch still validates brand, owner and budgets.
    fn native_leaf_getter(&self, _object:u32, _method:&str)->bool {false}
    /// A genuine canonical attribute setter may reuse the native write pipeline
    /// without performing ordinary property lookup again. Existing hosts opt out.
    fn native_setter_property(&self,_object:u32,_method:&str)->Option<&'static str>{None}
    /// Only fresh, still-unpublished supported Event constructor/factory results opt
    /// into atomic ordinary-store/unforgeable installation.
    fn unpublished_base_event_owner(&self,_object:u32,_method:&str,_result:u32)->Option<u32>{None}
    fn rollback_unpublished_base_event(&mut self,_event:u32)->Result<(),String>{
        Err("No unpublished Event transaction exists.".into())
    }
    fn attribute_name_snapshot(&mut self,_object:u32)->Result<Vec<crate::dom_string::DomString>,String>{Err("Attribute names are unavailable.".into())}
    fn event_path_snapshot(&mut self,_event:u32)->Result<Vec<u32>,String>{
        Err("Event path access is unavailable.".into())
    }
    /// Preserve original nullish inputs for an explicitly nullable DOMString.
    /// A null returned by object coercion still undergoes ordinary ToString.
    fn nullable_string_argument(&self, _object: u32, _method: &str, _index: usize) -> bool {
        false
    }
    /// Node nullable string setters normalize original nullish inputs to empty.
    fn nullish_string_setter(&self, _object: u32, _property: &str) -> bool { false }
    /// Nullable (boolean or unrestricted double or DOMString): scalar values
    /// retain their type, while other values undergo DOMString conversion.
    fn nullable_scalar_setter(&self,_object:u32,_property:&str)->bool{false}
    /// Nullable DOMString normalizes original null/undefined to native null.
    fn nullable_string_setter(&self,_object:u32,_property:&str)->bool{false}
    /// LegacyNullToEmptyString preserves normal undefined/object conversion.
    fn null_string_setter(&self, _object: u32, _property: &str) -> bool { false }
    /// Pure interface checks interleaved with scalar conversion in table order.
    fn validate_argument_prefix(&self, _object: u32, _method: &str, _next_index: usize, _args: &[Value])
        -> Result<(), (&'static str, String)> { Ok(()) }
    fn required_arguments(&self, _object: u32, _method: &str) -> usize {
        0
    }
    fn validate_call(
        &self,
        _object: u32,
        _method: &str,
        _args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        Ok(())
    }
    fn number_setter(&self, _object: u32, _property: &str) -> bool {
        false
    }
    /// Optional native dictionary conversion. The runtime reads only these
    /// named fields, under its ordinary property and copy budgets; the host
    /// never gains arbitrary access to the interpreter's object arena.
    fn dictionary_argument(
        &self,
        _object: u32,
        _method: &str,
        _args: &[Value],
    ) -> Option<(usize, &'static [&'static str])> {
        None
    }
    /// Optional immediate scalar conversion for an explicitly supported native
    /// dictionary member. It runs after that Get and before the next member Get.
    fn dictionary_member_conversion(&self,_object:u32,_method:&str,_member:&str)->Option<bool>{None}
    fn validate_dictionary_member(&self,_object:u32,_method:&str,_member:&str,_value:&Value)
        ->Result<(),(&'static str,String)>{Ok(())}
    /// Final native validation after dictionary getters have completed.
    fn validate_dictionary_call(&mut self,_object:u32,_method:&str,_args:&[Value],_fields:Option<&[(&str,Value)]>)->Result<(),(&'static str,String)>{Ok(())}
    fn call_with_dictionary(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        _fields: Option<&[(&str, Value)]>,
    ) -> Result<Value, String> {
        self.call(object, method, args)
    }
    /// Math.random is provided explicitly by the embedding host. The language
    /// runtime does not read files, clocks, device identifiers or OS entropy.
    fn random(&mut self) -> Result<f64, String> {
        Err("Math.random is not available in this host.".into())
    }
    /// Reserve native Window properties even when the corresponding operation
    /// is unavailable. They must never become ordinary script expandos.
    fn owns_global_property(&self, _property: &str) -> bool {
        false
    }
    fn owns_window_property(&self, _window: Option<u32>, property: &str) -> bool {
        self.owns_global_property(property)
    }
    /// Property-existence checks are explicit host capabilities. A getter's
    /// undefined value cannot distinguish an absent property from a present one.
    fn has(&mut self, _object: u32, _property: &str) -> Result<bool, String> {
        Err("Property-existence checks are not implemented for this host object.".into())
    }
    /// Explicit host authorization for string compilation. A false result is a
    /// policy rejection (EvalError); an error is a fatal missing capability.
    fn allow_eval(&mut self) -> Result<bool, String> {
        Err("String evaluation is not enabled by this host.".into())
    }
    fn get(&mut self, object: u32, property: &str) -> Result<Value, String>;
    /// One primitive native read with the original script/native caller. Hosts
    /// without caller-sensitive fields retain their original getter behavior.
    fn get_with_caller(&mut self, _caller: Option<u32>, object: u32, property: &str) -> Result<Value, String> {
        self.get(object, property)
    }
    fn reset_output_text(&mut self,_target:u32,_value:Value)->Result<(),String>{Err("Native OUTPUT reset is unavailable.".into())}
    fn prepare_node_normalize(&mut self,_target:u32)->Result<NativeNormalizeInfo,String>{Err("Native normalization preparation is unavailable.".into())}
    fn commit_node_normalize(&mut self,_target:u32)->Result<(),String>{Err("Native normalization publication is unavailable.".into())}
    fn abort_node_normalize(&mut self,_target:u32){}
    fn document_location_owner(&self, _target:u32)->Option<u32>{None}
    fn document_location_access(&self,_caller:Option<u32>,_target:u32)->Result<(),String>{Err("Unknown Document location owner.".into())}
    fn document_location_value(&mut self,_caller:Option<u32>,_target:u32)->Result<Value,String>{Err("No native Document location.".into())}
    fn child_removal_parent(&mut self,_node:u32)->Result<Option<u32>,String>{Err("Native ChildNode removal is unavailable.".into())}
    fn adjacent_receiver_access(&self, _target: u32) -> Result<(), (&'static str, String)> {
        Err(("NotSupportedError","Adjacent insertion is unavailable.".into()))
    }
    fn adjacent_destination(&mut self, _target: u32, _pos: adjacent_interface::Position, _text: bool) -> Result<Option<adjacent_interface::Destination>, (&'static str, String)> {
        Err(("NotSupportedError","Adjacent insertion is unavailable.".into()))
    }
    fn commit_adjacent_text(&mut self, _target: u32, _pos: adjacent_interface::Position, _destination: adjacent_interface::Destination, _text: &Value) -> Result<(), (&'static str, String)> {
        Err(("NotSupportedError","Adjacent Text insertion is unavailable.".into()))
    }
    fn prepare_text_split(&mut self,_target:u32,_args:&[Value])->Result<NativeTextSplitInfo,(&'static str,String)>{Err(("NotSupportedError","Native Text split preparation is unavailable.".into()))}
    fn commit_text_split(&mut self,_target:u32)->Result<u32,String>{Err("Native Text split publication is unavailable.".into())}
    fn abort_text_split(&mut self,_target:u32){}
    fn prepare_character_data_edit(&mut self,_target:u32,_method:&str,_args:&[Value])->Result<NativeCharacterDataEdit,String>{Err("Native CharacterData edit preparation is unavailable.".into())}
    fn set_character_data_splice(&mut self,_target:u32,_value:Value,_offset:usize,_removed:usize,_inserted:usize)->Result<(),String>{Err("Native CharacterData splice storage is unavailable.".into())}
    fn set(&mut self, object: u32, property: &str, value: Value) -> Result<(), String>;
    /// Incumbent native caller, captured independently of a saved method's owner
    /// realm. Called after coercion and immediately before a host operation.
    fn native_operation_caller(&mut self, _window: Option<u32>) {}
    /// Reserve a native stable-state microtask before a host mutation commits.
    /// The ticket is host-owned data, never a script callback or native handle.
    fn prepare_native_microtask(&mut self,_object:u32,_member:&str,_args:&[Value],_setter:bool)->Result<Option<u64>,String>{Ok(None)}
    /// True only if the operation committed the prepared task; false rolls the
    /// reserved queue entry back without touching previously queued jobs.
    fn finish_native_microtask(&mut self,_ticket:u64)->bool{false}
    /// Retire this ticket without running it. Hosts preparing tickets must
    /// override this for committed work; repeated cancellation is harmless.
    fn cancel_native_microtask(&mut self,ticket:u64){self.finish_native_microtask(ticket);}
    fn run_native_microtask(&mut self,_ticket:u64)->Result<(),String>{Err("Native microtask is unavailable.".into())}
    fn call(&mut self, object: u32, method: &str, args: &[Value]) -> Result<Value, String>;
}

fn number_text(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "Infinity".into()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".into()
    } else if value == 0.0 {
        "0".into()
    } else if value.abs() < 1e-6 || value.abs() >= 1e21 {
        let text = format!("{value:e}");
        if let Some((base, exponent)) = text.split_once('e') {
            let exponent = exponent.parse::<i32>().unwrap_or(0);
            format!("{base}e{}{exponent}", if exponent >= 0 { "+" } else { "" })
        } else {
            text
        }
    } else {
        value.to_string()
    }
}

fn js_space(ch: char) -> bool {
    matches!(
        ch,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

fn line_end(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn string_number(text: &str) -> f64 {
    let text = text.trim_matches(js_space);
    if text.is_empty() {
        return 0.0;
    }
    match text {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    if text.starts_with("0x") || text.starts_with("0X") {
        return radix_number(&text[2..], 16).unwrap_or(f64::NAN);
    }
    if text.starts_with("0b") || text.starts_with("0B") {
        return radix_number(&text[2..], 2).unwrap_or(f64::NAN);
    }
    if text.starts_with("0o") || text.starts_with("0O") {
        return radix_number(&text[2..], 8).unwrap_or(f64::NAN);
    }
    if decimal_end(text.as_bytes(), true) == Some(text.len()) {
        text.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

fn radix_number(text: &str, radix: u32) -> Option<f64> {
    if text.is_empty() {
        return None;
    }
    let mut result = 0.0;
    for ch in text.chars() {
        result = result * f64::from(radix) + f64::from(ch.to_digit(radix)?);
    }
    Some(result)
}

/// Longest syntactically complete decimal prefix; an incomplete exponent is
/// left out, as parseFloat requires. Full-number callers also compare lengths.
fn decimal_end(bytes:&[u8],signed:bool)->Option<usize>{crate::decimal_number::prefix_end(bytes,signed)}

#[derive(Debug)]
enum Exception {
    Value(Value),
    Error(&'static str),
}

#[derive(Debug)]
struct Fault {
    message: String,
    location: Option<Box<FaultLocation>>,
    // None is an interpreter/host capability or resource failure, not a JS
    // exception. Page catch/finally cannot turn one into successful execution.
    // Boxing keeps the Result type small throughout recursive evaluation.
    exception: Option<Box<Exception>>,
}

impl Fault {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: None,
            exception: None,
        }
    }
    fn exception(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: None,
            exception: Some(Box::new(Exception::Error(kind))),
        }
    }
    fn thrown(value: Value) -> Self {
        Self {
            message: format!("Uncaught exception: {}", value.to_text()),
            location: None,
            exception: Some(Box::new(Exception::Value(value))),
        }
    }
    fn at(mut self, source: &str, position: usize) -> Self {
        if self.location.is_none() {
            self.location = Some(FaultLocation::new(source, position));
        }
        self
    }
    fn display(self) -> String {
        match self.location {
            Some(location) => format!(
                "JavaScript error at line {}, column {}: {}\nSource: {}",
                location.line, location.column, self.message, location.context
            ),
            None => format!("JavaScript error: {}", self.message),
        }
    }
}

#[derive(Debug)]
struct FaultLocation {
    line: usize,
    column: usize,
    context: Box<str>,
}
impl FaultLocation {
    // Keep diagnostic formatting out of recursive parser/evaluator frames.
    #[inline(never)]
    fn new(source: &str, position: usize) -> Box<Self> {
        let mut position = position.min(source.len());
        while !source.is_char_boundary(position) {
            position -= 1;
        }
        let (mut line, mut column, mut cr) = (1, 1, false);
        for ch in source[..position].chars() {
            if ch == '\n' && cr {
                cr = false;
                continue;
            }
            if line_end(ch) {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            cr = ch == '\r';
        }
        // A bounded source excerpt makes minified real-site errors useful
        // without retaining another complete script or changing JS errors
        // caught by page code. [here] marks the recorded source position.
        let start = source[..position]
            .char_indices()
            .rev()
            .nth(48)
            .map_or(0, |(at, _)| at);
        let end = source[position..]
            .char_indices()
            .nth(64)
            .map_or(source.len(), |(at, _)| position + at);
        let before = source[start..position].escape_debug().to_string();
        let after = source[position..end].escape_debug().to_string();
        Box::new(Self {
            line,
            column,
            context: format!(
                "{}{}[here]{}{}",
                if start > 0 { "..." } else { "" },
                before,
                after,
                if end < source.len() { "..." } else { "" }
            )
            .into_boxed_str(),
        })
    }
}

#[derive(Clone, Debug)]
enum TokenKind {
    Word(String),
    EscapedWord(String),
    Number(f64),
    String(String),
    StringUnits(Vec<u16>),
    Template(String, template_literals::Part),
    RegExp(Rc<regexp::Regex>),
    Punct(&'static str),
    End,
}

struct Token {
    kind: TokenKind,
    start_and_newline: u32,
    end_or_delimiter: u32,
}

impl Token {
    const NEWLINE: u32 = 1 << 31;
    const MAX_SOURCE_BYTES: usize = (Self::NEWLINE - 1) as usize;
    const UNMATCHED_DELIMITER: u32 = u32::MAX;

    fn new(kind: TokenKind, start: usize, end: usize, newline: bool) -> Self {
        debug_assert!(start <= end && end <= Self::MAX_SOURCE_BYTES);
        let start_and_newline = start as u32 | if newline { Self::NEWLINE } else { 0 };
        let end_or_delimiter = if matches!(kind, TokenKind::Punct("(" | "{")) {
            debug_assert_eq!(end, start + 1);
            Self::UNMATCHED_DELIMITER
        } else {
            end as u32
        };
        Self { kind, start_and_newline, end_or_delimiter }
    }

    fn at(&self) -> usize { (self.start_and_newline & !Self::NEWLINE) as usize }
    fn end(&self) -> usize {
        if matches!(self.kind, TokenKind::Punct("(" | "{")) {
            self.at() + 1
        } else {
            self.end_or_delimiter as usize
        }
    }
    fn newline(&self) -> bool { self.start_and_newline & Self::NEWLINE != 0 }

    fn delimiter_end(&self) -> Option<usize> {
        debug_assert!(matches!(self.kind, TokenKind::Punct("(" | "{")));
        (self.end_or_delimiter != Self::UNMATCHED_DELIMITER).then_some(self.end_or_delimiter as usize)
    }
}

// Byte offsets also bound the token index, including for custom test profiles.
const _: () = assert!(Token::MAX_SOURCE_BYTES < Token::UNMATCHED_DELIMITER as usize);

fn index_token_delimiters(tokens: &mut [Token]) {
    let mut parentheses = Token::UNMATCHED_DELIMITER;
    let mut braces = Token::UNMATCHED_DELIMITER;
    // Use the one-byte opening token's redundant end slot to link pending
    // openers, then replace that link with the closing token's index.
    // Each delimiter kind keeps the lookup behavior used for malformed input.
    for index in 0..tokens.len() {
        let (head, opening) = match tokens[index].kind {
            TokenKind::Punct("(") => (&mut parentheses, true),
            TokenKind::Punct(")") => (&mut parentheses, false),
            TokenKind::Punct("{") => (&mut braces, true),
            TokenKind::Punct("}") => (&mut braces, false),
            _ => continue,
        };
        if opening {
            tokens[index].end_or_delimiter = std::mem::replace(head, index as u32);
        } else if *head != Token::UNMATCHED_DELIMITER {
            *head = std::mem::replace(&mut tokens[*head as usize].end_or_delimiter, index as u32);
        }
    }
    for mut head in [parentheses, braces] {
        while head != Token::UNMATCHED_DELIMITER {
            head = std::mem::replace(&mut tokens[head as usize].end_or_delimiter, Token::UNMATCHED_DELIMITER);
        }
    }
}

fn reserved(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "let"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
            | "await"
            | "implements"
            | "interface"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
    )
}

#[derive(Clone, Copy)]
enum LexDelimiter {
    Parenthesis,
    Control,
    ForControl,
    SwitchControl,
    Parameters(bool),
    Object,
    Block,
    SwitchBlock,
    Function(bool),
    Array,
}

/// Lexical goals for the supported grammar. A closing control parenthesis or
/// statement block starts a statement; a closing object/function expression
/// ends an expression. This distinction is essential for /regexp/ vs division.
struct LexGoal {
    end_expression: bool,
    statement: bool,
    previous: String,
    previous_is_property: bool,
    previous_is_label: bool,
    jump_end: bool,
    delimiters: Vec<LexDelimiter>,
    function_head: Option<bool>,
    function_body: Option<bool>,
    switch_body: bool,
    case_expressions: Vec<(usize, usize)>,
}
impl LexGoal {
    fn new() -> Self {
        Self {
            end_expression: false,
            statement: true,
            previous: String::new(),
            previous_is_property: false,
            previous_is_label: false,
            jump_end: false,
            delimiters: Vec::new(),
            function_head: None,
            function_body: None,
            switch_body: false,
            case_expressions: Vec::new(),
        }
    }
    fn accept(&mut self, token: &TokenKind, newline: bool) {
        let body = self.function_body.take();
        let switch_body = std::mem::take(&mut self.switch_body);
        match token {
            TokenKind::Word(word) | TokenKind::EscapedWord(word) => {
                let keyword = if matches!(token, TokenKind::Word(_)) { word.as_str() } else { "" };
                let property = matches!(self.previous.as_str(), "." | "?.");
                let label = !reserved(word)
                    && (self.statement || newline && self.end_expression)
                    && !property;
                let jump_end = !property
                    && (matches!(keyword, "break" | "continue" | "return")
                        || !newline
                            && self.jump_end
                            && matches!(self.previous.as_str(), "break" | "continue"));
                if !property
                    && matches!(keyword, "case" | "default")
                    && matches!(self.delimiters.last(), Some(LexDelimiter::SwitchBlock))
                {
                    self.case_expressions.push((self.delimiters.len(), 0));
                    self.end_expression = false;
                    self.statement = false;
                } else if keyword == "function" && !property {
                    // A contextual async prefix keeps the statement position it
                    // had before the word, just like a possible label there.
                    let declaration = if !newline && self.previous == "async" && !self.previous_is_property {
                        self.previous_is_label
                    } else {
                        self.statement || newline && self.end_expression
                    };
                    self.function_head = Some(!declaration);
                    self.end_expression = false;
                    self.statement = false;
                } else if !property
                    && (keyword == "of"
                        && self.end_expression
                        && matches!(self.delimiters.last(), Some(LexDelimiter::ForControl))
                        || matches!(
                            keyword,
                            "return"
                                | "throw"
                                | "typeof"
                                | "void"
                                | "delete"
                                | "new"
                                | "in"
                                | "instanceof"
                                | "var"
                                | "let"
                                | "const"
                                | "yield"
                                | "await"
                        ))
                {
                    self.end_expression = false;
                    self.statement = false;
                } else if !property
                    && matches!(
                        keyword,
                        "else" | "do" | "try" | "finally" | "break" | "continue"
                    )
                {
                    self.end_expression = false;
                    self.statement = true;
                } else {
                    self.end_expression = true;
                    self.statement = false;
                }
                self.previous = keyword.to_owned();
                self.previous_is_property = property;
                self.previous_is_label = label;
                self.jump_end = jump_end;
            }
            TokenKind::Punct(punct) => {
                match *punct {
                    "(" => {
                        let delimiter = if let Some(expression) = self.function_head.take() {
                            LexDelimiter::Parameters(expression)
                        } else if !self.previous_is_property && self.previous == "for" {
                            LexDelimiter::ForControl
                        } else if !self.previous_is_property && self.previous == "switch" {
                            LexDelimiter::SwitchControl
                        } else if !self.previous_is_property
                            && matches!(self.previous.as_str(), "if" | "while" | "with" | "catch")
                        {
                            LexDelimiter::Control
                        } else {
                            LexDelimiter::Parenthesis
                        };
                        self.delimiters.push(delimiter);
                        self.end_expression = false;
                        self.statement = false;
                    }
                    ")" => match self.delimiters.pop() {
                        Some(LexDelimiter::SwitchControl) => {
                            self.switch_body = true;
                            self.end_expression = false;
                            self.statement = true;
                        }
                        Some(LexDelimiter::Control | LexDelimiter::ForControl) => {
                            self.end_expression = false;
                            self.statement = true;
                        }
                        Some(LexDelimiter::Parameters(expression)) => {
                            self.function_body = Some(expression);
                            self.end_expression = false;
                            self.statement = false;
                        }
                        _ => {
                            self.end_expression = true;
                            self.statement = false;
                        }
                    },
                    "{" => {
                        let delimiter = if let Some(expression) = body {
                            LexDelimiter::Function(expression)
                        } else if switch_body {
                            LexDelimiter::SwitchBlock
                        } else if self.previous == "=>" {
                            LexDelimiter::Function(true)
                        } else if self.statement
                            || self.previous == ")"
                            || newline && self.end_expression
                        {
                            LexDelimiter::Block
                        } else {
                            LexDelimiter::Object
                        };
                        self.delimiters.push(delimiter);
                        self.end_expression = false;
                        self.statement = !matches!(delimiter, LexDelimiter::Object);
                    }
                    "}" => {
                        self.end_expression = matches!(
                            self.delimiters.pop(),
                            Some(LexDelimiter::Object | LexDelimiter::Function(true))
                        );
                        self.statement = !self.end_expression;
                    }
                    "[" => {
                        self.delimiters.push(LexDelimiter::Array);
                        self.end_expression = false;
                        self.statement = false;
                    }
                    "]" => {
                        self.delimiters.pop();
                        self.end_expression = true;
                        self.statement = false;
                    }
                    ";" => {
                        self.end_expression = false;
                        self.statement = true;
                    }
                    ":" if self.previous_is_label => {
                        self.end_expression = false;
                        self.statement = true;
                    }
                    "?" => {
                        if let Some((depth, ternaries)) = self.case_expressions.last_mut() {
                            if *depth == self.delimiters.len() {
                                *ternaries += 1;
                            }
                        }
                        self.end_expression = false;
                        self.statement = false;
                    }
                    ":" => {
                        self.end_expression = false;
                        self.statement = false;
                        if let Some((depth, ternaries)) = self.case_expressions.last_mut() {
                            if *depth == self.delimiters.len() {
                                if *ternaries != 0 {
                                    *ternaries -= 1;
                                } else {
                                    self.case_expressions.pop();
                                    self.statement = true;
                                }
                            }
                        }
                    }
                    "++" | "--" => {
                        if newline {
                            self.end_expression = false;
                        }
                        self.statement = false;
                    }
                    _ => {
                        self.end_expression = false;
                        self.statement = false;
                    }
                }
                if !matches!(*punct, "(") {
                    self.function_head = None;
                }
                self.previous = (*punct).into();
                self.previous_is_property = false;
                self.previous_is_label = false;
                self.jump_end = false;
            }
            TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::StringUnits(_)
            | TokenKind::RegExp(_)
            | TokenKind::Template(
                _,
                template_literals::Part::NoSubstitution | template_literals::Part::Tail,
            ) => {
                self.end_expression = true;
                self.statement = false;
                self.previous.clear();
                self.previous_is_property = false;
                self.previous_is_label = false;
                self.jump_end = false;
            }
            TokenKind::Template(
                _,
                template_literals::Part::Head | template_literals::Part::Middle,
            )
            | TokenKind::End => {}
        }
    }
}

fn regexp_literal(
    source: &str,
    at: &mut usize,
    budget: &mut usize,
) -> Result<Rc<regexp::Regex>, Fault> {
    let start = *at;
    *at += 1;
    let pattern_start = *at;
    let mut in_class = false;
    let mut escaped = false;
    while let Some(ch) = source[*at..].chars().next() {
        if line_end(ch) {
            return Err(Fault::exception(
                "SyntaxError",
                "A regexp literal cannot contain a line ending.",
            )
            .at(source, *at));
        }
        if !escaped && ch == '/' && !in_class {
            let pattern_end = *at;
            *at += 1;
            let flags_start = *at;
            while let Some(flag) = source[*at..].chars().next().filter(|&ch| identifier_names::part(ch)) {
                *at += flag.len_utf8();
            }
            if source[*at..].starts_with('\\') {
                return Err(Fault::exception("SyntaxError", "A regexp flag cannot contain an escape.").at(source, *at));
            }
            return regexp::Regex::compile(
                &source[pattern_start..pattern_end],
                &source[flags_start..*at],
                budget,
            )
            .map(Rc::new)
            .map_err(|error| Runtime::regexp_fault(error).at(source, start));
        }
        if !escaped {
            match ch {
                '[' => in_class = true,
                ']' => in_class = false,
                _ => {}
            }
        }
        escaped = !escaped && ch == '\\';
        *at += ch.len_utf8();
    }
    Err(
        Fault::exception("SyntaxError", "Unterminated regular-expression literal.")
            .at(source, start),
    )
}

#[cfg(test)]
fn tokenize(source: &str, regexp_budget: &mut usize) -> Result<Vec<Token>, Fault> {
    tokenize_with_capacity(source, regexp_budget, limits::REGRESSION_CAPACITIES)
}

#[cfg(test)]
fn tokenize_with_capacity(source: &str, regexp_budget: &mut usize, capacity: limits::Capacities) -> Result<Vec<Token>, Fault> {
    tokenize_for_parse(source, regexp_budget, capacity, None).map(|(tokens, _)| tokens)
}

fn check_compilation_deadline(deadline: Option<Instant>) -> Result<(), Fault> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(Fault::new("Script compilation deadline reached."));
    }
    Ok(())
}

fn compilation_storage_limit(capacity: limits::Capacities) -> Fault {
    Fault::new(format!("Script compilation storage limit reached ({} MiB).", capacity.compilation_bytes / (1024 * 1024)))
}

fn tokenize_for_parse(
    source: &str, regexp_budget: &mut usize, capacity: limits::Capacities, deadline: Option<Instant>,
) -> Result<(Vec<Token>, usize), Fault> {
    let source_limit = capacity.source_bytes.min(Token::MAX_SOURCE_BYTES);
    if source.len() > source_limit {
        return Err(Fault::new(format!("Script source limit reached ({} KiB).", source_limit / 1024)));
    }
    let bytes = source.as_bytes();
    let (mut at, mut newline) = (0, false);
    let mut tokens = Vec::new();
    let max_records = capacity.tokens.min(source.len()) + 1;
    let mut owned_bytes = source.len().saturating_mul(2);
    let mut next_deadline_check = 0usize;
    let mut goal = LexGoal::new();
    // Moving lexical goals into bounded interpolation frames preserves their
    // regexp/division context without copying enclosing delimiter stacks.
    let mut templates: Vec<(LexGoal, usize)> = Vec::new();
    let mut regexp_bytes = 0usize;
    while at < source.len() {
        if at >= next_deadline_check {
            check_compilation_deadline(deadline).map_err(|error| error.at(source, at))?;
            next_deadline_check = at + 65_536;
        }
        let ch = source[at..].chars().next().unwrap();
        if js_space(ch) {
            newline |= line_end(ch);
            at += ch.len_utf8();
            continue;
        }
        if source[at..].starts_with("//") {
            while at < source.len() {
                if at >= next_deadline_check {
                    check_compilation_deadline(deadline).map_err(|error| error.at(source, at))?;
                    next_deadline_check = at + 65_536;
                }
                let ch = source[at..].chars().next().unwrap();
                if line_end(ch) {
                    break;
                }
                at += ch.len_utf8();
            }
            continue;
        }
        if source[at..].starts_with("/*") {
            let Some(end) = source[at + 2..].find("*/") else {
                return Err(
                    Fault::exception("SyntaxError", "Unterminated block comment.").at(source, at),
                );
            };
            let end = at + 2 + end + 2;
            newline |= source[at..end].chars().any(line_end);
            at = end;
            continue;
        }
        if tokens.len() >= capacity.tokens {
            return Err(
                Fault::new(format!("Script token limit reached ({}).", capacity.tokens)).at(source, at),
            );
        }
        if newline && goal.jump_end {
            goal.end_expression = false;
            goal.statement = true;
        }
        let start = at;
        let kind = if ch == '`'
            || ch == '}' && templates.last().is_some_and(|(_, braces)| *braces == 0)
        {
            use template_literals::Part;
            let initial = ch == '`';
            at += 1;
            let (text, tail) = template_literals::chunk(source, &mut at, start, capacity.string_bytes, deadline)?;
            let part = match (initial, tail) {
                (true, true) => Part::NoSubstitution,
                (true, false) => {
                    if templates.len() >= MAX_PARSE_DEPTH {
                        return Err(
                            Fault::new("Template nesting limit reached (64).").at(source, start)
                        );
                    }
                    let mut expression_goal = LexGoal::new();
                    expression_goal.statement = false;
                    templates.push((std::mem::replace(&mut goal, expression_goal), 0));
                    Part::Head
                }
                (false, true) => {
                    goal = templates.pop().unwrap().0;
                    Part::Tail
                }
                (false, false) => {
                    goal = LexGoal::new();
                    goal.statement = false;
                    Part::Middle
                }
            };
            TokenKind::Template(text, part)
        } else if ch == '/' && !goal.end_expression {
            let expression = regexp_literal(source, &mut at, regexp_budget)?;
            regexp_bytes = regexp_bytes.saturating_add(expression.retained_bytes);
            if regexp_bytes > capacity.retained_regexp {
                return Err(Fault::new(format!("Retained regexp limit reached ({} MiB).", capacity.retained_regexp / (1024 * 1024))).at(source, start));
            }
            TokenKind::RegExp(expression)
        } else if identifier_names::start(ch) || ch == '\\' {
            identifier_names::read(source, &mut at)?
        } else if ch.is_ascii_digit()
            || (ch == '.' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit))
        {
            let tail = &source[at..];
            let value = if tail.starts_with("0x")
                || tail.starts_with("0X")
                || tail.starts_with("0b")
                || tail.starts_with("0B")
                || tail.starts_with("0o")
                || tail.starts_with("0O")
            {
                let radix = match bytes[at + 1].to_ascii_lowercase() {
                    b'x' => 16,
                    b'b' => 2,
                    _ => 8,
                };
                at += 2;
                let digits = at;
                while at < source.len() && source[at..].chars().next().unwrap().is_digit(radix) {
                    at += 1;
                }
                radix_number(&source[digits..at], radix).ok_or_else(|| {
                    Fault::exception("SyntaxError", "A radix literal needs digits.")
                        .at(source, start)
                })?
            } else {
                if ch == '0' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit) {
                    return Err(Fault::new(
                        "Legacy leading-zero number literals are not supported.",
                    )
                    .at(source, start));
                }
                at += decimal_end(tail.as_bytes(), false).unwrap();
                source[start..at].parse::<f64>().map_err(|_| {
                    Fault::exception("SyntaxError", "Invalid number literal.").at(source, start)
                })?
            };
            if source[at..].starts_with('\\')
                || source[at..].chars().next().is_some_and(|ch| !ch.is_ascii() && identifier_names::part(ch))
            {
                return Err(Fault::exception("SyntaxError", "An identifier cannot immediately follow a numeric literal.").at(source, at));
            }
            if at < source.len()
                && (identifier_names::part(source[at..].chars().next().unwrap())
                    || source[at..].starts_with('.'))
            {
                return Err(Fault::new("Invalid or unsupported numeric literal.").at(source, start));
            }
            TokenKind::Number(value)
        } else if ch == '\'' || ch == '"' {
            at += 1;
            let mut value = string_units::LiteralBuffer::new();
            let mut closed = false;
            while at < source.len() {
                let ch = source[at..].chars().next().unwrap();
                at += ch.len_utf8();
                if ch == source[start..].chars().next().unwrap() {
                    closed = true;
                    break;
                }
                if line_end(ch) {
                    return Err(
                        Fault::exception("SyntaxError", "Unescaped newline in string.")
                            .at(source, at - ch.len_utf8()),
                    );
                }
                if ch != '\\' {
                    value.push(ch);
                } else {
                    let escaped = source[at..].chars().next().ok_or_else(|| {
                        Fault::exception("SyntaxError", "Unterminated string escape.")
                            .at(source, at)
                    })?;
                    at += escaped.len_utf8();
                    match escaped {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        'b' => value.push('\u{8}'),
                        'f' => value.push('\u{c}'),
                        'v' => value.push('\u{b}'),
                        '\n' | '\u{2028}' | '\u{2029}' => {}
                        '\r' => {
                            if bytes.get(at) == Some(&b'\n') {
                                at += 1;
                            }
                        }
                        '0' if !bytes.get(at).is_some_and(u8::is_ascii_digit) => value.push('\0'),
                        '0'..='9' => {
                            return Err(Fault::new(
                                "Legacy numeric string escapes are not supported.",
                            )
                            .at(source, at - 1))
                        }
                        'x' | 'u' => {
                            let first = if escaped == 'u' {
                                unicode_escape(source, &mut at, deadline)?
                            } else { hex_escape(source, &mut at, 2)? };
                            if (0xd800..=0xdbff).contains(&first) && source[at..].starts_with("\\u") {
                                let mut next = at + 2;
                                let low = unicode_escape(source, &mut next, deadline)?;
                                if (0xdc00..=0xdfff).contains(&low) {
                                    at = next;
                                    let scalar = 0x10000 + ((first - 0xd800) << 10) + low - 0xdc00;
                                    value.push(char::from_u32(scalar).expect("valid surrogate pair"));
                                } else {
                                    value.push_unpaired(first as u16, capacity.string_bytes)?;
                                }
                            } else if let Some(c) = char::from_u32(first) {
                                value.push(c);
                            } else {
                                value.push_unpaired(first as u16, capacity.string_bytes)?;
                            }
                        }
                        other => value.push(other),
                    }
                }
                if value.len() > capacity.string_bytes {
                    return Err(Fault::new(format!("String limit reached ({} KiB).", capacity.string_bytes / 1024)).at(source, start));
                }
            }
            if !closed {
                return Err(
                    Fault::exception("SyntaxError", "Unterminated string literal.")
                        .at(source, start),
                );
            }
            value.finish(capacity.string_bytes)?
        } else if !ch.is_ascii() && identifier_names::part(ch) {
            return Err(Fault::exception("SyntaxError", "This character can only continue an identifier.").at(source, start));
        } else {
            let Some(punct) = [
                ">>>=", "**=", "&&=", "||=", "??=", "<<=", ">>=", "===", "!==", ">>>", "...", "=>",
                "==", "!=", "<=", ">=", "**", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=",
                "/=", "%=", "&=", "|=", "^=", "<<", ">>", "{", "}", "(", ")", "[", "]", ".", ";",
                ",", "?", ":", "+", "-", "*", "/", "%", "<", ">", "=", "!", "~", "&", "|", "^",
            ]
            .into_iter()
            .find(|punct| {
                source[at..].starts_with(punct)
                    && (*punct != "?." || !bytes.get(at + 2).is_some_and(u8::is_ascii_digit))
            }) else {
                return Err(Fault::new(format!(
                    "Unsupported token {ch:?}. This syntax is outside the JavaScript subset."
                ))
                .at(source, start));
            };
            at += punct.len();
            TokenKind::Punct(punct)
        };
        if let Some((_, braces)) = templates.last_mut() {
            match kind {
                TokenKind::Punct("{") => *braces += 1,
                TokenKind::Punct("}") => *braces -= 1,
                _ => {}
            }
        }
        goal.accept(&kind, newline);
        push_token(&mut tokens, Token::new(kind, start, at, newline), max_records, &mut owned_bytes, capacity)
            .map_err(|error| error.at(source, start))?;
        newline = false;
    }
    if !templates.is_empty() {
        return Err(
            Fault::exception("SyntaxError", "Unterminated template substitution.").at(source, at),
        );
    }
    check_compilation_deadline(deadline).map_err(|error| error.at(source, at))?;
    push_token(&mut tokens, Token::new(TokenKind::End, at, at, newline), max_records, &mut owned_bytes, capacity)?;
    let charged = owned_bytes.saturating_add(tokens.capacity().saturating_mul(size_of::<Token>()));
    Ok((tokens, charged))
}

fn push_token(
    tokens: &mut Vec<Token>, token: Token, max_records: usize, owned_bytes: &mut usize, capacity: limits::Capacities,
) -> Result<(), Fault> {
    let payload = match &token.kind {
        TokenKind::Word(value) | TokenKind::EscapedWord(value) | TokenKind::String(value)
        | TokenKind::Template(value, _) => value.capacity(),
        TokenKind::StringUnits(value) => value.capacity().saturating_mul(size_of::<u16>()),
        TokenKind::RegExp(value) => value.retained_bytes,
        _ => 0,
    };
    *owned_bytes = owned_bytes.saturating_add(payload);
    let usable_records = capacity.compilation_bytes.saturating_sub(*owned_bytes) / size_of::<Token>();
    if tokens.capacity() > usable_records || tokens.len() == usable_records {
        return Err(compilation_storage_limit(capacity));
    }
    if tokens.len() == tokens.capacity() {
        let next = tokens.capacity().saturating_mul(2).max(8).min(max_records).min(usable_records);
        tokens.try_reserve_exact(next - tokens.len())
            .map_err(|_| Fault::new("Script token storage reservation failed."))?;
    }
    tokens.push(token);
    Ok(())
}

#[test]
fn compact_token_records_preserve_byte_offsets_newlines_and_capacity() {
    let capacity = limits::Capacities { tokens: 3, ..limits::REGRESSION_CAPACITIES };
    let mut regexp_work = limits::BROWSER.regexp_work;
    let tokens = tokenize_with_capacity("'é'\n;1", &mut regexp_work, capacity).unwrap();
    let spans: Vec<_> = tokens.iter().map(|t| (t.at(), t.end(), t.newline())).collect();
    assert_eq!(spans, [(0, 4, false), (5, 6, true), (6, 7, false), (7, 7, false)]);
    assert!(tokens.capacity() <= capacity.tokens + 1);
    let error = tokenize_with_capacity(";;;;", &mut regexp_work, capacity).err().unwrap();
    assert!(error.message.contains("Script token limit reached (3)."));
}

#[test]
fn token_delimiter_index_preserves_byte_spans_and_independent_matching() {
    let mut regexp_work = limits::BROWSER.regexp_work;
    let mut tokens = tokenize("\n({)}(({", &mut regexp_work).unwrap();
    index_token_delimiters(&mut tokens);
    let spans: Vec<_> = tokens.iter().map(|token| (token.at(), token.end())).collect();
    assert_eq!(spans, [(1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 8)]);
    for (index, expected) in [(0, Some(2)), (1, Some(3)), (4, None), (5, None), (6, None)] {
        assert_eq!(tokens[index].delimiter_end(), expected);
    }
    assert!(tokens[0].newline());
}

#[test]
fn compilation_storage_and_deadline_reject_before_program_registration() {
    let source = ";".repeat(110);
    let mut regexp_work = limits::BROWSER.regexp_work;
    let limited = limits::Capacities { compilation_bytes: 8192, ..limits::REGRESSION_CAPACITIES };
    assert!(tokenize_with_capacity(&source, &mut regexp_work, limited).is_ok());
    let error = Parser::parse_context_with_capacity(&source, &mut regexp_work, false, 0, limited, limits::REGRESSION_HEAP)
        .err().unwrap();
    assert!(error.message.contains("Script compilation storage limit"), "{}", error.display());
    let error = Parser::parse_invocation_context("1;", &mut regexp_work, false, 0, limits::REGRESSION_CAPACITIES,
        limits::REGRESSION_HEAP, Some(Instant::now()), false, false, false).err().unwrap();
    assert_eq!(error.message, "Script compilation deadline reached.");
    assert!(error.exception.is_none());
}

// The caller has consumed the u. Both forms produce UTF-16 string
// values; the caller decides how to represent possible surrogate units.
fn unicode_escape(source: &str, at: &mut usize, deadline: Option<Instant>) -> Result<u32, Fault> {
    if source.as_bytes().get(*at) != Some(&b'{') { return hex_escape(source, at, 4); }
    *at += 1; // Opening brace after the already-consumed backslash-u.
    let start = *at;
    let mut value = 0u32;
    while let Some(digit) = source
        .as_bytes()
        .get(*at)
        .and_then(|b| (*b as char).to_digit(16))
    {
        value = value.saturating_mul(16).saturating_add(digit);
        *at += 1;
        if (*at-start) % 65_536 == 0 { check_compilation_deadline(deadline).map_err(|e| e.at(source, *at))?; }
        if value > 0x10ffff {
            return Err(
                Fault::exception("SyntaxError", "Unicode code point is out of range.")
                    .at(source, start),
            );
        }
    }
    if *at == start || source.as_bytes().get(*at) != Some(&b'}') {
        return Err(
            Fault::exception("SyntaxError", "Invalid braced Unicode escape.").at(source, start),
        );
    }
    *at += 1;
    Ok(value)
}

/// Scan exactly one chunk, after the opening backtick or closing substitution
/// brace. The boolean says whether a backtick (rather than ${) ended it.

fn hex_escape(source: &str, at: &mut usize, count: usize) -> Result<u32, Fault> {
    let end = at.saturating_add(count);
    let bytes = source.as_bytes();
    if end > bytes.len() || !bytes[*at..end].iter().all(u8::is_ascii_hexdigit) {
        return Err(
            Fault::exception("SyntaxError", "Invalid hexadecimal string escape.").at(source, *at),
        );
    }
    let value = u32::from_str_radix(&source[*at..end], 16).unwrap();
    *at = end;
    Ok(value)
}

type ExprId = usize;
type StmtId = usize;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Declaration {
    Var,
    Let,
    Const,
}

#[derive(Clone, Copy)]
struct Argument {
    expression: ExprId,
    spread: bool,
}

#[derive(Clone)]
enum ExprKind {
    Literal(Value),
    TemplateLiteral(String, Vec<(ExprId, String)>),
    TemplateObject(Rc<template_literals::TaggedTemplateSite>),
    RegExp(Rc<regexp::Regex>),
    Name(String),
    This,
    NewTarget,
    SuperCall(Vec<Argument>),
    SuperMember(String),
    Unary(&'static str, ExprId),
    Yield { operand: Option<ExprId>, delegate: bool },
    Binary(&'static str, ExprId, ExprId),
    Conditional(ExprId, ExprId, ExprId),
    Sequence(ExprId, ExprId),
    Assign(&'static str, ExprId, ExprId),
    Update(ExprId, bool, bool),
    Member(ExprId, ExprId),
    Call(ExprId, Vec<Argument>),
    OptionalChain(ExprId, Vec<optional_chaining::Step>),
    Construct(ExprId, Vec<Argument>),
    Function(usize),
    Class(usize),
    Object(Vec<object_literals::Member>),
    Array(Vec<Option<Argument>>),
    Pattern(binding_patterns::Id),
}

struct Expr {
    kind: ExprKind,
    at: usize,
    parenthesized: bool,
}

#[derive(Clone)]
enum StmtKind {
    Empty,
    Expr(ExprId),
    Block(Vec<StmtId>),
    Declare(Declaration, Vec<(binding_patterns::Id, Option<ExprId>)>),
    Function(String, usize),
    If(ExprId, StmtId, Option<StmtId>),
    While(ExprId, StmtId),
    DoWhile(StmtId, ExprId),
    For(Option<StmtId>, Option<ExprId>, Option<ExprId>, StmtId),
    ForIn(StmtId, ExprId, StmtId),
    ForOf(StmtId, ExprId, StmtId, bool),
    Switch {
        discriminant: ExprId,
        cases: Vec<(Option<ExprId>, usize)>,
        body: Vec<StmtId>,
    },
    Return(Option<ExprId>),
    Yield(Option<ExprId>),
    YieldFrom(ExprId),
    Throw(ExprId),
    Try(
        StmtId,
        Option<(Option<binding_patterns::Id>, StmtId)>,
        Option<StmtId>,
    ),
    Break(StmtId),
    Continue(StmtId),
}

struct Stmt {
    kind: StmtKind,
    at: usize,
}

struct Parameter {
    pattern: binding_patterns::Id,
    initial: Option<ExprId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FunctionKind {
    Ordinary,
    Async,
    Generator,
}

impl FunctionKind {
    fn from_async(asynchronous: bool) -> Self {
        if asynchronous { Self::Async } else { Self::Ordinary }
    }

    fn source_prefix(self) -> &'static str {
        match self {
            Self::Ordinary => "function",
            Self::Async => "async function",
            Self::Generator => "function*",
        }
    }
}

struct Template {
    source_start: usize,
    source_end: usize,
    dynamic_source: bool,
    arrow: bool,
    kind: FunctionKind,
    resumption_plan: Vec<async_plan::Instruction>,
    constructible: bool,
    class_constructor: bool,
    class_element: bool,
    derived_constructor: bool,
    default_constructor: bool,
    strict: bool,
    name: Option<String>,
    params: Vec<Parameter>,
    /// A non-simple rest list stores its final binding name in params.
    rest: bool,
    trailing_parameter_comma: bool,
    body: Vec<StmtId>,
}

struct Program {
    // Immutable owned backing; Cell permits shared unpublished parsed handles.
    // Publication writes the same snapshot without changing executable state.
    retained_bytes:std::cell::Cell<usize>,
    source: String,
    strict: bool,
    exprs: Vec<Expr>,
    stmts: Vec<Stmt>,
    templates: Vec<Template>,
    classes: Vec<class_parser::Class>,
    class_members: usize,
    patterns: Vec<binding_patterns::Pattern>,
    body: Vec<StmtId>,
    chain_steps: usize,
    resumption_plan_nodes: usize,
}

impl Program {
    fn node_count(&self) -> usize {
        self.exprs.len() + self.stmts.len() + self.chain_steps + self.patterns.len()
            + self.classes.len() + self.class_members + self.resumption_plan_nodes
    }

    fn compilation_ast_bytes(&self) -> usize {
        fn arena<T>(items: &Vec<T>) -> usize { items.capacity().saturating_mul(size_of::<T>()) }
        // Charge arena reservations and a per-node allowance for variable
        // child lists. Source-sized text copies are charged separately.
        arena(&self.exprs)
            .saturating_add(arena(&self.stmts))
            .saturating_add(arena(&self.templates))
            .saturating_add(arena(&self.classes))
            .saturating_add(arena(&self.patterns))
            .saturating_add(arena(&self.body))
            .saturating_add(self.node_count().saturating_mul(16))
            .saturating_add(self.resumption_plan_nodes.saturating_mul(size_of::<async_plan::Instruction>()))
            .saturating_add(self.source.len())
    }
}

struct DeferredBody {
    template: usize,
    start: usize,
    end: usize,
    strict: bool,
    function_depth: usize,
    new_target_allowed: bool,
    super_property_allowed: bool,
    super_call_allowed: bool,
}

struct Parser {
    capacity: limits::Capacities,
    heap: limits::HeapLimits,
    program: Program,
    tokens: Vec<Token>,
    token_storage_bytes: usize,
    compilation_deadline: Option<Instant>,
    deferred_bodies: Vec<DeferredBody>,
    current: usize,
    depth: usize,
    expression_frames: usize,
    expression_storage_bytes: usize,
    argument_entries: usize,
    if_branches: usize,
    strict: bool,
    loops: Vec<StmtId>,
    breakables: Vec<StmtId>,
    labels: Vec<(String, StmtId, bool)>,
    allow_in: bool,
    functions: usize,
    // Present only in an async function body. Nested function and parameter
    // goals cannot claim a suspension point from the enclosing template.
    await_count: Option<usize>,
    yield_count: Option<usize>,
    new_target_allowed: bool,
    super_property_allowed: bool,
    super_call_allowed: bool,
}

impl Parser {
    #[cfg(test)]
    fn parse(source: &str, regexp_budget: &mut usize) -> Result<Program, Fault> {
        Self::parse_context(source, regexp_budget, false, 0)
    }
    #[cfg(test)]
    fn parse_context(
        source: &str,
        regexp_budget: &mut usize,
        strict: bool,
        active_depth: usize,
    ) -> Result<Program, Fault> {
        Self::parse_context_with_capacity(source, regexp_budget, strict, active_depth,
            limits::REGRESSION_CAPACITIES, limits::REGRESSION_HEAP)
    }

    fn parse_context_with_capacity(source: &str, regexp_budget: &mut usize,
        strict: bool, active_depth: usize, capacity: limits::Capacities,
        heap: limits::HeapLimits) -> Result<Program, Fault> {
        Self::parse_invocation_context(source, regexp_budget, strict, active_depth, capacity, heap, None, false, false, false)
    }

    fn parse_invocation_context(source: &str, regexp_budget: &mut usize,
        strict: bool, active_depth: usize, capacity: limits::Capacities,
        heap: limits::HeapLimits, compilation_deadline: Option<Instant>, new_target_allowed: bool,
        super_property_allowed: bool, super_call_allowed: bool) -> Result<Program, Fault> {
        let (mut tokens, token_storage_bytes) = tokenize_for_parse(source, regexp_budget, capacity, compilation_deadline)?;
        // Index braces and parentheses once. Deferred bodies and arrow lookahead
        // reuse these links instead of rescanning nested token ranges.
        index_token_delimiters(&mut tokens);
        check_compilation_deadline(compilation_deadline)?;
        let mut owned_source = String::new();
        owned_source.try_reserve_exact(source.len()).map_err(|_| compilation_storage_limit(capacity))?;
        owned_source.push_str(source);
        let mut parser = Self {
            capacity,
            heap,
            program: Program {
                retained_bytes:std::cell::Cell::new(0),
                source: owned_source,
                strict,
                exprs: Vec::new(),
                stmts: Vec::new(),
                templates: Vec::new(),
                classes: Vec::new(),
                class_members: 0,
                patterns: Vec::new(),
                body: Vec::new(),
                chain_steps: 0,
                resumption_plan_nodes: 0,
            },
            tokens,
            token_storage_bytes,
            compilation_deadline,
            deferred_bodies: Vec::new(),
            current: 0,
            // Dynamic compilation runs on the caller's native stack. Account
            // for live evaluator frames instead of granting a fresh parse stack.
            depth: active_depth,
            expression_frames: 0,
            expression_storage_bytes: 0,
            argument_entries: 0,
            if_branches: 0,
            strict,
            loops: Vec::new(),
            breakables: Vec::new(),
            labels: Vec::new(),
            allow_in: true,
            functions: 0,
            await_count: None,
            yield_count: None,
            new_target_allowed,
            super_property_allowed,
            super_call_allowed,
        };
        let mut directives = true;
        while !matches!(parser.token().kind, TokenKind::End) {
            let stmt = parser.statement()?;
            parser.directive(stmt, &mut directives);
            parser.program.body.push(stmt);
        }
        parser.program.strict = parser.strict;
        parser.parse_deferred_bodies(active_depth)?;
        parser.check_compilation_progress()?;
        parser.check_finished_compilation_storage()?;
        validate_program(&parser.program)?;
        check_compilation_deadline(compilation_deadline)?;
        Ok(parser.program)
    }

    fn check_function_syntax_depth(&self) -> Result<(), Fault> {
        #[cfg(test)]
        if let Some(limit) = self.capacity.test_function_syntax_depth {
            if self.functions >= limit {
                return Err(self.unsupported(format!("Function syntax nesting limit reached ({limit}).")));
            }
        }
        Ok(())
    }

    fn record_template(&mut self, template: Template) -> Result<usize, Fault> {
        let id = self.program.templates.len();
        if id == self.program.templates.capacity() {
            let additional = self.ast_arena_growth(&self.program.templates)?;
            self.program.templates.try_reserve_exact(additional)
                .map_err(|_| compilation_storage_limit(self.capacity))?;
            self.check_compilation_progress()?;
        }
        self.program.templates.push(template);
        Ok(id)
    }

    fn defer_body(&mut self, mut template: Template) -> Result<usize, Fault> {
        self.check_function_syntax_depth()?;
        let start = self.current;
        self.need("{")?;
        let Some(end) = self.tokens[start].delimiter_end() else {
            return Err(self.error("Unterminated function body."));
        };
        template.source_end = self.tokens[end].end();
        let id = self.record_template(template)?;
        // These descriptors survive until their bodies are parsed, so charge
        // the queue's reservation alongside source, tokens and the AST.
        if self.deferred_bodies.len() == self.deferred_bodies.capacity() {
            let additional = self.ast_arena_growth(&self.deferred_bodies)?;
            self.deferred_bodies.try_reserve_exact(additional)
                .map_err(|_| compilation_storage_limit(self.capacity))?;
            self.check_compilation_progress()?;
        }
        self.deferred_bodies.push(DeferredBody {
            template: id,
            start,
            end,
            strict: self.strict,
            function_depth: self.functions + 1,
            new_target_allowed: !self.program.templates[id].arrow || self.new_target_allowed,
            super_property_allowed: self.super_property_allowed,
            super_call_allowed: self.super_call_allowed,
        });
        self.current = end + 1;
        Ok(id)
    }

    fn parse_deferred_bodies(&mut self, active_depth: usize) -> Result<(), Fault> {
        if self.expression_frames != 0 || self.if_branches != 0 {
            return Err(self.unsupported("Undrained parser continuations after program body."));
        }
        while let Some(body) = self.deferred_bodies.pop() {
            self.current = body.start;
            // The outer expression's parser stack has unwound, but eval's live
            // evaluator stack has not. Keep its original allowance and charge
            // one native function-body frame. The queued lexical depth is
            // independent of that native entry.
            self.depth = active_depth;
            self.enter()?;
            self.functions = body.function_depth;
            let kind = self.program.templates[body.template].kind;
            self.await_count = (kind == FunctionKind::Async).then_some(0);
            self.yield_count = (kind == FunctionKind::Generator).then_some(0);
            self.new_target_allowed = body.new_target_allowed;
            self.super_property_allowed = body.super_property_allowed;
            self.super_call_allowed = body.super_call_allowed;
            self.strict = body.strict;
            self.allow_in = true;
            self.loops.clear();
            self.breakables.clear();
            self.labels.clear();
            if self.expression_frames != 0 || self.if_branches != 0 {
                return Err(self.unsupported("Undrained parser continuations before deferred body."));
            }
            let (statements, strict, own_strict) = self.function_body()?;
            if self.current != body.end + 1 {
                return Err(self.error("Function body exceeded its token range."));
            }
            let template = &self.program.templates[body.template];
            if template.non_simple(&self.program) && own_strict {
                return Err(
                    self.error("A non-simple parameter list cannot have a use strict directive.")
                );
            }
            let names = template.parameter_names(&self.program);
            if (strict || template.non_simple(&self.program))
                && names.iter().collect::<HashSet<_>>().len() != names.len()
            {
                return Err(self.error("Duplicate parameters are forbidden in strict functions."));
            }
            for parameter in &names {
                self.strict_binding(parameter, strict)?;
            }
            if let Some(name) = &template.name {
                self.strict_binding(name, strict)?;
            }
            let template = &mut self.program.templates[body.template];
            template.strict = strict;
            template.body = statements;
            self.attach_resumption_plan(body.template, self.await_count.or(self.yield_count).unwrap_or(0))?;
            self.depth -= 1;
            if self.expression_frames != 0 || self.if_branches != 0 {
                return Err(self.unsupported("Undrained parser continuations after deferred body."));
            }
        }
        Ok(())
    }

    fn attach_resumption_plan(&mut self, template: usize, count: usize) -> Result<(), Fault> {
        if count != 0 {
            let available=self.capacity.compilation_bytes.saturating_sub(self.compilation_bytes());
            let plan = async_plan::compile(&self.program, template, count, self.capacity.ast_nodes,available,self.compilation_deadline)?;
            self.program.resumption_plan_nodes += plan.len();
            self.program.templates[template].resumption_plan = plan;
            self.check_compilation_progress()?;
        }
        Ok(())
    }

    fn token(&self) -> &Token {
        &self.tokens[self.current]
    }
    fn at(&self) -> usize {
        self.token().at()
    }
    fn error(&self, message: impl Into<String>) -> Fault {
        Fault::exception("SyntaxError", message).at(&self.program.source, self.at())
    }
    fn unsupported(&self, message: impl Into<String>) -> Fault {
        Fault::new(message).at(&self.program.source, self.at())
    }
    fn is(&self, text: &str) -> bool {
        match &self.token().kind {
            TokenKind::Word(word) => word == text,
            TokenKind::Punct(punct) => *punct == text,
            _ => false,
        }
    }
    fn eat(&mut self, text: &str) -> bool {
        if self.is(text) {
            self.current += 1;
            true
        } else {
            false
        }
    }
    fn need(&mut self, text: &str) -> Result<(), Fault> {
        if self.eat(text) {
            Ok(())
        } else {
            Err(self.error(format!("Expected {text:?}.")))
        }
    }
    fn name(&mut self) -> Result<String, Fault> {
        if let TokenKind::Word(word) | TokenKind::EscapedWord(word) = &self.token().kind {
            if !reserved(word) {
                let word = word.clone();
                self.current += 1;
                return Ok(word);
            }
        }
        Err(self.error("Expected an identifier."))
    }
    fn enter(&mut self) -> Result<(), Fault> {
        self.depth += 1;
        #[cfg(test)]
        if let Some(limit) = self.capacity.test_native_parser_depth {
            if self.depth > limit {
                return Err(self.unsupported(format!("Parser nesting limit reached ({limit}).")));
            }
        }
        #[cfg(windows)]
        {
            native_call_stack::check_windows_stack("parser entry")
                .map_err(|fault| fault.at(&self.program.source, self.at()))
        }
        #[cfg(not(windows))]
        {
            if self.depth > MAX_PARSE_DEPTH {
                return Err(self.unsupported(format!("Parser nesting limit reached on an unmeasured native stack ({MAX_PARSE_DEPTH}).")));
            }
            Ok(())
        }
    }
    fn stmt(&mut self, kind: StmtKind, at: usize) -> Result<StmtId, Fault> {
        self.check_nodes()?;
        let id = self.program.stmts.len();
        if id == self.program.stmts.capacity() {
            let slots = self.ast_arena_growth(&self.program.stmts)?;
            self.program.stmts.try_reserve_exact(slots).map_err(|_| compilation_storage_limit(self.capacity))?;
        }
        self.program.stmts.push(Stmt { kind, at });
        Ok(id)
    }
    fn expr(&mut self, kind: ExprKind, at: usize) -> Result<ExprId, Fault> {
        self.check_nodes()?;
        let id = self.program.exprs.len();
        if id == self.program.exprs.capacity() {
            let slots = self.ast_arena_growth(&self.program.exprs)?;
            self.program.exprs.try_reserve_exact(slots).map_err(|_| compilation_storage_limit(self.capacity))?;
        }
        self.program.exprs.push(Expr {
            kind,
            at,
            parenthesized: false,
        });
        Ok(id)
    }
    fn check_nodes(&self) -> Result<(), Fault> {
        let nodes = self.program.node_count();
        if nodes >= self.capacity.ast_nodes {
            return Err(self.unsupported(format!("AST node limit reached ({max_ast}).", max_ast=self.capacity.ast_nodes)));
        }
        if nodes % 4096 == 0 {
            self.check_compilation_progress()?;
        }
        Ok(())
    }

    fn compilation_bytes(&self) -> usize {
        self.token_storage_bytes.saturating_add(self.program.compilation_ast_bytes())
            .saturating_add(self.deferred_bodies.capacity().saturating_mul(size_of::<DeferredBody>()))
            .saturating_add(self.expression_storage_bytes)
    }

    fn check_compilation_progress(&self) -> Result<(), Fault> {
        check_compilation_deadline(self.compilation_deadline)?;
        if self.compilation_bytes() > self.capacity.compilation_bytes {
            return Err(compilation_storage_limit(self.capacity).at(&self.program.source, self.at()));
        }
        Ok(())
    }

    fn ast_arena_growth<T>(&self, arena: &Vec<T>) -> Result<usize, Fault> {
        let spare = self.capacity.compilation_bytes.saturating_sub(self.compilation_bytes());
        let slot_charge = size_of::<T>().saturating_add(16);
        let remaining = self.capacity.ast_nodes.saturating_sub(self.program.node_count()).max(1);
        let slots = arena.capacity().max(8).min(remaining).min(spare / slot_charge);
        if slots == 0 {
            return Err(compilation_storage_limit(self.capacity).at(&self.program.source, self.at()));
        }
        Ok(slots)
    }
    fn semi(&mut self) -> Result<(), Fault> {
        if self.eat(";")
            || self.is("}")
            || matches!(self.token().kind, TokenKind::End)
            || self.token().newline()
        {
            Ok(())
        } else {
            Err(self.error("Expected a semicolon or line ending."))
        }
    }
    fn block_body(&mut self) -> Result<Vec<StmtId>, Fault> {
        self.need("{")?;
        let mut body = Vec::new();
        while !self.eat("}") {
            if matches!(self.token().kind, TokenKind::End) {
                return Err(self.error("Unterminated block."));
            }
            body.push(self.statement()?);
        }
        Ok(body)
    }
    fn directive(&mut self, statement: StmtId, active: &mut bool) -> bool {
        if !*active {
            return false;
        }
        let stmt = &self.program.stmts[statement];
        if let StmtKind::Expr(expression) = stmt.kind {
            let expr = &self.program.exprs[expression];
            if matches!(&expr.kind, ExprKind::Literal(Value::String(_) | Value::StringUnits(_))) && expr.at == stmt.at {
                let source = &self.program.source[stmt.at..];
                if source.starts_with("\"use strict\"") || source.starts_with("'use strict'") {
                    self.strict = true;
                    return true;
                }
                return false;
            }
        }
        *active = false;
        false
    }
    fn function_body(&mut self) -> Result<(Vec<StmtId>, bool, bool), Fault> {
        self.need("{")?;
        let previous = self.strict;
        let mut body = Vec::new();
        let mut directives = true;
        let mut own_strict = false;
        while !self.eat("}") {
            if matches!(self.token().kind, TokenKind::End) {
                return Err(self.error("Unterminated function body."));
            }
            let statement = self.statement()?;
            own_strict |= self.directive(statement, &mut directives);
            body.push(statement);
        }
        let strict = self.strict;
        self.strict = previous;
        Ok((body, strict, own_strict))
    }

    fn yield_operand_omitted(&self) -> bool {
        self.token().newline() || matches!(self.token().kind,
            TokenKind::End | TokenKind::Punct(";" | "}" | ")" | "]" | "," | ":"))
    }

    fn generator_yield_statement(&mut self) -> Result<StmtKind, Fault> {
        let kind = if self.yield_operand_omitted() {
            StmtKind::Yield(None)
        } else if self.eat("*") {
            StmtKind::YieldFrom(self.assignment_with_in()?)
        } else {
            StmtKind::Yield(Some(self.assignment_with_in()?))
        };
        self.semi()?;
        *self.yield_count.as_mut().expect("generator statement context") += 1;
        Ok(kind)
    }
    fn strict_binding(&self, name: &str, strict: bool) -> Result<(), Fault> {
        if strict && matches!(name, "eval" | "arguments") {
            Err(self.error("eval and arguments cannot be bound or assigned in strict code."))
        } else {
            Ok(())
        }
    }
    fn declaration(&mut self, for_head: bool) -> Result<StmtKind, Fault> {
        let kind = if self.eat("let") {
            Declaration::Let
        } else if self.eat("const") {
            Declaration::Const
        } else {
            self.need("var")?;
            Declaration::Var
        };
        let mut declarations = Vec::new();
        loop {
            let pattern = self.binding_pattern(true)?;
            let initializer = if self.eat("=") {
                Some(self.assignment_expression()?)
            } else {
                None
            };
            if (kind == Declaration::Const
                || !matches!(
                    self.program.patterns[pattern].kind,
                    binding_patterns::Kind::Name(_)
                ))
                && initializer.is_none()
                && !(for_head && (self.is("in") || self.is("of")))
            {
                return Err(
                    self.error("A const or destructuring declaration needs an initializer.")
                );
            }
            declarations.push((pattern, initializer));
            if !self.eat(",") {
                break;
            }
        }
        Ok(StmtKind::Declare(kind, declarations))
    }
    fn statement(&mut self) -> Result<StmtId, Fault> {
        self.enter()?;
        let at = self.at();
        // Reserve the statement identity before parsing its body. Break and
        // continue targets are resolved once here, never by searching at runtime.
        let id = self.stmt(StmtKind::Empty, at)?;
        let previous_labels = self.labels.len();
        while self.label_ahead() {
            let name = self.name()?;
            self.need(":")?;
            if self.labels.iter().any(|(active, _, _)| *active == name) {
                return Err(self.error(format!("Duplicate active label {name:?}.")));
            }
            if self.labels.len() >= MAX_PARSE_DEPTH {
                return Err(self.unsupported("Label nesting limit reached (64)."));
            }
            self.labels.push((name, id, false));
        }
        let labelled = previous_labels != self.labels.len();
        let iteration = self.is("for") || self.is("while") || self.is("do");
        for (_, _, is_loop) in &mut self.labels[previous_labels..] {
            *is_loop = iteration;
        }
        if labelled && (self.is("let") || self.is("const") || self.is("function") || self.is("class") || self.async_function_ahead()) {
            return Err(self.error("Use a block for a labelled lexical or function declaration."));
        }
        let result = self.statement_inner(id);
        self.labels.truncate(previous_labels);
        self.depth -= 1;
        self.program.stmts[id].kind = result?;
        Ok(id)
    }

    fn label_ahead(&self) -> bool {
        matches!(&self.token().kind, TokenKind::Word(name) | TokenKind::EscapedWord(name) if !reserved(name))
            && matches!(
                self.tokens.get(self.current + 1).map(|token| &token.kind),
                Some(TokenKind::Punct(":"))
            )
    }

    fn jump_target(&mut self, continued: bool) -> Result<StmtId, Fault> {
        let name = if !self.token().newline() {
            match &self.token().kind {
                TokenKind::Word(word) | TokenKind::EscapedWord(word) if !reserved(word) => Some(self.name()?),
                _ => None,
            }
        } else {
            None
        };
        if let Some(name) = name {
            let Some((_, target, iteration)) = self
                .labels
                .iter()
                .rev()
                .find(|(label, _, _)| *label == name)
            else {
                return Err(self.error(format!("Unknown label {name:?}.")));
            };
            if continued && !iteration {
                return Err(self.error(format!(
                    "continue target {name:?} is not an iteration statement."
                )));
            }
            Ok(*target)
        } else {
            let targets = if continued {
                &self.loops
            } else {
                &self.breakables
            };
            targets.last().copied().ok_or_else(|| {
                self.error(if continued {
                    "continue is only valid inside a loop."
                } else {
                    "Unlabelled break is only valid inside a loop or switch."
                })
            })
        }
    }

    fn statement_inner(&mut self, id: StmtId) -> Result<StmtKind, Fault> {
        let at = self.at();
        let kind = if self.eat(";") {
            StmtKind::Empty
        } else if self.is("{") {
            StmtKind::Block(self.block_body()?)
        } else if self.is("let") || self.is("const") || self.is("var") {
            let kind = self.declaration(false)?;
            self.semi()?;
            kind
        } else if self.is("class") {
            let kind = self.class_expression(at, true)?;
            let ExprKind::Class(class) = kind else { unreachable!() };
            let name = self.program.classes[class].name.clone().expect("class declaration name");
            self.check_nodes()?;
            let pattern = self.program.patterns.len();
            self.program.patterns.push(binding_patterns::Pattern {
                kind: binding_patterns::Kind::Name(name), expressions: false, at,
            });
            let value = self.expr(ExprKind::Class(class), at)?;
            StmtKind::Declare(Declaration::Let, vec![(pattern, Some(value))])
        } else if self.is("function") || self.async_function_ahead() {
            let kind = self.function_head()?;
            let name = self.name()?;
            let function = self.function_with_super(None, at, kind, false, false)?;
            StmtKind::Function(name, function)
        } else if self.eat("if") {
            self.if_statement(id)?
        } else if self.eat("while") {
            self.need("(")?;
            let condition = self.expression()?;
            self.need(")")?;
            let body = self.loop_body(id)?;
            StmtKind::While(condition, body)
        } else if self.eat("do") {
            let body = self.loop_body(id)?;
            self.need("while")?;
            self.need("(")?;
            let condition = self.expression()?;
            self.need(")")?;
            // ASI explicitly permits this terminator even without a line break.
            self.eat(";");
            StmtKind::DoWhile(body, condition)
        } else if self.eat("for") {
            self.for_head(id, at)?
        } else if self.eat("switch") {
            self.switch_body(id)?
        } else if self.eat("throw") {
            if self.token().newline() {
                return Err(self.error("A line ending is not allowed after throw."));
            }
            let value = self.expression()?;
            self.semi()?;
            StmtKind::Throw(value)
        } else if self.eat("try") {
            self.try_body()?
        } else if self.yield_count.is_some() && self.eat("yield") {
            self.generator_yield_statement()?
        } else if self.eat("return") {
            if self.functions == 0 {
                return Err(self.error("return is only valid inside a function."));
            }
            let value = if self.token().newline()
                || self.is(";")
                || self.is("}")
                || matches!(self.token().kind, TokenKind::End)
            {
                None
            } else {
                Some(self.expression()?)
            };
            self.semi()?;
            StmtKind::Return(value)
        } else if self.eat("break") {
            let target = self.jump_target(false)?;
            self.semi()?;
            StmtKind::Break(target)
        } else if self.eat("continue") {
            let target = self.jump_target(true)?;
            self.semi()?;
            StmtKind::Continue(target)
        } else {
            let expression = self.expression()?;
            self.semi()?;
            StmtKind::Expr(expression)
        };
        Ok(kind)
    }

    fn if_statement(&mut self, id: StmtId) -> Result<StmtKind, Fault> {
        let previous = self.if_branches;
        let mut current = id;
        let mut first = StmtKind::Empty;
        loop {
            #[cfg(test)]
            if let Some(limit) = self.capacity.test_expression_frames {
                if self.if_branches >= limit {
                    return Err(self.unsupported(format!("Pending if-branch limit reached ({limit}).")));
                }
            }
            self.if_branches += 1;
            self.need("(")?;
            let condition = self.expression()?;
            self.need(")")?;
            let yes = self.statement()?;
            let has_else = self.eat("else");
            let at = self.at();
            let next = has_else && self.eat("if");
            let no = if next {
                Some(self.stmt(StmtKind::Empty, at)?)
            } else if has_else {
                Some(self.statement()?)
            } else {
                None
            };
            let kind = StmtKind::If(condition, yes, no);
            if current == id {
                first = kind;
            } else {
                self.program.stmts[current].kind = kind;
            }
            if next {
                // Only a literal else-if tail is flat. Then branches, blocks
                // and labelled else statements keep their ordinary depth and
                // label handling, including the nearest-if dangling else rule.
                current = no.expect("an else-if tail has a reserved statement");
            } else {
                self.if_branches = previous;
                return Ok(first);
            }
        }
    }

    fn parameters(&mut self, unique: bool) -> Result<(Vec<Parameter>, bool, bool), Fault> {
        self.need("(")?;
        let mut params = Vec::new();
        let mut rest = false;
        let mut trailing = false;
        if !self.is(")") {
            loop {
                if params.len() == self.capacity.arguments {
                    return Err(self.unsupported(format!("Function parameter limit reached ({}).", self.capacity.arguments)));
                }
                rest = self.eat("...");
                let pattern = self.binding_pattern(true)?;
                let initial = if !rest && self.eat("=") {
                    Some(self.assignment_with_in()?)
                } else {
                    None
                };
                params.push(Parameter { pattern, initial });
                if rest {
                    if !self.is(")") {
                        return Err(self.error(
                            "A rest parameter must be last, without a default or trailing comma.",
                        ));
                    }
                    break;
                }
                if !self.eat(",") {
                    break;
                }
                if self.is(")") {
                    trailing = true;
                    break;
                }
            }
        }
        self.need(")")?;
        let names = params
            .iter()
            .flat_map(|p| self.program.pattern_names(p.pattern))
            .collect::<Vec<_>>();
        if (unique
            || rest
            || params.iter().any(|p| {
                p.initial.is_some()
                    || !matches!(
                        self.program.patterns[p.pattern].kind,
                        binding_patterns::Kind::Name(_)
                    )
            }))
            && names.iter().collect::<HashSet<_>>().len() != names.len()
        {
            return Err(self.error("Duplicate parameters are forbidden in non-simple functions."));
        }
        Ok((params, rest, trailing))
    }

    fn async_function_ahead(&self) -> bool {
        self.is("async") && self.next_is("function") && !self.tokens[self.current + 1].newline()
    }

    fn function_head(&mut self) -> Result<FunctionKind, Fault> {
        let asynchronous = self.eat("async");
        self.need("function")?;
        if self.eat("*") {
            if asynchronous {
                return Err(self.unsupported("Async generator functions are not implemented."));
            }
            Ok(FunctionKind::Generator)
        } else {
            Ok(FunctionKind::from_async(asynchronous))
        }
    }

    fn function_with_super(&mut self, name: Option<String>, start: usize, kind: FunctionKind,
        super_property_allowed: bool, super_call_allowed: bool) -> Result<usize, Fault> {
        self.enter()?;
        let outer_await_count = self.await_count.take();
        let outer_yield_count = self.yield_count.take();
        let previous_new_target = std::mem::replace(&mut self.new_target_allowed, true);
        let previous_super_property = std::mem::replace(&mut self.super_property_allowed, super_property_allowed);
        let previous_super_call = std::mem::replace(&mut self.super_call_allowed, super_call_allowed);
        let (params, rest, trailing_parameter_comma) = self.parameters(false)?;
        let id = self.defer_body(Template {
            source_start: start,
            source_end: start,
            dynamic_source: false,
            arrow: false,
            kind,
            resumption_plan: Vec::new(),
            constructible: kind == FunctionKind::Ordinary,
            class_constructor: false,
            class_element: false,
            derived_constructor: false,
            default_constructor: false,
            strict: self.strict,
            name,
            params,
            rest,
            trailing_parameter_comma,
            body: Vec::new(),
        })?;
        self.await_count = outer_await_count;
        self.yield_count = outer_yield_count;
        self.new_target_allowed = previous_new_target;
        self.super_property_allowed = previous_super_property;
        self.super_call_allowed = previous_super_call;
        self.depth -= 1;
        Ok(id)
    }
    fn expression(&mut self) -> Result<ExprId, Fault> {
        let at = self.at();
        let mut value = self.assignment_expression()?;
        while self.eat(",") {
            let next = self.assignment_expression()?;
            value = self.expr(ExprKind::Sequence(value, next), at)?;
        }
        Ok(value)
    }
    fn expression_with_in(&mut self) -> Result<ExprId, Fault> {
        let previous = std::mem::replace(&mut self.allow_in, true);
        let result = self.expression();
        self.allow_in = previous;
        result
    }
    fn assignment_with_in(&mut self) -> Result<ExprId, Fault> {
        let previous = std::mem::replace(&mut self.allow_in, true);
        let result = self.assignment_expression();
        self.allow_in = previous;
        result
    }
    fn assignment_expression(&mut self) -> Result<ExprId, Fault> {
        self.enter()?;
        let result = self.continuation_assignment();
        self.depth -= 1;
        result
    }
    fn assignable(&self, expression: ExprId) -> bool {
        matches!(
            self.program.exprs[expression].kind,
            ExprKind::Name(_) | ExprKind::Member(_, _) | ExprKind::SuperMember(_)
        )
    }
    fn arrow_ahead(&self) -> bool {
        let tokens = &self.tokens;
        let mut at = self.current;
        let word = |token: &Token| matches!(&token.kind, TokenKind::Word(name) | TokenKind::EscapedWord(name) if !reserved(name));
        let punct = |token: Option<&Token>, text| matches!(token.map(|t| &t.kind), Some(TokenKind::Punct(value)) if *value == text);
        // In `async => ...`, async itself is the ordinary binding name.
        if self.is("async") && !self.next_is("=>") && !tokens[at + 1].newline() {
            at += 1;
        }
        if word(&tokens[at]) {
            return punct(tokens.get(at + 1), "=>");
        }
        if !punct(tokens.get(at), "(") {
            return false;
        }
        // Parenthesis matching was indexed once with the tokens. Looking through
        // every nested group here would make the new heap path quadratic.
        self.tokens[at].delimiter_end().is_some_and(|end| punct(tokens.get(end + 1), "=>"))
    }

    fn arrow(&mut self) -> Result<ExprId, Fault> {
        let at = self.at();
        self.enter()?;
        let asynchronous = self.is("async") && !self.next_is("=>");
        if asynchronous {
            self.current += 1;
        }
        let outer_await_count = self.await_count.take();
        let outer_yield_count = self.yield_count.take();
        let (params, rest, trailing_parameter_comma) = if self.is("(") {
            self.parameters(true)?
        } else {
            (
                vec![Parameter {
                    pattern: self.binding_pattern(true)?,
                    initial: None,
                }],
                false,
                false,
            )
        };
        if self.token().newline() {
            return Err(self.error("A line ending is not allowed before =>."));
        }
        self.need("=>")?;
        if self.is("{") {
            let template = self.defer_body(Template {
                source_start: at,
                source_end: at,
                dynamic_source: false,
                arrow: true,
                kind: FunctionKind::from_async(asynchronous),
                resumption_plan: Vec::new(),
                constructible: false,
                class_constructor: false,
                class_element: false,
                derived_constructor: false,
                default_constructor: false,
                strict: self.strict,
                name: None,
                params,
                rest,
                trailing_parameter_comma,
                body: Vec::new(),
            })?;
            self.await_count = outer_await_count;
            self.yield_count = outer_yield_count;
            self.depth -= 1;
            return self.expr(ExprKind::Function(template), at);
        }
        self.check_function_syntax_depth()?;
        let loops = std::mem::take(&mut self.loops);
        let breakables = std::mem::take(&mut self.breakables);
        let labels = std::mem::take(&mut self.labels);
        let previous_in = self.allow_in;
        self.functions += 1;
        self.await_count = asynchronous.then_some(0);
        let expression = self.assignment_expression()?;
        let body = vec![self.stmt(StmtKind::Return(Some(expression)), at)?];
        let strict = self.strict;
        for name in params
            .iter()
            .flat_map(|p| self.program.pattern_names(p.pattern))
        {
            self.strict_binding(&name, strict)?;
        }
        self.loops = loops;
        self.labels = labels;
        self.breakables = breakables;
        self.allow_in = previous_in;
        let count = self.await_count.unwrap_or(0);
        self.await_count = outer_await_count;
        self.yield_count = outer_yield_count;
        self.functions -= 1;
        self.depth -= 1;
        let template = self.record_template(Template {
            source_start: at,
            source_end: self.tokens[self.current - 1].end(),
            dynamic_source: false,
            arrow: true,
            kind: FunctionKind::from_async(asynchronous),
            resumption_plan: Vec::new(),
            constructible: false,
            class_constructor: false,
            class_element: false,
            derived_constructor: false,
            default_constructor: false,
            strict,
            name: None,
            params,
            rest,
            trailing_parameter_comma,
            body,
        })?;
        self.attach_resumption_plan(template, count)?;
        self.expr(ExprKind::Function(template), at)
    }

    fn arguments(&mut self) -> Result<Vec<Argument>, Fault> {
        self.need("(")?;
        let mut args = Vec::new();
        if !self.is(")") {
            loop {
                if args.len() == self.capacity.arguments {
                    return Err(self.unsupported(format!("Call argument limit reached ({}).", self.capacity.arguments)));
                }
                let spread = self.eat("...");
                let expression = self.assignment_with_in()?;
                self.append_argument(&mut args, Argument { expression, spread })?;
                if !self.eat(",") || self.is(")") {
                    break;
                }
            }
        }
        self.need(")")?;
        Ok(args)
    }
    fn primary(&mut self) -> Result<ExprId, Fault> {
        let at = self.at();
        if self.is("function") || self.async_function_ahead() {
            let kind = self.function_head()?;
            let name = if self.is("(") { None } else { Some(self.name()?) };
            let template = self.function_with_super(name, at, kind, false, false)?;
            return self.expr(ExprKind::Function(template), at);
        }
        if self.is("class") {
            let kind = self.class_expression(at, false)?;
            return self.expr(kind, at);
        }
        let kind = match self.token().kind.clone() {
            TokenKind::Number(value) => {
                self.current += 1;
                ExprKind::Literal(Value::Number(value))
            }
            TokenKind::String(value) => {
                self.current += 1;
                ExprKind::Literal(Value::String(value))
            }
            TokenKind::StringUnits(units) => {
                self.current += 1;
                ExprKind::Literal(Value::StringUnits(units))
            }
            TokenKind::Template(
                value,
                part @ (template_literals::Part::NoSubstitution | template_literals::Part::Head),
            ) => self.template_literal(value, matches!(part, template_literals::Part::Head))?,
            TokenKind::RegExp(expression) => {
                self.current += 1;
                ExprKind::RegExp(expression)
            }
            TokenKind::EscapedWord(name) => {
                if reserved(&name) {
                    return Err(self.error("A reserved word cannot be used as an identifier reference."));
                }
                self.current += 1;
                ExprKind::Name(name)
            }
            TokenKind::Word(word) => {
                self.current += 1;
                match word.as_str() {
                    "true" => ExprKind::Literal(Value::Bool(true)),
                    "false" => ExprKind::Literal(Value::Bool(false)),
                    "null" => ExprKind::Literal(Value::Null),
                    "this" => ExprKind::This,
                    _ if !reserved(&word) => ExprKind::Name(word),
                    _ => {
                        return Err(Fault::new(format!("Unsupported syntax {word:?}."))
                            .at(&self.program.source, at))
                    }
                }
            }
            TokenKind::Punct("{") => self.object_literal()?,
            TokenKind::Punct("[") => self.array_literal()?,
            _ => {
                return Err(self.error(
                    "Expected an expression. This JavaScript subset does not support this syntax.",
                ))
            }
        };
        self.expr(kind, at)
    }
    fn object_literal(&mut self) -> Result<ExprKind, Fault> {
        use object_literals::{Kind, Member};
        self.current += 1;
        let mut properties = Vec::new();
        let mut prototype_seen = false;
        while !self.is("}") {
            if properties.len() == self.heap.object_properties {
                return Err(self.unsupported(format!("Object property limit reached ({}).", self.heap.object_properties)));
            }
            if self.eat("...") {
                properties.push(Member::Spread(self.assignment_with_in()?));
                if !self.eat(",") {
                    break;
                }
                continue;
            }
            let position = self.at();
            // get/set are also ordinary property names. An accessor prefix
            // needs a following PropertyName, including a computed name.
            let next = self.tokens.get(self.current + 1);
            let next_name = matches!(
                next.map(|token| &token.kind),
                Some(
                    TokenKind::Word(_)
                        | TokenKind::EscapedWord(_)
                        | TokenKind::String(_)
                        | TokenKind::StringUnits(_)
                        | TokenKind::Number(_)
                        | TokenKind::Punct("[")
                )
            );
            let asynchronous = self.is("async")
                && next.is_some_and(|token| !token.newline())
                && (next_name || self.next_is("*"));
            let accessor = if asynchronous {
                self.current += 1;
                if self.is("*") {
                    return Err(self.unsupported("Async generator object methods are not implemented."));
                }
                None
            } else if next_name && (self.is("get") || self.is("set")) {
                let kind = if self.eat("get") {
                    Kind::Getter
                } else {
                    self.need("set")?;
                    Kind::Setter
                };
                Some(kind)
            } else {
                None
            };
            let (key, shorthand) = if self.eat("[") {
                let key = self.assignment_with_in()?;
                self.need("]")?;
                (key, None)
            } else if let TokenKind::StringUnits(units) = self.token().kind.clone() {
                if units.len().saturating_mul(2) > self.capacity.property_bytes {
                    return Err(self.unsupported("Property-name limit reached."));
                }
                self.current += 1;
                (self.expr(ExprKind::Literal(Value::StringUnits(units)), position)?, None)
            } else {
                let (name, shorthand) = match self.token().kind.clone() {
                    TokenKind::Word(name) | TokenKind::EscapedWord(name) => {
                        let shorthand = (!reserved(&name)).then(|| name.clone());
                        (name, shorthand)
                    }
                    TokenKind::String(name) => (name, None),
                    TokenKind::Number(number) => (number_text(number), None),
                    _ => return Err(self.error("Expected an object property name.")),
                };
                if name.len() > self.capacity.property_bytes {
                    return Err(self.unsupported("Property-name limit reached (256 bytes)."));
                }
                if name == "__proto__" {
                    // Only a noncomputed colon definition sets [[Prototype]].
                    // Methods/accessors/shorthand keep their existing refusal.
                    if asynchronous || accessor.is_some()
                        || !matches!(
                            self.tokens.get(self.current + 1).map(|token| &token.kind),
                            Some(TokenKind::Punct(":"))
                        )
                    {
                        return Err(
                            self.unsupported("Object prototype configuration is not implemented.")
                        );
                    }
                    if prototype_seen {
                        return Err(self.error("Duplicate __proto__ prototype setter."));
                    }
                    prototype_seen = true;
                    self.current += 1;
                    self.need(":")?;
                    properties.push(Member::Prototype(self.assignment_with_in()?));
                    if !self.eat(",") {
                        break;
                    }
                    continue;
                }
                self.current += 1;
                (
                    self.expr(ExprKind::Literal(Value::String(name)), position)?,
                    shorthand,
                )
            };
            let (value, kind) = if asynchronous || accessor.is_some() || self.is("(") {
                // A setter has one binding parameter. Its
                // PropertySetParameterList forbids even a trailing comma.
                if accessor == Some(Kind::Setter)
                    && matches!(
                        self.tokens.get(self.current + 2).map(|token| &token.kind),
                        Some(TokenKind::Punct(","))
                    )
                {
                    return Err(self.error("A setter parameter cannot be followed by a comma."));
                }
                let template = self.function_with_super(None, position, FunctionKind::from_async(asynchronous), true, false)?;
                let params = &self.program.templates[template].params;
                let kind = accessor.unwrap_or(Kind::Method);
                if kind == Kind::Getter && !params.is_empty() {
                    return Err(self.error("A getter must have no parameters."));
                }
                if kind == Kind::Setter
                    && (params.len() != 1
                        || self.program.templates[template].rest
                        || self.program.templates[template].trailing_parameter_comma)
                {
                    return Err(self.error("A setter must have exactly one parameter."));
                }
                let names = self.program.templates[template].parameter_names(&self.program);
                if names.iter().collect::<HashSet<_>>().len() != names.len() {
                    return Err(self.error("Duplicate method parameters are forbidden."));
                }
                self.program.templates[template].constructible = false;
                (self.expr(ExprKind::Function(template), position)?, kind)
            } else if self.eat(":") {
                (self.assignment_with_in()?, Kind::Data)
            } else if let Some(name) = shorthand {
                (self.expr(ExprKind::Name(name), position)?, Kind::Data)
            } else {
                return Err(self.error("Expected ':' after the object property name."));
            };
            properties.push(Member::Property { key, value, kind });
            if !self.eat(",") {
                break;
            }
        }
        self.need("}")?;
        Ok(ExprKind::Object(properties))
    }
    fn array_literal(&mut self) -> Result<ExprKind, Fault> {
        self.current += 1;
        let mut elements = Vec::new();
        while !self.is("]") {
            if elements.len() == self.heap.array_length {
                return Err(self.unsupported(format!("Array length limit reached ({}).", self.heap.array_length)));
            }
            if self.eat(",") {
                elements.push(None);
            } else {
                let spread = self.eat("...");
                elements.push(Some(Argument {
                    expression: self.assignment_with_in()?,
                    spread,
                }));
                if !self.eat(",") {
                    break;
                }
            }
        }
        self.need("]")?;
        Ok(ExprKind::Array(elements))
    }
    fn for_head(&mut self, id: StmtId, at: usize) -> Result<StmtKind, Fault> {
        let asynchronous = self.eat("await");
        if asynchronous && self.await_count.is_none() {
            return Err(self.error("for await requires an async function body."));
        }
        let kind = {
            self.need("(")?;
            let async_head = self.is("async");
            let previous_in = std::mem::replace(&mut self.allow_in, false);
            let initial = if self.is(";") {
                None
            } else if self.is("let") || self.is("const") || self.is("var") {
                let declaration = self.declaration(true)?;
                Some(self.stmt(declaration, at)?)
            } else {
                let expr = self.expression()?;
                Some(self.stmt(StmtKind::Expr(expr), at)?)
            };
            self.allow_in = previous_in;
            if self.is("in") || self.is("of") {
                let of = self.eat("of");
                if !of {
                    if asynchronous { return Err(self.error("for await requires an of clause.")); }
                    self.need("in")?;
                }
                let initial = initial.ok_or_else(|| {
                    self.error("for-in/of requires a binding or assignment target.")
                })?;
                match &self.program.stmts[initial].kind {
                    StmtKind::Declare(_, names) if names.len() == 1 && names[0].1.is_none() => {}
                    StmtKind::Declare(Declaration::Var, names)
                        if !of && names.len() == 1 && names[0].1.is_some() =>
                    {
                        return Err(self.unsupported(
                            "Legacy initialized for-in declarations are not implemented.",
                        ));
                    }
                    StmtKind::Expr(expression)
                        if self.assignable(*expression)
                            || matches!(
                                self.program.exprs[*expression].kind,
                                ExprKind::Pattern(_)
                            ) =>
                    {
                        if let ExprKind::Name(name) = &self.program.exprs[*expression].kind {
                            self.strict_binding(name, self.strict)?;
                            if of && !asynchronous && async_head && name == "async" {
                                return Err(self.error("An unparenthesized async identifier cannot be a for-of target."));
                            }
                        }
                    }
                    _ => return Err(self.error(
                        "for-in/of requires one uninitialized binding or an assignable reference.",
                    )),
                }
                let source = if of {
                    self.assignment_expression()?
                } else {
                    self.expression()?
                };
                self.need(")")?;
                let body = self.loop_body(id)?;
                if of {
                    if asynchronous { *self.await_count.as_mut().expect("async body") += 1; }
                    StmtKind::ForOf(initial, source, body, asynchronous)
                } else {
                    StmtKind::ForIn(initial, source, body)
                }
            } else {
                if asynchronous { return Err(self.error("for await requires an of clause.")); }
                self.need(";")?;
                let condition = if self.is(";") {
                    None
                } else {
                    Some(self.expression()?)
                };
                self.need(";")?;
                let advance = if self.is(")") {
                    None
                } else {
                    Some(self.expression()?)
                };
                self.need(")")?;
                let body = self.loop_body(id)?;
                StmtKind::For(initial, condition, advance, body)
            }
        };
        Ok(kind)
    }
    fn loop_body(&mut self, id: StmtId) -> Result<StmtId, Fault> {
        self.loops.push(id);
        self.breakables.push(id);
        let body = self.statement();
        self.breakables.pop();
        self.loops.pop();
        body
    }
    fn switch_body(&mut self, id: StmtId) -> Result<StmtKind, Fault> {
        self.need("(")?;
        let discriminant = self.expression()?;
        self.need(")")?;
        self.need("{")?;
        let mut cases = Vec::new();
        let mut body = Vec::new();
        let mut default = false;
        self.breakables.push(id);
        while !self.eat("}") {
            let test = if self.eat("case") {
                Some(self.expression()?)
            } else if self.eat("default") {
                if default {
                    return Err(self.error("A switch may have only one default clause."));
                }
                default = true;
                None
            } else {
                return Err(self.error("Expected case, default or the end of switch."));
            };
            self.need(":")?;
            cases.push((test, body.len()));
            while !self.is("case") && !self.is("default") && !self.is("}") {
                if matches!(self.token().kind, TokenKind::End) {
                    return Err(self.error("Unterminated switch."));
                }
                let statement = self.statement()?;
                if !self.strict
                    && matches!(self.program.stmts[statement].kind, StmtKind::Function(_, id) if self.program.templates[id].kind == FunctionKind::Ordinary)
                {
                    return Err(self.unsupported(
                        "Sloppy switch function declarations (Annex B) are not implemented.",
                    ));
                }
                body.push(statement);
            }
        }
        self.breakables.pop();
        Ok(StmtKind::Switch {
            discriminant,
            cases,
            body,
        })
    }
    fn try_body(&mut self) -> Result<StmtKind, Fault> {
        let kind = {
            if !self.is("{") {
                return Err(self.error("try requires a block."));
            }
            let body = self.statement()?;
            let catch = if self.eat("catch") {
                let binding = if self.eat("(") {
                    let pattern = self.binding_pattern(true)?;
                    self.need(")")?;
                    Some(pattern)
                } else {
                    None
                };
                if !self.is("{") {
                    return Err(self.error("catch requires a block."));
                }
                Some((binding, self.statement()?))
            } else {
                None
            };
            let finally = if self.eat("finally") {
                if !self.is("{") {
                    return Err(self.error("finally requires a block."));
                }
                Some(self.statement()?)
            } else {
                None
            };
            if catch.is_none() && finally.is_none() {
                return Err(self.error("try requires catch or finally."));
            }
            StmtKind::Try(body, catch, finally)
        };
        Ok(kind)
    }
}

fn collect_var_names(program: &Program, statements: &[StmtId]) -> Vec<String> {
    collect_var_names_with_work(program, statements, |_| Ok(()))
        .expect("uncharged parser scope walk")
}

fn collect_var_names_with_work(
    program: &Program,
    statements: &[StmtId],
    mut charge: impl FnMut(usize) -> Result<(), Fault>,
) -> Result<Vec<String>, Fault> {
    charge(std::mem::size_of_val(statements))?;
    let mut names = Vec::new();
    let mut pending = statements.to_vec();
    while let Some(statement) = pending.pop() {
        // Charge the statements visited and the names actually copied. A tiny
        // function inside a large script must not pay for unrelated source on
        // every call. Program/token/depth caps still bound each complete walk.
        charge(std::mem::size_of::<StmtId>())?;
        match &program.stmts[statement].kind {
            StmtKind::Declare(Declaration::Var, declarations) => {
                for (pattern, _) in declarations {
                    names.extend(program.pattern_names_with_work(*pattern, &mut charge)?);
                }
            }
            StmtKind::Block(body) | StmtKind::Switch { body, .. } => {
                charge(std::mem::size_of_val(body.as_slice()))?;
                pending.extend(body);
            }
            StmtKind::If(_, yes, no) => {
                pending.push(*yes);
                pending.extend(no);
            }
            StmtKind::Try(body, catch, finally) => {
                pending.push(*body);
                if let Some((_, body)) = catch {
                    pending.push(*body);
                }
                pending.extend(finally);
            }
            StmtKind::While(_, body) | StmtKind::DoWhile(body, _) => pending.push(*body),
            StmtKind::For(initial, _, _, body) => {
                pending.extend(initial);
                pending.push(*body);
            }
            StmtKind::ForIn(initial, _, body) | StmtKind::ForOf(initial, _, body, _) => {
                pending.push(*initial);
                pending.push(*body);
            }
            _ => {}
        }
    }
    Ok(names)
}

fn validate_program(program: &Program) -> Result<(), Fault> {
    fn scope(
        program: &Program,
        body: &[StmtId],
        params: &[String],
        function: bool,
    ) -> Result<(), Fault> {
        let mut lexical = HashSet::new();
        let mut functions = HashSet::new();
        for &statement in body {
            match &program.stmts[statement].kind {
                StmtKind::Declare(Declaration::Let | Declaration::Const, declarations) => {
                    for name in declarations
                        .iter()
                        .flat_map(|(p, _)| program.pattern_names(*p))
                    {
                        if !lexical.insert(name.clone())
                            || functions.contains(name.as_str())
                            || params.contains(&name)
                        {
                            return Err(Fault::exception(
                                "SyntaxError",
                                format!("Duplicate lexical declaration {name:?}."),
                            )
                            .at(&program.source, program.stmts[statement].at));
                        }
                    }
                }
                StmtKind::Function(name, _) => {
                    if lexical.contains(name.as_str())
                        || (!function && !functions.insert(name.clone()))
                    {
                        return Err(Fault::exception(
                            "SyntaxError",
                            format!("Conflicting function declaration {name:?}."),
                        )
                        .at(&program.source, program.stmts[statement].at));
                    }
                    functions.insert(name.clone());
                }
                _ => {}
            }
        }
        for name in collect_var_names(program, body) {
            if lexical.contains(name.as_str()) || (!function && functions.contains(name.as_str())) {
                return Err(Fault::exception(
                    "SyntaxError",
                    format!("var conflicts with lexical declaration {name:?}."),
                ));
            }
        }
        Ok(())
    }
    scope(program, &program.body, &[], true)?;
    for template in &program.templates {
        let names = template.parameter_names(program);
        scope(program, &template.body, &names, true)?;
    }
    for statement in &program.stmts {
        match &statement.kind {
            StmtKind::Function(name, template)
                if program.templates[*template].strict
                    && matches!(name.as_str(), "eval" | "arguments") =>
            {
                // A declaration's binding is not a named-function-expression
                // binding. Check its name after the deferred directive prologue
                // establishes strictness, without changing Template.name.
                return Err(Fault::exception(
                    "SyntaxError",
                    "eval and arguments cannot be bound or assigned in strict code.",
                )
                .at(&program.source, statement.at));
            }
            StmtKind::Block(body) | StmtKind::Switch { body, .. } => {
                scope(program, body, &[], false)?
            }
            StmtKind::If(_, yes, no) => {
                for &branch in std::iter::once(yes).chain(no.iter()) {
                    validate_body(program, branch)?;
                }
            }
            StmtKind::While(_, body)
            | StmtKind::DoWhile(body, _)
            | StmtKind::For(_, _, _, body)
            | StmtKind::ForIn(_, _, body)
            | StmtKind::ForOf(_, _, body, _) => validate_body(program, *body)?,
            StmtKind::Try(_, Some((Some(pattern), body)), _) => {
                let StmtKind::Block(body) = &program.stmts[*body].kind else {
                    unreachable!()
                };
                let names = program.pattern_names(*pattern);
                if names.iter().collect::<HashSet<_>>().len() != names.len() {
                    return Err(Fault::exception("SyntaxError", "Duplicate catch binding."));
                }
                scope(program, body, &names, false)?;
                if body.iter().any(|id| matches!(&program.stmts[*id].kind, StmtKind::Function(function, _) if names.contains(function)))
                    || (!matches!(program.patterns[*pattern].kind, binding_patterns::Kind::Name(_))
                        && collect_var_names(program, body).iter().any(|name| names.contains(name))) {
                    return Err(Fault::exception("SyntaxError", "Catch parameter conflicts with declaration.")
                        .at(&program.source, statement.at));
                }
            }
            _ => {}
        }
        if let StmtKind::For(Some(initial), _, _, body)
        | StmtKind::ForIn(initial, _, body)
        | StmtKind::ForOf(initial, _, body, _) = &statement.kind
        {
            if let StmtKind::Declare(Declaration::Let | Declaration::Const, declarations) =
                &program.stmts[*initial].kind
            {
                let variables: HashSet<_> =
                    collect_var_names(program, &[*body]).into_iter().collect();
                let mut names = HashSet::new();
                for name in declarations
                    .iter()
                    .flat_map(|(p, _)| program.pattern_names(*p))
                {
                    if !names.insert(name.clone()) || variables.contains(&name) {
                        return Err(Fault::exception(
                            "SyntaxError",
                            format!("Conflicting loop declaration {name:?}."),
                        )
                        .at(&program.source, statement.at));
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_body(program: &Program, body: StmtId) -> Result<(), Fault> {
    if matches!(
        program.stmts[body].kind,
        StmtKind::Declare(Declaration::Let | Declaration::Const, _) | StmtKind::Function(_, _)
    ) {
        Err(
            Fault::new("Use a block around a lexical or function declaration in a branch or loop.")
                .at(&program.source, program.stmts[body].at),
        )
    } else {
        Ok(())
    }
}

#[derive(Clone)]
struct Binding {
    order: usize,
    value: Option<Value>,
    kind: Declaration,
    // Valid only for this immutable primitive value. put_binding resets this
    // on every successful replacement, including mapped arguments/eval writes.
    utf16_length: Option<usize>,
    // One scalar boundary in this immutable value, never a payload/index table.
    // Replacement through put_binding invalidates both UTF-16 memos.
    utf16_cursor: Option<(usize, usize)>,
}

struct Environment {
    next_order: usize,
    // A closure or mapped arguments object can outlive the current activation.
    // Pin its ancestor chain before publishing the retaining handle.
    captured: bool,
    arguments: Option<arguments::Pending>,
    invocation: Option<derived_context::Data>,
    parent: Option<usize>,
    function_scope: bool,
    catch_parameter: Option<String>,
    bindings: BTreeMap<String, Binding>,
}

#[derive(Clone, Copy)]
enum Builtin {
    ThrowTypeError,
    Proxy,
    ReflectGet,
    ReflectSet,
    ReflectOwnKeys,
    Symbol(symbols::Op),
    Binary(binary_data::Op),
    Collection(collections::Op),
    Iterator(iterator_protocol::Op),
    Iterable(iterable_builtins::Op),
    TextEncoding(text_encoding::Op),
    Promise(promises::Op),
    Date(date::Op),
    Url(url_api::Op),
    Headers(headers_api::Op),
    Response(response_api::Op),
    Rect(rect_api::Op),
    ImageData(image_data_api::Op),
    DomException(dom_exception::Op),
    MutationObserver(mutation_observers::Op),
    TreeWalker(tree_walker::Op),
    Eval,
    JsonParse,
    JsonStringify,
    RegExp,
    Error(&'static str),
    EncodeUri(bool),
    DecodeUri(bool),
    Base64(bool),
    Object,
    Array,
    String,
    StringFromCharCode,
    Number,
    BigInt,
    BigIntMethod(bool),
    BigIntWidth(bool),
    Boolean,
    ParseInt,
    ParseFloat,
    IsFinite,
    IsNaN,
    Math(&'static str),
    Function,
    AsyncFunction,
    GeneratorFunction,
    Generator(generator_functions::Op),
    EmptyFunction,
    ObjectStatic(&'static str),
    ObjectMethod(&'static str),
    FunctionMethod(&'static str),
    ArrayMethod(&'static str),
    RegExpMethod(&'static str),
    StringMethod(&'static str),
    PrimitiveMethod(&'static str, bool),
    ErrorToString,
}

/// Prototype links are arena identities, never recursively owned values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ordinary {
    Object(usize),
    Function(usize),
}

impl Ordinary {
    fn value(self) -> Value {
        match self {
            Self::Object(id) => Value::Object(id),
            Self::Function(id) => Value::Function(id),
        }
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Object(id) => Some(Self::Object(*id)),
            Value::Function(id) => Some(Self::Function(*id)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum NativeGetter {
    ArgumentsCallee,
    SymbolDescription,
    Binary(binary_data::Field),
    CollectionSize(collections::Kind),
    TextEncoding(text_encoding::Field),
}

#[derive(Clone, Copy)]
struct Accessors {
    get: Option<usize>,
    set: Option<usize>,
}

#[derive(Clone)]
enum StoredValue {
    Value(Value),
    DeferredNative(usize),
    DeferredHostAttribute(usize),
}

impl StoredValue {
    fn bytes(&self) -> usize {
        match self { Self::Value(value) => value.bytes(), Self::DeferredNative(_) | Self::DeferredHostAttribute(_) => 0 }
    }
    fn as_value(&self) -> Result<&Value, Fault> {
        match self {
            Self::Value(value) => Ok(value),
            Self::DeferredNative(_) => Err(Fault::new("Deferred native data value requires materialization.")),
            Self::DeferredHostAttribute(_) => Err(Fault::new("Host accessor has no data value.")),
        }
    }
}

struct Property {
    value: StoredValue,
    /// Read-only native getters retain their built-in receiver checks.
    getter: Option<NativeGetter>,
    /// Script callbacks use the existing callable arena, not recursive ownership.
    accessor: Option<Accessors>,
    order: usize,
    enumerable: bool,
    writable: bool,
    configurable: bool,
}

struct Object {
    /// Creation provenance, independent of author-mutable prototype links.
    realm: Option<usize>,
    proxy: Option<proxy_objects::ProxyData>,
    date: Option<f64>,
    url: Option<url_api::Data>,
    headers: Option<headers_api::Data>,
    response: Option<Box<response_api::Data>>,
    rect: Option<rect_api::Data>,
    dom_exception: Option<dom_exception::Data>,
    primitive: Option<primitive_wrappers::Data>,
    promise: Option<promises::Data>,
    arguments: Option<arguments::Data>,
    binary: Option<binary_data::Data>,
    collection: Option<collections::Data>,
    iterator: Option<collections::IteratorData>,
    generator: Option<generator_functions::State>,
    text: Option<text_encoding::Data>,
    properties: StringProperties,
    symbol_properties: BTreeMap<usize, Property>,
    /// Arrays retain holes without allocating a Value for each missing index.
    length: Option<usize>,
    length_writable: bool,
    extensible: bool,
    immutable_prototype: bool,
    window_named_properties: bool,
    next_order: usize,
    error_name: Option<&'static str>,
    regexp: Option<Rc<regexp::Regex>>,
    prototype: Option<Ordinary>,
}

/// A real function environment exists even for an ordinary call whose
/// [[NewTarget]] is undefined. Arrows retain this lexical distinction so direct
/// eval in a top-level arrow cannot acquire function-only syntax permission.
#[derive(Clone, Copy)]
struct InvocationContext {
    environment: usize,
}

enum Function {
    Vacant,
    Native(Builtin),
    Promise(promises::Closure),
    Bound(bound_functions::Bound),
    Host {
        object: u32,
        method: &'static str,
        receiver_required: bool,
        constructible: bool,
        /// Engine-only eligibility for canonical interfaces whose actual
        /// construction is preflight-refused. Never inferred from JS properties.
        illegal_interface_constructor: bool,
    },
    Script {
        program: Rc<Program>,
        template: usize,
        environment: usize,
        lexical_this: Option<Value>,
        lexical_invocation: Option<InvocationContext>,
        home_object: Option<Ordinary>,
    },
}

fn classic_for_binding_bytes(bindings: &Vec<String>) -> usize {
    bindings.iter().fold(bindings.capacity().saturating_mul(std::mem::size_of::<String>()),
        |bytes, name| bytes.saturating_add(name.capacity()))
}

enum Control {
    Normal(Option<Value>),
    Return(Value),
    Break(StmtId, Option<Value>),
    Continue(StmtId, Option<Value>),
}

impl Control {
    fn update_empty(self, previous: Option<Value>) -> Self {
        match self {
            Self::Normal(value) => Self::Normal(value.or(previous)),
            Self::Break(target, value) => Self::Break(target, value.or(previous)),
            Self::Continue(target, value) => Self::Continue(target, value.or(previous)),
            returned @ Self::Return(_) => returned,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum PropertyKey {
    String(String),
    StringUnits(Vec<u16>),
    Symbol(usize),
}

impl PropertyKey {
    fn reference(self, receiver: Value) -> Reference {
        match self {
            Self::String(name) => Reference::Property(receiver, name),
            Self::StringUnits(units) => Reference::UnitsProperty(receiver, units),
            Self::Symbol(id) => Reference::SymbolProperty(receiver, id),
        }
    }
}

enum Reference {
    Binding(usize, String),
    Missing(String),
    Global(String),
    Property(Value, String),
    UnitsProperty(Value, Vec<u16>),
    SymbolProperty(Value, usize),
    // Keep the evaluated key value until GetValue/PutValue/Delete. In a plain
    // assignment the RHS runs first; compound operations convert independently
    // for their read and write, as required for observable user callbacks.
    ComputedProperty(Value, Value),
    SuperProperty { base: Option<Ordinary>, receiver: Value, name: Box<str> },
}

impl Reference {
    fn assignment_bytes(&self) -> usize {
        match self {
            Self::Binding(_, name) | Self::Global(name) | Self::Missing(name) => name.len(),
            Self::ComputedProperty(receiver, key) => receiver.bytes().saturating_add(key.bytes()),
            _ => unreachable!("suspended assignment needs an identifier or ordinary property"),
        }
    }
}

enum LoopIterator {
    Keys(Value, std::vec::IntoIter<StringKey>),
    Values(iterator_protocol::Record),
}

/// One runtime belongs to one document. The host starts one job for the complete
/// load batch and one for an event dispatch, so adding callbacks cannot multiply
/// the execution allowance. Eval requires an explicit host policy and shares
/// these budgets. The Function constructor uses the same host compilation gate.
pub struct Runtime {
    environments: Vec<Option<Environment>>,
    free_environments: Vec<usize>,
    functions: Vec<Function>,
    function_arena: function_arena::Arena,
    /// Ordinary own properties and [[Prototype]] of each callable identity.
    function_properties: Vec<usize>,
    function_realms: Vec<Option<usize>>,
    function_initial_names: Vec<Option<String>>,
    programs: Vec<Rc<Program>>,
    objects: Vec<Object>,
    symbols: Vec<symbols::Data>,
    symbol_registry: BTreeMap<String, usize>,
    symbol_prototype: Option<usize>,
    symbol_dispose: Option<usize>,
    symbol_to_primitive: Option<usize>,
    symbol_iterator: Option<usize>,
    symbol_async_iterator: Option<usize>,
    symbol_to_string_tag: Option<usize>,
    symbol_species: Option<usize>,
    promises: promises::Arena,
    mutations: mutation_observers::Arena,
    walkers: tree_walker::Arena,
    binary_roots: binary_data::Roots,
    retained_binary: usize,
    collection_roots: collections::Roots,
    collection_entries: usize,
    // Object identities are append-only. Zero skips weak-entry work during collection.
    weak_collection_count: usize,
    text_roots: text_encoding::Roots,
    object_prototype: Option<usize>,
    function_prototype: Option<usize>,
    async_function_roots: async_function_intrinsics::Roots,
    generator_roots: generator_functions::Roots,
    array_prototype: Option<usize>,
    active_array_joins: Vec<usize>,
    regexp_prototype: Option<usize>,
    regexp_constructor: Option<usize>,
    date_prototype: Option<usize>,
    url_roots: url_api::Roots,
    headers_roots: headers_api::Roots,
    response_roots: response_api::Roots,
    rect_roots: rect_api::Roots,
    image_data_prototype: Option<usize>,
    dom_exception_roots: dom_exception::Roots,
    string_prototype: Option<usize>,
    number_prototype: Option<usize>,
    big_int_prototype: Option<usize>,
    boolean_prototype: Option<usize>,
    error_prototypes: BTreeMap<&'static str, usize>,
    browser_global: Option<(u32, usize)>,
    host_objects: BTreeMap<u32, usize>,
    host_method_values: BTreeMap<(u32, &'static str), Value>,
    exported_function_roots: std::collections::BTreeSet<usize>,
    exported_object_roots: std::collections::BTreeSet<usize>,
    object_assigned_closures: std::collections::BTreeSet<usize>,
    scope_assigned_closures: std::collections::BTreeSet<usize>,
    transient_roots: transient_roots::State,
    json_scratch_bytes: usize,
    array_flat_workspace_bytes: usize,
    global_attributes: BTreeMap<String, (bool, bool, bool)>,
    deferred_global_orders: BTreeMap<&'static str, usize>,
    deferred_param_constructor: Option<usize>,
    deferred_datalist_constructor: Option<usize>,
    deferred_option_constructor: Option<usize>,
    deferred_optgroup_constructor: Option<usize>,
    host_globals: HashSet<String>,
    // Names explicitly exposed as ordinary properties by the embedding host.
    operation_globals: HashSet<&'static str>,
    arguments_thrower: Option<usize>,
    properties: usize,
    retained_source: usize,
    retained_ast: usize,
    retained_regexp: usize,
    program_allocation: program_allocation::State,
    regexp_work: usize,
    bindings: usize,
    value_bytes: usize,
    active_await_storage_bytes: usize,
    steps: usize,
    limits: limits::Limits,
    job_deadline: Instant,
    job_allowance: (usize, usize, usize),
    copy_bytes: usize,
    work_attribution: work_attribution::Tracker,
    eval_depth: usize,
    evaluation_frames: usize,
    evaluation_frame_slots: usize,
    evaluation_argument_slots: usize,
    evaluation_payload_bytes: usize,
    #[cfg(test)]
    recursive_evaluation_oracle: bool,
    #[cfg(test)]
    legacy_array_join_preflight: bool,
    call_depth: usize,
    invocation: Option<InvocationContext>,
    strict: bool,
    intrinsic_realms: Vec<intrinsic_realms::IntrinsicRealm>,
    global_environment: usize,
    active_realm: Option<usize>,
    parked_main_roots: Option<intrinsic_realms::ParkedRoots>,
    window_realms: BTreeMap<u32, Option<usize>>,
    native_constructor_prototypes:BTreeMap<(u32,&'static str),usize>,
    native_expando_owners:BTreeMap<u32,u32>,
    // Preserve the invoking Script realm through native call/apply/bind
    // forwarding. A real Script callee starts its own authority context.
    native_access_caller: Option<Option<u32>>,
    job_failed_fatally: bool,
    diagnostics: runtime_diagnostics::Data,
    failed_window_realms: HashSet<u32>,
    deferred_intrinsics: u8,
    failed_intrinsics: u8,
    array_to_string_intrinsic: Option<usize>,
    deferred_native_methods: Vec<deferred_native_methods::NativeMethod>,
    deferred_host_attributes: Vec<host_objects::HostAttribute>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    #[cold]
    #[inline(never)]
    fn argument_limit_fault(&self) -> Fault {
        Fault::new(format!("Call argument limit reached ({}).", self.limits.capacity.arguments))
    }

    #[cold]
    #[inline(never)]
    fn string_limit_fault(&self) -> Fault {
        Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024))
    }

    pub fn new() -> Self {
        Self::with_limits(limits::BROWSER)
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::with_limits(limits::REGRESSION)
    }

    #[cfg(test)]
    pub(crate) fn for_capacity_test() -> Self {
        Self::with_limits(limits::CAPACITY_REGRESSION)
    }

    /// Host configuration only. Dynamic scripts inherit this runtime's profile.
    pub fn with_limits(limits: limits::Limits) -> Self {
        Self::with_intrinsic_startup(limits, true)
    }

    fn with_intrinsic_startup(limits: limits::Limits, defer_families: bool) -> Self {
        let mut runtime = Self {
            environments: vec![Some(Environment {
                next_order: 0,
                captured: true,
                arguments: None,
                invocation: None,
                parent: None,
                function_scope: true,
                catch_parameter: None,
                bindings: BTreeMap::new(),
            })],
            free_environments: Vec::new(),
            functions: Vec::new(),
            function_arena: function_arena::Arena::default(),
            function_properties: Vec::new(),
            function_realms: Vec::new(),
            function_initial_names: Vec::new(),
            programs: Vec::new(),
            objects: Vec::new(),
            symbols: Vec::new(),
            symbol_registry: BTreeMap::new(),
            symbol_prototype: None,
            symbol_dispose: None,
            symbol_to_primitive: None,
            symbol_iterator: None,
            symbol_async_iterator: None,
            symbol_to_string_tag: None,
            symbol_species: None,
            promises: promises::Arena::default(),
            mutations: mutation_observers::Arena::default(),
            walkers: tree_walker::Arena::default(),
            binary_roots: binary_data::Roots::default(),
            retained_binary: 0,
            collection_roots: collections::Roots::default(),
            collection_entries: 0,
            weak_collection_count: 0,
            text_roots: text_encoding::Roots::default(),
            object_prototype: None,
            function_prototype: None,
            async_function_roots: async_function_intrinsics::Roots::default(),
            generator_roots: generator_functions::Roots::default(),
            array_prototype: None,
            active_array_joins: Vec::new(),
            regexp_prototype: None,
            regexp_constructor: None,
            date_prototype: None,
            url_roots: url_api::Roots::default(),
            headers_roots: headers_api::Roots::default(),
            response_roots: response_api::Roots::default(),
            rect_roots: rect_api::Roots::default(),
            image_data_prototype: None,
            dom_exception_roots: dom_exception::Roots::default(),
            string_prototype: None,
            number_prototype: None,
            big_int_prototype: None,
            boolean_prototype: None,
            error_prototypes: BTreeMap::new(),
            browser_global: None,
            host_objects: BTreeMap::new(),
            host_method_values: BTreeMap::new(),
            exported_function_roots: std::collections::BTreeSet::new(),
            exported_object_roots: std::collections::BTreeSet::new(),
            object_assigned_closures: std::collections::BTreeSet::new(),
            scope_assigned_closures: std::collections::BTreeSet::new(),
            transient_roots: transient_roots::State::default(),
            json_scratch_bytes: 0,
            array_flat_workspace_bytes: 0,
            global_attributes: BTreeMap::new(),
            deferred_global_orders: BTreeMap::new(),
            deferred_param_constructor: None,
            deferred_datalist_constructor: None,
            deferred_option_constructor: None,
            deferred_optgroup_constructor: None,
            host_globals: HashSet::new(),
            operation_globals: HashSet::new(),
            arguments_thrower: None,
            properties: 0,
            retained_source: 0,
            retained_ast: 0,
            retained_regexp: 0,
            program_allocation: program_allocation::State::default(),
            regexp_work: limits.regexp_work,
            bindings: 0,
            value_bytes: std::mem::size_of::<Option<Environment>>(),
            active_await_storage_bytes: 0,
            limits,
            steps: limits.steps,
            job_deadline: Instant::now() + Duration::from_secs(limits.job_seconds),
            job_allowance: (limits.steps, limits.copy_bytes, limits.regexp_work),
            copy_bytes: limits.copy_bytes,
            work_attribution: work_attribution::Tracker::new(limits.copy_bytes),
            eval_depth: 0,
            evaluation_frames: 0,
            evaluation_frame_slots: 0,
            evaluation_argument_slots: 0,
            evaluation_payload_bytes: 0,
            #[cfg(test)]
            recursive_evaluation_oracle: false,
            #[cfg(test)]
            legacy_array_join_preflight: false,
            call_depth: 0,
            invocation: None,
            strict: false,
            intrinsic_realms: Vec::new(),
            global_environment: 0,
            active_realm: None,
            parked_main_roots: None,
            window_realms: BTreeMap::new(),
            native_constructor_prototypes:BTreeMap::new(),
            native_expando_owners:BTreeMap::new(),
            native_access_caller: None,
            job_failed_fatally: false,
            diagnostics: runtime_diagnostics::Data::default(),
            failed_window_realms: HashSet::new(),
            deferred_intrinsics: intrinsic_realms::startup_deferred_families(defer_families),
            failed_intrinsics: 0,
            array_to_string_intrinsic: None,
            deferred_native_methods: Vec::new(),
            deferred_host_attributes: Vec::new(),
        };
        runtime.initialize_realm_intrinsics().expect("initial intrinsic capacity");
        runtime
    }

    fn initialize_realm_intrinsics(&mut self) -> Result<(), Fault> {
        self.initialize_prototype_roots()?;
        for (name, builtin) in [
            ("eval", Builtin::Eval),
            ("EvalError", Builtin::Error("EvalError")),
            ("RegExp", Builtin::RegExp),
            ("Proxy", Builtin::Proxy),
            ("SyntaxError", Builtin::Error("SyntaxError")),
            ("Error", Builtin::Error("Error")),
            ("TypeError", Builtin::Error("TypeError")),
            ("ReferenceError", Builtin::Error("ReferenceError")),
            ("RangeError", Builtin::Error("RangeError")),
            ("URIError", Builtin::Error("URIError")),
            ("encodeURIComponent", Builtin::EncodeUri(true)),
            ("decodeURIComponent", Builtin::DecodeUri(true)),
            ("encodeURI", Builtin::EncodeUri(false)),
            ("decodeURI", Builtin::DecodeUri(false)),
            ("atob", Builtin::Base64(false)),
            ("btoa", Builtin::Base64(true)),
            ("Object", Builtin::Object),
            ("Array", Builtin::Array),
            ("String", Builtin::String),
            ("Symbol", Builtin::Symbol(symbols::Op::Constructor)),
            ("Number", Builtin::Number),
            ("BigInt", Builtin::BigInt),
            ("Boolean", Builtin::Boolean),
            ("parseInt", Builtin::ParseInt),
            ("parseFloat", Builtin::ParseFloat),
            ("isFinite", Builtin::IsFinite),
            ("isNaN", Builtin::IsNaN),
            ("Function", Builtin::Function),
        ] {
            let value = self.native_function(builtin, name)?;
            if name == "RegExp" {
                if let Value::Function(id) = &value { self.regexp_constructor = Some(*id); }
            }
            if name == "Function" {
                if let Value::Function(id) = &value { self.async_function_roots.function_constructor = Some(*id); }
            }
            self
                .put_binding(self.global_environment, name.into(), Some(value), Declaration::Var, false)?;
        }
        self.initialize_intrinsic_prototypes()?;
        let Value::Function(to_string) = self.get_property(self.array_prototype.unwrap(), "toString")? else { unreachable!() };
        self.array_to_string_intrinsic = Some(to_string);
        self.initialize_symbols()?;
        self.initialize_big_int()?;
        self.initialize_reflect()?;
        self.initialize_number_statics()?;
        if self.deferred_intrinsics & 1 == 0 { self.initialize_binary()?; }
        else { self.reserve_deferred_global_orders(intrinsic_realms::BINARY_GLOBALS)?; }
        self.initialize_collections()?;
        self.initialize_iterable_builtins()?;
        self.initialize_promises()?;
        self.initialize_date()?;
        if self.deferred_intrinsics & 2 == 0 { self.initialize_url_api()?; }
        else { self.reserve_deferred_global_orders(intrinsic_realms::URL_GLOBALS)?; }
        if self.deferred_intrinsics & 4 == 0 { self.initialize_text_encoding()?; }
        else { self.reserve_deferred_global_orders(intrinsic_realms::TEXT_GLOBALS)?; }
        if self.deferred_intrinsics & 8 == 0 { self.initialize_rect_api()?; }
        else { self.reserve_deferred_global_orders(intrinsic_realms::RECT_GLOBALS)?; }
        if self.deferred_intrinsics & 16 == 0 { self.initialize_dom_exception()?; }
        else { self.reserve_deferred_global_orders(intrinsic_realms::DOM_EXCEPTION_GLOBALS)?; }
        if self.deferred_intrinsics & 32 == 0 { self.initialize_image_data()?; }
        else { self.reserve_deferred_global_orders(intrinsic_realms::IMAGE_DATA_GLOBALS)?; }
        self.reserve_deferred_global_orders(intrinsic_realms::HEADERS_GLOBALS)?;
        self.reserve_deferred_global_orders(intrinsic_realms::RESPONSE_GLOBALS)?;
        for (name, value) in [
            ("undefined", Value::Undefined),
            ("NaN", Value::Number(f64::NAN)),
            ("Infinity", Value::Number(f64::INFINITY)),
        ] {
            self
                .put_binding(self.global_environment, name.into(), Some(value), Declaration::Const, false)?;
        }
        let json = self.new_object(None)?;
        for (name, builtin) in [
            ("parse", Builtin::JsonParse),
            ("stringify", Builtin::JsonStringify),
        ] {
            let value = self.native_function(builtin, name)?;
            self.set_property(json, name.into(), value)?;
            self.objects[json]
                .properties
                .get_mut(name)
                .unwrap()
                .enumerable = false;
        }
        self
            .put_binding(
                self.global_environment,
                "JSON".into(),
                Some(Value::Object(json)),
                Declaration::Var,
                false,
            )?;
        let math = self.new_object(None)?;
        for (name, number) in [
            ("E", std::f64::consts::E),
            ("LN2", std::f64::consts::LN_2),
            ("LN10", std::f64::consts::LN_10),
            ("LOG2E", std::f64::consts::LOG2_E),
            ("LOG10E", std::f64::consts::LOG10_E),
            ("PI", std::f64::consts::PI),
            ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
            ("SQRT2", std::f64::consts::SQRT_2),
        ] {
            self
                .set_property(math, name.into(), Value::Number(number))?;
            let property = self.objects[math].properties.get_mut(name).unwrap();
            property.enumerable = false;
            property.writable = false;
            property.configurable = false;
        }
        for name in [
            "abs", "acos", "acosh", "asin", "asinh", "atan", "atanh", "atan2", "cbrt", "ceil",
            "clz32", "cos", "cosh", "exp", "expm1", "floor", "fround", "hypot", "imul", "log",
            "log1p", "log2", "log10", "max", "min", "pow", "random", "round", "sign", "sin",
            "sinh", "sqrt", "tan", "tanh", "trunc",
        ] {
            let value = self.native_function(Builtin::Math(name), name)?;
            self.set_property(math, name.into(), value)?;
            self.objects[math]
                .properties
                .get_mut(name)
                .unwrap()
                .enumerable = false;
        }
        self
            .put_binding(
                self.global_environment,
                "Math".into(),
                Some(Value::Object(math)),
                Declaration::Var,
                false,
            )?;
        self.global_attributes = self
            .environment(self.global_environment)
            .bindings
            .iter()
            .map(|(name, binding)| {
                let mutable = binding.kind == Declaration::Var;
                (name.clone(), (false, mutable, mutable))
            })
            .collect();
        Ok(())
    }

    /// Bind the browser's existing opaque Window handle to this runtime's
    /// ordinary global properties. Native Window properties remain host-owned.
    pub fn bind_browser_global(&mut self, handle: u32) -> Result<(), String> {
        if self.browser_global.is_some() {
            return Err("Browser global is already bound.".into());
        }
        let properties = self.new_object(None).map_err(Fault::display)?;
        self.browser_global = Some((handle, properties));
        self.window_realms.insert(handle, self.active_realm);
        Ok(())
    }

    /// Register one Rust-selected host capability as a real callable value.
    /// JavaScript cannot add capabilities through this API.
    pub fn define_host_function(
        &mut self,
        name: &str,
        object: u32,
        method: &'static str,
    ) -> Result<Value, String> {
        self.ensure_global_intrinsics(name).map_err(Fault::display)?;
        let allocation = self.allocate_host_function(object, method)?;
        let value = Value::Function(allocation.id);
        self.pin_exported_value(&value);
        if let Err(error) = self.define_global(name, value.clone()) {
            self.exported_function_roots.remove(&allocation.id);
            self.rollback_unpublished_function(allocation).map_err(Fault::display)?;
            return Err(error);
        }
        Ok(value)
    }

    /// Allocate a callable host capability without adding a global property.
    /// The host can expose the returned value on one specific native object.
    pub fn host_function(&mut self, object: u32, method: &'static str) -> Result<Value, String> {
        let allocation = self.allocate_host_function(object, method)?;
        let value = Value::Function(allocation.id);
        self.pin_exported_value(&value);
        Ok(value)
    }

    fn allocate_host_function(&mut self, object: u32, method: &'static str) -> Result<function_arena::Allocation, String> {
        self.allocate_host_function_with_length(object, method, usize::from(method == "compareDocumentPosition"))
    }

    fn allocate_host_function_with_length(&mut self, object: u32, method: &'static str, length: usize) -> Result<function_arena::Allocation, String> {
        if self.function_count_refused(1) {
            return Err(self.function_limit_fault("host").display());
        }
        let allocation = self.insert_function_slot(Function::Host {
            object,
            method,
            receiver_required: false,
            constructible: false,
            illegal_interface_constructor: false,
        }).map_err(Fault::display)?;
        self.initialize_function_properties(allocation, method, length, false)
            .map_err(Fault::display)?;
        Ok(allocation)
    }

    /// A native method value keeps its identity but must be called with its
    /// native receiver, unlike a global capability such as setTimeout.
    pub fn host_method(&mut self, object: u32, method: &'static str) -> Result<Value, String> {
        self.host_method_with_length(object, method, usize::from(method == "compareDocumentPosition"))
    }

    /// Allocate fixed native methods in their owner realm with their actual
    /// interface arity, before a foreign reader can materialize a wrapper.
    pub fn host_method_with_length(&mut self, object: u32, method: &'static str, length: usize) -> Result<Value, String> {
        let allocation = self.allocate_host_function_with_length(object, method, length)?;
        let function = Value::Function(allocation.id);
        self.pin_exported_value(&function);
        if let Some(Function::Host {
            receiver_required, ..
        }) = match function { Value::Function(id) => self.functions.get_mut(id), _ => None }
        {
            *receiver_required = true;
        }
        Ok(function)
    }

    pub fn begin_job(&mut self) {
        self.job_failed_fatally = false;
        self.work_attribution.begin_job(self.limits.copy_bytes);
        self.steps = self.limits.steps;
        self.job_deadline = Instant::now() + Duration::from_secs(self.limits.job_seconds);
        self.copy_bytes = self.limits.copy_bytes;
        self.regexp_work = self.limits.regexp_work;
        self.job_allowance = (self.limits.steps, self.limits.copy_bytes, self.limits.regexp_work);
    }

    pub fn constrain_job(&mut self, steps: usize, copies: usize, regexp: usize, deadline: Instant) {
        self.steps = self.steps.min(steps);
        let previous_copies = self.copy_bytes;
        self.copy_bytes = self.copy_bytes.min(copies);
        self.work_attribution.constrain(previous_copies, self.copy_bytes);
        self.regexp_work = self.regexp_work.min(regexp);
        self.job_deadline = self.job_deadline.min(deadline);
        self.job_allowance = (self.steps, self.copy_bytes, self.regexp_work);
    }

    pub(crate) fn execution_deadline(&self) -> Instant {
        self.job_deadline
    }

    pub fn job_usage(&self) -> (usize, usize, usize) {
        (
            self.job_allowance.0 - self.steps,
            self.job_allowance.1 - self.copy_bytes,
            self.job_allowance.2 - self.regexp_work,
        )
    }

    /// Latest failed job only. Formatting reads fixed numeric state; it never
    /// invokes page code, charges the job, or resets/consumes the snapshot.
    pub fn string_work_failure_report(&self) -> Option<String> {
        self.work_attribution.failure_report()
    }

    pub fn define_global(&mut self, name: &str, value: Value) -> Result<(), String> {
        if name.len() > 256
            || name.is_empty()
            || !name.chars().next().is_some_and(identifier_names::start)
            || !name.chars().all(identifier_names::part)
            || reserved(name)
        {
            return Err(Fault::new("Invalid global identifier.").display());
        }
        self.ensure_global_intrinsics(name).map_err(Fault::display)?;
        self.pin_exported_value(&value);
        self.put_binding(self.global_environment, name.into(), Some(value), Declaration::Var, true)
            .map_err(Fault::display)?;
        self.host_globals.insert(name.into());
        self.global_attributes.remove(name);
        Ok(())
    }

    pub fn execute(&mut self, source: &str, host: &mut dyn Host) -> Result<Value, String> {
        self.flush_native_microtask_cancellations(host);
        let previous_strict = self.strict;
        let previous_invocation = self.invocation.take();
        let result = (|| {
            let program = self.register_program(source, false)?;
            self.strict = program.strict;
            self.prepare(&program, &program.body, self.global_environment, true, host)?;
            let this = self.global_this()?;
            match self.statements(&program, &program.body, self.global_environment, &this, host)? {
                Control::Normal(value) => Ok(value.unwrap_or(Value::Undefined)),
                _ => Err(Fault::new(
                    "Unexpected control completion outside a function or loop.",
                )),
            }
        })();
        self.strict = previous_strict;
        self.invocation = previous_invocation;
        self.record_entry_result(false, &result);
        if result
            .as_ref()
            .is_err_and(|error| error.exception.is_none())
        {
            self.job_failed_fatally = true;
            self.cancel_microtasks();
            self.flush_native_microtask_cancellations(host);
        }
        if let Ok(value) = &result { self.pin_exported_value(value); }
        self.collect_environments();
        result.map_err(|error| self.display_fault(error))
    }

    fn is_eval(&self, value: &Value) -> bool {
        matches!(value, Value::Function(id) if matches!(self.functions.get(*id), Some(Function::Native(Builtin::Eval)))
            && self.function_realms.get(*id).copied().flatten() == self.active_realm)
    }

    fn invoke_plain_call(
        &mut self,
        function: Value,
        arguments: &[Value],
        direct: Option<(usize, &Value)>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if let Some(scope) = direct.filter(|_| self.is_eval(&function)) {
            self.string_eval(arguments.first().unwrap_or(&Value::Undefined), Some(scope), host)
        } else {
            self.call_value(function, arguments, Value::Undefined, host)
        }
    }

    fn global_this(&mut self) -> Result<Value, Fault> {
        if let Some((handle, _)) = self.browser_global {
            Ok(Value::Host(handle))
        } else if let Some(environment) = self.lookup(self.global_environment, "window")? {
            self.binding_value(environment, "window")
        } else {
            Ok(Value::Undefined)
        }
    }

    fn new_target_value(&mut self) -> Result<Value, Fault> {
        let context = self.invocation.ok_or_else(|| Fault::exception(
            "SyntaxError", "new.target requires an enclosing ordinary function."))?;
        Ok(self.invocation_data(context)?.new_target.map_or(Value::Undefined, Value::Function))
    }

    fn string_eval(
        &mut self,
        value: &Value,
        direct: Option<(usize, &Value)>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // Direct eval bypasses call_value. Source parsing invokes no script;
        // publish its incoming IDs and retain the new lexical scope below.
        let undefined = Value::Undefined;
        let (environment, this) = direct.unwrap_or((self.global_environment, &undefined));
        let marker = self.publish_statement_inputs(environment, this, Some(value))?;
        let result = self.string_eval_registered(value, direct, host);
        self.restore_root_marker(marker);
        result
    }

    fn string_eval_registered(
        &mut self,
        value: &Value,
        direct: Option<(usize, &Value)>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if !value.is_string() {
            return self.copy_value(value);
        }
        if !host.allow_eval_for_window(self.browser_global.map(|(handle, _)| handle)).map_err(Fault::new)? {
            return Err(Fault::exception(
                "EvalError",
                "Content Security Policy blocks string evaluation.",
            ));
        }
        let Value::String(source) = value else {
            return Err(Fault::new("Evaluating source containing unpaired UTF-16 surrogates is not implemented."));
        };
        self.check_native_call_stack("Function call")?;
        let previous_strict = self.strict;
        let previous_invocation = self.invocation;
        if direct.is_none() { self.invocation = None; }
        self.call_depth += 1;
        self.transient_roots.owned_call_depth += 1;
        let result = (|| {
            let (super_property_allowed, super_call_allowed) = if let Some(context) = self.invocation {
                let data = self.invocation_data(context)?;
                (data.home_object.is_some(), data.derived_constructor)
            } else { (false, false) };
            let program =
                self.register_program_invocation(source, false, direct.is_some() && self.strict,
                    direct.is_some() && self.invocation.is_some(),
                    direct.is_some() && super_property_allowed,
                    direct.is_some() && super_call_allowed)?;
            self.strict = program.strict;
            let outer = direct.map_or(self.global_environment, |(environment, _)| environment);
            let this = if let Some((_, this)) = direct {
                self.copy_value(this)?
            } else {
                self.global_this()?
            };
            // Eval lexical declarations never leak. Strict eval also owns its
            // var/function bindings; sloppy eval targets the caller's variable
            // environment (or the global environment for an indirect call).
            let lexical = self.new_environment(outer, program.strict)?;
            let result = match self.publish_statement_inputs(lexical, &this, None) {
                Err(error) => Err(error),
                Ok(marker) => {
                    let result = (|| {
                        let variable = if program.strict {
                            lexical
                        } else {
                            self.var_environment(outer)?
                        };
                        if !program.strict {
                            self.check_eval_declarations(&program, outer, variable)?;
                        }
                        self.prepare_scopes(&program, &program.body, lexical, variable, true, host)?;
                        match self.statements(&program, &program.body, lexical, &this, host)? {
                            Control::Normal(value) => Ok(value.unwrap_or(Value::Undefined)),
                            _ => Err(Fault::new("Unexpected control completion in eval.")),
                        }
                    })();
                    self.restore_root_marker(marker);
                    result
                }
            };
            self.release_environment(lexical);
            result
        })();
        self.call_depth -= 1;
        self.transient_roots.owned_call_depth -= 1;
        self.strict = previous_strict;
        self.invocation = previous_invocation;
        result
    }

    fn check_eval_declarations(
        &mut self,
        program: &Program,
        outer: usize,
        variable: usize,
    ) -> Result<(), Fault> {
        self.work_as(program.source.len(), WorkKind::ProgramCompilation)?;
        let mut names = collect_var_names(program, &program.body);
        for &statement in &program.body {
            if let StmtKind::Function(name, _) = &program.stmts[statement].kind {
                names.push(name.clone());
            }
        }
        for name in names {
            let mut scope = outer;
            loop {
                self.step()?;
                let environment = self.environment(scope);
                let conflict = if scope == variable {
                    environment
                        .bindings
                        .get(&name)
                        .is_some_and(|binding| binding.kind != Declaration::Var)
                } else {
                    environment.bindings.contains_key(&name)
                        && environment.catch_parameter.as_deref() != Some(&name)
                };
                if conflict {
                    return Err(Fault::exception("SyntaxError", format!("Eval var/function declaration conflicts with lexical binding {name:?}.")));
                }
                if scope == variable {
                    break;
                }
                scope = environment
                    .parent
                    .expect("eval variable environment is an ancestor");
            }
        }
        Ok(())
    }

    /// Compile an event-handler body without running it. Requiring the exact
    /// wrapper AST prevents an attribute from closing the wrapper and executing
    /// statements while the host is only registering callbacks.
    pub fn compile_handler(&mut self, source: &str) -> Result<Value, String> {
        let result = (|| {
            if source.len() > self.limits.capacity.source_bytes {
                return Err(Fault::new(format!("Script source limit reached ({} KiB).", self.limits.capacity.source_bytes / 1024)));
            }
            let wrapped = format!("(function(event) {{\n{source}\n}})");
            let program = self.register_program(&wrapped, true)?;
            let StmtKind::Expr(expression) = program.stmts[program.body[0]].kind else {
                unreachable!()
            };
            let ExprKind::Function(template) = program.exprs[expression].kind else {
                unreachable!()
            };
            self.make_function(&program, template, self.global_environment, &Value::Undefined)
        })();
        if result
            .as_ref()
            .is_err_and(|error| error.exception.is_none())
        {
            self.job_failed_fatally = true;
            self.cancel_microtasks();
        }
        if let Ok(value) = &result { self.pin_exported_value(value); }
        self.collect_environments();
        result.map_err(|error| self.display_fault(error))
    }

    pub fn invoke(
        &mut self,
        function: Value,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, String> {
        self.flush_native_microtask_cancellations(host);
        self.pin_exported_value(&function);
        self.pin_exported_value(&this);
        for value in args { self.pin_exported_value(value); }
        let result = self.call_value(function, args, this, host);
        self.finish_invocation(result, host)
    }

    pub(crate) fn invoke_native_dispatch(&mut self, token: usize, host: &mut dyn Host) -> Result<(), String> {
        self.flush_native_microtask_cancellations(host);
        let result = self.dispatch_native_event(token, host).map(Value::Bool);
        self.finish_invocation(result, host).map(|_| ())
    }

    fn finish_invocation(&mut self, result: Result<Value, Fault>, host: &mut dyn Host) -> Result<Value, String> {
        self.record_entry_result(true, &result);
        if result.as_ref().is_err_and(|error| error.exception.is_none()) {
            self.job_failed_fatally = true;
            self.cancel_microtasks();
        }
        self.flush_native_microtask_cancellations(host);
        if let Ok(value) = &result { self.pin_exported_value(value); }
        self.collect_environments();
        result.map_err(|error| self.display_fault(error))
    }

    /// Typed fatal state until the host begins a new job. Ordinary callback
    /// exceptions do not set it, and error formatting cannot change it.
    pub(crate) fn invocation_failed_fatally(&self) -> bool { self.job_failed_fatally }

    fn register_program(&mut self, source: &str, handler: bool) -> Result<Rc<Program>, Fault> {
        self.register_program_context(source, handler, false)
    }
    fn register_program_context(
        &mut self,
        source: &str,
        handler: bool,
        strict: bool,
    ) -> Result<Rc<Program>, Fault> {
        self.register_program_invocation(source, handler, strict, false, false, false)
    }

    fn register_program_invocation(
        &mut self, source: &str, handler: bool, strict: bool, new_target_allowed: bool,
        super_property_allowed: bool, super_call_allowed: bool,
    ) -> Result<Rc<Program>, Fault> {
        self.step()?;
        self.prepare_program_capacity(source.len(), 0, true)?;
        if self.program_count_refused()
            || self.retained_source.saturating_add(source.len()) > self.limits.capacity.retained_source
        {
            return Err(self.program_limit_fault(source, None, if handler { "handler" } else { "ordinary" }, Some(strict), false));
        }
        self.work_as(source.len(), WorkKind::ProgramCompilation)?;
        let program = Rc::new(Parser::parse_invocation_context(source, &mut self.regexp_work,
            strict, self.eval_depth, self.limits.capacity, self.limits.heap, Some(self.job_deadline), new_target_allowed,
            super_property_allowed, super_call_allowed).map_err(|mut fault| {
            if self.eval_depth != 0 && (fault.message.contains("Parser nesting limit")
                || fault.message.contains("Native stack headroom exhausted during parser entry")) {
                fault.message.push_str(&format!(" Active native entries={}; calls={}; heap expression frames={}.",
                    self.eval_depth, self.call_depth, self.evaluation_frames));
                fault.message.push_str(&format!(" {}.", self.inhibition_diagnostic()));
            }
            fault
        })?);
        if handler {
            let valid = match program.body.as_slice() {
                [statement] => match program.stmts[*statement].kind {
                    StmtKind::Expr(expression) => match program.exprs[expression].kind {
                        ExprKind::Function(template) => {
                            let function = &program.templates[template];
                            !function.arrow
                                && !function.non_simple(&program)
                                && function.name.is_none()
                                && function.params.len() == 1
                                && program.pattern_name(function.params[0].pattern) == Some("event")
                        }
                        _ => false,
                    },
                    _ => false,
                },
                _ => false,
            };
            if !valid {
                return Err(Fault::new("Invalid inline event-handler body.").at(source, 0));
            }
        }
        self.retain_parsed_program(program)
    }

    fn retain_parsed_program(&mut self,program:Rc<Program>) -> Result<Rc<Program>, Fault> {
        self.prepare_program_capacity(program.source.len(), program.node_count(), true)?;
        if self.program_count_refused()
            || self.retained_source.saturating_add(program.source.len()) > self.limits.capacity.retained_source
        {
            return Err(self.program_limit_fault(&program.source, Some(program.node_count()), "parsed-retention", None, false));
        }
        let regexp_bytes = program
            .exprs
            .iter()
            .map(|expression| match &expression.kind {
                ExprKind::RegExp(regex) => regex.retained_bytes,
                _ => 0,
            })
            .sum::<usize>();
        let nodes = program.node_count();
        if self.ast_count_refused(nodes) {
            return Err(self.program_limit_fault(&program.source, Some(nodes), "parsed-retention", None, true));
        }
        // Registry capacity is owned retained metadata. Reserve and admit
        // movement before mutating regexp/source/AST retention or publishing
        // the parsed program; failure drops only the unpublished reservation.
        let payload=program.retained_payload_bytes(||self.step())?;
        let registry=if self.programs.len()==self.programs.capacity(){std::mem::size_of::<Rc<Program>>()}else{0};
        self.prepare_program_storage(program.source.len(),nodes,false,payload.saturating_add(registry))?;
        let available=self.limits.heap.retained_values.saturating_sub(self.value_bytes).checked_sub(payload).ok_or_else(||Fault::new("Retained value program storage exhausted."))?;
        let growth=limits::prepare_retained_growth(&self.programs,1,available)?;
        if growth.added!=0{self.work(self.programs.len()*std::mem::size_of::<Rc<Program>>())?;}
        self.retain_regexp(regexp_bytes)?;
        program.retained_bytes.set(payload);
        let added=growth.added;growth.commit(&mut self.programs);self.value_bytes+=added+payload;
        self.retained_source += program.source.len();
        self.retained_ast += nodes;
        self.programs.push(program.clone());
        Ok(program)
    }

    fn environment(&self, id: usize) -> &Environment {
        self.environments[id]
            .as_ref()
            .expect("live script environment")
    }
    fn environment_mut(&mut self, id: usize) -> &mut Environment {
        self.environments[id]
            .as_mut()
            .expect("live script environment")
    }
    fn step(&mut self) -> Result<(), Fault> {
        if self.steps == 0 {
            return Err(Fault::new(format!("Execution step limit reached ({} per job).", self.limits.steps)));
        }
        if self.steps & 255 == 0 && Instant::now() >= self.job_deadline {
            self.steps = 0;
            return Err(Fault::new(format!(
                "Execution time limit reached ({} second{} per job).",
                self.limits.job_seconds, if self.limits.job_seconds == 1 { "" } else { "s" },
            )));
        }
        self.steps -= 1;
        Ok(())
    }
    fn work(&mut self, bytes: usize) -> Result<(), Fault> {
        self.work_as(bytes, WorkKind::Other)
    }

    fn work_as(&mut self, bytes: usize, kind: WorkKind) -> Result<(), Fault> {
        if bytes > self.copy_bytes {
            let remaining = self.copy_bytes;
            self.copy_bytes = 0;
            self.work_attribution.reject(kind, bytes, remaining, self.job_allowance.1);
            return Err(Fault::new(format!(
                "String/copy work limit reached ({} MiB per job).",
                self.limits.copy_bytes / (1024 * 1024),
            )));
        }
        self.copy_bytes -= bytes;
        self.work_attribution.accept(kind, bytes);
        Ok(())
    }
    fn checked_value(&mut self, value: Value) -> Result<Value, Fault> {
        if value.bytes() > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        self.work_as(value.bytes(), WorkKind::ConversionOutput)?;
        Ok(value)
    }
    fn copy_value(&mut self, value: &Value) -> Result<Value, Fault> {
        self.work_as(value.bytes(), WorkKind::ValueCopy)?;
        Ok(value.clone())
    }
    fn new_environment(&mut self, parent: usize, function_scope: bool) -> Result<usize, Fault> {
        self.step()?;
        let env = Some(Environment {
            next_order: 0,
            captured: false,
            arguments: None,
            invocation: None,
            parent: Some(parent),
            function_scope,
            catch_parameter: None,
            bindings: BTreeMap::new(),
        });
        if let Some(id) = self.free_environments.pop() {
            self.environments[id] = env;
            Ok(id)
        } else {
            if self.environment_count_refused() {
                return Err(self.environment_limit_fault("Environment", self.limits.capacity.environments));
            }
            self.prepare_environment_slot_storage()?;
            let id = self.environments.len();
            self.environments.push(env);
            Ok(id)
        }
    }
    fn put_binding(
        &mut self,
        env: usize,
        name: String,
        value: Option<Value>,
        kind: Declaration,
        replace: bool,
    ) -> Result<(), Fault> {
        self.put_binding_ordered(env, name, value, kind, replace, None)
    }

    fn put_binding_ordered(&mut self, env: usize, name: String, value: Option<Value>,
        kind: Declaration, replace: bool, preserved_order: Option<usize>) -> Result<(), Fault> {
        if name.len() > self.limits.capacity.property_bytes {
            return Err(Fault::new(format!("Identifier length limit reached ({} bytes).", self.limits.capacity.property_bytes)));
        }
        let old = self.environment(env).bindings.get(&name);
        if old.is_some() && !replace {
            return Err(Fault::new(format!("Duplicate binding {name:?}.")));
        }
        let old_bytes = old
            .and_then(|binding| binding.value.as_ref())
            .map_or(0, Value::bytes);
        let new_bytes = value.as_ref().map_or(0, Value::bytes);
        if new_bytes > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        if old.is_none() && self.bindings == self.limits.capacity.bindings {
            return Err(self.environment_limit_fault("Binding", self.limits.capacity.bindings));
        }
        let pending_bytes = if name == "arguments" {
            self.environment(env)
                .arguments
                .as_ref()
                .map_or(0, arguments::Pending::bytes)
        } else {
            0
        };
        if self.value_bytes - old_bytes - pending_bytes + new_bytes > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        let new = old.is_none();
        let inherited_order = old.map(|binding| binding.order).or(preserved_order).or_else(|| {
            (env == self.global_environment && kind == Declaration::Var)
                .then_some(self.browser_global).flatten()
                .and_then(|(_, id)| self.objects[id].properties.get(&name).map(|property| property.order))
        }).or_else(|| {
            (env == self.global_environment && kind == Declaration::Var)
                .then(|| self.deferred_global_orders.get(name.as_str()).copied()).flatten()
        });
        let order = inherited_order.unwrap_or(self.environment(env).next_order);
        let next_order = self.environment(env).next_order.checked_add(usize::from(inherited_order.is_none()))
            .ok_or_else(|| Fault::new("Binding creation order limit reached."))?;
        self.work_as(new_bytes.saturating_add(name.len()), WorkKind::BindingStore)?;
        if env == self.global_environment && kind == Declaration::Var {
            self.deferred_global_orders.remove(name.as_str());
        }
        if name == "arguments" {
            if let Some(pending) = self.environment_mut(env).arguments.take() {
                self.value_bytes -= pending.bytes();
            }
        }
        self.environment_mut(env)
            .bindings
            .insert(name, Binding { order, value, kind, utf16_length: None, utf16_cursor: None });
        self.environment_mut(env).next_order = next_order;
        self.bindings += usize::from(new);
        self.value_bytes = self.value_bytes - old_bytes + new_bytes;
        Ok(())
    }
    fn lookup(&mut self, mut env: usize, name: &str) -> Result<Option<usize>, Fault> {
        loop {
            self.step()?;
            self.work_as(name.len(), WorkKind::LookupName)?;
            if env == self.global_environment { self.ensure_global_intrinsics(name)?; }
            let current = self.environment(env);
            if current.bindings.contains_key(name) {
                return Ok(Some(env));
            }
            let Some(parent) = current.parent else {
                return Ok(None);
            };
            env = parent;
        }
    }
    fn global_binding(&self, name: &str) -> bool {
        self.environment(self.global_environment)
            .bindings
            .get(name)
            .is_some_and(|binding| {
                binding.kind == Declaration::Var || matches!(name, "undefined" | "NaN" | "Infinity")
            })
    }

    fn is_browser_global(&self, handle: u32) -> bool {
        self.browser_global
            .is_some_and(|(global, _)| handle == global)
    }

    fn host_owns_global(&self, name: &str, host: &dyn Host) -> bool {
        host.owns_window_property(self.browser_global.map(|(handle, _)| handle), name)
    }

    fn global_exists(&mut self, name: &str, host: &dyn Host) -> Result<bool, Fault> {
        self.step()?;
        self.ensure_global_intrinsics(name)?;
        let Some((_, properties)) = self.browser_global else {
            return Ok(false);
        };
        if self.host_owns_global(name, host)
            || self.global_binding(name)
            || self.object(properties)?.properties.contains_key(name)
        {
            return Ok(true);
        }
        if self.is_operation_global(name) {
            return Ok(self.property_owner(Ordinary::Object(properties), name)?.is_some());
        }
        Ok(false)
    }

    fn global_value(&mut self, name: &str, host: &mut dyn Host) -> Result<Value, Fault> {
        self.ensure_global_intrinsics(name)?;
        let (handle, properties) = self.browser_global.expect("bound browser global");
        self.property_name(name)?;
        if self.host_owns_global(name, host) {
            let value = self.host_get_checked(handle, name, host)?;
            self.checked_value(value)
        } else if self.global_binding(name) {
            self.binding_value(self.global_environment, name)
        } else {
            self.get_ordinary_property_with_receiver(
                Ordinary::Object(properties),
                name,
                &Value::Host(handle),
                host,
            )
        }
    }

    fn set_global(&mut self, name: String, value: Value, host: &mut dyn Host) -> Result<(), Fault> {
        self.ensure_global_intrinsics(&name)?;
        let (handle, properties) = self.browser_global.expect("bound browser global");
        self.property_name(&name)?;
        if self.host_owns_global(&name, host) {
            // Native globals such as Window.location need the same checked
            // conversions and validation as member assignments on other hosts.
            self.host_write(handle, name, value, host)
        } else if self.global_binding(&name) {
            self.store_binding(self.global_environment, &name, value, false)
        } else {
            self.assign_ordinary_property_with_receiver(
                Ordinary::Object(properties),
                name,
                value,
                Value::Host(handle),
                host,
            )
        }
    }

    fn read_binding(
        &mut self,
        env: usize,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if env == self.global_environment
            && self.browser_global.is_some()
            && self.global_binding(name)
            && self.host_owns_global(name, host)
        {
            self.global_value(name, host)
        } else {
            self.binding_value(env, name)
        }
    }

    fn write_binding(
        &mut self,
        env: usize,
        name: &str,
        value: Value,
        initialize: bool,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if env == self.global_environment
            && self.is_operation_global(name)
            && !self.environment(self.global_environment).bindings.contains_key(name)
        {
            self.set_global(name.into(), value, host)
        } else if env == self.global_environment
            && self.browser_global.is_some()
            && self.global_binding(name)
            && self.host_owns_global(name, host)
        {
            self.set_global(name.into(), value, host)
        } else {
            self.store_binding(env, name, value, initialize)
        }
    }

    fn var_environment(&mut self, mut env: usize) -> Result<usize, Fault> {
        while !self.environment(env).function_scope {
            self.step()?;
            env = self.environment(env).parent.unwrap();
        }
        Ok(env)
    }
    fn binding_value(&mut self, env: usize, name: &str) -> Result<Value, Fault> {
        if env == self.global_environment { self.ensure_global_intrinsics(name)?; }
        if name == "arguments" && self.environment(env).arguments.is_some() {
            self.materialize_arguments(env)?;
        }
        let binding =
            self.environment(env).bindings.get(name).ok_or_else(|| {
                Fault::exception("ReferenceError", format!("{name} is not defined."))
            })?;
        let value = binding.value.as_ref().ok_or_else(|| {
            Fault::exception(
                "ReferenceError",
                format!("Cannot access {name} before its initialization."),
            )
        })?;
        let bytes = value.bytes();
        self.work_as(bytes, WorkKind::BindingCopy)?;
        Ok(self.environment(env).bindings[name]
            .value
            .as_ref()
            .unwrap()
            .clone())
    }
    fn store_binding(
        &mut self,
        env: usize,
        name: &str,
        value: Value,
        initialize: bool,
    ) -> Result<(), Fault> {
        let binding =
            self.environment(env).bindings.get(name).ok_or_else(|| {
                Fault::exception("ReferenceError", format!("{name} is not defined."))
            })?;
        if !initialize && binding.value.is_none() {
            return Err(Fault::exception(
                "ReferenceError",
                format!("Cannot access {name} before its initialization."),
            ));
        }
        if !initialize
            && env == self.global_environment
            && self
                .global_attributes
                .get(name)
                .is_some_and(|attrs| !attrs.1)
        {
            return if self.strict {
                Err(Fault::exception(
                    "TypeError",
                    "The global property is read-only.",
                ))
            } else {
                Ok(())
            };
        }
        if !initialize && binding.kind == Declaration::Const {
            return Err(Fault::exception(
                "TypeError",
                format!("Cannot assign to const {name}."),
            ));
        }
        if initialize && binding.value.is_some() && binding.kind != Declaration::Var {
            return Err(Fault::new(format!(
                "Binding {name} is already initialized."
            )));
        }
        let kind = binding.kind;
        self.put_binding(env, name.into(), Some(value), kind, true)
    }
    fn make_function(
        &mut self,
        program: &Rc<Program>,
        template: usize,
        environment: usize,
        this: &Value,
    ) -> Result<Value, Fault> {
        let marker = self.inhibit_transient_collection();
        let result = self.make_function_unpublished(program, template, environment, this);
        self.restore_transient_marker(marker);
        result
    }

    fn make_function_unpublished(
        &mut self, program: &Rc<Program>, template: usize, environment: usize, this: &Value,
    ) -> Result<Value, Fault> {
        self.step()?;
        let kind = program.templates[template].kind;
        let function_prototype = match kind {
            FunctionKind::Ordinary => None,
            FunctionKind::Async => Some(self.async_function_prototype()?),
            FunctionKind::Generator => Some(self.generator_function_prototype()?),
        };
        if self.function_count_refused(1) {
            return Err(self.function_limit_fault("script"));
        }
        self.prepare_function_slot_storage()?;
        let index = self.next_function_slot()?;
        // A named expression's private name lives outside its parameter/body
        // scope. A parameter, var, or lexical binding may shadow that name.
        let environment = if let Some(name) = &program.templates[template].name {
            let private = self.new_environment(environment, false)?;
            self.put_binding(
                private,
                name.clone(),
                Some(Value::Function(index)),
                Declaration::Const,
                false,
            )?;
            private
        } else {
            environment
        };
        let lexical_this = if program.templates[template].arrow {
            let value = self.capture_arrow_this(this)?;
            self.value_bytes += value.bytes();
            Some(value)
        } else {
            None
        };
        self.capture_environment(environment);
        let inserted = self.insert_function_slot(Function::Script {
            program: program.clone(),
            template,
            environment,
            lexical_this,
            lexical_invocation: if program.templates[template].arrow { self.invocation } else { None },
            home_object: None,
        })?;
        debug_assert_eq!(inserted.id, index);
        if program.templates[template].class_element {
            self.scope_assigned_closures.insert(index);
        }
        self.initialize_function_properties(
            inserted,
            program.templates[template].name.as_deref().unwrap_or(""),
            program.templates[template].parameter_length(),
            program.templates[template].constructible,
        )?;
        if let Some(prototype) = function_prototype {
            let properties = self.function_properties[index];
            self.objects[properties].prototype = Some(Ordinary::Object(prototype));
        }
        if kind == FunctionKind::Generator {
            self.initialize_generator_function(index)?;
        }
        Ok(Value::Function(index))
    }
    fn prepare(
        &mut self,
        program: &Rc<Program>,
        body: &[StmtId],
        environment: usize,
        function_scope: bool,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.prepare_scopes(
            program,
            body,
            environment,
            environment,
            function_scope,
            host,
        )
    }
    fn prepare_scopes(
        &mut self,
        program: &Rc<Program>,
        body: &[StmtId],
        environment: usize,
        variable_environment: usize,
        function_scope: bool,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let marker = self.inhibit_transient_collection();
        let result = self.prepare_scopes_unpublished(program, body, environment, variable_environment, function_scope, host);
        self.restore_transient_marker(marker);
        result
    }

    fn prepare_scopes_unpublished(
        &mut self,
        program: &Rc<Program>,
        body: &[StmtId],
        environment: usize,
        variable_environment: usize,
        function_scope: bool,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.work_as(std::mem::size_of_val(body), WorkKind::ScopeNames)?;
        // Check the complete scope before creating bindings or running a host
        // call. This also catches cross-script let/var collisions at global scope.
        let variable_names = if function_scope {
            collect_var_names_with_work(program, body, |bytes| self.work_as(bytes, WorkKind::ScopeNames))?
        } else {
            Vec::new()
        };
        // Collect last direct declarations during the existing charged preflight.
        // This is temporary borrowed-name metadata, with no binding/host effects.
        let mut final_functions = BTreeMap::new();
        for &statement in body {
            self.step()?;
            if function_scope {
                if let StmtKind::Function(name, _) = &program.stmts[statement].kind {
                    self.work_as(name.len().saturating_add(std::mem::size_of::<(&str, StmtId)>()), WorkKind::ScopeNames)?;
                    final_functions.insert(name.as_str(), statement);
                }
            }
            match &program.stmts[statement].kind {
                StmtKind::Declare(Declaration::Let | Declaration::Const, names) => {
                    for (pattern, _) in names {
                        for name in program.pattern_names_with_work(*pattern, |n| self.work_as(n, WorkKind::PatternPreparation))? {
                            if environment == self.global_environment {
                                self.ensure_global_intrinsics(&name)?;
                                self.check_operation_lexical(&name)?;
                            }
                            if self.environment(environment).bindings.contains_key(&name) {
                                return Err(Fault::new(format!(
                                    "Duplicate lexical binding {name:?}."
                                ))
                                .at(&program.source, program.stmts[statement].at));
                            }
                        }
                    }
                }
                StmtKind::Function(name, _)
                    if self
                        .environment(variable_environment)
                        .bindings
                        .get(name)
                        .is_some_and(|b| b.kind != Declaration::Var) =>
                {
                    return Err(Fault::new(format!(
                        "Function conflicts with lexical binding {name:?}."
                    )));
                }
                _ => {}
            }
        }
        for name in &variable_names {
            self.step()?;
            if self
                .environment(variable_environment)
                .bindings
                .get(name)
                .is_some_and(|binding| binding.kind != Declaration::Var)
            {
                return Err(Fault::new(format!(
                    "var conflicts with lexical binding {name:?}."
                )));
            }
        }
        if variable_environment == self.global_environment {
            for name in &variable_names { self.ensure_global_intrinsics(name)?; }
            for &statement in body {
                if let StmtKind::Function(name, _) = &program.stmts[statement].kind { self.ensure_global_intrinsics(name)?; }
            }
            if let Some((_, properties)) = self.browser_global {
                // Global bindings and Window expandos have separate storage in
                // this subset. Migrating an accessor would invoke its getter
                // and discard its descriptor. Reject the complete declaration
                // set before any binding, function or host side effect instead.
                let function_names =
                    body.iter()
                        .filter_map(|&statement| match &program.stmts[statement].kind {
                            StmtKind::Function(name, _) => Some(name),
                            _ => None,
                        });
                for name in variable_names.iter().chain(function_names) {
                    self.step()?;
                    self.ensure_global_intrinsics(name)?;
                    if self.is_operation_global(name) {
                        continue;
                    }
                    if self
                        .object(properties)?
                        .properties
                        .get(name)
                        .is_some_and(|property| {
                            property.accessor.is_some() || property.getter.is_some()
                        })
                    {
                        return Err(Fault::new(format!(
                            "Global var/function declarations over accessor property {name:?} are not implemented."
                        )));
                    }
                }
            }
        }
        if variable_environment == self.global_environment {
            for name in &variable_names {
                self.check_operation_declaration(name, false)?;
            }
            for &statement in body {
                if let StmtKind::Function(name, _) = &program.stmts[statement].kind {
                    self.check_operation_declaration(name, true)?;
                }
            }
        }
        for name in variable_names {
            self.step()?;
            if variable_environment == self.global_environment && self.is_operation_global(&name) {
                self.declare_operation_var(&name, environment != variable_environment)?;
                continue;
            }
            if !self
                .environment(variable_environment)
                .bindings
                .contains_key(&name)
            {
                let migrated_order = if variable_environment == self.global_environment {
                    self.browser_global.and_then(|(_, id)| self.objects[id].properties.get(&name).map(|property| property.order))
                } else { None };
                let value = if variable_environment == self.global_environment {
                    if let Some((_, properties)) = self.browser_global {
                        if self.object(properties)?.properties.contains_key(&name) {
                            let value = self.get_property_with_host(properties, &name, host)?;
                            self.remove_property(properties, &name)?;
                            value
                        } else {
                            Value::Undefined
                        }
                    } else {
                        Value::Undefined
                    }
                } else {
                    Value::Undefined
                };
                // A new sloppy-eval global is configurable. Keep the descriptor
                // in this realm, charging its retained name copy before effects.
                let eval_global = variable_environment == self.global_environment
                    && environment != variable_environment
                    && self.browser_global.is_some_and(|(_, properties)| !self.objects[properties].properties.contains_key(&name));
                let descriptor_name = if eval_global { self.work(name.len())?; Some(name.clone()) } else { None };
                self.put_binding_ordered(
                    variable_environment,
                    name,
                    Some(value),
                    Declaration::Var,
                    false,
                    migrated_order,
                )?;
                if let Some(name) = descriptor_name { self.global_attributes.insert(name, (true, true, true)); }
            }
        }
        for &statement in body {
            self.step()?;
            match &program.stmts[statement].kind {
                StmtKind::Declare(kind @ (Declaration::Let | Declaration::Const), names) => {
                    for (pattern, _) in names {
                        for name in program.pattern_names_with_work(*pattern, |n| self.work_as(n, WorkKind::PatternPreparation))? {
                            self.put_binding(environment, name, None, *kind, false)?;
                        }
                    }
                }
                StmtKind::Function(name, template) => {
                    // All declarations were checked before any instantiation.
                    // Keep surviving source order and fresh per-entry identity.
                    if function_scope && final_functions.get(name.as_str()) != Some(&statement) {
                        continue;
                    }
                    let function =
                        self.make_function(program, *template, environment, &Value::Undefined)?;
                    if let Value::Function(index) = &function {
                        let properties = self.ordinary_id(Ordinary::Function(*index))?;
                        self.store_property(
                            properties,
                            "name".into(),
                            Value::String(name.clone()),
                            Some((false, false, true)),
                        )?;
                    }
                    if variable_environment == self.global_environment && self.is_operation_global(name) {
                        self.declare_operation_function(
                            name,
                            function,
                            environment != variable_environment,
                        )?;
                        continue;
                    }
                    if variable_environment == self.global_environment && self.host_owns_global(name, host) {
                        if let Some((handle, _)) = self.browser_global {
                            self.pin_exported_value(&function);
                            host.set(handle, name, self.copy_value(&function)?)
                                .map_err(Fault::new)?;
                        }
                    }
                    self.put_binding(
                        variable_environment,
                        name.clone(),
                        Some(function),
                        if function_scope {
                            Declaration::Var
                        } else {
                            Declaration::Let
                        },
                        true,
                    )?;
                    // A hoisted global function supersedes an earlier Window
                    // expando. Keep one owner for its value and retained bytes.
                    if variable_environment == self.global_environment {
                        if let Some((_, properties)) = self.browser_global {
                            self.remove_property(properties, name)?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn environment_limit_fault(&self, kind: &str, limit: usize) -> Fault {
        let occupied = self.environments.iter().flatten().count();
        let pinned = self.environments.iter().flatten().filter(|env| env.captured).count();
        Fault::new(format!(
            "{kind} limit reached ({limit}). Scope arena: occupied={occupied}; pinned={pinned}; reusable={}; bindings={}. Counts only; no binding names or values.",
            self.free_environments.len(), self.bindings,
        ))
    }

    fn capture_environment(&mut self, mut id: usize) {
        loop {
            let environment = self.environment_mut(id);
            if environment.captured {
                break;
            }
            environment.captured = true;
            match environment.parent {
                Some(parent) => id = parent,
                None => break,
            }
        }
    }

    fn discard_environment(&mut self, id: usize) {
        if let Some(environment) = self.environments[id].take() {
            self.bindings -= environment.bindings.len();
            self.value_bytes -= environment.arguments.as_ref().map_or(0, arguments::Pending::bytes);
            self.value_bytes -= environment.invocation.as_ref().map_or(0, derived_context::Data::bytes);
            self.value_bytes -= environment.bindings.values()
                .filter_map(|binding| binding.value.as_ref())
                .map(Value::bytes).sum::<usize>();
            self.free_environments.push(id);
        }
    }

    // Called only when the lexical activation has completed. Returned closures
    // and mapped arguments have already pinned every environment they need.
    // Cleanup invokes no page code, refunds no job work and also runs on errors.
    fn release_environment(&mut self, id: usize) {
        if id != 0 && self.environments[id].as_ref().is_some_and(|env| !env.captured) {
            self.discard_environment(id);
        }
    }

    fn collect_environments(&mut self) {
        let mut live = vec![false; self.environments.len()];
        let mut roots = vec![0];
        if let Some(context) = self.invocation { roots.push(context.environment); }
        roots.extend(self.intrinsic_realms.iter().map(|realm| realm.global_environment));
        roots.extend(self.transient_roots.roots.iter().filter_map(|edge| match edge {
            function_reachability::Edge::Environment(id) => Some(*id),
            _ => None,
        }));
        self.promises.await_environment_roots(&mut roots);
        for function in &self.functions {
            if let Function::Script { environment, lexical_invocation, .. } = function {
                roots.push(*environment);
                if let Some(context) = lexical_invocation { roots.push(context.environment); }
            }
        }
        for object in &self.objects {
            if let Some(data) = &object.promise { data.await_environment_roots(&mut roots); }
            if let Some(generator) = &object.generator { generator.environment_roots(&mut roots); }
            if let Some(arguments) = &object.arguments {
                if !arguments.mapped.is_empty() { roots.push(arguments.environment); }
            }
        }
        while let Some(mut env) = roots.pop() {
            loop {
                if live[env] {
                    break;
                }
                live[env] = true;
                let Some(parent) = self.environment(env).parent else {
                    break;
                };
                env = parent;
            }
        }
        for (id, used) in live.into_iter().enumerate() {
            if used {
                self.environment_mut(id).captured = true;
            } else {
                self.discard_environment(id);
            }
        }
    }
}

impl Runtime {
    fn retain_regexp(&mut self, bytes: usize) -> Result<(), Fault> {
        let total = self.retained_regexp.saturating_add(bytes);
        if total > self.limits.capacity.retained_regexp {
            return Err(Fault::new("Retained regexp limit reached (2 MiB)."));
        }
        self.retained_regexp = total;
        Ok(())
    }

    fn regexp_fault(error: regexp::Error) -> Fault {
        match error {
            regexp::Error::Syntax(message) => Fault::exception("SyntaxError", message),
            other => Fault::new(other.message()),
        }
    }

    fn regexp_object(&mut self, expression: Rc<regexp::Regex>) -> Result<Value, Fault> {
        let id = self.new_object(None)?;
        self.objects[id].regexp = Some(expression);
        self.objects[id].prototype = self.regexp_prototype.map(Ordinary::Object);
        self.set_property(id, "lastIndex".into(), Value::Number(0.0))?;
        self.objects[id]
            .properties
            .get_mut("lastIndex")
            .unwrap()
            .enumerable = false;
        self.objects[id]
            .properties
            .get_mut("lastIndex")
            .unwrap()
            .configurable = false;
        Ok(Value::Object(id))
    }


    fn regexp_run(&mut self, id: usize, input: &[u16]) -> Result<Option<regexp::Match>, Fault> {
        let expression = self
            .object(id)?
            .regexp
            .clone()
            .ok_or_else(|| Fault::new("Expected a regexp object."))?;
        let index = self.get_property(id, "lastIndex")?;
        index.primitive()?;
        self.work(index.bytes())?;
        let number = index.script_number()?;
        let stateful = expression.flags.global || expression.flags.sticky;
        let start = if stateful && !number.is_nan() && number > 0.0 {
            number.trunc() as usize
        } else {
            0
        };
        let found = expression
            .find(input, start, expression.flags.sticky, &mut self.regexp_work)
            .map_err(Self::regexp_fault)?;
        if stateful {
            self.set_property_throw(
                id,
                "lastIndex".into(),
                Value::Number(found.as_ref().map_or(0.0, |found| found.end() as f64)),
            )?;
        }
        Ok(found)
    }

    fn units_string(&mut self, input: &[u16]) -> Result<String, Fault> {
        self.work_as(input.len().saturating_mul(2), WorkKind::ConversionOutput)?;
        let text = String::from_utf16(input).map_err(|_| {
            Fault::new(
                "A regexp/string result containing a lone UTF-16 surrogate is not implemented.",
            )
        })?;
        if text.len() > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        Ok(text)
    }

    // Locate one UTF-16 unit without building a whole-string conversion.
    // The returned offset is within the scalar (zero or one), not the string.
    // No receiver borrow escapes this call or crosses a script/host callback.
    fn string_scalar_at(
        &mut self,
        text: &str,
        target: usize,
    ) -> Result<Option<(char, usize)>, Fault> {
        // UTF-16 unit count never exceeds UTF-8 byte count. An obviously
        // out-of-range lookup needs neither a scan nor a temporary buffer.
        if target >= text.len() {
            return Ok(None);
        }
        let mut byte_offset = 0;
        let mut unit_offset = 0;
        let mut characters = text.chars();
        while byte_offset < text.len() {
            // The input is valid UTF-8 and offset is at a scalar boundary.
            // Charge before decoding, including when the target is beyond
            // the UTF-16 end but still below the conservative byte bound.
            let first = text.as_bytes()[byte_offset];
            let width = if first < 0x80 {
                1
            } else if first < 0xe0 {
                2
            } else if first < 0xf0 {
                3
            } else {
                4
            };
            self.work_as(width, WorkKind::CharacterScan)?;
            let character = characters.next().expect("remaining UTF-8 scalar");
            byte_offset += width;
            let units = character.len_utf16();
            if target < unit_offset + units {
                return Ok(Some((character, target - unit_offset)));
            }
            unit_offset += units;
        }
        Ok(None)
    }

    fn string_character(
        &mut self,
        text: &str,
        method: &str,
        args: &[Value],
    ) -> Result<Value, Fault> {
        let position = args.first().unwrap_or(&Value::Undefined);
        position.primitive()?;
        self.work_as(position.bytes().saturating_mul(2), WorkKind::ConversionOutput)?;
        let position = position.script_number()?;
        let position = if position.is_nan() {
            0.0
        } else {
            position.trunc()
        };
        let missing = match method {
            "charAt" => Value::String(String::new()),
            "codePointAt" => Value::Undefined,
            _ => Value::Number(f64::NAN),
        };
        // Keep numeric coercion and the pre-cast infinity/range check on the
        // method route. Indexed properties already have an integer key.
        if position < 0.0 || position >= text.len() as f64 {
            return Ok(missing);
        }
        let Some((character, index)) = self.string_scalar_at(text, position as usize)? else {
            return Ok(missing);
        };
        let mut buffer = [0; 2];
        let units = character.encode_utf16(&mut buffer);
        if method == "charAt" {
            self.string_from_units(&units[index..index + 1])
        } else {
            let code = if method == "codePointAt" && index == 0 {
                u32::from(character)
            } else {
                u32::from(units[index])
            };
            Ok(Value::Number(f64::from(code)))
        }
    }




    fn split_string(&mut self, text: &str, args: &[Value]) -> Result<Value, Fault> {
        self.work(text.len().saturating_mul(3))?;
        let separator = args.first().unwrap_or(&Value::Undefined);
        let expression = match separator {
            Value::Object(id) => self.object(*id)?.regexp.clone(),
            _ => None,
        };
        if expression.is_none() {
            separator.primitive()?;
        }
        let limit_value = args.get(1).unwrap_or(&Value::Undefined);
        limit_value.primitive()?;
        self.work(limit_value.bytes().saturating_mul(2))?;
        let limit = if matches!(limit_value, Value::Undefined) {
            u32::MAX as usize
        } else {
            int32(limit_value.script_number()?) as u32 as usize
        };
        let mut pieces = Vec::new();
        if limit == 0 {
            return self.array_from_values(&pieces);
        }
        if matches!(separator, Value::Undefined) {
            return self.array_from_values(&[Value::String(text.into())]);
        }
        let input: Vec<_> = text.encode_utf16().collect();
        if let Some(expression) = expression {
            if input.is_empty() {
                if expression
                    .find(&input, 0, true, &mut self.regexp_work)
                    .map_err(Self::regexp_fault)?
                    .is_none()
                {
                    pieces.push(Value::String(String::new()));
                }
                return self.array_from_values(&pieces);
            }
            let mut previous = 0;
            let mut at = 0;
            while at < input.len() {
                self.step()?;
                let Some(found) = expression
                    .find(&input, at, true, &mut self.regexp_work)
                    .map_err(Self::regexp_fault)?
                else {
                    at = regexp::advance_string_index(&input, at, expression.flags.unicode);
                    continue;
                };
                if found.end() == previous {
                    at = regexp::advance_string_index(&input, at, expression.flags.unicode);
                    continue;
                }
                let value = Value::String(self.units_string(&input[previous..at])?);
                if self.push_split(&mut pieces, value, limit)? {
                    return self.array_from_values(&pieces);
                }
                for capture in found.captures.iter().skip(1) {
                    let value = if let Some((start, end)) = capture {
                        Value::String(self.units_string(&input[*start..*end])?)
                    } else {
                        Value::Undefined
                    };
                    if self.push_split(&mut pieces, value, limit)? {
                        return self.array_from_values(&pieces);
                    }
                }
                previous = found.end();
                at = previous;
            }
            let value = Value::String(self.units_string(&input[previous..])?);
            self.push_split(&mut pieces, value, limit)?;
        } else {
            self.work(separator.bytes())?;
            let separator = separator.script_text()?;
            if separator.is_empty() {
                for unit in input {
                    let value = Value::String(self.units_string(&[unit])?);
                    if self.push_split(&mut pieces, value, limit)? {
                        break;
                    }
                }
            } else {
                for piece in text.split(&separator) {
                    self.work(piece.len())?;
                    if self.push_split(&mut pieces, Value::String(piece.into()), limit)? {
                        break;
                    }
                }
            }
        }
        self.array_from_values(&pieces)
    }

    fn push_split(
        &mut self,
        pieces: &mut Vec<Value>,
        value: Value,
        limit: usize,
    ) -> Result<bool, Fault> {
        self.step()?;
        if pieces.len() >= self.limits.heap.array_length {
            return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
        }
        pieces.push(value);
        Ok(pieces.len() >= limit)
    }


    fn error_object(
        &mut self,
        kind: &'static str,
        message: Option<String>,
    ) -> Result<Value, Fault> {
        let id = self.new_error_object(kind, self.error_prototype(kind))?;
        if let Some(message) = message {
            self.store_property(id, "message".into(), Value::String(message), Some((false, true, true)))?;
        }
        Ok(Value::Object(id))
    }

    fn error_prototype(&self, kind: &str) -> Option<Ordinary> {
        self.error_prototypes.get(kind).or_else(|| self.error_prototypes.get("Error"))
            .copied().map(Ordinary::Object)
    }

    fn new_error_object(&mut self, kind: &'static str, prototype: Option<Ordinary>) -> Result<usize, Fault> {
        let id = self.new_object(None)?;
        self.objects[id].error_name = Some(kind);
        self.objects[id].prototype = prototype;
        if !self.error_prototypes.contains_key(kind) {
            self.store_property(
                id,
                "name".into(),
                Value::String(kind.into()),
                Some((false, true, true)),
            )?;
        }
        Ok(id)
    }

    fn construct_error(
        &mut self, kind: &'static str, args: &[Value], new_target: Option<usize>, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let prototype = self.builtin_instance_prototype(new_target, |runtime| runtime.error_prototype(kind), host)?;
        let id = self.new_error_object(kind, prototype)?;
        if let Some(value) = args.first().filter(|value| !matches!(value, Value::Undefined)) {
            let value = self.copy_value(value)?;
            let message = self.primitive_with_hint(value, true, host)?.script_text()?;
            self.store_property(id, "message".into(), Value::String(message), Some((false, true, true)))?;
        }
        Ok(Value::Object(id))
    }

    fn error_text(&mut self, id: usize, host: &mut dyn Host) -> Result<Value, Fault> {
        let name = self.get_property_with_host(id, "name", host)?;
        let name = if matches!(name, Value::Undefined) {
            "Error".into()
        } else {
            self.primitive_with_hint(name, true, host)?.script_text()?
        };
        // Name conversion can mutate message or throw. Observe that before
        // reading the message, as Error.prototype.toString requires.
        let message = self.get_property_with_host(id, "message", host)?;
        let message = if matches!(message, Value::Undefined) {
            String::new()
        } else {
            self.primitive_with_hint(message, true, host)?
                .script_text()?
        };
        self.work(name.len().saturating_add(message.len()))?;
        let text = if name.is_empty() {
            message
        } else if message.is_empty() {
            name
        } else {
            format!("{name}: {message}")
        };
        self.checked_value(Value::String(text))
    }

    fn object(&self, id: usize) -> Result<&Object, Fault> {
        self.objects
            .get(id)
            .ok_or_else(|| Fault::new("Invalid object handle."))
    }

    fn property_limit_fault(&self, object: &Object) -> Fault {
        Fault::new(format!(
            "Object property limit reached ({} per object / {} per document). \
             Target properties: {} string, {} symbol; document properties={}, objects={}; \
             retained-value bytes={}/{} (payload accounting, not total process memory).",
            self.limits.heap.object_properties, self.limits.heap.properties,
            object.properties.len(), object.symbol_properties.len(), self.properties,
            self.objects.len(), self.value_bytes, self.limits.heap.retained_values,
        ))
    }

    fn new_object(&mut self, length: Option<usize>) -> Result<usize, Fault> {
        self.step()?;
        if self.objects.len() >= self.limits.heap.objects {
            return Err(Fault::new(
                format!("Object identity limit reached ({} per document).", self.limits.heap.objects),
            ));
        }
        if length.is_some_and(|length| length > self.limits.heap.array_length) {
            return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
        }
        let id = self.objects.len();
        self.objects.push(Object {
            realm: self.active_realm,
            proxy: None,
            date: None,
            url: None,
            headers: None,
            response: None,
            rect: None,
            dom_exception: None,
            primitive: None,
            promise: None,
            arguments: None,
            binary: None,
            collection: None,
            iterator: None,
            generator: None,
            text: None,
            properties: StringProperties::new(),
            symbol_properties: BTreeMap::new(),
            length,
            length_writable: true,
            extensible: true,
            immutable_prototype: false,
            window_named_properties: false,
            next_order: 0,
            error_name: None,
            regexp: None,
            prototype: if length.is_some() {
                self.array_prototype.or(self.object_prototype)
            } else {
                self.object_prototype
            }
            .map(Ordinary::Object),
        });
        Ok(id)
    }

    fn get_property(&mut self, id: usize, name: &str) -> Result<Value, Fault> {
        self.step()?;
        self.property_name(name)?;
        self.get_ordinary_property(Ordinary::Object(id), name)
    }

    fn get_property_with_host(
        &mut self,
        id: usize,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        self.get_ordinary_property_with_host(Ordinary::Object(id), name, host)
    }

    /// Accessor callbacks, including borrowed native functions, share the
    /// active job and call-depth budget. The guard also bounds native-method
    /// getter cycles which do not themselves enter a script function frame.
    fn call_accessor(
        &mut self,
        callback: Option<usize>,
        receiver: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        let Some(callback) = callback else {
            return Ok(Value::Undefined);
        };
        self.check_native_call_stack("Accessor call")?;
        self.call_depth += 1;
        self.transient_roots.owned_call_depth += 1;
        let result = self.call_value(Value::Function(callback), args, receiver, host);
        self.call_depth -= 1;
        self.transient_roots.owned_call_depth -= 1;
        result
    }

    fn own_property(&mut self, id: usize, name: &str) -> Result<Value, Fault> {
        self.materialize_native_property(id, name)?;
        if let Some(index) = self.string_object_index(id, name) {
            return self.string_object_index_value(id, index);
        }
        if self
            .object(id)?
            .properties
            .get(name)
            .is_some_and(|property| property.accessor.is_some())
        {
            return Err(Fault::new(
                "An accessor read requires an execution context.",
            ));
        }
        if let Some((environment, binding)) = self.mapped_argument(id, name) {
            return self.binding_value(environment, &binding);
        }
        if let Some(index) = self.typed_numeric_key(id, name)? {
            return self.typed_index_value(id, index);
        }
        if let Some(expression) = self.object(id)?.regexp.clone() {
            let value = match name {
                "source" => return self.regexp_source_value(&expression),
                "flags" => Some(Value::String(expression.flags.text())),
                "global" => Some(Value::Bool(expression.flags.global)),
                "ignoreCase" => Some(Value::Bool(expression.flags.insensitive)),
                "multiline" => Some(Value::Bool(expression.flags.multiline)),
                "dotAll" => Some(Value::Bool(expression.flags.dot_all)),
                "sticky" => Some(Value::Bool(expression.flags.sticky)),
                "unicode" => Some(Value::Bool(expression.flags.unicode)),
                "unicodeSets" | "hasIndices" => Some(Value::Bool(false)),
                _ => None,
            };
            if let Some(value) = value {
                return self.checked_value(value);
            }
        }
        if name == "length" {
            if let Some(length) = self.object(id)?.length {
                return Ok(Value::Number(length as f64));
            }
        }
        let bytes = self
            .object(id)?
            .properties
            .get(name)
            .map_or(0, |property| property.value.bytes());
        self.work_as(bytes, WorkKind::ValueCopy)?;
        Ok(self
            .object(id)?
            .properties
            .get(name)
            .map_or(Ok(Value::Undefined), |property| property.value.as_value().cloned())?)
    }

    fn property_name(&mut self, name: &str) -> Result<(), Fault> {
        if name.len() > self.limits.capacity.property_bytes {
            return Err(Fault::new("Property-name limit reached (256 bytes)."));
        }
        self.work_as(name.len(), WorkKind::PropertyName)
    }

    /// Number/string adapters for the supported conversion APIs. Addition and
    /// loose equality use an explicit default hint. Native conversion still
    /// requires host opt-in before any ordinary or symbol property is read.
    fn primitive_with_hint(
        &mut self,
        value: Value,
        string_hint: bool,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.primitive_with_preference(
            value,
            if string_hint {
                primitive_conversion::Hint::String
            } else {
                primitive_conversion::Hint::Number
            },
            host,
        )
    }

    fn primitive_methods(
        &mut self,
        value: Value,
        string_hint: bool,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let methods = if string_hint {
            ["toString", "valueOf"]
        } else {
            ["valueOf", "toString"]
        };
        for name in methods {
            // Do not cache both methods. The first callback may replace the
            // second method, alter the prototype, or throw an arbitrary value.
            let method = if let Value::Host(object) = value {
                self.host_read(object, name, host)?
            } else {
                self.get_ordinary_property_with_host(
                    Ordinary::from_value(&value).unwrap(),
                    name,
                    host,
                )?
            };
            if matches!(method, Value::Function(_)) {
                let primitive = self.call_value(method, &[], value.clone(), host)?;
                if !matches!(
                    primitive,
                    Value::Object(_) | Value::Function(_) | Value::Host(_)
                ) {
                    return self.checked_value(primitive);
                }
            }
        }
        Err(Fault::exception(
            "TypeError",
            "Cannot convert the object to a primitive value.",
        ))
    }

    fn property_key(&mut self, value: Value, host: &mut dyn Host) -> Result<PropertyKey, Fault> {
        let primitive = self.primitive_with_hint(value, true, host)?;
        if let Value::Symbol(id) = primitive {
            self.symbol(id)?;
            return Ok(PropertyKey::Symbol(id));
        }
        self.work_as(primitive.bytes(), WorkKind::ConversionOutput)?;
        let key = match primitive {
            Value::String(text) => text,
            Value::StringUnits(units) => return self.canonical_units_key(units),
            primitive => primitive.script_text()?,
        };
        self.property_name(&key)?;
        Ok(PropertyKey::String(key))
    }

    fn require_property_receiver(value: &Value) -> Result<(), Fault> {
        if matches!(value, Value::Null | Value::Undefined) {
            Err(Fault::exception(
                "TypeError",
                "Cannot access a property of null or undefined.",
            ))
        } else {
            Ok(())
        }
    }

    fn set_property_throw(&mut self, id: usize, name: String, value: Value) -> Result<(), Fault> {
        let previous = self.strict;
        self.strict = true;
        let result = self.set_property(id, name, value);
        self.strict = previous;
        result
    }
    fn set_property(&mut self, id: usize, name: String, value: Value) -> Result<(), Fault> {
        self.store_property(id, name, value, None)
    }

    fn store_property(
        &mut self,
        id: usize,
        name: String,
        value: Value,
        attributes: Option<(bool, bool, bool)>,
    ) -> Result<(), Fault> {
        self.store_property_kind(id, name, value, attributes, None)
    }

    fn store_accessor_property(
        &mut self,
        id: usize,
        name: String,
        accessor: Accessors,
        attributes: (bool, bool),
    ) -> Result<(), Fault> {
        self.store_property_kind(
            id,
            name,
            Value::Undefined,
            Some((attributes.0, false, attributes.1)),
            Some(accessor),
        )
    }

    fn store_property_kind(
        &mut self,
        id: usize,
        name: String,
        value: Value,
        attributes: Option<(bool, bool, bool)>,
        accessor: Option<Accessors>,
    ) -> Result<(), Fault> {
        self.step()?;
        self.property_name(&name)?;
        if self.object(id)?.window_named_properties {
            return if self.strict || attributes.is_some() {
                Err(Fault::exception("TypeError", "Cannot define an own property on WindowProperties."))
            } else { Ok(()) };
        }
        if self.string_object_index(id, &name).is_some() {
            return if self.strict || attributes.is_some() {
                Err(Fault::exception(
                    "TypeError",
                    "The string property is read-only.",
                ))
            } else {
                Ok(())
            };
        }
        if attributes.is_none()
            && self
                .object(id)?
                .properties
                .get(&name)
                .is_some_and(|property| !property.writable)
        {
            return if self.strict {
                Err(Fault::exception("TypeError", "The property is read-only."))
            } else {
                Ok(())
            };
        }
        if self.object(id)?.regexp.is_some()
            && matches!(
                name.as_str(),
                "source"
                    | "flags"
                    | "global"
                    | "ignoreCase"
                    | "multiline"
                    | "dotAll"
                    | "sticky"
                    | "unicode"
                    | "unicodeSets"
                    | "hasIndices"
            )
        {
            return Err(Fault::exception(
                "TypeError",
                "Regular-expression configuration is read-only.",
            ));
        }
        if self.object(id)?.length.is_some() && name == "length" {
            if attributes.is_none() && !self.objects[id].length_writable {
                return if self.strict {
                    Err(Fault::exception("TypeError", "Array length is read-only."))
                } else {
                    Ok(())
                };
            }
            value.primitive()?;
            self.work(value.bytes())?;
            let length = value.script_number()?;
            if length != (int32(length) as u32) as f64 {
                return Err(Fault::exception("RangeError", "Invalid array length."));
            }
            if let Some((enumerable, writable, configurable)) = attributes {
                if !self.apply_array_length(
                    id,
                    Some(length as usize),
                    Some(writable),
                    !enumerable && !configurable && accessor.is_none(),
                )? {
                    return Err(Fault::exception(
                        "TypeError",
                        "Cannot redefine array length.",
                    ));
                }
                return Ok(());
            }
            return self.set_array_length(id, length as usize);
        }
        let object = self.object(id)?;
        let array_index = object.length.and_then(|_| property_index(&name));
        let previous = object.properties.get(&name);
        if previous.is_none() && !object.extensible {
            return if self.strict || attributes.is_some() {
                Err(Fault::exception(
                    "TypeError",
                    "The object is not extensible.",
                ))
            } else {
                Ok(())
            };
        }
        if array_index.is_some_and(|index| index >= object.length.unwrap_or(0))
            && !object.length_writable
        {
            return if self.strict || attributes.is_some() {
                Err(Fault::exception("TypeError", "Array length is read-only."))
            } else {
                Ok(())
            };
        }
        if array_index.is_some_and(|index| index >= self.limits.heap.array_length) {
            return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
        }
        let old_bytes = previous.map_or(0, |property| property.value.bytes());
        let is_new = previous.is_none();
        if is_new
            && (object.properties.len() + object.symbol_properties.len() >= self.limits.heap.object_properties
                || self.properties >= self.limits.heap.properties)
        {
            return Err(self.property_limit_fault(object));
        }
        let (order, next_order) = self.property_creation_order(id, previous.map(|property| property.order))?;
        let enumerable = attributes.map_or_else(
            || previous.is_none_or(|property| property.enumerable),
            |attrs| attrs.0,
        );
        let writable = attributes.is_none_or(|attrs| attrs.1);
        let configurable = attributes.map_or_else(
            || previous.is_none_or(|property| property.configurable),
            |attrs| attrs.2,
        );
        let new_bytes = value.bytes();
        if new_bytes > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        let retained =
            self.value_bytes - old_bytes + new_bytes + if is_new { name.len() } else { 0 };
        if retained > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work_as(new_bytes, WorkKind::PropertyStore)?;
        if let Some((environment, binding)) = self.mapped_argument(id, &name) {
            if accessor.is_none() {
                let parameter = self.copy_value(&value)?;
                self.store_binding(environment, &binding, parameter, false)?;
            }
            // The parameter write has its own retained-value accounting.
            // Recalculate the property total after that write.
            if !writable || accessor.is_some() {
                self.objects[id]
                    .arguments
                    .as_mut()
                    .unwrap()
                    .mapped
                    .remove(&name);
            }
        }
        let retained =
            self.value_bytes - old_bytes + new_bytes + if is_new { name.len() } else { 0 };
        if retained > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.commit_property_creation_order(id, next_order);
        let object = &mut self.objects[id];
        object.properties.insert(
            name,
            Property {
                value: StoredValue::Value(value),
                getter: None,
                accessor,
                order,
                enumerable,
                writable,
                configurable,
            },
        );
        object.next_order = next_order;
        if let (Some(length), Some(index)) = (&mut object.length, array_index) {
            *length = (*length).max(index + 1);
        }
        self.value_bytes = retained;
        self.properties += usize::from(is_new);
        Ok(())
    }

    fn remove_property(&mut self, id: usize, name: &str) -> Result<(), Fault> {
        self.step()?;
        self.property_name(name)?;
        self.object(id)?;
        if let Some(property) = self.objects[id].properties.remove(name) {
            if let Some(arguments) = self.objects[id].arguments.as_mut() {
                arguments.mapped.remove(name);
            }
            self.value_bytes -= name.len() + property.value.bytes();
            self.properties -= 1;
        }
        Ok(())
    }

    fn array_from_values(&mut self, values: &[Value]) -> Result<Value, Fault> {
        let id = self.new_object(Some(values.len()))?;
        for (index, value) in values.iter().enumerate() {
            let value = self.copy_value(value)?;
            self.set_property(id, index.to_string(), value)?;
        }
        Ok(Value::Object(id))
    }

    fn enumerable_keys(&mut self, id: usize) -> Result<Vec<StringKey>, Fault> {
        self.own_string_keys(id, true)
    }

    // Snapshot stored string keys and integer-indexed entries. Fixed,
    // nonenumerable virtual properties (array length / RegExp metadata) cannot
    // become enumerable and need no slot in assign/defineProperties snapshots.
    fn own_string_keys(&mut self, id: usize, enumerable_only: bool) -> Result<Vec<StringKey>, Fault> {
        self.reject_unsupported_proxy_operation(id, "ownKeys")?;
        let object = self.object(id)?;
        let indexed_length = self.typed_view(id).map_or_else(
            || self.string_object_length(id).unwrap_or(0),
            |view| view.length,
        );
        if indexed_length.saturating_add(object.properties.len()) > self.limits.heap.object_properties {
            return Err(Fault::new(
                format!("Property enumeration limit reached ({} keys).", self.limits.heap.object_properties),
            ));
        }
        self.work_as(
            object
                .properties
                .len()
                .saturating_mul(self.limits.capacity.property_bytes * 2)
                .saturating_add(indexed_length.saturating_mul(16)),
            WorkKind::PropertyEnumeration,
        )?;
        let mut keys: Vec<_> = self.objects[id]
            .properties
            .iter()
            .filter(|(_, property)| !enumerable_only || property.enumerable)
            .map(|(name, property)| (name.to_owned(), property.order))
            .collect();
        for index in 0..indexed_length {
            keys.push((StringKey::Scalar(index.to_string()), index));
        }
        // Canonical integer indices precede string keys. String keys retain
        // creation order, including after a removed array index is re-added.
        keys.sort_by_cached_key(|(name, order)| {
            name.index().map_or((1, *order), |index| (0, index))
        });
        Ok(keys.into_iter().map(|(name, _)| name).collect())
    }

    fn object_keys(&mut self, value: &Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let keys = if let Value::Host(handle) = value {
            self.host_keys(*handle, false, host)?
        } else {
            let object = self.ordinary_required(value)?;
            let id = self.ordinary_id(object)?;
            self.enumerable_keys(id)?
        };
        let result = self.new_object(Some(keys.len()))?;
        for (index, name) in keys.into_iter().enumerate() {
            self.set_property(result, index.to_string(), name.into_value())?;
        }
        Ok(Value::Object(result))
    }

    fn for_in_keys(&mut self, value: &Value, host: &mut dyn Host) -> Result<Vec<StringKey>, Fault> {
        match value {
            Value::Object(id) => self.inherited_keys(Ordinary::Object(*id)),
            Value::Function(id) => self.inherited_keys(Ordinary::Function(*id)),
            Value::String(_) | Value::StringUnits(_) => {
                self.work(value.bytes())?;
                let length = value.string_length().expect("matched string");
                if length > self.limits.heap.object_properties {
                    return Err(Fault::new(format!("for-in enumeration limit reached ({} keys).", self.limits.heap.object_properties)));
                }
                self.work(length.saturating_mul(8))?;
                Ok((0..length).map(|index| StringKey::Scalar(index.to_string())).collect())
            }
            Value::Host(handle) => self.host_keys(*handle, true, host),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::Symbol(_) => Ok(Vec::new()),
        }
    }

    fn has_property_value(
        &mut self,
        key: Value,
        object: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        // Check the right-hand type before converting the left, as the
        // relational operator requires. Presence is not a truthiness test.
        if !matches!(
            object,
            Value::Object(_) | Value::Function(_) | Value::Host(_)
        ) {
            return Err(Fault::exception(
                "TypeError",
                "The right side of in must be an object.",
            ));
        }
        let name = self.property_key(key, host)?;
        self.reject_foreign_window(&object, host)?;
        if let Some(ordinary) = Ordinary::from_value(&object) {
            return self.has_ordinary_key(ordinary, &name, host).map(Value::Bool);
        }
        if let PropertyKey::StringUnits(_) = &name {
            let base = self.units_lookup_base(&object, host)?.ok_or_else(||
                Fault::new("UTF-16 properties of this native object are not implemented."))?;
            return self.has_ordinary_key(base, &name, host).map(Value::Bool);
        }
        if let (Value::Host(handle), PropertyKey::String(name)) = (&object, &name) {
            if self.is_browser_global(*handle) { self.ensure_global_intrinsics(name)?; }
        }
        let PropertyKey::String(name) = name else {
            let PropertyKey::Symbol(key) = name else {
                unreachable!()
            };
            let own = self.symbol_backing(&object, host)?;
            return Ok(Value::Bool(
                if let Some(backing) = self.symbol_lookup_base(&object, key, own, host)? {
                    self.receiver_symbol_owner(&object, backing, key, host)?.is_some()
                } else {
                    false
                },
            ));
        };
        let present = match object {
            Value::Host(handle) => {
                if let Some(object) = self.host_ordinary(handle, &name, host)? {
                    self.host_ordinary_has(handle, object, &name, host)?
                } else if let Some(tail) = self.host_lookup_tail(handle,&name,host)? {
                    self.property_owner(tail,&name)?.is_some()
                } else if self.is_browser_global(handle) && !self.host_owns_global(&name, host) {
                    let (_, properties) = self.browser_global.unwrap();
                    if self.global_binding(&name)
                        || self.object(properties)?.properties.contains_key(&name)
                    {
                        true
                    } else if self.is_operation_global(&name) {
                        self.property_owner(Ordinary::Object(properties), &name)?.is_some()
                    } else {
                        host.has(handle, &name).map_err(Fault::new)?
                    }
                } else {
                    if host.has(handle, &name).map_err(Fault::new)? { true }
                    else if let Some(tail) = self.host_inherited_read_tail(handle, &name, host)? {
                        self.property_owner(tail, &name)?.is_some()
                    } else {
                        host.ordinary_prototype(handle)
                            && self.property_owner(Ordinary::Object(self.object_prototype.unwrap()), &name)?.is_some()
                    }
                }
            }
            _ => unreachable!(),
        };
        Ok(Value::Bool(present))
    }

    fn apply_arguments(&mut self, value: &Value, host: &mut dyn Host) -> Result<Vec<Value>, Fault> {
        // Earlier indexed getters may have produced fresh Values that only this
        // partial list owns. Inhibit construction, then hand the complete list
        // directly to call_value's input registration.
        let marker = self.inhibit_transient_collection();
        let result = self.apply_arguments_unpublished(value, host);
        self.restore_transient_marker(marker);
        result
    }

    fn apply_arguments_unpublished(&mut self, value: &Value, host: &mut dyn Host) -> Result<Vec<Value>, Fault> {
        if matches!(value, Value::Null | Value::Undefined) {
            return Ok(Vec::new());
        }
        if let Value::Host(handle) = value {
            self.reject_foreign_window(value, host)?;
            // CreateListFromArrayLike uses Get, not a presence test: missing
            // positions become actual undefined arguments. Capture length once
            // before conversion/index getters can change the live collection.
            let length_value = self.host_read(*handle, "length", host)?;
            let numeric = self.binary_number(&length_value, host)?;
            let length = if numeric.is_nan() || numeric <= 0.0 { 0.0 }
                else { numeric.floor().min(9_007_199_254_740_991.0) };
            if length > self.limits.capacity.arguments as f64 {
                return Err(self.argument_limit_fault());
            }
            if length > usize::MAX as f64 {
                return Err(Fault::new("Native apply length exceeds the index representation."));
            }
            let length = length as usize;
            let mut result = Vec::new();
            result.try_reserve_exact(length)
                .map_err(|_| Fault::new("Native apply argument reservation failed."))?;
            for index in 0..length {
                self.step()?;
                result.push(self.host_read(*handle, &index.to_string(), host)?);
            }
            return Ok(result);
        }
        let object = Ordinary::from_value(value).ok_or_else(|| {
            Fault::exception("TypeError", "Function.apply requires an array-like object.")
        })?;
        let length = self.get_ordinary_property_with_host(object, "length", host)?;
        let numeric = self.binary_number(&length, host)?;
        let length = if numeric.is_nan() || numeric <= 0.0 {
            0
        } else {
            if numeric.trunc() > self.limits.capacity.arguments as f64 {
                return Err(Fault::new(format!("Call argument limit reached ({}).", self.limits.capacity.arguments)));
            }
            numeric.trunc() as usize
        };
        let mut result = Vec::with_capacity(length);
        for index in 0..length {
            result.push(self.get_ordinary_property_with_host(object, &index.to_string(), host)?);
        }
        Ok(result)
    }

    fn append_string(&mut self, output: &mut String, text: &str) -> Result<(), Fault> {
        if output.len().saturating_add(text.len()) > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        self.work_as(text.len(), WorkKind::ConversionOutput)?;
        output.push_str(text);
        Ok(())
    }

    fn array_method(
        &mut self,
        id: usize,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if matches!(method, "sort" | "toSorted")
            && args
                .first()
                .is_some_and(|value| !matches!(value, Value::Undefined | Value::Function(_)))
        {
            return Err(Fault::exception(
                "TypeError",
                "Array comparator must be callable.",
            ));
        }
        let length = if method == "concat" {
            0
        } else {
            self.array_like_length(id, host)?
        };
        match method {
            "push" => {
                if length.saturating_add(args.len()) > self.limits.heap.array_length {
                    return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                }
                for (index, value) in args.iter().enumerate() {
                    let value = self.copy_value(value)?;
                    let previous_strict = self.strict;
                    self.strict = true;
                    let result = self.assign_ordinary_property(
                        Ordinary::Object(id),
                        (length + index).to_string(),
                        value,
                        host,
                    );
                    self.strict = previous_strict;
                    result?;
                }
                let previous_strict = self.strict;
                self.strict = true;
                let result = self.array_length_throw(id, length + args.len(), host);
                self.strict = previous_strict;
                result?;
                Ok(Value::Number((length + args.len()) as f64))
            }
            "pop" => {
                let value = if length == 0 {
                    Value::Undefined
                } else {
                    let key = (length - 1).to_string();
                    let value = self.get_property_with_host(id, &key, host)?;
                    if self.objects[id].proxy.is_none() && self.objects[id]
                        .properties
                        .get(&key)
                        .is_some_and(|property| !property.configurable)
                    {
                        return Err(Fault::exception(
                            "TypeError",
                            "Cannot remove a nonconfigurable array element.",
                        ));
                    }
                    self.array_delete_index(id, length - 1)?;
                    value
                };
                let previous_strict = self.strict;
                self.strict = true;
                let result = self.array_length_throw(id, length.saturating_sub(1), host);
                self.strict = previous_strict;
                result?;
                Ok(value)
            }
            "join" | "toString" => {
                let separator = if method == "toString"
                    || matches!(args.first(), None | Some(Value::Undefined))
                {
                    Value::String(",".into())
                } else {
                    let value = self.copy_value(&args[0])?;
                    Self::array_join_primitive(self.primitive_with_hint(value, true, host)?)?
                };
                let text = if self.objects[id].length.is_some() {
                    self.array_join(id, length, &separator, host)?
                } else {
                    self.array_like_join(id, length, &separator, host)?
                };
                self.checked_value(text)
            }
            "indexOf" | "includes" => {
                let needle = args.first().unwrap_or(&Value::Undefined);
                if length == 0 {
                    return Ok(if method == "includes" {
                        Value::Bool(false)
                    } else {
                        Value::Number(-1.0)
                    });
                }
                let start = self.binary_relative(args.get(1), length, 0, host)?;
                for index in start..length {
                    self.step()?;
                    let key = index.to_string();
                    if method == "indexOf"
                        && self.property_owner(Ordinary::Object(id), &key)?.is_none()
                    {
                        continue;
                    }
                    let value = self.get_property_with_host(id, &key, host)?;
                    self.work(value.bytes().saturating_add(needle.bytes()))?;
                    let matches = strict_equal(&value, needle)
                        || (method == "includes"
                            && matches!((&value, needle), (Value::Number(a), Value::Number(b)) if a.is_nan() && b.is_nan()));
                    if matches {
                        return Ok(if method == "includes" {
                            Value::Bool(true)
                        } else {
                            Value::Number(index as f64)
                        });
                    }
                }
                Ok(if method == "includes" {
                    Value::Bool(false)
                } else {
                    Value::Number(-1.0)
                })
            }
            "slice" => {
                let start = self.binary_relative(args.first(), length, 0, host)?;
                let end = self.binary_relative(args.get(1), length, length, host)?;
                let result = self.new_object(Some(end.saturating_sub(start)))?;
                for index in start..end {
                    self.step()?;
                    let key = index.to_string();
                    if self.property_owner(Ordinary::Object(id), &key)?.is_some() {
                        let value = self.get_property_with_host(id, &key, host)?;
                        self.set_property(result, (index - start).to_string(), value)?;
                    }
                }
                Ok(Value::Object(result))
            }
            _ => self.extra_array_method(id, length, method, args, host),
        }
    }
}

fn property_index(name: &str) -> Option<usize> {
    if name.is_empty()
        || (name.len() > 1 && name.starts_with('0'))
        || !name.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    name.parse::<u32>()
        .ok()
        .filter(|index| *index != u32::MAX)
        .map(|index| index as usize)
}

impl Runtime {
    fn statements(
        &mut self,
        program: &Rc<Program>,
        body: &[StmtId],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let mut last = None;
        for &statement in body {
            let marker = self.publish_statement_inputs(env, this, last.as_ref())?;
            let completion = self.statement(program, statement, env, this, host);
            self.restore_root_marker(marker);
            match completion? {
                Control::Normal(value) => {
                    if value.is_some() {
                        last = value;
                    }
                }
                abrupt => return Ok(abrupt.update_empty(last)),
            }
        }
        Ok(Control::Normal(last))
    }

    fn statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        self.step()?;
        self.check_native_evaluation_stack()?;
        let marker = self.publish_statement_inputs(env, this, None)?;
        self.eval_depth += 1;
        self.transient_roots.owned_eval_depth += 1;
        // Expression/return statements have no pending statement work while
        // their expression runs. Enter the heap evaluator from this activation,
        // rather than retaining statement_inner plus a second eval wrapper.
        // Keep the expression step and all heap/call quotas; eval_depth counts
        // the live native activation once. Compound statements keep their guard.
        let expression = match program.stmts[id].kind {
            StmtKind::Expr(expression) => Some((expression, false)),
            StmtKind::Return(Some(expression)) => Some((expression, true)),
            _ => None,
        };
        #[cfg(test)]
        let tail = !self.transient_roots.disable_if_statement_tail;
        #[cfg(not(test))]
        let tail = true;
        let result = if tail && matches!(program.stmts[id].kind, StmtKind::If(_, _, _)) {
            self.selected_if_statement(program, id, env, this, host)
        } else if matches!(program.stmts[id].kind, StmtKind::Try(_, _, _)) {
            // Avoid retaining the general match frame across a recursive try body.
            self.try_statement(program, id, env, this, host)
        } else if let Some((expression, returning)) = expression {
            self.step().and_then(|_| {
                #[cfg(test)]
                let value = if self.recursive_evaluation_oracle {
                    self.eval_inner(program, expression, env, this, host)
                } else {
                    self.eval_heap(program, expression, env, this, host)
                };
                #[cfg(not(test))]
                let value = self.eval_heap(program, expression, env, this, host);
                value.map_err(|error| error.at(&program.source, program.exprs[expression].at))
                    .map(|value| if returning { Control::Return(value) } else { Control::Normal(Some(value)) })
            })
        } else {
            self.statement_inner(program, id, env, this, host)
        }
                .map(|control| match control {
                    Control::Break(target, value) if target == id => Control::Normal(value),
                    other => other,
                });
        self.eval_depth -= 1;
        self.transient_roots.owned_eval_depth -= 1;
        self.restore_root_marker(marker);
        result.map_err(|error| error.at(&program.source, program.stmts[id].at))
    }

    fn statement_inner(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let statement = &program.stmts[id].kind;
        match statement {
            StmtKind::Empty | StmtKind::Function(_, _) => Ok(Control::Normal(None)),
            StmtKind::Expr(expr) => Ok(Control::Normal(Some(
                self.eval(program, *expr, env, this, host)?,
            ))),
            StmtKind::Block(body) => {
                let scope = self.new_environment(env, false)?;
                let result = match self.publish_statement_inputs(scope, this, None) {
                    Ok(marker) => {
                        let result = match self.prepare(program, body, scope, false, host) {
                            Ok(()) => self.statements(program, body, scope, this, host),
                            Err(error) => Err(error),
                        };
                        self.restore_root_marker(marker);
                        result
                    }
                    Err(error) => Err(error),
                };
                self.release_environment(scope);
                result
            }
            StmtKind::Declare(_, _) => self.declaration_statement(program, id, env, this, host),
            StmtKind::If(_, _, _) => self.if_statement(program, id, env, this, host),
            StmtKind::While(_, _) | StmtKind::DoWhile(_, _) => {
                self.while_statement(program, id, env, this, host)
            }
            StmtKind::For(_, _, _, _) => self.for_statement(program, id, env, this, host),
            StmtKind::ForIn(_, _, _) | StmtKind::ForOf(_, _, _, false) => {
                self.for_each_statement(program, id, env, this, host)
            }
            StmtKind::ForOf(_, _, _, true) => Err(Fault::new("Async iterator loop requires a resumption plan.")),
            StmtKind::Switch { .. } => self.switch_statement(program, id, env, this, host),
            StmtKind::Throw(expression) => {
                let value = self.eval(program, *expression, env, this, host)?;
                self.work(value.bytes())?;
                Err(Fault::thrown(value))
            }
            StmtKind::Try(_, _, _) => unreachable!("try statements have a dedicated dispatch"),
            StmtKind::Return(value) => Ok(Control::Return(match value {
                Some(expr) => self.eval(program, *expr, env, this, host)?,
                None => Value::Undefined,
            })),
            StmtKind::Yield(_) | StmtKind::YieldFrom(_) => {
                unreachable!("yield statements require an owned generator frame")
            }
            StmtKind::Break(target) => Ok(Control::Break(*target, None)),
            StmtKind::Continue(target) => Ok(Control::Continue(*target, None)),
        }
    }

    fn if_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let mut current = id;
        let mut ancestors = Vec::new();
        loop {
            let at = program.stmts[current].at;
            let StmtKind::If(condition, yes, no) = &program.stmts[current].kind else {
                unreachable!("only if statements enter a flat false-branch tail")
            };
            let selected = self
                .eval(program, *condition, env, this, host)
                .map_err(|error| error.at(&program.source, at))?
                .truthy();
            if !selected {
                if let Some(next) = no {
                    if matches!(program.stmts[*next].kind, StmtKind::If(_, _, _)) {
                        // The parser's shared branch cap also bounds this path.
                        // Each visited if keeps its execution and copy charges;
                        // only native-stack growth is removed.
                        self.work_as(std::mem::size_of::<StmtId>(), WorkKind::ConditionalTarget)
                            .map_err(|error| error.at(&program.source, at))?;
                        ancestors.push(current);
                        current = *next;
                        self.step().map_err(|error| {
                            error.at(&program.source, program.stmts[current].at)
                        })?;
                        continue;
                    }
                }
            }
            let branch = if selected { Some(*yes) } else { *no };
            let completion = if let Some(branch) = branch {
                self.statement(program, branch, env, this, host)
            } else {
                Ok(Control::Normal(None))
            };
            return completion
                .map(|control| control.update_empty(Some(Value::Undefined)))
                .map(|control| match control {
                    // A labelled else-if can be represented by the same AST
                    // shape. Retain every visited target when unwinding once.
                    Control::Break(target, value)
                        if target == current || ancestors.contains(&target) =>
                    {
                        Control::Normal(value)
                    }
                    other => other,
                })
                .map_err(|error| error.at(&program.source, at));
        }
    }

    fn declaration_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        match &program.stmts[id].kind {
            StmtKind::Declare(kind, declarations) => {
                let mode = self.declaration_mode(*kind, env)?;
                for &declarator in declarations {
                    self.evaluate_declarator(program, declarator, *kind, mode, env, this, host)?;
                }
                Ok(Control::Normal(None))
            }
            _ => unreachable!(),
        }
    }

    fn declaration_mode(&mut self, kind: Declaration, env: usize) -> Result<binding_patterns::Mode, Fault> {
        Ok(if kind == Declaration::Var {
            binding_patterns::Mode::Var(self.var_environment(env)?)
        } else {
            binding_patterns::Mode::Initialize(env)
        })
    }

    #[inline(never)]
    fn evaluate_resumption_declarator(
        &mut self, program: &Rc<Program>, statement: StmtId, index: usize, env: usize, this: &Value, host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let StmtKind::Declare(kind, declarations) = &program.stmts[statement].kind else {
            unreachable!("declarator instruction requires a declaration");
        };
        let result = (|| {
            let mode = self.declaration_mode(*kind, env)?;
            self.evaluate_declarator(program, declarations[index], *kind, mode, env, this, host)
        })();
        result.map_err(|error: Fault| error.at(&program.source, program.stmts[statement].at))
    }

    fn evaluate_declarator(
        &mut self,
        program: &Rc<Program>,
        (pattern, initial): (binding_patterns::Id, Option<ExprId>),
        kind: Declaration,
        mode: binding_patterns::Mode,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.step()?;
        if let Some(expression) = initial {
            let value = self.eval(program, expression, env, this, host)?;
            if let Some(name) = program.pattern_name(pattern) {
                self.infer_class_name(program, expression, name, &value)?;
            }
            let scope_candidate = if matches!(&program.patterns[pattern].kind, binding_patterns::Kind::Name(_)) {
                self.fresh_anonymous_scope_closure(program, expression, &value)
            } else { None };
            self.initialize_pattern(program, pattern, value, mode, env, this, host)?;
            if let Some(id) = scope_candidate { self.scope_assigned_closures.insert(id); }
        } else if kind == Declaration::Let {
            self.initialize_pattern(program, pattern, Value::Undefined, mode, env, this, host)?;
        }
        Ok(())
    }

    fn switch_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let StmtKind::Switch {
            discriminant,
            cases,
            body,
        } = &program.stmts[id].kind
        else {
            unreachable!()
        };
        // The discriminant sees the outer scope. All case tests/statements see
        // one shared lexical block, including TDZ bindings in unselected cases.
        let value = self.eval(program, *discriminant, env, this, host)?;
        let scope = self.new_environment(env, false)?;
        let marker = self.transient_marker();
        let result = (|| {
            self.push_transient_value(&value)?;
            self.push_transient_root(function_reachability::Edge::Environment(scope))?;
            self.prepare(program, body, scope, false, host)?;
            if let Some(index) = self.select_switch_case(program, cases, &value, scope, this, host)? {
                let start = cases[index].1;
                self.statements(program, &body[start..], scope, this, host)
                    .map(|control| control.update_empty(Some(Value::Undefined)))
            } else {
                Ok(Control::Normal(Some(Value::Undefined)))
            }
        })();
        self.restore_transient_marker(marker);
        self.release_environment(scope);
        result
    }

    #[inline(never)]
    fn select_switch_case(
        &mut self,
        program: &Rc<Program>,
        cases: &[(Option<ExprId>, usize)],
        value: &Value,
        scope: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Option<usize>, Fault> {
        let mut default = None;
        for (index, (test, _)) in cases.iter().enumerate() {
            self.step()?;
            if let Some(test) = test {
                let candidate = self.eval(program, *test, scope, this, host)?;
                self.work(value.bytes().saturating_add(candidate.bytes()))?;
                if strict_equal(value, &candidate) {
                    return Ok(Some(index));
                }
            } else {
                default = Some(index);
            }
        }
        Ok(default)
    }

    fn while_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let (condition, body, mut first) = match &program.stmts[id].kind {
            StmtKind::While(condition, body) => (condition, body, false),
            StmtKind::DoWhile(body, condition) => (condition, body, true),
            _ => unreachable!(),
        };
        let empty_true = matches!(program.exprs[*condition].kind, ExprKind::Literal(Value::Bool(true)))
            && matches!(program.stmts[*body].kind, StmtKind::Empty);
        #[cfg(test)]
        let empty_true = empty_true && !self.transient_roots.disable_empty_loop_fast_path;
        if empty_true { return self.empty_true_loop(program, *condition, first); }
        let mut last = Some(Value::Undefined);
        loop {
            if !first {
                let marker = self.publish_statement_inputs(env, this, last.as_ref())?;
                let condition = self.eval(program, *condition, env, this, host);
                self.restore_root_marker(marker);
                if !condition?.truthy() { break; }
            }
            first = false;
            let marker = self.publish_statement_inputs(env, this, last.as_ref())?;
            let completion = self.statement(program, *body, env, this, host);
            self.restore_root_marker(marker);
            match completion? {
                Control::Normal(value) => {
                    if value.is_some() {
                        last = value;
                    }
                }
                Control::Continue(target, value) if target == id => {
                    if value.is_some() {
                        last = value;
                    }
                }
                Control::Break(target, value) if target == id => {
                    if value.is_some() {
                        last = value;
                    }
                    break;
                }
                abrupt => return Ok(abrupt.update_empty(last)),
            }
        }
        Ok(Control::Normal(last))
    }

    #[inline(never)]
    fn empty_true_loop(&mut self, program: &Program, condition: ExprId,
        first_body: bool) -> Result<Control, Fault> {
        // No callback or live arena Value is produced by these two AST nodes.
        // Retain one step per condition/body, the condition's zero-byte copy
        // call, both native guards and the original source position of faults.
        let mut condition_phase = !first_body;
        self.step()?;
        // Neither phase can reenter evaluation or change the native stack
        // reservation. Measure once while preserving each phase's job step.
        self.check_native_evaluation_stack()?;
        loop {
            if condition_phase {
                self.check_pending_evaluation()
                    .map_err(|error| error.at(&program.source, program.exprs[condition].at))?;
                self.work_as(0, WorkKind::ValueCopy)?;
            }
            condition_phase = !condition_phase;
            self.step()?;
        }
    }

    fn for_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        match &program.stmts[id].kind {
            StmtKind::For(initial, condition, advance, body) => {
                let mut loop_env = self.new_environment(env, false)?;
                let result = (|| {
                    let per_iteration = if let Some(initial) = initial {
                        self.initialize_classic_for(program, *initial, loop_env, this, host)?
                    } else {
                        Vec::new()
                    };
                    // A for-let initializer and each loop body have distinct lexical
                    // environments, so closures retain the value for that iteration.
                    if !per_iteration.is_empty() {
                        loop_env = self.iteration_environment(loop_env, env, &per_iteration)?;
                    }
                    let mut last = Some(Value::Undefined);
                    loop {
                        self.step()?;
                        if let Some(condition) = condition {
                            let marker = self.publish_statement_inputs(loop_env, this, last.as_ref())?;
                            let value = self.eval(program, *condition, loop_env, this, host);
                            self.restore_root_marker(marker);
                            if !value?.truthy() {
                                break;
                            }
                        }
                        let marker = self.publish_statement_inputs(loop_env, this, last.as_ref())?;
                        let completion = self.statement(program, *body, loop_env, this, host);
                        self.restore_root_marker(marker);
                        match completion? {
                            Control::Normal(value) => {
                                if value.is_some() {
                                    last = value;
                                }
                            }
                            Control::Continue(target, value) if target == id => {
                                if value.is_some() {
                                    last = value;
                                }
                            }
                            Control::Break(target, value) if target == id => {
                                if value.is_some() {
                                    last = value;
                                }
                                break;
                            }
                            abrupt => return Ok(abrupt.update_empty(last)),
                        }
                        if !per_iteration.is_empty() {
                            loop_env = self.iteration_environment(loop_env, env, &per_iteration)?;
                        }
                        if let Some(advance) = advance {
                            let marker = self.publish_statement_inputs(loop_env, this, last.as_ref())?;
                            let value = self.eval(program, *advance, loop_env, this, host);
                            self.restore_root_marker(marker);
                            value?;
                        }
                    }
                    Ok(Control::Normal(last))
                })();
                self.release_environment(loop_env);
                result
            }
            _ => unreachable!(),
        }
    }

    fn for_each_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        // Iterator records, LHS snapshots and close completions are still held
        // in Rust locals. Keep the complete loop inhibited until all are ported.
        let marker = self.inhibit_transient_collection();
        let result = self.for_each_statement_unpublished(program, id, env, this, host);
        self.restore_transient_marker(marker);
        result
    }

    fn for_each_statement_unpublished(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let (initial, source, body, of) = match &program.stmts[id].kind {
            StmtKind::ForIn(initial, source, body) => (initial, source, body, false),
            StmtKind::ForOf(initial, source, body, false) => (initial, source, body, true),
            _ => unreachable!(),
        };
        let lexical = match &program.stmts[*initial].kind {
            StmtKind::Declare(kind @ (Declaration::Let | Declaration::Const), names) => {
                Some((*kind, &names[0].0))
            }
            _ => None,
        };
        // The lexical name is uninitialized while evaluating the right side,
        // but each body iteration gets a separate initialized binding.
        let mut right_scope = None;
        let source = (|| {
            let right_env = if let Some((kind, pattern)) = lexical {
                let scope = self.new_lexical_iteration_scope(program, env, kind, *pattern)?;
                right_scope = Some(scope);
                scope
            } else {
                env
            };
            self.eval(program, *source, right_env, this, host)
        })();
        if let Some(scope) = right_scope {
            self.release_environment(scope);
        }
        let source = source?;
        let mut iterator = self.loop_iterator(source, of, host)?;
        let mut last = Some(Value::Undefined);
        while let Some(key) = self.loop_next(&mut iterator, host)? {
            self.step()?;
            let mut iteration_scope = None;
            let completion = (|| {
                let body_env = if let Some((kind, pattern)) = lexical {
                    let scope = self.new_lexical_iteration_scope(program, env, kind, *pattern)?;
                    iteration_scope = Some(scope);
                    self.initialize_pattern(
                        program,
                        *pattern,
                        key,
                        binding_patterns::Mode::Initialize(scope),
                        scope,
                        this,
                        host,
                    )?;
                    scope
                } else {
                    match &program.stmts[*initial].kind {
                        StmtKind::Declare(Declaration::Var, names) => {
                            let scope = self.var_environment(env)?;
                            self.initialize_pattern(
                                program,
                                names[0].0,
                                key,
                                binding_patterns::Mode::Var(scope),
                                env,
                                this,
                                host,
                            )?;
                        }
                        StmtKind::Expr(expression) => {
                            if let ExprKind::Pattern(pattern) = program.exprs[*expression].kind {
                                self.initialize_pattern(
                                    program,
                                    pattern,
                                    key,
                                    binding_patterns::Mode::Assign,
                                    env,
                                    this,
                                    host,
                                )?;
                            } else {
                                let target =
                                    self.reference(program, *expression, env, this, host)?;
                                self.write_reference(target, key, host)?;
                            }
                        }
                        _ => unreachable!(),
                    }
                    env
                };
                self.statement(program, *body, body_env, this, host)
            })();
            if let Some(scope) = iteration_scope {
                self.release_environment(scope);
            }
            match completion {
                Ok(Control::Normal(value)) => {
                    if value.is_some() {
                        last = value;
                    }
                }
                Ok(Control::Continue(target, value)) if target == id => {
                    if value.is_some() {
                        last = value;
                    }
                }
                Ok(Control::Break(target, value)) if target == id => {
                    if value.is_some() {
                        last = value;
                    }
                    self.loop_close(&mut iterator, Ok(()), host)?;
                    break;
                }
                Ok(abrupt) => {
                    return self.loop_close(&mut iterator, Ok(abrupt.update_empty(last)), host);
                }
                Err(error) => return self.loop_close(&mut iterator, Err(error), host),
            }
        }
        Ok(Control::Normal(last))
    }

    #[inline(never)]
    fn new_lexical_iteration_scope(
        &mut self, program: &Program, parent: usize, kind: Declaration, pattern: binding_patterns::Id,
    ) -> Result<usize, Fault> {
        let scope = self.new_environment(parent, false)?;
        let prepared = (|| {
            for name in program.pattern_names_with_work(pattern, |n| self.work_as(n, WorkKind::PatternPreparation))? {
                self.put_binding(scope, name, None, kind, false)?;
            }
            Ok(scope)
        })();
        if prepared.is_err() {
            self.release_environment(scope);
        }
        prepared
    }

    #[inline(never)]
    fn initialize_classic_for(
        &mut self, program: &Rc<Program>, initial: StmtId, environment: usize,
        this: &Value, host: &mut dyn Host,
    ) -> Result<Vec<String>, Fault> {
        let mut per_iteration = Vec::new();
        if let StmtKind::Declare(kind @ (Declaration::Let | Declaration::Const), declarations) = &program.stmts[initial].kind {
            for (pattern, _) in declarations {
                for name in program.pattern_names_with_work(*pattern, |n| self.work_as(n, WorkKind::PatternPreparation))? {
                    self.put_binding(environment, name.clone(), None, *kind, false)?;
                    if *kind == Declaration::Let {
                        per_iteration.push(name);
                    }
                }
            }
        }
        self.statement(program, initial, environment, this, host)?;
        Ok(per_iteration)
    }

    fn collect_native_iterable(
        &mut self,
        source: &Value,
        host: &mut dyn Host,
    ) -> Result<Option<Vec<Value>>, Fault> {
        let Some(method) = self.iterator_method(source, host)? else {
            return Ok(None);
        };
        let source = self.copy_value(source)?;
        let mut iterator = self.iterator_from_method(source, method, host)?;
        let mut values = Vec::new();
        while let Some(value) = self.iterator_next(&mut iterator, host)? {
            // The shared copy budget bounds this temporary list, including an
            // infinite custom iterator that never allocates a fresh result.
            self.work(8usize.saturating_add(value.bytes()))?;
            values.push(value);
        }
        Ok(Some(values))
    }

    fn loop_iterator(
        &mut self,
        source: Value,
        of: bool,
        host: &mut dyn Host,
    ) -> Result<LoopIterator, Fault> {
        if !of {
            let keys = self.for_in_keys(&source, host)?;
            return Ok(LoopIterator::Keys(source, keys.into_iter()));
        }
        let method = self
            .iterator_method(&source, host)?
            .ok_or_else(|| Fault::exception("TypeError", "The value is not iterable."))?;
        self.iterator_from_method(source, method, host)
            .map(LoopIterator::Values)
    }

    fn loop_close<T>(
        &mut self,
        iterator: &mut LoopIterator,
        completion: Result<T, Fault>,
        host: &mut dyn Host,
    ) -> Result<T, Fault> {
        match iterator {
            LoopIterator::Keys(_, _) => completion,
            LoopIterator::Values(iterator) => self.iterator_close(iterator, completion, host),
        }
    }

    fn loop_next(
        &mut self,
        iterator: &mut LoopIterator,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        self.step()?;
        match iterator {
            LoopIterator::Keys(source, keys) => {
                for name in keys {
                    self.step()?;
                    self.work(name.byte_len())?;
                    if let StringKey::Units(units) = &name {
                        let Some(base) = self.units_lookup_base(source, host)? else { continue; };
                        let owner = self.units_dispatch_owner(base, units)?;
                        let Some(owner) = self.ordinary_descriptor_owner(owner)? else { continue; };
                        if !self.objects[owner].properties.get_units(units).is_some_and(|p| p.enumerable) { continue; }
                        return Ok(Some(name.into_value()));
                    }
                    let StringKey::Scalar(name) = name else { unreachable!() };
                    // Snapshot keys but recheck their current presence. Removed
                    // entries disappear; later additions do not enter for-in.
                    if let Some(object) = Ordinary::from_value(source) {
                        let Some(owner) = self.property_owner(object, &name)? else {
                            continue;
                        };
                        if !self.own_enumerable(owner, &name)? {
                            continue;
                        }
                    } else if let Value::Host(handle) = source {
                        if self.is_browser_global(*handle) {
                            if !self.window_enumerable(&name, host)? { continue; }
                        } else if let Some(object) = self.host_ordinary(*handle, &name, host)? {
                            let Some(owner) = self.property_owner(object, &name)? else {
                                continue;
                            };
                            if !self.own_enumerable(owner, &name)? {
                                continue;
                            }
                        } else if !host.enumeration_member(*handle, &name).map_err(Fault::new)? {
                            continue;
                        }
                    }
                    return Ok(Some(Value::String(name)));
                }
                Ok(None)
            }
            LoopIterator::Values(iterator) => self.iterator_next(iterator, host),
        }
    }

    fn try_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let StmtKind::Try(body, _, _) = &program.stmts[id].kind else {
            unreachable!()
        };
        let completion = self.statement(program, *body, env, this, host);
        self.finish_try_statement(program, id, completion, env, this, host)
    }

    #[inline(never)]
    fn finish_try_statement(
        &mut self,
        program: &Rc<Program>,
        id: StmtId,
        mut completion: Result<Control, Fault>,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Control, Fault> {
        let StmtKind::Try(_, catch, finally) = &program.stmts[id].kind else {
            unreachable!()
        };
        // Limit/capability failures stop this job, including finally.
        // They cannot be caught or replaced with return/break/throw.
        if completion
            .as_ref()
            .is_err_and(|error| error.exception.is_none())
        {
            return completion;
        }
        if let (Err(error), Some((binding, catch_body))) = (&mut completion, catch) {
            completion = self.catch_statement(program, *binding, *catch_body, error, env, this, host);
        }
        if completion
            .as_ref()
            .is_err_and(|error| error.exception.is_none())
        {
            return completion;
        }
        if let Some(finally) = finally {
            match self.finally_statement(program, *finally, &completion, env, this, host) {
                Ok(Control::Normal(_)) => {}
                replacement => completion = replacement,
            }
        }
        completion.map(|control| control.update_empty(Some(Value::Undefined)))
    }

    // Catch/finally-only scratch must not enlarge every recursively entered try
    // body. Existing one-MiB stack guards exercise this separation.
    #[inline(never)]
    fn catch_statement(&mut self, program: &Rc<Program>, binding: Option<binding_patterns::Id>,
        body: StmtId, error: &mut Fault, env: usize, this: &Value, host: &mut dyn Host) -> Result<Control, Fault> {
        let scope = self.enter_catch_scope(program, binding, error, env, this, host)?;
        let result = self.statement(program, body, scope, this, host);
        self.release_environment(scope);
        result
    }

    #[inline(never)]
    fn enter_catch_scope(&mut self, program: &Rc<Program>, binding: Option<binding_patterns::Id>,
        error: &mut Fault, env: usize, this: &Value, host: &mut dyn Host) -> Result<usize, Fault> {
        self.record_caught_error(error, "catch");
        let exception = error.exception.take().expect("catchable exception");
        let value = match *exception {
            Exception::Value(value) => value,
            Exception::Error(kind) => self.materialize_native_error(kind, error.message.clone(), error.location.take())?,
        };
        let catch_env = self.new_environment(env, false)?;
        let marker = self.transient_marker();
        let result = (|| {
            self.push_transient_value(&value)?;
            self.push_transient_root(function_reachability::Edge::Environment(catch_env))?;
            if let Some(pattern) = binding {
                for name in program.pattern_names_with_work(pattern, |n| self.work_as(n, WorkKind::PatternPreparation))? {
                    self.put_binding(catch_env, name, None, Declaration::Let, false)?;
                }
                if let binding_patterns::Kind::Name(name) = &program.patterns[pattern].kind {
                    self.environment_mut(catch_env).catch_parameter = Some(name.clone());
                }
                self.initialize_pattern(program, pattern, value, binding_patterns::Mode::Initialize(catch_env), catch_env, this, host)?;
            }
            Ok(())
        })();
        self.restore_transient_marker(marker);
        if let Err(error) = result {
            self.release_environment(catch_env);
            return Err(error);
        }
        Ok(catch_env)
    }

    #[inline(never)]
    fn finally_statement(&mut self, program: &Rc<Program>, body: StmtId,
        completion: &Result<Control, Fault>, env: usize, this: &Value, host: &mut dyn Host) -> Result<Control, Fault> {
        let marker = self.transient_marker();
        let result = self.push_completion_roots(completion)
            .and_then(|()| self.statement(program, body, env, this, host));
        self.restore_transient_marker(marker);
        result
    }

    fn iteration_environment(
        &mut self,
        old: usize,
        parent: usize,
        names: &[String],
    ) -> Result<usize, Fault> {
        // The old/new scope ownership transfer is atomic with respect to any
        // future collection; no root may reference the released old scope.
        let marker = self.inhibit_transient_collection();
        let result = self.iteration_environment_unpublished(old, parent, names);
        self.restore_transient_marker(marker);
        result
    }

    fn iteration_environment_unpublished(
        &mut self,
        old: usize,
        parent: usize,
        names: &[String],
    ) -> Result<usize, Fault> {
        let new = self.new_environment(parent, false)?;
        let result = (|| {
            for name in names {
                let value = self.binding_value(old, name)?;
                self.put_binding(new, name.clone(), Some(value), Declaration::Let, false)?;
            }
            Ok(new)
        })();
        if result.is_ok() {
            self.release_environment(old);
        } else {
            self.release_environment(new);
        }
        result
    }

    fn eval(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        self.check_native_evaluation_stack()?;
        let marker = self.publish_statement_inputs(env, this, None)?;
        self.eval_depth += 1;
        self.transient_roots.owned_eval_depth += 1;
        #[cfg(test)]
        let result = if self.recursive_evaluation_oracle {
            self.eval_inner(program, id, env, this, host)
        } else {
            self.eval_heap(program, id, env, this, host)
        };
        #[cfg(not(test))]
        let result = self.eval_heap(program, id, env, this, host);
        self.eval_depth -= 1;
        self.transient_roots.owned_eval_depth -= 1;
        self.restore_root_marker(marker);
        result.map_err(|error| error.at(&program.source, program.exprs[id].at))
    }

    // Retain the old recursive expression implementation only as a shallow
    // test oracle. Production compound expressions use small direct helpers.
    #[cfg(test)]
    fn eval_inner(
        &mut self,
        program: &Rc<Program>,
        mut id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // Selecting a conditional arm is a tail operation. Charge every arm,
        // but do not keep hundreds of native frames for a flat dispatcher.
        while let ExprKind::Conditional(condition, yes, no) = &program.exprs[id].kind {
            id = if self.eval(program, *condition, env, this, host)?.truthy() {
                *yes
            } else {
                *no
            };
            self.step()
                .map_err(|error| error.at(&program.source, program.exprs[id].at))?;
        }
        match &program.exprs[id].kind {
            ExprKind::Literal(value) => self.copy_value(value),
            ExprKind::TemplateLiteral(head, spans) => {
                self.eval_template(program, head, spans, env, this, host)
            }
            ExprKind::TemplateObject(site) => self.eval_template_object(program, site),
            ExprKind::RegExp(expression) => self.regexp_object(expression.clone()),
            ExprKind::Name(name) => match self.lookup(env, name)? {
                Some(scope) => self.read_binding(scope, name, host),
                None if self.global_exists(name, host)? => self.global_value(name, host),
                None => Err(Fault::exception(
                    "ReferenceError",
                    format!("{name} is not defined."),
                )),
            },
            ExprKind::This => self.context_this(this),
            ExprKind::NewTarget => self.new_target_value(),
            ExprKind::Function(template) => self.make_function(program, *template, env, this),
            ExprKind::Class(class) => self.make_class(program, *class, env, this, host),
            ExprKind::SuperCall(arguments) => self.eval_super_call(program, arguments, env, this, host),
            ExprKind::SuperMember(name) => {
                let reference = self.super_reference(name)?;
                self.read_reference(&reference, host)
            }
            ExprKind::Object(_) | ExprKind::Array(_) => {
                self.eval_collection(program, id, env, this, host)
            }
            ExprKind::Unary(_, _) => self.eval_unary(program, id, env, this, host),
            ExprKind::Yield { .. } => Err(Fault::new("Yield expression requires a generator resumption point.")),
            ExprKind::Binary(_, _, _) => self.eval_binary(program, id, env, this, host),
            ExprKind::Sequence(first, last) => {
                self.eval(program, *first, env, this, host)?;
                self.eval(program, *last, env, this, host)
            }
            ExprKind::Conditional(..) => unreachable!(),
            ExprKind::Assign(_, _, _) | ExprKind::Update(_, _, _) => {
                self.eval_assignment(program, id, env, this, host)
            }
            ExprKind::Member(object, property)
                if matches!(&program.exprs[*object].kind, ExprKind::Name(_))
                    && matches!(
                        &program.exprs[*property].kind,
                        ExprKind::Literal(Value::String(key)) if key == "length"
                    ) =>
            {
                self.eval_binding_length(program, *object, *property, env, this, host)
            }
            ExprKind::Member(_, _) => {
                let reference = self.reference(program, id, env, this, host)?;
                self.read_reference(&reference, host)
            }
            ExprKind::Call(_, _) => self.eval_call(program, id, env, this, host),
            ExprKind::OptionalChain(_, _) => self.eval_optional_chain(program, id, env, this, host),
            ExprKind::Construct(_, _) => self.eval_construct(program, id, env, this, host),
            ExprKind::Pattern(_) => Err(Fault::new("A destructuring target is not an expression.")),
        }
        .map_err(|error| error.at(&program.source, program.exprs[id].at))
    }

    #[cfg(test)]
    fn eval_collection(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match &program.exprs[id].kind {
            ExprKind::Object(properties) => {
                self.eval_object_literal(program, properties, env, this, host)
            }
            ExprKind::Array(elements) => {
                self.eval_array_literal(program, elements, env, this, host)
            }
            _ => unreachable!(),
        }
    }

    #[inline(never)]
    fn eval_object_literal(
        &mut self,
        program: &Rc<Program>,
        properties: &[object_literals::Member],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let object = self.new_object(None)?;
        self.eval_object_literal_members(program, object, properties, env, this, host)?;
        Ok(Value::Object(object))
    }

    fn eval_object_literal_members(
        &mut self,
        program: &Rc<Program>,
        object: usize,
        properties: &[object_literals::Member],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        for member in properties {
            match member {
                object_literals::Member::Property { key, value, kind } => {
                    let key = self.eval(program, *key, env, this, host)?;
                    let key = self.property_key(key, host)?;
                    let expression = *value;
                    let value = self.eval(program, *value, env, this, host)?;
                    if *kind == object_literals::Kind::Data {
                        self.infer_class_property_name(program, expression, &key, &value)?;
                    }
                    self.define_literal_member(object, key, value, *kind)?;
                }
                object_literals::Member::Prototype(expression) => {
                    let value = self.eval(program, *expression, env, this, host)?;
                    self.define_literal_prototype(object, &value)?;
                }
                object_literals::Member::Spread(expression) => {
                    let source = self.eval(program, *expression, env, this, host)?;
                    self.copy_data_properties(object, &source, &HashSet::new(), host)?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn eval_unary(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match &program.exprs[id].kind {
            ExprKind::Unary(op, argument) => {
                if *op == "delete" {
                    if matches!(program.exprs[*argument].kind, ExprKind::OptionalChain(_, _)) {
                        return self.delete_optional_chain(program, *argument, env, this, host);
                    }
                    if matches!(
                        program.exprs[*argument].kind,
                        ExprKind::Name(_) | ExprKind::Member(_, _) | ExprKind::SuperMember(_)
                    ) {
                        let reference = self.reference(program, *argument, env, this, host)?;
                        return self.delete_reference(reference, host);
                    }
                    self.eval(program, *argument, env, this, host)?;
                    return Ok(Value::Bool(true));
                }
                if *op == "typeof" {
                    if let ExprKind::Name(name) = &program.exprs[*argument].kind {
                        if self.lookup(env, name)?.is_none() && !self.global_exists(name, host)? {
                            return Ok(Value::String("undefined".into()));
                        }
                    }
                }
                let value = self.eval(program, *argument, env, this, host)?;
                match *op {
                    "!" => Ok(Value::Bool(!value.truthy())),
                    "void" => Ok(Value::Undefined),
                    "typeof" => Ok(Value::String(
                        match value {
                            Value::Undefined => "undefined",
                            Value::Null | Value::Object(_) | Value::Host(_) => "object",
                            Value::Bool(_) => "boolean",
                            Value::Number(_) => "number",
                            Value::BigInt(_) => "bigint",
                            Value::String(_) | Value::StringUnits(_) => "string",
                            Value::Symbol(_) => "symbol",
                            Value::Function(_) => "function",
                        }
                        .into(),
                    )),
                    "+" | "-" | "~" => {
                        let value = self.primitive_with_hint(value, false, host)?;
                        let n = value.script_number()?;
                        Ok(Value::Number(match *op {
                            "+" => n,
                            "-" => -n,
                            _ => f64::from(!int32(n)),
                        }))
                    }
                    _ => Err(Fault::new("Unsupported unary operator.")),
                }
            }
            _ => unreachable!(),
        }
    }

    #[cfg(test)]
    fn eval_binary(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match &program.exprs[id].kind {
            ExprKind::Binary(op, left, right) => {
                let left = self.eval(program, *left, env, this, host)?;
                if logical_short_circuits(op, &left) {
                    return Ok(left);
                }
                let right = self.eval(program, *right, env, this, host)?;
                if matches!(*op, "&&" | "||" | "??") {
                    Ok(right)
                } else if *op == "in" {
                    self.has_property_value(left, right, host)
                } else if *op == "instanceof" {
                    self.instance_of(left, right, host)
                } else {
                    self.binary_value(op, left, right, host)
                }
            }
            _ => unreachable!(),
        }
    }

    #[cfg(test)]
    fn eval_assignment(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match &program.exprs[id].kind {
            ExprKind::Assign(op, left, right) => {
                if let ExprKind::Pattern(pattern) = program.exprs[*left].kind {
                    let value = self.eval(program, *right, env, this, host)?;
                    let result = self.copy_value(&value)?;
                    self.initialize_pattern(
                        program,
                        pattern,
                        value,
                        binding_patterns::Mode::Assign,
                        env,
                        this,
                        host,
                    )?;
                    return Ok(result);
                }
                let logical = matches!(*op, "&&=" | "||=" | "??=");
                let reference = self.reference(program, *left, env, this, host)?;
                let reference = if logical || *op == "**=" {
                    self.resolved_operator_reference(reference, host)?
                } else {
                    reference
                };
                let before = if *op == "=" {
                    None
                } else {
                    let before = self.read_reference(&reference, host)?;
                    if (*op == "&&=" && !before.truthy())
                        || (*op == "||=" && before.truthy())
                        || (*op == "??=" && !matches!(before, Value::Null | Value::Undefined))
                    {
                        return Ok(before);
                    }
                    (!logical).then_some(before)
                };
                let mut value = self.eval(program, *right, env, this, host)?;
                if *op == "=" {
                    if let ExprKind::Name(name) = &program.exprs[*left].kind {
                        self.infer_class_name(program, *right, name, &value)?;
                    }
                }
                if logical {
                    self.name_logical_assignment_result(program, *left, *right, &value)?;
                }
                if let Some(before) = before {
                    value = self.binary_value(&op[..op.len() - 1], before, value, host)?;
                }
                let result = self.copy_value(&value)?;
                self.write_reference(reference, value, host)?;
                Ok(result)
            }
            ExprKind::Update(target, increase, prefix) => {
                let reference = self.reference(program, *target, env, this, host)?;
                let previous = self.read_reference(&reference, host)?;
                let previous = self.primitive_with_hint(previous, false, host)?;
                let before = previous.script_number()?;
                let after = before + if *increase { 1.0 } else { -1.0 };
                self.write_reference(reference, Value::Number(after), host)?;
                Ok(Value::Number(if *prefix { after } else { before }))
            }
            _ => unreachable!(),
        }
    }

    // Dispatch without retaining member-resolution scratch in every plain
    // nested call. Argument evaluation can recurse up to MAX_EVAL_DEPTH even
    // when no JavaScript function body has been entered yet.
    #[cfg(test)]
    fn eval_call(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let ExprKind::Call(callee, arguments) = &program.exprs[id].kind else {
            unreachable!()
        };
        match &program.exprs[*callee].kind {
            ExprKind::SuperMember(name) => {
                self.eval_super_member_call(program, name, arguments, env, this, host)
            }
            ExprKind::OptionalChain(_, _) => {
                self.call_parenthesized_chain(program, *callee, arguments, env, this, host)
            }
            ExprKind::Member(object, property) => {
                self.eval_member_call(program, *object, *property, arguments, env, this, host)
            }
            _ => self.eval_value_call(program, *callee, arguments, env, this, host),
        }
    }

    #[inline(never)]
    #[cfg(test)]
    fn eval_value_call(
        &mut self,
        program: &Rc<Program>,
        callee: ExprId,
        arguments: &[Argument],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let function = self.eval(program, callee, env, this, host)?;
        let args = self.eval_argument_list(program, arguments, env, this, host)?;
        let direct = matches!(&program.exprs[callee].kind, ExprKind::Name(name) if name == "eval");
        self.invoke_plain_call(function, &args, direct.then_some((env, this)), host)
    }

    // Resolve the receiver/key/callee completely before descending into the
    // arguments, then retain only the resolved values. In particular a getter
    // or argument may replace the original method or its receiver binding.
    #[inline(never)]
    #[cfg(test)]
    fn eval_member_call(
        &mut self,
        program: &Rc<Program>,
        object: ExprId,
        property: ExprId,
        arguments: &[Argument],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if let Some(result) = self.try_binding_character(program, object, property, arguments, env, host) {
            return result;
        }
        let (object, property, function) =
            self.prepare_member_call(program, object, property, env, this, host)?;
        let args = self.eval_argument_list(program, arguments, env, this, host)?;
        self.invoke_member_call(object, property, function, &args, host)
    }

    #[inline(never)]
    #[cfg(test)]
    fn prepare_member_call(
        &mut self,
        program: &Rc<Program>,
        object: ExprId,
        property: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<(Value, PropertyKey, Option<Value>), Fault> {
        let object = self.eval(program, object, env, this, host)?;
        let property = self.eval(program, property, env, this, host)?;
        self.resolve_member_call(object, property, host)
    }

    // Callers have already evaluated the receiver and key, in that order.
    #[inline(never)]
    fn resolve_member_call(
        &mut self,
        object: Value,
        property: Value,
        host: &mut dyn Host,
    ) -> Result<(Value, PropertyKey, Option<Value>), Fault> {
        Self::require_property_receiver(&object)?;
        let property = self.property_key(property, host)?;
        let function = match &property {
            PropertyKey::StringUnits(units) => Some(self.read_units_property(&object, units, host)?),
            PropertyKey::Symbol(key) => {
                let receiver = self.copy_value(&object)?;
                Some(self.get_symbol_property(receiver, *key, host)?)
            }
            PropertyKey::String(property) => {
                if let Some(ordinary) = Ordinary::from_value(&object) {
                    Some(self.get_ordinary_property_with_host(ordinary, property, host)?)
                } else if let Value::Host(handle) = &object {
                    if self.foreign_window(*handle, host)
                        || self.host_ordinary(*handle, property, host)?.is_some()
                        || host.ordinary_prototype(*handle)
                        || host.ordinary_lookup_tail_property(*handle,property)
                        || host.resolve_member_before_call(*handle)
                    {
                        Some(self.host_read(*handle, property, host)?)
                    } else if self.is_browser_global(*handle)
                        && !self.host_owns_global(property, host)
                    {
                        Some(self.global_value(property, host)?)
                    } else {
                        None
                    }
                } else {
                    Some(self.read_property(&object, property, host)?)
                }
            }
        };
        Ok((object, property, function))
    }

    // Invocation scratch is needed only after the argument expressions have
    // completed; do not reserve it in every suspended argument-evaluation frame.
    #[inline(never)]
    fn invoke_member_call(
        &mut self,
        object: Value,
        property: PropertyKey,
        function: Option<Value>,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match (property, function) {
            (PropertyKey::String(property), Some(function)) => {
                self.call_named_value(function, args, object, &property, host)
            }
            (PropertyKey::String(property), None) => {
                self.call_member(object, &property, args, host)
            }
            (PropertyKey::Symbol(_) | PropertyKey::StringUnits(_), Some(function)) => {
                self.call_value(function, args, object, host)
            }
            (PropertyKey::Symbol(_) | PropertyKey::StringUnits(_), None) => unreachable!(),
        }
    }

    #[cfg(test)]
    fn eval_construct(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let ExprKind::Construct(constructor, arguments) = &program.exprs[id].kind else {
            unreachable!()
        };
        // Resolve the constructor before arguments, then test constructability.
        // Even a non-constructor's argument expressions must run in order.
        let constructor = self.eval(program, *constructor, env, this, host)?;
        let args = self.eval_argument_list(program, arguments, env, this, host)?;
        self.construct_value(constructor, &args, host)
    }

    // Keep constructor-only scratch out of ordinary call dispatch on the
    // unchanged one-MiB unoptimized native-stack regressions.
    #[inline(never)]
    fn construct_value(&mut self, constructor: Value, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        let new_target = match &constructor { Value::Function(id) => Some(*id), _ => None };
        self.construct_value_with_target(constructor, args, new_target, host)
    }

    #[inline(never)]
    fn construct_value_with_target(&mut self, constructor: Value, args: &[Value], new_target: Option<usize>, host: &mut dyn Host) -> Result<Value, Fault> {
        let marker = self.publish_call_inputs(&constructor, &Value::Undefined, args)?;
        if let Some(target) = new_target {
            if let Err(error) = self.push_transient_root(function_reachability::Edge::Function(target)) {
                self.restore_transient_marker(marker);
                return Err(error);
            }
        }
        if !matches!(&constructor, Value::Function(id) if matches!(self.functions.get(*id), Some(Function::Script { .. } | Function::Bound(_)))) {
            if self.transient_roots.inhibited == 0 {
                self.transient_roots.outer_native = match constructor { Value::Function(id) => Some(id), _ => None };
            }
            self.inhibit_transient_collection_for("constructor");
        }
        let result = self.construct_value_routed(constructor, args, new_target, host);
        self.restore_transient_marker(marker);
        result
    }

    fn construct_value_routed(&mut self, constructor: Value, args: &[Value], new_target: Option<usize>, host: &mut dyn Host) -> Result<Value, Fault> {
        if self.changes_native_access_caller(&constructor) {
            return self.construct_with_native_access_caller(constructor,args,new_target,host);
        }
        let realm = match &constructor { Value::Function(id) => self.function_realms.get(*id).copied().unwrap_or(self.active_realm), _ => self.active_realm };
        if realm == self.active_realm { return self.construct_value_in_realm(constructor, args, new_target, host); }
        if !self.from_constructor(&constructor)? {
            self.step()?;
            return Err(Fault::exception("TypeError", "The value is not a constructor."));
        }
        self.with_intrinsic_realm(realm, |runtime| runtime.construct_value_in_realm(constructor, args, new_target, host))
    }

    #[inline(never)]
    fn construct_with_native_access_caller(&mut self,constructor:Value,args:&[Value],new_target:Option<usize>,host:&mut dyn Host)->Result<Value,Fault> {
        let previous=self.native_access_caller;
        self.native_access_caller=if matches!(&constructor,Value::Function(id) if matches!(self.functions.get(*id),Some(Function::Script {..}))) {None}
            else {Some(self.browser_global.map(|(window,_)|window))};
        let result=self.construct_value_routed(constructor,args,new_target,host);
        self.native_access_caller=previous;
        result
    }

    fn construct_value_in_realm(
        &mut self,
        constructor: Value,
        args: &[Value],
        new_target: Option<usize>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        let Value::Function(index) = constructor else {
            return Err(Fault::exception(
                "TypeError",
                "The value is not a constructor.",
            ));
        };
        match self.functions.get(index) {
            Some(Function::Bound(_)) => {
                let (target, arguments, _) = self.bound_arguments(index, args, false)?;
                let new_target = if new_target == Some(index) { Some(target) } else { new_target };
                return self.construct_value_with_target(Value::Function(target), &arguments, new_target, host);
            }
            Some(Function::Native(Builtin::Proxy)) => self.new_ordinary_proxy(
                args.first().unwrap_or(&Value::Undefined),
                args.get(1).unwrap_or(&Value::Undefined),
            ),
            Some(Function::Native(Builtin::Date(date::Op::Constructor))) => {
                self.date_construct(args, host)
            }
            Some(Function::Native(Builtin::Response(response_api::Op::Constructor))) => {
                self.response_construct(args,host)
            }
            Some(Function::Native(Builtin::Headers(headers_api::Op::Constructor))) => {
                self.headers_construct(args, host)
            }
            Some(Function::Native(Builtin::Url(url_api::Op::Constructor(kind)))) => {
                self.url_construct(*kind, args, host)
            }
            Some(Function::Native(Builtin::Rect(rect_api::Op::Constructor(mutable)))) => {
                self.rect_construct(*mutable, args, host)
            }
            Some(Function::Native(Builtin::ImageData(image_data_api::Op::Constructor))) => {
                self.image_data_construct(args, host)
            }
            Some(Function::Native(Builtin::DomException(dom_exception::Op::Constructor))) => {
                self.dom_exception_construct(args,host)
            }
            Some(Function::Native(Builtin::MutationObserver(mutation_observers::Op::Constructor))) => {
                self.construct_mutation_observer(args, new_target, host)
            }
            Some(Function::Native(Builtin::Promise(promises::Op::Constructor))) => {
                self.construct_promise(args, host)
            }
            Some(Function::Native(Builtin::Promise(promises::Op::AggregateError))) => {
                self.promise_builtin(promises::Op::AggregateError, args, Value::Undefined, host)
            }
            Some(Function::Native(Builtin::TextEncoding(text_encoding::Op::Constructor(
                encoder,
            )))) => self.construct_text_encoding(*encoder, args, host),
            Some(Function::Native(Builtin::Collection(collections::Op::Constructor(kind)))) => {
                self.construct_collection(*kind, args, new_target, host)
            }
            Some(Function::Native(Builtin::Binary(binary_data::Op::Constructor(kind)))) => {
                self.construct_binary(*kind, args, host)
            }
            Some(Function::Native(Builtin::RegExp)) => {
                if matches!(args.get(1), None | Some(Value::Undefined)) {
                    if let Some(Value::Object(id)) = args.first() {
                        if let Some(expression) = self.object(*id)?.regexp.clone() {
                            return self.regexp_object(expression);
                        }
                    }
                }
                self.regexp_factory(args)
            }
            Some(Function::Native(Builtin::Error(kind))) => {
                let kind = *kind;
                let bytes = args.first().map_or(0, Value::bytes);
                self.work_as(bytes.saturating_mul(2), WorkKind::NativeInput)?;
                self.construct_error(kind, args, new_target, host)
            }
            Some(Function::Native(builtin @ (Builtin::Object | Builtin::Array))) => self.builtin(*builtin, args, host),
            Some(Function::Native(builtin @ (Builtin::Number | Builtin::Boolean))) => {
                let value = self.builtin(*builtin, args, host)?;
                self.box_primitive(value)
            }
            Some(Function::Native(Builtin::String)) => self.construct_string(args, host),
            Some(Function::Script {
                program, template, ..
            }) if program.templates[*template].constructible => {
                if program.templates[*template].derived_constructor {
                    // A derived constructor owns an uninitialized binding, not
                    // a receiver allocated before super selects its real base.
                    return self.call_constructed_script(index, args, Value::Undefined, new_target, host);
                }
                let object = self.script_constructor_receiver(new_target.unwrap_or(index), host)?;
                let result =
                    self.call_constructed_script(index, args, Value::Object(object), new_target, host)?;
                Ok(
                    if matches!(
                        result,
                        Value::Object(_) | Value::Function(_) | Value::Host(_)
                    ) {
                        result
                    } else {
                        Value::Object(object)
                    },
                )
            }
            Some(Function::Native(Builtin::Function)) => self.function_constructor(args, host),
            Some(Function::Native(Builtin::AsyncFunction)) => self.async_function_constructor(args, new_target, host),
            Some(Function::Native(Builtin::GeneratorFunction)) => {
                self.dynamic_function_constructor(args, FunctionKind::Generator, new_target, host)
            }
            Some(Function::Host {
                object,
                method,
                constructible: true,
                ..
            }) => {
                let (object, method) = (*object, *method);
                self.call_host(object, method, args, host)
            }
            Some(Function::Host { .. }) => {
                Err(Fault::exception("TypeError", "The native method is not a constructor."))
            }
            Some(Function::Vacant) | None => Err(Fault::new("Invalid function handle.")),
            Some(_) => Err(Fault::exception(
                "TypeError",
                "The value is not a constructor.",
            )),
        }
    }

    fn builtin_instance_prototype(
        &mut self, new_target: Option<usize>, intrinsic: impl FnOnce(&Self) -> Option<Ordinary>,
        host: &mut dyn Host,
    ) -> Result<Option<Ordinary>, Fault> {
        let Some(target) = new_target else { return Ok(intrinsic(self)); };
        let prototype = self.get_ordinary_property_with_host(Ordinary::Function(target), "prototype", host)?;
        if let Some(prototype) = Ordinary::from_value(&prototype) {
            return Ok(Some(prototype));
        }
        if matches!(prototype, Value::Host(_)) {
            return Err(Fault::new("Native host object class prototypes are not implemented."));
        }
        // A primitive prototype selects the corresponding intrinsic of
        // newTarget's realm, which can differ from the builtin's realm.
        self.with_intrinsic_realm(self.function_realms[target], |runtime| Ok(intrinsic(runtime)))
    }

    fn script_constructor_receiver(&mut self, constructor: usize, host: &mut dyn Host) -> Result<usize, Fault> {
        // The prototype snapshot/new receiver are unpublished until linked.
        // Restore before call_value immediately publishes the complete receiver.
        let marker = self.inhibit_transient_collection_for("constructor-setup");
        let result = (|| {
            let prototype = self.get_ordinary_property_with_host(Ordinary::Function(constructor), "prototype", host)?;
            if matches!(prototype, Value::Host(_)) {
                return Err(Fault::new("Native host object prototypes are not implemented."));
            }
            let object = self.new_object(None)?;
            if let Some(prototype) = Ordinary::from_value(&prototype) {
                self.objects[object].prototype = Some(prototype);
            }
            Ok(object)
        })();
        self.restore_transient_marker(marker);
        result
    }

    fn reference(
        &mut self,
        program: &Rc<Program>,
        id: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Reference, Fault> {
        // Pattern/loop LHS evaluation keeps the base while computing its key.
        let marker = self.inhibit_transient_collection();
        let result = self.reference_unpublished(program, id, env, this, host);
        self.restore_transient_marker(marker);
        result
    }

    fn reference_unpublished(
        &mut self,
        program: &Rc<Program>,
        expression: ExprId,
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Reference, Fault> {
        self.step()?;
        match &program.exprs[expression].kind {
            ExprKind::Name(name) => Ok(match self.lookup(env, name)? {
                Some(scope) => Reference::Binding(scope, name.clone()),
                None if self.global_exists(name, host)? => Reference::Global(name.clone()),
                None => Reference::Missing(name.clone()),
            }),
            ExprKind::Member(object, property) => {
                let object = self.eval(program, *object, env, this, host)?;
                let property = self.eval(program, *property, env, this, host)?;
                Ok(Reference::ComputedProperty(object, property))
            }
            ExprKind::SuperMember(name) => self.super_reference(name),
            _ => Err(Fault::new("Invalid assignment reference.")),
        }
    }

    fn read_reference(
        &mut self,
        reference: &Reference,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if let Reference::Property(object, property) = reference {
            return self.read_property(object, property, host);
        }
        self.step()?;
        match reference {
            Reference::ComputedProperty(object, key) => {
                Self::require_property_receiver(object)?;
                let key = self.copy_value(key)?;
                let key = self.property_key(key, host)?;
                match key {
                    PropertyKey::String(name) => self.read_property(object, &name, host),
                    PropertyKey::StringUnits(units) => self.read_units_property(object, &units, host),
                    PropertyKey::Symbol(key) => {
                        let object = self.copy_value(object)?;
                        self.read_reference(&Reference::SymbolProperty(object, key), host)
                    }
                }
            }
            Reference::SymbolProperty(object, key) => {
                self.reject_foreign_window(&object, host)?;
                self.get_symbol_property(object.clone(), *key, host)
            }
            Reference::UnitsProperty(object, units) => self.read_units_property(object, units, host),
            Reference::Binding(env, name) => self.read_binding(*env, name, host),
            Reference::Global(name) => self.global_value(name, host),
            Reference::SuperProperty { base, receiver, name } => self.read_super_property(*base, receiver, name, host),
            Reference::Missing(name) => Err(Fault::exception(
                "ReferenceError",
                format!("{name} is not defined."),
            )),
            Reference::Property(_, _) => unreachable!(),
        }
    }

    // The reference/call target already owns the receiver. Borrow it during
    // lookup; only a selected accessor needs a separately owned receiver.
    // This retains the step previously charged by a resolved Property read.
    fn read_property(
        &mut self,
        object: &Value,
        property: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        match object {
            Value::Host(object) => {
                if self.is_browser_global(*object) {
                    return self.global_value(property, host);
                }
                self.host_read(*object, property, host)
            }
            Value::Object(object) => self.get_property_with_host(*object, property, host),
            Value::Function(function) => {
                self.get_ordinary_property_with_host(Ordinary::Function(*function), property, host)
            }
            Value::StringUnits(units) if property == "length" => Ok(Value::Number(units.len() as f64)),
            Value::StringUnits(units) if property_index(property).is_some_and(|i| i < units.len()) => {
                let index = property_index(property).unwrap();
                self.string_from_units(&units[index..=index])
            }
            Value::String(text) if property == "length" => {
                self.work_as(text.len(), WorkKind::LengthScan)?;
                Ok(Value::Number(text.encode_utf16().count() as f64))
            }
            Value::String(text) if property_index(property).is_some() => {
                let target = property_index(property).unwrap();
                if let Some((character, index)) = self.string_scalar_at(text, target)? {
                    let mut buffer = [0; 2];
                    let units = character.encode_utf16(&mut buffer);
                    self.string_from_units(&units[index..index + 1])
                } else {
                    // An absent own index may have an inherited getter. Use
                    // the original, already evaluated receiver, not a live
                    // binding or an empty-string sentinel.
                    self.get_ordinary_property_with_receiver(
                        Ordinary::Object(self.string_prototype.unwrap()),
                        property,
                        object,
                        host,
                    )
                }
            }
            Value::String(_) | Value::StringUnits(_) => self.get_ordinary_property_with_receiver(
                Ordinary::Object(self.string_prototype.unwrap()),
                property,
                object,
                host,
            ),
            Value::Number(_) => self.get_ordinary_property_with_receiver(
                Ordinary::Object(self.number_prototype.unwrap()),
                property,
                object,
                host,
            ),
            Value::BigInt(_) => self.get_ordinary_property_with_receiver(
                Ordinary::Object(self.big_int_prototype.unwrap()), property, object, host,
            ),
            Value::Bool(_) => self.get_ordinary_property_with_receiver(
                Ordinary::Object(self.boolean_prototype.unwrap()),
                property,
                object,
                host,
            ),
            Value::Symbol(id) => self.symbol_string_property(*id, property, host),
            _ => Ok(Value::Undefined),
        }
    }

    fn write_reference(
        &mut self,
        reference: Reference,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.step()?;
        match reference {
            Reference::ComputedProperty(object, key) => {
                Self::require_property_receiver(&object)?;
                let key = self.property_key(key, host)?;
                self.write_reference(key.reference(object), value, host)
            }
            Reference::SymbolProperty(object, key) => self.assign_symbol_property(object, key, value, host),
            Reference::Binding(env, name) => self.write_binding(env, &name, value, false, host),
            Reference::Global(name) => self.set_global(name, value, host),
            Reference::SuperProperty { base, receiver, name } => self.write_super_property(base, receiver, name, value, host),
            Reference::Missing(name) if self.is_operation_global(&name) => {
                if self.strict {
                    Err(Fault::exception("ReferenceError", format!("{name} is not defined.")))
                } else {
                    // Only a host-selected replaceable operation may be recreated
                    // this way. The general implicit-global policy is unchanged.
                    self.set_global(name, value, host)
                }
            }
            Reference::Missing(name) => {
                if self.strict {
                    Err(Fault::exception("ReferenceError", format!("{name} is not defined.")))
                } else {
                    Err(Fault::new(format!(
                        "{name} is not defined; implicit globals are not supported."
                    )))
                }
            }
            Reference::Property(Value::Host(object), property) => {
                if self.is_browser_global(object) { return self.set_global(property, value, host); }
                self.host_write(object, property, value, host)
            }
            Reference::UnitsProperty(receiver, units) => self.assign_units_property(receiver, units, value, host),
            Reference::Property(Value::Object(object), property) => self.assign_ordinary_property(Ordinary::Object(object), property, value, host),
            Reference::Property(Value::Function(function), property) => self.assign_ordinary_property(Ordinary::Function(function), property, value, host),
            Reference::Property(receiver @ (Value::String(_) | Value::StringUnits(_) | Value::Number(_) | Value::BigInt(_) | Value::Bool(_) | Value::Symbol(_)), property) => self.assign_primitive_property(receiver, property, value, host),
            Reference::Property(_, _) => Err(Fault::new(
                "Properties can only be assigned on ordinary objects, arrays or supported host objects.",
            )),
        }
    }

    fn delete_reference(
        &mut self,
        reference: Reference,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if let Reference::ComputedProperty(object, key) = reference {
            Self::require_property_receiver(&object)?;
            let key = self.property_key(key, host)?;
            return self.delete_reference(key.reference(object), host);
        }
        let deleted = match reference {
            Reference::SuperProperty { .. } => return Err(Fault::exception("ReferenceError", "A super property cannot be deleted.")),
            Reference::ComputedProperty(_, _) => unreachable!(),
            Reference::UnitsProperty(object, units) => {
                Self::require_property_receiver(&object)?;
                self.property_name_units(&units)?;
                if let Some(backing) = self.symbol_backing(&object, host)? {
                    let id = self.ordinary_id(backing)?;
                    self.reject_unsupported_proxy_operation(id, "deleteProperty")?;
                    if self.objects[id].window_named_properties || self.objects[id].properties.get_units(&units)
                        .is_some_and(|property| !property.configurable) {
                        false
                    } else {
                        self.remove_units_property(id, &units)?;
                        true
                    }
                } else if matches!(object, Value::Host(_)) {
                    return Err(Fault::new("UTF-16 properties of this native object are not implemented."));
                } else { true }
            }
            Reference::SymbolProperty(object, key) => {
                Self::require_property_receiver(&object)?;
                if let Some(backing) = self.symbol_backing(&object, host)? {
                    let id = self.ordinary_id(backing)?;
                    self.reject_unsupported_proxy_operation(id, "deleteProperty")?;
                    if self.objects[id].window_named_properties || self.objects[id]
                        .symbol_properties
                        .get(&key)
                        .is_some_and(|property| !property.configurable)
                    {
                        false
                    } else {
                        self.remove_symbol_property(id, key)?;
                        true
                    }
                } else {
                    true
                }
            }
            Reference::Missing(_) => true,
            Reference::Binding(environment, name)
                if environment == self.global_environment && self.browser_global.is_some() && self.global_attributes.contains_key(&name) =>
            {
                self.delete_global(&name, host)?
            }
            Reference::Binding(_, _) => false,
            Reference::Global(name) => self.delete_global(&name, host)?,
            Reference::Property(Value::Host(handle), name) if self.is_browser_global(handle) => {
                self.delete_global(&name, host)?
            }
            Reference::Property(Value::Host(handle), name) => {
                self.reject_foreign_window(&Value::Host(handle), host)?;
                if let Some(object) = self.host_ordinary(handle, &name, host)? {
                    return self.delete_reference(Reference::Property(object.value(), name), host);
                }
                host.delete(handle, &name).map_err(Fault::new)?
            }
            Reference::Property(Value::Object(id), name) => {
                self.property_name(&name)?;
                self.reject_unsupported_proxy_operation(id, "deleteProperty")?;
                let object = self.object(id)?;
                if object.window_named_properties || self
                    .typed_numeric_key(id, &name)?
                    .is_some_and(|index| self.typed_index_valid(id, index))
                    || self.string_object_index(id, &name).is_some()
                    || name == "length" && object.length.is_some()
                    || object
                        .properties
                        .get(&name)
                        .is_some_and(|property| !property.configurable)
                {
                    false
                } else {
                    self.remove_property(id, &name)?;
                    true
                }
            }
            Reference::Property(Value::Function(function), name) => {
                self.property_name(&name)?;
                let id = self.ordinary_id(Ordinary::Function(function))?;
                if self.objects[id]
                    .properties
                    .get(&name)
                    .is_some_and(|property| !property.configurable)
                {
                    false
                } else {
                    self.remove_property(id, &name)?;
                    true
                }
            }
            Reference::Property(receiver @ (Value::String(_) | Value::StringUnits(_)), name) => {
                self.work(receiver.bytes().saturating_add(name.len()))?;
                let index = name
                    .parse::<usize>()
                    .ok()
                    .filter(|index| index.to_string() == name);
                name != "length" && !index.is_some_and(|index| receiver.string_length().is_some_and(|length| index < length))
            }
            Reference::Property(Value::Number(_) | Value::BigInt(_) | Value::Bool(_) | Value::Symbol(_), _) => true,
            Reference::Property(_, _) => {
                return Err(Fault::new(
                    "Deleting native host properties is not implemented.",
                ))
            }
        };
        if !deleted && self.strict {
            Err(Fault::exception(
                "TypeError",
                "The property cannot be deleted.",
            ))
        } else {
            Ok(Value::Bool(deleted))
        }
    }

    fn delete_global(&mut self, name: &str, host: &dyn Host) -> Result<bool, Fault> {
        self.ensure_global_intrinsics(name)?;
        self.property_name(name)?;
        if self.host_owns_global(name, host) || self.host_globals.contains(name) {
            return Ok(false);
        }
        if self.global_binding(name) {
            if !self
                .global_attributes
                .get(name)
                .is_some_and(|attrs| attrs.2)
            {
                return Ok(false);
            }
            let binding = self.environment_mut(self.global_environment).bindings.remove(name).unwrap();
            self.bindings -= 1;
            self.value_bytes -= binding.value.as_ref().map_or(0, Value::bytes);
            self.global_attributes.remove(name);
            return Ok(true);
        }
        let (_, properties) = self.browser_global.expect("bound browser global");
        if self.objects[properties]
            .properties
            .get(name)
            .is_some_and(|property| !property.configurable)
        {
            return Ok(false);
        }
        self.remove_property(properties, name)?;
        Ok(true)
    }

    fn instance_of(
        &mut self,
        value: Value,
        constructor: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        let Value::Function(mut id) = constructor else {
            return Err(Fault::exception(
                "TypeError",
                "The right side of instanceof must be callable.",
            ));
        };
        // Bound functions delegate instanceof to their original target. Walk
        // the arena links without adding native stack frames for each binding.
        while let Some(Function::Bound(bound)) = self.functions.get(id) {
            id = bound.target;
            self.step()?;
        }
        if !matches!(
            value,
            Value::Object(_) | Value::Function(_) | Value::Host(_)
        ) {
            return Ok(Value::Bool(false));
        }
        if let (
            Value::Host(handle),
            Some(Function::Host {
                object,
                method,
                constructible: true,
                ..
            }),
        ) = (&value, self.functions.get(id))
        {
            if host.instance_ancestry(*handle).is_none() {
                if let Some(matches) = host.is_instance(*handle, *object, method) {
                    return Ok(Value::Bool(matches));
                }
            }
        }
        let prototype =
            self.get_ordinary_property_with_host(Ordinary::Function(id), "prototype", host)?;
        if matches!(prototype, Value::Host(_)) {
            return Err(Fault::new(
                "Native host prototype checks are not implemented.",
            ));
        }
        let Some(prototype) = Ordinary::from_value(&prototype) else {
            return Err(Fault::exception(
                "TypeError",
                "This function has no object prototype for instanceof.",
            ));
        };
        if let Value::Host(handle) = value {
            if self.is_browser_global(handle) {
                // The browser global already has a realm-owned ordinary
                // property store. Its prototype ancestry is fixed; scripts
                // cannot change native Window prototypes through this store.
                let (_, properties) = self.browser_global.unwrap();
                return self
                    .inherits_from(Ordinary::Object(properties), prototype, host)
                    .map(Value::Bool);
            }
            if host.native_ancestry_inspection(handle) {
                let anchor=self.native_instance_visible_anchor(handle,host)?;
                return Ok(Value::Bool(anchor==prototype || self.inherits_from(anchor,prototype,host)?));
            }
            self.reject_foreign_window(&Value::Host(handle),host)?;
            if let Some(ancestry) = host.instance_ancestry(handle) {
                let caller = self.browser_global.map(|(window,_)|window);
                if caller != Some(ancestry.window) {
                    host.validate_window_access(caller,ancestry.window).map_err(Fault::new)?;
                }
                let anchor = self.native_ancestry_anchor(ancestry)?;
                return Ok(Value::Bool(anchor==prototype || self.inherits_from(anchor,prototype,host)?));
            }
            return Err(Fault::new(
                "Native host prototype checks are not implemented.",
            ));
        }
        let object = Ordinary::from_value(&value).unwrap();
        self.inherits_from(object, prototype, host).map(Value::Bool)
    }

    #[inline(never)]
    fn dispatch_native_event(&mut self, token: usize, host: &mut dyn Host) -> Result<bool, Fault> {
        let result = (|| {
            loop {
                self.step()?;
                let Some(callback) = host.next_event_callback(token).map_err(Fault::new)? else { return Ok(()); };
                // This is nested within the author's current job and realm.
                match self.call_event_listener(callback.callback, &[Value::Host(callback.event)], Value::Host(callback.target), host) {
                    Ok(value) => host.event_callback_result(token, matches!(value, Value::Bool(false))),
                    Err(fault) if fault.exception.is_some() => host.event_callback_error(token, &fault.display()),
                    Err(fault) => return Err(fault),
                }
            }
        })();
        let accepted = host.finish_event_dispatch(token);
        result.map(|()| accepted)
    }

    fn call_host(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // Dictionary conversion retains earlier getter results while invoking
        // later getters. Direct Host member calls do not enter call_value.
        let marker = self.inhibit_transient_collection();
        let result = self.call_host_unpublished(object, method, args, host);
        self.restore_transient_marker(marker);
        result
    }

    fn call_host_unpublished(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.validate_native_access(object, host)?;
        if method==DOCUMENT_GET_ELEMENTS_NAME {
            // A borrowed main-realm operation can create a child-owned list
            // before the child's constructors have ever been read. Initialize
            // its actual roots after origin validation, before publishing it.
            if let Some(owner)=host.native_access_owner(object){self.ensure_window_realm(owner,host)?;}
        }
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) {
            host.native_operation_caller(self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(w,_)|w)));
            if let Err((kind,message))=host.adjacent_receiver_access(object) {return Err(self.adjacent_fault(kind,&message));}
        }
        if matches!(method,NODE_VALUE_SET|"\0Element.set.className") {
            // A retained child may have been detached since this accessor was
            // saved. Resolve its current native owner before invoking toString.
            host.native_operation_caller(self.native_access_caller
                .unwrap_or_else(|| self.browser_global.map(|(window, _)| window)));
            if let Err((kind, message)) = host.validate_call(object, method, &[]) {
                return Err(Fault::exception(kind, message));
            }
        }
        if matches!(method,document_location_interface::GET|document_location_interface::SET) {
            return self.document_location_call(object,method,args,host);
        }
        if matches!(method,"\0Element.set.classList"|"\0HTMLOutputElement.set.htmlFor"|"\0HTMLAreaElement.set.relList"|"\0HTMLFormElement.set.relList") {
            let property=if method=="\0Element.set.classList"{"classList"}else if method=="\0HTMLOutputElement.set.htmlFor"{"htmlFor"}else{"relList"};
            // Web IDL PutForwards reads the receiver's current property before
            // forwarding the original value. Only the selected value setter
            // converts it. The enclosing native call inhibits transient
            // collection while the getter and setter can reenter script.
            host.native_operation_caller(self.native_access_caller
                .unwrap_or_else(||self.browser_global.map(|(window,_)|window)));
            host.validate_call(object,method,&[])
                .map_err(|(kind,message)|Fault::exception(kind,message))?;
            let target=self.read_property(&Value::Host(object),property,host)?;
            if !matches!(target,Value::Object(_)|Value::Function(_)|Value::Host(_)) {
                return Err(Fault::exception("TypeError","Token list forwarding requires an object."));
            }
            let value=self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
            // Set(Q, "value", V, false) ignores a false internal Set result,
            // even when this accessor was called from a strict author script.
            let previous_strict=std::mem::replace(&mut self.strict,false);
            let result=self.write_reference(Reference::Property(target,"value".into()),value,host);
            self.strict=previous_strict;
            result?;
            return Ok(Value::Undefined);
        }
        // Canonical attribute getters ignore arguments; setters use one value.
        let attribute_setter=html_reflection_interface::operation(method).map(|(_,setter)|setter)
            .or_else(||aria_reflection_interface::operation(method).map(|(_,_,setter)|setter));
        let args = if let Some(setter) = attribute_setter {
            if setter { &args[..args.len().min(1)] } else { &[][..] }
        } else {
            match method {
                adjacent_interface::TEXT | adjacent_interface::ELEMENT => &args[..args.len().min(2)],
                ELEMENT_STYLE_GET | SVG_STYLE_GET | NODE_VALUE_GET | NODE_HAS_CHILDREN | ELEMENT_REMOVE | CHARACTER_REMOVE | DOCTYPE_REMOVE => &[][..],
                ELEMENT_STYLE_SET | SVG_STYLE_SET | NODE_VALUE_SET => &args[..args.len().min(1)],
                _ => args,
            }
        };
        if let Some(property)=host.native_setter_property(object,method) {
            // Web IDL attribute setters receive undefined when no argument is
            // supplied. Brand/origin validation preceded any value conversion.
            let value=self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
            self.host_set_native(object,property,value,host)?;
            return Ok(Value::Undefined);
        }
        let required = host.required_arguments(object, method);
        if args.len() < required {
            return Err(Fault::exception(
                "TypeError",
                format!("{method} requires at least {required} argument(s)."),
            ));
        }
        let conversions = host.argument_conversions(object, method, args);
        let variadic = host.variadic_string_arguments(object, method);
        let mut converted;
        let args = if conversions.is_empty() && !variadic {
            args
        } else {
            converted = Vec::new();
            converted.try_reserve(args.len()).map_err(|_|Fault::new("Could not reserve converted native arguments."))?;
            for value in args {
                converted.push(self.copy_value(value)?);
            }
            for (index, string_hint) in conversions.iter().copied()
                .chain((0..if variadic {args.len()} else {0}).map(|index|(index,true))) {
                if index<converted.len() {
                    host.validate_argument_prefix(object, method, index, &converted)
                        .map_err(|(kind, message)| Fault::exception(kind, message))?;
                    let value=std::mem::replace(&mut converted[index],Value::Undefined);
                    if string_hint && host.nullable_string_argument(object, method, index)
                        && matches!(value, Value::Null | Value::Undefined)
                    {
                        converted[index] = Value::Null;
                        continue;
                    }
                    let primitive = self.primitive_with_hint(value, string_hint, host)?;
                    self.work(primitive.bytes())?;
                    converted[index] = self.checked_value(if string_hint {
                        if host.preserves_dom_string_argument(object,method,index)&&primitive.is_string(){primitive}
                        else{Value::String(primitive.script_text()?)}
                    } else {
                        let number = primitive.script_number()?;
                        Value::Number(if method == canvas_readback::METHOD {
                            canvas_readback::enforce_long(number)?
                        } else {
                            number
                        })
                    })?;
                }
            }
            &converted
        };
        // Readback consumes converted arguments inside this transient call. Its
        // native pixel source receives no script object or function handles.
        if method != canvas_readback::METHOD {
            for value in args { self.pin_exported_value(value); }
        }
        for &index in host.native_reference_arguments(object,method) {
            if let Some(Value::Host(handle))=args.get(index) {self.validate_native_access(*handle,host)?;}
        }
        if host.synchronous_event_dispatch(object,method) {host.prepare_event_dispatch().map_err(Fault::new)?;}
        // Conversion may run author code in a different realm. Preflight
        // checks must see this call's incumbent, even after a refused call.
        host.native_operation_caller(self.native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(window, _)| window)));
        if range_interface::operation(method) {
            if let Err(error)=host.prepare_range_operation(object,method,args) {
                return Err(match error {
                    range_interface::Error::Fatal(message)=>Fault::new(message),
                    range_interface::Error::Exception("TypeError",message)=>Fault::exception("TypeError",message),
                    range_interface::Error::Exception(kind,message)=>Fault::thrown(self.native_dom_exception(kind,&message)?),
                });
            }
        }
        if let Err((kind,message))=host.validate_call(object,method,args) {
            if (method=="\0CharacterData.substringData"||character_data_interface::operation(method).is_some()||method==text_split_interface::METHOD)&&kind=="IndexSizeError"{return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));}
            if dom_exception::factory_validation(method, kind) {
                return Err(Fault::thrown(self.native_dom_exception(kind, &message)?));
            }
            if method==DOCUMENT_IMPORT_NODE&&kind=="NotSupportedError"{return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));}
            if matches!(method,"setAttributeNode"|"setAttributeNodeNS"|"removeAttributeNode")&&matches!(kind,"InUseAttributeError"|"NotFoundError"|"WrongDocumentError"|"NotSupportedError"){return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));}
            if method==DOCUMENT_ADOPT_NODE&&matches!(kind,"NotSupportedError"|"HierarchyRequestError"){return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));}
            if matches!(method,"appendChild"|"insertBefore"|"removeChild"|"replaceChild")&&matches!(kind,"HierarchyRequestError"|"NotFoundError"|"NotSupportedError"|"WrongDocumentError"){return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));}
            if method.starts_with("\0XHR.")&&!matches!(kind,"TypeError"|"RangeError") {let exception=self.native_dom_exception(kind,&message)?;return Err(Fault::thrown(exception));}
            if matches!(method,"setAttribute"|"setAttributeNS")&&matches!(kind,"NamespaceError"|"InvalidCharacterError"){return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));}
            if kind == "NotSupportedError"
                && matches!(method, DOCUMENT_IMPORT_NODE | HTML_FORM_SUBMIT | HTML_FORM_RESET | TEXTAREA_VALUE_GET | TEXTAREA_VALUE_SET | "\0Media.load" | "\0Media.pause" | "\0Node.cloneNode")
            {
                let exception=self.native_dom_exception(kind,&message)?;
                return Err(Fault::thrown(exception));
            }
            return Err(Fault::exception(kind,message));
        }
        if method=="\0Attr.set.value"{let value=self.copy_value(args.first().ok_or_else(||Fault::exception("TypeError","Attr value setter requires one argument."))?)?;self.host_set_native(object,"value",value,host)?;return Ok(Value::Undefined);}
        if child_remove_operation(method) {
            if let Some(parent)=host.child_removal_parent(object).map_err(Fault::new)? {
                self.call_host(parent,"removeChild",&[Value::Host(object)],host)?;
            }
            return Ok(Value::Undefined);
        }
        if matches!(method, adjacent_interface::TEXT | adjacent_interface::ELEMENT) { return self.insert_adjacent(object, method, args, host); }
        if let Some((name, true)) = window_relationships::operation(method) {
            return self.set_window_relationship(object, name, args, host);
        }
        if method=="\0Element.getAttributeNames" {
            let names=host.attribute_name_snapshot(object).map_err(Fault::new)?;
            if names.len()>self.limits.heap.object_properties{return Err(Fault::new("Attribute names exceed the array/property allowance."));}
            let output=Value::Object(self.new_object(Some(names.len()))?);
            for (index,name) in names.into_iter().enumerate(){
                self.step()?;
                let(text,units)=name.into_parts();let value=units.map_or_else(||Value::String(text),Value::StringUnits);self.create_data_property(&output,PropertyKey::String(index.to_string()),value,host)?;
            }
            return Ok(output);
        }
        if method==event_interface::PATH_METHOD {
            return self.event_path_array(object,host);
        }
        if method==HTML_FORM_RESET {
            let Some(token)=host.begin_form_reset(object).map_err(Fault::new)? else {return Ok(Value::Undefined)};
            let accepted=self.dispatch_native_event(token,host)?;
            // Nested callbacks may have installed another incumbent. Restore
            // the method caller before the final authority and native preflight.
            host.native_operation_caller(self.native_access_caller
                .unwrap_or_else(||self.browser_global.map(|(window,_)|window)));
            if accepted {
                let result=(||{
                    host.prepare_form_reset(object).map_err(Fault::new)?;
                    while let Some(output)=host.next_form_reset_output(object).map_err(Fault::new)? {
                        self.step()?;
                        self.reset_native_output_inner(output,host)?;
                    }
                    host.native_operation_caller(self.native_access_caller
                        .unwrap_or_else(||self.browser_global.map(|(window,_)|window)));
                    host.complete_form_reset(object).map_err(Fault::new)
                })();
                host.finish_form_reset(object);
                result?;
            }
            return Ok(Value::Undefined);
        }
        if host.synchronous_event_dispatch(object,method) {
            let token=host.begin_event_dispatch(object,args).map_err(Fault::new)?;
            return self.dispatch_native_event(token, host).map(Value::Bool);
        }
        if let Some(result) = self.abort_native_operation(object, method, args, host) {
            return result;
        }
        if method == "getBoundingClientRect" {
            if let Some(ancestry) = host.instance_ancestry(object) {
                let caller = self.native_access_caller.unwrap_or_else(|| self.browser_global.map(|(window, _)| window));
                if caller != Some(ancestry.window) { host.validate_window_access(caller, ancestry.window).map_err(Fault::new)?; }
                if let Some(snapshot) = host.bounding_rect_snapshot(object) {
                    let values = snapshot.map_err(Fault::new)?;
                    let realm = self.window_realms.get(&ancestry.window).copied().ok_or_else(|| Fault::new("Rectangle owner realm is unavailable."))?;
                    return self.with_intrinsic_realm(realm, |runtime| runtime.new_rect_value(values, true));
                }
            }
        }
        if host.indexed_for_each(object, method) {
            let callback = args.first().cloned().unwrap_or(Value::Undefined);
            if !matches!(callback, Value::Function(_)) {
                return Err(Fault::exception(
                    "TypeError",
                    "forEach requires a callable callback.",
                ));
            }
            let Value::Number(length) = self.host_get_checked(object, "length", host)? else {
                return Err(Fault::new("Invalid native collection length."));
            };
            if length < 0.0 || length.fract() != 0.0 || length > self.limits.heap.object_properties as f64 {
                return Err(Fault::new(
                    format!("Native collection iteration limit reached ({} entries).", self.limits.heap.object_properties),
                ));
            }
            let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
            for index in 0..length as usize {
                self.step()?;
                let key = index.to_string();
                if !host.has(object,&key).map_err(Fault::new)? { continue; }
                let value = self.host_read(object, &key, host)?;
                self.call_value(
                    callback.clone(),
                    &[value, Value::Number(index as f64), Value::Host(object)],
                    this_arg.clone(),
                    host,
                )?;
            }
            return Ok(Value::Undefined);
        }
        let mut fields = None;
        if let Some((index, names)) = host.dictionary_argument(object, method, args) {
            if let Some(value @ (Value::Object(_) | Value::Host(_) | Value::Function(_))) =
                args.get(index)
            {
                if names.len() > self.limits.capacity.arguments {
                    return Err(Fault::new(format!("Native dictionary field limit reached ({}).", self.limits.capacity.arguments)));
                }
                let mut values = Vec::new();
                values.try_reserve(names.len()).map_err(|_|Fault::new("Could not reserve native dictionary fields."))?;
                for &name in names {
                    let reference = Reference::Property(self.copy_value(value)?, name.into());
                    let mut member=self.read_reference(&reference, host)?;
                    if let Some(string_hint)=host.dictionary_member_conversion(object,method,name).filter(|_|!matches!(member,Value::Undefined)) {
                        let primitive=self.primitive_with_hint(member,string_hint,host)?;
                        self.work(primitive.bytes())?;
                        member=self.checked_value(if string_hint {
                            if primitive.is_string(){primitive}else{Value::String(primitive.script_text()?)}
                        } else {Value::Number(primitive.script_number()?)})?;
                    }
                    host.validate_dictionary_member(object,method,name,&member)
                        .map_err(|(kind,message)|Fault::exception(kind,message))?;
                    values.push((name,member));
                }
                fields = Some(values);
            }
        }
        if method == canvas_readback::METHOD {
            return self.canvas_image_data(object, args, fields.as_deref(), host);
        }
        host.native_operation_caller(self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(window,_)|window)));
        if method==DOCUMENT_IMPORT_NODE{self.validate_native_access(object,host)?;for &index in host.native_reference_arguments(object,method){if let Some(Value::Host(source))=args.get(index){self.validate_native_access(*source,host)?;}}}
        if let Err((kind,message))=host.validate_dictionary_call(object,method,args,fields.as_deref()){if method==DOCUMENT_IMPORT_NODE&&kind=="NotSupportedError"{return Err(Fault::thrown(self.native_dom_exception(kind,&message)?));}return Err(Fault::exception(kind,message));}
        if let Some(fields) = &fields { for (_, value) in fields { self.pin_exported_value(value); } }
        if method==text_split_interface::METHOD{return self.split_native_text(object,args,host);}
        if method==node_normalize_interface::METHOD{return self.normalize_native_node(object,host);}
        if character_data_interface::operation(method).is_some(){let edit=host.prepare_character_data_edit(object,method,args).map_err(Fault::new)?;if !edit.value.is_string(){return Err(Fault::new("Native CharacterData edit requires an already-converted string."));}self.host_set_character_data_splice(object,edit,host)?;return Ok(Value::Undefined);}
        let ticket=self.prepare_host_microtask(host,object,method,args,false)?;
        host.native_operation_caller(self.native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(window, _)| window)));
        let observation = self.prepare_operation_observation(object,method,args,host)?;
        let result=if let Some(limit)=host.console_message_limit(object,method) {
            self.console_message(args,limit,host).and_then(|message|host.print_console(object,method,&message).map_err(Fault::new))
        } else if xhr_interface::operation(method).is_some(){self.call_request_operation(object,method,args,host)}else{host.call_with_dictionary(object, method, args, fields.as_deref()).map_err(Fault::new)};
        self.finish_tree_observation(observation, host)?;
        if let Ok(value)=&result {self.finish_new_event_result(object,method,value,host)?;}
        if self.drain_native_promise_effects(host)?!=0 {
            self.defer_host_microtask_after_effects(ticket)?;
        }
        self.finish_host_microtask(host,ticket)?;
        result
    }

    fn binary_value(
        &mut self,
        op: &str,
        left: Value,
        right: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        // Equality and coercion also inspect strings; they share the same work
        // allowance as concatenation, searches, and value copying.
        self.work_as(left.bytes().saturating_add(right.bytes()), WorkKind::BinaryInspection)?;
        if matches!(op, "===" | "!==") {
            return Ok(Value::Bool(strict_equal(&left, &right) == (op == "===")));
        }
        if matches!(op, "==" | "!=") {
            let converts = |value: &Value, other: &Value| {
                matches!(
                    value,
                    Value::Object(_) | Value::Function(_) | Value::Host(_)
                ) && matches!(
                    other,
                    Value::Number(_) | Value::BigInt(_) | Value::String(_) | Value::StringUnits(_) | Value::Bool(_) | Value::Symbol(_)
                )
            };
            let left = if converts(&left, &right) {
                self.primitive_with_preference(left, primitive_conversion::Hint::Default, host)?
            } else {
                left
            };
            let right = if converts(&right, &left) {
                self.primitive_with_preference(right, primitive_conversion::Hint::Default, host)?
            } else {
                right
            };
            self.work_as(left.bytes().saturating_add(right.bytes()), WorkKind::BinaryInspection)?;
            let equal = if matches!(left, Value::BigInt(_)) || matches!(right, Value::BigInt(_)) {
                !matches!(left, Value::Undefined | Value::Null | Value::Symbol(_))
                    && !matches!(right, Value::Undefined | Value::Null | Value::Symbol(_))
                    && self.big_int_order(&left, &right)?.is_some_and(std::cmp::Ordering::is_eq)
            } else {
                loose_equal(&left, &right)?
            };
            return Ok(Value::Bool(equal == (op == "==")));
        }
        // Both operands have already been evaluated. Addition and comparisons
        // convert both primitives before testing their types. Other numeric
        // operators finish ToNumber(left) first: a Symbol error must prevent
        // the right-hand object's conversion callback from running.
        let hint = if op == "+" {
            primitive_conversion::Hint::Default
        } else {
            primitive_conversion::Hint::Number
        };
        let left = self.primitive_with_preference(left, hint, host)?;
        let numeric_left = if matches!(op, "+" | "<" | ">" | "<=" | ">=") {
            None
        } else {
            Some(left.script_number()?)
        };
        let right = self.primitive_with_preference(right, hint, host)?;
        left.primitive()?;
        right.primitive()?;
        if op == "+" && (matches!(left, Value::StringUnits(_)) || matches!(right, Value::StringUnits(_))) {
            return self.concat_unit_values(&left, &right);
        }
        if op == "+" && (matches!(left, Value::String(_)) || matches!(right, Value::String(_))) {
            self.work_as(
                left.bytes()
                    .saturating_add(right.bytes())
                    .saturating_add(64),
                WorkKind::ConcatenationScratch,
            )?;
            let mut text = left.script_text()?;
            let right = right.script_text()?;
            if text.len().saturating_add(right.len()) > self.limits.capacity.string_bytes {
                return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
            }
            text.push_str(&right);
            return self.checked_value(Value::String(text));
        }
        if matches!(op, "<" | ">" | "<=" | ">=") {
            let order = if let (Value::String(left), Value::String(right)) = (&left, &right) {
                self.work_as(left.len().saturating_add(right.len()), WorkKind::BinaryInspection)?;
                Some(left.encode_utf16().cmp(right.encode_utf16()))
            } else if left.is_string() && right.is_string() {
                Some(self.string_units_for_value(&left)?.cmp(&self.string_units_for_value(&right)?))
            } else if matches!(left, Value::BigInt(_)) || matches!(right, Value::BigInt(_)) {
                self.big_int_order(&left, &right)?
            } else {
                left.script_number()?.partial_cmp(&right.script_number()?)
            };
            let value = order.is_some_and(|order| match op {
                "<" => order.is_lt(),
                ">" => order.is_gt(),
                "<=" => !order.is_gt(),
                _ => !order.is_lt(),
            });
            return Ok(Value::Bool(value));
        }
        self.work_as(left.bytes().saturating_add(right.bytes()), WorkKind::BinaryInspection)?;
        let a = match numeric_left {
            Some(number) => number,
            None => left.script_number()?,
        };
        let b = right.script_number()?;
        Ok(Value::Number(match op {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "**" => operators::number_exponentiate(a, b),
            "/" => a / b,
            "%" => a % b,
            "&" => f64::from(int32(a) & int32(b)),
            "|" => f64::from(int32(a) | int32(b)),
            "^" => f64::from(int32(a) ^ int32(b)),
            "<<" => f64::from(int32(a).wrapping_shl(int32(b) as u32 & 31)),
            ">>" => f64::from(int32(a).wrapping_shr(int32(b) as u32 & 31)),
            ">>>" => f64::from((int32(a) as u32).wrapping_shr(int32(b) as u32 & 31)),
            _ => return Err(Fault::new("Unsupported binary operator.")),
        }))
    }

    fn call_named_value(
        &mut self,
        function: Value,
        args: &[Value],
        this: Value,
        property: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let noncallable = !matches!(function, Value::Function(_));
        match self.call_value(function, args, this, host) {
            Err(mut fault)
                if noncallable
                    && matches!(
                        fault.exception.as_deref(),
                        Some(Exception::Error("TypeError"))
                    ) =>
            {
                // Use only the previously resolved key and value type. Never
                // reread a getter, coerce a value or annotate a callee's throw.
                let mut escaped = property.chars().flat_map(char::escape_default);
                let name: String = escaped.by_ref().take(80).collect();
                fault.message.push_str(" Attempted member \"");
                fault.message.push_str(&name);
                if escaped.next().is_some() {
                    fault.message.push_str("...");
                }
                fault.message.push_str("\".");
                Err(fault)
            }
            result => result,
        }
    }

    #[inline(always)]
    fn call_value(&mut self, function: Value, args: &[Value], this: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        // Keep constructor-only root/error temporaries out of this inlined
        // ordinary-call frame. Recursive calls share the unchanged native-stack
        // and call-depth guards, including the one-MiB try/finally control.
        // Audited native leaf getters cannot reenter Script or expose arena
        // edges. The native-call inhibition below covers their full dispatch;
        // a redundant callee root need not consume scratch for every DOM read.
        let leaf = args.is_empty() && matches!(this, Value::Host(_))
            && matches!(&function, Value::Function(id) if matches!(self.functions.get(*id),
                Some(Function::Host {object,method,receiver_required:true,constructible:false,..})
                    if host.native_leaf_getter(*object,method)));
        let marker = if leaf {
            let marker=self.transient_marker();
            self.transient_roots.calls+=1;
            self.transient_roots.call_scopes=self.transient_roots.call_scopes.saturating_add(1);
            marker
        } else {self.publish_call_inputs(&function, &this, args)?};
        if !matches!(&function, Value::Function(id) if matches!(self.functions.get(*id),
            Some(Function::Script { .. } | Function::Bound(_)
                | Function::Native(Builtin::FunctionMethod("call" | "apply") | Builtin::Eval)))) {
            if self.transient_roots.inhibited == 0 {
                self.transient_roots.outer_native = match function { Value::Function(id) => Some(id), _ => None };
            }
            self.inhibit_transient_collection_for("native-call");
        }
        let result = self.call_value_routed(function, args, this, host);
        self.restore_transient_marker(marker);
        result
    }

    // construct_value_routed has already established this Script constructor's
    // realm and native-access caller, including across prototype getters. Keep
    // explicit construction arguments off the ordinary forwarding ABI below.
    #[inline(never)]
    fn call_constructed_script(&mut self, index: usize, args: &[Value], this: Value, new_target: Option<usize>, host: &mut dyn Host) -> Result<Value, Fault> {
        debug_assert!(matches!(self.functions.get(index), Some(Function::Script { .. })));
        let marker = self.publish_call_inputs(&Value::Function(index), &this, args)?;
        if let Some(target) = new_target {
            if let Err(error) = self.push_transient_root(function_reachability::Edge::Function(target)) {
                self.restore_transient_marker(marker);
                return Err(error);
            }
        }
        let result = (|| {
            // Preserve the ordinary call-entry checks and their work ordering.
            self.step()?;
            if args.len() > self.limits.capacity.arguments { return Err(self.argument_limit_fault()); }
            for value in args {
                if value.bytes() > self.limits.capacity.string_bytes { return Err(self.string_limit_fault()); }
            }
            if this.bytes() > self.limits.capacity.string_bytes { return Err(self.string_limit_fault()); }
            if self.is_derived_script_constructor(index) {
                self.call_derived_script_value(index, args, this, new_target, host)
            } else {
                self.call_script_value(index, args, this, new_target, host)
            }
        })();
        self.restore_transient_marker(marker);
        result
    }

    fn call_value_routed(&mut self, function: Value, args: &[Value], this: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        if self.changes_native_access_caller(&function) {
            return self.call_with_native_access_caller(function, args, this, host);
        }
        let realm = match &function { Value::Function(id) => self.function_realms.get(*id).copied().unwrap_or(self.active_realm), _ => self.active_realm };
        if realm == self.active_realm { return self.call_value_in_realm(function, args, this, host); }
        self.with_intrinsic_realm(realm, |runtime| runtime.call_value_in_realm(function, args, this, host))
    }

    #[inline(never)]
    fn changes_native_access_caller(&self, function: &Value) -> bool {
        if matches!(function, Value::Function(id) if matches!(self.functions.get(*id), Some(Function::Script { .. }))) {
            self.native_access_caller.is_some()
        } else { self.native_access_caller.is_none() }
    }

    // Saving a Result across restoration makes a materially larger debug frame.
    // Keep that frame off ordinary recursive Script calls on small test stacks.
    #[inline(never)]
    fn call_with_native_access_caller(&mut self, function: Value, args: &[Value], this: Value,
        host: &mut dyn Host) -> Result<Value, Fault> {
        let realm = match &function { Value::Function(id) => self.function_realms.get(*id).copied().unwrap_or(self.active_realm), _ => self.active_realm };
        let previous = self.native_access_caller;
        self.native_access_caller = if matches!(&function, Value::Function(id) if matches!(self.functions.get(*id), Some(Function::Script { .. }))) {
            None
        } else { Some(self.browser_global.map(|(window, _)| window)) };
        let result = if realm == self.active_realm { self.call_value_in_realm(function, args, this, host) }
            else { self.with_intrinsic_realm(realm, |runtime| runtime.call_value_in_realm(function, args, this, host)) };
        self.native_access_caller = previous;
        result
    }

    fn call_value_in_realm(
        &mut self,
        function: Value,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let (index, ordinary) = self.preflight_call_value(&function, args, &this)?;
        if ordinary {
            return self.call_script_value(index, args, this, None, host);
        }
        self.call_non_script_value(index, args, this, host)
    }

    // Finish validation before entering Script's larger invocation setup frame.
    // Constructors retain that entry's defensive guard and separate preflight.
    #[inline(never)]
    fn preflight_call_value(
        &mut self, function: &Value, args: &[Value], this: &Value,
    ) -> Result<(usize, bool), Fault> {
        self.step()?;
        if args.len() > self.limits.capacity.arguments {
            return Err(self.argument_limit_fault());
        }
        for value in args {
            if value.bytes() > self.limits.capacity.string_bytes {
                return Err(self.string_limit_fault());
            }
        }
        if this.bytes() > self.limits.capacity.string_bytes {
            return Err(self.string_limit_fault());
        }
        let Value::Function(index) = function else {
            return Err(Self::not_callable_fault(function));
        };
        let ordinary = self.is_ordinary_script_call(*index);
        if ordinary { self.check_native_call_stack("Function call")?; }
        Ok((*index, ordinary))
    }

    #[cold]
    #[inline(never)]
    fn not_callable_fault(function: &Value) -> Fault {
        Fault::exception("TypeError", format!(
            "The value is not a callable function. Actual value type: {}.",
            function.diagnostic_type()))
    }

    #[inline(never)]
    fn is_ordinary_script_call(&self, index: usize) -> bool {
        // Type classification returns before a recursive body starts. Keep its
        // Program/template borrows out of the persistent call-forwarding frame.
        matches!(self.functions.get(index), Some(Function::Script { program, template, .. })
            if !program.templates[*template].class_constructor)
    }

    #[inline(never)]
    fn is_derived_script_constructor(&self, index: usize) -> bool {
        matches!(self.functions.get(index), Some(Function::Script { program, template, .. })
            if program.templates[*template].derived_constructor)
    }

    #[inline(never)]
    fn call_non_script_value(
        &mut self, index: usize, args: &[Value], this: Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match self.functions.get(index) {
            Some(Function::Native(Builtin::Proxy)) => {
                return Err(Fault::exception("TypeError", "Proxy requires new."));
            }
            Some(Function::Native(Builtin::ReflectGet)) => {
                return self.reflect_get(args, host);
            }
            Some(Function::Native(Builtin::ReflectSet)) => {
                return self.reflect_set(args, host);
            }
            Some(Function::Native(Builtin::ReflectOwnKeys)) => {
                return self.reflect_own_keys(args, host);
            }
            Some(Function::Bound(_)) => {
                let (target, arguments, receiver) = self.bound_arguments(index, args, true)?;
                return self.call_value(Value::Function(target), &arguments, receiver, host);
            }
            Some(Function::Promise(closure)) => {
                let closure = closure.clone();
                return self.promise_closure_call(closure, args, host);
            }
            Some(Function::Native(Builtin::Date(operation))) => {
                return self.date_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Response(operation))) => {
                return self.response_builtin(*operation,args,this,host);
            }
            Some(Function::Native(Builtin::Headers(operation))) => {
                return self.headers_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Url(operation))) => {
                return self.url_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Rect(operation))) => {
                return self.rect_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::ImageData(operation))) => {
                return self.image_data_builtin(*operation, this);
            }
            Some(Function::Native(Builtin::DomException(operation))) => {
                return self.dom_exception_builtin(*operation,this);
            }
            Some(Function::Native(Builtin::MutationObserver(operation))) => {
                return self.mutation_observer_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::TreeWalker(operation))) => {
                return self.tree_walker_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Promise(operation))) => {
                return self.promise_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Symbol(operation))) => {
                return self.symbol_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::TextEncoding(operation))) => {
                return self.text_encoding_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Collection(operation))) => {
                return self.collection_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Iterator(operation))) => {
                return self.iterator_builtin(*operation, this, host);
            }
            Some(Function::Native(Builtin::Generator(operation))) => {
                return self.generator_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Iterable(operation))) => {
                return self.iterable_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Binary(operation))) => {
                return self.binary_builtin(*operation, args, this, host);
            }
            Some(Function::Native(Builtin::Eval)) => {
                return self.string_eval(args.first().unwrap_or(&Value::Undefined), None, host);
            }
            Some(Function::Native(Builtin::Math("random"))) => {
                let value = host.random().map_err(Fault::new)?;
                if !(0.0..1.0).contains(&value) {
                    return Err(Fault::new(
                        "The host returned an invalid Math.random value.",
                    ));
                }
                return Ok(Value::Number(value));
            }
            Some(Function::Native(
                builtin @ (Builtin::ObjectStatic(_)
                | Builtin::ObjectMethod(_)
                | Builtin::FunctionMethod(_)
                | Builtin::ArrayMethod(_)
                | Builtin::RegExpMethod(_)
                | Builtin::StringMethod(_)
                | Builtin::StringFromCharCode
                | Builtin::PrimitiveMethod(_, _)
                | Builtin::BigIntMethod(_)
                | Builtin::ErrorToString),
            )) => {
                return self.prototype_builtin(*builtin, args, this, host);
            }
            Some(Function::Native(builtin)) => return self.builtin(*builtin, args, host),
            Some(Function::Host {
                object,
                method,
                receiver_required,
                constructible,
                ..
            }) => {
                if *constructible {
                    return Err(Fault::exception(
                        "TypeError",
                        "This native constructor requires new.",
                    ));
                }
                let (mut object, method, receiver_required) =
                    (*object, *method, *receiver_required);
                // Namespace console operations have no interface receiver brand.
                // Their saved holder still undergoes owner/origin validation.
                let receiver_required=receiver_required&&host.console_message_limit(object,method).is_none();
                if host.promise_returning_operation(object,method) {
                    return self.call_host_promise(object,method,receiver_required,this,host);
                }
                // Opted-in canonical Window holders keep their origin guard
                // before `this` replaces the holder. Legacy instance-owned
                // methods retain their existing borrowing policy.
                if host.native_method_owner(object,method)==Some(object) {
                    self.validate_native_access(object,host)?;
                }
                if let Some(receiver) = host.global_operation_receiver(object, method, &this) {
                    self.validate_native_access(object, host)?;
                    if let Value::Host(handle) = &this { self.validate_native_access(*handle, host)?; }
                    object = receiver.map_err(|(kind, message)| Fault::exception(kind, message))?;
                }
                if receiver_required {
                    match this {
                        Value::Host(receiver)
                            if host.accepts_receiver(object, method, receiver) =>
                        {
                            object = receiver;
                        }
                        _ => {
                            return Err(Self::incompatible_native_receiver(method,&this,host))
                        }
                    }
                }
                self.check_native_call_stack("Function call")?;
                self.call_depth += 1;
                let result = self.call_host(object, method, args, host);
                self.call_depth -= 1;
                return self.checked_value(result?);
            }
            Some(Function::Script { program, template, .. }) if program.templates[*template].class_constructor => {
                return Err(Self::class_constructor_call_fault());
            }
            Some(Function::Script { .. }) => unreachable!("ordinary script dispatch handled separately"),
            Some(Function::Vacant) | None => return Err(Fault::new("Invalid function handle.")),
        }
    }

    #[inline(never)]
    fn register_script_invocation_inputs(
        &mut self, index: usize, parent: usize, this: &Value, new_target: Option<usize>,
    ) -> Result<(), Fault> {
        // Complete constructor/arrow selection and all registration scratch
        // before the recursive Script body is entered. The caller retains only
        // the previous explicit context, restoring it on every Result path.
        self.push_transient_value(this)?;
        self.push_transient_root(function_reachability::Edge::Environment(parent))?;
        let Function::Script { program, template, lexical_invocation, home_object, .. } = &self.functions[index]
            else { unreachable!() };
        let (arrow, lexical, derived, home) = (program.templates[*template].arrow,
            *lexical_invocation, program.templates[*template].derived_constructor, *home_object);
        if arrow {
            if let Some(context) = lexical {
                self.invocation_data(context)?;
                self.push_transient_root(function_reachability::Edge::Environment(context.environment))?;
            }
            self.invocation = lexical;
            return Ok(());
        }
        // Reclamation precedes reservation, including explicit regression caps.
        if self.free_environments.is_empty() && (self.environment_count_refused() || self.environments.len() == self.environments.capacity()) {
            self.collect_function_candidates_at_entry()?;
        }
        self.prepare_environment_slot_storage()?;
        let retained = if derived { 0 } else { this.bytes() };
        if self.value_bytes.saturating_add(retained) > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        let this_binding = if derived { derived_context::ThisBinding::Uninitialized }
            else { derived_context::ThisBinding::Initialized(self.copy_value(this)?) };
        let scope = self.new_environment(parent, true)?;
        self.environment_mut(scope).invocation = Some(derived_context::Data {
            callee: index, new_target, this_binding, home_object: home, derived_constructor: derived,
        });
        self.value_bytes += retained;
        if let Err(error) = self.push_transient_root(function_reachability::Edge::Environment(scope)) {
            self.release_environment(scope);
            return Err(error);
        }
        self.invocation = Some(InvocationContext { environment: scope });
        Ok(())
    }

    #[inline(never)]
    #[cold]
    fn incompatible_native_receiver(method:&str,receiver:&Value,host:&dyn Host)->Fault {
        let receiver=if let Value::Host(handle)=receiver {
            let name=host.instance_ancestry(*handle).and_then(|ancestry|ancestry.constructor.map(|(_,name)|name)).unwrap_or("native object");
            format!("{name} (native handle 0x{handle:08x})")
        }else{receiver.diagnostic_type().into()};
        Fault::exception("TypeError",format!("Native method called with an incompatible receiver. Operation {method:?}; actual receiver: {receiver}."))
    }

    #[inline(never)]
    #[cold]
    fn class_constructor_call_fault() -> Fault {
        // Keep typed Error allocation and its unoptimized scratch off ordinary
        // recursive Script frames. This is only the rejected class-call path.
        Fault::exception("TypeError", "Class constructors require new.")
    }

    #[inline(never)]
    fn call_script_value(
        &mut self, index: usize, args: &[Value], this: Value, new_target: Option<usize>, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.check_native_call_stack("Function call")?;
        let Function::Script {
            program,
            template,
            environment,
            lexical_this,
            ..
        } = &self.functions[index]
        else {
            unreachable!()
        };
        let (program, template, parent, lexical_this) = (
            program.clone(),
            *template,
            *environment,
            lexical_this.clone(),
        );
        let this = if let Some(value) = lexical_this {
            self.work(value.bytes())?;
            value
        } else if !program.templates[template].strict {
            match this {
                Value::Null | Value::Undefined => self.global_this()?,
                Value::Object(_) | Value::Function(_) | Value::Host(_) => this,
                primitive => self.box_primitive(primitive)?,
            }
        } else {
            this
        };
        let marker = self.transient_marker();
        let previous_invocation = self.invocation;
        if let Err(error) = self.register_script_invocation_inputs(index, parent, &this, new_target) {
            self.invocation = previous_invocation;
            self.restore_transient_marker(marker);
            return Err(error);
        }
        #[cfg(test)]
        if let Some(observe) = self.transient_roots.script_input_observer { observe(self, index, args, &this); }
        self.transient_roots.script_entries = self.transient_roots.script_entries.saturating_add(1);
        if self.transient_roots.inhibited == 0 {
            self.transient_roots.uninhibited_script_entries = self.transient_roots.uninhibited_script_entries.saturating_add(1);
        }
        if let Err(error) = self.collect_function_candidates_at_entry() {
            self.release_unentered_invocation(index);
            self.invocation = previous_invocation;
            self.restore_transient_marker(marker);
            return Err(error);
        }
        self.call_depth += 1;
        let previous_strict = self.strict;
        self.transient_roots.owned_call_depth += 1;
        self.strict = program.templates[template].strict;
        let result = match program.templates[template].kind {
            FunctionKind::Ordinary => {
                self.script_parameters_and_body(&program, template, index, parent, args, &this, host)
            }
            FunctionKind::Async => {
                self.async_function_body(&program, template, index, parent, args, &this, host)
            }
            FunctionKind::Generator => {
                self.start_generator(&program, template, index, parent, args, &this, host)
            }
        };
        self.call_depth -= 1;
        self.transient_roots.owned_call_depth -= 1;
        self.strict = previous_strict;
        self.invocation = previous_invocation;
        self.restore_transient_marker(marker);
        result
    }

    // Derived completion may execute real default-super construction before
    // releasing its invocation environment. Keep that completion pipeline out
    // of the qualified ordinary Script entry and its direct immutable body.
    #[inline(never)]
    fn call_derived_script_value(
        &mut self, index: usize, args: &[Value], this: Value, new_target: Option<usize>, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.check_native_call_stack("Function call")?;
        let Function::Script { program, template, environment, .. } = &self.functions[index]
            else { unreachable!() };
        let (program, template, parent) = (program.clone(), *template, *environment);
        debug_assert!(program.templates[template].derived_constructor && program.templates[template].strict);
        let marker = self.transient_marker();
        let previous_invocation = self.invocation;
        if let Err(error) = self.register_script_invocation_inputs(index, parent, &this, new_target) {
            self.invocation = previous_invocation;
            self.restore_transient_marker(marker);
            return Err(error);
        }
        #[cfg(test)]
        if let Some(observe) = self.transient_roots.script_input_observer { observe(self, index, args, &this); }
        self.transient_roots.script_entries = self.transient_roots.script_entries.saturating_add(1);
        if self.transient_roots.inhibited == 0 {
            self.transient_roots.uninhibited_script_entries = self.transient_roots.uninhibited_script_entries.saturating_add(1);
        }
        if let Err(error) = self.collect_function_candidates_at_entry() {
            self.release_unentered_invocation(index);
            self.invocation = previous_invocation;
            self.restore_transient_marker(marker);
            return Err(error);
        }
        self.call_depth += 1;
        let previous_strict = self.strict;
        self.transient_roots.owned_call_depth += 1;
        self.strict = program.templates[template].strict;
        let result = self.derived_script_parameters_and_body(&program, template, index, parent, args, &this, host);
        self.call_depth -= 1;
        self.transient_roots.owned_call_depth -= 1;
        self.strict = previous_strict;
        self.invocation = previous_invocation;
        self.restore_transient_marker(marker);
        result
    }

    fn call_member(
        &mut self,
        object: Value,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // Legacy direct member dispatch includes apply's array-like getters and
        // native helpers whose temporary values are not individually published.
        let marker = self.inhibit_transient_collection();
        let result = self.call_member_unpublished(object, method, args, host);
        self.restore_transient_marker(marker);
        result
    }

    fn call_member_unpublished(
        &mut self,
        object: Value,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if let Value::Function(index) = object {
            return match method {
                "call" => {
                    let this = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                    self.call_value(
                        Value::Function(index),
                        args.get(1..).unwrap_or(&[]),
                        this,
                        host,
                    )
                }
                "apply" => {
                    let this = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                    let values =
                        self.apply_arguments(args.get(1).unwrap_or(&Value::Undefined), host)?;
                    self.call_value(Value::Function(index), &values, this, host)
                }
                "keys"
                    if matches!(
                        self.functions.get(index),
                        Some(Function::Native(Builtin::Object))
                    ) =>
                {
                    self.object_keys(args.first().unwrap_or(&Value::Undefined), host)
                }
                "isArray"
                    if matches!(
                        self.functions.get(index),
                        Some(Function::Native(Builtin::Array))
                    ) =>
                {
                    let array = match args.first() {
                        Some(Value::Object(id)) => self.object(*id)?.length.is_some(),
                        _ => false,
                    };
                    Ok(Value::Bool(array))
                }
                _ => Err(Fault::new(format!(
                    "Function method {method:?} is not implemented."
                ))),
            };
        }
        if let Value::Object(id) = object {
            if self.object(id)?.regexp.is_some() && matches!(method, "test" | "exec" | "toString") {
                return self.regexp_method(id, method, args, host);
            }
            if method == "hasOwnProperty" {
                let key = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                let key = self.property_key(key, host)?;
                return Ok(Value::Bool(match key {
                    PropertyKey::String(name) => self.owns_property(id, &name)?,
                    PropertyKey::StringUnits(units) => self.owns_units_property(id, &units)?,
                    PropertyKey::Symbol(key) => {
                        self.object(id)?.symbol_properties.contains_key(&key)
                    }
                }));
            }
            if method == "valueOf" {
                return Ok(Value::Object(id));
            }
            if self.object(id)?.length.is_some() {
                if method == "toString" && self.object(id)?.properties.contains_key("join") {
                    let join = self.get_property_with_host(id, "join", host)?;
                    return if matches!(join, Value::Function(_)) {
                        self.call_value(join, &[], Value::Object(id), host)
                    } else {
                        Ok(Value::String("[object Array]".into()))
                    };
                }
                return self.array_method(id, method, args, host);
            }
            return if method == "toString" && self.object(id)?.error_name.is_some() {
                self.error_text(id, host)
            } else if method == "toString" {
                Ok(Value::String("[object Object]".into()))
            } else {
                Err(Fault::new(format!(
                    "Object method {method:?} is not implemented."
                )))
            };
        }
        if let Value::Host(object) = object {
            let value = self.call_host(object, method, args, host)?;
            return self.checked_value(value);
        }
        if matches!(method,"replace"|"replaceAll") && object.is_string() {
            return self.string_replace_value(object,args,method=="replaceAll",host);
        }
        if matches!(method, "slice" | "substring" | "substr") && object.is_string() {
            return self.string_slice_value(&object, method, args, host);
        }
        if method == "lastIndexOf" && object.is_string() {
            return self.string_last_index_value(&object, args, host);
        }
        if method == "indexOf" && object.is_string() {
            return self.string_index_value(&object, args, false, host);
        }
        if matches!(method, "charAt" | "charCodeAt" | "codePointAt") && object.is_string() {
            return self.string_character_value(&object,method,args,host);
        }
        if method == "trim" && object.is_string() {
            return self.string_trim_value(&object);
        }
        if let Value::StringUnits(ref units) = object {
            return match method {
                "toString" | "valueOf" => Ok(object),
                "concat" => self.string_primitive_concat(object, args, host),
                "search" | "match" => self.regexp_string_method_units(units, method, args, host),
                _ => Err(Fault::new("This string method does not support unpaired UTF-16 surrogates.")),
            };
        }
        if let Value::String(text) = object {
            if method == "concat" { return self.string_concat(&text,args,host); }
            if matches!(method, "search" | "match" | "replace") {
                return self.regexp_string_method(&text, method, args, host);
            }
            self.work_as(text.len().saturating_mul(3), WorkKind::StringMethodInput)?;
            return match method {
                "split" => self.split_string(&text, args),
                "toString" | "valueOf" => self.checked_value(Value::String(text)),
                "toLowerCase" => self.checked_value(Value::String(text.to_lowercase())),
                "toUpperCase" => self.checked_value(Value::String(text.to_uppercase())),
                "includes" | "startsWith" | "endsWith" => {
                    let search = args.first().unwrap_or(&Value::Undefined);
                    search.primitive()?;
                    self.work(search.bytes())?;
                    let search = search.script_text()?;
                    let units: Vec<_> = text.encode_utf16().collect();
                    let needle: Vec<_> = search.encode_utf16().collect();
                    let length = units.len();
                    if let Some(position) = args.get(1) {
                        position.primitive()?;
                        self.work(position.bytes())?;
                    }
                    let supplied = args
                        .get(1)
                        .filter(|value| !matches!(value, Value::Undefined))
                        .map(Value::to_number);
                    let position = if method == "endsWith" && supplied.is_none() {
                        length
                    } else {
                        bounded_index(supplied.unwrap_or(0.0), length)
                    };
                    let index = if method == "startsWith" {
                        units[position..].starts_with(&needle).then_some(position)
                    } else if method == "endsWith" {
                        units[..position]
                            .ends_with(&needle)
                            .then_some(position.saturating_sub(needle.len()))
                    } else if needle.is_empty() {
                        Some(position)
                    } else {
                        let mut found = None;
                        for (offset, part) in units[position..].windows(needle.len()).enumerate() {
                            // Charge the worst-case comparison before it
                            // runs. Repeated prefixes cannot hide quadratic
                            // native work behind one interpreter step.
                            self.work(needle.len().saturating_mul(2))?;
                            if part == needle {
                                found = Some(position + offset);
                                break;
                            }
                        }
                        found
                    };
                    Ok(Value::Bool(index.is_some()))
                }
                _ => Err(Fault::new(format!(
                    "String method {method:?} is not implemented."
                ))),
            };
        }
        match object {
            Value::Number(_) | Value::Bool(_)
                if method == "toString"
                    && (args.is_empty()
                        || matches!(args.first(), Some(Value::Number(radix)) if *radix == 10.0)) =>
            {
                self.checked_value(Value::String(object.script_text()?))
            }
            _ => Err(Fault::new(format!(
                "Method {method:?} is not callable on this value."
            ))),
        }
    }

    fn encode_uri(&mut self, text: &str, component: bool) -> Result<Value, Fault> {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut output = String::new();
        for byte in text.bytes() {
            self.step()?;
            if byte.is_ascii_alphanumeric()
                || b"-_.!~*'()".contains(&byte)
                || (!component && b";/?:@&=+$,#".contains(&byte))
            {
                self.append_string(&mut output, std::str::from_utf8(&[byte]).unwrap())?;
            } else {
                let encoded = [
                    b'%',
                    HEX[usize::from(byte >> 4)],
                    HEX[usize::from(byte & 15)],
                ];
                self.append_string(&mut output, std::str::from_utf8(&encoded).unwrap())?;
            }
        }
        self.checked_value(Value::String(output))
    }

    fn decode_uri(&mut self, text: &str, component: bool) -> Result<Value, Fault> {
        self.work(text.len())?;
        let mut output = Vec::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            self.step()?;
            if bytes[at] == b'%' {
                let Some(pair) = bytes.get(at + 1..at + 3) else {
                    return Err(Fault::exception("URIError", "Malformed URI escape."));
                };
                let Some(high) = char::from(pair[0]).to_digit(16) else {
                    return Err(Fault::exception("URIError", "Malformed URI escape."));
                };
                let Some(low) = char::from(pair[1]).to_digit(16) else {
                    return Err(Fault::exception("URIError", "Malformed URI escape."));
                };
                let byte = (high * 16 + low) as u8;
                if !component && b";/?:@&=+$,#".contains(&byte) {
                    output.extend_from_slice(&bytes[at..at + 3]);
                } else {
                    output.push(byte);
                }
                at += 3;
            } else {
                output.push(bytes[at]);
                at += 1;
            }
        }
        let decoded = String::from_utf8(output)
            .map_err(|_| Fault::exception("URIError", "URI escapes are not valid UTF-8."))?;
        self.checked_value(Value::String(decoded))
    }

    fn math_builtin(
        &mut self,
        name: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let count = match name {
            "min" | "max" | "hypot" => args.len(),
            "atan2" | "pow" | "imul" => 2,
            _ => 1,
        };
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            self.step()?;
            let value = args.get(index).unwrap_or(&Value::Undefined);
            values.push(self.binary_number(value, host)?);
        }
        let a = values.first().copied().unwrap_or(f64::NAN);
        let b = values.get(1).copied().unwrap_or(f64::NAN);
        let result = match name {
            "abs" => a.abs(),
            "acos" => a.acos(),
            "acosh" => a.acosh(),
            "asin" => a.asin(),
            "asinh" => a.asinh(),
            "atan" => a.atan(),
            "atanh" => a.atanh(),
            "atan2" => a.atan2(b),
            "cbrt" => a.cbrt(),
            "ceil" => a.ceil(),
            "clz32" => f64::from((int32(a) as u32).leading_zeros()),
            "cos" => a.cos(),
            "cosh" => a.cosh(),
            "exp" => a.exp(),
            "expm1" => a.exp_m1(),
            "floor" => a.floor(),
            "fround" => f64::from(a as f32),
            "imul" => f64::from(int32(a).wrapping_mul(int32(b))),
            "log" => a.ln(),
            "log1p" => a.ln_1p(),
            "log2" => a.log2(),
            "log10" => a.log10(),
            "pow" => operators::number_exponentiate(a, b),
            "round" => {
                if !a.is_finite() || a == 0.0 || a.abs() >= 4_503_599_627_370_496.0 {
                    a
                } else {
                    let floor = a.floor();
                    let rounded = if a - floor < 0.5 { floor } else { floor + 1.0 };
                    if rounded == 0.0 {
                        0.0f64.copysign(a)
                    } else {
                        rounded
                    }
                }
            }
            "sign" => {
                if a.is_nan() || a == 0.0 {
                    a
                } else {
                    a.signum()
                }
            }
            "sin" => a.sin(),
            "sinh" => a.sinh(),
            "sqrt" => a.sqrt(),
            "tan" => a.tan(),
            "tanh" => a.tanh(),
            "trunc" => a.trunc(),
            "min" | "max" => {
                let minimum = name == "min";
                let mut result = if minimum {
                    f64::INFINITY
                } else {
                    f64::NEG_INFINITY
                };
                for value in values {
                    if value.is_nan() {
                        result = f64::NAN;
                        break;
                    }
                    if if minimum {
                        value < result
                    } else {
                        value > result
                    } || value == 0.0 && result == 0.0 && value.is_sign_negative() == minimum
                    {
                        result = value;
                    }
                }
                result
            }
            "hypot" => {
                if values.iter().any(|value| value.is_infinite()) {
                    f64::INFINITY
                } else if values.iter().any(|value| value.is_nan()) {
                    f64::NAN
                } else {
                    let scale = values
                        .iter()
                        .fold(0.0f64, |largest, value| largest.max(value.abs()));
                    if scale == 0.0 {
                        0.0
                    } else {
                        let sum: f64 = values.iter().map(|value| (value / scale).powi(2)).sum();
                        scale * sum.sqrt()
                    }
                }
            }
            _ => return Err(Fault::new("Unsupported Math method.")),
        };
        Ok(Value::Number(result))
    }

    fn builtin(
        &mut self,
        builtin: Builtin,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = args.first().unwrap_or(&Value::Undefined);
        self.work_as(value.bytes().saturating_mul(2), WorkKind::NativeInput)?;
        match builtin {
            Builtin::ThrowTypeError => Err(Fault::exception(
                "TypeError",
                "Strict arguments callee access is forbidden.",
            )),
            Builtin::Symbol(_)
            | Builtin::Promise(_)
            | Builtin::Date(_)
            | Builtin::Url(_)
            | Builtin::Headers(_)
            | Builtin::Response(_)
            | Builtin::Rect(_)
            | Builtin::ImageData(_)
            | Builtin::DomException(_)
            | Builtin::MutationObserver(_)
            | Builtin::TreeWalker(_)
            | Builtin::Binary(_)
            | Builtin::Collection(_)
            | Builtin::Iterator(_)
            | Builtin::Generator(_)
            | Builtin::Iterable(_)
            | Builtin::TextEncoding(_) => Err(Fault::new(
                "Native data operations need an execution context.",
            )),
            Builtin::EmptyFunction => Ok(Value::Undefined),
            Builtin::Function => self.function_constructor(args, host),
            Builtin::AsyncFunction => self.async_function_constructor(args, None, host),
            Builtin::GeneratorFunction => self.dynamic_function_constructor(args, FunctionKind::Generator, None, host),
            Builtin::Proxy | Builtin::ReflectGet | Builtin::ReflectSet | Builtin::ReflectOwnKeys
            | Builtin::ObjectStatic(_)
            | Builtin::ObjectMethod(_)
            | Builtin::FunctionMethod(_)
            | Builtin::ArrayMethod(_)
            | Builtin::RegExpMethod(_)
            | Builtin::StringMethod(_)
            | Builtin::StringFromCharCode
            | Builtin::PrimitiveMethod(_, _)
            | Builtin::BigIntMethod(_)
            | Builtin::ErrorToString => {
                Err(Fault::new("Prototype methods need an execution context."))
            }
            Builtin::Math(name) => self.math_builtin(name, args, host),
            Builtin::Eval => Err(Fault::new("Intrinsic eval needs an execution context.")),
            Builtin::JsonParse => self.json_parse_builtin(args, host),
            Builtin::JsonStringify => self.json_stringify(args, host),
            Builtin::RegExp => {
                if matches!(args.get(1), None | Some(Value::Undefined)) {
                    if let Some(Value::Object(id)) = args.first() {
                        if self.object(*id)?.regexp.is_some() {
                            let constructor = self.get_property_with_host(*id, "constructor", host)?;
                            if matches!(constructor, Value::Function(id) if Some(id) == self.regexp_constructor) {
                                return self.copy_value(&args[0]);
                            }
                        }
                    }
                }
                self.regexp_factory(args)
            }
            Builtin::EncodeUri(component) => {
                let value = self.copy_value(value)?;
                let value = self.primitive_with_hint(value, true, host)?;
                self.encode_uri(&value.script_text()?, component)
            }
            Builtin::DecodeUri(component) => {
                let value = self.copy_value(value)?;
                let value = self.primitive_with_hint(value, true, host)?;
                self.decode_uri(&value.script_text()?, component)
            }
            Builtin::Base64(encode) => self.base64_builtin(encode, args, host),
            Builtin::Error(kind) => self.construct_error(kind, args, None, host),
            Builtin::Object => match value {
                Value::Null | Value::Undefined => self.new_object(None).map(Value::Object),
                Value::Object(_) | Value::Function(_) | Value::Host(_) => self.copy_value(value),
                primitive => {
                    let primitive = self.copy_value(primitive)?;
                    self.box_primitive(primitive)
                }
            },
            Builtin::Array => {
                if let [Value::Number(length)] = args {
                    if !length.is_finite() || *length < 0.0 || length.fract() != 0.0 {
                        return Err(Fault::exception("RangeError", "Invalid array length."));
                    }
                    if *length > self.limits.heap.array_length as f64 {
                        return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                    }
                    self.new_object(Some(*length as usize)).map(Value::Object)
                } else {
                    self.array_from_values(args)
                }
            }
            Builtin::Boolean => Ok(Value::Bool(value.truthy())),
            Builtin::String => {
                if args.is_empty() {
                    Ok(Value::String(String::new()))
                } else if let Value::Symbol(id) = value {
                    self.symbol_text(*id)
                } else {
                    let value = self.copy_value(value)?;
                    let value = self.primitive_with_hint(value, true, host)?;
                    if value.is_string() { self.checked_value(value) } else { self.checked_value(Value::String(value.script_text()?)) }
                }
            }
            Builtin::Number => Ok(Value::Number(if args.is_empty() {
                0.0
            } else {
                let primitive = self.binary_numeric_primitive(value, host)?;
                if let Value::BigInt(text) = primitive {
                    self.work_as(text.len(), WorkKind::NumberScratch)?;
                    text.parse::<f64>().expect("canonical BigInt digits")
                } else {
                    primitive.script_number()?
                }
            })),
            Builtin::BigInt => {
                let value = self.copy_value(value)?;
                self.big_int(value, host)
            }
            Builtin::BigIntWidth(signed) => self.big_int_width(signed, args, host),
            Builtin::IsFinite => Ok(Value::Bool(self.binary_number(value, host)?.is_finite())),
            Builtin::IsNaN => Ok(Value::Bool(self.binary_number(value, host)?.is_nan())),
            Builtin::ParseFloat => {
                let value = self.copy_value(value)?;
                let text = self.primitive_with_hint(value, true, host)?.script_text()?;
                let text = text.trim_start_matches(js_space);
                let number = if text.starts_with("Infinity") || text.starts_with("+Infinity") {
                    f64::INFINITY
                } else if text.starts_with("-Infinity") {
                    f64::NEG_INFINITY
                } else {
                    decimal_end(text.as_bytes(), true)
                        .and_then(|end| text[..end].parse::<f64>().ok())
                        .unwrap_or(f64::NAN)
                };
                Ok(Value::Number(number))
            }
            Builtin::ParseInt => {
                let value = self.copy_value(value)?;
                let text = self.primitive_with_hint(value, true, host)?.script_text()?;
                let mut text = text.trim_start_matches(js_space);
                let negative = text.starts_with('-');
                if text.starts_with(['+', '-']) {
                    text = &text[1..];
                }
                let radix = if let Some(radix) = args.get(1) {
                    int32(self.binary_number(radix, host)?)
                } else {
                    0
                };
                let infer = radix == 0;
                let mut radix = if infer { 10 } else { radix };
                if !(2..=36).contains(&radix) {
                    return Ok(Value::Number(f64::NAN));
                }
                if (infer || radix == 16) && (text.starts_with("0x") || text.starts_with("0X")) {
                    radix = 16;
                    text = &text[2..];
                }
                let mut number = 0.0;
                let mut count = 0;
                for ch in text.chars() {
                    let Some(digit) = ch.to_digit(radix as u32) else {
                        break;
                    };
                    number = number * f64::from(radix) + f64::from(digit);
                    count += 1;
                }
                Ok(Value::Number(if count == 0 {
                    f64::NAN
                } else if negative {
                    -number
                } else {
                    number
                }))
            }
        }
    }
}

pub(crate) fn int32(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        0
    } else {
        value.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
    }
}

fn bounded_index(value: f64, length: usize) -> usize {
    if value.is_nan() || value <= 0.0 {
        0
    } else {
        (value.trunc() as usize).min(length)
    }
}

enum LogicalOperand {
    Nullish,
    Falsy,
    Truthy,
}

impl LogicalOperand {
    fn classify(value: &Value) -> Self {
        match value {
            Value::Null | Value::Undefined => Self::Nullish,
            value if value.truthy() => Self::Truthy,
            _ => Self::Falsy,
        }
    }

    fn short_circuits(&self, operator: &str) -> bool {
        match operator {
            "&&" => !matches!(self, Self::Truthy),
            "||" => matches!(self, Self::Truthy),
            "??" => !matches!(self, Self::Nullish),
            _ => false,
        }
    }
}

fn logical_short_circuits(operator: &str, left: &Value) -> bool {
    LogicalOperand::classify(left).short_circuits(operator)
}

fn strict_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::BigInt(a), Value::BigInt(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::StringUnits(a), Value::StringUnits(b)) => a == b,
        (Value::Symbol(a), Value::Symbol(b)) => a == b,
        (Value::Function(a), Value::Function(b)) => a == b,
        (Value::Object(a), Value::Object(b)) => a == b,
        (Value::Host(a), Value::Host(b)) => a == b,
        _ => false,
    }
}

fn loose_equal(left: &Value, right: &Value) -> Result<bool, Fault> {
    if strict_equal(left, right) {
        return Ok(true);
    }
    match (left, right) {
        (Value::Undefined, Value::Null) | (Value::Null, Value::Undefined) => Ok(true),
        (Value::Number(_), Value::String(_)) | (Value::String(_), Value::Number(_)) => {
            Ok(left.script_number()? == right.script_number()?)
        }
        (Value::Bool(_), _) => loose_equal(&Value::Number(left.script_number()?), right),
        (_, Value::Bool(_)) => loose_equal(left, &Value::Number(right.script_number()?)),
        (
            Value::Function(_) | Value::Object(_) | Value::Host(_),
            Value::String(_) | Value::Number(_),
        )
        | (
            Value::String(_) | Value::Number(_),
            Value::Function(_) | Value::Object(_) | Value::Host(_),
        ) => Err(Fault::new("Implicit object conversion is not implemented.")),
        _ => Ok(false),
    }
}

#[cfg(test)]
#[path = "host_apply_tests.rs"]
mod host_apply_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_namespace_nullable_host_conversion_only_preserves_original_nullish_values() {
        struct Strings;
        impl Host for Strings {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Ok(Value::Undefined) }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { unreachable!() }
            fn method(&self, _: u32, name: &str) -> Option<&'static str> {
                match name { "nullable" => Some("nullable"), "plain" => Some("plain"), _ => None }
            }
            fn argument_conversions(&self, _: u32, _: &str, _: &[Value]) -> &'static [(usize, bool)] { &[(0, true)] }
            fn nullable_string_argument(&self, _: u32, method: &str, index: usize) -> bool { method == "nullable" && index == 0 }
            fn required_arguments(&self, _: u32, _: &str) -> usize { 1 }
            fn call(&mut self, _: u32, _: &str, args: &[Value]) -> Result<Value, String> { Ok(args[0].clone()) }
        }
        let mut runtime = Runtime::for_test();
        runtime.define_global("strings", Value::Host(1)).unwrap();
        runtime.execute(r#"
if(strings.nullable(null)!==null||strings.nullable(undefined)!==null||strings.nullable('')!=='')throw 'nullable';
if(strings.plain(null)!=='null'||strings.plain(undefined)!=='undefined')throw 'default hook';
var count=0,v={toString:function(){count++;return null;}};
if(strings.nullable(v)!=='null'||count!==1)throw 'object result';
var marker={},caught=false;try{strings.nullable({toString:function(){throw marker;}});}catch(e){caught=e===marker;}if(!caught)throw 'throw identity';
var errors=0;try{strings.nullable();}catch(e){if(e.name==='TypeError')errors++;}
try{strings.nullable(Symbol());}catch(e){if(e.name==='TypeError')errors++;}
if(errors!==2||strings.nullable(null)!==null)throw 'failure cleanup';
"#, &mut Strings).unwrap();
    }

    #[derive(Default)]
    struct TestHost {
        properties: BTreeMap<(u32, String), Value>,
        records: Vec<String>,
        callbacks: Vec<Value>,
        eval_permission: Option<bool>,
    }

    impl Host for TestHost {
        fn resolve_member_before_call(&self, object: u32) -> bool {
            object == 3
        }
        fn allow_eval(&mut self) -> Result<bool, String> {
            self.eval_permission
                .ok_or_else(|| "Eval is unavailable in this test host.".into())
        }
        fn owns_global_property(&self, property: &str) -> bool {
            matches!(
                property,
                "window" | "self" | "globalThis" | "location" | "onload"
            )
        }
        fn get(&mut self, object: u32, property: &str) -> Result<Value, String> {
            if object == 2 && matches!(property, "window" | "self" | "globalThis") {
                return Ok(Value::Host(2));
            }
            if object == 2 && property == "location" {
                return Err("Location is unavailable in the test host.".into());
            }
            if property == "child" {
                return Ok(Value::Host(object));
            }
            Ok(self
                .properties
                .get(&(object, property.into()))
                .cloned()
                .unwrap_or(Value::Undefined))
        }
        fn set(&mut self, object: u32, property: &str, value: Value) -> Result<(), String> {
            if object == 2 && matches!(property, "window" | "self" | "globalThis" | "location") {
                return Err("The native Window property cannot be replaced.".into());
            }
            self.properties.insert((object, property.into()), value);
            Ok(())
        }
        fn call(&mut self, _object: u32, method: &str, args: &[Value]) -> Result<Value, String> {
            match method {
                "record" => self
                    .records
                    .push(args.first().unwrap_or(&Value::Undefined).to_text()),
                "listen" => self
                    .callbacks
                    .push(args.first().cloned().unwrap_or(Value::Undefined)),
                _ => return Err(format!("Unsupported test method {method}")),
            }
            Ok(Value::Undefined)
        }
    }

    fn test_runtime() -> (Runtime, TestHost) {
        let mut runtime = Runtime::for_test();
        runtime.define_global("test", Value::Host(1)).unwrap();
        runtime.define_global("window", Value::Host(2)).unwrap();
        (runtime, TestHost::default())
    }

    fn result(source: &str) -> String {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute(source, &mut host).unwrap().to_text()
    }

    #[test]
    fn native_named_member_lookup_precedes_arguments_and_preserves_shadowing() {
        let (mut runtime, mut host) = test_runtime();
        runtime.define_global("native", Value::Host(3)).unwrap();
        runtime
            .execute(
                r#"
native.submit=function(){test.record('old');};
native.submit((test.record('argument'), native.submit=function(){test.record('new');}));
native.submit();
native.submit=7;
try{native.submit(test.record('shadow argument'));}catch(e){test.record(e.name);}
"#,
                &mut host,
            )
            .unwrap();
        assert_eq!(
            host.records,
            ["argument", "old", "new", "shadow argument", "TypeError"]
        );
    }

    #[test]
    fn diagnostics_mark_the_actual_token_with_a_bounded_utf8_source_excerpt() {
        let (mut runtime, mut host) = test_runtime();
        let error = runtime
            .execute("var s='é';\nfunction f(x {return x;}", &mut host)
            .unwrap_err();
        assert!(error.contains("line 2, column 14: Expected"), "{error}");
        assert!(error.contains("[here]{return x;}"), "{error}");
        assert!(error.contains("\\nfunction f(x "), "{error}");
        runtime.begin_job();
        let source = format!("/*{}*/ missing();", "z".repeat(10_000));
        let error = runtime.execute(&source, &mut host).unwrap_err();
        assert!(error.contains("missing is not defined"), "{error}");
        assert!(
            error.contains("Source: ...") && error.contains("[here]missing()"),
            "{error}"
        );
        assert!(error.len() < 512, "{error}");
    }

    #[test]
    fn call_diagnostics_preserve_member_getter_and_argument_order() {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute(r#"
var log='',object={number:17};
Object.defineProperty(object,'missing',{configurable:true,get:function(){log+='g';return undefined;}});
try{object.missing((log+='a',Object.defineProperty(object,'missing',{value:function(){log+='wrong';}})));}
catch(e){test.record(log);test.record(e.name);test.record(e.message);}
try{object.number();}catch(e){test.record(e.message);}
object.value={toString:function(){log+='coerced';return 'text';}};
try{object.value();}catch(e){test.record(e.message);}
test.record(log);
"#, &mut host).unwrap();
        assert_eq!(host.records[0], "ga");
        assert_eq!(host.records[1], "TypeError");
        assert_eq!(host.records[2], "The value is not a callable function. Actual value type: undefined. Attempted member \"missing\".");
        assert_eq!(host.records[3], "The value is not a callable function. Actual value type: number. Attempted member \"number\".");
        assert_eq!(host.records[4], "The value is not a callable function. Actual value type: object. Attempted member \"value\".");
        assert_eq!(host.records[5], "ga");
    }

    #[test]
    fn call_diagnostics_leave_callee_throws_unchanged_and_bound_member_text() {
        let (mut runtime, mut host) = test_runtime();
        runtime
            .execute(
                r#"
var marker=new TypeError('The value is not a callable function.'),getterMarker={},log='';
var object={run:function(){throw marker;}};
try{object.run(log+='argument');}catch(e){test.record(e===marker);test.record(e.message);}
Object.defineProperty(object,'unread',{get:function(){throw getterMarker;}});
try{object.unread(log+='wrong');}catch(e){test.record(e===getterMarker);}
test.record(log);
"#,
                &mut host,
            )
            .unwrap();
        assert_eq!(
            host.records,
            [
                "true",
                "The value is not a callable function.",
                "true",
                "argument"
            ]
        );
        runtime.begin_job();
        let source = format!("({{}})['{}']();", "\\u0001".repeat(120));
        let error = runtime.execute(&source, &mut host).unwrap_err();
        assert!(
            error.contains("The value is not a callable function."),
            "{error}"
        );
        assert!(error.contains("Actual value type: undefined."), "{error}");
        assert!(error.contains("Attempted member \"\\u{1}"), "{error}");
        assert!(error.contains("...\"."), "{error}");
        assert!(!error.contains('\u{1}'));
        assert!(error.len() < 512, "{error}");
    }

    #[test]
    fn global_is_finite_accepts_numeric_inputs_for_a_timeline_position() {
        assert_eq!(result(r#"
var playhead=0;
function seek(position) {
    if (!isFinite(position)) return false;
    playhead=Number(position);
    return true;
}
if (!seek('125.5') || playhead!==125.5 || seek(Infinity) || seek('125ms') || playhead!==125.5)
    throw 'finite playhead updates';
for (var value of [0,-0,1e308,null,false,true,'','  ','0x1f','0b10','-2.5e3',[],[7],new Number(4)]) {
    if (isFinite(value)!==true) throw 'expected a finite number after conversion';
}
for (var value of [NaN,Infinity,-Infinity,undefined,'1e999','6tail',{},[1,2]]) {
    if (isFinite(value)!==false) throw 'expected a nonfinite result';
}
if (isFinite()!==false || Number.isFinite('3') || Number.isFinite(null) || !Number.isFinite(3))
    throw 'missing argument and strict Number predicate';
'ok';
"#), "ok");
    }

    #[test]
    fn global_is_finite_uses_one_live_number_conversion_and_preserves_abrupt_values() {
        assert_eq!(result(r#"
var finite=isFinite, seen='', ignored={valueOf:function(){throw 'unused';}}, probe={};
Object.defineProperty(probe,Symbol.toPrimitive,{get:function(){
    seen+='get/';
    return function(hint){if(this!==probe)throw 'wrong this';seen+=hint;return '6';};
}});
if (Number.isFinite(probe) || seen!=='' || !finite.call(ignored,probe,ignored) || seen!=='get/number')
    throw 'number hint, receiver, or argument count';
var fallback={valueOf:function(){
    seen+='v';this.toString=function(){seen+='s';return '3';};return this;
}};
if (!finite(fallback) || seen!=='get/numbervs') throw 'live fallback order';
var marker={},caught=false;
try {finite({valueOf:function(){throw marker;},toString:function(){throw 'unreachable';}});}
catch(e){caught=e===marker;}
if(!caught) throw 'thrown identity';
caught=false;try {finite(Symbol('offset'));} catch(e){caught=e instanceof TypeError;}
if(!caught || Number.isFinite(Symbol('offset'))) throw 'symbol conversion';
'ok';
"#), "ok");
    }

    #[test]
    fn global_is_finite_uses_existing_host_conversion_and_job_budgets() {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute("var caught=false,finished=false;", &mut host).unwrap();
        let error = runtime.execute(
            "try{isFinite(test);}catch(e){caught=true;}finally{finished=true;}", &mut host,
        ).unwrap_err();
        assert!(error.contains("No conversion permission or hook read was granted"), "{error}");
        runtime.begin_job();
        assert_eq!(runtime.execute("!caught&&!finished;", &mut host).unwrap().to_text(), "true");

        // The caller already owns this String. Constrain the remaining work at the
        // native entry so that the argument copy itself must respect the limit.
        runtime.begin_job();
        runtime.constrain_job(usize::MAX, 1, usize::MAX, Instant::now() + Duration::from_secs(5));
        let error = runtime.builtin(Builtin::IsFinite, &[Value::String("1234".into())], &mut host).unwrap_err();
        assert!(error.exception.is_none() && error.message.contains("String/copy work limit"), "{}", error.message);
        runtime.begin_job();
        assert_eq!(runtime.execute("isFinite('20');", &mut host).unwrap().to_text(), "true");
    }

    #[test]
    fn math_constants_methods_coercion_and_ieee_edges() {
        assert_eq!(
            result(
                r#"
var floor=Math.floor;
var checks=[
 typeof Math==='object', typeof floor==='function', floor('3.9')===3,
 Math.PI>3.14159, Math.E>2.71828, Math.LN2>0.693, Math.SQRT2>1.414,
 Object.keys(Math).length===0,
 Math.abs(-3)===3, Math.ceil(-2.2)===-2, Math.trunc(-2.8)===-2,
 Math.round(0.49999999999999994)===0, Math.round(-1.5)===-1,
 1/Math.round(-0.5)===-Infinity, 1/Math.sign(-0)===-Infinity,
 1/Math.min(0,-0)===-Infinity, 1/Math.max(-0,0)===Infinity,
 Math.min()===Infinity, Math.max()===-Infinity,
 isNaN(Math.min(1,NaN)), isNaN(Math.sqrt(-1)), isNaN(Math.pow(1,Infinity)),
 Math.pow(NaN,0)===1, Math.pow(2,10)===1024, isNaN(Math.pow(1,NaN)),
 Math.hypot(3,4)===5, Math.hypot()===0, Math.hypot(NaN,Infinity)===Infinity,
 Math.abs(Math.hypot(3e200,4e200)/5e200-1)<1e-15, Math.clz32(0)===32, Math.clz32(1)===31,
 Math.imul(4294967295,5)===-5, Math.imul(4294967296,5)===0,
 Math.fround(1.337)!==1.337, Math.fround(1.5)===1.5,
 Math.log2(8)===3, Math.log10(100)===2, Math.exp(0)===1,
 Math.sin(0)===0, Math.cos(0)===1, Math.tanh(Infinity)===1,
 Math.atan2(-0,1)===0, 1/Math.atan2(-0,1)===-Infinity,
 isNaN(Math.floor()), Math.floor(null)===0
];
for(var i=0;i<checks.length;i++){if(!checks[i])throw ('Math check '+i);}
var pi=Math.PI; Math.PI=0;
if(Math.PI!==pi)throw Error('constant');
var strictFailure=false;
try { (function(){'use strict';Math.PI=0;})(); } catch(error){strictFailure=error.name==='TypeError';}
strictFailure;
"#
            ),
            "true"
        );
        let (mut runtime, mut host) = test_runtime();
        assert_eq!(
            runtime
                .execute("Math.abs({valueOf:function(){return 1;}})", &mut host)
                .unwrap()
                .to_text(),
            "1"
        );
    }

    #[test]
    fn flat_conditional_dispatchers_run_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let alternatives = format!(
                    "var n=0;{}n;",
                    "(++n<0)?missing():".repeat(512)
                );
                assert_eq!(result(&alternatives), "512");
                let true_arms = format!(
                    "{}7{};",
                    "true?".repeat(512),
                    ":missing()".repeat(512)
                );
                assert_eq!(result(&true_arms), "7");
                let mut dispatch = String::from("var u=511,total=0;for(;u!==-1;)");
                for index in 0..512 {
                    dispatch.push_str(&format!(
                        "u=={index}?(total=u,u=-1):"
                    ));
                }
                dispatch.push_str("missing();total;");
                assert_eq!(result(&dispatch), "511");
                assert_eq!(
                    result("var a=0,b=0;var f=true?(x=>x+1):(x=>x-1);for(var ok=true?'a' in {a:1}:false;ok;ok=false)a=f(4);false?a=8:b=a=7;[a,b].join();"),
                    "7,7"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn deferred_function_bodies_execute_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let inner = format!("{}7{}", "keep(".repeat(20), ")".repeat(20));
                for function in [
                    format!("function(){{return {inner};}}"),
                    format!("()=>{{return {inner};}}"),
                    format!("{{method(){{return {inner};}}}}.method"),
                ] {
                    let source = format!(
                        "function keep(x){{return x;}}var factory={}{}{};factory();",
                        "keep(".repeat(44),
                        function,
                        ")".repeat(44)
                    );
                    assert_eq!(result(&source), "7");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn deferred_function_nesting_and_local_syntax_remain_bounded() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let source = format!(
                    "{}return 7;{}test.record('parsed');",
                    "function nested(){".repeat(MAX_PARSE_DEPTH),
                    "}".repeat(MAX_PARSE_DEPTH)
                );
                let (mut runtime, mut host) = test_runtime();
                runtime.execute(&source, &mut host).unwrap();
                assert_eq!(host.records, ["parsed"]);
                let formerly_rejected = format!(
                    "function unused(){{return {}0{};}}test.record('heap parsed');",
                    "keep(".repeat(MAX_PARSE_DEPTH + 1),
                    ")".repeat(MAX_PARSE_DEPTH + 1)
                );
                runtime.begin_job();
                runtime.execute(&formerly_rejected, &mut host).unwrap();
                assert_eq!(host.records.last().unwrap(), "heap parsed");
                for (source, message) in [
                    (
                        format!(
                            "test.record('early');{}return 7;{}",
                            "function nested(){".repeat(MAX_PARSE_DEPTH + 1),
                            "}".repeat(MAX_PARSE_DEPTH + 1)
                        ),
                        "Function syntax nesting limit reached (64)",
                    ),
                    (
                        format!(
                            "test.record('early');function unused(){{return {}0{};}}",
                            "keep(".repeat(MAX_EXPRESSION_FRAMES + 1),
                            ")".repeat(MAX_EXPRESSION_FRAMES + 1)
                        ),
                        "Pending expression limit reached (4096)",
                    ),
                ] {
                    let (mut runtime, mut host) = test_runtime();
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(error.contains(message), "{error}");
                    assert!(host.records.is_empty());
                    assert!(runtime.programs.is_empty());
                    runtime.begin_job();
                    assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn binding_identifier_leaf_boundary_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                // Retain the original pre-continuation boundary fixtures.
                // Parentheses still disappear from the evaluator AST.
                assert_eq!(
                    result(include_str!(
                        "../tools/fixtures/parser-binding-identifier.js"
                    )),
                    "7"
                );
                // This original indirect-eval fixture creates an uncalled
                // function containing nested calls/groups. No site code runs.
                let (mut runtime, mut host) = test_runtime();
                host.eval_permission = Some(true);
                assert_eq!(
                    runtime
                        .execute(
                            include_str!("../tools/fixtures/parser-binding-identifier-eval.js"),
                            &mut host,
                        )
                        .unwrap()
                        .to_text(),
                    "undefined"
                );
                let call_shape = format!(
                    "var f={}function(value){{return value;}}{};",
                    "keep(".repeat(MAX_PARSE_DEPTH - 3),
                    ")".repeat(MAX_PARSE_DEPTH - 3)
                );
                assert!(Parser::parse(&call_shape, &mut MAX_REGEXP_WORK.clone()).is_ok());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn binding_identifier_leaf_keeps_recursive_limits_and_atomic_rejection() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let mut sources = Vec::new();
                for (groups, parameters) in [
                    (MAX_PARSE_DEPTH - 2, "value"),
                    (MAX_PARSE_DEPTH - 3, "[value]"),
                    (MAX_PARSE_DEPTH - 3, "{value}"),
                    (MAX_PARSE_DEPTH - 3, "value=7"),
                ] {
                    let heap_source = format!(
                        "test.record('early');var f={}function({parameters}){{return 7;}}{};",
                        "(".repeat(groups),
                        ")".repeat(groups)
                    );
                    let (mut runtime, mut host) = test_runtime();
                    runtime.execute(&heap_source, &mut host).unwrap();
                    assert_eq!(host.records, ["early"]);
                    sources.push(format!(
                        "test.record('early');var f={}function({parameters}){{return 7;}}{};",
                        "[".repeat(groups),
                        "]".repeat(groups)
                    ));
                }
                // An assignment reference can recurse through member syntax;
                // it must not use the identifier-binding leaf path.
                let old_reference = format!(
                    "test.record('early');var target={{}};{}[target.value]=[7]{};",
                    "(".repeat(MAX_PARSE_DEPTH - 3),
                    ")".repeat(MAX_PARSE_DEPTH - 3)
                );
                let (mut runtime, mut host) = test_runtime();
                runtime.execute(&old_reference, &mut host).unwrap();
                assert_eq!(host.records, ["early"]);
                sources.push(format!(
                    "test.record('early');var target={{}};{}[target.value]=[7]{};",
                    "[".repeat(MAX_PARSE_DEPTH - 3),
                    "]".repeat(MAX_PARSE_DEPTH - 3)
                ));
                for source in sources {
                    let (mut runtime, mut host) = test_runtime();
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(
                        error.contains("Parser nesting limit reached (64)"),
                        "{error}"
                    );
                    assert!(host.records.is_empty());
                    assert!(runtime.programs.is_empty());
                    runtime.begin_job();
                    assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn binding_identifier_leaf_keeps_active_dynamic_depth() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for active in [0, 1, 4, 16, 32] {
                    for excess in [false, true] {
                        let groups = MAX_PARSE_DEPTH - 3 - active + usize::from(excess);
                        let source = format!(
                            "var f={}function(value){{return value;}}{};",
                            "(".repeat(groups),
                            ")".repeat(groups)
                        );
                        let parsed = Parser::parse_context(
                            &source,
                            &mut MAX_REGEXP_WORK.clone(),
                            false,
                            active,
                        );
                        assert!(parsed.is_ok(), "heap groups with active depth {active}");
                        let native = format!(
                            "var f={}function(value){{return value;}}{};",
                            "[".repeat(groups),
                            "]".repeat(groups)
                        );
                        let parsed = Parser::parse_context(
                            &native, &mut MAX_REGEXP_WORK.clone(), false, active,
                        );
                        if excess {
                            let error =
                                parsed.err().expect("active parser depth must stay bounded");
                            assert!(error.message.contains("Parser nesting limit reached (64)"));
                        } else {
                            assert!(parsed.is_ok(), "active depth {active}");
                        }
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn binding_identifier_leaf_keeps_parameter_early_errors() {
        for function in [
            "function(if){}",
            "function(eval){'use strict';}",
            "function(arguments){'use strict';}",
            "function(value,value){'use strict';}",
        ] {
            let source = format!(
                "test.record('early');var f={}{function}{};",
                "(".repeat(MAX_PARSE_DEPTH - 3),
                ")".repeat(MAX_PARSE_DEPTH - 3)
            );
            let parsed = Parser::parse(&source, &mut MAX_REGEXP_WORK.clone());
            let error = parsed.err().expect("invalid parameter must still fail");
            assert!(matches!(
                error.exception.as_deref(),
                Some(Exception::Error("SyntaxError"))
            ));
            let (mut runtime, mut host) = test_runtime();
            assert!(runtime.execute(&source, &mut host).is_err());
            assert!(host.records.is_empty());
            assert!(runtime.programs.is_empty());
        }
    }

    #[test]
    fn every_deferred_body_is_validated_before_any_script_effects() {
        for source in [
            "test.record('early');function unused(){return 1+;}",
            "test.record('early');function unused(){let x;let x;}",
            "test.record('early');function unused(a,a){'use strict';}",
            "test.record('early');function eval(){'use strict';}",
            "test.record('early');var f=(arguments)=>{'use strict';};",
            "test.record('early');var o={method(eval){'use strict';}};",
            "test.record('early');outer:while(false){function unused(){break outer;}}",
        ] {
            let (mut runtime, mut host) = test_runtime();
            assert!(runtime.execute(source, &mut host).is_err(), "{source}");
            assert!(host.records.is_empty());
            assert!(runtime.programs.is_empty());
        }
    }

    #[test]
    fn browser_deferred_bodies_compile_deep_lexical_nesting_on_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let headers = "function deeper(){".repeat(2_048);
            let ends = "}".repeat(2_048);
            let source = format!("function outer(){{{headers}return 1;{ends}return 17;}}outer();");
            let mut runtime = Runtime::new();
            let mut host = TestHost::default();
            assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "17");
            assert_eq!(runtime.programs.last().unwrap().templates.len(), 2_049);

            let mut runtime = Runtime::new();
            runtime.define_global("test", Value::Host(1)).unwrap();
            let malformed = format!("test.record('before');{headers}let missing=;{ends}");
            let error = runtime.execute(&malformed, &mut host).unwrap_err();
            assert!(error.contains("Expected an expression"), "{error}");
            assert!(host.records.is_empty());
            assert!(runtime.programs.is_empty());
        }).unwrap().join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn browser_native_parser_accepts_finite_recursive_syntax_during_eval() {
        std::thread::Builder::new().stack_size(8 * 1024 * 1024).spawn(|| {
            let mut runtime = Runtime::new();
            let mut host = TestHost { eval_permission: Some(true), ..TestHost::default() };
            let source = format!(
                "function uncalled(){{{}return {}17{};{}}}typeof uncalled;",
                "{".repeat(140), "[".repeat(160), "]".repeat(160), "}".repeat(140),
            );
            runtime.define_global("source", Value::String(source)).unwrap();
            let result = runtime.execute("eval(source);", &mut host).unwrap();
            assert_eq!(result.to_text(), "function");
            assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
        }).unwrap().join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn browser_native_parser_stops_at_measured_headroom_on_a_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            for source in [
                format!("{}1;{}", "{".repeat(12_000), "}".repeat(12_000)),
                format!("{}17{};", "[".repeat(12_000), "]".repeat(12_000)),
                format!("{}17{};", "({a:".repeat(12_000), "})".repeat(12_000)),
            ] {
                let mut runtime = Runtime::new();
                let mut host = TestHost::default();
                let fault = runtime.execute(&source, &mut host).unwrap_err();
                assert!(fault.contains("Native stack headroom exhausted during parser entry"), "{fault}");
                assert!(runtime.programs.is_empty());
                runtime.begin_job();
                assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
                assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
            }
        }).unwrap().join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn browser_native_evaluator_accepts_finite_recursive_statements_and_values() {
        std::thread::Builder::new().stack_size(8 * 1024 * 1024).spawn(|| {
            let left = "[".repeat(160);
            let right = "]".repeat(160);
            for source in [
                format!("var answer=0;{}answer=17;{}answer;", "{".repeat(160), "}".repeat(160)),
                format!("var cursor={left}17{right};for(var i=0;i<160;i++)cursor=cursor[0];cursor;"),
                format!("var {left}answer{right}={left}17{right};answer;"),
            ] {
                let mut runtime = Runtime::new();
                let mut host = TestHost::default();
                assert_eq!(runtime.execute(&source, &mut host).unwrap().to_text(), "17");
                assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
                assert!(runtime.transient_registration_is_empty());
            }
        }).unwrap().join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn browser_native_evaluator_stops_at_measured_headroom_on_a_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut runtime = Runtime::new();
            let mut host = TestHost::default();
            let mut program = Parser::parse("17;", &mut MAX_REGEXP_WORK.clone()).unwrap();
            let mut statement = program.body[0];
            let StmtKind::Expr(mut expression) = program.stmts[statement].kind else { unreachable!() };
            for _ in 0..12_000 {
                let prior = statement;
                statement = program.stmts.len();
                program.stmts.push(Stmt { kind: StmtKind::Block(vec![prior]), at: 0 });
                let prior = expression;
                expression = program.exprs.len();
                program.exprs.push(Expr {
                    kind: ExprKind::Array(vec![Some(Argument { expression: prior, spread: false })]),
                    at: 0, parenthesized: false,
                });
            }
            // The direct arena isolates evaluator pressure from parser admission.
            let program = Rc::new(program);
            runtime.begin_job();
            let result = runtime.statement(&program, statement, 0, &Value::Undefined, &mut host);
            let fault = result.err().expect("the finite native stack must stop statement recursion");
            assert!(fault.message.contains("Native stack headroom exhausted during evaluator entry"), "{}", fault.display());
            assert!(runtime.transient_registration_is_empty());
            runtime.begin_job();
            let fault = runtime.eval(&program, expression, 0, &Value::Undefined, &mut host).unwrap_err();
            assert!(fault.message.contains("Native stack headroom exhausted during evaluator entry"), "{}", fault.display());
            assert!(runtime.transient_registration_is_empty());
            assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
            runtime.begin_job();
            assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
        }).unwrap().join().unwrap();
    }

    #[test]
    fn deferred_eval_bodies_keep_policy_and_active_stack_guards() {
        let inner = format!("{}7{}", "keep(".repeat(20), ")".repeat(20));
        let source = format!(
            "function keep(x){{return x;}}var factory={}function(){{return {inner};}}{};factory();",
            "keep(".repeat(44),
            ")".repeat(44)
        );
        let (mut runtime, mut host) = test_runtime();
        host.eval_permission = Some(false);
        runtime
            .define_global("code", Value::String(source))
            .unwrap();
        runtime
            .execute("try{eval(code);}catch(e){test.record(e.name);}", &mut host)
            .unwrap();
        assert_eq!(host.records, ["EvalError"]);
        assert_eq!(runtime.programs.len(), 1);

        host.eval_permission = Some(true);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("function run(){return eval(code);}run();", &mut host)
                .unwrap()
                .to_text(),
            "7"
        );
        let nested = format!(
            "function inactive(){{return {}1{};}}",
            "(".repeat(60),
            ")".repeat(60)
        );
        // This original grouped body now needs heap frames rather than native
        // recursion, including when eval compiles it on an active stack.
        Parser::parse(&nested, &mut MAX_REGEXP_WORK.clone()).unwrap();
        runtime
            .define_global("nested", Value::String(nested))
            .unwrap();
        host.records.clear();
        runtime.begin_job();
        runtime.execute("eval(nested);", &mut host).unwrap();
        assert!(host.records.is_empty());
        // Native array-literal reentry still shares eval's live stack budget.
        let native = format!(
            "function inactive(){{return {}1{};}}",
            "[".repeat(60), "]".repeat(60)
        );
        Parser::parse(&native, &mut MAX_REGEXP_WORK.clone()).unwrap();
        runtime.define_global("nested", Value::String(native)).unwrap();
        runtime.begin_job();
        let error = runtime
            .execute(
                "try{eval(nested);}catch(e){test.record('caught');}finally{test.record('finally');}",
                &mut host,
            )
            .unwrap_err();
        assert!(
            error.contains("Parser nesting limit reached (64)"),
            "{error}"
        );
        assert!(host.records.is_empty());
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
    }

    #[test]
    fn flat_if_dispatchers_run_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for selected in [0, 512, 1023, -1] {
                    let mut source = format!("var selected={selected},seen=0,answer=-1;");
                    for index in 0..1024 {
                        source.push_str(&format!(
                            "if((++seen,selected=={index}))answer={index};else "
                        ));
                    }
                    source.push_str("answer=-2;[seen,answer].join(':');");
                    let expected = if selected < 0 {
                        "1024:-2".into()
                    } else {
                        format!("{}:{selected}", selected + 1)
                    };
                    assert_eq!(result(&source), expected);
                }
                assert_eq!(
                    result(&format!("{}7;", "if(false);else ".repeat(MAX_IF_BRANCHES))),
                    "7"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn pending_if_bound_is_shared_and_parse_rejection_is_atomic() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for (source, message) in [
                    (
                        format!(
                            "test.record('early');{}0;",
                            "if(false);else ".repeat(MAX_IF_BRANCHES + 1)
                        ),
                        "Pending if-branch limit reached (4096)",
                    ),
                    (
                        format!(
                            "test.record('early');{}{{{}0;}}",
                            "if(false);else ".repeat(2048),
                            "if(false);else ".repeat(2049)
                        ),
                        "Pending if-branch limit reached (4096)",
                    ),
                    (
                        format!("test.record('early');{}0;", "if(true)".repeat(100)),
                        "Parser nesting limit reached (64)",
                    ),
                    (
                        format!(
                            "test.record('early');{}{}0;{}",
                            "if(false);else ".repeat(512),
                            "{".repeat(100),
                            "}".repeat(100)
                        ),
                        "Parser nesting limit reached (64)",
                    ),
                ] {
                    let (mut runtime, mut host) = test_runtime();
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(error.contains(message) || (message == "Parser nesting limit reached (64)" && error.contains("headroom exhausted during parser entry") && error.contains("native stack limit")), "{error}");
                    assert!(host.records.is_empty());
                    assert!(runtime.programs.is_empty());
                    runtime.begin_job();
                    assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn flat_if_execution_exhaustion_is_fatal_and_next_job_recovers() {
        let (mut runtime, mut host) = test_runtime();
        runtime.steps = 300;
        let source = format!(
            "try{{{}test.record('late');}}catch(e){{test.record('caught');}}finally{{test.record('finally');}}",
            "if(false);else ".repeat(1024)
        );
        let error = runtime.execute(&source, &mut host).unwrap_err();
        assert!(error.contains("Execution step limit reached"), "{error}");
        assert!(host.records.is_empty());
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
    }

    #[test]
    fn flat_if_errors_keep_the_selected_branch_source_position() {
        let (mut runtime, mut host) = test_runtime();
        let source = format!("{}if(true) missing();", "if(false);else\n".repeat(256));
        let error = runtime.execute(&source, &mut host).unwrap_err();
        assert!(error.contains("line 257,"), "{error}");
        assert!(error.contains("missing is not defined"), "{error}");
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
    }

    #[test]
    fn pending_expression_bound_is_shared_and_parse_rejection_is_atomic() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                assert_eq!(
                    result(&format!("{}9;", "false?0:".repeat(MAX_EXPRESSION_FRAMES))),
                    "9"
                );
                for source in [
                    format!(
                        "test.record(1);{}9;",
                        "false?0:".repeat(MAX_EXPRESSION_FRAMES + 1)
                    ),
                    format!(
                        "test.record(1);{}({}9);",
                        "false?0:".repeat(2048),
                        "false?0:".repeat(2049)
                    ),
                    format!(
                        "test.record(1);var x;{}1;",
                        "x=".repeat(MAX_EXPRESSION_FRAMES + 1)
                    ),
                ] {
                    let (mut runtime, mut host) = test_runtime();
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(
                        error.contains("Pending expression limit reached (4096)"),
                        "{error}"
                    );
                    assert!(host.records.is_empty());
                    assert!(runtime.programs.is_empty());
                    runtime.begin_job();
                    assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn ordinary_operator_conversion_keeps_hint_order_and_abrupt_completion() {
        assert_eq!(
            result(
                r#"
var log='',left={valueOf(){log+='L';return 2;}};
function right(){log+='R';return {valueOf(){log+='V';return 3;}};}
if(left+right()!==5 || log!=='RLV')throw 'operand order';
var obj={valueOf(){return 7;},toString(){return 'text';}};
if(''+obj!=='7' || String(obj)!=='text' || Number(obj)!==7)throw 'hint';
if('error='+TypeError('bad')!=='error=TypeError: bad')throw 'error text';
if('value='+{}!=='value=[object Object]')throw 'default text';
obj.valueOf=function(){this.toString=function(){return 'changed';};return this;};
if(''+obj!=='changed')throw 'live fallback';
var calls=0,zero={valueOf(){calls++;return 0;}};
if(zero!=false || zero!==zero || zero==null || zero==undefined || zero=={})throw 'equality';
if(calls!==1)throw 'unwanted conversion';
var number={valueOf(){return 4;}};
if(+number!==4 || -number!==-4 || ~number!==-5 || number*2!==8)throw 'numeric';
var old=number++;if(old!==4 || number!==5)throw 'update';
var marker={},bad={valueOf(){throw marker;}};
try{''+bad;throw 'missing throw';}catch(e){if(e!==marker)throw 'wrong throw';}
var invalid={valueOf(){return {};},toString(){return {};}};
try{String(invalid);throw 'missing type error';}catch(e){if(e.name!=='TypeError')throw e;}
true;
"#
            ),
            "true"
        );
    }

    #[test]
    fn error_and_builtin_string_conversion_observe_live_fields() {
        assert_eq!(
            result(
                r#"
var e=Error('old'),log='';
e.name={toString(){log+='n';e.message={toString(){log+='m';return 'changed';}};return 'Custom';}};
if(String(e)!=='Custom: changed' || log!=='nm')throw 'error field order';
if(new Error({toString(){return 'message';}}).message!=='message')throw 'constructor';
if(Error.prototype.toString.call({name:'',message:{toString(){return 'only';}}})!=='only')throw 'borrow';
var text={toString(){return '  12.5tail';}};
if(parseFloat(text)!==12.5 || parseInt(text)!==12)throw 'parse';
if(encodeURIComponent({toString(){return 'a b';}})!=='a%20b')throw 'encode';
if(decodeURIComponent({toString(){return 'a%20b';}})!=='a b')throw 'decode';
if(String.prototype.trim.call({toString(){return '  cat  ';}})!=='cat')throw 'generic';
var seen='';Math.min({valueOf(){seen+='a';return NaN;}},{valueOf(){seen+='b';return 1;}});
if(seen!=='ab')throw 'math order';
true;
"#
            ),
            "true"
        );
    }

    #[test]
    fn conversion_cycles_are_fatal_and_recover_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for source in [
                    "var e=Error();e.name=e;String(e);",
                    "var e=Error();e.message=e;e.toString();",
                    "var o={valueOf(){return +o;}};o+1;",
                    "var o={toString(){return String(o);}};String(o);",
                ] {
                    let (mut runtime, mut host) = test_runtime();
                    let wrapped = format!("try{{{source}}}catch(e){{test.record('caught');}}finally{{test.record('finally');}}");
                    let error = runtime.execute(&wrapped, &mut host).unwrap_err();
                    assert!(error.contains("limit"), "{source}: {error}");
                    assert!(host.records.is_empty());
                    assert_eq!(runtime.call_depth, 0);
                    assert_eq!(runtime.eval_depth, 0);
                    runtime.begin_job();
                    assert_eq!(runtime.execute("String(Error('ok'));", &mut host).unwrap().to_text(), "Error: ok");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn native_primitive_conversion_is_explicit_and_keeps_the_receiver() {
        #[derive(Default)]
        struct NativeText {
            calls: usize,
        }
        impl Host for NativeText {
            fn primitive_conversion(&self, object: u32) -> bool {
                object == 1
            }
            fn method(&self, _: u32, name: &str) -> Option<&'static str> {
                (name == "toString").then_some("toString")
            }
            fn get(&mut self, _: u32, name: &str) -> Result<Value, String> {
                assert_eq!(name, "valueOf");
                Ok(Value::Undefined)
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                unreachable!()
            }
            fn call(&mut self, object: u32, method: &str, _: &[Value]) -> Result<Value, String> {
                assert_eq!((object, method), (1, "toString"));
                self.calls += 1;
                Ok(Value::String("https://example.test/".into()))
            }
        }
        let mut runtime = Runtime::for_test();
        runtime.define_global("native", Value::Host(1)).unwrap();
        runtime.define_global("opaque", Value::Host(2)).unwrap();
        let mut host = NativeText::default();
        assert_eq!(runtime.execute("String(native)==='https://example.test/' && ''+native==='https://example.test/' && native=='https://example.test/';", &mut host).unwrap().to_text(), "true");
        assert_eq!(host.calls, 3);
        runtime.begin_job();
        let error = runtime.execute("''+opaque;", &mut host).unwrap_err();
        assert!(
            error.contains("Implicit native host object conversion"),
            "{error}"
        );
        assert_eq!(host.calls, 3);
    }

    #[test]
    fn math_random_uses_an_explicit_host_capability_and_rejects_bad_output() {
        struct Entropy(f64);
        impl Host for Entropy {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                unreachable!()
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                unreachable!()
            }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
                unreachable!()
            }
            fn random(&mut self) -> Result<f64, String> {
                Ok(self.0)
            }
        }
        let mut runtime = Runtime::for_test();
        assert_eq!(
            runtime
                .execute("var next=Math.random;next();", &mut Entropy(0.125))
                .unwrap()
                .to_text(),
            "0.125"
        );
        for invalid in [1.0, -0.1, f64::NAN, f64::INFINITY] {
            runtime.begin_job();
            assert!(runtime
                .execute("Math.random();", &mut Entropy(invalid))
                .unwrap_err()
                .contains("invalid Math.random"));
        }
    }

    #[test]
    fn delete_preserves_array_holes_constants_and_strict_mode() {
        assert_eq!(
            result(
                r#"
var a=[1,2,3],o={first:1,last:2},count=0;
if(!delete o.first || 'first' in o || Object.keys(o).join(',')!=='last')throw 'object delete';
if(!delete a[1] || a.length!==3 || 1 in a || a.join(',')!=='1,,3')throw 'array delete';
if(delete a.length || delete Math.PI || delete /x/.lastIndex)throw 'nonconfigurable';
if(!delete (count+=1) || count!==1 || !delete unknownVariable || delete count)throw 'evaluation';
if(delete 'abc'[0] || !delete 'abc'[9])throw 'primitive';
var caught=0;
try{(function(){'use strict';delete Math.PI;})();}catch(e){if(e.name==='TypeError')caught++;}
try{delete null.x;}catch(e){if(e.name==='TypeError')caught++;}
caught;
"#
            ),
            "2"
        );
        let (mut runtime, mut host) = test_runtime();
        assert!(runtime
            .execute("'use strict';delete missing;", &mut host)
            .unwrap_err()
            .contains("unqualified identifier"));
        runtime.begin_job();
        runtime.bind_browser_global(2).unwrap();
        assert_eq!(runtime.execute("window.temp=4; var fixed=5; (delete window.temp)+':'+(typeof window.temp==='undefined')+':'+(delete window.fixed)+':'+fixed;",&mut host).unwrap().to_text(),"true:true:false:5");
    }

    #[test]
    fn string_character_reads_match_original_unicode_reference_fixture() {
        assert_eq!(
            result(include_str!("../tools/fixtures/utf16-character-reads.js")),
            "ok"
        );
    }

    #[test]
    fn string_character_scan_charges_only_visited_scalars_before_decoding() {
        let mut runtime = Runtime::for_test();
        for (method, position, expected, charged) in [
            ("charCodeAt", 0.0, 65.0, 1),
            ("charCodeAt", 1.0, 233.0, 3),
            ("charCodeAt", 2.0, 55357.0, 7),
            ("charCodeAt", 3.0, 56832.0, 7),
            ("charCodeAt", 4.0, 90.0, 8),
            ("codePointAt", 2.0, 128512.0, 7),
            ("codePointAt", 3.0, 56832.0, 7),
        ] {
            runtime.begin_job();
            let value = runtime
                .string_character("Aé😀Z", method, &[Value::Number(position)])
                .unwrap();
            assert_eq!(value.to_number(), expected, "{method}({position})");
            assert_eq!(runtime.job_usage().1, charged);
            assert_eq!(runtime.job_usage().0, 0);
        }
        runtime.begin_job();
        runtime.copy_bytes = 2;
        let error = runtime
            .string_character("Aé😀Z", "charCodeAt", &[Value::Number(1.0)])
            .unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.display().contains("String/copy work limit"));
        assert_eq!(runtime.copy_bytes, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .string_character("A", "charAt", &[])
                .unwrap()
                .to_text(),
            "A"
        );
    }

    #[test]
    fn string_character_ranges_and_surrogate_boundaries_remain_explicit() {
        let mut runtime = Runtime::for_test();
        for position in [-1.0, f64::NEG_INFINITY, f64::INFINITY, f64::MAX, 4.0] {
            for method in ["charAt", "charCodeAt", "codePointAt"] {
                runtime.begin_job();
                let result = runtime
                    .string_character("😀", method, &[Value::Number(position)])
                    .unwrap();
                assert_eq!(runtime.job_usage().1, 0, "{method}({position})");
                match method {
                    "charAt" => assert_eq!(result.to_text(), ""),
                    "codePointAt" => assert!(matches!(result, Value::Undefined)),
                    _ => assert!(result.to_number().is_nan()),
                }
            }
        }
        // These positions are below the byte bound but past the UTF-16 end.
        for position in [2.0, 3.0] {
            runtime.begin_job();
            assert_eq!(
                runtime
                    .string_character("😀", "charAt", &[Value::Number(position)])
                    .unwrap()
                    .to_text(),
                ""
            );
            assert_eq!(runtime.job_usage().1, 4);
        }
        for (position, expected) in [(0.0, 0xd83d), (1.0, 0xde00)] {
            runtime.begin_job();
            let value = runtime
                .string_character("😀", "charAt", &[Value::Number(position)])
                .unwrap();
            assert!(matches!(value, Value::StringUnits(units) if units == [expected]));
        }
        for position in [f64::NAN, -0.0, -0.5] {
            runtime.begin_job();
            assert_eq!(
                runtime
                    .string_character("abc", "charAt", &[Value::Number(position)])
                    .unwrap()
                    .to_text(),
                "a"
            );
        }
    }

    #[test]
    fn string_index_original_semantic_fixture() {
        assert_eq!(
            result(include_str!("../tools/fixtures/primitive-string-index-reads.js")),
            "ok"
        );
    }

    #[test]
    fn string_index_scalar_scan_charges_visited_bytes_and_stops_at_target() {
        let mut runtime = Runtime::for_test();
        for (index, expected, bytes) in [
            (0, Some(65), 1),
            (1, Some(233), 3),
            (2, Some(55357), 7),
            (3, Some(56832), 7),
            (4, Some(90), 8),
            (5, None, 8),
            (7, None, 8),
            (8, None, 0),
            (usize::MAX, None, 0),
        ] {
            runtime.begin_job();
            let unit = runtime
                .string_scalar_at("Aé😀Z", index)
                .unwrap()
                .map(|(character, offset)| {
                    let mut buffer = [0; 2];
                    character.encode_utf16(&mut buffer)[offset]
                });
            assert_eq!(unit, expected, "index {index}");
            assert_eq!(runtime.job_usage().1, bytes, "index {index}");
            assert_eq!(runtime.job_usage().0, 0);
        }
        runtime.begin_job();
        assert_eq!(runtime.string_scalar_at("", 0).unwrap(), None);
        assert_eq!(runtime.job_usage().1, 0);
    }

    #[test]
    fn string_index_full_conversion_oracle_preserves_values_steps_and_errors() {
        // The former implementation is a test-only value/accounting oracle.
        // It never replaces an execution mode or weakens a production guard.
        fn original(
            runtime: &mut Runtime,
            receiver: &Value,
            property: &str,
            host: &mut TestHost,
        ) -> Result<Value, Fault> {
            runtime.step()?;
            let Value::String(text) = receiver else {
                unreachable!()
            };
            runtime.work(text.len().saturating_mul(2))?;
            let units: Vec<_> = text.encode_utf16().collect();
            let index = property_index(property).unwrap();
            if index < units.len() {
                runtime.string_from_units(&units[index..index + 1])
            } else {
                runtime.get_ordinary_property_with_receiver(
                    Ordinary::Object(runtime.string_prototype.unwrap()),
                    property,
                    receiver,
                    host,
                )
            }
        }
        for text in ["", "A", "ABCD", "éΩ中", "😀", "Aé😀Z", "\0\u{0800}Z"] {
            for index in [0, 1, 2, 3, 4, 7, text.len(), text.len() + 1] {
                let property = index.to_string();
                let receiver = Value::String(text.into());
                let mut before = Runtime::for_test();
                let mut after = Runtime::for_test();
                before.begin_job();
                after.begin_job();
                let expected = original(&mut before, &receiver, &property, &mut TestHost::default())
                    .map(|value| value.to_text())
                    .map_err(|error| error.display());
                let actual = after.read_property(&receiver, &property, &mut TestHost::default())
                    .map(|value| value.to_text())
                    .map_err(|error| error.display());
                assert_eq!(actual, expected, "{text:?}[{index}]");
                assert_eq!(after.job_usage().0, before.job_usage().0);
                let mut prefix_bytes = 0;
                if index < text.len() {
                    let mut units = 0;
                    for character in text.chars() {
                        prefix_bytes += character.len_utf8();
                        units += character.len_utf16();
                        if units > index {
                            break;
                        }
                    }
                }
                assert_eq!(
                    before.job_usage().1 - after.job_usage().1,
                    text.len() * 2 - prefix_bytes,
                    "{text:?}[{index}]"
                );
            }
        }
    }

    #[test]
    fn string_index_original_probe_counts_and_unchanged_controls() {
        for (bytes, expected_work) in [(1024, 1077), (8192, 8245)] {
            let mut runtime = Runtime::for_test();
            runtime.define_global("payload", Value::String("A".repeat(bytes))).unwrap();
            runtime.begin_job();
            // Exact bytes of the retained index-1024.js/index-8192.js fixture.
            let source = "payload[0];\n";
            assert_eq!(
                runtime.execute(source, &mut TestHost::default()).unwrap().to_text(),
                "A"
            );
            // Four flat AST records are scanned before publication.
            assert_eq!(runtime.job_usage(), (17, expected_work, 0));
        }
        for (source, work) in [
            ("payload;\n", 8238),
            ("payload.length;\n", 8269),
            ("payload['length'];\n", 8272),
            ("payload?.length;\n", 16462),
            ("payload.charCodeAt(0);\n", 8293),
        ] {
            let mut runtime = Runtime::for_test();
            runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
            runtime.begin_job();
            runtime.execute(source, &mut TestHost::default()).unwrap();
            assert_eq!(runtime.job_usage().1, work, "{source}");
        }
    }

    #[test]
    fn string_index_scan_and_result_charges_precede_success() {
        let mut runtime = Runtime::for_test();
        let receiver = Value::String("Aé😀Z".into());
        for (property, budget) in [("0", 0), ("0", 1), ("0", 2), ("1", 2), ("1", 4)] {
            runtime.begin_job();
            runtime.copy_bytes = budget;
            let error = runtime
                .read_property(&receiver, property, &mut TestHost::default())
                .unwrap_err();
            assert!(error.exception.is_none());
            assert!(error.display().contains("String/copy work limit"));
            assert_eq!(runtime.copy_bytes, 0);
        }
        runtime.begin_job();
        runtime.copy_bytes = 3;
        assert_eq!(
            runtime.read_property(&receiver, "0", &mut TestHost::default()).unwrap().to_text(),
            "A"
        );
        assert_eq!(runtime.copy_bytes, 0);
        // Missing-index fallback still charges its getter receiver copy before
        // calling user code. Avoid mistaking an empty result for a missing index.
        runtime.begin_job();
        runtime.execute(
            "var getterRuns=0;Object.defineProperty(String.prototype,'99999',{configurable:true,get:function(){getterRuns++;return 7;}});",
            &mut TestHost::default(),
        ).unwrap();
        let large = Value::String("A".repeat(8192));
        runtime.begin_job();
        runtime.copy_bytes = 100;
        let error = runtime.read_property(&large, "99999", &mut TestHost::default()).unwrap_err();
        assert!(error.display().contains("String/copy work limit"));
        assert_eq!(runtime.copy_bytes, 0);
        runtime.begin_job();
        assert_eq!(runtime.execute("getterRuns;", &mut TestHost::default()).unwrap().to_number(), 0.0);
        assert_eq!(runtime.read_property(&large, "99999", &mut TestHost::default()).unwrap().to_number(), 7.0);
        assert_eq!(runtime.execute("getterRuns;", &mut TestHost::default()).unwrap().to_number(), 1.0);
    }

    #[test]
    fn string_index_shared_budget_cancels_jobs_and_recovers_on_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for microtask in [false, true] {
                    for iterations in [512, 1536] {
                        let mut runtime = Runtime::for_test();
                        let mut host = TestHost::default();
                        runtime.define_global("payload", Value::String("A".repeat(8192))).unwrap();
                        let setup = "var completed=0,total=0,tail=0,sentinel=0;";
                        let scan = format!("while(completed<{iterations}){{if(payload[completed]!=='A')throw 'index';total++;completed++;}}tail=1;");
                        let queued = "Promise.resolve().then(function(){sentinel++;});";
                        let source = if microtask {
                            format!("{setup}Promise.resolve().then(function(){{{scan}}});{queued}")
                        } else {
                            format!("{setup}{queued}{scan}")
                        };
                        runtime.begin_job();
                        let execution = runtime.execute(&source, &mut host).map(|_| ());
                        let completion = if microtask || execution.is_ok() {
                            execution.unwrap();
                            runtime.microtask_checkpoint(&mut host)
                                .map(|messages| assert!(messages.is_empty()))
                        } else {
                            execution
                        };
                        if iterations == 512 {
                            completion.unwrap();
                            assert!(runtime.job_usage().1 < MAX_COPY_BYTES);
                        } else {
                            let error = completion.unwrap_err();
                            assert!(error.contains("String/copy work limit"), "{error}");
                            assert_eq!(runtime.job_usage().1, MAX_COPY_BYTES);
                        }
                        assert!(!runtime.has_microtasks());
                        assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
                        runtime.begin_job();
                        let completed = runtime.execute("completed;", &mut host).unwrap().to_number();
                        let total = runtime.execute("total;", &mut host).unwrap().to_number();
                        let tail = runtime.execute("tail;", &mut host).unwrap().to_number();
                        let sentinel = runtime.execute("sentinel;", &mut host).unwrap().to_number();
                        if iterations == 512 {
                            assert_eq!((completed, total, tail, sentinel), (512.0, 512.0, 1.0, 1.0));
                        } else {
                            assert!(completed > 0.0 && completed < 1536.0);
                            assert!(total == completed || total == completed + 1.0);
                            assert_eq!((tail, sentinel), (0.0, 0.0));
                        }
                        assert_eq!(runtime.execute("21+21;", &mut host).unwrap().to_number(), 42.0);
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn string_index_exact_surrogate_keys_preserve_callbacks_and_scalar_only_refusal_cancels_them() {
        let mut runtime = Runtime::for_test();
        let mut host = TestHost::default();
        runtime.begin_job();
        runtime.execute(
            "var seen=0,tail=0;Promise.resolve().then(function(){seen='😀'[0].charCodeAt(0);});Promise.resolve().then(function(){tail++;});",
            &mut host,
        ).unwrap();
        runtime.microtask_checkpoint(&mut host).unwrap();
        assert_eq!(runtime.execute("seen===55357&&tail===1;", &mut host).unwrap().to_text(), "true");
        runtime.begin_job();
        runtime.execute(
            "var caught=0,target={};tail=0;Promise.resolve().then(function(){try{target['😀'[0]]=1;}catch(e){caught++;}finally{caught+=10;}});Promise.resolve().then(function(){tail++;});",
            &mut host,
        ).unwrap();
        assert!(runtime.microtask_checkpoint(&mut host).unwrap().is_empty());
        assert_eq!(runtime.execute("caught===10&&tail===1&&target['😀'[0]]===1&&target['\\ufffd']===undefined;", &mut host).unwrap().to_text(), "true");
        runtime.begin_job();
        runtime.execute(
            "caught=0;tail=0;Promise.resolve().then(function(){try{encodeURIComponent('😀'[0]);}catch(e){caught++;}finally{caught+=10;}});Promise.resolve().then(function(){tail++;});",
            &mut host,
        ).unwrap();
        let error = runtime.microtask_checkpoint(&mut host).unwrap_err();
        assert!(error.contains("unpaired UTF-16"), "{error}");
        assert!(!runtime.has_microtasks());
        assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
        runtime.begin_job();
        assert_eq!(runtime.execute("caught+tail;", &mut host).unwrap().to_number(), 0.0);
        assert_eq!(runtime.execute("'😀'.codePointAt(0);", &mut host).unwrap().to_number(), 128512.0);
    }

    #[test]
    fn string_split_and_slices_handle_limits_captures_and_utf16_positions() {
        assert_eq!(
            result(
                r#"
var checks=[
'a,,b,'.split(',').join('|')==='a||b|', 'abc'.split().length===1,
'abc'.split(undefined,0).length===0, ''.split('').length===0,
'abc'.split('',2).join(',')==='a,b', 'xnullz'.split(null).join('-')==='x-z',
'ab'.split(',',-1).length===1, 'a,b,c'.split(',',2).join('|')==='a|b',
'a1b2c'.split(/(\d)/).join('|')==='a|1|b|2|c',
'a,b;c'.split(/(,)|(\;)/)[2]===undefined,
'ab'.split(/(?:)/).join('|')==='a|b', 'abc'.split(/x*/).join('|')==='a|b|c',
''.split(/a/).length===1, ''.split(/(?:)/).length===0,
'abc'.slice(-2)==='bc', 'abc'.slice(2,1)==='', 'abc'.substring(2,1)==='b',
'abc'.substring(-2,2)==='ab', 'abc'.slice(0,undefined)==='abc',
'abc'.charAt(1)==='b', 'abc'.charAt(9)==='', 'abc'.charCodeAt(1)===98,
isNaN('abc'.charCodeAt(9)), '😀'.charCodeAt(0)===55357,
'😀'.codePointAt(0)===128512, '😀'.codePointAt(1)===56832,
'😀'.codePointAt(2)===undefined, '😀x'.slice(2)==='x'
];
for(var i=0;i<checks.length;i++)if(!checks[i])throw ('String check '+i);
var separator=/,/g;separator.lastIndex=2;
'a,b,c'.split(separator);separator.lastIndex;
"#
            ),
            "2"
        );
        let (mut runtime, mut host) = test_runtime();
        assert!(runtime
            .execute("'😀'.split('');", &mut host)
            .unwrap_err()
            .contains("surrogate"));
        runtime.begin_job();
        let input = format!("'{}'.split('');", "x".repeat(MAX_ARRAY_LENGTH + 1));
        assert!(runtime
            .execute(&input, &mut host)
            .unwrap_err()
            .contains("Array length limit"));
        runtime.begin_job();
        let input = format!("'{}'.split('',3).length;", "x".repeat(MAX_ARRAY_LENGTH + 1));
        assert_eq!(runtime.execute(&input, &mut host).unwrap().to_text(), "3");
    }

    #[test]
    fn instanceof_follows_intrinsic_and_ordinary_prototype_chains() {
        assert_eq!(
            result(
                r#"
var checks=[[] instanceof Array, new Array(3) instanceof Object,
({}) instanceof Object, !(3 instanceof Number), !('s' instanceof String),
!/a/.test('b'), /a/ instanceof RegExp, /a/ instanceof Object,
new TypeError('x') instanceof TypeError, new TypeError('x') instanceof Error,
!(new TypeError('x') instanceof RangeError), !(null instanceof Object),
Math.floor instanceof Object, [] instanceof Object && !(new Object() instanceof Array)];
for(var i=0;i<checks.length;i++)if(!checks[i])throw Error('instance '+i);
var caught=0;try{({}) instanceof 3;}catch(e){if(e.name==='TypeError')caught++;}
try{({}) instanceof (()=>0);}catch(e){if(e.name==='TypeError')caught++;}
caught;
"#
            ),
            "2"
        );
        assert_eq!(
            result("function Custom(){};({}) instanceof Custom;"),
            "false"
        );
        assert_eq!(
            result("function Custom(){};new Custom() instanceof Custom;"),
            "true"
        );
    }

    #[test]
    fn sparse_padding_is_bounded_without_storing_thousands_of_holes() {
        let (mut runtime, mut host) = test_runtime();
        let properties = runtime.properties;
        let retained = runtime.value_bytes-runtime.programs.capacity()*std::mem::size_of::<Rc<Program>>()-runtime.programs.iter().map(|p|p.retained_bytes.get()).sum::<usize>();
        let value = runtime
            .execute(r"Array(7824).join('\n');", &mut host)
            .unwrap();
        assert_eq!(value.to_text().len(), 7823);
        assert_eq!(runtime.properties, properties);
        assert_eq!(runtime.value_bytes-runtime.programs.capacity()*std::mem::size_of::<Rc<Program>>()-runtime.programs.iter().map(|p|p.retained_bytes.get()).sum::<usize>(),retained);
        assert!(
            runtime.steps > MAX_STEPS / 2,
            "Holes should not consume one full property lookup each"
        );
        runtime.begin_job();
        assert!(runtime
            .execute("Array(8193);", &mut host)
            .unwrap_err()
            .contains("Array length limit"));
        runtime.begin_job();
        assert!(runtime
            .execute("Array(8192).join('012345678901234567890');", &mut host)
            .unwrap_err()
            .contains("String limit"));
        assert_eq!(
            result("Array.prototype[1]='inherited';Array(3).join('|');"),
            "|inherited|"
        );
    }

    #[test]
    fn data_descriptors_snapshot_fields_and_enforce_inherited_readonly_values() {
        assert_eq!(result("let t={};Object.defineProperties(t,{value:{value:7},result:t});t.value+':'+t.result;"), "7:undefined");
        assert_eq!(result("let p={};Object.defineProperty(p,'0',{value:8});let a=[];Object.setPrototypeOf(a,p);let kind;try{Array.prototype.push.call(a,1);}catch(e){kind=e.name;}kind+':'+a.length+':'+a[0];"), "TypeError:0:8");
        assert_eq!(result("function Box(x){this.x=x;}Box.prototype.read=function(){return this.x;};let a=new Box(4);a.read()+':'+(a instanceof Box)+':'+(Object.getPrototypeOf(a)===Box.prototype);"), "4:true:true");
    }

    #[test]
    fn script_global_data_descriptors_preserve_declared_var_rules() {
        let (mut runtime, mut host) = test_runtime();
        runtime.bind_browser_global(2).unwrap();
        runtime.execute("var declared=1;let d=Object.getOwnPropertyDescriptor(window,'declared');test.record(d.value);test.record(d.enumerable);test.record(d.writable);test.record(d.configurable);Object.defineProperty(window,'declared',{value:2,writable:false});declared=3;test.record(declared);test.record(delete window.declared);",&mut host).unwrap();
        assert_eq!(host.records, ["1", "true", "true", "false", "2", "false"]);
        runtime.begin_job();
        runtime.execute("try{Object.defineProperty(window,'declared',{configurable:true});}catch(e){test.record(e.name);}test.record(declared);",&mut host).unwrap();
        assert_eq!(&host.records[6..], ["TypeError", "2"]);
    }

    #[test]
    fn text_encode_into_follows_webidl_string_conversion_order() {
        // Node's util implementation rejects non-string encodeInto sources.
        // WHATWG Encoding declares USVString, so ordinary conversion runs first.
        let (mut runtime, mut host) = test_runtime();
        runtime.execute("let e=new TextEncoder(),n=0,s={toString(){n++;return 'abc';}},a=new Uint8Array(4);test.record(e.encodeInto(s,a).written);test.record(a.join());try{e.encodeInto(s,{});}catch(e){test.record(e.name);}test.record(n);",&mut host).unwrap();
        assert_eq!(host.records, ["3", "97,98,99,0", "TypeError", "2"]);
    }

    #[test]
    fn unused_arguments_stay_lazy_and_reused_calls_release_all_seed_bytes() {
        let (mut runtime, mut host) = test_runtime();
        let function = runtime
            .execute("function f(a){return a;}f;", &mut host)
            .unwrap();
        let objects = runtime.objects.len();
        // Admit activation backing once; unused arguments must create no object.
        runtime.invoke(function.clone(), &[Value::Undefined], Value::Undefined, &mut host).unwrap();
        assert_eq!(runtime.objects.len(), objects);
        let bytes = runtime.value_bytes;
        let bindings = runtime.bindings;
        let value = Value::String("x".repeat(1024));
        for _ in 0..5000 {
            runtime.begin_job();
            assert_eq!(
                runtime
                    .invoke(
                        function.clone(),
                        std::slice::from_ref(&value),
                        Value::Undefined,
                        &mut host
                    )
                    .unwrap()
                    .to_text()
                    .len(),
                1024
            );
            assert_eq!(runtime.objects.len(), objects);
            assert_eq!(runtime.value_bytes, bytes);
            assert_eq!(runtime.bindings, bindings);
        }
        assert!(runtime.environments.len() < 8);
    }

    #[test]
    fn overwriting_pending_arguments_releases_seed_before_quota_check() {
        let (mut runtime, mut host) = test_runtime();
        let function = runtime
            .execute("function f(s){arguments=s;return 7;}f;", &mut host)
            .unwrap();
        // Pre-admit reusable activation backing before filling payload storage.
        runtime.invoke(function.clone(), &[Value::String(String::new())], Value::Undefined, &mut host).unwrap();
        let size = MAX_STRING_BYTES;
        let target = MAX_RETAINED_VALUES - 2 * size;
        let mut serial = 0;
        while runtime.value_bytes < target {
            let length = (target - runtime.value_bytes).min(size);
            runtime.begin_job();
            runtime
                .define_global(&format!("fill{serial}"), Value::String("f".repeat(length)))
                .unwrap();
            serial += 1;
        }
        let before = runtime.value_bytes;
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(
                    function,
                    &[Value::String("s".repeat(size))],
                    Value::Undefined,
                    &mut host
                )
                .unwrap()
                .to_text(),
            "7"
        );
        assert_eq!(runtime.value_bytes, before);
    }

    #[test]
    fn mapped_arguments_keep_the_environment_alive_until_detached() {
        let (mut runtime, mut host) = test_runtime();
        let function = runtime
            .execute("function f(a){return arguments;}f;", &mut host)
            .unwrap();
        let Value::Object(id) = runtime
            .invoke(
                function,
                &[Value::String("initial".into())],
                Value::Undefined,
                &mut host,
            )
            .unwrap()
        else {
            panic!("object required")
        };
        let environment = runtime.objects[id].arguments.as_ref().unwrap().environment;
        assert!(runtime.environments[environment].is_some());
        runtime.begin_job();
        runtime
            .store_binding(environment, "a", Value::String("updated".into()), false)
            .unwrap();
        assert_eq!(runtime.get_property(id, "0").unwrap().to_text(), "updated");
        runtime.disconnect_arguments(id).unwrap();
        runtime.collect_environments();
        assert!(runtime.environments[environment].is_none());
        assert_eq!(runtime.get_property(id, "0").unwrap().to_text(), "updated");
    }

    #[test]
    fn strict_arguments_callee_setter_throws_even_from_sloppy_code() {
        // ECMA CreateUnmappedArgumentsObject installs %ThrowTypeError% as
        // both getter and setter. Node/V8's sloppy assignment optimization
        // currently ignores this particular setter; the intrinsic call and
        // strict assignment remain part of the independent Node comparisons.
        assert_eq!(result("var a=(function(){'use strict';return arguments;})();try{a.callee=1;}catch(e){e.name;}"), "TypeError");
    }

    #[test]
    fn binary_and_collection_callbacks_stop_on_a_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024*1024).spawn(|| {
            for source in [
                "let a=new Uint8Array(1),v={valueOf(){a[0]=v;return 1;}};try{a[0]=v;}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let a=new Uint8Array([2,1]);function f(){return a.sort(f);}try{a.sort(f);}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let s=new Set([1]);function f(){s.forEach(f);}try{s.forEach(f);}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let e=new TextEncoder(),s={toString(){return e.encode(s);}};try{e.encode(s);}catch(e){test.record('caught');}finally{test.record('finally');}",
            ] {
                let (mut runtime,mut host)=test_runtime();
                let error=runtime.execute(source,&mut host).unwrap_err();
                assert!(error.contains("limit reached") || error.contains("native stack limit"),"{error}");
                assert!(host.records.is_empty());
                assert_eq!(runtime.call_depth,0);
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn script_owned_global_descriptors_keep_configurability_and_native_ownership() {
        let (mut runtime, mut host) = test_runtime();
        runtime.define_global("window", Value::Host(2)).unwrap();
        runtime.bind_browser_global(2).unwrap();
        assert_eq!(runtime.execute("Object.defineProperty(window,'custom',{value:7});(delete window.custom)+':'+window.custom;", &mut host).unwrap().to_text(), "false:7");
        runtime.begin_job();
        assert_eq!(runtime.execute("function Token(){return this instanceof Token;}Token()+':'+(new Token() instanceof Token);", &mut host).unwrap().to_text(), "false:true");
        runtime.begin_job();
        let error = runtime.execute("try{Object.defineProperty(window,'window',{value:3});}catch(e){test.record('caught');}finally{test.record('finally');}", &mut host).unwrap_err();
        assert!(error.contains("native or declared global"), "{error}");
        assert!(host.records.is_empty());
        runtime.begin_job();
        let error = runtime
            .execute("Object.setPrototypeOf(window,{});", &mut host)
            .unwrap_err();
        assert!(error.contains("Native host"), "{error}");
    }

    #[test]
    fn prototype_cycles_and_recursive_calls_stop_on_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            assert_eq!(result("let a={},b=Object.create(a),caught=false;try{Object.setPrototypeOf(a,b);}catch(e){caught=e.name==='TypeError';}caught&&Object.getPrototypeOf(a)===Object.prototype;"), "true");
            for source in [
                "function F(){return new F();}try{new F();}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let a=[];a[0]=Function.prototype.apply;a[1]=a;try{Function.prototype.apply.apply(Function.prototype.apply,a);}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let o={};try{for(let i=0;i<5000;i++){o=Object.create(o);o.missing;}}catch(e){test.record('caught');}finally{test.record('finally');}",
            ] {
                let (mut runtime, mut host) = test_runtime();
                assert!(runtime.execute(source, &mut host).is_err(), "{source}");
                assert!(host.records.is_empty(), "Fatal limit escaped: {source}");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn switch_lexical_environment_and_closures_survive_jobs() {
        let (mut runtime, mut host) = test_runtime();
        runtime
            .execute(
                "var f;switch(1){case 1:let value=7;f=()=>++value;break;default:var never=9;}",
                &mut host,
            )
            .unwrap();
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("f()+':'+f()+':'+typeof never;", &mut host)
                .unwrap()
                .to_text(),
            "8:9:undefined"
        );
        assert_eq!(result("let x=1;switch(x){case 1:let x=3;x;}"), "3");
        assert_eq!(result("let s='';for(var i=0;i<3;i++){switch(i){case 0:continue;case 1:s+='a';break;default:s+='b';}s+='x';}s;"), "axbx");
    }

    #[test]
    fn json_parse_rolls_back_unpublished_allocations_without_refunding_work() {
        let (mut runtime, mut host) = test_runtime();
        let checkpoint = (
            runtime.objects.len(),
            runtime.properties,
            runtime.value_bytes,
        );
        let work = runtime.copy_bytes;
        let steps = runtime.steps;
        for _ in 0..20 {
            let error = runtime
                .json_parse(&[Value::String("{\"a\":\"value\",\"b\":[1,2],}".into())])
                .unwrap_err();
            assert!(error.display().contains("quoted string"));
            assert_eq!(
                (
                    runtime.objects.len(),
                    runtime.properties,
                    runtime.value_bytes
                ),
                checkpoint
            );
        }
        assert!(runtime.copy_bytes < work);
        assert!(runtime.steps < steps);
        let oversized = format!("[{}]", vec!["{}"; MAX_OBJECTS].join(","));
        let error = runtime.json_parse(&[Value::String(oversized)]).unwrap_err();
        assert!(error.display().contains("Object identity limit"));
        assert_eq!(
            (
                runtime.objects.len(),
                runtime.properties,
                runtime.value_bytes
            ),
            checkpoint
        );
        runtime.begin_job();
        let value = runtime
            .json_parse(&[Value::String("{\"keep\":1}".into())])
            .unwrap();
        assert_eq!(
            runtime
                .json_stringify(&[value], &mut host)
                .unwrap()
                .to_text(),
            "{\"keep\":1}"
        );
    }

    #[test]
    fn json_copy_and_output_limits_are_shared_with_the_document_job() {
        let (mut runtime, mut host) = test_runtime();
        let argument = Value::String("x".repeat(MAX_STRING_BYTES / 2));
        let error = loop {
            match runtime.json_stringify(std::slice::from_ref(&argument), &mut host) {
                Ok(_) => (),
                Err(error) => break error,
            }
        };
        assert!(error.display().contains("String/copy work limit"));
        assert!(runtime.json_parse(&[Value::String("1".into())]).is_err());
        runtime.begin_job();
        assert_eq!(
            runtime
                .json_parse(&[Value::String("1".into())])
                .unwrap()
                .to_text(),
            "1"
        );
        let error = runtime
            .json_stringify(&[Value::String("\n".repeat(MAX_STRING_BYTES))], &mut host)
            .unwrap_err();
        assert!(error.display().contains("String limit"));
    }

    #[test]
    fn switch_nesting_stops_on_a_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let source = format!("{};{}", "switch(1){case 1:".repeat(1000), "}".repeat(1000));
            let (mut runtime, mut host) = test_runtime();
            let error = runtime.execute(&source, &mut host).unwrap_err();
            assert!(error.contains("limit"), "{error}");
            assert_eq!(runtime.eval_depth, 0);
            assert_eq!(runtime.call_depth, 0);
            runtime.begin_job();
            assert_eq!(runtime.execute("JSON.stringify(JSON.parse('[1,true]'));", &mut host).unwrap().to_text(), "[1,true]");
        }).unwrap().join().unwrap();
    }

    #[test]
    fn json_parse_uses_heap_frames_on_a_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let (mut runtime, mut host) = test_runtime();
            let payload = format!("{}17{}", "[{\"child\":".repeat(384), "}]".repeat(384));
            runtime.define_global("payload", Value::String(payload)).unwrap();
            host.eval_permission = Some(true);
            let result = runtime.execute(
                "var cursor=eval('JSON.parse(payload)');for(var i=0;i<384;i++)cursor=cursor[0].child;cursor;",
                &mut host,
            ).unwrap();
            assert_eq!(result.to_text(), "17");
            assert_eq!(runtime.eval_depth, 0);
            assert_eq!(runtime.call_depth, 0);
        }).unwrap().join().unwrap();
    }

    #[test]
    fn json_callbacks_use_heap_frames_on_a_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let (mut runtime, mut host) = test_runtime();
            let payload = format!("{}17{}", "[{\"child\":".repeat(192), "}]".repeat(192));
            runtime.define_global("payload", Value::String(payload)).unwrap();
            let value = runtime.execute(r#"
                var revived=0,written=0;
                var value=JSON.parse(payload,function(key,value,context){
                    revived++;
                    if(value===17 && (context.source!=='17' || JSON.parse('[7]')[0]!==7)) throw 'leaf';
                    return value;
                });
                var result=JSON.stringify(value,function(key,value){written++;return value;});
                var node=value;
                for(var i=0;i<192;i++)node=node[0].child;
                result===payload && node===17 && revived===385 && written===385;
            "#, &mut host).unwrap();
            assert_eq!(value.to_text(), "true");
            assert_eq!(runtime.json_scratch_bytes, 0);
            assert_eq!(runtime.eval_depth, 0);
            assert_eq!(runtime.call_depth, 0);
        }).unwrap().join().unwrap();
    }

    #[test]
    fn json_parse_frame_budget_failure_rolls_back_objects_and_properties() {
        let (mut runtime, _) = test_runtime();
        let budget = runtime.limits.heap.retained_values;
        runtime.limits.heap.retained_values = 0;
        let checkpoint = (runtime.objects.len(), runtime.properties, runtime.value_bytes);
        let error = runtime
            .json_parse(&[Value::String("[1]".into())])
            .unwrap_err();
        assert!(error.display().contains("JSON parse scratch limit reached (0 bytes)"));
        assert_eq!((runtime.objects.len(), runtime.properties, runtime.value_bytes), checkpoint);
        runtime.limits.heap.retained_values = budget;
        assert!(runtime.json_parse(&[Value::String("[1]".into())]).is_ok());
    }

    #[test]
    fn eval_policy_denial_and_missing_capability_have_distinct_completions() {
        let (mut runtime, mut host) = test_runtime();
        assert_eq!(
            runtime.execute("eval(3);", &mut host).unwrap().to_text(),
            "3"
        );
        let error = runtime.execute("try{eval('test.record(1)');}catch(e){test.record('caught');}finally{test.record('finally');}", &mut host).unwrap_err();
        assert!(error.contains("unavailable"), "{error}");
        assert!(host.records.is_empty());
        let programs = runtime.programs.len();
        runtime.begin_job();
        host.eval_permission = Some(false);
        runtime.execute("try{eval('test.record(1)');}catch(e){test.record(e.name);}finally{test.record('finally');}", &mut host).unwrap();
        assert_eq!(host.records, ["EvalError", "finally"]);
        assert_eq!(
            runtime.programs.len(),
            programs + 1,
            "Denied text must not be compiled"
        );
        runtime.begin_job();
        host.eval_permission = Some(true);
        assert_eq!(
            runtime
                .execute("eval('2+3');", &mut host)
                .unwrap()
                .to_text(),
            "5"
        );
    }

    #[test]
    fn await_execution_resumed_callee_keeps_eval_indirect_and_csp_checked() {
        for allowed in [false, true] {
            let (mut runtime, mut host) = test_runtime();
            host.eval_permission = Some(allowed);
            runtime.execute(r#"
var token='global';
async function work(){
    let token='local';
    try {return (await Promise.resolve(eval))('token');}
    catch(error){return error.name;}
}
work().then(value=>test.record(value));
"#, &mut host).unwrap();
            assert!(host.records.is_empty());
            assert!(runtime.microtask_checkpoint(&mut host).unwrap().is_empty());
            assert_eq!(host.records, [if allowed { "global" } else { "EvalError" }]);
        }
    }

    #[test]
    fn await_execution_name_assignment_keeps_the_reference_across_direct_eval() {
        let (mut runtime, mut host) = test_runtime();
        host.eval_permission = Some(true);
        runtime.execute(r#"
var captured='outer';
var ordinary='outer';
function syncUpdate(){
    var assigned=ordinary=eval("var ordinary='local'; 'updated';");
    test.record(ordinary);
    test.record(assigned);
}
syncUpdate();test.record(ordinary);
async function update(){
    var assigned=captured=eval(await Promise.resolve("var captured='local'; 'updated';"));
    test.record(captured);
    return assigned;
}
update().then(function(value){test.record(value);test.record(captured);});
"#, &mut host).unwrap();
        assert_eq!(host.records, ["local", "updated", "updated"]);
        assert!(runtime.microtask_checkpoint(&mut host).unwrap().is_empty());
        assert_eq!(host.records, ["local", "updated", "updated", "local", "updated", "updated"]);
    }

    #[test]
    fn await_execution_direct_eval_keeps_the_resumed_scope_and_csp_check() {
        for allowed in [false, true] {
            let (mut runtime, mut host) = test_runtime();
            host.eval_permission = Some(allowed);
            runtime.execute(r#"
async function work() {
    let token = 'local';
    try { return eval(await Promise.resolve('token')); }
    catch (error) { return error.name; }
}
work().then(function(value) { test.record(value); });
"#, &mut host).unwrap();
            assert!(host.records.is_empty());
            assert!(runtime.microtask_checkpoint(&mut host).unwrap().is_empty());
            assert_eq!(host.records, [if allowed { "local" } else { "EvalError" }]);
        }
    }

    #[test]
    fn eval_parses_before_effects_and_keeps_fatal_job_limits() {
        let (mut runtime, mut host) = test_runtime();
        host.eval_permission = Some(true);
        runtime.execute("try{eval('test.record(99);let duplicate;let duplicate;');}catch(e){test.record(e.name);}try{eval('let r=/[z-a]/;');}catch(e){test.record(e.name);}", &mut host).unwrap();
        assert_eq!(host.records, ["SyntaxError", "SyntaxError"]);
        for (source, expected) in [
            ("try{eval('while(true);');}catch(e){test.record('caught');}finally{test.record('finally');}".to_owned(), "step limit"),
            ("var code='eval(code)';try{eval(code);}catch(e){test.record('caught');}finally{test.record('finally');}".to_owned(), "limit"),
            ("try{for(var n=0;n<200;n++)eval('1');}catch(e){test.record('caught');}finally{test.record('finally');}".to_owned(), "Retained script limit"),
            (format!("try{{eval('{}1;');}}catch(e){{test.record('caught');}}finally{{test.record('finally');}}", "!".repeat(500)), "Parser nesting limit"),
            ("try{eval('class Unsupported {field=1;}');}catch(e){test.record('caught');}finally{test.record('finally');}".to_owned(), "Class fields"),
            ("try{eval('({async *value(){return 1;}})');}catch(e){test.record('caught');}finally{test.record('finally');}".to_owned(), "Async generator object methods"),
        ] {
            let (mut runtime, mut host) = test_runtime();
            host.eval_permission = Some(true);
            // Original cumulative-program fatal guard remains an exact control.
            runtime.program_allocation.disabled = expected == "Retained script limit";
            let error = runtime.execute(&source, &mut host).unwrap_err();
            assert!(error.contains(expected), "{source}: {error}");
            assert!(host.records.is_empty(), "{source}");
            assert_eq!(runtime.eval_depth, 0);
            assert_eq!(runtime.call_depth, 0);
            assert!(!runtime.strict);
            assert!(runtime.retained_source <= MAX_RETAINED_SOURCE);
            assert!(runtime.retained_ast <= MAX_RETAINED_AST);
            assert!(runtime.programs.len() <= MAX_PROGRAMS);
        }
    }

    #[test]
    fn eval_closures_and_strict_context_survive_jobs_without_leaking_lexicals() {
        let (mut runtime, mut host) = test_runtime();
        host.eval_permission = Some(true);
        let closure = runtime.execute("eval(\"'use strict';let n=4;(function(){eval('var privateValue=9');return [++n,typeof privateValue].join(':');})\")", &mut host).unwrap();
        for expected in ["5:undefined", "6:undefined"] {
            runtime.begin_job();
            assert_eq!(
                runtime
                    .invoke(closure.clone(), &[], Value::Undefined, &mut host)
                    .unwrap()
                    .to_text(),
                expected
            );
            assert!(!runtime.strict);
        }
        runtime.begin_job();
        assert_eq!(
            runtime.execute("typeof n;", &mut host).unwrap().to_text(),
            "undefined"
        );
    }

    #[test]
    fn host_function_allocation_does_not_create_an_ambient_global() {
        let (mut runtime, mut host) = test_runtime();
        let callback = runtime.host_function(1, "record").unwrap();
        host.properties.insert((1, "native".into()), callback);
        runtime
            .execute("let f=test.native;f('called');typeof record;", &mut host)
            .unwrap();
        assert_eq!(host.records, ["called"]);
        assert!(runtime.lookup(0, "record").unwrap().is_none());
        while runtime.functions.len() < MAX_FUNCTIONS {
            runtime.host_function(1, "record").unwrap();
        }
        assert!(runtime
            .host_function(1, "record")
            .unwrap_err()
            .contains("Function limit"));
        assert_eq!(runtime.functions.len(), MAX_FUNCTIONS);
    }

    #[test]
    fn new_loop_and_dynamic_eval_nesting_stop_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                // Preserve the formerly rejected input as an exact success case.
                let grouped = format!("var n=0;var code='if(n++<16)eval(code);else eval(deep);';var deep='{}1{}';try{{eval(code);}}catch(e){{test.record('caught');}}finally{{test.record('finally');}}", "(".repeat(1000), ")".repeat(1000));
                let (mut accepted, mut recorder) = test_runtime();
                recorder.eval_permission = Some(true);
                assert_eq!(accepted.execute(&grouped, &mut recorder).unwrap().to_text(), "1");
                assert_eq!(recorder.records, ["finally"]);
                for source in [
                    format!("{}Array;", "new ".repeat(1000)),
                    format!("{};{}", "do{".repeat(100), "}while(false);".repeat(100)),
                    format!("{};", "for(var x of [1])".repeat(100)),
                    "var code='eval(code)';eval(code);".into(),
                    format!("var n=0;var code='if(n++<16)eval(code);else eval(deep);';var deep='{}1{}';try{{eval(code);}}catch(e){{test.record('caught');}}finally{{test.record('finally');}}", "(".repeat(MAX_EXPRESSION_FRAMES + 1), ")".repeat(MAX_EXPRESSION_FRAMES + 1)),
                ] {
                    let (mut runtime, mut host) = test_runtime();
                    host.eval_permission = Some(true);
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(error.contains("limit"), "{error}");
                    assert!(host.records.is_empty());
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn dense_script_caps_preserve_atomic_rejection_on_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let tokens = ";".repeat(MAX_TOKENS);
                let program = Parser::parse(&tokens, &mut MAX_REGEXP_WORK.clone()).unwrap();
                assert_eq!(program.stmts.len(), MAX_TOKENS);
                drop(program);
                let nodes = "0\n".repeat(MAX_AST / 2);
                let program = Parser::parse(&nodes, &mut MAX_REGEXP_WORK.clone()).unwrap();
                assert_eq!(program.exprs.len() + program.stmts.len(), MAX_AST);
                drop(program);
                for (source, message) in [
                    (
                        format!("test.record(1);{tokens}"),
                        "Script token limit reached (131072)",
                    ),
                    (
                        format!("test.record(1);{nodes}"),
                        "AST node limit reached (131072)",
                    ),
                ] {
                    let (mut runtime, mut host) = test_runtime();
                    host.eval_permission = Some(true);
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(error.contains(message), "{error}");
                    assert_eq!(runtime.programs.len(), 0);
                    assert!(host.records.is_empty());
                    // Eval cannot manufacture a source above MAX_STRING_BYTES;
                    // that independent string cap remains smaller than this bundle.
                    runtime.begin_job();
                    assert!(runtime
                        .define_global("payload", Value::String(source))
                        .is_err());
                    assert!(host.records.is_empty());
                    assert_eq!(runtime.call_depth, 0);
                    assert_eq!(runtime.eval_depth, 0);
                }
                let mut runtime = Runtime::for_test();
                runtime.limits.heap.retained_values=64*1024*1024;
                runtime.program_allocation.disabled = true;
                runtime.register_program(&nodes, false).unwrap();
                runtime.begin_job();
                runtime.register_program(&nodes, false).unwrap();
                assert_eq!(runtime.retained_ast, MAX_RETAINED_AST);
                let source_before = runtime.retained_source;
                let error = runtime.register_program("0;", false).err().unwrap();
                assert!(error.display().contains("Retained AST limit"));
                assert_eq!(runtime.retained_ast, MAX_RETAINED_AST);
                assert_eq!(runtime.retained_source, source_before);
                assert_eq!(runtime.programs.len(), 2);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn property_keys_run_callbacks_in_read_write_and_builtin_order() {
        assert_eq!(result("let log='',key={toString(){log+='K';return 'x';}},o={x:1};o[key]=(log+='R',2);o[key]+=(log+='A',3);o[key]++;log+':'+o.x;"), "RKKAKKK:6");
        assert_eq!(result("let log='',key={toString(){log+='K';return 'x';}};try{null[key]=(log+='R',1);}catch(e){log+=e.name;}try{Object.prototype.hasOwnProperty.call(null,key);}catch(e){log+=e.name;}log;"), "RTypeErrorKTypeError");
        assert_eq!(result("let descriptor={value:1},key={toString(){descriptor.value=4;return 'x';}},o={};Object.defineProperty(o,key,descriptor);o.x;"), "4");
        assert_eq!(result("function Token(text){this.text=text;}Token.prototype.toString=function(){return this.text;};let key=new Token('iterator_26');Object.defineProperty(Array.prototype,key,{configurable:true,value:function(){return this.length;}});[1,2][key]();"), "2");
    }

    #[test]
    fn property_key_fallback_uses_current_methods_and_preserves_exceptions() {
        assert_eq!(result("let key={toString(a){if(this!==key||a!==undefined)throw 'receiver';this.valueOf=function(){return 'fresh';};return this;}};({fresh:7})[key];"), "7");
        assert_eq!(result("let marker={},key={toString(){throw marker;}};try{({})[key];}catch(e){e===marker;}"), "true");
        assert_eq!(result("let key={toString(){return this;},valueOf(){return function(){};}};try{({})[key];}catch(e){e.name;}"), "TypeError");
    }

    #[test]
    fn property_key_conversion_cannot_inspect_native_hosts_or_replace_native_globals() {
        #[derive(Default)]
        struct GuardHost {
            gets: usize,
            writes: usize,
            calls: usize,
        }
        impl Host for GuardHost {
            fn owns_global_property(&self, name: &str) -> bool {
                name == "location"
            }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                self.gets += 1;
                Ok(Value::Undefined)
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                self.writes += 1;
                Ok(())
            }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
                self.calls += 1;
                Ok(Value::Undefined)
            }
        }
        let mut runtime = Runtime::for_test();
        let mut host = GuardHost::default();
        runtime.define_global("native", Value::Host(1)).unwrap();
        runtime.define_global("window", Value::Host(2)).unwrap();
        runtime.bind_browser_global(2).unwrap();
        assert!(runtime
            .execute("({})[native];", &mut host)
            .unwrap_err()
            .contains("Native host object property-key"));
        assert_eq!((host.gets, host.writes, host.calls), (0, 0, 0));
        runtime.begin_job();
        let error = runtime.execute("let key={toString(){return 'location';}};Object.defineProperty(window,key,{value:3});", &mut host).unwrap_err();
        assert!(error.contains("native or declared global"), "{error}");
        assert_eq!((host.gets, host.writes, host.calls), (0, 0, 0));
        runtime.begin_job();
        assert_eq!(runtime.execute("let fallback={toString(){return native;},valueOf(){return 'ok';}};({ok:4})[fallback];", &mut host).unwrap().to_text(), "4");
        assert_eq!((host.gets, host.writes, host.calls), (0, 0, 0));
    }

    #[test]
    fn property_key_callback_recursion_stops_on_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            for source in [
                "let o={},key={toString(){return o[key];}};try{o[key];}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let o={},key={toString(){return Object.getOwnPropertyDescriptor(o,key);}};try{o[key];}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let o={},key={toString(){return o.hasOwnProperty(key);}};try{o[key];}catch(e){test.record('caught');}finally{test.record('finally');}",
                "let key={toString(){while(true){}}};try{({})[key];}catch(e){test.record('caught');}finally{test.record('finally');}",
            ] {
                let (mut runtime, mut host) = test_runtime();
                let error = runtime.execute(source, &mut host).unwrap_err();
                assert!(error.contains("limit"), "{error}");
                assert!(host.records.is_empty());
                assert_eq!(runtime.call_depth, 0);
                assert_eq!(runtime.eval_depth, 0);
                runtime.begin_job();
                assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn in_probes_presence_without_getters_and_preserves_native_ownership() {
        #[derive(Default)]
        struct PresenceHost {
            checks: Vec<(u32, String)>,
            gets: usize,
        }
        impl Host for PresenceHost {
            fn owns_global_property(&self, name: &str) -> bool {
                name == "reserved"
            }
            fn has(&mut self, object: u32, name: &str) -> Result<bool, String> {
                self.checks.push((object, name.into()));
                Ok(object == 1 && name == "present")
            }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                self.gets += 1;
                Ok(Value::Undefined)
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                Err("No host writes expected.".into())
            }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
                Err("No host calls expected.".into())
            }
        }
        let mut runtime = Runtime::for_test();
        let mut host = PresenceHost::default();
        runtime.define_global("window", Value::Host(2)).unwrap();
        runtime.define_global("opaque", Value::Host(1)).unwrap();
        runtime.define_global("reserved", Value::Undefined).unwrap();
        runtime.bind_browser_global(2).unwrap();
        let value = runtime.execute("var own=undefined;let lexical=7;window.extra=undefined;['own' in window,'extra' in window,'lexical' in window,'reserved' in window,'present' in opaque,'missing' in opaque].join(',');", &mut host).unwrap();
        assert_eq!(value.to_text(), "true,true,false,false,true,false");
        assert_eq!(host.gets, 0);
        assert_eq!(
            host.checks,
            [
                (2, "lexical".into()),
                (2, "reserved".into()),
                (1, "present".into()),
                (1, "missing".into())
            ]
        );
        runtime.begin_job();
        let error = runtime
            .execute(
                &format!("'{}' in opaque;", "k".repeat(MAX_PROPERTY_BYTES + 1)),
                &mut host,
            )
            .unwrap_err();
        assert!(error.contains("Property-name limit"), "{error}");
        assert_eq!(host.checks.len(), 4);
        assert_eq!(
            result("let name;try{({}) in 1;}catch(e){name=e.name;}name;"),
            "TypeError"
        );
        assert_eq!(result("let a=[,undefined];(1 in a)+':'+(0 in a)+':'+('length' in a)+':'+('missing' in a);"), "true:false:true:false");
    }

    #[test]
    fn for_in_snapshots_keys_and_retains_iteration_bindings_across_jobs() {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute("let calls=[];for(const key in {b:1,2:2,a:3,1:4}){calls.push(()=>key);}let a=[0,1,2],seen='';for(let key in a){seen+=key;a.length=1;a.extra=9;}test.record(seen);", &mut host).unwrap();
        assert_eq!(host.records, ["0"]);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("calls[0]()+calls[1]()+calls[2]()+calls[3]();", &mut host)
                .unwrap()
                .to_text(),
            "12ba"
        );
        assert_eq!(
            result("let x={a:1},name;try{for(let x in x){}}catch(e){name=e.name;}name;"),
            "ReferenceError"
        );
        assert_eq!(result("const key='unchanged';let count=0,o={};for(key in {}){}for(o[count++] in null){}key+':'+count;"), "unchanged:0");
    }

    #[test]
    fn labelled_control_preserves_completions_and_finally_targets() {
        assert_eq!(result("let text='';outer:for(let i=0;i<3;i++){for(let j=0;j<3;j++){try{text+=i+':'+j+';';continue outer;}finally{text+='f';}}}text;"), "0:0;f1:0;f2:0;f");
        assert_eq!(
            result("let n=0;outer:while(n<3){n++;try{break outer;}finally{continue outer;}}n;"),
            "3"
        );
        assert_eq!(result("7;label:{9;break label;}"), "9");
        assert_eq!(result("7;label:{9;if(true)break label;}"), "undefined");
        assert_eq!(
            result("function f(){return\nouter:{} /a/.test('a');}f();"),
            "undefined"
        );
        for source in [
            "test.record('bad');exit:{break missing;}",
            "test.record('bad');exit:{exit:{}}",
            "test.record('bad');exit:{while(true)continue exit;}",
            "test.record('bad');exit:{function f(){break exit;}}",
            "test.record('bad');exit:for(;;){let f=()=>{continue exit;};}",
            "test.record('bad');for(let key in {}){var key;}",
        ] {
            let (mut runtime, mut host) = test_runtime();
            assert!(runtime.execute(source, &mut host).is_err(), "{source}");
            assert!(host.records.is_empty(), "{source}");
        }
    }

    #[test]
    fn enumeration_and_presence_limits_remain_fatal() {
        for source in [
            "try{'x' in test;}catch(e){test.record('caught');}finally{test.record('finally');}".into(),
            "try{for(let key in test){}}catch(e){test.record('caught');}finally{test.record('finally');}".into(),
            format!("try{{for(let key in '{}'){{}}}}catch(e){{test.record('caught');}}finally{{test.record('finally');}}", "x".repeat(MAX_OBJECT_PROPERTIES + 1)),
            "try{outer:while(true){continue outer;}}catch(e){test.record('caught');}finally{test.record('finally');}".into(),
        ] {
            let (mut runtime, mut host) = test_runtime();
            assert!(runtime.execute(&source, &mut host).is_err(), "{source}");
            assert!(host.records.is_empty());
        }
    }

    #[test]
    fn registered_host_functions_are_real_callables_with_fatal_host_errors() {
        let (mut runtime, mut host) = test_runtime();
        let function = runtime
            .define_host_function("nativeRecord", 1, "record")
            .unwrap();
        assert!(matches!(function, Value::Function(_)));
        runtime.execute("let alias=nativeRecord;test.record(typeof alias);alias(7);alias.call(null,8);alias.apply(null,[9]);", &mut host).unwrap();
        assert_eq!(host.records, ["function", "7", "8", "9"]);
        runtime
            .define_host_function("nativeFailure", 1, "unknown")
            .unwrap();
        let error = runtime.execute("try{nativeFailure();}catch(e){test.record('caught');}finally{test.record('finally');}", &mut host).unwrap_err();
        assert!(error.contains("Unsupported test method unknown"), "{error}");
        assert_eq!(host.records, ["function", "7", "8", "9"]);
    }

    #[test]
    fn legacy_opening_brace_literals_and_regexp_constructor_preserve_state_and_errors() {
        assert_eq!(result(r#"
var regex=/{/g,first=regex.exec('x{y{'),position=regex.lastIndex,second=regex.exec('x{y{');
if(first.index!==1||second.index!==3||position!==2||regex.lastIndex!==4||regex.source!=='{')throw 'global state';
if('x{y{'.replace(/{/g,'!')!=='x!y!'||!new RegExp('a{1001').test('a{1001')||!/^{{2}$/.test('{{'))throw 'literal braces';
var errors=0;for(var pattern of ['{1}','{1,}','{1,2}','a{2,1}','a{1}*','a{1}{2}']){
try{new RegExp(pattern);}catch(e){if(e.name==='SyntaxError')errors++;}}
if(errors!==6)throw 'quantifier syntax';'brace-ok';
"#), "brace-ok");
    }

    #[test]
    fn regexp_compilation_matching_and_calls_share_one_job_budget() {
        let (mut runtime, mut host) = test_runtime();
        runtime.regexp_work = 2000;
        let error = runtime
            .execute(
                "test.record('must not run'); /(?:(?:(?:){1000}){1000}){1000}/;",
                &mut host,
            )
            .unwrap_err();
        assert!(error.contains("Regular-expression work limit"), "{error}");
        assert!(host.records.is_empty());
        assert_eq!(runtime.regexp_work, 0);
        runtime.begin_job();
        runtime.regexp_work = 2000;
        let error = runtime.execute("try{RegExp('(?:(?:(?:){1000}){1000}){1000}');}catch(e){test.record('caught');}finally{test.record('finally');}", &mut host).unwrap_err();
        assert!(error.contains("Regular-expression work limit"), "{error}");
        assert!(host.records.is_empty());
        runtime.begin_job();
        runtime.execute("var r=/a+/;", &mut host).unwrap();
        runtime.regexp_work = 2000;
        let error = runtime
            .execute("while(true)r.test('aaaaaaaaaaaaaaaa');", &mut host)
            .unwrap_err();
        assert!(error.contains("Regular-expression work limit"), "{error}");
        assert_eq!(runtime.regexp_work, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("r.test('aaa');", &mut host)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn regexp_retained_code_and_backtracking_scratch_are_bounded() {
        let (mut runtime, _) = test_runtime();
        let pattern = Value::String("a".repeat(1000));
        loop {
            runtime.begin_job();
            let before = runtime.retained_regexp;
            let objects = runtime.objects.len();
            match runtime.regexp_factory(std::slice::from_ref(&pattern)) {
                Ok(_) => assert!(runtime.retained_regexp > before),
                Err(error) => {
                    assert!(error.display().contains("Retained regexp limit"));
                    assert_eq!(runtime.retained_regexp, before);
                    assert_eq!(runtime.objects.len(), objects);
                    break;
                }
            }
        }
        let (mut runtime, mut host) = test_runtime();
        let source = format!("/(a)*/.test('{}');", "a".repeat(2000));
        let error = runtime.execute(&source, &mut host).unwrap_err();
        assert!(error.contains("backtracking limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("/a+/.test('aaa');", &mut host)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn exceptions_preserve_identity_and_finally_completion() {
        assert_eq!(result("let original={n:1};let same=false;try{throw original;}catch(e){same=e===original;e.n++;}finally{original.n++;}same+':'+original.n;"), "true:3");
        assert_eq!(
            result("function f(){try{return 1;}finally{return 2;}}f();"),
            "2"
        );
        assert_eq!(
            result("function f(){try{throw 1;}finally{return 7;}}f();"),
            "7"
        );
        assert_eq!(result("let n=0;for(let i=0;i<4;i++){try{if(i===0)continue;if(i===2)break;n++;}finally{n+=10;}}n;"), "31");
        assert_eq!(result("1;try{}finally{2;}"), "undefined");
        assert_eq!(result("1;try{3;}finally{2;}"), "3");
        assert_eq!(result("1;try{throw 3;}catch(e){}"), "undefined");
        assert_eq!(result("let names=[];try{missing;}catch(e){names.push(e.name);}try{null.field;}catch(e){names.push(e.name);}try{Array(-1);}catch(e){names.push(e.name);}names.join();"), "ReferenceError,TypeError,RangeError");
    }

    #[test]
    fn resource_and_capability_failures_cannot_be_caught_or_suppressed() {
        for source in [
            "try{while(true){}}catch(e){test.record('caught');}finally{test.record('finally');}",
            "function f(){try{while(true){}}finally{return 1;}}test.record(f());",
            "function f(){try{return f();}finally{test.record('finally');}}try{f();}catch(e){test.record('caught');}",
            "try{test.unavailable();}catch(e){test.record('caught');}finally{test.record('finally');}",
            "try{Object.setPrototypeOf(test,{});}catch(e){test.record('caught');}finally{test.record('finally');}",
            "function f(){try{Array(8193);}finally{return 1;}}test.record(f());",
        ] {
            eprintln!("fatal capability witness: {source}");
            let (mut runtime, mut host) = test_runtime();
            assert!(runtime.execute(source, &mut host).is_err(), "{source}");
            assert!(host.records.is_empty(), "A fatal interpreter failure was suppressed: {source}");
        }
    }

    #[test]
    fn arrows_capture_this_and_catch_bindings_across_jobs() {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute("let receiver={n:4,make:function(){try{throw 3;}catch(e){return ()=>this.n+e;}}};test.listen(receiver.make());", &mut host).unwrap();
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(host.callbacks[0].clone(), &[], Value::Null, &mut host)
                .unwrap()
                .to_text(),
            "7"
        );
        runtime.execute("receiver.n=9;", &mut host).unwrap();
        assert_eq!(
            runtime
                .invoke(host.callbacks[0].clone(), &[], Value::Host(1), &mut host)
                .unwrap()
                .to_text(),
            "12"
        );
        assert_eq!(
            result("let sum=(a,b)=>a+b;let object=n=>({n:n});sum(object(2).n,3);"),
            "5"
        );
    }

    #[test]
    fn browser_globals_share_identity_but_keep_lexical_declarations_separate() {
        let (mut runtime, mut host) = test_runtime();
        runtime.bind_browser_global(2).unwrap();
        runtime
            .execute(
                "window.state={n:3};window.saved=17;window.replaceMe='old';",
                &mut host,
            )
            .unwrap();
        runtime.begin_job();
        let result = runtime.execute("var saved;let state=8;function replaceMe(){return 9;}saved++;window.state.n++;state+':'+window.state.n+':'+window.saved+':'+window.replaceMe();", &mut host).unwrap();
        assert_eq!(result.to_text(), "8:4:18:9");
        let (_, properties) = runtime.browser_global.unwrap();
        assert!(!runtime.objects[properties].properties.contains_key("saved"));
        assert!(!runtime.objects[properties]
            .properties
            .contains_key("replaceMe"));
        assert!(runtime.objects[properties].properties.contains_key("state"));
        assert!(host.properties.is_empty());
        assert_eq!(
            runtime
                .execute(
                    "self===globalThis && globalThis===window && typeof window.saved==='number';",
                    &mut host
                )
                .unwrap()
                .to_text(),
            "true"
        );
        let error = runtime
            .execute("test.record('must not run');var state;", &mut host)
            .unwrap_err();
        assert!(error.contains("conflicts"));
        assert!(host.records.is_empty());
    }

    #[test]
    fn global_accessor_declarations_fail_before_getters_or_scope_mutation() {
        for descriptor in [
            "{get:function(){reads++;return 7;},set:function(value){writes++;},enumerable:true,configurable:true}",
            "{get:function(){reads++;return 7;},configurable:false}",
            "{set:function(value){writes++;},configurable:false}",
            "{get:undefined,set:undefined,configurable:true}",
        ] {
            for source in [
                "var fresh;test.record('ran');var blocked;",
                "var fresh;function freshFunction(){}function blocked(){}test.record('ran');",
                "eval(\"var fresh;var blocked;test.record('ran');\");",
                "eval(\"var fresh;function freshFunction(){}function blocked(){}test.record('ran');\");",
                "eval.call(null,\"var fresh;var blocked;test.record('ran');\");",
            ] {
                let (mut runtime, mut host) = test_runtime();
                runtime.bind_browser_global(2).unwrap();
                host.eval_permission = Some(true);
                runtime.execute(&format!(
                    "var reads=0,writes=0;Object.defineProperty(window,'blocked',{descriptor});"
                ), &mut host).unwrap();
                let (_, properties) = runtime.browser_global.unwrap();
                let property = &runtime.objects[properties].properties["blocked"];
                let before = (
                    property.accessor.unwrap().get,
                    property.accessor.unwrap().set,
                    property.order,
                    property.enumerable,
                    property.writable,
                    property.configurable,
                );
                let counts = (runtime.bindings, runtime.functions.len(), runtime.properties, runtime.value_bytes - runtime.retained_arena_bytes());
                let error = runtime.execute(source, &mut host).unwrap_err();
                assert!(error.contains("Global var/function declarations over accessor property"), "{source}: {error}");
                assert!(host.records.is_empty(), "{source}");
                for name in ["fresh", "freshFunction", "blocked"] {
                    assert!(!runtime.environment(0).bindings.contains_key(name), "{source}: {name}");
                }
                assert_eq!(runtime.binding_value(0, "reads").unwrap().to_text(), "0", "{source}");
                assert_eq!(runtime.binding_value(0, "writes").unwrap().to_text(), "0", "{source}");
                let property = &runtime.objects[properties].properties["blocked"];
                assert_eq!(before, (
                    property.accessor.unwrap().get,
                    property.accessor.unwrap().set,
                    property.order,
                    property.enumerable,
                    property.writable,
                    property.configurable,
                ), "{source}");
                assert_eq!(counts, (runtime.bindings, runtime.functions.len(), runtime.properties, runtime.value_bytes - runtime.retained_arena_bytes()), "{source}");
            }
        }
    }

    #[test]
    fn global_accessor_preflight_allows_local_and_strict_eval_shadows_and_data_migration() {
        let (mut runtime, mut host) = test_runtime();
        runtime.bind_browser_global(2).unwrap();
        host.eval_permission = Some(true);
        runtime.execute(
            "var reads=0;Object.defineProperty(window,'blocked',{get:function(){reads++;return 11;}});window.data=7;window.replaceMe=8;",
            &mut host,
        ).unwrap();
        let result = runtime.execute(r#"
            let local=(function(){var blocked=3;return blocked;})();
            let localFunction=(function(){function blocked(){return 4;}return blocked();})();
            let strictVar=eval("'use strict';var blocked=5;blocked;");
            let strictFunction=eval("'use strict';function blocked(){return 6;}blocked();");
            let inheritedStrict=(function(){'use strict';return eval('var blocked=9;blocked;');})();
            var data;function replaceMe(){return 10;}
            [local,localFunction,strictVar,strictFunction,inheritedStrict,data,replaceMe(),reads].join(':');
        "#, &mut host).unwrap();
        assert_eq!(result.to_text(), "3:4:5:6:9:7:10:0");
        let (_, properties) = runtime.browser_global.unwrap();
        assert!(runtime.objects[properties].properties["blocked"]
            .accessor
            .is_some());
        assert!(!runtime.environment(0).bindings.contains_key("blocked"));
        assert!(!runtime.objects[properties].properties.contains_key("data"));
        assert!(!runtime.objects[properties]
            .properties
            .contains_key("replaceMe"));
        assert_eq!(
            runtime
                .execute("window.blocked+':'+reads;", &mut host)
                .unwrap()
                .to_text(),
            "11:1"
        );
    }

    #[test]
    fn native_window_fields_keep_their_host_ownership() {
        let (mut runtime, mut host) = test_runtime();
        runtime.bind_browser_global(2).unwrap();
        runtime
            .execute("window.onload=()=>7;test.listen(onload);", &mut host)
            .unwrap();
        assert_eq!(
            runtime
                .invoke(host.callbacks[0].clone(), &[], Value::Undefined, &mut host)
                .unwrap()
                .to_text(),
            "7"
        );
        runtime
            .execute("var onload=()=>9;test.listen(window.onload);", &mut host)
            .unwrap();
        assert_eq!(
            runtime
                .invoke(host.callbacks[1].clone(), &[], Value::Undefined, &mut host)
                .unwrap()
                .to_text(),
            "9"
        );
        for source in [
            "window.location='https://example.com/';",
            "location='https://example.com/';",
            "var location='https://example.com/';",
            "window.window={};",
            "try{window.location;}catch(e){test.record('caught');}finally{test.record('finally');}",
        ] {
            runtime.begin_job();
            assert!(runtime.execute(source, &mut host).is_err(), "{source}");
        }
        assert!(host.records.is_empty());
        let (_, properties) = runtime.browser_global.unwrap();
        assert!(!runtime.objects[properties]
            .properties
            .contains_key("location"));
        assert!(!runtime.objects[properties]
            .properties
            .contains_key("onload"));
    }

    #[test]
    fn uri_decoding_is_strict_and_sequence_expressions_preserve_order() {
        assert_eq!(
            result("decodeURIComponent(encodeURIComponent('a b/é😀'))"),
            "a b/é😀"
        );
        assert_eq!(result("decodeURI('%2f%3F%20%25')"), "%2f%3F %");
        assert_eq!(result("let invalid=['%','%GG','%C0%AF','%ED%A0%80','%F4%90%80%80'];let n=0;for(let i=0;i<invalid.length;i++){try{decodeURIComponent(invalid[i]);}catch(e){if(e.name==='URIError')n++;}}n;"), "5");
        assert_eq!(
            result("let a=0,b=0;function f(){return a=1,b=2,a+b;}f()+':'+a+':'+b;"),
            "3:1:2"
        );
    }

    #[test]
    fn expressions_coercion_and_short_circuit() {
        assert_eq!(result("let n=0; false && n++; true || n++; (2+3*4===14 && n===0) ? 'ok'+(true+2) : 'bad';"), "ok3");
        assert_eq!(
            result("null == undefined && null !== undefined && '7' == 7 && '7' !== 7 && !NaN;"),
            "true"
        );
        assert_eq!(
            result("(-1 >>> 0) + ':' + (5 << 2) + ':' + (-9 >> 1);"),
            "4294967295:20:-5"
        );
        assert_eq!(result("1/0 + ':' + (0/0) + ':' + (-0);"), "Infinity:NaN:0");
        assert_eq!(
            result("typeof missing + ':' + typeof null + ':' + typeof function(){};"),
            "undefined:object:function"
        );
    }

    #[test]
    fn declarations_hoist_and_scope_without_implicit_globals() {
        assert_eq!(result("var before=typeof x; if(false){var x=1;} let outside=7; {let outside=4;} before+':'+outside;"), "undefined:7");
        assert_eq!(result("answer(); function answer(){return 42;}"), "42");
        let (mut runtime, mut host) = test_runtime();
        assert!(runtime
            .execute("missing = 1;", &mut host)
            .unwrap_err()
            .contains("implicit globals"));
        assert!(runtime
            .execute("let hidden=hidden;", &mut host)
            .unwrap_err()
            .contains("before its initialization"));
        assert!(runtime
            .execute("const fixed=1; fixed++;", &mut host)
            .unwrap_err()
            .contains("const"));
    }

    #[test]
    fn global_state_and_closures_persist_between_jobs() {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute("let total=2; function make(step){return function(){total+=step;return total;};} test.listen(make(3));", &mut host).unwrap();
        let callback = host.callbacks[0].clone();
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(callback.clone(), &[], Value::Undefined, &mut host)
                .unwrap()
                .to_text(),
            "5"
        );
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(callback, &[], Value::Undefined, &mut host)
                .unwrap()
                .to_text(),
            "8"
        );
        assert_eq!(runtime.execute("total;", &mut host).unwrap().to_text(), "8");
        assert!(runtime
            .execute("var total;", &mut host)
            .unwrap_err()
            .contains("conflicts"));
    }

    #[test]
    fn named_function_expression_has_private_shadowable_scope() {
        assert_eq!(result("let f=function privateName(n){if(n<=1)return 1;return n*privateName(n-1);}; f(5)+':'+typeof privateName;"), "120:undefined");
        assert_eq!(
            result("let f=function named(){var named=4;return named;}; f();"),
            "4"
        );
        assert_eq!(
            result("let f=function named(){let named=5;return named;}; f();"),
            "5"
        );
        assert_eq!(
            result("let f=function named(named){return named;}; f(6);"),
            "6"
        );
    }

    #[test]
    fn loops_break_continue_and_per_iteration_closures() {
        let (mut runtime, mut host) = test_runtime();
        assert_eq!(runtime.execute("var total=0; for(let i=0;i<5;i++){if(i===1)continue;if(i===4)break;total+=i;test.listen(function(){return i;});} total;", &mut host).unwrap().to_text(), "5");
        let mut values = Vec::new();
        for function in host.callbacks.clone() {
            values.push(
                runtime
                    .invoke(function, &[], Value::Undefined, &mut host)
                    .unwrap()
                    .to_text(),
            );
        }
        assert_eq!(values, ["0", "2", "3"]);
        assert_eq!(
            runtime
                .execute("var n=0;while(n<3){n++;} n;", &mut host)
                .unwrap()
                .to_text(),
            "3"
        );
    }

    #[test]
    fn automatic_semicolons_return_newline_and_comments() {
        assert_eq!(
            result("let n=2\nlet f=function(){return\n n+8;}\nf() === undefined && n === 2"),
            "true"
        );
        assert_eq!(result("let n=1; // line\n n\n ++n; /* block\n */ n;"), "2");
        assert_eq!(result("'a\\\nb' + '\\uD83D\\uDE00';"), "ab😀");
    }

    #[test]
    fn primitive_builtins_and_utf16_string_positions() {
        assert_eq!(result("String(Number(' 0x20 ')) + ':' + Boolean('') + ':' + parseInt(' -ffrest',16) + ':' + parseFloat('1.5e+rest');"), "32:false:-255:1.5");
        assert_eq!(
            result("'😀a😀'.length+':'+ '😀a😀'.indexOf('a') + ':' + '😀a😀'.indexOf('😀',1);"),
            "5:2:3"
        );
        assert_eq!(result("'abc'.endsWith('c',undefined) && 'abc'.endsWith('b',2) && 'abc'.startsWith('b',1) && 'abc'.includes('',99);"), "true");
        assert_eq!(
            result("'  Hello  '.trim().toUpperCase().toLowerCase();"),
            "hello"
        );
        assert_eq!(
            result("isNaN(Number('12tail')) && Number('')===0 && parseInt('0b11')===0;"),
            "true"
        );
    }

    #[test]
    fn host_properties_arguments_and_explicit_callback_this() {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute("test.value=4; test.value+=3; test.record(test.value); test.listen(function(event){this.value+=event;return this.value;}); this.ready=true;", &mut host).unwrap();
        assert_eq!(host.records, ["7"]);
        assert!(host.properties[&(2, "ready".into())].truthy());
        let callback = host.callbacks[0].clone();
        assert_eq!(
            runtime
                .invoke(callback, &[Value::Number(5.0)], Value::Host(1), &mut host)
                .unwrap()
                .to_text(),
            "12"
        );
        assert!(runtime
            .execute("null.method(test.record('must not run'));", &mut host)
            .is_err());
        assert_eq!(host.records, ["7"]);
    }

    #[test]
    fn object_identity_methods_and_call_receiver_survive_jobs() {
        let (mut runtime, mut host) = test_runtime();
        runtime.execute("let state={total:2,values:[2],add(n){this.total+=n;this.values.push(this.total);return this.total;}}; let alias=state;state.self=state;test.listen(function(n){return state.add.call(alias,n);});", &mut host).unwrap();
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(
                    host.callbacks[0].clone(),
                    &[Value::Number(5.0)],
                    Value::Undefined,
                    &mut host
                )
                .unwrap()
                .to_text(),
            "7"
        );
        assert_eq!(
            runtime
                .execute(
                    "state.self===alias && state.values.join(',')==='2,7';",
                    &mut host
                )
                .unwrap()
                .to_text(),
            "true"
        );
        assert_eq!(result("let o={f(x){return 'first:'+x;}};function replace(){o.f=function(){return 'second';};return 9;}o.f(replace());"), "first:9");
        assert_eq!(
            result("(function(){return this;}).call(this)===window;"),
            "true"
        );
    }

    #[test]
    fn sparse_arrays_keep_identity_and_distinguish_holes() {
        assert_eq!(result("let a=[1,,undefined,3];a.length=6;Object.keys(a).join('|')+':'+a.indexOf(undefined)+':'+a.includes(undefined)+':'+a.join('-');"), "0|2|3:2:true:1---3--");
        assert_eq!(result("let o={n:4};let a=[o,,7];let b=a.slice(0,2);b[0].n=8; b.length+':'+b.hasOwnProperty(1)+':'+a[0].n;"), "2:false:8");
        assert_eq!(
            result("function sum(a,b){return this.n+a+b;}sum.apply({n:1},{0:2,1:3,length:2});"),
            "6"
        );
        assert_eq!(
            result("let a=[1];a.push(a);a.push([2,a]);a.join('|');"),
            "1||2,"
        );
    }

    #[test]
    fn object_property_accounting_releases_truncated_array_values() {
        let (mut runtime, _) = test_runtime();
        let baseline = runtime.value_bytes;
        let initial_properties = runtime.properties;
        let id = runtime.new_object(Some(0)).unwrap();
        runtime
            .set_property(id, "0".into(), Value::String("x".repeat(MAX_STRING_BYTES)))
            .unwrap();
        runtime
            .set_property(id, "3".into(), Value::Object(id))
            .unwrap();
        runtime
            .set_property(id, "note".into(), Value::String("keep".into()))
            .unwrap();
        assert_eq!(runtime.properties, initial_properties + 3);
        assert_eq!(runtime.value_bytes, baseline + MAX_STRING_BYTES + 2 + 8);
        runtime.set_array_length(id, 0).unwrap();
        assert_eq!(runtime.value_bytes, baseline + 8);
        assert_eq!(runtime.properties, initial_properties + 1);
        assert_eq!(runtime.get_property(id, "note").unwrap().to_text(), "keep");
        assert_eq!(runtime.get_property(id, "length").unwrap().to_text(), "0");
        let before = runtime.value_bytes;
        assert!(runtime
            .set_property(id, MAX_ARRAY_LENGTH.to_string(), Value::String("no".into()))
            .is_err());
        assert!(runtime
            .set_property(
                id,
                "long".into(),
                Value::String("x".repeat(MAX_STRING_BYTES + 1))
            )
            .is_err());
        assert_eq!(runtime.value_bytes, before);
        assert_eq!(runtime.properties, initial_properties + 1);
    }

    #[test]
    fn object_arenas_and_retained_properties_have_hard_document_caps() {
        let (mut runtime, _) = test_runtime();
        for _ in runtime.objects.len()..MAX_OBJECTS {
            runtime.new_object(None).unwrap();
        }
        assert!(runtime
            .new_object(None)
            .unwrap_err()
            .display()
            .contains("Object identity limit"));
        assert_eq!(runtime.objects.len(), MAX_OBJECTS);

        let (mut runtime, _) = test_runtime();
        let first_object = runtime.objects.len();
        for _ in 0..MAX_PROPERTIES / MAX_OBJECT_PROPERTIES {
            let id = runtime.new_object(None).unwrap();
            for key in 0..MAX_OBJECT_PROPERTIES.min(MAX_PROPERTIES - runtime.properties) {
                runtime
                    .set_property(id, key.to_string(), Value::Number(1.0))
                    .unwrap();
            }
        }
        let extra = runtime.new_object(None).unwrap();
        let before = runtime.value_bytes;
        assert!(runtime
            .set_property(extra, "extra".into(), Value::Null)
            .unwrap_err()
            .display()
            .contains("property limit"));
        assert_eq!(runtime.properties, MAX_PROPERTIES);
        assert_eq!(runtime.value_bytes, before);
        runtime
            .set_property(first_object, "0".into(), Value::Number(2.0))
            .unwrap();
        assert_eq!(
            runtime.get_property(first_object, "0").unwrap().to_text(),
            "2"
        );
    }

    #[test]
    fn array_expansion_and_object_bytes_obey_existing_job_budgets() {
        let (mut runtime, mut host) = test_runtime();
        let error = runtime
            .execute(
                "let large=Array(4096);large.join('0123456789012345678901234567890123456789');",
                &mut host,
            )
            .unwrap_err();
        assert!(error.contains("String limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("large.length;", &mut host)
                .unwrap()
                .to_text(),
            "4096"
        );

        let (mut runtime, _) = test_runtime();
        let object = runtime.new_object(None).unwrap();
        let initial_properties = runtime.properties;
        let mut filled = 0;
        loop {
            let before = runtime.value_bytes;
            match runtime.set_property(
                object,
                filled.to_string(),
                Value::String("x".repeat(MAX_STRING_BYTES)),
            ) {
                Ok(()) => filled += 1,
                Err(error) => {
                    assert!(error.display().contains("Retained value limit"));
                    assert_eq!(runtime.value_bytes, before);
                    break;
                }
            }
        }
        let capacity = MAX_RETAINED_VALUES / MAX_STRING_BYTES;
        assert!((capacity - 2..=capacity).contains(&filled));
        assert_eq!(runtime.properties, initial_properties + filled);
        runtime.begin_job();
        runtime
            .set_property(object, "0".into(), Value::String("small".into()))
            .unwrap();
        runtime
            .set_property(
                object,
                filled.to_string(),
                Value::String("x".repeat(MAX_STRING_BYTES)),
            )
            .unwrap();
        assert_eq!(runtime.properties, initial_properties + filled + 1);
    }

    #[test]
    fn parsing_finishes_before_any_script_effects() {
        for source in [
            "test.value=1; const thing={async *x(){return 1;}};",
            "test.value=1; class Nope {field=1;}",
            "test.value=1; let duplicate; let duplicate;",
            "test.value=1; function f(){break;}",
            "test.value=1; if(true) let local=1;",
            "test.value=1; const bad='\\u00g0';",
        ] {
            let (mut runtime, mut host) = test_runtime();
            assert!(runtime.execute(source, &mut host).is_err(), "{source}");
            assert!(
                host.properties.is_empty(),
                "Parsing ran a host mutation: {source}"
            );
        }
    }

    #[test]
    fn inline_handlers_compile_without_running_and_reject_wrapper_escape() {
        let (mut runtime, mut host) = test_runtime();
        let handler = runtime
            .compile_handler("this.value=event; test.record('clicked'); return false;")
            .unwrap();
        assert!(host.properties.is_empty());
        assert!(host.records.is_empty());
        assert_eq!(
            runtime
                .invoke(handler, &[Value::Number(3.0)], Value::Host(1), &mut host)
                .unwrap()
                .to_text(),
            "false"
        );
        assert_eq!(host.properties[&(1, "value".into())].to_text(), "3");
        assert_eq!(host.records, ["clicked"]);
        for source in [
            "});test.value='ran';(function(){",
            "})();test.value='ran';(function(event){",
            "});(function(event){",
        ] {
            assert!(
                runtime.compile_handler(source).is_err(),
                "Accepted escaped wrapper: {source}"
            );
        }
        assert_eq!(host.properties[&(1, "value".into())].to_text(), "3");
        assert_eq!(host.records, ["clicked"]);
    }

    #[test]
    fn errors_include_inner_source_location_and_leave_prior_effects() {
        let (mut runtime, mut host) = test_runtime();
        let error = runtime
            .execute("test.value=7;\nmissing;", &mut host)
            .unwrap_err();
        assert!(error.contains("line 2, column 1"), "{error}");
        assert_eq!(host.properties[&(1, "value".into())].to_text(), "7");
    }

    #[test]
    fn execution_budget_is_shared_until_the_host_begins_a_new_job() {
        let (mut runtime, mut host) = test_runtime();
        let callback = runtime
            .execute(
                "function work(){var n=0;while(n<1000)n++;return n;} work;",
                &mut host,
            )
            .unwrap();
        let mut completed = 0;
        loop {
            match runtime.invoke(callback.clone(), &[], Value::Undefined, &mut host) {
                Ok(value) => {
                    assert_eq!(value.to_text(), "1000");
                    completed += 1;
                }
                Err(error) => {
                    assert!(error.contains("step limit"), "{error}");
                    break;
                }
            }
            assert!(completed < 50);
        }
        assert!(completed > 1);
        assert!(runtime
            .execute("1;", &mut host)
            .unwrap_err()
            .contains("step limit"));
        runtime.begin_job();
        assert_eq!(
            runtime
                .invoke(callback, &[], Value::Undefined, &mut host)
                .unwrap()
                .to_text(),
            "1000"
        );
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
    }

    #[test]
    fn expired_execution_clock_is_fatal_and_next_job_recovers() {
        let (mut runtime, mut host) = test_runtime();
        runtime.begin_job();
        runtime.job_deadline = Instant::now() - Duration::from_millis(1);
        runtime.steps = MAX_STEPS & !255;
        let error = runtime
            .execute(
                "try{while(true);}catch(e){test.record('caught');}finally{test.record('finally');}",
                &mut host,
            )
            .unwrap_err();
        assert!(error.contains("Execution time limit"), "{error}");
        assert!(host.records.is_empty());
        runtime.begin_job();
        runtime
            .execute("test.record('recovered');", &mut host)
            .unwrap();
        assert_eq!(host.records, ["recovered"]);
    }

    #[test]
    fn accessor_recursion_and_native_getter_cycles_stop_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for source in [
                    "var o={};Object.defineProperty(o,'x',{get:function(){return this.x;}});o.x;",
                    "var o={};Object.defineProperty(o,'x',{set:function(v){this.x=v;}});o.x=1;",
                    "var o={},k=Symbol();Object.defineProperty(o,k,{get:function(){return this[k];}});o[k];",
                    // A borrowed native getter can recurse without entering a
                    // script-function frame. The accessor itself must guard it.
                    "var a=[];Object.defineProperty(a,'join',{get:Array.prototype.toString});a.join;",
                ] {
                    let (mut runtime, mut host) = test_runtime();
                    let error = runtime.execute(source, &mut host).unwrap_err();
                    assert!(error.contains("limit"), "{error}");
                    assert_eq!(runtime.call_depth, 0);
                    assert_eq!(runtime.eval_depth, 0);
                    runtime.begin_job();
                    assert_eq!(runtime.execute("6*7;", &mut host).unwrap().to_text(), "42");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn accessor_callbacks_share_job_work_and_keep_closures_between_jobs() {
        let (mut runtime, mut host) = test_runtime();
        runtime
            .define_global("payload", Value::String("x".repeat(MAX_STRING_BYTES / 2)))
            .unwrap();
        let error = runtime.execute(
            "var o={};Object.defineProperty(o,'x',{get:function(){return payload;}});try{while(true){o.x;}}catch(e){test.record('caught');}",
            &mut host,
        ).unwrap_err();
        assert!(error.contains("String/copy work limit"), "{error}");
        assert!(host.records.is_empty());
        assert_eq!(runtime.call_depth, 0);
        runtime.begin_job();
        runtime.execute(
            "var kept=(function(){let value=5;var o={};Object.defineProperty(o,'x',{get:function(){return value;},set:function(v){value=v;}});return o;})();",
            &mut host,
        ).unwrap();
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("kept.x=9;kept.x;", &mut host)
                .unwrap()
                .to_text(),
            "9"
        );
        runtime.begin_job();
        let error = runtime.execute(
            "var endless={};Object.defineProperty(endless,'x',{get:function(){while(true){}}});endless.x;",
            &mut host,
        ).unwrap_err();
        assert!(error.contains("limit"), "{error}");
        assert_eq!(runtime.call_depth, 0);
    }

    #[test]
    fn recursion_infinite_loops_and_flat_ast_chains_are_bounded() {
        // Retain the old 4000-group witness as an intentional acceptance case.
        assert_eq!(result(&format!("{}1{}", "(".repeat(4000), ")".repeat(4000))), "1");
        // Include the base-name frame: 4095 members plus the base exactly fit.
        // The original 4096-member witness remains a negative case below.
        let (mut accepted, mut recorder) = test_runtime();
        accepted.limits.heap.retained_values=64*1024*1024;
        assert!(matches!(accepted.execute(&format!("test{};", ".child".repeat(MAX_EVALUATION_FRAMES - 1)), &mut recorder).unwrap(), Value::Host(1)));
        for source in [
            "while(true);".to_string(),
            "function forever(){forever();} forever();".into(),
            format!("{}1{}", "(".repeat(MAX_EXPRESSION_FRAMES + 1), ")".repeat(MAX_EXPRESSION_FRAMES + 1)),
            format!("1{};", "+1".repeat(12_000)),
            format!("test{};", ".child".repeat(MAX_EVALUATION_FRAMES)),
            format!("{}true;", "!".repeat(4000)),
        ] {
            let (mut runtime, mut host) = test_runtime();
            runtime.limits.heap.retained_values=64*1024*1024;
            let error = runtime.execute(&source, &mut host).unwrap_err();
            assert!(error.contains("limit"), "{error}");
            runtime.begin_job();
            assert_eq!(runtime.execute("2+2;", &mut host).unwrap().to_text(), "4");
        }
    }

    #[test]
    fn hostile_nesting_stops_within_a_one_mib_thread_stack() {
        // Exercise the guards below the normal test thread's stack allowance.
        // Large evaluator match arms must not reserve their scratch space on
        // every level of an unrelated member/binary/sequence expression.
        std::thread::Builder::new()
            .name("hostile_nesting_stops_within_a_one_mib_thread_stack".into())
            .stack_size(1024 * 1024)
            .spawn(|| {
                eprintln!("hostile nesting: recursion/flat-chain guards");
                recursion_infinite_loops_and_flat_ast_chains_are_bounded();
                eprintln!("hostile nesting: fatal capability guards");
                resource_and_capability_failures_cannot_be_caught_or_suppressed();
                eprintln!("hostile nesting: accepted flat calls");
                let (mut accepted, mut recorder) = test_runtime();
                assert!(matches!(accepted.execute(&format!("function f(){{return f;}}f{};", "()".repeat(1000)), &mut recorder).unwrap(), Value::Function(_)));
                for (case, source) in [
                    format!("function f(){{return f;}}f{};", "()".repeat(MAX_EVALUATION_FRAMES)),
                    format!("{};", vec!["1"; 12_000].join(",")),
                    format!("{}1;{}", "try{".repeat(100), "}finally{}".repeat(100)),
                    "let f=()=>f();f();".into(),
                    format!("/{}a{}/;", "(?:".repeat(40), ")".repeat(40)),
                    "/(?:(?:(?:){1000}){1000}){1000}/;".into(),
                    format!("/(a)*/.test('{}');", "a".repeat(2000)),
                    format!(
                        "{}{{}}",
                        (0..4000).map(|i| format!("label{i}:")).collect::<String>()
                    ),
                    format!("{}1;", "for(let key in {a:1})".repeat(100)),
                    format!("{}1;", "while(true)".repeat(100)),
                ].into_iter().enumerate() {
                    eprintln!("hostile nesting: case {case}");
                    let (mut runtime, mut host) = test_runtime();
                    runtime.limits.heap.retained_values=64*1024*1024;
                    let error = runtime.execute(&source, &mut host).unwrap_err();
                    assert!(error.contains("limit"), "{error}");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn large_bundle_source_and_dense_code_run_with_unchanged_execution_bounds() {
        let (mut runtime, mut host) = test_runtime();
        let source = format!(
            "/*{}*/test.record('bootstrap');",
            "original fixture ".repeat(24_000)
        );
        assert!(source.len() > 369_762 && source.len() < MAX_SOURCE_BYTES);
        runtime.execute(&source, &mut host).unwrap();
        assert_eq!(host.records, ["bootstrap"]);
        runtime.begin_job();
        let dense = format!("{}test.record('parsed');", ";".repeat(70_000));
        runtime.limits.heap.retained_values=64*1024*1024;
        let dense=runtime.register_program(&dense,false).unwrap();
        // Isolate the existing dense top-level execution witness from the new
        // compilation-accounting scan; each phase keeps the same job budget.
        runtime.begin_job();
        let global=runtime.global_environment;
        runtime.prepare(&dense,&dense.body,global,true,&mut host).unwrap();
        let this=runtime.global_this().unwrap();
        assert!(matches!(runtime.statements(&dense,&dense.body,global,&this,&mut host).unwrap(),Control::Normal(_)));
        assert_eq!(host.records, ["bootstrap", "parsed"]);
        runtime.begin_job();
        let error = runtime.execute("while(true){}", &mut host).unwrap_err();
        assert!(error.contains("limit"), "{error}");
        runtime.begin_job();
        runtime
            .execute("test.record('recovered');", &mut host)
            .unwrap();
        assert_eq!(host.records.last().unwrap(), "recovered");
    }

    #[test]
    fn retained_bundle_source_limit_rejects_before_executing() {
        let (mut runtime, mut host) = test_runtime();
        runtime.limits.heap.retained_values=64*1024*1024;
        runtime.program_allocation.disabled = true;
        let source = format!("/*{}*/", "x".repeat(MAX_SOURCE_BYTES - 4));
        for _ in 0..MAX_RETAINED_SOURCE / MAX_SOURCE_BYTES {
            runtime.begin_job();
            runtime.execute(&source, &mut host).unwrap();
        }
        let retained = runtime.retained_source;
        runtime.begin_job();
        let error = runtime
            .execute("test.record('overflow');", &mut host)
            .unwrap_err();
        assert!(error.contains("Retained script limit"), "{error}");
        assert_eq!(runtime.retained_source, retained);
        assert!(host.records.is_empty());
        assert_eq!(
            runtime.programs.len(),
            MAX_RETAINED_SOURCE / MAX_SOURCE_BYTES
        );
    }

    #[test]
    fn source_string_and_argument_limits_are_explicit() {
        let (mut runtime, mut host) = test_runtime();
        assert!(runtime
            .execute(&" ".repeat(MAX_SOURCE_BYTES + 1), &mut host)
            .unwrap_err()
            .contains("source limit"));
        assert!(runtime
            .execute(
                &format!("let x='{}';x+x;", "x".repeat(MAX_STRING_BYTES / 2 + 1)),
                &mut host
            )
            .unwrap_err()
            .contains("String limit"));
        let source = format!("test.record({});", vec!["1"; MAX_ARGUMENTS + 1].join(","));
        assert!(runtime
            .execute(&source, &mut host)
            .unwrap_err()
            .contains("argument limit"));
        assert!(host.records.is_empty());
        let long = Value::String("x".repeat(MAX_STRING_BYTES + 1));
        assert!(runtime
            .define_global("long", long)
            .unwrap_err()
            .contains("String limit"));
    }

    #[test]
    fn temporary_strings_and_pathological_searches_share_work_limit() {
        let (mut runtime, mut host) = test_runtime();
        let haystack = "a".repeat(32_000);
        let needle = format!("{}b", "a".repeat(15_999));
        let source =
            format!("let haystack='{haystack}'; let needle='{needle}'; haystack.indexOf(needle);");
        assert!(runtime
            .execute(&source, &mut host)
            .unwrap_err()
            .contains("work limit"));
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("'abcd'.indexOf('bc');", &mut host)
                .unwrap()
                .to_text(),
            "1"
        );
        let source = format!(
            "let text='{}';for(var n=0;n<1000;n++)text.toUpperCase();",
            "b".repeat(16_000)
        );
        assert!(runtime
            .execute(&source, &mut host)
            .unwrap_err()
            .contains("work limit"));
    }

    #[test]
    fn retained_program_and_function_counts_are_bounded() {
        let (mut runtime, mut host) = test_runtime();
        runtime.program_allocation.disabled = true;
        for _ in 0..MAX_PROGRAMS {
            runtime.begin_job();
            runtime.execute("1;", &mut host).unwrap();
        }
        runtime.begin_job();
        assert!(runtime
            .execute("1;", &mut host)
            .unwrap_err()
            .contains("Retained script limit"));
        let (mut runtime, mut host) = test_runtime();
        let error = runtime
            .execute(
                "for(var i=0;i<1100;i++)test.listen(function(){return 1;});",
                &mut host,
            )
            .unwrap_err();
        assert!(error.contains("Function limit"), "{error}");
        assert!(host.callbacks.len() < MAX_FUNCTIONS);
    }

    #[test]
    fn transient_scopes_are_reclaimed_but_captured_scopes_stay_alive() {
        let (mut runtime, mut host) = test_runtime();
        let callback = runtime
            .execute(
                "let saved;function tick(){let n=4;{let temp=1;} return n;} tick;",
                &mut host,
            )
            .unwrap();
        for _ in 0..MAX_ENVIRONMENTS + 10 {
            runtime.begin_job();
            runtime
                .invoke(callback.clone(), &[], Value::Undefined, &mut host)
                .unwrap();
        }
        assert!(runtime.environments.len() < 10);
        assert!(
            runtime
                .environments
                .iter()
                .filter(|env| env.is_some())
                .count()
                < 3
        );
        runtime
            .execute(
                "function make(){let n=9;return function(){return n++;};} saved=make();",
                &mut host,
            )
            .unwrap();
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("saved()+saved();", &mut host)
                .unwrap()
                .to_text(),
            "19"
        );
    }
}

#[path="html_reflection_interface.rs"]
pub(crate) mod html_reflection_interface;

#[path="aria_reflection_interface.rs"]
pub(crate) mod aria_reflection_interface;

#[cfg(test)]
#[path="native_leaf_getter_tests.rs"]
mod native_leaf_getter_tests;

#[cfg(test)]
#[path = "string_from_char_code_utf16_tests.rs"]
mod string_from_char_code_utf16_tests;

#[path = "string_from_char_code.rs"]
mod string_from_char_code;

#[cfg(test)]
#[path = "character_position_tests.rs"]
mod character_position_tests;

#[path = "character_position.rs"]
mod character_position;

#[cfg(test)]
#[path = "string_trim_utf16_tests.rs"]
mod string_trim_utf16_tests;

#[path = "string_trim.rs"]
mod string_trim;

#[path="output_reset_interface.rs"]
mod output_reset_interface;

#[path="param_metadata_interface.rs"]
pub(crate) mod param_metadata_interface;
