//! Bounded selector snapshots and live name-filtered NodeLists. Querying never
//! constructs markup, runs scripts, changes resources or leaves this document.
//! Semantics: https://dom.spec.whatwg.org/#dom-parentnode-queryselectorall
use super::*;

pub(super) enum NodeList {
    Snapshot(Vec<usize>),
    Name(String),
    // A live SameObject list over this real node's immediate children.
    Children(usize),
}

pub(super) fn node_list_index(handle: u32) -> Option<usize> {
    (NODE_LIST_BASE..NODE_LIST_BASE + NODE_LIST_HANDLES as u32)
        .contains(&handle)
        .then(|| (handle - NODE_LIST_BASE) as usize)
}

impl Bridge<'_> {
    pub(super) fn node_list_iteration_member(&self,handle:u32,property:&str)->bool {
        // Iteration members and length use normal ordinary prototype lookup.
        // Existing item/forEach and protected native indices retain their
        // guarded routes; this does not expose other native collections.
        self.node_list_receiver(handle) && matches!(property,"constructor"|"length"|"keys"|"values"|"entries")
    }

    pub(super) fn node_list_receiver(&self, handle: u32) -> bool {
        node_list_index(handle).is_some_and(|index| index < self.state.node_lists.len())
    }

    fn reserve_node_list(&mut self, operation: &str) -> Result<u32, String> {
        let length = self.state.node_lists.len();
        if length >= self.state.limits.host.node_lists {
            return Err(format!("DOM NodeList {operation} limit reached ({} per document).", self.state.limits.host.node_lists));
        }
        if length >= NODE_LIST_HANDLES { return Err("DOM NodeList identity space exhausted.".into()); }
        let index = u32::try_from(length).map_err(|_| "DOM NodeList identity overflow.")?;
        let handle = NODE_LIST_BASE.checked_add(index).ok_or("DOM NodeList identity overflow.")?;
        self.state.node_lists.try_reserve(1).map_err(|_| "DOM NodeList storage reservation failed.")?;
        Ok(handle)
    }

    pub(super) fn empty_snapshot_node_list(&mut self) -> Result<u32, String> {
        self.state.work(1)?;
        let handle = self.reserve_node_list("identity")?;
        self.state.node_lists.push(NodeList::Snapshot(Vec::new()));
        Ok(handle)
    }

    pub(super) fn create_child_list(&mut self, object: u32) -> Result<Value, String> {
        let root = if object == DOCUMENT { 0 } else { self.dom_node(object)? };
        // The parser retains template input in an inert AST subtree, not a
        // DocumentFragment-backed template contents tree. Do not mislabel it.
        if tag(self.doc, root) == "template" {
            return Err("Template childNodes requires the unimplemented template contents tree.".into());
        }
        self.check_iframe_child_topology(root)?;
        self.state.work(1)?;
        if let Some(&handle) = self.state.dom.child_lists.get(&root) { return Ok(Value::Host(handle)); }
        let handle = self.reserve_node_list("identity")?;
        self.state.dom.child_lists.try_reserve(1).map_err(|_| "DOM childNodes identity reservation failed.")?;
        self.state.replace_bytes(0, std::mem::size_of::<usize>() + std::mem::size_of::<u32>())?;
        self.state.node_lists.push(NodeList::Children(root));
        self.state.dom.child_lists.insert(root, handle);
        Ok(Value::Host(handle))
    }

    pub(super) fn create_name_list(&mut self, value: &Value) -> Result<Value,String> {
        let name = self.text(value)?;
        let handle = self.reserve_node_list("identity")?;
        self.state.replace_bytes(0,name.len())?;
        self.state.node_lists.push(NodeList::Name(name));
        Ok(Value::Host(handle))
    }

    fn name_list_nodes(&mut self, index: usize) -> Result<Vec<usize>,String> {
        let NodeList::Name(name) = &self.state.node_lists[index] else { unreachable!() };
        self.state.copies(name.len())?;
        let NodeList::Name(name) = &self.state.node_lists[index] else { unreachable!() };
        let name = name.clone();
        let candidates = self.query_candidates(0)?;
        let mut found = Vec::new();
        for node in candidates {
            self.state.work(1)?;
            if self.doc.nodes[node].namespace != crate::html::Namespace::Html { continue; }
            let NodeKind::Element {attrs,..} = &self.doc.nodes[node].kind else { unreachable!() };
            self.state.work(attrs.len())?;
            self.state.copies(attrs.iter().map(|attr|attr.name.bytes()).sum())?;
            let value = self.doc.attr(node,"name");
            self.state.copies(value.map_or(0,str::len))?;
            if value == Some(name.as_str()) { found.push(node); }
        }
        Ok(found)
    }

    pub(super) fn node_list_get(&mut self, handle: u32, property: &str) -> Result<Value, String> {
        let index = node_list_index(handle).ok_or("Invalid NodeList handle.")?;
        let list = self
            .state
            .node_lists
            .get(index)
            .ok_or("Invalid NodeList handle.")?;
        if let NodeList::Children(root) = list {
            let root = *root;
            if property != "length" && collection_index(property).is_none() { return Ok(Value::Undefined); }
            // A replacement may have appended a new Text since this SameObject
            // list was obtained. Publish current topology/owner metadata before
            // returning its handle, including before cross-realm native access.
            self.state.rebuild(self.doc)?;
            self.state.work(1)?;
            self.check_iframe_child_topology(root)?;
            let children = &self.doc.nodes.get(root).ok_or("Invalid childNodes owner.")?.children;
            if property == "length" { return Ok(Value::Number(children.len() as f64)); }
            return Ok(collection_index(property).and_then(|index| children.get(index))
                .map_or(Value::Undefined, |&node| Value::Host(node_handle(node))));
        }
        if matches!(list,NodeList::Name(_)) {
            if property != "length" && collection_index(property).is_none() { return Ok(Value::Undefined); }
            let nodes = self.name_list_nodes(index)?;
            return if property == "length" { Ok(Value::Number(nodes.len() as f64)) }
                else { Ok(collection_index(property).and_then(|index|nodes.get(index)).map_or(Value::Undefined,|&node|Value::Host(node_handle(node)))) };
        }
        let NodeList::Snapshot(list) = list else { unreachable!() };
        if property == "length" {
            return Ok(Value::Number(list.len() as f64));
        }
        Ok(collection_index(property)
            .and_then(|index| list.get(index))
            .map_or(Value::Undefined, |&node| Value::Host(node_handle(node))))
    }

    pub(super) fn node_list_keys(&mut self, handle: u32) -> Result<Vec<String>, String> {
        let Value::Number(length) = self.node_list_get(handle,"length")? else { unreachable!() };
        let length = length as usize;
        self.state.work(length)?;
        self.state.copies(length.checked_mul(20).ok_or("NodeList key accounting overflow.")?)?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(length).map_err(|_| "NodeList enumeration reservation failed.")?;
        for index in 0..length { keys.push(index.to_string()); }
        Ok(keys)
    }

    pub(super) fn query_candidates(&mut self, root: usize) -> Result<Vec<usize>, String> {
        self.state.rebuild(self.doc)?;
        let mut candidates = Vec::new();
        let mut stack = self.doc.nodes[root]
            .children
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        while let Some(node) = stack.pop() {
            self.state.work(1)?;
            let name = tag(self.doc, node);
            let inert_children = dom_tree::inert_dom_tag(name, self.doc.scripting_enabled);
            if inert_children && !dom_tree::exposes_inert_host(name, self.doc.nodes[node].namespace) {
                continue;
            }
            if !self.state.tree.active.get(node).copied().unwrap_or(false)
                && !self.state.detached_nodes.contains(&node)
            {
                continue;
            }
            if matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) {
                candidates.push(node);
            }
            if !inert_children {
                stack.extend(self.doc.nodes[node].children.iter().rev().copied());
            }
        }
        Ok(candidates)
    }

    pub(super) fn selector_query(
        &mut self,
        object: u32,
        method: &str,
        source: &str,
    ) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        let root = if object == DOCUMENT {
            0
        } else {
            self.node(object)?
        };
        let candidates = match method {
            "matches" => vec![root],
            "closest" => {
                let mut candidates = Vec::new();
                let mut current = Some(root);
                while let Some(node) = current.filter(|&node| node != 0) {
                    self.state.work(1)?;
                    candidates.push(node);
                    current = self.state.tree.parents[node];
                }
                candidates
            }
            _ => self.query_candidates(root)?,
        };
        // Reserve the optional snapshot identity before the checked-state
        // closure borrows Forms; no callback or publication occurs here.
        let snapshot_handle = if method == "querySelectorAll" {
            Some(self.reserve_node_list("snapshot")?)
        } else { None };
        let checked = |node| {
            self.forms
                .controls
                .get(&node)
                .filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
                .map(|control| control.checked)
        };
        if let Some(handle) = snapshot_handle {
            let nodes = crate::css::query_all(
                self.doc,
                source,
                &candidates,
                Some(&checked),
                &mut self.state.host_work,
            )?;
            let references = self.state.node_list_references.checked_add(nodes.len())
                .ok_or("DOM NodeList retained-reference accounting overflow.")?;
            if references > self.state.limits.host.node_list_references {
                return Err(format!("DOM NodeList retained-reference limit reached ({} per document).", self.state.limits.host.node_list_references));
            }
            self.state.work(nodes.len())?;
            self.state.node_list_references = references;
            self.state.node_lists.push(NodeList::Snapshot(nodes));
            Ok(Value::Host(handle))
        } else {
            let found = crate::css::query_first(
                self.doc,
                source,
                &candidates,
                Some(&checked),
                &mut self.state.host_work,
            )?;
            if method == "matches" {
                Ok(Value::Bool(found.is_some()))
            } else {
                Ok(found.map_or(Value::Null, |node| Value::Host(node_handle(node))))
            }
        }
    }
}
