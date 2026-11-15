//! Original BMP/DIB and ICO/CUR decoding. No platform image APIs are used.
//! Supports indexed RGB, RGB555, contiguous RGB(A) bitfields, RLE4/RLE8,
//! PNG icon payloads and the traditional icon AND mask.
//! References: Microsoft BITMAPINFOHEADER and the ICO format description.

use super::{dimensions, le16, le32, pixel, reserved, Bitmap, ImageResult};

const MAX_ICON_ENTRIES: usize = 256;
const MAX_RLE_COMMANDS: usize = 8 * 1024 * 1024;

struct Dib<'a> {
    data: &'a [u8],
    width: usize,
    height: usize,
    top_down: bool,
    depth: usize,
    compression: u32,
    image_size: usize,
    offset: usize,
    stride: usize,
    palette: Vec<u32>,
    masks: [u32; 4],
    icon: bool,
}

fn stride(width: usize, depth: usize) -> ImageResult<usize> {
    width
        .checked_mul(depth)
        .and_then(|n| n.checked_add(31))
        .map(|n| n / 32 * 4)
        .ok_or_else(|| "BMP row size overflow.".into())
}

impl<'a> Dib<'a> {
    fn read(
        data: &'a [u8],
        pixels: Option<usize>,
        icon: Option<(usize, usize)>,
    ) -> ImageResult<Self> {
        if data.len() < 12 {
            return Err("Truncated BMP/DIB header.".into());
        }
        let header_size = le32(data) as usize;
        if !matches!(header_size, 12 | 40 | 52 | 56 | 108 | 124) || header_size > data.len() {
            return Err("Unsupported or truncated BMP/DIB header.".into());
        }
        let core = header_size == 12;
        let (signed_width, signed_height, planes, depth, compression, image_size, used) = if core {
            (
                i32::from(le16(&data[4..])),
                i32::from(le16(&data[6..])),
                le16(&data[8..]),
                le16(&data[10..]) as usize,
                0,
                0,
                0,
            )
        } else {
            (
                le32(&data[4..]) as i32,
                le32(&data[8..]) as i32,
                le16(&data[12..]),
                le16(&data[14..]) as usize,
                le32(&data[16..]),
                le32(&data[20..]) as usize,
                le32(&data[32..]) as usize,
            )
        };
        if signed_width <= 0 || signed_height == 0 || signed_height == i32::MIN || planes != 1 {
            return Err("Invalid BMP/DIB dimensions or plane count.".into());
        }
        let width = signed_width as usize;
        let mut height = signed_height.unsigned_abs() as usize;
        if let Some((wanted_width, wanted_height)) = icon {
            if signed_height < 0 || width != wanted_width || height != wanted_height * 2 {
                return Err("ICO/CUR DIB dimensions do not match the directory.".into());
            }
            height /= 2;
        }
        dimensions(width, height)?;
        let valid = match compression {
            0 => {
                matches!(depth, 1 | 4 | 8 | 16 | 24 | 32)
                    && (!core || matches!(depth, 1 | 4 | 8 | 24))
            }
            1 => depth == 8 && signed_height > 0 && icon.is_none(),
            2 => depth == 4 && signed_height > 0 && icon.is_none(),
            3 | 6 => matches!(depth, 16 | 32),
            _ => false,
        };
        if !valid {
            return Err("Unsupported BMP/DIB depth or compression (RGB, bitfields, RLE4 and RLE8 are supported).".into());
        }
        let mut offset = header_size;
        let mut masks = match depth {
            16 => [0x7c00, 0x03e0, 0x001f, 0],
            32 => [0x00ff0000, 0x0000ff00, 0x000000ff, 0],
            _ => [0; 4],
        };
        if matches!(compression, 3 | 6) {
            if header_size == 40 {
                let count = if compression == 6 { 4 } else { 3 };
                let end = offset + count * 4;
                if end > data.len() {
                    return Err("BMP bitfields are truncated.".into());
                }
                for (index, mask) in masks.iter_mut().enumerate().take(count) {
                    *mask = le32(&data[offset + index * 4..]);
                }
                offset = end;
            } else {
                if header_size < 52 || (compression == 6 && header_size < 56) {
                    return Err("BMP bitfields need a compatible header.".into());
                }
                for (index, mask) in masks.iter_mut().enumerate().take(3) {
                    *mask = le32(&data[40 + index * 4..]);
                }
                if header_size >= 56 {
                    masks[3] = le32(&data[52..]);
                }
            }
            let mut occupied = 0u32;
            for (index, &mask) in masks.iter().enumerate() {
                if mask == 0 {
                    if index < 3 || compression == 6 {
                        return Err("BMP required bitfield mask is empty.".into());
                    }
                    continue;
                }
                let normalized = mask >> mask.trailing_zeros();
                if mask & occupied != 0
                    || u64::from(mask) >= 1u64 << depth
                    || normalized.count_ones() != normalized.ilog2() + 1
                {
                    return Err(
                        "BMP bitfield masks overlap, exceed the depth, or are not contiguous."
                            .into(),
                    );
                }
                occupied |= mask;
            }
        }
        let palette_size = if depth <= 8 {
            if used == 0 {
                1usize << depth
            } else {
                used
            }
        } else {
            used
        };
        if palette_size > 256 || (depth <= 8 && palette_size > 1usize << depth) {
            return Err("BMP palette exceeds its depth or 256-entry limit.".into());
        }
        let entry_size = if core { 3 } else { 4 };
        let palette_end = offset
            .checked_add(palette_size * entry_size)
            .filter(|&n| n <= data.len())
            .ok_or("Truncated BMP palette.")?;
        let mut palette = reserved(palette_size)?;
        for entry in data[offset..palette_end].chunks_exact(entry_size) {
            palette.push(pixel(entry[2], entry[1], entry[0], 255));
        }
        offset = palette_end;
        if let Some(pixels) = pixels {
            if pixels < offset || pixels > data.len() {
                return Err("BMP pixel offset overlaps metadata or exceeds the file.".into());
            }
            offset = pixels;
        }
        Ok(Self {
            data,
            width,
            height,
            top_down: signed_height < 0,
            depth,
            compression,
            image_size,
            offset,
            stride: stride(width, depth)?,
            palette,
            masks,
            icon: icon.is_some(),
        })
    }

    fn palette_pixel(&self, index: usize) -> ImageResult<u32> {
        self.palette
            .get(index)
            .copied()
            .ok_or_else(|| "BMP palette index is out of range.".into())
    }

    fn decode(&self, check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
        if matches!(self.compression, 1 | 2) {
            return self.rle(check);
        }
        let raster_end = self
            .offset
            .checked_add(
                self.stride
                    .checked_mul(self.height)
                    .ok_or("BMP raster size overflow.")?,
            )
            .filter(|&n| n <= self.data.len())
            .ok_or("Truncated BMP/DIB pixels.")?;
        let raster = &self.data[self.offset..raster_end];
        let mask_stride = stride(self.width, 1)?;
        let mask = if self.icon {
            let end = raster_end
                .checked_add(mask_stride * self.height)
                .filter(|&n| n <= self.data.len())
                .ok_or("Truncated ICO/CUR transparency mask.")?;
            &self.data[raster_end..end]
        } else {
            &[]
        };
        let legacy_alpha = self.icon && self.depth == 32 && self.compression == 0;
        // A zero fourth byte means reserved 0RGB in legacy icons; any nonzero
        // alpha makes the entire frame ARGB. Explicit bitfield alpha has no
        // such heuristic, so an all-transparent image remains transparent.
        let has_alpha = self.masks[3] != 0
            || (legacy_alpha && raster.as_chunks::<4>().0.iter().any(|p| p[3] != 0));
        let mut pixels = reserved(dimensions(self.width, self.height)?)?;
        for row in 0..self.height {
            check()?;
            let source_row = if self.top_down {
                row
            } else {
                self.height - row - 1
            };
            let line = &raster[source_row * self.stride..(source_row + 1) * self.stride];
            for column in 0..self.width {
                let mut color = match self.depth {
                    1 => {
                        self.palette_pixel(((line[column / 8] >> (7 - column % 8)) & 1) as usize)?
                    }
                    4 => self.palette_pixel(
                        ((line[column / 2] >> (if column % 2 == 0 { 4 } else { 0 })) & 15) as usize,
                    )?,
                    8 => self.palette_pixel(line[column] as usize)?,
                    24 => pixel(
                        line[column * 3 + 2],
                        line[column * 3 + 1],
                        line[column * 3],
                        255,
                    ),
                    16 | 32 => {
                        let value = if self.depth == 16 {
                            u32::from(le16(&line[column * 2..]))
                        } else {
                            le32(&line[column * 4..])
                        };
                        let alpha = if legacy_alpha && has_alpha {
                            (value >> 24) as u8
                        } else if self.masks[3] != 0 {
                            channel(value, self.masks[3])
                        } else {
                            255
                        };
                        pixel(
                            channel(value, self.masks[0]),
                            channel(value, self.masks[1]),
                            channel(value, self.masks[2]),
                            alpha,
                        )
                    }
                    _ => unreachable!(),
                };
                if self.icon
                    && !has_alpha
                    && mask[source_row * mask_stride + column / 8] & (0x80 >> (column % 8)) != 0
                {
                    color &= 0x00ff_ffff;
                }
                pixels.push(color);
            }
        }
        Ok(Bitmap {
            width: self.width,
            height: self.height,
            pixels,
        })
    }

    fn rle(&self, check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
        let end = if self.image_size == 0 {
            self.data.len()
        } else {
            self.offset
                .checked_add(self.image_size)
                .filter(|&n| n <= self.data.len())
                .ok_or("Truncated BMP RLE data.")?
        };
        let data = &self.data[self.offset..end];
        let mut pixels = reserved(dimensions(self.width, self.height)?)?;
        pixels.resize(self.width * self.height, self.palette_pixel(0)?);
        let (mut x, mut y, mut at, mut commands) = (0usize, 0usize, 0usize, 0usize);
        loop {
            commands += 1;
            if commands > MAX_RLE_COMMANDS {
                return Err("BMP RLE command limit reached.".into());
            }
            if commands.is_multiple_of(256) {
                check()?;
            }
            let pair = data
                .get(at..at + 2)
                .ok_or("BMP RLE stream is missing its end marker.")?;
            let (count, value) = (pair[0] as usize, pair[1]);
            at += 2;
            if count == 0 {
                match value {
                    0 => {
                        x = 0;
                        y = y
                            .checked_add(1)
                            .filter(|&n| n <= self.height)
                            .ok_or("BMP RLE row exceeds the image.")?;
                    }
                    1 => break,
                    2 => {
                        let delta = data.get(at..at + 2).ok_or("Truncated BMP RLE delta.")?;
                        at += 2;
                        x = x
                            .checked_add(delta[0] as usize)
                            .filter(|&n| n <= self.width)
                            .ok_or("BMP RLE delta exceeds the row.")?;
                        y = y
                            .checked_add(delta[1] as usize)
                            .filter(|&n| n < self.height)
                            .ok_or("BMP RLE delta exceeds the image.")?;
                    }
                    length => {
                        let count = length as usize;
                        if y >= self.height || x + count > self.width {
                            return Err("BMP RLE literal exceeds the image.".into());
                        }
                        let bytes = if self.depth == 4 {
                            count.div_ceil(2)
                        } else {
                            count
                        };
                        let padded = (bytes + 1) & !1;
                        let literal = data
                            .get(at..at + padded)
                            .ok_or("Truncated BMP RLE literal or padding.")?;
                        at += padded;
                        for n in 0..count {
                            let index = if self.depth == 4 {
                                (literal[n / 2] >> if n % 2 == 0 { 4 } else { 0 }) & 15
                            } else {
                                literal[n]
                            };
                            pixels[(self.height - 1 - y) * self.width + x + n] =
                                self.palette_pixel(index as usize)?;
                        }
                        x += count;
                    }
                }
            } else {
                if y >= self.height || x + count > self.width {
                    return Err("BMP RLE run exceeds the image.".into());
                }
                for n in 0..count {
                    let index = if self.depth == 4 {
                        (value >> if n % 2 == 0 { 4 } else { 0 }) & 15
                    } else {
                        value
                    };
                    pixels[(self.height - 1 - y) * self.width + x + n] =
                        self.palette_pixel(index as usize)?;
                }
                x += count;
            }
        }
        check()?;
        Ok(Bitmap {
            width: self.width,
            height: self.height,
            pixels,
        })
    }
}

fn channel(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let maximum = u64::from(mask >> shift);
    let value = u64::from((value & mask) >> shift);
    ((value * 255 + maximum / 2) / maximum) as u8
}

pub(super) fn bmp(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    if data.len() < 26 {
        return Err("Truncated BMP header.".into());
    }
    let size = le32(&data[2..]) as usize;
    let offset = le32(&data[10..]) as usize;
    if size < 26 || size > data.len() || offset < 14 || offset > size {
        return Err("Invalid BMP file size or pixel offset.".into());
    }
    Dib::read(&data[14..size], Some(offset - 14), None)?.decode(check)
}

pub(super) fn icon(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    if data.len() < 6 || le16(data) != 0 || !matches!(le16(&data[2..]), 1 | 2) {
        return Err("Invalid ICO/CUR directory header.".into());
    }
    let cursor = le16(&data[2..]) == 2;
    let count = le16(&data[4..]) as usize;
    let header_end = 6 + count * 16;
    if count == 0 || count > MAX_ICON_ENTRIES || header_end > data.len() {
        return Err("ICO/CUR directory is truncated or exceeds 256 entries.".into());
    }
    let mut best = None;
    for entry in data[6..header_end].as_chunks::<16>().0 {
        check()?;
        let width = if entry[0] == 0 {
            256
        } else {
            entry[0] as usize
        };
        let height = if entry[1] == 0 {
            256
        } else {
            entry[1] as usize
        };
        let length = le32(&entry[8..]) as usize;
        let offset = le32(&entry[12..]) as usize;
        let end = offset
            .checked_add(length)
            .filter(|&n| n <= data.len())
            .ok_or("ICO/CUR image extends beyond the file.")?;
        if entry[3] != 0 || length == 0 || offset < header_end {
            return Err("Invalid ICO/CUR image directory entry.".into());
        }
        if cursor {
            if le16(&entry[4..]) as usize >= width || le16(&entry[6..]) as usize >= height {
                return Err("CUR hotspot is outside its image.".into());
            }
        } else if le16(&entry[4..]) > 1 {
            return Err("ICO image must have one plane.".into());
        }
        let payload = &data[offset..end];
        let depth = if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
            32
        } else if payload.len() >= 12 && le32(payload) == 12 {
            le16(&payload[10..]) as usize
        } else if payload.len() >= 16 {
            le16(&payload[14..]) as usize
        } else {
            0
        };
        let rank = (width * height, depth);
        if best
            .as_ref()
            .is_none_or(|&(previous, _, _, _)| rank > previous)
        {
            best = Some((rank, width, height, payload));
        }
    }
    let (_, width, height, payload) = best.ok_or("ICO/CUR has no images.")?;
    if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
        // Check before allocating/decompressing; a small directory entry cannot
        // hide a much larger PNG image. The PNG decoder validates the full file.
        if payload.len() < 24
            || super::be32(&payload[16..]) as usize != width
            || super::be32(&payload[20..]) as usize != height
        {
            return Err("ICO/CUR PNG dimensions do not match the directory.".into());
        }
        super::png::decode(payload, check)
    } else {
        Dib::read(payload, None, Some((width, height)))?.decode(check)
    }
}
