//! Clipped circular color backgrounds. Pixel coverage is composited once;
//! rectangles at radius zero use the unchanged square painting fast path.
use super::*;

impl Canvas {
    #[allow(clippy::too_many_arguments)]
    pub fn fill_rounded_rect_alpha(
        &mut self, x: i32, y: i32, width: i32, height: i32,
        radius: f32, color: u32, alpha: u8,
    ) {
        if width <= 0 || height <= 0 || alpha == 0 || !radius.is_finite() {
            return;
        }
        // Equal circular corners use the specification's common overlap
        // reduction: min(1, width/(2r), height/(2r)). Re-evaluate at final size.
        let radius = f64::from(radius.max(0.0)).min(f64::from(width.min(height)) * 0.5);
        if radius == 0.0 {
            self.fill_rect_alpha(x, y, width, height, color, alpha);
            return;
        }
        let bounds = ClipRect::new(x, y, width, height).intersect(self.clip);
        let band = radius.ceil() as i64;
        let inner_left = (i64::from(x) + band).clamp(i64::from(bounds.left), i64::from(bounds.right)) as i32;
        let inner_right = (i64::from(x) + i64::from(width) - band)
            .clamp(i64::from(inner_left), i64::from(bounds.right)) as i32;
        for row in bounds.top..bounds.bottom {
            let py = (i64::from(row) - i64::from(y)) as f64 + 0.5;
            if py >= radius && py <= f64::from(height) - radius {
                self.fill_rect_alpha(bounds.left, row, bounds.right - bounds.left, 1, color, alpha);
                continue;
            }
            self.fill_rect_alpha(inner_left, row, inner_right - inner_left, 1, color, alpha);
            for col in (bounds.left..inner_left).chain(inner_right..bounds.right) {
                let px = (i64::from(col) - i64::from(x)) as f64 + 0.5;
                let dx = (radius - px).max(px - (f64::from(width) - radius)).max(0.0);
                let dy = (radius - py).max(py - (f64::from(height) - radius)).max(0.0);
                let coverage = ((radius + 0.5 - dx.hypot(dy)).clamp(0.0, 1.0) * 255.0).round() as u16;
                let coverage = ((coverage * u16::from(alpha) + 127) / 255) as u8;
                self.blend_pixel(col, row, color, coverage);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_radius_canvas_corners_overlap_and_square_fast_path() {
        let mut canvas = Canvas::new(84, 44);
        canvas.fill_rect(0, 0, 84, 44, 0xffffff);
        canvas.fill_rounded_rect_alpha(2, 2, 80, 40, 8.0, 0x1464dc, 255);
        for (x, y) in [(2, 2), (81, 2), (2, 41), (81, 41)] {
            assert_eq!(canvas.pixels[y * canvas.width + x], 0xffffff);
        }
        assert_eq!(canvas.pixels[22 * canvas.width + 42], 0x1464dc);
        let mut huge = Canvas::new(8, 4);
        let mut reduced = Canvas::new(8, 4);
        huge.fill_rounded_rect_alpha(0, 0, 8, 4, 1000.0, 0x1464dc, 255);
        reduced.fill_rounded_rect_alpha(0, 0, 8, 4, 2.0, 0x1464dc, 255);
        assert_eq!(huge.pixels, reduced.pixels, "Adjacent equal radii shrink together");
        let mut square = Canvas::new(8, 4);
        let mut plain = Canvas::new(8, 4);
        square.fill_rounded_rect_alpha(-2, -1, 7, 4, 0.0, 0x1464dc, 128);
        plain.fill_rect_alpha(-2, -1, 7, 4, 0x1464dc, 128);
        assert_eq!(square.pixels, plain.pixels);
    }

    #[test]
    fn css_radius_canvas_clip_alpha_and_extreme_coordinates_are_bounded() {
        let mut canvas = Canvas::new(12, 12);
        canvas.fill_rect(0, 0, 12, 12, 0xffffff);
        let clip = ClipRect::new(3, 2, 6, 7);
        canvas.set_clip(Some(clip));
        canvas.fill_rounded_rect_alpha(1, 1, 10, 10, 3.0, 0, 128);
        assert_eq!(canvas.clip(), clip);
        assert_eq!(canvas.pixels[5 * 12 + 5], 0x7f7f7f);
        for y in 0..12 { for x in 0..12 {
            let value = canvas.pixels[y * 12 + x];
            if !clip.contains(x as i32, y as i32) { assert_eq!(value, 0xffffff); }
            assert!(value & 255 >= 127, "An edge cannot be composited twice");
        }}
        let before = canvas.pixels.clone();
        for (x, y, w, h, radius, alpha) in [
            (i32::MIN, i32::MIN, 20, 20, 8.0, 255),
            (i32::MAX, i32::MAX, i32::MAX, i32::MAX, 8.0, 255),
            (0, 0, 10, 10, f32::NAN, 255), (0, 0, 10, 10, 8.0, 0),
        ] { canvas.fill_rounded_rect_alpha(x, y, w, h, radius, 0, alpha); }
        assert_eq!(canvas.pixels, before);
    }
}
