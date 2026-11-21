//! Our TrueType outline reader and grayscale rasterizer. Font files are data;
//! no platform text renderer, shaping library or TrueType instruction VM runs.
//! Formats: OpenType `head`, `hhea`, `hmtx`, `maxp`, `loca`, `glyf`, `cmap`,
//! legacy horizontal `kern`, and the W3C WOFF 1.0 container.
use std::collections::BTreeMap;
use std::sync::Arc;

pub const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
const MAX_TABLES: usize = 256;
const MAX_POINTS: usize = 65_536;
const MAX_CONTOURS: usize = 4096;
const MAX_COMPONENTS: usize = 256;
const MAX_DEPTH: usize = 16;
const MAX_EDGES: usize = 65_536;
const MAX_PIXELS: usize = 1024 * 1024;
const MAX_SCAN_WORK: usize = 32 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug)]
struct Table {
    start: usize,
    len: usize,
}

#[derive(Debug)]
enum Cmap {
    Segments(Vec<Segment>),
    Groups(Vec<Group>),
}

#[derive(Debug)]
struct Segment {
    start: u16,
    end: u16,
    delta: u16,
    glyphs: Option<Vec<u16>>,
}

#[derive(Debug)]
struct Group {
    start: u32,
    end: u32,
    glyph: u32,
}

#[derive(Debug)]
pub struct Font {
    bytes: Arc<[u8]>,
    glyf: Table,
    offsets: Vec<u32>,
    advances: Vec<u16>,
    units: u16,
    ascent: i16,
    descent: i16,
    line_gap: i16,
    cmap: Cmap,
    kern: Vec<(u32, i32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub ascent: f32,
    /// Positive distance below the baseline.
    pub descent: f32,
    pub line_gap: f32,
}

#[derive(Debug)]
pub struct GlyphBitmap {
    pub left: i32,
    /// Vertical offset from the baseline in canvas coordinates, normally negative.
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub coverage: Vec<u8>,
    /// Pen advance for callers drawing one glyph without a separate metrics call.
    #[allow(dead_code)]
    pub advance: f32,
}

fn bytes(data: &[u8], at: usize, count: usize) -> Result<&[u8]> {
    let end = at.checked_add(count).ok_or("Font offset overflow.")?;
    data.get(at..end).ok_or_else(|| {
        format!(
            "Truncated font data (offset {at}, length {count}, table size {}).",
            data.len()
        )
    })
}
fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    let value = bytes(data, at, 2)?;
    Ok(u16::from_be_bytes([value[0], value[1]]))
}
fn i16_at(data: &[u8], at: usize) -> Result<i16> {
    Ok(u16_at(data, at)? as i16)
}
fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    let value = bytes(data, at, 4)?;
    Ok(u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
}
fn checksum(data: &[u8], zero_adjustment: bool) -> u32 {
    let mut sum = 0u32;
    for (index, chunk) in data.chunks(4).enumerate() {
        if zero_adjustment && index == 2 {
            continue;
        }
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

fn decode_woff(data: &[u8]) -> Result<Vec<u8>> {
    bytes(data, 0, 44)?;
    let flavor = u32_at(data, 4)?;
    if flavor != 0x00010000 && flavor != u32::from_be_bytes(*b"true") {
        return Err(
            "WOFF uses unsupported CFF/OpenType outlines; TrueType outlines are required.".into(),
        );
    }
    if u32_at(data, 8)? as usize != data.len() || u16_at(data, 14)? != 0 {
        return Err("Invalid WOFF length or reserved field.".into());
    }
    let count = usize::from(u16_at(data, 12)?);
    let total = u32_at(data, 16)? as usize;
    if count == 0 || count > MAX_TABLES || total > MAX_FONT_BYTES {
        return Err("WOFF table count or decoded size exceeds its limit.".into());
    }
    let directory_end = 44 + count * 20;
    bytes(data, 0, directory_end)?;
    let mut ranges = vec![(0usize, directory_end)];
    let mut decoded_size = 12 + 16 * count;
    let mut previous_tag = None;
    for index in 0..count {
        let at = 44 + index * 20;
        let tag = u32_at(data, at)?;
        let offset = u32_at(data, at + 4)? as usize;
        let compressed = u32_at(data, at + 8)? as usize;
        let original = u32_at(data, at + 12)? as usize;
        if previous_tag.is_some_and(|last| tag <= last)
            || !offset.is_multiple_of(4)
            || compressed > original
            || original > MAX_FONT_BYTES
        {
            return Err("Invalid WOFF table directory.".into());
        }
        previous_tag = Some(tag);
        bytes(data, offset, compressed)?;
        if compressed > 0 {
            ranges.push((offset, offset + compressed));
        }
        decoded_size = decoded_size
            .checked_add((original + 3) & !3)
            .ok_or("WOFF size overflow.")?;
    }
    if decoded_size != total || decoded_size > MAX_FONT_BYTES {
        return Err("WOFF totalSfntSize does not match its tables.".into());
    }
    for (offset_field, length_field) in [(24, 28), (36, 40)] {
        let offset = u32_at(data, offset_field)? as usize;
        let len = u32_at(data, length_field)? as usize;
        if offset == 0 {
            if len != 0 || (offset_field == 24 && u32_at(data, 32)? != 0) {
                return Err("Invalid optional WOFF block.".into());
            }
        } else {
            if !offset.is_multiple_of(4)
                || len == 0
                || (offset_field == 24 && u32_at(data, 32)? == 0)
            {
                return Err("Invalid optional WOFF block.".into());
            }
            bytes(data, offset, len)?;
            ranges.push((offset, offset + len));
        }
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err("Overlapping WOFF blocks.".into());
    }
    let mut output = vec![0u8; 12 + count * 16];
    output[..4].copy_from_slice(&flavor.to_be_bytes());
    output[4..6].copy_from_slice(&(count as u16).to_be_bytes());
    let power = 1usize << count.ilog2();
    output[6..8].copy_from_slice(&((power * 16) as u16).to_be_bytes());
    output[8..10].copy_from_slice(&(power.ilog2() as u16).to_be_bytes());
    output[10..12].copy_from_slice(&((count * 16 - power * 16) as u16).to_be_bytes());
    let mut head_start = None;
    for index in 0..count {
        let at = 44 + index * 20;
        let tag = bytes(data, at, 4)?;
        let offset = u32_at(data, at + 4)? as usize;
        let compressed = u32_at(data, at + 8)? as usize;
        let original = u32_at(data, at + 12)? as usize;
        let expected = u32_at(data, at + 16)?;
        let packed = bytes(data, offset, compressed)?;
        let decoded;
        let table = if compressed < original {
            decoded = crate::compression::zlib(packed, original, &|| Ok(()))?;
            if decoded.len() != original {
                return Err("WOFF table decompression length does not match.".into());
            }
            decoded.as_slice()
        } else {
            packed
        };
        if checksum(table, tag == b"head") != expected {
            return Err("WOFF table checksum does not match.".into());
        }
        let target = 12 + index * 16;
        let start = output.len();
        output[target..target + 4].copy_from_slice(tag);
        output[target + 4..target + 8].copy_from_slice(&expected.to_be_bytes());
        output[target + 8..target + 12].copy_from_slice(&(start as u32).to_be_bytes());
        output[target + 12..target + 16].copy_from_slice(&(original as u32).to_be_bytes());
        output.extend_from_slice(table);
        while !output.len().is_multiple_of(4) {
            output.push(0);
        }
        if tag == b"head" {
            if original < 12 {
                return Err("Truncated WOFF head table.".into());
            }
            output[start + 8..start + 12].fill(0);
            head_start = Some(start);
        }
    }
    if let Some(start) = head_start {
        let adjustment = 0xb1b0afbau32;
        let adjustment = adjustment.wrapping_sub(checksum(&output, false));
        output[start + 8..start + 12].copy_from_slice(&adjustment.to_be_bytes());
    }
    Ok(output)
}

impl Font {
    pub fn parse(source: &[u8]) -> Result<Self> {
        if source.len() > MAX_FONT_BYTES {
            return Err("Font exceeds the 16 MiB limit.".into());
        }
        let signature = bytes(source, 0, 4)?;
        let decoded;
        let source = if signature == b"wOFF" {
            decoded = decode_woff(source)?;
            decoded.as_slice()
        } else if signature == b"wOF2" {
            return Err(
                "WOFF2 fonts are not implemented; a TTF or WOFF1 source is required.".into(),
            );
        } else if signature == b"OTTO" {
            return Err(
                "CFF/OpenType outlines are not implemented; TrueType outlines are required.".into(),
            );
        } else if signature == b"ttcf" {
            return Err("TrueType font collections are not implemented.".into());
        } else {
            source
        };
        let version = u32_at(source, 0)?;
        if version != 0x00010000 && version != u32::from_be_bytes(*b"true") {
            return Err("Unrecognized TrueType font signature.".into());
        }
        let count = usize::from(u16_at(source, 4)?);
        if count == 0 || count > MAX_TABLES {
            return Err("Font table count exceeds its limit.".into());
        }
        let directory_end = 12 + 16 * count;
        bytes(source, 0, directory_end)?;
        let mut tables = BTreeMap::new();
        let mut ranges = vec![(0usize, directory_end)];
        for index in 0..count {
            let at = 12 + index * 16;
            let tag: [u8; 4] = bytes(source, at, 4)?.try_into().unwrap();
            let table = Table {
                start: u32_at(source, at + 8)? as usize,
                len: u32_at(source, at + 12)? as usize,
            };
            bytes(source, table.start, table.len)?;
            if !table.start.is_multiple_of(4) || tables.insert(tag, table).is_some() {
                return Err("Invalid or duplicate TrueType table.".into());
            }
            if table.len > 0 {
                ranges.push((table.start, table.start + table.len));
            }
        }
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err("Overlapping TrueType tables.".into());
        }
        if tables.contains_key(b"fvar") || tables.contains_key(b"gvar") {
            return Err(
                "Variable TrueType fonts are not implemented; use a static font face.".into(),
            );
        }
        let get = |name: &[u8; 4]| -> Result<&[u8]> {
            let table = tables.get(name).ok_or_else(|| {
                format!("Missing TrueType {} table.", String::from_utf8_lossy(name))
            })?;
            bytes(source, table.start, table.len)
        };
        let head = get(b"head")?;
        let maxp = get(b"maxp")?;
        let hhea = get(b"hhea")?;
        let hmtx = get(b"hmtx")?;
        let loca = get(b"loca")?;
        bytes(head, 0, 54)?;
        bytes(hhea, 0, 36)?;
        if u32_at(head, 12)? != 0x5f0f3cf5 || i16_at(head, 52)? != 0 {
            return Err("Invalid TrueType head table.".into());
        }
        let units = u16_at(head, 18)?;
        if !(16..=16384).contains(&units) {
            return Err("Invalid TrueType unitsPerEm.".into());
        }
        let glyph_count = usize::from(u16_at(maxp, 4)?);
        let metric_count = usize::from(u16_at(hhea, 34)?);
        if glyph_count == 0 || metric_count == 0 || metric_count > glyph_count {
            return Err("Invalid TrueType glyph or metric count.".into());
        }
        bytes(hmtx, 0, metric_count * 4 + (glyph_count - metric_count) * 2)?;
        let advances = (0..glyph_count)
            .map(|glyph| u16_at(hmtx, glyph.min(metric_count - 1) * 4))
            .collect::<Result<Vec<_>>>()?;
        let long = match i16_at(head, 50)? {
            0 => false,
            1 => true,
            _ => return Err("Invalid TrueType loca format.".into()),
        };
        bytes(loca, 0, (glyph_count + 1) * if long { 4 } else { 2 })?;
        let glyf = *tables.get(b"glyf").ok_or("Missing TrueType glyf table.")?;
        let mut offsets = Vec::with_capacity(glyph_count + 1);
        for glyph in 0..=glyph_count {
            let offset = if long {
                u32_at(loca, glyph * 4)?
            } else {
                u32::from(u16_at(loca, glyph * 2)?) * 2
            };
            if offset as usize > glyf.len || offsets.last().is_some_and(|&last| offset < last) {
                return Err("Invalid TrueType glyph offsets.".into());
            }
            offsets.push(offset);
        }
        let cmap = read_cmap(get(b"cmap")?)?;
        let kern = tables
            .get(b"kern")
            .map(|table| read_kern(&source[table.start..table.start + table.len]))
            .transpose()?
            .unwrap_or_default();
        let mut ascent = i16_at(hhea, 4)?;
        let mut descent = i16_at(hhea, 6)?;
        let mut line_gap = i16_at(hhea, 8)?;
        if let Some(table) = tables.get(b"OS/2") {
            let os2 = &source[table.start..table.start + table.len];
            if os2.len() >= 74 && u16_at(os2, 62)? & 0x0080 != 0 {
                ascent = i16_at(os2, 68)?;
                descent = i16_at(os2, 70)?;
                line_gap = i16_at(os2, 72)?;
            }
        }
        if ascent <= 0 || descent > 0 {
            return Err("Unsupported TrueType vertical metrics.".into());
        }
        Ok(Self {
            bytes: Arc::from(source),
            glyf,
            offsets,
            advances,
            units,
            ascent,
            descent,
            line_gap,
            cmap,
            kern,
        })
    }

    pub fn glyph_index(&self, character: char) -> u16 {
        let code = character as u32;
        let glyph = match &self.cmap {
            Cmap::Groups(groups) => {
                let index = groups.partition_point(|group| group.end < code);
                groups
                    .get(index)
                    .filter(|group| group.start <= code)
                    .map_or(0, |group| group.glyph + code - group.start)
            }
            Cmap::Segments(segments) if code <= 0xffff => {
                let code = code as u16;
                let index = segments.partition_point(|segment| segment.end < code);
                segments
                    .get(index)
                    .filter(|segment| segment.start <= code)
                    .map_or(0, |segment| {
                        if let Some(glyphs) = &segment.glyphs {
                            let glyph = glyphs[usize::from(code - segment.start)];
                            if glyph == 0 {
                                0
                            } else {
                                u32::from(glyph.wrapping_add(segment.delta))
                            }
                        } else {
                            u32::from(code.wrapping_add(segment.delta))
                        }
                    })
            }
            _ => 0,
        };
        if glyph < self.advances.len() as u32 {
            glyph as u16
        } else {
            0
        }
    }

    pub fn advance(&self, glyph: u16, size: f32) -> f32 {
        f32::from(self.advances[usize::from(glyph).min(self.advances.len() - 1)]) * self.scale(size)
    }

    pub fn metrics(&self, size: f32) -> Metrics {
        let scale = self.scale(size);
        Metrics {
            ascent: f32::from(self.ascent) * scale,
            descent: -f32::from(self.descent) * scale,
            line_gap: f32::from(self.line_gap.max(0)) * scale,
        }
    }

    pub fn kerning(&self, left: u16, right: u16, size: f32) -> f32 {
        let key = (u32::from(left) << 16) | u32::from(right);
        self.kern
            .binary_search_by_key(&key, |pair| pair.0)
            .ok()
            .map_or(0.0, |index| self.kern[index].1 as f32 * self.scale(size))
    }

    fn scale(&self, size: f32) -> f32 {
        if size.is_finite() {
            size.clamp(0.0, 512.0) / f32::from(self.units)
        } else {
            0.0
        }
    }

    pub fn rasterize(&self, glyph: u16, size: f32) -> Result<GlyphBitmap> {
        if !size.is_finite() || !(0.25..=512.0).contains(&size) {
            return Err("Font pixel size must be between 0.25 and 512.".into());
        }
        if usize::from(glyph) >= self.advances.len() {
            return Err("Glyph index exceeds the font.".into());
        }
        let mut component_budget = MAX_COMPONENTS;
        let outline = self.outline(glyph, &mut Vec::new(), &mut component_budget)?;
        raster(outline, self.scale(size), self.advance(glyph, size))
    }

    fn outline(&self, glyph: u16, ancestors: &mut Vec<u16>, budget: &mut usize) -> Result<Outline> {
        if ancestors.len() >= MAX_DEPTH || ancestors.contains(&glyph) || *budget == 0 {
            return Err("Composite glyph recursion or component limit exceeded.".into());
        }
        *budget -= 1;
        let index = usize::from(glyph);
        if index >= self.advances.len() {
            return Err("Composite glyph index exceeds the font.".into());
        }
        let start = self.offsets[index] as usize;
        let end = self.offsets[index + 1] as usize;
        if start == end {
            return Ok(Outline::default());
        }
        let data = &self.bytes[self.glyf.start + start..self.glyf.start + end];
        bytes(data, 0, 10)?;
        let count = i16_at(data, 0)?;
        if count >= 0 {
            return simple_outline(data, count as usize);
        }
        if count != -1 {
            return Err("Invalid TrueType contour count.".into());
        }
        ancestors.push(glyph);
        let result = self.composite_outline(data, ancestors, budget);
        ancestors.pop();
        result
    }

    fn composite_outline(
        &self,
        data: &[u8],
        ancestors: &mut Vec<u16>,
        budget: &mut usize,
    ) -> Result<Outline> {
        let mut cursor = Cursor::new(data, 10);
        let mut result = Outline::default();
        let final_flags;
        loop {
            let flags = cursor.u16()?;
            let component = cursor.u16()?;
            if flags & 0xe000 != 0 || flags & 0x1800 == 0x1800 || (flags & 0x00c8).count_ones() > 1
            {
                return Err("Invalid composite glyph flags.".into());
            }
            let xy = flags & 2 != 0;
            let (arg1, arg2) = if flags & 1 != 0 {
                if xy {
                    (i32::from(cursor.i16()?), i32::from(cursor.i16()?))
                } else {
                    (i32::from(cursor.u16()?), i32::from(cursor.u16()?))
                }
            } else if xy {
                (i32::from(cursor.u8()? as i8), i32::from(cursor.u8()? as i8))
            } else {
                (i32::from(cursor.u8()?), i32::from(cursor.u8()?))
            };
            let (mut a, mut b, mut c, mut d) = (1.0, 0.0, 0.0, 1.0);
            if flags & 8 != 0 {
                a = cursor.f2dot14()?;
                d = a;
            } else if flags & 0x40 != 0 {
                a = cursor.f2dot14()?;
                d = cursor.f2dot14()?;
            } else if flags & 0x80 != 0 {
                a = cursor.f2dot14()?;
                b = cursor.f2dot14()?;
                c = cursor.f2dot14()?;
                d = cursor.f2dot14()?;
            }
            let mut child = self.outline(component, ancestors, budget)?;
            for point in &mut child.points {
                let (x, y) = (point.x, point.y);
                point.x = a * x + c * y;
                point.y = b * x + d * y;
            }
            let (mut dx, mut dy) = if xy {
                (arg1 as f32, arg2 as f32)
            } else {
                let parent = result
                    .points
                    .get(arg1 as usize)
                    .ok_or("Composite parent anchor is out of range.")?;
                let anchor = child
                    .points
                    .get(arg2 as usize)
                    .ok_or("Composite component anchor is out of range.")?;
                (parent.x - anchor.x, parent.y - anchor.y)
            };
            if xy && flags & 0x0800 != 0 {
                (dx, dy) = (a * dx + c * dy, b * dx + d * dy);
            }
            if result.points.len() + child.points.len() > MAX_POINTS
                || result.ends.len() + child.ends.len() > MAX_CONTOURS
            {
                return Err("Composite glyph outline exceeds its limit.".into());
            }
            for point in &mut child.points {
                point.x += dx;
                point.y += dy;
                if point.x.abs() > 1_048_576.0 || point.y.abs() > 1_048_576.0 {
                    return Err("Composite glyph coordinate exceeds its limit.".into());
                }
            }
            let offset = result.points.len();
            result
                .ends
                .extend(child.ends.into_iter().map(|end| end + offset));
            result.points.extend(child.points);
            if flags & 0x0020 == 0 {
                final_flags = flags;
                break;
            }
        }
        if final_flags & 0x0100 != 0 {
            let instruction_count = usize::from(cursor.u16()?);
            cursor.skip(instruction_count)?;
        }
        Ok(result)
    }
}

fn read_cmap(data: &[u8]) -> Result<Cmap> {
    if u16_at(data, 0)? != 0 {
        return Err("Invalid cmap version.".into());
    }
    let count = usize::from(u16_at(data, 2)?);
    if count == 0 {
        return Err("Invalid cmap encoding count.".into());
    }
    bytes(data, 4, count * 8)?;
    // The u16 count and admitted eight-byte records bound this traversal.
    // Retain the existing (format, platform, offset) preference without
    // allocating and sorting a candidate for every encoding record.
    let mut candidate = None;
    for index in 0..count {
        let at = 4 + index * 8;
        let platform = u16_at(data, at)?;
        let encoding = u16_at(data, at + 2)?;
        let offset = u32_at(data, at + 4)? as usize;
        if platform != 0 && !(platform == 3 && matches!(encoding, 1 | 10)) {
            continue;
        }
        let format = u16_at(data, offset)?;
        if format == 4 || format == 12 {
            let current = (format, platform, offset);
            if candidate.is_none_or(|previous| current > previous) {
                candidate = Some(current);
            }
        }
    }
    let Some((format, _, offset)) = candidate else {
        return Err("No supported Unicode cmap (format 4 or 12).".into());
    };
    if format == 12 {
        let len = u32_at(data, offset + 4)? as usize;
        let table = bytes(data, offset, len)?;
        let count = u32_at(table, 12)? as usize;
        if count > 65_536 || u16_at(table, 2)? != 0 {
            return Err("Invalid cmap format 12 groups.".into());
        }
        bytes(table, 16, count * 12)?;
        let mut groups: Vec<Group> = Vec::with_capacity(count);
        for index in 0..count {
            let at = 16 + index * 12;
            let group = Group {
                start: u32_at(table, at)?,
                end: u32_at(table, at + 4)?,
                glyph: u32_at(table, at + 8)?,
            };
            if group.start > group.end
                || group.end > 0x10ffff
                || group
                    .glyph
                    .checked_add(group.end - group.start)
                    .is_none_or(|last| last > 0xffff)
                || groups.last().is_some_and(|last| last.end >= group.start)
            {
                return Err("Invalid cmap format 12 range.".into());
            }
            groups.push(group);
        }
        Ok(Cmap::Groups(groups))
    } else {
        let len = usize::from(u16_at(data, offset + 2)?);
        let table = bytes(data, offset, len)?;
        let doubled = usize::from(u16_at(table, 6)?);
        if doubled == 0 || doubled % 2 != 0 {
            return Err("Invalid cmap segment count.".into());
        }
        let count = doubled / 2;
        bytes(table, 0, 16 + count * 8)?;
        if u16_at(table, 14 + count * 2)? != 0 {
            return Err("Invalid cmap reserved padding.".into());
        }
        let mut segments: Vec<Segment> = Vec::with_capacity(count);
        for index in 0..count {
            let end = u16_at(table, 14 + index * 2)?;
            let start = u16_at(table, 16 + count * 2 + index * 2)?;
            let delta = u16_at(table, 16 + count * 4 + index * 2)?;
            let range_at = 16 + count * 6 + index * 2;
            let range = usize::from(u16_at(table, range_at)?);
            if start > end
                || range % 2 != 0
                || segments.last().is_some_and(|last| last.end >= start)
            {
                return Err("Invalid cmap format 4 segment.".into());
            }
            let glyphs = if range == 0 {
                None
            } else {
                let start_at = range_at + range;
                let n = usize::from(end - start) + 1;
                if start_at < 16 + count * 8 {
                    return Err("cmap glyph array overlaps its header.".into());
                }
                bytes(table, start_at, n * 2)?;
                Some(
                    (0..n)
                        .map(|i| u16_at(table, start_at + i * 2))
                        .collect::<Result<Vec<_>>>()?,
                )
            };
            segments.push(Segment {
                start,
                end,
                delta,
                glyphs,
            });
        }
        if segments.last().is_none_or(|segment| segment.end != 0xffff) {
            return Err("Missing cmap terminal segment.".into());
        }
        Ok(Cmap::Segments(segments))
    }
}

fn read_kern(data: &[u8]) -> Result<Vec<(u32, i32)>> {
    if u16_at(data, 0)? != 0 {
        return Ok(Vec::new());
    }
    let count = usize::from(u16_at(data, 2)?);
    // Each of these u16-counted subtables must advance through at least six
    // admitted bytes. The separate aggregate-pair budget still bounds storage.
    let mut at = 4;
    let mut pairs = BTreeMap::<u32, i32>::new();
    let mut total = 0;
    for _ in 0..count {
        let mut len = usize::from(u16_at(data, at + 2)?);
        let flags = u16_at(data, at + 4)?;
        if flags >> 8 == 0 {
            let pairs = usize::from(u16_at(data, at + 6)?);
            let expected = 14 + pairs * 6;
            // Some static Open Sans releases contain more pairs than fit in the
            // original 16-bit format-0 length field. Accept only that exact
            // wraparound; the full computed range must still fit the kern table.
            if expected > usize::from(u16::MAX) && expected & 0xffff == len {
                len = expected;
            }
        }
        if len < 6 {
            return Err("Invalid kerning subtable length.".into());
        }
        let table = bytes(data, at, len)?;
        if flags >> 8 == 0 && flags & 7 == 1 {
            let count = usize::from(u16_at(table, 6)?);
            total += count;
            if total > 262_144 {
                return Err("Kerning pairs exceed their limit.".into());
            }
            bytes(table, 14, count * 6)?;
            let mut previous = None;
            for index in 0..count {
                let pos = 14 + index * 6;
                let key = u32_at(table, pos)?;
                if previous.is_some_and(|last| last >= key) {
                    return Err("Unsorted kerning pairs.".into());
                }
                previous = Some(key);
                let value = i32::from(i16_at(table, pos + 4)?);
                if flags & 8 != 0 {
                    pairs.insert(key, value);
                } else {
                    // Sorted keys occur at most once per subtable. Summing
                    // u16::MAX signed-i16 values still fits the i32 accumulator.
                    *pairs.entry(key).or_default() += value;
                }
            }
        }
        at += len;
    }
    Ok(pairs.into_iter().collect())
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], at: usize) -> Self {
        Self { data, at }
    }
    fn skip(&mut self, n: usize) -> Result<()> {
        bytes(self.data, self.at, n)?;
        self.at += n;
        Ok(())
    }
    fn u8(&mut self) -> Result<u8> {
        let value = bytes(self.data, self.at, 1)?[0];
        self.at += 1;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16> {
        let value = u16_at(self.data, self.at)?;
        self.at += 2;
        Ok(value)
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }
    fn f2dot14(&mut self) -> Result<f32> {
        Ok(f32::from(self.i16()?) / 16384.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f32,
    y: f32,
    on: bool,
}
#[derive(Default, Debug)]
struct Outline {
    points: Vec<Point>,
    ends: Vec<usize>,
}

fn simple_outline(data: &[u8], count: usize) -> Result<Outline> {
    if count > MAX_CONTOURS {
        return Err("Glyph contours exceed their limit.".into());
    }
    if count == 0 {
        return Ok(Outline::default());
    }
    let mut cursor = Cursor::new(data, 10);
    let mut ends = Vec::with_capacity(count);
    for _ in 0..count {
        let end = usize::from(cursor.u16()?);
        if ends.last().is_some_and(|last| end <= *last) {
            return Err("Invalid glyph contour endpoints.".into());
        }
        ends.push(end);
    }
    let n = ends.last().unwrap() + 1;
    if n > MAX_POINTS {
        return Err("Glyph points exceed their limit.".into());
    }
    let instructions = usize::from(cursor.u16()?);
    cursor.skip(instructions)?;
    let mut flags = Vec::with_capacity(n);
    while flags.len() < n {
        let flag = cursor.u8()?;
        if flag & 0x80 != 0 {
            return Err("Invalid TrueType point flags.".into());
        }
        let repeats = if flag & 8 != 0 {
            usize::from(cursor.u8()?) + 1
        } else {
            1
        };
        if flags.len() + repeats > n {
            return Err("Glyph flag repeat exceeds point count.".into());
        }
        flags.extend(std::iter::repeat_n(flag, repeats));
    }
    let mut points = vec![
        Point {
            x: 0.0,
            y: 0.0,
            on: false
        };
        n
    ];
    for (axis, short, same) in [(0, 2, 16), (1, 4, 32)] {
        let mut value = 0i32;
        for (index, &flag) in flags.iter().enumerate() {
            let delta = if flag & short != 0 {
                let delta = i32::from(cursor.u8()?);
                if flag & same != 0 {
                    delta
                } else {
                    -delta
                }
            } else if flag & same != 0 {
                0
            } else {
                i32::from(cursor.i16()?)
            };
            value = value
                .checked_add(delta)
                .ok_or("Glyph coordinate overflow.")?;
            if value.abs() > 1_048_576 {
                return Err("Glyph coordinate exceeds its limit.".into());
            }
            if axis == 0 {
                points[index].x = value as f32;
                points[index].on = flag & 1 != 0;
            } else {
                points[index].y = value as f32;
            }
        }
    }
    Ok(Outline { points, ends })
}

#[derive(Clone, Copy)]
struct Edge {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}
fn line(edges: &mut Vec<Edge>, a: Point, b: Point) -> Result<()> {
    if a.y == b.y {
        return Ok(());
    }
    if edges.len() >= MAX_EDGES {
        return Err("Flattened glyph edges exceed their limit.".into());
    }
    edges.push(Edge {
        x0: a.x,
        y0: a.y,
        x1: b.x,
        y1: b.y,
    });
    Ok(())
}
fn middle(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
        on: true,
    }
}
fn quadratic(edges: &mut Vec<Edge>, a: Point, b: Point, c: Point, depth: u8) -> Result<()> {
    if depth >= 12 || (a.x - 2.0 * b.x + c.x).abs() + (a.y - 2.0 * b.y + c.y).abs() <= 0.35 {
        return line(edges, a, c);
    }
    let ab = middle(a, b);
    let bc = middle(b, c);
    let split = middle(ab, bc);
    quadratic(edges, a, ab, split, depth + 1)?;
    quadratic(edges, split, bc, c, depth + 1)
}

fn raster(mut outline: Outline, scale: f32, advance: f32) -> Result<GlyphBitmap> {
    let empty = || GlyphBitmap {
        left: 0,
        top: 0,
        width: 0,
        height: 0,
        coverage: Vec::new(),
        advance,
    };
    if outline.points.is_empty() {
        return Ok(empty());
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    for point in &mut outline.points {
        point.x *= scale;
        point.y *= -scale;
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return Err("Invalid scaled glyph coordinates.".into());
    }
    let left = min_x.floor() as i32 - 1;
    let top = min_y.floor() as i32 - 1;
    let width = (max_x.ceil() as i32 - left + 1) as usize;
    let height = (max_y.ceil() as i32 - top + 1) as usize;
    if width > 2048
        || height > 2048
        || width
            .checked_mul(height)
            .is_none_or(|pixels| pixels > MAX_PIXELS)
    {
        return Err("Glyph raster dimensions exceed their limit.".into());
    }
    let mut edges = Vec::new();
    let mut first = 0;
    for end in outline.ends {
        let contour = &outline.points[first..=end];
        first = end + 1;
        let a = contour[0];
        let last = contour[contour.len() - 1];
        let start = if a.on {
            a
        } else if last.on {
            last
        } else {
            middle(last, a)
        };
        let mut current = start;
        let mut control: Option<Point> = None;
        for &point in contour {
            if point.on {
                if let Some(off) = control.take() {
                    quadratic(&mut edges, current, off, point, 0)?;
                } else {
                    line(&mut edges, current, point)?;
                }
                current = point;
            } else if let Some(off) = control.replace(point) {
                let implied = middle(off, point);
                quadratic(&mut edges, current, off, implied, 0)?;
                current = implied;
            }
        }
        if let Some(off) = control {
            quadratic(&mut edges, current, off, start, 0)?;
        } else {
            line(&mut edges, current, start)?;
        }
    }
    if edges.is_empty() {
        return Ok(empty());
    }
    if height * 4 > MAX_SCAN_WORK / edges.len() {
        return Err("Glyph raster work exceeds its limit.".into());
    }
    let mut accumulation = vec![0u16; width * height];
    let mut crossings = Vec::with_capacity(edges.len());
    for row in 0..height {
        for sample in 0..4 {
            let y = top as f32 + row as f32 + (sample as f32 + 0.5) / 4.0;
            crossings.clear();
            for edge in &edges {
                if y >= edge.y0.min(edge.y1) && y < edge.y0.max(edge.y1) {
                    let x = edge.x0 + (y - edge.y0) * (edge.x1 - edge.x0) / (edge.y1 - edge.y0);
                    crossings.push((x, if edge.y1 > edge.y0 { 1i32 } else { -1i32 }));
                }
            }
            crossings.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            let mut previous = 0.0;
            for &(x, direction) in &crossings {
                if winding != 0 {
                    let from = (previous - left as f32).clamp(0.0, width as f32);
                    let to = (x - left as f32).clamp(from, width as f32);
                    for column in from.floor() as usize..to.ceil() as usize {
                        let cover =
                            (to.min(column as f32 + 1.0) - from.max(column as f32)).max(0.0);
                        let amount = (cover * 256.0).round() as u16;
                        let pixel = &mut accumulation[row * width + column];
                        *pixel = pixel.saturating_add(amount);
                    }
                }
                winding += direction;
                previous = x;
            }
        }
    }
    let coverage = accumulation
        .into_iter()
        .map(|value| ((u32::from(value) * 255 + 512) / 1024).min(255) as u8)
        .collect();
    Ok(GlyphBitmap {
        left,
        top,
        width,
        height,
        coverage,
        advance,
    })
}

#[cfg(test)]
#[path = "font_tests.rs"]
pub(crate) mod tests;
