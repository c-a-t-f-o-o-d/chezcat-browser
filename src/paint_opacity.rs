//! Normal source-over group opacity on the existing opaque RGB destination.
//! If a group paints C + (1-A)B, interpolation with its saved backdrop B gives
//! qC + (1-qA)B. Thus overlapping descendants are combined before opacity.
//! This relies on the current normal-blend painter, not arbitrary blend modes.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct OpacityGroup {
    pub start: usize,
    pub end: usize,
    pub opacity: f64,
}

struct Active {
    end: usize,
    opacity: f64,
    offset: usize,
    hidden: bool,
    rect: ClipRect,
    area: usize,
}

pub(crate) struct OpacityPainter<'a> {
    groups: &'a [OpacityGroup],
    next: usize,
    active: Vec<Active>,
    snapshots: Vec<u32>,
    used: usize,
    regions: Vec<ClipRect>,
}

impl<'a> OpacityPainter<'a> {
    /// Admit all snapshot work and simultaneous storage before any group paint.
    /// Regions are conservative unions of actual paint, already accounting for
    /// descendants, glyph overhang and current fixed/scroll geometry. The
    /// opacity=1 path retains no group records and needs no scratch allocation.
    pub(crate) fn new(
        canvas: &Canvas,
        groups: &'a [OpacityGroup],
        regions: &[Option<ClipRect>],
        viewport: ClipRect,
        item_count: usize,
        setup_work: usize,
    ) -> Option<Self> {
        Self::with_limit(
            canvas,
            groups,
            regions,
            viewport,
            item_count,
            setup_work,
            MAX_CANVAS_PIXELS,
        )
    }

    fn with_limit(
        canvas: &Canvas,
        groups: &'a [OpacityGroup],
        bounds: &[Option<ClipRect>],
        viewport: ClipRect,
        item_count: usize,
        setup_work: usize,
        limit: usize,
    ) -> Option<Self> {
        if bounds.len() != groups.len() {
            return None;
        }
        let viewport = viewport.intersect(ClipRect::new(
            0,
            0,
            canvas.width as i32,
            canvas.height as i32,
        ));
        let mut stack: Vec<(usize, bool, usize)> = Vec::new();
        stack.try_reserve_exact(128).ok()?;
        let mut simultaneous = 0usize;
        let mut peak = 0usize;
        let mut total = setup_work;
        let mut previous = 0usize;
        let mut regions = Vec::new();
        regions.try_reserve_exact(groups.len()).ok()?;
        for (group, bounds) in groups.iter().zip(bounds) {
            if group.start < previous
                || group.start >= group.end
                || group.end > item_count
                || !group.opacity.is_finite()
                || !(0.0..1.0).contains(&group.opacity)
            {
                return None;
            }
            previous = group.start;
            while stack.last().is_some_and(|&(end, _, _)| end <= group.start) {
                simultaneous -= stack.pop()?.2;
            }
            if stack.len() == 128 || stack.last().is_some_and(|&(end, _, _)| group.end > end) {
                return None;
            }
            let hidden = group.opacity == 0.0 || stack.last().is_some_and(|&(_, hidden, _)| hidden);
            let rect = bounds.map_or(ClipRect::new(0, 0, 0, 0), |rect| rect.intersect(viewport));
            let area = if hidden {
                0
            } else {
                ((rect.right - rect.left) as usize)
                    .checked_mul((rect.bottom - rect.top) as usize)?
            };
            if area != 0 {
                simultaneous = simultaneous.checked_add(area)?;
                total = total.checked_add(area.checked_mul(2)?)?;
                peak = peak.max(simultaneous);
            }
            regions.push(rect);
            stack.push((group.end, hidden, area));
        }
        // Canvas + simultaneous backdrops share the existing 256 MiB surface
        // envelope. Pixel visits (capture/composite/initialization) are finite
        // independently of nesting and of how often the App redraws.
        if canvas.pixels.len().checked_add(peak)? > limit || total.checked_add(peak)? > limit {
            return None;
        }
        let mut snapshots = Vec::new();
        snapshots.try_reserve_exact(peak).ok()?;
        snapshots.resize(peak, 0);
        let mut active = Vec::new();
        active.try_reserve_exact(stack.capacity()).ok()?;
        Some(Self {
            groups,
            next: 0,
            active,
            snapshots,
            used: 0,
            regions,
        })
    }

    pub(crate) fn advance(&mut self, canvas: &mut Canvas, item: usize) {
        while self.active.last().is_some_and(|group| group.end <= item) {
            self.close(canvas);
        }
        while self.next < self.groups.len() && self.groups[self.next].start == item {
            let group = self.groups[self.next];
            let rect = self.regions[self.next];
            self.next += 1;
            let hidden = group.opacity == 0.0 || self.hidden();
            let area = if hidden {
                0
            } else {
                (rect.right - rect.left) as usize * (rect.bottom - rect.top) as usize
            };
            let offset = self.used;
            if area != 0 {
                let width = (rect.right - rect.left) as usize;
                for (row, y) in (rect.top as usize..rect.bottom as usize).enumerate() {
                    let start = y * canvas.width + rect.left as usize;
                    self.snapshots[offset + row * width..offset + (row + 1) * width]
                        .copy_from_slice(&canvas.pixels[start..start + width]);
                }
                self.used += area;
            }
            self.active.push(Active {
                end: group.end,
                opacity: group.opacity,
                offset,
                hidden,
                rect,
                area,
            });
        }
    }

    pub(crate) fn hidden(&self) -> bool {
        self.active.last().is_some_and(|group| group.hidden)
    }

    pub(crate) fn finish(&mut self, canvas: &mut Canvas) {
        while !self.active.is_empty() {
            self.close(canvas);
        }
    }

    fn close(&mut self, canvas: &mut Canvas) {
        let group = self.active.pop().expect("admitted active opacity context");
        if group.area == 0 {
            return;
        }
        let width = (group.rect.right - group.rect.left) as usize;
        for (row, y) in (group.rect.top as usize..group.rect.bottom as usize).enumerate() {
            for x in 0..width {
                let pixel = &mut canvas.pixels[y * canvas.width + group.rect.left as usize + x];
                let before = self.snapshots[group.offset + row * width + x];
                let mut mixed = 0;
                for shift in [0, 8, 16] {
                    let src = f64::from((*pixel >> shift) & 255);
                    let dst = f64::from((before >> shift) & 255);
                    mixed |= ((src * group.opacity + dst * (1.0 - group.opacity)).round() as u32)
                        << shift;
                }
                *pixel = mixed;
            }
        }
        self.used = group.offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_paint_canvas_overlaps_nested_backdrops_and_clipped_destination() {
        let mut canvas = Canvas::new(4, 2);
        canvas.fill_rect(0, 0, 4, 2, 0x335577);
        let groups = [
            OpacityGroup {
                start: 0,
                end: 3,
                opacity: 0.5,
            },
            OpacityGroup {
                start: 2,
                end: 3,
                opacity: 0.5,
            },
        ];
        let regions = [
            Some(ClipRect::new(1, 0, 2, 2)),
            Some(ClipRect::new(2, 0, 1, 2)),
        ];
        let mut paint =
            OpacityPainter::new(&canvas, &groups, &regions, ClipRect::new(1, 0, 2, 2), 3, 0)
                .unwrap();
        canvas.set_clip(Some(ClipRect::new(1, 0, 2, 2)));
        paint.advance(&mut canvas, 0);
        canvas.fill_rect(0, 0, 2, 2, 0x010101);
        paint.advance(&mut canvas, 1);
        canvas.fill_rect(1, 0, 2, 2, 0x010101);
        paint.advance(&mut canvas, 2);
        canvas.fill_rect(2, 0, 1, 2, 0xffffff);
        paint.finish(&mut canvas);
        assert_eq!(
            canvas.pixels,
            [0x335577, 0x1a2b3c, 0x5a6b7c, 0x335577, 0x335577, 0x1a2b3c, 0x5a6b7c, 0x335577]
        );
    }

    #[test]
    fn opacity_paint_canvas_preflight_refuses_before_pixels_or_scratch_mutation() {
        let mut canvas = Canvas::new(4, 4);
        canvas.fill_rect(0, 0, 4, 4, 0x336699);
        let before = canvas.pixels.clone();
        let groups = [OpacityGroup {
            start: 0,
            end: 1,
            opacity: 0.5,
        }];
        let regions = [Some(canvas.clip())];
        assert!(
            OpacityPainter::with_limit(&canvas, &groups, &regions, canvas.clip(), 1, 0, 31)
                .is_none()
        );
        assert_eq!(canvas.pixels, before);
        let zero = [OpacityGroup {
            opacity: 0.0,
            ..groups[0]
        }];
        let mut paint =
            OpacityPainter::with_limit(&canvas, &zero, &regions, canvas.clip(), 1, 0, 16).unwrap();
        assert!(paint.snapshots.is_empty());
        paint.advance(&mut canvas, 0);
        assert!(paint.hidden());
        paint.finish(&mut canvas);
        assert_eq!(canvas.pixels, before);
        let invalid = [
            OpacityGroup {
                start: 0,
                end: 2,
                opacity: 0.5,
            },
            OpacityGroup {
                start: 1,
                end: 3,
                opacity: 0.5,
            },
        ];
        assert!(OpacityPainter::new(
            &canvas,
            &invalid,
            &[Some(canvas.clip()); 2],
            canvas.clip(),
            3,
            0
        )
        .is_none());
    }

    #[test]
    fn opacity_paint_canvas_many_tiny_and_offscreen_groups_fit_actual_regions() {
        let mut canvas = Canvas::new(64, 64);
        canvas.fill_rect(0, 0, 64, 64, 0xffffff);
        let groups: Vec<_> = (0..128)
            .map(|i| OpacityGroup {
                start: i,
                end: i + 1,
                opacity: 0.5,
            })
            .collect();
        let regions: Vec<_> = (0..128)
            .map(|i| {
                Some(ClipRect::new(
                    if i < 64 { 10 + (i % 8) as i32 } else { 100 },
                    10 + (i / 8) as i32,
                    1,
                    1,
                ))
            })
            .collect();
        let mut paint =
            OpacityPainter::with_limit(&canvas, &groups, &regions, canvas.clip(), 128, 128, 5000)
                .unwrap();
        assert_eq!(
            paint.snapshots.len(),
            1,
            "only one 1px backdrop is simultaneously needed"
        );
        for i in 0..128 {
            paint.advance(&mut canvas, i);
            let rect = regions[i].unwrap();
            canvas.fill_rect(rect.left, rect.top, 1, 1, 0x010101);
        }
        paint.finish(&mut canvas);
        for y in 0..64 {
            for x in 0..64 {
                assert_eq!(
                    canvas.pixels[y * 64 + x],
                    if (10..18).contains(&x) && (10..18).contains(&y) {
                        0x808080
                    } else {
                        0xffffff
                    }
                );
            }
        }
    }
}
