use super::*;

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!(
        "<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"
    ))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn at(doc: &Document, name: &str) -> order::Value {
    compute(doc, &HashMap::new())[id(doc, name)].order.clone()
}

fn set_style(doc: &mut Document, node: usize, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { panic!("element required") };
    if let Some(attr) = attrs.iter_mut().find(|attr| attr.name.html_is("style")) {
        attr.value = value.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified("style".into(), value.into()));
    }
}

#[test]
fn order_literals_normalize_sign_and_leading_zeroes() {
    for (source, expected) in [("0", "0"), ("-0", "0"), ("+0000", "0"),
        ("-00042", "-42"), ("+007", "7"), (" 17 ", "17")] {
        let doc = document(&format!("#target{{order:{source}}}"), "<div id=target></div>");
        assert_eq!(at(&doc, "target").text(), expected, "{source}");
        assert_eq!(order::specified(source).as_deref(), Some(expected));
        assert_eq!(at(&doc, "target").is_zero(), expected == "0");
    }
}

#[test]
fn order_exact_integer_comparison_does_not_round_or_clamp() {
    let positive = format!("1{}", "0".repeat(120));
    let before = "9".repeat(120);
    let expected = [format!("-{positive}"), format!("-{before}"), "-1".into(),
        "0".into(), "1".into(), before, positive];
    let mut values = Vec::new();
    for source in expected.iter().rev() {
        let doc = document(&format!("#target{{order:{source}}}"), "<div id=target></div>");
        values.push(at(&doc, "target"));
    }
    values.sort();
    assert_eq!(values.iter().map(|value| value.text()).collect::<Vec<_>>(),
        expected.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn order_defaults_to_zero_and_does_not_inherit() {
    let doc = document("#parent{order:-7}", "<div id=parent><div id=target></div></div>");
    assert_eq!(at(&doc, "parent").text(), "-7");
    assert_eq!(at(&doc, "target").text(), "0");
    assert_eq!(order::Value::default().text(), "0");
}

#[test]
fn order_css_wide_keywords_follow_noninherited_defaulting() {
    for (source, expected) in [("inherit", "-7"), ("INHERIT", "-7"), ("initial", "0"),
        ("unset", "0"), ("revert", "0"), ("revert-layer", "0")] {
        let doc = document(&format!("#parent{{order:-7}}#target{{order:3;order:{source}}}"),
            "<div id=parent><div id=target></div></div>");
        assert_eq!(at(&doc, "target").text(), expected, "{source}");
    }
    let doc = document("html{order:inherit}", "<div></div>");
    let html = doc.nodes.iter().position(|node| matches!(&node.kind,
        NodeKind::Element { tag, .. } if tag == "html")).unwrap();
    assert!(compute(&doc, &HashMap::new())[html].order.is_zero());
}

#[test]
fn order_inline_revert_layer_restores_the_actual_stylesheet_winner() {
    for (sheet, inline, expected) in [
        ("#target{order:-3}", "order:8;order:revert-layer", "-3"),
        ("#target{order:-3!important}", "order:8!important;order:revert-layer!important", "-3"),
        ("#target{order:-3!important}", "order:8;order:revert-layer", "-3"),
        ("", "order:8;order:revert-layer", "0"),
    ] {
        let doc = document(sheet, &format!("<div id=target style='{inline}'></div>"));
        assert_eq!(at(&doc, "target").text(), expected);
    }
}

#[test]
fn order_invalid_literals_preserve_prior_winners_and_recover() {
    for source in ["auto", "1.0", "1e2", "2px", "1 2", "- 1", "--1", "+", "NaN",
        "infinity", "'7'", "calc(1 + 1)", "var(not-custom)"] {
        let doc = document(&format!("#target{{order:-4;order:{source}}}#next{{order:8}}"),
            "<div id=target></div><div id=next></div>");
        assert_eq!(at(&doc, "target").text(), "-4", "{source}");
        assert_eq!(at(&doc, "next").text(), "8", "{source}");
        assert!(!order::valid(source), "{source}");
    }
}

#[test]
fn order_invalid_important_values_do_not_mask_valid_lower_priority_values() {
    let doc = document("#target{order:7;order:auto!important}#target{order:-2}",
        "<div id=target></div>");
    assert_eq!(at(&doc, "target").text(), "-2");
    let doc = document("#target{order:7!important}#target{order:-2}",
        "<div id=target style='order:8'></div>");
    assert_eq!(at(&doc, "target").text(), "7");
}

#[test]
fn order_variables_resolve_nested_fallbacks_and_preserve_name_case() {
    for (source, expected) in [
        ("--Rank:-8;--rank:3;order:var(--Rank)", "-8"),
        ("--Rank:-8;--rank:3;order:var(--rank)", "3"),
        ("--rank:+0006;order:VAR(--rank)", "6"),
        ("order:var(--missing,var(--other,-3))", "-3"),
        ("--rank:-9;--next:var(--rank);order:var(--next)", "-9"),
    ] {
        let doc = document(&format!("#target{{order:4;{source}}}"), "<div id=target></div>");
        assert_eq!(at(&doc, "target").text(), expected, "{source}");
    }
}

#[test]
fn order_invalid_substituted_winners_reset_to_zero() {
    for source in ["order:var(--missing)", "order:var(--missing,)", "--rank:auto;order:var(--rank)",
        "--rank:1.5;order:var(--rank)", "--rank:var(--rank);order:var(--rank)",
        "--a:var(--b);--b:var(--a);order:var(--a)", "--rank:2;order:var(--rank) 3"] {
        let doc = document(&format!("#parent{{order:9}}#target{{order:-4;{source}}}"),
            "<div id=parent><div id=target></div></div>");
        assert_eq!(at(&doc, "target").text(), "0", "{source}");
    }
}

#[test]
fn order_variable_fallback_keywords_and_inline_rollback_keep_their_cascade_scope() {
    for (keyword, expected) in [("inherit", "9"), ("initial", "0"), ("unset", "0"),
        ("revert", "0"), ("revert-layer", "-4")] {
        let doc = document("#parent{order:9}#target{order:-4}",
            &format!("<div id=parent><div id=target style='order:var(--missing,{keyword})'></div></div>"));
        assert_eq!(at(&doc, "target").text(), expected, "{keyword}");
    }
}

#[test]
fn order_computed_presentation_serializes_the_same_exact_value() {
    let huge = format!("-8{}", "1".repeat(80));
    let doc = document(&format!("#target{{order:{huge};z-index:6;display:flex;color:#123456}}"),
        "<div id=target></div>");
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    let target = id(&doc, "target");
    let presentation = result.computed_properties.read_presentation(target).unwrap().unwrap();
    assert_eq!(presentation.order.text(), huge);
    assert_eq!(presentation.order, result.styles[target].order);
    assert_eq!(presentation.z_index.as_ref().unwrap().text(), "6");
    assert_eq!(presentation.display, "flex");
    assert_eq!(presentation.color, 0x123456);
}

#[test]
fn order_inline_names_filter_bad_values_without_moving_valid_occurrences() {
    let supported = ["color", "order"];
    for source in ["1.5", "auto", "var(not-custom)", "calc(1 + 1)"] {
        let input = format!("order:-2;color:red;order:{source}");
        assert_eq!(inline_declaration_names(&input, &supported, None).unwrap(), ["order", "color"]);
    }
    for source in ["-7", "+008", "var(--Rank)", "inherit", "initial", "unset", "revert", "revert-layer"] {
        assert_eq!(inline_declaration_names(&format!("order:{source}"), &supported, None).unwrap(), ["order"]);
    }
}

#[test]
fn order_inline_text_uses_the_same_integer_and_variable_grammar() {
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    let source = "order:+0004;order:1.5;order:var(--Rank,-2)!important;order:auto";
    let declarations = inline_text_declarations(source, &["order"], &mut CssBudget::new(&mut work, &mut copies)).unwrap();
    assert_eq!(declarations, [("order".into(), "+0004".into(), false),
        ("order".into(), "var(--Rank,-2)".into(), true)]);
    assert_eq!(order::specified("VAR(--Rank, -2)").as_deref(), Some("VAR(--Rank, -2)"));
    assert_eq!(order::specified("InHeRiT").as_deref(), Some("inherit"));
}

#[test]
fn order_cached_rules_recompute_after_inline_and_parent_variable_changes() {
    let mut doc = document("#target{order:var(--Rank)}", "<div id=parent style='--Rank:2'><div id=target></div></div>");
    let target = id(&doc, "target");
    let first = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
    let parent = id(&doc, "parent");
    set_style(&mut doc, parent, "--Rank:-3");
    let next = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, first.stylesheet_cache.as_ref());
    assert_eq!(first.styles[target].order.text(), "2");
    assert_eq!(next.styles[target].order.text(), "-3");
    assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
    assert_eq!(first.computed_properties.read_presentation(target).unwrap().unwrap().order.text(), "2");
    set_style(&mut doc, target, "order:8");
    let inline = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, next.stylesheet_cache.as_ref());
    assert_eq!(inline.styles[target].order.text(), "8");
}

#[test]
fn order_budgeted_routes_refuse_short_allowances_and_recover_without_mutation() {
    let doc = document("#target{--Rank:-8;order:var(--Rank)}", "<div id=target></div>");
    let before = format!("{:?}", doc.nodes);
    for transform in [false, true] {
        for (mut work, mut copies) in [(0, usize::MAX), (usize::MAX, 0)] {
            let mut budget = CssBudget::new(&mut work, &mut copies);
            let refused = if transform {
                compute_with_environment_and_properties_transform_budgeted(&doc, &HashMap::new(), None, None, &mut budget)
            } else {
                compute_with_environment_and_properties_budgeted(&doc, &HashMap::new(), None, None, &mut budget)
            };
            assert!(refused.is_err());
        }
        let (mut work, mut copies) = (usize::MAX, usize::MAX);
        let mut budget = CssBudget::new(&mut work, &mut copies);
        let result = if transform {
            compute_with_environment_and_properties_transform_budgeted(&doc, &HashMap::new(), None, None, &mut budget)
        } else {
            compute_with_environment_and_properties_budgeted(&doc, &HashMap::new(), None, None, &mut budget)
        }.unwrap();
        assert_eq!(result.styles[id(&doc, "target")].order.text(), "-8");
        assert_eq!(format!("{:?}", doc.nodes), before);
    }
}

#[test]
fn order_resource_refusal_cannot_publish_complete_computed_values() {
    let doc = crate::html::parse("<!doctype html><link id=sheet rel=stylesheet href=order.css><div id=target></div>");
    let repeated = format!("order:{};", "1".repeat(250)).repeat(520);
    let mut sheets = HashMap::from([(id(&doc, "sheet"), format!("#target{{--control:yes;{repeated}order:var(--missing)}}"))]);
    let target = id(&doc, "target");
    let failed = compute_with_environment_and_properties(&doc, &sheets, None, None);
    assert!(failed.computed_properties.read_presentation(target).is_err());
    assert_eq!(failed.computed_properties.read(target, "--control"), Err(ComputedStyleUnavailable));
    for transform in [false, true] {
        let (mut work, mut copies) = (usize::MAX, usize::MAX);
        let mut budget = CssBudget::new(&mut work, &mut copies);
        let result = if transform {
            compute_with_environment_and_properties_transform_budgeted(&doc, &sheets, None, None, &mut budget)
        } else {
            compute_with_environment_and_properties_budgeted(&doc, &sheets, None, None, &mut budget)
        };
        assert!(result.is_err(), "route {transform}");
    }
    sheets.insert(id(&doc, "sheet"), "#target{--control:yes;order:-6}".into());
    let recovered = compute_with_environment_and_properties(&doc, &sheets, None, None);
    assert_eq!(recovered.computed_properties.read_presentation(target).unwrap().unwrap().order.text(), "-6");
    assert_eq!(recovered.computed_properties.read(target, "--control"), Ok(Some("yes")));
}

#[test]
fn order_comments_preserve_integer_boundaries_and_ignore_comment_contents() {
    for (source, expected) in [("/*x*/+002", "2"), ("-003/*tail*/", "-3"),
        ("/*var(--ignored)*/7", "7"), ("/*;{}!@'*/-4", "-4"), ("/**/InHerIt/**/", "inherit")] {
        assert!(order::valid(source), "{source}");
        assert_eq!(order::specified(source).as_deref(), Some(expected), "{source}");
        if expected != "inherit" {
            let doc = document(&format!("#target{{order:{source}}}"), "<div id=target></div>");
            assert_eq!(at(&doc, "target").text(), expected);
        }
    }
    for source in ["1/**/2", "+/**/2", "in/**/herit", "var/**/(--Rank)", "/*var(--Rank)*/auto"] {
        assert!(!order::valid(source), "{source}");
        assert!(order::specified(source).is_none(), "{source}");
    }
}

#[test]
fn order_inline_admission_charges_variable_parser_work_and_storage() {
    let source = "order:var(--Rank,-3)";
    for text in [false, true] {
        for (mut work, mut copies) in [(source.len() * 3, usize::MAX), (usize::MAX, source.len() * 3)] {
            let mut meter = CssBudget::new(&mut work, &mut copies);
            let refused = if text { inline_text_declarations(source, &["order"], &mut meter).is_err() }
                else { inline_declaration_names(source, &["order"], Some(&mut meter)).is_err() };
            assert!(refused, "whole text {text}");
        }
    }
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    let mut meter = CssBudget::new(&mut work, &mut copies);
    assert_eq!(inline_declaration_names(source, &["order"], Some(&mut meter)).unwrap(), ["order"]);
    assert_eq!(inline_text_declarations(source, &["order"], &mut meter).unwrap(),
        [("order".into(), "var(--Rank,-3)".into(), false)]);
}

#[test]
fn order_refused_custom_maps_cannot_supply_an_initial_computed_value() {
    let doc = crate::html::parse("<!doctype html><link id=sheet rel=stylesheet href=order.css><div id=target></div>");
    let target = id(&doc, "target");
    let declarations: String = (0..257).map(|index| format!("--v{index}:8;")).collect();
    let mut sheets = HashMap::from([(id(&doc, "sheet"), format!("#target{{{declarations}order:7;order:var(--v0)}}"))]);
    let result = compute_with_environment_and_properties(&doc, &sheets, None, None);
    match result.computed_properties.read_presentation(target) {
        Ok(Some(presentation)) => assert_eq!(presentation.order.text(), "8"),
        Err(ComputedStyleUnavailable) => assert_eq!(result.computed_properties.read(target, "--v0"), Err(ComputedStyleUnavailable)),
        Ok(None) => panic!("connected target has no computed presentation"),
    }
    sheets.insert(id(&doc, "sheet"), "#target{--v0:8;order:var(--v0)}".into());
    let recovered = compute_with_environment_and_properties(&doc, &sheets, None, None);
    assert_eq!(recovered.computed_properties.read_presentation(target).unwrap().unwrap().order.text(), "8");
}

#[test]
fn order_refused_expansion_cannot_supply_an_initial_computed_value() {
    let huge = "8".repeat(2048);
    let doc = document(&format!("#target{{--Rank:{huge};order:7;order:var(--Rank)}}"), "<div id=target></div>");
    let target = id(&doc, "target");
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    match result.computed_properties.read_presentation(target) {
        Ok(Some(presentation)) => assert_eq!(presentation.order.text(), huge),
        Err(ComputedStyleUnavailable) => {}
        Ok(None) => panic!("connected target has no computed presentation"),
    }
    let recovered = document("#target{--Rank:-8;order:var(--Rank)}", "<div id=target></div>");
    assert_eq!(at(&recovered, "target").text(), "-8");
}
