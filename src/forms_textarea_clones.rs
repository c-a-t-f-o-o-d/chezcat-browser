//! Native textarea copies remain private until the detached DOM plan commits.
use super::*;

impl Forms {
    pub(crate) fn prepare_textarea_clones(
        &self,
        doc: &Document,
        sources: &[(usize, usize)],
        deep: bool,
        retained_limit: usize,
        mut charge: impl FnMut(usize, usize) -> Result<(), String>,
    ) -> Result<SelectFormPlan, String> {
        if self.controls.len().saturating_add(sources.len()) > MAX_CONTROLS {
            return Err("Native form control limit reached before textarea cloning.".into());
        }
        charge(self.default_scan_work(), 0)?;
        let mut payload = self.text_payload(None)?;
        let mut plan = SelectFormPlan {
            additions: BTreeMap::new(),
            metadata: Vec::new(),
            radio_checked: Vec::new(),
            retained_bytes: 0,
        };
        for &(source, destination) in sources {
            let original = self
                .controls
                .get(&source)
                .filter(|control| control.kind == Kind::TextArea)
                .ok_or("Cloning requires an existing native HTML textarea.")?;
            let child_text = textarea_content(doc, source)
                .ok_or("Cloning this textarea child shape is not implemented.")?;
            if destination < doc.nodes.len()
                || self.controls.contains_key(&destination)
                || plan.additions.contains_key(&destination)
            {
                return Err("Textarea clone native identity is already owned.".into());
            }
            let raw_default = if deep { child_text } else { "" };
            let name = doc.attr(source, "name").unwrap_or("");
            let caption = doc.attr(source, "value").unwrap_or("");
            let scan = raw_default
                .len()
                .saturating_mul(2)
                .saturating_add(
                    original
                        .value
                        .chars
                        .len()
                        .saturating_mul(std::mem::size_of::<char>()),
                )
                .saturating_add(name.len())
                .saturating_add(caption.len())
                .saturating_add(doc.attr(source, "maxlength").map_or(0, str::len))
                .saturating_add(doc.attr(source, "tabindex").map_or(0, str::len))
                .saturating_add(1);
            charge(scan, 0)?;
            if name.len() > MAX_QUERY || raw_default.encode_utf16().count() > MAX_VALUE_UNITS {
                return Err("Textarea clone name/default limit reached.".into());
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
                return Err("Textarea clone live value exceeds the native editor limit.".into());
            }
            let initial_bytes = normalized_chars(raw_default)
                .map(char::len_utf8)
                .sum::<usize>();
            payload = payload
                .checked_add(name.len())
                .and_then(|bytes| bytes.checked_add(initial_bytes.max(live_bytes)))
                .ok_or("Textarea clone form payload overflow.")?;
            if payload > MAX_TOTAL_VALUE_BYTES {
                return Err(
                    "Combined form names, values and defaults exceed 128 KiB during cloning."
                        .into(),
                );
            }
            let caption_bytes = caption.chars().take(256).map(char::len_utf8).sum::<usize>();
            let editor_bytes = original
                .value
                .chars
                .len()
                .checked_mul(std::mem::size_of::<char>())
                .ok_or("Textarea clone editor size overflow.")?;
            let retained = name
                .len()
                .checked_add(initial_bytes)
                .and_then(|bytes| bytes.checked_add(caption_bytes))
                .and_then(|bytes| bytes.checked_add(editor_bytes))
                .ok_or("Textarea clone retained payload overflow.")?;
            plan.retained_bytes = plan
                .retained_bytes
                .checked_add(retained)
                .ok_or("Textarea clone retained payload overflow.")?;
            if plan.retained_bytes > retained_limit {
                return Err(
                    "DOM retained text/attribute growth limit reached during textarea cloning."
                        .into(),
                );
            }
            charge(0, retained)?;

            let mut initial = String::new();
            initial
                .try_reserve(initial_bytes)
                .map_err(|_| "Textarea clone default reservation failed.")?;
            initial.extend(normalized_chars(raw_default));
            let mut value = Editor::default();
            value
                .chars
                .try_reserve(original.value.chars.len())
                .map_err(|_| "Textarea clone editor reservation failed.")?;
            value.chars.extend_from_slice(&original.value.chars);
            value.cursor = value.chars.len();
            value.anchor = value.cursor;
            let (control, actual_retained) = select_fragments::fragment_editor_control_with_value(
                doc,
                source,
                Kind::TextArea,
                name,
                initial,
                value,
                original.dirty_value,
            )?;
            debug_assert_eq!(actual_retained, retained);
            plan.additions.insert(destination, control);
        }
        Ok(plan)
    }
}
