//! Original separable area reduction in the existing eight-bit display space.
//! Shrinking axes integrate every covered source pixel. Enlarged axes retain
//! the painter's bilinear pixel-center mapping. Premultiplied contributions
//! stay unrounded between passes and are composited only at the destination.
//! This is not linear-light filtering or color-profile conversion.
//!
//! The shrinking axis is processed first: a 16384x256 image becoming 1x4000
//! must not rescan its full source row for each output row, and the transposed
//! case must not expand every row before reducing it. Work follows covered
//! source pixels plus visible output, never the full CSS-sized rectangle.
//! CSS permits this smooth filter: https://www.w3.org/TR/css-images-3/#the-image-rendering
//! Alpha: https://www.w3.org/TR/compositing-1/#simplealphacompositing

use super::{image_axis, Canvas};
use crate::css::ImageRendering;
use crate::image::Bitmap;

#[derive(Clone, Copy, Default)]
struct Sum {
    alpha: u64,
    rgb: [u64; 3],
}

impl Sum {
    fn pixel(&mut self, pixel: u32, weight: u64) {
        let alpha = u64::from(pixel >> 24) * weight;
        self.alpha += alpha;
        for (channel, shift) in self.rgb.iter_mut().zip([0, 8, 16]) {
            *channel += u64::from((pixel >> shift) & 255) * alpha;
        }
    }

    fn add(&mut self, other: Self, weight: u64) {
        self.alpha += other.alpha * weight;
        for (channel, value) in self.rgb.iter_mut().zip(other.rgb) {
            *channel += value * weight;
        }
    }

    fn over(self, weight: u64, background: u32) -> u32 {
        if self.alpha == 0 {
            return background;
        }
        // The largest normalization is 16384 * 256 for mixed scaling,
        // or the four-megapixel source area when both axes shrink. Channel
        // sums are at most 65025 * normalization, safely inside u64.
        let denominator = 255 * weight;
        let remaining = denominator - self.alpha;
        let mut output = 0;
        for (color, shift) in self.rgb.into_iter().zip([0, 8, 16]) {
            let value =
                (color + u64::from((background >> shift) & 255) * remaining + denominator / 2)
                    / denominator;
            output |= (value as u32) << shift;
        }
        output
    }
}

#[derive(Clone, Copy)]
struct Span {
    first: usize,
    end: usize,
    first_weight: u32,
    last_weight: u32,
}

impl Span {
    fn new(offset: usize, source: usize, destination: usize) -> Self {
        // Only shrinking axes use this, so all factors are at most 16384.
        // Coordinates are exact integer multiples of 1 / destination.
        let left = offset * source;
        let right = left + source;
        let first = left / destination;
        let end = right.div_ceil(destination);
        Self {
            first,
            end,
            first_weight: (destination - left % destination) as u32,
            last_weight: (right - (end - 1) * destination) as u32,
        }
    }

    fn weight(self, index: usize, destination: usize) -> u64 {
        if index == self.first {
            u64::from(self.first_weight)
        } else if index + 1 == self.end {
            u64::from(self.last_weight)
        } else {
            destination as u64
        }
    }
}

fn scratch(count: usize) -> Option<Vec<Sum>> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).ok()?;
    values.resize(count, Sum::default());
    Some(values)
}

struct Rows {
    data: [Vec<Sum>; 2],
    source: [usize; 2],
    next: usize,
}

impl Rows {
    fn get(&mut self, y: usize, bitmap: &Bitmap, columns: &[Span], width: usize) -> &[Sum] {
        let index = if let Some(index) = self.source.iter().position(|&source| source == y) {
            index
        } else {
            let index = self.next;
            self.next ^= 1;
            self.source[index] = y;
            for (sum, span) in self.data[index].iter_mut().zip(columns) {
                *sum = Sum::default();
                for x in span.first..span.end {
                    sum.pixel(bitmap.pixels[y * bitmap.width + x], span.weight(x, width));
                }
            }
            index
        };
        &self.data[index]
    }
}

/// Returns false before painting if bounded scratch allocation fails; the
/// caller can retain its constant-memory bilinear fallback. No whole resized
/// bitmap is allocated. Maximum scratch is 120 bytes per source column
/// (under 1.9 MiB at the 16384-axis limit), or 32 bytes with vertical first.
pub(super) fn image(
    canvas: &mut Canvas,
    origin: (i32, i32),
    size: (usize, usize),
    bitmap: &Bitmap,
    visible: (usize, usize, usize, usize),
) -> bool {
    let (left, top, right, bottom) = visible;
    let (width, height) = size;
    let offset_x = |x: usize| (x as i64 - i64::from(origin.0)) as usize;
    let offset_y = |y: usize| (y as i64 - i64::from(origin.1)) as usize;
    if width < bitmap.width {
        let count = right - left;
        let mut columns = Vec::new();
        if columns.try_reserve_exact(count).is_err() {
            return false;
        }
        for x in left..right {
            columns.push(Span::new(offset_x(x), bitmap.width, width));
        }
        let (Some(first), Some(second), Some(mut output)) =
            (scratch(count), scratch(count), scratch(count))
        else {
            return false;
        };
        let mut rows = Rows {
            data: [first, second],
            source: [usize::MAX; 2],
            next: 0,
        };
        let y_weight = if height < bitmap.height {
            bitmap.height as u64
        } else {
            256
        };
        for y in top..bottom {
            output.fill(Sum::default());
            let mut add_row = |source_y, weight| {
                if weight != 0 {
                    for (sum, &row) in output
                        .iter_mut()
                        .zip(rows.get(source_y, bitmap, &columns, width))
                    {
                        sum.add(row, weight);
                    }
                }
            };
            if height < bitmap.height {
                let span = Span::new(offset_y(y), bitmap.height, height);
                for source_y in span.first..span.end {
                    add_row(source_y, span.weight(source_y, height));
                }
            } else {
                let axis = image_axis(
                    offset_y(y) as u64,
                    height as u64,
                    bitmap.height,
                    ImageRendering::Auto,
                );
                add_row(axis.first, u64::from(256 - axis.weight));
                add_row(axis.second, u64::from(axis.weight));
            }
            for (x, sum) in (left..right).zip(&output) {
                let destination = &mut canvas.pixels[y * canvas.width + x];
                *destination = sum.over(bitmap.width as u64 * y_weight, *destination);
            }
        }
    } else {
        // Only height shrinks. Reduce vertical columns before expanding them
        // horizontally; sample only the source columns touched by the clip.
        let axis = |x: usize| {
            image_axis(
                offset_x(x) as u64,
                width as u64,
                bitmap.width,
                ImageRendering::Auto,
            )
        };
        let first = axis(left).first;
        let end = axis(right - 1).second + 1;
        let Some(mut row) = scratch(end - first) else {
            return false;
        };
        for y in top..bottom {
            row.fill(Sum::default());
            let span = Span::new(offset_y(y), bitmap.height, height);
            for source_y in span.first..span.end {
                let weight = span.weight(source_y, height);
                for (source_x, sum) in (first..end).zip(&mut row) {
                    sum.pixel(bitmap.pixels[source_y * bitmap.width + source_x], weight);
                }
            }
            for x in left..right {
                let horizontal = axis(x);
                let mut sum = Sum::default();
                sum.add(
                    row[horizontal.first - first],
                    u64::from(256 - horizontal.weight),
                );
                sum.add(row[horizontal.second - first], u64::from(horizontal.weight));
                let destination = &mut canvas.pixels[y * canvas.width + x];
                *destination = sum.over(bitmap.height as u64 * 256, *destination);
            }
        }
    }
    true
}
