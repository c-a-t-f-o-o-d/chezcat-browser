use super::*;

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(id))
        .unwrap()
}

fn cascade(sheet: &str, body: &str) -> (Document, CascadeOutput) {
    let doc = crate::html::parse(&format!(
        "<html><head><style>{sheet}</style></head><body>{body}</body></html>"
    ));
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    (doc, output)
}

fn translated(x: f32, y: f32) -> Transform {
    Transform::Translate {
        x: Length::Px(x),
        y: Length::Px(y),
    }
}

fn expect(doc: &Document, output: &CascadeOutput, id: &str, value: Transform) {
    let node = node(doc, id);
    assert_eq!(output.styles[node].transform, value, "{id}");
    assert_eq!(
        output.computed_properties.read_transform(node),
        Ok(Some(value)),
        "{id}"
    );
}

#[test]
fn css_translate_finite_functions_normalize_and_identity_is_not_none() {
    for (source, normalized, expected) in [
        ("NoNe", "none", Transform::None),
        ("translate(0)", "translate(0px)", translated(0.0, 0.0)),
        (
            "TrAnSlAtE(+1.5PX,-2E1px)",
            "translate(1.5px, -20px)",
            translated(1.5, -20.0),
        ),
        (
            "translateX(-11px)",
            "translateX(-11px)",
            translated(-11.0, 0.0),
        ),
        (
            "translateY(.5px)",
            "translateY(0.5px)",
            translated(0.0, 0.5),
        ),
        (
            "translate3d(12px, 9px, 0em)",
            "translate3d(12px, 9px, 0px)",
            translated(12.0, 9.0),
        ),
        (
            "translate(3pt, 2pc)",
            "translate(4px, 32px)",
            translated(4.0, 32.0),
        ),
    ] {
        assert_eq!(normalize_transform(source).unwrap(), normalized, "{source}");
        let (doc, output) = cascade(
            "",
            &format!("<div id=target style='transform:{source}'></div>"),
        );
        expect(&doc, &output, "target", expected);
    }
    assert!(translated(0.0, 0.0).establishes_containing_block());
    assert!(!Transform::None.establishes_containing_block());
    let (doc, output) = cascade(
        "",
        "<div id=target style='transform:translate(-25%,50%)'></div>",
    );
    expect(
        &doc,
        &output,
        "target",
        Transform::Translate {
            x: Length::Percent(-25.0),
            y: Length::Percent(50.0),
        },
    );
    assert_eq!(normalize_transform(""), Ok(String::new()));
}

#[test]
fn css_translate_invalid_literals_do_not_win_and_unsupported_values_never_become_identity() {
    for source in [
        "banana",
        "translate()",
        "translate(1)",
        "translate(1px,)",
        "translate(1px 2px)",
        "translateX(1px,2px)",
        "translate3d(1px,2px)",
        "translate3d(1px,2px,0%)",
        "translate(1.px)",
        "translate(1 px)",
        "translate(NaNpx)",
        "translate(2quux)",
        "translate (1px)",
        "translate(1px))",
        "none translate(1px)",
        "potato(1px)",
        "translate3d(banana,2px,3px)",
        "translate(1vw,banana)",
        "rotate(banana)",
        "rotate(1px)",
        "rotate3d(1,0,0,1px)",
        "scale(,)",
        "scale(1px)",
        "scale3d(1,2)",
        "matrix(1)",
        "matrix(1,0,0,1,2px,3)",
        "skew(1px)",
        "perspective(-1px)",
        "translateZ(0%)",
        "translate(calc(1px),)",
        "translate(potato(1px))",
    ] {
        assert_eq!(
            normalize_transform(source),
            Err(TransformSyntax::Invalid),
            "{source}"
        );
        let (doc, output) = cascade(
            "",
            &format!("<div id=target style='transform:translateX(7px);transform:{source}'></div>"),
        );
        expect(&doc, &output, "target", translated(7.0, 0.0));
    }
    for source in ["translate(1px)\u{00a0}", "translate(\u{00a0}1px)"] {
        assert_eq!(normalize_transform(source), Err(TransformSyntax::Invalid));
    }
    for source in [
        "translateX(1px) translateY(2px)",
        "translate3d(1px,2px,3px)",
        "rotate(10deg)",
        "scale(2)",
        "matrix(1,0,0,1,2,3)",
        "translate(1vw)",
        "translate(calc(1px + 2px))",
        "translate(10001px)",
    ] {
        assert_eq!(
            normalize_transform(source),
            Err(TransformSyntax::Unsupported),
            "{source}"
        );
        let (doc, output) = cascade(
            "",
            &format!("<div id=target style='transform:translateX(7px);transform:{source}'></div>"),
        );
        assert_eq!(
            output
                .computed_properties
                .read_transform(node(&doc, "target")),
            Err(ComputedStyleUnavailable),
            "{source}"
        );
        let (doc, output) = cascade(
            "",
            &format!("<div id=target style='transform:{source};transform:none'></div>"),
        );
        expect(&doc, &output, "target", Transform::None);
    }
}

#[test]
fn css_translate_variables_preserve_case_tokens_fallbacks_and_iacvt() {
    let raw = "  TrAnSlAtE(var(--X), var(--y, 7px))  ";
    assert_eq!(normalize_transform(raw).unwrap(), raw.trim());
    let (doc, output) = cascade("", concat!(
        "<div id=parent style='--X:11px;--x:22px;--y:13px;--cycle:var(--cycle)'>",
        "<div id=case style='transform:translate(var(--X),var(--x))'></div>",
        "<div id=fallback style='transform:var(--missing,translate3d(2px,3px,0))'></div>",
        "<div id=missing style='transform:translateX(9px);transform:var(--missing)'></div>",
        "<div id=cycle style='transform:translateX(9px);transform:var(--cycle)'></div>",
        "<div id=invalid style='--bad:potato;transform:translateX(9px);transform:var(--bad)'></div>",
        "<div id=boundary style='--n:2;transform:translateX(9px);transform:translateX(var(--n)px)'></div>",
        "<div id=unsupported style='--list:translateX(1px) translateY(2px);transform:var(--list)'></div>",
        "</div>"
    ));
    expect(&doc, &output, "case", translated(11.0, 22.0));
    expect(&doc, &output, "fallback", translated(2.0, 3.0));
    for id in ["missing", "cycle", "invalid", "boundary"] {
        expect(&doc, &output, id, Transform::None);
    }
    assert_eq!(
        output
            .computed_properties
            .read_transform(node(&doc, "unsupported")),
        Err(ComputedStyleUnavailable)
    );
}

#[test]
fn css_translate_computed_lengths_use_final_fonts_and_inherit_absolute_values() {
    let (doc, output) = cascade("html{font-size:18px;transform:translateX(1rem)}", concat!(
        "<div id=parent style='transform:translate(2em,1rem);font-size:20px'>Text",
        "<div id=inherit style='font-size:30px;transform:inherit'></div>",
        "<div id=own style='font-size:30px;transform:translate(2em,1rem)'></div>",
        "<div id=normal></div></div>",
        "<div id=reverse style='font-size:20px;transform:translate(2em,1rem)'></div>",
        "<div id=percent style='transform:translate(25%,50%)'><div id=percent-inherit style='transform:inherit'></div></div>"
    ));
    let root = doc
        .nodes
        .iter()
        .position(|node| matches!(&node.kind, NodeKind::Element {tag,..} if tag == "html"))
        .unwrap();
    assert_eq!(output.styles[root].transform, translated(18.0, 0.0));
    expect(&doc, &output, "parent", translated(40.0, 18.0));
    expect(&doc, &output, "inherit", translated(40.0, 18.0));
    expect(&doc, &output, "own", translated(60.0, 18.0));
    expect(&doc, &output, "reverse", translated(40.0, 18.0));
    expect(&doc, &output, "normal", Transform::None);
    let percent = Transform::Translate {
        x: Length::Percent(25.0),
        y: Length::Percent(50.0),
    };
    expect(&doc, &output, "percent", percent);
    expect(&doc, &output, "percent-inherit", percent);
    let text = doc.nodes[node(&doc, "parent")].children[0];
    assert!(matches!(doc.nodes[text].kind, NodeKind::Text(_)));
    assert_eq!(output.styles[text].transform, Transform::None);
    assert_eq!(output.computed_properties.read_transform(text), Ok(None));
}

#[test]
fn css_translate_cascade_uses_real_external_priority_and_css_wide_rollbacks() {
    let doc = crate::html::parse(concat!(
        "<link id=sheet rel=stylesheet><style>div{transform:translateX(1px)}.picked{transform:translateX(2px)}",
        "#specific{transform:translateX(3px)}#order{transform:translateX(4px);transform:translateX(5px)}",
        "#parent{transform:translateY(7px)}#revert{transform:revert}#sheet-layer{transform:revert-layer}</style>",
        "<div id=specific class=picked></div><div id=order></div>",
        "<div id=important style='transform:translateX(6px)'></div>",
        "<div id=inline style='transform:translateX(8px)!important'></div>",
        "<div id=layer style='transform:translateX(9px)!important;transform:revert-layer!important'></div>",
        "<div id=revert></div><div id=sheet-layer></div><div id=parent>",
        "<div id=inherit style='transform:inherit'></div><div id=initial style='transform:initial'></div>",
        "<div id=unset style='transform:unset'></div><div id=var-inherit style='transform:var(--missing,inherit)'></div></div>"
    ));
    let external = HashMap::from([(node(&doc, "sheet"), "#important{transform:translateX(10px)!important}#inline{transform:translateX(11px)!important}#layer{transform:translateX(12px)!important}".into())]);
    let output = compute_with_environment_and_properties(&doc, &external, None, None);
    for (id, value) in [
        ("specific", translated(3.0, 0.0)),
        ("order", translated(5.0, 0.0)),
        ("important", translated(10.0, 0.0)),
        ("inline", translated(8.0, 0.0)),
        ("layer", translated(12.0, 0.0)),
        ("inherit", translated(0.0, 7.0)),
        ("var-inherit", translated(0.0, 7.0)),
    ] {
        expect(&doc, &output, id, value);
    }
    for id in ["revert", "sheet-layer", "initial", "unset"] {
        expect(&doc, &output, id, Transform::None);
    }
    for keyword in ["inherit", "initial", "unset", "revert", "revert-layer"] {
        assert_eq!(
            normalize_transform(&keyword.to_ascii_uppercase()).unwrap(),
            keyword
        );
    }
}

#[test]
fn css_translate_refused_values_and_shared_job_budgets_never_publish_known_none() {
    let variables = custom_properties::Variables::default();
    let declarations = parse_declarations("transform:var(--missing,translateX(1px))");
    let mut budget = custom_properties::Budget::default().operation(0, 0);
    assert_eq!(
        translate::cascade(
            Transform::None,
            true,
            16.0,
            16.0,
            declarations.iter().map(|declaration| (declaration, false)),
            &variables,
            &mut budget
        ),
        (Transform::None, false)
    );
    let literal = parse_declarations("transform:translateX(2px)");
    assert_eq!(
        translate::cascade(
            Transform::None,
            false,
            16.0,
            16.0,
            literal.iter().map(|declaration| (declaration, false)),
            &variables,
            &mut budget
        ),
        (translated(2.0, 0.0), true)
    );
    let (doc, output) = cascade("", "<div id=parent style='transform:rotate(10deg)'><div id=inherit style='transform:inherit'></div><div id=normal></div></div><div id=known style='transform:rotate(10deg);transform:translateY(3px)'></div>");
    for id in ["parent", "inherit"] {
        assert_eq!(
            output.computed_properties.read_transform(node(&doc, id)),
            Err(ComputedStyleUnavailable)
        );
    }
    expect(&doc, &output, "normal", Transform::None);
    expect(&doc, &output, "known", translated(0.0, 3.0));
    assert_eq!(
        normalize_transform(&"x".repeat(2049)),
        Err(TransformSyntax::Unsupported)
    );
    assert_eq!(
        normalize_transform(&"translateX(0) ".repeat(33)),
        Err(TransformSyntax::Unsupported)
    );
    for (initial_work, initial_copies) in [(0, 1_000_000), (1_000_000, 0)] {
        let (mut work, mut copies) = (initial_work, initial_copies);
        assert!(compute_with_environment_and_properties_budgeted(
            &doc,
            &HashMap::new(),
            None,
            None,
            &mut CssBudget::new(&mut work, &mut copies)
        )
        .is_err());
        if initial_work == 0 {
            assert_eq!(work, 0);
        } else {
            assert_eq!(copies, 0);
        }
    }
}

#[test]
fn css_translate_snapshot_retains_detached_inert_and_invalid_node_distinctions() {
    let mut doc = crate::html::parse("<div id=hidden style='display:none;transform:translateX(7px)'></div><div id=detached style='transform:translateX(9px)'></div><template><div id=inert style='transform:translateX(11px)'></div></template>");
    let detached = node(&doc, "detached");
    for entry in &mut doc.nodes {
        entry.children.retain(|&child| child != detached);
    }
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    expect(&doc, &output, "hidden", translated(7.0, 0.0));
    for id in ["detached", "inert"] {
        assert_eq!(
            output.computed_properties.read_transform(node(&doc, id)),
            Ok(None)
        );
    }
    assert_eq!(
        output.computed_properties.read_transform(doc.nodes.len()),
        Err(ComputedStyleUnavailable)
    );
}

#[test]
fn css_translate_snapshot_fast_path_distinguishes_no_declarations_none_identity_and_unknown() {
    let (_, output) = cascade("div{color:red}", "<div>Plain</div>");
    assert!(!output.computed_properties.has_transform_geometry());
    assert_eq!(output.computed_properties.transform_storage_len(), 0);
    let plain = crate::html::parse("<style>div{color:red}</style><div id=plain>Plain</div>");
    let (mut work, mut copies) = (0, 0);
    let plain = compute_with_environment_and_properties_transform_budgeted(
        &plain,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    assert_eq!((work, copies), (0, 0));
    assert_eq!(plain.computed_properties.transform_storage_len(), 0);
    for (value, needed) in [
        ("none", false),
        ("translate(0)", true),
        ("rotate(10deg)", true),
    ] {
        let (_, output) = cascade("", &format!("<div style='transform:{value}'>Actual</div>"));
        assert_eq!(
            output.computed_properties.has_transform_geometry(),
            needed,
            "{value}"
        );
        assert!(output.computed_properties.transform_storage_len() > 0);
    }
    let (_, output) = cascade(
        "#missing{transform:translateX(1px)}",
        "<div>Nonmatching</div>",
    );
    assert!(!output.computed_properties.has_transform_geometry());
    assert!(output.computed_properties.transform_storage_len() > 0);
}

#[test]
fn css_translate_transform_only_meter_predebits_sheet_and_inline_grammar_before_publication() {
    for body in [
        "<style>#target{transform:translateX(12px)}</style><div id=target></div>",
        "<div id=target style='transform:translateX(12px)'></div>",
        "<style>#target{transform:rotate(banana)}</style><div id=target></div>",
    ] {
        let doc = crate::html::parse(body);
        let bytes = "transform".len()
            + if body.contains("rotate") {
                "rotate(banana)".len()
            } else {
                "translateX(12px)".len()
            };
        let (required_work, required_copies) = transform_source_cost(bytes);
        let (mut work, mut copies) = (required_work - 1, 1_000_000);
        assert!(compute_with_environment_and_properties_transform_budgeted(
            &doc,
            &HashMap::new(),
            None,
            None,
            &mut CssBudget::new(&mut work, &mut copies)
        )
        .is_err());
        assert_eq!(work, 0);
        assert_eq!(copies, 1_000_000);
        let (mut work, mut copies) = (1_000_000, required_copies - 1);
        assert!(compute_with_environment_and_properties_transform_budgeted(
            &doc,
            &HashMap::new(),
            None,
            None,
            &mut CssBudget::new(&mut work, &mut copies)
        )
        .is_err());
        assert_eq!(copies, 0);
    }
    let doc=crate::html::parse("<style>#target{--move:translate(2px,3px);transform:var(--move)}</style><div id=target></div>");
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let output = compute_with_environment_and_properties_transform_budgeted(
        &doc,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    expect(&doc, &output, "target", translated(2.0, 3.0));
    assert!(work < 1_000_000 && copies < 1_000_000);
}

#[test]
fn css_translate_var_text_remains_one_property_and_invalid_substitution_never_injects_neighbors() {
    for value in [
        "var(--move);color:red",
        "var(--move)!important",
        "var(--move){color:red}",
    ] {
        assert_eq!(
            normalize_transform(value),
            Err(TransformSyntax::Invalid),
            "{value}"
        );
    }
    let raw = "TrAnSlAtE(var(--Case, 1px), var(--other, 2px))";
    assert_eq!(normalize_transform(raw).unwrap(), raw);
    let (doc,output)=cascade("#target{--payload:';color:red';--invalid:rotate(banana);color:blue;transform:var(--payload)}#invalid{--bad:rotate(banana);transform:translateX(7px);transform:var(--bad)}",
        "<div id=target></div><div id=invalid></div>");
    expect(&doc, &output, "target", Transform::None);
    assert_eq!(output.styles[node(&doc, "target")].color, 0x0000ff);
    expect(&doc, &output, "invalid", Transform::None);
    for value in [
        "scale(50%,100%)",
        "scale3d(50%,100%,150%)",
        "perspective(none)",
        "rotate3d(1,0,0,30deg)",
    ] {
        assert_eq!(
            normalize_transform(value),
            Err(TransformSyntax::Unsupported),
            "{value}"
        );
    }
}

#[test]
fn css_translate_rejected_sheet_remains_omitted_in_unbudgeted_and_transform_budgeted_paths() {
    let mut doc = crate::html::parse(concat!(
        "<style>#target{transform:translateX(99px)!important}.extra{color:red}</style>",
        "<style>#target{transform:translateX(3px)}</style><div id=target></div>"
    ));
    doc.stylesheet_limits.rules = 1;
    assert!(!stylesheet_diagnostics(&doc, &HashMap::new()).is_empty());
    let output = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    expect(&doc, &output, "target", translated(3.0, 0.0));
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let output = compute_with_environment_and_properties_transform_budgeted(
        &doc,
        &HashMap::new(),
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    expect(&doc, &output, "target", translated(3.0, 0.0));
}
