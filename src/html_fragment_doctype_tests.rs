//! Original fragment DOCTYPE cases over the shared native parser.
//! https://html.spec.whatwg.org/multipage/parsing.html#html-fragment-parsing-algorithm
use super::*;

fn text(document: &Document) -> String {
    document.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(value) => Some(value.as_str()), _ => None,
    }).collect()
}

fn name(document: &Document, node: usize) -> &str {
    match &document.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag, _ => "",
    }
}

fn no_doctype(document: &Document, context: &str, scripting: bool) {
    assert_eq!(document.kind, DocumentKind::Html);
    assert_eq!(document.mode, DocumentMode::NoQuirks);
    assert_eq!(document.scripting_enabled, scripting);
    assert_eq!(name(document, 0), context);
    assert!(document.nodes.iter().all(|node| !matches!(node.kind, NodeKind::DocumentType(_))));
}

#[test]
fn fragment_doctype_tokens_are_ignored_in_supported_contexts_without_document_state() {
    for context in ["div", "body", "span", "template", "select", "optgroup", "option"] {
        for declaration in ["<!doctype html>", "<!DoCtYpE other>",
            "<!DOCTYPE html PUBLIC '-//W3C//DTD XHTML 1.0 Transitional//EN'>",
            "<!doctype html SYSTEM 'about:legacy-compat'>"] {
            for scripting in [false, true] {
                let document = parse_fragment(&format!("{declaration}tail"), context, scripting).unwrap();
                no_doctype(&document, context, scripting);
                assert_eq!(document.nodes.len(), 2);
                assert_eq!(document.nodes[0].children, [1]);
                assert_eq!(text(&document), "tail");
            }
        }
    }
}

#[test]
fn fragment_doctype_positions_keep_real_element_comment_and_text_order() {
    let document = parse_fragment("before<!doctype html><span id=keep>x</span><!doctype html PUBLIC 'p' 's'>after<!doctype other><!--tail-->", "div", true).unwrap();
    no_doctype(&document, "div", true);
    let children = &document.nodes[0].children;
    assert_eq!(children.len(), 4);
    assert!(matches!(&document.nodes[children[0]].kind, NodeKind::Text(value) if value == "before"));
    assert_eq!(name(&document, children[1]), "span");
    assert_eq!(document.attr(children[1], "id"), Some("keep"));
    assert!(matches!(&document.nodes[children[2]].kind, NodeKind::Text(value) if value == "after"));
    assert!(matches!(&document.nodes[children[3]].kind, NodeKind::Comment(value) if value == "tail"));
    assert_eq!(text(&document), "beforexafter");
}

#[test]
fn fragment_doctype_malformed_tokens_use_real_boundaries_and_eof() {
    for source in ["<!doctype>tail", "<!DOCTYPEhtml>tail", "<!DOCTYPE雪>tail",
        "<!doctype html PUBLIC>tail", "<!doctype html PUBLIC 'unfinished>tail",
        "<!doctype html SYSTEM 'unfinished>tail", "<!doctype html bogus <b>tail",
        "<!doctype html SYSTEM '' junk <div>tail"] {
        let document = parse_fragment(source, "div", false).unwrap();
        no_doctype(&document, "div", false);
        assert_eq!(document.nodes.len(), 2, "{source}");
        assert_eq!(text(&document), "tail", "{source}");
    }
    for source in ["<!doctype", "<!doctype html", "<!doctype html PUBLIC",
        "<!doctype html PUBLIC 'unfinished", "<!doctype html SYSTEM 'unfinished"] {
        let document = parse_fragment(source, "div", false).unwrap();
        no_doctype(&document, "div", false);
        assert_eq!(document.nodes.len(), 1, "{source}");
        assert!(document.nodes[0].children.is_empty(), "{source}");
    }
}

#[test]
fn fragment_doctype_text_contexts_keep_declarations_literal() {
    let source = "<!DOCTYPE html><b>&amp;</b>";
    for context in ["title", "textarea", "script", "style", "xmp", "iframe", "noembed", "noframes", "plaintext"] {
        let document = parse_fragment(source, context, true).unwrap();
        no_doctype(&document, context, true);
        assert_eq!(document.nodes.len(), 2);
        assert_eq!(text(&document), if matches!(context, "title" | "textarea") { "<!DOCTYPE html><b>&</b>" } else { source });
    }
}

#[test]
fn fragment_doctype_does_not_close_formatting_or_reset_checked_svg_namespace() {
    let document = parse_fragment("<b>one<!doctype other>two</b><i>three</i>", "div", true).unwrap();
    no_doctype(&document, "div", true);
    let children = &document.nodes[0].children;
    assert_eq!(children.len(), 2);
    assert_eq!(name(&document, children[0]), "b");
    assert_eq!(name(&document, children[1]), "i");
    assert_eq!(document.nodes[children[0]].children.len(), 1);
    assert!(matches!(&document.nodes[document.nodes[children[0]].children[0]].kind, NodeKind::Text(value) if value == "onetwo"));
    let svg = parse_fragment("<svg><!--marker--><g><!doctype html><rect width='2' height='3'/></g></svg>", "template", true).unwrap();
    no_doctype(&svg, "template", true);
    for node in &svg.nodes[1..] {
        assert_eq!(node.namespace, if matches!(node.kind, NodeKind::Element { .. }) { Namespace::Svg } else { Namespace::None });
    }
    assert_eq!(svg.nodes.iter().filter(|node| matches!(node.kind, NodeKind::Element { .. })).count(), 4);
}

#[test]
fn fragment_doctype_does_not_change_select_option_or_group_ownership() {
    let document = parse_fragment("<select><!doctype other><option>A<!doctype x><option>B<optgroup label=G><!doctype html><option>C</select><p>after</p>", "div", false).unwrap();
    no_doctype(&document, "div", false);
    let roots = &document.nodes[0].children;
    assert_eq!(roots.len(), 2);
    assert_eq!(name(&document, roots[0]), "select");
    assert_eq!(name(&document, roots[1]), "p");
    let options = &document.nodes[roots[0]].children;
    assert_eq!(options.len(), 3);
    assert_eq!(name(&document, options[0]), "option");
    assert_eq!(name(&document, options[1]), "option");
    assert_eq!(name(&document, options[2]), "optgroup");
    assert_eq!(document.nodes[options[2]].children.len(), 1);
    assert_eq!(text(&document), "ABCafter");
}

#[test]
fn fragment_doctype_ignored_tokens_consume_no_node_slots_and_keep_real_limits() {
    let source = format!("{}<span></span>", "<!doctype other>".repeat(24));
    let document = parse_internal(&source, Some("div"), false,
        Some(LoadedTree { nodes: 2, check_cancel: &|| Ok(()) }), None).unwrap();
    no_doctype(&document, "div", false);
    assert_eq!(document.nodes.len(), 2);
    assert!(parse_internal(&format!("{source}<!doctype html><b></b>"), Some("div"), false,
        Some(LoadedTree { nodes: 2, check_cancel: &|| Ok(()) }), None).unwrap_err().contains("node/depth limit"));
    let wide = format!("<!doctype html>{}", "<br>".repeat(MAX_NODES));
    assert!(parse_fragment(&wide, "div", true).unwrap_err().contains("node/depth limit"));
    let deep = format!("<!doctype html>{}", "<div>".repeat(MAX_DEPTH));
    assert!(parse_fragment(&deep, "div", true).unwrap_err().contains("node/depth limit"));
}

#[test]
fn fragment_doctype_shared_scan_cancels_without_returning_a_prefix_and_recovers() {
    let source = format!("<!doctype html SYSTEM '{}'>tail", "x".repeat(32_768));
    let calls = std::cell::Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 4 { Err("original fragment doctype cancellation".into()) } else { Ok(()) }
    };
    let error = parse_internal(&source, Some("div"), true,
        Some(LoadedTree { nodes: 2, check_cancel: &check }), None).unwrap_err();
    assert_eq!(error, "original fragment doctype cancellation");
    assert_eq!(calls.get(), 4);
    let document = parse_internal(&source, Some("div"), true,
        Some(LoadedTree { nodes: 2, check_cancel: &|| Ok(()) }), None).unwrap();
    no_doctype(&document, "div", true);
    assert_eq!(text(&document), "tail");
}

#[test]
fn fragment_doctype_full_document_initial_and_late_tokens_keep_mode_and_identity() {
    for (declaration, mode) in [
        ("<!doctype html>", DocumentMode::NoQuirks),
        ("<!doctype other>", DocumentMode::Quirks),
        ("<!doctype html PUBLIC '-//W3C//DTD XHTML 1.0 Transitional//EN'>", DocumentMode::LimitedQuirks),
    ] {
        let source = format!("{declaration}<p>kept</p><!doctype ignored>");
        let loaded = parse_loaded_document(&source, &|| Ok(())).unwrap();
        let (inert, identity) = parse_inert_html(&source, 30, &|| Ok(())).unwrap();
        assert_eq!(identity, Some(inert.nodes[0].children[0]));
        for document in [parse(&source), loaded, inert] {
            assert_eq!(document.mode, mode);
            assert_eq!(document.nodes.iter().filter(|node| matches!(node.kind, NodeKind::DocumentType(_))).count(), 1);
            assert!(matches!(document.nodes[document.nodes[0].children[0]].kind, NodeKind::DocumentType(_)));
        }
    }
    let loaded = parse_loaded_document("<p>kept</p><!doctype html>", &|| Ok(())).unwrap();
    assert_eq!(loaded.mode, DocumentMode::Quirks);
    assert!(loaded.nodes.iter().all(|node| !matches!(node.kind, NodeKind::DocumentType(_))));
}

#[test]
fn fragment_doctype_does_not_hide_later_unsupported_tree_or_null_input() {
    for (source, message) in [
        ("<!doctype html><svg></svg>", "foreign-content"),
        ("<!doctype html><table><tr></tr></table>", "immediately matching"),
        ("<!doctype html><textarea/>", "ordinary start tag"),
        ("<!doctype html><select><input></select>", "nonconventional select"),
    ] {
        assert!(parse_fragment(source, "div", true).unwrap_err().contains(message), "{source}");
    }
    for context in ["table", "tr", "svg"] {
        assert!(parse_fragment("<!doctype html>", context, true).unwrap_err().contains("element context"));
    }
    assert!(parse_fragment("<!doctype h\0tml><span>tail</span>", "div", true).unwrap_err().contains("NULL handling"));
    assert!(parse_fragment("<svg><!doctype html><![CDATA[x]]></svg>", "template", true).unwrap_err().contains("declaration/comment"));
}

#[test]
fn fragment_doctype_near_keywords_and_comment_data_keep_their_own_tokens() {
    for (source, expected) in [("<!doctyp>", "doctyp"), ("<! docTYPE html>", " docTYPE html"),
        ("<!-- <!doctype html> -->", " <!doctype html> ")] {
        let document = parse_fragment(source, "div", true).unwrap();
        no_doctype(&document, "div", true);
        assert_eq!(document.nodes.len(), 2);
        assert!(matches!(&document.nodes[1].kind, NodeKind::Comment(value) if value == expected));
    }
}
