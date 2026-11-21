//! Original touch-action value grammar and computed author cascade.
//! Direct-manipulation hardware acquisition remains outside this CSS increment.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TouchAction(u8);
impl TouchAction {
    pub(crate) const AUTO: Self = Self(32);
    pub(crate) fn as_str(self) -> &'static str {
        const VALUES: [&str; 34] = [
            "none",
            "pinch-zoom",
            "pan-up",
            "pan-up pinch-zoom",
            "pan-down",
            "pan-down pinch-zoom",
            "pan-y",
            "pan-y pinch-zoom",
            "pan-left",
            "pan-left pinch-zoom",
            "pan-left pan-up",
            "pan-left pan-up pinch-zoom",
            "pan-left pan-down",
            "pan-left pan-down pinch-zoom",
            "pan-left pan-y",
            "pan-left pan-y pinch-zoom",
            "pan-right",
            "pan-right pinch-zoom",
            "pan-right pan-up",
            "pan-right pan-up pinch-zoom",
            "pan-right pan-down",
            "pan-right pan-down pinch-zoom",
            "pan-right pan-y",
            "pan-right pan-y pinch-zoom",
            "pan-x",
            "pan-x pinch-zoom",
            "pan-x pan-up",
            "pan-x pan-up pinch-zoom",
            "pan-x pan-down",
            "pan-x pan-down pinch-zoom",
            "pan-x pan-y",
            "pan-x pan-y pinch-zoom",
            "auto",
            "manipulation",
        ];
        VALUES[self.0 as usize]
    }
}
fn parse(source: &str) -> Option<TouchAction> {
    let source = source.trim_matches([' ', '\t', '\r', '\n', '\u{c}']);
    for (name, value) in [("auto", 32), ("none", 0), ("manipulation", 33)] {
        if source.eq_ignore_ascii_case(name) {
            return Some(TouchAction(value));
        }
    }
    let mut bits = 0;
    for token in source
        .split(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}'))
        .filter(|token| !token.is_empty())
    {
        let (mask, value) = if token.eq_ignore_ascii_case("pan-x") {
            (24, 24)
        } else if token.eq_ignore_ascii_case("pan-left") {
            (24, 8)
        } else if token.eq_ignore_ascii_case("pan-right") {
            (24, 16)
        } else if token.eq_ignore_ascii_case("pan-y") {
            (6, 6)
        } else if token.eq_ignore_ascii_case("pan-up") {
            (6, 2)
        } else if token.eq_ignore_ascii_case("pan-down") {
            (6, 4)
        } else if token.eq_ignore_ascii_case("pinch-zoom") {
            (1, 1)
        } else {
            return None;
        };
        if bits & mask != 0 {
            return None;
        }
        bits |= value;
    }
    (bits != 0).then_some(TouchAction(bits))
}
/// Canonical finite specified text; variable syntax retains its original case.
pub(crate) fn specified(source: &str) -> Option<&str> {
    let text = source.trim_matches([' ', '\t', '\r', '\n', '\u{c}']);
    for keyword in ["inherit", "initial", "unset", "revert", "revert-layer"] {
        if text.eq_ignore_ascii_case(keyword) {
            return Some(keyword);
        }
    }
    if custom_properties::has_var(text) {
        return custom_properties::valid_syntax(text).then_some(text);
    }
    parse(text).map(TouchAction::as_str)
}
pub(super) fn apply(
    current: &mut Option<TouchAction>,
    sheet: &mut Option<TouchAction>,
    parent: Option<TouchAction>,
    declaration: &Declaration,
    inline: bool,
    variables: &custom_properties::Variables,
    budget: &mut custom_properties::Budget,
) {
    let substituted;
    let source = if custom_properties::has_var(&declaration.value) {
        substituted = match variables.substitute(&declaration.value, budget) {
            Ok(Some(value)) => value,
            Ok(None) => {
                *current = Some(TouchAction::AUTO);
                if !inline {
                    *sheet = *current;
                }
                return;
            }
            Err(()) => {
                *current = None;
                if !inline {
                    *sheet = None;
                }
                return;
            }
        };
        substituted.as_str()
    } else {
        declaration.value.as_str()
    };
    *current = if source.trim().eq_ignore_ascii_case("inherit") {
        parent
    } else if source.trim().eq_ignore_ascii_case("revert-layer") && inline {
        *sheet
    } else {
        Some(parse(source).unwrap_or(TouchAction::AUTO))
    };
    if !inline {
        *sheet = *current;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn touch_action_keyword_groups_canonicalize_and_reject_invalid_combinations() {
        for (input, expected) in [
            ("NONE", "none"),
            (" pan-Y pan-X ", "pan-x pan-y"),
            (
                "pinch-zoom PAN-down pan-right",
                "pan-right pan-down pinch-zoom",
            ),
            ("manipulation", "manipulation"),
            ("pinch-zoom", "pinch-zoom"),
        ] {
            assert_eq!(specified(input), Some(expected));
        }
        for input in [
            "",
            "auto none",
            "pan-x pan-left",
            "pan-up pan-down",
            "pinch-zoom pinch-zoom",
            "none pinch-zoom",
            "pan-x,pan-y",
            "pan-x\u{b}pan-y",
            "inherit pan-y",
            "calc(1)",
        ] {
            assert_eq!(specified(input), None, "{input}");
        }
        assert_eq!(specified("var(--Case)"), Some("var(--Case)"));
        for code in 0..34 {
            assert_eq!(parse(TouchAction(code).as_str()), Some(TouchAction(code)));
        }
    }
}

#[cfg(test)]
mod cascade_tests {
    use super::*;
    #[test]
    fn touch_action_real_cascade_handles_priority_variables_inheritance_and_invalid_winners() {
        let doc=crate::html::parse("<style>#parent{touch-action:pan-y;--Pan:pan-right pinch-zoom}.pick{touch-action:none!important}#invalid{touch-action:pan-left;touch-action:pan-x pan-left}#var{touch-action:var(--Pan)}#inherit{touch-action:inherit}#unset{touch-action:unset}#rollback{touch-action:pan-down}</style><div id=parent><div id=normal></div><div id=inherit></div><div id=unset></div><div id=var></div><div id=invalid></div><div id=priority class=pick style='touch-action:manipulation'></div><div id=rollback style='touch-action:revert-layer'></div><div id=badvar style='touch-action:none;touch-action:var(--absent)'></div></div>");
        let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
        for (id, expected) in [
            ("parent", "pan-y"),
            ("normal", "auto"),
            ("inherit", "pan-y"),
            ("unset", "auto"),
            ("var", "pan-right pinch-zoom"),
            ("invalid", "pan-left"),
            ("priority", "none"),
            ("rollback", "pan-down"),
            ("badvar", "auto"),
        ] {
            let node = (0..doc.nodes.len())
                .find(|&n| doc.attr(n, "id") == Some(id))
                .unwrap();
            assert_eq!(
                result
                    .computed_properties
                    .read_touch_action(node)
                    .unwrap()
                    .unwrap()
                    .as_str(),
                expected,
                "{id}"
            );
        }
    }
}
