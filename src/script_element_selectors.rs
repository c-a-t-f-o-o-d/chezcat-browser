//! Canonical Element operations over the existing bounded selector engine.
//! https://dom.spec.whatwg.org/#interface-parentnode
//! https://webidl.spec.whatwg.org/#es-operations
//! The VM validates the real receiver/origin before required-argument and
//! DOMString conversion. Matching still runs after observable conversion.
use super::*;

pub(super) fn operation(method: &str) -> Option<&'static str> {
    match method {
        "\0Element.querySelector" => Some("querySelector"),
        "\0Element.querySelectorAll" => Some("querySelectorAll"),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn element_selector_receiver(&self, object: u32) -> bool {
        self.frame_element_sibling_receiver(object)
            || node_index(object).filter(|&node| self.dom_accessible(node))
                .is_some_and(|node| matches!(self.doc.nodes[node].kind, NodeKind::Element {..}))
    }

    pub(super) fn element_selector_member(&self, object: u32, property: &str) -> bool {
        matches!(property, "querySelector" | "querySelectorAll")
            && self.element_selector_receiver(object)
            && !self.form_named_control(object, property)
    }

    pub(super) fn validate_element_selector_receiver(&self, object: u32)
        -> Result<(), (&'static str, String)>
    {
        if !self.element_selector_receiver(object) {
            return Err(("TypeError", "Element selector operation requires a genuine accessible Element.".into()));
        }
        // Existing child contexts expose an immutable initial blank tree but
        // have no owner-tagged selector NodeLists. Do not alias child nodes to
        // main-document snapshot indices or fabricate child query results.
        if self.is_frame_handle(object) {
            return Err(("NotSupportedError", "Blank child Element queries are not implemented.".into()));
        }
        Ok(())
    }
}
