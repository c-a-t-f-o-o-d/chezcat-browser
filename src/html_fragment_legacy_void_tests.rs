//! Native checked fragments immediately close BASEFONT and BGSOUND starts.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead
use super::*;

fn id(doc: &Document, value: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}
fn element(doc: &Document, name: &str) -> usize {
    (1..doc.nodes.len()).find(|&node| matches!(&doc.nodes[node].kind,
        NodeKind::Element { tag, .. } if tag == name)).unwrap()
}
fn contents(doc: &Document, node: usize) -> String {
    doc.nodes[node].children.iter().map(|&child| match &doc.nodes[child].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", contents(doc, child)),
        NodeKind::Text(text) => text.as_str().to_owned(),
        kind => panic!("unexpected legacy-element fixture node: {kind:?}"),
    }).collect()
}
fn empty_html(doc: &Document, node: usize, expected: &str) {
    assert_eq!(doc.nodes[node].namespace, Namespace::Html);
    assert!(matches!(&doc.nodes[node].kind, NodeKind::Element { tag, .. } if tag == expected));
    assert!(doc.nodes[node].children.is_empty());
}
fn unique_parents(doc: &Document) {
    let mut parents = vec![0; doc.nodes.len()];
    for node in &doc.nodes { for &child in &node.children { parents[child] += 1; } }
    assert!(parents[1..].iter().all(|&count| count == 1));
}

#[test]
fn fragment_legacy_void_starts_keep_following_text_and_elements_as_siblings() {
    let source = "<basefont id=first>A<bgsound id=second>B<span>C</span>D";
    for context in ["div", "span", "p", "h1", "template"] { for scripting in [false, true] {
        let doc = parse_fragment(source, context, scripting).unwrap();
        assert_eq!(doc.nodes[0].namespace, Namespace::None);
        assert!(matches!(&doc.nodes[0].kind, NodeKind::Element { tag, .. } if tag == context));
        assert_eq!(contents(&doc, 0), "<basefont></basefont>A<bgsound></bgsound>B<span>C</span>D");
        assert_eq!(doc.nodes.len(), 8);
        empty_html(&doc, id(&doc, "first"), "basefont"); empty_html(&doc, id(&doc, "second"), "bgsound");
        assert_eq!(doc.scripting_enabled, scripting); unique_parents(&doc);
    }}
}

#[test]
fn fragment_legacy_void_keeps_folded_names_first_attributes_and_slash_tokens() {
    for name in ["basefont", "bgsound"] { for slash in ["", "/"] {
        let source = format!("<{} id=kept data-note='a&amp;b' DATA-NOTE=ignored xmlns='urn:other'{slash}>A<span>B</span>C", name.to_ascii_uppercase());
        let doc = parse_fragment(&source, "div", true).unwrap();
        assert_eq!(contents(&doc, 0), format!("<{name}></{name}>A<span>B</span>C"));
        let node = id(&doc, "kept"); empty_html(&doc, node, name);
        assert_eq!(doc.attr(node, "data-note"), Some("a&b")); assert_eq!(doc.attr(node, "xmlns"), Some("urn:other"));
        assert!(!doc.nonce_unsafe_nodes.contains(&node));
    }}
}

#[test]
fn fragment_legacy_void_end_tags_do_not_reopen_or_split_following_text() {
    for name in ["basefont", "bgsound"] { for end in [">", " data-discarded=x>", "/>"] {
        let doc = parse_fragment(&format!("<{name} id=kept>A</{name}{end}B"), "div", true).unwrap();
        assert_eq!(contents(&doc, 0), format!("<{name}></{name}>AB"));
        assert_eq!(doc.nodes.len(), 3); empty_html(&doc, id(&doc, "kept"), name);
        assert_eq!(doc.attr(id(&doc, "kept"), "data-discarded"), None);
    }}
}

#[test]
fn fragment_legacy_void_requires_complete_exact_start_names() {
    for name in ["basefont", "bgsound"] {
        for suffix in ["", " id=unfinished", " id='unfinished"] {
            let doc = parse_fragment(&format!("A<{name}{suffix}"), "div", true).unwrap();
            assert_eq!(doc.nodes.len(), 2); assert_eq!(contents(&doc, 0), "A");
        }
        for suffix in ["-extra", "é"] {
            let doc = parse_fragment(&format!("<{name}{suffix}>A<span>B</span>C"), "div", true).unwrap();
            assert_eq!(contents(&doc, 0), format!("<{name}{suffix}>A<span>B</span>C</{name}{suffix}>"));
        }
    }
}

#[test]
fn fragment_legacy_void_does_not_absorb_script_or_nonce_metadata() {
    for name in ["basefont", "bgsound"] {
        let source = format!("<{name} id=legacy><script id=script nonce=first NONCE=second>literal</script>tail");
        let doc = parse_fragment(&source, "div", true).unwrap();
        assert_eq!(contents(&doc, 0), format!("<{name}></{name}><script>literal</script>tail"));
        empty_html(&doc, id(&doc, "legacy"), name);
        assert_eq!(doc.attr(id(&doc, "script"), "nonce"), Some("first"));
        assert!(doc.nonce_unsafe_nodes.contains(&id(&doc, "script")));
        assert!(!doc.nonce_unsafe_nodes.contains(&id(&doc, "legacy")));
    }
}

#[test]
fn fragment_legacy_void_preserves_open_and_later_reconstructed_formatting() {
    for name in ["basefont", "bgsound"] {
        let doc = parse_fragment(&format!("<p><b data-copy=kept>A</p><{name}>B"), "div", true).unwrap();
        assert_eq!(contents(&doc, 0), format!("<p><b>A</b></p><{name}></{name}><b>B</b>"));
        assert_eq!(doc.nodes.len(), 7);
        let original = doc.nodes[element(&doc, "p")].children[0]; let copy = doc.nodes[0].children[2];
        assert_ne!(original, copy); assert_eq!(doc.attr(original, "data-copy"), Some("kept")); assert_eq!(doc.attr(copy, "data-copy"), Some("kept"));
        empty_html(&doc, element(&doc, name), name); unique_parents(&doc);
        let doc = parse_fragment(&format!("<p><b>A<{name}>B</b>C</p>D"), "div", false).unwrap();
        assert_eq!(contents(&doc, 0), format!("<p><b>A<{name}></{name}>B</b>C</p>D"));
        empty_html(&doc, element(&doc, name), name); unique_parents(&doc);
    }
}

#[test]
fn fragment_legacy_void_keeps_initial_and_nested_template_metadata_state() {
    for name in ["basefont", "bgsound"] { for scripting in [false, true] {
        let source = format!("<{name} id=outer>A</p></br>B<template id=inner><{name} id=nested>C</p></br>D</template>E");
        let doc = parse_fragment(&source, "template", scripting).unwrap();
        assert_eq!(contents(&doc, 0), format!("<{name}></{name}>AB<template><{name}></{name}>CD</template>E"));
        empty_html(&doc, id(&doc, "outer"), name); empty_html(&doc, id(&doc, "nested"), name);
        assert_eq!(doc.nodes.len(), 7); unique_parents(&doc);
    }}
}

#[test]
fn fragment_legacy_void_retains_existing_loaded_and_inert_head_behavior() {
    let source = "<head><basefont id=a><bgsound id=b><title>T</title></head><body><span>U</span>";
    for doc in [parse_loaded_document(source, &|| Ok(())).unwrap(), parse_inert_html(source, 100, &|| Ok(())).unwrap().0] {
        assert_eq!(contents(&doc, element(&doc, "head")), "<basefont></basefont><bgsound></bgsound><title>T</title>");
        assert_eq!(contents(&doc, element(&doc, "body")), "<span>U</span>");
        empty_html(&doc, id(&doc, "a"), "basefont"); empty_html(&doc, id(&doc, "b"), "bgsound");
    }
    // The flat parser's current foreign fallback must not inherit a new
    // shared void classification from this checked native HTML slice.
    let foreign = parse("<math><basefont id=a>A<bgsound id=b>B");
    assert_eq!(contents(&foreign, 0), "<math><basefont>A<bgsound>B</bgsound></basefont></math>");
    assert_eq!(foreign.nodes[id(&foreign, "a")].namespace, Namespace::UnknownForeign);
    assert_eq!(foreign.nodes[id(&foreign, "b")].namespace, Namespace::UnknownForeign);
}

#[test]
fn fragment_legacy_void_uses_real_node_budget_without_growing_open_depth() {
    let limited = |source: &str, nodes| parse_internal(source, Some("div"), true,
        Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    for name in ["basefont", "bgsound"] {
        let source = format!("<{name}>A");
        assert_eq!(limited(&source, 3).unwrap().nodes.len(), 3);
        assert!(limited(&source, 2).unwrap_err().contains("node limit"));
        let full = format!("<{name}>").repeat(MAX_NODES - 1);
        assert_eq!(parse_fragment(&full, "div", true).unwrap().nodes.len(), MAX_NODES);
        assert!(parse_fragment(&format!("{full}<{name}>"), "div", true).unwrap_err().contains("node/depth limit"));
    }
    let deep = format!("{}<basefont><bgsound>A", "<div>".repeat(MAX_DEPTH - 1));
    let doc = parse_fragment(&deep, "div", true).unwrap();
    assert_eq!(doc.nodes.len(), MAX_DEPTH + 3);
    assert!(parse_fragment(&format!("{deep}<span>"), "div", true).unwrap_err().contains("node/depth limit"));
}

#[test]
fn fragment_legacy_void_cancellation_discards_a_prefix_and_fresh_parse_recovers() {
    let source = "<basefont><bgsound>".repeat(1024);
    let calls = std::cell::Cell::new(0);
    let check = || { calls.set(calls.get() + 1); if calls.get() == 64 { Err("legacy fragment cancellation".into()) } else { Ok(()) } };
    let error = parse_internal(&source, Some("div"), true,
        Some(LoadedTree { nodes: 2049, check_cancel: &check }), None).unwrap_err();
    assert_eq!(error, "legacy fragment cancellation"); assert_eq!(calls.get(), 64);
    let fresh = parse_internal(&source, Some("div"), true,
        Some(LoadedTree { nodes: 2049, check_cancel: &|| Ok(()) }), None).unwrap();
    assert_eq!(fresh.nodes.len(), 2049); assert_eq!(fresh.nodes[0].children.len(), 2048);
    assert!(fresh.nodes.iter().skip(1).all(|node| node.children.is_empty()));
}

#[test]
fn fragment_legacy_void_preserves_frame_foreign_select_table_and_context_refusals() {
    for context in ["basefont", "bgsound", "frame", "button"] {
        assert!(parse_fragment("A", context, true).unwrap_err().contains("element context"));
    }
    for name in ["basefont", "bgsound"] {
        for source in [format!("<frame><{name}>"), format!("<{name}><frame>"), format!("<{name}><frameset></frameset>"),
            format!("<select><{name}></select>"), format!("<table><{name}></table>"), format!("<{name}>\0")] {
            assert!(parse_fragment(&source, "div", true).is_err(), "{source}");
        }
        assert!(parse_fragment(&format!("<svg><g><{name}></g></svg>"), "template", true).is_err());
        assert!(parse_fragment(&format!("<{name}><svg></svg>"), "div", true).is_err());
        let fresh = parse_fragment(&format!("<{name}>A"), "div", true).unwrap();
        assert_eq!(contents(&fresh, 0), format!("<{name}></{name}>A"));
    }
}

#[test]
fn fragment_legacy_void_keeps_raw_and_rcdata_contexts_literal() {
    let source = "<basefont>A&amp;<bgsound>B";
    for context in ["title", "textarea", "style", "script", "iframe", "plaintext"] {
        let doc = parse_fragment(source, context, true).unwrap();
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(contents(&doc, 0), if matches!(context, "title" | "textarea") { "<basefont>A&<bgsound>B" } else { source });
    }
}
