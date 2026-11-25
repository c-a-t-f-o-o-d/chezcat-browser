//! Finite inherited font values share the native family and shorthand parsers.
use super::*;
use custom_properties::{Budget, Variables};

pub(super) fn property(name: &str) -> bool {
    matches!(name, "font" | "font-family" | "font-weight" | "font-style")
}

/// Literal admission for the represented longhands. Family tokens retain case,
/// quotes and commas; var-looking text inside a quoted family is ordinary data.
pub(super) fn valid(name: &str, source: &str) -> bool {
    let value = source.trim().to_ascii_lowercase();
    if matches!(value.as_str(), "inherit" | "initial" | "unset") { return true; }
    match name {
        "font-family" => font_families(source).is_some(),
        "font-weight" => font_weight(&value, 400).is_some(),
        "font-style" => matches!(value.as_str(), "normal" | "italic" | "oblique"),
        _ => false,
    }
}

fn parser_storage(source: &str, budget: &mut Budget) -> Result<(), ()> {
    // The declaration parser bounds input to 2048 bytes. Reserve the same
    // conservative scan/token/String allowance as CssBudget::source, including
    // the expanded text: a short var() reference can produce a much larger font.
    budget.work(source.len().max(1).div_ceil(64).saturating_mul(512))?;
    budget.copy(source.len().saturating_mul(64))
}

fn reset(style: &mut Style, parent: Option<&Style>, name: &str,
    budget: &mut Budget, known: &mut bool, parent_known: bool, available: bool) -> Result<bool, ()> {
    let whole = name == "font";
    if whole || name == "font-family" {
        // Node-level inheritance does not cover this additional declaration
        // copy. Pay for every Vec entry and each owned family before cloning.
        let bytes = parent.map_or(5 + std::mem::size_of::<String>(), |parent| {
            parent.font_families.iter().fold(0usize, |bytes, family| {
                bytes.saturating_add(family.len()).saturating_add(std::mem::size_of::<String>())
            })
        });
        if budget.copy(bytes).is_err() {
            if whole { *known = false; }
            return Err(());
        }
        style.font_families = parent.map_or_else(|| vec!["serif".into()], |parent| parent.font_families.clone());
    }
    if whole || name == "font-weight" {
        style.font_weight = parent.map_or(400, |parent| parent.font_weight);
        style.bold = style.font_weight >= 600;
    }
    if whole || name == "font-style" {
        style.italic = parent.is_some_and(|parent| parent.italic);
    }
    if whole {
        style.font_size = parent.map_or(INITIAL_FONT_SIZE, |parent| parent.font_size);
        style.line_height = parent.map_or(LineHeight::Factor(1.5), |parent| parent.line_height);
        *known = available && (parent.is_none() || parent_known);
    }
    Ok(true)
}

/// Apply an accepted font declaration atomically. Invalid substituted winners
/// use inherited unset; malformed literals leave the previous declaration alone.
/// A refused substitution is distinct from a missing/cyclic computed value.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(style: &mut Style, parent: &Style, declaration: &Declaration,
    variables: &Variables, budget: &mut Budget, has_element_parent: bool,
    known: &mut bool, parent_known: bool, root_known: bool) -> Result<bool, ()> {
    let name = declaration.name.as_str();
    let parent = has_element_parent.then_some(parent);
    let parent_known = parent.is_none() || parent_known;
    if parser_storage(&declaration.value, budget).is_err() {
        if name == "font" { *known = false; }
        return Err(());
    }
    let variable = custom_properties::has_var(&declaration.value);
    let substituted;
    let source = if variable {
        substituted = match variables.substitute(&declaration.value, budget) {
            Ok(Some(value)) => value,
            Ok(None) => return reset(style, parent, name, budget, known, parent_known, true),
            Err(()) => {
                let (_, _, work_refused, copy_refused) = budget.usage();
                if work_refused || copy_refused {
                    if name == "font" { *known = false; }
                    return Err(());
                }
                return reset(style, parent, name, budget, known, parent_known, false);
            }
        };
        if parser_storage(&substituted, budget).is_err() {
            if name == "font" { *known = false; }
            return Err(());
        }
        substituted.as_str()
    } else { declaration.value.as_str() };
    let source = source.trim();
    let value = source.to_ascii_lowercase();
    if matches!(value.as_str(), "inherit" | "initial" | "unset") {
        let source_parent = if value == "initial" { None } else { parent };
        return reset(style, source_parent, name, budget, known, parent_known, true);
    }
    let parent_weight = parent.map_or(400, |parent| parent.font_weight);
    let accepted = match name {
        "font-family" => font_families(source).is_some_and(|families| {
            style.font_families = families;
            true
        }),
        "font-weight" => font_weight(&value, parent_weight).is_some_and(|weight| {
            style.font_weight = weight;
            style.bold = weight >= 600;
            true
        }),
        "font-style" => match value.as_str() {
            "normal" => { style.italic = false; true },
            "italic" | "oblique" => { style.italic = true; true },
            _ => false,
        },
        "font" => font_shorthand(source, parent_weight).is_some_and(|(size, line, families, weight, italic)| {
            let parent_size = parent.map_or(INITIAL_FONT_SIZE, |parent| parent.font_size);
            let Some(size_value) = font_size_value(&size, parent_size, style.root_font_size) else { return false; };
            let parent_line = parent.map_or(LineHeight::Factor(1.5), |parent| parent.line_height);
            let leading = line.as_deref().and_then(|line| line_height::literal(line, parent_line))
                .unwrap_or(LineHeight::Factor(1.5));
            style.font_size = size_value;
            style.font_families = families;
            style.font_weight = weight;
            style.bold = weight >= 600;
            style.italic = italic;
            style.line_height = leading;
            *known = font_size_available(&size, parent_known, root_known);
            true
        }),
        _ => false,
    };
    if !accepted && variable {
        reset(style, parent, name, budget, known, parent_known, true)
    } else { Ok(accepted) }
}
