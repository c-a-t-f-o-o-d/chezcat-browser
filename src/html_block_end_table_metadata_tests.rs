//! Native scripting-disabled NOSCRIPT is not an ordinary-scope boundary.
//! Supplemental original witnesses; table modes and generic fences stay intact.
use super::*;

fn tree(doc: &Document, node: usize) -> String {
    match &doc.nodes[node].kind {
        NodeKind::Element { tag, .. } => format!("<{tag}>{}</{tag}>", doc.nodes[node].children.iter().map(|&child| tree(doc, child)).collect::<String>()),
        NodeKind::Text(text) => text.as_str().to_owned(),
        _ => panic!("unexpected node in original table metadata witness"),
    }
}
fn assert_disabled_native(source: &str, expected: &str) {
    for doc in [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100, &|| Ok(())).unwrap().0] {
        let body = (1..doc.nodes.len()).find(|&n| matches!(&doc.nodes[n].kind, NodeKind::Element { tag, .. } if tag == "body")).unwrap();
        assert!(!doc.scripting_enabled);
        assert_eq!(tree(&doc, body), expected);
        let mut parents = vec![0; doc.nodes.len()];
        for node in &doc.nodes { for &child in &node.children { parents[child] += 1; } }
        assert!(parents[1..].iter().all(|&count| count == 1));
    }
}

#[test]
fn ordinary_end_disabled_noscript_under_table_keeps_following_fostered_text() {
    assert_disabled_native(
        "<body><table><section><noscript>A</section>B</noscript>C</table>D",
        "<body><section><noscript>A</noscript></section>BC<table></table>D</body>");
}

#[test]
fn ordinary_end_real_table_boundary_and_raw_iframe_keep_their_existing_roles() {
    assert_disabled_native(
        "<body><section><table><noscript>A</section>B</noscript>C</table>D</section>E",
        "<body><section><noscript>AB</noscript>C<table></table>D</section>E</body>");
    assert_disabled_native(
        "<body><table><section><iframe>A</section>B</iframe>C</section>D</table>E",
        "<body><section><iframe>A</section>B</iframe>C</section>D<table></table>E</body>");
}
