//! Original bounded translation rendering over the existing layout scene.
//! Raw CSSOM offset boxes are retained before this pass; visual rectangles keep
//! fractional translations while painting rounds final cumulative displacement.
//! https://drafts.csswg.org/css-transforms-1/#transform-rendering
use super::*;

#[cfg(test)]
#[path = "layout_translate_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(crate) struct VisualBorderRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fixed: bool,
}

struct Frame {
    node: usize,
    context: usize,
    layer: usize,
    delta: [f64; 2],
    known: bool,
    depth: usize,
}

fn shift_item(item: &mut PaintItem, delta: [i32; 2]) {
    match item {
        PaintItem::Rect { x, y, .. }
        | PaintItem::RoundedRect { x, y, .. }
        | PaintItem::Background { x, y, .. }
        | PaintItem::Image { x, y, .. }
        | PaintItem::HitBox { x, y, .. } => {
            *x = x.saturating_add(delta[0]);
            *y = y.saturating_add(delta[1]);
        }
        PaintItem::Text(_) | PaintItem::Control(_) => {}
    }
}

fn round_delta(delta: [f64; 2]) -> [i32; 2] {
    delta.map(|value| (value + 0.5).floor() as i32)
}

impl Engine<'_> {
    pub(super) fn transformable(&self, node: usize) -> bool {
        self.doc.nodes[node].namespace == crate::html::Namespace::Html
            && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. })
            && self.styles[node].display != Display::None
            && (self.styles[node].display != Display::Inline
                || matches!(
                    self.styles[node].position,
                    Position::Absolute | Position::Fixed
                )
                || self.boxes[node].is_some()
                || forms::kind(self.doc, node).is_some()
                || self.replaced_image(node))
    }

    pub(super) fn transform_establishes_cb(&self, node: usize) -> bool {
        self.styles[node].transform != css::Transform::None && self.transformable(node)
    }

    pub(super) fn transformed_parent(&self, node: usize) -> Option<usize> {
        if !self.layout.computed_properties.has_transform_geometry() {
            return None;
        }
        let mut parent = self.parents[node];
        for _ in 0..MAX_DEPTH {
            let node = parent?;
            if self.transform_establishes_cb(node) {
                return Some(node);
            }
            parent = self.parents[node];
        }
        None
    }

    pub(super) fn viewport_fixed(&self, node: usize) -> bool {
        let mut current = Some(node);
        let mut fixed = false;
        let transforms = self.layout.computed_properties.has_transform_geometry();
        for _ in 0..=MAX_DEPTH {
            let Some(node) = current else {
                break;
            };
            // An ancestor transform captures a fixed descendant, while an
            // outer viewport-fixed ancestor still carries the complete group.
            if transforms && self.transform_establishes_cb(node) {
                fixed = false;
            }
            if self.styles[node].position == Position::Fixed {
                if !transforms {
                    return true;
                }
                fixed = true;
            }
            current = self.parents[node];
        }
        fixed
    }

    fn resolve_translation(&self, node: usize, value: css::Transform) -> Option<[f64; 2]> {
        let css::Transform::Translate { x, y } = value else {
            return Some([0.0; 2]);
        };
        let rect = self.layout.border_rects.get(node).copied().flatten();
        let resolve = |length: Length, horizontal: bool| -> Option<f64> {
            let style = &self.styles[node];
            let value = match length {
                Length::Px(value) => f64::from(value) * f64::from(self.zoom),
                Length::Em(value) => {
                    f64::from(value) * f64::from(style.font_size) * f64::from(self.zoom)
                }
                Length::Rem(value) => {
                    f64::from(value) * f64::from(style.root_font_size) * f64::from(self.zoom)
                }
                Length::Percent(value) => {
                    let rect = rect?;
                    f64::from(value) * f64::from(if horizontal { rect.width } else { rect.height })
                        / 100.0
                }
                Length::Auto|Length::Calc(_) => return None,
            };
            (value.is_finite() && value.abs() <= f64::from(MAX_HEIGHT)).then_some(value)
        };
        Some([resolve(x, true)?, resolve(y, false)?])
    }

    pub(super) fn apply_translations(
        &mut self,
        mut budget: Option<&mut css::CssBudget<'_>>,
        fixed_boxes: &[bool],
        inline_rects: &HashMap<usize, BorderRect>,
    ) -> Result<(), String> {
        let count = self.doc.nodes.len();
        if !self.layout.computed_properties.has_transform_geometry() {
            return Ok(());
        }
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(count)?;
        }
        let needed = (1..count).any(|node| {
            self.transformable(node)
                && (self.layout.client_boxes[node].is_some()
                    || self.layout.offset_unavailable[node])
                && !matches!(
                    self.layout.computed_properties.read_transform(node),
                    Ok(Some(css::Transform::None)) | Ok(None)
                )
        });
        if !needed {
            return Ok(());
        }

        let items = self.layout.items.len();
        let atoms = self.atoms.len();
        let span_work = self.spans.iter().flatten().fold(0usize, |total, span| {
            total
                .saturating_add(span.end.items - span.start.items)
                .saturating_add(span.end.atoms - span.start.atoms)
        });
        let edges = self
            .doc
            .nodes
            .iter()
            .fold(0usize, |sum, node| sum.saturating_add(node.children.len()));
        let decorations = self.decorations.iter().fold(0usize, |sum, decoration| {
            sum.saturating_add(decoration.items.len())
        });
        let scene = items
            .saturating_add(atoms)
            .saturating_add(self.layout.runs.len())
            .saturating_add(self.layout.controls.len())
            .saturating_add(self.layout.anchors.len())
            .saturating_add(self.pending.len())
            .saturating_add(decorations);
        let work = count
            .saturating_mul(64 + 4 * MAX_DEPTH)
            .saturating_add(edges.saturating_mul(4))
            .saturating_add(span_work.saturating_mul(4))
            .saturating_add(scene.saturating_mul(16))
            .saturating_add(
                count
                    .saturating_mul(2)
                    .saturating_add(atoms)
                    .saturating_mul(16 + 4 * MAX_DEPTH),
            )
            .saturating_add(
                items
                    .saturating_mul(items.max(1).ilog2() as usize + 1)
                    .saturating_mul(4),
            );
        // Persistent visual records plus bounded node traversal/context data,
        // rounded-reference boxes, scroll-area union and clip caches. Every
        // temporary node vector has at most count entries; context child Vec
        // capacities together stay below four times the admitted node count
        // (including the initial small allocation of each nonempty child Vec).
        let node_storage = std::mem::size_of::<Option<VisualBorderRect>>()
            + std::mem::size_of::<Option<[f64; 2]>>()
            + std::mem::size_of::<Frame>()
            + std::mem::size_of::<Vec<usize>>()
            + 12 * std::mem::size_of::<usize>()
            + 4 * std::mem::size_of::<bool>()
            + std::mem::size_of::<[f64; 2]>()
            + std::mem::size_of::<[i32; 2]>()
            + std::mem::size_of::<Option<ClipRect>>()
            + std::mem::size_of::<Option<BorderRect>>()
            + 2 * std::mem::size_of::<Option<i32>>();
        let copies = count
            .saturating_mul(node_storage)
            .saturating_add(
                items
                    .saturating_add(atoms)
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
            // Static transformed contexts also need the existing stable paint
            // sort. Cover tuple/merge storage and cumulative rebuilt-vector
            // capacities without depending on allocator reuse.
            .saturating_add(items.saturating_mul(6).saturating_mul(
                std::mem::size_of::<PaintItem>()
                    + std::mem::size_of::<bool>()
                    + std::mem::size_of::<usize>()
                    + std::mem::size_of::<Option<ClipRect>>(),
            ));
        if let Some(meter) = budget.as_deref_mut() {
            meter.work(work)?;
            meter.copies(copies)?;
        }

        let mut deltas = vec![[0.0; 2]; count];
        let mut known = vec![false; count];
        let mut active = vec![false; count];
        let mut groups = vec![0usize; count];
        let mut layers = vec![Vec::<usize>::new(); count];
        let mut order = Vec::with_capacity(count);
        let mut seen = vec![false; count];
        let mut stack = Vec::with_capacity(count);
        self.layout.transform_translations = vec![None; count];
        self.layout.visual_border_rects = vec![None; count];
        self.layout.visual_unavailable = vec![true; count];
        for node in 1..count {
            if matches!(
                self.layout.computed_properties.read_transform(node),
                Ok(None)
            ) {
                self.layout.visual_unavailable[node] = false;
            }
        }
        if count != 0 {
            stack.push(Frame {
                node: 0,
                context: 0,
                layer: 0,
                delta: [0.0; 2],
                known: true,
                depth: 0,
            });
            seen[0] = true;
        }
        while let Some(Frame {
            node,
            mut context,
            mut layer,
            delta: inherited,
            known: parent_known,
            depth,
        }) = stack.pop()
        {
            order.push(node);
            let transformable = self.transformable(node);
            let value = self.layout.computed_properties.read_transform(node);
            let mut certain = parent_known;
            let own = if transformable {
                match value {
                    Ok(Some(value)) => {
                        active[node] = value != css::Transform::None;
                        // Without a measured reference/containing box, retain
                        // bounded visual fallback but never certify descendant
                        // geometry from a guessed transformed containing block.
                        if active[node] && self.layout.border_rects[node].is_none() {
                            certain = false;
                        }
                        match self.resolve_translation(node, value) {
                            Some(delta) => {
                                self.layout.transform_translations[node] =
                                    Some(delta.map(|value| value / f64::from(self.zoom)));
                                delta
                            }
                            None => {
                                certain = false;
                                [0.0; 2]
                            }
                        }
                    }
                    Ok(None) => [0.0; 2],
                    Err(_) => {
                        certain = false;
                        [0.0; 2]
                    }
                }
            } else {
                [0.0; 2]
            };
            let cumulative = [inherited[0] + own[0], inherited[1] + own[1]];
            if cumulative
                .iter()
                .any(|value| !value.is_finite() || value.abs() > f64::from(i32::MAX / 4))
            {
                certain = false;
                deltas[node] = inherited;
            } else {
                deltas[node] = cumulative;
            }
            known[node] = certain;
            if active[node] {
                layers[context].push(node);
                context = node;
                layer = node;
            } else if node != 0
                && self.styles[node].position != Position::Static
                && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. })
            {
                layers[context].push(node);
                layer = node;
            }
            groups[node] = layer;
            let raw = self.layout.border_rects[node].or_else(|| inline_rects.get(&node).copied());
            let no_box = raw.is_none()
                && (!self.layout.offset_unavailable[node]
                    || matches!(
                        self.layout.computed_properties.read_transform(node),
                        Ok(None)
                    ));
            self.layout.visual_unavailable[node] = !no_box && (raw.is_none() || !certain);
            if certain {
                self.layout.visual_border_rects[node] = raw.map(|rect| VisualBorderRect {
                    x: f64::from(rect.x) + deltas[node][0],
                    y: f64::from(rect.y) + deltas[node][1],
                    width: f64::from(rect.width),
                    height: f64::from(rect.height),
                    fixed: rect.fixed,
                });
            }
            if depth < MAX_DEPTH {
                for &child in self.visual_children(node).iter().rev() {
                    if child < count && !seen[child] && self.parents[child] == Some(node) {
                        seen[child] = true;
                        stack.push(Frame {
                            node: child,
                            context,
                            layer,
                            delta: deltas[node],
                            known: certain,
                            depth: depth + 1,
                        });
                    }
                }
            }
        }
        // Each transform is an atomic auto/zero stacking context. Positioned
        // descendants may escape ordinary positioned ancestors, but never a
        // transform context. Context-layer lists are already in DOM preorder.
        let mut ranks = vec![0usize; count];
        let mut next_rank = 0usize;
        let mut contexts = vec![0usize];
        while let Some(node) = contexts.pop() {
            ranks[node] = next_rank;
            next_rank += 1;
            if node == 0 || active[node] {
                contexts.extend(layers[node].iter().rev().copied());
            }
        }
        self.translated_ranks = groups.iter().map(|&group| ranks[group]).collect();
        let mut item_owners = vec![0usize; items];
        let mut atom_owners = vec![0usize; atoms];
        for &node in &order {
            if let Some(span) = self.spans[node] {
                item_owners[span.start.items..span.end.items].fill(node);
                atom_owners[span.start.atoms..span.end.atoms].fill(node);
            }
        }
        for (index, item) in self.layout.items.iter().enumerate() {
            item_owners[index] = match item {
                PaintItem::Text(run) => self.run_nodes[*run],
                PaintItem::Control(control) => self.layout.controls[*control].node,
                PaintItem::Image {node,..}|PaintItem::Background{node,..}|PaintItem::Rect{node,..}|PaintItem::RoundedRect{node,..}|PaintItem::HitBox{node,..}=>*node,
            };
        }
        for (index, atom) in self.atoms.iter().enumerate() {
            atom_owners[index] = match atom.kind {
                AtomKind::Text(run) => self.run_nodes[run],
                AtomKind::Control(control) => self.layout.controls[control].node,
                AtomKind::Image { node, .. } => node,
                AtomKind::InlineBlock(_) => atom_owners[index],
            };
        }
        let rounded: Vec<_> = deltas.iter().copied().map(round_delta).collect();
        for (index, run) in self.layout.runs.iter_mut().enumerate() {
            let delta = rounded[self.run_nodes[index]];
            run.x = run.x.saturating_add(delta[0]);
            run.y = run.y.saturating_add(delta[1]);
        }
        for control in &mut self.layout.controls {
            let delta = rounded[control.node];
            control.x = control.x.saturating_add(delta[0]);
            control.y = control.y.saturating_add(delta[1]);
        }
        for (index, item) in self.layout.items.iter_mut().enumerate() {
            shift_item(item, rounded[item_owners[index]]);
            self.layout.positioned_items[index] = self.translated_ranks[item_owners[index]];
        }
        for (index, atom) in self.atoms.iter_mut().enumerate() {
            let delta = rounded[atom_owners[index]];
            atom.x = atom.x.saturating_add(delta[0]);
            atom.y = atom.y.saturating_add(delta[1]);
        }
        for (index, (_, y)) in self.layout.anchors.iter_mut().enumerate() {
            *y = y.saturating_add(rounded[self.anchor_nodes[index]][1]);
        }
        for (node, record) in self.boxes.iter_mut().enumerate() {
            if let Some(record) = record {
                record.x = record.x.saturating_add(rounded[node][0]);
                record.y = record.y.saturating_add(rounded[node][1]);
            }
        }
        for pending in &mut self.pending {
            if let Some(bounds) = pending.placed_bounds {
                let delta = rounded[pending.node];
                pending.placed_bounds = Some(ClipRect::new(
                    bounds.left.saturating_add(delta[0]),
                    bounds.top.saturating_add(delta[1]),
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                ));
            }
        }
        for decoration in &mut self.decorations {
            for item in &mut decoration.items {
                shift_item(item, rounded[decoration.node]);
            }
        }
        self.translated_owners = Some((item_owners, atom_owners));

        // Transforms expand overflow; they do not remove the original area.
        // Reuse the existing measured-area pass in rounded visual coordinates
        // and union its vertical extent with the retained untransformed area.
        let previous_heights = self.layout.scroll_heights.clone();
        let previous_viewport = self.layout.viewport_scroll_height;
        let visual = self
            .layout
            .border_rects
            .iter()
            .enumerate()
            .map(|(node, rect)| {
                rect.map(|mut rect| {
                    rect.x = rect.x.saturating_add(rounded[node][0]);
                    rect.y = rect.y.saturating_add(rounded[node][1]);
                    rect
                })
            })
            .collect();
        let raw = std::mem::replace(&mut self.layout.border_rects, visual);
        self.finish_scroll_heights(fixed_boxes);
        self.layout.border_rects = raw;
        for (height, previous) in self.layout.scroll_heights.iter_mut().zip(previous_heights) {
            *height = match (*height, previous) {
                (Some(now), Some(old)) => Some(now.max(old)),
                _ => None,
            };
        }
        self.layout.viewport_scroll_height =
            self.layout.viewport_scroll_height.max(previous_viewport);
        if known
            .iter()
            .zip(&self.layout.border_rects)
            .any(|(known, rect)| !known && rect.is_some())
        {
            self.layout.viewport_scroll_unavailable = true;
        }
        for node in 1..count {
            if known[node] || self.layout.border_rects[node].is_none() {
                continue;
            }
            let mut current = Some(node);
            for _ in 0..=MAX_DEPTH {
                let Some(parent) = current else {
                    break;
                };
                self.layout.scroll_heights[parent] = None;
                current = self.clip_parent(parent);
            }
        }
        Ok(())
    }

    pub(super) fn translated_clips(&mut self) -> Option<Vec<Option<ClipRect>>> {
        let (items, atoms) = self.translated_owners.take()?;
        let clips: Vec<_> = (0..self.doc.nodes.len())
            .map(|node| self.ancestor_clip(node))
            .collect();
        self.layout.item_clips = items.iter().map(|&node| clips[node]).collect();
        let atoms = atoms.iter().map(|&node| clips[node]).collect();
        self.layout.run_clips = vec![None; self.layout.runs.len()];
        for (index, item) in self.layout.items.iter().enumerate() {
            if let PaintItem::Text(run) = item {
                self.layout.run_clips[*run] = self.layout.item_clips[index];
            }
        }
        Some(atoms)
    }
}
