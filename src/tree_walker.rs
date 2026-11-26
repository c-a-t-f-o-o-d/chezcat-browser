//! Live TreeWalker traversal with native NodeFilter callbacks and masks.
//! https://dom.spec.whatwg.org/#interface-treewalker
use super::*;
use std::collections::HashMap;

const MAX_TREE_WALKERS: usize = 65_536;
const FILTER_CONSTANTS: &[(&str, u32)] = &[
    ("FILTER_ACCEPT", 1), ("FILTER_REJECT", 2), ("FILTER_SKIP", 3),
    ("SHOW_ALL", u32::MAX), ("SHOW_ELEMENT", 1), ("SHOW_ATTRIBUTE", 2),
    ("SHOW_TEXT", 4), ("SHOW_CDATA_SECTION", 8), ("SHOW_ENTITY_REFERENCE", 16),
    ("SHOW_ENTITY", 32), ("SHOW_PROCESSING_INSTRUCTION", 64), ("SHOW_COMMENT", 128),
    ("SHOW_DOCUMENT", 256), ("SHOW_DOCUMENT_TYPE", 512), ("SHOW_DOCUMENT_FRAGMENT", 1024),
    ("SHOW_NOTATION", 2048),
];

#[derive(Clone, Copy)]
pub(super) enum Op {
    Interface,
    Create,
    Root,
    Mask,
    Filter,
    CurrentNode,
    SetCurrentNode,
    Parent,
    Child(bool),
    Sibling(bool),
    Previous,
    Next,
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("unexpected native read")}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected native write")}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("unexpected native call")}
        fn tree_walker_document(&self,id:u32)->bool{id==0}
        fn tree_walker_node_type(&self,id:u32)->Option<u32>{(id==1).then_some(1)}
    }
    #[test]
    fn tree_walker_filter_reachability_follows_only_reachable_walker_identity(){
        let mut runtime=Runtime::for_capacity_test();
        runtime.execute("function make(n){var box={};box.fn=function(){return n;};return box.fn;}var filter=make(29);0;",&mut NoHost).unwrap();
        let Value::Function(filter)=runtime.binding_value(runtime.global_environment,"filter").unwrap() else {panic!("function filter")};
        let held=runtime.create_tree_walker(&[Value::Host(1),Value::Number(1.0),Value::Function(filter)],Value::Host(0),&mut NoHost).unwrap();
        runtime.put_binding(runtime.global_environment,"held".into(),Some(held),Declaration::Var,true).unwrap();
        runtime.execute("filter=null;0;",&mut NoHost).unwrap();
        let refusal=runtime.retire_proven_inactive_candidate(filter).unwrap_err();
        assert!(refusal.display().contains("reachable"));
        runtime.execute("held=null;0;",&mut NoHost).unwrap();
        runtime.retire_proven_inactive_candidate(filter).unwrap();
        assert!(!runtime.function_is_occupied(filter));
    }
    #[test]
    fn tree_walker_filter_state_storage_refusal_precedes_identity_publication(){
        let mut runtime=Runtime::for_capacity_test();
        let before=(runtime.value_bytes,runtime.objects.len(),runtime.walkers.walkers.len());
        let limit=runtime.limits.heap.retained_values;
        runtime.limits.heap.retained_values=runtime.value_bytes;
        let result=runtime.create_tree_walker(&[Value::Host(1)],Value::Host(0),&mut NoHost);
        assert!(result.unwrap_err().display().contains("Retained value limit"));
        assert_eq!((runtime.value_bytes,runtime.objects.len(),runtime.walkers.walkers.len()),before);
        runtime.limits.heap.retained_values=limit;
        runtime.create_tree_walker(&[Value::Host(1)],Value::Host(0),&mut NoHost).unwrap();
        assert_eq!(runtime.value_bytes-before.0,std::mem::size_of::<Walker>());
        assert_eq!(runtime.walkers.walkers.len(),before.2+1);
    }
    #[test]
    fn tree_walker_filter_denied_native_owner_precedes_callback_and_cursor_change(){
        struct Guard;
        impl Host for Guard {
            fn get(&mut self,id:u32,key:&str)->Result<Value,String>{
                assert_eq!((id,key),(1,"firstChild"));Ok(Value::Host(2))
            }
            fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected write")}
            fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("unexpected call")}
            fn tree_walker_document(&self,id:u32)->bool{id==0}
            fn tree_walker_node_type(&self,_:u32)->Option<u32>{Some(1)}
            fn native_access_owner(&self,id:u32)->Option<u32>{(id==2).then_some(9)}
            fn validate_window_access(&self,_:Option<u32>,_:u32)->Result<(),String>{Err("denied native owner".into())}
        }
        let mut runtime=Runtime::for_capacity_test();
        runtime.execute("var calls=0;function filter(){calls++;return 1;}0;",&mut NoHost).unwrap();
        let filter=runtime.binding_value(runtime.global_environment,"filter").unwrap();
        let Value::Object(id)=runtime.create_tree_walker(&[Value::Host(1),Value::Number(1.0),filter],Value::Host(0),&mut Guard).unwrap() else {panic!("walker")};
        let error=runtime.tree_walker_builtin(Op::Next,&[],Value::Object(id),&mut Guard).unwrap_err();
        assert!(error.display().contains("denied native owner"));
        assert_eq!(runtime.binding_value(runtime.global_environment,"calls").unwrap().to_number(),0.0);
        assert_eq!(runtime.walkers.walkers[&id].current,1);
        assert!(!runtime.walkers.walkers[&id].active);
    }
}

impl Op {
    pub(super) fn arity(self) -> usize {
        usize::from(matches!(self, Self::Create | Self::SetCurrentNode))
    }
}

#[derive(Clone, Copy)]
enum Filter { Object(usize), Function(usize), Host(u32) }
impl Filter {
    fn value(self) -> Value {
        match self { Self::Object(id)=>Value::Object(id),Self::Function(id)=>Value::Function(id),Self::Host(id)=>Value::Host(id) }
    }
}

#[derive(Clone, Copy)]
struct Walker {
    root: u32,
    current: u32,
    mask: u32,
    filter: Option<Filter>,
    active: bool,
}

#[derive(Default)]
pub(super) struct Arena {
    pub(super) prototype: Option<usize>,
    walkers: HashMap<usize, Walker>,
}

impl Arena {
    pub(super) fn filter_value(&self, id: usize) -> Option<Value> {
        self.walkers.get(&id).and_then(|walker| walker.filter).map(Filter::value)
    }
}

impl Runtime {
    pub(crate) fn define_tree_walker(&mut self, window: u32) -> Result<(), String> {
        let marker = self.inhibit_transient_collection();
        let result = self.define_tree_walker_interfaces(window);
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    fn define_tree_walker_interfaces(&mut self, window: u32) -> Result<(), Fault> {
        let Value::Function(filter) = self.native_function(Builtin::TreeWalker(Op::Interface), "NodeFilter")?
            else { unreachable!() };
        let properties = self.function_properties[filter];
        for &(name, value) in FILTER_CONSTANTS {
            self.store_property(properties, name.into(), Value::Number(f64::from(value)), Some((true, false, false)))?;
        }
        self.put_binding(self.global_environment, "NodeFilter".into(), Some(Value::Function(filter)), Declaration::Var, false)?;
        self.global_attributes.insert("NodeFilter".into(), (false, true, true));

        let prototype = self.new_object(None)?;
        let constructor = self.native_function(Builtin::TreeWalker(Op::Interface), "TreeWalker")?;
        self.put_binding(self.global_environment, "TreeWalker".into(), Some(constructor), Declaration::Var, false)?;
        self.global_attributes.insert("TreeWalker".into(), (false, true, true));
        self.constructor_prototype("TreeWalker", Value::Object(prototype))?;
        self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(),
            Value::String("TreeWalker".into()), Some((false, false, true)))?;
        self.native_constructor_prototypes.insert((window, "TreeWalker"), prototype);
        if self.walkers.prototype.is_none() { self.walkers.prototype = Some(prototype); }

        for (name, op) in [("root", Op::Root), ("whatToShow", Op::Mask),
            ("filter", Op::Filter), ("currentNode", Op::CurrentNode)]
        {
            let Value::Function(get) = self.native_function(Builtin::TreeWalker(op), &format!("get {name}"))?
                else { unreachable!() };
            let set = if name == "currentNode" {
                let Value::Function(set) = self.native_function(
                    Builtin::TreeWalker(Op::SetCurrentNode), "set currentNode")?
                    else { unreachable!() };
                Some(set)
            } else { None };
            self.store_accessor_property(prototype, name.into(), Accessors { get: Some(get), set }, (true, true))?;
        }
        for (name, op) in [("parentNode", Op::Parent), ("firstChild", Op::Child(true)),
            ("lastChild", Op::Child(false)), ("nextSibling", Op::Sibling(true)),
            ("previousSibling", Op::Sibling(false)), ("previousNode", Op::Previous), ("nextNode", Op::Next)]
        {
            self.deferred_native_method(prototype, name, Builtin::TreeWalker(op))?;
        }
        let document = *self.native_constructor_prototypes.get(&(window, "Document"))
            .ok_or_else(|| Fault::new("Document must precede TreeWalker."))?;
        self.deferred_native_method(document, "createTreeWalker", Builtin::TreeWalker(Op::Create))?;
        Ok(())
    }

    pub(super) fn tree_walker_builtin(
        &mut self, op: Op, args: &[Value], this: Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match op {
            Op::Interface => return Err(Fault::exception("TypeError", "Illegal traversal interface constructor.")),
            Op::Create => return self.create_tree_walker(args, this, host),
            _ => {}
        }
        let Value::Object(id) = this else { return Err(Self::tree_walker_brand_error()) };
        let walker = self.walkers.walkers.get(&id).copied().ok_or_else(Self::tree_walker_brand_error)?;
        self.validate_native_access(walker.root, host)?;
        match op {
            Op::Root => Ok(Value::Host(walker.root)),
            Op::Mask => Ok(Value::Number(f64::from(walker.mask))),
            Op::Filter => Ok(walker.filter.map_or(Value::Null, Filter::value)),
            Op::CurrentNode => {
                self.validate_native_access(walker.current, host)?;
                Ok(Value::Host(walker.current))
            }
            Op::SetCurrentNode => {
                let node = self.tree_walker_node(args.first(), host)?;
                self.walkers.walkers.get_mut(&id).unwrap().current = node;
                Ok(Value::Undefined)
            }
            _ => {
                let node = self.traverse_tree_walker(id, walker, op, host)?;
                if let Some(node) = node {
                    self.walkers.walkers.get_mut(&id).unwrap().current = node;
                    Ok(Value::Host(node))
                } else { Ok(Value::Null) }
            }
        }
    }

    fn tree_walker_brand_error() -> Fault {
        Fault::exception("TypeError", "Incompatible TreeWalker receiver.")
    }

    fn tree_walker_node(&self, value: Option<&Value>, host: &dyn Host) -> Result<u32, Fault> {
        let Some(Value::Host(node)) = value else {
            return Err(Fault::exception("TypeError", "TreeWalker requires a Node."));
        };
        self.validate_native_access(*node, host)?;
        if host.tree_walker_node_type(*node).is_none() {
            return Err(Fault::exception("TypeError", "TreeWalker requires an accessible main-document Node."));
        }
        Ok(*node)
    }

    fn create_tree_walker(&mut self, args: &[Value], this: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let Value::Host(document) = this else {
            return Err(Fault::exception("TypeError", "createTreeWalker requires its Document receiver."));
        };
        self.validate_native_access(document, host)?;
        if !host.tree_walker_document(document) {
            return Err(Fault::exception("TypeError", "createTreeWalker requires its Document receiver."));
        }
        let root = self.tree_walker_node(args.first(), host)?;
        let mask = if let Some(mask) = args.get(1).filter(|value| !matches!(value, Value::Undefined)) {
            let value = self.copy_value(mask)?;
            int32(self.primitive_with_hint(value, false, host)?.script_number()?) as u32
        } else { u32::MAX };
        let filter = match args.get(2) {
            None | Some(Value::Null | Value::Undefined) => None,
            Some(Value::Object(id)) => Some(Filter::Object(*id)),
            Some(Value::Function(id)) => Some(Filter::Function(*id)),
            Some(Value::Host(id)) => Some(Filter::Host(*id)),
            _ => return Err(Fault::exception("TypeError", "TreeWalker filter must be an object or null.")),
        };
        // Numeric mask conversion can run author code. Recheck the captured
        // native inputs before reserving and publishing the walker identity.
        self.validate_native_access(document, host)?;
        self.validate_native_access(root, host)?;
        if !host.tree_walker_document(document) || host.tree_walker_node_type(root).is_none() {
            return Err(Fault::exception("TypeError", "TreeWalker native inputs are no longer accessible."));
        }
        if self.walkers.walkers.len() >= MAX_TREE_WALKERS {
            return Err(Fault::new("TreeWalker identity limit reached."));
        }
        let bytes = std::mem::size_of::<Walker>();
        let retained = self.value_bytes.checked_add(bytes)
            .filter(|&total| total <= self.limits.heap.retained_values)
            .ok_or_else(|| Fault::new("Retained value limit reached for TreeWalker state."))?;
        self.work(bytes)?;
        self.walkers.walkers.try_reserve(1).map_err(|_| Fault::new("Cannot reserve TreeWalker state."))?;
        let id = self.new_object(None)?;
        let prototype = if let Some((window, _)) = self.browser_global {
            self.native_constructor_prototypes.get(&(window, "TreeWalker")).copied()
        } else { self.walkers.prototype };
        self.objects[id].prototype = prototype.map(Ordinary::Object);
        self.walkers.walkers.insert(id, Walker { root, current: root, mask, filter, active: false });
        self.value_bytes = retained;
        Ok(Value::Object(id))
    }

    fn tree_walker_link(&mut self, node: u32, relationship: &str, host: &mut dyn Host) -> Result<Option<u32>, Fault> {
        self.step()?;
        match self.host_get_checked(node, relationship, host)? {
            Value::Host(node) => Ok(Some(node)),
            Value::Null => Ok(None),
            _ => Err(Fault::new("TreeWalker encountered an unrepresented node relationship.")),
        }
    }

    // DOM filtering keeps the active guard across operation lookup, invocation
    // and Web IDL unsigned-short conversion, including all abrupt completions.
    fn filter_tree_walker(&mut self, id: usize, node: u32, host: &mut dyn Host) -> Result<u16, Fault> {
        let walker = self.walkers.walkers[&id];
        if walker.active {
            return Err(Fault::thrown(self.native_dom_exception("InvalidStateError", "TreeWalker filter is already active.")?));
        }
        self.validate_native_access(node, host)?;
        let kind = host.tree_walker_node_type(node)
            .ok_or_else(|| Fault::new("TreeWalker encountered an inaccessible node."))?;
        let bit = kind.checked_sub(1).and_then(|shift| 1u32.checked_shl(shift))
            .ok_or_else(|| Fault::new("TreeWalker node type exceeds the mask representation."))?;
        if walker.mask & bit == 0 { return Ok(3); }
        let Some(filter) = walker.filter else { return Ok(1); };
        self.walkers.walkers.get_mut(&id).unwrap().active = true;
        let result = (|| {
            let callback = filter.value();
            let realm = if let Value::Function(function) = callback {
                self.function_realms.get(function).copied().unwrap_or(self.active_realm)
            } else { self.callback_interface_object_realm(&callback, host)? };
            self.with_intrinsic_realm(realm, |runtime| {
                let (function, this) = if matches!(callback, Value::Function(_)) {
                    (callback, Value::Undefined)
                } else {
                    (runtime.read_property(&callback, "acceptNode", host)?, callback)
                };
                let result = runtime.call_value(function, &[Value::Host(node)], this, host)?;
                Ok(int32(runtime.primitive_with_hint(result, false, host)?.script_number()?) as u16)
            })
        })();
        self.walkers.walkers.get_mut(&id).unwrap().active = false;
        result
    }

    fn traverse_tree_walker(&mut self, id: usize, walker: Walker, op: Op, host: &mut dyn Host) -> Result<Option<u32>, Fault> {
        let mut node = walker.current;
        match op {
            Op::Parent => {
                while node != walker.root {
                    let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { break };
                    node = parent;
                    if self.filter_tree_walker(id, node, host)? == 1 { return Ok(Some(node)); }
                }
                Ok(None)
            }
            Op::Child(forward) => self.tree_walker_child(id, walker, forward, host),
            Op::Sibling(forward) => self.tree_walker_sibling(id, walker, forward, host),
            Op::Next => {
                let mut result = 1;
                loop {
                    while result != 2 {
                        let Some(child) = self.tree_walker_link(node, "firstChild", host)? else { break };
                        node = child;
                        result = self.filter_tree_walker(id, node, host)?;
                        if result == 1 { return Ok(Some(node)); }
                    }
                    let mut temporary = node;
                    node = loop {
                        if temporary == walker.root { return Ok(None); }
                        if let Some(sibling) = self.tree_walker_link(temporary, "nextSibling", host)? { break sibling; }
                        let Some(parent) = self.tree_walker_link(temporary, "parentNode", host)? else { return Ok(None) };
                        temporary = parent;
                    };
                    result = self.filter_tree_walker(id, node, host)?;
                    if result == 1 { return Ok(Some(node)); }
                }
            }
            Op::Previous => {
                while node != walker.root {
                    let mut sibling = self.tree_walker_link(node, "previousSibling", host)?;
                    while let Some(peer) = sibling {
                        node = peer;
                        let mut result = self.filter_tree_walker(id, node, host)?;
                        while result != 2 {
                            let Some(child) = self.tree_walker_link(node, "lastChild", host)? else { break };
                            node = child;
                            result = self.filter_tree_walker(id, node, host)?;
                        }
                        if result == 1 { return Ok(Some(node)); }
                        sibling = self.tree_walker_link(node, "previousSibling", host)?;
                    }
                    if node == walker.root { return Ok(None); }
                    let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { return Ok(None) };
                    node = parent;
                    if self.filter_tree_walker(id, node, host)? == 1 { return Ok(Some(node)); }
                }
                Ok(None)
            }
            _ => unreachable!(),
        }
    }

    fn tree_walker_child(&mut self, id: usize, walker: Walker, forward: bool, host: &mut dyn Host) -> Result<Option<u32>, Fault> {
        let (child, sibling) = if forward { ("firstChild", "nextSibling") } else { ("lastChild", "previousSibling") };
        let Some(mut node) = self.tree_walker_link(walker.current, child, host)? else { return Ok(None) };
        loop {
            let result = self.filter_tree_walker(id, node, host)?;
            if result == 1 { return Ok(Some(node)); }
            if result == 3 {
                if let Some(descendant) = self.tree_walker_link(node, child, host)? {
                    node = descendant;
                    continue;
                }
            }
            loop {
                if let Some(peer) = self.tree_walker_link(node, sibling, host)? { node = peer; break; }
                let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { return Ok(None) };
                // Author callbacks may have changed the public cursor.
                if parent == walker.root || parent == self.walkers.walkers[&id].current { return Ok(None); }
                node = parent;
            }
        }
    }

    fn tree_walker_sibling(&mut self, id: usize, walker: Walker, forward: bool, host: &mut dyn Host) -> Result<Option<u32>, Fault> {
        let (child, sibling) = if forward { ("firstChild", "nextSibling") } else { ("lastChild", "previousSibling") };
        let mut node = walker.current;
        if node == walker.root { return Ok(None); }
        loop {
            let mut next = self.tree_walker_link(node, sibling, host)?;
            while let Some(peer) = next {
                node = peer;
                let result = self.filter_tree_walker(id, node, host)?;
                if result == 1 { return Ok(Some(node)); }
                next = self.tree_walker_link(node, child, host)?;
                if result == 2 || next.is_none() { next = self.tree_walker_link(node, sibling, host)?; }
            }
            let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { return Ok(None) };
            node = parent;
            if node == walker.root || self.filter_tree_walker(id, node, host)? == 1 { return Ok(None); }
        }
    }
}

#[cfg(test)]
mod inert_factory_tests {
 use super::*;
 #[test]
 fn inert_traversal_factory_rechecks_captured_document_authority_after_mask_conversion(){
  struct Guard{denied:bool}
  impl Host for Guard{
   fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Ok(Value::Undefined)}
   fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected native write")}
   fn method(&self,id:u32,key:&str)->Option<&'static str>{(id==0&&key=="revoke").then_some("revoke")}
   fn call(&mut self,id:u32,key:&str,_:&[Value])->Result<Value,String>{assert_eq!((id,key),(0,"revoke"));self.denied=true;Ok(Value::Undefined)}
   fn tree_walker_document(&self,id:u32)->bool{id==9}
   fn tree_walker_node_type(&self,id:u32)->Option<u32>{(id==1).then_some(1)}
   fn native_access_owner(&self,id:u32)->Option<u32>{(id==9).then_some(99)}
   fn validate_window_access(&self,_:Option<u32>,_:u32)->Result<(),String>{if self.denied{Err("denied captured document owner".into())}else{Ok(())}}
  }
  for iterator in [false,true]{
   let mut runtime=Runtime::for_capacity_test();let mut host=Guard{denied:false};
   runtime.put_binding(runtime.global_environment,"control".into(),Some(Value::Host(0)),Declaration::Var,true).unwrap();
   runtime.execute("var conversions=0;var mask={valueOf:function(){conversions++;control.revoke();return 1;}};0;",&mut host).unwrap();
   let mask=runtime.binding_value(runtime.global_environment,"mask").unwrap();let args=[Value::Host(1),mask];
   let result=if iterator{runtime.node_iterator_builtin(node_iterator::Op::Create,&args,Value::Host(9),&mut host)}else{runtime.tree_walker_builtin(Op::Create,&args,Value::Host(9),&mut host)};
   assert!(result.unwrap_err().display().contains("denied captured document owner"));
   assert_eq!(runtime.binding_value(runtime.global_environment,"conversions").unwrap().to_number(),1.0);
   host.denied=false;
   let made=if iterator{runtime.node_iterator_builtin(node_iterator::Op::Create,&[Value::Host(1)],Value::Host(9),&mut host)}else{runtime.tree_walker_builtin(Op::Create,&[Value::Host(1)],Value::Host(9),&mut host)}.unwrap();
   let root=if iterator{runtime.node_iterator_builtin(node_iterator::Op::Root,&[],made,&mut host)}else{runtime.tree_walker_builtin(Op::Root,&[],made,&mut host)}.unwrap();assert!(matches!(root,Value::Host(1)));
  }
 }
}
