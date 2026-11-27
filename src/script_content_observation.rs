//! Whole-content records are admitted before native replacement commits.
use super::*;
use crate::javascript::NativeChildListChange;

fn future_main_identity(index: usize) -> Result<u32, String> {
    if index >= (STYLE_BASE - NODE_BASE) as usize {
        return Err("Content replacement exceeds native Node identity space.".into());
    }
    Ok(node_handle(index))
}
impl Bridge<'_> {
    pub(super) fn content_observation_target(
        &mut self,
        object: u32,
        property: &str,
    ) -> Result<Option<u32>, String> {
        if object==DOCUMENT && property=="title" {
            self.state.rebuild(self.doc)?;
            return Ok(self.state.tree.title.map(node_handle));
        }
        if property=="text" && (self.title_receiver(object)||self.option_text_receiver(object)) { return Ok(Some(object)); }
        if self.textarea_default_value_member(object,property){self.validate_textarea_default_value(object).map_err(|(_,s)|s)?;return Ok(Some(object));}
        if self.output_value_member(object,property) {
            self.validate_output_value(object).map_err(|(_,message)|message)?;
            self.state.work(self.state.output_defaults.lookup_work())?;
            return Ok((property=="value"||!self.state.output_defaults.contains_key(&object)).then_some(object));
        }
        if matches!(property, "text" | "textContent")
            && node_index(object).is_some_and(|node| self.dom_accessible(node) && self.dom_script(node)) {
            return Ok(Some(object));
        }
        if !matches!(property, "innerHTML" | "textContent") {
            return Ok(None);
        }
        if property == "innerHTML" && self.template_receiver(object) {
            return self.existing_template_content(object);
        }
        if self.inert_node_type(object).is_some() {
            return Ok((property == "textContent"
                && self.inert_text_replacement_receiver(object))
            .then_some(object));
        }
        self.state.rebuild(self.doc)?;
        let Some(node) = node_index(object).filter(|&node| self.dom_accessible(node)) else {
            return Ok(None);
        };
        let passive_noscript = property == "textContent" && self.movable_noscript_host(node)?;
        let item = &self.doc.nodes[node];
        let supported = if property == "innerHTML" {
            item.namespace == crate::html::Namespace::Html
                && matches!(item.kind, NodeKind::Element { .. })
                && !matches!(tag(self.doc, node), "script" | "style")
        } else {
            self.text_content_container(node) || passive_noscript || self.owned_native_textarea(node) || self.title_receiver(object)
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
        let mut added = Vec::new();
        if property == "innerHTML" {
            let template=self.template_receiver(object);
            let parsed=if let Value::StringUnits(units)=value{
                if template{return Err("Exact UTF-16 template markup replacement is not implemented.".into());}
                self.parse_inner_html_units(node_index(object).ok_or("Content observer native receiver is unavailable.")?,units)?
            }else{
                let text=if matches!(value,Value::Null|Value::Undefined) {String::new()} else {self.text(value)?};
                if text.len() > self.state.limits.host.string_bytes {
                    return Err("HTML observation source exceeds the host allowance.".into());
                }
                self.state.work(text.len())?;
                self.state.copies(text.len().saturating_mul(2))?;
                let context = if template {
                    "template"
                } else {
                    tag(self.doc, node_index(object).unwrap())
            };
            crate::html::parse_fragment(&text, context, self.doc.scripting_enabled)?
            };
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
                for &node in &parsed.nodes[0].children {
                    added.push(future_main_identity(
                        base.checked_add(node)
                            .and_then(|n| n.checked_sub(1))
                            .ok_or("Content identity projection overflow.")?,
                    )?);
                }
            }
        } else if match value {
            Value::String(text)=>!text.is_empty(),Value::StringUnits(units)=>!units.is_empty(),
            Value::Undefined if node_index(target).is_some_and(|node| self.dom_script(node))=>true,
            Value::Null|Value::Undefined=>false,_=>!self.text(value)?.is_empty(),
        } {
            added
                .try_reserve_exact(1)
                .map_err(|_| "Content observer text reservation failed.")?;
            self.state.copies(size_of::<u32>())?;
            added.push(if self.inert_node_type(target).is_some() {
                self.next_inert_text_identity()?
            } else {
                future_main_identity(self.doc.nodes.len())?
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
