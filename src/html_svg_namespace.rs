//! Provenance for balanced inline SVG shapes in ordinary HTML flow.
use super::{AttributeNamespace, Namespace, Node, NodeKind, Tag};

#[derive(Default)]
pub(super) struct Island {
    // Nodes stay unknown until the outer SVG end tag has been consumed.
    pub(super) root: Option<usize>,
    // The first still-open HTML ancestor whose insertion mode is unsupported.
    pub(super) blocked_depth: Option<usize>,
}

pub(super) fn flow_context(name: &str) -> bool {
    matches!(name, "html" | "body" | "main" | "article" | "section" | "aside" | "nav" |
        "header" | "footer" | "figure" | "figcaption" | "div" | "span" | "p" |
        "a" | "button" | "label" | "ul" | "ol" | "li")
}

pub(super) fn basic_element(name: &str) -> bool {
    matches!(name, "svg" | "g" | "circle" | "ellipse" | "line" | "path" | "polygon" | "polyline" | "rect")
}

pub(super) fn supported_tag(tag: &Tag) -> bool {
    tag.clean_syntax && basic_element(&tag.name) && tag.attrs.iter().all(|(key, value)| match key.as_str() {
        "id" | "class" | "width" | "height" | "x" | "y" | "cx" | "cy" | "r" | "rx" | "ry" |
        "x1" | "y1" | "x2" | "y2" | "d" | "points" | "stroke-width" => true,
        "fill" | "stroke" => value.strip_prefix('#').map_or_else(
            || !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphabetic()),
            |digits| matches!(digits.len(), 3 | 4 | 6 | 8) && digits.bytes().all(|b| b.is_ascii_hexdigit())),
        "xmlns" => tag.name == "svg" && Some(value.as_str()) == Namespace::Svg.element_uri(),
        _ => false,
    })
}

pub(super) fn publish(nodes: &mut [Node]) {
    // Arena allocation follows source order. Each completed island owns a
    // disjoint slice, including only its elements and whitespace text.
    for node in nodes {
        if let NodeKind::Element { attrs, .. } = &mut node.kind {
            node.namespace = Namespace::Svg;
            for attr in attrs {
                if attr.name.html_is("xmlns") {
                    attr.name.namespace = Some(AttributeNamespace {
                        uri: "http://www.w3.org/2000/xmlns/".into(), prefix: None, local: "xmlns".into(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::{parse, parse_fragment, Document, MAX_DEPTH, MAX_NODES};

    fn node<'a>(doc: &'a Document, id: &str) -> &'a Node {
        &doc.nodes[(0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).expect(id)]
    }

    #[test]
    fn svg_namespace_balanced_islands_publish_only_scoped_elements() {
        let doc = parse("<html><body><circle id=outside></circle><svg id=root xmlns='http://www.w3.org/2000/svg'>\n\
            <g id=group><circle id='a'/><rect id='b'/><svg id=nested><path id=p d='M1 2'/></svg></g></svg>\
            <svg id='empty'/><div id=after xmlns='http://www.w3.org/2000/svg'></div></body></html>");
        for id in ["root", "group", "a", "b", "nested", "p", "empty"] {
            assert_eq!(node(&doc, id).namespace, Namespace::Svg, "{id}");
        }
        for id in ["outside", "after"] { assert_eq!(node(&doc, id).namespace, Namespace::Html); }
        let group = node(&doc, "group");
        assert_eq!(group.children.len(), 3, "self-closing shapes must be siblings");
        let NodeKind::Element {attrs,..} = &node(&doc, "root").kind else { panic!("SVG root") };
        assert!(attrs.iter().any(|a| a.name.matches(Some("http://www.w3.org/2000/xmlns/"), "xmlns")));
        assert!(doc.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Text(_))).all(|n| n.namespace == Namespace::None));
        assert_eq!(Namespace::Svg.element_uri(), Some("http://www.w3.org/2000/svg"));
        assert!(Namespace::Svg.is_foreign() && Namespace::UnknownForeign.is_foreign());
        assert!(!Namespace::Html.is_foreign() && !Namespace::None.is_foreign());
    }

    #[test]
    fn svg_namespace_uncertain_content_cannot_publish_an_earlier_valid_prefix() {
        for bad in ["<foreignObject></foreignObject>", "<use></use>", "<script></script>", "<math></math>",
            "<circle onclick='observed()'></circle>", "<rect fill='url(#paint)'/>", "<g><circle></g>",
            "<g id='a'id='b'></g>", "<g//>", "<g></g/>", "</missing>", "<!--comment-->", "<![CDATA[x]]>"] {
            let doc = parse(&format!("<body><svg id=root><circle id='first'/>{bad}</svg><p id=next></p></body>"));
            assert_eq!(node(&doc, "root").namespace, Namespace::UnknownForeign, "{bad}");
            assert_eq!(node(&doc, "first").namespace, Namespace::UnknownForeign, "{bad}");
            assert_eq!(node(&doc, "next").namespace, Namespace::Html, "{bad}");
        }
        for markup in ["<svg id=root><g/>", "<svg id=root><g", "<svg id=root><circle id=unquoted/></svg>",
                       "<select><svg id=root></svg></select>", "<template><div><svg id=root></svg></div></template>",
                       "<math><svg id=root></svg></math>"] {
            assert_eq!(node(&parse(markup), "root").namespace, Namespace::UnknownForeign, "{markup}");
        }
        let doc = parse("<select><option></option></select><svg id='s'/>");
        assert_eq!(node(&doc, "s").namespace, Namespace::Svg);
    }

    #[test]
    fn svg_namespace_node_and_stack_limits_preserve_unknown_prefix_and_fragment_refusal() {
        let depth = format!("<svg id=root>{}{}", "<g>".repeat(MAX_DEPTH), "</g>".repeat(MAX_DEPTH));
        let doc = parse(&depth);
        assert_eq!(doc.nodes.len(), MAX_DEPTH);
        assert_eq!(node(&doc, "root").namespace, Namespace::UnknownForeign);
        let many = format!("<svg id=root>{}</svg>", "<circle/>".repeat(MAX_NODES));
        let doc = parse(&many);
        assert_eq!(doc.nodes.len(), MAX_NODES);
        assert!(doc.nodes[1..].iter().all(|n| n.namespace == Namespace::UnknownForeign));
        assert!(parse_fragment("<svg><circle/></svg>", "div", true).is_err());
        assert!(parse_fragment("<circle/>", "svg", false).is_err());
    }
}
