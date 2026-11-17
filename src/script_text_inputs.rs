//! Native single-line INPUT default reflection and finite control metadata.
//! Attribute updates stage native state before the shared DOM commit; existing
//! edited editors and neighboring controls retain their identity and selection.
use super::*;

impl Bridge<'_> {
    pub(super) fn native_text_input(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|entry| entry.namespace == crate::html::Namespace::Html)
            && tag(self.doc,node) == "input"
            && self.forms.controls.get(&node).is_some_and(|control| control.kind == Kind::Text)
    }

    pub(super) fn text_input_default_property(&self, object: u32, property: &str) -> bool {
        property == "defaultValue" && self.input_receiver(object)
            && node_index(object).is_some_and(|node| self.native_text_input(node))
    }

    pub(super) fn button_type_property(&self, object: u32, property: &str) -> bool {
        property == "type" && self.attr_element_receiver(object)
            && node_index(object).is_some_and(|node|
                self.doc.nodes[node].namespace == crate::html::Namespace::Html && tag(self.doc,node) == "button")
    }

    pub(super) fn text_input_metadata_get(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if self.text_input_default_property(object,property) {
            let node=self.node(object)?;
            self.forms.text_default_value(node)?;
            let text=self.doc.attr(node,"value").unwrap_or("");
            if text.len()>self.state.limits.host.string_bytes {return Err("Text INPUT default exceeds the DOM string allowance.".into());}
            self.state.copies(text.len())?;
            return Ok(Some(Value::String(self.doc.attr(node,"value").unwrap_or("").to_owned())));
        }
        if self.button_type_property(object,property) {
            let node=self.node(object)?;
            let raw=self.doc.attr(node,"type").unwrap_or("");
            let explicit=["submit","reset","button"].into_iter().find(|kind|raw.eq_ignore_ascii_case(kind));
            // HTML's Auto state is a submit button except command buttons and
            // a button whose immediate parent is SELECT. This is metadata only;
            // command activation and type mutation retain their own boundaries.
            let text=explicit.unwrap_or_else(|| if self.doc.attr(node,"command").is_none()
                && self.doc.attr(node,"commandfor").is_none()
                && !self.state.tree.parents.get(node).copied().flatten().is_some_and(|parent|
                    self.doc.nodes[parent].namespace == crate::html::Namespace::Html && tag(self.doc,parent)=="select")
                {"submit"} else {"button"});
            return self.string(text).map(Some);
        }
        Ok(None)
    }

    pub(super) fn prepare_text_input_default(&mut self, node: usize, name: &crate::html::AttributeName,
        value: Option<&str>) -> Result<Option<forms::TextDefaultPlan>, String> {
        if !name.html_is("value") || !self.native_text_input(node) { return Ok(None); }
        let scanned=self.forms.default_scan_work();
        let input=value.map_or(0,str::len);
        self.state.work(scanned.saturating_add(input).saturating_add(self.forms.controls.len()))?;
        // One raw default plus at most one four-byte-per-character editor;
        // charge this upper bound before prepare makes the first allocation.
        self.state.copies(input.checked_mul(5).ok_or("Text INPUT default copy accounting overflow.")?)?;
        let plan=self.forms.prepare_text_default(node,value)?;
        // Native values have their own finite form payload allowance. DOM
        // retained-byte accounting continues to cover the content attribute;
        // native bytes that were never charged are never subtracted from it.
        Ok(Some(plan))
    }
}

#[cfg(test)]
#[path="script_text_input_tests.rs"]
mod tests;
