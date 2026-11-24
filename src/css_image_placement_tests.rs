use super::*;

const PROPERTIES: [&str; 6] = [
    "background-position", "background-repeat", "background-size",
    "image-rendering", "object-fit", "object-position",
];
const POSITIONS: [&str; 2] = ["object-position", "background-position"];
const PARENT: &str = "object-position:right 8px bottom 25%;background-position:25% 12px;object-fit:cover;image-rendering:pixelated;background-repeat:repeat-x;background-size:cover";
const CHILD: &str = "object-position:left 3px top 4px;background-position:75% 17px;object-fit:contain;image-rendering:smooth;background-repeat:no-repeat;background-size:contain";
const VALUES: [(&str, &str); 6] = [
    ("object-position", "right 8px bottom 25%"),
    ("background-position", "25% 12px"),
    ("object-fit", "cover"), ("image-rendering", "pixelated"),
    ("background-repeat", "repeat-x"), ("background-size", "cover"),
];

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style {
    &styles[id(doc, name)]
}

fn position(style: &Style, property: &str) -> [ObjectPosition; 2] {
    if property == "object-position" { style.object_position } else { style.background_position }
}

fn used(style: &Style, property: &str, free_space: [f32; 2]) -> [Option<f32>; 2] {
    let position = position(style, property);
    std::array::from_fn(|axis| {
        position[axis].offset.resolve(0.0, style.font_size, style.root_font_size)
            .map(|offset| position[axis].fraction * free_space[axis] + offset)
    })
}

fn assert_used(style: &Style, property: &str, free_space: [f32; 2], expected: [f32; 2]) {
    let actual = used(style, property, free_space);
    for axis in 0..2 {
        assert!(actual[axis].is_some_and(|value| (value - expected[axis]).abs() < 0.0005),
            "{property} axis {axis}: {actual:?}, expected {expected:?}, free space {free_space:?}");
    }
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    object: [Option<f32>; 2],
    background: [Option<f32>; 2],
    fit: ObjectFit,
    rendering: ImageRendering,
    repeat: [bool; 2],
    size: BackgroundSize,
}

fn snapshot(style: &Style) -> Placement {
    Placement {
        object: used(style, "object-position", [200.0, 100.0]),
        background: used(style, "background-position", [200.0, 100.0]),
        fit: style.object_fit, rendering: style.image_rendering,
        repeat: style.background_repeat, size: style.background_size,
    }
}

fn initial() -> Placement {
    Placement {
        object: [Some(100.0), Some(50.0)], background: [Some(0.0); 2],
        fit: ObjectFit::Fill, rendering: ImageRendering::Auto,
        repeat: [true; 2], size: BackgroundSize::Auto,
    }
}

fn parent_values() -> Placement {
    Placement {
        object: [Some(192.0), Some(75.0)], background: [Some(50.0), Some(12.0)],
        fit: ObjectFit::Cover, rendering: ImageRendering::Pixelated,
        repeat: [true, false], size: BackgroundSize::Cover,
    }
}

fn child_values() -> Placement {
    Placement {
        object: [Some(3.0), Some(4.0)], background: [Some(150.0), Some(17.0)],
        fit: ObjectFit::Contain, rendering: ImageRendering::Smooth,
        repeat: [false; 2], size: BackgroundSize::Contain,
    }
}

fn replace_property(target: &mut Placement, source: Placement, property: &str) {
    match property {
        "object-position" => target.object = source.object,
        "background-position" => target.background = source.background,
        "object-fit" => target.fit = source.fit,
        "image-rendering" => target.rendering = source.rendering,
        "background-repeat" => target.repeat = source.repeat,
        "background-size" => target.size = source.size,
        _ => panic!("unknown image property"),
    }
}

fn assert_names(source: &str, expected: &[&str]) {
    let names = inline_declaration_names(source, &PROPERTIES, None).unwrap();
    assert_eq!(names.iter().map(String::as_str).collect::<Vec<_>>(), expected.to_vec(), "{source}");
}

#[test]
fn image_placement_defaults_only_inherit_image_rendering_naturally() {
    let doc = document(&format!("#parent{{{PARENT}}}"), "<div id=parent><div id=child></div></div><div id=ordinary></div>");
    let styles = compute(&doc, &HashMap::new());
    let mut expected = initial();
    expected.rendering = ImageRendering::Pixelated;
    assert_eq!(snapshot(at(&doc, &styles, "child")), expected);
    assert_eq!(snapshot(at(&doc, &styles, "ordinary")), initial());
}

#[test]
fn image_placement_object_fit_keywords_share_literal_and_variable_behavior() {
    for (value, expected) in [
        ("fill", ObjectFit::Fill), ("contain", ObjectFit::Contain),
        ("cover", ObjectFit::Cover), ("none", ObjectFit::None), ("scale-down", ObjectFit::ScaleDown),
    ] {
        for declaration in [value.to_owned(), "var(--value)".to_owned()] {
            let doc = document(&format!("#target{{--value:{value};object-fit:{declaration};object-fit:cover none}}"), "<img id=target>");
            assert_eq!(at(&doc, &compute(&doc, &HashMap::new()), "target").object_fit, expected, "{value}/{declaration}");
        }
    }
}

#[test]
fn image_placement_rendering_keywords_and_existing_aliases_share_variable_behavior() {
    for (value, expected) in [
        ("auto", ImageRendering::Auto), ("smooth", ImageRendering::Smooth),
        ("high-quality", ImageRendering::HighQuality), ("crisp-edges", ImageRendering::CrispEdges),
        ("pixelated", ImageRendering::Pixelated), ("optimizeSpeed", ImageRendering::CrispEdges),
        ("optimizeQuality", ImageRendering::Smooth),
    ] {
        for declaration in [value.to_owned(), "var(--value)".to_owned()] {
            let doc = document(&format!("#target{{--value:{value};image-rendering:{declaration};image-rendering:nearest-neighbor}}"), "<img id=target>");
            assert_eq!(at(&doc, &compute(&doc, &HashMap::new()), "target").image_rendering, expected, "{value}/{declaration}");
        }
    }
}

#[test]
fn image_placement_background_repeat_keeps_the_supported_keyword_range() {
    for (value, expected) in [
        ("repeat", [true, true]), ("no-repeat", [false, false]),
        ("repeat-x", [true, false]), ("repeat-y", [false, true]),
    ] {
        for declaration in [value.to_owned(), "var(--value)".to_owned()] {
            let doc = document(&format!("#target{{--value:{value};background-repeat:{declaration};background-repeat:round}}"), "<div id=target></div>");
            assert_eq!(at(&doc, &compute(&doc, &HashMap::new()), "target").background_repeat, expected, "{value}/{declaration}");
        }
    }
}

#[test]
fn image_placement_background_size_keeps_the_supported_keyword_range() {
    for (value, expected) in [("auto", BackgroundSize::Auto), ("contain", BackgroundSize::Contain), ("cover", BackgroundSize::Cover)] {
        for declaration in [value.to_owned(), "var(--value)".to_owned()] {
            let doc = document(&format!("#target{{--value:{value};background-size:{declaration};background-size:32px 24px}}"), "<div id=target></div>");
            assert_eq!(at(&doc, &compute(&doc, &HashMap::new()), "target").background_size, expected, "{value}/{declaration}");
        }
    }
}

#[test]
fn image_placement_explicit_inherit_changes_only_the_named_property() {
    for property in PROPERTIES {
        for value in ["InHeRiT", "var(--missing,inherit)"] {
            let doc = document(&format!("#parent{{{PARENT}}}#child{{{CHILD};{property}:{value}}}"), "<div id=parent><div id=child></div></div>");
            let mut expected = child_values();
            replace_property(&mut expected, parent_values(), property);
            assert_eq!(snapshot(at(&doc, &compute(&doc, &HashMap::new()), "child")), expected, "{property}:{value}");
        }
    }
}

#[test]
fn image_placement_initial_resets_only_the_named_property_through_both_value_routes() {
    for property in PROPERTIES {
        for value in ["initial", "var(--missing,initial)"] {
            let doc = document(&format!("#parent{{{PARENT}}}#child{{{CHILD};{property}:{value}}}"), "<div id=parent><div id=child></div></div>");
            let mut expected = child_values();
            replace_property(&mut expected, initial(), property);
            assert_eq!(snapshot(at(&doc, &compute(&doc, &HashMap::new()), "child")), expected, "{property}:{value}");
        }
    }
}

#[test]
fn image_placement_unset_inherits_rendering_and_initializes_the_other_five_properties() {
    for property in PROPERTIES {
        for value in ["unset", "var(--missing,unset)"] {
            let doc = document(&format!("#parent{{{PARENT}}}#child{{{CHILD};{property}:{value}}}"), "<div id=parent><div id=child></div></div>");
            let mut expected = child_values();
            replace_property(&mut expected, if property == "image-rendering" { parent_values() } else { initial() }, property);
            assert_eq!(snapshot(at(&doc, &compute(&doc, &HashMap::new()), "child")), expected, "{property}:{value}");
        }
    }
}

#[test]
fn image_placement_all_six_properties_apply_resolved_custom_values_locally() {
    for (property, value) in VALUES {
        let doc = document(&format!("#child{{{CHILD};--chosen:{value};{property}:var(--chosen)}}"), "<div id=child></div>");
        let mut expected = child_values();
        replace_property(&mut expected, parent_values(), property);
        assert_eq!(snapshot(at(&doc, &compute(&doc, &HashMap::new()), "child")), expected, "{property}");
    }
}

#[test]
fn image_placement_invalid_literals_do_not_displace_valid_property_winners() {
    for (property, invalid) in [
        ("object-position", "left right"), ("background-position", "left left"),
        ("object-fit", "cover none"), ("image-rendering", "smooth pixelated"),
        ("background-repeat", "repeat no-repeat"), ("background-size", "contain,cover"),
    ] {
        let doc = document(&format!("#child{{{CHILD};{property}:{invalid}!important;width:17px}}"), "<div id=child></div>");
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(snapshot(at(&doc, &styles, "child")), child_values(), "{property}");
        assert_eq!(at(&doc, &styles, "child").width, Length::Px(17.0));
    }
}

#[test]
fn image_placement_invalid_variables_apply_unset_without_reviving_earlier_declarations() {
    for property in PROPERTIES {
        for value in ["var(--missing)", "var(--cycle)", "var(--invalid)", "var(--missing,)"] {
            let doc = document(&format!("#parent{{{PARENT}}}#child{{{CHILD};--cycle:var(--cycle);--invalid:bogus;{property}:{value}}}"), "<div id=parent><div id=child></div></div>");
            let mut expected = child_values();
            replace_property(&mut expected, if property == "image-rendering" { parent_values() } else { initial() }, property);
            assert_eq!(snapshot(at(&doc, &compute(&doc, &HashMap::new()), "child")), expected, "{property}:{value}");
        }
    }
}

#[test]
fn image_placement_root_defaults_and_keywords_ignore_the_styled_document_sentinel() {
    let mut doc = crate::html::parse("<!doctype html><html id=root><head></head><body></body></html>");
    set_style(&mut doc, 0, PARENT);
    assert_eq!(snapshot(at(&doc, &compute(&doc, &HashMap::new()), "root")), initial());
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            for value in [keyword.to_owned(), format!("var(--missing,{keyword})")] {
                let mut doc = crate::html::parse(&format!("<!doctype html><html id=root style='{CHILD};{property}:{value}'><head></head><body></body></html>"));
                set_style(&mut doc, 0, PARENT);
                let mut expected = child_values();
                replace_property(&mut expected, initial(), property);
                assert_eq!(snapshot(at(&doc, &compute(&doc, &HashMap::new()), "root")), expected, "{property}:{value}");
            }
        }
    }
}

#[test]
fn image_placement_fragment_context_is_excluded_but_a_real_null_namespace_parent_is_used() {
    for value in ["inherit", "var(--missing,inherit)"] {
        let declarations = PROPERTIES.map(|property| format!("{property}:{value};")).join("");
        let mut doc = crate::html::parse_fragment("<link id=sheet rel=stylesheet><section id=top><div id=real><span id=inner></span></div></section>", "div", false).unwrap();
        assert!(matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == "div"));
        assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
        let real = id(&doc, "real");
        doc.nodes[real].namespace = crate::html::Namespace::None;
        let external = HashMap::from([(id(&doc, "sheet"), format!("div{{{PARENT}}}#top,#inner{{{declarations}}}"))]);
        let styles = compute(&doc, &external);
        assert_eq!(snapshot(at(&doc, &styles, "top")), initial());
        assert_eq!(snapshot(at(&doc, &styles, "inner")), parent_values());
    }
}

#[test]
fn image_placement_one_and_two_component_positions_keep_physical_axis_rules() {
    for property in POSITIONS {
        for (value, expected) in [
            ("left", [0.0, 50.0]), ("bottom", [100.0, 100.0]), ("center", [100.0, 50.0]),
            ("12px", [12.0, 50.0]), ("25%", [50.0, 50.0]), ("top right", [200.0, 0.0]),
            ("center left", [0.0, 50.0]), ("right center", [200.0, 50.0]),
            ("left 4px", [0.0, 4.0]), ("25% bottom", [50.0, 100.0]), ("-4px 75%", [-4.0, 75.0]),
        ] {
            let doc = document(&format!("#target{{{property}:{value}}}"), "<div id=target></div>");
            assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), property, [200.0, 100.0], expected);
        }
    }
}

#[test]
fn image_placement_background_accepts_three_components_with_one_implicit_edge_offset() {
    for (value, expected) in [
        ("top 4px right", [200.0, 4.0]), ("right 8px bottom", [192.0, 100.0]),
        ("center top 4px", [100.0, 4.0]), ("right 5% center", [190.0, 50.0]),
        ("bottom 25% left", [0.0, 75.0]), ("left bottom 7px", [0.0, 93.0]),
        ("right calc(2px + 10%) bottom", [178.0, 100.0]),
    ] {
        for declaration in [value.to_owned(), format!("var(--missing,{value})")] {
            let doc = document(&format!("#target{{background-position:{declaration}}}"), "<div id=target></div>");
            assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), "background-position", [200.0, 100.0], expected);
        }
    }
}

#[test]
fn image_placement_object_rejects_three_components_even_when_background_would_accept_them() {
    for value in ["top 4px right", "right 8px bottom", "center top 4px", "right 5% center", "left bottom 7px"] {
        let doc = document(&format!("#literal{{object-position:12px 18px;object-position:{value}}}#variable{{object-position:12px 18px;object-position:var(--missing,{value})}}"), "<div id=literal></div><div id=variable></div>");
        let styles = compute(&doc, &HashMap::new());
        assert_used(at(&doc, &styles, "literal"), "object-position", [200.0, 100.0], [12.0, 18.0]);
        assert_used(at(&doc, &styles, "variable"), "object-position", [200.0, 100.0], [100.0, 50.0]);
        assert_names(&format!("object-position:{value}"), &[]);
    }
}

#[test]
fn image_placement_four_component_positions_apply_edges_in_either_axis_order() {
    for property in POSITIONS {
        for (value, expected) in [
            ("right 8px bottom 25%", [192.0, 75.0]), ("bottom 25% right 8px", [192.0, 75.0]),
            ("left -5px top -10%", [-5.0, -10.0]), ("bottom -6px right -25%", [250.0, 106.0]),
            ("top 2em left 3rem", [30.0, 40.0]),
        ] {
            let doc = document(&format!("html{{font-size:10px}}#target{{font-size:20px;{property}:{value}}}"), "<div id=target></div>");
            assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), property, [200.0, 100.0], expected);
        }
    }
}

#[test]
fn image_placement_lengths_allow_supported_units_negative_values_and_unitless_zero() {
    for property in POSITIONS {
        for (value, expected) in [
            ("0", [0.0, 50.0]), ("-0 +0", [0.0, 0.0]), ("3pt 2pc", [4.0, 32.0]),
            ("-.5em +.25rem", [-10.0, 2.5]), ("-25% 125%", [-50.0, 125.0]),
            ("+2px -3px", [2.0, -3.0]),
        ] {
            let doc = document(&format!("html{{font-size:10px}}#target{{font-size:20px;{property}:{value}}}"), "<div id=target></div>");
            assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), property, [200.0, 100.0], expected);
        }
    }
}

#[test]
fn image_placement_typed_calculations_accept_grouped_signed_arithmetic() {
    for property in POSITIONS {
        for (value, expected) in [
            ("calc(50% - 20px)", [80.0, 50.0]),
            ("calc(2 * (3pt + 1pc)) calc(-1rem + 2em)", [40.0, 30.0]),
            ("calc(2 * calc(10px + 5%)) calc(10px - 50%)", [40.0, -40.0]),
            ("calc((1em + 2rem) / 2) calc(-10px * 2)", [20.0, -20.0]),
        ] {
            let doc = document(&format!("html{{font-size:10px}}#target{{font-size:20px;{property}:{value}}}"), "<div id=target></div>");
            assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), property, [200.0, 100.0], expected);
        }
    }
}

#[test]
fn image_placement_calculated_percentages_follow_positive_and_negative_free_space() {
    for property in POSITIONS {
        let doc = document(&format!("#target{{{property}:calc(25% + 10px) calc(75% - 5px)}}"), "<div id=target></div>");
        let styles = compute(&doc, &HashMap::new());
        let target = at(&doc, &styles, "target");
        assert_used(target, property, [200.0, 100.0], [60.0, 70.0]);
        assert_used(target, property, [400.0, 200.0], [110.0, 145.0]);
        assert_used(target, property, [-80.0, -40.0], [-10.0, -35.0]);
        let axes = position(target, property);
        assert_eq!([axes[0].fraction, axes[1].fraction], [0.25, 0.75]);
        assert_eq!(axes[0].offset.resolve(0.0, target.font_size, target.root_font_size), Some(10.0));
        assert_eq!(axes[1].offset.resolve(0.0, target.font_size, target.root_font_size), Some(-5.0));
    }
}

#[test]
fn image_placement_right_and_bottom_calculations_reverse_percent_and_length_signs() {
    for property in POSITIONS {
        for (value, positive, negative) in [
            ("right calc(25% + 10px) bottom calc(50% - 6px)", [140.0, 56.0], [-70.0, -14.0]),
            ("bottom calc(10% + 1em) left calc(-25% + 2rem)", [-30.0, 70.0], [40.0, -56.0]),
            ("right calc(-25% - 2em) bottom calc(-50% + 1rem)", [290.0, 140.0], [-60.0, -70.0]),
        ] {
            let doc = document(&format!("html{{font-size:10px}}#target{{font-size:20px;{property}:{value}}}"), "<div id=target></div>");
            let styles = compute(&doc, &HashMap::new());
            assert_used(at(&doc, &styles, "target"), property, [200.0, 100.0], positive);
            assert_used(at(&doc, &styles, "target"), property, [-80.0, -40.0], negative);
        }
    }
}

#[test]
fn image_placement_explicit_inheritance_uses_final_parent_fonts_for_different_siblings() {
    let doc = document(
        "html{font-size:10px}#parent{object-position:right 2em bottom 3rem;background-position:1em -2rem;font-size:20px}#small{font-size:5px;object-position:inherit;background-position:var(--missing,inherit)}#large{object-position:var(--missing,inherit);background-position:inherit;font-size:80px}",
        "<div id=parent><div id=small></div><div id=large></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for name in ["small", "large"] {
        let style = at(&doc, &styles, name);
        assert_used(style, "object-position", [200.0, 100.0], [160.0, 70.0]);
        assert_used(style, "background-position", [200.0, 100.0], [20.0, -20.0]);
        assert_eq!(style.object_position[0].offset.resolve(0.0, 999.0, 888.0), Some(-40.0));
        assert_eq!(style.object_position[1].offset.resolve(0.0, 999.0, 888.0), Some(-30.0));
    }
}

#[test]
fn image_placement_multigeneration_inheritance_keeps_percentages_and_freezes_font_terms_once() {
    let doc = document(
        "html{font-size:10px}#parent{font-size:20px;object-position:right calc(25% + 2em) bottom calc(50% - 1rem);background-position:calc(10% + 1em + 1rem) calc(20% - 2em)}#child{font-size:70px;object-position:inherit;background-position:inherit}#grandchild{font-size:100px;object-position:var(--missing,inherit);background-position:var(--missing,inherit)}",
        "<div id=parent><div id=child><div id=grandchild></div></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    for name in ["child", "grandchild"] {
        let style = at(&doc, &styles, name);
        assert_used(style, "object-position", [200.0, 100.0], [110.0, 60.0]);
        assert_used(style, "object-position", [400.0, 200.0], [260.0, 110.0]);
        assert_used(style, "background-position", [200.0, 100.0], [50.0, -20.0]);
        assert_used(style, "background-position", [400.0, 200.0], [70.0, 0.0]);
    }
}

#[test]
fn image_placement_root_position_math_uses_its_final_font_before_explicit_inheritance() {
    let doc = crate::html::parse("<!doctype html><html id=root style='object-position:calc(1em + 2rem) calc(25% + 1rem);background-position:right calc(2em + 10%) bottom calc(1rem - 5%);font-size:10px'><head></head><body id=child style='font-size:70px;object-position:inherit;background-position:inherit'></body></html>");
    let styles = compute(&doc, &HashMap::new());
    for name in ["root", "child"] {
        assert_used(at(&doc, &styles, name), "object-position", [200.0, 100.0], [30.0, 35.0]);
        assert_used(at(&doc, &styles, name), "background-position", [200.0, 100.0], [160.0, 95.0]);
    }
    assert_eq!(at(&doc, &styles, "child").object_position[0].offset.resolve(0.0, 100.0, 200.0), Some(30.0));
}

#[test]
fn image_placement_viewport_lengths_use_the_real_environment_and_keep_percentage_free_space() {
    let doc = document("#target{object-position:calc(5vw + 10%) -10vh;background-position:right 2vw bottom calc(5vh + 25%)}", "<div id=target></div>");
    let styles = compute_with_environment(&doc, &HashMap::new(), None, Some(environment(1000.0, 600.0)));
    assert_used(at(&doc, &styles, "target"), "object-position", [200.0, 100.0], [70.0, -60.0]);
    assert_used(at(&doc, &styles, "target"), "background-position", [200.0, 100.0], [180.0, 45.0]);
}

#[test]
fn image_placement_unavailable_viewports_leave_supported_offsets_unresolved() {
    for property in POSITIONS {
        let doc = document(&format!("#target{{{property}:5vw calc(2vh + 10%)}}"), "<div id=target></div>");
        for env in [None, Some(environment(-1.0, 600.0)), Some(environment(f64::NAN, 600.0)), Some(environment(800.0, f64::INFINITY))] {
            let styles = compute_with_environment(&doc, &HashMap::new(), None, env);
            assert_eq!(used(at(&doc, &styles, "target"), property, [200.0, 100.0]), [None, None]);
        }
    }
}

#[test]
fn image_placement_viewport_changes_recompute_inherited_positions_with_cached_sources() {
    let doc = document(
        "#parent{font-size:20px;object-position:right calc(10vw + 1em) bottom 5vh;background-position:calc(5vw + 25%) calc(10vh - 1em)}#child{font-size:70px;object-position:inherit;background-position:var(--missing,inherit)}",
        "<div id=parent><div id=child></div></div>",
    );
    let external = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, Some(environment(800.0, 600.0)), None);
    assert_used(at(&doc, &first.styles, "child"), "object-position", [200.0, 100.0], [100.0, 70.0]);
    assert_used(at(&doc, &first.styles, "child"), "background-position", [200.0, 100.0], [90.0, 40.0]);
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, Some(environment(1200.0, 1000.0)), first.stylesheet_cache.as_ref());
    let fresh = compute_with_environment(&doc, &external, None, Some(environment(1200.0, 1000.0)));
    assert_used(at(&doc, &next.styles, "child"), "object-position", [200.0, 100.0], [60.0, 50.0]);
    assert_used(at(&doc, &next.styles, "child"), "background-position", [200.0, 100.0], [110.0, 80.0]);
    assert_eq!(snapshot(at(&doc, &next.styles, "child")), snapshot(at(&doc, &fresh, "child")));
}

#[test]
fn image_placement_custom_property_scope_preserves_aliases_case_and_grouped_fallbacks() {
    let doc = document(
        "html{font-size:10px}#parent{--edge:calc(1rem + 25%);--alias:var(--edge);--X:5px;--x:7px}#child{--edge:99px;object-position:var(--missing,var(--alias)) var(--X);background-position:var(--x) var(--alias)}",
        "<div id=parent><div id=child></div></div>",
    );
    let styles = compute(&doc, &HashMap::new());
    assert_used(at(&doc, &styles, "child"), "object-position", [200.0, 100.0], [60.0, 5.0]);
    assert_used(at(&doc, &styles, "child"), "background-position", [200.0, 100.0], [7.0, 35.0]);
}

#[test]
fn image_placement_variable_substitution_cannot_join_dimension_function_or_keyword_tokens() {
    for property in POSITIONS {
        for value in ["var(--number)px", "var(--number)%", "var(--number)vw", "calc(var(--number)px + 1px)", "var(--function)(2px)", "var(--edge)ward"] {
            let doc = document(&format!("#target{{--number:2;--function:calc;--edge:right;{property}:12px 18px;{property}:{value}}}"), "<div id=target></div>");
            let expected = if property == "object-position" { [100.0, 50.0] } else { [0.0; 2] };
            assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), property, [200.0, 100.0], expected);
        }
    }
    let doc = document("#parent{image-rendering:pixelated}#child{--fit:scale;--render:crisp;object-fit:cover;object-fit:var(--fit)-down;image-rendering:smooth;image-rendering:var(--render)-edges}", "<div id=parent><div id=child></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(at(&doc, &styles, "child").object_fit, ObjectFit::Fill);
    assert_eq!(at(&doc, &styles, "child").image_rendering, ImageRendering::Pixelated);
}

#[test]
fn image_placement_invalid_math_is_atomic_and_later_declarations_and_rules_survive() {
    for property in POSITIONS {
        for value in [
            "calc(1px + 0)", "calc(2px * 3px)", "calc(20px / 0)", "calc(1px+2px)",
            "calc(1px +2px)", "calc(2 * 3 px)", "calc(1e999 * 2px)",
            "calc(2px + auto)", "calc(2px + 1ch)", "calc(2px))", "calc()",
        ] {
            let doc = document(&format!("#target{{{property}:12px 18px;{property}:left 1px bottom {value};width:17px}}#after{{{property}:3px 4px}}"), "<div id=target></div><div id=after></div>");
            let styles = compute(&doc, &HashMap::new());
            assert_used(at(&doc, &styles, "target"), property, [200.0, 100.0], [12.0, 18.0]);
            assert_eq!(at(&doc, &styles, "target").width, Length::Px(17.0));
            assert_used(at(&doc, &styles, "after"), property, [200.0, 100.0], [3.0, 4.0]);
        }
    }
}

#[test]
fn image_placement_malformed_positions_do_not_publish_partial_axes() {
    for property in POSITIONS {
        for value in [
            "left right", "top bottom", "top 4px", "25% left", "left 1px right 2px",
            "center 4px top", "left center 4px", "left 2px bottom center",
            "left 1px top 2px 3px", "left, top", "left / top", "x-start top",
            "inherit top", "left initial", "auto", "1px 2 px",
        ] {
            let doc = document(&format!("#target{{{property}:12px 18px;{property}:{value};height:19px}}"), "<div id=target></div>");
            let styles = compute(&doc, &HashMap::new());
            assert_used(at(&doc, &styles, "target"), property, [200.0, 100.0], [12.0, 18.0]);
            assert_eq!(at(&doc, &styles, "target").height, Length::Px(19.0));
        }
    }
}

#[test]
fn image_placement_numeric_tokens_are_ascii_strict_and_reject_nonzero_unitless_lengths() {
    for property in POSITIONS {
        let doc = document(&format!("html{{font-size:10px}}#target{{{property}:CaLc(-2PX + 1e1px) -.5REM}}"), "<div id=target></div>");
        assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), property, [200.0, 100.0], [8.0, -5.0]);
        for value in ["2", "-2", "1e1", ".5", "1.px", "1.e2px", "1e+px", "+px", "NaNpx", "infpx", "1e999px", "١px", "1\u{a0}px", "2 px", "0 px"] {
            let doc = document(&format!("#target{{{property}:12px 18px;{property}:{value}}}"), "<div id=target></div>");
            assert_used(at(&doc, &compute(&doc, &HashMap::new()), "target"), property, [200.0, 100.0], [12.0, 18.0]);
        }
    }
}

#[test]
fn image_placement_specificity_source_order_and_inline_importance_apply_to_each_property() {
    for property in PROPERTIES {
        let doc = document(
            &format!("#parent{{{PARENT}}}.seed{{{CHILD}}}#specific{{{property}:inherit}}#order{{{property}:initial;{property}:var(--missing,inherit)}}#important{{{property}:var(--missing,inherit)!important}}#inline{{{property}:initial!important}}"),
            &format!("<div id=parent><div id=specific class=seed></div><div id=order class=seed></div><div id=important class=seed style='{property}:initial'></div><div id=inline class=seed style='{property}:var(--missing,inherit)!important'></div></div>"),
        );
        let styles = compute(&doc, &HashMap::new());
        let mut expected = child_values();
        replace_property(&mut expected, parent_values(), property);
        for name in ["specific", "order", "important", "inline"] {
            assert_eq!(snapshot(at(&doc, &styles, name)), expected, "{property}/{name}");
        }
    }
}

#[test]
fn image_placement_external_source_changes_invalidate_cached_positions_and_keywords() {
    let doc = crate::html::parse("<!doctype html><html><head><link id=sheet rel=stylesheet></head><body><div id=parent><div id=child style='font-size:70px;object-position:inherit;background-position:inherit;object-fit:inherit;image-rendering:inherit;background-repeat:inherit;background-size:inherit'></div></div></body></html>");
    let sheet = id(&doc, "sheet");
    let mut external = HashMap::from([(sheet, "#parent{font-size:10px;object-position:2em 25%;background-position:top 3px right;object-fit:cover;image-rendering:pixelated;background-repeat:repeat-x;background-size:cover}".into())]);
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_used(at(&doc, &first.styles, "child"), "object-position", [200.0, 100.0], [20.0, 25.0]);
    assert_used(at(&doc, &first.styles, "child"), "background-position", [200.0, 100.0], [200.0, 3.0]);
    external.insert(sheet, "#parent{font-size:12px;object-position:3em 75%;background-position:bottom 5px left;object-fit:contain;image-rendering:smooth;background-repeat:repeat-y;background-size:contain}".into());
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    let child = at(&doc, &next.styles, "child");
    assert_used(child, "object-position", [200.0, 100.0], [36.0, 75.0]);
    assert_used(child, "background-position", [200.0, 100.0], [0.0, 95.0]);
    assert_eq!((child.object_fit, child.image_rendering, child.background_repeat, child.background_size),
        (ObjectFit::Contain, ImageRendering::Smooth, [false, true], BackgroundSize::Contain));
    assert_eq!(snapshot(child), snapshot(at(&doc, &fresh, "child")));
    assert!(!first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn image_placement_inline_parent_changes_reuse_rules_and_recompute_inherited_font_offsets() {
    let mut doc = document(
        "html{font-size:10px}#child{font-size:70px;object-position:var(--missing,inherit);background-position:inherit}",
        "<div id=parent style='font-size:10px;object-position:2em calc(25% + 1rem);background-position:right calc(1em + 10%) bottom 2rem'><div id=child></div></div>",
    );
    let external = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_used(at(&doc, &first.styles, "child"), "object-position", [200.0, 100.0], [20.0, 35.0]);
    assert_used(at(&doc, &first.styles, "child"), "background-position", [200.0, 100.0], [170.0, 80.0]);
    let parent = id(&doc, "parent");
    set_style(&mut doc, parent, "font-size:12px;object-position:5em calc(50% + 2rem);background-position:right calc(2em + 20%) bottom 3rem");
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    let fresh = compute(&doc, &external);
    assert_used(at(&doc, &next.styles, "child"), "object-position", [200.0, 100.0], [60.0, 70.0]);
    assert_used(at(&doc, &next.styles, "child"), "background-position", [200.0, 100.0], [136.0, 70.0]);
    assert_eq!(snapshot(at(&doc, &next.styles, "child")), snapshot(at(&doc, &fresh, "child")));
    assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn image_placement_work_and_copy_refusals_preserve_sources_and_allow_fresh_recovery() {
    let doc = document(
        "html{font-size:10px}#parent{font-size:20px;object-position:right calc(25% + 2em) bottom 3rem;background-position:calc(10% + 1em) 2rem;image-rendering:pixelated}#child{font-size:70px;object-position:var(--missing,inherit);background-position:var(--missing,inherit);object-fit:var(--missing,cover);background-repeat:var(--missing,repeat-x);background-size:var(--missing,contain);image-rendering:var(--missing,unset)}",
        "<div id=parent><div id=child></div></div>",
    );
    let external = HashMap::new();
    let before = format!("{:?}", doc.nodes);
    let prior = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    for (mut work, mut copies) in [(0, 1_000_000), (1_000_000, 0)] {
        assert!(compute_with_environment_and_properties_budgeted_cached(&doc, &external, None, None,
            &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).is_err());
        assert_eq!(format!("{:?}", doc.nodes), before);
    }
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let recovered = compute_with_environment_and_properties_budgeted_cached(&doc, &external, None, None,
        &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
    let child = at(&doc, &recovered.styles, "child");
    assert_used(child, "object-position", [200.0, 100.0], [110.0, 70.0]);
    assert_used(child, "background-position", [200.0, 100.0], [40.0, 20.0]);
    assert_eq!((child.object_fit, child.image_rendering, child.background_repeat, child.background_size),
        (ObjectFit::Cover, ImageRendering::Pixelated, [true, false], BackgroundSize::Contain));
    assert_eq!(format!("{:?}", doc.nodes), before);
    assert!(work < 1_000_000 && copies < 1_000_000);
    assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn image_placement_inline_names_use_the_same_property_specific_position_and_keyword_grammar() {
    for property in PROPERTIES {
        for value in ["inherit", "initial", "unset", "var(--chosen)"] {
            assert_names(&format!("{property}:{value}"), &[property]);
        }
    }
    for (property, value) in VALUES { assert_names(&format!("{property}:{value}"), &[property]); }
    for property in POSITIONS {
        for value in ["calc(25% + 2px) -3vw", "right calc(10% + 1em) bottom 2vh", "0", "-0 +0"] {
            assert_names(&format!("{property}:{value}"), &[property]);
        }
    }
    assert_names("background-position:top 4px right", &["background-position"]);
    for (property, invalid) in [
        ("object-position", "top 4px right"), ("background-position", "left center 4px"),
        ("object-fit", "cover none"), ("image-rendering", "nearest-neighbor"),
        ("background-repeat", "repeat no-repeat"), ("background-size", "32px auto"),
    ] { assert_names(&format!("{property}:{invalid}"), &[]); }
    for property in POSITIONS {
        for invalid in ["2", "1.px", "1.e2px", "2 px", "NaNpx", "calc(1px * 2px)", "calc(20px / 0)"] {
            assert_names(&format!("{property}:{invalid}"), &[]);
            assert_names(&format!("{property}:left;object-fit:cover;{property}:{invalid}"), &[property, "object-fit"]);
        }
    }
    assert_names("object-position:left;object-fit:cover;object-position:calc(2px + 10%)", &["object-fit", "object-position"]);
}

#[test]
fn image_placement_inline_text_and_name_apis_preserve_valid_entries_and_refuse_exhausted_budgets() {
    let source = "object-position:calc(25% + 3px) bottom;background-position:top 4px right;background-repeat:var(--r,repeat-x);background-size:cover;object-fit:scale-down;image-rendering:optimizespeed;object-position:1.px;background-position:left center 4px;object-fit:cover none;image-rendering:nearest-neighbor;background-repeat:round;background-size:32px auto";
    for names in [true, false] {
        for (mut work, mut copies) in [(0, 1_000_000), (1_000_000, 0)] {
            let mut budget = CssBudget::new(&mut work, &mut copies);
            let refused = if names { inline_declaration_names(source, &PROPERTIES, Some(&mut budget)).is_err() }
                else { inline_text_declarations(source, &PROPERTIES, &mut budget).is_err() };
            assert!(refused);
        }
    }
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let mut budget = CssBudget::new(&mut work, &mut copies);
    let names = inline_declaration_names(source, &PROPERTIES, Some(&mut budget)).unwrap();
    assert_eq!(names.iter().map(String::as_str).collect::<Vec<_>>(), vec!["object-position", "background-position", "background-repeat", "background-size", "object-fit", "image-rendering"]);
    let entries = inline_text_declarations(source, &PROPERTIES, &mut budget).unwrap();
    let expected = [
        ("object-position", "calc(25% + 3px) bottom"), ("background-position", "top 4px right"),
        ("background-repeat", "var(--r,repeat-x)"), ("background-size", "cover"),
        ("object-fit", "scale-down"), ("image-rendering", "optimizespeed"),
    ];
    assert_eq!(entries, expected.map(|(name, value)| (name.to_owned(), value.to_owned(), false)).to_vec());
    assert!(work < 1_000_000 && copies < 1_000_000);
}
