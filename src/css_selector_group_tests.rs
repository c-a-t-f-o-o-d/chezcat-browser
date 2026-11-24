//! Selector groups share matching, cascade, live DOM state and bounded queries.
use super::*;

const FUNCTIONS: [&str; 2] = ["is", "where"];

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!(
        "<!doctype html><html id=docroot><head><style>{sheet}</style></head><body>{body}</body></html>"
    ))
}

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn nodes(doc: &Document, names: &[&str]) -> Vec<usize> {
    names.iter().map(|name| id(doc, name)).collect()
}

fn query(doc: &Document, source: &str, candidates: &[usize]) -> Vec<usize> {
    let mut work = usize::MAX;
    query_all(doc, source, candidates, None, &mut work).unwrap()
}

fn topology(doc: &Document) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut parents = vec![None; doc.nodes.len()];
    let mut slots = vec![usize::MAX; doc.nodes.len()];
    for (parent, entry) in doc.nodes.iter().enumerate() {
        for (slot, &child) in entry.children.iter().enumerate() {
            parents[child] = Some(parent);
            slots[child] = slot;
        }
    }
    (parents, slots)
}

fn assert_matches(source: &str, body: &str, candidates: &[&str], expected: &[&str]) {
    assert!(valid_query_selector(source), "{source}");
    let doc = document(&format!("{source}{{width:37px}}"), body);
    let candidates = nodes(&doc, candidates);
    let expected = nodes(&doc, expected);
    assert_eq!(query(&doc, source, &candidates), expected, "{source}");
    let styles = compute(&doc, &HashMap::new());
    for node in candidates {
        assert_eq!(styles[node].width, if expected.contains(&node) { Length::Px(37.0) } else { Length::Auto }, "{source}: node {node}");
    }
}

fn assert_refused(source: &str) {
    assert!(!valid_query_selector(source), "{source}");
    let doc = document("", "<p id=target class=safe>Target</p>");
    let (parents, slots) = topology(&doc);
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    assert!(query_with_topology(&doc, source, vec![id(&doc, "target")], None,
        &mut work, &parents, &slots, false, &mut copies).is_err(), "{source}");
    let mut rules = Vec::new();
    parse_sheet(&format!(".safe,{source}{{width:99px}}"), &mut rules);
    assert!(rules.is_empty(), "invalid outer list retained a rule: {source}");
}

fn set_units(doc: &mut Document, node: usize, name: &str, units: Vec<u16>) {
    let NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else { unreachable!() };
    attrs.iter_mut().find(|attribute| attribute.name.html_is(name)).unwrap().value =
        crate::dom_string::DomString::from_units(&units, 4096).unwrap();
}

fn set_attribute(doc: &mut Document, node: usize, name: &str, value: &str) {
    set_units(doc, node, name, value.encode_utf16().collect());
}

#[test]
fn selector_groups_match_each_surviving_alternative() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(.one,#b)"),
            "<p id=a class=one>A</p><p id=b>B</p><p id=c>C</p>",
            &["a", "b", "c"], &["a", "b"]);
    }
}

#[test]
fn selector_groups_preserve_candidate_order_without_duplicate_results() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(.one,#a,#b,.both)"),
            "<p id=a class='one both'>A</p><p id=b class=both>B</p><p id=c>C</p>",
            &["b", "a", "c"], &["b", "a"]);
    }
}

#[test]
fn selector_groups_retain_outer_compound_and_attribute_constraints() {
    for function in FUNCTIONS {
        assert_matches(&format!("section .card:{function}(button.primary,[data-kind=action])"),
            concat!("<section><button id=a class='card primary'>A</button>",
                "<p id=b class=card data-kind=action>B</p><button id=c class=primary>C</button>",
                "<p id=d class=card>D</p></section><button id=e class='card primary'>E</button>"),
            &["a", "b", "c", "d", "e"], &["a", "b"]);
    }
}

#[test]
fn selector_groups_in_one_compound_intersect() {
    let body = "<p id=a class='first third'>A</p><p id=chosen class=second>B</p><p id=c class=first>C</p><p id=d class=third>D</p>";
    for source in [":is(.first,.second):where(.third,#chosen)", ":where(.first,.second):is(.third,#chosen)"] {
        assert_matches(source, body, &["a", "chosen", "c", "d"], &["a", "chosen"]);
    }
}

#[test]
fn selector_groups_match_complex_child_and_descendant_arguments() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(.panel > .item,.outer .deep)"),
            concat!("<section class=panel><p id=a class=item>A</p><div><p id=b class=item>B</p></div></section>",
                "<div class=outer><section><p id=c class=deep>C</p></section></div><p id=d class=deep>D</p>"),
            &["a", "b", "c", "d"], &["a", "c"]);
    }
}

#[test]
fn selector_groups_match_siblings_without_crossing_parent_boundaries() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(.lead + .card,.lead ~ .gap + .card)"),
            concat!("<h2 class=lead>Lead</h2> text <p id=a class=card>A</p><span class=gap>Gap</span>",
                "<p id=b class=card>B</p><section><p id=c class=card>C</p></section>"),
            &["a", "b", "c"], &["a", "b"]);
    }
}

#[test]
fn selector_groups_can_constrain_an_outer_combinator_subject() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(.panel,.other) > .item + .item"),
            concat!("<section class=panel><p id=a class=item>A</p><p id=b class=item>B</p></section>",
                "<section><p id=c class=item>C</p><p id=d class=item>D</p></section>",
                "<section class=other><p id=e class=item>E</p><p id=f class=item>F</p></section>"),
            &["a", "b", "c", "d", "e", "f"], &["b", "f"]);
    }
}

#[test]
fn selector_groups_nest_with_both_specificity_forms() {
    let body = "<p id=a class=first>A</p><p id=b class=second>B</p><p id=c class=third>C</p><p id=d>D</p>";
    for source in [":is(:where(.first,.second),.third)", ":where(:is(.first,.second),.third)", ":IS(:WhErE(.first,.second),.third)"] {
        assert_matches(source, body, &["a", "b", "c", "d"], &["a", "b", "c"]);
    }
}

#[test]
fn selector_groups_compose_with_strict_negation() {
    let body = "<p id=a class=first>A</p><p id=b class=second>B</p><p id=c class=third>C</p>";
    for (source, expected) in [
        (":not(:is(.first,.second))", vec!["c"]),
        (":not(:where(.first,.second))", vec!["c"]),
        (":is(:not(.first,.second),.first)", vec!["a", "c"]),
        (":where(:not(:is(.first,.second)),.second)", vec!["b", "c"]),
        (":not(:not(:is(.first,.second)))", vec!["a", "b"]),
    ] {
        assert_matches(source, body, &["a", "b", "c"], &expected);
    }
}

#[test]
fn selector_groups_discard_invalid_negation_branches_without_relaxing_not() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(:not(.missing,:unknown),.safe)"),
            "<p id=a class=safe>A</p><p id=b>B</p>", &["a", "b"], &["a"]);
        assert_refused(&format!(":not(:{function}(.safe),:unknown)"));
    }
}

#[test]
fn selector_groups_empty_arguments_match_nothing_and_negate_normally() {
    for function in FUNCTIONS {
        for arguments in ["", " ", "/**/"] {
            let source = format!(":{function}({arguments})");
            assert_matches(&source, "<p id=a>A</p><p id=b>B</p>", &["a", "b"], &[]);
            assert_matches(&format!(":not({source})"), "<p id=a>A</p><p id=b>B</p>", &["a", "b"], &["a", "b"]);
        }
    }
}

#[test]
fn selector_groups_all_invalid_arguments_remain_valid_and_match_nothing() {
    for function in FUNCTIONS {
        let source = format!(":{function}(:unknown,>.x,.1bad,::before,:not(),#)");
        assert_matches(&source, "<p id=a class=x>A</p><p id=b>B</p>", &["a", "b"], &[]);
        assert_matches(&format!(":not({source})"), "<p id=a>A</p>", &["a"], &["a"]);
    }
}

#[test]
fn selector_groups_ignore_empty_comma_members() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(,.first,,#b,)"),
            "<p id=a class=first>A</p><p id=b>B</p><p id=c>C</p>", &["a", "b", "c"], &["a", "b"]);
        assert_matches(&format!(":{function}(,,,)"), "<p id=a>A</p>", &["a"], &[]);
    }
}

#[test]
fn selector_groups_discard_unsupported_pseudo_classes() {
    for function in FUNCTIONS {
        for invalid in [":has(.safe)", ":lang(en)", ":nth-child(2 of .safe)", ":active", "missing|p"] {
            assert_matches(&format!(":{function}({invalid},.safe)"),
                "<p id=a class=safe>A</p><p id=b>B</p>", &["a", "b"], &["a"]);
        }
    }
}

#[test]
fn selector_groups_discard_pseudo_element_arguments() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(.safe,::before,p::after,:before,::unknown)"),
            "<p id=a class=safe>A</p><p id=b>B</p>", &["a", "b"], &["a"]);
        assert_matches(&format!(":{function}(::before,p::after)"), "<p id=a>A</p>", &["a"], &[]);
    }
}

#[test]
fn selector_groups_preserve_quoted_attribute_delimiters() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}([data-note='a,b)'],[data-note='/*,)]{{}}*/'])"),
            "<p id=a data-note='a,b)'>A</p><p id=b data-note='/*,)]{}*/'>B</p><p id=c data-note=other>C</p>",
            &["a", "b", "c"], &["a", "b"]);
    }
}

#[test]
fn selector_groups_keep_balanced_opaque_branches_separate() {
    for function in FUNCTIONS {
        for invalid in [r#":unknown("x,y)",[a,b],{key:value})"#, r#":unknown('a\'b,c)', nested([x,y]))"#, "[unknown=[a,b]]"] {
            assert_matches(&format!(":{function}({invalid},.safe)"),
                "<p id=a class=safe>A</p><p id=b>B</p>", &["a", "b"], &["a"]);
        }
    }
}

#[test]
fn selector_groups_discard_balanced_unsupported_attribute_escapes() {
    for function in FUNCTIONS {
        for invalid in [r#"[data-note="escaped\"quote"]"#, r#"[data-note="escaped\,comma"]"#] {
            assert_matches(&format!(":{function}({invalid},.safe)"),
                "<p id=a class=safe>A</p><p id=b data-note=other>B</p>", &["a", "b"], &["a"]);
        }
    }
}

#[test]
fn selector_groups_skip_comment_delimiters_without_inserting_combinators() {
    for function in FUNCTIONS {
        assert_matches(&format!(":/**/{function}(.safe/* ),[]{{}} */.on, .other)"),
            "<p id=a class='safe on'>A</p><div class=safe><p id=b class=on>B</p></div><p id=c class=other>C</p>",
            &["a", "b", "c"], &["a", "c"]);
    }
}

#[test]
fn selector_groups_comments_do_not_join_identifier_or_function_tokens() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(.sa/**/fe,:i/**/s(.safe),.other)"),
            "<p id=a class=safe>A</p><p id=b class=other>B</p>", &["a", "b"], &["b"]);
        assert_refused(&format!(":{function}/**/(.safe)"));
    }
}

#[test]
fn selector_groups_leave_outer_selector_lists_strict() {
    for function in FUNCTIONS {
        for source in [format!(":{function}(.safe),:unknown"), format!(":{function}(.safe),>.safe"), format!(":{function}(.safe),"), format!(",:{function}(.safe)")] {
            assert_refused(&source);
        }
    }
}

#[test]
fn selector_groups_reject_bad_strings_and_unmatched_closing_delimiters() {
    for function in FUNCTIONS {
        for source in [
            format!(":{function}(.safe,])"), format!(":{function}(.safe,}})"), format!(":{function}(.safe))"),
            format!(":{function}(:unknown(\"broken\nstring\"),.safe)"),
            format!(":{function}([data-note='broken\nstring'],.safe)"),
        ] {
            assert_refused(&source);
        }
    }
}

#[test]
fn selector_is_uses_the_strongest_surviving_argument_even_when_unmatched() {
    for (function, color) in [("is", 0x112233), ("where", 0x445566)] {
        let doc = document(&format!(":{function}(#absent,.target){{color:#112233}}.target.extra{{color:#445566}}"),
            "<p id=target class='target extra'>Target</p>");
        assert_eq!(compute(&doc, &HashMap::new())[id(&doc, "target")].color, color, "{function}");
    }
}

#[test]
fn selector_is_uses_argument_maximum_instead_of_sum() {
    let doc = document(":is(#ghost,#other,.target){color:#112233}#target.extra{color:#445566}",
        "<p id=target class='target extra'>Target</p>");
    assert_eq!(compute(&doc, &HashMap::new())[id(&doc, "target")].color, 0x445566);
}

#[test]
fn selector_where_zeroes_nested_ids_and_preserves_outer_specificity() {
    let doc = document(concat!(":where(#target,:is(#other,.target)){color:#112233}p{color:#445566}",
        ".target:where(#target){background-color:#112233}p{background-color:#445566}"),
        "<p id=target class=target>Target</p>");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[id(&doc, "target")].color, 0x445566);
    assert_eq!(styles[id(&doc, "target")].background, Some(0x112233));
}

#[test]
fn selector_groups_add_only_their_defined_specificity_to_outer_constraints() {
    for (source, expected) in [
        (":not(:where(#absent)).target", (0, 1, 0)),
        (":where(:not(#absent),#target).target", (0, 1, 0)),
        ("p:is(.target,#absent).extra", (1, 1, 1)),
        (":is(:where(#absent),.target)", (0, 1, 0)),
        (":not(:is(.absent,#absent))", (1, 0, 0)),
        (":is(#a,.a):is(#b,.b)", (2, 0, 0)),
    ] {
        assert_eq!(parse_selector(source).unwrap().specificity, expected, "{source}");
    }
}

#[test]
fn selector_groups_invalid_arguments_contribute_no_specificity() {
    let doc = document(":is(#ghost:unknown,.target){color:#112233}.target.extra{color:#445566}",
        "<p id=target class='target extra'>Target</p>");
    assert_eq!(compute(&doc, &HashMap::new())[id(&doc, "target")].color, 0x445566);
    assert_eq!(parse_selector(":is(#ghost:unknown)").unwrap().specificity, (0, 0, 0));
}

#[test]
fn selector_groups_respect_source_order_importance_and_inline_styles() {
    let doc = document(concat!(":is(.target){color:#112233}.target{color:#445566}",
        ":where(#normal,#important){width:37px!important}",
        ":is(#normal,#important){height:41px}"),
        concat!("<p id=normal class=target style='width:11px;height:13px'>Normal</p>",
            "<p id=important class=target style='width:17px!important;height:19px'>Important</p>"));
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[id(&doc, "normal")].color, 0x445566);
    assert_eq!(styles[id(&doc, "normal")].width, Length::Px(37.0));
    assert_eq!(styles[id(&doc, "normal")].height, Length::Px(13.0));
    assert_eq!(styles[id(&doc, "important")].width, Length::Px(17.0));
    assert_eq!(styles[id(&doc, "important")].height, Length::Px(19.0));
}

#[test]
fn selector_groups_root_predicates_distinguish_documents_and_fragments() {
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}(:root,.safe)"), "<p id=a class=safe>A</p><p id=b>B</p>",
            &["docroot", "a", "b"], &["docroot", "a"]);
        let doc = crate::html::parse_fragment("<p id=a class=safe>A</p><p id=b>B</p>", "div", false).unwrap();
        let candidates = nodes(&doc, &["a", "b"]);
        assert_eq!(query(&doc, &format!(":{function}(:root,.safe)"), &candidates), vec![id(&doc, "a")]);
        assert!(query(&doc, &format!(":{function}(:root)"), &candidates).is_empty());
    }
}

#[test]
fn selector_groups_use_live_checkedness_and_native_disabledness() {
    let doc = document("", concat!("<input id=a type=checkbox checked><input id=b type=checkbox>",
        "<fieldset disabled><legend><button id=c>C</button></legend><button id=d>D</button></fieldset>",
        "<div id=e disabled>E</div>"));
    let candidates = nodes(&doc, &["a", "b", "c", "d", "e"]);
    let b = id(&doc, "b");
    let checked = |node| Some(node == b);
    for function in FUNCTIONS {
        let source = format!(":{function}(:checked,:disabled)");
        assert_eq!(query(&doc, &source, &candidates), nodes(&doc, &["a", "d"]));
        let mut work = usize::MAX;
        assert_eq!(query_all(&doc, &source, &candidates, Some(&checked), &mut work).unwrap(), nodes(&doc, &["b", "d"]));
    }
}

#[test]
fn selector_groups_use_current_focus_focus_within_and_hover() {
    let doc = document("", "<section id=panel><button id=button>Button</button></section><p id=other>Other</p>");
    let candidates = nodes(&doc, &["panel", "button", "other"]);
    let button = id(&doc, "button");
    for function in FUNCTIONS {
        assert!(query(&doc, &format!(":{function}(:focus,:hover)"), &candidates).is_empty());
        doc.native_focus.set(Some(button));
        assert_eq!(query(&doc, &format!(":{function}(:focus)"), &candidates), vec![button]);
        assert_eq!(query(&doc, &format!(":{function}(:focus-within)"), &candidates), nodes(&doc, &["panel", "button"]));
        doc.native_hover.set(Some(button));
        assert_eq!(query(&doc, &format!(":{function}(:hover)"), &candidates), nodes(&doc, &["panel", "button"]));
        doc.native_focus.set(None);
        doc.native_hover.set(None);
    }
}

#[test]
fn selector_groups_track_nested_hover_dependencies_when_reusing_rules() {
    let doc = document("#target:is(.never,:where(:hover)){width:37px}", "<p id=target>Target</p>");
    let target = id(&doc, "target");
    let external = HashMap::new();
    let initial = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert!(initial.has_hover);
    assert_eq!(initial.styles[target].width, Length::Auto);
    doc.native_hover.set(Some(target));
    let hovered = compute_with_environment_and_properties_cached(&doc, &external, None, None, initial.stylesheet_cache.as_ref());
    assert!(hovered.has_hover);
    assert_eq!(hovered.styles[target].width, Length::Px(37.0));
    assert!(initial.stylesheet_cache.as_ref().unwrap().shares_rules(hovered.stylesheet_cache.as_ref().unwrap()));
    doc.native_hover.set(None);
    let cleared = compute_with_environment_and_properties_cached(&doc, &external, None, None, hovered.stylesheet_cache.as_ref());
    assert_eq!(cleared.styles[target].width, Length::Auto);
}

#[test]
fn selector_groups_index_only_mandatory_outer_keys_and_recheck_current_classes() {
    let mut doc = document(concat!(".target{width:5px}:is(.a,#b){width:37px}",
        ".target:is(.a,#b){height:29px}:where(.a,#b){background-color:#112233}"),
        "<p id=a class='target a'>A</p><p id=b class=target>B</p><p id=c class=target>C</p>");
    let (a, b, c) = (id(&doc, "a"), id(&doc, "b"), id(&doc, "c"));
    let external = HashMap::new();
    let initial = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    for node in [a, b] {
        assert_eq!(initial.styles[node].width, Length::Px(37.0));
        assert_eq!(initial.styles[node].height, Length::Px(29.0));
        assert_eq!(initial.styles[node].background, Some(0x112233));
    }
    assert_eq!(initial.styles[c].width, Length::Px(5.0));
    assert_eq!(initial.styles[c].height, Length::Auto);
    set_attribute(&mut doc, a, "class", "target other");
    let changed = compute_with_environment_and_properties_cached(&doc, &external, None, None, initial.stylesheet_cache.as_ref());
    assert!(initial.stylesheet_cache.as_ref().unwrap().shares_rules(changed.stylesheet_cache.as_ref().unwrap()));
    assert_eq!(changed.styles[a].width, Length::Px(5.0));
    assert_eq!(changed.styles[a].height, Length::Auto);
    assert_eq!(changed.styles[b].width, Length::Px(37.0));
    assert_eq!(query(&doc, ":is(.a,#b)", &[a, b, c]), vec![b]);
}

#[test]
fn selector_groups_external_source_changes_invalidate_cached_rules() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet href=style.css><p id=target class=target>Target</p>");
    let (sheet, target) = (id(&doc, "sheet"), id(&doc, "target"));
    let mut external = HashMap::new();
    external.insert(sheet, ":is(.target){width:37px}".to_owned());
    let initial = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(initial.styles[target].width, Length::Px(37.0));
    external.insert(sheet, ":where(.target){width:53px}".to_owned());
    let changed = compute_with_environment_and_properties_cached(&doc, &external, None, None, initial.stylesheet_cache.as_ref());
    assert_eq!(changed.styles[target].width, Length::Px(53.0));
    assert!(!initial.stylesheet_cache.as_ref().unwrap().shares_rules(changed.stylesheet_cache.as_ref().unwrap()));
}

#[test]
fn selector_groups_match_class_tokens_without_lossy_utf16_aliases() {
    let mut doc = document("", "<p id=target class=original>Target</p>");
    let target = id(&doc, "target");
    set_units(&mut doc, target, "class", vec![65, 0xd800, 122, 32, 111, 107]);
    for function in FUNCTIONS {
        assert!(query(&doc, &format!(":{function}(.A�z)"), &[target]).is_empty());
        assert_eq!(query(&doc, &format!(":{function}(.missing,.ok)"), &[target]), vec![target]);
    }
    set_attribute(&mut doc, target, "class", "A😺 ok");
    for function in FUNCTIONS {
        assert_eq!(query(&doc, &format!(":{function}(.A😺)"), &[target]), vec![target]);
    }
}

#[test]
fn selector_groups_read_authoritative_raw_id_values() {
    let mut doc = document("", "<p id=target>Target</p>");
    let target = id(&doc, "target");
    set_units(&mut doc, target, "id", vec![65, 0xd800, 122]);
    for function in FUNCTIONS {
        assert!(query(&doc, &format!(":{function}(#A�z,#target)"), &[target]).is_empty());
    }
    set_attribute(&mut doc, target, "id", "A😺");
    for function in FUNCTIONS {
        assert_eq!(query(&doc, &format!(":{function}(#A😺)"), &[target]), vec![target]);
        assert!(query(&doc, &format!(":{function}(#target)"), &[target]).is_empty());
    }
}

#[test]
fn selector_groups_share_unicode_and_namespace_sensitive_type_matching() {
    let mut doc = document("", "<div id=target class=東京>Target</div><svg id=foreign></svg>");
    let (target, foreign) = (id(&doc, "target"), id(&doc, "foreign"));
    for function in FUNCTIONS {
        assert_eq!(query(&doc, &format!(":{function}(DIV.東京)"), &[target]), vec![target]);
        assert_eq!(query(&doc, &format!(":{function}(svg)"), &[foreign]), vec![foreign]);
        assert!(query(&doc, &format!(":{function}(SVG)"), &[foreign]).is_empty());
    }
    doc.kind = crate::html::DocumentKind::Xml;
    let NodeKind::Element { tag, .. } = &mut doc.nodes[target].kind else { unreachable!() };
    *tag = "Widget".into();
    for function in FUNCTIONS {
        assert_eq!(query(&doc, &format!(":{function}(Widget.東京)"), &[target]), vec![target]);
        assert!(query(&doc, &format!(":{function}(widget.東京)"), &[target]).is_empty());
    }
}

#[test]
fn selector_groups_use_recomputed_topology_after_reparenting_and_detachment() {
    let mut doc = document("", "<section id=left class=left><p id=a class=item>A</p><p id=b class=item>B</p></section><section id=right class=right></section>");
    let (left, right, a, b) = (id(&doc, "left"), id(&doc, "right"), id(&doc, "a"), id(&doc, "b"));
    let run = |doc: &Document, source: &str, first| {
        let (parents, slots) = topology(doc);
        let (mut work, mut copies) = (usize::MAX, usize::MAX);
        query_with_topology(doc, source, vec![b, a], None, &mut work, &parents, &slots, first, &mut copies).unwrap()
    };
    assert_eq!(run(&doc, ":is(.left > .item + .item,.right > .item)", false), vec![b]);
    doc.nodes[left].children.retain(|&node| node != a);
    doc.nodes[right].children.push(a);
    assert_eq!(run(&doc, ":is(.left > .item + .item,.right > .item)", false), vec![a]);
    assert_eq!(run(&doc, ":where(.item)", true), vec![b]);
    doc.nodes[right].children.retain(|&node| node != a);
    assert_eq!(run(&doc, ":where(.right > .item)", false), Vec::<usize>::new());
    assert_eq!(run(&doc, ":is(.item)", false), vec![b, a]);
    assert!(run(&doc, ":is(:root)", false).is_empty());
}

#[test]
fn selector_groups_retain_shared_nesting_capacity_without_forgiving_refusal() {
    let nested = |depth: usize| {
        let mut source = ".safe".to_owned();
        for index in 0..depth {
            source = format!(":{}({source})", if index % 2 == 0 { "is" } else { "where" });
        }
        source
    };
    assert_matches(&nested(MAX_SELECTOR_NESTING), "<p id=a class=safe>A</p>", &["a"], &["a"]);
    assert_refused(&nested(MAX_SELECTOR_NESTING + 1));
    assert_refused(&format!(":is(.safe,{})", nested(MAX_SELECTOR_NESTING)));
}

#[test]
fn selector_groups_count_every_attempted_member_against_list_capacity() {
    for function in FUNCTIONS {
        let mut members = vec![":unknown"; 15];
        members.push(".safe");
        assert_matches(&format!(":{function}({})", members.join(",")), "<p id=a class=safe>A</p>", &["a"], &["a"]);
        members.insert(0, ":unknown");
        assert_refused(&format!(":{function}({})", members.join(",")));
        assert_matches(&format!(":{function}({}.safe)", ",".repeat(15)), "<p id=a class=safe>A</p>", &["a"], &["a"]);
        assert_refused(&format!(":{function}({}.safe)", ",".repeat(16)));
        assert_refused(&format!(":{function}(.safe,:is({}))", members.join(",")));
    }
}

#[test]
fn selector_groups_preserve_complex_selector_compound_capacity() {
    let body = format!("{}<p id=target class=safe>Target</p>{}", "<section>".repeat(11), "</section>".repeat(11));
    let accepted = format!("{}p", "section > ".repeat(11));
    let refused = format!("{}p", "section > ".repeat(12));
    for function in FUNCTIONS {
        assert_matches(&format!(":{function}({accepted})"), &body, &["target"], &["target"]);
        assert_refused(&format!(":{function}(.safe,{refused})"));
    }
}

#[test]
fn selector_groups_preserve_effective_and_raw_source_byte_limits() {
    for function in FUNCTIONS {
        let prefix = format!(":{function}(.");
        let class = "x".repeat(MAX_SELECTOR_BYTES - prefix.len() - 1);
        let source = format!("{prefix}{class})");
        assert_eq!(source.len(), MAX_SELECTOR_BYTES);
        assert_matches(&source, &format!("<p id=target class={class}>Target</p>"), &["target"], &["target"]);
        assert_refused(&format!("{prefix}{class}x)"));
        let base = format!(":{function}(/**/.safe)");
        let source = format!(":{function}(/*{}*/.safe)", "x".repeat(MAX_SELECTOR_BYTES * 16 - base.len()));
        assert_eq!(source.len(), MAX_SELECTOR_BYTES * 16);
        let doc = document("", "<p id=target class=safe>Target</p>");
        assert!(valid_query_selector(&source));
        assert_eq!(query(&doc, &source, &[id(&doc, "target")]), vec![id(&doc, "target")]);
        assert!(!valid_query_selector(&(source + " ")));
    }
}

#[test]
fn selector_groups_never_invert_work_exhaustion_into_a_match() {
    let doc = document("", "<p id=target class=target>Target</p>");
    let target = id(&doc, "target");
    let parents = vec![None; doc.nodes.len()];
    let previous = vec![None; doc.nodes.len()];
    let context = MatchContext { doc: &doc, parents: &parents, previous_elements: &previous,
        sibling_slots: None, checked: None, charge_text: false };
    for source in [":not(:is(.target,.missing))", ":not(:where(.target))", ":is(:not(.target),.missing)"] {
        let selector = parse_selector(source).unwrap();
        let mut work = 1;
        assert!(!matches_selector(&selector, target, &context, &mut work), "{source}");
        assert_eq!(work, 0);
        let (parents, slots) = topology(&doc);
        let (mut work, mut copies) = (1, usize::MAX);
        let error = query_with_topology(&doc, source, vec![target], None, &mut work, &parents, &slots, false, &mut copies).unwrap_err();
        assert!(error.contains("work"), "{error}");
        assert!(query(&doc, source, &[target]).is_empty());
    }
}

#[test]
fn selector_groups_admit_query_storage_before_matching_and_recover_with_fresh_allowance() {
    let doc = document("", "<p id=target class=target>Target</p>");
    let target = id(&doc, "target");
    let (parents, slots) = topology(&doc);
    for function in FUNCTIONS {
        let source = format!(":{function}(.missing,.target)");
        let (mut work, mut copies) = (usize::MAX, 0);
        let error = query_with_topology(&doc, &source, vec![target], None,
            &mut work, &parents, &slots, false, &mut copies).unwrap_err();
        assert!(error.contains("copy"), "{error}");
        assert_eq!(copies, 0);
        let (mut work, mut copies) = (usize::MAX, usize::MAX);
        assert_eq!(query_with_topology(&doc, &source, vec![target], None,
            &mut work, &parents, &slots, false, &mut copies).unwrap(), vec![target]);
        assert!(copies < usize::MAX);
    }
    let (mut work, mut copies) = (usize::MAX, 0);
    assert_eq!(query_with_topology(&doc, ".target", vec![target], None,
        &mut work, &parents, &slots, false, &mut copies).unwrap(), vec![target]);
    assert_eq!(copies, 0);
}

#[test]
fn selector_groups_css_budget_refusal_preserves_sources_and_allows_cached_recovery() {
    let doc = document(":is(.target,#missing){width:37px}:where(.target){height:41px}", "<p id=target class=target>Target</p>");
    let target = id(&doc, "target");
    let external = HashMap::new();
    let original = format!("{:?}", doc.nodes);
    let prior = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    for (mut work, mut copies) in [(0, usize::MAX), (usize::MAX, 0)] {
        assert!(compute_with_environment_and_properties_budgeted_cached(&doc, &external, None, None,
            &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).is_err());
        assert_eq!(format!("{:?}", doc.nodes), original);
    }
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    let recovered = compute_with_environment_and_properties_budgeted_cached(&doc, &external, None, None,
        &mut CssBudget::new(&mut work, &mut copies), prior.stylesheet_cache.as_ref()).unwrap();
    assert_eq!(recovered.styles[target].width, Length::Px(37.0));
    assert_eq!(recovered.styles[target].height, Length::Px(41.0));
    assert!(prior.stylesheet_cache.as_ref().unwrap().shares_rules(recovered.stylesheet_cache.as_ref().unwrap()));
}
