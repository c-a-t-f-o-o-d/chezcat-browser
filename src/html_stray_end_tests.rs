//! Original native HTML oracles for attribute-free P/BR end-tag recovery.
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
     parse_inert_html(source, 50_000, &|| Ok(())).unwrap().0]
}

fn elements(document: &Document, name: &str) -> Vec<usize> {
    document.nodes.iter().enumerate().filter_map(|(node, item)| {
        (node != 0 && item.namespace == Namespace::Html
            && matches!(&item.kind, NodeKind::Element { tag, .. } if tag == name)).then_some(node)
    }).collect()
}

fn element(document: &Document, name: &str) -> usize {
    elements(document, name).into_iter().next().unwrap()
}

fn id(document: &Document, value: &str) -> usize {
    document.nodes.iter().position(|node| matches!(&node.kind,
        NodeKind::Element { id: Some(id), .. } if id == value)).unwrap()
}

fn children(document: &Document, parent: usize) -> Vec<String> {
    document.nodes[parent].children.iter().map(|&node| match &document.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag.clone(),
        NodeKind::Text(text) => format!("#{text}"),
        NodeKind::Comment(text) => format!("!{text}"),
        kind => panic!("Unexpected child: {kind:?}"),
    }).collect()
}

fn empty_native(document: &Document, node: usize, expected: &str) {
    assert_eq!(document.nodes[node].namespace, Namespace::Html);
    assert!(matches!(&document.nodes[node].kind,
        NodeKind::Element { tag, expanded_name, id: None, href: None, attrs }
            if tag == expected && expanded_name.is_empty() && attrs.is_empty()));
    assert!(document.nodes[node].children.is_empty());
    assert!(!document.nonce_unsafe_nodes.contains(&node));
}

#[test]
fn stray_p_ends_create_real_empty_siblings_in_loaded_and_inert_body() {
    for document in documents("<!doctype html><body>A</p>B</P >C</p/>D") {
        assert_eq!(document.mode, DocumentMode::NoQuirks);
        assert_eq!(children(&document, element(&document, "body")),
            ["#A", "p", "#B", "p", "#C", "p", "#D"]);
        let paragraphs = elements(&document, "p");
        assert_eq!(paragraphs.len(), 3);
        for node in paragraphs { empty_native(&document, node, "p"); }
        assert_eq!(document.nodes.len(), 12);
    }
}

#[test]
fn stray_p_ends_preserve_initial_head_after_head_and_post_body_dispatch() {
    for source in ["</p>", "<html></p>", "<head></p>", "<head></head></p>"] {
        for document in documents(source) {
            assert!(elements(&document, "p").is_empty(), "{source}");
            assert!(document.nodes[element(&document, "body")].children.is_empty());
            assert_eq!(document.nodes.len(), 4);
        }
    }
    for document in documents("<html></p><head></p><title>kept</title></head></p><body>A</p>B") {
        assert_eq!(children(&document, element(&document, "head")), ["title"]);
        assert_eq!(document.title, "kept");
        assert_eq!(children(&document, element(&document, "body")), ["#A", "p", "#B"]);
    }
    for document in documents("<body>A</body></p>B</html></p>C") {
        assert_eq!(children(&document, element(&document, "body")), ["#A", "p", "#B", "p", "#C"]);
    }
}

#[test]
fn stray_p_ends_keep_fragment_context_roots_and_scripting_flags() {
    for context in ["div", "p", "h1", "template"] {
        for scripting in [false, true] {
            let document = parse_fragment("A</p>B</p>C", context, scripting).unwrap();
            assert_eq!(document.scripting_enabled, scripting);
            assert_eq!(document.nodes[0].namespace, Namespace::None);
            assert!(matches!(&document.nodes[0].kind, NodeKind::Element { tag, .. } if tag == context));
            if context == "template" {
                assert_eq!(children(&document, 0), ["#ABC"]);
                assert!(elements(&document, "p").is_empty());
            } else {
                assert_eq!(children(&document, 0), ["#A", "p", "#B", "p", "#C"]);
            }
            for node in elements(&document, "p") { empty_native(&document, node, "p"); }
        }
    }
}

#[test]
fn stray_p_ends_respect_button_scope_and_match_a_target_before_its_barrier() {
    for boundary in ["button", "object", "applet", "marquee"] {
        let source = format!("<div id=host><p id=outer>A<{boundary} id=barrier>B</p>C</{boundary}>D</p>E</div>");
        for document in documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
            assert_eq!(children(&document, id(&document, "host")), ["p", "#E"]);
            assert_eq!(children(&document, id(&document, "outer")), ["#A", boundary, "#D"]);
            assert_eq!(children(&document, id(&document, "barrier")), ["#B", "p", "#C"]);
            let synthetic = document.nodes[id(&document, "barrier")].children[1];
            empty_native(&document, synthetic, "p");
        }
        let source = format!("<div id=host><{boundary} id=barrier><p id=inner>A<span>B</p>C</{boundary}>D</div>");
        for document in documents(&source).into_iter().chain([parse_fragment(&source, "div", false).unwrap()]) {
            assert_eq!(children(&document, id(&document, "host")), [boundary, "#D"]);
            assert_eq!(children(&document, id(&document, "barrier")), ["p", "#C"]);
            assert_eq!(children(&document, id(&document, "inner")), ["#A", "span"]);
            assert_eq!(children(&document, element(&document, "span")), ["#B"]);
            assert_eq!(elements(&document, "p").len(), 1);
        }
    }
}

#[test]
fn stray_p_ends_preserve_formatting_without_reconstructing_for_empty_p() {
    let source = "<div id=host><p><b id=fmt data-label=kept>A</p></p>B</b>C</div>";
    for document in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(children(&document, id(&document, "host")), ["p", "p", "b", "#C"]);
        let paragraphs = elements(&document, "p");
        assert_eq!(children(&document, paragraphs[0]), ["b"]);
        empty_native(&document, paragraphs[1], "p");
        let copies = elements(&document, "b");
        assert_eq!(copies.len(), 2);
        assert_eq!(children(&document, copies[0]), ["#A"]);
        assert_eq!(children(&document, copies[1]), ["#B"]);
        assert_ne!(copies[0], copies[1]);
        for node in copies { assert_eq!(document.attr(node, "data-label"), Some("kept")); }
    }
    let source = "<div id=host><b>A</p>B</b>C</div>";
    for document in documents(source).into_iter().chain([parse_fragment(source, "div", false).unwrap()]) {
        assert_eq!(children(&document, id(&document, "host")), ["b", "#C"]);
        assert_eq!(children(&document, element(&document, "b")), ["#A", "p", "#B"]);
        empty_native(&document, element(&document, "p"), "p");
    }
}

#[test]
fn stray_p_ends_keep_nested_template_scope_separate_from_an_outer_p() {
    let source = "<p id=outer>A<template id=content>B</p>C</template>D</p>E";
    for document in documents(source).into_iter().chain([parse_fragment(source, "template", true).unwrap()]) {
        let parent = if document.nodes[0].namespace == Namespace::None
            && matches!(&document.nodes[0].kind, NodeKind::Element { tag, .. } if tag == "template") { 0 }
            else { element(&document, "body") };
        assert_eq!(children(&document, parent), ["p", "#E"]);
        assert_eq!(children(&document, id(&document, "outer")), ["#A", "template", "#D"]);
        assert_eq!(children(&document, id(&document, "content")), ["#BC"]);
        assert_eq!(elements(&document, "p"), [id(&document, "outer")]);
    }
}

#[test]
fn stray_br_ends_create_real_void_siblings_in_documents_and_fragments() {
    let source = "<div id=host>A</br>B</BR >C</br/>D</div>";
    for document in documents(source).into_iter().chain(
        ["div", "p", "h1", "template"].into_iter().flat_map(|context|
            [false, true].into_iter().map(move |scripting| parse_fragment(source, context, scripting).unwrap()))) {
        assert_eq!(children(&document, id(&document, "host")), ["#A", "br", "#B", "br", "#C", "br", "#D"]);
        let breaks = elements(&document, "br");
        assert_eq!(breaks.len(), 3);
        for node in breaks { empty_native(&document, node, "br"); }
    }
}

#[test]
fn stray_br_ends_follow_document_head_and_post_body_reprocessing() {
    for source in ["</br>", "<html></br>", "<head></br>", "<head></head></br>"] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "body")), ["br"], "{source}");
            assert!(document.nodes[element(&document, "head")].children.is_empty());
            assert_eq!(document.nodes.len(), 5);
        }
    }
    for document in documents("<head><title>kept</title></br>B") {
        assert_eq!(children(&document, element(&document, "head")), ["title"]);
        assert_eq!(document.title, "kept");
        assert_eq!(children(&document, element(&document, "body")), ["br", "#B"]);
    }
    for document in documents("<body>A</body></br>B</html></br>C") {
        assert_eq!(children(&document, element(&document, "body")), ["#A", "br", "#B", "br", "#C"]);
    }
    for document in documents("<noscript></br id=discarded/><p>tail") {
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert_eq!(children(&document, element(&document, "body")), ["br", "p"]);
        empty_native(&document, element(&document, "br"), "br");
    }
}

#[test]
fn stray_br_ends_reconstruct_active_formatting_before_void_insertion() {
    for tail in ["", "B</b>C"] {
        let source = format!("<div id=host><p><b id=fmt data-label=kept>A</p></br>{tail}</div>");
        for document in documents(&source).into_iter().chain([parse_fragment(&source, "div", true).unwrap()]) {
            let copies = elements(&document, "b");
            assert_eq!(copies.len(), 2);
            assert_eq!(children(&document, copies[0]), ["#A"]);
            assert_eq!(children(&document, copies[1]), if tail.is_empty() { vec!["br"] } else { vec!["br", "#B"] });
            assert_eq!(children(&document, id(&document, "host")), if tail.is_empty() { vec!["p", "b"] } else { vec!["p", "b", "#C"] });
            for node in copies { assert_eq!(document.attr(node, "data-label"), Some("kept")); }
            empty_native(&document, element(&document, "br"), "br");
        }
    }
}

#[test]
fn stray_end_attributes_are_discarded_without_losing_real_resource_metadata() {
    let source = "<div id=host></p id=discarded href=/wrong src=/wrong nonce=a nonce=b style='color:red'></br id=discarded href=/wrong src=/wrong nonce=a nonce=b data-label=wrong><p id=real>A</p id=discarded nonce=a nonce=b><script nonce=one nonce=two>kept</script><style nonce=one nonce=two>p{color:red}</style><img id=image src=/kept></div>";
    for document in documents(source).into_iter().chain([parse_fragment(source, "div", true).unwrap()]) {
        assert_eq!(children(&document, id(&document, "host")), ["p", "br", "p", "script", "style", "img"]);
        let paragraphs = elements(&document, "p");
        empty_native(&document, paragraphs[0], "p");
        empty_native(&document, element(&document, "br"), "br");
        assert_eq!(paragraphs[1], id(&document, "real"));
        assert_eq!(children(&document, paragraphs[1]), ["#A"]);
        assert!(document.nodes.iter().all(|node| !matches!(&node.kind, NodeKind::Element { id: Some(id), .. } if id == "discarded")));
        assert_eq!(document.attr(id(&document, "image"), "src"), Some("/kept"));
        assert_eq!(document.nonce_unsafe_nodes.len(), 2);
        for name in ["script", "style"] { assert!(document.nonce_unsafe_nodes.contains(&element(&document, name))); }
    }
}

#[test]
fn stray_ends_use_normal_foster_locations_after_table_mode_dispatch() {
    for (middle, tail) in [("", ""), ("<tbody id=section>", "</tbody>"),
        ("<tbody id=section><tr id=row>", "</tr></tbody>"), ("<colgroup id=columns>", "")] {
        let source = format!("<body><table id=grid>{middle}</p id=discarded></br id=discarded>{tail}</table>tail");
        for document in documents(&source) {
            assert_eq!(children(&document, element(&document, "body")), ["p", "br", "table", "#tail"], "{source}");
            empty_native(&document, element(&document, "p"), "p");
            empty_native(&document, element(&document, "br"), "br");
            assert_eq!(document.attr(id(&document, "grid"), "id"), Some("grid"));
            if middle.is_empty() { assert!(document.nodes[id(&document, "grid")].children.is_empty()); }
            if middle.contains("id=row") { assert!(document.nodes[id(&document, "row")].children.is_empty()); }
            if middle.contains("id=columns") { assert!(document.nodes[id(&document, "columns")].children.is_empty()); }
        }
    }
}

#[test]
fn stray_ends_stay_inside_native_table_cells_captions_and_open_inline_nodes() {
    for (prefix, suffix) in [("<caption id=inside>", "</caption>"),
        ("<tbody><tr><td id=inside>", "</td></tr></tbody>")] {
        let source = format!("<body><table>{prefix}A</p></br>B{suffix}</table>");
        for document in documents(&source) {
            assert_eq!(children(&document, element(&document, "body")), ["table"]);
            assert_eq!(children(&document, id(&document, "inside")), ["#A", "p", "br", "#B"]);
            empty_native(&document, element(&document, "p"), "p");
            empty_native(&document, element(&document, "br"), "br");
        }
    }
    for document in documents("<body><table><b id=inline></p></br></b></table>") {
        assert_eq!(children(&document, element(&document, "body")), ["b", "table"]);
        assert_eq!(children(&document, id(&document, "inline")), ["p", "br"]);
    }
}

#[test]
fn stray_ends_require_complete_tokens_and_exact_tag_names() {
    for tail in ["</p", "</p ", "</p/", "</p id=", "</p id='x", "</br", "</br/", "</br id=\"x"] {
        let source = format!("<body>A{tail}");
        for document in documents(&source) {
            assert_eq!(children(&document, element(&document, "body")), ["#A"], "{tail}");
        }
        let document = parse_fragment(&format!("A{tail}"), "div", true).unwrap();
        assert_eq!(children(&document, 0), ["#A"], "{tail}");
    }
    let source = "A</p\u{b}>B</br\u{b}>C</paragraph>D</break>E";
    let fragment = parse_fragment(source, "div", true).unwrap();
    assert_eq!(children(&fragment, 0), ["#ABCDE"]);
    for document in documents(&format!("<body>{source}")) {
        assert_eq!(children(&document, element(&document, "body")), ["#ABCDE"]);
    }
}

#[test]
fn stray_ends_remain_literal_in_text_only_contexts_and_native_raw_elements() {
    let source = "A</p>B</br>C&amp;";
    for context in ["title", "textarea", "style", "script", "xmp", "iframe", "noembed", "noframes", "plaintext"] {
        let document = parse_fragment(source, context, true).unwrap();
        assert_eq!(document.nodes.len(), 2);
        assert_eq!(children(&document, 0), [if matches!(context, "title" | "textarea") {
            "#A</p>B</br>C&" } else { "#A</p>B</br>C&amp;" }]);
    }
    for name in ["script", "style", "textarea", "title", "xmp", "iframe", "noembed", "noframes"] {
        for document in documents(&format!("<body><{name}>{source}</{name}>")) {
            assert!(elements(&document, "p").is_empty());
            assert!(elements(&document, "br").is_empty());
            assert_eq!(children(&document, element(&document, name)), [if matches!(name, "title" | "textarea") {
                "#A</p>B</br>C&" } else { "#A</p>B</br>C&amp;" }]);
        }
    }
}

#[test]
fn stray_ends_preserve_strict_table_svg_select_null_and_later_refusals() {
    for source in ["<table></p></table>", "<table></br></table>", "<table><tr></tr></table>",
        "<select></p></select>", "<select></br></select>", "</p><svg></svg>", "</br>\0"] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source}");
    }
    for source in ["<svg></p></svg>", "<svg></br></svg>", "<svg><g></p></g></svg>",
        "<svg><g></br></g></svg>", "</p><svg><script>x</script></svg>"] {
        assert!(parse_fragment(source, "template", true).is_err(), "{source}");
    }
    let document = parse_fragment("<svg></svg></br></p>", "template", true).unwrap();
    assert_eq!(children(&document, 0), ["svg", "br", "p"]);
    assert_eq!(document.nodes[document.nodes[0].children[0]].namespace, Namespace::Svg);
    empty_native(&document, element(&document, "p"), "p");
    empty_native(&document, element(&document, "br"), "br");
    for source in ["</p><table><tr></tr></table>", "</br><select><input></select>", "</p><p>A<listing>B"] {
        assert!(parse_fragment(source, "div", false).is_err(), "{source}");
        let fresh = parse_fragment("</p></br>", "div", false).unwrap();
        assert_eq!(children(&fresh, 0), ["p", "br"]);
    }
}

#[test]
fn stray_ends_leave_existing_foreign_current_fallback_unchanged() {
    for source in ["<body><math id=foreign></p></br><mi>kept</mi></math><p id=tail>after",
        "<body><svg id=foreign><g></p></br></g></svg><p id=tail>after"] {
        for document in documents(source) {
            assert_eq!(elements(&document, "p"), [id(&document, "tail")]);
            assert!(elements(&document, "br").is_empty());
            assert_eq!(document.nodes[id(&document, "foreign")].namespace, Namespace::UnknownForeign);
            assert_eq!(children(&document, id(&document, "tail")), ["#after"]);
        }
    }
}

#[test]
fn stray_ends_charge_real_node_capacity_and_preserve_ordinary_depth_guards() {
    let limited = |source: &str, nodes| parse_internal(source, Some("div"), true,
        Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    assert_eq!(limited("</p></br>", 3).unwrap().nodes.len(), 3);
    for limit in [1, 2] { assert!(limited("</p></br>", limit).unwrap_err().contains("node/depth limit")); }
    assert_eq!(parse_inert_html("<body></p></br>", 6, &|| Ok(())).unwrap().0.nodes.len(), 6);
    for limit in [4, 5] { assert!(parse_inert_html("<body></p></br>", limit, &|| Ok(())).unwrap_err().contains("node limit")); }
    let full = "</p>".repeat(MAX_NODES - 1);
    assert_eq!(parse_fragment(&full, "div", true).unwrap().nodes.len(), MAX_NODES);
    assert!(parse_fragment(&format!("{full}</br>"), "div", true).unwrap_err().contains("node/depth limit"));
    let deep = "<div>".repeat(MAX_DEPTH - 1);
    assert!(parse_fragment(&format!("{deep}</p>"), "div", true).unwrap_err().contains("node/depth limit"));
    let void = parse_fragment(&format!("{deep}</br>"), "div", true).unwrap();
    assert_eq!(void.nodes.len(), MAX_DEPTH + 1);
    empty_native(&void, void.nodes.len() - 1, "br");
}

#[test]
fn stray_ends_cooperate_with_cancellation_without_publishing_a_prefix() {
    let source = "</p></br>".repeat(2048);
    let calls = std::cell::Cell::new(0);
    let check = || { calls.set(calls.get() + 1); if calls.get() == 64 {
        Err("original stray end cancellation".into()) } else { Ok(()) } };
    let error = parse_internal(&source, Some("div"), true,
        Some(LoadedTree { nodes: 5000, check_cancel: &check }), None).unwrap_err();
    assert_eq!(error, "original stray end cancellation");
    assert_eq!(calls.get(), 64);
    let fresh = parse_internal(&source, Some("div"), true,
        Some(LoadedTree { nodes: 5000, check_cancel: &|| Ok(()) }), None).unwrap();
    assert_eq!(fresh.nodes.len(), 4097);
    assert_eq!(fresh.nodes[0].children.len(), 4096);
    for pair in fresh.nodes[0].children.chunks_exact(2) {
        empty_native(&fresh, pair[0], "p"); empty_native(&fresh, pair[1], "br");
    }
}

#[test]
fn stray_ends_keep_flat_parse_distinct_from_full_document_dispatch() {
    let flat = parse("</p></br>");
    assert_eq!(children(&flat, 0), ["p", "br"]);
    empty_native(&flat, element(&flat, "p"), "p");
    empty_native(&flat, element(&flat, "br"), "br");
    for document in documents("</p></br>") {
        assert_eq!(children(&document, element(&document, "body")), ["br"]);
        assert!(elements(&document, "p").is_empty());
    }
}

#[test]
fn stray_ends_keep_initial_templates_after_text_leaves_and_exact_head_metadata() {
    for prefix in ["A", "<!--marker-->", "<?probe value?>", "<!doctype html>",
        "<base>", "<basefont>", "<bgsound>", "<link>", "<meta>",
        "<noframes>literal</noframes>", "<script>literal</script>", "<style>literal</style>",
        "<title>literal</title>", "<template id=nested>A</template>"] {
        let source = format!("<div id=host><template id=content>{prefix}</p></br></template></p></br></div>");
        for document in documents(&source) {
            assert_eq!(children(&document, id(&document, "host")), ["template", "p", "br"], "{prefix}");
            assert_eq!(elements(&document, "p").len(), 1);
            assert_eq!(elements(&document, "br").len(), 1);
            empty_native(&document, element(&document, "p"), "p");
            empty_native(&document, element(&document, "br"), "br");
        }
        if prefix.starts_with("<noframes>") {
            assert!(parse_fragment(&source, "template", true).is_err());
        } else {
            let document = parse_fragment(&source, "template", true).unwrap();
            assert_eq!(children(&document, id(&document, "host")), ["template", "p", "br"], "{prefix}");
            assert_eq!(elements(&document, "p").len(), 1);
            assert_eq!(elements(&document, "br").len(), 1);
        }
    }
    for scripting in [false, true] {
        let document = parse_fragment("A<!--marker--></template></p></br>B", "template", scripting).unwrap();
        assert_eq!(children(&document, 0), ["#A", "!marker", "#B"]);
        assert!(elements(&document, "p").is_empty());
        assert!(elements(&document, "br").is_empty());
    }
}

#[test]
fn stray_ends_keep_template_exit_after_void_closed_and_consumed_original_starts() {
    for (prefix, retained) in [("<hr>", Some("hr")), ("<span></span>", Some("span")),
        ("<noscript></noscript>", Some("noscript")), ("<html>", None), ("<head>", None), ("<body>", None)] {
        let source = format!("<div id=host><template id=content>{prefix}</p></br></template></div>");
        let mut expected = retained.into_iter().map(str::to_owned).collect::<Vec<_>>();
        expected.extend(["p".into(), "br".into()]);
        for document in documents(&source) {
            assert_eq!(children(&document, id(&document, "host")), ["template"]);
            assert_eq!(children(&document, id(&document, "content")), expected, "{prefix}");
            empty_native(&document, element(&document, "p"), "p");
            empty_native(&document, element(&document, "br"), "br");
        }
        if retained.is_some() {
            let document = parse_fragment(&source, "template", false).unwrap();
            assert_eq!(children(&document, id(&document, "content")), expected, "{prefix}");
        } else {
            assert!(parse_fragment(&source, "template", false).is_err());
        }
    }
    for prefix in ["<hr>", "<span></span>"] {
        let document = parse_fragment(&format!("{prefix}</template></p></br>"), "template", true).unwrap();
        assert_eq!(children(&document, 0), [if prefix == "<hr>" { "hr" } else { "span" }, "p", "br"]);
    }
}

#[test]
fn stray_ends_restore_each_outer_template_state_after_nested_close_and_formatting() {
    let initial_outer = "<div id=host><template id=outer>A<template id=inner><hr></p></br></template>B</p></br>C</template></div>";
    for document in documents(initial_outer).into_iter().chain([parse_fragment(initial_outer, "template", true).unwrap()]) {
        assert_eq!(children(&document, id(&document, "host")), ["template"]);
        assert_eq!(children(&document, id(&document, "outer")), ["#A", "template", "#BC"]);
        assert_eq!(children(&document, id(&document, "inner")), ["hr", "p", "br"]);
        assert_eq!(elements(&document, "p").len(), 1);
        assert_eq!(elements(&document, "br").len(), 1);
    }
    let exited_outer = "<div id=host><template id=outer><hr>A<template id=inner>D</p></br>E</template>B</p></br>C</template></div>";
    for document in documents(exited_outer).into_iter().chain([parse_fragment(exited_outer, "template", false).unwrap()]) {
        assert_eq!(children(&document, id(&document, "host")), ["template"]);
        assert_eq!(children(&document, id(&document, "outer")), ["hr", "#A", "template", "#B", "p", "br", "#C"]);
        assert_eq!(children(&document, id(&document, "inner")), ["#DE"]);
        assert_eq!(elements(&document, "p").len(), 1);
        assert_eq!(elements(&document, "br").len(), 1);
    }
    let document = parse_fragment("<template><p><b>A</p></template></p></br>", "template", true).unwrap();
    assert_eq!(children(&document, 0), ["template"]);
    assert_eq!(children(&document, element(&document, "template")), ["p"]);
    assert_eq!(children(&document, element(&document, "p")), ["b"]);
    assert_eq!(children(&document, element(&document, "b")), ["#A"]);
    assert_eq!(elements(&document, "b").len(), 1);
    assert!(elements(&document, "br").is_empty());
}

#[test]
fn stray_ends_bound_template_state_by_open_depth_and_charge_only_real_nodes() {
    let source = format!("<div id=host>{}</div>", "<template></p></br></template>".repeat(300));
    for document in documents(&source) {
        assert_eq!(document.nodes.len(), 305);
        assert_eq!(document.nodes[id(&document, "host")].children.len(), 300);
        assert!(elements(&document, "p").is_empty());
        assert!(elements(&document, "br").is_empty());
    }
    let fragment = parse_fragment(&source, "template", true).unwrap();
    assert_eq!(fragment.nodes.len(), 302);
    assert_eq!(fragment.nodes[id(&fragment, "host")].children.len(), 300);
    let exact = "<div><template></p></br></template></div>";
    let limited = |nodes| parse_internal(exact, Some("template"), true,
        Some(LoadedTree { nodes, check_cancel: &|| Ok(()) }), None);
    assert_eq!(limited(3).unwrap().nodes.len(), 3);
    assert!(limited(2).unwrap_err().contains("node/depth limit"));
    assert!(parse_fragment(&"<template>".repeat(MAX_DEPTH), "template", true).unwrap_err().contains("node/depth limit"));
}
