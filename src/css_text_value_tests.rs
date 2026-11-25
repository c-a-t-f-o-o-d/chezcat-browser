//! Represented alignment and whitespace values share inherited cascade rules.
use super::*;

const PROPERTIES: [&str; 2] = ["text-align", "white-space"];
const PARENT: &str = "text-align:right;white-space:pre";
const CHILD: &str = "text-align:left;white-space:nowrap";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextValue { align: Align, legacy: Option<Align>, space: WhiteSpace }

const INITIAL: TextValue = TextValue { align: Align::Left, legacy: None, space: WhiteSpace::Normal };
const PARENT_VALUE: TextValue = TextValue { align: Align::Right, legacy: None, space: WhiteSpace::Pre };
const CHILD_VALUE: TextValue = TextValue { align: Align::Left, legacy: None, space: WhiteSpace::NoWrap };
const CHOSEN: TextValue = TextValue { align: Align::Center, legacy: None, space: WhiteSpace::PreWrap };

fn value(style: &Style) -> TextValue {
    TextValue { align: style.align, legacy: style.legacy_align, space: style.white_space }
}

fn replacing(mut base: TextValue, property: &str, source: TextValue) -> TextValue {
    match property {
        "text-align" => { base.align = source.align; base.legacy = source.legacy; }
        "white-space" => base.space = source.space,
        _ => panic!("unrecognized text property"),
    }
    base
}

fn chosen(property: &str) -> &'static str { if property == "text-align" { "center" } else { "pre-wrap" } }
fn parent_value(property: &str) -> &'static str { if property == "text-align" { "right" } else { "pre" } }
fn initial_value(property: &str) -> &'static str { if property == "text-align" { "left" } else { "normal" } }

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style { &styles[id(doc, name)] }

fn set_attribute(doc: &mut Document, node: usize, name: &str, text: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { panic!("element required") };
    if let Some(attribute) = attrs.iter_mut().find(|attribute| attribute.name.html_is(name)) {
        attribute.value = text.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified(name.into(), text.into()));
    }
}

fn target(source: &str, expected: TextValue) {
    let doc = document(&format!("#parent{{{PARENT}}}#target{{{CHILD};{source}}}"),
        "<div id=parent><div id=target></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(value(at(&doc, &styles, "parent")), PARENT_VALUE);
    assert_eq!(value(at(&doc, &styles, "target")), expected, "{source}");
}

fn names(source: &str, expected: &[&str]) {
    assert_eq!(inline_declaration_names(source, &PROPERTIES, None).unwrap(),
        expected.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>(), "{source}");
}

fn run_cached(doc: &Document, external: &HashMap<usize, String>, transform: bool,
    budget: &mut CssBudget<'_>, cache: Option<&StylesheetCache>) -> Result<CascadeOutput, String> {
    if transform {
        compute_with_environment_and_properties_transform_budgeted_cached(doc, external, None, None, budget, cache)
    } else {
        compute_with_environment_and_properties_budgeted_cached(doc, external, None, None, budget, cache)
    }
}

#[test]
fn text_alignment_literals_and_variables_keep_the_represented_aliases() {
    for (source, align) in [("left", Align::Left), ("start", Align::Left), ("CENTER", Align::Center),
        ("right", Align::Right), ("EnD", Align::Right)] {
        for variable in [false, true] {
            let source = if variable { format!("--align:{source};text-align:var(--align)") }
                else { format!("text-align:{source}") };
            target(&source, TextValue { align, ..CHILD_VALUE });
        }
    }
}

#[test]
fn text_whitespace_literals_and_variables_keep_the_represented_modes() {
    for (source, space) in [("normal", WhiteSpace::Normal), ("NOWRAP", WhiteSpace::NoWrap),
        ("pre", WhiteSpace::Pre), ("Pre-Wrap", WhiteSpace::PreWrap), ("break-spaces", WhiteSpace::PreWrap)] {
        for variable in [false, true] {
            let source = if variable { format!("--space:{source};white-space:var(--space)") }
                else { format!("white-space:{source}") };
            target(&source, TextValue { space, ..CHILD_VALUE });
        }
    }
}

#[test]
fn text_values_naturally_inherit_the_completed_parent() {
    let doc = document(&format!("#parent{{{PARENT}}}"),
        "<div id=parent><section id=child><span id=leaf></span></section></div>");
    let styles = compute(&doc, &HashMap::new());
    for name in ["parent", "child", "leaf"] { assert_eq!(value(at(&doc, &styles, name)), PARENT_VALUE, "{name}"); }
}

#[test]
fn text_literal_wide_keywords_change_only_the_selected_property() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            let source = if keyword == "initial" { INITIAL } else { PARENT_VALUE };
            target(&format!("{property}:{keyword}"), replacing(CHILD_VALUE, property, source));
        }
    }
}

#[test]
fn text_variable_wide_keywords_change_only_the_selected_property() {
    for property in PROPERTIES {
        for keyword in ["InHeRiT", "INITIAL", "UnSeT"] {
            let source = if keyword == "INITIAL" { INITIAL } else { PARENT_VALUE };
            target(&format!("{property}:var(--missing,{keyword})"), replacing(CHILD_VALUE, property, source));
        }
    }
}

#[test]
fn text_root_literal_defaulting_excludes_the_document_container() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            let mut doc = crate::html::parse(&format!(r#"<html id=root style="{CHILD};{property}:{keyword}"><head></head><body></body></html>"#));
            set_attribute(&mut doc, 0, "style", PARENT);
            assert_eq!(value(at(&doc, &compute(&doc, &HashMap::new()), "root")),
                replacing(CHILD_VALUE, property, INITIAL), "{property}:{keyword}");
        }
    }
}

#[test]
fn text_root_variable_defaults_and_invalid_winners_use_initial_values() {
    for property in PROPERTIES {
        for source in ["var(--missing,inherit)", "var(--missing,initial)", "var(--missing,unset)",
            "var(--missing)", "var(--missing,broken)"] {
            let mut doc = crate::html::parse(&format!(r#"<html id=root style="{CHILD};{property}:{source}"><head></head><body></body></html>"#));
            set_attribute(&mut doc, 0, "style", PARENT);
            assert_eq!(value(at(&doc, &compute(&doc, &HashMap::new()), "root")),
                replacing(CHILD_VALUE, property, INITIAL), "{property}:{source}");
        }
    }
}

#[test]
fn text_fragment_context_is_excluded_but_real_null_namespace_parents_inherit() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            for variable in [false, true] {
                let source = if variable { format!("var(--missing,{keyword})") } else { keyword.into() };
                let mut doc = crate::html::parse_fragment(
                    "<link id=sheet rel=stylesheet><section id=top><div id=real><span id=inner></span></div></section>", "div", false).unwrap();
                assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
                let real = id(&doc, "real"); doc.nodes[real].namespace = crate::html::Namespace::None;
                let external = HashMap::from([(id(&doc, "sheet"),
                    format!("div{{{PARENT}}}#top,#inner{{{CHILD};{property}:{source}}}"))]);
                let styles = compute(&doc, &external);
                assert_eq!(value(at(&doc, &styles, "top")), replacing(CHILD_VALUE, property, INITIAL));
                assert_eq!(value(at(&doc, &styles, "inner")), replacing(CHILD_VALUE, property,
                    if keyword == "initial" { INITIAL } else { PARENT_VALUE }));
            }
        }
    }
}

#[test]
fn text_unsupported_literal_values_preserve_the_previous_declaration() {
    for (property, unsupported) in [
        ("text-align", vec!["justify", "justify-all", "match-parent", "center right", "12", "'center'", "'var(--x)'"]),
        ("white-space", vec!["pre-line", "preserve nowrap", "normal pre", "12", "'pre'", "'var(--x)'"]),
    ] {
        for source in unsupported { target(&format!("{property}:{source}!important"), CHILD_VALUE); }
    }
}

#[test]
fn text_malformed_variable_syntax_is_rejected_before_the_cascade() {
    for property in PROPERTIES {
        for source in ["var(not-custom)", "var(--,normal)", "var(--a --b)", "var()", "var (--missing)"] {
            target(&format!("{property}:{source}"), CHILD_VALUE);
        }
    }
}

#[test]
fn text_missing_variable_winners_inherit_without_reviving_previous_values() {
    for property in PROPERTIES {
        for source in ["var(--missing)", "var(--missing,)"] {
            target(&format!("{property}:{source}"), replacing(CHILD_VALUE, property, PARENT_VALUE));
        }
    }
}

#[test]
fn text_defined_empty_custom_values_do_not_take_the_fallback() {
    for property in PROPERTIES {
        target(&format!("--empty:;{property}:var(--empty,{})", chosen(property)), replacing(CHILD_VALUE, property, PARENT_VALUE));
    }
}

#[test]
fn text_invalid_substitutions_inherit_without_using_fallback_or_prior_values() {
    for property in PROPERTIES {
        for invalid in ["broken", "'var(--x)'", "12"] {
            target(&format!("--bad:{invalid};{property}:var(--bad,{})", chosen(property)), replacing(CHILD_VALUE, property, PARENT_VALUE));
        }
    }
}

#[test]
fn text_cycles_inherit_or_use_an_explicit_valid_fallback() {
    for property in PROPERTIES {
        for fallback in [false, true] {
            let source = if fallback { format!("var(--a,{})", chosen(property)) } else { "var(--a)".into() };
            target(&format!("--a:var(--b);--b:var(--a);{property}:{source}"),
                replacing(CHILD_VALUE, property, if fallback { CHOSEN } else { PARENT_VALUE }));
        }
    }
}

#[test]
fn text_nested_fallbacks_comments_and_keyword_case_use_the_same_grammar() {
    for (property, keyword) in [("text-align", "CeNtEr"), ("white-space", "PrE-WrAp")] {
        target(&format!("{property}:VaR(--missing,var(--other,/* before */ {keyword} /* after */))"),
            replacing(CHILD_VALUE, property, CHOSEN));
    }
}

#[test]
fn text_custom_names_are_case_sensitive_and_inherited_aliases_are_resolved() {
    for property in PROPERTIES {
        let doc = document(&format!(
            "#parent{{{PARENT};--Value:{};--value:{};--alias:var(--Value)}}#upper,#lower,#alias{{{CHILD}}}\
             #upper{{{property}:var(--Value)}}#lower{{{property}:var(--value)}}\
             #alias{{--Value:{};{property}:var(--alias)}}",
            chosen(property), parent_value(property), initial_value(property)),
            "<div id=parent><div id=upper></div><div id=lower></div><div id=alias></div></div>");
        let styles = compute(&doc, &HashMap::new());
        for name in ["upper", "alias"] { assert_eq!(value(at(&doc, &styles, name)), replacing(CHILD_VALUE, property, CHOSEN)); }
        assert_eq!(value(at(&doc, &styles, "lower")), replacing(CHILD_VALUE, property, PARENT_VALUE));
    }
}

#[test]
fn text_substitution_does_not_concatenate_keyword_tokens() {
    for (property, source) in [("text-align", "--part:cen;text-align:var(--part)ter"),
        ("white-space", "--part:no;white-space:var(--part)wrap")] {
        target(source, replacing(CHILD_VALUE, property, PARENT_VALUE));
    }
}

#[test]
fn text_source_order_and_specificity_select_variable_declarations() {
    for property in PROPERTIES {
        let sheet = format!("div{{{CHILD}}}.target{{{property}:initial}}#target{{{property}:var(--missing,{})}}.target{{{property}:initial}}", chosen(property));
        let doc = document(&sheet, "<div id=target class=target></div>");
        assert_eq!(value(at(&doc, &compute(&doc, &HashMap::new()), "target")), replacing(CHILD_VALUE, property, CHOSEN));
        let doc = document(&format!("{sheet}#target{{{property}:var(--missing,initial)}}"), "<div id=target class=target></div>");
        assert_eq!(value(at(&doc, &compute(&doc, &HashMap::new()), "target")), replacing(CHILD_VALUE, property, INITIAL));
    }
}

#[test]
fn text_important_stylesheet_and_inline_values_keep_their_priority() {
    for property in PROPERTIES {
        let doc = document(&format!(".target{{{CHILD};{property}:var(--missing,{})!important}}", chosen(property)),
            &format!(r#"<div id=ordinary class=target style="{property}:initial"></div><div id=important class=target style="{property}:var(--missing,initial)!important"></div>"#));
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(value(at(&doc, &styles, "ordinary")), replacing(CHILD_VALUE, property, CHOSEN));
        assert_eq!(value(at(&doc, &styles, "important")), replacing(CHILD_VALUE, property, INITIAL));
    }
}

#[test]
fn text_invalid_variable_priority_cannot_revive_or_displace_the_wrong_winner() {
    for property in PROPERTIES {
        for important in [false, true] {
            let source = if important { format!("{property}:{};{property}:var(--missing)!important", chosen(property)) }
                else { format!("{property}:{}!important;{property}:var(--missing)", chosen(property)) };
            target(&source, replacing(CHILD_VALUE, property, if important { PARENT_VALUE } else { CHOSEN }));
        }
    }
}

#[test]
fn text_defaulting_preserves_other_computed_style_components() {
    for property in PROPERTIES {
        let doc = document(&format!("#parent{{{PARENT}}}#target{{{CHILD};font:italic 700 23px/2 Face;display:flex;width:31px;height:17px;color:#123456;background:#abcdef;padding:3px;margin:4px;gap:5px;{property}:var(--missing)}}"),
            "<div id=parent><div id=target></div></div>");
        let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
        let style = at(&doc, &result.styles, "target");
        assert_eq!(value(style), replacing(CHILD_VALUE, property, PARENT_VALUE));
        assert_eq!((style.font_size, style.font_weight, style.bold, style.italic), (23.0, 700, true, true));
        assert_eq!(style.font_families, vec!["Face"]);
        assert!(matches!(style.line_height, LineHeight::Factor(2.0)));
        assert_eq!((style.display, style.width, style.height), (Display::Flex, Length::Px(31.0), Length::Px(17.0)));
        assert_eq!((style.color, style.background), (0x123456, Some(0xabcdef)));
        assert_eq!((style.padding, style.margin, style.gap), ([Length::Px(3.0); 4], [Length::Px(4.0); 4], [Length::Px(5.0); 2]));
        assert_eq!(result.computed_properties.read_font_size(id(&doc, "target")), Ok(Some(23.0)));
    }
}

#[test]
fn text_alignment_literals_clear_legacy_hints_while_inherit_keeps_the_parent_hint() {
    let parent = TextValue { align: Align::Center, legacy: Some(Align::Center), space: WhiteSpace::Normal };
    for (source, align) in [("left", Align::Left), ("start", Align::Left), ("center", Align::Center),
        ("right", Align::Right), ("end", Align::Right)] {
        let doc = document(&format!("#target{{text-align:{source}}}"), "<center id=parent><div id=target align=right></div></center>");
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(value(at(&doc, &styles, "parent")), parent);
        assert_eq!(value(at(&doc, &styles, "target")), TextValue { align, ..INITIAL });
    }
    let doc = document("#target{text-align:inherit}", "<center id=parent><div id=target align=right></div></center>");
    assert_eq!(value(at(&doc, &compute(&doc, &HashMap::new()), "target")), parent);
}

#[test]
fn text_alignment_variable_defaults_manage_the_legacy_component_atomically() {
    let parent = TextValue { align: Align::Center, legacy: Some(Align::Center), space: WhiteSpace::Pre };
    for (source, expected) in [
        ("inherit", parent), ("unset", parent), ("var(--missing)", parent),
        ("var(--missing,inherit)", parent), ("var(--missing,unset)", parent),
        ("var(--missing,broken)", parent),
        ("initial", INITIAL), ("var(--missing,initial)", INITIAL),
        ("var(--missing,center)", CHOSEN),
    ] {
        let doc = document(&format!("#parent{{white-space:pre}}#target{{{CHILD};text-align:{source}}}"),
            "<div id=parent align=center><div id=target align=right></div></div>");
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(value(at(&doc, &styles, "parent")), parent);
        assert_eq!(value(at(&doc, &styles, "target")), replacing(CHILD_VALUE, "text-align", expected), "{source}");
    }
}

#[test]
fn text_whitespace_invalid_winners_override_pre_and_nowrap_presentation_hints() {
    for (body, inherited, space) in [
        ("<div id=parent><pre id=target>AV</pre></div>", "nowrap", WhiteSpace::NoWrap),
        ("<div id=parent><table><tr><td id=target nowrap>AV</td></tr></table></div>", "pre", WhiteSpace::Pre),
    ] {
        for source in ["inherit", "unset", "var(--missing)", "var(--missing,unset)", "var(--missing,broken)"] {
            let doc = document(&format!("#parent{{white-space:{inherited}}}#target{{white-space:{source}}}"), body);
            assert_eq!(at(&doc, &compute(&doc, &HashMap::new()), "target").white_space, space, "{source}");
        }
        for source in ["initial", "var(--missing,initial)"] {
            let doc = document(&format!("#parent{{white-space:{inherited}}}#target{{white-space:{source}}}"), body);
            assert_eq!(at(&doc, &compute(&doc, &HashMap::new()), "target").white_space, WhiteSpace::Normal);
        }
    }
}

#[test]
fn text_alignment_invalid_winners_override_center_and_align_presentation_hints() {
    for body in ["<div id=parent><center id=target>AV</center></div>",
        "<div id=parent><div id=target align=center>AV</div></div>"] {
        for source in ["inherit", "unset", "var(--missing)", "var(--missing,unset)", "var(--missing,broken)"] {
            let doc = document(&format!("#parent{{{PARENT}}}#target{{text-align:{source}}}"), body);
            assert_eq!(value(at(&doc, &compute(&doc, &HashMap::new()), "target")), PARENT_VALUE, "{source}");
        }
        for source in ["initial", "var(--missing,initial)"] {
            let doc = document(&format!("#parent{{{PARENT}}}#target{{text-align:{source}}}"), body);
            assert_eq!(value(at(&doc, &compute(&doc, &HashMap::new()), "target")), replacing(PARENT_VALUE, "text-align", INITIAL));
        }
    }
}

#[test]
fn text_parent_changes_reuse_rules_and_recompute_inherited_values() {
    for property in PROPERTIES {
        let mut doc = document(&format!("#target{{{CHILD};{property}:var(--missing,inherit)}}"),
            &format!(r#"<div id=parent style="{PARENT}"><div id=target></div></div>"#));
        let first = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
        assert_eq!(value(at(&doc, &first.styles, "target")), replacing(CHILD_VALUE, property, PARENT_VALUE));
        let parent = id(&doc, "parent"); set_attribute(&mut doc, parent, "style", "text-align:center;white-space:pre-wrap");
        let next = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, first.stylesheet_cache.as_ref());
        assert_eq!(value(at(&doc, &next.styles, "target")), replacing(CHILD_VALUE, property, CHOSEN));
        assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
        assert_eq!(value(at(&doc, &next.styles, "target")), value(at(&doc, &compute(&doc, &HashMap::new()), "target")));
    }
}

#[test]
fn text_external_source_changes_replace_cached_values() {
    for property in PROPERTIES {
        let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=target></div>");
        let sheet = id(&doc, "sheet");
        let mut external = HashMap::from([(sheet, format!("#target{{{CHILD};--value:{};{property}:var(--value)}}", chosen(property)))]);
        let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
        assert_eq!(value(at(&doc, &first.styles, "target")), replacing(CHILD_VALUE, property, CHOSEN));
        external.insert(sheet, format!("#target{{{CHILD};--value:{};{property}:var(--value)}}", parent_value(property)));
        let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
        assert_eq!(value(at(&doc, &next.styles, "target")), replacing(CHILD_VALUE, property, PARENT_VALUE));
        assert!(!first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
    }
}

#[test]
fn text_custom_property_mutation_updates_an_unchanged_warm_stylesheet() {
    for property in PROPERTIES {
        let mut doc = document(&format!("#parent{{{PARENT}}}#target{{{CHILD};{property}:var(--value)}}"),
            &format!("<div id=parent style='--value:{}'><div id=target></div></div>", chosen(property)));
        let first = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
        assert_eq!(value(at(&doc, &first.styles, "target")), replacing(CHILD_VALUE, property, CHOSEN));
        let parent = id(&doc, "parent"); set_attribute(&mut doc, parent, "style", &format!("--value:{}", initial_value(property)));
        let next = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, first.stylesheet_cache.as_ref());
        assert_eq!(value(at(&doc, &next.styles, "target")), replacing(CHILD_VALUE, property, INITIAL));
        assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
    }
}

#[test]
fn text_blocked_variable_maps_inherit_only_the_target_and_fresh_sources_recover() {
    let many = (0..257).map(|index| format!("--n{index}:x;")).collect::<String>();
    for property in PROPERTIES {
        let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=parent><div id=target></div></div>");
        let sheet = id(&doc, "sheet");
        let source = format!("#parent{{{PARENT}}}#target{{{CHILD};font-size:23px;width:31px;{property}:var(--missing,{})}}", chosen(property));
        let mut external = HashMap::from([(sheet, format!("#target{{{many}}}{source}"))]);
        let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
        assert_eq!(value(at(&doc, &first.styles, "target")), replacing(CHILD_VALUE, property, PARENT_VALUE));
        assert_eq!(first.computed_properties.read_font_size(id(&doc, "target")), Ok(Some(23.0)));
        external.insert(sheet, source);
        let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
        assert_eq!(value(at(&doc, &next.styles, "target")), replacing(CHILD_VALUE, property, CHOSEN));
        assert_eq!(at(&doc, &next.styles, "target").width, Length::Px(31.0));
    }
}

#[test]
fn text_cached_job_work_and_copy_refusals_preserve_sources_and_recover() {
    for property in PROPERTIES {
        let doc = document(&format!("#target{{{CHILD};width:31px;{property}:var(--missing,{})}}", chosen(property)), "<div id=target></div>");
        let before = format!("{:?}", doc.nodes);
        let prior = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
        for transform in [false, true] {
            for (mut work, mut copies) in [(0, usize::MAX), (usize::MAX, 0)] {
                assert!(run_cached(&doc, &HashMap::new(), transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).is_err());
                assert_eq!(format!("{:?}", doc.nodes), before);
            }
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let recovered = run_cached(&doc, &HashMap::new(), transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            assert_eq!(value(at(&doc, &recovered.styles, "target")), replacing(CHILD_VALUE, property, CHOSEN));
            assert_eq!(at(&doc, &recovered.styles, "target").width, Length::Px(31.0));
            assert_eq!(format!("{:?}", doc.nodes), before);
            assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
        }
    }
}

#[test]
fn text_warm_substitution_charges_expanded_values_on_both_budget_routes() {
    for property in PROPERTIES {
        let selected = chosen(property);
        let sheet = format!("html{{--empty:;--short:{selected};--large:{}{selected}}}div{{{CHILD}}}.smallval{{{property}:var(--short)}}.largeval{{{property}:var(--large)}}", "var(--empty)".repeat(80));
        let mut doc = document(&sheet, "<div id=target class=smallval style='width:31px;color:#123456'></div>");
        let target = id(&doc, "target");
        let prior = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
        for transform in [false, true] {
            set_attribute(&mut doc, target, "class", "smallval");
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let control = run_cached(&doc, &HashMap::new(), transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(control.stylesheet_cache.as_ref().unwrap()));
            let short = control.computed_properties.read(target, "--short").unwrap().unwrap();
            let large = control.computed_properties.read(target, "--large").unwrap().unwrap();
            assert_eq!((short.trim(), large.trim()), (selected, selected));
            assert!(large.len() > short.len() + 64);
            let control_copies = usize::MAX - copies;
            set_attribute(&mut doc, target, "class", "largeval");
            let before = format!("{:?}", doc.nodes);
            let (mut work, mut copies) = (usize::MAX, control_copies.checked_add(64).unwrap());
            assert!(run_cached(&doc, &HashMap::new(), transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).is_err(),
                "{property}: larger expansion requires admission on route {transform}");
            assert_eq!(format!("{:?}", doc.nodes), before);
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let recovered = run_cached(&doc, &HashMap::new(), transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            assert_eq!(value(&recovered.styles[target]), replacing(CHILD_VALUE, property, CHOSEN));
            assert_eq!((recovered.styles[target].width, recovered.styles[target].color), (Length::Px(31.0), 0x123456));
        }
    }
}

fn finite_source(property: &str, heavy: bool) -> String {
    // The stored value remains below 2048 bytes. Its repeated consumers copy
    // more than 8 MiB even without any additional conservative parser reserve.
    let pad = "var(--e)".repeat(200);
    let consumers = if heavy { format!("{property}:var(--large);").repeat(4000) } else { String::new() };
    let source = format!("html{{--e:;--pad:{pad};--large:var(--pad)var(--pad)var(--pad){};--visible:yes}}\
        #parent{{{PARENT}}}#target{{{CHILD};font-size:23px;{consumers}{property}:var(--missing)}}", chosen(property));
    let usage = StylesheetLimits::default().accept(&source, StylesheetUsage::default()).unwrap();
    assert_eq!((usage.bytes, usage.rules), (source.len(), 3), "the complete fixture must be admitted");
    source
}

fn finite_document() -> Document {
    crate::html::parse("<!doctype html><html><head><link id=sheet rel=stylesheet href=text.css></head><body><div id=parent><div id=target></div></div></body></html>")
}

#[test]
fn text_finite_allowance_refuses_both_budgeted_routes_and_fresh_sources_recover() {
    for property in PROPERTIES {
        let doc = finite_document();
        let before = format!("{:?}", doc.nodes);
        let sheet = id(&doc, "sheet");
        let mut external = HashMap::from([(sheet, finite_source(property, true))]);
        for transform in [false, true] {
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            assert!(run_cached(&doc, &external, transform, &mut CssBudget::new(&mut work, &mut copies), None).is_err(),
                "{property}: ample caller allowances cannot hide finite refusal, route {transform}");
            assert_eq!(format!("{:?}", doc.nodes), before);
        }
        external.insert(sheet, finite_source(property, false));
        for transform in [false, true] {
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let recovered = run_cached(&doc, &external, transform, &mut CssBudget::new(&mut work, &mut copies), None).unwrap();
            assert_eq!(value(at(&doc, &recovered.styles, "target")), replacing(CHILD_VALUE, property, PARENT_VALUE));
            assert_eq!(recovered.computed_properties.read_font_size(id(&doc, "target")), Ok(Some(23.0)));
            assert_eq!(recovered.computed_properties.read(id(&doc, "target"), "--visible"), Ok(Some("yes")));
            assert_eq!(format!("{:?}", doc.nodes), before);
        }
    }
}

#[test]
fn text_finite_allowance_leaves_unmetered_readback_incomplete_until_recovery() {
    for property in PROPERTIES {
        let doc = finite_document();
        let before = format!("{:?}", doc.nodes);
        let sheet = id(&doc, "sheet");
        let mut external = HashMap::from([(sheet, finite_source(property, true))]);
        let refused = compute_with_environment_and_properties(&doc, &external, None, None);
        assert_eq!(refused.computed_properties.read_font_size(id(&doc, "target")), Err(ComputedStyleUnavailable), "{property}");
        assert_eq!(refused.computed_properties.read(id(&doc, "target"), "--visible"), Err(ComputedStyleUnavailable), "{property}");
        assert_eq!(format!("{:?}", doc.nodes), before);
        external.insert(sheet, finite_source(property, false));
        let recovered = compute_with_environment_and_properties(&doc, &external, None, None);
        assert_eq!(value(at(&doc, &recovered.styles, "target")), replacing(CHILD_VALUE, property, PARENT_VALUE));
        assert_eq!(recovered.computed_properties.read_font_size(id(&doc, "target")), Ok(Some(23.0)));
        assert_eq!(recovered.computed_properties.read(id(&doc, "target"), "--visible"), Ok(Some("yes")));
        assert_eq!(format!("{:?}", doc.nodes), before);
    }
}

#[test]
fn text_native_inline_names_admit_supported_literals_variables_and_wide_keywords() {
    for property in PROPERTIES {
        for source in ["inherit", "initial", "unset", "var(--chosen)", chosen(property)] {
            names(&format!("{property}:{source}"), &[property]);
        }
    }
    for source in ["left", "start", "center", "right", "end"] { names(&format!("text-align:{source}"), &["text-align"]); }
    for source in ["normal", "nowrap", "pre", "pre-wrap", "break-spaces"] { names(&format!("white-space:{source}"), &["white-space"]); }
}

#[test]
fn text_native_inline_names_reject_bad_values_without_moving_earlier_winners() {
    for (property, invalid) in [("text-align", "center right"), ("text-align", "'var(--x)'"),
        ("text-align", "justify"), ("text-align", "var(not-custom)"),
        ("white-space", "normal pre"), ("white-space", "'var(--x)'"),
        ("white-space", "preserve nowrap"), ("white-space", "var(not-custom)")] {
        names(&format!("{property}:{invalid}"), &[]);
        let other = if property == "text-align" { "white-space" } else { "text-align" };
        names(&format!("{property}:{};{other}:{};{property}:{invalid}", chosen(property), chosen(other)), &[property, other]);
    }
    names("text-align:right!important;white-space:pre;text-align:center", &["text-align", "white-space"]);
}

#[test]
fn text_native_inline_text_shares_admission_and_preserves_specified_tokens() {
    let source = "TEXT-ALIGN:CeNtEr !important;white-space:PrE-WrAp;text-align:var(--align,right);text-align:'var(--x)';white-space:'var(--x)';white-space:normal pre";
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    assert_eq!(inline_text_declarations(source, &PROPERTIES, &mut CssBudget::new(&mut work, &mut copies)).unwrap(), vec![
        ("text-align".into(), "CeNtEr".into(), true),
        ("white-space".into(), "PrE-WrAp".into(), false),
        ("text-align".into(), "var(--align,right)".into(), false),
    ]);
    for property in PROPERTIES {
        for source in ["inherit", "initial", "unset", "var(--missing)"] {
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            assert_eq!(inline_text_declarations(&format!("{property}:{source}"), &PROPERTIES,
                &mut CssBudget::new(&mut work, &mut copies)).unwrap(), vec![(property.into(), source.into(), false)]);
        }
    }
}

#[test]
fn text_native_inline_work_and_copy_refusals_allow_fresh_recovery() {
    let source = "text-align:var(--align,center);white-space:var(--space,pre-wrap)";
    for name_api in [false, true] {
        for (mut work, mut copies) in [(0, usize::MAX), (usize::MAX, 0)] {
            let mut budget = CssBudget::new(&mut work, &mut copies);
            let refused = if name_api { inline_declaration_names(source, &PROPERTIES, Some(&mut budget)).is_err() }
                else { inline_text_declarations(source, &PROPERTIES, &mut budget).is_err() };
            assert!(refused, "name API {name_api}");
        }
    }
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    let mut budget = CssBudget::new(&mut work, &mut copies);
    assert_eq!(inline_declaration_names(source, &PROPERTIES, Some(&mut budget)).unwrap(), PROPERTIES.map(str::to_owned).to_vec());
    assert_eq!(inline_text_declarations(source, &PROPERTIES, &mut budget).unwrap(), vec![
        ("text-align".into(), "var(--align,center)".into(), false),
        ("white-space".into(), "var(--space,pre-wrap)".into(), false),
    ]);
    assert!(work < usize::MAX && copies < usize::MAX);
}
