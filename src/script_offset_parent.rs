//! HTMLElement offsetParent reads the layout tree captured for this document.
//! https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsetparent
use super::*;

pub(super) const GETTER: &str = "\0HTMLElement.get.offsetParent";

impl Bridge<'_> {
    pub(super) fn offset_parent_receiver(&self, object: u32) -> bool {
        self.class_list_element_receiver(object)
            || matches!(
                self.frame_node_interface(object),
                Some("HTMLElement" | "HTMLBodyElement" | "HTMLHtmlElement")
            )
    }

    pub(super) fn offset_parent_member(&self, object: u32, name: &str) -> bool {
        name == "offsetParent"
            && self.offset_parent_receiver(object)
            && !self.form_named_control(object, name)
    }

    pub(super) fn validate_offset_parent(&self, object: u32) -> Result<(), (&'static str, String)> {
        if !self.offset_parent_receiver(object) {
            return Err(("TypeError", "offsetParent requires an HTMLElement.".into()));
        }
        if self.is_frame_handle(object) {
            self.frame_light_dom_element(object)
                .map_err(|error| ("SecurityError", error))?;
            // The represented blank document's root and body always have
            // a null offset parent, independent of their box geometry.
            if !matches!(
                self.frame_concrete_html_interface(object),
                Some("HTMLHtmlElement" | "HTMLBodyElement")
            ) {
                return Err((
                    "NotSupportedError",
                    "Child element geometry is unavailable.".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn offset_parent(&mut self, object: u32) -> Result<Value, String> {
        self.validate_offset_parent(object)
            .map_err(|(_, message)| message)?;
        self.state.work(1)?;
        if self.is_frame_handle(object) {
            return Ok(Value::Null);
        }
        let node = self.dom_node(object)?;
        if !self.state.dom.connected[node] {
            return Ok(Value::Null);
        }
        if let Some(root) = self.state.tree.document_element {
            if node == root {
                return Ok(Value::Null);
            }
            if self.state.tree.parents[node] == Some(root)
                && matches!(tag(self.doc, node), "body" | "frameset")
            {
                self.state.work(self.doc.nodes[root].children.len())?;
                if self.doc.body_element(root) == Some(node) {
                    return Ok(Value::Null);
                }
            }
        }
        let geometry = self
            .state
            .geometry
            .as_mut()
            .ok_or("offsetParent requires the document renderer.")?;
        match geometry.offset_parent(
            self.doc,
            self.forms,
            node,
            &mut self.state.host_work,
            &mut self.state.copy_work,
        )? {
            Some(parent) => Ok(Value::Host(node_handle(parent))),
            None => Ok(Value::Null),
        }
    }
}
