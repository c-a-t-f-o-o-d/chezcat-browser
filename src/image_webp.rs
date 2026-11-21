//! Original WebP lossless (VP8L), lossy (VP8), alpha and animation decoder.
//! No codec or platform image API. Transform images,
//! prefix-code groups, pixels, work and input are bounded before allocation.
//! Format references: https://developers.google.com/speed/webp/docs/riff_container
//! and https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification

use super::{
    dimensions, le16, le32, pixel, reserved, Animation, Bitmap, DecodedImage, ImageFrame,
    ImageResult, MAX_ANIMATION_FRAMES, MAX_ANIMATION_PIXELS,
};
use std::sync::Arc;

#[path = "image_vp8.rs"]
mod vp8;
pub(crate) use vp8::VideoDecoder;

pub(super) fn decode_vp8_frame(data:&[u8],maximum_pixels:usize,check:&dyn Fn()->ImageResult<()>)->ImageResult<Bitmap> {
    vp8::decode_with_pixels(data,check,maximum_pixels)
}

const MAX_GROUPS: usize = 512;
const MAX_DECODED_PIXELS: usize = 8 * 1024 * 1024;
const MAX_WORK: usize = 512 * 1024 * 1024;

struct Bits<'a> {
    data: &'a [u8],
    at: usize,
    work: usize,
    since_check: usize,
    decoded: usize,
    check: &'a dyn Fn() -> ImageResult<()>,
}
impl Bits<'_> {
    fn charge(&mut self, amount: usize) -> ImageResult<()> {
        self.work = self
            .work
            .checked_sub(amount)
            .ok_or("WebP decode work limit reached.")?;
        self.since_check += amount;
        if self.since_check >= 8192 {
            (self.check)()?;
            self.since_check = 0;
        }
        Ok(())
    }
    fn take(&mut self, count: usize) -> ImageResult<usize> {
        if count > 24 || self.at + count > self.data.len() * 8 {
            return Err("Truncated or invalid WebP bitstream.".into());
        }
        self.charge(count.max(1))?;
        if count == 0 {
            return Ok(0);
        }
        let byte = self.at / 8;
        let shift = self.at % 8;
        let length = (shift + count).div_ceil(8);
        let mut word = 0u32;
        for i in 0..length {
            word |= u32::from(self.data[byte + i]) << (8 * i);
        }
        self.at += count;
        Ok(((word >> shift) & ((1u32 << count) - 1)) as usize)
    }
    fn peek_byte(&self) -> Option<usize> {
        if self.at + 8 > self.data.len() * 8 {
            return None;
        }
        let byte = self.at / 8;
        let shift = self.at % 8;
        let word =
            u16::from(self.data[byte]) | (u16::from(*self.data.get(byte + 1).unwrap_or(&0)) << 8);
        Some(((word >> shift) & 255) as usize)
    }
    fn pixel_budget(&mut self, width: usize, height: usize) -> ImageResult<usize> {
        let count = dimensions(width, height)?;
        self.decoded = self
            .decoded
            .checked_add(count)
            .filter(|&n| n <= MAX_DECODED_PIXELS)
            .ok_or("WebP transform-image pixel budget reached.")?;
        self.charge(count)?;
        Ok(count)
    }
}

struct Prefix {
    single: Option<u16>,
    first: [u32; 16],
    counts: [u16; 16],
    start: [usize; 16],
    symbols: Vec<u16>,
    fast: Vec<u16>,
}
impl Prefix {
    fn build(lengths: &[u8], bits: &mut Bits<'_>) -> ImageResult<Self> {
        bits.charge(lengths.len())?;
        let mut counts = [0u16; 16];
        for &length in lengths {
            if length > 15 {
                return Err("WebP prefix code is longer than 15 bits.".into());
            }
            if length != 0 {
                counts[length as usize] += 1;
            }
        }
        let leaves = counts.iter().map(|&n| n as usize).sum::<usize>();
        if leaves == 0 {
            return Err("WebP prefix tree is empty.".into());
        }
        let single = if leaves == 1 {
            if counts[1] != 1 {
                return Err("Invalid WebP single-symbol prefix tree.".into());
            }
            Some(lengths.iter().position(|&n| n != 0).unwrap() as u16)
        } else {
            None
        };
        let mut available = 1i32;
        for &count in &counts[1..] {
            available = available * 2 - i32::from(count);
            if available < 0 {
                return Err("Oversubscribed WebP prefix tree.".into());
            }
        }
        if single.is_none() && available != 0 {
            return Err("Incomplete WebP prefix tree.".into());
        }
        let mut first = [0u32; 16];
        let mut start = [0usize; 16];
        let mut code = 0u32;
        let mut offset = 0usize;
        for length in 1..16 {
            code = (code + u32::from(counts[length - 1])) << 1;
            first[length] = code;
            start[length] = offset;
            offset += counts[length] as usize;
        }
        let mut symbols = reserved(leaves)?;
        symbols.resize(leaves, 0u16);
        let mut positions = start;
        let mut fast = reserved(if single.is_some() { 0 } else { 256 })?;
        if single.is_none() {
            fast.resize(256, 0u16);
        }
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let length = length as usize;
            let canonical = first[length] + (positions[length] - start[length]) as u32;
            symbols[positions[length]] = symbol as u16;
            positions[length] += 1;
            if length <= 8 && single.is_none() {
                let reverse = (canonical.reverse_bits() >> (32 - length)) as usize;
                for i in (reverse..256).step_by(1 << length) {
                    fast[i] = ((symbol as u16) << 4) | length as u16;
                }
            }
        }
        Ok(Self {
            single,
            first,
            counts,
            start,
            symbols,
            fast,
        })
    }
    fn read(&self, bits: &mut Bits<'_>) -> ImageResult<usize> {
        if let Some(single) = self.single {
            bits.charge(1)?;
            return Ok(single as usize);
        }
        if let Some(byte) = bits.peek_byte() {
            let entry = self.fast[byte];
            if entry != 0 {
                bits.take((entry & 15) as usize)?;
                return Ok((entry >> 4) as usize);
            }
        }
        let mut code = 0u32;
        for length in 1..16 {
            code = (code << 1) | bits.take(1)? as u32;
            if code >= self.first[length]
                && code - self.first[length] < u32::from(self.counts[length])
            {
                return Ok(
                    self.symbols[self.start[length] + (code - self.first[length]) as usize]
                        as usize,
                );
            }
        }
        Err("Invalid WebP prefix code.".into())
    }
}

fn prefix(bits: &mut Bits<'_>, alphabet: usize) -> ImageResult<Prefix> {
    let mut lengths = reserved(alphabet)?;
    lengths.resize(alphabet, 0u8);
    if bits.take(1)? != 0 {
        let count = bits.take(1)? + 1;
        let width = 1 + 7 * bits.take(1)?;
        let first = bits.take(width)?;
        if first >= alphabet {
            return Err("WebP simple prefix symbol exceeds its alphabet.".into());
        }
        lengths[first] = 1;
        if count == 2 {
            let second = bits.take(8)?;
            if second >= alphabet {
                return Err("WebP simple prefix symbol exceeds its alphabet.".into());
            }
            // Repeated symbols are explicitly permitted by the format.
            lengths[second] = 1;
        }
    } else {
        const ORDER: [usize; 19] = [
            17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ];
        let count = bits.take(4)? + 4;
        let mut codes = [0u8; 19];
        for &index in &ORDER[..count] {
            codes[index] = bits.take(3)? as u8;
        }
        let table = Prefix::build(&codes, bits)?;
        let max_symbols = if bits.take(1)? == 0 {
            alphabet
        } else {
            let count = 2 + 2 * bits.take(3)?;
            bits.take(count)? + 2
        };
        if max_symbols > alphabet {
            return Err("WebP prefix description exceeds its alphabet.".into());
        }
        let mut position = 0usize;
        let mut previous = 8u8;
        for _ in 0..max_symbols {
            if position == alphabet {
                break;
            }
            let code = table.read(bits)?;
            let (value, repeat) = match code {
                0..=15 => {
                    if code != 0 {
                        previous = code as u8;
                    }
                    (code as u8, 1)
                }
                16 => (previous, bits.take(2)? + 3),
                17 => (0, bits.take(3)? + 3),
                18 => (0, bits.take(7)? + 11),
                _ => return Err("Invalid WebP prefix-length code.".into()),
            };
            if position + repeat > alphabet {
                return Err("WebP repeated code length exceeds its alphabet.".into());
            }
            lengths[position..position + repeat].fill(value);
            position += repeat;
        }
    }
    Prefix::build(&lengths, bits)
}

fn prefixed_number(bits: &mut Bits<'_>, code: usize) -> ImageResult<usize> {
    if code < 4 {
        Ok(code + 1)
    } else {
        let extra = (code - 2) / 2;
        Ok(((2 + (code & 1)) << extra) + bits.take(extra)? + 1)
    }
}

// Normative near-neighbor distance order from the VP8L bitstream specification.
const NEIGHBORS: [(i32, i32); 120] = [
    (0, 1),
    (1, 0),
    (1, 1),
    (-1, 1),
    (0, 2),
    (2, 0),
    (1, 2),
    (-1, 2),
    (2, 1),
    (-2, 1),
    (2, 2),
    (-2, 2),
    (0, 3),
    (3, 0),
    (1, 3),
    (-1, 3),
    (3, 1),
    (-3, 1),
    (2, 3),
    (-2, 3),
    (3, 2),
    (-3, 2),
    (0, 4),
    (4, 0),
    (1, 4),
    (-1, 4),
    (4, 1),
    (-4, 1),
    (3, 3),
    (-3, 3),
    (2, 4),
    (-2, 4),
    (4, 2),
    (-4, 2),
    (0, 5),
    (3, 4),
    (-3, 4),
    (4, 3),
    (-4, 3),
    (5, 0),
    (1, 5),
    (-1, 5),
    (5, 1),
    (-5, 1),
    (2, 5),
    (-2, 5),
    (5, 2),
    (-5, 2),
    (4, 4),
    (-4, 4),
    (3, 5),
    (-3, 5),
    (5, 3),
    (-5, 3),
    (0, 6),
    (6, 0),
    (1, 6),
    (-1, 6),
    (6, 1),
    (-6, 1),
    (2, 6),
    (-2, 6),
    (6, 2),
    (-6, 2),
    (4, 5),
    (-4, 5),
    (5, 4),
    (-5, 4),
    (3, 6),
    (-3, 6),
    (6, 3),
    (-6, 3),
    (0, 7),
    (7, 0),
    (1, 7),
    (-1, 7),
    (5, 5),
    (-5, 5),
    (7, 1),
    (-7, 1),
    (4, 6),
    (-4, 6),
    (6, 4),
    (-6, 4),
    (2, 7),
    (-2, 7),
    (7, 2),
    (-7, 2),
    (3, 7),
    (-3, 7),
    (7, 3),
    (-7, 3),
    (5, 6),
    (-5, 6),
    (6, 5),
    (-6, 5),
    (8, 0),
    (4, 7),
    (-4, 7),
    (7, 4),
    (-7, 4),
    (8, 1),
    (8, 2),
    (6, 6),
    (-6, 6),
    (8, 3),
    (5, 7),
    (-5, 7),
    (7, 5),
    (-7, 5),
    (8, 4),
    (6, 7),
    (-6, 7),
    (7, 6),
    (-7, 6),
    (8, 5),
    (7, 7),
    (-7, 7),
    (8, 6),
    (8, 7),
];

fn image_data(
    bits: &mut Bits<'_>,
    width: usize,
    height: usize,
    main: bool,
) -> ImageResult<Vec<u32>> {
    let count = bits.pixel_budget(width, height)?;
    let cache_bits = if bits.take(1)? != 0 {
        let value = bits.take(4)?;
        if !(1..=11).contains(&value) {
            return Err("WebP color-cache bits must be 1 through 11.".into());
        }
        value
    } else {
        0
    };
    let cache_size = if cache_bits != 0 { 1 << cache_bits } else { 0 };
    let mut cache = reserved(cache_size)?;
    cache.resize(cache_size, 0u32);
    let mut entropy = Vec::new();
    let mut entropy_bits = 0usize;
    let mut entropy_width = 0usize;
    let mut group_count = 1usize;
    if main && bits.take(1)? != 0 {
        entropy_bits = bits.take(3)? + 2;
        entropy_width = width.div_ceil(1 << entropy_bits);
        entropy = image_data(
            bits,
            entropy_width,
            height.div_ceil(1 << entropy_bits),
            false,
        )?;
        group_count = entropy
            .iter()
            .map(|p| ((p >> 8) & 65535) as usize)
            .max()
            .unwrap_or(0)
            + 1;
        if group_count > MAX_GROUPS {
            return Err("WebP entropy-group limit reached (512).".into());
        }
    }
    let mut groups = reserved(group_count)?;
    for _ in 0..group_count {
        (bits.check)()?;
        groups.push([
            prefix(bits, 280 + cache_size)?,
            prefix(bits, 256)?,
            prefix(bits, 256)?,
            prefix(bits, 256)?,
            prefix(bits, 40)?,
        ]);
    }
    let mut output = reserved(count)?;
    let append = |output: &mut Vec<u32>, cache: &mut Vec<u32>, color: u32| {
        output.push(color);
        if cache_bits != 0 {
            let index = (color.wrapping_mul(0x1e35a7bd) >> (32 - cache_bits)) as usize;
            cache[index] = color;
        }
    };
    while output.len() < count {
        bits.charge(1)?;
        let at = output.len();
        let group = if entropy.is_empty() {
            0
        } else {
            ((entropy
                [((at / width) >> entropy_bits) * entropy_width + ((at % width) >> entropy_bits)]
                >> 8)
                & 65535) as usize
        };
        let table = &groups[group];
        let symbol = table[0].read(bits)?;
        match symbol {
            0..=255 => {
                let red = table[1].read(bits)? as u8;
                let blue = table[2].read(bits)? as u8;
                let alpha = table[3].read(bits)? as u8;
                append(
                    &mut output,
                    &mut cache,
                    pixel(red, symbol as u8, blue, alpha),
                );
            }
            256..=279 => {
                let length = prefixed_number(bits, symbol - 256)?;
                let code = table[4].read(bits)?;
                let distance = prefixed_number(bits, code)?;
                let distance = if distance > 120 {
                    distance - 120
                } else {
                    let (x, y) = NEIGHBORS[distance - 1];
                    (x + y * width as i32).max(1) as usize
                };
                if distance > at || length > count - at {
                    return Err("WebP backward reference is outside the decoded image.".into());
                }
                bits.charge(length)?;
                for _ in 0..length {
                    let color = output[output.len() - distance];
                    append(&mut output, &mut cache, color);
                }
            }
            _ => {
                let index = symbol - 280;
                let color = *cache
                    .get(index)
                    .ok_or("WebP color-cache index is out of bounds.")?;
                append(&mut output, &mut cache, color);
            }
        }
    }
    Ok(output)
}

struct Transform {
    kind: usize,
    width: usize,
    tile_bits: usize,
    data: Vec<u32>,
}
fn channels(value: u32) -> [u8; 4] {
    [
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ]
}
fn assemble(c: [u8; 4]) -> u32 {
    u32::from_be_bytes(c)
}
fn add_channels(a: u32, b: u32) -> u32 {
    let a = channels(a);
    let b = channels(b);
    assemble(std::array::from_fn(|i| a[i].wrapping_add(b[i])))
}
fn average(a: u32, b: u32) -> u32 {
    let a = channels(a);
    let b = channels(b);
    assemble(std::array::from_fn(|i| {
        ((u16::from(a[i]) + u16::from(b[i])) / 2) as u8
    }))
}
fn predict(mode: usize, left: u32, top: u32, tl: u32, tr: u32) -> ImageResult<u32> {
    Ok(match mode {
        0 => 0xff000000,
        1 => left,
        2 => top,
        3 => tr,
        4 => tl,
        5 => average(average(left, tr), top),
        6 => average(left, tl),
        7 => average(left, top),
        8 => average(tl, top),
        9 => average(top, tr),
        10 => average(average(left, tl), average(top, tr)),
        11 => {
            let l = channels(left);
            let t = channels(top);
            let corner = channels(tl);
            let mut dl = 0i32;
            let mut dt = 0i32;
            for i in 0..4 {
                let p = i32::from(l[i]) + i32::from(t[i]) - i32::from(corner[i]);
                dl += (p - i32::from(l[i])).abs();
                dt += (p - i32::from(t[i])).abs();
            }
            if dl < dt {
                left
            } else {
                top
            }
        }
        12 => {
            let l = channels(left);
            let t = channels(top);
            let corner = channels(tl);
            assemble(std::array::from_fn(|i| {
                (i32::from(l[i]) + i32::from(t[i]) - i32::from(corner[i])).clamp(0, 255) as u8
            }))
        }
        13 => {
            let avg = channels(average(left, top));
            let corner = channels(tl);
            assemble(std::array::from_fn(|i| {
                let a = i32::from(avg[i]);
                (a + (a - i32::from(corner[i])) / 2).clamp(0, 255) as u8
            }))
        }
        _ => return Err("WebP predictor mode exceeds 13.".into()),
    })
}
fn apply_transform(
    transform: Transform,
    mut output: Vec<u32>,
    height: usize,
    bits: &mut Bits<'_>,
) -> ImageResult<Vec<u32>> {
    let width = transform.width;
    if transform.kind == 3 {
        let packed_width = width.div_ceil(1 << transform.tile_bits);
        if output.len() != packed_width * height {
            return Err("Invalid WebP packed-image dimensions.".into());
        }
        let count = dimensions(width, height)?;
        let mut expanded = reserved(count)?;
        let index_bits = 8 >> transform.tile_bits;
        let mask = (1 << index_bits) - 1;
        for y in 0..height {
            bits.charge(width)?;
            for x in 0..width {
                let packed =
                    ((output[y * packed_width + (x >> transform.tile_bits)] >> 8) & 255) as usize;
                let index =
                    (packed >> ((x & ((1 << transform.tile_bits) - 1)) * index_bits)) & mask;
                expanded.push(*transform.data.get(index).unwrap_or(&0));
            }
        }
        return Ok(expanded);
    }
    if output.len() != width * height {
        return Err("Invalid WebP transform dimensions.".into());
    }
    let tile_width = width.div_ceil(1 << transform.tile_bits);
    for y in 0..height {
        bits.charge(width * 4)?;
        for x in 0..width {
            let at = y * width + x;
            match transform.kind {
                0 => {
                    let predicted = if at == 0 {
                        0xff000000
                    } else if y == 0 {
                        output[at - 1]
                    } else if x == 0 {
                        output[at - width]
                    } else {
                        let mode = ((transform.data
                            [(y >> transform.tile_bits) * tile_width + (x >> transform.tile_bits)]
                            >> 8)
                            & 255) as usize;
                        predict(
                            mode,
                            output[at - 1],
                            output[at - width],
                            output[at - width - 1],
                            output[at - width + 1],
                        )?
                    };
                    output[at] = add_channels(output[at], predicted);
                }
                1 => {
                    let parameters = transform.data
                        [(y >> transform.tile_bits) * tile_width + (x >> transform.tile_bits)];
                    let [a, r, g, b] = channels(output[at]);
                    let delta = |parameter: u8, color: u8| {
                        (i32::from(parameter as i8) * i32::from(color as i8)) >> 5
                    };
                    let red = (i32::from(r) + delta(parameters as u8, g)) as u8;
                    let blue = (i32::from(b)
                        + delta((parameters >> 8) as u8, g)
                        + delta((parameters >> 16) as u8, red))
                        as u8;
                    output[at] = pixel(red, g, blue, a);
                }
                2 => {
                    let [a, r, g, b] = channels(output[at]);
                    output[at] = pixel(r.wrapping_add(g), g, b.wrapping_add(g), a);
                }
                _ => return Err("Unknown WebP inverse transform.".into()),
            }
        }
    }
    Ok(output)
}

fn lossless(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    if data.len() < 5 || data[0] != 0x2f {
        return Err("Invalid WebP lossless signature or header.".into());
    }
    let mut bits = Bits {
        data: &data[1..],
        at: 0,
        work: MAX_WORK,
        since_check: 0,
        decoded: 0,
        check,
    };
    let width = bits.take(14)? + 1;
    let height = bits.take(14)? + 1;
    dimensions(width, height)?;
    bits.take(1)?; // Alpha hint; actual decoded alpha values remain authoritative.
    if bits.take(3)? != 0 {
        return Err("Unsupported WebP lossless version.".into());
    }
    let mut coded_width = width;
    let mut transforms = Vec::new();
    let mut seen = 0usize;
    while bits.take(1)? != 0 {
        let kind = bits.take(2)?;
        if seen & (1 << kind) != 0 {
            return Err("Duplicate WebP transform.".into());
        }
        seen |= 1 << kind;
        let mut transform = Transform {
            kind,
            width: coded_width,
            tile_bits: 0,
            data: Vec::new(),
        };
        match kind {
            0 | 1 => {
                transform.tile_bits = bits.take(3)? + 2;
                transform.data = image_data(
                    &mut bits,
                    coded_width.div_ceil(1 << transform.tile_bits),
                    height.div_ceil(1 << transform.tile_bits),
                    false,
                )?;
                if kind == 0 && transform.data.iter().any(|p| ((p >> 8) & 255) > 13) {
                    return Err("WebP predictor mode exceeds 13.".into());
                }
            }
            2 => {}
            3 => {
                let colors = bits.take(8)? + 1;
                transform.data = image_data(&mut bits, colors, 1, false)?;
                for i in 1..colors {
                    transform.data[i] = add_channels(transform.data[i], transform.data[i - 1]);
                }
                transform.tile_bits = if colors <= 2 {
                    3
                } else if colors <= 4 {
                    2
                } else if colors <= 16 {
                    1
                } else {
                    0
                };
                coded_width = coded_width.div_ceil(1 << transform.tile_bits);
            }
            _ => unreachable!(),
        }
        transforms.push(transform);
    }
    let mut output = image_data(&mut bits, coded_width, height, true)?;
    // End-of-image is the exact decoded pixel count. At most seven unused bits
    // may remain in the final byte; unrelated trailing payload bytes are rejected.
    if bits.at.div_ceil(8) != bits.data.len() {
        return Err("Trailing bytes in WebP lossless payload.".into());
    }
    for transform in transforms.into_iter().rev() {
        output = apply_transform(transform, output, height, &mut bits)?;
    }
    check()?;
    Ok(Bitmap {
        width,
        height,
        pixels: output,
    })
}

pub(super) fn decode(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    let asset = decode_asset(data, check)?;
    match Arc::try_unwrap(asset.bitmap) {
        Ok(bitmap) => Ok(bitmap),
        Err(bitmap) => {
            let mut pixels = reserved(bitmap.pixels.len())?;
            pixels.extend_from_slice(&bitmap.pixels);
            Ok(Bitmap {
                width: bitmap.width,
                height: bitmap.height,
                pixels,
            })
        }
    }
}

fn read24(data: &[u8]) -> usize {
    usize::from(data[0]) | (usize::from(data[1]) << 8) | (usize::from(data[2]) << 16)
}

fn chunk_at<'a>(data: &'a [u8], at: &mut usize) -> ImageResult<(&'a [u8], &'a [u8])> {
    if data.len().saturating_sub(*at) < 8 {
        return Err("Truncated WebP chunk header.".into());
    }
    let kind = &data[*at..*at + 4];
    let length = le32(&data[*at + 4..]) as usize;
    let start = *at + 8;
    let end = start
        .checked_add(length)
        .filter(|&n| n <= data.len())
        .ok_or("Truncated WebP chunk.")?;
    if length & 1 != 0 && data.get(end) != Some(&0) {
        return Err("WebP RIFF padding byte is missing or nonzero.".into());
    }
    *at = end + (length & 1);
    Ok((kind, &data[start..end]))
}

struct Frame<'a> {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    delay: u32,
    replace: bool,
    dispose: bool,
    lossless: bool,
    payload: &'a [u8],
    alpha: Option<&'a [u8]>,
}

fn frame<'a>(
    data: &'a [u8],
    canvas: (usize, usize),
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<Frame<'a>> {
    if data.len() < 16 {
        return Err("Truncated WebP animation frame header.".into());
    }
    let left = read24(data) * 2;
    let top = read24(&data[3..]) * 2;
    let width = read24(&data[6..]) + 1;
    let height = read24(&data[9..]) + 1;
    dimensions(width, height)?;
    if left + width > canvas.0 || top + height > canvas.1 {
        return Err("WebP animation frame exceeds its canvas.".into());
    }
    let mut at = 16;
    let mut alpha = None;
    let mut image = None;
    while at < data.len() {
        check()?;
        // Each checked chunk advances at least eight bytes within this frame;
        // unknown chunks retain no storage, and ANMF nesting is rejected.
        let (kind, payload) = chunk_at(data, &mut at)?;
        match kind {
            b"ALPH" => {
                if alpha.is_some() || image.is_some() {
                    return Err("Invalid WebP frame alpha chunk order.".into());
                }
                alpha = Some(payload);
            }
            b"VP8 " | b"VP8L" => {
                if image.is_some() {
                    return Err("WebP animation frame has multiple image payloads.".into());
                }
                image = Some((kind == b"VP8L", payload));
            }
            b"VP8X" | b"ANIM" | b"ANMF" | b"ICCP" => {
                return Err("Invalid chunk inside WebP animation frame.".into())
            }
            _ => {}
        }
    }
    let (lossless, payload) = image.ok_or("WebP animation frame is missing an image payload.")?;
    if lossless && alpha.is_some() {
        return Err("WebP ALPH cannot accompany a lossless image.".into());
    }
    Ok(Frame {
        left,
        top,
        width,
        height,
        delay: read24(&data[12..]) as u32,
        replace: data[15] & 2 != 0,
        dispose: data[15] & 1 != 0,
        lossless,
        payload,
        alpha,
    })
}

pub(super) fn decode_asset(
    data: &[u8],
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<DecodedImage> {
    check()?;
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return Err("Invalid WebP RIFF header.".into());
    }
    if (le32(&data[4..]) as usize).checked_add(8) != Some(data.len()) {
        return Err("WebP RIFF size does not match the file.".into());
    }
    let mut at = 12usize;
    let mut image = None;
    let mut alpha = None;
    let mut canvas = None;
    let mut profile_seen = false;
    let mut animated = false;
    let mut animation = None;
    let mut frames = Vec::new();
    let mut exif_seen = false;
    let mut orientation = None;
    while at < data.len() {
        check()?;
        // The admitted input and checked chunk extents bound this scan. Actual
        // image/frame storage and decode work keep their separate allowances.
        let start = at;
        let (kind, payload) = chunk_at(data, &mut at)?;
        let length = payload.len();
        match kind {
            b"EXIF" => {
                // The container permits ignoring duplicate EXIF chunks after
                // the first. Malformed optional metadata stays non-fatal.
                if !exif_seen {
                    orientation = super::metadata::webp_orientation(payload);
                }
                exif_seen = true;
            }
            b"VP8X" => {
                if start != 12 || canvas.is_some() || length < 10 {
                    return Err("Invalid WebP extended header or chunk order.".into());
                }
                animated = payload[0] & 2 != 0;
                let width = read24(&payload[4..]) + 1;
                let height = read24(&payload[7..]) + 1;
                dimensions(width, height)?;
                canvas = Some((width, height));
            }
            b"VP8L" | b"VP8 " => {
                if animated || image.is_some() {
                    return Err("WebP contains more than one image payload.".into());
                }
                image = Some((kind == b"VP8L", payload));
            }
            b"ALPH" => {
                if animated || canvas.is_none() || image.is_some() || alpha.is_some() {
                    return Err("Invalid WebP alpha chunk order.".into());
                }
                alpha = Some(payload);
            }
            b"ANIM" => {
                if !animated {
                    continue;
                }
                if animation.is_some() || !frames.is_empty() || length < 6 {
                    return Err("Invalid WebP animation header or chunk order.".into());
                }
                animation = Some((le32(payload), le16(&payload[4..])));
            }
            b"ANMF" => {
                if !animated || animation.is_none() || frames.len() >= MAX_ANIMATION_FRAMES {
                    return Err("WebP animation frame limit or invalid chunk order.".into());
                }
                frames.push(frame(
                    payload,
                    canvas.ok_or("WebP animation canvas is missing.")?,
                    check,
                )?);
            }
            b"ICCP" => {
                if image.is_some() || alpha.is_some() || animation.is_some() || profile_seen {
                    return Err("Invalid WebP color-profile chunk order.".into());
                }
                profile_seen = true; // Metadata is inert; color-profile conversion is not implemented.
            }
            _ => {} // XMP and unknown metadata do not execute code or fetch anything.
        }
    }
    if animated {
        let (background, plays) = animation.ok_or("Animated WebP is missing its ANIM header.")?;
        let (width, height) = canvas.ok_or("WebP animation canvas is missing.")?;
        let decoded = compose(width, height, background, plays, &frames, check)?;
        return super::metadata::orient_asset(decoded, orientation.unwrap_or(1), check);
    }
    let (is_lossless, payload) = image.ok_or("WebP image chunk is missing.")?;
    if is_lossless && alpha.is_some() {
        return Err("WebP ALPH cannot accompany a lossless image.".into());
    }
    let mut bitmap = if is_lossless {
        lossless(payload, check)?
    } else {
        vp8::decode(payload, check)?
    };
    if canvas.is_some_and(|size| size != (bitmap.width, bitmap.height)) {
        return Err("WebP image dimensions do not match its canvas.".into());
    }
    if let Some(payload) = alpha {
        apply_alpha(&mut bitmap, payload, check)?;
    }
    super::metadata::orient_asset(DecodedImage::still(bitmap), orientation.unwrap_or(1), check)
}

// Composites straight alpha in encoded sample space. ICC conversion and
// linear-light compositing are not implemented by the image pipeline yet.
fn source_over(source: u32, target: u32) -> u32 {
    let sa = source >> 24;
    let ta = target >> 24;
    if sa == 255 {
        return source;
    }
    if sa == 0 {
        return if ta == 0 { 0 } else { target };
    }
    let inverse = 255 - sa;
    let numerator = sa * 255 + ta * inverse;
    let component = |shift: u32| -> u32 {
        ((((source >> shift) & 255) * sa * 255 + ((target >> shift) & 255) * ta * inverse)
            + numerator / 2)
            / numerator
    };
    (((numerator + 127) / 255) << 24) | (component(16) << 16) | (component(8) << 8) | component(0)
}

fn compose(
    width: usize,
    height: usize,
    background: u32,
    plays: u16,
    frames: &[Frame<'_>],
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<DecodedImage> {
    if frames.is_empty() {
        return Err("WebP animation contains no frames.".into());
    }
    let count = dimensions(width, height)?;
    // At most 128 Mi pixels are decoded/composited, even for a static fallback.
    frames.iter().try_fold(0usize, |sum, frame| {
        sum.checked_add(frame.width * frame.height + count)
            .filter(|&n| n <= 128 * 1024 * 1024)
            .ok_or("WebP animation exceeds its decode work limit.")
    })?;
    let retain = count
        .checked_mul(frames.len())
        .is_some_and(|n| n <= MAX_ANIMATION_PIXELS);
    let mut canvas = reserved(count)?;
    canvas.resize(count, background);
    let mut snapshots = reserved(if retain { frames.len() } else { 1 })?;
    for (index, frame) in frames.iter().enumerate() {
        check()?;
        let mut bitmap = if frame.lossless {
            lossless(frame.payload, check)?
        } else {
            vp8::decode(frame.payload, check)?
        };
        if bitmap.width != frame.width || bitmap.height != frame.height {
            return Err("WebP frame dimensions do not match its image payload.".into());
        }
        if let Some(alpha) = frame.alpha {
            apply_alpha(&mut bitmap, alpha, check)?;
        }
        for y in 0..frame.height {
            check()?;
            for x in 0..frame.width {
                let at = (frame.top + y) * width + frame.left + x;
                let color = bitmap.pixels[y * frame.width + x];
                canvas[at] = if frame.replace {
                    color
                } else {
                    source_over(color, canvas[at])
                };
            }
        }
        if retain || index == 0 {
            let mut pixels = reserved(count)?;
            pixels.extend_from_slice(&canvas);
            snapshots.push(ImageFrame {
                bitmap: Arc::new(Bitmap {
                    width,
                    height,
                    pixels,
                }),
                delay_ms: frame.delay,
            });
        }
        if frame.dispose {
            for y in 0..frame.height {
                check()?;
                let at = (frame.top + y) * width + frame.left;
                canvas[at..at + frame.width].fill(background);
            }
        }
    }
    let bitmap = Arc::clone(&snapshots[0].bitmap);
    if !retain {
        return Ok(DecodedImage {
            bitmap,
            animation: None,
            notice: Some(
                "WebP animation exceeds the retained pixel limit; showing its first frame.".into(),
            ),
        });
    }
    Ok(DecodedImage {
        bitmap,
        animation: Some(Arc::new(Animation {
            frames: snapshots,
            plays: if plays == 0 {
                None
            } else {
                Some(u32::from(plays))
            },
        })),
        notice: None,
    })
}

fn apply_alpha(
    bitmap: &mut Bitmap,
    data: &[u8],
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<()> {
    let flags = *data.first().ok_or("Truncated WebP alpha header.")?;
    // Reserved high bits are ignored as required by the container specification.
    if (flags >> 4) & 3 > 1 || flags & 3 > 1 {
        return Err("Invalid or unsupported WebP alpha encoding.".into());
    }
    let count = dimensions(bitmap.width, bitmap.height)?;
    let mut alpha = reserved(count)?;
    if flags & 3 == 0 {
        if data.len() != count + 1 {
            return Err("WebP raw alpha size does not match its image.".into());
        }
        alpha.extend_from_slice(&data[1..]);
    } else {
        let mut encoded = reserved(data.len() + 4)?;
        encoded.push(0x2f);
        let packed = ((bitmap.width - 1) as u32) | (((bitmap.height - 1) as u32) << 14) | (1 << 28);
        encoded.extend_from_slice(&packed.to_le_bytes());
        encoded.extend_from_slice(&data[1..]);
        let decoded = lossless(&encoded, check)?;
        alpha.extend(decoded.pixels.iter().map(|p| (p >> 8) as u8));
    }
    let filter = (flags >> 2) & 3;
    for y in 0..bitmap.height {
        check()?;
        for x in 0..bitmap.width {
            let at = y * bitmap.width + x;
            let left = if x != 0 { i32::from(alpha[at - 1]) } else { 0 };
            let above = if y != 0 {
                i32::from(alpha[at - bitmap.width])
            } else {
                0
            };
            let prediction = match filter {
                0 => 0,
                1 => {
                    if x != 0 {
                        left
                    } else {
                        above
                    }
                }
                2 => {
                    if y != 0 {
                        above
                    } else {
                        left
                    }
                }
                _ => {
                    if y == 0 {
                        left
                    } else if x == 0 {
                        above
                    } else {
                        (left + above - i32::from(alpha[at - bitmap.width - 1])).clamp(0, 255)
                    }
                }
            };
            alpha[at] = alpha[at].wrapping_add(prediction as u8);
            bitmap.pixels[at] = (bitmap.pixels[at] & 0x00ff_ffff) | (u32::from(alpha[at]) << 24);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Writer {
        data: Vec<u8>,
        at: usize,
    }
    impl Writer {
        fn new() -> Self {
            Self {
                data: Vec::new(),
                at: 0,
            }
        }
        fn put(&mut self, value: usize, count: usize) {
            for shift in 0..count {
                if self.at / 8 == self.data.len() {
                    self.data.push(0);
                }
                self.data[self.at / 8] |= (((value >> shift) & 1) as u8) << (self.at % 8);
                self.at += 1;
            }
        }
        fn leaf(&mut self, symbol: u8) {
            self.put(1, 1);
            self.put(0, 1);
            self.put(1, 1);
            self.put(symbol as usize, 8);
        }
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = kind.to_vec();
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            out.push(0);
        }
        out
    }
    fn riff(chunks: &[u8]) -> Vec<u8> {
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&((chunks.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend_from_slice(chunks);
        out
    }
    fn solid(width: usize, height: usize, color: u32) -> Vec<u8> {
        let mut w = Writer::new();
        w.put(width - 1, 14);
        w.put(height - 1, 14);
        w.put(1, 1);
        w.put(0, 3);
        w.put(0, 1); // No transforms.
        w.put(0, 1);
        w.put(0, 1); // No cache or entropy image.
        for component in [
            (color >> 8) as u8,
            (color >> 16) as u8,
            color as u8,
            (color >> 24) as u8,
            0,
        ] {
            w.leaf(component);
        }
        let mut out = vec![0x2f];
        out.extend_from_slice(&w.data);
        riff(&chunk(b"VP8L", &out))
    }
    fn bit_reader<'a>(data: &'a [u8], check: &'a dyn Fn() -> ImageResult<()>) -> Bits<'a> {
        Bits {
            data,
            at: 0,
            work: MAX_WORK,
            since_check: 0,
            decoded: 0,
            check,
        }
    }
    #[test]
    fn webp_dispatcher_preserves_rgba_and_ignores_alpha_hint() {
        let data = solid(13, 7, 0x7bff002a);
        let bitmap = super::super::decode(&data).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (13, 7));
        assert_eq!(bitmap.pixels, vec![0x7bff002a; 91]);
        let mut hint = data;
        hint[24] &= !0x10; // Alpha-used bit is advisory.
        assert_eq!(super::super::decode(&hint).unwrap().pixels, bitmap.pixels);
    }
    #[test]
    fn webp_chunk_bounds_versions_and_unsupported_codecs() {
        let data = solid(3, 2, 0xffff0022);
        for length in 0..data.len() {
            assert!(
                super::super::decode(&data[..length]).is_err(),
                "truncation at {length}"
            );
        }
        let mut bad = data.clone();
        bad[24] |= 0x20;
        assert!(decode(&bad, &|| Ok(())).unwrap_err().contains("version"));
        let mut bad = data.clone();
        bad.push(0);
        assert!(decode(&bad, &|| Ok(())).unwrap_err().contains("RIFF size"));
        let lossy = riff(&chunk(b"VP8 ", b"x"));
        assert!(decode(&lossy, &|| Ok(())).unwrap_err().contains("Lossy"));
        let animated = riff(&chunk(b"VP8X", &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
        assert!(decode(&animated, &|| Ok(()))
            .unwrap_err()
            .contains("Animated"));
        let mut extended = chunk(b"VP8X", &[0x1c, 0, 0, 0, 2, 0, 0, 1, 0, 0]);
        extended.extend_from_slice(&data[12..]);
        extended.extend_from_slice(&chunk(b"EXIF", b"inert"));
        extended.extend_from_slice(&chunk(b"XMP ", b"also inert"));
        assert_eq!(
            decode(&riff(&extended), &|| Ok(())).unwrap().pixels,
            vec![0xffff0022; 6]
        );
        extended[12] = 3;
        assert!(decode(&riff(&extended), &|| Ok(()))
            .unwrap_err()
            .contains("canvas"));
    }
    #[test]
    fn webp_prefix_trees_enforce_full_codes_and_decode_bits_in_wire_order() {
        let check = || Ok(());
        let mut bits = bit_reader(&[0xd8, 0xd8], &check);
        let table = Prefix::build(&[2, 2, 2, 2], &mut bits).unwrap();
        for expected in [0, 1, 2, 3, 0, 1, 2, 3] {
            assert_eq!(table.read(&mut bits).unwrap(), expected);
        }
        assert!(table.read(&mut bits).is_err());
        for lengths in [
            &[1, 1, 1][..],
            &[2, 2][..],
            &[2][..],
            &[0, 0][..],
            &[16][..],
        ] {
            assert!(Prefix::build(lengths, &mut bits).is_err());
        }
        let table = Prefix::build(&[0, 1, 0], &mut bits).unwrap();
        assert_eq!(table.read(&mut bits).unwrap(), 1);
        assert_eq!(bits.at, 16); // A single-symbol tree consumes no data bits.
    }
    #[test]
    fn webp_transforms_use_signed_coefficients_and_channel_wrap() {
        assert_eq!(add_channels(0xfffe0080, 0x0203ff90), 0x0101ff10);
        assert_eq!(average(0x01030507, 0x02040608), 0x01030507);
        let check = || Ok(());
        let mut bits = bit_reader(&[], &check);
        let transform = Transform {
            kind: 1,
            width: 1,
            tile_bits: 2,
            data: vec![0xffff80ff],
        };
        // Signed green=-56; G->R=-1, G->B=-128, R->B=-1.
        // R=20+1=21; B=30+224-1=253 (arithmetic right shift).
        assert_eq!(
            apply_transform(transform, vec![pixel(20, 200, 30, 230)], 1, &mut bits).unwrap(),
            vec![pixel(21, 200, 253, 230)]
        );
        let transform = Transform {
            kind: 3,
            width: 5,
            tile_bits: 1,
            data: vec![0xff112233, 0xff445566, 0xff778899],
        };
        assert_eq!(
            apply_transform(
                transform,
                vec![0xff001000, 0xff003200, 0xff000100],
                1,
                &mut bits
            )
            .unwrap(),
            vec![0xff112233, 0xff445566, 0xff778899, 0, 0xff445566]
        );
    }
    #[test]
    fn webp_predictor_borders_and_right_edge_use_reconstructed_pixels() {
        let check = || Ok(());
        let mut bits = bit_reader(&[], &check);
        let residual = vec![pixel(1, 2, 3, 0); 6];
        let transform = Transform {
            kind: 0,
            width: 3,
            tile_bits: 2,
            data: vec![0xff000300],
        };
        let output = apply_transform(transform, residual, 2, &mut bits).unwrap();
        assert_eq!(
            output,
            vec![
                pixel(1, 2, 3, 255),
                pixel(2, 4, 6, 255),
                pixel(3, 6, 9, 255),
                pixel(2, 4, 6, 255),
                pixel(4, 8, 12, 255),
                pixel(3, 6, 9, 255)
            ]
        );
        assert!(predict(14, 0, 0, 0, 0).is_err());
        assert_eq!(
            predict(12, 0xffffffff, 0xffffffff, 0, 0).unwrap(),
            0xffffffff
        );
        assert_eq!(predict(13, 0, 0, 0xffffffff, 0).unwrap(), 0);
    }
    #[test]
    fn webp_cancel_and_dimensions_are_checked_for_constant_images() {
        let source = solid(512, 512, 0xff223344);
        let calls = Cell::new(0);
        let error = decode(&source, &|| {
            calls.set(calls.get() + 1);
            if calls.get() >= 6 {
                Err("cancelled while expanding WebP".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "cancelled while expanding WebP");
        assert!(decode(&solid(16384, 16384, 0), &|| Ok(()))
            .unwrap_err()
            .contains("4-megapixel"));
    }

    fn animation_frame(
        width: usize,
        height: usize,
        left: usize,
        top: usize,
        delay: usize,
        flags: u8,
        color: u32,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        for value in [left / 2, top / 2, width - 1, height - 1, delay] {
            data.extend_from_slice(&(value as u32).to_le_bytes()[..3]);
        }
        data.push(flags);
        data.extend_from_slice(&solid(width, height, color)[12..]);
        chunk(b"ANMF", &data)
    }

    fn animation_file(width: usize, height: usize, plays: u16, frames: &[Vec<u8>]) -> Vec<u8> {
        let mut header = vec![0x12, 0, 0, 0];
        header.extend_from_slice(&((width - 1) as u32).to_le_bytes()[..3]);
        header.extend_from_slice(&((height - 1) as u32).to_le_bytes()[..3]);
        let mut body = chunk(b"VP8X", &header);
        let mut parameters = vec![0; 4];
        parameters.extend_from_slice(&plays.to_le_bytes());
        body.extend_from_slice(&chunk(b"ANIM", &parameters));
        for frame in frames {
            body.extend_from_slice(frame);
        }
        riff(&body)
    }

    fn container_with_records(inner: bool, count: usize, tail: &[u8]) -> Vec<u8> {
        let records = chunk(b"junk", &[]).repeat(count);
        if inner {
            let first = animation_frame(1, 1, 0, 0, 70, 2, 0xff223344);
            let mut payload = first[8..].to_vec();
            payload.extend_from_slice(&records);
            payload.extend_from_slice(tail);
            animation_file(
                1,
                1,
                3,
                &[
                    chunk(b"ANMF", &payload),
                    animation_frame(1, 1, 0, 0, 110, 2, 0xff556677),
                ],
            )
        } else {
            let mut body = chunk(b"VP8X", &[0; 10]);
            body.extend_from_slice(&solid(1, 1, 0xff223344)[12..]);
            body.extend_from_slice(&records);
            body.extend_from_slice(tail);
            riff(&body)
        }
    }

    #[test]
    fn source_bounded_webp_chunks_preserve_static_pixels() {
        let data = container_with_records(false, 16_385, &[]);
        let bitmap = crate::image::decode(&data).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (1, 1));
        assert_eq!(bitmap.pixels, [0xff223344]);
        let asset = crate::image::decode_asset_checked(&data, &|| Ok(())).unwrap();
        assert_eq!(asset.bitmap.pixels, [0xff223344]);
        assert!(asset.animation.is_none());
    }

    #[test]
    fn source_bounded_webp_frame_chunks_preserve_animation_pixels_timing_and_loops() {
        let data = container_with_records(true, 16_385, &[]);
        assert_eq!(crate::image::decode(&data).unwrap().pixels, [0xff223344]);
        let asset = crate::image::decode_asset_checked(&data, &|| Ok(())).unwrap();
        assert_eq!((asset.bitmap.width, asset.bitmap.height), (1, 1));
        assert_eq!(asset.bitmap.pixels, [0xff223344]);
        let animation = asset.animation.unwrap();
        assert_eq!(animation.plays, Some(3));
        assert_eq!(animation.frames.len(), 2);
        assert_eq!(animation.frames[0].bitmap.pixels, [0xff223344]);
        assert_eq!(animation.frames[1].bitmap.pixels, [0xff556677]);
        assert_eq!(animation.frames[0].delay_ms, 70);
        assert_eq!(animation.frames[1].delay_ms, 110);
    }

    #[test]
    fn source_bounded_webp_chunks_preserve_late_extent_padding_and_order_checks() {
        let mut bad_extent = b"junk".to_vec();
        bad_extent.extend_from_slice(&u32::MAX.to_le_bytes());
        let mut bad_padding = chunk(b"junk", &[0]);
        *bad_padding.last_mut().unwrap() = 1;
        let cases = [
            (vec![0; 7], "Truncated WebP chunk header"),
            (bad_extent, "Truncated WebP chunk."),
            (bad_padding, "padding byte is missing or nonzero"),
            (solid(1, 1, 0xffabcdef)[12..].to_vec(), "image payload"),
            (chunk(b"ALPH", &[0]), "chunk order"),
        ];
        for inner in [false, true] {
            for (tail, expected) in &cases {
                let data = container_with_records(inner, 16_385, tail);
                let error = decode_asset(&data, &|| Ok(())).unwrap_err();
                assert!(error.contains(expected), "inner={inner}: {error}");
            }
        }
        let nested = container_with_records(true, 16_385, &chunk(b"ANMF", &[]));
        assert!(decode_asset(&nested, &|| Ok(()))
            .unwrap_err()
            .contains("Invalid chunk inside WebP animation frame"));
    }

    #[test]
    fn source_bounded_webp_chunk_scans_remain_cancelable_past_the_old_cutoff() {
        for inner in [false, true] {
            let data = container_with_records(inner, 20_000, &[]);
            let calls = Cell::new(0usize);
            let error = decode_asset(&data, &|| {
                calls.set(calls.get() + 1);
                if calls.get() >= 18_000 {
                    Err("cancelled during long WebP chunk scan".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error, "cancelled during long WebP chunk scan", "inner={inner}");
        }
    }

    #[test]
    fn webp_original_lossy_fixture_alpha_and_cancel() {
        // Independently encoded from this project's own single gray pixel.
        let bytes = [
            82, 73, 70, 70, 36, 0, 0, 0, 87, 69, 66, 80, 86, 80, 56, 32, 24, 0, 0, 0, 80, 1, 0,
            157, 1, 42, 1, 0, 1, 0, 2, 0, 52, 37, 164, 0, 4, 116, 0, 0, 254, 0, 0, 0,
        ];
        let decoded = decode(&bytes, &|| Ok(())).unwrap();
        assert_eq!(decoded.pixels, vec![0xff7f7f7f]);
        let mut body = chunk(b"VP8X", &[16, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        body.extend_from_slice(&chunk(b"ALPH", &[0, 61]));
        body.extend_from_slice(&bytes[12..]);
        assert_eq!(
            decode(&riff(&body), &|| Ok(())).unwrap().pixels,
            vec![0x3d7f7f7f]
        );
        assert_eq!(
            decode(&bytes, &|| Err("cancelled VP8".into())).unwrap_err(),
            "cancelled VP8"
        );
        for cut in 0..10 {
            assert!(decode(&riff(&chunk(b"VP8 ", &bytes[20..20 + cut])), &|| Ok(())).is_err());
        }
    }

    #[test]
    fn webp_animation_composites_blends_disposal_delays_and_loops() {
        let frames = [
            animation_frame(4, 4, 0, 0, 30, 2, 0xffff0000),
            animation_frame(2, 2, 2, 0, 0, 1, 0x800000ff),
            animation_frame(2, 2, 0, 2, 70, 2, 0xff00ff00),
        ];
        let asset = decode_asset(&animation_file(4, 4, 2, &frames), &|| Ok(())).unwrap();
        let animation = asset.animation.unwrap();
        assert_eq!(animation.plays, Some(2));
        assert_eq!(
            animation
                .frames
                .iter()
                .map(|frame| frame.delay_ms)
                .collect::<Vec<_>>(),
            vec![30, 0, 70]
        );
        assert!(Arc::ptr_eq(&asset.bitmap, &animation.frames[0].bitmap));
        assert_eq!(animation.frames[0].bitmap.pixels, vec![0xffff0000; 16]);
        assert_eq!(animation.frames[1].bitmap.pixels[2], 0xff7f0080);
        assert_eq!(animation.frames[2].bitmap.pixels[2], 0);
        assert_eq!(animation.frames[2].bitmap.pixels[8], 0xff00ff00);
        assert_eq!(source_over(0x80ff0000, 0x4000ff00), 0xa0cc3300);
        assert_eq!(source_over(0x00ffffff, 0x10203040), 0x10203040);
        let infinite = decode_asset(&animation_file(4, 4, 0, &frames), &|| Ok(())).unwrap();
        assert_eq!(infinite.animation.unwrap().plays, None);
        let invalid = animation_file(4, 4, 1, &[animation_frame(4, 4, 2, 0, 20, 0, 0)]);
        assert!(decode_asset(&invalid, &|| Ok(()))
            .unwrap_err()
            .contains("canvas"));
    }

    #[test]
    fn webp_animation_fallback_still_validates_later_frames() {
        let frames = vec![animation_frame(1, 1, 0, 0, 20, 2, 0xffffffff); 5];
        let data = animation_file(4096, 1024, 1, &frames);
        let asset = decode_asset(&data, &|| Ok(())).unwrap();
        assert!(asset.animation.is_none());
        assert!(asset.notice.is_some());
        assert_eq!((asset.bitmap.width, asset.bitmap.height), (4096, 1024));
        assert_eq!(asset.bitmap.pixels[0], 0xffffffff);
        assert_eq!(asset.bitmap.pixels[1], 0);
        let mut damaged = frames;
        // The final frame now advertises width2 but still carries a 1x1 image.
        damaged[4][14] = 1;
        let error = decode_asset(&animation_file(4096, 1024, 1, &damaged), &|| Ok(())).unwrap_err();
        assert!(error.contains("dimensions"));
        let too_many = vec![animation_frame(1, 1, 0, 0, 20, 2, 0); 257];
        assert!(
            decode_asset(&animation_file(1, 1, 1, &too_many), &|| Ok(()))
                .unwrap_err()
                .contains("limit")
        );
        let excessive = vec![animation_frame(1, 1, 0, 0, 20, 2, 0); 33];
        assert!(
            decode_asset(&animation_file(4096, 1024, 1, &excessive), &|| Ok(()))
                .unwrap_err()
                .contains("work limit")
        );
    }
}
