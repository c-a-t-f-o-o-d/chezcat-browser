use super::*;

const PARENT: &str = "border-width:2px 4px 6px 8px;border-style:solid none solid hidden;border-color:rgba(10,20,30,.2) rgba(40,50,60,.4) transparent #789abc";
const CHILD: &str = "border-width:11px 12px 13px 14px;border-style:solid;border-color:rgba(70,80,90,.6)";
const PARENT_RGB: [Option<u32>; 4] = [Some(0x0a141e), Some(0x28323c), Some(0), Some(0x789abc)];
const PARENT_ALPHA: [u8; 4] = [51, 102, 0, 255];
const SIDES: [(&str, usize); 4] = [("top", 0), ("right", 1), ("bottom", 2), ("left", 3)];

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style {
    &styles[id(doc, name)]
}

fn pixels(values: [f32; 4]) -> [Length; 4] { values.map(Length::Px) }

fn border(style: &Style, width: [f32; 4], solid: [bool; 4], rgb: [Option<u32>; 4], alpha: [u8; 4]) {
    assert_eq!(style.border_width, pixels(width));
    assert_eq!(style.border_solid, solid);
    assert_eq!(style.border_color, rgb);
    assert_eq!(style.border_alpha, alpha);
}

fn set_style(doc: &mut Document, node: usize, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { panic!("element required"); };
    if let Some(attr) = attrs.iter_mut().find(|attr| attr.name.html_is("style")) {
        attr.value = value.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified("style".into(), value.into()));
    }
}

#[test]
fn border_defaults_literal_shorthand_inherits_each_computed_parent_component() {
    let doc = document(
        &format!("#parent{{{PARENT}}}#child{{{CHILD};border:InHeRiT}}"),
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    border(at(&doc, &styles, "child"), [2.0, 0.0, 6.0, 0.0], [true, false, true, false], PARENT_RGB, PARENT_ALPHA);
}

#[test]
fn border_defaults_each_side_shorthand_inherits_only_the_named_side() {
    for (side, index) in SIDES {
        let doc = document(
            &format!("#parent{{{PARENT}}}#child{{{CHILD};border-{side}:inherit}}"),
            "<div id=parent><div id=child></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        let mut width = [11.0, 12.0, 13.0, 14.0];
        let mut solid = [true; 4];
        let mut rgb = [Some(0x46505a); 4];
        let mut alpha = [153; 4];
        width[index] = [2.0, 0.0, 6.0, 0.0][index];
        solid[index] = [true, false, true, false][index];
        rgb[index] = PARENT_RGB[index];
        alpha[index] = PARENT_ALPHA[index];
        border(at(&doc, &styles, "child"), width, solid, rgb, alpha);
    }
}

#[test]
fn border_defaults_component_shorthands_inherit_without_resetting_siblings() {
    for component in ["width", "style", "color"] {
        let doc = document(
            &format!("#parent{{{PARENT}}}#child{{{CHILD};border-{component}:inherit}}"),
            "<div id=parent><div id=child></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        border(
            at(&doc, &styles, "child"),
            if component == "width" { [2.0, 0.0, 6.0, 0.0] } else { [11.0, 12.0, 13.0, 14.0] },
            if component == "style" { [true, false, true, false] } else { [true; 4] },
            if component == "color" { PARENT_RGB } else { [Some(0x46505a); 4] },
            if component == "color" { PARENT_ALPHA } else { [153; 4] },
        );
    }
}

#[test]
fn border_defaults_all_twelve_longhands_inherit_only_the_selected_component() {
    for (side, index) in SIDES {
        for component in ["width", "style", "color"] {
            let doc = document(
                &format!("#parent{{{PARENT}}}#child{{{CHILD};border-{side}-{component}:inherit}}"),
                "<div id=parent><div id=child></div></div>",
            );
            let styles = compute(&doc, &HashMap::new());
            let mut width = [11.0, 12.0, 13.0, 14.0];
            let mut solid = [true; 4];
            let mut rgb = [Some(0x46505a); 4];
            let mut alpha = [153; 4];
            match component {
                "width" => width[index] = [2.0, 0.0, 6.0, 0.0][index],
                "style" => solid[index] = [true, false, true, false][index],
                _ => { rgb[index] = PARENT_RGB[index]; alpha[index] = PARENT_ALPHA[index]; }
            }
            border(at(&doc, &styles, "child"), width, solid, rgb, alpha);
        }
    }
}

#[test]
fn border_defaults_initial_and_unset_reset_all_components_together() {
    for keyword in ["initial", "unset"] {
        let doc = document(
            &format!("#parent{{{PARENT}}}#child{{{CHILD};border:{keyword}}}#shown{{{CHILD};border:{keyword};border-style:solid}}"),
            "<div id=parent><div id=child></div><div id=shown></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        border(at(&doc, &styles, "child"), [3.0; 4], [false; 4], [None; 4], [255; 4]);
        border(at(&doc, &styles, "shown"), [3.0; 4], [true; 4], [None; 4], [255; 4]);
    }
}

#[test]
fn border_defaults_each_side_shorthand_initial_and_unset_preserve_other_sides() {
    for keyword in ["initial", "unset"] {
        for (side, index) in SIDES {
            let doc = document(
                &format!("#parent{{{PARENT}}}#child{{{CHILD};border-{side}:{keyword}}}"),
                "<div id=parent><div id=child></div></div>",
            );
            let styles = compute(&doc, &HashMap::new());
            let mut width = [11.0, 12.0, 13.0, 14.0];
            let mut solid = [true; 4];
            let mut rgb = [Some(0x46505a); 4];
            let mut alpha = [153; 4];
            width[index] = 3.0; solid[index] = false; rgb[index] = None; alpha[index] = 255;
            border(at(&doc, &styles, "child"), width, solid, rgb, alpha);
        }
    }
}

#[test]
fn border_defaults_component_shorthands_initial_and_unset_preserve_other_components() {
    for keyword in ["initial", "unset"] {
        for component in ["width", "style", "color"] {
            let doc = document(
                &format!("#parent{{{PARENT}}}#child{{{CHILD};border-{component}:{keyword}}}"),
                "<div id=parent><div id=child></div></div>",
            );
            let styles = compute(&doc, &HashMap::new());
            border(
                at(&doc, &styles, "child"),
                if component == "width" { [3.0; 4] } else { [11.0, 12.0, 13.0, 14.0] },
                if component == "style" { [false; 4] } else { [true; 4] },
                if component == "color" { [None; 4] } else { [Some(0x46505a); 4] },
                if component == "color" { [255; 4] } else { [153; 4] },
            );
        }
    }
}

#[test]
fn border_defaults_all_twelve_longhands_initial_and_unset_are_independent() {
    for keyword in ["initial", "unset"] {
        for (side, index) in SIDES {
            for component in ["width", "style", "color"] {
                let doc = document(
                    &format!("#parent{{{PARENT}}}#child{{{CHILD};border-{side}-{component}:{keyword}}}"),
                    "<div id=parent><div id=child></div></div>",
                );
                let styles = compute(&doc, &HashMap::new());
                let mut width = [11.0, 12.0, 13.0, 14.0];
                let mut solid = [true; 4];
                let mut rgb = [Some(0x46505a); 4];
                let mut alpha = [153; 4];
                match component {
                    "width" => width[index] = 3.0,
                    "style" => solid[index] = false,
                    _ => { rgb[index] = None; alpha[index] = 255; }
                }
                border(at(&doc, &styles, "child"), width, solid, rgb, alpha);
            }
        }
    }
}

#[test]
fn border_defaults_root_inherit_uses_initial_values_before_later_style_changes() {
    for declaration in ["border:inherit", "border:var(--missing,inherit)", "border-width:inherit;border-color:inherit;border-style:inherit"] {
        let doc = crate::html::parse(&format!(
            "<!doctype html><html id=root style='color:blue;border:19px solid red;{declaration};border-style:solid'><head></head><body id=body style='border:inherit'></body></html>"
        ));
        let styles = compute(&doc, &HashMap::new());
        for name in ["root", "body"] {
            border(at(&doc, &styles, name), [3.0; 4], [true; 4], [None; 4], [255; 4]);
        }
    }
}

#[test]
fn border_defaults_ordinary_descendants_and_unset_do_not_inherit_parent_borders() {
    let doc = document(
        "#parent{border:9px solid rgba(1,2,3,.4)}#unset{border:unset}",
        "<div id=parent><div id=ordinary></div><div id=unset></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for name in ["ordinary", "unset"] {
        border(at(&doc, &styles, name), [3.0; 4], [false; 4], [None; 4], [255; 4]);
    }
}

#[test]
fn border_defaults_literal_width_inheritance_uses_final_parent_font_metrics() {
    let doc = document(
        "html{font-size:10px}#parent{border-width:2em 3rem .5em 1rem;border-style:solid;font-size:20px}#child{border-width:inherit;border-style:solid;font-size:70px}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(at(&doc, &styles, "child").font_size, 70.0);
    assert_eq!(at(&doc, &styles, "child").border_width, pixels([40.0, 30.0, 10.0, 10.0]));
}

#[test]
fn border_defaults_variable_width_inheritance_uses_parent_computed_lengths() {
    let doc = document(
        "html{font-size:12px}#parent{border:2em solid red;border-right-width:3rem;border-bottom-width:1em;border-left-width:2rem;font-size:20px}#child{font-size:60px;border:var(--missing,InHeRiT)}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    border(at(&doc, &styles, "child"), [40.0, 36.0, 20.0, 24.0], [true; 4], [Some(0xff0000); 4], [255; 4]);
}

#[test]
fn border_defaults_multigeneration_inheritance_does_not_rescale_absolute_widths() {
    let doc = document(
        "html{font-size:15px}#parent{font-size:20px;border-width:2em 3rem;border-style:solid}#child{font-size:70px;border:inherit}#grandchild{font-size:100px;border:var(--missing,inherit)}",
        "<div id=parent><div id=child><div id=grandchild></div></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for name in ["child", "grandchild"] {
        assert_eq!(at(&doc, &styles, name).border_width, pixels([40.0, 45.0, 40.0, 45.0]), "{name}");
        assert_eq!(at(&doc, &styles, name).border_solid, [true; 4]);
    }
}

#[test]
fn border_defaults_none_and_hidden_parent_widths_inherit_zero_even_when_child_is_solid() {
    for pattern in ["none", "hidden"] {
        for declaration in ["border:inherit", "border-width:inherit", "border:var(--missing,inherit)", "border-width:var(--missing,inherit)"] {
            let doc = document(
                &format!("#parent{{font-size:20px;border:2em {pattern} red}}#child{{font-size:60px;border:7px solid blue;{declaration};border-style:solid}}"),
                "<div id=parent><div id=child></div></div>",
            );
            let styles = compute(&doc, &HashMap::new());
            assert_eq!(at(&doc, &styles, "child").border_width, pixels([0.0; 4]), "{pattern}:{declaration}");
            assert_eq!(at(&doc, &styles, "child").border_solid, [true; 4]);
        }
    }
}

#[test]
fn border_defaults_parent_final_style_controls_inheritance_independent_of_declaration_order() {
    for parent in [
        "border-width:2em 3em 4em 5em;border-style:solid none hidden solid;font-size:10px",
        "font-size:10px;border-style:solid none hidden solid;border-width:2em 3em 4em 5em",
    ] {
        for child in [
            "border-style:solid;border-width:inherit",
            "border-width:inherit;border-style:solid",
            "border-style:solid;border-top-width:inherit;border-right-width:inherit;border-bottom-width:inherit;border-left-width:inherit",
        ] {
            let doc = document(&format!("#parent{{{parent}}}#child{{font-size:70px;{child}}}"), "<div id=parent><div id=child></div></div>");
            let styles = compute(&doc, &HashMap::new());
            assert_eq!(at(&doc, &styles, "child").border_width, pixels([20.0, 0.0, 0.0, 50.0]));
        }
    }
}

#[test]
fn border_defaults_inherited_currentcolor_remains_live_on_each_descendant() {
    for declaration in ["border:inherit", "border-color:inherit", "border:var(--missing,inherit)", "border-color:var(--missing,inherit)"] {
        let doc = document(
            &format!("#parent{{color:red;border:3px solid currentcolor}}#child{{border:8px solid black;{declaration};color:blue}}#grandchild{{color:green;border-color:inherit}}"),
            "<div id=parent><div id=child><div id=grandchild></div></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        for (name, color) in [("child", 0x0000ff), ("grandchild", 0x008000)] {
            let style = at(&doc, &styles, name);
            assert_eq!(style.border_color, [None; 4], "{name}:{declaration}");
            assert_eq!(style.border_alpha, [255; 4]);
            assert_eq!(style.border_color.map(|value| value.unwrap_or(style.color)), [color; 4]);
        }
    }
    for (side, index) in SIDES {
        let doc = document(
            &format!("#parent{{color:red;border-color:currentcolor}}#child{{color:blue;border-color:black;border-{side}-color:inherit}}"),
            "<div id=parent><div id=child></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        let child = at(&doc, &styles, "child");
        let mut rgb = [Some(0); 4]; rgb[index] = None;
        assert_eq!(child.border_color, rgb);
        assert_eq!(child.border_color[index].unwrap_or(child.color), 0x0000ff);
    }
}

#[test]
fn border_defaults_explicit_rgb_alpha_and_transparent_inherit_without_currentcolor_rebinding() {
    let doc = document(
        "#parent{color:red;border-width:2px;border-style:solid;border-color:rgba(10,20,30,.2) transparent rgba(40,50,60,.6) currentcolor}#child{color:blue;border:inherit}#grandchild{color:green;border:var(--missing,inherit)}",
        "<div id=parent><div id=child><div id=grandchild></div></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for (name, color) in [("child", 0x0000ff), ("grandchild", 0x008000)] {
        let style = at(&doc, &styles, name);
        border(style, [2.0; 4], [true; 4], [Some(0x0a141e), Some(0), Some(0x28323c), None], [51, 0, 153, 255]);
        assert_eq!(style.border_color.map(|rgb| rgb.unwrap_or(style.color)), [0x0a141e, 0, 0x28323c, color]);
    }
}

#[test]
fn border_defaults_literal_and_variable_keywords_agree_for_all_twenty_properties() {
    let mut properties = vec!["border".to_owned(), "border-width".into(), "border-style".into(), "border-color".into()];
    for (side, _) in SIDES {
        properties.push(format!("border-{side}"));
        for component in ["width", "style", "color"] { properties.push(format!("border-{side}-{component}")); }
    }
    assert_eq!(properties.len(), 20);
    for property in properties {
        for keyword in ["inherit", "initial", "unset"] {
            let doc = document(
                &format!("#parent{{{PARENT};border-top-color:currentcolor;color:red}}#literal{{{CHILD};color:blue;{property}:{keyword}}}#variable{{{CHILD};color:blue;{property}:var(--missing,{keyword})}}"),
                "<div id=parent><div id=literal></div><div id=variable></div></div>",
            );
            let styles = compute(&doc, &HashMap::new());
            let literal = at(&doc, &styles, "literal");
            let variable = at(&doc, &styles, "variable");
            assert_eq!(literal.border_width, variable.border_width, "{property}:{keyword}");
            assert_eq!(literal.border_solid, variable.border_solid, "{property}:{keyword}");
            assert_eq!(literal.border_color, variable.border_color, "{property}:{keyword}");
            assert_eq!(literal.border_alpha, variable.border_alpha, "{property}:{keyword}");
        }
    }
}

#[test]
fn border_defaults_shorthand_longhand_order_specificity_and_importance_remain_ordered() {
    let doc = document(
        "#parent{border:2px solid red;border-right-width:4px;border-bottom-width:6px;border-left-width:8px}#a{border-left:99px solid blue;border:inherit;border-top-width:11px}#b{border:inherit;border-left:12px solid blue}#c{border:inherit!important}.low{border:90px solid blue}#d{border:inherit}",
        "<div id=parent><div id=a></div><div id=b></div><div id=c class=low style='border:91px solid blue;border-right-width:17px!important'></div><div id=d class=low></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for (name, width) in [("a", [11.0, 4.0, 6.0, 8.0]), ("b", [2.0, 4.0, 6.0, 12.0]), ("c", [2.0, 17.0, 6.0, 8.0]), ("d", [2.0, 4.0, 6.0, 8.0])] {
        let mut rgb = [Some(0xff0000); 4];
        if name == "b" { rgb[3] = Some(0x0000ff); }
        border(at(&doc, &styles, name), width, [true; 4], rgb, [255; 4]);
    }
}

#[test]
fn border_defaults_invalid_literal_keywords_preserve_fallback_but_invalid_substitution_resets() {
    for value in ["inherit 1px", "1px initial", "unset solid", "inherit inherit", "red initial"] {
        let doc = document(
            &format!("#parent{{border:21px solid green}}#literal{{border:9px solid rgba(1,2,3,.4);border:{value};border-left-width:3px}}#variable{{border:9px solid rgba(1,2,3,.4);border:var(--missing,{value});border-left-width:3px}}"),
            "<div id=parent><div id=literal></div><div id=variable></div></div>",
        );
        let styles = compute(&doc, &HashMap::new());
        border(at(&doc, &styles, "literal"), [9.0, 9.0, 9.0, 3.0], [true; 4], [Some(0x010203); 4], [102; 4]);
        border(at(&doc, &styles, "variable"), [3.0; 4], [false; 4], [None; 4], [255; 4]);
    }
}

#[test]
fn border_defaults_invalid_widths_and_styles_preserve_borders_and_other_box_properties() {
    let doc = document(
        "#parent{border:7px solid red;margin:31px;padding:33px;width:90px}#child{border:inherit;border-width:-2px;border-right-width:10%;border-bottom-width:auto;border-left-width:calc(2px * 2);border-style:solid invalid;border-color:inherit invalid;margin:3px;padding:5px;width:21px;height:22px;gap:6px}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    let child = at(&doc, &styles, "child");
    border(child, [7.0; 4], [true; 4], [Some(0xff0000); 4], [255; 4]);
    assert_eq!(child.margin, pixels([3.0; 4]));
    assert_eq!(child.padding, pixels([5.0; 4]));
    assert_eq!(child.width, Length::Px(21.0));
    assert_eq!(child.height, Length::Px(22.0));
    assert_eq!(child.gap, [Length::Px(6.0); 2]);
}

#[test]
fn border_defaults_external_source_replacement_recomputes_inherited_width_style_and_color() {
    let doc = crate::html::parse(
        "<!doctype html><html><head><link id=sheet rel=stylesheet></head><body><div id=parent><div id=child style='color:blue;font-size:70px;border:inherit;border-style:solid'></div></div></body></html>",
    );
    let sheet = id(&doc, "sheet");
    let mut external = HashMap::from([(sheet, "#parent{font-size:10px;border:2em solid rgba(1,2,3,.4)}".into())]);
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    border(at(&doc, &first.styles, "child"), [20.0; 4], [true; 4], [Some(0x010203); 4], [102; 4]);
    external.insert(sheet, "#parent{font-size:12px;border:4em solid currentcolor;border-right-style:none}".into());
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    let child = at(&doc, &next.styles, "child");
    border(child, [48.0, 0.0, 48.0, 48.0], [true; 4], [None; 4], [255; 4]);
    assert_eq!(child.border_color.map(|rgb| rgb.unwrap_or(child.color)), [0x0000ff; 4]);
    assert_eq!(child.border_width, at(&doc, &fresh, "child").border_width);
    assert_eq!(child.border_color, at(&doc, &fresh, "child").border_color);
    assert!(!first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn border_defaults_parent_inline_changes_reuse_rules_and_recompute_descendant_values() {
    let mut doc = document(
        "#child{font-size:70px;color:blue;border:var(--missing,inherit);border-style:solid}",
        "<div id=parent style='font-size:10px;border:2em solid red'><div id=child></div></div>",
    );
    let external = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    border(at(&doc, &first.styles, "child"), [20.0; 4], [true; 4], [Some(0xff0000); 4], [255; 4]);
    let parent = id(&doc, "parent");
    set_style(&mut doc, parent, "font-size:12px;border:5em solid currentcolor;border-left-style:hidden");
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    border(at(&doc, &next.styles, "child"), [60.0, 60.0, 60.0, 0.0], [true; 4], [None; 4], [255; 4]);
    assert_eq!(at(&doc, &next.styles, "child").border_width, at(&doc, &fresh, "child").border_width);
    assert_eq!(at(&doc, &next.styles, "child").border_color, at(&doc, &fresh, "child").border_color);
    assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn border_defaults_work_and_copy_refusals_preserve_source_and_allow_fresh_recovery() {
    let doc = document(
        "#parent{font-size:10px;color:red;border:2em solid currentcolor;border-right-style:none}#child{font-size:70px;color:blue;border:inherit;border-left-width:var(--missing,inherit);border-style:solid}",
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
    border(at(&doc, &recovered.styles, "child"), [20.0, 0.0, 20.0, 20.0], [true; 4], [None; 4], [255; 4]);
    assert_eq!(format!("{:?}", doc.nodes), before);
    assert!(work < 1_000_000 && copies < 1_000_000);
    assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn border_defaults_document_sentinel_cannot_supply_inherited_root_border_values() {
    for declaration in ["border:inherit", "border:var(--missing,inherit)"] {
        for (after, solid) in [("", false), ("border-style:solid", true)] {
            let doc = crate::html::parse(&format!(
                "<!doctype html><html id=root style='color:blue;{declaration};{after}'><head><style>*{{border:19px solid rgba(1,2,3,.4)}}</style></head><body></body></html>"
            ));
            assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
            assert!(matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == "#document"));
            let styles = compute(&doc, &HashMap::new());
            let root = at(&doc, &styles, "root");
            border(root, [3.0; 4], [solid; 4], [None; 4], [255; 4]);
            assert_eq!(root.border_color.map(|rgb| rgb.unwrap_or(root.color)), [0x0000ff; 4]);
        }
    }
}

#[test]
fn border_defaults_fragment_context_is_not_a_parent_but_real_null_namespace_element_is() {
    for declaration in ["border:inherit", "border:var(--missing,inherit)"] {
        let mut doc = crate::html::parse_fragment(
            "<link id=sheet rel=stylesheet><section id=top><div id=real><span id=inner></span></div></section>",
            "div",
            false,
        ).unwrap();
        assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
        assert!(matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == "div"));
        let real = id(&doc, "real");
        assert_ne!(real, 0);
        doc.nodes[real].namespace = crate::html::Namespace::None;
        let external = HashMap::from([(id(&doc, "sheet"), format!(
            "div{{border:29px solid rgba(9,8,7,.2)}}#top{{color:blue;{declaration};border-style:solid}}#real{{border:7px solid rgba(1,2,3,.4)}}#inner{{color:green;{declaration}}}"
        ))]);
        let styles = compute(&doc, &external);
        border(at(&doc, &styles, "top"), [3.0; 4], [true; 4], [None; 4], [255; 4]);
        border(at(&doc, &styles, "inner"), [7.0; 4], [true; 4], [Some(0x010203); 4], [102; 4]);
    }
}
