//! Whole-content records are admitted before native replacement commits.
use super::*;
use crate::javascript::NativeChildListChange;

impl Bridge<'_> {
    pub(super) fn content_observation_target(
        &mut self,
        object: u32,
        property: &str,
    ) -> Result<Option<u32>, String> {
        if !matches!(property, "innerHTML" | "textContent") {
            return Ok(None);
        }
        if property == "innerHTML" && self.template_receiver(object) {
            return self.existing_template_content(object);
        }
        if let Some(kind) = self.inert_node_type(object) {
            return Ok((property == "textContent"
                && matches!(kind, 1 | 11)
                && !self.template_receiver(object))
            .then_some(object));
        }
        self.state.rebuild(self.doc)?;
        let Some(node) = node_index(object).filter(|&node| self.dom_accessible(node)) else {
            return Ok(None);
        };
        let item = &self.doc.nodes[node];
        let supported = if property == "innerHTML" {
            item.namespace == crate::html::Namespace::Html
                && matches!(item.kind, NodeKind::Element { .. })
                && !matches!(tag(self.doc, node), "script" | "style")
        } else {
            text_container(tag(self.doc, node))
                || self.created_html_style(node) && self.state.detached_nodes.contains(&node)
                || matches!(item.kind, NodeKind::DocumentFragment)
        };
        Ok(supported.then_some(object))
    }
    pub(super) fn content_observation_changes(
        &mut self,
        object: u32,
        property: &str,
        value: &Value,
    ) -> Result<Vec<NativeChildListChange>, String> {
        let Some(target) = self.content_observation_target(object, property)? else {
            return Ok(Vec::new());
        };
        let text = if matches!(value, Value::Null | Value::Undefined) {
            String::new()
        } else {
            self.text(value)?
        };
        let mut added = Vec::new();
        if property == "innerHTML" {
            if text.len() > self.state.limits.host.string_bytes {
                return Err("HTML observation source exceeds the host allowance.".into());
            }
            self.state.work(text.len())?;
            self.state.copies(text.len().saturating_mul(2))?;
            let template = self.template_receiver(object);
            let context = if template {
                "template"
            } else {
                tag(self.doc, node_index(object).unwrap())
            };
            let parsed = crate::html::parse_fragment(&text, context, self.doc.scripting_enabled)?;
            self.state.work(parsed.nodes.len())?;
            added
                .try_reserve_exact(parsed.nodes[0].children.len())
                .map_err(|_| "Content observer root reservation failed.")?;
            self.state.copies(
                parsed.nodes[0]
                    .children
                    .len()
                    .saturating_mul(size_of::<u32>()),
            )?;
            if template {
                let (handles, _) = self.template_replacement_handles(target, &parsed)?;
                added.extend(parsed.nodes[0].children.iter().map(|&node| handles[node]));
            } else {
                let base = self.doc.nodes.len();
                added.extend(
                    parsed.nodes[0]
                        .children
                        .iter()
                        .map(|&node| node_handle(base + node - 1)),
                );
            }
        } else if !text.is_empty() {
            added
                .try_reserve_exact(1)
                .map_err(|_| "Content observer text reservation failed.")?;
            self.state.copies(size_of::<u32>())?;
            added.push(if self.inert_node_type(target).is_some() {
                self.next_inert_text_identity()?
            } else {
                node_handle(self.doc.nodes.len())
            });
        }
        let count = if let Some(children) = self.inert_children(target) {
            children.len()
        } else {
            self.doc.nodes[node_index(target).ok_or("Content observer receiver is unavailable.")?]
                .children
                .len()
        };
        if count == 0 && added.is_empty() {
            return Ok(Vec::new());
        }
        self.state.work(count)?;
        self.state.copies(count.saturating_mul(size_of::<u32>()))?;
        let mut removed = Vec::new();
        removed
            .try_reserve_exact(count)
            .map_err(|_| "Content observer old-child reservation failed.")?;
        if let Some(children) = self.inert_children(target) {
            removed.extend_from_slice(children);
        } else {
            removed.extend(
                self.doc.nodes[node_index(target).unwrap()]
                    .children
                    .iter()
                    .copied()
                    .map(node_handle),
            );
        }
        let mut changes = Vec::new();
        changes
            .try_reserve_exact(1)
            .map_err(|_| "Content observer record reservation failed.")?;
        changes.push(NativeChildListChange {
            target,
            added,
            removed,
            previous: None,
            next: None,
        });
        Ok(changes)
    }
}
