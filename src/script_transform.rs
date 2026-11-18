//! Specified transform values in an inline CSS declaration block.
//! https://drafts.csswg.org/cssom/#dom-cssstyledeclaration-setproperty
use super::*;
use crate::css::TransformSyntax;

pub(super) enum Declaration {
    Absent,
    Value(String),
    Unsupported,
}

pub(super) fn normalize(
    value: &str,
    state: &mut State,
) -> Result<Result<String, TransformSyntax>, String> {
    let (work, copies) = crate::css::transform_source_cost(value.len());
    state.work(work)?;
    state.copies(copies)?;
    Ok(crate::css::normalize_transform(value))
}

/// Retain an unsupported winning declaration so a later removal can still
/// distinguish it from an absent declaration.
pub(super) fn declaration(
    style: &str,
    state: &mut State,
) -> Result<Declaration, String> {
    let mut winner = Declaration::Absent;
    let mut important = false;
    for item in crate::css::declaration_items(style) {
        let Some((name, raw)) = item.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("transform") {
            continue;
        }
        let (value, priority) = crate::css::declaration_priority(raw);
        if value.trim().is_empty() || important && !priority {
            continue;
        }
        winner = match normalize(value, state)? {
            Ok(value) => Declaration::Value(value),
            Err(TransformSyntax::Unsupported) => Declaration::Unsupported,
            Err(TransformSyntax::Invalid) => continue,
        };
        important = priority;
    }
    Ok(winner)
}

pub(super) fn specified(style: &str, state: &mut State) -> Result<String, String> {
    match declaration(style, state)? {
        Declaration::Value(value) => Ok(value),
        Declaration::Unsupported => {
            Err("The specified transform exceeds the supported translation grammar.".into())
        }
        Declaration::Absent => Ok(String::new()),
    }
}
