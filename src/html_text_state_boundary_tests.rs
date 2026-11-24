//! Original integration regressions for literal-text boundaries and provenance.
use super::*;

fn documents(source: &str) -> [Document; 2] {
    [parse_loaded_document(source, &|| Ok(())).unwrap(),
        parse_inert_html(source, 1000, &|| Ok(())).unwrap().0]
}

fn element(doc: &Document, name: &str) -> usize {
    let found: Vec<_> = doc.nodes.iter().enumerate().filter_map(|(index, node)| {
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == name).then_some(index)
    }).collect();
    assert_eq!(found.len(), 1, "expected one {name}");
    found[0]
}

fn text(doc: &Document, parent: usize) -> String {
    doc.nodes[parent].children.iter().map(|&child| match &doc.nodes[child].kind {
        NodeKind::Text(value) => value.render_projection(),
        other => panic!("unexpected non-text child: {other:?}"),
    }).collect()
}

#[test]
fn foreign_plaintext_preserves_outer_paragraph_and_foreign_parentage() {
    for foreign in ["svg", "math"] {
        for doc in documents(&format!("<p><{foreign}><plaintext>A</plaintext><g>B</g></{foreign}>C")) {
            let paragraph = element(&doc, "p");
            let foreign = element(&doc, foreign);
            let plaintext = element(&doc, "plaintext");
            let sibling = element(&doc, "g");
            assert_eq!(doc.nodes[paragraph].children[0], foreign);
            assert_eq!(doc.nodes[foreign].children, vec![plaintext, sibling]);
            assert_ne!(doc.nodes[plaintext].namespace, Namespace::Html);
            assert_eq!(text(&doc, plaintext), "A");
            assert_eq!(text(&doc, sibling), "B");
            assert!(matches!(&doc.nodes[*doc.nodes[paragraph].children.last().unwrap()].kind,
                NodeKind::Text(value) if value == "C"));
        }
    }
}

#[test]
fn rawtext_and_rcdata_end_tokens_use_exact_delimiters_and_eof_rules() {
    for name in ["style", "title", "textarea", "iframe", "xmp", "noembed", "noframes"] {
        for suffix in ["", "\u{000b}", "é", "x "] {
            let literal = format!("A</{name}{suffix}");
            for doc in documents(&format!("<{name}>{literal}")) {
                assert_eq!(text(&doc, element(&doc, name)), literal, "{name}, {suffix:?}");
            }
        }
        for tail in [" ", "/", " x='", " ==\"unclosed"] {
            for doc in documents(&format!("<{name}>A</{name}{tail}")) {
                assert_eq!(text(&doc, element(&doc, name)), "A", "{name}, {tail:?}");
            }
        }
        for tail in [" x=\u{000b}\">", " \u{000b}='>'>", " ==\">\">"] {
            for doc in documents(&format!("<{name}>A</{name}{tail}<p>tail</p>")) {
                assert_eq!(text(&doc, element(&doc, name)), "A", "{name}, {tail:?}");
                assert_eq!(text(&doc, element(&doc, "p")), "tail");
                let all: String = doc.nodes.iter().filter_map(|node| match &node.kind {
                    NodeKind::Text(value) => Some(value.render_projection()), _ => None,
                }).collect();
                assert_eq!(all, "Atail", "{name}, {tail:?}");
            }
        }
    }
    for doc in documents("<style>&amp;A\0B</style><p>tail</p>") {
        assert_eq!(text(&doc, element(&doc, "style")), "&amp;A\u{fffd}B");
        assert_eq!(text(&doc, element(&doc, "p")), "tail");
    }
    for context in ["div", "template"] {
        assert!(parse_fragment("<style>A</style =foo>", context, false).is_err());
        assert!(parse_fragment("<textarea>A</textarea x=y>", context, false).is_err());
    }
}
