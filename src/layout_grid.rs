//! One implicit auto column with auto-placed rows, independent item formatting,
//! and default auto-track stretching. Authored tracks and placement are deferred.
//! https://www.w3.org/TR/css-grid-1/#grid-items
use super::*;

pub(super) struct GridRow {
    node: Option<usize>,
    span: Span,
    natural_height: i32,
    offset: i32,
}

pub(super) struct GridGroup {
    rows: Vec<GridRow>,
    natural_height: i32,
    pub(super) track_end: i32,
    pub(super) complete: bool,
}

// Borrowed CSS counters are also needed by the transform pass after layout.
// Keep this pass's remaining allowance locally, then settle every debit before
// another pass can draw from the same job. Refused work consumes the remainder.
pub(super) struct GridBudget {
    work: usize,
    copies: usize,
    refused: bool,
    pub(super) measuring: bool,
    pub(super) measurement_complete: bool,
    pub(super) incomplete: bool,
}

impl GridBudget {
    pub(super) fn new(meter: Option<&css::CssBudget<'_>>) -> Self {
        Self {
            work: meter.map_or(usize::MAX, |m| m.remaining_work()),
            copies: meter.map_or(usize::MAX, |m| m.remaining_copies()),
            refused: false,
            measuring: false,
            measurement_complete: true,
            incomplete: false,
        }
    }

    fn charge(&mut self, work: usize, copies: usize) -> bool {
        if self.refused {
            return false;
        }
        if work > self.work {
            self.work = 0;
            self.refused = true;
            return false;
        }
        self.work -= work;
        if copies > self.copies {
            self.copies = 0;
            self.refused = true;
            return false;
        }
        self.copies -= copies;
        true
    }

    pub(super) fn settle(&self, meter: &mut css::CssBudget<'_>) -> Result<(), String> {
        meter.work(meter.remaining_work() - self.work)?;
        meter.copies(meter.remaining_copies() - self.copies)?;
        if self.refused {
            return Err("CSS grid layout work or copy limit reached.".into());
        }
        Ok(())
    }
}

impl Engine<'_> {
    pub(super) fn withhold_incomplete_grid_geometry(
        &mut self,
        mut meter: Option<&mut css::CssBudget<'_>>,
    ) -> Result<(), String> {
        // Rows painted before a cap was reached have not necessarily received
        // their final track sizes. This includes native and positioned children.
        let count = self.doc.nodes.len();
        if let Some(meter) = meter.as_deref_mut() {
            meter.work(count)?;
            meter.copies(count.saturating_mul(2 + std::mem::size_of::<usize>()))?;
        }
        let mut affected = vec![false; count];
        let mut scroll_affected = vec![false; count];
        let mut pending = Vec::with_capacity(count);
        for (node, record) in self.boxes.iter().enumerate() {
            if record.as_ref().is_some_and(|r| !r.grid_complete()) {
                affected[node] = true;
                pending.push(node);
                let mut parent = self.parents[node];
                while let Some(ancestor) = parent {
                    if scroll_affected[ancestor] {
                        break;
                    }
                    if let Some(meter) = meter.as_deref_mut() {
                        meter.work(1)?;
                    }
                    scroll_affected[ancestor] = true;
                    self.layout.scroll_heights[ancestor] = None;
                    parent = self.parents[ancestor];
                }
            }
        }
        while let Some(node) = pending.pop() {
            let children = &self.doc.nodes[node].children;
            if let Some(meter) = meter.as_deref_mut() {
                meter.work(children.len() + 1)?;
            }
            self.layout.client_boxes[node] = None;
            self.layout.border_rects[node] = None;
            self.layout.scroll_heights[node] = None;
            self.layout.offset_unavailable[node] = true;
            self.layout.offset_parents[node] = OffsetParent::Unavailable;
            for &child in children {
                if self.parents.get(child) == Some(&Some(node))
                    && !affected[child]
                    && !self.grid_skips(child)
                {
                    affected[child] = true;
                    pending.push(child);
                }
            }
        }
        self.layout.incomplete_grid_nodes = affected;
        self.layout.viewport_scroll_unavailable = true;
        Ok(())
    }

    fn grid_work(&mut self, work: usize) -> bool {
        if !self.grid_budget.charge(work.min(self.flex_work), 0) {
            return false;
        }
        if work > self.flex_work {
            self.flex_work = 0;
            return false;
        }
        self.flex_work -= work;
        true
    }

    pub(super) fn grid_intrinsic_cost(&mut self, node: usize) -> bool {
        let children = self.doc.nodes[node].children.len();
        let bytes = match &self.doc.nodes[node].kind {
            NodeKind::Text(text) => text.len(),
            _ => 0,
        };
        let work = children.saturating_add(bytes).saturating_add(1);
        self.grid_work(work)
            && self
                .grid_budget
                .charge(0, children.saturating_mul(std::mem::size_of::<usize>()))
    }

    fn grid_min_content(&mut self, child: usize, depth: usize) -> Option<i32> {
        self.grid_budget.measuring = true;
        self.grid_budget.measurement_complete = true;
        let intrinsic = self.intrinsic(child, depth);
        self.grid_budget.measuring = false;
        self.grid_budget
            .measurement_complete
            .then_some(intrinsic.minimum)
    }

    fn grid_skips(&self, node: usize) -> bool {
        self.styles[node].display == Display::None
            || forms::kind(self.doc, node) == Some(Kind::Hidden)
            || matches!(&self.doc.nodes[node].kind, NodeKind::Element { tag, .. }
                if matches!(tag.as_str(), "head" | "title" | "script" | "style" | "link"
                    | "meta" | "base" | "template" | "iframe" | "object" | "datalist")
                    || self.doc.scripting_enabled && tag == "noscript")
    }

    fn grid_column_width(&mut self, node: usize, width: i32, depth: usize) -> Option<i32> {
        let mut column = width;
        for &child in &self.doc.nodes[node].children {
            if !self.grid_work(1) {
                return None;
            }
            if self.grid_skips(child)
                || matches!(
                    self.doc.nodes[child].kind,
                    NodeKind::Comment(_) | NodeKind::DocumentFragment
                )
                || matches!(self.doc.nodes[child].kind, NodeKind::Element { .. })
                    && matches!(
                        self.styles[child].position,
                        Position::Absolute | Position::Fixed
                    )
            {
                continue;
            }
            let style = &self.styles[child];
            let minimum = if matches!(self.doc.nodes[child].kind, NodeKind::Element { .. }) {
                let margin = self.edges(style.margin, width, style, true);
                let padding = self.edges(style.padding, width, style, false);
                let border = self.border(style, width);
                let kind = forms::kind(self.doc, child);
                let edges = if kind.is_some() && !self.loaded_image_control(child) {
                    if native_button_padding(kind) {
                        padding[1] + padding[3]
                    } else {
                        0
                    }
                } else {
                    padding[1] + padding[3] + border[1] + border[3]
                };
                let specified = self
                    .dimension(style.width, None, style, edges)
                    .or_else(|| self.dimension(style.min_width, None, style, edges));
                let minimum = if let Some(content) = specified {
                    self.constrained(
                        content,
                        style.min_width,
                        style.max_width,
                        Some(width),
                        style,
                        edges,
                    )
                    .saturating_add(edges)
                } else if style.min_width == Length::Auto && style.overflow.x.scrollable() {
                    edges
                } else {
                    self.grid_min_content(child, depth + 1)?
                };
                minimum.saturating_add(margin[1] + margin[3])
            } else {
                self.grid_min_content(child, depth + 1)?
            };
            column = column.max(minimum).min(MAX_WIDTH);
        }
        Some(column)
    }

    pub(super) fn grid(
        &mut self,
        node: usize,
        style: &Style,
        link: Option<usize>,
        depth: usize,
        width: i32,
        height: Option<i32>,
    ) -> GridGroup {
        let mut group = GridGroup {
            rows: Vec::new(),
            natural_height: 0,
            track_end: 0,
            complete: true,
        };
        let children = &self.doc.nodes[node].children;
        if depth >= MAX_DEPTH && !children.is_empty() {
            group.complete = false;
            return group;
        }
        let Some(width) = self.grid_column_width(node, width, depth) else {
            group.complete = false;
            return group;
        };
        let capacity = children.len().min(MAX_FLEX_ITEMS);
        if !self
            .grid_budget
            .charge(0, capacity.saturating_mul(std::mem::size_of::<GridRow>()))
            || group.rows.try_reserve_exact(capacity).is_err()
        {
            group.complete = false;
            return group;
        }
        let (left, top) = (self.flow.left, self.flow.y);
        let gap = self
            .length(style.gap[0], height.unwrap_or(0), style)
            .unwrap_or(0)
            .max(0);
        let mut cursor = 0;
        while cursor < children.len() {
            if self.finished() || !self.grid_work(1) {
                group.complete = false;
                break;
            }
            let child = children[cursor];
            if self.grid_skips(child)
                || matches!(self.doc.nodes[child].kind, NodeKind::DocumentFragment)
            {
                cursor += 1;
                continue;
            }
            if matches!(self.doc.nodes[child].kind, NodeKind::Element { .. })
                && matches!(
                    self.styles[child].position,
                    Position::Absolute | Position::Fixed
                )
            {
                // The static-position rectangle is the grid's content box,
                // rather than a nonexistent row at this source position.
                self.walk(child, link, depth + 1);
                cursor += 1;
                continue;
            }
            let start_child = cursor;
            let element = matches!(self.doc.nodes[child].kind, NodeKind::Element { .. });
            if element {
                cursor += 1;
            } else {
                let mut has_text = false;
                while cursor < children.len() {
                    let child = children[cursor];
                    match &self.doc.nodes[child].kind {
                        NodeKind::Text(text) => {
                            if !self.grid_work(text.len().saturating_add(1)) {
                                group.complete = false;
                                break;
                            }
                            has_text |= !text
                                .chars()
                                .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{000c}'));
                        }
                        NodeKind::Comment(_) => {
                            if !self.grid_work(1) {
                                group.complete = false;
                                break;
                            }
                        }
                        _ => break,
                    }
                    cursor += 1;
                }
                if !group.complete {
                    break;
                }
                if !has_text {
                    continue;
                }
            }
            if group.rows.len() == MAX_FLEX_ITEMS {
                group.complete = false;
                break;
            }
            if !group.rows.is_empty() {
                group.natural_height = group.natural_height.saturating_add(gap).min(MAX_HEIGHT);
            }
            let offset = group.natural_height;
            let item_top = top.saturating_add(offset).clamp(-MAX_HEIGHT, MAX_HEIGHT);
            let prior = std::mem::replace(
                &mut self.flow,
                Flow::new(
                    left,
                    left.saturating_add(width.max(1)),
                    item_top,
                    style.align,
                    style.white_space,
                ),
            );
            self.separate("\n");
            let start = self.mark();
            for &child in &children[start_child..cursor] {
                self.walk(child, link, depth + 1);
            }
            self.finish_line(false, 0);
            let natural_height = self.flow.y.saturating_sub(item_top).max(0);
            let span = Span {
                start,
                end: self.mark(),
            };
            if element
                && self.boxes[child]
                    .as_ref()
                    .is_some_and(|record| !record.grid_complete())
            {
                group.complete = false;
            }
            self.flow = prior;
            group.rows.push(GridRow {
                node: element.then_some(child),
                span,
                natural_height,
                offset,
            });
            group.natural_height = group
                .natural_height
                .saturating_add(natural_height)
                .min(MAX_HEIGHT);
            if self.finished() || item_top.saturating_add(natural_height) >= MAX_HEIGHT {
                group.complete = false;
                break;
            }
        }
        self.position_grid(&mut group, node, height, depth);
        // Cyclic percentage gaps contribute zero to auto-height sizing, then
        // resolve against that height during placement and may overflow it.
        self.flow.y = top.saturating_add(group.natural_height).min(MAX_HEIGHT);
        self.flow.x = left;
        self.flow.space_pending = false;
        group
    }

    pub(super) fn position_grid(
        &mut self,
        group: &mut GridGroup,
        node: usize,
        height: Option<i32>,
        depth: usize,
    ) {
        if group.rows.is_empty() || !group.complete {
            return;
        }
        if depth > MAX_DEPTH || !self.grid_work(group.rows.len()) {
            group.complete = false;
            return;
        }
        let work = group.rows.iter().fold(0usize, |work, row| {
            let (a, b) = (row.span.start, row.span.end);
            work.saturating_add(b.runs - a.runs)
                .saturating_add(b.items - a.items)
                .saturating_add(b.atoms - a.atoms)
                .saturating_add(b.controls - a.controls)
                .saturating_add(b.anchors - a.anchors)
                .saturating_add(b.boxes - a.boxes)
                .saturating_add(b.pending - a.pending)
                .saturating_add(b.decorations - a.decorations)
        });
        if !self.grid_work(work) {
            group.complete = false;
            return;
        }
        let style = self.styles[node].clone();
        let gap = self
            .length(style.gap[0], height.unwrap_or(group.natural_height), &style)
            .unwrap_or(0)
            .max(0);
        let used = group
            .rows
            .iter()
            .fold(0i64, |used, row| used + i64::from(row.natural_height))
            + i64::from(gap) * group.rows.len().saturating_sub(1) as i64;
        let free = height.map_or(0, |height| (i64::from(height) - used).max(0)) as i32;
        let stretch = style.align_content == FlexPack::Stretch;
        let (start, extra_gap) = if stretch {
            (0.0, 0.0)
        } else {
            Self::pack(style.align_content, f64::from(free), group.rows.len())
        };
        let mut offset = start;
        let count = group.rows.len() as i64;
        for (index, row) in group.rows.iter_mut().enumerate() {
            let extra = if stretch {
                (i64::from(free) * (index as i64 + 1) / count
                    - i64::from(free) * index as i64 / count) as i32
            } else {
                0
            };
            let track_height = row.natural_height.saturating_add(extra).min(MAX_HEIGHT);
            let mut alignment_offset = 0;
            if let Some(child) = row.node {
                if let Some(record) = &self.boxes[child] {
                    let child_style = self.styles[child].clone();
                    let vertical =
                        record.padding[0] + record.padding[2] + record.border[0] + record.border[2];
                    let margin = self.used_margins.get(&child).copied().unwrap_or([0; 4]);
                    let auto_top = child_style.margin[0] == Length::Auto;
                    let auto_bottom = child_style.margin[2] == Length::Auto;
                    let align = child_style.align_self.unwrap_or(style.align_items);
                    let natural = (row.natural_height - margin[0] - margin[2] - vertical).max(0);
                    let specified = self.dimension(
                        child_style.height,
                        Some(track_height),
                        &child_style,
                        vertical,
                    );
                    let desired = specified.unwrap_or_else(|| {
                        if align == FlexAlign::Stretch && !auto_top && !auto_bottom {
                            (track_height - margin[0] - margin[2] - vertical).max(0)
                        } else {
                            natural
                        }
                    });
                    let desired = self.constrained(
                        desired,
                        child_style.min_height,
                        child_style.max_height,
                        Some(track_height),
                        &child_style,
                        vertical,
                    );
                    if desired != record.height {
                        self.resize_box(child, desired, depth + 1);
                    }
                    let spare = track_height - desired - vertical - margin[0] - margin[2];
                    alignment_offset = if auto_top || auto_bottom {
                        if auto_top && spare > 0 {
                            if auto_bottom {
                                spare / 2
                            } else {
                                spare
                            }
                        } else {
                            0
                        }
                    } else {
                        match align {
                            FlexAlign::End => spare,
                            FlexAlign::Center => spare / 2,
                            _ => 0,
                        }
                    };
                }
            }
            let next = (offset.round() as i32)
                .saturating_add(alignment_offset)
                .clamp(-MAX_HEIGHT, MAX_HEIGHT);
            if next != row.offset {
                self.move_span(row.span, 0, next.saturating_sub(row.offset));
            }
            row.offset = next;
            group.track_end = (offset.round() as i32).saturating_add(track_height);
            offset += f64::from(track_height.saturating_add(gap)) + extra_gap;
        }
    }
}
