//! CSSOM View offset parents from the renderer's actual tree and box evidence.
//! https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsetparent
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OffsetParent {
    Unavailable,
    None,
    Element(usize),
}

struct Frame {
    node: usize,
    ordinary: OffsetParent,
    static_target: OffsetParent,
    depth: usize,
}

/// Reserve the complete linear pass before a script-forced renderer rebuild.
/// Each node has a constant number of box/property reads and state updates;
/// each physical child edge is inspected once, without ancestor rescanning.
pub(crate) fn offset_parent_work(doc: &Document) -> usize {
    doc.nodes
        .iter()
        .fold(doc.nodes.len().saturating_mul(32), |work, node| {
            work.saturating_add(node.children.len().saturating_mul(4))
        })
}

/// Retained results plus the largest traversal stack and visited allocation.
/// A Vec<bool> uses no more than the conservatively charged byte per node.
pub(crate) fn offset_parent_storage(nodes: usize) -> usize {
    nodes.saturating_mul(
        std::mem::size_of::<OffsetParent>()
            + std::mem::size_of::<Frame>()
            + 2 * std::mem::size_of::<bool>(),
    )
}

impl Engine<'_> {
    pub(super) fn finish_offset_parents(&mut self) {
        let count = self.doc.nodes.len();
        if count == 0 {
            return;
        }
        let root = self.doc.nodes[0].children.iter().copied().find(|&node| {
            self.doc
                .nodes
                .get(node)
                .is_some_and(|node| matches!(node.kind, NodeKind::Element { .. }))
        });
        // HTML's body element is a direct child of the actual HTML root, not
        // an arbitrary element carrying a body tag elsewhere in the arena.
        let body = root.and_then(|root| self.doc.body_element(root));

        // Detached and inert elements have a proved lack of CSS boxes even
        // though they cannot be reached by the renderer traversal below.
        for node in 1..count {
            if self.layout.computed_properties.read_position(node) == Ok(None) {
                self.layout.offset_parents[node] = OffsetParent::None;
            }
        }
        let mut seen = vec![false; count];
        let mut stack = Vec::with_capacity(count);
        seen[0] = true;
        stack.push(Frame {
            node: 0,
            ordinary: OffsetParent::None,
            static_target: OffsetParent::None,
            depth: 0,
        });
        while let Some(Frame {
            node,
            mut ordinary,
            mut static_target,
            depth,
        }) = stack.pop()
        {
            let entry = &self.doc.nodes[node];
            let position = self.layout.computed_properties.read_position(node);
            let has_box = self.layout.client_boxes[node].is_some() || self.inline_boxes[node];
            let no_box = !has_box && !self.layout.offset_unavailable[node];
            if matches!(entry.kind, NodeKind::Element { .. }) && node != 0 {
                self.layout.offset_parents[node] =
                    if Some(node) == root || Some(node) == body || no_box || position == Ok(None) {
                        OffsetParent::None
                    } else if has_box {
                        match position {
                            Ok(Some(Position::Fixed)) => OffsetParent::None,
                            Ok(Some(Position::Static)) => static_target,
                            Ok(Some(_)) => ordinary,
                            _ => OffsetParent::Unavailable,
                        }
                    } else {
                        // Missing inline fragments, skipped replaced elements,
                        // or layout limits never establish a lack of CSS boxes.
                        OffsetParent::Unavailable
                    };

                let candidate = if has_box {
                    OffsetParent::Element(node)
                } else {
                    OffsetParent::Unavailable
                };
                // The implemented CSS position keywords establish absolute
                // containing blocks; none establishes a fixed containing
                // block. Author zoom, transforms and shadow trees are absent
                // from this renderer. Browser zoom is uniform for this tree.
                if Some(node) == body {
                    ordinary = candidate;
                    static_target = candidate;
                } else if entry.namespace != crate::html::Namespace::Html {
                    ordinary = OffsetParent::Unavailable;
                    static_target = OffsetParent::Unavailable;
                } else {
                    match position {
                        Ok(Some(Position::Static)) => {}
                        Ok(Some(_)) => {
                            ordinary = candidate;
                            static_target = candidate;
                        }
                        _ => {
                            ordinary = OffsetParent::Unavailable;
                            static_target = OffsetParent::Unavailable;
                        }
                    }
                    if matches!(&entry.kind, NodeKind::Element { tag, .. } if matches!(tag.as_str(), "td" | "th" | "table"))
                    {
                        static_target = candidate;
                    }
                }
            }
            if depth >= MAX_DEPTH {
                continue;
            }
            for &child in entry.children.iter().rev() {
                if child < count && !seen[child] && self.parents[child] == Some(node) {
                    seen[child] = true;
                    stack.push(Frame {
                        node: child,
                        ordinary,
                        static_target,
                        depth: depth + 1,
                    });
                }
            }
        }
    }
}
