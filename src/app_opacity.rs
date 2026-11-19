//! Conservative paint regions in the actual current App viewport. Resolve them
//! from retained group membership and real items on each draw, so fixed content,
//! scrolling, native state and font ink cannot leave stale snapshot extents.
use super::*;
use crate::paint::ClipRect;

fn include(bounds: &mut Option<ClipRect>, rect: Option<ClipRect>) {
    let Some(rect) = rect.filter(|rect| rect.left < rect.right && rect.top < rect.bottom) else {
        return;
    };
    *bounds = Some(bounds.map_or(rect, |before| ClipRect {
        left: before.left.min(rect.left),
        top: before.top.min(rect.top),
        right: before.right.max(rect.right),
        bottom: before.bottom.max(rect.bottom),
    }));
}

impl App {
    pub(super) fn opacity_paint_report(&self) -> String {
        let mut report = String::new();
        if self.page.opacity_root_background_unsupported {
            report.push_str("PAINT: root/body group opacity is deferred because page-background propagation is outside this slice. Prior root/body painting is retained. Ordinary descendant opacity groups remain active.\n");
        }
        if self.page.opacity_unsupported {
            report.push_str("PAINT: an unsupported computed opacity retains prior painting for that element. Its computed readback remains unavailable. Independent opacity groups remain active.\n");
        }
        if self.page.opacity_unavailable || self.page.opacity_draw_refused.get() {
            report.push_str("PAINT: document painting refused before authored pixels because opacity scene, work, memory or internal consistency limits prevent a complete content frame. Browser chrome and inspector remain available.\n");
        }
        report
    }

    pub(super) fn opacity_paint_bounds(
        &self,
        viewport: ClipRect,
    ) -> Option<(Vec<Option<ClipRect>>, usize)> {
        let groups = &self.page.opacity_groups;
        let mut work = groups
            .len()
            .checked_mul(4)?
            .checked_add(self.page.items.len())?;
        if work > crate::paint::MAX_CANVAS_PIXELS {
            return None;
        }
        let mut bounds = Vec::new();
        bounds.try_reserve_exact(groups.len()).ok()?;
        bounds.resize(groups.len(), None);
        let mut stack: Vec<(usize, bool)> = Vec::new();
        stack.try_reserve_exact(128).ok()?;
        let mut next = 0;
        let top = self.content_top();
        let bottom = self.content_bottom();
        let dy = top - self.scroll;
        for (index, item) in self.page.items.iter().enumerate() {
            while stack
                .last()
                .is_some_and(|&(group, _)| groups[group].end <= index)
            {
                let (group, _) = stack.pop()?;
                if let Some(&(parent, _)) = stack.last() {
                    let child = bounds[group];
                    include(&mut bounds[parent], child);
                }
            }
            while next < groups.len() && groups[next].start == index {
                if stack.len() == 128 {
                    return None;
                }
                let hidden =
                    groups[next].opacity == 0.0 || stack.last().is_some_and(|&(_, hidden)| hidden);
                stack.push((next, hidden));
                next += 1;
            }
            let Some(&(group, hidden)) = stack.last() else {
                continue;
            };
            if hidden {
                continue;
            }
            let clip = self
                .page
                .paint_clip(index)
                .map_or(viewport, |clip| viewport.intersect(clip.translate_y(dy)));
            if clip.left == clip.right || clip.top == clip.bottom {
                continue;
            }
            let rect = match *item {
                PaintItem::HitBox { .. } => continue,
                PaintItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    ..
                }
                | PaintItem::RoundedRect {
                    x,
                    y,
                    width,
                    height,
                    ..
                }
                | PaintItem::Background {
                    x,
                    y,
                    width,
                    height,
                    ..
                }
                | PaintItem::Image {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => ClipRect::new(x, y.saturating_add(dy), width, height),
                PaintItem::Control(control) => {
                    if self.view != View::Page {
                        continue;
                    }
                    let rect = &self.page.controls[control];
                    ClipRect::new(rect.x, rect.y.saturating_add(dy), rect.width, rect.height)
                }
                PaintItem::Text(run) => {
                    let run = &self.page.runs[run];
                    let y = run.y.saturating_add(dy);
                    // Match the existing painter's vertical admission before
                    // inspecting bounded glyph data and overhangs.
                    if y.saturating_add(run.height()) <= top || y >= bottom {
                        continue;
                    }
                    let visits = run
                        .shaped
                        .as_ref()
                        .map_or(run.text.len(), crate::text::Shape::len);
                    work = work.checked_add(visits)?;
                    if work > crate::paint::MAX_CANVAS_PIXELS {
                        return None;
                    }
                    let ink = run.opacity_paint_bounds();
                    ClipRect {
                        left: run.x.saturating_add(ink.left),
                        right: run.x.saturating_add(ink.right),
                        top: y.saturating_add(ink.top),
                        bottom: y.saturating_add(ink.bottom),
                    }
                }
            };
            include(&mut bounds[group], Some(rect.intersect(clip)));
        }
        while let Some((group, _)) = stack.pop() {
            if let Some(&(parent, _)) = stack.last() {
                let child = bounds[group];
                include(&mut bounds[parent], child);
            }
        }
        Some((bounds, work))
    }
}
