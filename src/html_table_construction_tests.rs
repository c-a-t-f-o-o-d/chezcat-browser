use super::*;

#[test]
fn table_construction_closing_template_unwinds_its_unfinished_table() {
    for content in ["<table>", "<table><tr>", "<table><td>", "<table><colgroup>"] {
        let source = format!("<template>{content}</template><body><span>outside");
        let doc = parse_loaded_document(&source, &|| Ok(())).unwrap();
        assert_eq!(child_tags(&doc, first(&doc, "head")), ["template"]);
        assert_eq!(child_tags(&doc, first(&doc, "template")), ["table"]);
        assert_eq!(child_tags(&doc, first(&doc, "body")), ["span"]);
        assert_eq!(child_tags(&doc, first(&doc, "span")), ["#outside"]);
    }
}

fn child_tags(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent]
        .children
        .iter()
        .map(|&node| match &doc.nodes[node].kind {
            NodeKind::Element { tag, .. } => tag.clone(),
            NodeKind::Text(data) => format!("#{data}"),
            NodeKind::Comment(data) => format!("!{data}"),
            NodeKind::ProcessingInstruction { target, data } => format!("?{target}:{data}"),
            _ => panic!("unexpected table child"),
        })
        .collect()
}
fn first(doc: &Document, tag: &str) -> usize {
    doc.nodes
        .iter()
        .position(|n| matches!(&n.kind,NodeKind::Element{tag:name,..} if name==tag))
        .unwrap()
}

#[test]
fn table_construction_inserts_sections_rows_and_cells_and_keeps_pi_in_row() {
    let source = "<table>A<tr><?something><!--row--><td>B<th>C<tr><td>D<thead><th>E</table>tail";
    let check = |doc: &Document| {
        assert_eq!(
            child_tags(doc, first(doc, "body")),
            ["#A", "table", "#tail"]
        );
        assert_eq!(child_tags(doc, first(doc, "table")), ["tbody", "thead"]);
        let section = first(doc, "tbody");
        assert_eq!(child_tags(doc, section), ["tr", "tr"]);
        let row = doc.nodes[section].children[0];
        assert_eq!(child_tags(doc, row), ["?something:", "!row", "td", "th"]);
        assert_eq!(child_tags(doc, doc.nodes[row].children[2]), ["#B"]);
        assert_eq!(child_tags(doc, doc.nodes[row].children[3]), ["#C"]);
        assert_eq!(child_tags(doc, doc.nodes[section].children[1]), ["td"]);
        let head = first(doc, "thead");
        assert_eq!(child_tags(doc, head), ["tr"]);
        assert_eq!(child_tags(doc, doc.nodes[head].children[0]), ["th"]);
    };
    check(&parse_loaded_document(source, &|| Ok(())).unwrap());
    check(&parse_inert_html(source, 100, &|| Ok(())).unwrap().0);
}

#[test]
fn table_construction_fosters_pending_character_groups_and_elements_in_source_order() {
    let doc = parse_loaded_document(
        "<main>pre<table> \t&#65;<1<i>X</i>B<!--stop-->\n<tr> \t<td>cell</table>end</main>",
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        child_tags(&doc, first(&doc, "main")),
        ["#pre \tA<1", "i", "#B", "table", "#end"]
    );
    assert_eq!(child_tags(&doc, first(&doc, "i")), ["#X"]);
    assert_eq!(
        child_tags(&doc, first(&doc, "table")),
        ["!stop", "#\n", "tbody"]
    );
    assert_eq!(child_tags(&doc, first(&doc, "tr")), ["# \t", "td"]);
}

#[test]
fn table_construction_caption_columns_nested_tables_and_scoped_end_tags() {
    let doc=parse_loaded_document("<main><table><caption>label<col><colgroup> \tX</main><tbody><td>outer<table><td>inner</table>tail</table></main>",&||Ok(())).unwrap();
    let table = first(&doc, "table");
    assert_eq!(child_tags(&doc, first(&doc, "main")), ["#X", "table"]);
    assert_eq!(
        child_tags(&doc, table),
        ["caption", "colgroup", "colgroup", "tbody"]
    );
    assert_eq!(child_tags(&doc, first(&doc, "caption")), ["#label"]);
    assert_eq!(child_tags(&doc, doc.nodes[table].children[1]), ["col"]);
    assert_eq!(child_tags(&doc, doc.nodes[table].children[2]), ["# \t"]);
    let cell = first(&doc, "td");
    assert_eq!(child_tags(&doc, cell), ["#outer", "table", "#tail"]);
    assert_eq!(child_tags(&doc, doc.nodes[cell].children[1]), ["tbody"]);
}

#[test]
fn table_construction_capacity_cancellation_and_inert_boundaries() {
    let source = "<table>A<td>B";
    assert_eq!(
        parse_inert_html(source, 10, &|| Ok(()))
            .unwrap()
            .0
            .nodes
            .len(),
        10
    );
    for n in 4..10 {
        assert!(
            parse_inert_html(source, n, &|| Ok(()))
                .unwrap_err()
                .contains("node limit")
        );
    }
    assert!(
        parse_loaded_document(source, &|| Err("cancelled".into()))
            .unwrap_err()
            .contains("cancelled")
    );
    let doc = parse_loaded_document(
        "<table><template></table><span>held</span></template><tr><td>visible</table>",
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(child_tags(&doc, first(&doc, "template")), ["span"]);
    assert_eq!(
        child_tags(&doc, first(&doc, "table")),
        ["template", "tbody"]
    );
    assert!(parse_fragment(source, "div", false).is_err());
}
