use super::*;

fn styles(css: &str, body: &str) -> (Document, Vec<Style>) {
    let doc = crate::html::parse(&format!(
        "<html><head><style>{css}</style></head><body>{body}</body></html>"
    ));
    let styles = compute(&doc, &HashMap::new());
    (doc, styles)
}
fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style {
    &styles[(0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()]
}
fn factor(style: &Style, expected: f32) {
    assert!(
        matches!(style.line_height, LineHeight::Factor(n) if n == expected),
        "{:?}",
        style.line_height
    );
}
fn pixels(style: &Style, expected: f32) {
    assert!(
        matches!(style.line_height, LineHeight::Length(Length::Px(n)) if (n-expected).abs()<0.001),
        "{:?}",
        style.line_height
    );
}
fn used(style: &Style) -> f32 {
    match style.line_height {
        LineHeight::Factor(n) => n * style.font_size,
        LineHeight::Length(length) => length
            .resolve(style.font_size, style.font_size, style.root_font_size)
            .unwrap(),
    }
}

#[test]
fn line_height_variable_cascade_keywords_case_and_invalid_computed_values() {
    let (doc, computed) = styles(
        "#p{font-size:20px;line-height:1.25;--Leading:2;--leading:1.75}\
         #a{line-height:var(--Leading)}#b{line-height:var(--leading)}\
         #forward{line-height:var(--later);--later:3}\
         #priority{line-height:var(--Leading)!important;line-height:4}\
         #missing{line-height:3;line-height:var(--missing)}\
         #invalid{--bad:2px 3px;line-height:3;line-height:var(--bad)}\
         #empty{--blank:;line-height:3;line-height:var(--blank,2)}\
         #inherit{line-height:3;line-height:var(--missing,inherit)}\
         #unset{line-height:3;line-height:var(--missing,unset)}\
         #initial{line-height:3;line-height:var(--missing,initial)}\
         #normal{line-height:var(--missing,NoRmAl)}\
         #literal{line-height:3;line-height:bogus}\
         #malformed{line-height:3;line-height:var(not-custom)}",
        "<div id=p><div id=a></div><div id=b></div><div id=forward></div><div id=priority style='line-height:5'></div>\
         <div id=missing></div><div id=invalid></div><div id=empty></div><div id=inherit></div><div id=unset></div>\
         <div id=initial></div><div id=normal></div><div id=literal></div><div id=malformed></div></div>",
    );
    for (name, value) in [
        ("a", 2.0),
        ("b", 1.75),
        ("forward", 3.0),
        ("priority", 2.0),
        ("missing", 1.25),
        ("invalid", 1.25),
        ("empty", 1.25),
        ("inherit", 1.25),
        ("unset", 1.25),
        ("initial", 1.5),
        ("normal", 1.5),
        ("literal", 3.0),
        ("malformed", 3.0),
    ] {
        factor(at(&doc, &computed, name), value);
    }
    let (doc, computed) = styles(
        "#p{line-height:2}#a{line-height:3;line-height:unset}#b{line-height:3;line-height:initial}",
        "<div id=p><div id=a></div><div id=b></div></div>",
    );
    factor(at(&doc, &computed, "a"), 2.0);
    factor(at(&doc, &computed, "b"), 1.5);
}

#[test]
fn line_height_variable_computed_lengths_and_unitless_inheritance_use_final_fonts() {
    let (doc, computed) = styles(
        "html{font-size:20px;line-height:var(--root);--root:2rem}\
         .parent{font-size:12px} .child{font-size:18px}\
         #factor{--h:2;line-height:var(--h)}\
         #em{--h:2em;line-height:var(--h)}\
         #percent{--h:200%;line-height:var(--h)}\
         #rem{--h:1.5rem;line-height:var(--h)}\
         #late{line-height:var(--h);--h:2em;font-size:30px}\
         #early{font-size:30px;line-height:2em}",
        "<div id=factor class=parent><span id=factor-child class=child></span></div>\
         <div id=em class=parent><span id=em-child class=child></span></div>\
         <div id=percent class=parent><span id=percent-child class=child></span></div>\
         <div id=rem class=parent><span id=rem-child class=child></span></div>\
         <div id=late></div><div id=early></div>",
    );
    let root = doc
        .nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{tag,..} if tag=="html"))
        .unwrap();
    pixels(&computed[root], 40.0);
    factor(at(&doc, &computed, "factor"), 2.0);
    factor(at(&doc, &computed, "factor-child"), 2.0);
    assert_eq!(used(at(&doc, &computed, "factor")), 24.0);
    assert_eq!(used(at(&doc, &computed, "factor-child")), 36.0);
    for name in ["em", "em-child", "percent", "percent-child"] {
        pixels(at(&doc, &computed, name), 24.0);
    }
    for name in ["rem", "rem-child"] {
        pixels(at(&doc, &computed, name), 30.0);
    }
    for name in ["late", "early"] {
        pixels(at(&doc, &computed, name), 60.0);
    }
    for (value, expected) in [
        ("24px", 24.0),
        ("18pt", 24.0),
        ("1.5pc", 24.0),
        ("2em", 24.0),
        ("200%", 24.0),
        ("1.2rem", 24.0),
    ] {
        let (doc, computed) = styles(&format!("html{{font-size:20px}}#p{{font-size:12px}}#a{{--h:{value};line-height:var(--h)}}#b{{line-height:{value}}}"),
            "<div id=p><div id=a></div><div id=b></div></div>");
        pixels(at(&doc, &computed, "a"), expected);
        pixels(at(&doc, &computed, "b"), expected);
    }
}

#[test]
fn line_height_variable_cycles_tokens_and_existing_finite_bounds() {
    for unsupported in [
        "-1",
        "-1px",
        "2 px",
        "2\u{a0}px",
        "'2'",
        "2/3",
        "2px 3px",
        "calc(1px + 2px)",
        "1vh",
        "auto",
        "NaN",
        "Infinity",
    ] {
        let (doc, computed) = styles(&format!("#p{{line-height:1.25}}#literal{{line-height:3;line-height:{unsupported}}}#variable{{--h:{unsupported};line-height:3;line-height:var(--h)}}"),
            "<div id=p><div id=literal></div><div id=variable></div></div>");
        factor(at(&doc, &computed, "literal"), 3.0);
        factor(at(&doc, &computed, "variable"), 1.25);
    }
    let (doc, computed) = styles(
        "#p{line-height:1.25}#cycle{--h:var(--h);line-height:3;line-height:var(--h)}\
         #fallback{--a:var(--b);--b:var(--a);line-height:var(--a,2)}\
         #joined{--n:2;line-height:3;line-height:var(--n)px}\
         #zero{--h:0;line-height:var(--h)}#cap{--h:99;line-height:var(--h)}",
        "<div id=p><div id=cycle></div><div id=fallback></div><div id=joined></div><div id=zero></div><div id=cap></div></div>");
    for (name, n) in [
        ("cycle", 1.25),
        ("fallback", 2.0),
        ("joined", 1.25),
        ("zero", 0.0),
        ("cap", 20.0),
    ] {
        factor(at(&doc, &computed, name), n);
    }
}

#[test]
fn line_height_variable_font_shorthand_and_sheet_rollback_share_one_order() {
    let (doc, computed) = styles(
        "#p{line-height:1.25;--h:3}\
         #a{line-height:var(--h);font:italic 700 12px/2 Geometry}\
         #b{font:italic 700 12px/2 Geometry;line-height:var(--h)}\
         #important{line-height:var(--h)!important;font:12px/2 Geometry}\
         #rollback{font:12px/2 Geometry}\
         #priority{font:12px/2 Geometry!important}\
         #invalid-font{line-height:2;font:bogus!important}\
         #revert{line-height:2}\
         #sheet-revert{line-height:2;line-height:revert-layer}\
         #wide{font:12px/2 Geometry;font:initial}\
         #whole{font:12px/2 Geometry;--font:24px/4 Geometry;font:var(--font);line-height:var(--h)}",
        "<div id=p><div id=a></div><div id=b></div><div id=important style='line-height:5'></div>\
         <div id=rollback style='font:30px/4 Geometry;line-height:revert-layer'></div>\
         <div id=priority style='font:30px/4 Geometry!important;line-height:var(--missing,revert-layer)!important'></div>\
         <div id=invalid-font style='line-height:3;line-height:revert-layer!important'></div>\
         <div id=revert style='line-height:4;line-height:revert'></div><div id=sheet-revert></div><div id=wide></div><div id=whole></div></div>");
    for (name, n) in [
        ("a", 2.0),
        ("b", 3.0),
        ("important", 3.0),
        ("rollback", 2.0),
        ("priority", 2.0),
        ("invalid-font", 2.0),
        ("revert", 1.25),
        ("sheet-revert", 1.25),
        ("wide", 1.5),
        ("whole", 3.0),
    ] {
        factor(at(&doc, &computed, name), n);
    }
    for name in ["a", "b"] {
        let style = at(&doc, &computed, name);
        assert!(style.bold && style.italic);
        assert_eq!(style.font_size, 12.0);
        assert_eq!(style.font_families, vec!["Geometry".to_owned()]);
    }
    for name in ["rollback", "priority"] {
        assert_eq!(at(&doc, &computed, name).font_size, 30.0);
    }
    assert_eq!(
        at(&doc, &computed, "whole").font_size,
        24.0,
        "Variable font shorthand supplies the size before the later line-height override"
    );
}

#[test]
fn line_height_variable_refused_substitution_inherits_without_reviving_previous_value() {
    let mut style = Style::default();
    style.line_height = LineHeight::Factor(3.0);
    style.font_size = 24.0;
    style.color = 0x123456;
    let mut parent = Style::default();
    parent.line_height = LineHeight::Length(Length::Px(28.0));
    let mut sheet = LineHeight::Factor(2.0);
    let declaration = Declaration {
        name: "line-height".into(),
        value: "var(--missing,2em)".into(),
        important: false,
        stylesheet: None,
    };
    let mut budget = custom_properties::Budget::default().operation(0, 0);
    assert!(apply(
        &mut style,
        &parent,
        &declaration,
        false,
        &mut sheet,
        &custom_properties::Variables::default(),
        &mut budget
    ));
    pixels(&style, 28.0);
    assert!(matches!(sheet,LineHeight::Length(Length::Px(n)) if n==28.0));
    assert_eq!(style.font_size, 24.0);
    assert_eq!(style.color, 0x123456);
}
