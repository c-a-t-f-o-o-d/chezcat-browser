//! Live TreeWalker traversal with NodeFilter masks.
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

impl Op {
    pub(super) fn arity(self) -> usize {
        usize::from(matches!(self, Self::Create | Self::SetCurrentNode))
    }
}

#[derive(Clone, Copy)]
struct Walker {
    root: u32,
    current: u32,
    mask: u32,
}

#[derive(Default)]
pub(super) struct Arena {
    pub(super) prototype: Option<usize>,
    walkers: HashMap<usize, Walker>,
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
        self.walkers.prototype = Some(prototype);

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
            let method = self.native_function(Builtin::TreeWalker(op), name)?;
            self.store_property(prototype, name.into(), method, Some((true, true, true)))?;
        }
        let document = *self.native_constructor_prototypes.get(&(window, "Document"))
            .ok_or_else(|| Fault::new("Document must precede TreeWalker."))?;
        let create = self.native_function(Builtin::TreeWalker(Op::Create), "createTreeWalker")?;
        self.store_property(document, "createTreeWalker".into(), create, Some((true, true, true)))?;
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
            Op::Filter => Ok(Value::Null),
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
                let node = self.traverse_tree_walker(walker, op, host)?;
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
        if let Some(filter) = args.get(2).filter(|value| !matches!(value, Value::Null | Value::Undefined)) {
            if !matches!(filter, Value::Host(_) | Value::Object(_) | Value::Function(_)) {
                return Err(Fault::exception("TypeError", "TreeWalker filter must be an object or null."));
            }
            let error = self.native_dom_exception("NotSupportedError", "TreeWalker callback filters are unavailable.")?;
            return Err(Fault::thrown(error));
        }
        if self.walkers.walkers.len() >= MAX_TREE_WALKERS {
            return Err(Fault::new("TreeWalker identity limit reached."));
        }
        self.walkers.walkers.try_reserve(1).map_err(|_| Fault::new("Cannot reserve TreeWalker state."))?;
        let id = self.new_object(None)?;
        self.objects[id].prototype = self.walkers.prototype.map(Ordinary::Object);
        self.walkers.walkers.insert(id, Walker { root, current: root, mask });
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

    fn tree_walker_accepts(&self, walker: Walker, node: u32, host: &dyn Host) -> Result<bool, Fault> {
        self.validate_native_access(node, host)?;
        let Some(kind) = host.tree_walker_node_type(node) else {
            return Err(Fault::new("TreeWalker encountered an inaccessible node."));
        };
        let bit = kind.checked_sub(1).and_then(|shift| 1u32.checked_shl(shift))
            .ok_or_else(|| Fault::new("TreeWalker node type exceeds the mask representation."))?;
        Ok(walker.mask & bit != 0)
    }

    fn traverse_tree_walker(&mut self, walker: Walker, op: Op, host: &mut dyn Host) -> Result<Option<u32>, Fault> {
        if walker.mask == 0 {
            return Ok(None);
        }
        let mut node = walker.current;
        match op {
            Op::Parent => {
                while node != walker.root {
                    let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { break };
                    if self.tree_walker_accepts(walker, parent, host)? { return Ok(Some(parent)); }
                    node = parent;
                }
                Ok(None)
            }
            Op::Child(forward) => self.tree_walker_child(walker, forward, host),
            Op::Sibling(forward) => self.tree_walker_sibling(walker, forward, host),
            Op::Next => {
                loop {
                    let next = if let Some(child) = self.tree_walker_link(node, "firstChild", host)? {
                        child
                    } else {
                        loop {
                            if node == walker.root { return Ok(None); }
                            if let Some(sibling) = self.tree_walker_link(node, "nextSibling", host)? {
                                break sibling;
                            }
                            let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { return Ok(None) };
                            node = parent;
                        }
                    };
                    if self.tree_walker_accepts(walker, next, host)? { return Ok(Some(next)); }
                    node = next;
                }
            }
            Op::Previous => {
                while node != walker.root {
                    if let Some(sibling) = self.tree_walker_link(node, "previousSibling", host)? {
                        node = sibling;
                        while let Some(child) = self.tree_walker_link(node, "lastChild", host)? { node = child; }
                    } else {
                        let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { break };
                        node = parent;
                    }
                    if self.tree_walker_accepts(walker, node, host)? { return Ok(Some(node)); }
                }
                Ok(None)
            }
            _ => unreachable!(),
        }
    }

    fn tree_walker_child(&mut self, walker: Walker, forward: bool, host: &mut dyn Host) -> Result<Option<u32>, Fault> {
        let (child, sibling) = if forward { ("firstChild", "nextSibling") } else { ("lastChild", "previousSibling") };
        let Some(mut node) = self.tree_walker_link(walker.current, child, host)? else { return Ok(None) };
        loop {
            if self.tree_walker_accepts(walker, node, host)? { return Ok(Some(node)); }
            if let Some(descendant) = self.tree_walker_link(node, child, host)? {
                node = descendant;
                continue;
            }
            loop {
                if let Some(peer) = self.tree_walker_link(node, sibling, host)? {
                    node = peer;
                    break;
                }
                let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { return Ok(None) };
                if parent == walker.root || parent == walker.current { return Ok(None); }
                node = parent;
            }
        }
    }

    fn tree_walker_sibling(&mut self, walker: Walker, forward: bool, host: &mut dyn Host) -> Result<Option<u32>, Fault> {
        let (child, sibling) = if forward { ("firstChild", "nextSibling") } else { ("lastChild", "previousSibling") };
        let mut node = walker.current;
        while node != walker.root {
            let mut next = self.tree_walker_link(node, sibling, host)?;
            while let Some(peer) = next {
                node = peer;
                if self.tree_walker_accepts(walker, node, host)? { return Ok(Some(node)); }
                next = match self.tree_walker_link(node, child, host)? {
                    Some(child) => Some(child),
                    None => self.tree_walker_link(node, sibling, host)?,
                };
            }
            let Some(parent) = self.tree_walker_link(node, "parentNode", host)? else { break };
            if parent == walker.root || self.tree_walker_accepts(walker, parent, host)? { break; }
            node = parent;
        }
        Ok(None)
    }
}
