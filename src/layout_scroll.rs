//! Physical vertical scrolling areas, measured before paint clipping. The
//! receiver's overflow is retained; only propagation to ancestors is clipped.
use super::*;

impl Engine<'_> {
    pub(super) fn finish_scroll_heights(&mut self, fixed_boxes: &[bool]) {
        let mut heights: Vec<Option<i32>> = self
            .layout
            .client_boxes
            .iter()
            .map(|b| b.map(|b| b.height))
            .collect();
        let mut viewport = 0;
        for (node, record) in self.boxes.iter().enumerate() {
            let Some(b) = record else {
                continue;
            };
            let origin_y = b.y.saturating_add(b.border[0]);
            let flow = match &b.formatting {
                Some(FormattingContext::Flex(group)) => group
                    .items
                    .iter()
                    .map(|item| {
                        let margin = item
                            .nodes
                            .first()
                            .and_then(|node| self.used_margins.get(node))
                            .unwrap_or(&item.margin);
                        item.y
                            .saturating_add(item.height)
                            .saturating_add(margin[2].max(0))
                    })
                    .max()
                    .unwrap_or(0),
                Some(FormattingContext::Grid(group)) => group.track_end,
                None => b.natural_height,
            };
            let natural = b
                .height
                .max(flow)
                .saturating_add(b.padding[0])
                .saturating_add(b.padding[2]);
            // Normal-flow end padding belongs to this box, unlike overflow
            // propagated from a constrained descendant or positioned child.
            let area = ClipRect::new(
                b.x.saturating_add(b.border[3]),
                origin_y,
                b.width
                    .saturating_add(b.padding[1])
                    .saturating_add(b.padding[3]),
                natural,
            );
            self.scroll_contribution(
                node,
                area,
                true,
                fixed_boxes.get(node).copied().unwrap_or(false),
                &mut heights,
                &mut viewport,
            );
        }
        for (node, rect) in self.layout.border_rects.iter().enumerate() {
            let Some(rect) = rect else {
                continue;
            };
            let margin = self.used_margins.get(&node).copied().unwrap_or([0; 4]);
            let area = ClipRect::new(
                rect.x.saturating_sub(margin[3].max(0)),
                rect.y.saturating_sub(margin[0].max(0)),
                rect.width
                    .saturating_add(margin[3].max(0))
                    .saturating_add(margin[1].max(0)),
                rect.height
                    .saturating_add(margin[0].max(0))
                    .saturating_add(margin[2].max(0)),
            );
            self.scroll_contribution(node, area, false, rect.fixed, &mut heights, &mut viewport);
        }
        for (index, atom) in self.atoms.iter().enumerate() {
            if let AtomKind::Text(run) = atom.kind {
                if let Some(&node) = self.run_nodes.get(run) {
                    self.scroll_contribution(
                        node,
                        ClipRect::new(atom.x, atom.y, atom.width, atom.height),
                        false,
                        self.layout.fixed_hits.get(index).copied().unwrap_or(false),
                        &mut heights,
                        &mut viewport,
                    );
                }
            }
        }
        self.layout.scroll_heights = heights;
        for (node, style) in self.styles.iter().enumerate() {
            if style.display.flex_container()
                && style.flex_direction == FlexDirection::ColumnReverse
                && self.layout.client_boxes[node].is_some()
            {
                self.layout.scroll_heights[node] = None;
            }
        }
        self.layout.viewport_scroll_height = viewport;
        // Viewport overflow propagation is a distinct renderer operation.
        // Decline that root measurement rather than aliasing a clipped extent.
        if let Some(root) = self.doc.nodes.first().and_then(|n| {
            n.children
                .iter()
                .copied()
                .find(|node| matches!(self.doc.nodes[*node].kind, NodeKind::Element { .. }))
        }) {
            if self.layout.client_boxes[root].is_some() {
                let overflow = self.styles[root].overflow;
                self.layout.viewport_scroll_unavailable = overflow.clips_x() || overflow.clips_y();
                if !self.layout.viewport_scroll_unavailable
                    && self.doc.nodes[root].namespace == crate::html::Namespace::Html
                    && matches!(&self.doc.nodes[root].kind,NodeKind::Element {tag,..} if tag=="html")
                {
                    if let Some(body)=self.doc.nodes[root].children.iter().copied().find(|node|self.doc.nodes[*node].namespace==crate::html::Namespace::Html && matches!(&self.doc.nodes[*node].kind,NodeKind::Element {tag,..} if tag=="body")){
                        let overflow = self.styles[body].overflow;
                        self.layout.viewport_scroll_unavailable = self.layout.client_boxes[body].is_some()
                            && (overflow.clips_x() || overflow.clips_y());
                    }
                }
            }
        }
    }

    fn scroll_contribution(
        &self,
        node: usize,
        mut area: ClipRect,
        own: bool,
        fixed: bool,
        heights: &mut [Option<i32>],
        viewport: &mut i32,
    ) {
        let mut current = if own {
            Some(node)
        } else {
            self.clip_parent(node)
        };
        for _ in 0..MAX_DEPTH {
            let Some(parent) = current else {
                if !fixed && area.right >= 0 && area.bottom >= 0 {
                    *viewport = (*viewport).max(area.bottom);
                }
                return;
            };
            if let Some(rect) = self.layout.border_rects[parent] {
                if let Some(client) = self.layout.client_boxes[parent] {
                    let left = rect.x.saturating_add(client.left);
                    let top = rect.y.saturating_add(client.top);
                    // Content entirely in an unreachable negative region must
                    // not create a positive scroll extent on the other axis.
                    if area.right >= left && area.bottom >= top {
                        if let Some(height) = &mut heights[parent] {
                            *height = (*height).max(area.bottom.saturating_sub(top));
                        }
                    }
                    let padding = ClipRect::new(left, top, client.width, client.height);
                    if let Some(bounds) = overflow_clip(padding, self.styles[parent].overflow) {
                        area = area.intersect(bounds);
                        if area.left >= area.right || area.top >= area.bottom {
                            return;
                        }
                    }
                }
            }
            current = self.clip_parent(parent);
        }
        if current.is_none() && !fixed && area.right >= 0 && area.bottom >= 0 {
            *viewport = (*viewport).max(area.bottom);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn measure(body: &str) -> (Document, Layout) {
        let doc = crate::html::parse(&format!(
            "<html><head><style>body{{margin:0}}</style></head><body>{body}</body></html>"
        ));
        let layout = build_viewport(&doc, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
        (doc, layout)
    }
    fn node(doc: &Document, id: &str) -> usize {
        (0..doc.nodes.len())
            .find(|n| doc.attr(*n, "id") == Some(id))
            .unwrap()
    }
    fn height(doc: &Document, layout: &Layout, id: &str) -> i32 {
        layout.scroll_heights[node(doc, id)].unwrap()
    }
    #[test]
    fn scroll_height_normal_flow_padding_and_nested_overflow_are_distinct() {
        for (inner,expected) in [("<div style='width:250px;height:150px'></div>",200),("<div style='width:60px;height:30px'><div style='width:250px;height:150px'></div></div>",150)] {
            let (doc,page)=measure(&format!("<div id=p style='overflow:hidden;width:200px;height:40px;padding-right:40px;padding-bottom:50px'>{inner}</div>"));
            assert_eq!(height(&doc,&page,"p"),expected);assert_eq!(page.viewport_scroll_height,90);
        }
        let (doc,page)=measure("<div id=p style='height:40px;padding-bottom:50px'><div style='height:30px'><div style='height:150px'></div></div></div>");
        assert_eq!(height(&doc, &page, "p"), 150);
        assert_eq!(page.viewport_scroll_height, 150);
    }
    #[test]
    fn scroll_height_relative_absolute_fixed_and_clipping_follow_containing_blocks() {
        for (position, expected, viewport) in [
            ("relative;top:300px", 340, 340),
            ("relative;top:-300px", 40, 40),
            ("fixed;top:1000px", 0, 0),
        ] {
            let (doc, page) = measure(&format!(
                "<div id=p><div style='position:{position};height:40px'></div></div>"
            ));
            assert_eq!(height(&doc, &page, "p"), expected, "{position}");
            assert_eq!(page.viewport_scroll_height, viewport, "{position}");
        }
        let (doc,page)=measure("<div id=p style='position:relative;height:40px'><div id=clip style='overflow:hidden;height:20px'><div style='position:absolute;top:300px;height:30px;width:20px'></div></div></div>");
        assert_eq!(height(&doc, &page, "p"), 330);
        assert_eq!(height(&doc, &page, "clip"), 20);
        assert_eq!(page.viewport_scroll_height, 330);
        let (doc,page)=measure("<div id=p style='height:40px'><div style='position:absolute;top:300px;height:30px;width:20px'></div></div>");
        assert_eq!(height(&doc, &page, "p"), 40);
        assert_eq!(page.viewport_scroll_height, 330);
    }

    #[test]
    fn overflow_axes_propagate_vertical_extent_only_within_the_horizontal_clip() {
        for (rule, child_left, expected_viewport) in [
            ("clip visible", 10, 120),
            ("clip visible", 90, 40),
            ("visible clip", 90, 40),
            ("visible", 90, 120),
        ] {
            let (doc, page) = measure(&format!(
                "<div id=p style='position:relative;width:80px;height:40px;overflow:{rule}'>\
                 <div style='position:absolute;left:{child_left}px;top:100px;width:20px;height:20px'></div></div>"
            ));
            assert_eq!(height(&doc, &page, "p"), 120, "{rule} at {child_left}");
            assert_eq!(page.viewport_scroll_height, expected_viewport, "{rule} at {child_left}");
            assert_eq!(page.height, expected_viewport + 16, "painted page and its tail for {rule} at {child_left}");
        }
    }
    #[test]
    fn scroll_height_own_margin_border_and_negative_regions_do_not_inflate() {
        let (doc,page)=measure("<div id=p style='margin:17px;border:3px solid black;padding:5px;height:40px'><div style='height:20px'></div></div>");
        assert_eq!(height(&doc, &page, "p"), 50);
        assert!(
            page.height > page.viewport_scroll_height,
            "The renderer tail is separate."
        );
        let (doc,page)=measure("<div id=p style='height:40px;position:relative'><div style='position:absolute;left:-300px;top:800px;width:10px;height:30px'></div></div>");
        assert_eq!(height(&doc, &page, "p"), 40);
        assert_eq!(page.viewport_scroll_height, 40);
    }
    #[test]
    fn scroll_height_flex_replaced_margins_and_nested_shrink_use_final_measurements() {
        for parent in [10, 40] {
            let (doc,page)=measure(&format!("<div id=p style='display:flex;flex-direction:column;height:{parent}px'><div style='height:1px;flex:none;margin-top:auto;margin-bottom:auto'></div></div>"));
            assert_eq!(height(&doc, &page, "p"), parent);
        }
        for child in [
            "<img style='width:10px;height:80px;flex:none;margin-bottom:25px'>",
            "<video style='width:10px;height:80px;flex:none;margin-bottom:25px'></video>",
            "<input type=button style='width:10px;height:80px;flex:none;margin-bottom:25px'>",
        ] {
            let (doc, page) = measure(&format!(
                "<div id=p style='display:flex;height:40px;align-items:flex-start'>{child}</div>"
            ));
            assert_eq!(height(&doc, &page, "p"), 105, "{child}");
        }
        let (doc,page)=measure("<div id=p style='display:flex;flex-direction:column;height:40px'><div id=inner style='display:flex;flex-direction:column;flex:1;min-height:0'><div style='height:200px;flex:1;min-height:0'></div></div></div>");
        assert_eq!(height(&doc, &page, "p"), 40);
        assert_eq!(height(&doc, &page, "inner"), 40);
        let (doc,page)=measure("<div id=p style='position:relative;height:40px'><div style='position:absolute;top:300px;height:30px;margin-bottom:25px'></div></div>");
        assert_eq!(height(&doc, &page, "p"), 355);
    }
    #[test]
    fn scroll_height_depth_boundary_keeps_last_ancestor_viewport_transition() {
        let mut measured = 0;
        let mut longest = 0;
        for depth in 118..=128 {
            let markup = format!(
                "<div id=p style='height:1px'>{}<img id=deep width=1 height=900>{}</div>",
                "<div style='height:1px'>".repeat(depth),
                "</div>".repeat(depth)
            );
            let (doc, page) = measure(&markup);
            let Some(image) = (0..doc.nodes.len()).find(|n| doc.attr(*n, "id") == Some("deep"))
            else {
                continue;
            };
            if let Some(rect) = page.border_rects[image] {
                measured += 1;
                let mut current = image;
                let mut ancestors = 0;
                while let Some(parent) =
                    (0..doc.nodes.len()).find(|p| doc.nodes[*p].children.contains(&current))
                {
                    ancestors += 1;
                    current = parent;
                }
                longest = longest.max(ancestors);
                assert!(
                    page.viewport_scroll_height >= rect.y + rect.height,
                    "depth={depth}; viewport={}; image bottom={}",
                    page.viewport_scroll_height,
                    rect.y + rect.height
                );
                assert!(height(&doc, &page, "p") >= 900);
            }
        }
        assert!(measured >= 6, "Measured {measured} deep renderer controls.");
        assert!(
            longest >= MAX_DEPTH,
            "Deepest measured ancestor chain was {longest}."
        );
    }
    #[test]
    fn scroll_height_reverse_origin_is_explicitly_unavailable() {
        let (doc,page)=measure("<div id=p style='display:flex;flex-direction:column-reverse;height:40px'><div style='height:100px;flex:none'></div><div style='height:100px;flex:none'></div></div>");
        let node = node(&doc, "p");
        assert!(page.client_boxes[node].is_some());
        assert!(page.scroll_heights[node].is_none());
    }
}
