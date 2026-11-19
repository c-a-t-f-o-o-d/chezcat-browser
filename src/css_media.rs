//! Finite screen-media evaluation and a checked, nonrecursive conditional walk.
//! No DOM/JavaScript API or network request is created by this module.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover { Unknown, None, Hover }

/// Host-owned CSS-pixel viewport captured for this document. The Windows App's
/// current desktop-mouse profile selects Hover; tests can select None. This is
/// a declared primary-input profile, not detection of attached touch hardware.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaEnvironment {
    pub width: f64,
    pub height: f64,
    pub hover: Hover,
}

impl MediaEnvironment {
    pub fn desktop_viewport(width: i32, height: i32, zoom: f32) -> Self {
        let zoom = if zoom.is_finite() { zoom.clamp(0.25, 5.0) } else { 1.0 };
        let css = |pixels: i32| f64::from(pixels.clamp(1, 16_384)) / f64::from(zoom);
        Self { width: css(width), height: css(height), hover: Hover::Hover }
    }
    pub(super) fn valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }
}

fn space(byte: u8) -> bool { matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c) }
fn trim(text: &str) -> &str { text.trim_matches(|c: char| c.is_ascii() && space(c as u8)) }
fn word(text: &str) -> Option<(&str, &str)> {
    let end = text.bytes().take_while(|byte| byte.is_ascii_alphabetic() || *byte == b'-').count();
    (end != 0).then(|| (&text[..end], &text[end..]))
}

/// An unsupported/malformed expression is not silently broadened. The finite
/// grammar supports media types, comma alternatives, type-level not/only,
/// conjunctions and simple parenthesized features. No general CSS evaluator.
pub(super) fn matches(source: &str, environment: Option<MediaEnvironment>) -> bool {
    let environment = environment.filter(|env| env.valid());
    let mut any = false;
    list_items(source, |item| any |= query(item, environment).unwrap_or(false));
    any
}

fn list_items(source: &str, mut visit: impl FnMut(&str)) {
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote = 0u8;
    let mut escaped = false;
    for (at, byte) in source.bytes().enumerate() {
        if quote != 0 {
            if escaped { escaped = false; }
            else if byte == b'\\' { escaped = true; }
            else if byte == quote { quote = 0; }
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = byte,
            b'(' => { depth += 1; }
            b')' => { depth = depth.saturating_sub(1); }
            b',' if depth == 0 => {
                visit(trim(&source[start..at]));
                start = at + 1;
            }
            _ => {}
        }
    }
    visit(if depth == 0 && quote == 0 { trim(&source[start..]) } else { "" });
}

pub(super) fn serialize_list(source: &str) -> String {
    if trim(source).is_empty() { return String::new(); }
    let mut output = String::new();
    // The viewport here is only used to recognize the shared finite grammar.
    // Every live matches read uses the document's actual CSS environment.
    let syntax_environment = Some(MediaEnvironment::desktop_viewport(1, 1, 1.0));
    list_items(source, |item| {
        if !output.is_empty() { output.push_str(", "); }
        if query(item, syntax_environment).is_none() {
            output.push_str("not all");
            return;
        }
        let mut rest = item;
        let mut prior = false;
        while let Some((prefix, feature)) = rest.split_once('(') {
            for word in prefix.split_ascii_whitespace() {
                if prior { output.push(' '); }
                output.push_str(&word.to_ascii_lowercase());
                prior = true;
            }
            if prior { output.push(' '); }
            let (body, next) = feature.split_once(')').unwrap();
            output.push('(');
            if let Some((name, value)) = body.split_once(':') {
                output.push_str(&trim(name).to_ascii_lowercase());
                output.push_str(": ");
                output.push_str(&trim(value).to_ascii_lowercase());
            } else {
                output.push_str(&trim(body).to_ascii_lowercase());
            }
            output.push(')');
            prior = true;
            rest = next;
        }
        for word in rest.split_ascii_whitespace() {
            if prior { output.push(' '); }
            output.push_str(&word.to_ascii_lowercase());
            prior = true;
        }
    });
    output
}

fn query(mut text: &str, environment: Option<MediaEnvironment>) -> Option<bool> {
    if text.is_empty() { return None; }
    let mut negate = false;
    let mut value = true;
    let mut has_type = false;
    if !text.starts_with('(') {
        let (mut name, mut rest) = word(text)?;
        if name.eq_ignore_ascii_case("not") || name.eq_ignore_ascii_case("only") {
            negate = name.eq_ignore_ascii_case("not");
            if !rest.as_bytes().first().is_some_and(|byte| space(*byte)) { return None; }
            rest = trim(rest);
            let next = word(rest)?;
            name = next.0;
            rest = next.1;
        }
        if ["not", "only", "and", "or"].iter().any(|reserved| name.eq_ignore_ascii_case(reserved)) { return None; }
        value = name.eq_ignore_ascii_case("screen") || name.eq_ignore_ascii_case("all");
        has_type = true;
        text = trim(rest);
    }
    let mut features = 0usize;
    while !text.is_empty() {
        if has_type || features != 0 {
            let (and, rest) = word(text)?;
            if !and.eq_ignore_ascii_case("and") { return None; }
            // Without separating whitespace `and(` is a CSS function token,
            // not the conjunction followed by a parenthesized feature.
            if !rest.as_bytes().first().is_some_and(|byte| space(*byte)) { return None; }
            text = trim(rest);
        }
        if !text.starts_with('(') { return None; }
        let close = text.find(')')?;
        let body = &text[1..close];
        if body.contains(['(', ')']) { return None; }
        value &= feature(body, environment)?;
        features = features.checked_add(1)?;
        text = trim(&text[close + 1..]);
    }
    if !has_type && features == 0 { return None; }
    Some(if negate { !value } else { value })
}

fn feature(text: &str, environment: Option<MediaEnvironment>) -> Option<bool> {
    // The renderer preserves author colors. It has no forced-palette mode.
    if trim(text).eq_ignore_ascii_case("forced-colors") { return Some(false); }
    let (name, value) = text.split_once(':')?;
    let name = trim(name);
    let value = trim(value);
    if name.is_empty() || value.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') || value.contains(':') { return None; }
    if name.eq_ignore_ascii_case("forced-colors") {
        if value.eq_ignore_ascii_case("none") { return Some(true); }
        if value.eq_ignore_ascii_case("active") { return Some(false); }
        return None;
    }
    if name.eq_ignore_ascii_case("hover") {
        let expected = if value.eq_ignore_ascii_case("hover") { Hover::Hover }
            else if value.eq_ignore_ascii_case("none") { Hover::None } else { return None };
        let env = environment?;
        if env.hover == Hover::Unknown { return None; }
        return Some(env.hover == expected);
    }
    let mut axis = name;
    let mut comparison = 0;
    if name.len() >= 4 && name[..4].eq_ignore_ascii_case("min-") { axis = &name[4..]; comparison = 1; }
    else if name.len() >= 4 && name[..4].eq_ignore_ascii_case("max-") { axis = &name[4..]; comparison = -1; }
    if !axis.eq_ignore_ascii_case("width") && !axis.eq_ignore_ascii_case("height") { return None; }
    let amount = pixels(value)?;
    if amount < 0.0 { return Some(false); }
    let env = environment?;
    let actual = if axis.eq_ignore_ascii_case("width") { env.width } else { env.height };
    Some(match comparison { 1 => actual >= amount, -1 => actual <= amount, _ => actual == amount })
}

fn pixels(text: &str) -> Option<f64> {
    let tail=text.as_bytes().get(text.len().saturating_sub(2)..);
    let scale=if tail.is_some_and(|unit|unit.eq_ignore_ascii_case(b"px")) {Some(1.0)}
        else if tail.is_some_and(|unit|unit.eq_ignore_ascii_case(b"em")) {Some(f64::from(super::INITIAL_FONT_SIZE))}
        else {None};
    let number = if scale.is_some() { &text[..text.len()-2] } else { text };
    let bytes = number.as_bytes();
    let mut at = 0;
    if bytes.first().is_some_and(|b| matches!(*b, b'+' | b'-')) { at += 1; }
    let mut digits = 0usize;
    while bytes.get(at).is_some_and(u8::is_ascii_digit) { at += 1; digits += 1; }
    if bytes.get(at) == Some(&b'.') {
        at += 1;
        let beginning = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) { at += 1; digits += 1; }
        if at == beginning { return None; }
    }
    if digits == 0 { return None; }
    if bytes.get(at).is_some_and(|b| matches!(*b, b'e' | b'E')) {
        at += 1;
        if bytes.get(at).is_some_and(|b| matches!(*b, b'+' | b'-')) { at += 1; }
        let beginning = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) { at += 1; }
        if at == beginning { return None; }
    }
    if at != bytes.len() { return None; }
    let value: f64 = number.parse().ok()?;
    let amount=value*scale.unwrap_or(1.0);
    (value.is_finite() && amount.is_finite() && (scale.is_some() || value==0.0)).then_some(amount)
}

pub(super) fn is_media(prelude: &str) -> Option<&str> {
    let text = trim(prelude).strip_prefix('@')?;
    let (name, rest) = word(text)?;
    name.eq_ignore_ascii_case("media").then(|| trim(rest))
}

/// Each container and leaf is charged, even when its condition does not match.
/// The only stack is an on-demand Vec of parent activity flags; its depth and
/// total work are bounded by the admitted block count and complete input bytes.
/// Unknown at-rules retain their existing opaque/top-level behavior.
pub(super) fn walk(source: &str, maximum_blocks: usize, environment: Option<MediaEnvironment>,
    mut visit: impl FnMut(&str, &str, bool) -> bool) -> Result<usize, String> {
    let text = super::without_comments(source);
    let bytes = text.as_bytes();
    let mut at = 0usize;
    let mut blocks = 0usize;
    let mut parents = Vec::new();
    let mut active = true;
    while at < bytes.len() {
        while bytes.get(at).is_some_and(|b| space(*b)) { at += 1; }
        if at == bytes.len() { break; }
        if bytes[at] == b'}' {
            if let Some(parent) = parents.pop() { active = parent; }
            at += 1;
            continue;
        }
        let start = at;
        let mut quote = 0u8;
        let mut parentheses = 0usize;
        let mut brackets = 0usize;
        while at < bytes.len() {
            let byte = bytes[at];
            if quote != 0 {
                if byte == b'\\' { at = (at + 2).min(bytes.len()); continue; }
                if byte == quote { quote = 0; }
            } else if matches!(byte, b'\'' | b'"') { quote = byte; }
            else if byte == b'(' { parentheses = parentheses.checked_add(1).ok_or("CSS media parenthesis overflow.")?; }
            else if byte == b')' { parentheses = parentheses.saturating_sub(1); }
            else if byte == b'[' { brackets = brackets.checked_add(1).ok_or("CSS media bracket overflow.")?; }
            else if byte == b']' { brackets = brackets.saturating_sub(1); }
            else if parentheses == 0 && brackets == 0 && matches!(byte, b'{' | b';' | b'}') { break; }
            at += 1;
        }
        if at == bytes.len() { break; }
        if bytes[at] != b'{' { if bytes[at] == b';' { at += 1; } continue; }
        let prelude = &text[start..at];
        at += 1;
        blocks = blocks.checked_add(1).filter(|count| *count <= maximum_blocks)
            .ok_or_else(|| format!("Complete stylesheet refused: document parser block allowance {maximum_blocks} reached."))?;
        if let Some(condition) = is_media(prelude) {
            if !visit(prelude, "", active) { return Ok(blocks); }
            parents.try_reserve(1).map_err(|_| "CSS conditional stack allocation failed.")?;
            parents.push(active);
            active = active && matches(condition, environment);
            continue;
        }
        let body_start = at;
        let mut depth = 1usize;
        quote = 0;
        while at < bytes.len() && depth != 0 {
            let byte = bytes[at];
            if quote != 0 {
                if byte == b'\\' { at = (at + 2).min(bytes.len()); continue; }
                if byte == quote { quote = 0; }
            } else if matches!(byte, b'\'' | b'"') { quote = byte; }
            else if byte == b'{' { depth = depth.checked_add(1).ok_or("CSS block nesting overflow.")?; }
            else if byte == b'}' { depth -= 1; }
            if depth != 0 { at += 1; }
        }
        let body_end = at;
        at = at.saturating_add(1).min(bytes.len());
        if !visit(prelude, &text[body_start..body_end], active) { return Ok(blocks); }
    }
    Ok(blocks)
}
