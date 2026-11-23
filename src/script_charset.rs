//! Exact legacy charset metadata does not grant resource-loading authority.
use super::*;

#[cfg(test)]
#[path = "script_charset_interface_tests.rs"]
mod interface_tests;

#[cfg(test)]
#[path = "script_charset_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "script_parsed_charset_tests.rs"]
mod parsed_tests;

impl Bridge<'_> {
    pub(super) fn charset_interface(&self, object: u32) -> Option<&'static str> {
        if self.inert_node_type(object).is_some() {
            return self.inert_interface(object).filter(|name| matches!(*name, "HTMLScriptElement" | "HTMLLinkElement"));
        }
        if self.is_frame_handle(object) {
            return self.frame_concrete_html_interface(object).filter(|name| matches!(*name, "HTMLScriptElement" | "HTMLLinkElement"));
        }
        let node = node_index(object)?;
        if !self.attr_element_receiver(object) || !self.resource_charset_element(node) { return None; }
        Some(if tag(self.doc, node) == "script" { "HTMLScriptElement" } else { "HTMLLinkElement" })
    }

    pub(super) fn charset_member(&self, object: u32, property: &str) -> bool {
        property == "charset" && self.inert_node_type(object).is_none()
            && !self.is_frame_handle(object) && self.charset_interface(object).is_some()
    }

    pub(super) fn validate_charset(&self, object: u32, interface: &str) -> Result<(), (&'static str, String)> {
        if self.charset_interface(object) != Some(interface) {
            return Err(("TypeError", format!("Charset accessor requires a genuine {interface}.")));
        }
        self.native_node_access(object)?;
        if self.inert_node_type(object).is_some() || self.is_frame_handle(object) {
            return Err(("NotSupportedError", "Charset accessors require the main document owner in this reflection subset.".into()));
        }
        Ok(())
    }

    pub(super) fn charset_reflection_get(&mut self, object: u32) -> Result<Value, String> {
        let interface = self.charset_interface(object).ok_or("Charset getter requires a genuine resource element.")?;
        self.validate_charset(object, interface).map_err(|(_, message)| message)?;
        let node = self.dom_node(object)?;
        self.resource_charset_get(node)
    }

    pub(super) fn charset_reflection_set(&mut self, object: u32, value: &Value) -> Result<(), String> {
        let interface = self.charset_interface(object).ok_or("Charset setter requires a genuine resource element.")?;
        self.validate_charset(object, interface).map_err(|(_, message)| message)?;
        let node = self.dom_node(object)?;
        self.resource_charset_set(node, value)
    }

    pub(super) fn resource_charset_element(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|entry|
            entry.namespace == crate::html::Namespace::Html
                && matches!(&entry.kind, NodeKind::Element {tag,..} if matches!(tag.as_str(), "link" | "script")))
    }

    pub(super) fn resource_charset_attribute(&self, node: usize, name: &crate::html::AttributeName) -> bool {
        self.resource_charset_element(node) && name.html_is("charset")
    }

    pub(super) fn resource_charset_get(&mut self, node: usize) -> Result<Value, String> {
        let NodeKind::Element {attrs,..} = &self.doc.nodes[node].kind else {unreachable!()};
        self.state.work(attrs.len())?;
        match self.doc.dom_attr(node, "charset") {
            Some(value) => Self::dom_string_value(self.state, value),
            None => self.string(""),
        }
    }

    pub(super) fn resource_charset_set(&mut self, node: usize, value: &Value) -> Result<(), String> {
        let value = self.dom_text(value)?;
        self.state.copies("charset".len())?;
        let name = crate::html::AttributeName::unqualified(script_clone_node::copy_text("charset")?);
        self.public_dom_attribute(node, name, Some(value), script_attr::InputPreparation::Pending)
    }
}
