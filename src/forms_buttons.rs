//! Staged native BUTTON state. Detached buttons retain ordinary caption
//! construction until insertion; existing native controls are never rebuilt.
use super::*;

enum ButtonUpdate {
    Name(String),
    Value { initial: String, value: Editor },
    Type(Kind),
}
pub(crate) struct ButtonAttributePlan {
    node: usize,
    update: ButtonUpdate,
}
impl ButtonAttributePlan {
    pub(crate) fn commit(self, forms: &mut Forms) {
        let control = forms.controls.get_mut(&self.node).expect("Prepared native BUTTON");
        match self.update {
            ButtonUpdate::Name(name) => control.name = name,
            ButtonUpdate::Value { initial, value } => {
                control.initial = initial;
                control.value = value;
            }
            ButtonUpdate::Type(kind) => control.kind = kind,
        }
    }
}

fn copied(text: &str) -> Result<String, String> {
    let mut result = String::new();
    result.try_reserve(text.len()).map_err(|_| "BUTTON string reservation failed.")?;
    result.push_str(text);
    Ok(result)
}
fn raw_editor(text: &str) -> Result<Editor, String> {
    let mut value = Editor::default();
    value.chars.try_reserve(text.chars().count()).map_err(|_| "BUTTON value reservation failed.")?;
    value.chars.extend(text.chars());
    value.cursor = value.chars.len(); value.anchor = value.cursor;
    Ok(value)
}
fn button_kind(value: Option<&str>) -> Kind {
    match value {
        Some(value) if value.eq_ignore_ascii_case("button") => Kind::Button,
        Some(value) if value.eq_ignore_ascii_case("reset") => Kind::Reset,
        _ => Kind::Submit,
    }
}
fn button_identity(doc: &Document, node: usize) -> Result<(), String> {
    if doc.nodes.get(node).is_none_or(|entry| entry.namespace != crate::html::Namespace::Html)
        || tag(doc,node) != "button" {
        return Err("Native BUTTON state requires a known HTML button.".into());
    }
    Ok(())
}
fn button_shape(doc: &Document, node: usize) -> Result<(), String> {
    button_identity(doc,node)?;
    if ["form", "command", "commandfor", "popovertarget", "popovertargetaction"].into_iter().any(|name|doc.attr(node,name).is_some()) {
        return Err("Dynamic BUTTON explicit form, command or popover association is not implemented; no control was changed.".into());
    }
    Ok(())
}

/// Plain caption leaves only. A BUTTON itself is not an admissible descendant.
pub(crate) fn button_caption_node(doc: &Document, node: usize) -> bool {
    doc.nodes.get(node).is_some_and(|entry| match &entry.kind {
        NodeKind::Text(_) | NodeKind::Comment(_) => entry.namespace == crate::html::Namespace::None && entry.children.is_empty(),
        NodeKind::Element { tag, .. } => entry.namespace == crate::html::Namespace::Html && matches!(tag.as_str(),
            "span" | "b" | "i" | "em" | "strong" | "small" | "s" | "u" | "mark" | "sub" | "sup"
            | "code" | "kbd" | "samp" | "var" | "abbr" | "cite" | "q" | "time" | "br" | "wbr"),
        NodeKind::DocumentFragment => false,
    })
}

/// No allocation or caption walk occurs here. Charge these conservative work
/// and copy bounds before validation/building creates private scratch storage.
pub(crate) fn button_caption_work(doc: &Document, node: usize) -> Result<(usize, usize), String> {
    button_shape(doc,node)?;
    let nodes = doc.nodes.len().min(MAX_SELECT_WALK);
    let attrs = match &doc.nodes[node].kind { NodeKind::Element { attrs, .. } => attrs.len(), _ => unreachable!() };
    let name = doc.attr(node,"name").map_or(0,str::len);
    let value = doc.attr(node,"value").map_or(0,str::len);
    let work = doc.nodes.len().saturating_add(nodes.saturating_mul(4)).saturating_add(attrs.saturating_mul(16))
        .saturating_add(256 * 4).saturating_add(name).saturating_add(value.saturating_mul(2));
    let copy = doc.nodes.len().saturating_add(nodes.saturating_mul(std::mem::size_of::<(usize,usize)>()))
        .saturating_add(256 * 4).saturating_add(name).saturating_add(value.saturating_mul(5));
    Ok((work,copy))
}

fn checked_caption(doc: &Document, root: usize) -> Result<String, String> {
    button_shape(doc,root)?;
    let bound = doc.nodes.len().min(MAX_SELECT_WALK);
    let mut stack = Vec::new();
    stack.try_reserve(bound).map_err(|_| "BUTTON caption walk reservation failed.")?;
    let mut seen = Vec::new();
    seen.try_reserve(doc.nodes.len()).map_err(|_| "BUTTON caption topology reservation failed.")?;
    seen.resize(doc.nodes.len(),false);
    seen[root] = true;
    stack.push((root,1usize));
    let mut queued = 1usize;
    let mut raw_chars = 0usize;
    let mut pending_space = false;
    let mut caption = String::new();
    caption.try_reserve(256 * 4).map_err(|_| "BUTTON caption reservation failed.")?;
    while let Some((node,depth)) = stack.pop() {
        if depth > 128 { return Err("BUTTON caption depth limit reached; no control was admitted.".into()); }
        if node != root && !button_caption_node(doc,node) {
            return Err("BUTTON captions accept only plain HTML phrasing text; native, interactive, resource or foreign interiors are not implemented.".into());
        }
        if let NodeKind::Text(text) = &doc.nodes[node].kind {
            for ch in text.chars().take(256 - raw_chars) {
                raw_chars += 1;
                if ch.is_ascii_whitespace() { pending_space = !caption.is_empty(); }
                else {
                    if pending_space { caption.push(' '); }
                    caption.push(ch); pending_space = false;
                }
            }
        }
        for &child in doc.nodes[node].children.iter().rev() {
            if child >= doc.nodes.len() || seen[child] || queued >= MAX_SELECT_WALK {
                return Err("BUTTON caption topology or walk limit reached; no control was admitted.".into());
            }
            seen[child] = true; queued += 1;
            stack.push((child,depth + 1));
        }
    }
    Ok(caption)
}

pub(crate) fn validate_button_caption(doc: &Document, node: usize) -> Result<(), String> {
    checked_caption(doc,node).map(|_| ())
}

impl Forms {
    /// A detached, unbound BUTTON gets validation only. Its eventual insertion
    /// stages a complete native value/caption/owner plan with its siblings.
    pub(crate) fn prepare_button_attribute(&self, doc: &Document, node: usize, key: &str,
        value: Option<&str>) -> Result<Option<ButtonAttributePlan>, String> {
        button_identity(doc,node)?;
        if !matches!(key,"name"|"value"|"type") { return Err("Unsupported native BUTTON attribute.".into()); }
        let name = if key == "name" { value.unwrap_or("") } else { doc.attr(node,"name").unwrap_or("") };
        let text = if key == "value" { value.unwrap_or("") } else { doc.attr(node,"value").unwrap_or("") };
        if name.len() > MAX_QUERY || text.encode_utf16().count() > MAX_VALUE_UNITS {
            return Err("BUTTON name/value limit reached; no attribute was changed.".into());
        }
        let existing = self.controls.get(&node);
        if existing.is_some_and(|control|!matches!(control.kind,Kind::Button|Kind::Submit|Kind::Reset)) {
            return Err("BUTTON native identity does not match its control kind.".into());
        }
        // Existing parsed BUTTON name/value reflection does not reassociate
        // ownership or change activation. Preserve that established path even
        // for modes whose new admission or type transitions remain refused.
        if existing.is_none() || key == "type" { button_shape(doc,node)?; }
        let mut bytes = self.text_payload(None)?;
        if let Some(control) = existing { bytes = bytes.checked_sub(text_inputs::control_payload(control)?).ok_or("BUTTON payload accounting underflow.")?; }
        bytes = bytes.checked_add(name.len()).and_then(|bytes|bytes.checked_add(text.len())).ok_or("BUTTON payload accounting overflow.")?;
        if bytes > MAX_TOTAL_VALUE_BYTES { return Err("Combined form names, values and defaults exceed 128 KiB; no BUTTON attribute was changed.".into()); }
        if existing.is_none() { return Ok(None); }
        let update = match key {
            "name" => ButtonUpdate::Name(copied(name)?),
            "value" => ButtonUpdate::Value { initial: copied(text)?, value: raw_editor(text)? },
            _ => ButtonUpdate::Type(button_kind(value)),
        };
        Ok(Some(ButtonAttributePlan { node, update }))
    }

    pub(crate) fn add_created_buttons(&self, doc: &Document, nodes: &[usize], plan: &mut SelectFormPlan) -> Result<(), String> {
        if self.controls.len().saturating_add(plan.additions.len()).saturating_add(nodes.len()) > MAX_CONTROLS {
            return Err("Native form control limit reached before BUTTON insertion.".into());
        }
        let mut bytes = self.text_payload(None)?;
        for control in plan.additions.values() { bytes = bytes.checked_add(text_inputs::control_payload(control)?).ok_or("Native form payload overflow.")?; }
        for &node in nodes {
            if self.controls.contains_key(&node) || plan.additions.contains_key(&node) { return Err("BUTTON native identity is already owned.".into()); }
            button_shape(doc,node)?;
            let name = doc.attr(node,"name").unwrap_or("");
            let initial = doc.attr(node,"value").unwrap_or("");
            if name.len() > MAX_QUERY || initial.encode_utf16().count() > MAX_VALUE_UNITS { return Err("BUTTON name/value limit reached before insertion.".into()); }
            bytes = bytes.checked_add(name.len()).and_then(|bytes|bytes.checked_add(initial.len())).ok_or("BUTTON payload accounting overflow.")?;
            if bytes > MAX_TOTAL_VALUE_BYTES { return Err("Combined form names, values and defaults exceed 128 KiB; no BUTTON was admitted.".into()); }
            let caption = checked_caption(doc,node)?;
            let value = raw_editor(initial)?;
            let retained = name.len().checked_add(initial.len()).and_then(|bytes|bytes.checked_add(caption.len()))
                .and_then(|bytes|bytes.checked_add(value.chars.len().checked_mul(std::mem::size_of::<char>())?))
                .ok_or("BUTTON retained payload overflow.")?;
            let control = Control { owner: None, group_root: node, kind: button_kind(doc.attr(node,"type")), value, dirty_value: false,
                name: copied(name)?, caption, disabled: doc.attr(node,"disabled").is_some(), readonly: false, required: false,
                checked: false, dirty_checked: false, max_length: MAX_VALUE_UNITS,
                tab_index: doc.attr(node,"tabindex").and_then(parse_tab_index).unwrap_or(0), select: None,
                button_element: true, initial: copied(initial)?, initially_checked: false, image_point: (0,0) };
            plan.retained_bytes = plan.retained_bytes.checked_add(retained).ok_or("Native BUTTON retained payload overflow.")?;
            plan.additions.insert(node,control);
        }
        plan.metadata.try_reserve(nodes.len()).map_err(|_| "BUTTON owner plan reservation failed.")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(doc: &Document, value: &str) -> usize {
        (0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(value)).unwrap()
    }

    #[test]
    fn created_button_forms_reset_uses_element_identity_and_preserves_input_behavior() {
        let mut doc = crate::html::parse("<form id=f><input id=submit type=submit value=old><input id=reset type=reset value=old><input id=input-button type=button value=old><input id=hidden type=hidden value=old><button id=actual type=button value=old>Caption</button><button id=auto value=old>Auto</button><button id=foreign type=button value=old>Foreign</button></form>");
        let foreign = id(&doc,"foreign");
        doc.nodes[foreign].namespace = crate::html::Namespace::UnknownForeign;
        let mut forms = Forms::new(&doc);
        for name in ["submit","reset","input-button","hidden","actual","auto","foreign"] {
            let node = id(&doc,name);
            forms.script_set_value(node,"a\r\nb").unwrap();
            assert_eq!(forms.controls[&node].value.text(),"a\r\nb");
            assert_eq!(forms.controls[&node].button_element,matches!(name,"actual"|"auto"),"{name}");
        }
        forms.reset(id(&doc,"f"));
        for name in ["submit","reset","input-button","hidden","foreign"] {
            assert_eq!(forms.controls[&id(&doc,name)].value.text(),"ab","The existing INPUT/non-HTML normalization path must remain: {name}");
        }
        for name in ["actual","auto"] {
            let control = &forms.controls[&id(&doc,name)];
            assert_eq!(control.value.text(),"a\r\nb","Actual HTML BUTTON Reset preserves current raw value");
            assert_eq!(control.initial,"a\r\nb");
        }
    }

    #[test]
    fn created_button_forms_checked_caption_matches_existing_native_labels_and_prefix_limits() {
        let samples = [
            "Original button".to_owned(),
            " \tOne\r\n<span> Two \u{000c}<b>Three</b></span> Four\t ".to_owned(),
            "<span>A\u{00a0}B</span>\t<span>\u{2003}C</span>".to_owned(),
            format!("{} <span>excluded</span>","a".repeat(255)),
            format!("{}<span> \tZ</span>","x".repeat(254)),
            format!("{}<span>🦀</span>excluded","x".repeat(255)),
            format!("{}excluded"," \t".repeat(128)),
            format!("{}<span> </span>bZ","a".repeat(254)),
        ];
        for sample in samples {
            let mut doc = crate::html::parse(&format!("<button id=button type=button>{sample}</button>"));
            let node = id(&doc,"button");
            // Parsed comments are omitted by this parser; retain one genuine
            // Comment Node so both caption paths must ignore its actual data.
            let comment = doc.nodes.len();
            doc.nodes.push(crate::html::Node { kind: NodeKind::Comment("not part of the label".into()),
                namespace: crate::html::Namespace::None, children: Vec::new() });
            doc.nodes[node].children.insert(0,comment);
            let expected = caption(&doc,node,Kind::Button);
            let actual = checked_caption(&doc,node).unwrap();
            assert_eq!(actual,expected,"{sample:?}");
            assert_eq!(Forms::new(&doc).controls[&node].caption,actual);
            assert!(actual.chars().count()<=256);
        }
    }
}
