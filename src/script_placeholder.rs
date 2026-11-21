//! Genuine input/textarea hint reflection, independent of editing/default data.
//! https://html.spec.whatwg.org/multipage/form-elements.html#attr-textarea-placeholder
use super::*;

pub(super) fn operation(method: &str) -> Option<(&'static str, bool)> {
    match method {
        "\0HTMLInputElement.get.placeholder" => Some(("HTMLInputElement", false)),
        "\0HTMLInputElement.set.placeholder" => Some(("HTMLInputElement", true)),
        "\0HTMLTextAreaElement.get.placeholder" => Some(("HTMLTextAreaElement", false)),
        "\0HTMLTextAreaElement.set.placeholder" => Some(("HTMLTextAreaElement", true)),
        _ => None,
    }
}
impl Bridge<'_> {
    pub(super) fn placeholder_interface(&self, object: u32) -> Option<&'static str> {
        if let Some(interface) = self.inert_interface(object) {
            return matches!(interface, "HTMLInputElement" | "HTMLTextAreaElement")
                .then_some(interface);
        }
        let node = node_index(object).filter(|&node| self.dom_accessible(node))?;
        let item = self.doc.nodes.get(node)?;
        if item.namespace != crate::html::Namespace::Html {
            return None;
        }
        match tag(self.doc, node) {
            "input" => Some("HTMLInputElement"),
            "textarea" => Some("HTMLTextAreaElement"),
            _ => None,
        }
    }
    pub(super) fn placeholder_member(&self, object: u32, property: &str) -> bool {
        property == "placeholder" && self.placeholder_interface(object).is_some()
    }
    pub(super) fn placeholder_get(&mut self, object: u32) -> Result<Value, String> {
        if self.placeholder_interface(object).is_none() {
            return Err("Placeholder requires a genuine supported input or textarea.".into());
        }
        if self.inert_node_type(object).is_some() {
            let value =
                self.inert_element_attribute(object, &Value::String("placeholder".into()))?;
            return Ok(if matches!(value, Value::Null) {
                Value::String(String::new())
            } else {
                value
            });
        }
        let node = self.attr_element_node(object)?;
        let text = self.doc.attr(node, "placeholder").unwrap_or("");
        if text.len() > self.state.limits.host.string_bytes {
            return Err("Placeholder exceeds the host string allowance.".into());
        }
        self.state.copies(text.len())?;
        Ok(Value::String(script_clone_node::copy_text(
            self.doc.attr(node, "placeholder").unwrap_or(""),
        )?))
    }
    pub(super) fn placeholder_set(&mut self, object: u32, value: &Value) -> Result<(), String> {
        if self.placeholder_interface(object).is_none() {
            return Err("Placeholder requires a genuine supported input or textarea.".into());
        }
        let text = self.text(value)?;
        if self.inert_node_type(object).is_some() {
            return self
                .set_inert_attribute(
                    object,
                    &[Value::String("placeholder".into()), Value::String(text)],
                )
                .map(|_| ());
        }
        let node = self.attr_element_node(object)?;
        self.state.copies(11)?;
        self.public_attribute(
            node,
            crate::html::AttributeName::unqualified("placeholder".into()),
            Some(text),
        )
    }
}
