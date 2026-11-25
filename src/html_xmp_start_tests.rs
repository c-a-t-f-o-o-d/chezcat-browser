//! Original native XMP paragraph-closure and RAWTEXT witnesses.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100_000, &|| Ok(())).unwrap().0]
}
fn id(doc: &Document, value: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}
fn element(doc: &Document, name: &str) -> usize {
    doc.nodes.iter().position(|node| matches!(&node.kind,
        NodeKind::Element { tag, .. } if tag == name)).unwrap()
}
fn tree(doc: &Document, node: usize) -> String {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", contents(doc, node)),
        NodeKind::Text(value) => value.as_str().to_owned(),
        other => panic!("unexpected XMP witness node: {other:?}"),
    }
}
fn contents(doc: &Document, node: usize) -> String {
    doc.nodes[node].children.iter().map(|&child| tree(doc, child)).collect()
}
fn parent(doc: &Document, child: usize) -> usize {
    doc.nodes.iter().position(|node| node.children.contains(&child)).unwrap()
}
fn unique_links(doc: &Document) {
    let mut counts = vec![0; doc.nodes.len()];
    for node in &doc.nodes { for &child in &node.children { counts[child] += 1; } }
    assert_eq!(counts[0], 0);
    assert!(counts[1..].iter().all(|&count| count == 1));
}

#[test]
fn xmp_start_closes_the_current_paragraph_before_raw_element_insertion() {
    for doc in documents("<div id=host><p id=old>A<xmp id=subject>B</xmp>C</div>") {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><p>A</p><xmp>B</xmp>C</div>");
        assert_eq!(parent(&doc, id(&doc, "old")), id(&doc, "host"));
        assert_eq!(parent(&doc, id(&doc, "subject")), id(&doc, "host"));
        assert_eq!(doc.nodes[id(&doc, "subject")].namespace, Namespace::Html);
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_closes_a_paragraph_through_inline_descendants_without_moving_them() {
    for wrapper in ["span", "label", "x-inline"] {
        let source = format!("<div id=host><p id=old>A<{wrapper} id=inside>B<xmp id=subject>C</xmp>D</div>");
        for doc in documents(&source) {
            assert_eq!(tree(&doc, id(&doc, "host")), format!("<div><p>A<{wrapper}>B</{wrapper}></p><xmp>C</xmp>D</div>"));
            assert_eq!(parent(&doc, id(&doc, "inside")), id(&doc, "old"));
            assert_eq!(parent(&doc, id(&doc, "subject")), id(&doc, "host"));
            unique_links(&doc);
        }
    }
}

#[test]
fn xmp_start_without_a_paragraph_preserves_the_existing_current_parent() {
    for doc in documents("<div id=host><span id=inside>A<xmp id=subject>B</xmp>C</span>D</div>") {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><span>A<xmp>B</xmp>C</span>D</div>");
        assert_eq!(parent(&doc, id(&doc, "subject")), id(&doc, "inside"));
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_respects_button_scope_boundaries_around_an_outer_paragraph() {
    for boundary in ["button", "object", "applet", "marquee", "select", "template"] {
        let source = format!("<div id=host><p id=old>A<{boundary} id=gate><xmp id=subject>B</xmp>C</{boundary}>D</div>");
        for doc in documents(&source) {
            assert_eq!(tree(&doc, id(&doc, "host")), format!("<div><p>A<{boundary}><xmp>B</xmp>C</{boundary}>D</p></div>"));
            assert_eq!(parent(&doc, id(&doc, "subject")), id(&doc, "gate"));
            unique_links(&doc);
        }
    }
}

#[test]
fn xmp_start_closes_only_the_nearest_paragraph_inside_a_button_boundary() {
    let source = "<div id=host><p id=outer>A<button id=gate><p id=inner>B<xmp id=subject>C</xmp>D</button>E</div>";
    for doc in documents(source) {
        assert_eq!(tree(&doc, id(&doc, "host")), "<div><p>A<button><p>B</p><xmp>C</xmp>D</button>E</p></div>");
        assert_eq!(parent(&doc, id(&doc, "subject")), id(&doc, "gate"));
        assert_eq!(parent(&doc, id(&doc, "gate")), id(&doc, "outer"));
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_reconstructs_formatting_after_closing_the_paragraph() {
    let source = "<p id=old><b data-copy=kept>A<xmp id=subject>B</xmp>C";
    for doc in documents(source) {
        let body = element(&doc, "body");
        assert_eq!(contents(&doc, body), "<p><b>A</b></p><b><xmp>B</xmp>C</b>");
        let original = doc.nodes[id(&doc, "old")].children[0];
        let copy = doc.nodes[body].children[1];
        assert_ne!(original, copy);
        assert_eq!(doc.attr(original, "data-copy"), Some("kept"));
        assert_eq!(doc.attr(copy, "data-copy"), Some("kept"));
        assert_eq!(parent(&doc, id(&doc, "subject")), copy);
        assert_eq!(contents(&doc, original), "A");
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_refreshes_foster_placement_after_paragraph_closure_exposes_table() {
    let source = "<body><table id=table><p id=old>A<xmp id=subject>B</xmp>C</table>D";
    for doc in documents(source) {
        let body = element(&doc, "body");
        assert_eq!(contents(&doc, body), "<p>A</p><xmp>B</xmp>C<table></table>D");
        assert_eq!(parent(&doc, id(&doc, "old")), body);
        assert_eq!(parent(&doc, id(&doc, "subject")), body);
        assert!(doc.nodes[id(&doc, "table")].children.is_empty());
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_refreshes_foster_placement_before_reconstructing_formatting() {
    let source = "<body><table id=table><p id=old><b data-copy=kept>A<xmp id=subject>B</xmp>C</table>D";
    for doc in documents(source) {
        let body = element(&doc, "body");
        assert_eq!(contents(&doc, body), "<p><b>A</b></p><b><xmp>B</xmp>C</b><table></table><b>D</b>");
        let original = doc.nodes[id(&doc, "old")].children[0];
        let around_xmp = doc.nodes[body].children[1];
        let after_table = doc.nodes[body].children[3];
        assert_ne!(original, around_xmp); assert_ne!(around_xmp, after_table); assert_ne!(original, after_table);
        for node in [original, around_xmp, after_table] { assert_eq!(doc.attr(node, "data-copy"), Some("kept")); }
        assert_eq!(parent(&doc, id(&doc, "subject")), around_xmp);
        assert!(doc.nodes[id(&doc, "table")].children.is_empty());
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_keeps_actual_cell_and_caption_placement() {
    for (prefix, suffix) in [("<table><tr><td id=host>", "</td></tr></table>"),
        ("<table><caption id=host>", "</caption></table>")] {
        let source = format!("{prefix}<p id=old>A<xmp id=subject>B</xmp>C{suffix}");
        for doc in documents(&source) {
            assert_eq!(contents(&doc, id(&doc, "host")), "<p>A</p><xmp>B</xmp>C");
            assert_eq!(parent(&doc, id(&doc, "subject")), id(&doc, "host"));
            unique_links(&doc);
        }
    }
}

#[test]
fn xmp_start_preserves_exact_raw_text_newlines_references_and_null_replacement() {
    let source = "<p id=old>A<XMP id=subject data-note='a&amp;b'>\n<span>literal</span>&amp;\0</XmP><i id=tail>B</i>";
    for doc in documents(source) {
        let body = element(&doc, "body"); let subject = id(&doc, "subject");
        assert_eq!(doc.nodes[body].children, [id(&doc, "old"), subject, id(&doc, "tail")]);
        assert_eq!(contents(&doc, subject), "\n<span>literal</span>&amp;\u{fffd}");
        assert_eq!(doc.attr(subject, "data-note"), Some("a&b"));
        assert_eq!(doc.nodes[subject].children.len(), 1);
        assert!(matches!(&doc.nodes[doc.nodes[subject].children[0]].kind, NodeKind::Text(_)));
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_requires_a_complete_exact_start_token_and_handles_raw_eof() {
    for suffix in ["", " id=unfinished", " id='unfinished"] {
        let source = format!("<div id=host><p>A<xmp{suffix}");
        for doc in documents(&source) { assert_eq!(contents(&doc, id(&doc, "host")), "<p>A</p>"); }
    }
    for suffix in ["-extra", "é"] {
        let source = format!("<div id=host><p>A<xmp{suffix}>B");
        for doc in documents(&source) { assert_eq!(contents(&doc, id(&doc, "host")), format!("<p>A<xmp{suffix}>B</xmp{suffix}></p>")); }
    }
    for doc in documents("<div id=host><p>A<xmp id=subject>\nB") {
        assert_eq!(contents(&doc, id(&doc, "host")), "<p>A</p><xmp>\nB</xmp>");
    }
}

#[test]
fn xmp_start_preserves_checked_refusals_and_text_only_contexts() {
    for context in ["div", "p", "template"] {
        for scripting in [false, true] {
            assert!(parse_fragment("<p>A<xmp>B</xmp>", context, scripting).unwrap_err().contains("structural"));
            let doc = parse_fragment("<p>A<xmp>B</xmp>", "xmp", scripting).unwrap();
            assert_eq!(contents(&doc, 0), "<p>A<xmp>B</xmp>");
            assert_eq!(doc.nodes.len(), 2);
            assert_eq!(doc.scripting_enabled, scripting);
        }
    }
    for context in ["script", "style", "textarea", "plaintext"] {
        let doc = parse_fragment("<p>A<xmp>B</xmp>", context, false).unwrap();
        assert_eq!(contents(&doc, 0), "<p>A<xmp>B</xmp>");
    }
}

#[test]
fn xmp_start_preserves_flat_foreign_and_uncertain_native_subsets() {
    let flat = parse("<div id=host><p>A<xmp>B</xmp>C");
    assert_eq!(contents(&flat, id(&flat, "host")), "<p>A<xmp>B</xmp>C</p>");
    for foreign in ["svg", "math"] {
        for doc in documents(&format!("<{foreign}><xmp id=subject>\nA</xmp></{foreign}>")) {
            assert_eq!(doc.nodes[id(&doc, "subject")].namespace, Namespace::UnknownForeign);
            assert_eq!(contents(&doc, id(&doc, "subject")), "\nA");
        }
    }
    for doc in documents("<div><svg><span></div><xmp id=subject>\nA</xmp>") {
        assert_eq!(doc.nodes[id(&doc, "subject")].namespace, Namespace::UnknownForeign);
        assert_eq!(contents(&doc, id(&doc, "subject")), "\nA");
    }
}

#[test]
fn xmp_start_counts_actual_retained_and_reconstructed_nodes() {
    for (source, expected, nodes) in [
        ("<p>A<xmp></xmp>", "<p>A</p><xmp></xmp>", 7),
        ("<p><b>A<xmp>B</xmp>C", "<p><b>A</b></p><b><xmp>B</xmp>C</b>", 11),
    ] {
        let (doc, doctype) = parse_inert_html(source, nodes, &|| Ok(())).unwrap();
        assert_eq!(doc.nodes.len(), nodes);
        assert_eq!(doctype, None);
        assert_eq!(contents(&doc, element(&doc, "body")), expected);
        assert!(parse_inert_html(source, nodes - 1, &|| Ok(())).unwrap_err().contains("node limit"));
        unique_links(&doc);
    }
}

#[test]
fn xmp_start_remains_cancelable_and_fresh_parses_recover() {
    let source = format!("<p>A<xmp>{}B</xmp>", "<span>".repeat(20_000));
    for inert in [false, true] {
        let calls = std::cell::Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            if calls.get() == 8 { Err("original XMP cancellation".into()) } else { Ok(()) }
        };
        let error = if inert { parse_inert_html(&source, 1000, &cancel).unwrap_err() }
            else { parse_loaded_document(&source, &cancel).unwrap_err() };
        assert_eq!(error, "original XMP cancellation");
        assert_eq!(calls.get(), 8);
    }
    for doc in documents("<p>A<xmp>B</xmp>C") {
        assert_eq!(contents(&doc, element(&doc, "body")), "<p>A</p><xmp>B</xmp>C");
        unique_links(&doc);
    }
}
