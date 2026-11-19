//! Finite single-layer color clipping, using the existing ordered cascade.
//! https://www.w3.org/TR/css-backgrounds-3/#background-clip
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundClip {
    BorderBox,
    PaddingBox,
    ContentBox,
}

fn keyword(value: &str) -> Option<BackgroundClip> {
    if value.eq_ignore_ascii_case("border-box") {
        Some(BackgroundClip::BorderBox)
    } else if value.eq_ignore_ascii_case("padding-box") {
        Some(BackgroundClip::PaddingBox)
    } else if value.eq_ignore_ascii_case("content-box") {
        Some(BackgroundClip::ContentBox)
    } else {
        None
    }
}

/// Only accepted forms of the existing color-only background shorthand reset
/// clip. In particular, an unsupported literal URL shorthand is not a reset.
pub(super) fn resets_background(value: &str) -> bool {
    matches!(
        value,
        "inherit" | "initial" | "unset" | "transparent" | "none" | "currentcolor"
    ) || opaque_background_color(value).is_some()
}

/// The sheet checkpoint excludes element-attached declarations. This supports
/// inline revert-layer without a second variable substitution or cascade pass.
pub(super) fn apply(
    style: &mut Style,
    parent: &Style,
    declaration: &Declaration,
    is_inline: bool,
    sheet: &mut Option<BackgroundClip>,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> bool {
    if declaration.name != "background-clip" {
        return false;
    }
    let has_var = custom_properties::has_var(&declaration.value);
    let substituted;
    let source = if has_var {
        match variables.substitute(&declaration.value, budget) {
            Ok(Some(value)) => {
                substituted = value;
                substituted.as_str()
            }
            Ok(None) => "initial",
            Err(()) => {
                style.background_clip = None;
                if !is_inline {
                    *sheet = None;
                }
                return true;
            }
        }
    } else {
        &declaration.value
    };
    let value = source.trim();
    let clip = if let Some(clip) = keyword(value) {
        Some(Some(clip))
    } else if value.eq_ignore_ascii_case("inherit") {
        Some(parent.background_clip)
    } else if ["initial", "unset", "revert"]
        .iter()
        .any(|word| value.eq_ignore_ascii_case(word))
    {
        Some(Some(BackgroundClip::BorderBox))
    } else if value.eq_ignore_ascii_case("revert-layer") {
        Some(if is_inline {
            *sheet
        } else {
            Some(BackgroundClip::BorderBox)
        })
    } else if value.eq_ignore_ascii_case("text")
        || value.split(',').count() > 1
            && value.split(',').all(|part| keyword(part.trim()).is_some())
    {
        // Retain a recognized but unsupported winner, rather than resurrecting
        // an earlier supported value. This slice does not clip image/text or
        // multiple background layers.
        Some(None)
    } else if has_var {
        Some(Some(BackgroundClip::BorderBox))
    } else {
        None
    };
    if let Some(clip) = clip {
        style.background_clip = clip;
        if !is_inline {
            *sheet = clip;
        }
    }
    true
}

#[cfg(test)]
#[path = "css_background_clip_tests.rs"]
mod tests;
