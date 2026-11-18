//! Original document-root identity and shared CSS/query matching controls.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn query(doc: &Document, source: &str, candidates: &[usize]) -> Vec<usize> {
    query_all(doc, source, candidates, None, &mut 100_000).unwrap()
}

#[test]
fn css_root_first_real_document_element_excludes_context_fragment_and_detached_roots() {
    let mut doc = crate::html::parse("<!--lead-->text<article id=first><div id=child><html id=nested></html></div></article><section id=second></section>");
    let first = id(&doc, "first");
    let child = id(&doc, "child");
    let second = id(&doc, "second");
    let nested = id(&doc, "nested");
    let detached = doc.nodes.len();
    doc.nodes.push(crate::html::Node {
        kind: NodeKind::Element { tag: "html".into(), id: Some("detached".into()), href: None, attrs: Vec::new() },
        children: Vec::new(), namespace: crate::html::Namespace::Html,
    });
    let candidates = [0, first, child, nested, second, detached];
    assert_eq!(query(&doc, "article", &candidates), vec![first]);
    assert_eq!(query(&doc, "html", &candidates), vec![nested, detached]);
    assert_eq!(query(&doc, ":root", &candidates), vec![first]);
    assert_eq!(query(&doc, ":ROOT", &candidates), vec![first]);
    assert_eq!(query(&doc, ":not(:root)", &[first, child, nested, second, detached]), vec![child, nested, second, detached]);
    assert_eq!(query(&doc, ":root > #child", &candidates), vec![child]);
    assert_eq!(query(&doc, ":root #nested", &candidates), vec![nested]);
    assert_eq!(query(&doc, ":root + #second", &candidates), vec![second]);
    assert!(query(&doc, "body :root", &candidates).is_empty());

    let mut fragment = crate::html::parse_fragment("<section id=fragment-top><span id=leaf></span></section>", "div", false).unwrap();
    let candidates: Vec<_> = (0..fragment.nodes.len()).collect();
    assert!(query(&fragment, ":root", &candidates).is_empty(), "A parser context Element is not a Document");
    fragment.nodes[0].kind = NodeKind::DocumentFragment;
    assert!(query(&fragment, ":root", &candidates).is_empty(), "A fragment's top-level Element is not a document element");
}

#[test]
fn css_root_specificity_groups_negation_and_combinators_share_the_real_cascade() {
    let doc = crate::html::parse(concat!(
        "<html id=root class=theme><head><style>",
        ":root{color:#112233;background:#aabbcc}html{color:#445566}.theme{background:#334455}",
        ":root,[data-bs-theme=light]{--ink:#123456}",
        ":root,:hover{color:red;--ink:red}",
        ":root > body > .target:not(:root){color:var(--ink)}",
        "</style></head><body id=body><div id=target class=target>Original</div></body></html>"
    ));
    let styles = compute(&doc, &HashMap::new());
    let root = id(&doc, "root");
    assert_eq!(styles[root].color, 0x112233, "Root pseudo-class outranks a later type selector");
    assert_eq!(styles[root].background, Some(0x334455), "A later class ties root's specificity");
    assert_eq!(styles[id(&doc, "target")].color, 0x123456, "Grouped theme variables reach an ordinary descendant through negation/combinators");
    for source in [":root", "html:root", ":root,[data-bs-theme=light]", ":not(:root)", ":not(:not(:root))"] {
        assert!(valid_query_selector(source), "{source}");
    }
    for source in [":root()", "::root", ":root,:hover", ":not(:root,:hover)", ":root >"] {
        assert!(!valid_query_selector(source), "{source}");
    }
    assert_eq!(query(&doc, ":not(:not(:root))", &[root, id(&doc, "body"), id(&doc, "target")]), vec![root]);
}

#[test]
fn css_root_shared_query_budget_failure_cannot_invert_a_root_match() {
    let doc = crate::html::parse("<html id=root><body id=body></body></html>");
    let root = id(&doc, "root");
    let mut work = doc.nodes.len() * 3 + 1;
    let error = query_first(&doc, ":not(:root)", &[root], None, &mut work).unwrap_err();
    assert!(error.contains("work limit"), "{error}");
    assert_eq!(work, 0);
    let mut work = 100_000;
    assert_eq!(query_first(&doc, ":root", &[0, id(&doc, "body"), root], None, &mut work).unwrap(), Some(root));
}
