//! Finite line-height values in the renderer's ordered declaration pass.
//! Unitless numbers inherit as numbers; font-relative lengths inherit after
//! computation against the declaring element's final font metrics.
use super::*;

pub(super) fn literal(value: &str, parent: LineHeight) -> Option<LineHeight> {
    match value {
        "normal" | "initial" => return Some(LineHeight::Factor(1.5)),
        "inherit" | "unset" | "revert" | "revert-layer" => return Some(parent),
        _ => {}
    }
    // Substitution cannot manufacture a dimension by adjoining separate tokens,
    // e.g. var(--number)px. Only the existing single-token unit grammar is used.
    if !value.is_ascii() || value.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return None;
    }
    if let Ok(number) = value.parse::<f32>() {
        return (number.is_finite() && number >= 0.0)
            .then_some(LineHeight::Factor(number.min(20.0)));
    }
    parse_length(value, false, false).map(LineHeight::Length)
}

/// A stylesheet-only checkpoint permits element-attached revert-layer without
/// a second substitution or cascade implementation. Accepted font shorthand
/// resets are included by the same forward pass in css.rs.
pub(super) fn apply(
    style: &mut Style,
    parent: &Style,
    declaration: &Declaration,
    inline: bool,
    sheet: &mut LineHeight,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> bool {
    if declaration.name != "line-height" {
        return false;
    }
    let substituted = custom_properties::has_var(&declaration.value);
    let text;
    let source = if substituted {
        match variables.substitute(&declaration.value, budget) {
            Ok(Some(value)) => {
                text = value;
                text.as_str()
            }
            // This inherited property uses unset at invalid computed value
            // time. Refused finite expansion uses the existing inherited
            // renderer fallback, without reviving an earlier declaration.
            Ok(None) | Err(()) => "unset",
        }
    } else {
        declaration.value.as_str()
    };
    let value = source.trim().to_ascii_lowercase();
    let computed = if inline && value == "revert-layer" {
        Some(*sheet)
    } else {
        literal(&value, parent.line_height).or_else(|| substituted.then_some(parent.line_height))
    };
    if let Some(value) = computed {
        style.line_height = value;
        if !inline {
            *sheet = value;
        }
    }
    true
}

pub(super) fn computed(value: LineHeight, font: f32, root_font: f32) -> LineHeight {
    match value {
        LineHeight::Factor(_) => value,
        LineHeight::Length(length) => LineHeight::Length(Length::Px(
            length.resolve(font, font, root_font).unwrap_or(font * 1.5),
        )),
    }
}

#[cfg(test)]
#[path = "css_line_height_tests.rs"]
mod tests;
