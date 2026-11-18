//! Computed position from the renderer's already ordered author declarations.
//! https://www.w3.org/TR/css-position-3/#position-property
//! https://www.w3.org/TR/css-cascade-5/#revert-layer
use super::*;

impl Position {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Relative => "relative",
            Self::Absolute => "absolute",
            Self::Fixed => "fixed",
            Self::Sticky => "sticky",
        }
    }
}

#[derive(Clone, Copy)]
enum Value {
    Keyword(Position),
    Initial,
    Inherit,
    Revert,
    RevertLayer,
}

fn value(source: &str) -> Option<Value> {
    let source = source.trim();
    const VALUES: [(&str, Value); 10] = [
        ("static", Value::Keyword(Position::Static)),
        ("relative", Value::Keyword(Position::Relative)),
        ("absolute", Value::Keyword(Position::Absolute)),
        ("fixed", Value::Keyword(Position::Fixed)),
        ("sticky", Value::Keyword(Position::Sticky)),
        ("inherit", Value::Inherit),
        ("initial", Value::Initial),
        ("unset", Value::Initial),
        ("revert", Value::Revert),
        ("revert-layer", Value::RevertLayer),
    ];
    VALUES
        .iter()
        .find(|(keyword, _)| source.eq_ignore_ascii_case(keyword))
        .map(|(_, value)| *value)
}

/// Invalid literal declarations never become cascade candidates. A valid
/// var() declaration instead retains its priority until computed-value time.
pub(super) fn valid_declaration(source: &str) -> bool {
    value(source).is_some() || custom_properties::has_var(source)
}

/// Entries arrive highest priority first from the one real selector/cascade.
/// Only actual winner/rollback candidates substitute variables. There are no
/// supported author layers, user sheets, animations or UA position overrides;
/// unlayered sheet rollback therefore reaches UA static. Element-attached
/// rollback removes inline declarations while preserving stylesheet winners,
/// including stylesheet !important below inline !important revert-layer.
pub(super) fn cascade<'a>(
    parent: Position,
    parent_known: bool,
    declarations: impl Iterator<Item = (&'a Declaration, bool)>,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) -> (Position, bool) {
    let mut skip_inline = false;
    for (declaration, is_inline) in declarations {
        if skip_inline && is_inline {
            continue;
        }
        let substituted = custom_properties::has_var(&declaration.value);
        let parsed = if substituted {
            match variables.substitute(&declaration.value, budget) {
                Ok(Some(text)) => value(&text),
                Ok(None) => return (Position::Static, true),
                Err(()) => return (Position::Static, false),
            }
        } else {
            value(&declaration.value)
        };
        match parsed {
            Some(Value::Keyword(position)) => return (position, true),
            Some(Value::Inherit) => return (parent, parent_known),
            Some(Value::Initial | Value::Revert) => return (Position::Static, true),
            Some(Value::RevertLayer) if is_inline => skip_inline = true,
            Some(Value::RevertLayer) => return (Position::Static, true),
            None if substituted => return (Position::Static, true),
            None => {}
        }
    }
    (Position::Static, true)
}
