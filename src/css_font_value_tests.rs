//! Finite font values, inherited variable winners and native inline admission.
use super::*;

const PROPERTIES: [&str; 4] = ["font", "font-family", "font-style", "font-weight"];
const LONGHANDS: [&str; 3] = ["font-family", "font-style", "font-weight"];
const PARENT: &str = "font:italic 500 20px/1.75 'Parent Face',serif";
const CHILD: &str = "font:800 27px/2.25 'Child Face',monospace";

#[derive(Clone, Debug, PartialEq)]
enum Leading { Factor(f32), Pixels(f32) }

#[derive(Clone, Debug, PartialEq)]
struct Font {
    families: Vec<String>,
    size: f32,
    weight: u16,
    bold: bool,
    italic: bool,
    leading: Leading,
}

fn font(style: &Style) -> Font {
    let leading = match style.line_height {
        LineHeight::Factor(value) => Leading::Factor(value),
        LineHeight::Length(Length::Px(value)) => Leading::Pixels(value),
        other => panic!("line-height must be computed: {other:?}"),
    };
    Font { families: style.font_families.clone(), size: style.font_size,
        weight: style.font_weight, bold: style.bold, italic: style.italic, leading }
}

fn value(families: &[&str], size: f32, weight: u16, italic: bool, leading: Leading) -> Font {
    Font { families: families.iter().map(|name| (*name).into()).collect(), size,
        weight, bold: weight >= 600, italic, leading }
}

fn initial() -> Font { value(&["serif"], 16.0, 400, false, Leading::Factor(1.5)) }
fn parent() -> Font { value(&["Parent Face", "serif"], 20.0, 500, true, Leading::Factor(1.75)) }
fn child() -> Font { value(&["Child Face", "monospace"], 27.0, 800, false, Leading::Factor(2.25)) }
fn chosen() -> Font { value(&["Chosen, Face", "sans-serif"], 24.0, 600, true, Leading::Factor(2.0)) }

fn chosen_value(property: &str) -> &'static str {
    match property {
        "font" => "oblique 600 24px/2 'Chosen, Face',sans-serif",
        "font-family" => "'Chosen, Face',sans-serif",
        "font-style" => "oblique",
        "font-weight" => "600",
        _ => panic!("unknown font property"),
    }
}

fn invalid_value(property: &str) -> &'static str {
    match property {
        "font" => "italic 24px/2/3 Broken",
        "font-family" => "12",
        "font-style" => "italic normal",
        "font-weight" => "1001",
        _ => panic!("unknown font property"),
    }
}

fn replace(target: &mut Font, source: &Font, property: &str) {
    match property {
        "font" => *target = source.clone(),
        "font-family" => target.families = source.families.clone(),
        "font-weight" => { target.weight = source.weight; target.bold = source.bold; }
        "font-style" => target.italic = source.italic,
        _ => panic!("unknown font property"),
    }
}

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style { &styles[id(doc, name)] }

fn set_attribute(doc: &mut Document, node: usize, name: &str, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { panic!("element required") };
    if let Some(attribute) = attrs.iter_mut().find(|attribute| attribute.name.html_is(name)) {
        attribute.value = value.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified(name.into(), value.into()));
    }
}

fn assert_target(property: &str, declarations: &str, expected: Font) {
    let doc = document(&format!("#parent{{{PARENT}}}#target{{{CHILD};{declarations}}}"),
        "<div id=parent><div id=target></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(font(at(&doc, &styles, "parent")), parent());
    assert_eq!(font(at(&doc, &styles, "target")), expected, "{property}: {declarations}");
}

fn names(source: &str, expected: &[&str]) {
    assert_eq!(inline_declaration_names(source, &LONGHANDS, None).unwrap(),
        expected.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>(), "{source}");
}

fn run_cached(doc: &Document, transform: bool, budget: &mut CssBudget<'_>,
    cache: Option<&StylesheetCache>) -> Result<CascadeOutput, String> {
    if transform {
        compute_with_environment_and_properties_transform_budgeted_cached(doc, &HashMap::new(), None, None, budget, cache)
    } else {
        compute_with_environment_and_properties_budgeted_cached(doc, &HashMap::new(), None, None, budget, cache)
    }
}

#[test]
fn font_family_literals_and_variables_keep_case_quotes_commas_and_escapes() {
    for (source, expected) in [
        ("'Display Face', Times New Roman, serif", vec!["Display Face", "Times New Roman", "serif"]),
        ("'Quoted, Face',monospace", vec!["Quoted, Face", "monospace"]),
        ("'var(--missing)',serif", vec!["var(--missing)", "serif"]),
        (r#"'Escaped \46 ace',sans-serif"#, vec!["Escaped Face", "sans-serif"]),
        ("Café Display, serif", vec!["Café Display", "serif"]),
    ] {
        for variable in [false, true] {
            let declarations = if variable { format!("--Family:{source};font-family:var(--Family)") }
                else { format!("font-family:{source}") };
            let mut expected_font = child(); expected_font.families = expected.iter().map(|name| (*name).into()).collect();
            assert_target("font-family", &declarations, expected_font);
        }
    }
}

#[test]
fn font_weight_integer_range_and_keywords_share_literal_and_variable_values() {
    for (source, weight) in [("1", 1), ("99", 99), ("100", 100), ("349", 349), ("350", 350),
        ("549", 549), ("550", 550), ("599", 599), ("600", 600), ("749", 749), ("750", 750),
        ("899", 899), ("900", 900), ("901", 901), ("999", 999), ("1000", 1000), ("normal", 400), ("BoLd", 700)] {
        for variable in [false, true] {
            let declarations = if variable { format!("--Weight:{source};font-weight:var(--Weight)") }
                else { format!("font-weight:{source}") };
            let mut expected = child(); expected.weight = weight; expected.bold = weight >= 600;
            assert_target("font-weight", &declarations, expected);
        }
    }
}

#[test]
fn font_style_finite_keywords_share_literal_and_variable_values() {
    for (source, italic) in [("normal", false), ("ITALIC", true), ("ObLiQuE", true)] {
        for variable in [false, true] {
            let declarations = if variable { format!("--Style:{source};font-style:var(--Style)") }
                else { format!("font-style:{source}") };
            let mut expected = child(); expected.italic = italic;
            assert_target("font-style", &declarations, expected);
        }
    }
}

#[test]
fn font_shorthand_literals_and_variables_compute_all_represented_components() {
    for (source, expected) in [
        ("italic 600 18px/1.25 'Whole Face',serif", value(&["Whole Face", "serif"], 18.0, 600, true, Leading::Factor(1.25))),
        ("bold oblique 12pt/24px CaseFace", value(&["CaseFace"], 16.0, 700, true, Leading::Pixels(24.0))),
        ("1.5em/150% 'Mixed Case',sans-serif", value(&["Mixed Case", "sans-serif"], 30.0, 400, false, Leading::Pixels(45.0))),
        ("normal 2pc/normal Narrow", value(&["Narrow"], 32.0, 400, false, Leading::Factor(1.5))),
    ] {
        for variable in [false, true] {
            let declarations = if variable { format!("--Font:{source};font:var(--Font)") }
                else { format!("font:{source}") };
            assert_target("font", &declarations, expected.clone());
        }
    }
}

#[test]
fn font_shorthand_omitted_options_reset_prior_weight_style_and_line_height() {
    for source in ["font:24px ResetFace", "--Font:24px ResetFace;font:var(--Font)"] {
        assert_target("font", source, value(&["ResetFace"], 24.0, 400, false, Leading::Factor(1.5)));
    }
}

#[test]
fn font_values_naturally_inherit_completed_parent_components() {
    let doc = document(&format!("#parent{{{PARENT}}}"), "<div id=parent><div id=child><span id=leaf></span></div></div>");
    let styles = compute(&doc, &HashMap::new());
    for name in ["parent", "child", "leaf"] { assert_eq!(font(at(&doc, &styles, name)), parent(), "{name}"); }
}

#[test]
fn font_literal_wide_keywords_change_only_the_selected_components() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            let source = if keyword == "initial" { initial() } else { parent() };
            let mut expected = child(); replace(&mut expected, &source, property);
            assert_target(property, &format!("{property}:{keyword}"), expected);
        }
    }
}

#[test]
fn font_variable_wide_keywords_use_inherited_unset_and_initial_defaults() {
    for property in PROPERTIES {
        for keyword in ["InHeRiT", "INITIAL", "UnSeT"] {
            let source = if keyword == "INITIAL" { initial() } else { parent() };
            let mut expected = child(); replace(&mut expected, &source, property);
            assert_target(property, &format!("{property}:var(--missing,{keyword})"), expected);
        }
    }
}

#[test]
fn font_root_literal_wide_keywords_ignore_the_document_container() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            let mut doc = crate::html::parse(&format!(r#"<html id=root style="{CHILD};{property}:{keyword}"><head></head><body></body></html>"#));
            set_attribute(&mut doc, 0, "style", PARENT);
            let mut expected = child(); replace(&mut expected, &initial(), property);
            assert_eq!(font(at(&doc, &compute(&doc, &HashMap::new()), "root")), expected, "{property}:{keyword}");
        }
    }
}

#[test]
fn font_root_variable_wide_keywords_ignore_the_document_container() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            let mut doc = crate::html::parse(&format!(r#"<html id=root style="{CHILD};{property}:var(--missing,{keyword})"><head></head><body></body></html>"#));
            set_attribute(&mut doc, 0, "style", PARENT);
            let mut expected = child(); replace(&mut expected, &initial(), property);
            assert_eq!(font(at(&doc, &compute(&doc, &HashMap::new()), "root")), expected, "{property}:{keyword}");
        }
    }
}

#[test]
fn font_root_missing_and_invalid_variable_winners_use_initial_components() {
    for property in PROPERTIES {
        for source in ["var(--missing)".to_owned(), format!("var(--missing,{})", invalid_value(property))] {
            let mut doc = crate::html::parse(&format!(r#"<html id=root style="{CHILD};{property}:{source}"><head></head><body></body></html>"#));
            set_attribute(&mut doc, 0, "style", PARENT);
            let mut expected = child(); replace(&mut expected, &initial(), property);
            assert_eq!(font(at(&doc, &compute(&doc, &HashMap::new()), "root")), expected, "{property}:{source}");
        }
    }
}

#[test]
fn font_fragment_context_is_excluded_but_real_null_namespace_parents_inherit() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            for variable in [false, true] {
                let source = if variable { format!("var(--missing,{keyword})") } else { keyword.into() };
                let mut doc = crate::html::parse_fragment(
                    "<link id=sheet rel=stylesheet><section id=top><div id=real><span id=inner></span></div></section>", "div", false).unwrap();
                assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
                let real = id(&doc, "real"); doc.nodes[real].namespace = crate::html::Namespace::None;
                let external = HashMap::from([(id(&doc, "sheet"), format!("div{{{PARENT}}}#top,#inner{{{CHILD};{property}:{source}}}"))]);
                let styles = compute(&doc, &external);
                let mut top = child(); replace(&mut top, &initial(), property);
                let mut inner = child(); replace(&mut inner, &if keyword == "initial" { initial() } else { parent() }, property);
                assert_eq!(font(at(&doc, &styles, "top")), top, "{property}:{source}");
                assert_eq!(font(at(&doc, &styles, "inner")), inner, "{property}:{source}");
            }
        }
    }
}

#[test]
fn font_relative_weight_thresholds_use_the_parents_numeric_weight() {
    for (parent_weight, bolder, lighter) in [(100, 400, 100), (349, 400, 100), (350, 700, 100),
        (549, 700, 100), (550, 900, 400), (749, 900, 400), (750, 900, 700), (899, 900, 700), (900, 900, 700)] {
        for (relative, expected) in [("bolder", bolder), ("lighter", lighter)] {
            for variable in [false, true] {
                let source = if variable { format!("var(--missing,{relative})") } else { relative.into() };
                let doc = document(&format!("#parent{{font-weight:{parent_weight}}}#target{{font-weight:{source}}}"),
                    "<div id=parent><div id=target></div></div>");
                let styles = compute(&doc, &HashMap::new()); let style = at(&doc, &styles, "target");
                assert_eq!((style.font_weight, style.bold), (expected, expected >= 600), "{parent_weight}: {source}");
            }
        }
    }
}

#[test]
fn font_relative_extreme_weights_preserve_values_beyond_the_anchor_weights() {
    for (parent_weight, relative) in [(1, "lighter"), (99, "lighter"), (901, "bolder"), (950, "bolder"), (1000, "bolder")] {
        for property in ["font-weight", "font"] {
            let literal = if property == "font" { format!("{relative} 18px EdgeFace") } else { relative.into() };
            for variable in [false, true] {
                let source = if variable { format!("var(--missing,{literal})") } else { literal.clone() };
                let doc = document(&format!("#parent{{font-weight:{parent_weight}}}#target{{{property}:{source}}}"),
                    "<div id=parent><div id=target></div></div>");
                let styles = compute(&doc, &HashMap::new()); let style = at(&doc, &styles, "target");
                assert_eq!((style.font_weight, style.bold), (parent_weight, parent_weight >= 600), "{property}: {source}");
            }
        }
    }
}

#[test]
fn font_relative_weights_compute_once_before_multigeneration_inheritance() {
    let doc = document(concat!(
        "#grand{font-weight:400;--step:bolder;--all:bolder 18px StepFace}",
        "#parent{font-weight:var(--step)}#inherited{font-weight:var(--missing,inherit)}",
        "#next{font-weight:var(--step)}#whole{font:var(--all)}#leaf{font:var(--missing,inherit)}"),
        "<div id=grand><div id=parent><div id=ordinary></div><div id=inherited></div><div id=next></div><div id=whole><span id=leaf></span></div></div></div>");
    let styles = compute(&doc, &HashMap::new());
    for (name, expected) in [("parent", 700), ("ordinary", 700), ("inherited", 700), ("next", 900), ("whole", 900), ("leaf", 900)] {
        assert_eq!(at(&doc, &styles, name).font_weight, expected, "{name}");
    }
    assert_eq!(font(at(&doc, &styles, "whole")), font(at(&doc, &styles, "leaf")));
}

#[test]
fn font_custom_names_are_case_sensitive_and_inherited_aliases_are_already_resolved() {
    let doc = document(concat!(
        "#parent{--Family:'Upper Face';--family:'lower face';--Alias:var(--Family);--Weight:600;--weight:300}",
        "#upper{font-family:var(--Family);font-weight:var(--Weight)}",
        "#lower{font-family:var(--family);font-weight:var(--weight)}",
        "#alias{--Family:'Local Face';font-family:var(--Alias)}",
        "#forward{font:var(--later);--later:italic 700 22px/2 'Forward Face'}"),
        "<div id=parent><div id=upper></div><div id=lower></div><div id=alias></div><div id=forward></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(at(&doc, &styles, "upper").font_families, vec!["Upper Face"]);
    assert_eq!(at(&doc, &styles, "upper").font_weight, 600);
    assert_eq!(at(&doc, &styles, "lower").font_families, vec!["lower face"]);
    assert_eq!(at(&doc, &styles, "lower").font_weight, 300);
    assert_eq!(at(&doc, &styles, "alias").font_families, vec!["Upper Face"]);
    assert_eq!(font(at(&doc, &styles, "forward")), value(&["Forward Face"], 22.0, 700, true, Leading::Factor(2.0)));
}

#[test]
fn font_nested_fallbacks_comments_and_keyword_case_preserve_family_text() {
    for property in PROPERTIES {
        let source = format!("{property}:VaR(--missing,var(--other,/* before */ {} /* after */))", chosen_value(property));
        let mut expected = child(); replace(&mut expected, &chosen(), property);
        assert_target(property, &source, expected);
    }
    assert_target("font", "--Font:/* start */ ItAlIc BoLd 24PX/2 'MiXeD Face' /* end */;font:var(--Font)",
        value(&["MiXeD Face"], 24.0, 700, true, Leading::Factor(2.0)));
}

#[test]
fn font_longhand_values_outside_the_finite_grammar_preserve_literal_winners() {
    for (property, unsupported) in [
        ("font-family", vec!["12", "Face,", ",Face", "url(Unrequested.ttf)", "'One' 'Two'"]),
        ("font-weight", vec!["0", "1001", "-1", "500.5", "5e2", "600px", "normal bold", "'var(--x)'"]),
        ("font-style", vec!["oblique 12deg", "italic normal", "bold", "'var(--x)'"]),
    ] {
        for source in unsupported { assert_target(property, &format!("{property}:{source}!important"), child()); }
    }
}

#[test]
fn font_malformed_literal_shorthands_leave_every_prior_component_intact() {
    for source in ["italic 600 24px", "24px/2/3 Broken", "italic oblique 24px Broken",
        "italic bold Broken", "banana 24px Broken", "italic -1px Broken"] {
        assert_target("font", &format!("font:{source}!important"), child());
    }
}

#[test]
fn font_malformed_variable_syntax_is_rejected_before_the_cascade() {
    for property in PROPERTIES {
        for source in ["var(not-custom)", "var(--,normal)", "var(--a --b)", "var()", "var (--missing)"] {
            assert_target(property, &format!("{property}:{source}"), child());
        }
    }
}

#[test]
fn font_missing_variable_winners_inherit_without_reviving_previous_values() {
    for property in PROPERTIES {
        for source in ["var(--missing)", "var(--missing,)"] {
            let mut expected = child(); replace(&mut expected, &parent(), property);
            assert_target(property, &format!("{property}:{source}"), expected);
        }
    }
}

#[test]
fn font_defined_empty_custom_values_do_not_take_the_fallback() {
    for property in PROPERTIES {
        let mut expected = child(); replace(&mut expected, &parent(), property);
        assert_target(property, &format!("--empty:;{property}:var(--empty,{})", chosen_value(property)), expected);
    }
}

#[test]
fn font_invalid_substitutions_inherit_atomically_instead_of_using_fallback_or_prior_values() {
    for property in PROPERTIES {
        let mut expected = child(); replace(&mut expected, &parent(), property);
        assert_target(property, &format!("--bad:{};{property}:var(--bad,{})", invalid_value(property), chosen_value(property)), expected);
    }
}

#[test]
fn font_cycles_inherit_or_resolve_the_explicit_fallback() {
    for property in PROPERTIES {
        for fallback in [false, true] {
            let source = if fallback { format!("var(--a,{})", chosen_value(property)) } else { "var(--a)".into() };
            let mut expected = child(); replace(&mut expected, &if fallback { chosen() } else { parent() }, property);
            assert_target(property, &format!("--a:var(--b);--b:var(--a);{property}:{source}"), expected);
        }
    }
}

#[test]
fn font_substitution_preserves_token_boundaries_without_concatenating_family_names() {
    for (property, source) in [
        ("font-weight", "--n:6;font-weight:var(--n)00"),
        ("font-style", "--part:it;font-style:var(--part)alic"),
        ("font", "--n:24;font:var(--n)px JoinedFace"),
        ("font-family", "--part:'Read';font-family:var(--part)Sans"),
    ] {
        let mut expected = child(); replace(&mut expected, &parent(), property);
        assert_target(property, source, expected);
    }
    let mut expected = child(); expected.families = vec!["Read Sans".into()];
    assert_target("font-family", "--part:Read;font-family:var(--part)Sans", expected);
}

#[test]
fn font_source_order_and_specificity_select_variable_declarations() {
    for property in PROPERTIES {
        let base = format!("div{{{CHILD}}}.target{{{property}:initial}}#target{{{property}:var(--missing,{})}}.target{{{property}:initial}}", chosen_value(property));
        let doc = document(&base, "<div id=target class=target></div>");
        let mut expected = child(); replace(&mut expected, &chosen(), property);
        assert_eq!(font(at(&doc, &compute(&doc, &HashMap::new()), "target")), expected);
        let doc = document(&format!("{base}#target{{{property}:var(--missing,initial)}}"), "<div id=target class=target></div>");
        replace(&mut expected, &initial(), property);
        assert_eq!(font(at(&doc, &compute(&doc, &HashMap::new()), "target")), expected);
    }
}

#[test]
fn font_important_stylesheet_and_inline_values_keep_their_priority() {
    for property in PROPERTIES {
        let doc = document(&format!(".target{{{CHILD};{property}:var(--missing,{})!important}}", chosen_value(property)),
            &format!(r#"<div id=ordinary class=target style="{property}:initial"></div><div id=important class=target style="{property}:var(--missing,initial)!important"></div>"#));
        let styles = compute(&doc, &HashMap::new());
        let mut expected = child(); replace(&mut expected, &chosen(), property);
        assert_eq!(font(at(&doc, &styles, "ordinary")), expected);
        replace(&mut expected, &initial(), property);
        assert_eq!(font(at(&doc, &styles, "important")), expected);
    }
}

#[test]
fn font_invalid_variable_priority_cannot_revive_or_displace_the_wrong_winner() {
    for property in PROPERTIES {
        for important in [false, true] {
            let selected = if important { format!("{property}:{};{property}:var(--missing)!important", chosen_value(property)) }
                else { format!("{property}:{}!important;{property}:var(--missing)", chosen_value(property)) };
            let mut expected = child(); replace(&mut expected, &if important { parent() } else { chosen() }, property);
            assert_target(property, &selected, expected);
        }
    }
}

#[test]
fn font_invalid_variable_resets_preserve_nonfont_computed_state() {
    for property in PROPERTIES {
        let doc = document(&format!(concat!("#parent{{{PARENT}}}#target{{{CHILD};display:flex;width:31px;height:17px;",
            "color:#123456;background:#abcdef;padding:3px;margin:4px;gap:5px;{property}:var(--missing)}}"),
            PARENT = PARENT, CHILD = CHILD, property = property),
            "<div id=parent><div id=target></div></div>");
        let styles = compute(&doc, &HashMap::new()); let style = at(&doc, &styles, "target");
        let mut expected = child(); replace(&mut expected, &parent(), property);
        assert_eq!(font(style), expected);
        assert_eq!((style.display, style.width, style.height), (Display::Flex, Length::Px(31.0), Length::Px(17.0)));
        assert_eq!((style.color, style.background), (0x123456, Some(0xabcdef)));
        assert_eq!((style.padding, style.margin, style.gap), ([Length::Px(3.0); 4], [Length::Px(4.0); 4], [Length::Px(5.0); 2]));
    }
}

#[test]
fn font_shorthand_and_longhands_share_source_order_and_importance() {
    let doc = document(concat!(
        "#a{font-family:Before;font-weight:900;font-style:normal;font-size:31px;line-height:3;--all:italic 600 18px/2 After;font:var(--all)}",
        "#b{--all:italic 600 18px/2 Before;font:var(--all);font-family:After;font-weight:300;font-style:normal;font-size:31px;line-height:3}",
        "#c{font-family:Kept!important;font-weight:300!important;--all:italic 600 18px/2 After;font:var(--all)}"),
        "<div id=a></div><div id=b></div><div id=c></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(font(at(&doc, &styles, "a")), value(&["After"], 18.0, 600, true, Leading::Factor(2.0)));
    assert_eq!(font(at(&doc, &styles, "b")), value(&["After"], 31.0, 300, false, Leading::Factor(3.0)));
    assert_eq!(font(at(&doc, &styles, "c")), value(&["Kept"], 18.0, 300, true, Leading::Factor(2.0)));
}

#[test]
fn font_shorthand_line_height_uses_final_size_and_inherits_computed_lengths() {
    let doc = document(concat!(
        "#parent{--all:20px/2em Measure;font:var(--all);font-size:30px}",
        "#child{font-size:50px}#percent{font:var(--missing,150%/125% Percent)}",
        "#percent-child{font-size:60px}"),
        "<div id=parent><div id=child></div><div id=percent><span id=percent-child></span></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(font(at(&doc, &styles, "parent")), value(&["Measure"], 30.0, 400, false, Leading::Pixels(60.0)));
    assert_eq!(font(at(&doc, &styles, "child")), value(&["Measure"], 50.0, 400, false, Leading::Pixels(60.0)));
    assert_eq!(font(at(&doc, &styles, "percent")), value(&["Percent"], 45.0, 400, false, Leading::Pixels(56.25)));
    assert_eq!(font(at(&doc, &styles, "percent-child")), value(&["Percent"], 60.0, 400, false, Leading::Pixels(56.25)));
}

#[test]
fn font_variable_stylesheet_shorthand_updates_inline_line_height_rollback() {
    let doc = document("#plain,#important{font:var(--missing,italic 600 20px/2.5 SheetFace)}#important{font:var(--missing,italic 600 20px/2.5 SheetFace)!important}",
        "<div id=plain style='font:var(--missing,30px/4 InlineFace);line-height:revert-layer'></div><div id=important style='font:var(--missing,30px/4 InlineFace)!important;line-height:var(--missing,revert-layer)!important'></div>");
    let styles = compute(&doc, &HashMap::new());
    for name in ["plain", "important"] {
        assert_eq!(font(at(&doc, &styles, name)), value(&["InlineFace"], 30.0, 400, false, Leading::Factor(2.5)), "{name}");
    }
}

#[test]
fn font_invalid_computed_shorthand_updates_the_line_height_checkpoint_atomically() {
    let doc = document(&format!(concat!("#parent{{{PARENT}}}",
        "#missing{{line-height:2;font:var(--missing)}}",
        "#invalid{{--bad:italic 24px/2/3 Broken;line-height:2;font:var(--bad)}}",
        "#literal{{line-height:2;font:italic 24px/2/3 Broken}}",
        "#initial{{line-height:2;font:var(--missing,initial)}}"), PARENT = PARENT),
        "<div id=parent><div id=missing style='line-height:4;line-height:revert-layer'></div><div id=invalid style='line-height:4;line-height:revert-layer'></div><div id=literal style='line-height:4;line-height:revert-layer'></div><div id=initial style='line-height:4;line-height:revert-layer'></div></div>");
    let styles = compute(&doc, &HashMap::new());
    for name in ["missing", "invalid"] { assert_eq!(font(at(&doc, &styles, name)), parent(), "{name}"); }
    let mut literal = parent(); literal.leading = Leading::Factor(2.0);
    assert_eq!(font(at(&doc, &styles, "literal")), literal);
    assert_eq!(font(at(&doc, &styles, "initial")), initial());
}

#[test]
fn font_shorthand_substitution_refusal_keeps_size_unavailable_until_a_literal_recovers() {
    let many = (0..257).map(|index| format!("--n{index}:x;")).collect::<String>();
    let doc = document(&format!(concat!("#parent{{{PARENT};{many}font:var(--missing,22px Refused)}}",
        "#relative{{font:2em Relative}}#rooted{{font:1rem Rooted}}#absolute{{font:24px Absolute}}",
        "#initial{{font:initial}}#inherit{{font:inherit}}"), PARENT = PARENT, many = many),
        "<div id=parent><div id=relative></div><div id=rooted></div><div id=absolute></div><div id=initial></div><div id=inherit></div></div>");
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    for name in ["parent", "relative", "inherit"] {
        assert_eq!(result.computed_properties.read_font_size(id(&doc, name)), Err(ComputedStyleUnavailable), "{name}");
    }
    for (name, size) in [("rooted", 16.0), ("absolute", 24.0), ("initial", 16.0)] {
        assert_eq!(result.computed_properties.read_font_size(id(&doc, name)), Ok(Some(size)), "{name}");
    }
    assert_eq!(font(at(&doc, &result.styles, "parent")), initial());
    assert_eq!(at(&doc, &result.styles, "absolute").font_families, vec!["Absolute"]);
}

#[test]
fn font_missing_and_invalid_shorthands_preserve_known_inherited_size() {
    let doc = document(&format!("#parent{{{PARENT}}}#missing{{{CHILD};font:var(--missing)}}#invalid{{--bad:italic 24px/2/3 Broken;{CHILD};font:var(--bad)}}#relative{{font:var(--missing,2em Known)}}"),
        "<div id=parent><div id=missing></div><div id=invalid></div><div id=relative></div></div>");
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    for name in ["missing", "invalid"] {
        assert_eq!(font(at(&doc, &result.styles, name)), parent());
        assert_eq!(result.computed_properties.read_font_size(id(&doc, name)), Ok(Some(20.0)));
    }
    assert_eq!(result.computed_properties.read_font_size(id(&doc, "relative")), Ok(Some(40.0)));
}

#[test]
fn font_parent_changes_reuse_stylesheet_rules_and_recompute_inherited_values() {
    let mut doc = document("#child{font:var(--missing,inherit)}", &format!(r#"<div id=parent style="{PARENT}"><div id=child></div></div>"#));
    let first = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
    assert_eq!(font(at(&doc, &first.styles, "child")), parent());
    let parent_node = id(&doc, "parent"); set_attribute(&mut doc, parent_node, "style", &format!("font:{}", chosen_value("font")));
    let next = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, first.stylesheet_cache.as_ref());
    assert_eq!(font(at(&doc, &next.styles, "child")), chosen());
    assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
    assert_eq!(font(at(&doc, &next.styles, "child")), font(at(&doc, &compute(&doc, &HashMap::new()), "child")));
}

#[test]
fn font_external_source_changes_replace_cached_family_and_shorthand_values() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=target></div>");
    let sheet = id(&doc, "sheet");
    let mut external = HashMap::from([(sheet, "#target{--font:italic 500 20px/2 'First Face';font:var(--font)}".into())]);
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(font(at(&doc, &first.styles, "target")), value(&["First Face"], 20.0, 500, true, Leading::Factor(2.0)));
    external.insert(sheet, "#target{--font:300 22px/3 'Second Face';font:var(--font)}".into());
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    assert_eq!(font(at(&doc, &next.styles, "target")), value(&["Second Face"], 22.0, 300, false, Leading::Factor(3.0)));
    assert!(!first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn font_longhand_blocked_variable_maps_inherit_only_the_target_and_fresh_sources_recover() {
    let many = (0..257).map(|index| format!("--n{index}:x;")).collect::<String>();
    for property in LONGHANDS {
        let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=parent><div id=target></div></div>");
        let sheet = id(&doc, "sheet");
        let source = format!("#parent{{{PARENT}}}#target{{{CHILD};width:31px;color:#123456;{property}:var(--missing,{})}}", chosen_value(property));
        let mut external = HashMap::from([(sheet, format!("#target{{{many}}}{source}"))]);
        let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
        let mut expected = child(); replace(&mut expected, &parent(), property);
        assert_eq!(font(at(&doc, &first.styles, "target")), expected);
        assert_eq!(first.computed_properties.read_font_size(id(&doc, "target")), Ok(Some(27.0)));
        external.insert(sheet, source);
        let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
        replace(&mut expected, &chosen(), property);
        let style = at(&doc, &next.styles, "target");
        assert_eq!(font(style), expected);
        assert_eq!((style.width, style.color), (Length::Px(31.0), 0x123456));
    }
}

#[test]
fn font_cached_job_work_and_copy_refusals_preserve_sources_and_recover() {
    for property in PROPERTIES {
        let doc = document(&format!("#target{{{CHILD};width:31px;{property}:var(--missing,{})}}", chosen_value(property)), "<div id=target></div>");
        let before = format!("{:?}", doc.nodes);
        let prior = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
        for transform in [false, true] {
            for (mut work, mut copies) in [(0, usize::MAX), (usize::MAX, 0)] {
                assert!(run_cached(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).is_err());
                assert_eq!(format!("{:?}", doc.nodes), before);
            }
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let recovered = run_cached(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            let mut expected = child(); replace(&mut expected, &chosen(), property);
            assert_eq!(font(at(&doc, &recovered.styles, "target")), expected);
            assert_eq!(at(&doc, &recovered.styles, "target").width, Length::Px(31.0));
            assert_eq!(format!("{:?}", doc.nodes), before);
            assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
        }
    }
}

#[test]
fn font_warm_substitution_charges_expanded_values_on_both_budget_routes() {
    for property in PROPERTIES {
        let selected = chosen_value(property);
        let sheet = format!("html{{--empty:;--short:{selected};--large:{}{selected}}}.smallval{{{property}:var(--short)}}.largeval{{{property}:var(--large)}}", "var(--empty)".repeat(80));
        let mut doc = document(&sheet, "<div id=target class=smallval style='width:31px;color:#123456'></div>");
        let target = id(&doc, "target");
        let prior = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
        for transform in [false, true] {
            set_attribute(&mut doc, target, "class", "smallval");
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let control = run_cached(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(control.stylesheet_cache.as_ref().unwrap()));
            let short = control.computed_properties.read(target, "--short").unwrap().unwrap();
            let large = control.computed_properties.read(target, "--large").unwrap().unwrap();
            assert_eq!((short.trim(), large.trim()), (selected, selected));
            assert!(large.len() > short.len() + 64, "expanded fixture must retain additional copied bytes");
            let control_copies = usize::MAX - copies;
            set_attribute(&mut doc, target, "class", "largeval");
            let before = format!("{:?}", doc.nodes);
            let (mut work, mut copies) = (usize::MAX, control_copies.checked_add(64).unwrap());
            assert!(run_cached(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).is_err(),
                "{property}: expanded substitution must be charged on transform route {transform}");
            assert_eq!(format!("{:?}", doc.nodes), before);
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let recovered = run_cached(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            let mut expected = initial(); replace(&mut expected, &chosen(), property);
            assert_eq!(font(&recovered.styles[target]), expected);
            assert_eq!((recovered.styles[target].width, recovered.styles[target].color), (Length::Px(31.0), 0x123456));
            assert_eq!(format!("{:?}", doc.nodes), before);
        }
    }
}

#[test]
fn font_native_inline_longhands_admit_supported_literals_variables_and_wide_keywords() {
    for property in LONGHANDS {
        for source in ["inherit", "initial", "unset", "var(--chosen)", chosen_value(property)] {
            names(&format!("{property}:{source}"), &[property]);
        }
    }
    for source in ["'Quoted, Face',serif", "'var(--missing)'", "Case Sensitive Face,monospace"] {
        names(&format!("font-family:{source}"), &["font-family"]);
    }
    for source in ["1", "99", "600", "1000", "normal", "bold", "bolder", "lighter"] {
        names(&format!("font-weight:{source}"), &["font-weight"]);
    }
    for source in ["normal", "italic", "oblique"] { names(&format!("font-style:{source}"), &["font-style"]); }
}

#[test]
fn font_native_inline_names_reject_bad_values_without_moving_earlier_winners() {
    for (property, invalid) in [("font-family", "12"), ("font-family", "'One' 'Two'"),
        ("font-weight", "1001"), ("font-weight", "'var(--x)'"), ("font-weight", "normal bold"),
        ("font-style", "'var(--x)'"), ("font-style", "italic normal"), ("font-style", "var(not-custom)")] {
        names(&format!("{property}:{invalid}"), &[]);
        let other = if property == "font-style" { "font-family" } else { "font-style" };
        names(&format!("{property}:{};{other}:{};{property}:{invalid}", chosen_value(property), chosen_value(other)), &[property, other]);
    }
    names("font-family:First!important;font-style:italic;font-family:Second;font-weight:600", &["font-family", "font-style", "font-weight"]);
}

#[test]
fn font_native_inline_text_shares_longhand_admission_and_preserves_raw_family_values() {
    let source = "FONT-FAMILY:'MiXeD, Face',serif !important;font-style:ObLiQuE;font-weight:var(--weight,600);font-family:12;font-style:'var(--x)';font-weight:'var(--x)';font-family:'var(--x)'";
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    assert_eq!(inline_text_declarations(source, &LONGHANDS, &mut CssBudget::new(&mut work, &mut copies)).unwrap(), vec![
        ("font-family".into(), "'MiXeD, Face',serif".into(), true),
        ("font-style".into(), "ObLiQuE".into(), false),
        ("font-weight".into(), "var(--weight,600)".into(), false),
        ("font-family".into(), "'var(--x)'".into(), false),
    ]);
    for property in LONGHANDS {
        for source in ["inherit", "initial", "unset", "var(--missing)"] {
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            assert_eq!(inline_text_declarations(&format!("{property}:{source}"), &LONGHANDS,
                &mut CssBudget::new(&mut work, &mut copies)).unwrap(), vec![(property.into(), source.into(), false)]);
        }
    }
}

#[test]
fn font_native_inline_shorthand_enumeration_keeps_the_full_expansion_refusal() {
    for source in ["font:italic 600 20px/2 Face", "font:var(--chosen)", "font:inherit"] {
        assert!(inline_declaration_names(source, &PROPERTIES, None).unwrap_err().contains("font"));
        let (mut work, mut copies) = (usize::MAX, usize::MAX);
        assert!(inline_text_declarations(source, &PROPERTIES, &mut CssBudget::new(&mut work, &mut copies)).unwrap_err().contains("font"));
    }
}

#[test]
fn font_native_inline_work_and_copy_refusals_allow_fresh_recovery() {
    let source = "font-family:var(--family,'Case Face');font-style:var(--style,italic);font-weight:var(--weight,600)";
    for name_api in [true, false] {
        for (mut work, mut copies) in [(0, usize::MAX), (usize::MAX, 0)] {
            let mut budget = CssBudget::new(&mut work, &mut copies);
            let refused = if name_api { inline_declaration_names(source, &LONGHANDS, Some(&mut budget)).is_err() }
                else { inline_text_declarations(source, &LONGHANDS, &mut budget).is_err() };
            assert!(refused, "name API {name_api}");
        }
    }
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    let mut budget = CssBudget::new(&mut work, &mut copies);
    assert_eq!(inline_declaration_names(source, &LONGHANDS, Some(&mut budget)).unwrap(), LONGHANDS.map(str::to_owned).to_vec());
    assert_eq!(inline_text_declarations(source, &LONGHANDS, &mut budget).unwrap(), vec![
        ("font-family".into(), "var(--family,'Case Face')".into(), false),
        ("font-style".into(), "var(--style,italic)".into(), false),
        ("font-weight".into(), "var(--weight,600)".into(), false),
    ]);
    assert!(work < usize::MAX && copies < usize::MAX);
}
