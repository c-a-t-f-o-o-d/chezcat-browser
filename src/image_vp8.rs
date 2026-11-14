//! Original VP8 key-frame image decoder, following RFC 6386 sections 7–15.
//! Only the normative numeric tables are shared with the format specification.
//! No video reference buffers, external codecs, platform decoding or unsafe code.

use super::super::{dimensions_with_limit, le16, pixel, reserved, Bitmap, ImageResult};
use std::cell::Cell;

#[path = "image_vp8_tables.rs"]
mod tables;
use tables::{AC_QUANT, BMODE_PROBS, COEFF_DEFAULTS, COEFF_UPDATES, DC_QUANT};

const MAX_BOOLEAN_WORK: usize = 128 * 1024 * 1024;
const ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];
const BANDS: [usize; 16] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7];
const CATEGORIES: [&[u8]; 6] = [
    &[159],
    &[165, 145],
    &[173, 148, 140],
    &[176, 155, 140, 135],
    &[180, 157, 141, 134, 130],
    &[254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129],
];

struct Work<'a> {
    remaining: Cell<usize>,
    check: &'a dyn Fn() -> ImageResult<()>,
}
impl Work<'_> {
    fn step(&self) -> ImageResult<()> {
        let next = self
            .remaining
            .get()
            .checked_sub(1)
            .ok_or("VP8 boolean decoding exceeds its work limit.")?;
        self.remaining.set(next);
        if next & 4095 == 0 {
            (self.check)()?;
        }
        Ok(())
    }
}

/// A bit-oriented range decoder. `value` is the next eight fractional bits;
/// no read can cross into an adjacent partition or silently synthesize bytes.
struct Bool<'a> {
    data: &'a [u8],
    at: usize,
    value: u32,
    range: u32,
    work: &'a Work<'a>,
}
impl<'a> Bool<'a> {
    fn new(data: &'a [u8], work: &'a Work<'a>) -> Self {
        Self {
            data,
            at: 8,
            value: data.first().copied().unwrap_or(0).into(),
            range: 255,
            work,
        }
    }
    fn bit(&mut self, probability: u8) -> ImageResult<bool> {
        self.work.step()?;
        if self.data.is_empty() {
            return Err("Truncated VP8 boolean partition.".into());
        }
        let split = 1 + (((self.range - 1) * u32::from(probability)) >> 8);
        let one = self.value >= split;
        if one {
            self.value -= split;
            self.range -= split;
        } else {
            self.range = split;
        }
        while self.range < 128 {
            let byte = *self
                .data
                .get(self.at / 8)
                .ok_or("Truncated VP8 boolean partition.")?;
            self.value = (self.value << 1) | u32::from((byte >> (7 - self.at % 8)) & 1);
            self.at += 1;
            self.range <<= 1;
        }
        Ok(one)
    }
    fn literal(&mut self, count: usize) -> ImageResult<i32> {
        let mut value = 0;
        for _ in 0..count {
            value = value * 2 + i32::from(self.bit(128)?);
        }
        Ok(value)
    }
    fn signed(&mut self, count: usize) -> ImageResult<i32> {
        let value = self.literal(count)?;
        Ok(if self.bit(128)? { -value } else { value })
    }
    fn optional_signed(&mut self, count: usize) -> ImageResult<i32> {
        if self.bit(128)? {
            self.signed(count)
        } else {
            Ok(0)
        }
    }
    fn tree(&mut self, tree: &[i8], probabilities: &[u8]) -> ImageResult<u8> {
        let mut at = 0usize;
        loop {
            let branch = usize::from(self.bit(probabilities[at / 2])?);
            let next = tree[at + branch];
            if next <= 0 {
                return Ok((-next) as u8);
            }
            at = next as usize;
        }
    }
}

struct Header {
    segments: bool,
    segment_map: bool,
    absolute: bool,
    quant: [i32; 4],
    filter: [i32; 4],
    segment_probs: [u8; 3],
    simple: bool,
    filter_level: i32,
    sharpness: i32,
    filter_adjust: bool,
    reference_delta: [i32; 4],
    mode_delta: [i32; 4],
    partitions: usize,
    base_quant: i32,
    quant_delta: [i32; 5],
    probabilities: [u8; 1056],
    skip_probability: Option<u8>,
}
impl Header {
    fn read(bits: &mut Bool<'_>) -> ImageResult<Self> {
        if bits.bit(128)? {
            return Err("VP8 reserved color space is unsupported.".into());
        }
        bits.bit(128)?; // Clamping hint; output is clamped in either case.
        let mut h = Self {
            segments: bits.bit(128)?,
            segment_map: false,
            absolute: false,
            quant: [0; 4],
            filter: [0; 4],
            segment_probs: [255; 3],
            simple: false,
            filter_level: 0,
            sharpness: 0,
            filter_adjust: false,
            reference_delta: [0; 4],
            mode_delta: [0; 4],
            partitions: 1,
            base_quant: 0,
            quant_delta: [0; 5],
            probabilities: COEFF_DEFAULTS,
            skip_probability: None,
        };
        if h.segments {
            h.segment_map = bits.bit(128)?;
            if bits.bit(128)? {
                // Annex A corrects the reversed prose labels in RFC section 9.3:
                // the transmitted bit is 0 for deltas, 1 for absolute values.
                h.absolute = bits.bit(128)?;
                for q in &mut h.quant {
                    *q = bits.optional_signed(7)?;
                }
                for level in &mut h.filter {
                    *level = bits.optional_signed(6)?;
                }
            }
            if h.segment_map {
                for p in &mut h.segment_probs {
                    if bits.bit(128)? {
                        *p = bits.literal(8)? as u8;
                    }
                }
            }
        }
        h.simple = bits.bit(128)?;
        h.filter_level = bits.literal(6)?;
        h.sharpness = bits.literal(3)?;
        h.filter_adjust = bits.bit(128)?;
        if h.filter_adjust && bits.bit(128)? {
            for d in h.reference_delta.iter_mut().chain(&mut h.mode_delta) {
                if bits.bit(128)? {
                    *d = bits.signed(6)?;
                }
            }
        }
        h.partitions = 1 << bits.literal(2)?;
        h.base_quant = bits.literal(7)?;
        for d in &mut h.quant_delta {
            *d = bits.optional_signed(4)?;
        }
        bits.bit(128)?; // Future probability refresh has no effect on a still image.
        for (p, update) in h.probabilities.iter_mut().zip(COEFF_UPDATES) {
            if bits.bit(update)? {
                *p = bits.literal(8)? as u8;
            }
        }
        if bits.bit(128)? {
            h.skip_probability = Some(bits.literal(8)? as u8);
        }
        Ok(h)
    }
    fn quantizers(&self, segment: usize) -> [[i32; 2]; 3] {
        let base = if !self.segments {
            self.base_quant
        } else if self.absolute {
            self.quant[segment]
        } else {
            self.base_quant + self.quant[segment]
        };
        let dc = |delta: i32| DC_QUANT[(base + delta).clamp(0, 127) as usize];
        let ac = |delta: i32| AC_QUANT[(base + delta).clamp(0, 127) as usize];
        let [ydc, y2dc, y2ac, uvdc, uvac] = self.quant_delta;
        [
            [dc(ydc), ac(0)],
            [dc(y2dc) * 2, (ac(y2ac) * 155 / 100).max(8)],
            [dc(uvdc).min(132), ac(uvac)],
        ]
    }
    fn level(&self, segment: usize, subblocks: bool) -> i32 {
        let mut level = if !self.segments {
            self.filter_level
        } else if self.absolute {
            self.filter[segment]
        } else {
            self.filter_level + self.filter[segment]
        };
        if self.filter_adjust {
            level += self.reference_delta[0];
            if subblocks {
                level += self.mode_delta[0];
            }
        }
        level.clamp(0, 63)
    }
}

#[derive(Clone, Copy, Default)]
struct Macroblock {
    segment: usize,
    skip: bool,
    luma: u8,
    chroma: u8,
    modes: [u8; 16],
    residual: bool,
}

fn mode_record(
    bits: &mut Bool<'_>,
    header: &Header,
    above: &mut [u8],
    left: &mut [u8; 4],
) -> ImageResult<Macroblock> {
    let mut block = Macroblock::default();
    if header.segment_map {
        block.segment = bits.tree(&[2, 4, 0, -1, -2, -3], &header.segment_probs)? as usize;
    }
    if let Some(p) = header.skip_probability {
        block.skip = bits.bit(p)?;
    }
    block.luma = bits.tree(&[-4, 2, 4, 6, 0, -1, -2, -3], &[145, 156, 163, 128])?;
    if block.luma == 4 {
        for (y, left_mode) in left.iter_mut().enumerate() {
            for (x, above_mode) in above.iter_mut().enumerate() {
                let p = (usize::from(*above_mode) * 10 + usize::from(*left_mode)) * 9;
                let mode = bits.tree(
                    &[
                        0, 2, -1, 4, -2, 6, 8, 12, -3, 10, -5, -6, -4, 14, -7, 16, -8, -9,
                    ],
                    &BMODE_PROBS[p..p + 9],
                )?;
                block.modes[y * 4 + x] = mode;
                *above_mode = mode;
                *left_mode = mode;
            }
        }
    } else {
        let mode = [0, 2, 3, 1][block.luma as usize];
        above.fill(mode);
        left.fill(mode);
        block.modes.fill(mode);
    }
    block.chroma = bits.tree(&[0, 2, -1, 4, -2, -3], &[142, 114, 183])?;
    Ok(block)
}

fn coefficients(
    bits: &mut Bool<'_>,
    probabilities: &[u8; 1056],
    kind: usize,
    mut context: usize,
    quant: [i32; 2],
) -> ImageResult<([i32; 16], bool)> {
    let mut out = [0; 16];
    let mut previous_zero = false;
    let mut nonzero = false;
    for n in usize::from(kind == 0)..16 {
        let at = ((kind * 8 + BANDS[n]) * 3 + context) * 11;
        let p = &probabilities[at..at + 11];
        if !previous_zero && !bits.bit(p[0])? {
            break;
        }
        if !bits.bit(p[1])? {
            previous_zero = true;
            context = 0;
            continue;
        }
        let magnitude = if !bits.bit(p[2])? {
            1
        } else if !bits.bit(p[3])? {
            if !bits.bit(p[4])? {
                2
            } else {
                3 + i32::from(bits.bit(p[5])?)
            }
        } else {
            let category = if !bits.bit(p[6])? {
                usize::from(bits.bit(p[7])?)
            } else if !bits.bit(p[8])? {
                2 + usize::from(bits.bit(p[9])?)
            } else {
                4 + usize::from(bits.bit(p[10])?)
            };
            let mut extra = 0;
            for &probability in CATEGORIES[category] {
                extra = extra * 2 + i32::from(bits.bit(probability)?);
            }
            [5, 7, 11, 19, 35, 67][category] + extra
        };
        let sign = if bits.bit(128)? { -1 } else { 1 };
        out[ZIGZAG[n]] = (sign * magnitude * quant[usize::from(n != 0)]) as i16 as i32;
        previous_zero = false;
        nonzero = true;
        context = usize::from(magnitude != 1) + 1;
    }
    Ok((out, nonzero))
}

fn transform_line(a: [i32; 4], walsh: bool) -> [i32; 4] {
    if walsh {
        [
            a[0] + a[1] + a[2] + a[3],
            a[0] + a[1] - a[2] - a[3],
            a[0] - a[1] - a[2] + a[3],
            a[0] - a[1] + a[2] - a[3],
        ]
    } else {
        let even = [a[0] + a[2], a[0] - a[2]];
        let odd = [
            ((a[1] * 35468) >> 16) - a[3] - ((a[3] * 20091) >> 16),
            a[1] + ((a[1] * 20091) >> 16) + ((a[3] * 35468) >> 16),
        ];
        [
            even[0] + odd[1],
            even[1] + odd[0],
            even[1] - odd[0],
            even[0] - odd[1],
        ]
    }
}

fn inverse(input: [i32; 16], walsh: bool) -> [i32; 16] {
    let mut temporary = [0; 16];
    for x in 0..4 {
        let column = transform_line([input[x], input[4 + x], input[8 + x], input[12 + x]], walsh);
        for y in 0..4 {
            temporary[y * 4 + x] = column[y] as i16 as i32;
        }
    }
    let mut out = [0; 16];
    for y in 0..4 {
        let values = transform_line(temporary[y * 4..y * 4 + 4].try_into().unwrap(), walsh);
        for x in 0..4 {
            out[y * 4 + x] = (values[x] + if walsh { 3 } else { 4 }) >> 3;
        }
    }
    out
}

struct Plane {
    width: usize,
    height: usize,
    data: Vec<u8>,
}
impl Plane {
    fn new(width: usize, height: usize) -> ImageResult<Self> {
        let mut data = reserved(width * height)?;
        data.resize(width * height, 0);
        Ok(Self {
            width,
            height,
            data,
        })
    }
    fn sample(&self, x: isize, y: isize) -> i32 {
        if y < 0 {
            127
        } else if x < 0 {
            129
        } else {
            i32::from(
                self.data[(y as usize).min(self.height - 1) * self.width
                    + (x as usize).min(self.width - 1)],
            )
        }
    }
    fn predict(&mut self, x: usize, y: usize, size: usize, mode: u8) {
        let mut top = [0; 16];
        let mut left = [0; 16];
        for i in 0..size {
            top[i] = self.sample((x + i) as isize, y as isize - 1);
            left[i] = self.sample(x as isize - 1, (y + i) as isize);
        }
        let corner = self.sample(x as isize - 1, y as isize - 1);
        let count = size * (usize::from(x != 0) + usize::from(y != 0));
        let sum = (if y != 0 { top[..size].iter().sum() } else { 0 })
            + (if x != 0 {
                left[..size].iter().sum::<i32>()
            } else {
                0
            });
        let dc = if count == 0 {
            128
        } else {
            (sum + count as i32 / 2) / count as i32
        };
        for (r, &left_value) in left.iter().enumerate().take(size) {
            for (c, &top_value) in top.iter().enumerate().take(size) {
                let v = match mode {
                    0 => dc,
                    1 => top_value,
                    2 => left_value,
                    _ => left_value + top_value - corner,
                };
                self.data[(y + r) * self.width + x + c] = v.clamp(0, 255) as u8;
            }
        }
    }
    fn predict4(&mut self, x: usize, y: usize, macro_y: usize, mode: u8) {
        let mut top = [0; 8];
        let mut left = [0; 4];
        for (i, value) in top.iter_mut().enumerate() {
            let above_y = if x % 16 == 12 && i >= 4 {
                macro_y as isize - 1
            } else {
                y as isize - 1
            };
            *value = self.sample((x + i) as isize, above_y);
        }
        for (i, value) in left.iter_mut().enumerate() {
            *value = self.sample(x as isize - 1, (y + i) as isize);
        }
        let corner = self.sample(x as isize - 1, y as isize - 1);
        let edge = [
            left[3], left[2], left[1], left[0], corner, top[0], top[1], top[2], top[3],
        ];
        let avg2 = |a, b| (a + b + 1) >> 1;
        let avg3 = |a, b, c| (a + 2 * b + c + 2) >> 2;
        let e2 = |n: usize| avg2(edge[n], edge[n + 1]);
        let e3 = |n: usize| avg3(edge[n - 1], edge[n], edge[n + 1]);
        let t3 = |n: usize| avg3(top[n - 1], top[n], top[(n + 1).min(7)]);
        let dc = (top[..4].iter().sum::<i32>() + left.iter().sum::<i32>() + 4) >> 3;
        for r in 0usize..4 {
            for c in 0..4 {
                let v = match mode {
                    0 => dc,
                    1 => left[r] + top[c] - corner,
                    2 => avg3(if c == 0 { corner } else { top[c - 1] }, top[c], top[c + 1]),
                    3 => avg3(
                        if r == 0 { corner } else { left[r - 1] },
                        left[r],
                        left[(r + 1).min(3)],
                    ),
                    4 => t3(r + c + 1),
                    5 => e3(4 + c - r),
                    6 => {
                        let position = 2 * c as i32 - r as i32;
                        if position < -1 {
                            e3((5 + position) as usize)
                        } else if position & 1 == 0 {
                            e2((4 + position / 2) as usize)
                        } else {
                            e3((5 + (position - 1) / 2) as usize)
                        }
                    }
                    7 => {
                        if r == 2 && c == 3 {
                            t3(5)
                        } else if r == 3 && c == 3 {
                            t3(6)
                        } else if r & 1 == 0 {
                            avg2(top[c + r / 2], top[c + r / 2 + 1])
                        } else {
                            t3(c + r.div_ceil(2))
                        }
                    }
                    8 => {
                        let position = 2 * r as i32 - c as i32;
                        if position < -1 {
                            e3((3 - position) as usize)
                        } else if position & 1 == 0 {
                            e2((3 - position / 2) as usize)
                        } else {
                            e3((4 - (position + 1) / 2) as usize)
                        }
                    }
                    _ => {
                        let position = 2 * r + c;
                        if position >= 6 {
                            left[3]
                        } else if position & 1 == 0 {
                            avg2(left[position / 2], left[position / 2 + 1])
                        } else {
                            avg3(
                                left[position / 2],
                                left[position / 2 + 1],
                                left[(position / 2 + 2).min(3)],
                            )
                        }
                    }
                };
                self.data[(y + r) * self.width + x + c] = v.clamp(0, 255) as u8;
            }
        }
    }
    fn add(&mut self, x: usize, y: usize, coefficients: [i32; 16]) {
        let residual = inverse(coefficients, false);
        for r in 0..4 {
            for c in 0..4 {
                let at = (y + r) * self.width + x + c;
                self.data[at] =
                    (i32::from(self.data[at]) + residual[r * 4 + c]).clamp(0, 255) as u8;
            }
        }
    }
    fn filter_edge(
        &mut self,
        x: usize,
        y: usize,
        vertical: bool,
        length: usize,
        cfg: Filter,
        macro_edge: bool,
    ) {
        let step = if vertical { 1 } else { self.width };
        let advance = if vertical { self.width } else { 1 };
        for n in 0..length {
            let base = y * self.width + x + n * advance;
            let mut p = [0; 8];
            for (i, value) in p.iter_mut().enumerate() {
                *value = i32::from(self.data[base + i * step - 4 * step]);
            }
            let edge_limit = 2 * (cfg.level + if macro_edge { 2 } else { 0 }) + cfg.interior;
            if 2 * (p[3] - p[4]).abs() + (p[2] - p[5]).abs() / 2 > edge_limit {
                continue;
            }
            if !cfg.simple
                && (0..3)
                    .chain(4..7)
                    .any(|i| (p[i] - p[i + 1]).abs() > cfg.interior)
            {
                continue;
            }
            let high = (p[2] - p[3]).abs() > cfg.variance || (p[4] - p[5]).abs() > cfg.variance;
            let outer = cfg.simple || macro_edge || high;
            let delta = ((if outer {
                (p[2] - p[5]).clamp(-128, 127)
            } else {
                0
            }) + 3 * (p[4] - p[3]))
                .clamp(-128, 127);
            if macro_edge && !cfg.simple && !high {
                for (i, multiplier) in [27, 18, 9].into_iter().enumerate() {
                    let adjustment = (multiplier * delta + 63) >> 7;
                    p[3 - i] = (p[3 - i] + adjustment).clamp(0, 255);
                    p[4 + i] = (p[4 + i] - adjustment).clamp(0, 255);
                }
            } else {
                let after = (delta + 4).clamp(-128, 127) >> 3;
                let before = (delta + 3).clamp(-128, 127) >> 3;
                p[3] = (p[3] + before).clamp(0, 255);
                p[4] = (p[4] - after).clamp(0, 255);
                if !cfg.simple && !high {
                    let half = (after + 1) >> 1;
                    p[2] = (p[2] + half).clamp(0, 255);
                    p[5] = (p[5] - half).clamp(0, 255);
                }
            }
            for (i, &value) in p.iter().enumerate() {
                self.data[base + i * step - 4 * step] = value as u8;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Filter {
    level: i32,
    interior: i32,
    variance: i32,
    simple: bool,
}
fn filtering(
    planes: &mut [Plane; 3],
    blocks: &[Macroblock],
    header: &Header,
    columns: usize,
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<()> {
    // RFC 6386 section 15: a zero frame-level filter disables the pass,
    // even when reference/mode adjustments would otherwise raise the level.
    if header.filter_level == 0 {
        return Ok(());
    }
    for (index, block) in blocks.iter().enumerate() {
        check()?;
        let level = header.level(block.segment, block.luma == 4);
        if level == 0 {
            continue;
        }
        let mut interior = level;
        if header.sharpness != 0 {
            interior >>= if header.sharpness > 4 { 2 } else { 1 };
            interior = interior.min(9 - header.sharpness);
        }
        let cfg = Filter {
            level,
            interior: interior.max(1),
            variance: if level >= 40 {
                2
            } else {
                i32::from(level >= 15)
            },
            simple: header.simple,
        };
        let mx = index % columns;
        let my = index / columns;
        for (plane_index, plane) in planes.iter_mut().enumerate() {
            if plane_index > 0 && cfg.simple {
                continue;
            }
            let size = if plane_index == 0 { 16 } else { 8 };
            let x = mx * size;
            let y = my * size;
            if mx != 0 {
                plane.filter_edge(x, y, true, size, cfg, true);
            }
            if block.luma == 4 || block.residual {
                for offset in (4..size).step_by(4) {
                    plane.filter_edge(x + offset, y, true, size, cfg, false);
                }
            }
            if my != 0 {
                plane.filter_edge(x, y, false, size, cfg, true);
            }
            if block.luma == 4 || block.residual {
                for offset in (4..size).step_by(4) {
                    plane.filter_edge(x, y + offset, false, size, cfg, false);
                }
            }
        }
    }
    Ok(())
}

fn upsample(plane: &Plane, x: usize, y: usize, width: usize, height: usize) -> i32 {
    let point = |coordinate: usize, size: usize| {
        let half = coordinate / 2;
        if coordinate & 1 == 0 {
            (
                half.saturating_sub(1),
                half.min(size - 1),
                if coordinate == 0 { 0 } else { 1 },
                3,
            )
        } else {
            (half.min(size - 1), (half + 1).min(size - 1), 3, 1)
        }
    };
    let (x0, x1, a, b) = point(x, width.div_ceil(2));
    let (y0, y1, c, d) = point(y, height.div_ceil(2));
    // At the left/top boundary both samples are the first pixel.
    let a = if a == 0 { 1 } else { a };
    let c = if c == 0 { 1 } else { c };
    let row = |row| {
        i32::from(plane.data[row * plane.width + x0]) * a
            + i32::from(plane.data[row * plane.width + x1]) * b
    };
    (row(y0) * c + row(y1) * d + 8) >> 4
}

pub(super) fn decode(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    decode_with_pixels(data,check,super::super::MAX_PIXELS)
}

pub(super) fn decode_with_pixels(data:&[u8],check:&dyn Fn()->ImageResult<()>,maximum_pixels:usize)->ImageResult<Bitmap> {
    check()?;
    if data.len() < 10 {
        return Err("Lossy WebP has a truncated VP8 frame header.".into());
    }
    let tag = usize::from(data[0]) | (usize::from(data[1]) << 8) | (usize::from(data[2]) << 16);
    if tag & 1 != 0 {
        return Err("WebP requires a VP8 key frame; interframes are unsupported.".into());
    }
    if (tag >> 1) & 7 > 3 {
        return Err("Unsupported VP8 bitstream version.".into());
    }
    if tag & 16 == 0 {
        return Err("WebP VP8 frame is not marked for display.".into());
    }
    if data[3..6] != [0x9d, 1, 0x2a] {
        return Err("Invalid VP8 key-frame start code.".into());
    }
    let w = le16(&data[6..]);
    let h = le16(&data[8..]);
    if w & 0xc000 != 0 || h & 0xc000 != 0 {
        return Err("VP8 display scaling is not supported in WebP images.".into());
    }
    let width = usize::from(w);
    let height = usize::from(h);
    let pixels = dimensions_with_limit(width, height,maximum_pixels)?;
    let first_end = 10usize
        .checked_add(tag >> 5)
        .filter(|&n| n <= data.len())
        .ok_or("VP8 header partition is truncated.")?;
    let work = Work {
        remaining: Cell::new(MAX_BOOLEAN_WORK),
        check,
    };
    let mut bits = Bool::new(&data[10..first_end], &work);
    let header = Header::read(&mut bits)?;
    let sizes = (header.partitions - 1) * 3;
    let mut next = first_end
        .checked_add(sizes)
        .filter(|&n| n <= data.len())
        .ok_or("VP8 token partition directory is truncated.")?;
    let mut tokens = Vec::new();
    for partition in 0..header.partitions {
        let end = if partition + 1 == header.partitions {
            data.len()
        } else {
            let at = first_end + partition * 3;
            let size = usize::from(data[at])
                | (usize::from(data[at + 1]) << 8)
                | (usize::from(data[at + 2]) << 16);
            next.checked_add(size)
                .filter(|&n| n <= data.len())
                .ok_or("VP8 token partition is truncated.")?
        };
        tokens.push(Bool::new(&data[next..end], &work));
        next = end;
    }
    let columns = width.div_ceil(16);
    let rows = height.div_ceil(16);
    let mut planes = [
        Plane::new(columns * 16, rows * 16)?,
        Plane::new(columns * 8, rows * 8)?,
        Plane::new(columns * 8, rows * 8)?,
    ];
    let mut modes = vec![0; columns * 4];
    let mut context = vec![[0u8; 9]; columns];
    let mut records = reserved(columns * rows)?;
    for my in 0..rows {
        let mut left_modes = [0; 4];
        let mut left = [0u8; 9];
        for mx in 0..columns {
            check()?;
            let mut block = mode_record(
                &mut bits,
                &header,
                &mut modes[mx * 4..mx * 4 + 4],
                &mut left_modes,
            )?;
            let quant = header.quantizers(block.segment);
            let y2 = block.luma != 4;
            let mut residue = [[0i32; 16]; 24];
            if block.skip {
                left[..8].fill(0);
                context[mx][..8].fill(0);
                if y2 {
                    left[8] = 0;
                    context[mx][8] = 0;
                }
            } else {
                let reader = &mut tokens[my % header.partitions];
                let mut dc = [0; 16];
                if y2 {
                    let (coeff, nz) = coefficients(
                        reader,
                        &header.probabilities,
                        1,
                        usize::from(left[8] + context[mx][8]),
                        quant[1],
                    )?;
                    left[8] = u8::from(nz);
                    context[mx][8] = u8::from(nz);
                    block.residual |= nz;
                    dc = inverse(coeff, true);
                }
                for (n, coeff) in residue.iter_mut().enumerate() {
                    let (kind, base, row, col, q) = if n < 16 {
                        (if y2 { 0 } else { 3 }, 0, n / 4, n % 4, quant[0])
                    } else {
                        (2, if n < 20 { 4 } else { 6 }, (n % 4) / 2, n % 2, quant[2])
                    };
                    let (mut value, nz) = coefficients(
                        reader,
                        &header.probabilities,
                        kind,
                        usize::from(left[base + row] + context[mx][base + col]),
                        q,
                    )?;
                    left[base + row] = u8::from(nz);
                    context[mx][base + col] = u8::from(nz);
                    block.residual |= nz;
                    if y2 && n < 16 {
                        value[0] = dc[n];
                    }
                    *coeff = value;
                }
            }
            if block.luma != 4 {
                planes[0].predict(mx * 16, my * 16, 16, block.luma);
            }
            for (n, &coeff) in residue[..16].iter().enumerate() {
                let x = mx * 16 + n % 4 * 4;
                let y = my * 16 + n / 4 * 4;
                if block.luma == 4 {
                    planes[0].predict4(x, y, my * 16, block.modes[n]);
                }
                planes[0].add(x, y, coeff);
            }
            for component in 1..3 {
                planes[component].predict(mx * 8, my * 8, 8, block.chroma);
                for n in 0..4 {
                    planes[component].add(
                        mx * 8 + n % 2 * 4,
                        my * 8 + n / 2 * 4,
                        residue[16 + (component - 1) * 4 + n],
                    );
                }
            }
            records.push(block);
        }
    }
    filtering(&mut planes, &records, &header, columns, check)?;
    let mut output = reserved(pixels)?;
    for y in 0..height {
        check()?;
        for x in 0..width {
            let luma = i32::from(planes[0].data[y * planes[0].width + x]) - 16;
            let u = upsample(&planes[1], x, y, width, height) - 128;
            let v = upsample(&planes[2], x, y, width, height) - 128;
            let color = |sum: i32| ((sum + 8192) >> 14).clamp(0, 255) as u8;
            output.push(pixel(
                color(19077 * luma + 26149 * v),
                color(19077 * luma - 6419 * u - 13320 * v),
                color(19077 * luma + 33050 * u),
                255,
            ));
        }
    }
    Ok(Bitmap {
        width,
        height,
        pixels: output,
    })
}
