//! Original native PRE/LISTING insertion and next-token witnesses.
//! https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100_000, &|| Ok(())).unwrap().0]
}

fn id(doc: &Document, value: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(value)).unwrap()
}

fn element(doc: &Document, name: &str) -> usize {
    doc.nodes.iter().position(|node| matches!(&node.kind,
        NodeKind::Element { tag, .. } if tag == name)).unwrap()
}

fn children(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent].children.iter().map(|&node| match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => tag.clone(),
        NodeKind::Text(value) => format!("#{value}"),
        NodeKind::Comment(value) => format!("!{value}"),
        NodeKind::ProcessingInstruction { target, data } => format!("?{target}:{data}"),
        other => panic!("unexpected child {other:?}"),
    }).collect()
}

fn unique_links(doc: &Document) {
    let mut counts = vec![0; doc.nodes.len()];
    for node in &doc.nodes {
        for &child in &node.children { counts[child] += 1; }
    }
    assert_eq!(counts[0], 0);
    assert!(counts[1..].iter().all(|&count| count == 1));
}

#[test]
fn pre_listing_suppress_one_initial_decoded_lf_in_loaded_and_inert_documents() {
    for name in ["pre", "listing"] {
        for first in ["\n", "\r", "\r\n", "&#10;", "&#xA;", "&NewLine;"] {
            let source = format!("<{name} id=subject>{first}A\nB</{name}><span id=tail>C</span>");
            for doc in documents(&source) {
                assert_eq!(children(&doc, id(&doc, "subject")), ["#A\nB"], "{source:?}");
                assert_eq!(children(&doc, id(&doc, "tail")), ["#C"]);
                assert_eq!(doc.nodes[id(&doc, "subject")].namespace, Namespace::Html);
                unique_links(&doc);
            }
        }
    }
}

#[test]
fn pre_listing_suppress_exactly_one_lf_even_when_references_share_a_text_run() {
    for name in ["pre", "listing"] {
        for first in ["\n\n", "&#10;\n", "\n&#10;", "&#10;&NewLine;", "\r\n\r\n"] {
            let source = format!("<{name} id=subject>{first}A</{name}>");
            for doc in documents(&source) {
                assert_eq!(children(&doc, id(&doc, "subject")), ["#\nA"], "{source:?}");
            }
        }
    }
}

#[test]
fn pre_listing_first_non_lf_character_consumes_the_opportunity() {
    for name in ["pre", "listing"] {
        for (source, expected) in [(" \nA", "# \nA"), ("\t\nA", "#\t\nA"),
            ("X\nA", "#X\nA"), ("&#13;\nA", "#\r\nA"),
            ("&#0;\nA", "#\u{fffd}\nA"), ("&amp;\nA", "#&\nA")] {
            for doc in documents(&format!("<{name} id=subject>{source}</{name}>")) {
                assert_eq!(children(&doc, id(&doc, "subject")), [expected], "{name}/{source:?}");
            }
        }
    }
}

#[test]
fn pre_listing_initial_lf_dispatch_precedes_data_null_filtering() {
    for name in ["pre", "listing"] {
        for (source, expected) in [("\0\nA", "#\nA"), ("\n\0A", "#A"),
            ("\0&#10;A", "#\nA"), ("&#10;\0A", "#A")] {
            for doc in documents(&format!("<{name} id=subject>{source}</{name}>")) {
                assert_eq!(children(&doc, id(&doc, "subject")), [expected], "{name}/{source:?}");
            }
        }
    }
}

#[test]
fn pre_listing_noncharacter_tokens_consume_the_opportunity_even_if_ignored() {
    for name in ["pre", "listing"] {
        for (token, expected) in [
            ("<!--kept-->", vec!["!kept", "#\nA"]),
            ("<?kept data?>", vec!["?kept:data", "#\nA"]),
            ("<!doctype html>", vec!["#\nA"]),
            ("</missing>", vec!["#\nA"]),
        ] {
            for doc in documents(&format!("<{name} id=subject>{token}\nA</{name}>")) {
                assert_eq!(children(&doc, id(&doc, "subject")), expected, "{name}/{token}");
                unique_links(&doc);
            }
        }
    }
}

#[test]
fn pre_listing_discarded_empty_end_open_does_not_emit_a_token() {
    for name in ["pre", "listing"] {
        for skipped in ["</>", "</></>", "</></></>"] {
            for doc in documents(&format!("<{name} id=subject>{skipped}\nA</{name}>")) {
                assert_eq!(children(&doc, id(&doc, "subject")), ["#A"]);
            }
        }
    }
}

#[test]
fn pre_listing_non_tag_less_than_is_the_next_character_token() {
    for name in ["pre", "listing"] {
        for doc in documents(&format!("<{name} id=subject><\nA</{name}>")) {
            assert_eq!(children(&doc, id(&doc, "subject")), ["#<\nA"]);
        }
    }
}

#[test]
fn pre_listing_child_start_consumes_the_outer_opportunity() {
    for name in ["pre", "listing"] {
        let source = format!("<{name} id=subject><span id=child>\nA</span>\nB</{name}>");
        for doc in documents(&source) {
            assert_eq!(children(&doc, id(&doc, "subject")), ["span", "#\nB"]);
            assert_eq!(children(&doc, id(&doc, "child")), ["#\nA"]);
        }
    }
}

#[test]
fn pre_listing_nested_elements_have_independent_node_identity_opportunities() {
    for outer in ["pre", "listing"] {
        for inner in ["pre", "listing"] {
            let source = format!("<{outer} id=outer>\nA<{inner} id=inner>\nB</{inner}>\nC</{outer}>");
            for doc in documents(&source) {
                assert_eq!(children(&doc, id(&doc, "outer")), ["#A", inner, "#\nC"]);
                assert_eq!(children(&doc, id(&doc, "inner")), ["#B"]);
                assert_ne!(id(&doc, "outer"), id(&doc, "inner"));
                unique_links(&doc);
            }
        }
    }
}

#[test]
fn pre_listing_close_the_nearest_paragraph_before_insertion() {
    for name in ["pre", "listing"] {
        let source = format!("<div id=host><p id=old><span id=inline>A<{name} id=subject>\nB</{name}>C</div>");
        for doc in documents(&source) {
            assert_eq!(children(&doc, id(&doc, "host")), ["p", name, "#C"]);
            assert_eq!(children(&doc, id(&doc, "old")), ["span"]);
            assert_eq!(children(&doc, id(&doc, "inline")), ["#A"]);
            assert_eq!(children(&doc, id(&doc, "subject")), ["#B"]);
            unique_links(&doc);
        }
    }
}

#[test]
fn pre_listing_paragraph_closure_respects_button_scope() {
    for name in ["pre", "listing"] {
        for boundary in ["button", "object", "template"] {
            let source = format!("<div id=host><p id=old>A<{boundary} id=gate><{name} id=subject>\nB</{name}>C</{boundary}>D</div>");
            for doc in documents(&source) {
                assert_eq!(doc.nodes[id(&doc, "host")].children, [id(&doc, "old")]);
                assert_eq!(children(&doc, id(&doc, "old")), ["#A", boundary, "#D"]);
                assert_eq!(children(&doc, id(&doc, "gate")), [name, "#C"]);
                assert_eq!(children(&doc, id(&doc, "subject")), ["#B"]);
            }
        }
    }
}

#[test]
fn pre_listing_ignored_lf_allocates_neither_text_nor_formatting_clones() {
    for name in ["pre", "listing"] {
        for doc in documents(&format!("<p id=old><b id=bold>A<{name} id=subject>\n</{name}>B")) {
            let body = element(&doc, "body");
            assert_eq!(children(&doc, body), ["p", name, "b"]);
            assert!(doc.nodes[id(&doc, "subject")].children.is_empty());
            assert_eq!(children(&doc, id(&doc, "old")), ["b"]);
            let first = doc.nodes[id(&doc, "old")].children[0];
            let after = doc.nodes[body].children[2];
            assert_ne!(first, after);
            assert_eq!(children(&doc, first), ["#A"]);
            assert_eq!(children(&doc, after), ["#B"]);
            assert_eq!(doc.attr(after, "id"), Some("bold"));
            assert_eq!(doc.nodes.len(), 10);
            unique_links(&doc);
        }
    }
}

#[test]
fn pre_listing_refresh_foster_placement_when_paragraph_closure_exposes_table() {
    for name in ["pre", "listing"] {
        let source = format!("<body><table id=table><p id=old>A<{name} id=subject>\nB</{name}>C</table>D");
        for doc in documents(&source) {
            assert_eq!(children(&doc, element(&doc, "body")), ["p", name, "#C", "table", "#D"]);
            assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
            assert_eq!(children(&doc, id(&doc, "subject")), ["#B"]);
            assert!(doc.nodes[id(&doc, "table")].children.is_empty());
            unique_links(&doc);
        }
    }
}

#[test]
fn pre_listing_keep_actual_cell_and_caption_placement_after_paragraph_closure() {
    for name in ["pre", "listing"] {
        for (prefix, suffix, host) in [("<table><tr><td id=host>", "</td></tr></table>", "td"),
            ("<table><caption id=host>", "</caption></table>", "caption")] {
            let source = format!("{prefix}<p id=old>A<{name} id=subject>\nB</{name}>C{suffix}");
            for doc in documents(&source) {
                assert_eq!(children(&doc, id(&doc, "host")), ["p", name, "#C"], "{host}/{name}");
                assert_eq!(children(&doc, id(&doc, "old")), ["#A"]);
                assert_eq!(children(&doc, id(&doc, "subject")), ["#B"]);
                unique_links(&doc);
            }
        }
    }
}

#[test]
fn pre_listing_leave_foreign_and_uncertain_native_subsets_unchanged() {
    for name in ["pre", "listing"] {
        for foreign in ["svg", "math"] {
            let source = format!("<{foreign}><{name} id=subject>\nA</{name}></{foreign}>");
            for doc in documents(&source) {
                assert_eq!(doc.nodes[id(&doc, "subject")].namespace, Namespace::UnknownForeign);
                assert_eq!(children(&doc, id(&doc, "subject")), ["#\nA"]);
            }
        }
        let source = format!("<div><svg><span></div><{name} id=subject>\nA</{name}>");
        for doc in documents(&source) {
            assert_eq!(doc.nodes[id(&doc, "subject")].namespace, Namespace::UnknownForeign);
            assert_eq!(children(&doc, id(&doc, "subject")), ["#\nA"]);
        }
    }
}

#[test]
fn pre_listing_preserve_checked_pre_and_context_only_refusal_boundaries() {
    for context in ["div", "pre", "template"] {
        for scripting in [false, true] {
            let doc = parse_fragment("<pre id=subject>\nA</pre>", context, scripting).unwrap();
            assert_eq!(children(&doc, id(&doc, "subject")), ["#A"]);
            assert_eq!(doc.scripting_enabled, scripting);
            let doc = parse_fragment("\nA", "pre", scripting).unwrap();
            assert_eq!(children(&doc, 0), ["#\nA"]);
            assert!(parse_fragment("<listing>\nA</listing>", context, scripting).unwrap_err().contains("structural"));
            assert!(parse_fragment("\nA", "listing", scripting).unwrap_err().contains("element context"));
        }
    }
}

#[test]
fn pre_listing_preserve_flat_and_text_only_entries() {
    for name in ["pre", "listing"] {
        let doc = parse(&format!("<{name} id=subject>\nA</{name}>"));
        assert_eq!(children(&doc, id(&doc, "subject")), ["#\nA"]);
    }
    for doc in documents("<textarea id=t>\nA</textarea><style id=s>\nB</style><script id=c>\nC</script>") {
        assert_eq!(children(&doc, id(&doc, "t")), ["#A"]);
        assert_eq!(children(&doc, id(&doc, "s")), ["#\nB"]);
        assert_eq!(children(&doc, id(&doc, "c")), ["#\nC"]);
    }
    for context in ["textarea", "style", "script"] {
        let doc = parse_fragment("\nA", context, false).unwrap();
        assert_eq!(children(&doc, 0), ["#\nA"]);
    }
}

#[test]
fn pre_listing_suppressed_lf_requires_only_actual_retained_node_capacity() {
    for name in ["pre", "listing"] {
        for middle in ["\n", "\n\0", "&#10;", "</>\n"] {
            let source = format!("<{name} id=subject>{middle}</{name}>");
            let (doc, doctype) = parse_inert_html(&source, 5, &|| Ok(())).unwrap();
            assert_eq!(doc.nodes.len(), 5);
            assert_eq!(doctype, None);
            assert!(doc.nodes[id(&doc, "subject")].children.is_empty());
            assert!(parse_inert_html(&source, 4, &|| Ok(())).unwrap_err().contains("node limit"));
            unique_links(&doc);
        }
    }
}

#[test]
fn pre_listing_remain_cancelable_and_a_new_parse_has_fresh_state() {
    for name in ["pre", "listing"] {
        let source = format!("<{name}>{}A</{name}>", "\n".repeat(100_000));
        for inert in [false, true] {
            let calls = std::cell::Cell::new(0);
            let cancel = || {
                calls.set(calls.get() + 1);
                if calls.get() == 8 { Err("original pre/listing cancellation".into()) } else { Ok(()) }
            };
            let error = if inert { parse_inert_html(&source, 1000, &cancel).unwrap_err() }
                else { parse_loaded_document(&source, &cancel).unwrap_err() };
            assert_eq!(error, "original pre/listing cancellation");
            assert_eq!(calls.get(), 8);
        }
        for doc in documents(&format!("<{name} id=subject>\nA</{name}>")) {
            assert_eq!(children(&doc, id(&doc, "subject")), ["#A"]);
        }
    }
}
