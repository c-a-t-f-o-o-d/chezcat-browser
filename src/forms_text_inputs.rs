//! Prepared native INPUT admission and reflection. Existing editors are
//! updated in place; DOM callers publish these plans only after preflight.
use super::*;

pub(crate) enum InputPlan {
    Admit { controls: BTreeMap<usize, Control>, retained_bytes: usize },
    Name { node: usize, name: String },
    RadioName { node: usize, name: String, uncheck: Vec<usize> },
    Live { node: usize, value: Option<Editor> },
    CheckableValue { node: usize, initial: String, value: Editor, caption: String },
    Default(TextDefaultPlan),
    Checked { node: usize, present: bool, synchronize: bool },
}
impl InputPlan {
    pub(crate) fn retained_bytes(&self) -> usize {
        match self { Self::Admit { retained_bytes, .. } => *retained_bytes, _ => 0 }
    }
    pub(crate) fn commit(self, forms: &mut Forms) {
        match self {
            Self::Admit { mut controls, .. } => forms.controls.append(&mut controls),
            Self::Name { node, name } => forms.controls.get_mut(&node).expect("Prepared INPUT").name = name,
            Self::RadioName { node, name, uncheck } => {
                forms.controls.get_mut(&node).expect("Prepared radio INPUT").name = name;
                for other in uncheck { forms.controls.get_mut(&other).expect("Prepared radio peer").checked = false; }
            }
            Self::Live { node, value } => {
                let control = forms.controls.get_mut(&node).expect("Prepared text INPUT");
                if let Some(value) = value { control.value = value; }
                control.dirty_value = true;
            }
            Self::CheckableValue { node, initial, value, caption } => {
                let control = forms.controls.get_mut(&node).expect("Prepared checkable INPUT");
                control.initial = initial;
                control.value = value;
                control.caption = caption;
            }
            Self::Default(plan) => plan.commit(forms),
            Self::Checked { node, present, synchronize } => {
                let control = forms.controls.get_mut(&node).expect("Prepared INPUT checkedness");
                control.initially_checked = present;
                if synchronize && !control.dirty_checked {
                    control.checked = present;
                    if present && control.kind == Kind::Radio { forms.check_radio(node); }
                }
            }
        }
    }

    pub(crate) fn set_created_checked(&mut self, checked: bool) -> Result<(), String> {
        let Self::Admit { controls, .. } = self else {
            return Err("Checkedness admission requires a prepared new INPUT.".into());
        };
        if controls.len() != 1 { return Err("Checkedness admission requires exactly one new INPUT.".into()); }
        let control = controls.values_mut().next().unwrap();
        control.checked = checked;
        control.dirty_checked = true;
        Ok(())
    }
}

fn copy_text(text: &str) -> Result<String, String> {
    let mut copy = String::new();
    copy.try_reserve(text.len()).map_err(|_| "Text INPUT string reservation failed.")?;
    copy.push_str(text);
    Ok(copy)
}
fn text_editor(text: &str) -> Result<Editor, String> {
    let count = text.chars().filter(|ch| !matches!(ch, '\r' | '\n')).count();
    let mut value = Editor::default();
    value.chars.try_reserve(count).map_err(|_| "Text INPUT editor reservation failed.")?;
    value.chars.extend(text.chars().filter(|ch| !matches!(ch, '\r' | '\n')));
    value.cursor = value.chars.len();
    value.anchor = value.cursor;
    Ok(value)
}

/// Shared constructor for parsed fragments and fresh created inputs. The
/// bridge separately validates prospective ancestry and created-node ownership.
pub(super) fn text_control(doc: &Document, node: usize, name: &str, initial: &str,
    live: Option<&str>) -> Result<(Control, usize), String> {
    if !fragment_text_input(doc, node) || doc.attr(node, "form").is_some() || !doc.nodes[node].children.is_empty() {
        return Err("Text INPUT admission requires an empty HTML default/text input without explicit form association.".into());
    }
    let text = live.unwrap_or(initial);
    if name.len() > MAX_QUERY || initial.encode_utf16().count() > MAX_VALUE_UNITS
        || text.chars().filter(|ch| !matches!(ch, '\r' | '\n')).map(char::len_utf16).sum::<usize>() > MAX_VALUE_UNITS {
        return Err("Text INPUT name/default/value limit reached; no control was admitted.".into());
    }
    let value = text_editor(text)?;
    let caption: String = initial.chars().take(256).collect();
    let retained = name.len().checked_add(initial.len()).and_then(|bytes| bytes.checked_add(caption.len()))
        .and_then(|bytes| bytes.checked_add(value.chars.len().checked_mul(std::mem::size_of::<char>())?))
        .ok_or("Text INPUT retained payload overflow.")?;
    Ok((Control { owner: None, group_root: node, kind: Kind::Text, value, dirty_value: live.is_some(),
        name: copy_text(name)?, caption, disabled: doc.attr(node, "disabled").is_some(),
        readonly: doc.attr(node, "readonly").is_some(), required: doc.attr(node, "required").is_some(),
        checked: doc.attr(node, "checked").is_some(), dirty_checked: false,
        max_length: doc.attr(node, "maxlength").and_then(|value| value.parse().ok()).unwrap_or(MAX_VALUE_UNITS).min(MAX_VALUE_UNITS),
        tab_index: doc.attr(node, "tabindex").and_then(parse_tab_index).unwrap_or(0), select: None,
        button_element: false, initial: copy_text(initial)?, initially_checked: doc.attr(node, "checked").is_some(), image_point: (0, 0) }, retained))
}

pub(super) fn control_payload(control: &Control) -> Result<usize, String> {
    let live = control.value.chars.iter().map(|ch| ch.len_utf8()).sum::<usize>();
    let mut bytes = control.name.len().checked_add(control.initial.len().max(live)).ok_or("Native form payload overflow.")?;
    if let Some(select) = &control.select {
        for option in &select.options {
            bytes = bytes.checked_add(option.value.len()).and_then(|bytes| bytes.checked_add(option.label.len()))
                .ok_or("Native option payload overflow.")?;
        }
        for row in &select.rows { if let SelectRow::Group { label, .. } = row {
            bytes = bytes.checked_add(label.len()).ok_or("Native group payload overflow.")?;
        } }
    }
    Ok(bytes)
}

impl Forms {
    // Prepared mutations supply both default and live byte lengths. Replacing
    // an old default must not leave its larger length in the aggregate bound.
    pub(super) fn text_payload(&self, changed: Option<(usize, &str, usize, usize)>) -> Result<usize, String> {
        if self.limited || self.controls.len() > MAX_CONTROLS {
            return Err("Native form limits already exceeded; no text INPUT was changed.".into());
        }
        let mut bytes = 0usize;
        for (&node, control) in &self.controls {
            let payload = if let Some((_, name, initial, live)) = changed.filter(|(changed, _, _, _)| *changed == node) {
                if name.len() > MAX_QUERY { return Err("A text INPUT name is limited to 8192 bytes.".into()); }
                name.len().checked_add(initial.max(live)).ok_or("Text INPUT payload overflow.")?
            } else {
                if control.name.len() > MAX_QUERY { return Err("A form control name exceeds 8192 bytes.".into()); }
                control_payload(control)?
            };
            bytes = bytes.checked_add(payload).ok_or("Native form payload overflow.")?;
            if bytes > MAX_TOTAL_VALUE_BYTES { return Err("Combined form names, values and defaults exceed 128 KiB.".into()); }
        }
        Ok(bytes)
    }

    pub(crate) fn prepare_input_name(&self, node: usize, name: Option<&str>) -> Result<InputPlan, String> {
        let control = self.controls.get(&node).filter(|control| matches!(control.kind, Kind::Text | Kind::Checkbox | Kind::Radio))
            .ok_or("Name update requires a native text or checkable INPUT.")?;
        let name = name.unwrap_or("");
        let live = control.value.chars.iter().map(|ch| ch.len_utf8()).sum();
        self.text_payload(Some((node, name, control.initial.len(), live)))?;
        if control.kind == Kind::Radio {
            let mut uncheck = Vec::new();
            if control.checked && !name.is_empty() && control.name != name {
                uncheck.try_reserve(self.controls.len()).map_err(|_| "Radio name group reservation failed.")?;
                uncheck.extend(self.controls.iter().filter_map(|(&other,c)|
                    (other != node && c.kind == Kind::Radio && c.group_root == control.group_root
                        && c.owner == control.owner && c.name == name).then_some(other)));
            }
            return Ok(InputPlan::RadioName { node, name: copy_text(name)?, uncheck });
        }
        Ok(InputPlan::Name { node, name: copy_text(name)? })
    }

    pub(crate) fn prepare_text_live(&self, node: usize, text: &str) -> Result<InputPlan, String> {
        let control = self.controls.get(&node).filter(|control| control.kind == Kind::Text)
            .ok_or("Value update requires a native text INPUT.")?;
        let mut units = 0usize;
        let mut bytes = 0usize;
        for ch in text.chars().filter(|ch| !matches!(ch, '\r' | '\n')) {
            units = units.saturating_add(ch.len_utf16()); bytes = bytes.saturating_add(ch.len_utf8());
        }
        if units > MAX_VALUE_UNITS { return Err("A form value is limited to 4096 UTF-16 units.".into()); }
        self.text_payload(Some((node, &control.name, control.initial.len(), bytes)))?;
        let same = control.value.chars.iter().copied().eq(text.chars().filter(|ch| !matches!(ch, '\r' | '\n')));
        Ok(InputPlan::Live { node, value: if same { None } else { Some(text_editor(text)?) } })
    }

    /// Radio and checkbox values use default/on mode: both IDL reflections
    /// update the raw attribute, without changing either dirty flag or checkedness.
    pub(crate) fn prepare_checkable_value(&self, node: usize, attribute: Option<&str>) -> Result<InputPlan, String> {
        let control = self.controls.get(&node).filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
            .ok_or("Default/on value update requires a native radio or checkbox INPUT.")?;
        let text = attribute.unwrap_or("on");
        if text.encode_utf16().count() > MAX_VALUE_UNITS {
            return Err("A checkable INPUT value is limited to 4096 UTF-16 units.".into());
        }
        self.text_payload(Some((node, &control.name, text.len(), text.len())))?;
        let initial = copy_text(text)?;
        let mut value = Editor::default();
        value.chars.try_reserve(text.chars().count()).map_err(|_| "Checkable INPUT editor reservation failed.")?;
        value.chars.extend(text.chars());
        value.cursor = value.chars.len();
        value.anchor = value.cursor;
        let caption_text = attribute.unwrap_or("");
        let caption_bytes = caption_text.chars().take(256).map(char::len_utf8).sum();
        let mut caption = String::new();
        caption.try_reserve(caption_bytes).map_err(|_| "Checkable INPUT caption reservation failed.")?;
        caption.extend(caption_text.chars().take(256));
        Ok(InputPlan::CheckableValue { node, initial, value, caption })
    }

    pub(crate) fn prepare_created_input(&self, doc: &Document, node: usize,
        kind: Kind, name: &str, initial: &str, live: Option<&str>) -> Result<InputPlan, String> {
        if self.controls.contains_key(&node) || self.controls.len() >= MAX_CONTROLS {
            return Err("Native form control limit reached or INPUT identity already admitted.".into());
        }
        let bytes = self.text_payload(None)?;
        let (control, retained_bytes) = match kind {
            Kind::Checkbox | Kind::Radio if live.is_none() => select_fragments::fragment_editor_control(doc, node, kind, name, initial)?,
            Kind::Text => text_control(doc, node, name, initial, live)?,
            _ => return Err("Unsupported created INPUT admission.".into()),
        };
        if bytes.checked_add(control_payload(&control)?).is_none_or(|bytes| bytes > MAX_TOTAL_VALUE_BYTES) {
            return Err("Combined form names, values and defaults exceed 128 KiB.".into());
        }
        let mut controls = BTreeMap::new(); controls.insert(node, control);
        Ok(InputPlan::Admit { controls, retained_bytes })
    }

    pub(crate) fn add_created_text_inputs(&self, doc: &Document, nodes: &[usize], plan: &mut SelectFormPlan) -> Result<(), String> {
        if self.controls.len().saturating_add(plan.additions.len()).saturating_add(nodes.len()) > MAX_CONTROLS {
            return Err("Native form control limit reached before text INPUT insertion.".into());
        }
        let mut bytes = self.text_payload(None)?;
        for control in plan.additions.values() { bytes = bytes.checked_add(control_payload(control)?).ok_or("Native form payload overflow.")?; }
        for &node in nodes {
            if self.controls.contains_key(&node) || plan.additions.contains_key(&node) { return Err("Text INPUT native identity is already owned.".into()); }
            let (control, retained) = text_control(doc, node, doc.attr(node, "name").unwrap_or(""), doc.attr(node, "value").unwrap_or(""), None)?;
            bytes = bytes.checked_add(control_payload(&control)?).ok_or("Native form payload overflow.")?;
            if bytes > MAX_TOTAL_VALUE_BYTES { return Err("Combined form names, values and defaults exceed 128 KiB.".into()); }
            plan.retained_bytes = plan.retained_bytes.checked_add(retained).ok_or("Text INPUT retained payload overflow.")?;
            plan.additions.insert(node, control);
        }
        plan.metadata.try_reserve(nodes.len()).map_err(|_| "Text INPUT owner plan reservation failed.")?;
        Ok(())
    }
}
