//! Atomic normal-blend opacity contexts over the retained original paint scene.
//! Geometry and hit ownership are unchanged, including at opacity zero.
use super::*;

pub(super) struct Context {
    parent: usize,
    opacity: f64,
}

struct Frame {
    node: usize,
    context: usize,
    layer: usize,
    opacity: usize,
    depth: usize,
}

impl Engine<'_> {
    pub(super) fn prepare_opacity(
        &mut self,
        mut budget: Option<&mut css::CssBudget<'_>>,
    ) -> Result<(), String> {
        if !self.layout.computed_properties.has_opacity_paint() {
            return Ok(());
        }
        let count = self.doc.nodes.len();
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(count.saturating_mul(3))?;
        }
        let span_work = self.spans.iter().flatten().fold(0usize, |sum, span| {
            sum.saturating_add(span.end.items - span.start.items)
        });
        let items = self.layout.items.len();
        let decorations = self
            .decorations
            .iter()
            .fold(0usize, |sum, entry| sum.saturating_add(entry.items.len()));
        let edges = self
            .doc
            .nodes
            .iter()
            .fold(0usize, |sum, node| sum.saturating_add(node.children.len()));
        let scene = items.saturating_add(decorations);
        let work = count
            .saturating_mul(40)
            .saturating_add(edges.saturating_mul(4))
            .saturating_add(span_work)
            .saturating_add(scene.saturating_mul(16 + 4 * (scene.max(1).ilog2() as usize + 1)));
        // Reuse the finite scene-work envelope. Refuse the complete frame
        // rather than paint an unbounded or partially grouped prefix.
        if work > MAX_FLEX_WORK {
            self.layout.opacity_unavailable = true;
            return Ok(());
        }
        let storage = count
            .saturating_mul(
                std::mem::size_of::<Frame>()
                    + std::mem::size_of::<Context>()
                    + std::mem::size_of::<Vec<usize>>()
                    + 32 * std::mem::size_of::<usize>()
                    + 2 * std::mem::size_of::<bool>()
                    + 4 * std::mem::size_of::<crate::paint::OpacityGroup>(),
            )
            .saturating_add(scene.saturating_mul(6).saturating_mul(
                std::mem::size_of::<PaintItem>()
                    + 3 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<bool>()
                    + std::mem::size_of::<Option<ClipRect>>(),
            ));
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(work)?;
            meter.copies(storage)?;
        }

        let mut order = Vec::with_capacity(count);
        let mut seen = vec![false; count];
        let mut active = vec![false; count];
        let mut groups = vec![0usize; count];
        let mut layers = vec![Vec::<usize>::new(); count];
        let mut stack = Vec::with_capacity(count);
        self.opacity_nodes = vec![0; count];
        self.opacity_contexts.push(Context {
            parent: 0,
            opacity: 1.0,
        });
        if count != 0 {
            stack.push(Frame {
                node: 0,
                context: 0,
                layer: 0,
                opacity: 0,
                depth: 0,
            });
            seen[0] = true;
        }
        while let Some(Frame {
            node,
            mut context,
            mut layer,
            mut opacity,
            depth,
        }) = stack.pop()
        {
            order.push(node);
            let element = self.doc.nodes[node].namespace == crate::html::Namespace::Html
                && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. });
            let value = if element && self.styles[node].display != Display::None {
                self.layout.computed_properties.read_opacity(node)
            } else {
                Ok(None)
            };
            let root_background = matches!(&self.doc.nodes[node].kind,
                NodeKind::Element { tag, .. } if tag == "html" || tag == "body");
            if root_background {
                if !value.is_ok_and(|value| value.is_none_or(|value| value == 1.0)) {
                    self.layout.opacity_root_background_unsupported = true;
                }
            } else {
                match value {
                    Ok(Some(value)) if value < 1.0 => {
                        self.opacity_contexts.push(Context {
                            parent: opacity,
                            opacity: value,
                        });
                        opacity = self.opacity_contexts.len() - 1;
                        active[node] = true;
                    }
                    // An unsupported computed value preserves this group's
                    // prior paint. Readback remains unavailable, while genuine
                    // independent descendant/sibling groups can still paint.
                    Err(_) => self.layout.opacity_unsupported = true,
                    _ => {}
                }
            }
            self.opacity_nodes[node] = opacity;
            // Share the existing default auto/zero stacking order with genuine
            // transform contexts. Opacity does not establish a containing box.
            active[node] |= self.transformable(node)
                && matches!(self.layout.computed_properties.read_transform(node),
                    Ok(Some(value)) if value != css::Transform::None);
            if active[node] {
                layers[context].push(node);
                context = node;
                layer = node;
            } else if node != 0 && element && self.styles[node].position != Position::Static {
                layers[context].push(node);
                layer = node;
            }
            groups[node] = layer;
            if depth < MAX_DEPTH {
                for &child in self.visual_children(node).iter().rev() {
                    if child < count && !seen[child] && self.parents[child] == Some(node) {
                        seen[child] = true;
                        stack.push(Frame {
                            node: child,
                            context,
                            layer,
                            opacity,
                            depth: depth + 1,
                        });
                    }
                }
            }
        }
        if self.opacity_contexts.len() == 1 {
            return Ok(());
        }
        let mut ranks = vec![0usize; count];
        let mut contexts = vec![0usize];
        let mut next = 0;
        while let Some(node) = contexts.pop() {
            ranks[node] = next;
            next += 1;
            if node == 0 || active[node] {
                contexts.extend(layers[node].iter().rev().copied());
            }
        }
        self.translated_ranks = groups.iter().map(|&group| ranks[group]).collect();
        let mut owners = vec![0; items];
        for node in order {
            if let Some(span) = self.spans[node] {
                owners[span.start.items..span.end.items].fill(node);
            }
        }
        self.opacity_items = vec![0; items];
        for (index, item) in self.layout.items.iter().enumerate() {
            let owner = match item {
                PaintItem::Text(run) => self.run_nodes[*run],
                PaintItem::Control(control) => self.layout.controls[*control].node,
                PaintItem::Image { node, .. } | PaintItem::Background { node, .. } => *node,
                _ => owners[index],
            };
            self.opacity_items[index] = self.opacity_nodes[owner];
            self.layout.positioned_items[index] = self.translated_ranks[owner];
        }
        Ok(())
    }

    pub(super) fn finish_opacity(&mut self) {
        if self.opacity_items.is_empty() {
            return;
        }
        let mut ranges = vec![(usize::MAX, 0usize); self.opacity_contexts.len()];
        for (index, &group) in self.opacity_items.iter().enumerate() {
            ranges[group].0 = ranges[group].0.min(index);
            ranges[group].1 = index + 1;
        }
        for group in (1..ranges.len()).rev() {
            let parent = self.opacity_contexts[group].parent;
            ranges[parent].0 = ranges[parent].0.min(ranges[group].0);
            ranges[parent].1 = ranges[parent].1.max(ranges[group].1);
        }
        let mut ordered: Vec<_> = ranges
            .into_iter()
            .enumerate()
            .skip(1)
            .filter(|(_, (start, end))| start < end)
            .collect();
        ordered.sort_by_key(|&(group, (start, end))| (start, std::cmp::Reverse(end), group));
        let mut stack: Vec<(usize, usize)> = Vec::new();
        let mut next = 0;
        for (index, &owner) in self.opacity_items.iter().enumerate() {
            while stack.last().is_some_and(|&(_, end)| end == index) {
                stack.pop();
            }
            while next < ordered.len() && ordered[next].1 .0 == index {
                let (group, (_, end)) = ordered[next];
                if stack
                    .last()
                    .is_some_and(|&(_, parent_end)| end > parent_end)
                {
                    self.layout.opacity_unavailable = true;
                    return;
                }
                stack.push((group, end));
                next += 1;
            }
            if stack.last().map_or(0, |&(group, _)| group) != owner {
                self.layout.opacity_unavailable = true;
                return;
            }
        }
        self.layout.opacity_groups = ordered
            .into_iter()
            .map(|(group, (start, end))| crate::paint::OpacityGroup {
                start,
                end,
                opacity: self.opacity_contexts[group].opacity,
            })
            .collect();
    }
}

impl TextRun {
    /// Bounds relative to the run origin. Include logical highlight rectangles
    /// and underline as well as real glyph overhang, without guessing metrics.
    pub(crate) fn opacity_paint_bounds(&self) -> ClipRect {
        let mut bounds = ClipRect::new(
            0,
            -1,
            self.width(),
            self.height().max(self.ascent() + 3) + 1,
        );
        let mut include = |rect: ClipRect| {
            if rect.left < rect.right && rect.top < rect.bottom {
                bounds.left = bounds.left.min(rect.left);
                bounds.top = bounds.top.min(rect.top);
                bounds.right = bounds.right.max(rect.right);
                bounds.bottom = bounds.bottom.max(rect.bottom);
            }
        };
        if let Some(shape) = &self.shaped {
            include(shape.paint_bounds());
        } else {
            let (mut x, mut y) = (0i32, 0i32);
            for ch in self.text.chars() {
                match ch {
                    '\n' => {
                        x = 0;
                        y = y.saturating_add((GLYPH_HEIGHT + 2) * self.scale);
                    }
                    '\r' => {}
                    '\t' => x = x.saturating_add(4 * GLYPH_ADVANCE * self.scale),
                    _ => {
                        include(ClipRect::new(
                            x,
                            y,
                            (crate::paint::GLYPH_WIDTH + if self.italic { 2 } else { 0 })
                                * self.scale
                                + i32::from(self.bold),
                            GLYPH_HEIGHT * self.scale,
                        ));
                        x = x.saturating_add(GLYPH_ADVANCE * self.scale);
                    }
                }
            }
        }
        bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_paint_layout_atomic_contexts_keep_positioned_siblings_outside() {
        let doc = crate::html::parse("<style>body{margin:0}#g{position:absolute;left:20px;top:80px;width:72px;height:48px;opacity:.5;background:#010101}#inside{position:absolute;left:24px;top:0;width:48px;height:48px;background:#020202}#outside{position:absolute;left:44px;top:80px;width:48px;height:48px;background:#336699}</style><body><div id=g><div id=inside></div></div><div id=outside></div></body>");
        let page = layout(&doc, 300);
        assert!(!page.opacity_unavailable);
        assert_eq!(page.opacity_groups.len(), 1);
        let group = page.opacity_groups[0];
        let painted = |color| {
            page.items
                .iter()
                .position(
                    |item| matches!(item, PaintItem::Rect {color:value,..} if *value == color),
                )
                .unwrap()
        };
        assert!((group.start..group.end).contains(&painted(0x010101)));
        assert!((group.start..group.end).contains(&painted(0x020202)));
        assert!(group.end <= painted(0x336699));

        // An opacity context is not a positioned containing block.
        let doc = crate::html::parse("<body style='margin:0'><div style='margin:30px;opacity:.5'><div id=abs style='position:absolute;left:11px;top:13px;width:20px;height:20px;background:red'></div></div></body>");
        let page = layout(&doc, 300);
        let node = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("abs"))
            .unwrap();
        let rect = page.border_rects[node].unwrap();
        assert_eq!((rect.x, rect.y), (11, 13));
        assert!(!page.opacity_unavailable);
    }

    #[test]
    fn opacity_paint_layout_one_fast_path_and_explicit_root_background_boundary() {
        let ordinary = crate::html::parse(
            "<body><div style='opacity:1;background:red'>Unchanged</div></body>",
        );
        let page = layout(&ordinary, 300);
        assert!(page.opacity_groups.is_empty());
        assert!(!page.opacity_unavailable && !page.opacity_root_background_unsupported);
        let root = crate::html::parse("<body style='opacity:.5;background:red'><div style='opacity:.5;background:blue'>Real descendant</div></body>");
        let page = layout(&root, 300);
        assert_eq!(page.opacity_groups.len(), 1);
        assert!(page.opacity_root_background_unsupported);
        assert!(!page.opacity_unavailable);
    }
}
