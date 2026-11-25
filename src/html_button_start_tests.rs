//! Native HTML button starts close an earlier button in ordinary scope.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

fn id(doc: &Document, value: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}
fn element(doc: &Document, name: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| matches!(&doc.nodes[node].kind, NodeKind::Element { tag, .. } if tag == name)).unwrap()
}
fn parent(doc: &Document, child: usize) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.nodes[node].children.contains(&child)).unwrap()
}
fn tree(doc: &Document, node: usize) -> String {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", contents(doc, node)),
        NodeKind::Text(text) => text.as_str().to_owned(),
        NodeKind::Comment(text) => format!("<!--{text}-->"),
        _ => panic!("unexpected node in button tree"),
    }
}
fn contents(doc: &Document, node: usize) -> String {
    doc.nodes[node].children.iter().map(|&child| tree(doc, child)).collect()
}
fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 2000, &|| Ok(())).unwrap().0]
}
fn assert_host(source: &str, expected: &str) {
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(tree(&doc, id(&doc, "host")), expected, "{source}");
    }
}

#[test]
fn button_start_closes_an_existing_button_before_inserting_a_real_sibling() {
    let source = "<div id=host><button id=old data-x='a&amp;b'>A<button id=new data-x=c>B";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let host = id(&doc, "host"); let old = id(&doc, "old"); let new = id(&doc, "new");
        assert_eq!(doc.nodes[host].children, [old, new]);
        assert_eq!(contents(&doc, old), "A"); assert_eq!(contents(&doc, new), "B");
        assert_eq!(doc.attr(old, "data-x"), Some("a&b")); assert_eq!(doc.attr(new, "data-x"), Some("c"));
        assert_eq!(doc.nodes[old].namespace, Namespace::Html); assert_eq!(doc.nodes[new].namespace, Namespace::Html);
    }
}

#[test]
fn button_start_crosses_non_scope_special_nodes_and_retains_old_descendants() {
    for wrapper in ["span", "section", "article", "p", "ul", "ol", "li", "dd", "dt", "dialog", "pre"] {
        assert_host(&format!("<div id=host><button>A<{wrapper}>B<button>C"),
            &format!("<div><button>A<{wrapper}>B</{wrapper}></button><button>C</button></div>"));
    }
}

#[test]
fn button_start_distinguishes_an_outer_paragraph_from_one_inside_the_old_button() {
    assert_host("<div id=host><p>A<button>B<button>C", "<div><p>A<button>B</button><button>C</button></p></div>");
    assert_host("<div id=host><button>A<p>B<button>C", "<div><button>A<p>B</p></button><button>C</button></div>");
    assert_host("<div id=host><p>A<button><p>B<button>C", "<div><p>A<button><p>B</p></button><button>C</button></p></div>");
}

#[test]
fn button_start_stops_at_ordinary_scope_boundaries() {
    for barrier in ["object", "applet", "marquee", "select"] {
        let source = format!("<div id=host><button>A<{barrier}>B<button>C");
        let expected = format!("<div><button>A<{barrier}>B<button>C</button></{barrier}></button></div>");
        for doc in documents(&source) { assert_eq!(tree(&doc, id(&doc, "host")), expected); }
        if barrier != "select" {
            let doc = parse_fragment(&source, "div", true).unwrap(); assert_eq!(tree(&doc, id(&doc, "host")), expected);
        }
    }
}

#[test]
fn button_start_closes_a_nearer_button_before_an_older_scope_boundary() {
    for barrier in ["object", "applet", "marquee"] {
        assert_host(&format!("<div id=host><button>A<{barrier}><button>B<button>C"),
            &format!("<div><button>A<{barrier}><button>B</button><button>C</button></{barrier}></button></div>"));
    }
}

#[test]
fn button_start_uses_real_template_separation_and_restores_the_outer_scope() {
    let source = "<div id=host><button id=outer>A<template><button id=old>B<button id=new>C</button></template>D<button id=after>E";
    let expected = "<div><button>A<template><button>B</button><button>C</button></template>D</button><button>E</button></div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "template", true).unwrap()]) {
        assert_eq!(tree(&doc, id(&doc, "host")), expected);
        assert_eq!(parent(&doc, id(&doc, "old")), element(&doc, "template"));
        assert_eq!(parent(&doc, id(&doc, "new")), element(&doc, "template"));
        assert_eq!(parent(&doc, id(&doc, "after")), id(&doc, "host"));
    }
}

#[test]
fn button_start_table_and_cell_boundaries_protect_the_outer_button() {
    let source = "<div id=host><button id=outer>A<table><tr><td id=cell><button id=old>B<button id=new>C</button></td></tr></table>D";
    for doc in documents(source) {
        assert_eq!(doc.nodes[id(&doc, "cell")].children, [id(&doc, "old"), id(&doc, "new")]);
        assert_eq!(parent(&doc, element(&doc, "table")), id(&doc, "outer"));
        assert_eq!(contents(&doc, id(&doc, "old")), "B"); assert_eq!(contents(&doc, id(&doc, "new")), "C");
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><button>A<table><tbody><tr><td><button>B</button><button>C</button></td></tr></tbody></table>D</button></div>");
    }
}

#[test]
fn button_start_refreshes_foster_placement_after_closing_the_old_button() {
    let source = "<body><table><button id=old>A<button id=new>B</button>C</table>D";
    for doc in documents(source) {
        let body = element(&doc, "body"); let old = id(&doc, "old"); let new = id(&doc, "new");
        assert_eq!(contents(&doc, body), "<button>A</button><button>B</button>C<table></table>D");
        assert_eq!(parent(&doc, old), body); assert_eq!(parent(&doc, new), body);
        assert!(doc.nodes[element(&doc, "table")].children.is_empty());
    }
    let source = "<body><table><button id=old><b data-x=kept>A<button id=new>B</button>C</table>D";
    for doc in documents(source) {
        let body = element(&doc, "body"); let old = id(&doc, "old"); let new = id(&doc, "new");
        assert_eq!(contents(&doc, body), "<button><b>A</b></button><b><button>B</button>C</b><table></table><b>D</b>");
        let original = doc.nodes[old].children[0];
        let copied = doc.nodes[body].children[1]; let after_table = doc.nodes[body].children[3];
        assert_ne!(original, copied); assert_ne!(copied, after_table); assert_ne!(original, after_table);
        for node in [original, copied, after_table] { assert_eq!(doc.attr(node, "data-x"), Some("kept")); }
        assert_eq!(parent(&doc, new), copied);
        assert!(doc.nodes[element(&doc, "table")].children.is_empty());
        let mut parents = vec![0; doc.nodes.len()];
        for node in &doc.nodes { for &child in &node.children { parents[child] += 1; } }
        assert!(parents[1..].iter().all(|&count| count == 1));
    }
}

#[test]
fn button_start_reconstructs_active_formatting_before_inserting_the_new_button() {
    let source = "<button id=old><b data-x=kept>A<button id=new>B</button>C";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let root = if doc.nodes[0].namespace == Namespace::None && matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == "div") { 0 } else { element(&doc, "body") };
        assert_eq!(contents(&doc, root), "<button><b>A</b></button><b><button>B</button>C</b>");
        let first = doc.nodes[id(&doc, "old")].children[0]; let copy = doc.nodes[root].children[1];
        assert_ne!(first, copy); assert_eq!(doc.attr(first, "data-x"), Some("kept")); assert_eq!(doc.attr(copy, "data-x"), Some("kept"));
        assert_eq!(parent(&doc, id(&doc, "new")), copy); assert_eq!(contents(&doc, first), "A");
        let mut parents = vec![0; doc.nodes.len()];
        for node in &doc.nodes { for &child in &node.children { parents[child] += 1; } }
        assert!(parents[1..].iter().all(|&count| count == 1));
    }
}

#[test]
fn button_start_uses_complete_folded_tokens_duplicate_attributes_and_nonvoid_slashes() {
    let source = "<div id=host><BUTTON id=old>A<BuTtOn id=new data-x='first' DATA-X='second'/>B</button>C";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><button>A</button><button>B</button>C</div>");
        assert_eq!(doc.attr(id(&doc, "new"), "data-x"), Some("first"));
    }
    for tail in ["", " id=new", " id='unfinished"] {
        assert_host(&format!("<div id=host><button>A<button{tail}"), "<div><button>A</button></div>");
    }
    assert_host("<div id=host><button>A<button-extra>B<button>C", "<div><button>A<button-extra>B</button-extra></button><button>C</button></div>");
}

#[test]
fn button_start_preserves_native_foreign_current_fallback_without_new_integration_claims() {
    let source = "<div id=host><math><button id=foreign>A<button id=nested>B</button>C</button></math><button>D<button>E";
    for doc in documents(source) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><math><button>A<button>B</button>C</button></math><button>D</button><button>E</button></div>");
        assert_eq!(doc.nodes[id(&doc, "foreign")].namespace, Namespace::UnknownForeign);
        assert_eq!(doc.nodes[id(&doc, "nested")].namespace, Namespace::UnknownForeign);
        assert_eq!(parent(&doc, id(&doc, "nested")), id(&doc, "foreign"));
    }
}

#[test]
fn button_start_keeps_supported_fragment_roots_and_the_existing_button_context_refusal() {
    for context in ["div", "p", "h1", "template"] {
        let doc = parse_fragment("<button>A<button>B", context, true).unwrap();
        assert_eq!(doc.nodes[0].namespace, Namespace::None); assert_eq!(doc.nodes[0].children.len(), 2);
        assert_eq!(contents(&doc, 0), "<button>A</button><button>B</button>");
    }
    // The public checked parser currently does not admit BUTTON as a context.
    // This increment must not bypass that boundary to fabricate a positive.
    assert!(parse_fragment("<button>A<button>B", "button", true).unwrap_err().contains("element context"));
}

#[test]
fn button_start_counts_reconstructed_nodes_against_actual_inert_admission() {
    let simple = "<button>A<button>B";
    assert_eq!(parse_inert_html(simple, 8, &|| Ok(())).unwrap().0.nodes.len(), 8);
    assert!(parse_inert_html(simple, 7, &|| Ok(())).unwrap_err().contains("node limit"));
    let copied = "<button><b>A<button>B</button>C";
    assert_eq!(parse_inert_html(copied, 11, &|| Ok(())).unwrap().0.nodes.len(), 11);
    assert!(parse_inert_html(copied, 10, &|| Ok(())).unwrap_err().contains("node limit"));
    let full = "<button>".repeat(MAX_NODES - 1);
    assert_eq!(parse_fragment(&full, "div", true).unwrap().nodes.len(), MAX_NODES);
    assert!(parse_fragment(&format!("{full}<button>"), "div", true).unwrap_err().contains("node/depth limit"));
}

#[test]
fn button_start_performs_target_closure_before_depth_admission() {
    let source = format!("{}<button id=old><span>A<button id=new>B", "<div>".repeat(125));
    let doc = parse_fragment(&source, "div", true).unwrap();
    assert_eq!(doc.nodes.len(), 131);
    assert_eq!(parent(&doc, id(&doc, "old")), parent(&doc, id(&doc, "new")));
    let rejected = format!("{}<button><span>", "<div>".repeat(126));
    assert!(parse_fragment(&rejected, "div", true).unwrap_err().contains("node/depth limit"));
}

#[test]
fn button_start_cancellation_discards_prefix_and_a_fresh_load_recovers() {
    let source = format!("<div id=host>{}", "<button>".repeat(2048));
    let calls = std::cell::Cell::new(0);
    let check = || { calls.set(calls.get() + 1); if calls.get() == 64 { Err("native button-start cancellation".into()) } else { Ok(()) } };
    assert_eq!(parse_loaded_document(&source, &check).unwrap_err(), "native button-start cancellation");
    assert_eq!(calls.get(), 64);
    let fresh = parse_loaded_document(&source, &|| Ok(())).unwrap();
    assert_eq!(fresh.nodes[id(&fresh, "host")].children.len(), 2048);
    assert_eq!(fresh.nodes.len(), 2053);
}

#[test]
fn button_start_preserves_literal_text_modes_and_existing_checked_refusals() {
    let source = "<button>A&amp;<button>B";
    for context in ["title", "textarea", "style", "script", "iframe", "plaintext"] {
        let doc = parse_fragment(source, context, true).unwrap(); assert_eq!(doc.nodes.len(), 2);
        assert_eq!(contents(&doc, 0), if matches!(context, "title" | "textarea") { "<button>A&<button>B" } else { source });
    }
    for source in ["<select><button>A<button>B</select>", "<table><button>A</table>", "<button>A<button>B\0"] {
        assert!(parse_fragment(source, "div", true).is_err());
    }
    assert!(parse_fragment("<button><svg><g><button></button></g></svg>", "template", true).is_err());
    for suffix in ["<table><tr></tr></table>", "<svg></svg>", "<select><input></select>"] {
        assert!(parse_fragment(&format!("<button>A<button>B{suffix}"), "div", true).is_err());
        assert_eq!(contents(&parse_fragment("<button>A<button>B", "div", true).unwrap(), 0), "<button>A</button><button>B</button>");
    }
}
