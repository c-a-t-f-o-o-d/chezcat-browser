//! Canonical Element operations over the existing bounded selector engine.
//! https://dom.spec.whatwg.org/#interface-parentnode
//! https://webidl.spec.whatwg.org/#es-operations
//! The VM validates the real receiver/origin before required-argument and
//! DOMString conversion. Matching still runs after observable conversion.
use super::*;

pub(super) fn operation(method: &str) -> Option<&'static str> {
    match method {
        "\0Element.querySelector" | "\0Document.querySelector" | "\0DocumentFragment.querySelector" => Some("querySelector"),
        "\0Element.querySelectorAll" | "\0Document.querySelectorAll" | "\0DocumentFragment.querySelectorAll" => Some("querySelectorAll"),
        "\0Element.matches" => Some("matches"),
        "\0Element.closest" => Some("closest"),
        _ => None,
    }
}

pub(super) fn member(property:&str)->bool{matches!(property,"querySelector"|"querySelectorAll"|"matches"|"closest")}

impl Bridge<'_> {
    pub(super) fn element_selector_receiver(&self, object: u32) -> bool {
        self.frame_element_sibling_receiver(object)
            || node_index(object).filter(|&node| self.dom_accessible(node))
                .is_some_and(|node| matches!(self.doc.nodes[node].kind, NodeKind::Element {..}))
    }

    pub(super) fn element_selector_member(&self, object: u32, property: &str) -> bool {
        member(property)
            && self.canonical_selector_receiver(object)
            && !self.form_named_control(object, property)
    }

    pub(super) fn canonical_selector_receiver(&self,object:u32)->bool{
        self.element_selector_receiver(object)||self.inert_node_type(object)==Some(1)
    }

    pub(super) fn selector_operation_receiver(&self,object:u32,method:&str)->bool{
        if method.starts_with("\0Document."){object==DOCUMENT||self.inert_node_type(object)==Some(9)||self.frame_contains_node(object).is_some_and(|(_,node)|node==0)}
        else if method.starts_with("\0DocumentFragment."){match self.inert_node_type(object){Some(kind)=>kind==11,None=>self.fragment_receiver(object)}}
        else{self.canonical_selector_receiver(object)}
    }
    pub(super) fn validate_selector_operation_receiver(&self,object:u32,method:&str)->Result<(),(&'static str,String)>{
        if method.starts_with("\0Document.")||method.starts_with("\0DocumentFragment."){
            if !self.selector_operation_receiver(object,method){return Err(("TypeError","Selector operation requires a genuine receiver implementing its defining interface.".into()));}
            if self.is_frame_handle(object){return Err(("NotSupportedError","Blank child Document queries are not implemented.".into()));}Ok(())
        }else{self.validate_element_selector_receiver(object)}
    }
    pub(super) fn validate_element_selector_receiver(&self, object: u32)
        -> Result<(), (&'static str, String)>
    {
        if !self.canonical_selector_receiver(object) {
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
