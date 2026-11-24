use super::*;
use super::super::{parse_inert_html, parse_loaded_document, Document, NodeKind, RAW_TEXT_CHECK_BYTES};

#[test]
fn unchanged_character_data_keeps_its_owned_buffer() {
    let text = "雪🙂&amp;\u{fffd}".repeat(RAW_TEXT_CHECK_BYTES);
    let pointer = text.as_ptr();
    let capacity = text.capacity();
    let output = without_nulls(text, &|| Ok(())).unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(output.capacity(), capacity);
    assert_eq!(output, "雪🙂&amp;\u{fffd}".repeat(RAW_TEXT_CHECK_BYTES));
}

#[test]
fn null_removal_preserves_unicode_across_copy_boundaries() {
    let prefix = "x".repeat(RAW_TEXT_CHECK_BYTES - 2);
    let source = format!("{prefix}雪\0🙂\0&amp;\0\u{fffd}\0e\u{301}\0");
    assert_eq!(without_nulls(source, &|| Ok(())).unwrap(),
        format!("{prefix}雪🙂&amp;\u{fffd}e\u{301}"));
    assert_eq!(without_nulls("\0".repeat(RAW_TEXT_CHECK_BYTES * 3), &|| Ok(())).unwrap(), "");
    assert_eq!(without_nulls(String::new(), &|| Ok(())).unwrap(), "");
}

#[test]
fn null_counting_checks_cancellation_before_finishing_the_run() {
    let calls = std::cell::Cell::new(0usize);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 2 { Err("stop character sizing".into()) } else { Ok(()) }
    };
    let text = "\0".repeat(RAW_TEXT_CHECK_BYTES * 3);
    assert_eq!(without_nulls(text.clone(), &check).unwrap_err(), "stop character sizing");
    assert_eq!(calls.get(), 2);
    assert_eq!(without_nulls(text, &|| Ok(())).unwrap(), "");
}

#[test]
fn null_copying_checks_cancellation_after_sizing_and_recovers() {
    let text = "\0雪🙂".repeat(RAW_TEXT_CHECK_BYTES);
    let sizing_chunks = text.len().div_ceil(RAW_TEXT_CHECK_BYTES);
    let calls = std::cell::Cell::new(0usize);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == sizing_chunks + 2 { Err("stop character copy".into()) } else { Ok(()) }
    };
    assert_eq!(without_nulls(text.clone(), &check).unwrap_err(), "stop character copy");
    assert_eq!(calls.get(), sizing_chunks + 2);
    assert_eq!(without_nulls(text, &|| Ok(())).unwrap(), "雪🙂".repeat(RAW_TEXT_CHECK_BYTES));
}

fn element(document: &Document, name: &str) -> usize {
    document.nodes.iter().position(|node|
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == name)).unwrap()
}

#[test]
fn tail_whitespace_reconstructs_without_changing_the_outer_mode() {
    for (tail, after_html) in [("</body>", false), ("</body></html>", true)] {
        for nul in ["", "\0"] {
            let source = format!("<p><b>x</p>{tail} {nul}<!--marker-->");
            for document in [parse_loaded_document(&source, &|| Ok(())).unwrap(),
                parse_inert_html(&source, 100, &|| Ok(())).unwrap().0] {
                let body = element(&document, "body");
                assert_eq!(document.nodes[body].children.len(), 2);
                let reconstructed = document.nodes[body].children[1];
                assert!(matches!(&document.nodes[reconstructed].kind,
                    NodeKind::Element { tag, .. } if tag == "b"));
                let children = &document.nodes[reconstructed].children;
                assert!(matches!(&document.nodes[children[0]].kind, NodeKind::Text(value) if value == " "));
                let comment = document.nodes.iter().position(|node|
                    matches!(&node.kind, NodeKind::Comment(value) if value == "marker")).unwrap();
                let parent = if !nul.is_empty() { reconstructed }
                    else if after_html { 0 } else { element(&document, "html") };
                assert!(document.nodes[parent].children.contains(&comment));
                assert_eq!(children.len(), if nul.is_empty() { 1 } else { 2 });
            }
        }
    }
}

#[test]
fn ignored_table_run_preserves_previously_buffered_characters() {
    for source in ["<table> </>\0</>\t<!--marker-->", "<table>A</>\0</>B<!--marker-->"] {
        for document in [parse_loaded_document(source, &|| Ok(())).unwrap(),
            parse_inert_html(source, 100, &|| Ok(())).unwrap().0] {
            let table = element(&document, "table");
            let body = element(&document, "body");
            let whitespace = source.starts_with("<table> ");
            let parent = if whitespace { table } else { body };
            let text = document.nodes[parent].children[0];
            assert!(matches!(&document.nodes[text].kind, NodeKind::Text(value)
                if value == if whitespace { " \t" } else { "AB" }));
            let comment = *document.nodes[table].children.last().unwrap();
            assert!(matches!(&document.nodes[comment].kind, NodeKind::Comment(value) if value == "marker"));
            assert_eq!(document.nodes[body].children.len(), if whitespace { 1 } else { 2 });
            assert_eq!(document.nodes[table].children.len(), if whitespace { 2 } else { 1 });
        }
    }
}
