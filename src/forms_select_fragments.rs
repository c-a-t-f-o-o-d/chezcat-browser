//! New select controls are prepared independently; existing editors/selections
//! are never reconstructed. Removed controls retain native identity and values.
use super::*;

pub(crate) struct SelectFormPlan {
    pub(crate) additions: BTreeMap<usize, Control>,
    pub(crate) metadata: Vec<(usize, Option<usize>, bool)>,
    pub(crate) retained_bytes: usize,
}
impl SelectFormPlan {
    pub(crate) fn commit(mut self, forms: &mut Forms) {
        // BTreeMap has no fallible reserve API. New entries were constructed in
        // the private plan; append/update has no recoverable Result or reentry.
        forms.controls.append(&mut self.additions);
        for (node, owner, disabled) in self.metadata {
            let control = forms.controls.get_mut(&node).expect("Prepared select control");
            control.owner = owner;
            control.disabled = disabled;
        }
    }
}
fn select_bytes(control: &Control) -> usize {
    control.select.as_ref().map_or(0, |select| {
        select.options.iter().map(|option| option.value.len() + option.label.len()).sum::<usize>()
            + select.rows.iter().map(|row| match row { SelectRow::Group { label, .. } => label.len(), _ => 0 }).sum::<usize>()
    })
}
impl Forms {
    pub(crate) fn prepare_selects(&self, fragment: Option<(&Document, usize)>) -> Result<SelectFormPlan, String> {
        if self.limited { return Err("Native form limits already exceeded; no select was admitted.".into()); }
        let mut plan = SelectFormPlan { additions: BTreeMap::new(), metadata: Vec::new(), retained_bytes: 0 };
        let mut options: usize = self.controls.values().filter_map(|control| control.select.as_ref()).map(|select| select.options.len()).sum();
        let mut bytes: usize = self.controls.values().map(|control| control.name.len()
            + control.value.chars.iter().map(|ch| ch.len_utf8()).sum::<usize>() + select_bytes(control)).sum();
        if let Some((doc, base)) = fragment {
            if base == 0 || base.checked_add(doc.nodes.len().saturating_sub(1)).is_none() { return Err("Select native identity range overflow.".into()); }
            for local in 1..doc.nodes.len() {
                if tag(doc, local) != "select" { continue; }
                if doc.nodes[local].namespace != crate::html::Namespace::Html || kind(doc, local) != Some(Kind::Select) {
                    return Err("Dynamic select currently requires a conventional single-select control.".into());
                }
                // Explicit form reassociation depends on prospective document
                // ID order; keep this separate rather than guess an owner.
                if doc.attr(local, "form").is_some() {
                    return Err("Explicit form association on a new fragment select is not implemented.".into());
                }
                if self.controls.len().saturating_add(plan.additions.len()) >= MAX_CONTROLS {
                    return Err("Native form control limit reached before select admission.".into());
                }
                let (mut selected, limited, payload) = select_state(doc, local);
                let name = doc.attr(local, "name").unwrap_or("");
                options = options.checked_add(selected.options.len()).ok_or("Select option count overflow.")?;
                bytes = bytes.checked_add(payload).and_then(|bytes| bytes.checked_add(name.len())).ok_or("Select payload accounting overflow.")?;
                if limited || options > MAX_TOTAL_OPTIONS || name.len() > MAX_QUERY || bytes > MAX_TOTAL_VALUE_BYTES {
                    return Err("Dynamic select value, option or form payload limit reached.".into());
                }
                for option in &mut selected.options {
                    option.node += base - 1;
                    option.group = option.group.map(|node| node + base - 1);
                }
                for row in &mut selected.rows { if let SelectRow::Group { node, .. } = row { *node += base - 1; } }
                let mut control = Control { owner: None, kind: Kind::Select, value: Editor::default(), name: name.into(),
                    caption: String::new(), disabled: doc.attr(local, "disabled").is_some(), readonly: false,
                    required: doc.attr(local, "required").is_some(), checked: false, max_length: MAX_VALUE_UNITS,
                    tab_index: doc.attr(local, "tabindex").and_then(parse_tab_index).unwrap_or(0),
                    select: Some(selected), initial: String::new(), initially_checked: false, image_point: (0, 0) };
                let initial = control.select.as_ref().unwrap().selected;
                control.set_selection(initial);
                let extra = name.len().checked_add(select_bytes(&control)).and_then(|bytes| bytes.checked_add(control.caption.len()))
                    .and_then(|bytes| bytes.checked_add(control.value.chars.len().saturating_mul(std::mem::size_of::<char>())))
                    .ok_or("Select retained payload overflow.")?;
                plan.retained_bytes = plan.retained_bytes.checked_add(extra).ok_or("Select retained payload overflow.")?;
                plan.additions.insert(base + local - 1, control);
            }
        }
        plan.metadata.try_reserve(self.controls.len().saturating_add(plan.additions.len())).map_err(|_| "Select metadata reservation failed.")?;
        Ok(plan)
    }

    // Arena IDs describe allocation, not current tree order. Traversing only
    // the connected document also excludes saved detached controls from submit.
    pub(super) fn submission_order(&self, doc: &Document) -> Result<Vec<usize>, String> {
        let mut order = Vec::new();
        order.try_reserve(self.controls.len()).map_err(|_| "Submission order reservation failed.")?;
        let mut seen = Vec::new();
        seen.try_reserve(doc.nodes.len()).map_err(|_| "Submission topology reservation failed.")?;
        seen.resize(doc.nodes.len(), false);
        let mut stack = Vec::new();
        stack.try_reserve(doc.nodes.len()).map_err(|_| "Submission walk reservation failed.")?;
        if !doc.nodes.is_empty() { stack.push(0); }
        while let Some(node) = stack.pop() {
            if node >= doc.nodes.len() || seen[node] { return Err("Submission topology is invalid; nothing was submitted.".into()); }
            seen[node] = true;
            if self.controls.contains_key(&node) { order.push(node); }
            stack.extend(doc.nodes[node].children.iter().rev().copied());
        }
        Ok(order)
    }
}

#[cfg(test)]
#[path = "forms_select_fragment_tests.rs"]
mod tests;
