use super::*;

fn node(doc: &Document, id: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(id))
        .unwrap()
}
fn read<'a>(snapshot: &'a ComputedProperties, node: usize, name: &str) -> &'a str {
    snapshot.read(node, name).unwrap().unwrap().trim()
}

#[test]
fn computed_custom_readback_uses_real_external_inline_priority_and_resolved_inheritance() {
    let doc = crate::html::parse("<html><head><link id=sheet rel=stylesheet><style>#parent{--x:4px;--alias:var(--x);--Priority:sheet!important;--Case:upper;--case:lower;--empty:;--cycle:var(--cycle)} #child{--Priority:rule!important}</style></head><body><div id=parent><div id=child style='--x:9px;--Priority:inline!important;--fallback:var(--missing, end);padding:var(--alias)'></div></div></body></html>");
    let mut external = HashMap::new();
    external.insert(
        node(&doc, "sheet"),
        "#child{--external:present;--Priority:external!important}".into(),
    );
    let result = compute_with_environment_and_properties(&doc, &external, None, None);
    let child = node(&doc, "child");
    let values = &result.computed_properties;
    assert_eq!(read(values, child, "--Priority"), "inline");
    assert_eq!(read(values, child, "--external"), "present");
    assert_eq!(read(values, child, "--x"), "9px");
    assert_eq!(read(values, child, "--alias"), "4px");
    assert_eq!(read(values, child, "--fallback"), "end");
    assert_eq!(read(values, child, "--Case"), "upper");
    assert_eq!(read(values, child, "--case"), "lower");
    assert_eq!(values.read(child, "--empty"), Ok(Some("")));
    assert_eq!(values.read(child, "--cycle"), Ok(None));
    assert_eq!(values.read(child, "--absent"), Ok(None));
    assert_eq!(result.styles[child].padding, [Length::Px(4.0); 4]);
}

#[test]
fn computed_custom_readback_respects_initial_inherit_unset_and_ordinary_specificity() {
    let doc = crate::html::parse("<style>#parent{--v:parent} div{--v:tag} .choice{--v:class} #child{--v:id;--v:later} #initial{--v:initial} #inherit{--v:inherit} #unset{--v:unset}</style><div id=parent><div id=child class=choice></div><div id=initial></div><div id=inherit></div><div id=unset></div></div>");
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    assert_eq!(
        read(&result.computed_properties, node(&doc, "child"), "--v"),
        "later"
    );
    assert_eq!(
        result.computed_properties.read(node(&doc, "initial"), "--v"),
        Ok(None)
    );
    for id in ["inherit", "unset"] {
        assert_eq!(
            read(&result.computed_properties, node(&doc, id), "--v"),
            "parent"
        );
    }
}

#[test]
fn computed_custom_readback_uses_actual_fractional_media_and_live_checkedness() {
    let doc = crate::html::parse("<style>#result{--viewport:narrow;--check:off}@media(min-width:64em){#result{--viewport:wide}}input:checked + #result{--check:on}</style><input id=check type=checkbox><div id=result></div>");
    let check = node(&doc, "check");
    let target = node(&doc, "result");
    for (width, live, viewport, checked) in [
        (1023.75, false, "narrow", "off"),
        (1024.0, true, "wide", "on"),
    ] {
        let environment = Some(MediaEnvironment {
            width,
            height: 660.0,
            hover: Hover::Hover,
        });
        let state = |node| (node == check).then_some(live);
        let result = compute_with_environment_and_properties(
            &doc,
            &HashMap::new(),
            Some(&state),
            environment,
        );
        assert_eq!(
            read(&result.computed_properties, target, "--viewport"),
            viewport
        );
        assert_eq!(read(&result.computed_properties, target, "--check"), checked);
    }
}

#[test]
fn computed_custom_readback_has_no_detached_or_inert_style_and_reports_blocked_maps() {
    let mut doc = crate::html::parse("<style>div{--v:live}</style><div id=detached></div><template><div id=inert></div></template><div id=blocked></div>");
    let detached = node(&doc, "detached");
    for entry in &mut doc.nodes {
        entry.children.retain(|&child| child != detached);
    }
    let blocked = node(&doc, "blocked");
    if let NodeKind::Element { attrs, .. } = &mut doc.nodes[blocked].kind {
        attrs.push(crate::html::Attribute {
            name: crate::html::AttributeName::unqualified("style".into()),
            value: (0..257).map(|n| format!("--x{n}:value;")).collect(),
        });
    }
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    assert_eq!(result.computed_properties.read(detached, "--v"), Ok(None));
    assert_eq!(
        result.computed_properties.read(node(&doc, "inert"), "--v"),
        Ok(None)
    );
    assert_eq!(
        result.computed_properties.read(blocked, "--v"),
        Err(ComputedStyleUnavailable)
    );
    assert_eq!(
        result.computed_properties.read(doc.nodes.len(), "--v"),
        Err(ComputedStyleUnavailable)
    );
}

#[test]
fn computed_custom_readback_shares_job_counters_and_never_returns_a_partial_budget_result() {
    let doc = crate::html::parse("<style>#parent{--x:4px}#child{--alias:var(--x);padding:var(--alias)}</style><div id=parent><div id=child></div></div>");
    let external = HashMap::new();
    let mut work = 64_000_000;
    let mut copies = 64 * 1024 * 1024;
    let start = work;
    let result = compute_with_environment_and_properties_budgeted(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
    )
    .unwrap();
    assert_eq!(
        read(&result.computed_properties, node(&doc, "child"), "--alias"),
        "4px"
    );
    assert_eq!(
        result.styles[node(&doc, "child")].padding,
        [Length::Px(4.0); 4]
    );
    assert!(copies < 64 * 1024 * 1024);
    let used = start - work;
    assert!(used > doc.nodes.len());
    let mut shared_work = used * 2 - 1;
    let mut shared_copies = 64 * 1024 * 1024;
    compute_with_environment_and_properties_budgeted(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut shared_work, &mut shared_copies),
    )
    .unwrap();
    assert!(compute_with_environment_and_properties_budgeted(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut shared_work, &mut shared_copies)
    )
    .is_err());
    assert_eq!(shared_work, 0);
    let mut work = 64_000_000;
    let mut copies = 0;
    assert!(compute_with_environment_and_properties_budgeted(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies)
    )
    .is_err());
    assert_eq!(copies, 0);
}

#[test]
fn computed_custom_readback_incomplete_snapshot_is_indeterminate_and_names_share_the_finite_grammar(
) {
    let snapshot = ComputedProperties::new(
        vec![custom_properties::Variables::default()],
        vec![true],
        false,
        vec![Some(Overflow::default())],
        vec![Some(Position::Static)],
        vec![Some(Transform::None)],
        false,
        HashMap::from([(0, 0x123456)]),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        Vec::new(),
    );
    assert_eq!(snapshot.read(0, "--absent"), Err(ComputedStyleUnavailable));
    assert_eq!(snapshot.read_overflow(0), Err(ComputedStyleUnavailable));
    assert_eq!(snapshot.read_font_size(0), Err(ComputedStyleUnavailable));
    assert_eq!(snapshot.read_position(0), Err(ComputedStyleUnavailable));
    assert_eq!(snapshot.read_transform(0), Err(ComputedStyleUnavailable));
    assert_eq!(snapshot.canvas_foreground(0), Err(ComputedStyleUnavailable));
    assert_eq!(snapshot.read_opacity(0), Err(ComputedStyleUnavailable));
    for name in ["--bs-position", "--Case", "--é"] {
        assert!(computed_custom_property_name(name));
    }
    for name in ["", "padding", "--", "--has space", "--escaped\\name"] {
        assert!(!computed_custom_property_name(name));
    }
}

#[test]
fn computed_custom_variable_budget_keeps_local_ceiling_and_debits_each_operation_exactly() {
    let variables = custom_properties::Variables::default();
    let mut local = custom_properties::Budget::default();
    let initial = local.remaining();
    let mut work = 100;
    let mut copies = 100;
    for _ in 0..2 {
        let value = CssBudget::new(&mut work, &mut copies)
            .variables(&mut local, |budget| variables.substitute("a", budget))
            .unwrap()
            .unwrap();
        assert_eq!(value.as_deref(), Some("a"));
    }
    // Each one-byte literal pays one scanned byte, a 32-unit parts record and
    // one copied byte. The renderer cap must not be replaced with the job cap.
    assert_eq!((work, copies), (32, 98));
    assert_eq!(local.remaining(), (initial.0 - 68, initial.1 - 2));
    let failed = CssBudget::new(&mut work, &mut copies).variables(&mut local, |budget| {
        variables.substitute("abcdefghijklmnopqrstuvwxyz1234567890", budget)
    });
    assert!(failed.is_err());
    assert_eq!(
        (work, copies),
        (0, 98),
        "refused work must not charge unperformed string copies"
    );

    let mut local = custom_properties::Budget::default();
    let mut work = 34;
    let mut copies = 1;
    let exact = CssBudget::new(&mut work, &mut copies)
        .variables(&mut local, |budget| variables.substitute("a", budget))
        .unwrap()
        .unwrap();
    assert_eq!(exact.as_deref(), Some("a"));
    assert_eq!(
        (work, copies),
        (0, 0),
        "an exact successful debit is allowed"
    );

    let mut local = custom_properties::Budget::default();
    let mut work = 100;
    let mut copies = 0;
    assert!(CssBudget::new(&mut work, &mut copies)
        .variables(&mut local, |budget| variables.substitute("a", budget))
        .is_err());
    assert_eq!(
        (work, copies),
        (66, 0),
        "copy refusal preserves the actual preceding work debit"
    );
}
