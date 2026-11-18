//! Finite grid item inline alignment in the ordinary ordered cascade.
//! https://www.w3.org/TR/css-align-3/#justify-self-property
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Auto,
    Normal,
    Stretch,
    Start,
    Center,
    End,
    /// A recognized unsupported winner or refused bounded substitution.
    Unavailable,
}

fn keyword(value: &str, item: bool) -> Option<Justify> {
    Some(match value {
        "auto" if item => Justify::Auto,
        "normal" => Justify::Normal,
        "stretch" => Justify::Stretch,
        "start" => Justify::Start,
        "center" => Justify::Center,
        "end" => Justify::End,
        _ => return None,
    })
}

fn unsupported(value: &str, item: bool) -> bool {
    let position = |word| {
        matches!(
            word,
            "start"
                | "center"
                | "end"
                | "self-start"
                | "self-end"
                | "flex-start"
                | "flex-end"
                | "left"
                | "right"
        )
    };
    let mut words = value.split_ascii_whitespace();
    let (Some(first), second, third) = (words.next(), words.next(), words.next()) else {
        return false;
    };
    if third.is_some() {
        return false;
    }
    if second.is_none()
        && matches!(
            first,
            "baseline" | "self-start" | "self-end" | "flex-start" | "flex-end" | "left" | "right"
        )
        || matches!(first, "first" | "last") && second == Some("baseline")
    {
        return true;
    }
    if matches!(first, "safe" | "unsafe") {
        return second.is_some_and(|word| position(word) || item && word == "normal");
    }
    !item
        && (first == "legacy"
            && second.is_none_or(|word| matches!(word, "left" | "right" | "center"))
            || matches!(first, "left" | "right" | "center") && second == Some("legacy"))
}

pub(super) fn apply(
    style: &mut Style,
    parent: &Style,
    declaration: &Declaration,
    inline: bool,
    sheet: &mut (Justify, Justify),
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> bool {
    let item = match declaration.name.as_str() {
        "justify-items" => false,
        "justify-self" => true,
        _ => return false,
    };
    let initial = if item { Justify::Auto } else { Justify::Normal };
    let substituted = custom_properties::has_var(&declaration.value);
    let text;
    let source = if substituted {
        match variables.substitute(&declaration.value, budget) {
            Ok(Some(value)) => {
                text = value;
                text.as_str()
            }
            Ok(None) => "unset",
            Err(()) => {
                let target = if item {
                    &mut style.justify_self
                } else {
                    &mut style.justify_items
                };
                *target = Justify::Unavailable;
                if !inline {
                    if item {
                        sheet.1 = Justify::Unavailable;
                    } else {
                        sheet.0 = Justify::Unavailable;
                    }
                }
                return true;
            }
        }
    } else {
        declaration.value.as_str()
    };
    let value = source.trim().to_ascii_lowercase();
    let computed = match value.as_str() {
        "inherit" => Some(if item {
            parent.justify_self
        } else {
            parent.justify_items
        }),
        "initial" | "unset" | "revert" => Some(initial),
        "revert-layer" => Some(if inline {
            if item {
                sheet.1
            } else {
                sheet.0
            }
        } else {
            initial
        }),
        _ => keyword(&value, item)
            .or_else(|| unsupported(&value, item).then_some(Justify::Unavailable))
            .or_else(|| substituted.then_some(initial)),
    };
    if let Some(value) = computed {
        if item {
            style.justify_self = value;
        } else {
            style.justify_items = value;
        }
        if !inline {
            if item {
                sheet.1 = value;
            } else {
                sheet.0 = value;
            }
        }
    }
    true
}
