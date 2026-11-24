//! Original scripting-disabled document head-noscript recovery witnesses.
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
     parse_inert_html(source, 100_000, &|| Ok(())).unwrap().0]
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
    document.nodes[parent].children.iter().map(|&child| match &document.nodes[child].kind {
        NodeKind::Element { tag, .. } => tag.clone(),
        NodeKind::Text(value) => format!("text {value}"),
        NodeKind::Comment(value) => format!("comment {value}"),
        NodeKind::ProcessingInstruction { target, data } => format!("pi {target} {data}"),
        NodeKind::DocumentType(value) => format!("doctype {}", value.name),
        other => panic!("unexpected retained node {other:?}"),
    }).collect()
}

#[test]
fn head_noscript_text_reprocesses_into_body() {
    for source in ["<noscript>fallback", "<html><head><noscript>fallback</noscript>"] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "head")), ["noscript"]);
            assert!(document.nodes[element(&document, "noscript")].children.is_empty());
            assert_eq!(children(&document, element(&document, "body")), ["text fallback"]);
        }
    }
}

#[test]
fn head_noscript_whitespace_stays_before_body_text() {
    for document in documents("<noscript> \r\n\t&#12;fallback<!--body-->") {
        assert_eq!(children(&document, element(&document, "noscript")), ["text  \n\t\u{c}"]);
        assert_eq!(children(&document, element(&document, "body")), ["text fallback", "comment body"]);
    }
}

#[test]
fn head_noscript_null_reprocesses_before_body_filtering() {
    for source in ["<noscript> \0\t<!--body-->", "<noscript> \0</>\t<!--body-->"] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "noscript")), ["text  "]);
            assert_eq!(children(&document, element(&document, "body")), ["text \t", "comment body"]);
            assert!(document.nodes.iter().all(|node| !matches!(&node.kind, NodeKind::Text(value) if value.as_str().contains('\0'))));
        }
    }
}

#[test]
fn head_noscript_reference_tokens_reprocess_without_retokenizing() {
    for (source, expected) in [
        ("<noscript>&#0;", "text \u{fffd}"),
        ("<noscript>&lt;b&gt;safe", "text <b>safe"),
        ("<noscript>&nbsp;", "text \u{a0}"),
    ] {
        for document in documents(source) {
            assert!(document.nodes[element(&document, "noscript")].children.is_empty());
            assert_eq!(children(&document, element(&document, "body")), [expected]);
            assert!(elements(&document, "b").is_empty());
        }
    }
}

#[test]
fn head_noscript_allowed_void_metadata_retains_parent() {
    for document in documents("<noscript><basefont id=a><bgsound id=b><link id=c><meta id=d></noscript><title>T</title>") {
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "title"]);
        assert_eq!(children(&document, element(&document, "noscript")), ["basefont", "bgsound", "link", "meta"]);
        assert!(document.nodes[element(&document, "body")].children.is_empty());
        assert_eq!(document.title, "T");
    }
}

#[test]
fn head_noscript_style_and_noframes_return_to_noscript_mode() {
    for document in documents("<noscript><style>p{content:'&amp;'}\0</style><noframes>&amp;<p>raw</noframes> \t<meta name=x></noscript>") {
        assert_eq!(children(&document, element(&document, "noscript")), ["style", "noframes", "text  \t", "meta"]);
        assert_eq!(children(&document, element(&document, "style")), ["text p{content:'&amp;'}\u{fffd}"]);
        assert_eq!(children(&document, element(&document, "noframes")), ["text &amp;<p>raw"]);
        assert!(elements(&document, "p").is_empty());
    }
}

#[test]
fn head_noscript_base_reprocesses_in_head() {
    for document in documents("<noscript><base href='https://example.test/'><meta name=tail>") {
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "base", "meta"]);
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert!(document.nodes[element(&document, "body")].children.is_empty());
    }
}

#[test]
fn head_noscript_title_reprocesses_in_head() {
    for document in documents("<noscript><title>A &amp; B</title><meta name=tail>") {
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "title", "meta"]);
        assert_eq!(document.title, "A & B");
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
    }
}

#[test]
fn head_noscript_script_reprocesses_without_losing_nonce_provenance() {
    for document in documents("<noscript><style nonce=a nonce=b>x</style><script nonce=c nonce=d>x</script><meta name=tail>") {
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "script", "meta"]);
        assert_eq!(children(&document, element(&document, "noscript")), ["style"]);
        for name in ["style", "script"] {
            assert!(document.nonce_unsafe_nodes.contains(&element(&document, name)));
            assert_eq!(children(&document, element(&document, name)), ["text x"]);
        }
        assert!(!document.scripting_enabled);
    }
}

#[test]
fn head_noscript_template_reprocesses_in_head() {
    for document in documents("<noscript><template><span>kept</span></template><p>body") {
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "template"]);
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert_eq!(children(&document, element(&document, "template")), ["span"]);
        assert_eq!(children(&document, element(&document, "body")), ["p"]);
    }
}

#[test]
fn head_noscript_html_attributes_merge_without_leaving_mode() {
    for document in documents("<html id=first><head><noscript><html id=other lang=en> <!--inside--><meta name=x></noscript><title>T</title>") {
        assert_eq!(document.attr(element(&document, "html"), "id"), Some("first"));
        assert_eq!(document.attr(element(&document, "html"), "lang"), Some("en"));
        assert_eq!(children(&document, element(&document, "noscript")), ["text  ", "comment inside", "meta"]);
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "title"]);
    }
}

#[test]
fn head_noscript_nested_start_and_head_start_are_ignored() {
    for document in documents("<head id=original><noscript id=outer><noscript id=inner><head id=other><meta name=x></noscript><title>T</title>") {
        assert_eq!(elements(&document, "noscript").len(), 1);
        assert_eq!(document.attr(element(&document, "head"), "id"), Some("original"));
        assert_eq!(document.attr(element(&document, "noscript"), "id"), Some("outer"));
        assert_eq!(children(&document, element(&document, "noscript")), ["meta"]);
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "title"]);
    }
}

#[test]
fn head_noscript_other_end_tags_are_ignored() {
    for end in ["head", "body", "html", "div", "template", "style", "table"] {
        for document in documents(&format!("<noscript></{end}><meta name=x></noscript><title>T</title>")) {
            assert_eq!(children(&document, element(&document, "noscript")), ["meta"], "{end}");
            assert_eq!(children(&document, element(&document, "head")), ["noscript", "title"], "{end}");
        }
    }
}

#[test]
fn head_noscript_explicit_end_restores_head_parent() {
    for document in documents("<noscript> \t</noscript><!--head--><meta name=x></head><p>body") {
        assert_eq!(children(&document, element(&document, "noscript")), ["text  \t"]);
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "comment head", "meta"]);
        assert_eq!(children(&document, element(&document, "body")), ["p"]);
    }
}

#[test]
fn head_noscript_br_end_reprocesses_to_attribute_free_body_br() {
    for source in ["<noscript></br><p>x", "<noscript></br id=ignored/><p>x"] {
        for document in documents(source) {
            assert!(document.nodes[element(&document, "noscript")].children.is_empty());
            assert_eq!(children(&document, element(&document, "body")), ["br", "p"]);
            assert_eq!(document.attr(element(&document, "br"), "id"), None);
        }
    }
}

#[test]
fn head_noscript_body_start_retains_explicit_attributes() {
    for document in documents("<noscript><body id=actual><p>x") {
        assert_eq!(document.attr(element(&document, "body"), "id"), Some("actual"));
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert_eq!(children(&document, element(&document, "body")), ["p"]);
    }
}

#[test]
fn head_noscript_ordinary_start_reprocesses_into_body() {
    for document in documents("<noscript><p>fallback</p></noscript><p>tail") {
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert_eq!(children(&document, element(&document, "body")), ["p", "p"]);
        let paragraphs = elements(&document, "p");
        assert_eq!(children(&document, paragraphs[0]), ["text fallback"]);
        assert_eq!(children(&document, paragraphs[1]), ["text tail"]);
    }
}

#[test]
fn head_noscript_table_reprocesses_before_table_modes() {
    for document in documents("<noscript><table>foster<tr><td>cell</table>tail") {
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert_eq!(children(&document, element(&document, "body")), ["text foster", "table", "text tail"]);
        assert_eq!(children(&document, element(&document, "table")), ["tbody"]);
        assert_eq!(children(&document, element(&document, "td")), ["text cell"]);
    }
}

#[test]
fn head_noscript_comments_and_processing_instructions_keep_mode() {
    for document in documents("<noscript><!--a--><!bogus><?marker data?><meta name=x></noscript><!--head-->") {
        assert_eq!(children(&document, element(&document, "noscript")), ["comment a", "comment bogus", "pi marker data", "meta"]);
        assert_eq!(children(&document, element(&document, "head")), ["noscript", "comment head"]);
    }
}

#[test]
fn head_noscript_late_doctype_is_ignored_without_changing_mode() {
    for document in documents("<!doctype html><noscript><!doctype other><meta name=x></noscript>") {
        assert_eq!(document.mode, DocumentMode::NoQuirks);
        assert_eq!(children(&document, 0), ["doctype html", "html"]);
        assert_eq!(children(&document, element(&document, "noscript")), ["meta"]);
    }
}

#[test]
fn head_noscript_malformed_less_than_reprocesses_as_text() {
    for source in ["<noscript><42", "<noscript><\0b>", "<noscript></"] {
        for document in documents(source) {
            let expected = match source { "<noscript><42" => "text <42", "<noscript></" => "text </", _ => "text <b>" };
            assert!(document.nodes[element(&document, "noscript")].children.is_empty());
            assert_eq!(children(&document, element(&document, "body")), [expected]);
            assert!(elements(&document, "b").is_empty());
        }
    }
}

#[test]
fn head_noscript_eof_and_self_closing_syntax_keep_real_nodes() {
    for source in ["<noscript>", "<noscript/>", "<noscript><!--held-->", "<noscript><style>x"] {
        for document in documents(source) {
            assert_eq!(children(&document, element(&document, "html")), ["head", "body"]);
            assert_eq!(children(&document, element(&document, "head")), ["noscript"]);
            assert!(document.nodes[element(&document, "body")].children.is_empty());
        }
    }
}

#[test]
fn head_noscript_body_template_and_foreign_contexts_stay_separate() {
    for document in documents("<body><noscript><p>fallback</p></noscript><p>tail") {
        assert_eq!(children(&document, element(&document, "body")), ["noscript", "p"]);
        assert_eq!(children(&document, element(&document, "noscript")), ["p"]);
    }
    for document in documents("<template><noscript><p>kept</p></noscript></template><p>tail") {
        assert_eq!(children(&document, element(&document, "head")), ["template"]);
        assert_eq!(children(&document, element(&document, "template")), ["noscript"]);
        assert_eq!(children(&document, element(&document, "noscript")), ["p"]);
    }
    for document in documents("<body><svg><noscript>x\0</noscript></svg>") {
        assert_eq!(document.nodes[element(&document, "noscript")].namespace, Namespace::UnknownForeign);
        assert_eq!(children(&document, element(&document, "noscript")), ["text x\0"]);
    }
}

#[test]
fn head_noscript_checked_fragment_admission_is_unchanged() {
    assert!(parse_fragment("<noscript>x</noscript>", "div", true).unwrap_err().contains("recovery is not implemented"));
    assert!(parse_fragment("x", "noscript", false).unwrap_err().contains("element context"));
    let document = parse_fragment("<noscript><span>x</span></noscript>", "div", false).unwrap();
    assert_eq!(children(&document, 0), ["noscript"]);
    assert_eq!(children(&document, element(&document, "noscript")), ["span"]);
}

#[test]
fn head_noscript_node_limit_counts_only_retained_nodes() {
    for source in ["<noscript>", "<noscript>\0"] {
        let document = parse_inert_html(source, 5, &|| Ok(())).unwrap().0;
        assert_eq!(document.nodes.len(), 5);
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert!(document.nodes[element(&document, "body")].children.is_empty());
        assert!(parse_inert_html(source, 4, &|| Ok(())).unwrap_err().contains("node limit"));
    }
    assert!(parse_inert_html("<noscript>x", 5, &|| Ok(())).unwrap_err().contains("node limit"));
    let document = parse_inert_html("<noscript>x", 6, &|| Ok(())).unwrap().0;
    assert_eq!(children(&document, element(&document, "body")), ["text x"]);
}

#[test]
fn head_noscript_large_run_cancels_and_recovers() {
    let source = format!("<noscript>{}tail", "\0".repeat(128 * 1024));
    for inert in [false, true] {
        let calls = std::cell::Cell::new(0usize);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() >= 9 { Err("stop head noscript scan".into()) } else { Ok(()) }
        };
        let result = if inert { parse_inert_html(&source, 100_000, &check).map(|(document, _)| document) }
            else { parse_loaded_document(&source, &check) };
        assert_eq!(result.unwrap_err(), "stop head noscript scan");
        assert!(calls.get() > 1);
    }
    for document in documents(&source) {
        assert!(document.nodes[element(&document, "noscript")].children.is_empty());
        assert_eq!(children(&document, element(&document, "body")), ["text tail"]);
        assert_eq!(document.nodes.len(), 6);
    }
}
