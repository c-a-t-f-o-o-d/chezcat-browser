use super::*;

fn computed(sheet: &str, body: &str) -> (Document, Vec<Style>) {
    let doc = crate::html::parse(&format!("<html><head><style>{sheet}</style></head><body>{body}</body></html>"));
    let styles = compute(&doc, &HashMap::new());
    (doc, styles)
}

fn at<'a>(doc: &Document, styles: &'a [Style], id: &str) -> &'a Style {
    &styles[(0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(id)).unwrap()]
}

#[test]
fn css_radius_cascade_variables_final_fonts_and_noninheritance() {
    let (doc, styles) = computed(
        "html{font-size:20px}#parent{font-size:24px;border-radius:.5em}#rem{border-radius:.4rem;font-size:40px}#em{border-radius:.5em;font-size:16px}#var{--r:8px;border-radius:var(--r)}#invalid{border-radius:8px;border-radius:-1px;border-radius:1_px;border-radius:1.px}#invalid-var{border-radius:8px;--r:potato;border-radius:var(--r)}#zero{border-radius:8px;border-radius:-0e5}#inherit{font-size:8px;border-radius:inherit}#unset{border-radius:8px;border-radius:unset}",
        "<div id=parent><div id=child></div><div id=inherit></div></div><div id=rem></div><div id=em></div><div id=var></div><div id=invalid></div><div id=invalid-var></div><div id=zero></div><div id=unset></div>",
    );
    for (id, value) in [("parent", Some(12.0)), ("inherit", Some(12.0)), ("rem", Some(8.0)),
        ("em", Some(8.0)), ("var", Some(8.0)), ("invalid", Some(8.0)),
        ("child", None), ("invalid-var", None), ("zero", None), ("unset", None)] {
        assert_eq!(at(&doc, &styles, id).circular_radius(), value, "{id}");
    }
    assert_eq!(at(&doc, &styles, "invalid-var").border_radius, [Some(Length::Px(0.0)); 4]);
}

#[test]
fn css_radius_shorthand_longhand_priorities_rollback_and_finite_boundaries() {
    let (doc, styles) = computed(
        "#priority{border-top-left-radius:8px!important;border-radius:4px}#restored{border-radius:4px;border-top-left-radius:8px;border-top-right-radius:8px;border-bottom-right-radius:8px;border-bottom-left-radius:8px}#different{border-radius:8px;border-bottom-right-radius:3px}#ellipse{border-radius:8px;border-top-left-radius:8px 4px}#percent{border-radius:8px;border-radius:50%}#equal{border-radius:8px 8px 8px 8px / 8px}#rollback{border-radius:8px}#unlayered{border-radius:8px;border-radius:revert-layer}",
        "<div id=priority></div><div id=restored></div><div id=different></div><div id=ellipse></div><div id=percent></div><div id=equal></div><div id=rollback style='border-radius:12px;border-radius:revert-layer'></div><div id=unlayered></div>",
    );
    for id in ["restored", "equal", "rollback"] { assert_eq!(at(&doc, &styles, id).circular_radius(), Some(8.0), "{id}"); }
    for id in ["priority", "different", "ellipse", "percent", "unlayered"] { assert_eq!(at(&doc, &styles, id).circular_radius(), None, "{id}"); }
    assert_eq!(at(&doc, &styles, "priority").border_radius,
        [Some(Length::Px(8.0)), Some(Length::Px(4.0)), Some(Length::Px(4.0)), Some(Length::Px(4.0))]);
    assert_eq!(at(&doc, &styles, "percent").border_radius, [None; 4]);
}

#[test]
fn css_radius_refused_substitution_never_becomes_a_supported_circle() {
    let refused = Declaration { name: "border-radius".into(), value: "var(--missing,8px)".into(), important: false, stylesheet: None };
    let valid = Declaration { name: "border-radius".into(), value: "6px".into(), important: false, stylesheet: None };
    let variables = custom_properties::Variables::default();
    let mut budget = custom_properties::Budget::default().operation(0, 0);
    let result = cascade([Some(Length::Px(9.0)); 4], 16.0, 16.0,
        [(&refused, false)].into_iter(), &variables, &mut budget);
    assert_eq!(result, [None; 4]);
    let mut budget = custom_properties::Budget::default().operation(0, 0);
    let result = cascade([None; 4], 16.0, 16.0,
        [(&valid, false), (&refused, false)].into_iter(), &variables, &mut budget);
    assert_eq!(uniform(result), Some(6.0), "A winning literal needs no refused lower substitution");
}

#[test]
fn css_radius_grouped_unsupported_math_wins_without_reviving_a_lower_circle() {
    let (doc, styles) = computed(
        "#spaces{border-radius:8px;border-radius:calc(8px + 2px)}#division{border-radius:8px;border-radius:calc(8px / 2)}#axes{border-radius:8px;border-radius:calc(8px / 2) / calc(4px + 2px)}#longhand{border-radius:8px;border-top-left-radius:calc(8px / 2)}#variable{--r:calc(8px + 2px);border-radius:8px;border-radius:var(--r)}#malformed{border-radius:8px;border-radius:calc();border-radius:unknown(8px);border-radius:8px / / 8px}#malformed-longhand{border-radius:8px;border-top-left-radius:calc()}",
        "<div id=spaces></div><div id=division></div><div id=axes></div><div id=longhand></div><div id=variable></div><div id=malformed></div><div id=malformed-longhand></div>",
    );
    for id in ["spaces", "division", "axes", "longhand", "variable"] {
        assert_eq!(at(&doc, &styles, id).circular_radius(), None, "{id}");
    }
    assert_eq!(at(&doc, &styles, "spaces").border_radius, [None; 4]);
    assert_eq!(at(&doc, &styles, "longhand").border_radius,
        [None, Some(Length::Px(8.0)), Some(Length::Px(8.0)), Some(Length::Px(8.0))]);
    for id in ["malformed", "malformed-longhand"] {
        assert_eq!(at(&doc, &styles, id).circular_radius(), Some(8.0), "{id}");
    }
    assert!(radii("calc(8px / 2", false).is_none());
    assert!(radii("calc(8px / 2))", false).is_none());
    assert_eq!(radii(&"calc(".repeat(9), false), Some([None; 4]), "Depth refusal remains unavailable");
}
