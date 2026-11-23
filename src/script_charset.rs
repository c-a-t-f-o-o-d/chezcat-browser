//! Exact legacy charset metadata does not grant resource-loading authority.
use super::*;

#[cfg(test)]
#[path = "script_charset_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "script_parsed_charset_tests.rs"]
mod parsed_tests;

impl Bridge<'_> {
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
