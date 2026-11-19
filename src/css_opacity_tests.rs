use super::*;

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some(id))
        .unwrap()
}
fn cascade(css: &str, body: &str) -> (Document, CascadeOutput) {
    let doc = crate::html::parse(&format!(
        "<!doctype html><html><head><style>{css}</style></head><body>{body}</body></html>"
    ));
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    (doc, result)
}
fn values(doc: &Document, result: &CascadeOutput, expected: &[(&str, f64)]) {
    for &(id, value) in expected {
        let n = node(doc, id);
        assert_eq!(result.styles[n].opacity, value, "renderer scalar {id}");
        assert_eq!(
            result.computed_properties.read_opacity(n),
            Ok(Some(value)),
            "snapshot {id}"
        );
    }
}

#[test]
fn css_opacity_numbers_percentages_clamping_and_literal_token_boundaries() {
    for (text, expected) in [
        ("0", 0.0),
        ("-0", 0.0),
        ("1", 1.0),
        (".125", 0.125),
        ("+25%", 0.25),
        ("1e-2", 0.01),
        ("2.5e1%", 0.25),
        ("-20%", 0.0),
        ("120%", 1.0),
        ("2", 1.0),
        ("1e999", 1.0),
        ("-1e999%", 0.0),
    ] {
        let (doc, out) = cascade("", &format!("<div id=target style='opacity:{text}'></div>"));
        values(&doc, &out, &[("target", expected)]);
    }
    for invalid in [
        "",
        "none",
        "auto",
        "NaN",
        "Infinity",
        "1px",
        "1.",
        ".",
        "25 %",
        "0.2 0.3",
        "inherit .5",
        "unknown(.5)",
        "calc()",
    ] {
        let (doc, out) = cascade(
            "",
            &format!("<div id=target style='opacity:.75;opacity:{invalid}'></div>"),
        );
        values(&doc, &out, &[("target", 0.75)]);
    }
}

#[test]
fn css_opacity_variables_invalid_computed_values_and_real_noninheritance() {
    let (doc,out)=cascade("#parent{opacity:25%;--value:50%}",concat!(
        "<div id=parent><div id=normal></div><div id=inherit style='opacity:inherit'></div>",
        "<div id=initial style='opacity:.2;opacity:initial'></div><div id=unset style='opacity:.2;opacity:unset'></div>",
        "<div id=variable style='opacity:var(--value)'></div><div id=fallback style='opacity:var(--missing,.125)'></div>",
        "<div id=missing style='opacity:.2;opacity:var(--missing)'></div><div id=invalid style='--bad:potato;opacity:.2;opacity:var(--bad)'></div>",
        "<div id=cycle style='--a:var(--b);--b:var(--a);opacity:.2;opacity:var(--a)'></div>",
        "<div id=token style='--n:25;opacity:.2;opacity:var(--n)%'></div>",
        "<div id=explicit style='opacity:var(--missing,inherit)'></div></div>"
    ));
    values(
        &doc,
        &out,
        &[
            ("parent", 0.25),
            ("normal", 1.0),
            ("inherit", 0.25),
            ("initial", 1.0),
            ("unset", 1.0),
            ("variable", 0.5),
            ("fallback", 0.125),
            ("missing", 1.0),
            ("invalid", 1.0),
            ("cycle", 1.0),
            ("token", 1.0),
            ("explicit", 0.25),
        ],
    );
    assert_eq!(
        doc.attr(node(&doc, "missing"), "style"),
        Some("opacity:.2;opacity:var(--missing)")
    );
}

#[test]
fn css_opacity_external_priority_specificity_and_origin_layer_rollbacks() {
    let doc=crate::html::parse(concat!(
        "<link id=sheet rel=stylesheet><style>.picked{opacity:.2}#specific{opacity:.3}",
        "#revert{opacity:.2;opacity:revert}#sheet-layer{opacity:.2;opacity:revert-layer}",
        "#inline-layer{opacity:.3}#important-inline{opacity:.4!important}#important-sheet{opacity:.3;opacity:revert-layer!important}</style>",
        "<div id=specific class=picked></div><div id=external style='opacity:.5'></div>",
        "<div id=inline style='opacity:.6!important'></div><div id=order></div>",
        "<div id=revert></div><div id=sheet-layer></div><div id=inline-layer style='opacity:.7;opacity:revert-layer'></div>",
        "<div id=important-inline style='opacity:.7!important;opacity:revert-layer!important'></div>",
        "<div id=important-sheet style='opacity:.8'></div>"
    ));
    let external=HashMap::from([(node(&doc,"sheet"),"#external{opacity:40%!important}#inline{opacity:.4!important}#order{opacity:.2}#order{opacity:.6}".into())]);
    let out = compute_with_environment_and_properties(&doc, &external, None, None);
    values(
        &doc,
        &out,
        &[
            ("specific", 0.3),
            ("external", 0.4),
            ("inline", 0.6),
            ("order", 0.6),
            ("revert", 1.0),
            ("sheet-layer", 1.0),
            ("inline-layer", 0.3),
            ("important-inline", 0.4),
            ("important-sheet", 1.0),
        ],
    );
}

#[test]
fn css_opacity_unknown_winners_are_local_and_overrides_restore_certainty() {
    let huge = "x".repeat(2048);
    let (doc,out)=cascade(&format!("#parent{{--huge:{huge};opacity:var(--huge)}}#overridden{{--huge:{huge};opacity:var(--huge);opacity:.75}}"),concat!(
        "<div id=parent><div id=inherit style='opacity:inherit'></div><div id=normal></div></div>",
        "<div id=sibling style='opacity:.5'></div><div id=overridden></div>",
        "<div id=math style='opacity:.25;opacity:calc(.25 + .25)'></div>",
        "<div id=restored style='opacity:calc(.25 + .25);opacity:1'></div>"
    ));
    for id in ["parent", "inherit", "math"] {
        assert_eq!(
            out.computed_properties.read_opacity(node(&doc, id)),
            Err(ComputedStyleUnavailable),
            "{id}"
        );
    }
    values(
        &doc,
        &out,
        &[
            ("normal", 1.0),
            ("sibling", 0.5),
            ("overridden", 0.75),
            ("restored", 1.0),
        ],
    );
}

#[test]
fn css_opacity_hidden_eligible_detached_inert_and_neighbor_snapshots() {
    let mut doc=crate::html::parse("<div id=live style='display:none;opacity:.25;position:relative;overflow:clip;--control:kept'></div><div id=detached style='opacity:.5'></div><template><div id=inert style='opacity:.5'></div></template>");
    let detached = node(&doc, "detached");
    for n in &mut doc.nodes {
        n.children.retain(|&c| c != detached);
    }
    let out = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    values(&doc, &out, &[("live", 0.25)]);
    assert_eq!(out.computed_properties.read_opacity(detached), Ok(None));
    assert_eq!(
        out.computed_properties.read_opacity(node(&doc, "inert")),
        Ok(None)
    );
    assert_eq!(
        out.computed_properties.read_opacity(doc.nodes.len()),
        Err(ComputedStyleUnavailable)
    );
    let n = node(&doc, "live");
    assert_eq!(
        out.computed_properties.read(n, "--control"),
        Ok(Some("kept"))
    );
    assert_eq!(
        out.computed_properties.read_position(n),
        Ok(Some(Position::Relative))
    );
    assert_eq!(
        out.computed_properties.read_overflow(n),
        Ok(Some(Overflow {
            x: OverflowValue::Clip,
            y: OverflowValue::Clip
        }))
    );
}

#[test]
fn css_opacity_media_checked_state_and_forced_budget_preserve_existing_no_property_path() {
    let doc=crate::html::parse("<style>#target{opacity:.2}@media(min-width:64em){#target{opacity:.4}}input:checked + #target{opacity:.8}</style><input id=check type=checkbox><div id=target></div>");
    let check = node(&doc, "check");
    for (width, checked, expected) in [
        (640.0, false, 0.2),
        (1024.0, false, 0.4),
        (640.0, true, 0.8),
    ] {
        let state = |n| (n == check).then_some(checked);
        let mut work = 64_000_000;
        let mut copies = 64 * 1024 * 1024;
        let out = compute_with_environment_and_properties_budgeted(
            &doc,
            &HashMap::new(),
            Some(&state),
            Some(MediaEnvironment {
                width,
                height: 600.0,
                hover: Hover::Hover,
            }),
            &mut CssBudget::new(&mut work, &mut copies),
        )
        .unwrap();
        values(&doc, &out, &[("target", expected)]);
    }
    let plain = crate::html::parse("<div id=target></div>");
    let (mut work, mut copies) = (0, 0);
    let out = compute_with_environment_and_properties_transform_budgeted(
        &plain,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    values(&plain, &out, &[("target", 1.0)]);
    assert_eq!((work, copies), (0, 0));
    let doc = crate::html::parse("<div id=target style='opacity:.5'></div>");
    assert!(compute_with_environment_and_properties_transform_budgeted(
        &doc,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies)
    )
    .is_err());
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    compute_with_environment_and_properties_transform_budgeted(
        &doc,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    let (mut exact_work, mut exact_copies) = (1_000_000 - work, 1_000_000 - copies);
    let out = compute_with_environment_and_properties_transform_budgeted(
        &doc,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut exact_work, &mut exact_copies),
    )
    .unwrap();
    values(&doc, &out, &[("target", 0.5)]);
    assert_eq!((exact_work, exact_copies), (0, 0));
}

#[test]
fn css_opacity_number_serialization_is_bounded_fixed_decimal_without_negative_zero() {
    for (value, expected) in [
        (0.0, "0"),
        (-0.0, "0"),
        (1.0, "1"),
        (0.25, "0.25"),
        (0.1234564, "0.123456"),
        (0.1234565, "0.123457"),
        (0.0000004, "0"),
        (0.0000005, "0.000001"),
        (0.9999999, "1"),
    ] {
        let (bytes, len) = serialize(value).unwrap();
        assert_eq!(std::str::from_utf8(&bytes[..len]).unwrap(), expected);
    }
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(serialize(value).is_none());
    }
}
