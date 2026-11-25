//! The represented alignment and whitespace modes share inherited defaults.
use super::*;
use custom_properties::{Budget, Variables};

pub(super) fn property(name: &str) -> bool {
    matches!(name, "text-align" | "white-space")
}

fn alignment(value: &str) -> Option<Align> {
    match value {
        "left" | "start" => Some(Align::Left),
        "center" => Some(Align::Center),
        "right" | "end" => Some(Align::Right),
        _ => None,
    }
}

fn whitespace(value: &str) -> Option<WhiteSpace> {
    match value {
        "normal" => Some(WhiteSpace::Normal),
        "nowrap" => Some(WhiteSpace::NoWrap),
        "pre" => Some(WhiteSpace::Pre),
        "pre-wrap" | "break-spaces" => Some(WhiteSpace::PreWrap),
        _ => None,
    }
}

pub(super) fn valid(name: &str, source: &str) -> bool {
    let value = source.trim().to_ascii_lowercase();
    if matches!(value.as_str(), "inherit" | "initial" | "unset") { return true; }
    match name {
        "text-align" => alignment(&value).is_some(),
        "white-space" => whitespace(&value).is_some(),
        _ => false,
    }
}

fn reset(style: &mut Style, parent: Option<&Style>, name: &str) {
    match name {
        "text-align" => {
            style.align = parent.map_or(Align::Left, |parent| parent.align);
            style.legacy_align = parent.and_then(|parent| parent.legacy_align);
        }
        "white-space" => style.white_space = parent.map_or(WhiteSpace::Normal, |parent| parent.white_space),
        _ => {}
    }
}

pub(super) fn literal(style: &mut Style, parent: Option<&Style>, name: &str, value: &str) -> bool {
    if matches!(value, "inherit" | "initial" | "unset") {
        reset(style, if value == "initial" { None } else { parent }, name);
        return true;
    }
    match name {
        "text-align" => alignment(value).is_some_and(|align| {
            style.align = align;
            style.legacy_align = None;
            true
        }),
        "white-space" => whitespace(value).is_some_and(|space| {
            style.white_space = space;
            true
        }),
        _ => false,
    }
}

fn parser_storage(source: &str, budget: &mut Budget) -> Result<(), ()> {
    // Reserve scan/token/String storage before token detection or lowercasing,
    // including expanded values whose size can exceed their short reference.
    budget.work(source.len().max(1).div_ceil(64).saturating_mul(512))?;
    budget.copy(source.len().saturating_mul(64))
}

/// Invalid substituted winners inherit; malformed literals preserve prior values.
/// A resource refusal must remain distinguishable from either kind of invalidity.
pub(super) fn apply(style: &mut Style, parent: &Style, declaration: &Declaration,
    variables: &Variables, budget: &mut Budget, has_element_parent: bool) -> Result<bool, ()> {
    let name = declaration.name.as_str();
    let parent = has_element_parent.then_some(parent);
    parser_storage(&declaration.value, budget)?;
    let variable = custom_properties::has_var(&declaration.value);
    let substituted;
    let source = if variable {
        substituted = match variables.substitute(&declaration.value, budget) {
            Ok(Some(value)) => value,
            Ok(None) => { reset(style, parent, name); return Ok(true); }
            Err(()) => {
                let (_, _, work_refused, copy_refused) = budget.usage();
                if work_refused || copy_refused { return Err(()); }
                reset(style, parent, name);
                return Ok(true);
            }
        };
        parser_storage(&substituted, budget)?;
        substituted.as_str()
    } else { declaration.value.as_str() };
    let value = source.trim().to_ascii_lowercase();
    let accepted = literal(style, parent, name, &value);
    if !accepted && variable {
        reset(style, parent, name);
        Ok(true)
    } else { Ok(accepted) }
}
