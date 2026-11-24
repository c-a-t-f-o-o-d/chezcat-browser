//! Original closing-token regression found during script-state integration review.
use super::*;

#[test]
fn script_end_tag_initial_equals_preserves_quoted_greater_than() {
    for closing in ["</script ==\">\">", "</script =='>x'>", "</script =name='a>b'>"] {
        let source = format!("<script>A{closing}<p>tail</p>");
        let mut documents = vec![
            parse_loaded_document(&source, &|| Ok(())).unwrap(),
            parse_inert_html(&source, 100, &|| Ok(())).unwrap().0,
        ];
        for context in ["div", "template"] {
            for scripting in [false, true] {
                documents.push(parse_fragment(&source, context, scripting).unwrap());
            }
        }
        for document in documents {
            let scripts: Vec<_> = document.nodes.iter().enumerate().filter_map(|(index, node)| {
                matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "script").then_some(index)
            }).collect();
            assert_eq!(scripts.len(), 1);
            let script = &document.nodes[scripts[0]];
            assert_eq!(script.children.len(), 1);
            assert!(matches!(&document.nodes[script.children[0]].kind, NodeKind::Text(text) if text == "A"));
            let all_text: String = document.nodes.iter().filter_map(|node| match &node.kind {
                NodeKind::Text(text) => Some(text.render_projection()),
                _ => None,
            }).collect();
            assert_eq!(all_text, "Atail", "closing token: {closing}");
        }
    }
}
