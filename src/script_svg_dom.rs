//! Namespace-preserving passive SVG edges and shared transfer admission.
use super::*;
impl Bridge<'_> {
    fn svg_node_facts(&self, object: u32) -> Option<(crate::html::Namespace, &str, bool)> {
        if let Some(source) = self.inert_import_source(object) {
            return Some(match source {
                script_inert_documents::ImportSource::Parsed { node, .. } => match &node.kind {
                    NodeKind::Element { tag, .. } => (node.namespace, tag.as_str(), false),
                    NodeKind::Text(text) | NodeKind::Cdata(text) => (
                        crate::html::Namespace::None,
                        "",
                        text.chars()
                            .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')),
                    ),
                    _ => (crate::html::Namespace::None, "", true),
                },
                script_inert_documents::ImportSource::Element(tag) => {
                    (crate::html::Namespace::Html, tag, false)
                }
                script_inert_documents::ImportSource::Text(text) => (
                    crate::html::Namespace::None,
                    "",
                    text.chars()
                        .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')),
                ),
                _ => (crate::html::Namespace::None, "", true),
            });
        }
        let index = node_index(object)?;
        let node = self.doc.nodes.get(index)?;
        Some(match &node.kind {
            NodeKind::Element { tag, .. } => (node.namespace, tag.as_str(), false),
            NodeKind::Text(text) | NodeKind::Cdata(text) => (
                crate::html::Namespace::None,
                "",
                text.chars()
                    .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')),
            ),
            _ => (crate::html::Namespace::None, "", true),
        })
    }
    pub(super) fn validate_svg_edge(&mut self, parent: u32, child: u32) -> Result<(), String> {
        self.state.work(1)?;self.validate_svg_activation_edge(parent,child)
    }
    pub(super) fn validate_svg_activation_edge(&self,parent:u32,child:u32)->Result<(),String>{
        let (pns, ptag, _) = self
            .svg_node_facts(parent)
            .ok_or("SVG parent identity is unavailable.")?;
        let (cns, ctag, whitespace) = self
            .svg_node_facts(child)
            .ok_or("SVG child identity is unavailable.")?;
        if pns == crate::html::Namespace::Svg {
            if !crate::html::svg_dom_name(ptag) {
                return Err("Unsupported SVG parent provenance.".into());
            }
            if cns == crate::html::Namespace::None && whitespace {
                return Ok(());
            }
            if cns != crate::html::Namespace::Svg || !crate::html::svg_dom_name(ctag) {
                return Err(
                    "SVG containers accept supported SVG shapes and whitespace/comments only."
                        .into(),
                );
            }
            let valid = match ptag {
                "linearGradient" | "radialGradient" => ctag == "stop",
                "svg" | "g" | "defs" | "mask" | "clipPath" | "symbol" => true,
                _ => false,
            };
            if !valid {
                return Err(
                    "SVG leaf or definition child shape is outside the supported hierarchy.".into(),
                );
            }
        } else if cns == crate::html::Namespace::Svg {
            let fragment =
                self.fragment_receiver(parent) || self.inert_node_type(parent) == Some(11);
            if !fragment && (pns != crate::html::Namespace::Html || ctag != "svg") {
                return Err("Only a real SVG root may enter ordinary HTML flow; SVG fragments remain data until wrapped.".into());
            }
        }
        Ok(())
    }
    pub(super) fn validate_svg_node(&mut self, node: usize) -> Result<(), String> {
        let item = &self.doc.nodes[node];
        if item.namespace != crate::html::Namespace::Svg {
            return Ok(());
        }
        let work = if let NodeKind::Element { attrs, .. } = &item.kind {
            attrs.iter().fold(attrs.len() + 1, |n, a| {
                n.saturating_add(a.name.bytes())
                    .saturating_add(a.value.len())
            })
        } else {
            1
        };
        self.state.work(work)?;
        if !crate::html::svg_dom_transfer(&self.doc.nodes[node]) {
            return Err("SVG transfer requires supported passive namespace/attributes and local references.".into());
        }
        if let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind {
            for attr in attrs {
                if attr.name.html_is("style") {
                    validate_style(&attr.value, self.state.limits.host.string_bytes)?;
                }
            }
        }
        Ok(())
    }
}
