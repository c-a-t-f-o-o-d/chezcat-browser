//! Bounded DOM creation, tree mutation and named form lookup. Node identities
//! remain in the document arena until realm teardown. Loaded resources and the
//! native form model are protected from unsupported structural mutations.
//! Semantics: https://dom.spec.whatwg.org/#concept-node-pre-insert
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-nameditem
use super::*;

const MAX_DOM_DEPTH: usize = 128;

#[derive(Default)]
pub(super) struct DomState {
    pub(super) created: HashSet<usize>,
    pub(super) connected: Vec<bool>,
    pub(super) pending_focus: Option<usize>,
    pub(super) collections: Vec<dom_collections::TagCollection>,
    force_async: HashSet<usize>,
}

pub(super) fn inert_dom_tag(name: &str, scripting: bool) -> bool {
    matches!(name, "script" | "style" | "template" | "iframe" | "object")
        || name == "noscript" && scripting
}

fn common_node_property(name: &str) -> bool {
    matches!(
        name,
        "nodeName"
            | "nodeType"
            | "parentNode"
            | "parentElement"
            | "ownerDocument"
            | "isConnected"
            | "firstChild"
            | "lastChild"
            | "nextSibling"
            | "previousSibling"
    )
}

fn script_attribute(property: &str) -> Option<&'static str> {
    Some(match property {
        "src" => "src",
        "type" => "type",
        "charset" => "charset",
        "nonce" => "nonce",
        "integrity" => "integrity",
        "crossOrigin" => "crossorigin",
        "referrerPolicy" => "referrerpolicy",
        "fetchPriority" => "fetchpriority",
        "async" => "async",
        "defer" => "defer",
        "noModule" => "nomodule",
        _ => return None,
    })
}

fn boolean_script_attribute(name: &str) -> bool {
    matches!(name, "async" | "defer" | "nomodule")
}

impl State {
    pub(super) fn rebuild_dom_tree(&mut self, doc: &Document) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        // Active lookup deliberately excludes inert subtrees; physical parent
        // links and connection state must still describe created script nodes.
        self.work(doc.nodes.len())?;
        let mut tree = Tree {
            active: vec![false; doc.nodes.len()],
            parents: vec![None; doc.nodes.len()],
            ..Tree::default()
        };
        let mut connected = vec![false; doc.nodes.len()];
        let mut stack = if doc.nodes.is_empty() {
            Vec::new()
        } else {
            vec![(0, None, false)]
        };
        while let Some((node, parent, inert)) = stack.pop() {
            self.work(1)?;
            if node >= doc.nodes.len() || connected[node] {
                return Err("DOM tree has an invalid child or repeated parent.".into());
            }
            connected[node] = true;
            tree.parents[node] = parent;
            let name = tag(doc, node);
            let inert = inert || name != "script" && inert_dom_tag(name, doc.scripting_enabled);
            if !inert {
                tree.active[node] = true;
                tree.order.push(node);
                if name == "body" && tree.body.is_none() {
                    tree.body = Some(node);
                }
                if name == "head" && tree.head.is_none() {
                    tree.head = Some(node);
                }
                if parent == Some(0)
                    && tree.document_element.is_none()
                    && matches!(doc.nodes[node].kind, NodeKind::Element { .. })
                {
                    tree.document_element = Some(node);
                }
                if let Some(id) = doc
                    .attr(node, "id")
                    .filter(|id| !id.is_empty() && id.len() <= MAX_STRING)
                {
                    self.copies(id.len())?;
                    tree.ids.entry(id.to_owned()).or_insert(node);
                }
            }
            stack.extend(
                doc.nodes[node]
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, Some(node), inert || name == "script")),
            );
        }
        // Saved handles and static NodeLists retain detached ordinary subtrees,
        // whose internal topology is separate from connected document lookup.
        self.work(self.detached_nodes.len())?;
        self.work(
            self.detached_nodes
                .iter()
                .map(|&node| doc.nodes[node].children.len())
                .sum(),
        )?;
        for &parent in &self.detached_nodes {
            for &child in &doc.nodes[parent].children {
                if self.detached_nodes.contains(&child) {
                    tree.parents[child] = Some(parent);
                }
            }
        }
        self.tree = tree;
        self.dom.connected = connected;
        self.dirty = false;
        Ok(())
    }
}

impl Page {
    pub fn take_focus_request(&mut self) -> Option<usize> {
        self.state.dom.pending_focus.take()
    }
}

impl Bridge<'_> {
    fn dom_accessible(&self, node: usize) -> bool {
        node != 0
            && node < self.doc.nodes.len()
            && (self.state.tree.active.get(node).copied().unwrap_or(false)
                || self.state.detached_nodes.contains(&node)
                || self.state.detached_images.contains(&node)
                || self.state.dom.created.contains(&node))
    }

    fn dom_node(&mut self, handle: u32) -> Result<usize, String> {
        self.state.rebuild(self.doc)?;
        node_index(handle)
            .filter(|&node| self.dom_accessible(node))
            .ok_or_else(|| "DOM node is inert or unavailable in this subset.".into())
    }

    pub(super) fn dom_stylesheet(&self, node: usize) -> bool {
        tag(self.doc, node) == "link" && self.state.dom.created.contains(&node)
    }

    fn dom_script(&self, node: usize) -> bool {
        tag(self.doc, node) == "script" && self.state.dom.created.contains(&node)
    }

    pub(super) fn dom_method(&self, object: u32, property: &str) -> Option<&'static str> {
        let node = node_index(object).filter(|&node| node < self.doc.nodes.len())?;
        match property {
            "appendChild" => Some("appendChild"),
            "insertBefore" => Some("insertBefore"),
            "removeChild" => Some("removeChild"),
            "focus" if matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) => {
                Some("focus")
            }
            _ => None,
        }
    }

    pub(super) fn dom_readonly(&self, object: u32, property: &str) -> bool {
        node_index(object).is_some() && common_node_property(property)
    }

    pub(super) fn dom_string_setter(&self, object: u32, property: &str) -> bool {
        let Some(node) = node_index(object).filter(|&node| node < self.doc.nodes.len()) else {
            return false;
        };
        self.dom_script(node)
            && script_attribute(property).is_some_and(|name| !boolean_script_attribute(name))
            || self.dom_stylesheet(node)
                && script_stylesheets::link_attribute(property)
                    .is_some_and(|name| name != "disabled")
            || matches!(self.doc.nodes[node].kind, NodeKind::Text(_)) && property == "data"
    }

    pub(super) fn dom_validate_call(
        &self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if object == DOCUMENT && method == "createElement" {
            if let Some(Value::String(name)) = args.first() {
                // This HTML subset accepts bounded ASCII element names. Other
                // namespaces and customized built-in element options are absent.
                if name.is_empty()
                    || name.len() > 128
                    || !name.as_bytes()[0].is_ascii_alphabetic()
                    || !name.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
                    })
                {
                    return Err((
                        "InvalidCharacterError",
                        "Element name is outside the supported ASCII name syntax (128 bytes)."
                            .into(),
                    ));
                }
            }
        }
        if node_index(object).is_some()
            && matches!(method, "appendChild" | "insertBefore" | "removeChild")
        {
            if !matches!(args.first(), Some(Value::Host(handle)) if node_index(*handle).is_some()) {
                return Err(("TypeError", format!("{method} requires a DOM node.")));
            }
            if method == "insertBefore"
                && !matches!(
                    args.get(1),
                    Some(Value::Null | Value::Undefined) | Some(Value::Host(_))
                )
            {
                return Err((
                    "TypeError",
                    "insertBefore requires a node or null reference.".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn dom_has(&mut self, object: u32, property: &str) -> Result<Option<bool>, String> {
        let Some(node) = node_index(object) else {
            return Ok(None);
        };
        let node = self.dom_node(node_handle(node))?;
        if self.frame_content(node, property).is_some() { return Ok(Some(true)); }
        if self.dom_stylesheet(node)
            && (script_stylesheets::link_attribute(property).is_some()
                || matches!(property, "onload" | "onerror"))
        {
            return Ok(Some(true));
        }
        if common_node_property(property) || self.dom_method(object, property).is_some() {
            return Ok(Some(true));
        }
        if matches!(self.doc.nodes[node].kind, NodeKind::Text(_)) {
            return Ok(Some(matches!(
                property,
                "data" | "nodeValue" | "textContent" | "length"
            )));
        }
        if tag(self.doc, node) == "script" && matches!(property, "nonce" | "text" | "textContent") {
            return Ok(Some(true));
        }
        if self.dom_script(node)
            && (script_attribute(property).is_some()
                || matches!(property, "text" | "textContent" | "onload" | "onerror"))
        {
            return Ok(Some(true));
        }
        Ok(None)
    }

    pub(super) fn dom_get(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if node_index(object).is_none() {
            return Ok(None);
        }
        let node = self.dom_node(object)?;
        let text = matches!(self.doc.nodes[node].kind, NodeKind::Text(_));
        if let Some(value) = self.frame_content(node, property) { return Ok(Some(value)); }
        if common_node_property(property) {
            return Ok(Some(match property {
                "nodeType" => Value::Number(if text { 3.0 } else { 1.0 }),
                "nodeName" => {
                    let name = if text {
                        "#text".into()
                    } else {
                        tag(self.doc, node).to_ascii_uppercase()
                    };
                    return self.string(&name).map(Some);
                }
                "ownerDocument" => Value::Host(DOCUMENT),
                "isConnected" => Value::Bool(self.state.dom.connected[node]),
                "parentNode" => return self.element_parent(node, true).map(Some),
                "parentElement" => return self.element_parent(node, false).map(Some),
                "firstChild" | "lastChild" => {
                    self.state.work(self.doc.nodes[node].children.len())?;
                    let children = &self.doc.nodes[node].children;
                    let child = if property == "firstChild" {
                        children
                            .iter()
                            .copied()
                            .find(|&child| self.dom_accessible(child))
                    } else {
                        children
                            .iter()
                            .rev()
                            .copied()
                            .find(|&child| self.dom_accessible(child))
                    };
                    child.map_or(Value::Null, |child| Value::Host(node_handle(child)))
                }
                _ => {
                    let mut found = None;
                    if let Some(parent) = self.state.tree.parents[node] {
                        self.state.work(self.doc.nodes[parent].children.len())?;
                        let siblings = &self.doc.nodes[parent].children;
                        if let Some(index) = siblings.iter().position(|&child| child == node) {
                            found = if property == "nextSibling" {
                                siblings[index + 1..]
                                    .iter()
                                    .copied()
                                    .find(|&child| self.dom_accessible(child))
                            } else {
                                siblings[..index]
                                    .iter()
                                    .rev()
                                    .copied()
                                    .find(|&child| self.dom_accessible(child))
                            };
                        }
                    }
                    found.map_or(Value::Null, |child| Value::Host(node_handle(child)))
                }
            }));
        }
        if let NodeKind::Text(value) = &self.doc.nodes[node].kind {
            return Ok(Some(match property {
                "data" | "nodeValue" | "textContent" => {
                    if value.len() > MAX_STRING {
                        return Err("DOM result exceeds 32 KiB.".into());
                    }
                    self.state.copies(value.len())?;
                    Value::String(value.clone())
                }
                "length" => {
                    self.state.copies(value.len())?;
                    Value::Number(value.encode_utf16().count() as f64)
                }
                _ => Value::Undefined,
            }));
        }
        if let Some(value) = self.stylesheet_get(node, property)? {
            return Ok(Some(value));
        }
        if tag(self.doc, node) == "script" && property == "nonce" {
            let nonce = self.doc.attr(node, "nonce").unwrap_or("").to_owned();
            return self.string(&nonce).map(Some);
        }
        if tag(self.doc, node) == "script" && matches!(property, "text" | "textContent") {
            return self
                .dom_script_text(node)
                .map(|text| Some(Value::String(text)));
        }
        if self.dom_script(node) {
            if let Some(name) = script_attribute(property) {
                return Ok(Some(if boolean_script_attribute(name) {
                    Value::Bool(
                        self.doc.attr(node, name).is_some()
                            || name == "async" && self.state.dom.force_async.contains(&node),
                    )
                } else if name == "src" {
                    self.image_source(node)?
                } else {
                    let value = self.doc.attr(node, name).unwrap_or("").to_owned();
                    self.string(&value)?
                }));
            }
        }
        Ok(None)
    }

    fn create_dom_node(&mut self, kind: NodeKind) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        if self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes) >= self.state.node_limit {
            return Err("DOM appended-node limit reached (256 per document).".into());
        }
        let bytes = match &kind {
            NodeKind::Element { tag, .. } => tag.len(),
            NodeKind::Text(text) => text.len(),
        };
        self.state.replace_bytes(0, bytes)?;
        let node = self.doc.nodes.len();
        if matches!(&kind, NodeKind::Element { tag, .. } if tag == "script") {
            self.state.dom.force_async.insert(node);
        }
        self.doc.nodes.push(Node {
            kind,
            children: Vec::new(),
        });
        self.state.dom.created.insert(node);
        self.state.detached_nodes.insert(node);
        self.state.tree.active.push(false);
        self.state.tree.parents.push(None);
        self.state.dom.connected.push(false);
        Ok(Value::Host(node_handle(node)))
    }

    // No mutation may invalidate a loaded resource, native form value, form
    // ownership, button caption or disabled fieldset's first-legend relation.
    fn mutation_ancestors(&mut self, node: usize) -> Result<(), String> {
        let mut current = Some(node);
        while let Some(index) = current {
            self.state.work(1)?;
            if self.forms.controls.contains_key(&index)
                || tag(self.doc, index) == "script" && !self.dom_script(index)
                || matches!(
                    tag(self.doc, index),
                    "title"
                        | "style"
                        | "template"
                        | "iframe"
                        | "object"
                        | "select"
                        | "option"
                        | "optgroup"
                        | "textarea"
                )
                || tag(self.doc, index) == "noscript" && self.doc.scripting_enabled
            {
                return Err("Changing native control contents or inert resource subtrees is not implemented.".into());
            }
            current = self.state.tree.parents[index];
        }
        Ok(())
    }

    fn mutation_subtree(&mut self, root: usize) -> Result<(Vec<usize>, usize), String> {
        let mut nodes = Vec::new();
        let mut stack = vec![(root, 1usize)];
        let mut depth = 0;
        while let Some((node, level)) = stack.pop() {
            self.state.work(1)?;
            depth = depth.max(level);
            let name = tag(self.doc, node);
            if forms::kind(self.doc, node).is_some()
                || matches!(
                    name,
                    "html"
                        | "head"
                        | "body"
                        | "form"
                        | "fieldset"
                        | "legend"
                        | "option"
                        | "optgroup"
                        | "img"
                        | "style"
                        | "title"
                        | "object"
                        | "embed"
                        | "base"
                        | "meta"
                        | "template"
                )
                || name == "noscript" && self.doc.scripting_enabled
                || name == "script" && !self.dom_script(node)
                || name == "link" && !self.dom_stylesheet(node)
            {
                let frame = if name == "iframe" {
                    let source = match self.doc.attr(node, "src").map(str::trim) {
                        None => "absent",
                        Some("") => "empty",
                        Some("about:blank") => "blank",
                        Some(_) => "other",
                    };
                    format!(" Frame source={source}; srcdoc={}; sandbox={}; hidden={}; inline-style={}.",
                        self.doc.attr(node, "srcdoc").is_some(), self.doc.attr(node, "sandbox").is_some(),
                        self.doc.attr(node, "hidden").is_some(), self.doc.attr(node, "style").is_some())
                } else { String::new() };
                return Err(format!("DOM tree mutation of controls, loaded resources or structural roots is not implemented. Blocked node={node}; tag={:?}; connected={}; created={}.{frame}",
                    prefix(name, 64), self.state.dom.connected.get(node).copied().unwrap_or(false), self.state.dom.created.contains(&node)));
            }
            if name == "iframe" { self.validate_blank_frame(node)?; }
            if let Some(id) = self.doc.attr(node, "id").filter(|id| !id.is_empty()) {
                self.state.work(self.forms.controls.len())?;
                self.state.copies(
                    self.forms
                        .controls
                        .keys()
                        .map(|&control| self.doc.attr(control, "form").map_or(0, str::len))
                        .sum(),
                )?;
                if self
                    .forms
                    .controls
                    .keys()
                    .any(|&control| self.doc.attr(control, "form") == Some(id))
                {
                    return Err(
                        "Moving an id used by an explicit form association is not implemented."
                            .into(),
                    );
                }
            }
            nodes.push(node);
            stack.extend(
                self.doc.nodes[node]
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, level + 1)),
            );
        }
        Ok((nodes, depth))
    }

    fn mutate_dom_tree(
        &mut self,
        parent: usize,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if !matches!(self.doc.nodes[parent].kind, NodeKind::Element { .. }) {
            return Err("HierarchyRequestError: Text nodes cannot contain children.".into());
        }
        let Some(Value::Host(handle)) = args.first() else {
            return Err("A DOM node is required.".into());
        };
        let child = self.dom_node(*handle)?;
        let reference = if method == "insertBefore" {
            match args.get(1) {
                Some(Value::Host(handle)) => Some(self.dom_node(*handle)?),
                Some(Value::Null | Value::Undefined) => None,
                _ => return Err("insertBefore requires a node or null reference.".into()),
            }
        } else {
            None
        };
        if reference.is_some_and(|reference| self.state.tree.parents[reference] != Some(parent))
            || method == "removeChild" && self.state.tree.parents[child] != Some(parent)
        {
            return Err("NotFoundError: The reference node is not a child of this parent.".into());
        }
        let mut ancestor = Some(parent);
        let mut parent_depth = 0usize;
        while let Some(index) = ancestor {
            self.state.work(1)?;
            if index == child && method != "removeChild" {
                return Err("HierarchyRequestError: Insertion would create a DOM cycle.".into());
            }
            parent_depth += 1;
            ancestor = self.state.tree.parents[index];
        }
        if reference == Some(child) {
            return Ok(Value::Host(node_handle(child)));
        }
        self.mutation_ancestors(parent)?;
        let old_parent = self.state.tree.parents[child];
        if let Some(old_parent) = old_parent {
            self.mutation_ancestors(old_parent)?;
        }
        let (descendants, depth) = self.mutation_subtree(child).map_err(|error| format!(
            "{error} Operation={method}; parent={parent}<{:?}>; child={child}<{:?}>; old_parent={old_parent:?}; reference={reference:?}.",
            prefix(tag(self.doc, parent), 64), prefix(tag(self.doc, child), 64)))?;
        if method != "removeChild" && parent_depth.saturating_add(depth) > MAX_DOM_DEPTH {
            return Err("DOM tree depth limit reached (128).".into());
        }
        if self.dom_script(parent) && !matches!(self.doc.nodes[child].kind, NodeKind::Text(_)) {
            return Err("Created scripts accept text children only in this subset.".into());
        }
        self.state.work(
            self.doc.nodes[parent]
                .children
                .len()
                .saturating_add(old_parent.map_or(0, |node| self.doc.nodes[node].children.len()))
                .saturating_add(descendants.len()),
        )?;
        let was_connected = self.state.dom.connected[child];
        let connected = method != "removeChild" && self.state.dom.connected[parent];
        let frames = self.prepare_blank_frames(&descendants, connected, parent)?;
        if let Some(old_parent) = old_parent {
            self.doc.nodes[old_parent]
                .children
                .retain(|&node| node != child);
        }
        if method != "removeChild" {
            let position = reference
                .and_then(|reference| {
                    self.doc.nodes[parent]
                        .children
                        .iter()
                        .position(|&node| node == reference)
                })
                .unwrap_or(self.doc.nodes[parent].children.len());
            self.doc.nodes[parent].children.insert(position, child);
        }
        self.state.tree.parents[child] = if method == "removeChild" {
            None
        } else {
            Some(parent)
        };
        for &node in &descendants {
            self.state.dom.connected[node] = connected;
            if connected {
                self.state.detached_nodes.remove(&node);
            } else {
                self.state.detached_nodes.insert(node);
            }
        }
        self.commit_blank_frames(&descendants, frames);
        self.state.dirty = true;
        if was_connected || connected {
            self.state.layout_changed();
        }
        for &node in &descendants {
            if self.dom_stylesheet(node) {
                self.queue_dynamic_stylesheet(node)?;
            }
        }
        if connected {
            for node in descendants {
                if self.dom_script(node) {
                    self.queue_dynamic_script(node)?;
                }
            }
            if self.dom_script(parent) {
                self.queue_dynamic_script(parent)?;
            }
        }
        Ok(Value::Host(node_handle(child)))
    }

    fn dom_script_text(&mut self, node: usize) -> Result<String, String> {
        let mut text = String::new();
        for &child in &self.doc.nodes[node].children {
            self.state.work(1)?;
            if let NodeKind::Text(value) = &self.doc.nodes[child].kind {
                if text.len().saturating_add(value.len()) > MAX_STRING {
                    return Err("Script text exceeds 32 KiB.".into());
                }
                self.state.copies(value.len())?;
                text.push_str(value);
            }
        }
        Ok(text)
    }

    fn set_script_text(&mut self, node: usize, value: &Value) -> Result<(), String> {
        let text = if matches!(value, Value::Null) {
            String::new()
        } else {
            self.text(value)?
        };
        self.state.work(self.doc.nodes[node].children.len())?;
        let new_child = if text.is_empty() {
            None
        } else {
            let Value::Host(handle) = self.create_dom_node(NodeKind::Text(text))? else {
                unreachable!();
            };
            node_index(handle)
        };
        for child in self.doc.nodes[node].children.drain(..) {
            self.state.detached_nodes.insert(child);
            self.state.dom.connected[child] = false;
            self.state.tree.parents[child] = None;
        }
        if let Some(child) = new_child {
            self.doc.nodes[node].children.push(child);
            self.state.tree.parents[child] = Some(node);
            self.state.dom.connected[child] = self.state.dom.connected[node];
            if self.state.dom.connected[node] {
                self.state.detached_nodes.remove(&child);
            }
        }
        self.state.dirty = true;
        if self.state.dom.connected[node] {
            self.queue_dynamic_script(node)?;
        }
        Ok(())
    }

    pub(super) fn dom_set(
        &mut self,
        object: u32,
        property: &str,
        value: &Value,
    ) -> Result<bool, String> {
        if node_index(object).is_none() {
            return Ok(false);
        }
        let node = self.dom_node(object)?;
        if let NodeKind::Text(old) = &self.doc.nodes[node].kind {
            if !matches!(property, "data" | "nodeValue" | "textContent") {
                return Ok(false);
            }
            let old_len = old.len();
            self.mutation_ancestors(node)?;
            let text = if matches!(value, Value::Null) && property != "data" {
                String::new()
            } else {
                self.text(value)?
            };
            self.state.replace_bytes(old_len, text.len())?;
            self.doc.nodes[node].kind = NodeKind::Text(text);
            if self.state.dom.connected[node] {
                self.state.layout_changed();
                if let Some(parent) =
                    self.state.tree.parents[node].filter(|&parent| self.dom_script(parent))
                {
                    self.queue_dynamic_script(parent)?;
                }
            }
            return Ok(true);
        }
        if self.stylesheet_set(node, property, value)? {
            return Ok(true);
        }
        if self.dom_script(node) {
            if matches!(property, "text" | "textContent") {
                self.set_script_text(node, value)?;
                return Ok(true);
            }
            if let Some(name) = script_attribute(property) {
                let text = if boolean_script_attribute(name) {
                    value.truthy().then(String::new)
                } else {
                    Some(self.text(value)?)
                };
                self.attribute(node, name, text)?;
                if name == "async" {
                    self.state.dom.force_async.remove(&node);
                }
                if name == "src" && self.state.dom.connected[node] {
                    self.queue_dynamic_script(node)?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn dom_call(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Option<Value>, String> {
        if object == DOCUMENT && matches!(method, "createElement" | "createTextNode") {
            let text = self.text(args.first().ok_or("DOM creation requires one argument.")?)?;
            if method == "createTextNode" {
                return self.create_dom_node(NodeKind::Text(text)).map(Some);
            }
            if text.eq_ignore_ascii_case("img") {
                return self.create_image(&[]).map(Some);
            }
            return self
                .create_dom_node(NodeKind::Element {
                    tag: text.to_ascii_lowercase(),
                    id: None,
                    href: None,
                    attrs: Vec::new(),
                })
                .map(Some);
        }
        if node_index(object).is_none() {
            return Ok(None);
        }
        if matches!(method, "appendChild" | "insertBefore" | "removeChild") {
            let node = self.dom_node(object)?;
            return self.mutate_dom_tree(node, method, args).map(Some);
        }
        if method == "focus" {
            let node = self.node(object)?;
            if self.state.tree.active[node]
                && self
                    .forms
                    .controls
                    .get(&node)
                    .is_some_and(|control| control.focusable())
            {
                self.state.dom.pending_focus = Some(node);
            }
            return Ok(Some(Value::Undefined));
        }
        if matches!(method, "setAttribute" | "removeAttribute") {
            let node = self.node(object)?;
            if self.stylesheet_attribute_call(node, method, args)? {
                return Ok(Some(Value::Undefined));
            }
            if self.dom_script(node) {
                let name = self
                    .text(args.first().ok_or("Attribute name is required.")?)?
                    .to_ascii_lowercase();
                if matches!(
                    name.as_str(),
                    "src"
                        | "type"
                        | "charset"
                        | "nonce"
                        | "integrity"
                        | "crossorigin"
                        | "referrerpolicy"
                        | "fetchpriority"
                        | "async"
                        | "defer"
                        | "nomodule"
                ) {
                    let value = if method == "removeAttribute" {
                        None
                    } else {
                        Some(self.text(args.get(1).ok_or("Attribute value is required.")?)?)
                    };
                    self.attribute(node, &name, value)?;
                    if name == "src" && self.state.dom.connected[node] {
                        self.queue_dynamic_script(node)?;
                    }
                    return Ok(Some(Value::Undefined));
                }
            }
        }
        Ok(None)
    }

    pub(super) fn named_document_property(&mut self, name: &str) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        let mut found = None;
        if name.is_empty() {
            return Ok(Value::Undefined);
        }
        for position in 0..self.state.tree.order.len() {
            let node = self.state.tree.order[position];
            self.state.work(1)?;
            if tag(self.doc, node) != "form" {
                continue;
            }
            let candidate = self.doc.attr(node, "name");
            self.state.copies(candidate.map_or(0, str::len))?;
            if candidate == Some(name) {
                if found.is_some() {
                    return Err("Multiple named forms require a live HTMLCollection, which is not implemented.".into());
                }
                found = Some(node);
            }
        }
        Ok(found.map_or(Value::Undefined, |node| Value::Host(node_handle(node))))
    }

    pub(super) fn named_form_property(&mut self, form: usize, name: &str) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        if name.is_empty() {
            return Ok(Value::Undefined);
        }
        let mut found = None;
        for position in 0..self.state.tree.order.len() {
            let node = self.state.tree.order[position];
            self.state.work(1)?;
            let Some(control) = self.forms.controls.get(&node) else {
                continue;
            };
            if control.owner != Some(form) || control.kind == Kind::Image {
                continue;
            }
            let id = self.doc.attr(node, "id");
            let candidate = self.doc.attr(node, "name");
            self.state.copies(
                id.map_or(0, str::len)
                    .saturating_add(candidate.map_or(0, str::len)),
            )?;
            if id == Some(name) || candidate == Some(name) {
                if found.is_some() {
                    return Err(
                        "Multiple named controls require RadioNodeList, which is not implemented."
                            .into(),
                    );
                }
                found = Some(node);
            }
        }
        Ok(found.map_or(Value::Undefined, |node| Value::Host(node_handle(node))))
    }
}
