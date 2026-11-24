//! Genuine nullable ARIA string reflection on supported native Elements.
//! This is attribute state, not an accessibility tree or notification bridge.
use super::*;
use crate::javascript::aria_reflection_interface as interface;
impl Bridge<'_> {
    pub(super) fn aria_reflection_receiver(&self, object: u32) -> bool {
        self.attr_element_receiver(object)
            || self.inert_node_type(object) == Some(1)
            || self.frame_concrete_html_interface(object).is_some()
            || matches!(
                self.frame_node_interface(object),
                Some("Element" | "HTMLElement" | "HTMLBodyElement" | "HTMLHtmlElement")
            )
    }
    pub(super) fn aria_reflection_attribute(&self, object: u32, property: &str) -> bool {
        interface::property(property).is_some() && self.aria_reflection_receiver(object)
    }
    pub(super) fn aria_reflection_member(&self, object: u32, property: &str) -> bool {
        self.aria_reflection_attribute(object, property)
            && !self.form_named_control(object, property)
    }
    pub(super) fn validate_aria_reflection(
        &self,
        object: u32,
        setter: bool,
    ) -> Result<(), (&'static str, String)> {
        if !self.aria_reflection_receiver(object) {
            return Err((
                "TypeError",
                "ARIA reflection requires a genuine accessible Element.".into(),
            ));
        }
        if setter && self.is_frame_handle(object) {
            return Err((
                "NotSupportedError",
                "Blank child ARIA attribute writes are unavailable.".into(),
            ));
        }
        Ok(())
    }
    fn aria_reflection_attributes(&self, object: u32) -> Result<&[crate::html::Attribute], String> {
        if let Some((owner, node)) = self.frame_contains_node(object) {
            self.frame_light_dom_element(object)?;
            let NodeKind::Element { attrs, .. } =
                &self.contains_child_document(owner)?.nodes[node].kind
            else {
                return Err("Child ARIA reflection requires an Element.".into());
            };
            return Ok(attrs);
        }
        self.element_attribute_source(object)
            .ok_or("ARIA Element attributes are unavailable.".into())
    }
    pub(super) fn aria_reflection_get(
        &mut self,
        object: u32,
        property: &str,
    ) -> Result<Value, String> {
        self.validate_aria_reflection(object, false)
            .map_err(|(_, m)| m)?;
        let key = interface::property(property).ok_or("Unknown ARIA string reflection.")?;
        // Retain the child light-DOM preflight before the shared exact reader.
        self.aria_reflection_attributes(object)?;
        Ok(self.element_optional_attribute_string_get(object,key)?.unwrap_or(Value::Null))
    }
    pub(super) fn aria_reflection_set(
        &mut self,
        object: u32,
        property: &str,
        value: &Value,
    ) -> Result<(), String> {
        self.validate_aria_reflection(object, true)
            .map_err(|(_, m)| m)?;
        let key = interface::property(property).ok_or("Unknown ARIA string reflection.")?;
        let value = if matches!(value, Value::Null | Value::Undefined) {
            None
        } else {
            Some(self.dom_text(value)?)
        };
        self.state.copies(key.len())?;
        let name = crate::html::AttributeName::unqualified(script_clone_node::copy_text(key)?);
        if self.inert_node_type(object) == Some(1) {
            self.write_inert_dom_attribute_ns(object, name, value)
        } else {
            let node = self.node(object)?;
            self.public_dom_attribute(node,name,value,script_attr::InputPreparation::Pending)
        }
    }
}
