//! Physical margin and padding lengths share guarded typed sizing arithmetic.
use super::{border_parts, four, parse_length, Length};

pub(super) fn length(value:&str,margin:bool)->Option<Length>{let value=value.trim_ascii();if value.to_ascii_lowercase().starts_with("calc("){return super::sizing_math::length(value,margin);}if value.chars().any(char::is_whitespace){return None;}parse_length(value,margin,margin)}

pub(super) fn shorthand(value: &str, margin: bool) -> Option<[Length; 4]> {
    let parts = border_parts(value)?;
    let mut values = [Length::Px(0.0); 4];
    for (slot, part) in values.iter_mut().zip(&parts) {
        *slot = length(part, margin)?;
    }
    four(&values[..parts.len()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::{compute, Style};
    use std::collections::HashMap;

    fn computed(source: &str) -> Style {
        let doc = crate::html::parse(&format!(
            "<html><head><style>{source}</style></head><body><div id=parent><div id=test></div></div></body></html>"));
        let id = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("test")).unwrap();
        compute(&doc, &HashMap::new()).swap_remove(id)
    }

    #[test]
    fn box_spacing_products_keep_number_type_units_and_property_ranges() {
        assert_eq!(length("calc(1.5rem * .5)", false), Some(Length::Rem(0.75)));
        assert_eq!(length("calc(-.5 * 1.5rem)", true), Some(Length::Rem(-0.75)));
        assert_eq!(length("calc(25%*2e0)", true), Some(Length::Percent(50.0)));
        assert_eq!(length("calc(-2em * -.25)", false), Some(Length::Em(0.5)));
        assert_eq!(length("calc(0 * 3px)", false), Some(Length::Px(0.0)));
        assert_eq!(length("calc(2 * 3pt)", true), Some(Length::Px(8.0)));
        assert_eq!(length("calc(.25 * 4pc)", true), Some(Length::Px(16.0)));
        assert_eq!(length("calc(-3px * 2)", false), Some(Length::Px(0.0)));
        assert_eq!(length("-3px", false), None);
        assert_eq!(length("calc(20000px * .25)", true), Some(Length::Px(5000.0)));
        assert_eq!(length("calc(9000px * 2)", true), Some(Length::Px(10000.0)));
        let style = computed("#test{margin:Auto CaLc(-.5 * 1.5REM) 1px; padding:calc(25% * .5) calc(2 * .5em)}");
        assert_eq!(style.margin, [Length::Auto, Length::Rem(-0.75), Length::Px(1.0), Length::Rem(-0.75)]);
        assert_eq!(style.padding, [Length::Percent(12.5), Length::Em(1.0), Length::Percent(12.5), Length::Em(1.0)]);
    }

    #[test]
    fn box_spacing_rejects_other_types_operators_and_malformed_components() {
        for value in ["calc(-1 * 0)", "calc(2 * 3)", "calc(2px * 3%)", "calc(2 * 1vh)",
            "calc(2 * auto)", "calc(1rem * 2px * 3)", "calc(1px + 2)", "calc(1px / 0)",
            "calc(calc(1px * 2px) * 2)", "calc(1. * 2px)", "calc(1e * 2px)", "calc(1px * nan)",
            "calc(1e999 * 0px)", "calc(2 * 3 px)", "calc(2 * 3\u{00a0}px)",
            "calc(2 * 3px)more", "calc(2 * 3px", "calc(2 * )", "0 px"] {
            assert_eq!(length(value, true), None, "{value}");
        }
        for (source,expected) in [("calc(1rem * 2 * 3)",Length::Rem(6.0)),("calc(1px + 2px)",Length::Px(3.0)),("calc(1px / 2)",Length::Px(0.5)),("calc(calc(1px * 2) * 2)",Length::Px(4.0))]{assert_eq!(length(source,true),Some(expected),"{source}");}
        assert!(length(&format!("calc({}1px*2)", " ".repeat(2048)), true).is_none());
        assert!(shorthand("1px 2px 3px 4px 5px", true).is_none());
        assert!(shorthand(&"calc(".repeat(9), false).is_none());
        let style = computed("#test{padding:3px;margin:4px;padding:10px calc(2px*2px);margin:calc(2*1vh);padding-left:-4px}");
        assert_eq!(style.padding, [Length::Px(3.0); 4]);
        assert_eq!(style.margin, [Length::Px(4.0); 4]);
    }

    #[test]
    fn box_spacing_variables_expand_with_token_boundaries_and_targeted_resets() {
        let style = computed("#parent{--Gutter:1.5rem;--half:.5;--n:7;--zero:0;margin:6px 8px} \
            #test{margin:calc(-.5*var(--Gutter));padding:calc(var(--Gutter)*var(--half)); \
            margin-top:var(--zero);margin-right:var(--missing,inherit);margin-bottom:calc(2*var(--n)px); \
            padding-right:calc(2px*var(--missing));padding-bottom:calc(2.5 * -2px)}");
        assert_eq!(style.margin, [Length::Px(0.0), Length::Px(8.0), Length::Px(0.0), Length::Rem(-0.75)]);
        assert_eq!(style.padding, [Length::Rem(0.75), Length::Px(0.0), Length::Px(0.0), Length::Rem(0.75)]);
        let reset = computed("#test{--two:2;--bad:3vw; margin:8px; margin:3px calc(var(--two)*var(--bad)); \
            margin-left:4px; padding:9px; padding-left:var(--two)px; border:3px solid red}");
        assert_eq!(reset.margin, [Length::Px(0.0),Length::Px(0.0),Length::Px(0.0),Length::Px(4.0)]);
        assert_eq!(reset.padding, [Length::Px(9.0),Length::Px(9.0),Length::Px(9.0),Length::Px(0.0)]);
        assert_eq!(reset.border_width, [Length::Px(3.0); 4]);
    }

    #[test]
    fn box_spacing_math_leaves_other_property_grammars_intact() {
        let style = computed("#test{width:31px;width:calc(3px*2);top:7px;top:calc(3px*2); \
            gap:5px;gap:calc(3px*2);border:3px solid red;border-width:calc(3px*2); \
            font-size:12px;font-size:calc(3px*2);padding-left:calc(3px*2)}");
        assert_eq!(style.width, Length::Px(6.0));
        assert_eq!(style.inset[0], Length::Px(7.0));
        assert_eq!(style.gap, [Length::Px(6.0); 2]);
        assert_eq!(style.border_width, [Length::Px(3.0); 4]);
        assert_eq!(style.font_size, 12.0);
        assert_eq!(style.padding[3], Length::Px(6.0));
    }
}
