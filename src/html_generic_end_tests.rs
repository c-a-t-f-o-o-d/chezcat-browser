//! Draft original oracles for native in-body "any other end tag" handling.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

fn id(doc: &Document, value: &str) -> usize {
    (1..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(value)).unwrap()
}
fn element(doc: &Document, name: &str) -> usize {
    (1..doc.nodes.len()).find(|&n| matches!(&doc.nodes[n].kind, NodeKind::Element { tag, .. } if tag == name)).unwrap()
}
fn contents(doc: &Document, node: usize) -> String {
    doc.nodes[node].children.iter().map(|&child| tree(doc, child)).collect()
}
fn tree(doc: &Document, node: usize) -> String {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", contents(doc, node)),
        NodeKind::Text(text) => text.as_str().to_owned(),
        NodeKind::Comment(text) => format!("<!--{text}-->"),
        _ => panic!("unexpected node in generic-end tree fixture"),
    }
}
fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 1000, &|| Ok(())).unwrap().0]
}
fn native_host(source: &str, expected: &str) {
    for doc in documents(source).into_iter().chain([parse(source)]) {
        assert_eq!(tree(&doc, id(&doc, "host")), expected, "{source}");
    }
}
fn checked_host(source: &str, expected: &str) {
    native_host(source, expected);
    for context in ["div", "template"] { for scripting in [false, true] {
        let doc = parse_fragment(source, context, scripting).unwrap();
        assert_eq!(tree(&doc, id(&doc, "host")), expected, "{source}");
    }}
}
fn unique_parents(doc: &Document) {
    let mut counts = vec![0; doc.nodes.len()];
    for parent in &doc.nodes { for &child in &parent.children { counts[child] += 1; } }
    assert!(counts[1..].iter().all(|&count| count == 1));
}

#[test]
fn generic_end_custom_and_inline_targets_stop_at_special_descendants() {
    for target in ["span", "label", "output", "datalist", "x-card", "x-cardé", "sarcasm"] {
        checked_host(&format!("<div id=host><{target}>A<section>B</{target}>C</section>D</{target}>E"),
            &format!("<div><{target}>A<section>BC</section>D</{target}>E</div>"));
    }
}

#[test]
fn generic_end_special_barriers_have_no_list_start_exceptions() {
    for barrier in ["address", "article", "aside", "blockquote", "button", "center", "dd",
        "details", "dir", "div", "dl", "dt", "fieldset", "figcaption", "figure", "footer",
        "form", "h1", "h6", "header", "hgroup", "li", "main", "marquee", "menu", "nav",
        "object", "ol", "p", "pre", "search", "section", "summary", "ul"] {
        checked_host(&format!("<div id=host><span>A<{barrier}>B</span>C</{barrier}>D</span>E"),
            &format!("<div><span>A<{barrier}>BC</{barrier}>D</span>E</div>"));
    }
    native_host("<div id=host><span>A<listing>B</span>C</listing>D</span>E",
        "<div><span>A<listing>BC</listing>D</span>E</div>");
}

#[test]
fn generic_end_native_option_and_optgroup_targets_use_current_in_body_rule() {
    for target in ["option", "optgroup"] {
        let source = format!("<div id=host><select><{target}>A<section>B</{target}>C</section>D</{target}>E</select>F");
        native_host(&source, &format!("<div><select><{target}>A<section>BC</section>D</{target}>E</select>F</div>"));
        assert!(parse_fragment(&source, "div", true).is_err());
    }
    checked_host("<div id=host><select><optgroup><option>A</option><option>B</optgroup></select>C",
        "<div><select><optgroup><option>A</option><option>B</option></optgroup></select>C</div>");
}

#[test]
fn generic_end_ruby_and_annotation_targets_stop_on_special_descendants() {
    checked_host("<div id=host><ruby>A<section>B</ruby>C</section>D</ruby>E",
        "<div><ruby>A<section>BC</section>D</ruby>E</div>");
    for target in ["rb", "rp", "rt", "rtc"] {
        checked_host(&format!("<div id=host><ruby><{target}>A<section>B</{target}>C</section>D</{target}>E</ruby>F"),
            &format!("<div><ruby><{target}>A<section>BC</section>D</{target}>E</ruby>F</div>"));
    }
}

#[test]
fn generic_end_traverses_non_special_wrappers_and_implied_end_elements() {
    for wrapper in ["span", "label", "output", "datalist", "dialog", "ruby", "rb", "rp", "rt", "rtc", "x-inner"] {
        checked_host(&format!("<div id=host><x-outer>A<{wrapper}>B</x-outer>C"),
            &format!("<div><x-outer>A<{wrapper}>B</{wrapper}></x-outer>C</div>"));
    }
    for wrapper in ["option", "optgroup"] {
        native_host(&format!("<div id=host><x-outer>A<{wrapper}>B</x-outer>C"),
            &format!("<div><x-outer>A<{wrapper}>B</{wrapper}></x-outer>C</div>"));
    }
}

#[test]
fn generic_end_matches_the_nearest_exact_target() {
    for target in ["span", "label", "x-card"] {
        checked_host(&format!("<div id=host><{target} id=outer>A<span><{target} id=inner>B</{target}>C</span>D</{target}>E"),
            &format!("<div><{target}>A<span><{target}>B</{target}>C</span>D</{target}>E</div>"));
    }
}

#[test]
fn generic_end_disabled_noscript_is_a_barrier_but_its_own_target_matches_first() {
    for (source, expected) in [
        ("<div id=host><span>A<noscript>B</span>C</noscript>D</span>E",
         "<div><span>A<noscript>BC</noscript>D</span>E</div>"),
        ("<div id=host><noscript>A<span>B</noscript>C",
         "<div><noscript>A<span>B</span></noscript>C</div>"),
    ] {
        native_host(source, expected);
        for context in ["div", "template"] {
            let doc = parse_fragment(source, context, false).unwrap();
            assert!(!doc.scripting_enabled);
            assert_eq!(tree(&doc, id(&doc, "host")), expected);
        }
        let enabled=parse_fragment(source,"div",true).unwrap();
        let host=element(&enabled,"noscript");
        let raw=source.split_once("<noscript>").unwrap().1.split_once("</noscript>").unwrap().0;
        let [child]=enabled.nodes[host].children.as_slice() else{panic!("one literal enabled noscript Text")};
        assert!(matches!(&enabled.nodes[*child].kind,NodeKind::Text(text)if text==raw));
        assert!(enabled.scripting_enabled);
    }
}

#[test]
fn generic_end_template_boundary_protects_outer_targets_and_allows_inner_targets() {
    for (source, expected) in [
        ("<div id=host><span>A<template>B</span>C</template>D</span>E",
         "<div><span>A<template>BC</template>D</span>E</div>"),
        ("<div id=host><span>A<template><span>B</span>C</template>D</span>E",
         "<div><span>A<template><span>B</span>C</template>D</span>E</div>"),
        ("<div id=host><template><x-card>A<section>B</x-card>C</section>D</x-card>E</template>F",
         "<div><template><x-card>A<section>BC</section>D</x-card>E</template>F</div>"),
    ] {
        for doc in documents(source).into_iter().chain([parse_fragment(source, "template", true).unwrap()]) {
            assert_eq!(tree(&doc, id(&doc, "host")), expected, "{source}");
            unique_parents(&doc);
        }
    }
}

#[test]
fn generic_end_table_cell_and_caption_routes_keep_outer_ownership() {
    for (source, expected) in [
        ("<div id=host><span>A<table><tr><td><label>B</span>C</label></td></tr></table>D</span>E",
         "<div><span>A<table><tbody><tr><td><label>BC</label></td></tr></tbody></table>D</span>E</div>"),
        ("<div id=host><span>A<table><caption><label>B</span>C</label></caption></table>D</span>E",
         "<div><span>A<table><caption><label>BC</label></caption></table>D</span>E</div>"),
    ] {
        for doc in documents(source) {
            assert_eq!(tree(&doc, id(&doc, "host")), expected);
            unique_parents(&doc);
        }
    }
}

#[test]
fn generic_end_fostered_native_wrapper_retains_special_descendants_and_following_text() {
    let source = "<body id=host><table><span>A<section>B</span>C</section>D</span></table>E";
    for doc in documents(source) {
        assert_eq!(tree(&doc, id(&doc, "host")),
            "<body><span>A<section>BC</section>D</span><table></table>E</body>");
        unique_parents(&doc);
    }
}

#[test]
fn generic_end_preserves_formatting_identity_after_blocked_and_successful_ends() {
    let source = "<div id=host><span><b data-note=kept>A<section>B</span>C</section>D</span>E";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><span><b>A<section>BC</section>D</b></span><b>E</b></div>");
        let copies: Vec<_> = (1..doc.nodes.len()).filter(|&n| doc.attr(n, "data-note") == Some("kept")).collect();
        assert_eq!(copies.len(), 2); assert_ne!(copies[0], copies[1]);
        unique_parents(&doc);
    }
    let source = "<div id=host><b><i>A</b>B</i>C";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><b><i>A</i></b><i>B</i>C</div>");
    }
}

#[test]
fn generic_end_specialized_marker_form_and_template_routes_remain_separate() {
    for target in ["applet", "marquee", "object"] {
        let source = format!("<div id=host><{target}><b data-note=kept>A<section>B</{target}>C");
        for doc in documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
            assert_eq!(tree(&doc, id(&doc, "host")), format!("<div><{target}><b>A<section>B</section></b></{target}>C</div>"));
            assert_eq!((1..doc.nodes.len()).filter(|&n| doc.attr(n, "data-note") == Some("kept")).count(), 1);
        }
    }
    let source = "<div id=host><template><form><section>A</form>B</template>C";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "template", true).unwrap()]) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><template><form><section>A</section></form>B</template>C</div>");
    }
    let source = "<div id=host><template><table><tr><td><span>A</template><span>B";
    for doc in documents(source) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><template><table><tbody><tr><td><span>A</span></td></tr></tbody></table></template><span>B</span></div>");
    }
}

#[test]
fn generic_end_native_identity_keeps_ascii_case_attributes_and_namespace_rules() {
    let source = "<div id=host><SPAN id=target data-note='a&amp;b' DATA-NOTE=ignored xmlns='urn:other'>A<section>B</sPaN>C</section>D</span>E";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let target = id(&doc, "target");
        assert_eq!(doc.nodes[target].namespace, Namespace::Html);
        assert_eq!(doc.attr(target, "data-note"), Some("a&b"));
        assert_eq!(doc.attr(target, "xmlns"), Some("urn:other"));
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><span>A<section>BC</section>D</span>E</div>");
    }
}

#[test]
fn generic_end_preserves_current_foreign_exact_name_fallback() {
    let source = "<div id=host><math><span>A<section>B</span>C</section></math><span>D<section>E</span>F</section>G</span>H";
    for doc in documents(source).into_iter().chain([parse(source)]) {
        let math = element(&doc, "math");
        assert_eq!(contents(&doc, math), "<span>A<section>B</section></span>C");
        assert_eq!(doc.nodes[doc.nodes[math].children[0]].namespace, Namespace::UnknownForeign);
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><math><span>A<section>B</section></span>C</math><span>D<section>EF</section>G</span>H</div>");
    }
}

#[test]
fn generic_end_preserves_uncertain_foreign_escape_fallback() {
    let source = "<div id=host><span id=outer><math><section>A</span><span id=uncertain>B<section>C</span>D";
    for doc in documents(source).into_iter().chain([parse(source)]) {
        assert_eq!(doc.nodes[id(&doc, "outer")].namespace, Namespace::Html);
        assert_eq!(doc.nodes[id(&doc, "uncertain")].namespace, Namespace::UnknownForeign);
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><span><math><section>A</section></math></span><span>B<section>C</section></span>D</div>");
    }
}

#[test]
fn generic_end_complete_exact_tokens_ignore_end_attributes_without_partial_dispatch() {
    for tail in [" data-note='>'>", " data-note=x>", "/>"] {
        checked_host(&format!("<div id=host><span>A<section>B</span{tail}C</section>D</span>E"),
            "<div><span>A<section>BC</section>D</span>E</div>");
        checked_host(&format!("<div id=host><span>A</span{tail}B"), "<div><span>A</span>B</div>");
    }
    for tail in [".more>", "é>"] {
        checked_host(&format!("<div id=host><span>A</span{tail}B"), "<div><span>AB</span></div>");
    }
    for tail in ["", " data-note=unfinished", " data-note='unfinished"] {
        checked_host(&format!("<div id=host><span>A</span{tail}"), "<div><span>A</span></div>");
    }
}

#[test]
fn generic_end_missing_targets_and_synthetic_context_roots_do_not_allocate() {
    let ends = "</span></label></x-card></ruby></rb></rp></rt></rtc></sarcasm>";
    for context in ["div", "span", "label", "a", "b", "em", "section", "template"] {
        let doc = parse_fragment(&format!("A{ends}B"), context, true).unwrap();
        assert_eq!(doc.nodes.len(), 2); assert_eq!(contents(&doc, 0), "AB");
        assert_eq!(doc.nodes[0].namespace, Namespace::None);
    }
    native_host(&format!("<div id=host><span>A</x-missing></rt>B</span>C"), "<div><span>AB</span>C</div>");
}

#[test]
fn generic_end_node_and_depth_limits_charge_actual_retained_and_reconstructed_nodes() {
    let limited = |source: &str, nodes| parse_internal(source, Some("div"), true, Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    for (source, nodes) in [
        ("A</x-missing>B", 2),
        ("<span>A<section>B</span>C</section>D</span>E", 7),
        ("<span><b>A<section>B</span>C</section>D</span>E", 9),
    ] {
        assert_eq!(limited(source, nodes).unwrap().nodes.len(), nodes);
        assert!(limited(source, nodes - 1).unwrap_err().contains("node limit"));
    }
    let full = "<span></span>".repeat(MAX_NODES - 1);
    assert_eq!(parse_fragment(&full, "div", true).unwrap().nodes.len(), MAX_NODES);
    assert!(parse_fragment(&format!("{full}<span>"), "div", true).unwrap_err().contains("node/depth limit"));
    let blocked = format!("<x-outer><section>{}</x-outer><i>", "<span>".repeat(MAX_DEPTH - 3));
    assert!(parse_fragment(&blocked, "div", true).unwrap_err().contains("node/depth limit"));
    let released = format!("<x-outer>{}</x-outer><em id=fresh>E", "<span>".repeat(MAX_DEPTH - 2));
    let doc = parse_fragment(&released, "div", true).unwrap();
    assert_eq!(doc.nodes.len(), MAX_DEPTH + 2);
    assert!(doc.nodes[0].children.contains(&id(&doc, "fresh")));
}

#[test]
fn generic_end_cancellation_returns_no_prefix_and_fresh_parser_recovers() {
    let source = format!("A{}B", "</x-missing></label></ruby></sarcasm>".repeat(2048));
    let calls = std::cell::Cell::new(0);
    let check = || { calls.set(calls.get() + 1); if calls.get() == 64 { Err("original generic-end cancellation".into()) } else { Ok(()) } };
    let error = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 2, check_cancel: &check }), None).unwrap_err();
    assert_eq!(error, "original generic-end cancellation"); assert_eq!(calls.get(), 64);
    let fresh = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 2, check_cancel: &|| Ok(()) }), None).unwrap();
    assert_eq!(fresh.nodes.len(), 2); assert_eq!(contents(&fresh, 0), "AB");
}

#[test]
fn generic_end_checked_select_table_svg_null_and_later_refusal_gates_stay_atomic() {
    for source in ["</option>", "</optgroup>", "<option>A</option>", "<select><option><span>A</span></option></select>", "<select></span></select>"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    for end in ["span", "label", "x-card", "option", "ruby", "noscript"] {
        assert!(parse_fragment(&format!("<table></{end}></table>"), "div", true).is_err(), "{end}");
        assert!(parse_fragment(&format!("<svg><g></{end}></g></svg>"), "template", true).is_err(), "{end}");
    }
    assert!(parse_fragment("<svg><g></g data-note='>'></svg>", "template", true).is_err());
    for suffix in ["<table><tr></tr></table>", "<svg></svg>", "<select><input></select>", "\0"] {
        assert!(parse_fragment(&format!("<span>A<section>B</span>C</section>D</span>E{suffix}"), "div", true).is_err());
        let fresh = parse_fragment("<span>A<section>B</span>C</section>D</span>E", "div", true).unwrap();
        assert_eq!(contents(&fresh, 0), "<span>A<section>BC</section>D</span>E");
    }
}

#[test]
fn generic_end_raw_contexts_document_modes_and_head_routing_remain_distinct() {
    let source = "</span></x-card></ruby>&amp;";
    for context in ["title", "textarea", "style", "script", "xmp", "iframe", "noembed", "noframes", "plaintext"] {
        let doc = parse_fragment(source, context, true).unwrap();
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(contents(&doc, 0), if matches!(context, "title" | "textarea") { "</span></x-card></ruby>&" } else { source });
    }
    let source = "<!doctype html></x-missing><html></span><head></ruby><title>x</title></head></label><body id=host><span>A<section>B</span>C</section>D</span>E</body></html>";
    let (doc, doctype) = parse_inert_html(source, 30, &|| Ok(())).unwrap();
    assert_eq!(doc.mode, DocumentMode::NoQuirks); assert_eq!(doctype, Some(1));
    assert_eq!(contents(&doc, element(&doc, "head")), "<title>x</title>");
    assert_eq!(contents(&doc, id(&doc, "host")), "<span>A<section>BC</section>D</span>E");
}
