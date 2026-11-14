//! Original PNG decoder: all standard sample depths/color types and Adam7.
//! Samples are scaled to straight-alpha RGBA8. APNG frames are composited with
//! bounded disposal/blending and timing metadata. Color management is separate;
//! ancillary metadata never causes requests or code.
//! Format reference: https://www.w3.org/TR/png-3/

use super::{
    be32, dimensions, pixel, reserved, Animation, Bitmap, DecodedImage, ImageFrame, ImageResult,
    MAX_ANIMATION_FRAMES, MAX_ANIMATION_PIXELS, MAX_INPUT_BYTES,
};
use crate::compression::{self, crc32_checked};
use std::sync::Arc;

// Keep the existing shared 32 MiB inflate ceiling. RGBA16 and Adam7 count all
// sample/filter bytes against it before allocation; the result is RGBA8.
const MAX_RAW_BYTES: usize = compression::MAX_OUTPUT_BYTES;
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

#[derive(Clone, Copy)]
struct Info {
    width: usize,
    height: usize,
    color: u8,
    depth: usize,
    channels: usize,
    interlaced: bool,
}

impl Info {
    fn passes(self) -> &'static [(usize, usize, usize, usize)] {
        if self.interlaced {
            &ADAM7
        } else {
            &[(0, 0, 1, 1)]
        }
    }

    fn maximum(self) -> u16 {
        ((1u32 << self.depth) - 1) as u16
    }

    fn row_bytes(self, width: usize) -> ImageResult<usize> {
        width
            .checked_mul(self.channels)
            .and_then(|n| n.checked_mul(self.depth))
            .and_then(|n| n.checked_add(7))
            .map(|n| n / 8)
            .ok_or_else(|| "PNG row is too large.".into())
    }
}

fn pass_size(size: usize, start: usize, step: usize) -> usize {
    if size <= start {
        0
    } else {
        (size - start).div_ceil(step)
    }
}

pub(super) fn decode(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    let (bitmap, orientation) = decode_raw(data, check)?;
    super::metadata::orient_bitmap(bitmap, orientation, check)
}

fn decode_raw(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<(Bitmap, u8)> {
    check()?;
    if data.len() > MAX_INPUT_BYTES || !data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid PNG signature or input size.".into());
    }
    let mut position = 8usize;
    let mut header: Option<Info> = None;
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut transparency: Option<Vec<u8>> = None;
    let mut compressed = Vec::new();
    let mut seen_data = false;
    let mut data_finished = false;
    let mut seen_end = false;
    let mut chunks = 0usize;
    let mut exif_seen = false;
    let mut orientation = None;

    while position < data.len() {
        check()?;
        chunks += 1;
        if chunks > 65_536 || data.len() - position < 12 {
            return Err("PNG chunk list is truncated or exceeds the chunk limit.".into());
        }
        let length = be32(&data[position..]) as usize;
        let end = position
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .filter(|&n| n <= data.len())
            .ok_or("PNG chunk extends beyond the input.")?;
        if length > 0x7fff_ffff {
            return Err("Invalid PNG chunk length.".into());
        }
        let kind = &data[position + 4..position + 8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            return Err("Invalid PNG chunk type.".into());
        }
        let contents = &data[position + 8..end - 4];
        if crc32_checked(&data[position + 4..end - 4], check)? != be32(&data[end - 4..end]) {
            return Err("PNG chunk checksum does not match.".into());
        }
        if header.is_none() && kind != b"IHDR" {
            return Err("PNG must begin with its image header.".into());
        }
        if seen_data && kind != b"IDAT" {
            data_finished = true;
        }
        match kind {
            b"eXIf" => {
                // Duplicate/invalid ancillary metadata is ignored rather than
                // turning an otherwise valid raster into a broken image.
                orientation = if exif_seen {
                    None
                } else {
                    super::metadata::tiff_orientation(contents)
                };
                exif_seen = true;
            }
            b"IHDR" => {
                if header.is_some() || position != 8 || length != 13 {
                    return Err("Invalid or duplicate PNG image header.".into());
                }
                let width = be32(contents) as usize;
                let height = be32(&contents[4..]) as usize;
                dimensions(width, height)?;
                let depth = contents[8] as usize;
                let channels = match (contents[9], depth) {
                    (0, 1 | 2 | 4 | 8 | 16) | (3, 1 | 2 | 4 | 8) => 1,
                    (2, 8 | 16) => 3,
                    (4, 8 | 16) => 2,
                    (6, 8 | 16) => 4,
                    _ => return Err("Invalid PNG sample depth/color type combination.".into()),
                };
                if contents[10] != 0 || contents[11] != 0 || contents[12] > 1 {
                    return Err("Unsupported PNG compression, filter, or interlace method.".into());
                }
                header = Some(Info {
                    width,
                    height,
                    color: contents[9],
                    depth,
                    channels,
                    interlaced: contents[12] == 1,
                });
            }
            b"PLTE" => {
                let info = header.ok_or("PNG header is missing.")?;
                if seen_data
                    || !palette.is_empty()
                    || transparency.is_some()
                    || matches!(info.color, 0 | 4)
                    || length == 0
                    || length > 768
                    || !length.is_multiple_of(3)
                    || (info.color == 3 && length / 3 > 1 << info.depth)
                {
                    return Err("Invalid PNG palette or palette ordering.".into());
                }
                for entry in contents.as_chunks::<3>().0 {
                    palette.push([entry[0], entry[1], entry[2], 255]);
                }
            }
            b"tRNS" => {
                let info = header.ok_or("PNG header is missing.")?;
                let valid = match info.color {
                    0 => length == 2,
                    2 => length == 6,
                    3 => !palette.is_empty() && length > 0 && length <= palette.len(),
                    _ => false,
                };
                if seen_data || transparency.is_some() || !valid {
                    return Err("Invalid PNG transparency chunk.".into());
                }
                if info.color == 3 {
                    for (index, &alpha) in contents.iter().enumerate() {
                        palette[index][3] = alpha;
                    }
                } else if contents
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .any(|key| u16::from_be_bytes(*key) > info.maximum())
                {
                    return Err("PNG transparency sample exceeds the declared depth.".into());
                }
                transparency = Some(contents.to_vec());
            }
            b"IDAT" => {
                let info = header.ok_or("PNG header is missing.")?;
                if data_finished || (info.color == 3 && palette.is_empty()) {
                    return Err("PNG image data is out of order or its palette is missing.".into());
                }
                compressed
                    .try_reserve(length)
                    .map_err(|_| "Not enough memory for PNG data.")?;
                compressed.extend_from_slice(contents);
                seen_data = true;
            }
            b"IEND" => {
                if length != 0 || !seen_data || end != data.len() {
                    return Err("Invalid PNG end chunk or trailing data.".into());
                }
                seen_end = true;
                break;
            }
            _ if kind[0].is_ascii_uppercase() => {
                return Err("PNG contains an unsupported critical chunk.".into())
            }
            _ => {}
        }
        position = end;
    }
    if !seen_end {
        return Err("PNG end chunk is missing.".into());
    }
    let info = header.ok_or("PNG header is missing.")?;
    Ok((
        raster(info, &palette, transparency.as_deref(), &compressed, check)?,
        orientation.unwrap_or(1),
    ))
}

fn raster(
    info: Info,
    palette: &[[u8; 4]],
    transparency: Option<&[u8]>,
    compressed: &[u8],
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<Bitmap> {
    let mut expected = 0usize;
    for &(x, y, dx, dy) in info.passes() {
        let width = pass_size(info.width, x, dx);
        let height = pass_size(info.height, y, dy);
        if width == 0 || height == 0 {
            continue;
        }
        expected = expected
            .checked_add(
                (info.row_bytes(width)? + 1)
                    .checked_mul(height)
                    .ok_or("PNG pass size overflow.")?,
            )
            .filter(|&n| n <= MAX_RAW_BYTES)
            .ok_or("PNG raster is too large.")?;
    }
    let mut raw = inflate(compressed, expected, check)?;
    let count = dimensions(info.width, info.height)?;
    let mut pixels = reserved(count)?;
    pixels.resize(count, 0);
    let mut pass_start = 0usize;
    let filter_distance = (info.channels * info.depth).div_ceil(8).max(1);
    let maximum = u32::from(info.maximum());
    let scaled = |value: u16| -> u8 { ((u32::from(value) * 255 + maximum / 2) / maximum) as u8 };
    for &(x, y, dx, dy) in info.passes() {
        let width = pass_size(info.width, x, dx);
        let height = pass_size(info.height, y, dy);
        if width == 0 || height == 0 {
            continue;
        }
        let stride = info.row_bytes(width)?;
        for row in 0..height {
            check()?;
            let filter_position = pass_start + row * (stride + 1);
            let filter = raw[filter_position];
            if filter > 4 {
                return Err("Unknown PNG row filter.".into());
            }
            let start = filter_position + 1;
            for column in 0..stride {
                let left = if column >= filter_distance {
                    raw[start + column - filter_distance]
                } else {
                    0
                };
                let above = if row > 0 {
                    raw[start + column - stride - 1]
                } else {
                    0
                };
                let corner = if row > 0 && column >= filter_distance {
                    raw[start + column - stride - 1 - filter_distance]
                } else {
                    0
                };
                let prediction = match filter {
                    0 => 0,
                    1 => left,
                    2 => above,
                    3 => ((u16::from(left) + u16::from(above)) / 2) as u8,
                    4 => paeth(left, above, corner),
                    _ => unreachable!(),
                };
                raw[start + column] = raw[start + column].wrapping_add(prediction);
            }
            let row_data = &raw[start..start + stride];
            for column in 0..width {
                let mut values = [0u16; 4];
                for (channel, value) in values.iter_mut().enumerate().take(info.channels) {
                    let sample = column * info.channels + channel;
                    *value = match info.depth {
                        16 => u16::from_be_bytes([row_data[sample * 2], row_data[sample * 2 + 1]]),
                        8 => u16::from(row_data[sample]),
                        depth => {
                            let bit = sample * depth;
                            u16::from(
                                (row_data[bit / 8] >> (8 - depth - bit % 8)) & ((1 << depth) - 1),
                            )
                        }
                    };
                }
                let rgba = match info.color {
                    0 => {
                        let alpha = if transparency
                            .as_ref()
                            .is_some_and(|key| u16::from_be_bytes([key[0], key[1]]) == values[0])
                        {
                            0
                        } else {
                            255
                        };
                        [
                            scaled(values[0]),
                            scaled(values[0]),
                            scaled(values[0]),
                            alpha,
                        ]
                    }
                    2 => {
                        let transparent = transparency.as_ref().is_some_and(|key| {
                            key.as_chunks::<2>()
                                .0
                                .iter()
                                .zip(values.iter())
                                .all(|(key, value)| u16::from_be_bytes(*key) == *value)
                        });
                        [
                            scaled(values[0]),
                            scaled(values[1]),
                            scaled(values[2]),
                            if transparent { 0 } else { 255 },
                        ]
                    }
                    3 => *palette
                        .get(values[0] as usize)
                        .ok_or("PNG palette index is out of range.")?,
                    4 => [
                        scaled(values[0]),
                        scaled(values[0]),
                        scaled(values[0]),
                        scaled(values[1]),
                    ],
                    6 => [
                        scaled(values[0]),
                        scaled(values[1]),
                        scaled(values[2]),
                        scaled(values[3]),
                    ],
                    _ => unreachable!(),
                };
                pixels[(y + row * dy) * info.width + x + column * dx] =
                    pixel(rgba[0], rgba[1], rgba[2], rgba[3]);
            }
        }
        pass_start += (stride + 1) * height;
    }
    Ok(Bitmap {
        width: info.width,
        height: info.height,
        pixels,
    })
}

#[derive(Clone, Copy)]
struct FrameControl {
    width: usize,
    height: usize,
    left: usize,
    top: usize,
    delay_ms: u32,
    disposal: u8,
    blend: u8,
    default_image: bool,
}

struct PngAnimation {
    declared: usize,
    processed: usize,
    sequence: u32,
    plays: Option<u32>,
    control: Option<FrameControl>,
    compressed: Vec<u8>,
    data_seen: bool,
    canvas: Vec<u32>,
    frames: Vec<ImageFrame>,
    retain: bool,
    notice: Option<String>,
    work: usize,
}

impl PngAnimation {
    fn spend(&mut self, amount: usize, check: &dyn Fn() -> ImageResult<()>) -> ImageResult<()> {
        self.work = self
            .work
            .checked_sub(amount)
            .ok_or("APNG decoding work limit reached (128 Mi units).")?;
        check()
    }

    fn sequence(&mut self, value: u32) -> ImageResult<()> {
        if value != self.sequence {
            return Err(
                "APNG sequence numbers must start at zero with no gaps or duplicates.".into(),
            );
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("APNG sequence number overflow.")?;
        Ok(())
    }

    fn begin(&mut self, contents: &[u8], info: Info, seen_idat: bool) -> ImageResult<()> {
        if contents.len() != 26 || self.control.is_some() || self.processed >= self.declared {
            return Err("Invalid APNG frame control or frame count.".into());
        }
        self.sequence(be32(contents))?;
        let width = be32(&contents[4..]) as usize;
        let height = be32(&contents[8..]) as usize;
        let left = be32(&contents[12..]) as usize;
        let top = be32(&contents[16..]) as usize;
        dimensions(width, height)?;
        if left.checked_add(width).is_none_or(|n| n > info.width)
            || top.checked_add(height).is_none_or(|n| n > info.height)
            || contents[24] > 2
            || contents[25] > 1
        {
            return Err("APNG frame rectangle, disposal or blend mode is invalid.".into());
        }
        let default_image = !seen_idat;
        if default_image
            && (self.processed != 0
                || left != 0
                || top != 0
                || width != info.width
                || height != info.height)
        {
            return Err("APNG default frame must cover the complete PNG canvas.".into());
        }
        if self.processed == 0 {
            let retained = info.width * info.height * (self.declared + usize::from(!default_image));
            if retained > MAX_ANIMATION_PIXELS {
                self.retain = false;
                self.notice = Some("APNG animation exceeds the retained-frame pixel limit; showing its default image after validating all frames.".into());
            }
        }
        let numerator = u32::from(u16::from_be_bytes([contents[20], contents[21]]));
        let denominator = u32::from(u16::from_be_bytes([contents[22], contents[23]]));
        let denominator = if denominator == 0 { 100 } else { denominator };
        self.control = Some(FrameControl {
            width,
            height,
            left,
            top,
            delay_ms: (numerator * 1000 + denominator / 2) / denominator,
            disposal: contents[24],
            blend: contents[25],
            default_image,
        });
        self.data_seen = false;
        Ok(())
    }

    fn finish(
        &mut self,
        info: Info,
        palette: &[[u8; 4]],
        transparency: Option<&[u8]>,
        poster: &Arc<Bitmap>,
        check: &dyn Fn() -> ImageResult<()>,
    ) -> ImageResult<()> {
        let Some(control) = self.control.take() else {
            return Ok(());
        };
        if !self.data_seen {
            return Err("APNG frame has no image data.".into());
        }
        let area = control.width * control.height;
        self.spend(area, check)?;
        let decoded;
        let frame = if control.default_image {
            poster.as_ref()
        } else {
            decoded = raster(
                Info {
                    width: control.width,
                    height: control.height,
                    ..info
                },
                palette,
                transparency,
                &self.compressed,
                check,
            )?;
            &decoded
        };
        self.compressed.clear();
        let count = info.width * info.height;
        if self.canvas.is_empty() {
            self.canvas = reserved(count)?;
            while self.canvas.len() < count {
                let amount = (count - self.canvas.len()).min(4096);
                self.spend(amount, check)?;
                self.canvas.resize(self.canvas.len() + amount, 0);
            }
        }
        let mut restore = Vec::new();
        if control.disposal == 2 && self.processed != 0 {
            restore = reserved(area)?;
            for row in 0..control.height {
                self.spend(control.width, check)?;
                let start = (control.top + row) * info.width + control.left;
                restore.extend_from_slice(&self.canvas[start..start + control.width]);
            }
        }
        for row in 0..control.height {
            self.spend(control.width, check)?;
            let start = (control.top + row) * info.width + control.left;
            let destination = &mut self.canvas[start..start + control.width];
            let source = &frame.pixels[row * control.width..(row + 1) * control.width];
            if control.blend == 0 {
                destination.copy_from_slice(source);
            } else {
                for (target, &source) in destination.iter_mut().zip(source) {
                    *target = over(source, *target);
                }
            }
        }
        if self.retain {
            let bitmap = if control.default_image {
                // At the first frame the canvas is transparent, so SOURCE and
                // OVER are visually identical to the default PNG. Share it.
                poster.clone()
            } else {
                let mut pixels = reserved(count)?;
                for start in (0..count).step_by(4096) {
                    let end = (start + 4096).min(count);
                    self.spend(end - start, check)?;
                    pixels.extend_from_slice(&self.canvas[start..end]);
                }
                Arc::new(Bitmap {
                    width: info.width,
                    height: info.height,
                    pixels,
                })
            };
            self.frames.push(ImageFrame {
                bitmap,
                delay_ms: control.delay_ms,
            });
        }
        if control.disposal != 0 {
            for row in 0..control.height {
                self.spend(control.width, check)?;
                let start = (control.top + row) * info.width + control.left;
                let destination = &mut self.canvas[start..start + control.width];
                if control.disposal == 1 || self.processed == 0 {
                    destination.fill(0);
                } else {
                    destination
                        .copy_from_slice(&restore[row * control.width..(row + 1) * control.width]);
                }
            }
        }
        self.processed += 1;
        Ok(())
    }
}

/// Straight-alpha source-over. Work in integer premultiplied numerators, then
/// unpremultiply only once; fully transparent colors never leak into the result.
fn over(source: u32, target: u32) -> u32 {
    let source_alpha = source >> 24;
    if source_alpha == 255 {
        return source;
    }
    if source_alpha == 0 {
        return target;
    }
    let target_alpha = target >> 24;
    let inverse = 255 - source_alpha;
    let alpha = source_alpha * 255 + target_alpha * inverse;
    let channel = |shift: u32| -> u32 {
        let source = (source >> shift) & 255u32;
        let target = (target >> shift) & 255u32;
        (source * source_alpha * 255 + target * target_alpha * inverse + alpha / 2) / alpha
    };
    (((alpha + 127) / 255) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

pub(super) fn decode_asset(
    data: &[u8],
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<DecodedImage> {
    // The static parser validates every chunk boundary, CRC, critical chunk and
    // palette ordering, and decodes the mandatory default image exactly once.
    let (poster, orientation) = decode_raw(data, check)?;
    let poster = Arc::new(poster);
    let channels = match data[25] {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => unreachable!(),
    };
    let info = Info {
        width: poster.width,
        height: poster.height,
        color: data[25],
        depth: data[24] as usize,
        channels,
        interlaced: data[28] == 1,
    };
    let mut palette = Vec::new();
    let mut transparency = None;
    let mut animation: Option<PngAnimation> = None;
    let mut seen_idat = false;
    let mut position = 8;
    while position < data.len() {
        check()?;
        // These slices were validated by decode() above; source is immutable.
        let length = be32(&data[position..]) as usize;
        let kind = &data[position + 4..position + 8];
        let contents = &data[position + 8..position + 8 + length];
        match kind {
            b"PLTE" => {
                palette = contents
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|c| [c[0], c[1], c[2], 255])
                    .collect()
            }
            b"tRNS" => {
                if info.color == 3 {
                    for (entry, &alpha) in palette.iter_mut().zip(contents) {
                        entry[3] = alpha;
                    }
                } else {
                    transparency = Some(contents);
                }
            }
            b"acTL" => {
                if seen_idat || animation.is_some() || length != 8 {
                    return Err(
                        "APNG animation control must occur once before PNG image data.".into(),
                    );
                }
                let declared = be32(contents) as usize;
                let plays = be32(&contents[4..]);
                if declared == 0 || declared > MAX_ANIMATION_FRAMES || plays > 0x7fff_ffff {
                    return Err(
                        "APNG animation frame count or play count exceeds its limit.".into(),
                    );
                }
                animation = Some(PngAnimation {
                    declared,
                    processed: 0,
                    sequence: 0,
                    plays: (plays != 0).then_some(plays),
                    control: None,
                    compressed: Vec::new(),
                    data_seen: false,
                    canvas: Vec::new(),
                    frames: Vec::new(),
                    retain: true,
                    notice: None,
                    work: 128 * 1024 * 1024,
                });
            }
            b"fcTL" => {
                let animation = animation
                    .as_mut()
                    .ok_or("APNG frame control has no preceding animation control.")?;
                animation.finish(info, &palette, transparency, &poster, check)?;
                animation.begin(contents, info, seen_idat)?;
            }
            b"IDAT" => {
                seen_idat = true;
                if let Some(animation) = &mut animation {
                    if let Some(control) = animation.control {
                        if !control.default_image {
                            return Err("PNG default data cannot encode a later APNG frame.".into());
                        }
                        animation.data_seen = true;
                    }
                }
            }
            b"fdAT" => {
                let animation = animation
                    .as_mut()
                    .ok_or("APNG frame data has no preceding animation control.")?;
                if length < 4 || !seen_idat {
                    return Err(
                        "APNG frame data is truncated or precedes the PNG default image.".into(),
                    );
                }
                animation.sequence(be32(contents))?;
                if animation.control.is_none_or(|c| c.default_image) {
                    return Err("APNG frame data has no matching frame control.".into());
                }
                animation
                    .compressed
                    .try_reserve(length - 4)
                    .map_err(|_| "Not enough memory for APNG compressed data.")?;
                animation.compressed.extend_from_slice(&contents[4..]);
                animation.data_seen = true;
            }
            b"IEND" => {
                if let Some(animation) = &mut animation {
                    animation.finish(info, &palette, transparency, &poster, check)?;
                    if animation.processed != animation.declared {
                        return Err("APNG declared frame count does not match its frames.".into());
                    }
                }
                break;
            }
            _ => {}
        }
        position += length + 12;
    }
    let decoded = if let Some(animation) = animation {
        DecodedImage {
            bitmap: poster,
            animation: animation.retain.then(|| {
                Arc::new(Animation {
                    frames: animation.frames,
                    plays: animation.plays,
                })
            }),
            notice: animation.notice,
        }
    } else {
        DecodedImage {
            bitmap: poster,
            animation: None,
            notice: None,
        }
    };
    super::metadata::orient_asset(decoded, orientation, check)
}

fn paeth(left: u8, above: u8, corner: u8) -> u8 {
    let prediction = i32::from(left) + i32::from(above) - i32::from(corner);
    let a = (prediction - i32::from(left)).abs();
    let b = (prediction - i32::from(above)).abs();
    let c = (prediction - i32::from(corner)).abs();
    if a <= b && a <= c {
        left
    } else if b <= c {
        above
    } else {
        corner
    }
}

pub(super) fn inflate(
    data: &[u8],
    expected: usize,
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<Vec<u8>> {
    if expected > MAX_RAW_BYTES {
        return Err("PNG decompression exceeds its raster limit.".into());
    }
    let output = compression::zlib(data, expected, check)?;
    if output.len() != expected {
        return Err("PNG compressed data has an unexpected raster size.".into());
    }
    Ok(output)
}

#[cfg(test)]
mod animation_tests {
    use super::*;

    struct Frame {
        width: u32,
        left: u32,
        pixels: Vec<u32>,
        delay: (u16, u16),
        disposal: u8,
        blend: u8,
    }

    fn frame(left: u32, pixels: &[u32], disposal: u8, blend: u8) -> Frame {
        Frame {
            width: pixels.len() as u32,
            left,
            pixels: pixels.to_vec(),
            delay: (7, 100),
            disposal,
            blend,
        }
    }

    fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], contents: &[u8]) {
        output.extend_from_slice(&(contents.len() as u32).to_be_bytes());
        let start = output.len();
        output.extend_from_slice(kind);
        output.extend_from_slice(contents);
        output.extend_from_slice(&compression::crc32(&output[start..]).to_be_bytes());
    }

    // Original stored-DEFLATE fixture writer, deliberately independent of the
    // PNG decoder and small enough that no compression library is needed.
    fn image_data(pixels: &[u32]) -> Vec<u8> {
        let mut raw = vec![0];
        for &p in pixels {
            raw.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
        let size = raw.len() as u16;
        let mut result = vec![0x78, 0x01, 0x01];
        result.extend_from_slice(&size.to_le_bytes());
        result.extend_from_slice(&(!size).to_le_bytes());
        result.extend_from_slice(&raw);
        result.extend_from_slice(&compression::adler32(&raw).to_be_bytes());
        result
    }

    fn fixture(width: u32, poster: Option<&[u32]>, frames: &[Frame], plays: u32) -> Vec<u8> {
        let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = width.to_be_bytes().to_vec();
        header.extend_from_slice(&1u32.to_be_bytes());
        header.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut output, b"IHDR", &header);
        let mut control = (frames.len() as u32).to_be_bytes().to_vec();
        control.extend_from_slice(&plays.to_be_bytes());
        chunk(&mut output, b"acTL", &control);
        if let Some(poster) = poster {
            chunk(&mut output, b"IDAT", &image_data(poster));
        }
        let mut sequence = 0u32;
        for (index, frame) in frames.iter().enumerate() {
            let mut control = sequence.to_be_bytes().to_vec();
            for value in [frame.width, 1, frame.left, 0] {
                control.extend_from_slice(&value.to_be_bytes());
            }
            control.extend_from_slice(&frame.delay.0.to_be_bytes());
            control.extend_from_slice(&frame.delay.1.to_be_bytes());
            control.extend_from_slice(&[frame.disposal, frame.blend]);
            chunk(&mut output, b"fcTL", &control);
            sequence += 1;
            let mut compressed = image_data(&frame.pixels);
            if index == 0 && poster.is_none() {
                chunk(&mut output, b"IDAT", &compressed);
            } else {
                let mut data = sequence.to_be_bytes().to_vec();
                data.append(&mut compressed);
                chunk(&mut output, b"fdAT", &data);
                sequence += 1;
            }
        }
        chunk(&mut output, b"IEND", &[]);
        output
    }

    #[test]
    fn alpha_source_over_preserves_straight_alpha_and_transparent_background() {
        assert_eq!(over(0x8064c832, 0x80c86400), 0xc085a721);
        assert_eq!(over(0x8000ff00, 0x80ff0000), 0xc055aa00);
        assert_eq!(over(0x00112233, 0x80445566), 0x80445566);
        assert_eq!(over(0x80778899, 0x00112233), 0x80778899);
        assert_eq!(over(0xffaabbcc, 0x80112233), 0xffaabbcc);
    }

    #[test]
    fn frame_compositing_restores_previous_clears_and_overwrites_transparency() {
        let frames = [
            frame(0, &[0x80ff0000, 0xff0000ff], 0, 0),
            frame(0, &[0x8000ff00], 2, 1),
            frame(1, &[0x00ffffff], 1, 0),
            frame(1, &[0xff00ff00], 0, 1),
        ];
        let data = fixture(2, None, &frames, 3);
        let asset = crate::image::decode_asset_checked(&data, &|| Ok(())).unwrap();
        let animation = asset.animation.as_ref().unwrap();
        assert_eq!(animation.plays, Some(3));
        assert_eq!(animation.frames.len(), 4);
        assert!(Arc::ptr_eq(&asset.bitmap, &animation.frames[0].bitmap));
        assert_eq!(animation.frames[0].bitmap.pixels, [0x80ff0000, 0xff0000ff]);
        assert_eq!(animation.frames[1].bitmap.pixels, [0xc055aa00, 0xff0000ff]);
        assert_eq!(animation.frames[2].bitmap.pixels, [0x80ff0000, 0x00ffffff]);
        assert_eq!(animation.frames[3].bitmap.pixels, [0x80ff0000, 0xff00ff00]);
        assert_eq!(animation.frames[1].delay_ms, 70);
        assert_eq!(asset.retained_bytes().unwrap(), 4 * 2 * 4);
        assert_eq!(
            decode(&data, &|| Ok(())).unwrap().pixels,
            asset.bitmap.pixels
        );
    }

    #[test]
    fn separate_poster_is_excluded_from_animation_and_first_previous_clears() {
        let poster = [0xff123456, 0xffaabbcc];
        let mut frames = [frame(1, &[0x80ff0000], 2, 1), frame(0, &[0xff00ff00], 0, 1)];
        frames[0].delay = (0, 0);
        frames[1].delay = (1, 3);
        let asset = decode_asset(&fixture(2, Some(&poster), &frames, 0), &|| Ok(())).unwrap();
        let animation = asset.animation.as_ref().unwrap();
        assert_eq!(asset.bitmap.pixels, poster);
        assert_eq!(animation.plays, None);
        assert_eq!(animation.frames[0].bitmap.pixels, [0, 0x80ff0000]);
        assert_eq!(animation.frames[1].bitmap.pixels, [0xff00ff00, 0]);
        assert_eq!(animation.frames[0].delay_ms, 0);
        assert_eq!(animation.frames[1].delay_ms, 333);
        assert_eq!(asset.retained_bytes().unwrap(), 3 * 2 * 4);
        let single = decode_asset(&fixture(2, Some(&poster), &frames[..1], 1), &|| Ok(())).unwrap();
        assert_eq!(single.animation.as_ref().unwrap().frames.len(), 1);
    }

    #[test]
    fn animation_errors_never_replace_the_static_png_default() {
        let mut invalid_frame = frame(0, &[0xff123456], 0, 0);
        invalid_frame.blend = 2;
        let invalid = fixture(1, None, &[invalid_frame], 0);
        assert_eq!(decode(&invalid, &|| Ok(())).unwrap().pixels, [0xff123456]);
        assert!(decode_asset(&invalid, &|| Ok(()))
            .unwrap_err()
            .contains("blend"));
        let valid = fixture(1, None, &[frame(0, &[0xff123456], 0, 0)], 1);
        for end in 0..valid.len() {
            assert!(
                decode_asset(&valid[..end], &|| Ok(())).is_err(),
                "accepted prefix {end}"
            );
        }
    }
}
