//! Finite HTML hidden-input attributes and retained native form state.
//! Other existing input-type transitions remain explicitly refused.
//! https://html.spec.whatwg.org/multipage/input.html#hidden-state-(type=hidden)
use super::*;

impl Bridge<'_> {
    pub(super) fn input_receiver(&self, object: u32) -> bool {
        self.attr_element_receiver(object) && node_index(object).is_some_and(|node|
            self.doc.nodes[node].namespace == crate::html::Namespace::Html
            && tag(self.doc,node) == "input")
    }
    pub(super) fn hidden_input(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|item| item.namespace == crate::html::Namespace::Html)
            && tag(self.doc,node) == "input"
            && self.doc.attr(node,"type").is_some_and(|value| value.eq_ignore_ascii_case("hidden"))
            && self.forms.controls.get(&node).is_some_and(|control| control.kind == Kind::Hidden)
    }
    pub(super) fn hidden_input_property(&self, object: u32, property: &str) -> bool {
        self.input_receiver(object) && (property == "type" || node_index(object).is_some_and(|node|
            self.hidden_input(node) && matches!(property,"name"|"value"|"defaultValue")))
    }
    pub(super) fn hidden_input_get(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if !self.hidden_input_property(object,property) { return Ok(None); }
        let node = self.node(object)?;
        let text = if property == "type" {
            let raw = self.doc.attr(node,"type").unwrap_or("text");
            let value = ["hidden","text","search","tel","url","email","password","date","month","week","time","datetime-local","number","range","color","checkbox","radio","file","submit","image","reset","button"]
                .into_iter().find(|value|raw.eq_ignore_ascii_case(value)).unwrap_or("text");
            value.to_owned()
        } else { self.doc.attr(node,if property=="name" {"name"} else {"value"}).unwrap_or("").to_owned() };
        self.string(&text).map(Some)
    }
    pub(super) fn hidden_input_set(&mut self, object: u32, property: &str, value: &Value) -> Result<bool, String> {
        if !self.hidden_input_property(object,property) { return Ok(false); }
        let node = self.node(object)?;
        let key = match property { "defaultValue"|"value"=>"value", "name"=>"name", _=>"type" };
        let text = if property=="value" && matches!(value,Value::Null) { String::new() } else { self.text(value)? };
        self.hidden_input_attribute(node,&crate::html::AttributeName::unqualified(key.into()),&Some(text))
    }
    pub(super) fn hidden_input_attribute(&mut self, node: usize, name: &crate::html::AttributeName,
        value: &Option<String>) -> Result<bool, String> {
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html || tag(self.doc,node)!="input" { return Ok(false); }
        let key = name.qualified.as_str();
        if name.namespace.is_some() || !matches!(key,"type"|"name"|"value") { return Ok(false); }
        if key == "type" {
            if !value.as_deref().is_some_and(|value| value.eq_ignore_ascii_case("hidden")) {
                return Err("Supported attributes: input type assignment is limited to hidden; no input was changed.".into());
            }
            if self.forms.controls.get(&node).is_none()
                && (!self.state.dom.created.contains(&node) || self.state.dom.connected[node]) {
                return Err("Hidden type admission requires a fresh detached input; no input was changed.".into());
            }
        } else if !self.hidden_input(node) { return Ok(false); }
        self.state.work(self.forms.controls.len().saturating_add(self.doc.nodes.len()))?;
        let scanned = self.forms.controls.values().fold(0usize, |bytes, control| {
            let bytes = bytes.saturating_add(control.name.len()).saturating_add(control.value.chars.len().saturating_mul(std::mem::size_of::<char>()));
            control.select.as_ref().map_or(bytes, |select| select.options.iter().fold(bytes,
                |bytes, option| bytes.saturating_add(option.label.len()).saturating_add(option.value.len())))
        });
        self.state.copies(scanned)?;
        let plan = self.forms.prepare_hidden_input(self.doc,node,key,value.as_deref())?;
        // Native form payload has its own finite control/value limits. The
        // existing attribute transaction owns DOM retained-byte accounting.
        self.state.copies(plan.copy_bytes)?;
        self.attribute_named(node,name.copy(),value.clone())?;
        plan.commit(self.forms);
        self.state.dirty = true;
        Ok(true)
    }
}
