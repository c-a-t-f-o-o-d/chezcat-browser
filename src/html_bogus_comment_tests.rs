//! Original parser witnesses for malformed declarations and end-tag opens.
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [
        parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100_000, &|| Ok(())).unwrap().0,
    ]
}

fn all_entries(source: &str) -> [Document; 4] {
    let [loaded, inert] = documents(source);
    [parse(source), loaded, inert, parse_fragment(source, "div", true).unwrap()]
}

fn comments(document: &Document) -> Vec<&str> {
    document.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Comment(value) => Some(value.as_str()),
        _ => None,
    }).collect()
}

fn text(document: &Document) -> String {
    document.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(value) => Some(value.as_str()),
        _ => None,
    }).collect()
}

fn element(document: &Document, name: &str) -> usize {
    let found: Vec<_> = document.nodes.iter().enumerate().filter_map(|(index, node)| {
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == name).then_some(index)
    }).collect();
    assert_eq!(found.len(), 1, "expected one {name} element");
    found[0]
}

fn shape(node: &Node) -> String {
    match &node.kind {
        NodeKind::Element { tag, .. } => tag.clone(),
        NodeKind::Comment(value) => format!("comment {value}"),
        NodeKind::Text(value) => format!("text {value}"),
        NodeKind::ProcessingInstruction { target, data } => format!("PI {target}:{data}"),
        NodeKind::DocumentType(value) => format!("doctype {}", value.name),
        other => panic!("unexpected node {other:?}"),
    }
}

fn children(document: &Document, parent: usize) -> Vec<String> {
    document.nodes[parent].children.iter().map(|&child| shape(&document.nodes[child])).collect()
}

#[test]
fn bogus_comment_declarations_retain_literal_data_through_first_greater_than() {
    for (source, expected) in [
        ("<!bad>tail", "bad"),
        ("<!>tail", ""),
        ("<!!>tail", "!"),
        ("<!-x>tail", "-x"),
        ("<!foo--!>tail", "foo--!"),
        ("<!foo 'quoted>tail", "foo 'quoted"),
        ("<!a<em id=hidden>tail", "a<em id=hidden"),
        ("<![cdata[x]]>tail", "[cdata[x]]"),
        ("<!dOcTyp>tail", "dOcTyp"),
        ("<!snow 雪🙂 &amp; &#65;>tail", "snow 雪🙂 &amp; &#65;"),
    ] {
        for document in all_entries(source) {
            assert_eq!(comments(&document), [expected], "{source:?}");
            assert_eq!(text(&document), "tail", "{source:?}");
            assert!(!document.nodes.iter().any(|node| matches!(&node.kind,
                NodeKind::Element { tag, .. } if tag == "em")), "{source:?}");
        }
    }
}

#[test]
fn bogus_comment_declaration_eof_keeps_pending_punctuation() {
    for (source, expected) in [
        ("<!", ""), ("<!-", "-"), ("<!x", "x"),
        ("<!x--", "x--"), ("<!x--!", "x--!"),
        ("<!雪 &amp; <b", "雪 &amp; <b"),
    ] {
        for document in all_entries(source) {
            assert_eq!(comments(&document), [expected], "{source:?}");
            assert!(text(&document).is_empty(), "{source:?}");
        }
    }
}

#[test]
fn bogus_comment_invalid_end_tag_opens_keep_the_first_invalid_character() {
    for (source, expected) in [
        ("</1name>tail", "1name"),
        ("</ name>tail", " name"),
        ("</\t!?>tail", "\t!?"),
        ("</\u{c}end>tail", "\u{c}end"),
        ("</é>tail", "é"),
        ("<//>tail", "/"),
        ("</=quoted '>tail", "=quoted '"),
        ("</<b id=hidden>tail", "<b id=hidden"),
    ] {
        for document in all_entries(source) {
            assert_eq!(comments(&document), [expected], "{source:?}");
            assert_eq!(text(&document), "tail", "{source:?}");
            assert!(!document.nodes.iter().any(|node| matches!(&node.kind,
                NodeKind::Element { tag, .. } if tag == "b")), "{source:?}");
        }
    }
}

#[test]
fn bogus_comment_invalid_end_tag_eof_emits_data_but_empty_close_emits_nothing() {
    for (source, expected) in [("</1", "1"), ("</ ", " "), ("<//--!", "/--!")] {
        for document in all_entries(source) {
            assert_eq!(comments(&document), [expected], "{source:?}");
            assert!(text(&document).is_empty());
        }
    }
    for document in all_entries("<p>a</>b</p>") {
        assert!(comments(&document).is_empty());
        assert_eq!(children(&document, element(&document, "p")), ["text ab"]);
    }
}

#[test]
fn bogus_comment_end_tag_open_at_eof_remains_literal_text() {
    for document in all_entries("<p>before</") {
        assert!(comments(&document).is_empty());
        assert_eq!(children(&document, element(&document, "p")), ["text before</"]);
    }
    for document in all_entries("</") {
        assert!(comments(&document).is_empty());
        assert_eq!(text(&document), "</");
    }
}

#[test]
fn bogus_comment_invalid_start_tag_controls_remain_character_data() {
    for (source, expected) in [
        ("<", "<"), ("<1>", "<1>"), ("<#>&amp;", "<#>&"),
        ("< 雪>", "< 雪>"), ("<é>", "<é>"),
    ] {
        for document in all_entries(&format!("<p>{source}")) {
            assert!(comments(&document).is_empty(), "{source:?}");
            assert_eq!(text(&document), expected, "{source:?}");
        }
    }
}

#[test]
fn bogus_comment_loaded_and_inert_preprocessing_replaces_nulls_without_decoding_references() {
    for (source, expected) in [
        ("<!a\r\nb\rc\0&amp; &#65; <b>tail", "a\nb\nc\u{fffd}&amp; &#65; <b"),
        ("</\0x\r\ny &lt;>tail", "\u{fffd}x\ny &lt;"),
        ("<![CDATA[\0&NewLine;\r\n]]>tail", "[CDATA[\u{fffd}&NewLine;\n]]"),
    ] {
        for document in documents(source) {
            assert_eq!(comments(&document), [expected], "{source:?}");
            assert_eq!(text(&document), "tail");
        }
    }
}

#[test]
fn bogus_comment_checked_null_and_later_unsupported_markup_guards_remain() {
    for source in ["<!bad\0>", "</\0bad>", "<![CDATA[\0]]>", "<!--\0-->"] {
        assert!(parse_fragment(source, "div", true).unwrap_err()
            .contains("NULL handling outside text-only contexts"), "{source:?}");
    }
    for source in [
        "<!marker><table><tr><td>x</td></tr></table>",
        "<!marker><svg><rect/></svg>",
        "<!marker><plaintext>rest",
    ] {
        assert!(parse_fragment(source, "div", true).is_err(), "{source:?}");
    }
    assert_eq!(comments(&parse_fragment("<!recovered>", "div", true).unwrap()), ["recovered"]);
}

#[test]
fn bogus_comment_html_cdata_open_is_a_comment_and_does_not_create_cdata_nodes() {
    for (source, expected) in [
        ("<![CDATA[x<em&copy;]]>tail", "[CDATA[x<em&copy;]]"),
        ("<![CDATA[quoted '>tail", "[CDATA[quoted '"),
        ("<![CDATA[>tail", "[CDATA["),
    ] {
        for document in all_entries(source) {
            assert_eq!(comments(&document), [expected], "{source:?}");
            assert_eq!(text(&document), "tail");
            assert!(!document.nodes.iter().any(|node| matches!(node.kind, NodeKind::Cdata(_))));
        }
    }
    for document in all_entries("<![CDATA[unfinished]]") {
        assert_eq!(comments(&document), ["[CDATA[unfinished]]"]);
        assert!(text(&document).is_empty());
    }
}

#[test]
fn bogus_comment_document_positions_and_initial_doctype_mode_are_preserved() {
    let source = "<!before><!doctype html><html><!root><head><!head></head><body><div><!inner></div></body><!afterbody></html><!afterhtml>";
    for document in documents(source) {
        assert_eq!(document.mode, DocumentMode::NoQuirks);
        assert_eq!(children(&document, 0), ["comment before", "doctype html", "html", "comment afterhtml"]);
        assert_eq!(children(&document, element(&document, "html")), ["comment root", "head", "body", "comment afterbody"]);
        assert_eq!(children(&document, element(&document, "head")), ["comment head"]);
        assert_eq!(children(&document, element(&document, "div")), ["comment inner"]);
        assert!(document.nodes.iter().filter(|node| matches!(node.kind, NodeKind::Comment(_)))
            .all(|node| node.namespace == Namespace::None && node.children.is_empty()));
    }
}

#[test]
fn bogus_comment_table_comments_stay_in_table_while_pending_text_is_fostered() {
    let source = "<table>before<!marker><tr><td>inside<!cell></td></tr>after</table><p>tail</p>";
    for document in documents(source) {
        let table = element(&document, "table");
        assert_eq!(children(&document, table), ["comment marker", "tbody"]);
        assert_eq!(children(&document, element(&document, "td")), ["text inside", "comment cell"]);
        let body = element(&document, "body");
        let table_slot = document.nodes[body].children.iter().position(|&node| node == table).unwrap();
        let before: String = document.nodes[body].children[..table_slot].iter().map(|&node| {
            match &document.nodes[node].kind { NodeKind::Text(value) => value.as_str(), _ => panic!("expected fostered text") }
        }).collect();
        assert_eq!(before, "beforeafter");
        assert_eq!(children(&document, element(&document, "p")), ["text tail"]);
    }
}

#[test]
fn bogus_comment_does_not_reconstruct_active_formatting_until_following_text() {
    for document in documents("<p><b>one</p><!marker>two") {
        let body = element(&document, "body");
        assert_eq!(children(&document, body), ["p", "comment marker", "b"]);
        let reconstructed = document.nodes[body].children[2];
        assert_eq!(children(&document, reconstructed), ["text two"]);
    }
    let document = parse_fragment("<p><b>one</p><!marker>two", "div", true).unwrap();
    assert_eq!(children(&document, 0), ["p", "comment marker", "b"]);
}

#[test]
fn bogus_comment_checked_selects_keep_comment_and_option_order() {
    let source = "<select><!before><option>one</1>two</option><!after></select>";
    let [loaded, inert] = documents(source);
    for document in [parse_fragment(source, "div", true).unwrap(), loaded, inert] {
        assert_eq!(children(&document, element(&document, "select")), ["comment before", "option", "comment after"]);
        assert_eq!(children(&document, element(&document, "option")), ["text one", "comment 1", "text two"]);
    }
    let document = parse_fragment("<!marker><option>one</option>", "select", true).unwrap();
    assert_eq!(children(&document, 0), ["comment marker", "option"]);
}

#[test]
fn bogus_comment_balanced_svg_islands_preserve_element_and_leaf_namespaces() {
    let source = "<svg><!before><g><rect width='2' height='3'/></1></g><!after></svg><p>tail</p>";
    let [loaded, inert] = documents(source);
    for document in [parse(source), loaded, inert, parse_fragment(source, "template", true).unwrap()] {
        assert_eq!(comments(&document), ["before", "1", "after"]);
        for tag in ["svg", "g", "rect"] {
            assert_eq!(document.nodes[element(&document, tag)].namespace, Namespace::Svg, "{tag}");
        }
        assert_eq!(document.nodes[element(&document, "p")].namespace, Namespace::Html);
        assert!(document.nodes.iter().filter(|node| matches!(node.kind, NodeKind::Comment(_)))
            .all(|node| node.namespace == Namespace::None));
    }
}

#[test]
fn bogus_comment_foreign_cdata_retains_existing_unimplemented_boundaries() {
    for tag in ["svg", "math"] {
        let source = format!("<{tag}><![CDATA[x]]></{tag}><p>tail</p>");
        for document in [parse(&source), parse_loaded_document(&source, &|| Ok(())).unwrap()] {
            assert_eq!(document.nodes[element(&document, tag)].namespace, Namespace::UnknownForeign);
            assert!(comments(&document).is_empty());
            assert!(!document.nodes.iter().any(|node| matches!(node.kind, NodeKind::Cdata(_))));
            assert_eq!(children(&document, element(&document, "p")), ["text tail"]);
        }
        assert!(parse_inert_html(&source, 100, &|| Ok(())).is_err());
    }
    assert!(parse_fragment("<svg><![CDATA[x]]></svg>", "template", true).is_err());
    assert_eq!(comments(&parse_fragment("<![CDATA[recovered]]>", "div", true).unwrap()), ["[CDATA[recovered]]"]);
}

#[test]
fn bogus_comment_modern_processing_instructions_remain_distinct() {
    let source = "<?target data?><!new><?xml-stylesheet href=ignored?><p>tail</p>";
    for document in all_entries(source) {
        assert_eq!(comments(&document), ["new", "?xml-stylesheet href=ignored?"]);
        let instructions: Vec<_> = document.nodes.iter().filter_map(|node| match &node.kind {
            NodeKind::ProcessingInstruction { target, data } => Some((target.as_str(), data.as_str())),
            _ => None,
        }).collect();
        assert_eq!(instructions, [("target", "data")]);
        assert_eq!(text(&document), "tail");
    }
}

#[test]
fn bogus_comment_text_only_contexts_keep_declarations_and_end_opens_literal() {
    let source = "<!raw></1><![CDATA[x]]><?target?>";
    for context in ["title", "textarea", "script", "style", "xmp", "iframe", "noembed", "noframes", "plaintext"] {
        let document = parse_fragment(source, context, true).unwrap();
        assert!(comments(&document).is_empty(), "{context}");
        assert_eq!(text(&document), source, "{context}");
        assert_eq!(document.nodes.len(), 2);
    }
}

#[test]
fn bogus_comment_ordinary_comment_transition_controls_keep_literal_pending_data() {
    for (source, expected) in [
        ("<!--a<!-->", "a<!"),
        ("<!--a<!--!>", "a<!"),
        ("<!--a<!--", "a<!"),
        ("<!--a--!-", "a--!"),
        ("<!--a--!x", "a--!x"),
        ("<!--a--!->b-->", "a--!->b"),
        ("<!--<<!x-->", "<<!x"),
    ] {
        for document in all_entries(source) {
            assert_eq!(comments(&document), [expected], "{source:?}");
            assert!(text(&document).is_empty());
        }
    }
}

#[test]
fn bogus_comment_node_admission_refuses_without_partial_document_and_recovers() {
    for source in ["<!one>", "</1>", "<![CDATA[x]]>"] {
        assert!(parse_inert_html(source, 4, &|| Ok(())).unwrap_err().contains("node limit"));
        let (document, doctype) = parse_inert_html(source, 5, &|| Ok(())).unwrap();
        assert_eq!(document.nodes.len(), 5);
        assert_eq!(comments(&document).len(), 1);
        assert_eq!(doctype, None);
    }
    assert!(parse_inert_html("<!one><!two>", 5, &|| Ok(())).unwrap_err().contains("node limit"));
    let document = parse_inert_html("<!one><!two>", 6, &|| Ok(())).unwrap().0;
    assert_eq!(comments(&document), ["one", "two"]);
}

#[test]
fn bogus_comment_checked_fragment_node_limit_is_not_bypassed() {
    let source = "<!x>".repeat(MAX_NODES);
    assert!(parse_fragment(&source, "div", true).unwrap_err().contains("node limit"));
    let document = parse_fragment("<!fresh><p>ok</p>", "div", true).unwrap();
    assert_eq!(comments(&document), ["fresh"]);
    assert_eq!(text(&document), "ok");
}

#[test]
fn bogus_comment_long_ordinary_scans_cooperate_with_cancellation() {
    for suffix in ["-->", "--!>", ""] {
        let source = format!("<!--{}{suffix}", "x".repeat(512 * 1024));
        let calls = std::cell::Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 12 { Err("cancel long ordinary comment".into()) } else { Ok(()) }
        };
        assert!(parse_loaded_document(&source, &check).unwrap_err().contains("cancel long ordinary comment"));
        assert_eq!(calls.get(), 12);
    }
    assert_eq!(comments(&parse_loaded_document("<!--fresh-->", &|| Ok(())).unwrap()), ["fresh"]);
}

#[test]
fn bogus_comment_long_declaration_and_invalid_end_scans_cooperate_with_cancellation() {
    for (opening, suffix) in [("<!", ">"), ("</1", ">"), ("<![CDATA[", ""), ("<!", "")] {
        let source = format!("{opening}{}{suffix}", "x".repeat(512 * 1024));
        let calls = std::cell::Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 12 { Err("cancel long bogus comment".into()) } else { Ok(()) }
        };
        assert!(parse_loaded_document(&source, &check).unwrap_err().contains("cancel long bogus comment"));
        assert_eq!(calls.get(), 12);
    }
    assert_eq!(comments(&parse_inert_html("<!fresh>", 5, &|| Ok(())).unwrap().0), ["fresh"]);
}

#[test]
fn bogus_comment_large_unicode_and_null_payloads_keep_exact_data() {
    let source_data = format!("{}\0{}&amp;\0{}", "雪".repeat(6000), "🙂".repeat(6000), "é".repeat(6000));
    let expected = source_data.replace('\0', "\u{fffd}");
    for (open, close) in [("<!", ">"), ("<!--", "-->")] {
        let source = format!("{open}{source_data}{close}");
        for document in documents(&source) {
            assert_eq!(comments(&document), [expected.as_str()]);
            assert!(text(&document).is_empty());
        }
    }
}

#[test]
fn bogus_comment_ignored_empty_end_tag_preserves_pre_initial_newline_rule() {
    for source in ["<pre></>\nx</pre>", "<pre></></>\r\nx</pre>"] {
        let document = parse_fragment(source, "div", true).unwrap();
        assert_eq!(children(&document, element(&document, "pre")), ["text x"]);
        assert!(comments(&document).is_empty());
    }
    let document = parse_fragment("<pre><!marker>\nx</pre>", "div", true).unwrap();
    assert_eq!(children(&document, element(&document, "pre")), ["comment marker", "text \nx"]);
}
