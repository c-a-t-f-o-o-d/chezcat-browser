use super::*;

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(id))
        .unwrap()
}
fn cascade(source: &str, body: &str) -> (Document, CascadeOutput) {
    let doc = crate::html::parse(&format!(
        "<html><head><style>{source}</style></head><body>{body}</body></html>"
    ));
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    (doc, output)
}
fn positions(doc: &Document, output: &CascadeOutput, expected: &[(&str, Position)]) {
    for &(id, value) in expected {
        let node = node(doc, id);
        assert_eq!(output.styles[node].position, value, "{id}");
        assert_eq!(
            output.computed_properties.read_position(node),
            Ok(Some(value)),
            "{id}"
        );
    }
}

#[test]
fn css_position_keywords_share_the_renderer_style_and_exact_serialization() {
    for (text, expected) in [
        ("static", Position::Static),
        ("relative", Position::Relative),
        ("absolute", Position::Absolute),
        ("fixed", Position::Fixed),
        ("sticky", Position::Sticky),
    ] {
        let (doc, output) = cascade(
            &format!("#target{{position:{text}}}"),
            "<div id=target></div>",
        );
        positions(&doc, &output, &[("target", expected)]);
        assert_eq!(expected.as_str(), text);
    }
    let (doc, output) = cascade(
        "#target{position:StIcKy}",
        "<span id=target style='display:none'></span>",
    );
    positions(&doc, &output, &[("target", Position::Sticky)]);
    assert_eq!(output.styles[node(&doc, "target")].display, Display::None);
}

#[test]
fn css_position_literal_rejection_and_computed_var_invalidation_have_different_cascade_effects() {
    let (doc, output) = cascade("", "<div id=literal style='position:relative;position:potato'></div><div id=pair style='position:relative;position:absolute fixed'></div><div id=missing style='position:relative;position:var(--missing)'></div><div id=grammar style='--mode:potato;position:relative;position:var(--mode)'></div><div id=cycle style='--mode:var(--mode);position:relative;position:var(--mode)'></div><div id=valid style='--mode:absolute;position:relative;position:var(--mode)'></div><div id=fallback style='position:relative;position:var(--missing,fixed)'></div>");
    positions(
        &doc,
        &output,
        &[
            ("literal", Position::Relative),
            ("pair", Position::Relative),
            ("missing", Position::Static),
            ("grammar", Position::Static),
            ("cycle", Position::Static),
            ("valid", Position::Absolute),
            ("fallback", Position::Fixed),
        ],
    );
    assert_eq!(
        doc.attr(node(&doc, "missing"), "style"),
        Some("position:relative;position:var(--missing)")
    );
    assert!(
        parse_declarations("position:potato;position:absolute fixed;position:inherit sticky")
            .is_empty()
    );
    assert_eq!(
        parse_declarations("position:var(--missing);position:sticky").len(),
        2
    );
}

#[test]
fn css_position_winners_preserve_external_inline_importance_specificity_and_order() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet><style>div{position:static}.picked{position:fixed}#specific{position:absolute}#important{position:sticky}#invalid{position:potato!important}</style><div id=specific class=picked></div><div id=important style='position:fixed'></div><div id=inline style='position:sticky!important'></div><div id=order></div><div id=invalid></div>");
    let mut external = HashMap::new();
    external.insert(node(&doc, "sheet"), "#important{position:relative!important}#inline{position:relative!important}#order{position:relative}#order{position:fixed}#invalid{position:absolute}".into());
    let output = compute_with_environment_and_properties(&doc, &external, None, None);
    positions(
        &doc,
        &output,
        &[
            ("specific", Position::Absolute),
            ("important", Position::Relative),
            ("inline", Position::Sticky),
            ("order", Position::Fixed),
            ("invalid", Position::Absolute),
        ],
    );
}

#[test]
fn css_position_css_wide_and_supported_origin_layer_rollbacks_select_real_lower_winners() {
    let (doc, output) = cascade("#parent{position:fixed}#revert{position:relative;position:revert}#sheet-layer{position:relative;position:revert-layer}#inline-layer{position:absolute}#important-inline{position:fixed!important}#important-revert{position:fixed!important}#important-sheet{position:relative;position:revert-layer!important}#fallback-layer{position:absolute}",
        "<div id=parent><div id=normal></div><div id=inherit style='position:inherit'></div><div id=initial style='position:relative;position:initial'></div><div id=unset style='position:relative;position:unset'></div><div id=var-inherit style='position:var(--missing,inherit)'></div></div><div id=revert></div><div id=sheet-layer></div><div id=inline-layer style='position:sticky;position:revert-layer'></div><div id=important-inline style='position:relative!important;position:revert-layer!important'></div><div id=important-revert style='position:relative!important;position:revert!important'></div><div id=important-sheet style='position:fixed'></div><div id=fallback-layer style='position:var(--missing,revert-layer)'></div>");
    positions(
        &doc,
        &output,
        &[
            ("parent", Position::Fixed),
            ("normal", Position::Static),
            ("inherit", Position::Fixed),
            ("initial", Position::Static),
            ("unset", Position::Static),
            ("var-inherit", Position::Fixed),
            ("revert", Position::Static),
            ("sheet-layer", Position::Static),
            ("inline-layer", Position::Absolute),
            ("important-inline", Position::Fixed),
            ("important-revert", Position::Static),
            ("important-sheet", Position::Static),
            ("fallback-layer", Position::Absolute),
        ],
    );
}

#[test]
fn css_position_refused_winner_is_local_inherit_propagates_and_higher_winner_stays_known() {
    let huge = "x".repeat(2048);
    let declarations = (0..257)
        .map(|n| format!("--x{n}:value;"))
        .collect::<String>();
    let (doc, output) = cascade(&format!("#parent{{--huge:{huge};position:var(--huge)}}#overridden{{--huge:{huge};position:var(--huge);position:sticky}}"),
        &format!("<div id=parent><div id=inherit style='position:inherit'></div><div id=normal></div></div><div id=sibling style='position:relative'></div><div id=overridden></div><div id=blocked style='{declarations}position:fixed'></div>"));
    for id in ["parent", "inherit"] {
        assert_eq!(
            output.computed_properties.read_position(node(&doc, id)),
            Err(ComputedStyleUnavailable),
            "{id}"
        );
    }
    positions(
        &doc,
        &output,
        &[
            ("normal", Position::Static),
            ("sibling", Position::Relative),
            ("overridden", Position::Sticky),
            ("blocked", Position::Fixed),
        ],
    );
    assert_eq!(
        output
            .computed_properties
            .read(node(&doc, "blocked"), "--x0"),
        Err(ComputedStyleUnavailable)
    );
}

#[test]
fn css_position_snapshot_eligibility_and_neighbor_properties_remain_independent() {
    let mut doc = crate::html::parse("<div id=live style='display:none;--actual:kept;overflow:clip visible;position:fixed'></div><div id=detached style='position:absolute'></div><template><div id=inert style='position:sticky'></div></template>");
    let detached = node(&doc, "detached");
    for entry in &mut doc.nodes {
        entry.children.retain(|&child| child != detached);
    }
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    let live = node(&doc, "live");
    assert_eq!(
        output.computed_properties.read_position(live),
        Ok(Some(Position::Fixed))
    );
    assert_eq!(
        output.computed_properties.read(live, "--actual"),
        Ok(Some("kept"))
    );
    assert_eq!(
        output.computed_properties.read_overflow(live),
        Ok(Some(Overflow {
            x: OverflowValue::Clip,
            y: OverflowValue::Visible
        }))
    );
    assert_eq!(output.computed_properties.read_position(detached), Ok(None));
    assert_eq!(
        output
            .computed_properties
            .read_position(node(&doc, "inert")),
        Ok(None)
    );
    assert_eq!(
        output.computed_properties.read_position(doc.nodes.len()),
        Err(ComputedStyleUnavailable)
    );
}

#[test]
fn css_position_actual_media_and_checked_state_use_the_same_budgeted_cascade() {
    let doc = crate::html::parse("<style>#target{position:relative}@media(min-width:64em){#target{position:fixed}}input:checked + #target{position:sticky}</style><input id=check type=checkbox><div id=target></div>");
    let check = node(&doc, "check");
    let target = node(&doc, "target");
    for (width, checked, expected) in [
        (1023.75, false, Position::Relative),
        (1024.0, false, Position::Fixed),
        (640.0, true, Position::Sticky),
    ] {
        let state = |node| (node == check).then_some(checked);
        let environment = Some(MediaEnvironment {
            width,
            height: 660.0,
            hover: Hover::Hover,
        });
        let mut work = 64_000_000;
        let mut copies = 64 * 1024 * 1024;
        let output = compute_with_environment_and_properties_budgeted(
            &doc,
            &HashMap::new(),
            Some(&state),
            environment,
            &mut CssBudget::new(&mut work, &mut copies),
        )
        .unwrap();
        assert_eq!(output.styles[target].position, expected);
        assert_eq!(
            output.computed_properties.read_position(target),
            Ok(Some(expected))
        );
        assert!(work < 64_000_000 && copies < 64 * 1024 * 1024);
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
    assert_eq!(work, 0);
    let mut work = 64_000_000;
    let mut copies = 0;
    assert!(compute_with_environment_and_properties_budgeted(
        &doc,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies)
    )
    .is_err());
    assert_eq!(copies, 0);
}

#[test]
fn css_position_bounded_substitution_refusal_cannot_turn_into_static_readback() {
    let variables = custom_properties::Variables::default();
    let declarations = parse_declarations("position:var(--missing,relative)");
    let mut budget = custom_properties::Budget::default().operation(0, 0);
    assert_eq!(
        position::cascade(
            Position::Fixed,
            true,
            declarations.iter().map(|d| (d, false)),
            &variables,
            &mut budget
        ),
        (Position::Static, false)
    );
    let literal = parse_declarations("position:fixed");
    assert_eq!(
        position::cascade(
            Position::Static,
            false,
            literal.iter().map(|d| (d, false)),
            &variables,
            &mut budget
        ),
        (Position::Fixed, true)
    );
}
