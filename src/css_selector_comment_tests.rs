use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn document(sheet: &str, body: &str) -> Document {
    crate::html::parse(&format!(
        "<!doctype html><html><head><style>{sheet}</style></head><body>{body}</body></html>"
    ))
}

fn assert_matches(selector: &str, body: &str, names: &[&str], wanted: &[&str]) {
    let doc = document(&format!("{selector}{{width:37px}}"), body);
    let candidates: Vec<_> = names.iter().map(|name| id(&doc, name)).collect();
    let expected: Vec<_> = wanted.iter().map(|name| id(&doc, name)).collect();
    let mut work = 100_000;
    assert!(valid_query_selector(selector), "{selector:?}");
    assert_eq!(
        query_all(&doc, selector, &candidates, None, &mut work).unwrap(),
        expected,
        "query {selector:?}"
    );
    let styles = compute(&doc, &HashMap::new());
    for &node in &candidates {
        assert_eq!(
            styles[node].width == Length::Px(37.0),
            expected.contains(&node),
            "stylesheet {selector:?}, node {node}"
        );
    }
}

const COMPOUND_BODY: &str = "<p id=compound class=x><span id=descendant class=x></span></p><div id=other class=x></div>";
const COMPOUND_NAMES: &[&str] = &["compound", "descendant", "other"];

#[test]
fn selector_comments_compound_stays_on_the_same_element() {
    assert_matches("p/**/.x", COMPOUND_BODY, COMPOUND_NAMES, &["compound"]);
}

#[test]
fn selector_comments_real_space_still_separates_descendants() {
    for selector in ["p /**/.x", "p/**/ .x", "p/* note */\t.x", "p\n/**/.x"] {
        assert_matches(selector, COMPOUND_BODY, COMPOUND_NAMES, &["descendant"]);
    }
}

#[test]
fn selector_comments_internal_whitespace_does_not_create_a_combinator() {
    for selector in ["p/* \t\n\r\u{c} */.x", "p/*😺 space\n*/.x", "p/**//**/.x"] {
        assert_matches(selector, COMPOUND_BODY, COMPOUND_NAMES, &["compound"]);
    }
}

#[test]
fn selector_comments_do_not_join_identifier_tokens() {
    for selector in ["p/**/span", ".x/**/y", "[data-key=wo/**/rd]", "[data/**/-key=word]"] {
        assert!(!valid_query_selector(selector), "{selector}");
        let doc = document(&format!("{selector}{{width:37px}}"), COMPOUND_BODY);
        let styles = compute(&doc, &HashMap::new());
        for name in COMPOUND_NAMES {
            assert_ne!(styles[id(&doc, name)].width, Length::Px(37.0), "{selector}: {name}");
        }
    }
}

#[test]
fn selector_comments_class_and_pseudo_delimiters_keep_their_tokens() {
    assert_matches("./**/x", COMPOUND_BODY, COMPOUND_NAMES, COMPOUND_NAMES);
    assert_matches(
        "p/**/:/**/empty",
        "<p id=empty></p><p id=text>kept</p>",
        &["empty", "text"],
        &["empty"],
    );
}

#[test]
fn selector_comments_hash_and_function_tokens_cannot_be_manufactured() {
    for selector in ["#/**/target", "p#/**/target", ":not/**/(.x)", ":nth-child/**/(1)"] {
        assert!(!valid_query_selector(selector), "{selector}");
        let doc = document(&format!("{selector}{{width:37px}}"), "<p id=target class=x></p>");
        assert_ne!(compute(&doc, &HashMap::new())[id(&doc, "target")].width, Length::Px(37.0));
    }
}

#[test]
fn selector_comments_attribute_delimiters_and_flags_match_exact_values() {
    let body = "<p id=target data-key=word></p><p id=other data-key=other></p>";
    for selector in [
        "[/**/data-key/**/=/**/word/**/]",
        "[data-key=\"WORD\"/**/i]",
        "[data-key=\"word\"/**/s]",
        "[/**/data-key/**/]",
    ] {
        let wanted: &[&str] = if selector == "[/**/data-key/**/]" { &["target", "other"] } else { &["target"] };
        assert_matches(selector, body, &["target", "other"], wanted);
    }
}

#[test]
fn selector_comments_attribute_matchers_keep_separate_delimiter_tokens() {
    let body = "<p id=a data-key='word tail'></p><p id=b data-key=word-more></p><p id=c data-key=sword></p>";
    for (selector, wanted) in [
        ("[data-key~/**/=word]", vec!["a"]),
        ("[data-key|/**/=word]", vec!["b"]),
        ("[data-key^/**/=word]", vec!["a", "b"]),
        ("[data-key$/**/=word]", vec!["c"]),
        ("[data-key*/**/=word]", vec!["a", "b", "c"]),
    ] {
        assert_matches(selector, body, &["a", "b", "c"], &wanted);
    }
}

#[test]
fn selector_comments_do_not_erase_forbidden_real_whitespace() {
    for selector in [". /**/x", ": /**/empty", "[data-key~ /**/=word]", "[data-key^/**/ =word]"] {
        assert!(!valid_query_selector(selector), "{selector}");
    }
}

#[test]
fn selector_comments_inside_quoted_attributes_remain_literal_data() {
    let body = "<p id=a data-note='/*,)]{}*/'></p><p id=b data-note='a/*x*/b'></p><p id=c data-note='ab'></p>";
    assert_matches("[data-note='/*,)]{}*/']", body, &["a", "b", "c"], &["a"]);
    assert_matches("p/**/[data-note=\"a/*x*/b\"]", body, &["a", "b", "c"], &["b"]);
}

#[test]
fn selector_comments_hide_list_and_block_punctuation() {
    assert_matches(
        "p.x/*, #missing [ ) } {*/,/**/span.y",
        "<p id=a class=x></p><span id=b class=y></span><div id=c class=x></div>",
        &["a", "b", "c"],
        &["a", "b"],
    );
}

#[test]
fn selector_comments_invalid_members_reject_the_entire_list() {
    for selector in [".x,p/**/span", ".x,#/**/target", ".x,:not/**/(.missing)"] {
        let doc = document(&format!("{selector}{{width:37px}}"), "<p id=target class=x></p>");
        let mut work = 100_000;
        assert!(query_all(&doc, selector, &[id(&doc, "target")], None, &mut work).is_err());
        assert_eq!(work, 100_000, "invalid list must not start matching");
        assert_ne!(compute(&doc, &HashMap::new())[id(&doc, "target")].width, Length::Px(37.0));
    }
}

#[test]
fn selector_comments_nested_negation_ignores_comment_punctuation() {
    assert_matches(
        "p:not(/*[),{},*/.blocked,/**/:not(.x))",
        "<p id=a class=x></p><p id=b class='x blocked'></p><p id=c></p>",
        &["a", "b", "c"],
        &["a"],
    );
}

#[test]
fn selector_comments_negation_retains_argument_specificity() {
    let selector = ".x:not(/*, []*/#absent,.missing)";
    let doc = document(&format!("{selector}{{width:41px}}#target{{width:17px}}"), "<p id=target class=x></p>");
    let node = id(&doc, "target");
    let mut work = 100_000;
    assert_eq!(query_all(&doc, selector, &[node], None, &mut work).unwrap(), vec![node]);
    assert_eq!(compute(&doc, &HashMap::new())[node].width, Length::Px(41.0));
}

#[test]
fn selector_comments_structural_formulas_preserve_tokens_and_closing_parentheses() {
    let body = "<ul><li id=a></li><li id=b></li><li id=c></li><li id=d></li></ul>";
    for (selector, wanted) in [
        ("li:nth-child(2n/**/+/**/1)", vec!["a", "c"]),
        ("li:nth-child(2n/*) , []*/+1)", vec!["a", "c"]),
        ("li:nth-child(/**/odd/**/)", vec!["a", "c"]),
        ("li:nth-child(+/**/n)", vec!["a", "b", "c", "d"]),
        ("li:nth-child(+/**/n-/**/1)", vec!["a", "b", "c", "d"]),
    ] {
        assert_matches(selector, body, &["a", "b", "c", "d"], &wanted);
    }
}

#[test]
fn selector_comments_structural_token_fusion_remains_invalid() {
    for selector in [
        "li:nth-child(2/**/n+1)", "li:nth-child(o/**/dd)", "li:nth-child(-/**/n)",
        "li:nth-child(1/**/2)", "li:nth-child(+ /**/n)", "li:nth-child(+/**/ n)",
        "li:nth-child(2n of/**/.x)",
    ] {
        assert!(!valid_query_selector(selector), "{selector}");
    }
}

#[test]
fn selector_comments_structural_query_storage_is_admitted_before_parsing() {
    let doc = document("", "<ul><li id=a></li><li id=b></li><li id=c></li></ul>");
    let mut parents = vec![None; doc.nodes.len()];
    let mut slots = vec![usize::MAX; doc.nodes.len()];
    for (parent, entry) in doc.nodes.iter().enumerate() {
        for (slot, &child) in entry.children.iter().enumerate() {
            parents[child] = Some(parent);
            slots[child] = slot;
        }
    }
    let candidates = [id(&doc, "a"), id(&doc, "b"), id(&doc, "c")];
    let before = format!("{:?}", doc.nodes);
    let (mut work, mut copies) = (100_000, 0);
    let error = query_with_topology(&doc, "li:/**/nth-child(2n + 1)", candidates.to_vec(), None,
        &mut work, &parents, &slots, false, &mut copies).unwrap_err();
    assert!(error.contains("syntax copy allowance"), "{error}");
    assert_eq!(copies, 0);
    let (mut work, mut copies) = (100_000, 100_000);
    assert_eq!(query_with_topology(&doc, "li:/**/nth-child(2n + 1)", candidates.to_vec(), None,
        &mut work, &parents, &slots, false, &mut copies).unwrap(), vec![candidates[0], candidates[2]]);
    assert!(copies < 100_000);
    assert_eq!(format!("{:?}", doc.nodes), before);
}

#[test]
fn selector_comments_combinators_keep_topology_and_html_name_case() {
    let body = "<p id=parent><span id=child class=x></span></p><span id=adjacent class=x></span><b></b><span id=later class=x></span>";
    let names = &["parent", "child", "adjacent", "later"];
    assert_matches("P/**/>/**/SPAN./**/x", body, names, &["child"]);
    assert_matches("p/**/+/**/span.x", body, names, &["adjacent"]);
    assert_matches("p/**/~/**/span.x", body, names, &["adjacent", "later"]);
}

#[test]
fn selector_comments_non_ascii_names_keep_exact_spelling() {
    let body = "<p id=É class='猫 😺' 名字=é></p><p id=other class='猫 😸' 名字=É></p>";
    for selector in [".猫/**/.😺", "p/**/#É", "[名字/**/=/**/é]"] {
        assert_matches(selector, body, &["É", "other"], &["É"]);
    }
}

#[test]
fn selector_comments_eof_preserves_a_complete_selector_and_prior_rule() {
    let doc = document("p.x{width:19px}p.x/* unfinished {width:99px}", COMPOUND_BODY);
    let node = id(&doc, "compound");
    for selector in ["p.x/*tail", "p.x/**//*tail", "p.x/* \" ) , ["] {
        let mut work = 100_000;
        assert!(valid_query_selector(selector), "{selector}");
        assert_eq!(query_all(&doc, selector, &[node], None, &mut work).unwrap(), vec![node]);
    }
    for selector in ["/*tail", "/**/", "p[/*tail", ". /*tail"] {
        assert!(!valid_query_selector(selector), "{selector}");
    }
    assert_eq!(compute(&doc, &HashMap::new())[node].width, Length::Px(19.0));
}

#[test]
fn selector_comments_nested_openers_end_at_the_first_terminator() {
    for selector in ["p/* outer /* inner */.x", "p/*one*//*two*/.x"] {
        assert_matches(selector, COMPOUND_BODY, COMPOUND_NAMES, &["compound"]);
    }
}

#[test]
fn selector_comments_large_query_payload_does_not_consume_selector_syntax_limit() {
    let selector = format!("p/*{}*/.x", "x".repeat(2048));
    assert_matches(&selector, COMPOUND_BODY, COMPOUND_NAMES, &["compound"]);
    assert!(!valid_query_selector(&format!(".{}", "x".repeat(513))));
    assert!(!valid_query_selector(&format!("p/*{}*/.x", "x".repeat(8192))));
}

#[test]
fn selector_comments_large_stylesheet_payload_retains_raw_byte_admission() {
    let source = format!("p/*{}*/.x{{width:37px}}", "x".repeat(12_000));
    let limits = StylesheetLimits { sheet_bytes: source.len(), document_bytes: source.len() * 2, rules: 1 };
    let usage = limits.accept(&source, StylesheetUsage::default()).unwrap();
    assert_eq!((usage.bytes, usage.rules), (source.len(), 1));
    assert!(StylesheetLimits { sheet_bytes: source.len() - 1, ..limits }.accept(&source, StylesheetUsage::default()).is_err());
    let mut doc = document(&source, COMPOUND_BODY);
    doc.stylesheet_limits = limits;
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[id(&doc, "compound")].width, Length::Px(37.0));
    assert_ne!(styles[id(&doc, "descendant")].width, Length::Px(37.0));
}

#[test]
fn selector_comments_nested_media_walk_ignores_comment_braces() {
    let source = "@media/**/screen{/* } { */p/**/.x{width:37px}@media (min-width:700px){p/**/.x{width:49px}}}@media print{p/**/.x{width:99px}}";
    let mut doc = document(source, COMPOUND_BODY);
    for (viewport, expected) in [(600.0, 37.0), (800.0, 49.0)] {
        doc.media_environment = Some(MediaEnvironment { width: viewport, height: 600.0, hover: Hover::Hover });
        let styles = compute(&doc, &HashMap::new());
        assert_eq!(styles[id(&doc, "compound")].width, Length::Px(expected));
        assert_eq!(styles[id(&doc, "descendant")].width, Length::Auto);
    }
}

#[test]
fn selector_comments_external_cache_matches_fresh_computation_after_source_changes() {
    let doc = crate::html::parse(&format!("<link id=sheet rel=stylesheet>{COMPOUND_BODY}"));
    let sheet = id(&doc, "sheet");
    let node = id(&doc, "compound");
    let child = id(&doc, "descendant");
    let mut external = HashMap::from([(sheet, "p/*first*/.x{width:37px}".to_owned())]);
    let first = compute_with_environment_and_properties_cached(&doc, &external, None, None, None);
    assert_eq!(first.styles[node].width, Length::Px(37.0));
    for (source, parent, descendant) in [
        ("p/*changed [,})*/.x{width:37px}", Length::Px(37.0), Length::Auto),
        ("p/*changed [,})*/ .x{width:37px}", Length::Auto, Length::Px(37.0)),
    ] {
        external.insert(sheet, source.to_owned());
        let cached = compute_with_environment_and_properties_cached(&doc, &external, None, None, first.stylesheet_cache.as_ref());
        let fresh = compute_with_environment_and_properties(&doc, &external, None, None);
        assert_eq!((cached.styles[node].width, cached.styles[child].width), (parent, descendant));
        assert_eq!((fresh.styles[node].width, fresh.styles[child].width), (parent, descendant));
    }
}

#[test]
fn selector_comments_query_refusals_preserve_document_and_fresh_recovery() {
    let doc = document("", COMPOUND_BODY);
    let before = format!("{:?}", doc.nodes);
    let candidates = [id(&doc, "compound"), id(&doc, "descendant")];
    let mut work = 100_000;
    assert!(query_all(&doc, ".x,p/**/span", &candidates, None, &mut work).is_err());
    assert_eq!(work, 100_000);
    let mut work = 0;
    assert!(query_all(&doc, "p/**/.x", &candidates, None, &mut work).is_err());
    assert_eq!(work, 0);
    let (mut work, mut copies) = (100_000, 100_000);
    let error = query_all_with_storage(&doc, "p/**/.x", &candidates, None, &mut work, 0, None, &mut copies).unwrap_err();
    assert!(error.contains("retained selector result allowance"), "{error}");
    let mut work = 100_000;
    assert_eq!(query_all(&doc, "p/**/.x", &candidates, None, &mut work).unwrap(), vec![candidates[0]]);
    assert_eq!(format!("{:?}", doc.nodes), before);
}

#[test]
fn selector_comments_rule_limit_refusal_does_not_publish_a_prefix() {
    let source = "p/**/.x{width:37px}@media screen{span/**/.x{height:19px}}";
    let limits = StylesheetLimits { sheet_bytes: 4096, document_bytes: 8192, rules: 2 };
    assert!(limits.accept(source, StylesheetUsage::default()).is_err());
    let mut doc = crate::html::parse(&format!("<style>{source}</style><style>p.x{{width:17px}}</style>{COMPOUND_BODY}"));
    doc.stylesheet_limits = limits;
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[id(&doc, "compound")].width, Length::Px(17.0));
    assert_eq!(styles[id(&doc, "descendant")].height, Length::Auto);
    assert_eq!(stylesheet_usage(&doc, &HashMap::new()).rules, 1);
    assert_eq!(stylesheet_diagnostics(&doc, &HashMap::new()).len(), 1);
}

#[test]
fn selector_comments_leave_declaration_token_and_quoted_value_controls_intact() {
    let doc = document("p/**/.x{width:37px;color:blue;color:r/**/ed;--literal:\"a/*keep*/b\";font-family:'A/*keep*/B'}", COMPOUND_BODY);
    let node = id(&doc, "compound");
    let result = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    assert_eq!(result.styles[node].width, Length::Px(37.0));
    assert_eq!(result.styles[node].color, 0x0000ff);
    assert_eq!(result.styles[node].font_families, ["A/*keep*/B"]);
    assert_eq!(result.computed_properties.read(node, "--literal"), Ok(Some("\"a/*keep*/b\"")));
}
