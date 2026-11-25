//! Current in-body ordinary block ends require an exact HTML target in scope.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

// The current group includes SELECT and BUTTON; specialized marker and FORM
// ends are separate rules. LISTING retains its existing checked refusal.
const ORDINARY_ENDS: &[&str] = &[
    "address", "article", "aside", "blockquote", "button", "center", "details",
    "dialog", "dir", "div", "dl", "fieldset", "figcaption", "figure", "footer",
    "header", "hgroup", "listing", "main", "menu", "nav", "ol", "pre", "search",
    "section", "select", "summary", "ul",
];

fn id(doc: &Document, value: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}
fn element(doc: &Document, name: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| matches!(&doc.nodes[node].kind, NodeKind::Element { tag, .. } if tag == name)).unwrap()
}
fn tree(doc: &Document, node: usize) -> String {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", contents(doc, node)),
        NodeKind::Text(text) => text.as_str().to_owned(),
        NodeKind::Comment(text) => format!("<!--{text}-->"),
        _ => panic!("unexpected node in original ordinary-end tree fixture"),
    }
}
fn contents(doc: &Document, node: usize) -> String {
    doc.nodes[node].children.iter().map(|&child| tree(doc, child)).collect()
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

#[test]
fn ordinary_end_all_twenty_eight_names_respect_object_scope_in_native_documents() {
    assert_eq!(ORDINARY_ENDS.len(), 28);
    for target in ORDINARY_ENDS {
        native_host(&format!("<div id=host><{target}>A<object>B</{target}>C</object>D"),
            &format!("<div><{target}>A<object>BC</object>D</{target}></div>"));
    }
}

#[test]
fn ordinary_end_checked_admitted_group_members_keep_the_same_exact_scope() {
    for target in ORDINARY_ENDS.iter().copied().filter(|name| !matches!(*name, "listing" | "select")) {
        checked_host(&format!("<div id=host><{target}>A<object>B</{target}>C</object>D"),
            &format!("<div><{target}>A<object>BC</object>D</{target}></div>"));
    }
}

#[test]
fn ordinary_end_matches_its_exact_target_and_closes_the_nearest_duplicate() {
    for (outer, inner) in [("section", "article"), ("article", "section"), ("div", "blockquote"), ("pre", "section")] {
        checked_host(&format!("<div id=host><{outer}>A<{inner}>B</{outer}>C"),
            &format!("<div><{outer}>A<{inner}>B</{inner}></{outer}>C</div>"));
    }
    for target in ["section", "article", "div", "pre"] {
        checked_host(&format!("<div id=host><{target}>A<span><{target}>B</{target}>C"),
            &format!("<div><{target}>A<span><{target}>B</{target}>C</span></{target}></div>"));
    }
}

#[test]
fn ordinary_end_crosses_button_lists_paragraph_and_non_scope_special_nodes() {
    for wrapper in ["button", "ul", "ol", "li", "dd", "dt", "section", "p", "dialog", "span"] {
        checked_host(&format!("<div id=host><article>A<{wrapper}>B</article>C"),
            &format!("<div><article>A<{wrapper}>B</{wrapper}></article>C</div>"));
    }
    checked_host("<div id=host><button>A<section><p>B</button>C",
        "<div><button>A<section><p>B</p></section></button>C</div>");
}

#[test]
fn ordinary_end_stops_at_applet_marquee_object_and_select_boundaries() {
    for barrier in ["applet", "marquee", "object", "select"] {
        for target in ["div", "section", "button", "listing"] {
            native_host(&format!("<div id=host><{target}>A<{barrier}>B</{target}>C</{barrier}>D"),
                &format!("<div><{target}>A<{barrier}>BC</{barrier}>D</{target}></div>"));
        }
    }
}

#[test]
fn ordinary_end_template_scope_protects_outer_target_and_admits_inner_target() {
    for target in ["section", "button", "div", "pre"] {
        for (source, expected) in [
            (format!("<div id=host><{target}>A<template>B</{target}>C</template>D</{target}>E"),
             format!("<div><{target}>A<template>BC</template>D</{target}>E</div>")),
            (format!("<div id=host><{target}>A<template><{target}>B</{target}>C</template>D</{target}>E"),
             format!("<div><{target}>A<template><{target}>B</{target}>C</template>D</{target}>E</div>")),
        ] {
            for doc in documents(&source).into_iter().chain([parse_fragment(&source, "template", true).unwrap()]) {
                assert_eq!(tree(&doc, id(&doc, "host")), expected, "{source}");
            }
        }
    }
}

#[test]
fn ordinary_end_table_cell_and_caption_boundaries_keep_outer_ownership() {
    for source in [
        "<div id=host><section>A<table><tr><td>B</section>C</td></tr></table>D</section>E",
        "<div id=host><section>A<table><tr><th>B</section>C</th></tr></table>D</section>E",
        "<div id=host><section>A<table><caption>B</section>C</caption></table>D</section>E",
    ] {
        for doc in documents(source) {
            let section = element(&doc, "section");
            assert_eq!(doc.nodes[id(&doc, "host")].children.len(), 2);
            assert_eq!(doc.nodes[id(&doc, "host")].children[0], section);
            assert!(matches!(&doc.nodes[*doc.nodes[id(&doc, "host")].children.last().unwrap()].kind, NodeKind::Text(text) if text == "E"));
            let leaf = (1..doc.nodes.len()).find(|&n| matches!(&doc.nodes[n].kind, NodeKind::Element { tag, .. } if matches!(tag.as_str(), "td" | "th" | "caption"))).unwrap();
            assert_eq!(contents(&doc, leaf), "BC");
            assert!(matches!(&doc.nodes[*doc.nodes[section].children.last().unwrap()].kind, NodeKind::Text(text) if text == "D"));
        }
    }
    for (source, expected) in [
        ("<div id=host><table><tr><td><section>A</section>B</td></tr></table>", "<div><table><tbody><tr><td><section>A</section>B</td></tr></tbody></table></div>"),
        ("<div id=host><table><caption><section>A</section>B</caption></table>", "<div><table><caption><section>A</section>B</caption></table></div>"),
    ] { for doc in documents(source) { assert_eq!(tree(&doc, id(&doc, "host")), expected); } }
}

#[test]
fn ordinary_end_select_matches_before_its_own_scope_boundary() {
    checked_host("<div id=host><select><optgroup><option>A</select>B",
        "<div><select><optgroup><option>A</option></optgroup></select>B</div>");
    native_host("<div id=host><select>A<object>B</select>C</object>D</select>E",
        "<div><select>A<object>BC</object>D</select>E</div>");
}

#[test]
fn ordinary_end_specialized_marker_and_form_paths_remain_separate() {
    for marker in ["applet", "object", "marquee"] {
        let source = format!("<div id=host><b>A<{marker}><i>B</{marker}>C</b>D");
        let expected = format!("<div><b>A<{marker}><i>B</i></{marker}>C</b>D</div>");
        for doc in documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
            assert_eq!(tree(&doc, id(&doc, "host")), expected);
        }
    }
    checked_host("<div id=host><form><p>A</p></form>B", "<div><form><p>A</p></form>B</div>");
    let doc = parse_fragment("<span></span>", "div", true).unwrap();
    for name in ["form", "applet", "object", "marquee", "template", "table", "td", "caption", "html", "body", "p", "br", "span", "section-extra", "selecté"] {
        let mut stack = vec![0, 1];
        assert!(!close_scoped_end(&mut stack, &doc, name), "{name}");
        assert_eq!(stack, [0, 1]);
    }
}

#[test]
fn ordinary_end_preserves_active_formatting_after_a_pop_and_across_a_blocked_end() {
    for target in ["section", "button", "div", "ul"] {
        let source = format!("<div id=host><{target}><b data-note=kept>A</{target}>B</b>C");
        let expected = format!("<div><{target}><b>A</b></{target}><b>B</b>C</div>");
        for doc in documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
            assert_eq!(tree(&doc, id(&doc, "host")), expected);
            let copies: Vec<_> = (1..doc.nodes.len()).filter(|&n| doc.attr(n, "data-note") == Some("kept")).collect();
            assert_eq!(copies.len(), 2); assert_ne!(copies[0], copies[1]);
        }
    }
    let source = "<div id=host><section><b>A<object><i>B</section>C</object>D</section>E";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><section><b>A<object><i>BC</i></object>D</b></section><b>E</b></div>");
        let mut parents = vec![0; doc.nodes.len()];
        for node in &doc.nodes { for &child in &node.children { parents[child] += 1; } }
        assert!(parents[1..].iter().all(|&count| count == 1));
    }
}

#[test]
fn ordinary_end_native_identity_is_independent_of_case_xmlns_and_attributes() {
    let source = "<div id=host><SECTION id=target data-note='a&amp;b' DATA-NOTE=ignored xmlns='urn:other'>A<object>B</SeCtIoN>C</object>D";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let target = id(&doc, "target");
        assert_eq!(doc.attr(target, "data-note"), Some("a&b"));
        assert_eq!(doc.attr(target, "xmlns"), Some("urn:other"));
        assert_eq!(doc.nodes[target].namespace, Namespace::Html);
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><section>A<object>BC</object>D</section></div>");
    }
}

#[test]
fn ordinary_end_helper_keeps_conservative_foreign_ancestor_and_context_boundaries() {
    for namespace in [Namespace::Html, Namespace::Svg, Namespace::UnknownForeign, Namespace::Other, Namespace::None] {
        let mut doc = parse_fragment("<section id=outer><span id=middle><span id=current>", "div", true).unwrap();
        let outer = id(&doc, "outer"); let middle = id(&doc, "middle"); let current = id(&doc, "current");
        doc.nodes[middle].namespace = namespace;
        let original = vec![0, outer, middle, current]; let mut stack = original.clone();
        assert!(close_scoped_end(&mut stack, &doc, "section"));
        assert_eq!(stack, if namespace == Namespace::Html { vec![0] } else { original });
    }
    for context in ["div", "section", "article", "main", "ul", "ol", "dl", "pre", "blockquote"] {
        let doc = parse_fragment("", context, true).unwrap();
        let mut stack = vec![0]; assert!(close_scoped_end(&mut stack, &doc, context)); assert_eq!(stack, [0]);
    }
}

#[test]
fn ordinary_end_preserves_foreign_current_exact_name_fallback() {
    let source = "<div id=host><math><section>A</section><button>B</button><select>C</select><listing>D</listing></math><section>E<object>F</section>G</object>H";
    for doc in documents(source).into_iter().chain([parse(source)]) {
        let math = element(&doc, "math");
        assert_eq!(contents(&doc, math), "<section>A</section><button>B</button><select>C</select><listing>D</listing>");
        assert!(doc.nodes[math].children.iter().all(|&n| doc.nodes[n].namespace == Namespace::UnknownForeign));
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><math><section>A</section><button>B</button><select>C</select><listing>D</listing></math><section>E<object>FG</object>H</section></div>");
    }
}

#[test]
fn ordinary_end_missing_targets_do_not_allocate_or_split_text() {
    let ends: String = ORDINARY_ENDS.iter().map(|name| format!("</{name}>")).collect();
    native_host(&format!("<span id=host>A{ends}B"), "<span>AB</span>");
    let checked_ends: String = ORDINARY_ENDS.iter().copied().filter(|name| !matches!(*name, "listing" | "select")).map(|name| format!("</{name}>")).collect();
    for context in ["div", "section", "article", "main", "ul", "ol", "dl", "pre", "blockquote", "template"] {
        let doc = parse_fragment(&format!("A{checked_ends}B"), context, true).unwrap();
        assert_eq!(doc.nodes.len(), 2); assert_eq!(contents(&doc, 0), "AB");
        assert_eq!(doc.nodes[0].namespace, Namespace::None);
    }
}

#[test]
fn ordinary_end_uses_complete_exact_tokens_and_discards_end_attributes() {
    for tail in [" data-note='>'>", " data-note=x>", "/>"] {
        checked_host(&format!("<div id=host><section>A<object>B</section{tail}C</object>D"),
            "<div><section>A<object>BC</object>D</section></div>");
        checked_host(&format!("<div id=host><section>A</section{tail}B"), "<div><section>A</section>B</div>");
    }
    for tail in [".more>", "é>"] {
        checked_host(&format!("<div id=host><section>A</section{tail}B"), "<div><section>AB</section></div>");
    }
    for tail in ["", " data-note=unfinished", " data-note='unfinished"] {
        checked_host(&format!("<div id=host><section>A</section{tail}"), "<div><section>A</section></div>");
    }
}

#[test]
fn ordinary_end_keeps_exact_node_depth_and_formatting_admission() {
    let limited = |source: &str, nodes| parse_internal(source, Some("div"), true, Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    for (source, nodes) in [
        ("A</section>B", 2),
        ("<section>A<object>B</section>C</object>D</section>E", 7),
        ("<section><b>A<object><i>B</section>C</object>D</section>E", 10),
    ] {
        assert_eq!(limited(source, nodes).unwrap().nodes.len(), nodes);
        assert!(limited(source, nodes - 1).unwrap_err().contains("node limit"));
    }
    let full = "<section></section>".repeat(MAX_NODES - 1);
    assert_eq!(parse_fragment(&full, "div", true).unwrap().nodes.len(), MAX_NODES);
    assert!(parse_fragment(&format!("{full}<section>"), "div", true).unwrap_err().contains("node/depth limit"));
    let deep = "<section><object>".repeat(MAX_DEPTH);
    assert!(parse_fragment(&deep, "div", true).unwrap_err().contains("node/depth limit"));
}

#[test]
fn ordinary_end_cancellation_discards_prefix_and_fresh_parser_recovers() {
    let source = format!("A{}B", "</section></button></ul></dialog>".repeat(2048));
    let calls = std::cell::Cell::new(0);
    let check = || { calls.set(calls.get() + 1); if calls.get() == 64 { Err("original ordinary-end cancellation".into()) } else { Ok(()) } };
    let error = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 2, check_cancel: &check }), None).unwrap_err();
    assert_eq!(error, "original ordinary-end cancellation"); assert_eq!(calls.get(), 64);
    let fresh = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 2, check_cancel: &|| Ok(()) }), None).unwrap();
    assert_eq!(fresh.nodes.len(), 2); assert_eq!(contents(&fresh, 0), "AB");
}

#[test]
fn ordinary_end_preserves_checked_listing_select_table_and_strict_svg_gates() {
    for source in ["</listing>", "<listing>A</listing>", "</select>", "<select><object>A</select>", "<select></section></select>"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    assert!(parse_fragment("</select>", "select", true).is_err());
    for end in ORDINARY_ENDS {
        assert!(parse_fragment(&format!("<table></{end}></table>"), "div", true).is_err(), "{end}");
        assert!(parse_fragment(&format!("<svg><g></{end}></g></svg>"), "template", true).is_err(), "{end}");
    }
    for suffix in ["<table><tr></tr></table>", "<svg></svg>", "<select><input></select>", "\0"] {
        assert!(parse_fragment(&format!("<section>A<object>B</section>C</object>D{suffix}"), "div", true).is_err());
        let fresh = parse_fragment("<section>A<object>B</section>C</object>D", "div", true).unwrap();
        assert_eq!(contents(&fresh, 0), "<section>A<object>BC</object>D</section>");
    }
}

#[test]
fn ordinary_end_keeps_text_contexts_document_modes_and_head_routing() {
    let source = "</section></button></select>&amp;";
    for context in ["title", "textarea", "style", "script", "iframe", "plaintext"] {
        let doc = parse_fragment(source, context, true).unwrap(); assert_eq!(doc.nodes.len(), 2);
        assert_eq!(contents(&doc, 0), if matches!(context, "title" | "textarea") { "</section></button></select>&" } else { source });
    }
    let source = "<!doctype html></section><html></button><head></select><title>x</title></head></div><body><section>A<object>B</section>C</object>D</body></html>";
    let (doc, doctype) = parse_inert_html(source, 30, &|| Ok(())).unwrap();
    assert_eq!(doc.mode, DocumentMode::NoQuirks); assert_eq!(doctype, Some(1));
    assert_eq!(contents(&doc, element(&doc, "head")), "<title>x</title>");
    assert_eq!(contents(&doc, element(&doc, "body")), "<section>A<object>BC</object>D</section>");
}
