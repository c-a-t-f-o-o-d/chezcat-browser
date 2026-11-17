//! Staged hidden-input state. Existing native controls are never rebuilt.
use super::*;

pub(crate) struct HiddenInputPlan {
    controls: BTreeMap<usize, Control>,
    pub(crate) copy_bytes: usize,
}
impl HiddenInputPlan {
    pub(crate) fn commit(mut self, forms: &mut Forms) {
        forms.controls.append(&mut self.controls);
    }
}

impl Forms {
    pub(crate) fn prepare_hidden_input(&self, doc: &Document, node: usize,
        attribute: &str, value: Option<&str>) -> Result<HiddenInputPlan, String> {
        if self.limited { return Err("Native form limits already exceeded; hidden input was not changed.".into()); }
        let existing = self.controls.get(&node);
        if existing.is_some_and(|control| control.kind != Kind::Hidden) {
            return Err("Changing an existing native input type is not implemented; no input was changed.".into());
        }
        if existing.is_none() && self.controls.len() >= MAX_CONTROLS {
            return Err("Native form control limit reached before hidden input admission.".into());
        }
        let name = if attribute == "name" { value.unwrap_or("") } else { doc.attr(node,"name").unwrap_or("") };
        let text = if attribute == "value" { value.unwrap_or("") } else { doc.attr(node,"value").unwrap_or("") };
        if name.len() > MAX_QUERY || text.encode_utf16().count() > MAX_VALUE_UNITS {
            return Err("Hidden input name/value limit reached; no input was changed.".into());
        }
        let mut total = name.len().checked_add(text.len()).ok_or("Hidden input payload overflow.")?;
        for (&other, control) in &self.controls {
            if other == node { continue; }
            let options = control.select.as_ref().map_or(0, |select|
                select.options.iter().map(|option| option.value.len() + option.label.len()).sum::<usize>()
                + select.rows.iter().map(|row| match row { SelectRow::Group { label, .. } => label.len(), _ => 0 }).sum::<usize>());
            total = total.checked_add(control.name.len())
                .and_then(|total| total.checked_add(control.value.chars.iter().map(|ch| ch.len_utf8()).sum::<usize>()))
                .and_then(|total| total.checked_add(options)).ok_or("Hidden input payload overflow.")?;
        }
        if total > MAX_TOTAL_VALUE_BYTES { return Err("Combined form names and values exceed 128 KiB; no input was changed.".into()); }
        let mut editor = Editor::default();
        set_form_value(&mut editor, text, Kind::Hidden);
        let copy_bytes = name.len().checked_add(text.len()).and_then(|bytes|
            bytes.checked_add(editor.chars.len().saturating_mul(std::mem::size_of::<char>())))
            .ok_or("Hidden input copy size overflow.")?;
        let control = Control {
            owner: existing.and_then(|control| control.owner), kind: Kind::Hidden,
            value: editor, dirty_value: false, name: name.into(), caption: String::new(),
            disabled: existing.map_or(doc.attr(node,"disabled").is_some(), |control| control.disabled),
            readonly: doc.attr(node,"readonly").is_some(), required: doc.attr(node,"required").is_some(),
            checked: existing.is_some_and(|control| control.checked),
            max_length: existing.map_or(MAX_VALUE_UNITS, |control| control.max_length),
            tab_index: existing.map_or(0, |control| control.tab_index), select: None,
            initial: text.into(), initially_checked: existing.is_some_and(|control| control.initially_checked),
            image_point: (0,0),
        };
        let mut controls = BTreeMap::new();
        controls.insert(node, control);
        Ok(HiddenInputPlan { controls, copy_bytes })
    }
}
