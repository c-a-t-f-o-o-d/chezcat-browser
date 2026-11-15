pub const GLYPH_WIDTH: i32 = 5;
pub const GLYPH_ADVANCE: i32 = 6;
pub const GLYPH_HEIGHT: i32 = 7;

// Limit a single surface to 256 MiB, including on allocation requests from
// unexpected window dimensions. An unavailable surface is represented as empty.
const MAX_CANVAS_PIXELS: usize = 64 * 1024 * 1024;

#[path = "paint_area.rs"]
mod area;

#[cfg(test)]
#[path = "paint_resample_tests.rs"]
mod resample_tests;

#[cfg(test)]
#[path = "paint_background_tests.rs"]
mod background_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ClipRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: x.saturating_add(width.max(0)),
            bottom: y.saturating_add(height.max(0)),
        }
    }

    pub fn intersect(self, other: Self) -> Self {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        Self {
            left,
            top,
            right: self.right.min(other.right).max(left),
            bottom: self.bottom.min(other.bottom).max(top),
        }
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn translate_y(self, delta: i32) -> Self {
        Self {
            top: self.top.saturating_add(delta),
            bottom: self.bottom.saturating_add(delta),
            ..self
        }
    }
}

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    clip: ClipRect,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        let empty = || Self {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            clip: ClipRect::new(0, 0, 0, 0),
        };
        let Some(count) = width.checked_mul(height) else {
            return empty();
        };
        if count == 0 || count > MAX_CANVAS_PIXELS {
            return empty();
        }

        let mut pixels = Vec::new();
        if pixels.try_reserve_exact(count).is_err() {
            return empty();
        }
        pixels.resize(count, 0);
        Self {
            width,
            height,
            pixels,
            clip: ClipRect::new(0, 0, width as i32, height as i32),
        }
    }

    pub fn set_clip(&mut self, clip: Option<ClipRect>) {
        let surface = ClipRect::new(0, 0, self.width as i32, self.height as i32);
        self.clip = clip.map_or(surface, |clip| surface.intersect(clip));
    }

    pub fn clip(&self) -> ClipRect {
        self.clip
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.fill_rect_wide(
            i64::from(x),
            i64::from(y),
            i64::from(w),
            i64::from(h),
            color,
        );
    }

    /// Blend one RGB pixel with an eight-bit coverage value. Like the other
    /// painting operations, this observes both the surface bounds and clip.
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: u32, coverage: u8) {
        if coverage == 0 || !self.clip.contains(x, y) || x < 0 || y < 0 {
            return;
        }
        if x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let Some(index) = (y as usize)
            .checked_mul(self.width)
            .and_then(|row| row.checked_add(x as usize))
        else {
            return;
        };
        let Some(pixel) = self.pixels.get_mut(index) else {
            return;
        };
        let color = color & 0x00ff_ffff;
        if coverage == 255 {
            *pixel = color;
            return;
        }
        let alpha = u32::from(coverage);
        let inverse = 255 - alpha;
        let channel = |shift: u32| -> u32 {
            ((((color >> shift) & 255) * alpha + ((*pixel >> shift) & 255) * inverse + 127) / 255)
                << shift
        };
        *pixel = channel(16) | channel(8) | channel(0);
    }

    fn fill_rect_wide(&mut self, x: i64, y: i64, w: i64, h: i64, color: u32) {
        if w <= 0 || h <= 0 || self.pixels.is_empty() {
            return;
        }
        let width = self.width.min(i64::MAX as usize) as i64;
        let height = self.height.min(i64::MAX as usize) as i64;
        let left = x.clamp(0, width).max(i64::from(self.clip.left));
        let top = y.clamp(0, height).max(i64::from(self.clip.top));
        let right = x
            .saturating_add(w)
            .clamp(0, width)
            .min(i64::from(self.clip.right));
        let bottom = y
            .saturating_add(h)
            .clamp(0, height)
            .min(i64::from(self.clip.bottom));
        if left >= right || top >= bottom {
            return;
        }

        let color = color & 0x00ff_ffff;
        for row in top as usize..bottom as usize {
            let Some(start) = row
                .checked_mul(self.width)
                .and_then(|offset| offset.checked_add(left as usize))
            else {
                return;
            };
            if start >= self.pixels.len() {
                return;
            }
            let end = start
                .saturating_add((right - left) as usize)
                .min(self.pixels.len());
            self.pixels[start..end].fill(color);
        }
    }

    /// Draw the built-in pixel font. Newlines advance nine scaled pixels;
    /// tabs advance four character cells. Unsupported characters use a box.
    pub fn text(&mut self, x: i32, y: i32, text: &str, scale: i32, color: u32) {
        self.styled_text(x, y, text, scale, color, false);
    }

    pub fn styled_text(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        scale: i32,
        color: u32,
        italic: bool,
    ) {
        if scale <= 0 || self.pixels.is_empty() {
            return;
        }

        let scale = i64::from(scale);
        let advance = i64::from(GLYPH_ADVANCE) * scale;
        let glyph_width = i64::from(GLYPH_WIDTH) * scale;
        let glyph_height = i64::from(GLYPH_HEIGHT) * scale;
        let line_height = i64::from(GLYPH_HEIGHT + 2) * scale;
        let width = self.width.min(i64::MAX as usize) as i64;
        let height = self.height.min(i64::MAX as usize) as i64;
        let origin_x = i64::from(x);
        let mut pen_x = origin_x;
        let mut pen_y = i64::from(y);

        for character in text.chars() {
            match character {
                '\n' => {
                    pen_x = origin_x;
                    pen_y = pen_y.saturating_add(line_height);
                    continue;
                }
                '\r' => continue,
                '\t' => {
                    pen_x = pen_x.saturating_add(advance * 4);
                    continue;
                }
                _ => {}
            }

            if pen_x < width
                && pen_y < height
                && pen_x.saturating_add(glyph_width) > 0
                && pen_y.saturating_add(glyph_height) > 0
            {
                let rows = glyph(character);
                for (row, bits) in rows.iter().enumerate() {
                    for column in 0..GLYPH_WIDTH {
                        if bits & (1 << (GLYPH_WIDTH - 1 - column)) != 0 {
                            self.fill_rect_wide(
                                pen_x.saturating_add(
                                    i64::from(column) * scale
                                        + if italic {
                                            (6 - row as i64) * scale / 3
                                        } else {
                                            0
                                        },
                                ),
                                pen_y.saturating_add(row as i64 * scale),
                                scale,
                                scale,
                                color,
                            );
                        }
                    }
                }
            }
            pen_x = pen_x.saturating_add(advance);
        }
    }

    /// Smooth scaling on our surface. Source pixels remain straight alpha;
    /// interpolation and compositing use premultiplied contributions to avoid
    /// colored fringes from RGB stored underneath transparent pixels.
    pub fn image(&mut self, x: i32, y: i32, w: i32, h: i32, bitmap: &crate::image::Bitmap) {
        self.image_filtered(x, y, w, h, bitmap, crate::css::ImageRendering::Auto);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn image_filtered(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        bitmap: &crate::image::Bitmap,
        rendering: crate::css::ImageRendering,
    ) {
        if w <= 0 || h <= 0 || bitmap.width == 0 || bitmap.height == 0 || self.pixels.is_empty() {
            return;
        }
        let Some(count) = bitmap.width.checked_mul(bitmap.height) else {
            return;
        };
        if count > bitmap.pixels.len()
            || count > crate::image::MAX_PIXELS
            || bitmap.width > 16_384
            || bitmap.height > 16_384
        {
            return;
        }
        let left = i64::from(x)
            .clamp(0, self.width as i64)
            .max(i64::from(self.clip.left)) as usize;
        let top = i64::from(y)
            .clamp(0, self.height as i64)
            .max(i64::from(self.clip.top)) as usize;
        let right = (i64::from(x) + i64::from(w))
            .clamp(0, self.width as i64)
            .min(i64::from(self.clip.right)) as usize;
        let bottom = (i64::from(y) + i64::from(h))
            .clamp(0, self.height as i64)
            .min(i64::from(self.clip.bottom)) as usize;
        if left >= right || top >= bottom {
            return;
        }
        if matches!(
            rendering,
            crate::css::ImageRendering::Auto
                | crate::css::ImageRendering::Smooth
                | crate::css::ImageRendering::HighQuality
        ) && ((w as usize) < bitmap.width || (h as usize) < bitmap.height)
            && area::image(
                self,
                (x, y),
                (w as usize, h as usize),
                bitmap,
                (left, top, right, bottom),
            )
        {
            return;
        }
        // Only visible columns are prepared. No intermediate scaled bitmap is
        // allocated, even for extreme CSS dimensions or image-rendering's
        // virtual integer expansion. At most 16K small column descriptors.
        let mut columns = Vec::new();
        if right - left <= 16_384 && columns.try_reserve_exact(right - left).is_ok() {
            for px in left..right {
                columns.push(image_axis(
                    (px as i64 - i64::from(x)) as u64,
                    w as u64,
                    bitmap.width,
                    rendering,
                ));
            }
        }
        for py in top..bottom {
            let vertical = image_axis(
                (py as i64 - i64::from(y)) as u64,
                h as u64,
                bitmap.height,
                rendering,
            );
            for px in left..right {
                let horizontal = columns.get(px - left).copied().unwrap_or_else(|| {
                    image_axis(
                        (px as i64 - i64::from(x)) as u64,
                        w as u64,
                        bitmap.width,
                        rendering,
                    )
                });
                let dst = &mut self.pixels[py * self.width + px];
                let samples = [
                    bitmap.pixels[vertical.first * bitmap.width + horizontal.first],
                    bitmap.pixels[vertical.first * bitmap.width + horizontal.second],
                    bitmap.pixels[vertical.second * bitmap.width + horizontal.first],
                    bitmap.pixels[vertical.second * bitmap.width + horizontal.second],
                ];
                *dst = image_mix(samples, horizontal.weight, vertical.weight, *dst);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ImageAxis {
    first: usize,
    second: usize,
    weight: u32,
}

fn image_axis(
    offset: u64,
    destination: u64,
    source: usize,
    rendering: crate::css::ImageRendering,
) -> ImageAxis {
    use crate::css::ImageRendering;
    let source = source as u64;
    let denominator = destination * 2;
    if rendering == ImageRendering::CrispEdges {
        let index = (((offset * 2 + 1) * source) / denominator).min(source - 1) as usize;
        return ImageAxis {
            first: index,
            second: index,
            weight: 0,
        };
    }
    // Pixelated first chooses the nearest integer expansion on each axis,
    // then smooths any residual fractional scaling. Mapping its virtual grid
    // back to the original image costs four samples, not a huge temporary.
    let integer = if rendering == ImageRendering::Pixelated {
        ((destination + source / 2) / source).max(1)
    } else {
        1
    };
    let extent = source * integer;
    let numerator = (offset * 2 + 1) * extent;
    // Quotient/remainder avoids multiplying a huge CSS coordinate by 256
    // before division. Each term is bounded by i32 destination dimensions
    // and our existing four-megapixel source limit.
    let fixed = ((numerator / denominator) * 256 + (numerator % denominator) * 256 / denominator)
        as i64
        - 128;
    if fixed <= 0 {
        return ImageAxis {
            first: 0,
            second: 0,
            weight: 0,
        };
    }
    let first = ((fixed as u64) >> 8).min(extent - 1);
    let second = (first + 1).min(extent - 1);
    ImageAxis {
        first: (first / integer) as usize,
        second: (second / integer) as usize,
        weight: (fixed as u32) & 255,
    }
}

fn image_mix(samples: [u32; 4], x: u32, y: u32, background: u32) -> u32 {
    if x == 0 && y == 0 {
        let pixel = samples[0];
        let alpha = pixel >> 24;
        if alpha == 0 {
            return background;
        }
        if alpha == 255 {
            return pixel & 0xffffff;
        }
        let mut output = 0;
        for shift in [0, 8, 16] {
            output |= ((((pixel >> shift) & 255) * alpha
                + ((background >> shift) & 255) * (255 - alpha)
                + 127)
                / 255)
                << shift;
        }
        return output;
    }
    let weights = [(256 - x) * (256 - y), x * (256 - y), (256 - x) * y, x * y];
    if samples.iter().all(|pixel| pixel >> 24 == 255) {
        let mut output = 0;
        for shift in [0, 8, 16] {
            let mixed: u32 = samples
                .iter()
                .zip(weights)
                .map(|(pixel, weight)| ((pixel >> shift) & 255) * weight)
                .sum();
            output |= ((mixed + 32_768) >> 16) << shift;
        }
        return output;
    }
    let alpha: u64 = samples
        .iter()
        .zip(weights)
        .map(|(pixel, weight)| u64::from(pixel >> 24) * u64::from(weight))
        .sum();
    if alpha == 0 {
        return background;
    }
    let denominator = 255u64 * 65_536;
    let remaining = denominator - alpha;
    let mut output = 0;
    for shift in [0, 8, 16] {
        let premultiplied: u64 = samples
            .iter()
            .zip(weights)
            .map(|(pixel, weight)| {
                u64::from((pixel >> shift) & 255) * u64::from(pixel >> 24) * u64::from(weight)
            })
            .sum();
        let channel =
            (premultiplied + u64::from((background >> shift) & 255) * remaining + denominator / 2)
                / denominator;
        output |= (channel as u32) << shift;
    }
    output
}

#[derive(Clone, Copy, Debug)]
pub struct BackgroundPresentation {
    pub size: crate::css::BackgroundSize,
    /// Horizontal and vertical repetition of the single background layer.
    pub repeat: [bool; 2],
    /// Alignment fraction of leftover space, followed by zoomed pixel offset.
    pub position: [(f32, f32); 2],
    pub scale: f32,
    pub rendering: crate::css::ImageRendering,
}

fn background_axis(pixel: i64, start: i64, extent: u64, repeat: bool) -> Option<u64> {
    let offset = pixel.checked_sub(start)?;
    let extent = i64::try_from(extent).ok().filter(|extent| *extent > 0)?;
    if repeat {
        Some(offset.rem_euclid(extent) as u64)
    } else {
        (offset >= 0 && offset < extent).then_some(offset as u64)
    }
}

impl Canvas {
    /// Paint one background layer. `rect` is the border-box clip and `origin`
    /// the padding-box positioning area. Work is proportional only to visible
    /// surface pixels: no expanded bitmap, tile list or tile-count allocation.
    pub fn background_image(
        &mut self,
        rect: ClipRect,
        origin: ClipRect,
        bitmap: &crate::image::Bitmap,
        presentation: BackgroundPresentation,
    ) {
        if bitmap.width == 0 || bitmap.height == 0 || self.pixels.is_empty()
            || bitmap.width > 16_384 || bitmap.height > 16_384
            || !presentation.scale.is_finite() || presentation.scale <= 0.0
            || presentation.position.iter().any(|&(fraction, offset)| !fraction.is_finite() || !offset.is_finite())
        {
            return;
        }
        let Some(count) = bitmap.width.checked_mul(bitmap.height) else { return; };
        if count > bitmap.pixels.len() || count > crate::image::MAX_PIXELS { return; }
        let ow = i64::from(origin.right) - i64::from(origin.left);
        let oh = i64::from(origin.bottom) - i64::from(origin.top);
        if ow <= 0 || oh <= 0 { return; }
        let (sw, sh) = (bitmap.width as f64, bitmap.height as f64);
        let (dw, dh) = match presentation.size {
            crate::css::BackgroundSize::Auto => (sw * f64::from(presentation.scale), sh * f64::from(presentation.scale)),
            crate::css::BackgroundSize::Contain => {
                let factor = (ow as f64 / sw).min(oh as f64 / sh);
                (sw * factor, sh * factor)
            }
            crate::css::BackgroundSize::Cover => {
                let factor = (ow as f64 / sw).max(oh as f64 / sh);
                (sw * factor, sh * factor)
            }
        };
        if !dw.is_finite() || !dh.is_finite() || dw <= 0.0 || dh <= 0.0
            || dw.round() > f64::from(i32::MAX) || dh.round() > f64::from(i32::MAX)
        { return; }
        let (width, height) = (dw.round().max(1.0) as u64, dh.round().max(1.0) as u64);
        let x = (f64::from(origin.left) + (ow as f64 - width as f64) * f64::from(presentation.position[0].0)
            + f64::from(presentation.position[0].1)).round();
        let y = (f64::from(origin.top) + (oh as f64 - height as f64) * f64::from(presentation.position[1].0)
            + f64::from(presentation.position[1].1)).round();
        if !x.is_finite() || !y.is_finite()
            || x < f64::from(i32::MIN) || x > f64::from(i32::MAX)
            || y < f64::from(i32::MIN) || y > f64::from(i32::MAX)
        { return; }
        let (x, y) = (x as i64, y as i64);
        let visible = rect.intersect(self.clip).intersect(ClipRect::new(0, 0, self.width as i32, self.height as i32));
        if visible.left >= visible.right || visible.top >= visible.bottom { return; }
        for py in visible.top as usize..visible.bottom as usize {
            let Some(offset_y) = background_axis(py as i64, y, height, presentation.repeat[1]) else { continue; };
            let vertical = image_axis(offset_y, height, bitmap.height, presentation.rendering);
            for px in visible.left as usize..visible.right as usize {
                let Some(offset_x) = background_axis(px as i64, x, width, presentation.repeat[0]) else { continue; };
                let horizontal = image_axis(offset_x, width, bitmap.width, presentation.rendering);
                let samples = [
                    bitmap.pixels[vertical.first * bitmap.width + horizontal.first],
                    bitmap.pixels[vertical.first * bitmap.width + horizontal.second],
                    bitmap.pixels[vertical.second * bitmap.width + horizontal.first],
                    bitmap.pixels[vertical.second * bitmap.width + horizontal.second],
                ];
                let Some(index) = py.checked_mul(self.width).and_then(|row| row.checked_add(px)) else { return; };
                let Some(dst) = self.pixels.get_mut(index) else { return; };
                *dst = image_mix(samples, horizontal.weight, vertical.weight, *dst);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ImagePresentation {
    pub fit: crate::css::ObjectFit,
    pub rendering: crate::css::ImageRendering,
    /// Each axis is an alignment fraction of the leftover space plus pixels.
    pub position: [(f32, f32); 2],
    pub natural: (f32, f32),
}

impl Canvas {
    pub fn fitted_image(
        &mut self,
        rect: ClipRect,
        bitmap: &crate::image::Bitmap,
        presentation: ImagePresentation,
    ) {
        let width = rect.right.saturating_sub(rect.left);
        let height = rect.bottom.saturating_sub(rect.top);
        if width <= 0 || height <= 0 || bitmap.width == 0 || bitmap.height == 0 {
            return;
        }
        use crate::css::ObjectFit;
        let (bw, bh) = (width as f64, height as f64);
        let (sw, sh) = (bitmap.width as f64, bitmap.height as f64);
        let contain = (bw / sw).min(bh / sh);
        let (dw, dh) = match presentation.fit {
            ObjectFit::Contain => (sw * contain, sh * contain),
            ObjectFit::Cover => {
                let scale = (bw / sw).max(bh / sh);
                (sw * scale, sh * scale)
            }
            ObjectFit::None => (
                f64::from(presentation.natural.0),
                f64::from(presentation.natural.1),
            ),
            ObjectFit::ScaleDown => {
                let (nw, nh) = (
                    f64::from(presentation.natural.0),
                    f64::from(presentation.natural.1),
                );
                if nw <= sw * contain && nh <= sh * contain {
                    (nw, nh)
                } else {
                    (sw * contain, sh * contain)
                }
            }
            ObjectFit::Fill => (bw, bh),
        };
        if !dw.is_finite() || !dh.is_finite() || dw <= 0.0 || dh <= 0.0 {
            return;
        }
        let dx = (bw - dw) * f64::from(presentation.position[0].0)
            + f64::from(presentation.position[0].1);
        let dy = (bh - dh) * f64::from(presentation.position[1].0)
            + f64::from(presentation.position[1].1);
        let x = (f64::from(rect.left) + dx)
            .round()
            .clamp(-1_000_000_000.0, 1_000_000_000.0) as i32;
        let y = (f64::from(rect.top) + dy)
            .round()
            .clamp(-1_000_000_000.0, 1_000_000_000.0) as i32;
        let clip = self.clip();
        self.set_clip(Some(clip.intersect(rect)));
        let width = dw.round().clamp(1.0, 1_000_000_000.0) as i32;
        let height = dh.round().clamp(1.0, 1_000_000_000.0) as i32;
        if presentation.rendering == crate::css::ImageRendering::Auto {
            self.image(x, y, width, height, bitmap);
        } else {
            self.image_filtered(x, y, width, height, bitmap, presentation.rendering);
        }
        self.set_clip(Some(clip));
    }
}

// Each row is five pixels, with the leftmost pixel in bit four. Every letterform
// here was drawn for this browser; no font file, font table, or OS font renderer
// is used. Latin accents are composed below without changing the 5x7 cell.
const MISSING_GLYPH: [u8; 7] = [
    0b11111, 0b10001, 0b10101, 0b10001, 0b10101, 0b10001, 0b11111,
];

fn glyph(character: char) -> [u8; 7] {
    match character {
        // Spaces retain one cell each in this fixed-width font. Soft hyphen is
        // invisible; discretionary hyphenation belongs to future line layout.
        ' ' | '\u{00a0}' | '\u{00ad}' | '\u{2002}' | '\u{2003}' | '\u{2009}' => [0; 7],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '"' => [0b01010, 0b01010, 0b01010, 0, 0, 0, 0],
        '#' => [0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0],
        '$' => [
            0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100,
        ],
        '%' => [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
        '&' => [
            0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
        ],
        '\'' => [0b00100, 0b00100, 0b00010, 0, 0, 0, 0],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '*' => [0, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        ',' => [0, 0, 0, 0, 0b00110, 0b00100, 0b01000],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0b00110, 0b00110],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b00111, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100,
        ],
        ':' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
        ';' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0b01000],
        '<' => [
            0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001,
        ],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        '>' => [
            0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000,
        ],
        '?' => [0b01110, 0b10001, 0b00010, 0b00100, 0b00100, 0, 0b00100],
        '@' => [
            0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b11001, 0b10101, 0b10011, 0b10011, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        '\\' => [
            0b10000, 0b01000, 0b01000, 0b00100, 0b00010, 0b00010, 0b00001,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        '^' => [0b00100, 0b01010, 0b10001, 0, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '`' => [0b01000, 0b00100, 0, 0, 0, 0, 0],
        'a' => [0, 0, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
        'b' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110,
        ],
        'c' => [0, 0, 0b01111, 0b10000, 0b10000, 0b10000, 0b01111],
        'd' => [
            0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111,
        ],
        'e' => [0, 0, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
        'f' => [
            0b00110, 0b01001, 0b01000, 0b11110, 0b01000, 0b01000, 0b01000,
        ],
        'g' => [0, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'h' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001,
        ],
        'i' => [0b00100, 0, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        'j' => [0b00010, 0, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100],
        'k' => [
            0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010,
        ],
        'l' => [
            0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'm' => [0, 0, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101],
        'n' => [0, 0, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        'o' => [0, 0, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'p' => [0, 0, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        'q' => [0, 0, 0b01111, 0b10001, 0b01111, 0b00001, 0b00001],
        'r' => [0, 0, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
        's' => [0, 0, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
        't' => [
            0b01000, 0b01000, 0b11110, 0b01000, 0b01000, 0b01001, 0b00110,
        ],
        'u' => [0, 0, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101],
        'v' => [0, 0, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'w' => [0, 0, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
        'x' => [0, 0, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
        'y' => [0, 0, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'z' => [0, 0, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
        '{' => [
            0b00011, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00011,
        ],
        '|' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        '}' => [
            0b11000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b11000,
        ],
        '~' => [0, 0, 0b01001, 0b10110, 0, 0, 0],
        '¡' => [0b00100, 0, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        '¢' => [
            0b00100, 0b01111, 0b10100, 0b10100, 0b10100, 0b01111, 0b00100,
        ],
        '£' => [
            0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b11111,
        ],
        '¤' => [0, 0b10001, 0b01110, 0b01010, 0b01110, 0b10001, 0],
        '¥' => [
            0b10001, 0b01010, 0b00100, 0b11111, 0b00100, 0b11111, 0b00100,
        ],
        '¦' => [0b00100, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0b00100],
        '§' => [
            0b01110, 0b10000, 0b01110, 0b10001, 0b01110, 0b00001, 0b01110,
        ],
        '¨' => [0b01010, 0, 0, 0, 0, 0, 0],
        '©' => [
            0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10001, 0b01110,
        ],
        'ª' => [0b01110, 0b00010, 0b01110, 0b01010, 0b01110, 0, 0b01110],
        '«' => [0, 0b00101, 0b01010, 0b10100, 0b01010, 0b00101, 0],
        '¬' => [0, 0, 0b11111, 0b00001, 0b00001, 0, 0],
        '®' => [
            0b01110, 0b10001, 0b11101, 0b11011, 0b11011, 0b10001, 0b01110,
        ],
        '¯' => [0b11111, 0, 0, 0, 0, 0, 0],
        '°' => [0b00110, 0b01001, 0b01001, 0b00110, 0, 0, 0],
        '±' => [0, 0b00100, 0b11111, 0b00100, 0, 0b11111, 0],
        '²' => [0b01100, 0b00010, 0b00100, 0b01110, 0, 0, 0],
        '³' => [0b01100, 0b00010, 0b00100, 0b00010, 0b01100, 0, 0],
        '´' => [0b00010, 0b00100, 0, 0, 0, 0, 0],
        'µ' => [0, 0, 0b10001, 0b10001, 0b10011, 0b11101, 0b10000],
        '¶' => [
            0b01111, 0b10101, 0b10101, 0b01101, 0b00101, 0b00101, 0b00101,
        ],
        '·' => [0, 0, 0, 0b00100, 0, 0, 0],
        '¸' => [0, 0, 0, 0, 0, 0b00100, 0b01000],
        '¹' => [0b00100, 0b01100, 0b00100, 0b01110, 0, 0, 0],
        'º' => [0b01110, 0b10001, 0b10001, 0b01110, 0, 0b11111, 0],
        '»' => [0, 0b10100, 0b01010, 0b00101, 0b01010, 0b10100, 0],
        '¼' => [
            0b01000, 0b11001, 0b01010, 0b00100, 0b01010, 0b10111, 0b00010,
        ],
        '½' => [
            0b01000, 0b11001, 0b01010, 0b00100, 0b01011, 0b10010, 0b00111,
        ],
        '¾' => [
            0b11000, 0b01001, 0b11010, 0b00100, 0b01010, 0b10111, 0b00010,
        ],
        '¿' => [0b00100, 0, 0b00100, 0b01000, 0b10000, 0b10001, 0b01110],
        'Æ' => [
            0b01111, 0b10100, 0b10100, 0b11110, 0b10100, 0b10100, 0b10111,
        ],
        'æ' => [0, 0, 0b11010, 0b00101, 0b11111, 0b10100, 0b11111],
        'Ç' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b01111, 0b00100, 0b01000,
        ],
        'ç' => [0, 0b01111, 0b10000, 0b10000, 0b01111, 0b00100, 0b01000],
        'Ð' => [
            0b01100, 0b01010, 0b01001, 0b11101, 0b01001, 0b01010, 0b01100,
        ],
        'ð' => [
            0b01010, 0b00100, 0b01010, 0b00001, 0b01111, 0b10001, 0b01110,
        ],
        '×' => [0, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0],
        'Ø' => [
            0b01111, 0b10011, 0b10101, 0b10101, 0b10101, 0b11001, 0b11110,
        ],
        'ø' => [0, 0, 0b01111, 0b10011, 0b10101, 0b11001, 0b11110],
        'Þ' => [
            0b10000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000,
        ],
        'þ' => [
            0b10000, 0b10000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000,
        ],
        'ß' => [
            0b01100, 0b10010, 0b10010, 0b10100, 0b10010, 0b10010, 0b10100,
        ],
        '÷' => [0, 0b00100, 0, 0b11111, 0, 0b00100, 0],
        '€' => [
            0b00111, 0b01000, 0b11110, 0b01000, 0b11110, 0b01000, 0b00111,
        ],
        '‚' => [0, 0, 0, 0, 0b00100, 0b00100, 0b01000],
        'ƒ' => [
            0b00110, 0b01001, 0b01000, 0b11110, 0b01000, 0b01000, 0b10000,
        ],
        '„' => [0, 0, 0, 0, 0b01010, 0b01010, 0b10100],
        '…' => [0, 0, 0, 0, 0, 0b10101, 0b10101],
        '†' => [
            0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        '‡' => [
            0b00100, 0b11111, 0b00100, 0b00100, 0b00100, 0b11111, 0b00100,
        ],
        'ˆ' => [0b00100, 0b01010, 0, 0, 0, 0, 0],
        '‰' => [
            0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b10101, 0b00110,
        ],
        '‹' => [0, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0],
        '›' => [0, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0],
        'Œ' => [
            0b01111, 0b10100, 0b10100, 0b10110, 0b10100, 0b10100, 0b01111,
        ],
        'œ' => [0, 0, 0b01010, 0b10101, 0b10111, 0b10100, 0b01011],
        '‘' => [0b00010, 0b00100, 0b00100, 0, 0, 0, 0],
        '’' => [0b00100, 0b00100, 0b01000, 0, 0, 0, 0],
        '“' => [0b00101, 0b01010, 0b01010, 0, 0, 0, 0],
        '”' => [0b01010, 0b01010, 0b10100, 0, 0, 0, 0],
        '•' => [0, 0, 0b00100, 0b01110, 0b00100, 0, 0],
        '–' => [0, 0, 0, 0b01111, 0, 0, 0],
        '—' | '−' => [0, 0, 0, 0b11111, 0, 0, 0],
        '˜' => [0b01001, 0b10110, 0, 0, 0, 0, 0],
        '™' => [0b11101, 0b01011, 0b01001, 0b01001, 0, 0, 0],
        '≠' => [
            0b00001, 0b00010, 0b11111, 0b00100, 0b11111, 0b01000, 0b10000,
        ],
        '≤' => [0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0, 0b11111],
        '≥' => [0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0, 0b11111],
        '∉' => [
            0b00001, 0b01110, 0b10010, 0b11111, 0b10100, 0b01110, 0b10000,
        ],
        '←' => [0, 0b00100, 0b01000, 0b11111, 0b01000, 0b00100, 0],
        '→' => [0, 0b00100, 0b00010, 0b11111, 0b00010, 0b00100, 0],
        '↑' => [0b00100, 0b01110, 0b10101, 0b00100, 0b00100, 0b00100, 0],
        '↓' => [0, 0b00100, 0b00100, 0b00100, 0b10101, 0b01110, 0b00100],
        '↔' => [0, 0, 0b01010, 0b11111, 0b01010, 0, 0],
        _ => accented_glyph(character).unwrap_or(MISSING_GLYPH),
    }
}

enum Accent {
    Grave,
    Acute,
    Circumflex,
    Tilde,
    Diaeresis,
    Ring,
    Caron,
}

fn accented_glyph(character: char) -> Option<[u8; 7]> {
    use Accent::*;
    let (base, accent) = match character {
        'À' => ('A', Grave),
        'Á' => ('A', Acute),
        'Â' => ('A', Circumflex),
        'Ã' => ('A', Tilde),
        'Ä' => ('A', Diaeresis),
        'Å' => ('A', Ring),
        'È' => ('E', Grave),
        'É' => ('E', Acute),
        'Ê' => ('E', Circumflex),
        'Ë' => ('E', Diaeresis),
        'Ì' => ('I', Grave),
        'Í' => ('I', Acute),
        'Î' => ('I', Circumflex),
        'Ï' => ('I', Diaeresis),
        'Ñ' => ('N', Tilde),
        'Ò' => ('O', Grave),
        'Ó' => ('O', Acute),
        'Ô' => ('O', Circumflex),
        'Õ' => ('O', Tilde),
        'Ö' => ('O', Diaeresis),
        'Ù' => ('U', Grave),
        'Ú' => ('U', Acute),
        'Û' => ('U', Circumflex),
        'Ü' => ('U', Diaeresis),
        'Ý' => ('Y', Acute),
        'Ÿ' => ('Y', Diaeresis),
        'Š' => ('S', Caron),
        'Ž' => ('Z', Caron),
        'à' => ('a', Grave),
        'á' => ('a', Acute),
        'â' => ('a', Circumflex),
        'ã' => ('a', Tilde),
        'ä' => ('a', Diaeresis),
        'å' => ('a', Ring),
        'è' => ('e', Grave),
        'é' => ('e', Acute),
        'ê' => ('e', Circumflex),
        'ë' => ('e', Diaeresis),
        'ì' => ('i', Grave),
        'í' => ('i', Acute),
        'î' => ('i', Circumflex),
        'ï' => ('i', Diaeresis),
        'ñ' => ('n', Tilde),
        'ò' => ('o', Grave),
        'ó' => ('o', Acute),
        'ô' => ('o', Circumflex),
        'õ' => ('o', Tilde),
        'ö' => ('o', Diaeresis),
        'ù' => ('u', Grave),
        'ú' => ('u', Acute),
        'û' => ('u', Circumflex),
        'ü' => ('u', Diaeresis),
        'ý' => ('y', Acute),
        'ÿ' => ('y', Diaeresis),
        'š' => ('s', Caron),
        'ž' => ('z', Caron),
        _ => return None,
    };
    // Lowercase x-height already leaves two top rows. Capitals use intentionally
    // compact five-row outlines so their accent also stays inside this cell.
    let compact = match base {
        'A' => Some([0b01110, 0b10001, 0b11111, 0b10001, 0b10001]),
        'E' => Some([0b11111, 0b10000, 0b11110, 0b10000, 0b11111]),
        'I' => Some([0b01110, 0b00100, 0b00100, 0b00100, 0b01110]),
        'N' => Some([0b10001, 0b11001, 0b10101, 0b10011, 0b10001]),
        'O' => Some([0b01110, 0b10001, 0b10001, 0b10001, 0b01110]),
        'U' => Some([0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'Y' => Some([0b10001, 0b01010, 0b00100, 0b00100, 0b00100]),
        'S' => Some([0b01111, 0b10000, 0b01110, 0b00001, 0b11110]),
        'Z' => Some([0b11111, 0b00010, 0b00100, 0b01000, 0b11111]),
        _ => None,
    };
    let mut rows = glyph(base);
    if let Some(compact) = compact {
        rows[2..].copy_from_slice(&compact);
    }
    let mark = match accent {
        Grave => [0b01000, 0b00100],
        Acute => [0b00010, 0b00100],
        Circumflex => [0b00100, 0b01010],
        Tilde => [0b01001, 0b10110],
        Diaeresis => [0b01010, 0],
        Ring => [0b01110, 0b01010],
        Caron => [0b01010, 0b00100],
    };
    rows[..2].copy_from_slice(&mark);
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::{glyph, Canvas, GLYPH_ADVANCE, GLYPH_HEIGHT, GLYPH_WIDTH, MISSING_GLYPH};

    #[test]
    fn object_fit_positions_and_clips_original_pixels_without_touching_neighbors() {
        use super::{ClipRect, ImagePresentation};
        use crate::css::ObjectFit;
        let bitmap = crate::image::Bitmap {
            width: 4,
            height: 2,
            pixels: vec![
                0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffffff, 0xff000000, 0xffffff00, 0xffff00ff,
                0xff00ffff,
            ],
        };
        let render = |fit, position, natural| {
            let mut canvas = Canvas::new(8, 8);
            canvas.fill_rect(0, 0, 8, 8, 0x123456);
            let clip = ClipRect::new(1, 1, 6, 6);
            canvas.set_clip(Some(clip));
            canvas.fitted_image(
                ClipRect::new(2, 2, 4, 4),
                &bitmap,
                ImagePresentation {
                    fit,
                    rendering: crate::css::ImageRendering::CrispEdges,
                    position,
                    natural,
                },
            );
            assert_eq!(canvas.clip(), clip);
            for y in 0..8 {
                for x in 0..8 {
                    if !(2..6).contains(&x) || !(2..6).contains(&y) {
                        assert_eq!(canvas.pixels[y * 8 + x], 0x123456);
                    }
                }
            }
            canvas
        };
        let center = [(0.5, 0.0); 2];
        let contain = render(ObjectFit::Contain, center, (4.0, 2.0));
        assert_eq!(&contain.pixels[2 * 8 + 2..2 * 8 + 6], &[0x123456; 4]);
        assert_eq!(
            &contain.pixels[3 * 8 + 2..3 * 8 + 6],
            &[0xff0000, 0x00ff00, 0x0000ff, 0xffffff]
        );
        assert_eq!(
            &contain.pixels[4 * 8 + 2..4 * 8 + 6],
            &[0, 0xffff00, 0xff00ff, 0x00ffff]
        );
        let bottom = render(ObjectFit::Contain, [(0.5, 0.0), (1.0, 0.0)], (4.0, 2.0));
        assert_eq!(
            &bottom.pixels[4 * 8 + 2..4 * 8 + 6],
            &[0xff0000, 0x00ff00, 0x0000ff, 0xffffff]
        );
        let cover = render(ObjectFit::Cover, center, (4.0, 2.0));
        assert_eq!(
            &cover.pixels[2 * 8 + 2..2 * 8 + 6],
            &[0x00ff00, 0x00ff00, 0x0000ff, 0x0000ff]
        );
        let left = render(ObjectFit::Cover, [(0.0, 0.0), (0.5, 0.0)], (4.0, 2.0));
        assert_eq!(
            &left.pixels[2 * 8 + 2..2 * 8 + 6],
            &[0xff0000, 0xff0000, 0x00ff00, 0x00ff00]
        );
        assert_eq!(
            render(ObjectFit::None, center, (4.0, 2.0)).pixels,
            contain.pixels
        );
        let small = render(ObjectFit::ScaleDown, center, (2.0, 1.0));
        assert_eq!(
            &small.pixels[4 * 8 + 2..4 * 8 + 6],
            &[0x123456, 0xffff00, 0x00ffff, 0x123456]
        );
        assert_eq!(
            render(ObjectFit::ScaleDown, center, (8.0, 4.0)).pixels,
            contain.pixels
        );
        let offset = render(ObjectFit::None, [(1.0, -1.0), (0.0, 0.0)], (4.0, 2.0));
        assert_eq!(
            &offset.pixels[2 * 8 + 2..2 * 8 + 6],
            &[0x00ff00, 0x0000ff, 0xffffff, 0x123456]
        );
        assert!(render(ObjectFit::None, center, (0.0, 0.0))
            .pixels
            .iter()
            .all(|p| *p == 0x123456));
        let fill = render(ObjectFit::Fill, [(0.0, 1.0), (0.0, 1.0)], (4.0, 2.0));
        assert_eq!(&fill.pixels[2 * 8 + 2..2 * 8 + 6], &[0x123456; 4]);
        assert_eq!(
            &fill.pixels[3 * 8 + 2..3 * 8 + 6],
            &[0x123456, 0xff0000, 0x00ff00, 0x0000ff]
        );
    }

    #[test]
    fn clipping_applies_to_rectangles_text_and_scaled_images_and_resets() {
        let mut canvas = Canvas::new(30, 24);
        let clip = super::ClipRect::new(7, 5, 9, 10);
        canvas.set_clip(Some(clip));
        canvas.fill_rect(-2, -3, 40, 40, 0x010203);
        canvas.text(0, 0, "XXXXX", 3, 0xff0000);
        let bitmap = crate::image::Bitmap {
            width: 2,
            height: 1,
            pixels: vec![0xff00ff00, 0xff0000ff],
        };
        canvas.image_filtered(
            0,
            0,
            20,
            20,
            &bitmap,
            crate::css::ImageRendering::CrispEdges,
        );
        for y in 0..24 {
            for x in 0..30 {
                assert_eq!(
                    canvas.pixels[y * 30 + x],
                    if clip.contains(x as i32, y as i32) {
                        if x < 10 {
                            0x00ff00
                        } else {
                            0x0000ff
                        }
                    } else {
                        0
                    }
                );
            }
        }
        canvas.set_clip(Some(super::ClipRect::new(
            i32::MAX,
            i32::MIN,
            i32::MAX,
            i32::MAX,
        )));
        let before = canvas.pixels.clone();
        canvas.image(0, 0, 20, 20, &bitmap);
        canvas.fill_rect(0, 0, 30, 24, 0xffffff);
        assert_eq!(canvas.pixels, before);
        canvas.set_clip(None);
        canvas.fill_rect(0, 0, 1, 1, 0xffffff);
        assert_eq!(canvas.pixels[0], 0xffffff);
    }

    #[test]
    fn latin1_and_windows1252_printable_characters_have_original_glyphs() {
        for codepoint in 0x00a0..=0x00ff {
            let character = char::from_u32(codepoint).unwrap();
            assert_ne!(glyph(character), MISSING_GLYPH, "U+{codepoint:04X}");
            assert!(glyph(character).iter().all(|row| *row < (1 << GLYPH_WIDTH)));
        }
        for character in "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ".chars()
        {
            assert_ne!(glyph(character), MISSING_GLYPH, "{character}");
            assert!(glyph(character).iter().any(|row| *row != 0));
        }
        assert_eq!(glyph('\u{00a0}'), [0; 7]);
        assert_eq!(glyph('雪'), MISSING_GLYPH);
    }

    #[test]
    fn accents_preserve_lowercase_shapes_and_the_existing_cell_metrics() {
        assert_eq!((GLYPH_WIDTH, GLYPH_ADVANCE, GLYPH_HEIGHT), (5, 6, 7));
        for (accented, base) in [
            ('á', 'a'),
            ('é', 'e'),
            ('í', 'i'),
            ('ó', 'o'),
            ('ú', 'u'),
            ('ñ', 'n'),
            ('ü', 'u'),
        ] {
            assert_eq!(&glyph(accented)[2..], &glyph(base)[2..]);
            assert_ne!(glyph(accented), glyph(base));
        }
        assert_ne!(glyph('á'), glyph('à'));
        assert_ne!(glyph('å'), glyph('â'));
        let mut canvas = Canvas::new(18, 7);
        canvas.text(0, 0, "á\u{00a0}ú", 1, 0xffffff);
        assert!(canvas
            .pixels
            .chunks(18)
            .all(|row| row[5..12].iter().all(|pixel| *pixel == 0)));
        assert!(canvas
            .pixels
            .chunks(18)
            .any(|row| row[12..17].iter().any(|pixel| *pixel != 0)));
    }

    #[test]
    fn rectangles_clip_without_wrapping_coordinates() {
        let mut canvas = Canvas::new(4, 3);
        canvas.fill_rect(-2, -1, 4, 3, 0xff11_2233);
        assert_eq!(
            canvas.pixels,
            [0x112233, 0x112233, 0, 0, 0x112233, 0x112233, 0, 0, 0, 0, 0, 0]
        );
        canvas.fill_rect(i32::MAX, 0, i32::MAX, 10, 0xffffff);
        canvas.fill_rect(0, i32::MIN, 10, i32::MAX, 0xffffff);
        canvas.fill_rect(0, 0, -1, 10, 0xffffff);
        assert_eq!(canvas.pixels[0], 0x112233);
        assert_eq!(canvas.pixels[11], 0);
    }

    #[test]
    fn missing_characters_are_visible_and_newlines_move_down() {
        let mut canvas = Canvas::new(12, 16);
        canvas.text(0, 0, "\u{2603}\nA", 1, 0xabcdef);
        assert!(canvas.pixels[..7 * 12].iter().any(|pixel| *pixel != 0));
        assert!(canvas.pixels[7 * 12..9 * 12]
            .iter()
            .all(|pixel| *pixel == 0));
        assert!(canvas.pixels[9 * 12..].iter().any(|pixel| *pixel != 0));
    }

    #[test]
    fn empty_surfaces_and_extreme_text_coordinates_are_safe() {
        let mut empty = Canvas::new(0, 20);
        empty.fill_rect(0, 0, 10, 10, 0xffffff);
        empty.text(0, 0, "Example", 1, 0xffffff);
        assert!(empty.pixels.is_empty());
        assert!(Canvas::new(usize::MAX, 2).pixels.is_empty());

        let mut canvas = Canvas::new(2, 2);
        canvas.text(i32::MAX, i32::MIN, "Example\nText", i32::MAX, 0xffffff);
        canvas.text(0, 0, "Example", 0, 0xffffff);
        assert_eq!(canvas.pixels, [0; 4]);
    }
}
