//! Inherited visibility and HTML pointer eligibility in the ordered cascade.
use super::*;
pub(super) fn property(name: &str) -> bool {
    matches!(name, "visibility" | "pointer-events")
}
fn literal(name: &str, value: &str, parent: bool) -> Option<bool> {
    match value {
        "inherit" | "unset" | "revert" | "revert-layer" => return Some(parent),
        "initial" => return Some(true),
        _ => {}
    }
    match (name, value) {
        ("visibility", "visible") | ("pointer-events", "auto") => Some(true),
        ("visibility", "hidden") | ("pointer-events", "none") => Some(false),
        _ => None,
    }
}
pub(super) fn valid(name: &str, source: &str) -> bool {
    let value = source.trim_ascii().to_ascii_lowercase();
    literal(name, &value, true).is_some()
        || custom_properties::has_var(source) && custom_properties::valid_syntax(source)
}
pub(super) fn apply(
    style: &mut Style,
    parent: &Style,
    decl: &Declaration,
    inline: bool,
    sheet: &mut [bool; 2],
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> bool {
    if !property(&decl.name) {
        return false;
    }
    let index = usize::from(decl.name == "pointer-events");
    let inherited = if index == 0 {
        parent.visible
    } else {
        parent.pointer_events
    };
    let substituted = custom_properties::has_var(&decl.value);
    let text;
    let source = if substituted {
        match variables.substitute(&decl.value, budget) {
            Ok(Some(value)) => {
                text = value;
                text.as_str()
            }
            _ => "unset",
        }
    } else {
        decl.value.as_str()
    };
    let value = source.trim_ascii().to_ascii_lowercase();
    let result = if inline && value == "revert-layer" {
        Some(sheet[index])
    } else {
        literal(&decl.name, &value, inherited).or_else(|| substituted.then_some(inherited))
    };
    if let Some(value) = result {
        if index == 0 {
            style.visible = value
        } else {
            style.pointer_events = value
        }
        if !inline {
            sheet[index] = value;
        }
    }
    true
}
