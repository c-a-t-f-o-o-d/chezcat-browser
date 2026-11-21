//! HTMLElement.contentEditable reflects the enumerated raw attribute only.
//! Editing hosts, selection, designMode and isContentEditable are not added.
//! https://html.spec.whatwg.org/multipage/interaction.html#contenteditable
use super::*;

pub(super) const GET: &str = "\0HTMLElement.get.contentEditable";
pub(super) const SET: &str = "\0HTMLElement.set.contentEditable";

pub(super) fn accessor(operation: &str) -> bool {
    matches!(operation, GET | SET)
}

fn keyword(text: &str) -> Option<&'static str> {
    ["inherit", "true", "false", "plaintext-only"]
        .into_iter()
        .find(|known| text.eq_ignore_ascii_case(known))
}

fn reflected(raw: Option<&str>) -> &'static str {
    match raw {
        Some("") => "true",
        Some(value) => keyword(value).unwrap_or("inherit"),
        None => "inherit",
    }
}

impl Bridge<'_> {
    pub(super) fn content_editable_receiver(&self, object: u32) -> bool {
        self.class_list_element_receiver(object)
            && node_index(object).is_some_and(|node| self.doc.nodes[node].namespace == crate::html::Namespace::Html)
            || self.frame_concrete_html_interface(object).is_some()
            || self.frame_node_interface(object) == Some("HTMLElement")
    }

    pub(super) fn content_editable_attribute(&self, object: u32, property: &str) -> bool {
        property == "contentEditable" && self.content_editable_receiver(object)
    }

    pub(super) fn content_editable_member(&self, object: u32, property: &str) -> bool {
        self.content_editable_attribute(object, property)
            && !self.form_named_control(object, property)
    }

    pub(super) fn validate_content_editable(
        &self,
        object: u32,
    ) -> Result<(), (&'static str, String)> {
        if self.content_editable_receiver(object) {
            Ok(())
        } else {
            Err((
                "TypeError",
                "contentEditable requires a genuine accessible HTMLElement.".into(),
            ))
        }
    }

    pub(super) fn validate_content_editable_value(
        &self,
        object: u32,
        value: &Value,
    ) -> Result<(), (&'static str, String)> {
        self.validate_content_editable(object)?;
        match value {
            Value::String(value) if keyword(value).is_some() => Ok(()),
            Value::String(_) => Err((
                "SyntaxError",
                "contentEditable must be inherit, true, false, or plaintext-only.".into(),
            )),
            _ => Err((
                "TypeError",
                "contentEditable requires a converted DOMString.".into(),
            )),
        }
    }

    pub(super) fn content_editable_get(&mut self, object: u32) -> Result<Value, String> {
        self.validate_content_editable(object)
            .map_err(|(_, message)| message)?;
        let child = self.frame_contains_node(object);
        let node = if let Some((_, node)) = child {
            self.frame_light_dom_element(object)?;
            node
        } else {
            self.dom_node(object)?
        };
        let attrs = if let Some((owner, _)) = child {
            &self.contains_child_document(owner)?.nodes[node].kind
        } else {
            &self.doc.nodes[node].kind
        };
        let NodeKind::Element { attrs, .. } = attrs else {
            return Err("contentEditable requires an Element.".into());
        };
        let attr_count = attrs.len();
        // Two attribute-list lookups and one bounded state comparison precede
        // the finite canonical result copy. No raw attribute is copied.
        self.state.work(attr_count.saturating_mul(30))?;
        let raw_bytes = if let Some((owner, _)) = child {
            self.contains_child_document(owner)?
                .attr(node, "contenteditable")
                .map_or(0, str::len)
        } else {
            self.doc.attr(node, "contenteditable").map_or(0, str::len)
        };
        self.state.work(raw_bytes)?;
        let value = if let Some((owner, _)) = child {
            reflected(
                self.contains_child_document(owner)?
                    .attr(node, "contenteditable"),
            )
        } else {
            reflected(self.doc.attr(node, "contenteditable"))
        };
        self.string(value)
    }

    pub(super) fn content_editable_set(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<(), String> {
        self.validate_content_editable_value(object, value)
            .map_err(|(_, message)| message)?;
        if self.is_frame_handle(object) {
            self.frame_light_dom_element(object)?;
            return Err("Blank child attribute writes are not implemented.".into());
        }
        let node = self.dom_node(object)?;
        let Value::String(value) = value else {
            unreachable!()
        };
        let canonical = keyword(value).expect("Validated contentEditable keyword");
        let raw = if canonical == "inherit" {
            None
        } else {
            self.state.copies(canonical.len())?;
            Some(canonical.to_owned())
        };
        self.attribute(node, "contenteditable", raw)
    }
}
