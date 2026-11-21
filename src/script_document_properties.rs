//! Main Document ordinary keys share the VM store; native members keep routing.
//! The ordinary interface belongs to the actual document's Window realm.
use super::*;

impl Bridge<'_> {
    pub(super) fn document_interface_receiver(&self, object:u32)->bool {
        object==DOCUMENT || self.active_frame_document(object) || self.inert_document(object)
    }

    pub(super) fn document_inherited_property(&self, object:u32, name:&str)->bool {
        if name == "importNode" { return object == DOCUMENT; }
        if matches!(name, "createCDATASection" | "createProcessingInstruction") {
            return object == DOCUMENT || self.inert_document(object);
        }
        object==DOCUMENT && matches!(name, "createTreeWalker" | "implementation") || self.document_interface_receiver(object)
            && (matches!(name, "documentElement" | "currentScript" | "activeElement") || script_element_properties::ordinary_interface_member(name))
    }

    pub(super) fn frame_document_prototype_lookup(&self, object:u32, name:&str)->bool {
        // Read author properties on the actual prototype without granting the
        // immutable child an own store, enumeration or definition capability.
        self.active_frame_document(object) && !self.document_native_property(name)
    }

    pub(super) fn document_native_property(&self, name: &str) -> bool {
        dom_tree::native_node_member(name)
            // An unavailable native factory keeps its diagnostic dispatch;
            // its presence/callable exposure remains controlled by the gate.
            || matches!(name, "createEvent" | "createTreeWalker" | "implementation" | "createCDATASection" | "createProcessingInstruction" | "importNode")
            || self.method(DOCUMENT, name).is_some()
            // The fixed startup table also owns legacy prebound methods that
            // Host::method does not expose through dynamic native dispatch.
            || self.state.native_methods.iter().any(|(object, method, _)|
                *object == DOCUMENT && *method == name)
            || handler_event(name).is_some()
            || matches!(name,
                "location" | "cookie" | "URL" | "documentURI" | "contentType" | "title"
                | "body" | "head" | "documentElement" | "currentScript" | "activeElement" | "readyState" | "referrer"
                | "compatMode" | "prerendering" | "images" | "defaultView"
                | "localName" | "prefix" | "namespaceURI")
    }
}
