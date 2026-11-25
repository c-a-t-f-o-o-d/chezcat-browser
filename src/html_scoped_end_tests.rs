//! Explicit list/definition/heading end tags use their own HTML scope rules.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

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
        _ => panic!("unexpected node in original scoped-end tree fixture"),
    }
}
fn contents(doc: &Document, node: usize) -> String {
    doc.nodes[node].children.iter().map(|&child| tree(doc, child)).collect()
}
fn native_documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 1000, &|| Ok(())).unwrap().0]
}
fn assert_trees(source: &str, root: &str, expected: &str) {
    for doc in native_documents(source).into_iter().chain([
        parse(source), parse_fragment(source, "div", true).unwrap(),
    ]) {
        assert_eq!(tree(&doc, element(&doc, root)), expected, "{source}");
    }
}

#[test]
fn li_end_crosses_non_scope_special_nodes_and_button() {
    for wrapper in ["section", "article", "address", "div", "p", "button", "dl", "dt", "dd", "dialog", "span"] {
        assert_trees(&format!("<ul><li>A<{wrapper}>B</li>C"), "ul",
            &format!("<ul><li>A<{wrapper}>B</{wrapper}></li>C</ul>"));
    }
}

#[test]
fn li_end_stops_at_each_nested_list_container() {
    for container in ["ul", "ol"] {
        assert_trees(&format!("<ul><li>A<{container}>B</li>C</{container}>D"), "ul",
            &format!("<ul><li>A<{container}>BC</{container}>D</li></ul>"));
    }
}

#[test]
fn li_end_stops_at_ordinary_scope_barriers() {
    for barrier in ["object", "applet", "marquee"] {
        assert_trees(&format!("<ul><li>A<{barrier}>B</li>C</{barrier}>D"), "ul",
            &format!("<ul><li>A<{barrier}>BC</{barrier}>D</li></ul>"));
    }
}

#[test]
fn li_end_matches_the_nearest_item_before_a_list_scope_barrier() {
    for container in ["ul", "ol"] {
        assert_trees(&format!("<ul><li>A<{container}><li>B</li>C</{container}>D"), "ul",
            &format!("<ul><li>A<{container}><li>B</li>C</{container}>D</li></ul>"));
    }
}

#[test]
fn definition_end_requires_its_exact_requested_tag() {
    for (open, other) in [("dd", "dt"), ("dt", "dd")] {
        assert_trees(&format!("<dl><{open}>A</{other}>B"), "dl",
            &format!("<dl><{open}>AB</{open}></dl>"));
        assert_trees(&format!("<dl><{open}>A<section><{other}>B</{open}>C"), "dl",
            &format!("<dl><{open}>A<section><{other}>B</{other}></section></{open}>C</dl>"));
    }
}

#[test]
fn definition_end_crosses_section_lists_and_button_in_ordinary_scope() {
    for target in ["dd", "dt"] { for wrapper in ["section", "ul", "ol", "button", "div", "dialog", "li"] {
        assert_trees(&format!("<dl><{target}>A<{wrapper}>B</{target}>C"), "dl",
            &format!("<dl><{target}>A<{wrapper}>B</{wrapper}></{target}>C</dl>"));
    }}
}

#[test]
fn definition_end_stops_at_ordinary_scope_barriers() {
    for target in ["dd", "dt"] { for barrier in ["object", "applet", "marquee"] {
        assert_trees(&format!("<dl><{target}>A<{barrier}>B</{target}>C</{barrier}>D"), "dl",
            &format!("<dl><{target}>A<{barrier}>BC</{barrier}>D</{target}></dl>"));
    }}
}

#[test]
fn heading_end_accepts_all_thirty_six_open_and_end_rank_pairs() {
    for open in HEADINGS { for end in HEADINGS {
        assert_trees(&format!("<div><{open}>A</{end}>B"), "div",
            &format!("<div><{open}>A</{open}>B</div>"));
    }}
}

#[test]
fn heading_end_closes_the_nearest_heading_regardless_of_requested_rank() {
    for end in HEADINGS {
        assert_trees(&format!("<div><h1>A<span><h4>B</{end}>C"), "div",
            "<div><h1>A<span><h4>B</h4>C</span></h1></div>");
    }
}

#[test]
fn heading_end_crosses_section_lists_and_button_in_ordinary_scope() {
    for wrapper in ["section", "ul", "ol", "button", "div", "dialog", "li", "dd", "dt"] {
        assert_trees(&format!("<div><h2>A<{wrapper}>B</h5>C"), "div",
            &format!("<div><h2>A<{wrapper}>B</{wrapper}></h2>C</div>"));
    }
}

#[test]
fn heading_end_stops_at_ordinary_scope_barriers() {
    for barrier in ["object", "applet", "marquee"] {
        assert_trees(&format!("<div><h2>A<{barrier}>B</h2>C</{barrier}>D"), "div",
            &format!("<div><h2>A<{barrier}>BC</{barrier}>D</h2></div>"));
        assert_trees(&format!("<div><h2>A<{barrier}><h4>B</h1>C</{barrier}>D"), "div",
            &format!("<div><h2>A<{barrier}><h4>B</h4>C</{barrier}>D</h2></div>"));
    }
}

#[test]
fn scoped_end_template_boundary_protects_outer_targets_and_closes_inner_targets() {
    for (target, inner, end) in [("li", "li", "li"), ("dd", "dd", "dd"), ("dt", "dt", "dt"), ("h1", "h4", "h2")] {
        let source = format!("<div><{target}>A<template>B</{target}>C</template>D</{target}>E");
        let expected = format!("<div><{target}>A<template>BC</template>D</{target}>E</div>");
        for doc in native_documents(&source).into_iter().chain([parse_fragment(&source, "template", true).unwrap()]) {
            assert_eq!(tree(&doc, element(&doc, "div")), expected, "{source}");
        }
        let source = format!("<div><{target}>A<template><{inner}>B</{end}>C</template>D</{target}>E");
        let expected = format!("<div><{target}>A<template><{inner}>B</{inner}>C</template>D</{target}>E</div>");
        for doc in native_documents(&source).into_iter().chain([parse_fragment(&source, "template", true).unwrap()]) {
            assert_eq!(tree(&doc, element(&doc, "div")), expected, "{source}");
        }
    }
}

#[test]
fn scoped_end_table_cells_and_select_preserve_outer_targets_in_real_documents() {
    for target in ["li", "dd", "dt", "h2"] {
        let source = format!("<div id=host><{target} id=old>A<table><tr><td id=cell>B</{target}>C</td></tr></table>D</{target}>E</div>");
        for doc in native_documents(&source) {
            assert_eq!(contents(&doc, id(&doc, "cell")), "BC");
            assert_eq!(tree(&doc, id(&doc, "old")), format!("<{target}>A<table><tbody><tr><td>BC</td></tr></tbody></table>D</{target}>"));
            assert_eq!(contents(&doc, id(&doc, "host")), format!("<{target}>A<table><tbody><tr><td>BC</td></tr></tbody></table>D</{target}>E"));
        }
        let source = format!("<div><{target}>A<select>B</{target}>C</select>D</{target}>E");
        for doc in native_documents(&source) {
            assert_eq!(tree(&doc, element(&doc, "div")), format!("<div><{target}>A<select>BC</select>D</{target}>E</div>"));
        }
    }
}

#[test]
fn scoped_end_ordinary_button_crossing_does_not_change_paragraph_button_scope() {
    assert_trees("<div><p>A<button><dd>B</dd>C</button>D</div>", "div",
        "<div><p>A<button><dd>B</dd>C</button>D</p></div>");
    for target in ["li", "dd", "dt", "h2"] {
        assert_trees(&format!("<div><{target}>A<button><p>B</{target}>C"), "div",
            &format!("<div><{target}>A<button><p>B</p></button></{target}>C</div>"));
    }
}

#[test]
fn scoped_end_retains_html_identity_attributes_comments_and_case_folding() {
    let source = "<div><H2 id=old data-note='a&amp;b' DATA-NOTE=ignored xmlns='urn:other'>A<!--m--><span>B</H6>C";
    for doc in native_documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let old = id(&doc, "old");
        assert_eq!(doc.attr(old, "data-note"), Some("a&b"));
        assert_eq!(doc.attr(old, "xmlns"), Some("urn:other"));
        assert_eq!(doc.nodes[old].namespace, Namespace::Html);
        assert_eq!(tree(&doc, element(&doc, "div")), "<div><h2>A<!--m--><span>B</span></h2>C</div>");
    }
}

#[test]
fn scoped_end_preserves_existing_exact_foreign_name_fallback() {
    let source = "<div><math><li>A</li><dd>B</dd><dt>C</dt><h2>D</h2><h3>E</h1>F</h3></math><h2>G</h1>H";
    for doc in native_documents(source).into_iter().chain([parse(source)]) {
        let math = element(&doc, "math");
        assert_eq!(tree(&doc, math), "<math><li>A</li><dd>B</dd><dt>C</dt><h2>D</h2><h3>EF</h3></math>");
        assert!(doc.nodes[math].children.iter().all(|&node| doc.nodes[node].namespace == Namespace::UnknownForeign));
        assert_eq!(tree(&doc, element(&doc, "div")), "<div><math><li>A</li><dd>B</dd><dt>C</dt><h2>D</h2><h3>EF</h3></math><h2>G</h2>H</div>");
    }
}

#[test]
fn scoped_end_missing_targets_and_fragment_contexts_never_create_or_pop_nodes() {
    let source = "A</li></dd></dt></h1>B";
    for context in ["div", "li", "dd", "dt", "h1", "h2", "h6", "template"] {
        let doc = parse_fragment(source, context, true).unwrap();
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(contents(&doc, 0), "AB");
        assert_eq!(doc.nodes[0].namespace, Namespace::None);
        assert!(matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == context));
    }
    assert_trees("<div><li>A</li>B</li>C</dd>D</dt>E</h3>F", "div", "<div><li>A</li>BCDEF</div>");
}

#[test]
fn scoped_end_dispatches_only_complete_exact_tokens_and_ignores_html_end_attributes() {
    for (open, end) in [("li", "li"), ("dd", "dd"), ("dt", "dt"), ("h2", "h6")] {
        for tail in [" data-note='>'>", " data-note=x>", "/>"] {
            assert_trees(&format!("<div><{open}>A</{end}{tail}B"), "div", &format!("<div><{open}>A</{open}>B</div>"));
        }
        for tail in [".more>", "é>", " data-note='unfinished", " data-note=unfinished", ""] {
            let suffix = if tail.ends_with('>') { "B" } else { "" };
            assert_trees(&format!("<div><{open}>A</{end}{tail}{suffix}"), "div", &format!("<div><{open}>A{suffix}</{open}></div>"));
        }
    }
}

#[test]
fn scoped_end_reconstructs_active_formatting_after_a_real_pop() {
    for (open, end) in [("li", "li"), ("dd", "dd"), ("dt", "dt"), ("h2", "h6")] {
        let source = format!("<div><{open}><b data-note=kept>A</{end}>B</b>C");
        for doc in native_documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
            assert_eq!(tree(&doc, element(&doc, "div")), format!("<div><{open}><b>A</b></{open}><b>B</b>C</div>"));
            let copies: Vec<_> = (1..doc.nodes.len()).filter(|&n| doc.attr(n, "data-note") == Some("kept")).collect();
            assert_eq!(copies.len(), 2);
            assert_ne!(copies[0], copies[1]);
            let mut parents = vec![0; doc.nodes.len()];
            for node in &doc.nodes { for &child in &node.children { parents[child] += 1; } }
            assert!(parents[1..].iter().all(|&count| count == 1));
        }
    }
}

#[test]
fn scoped_end_ignored_across_a_marker_keeps_the_current_formatting_tree() {
    let source = "<ul><li><b>A<object><i>B</li>C</object>D</li>E";
    let expected = "<ul><li><b>A<object><i>BC</i></object>D</b></li><b>E</b></ul>";
    for doc in native_documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(tree(&doc, element(&doc, "ul")), expected);
    }
}

#[test]
fn scoped_end_retains_real_node_depth_and_formatting_capacity_limits() {
    let limited = |source: &str, nodes| parse_internal(source, Some("div"), true, Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    assert_eq!(limited("A</li></dd></dt></h6>B", 2).unwrap().nodes.len(), 2);
    assert!(limited("A</li></dd></dt></h6>B", 1).unwrap_err().contains("node limit"));
    for (source, nodes) in [("<li>A</li>B", 4), ("<dd>A</dd>B", 4), ("<dt>A</dt>B", 4), ("<h2>A</h1>B", 4), ("<h2><b>A</h1>B", 6)] {
        assert_eq!(limited(source, nodes).unwrap().nodes.len(), nodes);
        assert!(limited(source, nodes - 1).unwrap_err().contains("node limit"));
    }
    let full = "<h2></h1>".repeat(MAX_NODES - 1);
    assert_eq!(parse_fragment(&full, "div", true).unwrap().nodes.len(), MAX_NODES);
    assert!(parse_fragment(&format!("{full}<h2>"), "div", true).unwrap_err().contains("node/depth limit"));
    let deep = format!("{}<h2>A</h1>", "<span>".repeat(MAX_DEPTH));
    assert!(parse_fragment(&deep, "div", true).unwrap_err().contains("node/depth limit"));
}

#[test]
fn scoped_end_cancellation_discards_the_prefix_and_allows_a_fresh_parse() {
    let source = format!("A{}B", "</li></dd></dt></h1>".repeat(2048));
    let calls = std::cell::Cell::new(0);
    let check = || { calls.set(calls.get() + 1); if calls.get() == 64 { Err("original scoped-end cancellation".into()) } else { Ok(()) } };
    let error = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 2, check_cancel: &check }), None).unwrap_err();
    assert_eq!(error, "original scoped-end cancellation");
    assert_eq!(calls.get(), 64);
    let fresh = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 2, check_cancel: &|| Ok(()) }), None).unwrap();
    assert_eq!(fresh.nodes.len(), 2); assert_eq!(contents(&fresh, 0), "AB");
}

#[test]
fn scoped_end_checked_table_select_and_strict_svg_validation_still_precede_dispatch() {
    for end in ["li", "dd", "dt", "h1", "h2", "h3", "h4", "h5", "h6"] {
        assert!(parse_fragment(&format!("<table></{end}></table>"), "div", true).is_err());
        assert!(parse_fragment(&format!("<select></{end}></select>"), "div", true).is_err());
        for source in [format!("<svg><g></{end}></g></svg>"), format!("<svg></{end}></svg>")] {
            assert!(parse_fragment(&source, "template", true).unwrap_err().contains("Template SVG requires matching end tags"), "{source}");
        }
        let doc = parse_fragment(&format!("<svg><g></g></svg></{end}>"), "template", true).unwrap();
        assert_eq!(contents(&doc, 0), "<svg><g></g></svg>");
        assert_eq!(doc.nodes.len(), 3);
    }
}

#[test]
fn scoped_end_keeps_text_contexts_later_refusals_and_document_mode() {
    let source = "</li></dd></dt></h6>&amp;";
    for context in ["title", "textarea", "style", "script", "iframe", "plaintext"] {
        let doc = parse_fragment(source, context, true).unwrap();
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(contents(&doc, 0), if matches!(context, "title" | "textarea") { "</li></dd></dt></h6>&" } else { source });
    }
    for suffix in ["<table><tr></tr></table>", "<svg></svg>", "<select><input></select>", "\0"] {
        assert!(parse_fragment(&format!("<h2>A</h1>B{suffix}"), "div", true).is_err(), "{suffix:?}");
        let fresh = parse_fragment("<h2>A</h1>B", "div", true).unwrap();
        assert_eq!(contents(&fresh, 0), "<h2>A</h2>B");
    }
    for (suffix, expected) in [("</p>", "<p></p>"), ("</br>", "<br></br>")] {
        let doc = parse_fragment(&format!("<h2>A</h1>B{suffix}"), "div", true).unwrap();
        assert_eq!(contents(&doc, 0), format!("<h2>A</h2>B{expected}"));
    }
    let (doc, doctype) = parse_inert_html("<!doctype html><div><h2>A</h1>B</div>", 20, &|| Ok(())).unwrap();
    assert_eq!(doc.mode, DocumentMode::NoQuirks);
    assert_eq!(doctype, Some(1));
    assert_eq!(tree(&doc, element(&doc, "div")), "<div><h2>A</h2>B</div>");
}
