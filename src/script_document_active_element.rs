//! Read the represented document focus without changing focus or activation.
use super::*;
impl Bridge<'_> {
    pub(super) fn document_active_element(&mut self, object: u32) -> Result<Value, String> {
        if !self.document_interface_receiver(object) {
            return Err("activeElement requires a genuine accessible Document.".into());
        }
        if object == DOCUMENT {
            self.state.rebuild(self.doc)?;
            let focused = self.doc.native_focus.get().filter(|&node| {
                self.state.dom.connected.get(node) == Some(&true)
                    && self
                        .doc
                        .nodes
                        .get(node)
                        .is_some_and(|node| matches!(node.kind, NodeKind::Element { .. }))
            });
            return Ok(focused
                .or(self.state.tree.body)
                .or(self.state.tree.document_element)
                .map_or(Value::Null, |node| Value::Host(node_handle(node))));
        }
        // Inert documents and the represented read-only blank child have no
        // focused element. Their document viewport fallback is their real body
        // or document element; a template contents owner has neither.
        let caller = self.state.navigation_incumbent;
        let body = self.get_with_caller(caller, object, "body")?;
        if !matches!(body, Value::Null | Value::Undefined) {
            return Ok(body);
        }
        self.get_with_caller(caller, object, "documentElement")
    }
}
