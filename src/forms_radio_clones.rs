//! Radio state is copied privately into the existing native clone transaction.
//! https://html.spec.whatwg.org/multipage/input.html#the-input-element
use super::*;

impl Forms {
    pub(crate) fn add_radio_clones(
        &self,
        doc: &Document,
        sources: &[(usize, usize)],
        clone_root: usize,
        plan: &mut SelectFormPlan,
        retained_limit: usize,
        mut charge: impl FnMut(usize, usize) -> Result<(), String>,
    ) -> Result<(), String> {
        if clone_root < doc.nodes.len()
            || self
                .controls
                .len()
                .saturating_add(plan.additions.len())
                .saturating_add(sources.len())
                > MAX_CONTROLS
        {
            return Err("Native form control limit or clone root identity is invalid.".into());
        }
        charge(self.default_scan_work(), 0)?;
        let mut payload = self.text_payload(None)?;
        for control in plan.additions.values() {
            charge(control.value.chars.len().saturating_add(1), 0)?;
            payload = payload
                .checked_add(text_inputs::control_payload(control)?)
                .ok_or("Native clone form payload overflow.")?;
        }
        for &(source, destination) in sources {
            let item = doc
                .nodes
                .get(source)
                .ok_or("Radio clone source is unavailable.")?;
            let NodeKind::Element { attrs, .. } = &item.kind else {
                return Err("Radio clone source must be an HTML INPUT.".into());
            };
            // Bound repeated native attribute lookups before inspecting the
            // shape or assembling metadata. No author getters are involved.
            charge(attrs.len().saturating_mul(16).saturating_add(1), 0)?;
            let original = self
                .controls
                .get(&source)
                .filter(|c| c.kind == Kind::Radio && native_radio_input(doc, source))
                .ok_or("Cloning requires an existing native HTML radio INPUT.")?;
            if destination < clone_root
                || self.controls.contains_key(&destination)
                || plan.additions.contains_key(&destination)
            {
                return Err(
                    "Radio clone native identity is already owned or outside its cloned tree."
                        .into(),
                );
            }
            let name = doc.attr(source, "name").unwrap_or("");
            let initial = doc.attr(source, "value").unwrap_or("on");
            let caption = doc.attr(source, "value").unwrap_or("");
            let editor_bytes = original
                .value
                .chars
                .len()
                .checked_mul(std::mem::size_of::<char>())
                .ok_or("Radio clone editor size overflow.")?;
            charge(
                name.len()
                    .saturating_add(initial.len().saturating_mul(2))
                    .saturating_add(editor_bytes)
                    .saturating_add(caption.len())
                    .saturating_add(doc.attr(source, "maxlength").map_or(0, str::len))
                    .saturating_add(doc.attr(source, "tabindex").map_or(0, str::len)),
                0,
            )?;
            if name.len() > MAX_QUERY || initial.encode_utf16().count() > MAX_VALUE_UNITS {
                return Err("Radio clone name/default limit reached.".into());
            }
            let (units, live_bytes) =
                original
                    .value
                    .chars
                    .iter()
                    .fold((0usize, 0usize), |(units, bytes), ch| {
                        (
                            units.saturating_add(ch.len_utf16()),
                            bytes.saturating_add(ch.len_utf8()),
                        )
                    });
            if units > MAX_VALUE_UNITS {
                return Err("Radio clone live value exceeds the native limit.".into());
            }
            payload = payload
                .checked_add(name.len())
                .and_then(|bytes| bytes.checked_add(initial.len().max(live_bytes)))
                .ok_or("Radio clone form payload overflow.")?;
            if payload > MAX_TOTAL_VALUE_BYTES {
                return Err(
                    "Combined form names, values and defaults exceed 128 KiB during cloning."
                        .into(),
                );
            }
            let caption_bytes = caption.chars().take(256).map(char::len_utf8).sum::<usize>();
            let retained = name
                .len()
                .checked_add(initial.len())
                .and_then(|bytes| bytes.checked_add(caption_bytes))
                .and_then(|bytes| bytes.checked_add(editor_bytes))
                .ok_or("Radio clone retained payload overflow.")?;
            let total = plan
                .retained_bytes
                .checked_add(retained)
                .ok_or("Native clone retained payload overflow.")?;
            if total > retained_limit {
                return Err(
                    "DOM retained text/attribute growth limit reached during radio cloning.".into(),
                );
            }
            charge(0, retained)?;
            let mut initial_copy = String::new();
            initial_copy
                .try_reserve(initial.len())
                .map_err(|_| "Radio clone default reservation failed.")?;
            initial_copy.push_str(initial);
            let mut value = Editor::default();
            value
                .chars
                .try_reserve(original.value.chars.len())
                .map_err(|_| "Radio clone editor reservation failed.")?;
            value.chars.extend_from_slice(&original.value.chars);
            value.cursor = value.chars.len();
            value.anchor = value.cursor;
            let (mut control, actual_retained) =
                select_fragments::fragment_editor_control_with_value(
                    doc,
                    source,
                    Kind::Radio,
                    name,
                    initial_copy,
                    value,
                    original.dirty_value,
                )?;
            debug_assert_eq!(actual_retained, retained);
            control.checked = original.checked;
            control.dirty_checked = original.dirty_checked;
            control.group_root = clone_root;
            // Metadata assembly uses only this copied radio's own attributes:
            // no owner or disabled-fieldset state leaks from the source tree.
            plan.additions.insert(destination, control);
            plan.retained_bytes = total;
        }
        Ok(())
    }
}
