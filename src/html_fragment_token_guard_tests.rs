//! Native token assertions shared with the innerHTML consumer fixture.
use super::*;

pub(crate) fn assert_supported_fragment_token_recovery() {
    for context in ["div", "template"] {
        for scripting in [false, true] {
            let bogus = parse_fragment("<!bogus>", context, scripting).unwrap();
            assert_eq!(bogus.nodes.len(), 2);
            assert_eq!(bogus.nodes[0].children, [1]);
            assert!(matches!(&bogus.nodes[1].kind, NodeKind::Comment(value) if value == "bogus"));

            let empty_end = parse_fragment("</>", context, scripting).unwrap();
            assert_eq!(empty_end.nodes.len(), 1);
            assert!(empty_end.nodes[0].children.is_empty());
            let invalid_end = parse_fragment("</!>", context, scripting).unwrap();
            assert_eq!(invalid_end.nodes.len(), 2);
            assert_eq!(invalid_end.nodes[0].children, [1]);
            assert!(matches!(&invalid_end.nodes[1].kind, NodeKind::Comment(value) if value == "!"));

            for (source, expected) in [("<img! src=x>", "img!"), ("<imgé src=x>", "imgé")] {
                let doc = parse_fragment(source, context, scripting).unwrap();
                assert_eq!(doc.nodes.len(), 2);
                assert_eq!(doc.nodes[0].children, [1]);
                assert_eq!(doc.nodes[1].namespace, Namespace::Html);
                assert!(matches!(&doc.nodes[1].kind, NodeKind::Element { tag, attrs, .. }
                    if tag == expected && attrs.len() == 1));
                assert_eq!(doc.attr(1, "src"), Some("x"));
            }
            let vt = parse_fragment("<img\u{b}src=x>", context, scripting).unwrap();
            assert_eq!(vt.nodes.len(), 2);
            assert_eq!(vt.nodes[0].children, [1]);
            assert!(matches!(&vt.nodes[1].kind, NodeKind::Element { tag, attrs, .. }
                if tag == "img\u{b}src=x" && attrs.is_empty()));
            assert_eq!(vt.attr(1, "src"), None);

            let equals = parse_fragment("<img =src=x>", context, scripting).unwrap();
            assert_eq!(equals.nodes.len(), 2);
            assert_eq!(equals.nodes[0].children, [1]);
            assert!(matches!(&equals.nodes[1].kind, NodeKind::Element { tag, attrs, .. }
                if tag == "img" && attrs.len() == 1));
            assert_eq!(equals.attr(1, "src"), None);
            assert_eq!(equals.attr(1, "=src"), Some("x"));

            let script = parse_fragment("<script><!--<script></script><img src=x></script>", context, scripting).unwrap();
            assert_eq!(script.nodes.len(), 3);
            assert_eq!(script.nodes[0].children, [1]);
            assert_eq!(script.nodes[1].children, [2]);
            assert!(matches!(&script.nodes[1].kind, NodeKind::Element { tag, .. } if tag == "script"));
            assert!(matches!(&script.nodes[2].kind, NodeKind::Text(value)
                if value == "<!--<script></script><img src=x>"));
        }
    }
}

#[test]
fn former_fragment_refusals_retain_actual_names_attributes_and_tokens() {
    assert_supported_fragment_token_recovery();
}
