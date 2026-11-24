use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}

fn topology(doc: &Document) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut parents = vec![None; doc.nodes.len()];
    let mut slots = vec![usize::MAX; doc.nodes.len()];
    for (parent, entry) in doc.nodes.iter().enumerate() {
        for (slot, &child) in entry.children.iter().enumerate() {
            parents[child] = Some(parent);
            slots[child] = slot;
        }
    }
    (parents, slots)
}

#[test]
fn selector_comment_boundaries_ordinary_names_keep_noncompacting_zero_copy_queries() {
    let doc = crate::html::parse("<p id=nth-control class=empty data-empty></p>");
    let target = id(&doc, "nth-control");
    let (parents, slots) = topology(&doc);
    for selector in [".empty", "#nth-control", "[data-empty]"] {
        let (mut work, mut copies) = (100_000, 0);
        assert_eq!(
            query_with_topology(&doc, selector, vec![target], None, &mut work,
                &parents, &slots, false, &mut copies).unwrap(),
            vec![target],
            "{selector}"
        );
        assert!(work < 100_000);
        assert_eq!(copies, 0);
    }
}

#[test]
fn selector_comment_boundaries_structural_suffixes_prepay_raw_source_after_quoted_comments() {
    let doc = crate::html::parse("<p id=target data-note='/*'></p>");
    let target = id(&doc, "target");
    let (parents, slots) = topology(&doc);
    for selector in [
        "p:/**/nth-child(1)".to_owned(),
        "[data-note='/*']:/**/nth-child(1)".to_owned(),
        format!("[data-note='/*']:/*{}*/nth-child(1)", "x".repeat(96)),
    ] {
        assert!(valid_query_selector(&selector), "{selector}");
        let source_work = selector.len() * 4;
        let source_copies = selector.len() * 64;
        let (mut work, mut copies) = (source_work - 1, 100_000);
        let error = query_with_topology(&doc, &selector, vec![target], None, &mut work,
            &parents, &slots, false, &mut copies).unwrap_err();
        assert!(error.contains("syntax work allowance"), "{selector}: {error}");
        assert_eq!((work, copies), (0, 100_000));

        let (mut work, mut copies) = (100_000, source_copies - 1);
        let error = query_with_topology(&doc, &selector, vec![target], None, &mut work,
            &parents, &slots, false, &mut copies).unwrap_err();
        assert!(error.contains("syntax copy allowance"), "{selector}: {error}");
        assert_eq!((work, copies), (100_000 - source_work, 0));

        let (mut work, mut copies) = (100_000, 100_000);
        assert_eq!(query_with_topology(&doc, &selector, vec![target], None, &mut work,
            &parents, &slots, false, &mut copies).unwrap(), vec![target]);
        assert_eq!(copies, 100_000 - source_copies);
    }
}

#[test]
fn selector_comment_boundaries_terminal_pseudo_elements_keep_strict_lists() {
    let doc = crate::html::parse(concat!(
        "<style>.x,::before/**/{width:37px}",
        ".x,::after/**/.bad{width:99px}",
        ".x,::before/**/>p{width:101px}</style>",
        "<p id=target class=x></p><p id=other></p>"
    ));
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[id(&doc, "target")].width, Length::Px(37.0));
    assert_eq!(styles[id(&doc, "other")].width, Length::Auto);

    let doc = crate::html::parse(concat!(
        "<style>.x,:/**/:/**/after/*tail*/{height:29px}",
        ".x,::after/**/ :hover{height:91px}</style>",
        "<p id=target class=x></p><p id=other></p>"
    ));
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[id(&doc, "target")].height, Length::Px(29.0));
    assert_eq!(styles[id(&doc, "other")].height, Length::Auto);
}

#[test]
fn selector_comment_boundaries_preserve_exact_spelling_capacity_and_outer_trim() {
    let at_limit = format!(".{}/*{}*/.y", "x".repeat(508), "payload".repeat(128));
    let over_limit = format!(".{}/*{}*/.y", "x".repeat(509), "payload".repeat(128));
    assert!(valid_query_selector(&at_limit));
    assert!(!valid_query_selector(&over_limit));
    assert!(!valid_query_selector(&format!(".{}/*one*//*two*/.y", "x".repeat(508))));

    let outer = format!("/*before*/ \t.{}/*after*/\n", "x".repeat(511));
    assert!(valid_query_selector(&outer));
    assert!(!valid_query_selector(&format!("/*before*/.{}/*after*/", "x".repeat(512))));

    let doc = crate::html::parse(&format!(
        "<style>{at_limit}{{width:37px}}.safe,{over_limit}{{width:99px}}</style>\
         <p id=target class='{} y safe'></p>", "x".repeat(508)
    ));
    assert_eq!(compute(&doc, &HashMap::new())[id(&doc, "target")].width, Length::Px(37.0));
}
