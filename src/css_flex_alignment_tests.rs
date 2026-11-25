//! Flex alignment defaults, variable winners and native inline admission.
use super::*;

const PROPERTIES: [&str; 4] = ["align-content", "align-items", "align-self", "justify-content"];
const PARENT: &str = "justify-content:space-between;align-content:space-around;align-items:flex-end;align-self:flex-start";
const CHILD: &str = "justify-content:flex-end;align-content:space-between;align-items:flex-start;align-self:center";
const PACK_VALUES: [(&str, FlexPack); 10] = [
    ("flex-start", FlexPack::Start), ("flex-end", FlexPack::End),
    ("start", FlexPack::LogicalStart), ("end", FlexPack::LogicalEnd),
    ("center", FlexPack::Center), ("space-between", FlexPack::Between),
    ("space-around", FlexPack::Around), ("space-evenly", FlexPack::Evenly),
    ("stretch", FlexPack::Stretch), ("normal", FlexPack::Start),
];
const ITEM_VALUES: [(&str, FlexAlign); 7] = [
    ("normal", FlexAlign::Stretch), ("stretch", FlexAlign::Stretch),
    ("start", FlexAlign::Start), ("flex-start", FlexAlign::Start),
    ("end", FlexAlign::End), ("flex-end", FlexAlign::End),
    ("center", FlexAlign::Center),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Alignment {
    justify: FlexPack,
    content: FlexPack,
    items: FlexAlign,
    self_align: Option<FlexAlign>,
}

fn alignment(style: &Style) -> Alignment {
    Alignment { justify: style.justify_content, content: style.align_content,
        items: style.align_items, self_align: style.align_self }
}

fn initial() -> Alignment {
    Alignment { justify: FlexPack::Start, content: FlexPack::Stretch,
        items: FlexAlign::Stretch, self_align: None }
}

fn parent_values() -> Alignment {
    Alignment { justify: FlexPack::Between, content: FlexPack::Around,
        items: FlexAlign::End, self_align: Some(FlexAlign::Start) }
}

fn child_values() -> Alignment {
    Alignment { justify: FlexPack::End, content: FlexPack::Between,
        items: FlexAlign::Start, self_align: Some(FlexAlign::Center) }
}

fn centered() -> Alignment {
    Alignment { justify: FlexPack::Center, content: FlexPack::Center,
        items: FlexAlign::Center, self_align: Some(FlexAlign::Center) }
}

fn ended() -> Alignment {
    Alignment { justify: FlexPack::End, content: FlexPack::End,
        items: FlexAlign::End, self_align: Some(FlexAlign::End) }
}

fn replace(target: &mut Alignment, source: Alignment, property: &str) {
    match property {
        "justify-content" => target.justify = source.justify,
        "align-content" => target.content = source.content,
        "align-items" => target.items = source.items,
        "align-self" => target.self_align = source.self_align,
        _ => panic!("unknown alignment property"),
    }
}

fn all(value: &str) -> String {
    PROPERTIES.map(|property| format!("{property}:{value};")).join("")
}

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn at<'a>(doc: &Document, styles: &'a [Style], name: &str) -> &'a Style {
    &styles[id(doc, name)]
}

fn set_style(doc: &mut Document, node: usize, value: &str) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { panic!("element required") };
    if let Some(attribute) = attrs.iter_mut().find(|attribute| attribute.name.html_is("style")) {
        attribute.value = value.into();
    } else {
        attrs.push(crate::html::Attribute::unqualified("style".into(), value.into()));
    }
}

fn assert_target(property: &str, declarations: &str, expected: Alignment) {
    let doc = document(&format!("#parent{{{PARENT}}}#target{{{CHILD};{declarations}}}"),
        "<div id=parent><div id=target></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(alignment(at(&doc, &styles, "target")), expected, "{property}: {declarations}");
}

fn names(source: &str, expected: &[&str]) {
    assert_eq!(inline_declaration_names(source, &PROPERTIES, None).unwrap(),
        expected.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>(), "{source}");
}

#[test]
fn flex_alignment_justify_content_keywords_share_literal_and_variable_values() {
    for (value, expected) in PACK_VALUES {
        for variable in [false, true] {
            let source = if variable { format!("--chosen:{value};justify-content:var(--chosen)") }
                else { format!("justify-content:{value}") };
            let mut result = child_values(); result.justify = expected;
            assert_target("justify-content", &source, result);
        }
    }
}

#[test]
fn flex_alignment_align_content_keywords_keep_normal_as_stretch() {
    for (value, common) in PACK_VALUES {
        let expected = if value == "normal" { FlexPack::Stretch } else { common };
        for variable in [false, true] {
            let source = if variable { format!("--chosen:{value};align-content:var(--chosen)") }
                else { format!("align-content:{value}") };
            let mut result = child_values(); result.content = expected;
            assert_target("align-content", &source, result);
        }
    }
}

#[test]
fn flex_alignment_align_items_keywords_share_literal_and_variable_values() {
    for (value, expected) in ITEM_VALUES {
        for variable in [false, true] {
            let source = if variable { format!("--chosen:{value};align-items:var(--chosen)") }
                else { format!("align-items:{value}") };
            let mut result = child_values(); result.items = expected;
            assert_target("align-items", &source, result);
        }
    }
}

#[test]
fn flex_alignment_align_self_keywords_include_deferred_auto() {
    let values = ITEM_VALUES.into_iter().map(|(name, value)| (name, Some(value)))
        .chain(std::iter::once(("auto", None)));
    for (value, expected) in values {
        for variable in [false, true] {
            let source = if variable { format!("--chosen:{value};align-self:var(--chosen)") }
                else { format!("align-self:{value}") };
            let mut result = child_values(); result.self_align = expected;
            assert_target("align-self", &source, result);
        }
    }
}

#[test]
fn flex_alignment_initial_and_normal_values_retain_their_distinct_meanings() {
    let doc = document("#normal{justify-content:normal;align-content:normal;align-items:normal;align-self:normal}",
        "<div id=ordinary></div><div id=normal></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(alignment(at(&doc, &styles, "ordinary")), initial());
    let mut normal = initial(); normal.self_align = Some(FlexAlign::Stretch);
    assert_eq!(alignment(at(&doc, &styles, "normal")), normal);
}

#[test]
fn flex_alignment_longhands_do_not_naturally_inherit() {
    let doc = document(&format!("#parent{{{PARENT}}}#unset{{{CHILD};{}}}", all("unset")),
        "<div id=parent><div id=ordinary></div><div id=unset></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(alignment(at(&doc, &styles, "parent")), parent_values());
    for name in ["ordinary", "unset"] { assert_eq!(alignment(at(&doc, &styles, name)), initial(), "{name}"); }
}

#[test]
fn flex_alignment_literal_inherit_changes_only_the_named_longhand() {
    for property in PROPERTIES {
        let mut expected = child_values(); replace(&mut expected, parent_values(), property);
        assert_target(property, &format!("{property}:inherit"), expected);
    }
}

#[test]
fn flex_alignment_literal_initial_changes_only_the_named_longhand() {
    for property in PROPERTIES {
        let mut expected = child_values(); replace(&mut expected, initial(), property);
        assert_target(property, &format!("{property}:initial"), expected);
    }
}

#[test]
fn flex_alignment_literal_unset_changes_only_the_named_longhand() {
    for property in PROPERTIES {
        let mut expected = child_values(); replace(&mut expected, initial(), property);
        assert_target(property, &format!("{property}:unset"), expected);
    }
}

#[test]
fn flex_alignment_variable_inherit_copies_the_corresponding_parent_value() {
    for property in PROPERTIES {
        let mut expected = child_values(); replace(&mut expected, parent_values(), property);
        assert_target(property, &format!("{property}:var(--missing,InHeRiT)"), expected);
    }
}

#[test]
fn flex_alignment_variable_initial_and_unset_reset_only_the_target() {
    for property in PROPERTIES {
        for keyword in ["initial", "unset"] {
            let mut expected = child_values(); replace(&mut expected, initial(), property);
            assert_target(property, &format!("{property}:var(--missing,{keyword})"), expected);
        }
    }
}

#[test]
fn flex_alignment_multigeneration_inherit_uses_the_parents_completed_values() {
    let doc = document(&format!(concat!(
        "#grand{{--Main:space-evenly;--Cross:flex-end;--Items:center;--Self:flex-end}}",
        "#parent{{justify-content:var(--Main);align-content:var(--Cross);align-items:var(--Items);align-self:var(--Self)}}",
        "#child{{--Main:flex-start;--Cross:flex-start;--Items:flex-start;--Self:flex-start;{}}}",
        "#leaf{{{}}}"), all("inherit"), all("var(--missing,inherit)")),
        "<div id=grand><div id=parent><div id=child><div id=leaf></div></div></div></div>");
    let expected = Alignment { justify: FlexPack::Evenly, content: FlexPack::End,
        items: FlexAlign::Center, self_align: Some(FlexAlign::End) };
    let styles = compute(&doc, &HashMap::new());
    for name in ["parent", "child", "leaf"] { assert_eq!(alignment(at(&doc, &styles, name)), expected, "{name}"); }
}

#[test]
fn flex_alignment_inherited_align_self_auto_is_not_replaced_by_parent_align_items() {
    let doc = document(concat!(
        "#grand{align-items:center;align-self:flex-end}",
        "#parent{align-items:flex-start;align-self:auto}",
        "#child{align-items:flex-end;align-self:inherit}",
        "#leaf{align-items:center;align-self:var(--missing,inherit)}"),
        "<div id=grand><div id=parent><div id=child><div id=leaf></div></div></div></div>");
    let styles = compute(&doc, &HashMap::new());
    for name in ["parent", "child", "leaf"] { assert_eq!(at(&doc, &styles, name).align_self, None, "{name}"); }
    assert_eq!(at(&doc, &styles, "parent").align_items, FlexAlign::Start);
    assert_eq!(at(&doc, &styles, "child").align_items, FlexAlign::End);
}

#[test]
fn flex_alignment_root_literal_keywords_ignore_the_document_sentinel() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            let mut doc = crate::html::parse(&format!(
                "<!doctype html><html id=root style='{CHILD};{property}:{keyword}'><head></head><body></body></html>"));
            set_style(&mut doc, 0, PARENT);
            let mut expected = child_values(); replace(&mut expected, initial(), property);
            assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "root")), expected, "{property}:{keyword}");
        }
    }
}

#[test]
fn flex_alignment_root_variable_keywords_ignore_the_document_sentinel() {
    for property in PROPERTIES {
        for keyword in ["inherit", "initial", "unset"] {
            let mut doc = crate::html::parse(&format!(
                "<!doctype html><html id=root style='{CHILD};{property}:var(--missing,{keyword})'><head></head><body></body></html>"));
            set_style(&mut doc, 0, PARENT);
            let mut expected = child_values(); replace(&mut expected, initial(), property);
            assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "root")), expected, "{property}:{keyword}");
        }
    }
}

#[test]
fn flex_alignment_fragment_context_is_excluded_but_real_null_namespace_parent_counts() {
    for value in ["inherit", "var(--missing,inherit)"] {
        let mut doc = crate::html::parse_fragment(
            "<link id=sheet rel=stylesheet><section id=top><div id=real><span id=inner></span></div></section>", "div", false).unwrap();
        assert!(matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == "div"));
        assert_eq!(doc.nodes[0].namespace, crate::html::Namespace::None);
        let real = id(&doc, "real"); doc.nodes[real].namespace = crate::html::Namespace::None;
        let external = HashMap::from([(id(&doc, "sheet"), format!("div{{{PARENT}}}#top,#inner{{{}}}", all(value)))]);
        let styles = compute(&doc, &external);
        assert_eq!(alignment(at(&doc, &styles, "top")), initial());
        assert_eq!(alignment(at(&doc, &styles, "inner")), parent_values());
    }
}

#[test]
fn flex_alignment_custom_names_remain_case_sensitive_and_forward_values_resolve() {
    let doc = document(&format!(
        "#parent{{--Align:flex-end;--align:center}}#upper{{{}}}#lower{{{}}}#forward{{{}--later:flex-start}}",
        all("var(--Align)"), all("var(--align)"), all("var(--later)")),
        "<div id=parent><div id=upper></div><div id=lower></div><div id=forward></div></div>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(alignment(at(&doc, &styles, "upper")), ended());
    assert_eq!(alignment(at(&doc, &styles, "lower")), centered());
    assert_eq!(alignment(at(&doc, &styles, "forward")), Alignment {
        justify: FlexPack::Start, content: FlexPack::Start, items: FlexAlign::Start, self_align: Some(FlexAlign::Start) });
}

#[test]
fn flex_alignment_nested_fallbacks_comments_and_keyword_case_share_the_value_path() {
    let doc = document(&format!("#target{{{}}}", all("VaR(--missing, var(--other, /* fallback */ CeNtEr))")),
        "<div id=target></div>");
    assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "target")), centered());
    let doc = document(&format!("#target{{--chosen:/* before */ FleX-EnD /* after */;{}}}", all("var(--chosen)")),
        "<div id=target></div>");
    assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "target")), ended());
}

#[test]
fn flex_alignment_defined_empty_or_invalid_custom_values_do_not_use_fallbacks() {
    for property in PROPERTIES {
        for value in ["", "bogus", "center end"] {
            let mut expected = child_values(); replace(&mut expected, initial(), property);
            assert_target(property, &format!("--chosen:{value};{property}:var(--chosen,center)"), expected);
        }
    }
}

#[test]
fn flex_alignment_malformed_literals_preserve_earlier_valid_declarations() {
    for property in PROPERTIES {
        for invalid in ["center end", "inherit center", "unset center", "banana", "flex-/**/end", "'var(--x)'"] {
            assert_target(property, &format!("{property}:{invalid}!important"), child_values());
        }
    }
    for property in ["justify-content", "align-content", "align-items"] {
        assert_target(property, &format!("{property}:auto"), child_values());
    }
    for property in ["align-items", "align-self"] {
        assert_target(property, &format!("{property}:space-between"), child_values());
    }
}

#[test]
fn flex_alignment_missing_or_empty_fallback_winners_use_initial_values() {
    for property in PROPERTIES {
        for value in ["var(--missing)", "var(--missing,)"] {
            let mut expected = child_values(); replace(&mut expected, initial(), property);
            assert_target(property, &format!("{property}:{value}"), expected);
        }
    }
}

#[test]
fn flex_alignment_cycles_default_or_use_the_explicit_fallback() {
    for property in PROPERTIES {
        for (value, source) in [("var(--a)", initial()), ("var(--a,center)", centered())] {
            let mut expected = child_values(); replace(&mut expected, source, property);
            assert_target(property, &format!("--a:var(--b);--b:var(--a);{property}:{value}"), expected);
        }
    }
}

#[test]
fn flex_alignment_invalid_substitutions_cannot_join_tokens_or_revive_prior_values() {
    for property in PROPERTIES {
        for declarations in [
            format!("--chosen:center end;{property}:var(--chosen)"),
            format!("--chosen:'center';{property}:var(--chosen)"),
            format!("--prefix:flex;{property}:var(--prefix)-end"),
            format!("--prefix:cen;{property}:var(--prefix)ter"),
        ] {
            let mut expected = child_values(); replace(&mut expected, initial(), property);
            assert_target(property, &declarations, expected);
        }
    }
}

#[test]
fn flex_alignment_malformed_variable_syntax_preserves_the_previous_literal() {
    for property in PROPERTIES {
        for value in ["var(not-custom)", "var(--,center)", "var(--a --b)", "var()", "var (--missing)"] {
            assert_target(property, &format!("{property}:{value}"), child_values());
        }
    }
}

#[test]
fn flex_alignment_source_order_and_specificity_apply_to_variable_winners() {
    for property in PROPERTIES {
        let first = format!("div{{{CHILD}}}.target{{{property}:center}}#target{{{property}:var(--missing,flex-end)}}.target{{{property}:flex-start}}");
        let doc = document(&first, "<div id=target class=target></div>");
        let mut expected = child_values(); replace(&mut expected, ended(), property);
        assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "target")), expected);
        let doc = document(&format!("{first}#target{{{property}:var(--missing,center)}}"), "<div id=target class=target></div>");
        replace(&mut expected, centered(), property);
        assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "target")), expected);
    }
}

#[test]
fn flex_alignment_important_sheet_and_inline_declarations_keep_their_priority() {
    let doc = document(&format!(".target{{{}}}", all("var(--missing,center)!important")), &format!(
        "<div id=ordinary class=target style='{}'></div><div id=important class=target style='{}'></div>",
        all("flex-start"), all("var(--missing,flex-end)!important")));
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(alignment(at(&doc, &styles, "ordinary")), centered());
    assert_eq!(alignment(at(&doc, &styles, "important")), ended());
}

#[test]
fn flex_alignment_lower_priority_invalid_variables_cannot_reset_important_winners() {
    let doc = document(&format!("#target{{{}{}}}", all("center!important"), all("var(--missing)")),
        &format!("<div id=target style='{}'></div>", all("var(--missing)")));
    assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "target")), centered());
    let doc = document(&format!("#target{{{}{}}}", all("center"), all("var(--missing)!important")),
        &format!("<div id=target style='{}'></div>", all("flex-end")));
    assert_eq!(alignment(at(&doc, &compute(&doc, &HashMap::new()), "target")), initial());
}

#[test]
fn flex_alignment_invalid_variable_reset_preserves_other_computed_fields() {
    for property in PROPERTIES {
        let doc = document(&format!("#target{{{CHILD};display:flex;flex-flow:column-reverse wrap;width:23px;height:29px;margin:3px;gap:7px;color:#123456;{property}:var(--missing)}}"),
            "<div id=target></div>");
        let styles = compute(&doc, &HashMap::new()); let style = at(&doc, &styles, "target");
        let mut expected = child_values(); replace(&mut expected, initial(), property);
        assert_eq!(alignment(style), expected);
        assert_eq!((style.display, style.flex_direction, style.flex_wrap), (Display::Flex, FlexDirection::ColumnReverse, true));
        assert_eq!((style.width, style.height), (Length::Px(23.0), Length::Px(29.0)));
        assert_eq!(style.margin, [Length::Px(3.0); 4]);
        assert_eq!(style.gap, [Length::Px(7.0); 2]);
        assert_eq!(style.color, 0x123456);
    }
}

#[test]
fn flex_alignment_parent_changes_reuse_rules_and_recompute_explicit_inheritance() {
    let mut doc = document(&format!("#child{{{}}}", all("var(--missing,inherit)")),
        &format!("<div id=parent style='{PARENT}'><div id=child></div></div>"));
    let external = HashMap::new();
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(alignment(at(&doc, &first.styles, "child")), parent_values());
    let parent = id(&doc, "parent"); set_style(&mut doc, parent, &all("center"));
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    assert_eq!(alignment(at(&doc, &next.styles, "child")), centered());
    assert!(first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
    assert_eq!(alignment(at(&doc, &next.styles, "child")), alignment(at(&doc, &compute(&doc, &external), "child")));
}

#[test]
fn flex_alignment_changed_external_source_replaces_cached_variable_values() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=target></div>");
    let sheet = id(&doc, "sheet");
    let mut external = HashMap::from([(sheet, format!("#target{{{}}}", all("var(--missing,center)")))]);
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(alignment(at(&doc, &first.styles, "target")), centered());
    external.insert(sheet, format!("#target{{{}}}", all("var(--missing,flex-end)")));
    let next = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
    assert_eq!(alignment(at(&doc, &next.styles, "target")), ended());
    assert!(!first.stylesheet_cache.as_ref().unwrap().shares_rules(next.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn flex_alignment_bounded_substitution_refusal_resets_only_the_target_and_recovers() {
    let declarations = parse_declarations(&(0..257).map(|index| format!("--n{index}:center;")).collect::<String>());
    let blocked = custom_properties::Variables::cascade(&custom_properties::Variables::default(), declarations.iter(), &mut custom_properties::Budget::default());
    for property in PROPERTIES {
        let declaration = Declaration { name: property.into(), value: "var(--missing,center)".into(), important: false, stylesheet: None };
        for (work, copies, variables) in [
            (0, 100_000, custom_properties::Variables::default()),
            (100_000, 0, custom_properties::Variables::default()),
            (100_000, 100_000, blocked.clone()),
        ] {
            let mut style = Style::default();
            style.justify_content = FlexPack::End; style.align_content = FlexPack::Between;
            style.align_items = FlexAlign::Start; style.align_self = Some(FlexAlign::Center);
            style.width = Length::Px(23.0); style.height = Length::Px(29.0);
            style.color = 0x123456; style.margin = [Length::Px(3.0); 4];
            assert!(custom_properties::apply_box_with_parent(&mut style, &Style::default(), &declaration,
                &variables, &mut custom_properties::Budget::with_remaining(work, copies), true));
            let mut expected = child_values(); replace(&mut expected, initial(), property);
            assert_eq!(alignment(&style), expected, "{property}: work {work}, copies {copies}");
            assert_eq!((style.width, style.height, style.color, style.margin),
                (Length::Px(23.0), Length::Px(29.0), 0x123456, [Length::Px(3.0); 4]));
            assert!(custom_properties::apply_box_with_parent(&mut style, &Style::default(), &declaration,
                &custom_properties::Variables::default(), &mut custom_properties::Budget::default(), true));
            replace(&mut expected, centered(), property);
            assert_eq!(alignment(&style), expected);
        }
    }
}

#[test]
fn flex_alignment_cached_job_refusals_preserve_sources_and_allow_both_routes_to_recover() {
    let doc = document(&format!("#target{{{}width:23px}}", all("var(--missing,center)")), "<div id=target></div>");
    let external = HashMap::new(); let before = format!("{:?}", doc.nodes);
    let prior = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    for transform in [false, true] {
        for (mut work, mut copies) in [(0, usize::MAX), (usize::MAX, 0)] {
            let mut budget = CssBudget::new(&mut work, &mut copies);
            let result = if transform {
                compute_with_environment_and_properties_transform_budgeted_cached(&doc, &external, None, None, &mut budget, prior.stylesheet_cache.as_ref())
            } else {
                compute_with_environment_and_properties_budgeted_cached(&doc, &external, None, None, &mut budget, prior.stylesheet_cache.as_ref())
            };
            assert!(result.is_err()); assert_eq!(format!("{:?}", doc.nodes), before);
        }
        let (mut work, mut copies) = (usize::MAX, usize::MAX);
        let mut budget = CssBudget::new(&mut work, &mut copies);
        let recovered = if transform {
            compute_with_environment_and_properties_transform_budgeted_cached(&doc, &external, None, None, &mut budget, prior.stylesheet_cache.as_ref())
        } else {
            compute_with_environment_and_properties_budgeted_cached(&doc, &external, None, None, &mut budget, prior.stylesheet_cache.as_ref())
        }.unwrap();
        assert_eq!(alignment(at(&doc, &recovered.styles, "target")), centered());
        assert_eq!(at(&doc, &recovered.styles, "target").width, Length::Px(23.0));
        assert_eq!(format!("{:?}", doc.nodes), before);
        assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
    }
}

#[test]
fn flex_alignment_native_inline_names_admit_the_finite_values_and_css_wide_forms() {
    for property in PROPERTIES {
        for value in ["inherit", "initial", "unset", "var(--chosen)", "var(--missing,center)"] {
            names(&format!("{property}:{value}"), &[property]);
        }
    }
    for property in ["justify-content", "align-content"] {
        for (value, _) in PACK_VALUES { names(&format!("{property}:{value}"), &[property]); }
    }
    for property in ["align-items", "align-self"] {
        for (value, _) in ITEM_VALUES { names(&format!("{property}:{value}"), &[property]); }
    }
    names("align-self:auto", &["align-self"]);
}

#[test]
fn flex_alignment_native_inline_names_reject_bad_values_without_erasing_winner_order() {
    for property in PROPERTIES {
        for invalid in ["center end", "inherit center", "banana", "flex-/**/end", "'var(--x)'", "var(not-custom)"] {
            names(&format!("{property}:{invalid}"), &[]);
            let other = if property == "align-self" { "align-items" } else { "align-self" };
            names(&format!("{property}:center;{other}:center;{property}:{invalid}"), &[property, other]);
        }
    }
    for property in ["justify-content", "align-content", "align-items"] { names(&format!("{property}:auto"), &[]); }
    for property in ["align-items", "align-self"] { names(&format!("{property}:space-between"), &[]); }
    names("align-items:center!important;justify-content:center;align-items:flex-end;align-self:auto;align-content:normal",
        &["align-items", "justify-content", "align-self", "align-content"]);
}

#[test]
fn flex_alignment_native_inline_text_uses_the_same_longhand_admission() {
    let source = "ALIGN-CONTENT: NORMAL !important;align-items:center;align-self:var(--x,flex-end);justify-content:space-around;align-content:auto;align-items:space-between;align-self:banana;justify-content:inherit center;align-items:'var(--x)'";
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    assert_eq!(inline_text_declarations(source, &PROPERTIES, &mut CssBudget::new(&mut work, &mut copies)).unwrap(), vec![
        ("align-content".into(), "NORMAL".into(), true), ("align-items".into(), "center".into(), false),
        ("align-self".into(), "var(--x,flex-end)".into(), false), ("justify-content".into(), "space-around".into(), false),
    ]);
    for property in PROPERTIES {
        for value in ["inherit", "initial", "unset", "var(--missing,center)"] {
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            assert_eq!(inline_text_declarations(&format!("{property}:{value}"), &PROPERTIES,
                &mut CssBudget::new(&mut work, &mut copies)).unwrap(), vec![(property.into(), value.into(), false)]);
        }
    }
}

#[test]
fn flex_alignment_native_inline_admission_refuses_exhausted_budgets_and_recovers() {
    let source = "align-content:var(--a,normal);align-items:var(--b,center);align-self:var(--c,auto);justify-content:var(--d,space-between)";
    for name_api in [true, false] {
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
        ("align-content".into(), "var(--a,normal)".into(), false), ("align-items".into(), "var(--b,center)".into(), false),
        ("align-self".into(), "var(--c,auto)".into(), false), ("justify-content".into(), "var(--d,space-between)".into(), false),
    ]);
    assert!(work < usize::MAX && copies < usize::MAX);
}

#[test]
fn flex_alignment_warm_substitution_charges_expanded_values_on_both_budget_routes() {
    fn run(doc: &Document, transform: bool, budget: &mut CssBudget<'_>,
        cache: Option<&StylesheetCache>) -> Result<CascadeOutput, String> {
        if transform {
            compute_with_environment_and_properties_transform_budgeted_cached(doc, &HashMap::new(), None, None, budget, cache)
        } else {
            compute_with_environment_and_properties_budgeted_cached(doc, &HashMap::new(), None, None, budget, cache)
        }
    }
    fn set_class(doc: &mut Document, node: usize, value: &str) {
        let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { unreachable!() };
        attrs.iter_mut().find(|attribute| attribute.name.html_is("class")).unwrap().value = value.into();
    }
    for property in PROPERTIES {
        let sheet = format!(
            "html{{--empty:;--short:center;--large:{}center}}.smallval{{{property}:var(--short)}}.largeval{{{property}:var(--large)}}",
            "var(--empty)".repeat(80));
        let mut doc = document(&sheet, "<div id=target class=smallval style='width:23px;color:#123456'></div>");
        let target = id(&doc, "target");
        let prior = compute_with_environment_and_properties_cached(&doc, &HashMap::new(), None, None, None);
        for transform in [false, true] {
            set_class(&mut doc, target, "smallval");
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let control = run(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(control.stylesheet_cache.as_ref().unwrap()));
            let short = control.computed_properties.read(target, "--short").unwrap().unwrap();
            let large = control.computed_properties.read(target, "--large").unwrap().unwrap();
            assert_eq!((short.trim(), large.trim()), ("center", "center"));
            assert!(large.len() > short.len() + 64, "expanded-value fixture must retain additional copied bytes");
            let control_copies = usize::MAX - copies;
            set_class(&mut doc, target, "largeval");
            let before = format!("{:?}", doc.nodes);
            let (mut work, mut copies) = (usize::MAX, control_copies.checked_add(64).unwrap());
            assert!(run(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).is_err(),
                "{property}: expanded substitution must be admitted on transform route {transform}");
            assert_eq!(format!("{:?}", doc.nodes), before);
            let (mut work, mut copies) = (usize::MAX, usize::MAX);
            let recovered = run(&doc, transform, &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
            let mut expected = initial(); replace(&mut expected, centered(), property);
            assert_eq!(alignment(&recovered.styles[target]), expected);
            assert_eq!((recovered.styles[target].width, recovered.styles[target].color), (Length::Px(23.0), 0x123456));
            assert_eq!(format!("{:?}", doc.nodes), before);
        }
    }
}
