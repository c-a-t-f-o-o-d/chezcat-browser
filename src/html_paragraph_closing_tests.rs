//! Paragraph closure for the ordinary in-body block-start group.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
//! https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-button-scope
use super::*;

const BLOCKS: &[&str] = &[
    "address", "aside", "details", "dialog", "dir", "dl", "fieldset",
    "figcaption", "figure", "hgroup", "menu", "nav", "search", "summary",
];

fn id(doc: &Document, value: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}

fn children(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent].children.iter().map(|&node| match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag.clone(),
        NodeKind::Text(value) => format!("#{value}"),
        NodeKind::Comment(value) => format!("!{value}"),
        _ => panic!("unexpected child kind"),
    }).collect()
}

fn text(doc: &Document) -> String {
    doc.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(value) => Some(value.render_projection()), _ => None,
    }).collect()
}

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 1000, &|| Ok(())).unwrap().0]
}

fn assert_block_siblings(doc: &Document, host: usize, name: &str) {
    let old = id(doc, "old");
    let block = id(doc, "new");
    let tail = id(doc, "tail");
    assert_eq!(doc.nodes[host].children, [old, block, tail], "{name}");
    assert_eq!(children(doc, old), ["#A"], "{name}");
    assert_eq!(children(doc, block), ["#B"], "{name}");
    assert_eq!(children(doc, tail), ["#C"], "{name}");
    assert_eq!(text(doc), "ABC", "{name}");
    assert_eq!(doc.nodes[block].namespace, Namespace::Html);
}

#[test]
fn paragraph_block_starts_close_actual_loaded_and_inert_paragraphs() {
    for name in BLOCKS {
        let source = format!("<!doctype html><div id=host><p id=old>A<{name} id=new>B</{name}><p id=tail>C</p></div>");
        for doc in documents(&source) {
            assert_block_siblings(&doc, id(&doc, "host"), name);
            assert_eq!(doc.mode, DocumentMode::NoQuirks);
        }
    }
}

#[test]
fn paragraph_block_starts_work_in_checked_fragments_with_real_context() {
    for name in BLOCKS {
        let source = format!("<p id=old>A<{name} id=new>B</{name}><p id=tail>C</p>");
        for context in ["div", "template"] {
            for scripting in [false, true] {
                let doc = parse_fragment(&source, context, scripting).unwrap();
                assert_block_siblings(&doc, 0, name);
                assert_eq!(doc.scripting_enabled, scripting);
                assert_eq!(doc.mode, DocumentMode::NoQuirks);
            }
        }
    }
}

#[test]
fn paragraph_closure_does_not_pop_ordinary_inline_ancestors_without_a_paragraph() {
    for name in BLOCKS {
        let source = format!("<div id=host><span id=inline>A<{name} id=new>B</{name}>C</span></div>");
        for doc in documents(&source) {
            let inline = id(&doc, "inline");
            assert_eq!(doc.nodes[id(&doc, "host")].children, [inline]);
            assert_eq!(children(&doc, inline), ["#A", name, "#C"]);
            assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
        }
        let doc = parse_fragment(&source, "div", false).unwrap();
        assert_eq!(children(&doc, id(&doc, "inline")), ["#A", name, "#C"]);
    }
}

#[test]
fn paragraph_closure_pops_through_nonformatting_inline_nodes_without_reparenting_them() {
    let source = "<div id=host><p id=old><span id=inline>A<address id=new>B</address>C</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(children(&doc, id(&doc, "host")), ["p", "address", "#C"]);
        assert_eq!(doc.nodes[id(&doc, "old")].children, [id(&doc, "inline")]);
        assert_eq!(children(&doc, id(&doc, "inline")), ["#A"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
    }
}

#[test]
fn paragraph_closure_preserves_active_formatting_reconstruction_and_attributes() {
    let source = "<div id=host><p id=old><b id=marker data-label=kept>A<address id=new>B</address>C</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let host = id(&doc, "host");
        let old = id(&doc, "old");
        let block = id(&doc, "new");
        assert_eq!(children(&doc, host), ["p", "address", "b"]);
        assert_eq!(children(&doc, old), ["b"]);
        assert_eq!(children(&doc, block), ["b"]);
        let nodes = [doc.nodes[old].children[0], doc.nodes[block].children[0], doc.nodes[host].children[2]];
        assert_ne!(nodes[0], nodes[1]); assert_ne!(nodes[1], nodes[2]); assert_ne!(nodes[0], nodes[2]);
        for (node, value) in nodes.into_iter().zip(["#A", "#B", "#C"]) {
            assert_eq!(doc.attr(node, "id"), Some("marker"));
            assert_eq!(doc.attr(node, "data-label"), Some("kept"));
            assert_eq!(children(&doc, node), [value]);
        }
        let mut counts = vec![0; doc.nodes.len()];
        for node in &doc.nodes { for &child in &node.children { counts[child] += 1; } }
        assert_eq!(counts[0], 0); assert!(counts[1..].iter().all(|&count| count == 1));
    }
}

#[test]
fn paragraph_button_scope_barriers_keep_older_paragraphs_and_native_children() {
    for barrier in ["object", "applet", "marquee", "button"] {
        for block in ["address", "div"] {
            let source = format!("<div id=host><p id=old>A<{barrier} id=barrier>B<{block} id=new>C</{block}>D</{barrier}>E</div>");
            for doc in documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
                let old = id(&doc, "old");
                let boundary = id(&doc, "barrier");
                assert_eq!(doc.nodes[id(&doc, "host")].children, [old], "{barrier}/{block}");
                assert_eq!(children(&doc, old), ["#A", barrier, "#E"], "{barrier}/{block}");
                assert_eq!(children(&doc, boundary), ["#B", block, "#D"], "{barrier}/{block}");
                assert_eq!(children(&doc, id(&doc, "new")), ["#C"]);
                assert_eq!(text(&doc), "ABCDE");
            }
        }
    }
}

#[test]
fn paragraph_button_scope_closes_the_nearest_paragraph_before_a_barrier() {
    let source = "<div id=host><p id=outer>A<object id=boundary><p id=old>B<address id=new>C</address>D</object>E</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", false).unwrap()]) {
        let outer = id(&doc, "outer");
        assert_eq!(doc.nodes[id(&doc, "host")].children, [outer]);
        assert_eq!(children(&doc, outer), ["#A", "object", "#E"]);
        assert_eq!(children(&doc, id(&doc, "boundary")), ["p", "address", "#D"]);
        assert_eq!(children(&doc, id(&doc, "old")), ["#B"]);
        assert_eq!(children(&doc, id(&doc, "new")), ["#C"]);
    }
}

#[test]
fn paragraph_closure_keeps_nested_template_scope_and_the_fragment_root_floor() {
    let source = "<div id=host><p id=outer>A<template id=boundary><p id=old>B<address id=new>C</address>D</template>E</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "template", true).unwrap()]) {
        assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "outer")]);
        assert_eq!(children(&doc, id(&doc, "outer")), ["#A", "template", "#E"]);
        assert_eq!(children(&doc, id(&doc, "boundary")), ["p", "address", "#D"]);
    }
    let doc = parse_fragment("<address id=new>B</address>C", "p", true).unwrap();
    assert_eq!(children(&doc, 0), ["address", "#C"]);
    assert_eq!(children(&doc, id(&doc, "new")), ["#B"]);
}

fn scope_document(boundary: &str, namespace: Namespace) -> (Document, usize, usize, usize) {
    let mut doc = parse("<p id=target></p><span id=boundary></span><span id=current></span>");
    let target = id(&doc, "target");
    let barrier = id(&doc, "boundary");
    let current = id(&doc, "current");
    if let NodeKind::Element { tag, .. } = &mut doc.nodes[barrier].kind { *tag = boundary.into(); }
    doc.nodes[barrier].namespace = namespace;
    doc.nodes[0].children = vec![target];
    doc.nodes[target].children = vec![barrier];
    doc.nodes[barrier].children = vec![current];
    (doc, target, barrier, current)
}

#[test]
fn paragraph_button_scope_uses_all_html_boundaries_and_checks_the_target_first() {
    for boundary in ["applet", "caption", "html", "table", "td", "th", "marquee", "object", "select", "template", "button"] {
        let (doc, target, barrier, current) = scope_document(boundary, Namespace::Html);
        let mut stack = vec![0, target, barrier, current];
        close_implied(&mut stack, &doc, "address");
        assert_eq!(stack, [0, target, barrier, current], "{boundary}");
        // The nearest real paragraph is found before the older boundary.
        let mut stack = vec![0, barrier, target, current];
        close_implied(&mut stack, &doc, "address");
        assert_eq!(stack, [0, barrier], "{boundary}");
    }
    for ordinary in ["span", "div", "mi", "foreignObject"] {
        let (doc, target, middle, current) = scope_document(ordinary, Namespace::Html);
        let mut stack = vec![0, target, middle, current];
        close_implied(&mut stack, &doc, "address");
        assert_eq!(stack, [0], "ordinary HTML name {ordinary}");
    }
}

#[test]
fn paragraph_button_scope_keeps_uncertain_foreign_nodes_outside_this_html_slice() {
    for namespace in [Namespace::Svg, Namespace::UnknownForeign, Namespace::Other] {
        let (mut doc, target, barrier, current) = scope_document("g", namespace);
        let mut stack = vec![0, target, barrier, current];
        close_implied(&mut stack, &doc, "address");
        assert_eq!(stack, [0, target, barrier, current]);
        doc.nodes[target].namespace = namespace;
        let mut stack = vec![0, target];
        close_implied(&mut stack, &doc, "address");
        assert_eq!(stack, [0, target]);
    }
}

#[test]
fn paragraph_block_slice_retains_definition_heading_and_other_checked_refusals() {
    for (source, expected) in [("<dt>A<dd>B", ["dt", "dd"]), ("<p>A<dt>B", ["p", "dt"]), ("<p>A<dd>B", ["p", "dd"])] {
        let doc = parse_fragment(source, "div", true).unwrap();
        assert_eq!(children(&doc, 0), expected, "{source}");
        assert_eq!(children(&doc, doc.nodes[0].children[0]), ["#A"]);
        assert_eq!(children(&doc, doc.nodes[0].children[1]), ["#B"]);
    }
    let headings = parse_fragment("<h1>A<h2>B", "div", true).unwrap();
    assert_eq!(children(&headings, 0), ["h1", "h2"]);
    assert_eq!(children(&headings, headings.nodes[0].children[0]), ["#A"]);
    assert_eq!(children(&headings, headings.nodes[0].children[1]), ["#B"]);
    for source in ["<p>A<listing>B", "<p>A<xmp>B</xmp>", "<p>A<svg></svg>", "<p>A<table><tr></tr></table>"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    assert!(parse_fragment("<p>A<address>\0", "div", true).unwrap_err().contains("NULL handling"));
    assert!(parse_fragment("<svg><g id='a'id='b'></g></svg>", "template", true).is_err());
}

#[test]
fn paragraph_closure_respects_real_node_capacity_and_cancellation_without_a_prefix() {
    let source = "<!doctype html><div id=host><p id=old><b id=marker>A<address id=new>B</address>C</div>";
    let (doc, doctype) = parse_inert_html(source, 14, &|| Ok(())).unwrap();
    assert_eq!(doc.nodes.len(), 14); assert_eq!(doctype, Some(1));
    assert_eq!(children(&doc, id(&doc, "host")), ["p", "address", "b"]);
    assert!(parse_inert_html(source, 13, &|| Ok(())).unwrap_err().contains("node limit"));
    let source = format!("<div>{}</div>", "<p>A<address>B</address>".repeat(512));
    let calls = std::cell::Cell::new(0);
    let error = parse_loaded_document(&source, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 8 { Err("paragraph scan cancelled".into()) } else { Ok(()) }
    }).unwrap_err();
    assert_eq!(error, "paragraph scan cancelled"); assert_eq!(calls.get(), 8);
    let recovered = parse_fragment("<p id=old>A<address id=new>B</address><p id=tail>C</p>", "div", false).unwrap();
    assert_block_siblings(&recovered, 0, "address");
}

#[test]
fn paragraph_closure_keeps_resource_attributes_and_duplicate_nonce_evidence() {
    let source = "<div id=host><p id=old><b>A<address id=new><script id=s nonce=one NONCE=two></script><img id=i src='image.png'>B</address>C</div>";
    for doc in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        let script = id(&doc, "s");
        let image = id(&doc, "i");
        let block = id(&doc, "new");
        assert_eq!(children(&doc, id(&doc, "host")), ["p", "address", "b"]);
        assert!(doc.nodes[block].children.contains(&script));
        assert_eq!(doc.attr(script, "nonce"), Some("one"));
        assert_eq!(doc.nonce_unsafe_nodes.len(), 1);
        assert!(doc.nonce_unsafe_nodes.contains(&script));
        assert_eq!(doc.attr(image, "src"), Some("image.png"));
        assert_eq!(text(&doc), "ABC");
    }
}
