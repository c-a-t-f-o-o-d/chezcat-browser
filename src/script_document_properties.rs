//! Main Document ordinary keys share the VM store; native members keep routing.
//! This is not a new Document interface or a child-Document capability.
use super::*;

impl Bridge<'_> {
    pub(super) fn document_native_property(&self, name: &str) -> bool {
        dom_tree::native_node_member(name)
            // An unavailable native factory keeps its diagnostic dispatch;
            // its presence/callable exposure remains controlled by the gate.
            || name == "createEvent"
            || self.method(DOCUMENT, name).is_some()
            // The fixed startup table also owns legacy prebound methods that
            // Host::method does not expose through dynamic native dispatch.
            || self.state.native_methods.iter().any(|(object, method, _)|
                *object == DOCUMENT && *method == name)
            || handler_event(name).is_some()
            || matches!(name,
                "location" | "cookie" | "URL" | "documentURI" | "contentType" | "title"
                | "body" | "head" | "documentElement" | "readyState" | "referrer"
                | "compatMode" | "prerendering" | "images" | "defaultView"
                | "localName" | "prefix" | "namespaceURI")
    }
}
