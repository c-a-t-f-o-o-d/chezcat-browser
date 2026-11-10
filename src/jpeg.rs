//! Original, bounded JPEG decoder. No decoder library or foreign source is used.
//! The bitstream and DCT math follow ITU-T T.81 Annexes A/B/C/F/G:
//! <https://www.w3.org/Graphics/JPEG/itu-t81.pdf>.
//! JFIF sample placement and YCbCr conversion follow ITU-T T.871/JFIF 1.02.
//! Exif IFD0 orientation follows CIPA DC-008. Other metadata is not executed.

use crate::image::{Bitmap, MAX_INPUT_BYTES, MAX_PIXELS};

type Result<T> = std::result::Result<T, String>;
type Check<'a> = &'a dyn Fn() -> Result<()>;
const MAX_DIMENSION: usize = 16_384;
const MAX_COEFFICIENTS: usize = 16 * 1024 * 1024;
const MAX_SCANS: usize = 256;
const MAX_MARKERS: usize = 65_536;
const MAX_SCAN_WORK: usize = 128_000_000;

// Natural row-major positions visited by JPEG's diagonal coefficient order.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

fn allocated<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut items = Vec::new();
    items
        .try_reserve_exact(count)
        .map_err(|_| "Not enough memory for the JPEG image.")?;
    items.resize(count, value);
    Ok(items)
}

fn word(data: &[u8], offset: usize) -> Result<usize> {
    let value = data
        .get(offset..offset + 2)
        .ok_or("Truncated JPEG parameter.")?;
    Ok(usize::from(u16::from_be_bytes([value[0], value[1]])))
}

fn marker(data: &[u8], position: &mut usize) -> Result<u8> {
    if data.get(*position) != Some(&255) {
        return Err("JPEG marker is missing or extra entropy data remains.".into());
    }
    while data.get(*position) == Some(&255) {
        *position += 1;
    }
    let value = *data.get(*position).ok_or("Truncated JPEG marker.")?;
    *position += 1;
    if value == 0 {
        return Err("Stuffed JPEG byte occurs outside entropy data.".into());
    }
    Ok(value)
}

fn segment<'a>(data: &'a [u8], position: &mut usize) -> Result<&'a [u8]> {
    let size = word(data, *position)?;
    if size < 2 {
        return Err("Invalid JPEG marker length.".into());
    }
    let end = position
        .checked_add(size)
        .filter(|&end| end <= data.len())
        .ok_or("JPEG marker extends beyond the input.")?;
    let body = &data[*position + 2..end];
    *position = end;
    Ok(body)
}

struct Bits<'a> {
    data: &'a [u8],
    position: usize,
    buffer: u32,
    count: u8,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8], position: usize) -> Self {
        Self {
            data,
            position,
            buffer: 0,
            count: 0,
        }
    }

    // Never read through a marker while looking ahead for a short Huffman code.
    fn fill(&mut self, count: u8) -> Result<bool> {
        while self.count < count {
            let Some(&value) = self.data.get(self.position) else {
                return Ok(false);
            };
            if value == 255 {
                let Some(&next) = self.data.get(self.position + 1) else {
                    return Ok(false);
                };
                if next != 0 {
                    return Ok(false);
                }
                self.position += 1;
            }
            self.position += 1;
            self.buffer = (self.buffer << 8) | u32::from(value);
            self.count += 8;
        }
        Ok(true)
    }

    fn take(&mut self, count: u8) -> Result<u32> {
        if count == 0 {
            return Ok(0);
        }
        if !self.fill(count)? {
            return Err("JPEG entropy data is truncated or reached an unexpected marker.".into());
        }
        self.count -= count;
        let value = self.buffer >> self.count;
        self.buffer &= (1u32 << self.count) - 1;
        Ok(value)
    }

    fn finish(&mut self) -> Result<()> {
        if self.count > 7 || self.buffer != (1u32 << self.count) - 1 {
            return Err("Invalid JPEG entropy padding or extra entropy data.".into());
        }
        self.buffer = 0;
        self.count = 0;
        Ok(())
    }
}

#[derive(Clone)]
struct Huffman {
    first: [i32; 17],
    last: [i32; 17],
    offset: [usize; 17],
    symbols: Vec<u8>,
    quick: [(u8, u8); 256],
}

impl Huffman {
    fn new(counts: &[u8], symbols: &[u8], dc: bool) -> Result<Self> {
        if counts.len() != 16 || symbols.is_empty() || symbols.len() > 256 {
            return Err("Invalid JPEG Huffman table size.".into());
        }
        let mut table = Self {
            first: [0; 17],
            last: [-1; 17],
            offset: [0; 17],
            symbols: symbols.to_vec(),
            quick: [(0, 0); 256],
        };
        let mut code = 0usize;
        let mut index = 0usize;
        let mut seen = [false; 256];
        for length in 1..=16 {
            let count = usize::from(counts[length - 1]);
            // A complete tree would assign the forbidden all-one padding code.
            if count > 0 && code + count >= 1usize << length {
                return Err("Oversubscribed JPEG Huffman table or forbidden all-one code.".into());
            }
            if index + count > symbols.len() {
                return Err("Truncated JPEG Huffman values.".into());
            }
            table.first[length] = code as i32;
            table.offset[length] = index;
            if count > 0 {
                table.last[length] = (code + count - 1) as i32;
            }
            for (step, &symbol) in symbols[index..index + count].iter().enumerate() {
                if seen[usize::from(symbol)] || (dc && symbol > 11) || (!dc && symbol & 15 > 10) {
                    return Err("Invalid or duplicate 8-bit JPEG Huffman value.".into());
                }
                seen[usize::from(symbol)] = true;
                if length <= 8 {
                    let from = (code + step) << (8 - length);
                    let until = from + (1 << (8 - length));
                    table.quick[from..until].fill((length as u8, symbol));
                }
            }
            index += count;
            code = (code + count) << 1;
        }
        if index != symbols.len() {
            return Err("JPEG Huffman value count does not match its lengths.".into());
        }
        Ok(table)
    }

    fn read(&self, bits: &mut Bits<'_>) -> Result<u8> {
        if bits.fill(8)? {
            let prefix = (bits.buffer >> (bits.count - 8)) as usize;
            let (length, symbol) = self.quick[prefix];
            if length != 0 {
                bits.take(length)?;
                return Ok(symbol);
            }
        }
        let mut code = 0;
        for length in 1..=16 {
            code = (code << 1) | bits.take(1)? as i32;
            if code <= self.last[length] && code >= self.first[length] {
                return Ok(self.symbols[self.offset[length] + (code - self.first[length]) as usize]);
            }
        }
        Err("JPEG Huffman code is not defined by its table.".into())
    }
}

#[derive(Clone)]
struct Quant {
    values: [u16; 64],
    wide: bool,
}

struct Component {
    id: u8,
    horizontal: usize,
    vertical: usize,
    quant_index: usize,
    quant: Option<[u16; 64]>,
    width: usize,
    height: usize,
    block_width: usize,
    coefficients: Vec<i16>,
    progression: [i8; 64],
}

struct Frame {
    width: usize,
    height: usize,
    horizontal: usize,
    vertical: usize,
    columns: usize,
    rows: usize,
    kind: u8,
    components: Vec<Component>,
}

impl Frame {
    fn new(data: &[u8], kind: u8) -> Result<Self> {
        if data.len() < 6 || data[0] != 8 {
            return Err("Only 8-bit JPEG samples are supported in this build.".into());
        }
        let height = word(data, 1)?;
        let width = word(data, 3)?;
        if width == 0
            || height == 0
            || width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || width * height > MAX_PIXELS
        {
            return Err("JPEG exceeds the 4-megapixel or 16384-pixel dimension limit.".into());
        }
        let count = usize::from(data[5]);
        if !matches!(count, 1 | 3 | 4) {
            return Err("JPEG requires one, three, or four supported color components.".into());
        }
        if data.len() != 6 + 3 * count {
            return Err("Invalid JPEG frame header length.".into());
        }
        let (mut horizontal, mut vertical) = (0, 0);
        let mut components: Vec<Component> = Vec::new();
        for values in data[6..].as_chunks::<3>().0 {
            let h = usize::from(values[1] >> 4);
            let v = usize::from(values[1] & 15);
            if !(1..=4).contains(&h)
                || !(1..=4).contains(&v)
                || values[2] > 3
                || components.iter().any(|c| c.id == values[0])
            {
                return Err("Invalid or duplicate JPEG component parameters.".into());
            }
            horizontal = horizontal.max(h);
            vertical = vertical.max(v);
            components.push(Component {
                id: values[0],
                horizontal: h,
                vertical: v,
                quant_index: usize::from(values[2]),
                quant: None,
                width: 0,
                height: 0,
                block_width: 0,
                coefficients: Vec::new(),
                progression: [-1; 64],
            });
        }
        let columns = width.div_ceil(8 * horizontal);
        let rows = height.div_ceil(8 * vertical);
        let mut total = 0usize;
        for component in &mut components {
            component.width = (width * component.horizontal).div_ceil(horizontal);
            component.height = (height * component.vertical).div_ceil(vertical);
            component.block_width = columns * component.horizontal;
            let count = component.block_width * rows * component.vertical * 64;
            total = total
                .checked_add(count)
                .ok_or("JPEG coefficient count overflow.")?;
            if total > MAX_COEFFICIENTS {
                return Err(
                    "JPEG padded coefficient storage exceeds its 16-million coefficient limit."
                        .into(),
                );
            }
            component.coefficients = allocated(count, 0)?;
        }
        Ok(Self {
            width,
            height,
            horizontal,
            vertical,
            columns,
            rows,
            kind,
            components,
        })
    }
}

#[derive(Clone, Copy)]
struct Selection {
    component: usize,
    dc: usize,
    ac: usize,
}

struct Scan {
    selected: Vec<Selection>,
    first: usize,
    last: usize,
    high: u8,
    low: u8,
}

impl Scan {
    fn new(data: &[u8], frame: &Frame) -> Result<Self> {
        let count = data.first().copied().map(usize::from).unwrap_or(0);
        if count == 0 || count > frame.components.len() || data.len() != 4 + 2 * count {
            return Err("Invalid JPEG scan header length or component count.".into());
        }
        let mut selected: Vec<Selection> = Vec::new();
        for values in data[1..1 + 2 * count].as_chunks::<2>().0 {
            let component = frame
                .components
                .iter()
                .position(|c| c.id == values[0])
                .ok_or("JPEG scan references an unknown component.")?;
            let dc = usize::from(values[1] >> 4);
            let ac = usize::from(values[1] & 15);
            if selected
                .last()
                .is_some_and(|old| old.component >= component)
                || dc > 3
                || ac > 3
                || (frame.kind == 0xc0 && (dc > 1 || ac > 1))
            {
                return Err("Invalid JPEG scan component order or Huffman table selector.".into());
            }
            selected.push(Selection { component, dc, ac });
        }
        if count > 1
            && selected
                .iter()
                .map(|s| {
                    let c = &frame.components[s.component];
                    c.horizontal * c.vertical
                })
                .sum::<usize>()
                > 10
        {
            return Err("JPEG interleaved MCU contains more than 10 blocks.".into());
        }
        let scan = Self {
            selected,
            first: usize::from(data[1 + 2 * count]),
            last: usize::from(data[2 + 2 * count]),
            high: data[3 + 2 * count] >> 4,
            low: data[3 + 2 * count] & 15,
        };
        if frame.kind != 0xc2 {
            if scan.first != 0 || scan.last != 63 || scan.high != 0 || scan.low != 0 {
                return Err("Invalid sequential JPEG spectral or approximation parameters.".into());
            }
        } else if scan.first > scan.last
            || scan.last > 63
            || scan.low > 13
            || scan.high > 13
            || (scan.first == 0 && scan.last != 0)
            || (scan.first != 0 && count != 1)
            || (scan.high != 0 && scan.high != scan.low + 1)
        {
            return Err("Invalid progressive JPEG spectral or approximation parameters.".into());
        }
        for selection in &scan.selected {
            let c = &frame.components[selection.component];
            if scan.first > 0 && c.progression[0] < 0 {
                return Err("JPEG AC coefficients appear before their DC scan.".into());
            }
            for &previous in &c.progression[scan.first..=scan.last] {
                if (scan.high == 0 && previous != -1)
                    || (scan.high != 0 && previous != scan.high as i8)
                {
                    return Err(
                        "JPEG scan repeats coefficients or has an invalid refinement history."
                            .into(),
                    );
                }
            }
        }
        Ok(scan)
    }
}

fn signed(bits: &mut Bits<'_>, count: u8) -> Result<i32> {
    if count == 0 {
        return Ok(0);
    }
    let value = bits.take(count)? as i32;
    Ok(if value < 1 << (count - 1) {
        value + 1 - (1 << count)
    } else {
        value
    })
}

fn coefficient(value: i32) -> Result<i16> {
    i16::try_from(value)
        .map_err(|_| "JPEG coefficient exceeds the bounded 8-bit decoding range.".into())
}

fn refine(value: &mut i16, bits: &mut Bits<'_>, low: u8) -> Result<()> {
    let amount = 1i32 << low;
    if bits.take(1)? != 0 {
        let current = i32::from(*value);
        if current.unsigned_abs() & amount as u32 != 0 {
            return Err("JPEG refinement bit was already present.".into());
        }
        *value = coefficient(current + if current < 0 { -amount } else { amount })?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Explicit entropy state keeps the scan loop straightforward.
fn block(
    coefficients: &mut [i16],
    bits: &mut Bits<'_>,
    scan: &Scan,
    progressive: bool,
    dc: Option<&Huffman>,
    ac: Option<&Huffman>,
    predictor: &mut i32,
    eob: &mut usize,
) -> Result<()> {
    if scan.first == 0 {
        if scan.high == 0 {
            let size = dc.ok_or("JPEG DC Huffman table is missing.")?.read(bits)?;
            if size > 11 {
                return Err("Invalid JPEG DC coefficient size.".into());
            }
            *predictor = predictor
                .checked_add(signed(bits, size)?)
                .ok_or("JPEG DC prediction overflow.")?;
            coefficients[0] = coefficient(
                predictor
                    .checked_mul(1 << scan.low)
                    .ok_or("JPEG DC coefficient overflow.")?,
            )?;
        } else {
            coefficients[0] =
                coefficient(i32::from(coefficients[0]) | ((bits.take(1)? as i32) << scan.low))?;
        }
        if progressive {
            return Ok(());
        }
    }
    let table = ac.ok_or("JPEG AC Huffman table is missing.")?;
    let mut at = scan.first.max(1);
    if scan.high == 0 {
        if *eob > 0 {
            *eob -= 1;
            return Ok(());
        }
        while at <= scan.last {
            let symbol = table.read(bits)?;
            let run = usize::from(symbol >> 4);
            let size = symbol & 15;
            if size != 0 {
                if size > 10 || at + run > scan.last {
                    return Err("JPEG AC coefficient run exceeds the scan band.".into());
                }
                at += run;
                coefficients[ZIGZAG[at]] = coefficient(signed(bits, size)? * (1 << scan.low))?;
                at += 1;
            } else if run == 15 {
                if at + 16 > scan.last + 1 {
                    return Err("JPEG zero run exceeds the scan band.".into());
                }
                at += 16;
            } else if progressive {
                *eob = (1usize << run) + bits.take(run as u8)? as usize - 1;
                break;
            } else if run == 0 {
                break;
            } else {
                return Err("Invalid sequential JPEG end-of-block symbol.".into());
            }
        }
        return Ok(());
    }
    // A refinement run counts only still-zero coefficients. Previously nonzero
    // entries consume correction bits while we move past them.
    if *eob == 0 {
        while at <= scan.last {
            let symbol = table.read(bits)?;
            let mut zeros = usize::from(symbol >> 4);
            let size = symbol & 15;
            let new_value = if size == 1 {
                if bits.take(1)? == 0 {
                    -(1i32 << scan.low)
                } else {
                    1i32 << scan.low
                }
            } else if size == 0 {
                if zeros != 15 {
                    *eob = (1usize << zeros) + bits.take(zeros as u8)? as usize;
                    break;
                }
                zeros = 16;
                0
            } else {
                return Err("JPEG AC refinement introduces more than one bit.".into());
            };
            while at <= scan.last {
                let value = &mut coefficients[ZIGZAG[at]];
                if *value != 0 {
                    refine(value, bits, scan.low)?;
                } else if zeros != 0 {
                    zeros -= 1;
                } else {
                    break;
                }
                at += 1;
                if new_value == 0 && zeros == 0 {
                    break;
                }
            }
            if zeros != 0 || (new_value != 0 && at > scan.last) {
                return Err("JPEG AC refinement run exceeds the scan band.".into());
            }
            if new_value != 0 {
                coefficients[ZIGZAG[at]] = coefficient(new_value)?;
                at += 1;
            }
        }
    }
    if *eob > 0 {
        while at <= scan.last {
            let value = &mut coefficients[ZIGZAG[at]];
            if *value != 0 {
                refine(value, bits, scan.low)?;
            }
            at += 1;
        }
        *eob -= 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decode_scan(
    data: &[u8],
    position: &mut usize,
    frame: &mut Frame,
    scan: &Scan,
    tables: &[Option<Huffman>; 8],
    restart: usize,
    work: &mut usize,
    check: Check<'_>,
) -> Result<()> {
    let interleaved = scan.selected.len() > 1;
    let first = &frame.components[scan.selected[0].component];
    let columns = if interleaved {
        frame.columns
    } else {
        first.width.div_ceil(8)
    };
    let rows = if interleaved {
        frame.rows
    } else {
        first.height.div_ceil(8)
    };
    let blocks_per_unit = if interleaved {
        scan.selected
            .iter()
            .map(|s| {
                let c = &frame.components[s.component];
                c.horizontal * c.vertical
            })
            .sum()
    } else {
        1
    };
    let charge = columns
        .checked_mul(rows)
        .and_then(|n| n.checked_mul(blocks_per_unit))
        .and_then(|n| n.checked_mul(scan.last - scan.first + 1))
        .ok_or("JPEG scan work overflow.")?;
    *work = work
        .checked_sub(charge)
        .ok_or("JPEG exceeds the coefficient scan work limit.")?;
    let mut bits = Bits::new(data, *position);
    let mut predictors = [0i32; 4];
    let mut eob = 0;
    let mut restart_number = 0;
    let mut blocks = 0usize;
    for unit in 0..columns * rows {
        if restart != 0 && unit != 0 && unit.is_multiple_of(restart) {
            if eob != 0 {
                return Err("JPEG end-of-band run crosses a restart boundary.".into());
            }
            bits.finish()?;
            if marker(data, &mut bits.position)? != 0xd0 + restart_number {
                return Err("JPEG restart marker is missing or out of sequence.".into());
            }
            restart_number = (restart_number + 1) & 7;
            predictors.fill(0);
        }
        for selection in &scan.selected {
            let component = &mut frame.components[selection.component];
            let h = if interleaved { component.horizontal } else { 1 };
            let v = if interleaved { component.vertical } else { 1 };
            for y in 0..v {
                for x in 0..h {
                    if blocks.is_multiple_of(128) {
                        check()?;
                    }
                    blocks += 1;
                    let index = (((unit / columns) * v + y) * component.block_width
                        + (unit % columns) * h
                        + x)
                        * 64;
                    block(
                        &mut component.coefficients[index..index + 64],
                        &mut bits,
                        scan,
                        frame.kind == 0xc2,
                        tables[selection.dc].as_ref(),
                        tables[4 + selection.ac].as_ref(),
                        &mut predictors[selection.component],
                        &mut eob,
                    )?;
                }
            }
        }
    }
    if eob != 0 {
        return Err("JPEG end-of-band run extends past the scan.".into());
    }
    bits.finish()?;
    *position = bits.position;
    for selection in &scan.selected {
        frame.components[selection.component].progression[scan.first..=scan.last]
            .fill(scan.low as i8);
    }
    Ok(())
}

fn exif_orientation(data: &[u8]) -> Option<u8> {
    let tiff = data.strip_prefix(b"Exif\0\0")?;
    let little = match tiff.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let b: [u8; 2] = tiff.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(if little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b: [u8; 4] = tiff.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
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
                .filter(|n| (1..=8).contains(n));
        }
    }
    None
}

#[allow(clippy::needless_range_loop)] // The indices name the two transform axes.
fn idct(coefficients: &[i16], quant: &[u16; 64], basis: &[[f64; 8]; 8]) -> [u8; 64] {
    let mut output = [0; 64];
    if coefficients[1..].iter().all(|&n| n == 0) {
        let value = (f64::from(coefficients[0]) * f64::from(quant[0]) / 8.0 + 128.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        output.fill(value);
        return output;
    }
    let mut intermediate = [[0.0; 8]; 8];
    for vertical in 0..8 {
        for horizontal in 0..8 {
            let index = 8 * vertical + horizontal;
            if coefficients[index] == 0 {
                continue;
            }
            let amplitude = f64::from(coefficients[index]) * f64::from(quant[index]);
            for x in 0..8 {
                intermediate[vertical][x] += amplitude * basis[x][horizontal];
            }
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            let value = (0..8)
                .map(|v| intermediate[v][x] * basis[y][v])
                .sum::<f64>();
            output[y * 8 + x] = (value + 128.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    output
}

#[derive(Clone, Copy)]
struct Sample {
    first: usize,
    second: usize,
    weight: usize,
    divisor: usize,
}

fn sample_axis(position: usize, factor: usize, maximum: usize, size: usize) -> Sample {
    let numerator = ((2 * position + 1) * factor).saturating_sub(maximum);
    let divisor = 2 * maximum;
    let first = (numerator / divisor).min(size - 1);
    Sample {
        first,
        second: (first + 1).min(size - 1),
        weight: numerator % divisor,
        divisor,
    }
}

fn sample(plane: &[u8], width: usize, x: Sample, y: Sample) -> u8 {
    if x.weight == 0 && y.weight == 0 {
        return plane[y.first * width + x.first];
    }
    let top = usize::from(plane[y.first * width + x.first]) * (x.divisor - x.weight)
        + usize::from(plane[y.first * width + x.second]) * x.weight;
    let bottom = usize::from(plane[y.second * width + x.first]) * (x.divisor - x.weight)
        + usize::from(plane[y.second * width + x.second]) * x.weight;
    let divisor = x.divisor * y.divisor;
    ((top * (y.divisor - y.weight) + bottom * y.weight + divisor / 2) / divisor) as u8
}

#[allow(clippy::needless_range_loop)] // Output x indexes every component's sampling axis.
fn render(mut frame: Frame, orientation: u8, color: Color, check: Check<'_>) -> Result<Bitmap> {
    let mut basis = [[0.0; 8]; 8];
    for (position, row) in basis.iter_mut().enumerate() {
        for (frequency, value) in row.iter_mut().enumerate() {
            *value = if frequency == 0 {
                (1.0f64 / 8.0).sqrt()
            } else {
                (((2 * position + 1) * frequency) as f64 * std::f64::consts::PI / 16.0).cos() / 2.0
            };
        }
    }
    let mut planes = Vec::new();
    for component in &mut frame.components {
        let quant = component
            .quant
            .as_ref()
            .ok_or("JPEG component has no quantization table.")?;
        let mut plane = allocated(component.width * component.height, 0u8)?;
        for y in 0..component.height.div_ceil(8) {
            check()?;
            for x in 0..component.width.div_ceil(8) {
                let index = (y * component.block_width + x) * 64;
                let samples = idct(&component.coefficients[index..index + 64], quant, &basis);
                for row in 0..8.min(component.height - y * 8) {
                    let count = 8.min(component.width - x * 8);
                    let destination = (y * 8 + row) * component.width + x * 8;
                    plane[destination..destination + count]
                        .copy_from_slice(&samples[row * 8..row * 8 + count]);
                }
            }
        }
        component.coefficients = Vec::new();
        planes.push(plane);
    }
    let (width, height) = if orientation >= 5 {
        (frame.height, frame.width)
    } else {
        (frame.width, frame.height)
    };
    let mut pixels = allocated(width * height, 0u32)?;
    let axes: Vec<Vec<Sample>> = frame
        .components
        .iter()
        .map(|c| {
            (0..frame.width)
                .map(|x| sample_axis(x, c.horizontal, frame.horizontal, c.width))
                .collect()
        })
        .collect();
    for y in 0..frame.height {
        check()?;
        let mut vertical = [Sample {
            first: 0,
            second: 0,
            weight: 0,
            divisor: 1,
        }; 4];
        for (axis, component) in vertical.iter_mut().zip(&frame.components) {
            *axis = sample_axis(y, component.vertical, frame.vertical, component.height);
        }
        for x in 0..frame.width {
            let mut values = [0u8; 4];
            for (index, c) in frame.components.iter().enumerate() {
                values[index] = sample(&planes[index], c.width, axes[index][x], vertical[index]);
            }
            let [red, green, blue] = color.rgb(values);
            let (target_x, target_y) = match orientation {
                2 => (frame.width - 1 - x, y),
                3 => (frame.width - 1 - x, frame.height - 1 - y),
                4 => (x, frame.height - 1 - y),
                5 => (y, x),
                6 => (frame.height - 1 - y, x),
                7 => (frame.height - 1 - y, frame.width - 1 - x),
                8 => (y, frame.width - 1 - x),
                _ => (x, y),
            };
            pixels[target_y * width + target_x] =
                0xff000000 | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue);
        }
    }
    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

#[derive(Clone, Copy)]
enum Color {
    Gray(bool),
    Rgb,
    Ycbcr,
    Cmyk(bool),
    Ycck,
}

impl Color {
    fn rgb(self, values: [u8; 4]) -> [u8; 3] {
        let ycbcr = || {
            let y = i32::from(values[0]) * 65536;
            let cb = i32::from(values[1]) - 128;
            let cr = i32::from(values[2]) - 128;
            let channel = |value: i32| ((value + 32768) >> 16).clamp(0, 255) as u8;
            [
                channel(y + 91881 * cr),
                channel(y - 22554 * cb - 46802 * cr),
                channel(y + 116130 * cb),
            ]
        };
        let multiply = |a: u8, b: u8| ((u32::from(a) * u32::from(b) + 127) / 255) as u8;
        match self {
            Self::Gray(false) => [values[0]; 3],
            Self::Gray(true) => [255 - values[0]; 3],
            Self::Rgb => [values[0], values[1], values[2]],
            Self::Ycbcr => ycbcr(),
            // Adobe's standalone four-channel encoding stores inverted ink
            // values. TIFF stores normal ink values and supplies its own color
            // interpretation instead of consulting standalone APP14 markers.
            Self::Cmyk(inverted) => [values[0], values[1], values[2]].map(|v| {
                if inverted {
                    multiply(v, values[3])
                } else {
                    multiply(255 - v, 255 - values[3])
                }
            }),
            Self::Ycck => ycbcr().map(|v| multiply(255 - v, values[3])),
        }
    }
}

pub fn decode(data: &[u8], check: Check<'_>) -> Result<Bitmap> {
    decode_inner(data, None, check)
}

#[derive(Clone, Copy)]
struct TiffColor {
    photometric: u16,
    subsampling: Option<(usize, usize)>,
}

/// TIFF supplies the color model and orientation separately. Its JPEG payloads
/// use ISO component values; APP markers cannot redefine that interpretation.
pub fn decode_tiff(
    data: &[u8],
    photometric: u16,
    subsampling: Option<(usize, usize)>,
    check: Check<'_>,
) -> Result<Bitmap> {
    decode_inner(
        data,
        Some(TiffColor {
            photometric,
            subsampling,
        }),
        check,
    )
}

fn decode_inner(data: &[u8], tiff: Option<TiffColor>, check: Check<'_>) -> Result<Bitmap> {
    check()?;
    if data.len() > MAX_INPUT_BYTES {
        return Err("JPEG input exceeds the 32 MiB limit.".into());
    }
    if !data.starts_with(&[255, 0xd8]) {
        return Err("JPEG start-of-image marker is missing.".into());
    }
    let mut position = 2;
    let mut frame: Option<Frame> = None;
    let mut quant: [Option<Quant>; 4] = std::array::from_fn(|_| None);
    let mut tables: [Option<Huffman>; 8] = std::array::from_fn(|_| None);
    let (mut scans, mut restart, mut work) = (0usize, 0usize, MAX_SCAN_WORK);
    let mut adobe = None;
    let mut jfif = false;
    let mut orientation = None;
    for _ in 0..MAX_MARKERS {
        check()?;
        let code = marker(data, &mut position)?;
        match code {
            0xd9 => {
                if position != data.len() {
                    return Err("Trailing bytes after JPEG end-of-image marker.".into());
                }
                let frame = frame.ok_or("JPEG frame is missing.")?;
                if scans == 0 || frame.components.iter().any(|c| c.progression[0] < 0) {
                    return Err("JPEG image is missing component scan data.".into());
                }
                let count = frame.components.len();
                let color = if let Some(TiffColor {
                    photometric,
                    subsampling,
                }) = tiff
                {
                    if photometric == 6 && count == 3 {
                        let (h, v) = subsampling.ok_or("TIFF JPEG subsampling is missing.")?;
                        let components = &frame.components;
                        if components[1].horizontal != components[2].horizontal
                            || components[1].vertical != components[2].vertical
                            || components[0].horizontal != components[1].horizontal * h
                            || components[0].vertical != components[1].vertical * v
                        {
                            return Err(
                                "TIFF JPEG sampling factors do not match YCbCrSubSampling.".into(),
                            );
                        }
                    } else if frame
                        .components
                        .iter()
                        .any(|c| c.horizontal != frame.horizontal || c.vertical != frame.vertical)
                    {
                        return Err(
                            "TIFF JPEG subsampling is only supported for YCbCr data.".into()
                        );
                    }
                    match (photometric, count) {
                        (0, 1) => Color::Gray(true),
                        (1, 1) => Color::Gray(false),
                        (2, 3) => Color::Rgb,
                        (5, 4) => Color::Cmyk(false),
                        (6, 3) => Color::Ycbcr,
                        _ => {
                            return Err(
                                "TIFF/JPEG color model does not match its component count.".into()
                            )
                        }
                    }
                } else {
                    match (count, adobe) {
                        (1, None | Some(0)) => Color::Gray(false),
                        (3, Some(0)) => Color::Rgb,
                        (3, Some(1)) => Color::Ycbcr,
                        (3, None) if !jfif && frame.components.iter().map(|c| c.id).eq(b"RGB".iter().copied()) => Color::Rgb,
                        (3, None) => Color::Ycbcr,
                        (4, Some(0)) => Color::Cmyk(true),
                        (4, Some(2)) => Color::Ycck,
                        (4, None) => return Err("Four-component JPEG requires Adobe CMYK/YCCK metadata; unlabeled color is ambiguous.".into()),
                        _ => return Err("JPEG Adobe color transform does not match its component count.".into()),
                    }
                };
                return render(
                    frame,
                    if tiff.is_some() {
                        1
                    } else {
                        orientation.unwrap_or(1)
                    },
                    color,
                    check,
                );
            }
            0xd8 | 0xd0..=0xd7 | 0x01 => return Err("Unexpected standalone JPEG marker.".into()),
            0xc0..=0xc2 => {
                if frame.is_some() {
                    return Err("Multiple JPEG frames are not supported.".into());
                }
                if tiff.is_some() && code == 0xc2 {
                    return Err("Progressive JPEG is not supported within TIFF segments.".into());
                }
                frame = Some(Frame::new(segment(data, &mut position)?, code)?);
            }
            0xc4 => {
                let mut values = segment(data, &mut position)?;
                if values.is_empty() {
                    return Err("Empty JPEG Huffman marker.".into());
                }
                while !values.is_empty() {
                    if values.len() < 17 || values[0] >> 4 > 1 || values[0] & 15 > 3 {
                        return Err("Invalid JPEG Huffman marker.".into());
                    }
                    let count: usize = values[1..17].iter().map(|&n| usize::from(n)).sum();
                    if count > 256 || 17 + count > values.len() {
                        return Err("Truncated JPEG Huffman table.".into());
                    }
                    let index = usize::from(values[0] >> 4) * 4 + usize::from(values[0] & 15);
                    tables[index] = Some(Huffman::new(
                        &values[1..17],
                        &values[17..17 + count],
                        index < 4,
                    )?);
                    values = &values[17 + count..];
                }
            }
            0xdb => {
                let mut values = segment(data, &mut position)?;
                if values.is_empty() {
                    return Err("Empty JPEG quantization marker.".into());
                }
                while !values.is_empty() {
                    let wide = values[0] >> 4;
                    let index = usize::from(values[0] & 15);
                    let bytes = 1 + 64 * (usize::from(wide) + 1);
                    if wide > 1 || index > 3 || values.len() < bytes {
                        return Err("Invalid JPEG quantization table.".into());
                    }
                    let mut table = [0u16; 64];
                    for step in 0..64 {
                        let value = if wide == 0 {
                            usize::from(values[1 + step])
                        } else {
                            word(values, 1 + 2 * step)?
                        };
                        if value == 0 {
                            return Err("JPEG quantization values must be nonzero.".into());
                        }
                        table[ZIGZAG[step]] = value as u16;
                    }
                    quant[index] = Some(Quant {
                        values: table,
                        wide: wide != 0,
                    });
                    values = &values[bytes..];
                }
            }
            0xdd => {
                let values = segment(data, &mut position)?;
                if values.len() != 2 {
                    return Err("Invalid JPEG restart interval marker.".into());
                }
                restart = word(values, 0)?;
            }
            0xda => {
                scans += 1;
                if scans > MAX_SCANS {
                    return Err("JPEG exceeds the 256-scan limit.".into());
                }
                let frame = frame
                    .as_mut()
                    .ok_or("JPEG scan appears before its frame.")?;
                let scan = Scan::new(segment(data, &mut position)?, frame)?;
                for selection in &scan.selected {
                    let component = &mut frame.components[selection.component];
                    let table = quant[component.quant_index]
                        .as_ref()
                        .ok_or("JPEG quantization table is missing.")?;
                    if frame.kind == 0xc0 && table.wide {
                        return Err("Baseline JPEG requires 8-bit quantization tables.".into());
                    }
                    if component.quant.is_none() {
                        component.quant = Some(table.values);
                    }
                    if scan.first == 0 && scan.high == 0 && tables[selection.dc].is_none() {
                        return Err("JPEG DC Huffman table is missing.".into());
                    }
                    if scan.last != 0 && tables[4 + selection.ac].is_none() {
                        return Err("JPEG AC Huffman table is missing.".into());
                    }
                }
                decode_scan(
                    data,
                    &mut position,
                    frame,
                    &scan,
                    &tables,
                    restart,
                    &mut work,
                    check,
                )?;
            }
            0xe0 => {
                let values = segment(data, &mut position)?;
                if tiff.is_none() && values.starts_with(b"JFIF\0") {
                    if values.len() < 14
                        || values.len() < 14 + 3 * usize::from(values[12]) * usize::from(values[13])
                    {
                        return Err("Truncated JPEG JFIF metadata or thumbnail.".into());
                    }
                    jfif = true;
                }
            }
            0xe1 => {
                let values = segment(data, &mut position)?;
                if tiff.is_none() && orientation.is_none() {
                    orientation = exif_orientation(values);
                }
            }
            0xee => {
                let values = segment(data, &mut position)?;
                if tiff.is_none() && values.starts_with(b"Adobe") {
                    let value = *values
                        .get(11)
                        .ok_or("Truncated Adobe JPEG color metadata.")?;
                    if adobe.is_some_and(|old| old != value) {
                        return Err("Conflicting Adobe JPEG color transforms.".into());
                    }
                    adobe = Some(value);
                }
            }
            0xe0..=0xef | 0xfe => {
                segment(data, &mut position)?;
            }
            0xc3 | 0xc5..=0xcf => {
                return Err(
                    "Arithmetic, lossless and hierarchical JPEG modes are not implemented.".into(),
                )
            }
            _ => {
                return Err(format!(
                    "JPEG marker FF{code:02X} is not supported in this build."
                ))
            }
        }
    }
    Err("JPEG exceeds the marker count limit.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker_bytes(output: &mut Vec<u8>, code: u8, body: &[u8]) {
        output.extend([255, code]);
        output.extend(((body.len() + 2) as u16).to_be_bytes());
        output.extend(body);
    }

    // A one-pixel gray JPEG whose sole DC and EOB Huffman codes are both zero.
    // Everything is authored here, including the two entropy bits and padding.
    fn gray(progressive: bool) -> Vec<u8> {
        let mut bytes = vec![255, 0xd8];
        let mut quant = vec![0];
        quant.extend([1; 64]);
        marker_bytes(&mut bytes, 0xdb, &quant);
        marker_bytes(
            &mut bytes,
            if progressive { 0xc2 } else { 0xc0 },
            &[8, 0, 1, 0, 1, 1, 1, 0x11, 0],
        );
        let mut tables = Vec::new();
        for kind in [0, 0x10] {
            tables.extend([kind, 1]);
            tables.extend([0; 15]);
            tables.push(0);
        }
        marker_bytes(&mut bytes, 0xc4, &tables);
        marker_bytes(
            &mut bytes,
            0xda,
            &[1, 1, 0, 0, if progressive { 0 } else { 63 }, 0],
        );
        bytes.push(if progressive { 0x7f } else { 0x3f });
        if progressive {
            marker_bytes(&mut bytes, 0xda, &[1, 1, 0, 1, 63, 0]);
            bytes.push(0x7f);
        }
        bytes.extend([255, 0xd9]);
        bytes
    }

    #[test]
    fn independent_tiny_sequential_and_progressive_images_decode() {
        for progressive in [false, true] {
            let image = decode(&gray(progressive), &|| Ok(())).unwrap();
            assert_eq!((image.width, image.height), (1, 1));
            assert_eq!(image.pixels, [0xff808080]);
        }
    }

    // Four independently coded blocks have sample values 192, 64, 128, 192.
    // The fixture is authored from marker/codeword definitions, not encoder
    // output. Its non-neutral channels distinguish all three interpretations.
    fn four_components(adobe: Option<u8>) -> Vec<u8> {
        let mut bytes = vec![255, 0xd8];
        if let Some(transform) = adobe {
            let mut app = b"Adobe\0d\0\0\0\0".to_vec();
            app.push(transform);
            marker_bytes(&mut bytes, 0xee, &app);
        }
        let mut quant = vec![0];
        quant.extend([1; 64]);
        marker_bytes(&mut bytes, 0xdb, &quant);
        let mut frame = vec![8, 0, 1, 0, 1, 4];
        for id in b"CMYK" {
            frame.extend([*id, 0x11, 0]);
        }
        marker_bytes(&mut bytes, 0xc0, &frame);
        let mut tables = vec![0, 0, 2];
        tables.extend([0; 14]);
        tables.extend([0, 10]);
        tables.extend([0x10, 1]);
        tables.extend([0; 15]);
        tables.push(0);
        marker_bytes(&mut bytes, 0xc4, &tables);
        let mut scan = vec![4];
        for id in b"CMYK" {
            scan.extend([*id, 0]);
        }
        scan.extend([0, 63, 0]);
        marker_bytes(&mut bytes, 0xda, &scan);
        let mut bits = Vec::new();
        for dc in [512i32, -512, 0, 512] {
            bits.extend([0u8, u8::from(dc != 0)]);
            if dc != 0 {
                let encoded = if dc > 0 { dc } else { dc + 1023 };
                bits.extend((0..10).rev().map(|shift| ((encoded >> shift) & 1) as u8));
            }
            bits.push(0); // AC EOB.
        }
        while !bits.len().is_multiple_of(8) {
            bits.push(1);
        }
        for octet in bits.as_chunks::<8>().0 {
            let byte = octet.iter().fold(0, |v, bit| (v << 1) | bit);
            bytes.push(byte);
            if byte == 255 {
                bytes.push(0);
            }
        }
        bytes.extend([255, 0xd9]);
        bytes
    }

    #[test]
    fn cmyk_ycck_and_tiff_component_interpretations_are_distinct() {
        let inverted = four_components(Some(0));
        assert_eq!(decode(&inverted, &|| Ok(())).unwrap().pixels, [0xff913060]);
        let transformed = four_components(Some(2));
        assert_eq!(
            decode(&transformed, &|| Ok(())).unwrap().pixels,
            [0xff2f1f85]
        );
        for data in [&inverted, &transformed, &four_components(None)] {
            assert_eq!(
                decode_tiff(data, 5, None, &|| Ok(())).unwrap().pixels,
                [0xff102f1f]
            );
            assert!(decode_tiff(data, 2, None, &|| Ok(())).is_err());
        }
        assert!(decode(&four_components(None), &|| Ok(()))
            .unwrap_err()
            .contains("ambiguous"));
        assert!(decode(&four_components(Some(1)), &|| Ok(()))
            .unwrap_err()
            .contains("component count"));
        assert!(decode_tiff(&gray(true), 1, None, &|| Ok(()))
            .unwrap_err()
            .contains("Progressive"));
    }

    #[test]
    fn markers_padding_lengths_and_truncation_are_checked() {
        let original = gray(false);
        for end in 0..original.len() {
            assert!(
                decode(&original[..end], &|| Ok(())).is_err(),
                "accepted prefix {end}"
            );
        }
        let mut trailing = original.clone();
        trailing.push(0);
        assert!(decode(&trailing, &|| Ok(()))
            .unwrap_err()
            .contains("Trailing"));
        let mut padding = original.clone();
        let end = padding.len();
        padding[end - 3] = 0;
        assert!(decode(&padding, &|| Ok(()))
            .unwrap_err()
            .contains("padding"));
        assert!(decode(&[255, 0xd8, 255, 0xfe, 0, 1], &|| Ok(())).is_err());
        assert!(decode(&[255, 0xd8, 255, 0xd0], &|| Ok(())).is_err());
    }

    #[test]
    fn huffman_trees_and_coefficient_runs_are_bounded() {
        let mut counts = [0; 16];
        counts[0] = 2;
        assert!(Huffman::new(&counts, &[0, 1], true).is_err());
        counts[0] = 1;
        assert!(Huffman::new(&counts, &[12], true).is_err());
        assert!(Huffman::new(&counts, &[0x0b], false).is_err());
        counts[0] = 0;
        counts[1] = 2;
        assert!(Huffman::new(&counts, &[1, 1], true).is_err());
        assert!(coefficient(32768).is_err());
        assert!(coefficient(-32769).is_err());
    }

    #[test]
    fn dimensions_progression_and_cancellation_fail_explicitly() {
        assert!(Frame::new(&[8, 0, 0, 0, 1, 1, 1, 0x11, 0], 0xc0).is_err());
        assert!(Frame::new(&[8, 0x40, 1, 0, 1, 1, 1, 0x11, 0], 0xc0).is_err());
        assert!(Frame::new(&[12, 0, 1, 0, 1, 1, 1, 0x11, 0], 0xc0).is_err());
        let mut frame = Frame::new(&[8, 0, 1, 0, 1, 1, 1, 0x11, 0], 0xc2).unwrap();
        assert!(Scan::new(&[1, 1, 0, 1, 63, 0], &frame).is_err());
        frame.components[0].progression[0] = 2;
        assert!(Scan::new(&[1, 1, 0, 0, 0, 0x10], &frame).is_err());
        assert!(Scan::new(&[1, 1, 0, 0, 0, 0x21], &frame).is_ok());
        let calls = std::cell::Cell::new(0);
        let error = decode(&gray(true), &|| {
            calls.set(calls.get() + 1);
            if calls.get() > 6 {
                Err("Stopped".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "Stopped");
    }

    #[test]
    fn marker_and_scan_limits_bound_small_but_repetitive_files() {
        let mut markers = vec![255, 0xd8];
        for _ in 0..MAX_MARKERS {
            marker_bytes(&mut markers, 0xfe, &[]);
        }
        markers.extend([255, 0xd9]);
        assert!(decode(&markers, &|| Ok(()))
            .unwrap_err()
            .contains("marker count limit"));

        let original = gray(true);
        let header_end = original
            .windows(2)
            .position(|bytes| bytes == [255, 0xda])
            .unwrap();
        let make_scans = |count: usize| {
            let mut bytes = original[..header_end].to_vec();
            for index in 0..count {
                let band = (index % 64) as u8;
                let generation = (index / 64) as u8;
                let approximation = if generation == 0 {
                    13
                } else {
                    ((14 - generation) << 4) | (13 - generation)
                };
                marker_bytes(&mut bytes, 0xda, &[1, 1, 0, band, band, approximation]);
                bytes.push(0x7f); // A zero DC/refinement bit or zero EOB code, then one padding bits.
            }
            bytes.extend([255, 0xd9]);
            bytes
        };
        assert_eq!(
            decode(&make_scans(MAX_SCANS), &|| Ok(())).unwrap().pixels,
            [0xff808080]
        );
        assert!(decode(&make_scans(MAX_SCANS + 1), &|| Ok(()))
            .unwrap_err()
            .contains("256-scan limit"));
    }

    #[test]
    fn exif_orientation_and_idct_use_bounded_original_math() {
        for little in [false, true] {
            for orientation in 1u16..=8 {
                let mut bytes = b"Exif\0\0".to_vec();
                bytes.extend(if little { b"II" } else { b"MM" });
                let w = |value: u16| {
                    if little {
                        value.to_le_bytes()
                    } else {
                        value.to_be_bytes()
                    }
                };
                let d = |value: u32| {
                    if little {
                        value.to_le_bytes()
                    } else {
                        value.to_be_bytes()
                    }
                };
                bytes.extend(w(42));
                bytes.extend(d(8));
                bytes.extend(w(1));
                bytes.extend(w(0x112));
                bytes.extend(w(3));
                bytes.extend(d(1));
                bytes.extend(w(orientation));
                bytes.extend(w(0));
                bytes.extend(d(0));
                assert_eq!(exif_orientation(&bytes), Some(orientation as u8));
                bytes.truncate(bytes.len() - 1);
                assert_eq!(exif_orientation(&bytes), None);
            }
        }
        let mut coefficients = [0; 64];
        coefficients[0] = 80;
        assert_eq!(idct(&coefficients, &[1; 64], &[[0.0; 8]; 8]), [138; 64]);
    }
}
