//! Original bounded classic TIFF decoder. No TIFF/image library is used.
//! References: TIFF 6.0 (ITU's published copy) and TIFF Technical Note #2.
//! The first image directory is displayed; further pages are not traversed.
//! Integer gray/palette/RGB/CMYK data, strips/tiles and planar/chunky layout
//! use none, PackBits, LZW or zlib compression. TIFF/JPEG uses our JPEG decoder.
//! Bilevel CCITT RLE, Group 3 and Group 4 are decoded from their published codes.

use super::{dimensions, pixel, reserved, Bitmap, ImageResult, MAX_INPUT_BYTES};
use std::collections::BTreeMap;

const MAX_FIELDS: usize = 4096;
const MAX_FIELD_VALUES: usize = 262_144;
const MAX_RASTER_BYTES: usize = 64 * 1024 * 1024;
const MAX_SEGMENT_BYTES: usize = 32 * 1024 * 1024;
const MAX_WORK: usize = 256 * 1024 * 1024;
type Check<'a> = &'a dyn Fn() -> ImageResult<()>;

struct Work<'a> {
    remaining: usize,
    since_check: usize,
    check: Check<'a>,
}
impl Work<'_> {
    fn charge(&mut self, amount: usize) -> ImageResult<()> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or("TIFF decoding work limit reached.")?;
        self.since_check += amount;
        if self.since_check >= 8192 {
            (self.check)()?;
            self.since_check = 0;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Reader<'a> {
    data: &'a [u8],
    little: bool,
}
impl<'a> Reader<'a> {
    fn bytes(self, at: usize, length: usize) -> ImageResult<&'a [u8]> {
        let end = at.checked_add(length).ok_or("TIFF offset overflow.")?;
        self.data
            .get(at..end)
            .ok_or_else(|| "TIFF field or segment is outside the input.".into())
    }
    fn short(self, at: usize) -> ImageResult<u16> {
        let b = self.bytes(at, 2)?;
        Ok(if self.little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    }
    fn long(self, at: usize) -> ImageResult<u32> {
        let b = self.bytes(at, 4)?;
        Ok(if self.little {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        })
    }
}

#[derive(Clone, Copy)]
struct Field {
    kind: u16,
    count: usize,
    at: usize,
}
struct Directory<'a> {
    reader: Reader<'a>,
    fields: BTreeMap<u16, Field>,
}
impl<'a> Directory<'a> {
    fn parse(data: &'a [u8], check: Check<'_>) -> ImageResult<Self> {
        if data.len() > MAX_INPUT_BYTES {
            return Err("TIFF input exceeds 32 MiB.".into());
        }
        let little = match data.get(..2) {
            Some(b"II") => true,
            Some(b"MM") => false,
            _ => return Err("TIFF byte-order signature is missing.".into()),
        };
        let reader = Reader { data, little };
        match reader.short(2)? {
            42 => (),
            43 => return Err("BigTIFF is not implemented; classic TIFF is supported.".into()),
            _ => return Err("Invalid TIFF version.".into()),
        }
        let first = reader.long(4)? as usize;
        if first < 8 || !first.is_multiple_of(2) {
            return Err("Invalid TIFF first-directory offset.".into());
        }
        let count = usize::from(reader.short(first)?);
        if count == 0 || count > MAX_FIELDS {
            return Err("TIFF directory field count exceeds its limit or is empty.".into());
        }
        reader.bytes(first + 2, count * 12 + 4)?;
        let mut fields = BTreeMap::new();
        for index in 0..count {
            if index.is_multiple_of(128) {
                check()?;
            }
            let entry = first + 2 + index * 12;
            let tag = reader.short(entry)?;
            let kind = reader.short(entry + 2)?;
            let count = reader.long(entry + 4)? as usize;
            let width = match kind {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 | 13 => 4,
                5 | 10 | 12 => 8,
                // Unknown field types are skippable TIFF extensions. They may
                // not replace a field that this decoder needs to interpret.
                _ => 0,
            };
            let length = count
                .checked_mul(width)
                .ok_or("TIFF field length overflow.")?;
            let at = if length <= 4 {
                entry + 8
            } else {
                reader.long(entry + 8)? as usize
            };
            if width != 0 {
                reader.bytes(at, length)?;
            }
            if fields.insert(tag, Field { kind, count, at }).is_some() {
                return Err("TIFF directory repeats a field tag.".into());
            }
        }
        Ok(Self { reader, fields })
    }
    fn values(&self, tag: u16) -> ImageResult<Option<Vec<u32>>> {
        let Some(field) = self.fields.get(&tag) else {
            return Ok(None);
        };
        if field.count > MAX_FIELD_VALUES {
            return Err("TIFF field array exceeds its value limit.".into());
        }
        let mut out = reserved(field.count)?;
        for index in 0..field.count {
            out.push(match field.kind {
                1 => u32::from(self.reader.bytes(field.at + index, 1)?[0]),
                3 => u32::from(self.reader.short(field.at + 2 * index)?),
                4 => self.reader.long(field.at + 4 * index)?,
                _ => return Err(format!("TIFF field {tag} has an unsupported value type.")),
            });
        }
        Ok(Some(out))
    }
    fn scalar(&self, tag: u16, default: Option<u32>) -> ImageResult<u32> {
        match self.values(tag)? {
            Some(values) if values.len() == 1 => Ok(values[0]),
            Some(_) => Err(format!("TIFF field {tag} must contain one value.")),
            None => default.ok_or_else(|| format!("Required TIFF field {tag} is missing.")),
        }
    }
    fn required(&self, tag: u16) -> ImageResult<Vec<u32>> {
        self.values(tag)?
            .ok_or_else(|| format!("Required TIFF field {tag} is missing."))
    }
    fn data(&self, tag: u16) -> ImageResult<Option<&'a [u8]>> {
        let Some(f) = self.fields.get(&tag) else {
            return Ok(None);
        };
        if !matches!(f.kind, 1 | 7) {
            return Err(format!("TIFF field {tag} must contain bytes."));
        }
        Ok(Some(self.reader.bytes(f.at, f.count)?))
    }
    fn rational_is(&self, tag: u16, expected: &[(u32, u32)]) -> ImageResult<bool> {
        let Some(field) = self.fields.get(&tag) else {
            return Ok(true);
        };
        if field.kind != 5 || field.count != expected.len() {
            return Err(format!(
                "TIFF field {tag} requires {} rational values.",
                expected.len()
            ));
        }
        for (index, &(numerator, denominator)) in expected.iter().enumerate() {
            let n = self.reader.long(field.at + 8 * index)?;
            let d = self.reader.long(field.at + 8 * index + 4)?;
            if d == 0 {
                return Err("TIFF rational denominator is zero.".into());
            }
            if u64::from(n) * u64::from(denominator) != u64::from(d) * u64::from(numerator) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn allocation<T: Clone>(count: usize, value: T) -> ImageResult<Vec<T>> {
    let mut out = reserved(count)?;
    out.resize(count, value);
    Ok(out)
}

fn packbits(data: &[u8], expected: usize, work: &mut Work<'_>) -> ImageResult<Vec<u8>> {
    let mut out = reserved(expected)?;
    let mut at = 0;
    while at < data.len() {
        work.charge(1)?;
        let code = data[at] as i8;
        at += 1;
        match code {
            0..=127 => {
                let length = code as usize + 1;
                if length > expected - out.len() {
                    return Err("TIFF PackBits output exceeds its segment.".into());
                }
                let bytes = data
                    .get(at..at + length)
                    .ok_or("Truncated TIFF PackBits literal.")?;
                out.extend_from_slice(bytes);
                at += length;
                work.charge(length)?;
            }
            -127..=-1 => {
                let length = (1 - i16::from(code)) as usize;
                if length > expected - out.len() {
                    return Err("TIFF PackBits run exceeds its segment.".into());
                }
                let byte = *data.get(at).ok_or("Truncated TIFF PackBits run.")?;
                at += 1;
                out.resize(out.len() + length, byte);
                work.charge(length)?;
            }
            -128 => (),
        }
    }
    if out.len() != expected {
        return Err("TIFF PackBits output is shorter than its segment.".into());
    }
    Ok(out)
}

fn lzw(data: &[u8], expected: usize, work: &mut Work<'_>) -> ImageResult<Vec<u8>> {
    let mut out = reserved(expected)?;
    let mut prefixes = [0u16; 4096];
    let mut suffixes = [0u8; 4096];
    let mut stack = [0u8; 4096];
    let mut at = 0usize;
    let mut width = 9usize;
    let mut next = 258usize;
    let mut previous: Option<usize> = None;
    let mut cleared = false;
    loop {
        work.charge(1)?;
        if at + width > data.len() * 8 {
            return Err("TIFF LZW end code is missing.".into());
        }
        let mut code = 0usize;
        for bit in at..at + width {
            code = (code << 1) | usize::from((data[bit / 8] >> (7 - bit % 8)) & 1);
        }
        at += width;
        if code == 256 {
            width = 9;
            next = 258;
            previous = None;
            cleared = true;
            continue;
        }
        if !cleared {
            return Err("TIFF LZW must start with a clear code.".into());
        }
        if code == 257 {
            if out.len() != expected {
                return Err("TIFF LZW output is shorter than its segment.".into());
            }
            if data.len() * 8 - at >= 8 {
                return Err("Trailing bytes after TIFF LZW end code.".into());
            }
            return Ok(out);
        }
        if code > next || code == next && previous.is_none() {
            return Err("TIFF LZW references an undefined dictionary code.".into());
        }
        let special = code == next;
        let mut link = if special { previous.unwrap() } else { code };
        let mut length = 0usize;
        while link >= 256 {
            if link >= next || length >= stack.len() - 1 {
                return Err("Invalid TIFF LZW dictionary chain.".into());
            }
            stack[length] = suffixes[link];
            length += 1;
            link = usize::from(prefixes[link]);
        }
        stack[length] = link as u8;
        length += 1;
        let first = stack[length - 1];
        let output_length = length + usize::from(special);
        if output_length > expected - out.len() {
            return Err("TIFF LZW output exceeds its segment.".into());
        }
        work.charge(output_length)?;
        out.extend(stack[..length].iter().rev());
        if special {
            out.push(first);
        }
        if let Some(old) = previous {
            if next < 4096 {
                prefixes[next] = old as u16;
                suffixes[next] = first;
                next += 1;
                // TIFF's early-change rule: the next code widens immediately
                // after entries 510, 1022 and 2046 are installed.
                if width < 12 && next == (1 << width) - 1 {
                    width += 1;
                }
            }
        }
        previous = Some(code);
    }
}

fn decompress(
    data: &[u8],
    compression: u32,
    expected: usize,
    work: &mut Work<'_>,
) -> ImageResult<Vec<u8>> {
    if expected > MAX_SEGMENT_BYTES {
        return Err("TIFF segment exceeds the 32 MiB decoded limit.".into());
    }
    work.charge(expected)?;
    let out = match compression {
        1 => {
            if data.len() != expected {
                return Err("TIFF uncompressed segment has the wrong byte count.".into());
            }
            let mut out = reserved(expected)?;
            out.extend_from_slice(data);
            out
        }
        5 => lzw(data, expected, work)?,
        8 | 32946 => crate::compression::zlib(data, expected, work.check)?,
        32773 => packbits(data, expected, work)?,
        _ => {
            return Err(format!(
                "TIFF compression {compression} is not implemented."
            ))
        }
    };
    if out.len() != expected {
        return Err("TIFF decoded segment has the wrong byte count.".into());
    }
    Ok(out)
}

// Normative codewords from TIFF 6.0 Tables 1/2 and ITU-T T.6 Tables 1/2/3.
// These are format data, not another codec's implementation. A small prefix
// tree bounds code lookup to thirteen bits instead of scanning the tables.
const FAX_WHITE: &str = "00110101 000111 0111 1000 1011 1100 1110 1111
10011 10100 00111 01000 001000 000011 110100 110101
101010 101011 0100111 0001100 0001000 0010111 0000011 0000100
0101000 0101011 0010011 0100100 0011000 00000010 00000011 00011010
00011011 00010010 00010011 00010100 00010101 00010110 00010111 00101000
00101001 00101010 00101011 00101100 00101101 00000100 00000101 00001010
00001011 01010010 01010011 01010100 01010101 00100100 00100101 01011000
01011001 01011010 01011011 01001010 01001011 00110010 00110011 00110100";
const FAX_BLACK: &str = "0000110111 010 11 10 011 0011 0010 00011
000101 000100 0000100 0000101 0000111 00000100 00000111 000011000
0000010111 0000011000 0000001000 00001100111 00001101000 00001101100 00000110111 00000101000
00000010111 00000011000 000011001010 000011001011 000011001100 000011001101 000001101000 000001101001
000001101010 000001101011 000011010010 000011010011 000011010100 000011010101 000011010110 000011010111
000001101100 000001101101 000011011010 000011011011 000001010100 000001010101 000001010110 000001010111
000001100100 000001100101 000001010010 000001010011 000000100100 000000110111 000000111000 000000100111
000000101000 000001011000 000001011001 000000101011 000000101100 000001011010 000001100110 000001100111";
const FAX_WHITE_MAKEUP: &str = "11011 10010 010111 0110111 00110110 00110111 01100100 01100101
01101000 01100111 011001100 011001101 011010010 011010011 011010100 011010101
011010110 011010111 011011000 011011001 011011010 011011011 010011000 010011001
010011010 011000 010011011";
const FAX_BLACK_MAKEUP: &str = "0000001111 000011001000 000011001001 000001011011
000000110011 000000110100 000000110101 0000001101100 0000001101101 0000001001010
0000001001011 0000001001100 0000001001101 0000001110010 0000001110011 0000001110100
0000001110101 0000001110110 0000001110111 0000001010010 0000001010011 0000001010100
0000001010101 0000001011010 0000001011011 0000001100100 0000001100101";
const FAX_SHARED_MAKEUP: &str = "00000001000 00000001100 00000001101 000000010010
000000010011 000000010100 000000010101 000000010110 000000010111 000000011100
000000011101 000000011110 000000011111";

#[derive(Clone, Copy)]
struct FaxNode {
    next: [u16; 2],
    value: u16,
}
struct FaxTable {
    nodes: Vec<FaxNode>,
}
impl FaxTable {
    fn new(black: bool) -> ImageResult<Self> {
        let empty = FaxNode {
            next: [u16::MAX; 2],
            value: u16::MAX,
        };
        let mut nodes = reserved(1024)?;
        nodes.push(empty);
        for (text, first, increment) in [
            (if black { FAX_BLACK } else { FAX_WHITE }, 0, 1),
            (
                if black {
                    FAX_BLACK_MAKEUP
                } else {
                    FAX_WHITE_MAKEUP
                },
                64,
                64,
            ),
            (FAX_SHARED_MAKEUP, 1792, 64),
        ] {
            for (index, word) in text.split_ascii_whitespace().enumerate() {
                let mut at = 0;
                for bit in word.bytes() {
                    let bit = usize::from(bit == b'1');
                    if nodes[at].value != u16::MAX {
                        return Err("Invalid internal TIFF fax code table.".into());
                    }
                    if nodes[at].next[bit] == u16::MAX {
                        if nodes.len() >= 1024 {
                            return Err("TIFF fax table limit reached.".into());
                        }
                        nodes[at].next[bit] = nodes.len() as u16;
                        nodes.push(empty);
                    }
                    at = usize::from(nodes[at].next[bit]);
                }
                if nodes[at].value != u16::MAX || nodes[at].next != [u16::MAX; 2] {
                    return Err("Invalid internal TIFF fax prefix table.".into());
                }
                nodes[at].value = (first + index * increment) as u16;
            }
        }
        Ok(Self { nodes })
    }
    fn run(
        &self,
        bits: &mut FaxBits<'_>,
        available: usize,
        work: &mut Work<'_>,
    ) -> ImageResult<usize> {
        let mut length = 0usize;
        loop {
            let mut node = 0usize;
            loop {
                work.charge(1)?;
                let next = self.nodes[node].next[usize::from(bits.bit()?)];
                if next == u16::MAX {
                    return Err("Invalid TIFF CCITT run code.".into());
                }
                node = usize::from(next);
                let value = self.nodes[node].value;
                if value != u16::MAX {
                    length += usize::from(value);
                    if length > available {
                        return Err("TIFF CCITT run exceeds its row.".into());
                    }
                    if value < 64 {
                        return Ok(length);
                    }
                    break;
                }
            }
        }
    }
}

struct FaxBits<'a> {
    data: &'a [u8],
    at: usize,
    reversed: bool,
}
impl FaxBits<'_> {
    fn bit(&mut self) -> ImageResult<u8> {
        let byte = *self
            .data
            .get(self.at / 8)
            .ok_or("Truncated TIFF CCITT data.")?;
        let shift = if self.reversed {
            self.at % 8
        } else {
            7 - self.at % 8
        };
        self.at += 1;
        Ok((byte >> shift) & 1)
    }
    fn eol(&mut self, aligned: bool) -> ImageResult<()> {
        let mut zeros = 0usize;
        while self.bit()? == 0 {
            zeros += 1;
            if zeros > 8192 {
                return Err("TIFF CCITT fill sequence exceeds its limit.".into());
            }
        }
        if zeros < 11 || aligned && !self.at.is_multiple_of(8) {
            return Err("Invalid TIFF CCITT end-of-line code or alignment.".into());
        }
        Ok(())
    }
    fn mode(&mut self) -> ImageResult<i32> {
        let mut code = 0;
        for length in 1..=7 {
            code = (code << 1) | i32::from(self.bit()?);
            match (length, code) {
                (1, 1) => return Ok(0),
                (3, 3) => return Ok(1),
                (3, 2) => return Ok(-1),
                (3, 1) => return Ok(4), // Horizontal.
                (4, 1) => return Ok(5), // Pass.
                (6, 3) => return Ok(2),
                (6, 2) => return Ok(-2),
                (7, 3) => return Ok(3),
                (7, 2) => return Ok(-3),
                (7, 1) => {
                    return Err("TIFF CCITT uncompressed extension is not implemented.".into())
                }
                _ => (),
            }
        }
        Err("Invalid TIFF CCITT two-dimensional mode.".into())
    }
    fn remaining_zero(&self) -> bool {
        let mut bits = Self {
            data: self.data,
            at: self.at,
            reversed: self.reversed,
        };
        while bits.at < bits.data.len() * 8 && !bits.at.is_multiple_of(8) {
            if bits.bit() != Ok(0) {
                return false;
            }
        }
        bits.data[bits.at / 8..].iter().all(|&byte| byte == 0)
    }
}

#[derive(Clone, Copy)]
struct FaxMode {
    compression: u32,
    options: u32,
    reversed: bool,
}

fn fax_decode(
    data: &[u8],
    width: usize,
    height: usize,
    mode: FaxMode,
    tables: &[FaxTable; 2],
    work: &mut Work<'_>,
) -> ImageResult<Vec<u8>> {
    let stride = width.div_ceil(8);
    let expected = stride
        .checked_mul(height)
        .ok_or("TIFF CCITT raster overflow.")?;
    if expected > MAX_SEGMENT_BYTES {
        return Err("TIFF CCITT segment exceeds 32 MiB.".into());
    }
    work.charge(data.len() + expected)?;
    let mut out = allocation(expected, 0u8)?;
    let mut row = allocation(width, 0u8)?;
    let mut reference = reserved(width + 2)?;
    reference.extend([width, width]);
    let mut bits = FaxBits {
        data,
        at: 0,
        reversed: mode.reversed,
    };
    for y in 0..height {
        (work.check)()?;
        work.charge(width)?;
        row.fill(0);
        let one_dimensional = match mode.compression {
            2 => true,
            3 => {
                bits.eol(mode.options & 4 != 0)?;
                let one = mode.options & 1 == 0 || bits.bit()? != 0;
                if y == 0 && !one {
                    return Err(
                        "TIFF Group 3 segment must start with a one-dimensional row.".into(),
                    );
                }
                one
            }
            _ => false,
        };
        let mut at = 0usize;
        let mut color = 0usize;
        let mut initial = true;
        let mut steps = 0usize;
        while at < width {
            steps += 1;
            if steps > width * 2 + 16 {
                return Err("TIFF CCITT row exceeds its coding limit.".into());
            }
            work.charge(8)?;
            if one_dimensional {
                let end = at + tables[color].run(&mut bits, width - at, work)?;
                row[at..end].fill(color as u8);
                at = end;
                color ^= 1;
            } else {
                let mut index = if initial {
                    0
                } else {
                    reference.partition_point(|&position| position <= at)
                };
                if index % 2 != color {
                    index += 1;
                }
                let b1 = reference.get(index).copied().unwrap_or(width);
                let b2 = reference.get(index + 1).copied().unwrap_or(width);
                match bits.mode()? {
                    4 => {
                        let first = at + tables[color].run(&mut bits, width - at, work)?;
                        let end = first + tables[color ^ 1].run(&mut bits, width - first, work)?;
                        if end == at {
                            return Err("TIFF CCITT horizontal pair makes no progress.".into());
                        }
                        row[at..first].fill(color as u8);
                        row[first..end].fill((color ^ 1) as u8);
                        at = end;
                    }
                    5 => {
                        if b2 <= at {
                            return Err("TIFF CCITT pass mode makes no progress.".into());
                        }
                        row[at..b2].fill(color as u8);
                        at = b2;
                    }
                    delta => {
                        let end = b1 as i32 + delta;
                        if end < at as i32 || end > width as i32 {
                            return Err("TIFF CCITT vertical position is outside its row.".into());
                        }
                        row[at..end as usize].fill(color as u8);
                        at = end as usize;
                        color ^= 1;
                    }
                }
            }
            initial = false;
        }
        reference.clear();
        let mut previous = 0u8;
        for (x, &value) in row.iter().enumerate() {
            if value != previous {
                reference.push(x);
                previous = value;
            }
            out[y * stride + x / 8] |= value << (7 - x % 8);
        }
        reference.extend([width, width]);
        if mode.compression == 2 {
            bits.at = bits.at.div_ceil(8) * 8;
        }
    }
    if mode.compression == 4 {
        for expected in (0..24)
            .rev()
            .map(|shift| ((0x001001u32 >> shift) & 1) as u8)
        {
            if bits.bit()? != expected {
                return Err("TIFF Group 4 end-of-block code is missing.".into());
            }
        }
        // TIFF explicitly permits padding beyond the final EOFB byte.
    } else if mode.compression == 3 {
        // RTC is optional for TIFF; if present it contains up to six EOLs.
        for _ in 0..6 {
            if bits.remaining_zero() {
                break;
            }
            bits.eol(mode.options & 4 != 0)?;
            if mode.options & 1 != 0 && bits.bit()? != 1 {
                return Err("Invalid TIFF Group 3 RTC tag.".into());
            }
        }
        if !bits.remaining_zero() {
            return Err("Unexpected data after TIFF Group 3 rows.".into());
        }
    } else if bits.at != data.len() * 8 {
        return Err("Unexpected data after TIFF CCITT RLE rows.".into());
    }
    Ok(out)
}

fn oriented(x: usize, y: usize, width: usize, height: usize, orientation: u32) -> (usize, usize) {
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

struct Raster {
    width: usize,
    height: usize,
    samples: usize,
    depth: usize,
    photo: u32,
    alpha: u32,
    orientation: u32,
    palette: Vec<u32>,
}
impl Raster {
    fn render(&self, samples: &[u16], work: &mut Work<'_>) -> ImageResult<Bitmap> {
        let (width, height) = if self.orientation >= 5 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        let mut pixels = allocation(width * height, 0u32)?;
        let maximum = (1u32 << self.depth) - 1;
        let scale = |v: u32| ((v * 255 + maximum / 2) / maximum) as u8;
        for y in 0..self.height {
            (work.check)()?;
            work.charge(self.width * self.samples)?;
            for x in 0..self.width {
                let at = (y * self.width + x) * self.samples;
                let values = &samples[at..at + self.samples];
                let alpha = if self.alpha != 0 {
                    u32::from(values[self.samples - 1])
                } else {
                    maximum
                };
                let unassociate = |v: u32| {
                    if self.alpha == 1 {
                        (v * maximum + alpha / 2)
                            .checked_div(alpha)
                            .unwrap_or(0)
                            .min(maximum)
                    } else {
                        v
                    }
                };
                let rgb = match self.photo {
                    0 | 1 => {
                        let mut gray = u32::from(values[0]);
                        if self.photo == 0 {
                            gray = maximum - gray;
                        }
                        [scale(unassociate(gray)); 3]
                    }
                    2 => {
                        [values[0], values[1], values[2]].map(|v| scale(unassociate(u32::from(v))))
                    }
                    3 => {
                        let index = usize::from(values[0]);
                        let length = 1usize << self.depth;
                        [
                            self.palette[index],
                            self.palette[length + index],
                            self.palette[2 * length + index],
                        ]
                        .map(|v| ((v * 255 + 32767) / 65535) as u8)
                    }
                    5 => {
                        let black = unassociate(u32::from(values[3]));
                        [values[0], values[1], values[2]].map(|v| {
                            let ink = unassociate(u32::from(v));
                            let reflected = (u64::from(maximum - ink) * u64::from(maximum - black)
                                + u64::from(maximum / 2))
                                / u64::from(maximum);
                            scale(reflected as u32)
                        })
                    }
                    _ => unreachable!(),
                };
                let (tx, ty) = oriented(x, y, self.width, self.height, self.orientation);
                pixels[ty * width + tx] = pixel(rgb[0], rgb[1], rgb[2], scale(alpha));
            }
        }
        Ok(Bitmap {
            width,
            height,
            pixels,
        })
    }
}

/// Assemble the optional shared JPEG tables without interpreting or importing
/// another decoder. The JPEG reader validates every marker/table afterward.
fn jpeg_stream(tables: Option<&[u8]>, segment: &[u8]) -> ImageResult<Vec<u8>> {
    if !segment.starts_with(&[255, 0xd8]) || !segment.ends_with(&[255, 0xd9]) {
        return Err("TIFF JPEG segment requires complete SOI/EOI markers.".into());
    }
    let Some(tables) = tables else {
        return Ok(segment.to_vec());
    };
    if !tables.starts_with(&[255, 0xd8]) || !tables.ends_with(&[255, 0xd9]) {
        return Err("TIFF JPEG tables require complete SOI/EOI markers.".into());
    }
    let mut at = 2;
    while at < tables.len() - 2 {
        if tables.get(at) != Some(&255) {
            return Err("Invalid TIFF JPEG tables marker.".into());
        }
        while tables.get(at) == Some(&255) {
            at += 1;
        }
        let marker = *tables.get(at).ok_or("Truncated TIFF JPEG tables.")?;
        if !matches!(marker, 0xc4 | 0xdb | 0xdd | 0xe0..=0xef | 0xfe) {
            return Err("TIFF shared JPEG tables contain an image or unsupported marker.".into());
        }
        at += 1;
        let length = tables
            .get(at..at + 2)
            .ok_or("Truncated TIFF JPEG table length.")?;
        let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
        if length < 2 || at + length > tables.len() - 2 {
            return Err("Invalid TIFF JPEG table length.".into());
        }
        at += length;
    }
    if at != tables.len() - 2 {
        return Err("TIFF JPEG tables have trailing data.".into());
    }
    let size = tables
        .len()
        .checked_add(segment.len())
        .and_then(|n| n.checked_sub(4))
        .ok_or("TIFF JPEG stream overflow.")?;
    if size > MAX_INPUT_BYTES {
        return Err("TIFF combined JPEG stream exceeds 32 MiB.".into());
    }
    let mut out = reserved(size)?;
    out.extend_from_slice(&tables[..tables.len() - 2]);
    out.extend_from_slice(&segment[2..]);
    Ok(out)
}

pub(super) fn decode(data: &[u8], check: Check<'_>) -> ImageResult<Bitmap> {
    check()?;
    let directory = Directory::parse(data, check)?;
    let width = directory.scalar(256, None)? as usize;
    let height = directory.scalar(257, None)? as usize;
    let count = dimensions(width, height)?;
    let photo = directory.scalar(262, None)?;
    let compression = directory.scalar(259, Some(1))?;
    if !matches!(compression, 1..=5 | 7 | 8 | 32946 | 32773) {
        return Err(format!("TIFF compression {compression} is not implemented; supported: none, CCITT, LZW, JPEG, Deflate and PackBits."));
    }
    let base_samples = match photo {
        0 | 1 | 3 => 1,
        2 => 3,
        5 => 4,
        6 if compression == 7 => 3,
        _ => {
            return Err(format!(
                "TIFF photometric interpretation {photo} is not implemented."
            ))
        }
    };
    let samples = directory.scalar(277, Some(1))? as usize;
    if samples < base_samples || samples > base_samples + 1 || samples > 5 {
        return Err("TIFF supports its color components and at most one extra sample.".into());
    }
    let depths = directory.values(258)?.unwrap_or_else(|| vec![1; samples]);
    if depths.len() != samples || depths.iter().any(|&v| v != depths[0]) {
        return Err("TIFF requires matching BitsPerSample values for each component.".into());
    }
    let depth = depths[0] as usize;
    if !matches!(depth, 1 | 2 | 4 | 8 | 16) || base_samples > 1 && depth < 8 {
        return Err(
            "TIFF supports 1/2/4/8/16-bit integer gray/palette and 8/16-bit color samples.".into(),
        );
    }
    let formats = directory.values(339)?.unwrap_or_else(|| vec![1; samples]);
    if !matches!(formats.len(), 1) && formats.len() != samples || formats.iter().any(|&v| v != 1) {
        return Err(
            "TIFF signed, floating-point and noninteger samples are not implemented.".into(),
        );
    }
    let extras = directory.values(338)?.unwrap_or_default();
    if extras.len() != samples - base_samples || extras.iter().any(|&v| v > 2) {
        return Err("TIFF ExtraSamples does not describe its extra component.".into());
    }
    let alpha = extras.first().copied().unwrap_or(0);
    if photo == 3 && alpha == 1 {
        return Err("TIFF associated alpha with palette indices is undefined.".into());
    }
    if photo == 5 && directory.scalar(332, Some(1))? != 1 {
        return Err("TIFF separated color supports CMYK inks only.".into());
    }
    if photo == 5 && directory.scalar(334, Some(4))? != 4 {
        return Err("TIFF CMYK requires four inks.".into());
    }
    if directory.fields.contains_key(&336) {
        return Err("TIFF custom ink DotRange is not implemented.".into());
    }
    let planar = directory.scalar(284, Some(1))?;
    let predictor = directory.scalar(317, Some(1))?;
    let fill = directory.scalar(266, Some(1))?;
    let orientation = directory.scalar(274, Some(1))?;
    if !matches!(planar, 1 | 2)
        || !matches!(predictor, 1 | 2)
        || !matches!(fill, 1 | 2)
        || !(1..=8).contains(&orientation)
    {
        return Err("Unsupported TIFF planar, predictor, fill-order or orientation value.".into());
    }
    if predictor == 2 && !matches!(compression, 5 | 8 | 32946) {
        return Err("TIFF horizontal prediction requires LZW or Deflate compression.".into());
    }
    let fax = matches!(compression, 2..=4);
    if fill == 2 && (depth != 1 || !matches!(compression, 1..=4)) {
        return Err("TIFF reversed bit order requires uncompressed or CCITT 1-bit data.".into());
    }
    let fax_options = match compression {
        3 => directory.scalar(292, Some(0))?,
        4 => directory.scalar(293, Some(0))?,
        _ => 0,
    };
    if fax && (depth != 1 || samples != 1 || photo > 1 || predictor != 1) {
        return Err("TIFF CCITT requires one 1-bit grayscale channel without prediction.".into());
    }
    if compression == 3 && fax_options & !5 != 0 || compression == 4 && fax_options != 0 {
        return Err(
            "TIFF CCITT uncompressed extensions or unknown options are not implemented.".into(),
        );
    }
    let palette = if photo == 3 {
        let p = directory.required(320)?;
        if p.len() != 3 * (1usize << depth) || p.iter().any(|&v| v > 65535) {
            return Err("TIFF ColorMap must contain three complete 16-bit palette planes.".into());
        }
        p
    } else {
        Vec::new()
    };
    let raster = Raster {
        width,
        height,
        samples,
        depth,
        photo,
        alpha,
        orientation,
        palette,
    };
    let tiled = directory.fields.contains_key(&322)
        || directory.fields.contains_key(&323)
        || directory.fields.contains_key(&324)
        || directory.fields.contains_key(&325);
    let (block_width, block_height, offsets, lengths) = if tiled {
        if directory.fields.contains_key(&273) || directory.fields.contains_key(&279) {
            return Err("TIFF cannot mix strip and tile storage.".into());
        }
        let w = directory.scalar(322, None)? as usize;
        let h = directory.scalar(323, None)? as usize;
        if w == 0
            || h == 0
            || w > 16384
            || h > 16384
            || !w.is_multiple_of(16)
            || !h.is_multiple_of(16)
        {
            return Err("TIFF tile dimensions must be bounded nonzero multiples of 16.".into());
        }
        (w, h, directory.required(324)?, directory.required(325)?)
    } else {
        let rows = directory.scalar(278, Some(u32::MAX))? as usize;
        if rows == 0 {
            return Err("TIFF RowsPerStrip must be nonzero.".into());
        }
        (
            width,
            rows.min(height),
            directory.required(273)?,
            directory.required(279)?,
        )
    };
    let across = width.div_ceil(block_width);
    let down = height.div_ceil(block_height);
    let per_plane = across * down;
    let planes = if planar == 2 { samples } else { 1 };
    let segments = per_plane
        .checked_mul(planes)
        .ok_or("TIFF segment count overflow.")?;
    // Geometry and the admitted field arrays bound this traversal. Each segment
    // also retains its source-extent, cancellation, decoded-byte and work checks;
    // a separate record ceiling rejects valid small planar images unnecessarily.
    if offsets.len() != segments || lengths.len() != segments {
        return Err("TIFF segment tables have the wrong count.".into());
    }
    let jpeg = compression == 7;
    if jpeg
        && (depth != 8
            || planar != 1
            || samples != base_samples
            || predictor != 1
            || fill != 1
            || photo == 3)
    {
        return Err(
            "TIFF JPEG requires chunky 8-bit color without extra samples or prediction.".into(),
        );
    }
    let mut jpeg_subsampling = None;
    if jpeg && photo == 6 {
        if !directory.rational_is(529, &[(299, 1000), (587, 1000), (114, 1000)])?
            || !directory.rational_is(
                532,
                &[(0, 1), (255, 1), (128, 1), (255, 1), (128, 1), (255, 1)],
            )?
        {
            return Err(
                "TIFF JPEG custom YCbCr coefficients/reference levels are not implemented.".into(),
            );
        }
        let subsampling = directory.values(530)?.unwrap_or_else(|| vec![2, 2]);
        if subsampling.len() != 2 || subsampling.iter().any(|&v| !matches!(v, 1 | 2 | 4)) {
            return Err("TIFF JPEG YCbCr subsampling is invalid.".into());
        }
        if directory.scalar(531, Some(1))? != 1 {
            return Err("TIFF JPEG cosited chroma is not implemented.".into());
        }
        jpeg_subsampling = Some((subsampling[0] as usize, subsampling[1] as usize));
    }
    let table = if jpeg { directory.data(347)? } else { None };
    let sample_count = count
        .checked_mul(samples)
        .ok_or("TIFF raster size overflow.")?;
    if sample_count > MAX_RASTER_BYTES / 2 {
        return Err("TIFF sample raster exceeds 64 MiB.".into());
    }
    let mut values = if jpeg {
        Vec::new()
    } else {
        allocation(sample_count, 0u16)?
    };
    let mut jpeg_pixels = if jpeg {
        allocation(count, 0u32)?
    } else {
        Vec::new()
    };
    let mut work = Work {
        remaining: MAX_WORK,
        since_check: 0,
        check,
    };
    let mut total_bytes = 0usize;
    let fax_tables = if fax {
        Some([FaxTable::new(false)?, FaxTable::new(true)?])
    } else {
        None
    };
    for index in 0..segments {
        check()?;
        let plane = index / per_plane;
        let tile = index % per_plane;
        let left = (tile % across) * block_width;
        let top = (tile / across) * block_height;
        let encoded_height = if tiled {
            block_height
        } else {
            block_height.min(height - top)
        };
        let active_width = block_width.min(width - left);
        let active_height = encoded_height.min(height - top);
        let channels = if planar == 2 { 1 } else { samples };
        let row_bytes = (block_width * channels * depth).div_ceil(8);
        let decoded_bytes = row_bytes
            .checked_mul(encoded_height)
            .ok_or("TIFF segment size overflow.")?;
        total_bytes = total_bytes
            .checked_add(decoded_bytes)
            .filter(|&n| n <= MAX_RASTER_BYTES)
            .ok_or("TIFF padded/planar raster exceeds 64 MiB.")?;
        let data = directory
            .reader
            .bytes(offsets[index] as usize, lengths[index] as usize)?;
        if jpeg {
            work.charge(data.len() + table.map_or(0, <[u8]>::len) + decoded_bytes)?;
            let stream = jpeg_stream(table, data)?;
            let bitmap = crate::jpeg::decode_tiff(&stream, photo as u16, jpeg_subsampling, check)?;
            if bitmap.width != block_width || bitmap.height != encoded_height {
                return Err("TIFF JPEG dimensions do not match its segment.".into());
            }
            for y in 0..active_height {
                check()?;
                let destination = (top + y) * width + left;
                jpeg_pixels[destination..destination + active_width].copy_from_slice(
                    &bitmap.pixels[y * bitmap.width..y * bitmap.width + active_width],
                );
            }
            continue;
        }
        let decoded = if let Some(tables) = &fax_tables {
            fax_decode(
                data,
                block_width,
                encoded_height,
                FaxMode {
                    compression,
                    options: fax_options,
                    reversed: fill == 2,
                },
                tables,
                &mut work,
            )?
        } else {
            decompress(data, compression, decoded_bytes, &mut work)?
        };
        let maximum = (1u32 << depth) - 1;
        for y in 0..encoded_height {
            check()?;
            work.charge(block_width * channels)?;
            let row = &decoded[y * row_bytes..(y + 1) * row_bytes];
            let mut previous = [0u32; 5];
            for x in 0..block_width {
                for (channel, last) in previous.iter_mut().enumerate().take(channels) {
                    let sample = x * channels + channel;
                    let mut value = if depth == 16 {
                        let b = &row[sample * 2..sample * 2 + 2];
                        u32::from(if directory.reader.little {
                            u16::from_le_bytes([b[0], b[1]])
                        } else {
                            u16::from_be_bytes([b[0], b[1]])
                        })
                    } else {
                        let bit = sample * depth;
                        let byte = if fill == 2 && !fax {
                            row[bit / 8].reverse_bits()
                        } else {
                            row[bit / 8]
                        };
                        u32::from(byte >> (8 - depth - bit % 8)) & maximum
                    };
                    if predictor == 2 {
                        value = (value + *last) & maximum;
                        *last = value;
                    }
                    if x < active_width && y < active_height {
                        let target = ((top + y) * width + left + x) * samples
                            + if planar == 2 { plane } else { channel };
                        values[target] = value as u16;
                    }
                }
            }
        }
    }
    if jpeg {
        if orientation == 1 {
            return Ok(Bitmap {
                width,
                height,
                pixels: jpeg_pixels,
            });
        }
        let (out_width, out_height) = if orientation >= 5 {
            (height, width)
        } else {
            (width, height)
        };
        let mut output = allocation(count, 0u32)?;
        for y in 0..height {
            check()?;
            for x in 0..width {
                let (tx, ty) = oriented(x, y, width, height, orientation);
                output[ty * out_width + tx] = jpeg_pixels[y * width + x];
            }
        }
        Ok(Bitmap {
            width: out_width,
            height: out_height,
            pixels: output,
        })
    } else {
        raster.render(&values, &mut work)
    }
}

#[cfg(test)]
#[path = "image_tiff_capacity_tests.rs"]
mod capacity_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(little: bool, compression: u16, payload: &[u8]) -> Vec<u8> {
        let short = |v: u16| {
            if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            }
        };
        let long = |v: u32| {
            if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            }
        };
        let fields = [
            (256, 2u32),
            (257, 1),
            (258, 8),
            (259, u32::from(compression)),
            (262, 1),
            (273, 122),
            (277, 1),
            (278, 1),
            (279, payload.len() as u32),
        ];
        let mut bytes = if little {
            b"II".to_vec()
        } else {
            b"MM".to_vec()
        };
        bytes.extend(short(42));
        bytes.extend(long(8));
        bytes.extend(short(fields.len() as u16));
        for (tag, value) in fields {
            bytes.extend(short(tag));
            bytes.extend(short(4));
            bytes.extend(long(1));
            bytes.extend(long(value));
        }
        bytes.extend(long(0));
        bytes.extend(payload);
        bytes
    }

    #[test]
    fn opposite_byte_orders_and_packbits_decode_the_same_pixels() {
        for little in [false, true] {
            let plain = decode(&gray(little, 1, &[23, 245]), &|| Ok(())).unwrap();
            let packed = decode(&gray(little, 32773, &[1, 23, 245]), &|| Ok(())).unwrap();
            assert_eq!(plain.pixels, [0xff171717, 0xfff5f5f5]);
            assert_eq!(packed.pixels, plain.pixels);
        }
    }

    #[test]
    fn small_lzw_clear_literal_end_and_bad_codes() {
        let codes = [256u16, 23, 245, 257];
        let mut bytes = vec![0; 5];
        for (i, code) in codes.iter().enumerate() {
            for bit in 0..9 {
                bytes[(i * 9 + bit) / 8] |=
                    (((code >> (8 - bit)) & 1) as u8) << (7 - (i * 9 + bit) % 8);
            }
        }
        assert_eq!(
            decode(&gray(true, 5, &bytes), &|| Ok(())).unwrap().pixels,
            [0xff171717, 0xfff5f5f5]
        );
        bytes[0] = 0;
        assert!(decode(&gray(true, 5, &bytes), &|| Ok(()))
            .unwrap_err()
            .contains("clear"));
    }

    #[test]
    fn authored_ccitt_rows_and_missing_end_codes_are_checked() {
        let packed = |bits: &str| {
            let mut bytes = vec![0u8; bits.len().div_ceil(8)];
            for (index, bit) in bits.bytes().enumerate() {
                bytes[index / 8] |= u8::from(bit == b'1') << (7 - index % 8);
            }
            bytes
        };
        let bilevel = |compression, payload: &[u8]| {
            let mut bytes = gray(true, compression, payload);
            bytes[42..46].copy_from_slice(&1u32.to_le_bytes()); // BitsPerSample.
            bytes[66..70].copy_from_slice(&0u32.to_le_bytes()); // WhiteIsZero.
            bytes
        };
        // W(1), B(1), with each container's specified framing.
        for (compression, codes) in [
            (2, "000111010"),
            (3, "000000000001000111010"),
            (4, "001000111010000000000001000000000001"),
        ] {
            let data = bilevel(compression, &packed(codes));
            assert_eq!(
                decode(&data, &|| Ok(())).unwrap().pixels,
                [0xffffffff, 0xff000000]
            );
        }
        // Second Group 4 row repeats the first using two vertical-zero codes.
        let mut data = bilevel(4, &packed("00100011101011000000000001000000000001"));
        data[30..34].copy_from_slice(&2u32.to_le_bytes());
        data[102..106].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            decode(&data, &|| Ok(())).unwrap().pixels,
            [0xffffffff, 0xff000000, 0xffffffff, 0xff000000]
        );
        assert!(decode(&bilevel(2, &packed("1000")), &|| Ok(()))
            .unwrap_err()
            .contains("exceeds its row"));
        assert!(decode(&bilevel(3, &packed("000111010")), &|| Ok(()))
            .unwrap_err()
            .contains("end-of-line"));
        assert!(decode(&bilevel(4, &packed("001000111010")), &|| Ok(())).is_err());
        assert!(decode(&bilevel(4, &packed("0000001111")), &|| Ok(()))
            .unwrap_err()
            .contains("extension"));
    }

    #[test]
    fn truncation_overflow_unknown_modes_and_cancellation_are_errors() {
        let bytes = gray(true, 1, &[23, 245]);
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &|| Ok(())).is_err());
        }
        assert!(decode(&gray(true, 32773, &[127, 1]), &|| Ok(())).is_err());
        assert!(decode(&gray(true, 6, &[0]), &|| Ok(()))
            .unwrap_err()
            .contains("compression"));
        assert!(decode(b"II+\0\x08\0\0\0", &|| Ok(()))
            .unwrap_err()
            .contains("BigTIFF"));
        assert_eq!(
            decode(&bytes, &|| Err("cancelled".into())).unwrap_err(),
            "cancelled"
        );
        let mut huge = bytes;
        huge[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&huge, &|| Ok(())).is_err());
    }
}
