use super::*;

fn kids(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent]
        .children
        .iter()
        .map(|&n| match &doc.nodes[n].kind {
            NodeKind::Element { tag, .. } => tag.clone(),
            NodeKind::Text(data) => format!("#{data}"),
            NodeKind::Comment(data) => format!("!{data}"),
            _ => panic!("unexpected child"),
        })
        .collect()
}
fn first(doc: &Document, name: &str) -> usize {
    doc.nodes
        .iter()
        .position(|n| matches!(&n.kind,NodeKind::Element{tag,..} if tag==name))
        .unwrap()
}

#[test]
fn active_formatting_reconstructs_original_attributes_and_distinct_native_siblings() {
    let source = "<p><b id=marker data-a=original><i>A</p><!--leave-->B</b>C</i>D";
    for doc in [
        parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 100, &|| Ok(())).unwrap().0,
    ] {
        let body = first(&doc, "body");
        assert_eq!(kids(&doc, body), ["p", "!leave", "b", "i", "#D"]);
        let old = first(&doc, "b");
        let new = doc.nodes[body].children[2];
        assert_ne!(old, new);
        assert_eq!(doc.attr(new, "data-a"), Some("original"));
        assert_eq!(doc.attr(new, "id"), Some("marker"));
        assert_eq!(kids(&doc, old), ["i"]);
        assert_eq!(kids(&doc, new), ["i"]);
        assert_eq!(kids(&doc, doc.nodes[old].children[0]), ["#A"]);
        assert_eq!(kids(&doc, doc.nodes[new].children[0]), ["#B"]);
        assert_eq!(kids(&doc, doc.nodes[body].children[3]), ["#C"]);
    }
}
#[test]
fn active_formatting_isolated_markers_and_foster_placement() {
    let doc =
        parse_loaded_document("<table><b>A<tr><td>X<i>Y<td>Z</table>after", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["b", "table", "b"]);
    assert_eq!(kids(&doc, doc.nodes[body].children[0]), ["#A"]);
    assert_eq!(kids(&doc, doc.nodes[body].children[2]), ["#after"]);
    let row = first(&doc, "tr");
    assert_eq!(kids(&doc, row), ["td", "td"]);
    assert_eq!(kids(&doc, doc.nodes[row].children[0]), ["#X", "i"]);
    assert_eq!(kids(&doc, doc.nodes[row].children[1]), ["#Z"]);
    let doc = parse_loaded_document("<table><b>A<tr>B<!--here--></table>", &|| Ok(())).unwrap();
    assert_eq!(kids(&doc, first(&doc, "body")), ["b", "b", "table"]);
    let b = doc.nodes[first(&doc, "body")].children[1];
    assert_eq!(kids(&doc, b), ["#B", "!here"]);
}
#[test]
fn active_formatting_limits_equivalent_entries_with_unordered_attributes() {
    let doc = parse_loaded_document(
        "<p><b x=1 y=2><b y=2 x=1><b x=1 y=2><b y=2 x=1><b x=1 y=3>A</p>B",
        &|| Ok(()),
    )
    .unwrap();
    let mut chain = doc.nodes[first(&doc, "body")].children[1];
    let mut values = Vec::new();
    while let NodeKind::Element { .. } = &doc.nodes[chain].kind {
        values.push(doc.attr(chain, "y").unwrap());
        chain = doc.nodes[chain].children[0];
    }
    assert_eq!(values, ["2", "2", "2", "3"]);
    assert!(matches!(&doc.nodes[chain].kind,NodeKind::Text(t) if t=="B"));
}
#[test]
fn active_formatting_charges_clones_and_preserves_checked_fragment_refusal() {
    let source = "<p><b>X</p>Y";
    let doc = parse_inert_html(source, 9, &|| Ok(())).unwrap().0;
    assert_eq!(doc.nodes.len(), 9);
    assert!(
        parse_inert_html(source, 8, &|| Ok(()))
            .unwrap_err()
            .contains("node limit")
    );
    assert!(parse_fragment("<b><i></b>", "div", false).is_err());
    let mut doc = parse_loaded_document("<p><b data-x=1234>A", &|| Ok(())).unwrap();
    let b = first(&doc, "b");
    let body = first(&doc, "body");
    let mut fmt = Formatting::default();
    fmt.opened(&doc, &[0, body, b], b).unwrap();
    fmt.copied = crate::javascript::limits::BROWSER.host.retained_bytes;
    let before = doc.nodes.len();
    let mut stack = vec![0, body];
    assert!(
        fmt.reconstruct(&mut doc, &mut stack, 100, None)
            .unwrap_err()
            .contains("storage limit")
    );
    assert_eq!(doc.nodes.len(), before);
    assert_eq!(stack, [0, body]);
}

#[test]
fn active_formatting_marker_survives_unrelated_table_pop_until_explicit_clear() {
    let doc = parse_loaded_document("<a><table><marquee></table><a>", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["a"]);
    let outer = doc.nodes[body].children[0];
    assert_eq!(kids(&doc, outer), ["marquee", "table", "a"]);
    let doc = parse_loaded_document("<a><marquee><i>held</marquee><a>outside", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["a", "a"]);
    assert_eq!(kids(&doc, doc.nodes[body].children[1]), ["#outside"]);
}

#[test]
fn active_formatting_nested_nobr_uses_open_element_scope_after_reconstruction() {
    let doc = parse_loaded_document("<nobr><table><marquee></table><nobr>", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["nobr", "nobr"]);
    assert_eq!(
        kids(&doc, doc.nodes[body].children[0]),
        ["marquee", "table"]
    );
    let doc = parse_loaded_document("<p><nobr>old</p><nobr>new", &|| Ok(())).unwrap();
    let body = first(&doc, "body");
    assert_eq!(kids(&doc, body), ["p", "nobr", "nobr"]);
    assert_eq!(
        kids(&doc, doc.nodes[body].children[1]),
        Vec::<String>::new()
    );
    assert_eq!(kids(&doc, doc.nodes[body].children[2]), ["#new"]);
}
