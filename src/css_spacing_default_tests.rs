use super::*;

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!(
        "<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"
    ))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style {
    &styles[id(doc, name)]
}

fn pixels(values: [f32; 4]) -> [Length; 4] {
    values.map(Length::Px)
}

fn set_style(doc: &mut Document, node: usize, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else {
        panic!("element required");
    };
    if let Some(attr) = attrs.iter_mut().find(|attr| attr.name.html_is("style")) {
        attr.value = value.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified("style".into(), value.into()));
    }
}

#[test]
fn spacing_defaults_literal_shorthands_inherit_distinct_parent_sides() {
    let doc = document(
        "#parent{margin:2px 4px 6px 8px;padding:3px 5px 7px 9px}#child{margin:91px;padding:93px;margin:InHeRiT;padding: inherit }",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    let child = at(&doc, &styles, "child");
    assert_eq!(child.margin, pixels([2.0, 4.0, 6.0, 8.0]));
    assert_eq!(child.padding, pixels([3.0, 5.0, 7.0, 9.0]));
}

#[test]
fn spacing_defaults_each_literal_longhand_inherits_only_its_own_side() {
    for property in ["margin", "padding"] {
        for (side, index) in [("top", 0), ("right", 1), ("bottom", 2), ("left", 3)] {
            let sheet = format!(
                "#parent{{margin:2px 4px 6px 8px;padding:3px 5px 7px 9px}}#child{{margin:11px 12px 13px 14px;padding:21px 22px 23px 24px;{property}-{side}:inherit}}"
            );
            let doc = document(&sheet, "<div id=parent><div id=child></div></div>");
            let styles = compute(&doc, &HashMap::new());
            let child = at(&doc, &styles, "child");
            let mut margin = pixels([11.0, 12.0, 13.0, 14.0]);
            let mut padding = pixels([21.0, 22.0, 23.0, 24.0]);
            if property == "margin" {
                margin[index] = pixels([2.0, 4.0, 6.0, 8.0])[index];
            } else {
                padding[index] = pixels([3.0, 5.0, 7.0, 9.0])[index];
            }
            assert_eq!(child.margin, margin, "{property}-{side}");
            assert_eq!(child.padding, padding, "{property}-{side}");
        }
    }
}

#[test]
fn spacing_defaults_initial_and_unset_reset_entire_shorthands() {
    for keyword in ["initial", "unset"] {
        let doc = document(
            &format!("#parent{{margin:31px;padding:33px}}#child{{margin:2px 4px 6px 8px;padding:3px 5px 7px 9px;margin:{keyword};padding:{keyword}}}"),
            "<div id=parent><div id=child></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        let child = at(&doc, &styles, "child");
        assert_eq!(child.margin, pixels([0.0; 4]), "{keyword}");
        assert_eq!(child.padding, pixels([0.0; 4]), "{keyword}");
    }
}

#[test]
fn spacing_defaults_initial_and_unset_reset_each_longhand_without_other_sides() {
    for keyword in ["initial", "unset"] {
        for property in ["margin", "padding"] {
            for (side, index) in [("top", 0), ("right", 1), ("bottom", 2), ("left", 3)] {
                let doc = document(
                    &format!("#parent{{margin:31px;padding:33px}}#child{{margin:2px 4px 6px 8px;padding:3px 5px 7px 9px;{property}-{side}:{keyword}}}"),
                    "<div id=parent><div id=child></div></div>",
                );
                let styles = compute(&doc, &HashMap::new());
                let child = at(&doc, &styles, "child");
                let mut margin = pixels([2.0, 4.0, 6.0, 8.0]);
                let mut padding = pixels([3.0, 5.0, 7.0, 9.0]);
                if property == "margin" { margin[index] = Length::Px(0.0); }
                else { padding[index] = Length::Px(0.0); }
                assert_eq!(child.margin, margin, "{property}-{side}:{keyword}");
                assert_eq!(child.padding, padding, "{property}-{side}:{keyword}");
            }
        }
    }
}

#[test]
fn spacing_defaults_root_inherit_and_body_resets_use_initial_spacing() {
    for keyword in ["initial", "unset"] {
        let doc = crate::html::parse(&format!(
            "<!doctype html><html id=root style='margin:13px;padding:17px;margin:inherit;padding:inherit'><head></head><body id=body style='margin:{keyword};padding:7px;padding:{keyword}'></body></html>"
        ));
        let styles = compute(&doc, &HashMap::new());
        for name in ["root", "body"] {
            let style = at(&doc, &styles, name);
            assert_eq!(style.margin, pixels([0.0; 4]), "{name}:{keyword}");
            assert_eq!(style.padding, pixels([0.0; 4]), "{name}:{keyword}");
        }
    }
}

#[test]
fn spacing_defaults_ordinary_descendants_do_not_implicitly_inherit_spacing() {
    let doc = document(
        "#parent{margin:17px;padding:19px}#unset{margin:unset;padding:unset}",
        "<div id=parent><div id=ordinary></div><div id=unset></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(at(&doc, &styles, "parent").margin, pixels([17.0; 4]));
    assert_eq!(at(&doc, &styles, "parent").padding, pixels([19.0; 4]));
    for name in ["ordinary", "unset"] {
        assert_eq!(at(&doc, &styles, name).margin, pixels([0.0; 4]));
        assert_eq!(at(&doc, &styles, name).padding, pixels([0.0; 4]));
    }
}

#[test]
fn spacing_defaults_literal_inheritance_uses_final_parent_font_metrics() {
    let doc = document(
        "html{font-size:10px}#parent{margin:2em 3rem -1em 5%;padding:1em 2rem 3em 6%;font-size:20px}#child{margin:inherit;padding:inherit;font-size:40px}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    let child = at(&doc, &styles, "child");
    assert_eq!(child.font_size, 40.0);
    assert_eq!(child.margin, [Length::Px(40.0), Length::Px(30.0), Length::Px(-20.0), Length::Percent(5.0)]);
    assert_eq!(child.padding, [Length::Px(20.0), Length::Px(20.0), Length::Px(60.0), Length::Percent(6.0)]);
}

#[test]
fn spacing_defaults_substituted_inheritance_uses_parent_computed_lengths() {
    let doc = document(
        "html{font-size:12px}#parent{margin:2em 3rem auto -1em;padding:1em 2rem 3em 4%;font-size:20px}#child{font-size:60px;margin:var(--missing,InHeRiT);padding:var(--missing,inherit)}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    let child = at(&doc, &styles, "child");
    assert_eq!(child.margin, [Length::Px(40.0), Length::Px(36.0), Length::Auto, Length::Px(-20.0)]);
    assert_eq!(child.padding, [Length::Px(20.0), Length::Px(24.0), Length::Px(60.0), Length::Percent(4.0)]);
}

#[test]
fn spacing_defaults_mixed_calculations_keep_percentages_but_resolve_parent_fonts() {
    let doc = document(
        "html{font-size:10px}#parent{font-size:20px;margin:calc(1em + 2rem + 10%) auto -1em 25%;padding:calc(2em + 1rem + 5%) 2em}#child{font-size:70px;margin:inherit;padding:var(--missing,inherit)}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    let child = at(&doc, &styles, "child");
    assert!(child.margin[0].has_percentage());
    assert!(child.padding[0].has_percentage());
    for font in [30.0, 80.0] {
        assert_eq!(child.margin[0].resolve(200.0, font, 50.0), Some(60.0));
        assert_eq!(child.margin[0].resolve(400.0, font, 50.0), Some(80.0));
        assert_eq!(child.padding[0].resolve(200.0, font, 50.0), Some(60.0));
        assert_eq!(child.padding[0].resolve(400.0, font, 50.0), Some(70.0));
    }
    assert_eq!(child.margin[1], Length::Auto);
    assert_eq!(child.margin[2], Length::Px(-20.0));
    assert_eq!(child.margin[3], Length::Percent(25.0));
    assert_eq!(child.padding[1], Length::Px(40.0));
    assert_eq!(child.padding[3], Length::Px(40.0));
}

#[test]
fn spacing_defaults_multigeneration_inheritance_does_not_rescale_absolute_lengths() {
    let doc = document(
        "html{font-size:15px}#parent{font-size:20px;margin:2em 3rem;padding:1em 2rem}#child{font-size:70px;margin:inherit;padding:var(--missing,inherit)}#grandchild{font-size:100px;margin:var(--missing,inherit);padding:inherit}",
        "<div id=parent><div id=child><div id=grandchild></div></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for name in ["child", "grandchild"] {
        assert_eq!(at(&doc, &styles, name).margin, pixels([40.0, 45.0, 40.0, 45.0]), "{name}");
        assert_eq!(at(&doc, &styles, name).padding, pixels([20.0, 30.0, 20.0, 30.0]), "{name}");
    }
}

#[test]
fn spacing_defaults_shorthands_and_longhands_keep_cascade_priority_and_order() {
    let doc = document(
        "#parent{margin:1px 2px 3px 4px;padding:5px 6px 7px 8px}#a{margin-left:99px;margin:inherit;margin-top:11px;padding-left:99px;padding:inherit;padding-top:12px}#b{margin:inherit;margin-left:99px;padding:inherit;padding-left:98px}#c{margin:inherit!important;padding:inherit!important}#d{margin:inherit;padding:inherit}.low{margin:80px;padding:80px}",
        "<div id=parent><div id=a></div><div id=b></div><div id=c class=low style='margin-top:90px;padding-top:90px;margin-right:17px!important;padding-right:18px!important'></div><div id=d class=low></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for (name, margin, padding) in [
        ("a", [11.0, 2.0, 3.0, 4.0], [12.0, 6.0, 7.0, 8.0]),
        ("b", [1.0, 2.0, 3.0, 99.0], [5.0, 6.0, 7.0, 98.0]),
        ("c", [1.0, 17.0, 3.0, 4.0], [5.0, 18.0, 7.0, 8.0]),
        ("d", [1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]),
    ] {
        assert_eq!(at(&doc, &styles, name).margin, pixels(margin), "{name}");
        assert_eq!(at(&doc, &styles, name).padding, pixels(padding), "{name}");
    }
}

#[test]
fn spacing_defaults_mixed_keywords_distinguish_invalid_literals_from_substitution() {
    for value in ["inherit 1px", "1px initial", "unset auto", "inherit inherit"] {
        let doc = document(
            &format!("#parent{{margin:21px;padding:23px}}#literal{{margin:9px;padding:7px;margin:{value};padding:{value};margin-left:3px;padding-left:4px}}#substituted{{margin:9px;padding:7px;margin:var(--missing,{value});padding:var(--missing,{value});margin-left:3px;padding-left:4px}}"),
            "<div id=parent><div id=literal></div><div id=substituted></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(at(&doc, &styles, "literal").margin, pixels([9.0, 9.0, 9.0, 3.0]), "{value}");
        assert_eq!(at(&doc, &styles, "literal").padding, pixels([7.0, 7.0, 7.0, 4.0]), "{value}");
        assert_eq!(at(&doc, &styles, "substituted").margin, pixels([0.0, 0.0, 0.0, 3.0]), "{value}");
        assert_eq!(at(&doc, &styles, "substituted").padding, pixels([0.0, 0.0, 0.0, 4.0]), "{value}");
    }
}

#[test]
fn spacing_defaults_external_source_changes_replace_cached_inherited_values() {
    let doc = crate::html::parse(
        "<!doctype html><html><head><link id=sheet rel=stylesheet></head><body><div id=parent><div id=child style='font-size:40px;margin:inherit;padding:inherit'></div></div></body></html>",
    );
    let sheet = id(&doc, "sheet");
    let mut external = HashMap::from([(sheet, "#parent{font-size:10px;margin:2em;padding:3em}".into())]);
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(at(&doc, &first.styles, "child").margin, pixels([20.0; 4]));
    assert_eq!(at(&doc, &first.styles, "child").padding, pixels([30.0; 4]));
    external.insert(sheet, "#parent{font-size:10px;margin:4em;padding:5em}".into());
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    assert_eq!(at(&doc, &next.styles, "child").margin, pixels([40.0; 4]));
    assert_eq!(at(&doc, &next.styles, "child").padding, pixels([50.0; 4]));
    assert_eq!(at(&doc, &next.styles, "child").margin, at(&doc, &fresh, "child").margin);
    assert_eq!(at(&doc, &next.styles, "child").padding, at(&doc, &fresh, "child").padding);
    assert!(!first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn spacing_defaults_parent_inline_changes_recascade_reused_stylesheet_rules() {
    let mut doc = document(
        "#child{font-size:70px;margin:inherit;padding:var(--missing,inherit)}",
        "<div id=parent style='font-size:10px;margin:2em;padding:3em'><div id=child></div></div>",
    );
    let external = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(at(&doc, &first.styles, "child").margin, pixels([20.0; 4]));
    assert_eq!(at(&doc, &first.styles, "child").padding, pixels([30.0; 4]));
    let parent = id(&doc, "parent");
    set_style(&mut doc, parent, "font-size:12px;margin:5em;padding:4em");
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    assert_eq!(at(&doc, &next.styles, "child").margin, pixels([60.0; 4]));
    assert_eq!(at(&doc, &next.styles, "child").padding, pixels([48.0; 4]));
    assert_eq!(at(&doc, &next.styles, "child").margin, at(&doc, &fresh, "child").margin);
    assert_eq!(at(&doc, &next.styles, "child").padding, at(&doc, &fresh, "child").padding);
    assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn spacing_defaults_explicit_resource_refusals_leave_fresh_recovery_available() {
    let doc = document(
        "#parent{font-size:10px;margin:2em;padding:3em}#child{font-size:40px;margin:inherit;padding:var(--missing,inherit)}",
        "<div id=parent><div id=child></div></div>",
    );
    let external = HashMap::new();
    let prior = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    for (mut work, mut copies) in [(0, 1_000_000), (1_000_000, 0)] {
        assert!(compute_with_environment_and_properties_budgeted_cached(
            &doc, &external, None, None, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref(),
        ).is_err());
    }
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let recovered = compute_with_environment_and_properties_budgeted_cached(
        &doc, &external, None, None, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref(),
    ).unwrap();
    assert_eq!(at(&doc, &recovered.styles, "child").margin, pixels([20.0; 4]));
    assert_eq!(at(&doc, &recovered.styles, "child").padding, pixels([30.0; 4]));
    assert!(work < 1_000_000 && copies < 1_000_000);
    assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn spacing_defaults_leave_other_box_properties_and_invalid_ranges_unchanged() {
    let doc = document(
        "#parent{width:80px;height:50px;border:3px solid red;gap:9px;margin:17px;padding:19px}#child{width:21px;height:22px;border:2px solid blue;gap:3px;margin:inherit;padding:initial;padding-left:5px;padding-left:-7px;margin-right:-6px}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    let child = at(&doc, &styles, "child");
    assert_eq!(child.margin, pixels([17.0, -6.0, 17.0, 17.0]));
    assert_eq!(child.padding, pixels([0.0, 0.0, 0.0, 5.0]));
    assert_eq!(child.width, Length::Px(21.0));
    assert_eq!(child.height, Length::Px(22.0));
    assert_eq!(child.border_width, pixels([2.0; 4]));
    assert_eq!(child.border_solid, [true; 4]);
    assert_eq!(child.border_color, [Some(0x0000ff); 4]);
    assert_eq!(child.gap, [Length::Px(3.0); 2]);
}
