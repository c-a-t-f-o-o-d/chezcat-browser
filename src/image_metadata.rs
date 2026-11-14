//! The browser's shared, bounded Exif orientation subset. The IFD0 reader and
//! eight pixel mappings originated in our JPEG decoder; no metadata library is
//! used. It never follows thumbnail/sub-IFD pointers or opens external data.
//! Malformed or unsupported optional metadata does not invalidate image pixels.
//!
//! References: CIPA DC-008, PNG 3 eXIf, and the WebP RIFF container specification.

use super::{dimensions, reserved, Animation, Bitmap, DecodedImage, ImageFrame, ImageResult};
use std::collections::HashMap;
use std::sync::Arc;

/// JPEG APP1 includes the Exif identifier before its TIFF header.
pub(crate) fn jpeg_orientation(data: &[u8]) -> Option<u8> {
    tiff_orientation(data.strip_prefix(b"Exif\0\0")?)
}

/// WebP encoders use both Exif-identifier and raw-TIFF EXIF chunk payloads.
pub(crate) fn webp_orientation(data: &[u8]) -> Option<u8> {
    tiff_orientation(data.strip_prefix(b"Exif\0\0").unwrap_or(data))
}

/// PNG eXIf starts at the TIFF header, without the JPEG Exif identifier. This
/// reads only SHORT/count=1 tag 0x0112 in IFD0, with at most 1024 directory entries.
pub(crate) fn tiff_orientation(tiff: &[u8]) -> Option<u8> {
    let little = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let bytes: [u8; 2] = tiff.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(if little {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let bytes: [u8; 4] = tiff.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if little {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let start = u32_at(4)? as usize;
    if start < 8 {
        return None;
    }
    let entries = usize::from(u16_at(start)?);
    if entries > 1024 {
        return None;
    }
    let end = start.checked_add(2 + 12 * entries + 4)?;
    if end > tiff.len() {
        return None;
    }
    for index in 0..entries {
        let at = start + 2 + 12 * index;
        if u16_at(at)? == 0x112 && u16_at(at + 2)? == 3 && u32_at(at + 4)? == 1 {
            return u8::try_from(u16_at(at + 8)?)
                .ok()
                .filter(|value| (1..=8).contains(value));
        }
    }
    None
}

pub(crate) fn oriented_dimensions(width: usize, height: usize, orientation: u8) -> (usize, usize) {
    if (5..=8).contains(&orientation) {
        (height, width)
    } else {
        (width, height)
    }
}

/// Input coordinates are in the encoded canvas. Output coordinates are in the
/// upright canvas. JPEG uses this directly during color conversion, avoiding an
/// extra JPEG image allocation and preserving its existing pixel math.
pub(crate) fn oriented_pixel(
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    orientation: u8,
) -> (usize, usize) {
    match orientation {
        2 => (width - 1 - x, y),
        3 => (width - 1 - x, height - 1 - y),
        4 => (x, height - 1 - y),
        5 => (y, x),
        6 => (height - 1 - y, x),
        7 => (height - 1 - y, width - 1 - x),
        8 => (y, width - 1 - x),
        _ => (x, y),
    }
}

fn transformed(
    bitmap: &Bitmap,
    orientation: u8,
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<Bitmap> {
    let count = dimensions(bitmap.width, bitmap.height)?;
    if bitmap.pixels.len() != count {
        return Err("Cannot orient an image with inconsistent dimensions.".into());
    }
    let (width, height) = oriented_dimensions(bitmap.width, bitmap.height, orientation);
    let mut pixels = reserved(count)?;
    pixels.resize(count, 0);
    for y in 0..bitmap.height {
        check()?;
        for x in 0..bitmap.width {
            let (target_x, target_y) =
                oriented_pixel(bitmap.width, bitmap.height, x, y, orientation);
            pixels[target_y * width + target_x] = bitmap.pixels[y * bitmap.width + x];
        }
    }
    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

pub(crate) fn orient_bitmap(
    bitmap: Bitmap,
    orientation: u8,
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<Bitmap> {
    check()?;
    if !(2..=8).contains(&orientation) {
        return Ok(bitmap);
    }
    transformed(&bitmap, orientation, check)
}

/// Animation coordinates/disposal/blending are resolved by the format decoder
/// first. Transforming its complete snapshots then gives every displayed frame
/// the same orientation without changing its timing or frame rectangle rules.
pub(crate) fn orient_asset(
    asset: DecodedImage,
    orientation: u8,
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<DecodedImage> {
    check()?;
    if !(2..=8).contains(&orientation) {
        return Ok(asset);
    }
    asset.retained_bytes()?;
    let DecodedImage {
        bitmap,
        animation,
        notice,
    } = asset;
    let poster_key = Arc::as_ptr(&bitmap) as usize;
    let mut images = HashMap::new();
    images
        .try_reserve(animation.as_ref().map_or(1, |a| a.frames.len() + 1))
        .map_err(|_| "Not enough memory for oriented image references.")?;
    images.insert(poster_key, bitmap);
    let animation = animation.map(|animation| {
        Arc::try_unwrap(animation).unwrap_or_else(|animation| Animation {
            frames: animation
                .frames
                .iter()
                .map(|frame| ImageFrame {
                    bitmap: frame.bitmap.clone(),
                    delay_ms: frame.delay_ms,
                })
                .collect(),
            plays: animation.plays,
        })
    });
    let mut frame_keys = reserved(animation.as_ref().map_or(0, |a| a.frames.len()))?;
    let plays = animation.as_ref().map(|animation| animation.plays);
    if let Some(animation) = animation {
        for frame in animation.frames {
            let key = Arc::as_ptr(&frame.bitmap) as usize;
            frame_keys.push((key, frame.delay_ms));
            images.entry(key).or_insert(frame.bitmap);
        }
    }
    // Each unique input is retained once before conversion. Replacing entries
    // individually needs at most one extra canvas, not another whole animation.
    // Shared poster/frame pixels stay shared in the resulting animation.
    for bitmap in images.values_mut() {
        check()?;
        *bitmap = Arc::new(transformed(bitmap, orientation, check)?);
    }
    let mut frames = reserved(frame_keys.len())?;
    for (key, delay_ms) in frame_keys {
        frames.push(ImageFrame {
            bitmap: images[&key].clone(),
            delay_ms,
        });
    }
    let oriented = DecodedImage {
        bitmap: images[&poster_key].clone(),
        animation: plays.map(|plays| Arc::new(Animation { frames, plays })),
        notice,
    };
    oriented.retained_bytes()?;
    check()?;
    Ok(oriented)
}

#[cfg(test)]
#[path = "image_metadata_tests.rs"]
mod tests;
