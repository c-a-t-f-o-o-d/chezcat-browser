use super::*;

fn id(doc: &Document, wanted: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(wanted))
        .unwrap()
}
fn attribute(doc: &mut Document, node: usize, name: &str, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else {
        panic!("real Element");
    };
    if let Some(attr) = attrs.iter_mut().find(|attr| attr.name.html_is(name)) {
        attr.value = value.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified(
            name.into(),
            value.into(),
        ));
    }
}
fn opacity(result: &CascadeOutput, node: usize) -> f64 {
    result
        .computed_properties
        .read_opacity(node)
        .unwrap()
        .unwrap()
}
fn cached(
    doc: &Document,
    external: &HashMap<usize, String>,
    previous: Option<&StylesheetCache>,
) -> CascadeOutput {
    compute_with_environment_and_properties_cached(doc, external, None, None, previous)
}
fn large_sheet() -> String {
    let rule=".original-utility.state-ready{display:block;padding:1px 2px;color:#123456;background:#f7f8f9;width:auto;height:auto}\n";
    let tail = "#box{opacity:.5}";
    let mut source = rule.repeat((65536 - tail.len() - 4) / rule.len());
    source.push_str("/*");
    source.extend(std::iter::repeat_n(
        ' ',
        65536 - source.len() - tail.len() - 2,
    ));
    source.push_str("*/");
    source.push_str(tail);
    assert_eq!(source.len(), 65536);
    source
}

#[test]
fn stylesheet_cache_id_candidates_keep_live_ids_full_matching_and_cascade_order() {
    let mut doc = crate::html::parse("<style>#alpha{opacity:.2}#beta,.selected{opacity:.4}#beta{opacity:.6}#alpha#beta{opacity:0}</style><body><p id=alpha></p><p id=beta></p><p id=other class=selected></p>");
    let (a, b, other) = (id(&doc, "alpha"), id(&doc, "beta"), id(&doc, "other"));
    let sources = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &sources, None, None, None);
    assert_eq!((opacity(&first, a), opacity(&first, b), opacity(&first, other)), (0.2, 0.6, 0.4));
    attribute(&mut doc, a, "id", "beta");
    let NodeKind::Element { id: current, .. } = &mut doc.nodes[a].kind else { unreachable!() };
    *current = Some("beta".into());
    let next = compute_with_environment_and_properties_cached(&doc, &sources, None, None, first.stylesheet_cache.as_ref());
    assert_eq!((opacity(&next, a), opacity(&next, b), opacity(&next, other)), (0.6, 0.6, 0.4));
}

#[test]
fn stylesheet_cache_same_job_inline_mutations_reuse_syntax_with_live_values_and_original_budget() {
    let mut doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=box></div>");
    let sheet = id(&doc, "sheet");
    let node = id(&doc, "box");
    let external = HashMap::from([(sheet, large_sheet())]);
    let (mut work, mut copies) = (1_000_000, 16_000_000);
    let first = compute_with_environment_and_properties_budgeted_cached(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
        None,
    )
    .unwrap();
    assert_eq!(opacity(&first, node), 0.5);
    let source = first.stylesheet_cache.unwrap();
    assert!(
        work < 65536 / 64 * 512,
        "remaining job allowance cannot pay a second cold source reserve"
    );
    let (mut cold_work, mut cold_copies) = (work, copies);
    let cold = compute_with_environment_and_properties_budgeted(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut cold_work, &mut cold_copies),
    );
    assert_eq!(
        cold.err().as_deref(),
        Some("CSS computation work limit reached.")
    );
    assert_eq!(cold_work, 0);
    for value in ["1", "0.5", "1", "0.5"] {
        attribute(&mut doc, node, "style", &format!("opacity:{value}"));
        let before = (work, copies);
        let result = compute_with_environment_and_properties_budgeted_cached(
            &doc,
            &external,
            None,
            None,
            &mut CssBudget::new(&mut work, &mut copies),
            Some(&source),
        )
        .unwrap();
        assert_eq!(opacity(&result, node), value.parse::<f64>().unwrap());
        assert!(source.shares_rules(result.stylesheet_cache.as_ref().unwrap()));
        assert!(
            work < before.0 && copies < before.1,
            "a cache hit does not refill either allowance"
        );
    }
    assert!(work > 0 && copies > 0);
}

#[test]
fn selector_candidates_leave_budget_for_opacity_after_unrelated_rules() {
    use std::fmt::Write;

    let markup = format!(
        "<link id=sheet rel=stylesheet>{}<p id=probe class=marked></p><section id=group class=panel-37><p id=member class=item-37></p></section>",
        "<p class=unrelated></p>".repeat(180),
    );
    let doc = crate::html::parse(&markup);
    let mut css = String::new();
    for number in 0..600 {
        writeln!(css, ".panel-{number} > .item-{number} {{ opacity: .1 }}").unwrap();
    }
    css.push_str("* { opacity: .25 } .marked, section { opacity: .5 } #probe { opacity: .75 }");
    let external = HashMap::from([(id(&doc, "sheet"), css)]);
    let probe = id(&doc, "probe");
    let member = id(&doc, "member");
    let original = cached(&doc, &external, None);
    assert_eq!(opacity(&original, probe), 0.75);
    let mut work = 250_000;
    let mut copies = 32_000_000;
    let updated = compute_with_environment_and_properties_budgeted_cached(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
        original.stylesheet_cache.as_ref(),
    ).unwrap();
    assert_eq!(opacity(&updated, probe), 0.75);
    assert_eq!(opacity(&updated, member), 0.1);
    assert_eq!(opacity(&updated, id(&doc, "group")), 0.5);
    assert!(work > 0);
}

#[test]
fn stylesheet_cache_comparison_has_exact_precharged_work_and_no_copy_or_refill() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=box></div>");
    let sheet = id(&doc, "sheet");
    let external = HashMap::from([(sheet, large_sheet())]);
    let cache = cached(&doc, &external, None).stylesheet_cache.unwrap();
    let sources = stylesheet_sources_with(&doc, &external, None, None).unwrap();
    let (mut work, mut copies) = (1025, 117);
    assert!(cache
        .matches(
            &doc,
            &sources,
            None,
            Some(&mut CssBudget::new(&mut work, &mut copies))
        )
        .unwrap());
    assert_eq!((work, copies), (0, 117));
    let (mut work, mut copies) = (1024, 117);
    assert_eq!(
        cache
            .matches(
                &doc,
                &sources,
                None,
                Some(&mut CssBudget::new(&mut work, &mut copies))
            )
            .err()
            .as_deref(),
        Some("CSS computation work limit reached.")
    );
    assert_eq!((work, copies), (0, 117));
    let mismatch = HashMap::from([(sheet, "#box{opacity:1}".into())]);
    let sources = stylesheet_sources_with(&doc, &mismatch, None, None).unwrap();
    let (mut work, mut copies) = (1, 117);
    assert!(!cache
        .matches(
            &doc,
            &sources,
            None,
            Some(&mut CssBudget::new(&mut work, &mut copies))
        )
        .unwrap());
    assert_eq!(
        (work, copies),
        (0, 117),
        "different length never scans unrelated source bytes"
    );
}

#[test]
fn stylesheet_cache_revalidates_external_bytes_order_media_and_admission_limits() {
    let mut doc = crate::html::parse(
        "<head><link id=a rel=stylesheet><link id=b rel=stylesheet></head><div id=box></div>",
    );
    let a = id(&doc, "a");
    let b = id(&doc, "b");
    let node = id(&doc, "box");
    let mut external = HashMap::from([
        (a, "#box{opacity:.25}".into()),
        (b, "#box{opacity:.75}".into()),
    ]);
    let first = cached(&doc, &external, None);
    assert_eq!(opacity(&first, node), 0.75);
    let original = first.stylesheet_cache.unwrap();
    external.insert(b, "#box{opacity:.50}".into());
    let changed = cached(&doc, &external, Some(&original));
    assert_eq!(opacity(&changed, node), 0.5);
    assert!(!original.shares_rules(changed.stylesheet_cache.as_ref().unwrap()));
    let parent = doc
        .nodes
        .iter()
        .position(|node| node.children.contains(&a) && node.children.contains(&b))
        .unwrap();
    let ai = doc.nodes[parent]
        .children
        .iter()
        .position(|&node| node == a)
        .unwrap();
    let bi = doc.nodes[parent]
        .children
        .iter()
        .position(|&node| node == b)
        .unwrap();
    doc.nodes[parent].children.swap(ai, bi);
    let reordered = cached(&doc, &external, changed.stylesheet_cache.as_ref());
    assert_eq!(opacity(&reordered, node), 0.25);
    external.insert(
        a,
        "#box{opacity:.25}@media(min-width:700px){#box{opacity:.9}}".into(),
    );
    let narrow = Some(MediaEnvironment {
        width: 600.0,
        height: 400.0,
        hover: Hover::Hover,
    });
    let wide = Some(MediaEnvironment {
        width: 900.0,
        height: 400.0,
        hover: Hover::Hover,
    });
    let n = compute_with_environment_and_properties_cached(
        &doc,
        &external,
        None,
        narrow,
        reordered.stylesheet_cache.as_ref(),
    );
    assert_eq!(opacity(&n, node), 0.25);
    let w = compute_with_environment_and_properties_cached(
        &doc,
        &external,
        None,
        wide,
        n.stylesheet_cache.as_ref(),
    );
    assert_eq!(opacity(&w, node), 0.9);
    assert!(!n
        .stylesheet_cache
        .as_ref()
        .unwrap()
        .shares_rules(w.stylesheet_cache.as_ref().unwrap()));
    doc.stylesheet_limits.sheet_bytes = 8;
    let refused = compute_with_environment_and_properties_cached(
        &doc,
        &external,
        None,
        wide,
        w.stylesheet_cache.as_ref(),
    );
    assert_eq!(opacity(&refused, node), 1.0);
    assert!(refused.stylesheet_cache.is_none());
    assert_eq!(stylesheet_diagnostics(&doc, &external).len(), 2);
}

#[test]
fn stylesheet_cache_withholding_and_readmitting_the_same_inline_source_changes_the_cascade() {
    let mut doc = crate::html::parse(
        "<style>#box{opacity:.25}</style><style id=dynamic>#box{opacity:.75}</style><div id=box></div>",
    );
    let sheet = id(&doc, "dynamic");
    let node = id(&doc, "box");
    let external = HashMap::new();
    let initial = cached(&doc, &external, None);
    assert_eq!(opacity(&initial, node), 0.75);
    let initial_cache = initial.stylesheet_cache.unwrap();

    doc.withheld_inline_styles.insert(sheet);
    let denied = cached(&doc, &external, Some(&initial_cache));
    assert_eq!(opacity(&denied, node), 0.25);
    let denied_cache = denied.stylesheet_cache.unwrap();
    assert!(!initial_cache.shares_rules(&denied_cache));

    doc.withheld_inline_styles.remove(&sheet);
    let restored = cached(&doc, &external, Some(&denied_cache));
    assert_eq!(opacity(&restored, node), 0.75);
    assert!(!denied_cache.shares_rules(restored.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn stylesheet_cache_connected_inert_and_scripting_source_scope_remains_live() {
    let mut doc=crate::html::parse("<style id=live>#box{opacity:.25}</style><template id=hidden><style id=inside>#box{opacity:.75}</style></template><noscript><style>#box{opacity:.9}</style></noscript><div id=box></div>");
    doc.scripting_enabled = true;
    let node = id(&doc, "box");
    let inside = id(&doc, "inside");
    let hidden = id(&doc, "hidden");
    let external = HashMap::new();
    let first = cached(&doc, &external, None);
    assert_eq!(opacity(&first, node), 0.25);
    let source = first.stylesheet_cache.unwrap();
    doc.scripting_enabled = false;
    let noscript = cached(&doc, &external, Some(&source));
    assert_eq!(opacity(&noscript, node), 0.9);
    assert!(!source.shares_rules(noscript.stylesheet_cache.as_ref().unwrap()));
    doc.scripting_enabled = true;
    assert!(source.shares_rules(
        cached(&doc, &external, Some(&source))
            .stylesheet_cache
            .as_ref()
            .unwrap()
    ));
    doc.nodes[hidden].children.retain(|&child| child != inside);
    doc.nodes[0].children.push(inside);
    let moved = cached(&doc, &external, Some(&source));
    assert_eq!(opacity(&moved, node), 0.75);
    assert!(!source.shares_rules(moved.stylesheet_cache.as_ref().unwrap()));
    doc.nodes[0].children.retain(|&child| child != inside);
    let detached = cached(&doc, &external, moved.stylesheet_cache.as_ref());
    assert_eq!(opacity(&detached, node), 0.25);
}

#[test]
fn stylesheet_cache_hits_recascade_dom_checkedness_and_inherited_variables() {
    let mut doc=crate::html::parse("<style>#parent{--o:.25}#parent.active{--o:.75}#box{opacity:var(--o)}input:checked + #box{opacity:.5}</style><div id=parent><input id=check type=checkbox><div id=box></div></div>");
    let parent = id(&doc, "parent");
    let check = id(&doc, "check");
    let node = id(&doc, "box");
    let external = HashMap::new();
    let initial = compute_with_environment_and_properties_cached(
        &doc,
        &external,
        Some(&|index| (index == check).then_some(false)),
        None,
        None,
    );
    assert_eq!(opacity(&initial, node), 0.25);
    let source = initial.stylesheet_cache.unwrap();
    attribute(&mut doc, parent, "class", "active");
    let class = compute_with_environment_and_properties_cached(
        &doc,
        &external,
        Some(&|index| (index == check).then_some(false)),
        None,
        Some(&source),
    );
    assert_eq!(opacity(&class, node), 0.75);
    assert!(source.shares_rules(class.stylesheet_cache.as_ref().unwrap()));
    let checked = compute_with_environment_and_properties_cached(
        &doc,
        &external,
        Some(&|index| (index == check).then_some(true)),
        None,
        Some(&source),
    );
    assert_eq!(opacity(&checked, node), 0.5);
    assert!(source.shares_rules(checked.stylesheet_cache.as_ref().unwrap()));
    assert_eq!(
        checked.computed_properties.read(node, "--o").unwrap(),
        Some(".75")
    );
}

#[test]
fn stylesheet_cache_cold_and_changed_sources_only_charge_active_sheets() {
    let large = large_sheet();
    let mut doc = crate::html::parse(&format!(
        "<link rel=stylesheet id=live><link rel=stylesheet id=inactive media=print>+         <style id=blocked>{large}</style><template><style>{large}</style></template>+         <div id=box></div>"
    ));
    let active = id(&doc, "live");
    let inactive = id(&doc, "inactive");
    let blocked = id(&doc, "blocked");
    let node = id(&doc, "box");
    doc.withheld_inline_styles.insert(blocked);
    let mut external = HashMap::from([(active, "#box{opacity:.25}".into()), (inactive, large)]);
    let first = compute_with_environment_and_properties_budgeted_cached(
        &doc, &external, None, None, &mut CssBudget::new(&mut 100_000, &mut 8_000_000), None,
    ).unwrap();
    assert_eq!(opacity(&first, node), 0.25);
    let cache = first.stylesheet_cache.as_ref().unwrap();

    external.insert(active, "#box{opacity:.75}".into());
    let second = compute_with_environment_and_properties_budgeted_cached(
        &doc, &external, None, None,
        &mut CssBudget::new(&mut 100_000, &mut 8_000_000), Some(cache),
    ).unwrap();
    assert_eq!(opacity(&second, node), 0.75);
    assert!(!cache.shares_rules(second.stylesheet_cache.as_ref().unwrap()));

    doc.withheld_inline_styles.remove(&blocked);
    let mut work = 100_000;
    let denied = compute_with_environment_and_properties_budgeted_cached(
        &doc, &external, None, None, &mut CssBudget::new(&mut work, &mut 8_000_000),
        second.stylesheet_cache.as_ref(),
    );
    assert_eq!(denied.err().as_deref(), Some("CSS computation work limit reached."));
    assert_eq!(work, 0);
}

#[test]
fn stylesheet_cache_failed_miss_and_refused_later_sheet_do_not_publish_partial_cache() {
    let mut doc = crate::html::parse(
        "<link id=a rel=stylesheet><link id=b rel=stylesheet><div id=box></div>",
    );
    let a = id(&doc, "a");
    let b = id(&doc, "b");
    let node = id(&doc, "box");
    let mut external = HashMap::from([(a, large_sheet())]);
    let first = cached(&doc, &external, None);
    let source = first.stylesheet_cache.unwrap();
    external.get_mut(&a).unwrap().replace_range(0..1, " ");
    let (mut work, mut copies) = (100_000, 8_000_000);
    let failed = compute_with_environment_and_properties_budgeted_cached(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut work, &mut copies),
        Some(&source),
    );
    assert_eq!(
        failed.err().as_deref(),
        Some("CSS computation work limit reached.")
    );
    assert_eq!(work, 0);
    external.insert(a, large_sheet());
    let (mut next_job_work, mut next_job_copies) = (100_000, 8_000_000);
    let recovered = compute_with_environment_and_properties_budgeted_cached(
        &doc,
        &external,
        None,
        None,
        &mut CssBudget::new(&mut next_job_work, &mut next_job_copies),
        Some(&source),
    )
    .unwrap();
    assert_eq!(opacity(&recovered, node), 0.5);
    assert!(source.shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
    assert_eq!(work, 0);
    doc.stylesheet_limits.sheet_bytes = 65536;
    external.insert(b, format!("#box{{opacity:0}}/*{}*/", "x".repeat(65536)));
    let refused = cached(&doc, &external, Some(&source));
    assert_eq!(opacity(&refused, node), 0.5);
    assert!(refused.stylesheet_cache.is_none());
    assert!(stylesheet_diagnostics(&doc, &external)
        .iter()
        .any(|text| text.contains("Complete stylesheet refused")));
}
