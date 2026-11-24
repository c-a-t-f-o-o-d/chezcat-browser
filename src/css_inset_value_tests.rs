use super::*;

const PROPERTIES: [(&str, Option<usize>); 5] = [
    ("inset", None), ("top", Some(0)), ("right", Some(1)),
    ("bottom", Some(2)), ("left", Some(3)),
];
const INLINE_NAMES: [&str; 5] = ["bottom", "inset", "left", "right", "top"];

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!(
        "<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"
    ))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style {
    &styles[id(doc, name)]
}

fn pixels(values: [f32; 4]) -> [Length; 4] { values.map(Length::Px) }

fn resolved(style: &Style, reference: f32) -> [Option<f32>; 4] {
    style.inset.map(|length| length.resolve(reference, style.font_size, style.root_font_size))
}

fn environment(width: f64, height: f64) -> MediaEnvironment {
    MediaEnvironment { width, height, hover: Hover::Hover }
}

fn set_style(doc: &mut Document, node: usize, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { panic!("element required"); };
    if let Some(attr) = attrs.iter_mut().find(|attr| attr.name.html_is("style")) {
        attr.value = value.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified("style".into(), value.into()));
    }
}

fn assert_names(source: &str, expected: &[&str]) {
    let actual = inline_declaration_names(source, &INLINE_NAMES, None).unwrap();
    assert_eq!(actual.iter().map(String::as_str).collect::<Vec<_>>(), expected.to_vec(), "{source}");
}

#[test]
fn inset_values_plain_shorthands_expand_one_through_four_components() {
    for (value, expected) in [
        ("7px", pixels([7.0; 4])),
        ("-2px 25%", [Length::Px(-2.0), Length::Percent(25.0), Length::Px(-2.0), Length::Percent(25.0)]),
        ("auto -3px 4em", [Length::Auto, Length::Px(-3.0), Length::Em(4.0), Length::Px(-3.0)]),
        ("0 2rem -5% auto", [Length::Px(0.0), Length::Rem(2.0), Length::Percent(-5.0), Length::Auto]),
    ] {
        let doc = document(&format!("#target{{inset:{value}}}"), "<div id=target></div>");
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "target").inset, expected, "{value}");
    }
}

#[test]
fn inset_values_each_longhand_changes_only_its_physical_side() {
    for (property, side) in PROPERTIES.into_iter().skip(1) {
        let doc = document(&format!("#target{{inset:1px 2px 3px 4px;{property}:-5%}}"), "<div id=target></div>");
        let styles = compute(&doc, &HashMap::new());
        let mut expected = pixels([1.0, 2.0, 3.0, 4.0]);
        expected[side.unwrap()] = Length::Percent(-5.0);
        assert_eq!(at(&doc, &styles, "target").inset, expected, "{property}");
    }
}

#[test]
fn inset_values_literal_inherit_copies_computed_values_for_all_five_properties() {
    let inherited = [Length::Px(40.0), Length::Percent(25.0), Length::Auto, Length::Px(-30.0)];
    for (property, side) in PROPERTIES {
        let doc = document(
            &format!("html{{font-size:10px}}#parent{{font-size:20px;inset:2em 25% auto -3rem}}#child{{font-size:70px;inset:91px;{property}:InHeRiT}}"),
            "<div id=parent><div id=child></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        let mut expected = pixels([91.0; 4]);
        if let Some(side) = side { expected[side] = inherited[side]; } else { expected = inherited; }
        assert_eq!(at(&doc, &styles, "child").inset, expected, "{property}");
    }
}

#[test]
fn inset_values_initial_and_unset_reset_only_the_declared_components() {
    for keyword in ["initial", "unset"] {
        for (property, side) in PROPERTIES {
            for value in [keyword.to_owned(), format!("var(--missing,{keyword})")] {
                let doc = document(
                    &format!("#parent{{inset:19px}}#child{{inset:1px 2px 3px 4px;{property}:{value}}}"),
                    "<div id=parent><div id=child></div></div>",
                );
                let styles = compute(&doc, &HashMap::new());
                let mut expected = pixels([1.0, 2.0, 3.0, 4.0]);
                if let Some(side) = side { expected[side] = Length::Auto; } else { expected = [Length::Auto; 4]; }
                assert_eq!(at(&doc, &styles, "child").inset, expected, "{property}:{value}");
            }
        }
    }
}

#[test]
fn inset_values_ordinary_descendants_do_not_inherit_physical_offsets() {
    let doc = document(
        "#parent{position:relative;inset:2em 3rem 25% -4px}#unset{inset:unset}#variable{inset:var(--missing,unset)}",
        "<div id=parent><div id=ordinary></div><div id=unset></div><div id=variable></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for name in ["ordinary", "unset", "variable"] {
        assert_eq!(at(&doc, &styles, name).inset, [Length::Auto; 4], "{name}");
    }
}

#[test]
fn inset_values_root_keywords_use_initial_offsets_without_an_element_parent() {
    for (property, side) in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            for value in [keyword.to_owned(), format!("var(--missing,{keyword})")] {
                let doc = crate::html::parse(&format!(
                    "<!doctype html><html id=root style='inset:1px 2px 3px 4px;{property}:{value}'><head></head><body></body></html>"
                ));
                let styles = compute(&doc, &HashMap::new());
                let mut expected = pixels([1.0, 2.0, 3.0, 4.0]);
                if let Some(side) = side { expected[side] = Length::Auto; } else { expected = [Length::Auto; 4]; }
                assert_eq!(at(&doc, &styles, "root").inset, expected, "{property}:{value}");
            }
        }
    }
}

#[test]
fn inset_values_universal_rules_on_the_document_sentinel_do_not_supply_root_inheritance() {
    for value in ["inherit", "var(--missing,inherit)"] {
        let doc = crate::html::parse(&format!(
            "<!doctype html><html id=root style='inset:{value}'><head><style>*{{inset:19px 23px 29px 31px}}</style></head><body></body></html>"
        ));
        assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "root").inset, [Length::Auto; 4]);
    }
}

#[test]
fn inset_values_fragment_context_is_excluded_but_real_null_namespace_parent_is_used() {
    for value in ["inherit", "var(--missing,inherit)"] {
        let mut doc = crate::html::parse_fragment(
            "<link id=sheet rel=stylesheet><section id=top><div id=real><span id=inner></span></div></section>",
            "div", false,
        ).unwrap();
        assert!(matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == "div"));
        assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
        let real = id(&doc, "real");
        doc.nodes[real].namespace = crate::html::Namespace::None;
        let sources = HashMap::from([(id(&doc, "sheet"), format!(
            "div{{inset:99px}}#top{{inset:{value}}}#real{{font-size:10px;inset:2em 3px auto -5%}}#inner{{font-size:70px;inset:{value}}}"
        ))]);
        let styles = compute(&doc, &sources);
        assert_eq!(at(&doc, &styles, "top").inset, [Length::Auto; 4]);
        assert_eq!(at(&doc, &styles, "inner").inset, [Length::Px(20.0), Length::Px(3.0), Length::Auto, Length::Percent(-5.0)]);
    }
}

#[test]
fn inset_values_literal_inheritance_uses_final_parent_font_and_root_metrics() {
    let doc = document(
        "html{font-size:10px}#parent{inset:2em 3rem -1em -2rem;font-size:20px}#child{inset:inherit;font-size:70px}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(at(&doc, &styles, "child").inset, pixels([40.0, 30.0, -20.0, -20.0]));
}

#[test]
fn inset_values_variable_inheritance_uses_parent_computation_for_each_longhand() {
    let doc = document(
        "html{font-size:12px}#parent{font-size:20px;inset:2em 3rem -1em -2rem}#child{font-size:60px;top:var(--x,inherit);right:var(--x,inherit);bottom:var(--x,inherit);left:var(--x,inherit)}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(at(&doc, &styles, "child").inset, pixels([40.0, 36.0, -20.0, -24.0]));
}

#[test]
fn inset_values_multigeneration_inheritance_keeps_computed_font_terms_and_deferred_percentages() {
    let doc = document(
        "html{font-size:10px}#parent{font-size:20px;inset:calc(2em + 1rem + 10%) auto -1em 2rem}#child{font-size:70px;inset:inherit}#grandchild{font-size:100px;inset:var(--missing,inherit)}",
        "<div id=parent><div id=child><div id=grandchild></div></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for name in ["child", "grandchild"] {
        let style = at(&doc, &styles, name);
        assert!(style.inset[0].has_percentage());
        assert_eq!(resolved(style, 200.0), [Some(70.0), None, Some(-20.0), Some(20.0)], "{name}");
        assert_eq!(style.inset[0].resolve(400.0, 200.0, 300.0), Some(90.0));
    }
}

#[test]
fn inset_values_typed_calculations_accept_signed_sums_products_division_and_nested_groups() {
    for (value, expected) in [
        ("calc(50% - 20px)", 80.0),
        ("calc(-10px * 2)", -20.0),
        ("calc((1em + 2rem) * 2)", 80.0),
        ("calc(120px / 3 + 10%)", 60.0),
        ("calc(2 * calc(10px + 5%))", 40.0),
        ("calc(-2em + 3rem)", -10.0),
        ("calc(10px - 50%)", -90.0),
    ] {
        let doc = document(
            &format!("html{{font-size:10px}}#target{{font-size:20px;top:{value}}}"),
            "<div id=target></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(resolved(at(&doc, &styles, "target"), 200.0)[0], Some(expected), "{value}");
    }
}

#[test]
fn inset_values_shorthand_keeps_calculation_groups_atomic_and_expands_missing_sides() {
    for (value, expected) in [
        ("calc(1em + 2px)", [Some(22.0); 4]),
        ("calc(2px + 1em) auto", [Some(22.0), None, Some(22.0), None]),
        ("calc(2px + 1em) auto calc(10% - 30px)", [Some(22.0), None, Some(-10.0), None]),
        ("calc(1em + 1rem) calc(50% - 10px) calc(-2 * 1em) calc(2 * (5px + 5%))", [Some(30.0), Some(90.0), Some(-40.0), Some(30.0)]),
    ] {
        let doc = document(
            &format!("html{{font-size:10px}}#target{{font-size:20px;inset:{value}}}"),
            "<div id=target></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(resolved(at(&doc, &styles, "target"), 200.0), expected, "{value}");
    }
}

#[test]
fn inset_values_inherited_percentages_and_auto_remain_unresolved_until_the_containing_block() {
    let doc = document(
        "#parent{inset:-25% 50% auto 10%;font-size:10px}#child{inset:inherit;font-size:70px}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    let child = at(&doc, &styles, "child");
    assert_eq!(child.inset, [Length::Percent(-25.0), Length::Percent(50.0), Length::Auto, Length::Percent(10.0)]);
    assert_eq!(resolved(child, 200.0), [Some(-50.0), Some(100.0), None, Some(20.0)]);
    assert_eq!(resolved(child, 400.0), [Some(-100.0), Some(200.0), None, Some(40.0)]);
}

#[test]
fn inset_values_root_font_calculations_use_final_root_metrics_before_child_inheritance() {
    let doc = crate::html::parse(
        "<!doctype html><html id=root style='inset:calc(2rem + 1em);font-size:10px'><head></head><body id=child style='font-size:70px;inset:inherit'></body></html>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(resolved(at(&doc, &styles, "root"), 200.0), [Some(30.0); 4]);
    let child = at(&doc, &styles, "child");
    assert_eq!(resolved(child, 200.0), [Some(30.0); 4]);
    for length in child.inset { assert_eq!(length.resolve(200.0, 100.0, 200.0), Some(30.0)); }
}

#[test]
fn inset_values_viewport_literals_accept_negative_offsets_on_both_axes() {
    let doc = document("#target{inset:5vw -10vh 2vw -5vh}", "<div id=target></div>");
    let styles = compute_with_environment(&doc, &HashMap::new(), None, Some(environment(1000.0, 600.0)));
    assert_eq!(resolved(at(&doc, &styles, "target"), 200.0), [Some(50.0), Some(-60.0), Some(20.0), Some(-30.0)]);
}

#[test]
fn inset_values_mixed_viewport_font_and_percentage_math_resolves_without_clamping_negative_results() {
    let doc = document(
        "html{font-size:10px}#target{font-size:20px;--gutter:2vw;inset:calc(5vw - var(--gutter) + 10%) calc(-10vh + 1em) calc(2rem - 5vw) auto}",
        "<div id=target></div>",
    );
    let styles = compute_with_environment(&doc, &HashMap::new(), None, Some(environment(1000.0, 600.0)));
    let target = at(&doc, &styles, "target");
    assert!(target.inset[0].has_percentage());
    assert_eq!(resolved(target, 200.0), [Some(50.0), Some(-40.0), Some(-30.0), None]);
    assert_eq!(resolved(target, 400.0)[0], Some(70.0));
}

#[test]
fn inset_values_missing_or_invalid_viewport_environment_is_not_replaced_with_a_fake_size() {
    let doc = document(
        "#target{top:5vw;right:calc(2vh + 3px);bottom:var(--missing,-1vw);left:4px}",
        "<div id=target></div>",
    );
    for env in [None, Some(environment(-1.0, 600.0)), Some(environment(f64::NAN, 600.0)), Some(environment(800.0, f64::INFINITY))] {
        let styles = compute_with_environment(&doc, &HashMap::new(), None, env);
        let target = at(&doc, &styles, "target");
        assert!(!matches!(target.inset[0], Length::Auto), "accepted viewport syntax remains represented");
        assert!(!matches!(target.inset[1], Length::Auto));
        assert!(!matches!(target.inset[2], Length::Auto));
        assert_eq!(resolved(target, 200.0), [None, None, None, Some(4.0)]);
    }
}

#[test]
fn inset_values_viewport_changes_recompute_parent_and_inherited_child_values_with_cached_sources() {
    let doc = document(
        "html{font-size:10px}#parent{font-size:20px;inset:calc(10vw + 1em) 5vh auto calc(-5vw + 10%)}#child{font-size:70px;inset:inherit}",
        "<div id=parent><div id=child></div></div>",
    );
    let external = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, Some(environment(800.0, 600.0)), None);
    assert_eq!(resolved(at(&doc, &first.styles, "child"), 200.0), [Some(100.0), Some(30.0), None, Some(-20.0)]);
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, Some(environment(1200.0, 1000.0)), first.stylesheet_cache.as_ref());
    let fresh = compute_with_environment(&doc, &external, None, Some(environment(1200.0, 1000.0)));
    let child = at(&doc, &next.styles, "child");
    assert_eq!(resolved(child, 200.0), [Some(140.0), Some(50.0), None, Some(-40.0)]);
    assert_eq!(child.inset, at(&doc, &fresh, "child").inset);
}

#[test]
fn inset_values_all_five_properties_use_real_custom_values_and_keyword_fallbacks() {
    for (property, side) in PROPERTIES {
        for (value, inherited) in [("var(--offset)", false), ("var(--missing,inherit)", true)] {
            let doc = document(
                &format!("#parent{{inset:1px 2px 3px 4px;--offset:-7px}}#child{{inset:91px;{property}:{value}}}"),
                "<div id=parent><div id=child></div></div>",
            );
            let styles = compute(&doc, &HashMap::new());
            let source = if inherited { pixels([1.0, 2.0, 3.0, 4.0]) } else { pixels([-7.0; 4]) };
            let mut expected = pixels([91.0; 4]);
            if let Some(side) = side { expected[side] = source[side]; } else { expected = source; }
            assert_eq!(at(&doc, &styles, "child").inset, expected, "{property}:{value}");
        }
    }
}

#[test]
fn inset_values_custom_property_scope_and_nested_fallbacks_preserve_grouped_tokens() {
    let doc = document(
        "html{font-size:10px}#parent{--edge:calc(1rem + 10%);--alias:var(--edge);--Case:3px;--case:9px}#child{--edge:99px;inset:var(--missing,var(--alias)) var(--Case) var(--case) auto}#empty{inset:17px;inset:var(--missing,)}",
        "<div id=parent><div id=child></div><div id=empty></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(resolved(at(&doc, &styles, "child"), 200.0), [Some(30.0), Some(3.0), Some(9.0), None]);
    assert_eq!(at(&doc, &styles, "empty").inset, [Length::Auto; 4]);
}

#[test]
fn inset_values_missing_cyclic_and_invalid_variables_reset_winners_without_revival() {
    for (property, side) in PROPERTIES {
        for value in ["var(--missing)", "var(--cycle)", "var(--bad)"] {
            let doc = document(
                &format!("#target{{--cycle:var(--cycle);--bad:solid;inset:1px 2px 3px 4px;{property}:{value}}}"),
                "<div id=target></div>",
            );
            let styles = compute(&doc, &HashMap::new());
            let mut expected = pixels([1.0, 2.0, 3.0, 4.0]);
            if let Some(side) = side { expected[side] = Length::Auto; } else { expected = [Length::Auto; 4]; }
            assert_eq!(at(&doc, &styles, "target").inset, expected, "{property}:{value}");
        }
    }
}

#[test]
fn inset_values_substitution_cannot_join_number_unit_or_function_tokens() {
    for value in ["var(--number)px", "var(--number)%", "var(--number)vw", "calc(var(--number)px + 1px)", "var(--function)(2px)"] {
        let doc = document(
            &format!("#target{{--number:2;--function:calc;inset:1px 2px 3px 4px;left:{value}}}"),
            "<div id=target></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "target").inset, [Length::Px(1.0), Length::Px(2.0), Length::Px(3.0), Length::Auto], "{value}");
    }
}

#[test]
fn inset_values_invalid_calculations_preserve_earlier_literal_offsets_and_later_rules() {
    for value in [
        "calc(0 + 2px)", "calc(2px * 3px)", "calc(20px / 0)",
        "calc(1px+2px)", "calc(1px +2px)", "calc(2 * 3 px)",
        "calc(1e999 * 2px)", "calc(2px + auto)", "calc(2px + 1ch)",
        "calc(2px))", "calc()", "calc(2px / )",
    ] {
        let doc = document(
            &format!("#target{{inset:7px;top:{value}}}#after{{inset:-3px}}"),
            "<div id=target></div><div id=after></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "target").inset, pixels([7.0; 4]), "{value}");
        assert_eq!(at(&doc, &styles, "after").inset, pixels([-3.0; 4]), "{value}");
    }
}

#[test]
fn inset_values_invalid_shorthand_components_never_publish_partial_sides() {
    for value in [
        "1px 2px 3px 4px 5px", "1px broken", "inherit 1px", "1px initial",
        "auto unset", "1px calc(2px * 3px)", "1px 2 px", "1px calc(2px + 1ch)",
    ] {
        let doc = document(
            &format!("#target{{inset:11px 12px 13px 14px;inset:{value};left:17px}}"),
            "<div id=target></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "target").inset, pixels([11.0, 12.0, 13.0, 17.0]), "{value}");
    }
}

#[test]
fn inset_values_invalid_literal_and_substituted_components_have_distinct_cascade_outcomes() {
    for value in ["inherit 1px", "2px broken", "calc(2px * 3px)", "2 px"] {
        let doc = document(
            &format!("#literal{{inset:9px;inset:{value};left:3px}}#variable{{inset:9px;inset:var(--missing,{value});left:3px}}"),
            "<div id=literal></div><div id=variable></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "literal").inset, pixels([9.0, 9.0, 9.0, 3.0]), "{value}");
        assert_eq!(at(&doc, &styles, "variable").inset, [Length::Auto, Length::Auto, Length::Auto, Length::Px(3.0)], "{value}");
    }
}

#[test]
fn inset_values_shorthand_longhand_specificity_source_order_and_importance_are_preserved() {
    let doc = document(
        "#parent{inset:1px 2px 3px 4px;--edge:6px}#a{left:99px;inset:var(--missing,inherit);top:11px}#b{inset:inherit;left:calc(5px + var(--edge))}#c{inset:var(--missing,inherit)!important}.low{inset:80px}#d{inset:inherit}",
        "<div id=parent><div id=a></div><div id=b></div><div id=c class=low style='inset:91px;right:17px!important'></div><div id=d class=low></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for (name, expected) in [
        ("a", [11.0, 2.0, 3.0, 4.0]), ("b", [1.0, 2.0, 3.0, 11.0]),
        ("c", [1.0, 17.0, 3.0, 4.0]), ("d", [1.0, 2.0, 3.0, 4.0]),
    ] {
        assert_eq!(at(&doc, &styles, name).inset, pixels(expected), "{name}");
    }
}

#[test]
fn inset_values_external_source_replacement_recomputes_inherited_offsets_without_stale_values() {
    let doc = crate::html::parse(
        "<!doctype html><html><head><link id=sheet rel=stylesheet></head><body><div id=parent><div id=child style='font-size:70px;inset:inherit'></div></div></body></html>",
    );
    let sheet = id(&doc, "sheet");
    let mut external = HashMap::from([(sheet, "#parent{font-size:10px;inset:2em 3px auto 4%}".into())]);
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(at(&doc, &first.styles, "child").inset, [Length::Px(20.0), Length::Px(3.0), Length::Auto, Length::Percent(4.0)]);
    external.insert(sheet, "#parent{font-size:12px;inset:3em 5px auto 6%}".into());
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    assert_eq!(at(&doc, &next.styles, "child").inset, [Length::Px(36.0), Length::Px(5.0), Length::Auto, Length::Percent(6.0)]);
    assert_eq!(at(&doc, &next.styles, "child").inset, at(&doc, &fresh, "child").inset);
    assert!(!first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn inset_values_parent_inline_changes_reuse_rules_but_recompute_font_terms() {
    let mut doc = document(
        "html{font-size:10px}#child{font-size:70px;inset:var(--missing,inherit)}",
        "<div id=parent style='font-size:10px;inset:2em calc(1rem + 10%) auto -1em'><div id=child></div></div>",
    );
    let external = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(resolved(at(&doc, &first.styles, "child"), 200.0), [Some(20.0), Some(30.0), None, Some(-10.0)]);
    let parent = id(&doc, "parent");
    set_style(&mut doc, parent, "font-size:12px;inset:5em calc(2rem + 20%) auto -4em");
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    assert_eq!(resolved(at(&doc, &next.styles, "child"), 200.0), [Some(60.0), Some(60.0), None, Some(-48.0)]);
    assert_eq!(at(&doc, &next.styles, "child").inset, at(&doc, &fresh, "child").inset);
    assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn inset_values_work_and_copy_refusals_preserve_source_and_allow_fresh_recovery() {
    let doc = document(
        "html{font-size:10px}#parent{font-size:20px;inset:calc(1em + 10%) auto -2em 3rem}#child{font-size:70px;inset:var(--missing,inherit)}",
        "<div id=parent><div id=child></div></div>",
    );
    let external = HashMap::new();
    let before = format!("{:?}", doc.nodes);
    let prior = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    for (mut work, mut copies) in [(0, 1_000_000), (1_000_000, 0)] {
        assert!(compute_with_environment_and_properties_budgeted_cached(
            &doc, &external, None, None, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref(),
        ).is_err());
        assert_eq!(format!("{:?}", doc.nodes), before);
    }
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let recovered = compute_with_environment_and_properties_budgeted_cached(
        &doc, &external, None, None, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref(),
    ).unwrap();
    assert_eq!(resolved(at(&doc, &recovered.styles, "child"), 200.0), [Some(40.0), None, Some(-40.0), Some(30.0)]);
    assert_eq!(format!("{:?}", doc.nodes), before);
    assert!(work < 1_000_000 && copies < 1_000_000);
    assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn inset_values_inline_names_admit_grouped_math_viewports_and_keywords_through_the_same_grammar() {
    for value in ["calc(2px + 10%) auto -1vw 2vh", "inherit", "initial", "unset", "var(--edge)"] {
        assert_names(&format!("inset:{value}"), &["top", "right", "bottom", "left"]);
    }
    for value in ["calc(-1em + 2px)", "-2vw", "3vh", "inherit", "initial", "unset", "var(--edge)"] {
        for (property, _) in PROPERTIES.into_iter().skip(1) {
            assert_names(&format!("{property}:{value}"), &[property]);
        }
    }
    assert_names("inset:calc(2px + 10%) auto;top:var(--edge)", &["right", "bottom", "left", "top"]);
}

#[test]
fn inset_values_inline_names_reject_bad_math_or_dimensions_without_erasing_prior_valid_names() {
    for value in ["calc(2px * 3px)", "calc(20px / 0)", "2 px", "1.px", "1.e2px", "NaNpx"] {
        assert_names(&format!("top:{value}"), &[]);
        assert_names(&format!("inset:1px {value}"), &[]);
        assert_names(&format!("top:7px;left:3px;top:{value}"), &["top", "left"]);
    }
    assert_names("inset:1px 2px 3px 4px 5px", &[]);
    assert_names("inset:inherit 1px", &[]);
}

#[test]
fn inset_values_inline_text_declarations_share_valid_and_invalid_inset_admission() {
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let entries = inline_text_declarations(
        "inset:calc(50% - 4px) auto;left:var(--x,-3px);top:2 px;right:calc(1px * 2px);bottom:1.px",
        &INLINE_NAMES, &mut CssBudget::new(&mut work, &mut copies),
    ).unwrap();
    assert_eq!(entries, vec![
        ("inset".to_owned(), "calc(50% - 4px) auto".to_owned(), false),
        ("left".to_owned(), "var(--x,-3px)".to_owned(), false),
    ]);
    assert!(work < 1_000_000 && copies < 1_000_000);
}

#[test]
fn inset_values_ascii_number_tokens_case_and_unit_boundaries_are_consistent() {
    let doc = document(
        "html{font-size:10px}#target{font-size:20px;inset:CaLc(-2PX + 1e1px) -.5REM +.25EM -2E1%}",
        "<div id=target></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(resolved(at(&doc, &styles, "target"), 200.0), [Some(8.0), Some(-5.0), Some(5.0), Some(-40.0)]);
    for value in ["1.px", "1.e2px", "1e+px", "+px", "NaNpx", "infpx", "1e999px", "١px", "1\u{a0}px", "2 px", "0 px"] {
        let doc = document(&format!("#target{{inset:7px;left:{value}}}"), "<div id=target></div>");
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "target").inset, pixels([7.0; 4]), "{value}");
    }
}
