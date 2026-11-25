//! Original witnesses for shared HTML tag and attribute token states.
use super::*;
use std::cell::Cell;

fn token(source: &str, checked: bool) -> Tag {
    match read_tag_mode(source, 0, checked, None).unwrap() {
        TagToken::Complete(tag) => tag,
        TagToken::Incomplete => panic!("incomplete token: {source:?}"),
        TagToken::NotATag => panic!("missing token: {source:?}"),
        TagToken::Unsupported(error) => panic!("refused token {source:?}: {error}"),
    }
}

fn attrs(tag: &Tag) -> Vec<(&str, &str)> {
    tag.attrs.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect()
}

fn element(doc: &Document, name: &str) -> usize {
    doc.nodes.iter().position(|node| matches!(&node.kind,
        NodeKind::Element { tag, .. } if tag == name)).unwrap_or_else(|| panic!("missing {name:?}"))
}

fn text(doc: &Document) -> String {
    doc.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(value) => Some(value.render_projection()),
        _ => None,
    }).collect()
}

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
     parse_inert_html(source, 1000, &|| Ok(())).unwrap().0]
}

#[test]
fn tag_names_keep_non_delimiters_and_unicode() {
    for name in ["x.foo", "x-é", "x=part", "x\"part", "x'part", "x<part", "x?part", "x!part"] {
        for checked in [false, true] {
            let source = format!("<{name} id=keep>");
            let tag = token(&source, checked);
            assert_eq!(tag.name, name, "{source:?}");
            assert_eq!(attrs(&tag), [("id", "keep")]);
            assert_eq!(tag.end, source.len());
        }
    }
}

#[test]
fn tag_and_attribute_names_fold_only_ascii() {
    for checked in [false, true] {
        let tag = token("<X-É DATA-É='MiXeD' ID=Actual>", checked);
        assert_eq!(tag.name, "x-É");
        assert_eq!(attrs(&tag), [("data-É", "MiXeD"), ("id", "Actual")]);
        assert_eq!(tag.id.as_deref(), Some("Actual"));
    }
}

#[test]
fn tag_names_use_exact_html_space_delimiters() {
    for space in [' ', '\t', '\n', '\r', '\x0c'] {
        for checked in [false, true] {
            let tag = token(&format!("<DiV{space}A{space}={space}x{space}>"), checked);
            assert_eq!(tag.name, "div");
            assert_eq!(attrs(&tag), [("a", "x")]);
        }
    }
}

#[test]
fn vertical_tab_never_aliases_a_native_tag_name() {
    for name in ["div", "script", "style", "iframe", "textarea"] {
        for checked in [false, true] {
            let source = format!("<{name}\x0bsrc=x>");
            let tag = token(&source, checked);
            assert_eq!(tag.name, format!("{name}\x0bsrc=x"));
            assert!(tag.attrs.is_empty());
        }
    }
}

#[test]
fn vertical_tab_is_literal_attribute_name_and_value_data() {
    for checked in [false, true] {
        let tag = token("<div A\x0bB=x\x0bY q='a\x0bb' d=\"c\x0bd\">", checked);
        assert_eq!(attrs(&tag), [("a\x0bb", "x\x0bY"), ("q", "a\x0bb"), ("d", "c\x0bd")]);
    }
}

#[test]
fn tag_name_nulls_become_replacement_characters() {
    for (source, expected) in [("<x\0a>", "x\u{fffd}a"), ("<script\0>", "script\u{fffd}"), ("</X\0É>", "x\u{fffd}É")] {
        let tag = token(source, false);
        assert_eq!(tag.name, expected);
        assert_eq!(tag.closing, source.starts_with("</"));
        assert!(tag.attrs.is_empty());
    }
}

#[test]
fn attribute_names_and_all_value_states_replace_literal_nulls() {
    let tag = token("<div A\0B='x\0y' c=\"x\0y\" d=x\0y e='&#0;&#xD800;\0'>", false);
    assert_eq!(attrs(&tag), [("a\u{fffd}b", "x\u{fffd}y"), ("c", "x\u{fffd}y"),
        ("d", "x\u{fffd}y"), ("e", "\u{fffd}\u{fffd}\u{fffd}")]);
}

#[test]
fn initial_equals_belongs_to_the_attribute_name() {
    for checked in [false, true] {
        for (source, expected) in [
            ("<div =x='a>b'>", vec![("=x", "a>b")]),
            ("<div ==\">\">", vec![("=", ">")]),
            ("<div =='>x'>", vec![("=", ">x")]),
            ("<div = =next>", vec![("=", "next")]),
        ] {
            let tag = token(source, checked);
            assert_eq!(attrs(&tag), expected, "{source:?}");
            assert_eq!(tag.end, source.len(), "{source:?}");
        }
    }
}

#[test]
fn adjacent_quoted_attributes_reconsume_without_losing_data() {
    for checked in [false, true] {
        let tag = token("<div a='A'b=\"B\"c=C>", checked);
        assert_eq!(attrs(&tag), [("a", "A"), ("b", "B"), ("c", "C")]);
    }
    assert!(!token("<svg a='A'b='B'>", false).clean_syntax);
}

#[test]
fn unquoted_values_keep_slashes_quotes_and_parse_error_characters() {
    for checked in [false, true] {
        let tag = token("<div a=b/ b=`x\"'<=y c=&amp=1 d=&amp;=2>", checked);
        assert_eq!(attrs(&tag), [("a", "b/"), ("b", "`x\"'<=y"), ("c", "&amp=1"), ("d", "&=2")]);
        assert!(!tag.self_closing);
        assert_eq!(attrs(&token("<div a=b/>", checked)), [("a", "b/")]);
    }
}

#[test]
fn boolean_and_empty_attributes_follow_before_value_state() {
    for checked in [false, true] {
        let tag = token("<div a b = c d=>", checked);
        assert_eq!(attrs(&tag), [("a", ""), ("b", "c"), ("d", "")]);
        assert_eq!(attrs(&token("<div a='' b=\"\" c>", checked)), [("a", ""), ("b", ""), ("c", "")]);
    }
}

#[test]
fn self_closing_recovery_keeps_the_real_token_flag_and_svg_evidence() {
    for checked in [false, true] {
        assert!(token("<div/>", checked).self_closing);
        assert!(!token("<div/ a=x>", checked).self_closing);
        assert_eq!(attrs(&token("<div/ a=x>", checked)), [("a", "x")]);
        assert!(!token("<div / >", checked).self_closing);
    }
    assert!(token("<svg/>", false).clean_syntax);
    assert!(!token("<svg//>", false).clean_syntax);
    assert!(!token("<svg / >", false).clean_syntax);
}

#[test]
fn duplicate_names_compare_after_ascii_and_null_normalization() {
    let tag = token("<div A\0B=first a\u{fffd}b=second ID=original id=other>", false);
    assert_eq!(attrs(&tag), [("a\u{fffd}b", "first"), ("id", "original")]);
    assert!(tag.duplicate_attributes);
    assert_eq!(tag.id.as_deref(), Some("original"));
}

#[test]
fn end_tag_tokens_share_attribute_states_and_keep_consumed_boundary() {
    for checked in [false, true] {
        let source = "</X.foo =name='a>b' A=one a=two/>";
        let tag = token(source, checked);
        assert_eq!(tag.name, "x.foo");
        assert!(tag.closing);
        assert_eq!(attrs(&tag), [("=name", "a>b"), ("a", "one")]);
        assert!(tag.duplicate_attributes);
        // The slash remains in the discarded duplicate unquoted value.
        assert!(!tag.self_closing);
        assert_eq!(tag.end, source.len());
    }
}

#[test]
fn eof_in_tag_attribute_and_self_closing_states_drops_the_token() {
    for source in ["<div", "<div ", "<div a", "<div a=", "<div a='x", "<div a=\"x", "<div a=x", "<div/", "<div a='x'", "<x.foo", "<div =='>"] {
        for checked in [false, true] {
            assert!(matches!(read_tag_mode(source, 0, checked, None).unwrap(), TagToken::Incomplete), "{source:?}");
        }
    }
    for source in ["<", "</", "<1x>", "<é>"] {
        assert!(matches!(read_tag_mode(source, 0, false, None).unwrap(), TagToken::NotATag), "{source:?}");
    }
    for source in ["<table", "<textarea a='"] {
        assert!(matches!(read_tag_mode(source, 0, true, None).unwrap(), TagToken::Unsupported(_)));
    }
}

#[test]
fn loaded_and_inert_documents_keep_actual_names_attributes_and_tail() {
    for doc in documents("<x.foo ID=kept =x='a>b' a\0b='x\0y'>A</x.foo><p>tail</p>") {
        let node = element(&doc, "x.foo");
        assert_eq!(doc.attr(node, "id"), Some("kept"));
        assert_eq!(doc.attr(node, "=x"), Some("a>b"));
        assert_eq!(doc.attr(node, "a\u{fffd}b"), Some("x\u{fffd}y"));
        assert_eq!(text(&doc), "Atail");
        assert_eq!(doc.nodes[node].namespace, Namespace::Html);
        assert!(!doc.nodes[node].children.contains(&element(&doc, "p")));
    }
}

#[test]
fn loaded_and_inert_resource_like_names_do_not_become_resource_elements() {
    for doc in documents("<script\0 id=a>A<b>B</b></script\0><style\x0b id=c>C<i>D</i></style\x0b>") {
        assert!(!doc.nodes.iter().any(|node| matches!(&node.kind, NodeKind::Element {tag,..} if tag=="script"||tag=="style")));
        element(&doc, "script\u{fffd}");
        element(&doc, "style\x0b");
        element(&doc, "b");
        element(&doc, "i");
        assert_eq!(text(&doc), "ABCD");
    }
}

#[test]
fn rawtext_and_rcdata_vt_end_names_stay_literal_until_real_end() {
    for name in ["style", "iframe", "xmp", "noembed", "noframes", "title", "textarea"] {
        let source = format!("<{name}>A</{name}\x0b>B</{name}><p>tail</p>");
        for doc in documents(&source) {
            let node = element(&doc, name);
            let [child] = doc.nodes[node].children.as_slice() else { panic!("single raw/RCDATA child") };
            assert!(matches!(&doc.nodes[*child].kind, NodeKind::Text(value) if value.render_projection()==format!("A</{name}\x0b>B")));
            assert_eq!(text(&doc), format!("A</{name}\x0b>Btail"));
        }
    }
}

#[test]
fn checked_fragments_enable_names_but_retain_null_and_svg_refusals() {
    for context in ["div", "template"] {
        for scripting in [false, true] {
            let doc = parse_fragment("<x.foo =x='a>b'>A</x.foo><p>tail</p>", context, scripting).unwrap();
            assert_eq!(doc.attr(element(&doc, "x.foo"), "=x"), Some("a>b"));
            assert_eq!(text(&doc), "Atail");
            for source in ["<x\0a>", "<div a\0b=x>", "<div a='x\0y'>", "A\0B"] {
                assert!(parse_fragment(source, context, scripting).unwrap_err().contains("NULL handling outside text-only contexts"));
            }
        }
    }
    for source in ["<svg><g id='a'id='b'></g></svg>", "<svg><g//></svg>", "<svg><g data-v=x\x0by></g></svg>", "<svg><g =x='a>b'></g></svg>"] {
        assert!(parse_fragment(source, "template", false).is_err(), "{source:?}");
    }
    let valid = parse_fragment("<svg><g data-value='literal'><rect width='1'/></g></svg>", "template", false).unwrap();
    assert_eq!(valid.nodes[element(&valid, "svg")].namespace, Namespace::Svg);
}

#[test]
fn checked_style_end_attribute_recovery_discards_attributes_and_preserves_tail() {
    for closing in ["</style =foo>", "</style =name='a>b'>", "</style ==\">\">"] {
        let source = format!("<style id=kept>A{closing}<p id=tail>tail</p>");
        for context in ["div", "template"] {
            for scripting in [false, true] {
                let doc = parse_fragment(&source, context, scripting).unwrap();
                let style = element(&doc, "style");
                let tail = element(&doc, "p");
                assert_eq!(doc.nodes[0].children, [style, tail]);
                let NodeKind::Element { attrs, .. } = &doc.nodes[style].kind else { unreachable!() };
                assert_eq!(attrs.len(), 1);
                assert_eq!(doc.attr(style, "id"), Some("kept"));
                assert_eq!(doc.attr(tail, "id"), Some("tail"));
                let [child] = doc.nodes[style].children.as_slice() else { panic!("single STYLE text child") };
                assert!(matches!(&doc.nodes[*child].kind, NodeKind::Text(value) if value == "A"));
                assert_eq!(text(&doc), "Atail");
            }
        }
    }
}

#[test]
fn duplicate_resource_attributes_retain_nonce_unsafe_evidence() {
    for doc in documents("<script id=s nonce=one NONCE=two></script><style id=c nonce=one nonce=two></style><link id=l href=one HREF=two>") {
        for (name, key, value) in [("script", "nonce", "one"), ("style", "nonce", "one"), ("link", "href", "one")] {
            let node = element(&doc, name);
            assert_eq!(doc.attr(node, key), Some(value));
            assert!(doc.nonce_unsafe_nodes.contains(&node));
        }
    }
}

#[test]
fn long_tag_scans_cancel_and_fresh_parse_recovers() {
    let long = "a".repeat(262_144);
    for source in [format!("<x{long}"), format!("<div {}", " ".repeat(262_144)),
        format!("<div a{long}"), format!("<div a {}", " ".repeat(262_144)),
        format!("<div a={}{}", " ".repeat(262_144), long),
        format!("<div a='{long}"), format!("<div a={long}")]
    {
        let count = Cell::new(0usize);
        let result = read_tag_mode(&source, 0, false, Some(&|| {
            let next = count.get() + 1; count.set(next);
            if next == 8 { Err("tag token cancelled".into()) } else { Ok(()) }
        }));
        assert!(matches!(result, Err(error) if error=="tag token cancelled"), "source prefix {:?}, polls {}", &source[..source.len().min(20)], count.get());
        assert_eq!(count.get(), 8);
        assert_eq!(attrs(&token("<div id=recovered>", false)), [("id", "recovered")]);
    }
}

#[test]
fn native_node_refusal_and_parser_cancellation_do_not_return_a_prefix() {
    assert!(parse_inert_html("<x.foo id=one></x.foo><p>tail</p>", 4, &|| Ok(())).is_err());
    let count = Cell::new(0usize);
    let source = format!("<x{}>", "a".repeat(262_144));
    let result = parse_loaded_document(&source, &|| {
        let next = count.get() + 1; count.set(next);
        if next == 10 { Err("loaded tag cancelled".into()) } else { Ok(()) }
    });
    assert!(matches!(result, Err(error) if error=="loaded tag cancelled"));
    assert_eq!(count.get(), 10);
    let recovered = parse_inert_html("<p id=kept>ready</p>", 20, &|| Ok(())).unwrap().0;
    assert_eq!(recovered.attr(element(&recovered, "p"), "id"), Some("kept"));
    assert_eq!(text(&recovered), "ready");
}
