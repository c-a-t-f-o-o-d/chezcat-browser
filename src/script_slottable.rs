//! Slottable is implemented by Element and Text. Both supported document
//! stores contain ordinary trees only. An HTML slot in such a tree is an
//! ordinary element, even when its name matches the slottable's slot attribute.
//! https://dom.spec.whatwg.org/#find-a-slot
use super::*;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Slottable {
    Element,
    Text,
}

impl Slottable {
    pub(super) fn getter(method: &str) -> Option<Self> {
        match method {
            "\0Element.get.assignedSlot" => Some(Self::Element),
            "\0Text.get.assignedSlot" => Some(Self::Text),
            _ => None,
        }
    }
}

impl Bridge<'_> {
    pub(super) fn html_slot_receiver(&self, object: u32) -> bool {
        self.inert_interface(object)==Some("HTMLSlotElement") || node_index(object).is_some()
            && self.concrete_html_interface(object) == Some("HTMLSlotElement")
    }

    pub(super) fn slottable(&self, object: u32) -> Option<Slottable> {
        if let Some(kind)=self.inert_node_type(object){return match kind{1=>Some(Slottable::Element),3=>Some(Slottable::Text),_=>None};}
        if self.frame_element_sibling_receiver(object) {
            return Some(Slottable::Element);
        }
        let node = node_index(object).filter(|&node| self.dom_accessible(node))?;
        match self.doc.nodes.get(node)?.kind {
            NodeKind::Element { .. } => Some(Slottable::Element),
            NodeKind::Text(_) => Some(Slottable::Text),
            NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. } | NodeKind::DocumentFragment | NodeKind::DocumentType(_) => None,
        }
    }

    pub(super) fn assigned_slot_lookup(&self, object: u32, name: &str) -> bool {
        name == "assignedSlot"
            && (self.frame_element_sibling_receiver(object) || self.namespace_receiver(object) || matches!(self.inert_node_type(object),Some(1|3)))
            && !self.form_named_control(object, name)
    }

    pub(super) fn text_prototype_property(&self, object: u32, name: &str) -> bool {
        matches!(name, "constructor" | "__proto__" | "toString")
            && self.slottable(object) == Some(Slottable::Text)
    }

    pub(super) fn assigned_slot(
        &mut self,
        object: u32,
        interface: Slottable,
    ) -> Result<Value, String> {
        self.state.work(1)?;
        if self.slottable(object) != Some(interface) {
            return Err("assignedSlot getter has an incompatible receiver.".into());
        }
        if self.inert_node_type(object).is_some(){
            if let Some(parent)=self.inert_observation_parent(object){
                if matches!(self.inert_import_source(parent),Some(script_inert_documents::ImportSource::Parsed{node,..}) if node.namespace.is_foreign()){return Err("Slottable parent namespace is unavailable.".into());}
            }
            return Ok(Value::Null);
        }
        if self.is_frame_handle(object) {
            self.frame_light_dom_element(object)?;
        } else {
            let node = self.dom_node(object)?;
            let parent = self
                .state
                .tree
                .parents
                .get(node)
                .ok_or("Missing slottable parent record.")?;
            if let Some(parent) = parent {
                let parent = self
                    .doc
                    .nodes
                    .get(*parent)
                    .ok_or("Invalid slottable parent.")?;
                if parent.namespace.is_foreign() {
                    return Err("Slottable parent namespace is unavailable.".into());
                }
            }
        }
        // A parent without a shadow root cannot assign a slot.
        Ok(Value::Null)
    }
}
