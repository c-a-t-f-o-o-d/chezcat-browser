//! Common inherited HTML reflections over real native attribute sources.
//! Hidden-until-found reveal/layout remains outside this reflection slice.
use super::*;
use crate::javascript::html_reflection_interface as interface;
impl Bridge<'_> {
    pub(super) fn html_reflection_receiver(&self, object: u32) -> bool {
        self.attr_element_receiver(object)
            && node_index(object)
                .is_some_and(|node| self.doc.nodes[node].namespace == crate::html::Namespace::Html)
            || self.frame_concrete_html_interface(object).is_some()
            || self.frame_node_interface(object) == Some("HTMLElement")
            || self.inert_node_type(object) == Some(1)
                && self
                    .inert_interface(object)
                    .is_some_and(|i| i == "HTMLElement" || i.starts_with("HTML"))
    }
    pub(super) fn html_reflection_member(&self, object: u32, name: &str) -> bool {
        interface::property(name).is_some()
            && self.html_reflection_receiver(object)
            && !self.form_named_control(object, name)
    }
    pub(super) fn validate_html_reflection(
        &self,
        object: u32,
        setter: bool,
    ) -> Result<(), (&'static str, String)> {
        if !self.html_reflection_receiver(object) {
            return Err((
                "TypeError",
                "HTML reflection requires a genuine accessible HTMLElement.".into(),
            ));
        }
        if setter && self.is_frame_handle(object) {
            return Err((
                "NotSupportedError",
                "Blank child native reflection writes are unavailable.".into(),
            ));
        }
        Ok(())
    }
    fn html_reflection_attributes(&self, object: u32) -> Result<&[crate::html::Attribute], String> {
        if let Some((owner, node)) = self.frame_contains_node(object) {
            self.frame_light_dom_element(object)?;
            let NodeKind::Element { attrs, .. } =
                &self.contains_child_document(owner)?.nodes[node].kind
            else {
                return Err("Child HTML reflection requires an Element.".into());
            };
            return Ok(attrs);
        }
        self.element_attribute_source(object)
            .ok_or("HTML reflection attributes are unavailable.".into())
    }
    pub(super) fn html_reflection_get(&mut self, object: u32, name: &str) -> Result<Value, String> {
        self.validate_html_reflection(object, false)
            .map_err(|(_, m)| m)?;
        if name=="tabIndex" && self.inert_node_type(object).is_none() {
            if let Some(value)=self.frame_get_with_caller(self.state.navigation_incumbent.or(Some(WINDOW)),object,"tabIndex") {return value;}
            let node=self.node(object)?;
            return self.html_tab_index(node).map(|value|Value::Number(f64::from(value)));
        }
        let attribute=interface::attribute(name).ok_or("Unknown HTML reflected attribute.")?;
        let count = self.html_reflection_attributes(object)?.len();
        self.state.work(count.saturating_mul(4))?;
        fn raw<'a>(attrs: &'a [crate::html::Attribute], name: &str) -> Option<&'a str> {
            attrs
                .iter()
                .find(|a| a.name.html_is(name))
                .map(|a| a.value.as_str())
        }
        let bytes = raw(self.html_reflection_attributes(object)?, attribute).map_or(0, str::len);
        self.state.work(bytes)?;
        if name=="tabIndex" {
            self.state.copies(bytes)?;
            if let Some(value)=raw(self.html_reflection_attributes(object)?,attribute).and_then(forms::parse_tab_index) {
                return Ok(Value::Number(f64::from(value)));
            }
            let name=self.inert_html_reflection_tag(object).ok_or("Inert HTML tabIndex source is unavailable.")?;
            if matches!(name,"a"|"area"|"button"|"frame"|"iframe"|"input"|"object"|"select"|"textarea") {
                return Ok(Value::Number(0.0));
            }
            if name=="summary" {
                if let Some(parent)=self.inert_observation_parent(object).filter(|&parent|self.inert_html_reflection_tag(parent)==Some("details")) {
                    let count=self.inert_children(parent).map_or(0,|children|children.len());
                    for offset in 0..count {
                        self.state.work(1)?;
                        let child=self.inert_children(parent).unwrap()[offset];
                        if self.inert_html_reflection_tag(child)==Some("summary") {return Ok(Value::Number(if child==object {0.0}else{-1.0}));}
                    }
                }
            }
            return Ok(Value::Number(-1.0));
        }
        if name == "hidden" {
            let value = raw(self.html_reflection_attributes(object)?, name);
            return match value {
                Some(v) if v.eq_ignore_ascii_case("until-found") => self.string("until-found"),
                Some(_) => Ok(Value::Bool(true)),
                None => Ok(Value::Bool(false)),
            };
        }
        if name == "dir" {
            let value = raw(self.html_reflection_attributes(object)?, name).unwrap_or("");
            let value = ["ltr", "rtl", "auto"]
                .into_iter()
                .find(|s| value.eq_ignore_ascii_case(s))
                .unwrap_or("");
            return self.string(value);
        }
        if bytes > self.state.limits.host.string_bytes {
            return Err("HTML reflected value exceeds the string allowance.".into());
        }
        self.state.copies(bytes)?;
        let value = raw(self.html_reflection_attributes(object)?, name).unwrap_or("");
        Ok(Value::String(script_clone_node::copy_text(value)?))
    }
    fn inert_html_reflection_tag(&self,object:u32)->Option<&str> {
        match self.inert_import_source(object)? {
            script_inert_documents::ImportSource::Element(tag)=>Some(tag),
            script_inert_documents::ImportSource::Parsed {node:Node {kind:NodeKind::Element {tag,..},namespace:crate::html::Namespace::Html,..},..}=>Some(tag),
            _=>None,
        }
    }
    pub(super) fn html_reflection_set(
        &mut self,
        object: u32,
        name: &str,
        value: &Value,
    ) -> Result<(), String> {
        self.validate_html_reflection(object, true)
            .map_err(|(_, m)| m)?;
        let text = if name == "hidden" {
            match value {
                Value::String(s) if s.eq_ignore_ascii_case("until-found") => {
                    self.state.copies(11)?;
                    Some(script_clone_node::copy_text("until-found")?)
                }
                Value::Null | Value::Undefined => None,
                _ => value.truthy().then(String::new),
            }
        } else if name=="tabIndex" {
            let text=timer_integer(value)?.to_string();self.state.copies(text.len())?;Some(text)
        } else {
            Some(self.text(value)?)
        };
        let name=interface::attribute(name).ok_or("Unknown HTML reflected attribute.")?;
        if self.inert_node_type(object) == Some(1) {
            self.state.copies(name.len())?;
            let name = crate::html::AttributeName::unqualified(script_clone_node::copy_text(name)?);
            self.write_inert_attribute(object, name, text, false)
        } else {
            let node = self.node(object)?;
            self.attribute(node, name, text)
        }
    }
}
