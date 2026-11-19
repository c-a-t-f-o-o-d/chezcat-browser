//! Detached copies of ordinary nodes in the main document arena.
//! https://dom.spec.whatwg.org/#concept-node-clone
use super::*;

pub(super) const CLONE_METHOD: &str = "\0Node.cloneNode";

fn ordinary_clone_element(name: &str) -> bool {
    (text_container(name) && !matches!(name, "button" | "legend"))
        || matches!(name, "nav" | "main" | "section" | "article" | "aside" | "header"
            | "footer" | "address" | "figure" | "figcaption" | "ol" | "ul" | "dl"
            | "table" | "caption" | "col" | "colgroup" | "thead" | "tbody" | "tfoot"
            | "tr" | "td" | "th" | "hr" | "time")
}

struct DetachedCopy {
    node: Node,
    parent: Option<usize>,
    unsafe_nonce: bool,
}

impl Bridge<'_> {
    pub(super) fn validate_clone_receiver(&self, object: u32) -> Result<(), (&'static str, String)> {
        if !self.root_node_receiver(object) {
            return Err(("TypeError", "cloneNode requires a genuine accessible Node.".into()));
        }
        if !node_index(object).is_some_and(|node| self.dom_accessible(node)) {
            return Err(("NotSupportedError", "Cloning this Node interface or child document is unavailable.".into()));
        }
        Ok(())
    }

    pub(super) fn clone_node(&mut self, object: u32, deep: bool) -> Result<Value, String> {
        self.validate_clone_receiver(object).map_err(|(_, message)| message)?;
        let root = self.dom_node(object)?;
        let base = self.doc.nodes.len();
        let remaining = self.state.node_limit.saturating_sub(base.saturating_add(self.state.blank_frame_nodes))
            .min(((STYLE_BASE - NODE_BASE) as usize).saturating_sub(base));
        if remaining == 0 {
            return Err("DOM appended-node limit reached during cloning.".into());
        }

        // Each parent's direct children occupy a contiguous range in the plan.
        // All checks and copies finish before the first new handle is published.
        let mut plan = vec![(root, None, 1usize)];
        let mut staged = Vec::new();
        let mut textareas = Vec::new();
        let mut radios = Vec::new();
        let mut bytes = 0usize;
        let mut nonce_count = 0usize;
        while staged.len() < plan.len() {
            let offset = staged.len();
            let (source, parent, depth) = plan[offset];
            let item = &self.doc.nodes[source];
            let children = if deep { item.children.as_slice() } else { &[] };
            self.state.work(1 + children.len())?;
            if children.len() > remaining - plan.len() {
                return Err("DOM appended-node limit reached during cloning.".into());
            }
            if !children.is_empty() && depth >= dom_tree::MAX_DOM_DEPTH {
                return Err("DOM depth limit reached during cloning.".into());
            }
            let first = base + plan.len();
            plan.try_reserve(children.len()).map_err(|_| "DOM clone plan reservation failed.")?;
            plan.extend(children.iter().map(|&child| (child, Some(base + offset), depth + 1)));

            let mut metadata_bytes = 0;
            if let NodeKind::Element { tag, href, attrs, .. } = &item.kind {
                self.state.work(attrs.len())?;
                let textarea = tag == "textarea" && self.owned_native_textarea(source);
                let radio = tag == "input" && self.owned_native_radio(source);
                if item.namespace != crate::html::Namespace::Html || !(ordinary_clone_element(tag) || textarea || radio)
                    || attrs.iter().any(|attr| attr.name.namespace.is_none()
                        && (attr.name.qualified.starts_with("on") || attr.name.html_is("is"))) {
                    return Err("Cloning this element or its native state is not implemented.".into());
                }
                if textarea || radio {
                    if self.forms.controls.len().saturating_add(textareas.len()).saturating_add(radios.len()) >= forms::MAX_CONTROLS {
                        return Err("Native form control limit reached before native control cloning.".into());
                    }
                    let controls=if textarea {&mut textareas} else {&mut radios};
                    controls.try_reserve(1).map_err(|_| "Native control clone plan reservation failed.")?;
                    controls.push((source, base + offset));
                }
                metadata_bytes = tag.len() + href.as_ref().map_or(0, String::len);
            }
            let size = node_bytes(item) + metadata_bytes;
            bytes = bytes.checked_add(size).ok_or("DOM clone size overflow.")?;
            if bytes > self.state.retained_limit.saturating_sub(self.state.retained) {
                return Err("DOM retained text/attribute growth limit reached during cloning.".into());
            }
            self.state.copies(size)?;
            let kind = match &item.kind {
                NodeKind::Element { tag, id, href, attrs } => NodeKind::Element {
                    tag: tag.clone(), id: id.clone(), href: href.clone(),
                    attrs: attrs.iter().map(|attr| crate::html::Attribute {
                        name: attr.name.copy(), value: attr.value.clone(),
                    }).collect(),
                },
                NodeKind::Text(text) => NodeKind::Text(text.clone()),
                NodeKind::Comment(text) => NodeKind::Comment(text.clone()),
                NodeKind::ProcessingInstruction { target, data } => NodeKind::ProcessingInstruction {
                    target: target.clone(), data: data.clone(),
                },
                NodeKind::DocumentFragment => NodeKind::DocumentFragment,
            };
            let unsafe_nonce = self.doc.nonce_unsafe_nodes.contains(&source);
            nonce_count += usize::from(unsafe_nonce);
            staged.push(DetachedCopy {
                node: Node { kind, namespace: item.namespace, children: (first..first + children.len()).collect() },
                parent,
                unsafe_nonce,
            });
        }

        let retained_limit = self.state.retained_limit.saturating_sub(self.state.retained)
            .saturating_sub(bytes);
        let mut form_plan = if textareas.is_empty() { None } else {
            let state = &mut *self.state;
            Some(self.forms.prepare_textarea_clones(self.doc, &textareas, deep, retained_limit, |work, copies| {
                state.work(work)?;
                state.copies(copies)
            })?)
        };
        if !radios.is_empty() {
            let mut plan=form_plan.unwrap_or_else(||forms::SelectFormPlan {
                additions:std::collections::BTreeMap::new(),metadata:Vec::new(),radio_checked:Vec::new(),retained_bytes:0,
            });
            let state=&mut *self.state;
            self.forms.add_radio_clones(self.doc,&radios,base,&mut plan,retained_limit,|work,copies| {
                state.work(work)?;state.copies(copies)
            })?;
            form_plan=Some(plan);
        }
        let bytes = bytes.checked_add(form_plan.as_ref().map_or(0, |plan| plan.retained_bytes))
            .ok_or("DOM/native control clone size overflow.")?;
        let count = staged.len();
        self.doc.nodes.try_reserve(count).map_err(|_| "DOM clone node reservation failed.")?;
        self.doc.nonce_unsafe_nodes.try_reserve(nonce_count).map_err(|_| "DOM clone nonce reservation failed.")?;
        self.state.dom.created.try_reserve(count).map_err(|_| "DOM clone identity reservation failed.")?;
        self.state.detached_nodes.try_reserve(count).map_err(|_| "DOM clone detached reservation failed.")?;
        self.state.tree.parents.try_reserve(count).map_err(|_| "DOM clone parent reservation failed.")?;
        self.state.tree.active.try_reserve(count).map_err(|_| "DOM clone active-state reservation failed.")?;
        self.state.dom.connected.try_reserve(count).map_err(|_| "DOM clone connection reservation failed.")?;
        self.state.replace_bytes(0, bytes)?;
        for copy in staged {
            let node = self.doc.nodes.len();
            self.doc.nodes.push(copy.node);
            self.state.dom.created.insert(node);
            self.state.detached_nodes.insert(node);
            self.state.tree.parents.push(copy.parent);
            self.state.tree.active.push(false);
            self.state.dom.connected.push(false);
            if copy.unsafe_nonce { self.doc.nonce_unsafe_nodes.insert(node); }
        }
        if let Some(plan) = form_plan { plan.commit(self.forms); }
        Ok(Value::Host(node_handle(base)))
    }
}

#[cfg(test)]
#[path = "script_radio_clone_tests.rs"]
mod radio_clone_tests;
