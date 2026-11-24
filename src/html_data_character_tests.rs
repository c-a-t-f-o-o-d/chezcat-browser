//! Original full-document character-dispatch and retained-tree witnesses.
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [
        parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100_000, &|| Ok(())).unwrap().0,
    ]
}

fn elements(document: &Document, name: &str) -> Vec<usize> {
    document.nodes.iter().enumerate().filter_map(|(index, node)| {
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == name).then_some(index)
    }).collect()
}

fn element(document: &Document, name: &str) -> usize {
    let found = elements(document, name);
    assert_eq!(found.len(), 1, "expected one {name} element");
    found[0]
}

fn children(document: &Document, parent: usize) -> Vec<String> {
    document.nodes[parent].children.iter().map(|&child| {
        match &document.nodes[child].kind {
            NodeKind::Element { tag, .. } => tag.clone(),
            NodeKind::Text(value) => format!("text {value}"),
            NodeKind::Comment(value) => format!("comment {value}"),
            NodeKind::DocumentType(value) => format!("doctype {}", value.name),
            other => panic!("unexpected retained node {other:?}"),
        }
    }).collect()
}

fn no_null_text(document: &Document) {
    assert!(document.nodes.iter().all(|node| match &node.kind {
        NodeKind::Text(value) => !value.as_str().contains('\0'),
        _ => true,
    }));
}

#[test]
fn data_null_empty_body_runs_do_not_allocate_text() {
    for source in ["\0", "\0\0", "<body>\0"] {
        for document in documents(source) {
            assert_eq!(children(&document, 0), ["html"]);
            assert_eq!(children(&document, element(&document, "html")), ["head", "body"]);
            assert!(document.nodes[element(&document, "head")].children.is_empty());
            assert!(document.nodes[element(&document, "body")].children.is_empty());
            assert_eq!(document.nodes.len(), 4, "{source:?}");
        }
    }
}

#[test]
fn data_null_body_runs_coalesce_without_empty_siblings() {
    for source in ["<body>A\0B\0", "<body>\0A\0\0B\0", "<body>A\0</>\0B\0"] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "body")), ["text AB"]);
            assert_eq!(document.nodes.len(), 5, "{source:?}");
            no_null_text(&document);
        }
    }
}

#[test]
fn data_null_bogus_comment_tail_keeps_exact_native_children() {
    for document in documents("FOO<!doc>\0") {
        assert_eq!(children(&document, element(&document, "body")), ["text FOO", "comment doc"]);
        assert_eq!(document.nodes.len(), 6);
        no_null_text(&document);
    }
}

#[test]
fn data_null_numeric_references_keep_their_replacement_characters() {
    for source in ["<p>A\0&#0;\0B", "<p>A\0&#x0;\0B"] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "p")), ["text A\u{fffd}B"]);
            no_null_text(&document);
        }
    }
}

#[test]
fn data_null_does_not_retokenize_references_or_markup() {
    for (source, expected) in [
        ("<body>&am\0p;|&#x\0;|&\0amp;", "text &amp;|&#x;|&amp;"),
        ("<body><\0script>alert(1)</script>", "text <script>alert(1)"),
        ("<body><\0b>safe</b>", "text <b>safe"),
    ] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "body")), [expected], "{source:?}");
            assert!(elements(&document, "script").is_empty());
            assert!(elements(&document, "b").is_empty());
            assert_eq!(document.nodes.len(), 5);
            no_null_text(&document);
        }
    }
}

#[test]
fn data_null_initial_token_ends_the_doctype_opportunity() {
    for source in ["\0<!doctype html><!--marker-->", "\t \0<!doctype html><!--marker-->"] {
        for document in documents(source) {
            assert_eq!(document.mode, DocumentMode::Quirks);
            assert_eq!(children(&document, 0), ["html"]);
            assert_eq!(children(&document, element(&document, "body")), ["comment marker"]);
            assert!(!document.nodes.iter().any(|node| matches!(node.kind, NodeKind::DocumentType(_))));
            assert_eq!(document.nodes.len(), 5);
        }
    }
    for document in documents("\t <!doctype html><!--marker-->") {
        assert_eq!(document.mode, DocumentMode::NoQuirks);
        assert_eq!(children(&document, 0), ["doctype html", "comment marker", "html"]);
        assert!(document.nodes[element(&document, "body")].children.is_empty());
    }
}

#[test]
fn data_null_before_html_and_head_creates_structure_before_comments() {
    for source in [" \0<!--marker-->", "<html> \0<!--marker-->"] {
        for document in documents(source) {
            assert_eq!(children(&document, 0), ["html"]);
            assert_eq!(children(&document, element(&document, "html")), ["head", "body"]);
            assert!(document.nodes[element(&document, "head")].children.is_empty());
            assert_eq!(children(&document, element(&document, "body")), ["comment marker"]);
            assert_eq!(document.nodes.len(), 5);
        }
    }
}

#[test]
fn data_null_in_head_preserves_whitespace_on_each_side() {
    for document in documents("<head> \0\t<!--marker-->") {
        assert_eq!(children(&document, element(&document, "html")), ["head", "body"]);
        assert_eq!(children(&document, element(&document, "head")), ["text  "]);
        assert_eq!(children(&document, element(&document, "body")), ["text \t", "comment marker"]);
        assert_eq!(document.nodes.len(), 7);
        no_null_text(&document);
    }
}

#[test]
fn data_null_after_head_preserves_html_and_body_whitespace() {
    for document in documents("<head></head> \0\t<!--marker-->") {
        assert_eq!(children(&document, element(&document, "html")), ["head", "text  ", "body"]);
        assert!(document.nodes[element(&document, "head")].children.is_empty());
        assert_eq!(children(&document, element(&document, "body")), ["text \t", "comment marker"]);
        assert_eq!(document.nodes.len(), 7);
        no_null_text(&document);
    }
}

#[test]
fn data_null_after_body_and_html_reenters_body_before_comments() {
    for source in [
        "<body><p>x</p></body>\0<!--marker-->",
        "<body><p>x</p></body></html>\0<!--marker-->",
    ] {
        for document in documents(source) {
            assert_eq!(children(&document, 0), ["html"]);
            assert_eq!(children(&document, element(&document, "html")), ["head", "body"]);
            assert_eq!(children(&document, element(&document, "body")), ["p", "comment marker"]);
            assert_eq!(children(&document, element(&document, "p")), ["text x"]);
            assert_eq!(document.nodes.len(), 7);
        }
    }
}

#[test]
fn data_null_only_does_not_reconstruct_active_formatting() {
    for document in documents("<p><b>x</p>\0") {
        assert_eq!(children(&document, element(&document, "body")), ["p"]);
        assert_eq!(children(&document, element(&document, "p")), ["b"]);
        assert_eq!(children(&document, element(&document, "b")), ["text x"]);
        assert_eq!(document.nodes.len(), 7);
    }
}

#[test]
fn data_null_before_comment_defers_formatting_until_later_text() {
    for document in documents("<p><b>x</p>\0<!--marker-->y") {
        let body = element(&document, "body");
        assert_eq!(children(&document, body), ["p", "comment marker", "b"]);
        assert_eq!(elements(&document, "b").len(), 2);
        let original = document.nodes[element(&document, "p")].children[0];
        let reconstructed = document.nodes[body].children[2];
        assert_ne!(original, reconstructed);
        assert_eq!(children(&document, original), ["text x"]);
        assert_eq!(children(&document, reconstructed), ["text y"]);
        assert_eq!(document.nodes.len(), 10);
    }
}

#[test]
fn data_null_table_pending_whitespace_is_not_fostered() {
    for source in ["<table> \0\t<!--marker--></table>", "<table> \0</>\t<!--marker--></table>"] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "body")), ["table"]);
            assert_eq!(children(&document, element(&document, "table")), ["text  \t", "comment marker"]);
            assert_eq!(document.nodes.len(), 7);
            no_null_text(&document);
        }
    }
}

#[test]
fn data_null_only_table_characters_do_not_allocate_or_reconstruct() {
    for document in documents("<table>\0</table>") {
        assert_eq!(children(&document, element(&document, "body")), ["table"]);
        assert!(document.nodes[element(&document, "table")].children.is_empty());
        assert_eq!(document.nodes.len(), 5);
    }
    for document in documents("<p><b>x</p><table>\0<!--marker--></table>") {
        assert_eq!(children(&document, element(&document, "body")), ["p", "table"]);
        assert_eq!(children(&document, element(&document, "table")), ["comment marker"]);
        assert_eq!(children(&document, element(&document, "b")), ["text x"]);
        assert_eq!(document.nodes.len(), 9);
    }
}

#[test]
fn data_null_column_group_reprocesses_before_dropping_the_token() {
    for document in documents("<table><colgroup> \0\t<!--marker--><col></table>") {
        let table = element(&document, "table");
        assert_eq!(children(&document, element(&document, "body")), ["table"]);
        assert_eq!(children(&document, table), ["colgroup", "text \t", "comment marker", "colgroup"]);
        let groups = elements(&document, "colgroup");
        assert_eq!(groups.len(), 2);
        assert_eq!(children(&document, groups[0]), ["text  "]);
        assert_eq!(children(&document, groups[1]), ["col"]);
        no_null_text(&document);
    }
}

#[test]
fn data_null_table_section_row_and_cell_keep_comment_ownership() {
    for document in documents("<table><tbody>\0<!--section--><tr>\0<!--row--><td>\0<!--cell-->x") {
        assert_eq!(children(&document, element(&document, "body")), ["table"]);
        assert_eq!(children(&document, element(&document, "table")), ["tbody"]);
        assert_eq!(children(&document, element(&document, "tbody")), ["comment section", "tr"]);
        assert_eq!(children(&document, element(&document, "tr")), ["comment row", "td"]);
        assert_eq!(children(&document, element(&document, "td")), ["comment cell", "text x"]);
        no_null_text(&document);
    }
}

#[test]
fn data_null_fostered_text_and_cell_text_keep_surrounding_data() {
    for document in documents("<table>A\0B<tr><td>C\0D</table>") {
        assert_eq!(children(&document, element(&document, "body")), ["text AB", "table"]);
        assert_eq!(children(&document, element(&document, "table")), ["tbody"]);
        assert_eq!(children(&document, element(&document, "tbody")), ["tr"]);
        assert_eq!(children(&document, element(&document, "tr")), ["td"]);
        assert_eq!(children(&document, element(&document, "td")), ["text CD"]);
        no_null_text(&document);
    }
}

#[test]
fn data_null_caption_and_cell_text_do_not_close_their_contexts() {
    let source = "<table><caption>A\0B<!--caption--></caption><tr><td>C\0D<!--cell--></td></tr></table>";
    for document in documents(source) {
        assert_eq!(children(&document, element(&document, "table")), ["caption", "tbody"]);
        assert_eq!(children(&document, element(&document, "caption")), ["text AB", "comment caption"]);
        assert_eq!(children(&document, element(&document, "td")), ["text CD", "comment cell"]);
        no_null_text(&document);
    }
}

#[test]
fn data_null_select_and_option_text_remain_in_the_current_parent() {
    for document in documents("<select>\0<!--select--><option>A\0B</option>\0</select>") {
        assert_eq!(children(&document, element(&document, "body")), ["select"]);
        assert_eq!(children(&document, element(&document, "select")), ["comment select", "option"]);
        assert_eq!(children(&document, element(&document, "option")), ["text AB"]);
        no_null_text(&document);
    }
}

#[test]
fn data_null_template_text_remains_in_the_native_template_tree() {
    for document in documents("<template>\0<!--marker--><p>A\0B</p></template><p>tail</p>") {
        let template = element(&document, "template");
        let body = element(&document, "body");
        assert_eq!(children(&document, element(&document, "head")), ["template"]);
        assert_eq!(children(&document, template), ["comment marker", "p"]);
        assert_eq!(children(&document, document.nodes[template].children[1]), ["text AB"]);
        assert_eq!(children(&document, body), ["p"]);
        assert_eq!(children(&document, document.nodes[body].children[0]), ["text tail"]);
        assert_eq!(elements(&document, "p").len(), 2);
        no_null_text(&document);
    }
}

#[test]
fn data_null_first_pre_token_keeps_the_following_line_feed() {
    for document in documents("<pre>\0\nx</pre>") {
        assert_eq!(children(&document, element(&document, "body")), ["pre"]);
        assert_eq!(children(&document, element(&document, "pre")), ["text \nx"]);
        no_null_text(&document);
    }
}

#[test]
fn data_null_raw_text_and_comments_keep_their_replacement_rules() {
    let source = "<title>\0&#0;</title><style>\0&amp;</style><script>\0&amp;</script><body><textarea>\0&#0;</textarea><xmp>\0&amp;</xmp><!--a\0b--><!x\0y><plaintext>\0&amp;";
    for document in documents(source) {
        for name in ["title", "textarea"] {
            assert_eq!(children(&document, element(&document, name)), ["text \u{fffd}\u{fffd}"]);
        }
        for name in ["style", "script", "xmp", "plaintext"] {
            assert_eq!(children(&document, element(&document, name)), ["text \u{fffd}&amp;"]);
        }
        assert_eq!(children(&document, element(&document, "body")),
            ["textarea", "xmp", "comment a\u{fffd}b", "comment x\u{fffd}y", "plaintext"]);
        no_null_text(&document);
    }
}

#[test]
fn data_null_checked_fragments_keep_explicit_admission_guards() {
    for context in ["div", "select", "option", "template"] {
        assert!(parse_fragment("\0", context, true).unwrap_err()
            .contains("NULL handling outside text-only contexts"), "{context}");
    }
    for source in ["<p title='\0'>x", "<\0script>alert(1)</script>", "<!--\0-->", "<!x\0>"] {
        assert!(parse_fragment(source, "div", true).unwrap_err()
            .contains("NULL handling outside text-only contexts"), "{source:?}");
    }
    for context in ["title", "textarea", "script", "style", "plaintext"] {
        let document = parse_fragment("A\0B", context, true).unwrap();
        assert_eq!(children(&document, 0), ["text A\u{fffd}B"], "{context}");
        assert_eq!(document.nodes.len(), 2);
    }
}

#[test]
fn data_null_node_limits_count_retained_nodes_and_recover() {
    let document = parse_inert_html("\0", 4, &|| Ok(())).unwrap().0;
    assert_eq!(document.nodes.len(), 4);
    assert!(document.nodes[element(&document, "body")].children.is_empty());
    assert!(parse_inert_html("\0", 3, &|| Ok(())).unwrap_err().contains("node limit"));
    let document = parse_inert_html("<p><b>x</p>\0", 7, &|| Ok(())).unwrap().0;
    assert_eq!(document.nodes.len(), 7);
    assert_eq!(children(&document, element(&document, "body")), ["p"]);
    assert!(parse_inert_html("A\0B", 4, &|| Ok(())).unwrap_err().contains("node limit"));
    let document = parse_inert_html("A\0B", 5, &|| Ok(())).unwrap().0;
    assert_eq!(document.nodes.len(), 5);
    assert_eq!(children(&document, element(&document, "body")), ["text AB"]);
}

#[test]
fn data_null_large_runs_cancel_and_recover_without_partial_results() {
    let source = format!("<p>{}tail", "\0".repeat(128 * 1024));
    for inert in [false, true] {
        let calls = std::cell::Cell::new(0usize);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() >= 9 { Err("stop long character run".into()) } else { Ok(()) }
        };
        let result = if inert {
            parse_inert_html(&source, 100_000, &check).map(|(document, _)| document)
        } else {
            parse_loaded_document(&source, &check)
        };
        assert_eq!(result.unwrap_err(), "stop long character run");
        assert!(calls.get() > 1);
    }
    for document in documents(&source) {
        assert_eq!(children(&document, element(&document, "p")), ["text tail"]);
        assert_eq!(document.nodes.len(), 6);
        no_null_text(&document);
    }
}
