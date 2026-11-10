//! ChezCat's original browser chrome graphics. The small geometric glyphs are
//! authored here rather than loaded from an icon font, image pack, or toolkit.

use crate::paint::Canvas;
use std::f32::consts::PI;

pub const NAVY: u32 = 0x0b153c;
pub const NAVY_LIGHT: u32 = 0x18284a;
pub const CREAM: u32 = 0xfff3dc;
pub const TEAL: u32 = 0x42c7bc;
pub const TEAL_DARK: u32 = 0x167d78;
pub const SURFACE: u32 = 0xf3f5f4;
pub const CARD: u32 = 0xffffff;
pub const BORDER: u32 = 0xd7e1e3;
pub const INK: u32 = 0x182647;
pub const MUTED: u32 = 0x5e7184;
pub const ERROR: u32 = 0xa63f48;
pub const ERROR_BG: u32 = 0xf9e7e6;
pub const SELECTED: u32 = 0xdcefe9;
pub const DISABLED: u32 = 0x60718b;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Back,
    Forward,
    Home,
    Refresh,
    Stop,
    Menu,
    Search,
    Close,
    Bookmark,
    Bookmarks,
    History,
    Settings,
    Plus,
    Minus,
    Reset,
    File,
    Code,
    Activity,
    Check,
    Up,
    Down,
}

fn coverage(distance: f32) -> u8 {
    ((0.5 - distance).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn surface_bounds(canvas: &Canvas, rect: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let (x, y, w, h) = rect;
    let right = i64::from(x) + i64::from(w.max(0));
    let bottom = i64::from(y) + i64::from(h.max(0));
    (
        x.max(0).min(canvas.width as i32),
        y.max(0).min(canvas.height as i32),
        right.clamp(0, canvas.width as i64) as i32,
        bottom.clamp(0, canvas.height as i64) as i32,
    )
}

fn rounded_distance(px: f32, py: f32, width: f32, height: f32, radius: f32) -> f32 {
    let qx = (px - width * 0.5).abs() - (width * 0.5 - radius);
    let qy = (py - height * 0.5).abs() - (height * 0.5 - radius);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
}

/// Fill a rounded rectangle with a one-pixel antialiased edge. Only its corner
/// bands need per-pixel geometry; the flat interior uses Canvas's row fills.
pub fn rounded_rect(canvas: &mut Canvas, rect: (i32, i32, i32, i32), radius: f32, color: u32) {
    let (x, y, w, h) = rect;
    if w <= 0 || h <= 0 || !radius.is_finite() {
        return;
    }
    let radius = radius.max(0.0).min(w.min(h) as f32 * 0.5);
    if radius == 0.0 {
        canvas.fill_rect(x, y, w, h, color);
        return;
    }
    let (left, top, right, bottom) = surface_bounds(canvas, rect);
    let band = radius.ceil() as i64;
    let inner_left =
        (i64::from(x) + band).clamp(i64::from(left), i64::from(right.max(left))) as i32;
    let inner_right = (i64::from(x) + i64::from(w) - band)
        .clamp(i64::from(inner_left), i64::from(right.max(inner_left)))
        as i32;
    for row in top..bottom {
        let py = (i64::from(row) - i64::from(y)) as f32 + 0.5;
        if py >= radius && py <= h as f32 - radius {
            canvas.fill_rect(left, row, right - left, 1, color);
            continue;
        }
        canvas.fill_rect(inner_left, row, inner_right - inner_left, 1, color);
        for col in (left..inner_left).chain(inner_right..right) {
            let px = (i64::from(col) - i64::from(x)) as f32 + 0.5;
            let alpha = coverage(rounded_distance(px, py, w as f32, h as f32, radius));
            canvas.blend_pixel(col, row, color, alpha);
        }
    }
}

/// Paint an inset rounded outline. The untouched center stays transparent.
pub fn rounded_border(
    canvas: &mut Canvas,
    rect: (i32, i32, i32, i32),
    radius: f32,
    width: f32,
    color: u32,
) {
    let (x, y, w, h) = rect;
    if w <= 0 || h <= 0 || !radius.is_finite() || !width.is_finite() || width <= 0.0 {
        return;
    }
    let radius = radius.max(0.0).min(w.min(h) as f32 * 0.5);
    let width = width.min(w.min(h) as f32 * 0.5);
    let (left, top, right, bottom) = surface_bounds(canvas, rect);
    let band = (radius.max(width) + 1.0).ceil() as i64;
    let inner_left =
        (i64::from(x) + band).clamp(i64::from(left), i64::from(right.max(left))) as i32;
    let inner_right = (i64::from(x) + i64::from(w) - band)
        .clamp(i64::from(inner_left), i64::from(right.max(inner_left)))
        as i32;
    let has_hole = w as f32 > width * 2.0 && h as f32 > width * 2.0;
    for row in top..bottom {
        let py = (i64::from(row) - i64::from(y)) as f32 + 0.5;
        let scan_full = py <= band as f32 || py >= h as f32 - band as f32;
        let spans = if scan_full {
            [(left, right), (right, right)]
        } else {
            [(left, inner_left), (inner_right, right)]
        };
        for (start, end) in spans {
            for col in start..end {
                let px = (i64::from(col) - i64::from(x)) as f32 + 0.5;
                let outer = coverage(rounded_distance(px, py, w as f32, h as f32, radius));
                let inner = if has_hole {
                    coverage(rounded_distance(
                        px - width,
                        py - width,
                        w as f32 - 2.0 * width,
                        h as f32 - 2.0 * width,
                        (radius - width).max(0.0),
                    ))
                } else {
                    0
                };
                canvas.blend_pixel(col, row, color, outer.saturating_sub(inner));
            }
        }
    }
}

// Union coverage before painting: connected strokes do not gain dark seams
// from repeatedly alpha-blending their shared pixels. Two fixed stack masks
// cover every normal chrome size without allocation or a mutable icon cache.
const MAX_ICON_SIZE: usize = 64;
struct Glyph {
    size: usize,
    scale: f32,
    layers: [[u8; MAX_ICON_SIZE * MAX_ICON_SIZE]; 2],
}

impl Glyph {
    fn new(size: usize) -> Self {
        Self {
            size,
            scale: size as f32 / 24.0,
            layers: [[0; MAX_ICON_SIZE * MAX_ICON_SIZE]; 2],
        }
    }

    fn mask(
        &mut self,
        layer: usize,
        bounds: (f32, f32, f32, f32),
        distance: impl Fn(f32, f32) -> f32,
    ) {
        let (x0, y0, x1, y1) = bounds;
        let left = (x0 * self.scale - 1.0).floor().max(0.0) as usize;
        let top = (y0 * self.scale - 1.0).floor().max(0.0) as usize;
        let right = ((x1 * self.scale + 1.0).ceil() as usize).min(self.size);
        let bottom = ((y1 * self.scale + 1.0).ceil() as usize).min(self.size);
        for y in top..bottom {
            for x in left..right {
                let alpha = coverage(
                    distance((x as f32 + 0.5) / self.scale, (y as f32 + 0.5) / self.scale)
                        * self.scale,
                );
                let pixel = &mut self.layers[layer][y * self.size + x];
                *pixel = (*pixel).max(alpha);
            }
        }
    }

    fn line(&mut self, layer: usize, a: (f32, f32), b: (f32, f32), width: f32) {
        let radius = width * 0.5;
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let length_squared = dx * dx + dy * dy;
        self.mask(
            layer,
            (
                a.0.min(b.0) - radius,
                a.1.min(b.1) - radius,
                a.0.max(b.0) + radius,
                a.1.max(b.1) + radius,
            ),
            |x, y| {
                let t = if length_squared > 0.0 {
                    ((x - a.0) * dx + (y - a.1) * dy) / length_squared
                } else {
                    0.0
                }
                .clamp(0.0, 1.0);
                (x - a.0 - t * dx).hypot(y - a.1 - t * dy) - radius
            },
        );
    }

    fn path(&mut self, layer: usize, points: &[(f32, f32)]) {
        for edge in points.windows(2) {
            self.line(layer, edge[0], edge[1], 1.85);
        }
    }

    fn circle(&mut self, layer: usize, cx: f32, cy: f32, radius: f32, width: f32) {
        let extent = radius + width * 0.5;
        self.mask(
            layer,
            (cx - extent, cy - extent, cx + extent, cy + extent),
            |x, y| ((x - cx).hypot(y - cy) - radius).abs() - width * 0.5,
        );
    }

    fn arc(&mut self, layer: usize, center: (f32, f32), radius: f32, start: f32, end: f32) {
        let steps = ((end - start).abs() / 12.0).ceil() as usize;
        let point = |angle: f32| {
            (
                center.0 + angle.to_radians().cos() * radius,
                center.1 + angle.to_radians().sin() * radius,
            )
        };
        for step in 0..steps {
            self.line(
                layer,
                point(start + (end - start) * step as f32 / steps as f32),
                point(start + (end - start) * (step + 1) as f32 / steps as f32),
                1.85,
            );
        }
    }
}

/// Draw an original 24-unit glyph at 1–64 pixels. The accent is a second color,
/// so disabled or monochrome glyphs simply pass the same color for both.
pub fn icon(
    canvas: &mut Canvas,
    kind: Icon,
    x: i32,
    y: i32,
    size: i32,
    foreground: u32,
    accent: u32,
) {
    if size <= 0 || size > MAX_ICON_SIZE as i32 {
        return;
    }
    let mut g = Glyph::new(size as usize);
    match kind {
        Icon::Back => g.path(0, &[(15.0, 5.0), (8.0, 12.0), (15.0, 19.0)]),
        Icon::Forward => g.path(0, &[(9.0, 5.0), (16.0, 12.0), (9.0, 19.0)]),
        Icon::Up => g.path(0, &[(5.0, 15.0), (12.0, 8.0), (19.0, 15.0)]),
        Icon::Down => g.path(0, &[(5.0, 9.0), (12.0, 16.0), (19.0, 9.0)]),
        Icon::Home => {
            g.path(0, &[(3.3, 11.0), (12.0, 3.5), (20.7, 11.0)]);
            g.path(0, &[(5.8, 9.4), (5.8, 20.0), (18.2, 20.0), (18.2, 9.4)]);
            g.path(1, &[(10.0, 19.7), (10.0, 14.3), (14.0, 14.3), (14.0, 19.7)]);
        }
        Icon::Refresh => {
            g.arc(0, (12.0, 12.0), 7.5, 43.0, 315.0);
            g.path(1, &[(18.9, 3.6), (18.9, 8.5), (14.0, 8.5)]);
        }
        Icon::Stop => {
            g.mask(0, (6.5, 6.5, 17.5, 17.5), |x, y| {
                rounded_distance(x - 6.5, y - 6.5, 11.0, 11.0, 1.6)
            });
        }
        Icon::Menu => {
            g.path(0, &[(5.0, 6.0), (19.0, 6.0)]);
            g.path(0, &[(5.0, 12.0), (19.0, 12.0)]);
            g.path(1, &[(5.0, 18.0), (19.0, 18.0)]);
        }
        Icon::Search => {
            g.circle(0, 10.4, 10.4, 6.3, 1.85);
            g.path(1, &[(15.0, 15.0), (20.0, 20.0)]);
        }
        Icon::Close => {
            g.path(0, &[(6.0, 6.0), (18.0, 18.0)]);
            g.path(0, &[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::Bookmark => {
            g.path(
                0,
                &[
                    (6.3, 4.0),
                    (17.7, 4.0),
                    (17.7, 20.5),
                    (12.0, 16.5),
                    (6.3, 20.5),
                    (6.3, 4.0),
                ],
            );
        }
        Icon::Bookmarks => {
            g.path(1, &[(9.5, 3.0), (20.0, 3.0), (20.0, 17.0)]);
            g.path(
                0,
                &[
                    (4.0, 6.0),
                    (15.5, 6.0),
                    (15.5, 21.0),
                    (9.75, 17.3),
                    (4.0, 21.0),
                    (4.0, 6.0),
                ],
            );
        }
        Icon::History => {
            g.arc(0, (12.5, 12.0), 7.6, -130.0, 153.0);
            g.path(1, &[(3.5, 4.5), (3.5, 9.2), (8.2, 9.2)]);
            g.path(0, &[(12.5, 7.5), (12.5, 12.0), (16.0, 14.0)]);
        }
        Icon::Settings => {
            g.circle(0, 12.0, 12.0, 6.7, 1.85);
            g.circle(1, 12.0, 12.0, 2.5, 1.75);
            for tooth in 0..8 {
                let a = tooth as f32 * PI / 4.0;
                g.line(
                    0,
                    (12.0 + 6.4 * a.cos(), 12.0 + 6.4 * a.sin()),
                    (12.0 + 9.2 * a.cos(), 12.0 + 9.2 * a.sin()),
                    2.9,
                );
            }
        }
        Icon::Plus => {
            g.path(0, &[(5.0, 12.0), (19.0, 12.0)]);
            g.path(0, &[(12.0, 5.0), (12.0, 19.0)]);
        }
        Icon::Minus => g.path(0, &[(5.0, 12.0), (19.0, 12.0)]),
        Icon::Reset => {
            g.arc(0, (12.0, 12.0), 7.3, -133.0, 152.0);
            g.path(1, &[(3.5, 4.0), (3.5, 9.0), (8.5, 9.0)]);
            g.path(0, &[(9.0, 12.0), (15.0, 12.0)]);
        }
        Icon::File => {
            g.path(
                0,
                &[
                    (13.5, 3.5),
                    (5.8, 3.5),
                    (5.8, 20.5),
                    (18.2, 20.5),
                    (18.2, 8.5),
                    (13.5, 3.5),
                ],
            );
            g.path(1, &[(13.0, 4.0), (13.0, 9.0), (17.7, 9.0)]);
            g.path(0, &[(9.0, 13.0), (15.0, 13.0)]);
            g.path(0, &[(9.0, 16.5), (14.0, 16.5)]);
        }
        Icon::Code => {
            g.path(0, &[(7.0, 7.0), (2.5, 12.0), (7.0, 17.0)]);
            g.path(0, &[(17.0, 7.0), (21.5, 12.0), (17.0, 17.0)]);
            g.path(1, &[(14.0, 4.5), (10.0, 19.5)]);
        }
        Icon::Activity => {
            g.path(
                0,
                &[
                    (2.5, 12.0),
                    (6.5, 12.0),
                    (9.5, 5.5),
                    (14.0, 19.0),
                    (17.0, 12.0),
                    (21.5, 12.0),
                ],
            );
        }
        Icon::Check => g.path(0, &[(5.0, 12.0), (10.0, 17.0), (19.0, 7.0)]),
    }
    for (layer, color) in [foreground, accent].into_iter().enumerate() {
        for row in 0..g.size {
            for col in 0..g.size {
                let alpha = g.layers[layer][row * g.size + col];
                if alpha > 0 {
                    canvas.blend_pixel(
                        x.saturating_add(col as i32),
                        y.saturating_add(row as i32),
                        color,
                        alpha,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::ClipRect;

    #[test]
    fn chrome_edges_respect_clip_and_keep_outline_center_clear() {
        let mut canvas = Canvas::new(32, 32);
        canvas.fill_rect(0, 0, 32, 32, NAVY);
        canvas.set_clip(Some(ClipRect::new(8, 8, 16, 16)));
        rounded_rect(&mut canvas, (2, 2, 28, 28), 7.0, CREAM);
        rounded_border(&mut canvas, (0, 0, 32, 32), 8.0, 2.0, TEAL);
        icon(&mut canvas, Icon::Home, 4, 4, 24, INK, TEAL_DARK);
        for y in 0..32 {
            for x in 0..32 {
                if !(8..24).contains(&x) || !(8..24).contains(&y) {
                    assert_eq!(canvas.pixels[y * 32 + x], NAVY);
                }
            }
        }
        let mut outline = Canvas::new(32, 32);
        outline.fill_rect(0, 0, 32, 32, CREAM);
        rounded_border(&mut outline, (2, 2, 28, 28), 8.0, 1.5, INK);
        assert_eq!(outline.pixels[16 * 32 + 16], CREAM);
        assert_eq!(outline.pixels[2 * 32 + 16], INK);
        assert_eq!(outline.pixels[2 * 32 + 2], CREAM);
    }

    #[test]
    fn glyphs_have_coverage_and_round_edges_without_touching_adjacent_pixels() {
        for kind in [Icon::Home, Icon::Refresh, Icon::Settings, Icon::History] {
            let mut canvas = Canvas::new(32, 32);
            icon(&mut canvas, kind, 4, 4, 24, 0xffffff, 0xffffff);
            assert!(canvas.pixels.contains(&0xffffff));
            assert!(canvas
                .pixels
                .iter()
                .any(|&pixel| pixel != 0 && pixel != 0xffffff));
            assert!(canvas.pixels[0..32 * 4].iter().all(|&pixel| pixel == 0));
            assert!(canvas.pixels[32 * 28..].iter().all(|&pixel| pixel == 0));
        }
    }
}
