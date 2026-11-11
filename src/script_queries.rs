//! Bounded selector queries and static NodeList snapshots. Querying never
//! constructs markup, runs scripts, changes resources or leaves this document.
//! Semantics: https://dom.spec.whatwg.org/#dom-parentnode-queryselectorall
use super::*;

pub(super) fn node_list_index(handle: u32) -> Option<usize> {
    (NODE_LIST_BASE..NODE_LIST_BASE + MAX_NODE_LISTS as u32)
        .contains(&handle)
        .then(|| (handle - NODE_LIST_BASE) as usize)
}

impl Bridge<'_> {
    pub(super) fn node_list_get(&mut self, handle: u32, property: &str) -> Result<Value, String> {
        let list = self
            .state
            .node_lists
            .get(node_list_index(handle).ok_or("Invalid NodeList handle.")?)
            .ok_or("Invalid NodeList handle.")?;
        if property == "length" {
            return Ok(Value::Number(list.len() as f64));
        }
        Ok(collection_index(property)
            .and_then(|index| list.get(index))
            .map_or(Value::Undefined, |&node| Value::Host(node_handle(node))))
    }

    pub(super) fn node_list_keys(&mut self, handle: u32) -> Result<Vec<String>, String> {
        let length = self
            .state
            .node_lists
            .get(node_list_index(handle).ok_or("Invalid NodeList handle.")?)
            .ok_or("Invalid NodeList handle.")?
            .len();
        self.state.work(length)?;
        self.state.copies(length.saturating_mul(4))?;
        Ok((0..length).map(|index| index.to_string()).collect())
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
            if name != "script" && dom_tree::inert_dom_tag(name, self.doc.scripting_enabled) {
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
            if name != "script" {
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
        let checked = |node| {
            self.forms
                .controls
                .get(&node)
                .filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
                .map(|control| control.checked)
        };
        if method == "querySelectorAll" {
            if self.state.node_lists.len() >= MAX_NODE_LISTS {
                return Err("DOM NodeList snapshot limit reached (128 per document).".into());
            }
            let nodes = crate::css::query_all(
                self.doc,
                source,
                &candidates,
                Some(&checked),
                &mut self.state.host_work,
            )?;
            let references = self.state.node_list_references.saturating_add(nodes.len());
            if references > MAX_NODE_LIST_REFERENCES {
                return Err(
                    "DOM NodeList retained-reference limit reached (16384 per document).".into(),
                );
            }
            self.state.work(nodes.len())?;
            self.state.node_list_references = references;
            let handle = NODE_LIST_BASE + self.state.node_lists.len() as u32;
            self.state.node_lists.push(nodes);
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
