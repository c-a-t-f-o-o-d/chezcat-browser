use super::*;

fn target(source: &str) -> Style {
    let doc = crate::html::parse(&format!(
        "<style>{source}</style><div id=target>Content</div>"
    ));
    let node = (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some("target"))
        .unwrap();
    compute(&doc, &HashMap::new())[node].clone()
}

#[test]
fn css_overflow_independent_axis_participates_in_the_cascade() {
    assert_eq!(
        target("#target{overflow-x:hidden}").overflow,
        Overflow {
            x: OverflowValue::Hidden,
            y: OverflowValue::Auto
        }
    );
}

#[test]
fn css_overflow_pair_shorthand_expands_before_rendering() {
    assert_eq!(
        target("#target{overflow:hidden auto}").overflow,
        Overflow {
            x: OverflowValue::Hidden,
            y: OverflowValue::Auto
        }
    );
}

#[test]
fn css_overflow_variables_use_the_custom_property_map() {
    assert_eq!(
        target("#target{--mode:hidden;overflow:var(--mode)}").overflow,
        Overflow {
            x: OverflowValue::Hidden,
            y: OverflowValue::Hidden
        }
    );
}

#[test]
fn css_overflow_overlay_is_the_auto_legacy_alias() {
    assert_eq!(
        target("#target{overflow:overlay}").overflow,
        Overflow {
            x: OverflowValue::Auto,
            y: OverflowValue::Auto
        }
    );
}

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(id))
        .unwrap()
}
fn cascade(source: &str, body: &str) -> (Document, CascadeOutput) {
    let doc = crate::html::parse(&format!(
        "<html><head><style>{source}</style></head><body>{body}</body></html>"
    ));
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    (doc, result)
}
fn pair(x: OverflowValue, y: OverflowValue) -> Overflow {
    Overflow { x, y }
}

#[test]
fn css_overflow_computed_matrix_and_serialization_share_the_final_style() {
    use OverflowValue::{Auto, Clip, Hidden, Scroll, Visible};
    let cases = [
        ("visible visible", Visible, Visible),
        ("visible clip", Visible, Clip),
        ("visible hidden", Auto, Hidden),
        ("visible auto", Auto, Auto),
        ("visible scroll", Auto, Scroll),
        ("clip visible", Clip, Visible),
        ("clip clip", Clip, Clip),
        ("clip hidden", Hidden, Hidden),
        ("clip auto", Hidden, Auto),
        ("clip scroll", Hidden, Scroll),
        ("hidden visible", Hidden, Auto),
        ("hidden clip", Hidden, Hidden),
        ("hidden hidden", Hidden, Hidden),
        ("hidden auto", Hidden, Auto),
        ("hidden scroll", Hidden, Scroll),
        ("auto visible", Auto, Auto),
        ("auto clip", Auto, Hidden),
        ("auto hidden", Auto, Hidden),
        ("auto auto", Auto, Auto),
        ("auto scroll", Auto, Scroll),
        ("scroll visible", Scroll, Auto),
        ("scroll clip", Scroll, Hidden),
        ("scroll hidden", Scroll, Hidden),
        ("scroll auto", Scroll, Auto),
        ("scroll scroll", Scroll, Scroll),
    ];
    for (source, x, y) in cases {
        let (doc, result) = cascade(
            &format!("#target{{overflow:{source}}}"),
            "<div id=target></div>",
        );
        let target = node(&doc, "target");
        let expected = pair(x, y);
        assert_eq!(result.styles[target].overflow, expected, "{source}");
        assert_eq!(
            result.computed_properties.read_overflow(target),
            Ok(Some(expected)),
            "{source}"
        );
        let text = expected.to_css();
        let length = x.as_css().len() + if x == y { 0 } else { 1 + y.as_css().len() };
        assert_eq!(text.len(), length);
        assert_eq!(
            text,
            if x == y {
                x.as_css().into()
            } else {
                format!("{} {}", x.as_css(), y.as_css())
            }
        );
        assert_eq!(expected.clips_x(), x != Visible);
        assert_eq!(expected.clips_y(), y != Visible);
    }
    assert_eq!(
        target("#target{overflow:OvErLaY CLIP}").overflow,
        pair(Auto, Hidden)
    );
}

#[test]
fn css_overflow_normalization_waits_for_all_axis_winners() {
    use OverflowValue::{Clip, Hidden, Visible};
    assert_eq!(
        target("#target{overflow:hidden;overflow-x:visible;overflow-y:visible}").overflow,
        pair(Visible, Visible)
    );
    assert_eq!(
        target("#target{overflow-y:auto;overflow-x:clip;overflow-y:visible}").overflow,
        pair(Clip, Visible)
    );
    assert_eq!(
        target("#target{overflow:clip visible;overflow-y:hidden;overflow-y:visible}").overflow,
        pair(Clip, Visible)
    );
    assert_eq!(
        target("#target{overflow:hidden;overflow:visible potato;overflow-x:clip visible}").overflow,
        pair(Hidden, Hidden)
    );
    assert_eq!(
        target("#target{overflow:hidden;overflow:visible clip auto;overflow:initial hidden}")
            .overflow,
        pair(Hidden, Hidden)
    );
}

#[test]
fn css_overflow_axis_cascade_keeps_importance_specificity_order_and_external_sources() {
    use OverflowValue::{Auto, Clip, Hidden, Scroll, Visible};
    let doc = crate::html::parse("<link id=sheet rel=stylesheet><style>div{overflow:scroll}.pick{overflow-x:clip}#specific{overflow-x:hidden}#important{overflow-y:hidden!important}#inline{overflow:clip!important}#tie{overflow-x:hidden}#tie{overflow-x:clip}</style><div id=specific class=pick></div><div id=important style='overflow:visible'></div><div id=inline style='overflow-y:visible!important'></div><div id=tie style='overflow-y:visible'></div><div id=external></div>");
    let mut external = HashMap::new();
    external.insert(
        node(&doc, "sheet"),
        "#external{overflow:clip visible}#inline{overflow-y:hidden!important}".into(),
    );
    let result = compute_with_environment_and_properties(&doc, &external, None, None);
    for (id, expected) in [
        ("specific", pair(Hidden, Scroll)),
        ("important", pair(Auto, Hidden)),
        ("inline", pair(Clip, Visible)),
        ("tie", pair(Clip, Visible)),
        ("external", pair(Clip, Visible)),
    ] {
        let index = node(&doc, id);
        assert_eq!(result.styles[index].overflow, expected, "{id}");
        assert_eq!(
            result.computed_properties.read_overflow(index),
            Ok(Some(expected)),
            "{id}"
        );
    }
}

#[test]
fn css_overflow_css_wide_values_use_parent_computed_axes_and_remain_noninherited() {
    use OverflowValue::{Auto, Hidden, Visible};
    let (doc, result) = cascade("#parent{overflow:hidden visible}", "<div id=parent><div id=normal></div><div id=inherit style='overflow:inherit'></div><div id=initial style='overflow:hidden;overflow:initial'></div><div id=unset style='overflow:hidden;overflow:unset'></div><div id=axis style='overflow-y:inherit'></div><div id=reset style='overflow:hidden;overflow-x:initial'></div></div>");
    for (id, expected) in [
        ("parent", pair(Hidden, Auto)),
        ("normal", pair(Visible, Visible)),
        ("inherit", pair(Hidden, Auto)),
        ("initial", pair(Visible, Visible)),
        ("unset", pair(Visible, Visible)),
        ("axis", pair(Auto, Auto)),
        ("reset", pair(Auto, Hidden)),
    ] {
        assert_eq!(result.styles[node(&doc, id)].overflow, expected, "{id}");
    }
}

#[test]
fn css_overflow_variables_and_invalid_computed_values_reset_only_the_affected_axes() {
    use OverflowValue::{Auto, Clip, Hidden, Visible};
    let (doc, result) = cascade("#parent{--pair:clip visible;overflow:hidden auto}", "<div id=parent><div id=valid style='overflow:var(--pair)'></div><div id=missing style='overflow:hidden;overflow:var(--missing)'></div><div id=grammar style='--bad:potato;overflow:hidden;overflow:var(--bad)'></div><div id=axis style='--bad:clip visible;overflow:hidden;overflow-x:var(--bad)'></div><div id=override style='overflow:hidden;overflow:var(--missing);overflow-y:clip'></div><div id=inherit style='overflow:var(--missing,inherit)'></div><div id=cycle style='--cycle:var(--cycle);overflow:hidden;overflow:var(--cycle)'></div><div id=fallback style='overflow:var(--missing,clip) visible'></div></div>");
    for (id, expected) in [
        ("valid", pair(Clip, Visible)),
        ("missing", pair(Visible, Visible)),
        ("grammar", pair(Visible, Visible)),
        ("axis", pair(Auto, Hidden)),
        ("override", pair(Visible, Clip)),
        ("inherit", pair(Hidden, Auto)),
        ("cycle", pair(Visible, Visible)),
        ("fallback", pair(Clip, Visible)),
    ] {
        let index = node(&doc, id);
        assert_eq!(result.styles[index].overflow, expected, "{id}");
        assert_eq!(
            result.computed_properties.read_overflow(index),
            Ok(Some(expected)),
            "{id}"
        );
    }
    assert_eq!(
        doc.attr(node(&doc, "axis"), "style"),
        Some("--bad:clip visible;overflow:hidden;overflow-x:var(--bad)")
    );
}

#[test]
fn css_overflow_snapshot_media_checkedness_eligibility_and_forced_budget_are_real() {
    use OverflowValue::{Auto, Clip, Hidden, Visible};
    let mut doc = crate::html::parse("<style>#target{overflow:clip visible}@media(min-width:64em){#target{overflow-x:hidden}}input:checked + #target{overflow-y:scroll}</style><input id=check type=checkbox><div id=target style='display:none'></div><div id=detached style='overflow:hidden'></div><template><div id=inert style='overflow:scroll'></div></template>");
    let detached = node(&doc, "detached");
    for entry in &mut doc.nodes {
        entry.children.retain(|&child| child != detached);
    }
    let check = node(&doc, "check");
    let target = node(&doc, "target");
    let off = |_| Some(false);
    let on = |index| (index == check).then_some(true);
    for (width, checked, expected) in [
        (
            640.0,
            &off as &dyn Fn(usize) -> Option<bool>,
            pair(Clip, Visible),
        ),
        (
            1280.0,
            &off as &dyn Fn(usize) -> Option<bool>,
            pair(Hidden, Auto),
        ),
        (
            640.0,
            &on as &dyn Fn(usize) -> Option<bool>,
            pair(Hidden, OverflowValue::Scroll),
        ),
    ] {
        let environment = Some(MediaEnvironment {
            width,
            height: 660.0,
            hover: Hover::Hover,
        });
        let mut work = 64_000_000;
        let mut copies = 64 * 1024 * 1024;
        let result = compute_with_environment_and_properties_budgeted(
            &doc,
            &HashMap::new(),
            Some(checked),
            environment,
            &mut CssBudget::new(&mut work, &mut copies),
        )
        .unwrap();
        assert_eq!(result.styles[target].overflow, expected);
        assert_eq!(
            result.computed_properties.read_overflow(target),
            Ok(Some(expected))
        );
        assert_eq!(result.computed_properties.read_overflow(detached), Ok(None));
        assert_eq!(
            result
                .computed_properties
                .read_overflow(node(&doc, "inert")),
            Ok(None)
        );
        assert_eq!(
            result.computed_properties.read_overflow(doc.nodes.len()),
            Err(ComputedStyleUnavailable)
        );
    }
    let mut work = 0;
    let mut copies = 64 * 1024 * 1024;
    assert!(compute_with_environment_and_properties_budgeted(
        &doc,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies)
    )
    .is_err());
}

#[test]
fn css_overflow_refused_substitution_is_local_and_later_axis_winners_restore_availability() {
    use OverflowValue::{Auto, Clip, Hidden, Visible};
    // The bounded substituted output cannot hold this valid 2048-byte custom
    // value plus token separators. No job or production allowance is enlarged.
    let huge = "x".repeat(2048);
    // An independent node has the same refused shorthand followed by two
    // valid longhands, restoring both axes before normalization.
    let body = format!("<div id=parent><div id=inherit style='overflow:inherit'></div><div id=axis style='overflow-y:inherit'></div><div id=unknown-axis style='overflow-x:inherit'></div></div><div id=sibling style='overflow:visible'></div><div id=restored style='overflow:var(--missing,clip);overflow-y:visible'></div><div id=overridden style='--huge:{huge};overflow:var(--huge);overflow-x:clip;overflow-y:visible'></div>");
    let (doc, result) = cascade(
        &format!("#parent{{--huge:{huge};overflow-y:hidden;overflow-x:var(--huge)}}"),
        &body,
    );
    for id in ["parent", "inherit", "unknown-axis"] {
        assert_eq!(
            result.computed_properties.read_overflow(node(&doc, id)),
            Err(ComputedStyleUnavailable),
            "{id}"
        );
    }
    for (id, expected) in [
        ("axis", pair(Auto, Hidden)),
        ("sibling", pair(Visible, Visible)),
        ("restored", pair(Clip, Visible)),
        ("overridden", pair(Clip, Visible)),
    ] {
        assert_eq!(
            result.computed_properties.read_overflow(node(&doc, id)),
            Ok(Some(expected)),
            "{id}"
        );
    }
}
