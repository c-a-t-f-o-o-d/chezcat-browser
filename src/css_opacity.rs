//! Computed opacity from the renderer's ordered author declarations.
//! https://www.w3.org/TR/css-color-4/#transparency
//! This scalar does not implement group painting or stacking contexts.
use super::*;

#[derive(Clone, Copy)]
enum Value {
    Number(f64),
    Initial(&'static str),
    Inherit,
    RevertLayer,
    Unsupported,
}

fn value(source: &str) -> Option<Value> {
    let source = source.trim_matches(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{000c}'));
    for keyword in ["initial", "unset", "revert"] {
        if source.eq_ignore_ascii_case(keyword) {
            return Some(Value::Initial(keyword));
        }
    }
    if source.eq_ignore_ascii_case("inherit") {
        return Some(Value::Inherit);
    }
    if source.eq_ignore_ascii_case("revert-layer") {
        return Some(Value::RevertLayer);
    }
    if source.contains('(') {
        return translate::math_value(source)
            .ok()
            .map(|_| Value::Unsupported);
    }
    let end = translate::number_end(source)?;
    let percentage = &source[end..] == "%";
    if end != source.len() && !percentage {
        return None;
    }
    let mut number = source[..end].parse::<f64>().ok()?;
    if percentage {
        number /= 100.0;
    }
    Some(Value::Number(if number == 0.0 { 0.0 } else { number }))
}

pub(crate) enum Specified<'a> {
    Number(f64),
    Text(&'a str),
    Unsupported,
}

impl Specified<'_> {
    pub(crate) fn serialize(&self) -> Result<String, String> {
        match self {
            Self::Number(n) if n.is_finite() => Ok(n.to_string()),
            Self::Text(text) => Ok((*text).to_owned()),
            _ => Err("The specified opacity exceeds the supported numeric serialization.".into()),
        }
    }
}

pub(crate) fn specified(source: &str) -> Option<Specified<'_>> {
    match value(source) {
        Some(Value::Number(n)) => Some(Specified::Number(n)),
        Some(Value::Initial(s)) => Some(Specified::Text(s)),
        Some(Value::Inherit) => Some(Specified::Text("inherit")),
        Some(Value::RevertLayer) => Some(Specified::Text("revert-layer")),
        _ if custom_properties::has_var(source) => Some(Specified::Text(source.trim())),
        Some(Value::Unsupported) => Some(Specified::Unsupported),
        None => None,
    }
}

pub(super) fn valid_declaration(source: &str) -> bool {
    value(source).is_some() || custom_properties::has_var(source)
}

/// The existing priority-ordered pass supplies the stylesheet checkpoint.
/// Inline revert-layer rolls back element-attached declarations. Other author
/// rollback reaches initial 1 in the currently supported unlayered origin.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    style: &mut Style,
    declaration: &Declaration,
    inline: bool,
    sheet: &mut (f64, bool),
    parent: (f64, bool),
    known: &mut bool,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) {
    let substituted = custom_properties::has_var(&declaration.value);
    let parsed = if substituted {
        match variables.substitute(&declaration.value, budget) {
            Ok(Some(text)) => value(&text).or(Some(Value::Initial("initial"))),
            Ok(None) => Some(Value::Initial("initial")),
            Err(()) => Some(Value::Unsupported),
        }
    } else {
        value(&declaration.value)
    };
    let computed = match parsed {
        Some(Value::Number(number)) => (number.clamp(0.0, 1.0), true),
        Some(Value::Initial(_)) => (1.0, true),
        Some(Value::Inherit) => parent,
        Some(Value::RevertLayer) if inline => *sheet,
        Some(Value::RevertLayer) => (1.0, true),
        Some(Value::Unsupported) => (1.0, false),
        None => return,
    };
    (style.opacity, *known) = computed;
    if !inline {
        *sheet = computed;
    }
}

/// CSSOM number serialization uses at most six decimal places, no exponent,
/// and no unnecessary trailing zeroes. Keep all formatting on the stack until
/// the caller has charged the exact owned result length.
pub(crate) fn serialize(value: f64) -> Option<([u8; 8], usize)> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    let scaled = (value * 1_000_000.0).round() as u32;
    let mut bytes = [b'0'; 8];
    if scaled == 0 {
        return Some((bytes, 1));
    }
    if scaled == 1_000_000 {
        bytes[0] = b'1';
        return Some((bytes, 1));
    }
    bytes[1] = b'.';
    let mut remainder = scaled;
    for slot in bytes[2..].iter_mut().rev() {
        *slot = b'0' + (remainder % 10) as u8;
        remainder /= 10;
    }
    let mut length = bytes.len();
    while bytes[length - 1] == b'0' {
        length -= 1;
    }
    Some((bytes, length))
}

#[cfg(test)]
#[path = "css_opacity_tests.rs"]
mod tests;
