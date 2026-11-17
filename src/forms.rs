//! Small, explicit HTML form model. Submission is user-initiated HTTP GET or
//! URL-encoded POST. Layout owns rectangles; this module owns submitted values.
//! This module owns values; layout only owns rectangles. Script writes use the
//! same bounded model. There is no autofill or persisted form state.
use std::collections::{BTreeMap, HashMap};

use crate::editor::Editor;
use crate::html::{Document, NodeKind};
use crate::navigation::{self, Location};

pub const MAX_CONTROLS: usize = 512;
pub const MAX_VALUE_UNITS: usize = 4096;
const MAX_TOTAL_VALUE_BYTES: usize = 128 * 1024;
const MAX_QUERY: usize = 8192;
pub const MAX_SELECT_OPTIONS: usize = 512;
const MAX_TOTAL_OPTIONS: usize = 4096;
const MAX_SELECT_ROWS: usize = 1024;
const MAX_SELECT_WALK: usize = 8192;

#[derive(Debug)]
pub struct Submission {
    pub location: Location,
    pub request: crate::http::Request,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    TextArea,
    Hidden,
    Checkbox,
    Radio,
    Submit,
    Image,
    Select,
    Reset,
    Button,
    Unsupported,
}

pub fn kind(doc: &Document, node: usize) -> Option<Kind> {
    let tag = tag(doc, node);
    Some(match tag {
        "input" => match doc
            .attr(node, "type")
            .unwrap_or("text")
            .to_ascii_lowercase()
            .as_str()
        {
            "hidden" => Kind::Hidden,
            "checkbox" => Kind::Checkbox,
            "radio" => Kind::Radio,
            "submit" => Kind::Submit,
            "image" => Kind::Image,
            "reset" => Kind::Reset,
            "button" => Kind::Button,
            "password" | "file" | "number" | "range" | "color" | "date" | "datetime-local"
            | "month" | "week" | "time" | "email" | "url" => Kind::Unsupported,
            _ => Kind::Text, // text/search/tel, plus the HTML invalid-value default.
        },
        "button" => match doc
            .attr(node, "type")
            .unwrap_or("submit")
            .to_ascii_lowercase()
            .as_str()
        {
            "reset" => Kind::Reset,
            "button" => Kind::Button,
            _ => Kind::Submit,
        },
        "select"
            if doc.attr(node, "multiple").is_none()
                && doc
                    .attr(node, "size")
                    .and_then(|value| value.trim().parse::<u32>().ok())
                    .unwrap_or(1)
                    <= 1 =>
        {
            Kind::Select
        }
        "textarea" => Kind::TextArea,
        "select" => Kind::Unsupported,
        _ => return None,
    })
}

fn tag(doc: &Document, node: usize) -> &str {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag,
        NodeKind::Text(_) | NodeKind::Comment(_) | NodeKind::DocumentFragment => "",
    }
}

/// HTML integer-prefix parsing restricted to the signed WebIDL long range.
/// Attribute GET parsing does not wrap out-of-range values like an IDL setter.
pub(crate) fn parse_tab_index(input: &str) -> Option<i32> {
    let bytes = input.as_bytes();
    let mut position = 0;
    while bytes.get(position).is_some_and(|byte| matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')) { position += 1; }
    let negative = bytes.get(position) == Some(&b'-');
    if matches!(bytes.get(position), Some(b'+' | b'-')) { position += 1; }
    let first = position;
    let limit = if negative { 2_147_483_648u64 } else { 2_147_483_647u64 };
    let mut number = 0u64;
    while let Some(byte) = bytes.get(position).filter(|byte| byte.is_ascii_digit()) {
        number = number * 10 + u64::from(byte - b'0');
        if number > limit { return None; }
        position += 1;
    }
    if position == first { return None; }
    Some(if negative { -(number as i64) } else { number as i64 } as i32)
}

pub fn caption(doc: &Document, node: usize, kind: Kind) -> String {
    if kind == Kind::Select {
        return select_state(doc, node)
            .0
            .options
            .into_iter()
            .map(|option| option.label)
            .max_by_key(|label| label.chars().count())
            .unwrap_or_default();
    }
    if kind == Kind::Image {
        return doc
            .attr(node, "alt")
            .filter(|text| !text.trim().is_empty())
            .unwrap_or("Submit")
            .chars()
            .take(256)
            .collect();
    }
    if tag(doc, node) == "button" {
        let mut text = String::new();
        let mut stack: Vec<_> = doc.nodes[node].children.iter().rev().copied().collect();
        while let Some(child) = stack.pop() {
            match &doc.nodes[child].kind {
                NodeKind::Comment(_) => {},
                NodeKind::DocumentFragment => stack.extend(doc.nodes[child].children.iter().rev().copied()),
                NodeKind::Text(value) => {
                    for ch in value.chars().take(256 - text.chars().count().min(256)) {
                        text.push(ch);
                    }
                }
                NodeKind::Element { tag, .. }
                    if !matches!(tag.as_str(), "script" | "style" | "template")
                        && !(doc.scripting_enabled && tag == "noscript") =>
                {
                    stack.extend(doc.nodes[child].children.iter().rev());
                }
                _ => {}
            }
            if text.chars().count() >= 256 {
                break;
            }
        }
        return text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    }
    if kind == Kind::Unsupported {
        return format!(
            "{} unavailable",
            doc.attr(node, "type").unwrap_or(tag(doc, node))
        );
    }
    doc.attr(node, "value")
        .unwrap_or(match kind {
            Kind::Submit => "Submit",
            Kind::Reset => "Reset",
            _ => "",
        })
        .chars()
        .take(256)
        .collect()
}

#[derive(Debug)]
pub struct SelectOption {
    pub node: usize,
    pub label: String,
    pub value: String,
    pub disabled: bool,
    pub group: Option<usize>,
    placeholder: bool,
}

#[derive(Debug)]
pub enum SelectRow {
    Group { node: usize, label: String },
    Option(usize),
}

#[derive(Debug)]
pub struct SelectState {
    pub options: Vec<SelectOption>,
    pub rows: Vec<SelectRow>,
    pub selected: Option<usize>,
    initially_selected: Option<usize>,
}

fn bounded_value(text: &str) -> (String, bool) {
    let mut units = 0;
    let mut result = String::new();
    for ch in text.chars() {
        units += ch.len_utf16();
        if units > MAX_VALUE_UNITS {
            return (result, true);
        }
        result.push(ch);
    }
    (result, false)
}

fn option_text(doc: &Document, node: usize) -> (String, bool) {
    let mut stack: Vec<_> = doc.nodes[node].children.iter().rev().copied().collect();
    let mut text = String::new();
    let mut space = false;
    let (mut nodes, mut bytes, mut units) = (0, 0, 0);
    while let Some(child) = stack.pop() {
        nodes += 1;
        if nodes > MAX_SELECT_WALK {
            return (text, true);
        }
        match &doc.nodes[child].kind {
            NodeKind::Comment(_) => {},
            NodeKind::DocumentFragment => stack.extend(doc.nodes[child].children.iter().rev().copied()),
            NodeKind::Text(value) => {
                for ch in value.chars() {
                    bytes += ch.len_utf8();
                    if bytes > MAX_TOTAL_VALUE_BYTES {
                        return (text, true);
                    }
                    if matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{c}') {
                        space = !text.is_empty();
                    } else {
                        units += ch.len_utf16() + usize::from(space);
                        if units > MAX_VALUE_UNITS {
                            return (text, true);
                        }
                        if space {
                            text.push(' ');
                        }
                        text.push(ch);
                        space = false;
                    }
                }
            }
            NodeKind::Element { tag, .. }
                if !matches!(
                    tag.as_str(),
                    "script" | "style" | "template" | "option" | "select" | "datalist"
                ) =>
            {
                stack.extend(doc.nodes[child].children.iter().rev().copied());
            }
            _ => {}
        }
    }
    (text, false)
}

fn select_state(doc: &Document, node: usize) -> (SelectState, bool, usize) {
    let mut state = SelectState {
        options: Vec::new(),
        rows: Vec::new(),
        selected: None,
        initially_selected: None,
    };
    let mut stack: Vec<_> = doc.nodes[node]
        .children
        .iter()
        .rev()
        .map(|&node| (node, None))
        .collect();
    let (mut walked, mut bytes) = (0, 0);
    let mut limited = false;
    while let Some((child, group)) = stack.pop() {
        walked += 1;
        if walked > MAX_SELECT_WALK || state.rows.len() >= MAX_SELECT_ROWS {
            limited = true;
            break;
        }
        match tag(doc, child) {
            "option" => {
                if state.options.len() == MAX_SELECT_OPTIONS {
                    limited = true;
                    break;
                }
                let (text, text_limited) = option_text(doc, child);
                let (value, value_limited) = doc
                    .attr(child, "value")
                    .map(bounded_value)
                    .unwrap_or_else(|| (text.clone(), text_limited));
                let label: String = doc
                    .attr(child, "label")
                    .filter(|label| !label.is_empty())
                    .unwrap_or(&text)
                    .chars()
                    .take(256)
                    .collect();
                bytes += value.len() + label.len();
                limited |= value_limited || text_limited || bytes > MAX_TOTAL_VALUE_BYTES;
                if limited {
                    break;
                }
                let index = state.options.len();
                let disabled = doc.attr(child, "disabled").is_some()
                    || group.is_some_and(|group| doc.attr(group, "disabled").is_some());
                state.options.push(SelectOption {
                    node: child,
                    label,
                    placeholder: index == 0 && group.is_none() && value.is_empty(),
                    value,
                    disabled,
                    group,
                });
                state.rows.push(SelectRow::Option(index));
                if doc.attr(child, "selected").is_some() {
                    state.initially_selected = Some(index);
                }
            }
            "optgroup" => {
                let label: String = doc
                    .attr(child, "label")
                    .unwrap_or("")
                    .chars()
                    .take(256)
                    .collect();
                bytes += label.len();
                if bytes > MAX_TOTAL_VALUE_BYTES {
                    limited = true;
                    break;
                }
                state.rows.push(SelectRow::Group { node: child, label });
                stack.extend(
                    doc.nodes[child]
                        .children
                        .iter()
                        .rev()
                        .map(|&node| (node, Some(child))),
                );
            }
            "select" | "script" | "style" | "template" | "datalist" | "iframe" | "object" => {}
            "noscript" if doc.scripting_enabled => {}
            _ => stack.extend(
                doc.nodes[child]
                    .children
                    .iter()
                    .rev()
                    .map(|&node| (node, group)),
            ),
        }
    }
    state.selected = state
        .initially_selected
        .or_else(|| state.options.iter().position(|option| !option.disabled));
    (state, limited, bytes)
}

pub fn label_targets(doc: &Document) -> HashMap<usize, usize> {
    let mut ids = HashMap::new();
    let mut active = vec![false; doc.nodes.len()];
    let mut order = Vec::new();
    let mut stack = if doc.nodes.is_empty() {
        Vec::new()
    } else {
        vec![0]
    };
    while let Some(index) = stack.pop() {
        if index >= doc.nodes.len()
            || active[index]
            || matches!(
                tag(doc, index),
                "head" | "script" | "style" | "template" | "iframe" | "object" | "datalist"
            )
            || doc.scripting_enabled && tag(doc, index) == "noscript"
        {
            continue;
        }
        active[index] = true;
        order.push(index);
        if let Some(id) = doc.attr(index, "id") {
            ids.entry(id).or_insert(index);
        }
        stack.extend(doc.nodes[index].children.iter().rev().copied());
    }
    let mut labels = HashMap::new();
    for node in order {
        if tag(doc, node) != "label" {
            continue;
        }
        let target = if let Some(id) = doc.attr(node, "for") {
            ids.get(id)
                .copied()
                .filter(|&index| kind(doc, index).is_some_and(|k| k != Kind::Hidden))
        } else {
            let mut stack: Vec<_> = doc.nodes[node].children.iter().rev().copied().collect();
            let mut found = None;
            while let Some(child) = stack.pop() {
                if !active[child] {
                    continue;
                }
                if kind(doc, child).is_some_and(|k| k != Kind::Hidden) {
                    found = Some(child);
                    break;
                }
                if !matches!(
                    tag(doc, child),
                    "script" | "style" | "template" | "iframe" | "object"
                ) {
                    stack.extend(doc.nodes[child].children.iter().rev());
                }
            }
            found
        };
        if let Some(target) = target {
            labels.insert(node, target);
        }
    }
    labels
}

pub struct Control {
    pub owner: Option<usize>,
    pub kind: Kind,
    pub value: Editor,
    // The HTML dirty value flag belongs to the control, not its current text.
    // Setting .value to the existing/default value still makes a text control dirty.
    pub dirty_value: bool,
    pub name: String,
    pub caption: String,
    pub disabled: bool,
    pub readonly: bool,
    pub required: bool,
    pub checked: bool,
    pub max_length: usize,
    pub tab_index: i32,
    pub select: Option<SelectState>,
    initial: String,
    initially_checked: bool,
    // CSS-pixel coordinates selected by a real image activation. Keyboard and
    // label activation leave this point unchanged; its initial value is (0,0).
    image_point: (i32, i32),
}

impl Control {
    pub fn mark_value_dirty(&mut self) {
        if matches!(self.kind, Kind::Text | Kind::TextArea) {
            self.dirty_value = true;
        }
    }
    pub fn editable(&self) -> bool {
        matches!(self.kind, Kind::Text | Kind::TextArea) && !self.disabled && !self.readonly
    }
    pub fn focusable(&self) -> bool {
        !self.disabled && !matches!(self.kind, Kind::Hidden | Kind::Unsupported)
    }
    fn submitted_value(&self) -> String {
        if matches!(self.kind, Kind::Text | Kind::TextArea | Kind::Select) {
            self.value.text()
        } else {
            self.initial.clone()
        }
    }
    fn set_selection(&mut self, selected: Option<usize>) -> bool {
        let state = self
            .select
            .as_mut()
            .expect("select control has option state");
        let changed = state.selected != selected;
        state.selected = selected;
        let option = selected.and_then(|index| state.options.get(index));
        self.caption = option
            .map(|option| option.label.clone())
            .unwrap_or_default();
        self.value.chars = option
            .map(|option| option.value.chars().collect())
            .unwrap_or_default();
        self.value.cursor = self.value.chars.len();
        self.value.anchor = self.value.cursor;
        self.value.offset = 0;
        changed
    }
    pub fn insert(&mut self, text: &str) {
        if !self.editable() {
            return;
        }
        let deleted = self.value.delete_selection();
        let used: usize = self.value.chars.iter().map(|ch| ch.len_utf16()).sum();
        let mut available = self.max_length.saturating_sub(used);
        let incoming: Vec<char> = normalized_chars(text)
            .filter(|ch| self.kind == Kind::TextArea || *ch != '\n')
            .take_while(|ch| {
                if ch.len_utf16() > available {
                    return false;
                }
                available -= ch.len_utf16();
                true
            })
            .collect();
        let count = incoming.len();
        self.value
            .chars
            .splice(self.value.cursor..self.value.cursor, incoming);
        self.value.cursor += count;
        self.value.anchor = self.value.cursor;
        if deleted || count != 0 {
            self.mark_value_dirty();
        }
    }
}

/// Prepared text INPUT default-value state. The caller must commit without
/// callbacks after its DOM attribute transaction has passed every fallible check.
/// Native byte metrics are separate from the DOM attribute's retained bytes;
/// old_native_bytes is not a DOM debit unless the caller previously charged it.
pub(crate) struct TextDefaultPlan {
    node: usize,
    initial: String,
    value: Option<Editor>,
    pub(crate) copy_bytes: usize,
    pub(crate) old_native_bytes: usize,
    pub(crate) new_native_bytes: usize,
}

impl TextDefaultPlan {
    pub(crate) fn commit(self, forms: &mut Forms) {
        let control = forms.controls.get_mut(&self.node).expect("Prepared text INPUT identity");
        debug_assert_eq!(control.kind, Kind::Text);
        control.initial = self.initial;
        if let Some(value) = self.value {
            control.value = value;
        }
    }
}

fn normalized_chars(text: &str) -> impl Iterator<Item = char> + '_ {
    let mut previous_cr = false;
    text.chars().filter_map(move |ch| {
        let skip = previous_cr && ch == '\n';
        previous_cr = ch == '\r';
        (!skip).then_some(if ch == '\r' { '\n' } else { ch })
    })
}

fn textarea_initial(doc: &Document, node: usize) -> (String, bool) {
    let mut text = String::new();
    let mut stack: Vec<_> = doc.nodes[node].children.iter().rev().copied().collect();
    let (mut walked, mut units) = (0, 0);
    while let Some(child) = stack.pop() {
        walked += 1;
        if walked > MAX_SELECT_WALK {
            return (text, true);
        }
        match &doc.nodes[child].kind {
            NodeKind::Comment(_) => {},
            NodeKind::Text(value) => {
                for ch in value.chars() {
                    units += ch.len_utf16();
                    if units > MAX_VALUE_UNITS {
                        return (text, true);
                    }
                    text.push(ch);
                }
            }
            NodeKind::Element { .. } | NodeKind::DocumentFragment => {
                stack.extend(doc.nodes[child].children.iter().rev().copied());
            }
        }
    }
    (normalized_chars(&text).collect(), false)
}

fn set_form_value(editor: &mut Editor, text: &str, kind: Kind) {
    // Textarea API values normalize CR/CRLF to LF. Single-line inputs remove
    // CR/LF, while tabs remain part of either value. Submission normalizes LF
    // back to CRLF through the existing URL-encoded form serializer.
    let mut units = 0;
    editor.chars = normalized_chars(text)
        .filter(|ch| kind == Kind::TextArea || *ch != '\n')
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= MAX_VALUE_UNITS
        })
        .collect();
    editor.cursor = editor.chars.len();
    editor.anchor = editor.cursor;
    editor.offset = 0;
}

pub struct Forms {
    pub controls: BTreeMap<usize, Control>,
    /// Bounds must not silently change the values sent to a server.
    limited: bool,
}

impl Forms {
    /// Upper bound for the native payload scan before preparing an attribute
    /// plan. Include item counts even when all names/values/labels are empty.
    pub(crate) fn default_scan_work(&self) -> usize {
        self.controls.values().fold(self.controls.len().saturating_add(1), |work, control| {
            let mut work = work.saturating_add(control.name.len()).saturating_add(control.initial.len())
                .saturating_add(control.value.chars.len().saturating_mul(std::mem::size_of::<char>()));
            if let Some(select) = &control.select {
                work = work.saturating_add(select.options.len()).saturating_add(select.rows.len());
                for option in &select.options {
                    work = work.saturating_add(option.value.len()).saturating_add(option.label.len());
                }
                for row in &select.rows {
                    if let SelectRow::Group { label, .. } = row { work = work.saturating_add(label.len()); }
                }
            }
            work
        })
    }

    pub(crate) fn text_default_value(&self, node: usize) -> Result<&str, String> {
        let control = self.controls.get(&node).filter(|control| control.kind == Kind::Text)
            .ok_or("Default value update requires a native text INPUT.")?;
        Ok(&control.initial)
    }

    /// Attribute removal supplies None. Raw default text is retained, while a
    /// pristine live value uses the existing single-line CR/LF sanitization.
    /// Dirty editors, including caret/selection/offset, are not reconstructed.
    pub(crate) fn prepare_text_default(&self, node: usize, value: Option<&str>) -> Result<TextDefaultPlan, String> {
        if self.limited || self.controls.len() > MAX_CONTROLS {
            return Err("Native form limits already exceeded; text default was not changed.".into());
        }
        let control = self.controls.get(&node).filter(|control| control.kind == Kind::Text)
            .ok_or("Default value update requires a native text INPUT.")?;
        let text = value.unwrap_or("");
        if text.encode_utf16().count() > MAX_VALUE_UNITS {
            return Err("A text INPUT default is limited to 4096 UTF-16 units.".into());
        }
        let sanitized_bytes = text.chars().filter(|ch| !matches!(ch, '\r' | '\n'))
            .map(char::len_utf8).sum::<usize>();
        let mut total = 0usize;
        for (&other, item) in &self.controls {
            if item.name.len() > MAX_QUERY {
                return Err("A form control name exceeds 8192 bytes; text default was not changed.".into());
            }
            let live = if other == node && !control.dirty_value { sanitized_bytes }
                else { item.value.chars.iter().map(|ch| ch.len_utf8()).sum() };
            let default = if other == node { text.len() } else { item.initial.len() };
            // Bound either the edited value or its retained Reset default. Do
            // not count identical initial/live text twice or discard defaults.
            total = total.checked_add(item.name.len())
                .and_then(|total| total.checked_add(live.max(default)))
                .ok_or("Text default form payload overflow.")?;
            if let Some(select) = &item.select {
                for option in &select.options {
                    total = total.checked_add(option.value.len())
                        .and_then(|total| total.checked_add(option.label.len()))
                        .ok_or("Text default option payload overflow.")?;
                }
                for row in &select.rows {
                    if let SelectRow::Group { label, .. } = row {
                        total = total.checked_add(label.len()).ok_or("Text default group payload overflow.")?;
                    }
                }
            }
            if total > MAX_TOTAL_VALUE_BYTES {
                return Err("Combined form names, values and defaults exceed 128 KiB; no default was changed.".into());
            }
        }
        let mut initial = String::new();
        initial.try_reserve(text.len()).map_err(|_| "Text default reservation failed.")?;
        initial.push_str(text);
        let same_value = control.value.chars.iter().copied()
            .eq(text.chars().filter(|ch| !matches!(ch, '\r' | '\n')));
        let value = if control.dirty_value || same_value { None } else {
            let mut editor = Editor::default();
            let count = text.chars().filter(|ch| !matches!(ch, '\r' | '\n')).count();
            editor.chars.try_reserve(count).map_err(|_| "Text default editor reservation failed.")?;
            editor.chars.extend(text.chars().filter(|ch| !matches!(ch, '\r' | '\n')));
            editor.cursor = editor.chars.len();
            editor.anchor = editor.cursor;
            Some(editor)
        };
        let old_editor_bytes = control.value.chars.len().checked_mul(std::mem::size_of::<char>())
            .ok_or("Text default native payload overflow.")?;
        let copied_editor_bytes = value.as_ref().map_or(0, |editor| editor.chars.len() * std::mem::size_of::<char>());
        let old_native_bytes = control.initial.len().checked_add(old_editor_bytes)
            .ok_or("Text default native payload overflow.")?;
        let new_native_bytes = initial.len().checked_add(if value.is_some() { copied_editor_bytes } else { old_editor_bytes })
            .ok_or("Text default native payload overflow.")?;
        let copy_bytes = initial.len().checked_add(copied_editor_bytes).ok_or("Text default copy size overflow.")?;
        Ok(TextDefaultPlan { node, initial, value, copy_bytes, old_native_bytes, new_native_bytes })
    }

    pub fn new(doc: &Document) -> Self {
        let mut result = Self {
            controls: BTreeMap::new(),
            limited: false,
        };
        let mut parents = vec![None; doc.nodes.len()];
        for (index, node) in doc.nodes.iter().enumerate() {
            for &child in &node.children {
                parents[child] = Some(index);
            }
        }
        let mut ids = HashMap::new();
        let mut inactive = vec![false; doc.nodes.len()];
        for node in 0..doc.nodes.len() {
            inactive[node] = parents[node].is_some_and(|p| inactive[p])
                || matches!(
                    tag(doc, node),
                    "head" | "script" | "style" | "template" | "iframe" | "object" | "datalist"
                )
                || doc.scripting_enabled && tag(doc, node) == "noscript";
            if !inactive[node] {
                if let Some(id) = doc.attr(node, "id") {
                    ids.entry(id).or_insert(node);
                }
            }
        }
        let mut total = 0usize;
        let mut option_total = 0usize;
        for node in 0..doc.nodes.len() {
            let Some(kind) = kind(doc, node) else {
                continue;
            };
            if inactive[node] {
                continue;
            }
            if result.controls.len() == MAX_CONTROLS {
                result.limited = true;
                break;
            }
            let owner = if let Some(id) = doc.attr(node, "form") {
                ids.get(id).copied().filter(|&id| tag(doc, id) == "form")
            } else {
                let mut p = parents[node];
                while let Some(index) = p {
                    if tag(doc, index) == "form" {
                        break;
                    }
                    p = parents[index];
                }
                p
            };
            let disabled = disabled(doc, node, &parents);
            let initial = if kind == Kind::TextArea {
                let (text, limited) = textarea_initial(doc, node);
                result.limited |= limited;
                text
            } else if kind == Kind::Select {
                String::new()
            } else {
                doc.attr(node, "value")
                    .unwrap_or(if matches!(kind, Kind::Checkbox | Kind::Radio) {
                        "on"
                    } else {
                        ""
                    })
                    .to_owned()
            };
            let name = doc.attr(node, "name").unwrap_or("");
            total = total
                .saturating_add(initial.len())
                .saturating_add(name.len());
            if initial.encode_utf16().count() > MAX_VALUE_UNITS
                || name.len() > MAX_QUERY
                || total > MAX_TOTAL_VALUE_BYTES
            {
                result.limited = true;
            }
            let mut units = 0;
            let initial: String = initial
                .chars()
                .take_while(|ch| {
                    units += ch.len_utf16();
                    units <= MAX_VALUE_UNITS
                })
                .collect();
            let mut value = Editor::default();
            set_form_value(&mut value, &initial, kind);
            let checked = doc.attr(node, "checked").is_some();
            let select = if kind == Kind::Select {
                let (state, limited, bytes) = select_state(doc, node);
                total = total.saturating_add(bytes);
                option_total = option_total.saturating_add(state.options.len());
                if limited || total > MAX_TOTAL_VALUE_BYTES || option_total > MAX_TOTAL_OPTIONS {
                    result.limited = true;
                    None
                } else {
                    Some(state)
                }
            } else {
                None
            };
            let mut control = Control {
                owner,
                kind: if kind == Kind::Select && select.is_none() {
                    Kind::Unsupported
                } else {
                    kind
                },
                value,
                dirty_value: false,
                name: name.chars().take(MAX_QUERY).collect(),
                caption: if kind == Kind::Select {
                    String::new()
                } else {
                    caption(doc, node, kind)
                },
                disabled,
                readonly: doc.attr(node, "readonly").is_some(),
                required: doc.attr(node, "required").is_some(),
                checked,
                max_length: doc
                    .attr(node, "maxlength")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(MAX_VALUE_UNITS)
                    .min(MAX_VALUE_UNITS),
                tab_index: doc
                    .attr(node, "tabindex")
                    .and_then(parse_tab_index)
                    .unwrap_or(0),
                select,
                initial,
                initially_checked: checked,
                image_point: (0, 0),
            };
            if let Some(state) = &control.select {
                control.set_selection(state.selected);
            } else if kind == Kind::Select {
                control.caption = "select exceeds limits".into();
            }
            result.controls.insert(node, control);
            if kind == Kind::Radio && checked {
                result.check_radio(node);
            }
        }
        result
    }

    /// Native selection cannot choose a disabled option. Scripted selectedIndex
    /// and value use separate setters, which may select disabled options.
    pub fn select_option(&mut self, node: usize, index: usize) -> Result<bool, String> {
        let control = self
            .controls
            .get_mut(&node)
            .ok_or("This select is not available.")?;
        if control.kind != Kind::Select
            || control.disabled
            || control
                .select
                .as_ref()
                .and_then(|state| state.options.get(index))
                .is_none_or(|option| option.disabled)
        {
            return Err("This option is not available.".into());
        }
        Ok(control.set_selection(Some(index)))
    }

    pub fn script_set_selected_index(&mut self, node: usize, index: i32) -> Result<(), String> {
        let control = self
            .controls
            .get_mut(&node)
            .ok_or("This select is not available.")?;
        let state = control
            .select
            .as_ref()
            .ok_or("selectedIndex requires a supported select.")?;
        let selected = usize::try_from(index)
            .ok()
            .filter(|&index| index < state.options.len());
        control.set_selection(selected);
        Ok(())
    }

    /// Select a point only for an enabled image button. Layout/App determine
    /// the rendered-image origin, zoom and border/padding geometry; this model
    /// keeps signed CSS-pixel integers and the existing submission size bounds.
    pub fn select_image_point(&mut self, node: usize, x: i32, y: i32) -> Result<(), String> {
        let control = self
            .controls
            .get_mut(&node)
            .ok_or("This image button is not available.")?;
        if control.kind != Kind::Image || control.disabled {
            return Err("This image button is not available.".into());
        }
        control.image_point = (x, y);
        Ok(())
    }

    pub fn toggle(&mut self, node: usize) {
        let Some(control) = self.controls.get_mut(&node) else {
            return;
        };
        if control.disabled {
            return;
        }
        match control.kind {
            Kind::Checkbox => control.checked = !control.checked,
            Kind::Radio => self.check_radio(node),
            _ => {}
        }
    }

    pub fn script_set_value(&mut self, node: usize, text: &str) -> Result<(), String> {
        let kind = self
            .controls
            .get(&node)
            .ok_or("This element is not a supported form control.")?
            .kind;
        if kind == Kind::Unsupported {
            return Err("This form control does not support scripted values yet.".into());
        }
        let text: String = if kind == Kind::TextArea {
            normalized_chars(text).take(MAX_VALUE_UNITS + 1).collect()
        } else {
            text.chars()
                .filter(|ch| kind != Kind::Text || !matches!(ch, '\r' | '\n'))
                .take(MAX_VALUE_UNITS + 1)
                .collect()
        };
        if text.encode_utf16().count() > MAX_VALUE_UNITS {
            return Err("A form value is limited to 4096 UTF-16 units.".into());
        }
        if kind == Kind::Select {
            let control = self.controls.get_mut(&node).unwrap();
            let selected = control
                .select
                .as_ref()
                .unwrap()
                .options
                .iter()
                .position(|option| option.value == text);
            control.set_selection(selected);
            return Ok(());
        }
        let total = self.controls.iter().fold(text.len(), |total, (&id, c)| {
            total
                .saturating_add(c.name.len())
                .saturating_add(if id == node {
                    0
                } else {
                    c.value.chars.iter().map(|ch| ch.len_utf8()).sum()
                })
        });
        if total > MAX_TOTAL_VALUE_BYTES {
            return Err("Combined form names and values exceed 128 KiB.".into());
        }
        let control = self.controls.get_mut(&node).unwrap();
        let same_text = matches!(kind, Kind::Text | Kind::TextArea)
            && control.value.chars.iter().copied().eq(text.chars());
        if !same_text {
            control.value.chars = text.chars().collect();
            control.value.cursor = control.value.chars.len();
            control.value.anchor = control.value.cursor;
            control.value.offset = 0;
        }
        control.mark_value_dirty();
        // Text inputs keep their original default for Reset. The other supported
        // input types use the value attribute/default-value mode.
        if !matches!(kind, Kind::Text | Kind::TextArea) {
            control.initial = text;
        }
        Ok(())
    }

    pub fn script_set_checked(&mut self, node: usize, checked: bool) -> Result<(), String> {
        let kind = self
            .controls
            .get(&node)
            .ok_or("This element is not a supported form control.")?
            .kind;
        if !matches!(kind, Kind::Checkbox | Kind::Radio) {
            return Err("The checked property requires a checkbox or radio input.".into());
        }
        if kind == Kind::Radio && checked {
            self.check_radio(node);
        } else {
            self.controls.get_mut(&node).unwrap().checked = checked;
        }
        Ok(())
    }

    /// Re-evaluate disabled attributes without recreating edited/default values.
    pub fn refresh_disabled(&mut self, doc: &Document) {
        let mut parents = vec![None; doc.nodes.len()];
        for (node, item) in doc.nodes.iter().enumerate() {
            for &child in &item.children {
                parents[child] = Some(node);
            }
        }
        for (&node, control) in &mut self.controls {
            control.disabled = disabled(doc, node, &parents);
            if let Some(state) = &mut control.select {
                for option in &mut state.options {
                    option.disabled = doc.attr(option.node, "disabled").is_some()
                        || option
                            .group
                            .is_some_and(|group| doc.attr(group, "disabled").is_some());
                }
            }
        }
    }

    fn check_radio(&mut self, node: usize) {
        let Some(control) = self.controls.get(&node) else {
            return;
        };
        let owner = control.owner;
        let name = control.name.clone();
        for (&index, other) in &mut self.controls {
            if other.kind == Kind::Radio
                && other.owner == owner
                && other.name == name
                && (!name.is_empty() || index == node)
            {
                other.checked = index == node;
            }
        }
    }

    pub fn reset(&mut self, owner: usize) {
        let mut radios = Vec::new();
        for (&node, control) in &mut self.controls {
            if control.owner == Some(owner) {
                if let Some(state) = &control.select {
                    let selected = state
                        .initially_selected
                        .or_else(|| state.options.iter().position(|option| !option.disabled));
                    control.set_selection(selected);
                    continue;
                }
                set_form_value(&mut control.value, &control.initial, control.kind);
                if matches!(control.kind, Kind::Text | Kind::TextArea) {
                    control.dirty_value = false;
                }
                control.checked = control.initially_checked;
                if control.kind == Kind::Radio && control.checked {
                    radios.push(node);
                }
            }
        }
        for node in radios {
            self.check_radio(node);
        }
    }

    pub fn implicit_submitter(&self, node: usize) -> Result<(usize, Option<usize>), String> {
        let owner = self
            .controls
            .get(&node)
            .and_then(|c| c.owner)
            .ok_or("This field has no form.")?;
        if let Some((&node, button)) = self
            .controls
            .iter()
            .find(|(_, c)| c.owner == Some(owner) && matches!(c.kind, Kind::Submit | Kind::Image))
        {
            if button.disabled {
                return Err("The form's default submit button is disabled.".into());
            }
            return Ok((owner, Some(node)));
        }
        if self
            .controls
            .values()
            .filter(|c| c.owner == Some(owner) && c.kind == Kind::Text)
            .count()
            > 1
        {
            return Err("This form has multiple text fields and no submit button.".into());
        }
        Ok((owner, None))
    }

    pub fn validate_submission(
        &self,
        doc: &Document,
        owner: usize,
        submitter: Option<usize>,
    ) -> Result<(), String> {
        if self.limited {
            return Err(
                "This page exceeds the form value/control limits; nothing was submitted.".into(),
            );
        }
        if submitter.is_some_and(|node| {
            !self.controls.get(&node).is_some_and(|c| {
                c.owner == Some(owner)
                    && matches!(c.kind, Kind::Submit | Kind::Image)
                    && !c.disabled
            })
        }) {
            return Err("The submit button is not available.".into());
        }
        let validate = doc.attr(owner, "novalidate").is_none()
            && !submitter.is_some_and(|id| doc.attr(id, "formnovalidate").is_some());
        for control in self.controls.values() {
            if control.owner != Some(owner) || control.disabled {
                continue;
            }
            let required = control.required
                || control.kind == Kind::Radio
                    && !control.name.is_empty()
                    && self.controls.values().any(|c| {
                        c.owner == Some(owner)
                            && c.kind == Kind::Radio
                            && c.name == control.name
                            && c.required
                    });
            if validate
                && required
                && !(matches!(control.kind, Kind::Text | Kind::TextArea) && control.readonly)
            {
                let missing = match control.kind {
                    Kind::Text | Kind::TextArea => control.value.chars.is_empty(),
                    Kind::Select => control
                        .select
                        .as_ref()
                        .and_then(|state| state.selected.and_then(|index| state.options.get(index)))
                        .is_none_or(|option| option.placeholder),
                    Kind::Checkbox => !control.checked,
                    Kind::Radio if control.name.is_empty() => !control.checked,
                    Kind::Radio => !self.controls.values().any(|c| {
                        c.owner == Some(owner)
                            && c.kind == Kind::Radio
                            && c.name == control.name
                            && c.checked
                    }),
                    _ => false,
                };
                if missing {
                    return Err(format!(
                        "Complete the required field: {}.",
                        if control.name.is_empty() {
                            "unnamed field"
                        } else {
                            &control.name
                        }
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn submit_request(
        &self,
        doc: &Document,
        owner: usize,
        submitter: Option<usize>,
        current: &Location,
        base: &Location,
    ) -> Result<Submission, String> {
        self.serialize_submission(doc, owner, submitter, current, base, true)
    }

    /// HTMLFormElement.submit() bypasses interactive constraint validation and
    /// has no submitter. Encoding, unsupported-control, size and URL checks are
    /// shared with a real button submission; no network request starts here.
    pub fn script_submit_request(
        &self,
        doc: &Document,
        owner: usize,
        current: &Location,
        base: &Location,
    ) -> Result<Submission, String> {
        self.serialize_submission(doc, owner, None, current, base, false)
    }

    fn serialize_submission(
        &self,
        doc: &Document,
        owner: usize,
        submitter: Option<usize>,
        current: &Location,
        base: &Location,
        validate: bool,
    ) -> Result<Submission, String> {
        if owner >= doc.nodes.len() || tag(doc, owner) != "form" {
            return Err("Submission requires an existing form.".into());
        }
        if self.limited {
            return Err(
                "This page exceeds the form value/control limits; nothing was submitted.".into(),
            );
        }
        if let Some(node) = submitter {
            if !self.controls.get(&node).is_some_and(|c| {
                c.owner == Some(owner)
                    && matches!(c.kind, Kind::Submit | Kind::Image)
                    && !c.disabled
            }) {
                return Err("The submit button is not available.".into());
            }
        }
        let override_attr = |button_name, form_name| {
            submitter
                .and_then(|node| doc.attr(node, button_name))
                .or_else(|| doc.attr(owner, form_name))
        };
        let method = override_attr("formmethod", "method").unwrap_or("get");
        if method.eq_ignore_ascii_case("dialog") {
            return Err("Form method DIALOG is not implemented.".into());
        }
        let post = method.eq_ignore_ascii_case("post");
        let enctype =
            override_attr("formenctype", "enctype").unwrap_or("application/x-www-form-urlencoded");
        if post
            && (enctype.eq_ignore_ascii_case("multipart/form-data")
                || enctype.eq_ignore_ascii_case("text/plain"))
        {
            return Err("POST currently supports application/x-www-form-urlencoded only; multipart and text/plain forms are not implemented.".into());
        }
        let encoded_limit = if post {
            crate::http::MAX_FORM_BYTES
        } else {
            MAX_QUERY
        };
        let action = override_attr("formaction", "action").unwrap_or("");
        let mut target = if action.is_empty() {
            current.clone()
        } else {
            navigation::resolve(action, base)?
        };
        let url = target.url.as_mut().ok_or(
            "Forms need an HTTP or HTTPS action; local-file submissions are not supported.",
        )?;
        if current.url.as_ref().is_some_and(|origin| origin.secure) && !url.secure {
            return Err("HTTPS form submission to unencrypted HTTP was blocked.".into());
        }
        if validate {
            self.validate_submission(doc, owner, submitter)?;
        }
        let mut query = String::new();
        for node in self.submission_order(doc)? {
            let control = &self.controls[&node];
            if control.owner != Some(owner) || control.disabled {
                continue;
            }
            // An image contributes only its selected coordinates, and only
            // when it is the submitter. Even an unnamed image contributes x/y.
            if control.kind == Kind::Image {
                if Some(node) == submitter {
                    for (axis, value) in
                        [("x", control.image_point.0), ("y", control.image_point.1)]
                    {
                        let name = if control.name.is_empty() {
                            axis.to_owned()
                        } else {
                            format!("{}.{axis}", control.name)
                        };
                        append_entry(&name, &value.to_string(), &mut query, encoded_limit)?;
                    }
                }
                continue;
            }
            if control.kind == Kind::Select {
                if !control.name.is_empty() {
                    if let Some(option) = control
                        .select
                        .as_ref()
                        .and_then(|state| state.selected.and_then(|index| state.options.get(index)))
                        .filter(|option| !option.disabled)
                    {
                        append_entry(&control.name, &option.value, &mut query, encoded_limit)?;
                    }
                }
                continue;
            }
            if control.name.is_empty() {
                continue;
            }
            if control.kind == Kind::Unsupported {
                return Err(format!(
                    "A named {} control is not implemented; nothing was submitted.",
                    tag(doc, node)
                ));
            }
            if matches!(control.kind, Kind::Button | Kind::Reset)
                || control.kind == Kind::Submit && Some(node) != submitter
                || matches!(control.kind, Kind::Checkbox | Kind::Radio) && !control.checked
            {
                continue;
            }
            let value =
                if control.kind == Kind::Hidden && control.name.eq_ignore_ascii_case("_charset_") {
                    "UTF-8".to_owned()
                } else {
                    control.submitted_value()
                };
            append_entry(&control.name, &value, &mut query, encoded_limit)?;
        }
        let request = if post {
            // A POST action keeps its existing query; successful controls form
            // the request body. Origin belongs to the document, never <base>.
            crate::http::Request::FormPost {
                body: query.into_bytes(),
                origin: current.url.clone(),
            }
        } else {
            // The HTML GET algorithm replaces the action query.
            url.query = Some(query);
            crate::http::Request::Get
        };
        if url.request_target().len() > 8176 || url.display().len() > 16 * 1024 {
            return Err("The form creates a URL longer than this browser's request limit.".into());
        }
        Ok(Submission {
            location: target,
            request,
        })
    }

    #[cfg(test)]
    fn submit(
        &self,
        doc: &Document,
        owner: usize,
        submitter: Option<usize>,
        current: &Location,
        base: &Location,
    ) -> Result<Location, String> {
        self.submit_request(doc, owner, submitter, current, base)
            .map(|s| s.location)
    }
}

fn disabled(doc: &Document, node: usize, parents: &[Option<usize>]) -> bool {
    if doc.attr(node, "disabled").is_some() {
        return true;
    }
    let mut ancestor = parents[node];
    while let Some(index) = ancestor {
        if tag(doc, index) == "fieldset" && doc.attr(index, "disabled").is_some() {
            let legend = doc.nodes[index]
                .children
                .iter()
                .copied()
                .find(|&id| tag(doc, id) == "legend");
            let mut current = Some(node);
            while current.is_some() && current != Some(index) && current != legend {
                current = parents[current.unwrap()];
            }
            if legend.is_none() || current != legend {
                return true;
            }
        }
        ancestor = parents[index];
    }
    false
}

fn append_entry(name: &str, value: &str, output: &mut String, limit: usize) -> Result<(), String> {
    if !output.is_empty() {
        output.push('&');
    }
    encode_into(name, output, limit)?;
    output.push('=');
    encode_into(value, output, limit)
}

pub fn encode_query_component(text: &str, limit: usize) -> Result<String,String> {
    if text.len()>limit { return Err("Search query exceeds the address allowance.".into()); }
    let mut output=String::new();
    encode_into(text,&mut output,limit)?;
    Ok(output)
}

fn encode_into(text: &str, output: &mut String, limit: usize) -> Result<(), String> {
    // Convert lone CR/LF to CRLF before UTF-8 application/x-www-form-urlencoded.
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        let mut utf8 = [0u8; 4];
        let bytes = if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            &b"\r\n"[..]
        } else if ch == '\n' {
            &b"\r\n"[..]
        } else {
            ch.encode_utf8(&mut utf8).as_bytes()
        };
        for &byte in bytes {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                    output.push(byte as char)
                }
                b' ' => output.push('+'),
                _ => {
                    const HEX: &[u8; 16] = b"0123456789ABCDEF";
                    output.push('%');
                    output.push(HEX[(byte >> 4) as usize] as char);
                    output.push(HEX[(byte & 15) as usize] as char);
                }
            }
        }
        if output.len() > limit {
            return Err("Encoded form values exceed this browser's request limit.".into());
        }
    }
    if output.len() > limit {
        return Err("Encoded form values exceed this browser's request limit.".into());
    }
    Ok(())
}

#[path = "forms_select_fragments.rs"]
mod select_fragments;

#[path = "forms_hidden_inputs.rs"]
mod hidden_inputs;
pub(crate) use select_fragments::{fragment_text_input, SelectFormPlan};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    #[test]
    fn tab_index_html_integer_prefix_parser_preserves_signed_range_and_ascii_rules() {
        for (text, expected) in [(" +12junk", Some(12)), ("\t\n\x0c\r -7tail", Some(-7)),
            ("1e3", Some(1)), ("0x10", Some(0)), ("-0", Some(0)),
            ("2147483647", Some(i32::MAX)), ("-2147483648", Some(i32::MIN)),
            ("2147483648", None), ("-2147483649", None), ("", None), ("+", None),
            ("-", None), ("+ 1", None), ("\u{a0}2", None), ("\x0b2", None)] {
            assert_eq!(parse_tab_index(text), expected, "{text:?}");
        }
        assert_eq!(parse_tab_index(&("0".repeat(20_000) + "12rest")), Some(12));
        assert_eq!(parse_tab_index(&"9".repeat(20_000)), None);
    }
    fn location(url: &str) -> Location {
        navigation::address(url, &Location::home()).unwrap()
    }

    fn node_id(doc: &Document, id: &str) -> usize {
        doc.nodes
            .iter()
            .enumerate()
            .find(|(node, _)| doc.attr(*node, "id") == Some(id))
            .unwrap()
            .0
    }

    #[test]
    fn text_input_default_plan_stages_pristine_value_without_rebuilding_neighbors() {
        let doc = html::parse("<form id=f><input id=i name=q value=before><input id=other name=keep value=neighbor></form>");
        let node = node_id(&doc, "i");
        let other = node_id(&doc, "other");
        let mut forms = Forms::new(&doc);
        forms.controls.get_mut(&other).unwrap().value.cursor = 2;
        forms.controls.get_mut(&other).unwrap().value.anchor = 5;
        forms.controls.get_mut(&other).unwrap().value.offset = 1;
        forms.controls.get_mut(&node).unwrap().value.cursor = 1;
        forms.controls.get_mut(&node).unwrap().value.anchor = 4;
        forms.controls.get_mut(&node).unwrap().value.offset = 1;
        let unchanged = forms.prepare_text_default(node, Some("before")).unwrap();
        assert_eq!(unchanged.copy_bytes, 6, "Equal pristine values retain their editor");
        unchanged.commit(&mut forms);
        let editor = &forms.controls[&node].value;
        assert_eq!((editor.cursor, editor.anchor, editor.offset), (1, 4, 1));
        assert!(!forms.controls[&node].dirty_value);
        let plan = forms.prepare_text_default(node, Some("a\r\nb")).unwrap();
        assert_eq!(forms.text_default_value(node).unwrap(), "before");
        assert_eq!(forms.controls[&node].value.text(), "before");
        assert_eq!((plan.copy_bytes, plan.old_native_bytes, plan.new_native_bytes), (12, 30, 12));
        plan.commit(&mut forms);
        assert_eq!(forms.text_default_value(node).unwrap(), "a\r\nb");
        let control = &forms.controls[&node];
        assert_eq!(control.value.text(), "ab");
        assert_eq!((control.value.cursor, control.value.anchor, control.value.offset), (2, 2, 0));
        assert!(!control.dirty_value);
        let neighbor = &forms.controls[&other];
        assert_eq!(neighbor.value.text(), "neighbor");
        assert_eq!((neighbor.value.cursor, neighbor.value.anchor, neighbor.value.offset), (2, 5, 1));
    }

    #[test]
    fn text_input_default_same_value_write_dirties_and_reset_restores_latest_default() {
        let doc = html::parse("<form id=f><input id=i value=base></form>");
        let node = node_id(&doc, "i");
        let mut forms = Forms::new(&doc);
        {
            let editor = &mut forms.controls.get_mut(&node).unwrap().value;
            editor.cursor = 1;
            editor.anchor = 3;
            editor.offset = 1;
        }
        forms.script_set_value(node, "base").unwrap();
        assert!(forms.controls[&node].dirty_value, "Equal .value assignment must still set the dirty flag");
        let plan = forms.prepare_text_default(node, Some("next")).unwrap();
        assert_eq!(plan.copy_bytes, 4, "A dirty editor is retained, not recopied");
        plan.commit(&mut forms);
        let control = &forms.controls[&node];
        assert_eq!(control.value.text(), "base");
        assert_eq!((control.value.cursor, control.value.anchor, control.value.offset), (1, 3, 1));
        assert_eq!(forms.text_default_value(node).unwrap(), "next");
        forms.reset(node_id(&doc, "f"));
        assert_eq!(forms.controls[&node].value.text(), "next");
        assert!(!forms.controls[&node].dirty_value);
        forms.prepare_text_default(node, None).unwrap().commit(&mut forms);
        assert_eq!(forms.text_default_value(node).unwrap(), "");
        assert_eq!(forms.controls[&node].value.text(), "");
        assert!(!forms.controls[&node].dirty_value);
    }

    #[test]
    fn text_input_default_native_edits_and_other_control_modes_keep_distinct_flags() {
        let doc = html::parse("<form id=f><input id=i value=base><textarea id=t>area</textarea><input id=h type=hidden value=hidden><input id=c type=checkbox value=yes checked><select id=s><option value=a>A<option value=b>B</select></form>");
        let mut forms = Forms::new(&doc);
        let node = node_id(&doc, "i");
        forms.controls.get_mut(&node).unwrap().insert("");
        assert!(!forms.controls[&node].dirty_value);
        forms.controls.get_mut(&node).unwrap().readonly = true;
        forms.controls.get_mut(&node).unwrap().insert("ignored");
        assert_eq!(forms.controls[&node].value.text(), "base");
        assert!(!forms.controls[&node].dirty_value);
        forms.controls.get_mut(&node).unwrap().readonly = false;
        forms.controls.get_mut(&node).unwrap().insert("X");
        assert_eq!(forms.controls[&node].value.text(), "baseX");
        assert!(forms.controls[&node].dirty_value);
        let textarea = node_id(&doc, "t");
        forms.script_set_value(textarea, "edited\r\narea").unwrap();
        assert_eq!(forms.controls[&textarea].value.text(), "edited\narea");
        assert!(forms.controls[&textarea].dirty_value);
        for (id, value) in [("h", "newhidden"), ("c", "other"), ("s", "b")] {
            let other = node_id(&doc, id);
            forms.script_set_value(other, value).unwrap();
            assert!(!forms.controls[&other].dirty_value);
            assert!(forms.prepare_text_default(other, Some("refused")).is_err());
        }
        assert_eq!(forms.controls[&node_id(&doc, "h")].initial, "newhidden");
        assert!(forms.controls[&node_id(&doc, "c")].checked);
        assert_eq!(forms.controls[&node_id(&doc, "s")].value.text(), "b");
        assert!(forms.prepare_text_default(textarea, Some("refused")).is_err());
        forms.reset(node_id(&doc, "f"));
        assert_eq!(forms.controls[&node].value.text(), "base");
        assert_eq!(forms.controls[&textarea].value.text(), "area");
        assert!(!forms.controls[&node].dirty_value && !forms.controls[&textarea].dirty_value);
    }

    #[test]
    fn text_input_default_refusals_preserve_state_and_existing_payload_bounds() {
        let empty_select = Forms::new(&html::parse("<select></select>"));
        let one_empty_option = Forms::new(&html::parse("<select><option></option></select>"));
        assert!(one_empty_option.default_scan_work() > empty_select.default_scan_work(),
            "Empty option/row entries still consume scan work");
        let source = format!("<form id=f>{}<input id=empty></form>",
            (0..32).map(|index| format!("<input id=i{index} value='{}'>", "x".repeat(MAX_VALUE_UNITS))).collect::<String>());
        let doc = html::parse(&source);
        let mut forms = Forms::new(&doc);
        assert!(!forms.limited, "The original defaults exactly fill the existing 128 KiB allowance");
        let first = node_id(&doc, "i0");
        let empty = node_id(&doc, "empty");
        forms.script_set_value(first, "").unwrap();
        assert!(forms.controls[&first].dirty_value);
        assert!(forms.prepare_text_default(empty, Some("x")).is_err(), "Edited live text cannot hide retained Reset defaults from the payload bound");
        assert_eq!(forms.text_default_value(empty).unwrap(), "");
        assert_eq!(forms.controls[&empty].value.text(), "");
        assert!(!forms.controls[&empty].dirty_value);
        assert!(forms.prepare_text_default(empty, Some(&"🦀".repeat(MAX_VALUE_UNITS / 2 + 1))).is_err());
        assert!(forms.prepare_text_default(usize::MAX, None).is_err());
        forms.controls.get_mut(&empty).unwrap().name = "n".repeat(MAX_QUERY + 1);
        assert!(forms.prepare_text_default(empty, None).is_err());
        assert_eq!(forms.text_default_value(empty).unwrap(), "");
        assert_eq!(forms.controls[&first].value.text(), "");
        assert_eq!(forms.text_default_value(first).unwrap().len(), MAX_VALUE_UNITS);
        assert!(forms.controls[&first].dirty_value);
    }

    #[test]
    fn select_defaults_labels_groups_and_reset_keep_selection_and_value_distinct() {
        let doc = html::parse("<form id=f><select id=s name=pick><option disabled>Unavailable<optgroup label=Disabled disabled><option value=no>No</optgroup><optgroup label=Choices><option value=a label=Alpha>not the label<option value=b selected> Beta \n choice <option value=c selected>Gamma</optgroup></select></form>");
        let node = node_id(&doc, "s");
        let owner = node_id(&doc, "f");
        let mut forms = Forms::new(&doc);
        let control = &forms.controls[&node];
        assert_eq!(control.kind, Kind::Select);
        assert!(!control.editable());
        assert!(control.focusable());
        let state = control.select.as_ref().unwrap();
        assert_eq!(state.options.len(), 5);
        assert_eq!(state.rows.len(), 7);
        assert_eq!(state.selected, Some(4));
        assert_eq!(state.options[2].label, "Alpha");
        assert_eq!(state.options[3].label, "Beta choice");
        assert_eq!(control.value.text(), "c");
        assert_eq!(control.caption, "Gamma");
        assert!(forms.select_option(node, 1).is_err());
        assert!(forms.select_option(node, 2).unwrap());
        assert!(!forms.select_option(node, 2).unwrap());
        assert_eq!(forms.controls[&node].value.text(), "a");
        assert_eq!(forms.controls[&node].caption, "Alpha");
        forms.reset(owner);
        assert_eq!(forms.controls[&node].value.text(), "c");

        let doc = html::parse("<select id=s><option disabled>A<optgroup disabled label=G><option>B</optgroup><option>C<option>D</select>");
        let forms = Forms::new(&doc);
        let control = &forms.controls[&node_id(&doc, "s")];
        assert_eq!(control.select.as_ref().unwrap().selected, Some(2));
        assert_eq!(control.value.text(), "C");
    }

    #[test]
    fn select_submits_selected_value_in_tree_order_for_get_and_post() {
        let current = location("http://example.test/form");
        for method in ["get", "post"] {
            let doc = html::parse(&format!("<form id=f method={method} action='/result?old=yes'><input name=before value=1><select id=s name=pick><option>fallback \t text<option value='a&#10;b &amp;é' label=Shown selected>Other</select><select name=omit><option disabled selected value=no>No</select><select name=empty></select><select><option value=unnamed>Unnamed</select><input name=after value=2></form>"));
            let forms = Forms::new(&doc);
            assert_eq!(forms.controls[&node_id(&doc, "s")].value.text(), "a\nb &é");
            let submission = forms
                .submit_request(&doc, node_id(&doc, "f"), None, &current, &current)
                .unwrap();
            let expected = "before=1&pick=a%0D%0Ab+%26%C3%A9&after=2";
            if method == "get" {
                assert_eq!(
                    submission.location.url.unwrap().query.as_deref(),
                    Some(expected)
                );
            } else {
                assert_eq!(
                    submission.location.url.unwrap().query.as_deref(),
                    Some("old=yes")
                );
                let crate::http::Request::FormPost { body, .. } = submission.request else {
                    panic!("expected POST");
                };
                assert_eq!(body, expected.as_bytes());
            }
        }
    }

    #[test]
    fn select_required_placeholder_and_scripted_disabled_selection_are_explicit() {
        let current = location("http://example.test/");
        let doc = html::parse("<form id=f><select id=s name=pick required><option value=''>Choose<option value=ok>Okay<optgroup label=Group><option value='' label='Empty value allowed'><option value=no disabled>Disabled</optgroup></select></form>");
        let owner = node_id(&doc, "f");
        let node = node_id(&doc, "s");
        let mut forms = Forms::new(&doc);
        assert!(forms.validate_submission(&doc, owner, None).is_err());
        forms.select_option(node, 2).unwrap();
        assert!(forms.validate_submission(&doc, owner, None).is_ok());
        assert_eq!(
            forms
                .submit(&doc, owner, None, &current, &current)
                .unwrap()
                .url
                .unwrap()
                .query
                .as_deref(),
            Some("pick=")
        );
        forms.script_set_value(node, "no").unwrap();
        assert_eq!(
            forms.controls[&node].select.as_ref().unwrap().selected,
            Some(3)
        );
        assert!(forms.validate_submission(&doc, owner, None).is_ok());
        assert_eq!(
            forms
                .submit(&doc, owner, None, &current, &current)
                .unwrap()
                .url
                .unwrap()
                .query
                .as_deref(),
            Some("")
        );
        assert!(forms.select_option(node, 3).is_err());
        forms.script_set_value(node, "unmatched").unwrap();
        assert_eq!(
            forms.controls[&node].select.as_ref().unwrap().selected,
            None
        );
        assert_eq!(forms.controls[&node].value.text(), "");
        assert!(forms.validate_submission(&doc, owner, None).is_err());
        forms.script_set_selected_index(node, 1).unwrap();
        forms.reset(owner);
        assert_eq!(
            forms.controls[&node].select.as_ref().unwrap().selected,
            Some(0)
        );
        forms.script_set_selected_index(node, i32::MAX).unwrap();
        assert_eq!(
            forms.controls[&node].select.as_ref().unwrap().selected,
            None
        );
    }

    #[test]
    fn select_disabled_state_updates_without_discarding_a_current_selection() {
        let mut doc = html::parse("<form id=f><select id=s name=pick><optgroup id=g label=Group><option value=a>A</optgroup><option value=b>B</select><fieldset disabled><select id=off name=off><option>No</select></fieldset></form>");
        let node = node_id(&doc, "s");
        let group = node_id(&doc, "g");
        let owner = node_id(&doc, "f");
        let mut forms = Forms::new(&doc);
        assert!(!forms.controls[&node_id(&doc, "off")].focusable());
        if let NodeKind::Element { attrs, .. } = &mut doc.nodes[group].kind {
            attrs.push(crate::html::Attribute::unqualified("disabled".into(), "".into()));
        }
        forms.refresh_disabled(&doc);
        assert!(forms.controls[&node].select.as_ref().unwrap().options[0].disabled);
        assert_eq!(forms.controls[&node].value.text(), "a");
        assert!(forms.select_option(node, 0).is_err());
        forms.reset(owner);
        assert_eq!(forms.controls[&node].value.text(), "b");
    }

    #[test]
    fn select_limits_and_unsupported_listboxes_never_submit_partial_choices() {
        let current = location("http://example.test/");
        for source in [
            "<select name=s multiple><option>A</select>".to_owned(),
            "<select name=s size=2><option>A</select>".to_owned(),
            format!(
                "<select name=s>{}</select>",
                "<option value=x>X".repeat(MAX_SELECT_OPTIONS + 1)
            ),
            format!(
                "<select name=s><option value='{}'>Long</select>",
                "x".repeat(MAX_VALUE_UNITS + 1)
            ),
            format!(
                "<select name=s>{}</select>",
                "<optgroup label=G></optgroup>".repeat(MAX_SELECT_ROWS + 1)
            ),
            (0..9)
                .map(|_| {
                    format!(
                        "<select name=s>{}</select>",
                        "<option value=x>X".repeat(512)
                    )
                })
                .collect(),
        ] {
            let doc = html::parse(&format!("<form id=f>{source}</form>"));
            let forms = Forms::new(&doc);
            assert!(forms
                .submit(&doc, node_id(&doc, "f"), None, &current, &current)
                .is_err());
            assert!(forms.controls.values().all(|control| control
                .select
                .as_ref()
                .is_none_or(|state| state.options.len() <= MAX_SELECT_OPTIONS)));
        }
        let doc = html::parse(&format!(
            "<form id=f><select name=s>{}</select></form>",
            "<option value=x>X".repeat(MAX_SELECT_OPTIONS)
        ));
        assert!(Forms::new(&doc)
            .submit(&doc, node_id(&doc, "f"), None, &current, &current)
            .is_ok());
    }

    #[test]
    fn image_submit_entries_keep_tree_order_and_omit_other_buttons_and_value() {
        let doc = html::parse("<form action='/send?old=1'><input type=hidden name=before value=one><input id=picture type=IMAGE name='common ports' value=unused alt='Common ports'><input type=image name=other value=unused><button name=ordinary value=no>Ordinary</button><input type=hidden name=after value=two></form>");
        let mut forms = Forms::new(&doc);
        let image = node_id(&doc, "picture");
        let control = &forms.controls[&image];
        assert_eq!(control.kind, Kind::Image);
        assert_eq!(control.caption, "Common ports");
        assert!(control.focusable());
        let owner = control.owner.unwrap();
        forms.select_image_point(image, 12, 7).unwrap();
        let current = location("https://example.test/form");
        let target = forms
            .submit(&doc, owner, Some(image), &current, &current)
            .unwrap();
        assert_eq!(
            target.url.unwrap().query.unwrap(),
            "before=one&common+ports.x=12&common+ports.y=7&after=two"
        );
        let ordinary = forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Submit)
            .map(|(&node, _)| node)
            .unwrap();
        let target = forms
            .submit(&doc, owner, Some(ordinary), &current, &current)
            .unwrap();
        assert_eq!(
            target.url.unwrap().query.unwrap(),
            "before=one&ordinary=no&after=two"
        );
    }

    #[test]
    fn unnamed_images_submit_coordinates_and_nonpoint_activation_preserves_them() {
        let current = location("https://example.test/form");
        for name in ["", "name=''"] {
            let doc = html::parse(&format!(
                "<form><input id=picture type=image {name} alt=' ' value=unused></form>"
            ));
            let mut forms = Forms::new(&doc);
            let image = node_id(&doc, "picture");
            let owner = forms.controls[&image].owner.unwrap();
            assert_eq!(forms.controls[&image].caption, "Submit");
            let query = |forms: &Forms| {
                forms
                    .submit(&doc, owner, Some(image), &current, &current)
                    .unwrap()
                    .url
                    .unwrap()
                    .query
                    .unwrap()
            };
            assert_eq!(query(&forms), "x=0&y=0");
            forms.select_image_point(image, -4, 19).unwrap();
            assert_eq!(query(&forms), "x=-4&y=19");
            // Keyboard/label activation supplies no new point, so the selected
            // coordinate persists, independently of the input's value.
            forms.script_set_value(image, "still unused").unwrap();
            assert_eq!(query(&forms), "x=-4&y=19");
            assert_eq!(forms.controls[&image].value.text(), "still unused");
            assert_eq!(forms.controls[&image].caption, "Submit");
            forms.reset(owner);
            assert_eq!(query(&forms), "x=-4&y=19");
            assert_eq!(forms.controls[&image].caption, "Submit");
        }
    }

    #[test]
    fn image_submitter_uses_post_overrides_and_form_validation() {
        let current = location("https://example.test/form?original=1");
        for novalidate in ["", "formnovalidate"] {
            let doc = html::parse(&format!("<form method=get action='/ignored'><input name=q required><input id=picture type=image name='coörd' formmethod=post formaction='/send?keep=1#done' formenctype='application/x-www-form-urlencoded' {novalidate}></form>"));
            let mut forms = Forms::new(&doc);
            let image = node_id(&doc, "picture");
            let owner = forms.controls[&image].owner.unwrap();
            forms.select_image_point(image, -3, 8).unwrap();
            let result = forms.submit_request(&doc, owner, Some(image), &current, &current);
            if novalidate.is_empty() {
                assert!(result.unwrap_err().contains("required"));
            } else {
                let sent = result.unwrap();
                assert_eq!(
                    sent.location.display(),
                    "https://example.test/send?keep=1#done"
                );
                let crate::http::Request::FormPost { body, origin } = sent.request else {
                    panic!("expected POST")
                };
                assert_eq!(
                    String::from_utf8(body).unwrap(),
                    "q=&co%C3%B6rd.x=-3&co%C3%B6rd.y=8"
                );
                assert_eq!(origin, current.url);
            }
        }
    }

    #[test]
    fn images_participate_in_default_selection_and_reject_invalid_submitters() {
        let current = location("https://example.test/form");
        let doc = html::parse("<form id=first><input id=field name=q><input id=picture type=image name=image><button id=ordinary>Go</button></form><form><input id=foreign type=image name=foreign></form>");
        let mut forms = Forms::new(&doc);
        let field = node_id(&doc, "field");
        let image = node_id(&doc, "picture");
        let owner = forms.controls[&field].owner.unwrap();
        assert_eq!(
            forms.implicit_submitter(field).unwrap(),
            (owner, Some(image))
        );
        assert!(forms.select_image_point(field, 1, 2).is_err());
        assert!(forms.select_image_point(usize::MAX, 1, 2).is_err());
        assert!(forms
            .submit(
                &doc,
                owner,
                Some(node_id(&doc, "foreign")),
                &current,
                &current
            )
            .unwrap_err()
            .contains("not available"));
        forms.controls.get_mut(&image).unwrap().disabled = true;
        assert!(forms.select_image_point(image, 1, 2).is_err());
        assert!(forms
            .implicit_submitter(field)
            .unwrap_err()
            .contains("disabled"));
        assert!(forms
            .submit(&doc, owner, Some(image), &current, &current)
            .unwrap_err()
            .contains("not available"));
        let ordinary = node_id(&doc, "ordinary");
        assert!(forms
            .submit(&doc, owner, Some(ordinary), &current, &current)
            .is_ok());
        let doc = html::parse(
            "<form><input id=field><button id=ordinary>Go</button><input type=image></form>",
        );
        let forms = Forms::new(&doc);
        assert_eq!(
            forms.implicit_submitter(node_id(&doc, "field")).unwrap().1,
            Some(node_id(&doc, "ordinary"))
        );
    }

    #[test]
    fn image_overrides_preserve_encoding_and_downgrade_rejections() {
        let current = location("https://example.test/form");
        for (attributes, expected) in [
            ("formaction='http://example.test/send'", "HTTPS form"),
            (
                "formmethod=post formenctype=multipart/form-data",
                "application/x-www-form-urlencoded",
            ),
            ("formmethod=dialog", "DIALOG"),
        ] {
            let doc = html::parse(&format!(
                "<form><input id=picture type=image {attributes}></form>"
            ));
            let forms = Forms::new(&doc);
            let image = node_id(&doc, "picture");
            let owner = forms.controls[&image].owner.unwrap();
            assert!(forms
                .submit_request(&doc, owner, Some(image), &current, &current)
                .unwrap_err()
                .contains(expected));
        }
    }

    #[test]
    fn image_coordinate_entries_keep_signed_integer_and_request_size_bounds() {
        let current = location("https://example.test/form");
        let doc = html::parse("<form><input id=picture type=image name=point></form>");
        let mut forms = Forms::new(&doc);
        let image = node_id(&doc, "picture");
        let owner = forms.controls[&image].owner.unwrap();
        forms.select_image_point(image, i32::MIN, i32::MAX).unwrap();
        assert_eq!(
            forms
                .submit(&doc, owner, Some(image), &current, &current)
                .unwrap()
                .url
                .unwrap()
                .query
                .unwrap(),
            "point.x=-2147483648&point.y=2147483647"
        );
        for (method, fields, name) in [
            ("get", String::new(), "x".repeat(4096)),
            (
                "post",
                (0..10)
                    .map(|i| format!("<input name=v{i} value='{}'>", "%".repeat(4096)))
                    .collect(),
                "%".repeat(1500),
            ),
        ] {
            let doc = html::parse(&format!(
                "<form method={method}>{fields}<input id=picture type=image name='{name}'></form>"
            ));
            let forms = Forms::new(&doc);
            let image = node_id(&doc, "picture");
            let owner = forms.controls[&image].owner.unwrap();
            assert!(forms
                .submit_request(&doc, owner, Some(image), &current, &current)
                .unwrap_err()
                .contains("request limit"));
        }
    }

    #[test]
    fn secure_forms_reject_downgrade_in_actions_bases_and_button_overrides() {
        let secure = location("https://example.test/form");
        let insecure = location("http://example.test/base/");
        for (source, base) in [
            ("<form action='http://example.test/send'><input name=q value=secret><button>Go</button></form>", &secure),
            ("<form action='send'><input name=q value=secret><button>Go</button></form>", &insecure),
            ("<form action='/send'><input name=q value=secret><button formaction='http://other.test/'>Go</button></form>", &secure),
        ] {
            let doc = html::parse(source);
            let forms = Forms::new(&doc);
            let (&node, button) = forms.controls.iter().find(|(_, c)| c.kind == Kind::Submit).unwrap();
            assert!(forms.submit(&doc, button.owner.unwrap(), Some(node), &secure, base).unwrap_err().contains("HTTPS form"));
        }
        let doc = html::parse("<form><input name=q value=ok></form>");
        let forms = Forms::new(&doc);
        let owner = forms.controls.values().next().unwrap().owner.unwrap();
        // An empty action uses the secure document even with an HTTP base.
        assert!(
            forms
                .submit(&doc, owner, None, &secure, &insecure)
                .unwrap()
                .url
                .unwrap()
                .secure
        );
    }
    #[test]
    fn radio_required_groups_include_disabled_members_but_not_nameless_peers() {
        let current = location("http://example.test/");
        for source in [
            "<form><input type=radio required><input type=radio checked><button>Go</button></form>",
            "<form><input type=radio name=r required disabled><input type=radio name=r><button>Go</button></form>",
            "<form><input type=checkbox name=c required readonly><button>Go</button></form>",
        ] {
            let doc = html::parse(source);
            let forms = Forms::new(&doc);
            let (&node, button) = forms.controls.iter().find(|(_, c)| c.kind == Kind::Submit).unwrap();
            assert!(forms.submit(&doc, button.owner.unwrap(), Some(node), &current, &current).unwrap_err().contains("required"));
        }
    }
    #[test]
    fn tabs_hidden_newlines_and_backslashes_round_trip_without_wire_controls() {
        let doc = html::parse("<form><input name=q value='C:&#92;folder&#9;tab&#10;end'><input type=hidden name=lines value='a&#13;b'><input type=hidden name=_CHARSET_ value=no><button>Go</button></form>");
        let mut forms = Forms::new(&doc);
        let node = *forms.controls.keys().next().unwrap();
        assert_eq!(forms.controls[&node].value.text(), "C:\\folder\ttabend");
        forms.controls.get_mut(&node).unwrap().insert("\t\nX");
        let (owner, submitter) = forms.implicit_submitter(node).unwrap();
        let current = location("http://example.test/");
        let target = forms
            .submit(&doc, owner, submitter, &current, &current)
            .unwrap();
        assert_eq!(
            target.url.as_ref().unwrap().query.as_deref(),
            Some("q=C%3A%5Cfolder%09tabend%09X&lines=a%0D%0Ab&_CHARSET_=UTF-8")
        );
        assert_eq!(
            navigation::address(&target.display(), &current).unwrap(),
            target
        );
        assert!(!target
            .url
            .as_ref()
            .unwrap()
            .request_target()
            .contains(['\r', '\n', '\t', '\\']));
        forms.reset(owner);
        assert_eq!(forms.controls[&node].value.text(), "C:\\folder\ttabend");
    }
    #[test]
    fn bounds_reject_incomplete_values_and_oversized_requests() {
        let current = location("http://example.test/");
        for source in [
            format!(
                "<form><input name=a value='{}'><input name=b value='{}'></form>",
                "x".repeat(4090),
                "x".repeat(4090)
            ),
            format!("<form><input name=q value='{}'></form>", "🦀".repeat(2049)),
            format!("<form>{}</form>", "<input name=q>".repeat(MAX_CONTROLS + 1)),
        ] {
            let doc = html::parse(&source);
            let forms = Forms::new(&doc);
            let owner = forms.controls.values().next().unwrap().owner.unwrap();
            assert!(forms.submit(&doc, owner, None, &current, &current).is_err());
            assert!(forms.controls.len() <= MAX_CONTROLS);
            assert!(forms
                .controls
                .values()
                .all(|c| c.value.text().encode_utf16().count() <= MAX_VALUE_UNITS));
        }
    }
    #[test]
    fn get_entries_order_overrides_unicode_and_query_replacement() {
        let doc = html::parse("<form id=f action='/old?discard=1#result'><input name=q value='café +~'><input type=hidden name=x value='a&#10;b'><input type=checkbox name=c checked><input type=checkbox name=off><button name=go value=yes formaction='/search?old=1#done'>Go</button></form><input form=f name=outside value=ok><input form=missing name=no value=bad>");
        let forms = Forms::new(&doc);
        let (&submitter, button) = forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Submit)
            .unwrap();
        let current = location("http://example.test/dir/doc");
        let target = forms
            .submit(
                &doc,
                button.owner.unwrap(),
                Some(submitter),
                &current,
                &current,
            )
            .unwrap();
        assert_eq!(
            target.url.unwrap().display(),
            "http://example.test/search?q=caf%C3%A9+%2B%7E&x=a%0D%0Ab&c=on&go=yes&outside=ok"
        );
        assert_eq!(target.fragment, "done");
    }
    #[test]
    fn disabled_fieldset_first_legend_inert_controls_and_radio_reset() {
        let doc = html::parse("<form><fieldset disabled><legend><input name=yes value=ok></legend><input name=no value=bad><legend><input name=no2></legend></fieldset><template><input name=no3></template><input type=radio name=r value=a checked><input type=radio name=r value=b checked><input type=radio name=r value=c><button>Go</button></form>");
        let mut forms = Forms::new(&doc);
        let ids: Vec<_> = forms
            .controls
            .iter()
            .filter(|(_, c)| c.kind == Kind::Radio)
            .map(|(&id, _)| id)
            .collect();
        assert!(!forms.controls[&ids[0]].checked);
        assert!(forms.controls[&ids[1]].checked);
        forms.toggle(ids[2]);
        assert!(!forms.controls[&ids[1]].checked);
        assert!(forms.controls[&ids[2]].checked);
        let owner = forms.controls[&ids[2]].owner.unwrap();
        forms.reset(owner);
        assert!(forms.controls[&ids[1]].checked);
        let current = location("http://example.test/");
        let target = forms.submit(&doc, owner, None, &current, &current).unwrap();
        assert_eq!(target.url.unwrap().query.unwrap(), "yes=ok&r=b");
    }
    #[test]
    fn readonly_required_limits_and_unsupported_methods_do_not_submit() {
        let doc = html::parse("<form method=dialog><input name=q maxlength=3 required><input name=r value=fixed readonly><button>Go</button></form>");
        let mut forms = Forms::new(&doc);
        let node = *forms.controls.keys().next().unwrap();
        forms.controls.get_mut(&node).unwrap().insert("é🦀x");
        assert_eq!(forms.controls[&node].value.text(), "é🦀");
        let (owner, submitter) = forms.implicit_submitter(node).unwrap();
        let current = location("http://example.test/");
        assert!(forms
            .submit(&doc, owner, submitter, &current, &current)
            .unwrap_err()
            .contains("DIALOG"));
        let doc = html::parse("<form><input name=q required><button>Go</button></form>");
        let forms = Forms::new(&doc);
        let node = *forms.controls.keys().next().unwrap();
        let (owner, submitter) = forms.implicit_submitter(node).unwrap();
        assert!(forms
            .submit(&doc, owner, submitter, &current, &current)
            .unwrap_err()
            .contains("required"));
    }
    #[test]
    fn missing_action_uses_document_not_base_and_named_unsupported_is_explicit() {
        let doc = html::parse("<form><input name=q value=yes></form>");
        let forms = Forms::new(&doc);
        let node = *forms.controls.keys().next().unwrap();
        let (owner, submitter) = forms.implicit_submitter(node).unwrap();
        let current = location("http://example.test/doc?old=1");
        let base = location("http://foreign.test/base");
        let target = forms
            .submit(&doc, owner, submitter, &current, &base)
            .unwrap();
        assert_eq!(
            target.url.unwrap().display(),
            "http://example.test/doc?q=yes"
        );
        let doc = html::parse("<form><select name=q multiple><option>hello</option></select><button>Go</button></form>");
        let forms = Forms::new(&doc);
        let (&node, button) = forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Submit)
            .unwrap();
        assert!(forms
            .submit(&doc, button.owner.unwrap(), Some(node), &current, &base)
            .unwrap_err()
            .contains("not implemented"));
    }
    #[test]
    fn textarea_values_edit_normalize_submit_and_reset_without_changing_the_default() {
        let doc = html::parse("<form id=f action='/search'><textarea id=q name=q rows=2>\nfirst &amp; second\nthird</textarea><button id=send>Search</button></form>");
        let mut forms = Forms::new(&doc);
        let node = node_id(&doc, "q");
        let owner = node_id(&doc, "f");
        let send = node_id(&doc, "send");
        assert_eq!(forms.controls[&node].kind, Kind::TextArea);
        assert!(forms.controls[&node].editable());
        assert_eq!(forms.controls[&node].value.text(), "first & second\nthird");
        forms.controls.get_mut(&node).unwrap().value.select_all();
        forms
            .controls
            .get_mut(&node)
            .unwrap()
            .insert("café +&\r\nnext\rlast");
        assert_eq!(forms.controls[&node].value.text(), "café +&\nnext\nlast");
        let current = location("https://example.test/home");
        let result = forms
            .submit(&doc, owner, Some(send), &current, &current)
            .unwrap();
        assert_eq!(
            result.display(),
            "https://example.test/search?q=caf%C3%A9+%2B%26%0D%0Anext%0D%0Alast"
        );
        forms.script_set_value(node, "script\r\nvalue").unwrap();
        assert_eq!(forms.controls[&node].value.text(), "script\nvalue");
        forms.reset(owner);
        assert_eq!(forms.controls[&node].value.text(), "first & second\nthird");
    }

    #[test]
    fn textarea_required_readonly_disabled_and_utf16_limits_are_preserved() {
        let doc = html::parse("<form id=f><textarea id=q name=q required maxlength=3></textarea><textarea id=ro name=r readonly required></textarea><textarea name=skip disabled>omitted</textarea><button id=send>Search</button></form>");
        let mut forms = Forms::new(&doc);
        let (owner, node, send) = (
            node_id(&doc, "f"),
            node_id(&doc, "q"),
            node_id(&doc, "send"),
        );
        let current = location("http://example.test/search");
        assert!(forms
            .submit(&doc, owner, Some(send), &current, &current)
            .unwrap_err()
            .contains("required"));
        forms.controls.get_mut(&node).unwrap().insert("🦀ab");
        assert_eq!(forms.controls[&node].value.text(), "🦀a");
        assert!(!forms.controls[&node_id(&doc, "ro")].editable());
        let result = forms
            .submit(&doc, owner, Some(send), &current, &current)
            .unwrap();
        assert_eq!(
            result.display(),
            "http://example.test/search?q=%F0%9F%A6%80a&r="
        );
        assert!(forms
            .script_set_value(node, &"🦀".repeat(MAX_VALUE_UNITS))
            .is_err());
        let oversized = html::parse(&format!(
            "<form id=f><textarea name=q>{}</textarea></form>",
            "x".repeat(MAX_VALUE_UNITS + 1)
        ));
        let forms = Forms::new(&oversized);
        assert!(forms
            .submit(
                &oversized,
                node_id(&oversized, "f"),
                None,
                &current,
                &current
            )
            .unwrap_err()
            .contains("limits"));
    }

    #[test]
    fn post_keeps_action_query_submitter_spaces_and_source_origin() {
        let doc=html::parse("<form id=f method=post action='/send?token=target#result'><input type=hidden name=1 value='hidden &amp; value'><input name=q value='café'><input type=checkbox name=chosen value=yes checked><input name=ignored value=secret disabled><input type=submit name=12 value='    Proceed    '></form>");
        let forms = Forms::new(&doc);
        let (&button, control) = forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Submit)
            .unwrap();
        let current = location("https://example.test/page?private=query");
        let base = location("https://example.test/different/");
        let sent = forms
            .submit_request(&doc, control.owner.unwrap(), Some(button), &current, &base)
            .unwrap();
        assert_eq!(
            sent.location.display(),
            "https://example.test/send?token=target#result"
        );
        let crate::http::Request::FormPost { body, origin } = sent.request else {
            panic!("expected POST")
        };
        assert_eq!(
            String::from_utf8(body).unwrap(),
            "1=hidden+%26+value&q=caf%C3%A9&chosen=yes&12=++++Proceed++++"
        );
        assert_eq!(origin, current.url);
    }

    #[test]
    fn post_overrides_encodings_and_downgrades_are_explicit() {
        let current = location("https://example.test/form");
        for (attributes, expected) in [
            (
                "method=post enctype=multipart/form-data",
                "application/x-www-form-urlencoded",
            ),
            (
                "method=post enctype=text/plain",
                "application/x-www-form-urlencoded",
            ),
            (
                "method=post action='http://example.test/send'",
                "HTTPS form",
            ),
            ("method=dialog", "DIALOG"),
        ] {
            let doc = html::parse(&format!(
                "<form {attributes}><input name=q value=private><button>Go</button></form>"
            ));
            let forms = Forms::new(&doc);
            let (&button, c) = forms
                .controls
                .iter()
                .find(|(_, c)| c.kind == Kind::Submit)
                .unwrap();
            assert!(forms
                .submit_request(&doc, c.owner.unwrap(), Some(button), &current, &current)
                .unwrap_err()
                .contains(expected));
        }
        let doc=html::parse("<form method=get><input name=q value=ok><button formmethod=post formaction='https://other.test/receive?keep=yes'>Go</button></form>");
        let forms = Forms::new(&doc);
        let (&button, c) = forms
            .controls
            .iter()
            .find(|(_, c)| c.kind == Kind::Submit)
            .unwrap();
        let sent = forms
            .submit_request(
                &doc,
                c.owner.unwrap(),
                Some(button),
                &Location::home(),
                &Location::home(),
            )
            .unwrap();
        assert!(matches!(
            sent.request,
            crate::http::Request::FormPost { origin: None, .. }
        ));
        assert!(sent.location.display().ends_with("/receive?keep=yes"));
    }

    #[test]
    fn post_body_has_a_separate_bound_from_get_urls() {
        let current = location("https://example.test/form");
        for (value, count, succeeds) in [("x", 3, true), ("%", 11, false)] {
            let fields = (0..count)
                .map(|i| format!("<input name=v{i} value='{}'>", value.repeat(4096)))
                .collect::<String>();
            let doc = html::parse(&format!(
                "<form method=post>{fields}<button>Go</button></form>"
            ));
            let forms = Forms::new(&doc);
            let (&button, c) = forms
                .controls
                .iter()
                .find(|(_, c)| c.kind == Kind::Submit)
                .unwrap();
            let sent =
                forms.submit_request(&doc, c.owner.unwrap(), Some(button), &current, &current);
            assert_eq!(sent.is_ok(), succeeds);
            if let Ok(Submission {
                request: crate::http::Request::FormPost { body, .. },
                ..
            }) = sent
            {
                assert!(body.len() > 8192 && body.len() <= crate::http::MAX_FORM_BYTES);
            }
        }
    }
}
