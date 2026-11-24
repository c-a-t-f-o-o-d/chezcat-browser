//! Original parser witnesses for HTML script data and PLAINTEXT.
//!
//! These exercise native document/fragment production, not script execution or
//! the required tokenizer diagnostic and streaming fixture gates.

use super::*;

fn documents(source: &str) -> [(&'static str, Document); 2] {
    [
        ("loaded", parse_loaded_document(source, &|| Ok(())).unwrap()),
        ("inert", parse_inert_html(source, 100_000, &|| Ok(())).unwrap().0),
    ]
}

fn elements(document: &Document, name: &str) -> Vec<usize> {
    document.nodes.iter().enumerate().filter_map(|(index, node)| {
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == name)
            .then_some(index)
    }).collect()
}

fn one_element(document: &Document, name: &str) -> usize {
    let found = elements(document, name);
    assert_eq!(found.len(), 1, "expected one {name} element");
    found[0]
}

fn direct_text(document: &Document, parent: usize) -> String {
    document.nodes[parent].children.iter().map(|&child| {
        match &document.nodes[child].kind {
            NodeKind::Text(value) => value.render_projection(),
            other => panic!("unexpected non-text child: {other:?}"),
        }
    }).collect()
}

fn assert_script(document: &Document, expected: &str, paragraph: Option<&str>) {
    let script = one_element(document, "script");
    assert_eq!(document.nodes[script].namespace, Namespace::Html);
    assert_eq!(direct_text(document, script), expected);
    assert_eq!(document.nodes[script].children.len(), usize::from(!expected.is_empty()));
    match paragraph {
        Some(text) => assert_eq!(direct_text(document, one_element(document, "p")), text),
        None => assert!(elements(document, "p").is_empty()),
    }
    let all_text: String = document.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(value) => Some(value.render_projection()),
        _ => None,
    }).collect();
    assert_eq!(all_text, format!("{expected}{}", paragraph.unwrap_or("")));
}

fn script_case(source: &str, expected: &str, paragraph: Option<&str>) {
    for (entry, document) in documents(source) {
        assert_script(&document, expected, paragraph);
        assert!(elements(&document, "b").is_empty(), "markup escaped script text in {entry}");
    }
}

#[test]
fn script_data_plain_and_escaped_end_tags_are_appropriate() {
    for body in [
        "plain &amp; &#65; <b>literal</b>",
        "<!-not-an-escape",
        "<!--one",
        "<!--one-",
        "<!--one--",
        "<!--one-->two",
        "<!--one-<b>two</b>",
    ] {
        script_case(&format!("<script>{body}</sCrIpT><p>tail</p>"), body, Some("tail"));
    }
}

#[test]
fn script_data_double_escaped_end_tags_stay_literal() {
    for body in [
        "<!--<script>one</script>two",
        "<!--<ScRiPt/>one</sCrIpT>two",
        "<!--<script >one</script >two",
        "<!--<script\t>one</script\t>two",
        "<!--<script\n>one</script\n>two",
        "<!--<script\u{000c}>one</script\u{000c}>two",
        "<!--<script>one</scriptx>two</script>three",
        "<!--<ScRiPt>one</scripté>two</sCrIpT>three",
        "<!--<script>one</script>two<!--<script>three</script>four",
        "<!--<script>é🙂</script>ω",
    ] {
        script_case(&format!("<script>{body}</SCRIPT><p>tail</p>"), body, Some("tail"));
    }
}

#[test]
fn script_data_comment_ends_leave_double_escape() {
    for body in [
        "<!--<script>one-->two",
        "<!--<script>one--->two",
        "<!--<script>one-<script>two-->three",
        "<!--<script>--><script>one",
        "<!--<script>--><!--<script>one</script>two",
    ] {
        script_case(&format!("<script>{body}</script><p>tail</p>"), body, Some("tail"));
    }
}

#[test]
fn script_data_double_escape_start_requires_exact_name_and_delimiter() {
    for body in [
        "<!--<scrip>one",
        "<!--<scriptx>one",
        "<!--<script0>one",
        "<!--<scripté>one",
        "<!--<script\u{000b}>one",
        "<!--<script=one",
        "<!--<script:one",
        "<!--<SCRIPT_>one",
        "<!--<ſcript>one",
    ] {
        script_case(&format!("<script>{body}</script><p>tail</p>"), body, Some("tail"));
    }
}

#[test]
fn script_data_appropriate_end_tag_accepts_only_html_boundaries() {
    for prefix in ["A", "<!--A"] {
        for closing in [
            "</script>", "</ScRiPt >", "</script\t>", "</script\n>",
            "</script\u{000c}>", "</script\r\n>", "</script/>",
            "</script data-x='value'>", "</script data-x='a>b'>",
            "</script =foo>", "</script \u{000b}='>'>",
            "</script x=\u{000b}\">",
        ] {
            script_case(&format!("<script>{prefix}{closing}<p>tail</p>"), prefix, Some("tail"));
        }
        for false_close in [
            "</scriptx>", "</scrip>", "</scripté>", "</script\u{000b}>",
            "</script=bad>", "</script_>", "</script:>", "</ſcript>",
        ] {
            let body = format!("{prefix}{false_close}B");
            script_case(&format!("<script>{body}</script><p>tail</p>"), &body, Some("tail"));
        }
    }
}

#[test]
fn script_data_javascript_quotes_do_not_hide_html_end_tags() {
    for body in ["'unterminated", "\"unterminated", "`unterminated", "/* comment"] {
        script_case(&format!("<script>{body}</script><p>tail</p>"), body, Some("tail"));
    }
}

#[test]
fn script_data_references_stay_literal_and_nulls_are_replaced_in_all_text_states() {
    for (body, expected) in [
        ("&amp;&#65;&lt;/script&gt;\0é🙂", "&amp;&#65;&lt;/script&gt;\u{fffd}é🙂"),
        ("<!--&amp;\0", "<!--&amp;\u{fffd}"),
        ("<!--A-\0B", "<!--A-\u{fffd}B"),
        ("<!--A--\0B", "<!--A--\u{fffd}B"),
        ("<!--<script>&amp;\0</script>tail", "<!--<script>&amp;\u{fffd}</script>tail"),
        ("<!--<script>A-\0B</script>tail", "<!--<script>A-\u{fffd}B</script>tail"),
        ("<!--<script>A--\0B</script>tail", "<!--<script>A--\u{fffd}B</script>tail"),
        ("A</script\0>B", "A</script\u{fffd}>B"),
        ("<!--<script\0>A", "<!--<script\u{fffd}>A"),
    ] {
        script_case(&format!("<script>{body}</script><p>tail</p>"), expected, Some("tail"));
    }
}

#[test]
fn script_data_eof_preserves_uncommitted_prefixes() {
    for body in [
        "", "A", "A<", "A</", "A</s", "A</script", "A</ScRiPt",
        "A</scriptx", "A</scriptx ", "A</scripté", "A<!", "A<!-",
        "A<!--", "A<!---", "A<!--<", "A<!--</", "A<!--</s",
        "A<!--</script", "A<!--</scriptx ", "A<!--<s", "A<!--<script",
        "A<!--<script>", "A<!--<script>-", "A<!--<script>--",
        "A<!--<script><", "A<!--<script></", "A<!--<script></s",
        "A<!--<script></script", "A<!--<script></script ",
    ] {
        script_case(&format!("<script>{body}"), body, None);
    }
}

#[test]
fn script_data_eof_discards_an_appropriate_unfinished_end_tag() {
    for prefix in ["A", "<!--A"] {
        for unfinished in [
            "</script ", "</script\t", "</script\n", "</script\u{000c}",
            "</script/", "</script x", "</script x=", "</script x='",
            "</script x=\"value", "</script x='value'", "</script x=value",
        ] {
            script_case(&format!("<script>{prefix}{unfinished}"), prefix, None);
        }
    }
}

#[test]
fn script_data_checked_children_accept_escaped_and_double_escaped_text() {
    for context in ["div", "template"] {
        for scripting in [false, true] {
            let source = "<script><!--<script>one</script>two</script><p>tail</p>";
            let document = parse_fragment(source, context, scripting).unwrap();
            assert_script(&document, "<!--<script>one</script>two", Some("tail"));
            assert_eq!(document.scripting_enabled, scripting);
            let document = parse_fragment("<script><!--A</script ", context, scripting).unwrap();
            assert_script(&document, "<!--A", None);
            for closing in ["</script =foo>", "</script x=\u{000b}\">"] {
                let document = parse_fragment(&format!("<script>A{closing}<p>tail</p>"), context, scripting).unwrap();
                assert_script(&document, "A", Some("tail"));
            }
        }
    }
}

#[test]
fn script_context_fragments_never_treat_input_end_tags_as_appropriate() {
    let source = "<!--<script>one</script><p>tail</p>&amp;\r\n\0";
    let expected = "<!--<script>one</script><p>tail</p>&amp;\n\u{fffd}";
    for context in ["script", "plaintext"] {
        for scripting in [false, true] {
            let document = parse_fragment(source, context, scripting).unwrap();
            assert_eq!(document.nodes.len(), 2);
            assert_eq!(direct_text(&document, 0), expected);
            assert!(elements(&document, "p").is_empty());
            assert_eq!(document.scripting_enabled, scripting);
        }
    }
}

#[test]
fn script_data_checked_fragment_guards_remain_explicit() {
    assert!(parse_fragment("<script>A\0B</script>", "div", false)
        .unwrap_err().contains("NULL handling outside text-only contexts"));
    assert!(parse_fragment("<plaintext>A</plaintext>", "div", false)
        .unwrap_err().contains("structural, table/select or foreign-content recovery"));
    assert!(parse_fragment("A", "table", false)
        .unwrap_err().contains("element context is not implemented"));
}

#[test]
fn script_data_long_utf8_runs_preserve_transitions_across_scan_boundaries() {
    for padding in [16_380, 16_381, 16_382, 16_383, 16_384, 16_385] {
        let body = format!("<!--{}<script>é🙂\0</script>ω", "x".repeat(padding));
        let expected = format!("<!--{}<script>é🙂\u{fffd}</script>ω", "x".repeat(padding));
        script_case(&format!("<script>{body}</script><p>tail</p>"), &expected, Some("tail"));
    }
    let body = format!("<!--<script>{}</script>tail", "é🙂".repeat(16_384));
    script_case(&format!("<script>{body}</script><p>tail</p>"), &body, Some("tail"));
}

fn cancelled_text_scan(source: &str) {
    for inert in [false, true] {
        let calls = std::cell::Cell::new(0usize);
        let check = || {
            let next = calls.get() + 1;
            calls.set(next);
            if next == 8 { Err("text-state scan cancelled".into()) } else { Ok(()) }
        };
        let result = if inert {
            parse_inert_html(source, 100_000, &check).map(|(document, _)| document)
        } else {
            parse_loaded_document(source, &check)
        };
        assert_eq!(result.unwrap_err(), "text-state scan cancelled");
        assert_eq!(calls.get(), 8);
    }
    script_case("<script><!--<script>x</script>y</script><p>fresh</p>",
        "<!--<script>x</script>y", Some("fresh"));
}

#[test]
fn script_data_long_scans_cancel_and_fresh_documents_recover() {
    cancelled_text_scan(&format!("<script>{}</script>", "x".repeat(262_144)));
    cancelled_text_scan(&format!("<script><!--<script>{}</script>x</script>", "é".repeat(131_072)));
    cancelled_text_scan(&format!("<script><!--<{}>x</script>", "s".repeat(262_144)));
}

#[test]
fn script_data_node_admission_refuses_without_poisoning_fresh_parse() {
    let source = "<script><!--<script>x</script>y</script><p>fresh</p>";
    let (complete, _) = parse_inert_html(source, 100, &|| Ok(())).unwrap();
    assert_script(&complete, "<!--<script>x</script>y", Some("fresh"));
    let limit = complete.nodes.len() - 1;
    assert!(parse_inert_html(source, limit, &|| Ok(())).unwrap_err().contains("node limit"));
    let (recovery, _) = parse_inert_html(source, complete.nodes.len(), &|| Ok(())).unwrap();
    assert_eq!(recovery.nodes.len(), complete.nodes.len());
    assert_script(&recovery, "<!--<script>x</script>y", Some("fresh"));
}

#[test]
fn plaintext_loaded_and_inert_paths_consume_literal_text_through_eof() {
    for (source, expected) in [
        ("<plaintext>A&amp;</plaintext><b>B</b>\0", "A&amp;</plaintext><b>B</b>\u{fffd}"),
        ("<plaintext>\nfirst line", "\nfirst line"),
        ("<plaintext>\r\nA\rB\0é🙂", "\nA\nB\u{fffd}é🙂"),
        ("<plaintext/>text</plaintext><p>tail</p>", "text</plaintext><p>tail</p>"),
        ("<plaintext>", ""),
    ] {
        for (_, document) in documents(source) {
            let plaintext = one_element(&document, "plaintext");
            assert_eq!(document.nodes[plaintext].namespace, Namespace::Html);
            assert_eq!(direct_text(&document, plaintext), expected);
            assert!(elements(&document, "b").is_empty());
            assert!(elements(&document, "p").is_empty());
        }
    }
}

#[test]
fn plaintext_start_closes_paragraph_and_keeps_attribute_decoding() {
    for (_, document) in documents("<p>before<plaintext data-x='&amp;'>after<p>literal</p>") {
        let body = one_element(&document, "body");
        let paragraph = one_element(&document, "p");
        let plaintext = one_element(&document, "plaintext");
        assert_eq!(document.nodes[body].children, vec![paragraph, plaintext]);
        assert_eq!(direct_text(&document, paragraph), "before");
        assert_eq!(document.attr(plaintext, "data-x"), Some("&"));
        assert_eq!(direct_text(&document, plaintext), "after<p>literal</p>");
    }
}

#[test]
fn plaintext_character_insertion_reconstructs_active_formatting() {
    for (_, document) in documents("<p><b>before<plaintext>after") {
        let body = one_element(&document, "body");
        let paragraph = one_element(&document, "p");
        let plaintext = one_element(&document, "plaintext");
        let bold = elements(&document, "b");
        assert_eq!(bold.len(), 2);
        assert_eq!(document.nodes[body].children, vec![paragraph, plaintext]);
        assert_eq!(document.nodes[paragraph].children, vec![bold[0]]);
        assert_eq!(document.nodes[plaintext].children, vec![bold[1]]);
        assert_eq!(direct_text(&document, bold[0]), "before");
        assert_eq!(direct_text(&document, bold[1]), "after");
    }
}

#[test]
fn plaintext_does_not_switch_tokenizer_for_foreign_elements() {
    for (_, document) in documents("<svg><plaintext>A</plaintext><g>B</g></svg><p>after</p>") {
        let plaintext = one_element(&document, "plaintext");
        assert_ne!(document.nodes[plaintext].namespace, Namespace::Html);
        assert_eq!(direct_text(&document, plaintext), "A");
        assert_eq!(direct_text(&document, one_element(&document, "g")), "B");
        assert_eq!(direct_text(&document, one_element(&document, "p")), "after");
    }
}

#[test]
fn plaintext_long_scans_cancel_and_node_admission_recovers() {
    cancelled_text_scan(&format!("<plaintext>{}", "é\0".repeat(131_072)));
    let source = "<plaintext>fresh<b>literal</b>";
    let (complete, _) = parse_inert_html(source, 100, &|| Ok(())).unwrap();
    let limit = complete.nodes.len() - 1;
    assert!(parse_inert_html(source, limit, &|| Ok(())).unwrap_err().contains("node limit"));
    let (recovery, _) = parse_inert_html(source, complete.nodes.len(), &|| Ok(())).unwrap();
    assert_eq!(direct_text(&recovery, one_element(&recovery, "plaintext")), "fresh<b>literal</b>");
    assert!(elements(&recovery, "b").is_empty());
}
