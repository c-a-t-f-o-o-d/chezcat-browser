//! Native BUTTON reflection and staged whole-node admission. Fresh detached
//! buttons keep ordinary caption construction until their first insertion.
use super::*;

impl Bridge<'_> {
    pub(super) fn button_node(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|entry| entry.namespace == crate::html::Namespace::Html)
            && tag(self.doc,node) == "button"
    }
    pub(super) fn native_button(&self, node: usize) -> bool {
        self.button_node(node) && self.forms.controls.get(&node)
            .is_some_and(|control| matches!(control.kind,Kind::Button|Kind::Submit|Kind::Reset))
    }
    pub(super) fn fresh_button(&self, node: usize) -> bool {
        self.button_node(node) && !self.forms.controls.contains_key(&node)
            && self.state.dom.created.contains(&node) && self.state.dom.connected.get(node)==Some(&false)
    }
    pub(super) fn button_property(&self, object: u32, property: &str) -> bool {
        matches!(property,"name"|"value"|"form") && self.attr_element_receiver(object)
            && node_index(object).is_some_and(|node| self.native_button(node) || self.fresh_button(node))
    }
    pub(super) fn button_string_property(&self, object: u32, property: &str) -> bool {
        (matches!(property,"name"|"value") && self.button_property(object,property))
            || self.button_type_property(object,property)
    }
    pub(super) fn button_get(&mut self, object: u32, property: &str) -> Result<Option<Value>,String> {
        if !self.button_property(object,property) { return Ok(None); }
        let node = self.node(object)?;
        if property == "form" {
            return Ok(Some(self.forms.controls.get(&node).and_then(|control|control.owner)
                .map_or(Value::Null,|owner|Value::Host(node_handle(owner)))));
        }
        let text = self.doc.attr(node,property).unwrap_or("");
        if text.len()>self.state.limits.host.string_bytes {return Err("BUTTON reflected value exceeds the host string allowance.".into());}
        self.state.copies(text.len())?;
        Ok(Some(Value::String(self.doc.attr(node,property).unwrap_or("").to_owned())))
    }
    pub(super) fn button_set(&mut self, object: u32, property: &str, value: &Value) -> Result<bool,String> {
        if !self.button_string_property(object,property) { return Ok(false); }
        let node = self.node(object)?;
        let text = self.text(value)?;
        self.attribute(node,property,Some(text))?;
        Ok(true)
    }
    pub(super) fn prepare_button_attribute(&mut self, node: usize, name: &crate::html::AttributeName,
        value: Option<&str>) -> Result<Option<forms::ButtonAttributePlan>,String> {
        if !self.button_node(node) || !(name.html_is("name") || name.html_is("type") || name.html_is("value")) { return Ok(None); }
        if name.html_is("type") {
            self.state.rebuild(self.doc)?;
            self.state.work(1)?;
            if self.state.tree.parents.get(node).copied().flatten().is_some_and(|parent|
                self.doc.nodes[parent].namespace == crate::html::Namespace::Html && tag(self.doc,parent)=="select") {
                return Err("BUTTON type changes immediately inside SELECT are not implemented; no attribute or native control was changed.".into());
            }
        }
        let name_bytes = if name.html_is("name") {value.map_or(0,str::len)} else {self.doc.attr(node,"name").map_or(0,str::len)};
        let value_bytes = if name.html_is("value") {value.map_or(0,str::len)} else {self.doc.attr(node,"value").map_or(0,str::len)};
        self.state.work(self.forms.default_scan_work().saturating_add(name_bytes).saturating_add(value_bytes).saturating_add(value.map_or(0,str::len)))?;
        self.state.copies(name_bytes.saturating_add(value_bytes.saturating_mul(6)))?;
        self.forms.prepare_button_attribute(self.doc,node,&name.qualified,value)
    }

    pub(super) fn validate_button_children(&mut self, parent: usize, nodes: &[usize]) -> Result<(),String> {
        let mut ancestor = Some(parent);
        for _ in 0..128 {
            let Some(node) = ancestor else {return Ok(());};
            self.state.work(1)?;
            if self.fresh_button(node) {
                self.state.work(nodes.len())?;
                if nodes.iter().any(|&node| !forms::button_caption_node(self.doc,node)) {
                    return Err("Created BUTTON captions accept plain HTML text content only; no tree was changed.".into());
                }
                return Ok(());
            }
            ancestor = self.state.tree.parents.get(node).copied().flatten();
        }
        // The final permitted step may exhaust the chain. The tree operation
        // still checks the prospective parent-plus-child depth independently.
        if ancestor.is_none() {Ok(())} else {Err("BUTTON caption ancestry limit reached.".into())}
    }
}

#[cfg(test)]
#[path="script_created_button_tests.rs"]
mod tests;
