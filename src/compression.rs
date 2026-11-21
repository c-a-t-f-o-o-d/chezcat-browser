//! Our bounded DEFLATE decoder, shared by PNG and HTTP content decoding.
//! The bit/Huffman code originated in this project's PNG decoder. Wrappers
//! follow RFC 1950, RFC 1951 and RFC 1952; no library implementation is used.

pub const MAX_INPUT_BYTES: usize = 512 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 512 * 1024 * 1024;
const CHECK_INTERVAL: usize = 8192;

type DecodeResult<T> = Result<T, String>;

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 0 {
                value >> 1
            } else {
                (value >> 1) ^ 0xedb8_8320
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

fn crc_update(mut checksum: u32, data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = crc_table();
    for &byte in data {
        checksum = (checksum >> 8) ^ TABLE[((checksum as u8) ^ byte) as usize];
    }
    checksum
}

// Uncancelable convenience used by the independent fixture/probe programs.
// Production decoding uses crc32_checked so large chunks remain cancelable.
#[cfg_attr(not(test), allow(dead_code))]
pub fn crc32(data: &[u8]) -> u32 {
    !crc_update(u32::MAX, data)
}

pub(crate) fn crc32_checked(
    data: &[u8],
    check: &dyn Fn() -> DecodeResult<()>,
) -> DecodeResult<u32> {
    let mut checksum = u32::MAX;
    for chunk in data.chunks(CHECK_INTERVAL) {
        check()?;
        checksum = crc_update(checksum, chunk);
    }
    Ok(!checksum)
}

fn adler32_checked(data: &[u8], check: &dyn Fn() -> DecodeResult<()>) -> DecodeResult<u32> {
    let mut first = 1u32;
    let mut second = 0u32;
    // Reducing every 4096 bytes keeps both sums within u32, including 0xff input.
    for block in data.chunks(4096) {
        check()?;
        for &byte in block {
            first += u32::from(byte);
            second += first;
        }
        first %= 65_521;
        second %= 65_521;
    }
    Ok((second << 16) | first)
}

#[cfg(test)]
pub fn adler32(data: &[u8]) -> u32 {
    adler32_checked(data, &|| Ok(())).unwrap()
}

struct Bits<'a> {
    data: &'a [u8],
    position: usize,
    pending: u64,
    count: u8,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            pending: 0,
            count: 0,
        }
    }

    fn read(&mut self, count: u8) -> DecodeResult<u32> {
        if count > 16 {
            return Err("Invalid compressed bit count.".into());
        }
        while self.count < count {
            let byte = *self
                .data
                .get(self.position)
                .ok_or("Truncated DEFLATE stream.")?;
            self.pending |= u64::from(byte) << self.count;
            self.position += 1;
            self.count += 8;
        }
        let value = self.pending & ((1u64 << count) - 1);
        self.pending >>= count;
        self.count -= count;
        Ok(value as u32)
    }

    fn align(&mut self) {
        // read() never looks ahead by a complete unused byte.
        self.pending = 0;
        self.count = 0;
    }

    fn bytes(&mut self, count: usize) -> DecodeResult<&'a [u8]> {
        if self.count != 0 {
            return Err("Unaligned stored DEFLATE block.".into());
        }
        let end = self
            .position
            .checked_add(count)
            .filter(|&n| n <= self.data.len())
            .ok_or("Truncated stored DEFLATE block.")?;
        let result = &self.data[self.position..end];
        self.position = end;
        Ok(result)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TreeKind {
    Codes,
    Literals,
    Distances,
}

struct Huffman {
    counts: [usize; 16],
    first_code: [u32; 16],
    first_index: [usize; 16],
    symbols: Vec<u16>,
    maximum: usize,
}

impl Huffman {
    fn new(lengths: &[u8], kind: TreeKind) -> DecodeResult<Self> {
        let mut counts = [0usize; 16];
        let mut maximum = 0usize;
        for &length in lengths {
            if length > 15 {
                return Err("Invalid Huffman code length.".into());
            }
            if length > 0 {
                counts[length as usize] += 1;
                maximum = maximum.max(length as usize);
            }
        }
        let used: usize = counts.iter().sum();
        if used == 0 && kind != TreeKind::Distances {
            return Err("Empty Huffman tree.".into());
        }
        let mut available = 1i32;
        for &count in &counts[1..] {
            available = available * 2 - count as i32;
            if available < 0 {
                return Err("Over-subscribed Huffman tree.".into());
            }
        }
        if used > 0 && available != 0 && !(used == 1 && maximum == 1 && kind != TreeKind::Codes) {
            return Err("Incomplete Huffman tree.".into());
        }
        let mut first_code = [0u32; 16];
        let mut first_index = [0usize; 16];
        let mut code = 0u32;
        let mut index = 0usize;
        for length in 1..16 {
            code = (code + counts[length - 1] as u32) << 1;
            first_code[length] = code;
            first_index[length] = index;
            index += counts[length];
        }
        let mut symbols = vec![0u16; used];
        let mut offsets = first_index;
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                let index = &mut offsets[length as usize];
                symbols[*index] = symbol as u16;
                *index += 1;
            }
        }
        Ok(Self {
            counts,
            first_code,
            first_index,
            symbols,
            maximum,
        })
    }

    fn symbol(&self, bits: &mut Bits<'_>) -> DecodeResult<u16> {
        let mut code = 0u32;
        // Huffman code bits arrive most-significant first, even though the
        // enclosing DEFLATE bytes and numeric fields are consumed low-bit first.
        for length in 1..=self.maximum {
            code = (code << 1) | bits.read(1)?;
            let first = self.first_code[length];
            if code >= first && (code - first) < self.counts[length] as u32 {
                return Ok(self.symbols[self.first_index[length] + (code - first) as usize]);
            }
        }
        Err("Invalid Huffman code in DEFLATE stream.".into())
    }
}

fn fixed_trees() -> DecodeResult<(Huffman, Huffman)> {
    let mut lengths = [8u8; 288];
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    Ok((
        Huffman::new(&lengths, TreeKind::Literals)?,
        Huffman::new(&[5u8; 32], TreeKind::Distances)?,
    ))
}

fn dynamic_trees(bits: &mut Bits<'_>) -> DecodeResult<(Huffman, Huffman)> {
    let literal_count = bits.read(5)? as usize + 257;
    let distance_count = bits.read(5)? as usize + 1;
    let code_count = bits.read(4)? as usize + 4;
    if literal_count > 286 {
        return Err("Invalid dynamic literal-code count.".into());
    }
    let order = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut code_lengths = [0u8; 19];
    for &index in &order[..code_count] {
        code_lengths[index] = bits.read(3)? as u8;
    }
    let tree = Huffman::new(&code_lengths, TreeKind::Codes)?;
    let total = literal_count + distance_count;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        let symbol = tree.symbol(bits)?;
        let (value, repetitions) = match symbol {
            0..=15 => (symbol as u8, 1),
            16 => (
                *lengths
                    .last()
                    .ok_or("A repeated code length has no predecessor.")?,
                bits.read(2)? as usize + 3,
            ),
            17 => (0, bits.read(3)? as usize + 3),
            18 => (0, bits.read(7)? as usize + 11),
            _ => return Err("Invalid code-length symbol.".into()),
        };
        if repetitions > total - lengths.len() {
            return Err("Repeated Huffman lengths exceed their declared count.".into());
        }
        lengths.resize(lengths.len() + repetitions, value);
    }
    if lengths[256] == 0 {
        return Err("DEFLATE tree has no end-of-block symbol.".into());
    }
    Ok((
        Huffman::new(&lengths[..literal_count], TreeKind::Literals)?,
        Huffman::new(&lengths[literal_count..], TreeKind::Distances)?,
    ))
}

struct Output<'a> {
    bytes: Vec<u8>,
    limit: usize,
    next_check: usize,
    check: &'a dyn Fn() -> DecodeResult<()>,
}

// Validate lengths independently of a backing slice, so boundary and overflow
// controls need no maximum-sized input allocation. The 512 MiB ceiling also
// keeps Vec/isize and gzip's u32 member-size representations in range on x86.
fn preflight(input_len: usize, limit: usize) -> DecodeResult<()> {
    if input_len > MAX_INPUT_BYTES {
        return Err("Compressed input exceeds the 512 MiB limit.".into());
    }
    if limit > MAX_OUTPUT_BYTES {
        return Err("Decompression output limit exceeds 512 MiB.".into());
    }
    Ok(())
}

impl<'a> Output<'a> {
    fn new(
        data: &[u8],
        limit: usize,
        check: &'a dyn Fn() -> DecodeResult<()>,
    ) -> DecodeResult<Self> {
        check()?;
        preflight(data.len(), limit)?;
        Ok(Self {
            bytes: Vec::new(),
            limit,
            next_check: CHECK_INTERVAL,
            check,
        })
    }

    fn room(&mut self, additional: usize) -> DecodeResult<()> {
        if additional > self.limit - self.bytes.len() {
            return Err("Decompressed data exceeds its output limit.".into());
        }
        if self.bytes.len() >= self.next_check {
            (self.check)()?;
            self.next_check = self.bytes.len() + CHECK_INTERVAL;
        }
        if self.bytes.capacity() - self.bytes.len() < additional {
            let capacity = (self.bytes.len() + additional)
                .max(self.bytes.capacity().saturating_mul(2))
                .max(CHECK_INTERVAL)
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|_| "Cannot allocate decompressed data.")?;
        }
        Ok(())
    }
}

fn inflate_codes(
    bits: &mut Bits<'_>,
    literals: &Huffman,
    distances: &Huffman,
    output: &mut Output<'_>,
    history_start: usize,
    window: usize,
) -> DecodeResult<()> {
    loop {
        let symbol = literals.symbol(bits)?;
        match symbol {
            0..=255 => {
                output.room(1)?;
                output.bytes.push(symbol as u8);
            }
            256 => return Ok(()),
            257..=285 => {
                let length = if symbol <= 264 {
                    symbol as usize - 254
                } else if symbol == 285 {
                    258
                } else {
                    let group = symbol as usize - 265;
                    let extra = group / 4 + 1;
                    (1usize << (extra + 2))
                        + 3
                        + (group % 4) * (1usize << extra)
                        + bits.read(extra as u8)? as usize
                };
                let code = distances.symbol(bits)? as usize;
                if code > 29 {
                    return Err("Reserved DEFLATE distance code.".into());
                }
                let distance = if code < 4 {
                    code + 1
                } else {
                    let extra = code / 2 - 1;
                    ((2 + code % 2) << extra) + 1 + bits.read(extra as u8)? as usize
                };
                if distance > output.bytes.len() - history_start || distance > window {
                    return Err("DEFLATE distance exceeds available history.".into());
                }
                output.room(length)?;
                // Reading from the growing output intentionally permits overlap.
                // The history boundary prevents references into a prior gzip member.
                for _ in 0..length {
                    let value = output.bytes[output.bytes.len() - distance];
                    output.bytes.push(value);
                }
            }
            _ => return Err("Reserved DEFLATE length code.".into()),
        }
    }
}

/// Return the number of whole input bytes occupied by one DEFLATE stream.
/// Bits never looks ahead by a complete byte, so wrapper trailers stay unread.
fn inflate(data: &[u8], output: &mut Output<'_>, window: usize) -> DecodeResult<usize> {
    let history_start = output.bytes.len();
    let mut bits = Bits::new(data);
    let mut fixed = None;
    loop {
        // Every block consumes header bits from the admitted input, including
        // empty blocks. Keep cancellation at that boundary instead of imposing
        // a separate count limit on valid streams.
        (output.check)()?;
        let last = bits.read(1)? != 0;
        match bits.read(2)? {
            0 => {
                bits.align();
                let length = bits.read(16)? as usize;
                let complement = bits.read(16)? as usize;
                if (length ^ complement) != 0xffff {
                    return Err("Invalid stored DEFLATE block length.".into());
                }
                for chunk in bits.bytes(length)?.chunks(CHECK_INTERVAL) {
                    output.room(chunk.len())?;
                    output.bytes.extend_from_slice(chunk);
                }
            }
            1 => {
                if fixed.is_none() {
                    fixed = Some(fixed_trees()?);
                }
                let (literals, distances) = fixed.as_ref().ok_or("Missing fixed Huffman trees.")?;
                inflate_codes(
                    &mut bits,
                    literals,
                    distances,
                    output,
                    history_start,
                    window,
                )?;
            }
            2 => {
                let (literals, distances) = dynamic_trees(&mut bits)?;
                inflate_codes(
                    &mut bits,
                    &literals,
                    &distances,
                    output,
                    history_start,
                    window,
                )?;
            }
            _ => return Err("Reserved DEFLATE block type.".into()),
        }
        if last {
            return Ok(bits.position);
        }
    }
}

/// Decode one complete RFC 1950 zlib stream. HTTP's "deflate" coding uses this
/// wrapper; raw DEFLATE and preset dictionaries are deliberately not guessed.
pub fn zlib(
    data: &[u8],
    limit: usize,
    check: &dyn Fn() -> DecodeResult<()>,
) -> DecodeResult<Vec<u8>> {
    let mut output = Output::new(data, limit, check)?;
    if data.len() < 6 {
        return Err("Truncated zlib stream.".into());
    }
    let cmf = data[0];
    let flags = data[1];
    if cmf & 15 != 8
        || cmf >> 4 > 7
        || !(u16::from(cmf) * 256 + u16::from(flags)).is_multiple_of(31)
    {
        return Err("Invalid zlib compression header.".into());
    }
    if flags & 0x20 != 0 {
        return Err("Preset zlib dictionaries are not supported.".into());
    }
    let compressed = &data[2..data.len() - 4];
    let consumed = inflate(compressed, &mut output, 1usize << ((cmf >> 4) + 8))?;
    if consumed != compressed.len() {
        return Err("Unexpected trailing bytes after the zlib DEFLATE stream.".into());
    }
    let tail = &data[data.len() - 4..];
    let expected = u32::from_be_bytes([tail[0], tail[1], tail[2], tail[3]]);
    if adler32_checked(&output.bytes, check)? != expected {
        return Err("Zlib Adler-32 checksum does not match.".into());
    }
    check()?;
    Ok(output.bytes)
}

fn header_advance(data: &[u8], position: &mut usize, count: usize) -> DecodeResult<()> {
    let end = position
        .checked_add(count)
        .filter(|&end| end <= data.len())
        .ok_or("Truncated gzip header.")?;
    *position = end;
    Ok(())
}

fn header_string(
    data: &[u8],
    position: &mut usize,
    check: &dyn Fn() -> DecodeResult<()>,
) -> DecodeResult<()> {
    // Names and comments are terminated by NUL, with no format-defined length
    // field. Scan the admitted input without copying or reserving header bytes.
    let remaining = data.get(*position..).ok_or("Truncated gzip header.")?;
    for chunk in remaining.chunks(CHECK_INTERVAL) {
        check()?;
        if let Some(index) = chunk.iter().position(|&byte| byte == 0) {
            header_advance(data, position, index + 1)?;
            return Ok(());
        }
        header_advance(data, position, chunk.len())?;
    }
    Err("Gzip filename/comment is unterminated.".into())
}

fn gzip_header(data: &[u8], check: &dyn Fn() -> DecodeResult<()>) -> DecodeResult<usize> {
    if data.len() < 10 {
        return Err("Truncated gzip header.".into());
    }
    if data[..3] != [0x1f, 0x8b, 8] {
        return Err("Invalid gzip signature or compression method.".into());
    }
    let flags = data[3];
    if flags & 0xe0 != 0 {
        return Err("Reserved gzip flags are set.".into());
    }
    let mut position = 10;
    if flags & 4 != 0 {
        header_advance(data, &mut position, 2)?;
        let length = u16::from_le_bytes([data[10], data[11]]) as usize;
        header_advance(data, &mut position, length)?;
    }
    for flag in [8, 16] {
        if flags & flag != 0 {
            header_string(data, &mut position, check)?;
        }
    }
    if flags & 2 != 0 {
        let header_end = position;
        header_advance(data, &mut position, 2)?;
        let expected = u16::from_le_bytes([data[header_end], data[header_end + 1]]);
        if crc32_checked(&data[..header_end], check)? as u16 != expected {
            return Err("Gzip header CRC-16 does not match.".into());
        }
    }
    Ok(position)
}

/// Decode all RFC 1952 gzip members. Each member has independent history and
/// checksums; all members share the same output budget and cancellation check.
/// No partial result escapes if a later member, trailer or checksum fails.
pub fn gzip(
    data: &[u8],
    limit: usize,
    check: &dyn Fn() -> DecodeResult<()>,
) -> DecodeResult<Vec<u8>> {
    let mut output = Output::new(data, limit, check)?;
    if data.is_empty() {
        return Err("Truncated gzip stream.".into());
    }
    let mut position = 0;
    while position < data.len() {
        // Each successful member advances through a header, DEFLATE stream and
        // trailer. Input admission and cancellation also bound empty members;
        // all decoded bytes still share the caller's output budget.
        check()?;
        let header_len = gzip_header(&data[position..], check)?;
        header_advance(data, &mut position, header_len)?;
        let member_start = output.bytes.len();
        let consumed = inflate(&data[position..], &mut output, 32_768)?;
        position = position
            .checked_add(consumed)
            .filter(|&end| end <= data.len())
            .ok_or("Truncated gzip DEFLATE stream.")?;
        let end = position
            .checked_add(8)
            .filter(|&end| end <= data.len())
            .ok_or("Truncated gzip trailer.")?;
        let tail = &data[position..end];
        let expected_crc = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
        let expected_size = u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]);
        let member = &output.bytes[member_start..];
        if member.len() as u32 != expected_size {
            return Err("Gzip uncompressed size does not match its trailer.".into());
        }
        if crc32_checked(member, check)? != expected_crc {
            return Err("Gzip CRC-32 checksum does not match.".into());
        }
        position = end;
    }
    check()?;
    Ok(output.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn stored(raw: &[u8]) -> Vec<u8> {
        assert!(raw.len() <= 65535);
        let length = raw.len() as u16;
        let mut data = vec![1];
        data.extend_from_slice(&length.to_le_bytes());
        data.extend_from_slice(&(!length).to_le_bytes());
        data.extend_from_slice(raw);
        data
    }

    fn member(raw: &[u8], optional: bool) -> Vec<u8> {
        let mut data = vec![
            0x1f,
            0x8b,
            8,
            if optional { 31 } else { 0 },
            0,
            0,
            0,
            0,
            0,
            255,
        ];
        if optional {
            data.extend_from_slice(&[4, 0, b'O', b'B', 0, 0]);
            data.extend_from_slice(b"page.html\0A test comment\0");
            data.extend_from_slice(&(crc32(&data) as u16).to_le_bytes());
        }
        data.extend_from_slice(&stored(raw));
        data.extend_from_slice(&crc32(raw).to_le_bytes());
        data.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        data
    }

    #[test]
    fn gzip_optional_fields_and_independent_members() {
        let mut bytes = member(b"first", true);
        bytes.extend_from_slice(&member(b"", false));
        bytes.extend_from_slice(&member(b" second", false));
        assert_eq!(gzip(&bytes, 12, &|| Ok(())).unwrap(), b"first second");
        assert!(gzip(&bytes, 11, &|| Ok(())).is_err());
        assert_eq!(gzip(&member(b"", true), 0, &|| Ok(())).unwrap(), b"");
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    }

    #[test]
    fn gzip_truncation_flags_checksums_and_trailing_bytes_fail() {
        let valid = member(b"body", true);
        for end in 0..valid.len() {
            assert!(
                gzip(&valid[..end], 100, &|| Ok(())).is_err(),
                "prefix {end}"
            );
        }
        for offset in [0, 1, 2, 3, valid.len() - 1, valid.len() - 5, 17] {
            let mut wrong = valid.clone();
            wrong[offset] ^= 0x40;
            assert!(gzip(&wrong, 100, &|| Ok(())).is_err(), "offset {offset}");
        }
        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(gzip(&trailing, 100, &|| Ok(())).is_err());
        let mut bad_second = member(b"ok", false);
        let mut second = member(b"bad", false);
        *second.last_mut().unwrap() ^= 1;
        bad_second.extend_from_slice(&second);
        assert!(gzip(&bad_second, 100, &|| Ok(())).is_err());
    }

    #[test]
    fn gzip_many_members_keep_the_final_payload_and_shared_output_budget() {
        let empty = member(b"", false);
        let raw = b"last member";
        let mut data = empty.repeat(256);
        data.extend_from_slice(&member(raw, false));
        assert_eq!(gzip(&data, raw.len(), &|| Ok(())).unwrap(), raw);
        assert!(gzip(&data, raw.len() - 1, &|| Ok(()))
            .unwrap_err()
            .contains("output limit"));

        // Cancellation still reaches work after the former 128-member cutoff,
        // even though the empty members add no bytes to the output budget.
        let checks = Cell::new(0);
        let check = || {
            checks.set(checks.get() + 1);
            if checks.get() == 300 { Err("cancelled".into()) } else { Ok(()) }
        };
        assert_eq!(gzip(&data, raw.len(), &check).unwrap_err(), "cancelled");
        assert_eq!(checks.get(), 300);
        let crc_offset = data.len() - 8;
        data[crc_offset] ^= 1;
        assert!(gzip(&data, raw.len(), &|| Ok(()))
            .unwrap_err()
            .contains("CRC-32"));
    }

    #[test]
    fn many_deflate_blocks_check_the_tail_cancellation_and_output_budget() {
        let raw = b"final block";
        // More than twice the former aggregate 16,384-block limit. All early
        // blocks are empty, so progress must be governed by input, not output.
        let prefix = [0, 0, 0, 255, 255].repeat(32_768);
        let gzip_tail = member(raw, false);
        let mut gzip_data = gzip_tail[..10].to_vec();
        gzip_data.extend_from_slice(&prefix);
        gzip_data.extend_from_slice(&gzip_tail[10..]);
        let mut zlib_data = vec![0x78, 1];
        zlib_data.extend_from_slice(&prefix);
        zlib_data.extend_from_slice(&stored(raw));
        zlib_data.extend_from_slice(&adler32(raw).to_be_bytes());

        for (is_gzip, mut data, header_len) in [(true, gzip_data, 10), (false, zlib_data, 2)] {
            let decode = |bytes: &[u8], limit, check: &dyn Fn() -> DecodeResult<()>| {
                if is_gzip { gzip(bytes, limit, check) } else { zlib(bytes, limit, check) }
            };
            assert_eq!(decode(&data, raw.len(), &|| Ok(())).unwrap(), raw);
            assert!(decode(&data, raw.len() - 1, &|| Ok(()))
                .unwrap_err()
                .contains("output limit"));
            let checks = Cell::new(0);
            let check = || {
                checks.set(checks.get() + 1);
                if checks.get() == 17_000 { Err("cancelled".into()) } else { Ok(()) }
            };
            assert_eq!(decode(&data, raw.len(), &check).unwrap_err(), "cancelled");
            assert_eq!(checks.get(), 17_000);

            // A reserved final block after the old ceiling must fail, rather
            // than return an apparently successful prefix.
            data[header_len + prefix.len()] = 7;
            assert!(decode(&data, raw.len(), &|| Ok(()))
                .unwrap_err()
                .contains("Reserved DEFLATE block type"));
        }
    }

    #[test]
    fn gzip_maximum_extra_field_preserves_format_length_and_truncation_checks() {
        let raw = b"extra field tail";
        let tail = member(raw, false);
        let mut data = tail[..10].to_vec();
        data[3] = 4;
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        // One unknown, well-formed subfield occupies the full legal XLEN.
        data.extend_from_slice(&[b'O', b'B']);
        data.extend_from_slice(&(u16::MAX - 4).to_le_bytes());
        data.resize(12 + usize::from(u16::MAX), b'x');
        let header_len = data.len();
        assert!(header_len > 65_536);
        data.extend_from_slice(&tail[10..]);
        assert_eq!(gzip(&data, raw.len(), &|| Ok(())).unwrap(), raw);
        assert!(gzip(&data[..header_len - 1], raw.len(), &|| Ok(()))
            .unwrap_err()
            .contains("Truncated gzip header"));
    }

    #[test]
    fn gzip_long_metadata_keeps_termination_header_crc_and_cancellation_checks() {
        let raw = b"metadata tail";
        let tail = member(raw, false);
        for flag in [8, 16] {
            let mut data = tail[..10].to_vec();
            data[3] = flag | 2;
            data.resize(10 + 131_073, b'n');
            let terminator = data.len();
            assert!(gzip(&data, raw.len(), &|| Ok(()))
                .unwrap_err()
                .contains("unterminated"));
            data.push(0);
            let crc_offset = data.len();
            data.extend_from_slice(&(crc32(&data) as u16).to_le_bytes());
            data.extend_from_slice(&tail[10..]);
            assert_eq!(gzip(&data, raw.len(), &|| Ok(())).unwrap(), raw);

            let checks = Cell::new(0);
            let check = || {
                checks.set(checks.get() + 1);
                if checks.get() == 12 { Err("cancelled".into()) } else { Ok(()) }
            };
            assert_eq!(gzip(&data, raw.len(), &check).unwrap_err(), "cancelled");
            assert_eq!(checks.get(), 12);
            assert!(gzip(&data[..crc_offset + 1], raw.len(), &|| Ok(()))
                .unwrap_err()
                .contains("Truncated gzip header"));
            // A changed byte beyond 64 KiB must still be authenticated by FHCRC.
            data[terminator - 1] ^= 1;
            assert!(gzip(&data, raw.len(), &|| Ok(()))
                .unwrap_err()
                .contains("header CRC"));
        }
    }

    #[test]
    fn zlib_is_wrapped_exact_bounded_and_dictionary_free() {
        let raw = b"data";
        let mut data = vec![0x78, 1];
        data.extend_from_slice(&stored(raw));
        data.extend_from_slice(&adler32(raw).to_be_bytes());
        assert_eq!(zlib(&data, 4, &|| Ok(())).unwrap(), raw);
        assert!(zlib(&data, 3, &|| Ok(())).is_err());
        assert!(zlib(&data, MAX_OUTPUT_BYTES + 1, &|| Ok(())).is_err());
        assert!(zlib(&stored(raw), 4, &|| Ok(())).is_err());
        let mut extra = data.clone();
        extra.push(0);
        assert!(zlib(&extra, 4, &|| Ok(())).is_err());
        for offset in [0, 1, data.len() - 1] {
            let mut wrong = data.clone();
            wrong[offset] ^= 1;
            assert!(zlib(&wrong, 4, &|| Ok(())).is_err());
        }
        data[1] = 0x20;
        assert!(zlib(&data, 4, &|| Ok(()))
            .unwrap_err()
            .contains("dictionar"));
        assert!(Huffman::new(&[1, 1, 1], TreeKind::Literals).is_err());
        assert!(Huffman::new(&[2], TreeKind::Literals).is_err());
    }

    #[test]
    fn cancellation_reaches_decode_and_checksum_work() {
        let data = member(&vec![b'x'; 32768], false);
        for stop_at in [1, 4, 7, 8, 11] {
            let checks = Cell::new(0);
            let check = || {
                let count = checks.get() + 1;
                checks.set(count);
                if count >= stop_at {
                    Err("cancelled".to_owned())
                } else {
                    Ok(())
                }
            };
            assert_eq!(gzip(&data, 32768, &check).unwrap_err(), "cancelled");
            assert_eq!(checks.get(), stop_at);
        }
        assert!(preflight(MAX_INPUT_BYTES + 1, 0).is_err());
    }

    #[test]
    fn compression_generous_admission_is_checked_without_maximum_preallocation() {
        assert!(preflight(MAX_INPUT_BYTES,MAX_OUTPUT_BYTES).is_ok());
        for (input,limit) in [(MAX_INPUT_BYTES+1,0),(0,MAX_OUTPUT_BYTES+1),(usize::MAX,0),(0,usize::MAX)] {
            assert!(preflight(input,limit).is_err());
        }
        let check=||Ok(());let output=Output::new(&[],MAX_OUTPUT_BYTES,&check).unwrap();
        assert_eq!(output.bytes.capacity(),0);
        assert_eq!(gzip(&member(b"ordinary",false),MAX_OUTPUT_BYTES,&||Ok(())).unwrap(),b"ordinary");
    }

    #[test]
    fn compression_original_stored_gzip_crosses_old_32_mib_with_complete_checked_tail() {
        const SIZE:usize=33*1024*1024;
        let block=vec![0xa7;65535];let mut left=SIZE;let mut crc=u32::MAX;
        let mut encoded=vec![31,139,8,0,0,0,0,0,0,255];
        encoded.try_reserve_exact(SIZE+SIZE.div_ceil(65535)*5+8).unwrap();
        while left!=0 {
            let count=left.min(block.len());let length=count as u16;left-=count;
            encoded.push(u8::from(left==0));encoded.extend_from_slice(&length.to_le_bytes());
            encoded.extend_from_slice(&(!length).to_le_bytes());encoded.extend_from_slice(&block[..count]);
            crc=crc_update(crc,&block[..count]);
        }
        encoded.extend_from_slice(&(!crc).to_le_bytes());encoded.extend_from_slice(&(SIZE as u32).to_le_bytes());
        let decoded=gzip(&encoded,MAX_OUTPUT_BYTES,&||Ok(())).unwrap();
        assert_eq!(decoded.len(),SIZE);assert!(decoded.iter().all(|&byte|byte==0xa7));
        assert!(decoded.capacity()<MAX_OUTPUT_BYTES);
        // Corrupt the actual last output byte without updating the wrapper's
        // CRC. A decoder returning a successful historical prefix would miss it.
        drop(decoded);let last=encoded.len()-9;encoded[last]^=1;
        assert!(gzip(&encoded,MAX_OUTPUT_BYTES,&||Ok(())).unwrap_err().contains("CRC-32"));
    }
}
