use super::*;

fn text(document: &Document) -> String {
    document.nodes.iter().filter_map(|node| match &node.kind {
        NodeKind::Text(value) => Some(value.render_projection()),
        _ => None,
    }).collect()
}

fn element(document: &Document, name: &str) -> usize {
    document.nodes.iter().position(|node| matches!(&node.kind,
        NodeKind::Element { tag, .. } if tag == name)).unwrap()
}

#[test]
fn named_reference_full_string_and_supplementary_witnesses() {
    for (source, expected) in [
        ("&NotEqualTilde;", "\u{2242}\u{0338}"),
        ("&acE;", "\u{223e}\u{0333}"),
        ("&fjlig;", "fj"),
        ("&Afr;", "\u{1d504}"),
        ("&CounterClockwiseContourIntegral;", "\u{2233}"),
        ("&ThickSpace;", "\u{205f}\u{200a}"),
    ] {
        let source = format!("<!doctype html><p>{source}</p>");
        let document = parse_loaded_document(&source, &|| Ok(())).unwrap();
        assert_eq!(text(&document), expected, "{source}");
    }
}

#[test]
fn named_reference_attribute_value_boundaries_emit_complete_strings() {
    let document = parse_loaded_document(
        "<a data-s='&NotEqualTilde;' data-d=\"&Afr;\" data-u=&fjlig; href='https&colon;&sol;&sol;example.com&sol;&quest;a&equals;1&amp;b&equals;2'>x</a>",
        &|| Ok(())).unwrap();
    let anchor = element(&document, "a");
    assert_eq!(document.attr(anchor, "data-s"), Some("\u{2242}\u{0338}"));
    assert_eq!(document.attr(anchor, "data-d"), Some("\u{1d504}"));
    assert_eq!(document.attr(anchor, "data-u"), Some("fj"));
    assert_eq!(document.attr(anchor, "href"), Some("https://example.com/?a=1&b=2"));
}

#[test]
fn named_reference_rcdata_and_fragment_paths_share_exact_output() {
    let source = "&lt;/textarea&gt;&NotEqualTilde;&Afr;";
    let expected = "</textarea>\u{2242}\u{0338}\u{1d504}";
    for context in ["title", "textarea", "div", "template"] {
        let document = parse_fragment(source, context, false).unwrap();
        assert_eq!(text(&document), expected, "{context}");
        assert_eq!(document.nodes.len(), 2);
    }
    let (document, _) = parse_inert_html(
        "<title>&fjlig; &Afr;</title><textarea>&NotEqualTilde;</textarea>",
        100, &|| Ok(())).unwrap();
    assert_eq!(document.title, "fj \u{1d504}");
    assert_eq!(text(&document), "fj \u{1d504}\u{2242}\u{0338}");
}

#[test]
fn named_reference_longest_match_and_attribute_exception_are_distinct() {
    let value = "&notinva;|&notin|&notit;|&copycat|&amp=1|&amp;=1|&copyé|&copy_";
    let document = parse_loaded_document(&format!("<p data-v='{value}'>{value}</p>"), &|| Ok(())).unwrap();
    assert_eq!(text(&document), "∉|¬in|¬it;|©cat|&=1|&=1|©é|©_");
    assert_eq!(document.attr(element(&document, "p"), "data-v"),
        Some("∉|&notin|&notit;|&copycat|&amp=1|&=1|©é|©_"));
}

#[test]
fn named_reference_unknown_case_and_missing_semicolon_controls() {
    let source = "&Amp;|&apos|&NotEqualTilde|&unknown;|&;|&&|&AMP;|&AElig!";
    let expected = "&Amp;|&apos|&NotEqualTilde|&unknown;|&;|&&|&|Æ!";
    let document = parse_loaded_document(&format!("<p data-v='{source}'>{source}</p>"), &|| Ok(())).unwrap();
    assert_eq!(text(&document), expected);
    assert_eq!(document.attr(element(&document, "p"), "data-v"), Some(expected));
}

#[test]
fn named_reference_output_is_not_retokenized_or_recursively_decoded() {
    let document = parse_loaded_document(
        "<p data-v='&apos; data-other=&quot;injected&quot;'>&amp;lt;b&amp;gt;|&lt;b&gt;&fjlig;&lt;/b&gt;</p>",
        &|| Ok(())).unwrap();
    let paragraph = element(&document, "p");
    assert_eq!(document.attr(paragraph, "data-v"), Some("' data-other=\"injected\""));
    assert_eq!(document.attr(paragraph, "data-other"), None);
    assert_eq!(text(&document), "&lt;b&gt;|<b>fj</b>");
    assert!(!document.nodes.iter().any(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "b")));
}

#[test]
fn named_reference_raw_text_and_comment_controls_remain_literal() {
    for tag in ["script", "style", "iframe", "xmp", "noembed", "noframes"] {
        let source = format!("<{tag}>&NotEqualTilde;&amp;&#65;</{tag}>");
        let document = parse_loaded_document(&source, &|| Ok(())).unwrap();
        assert_eq!(text(&document), "&NotEqualTilde;&amp;&#65;", "{tag}");
    }
    let document = parse_loaded_document("<!--&NotEqualTilde;&amp;&#65;-->", &|| Ok(())).unwrap();
    assert!(document.nodes.iter().any(|node| matches!(&node.kind,
        NodeKind::Comment(data) if data.render_projection() == "&NotEqualTilde;&amp;&#65;")));
}

#[test]
fn named_reference_expansion_retains_both_scalars_and_complete_attributes() {
    let source = "&nLt;&nGt;".repeat(4096);
    let expected = "\u{226a}\u{20d2}\u{226b}\u{20d2}".repeat(4096);
    assert!(expected.len() > source.len());
    let document = parse_loaded_document(&format!("<p data-v='{source}'>{source}</p>"), &|| Ok(())).unwrap();
    assert_eq!(text(&document), expected);
    assert_eq!(document.attr(element(&document, "p"), "data-v"), Some(expected.as_str()));
}

#[test]
fn named_reference_decoded_urls_retain_actual_scheme_authority() {
    let document = parse_loaded_document(
        "<a href='javascript&colon;alert(1)'>x</a><a href='https&colon;&sol;&sol;example.com&sol;?a&equals;1&amp;b&equals;2'>y</a>",
        &|| Ok(())).unwrap();
    let anchors: Vec<_> = document.nodes.iter().enumerate().filter_map(|(id, node)|
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "a").then_some(id)).collect();
    let refused = document.attr(anchors[0], "href").unwrap();
    assert_eq!(refused, "javascript:alert(1)");
    assert!(crate::url::parse(refused).is_err());
    let allowed = document.attr(anchors[1], "href").unwrap();
    let (url, fragment) = crate::url::parse(allowed).unwrap();
    assert!(url.secure);
    assert_eq!(url.host, "example.com");
    assert_eq!(url.query.as_deref(), Some("a=1&b=2"));
    assert!(fragment.is_empty());
}

fn cancel_single_token(source: &str) {
    let checks = std::cell::Cell::new(0usize);
    let result = parse_loaded_document(source, &|| {
        let count = checks.get() + 1;
        checks.set(count);
        if count == 32 { Err("entity walk cancelled".into()) } else { Ok(()) }
    });
    assert!(matches!(result, Err(error) if error == "entity walk cancelled"));
    assert_eq!(checks.get(), 32);
    let recovery = parse_loaded_document("<p>&amp;&nLt;</p>", &|| Ok(())).unwrap();
    assert_eq!(text(&recovery), "&\u{226a}\u{20d2}");
}

#[test]
fn named_reference_single_text_token_cancellation_and_recovery() {
    cancel_single_token(&format!("<p>{}</p>", "x".repeat(262_144)));
}

#[test]
fn named_reference_single_attribute_cancellation_and_recovery() {
    cancel_single_token(&format!("<p data-v='{}'></p>", "&nGt;".repeat(52_429)));
}

#[test]
fn named_reference_long_numeric_token_cancellation_and_recovery() {
    cancel_single_token(&format!("<p>&#{};</p>", "9".repeat(262_144)));
}

#[test]
fn named_reference_node_refusal_does_not_prevent_fresh_recovery() {
    assert!(parse_inert_html("<p>&nLt;</p>", 1, &|| Ok(())).is_err());
    let (document, _) = parse_inert_html("<p>&nLt;</p>", 100, &|| Ok(())).unwrap();
    assert_eq!(text(&document), "\u{226a}\u{20d2}");
}
