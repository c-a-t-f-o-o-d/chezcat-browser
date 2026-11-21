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

    fn grid_intrinsic_size(&mut self, child: usize, depth: usize) -> Option<Intrinsic> {
        self.grid_budget.measuring = true;
        self.grid_budget.measurement_complete = true;
        let intrinsic = self.intrinsic(child, depth);
        self.grid_budget.measuring = false;
        self.grid_budget.measurement_complete.then_some(intrinsic)
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
                    NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. } | NodeKind::DocumentFragment
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
                    self.grid_intrinsic_size(child, depth + 1)?.minimum
                };
                minimum.saturating_add(margin[1] + margin[3])
            } else {
                self.grid_intrinsic_size(child, depth + 1)?.minimum
            };
            column = column.max(minimum).min(MAX_WIDTH);
        }
        Some(column)
    }

    fn grid_item_justify(&self, child: usize, parent: &Style) -> css::Justify {
        match self.styles[child].justify_self {
            css::Justify::Auto => parent.justify_items,
            value => value,
        }
    }

    fn grid_anonymous_width(
        &mut self,
        nodes: &[usize],
        style: &Style,
        width: i32,
    ) -> Result<i32, ()> {
        use css::Justify;
        if matches!(style.justify_items, Justify::Normal | Justify::Stretch) {
            return Ok(width);
        }
        if !matches!(
            style.justify_items,
            Justify::Start | Justify::Center | Justify::End
        ) || !matches!(style.white_space, WhiteSpace::Normal | WhiteSpace::NoWrap)
        {
            return Err(());
        }
        if !self.grid_work(nodes.len()) {
            return Err(());
        }
        let bytes = nodes
            .iter()
            .try_fold(0usize, |bytes, &node| {
                bytes.checked_add(match &self.doc.nodes[node].kind {
                    NodeKind::Text(text) => text.len(),
                    _ => 0,
                })
            })
            .ok_or(())?;
        if !self.grid_work(bytes) || !self.grid_budget.charge(0, bytes) {
            return Err(());
        }
        let mut text = String::new();
        text.try_reserve_exact(bytes).map_err(|_| ())?;
        for &node in nodes {
            if let NodeKind::Text(value) = &self.doc.nodes[node].kind {
                text.push_str(value);
            }
        }
        let size = self.collapsed_text_intrinsic(&text, style);
        Ok(width
            .max(size.minimum)
            .min(size.preferred)
            .clamp(0, MAX_WIDTH))
    }

    // Compute only the used width. Retain the author's box-sizing, height and
    // percentage edge references while the ordinary renderer builds the item.
    fn grid_item_width(
        &mut self,
        child: usize,
        parent: &Style,
        width: i32,
        depth: usize,
    ) -> Result<Option<Length>, ()> {
        use css::Justify;
        let align = self.grid_item_justify(child, parent);
        if align == Justify::Unavailable {
            return Err(());
        }
        let style = &self.styles[child];
        if style.width != Length::Auto {
            return Ok(None);
        }
        let auto_margin = style.margin[1] == Length::Auto || style.margin[3] == Length::Auto;
        let replaced = self.replaced_image(child) || forms::kind(self.doc, child).is_some();
        let stretch = matches!(align, Justify::Stretch) || align == Justify::Normal && !replaced;
        if replaced && (align != Justify::Stretch || auto_margin) {
            return Ok(None);
        }
        if stretch && !auto_margin && !replaced && align != Justify::Stretch {
            return Ok(None);
        }
        if !self.grid_work(1) {
            return Err(());
        }
        let intrinsic = if stretch && !auto_margin {
            None
        } else {
            Some(self.grid_intrinsic_size(child, depth + 1).ok_or(())?)
        };
        let style = &self.styles[child];
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
        let margin = self.edges(style.margin, width, style, true);
        let room = width.saturating_sub(margin[1] + margin[3] + edges).max(0);
        let desired = intrinsic.map_or(room, |size| {
            room.max((size.minimum - edges).max(0))
                .min((size.preferred - edges).max(0))
        });
        let desired = self
            .constrained(
                desired,
                style.min_width,
                style.max_width,
                Some(width),
                style,
                edges,
            )
            .min(MAX_WIDTH);
        Ok(Some(Length::Px(
            (desired + if style.border_box { edges } else { 0 }) as f32 / self.zoom,
        )))
    }

    fn grid_span_work(span: Span) -> usize {
        let (a, b) = (span.start, span.end);
        (b.runs - a.runs)
            .saturating_add(b.items - a.items)
            .saturating_add(b.atoms - a.atoms)
            .saturating_add(b.controls - a.controls)
            .saturating_add(b.anchors - a.anchors)
            .saturating_add(b.boxes - a.boxes)
            .saturating_add(b.pending - a.pending)
            .saturating_add(b.decorations - a.decorations)
    }

    fn grid_item_inline_position(
        &mut self,
        child: usize,
        parent: &Style,
        left: i32,
        width: i32,
        span: Span,
    ) -> bool {
        use css::Justify;
        let align = self.grid_item_justify(child, parent);
        if align == Justify::Unavailable {
            return false;
        }
        let style = &self.styles[child];
        let auto_left = style.margin[3] == Length::Auto;
        let auto_right = style.margin[1] == Length::Auto;
        if align == Justify::Normal
            && style.justify_self == Justify::Auto
            && parent.justify_items == Justify::Normal
            && !auto_left
            && !auto_right
        {
            return true;
        }
        if !self.grid_work(
            Self::grid_span_work(span)
                .saturating_mul(2)
                .saturating_add(1),
        ) {
            return false;
        }
        let bounds = if let Some(record) = &self.boxes[child] {
            Some((
                record.x,
                record.width
                    + record.padding[1]
                    + record.padding[3]
                    + record.border[1]
                    + record.border[3],
            ))
        } else if let Some(control) = self.layout.controls[span.start.controls..span.end.controls]
            .iter()
            .find(|c| c.node == child)
        {
            Some((control.x, control.width))
        } else {
            self.layout.items[span.start.items..span.end.items]
                .iter()
                .find_map(|item| {
                    if let PaintItem::Image { node, x, width, .. } = item {
                        if *node == child {
                            return self.layout.client_boxes[child].map(|client| {
                                (
                                    *x - client.padding[3] - client.left,
                                    *width + client.width + client.left + client.right,
                                )
                            });
                        }
                    }
                    None
                })
        };
        let Some((actual_left, border_width)) = bounds else {
            // A zero-sized replaced item has no paint box. Other unknown boxes
            // retain their bounded fallback and decline complete grid geometry.
            return self.replaced_image(child) || forms::kind(self.doc, child).is_some();
        };
        let style = &self.styles[child];
        let mut margin = self.edges(style.margin, width, style, true);
        let spare = width
            .saturating_sub(border_width)
            .saturating_sub(margin[1] + margin[3]);
        let offset = if auto_left || auto_right {
            if spare > 0 {
                if auto_left {
                    margin[3] = if auto_right { spare / 2 } else { spare };
                }
                if auto_right {
                    margin[1] = if auto_left { spare - margin[3] } else { spare };
                }
            }
            0
        } else {
            match align {
                Justify::Center => (f64::from(spare) / 2.0).round() as i32,
                Justify::End => spare,
                _ => 0,
            }
        };
        let desired = left
            .saturating_add(margin[3])
            .saturating_add(offset)
            .clamp(-MAX_HEIGHT, MAX_HEIGHT);
        if desired != actual_left {
            self.move_span(span, desired.saturating_sub(actual_left), 0);
        }
        self.used_margins.insert(child, margin);
        true
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
                        NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. } => {
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
            let item_width = if element {
                width
            } else {
                match self.grid_anonymous_width(&children[start_child..cursor], style, width) {
                    Ok(value) => value,
                    Err(()) => {
                        group.complete = false;
                        width
                    }
                }
            };
            let prior = std::mem::replace(
                &mut self.flow,
                Flow::new(
                    left,
                    left.saturating_add(item_width.max(1)),
                    item_top,
                    style.align,
                    style.white_space,
                ),
            );
            self.separate("\n");
            let start = self.mark();
            let original_width = self.styles[child].width;
            if element {
                match self.grid_item_width(child, style, width, depth) {
                    Ok(Some(value)) => std::rc::Rc::make_mut(&mut self.styles[child]).width = value,
                    Ok(None) => {}
                    Err(()) => group.complete = false,
                }
            }
            for &child in &children[start_child..cursor] {
                self.walk(child, link, depth + 1);
            }
            std::rc::Rc::make_mut(&mut self.styles[child]).width = original_width;
            self.finish_line(false, 0);
            let natural_height = self.flow.y.saturating_sub(item_top).max(0);
            let span = Span {
                start,
                end: self.mark(),
            };
            if element && !self.grid_item_inline_position(child, style, left, width, span) {
                group.complete = false;
            } else if !element && item_width != width {
                let spare = width.saturating_sub(item_width);
                let dx = match style.justify_items {
                    css::Justify::Center => (f64::from(spare) / 2.0).round() as i32,
                    css::Justify::End => spare,
                    _ => 0,
                };
                if dx != 0 {
                    if self.grid_work(Self::grid_span_work(span)) {
                        self.move_span(span, dx, 0);
                    } else {
                        group.complete = false;
                    }
                }
            }
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
