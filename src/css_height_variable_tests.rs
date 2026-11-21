//! Original height-consumer regressions; the independent App probe covers pixels.
use super::*;

fn styles(css: &str, body: &str) -> (Document, Vec<Style>) {
    let document = crate::html::parse(&format!(
        "<html><head><style>{css}</style></head><body>{body}</body></html>"
    ));
    let computed = compute(&document, &HashMap::new());
    (document, computed)
}

fn at<'a>(document: &Document, computed: &'a [Style], id: &str) -> &'a Style {
    &computed[(0..document.nodes.len())
        .find(|&node| document.attr(node, "id") == Some(id))
        .unwrap()]
}

#[test]
fn height_variable_supported_lengths_keywords_and_cascade() {
    let (document, computed) = styles(
        "html{font-size:20px}#parent{font-size:12px;height:40px;--Height:24px;--height:32px}\
         #px{height:var(--Height)}#case{height:var(--height)}\
         #pt{--h:18pt;height:var(--h)}#pc{--h:1.5pc;height:var(--h)}\
         #em{--h:2em;height:var(--h)}#rem{--h:1.5rem;height:var(--h)}\
         #percent{--h:50%;height:var(--h)}#zero{--h:0;height:var(--h)}\
         #auto{--h:AuTo;height:var(--h)}#inherit{height:var(--missing,inherit)}\
         #initial{height:8px;height:var(--missing,initial)}#unset{height:8px;height:var(--missing,unset)}\
         #fallback{height:var(--missing,var(--also-missing,2em))}\
         #forward{height:var(--later);--later:5rem}\
         #important{height:var(--Height)!important;height:9px}\
         #inline{height:var(--Height)!important}\
         #last{height:var(--Height);height:7px}\
         #late{height:var(--h);--h:2em;font-size:30px}\
         #early{font-size:30px;--h:2em;height:var(--h)}",
        "<div id=parent><div id=px></div><div id=case></div><div id=pt></div><div id=pc></div>\
         <div id=em></div><div id=rem></div><div id=percent></div><div id=zero></div><div id=auto></div>\
         <div id=inherit></div><div id=initial></div><div id=unset></div><div id=fallback></div><div id=forward></div>\
         <div id=important style='height:99px'></div><div id=inline style='height:3px!important'></div>\
         <div id=last></div><div id=late></div><div id=early></div></div>",
    );
    for (name, expected) in [
        ("px", Length::Px(24.0)),
        ("case", Length::Px(32.0)),
        ("pt", Length::Px(24.0)),
        ("pc", Length::Px(24.0)),
        ("em", Length::Px(24.0)),
        ("rem", Length::Px(30.0)),
        ("percent", Length::Percent(50.0)),
        ("zero", Length::Px(0.0)),
        ("auto", Length::Auto),
        ("inherit", Length::Px(40.0)),
        ("initial", Length::Auto),
        ("unset", Length::Auto),
        ("fallback", Length::Px(24.0)),
        ("forward", Length::Px(100.0)),
        ("important", Length::Px(24.0)),
        ("inline", Length::Px(3.0)),
        ("last", Length::Px(7.0)),
        ("late", Length::Px(60.0)),
        ("early", Length::Px(60.0)),
    ] {
        assert_eq!(at(&document, &computed, name).height, expected, "{name}");
    }
}

#[test]
fn height_variable_invalid_computed_values_do_not_revive_an_earlier_height() {
    for invalid in [
        "-1px",
        "2 px",
        "2\u{a0}px",
        "'2px'",
        "2px 3px",
        "2/3",
        "none",
        "normal",
        "NaN",
        "Infinity",
        "calc(1px + 2)",
        "1ch",
    ] {
        let (document, computed) = styles(
            &format!(
                "#parent{{height:70px}}#literal{{height:33px;height:{invalid}}}\
                      #variable{{--h:{invalid};height:33px;height:var(--h)}}"
            ),
            "<div id=parent><div id=literal></div><div id=variable></div></div>",
        );
        assert_eq!(
            at(&document, &computed, "literal").height,
            Length::Px(33.0),
            "literal {invalid}"
        );
        assert_eq!(
            at(&document, &computed, "variable").height,
            Length::Auto,
            "variable {invalid}"
        );
    }
    let (document, computed) = styles(
        "html{font-size:20px}#parent{height:70px}\
         #missing{height:33px;height:var(--missing)}\
         #cycle{--h:var(--h);height:33px;height:var(--h)}\
         #fallback{--a:var(--b);--b:var(--a);height:var(--a,2rem)}\
         #joined{--number:2;height:33px;height:var(--number)px}\
         #joined-percent{--number:50;height:33px;height:var(--number)%}\
         #empty{--blank:;height:33px;height:var(--blank,2rem)}\
         #empty-fallback{height:33px;height:var(--missing,)}\
         #malformed{height:33px;height:var(not-custom)}\
         #literal-initial{height:33px;height:initial}#literal-unset{height:33px;height:unset}",
        "<div id=parent><div id=missing></div><div id=cycle></div><div id=fallback></div>\
         <div id=joined></div><div id=joined-percent></div><div id=empty></div><div id=empty-fallback></div>\
         <div id=malformed></div><div id=literal-initial></div><div id=literal-unset></div></div>",
    );
    for name in [
        "missing",
        "cycle",
        "joined",
        "joined-percent",
        "empty",
        "empty-fallback",
        "literal-initial",
        "literal-unset",
    ] {
        assert_eq!(
            at(&document, &computed, name).height,
            Length::Auto,
            "{name}"
        );
    }
    assert_eq!(
        at(&document, &computed, "fallback").height,
        Length::Px(40.0)
    );
    assert_eq!(
        at(&document, &computed, "malformed").height,
        Length::Px(33.0)
    );
}

#[test]
fn height_variable_inheritance_uses_computed_lengths_but_preserves_percent_and_auto() {
    let (document, computed) = styles(
        "html{font-size:20px;height:1rem}.parent{font-size:12px}.child{font-size:30px}\
         #em{--h:2em;height:var(--h)}#em-inherit{height:inherit}\
         #em-variable{height:var(--h)}#rem{height:1.5rem}#rem-inherit{height:var(--missing,inherit)}\
         #percent{height:50%}#percent-inherit{height:inherit}\
         #auto{height:auto}#auto-inherit{height:inherit}",
        "<div id=em class=parent><div id=em-inherit class=child></div><div id=em-variable class=child></div><div id=unassigned></div></div>\
         <div id=rem class=parent><div id=rem-inherit class=child></div></div>\
         <div id=percent><div id=percent-inherit></div></div><div id=auto><div id=auto-inherit></div></div>",
    );
    let root = document
        .nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{tag,..} if tag=="html"))
        .unwrap();
    assert_eq!(computed[root].height, Length::Px(20.0));
    for (name, expected) in [
        ("em", Length::Px(24.0)),
        ("em-inherit", Length::Px(24.0)),
        ("em-variable", Length::Px(60.0)),
        ("unassigned", Length::Auto),
        ("rem", Length::Px(30.0)),
        ("rem-inherit", Length::Px(30.0)),
        ("percent", Length::Percent(50.0)),
        ("percent-inherit", Length::Percent(50.0)),
        ("auto", Length::Auto),
        ("auto-inherit", Length::Auto),
    ] {
        assert_eq!(at(&document, &computed, name).height, expected, "{name}");
    }
}

#[test]
fn height_variable_shares_one_substitution_budget_and_preserves_other_dimensions() {
    let declarations = parse_declarations("--h:2rem");
    let variables = custom_properties::Variables::cascade(
        &custom_properties::Variables::default(),
        declarations.iter(),
        &mut custom_properties::Budget::default(),
    );
    let declaration = Declaration {
        name: "height".into(),
        value: "var(--h)".into(),
        important: false,
        stylesheet: None,
    };
    let mut style = Style::default();
    style.height = Length::Px(9.0);
    style.width = Length::Percent(25.0);
    style.min_width = Length::Px(8.0);
    style.max_width = Some(Length::Px(180.0));
    style.min_height = Length::Px(6.0);
    style.max_height = Some(Length::Px(100.0));
    let mut parent = Style::default();
    parent.height = Length::Px(70.0);
    let mut actual = custom_properties::Budget::default();
    let mut expected = custom_properties::Budget::default();
    assert!(variables
        .substitute(&declaration.value, &mut expected)
        .unwrap()
        .is_some());
    assert!(custom_properties::apply_box(
        &mut style,
        &parent,
        &declaration,
        &variables,
        &mut actual
    ));
    assert_eq!(
        actual.usage(),
        expected.usage(),
        "Exactly one shared-budget substitution"
    );
    assert_eq!(style.height, Length::Rem(2.0));
    let mut exhausted = custom_properties::Budget::default().operation(0, 0);
    assert!(custom_properties::apply_box(
        &mut style,
        &parent,
        &declaration,
        &variables,
        &mut exhausted
    ));
    assert_eq!(
        style.height,
        Length::Auto,
        "Refusal uses noninherited unset, not parent or previous height"
    );
    assert_eq!(
        (
            style.width,
            style.min_width,
            style.max_width,
            style.min_height,
            style.max_height
        ),
        (
            Length::Percent(25.0),
            Length::Px(8.0),
            Some(Length::Px(180.0)),
            Length::Px(6.0),
            Some(Length::Px(100.0))
        )
    );

    let (document, computed) = styles(
        "#box{--h:1rem;width:25%;min-width:8px;max-width:180px;min-height:6px;max-height:100px;height:var(--h)}",
        "<div id=box></div>",
    );
    let style = at(&document, &computed, "box");
    assert_eq!(style.height, Length::Px(16.0));
    assert_eq!(
        (
            style.width,
            style.min_width,
            style.max_width,
            style.min_height,
            style.max_height
        ),
        (
            Length::Percent(25.0),
            Length::Px(8.0),
            Some(Length::Px(180.0)),
            Length::Px(6.0),
            Some(Length::Px(100.0))
        )
    );
}
