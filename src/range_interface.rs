//! Canonical DOM Range interfaces over the native live-boundary store.
use super::*;
pub(crate) const RANGE: &str = "Range";
pub(crate) const ABSTRACT: &str = "AbstractRange";
pub(crate) const NODE_RANGE: &str = "NodeRange";
pub(crate) const CREATE: &str = "createRange";
pub(crate) const START_NODE: &str = "\0Range.startContainer";
pub(crate) const END_NODE: &str = "\0Range.endContainer";
pub(crate) const START_OFFSET: &str = "\0Range.startOffset";
pub(crate) const END_OFFSET: &str = "\0Range.endOffset";
pub(crate) const COLLAPSED: &str = "\0Range.collapsed";
pub(crate) const COMMON: &str = "\0Range.commonAncestorContainer";
pub(crate) const SET_START: &str = "\0Range.setStart";
pub(crate) const SET_END: &str = "\0Range.setEnd";
pub(crate) const START_BEFORE: &str = "\0Range.setStartBefore";
pub(crate) const START_AFTER: &str = "\0Range.setStartAfter";
pub(crate) const END_BEFORE: &str = "\0Range.setEndBefore";
pub(crate) const END_AFTER: &str = "\0Range.setEndAfter";
pub(crate) const SELECT_NODE: &str = "\0Range.selectNode";
pub(crate) const SELECT_CONTENTS: &str = "\0Range.selectNodeContents";
pub(crate) const COLLAPSE: &str = "\0Range.collapse";
pub(crate) const COMPARE_BOUNDARIES: &str = "\0Range.compareBoundaryPoints";
pub(crate) const COMPARE_POINT: &str = "\0Range.comparePoint";
pub(crate) const POINT_IN_RANGE: &str = "\0Range.isPointInRange";
pub(crate) const INTERSECTS_NODE: &str = "\0Range.intersectsNode";
pub(crate) fn comparison(method:&str)->bool {matches!(method,COMPARE_BOUNDARIES|COMPARE_POINT|POINT_IN_RANGE|INTERSECTS_NODE)}
pub(crate) const CLONE_CONTENTS: &str = "\0Range.cloneContents";
pub(crate) const CLONE: &str = "\0Range.cloneRange";
pub(crate) const STRINGIFY: &str = "\0Range.toString";
pub(crate) const DETACH: &str = "\0Range.detach";
pub(crate) fn operation(method: &str) -> bool {
    matches!(
        method,
        RANGE
            | ABSTRACT
            | NODE_RANGE
            | CREATE
            | START_NODE
            | END_NODE
            | START_OFFSET
            | END_OFFSET
            | COLLAPSED
            | COMMON
            | SET_START
            | SET_END
            | START_BEFORE
            | START_AFTER
            | END_BEFORE
            | END_AFTER
            | SELECT_NODE
            | SELECT_CONTENTS
            | COLLAPSE
            | CLONE
            | CLONE_CONTENTS
            | COMPARE_BOUNDARIES | COMPARE_POINT | POINT_IN_RANGE | INTERSECTS_NODE
            | DETACH
            | STRINGIFY
    )
}
pub(crate) fn node_argument(method: &str) -> bool {
    matches!(
        method,
        SET_START
            | SET_END
            | START_BEFORE
            | START_AFTER
            | END_BEFORE
            | END_AFTER
            | SELECT_NODE
            | SELECT_CONTENTS
            | COMPARE_POINT | POINT_IN_RANGE | INTERSECTS_NODE
    )
}
pub(crate) fn arity(method: &str) -> usize {
    if matches!(method, SET_START | SET_END | COMPARE_BOUNDARIES | COMPARE_POINT | POINT_IN_RANGE) {
        2
    } else {
        usize::from(node_argument(method))
    }
}
impl Runtime {
    pub(crate) fn define_range_interfaces(&mut self, window: u32) -> Result<(), String> {
        let marker = self.inhibit_transient_collection();
        let result = (|| -> Result<(), Fault> {
            let mut parent: Option<(usize, usize)> = None;
            for name in [ABSTRACT, NODE_RANGE, RANGE] {
                let function = self
                    .allocate_host_function_with_length(window, name, 0)
                    .map_err(Fault::new)?;
                if let Some(Function::Host {
                    constructible,
                    illegal_interface_constructor,
                    ..
                }) = self.functions.get_mut(function.id)
                {
                    *constructible = true;
                    *illegal_interface_constructor = name != RANGE;
                }
                let prototype = self.new_object(None)?;
                let properties = self.function_properties[function.id];
                if let Some((proto, ctor)) = parent {
                    self.objects[prototype].prototype = Some(Ordinary::Object(proto));
                    self.objects[properties].prototype = Some(Ordinary::Function(ctor));
                }
                self.store_property(
                    prototype,
                    "constructor".into(),
                    Value::Function(function.id),
                    Some((false, true, true)),
                )?;
                self.store_property(
                    properties,
                    "prototype".into(),
                    Value::Object(prototype),
                    Some((false, false, false)),
                )?;
                self.store_symbol_property(
                    prototype,
                    self.symbol_to_string_tag.unwrap(),
                    Value::String(name.into()),
                    Some((false, false, true)),
                )?;
                let getters: &[(&str, &str)] = match name {
                    ABSTRACT => &[
                        ("startOffset", START_OFFSET),
                        ("endOffset", END_OFFSET),
                        ("collapsed", COLLAPSED),
                    ],
                    NODE_RANGE => &[("startContainer", START_NODE), ("endContainer", END_NODE)],
                    _ => &[("commonAncestorContainer", COMMON)],
                };
                for &(property, method) in getters {
                    self.define_readonly_node_getter(prototype, window, property, method)?;
                }
                if name == RANGE {
                    for (property, method) in [
                        ("setStart", SET_START),
                        ("setEnd", SET_END),
                        ("setStartBefore", START_BEFORE),
                        ("setStartAfter", START_AFTER),
                        ("setEndBefore", END_BEFORE),
                        ("setEndAfter", END_AFTER),
                        ("selectNode", SELECT_NODE),
                        ("selectNodeContents", SELECT_CONTENTS),
                        ("collapse", COLLAPSE),
                        ("cloneRange", CLONE),
                        ("detach", DETACH),
                        ("toString", STRINGIFY),
                    ] {
                        self.define_canvas_operation(
                            prototype,
                            window,
                            property,
                            method,
                            arity(method),
                        )?;
                    }
                    self.deferred_host_method(prototype,window,"cloneContents",CLONE_CONTENTS,0)?;
                    for (property,method) in [("compareBoundaryPoints",COMPARE_BOUNDARIES),("comparePoint",COMPARE_POINT),("isPointInRange",POINT_IN_RANGE),("intersectsNode",INTERSECTS_NODE)] {
                        self.deferred_host_method(prototype,window,property,method,arity(method))?;
                    }
                    for (key, value) in [
                        ("START_TO_START", 0),
                        ("START_TO_END", 1),
                        ("END_TO_END", 2),
                        ("END_TO_START", 3),
                    ] {
                        for object in [prototype, properties] {
                            self.store_property(
                                object,
                                key.into(),
                                Value::Number(f64::from(value)),
                                Some((true, false, false)),
                            )?;
                        }
                    }
                }
                self.native_constructor_prototypes
                    .insert((window, name), prototype);
                self.put_binding(
                    self.global_environment,
                    name.into(),
                    Some(Value::Function(function.id)),
                    Declaration::Var,
                    false,
                )?;
                self.global_attributes
                    .insert(name.into(), (false, true, true));
                parent = Some((prototype, function.id));
            }
            let document = self.native_constructor_prototypes[&(window, "Document")];
            self.define_canvas_operation(document, window, CREATE, CREATE, 0)?;
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }
}

pub(crate) enum Error {
    Exception(&'static str, String),
    Fatal(String),
}
