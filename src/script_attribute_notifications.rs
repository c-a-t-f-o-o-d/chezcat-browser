//! Native alias/token/reflection writes describe their actual Element owner.
use super::*;
use crate::javascript::NativeAttributeChange;

impl Bridge<'_> {
    pub(super) fn attribute_change_for_name(
        &mut self,
        object: u32,
        name: &crate::html::AttributeName,
    ) -> Result<NativeAttributeChange, String> {
        if self.inert_node_type(object).is_none() {
            self.attr_element_node(object)?;
        }
        let attrs = self
            .element_attribute_source(object)
            .ok_or("Observed Element attributes are unavailable.")?;
        let work = attrs
            .iter()
            .fold(attrs.len(), |n, a| n.saturating_add(a.name.bytes()));
        self.state.work(work)?;
        let slot = self
            .element_attribute_source(object)
            .unwrap()
            .iter()
            .position(|attr| attr.name.same_expanded_name(name));
        let old_length = slot.map_or(0, |slot| {
            self.element_attribute_source(object).unwrap()[slot]
                .value
                .len()
        });
        self.state.copies(
            name.local()
                .len()
                .saturating_add(name.namespace_uri().map_or(0, str::len))
                .saturating_add(old_length),
        )?;
        let old = slot
            .map(|slot| {
                script_clone_node::copy_text(
                    &self.element_attribute_source(object).unwrap()[slot].value,
                )
            })
            .transpose()?;
        Ok(NativeAttributeChange {
            local_name: script_clone_node::copy_text(name.local())?,
            namespace: name
                .namespace_uri()
                .map(script_clone_node::copy_text)
                .transpose()?,
            old_value: old,
        })
    }
    pub(super) fn token_attribute_change(
        &mut self,
        object: u32,
    ) -> Result<Option<(u32, NativeAttributeChange)>, String> {
        if !self.class_list_receiver(object) {
            return Ok(None);
        }
        let owner =
            script_class_list::element(object).ok_or("Token attribute owner is unavailable.")?;
        let key =
            script_class_list::attribute(object).ok_or("Token attribute name is unavailable.")?;
        self.state.copies(key.len())?;
        let name = crate::html::AttributeName::unqualified(script_clone_node::copy_text(key)?);
        self.attribute_change_for_name(owner, &name)
            .map(|change| Some((owner, change)))
    }
    pub(super) fn attribute_setter_change(
        &mut self,
        object: u32,
        property: &str,
    ) -> Result<Option<(u32, NativeAttributeChange)>, String> {
        if self.attr_receiver(object) && matches!(property, "value" | "nodeValue" | "textContent") {
            let id = script_attr::index(object).unwrap();
            let Some(owner) = self.state.attributes[id].owner else {
                return Ok(None);
            };
            let bytes = self.state.attributes[id].name.bytes();
            self.state.copies(bytes)?;
            let name = script_attr::copy_name(&self.state.attributes[id].name)?;
            return self
                .attribute_change_for_name(owner, &name)
                .map(|change| Some((owner, change)));
        }
        if property == "value" && self.class_list_receiver(object) {
            return self.token_attribute_change(object);
        }
        if !self.attr_element_receiver(object) {
            return Ok(None);
        }
        let node = node_index(object).unwrap();
        let html = self.doc.nodes[node].namespace == crate::html::Namespace::Html;
        let svg = self.doc.nodes[node].namespace == crate::html::Namespace::Svg;
        let key = match property {
            "id" if html || svg => "id",
            "className" if html => "class",
            "classList" if self.class_list_element_receiver(object) => "class",
            "title" if html => "title",
            _ => return Ok(None),
        };
        self.state.copies(key.len())?;
        let name = crate::html::AttributeName::unqualified(script_clone_node::copy_text(key)?);
        self.attribute_change_for_name(object, &name)
            .map(|change| Some((object, change)))
    }
}
