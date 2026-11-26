//! Native text and checkbox INPUT admission and finite control metadata.
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
        matches!(property, "name" | "defaultValue" | "value") && self.input_receiver(object)
            && node_index(object).is_some_and(|node| self.native_text_input(node) || self.fresh_text_input(node)
                || self.owned_native_checkbox(node) || self.owned_native_radio(node))
    }

    pub(super) fn fresh_text_input(&self, node: usize) -> bool {
        forms::fragment_text_input(self.doc, node) && !self.forms.controls.contains_key(&node)
            && self.state.dom.created.contains(&node)
            && !self.state.dom.connected.get(node).copied().unwrap_or(true)
    }

    pub(super) fn unadmitted_parentless_input_form(&self, object: u32) -> bool {
        self.input_receiver(object) && node_index(object).is_some_and(|node|
            self.fresh_text_input(node)
                && self.state.tree.parents.get(node) == Some(&None)
                && self.doc.attr(node,"form").is_none())
    }

    fn created_input_admission_guard(&mut self, node: usize) -> Result<usize, String> {
        if !self.fresh_text_input(node) || self.doc.attr(node, "form").is_some() || !self.doc.nodes[node].children.is_empty() {
            return Err("Native INPUT admission requires a fresh detached empty default/text input without explicit form association.".into());
        }
        let mut ancestor = Some(node);
        let mut root = node;
        for _ in 0..128 {
            let Some(index) = ancestor else { return Ok(root); };
            root = index;
            self.state.work(1)?;
            let fragment = self.doc.nodes.get(index).is_some_and(|entry|
                entry.namespace == crate::html::Namespace::None && matches!(entry.kind,NodeKind::DocumentFragment))
                && self.fragment_receiver(node_handle(index));
            if index == 0 || self.doc.nodes.get(index).is_none_or(|entry| entry.namespace != crate::html::Namespace::Html) && !fragment
                || index != node && (matches!(tag(self.doc,index), "head" | "script" | "style" | "template" | "iframe" | "object" | "datalist" | "select" | "option" | "optgroup")
                    || tag(self.doc,index) == "noscript" && self.doc.scripting_enabled) {
                return Err("Native INPUT admission requires detached known HTML ancestry outside inert/native interiors.".into());
            }
            ancestor = self.state.tree.parents.get(index).copied().flatten();
        }
        Err("Native INPUT admission ancestry limit reached.".into())
    }

    fn prepare_created_input_plan(&mut self, node: usize, attribute: Option<(&str, Option<&str>)>,
        live: Option<&str>) -> Result<forms::InputPlan, String> {
        let root = self.created_input_admission_guard(node)?;
        let kind = match attribute {
            Some(("type", Some(value))) if value.eq_ignore_ascii_case("checkbox") => Kind::Checkbox,
            Some(("type", Some(value))) if value.eq_ignore_ascii_case("radio") => Kind::Radio,
            _ => Kind::Text,
        };
        let name = match attribute { Some(("name", value)) => value.unwrap_or(""), _ => self.doc.attr(node,"name").unwrap_or("") };
        let initial = match attribute { Some(("value", value)) => value.unwrap_or(""), _ => self.doc.attr(node,"value").unwrap_or(if matches!(kind,Kind::Checkbox|Kind::Radio) { "on" } else { "" }) };
        let input = name.len().saturating_add(initial.len()).saturating_add(live.map_or(0,str::len));
        self.state.work(self.forms.default_scan_work().saturating_add(input).saturating_add(self.doc.nodes[node].children.len()))?;
        // Name/default copies, one sanitized four-byte editor, and the bounded
        // caption all precede allocation. Retained growth is committed with DOM.
        self.state.copies(name.len().saturating_add(initial.len().saturating_mul(6)).saturating_add(live.map_or(0,str::len).saturating_mul(4)))?;
        let mut plan = self.forms.prepare_created_input(self.doc,node,kind,name,initial,live)?;
        if let forms::InputPlan::Admit { controls, .. } = &mut plan {
            let control = controls.get_mut(&node).ok_or("Missing prepared INPUT identity.")?;
            control.group_root = root;
            let type_value = match attribute { Some(("type",value)) => value, _ => self.doc.attr(node,"type") };
            control.password = kind == Kind::Text && type_value.is_some_and(|value|value.eq_ignore_ascii_case("password"));
        }
        Ok(plan)
    }

    pub(super) fn accepts_input_type(&self, node: usize, name: &crate::html::AttributeName, value: Option<&str>) -> bool {
        if !name.html_is("type") { return false; }
        let raw = value.unwrap_or("");
        if (self.native_text_input(node) || self.fresh_text_input(node)) && forms::input_kind(raw) == Kind::Text { return true; }
        if raw.eq_ignore_ascii_case("checkbox") {
            self.fresh_text_input(node) || self.owned_native_checkbox(node)
        } else if raw.eq_ignore_ascii_case("radio") {
            self.fresh_text_input(node) || self.owned_native_radio(node)
        } else {
            (raw.is_empty() || raw.eq_ignore_ascii_case("text")) && (self.fresh_text_input(node) || self.owned_fragment_text_input(node))
        }
    }

    pub(super) fn text_input_set(&mut self, object: u32, property: &str, value: &Value) -> Result<bool, String> {
        if property == "checked" && self.input_receiver(object) {
            let node = self.node(object)?;
            if self.fresh_text_input(node) {
                let mut plan = self.prepare_created_input_plan(node,None,None)?;
                plan.set_created_checked(value.truthy())?;
                self.state.replace_bytes(0,plan.retained_bytes())?;
                plan.commit(self.forms);
                self.state.layout_changed();
                return Ok(true);
            }
        }
        if property == "type" && self.input_receiver(object) {
            let node = self.node(object)?;
            let text = self.text(value)?;
            let name = crate::html::AttributeName::unqualified("type".into());
            let converted = Some(text);
            if self.accepts_input_type(node, &name, converted.as_deref()) {
                self.attribute_named(node, name, converted)?;
                return Ok(true);
            }
            // The hidden path owns the remaining finite type policy. Pass the
            // already-converted DOMString, rather than revisiting its setter.
            return self.hidden_input_attribute(node,&name,&converted);
        }
        if !self.text_input_default_property(object,property) { return Ok(false); }
        let node = self.node(object)?;
        let text = if property == "value" && matches!(value,Value::Null) { String::new() } else { self.text(value)? };
        if property != "value" || self.owned_native_checkbox(node) || self.owned_native_radio(node) {
            self.attribute(node,if property == "name" { "name" } else { "value" },Some(text))?;
        } else {
            let plan = if self.fresh_text_input(node) { self.prepare_created_input_plan(node,None,Some(&text))? } else {
                self.state.work(self.forms.default_scan_work().saturating_add(text.len()))?;
                self.state.copies(text.len().saturating_mul(4))?;
                self.forms.prepare_text_live(node,&text)?
            };
            self.state.replace_bytes(0,plan.retained_bytes())?;
            plan.commit(self.forms);
            self.state.layout_changed();
        }
        Ok(true)
    }

    pub(super) fn button_type_property(&self, object: u32, property: &str) -> bool {
        property == "type" && self.attr_element_receiver(object)
            && node_index(object).is_some_and(|node|
                self.doc.nodes[node].namespace == crate::html::Namespace::Html && tag(self.doc,node) == "button")
    }

    pub(super) fn text_input_metadata_get(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if property == "form" && self.unadmitted_parentless_input_form(object) {
            // With neither an ancestor nor an explicit association, the owner
            // is known without allocating native value/checkedness state.
            return Ok(Some(Value::Null));
        }
        if property == "checked" && self.input_receiver(object) {
            let node = self.node(object)?;
            if self.fresh_text_input(node) {
                // An untouched detached INPUT has no dirty state to retain.
                // Observing its default must not admit a text editor or make
                // its otherwise supported initial type selection unavailable.
                return Ok(Some(Value::Bool(self.doc.attr(node,"checked").is_some())));
            }
        }
        if self.text_input_default_property(object,property) {
            let node=self.node(object)?;
            let fresh = self.fresh_text_input(node);
            let checkable = self.owned_native_checkbox(node) || self.owned_native_radio(node);
            let live_editor = property == "value" && !fresh && !checkable;
            let raw = self.doc.attr(node, if property == "name" { "name" } else { "value" })
                .unwrap_or(if checkable && property == "value" { "on" } else { "" });
            let size = if live_editor {
                self.forms.controls[&node].value.chars.iter().map(|ch| ch.len_utf8()).sum()
            } else { raw.len() };
            if size>self.state.limits.host.string_bytes {return Err("Text INPUT result exceeds the DOM string allowance.".into());}
            // A read may materialize native state, but a failed result-copy or
            // admission check must never publish that state.
            self.state.copies(size)?;
            let text = if live_editor {
                self.forms.controls[&node].value.text()
            } else if fresh && property == "value" {
                raw.chars().filter(|ch| !matches!(ch,'\r'|'\n')).collect()
            } else { raw.to_owned() };
            let plan = if fresh { Some(self.prepare_created_input_plan(node,None,None)?) } else { None };
            if let Some(plan) = plan { self.state.replace_bytes(0,plan.retained_bytes())?; plan.commit(self.forms); }
            return Ok(Some(Value::String(text)));
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

    pub(super) fn prepare_input_attribute(&mut self, node: usize, name: &crate::html::AttributeName,
        value: Option<&str>) -> Result<Option<forms::InputPlan>, String> {
        if name.html_is("type") && self.native_text_input(node) && forms::input_kind(value.unwrap_or("")) == Kind::Text {
            self.state.work(value.map_or(1, |value|value.len().saturating_add(1)))?;
            script_text_content::deadline(self.state)?;
            return Ok(Some(forms::InputPlan::TextType { node, password: value.is_some_and(|value|value.eq_ignore_ascii_case("password")) }));
        }
        if name.html_is("checked") && self.input_receiver(node_handle(node)) {
            if let Some(control) = self.forms.controls.get(&node) {
                let present = value.is_some();
                let synchronize = self.doc.attr(node,"checked").is_some() != present;
                if control.kind == Kind::Radio && !control.dirty_checked && synchronize && present {
                    self.state.work(self.forms.controls.len())?;
                    self.state.copies(self.forms.controls.values().map(|c| c.name.len()).sum())?;
                } else { self.state.work(1)?; }
                return Ok(Some(forms::InputPlan::Checked { node, present, synchronize }));
            }
            return Ok(None);
        }
        if !(name.html_is("value") || name.html_is("name") || name.html_is("type")) { return Ok(None); }
        if name.html_is("type") && value.is_some_and(|value| forms::input_kind(value) != Kind::Text && !value.eq_ignore_ascii_case("checkbox") && !value.eq_ignore_ascii_case("radio")) { return Ok(None); }
        if self.fresh_text_input(node) { return self.prepare_created_input_plan(node,Some((&name.qualified,value)),None).map(Some); }
        if name.html_is("type") { return Ok(None); }
        if name.html_is("value") && (self.owned_native_checkbox(node) || self.owned_native_radio(node)) {
            let bytes = value.map_or(2,str::len);
            self.state.work(self.forms.default_scan_work().saturating_add(bytes).saturating_add(self.forms.controls.len()))?;
            // Raw initial text, the four-byte editor, and the bounded caption
            // are prepared privately before the attribute's own preflight.
            self.state.copies(bytes.checked_mul(6).ok_or("Checkable INPUT value copy accounting overflow.")?)?;
            return self.forms.prepare_checkable_value(node,value).map(Some);
        }
        if !(self.native_text_input(node) || name.html_is("name") && (self.owned_native_checkbox(node) || self.owned_native_radio(node))) { return Ok(None); }
        let scanned=self.forms.default_scan_work();
        let input=value.map_or(0,str::len);
        self.state.work(scanned.saturating_add(input).saturating_add(self.forms.controls.len()))?;
        // One raw default plus at most one four-byte-per-character editor;
        // charge this upper bound before prepare makes the first allocation.
        self.state.copies(input.checked_mul(5).ok_or("Text INPUT default copy accounting overflow.")?)?;
        let plan=if name.html_is("name") { self.forms.prepare_input_name(node,value)? }
            else { forms::InputPlan::Default(self.forms.prepare_text_default(node,value)?) };
        // Native values have their own finite form payload allowance. DOM
        // retained-byte accounting continues to cover the content attribute;
        // native bytes that were never charged are never subtracted from it.
        Ok(Some(plan))
    }
}

#[cfg(test)]
#[path="script_text_input_tests.rs"]
mod tests;

#[cfg(test)]
#[path="script_created_text_input_tests.rs"]
mod created_tests;
