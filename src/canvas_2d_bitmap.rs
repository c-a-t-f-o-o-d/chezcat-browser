//! Lazy, bounded Canvas 2D pixels in the image renderer's straight-alpha format.
//! The host owns context identity, styles, dimension reflection and document
//! accounting. Borrowing pixels for painting never clones or allocates them.

use crate::image::{source_over, Bitmap};

pub(crate) const MAX_DIMENSION: u32 = 16_384;
pub(crate) const MAX_PIXELS: usize = crate::image::MAX_PIXELS;
const MAX_OPERATION_WORK: usize = 1 + 2 * MAX_PIXELS;

pub(crate) struct CanvasBitmap {
    width: u32,
    height: u32,
    bitmap: Option<Bitmap>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadbackError {
    ZeroSize,
    TooLarge,
    DestinationLength,
}

/// An output rectangle independent of raster allocation or CSS presentation.
/// The caller supplies the real VM buffer limit, before reserving its output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReadbackRect {
    x: i64,
    y: i64,
    pub width: u32,
    pub height: u32,
    bytes: usize,
}

impl ReadbackRect {
    pub(crate) fn new(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        max_bytes: usize,
    ) -> Result<Self, ReadbackError> {
        if width == 0 || height == 0 {
            return Err(ReadbackError::ZeroSize);
        }
        // Widen before negation/addition, including i32::MIN. Negative extents
        // move the origin; they do not reverse the row or pixel order.
        let (mut x, mut y) = (i64::from(x), i64::from(y));
        let (mut width, mut height) = (i64::from(width), i64::from(height));
        if width < 0 {
            x += width;
            width = -width;
        }
        if height < 0 {
            y += height;
            height = -height;
        }
        let bytes = (width as u64)
            .checked_mul(height as u64)
            .and_then(|pixels| pixels.checked_mul(4))
            .and_then(|bytes| usize::try_from(bytes).ok())
            .filter(|&bytes| bytes <= max_bytes)
            .ok_or(ReadbackError::TooLarge)?;
        Ok(Self {
            x,
            y,
            width: width as u32,
            height: height as u32,
            bytes,
        })
    }

    pub(crate) fn byte_len(self) -> usize {
        self.bytes
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Bounds {
    fn clipped(rect: [f64; 4], width: u32, height: u32) -> Option<Self> {
        let [x, y, width_extent, height_extent] = rect;
        if !rect.iter().all(|n| n.is_finite())
            || width_extent == 0.0
            || height_extent == 0.0
            || width == 0
            || height == 0
        {
            return None;
        }
        // Endpoint addition may overflow for finite, extremely large inputs.
        // Clipping the normalized endpoints still gives the correct bounded
        // intersection; integer conversion happens only after that clipping.
        let x_end = x + width_extent;
        let y_end = y + height_extent;
        let left = x.min(x_end).max(0.0);
        let top = y.min(y_end).max(0.0);
        let right = x.max(x_end).min(f64::from(width));
        let bottom = y.max(y_end).min(f64::from(height));
        if left >= right || top >= bottom {
            return None;
        }
        Some(Self {
            left,
            top,
            right,
            bottom,
            x0: left.floor() as usize,
            y0: top.floor() as usize,
            x1: right.ceil() as usize,
            y1: bottom.ceil() as usize,
        })
    }

    fn visits(self) -> Option<usize> {
        (self.x1 - self.x0).checked_mul(self.y1 - self.y0)
    }

    fn coverage(self, x: usize, y: usize) -> f64 {
        let x = x as f64;
        let y = y as f64;
        let horizontal = (self.right.min(x + 1.0) - self.left.max(x)).clamp(0.0, 1.0);
        let vertical = (self.bottom.min(y + 1.0) - self.top.max(y)).clamp(0.0, 1.0);
        horizontal * vertical
    }
}

impl CanvasBitmap {
    /// Dimension reflection is independent of raster capacity. An oversized
    /// canvas can keep its context while its backing remains transparent.
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bitmap: None,
        }
    }

    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// None represents a fully transparent surface without pixel allocation.
    pub(crate) fn bitmap(&self) -> Option<&Bitmap> {
        self.bitmap.as_ref()
    }

    /// Copy natural bitmap pixels as straight RGBA, with transparent black
    /// outside the surface or before backing allocation. The host must validate
    /// native access/origin cleanliness and debit output storage and work first.
    /// No CSS background, scaling, paint clip or external image is sampled.
    pub(crate) fn copy_rgba(
        &self,
        rect: ReadbackRect,
        dst: &mut [u8],
    ) -> Result<(), ReadbackError> {
        let expected = (rect.width as usize)
            .checked_mul(rect.height as usize)
            .and_then(|pixels| pixels.checked_mul(4));
        if rect.width == 0
            || rect.height == 0
            || expected != Some(rect.bytes)
            || dst.len() != rect.bytes
        {
            return Err(ReadbackError::DestinationLength);
        }
        let Some(bitmap) = &self.bitmap else {
            dst.fill(0);
            return Ok(());
        };
        let row_bytes = rect.width as usize * 4;
        for (row, output) in dst.chunks_exact_mut(row_bytes).enumerate() {
            let y = rect.y + row as i64;
            if y < 0 || y >= bitmap.height as i64 {
                output.fill(0);
                continue;
            }
            for (column, rgba) in output.chunks_exact_mut(4).enumerate() {
                let x = rect.x + column as i64;
                let pixel = if x >= 0 && x < bitmap.width as i64 {
                    bitmap.pixels[y as usize * bitmap.width + x as usize]
                } else {
                    0
                };
                rgba.copy_from_slice(&[
                    (pixel >> 16) as u8,
                    (pixel >> 8) as u8,
                    pixel as u8,
                    (pixel >> 24) as u8,
                ]);
            }
        }
        Ok(())
    }

    /// Logical retained pixel bytes, using the existing Bitmap accounting.
    pub(crate) fn retained_bytes(&self) -> usize {
        self.bitmap.as_ref().map_or(0, |b| b.pixels.len() * 4)
    }

    /// Infallible after a DOM dimension commit, even for equal or oversized
    /// dimensions. The owner credits these released bytes once, bounded by its
    /// document allowance. No mutable external total can fail after the reset.
    pub(crate) fn reset(&mut self, width: u32, height: u32) -> usize {
        let released = self.release();
        self.width = width;
        self.height = height;
        released
    }

    /// Drop storage but keep reflected dimensions. Repeated release returns 0.
    pub(crate) fn release(&mut self) -> usize {
        let released = self.retained_bytes();
        self.bitmap = None;
        released
    }

    fn raster_pixels(&self) -> Result<usize, String> {
        let count = (self.width as usize).checked_mul(self.height as usize);
        if self.width > MAX_DIMENSION
            || self.height > MAX_DIMENSION
            || count.is_none_or(|n| n > MAX_PIXELS)
        {
            return Err(
                "Canvas 2D bitmap exceeds the 4-megapixel or 16384-pixel dimension limit.".into(),
            );
        }
        Ok(count.unwrap())
    }

    /// Prepay one bounded operation plus every pixel initialized or visited.
    /// Budget refusal leaves pixels and both counters unchanged. Allocation
    /// failure retains the work debit, but never debits document pixel bytes.
    fn prepare(
        &mut self,
        bounds: Bounds,
        remaining_bytes: &mut usize,
        work: &mut usize,
        allocate: bool,
    ) -> Result<(), String> {
        let pixels = self.raster_pixels()?;
        let initialize = if allocate && self.bitmap.is_none() {
            pixels
        } else {
            0
        };
        let bytes = initialize
            .checked_mul(4)
            .ok_or("Canvas 2D bitmap byte count overflow.")?;
        let charge = bounds
            .visits()
            .and_then(|n| n.checked_add(initialize))
            .and_then(|n| n.checked_add(1))
            .filter(|&n| n <= MAX_OPERATION_WORK)
            .ok_or("Canvas 2D paint operation exceeds its work limit.")?;
        if bytes > *remaining_bytes {
            return Err("Canvas 2D document bitmap byte allowance exceeded.".into());
        }
        if charge > *work {
            return Err("Canvas 2D paint work allowance exceeded.".into());
        }
        *work -= charge;
        if initialize != 0 {
            let mut data = Vec::new();
            data.try_reserve_exact(pixels)
                .map_err(|_| "Could not allocate the Canvas 2D bitmap.")?;
            data.resize(pixels, 0);
            self.bitmap = Some(Bitmap {
                width: self.width as usize,
                height: self.height as usize,
                pixels: data,
            });
            *remaining_bytes -= bytes;
        }
        Ok(())
    }

    /// Source-over a clipped, axis-aligned rectangle. Color is 0xAARRGGBB.
    /// Fractional edge coverage is its area inside each pixel, quantized once
    /// to the image renderer's eight-bit alpha. No transform or clip-path state
    /// is implied. True means a material rectangle visited visible pixels;
    /// repeated same-color writes need not change their final values.
    pub(crate) fn fill_rect(
        &mut self,
        rect: [f64; 4],
        color: u32,
        remaining_bytes: &mut usize,
        work: &mut usize,
    ) -> Result<bool, String> {
        let Some(bounds) = Bounds::clipped(rect, self.width, self.height) else {
            return Ok(false);
        };
        if color >> 24 == 0 {
            return Ok(false);
        }
        self.prepare(bounds, remaining_bytes, work, true)?;
        let bitmap = self.bitmap.as_mut().unwrap();
        for y in bounds.y0..bounds.y1 {
            for x in bounds.x0..bounds.x1 {
                let alpha = (f64::from(color >> 24) * bounds.coverage(x, y)).round() as u32;
                let source = (color & 0x00ff_ffff) | (alpha << 24);
                let pixel = &mut bitmap.pixels[y * bitmap.width + x];
                *pixel = source_over(source, *pixel);
            }
        }
        Ok(true)
    }

    /// Clear covered pixels to transparent black. Fractional coverage removes
    /// the corresponding alpha fraction without introducing a source color.
    /// Clearing an untouched transparent surface never allocates backing.
    pub(crate) fn clear_rect(
        &mut self,
        rect: [f64; 4],
        remaining_bytes: &mut usize,
        work: &mut usize,
    ) -> Result<bool, String> {
        let Some(bounds) = Bounds::clipped(rect, self.width, self.height) else {
            return Ok(false);
        };
        if self.bitmap.is_none() {
            return Ok(false);
        }
        self.prepare(bounds, remaining_bytes, work, false)?;
        let bitmap = self.bitmap.as_mut().unwrap();
        for y in bounds.y0..bounds.y1 {
            for x in bounds.x0..bounds.x1 {
                let pixel = &mut bitmap.pixels[y * bitmap.width + x];
                let alpha =
                    (f64::from(*pixel >> 24) * (1.0 - bounds.coverage(x, y))).round() as u32;
                *pixel = if alpha == 0 {
                    0
                } else {
                    (*pixel & 0x00ff_ffff) | (alpha << 24)
                };
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_bitmap_readback_rect_normalizes_and_checks_exact_output_bound() {
        let rect = ReadbackRect::new(3, 2, -2, -1, 8).unwrap();
        assert_eq!(rect, ReadbackRect::new(1, 1, 2, 1, 8).unwrap());
        assert_eq!((rect.width, rect.height, rect.byte_len()), (2, 1, 8));
        for (width, height) in [(0, 1), (1, 0), (0, 0)] {
            assert_eq!(
                ReadbackRect::new(0, 0, width, height, usize::MAX),
                Err(ReadbackError::ZeroSize)
            );
        }
        assert_eq!(
            ReadbackRect::new(0, 0, 2, 1, 7),
            Err(ReadbackError::TooLarge)
        );
        assert_eq!(
            ReadbackRect::new(i32::MIN, i32::MIN, i32::MIN, i32::MIN, usize::MAX),
            Err(ReadbackError::TooLarge)
        );
        let binary_limit = 4 * 1024 * 1024;
        assert_eq!(
            ReadbackRect::new(0, 0, 1024, 1024, binary_limit)
                .unwrap()
                .byte_len(),
            binary_limit
        );
        assert_eq!(
            ReadbackRect::new(0, 0, 1024, 1025, binary_limit),
            Err(ReadbackError::TooLarge)
        );
    }

    #[test]
    fn canvas_bitmap_readback_copies_rgba_in_row_order_and_clears_every_exterior_edge() {
        let mut surface = CanvasBitmap::new(3, 2);
        let (mut bytes, mut work) = (24, 1000);
        for (x, y, color) in [
            (0.0, 0.0, 0xff11_2233),
            (1.0, 0.0, 0x8040_2010),
            (2.0, 0.0, 0xffab_cdef),
            (0.0, 1.0, 0xff00_ff00),
            (2.0, 1.0, 0x4000_00ff),
        ] {
            surface
                .fill_rect([x, y, 1.0, 1.0], color, &mut bytes, &mut work)
                .unwrap();
        }
        let rect = ReadbackRect::new(-1, -1, 5, 4, 80).unwrap();
        let mut output = [99; 80];
        let counters = (bytes, work, surface.retained_bytes());
        surface.copy_rgba(rect, &mut output).unwrap();
        assert_eq!(
            output,
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 17, 34, 51,
                255, 64, 32, 16, 128, 171, 205, 239, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 255,
                0, 0, 0, 0, 0, 0, 255, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0,
            ]
        );
        assert_eq!((bytes, work, surface.retained_bytes()), counters);
        let mut negative = [0; 16];
        surface
            .copy_rgba(ReadbackRect::new(2, 2, -2, -2, 16).unwrap(), &mut negative)
            .unwrap();
        assert_eq!(
            negative,
            [17, 34, 51, 255, 64, 32, 16, 128, 0, 255, 0, 255, 0, 0, 0, 0]
        );
        output[24] = 0;
        assert_eq!(surface.bitmap().unwrap().pixels[0], 0xff11_2233);
        surface
            .clear_rect([0.0, 0.0, 3.0, 2.0], &mut bytes, &mut work)
            .unwrap();
        assert_eq!(&negative[..4], &[17, 34, 51, 255]);
    }

    #[test]
    fn canvas_bitmap_readback_transparent_unallocated_and_bad_destination_are_atomic() {
        let surface = CanvasBitmap::new(u32::MAX, u32::MAX);
        for (x, y) in [(0, 0), (i32::MIN, i32::MAX), (i32::MAX, i32::MIN)] {
            let mut output = [99; 4];
            surface
                .copy_rgba(ReadbackRect::new(x, y, 1, 1, 4).unwrap(), &mut output)
                .unwrap();
            assert_eq!(output, [0; 4]);
        }
        assert_eq!(surface.retained_bytes(), 0);
        for len in [3, 5] {
            let mut output = vec![99; len];
            assert_eq!(
                surface.copy_rgba(ReadbackRect::new(0, 0, 1, 1, 4).unwrap(), &mut output),
                Err(ReadbackError::DestinationLength)
            );
            assert_eq!(output, vec![99; len]);
        }
    }

    #[test]
    fn canvas_bitmap_lazy_clipped_fill_clear_and_independent_storage() {
        let mut surface = CanvasBitmap::new(4, 3);
        let other = CanvasBitmap::new(4, 3);
        let (mut bytes, mut work) = (1024, 1000);
        assert!(surface.bitmap().is_none());
        assert!(!surface
            .clear_rect([0.0, 0.0, 4.0, 3.0], &mut bytes, &mut work)
            .unwrap());
        assert!(!surface
            .fill_rect([0.0, 0.0, 4.0, 3.0], 0x00ff_ffff, &mut bytes, &mut work)
            .unwrap());
        assert_eq!((bytes, work), (1024, 1000));
        assert!(surface
            .fill_rect([1.0, 0.0, 2.0, 2.0], 0xff00_0000, &mut bytes, &mut work)
            .unwrap());
        assert_eq!((surface.retained_bytes(), bytes, work), (48, 976, 983));
        assert_eq!(
            surface.bitmap().unwrap().pixels,
            [
                0,
                0xff00_0000,
                0xff00_0000,
                0,
                0,
                0xff00_0000,
                0xff00_0000,
                0,
                0,
                0,
                0,
                0
            ]
        );
        assert!(surface
            .clear_rect([2.0, 1.0, 1.0, 1.0], &mut bytes, &mut work)
            .unwrap());
        assert_eq!(surface.bitmap().unwrap().pixels[6], 0);
        assert_eq!(surface.bitmap().unwrap().pixels[1], 0xff00_0000);
        assert!(other.bitmap().is_none());
        assert_eq!(bytes, 976);
    }

    #[test]
    fn canvas_bitmap_straight_alpha_and_fractional_area_do_not_leak_rgb() {
        let mut surface = CanvasBitmap::new(2, 1);
        let (mut bytes, mut work) = (8, 100);
        surface
            .fill_rect([0.0, 0.0, 1.0, 1.0], 0x80ff_0000, &mut bytes, &mut work)
            .unwrap();
        assert_eq!(surface.bitmap().unwrap().pixels, [0x80ff_0000, 0]);
        surface
            .fill_rect([0.0, 0.0, 1.0, 1.0], 0x8000_ff00, &mut bytes, &mut work)
            .unwrap();
        assert_eq!(surface.bitmap().unwrap().pixels[0], 0xc055_aa00);
        surface
            .clear_rect([0.5, 0.0, 0.5, 1.0], &mut bytes, &mut work)
            .unwrap();
        assert_eq!(surface.bitmap().unwrap().pixels[0], 0x6055_aa00);
        surface
            .fill_rect([1.0, 0.0, 0.5, 1.0], 0xff00_00ff, &mut bytes, &mut work)
            .unwrap();
        assert_eq!(surface.bitmap().unwrap().pixels[1], 0x8000_00ff);
        surface
            .clear_rect([0.0, 0.0, 2.0, 1.0], &mut bytes, &mut work)
            .unwrap();
        assert_eq!(surface.bitmap().unwrap().pixels, [0, 0]);
    }

    #[test]
    fn canvas_bitmap_budget_refusal_is_atomic_and_debits_exact_pixel_work() {
        let mut surface = CanvasBitmap::new(2, 2);
        let (mut bytes, mut work) = (15, 100);
        assert!(surface
            .fill_rect([0.0, 0.0, 2.0, 2.0], 0xff12_3456, &mut bytes, &mut work)
            .is_err());
        assert_eq!((bytes, work, surface.retained_bytes()), (15, 100, 0));
        bytes = 16;
        work = 8;
        assert!(surface
            .fill_rect([0.0, 0.0, 2.0, 2.0], 0xff12_3456, &mut bytes, &mut work)
            .is_err());
        assert_eq!((bytes, work, surface.retained_bytes()), (16, 8, 0));
        work = 9;
        surface
            .fill_rect([0.0, 0.0, 2.0, 2.0], 0xff12_3456, &mut bytes, &mut work)
            .unwrap();
        assert_eq!((bytes, work, surface.retained_bytes()), (0, 0, 16));
        assert!(surface
            .clear_rect([0.0, 0.0, 1.0, 1.0], &mut bytes, &mut work)
            .is_err());
        assert_eq!(surface.bitmap().unwrap().pixels, [0xff12_3456; 4]);
        assert_eq!(surface.release(), 16);
        assert_eq!(surface.release(), 0);
    }

    #[test]
    fn canvas_bitmap_dimension_reset_is_unconditional_and_drops_pixels_once() {
        let mut surface = CanvasBitmap::new(2, 2);
        let (mut bytes, mut work) = (16, 100);
        surface
            .fill_rect([0.0, 0.0, 2.0, 2.0], 0xff12_3456, &mut bytes, &mut work)
            .unwrap();
        assert_eq!(surface.reset(2, 2), 16);
        assert!(surface.bitmap().is_none());
        bytes += 16;
        surface
            .fill_rect([0.0, 0.0, 2.0, 2.0], 0xff00_0000, &mut bytes, &mut work)
            .unwrap();
        assert_eq!(surface.reset(u32::MAX, 2), 16);
        assert_eq!(surface.dimensions(), (u32::MAX, 2));
        assert!(surface.bitmap().is_none());
        assert!(surface
            .fill_rect([0.0, 0.0, 1.0, 1.0], 0xff00_0000, &mut bytes, &mut work)
            .is_err());
        assert_eq!(surface.reset(u32::MAX, 0), 0);
        assert!(!surface
            .fill_rect([0.0, 0.0, 1.0, 1.0], 0xff00_0000, &mut bytes, &mut work)
            .unwrap());
        assert_eq!(surface.retained_bytes(), 0);
        assert_eq!(surface.reset(2, 2), 0);
    }

    #[test]
    fn canvas_bitmap_negative_extents_huge_clips_and_nonfinite_noops_are_bounded() {
        let mut surface = CanvasBitmap::new(4, 3);
        let (mut bytes, mut work) = (48, 1000);
        surface
            .fill_rect([3.0, 2.0, -2.0, -1.0], 0xff00_ff00, &mut bytes, &mut work)
            .unwrap();
        assert_eq!(
            surface.bitmap().unwrap().pixels,
            [0, 0, 0, 0, 0, 0xff00_ff00, 0xff00_ff00, 0, 0, 0, 0, 0]
        );
        let before = work;
        for rect in [
            [f64::NAN, 0.0, 1.0, 1.0],
            [0.0, 0.0, f64::INFINITY, 1.0],
            [-f64::MAX, -f64::MAX, f64::MAX, f64::MAX],
            [0.0, 0.0, 0.0, 1.0],
        ] {
            assert!(!surface
                .fill_rect(rect, 0xffff_ffff, &mut bytes, &mut work)
                .unwrap());
        }
        assert_eq!(work, before);
        surface
            .fill_rect(
                [0.0, 0.0, f64::MAX, f64::MAX],
                0xffaa_bbcc,
                &mut bytes,
                &mut work,
            )
            .unwrap();
        assert_eq!(surface.bitmap().unwrap().pixels, [0xffaa_bbcc; 12]);
        assert_eq!(before - work, 13);
    }
}
