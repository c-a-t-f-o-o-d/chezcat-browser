//! Canonical DOM Range interfaces over the native live-boundary store.
use super::*;
pub(crate) const STATIC: &str = "StaticRange";
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
pub(crate) const SURROUND_CONTENTS: &str = "\0Range.surroundContents";
pub(crate) const INSERT_NODE: &str = "\0Range.insertNode";
pub(crate) const EXTRACT_CONTENTS: &str = "\0Range.extractContents";
pub(crate) const DELETE_CONTENTS: &str = "\0Range.deleteContents";
pub(crate) const CONTEXTUAL_FRAGMENT: &str = "\0Range.createContextualFragment";
pub(crate) const CLONE_CONTENTS: &str = "\0Range.cloneContents";
pub(crate) const CLONE: &str = "\0Range.cloneRange";
pub(crate) const STRINGIFY: &str = "\0Range.toString";
pub(crate) const DETACH: &str = "\0Range.detach";
pub(crate) fn operation(method: &str) -> bool {
    matches!(
        method,
        RANGE | STATIC
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
            | CONTEXTUAL_FRAGMENT | CLONE_CONTENTS | DELETE_CONTENTS | EXTRACT_CONTENTS | INSERT_NODE | SURROUND_CONTENTS
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
            | COMPARE_POINT | POINT_IN_RANGE | INTERSECTS_NODE | INSERT_NODE | SURROUND_CONTENTS
    )
}
pub(crate) fn snapshot_getter(method:&str)->bool {matches!(method,START_NODE|END_NODE|START_OFFSET|END_OFFSET|COLLAPSED)}
pub(crate) fn arity(method: &str) -> usize {
    if matches!(method,STATIC|CONTEXTUAL_FRAGMENT) {return 1;}
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
            let mut node_parent=None;
            for name in [ABSTRACT, NODE_RANGE, RANGE, STATIC] {
                if name==STATIC {parent=node_parent;}
                let function = self
                    .allocate_host_function_with_length(window, name, usize::from(name==STATIC))
                    .map_err(Fault::new)?;
                if let Some(Function::Host {
                    constructible,
                    illegal_interface_constructor,
                    ..
                }) = self.functions.get_mut(function.id)
                {
                    *constructible = true;
                    *illegal_interface_constructor = !matches!(name,RANGE|STATIC);
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
                    STATIC => &[],
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
                        self.deferred_host_method(
                            prototype,
                            window,
                            property,
                            method,
                            arity(method),
                        )?;
                    }
                    self.deferred_host_method(prototype,window,"createContextualFragment",CONTEXTUAL_FRAGMENT,1)?;
                    self.deferred_host_method(prototype,window,"cloneContents",CLONE_CONTENTS,0)?;
                    self.deferred_host_method(prototype,window,"deleteContents",DELETE_CONTENTS,0)?;
                    self.deferred_host_method(prototype,window,"extractContents",EXTRACT_CONTENTS,0)?;
                    self.deferred_host_method(prototype,window,"insertNode",INSERT_NODE,1)?;
                    self.deferred_host_method(prototype,window,"surroundContents",SURROUND_CONTENTS,1)?;
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
                if name==NODE_RANGE {node_parent=parent;}
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

/// Native identities and scalar offsets only; no author callables are retained.
pub enum DeleteStep {
    Collapse { node:u32, offset:usize },
    Data { node:u32, offset:usize, count:usize },
    Remove { parent:u32, node:u32 },
}
impl Runtime {
    pub(super) fn delete_native_range(&mut self,receiver:u32,host:&mut dyn Host)->Result<Value,Fault>{
        let plan=host.prepare_range_delete(receiver).map_err(|error|match error{
            Error::Fatal(message)=>Fault::new(message),
            Error::Exception("TypeError",message)=>Fault::exception("TypeError",message),
            Error::Exception(kind,message)=>match self.native_dom_exception(kind,&message){Ok(value)=>Fault::thrown(value),Err(error)=>error},
        })?;
        let bytes=plan.capacity()*std::mem::size_of::<DeleteStep>();
        let result=(||->Result<Value,Fault>{
        self.work(plan.len())?;
        for step in plan {
            match step {
                DeleteStep::Collapse{node,offset}=>{
                    self.validate_native_access(node,host)?;
                    host.collapse_range_delete(receiver,node,offset).map_err(Fault::new)?;
                },
                DeleteStep::Data{node,offset,count}=>{
                    self.call_host(node,"\0CharacterData.deleteData",&[Value::Number(offset as f64),Value::Number(count as f64)],host)?;
                },
                DeleteStep::Remove{parent,node}=>{
                    self.call_host(parent,"removeChild",&[Value::Host(node)],host)?;
                },
            }
        }
        Ok(Value::Undefined)
        })();
        host.finish_range_delete(bytes);result
    }
}

pub enum ExtractStep {
    Fragment{document:u32,slot:usize},
    Clone{source:u32,slot:usize},
    Slice{source:u32,slot:usize,offset:usize,count:usize},
    Append{parent:usize,child:usize},
    Move{parent:usize,node:u32},
    Collapse{node:u32,offset:usize},
    Data{node:u32,offset:usize,count:usize},
}
pub struct ExtractPlan {
    pub steps:Vec<ExtractStep>,
    pub slots:Vec<Option<u32>>,
    pub bytes:usize,
}
impl Runtime {
    pub(super) fn extract_native_range(&mut self,receiver:u32,host:&mut dyn Host)->Result<Value,Fault>{
        let plan=host.prepare_range_extract(receiver).map_err(|error|match error{
            Error::Fatal(message)=>Fault::new(message),
            Error::Exception("TypeError",message)=>Fault::exception("TypeError",message),
            Error::Exception(kind,message)=>match self.native_dom_exception(kind,&message){Ok(value)=>Fault::thrown(value),Err(error)=>error},
        })?;
        let bytes=plan.bytes;
        let result=(||->Result<Value,Fault>{
            self.work(plan.steps.len())?;
            let mut slots=plan.slots;
            let get=|slots:&[Option<u32>],slot:usize|slots.get(slot).copied().flatten().ok_or_else(||Fault::new("Native extraction slot has not been produced."));
            for step in plan.steps {
                match step {
                    ExtractStep::Fragment{document,slot}=>{
                        let Value::Host(node)=self.call_host(document,"createDocumentFragment",&[],host)? else{return Err(Fault::new("Native extraction requires a fragment."));};slots[slot]=Some(node);
                    },
                    ExtractStep::Clone{source,slot}=>{
                        let Value::Host(node)=self.call_host(source,"\0Node.cloneNode",&[Value::Bool(false)],host)? else{return Err(Fault::new("Native extraction requires a cloned Node."));};slots[slot]=Some(node);
                    },
                    ExtractStep::Slice{source,slot,offset,count}=>{
                        let Value::Host(node)=self.call_host(source,"\0Node.cloneNode",&[Value::Bool(false)],host)? else{return Err(Fault::new("Native extraction requires a CharacterData clone."));};
                        let value=self.call_host(source,"\0CharacterData.substringData",&[Value::Number(offset as f64),Value::Number(count as f64)],host)?;
                        self.host_set_native(node,"data",value,host)?;slots[slot]=Some(node);
                    },
                    ExtractStep::Append{parent,child}=>{self.call_host(get(&slots,parent)?,"appendChild",&[Value::Host(get(&slots,child)?)],host)?;},
                    ExtractStep::Move{parent,node}=>{self.call_host(get(&slots,parent)?,"appendChild",&[Value::Host(node)],host)?;},
                    ExtractStep::Collapse{node,offset}=>{self.validate_native_access(node,host)?;host.collapse_range_delete(receiver,node,offset).map_err(Fault::new)?;},
                    ExtractStep::Data{node,offset,count}=>{self.call_host(node,"\0CharacterData.deleteData",&[Value::Number(offset as f64),Value::Number(count as f64)],host)?;},
                }
            }
            Ok(Value::Host(get(&slots,0)?))
        })();host.finish_range_extract(bytes);result
    }
}

pub struct InsertInfo {
    pub node:u32,
    pub parent:u32,
    pub reference:Option<u32>,
    pub split:Option<(u32,usize)>,
}
impl Runtime{
    pub(super) fn insert_native_range(&mut self,receiver:u32,args:&[Value],host:&mut dyn Host)->Result<Value,Fault>{
        let info=host.prepare_range_insert(receiver,args).map_err(|error|match error{
            Error::Fatal(message)=>Fault::new(message),
            Error::Exception("TypeError",message)=>Fault::exception("TypeError",message),
            Error::Exception(kind,message)=>match self.native_dom_exception(kind,&message){Ok(value)=>Fault::thrown(value),Err(error)=>error},
        })?;
        self.validate_native_access(info.parent,host)?;
        let reference=if let Some((text,offset))=info.split{
            let Value::Host(tail)=self.call_host(text,text_split_interface::METHOD,&[Value::Number(offset as f64)],host)? else{return Err(Fault::new("Native Range insertion requires a split Text node."));};Some(tail)
        }else{info.reference};
        let reference=host.range_insert_reference(reference,info.node).map_err(Fault::new)?;
        if let Some(parent)=host.child_removal_parent(info.node).map_err(Fault::new)?{self.call_host(parent,"removeChild",&[Value::Host(info.node)],host)?;}
        let offset=host.range_insert_position(info.parent,reference,info.node).map_err(Fault::new)?;
        self.call_host(info.parent,"insertBefore",&[Value::Host(info.node),reference.map_or(Value::Null,Value::Host)],host)?;
        host.finish_range_insert(receiver,info.parent,offset).map_err(Fault::new)?;Ok(Value::Undefined)
    }
}

impl Runtime{
    pub(super) fn surround_native_range(&mut self,receiver:u32,args:&[Value],host:&mut dyn Host)->Result<Value,Fault>{
        let parent=host.prepare_range_surround(receiver,args).map_err(|error|match error{
            Error::Fatal(message)=>Fault::new(message),
            Error::Exception("TypeError",message)=>Fault::exception("TypeError",message),
            Error::Exception(kind,message)=>match self.native_dom_exception(kind,&message){Ok(value)=>Fault::thrown(value),Err(error)=>error},
        })?;
        let fragment=self.call_host(receiver,EXTRACT_CONTENTS,&[],host)?;
        if matches!(self.call_host(parent,NODE_HAS_CHILDREN,&[],host)?,Value::Bool(true)){
            self.call_host(parent,NODE_TEXT_CONTENT_SET,&[Value::Null],host)?;
        }
        self.call_host(receiver,INSERT_NODE,&[Value::Host(parent)],host)?;
        host.validate_range_surround_append(parent).map_err(|error|match error{
            Error::Fatal(message)=>Fault::new(message),
            Error::Exception("TypeError",message)=>Fault::exception("TypeError",message),
            Error::Exception(kind,message)=>match self.native_dom_exception(kind,&message){Ok(value)=>Fault::thrown(value),Err(error)=>error},
        })?;
        self.call_host(parent,"appendChild",&[fragment],host)?;
        self.call_host(receiver,SELECT_NODE,&[Value::Host(parent)],host)?;
        Ok(Value::Undefined)
    }
}
