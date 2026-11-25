//! Original definition/list-item start-tag scan and tree ownership cases.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

fn name(document: &Document, node: usize) -> &str {
    match &document.nodes[node].kind { NodeKind::Element { tag, .. } => tag, _ => "" }
}
fn element(document: &Document, tag: &str) -> usize {
    document.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag: found, .. } if found == tag)).unwrap()
}
fn id(document: &Document, value: &str) -> usize {
    (0..document.nodes.len()).find(|&node| document.attr(node, "id") == Some(value)).unwrap()
}
fn parent(document: &Document, child: usize) -> usize {
    document.nodes.iter().position(|node| node.children.contains(&child)).unwrap()
}
fn tree(document: &Document, node: usize) -> String {
    match &document.nodes[node].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", document.nodes[node].children.iter().map(|&child| tree(document, child)).collect::<String>()),
        NodeKind::Text(value) => value.as_str().to_owned(),
        NodeKind::Comment(value) => format!("<!--{value}-->"),
        _ => panic!("unexpected node in original tree fixture"),
    }
}
fn documents(source: &str) -> Vec<Document> {
    vec![parse(source), parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 200, &|| Ok(())).unwrap().0,
        parse_fragment(source, "div", true).unwrap()]
}
fn assert_trees(source: &str, root: &str, expected: &str) {
    for document in documents(source) {
        assert_eq!(tree(&document, element(&document, root)), expected, "{source}");
    }
}

#[test]
fn definition_start_closes_nearest_dt_or_dd_before_inserting_its_sibling() {
    for first in ["dt", "dd"] { for second in ["dt", "dd"] {
        assert_trees(&format!("<dl><{first}>A<{second}>B"), "dl",
            &format!("<dl><{first}>A</{first}><{second}>B</{second}></dl>"));
    }}
    assert_trees("<dl><dt>A<dd>B<dd>C<dt>D", "dl", "<dl><dt>A</dt><dd>B</dd><dd>C</dd><dt>D</dt></dl>");
}

#[test]
fn definition_start_crosses_address_div_p_and_actual_non_special_elements() {
    for wrapper in ["address", "div", "p", "dialog", "span", "custom-one", "dt-extra"] {
        assert_trees(&format!("<dl><dt>A<{wrapper}>B<dd>C"), "dl",
            &format!("<dl><dt>A<{wrapper}>B</{wrapper}></dt><dd>C</dd></dl>"));
    }
}

#[test]
fn definition_start_stops_at_special_nodes_except_its_own_matching_target() {
    for barrier in ["section", "article", "aside", "blockquote", "dl", "ul", "ol", "li",
        "button", "object", "applet", "marquee", "fieldset", "figure", "details", "menu", "nav", "pre", "search", "summary", "h1"] {
        assert_trees(&format!("<dl><dt>A<{barrier}>B<dd>C"), "dl",
            &format!("<dl><dt>A<{barrier}>B<dd>C</dd></{barrier}></dt></dl>"));
    }
}

#[test]
fn definition_start_closes_p_after_the_scan_with_real_button_scope() {
    for (source, expected) in [
        ("<dl><dt>A<section><p>B<dd>C", "<dl><dt>A<section><p>B</p><dd>C</dd></section></dt></dl>"),
        ("<dl><dt>A<button><p>B<dd>C", "<dl><dt>A<button><p>B</p><dd>C</dd></button></dt></dl>"),
        ("<dl><dt>A<p>B<button><dd>C", "<dl><dt>A<p>B<button><dd>C</dd></button></p></dt></dl>"),
        ("<dl><p>A<dd>B", "<dl><p>A</p><dd>B</dd></dl>"),
    ] { assert_trees(source, "dl", expected); }
}

#[test]
fn definition_start_template_scope_preserves_outer_and_inner_definitions() {
    let source = "<dl><dt>A<template><dd>B<dt>C</dt></template><dd>D";
    let expected = "<dl><dt>A<template><dd>B</dd><dt>C</dt></template></dt><dd>D</dd></dl>";
    for document in [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100, &|| Ok(())).unwrap().0,
        parse_fragment(source, "template", true).unwrap()] {
        assert_eq!(tree(&document, element(&document, "dl")), expected);
    }
}

#[test]
fn definition_start_reconstructs_active_formatting_only_for_following_content() {
    let source = "<dl><dt><b>A<dd>B</b><dt>C";
    let expected = "<dl><dt><b>A</b></dt><dd><b>B</b></dd><dt>C</dt></dl>";
    for document in [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100, &|| Ok(())).unwrap().0,
        parse_fragment(source, "div", true).unwrap()] {
        assert_eq!(tree(&document, element(&document, "dl")), expected);
    }
}

#[test]
fn definition_start_never_pops_the_fragment_context_or_matches_name_prefixes() {
    for context in ["div", "dl", "dt", "dd"] {
        let document = parse_fragment("<dt>A<dd>B", context, true).unwrap();
        assert_eq!(document.nodes[0].children.len(), 2);
        assert_eq!(name(&document, 0), context);
        assert_eq!(tree(&document, document.nodes[0].children[0]), "<dt>A</dt>");
        assert_eq!(tree(&document, document.nodes[0].children[1]), "<dd>B</dd>");
    }
    assert_trees("<dl><dt>A<dd-extra>B<dt-extra>C<dd>D", "dl",
        "<dl><dt>A<dd-extra>B<dt-extra>C</dt-extra></dd-extra></dt><dd>D</dd></dl>");
}

#[test]
fn definition_start_retains_attributes_comments_namespace_and_case_folding() {
    for document in documents("<dl><DT id=term data-x='a&amp;b' DATA-X=ignored>A<!--m--><div id=middle xmlns='urn:other'>B<dD id=definition>C") {
        let term = id(&document, "term");
        let definition = id(&document, "definition");
        let dl = element(&document, "dl");
        assert_eq!(document.nodes[dl].children, [term, definition]);
        assert_eq!(document.attr(term, "data-x"), Some("a&b"));
        assert_eq!(tree(&document, term), "<dt>A<!--m--><div>B</div></dt>");
        assert_eq!(document.nodes[term].namespace, Namespace::Html);
        assert_eq!(document.nodes[definition].namespace, Namespace::Html);
        assert_eq!(document.nodes[id(&document, "middle")].namespace, Namespace::Html);
    }
}

#[test]
fn definition_start_keeps_conservative_foreign_boundaries_and_resumes_in_html() {
    let source = "<dl><dt id=outer>A<math><dt id=foreign>B<dd id=inside>C</dd></dt></math><dd id=after>D";
    for document in [parse(source), parse_loaded_document(source, &|| Ok(())).unwrap(), parse_inert_html(source, 100, &|| Ok(())).unwrap().0] {
        let outer = id(&document, "outer");
        let foreign = id(&document, "foreign");
        let inside = id(&document, "inside");
        let after = id(&document, "after");
        assert_eq!(parent(&document, inside), foreign);
        assert_eq!(parent(&document, element(&document, "math")), outer);
        assert_eq!(parent(&document, after), element(&document, "dl"));
        assert_eq!(document.nodes[foreign].namespace, Namespace::UnknownForeign);
        assert_eq!(document.nodes[inside].namespace, Namespace::UnknownForeign);
        assert_eq!(document.nodes[after].namespace, Namespace::Html);
    }
    assert!(parse_fragment("<dl><dt>A<svg><g><dd>B</dd></g></svg></dl>", "template", true).is_err());
}

#[test]
fn definition_start_keeps_real_node_depth_and_formatting_capacity_admission() {
    let limited = |source: &str, nodes| parse_internal(source, Some("div"), true, Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    assert_eq!(limited("<dl><dt>A<dd>B", 6).unwrap().nodes.len(), 6);
    assert!(limited("<dl><dt>A<dd>B", 5).unwrap_err().contains("node limit"));
    assert_eq!(limited("<dl><dt><b>A<dd>B", 8).unwrap().nodes.len(), 8);
    assert!(limited("<dl><dt><b>A<dd>B", 7).unwrap_err().contains("node limit"));
    let full = format!("<dl>{}", "<dt>".repeat(MAX_NODES - 2));
    assert_eq!(parse_fragment(&full, "div", true).unwrap().nodes.len(), MAX_NODES);
    assert!(parse_fragment(&format!("{full}<dd>"), "div", true).unwrap_err().contains("node/depth limit"));
    let deep = format!("<dl>{}<dd>", "<dt><section>".repeat(MAX_DEPTH));
    assert!(parse_fragment(&deep, "div", true).unwrap_err().contains("node/depth limit"));
}

#[test]
fn definition_start_cancellation_returns_no_prefix_and_fresh_parse_recovers() {
    let source = format!("<dl>{}", "<dt><dd>".repeat(2048));
    let calls = std::cell::Cell::new(0);
    let check = || { calls.set(calls.get() + 1); if calls.get() == 64 { Err("original definition start cancellation".into()) } else { Ok(()) } };
    let error = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 5000, check_cancel: &check }), None).unwrap_err();
    assert_eq!(error, "original definition start cancellation");
    assert_eq!(calls.get(), 64);
    let fresh = parse_internal(&source, Some("div"), true, Some(LoadedTree { nodes: 5000, check_cancel: &|| Ok(()) }), None).unwrap();
    assert_eq!(fresh.nodes.len(), 4098);
    assert_eq!(fresh.nodes[element(&fresh, "dl")].children.len(), 4096);
}

#[test]
fn definition_start_keeps_later_end_tag_and_unsupported_tree_refusals() {
    for suffix in ["<table><tr><td>x</td></tr></table>", "<svg></svg>", "<select><input></select>", "</p>", "a</br>b"] {
        assert!(parse_fragment(&format!("<dl><dt>A<dd>B</dl>{suffix}"), "div", true).is_err(), "{suffix}");
        let fresh = parse_fragment("<dl><dt>A<dd>B", "div", true).unwrap();
        assert_eq!(fresh.nodes[element(&fresh, "dl")].children.len(), 2);
    }
}

#[test]
fn definition_start_text_contexts_keep_list_markup_literal() {
    for context in ["title", "textarea", "style", "script", "iframe", "plaintext"] {
        let document = parse_fragment("<dl><dt>A<dd>B", context, true).unwrap();
        assert_eq!(document.nodes.len(), 2);
        assert!(matches!(&document.nodes[1].kind, NodeKind::Text(value) if value == "<dl><dt>A<dd>B"));
    }
}

#[test]
fn list_item_start_uses_special_barriers_instead_of_only_list_container_names() {
    for barrier in ["section", "blockquote", "dl", "dt", "dd", "object", "button", "menu", "ul", "ol"] {
        assert_trees(&format!("<ul><li>A<{barrier}>B<li>C"), "ul",
            &format!("<ul><li>A<{barrier}>B<li>C</li></{barrier}></li></ul>"));
    }
}

#[test]
fn list_item_start_crosses_the_shared_exceptions_and_non_special_dialog() {
    for wrapper in ["address", "div", "p", "dialog", "span", "li-extra"] {
        assert_trees(&format!("<ul><li>A<{wrapper}>B<li>C"), "ul",
            &format!("<ul><li>A<{wrapper}>B</{wrapper}></li><li>C</li></ul>"));
    }
}

#[test]
fn list_item_start_closes_p_after_scan_and_keeps_definition_targets_separate() {
    assert_trees("<ul><li>A<section><p>B<li>C", "ul", "<ul><li>A<section><p>B</p><li>C</li></section></li></ul>");
    assert_trees("<ul><li>A<p>B<button><li>C", "ul", "<ul><li>A<p>B<button><li>C</li></button></p></li></ul>");
    assert_trees("<dl><dt>A<li>B<dd>C", "dl", "<dl><dt>A<li>B<dd>C</dd></li></dt></dl>");
    assert_trees("<ul><li>A<dt>B<li>C", "ul", "<ul><li>A<dt>B<li>C</li></dt></li></ul>");
}

#[test]
fn list_item_start_keeps_formatting_and_template_scope_boundaries() {
    let source = "<ul><li><i>A<li>B</i><li>C";
    let expected = "<ul><li><i>A</i></li><li><i>B</i></li><li>C</li></ul>";
    for document in [parse_loaded_document(source, &|| Ok(())).unwrap(), parse_inert_html(source, 100, &|| Ok(())).unwrap().0, parse_fragment(source, "div", true).unwrap()] {
        assert_eq!(tree(&document, element(&document, "ul")), expected);
    }
    let document = parse_fragment("<ul><li>A<template><li>B<li>C</li></template><li>D", "template", true).unwrap();
    assert_eq!(tree(&document, element(&document, "ul")), "<ul><li>A<template><li>B</li><li>C</li></template></li><li>D</li></ul>");
}

#[test]
fn list_item_start_keeps_foreign_identity_fragment_root_and_node_limits() {
    let source = "<ul><li id=outer>A<math><li id=foreign>B<li id=inside>C</li></li></math><li id=after>D";
    for document in [parse(source), parse_loaded_document(source, &|| Ok(())).unwrap(), parse_inert_html(source, 100, &|| Ok(())).unwrap().0] {
        let foreign = id(&document, "foreign");
        let inside = id(&document, "inside");
        assert_eq!(parent(&document, inside), foreign);
        assert_eq!(parent(&document, element(&document, "math")), id(&document, "outer"));
        assert_eq!(parent(&document, id(&document, "after")), element(&document, "ul"));
        assert_eq!(document.nodes[foreign].namespace, Namespace::UnknownForeign);
        assert_eq!(document.nodes[inside].namespace, Namespace::UnknownForeign);
    }
    let context = parse_fragment("<li>A<li>B", "li", true).unwrap();
    assert_eq!(context.nodes[0].children.len(), 2);
    let limited = |nodes| parse_internal("<ul><li>A<li>B", Some("div"), true, Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    assert_eq!(limited(6).unwrap().nodes.len(), 6);
    assert!(limited(5).unwrap_err().contains("node limit"));
}
