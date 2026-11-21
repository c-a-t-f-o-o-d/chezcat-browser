//! Original TGA and PCX readers. No external decoder is called.
//! Format references: Truevision TGA File Format 2.0 (1991) and the
//! ZSoft PCX Technical Reference Manual, revision 5 (format sections only).
use super::{dimensions, le16, le32, pixel, reserved, Bitmap, ImageResult};

const TGA_SIGNATURE: &[u8] = b"TRUEVISION-XFILE.\0";
const MAX_PCX_RASTER: usize = 64 * 1024 * 1024;

pub(super) fn looks_like_pcx(data: &[u8]) -> bool {
    data.len() >= 128
        && data[0] == 10
        && matches!(data[1], 0 | 2 | 3 | 4 | 5)
        && data[2] <= 1
        && matches!(data[3], 1 | 2 | 4 | 8)
        && (1..=4).contains(&data[65])
}

pub(super) fn looks_like_tga(data: &[u8]) -> bool {
    if data.len() >= 26 && data.ends_with(TGA_SIGNATURE) {
        return true;
    }
    // Original TGA has no magic. Only consider its narrowly validated header
    // after other formats, and require an exact byte size for uncompressed data.
    if data.len() < 18 || data[1] > 1 || data[17] & 0xc0 != 0 {
        return false;
    }
    let kind = data[2];
    let base = if kind >= 8 { kind - 8 } else { kind };
    let depth = data[16];
    let layout = match base {
        1 => data[1] == 1 && matches!(depth, 8 | 16),
        2 => matches!(depth, 15 | 16 | 24 | 32),
        3 => matches!(depth, 8 | 16),
        _ => false,
    };
    if !matches!(kind, 1 | 2 | 3 | 9 | 10 | 11) || !layout {
        return false;
    }
    if data[1] == 0 && data[3..8] != [0; 5] {
        return false;
    }
    let entries = usize::from(le16(&data[5..]));
    let first = usize::from(le16(&data[3..]));
    let palette = if data[1] == 1 {
        if entries == 0 || first + entries > 65536 || !matches!(data[7], 15 | 16 | 24 | 32) {
            return false;
        }
        entries * usize::from(data[7]).div_ceil(8)
    } else {
        0
    };
    let width = usize::from(le16(&data[12..]));
    let height = usize::from(le16(&data[14..]));
    if width == 0 || height == 0 {
        return false;
    }
    let start = 18 + usize::from(data[0]) + palette;
    if kind < 8 {
        width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(usize::from(depth).div_ceil(8)))
            .and_then(|n| start.checked_add(n))
            == Some(data.len())
    } else {
        start < data.len()
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    end: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, bytes: usize) -> ImageResult<&'a [u8]> {
        let end = self
            .at
            .checked_add(bytes)
            .filter(|&n| n <= self.end)
            .ok_or("Truncated legacy image data.")?;
        let slice = &self.data[self.at..end];
        self.at = end;
        Ok(slice)
    }
    fn byte(&mut self) -> ImageResult<u8> {
        Ok(self.take(1)?[0])
    }
}

#[derive(Clone, Copy)]
enum Alpha {
    Opaque,
    Straight,
    Premultiplied,
}

fn tga_color(sample: &[u8], depth: u8, gray: bool, alpha: Alpha) -> u32 {
    let (mut r, mut g, mut b, a) = if gray {
        (
            sample[0],
            sample[0],
            sample[0],
            if depth == 16 { sample[1] } else { 255 },
        )
    } else if depth <= 16 {
        let value = le16(sample);
        let expand = |shift: u32| (((u32::from(value) >> shift) & 31u32) * 255 + 15) / 31;
        (
            expand(10) as u8,
            expand(5) as u8,
            expand(0) as u8,
            if depth == 16 && value & 0x8000 == 0 {
                0
            } else {
                255
            },
        )
    } else {
        (
            sample[2],
            sample[1],
            sample[0],
            if depth == 32 { sample[3] } else { 255 },
        )
    };
    let a = if matches!(alpha, Alpha::Opaque) {
        255
    } else {
        a
    };
    if matches!(alpha, Alpha::Premultiplied) {
        let straight = |v: u8| {
            if a == 0 {
                0
            } else {
                ((u32::from(v) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
            }
        };
        r = straight(r);
        g = straight(g);
        b = straight(b);
    }
    pixel(r, g, b, a)
}

/// Only range-check optional metadata; it never causes a read outside this file.
fn metadata_range(
    start: usize,
    length: usize,
    footer: usize,
    raster_end: &mut usize,
) -> ImageResult<()> {
    if start < 18 || start.checked_add(length).is_none_or(|end| end > footer) {
        return Err("Invalid TGA metadata offset or size.".into());
    }
    *raster_end = (*raster_end).min(start);
    Ok(())
}

pub(super) fn tga(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    check()?;
    if data.len() < 18 {
        return Err("Truncated TGA header.".into());
    }
    let kind = data[2];
    if !matches!(kind, 1 | 2 | 3 | 9 | 10 | 11) {
        return Err("TGA supports mapped, true-color or grayscale images, raw or RLE.".into());
    }
    if data[1] > 1 || data[17] & 0xc0 != 0 {
        return Err(
            "Unsupported TGA color-map type or interleaving/reserved descriptor bits.".into(),
        );
    }
    let base = if kind >= 8 { kind - 8 } else { kind };
    let depth = data[16];
    let width = usize::from(le16(&data[12..]));
    let height = usize::from(le16(&data[14..]));
    let count = dimensions(width, height)?;
    let valid = match base {
        1 => data[1] == 1 && matches!(depth, 8 | 16),
        2 => matches!(depth, 15 | 16 | 24 | 32),
        3 => matches!(depth, 8 | 16),
        _ => false,
    };
    if !valid {
        return Err("Unsupported TGA sample layout or pixel depth.".into());
    }
    let first = usize::from(le16(&data[3..]));
    let entries = usize::from(le16(&data[5..]));
    if data[1] == 0 && data[3..8] != [0; 5] {
        return Err("TGA without a color map must clear its color-map description.".into());
    }
    if data[1] == 1
        && (entries == 0 || first + entries > 65536 || !matches!(data[7], 15 | 16 | 24 | 32))
    {
        return Err("Invalid TGA color-map bounds or depth.".into());
    }
    let color_depth = if base == 1 { data[7] } else { depth };
    let alpha_bits = match (base == 3, color_depth) {
        (true, 16) | (false, 32) => 8,
        (false, 16) => 1,
        _ => 0,
    };
    let declared_alpha = data[17] & 15;
    if declared_alpha != 0 && declared_alpha != alpha_bits {
        return Err("TGA attribute bits do not match its color layout.".into());
    }
    let sample_bytes = usize::from(depth).div_ceil(8);
    let mut raster_end = data.len();
    let mut alpha = if declared_alpha == 0 {
        Alpha::Opaque
    } else {
        Alpha::Straight
    };
    let has_footer = data.len() >= 26 && data.ends_with(TGA_SIGNATURE);
    if has_footer {
        let footer = data.len() - 26;
        raster_end = footer;
        let extension = le32(&data[footer..]) as usize;
        let directory = le32(&data[footer + 4..]) as usize;
        if extension != 0 {
            metadata_range(extension, 495, footer, &mut raster_end)?;
            let length = usize::from(le16(&data[extension..]));
            if length < 495 {
                return Err("Unsupported short TGA extension area.".into());
            }
            metadata_range(extension, length, footer, &mut raster_end)?;
            alpha = match data[extension + 494] {
                0..=2 => Alpha::Opaque,
                3 if alpha_bits != 0 => Alpha::Straight,
                4 if alpha_bits != 0 => Alpha::Premultiplied,
                _ => return Err("Unsupported or inconsistent TGA alpha attributes type.".into()),
            };
            for (field, bytes) in [(482, 2048), (490, height * 4)] {
                let offset = le32(&data[extension + field..]) as usize;
                if offset != 0 {
                    metadata_range(offset, bytes, footer, &mut raster_end)?;
                }
            }
            let stamp = le32(&data[extension + 486..]) as usize;
            if stamp != 0 {
                metadata_range(stamp, 2, footer, &mut raster_end)?;
                let length =
                    2 + usize::from(data[stamp]) * usize::from(data[stamp + 1]) * sample_bytes;
                metadata_range(stamp, length, footer, &mut raster_end)?;
            }
        }
        if directory != 0 {
            metadata_range(directory, 2, footer, &mut raster_end)?;
            let tags = usize::from(le16(&data[directory..]));
            // The u16 count and checked ten-byte records bound this source scan;
            // no per-tag metadata is retained, and cancellation remains active.
            metadata_range(directory, 2 + tags * 10, footer, &mut raster_end)?;
            for n in 0..tags {
                if n & 255 == 0 {
                    check()?;
                }
                let p = directory + 2 + n * 10;
                let offset = le32(&data[p + 2..]) as usize;
                let length = le32(&data[p + 6..]) as usize;
                metadata_range(offset, length, footer, &mut raster_end)?;
            }
        }
    }
    let mut reader = Reader {
        data,
        at: 18,
        end: raster_end,
    };
    reader.take(usize::from(data[0]))?;
    let mut palette = Vec::new();
    if data[1] == 1 {
        palette = reserved(entries)?;
        let bytes = usize::from(data[7]).div_ceil(8);
        for n in 0..entries {
            if n & 1023 == 0 {
                check()?;
            }
            let value = reader.take(bytes)?;
            palette.push(tga_color(value, data[7], false, alpha));
        }
    }
    let mut pixels = reserved(count)?;
    pixels.resize(count, 0);
    let mut written = 0;
    while written < count {
        check()?;
        let (length, repeated) = if kind >= 8 {
            let packet = reader.byte()?;
            (usize::from(packet & 127) + 1, packet & 128 != 0)
        } else {
            ((count - written).min(1024), false)
        };
        if length > count - written {
            return Err("TGA packet exceeds the image dimensions.".into());
        }
        let mut color = 0;
        for n in 0..length {
            if n == 0 || !repeated {
                let sample = reader.take(sample_bytes)?;
                color = if base == 1 {
                    let index = if depth == 8 {
                        usize::from(sample[0])
                    } else {
                        usize::from(le16(sample))
                    };
                    *index
                        .checked_sub(first)
                        .and_then(|index| palette.get(index))
                        .ok_or("TGA pixel refers outside its color map.")?
                } else {
                    tga_color(sample, depth, base == 3, alpha)
                };
            }
            let x = written % width;
            let y = written / width;
            let x = if data[17] & 16 == 0 { x } else { width - 1 - x };
            let y = if data[17] & 32 != 0 {
                y
            } else {
                height - 1 - y
            };
            pixels[y * width + x] = color;
            written += 1;
        }
    }
    if reader.at != raster_end {
        return Err("TGA raster has unexplained trailing data before its metadata/footer.".into());
    }
    check()?;
    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

pub(super) fn pcx(data: &[u8], check: &dyn Fn() -> ImageResult<()>) -> ImageResult<Bitmap> {
    check()?;
    if !looks_like_pcx(data) || data[64] != 0 {
        return Err("Invalid or unsupported PCX header.".into());
    }
    let xmin = usize::from(le16(&data[4..]));
    let ymin = usize::from(le16(&data[6..]));
    let xmax = usize::from(le16(&data[8..]));
    let ymax = usize::from(le16(&data[10..]));
    let width = xmax
        .checked_sub(xmin)
        .ok_or("Reversed PCX horizontal bounds.")?
        + 1;
    let height = ymax
        .checked_sub(ymin)
        .ok_or("Reversed PCX vertical bounds.")?
        + 1;
    let count = dimensions(width, height)?;
    let bits = usize::from(data[3]);
    let planes = usize::from(data[65]);
    let line = usize::from(le16(&data[66..]));
    if line == 0 || line & 1 != 0 || line < (width * bits).div_ceil(8) {
        return Err("PCX BytesPerLine must be even and large enough for one plane.".into());
    }
    let rgb = bits == 8 && planes == 3;
    let indexed = bits * planes <= 4 || bits == 8 && planes == 1;
    if !rgb && !indexed {
        return Err("Unsupported PCX plane/bit-depth combination.".into());
    }
    if rgb && data[1] != 5 {
        return Err("True-color PCX requires a version-5 header.".into());
    }
    if indexed && bits * planes > 1 && matches!(data[1], 0 | 3) {
        return Err("PCX implicit CGA/EGA palettes from old versions are not implemented.".into());
    }
    if bits == 2 && planes == 1 && data[1] < 5 {
        return Err(
            "Legacy PCX CGA palette flags are not implemented; RGB palette entries are supported."
                .into(),
        );
    }
    let row_bytes = line * planes;
    if row_bytes
        .checked_mul(height)
        .is_none_or(|n| n > MAX_PCX_RASTER)
    {
        return Err("PCX padded scanline expansion exceeds 64 MiB.".into());
    }
    let mut palette = vec![0u32; 256];
    for (n, color) in palette[..16].iter_mut().enumerate() {
        let p = 16 + n * 3;
        *color = pixel(data[p], data[p + 1], data[p + 2], 255);
    }
    if bits == 1 && planes == 1 {
        // The monochrome mode has fixed black/white samples. Common writers
        // intentionally leave all sixteen header palette entries zero.
        palette[0] = 0xff000000;
        palette[1] = 0xffffffff;
    }
    let mut end = data.len();
    if bits == 8 && planes == 1 {
        if data.len() >= 128 + 769 && data[data.len() - 769] == 12 {
            end -= 769;
            for (n, color) in palette.iter_mut().enumerate() {
                let p = end + 1 + n * 3;
                *color = pixel(data[p], data[p + 1], data[p + 2], 255);
            }
        } else if le16(&data[68..]) == 2 {
            for (n, color) in palette.iter_mut().enumerate() {
                *color = pixel(n as u8, n as u8, n as u8, 255);
            }
        } else {
            return Err("Eight-bit indexed PCX requires its 256-entry trailing palette.".into());
        }
    }
    let mut reader = Reader { data, at: 128, end };
    let mut row = reserved(row_bytes)?;
    row.resize(row_bytes, 0u8);
    let mut pixels = reserved(count)?;
    for _ in 0..height {
        check()?;
        if data[2] == 0 {
            row.copy_from_slice(reader.take(row_bytes)?);
        } else {
            let mut at = 0;
            let mut next_check = 0;
            while at < row_bytes {
                if at >= next_check {
                    check()?;
                    next_check = at + 4096;
                }
                let value = reader.byte()?;
                let (length, value) = if value & 0xc0 == 0xc0 {
                    (usize::from(value & 63), reader.byte()?)
                } else {
                    (1, value)
                };
                if length == 0 || length > row_bytes - at {
                    return Err("PCX RLE run is empty or crosses a scanline boundary.".into());
                }
                row[at..at + length].fill(value);
                at += length;
            }
        }
        for x in 0..width {
            if x & 1023 == 0 {
                check()?;
            }
            let color = if rgb {
                pixel(row[x], row[line + x], row[line * 2 + x], 255)
            } else if bits == 8 {
                palette[usize::from(row[x])]
            } else {
                let bit = x * bits;
                let mut value = 0usize;
                for plane in 0..planes {
                    let sample = (usize::from(row[plane * line + bit / 8]) >> (8 - bits - bit % 8))
                        & ((1 << bits) - 1);
                    value |= sample << (plane * bits);
                }
                palette[value]
            };
            pixels.push(color);
        }
    }
    if reader.at != end {
        return Err("PCX raster has unexplained trailing data.".into());
    }
    check()?;
    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tga_with_developer_tags(tags: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 18];
        bytes[2] = 2;
        bytes[12] = 1;
        bytes[14] = 1;
        bytes[16] = 24;
        bytes[17] = 32;
        bytes.extend_from_slice(&[0x56, 0x34, 0x12]);
        let metadata = bytes.len() as u32;
        bytes.push(42);
        let directory = bytes.len() as u32;
        bytes.extend_from_slice(&tags.to_le_bytes());
        for tag in 0..tags {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&metadata.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
        }
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&directory.to_le_bytes());
        bytes.extend_from_slice(TGA_SIGNATURE);
        bytes
    }

    #[test]
    fn tga_developer_directory_accepts_the_format_maximum_count() {
        let data = tga_with_developer_tags(u16::MAX);
        let bitmap = crate::image::decode_checked(&data, &|| Ok(())).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (1, 1));
        assert_eq!(bitmap.pixels, [0xff123456]);
    }

    #[test]
    fn tga_developer_directory_preserves_late_extent_validation() {
        let data = tga_with_developer_tags(4097);
        let footer = data.len() - 26;
        let directory = le32(&data[footer + 4..]) as usize;
        let last = directory + 2 + 4096 * 10;
        let mut bad_offset = data.clone();
        bad_offset[last + 2..last + 6].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(tga(&bad_offset, &|| Ok(()))
            .unwrap_err()
            .contains("metadata offset or size"));

        let mut bad_length = data.clone();
        bad_length[last + 6..last + 10].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(tga(&bad_length, &|| Ok(()))
            .unwrap_err()
            .contains("metadata offset or size"));

        let mut truncated = data;
        truncated.remove(footer - 1);
        assert!(tga(&truncated, &|| Ok(()))
            .unwrap_err()
            .contains("metadata offset or size"));
    }

    #[test]
    fn tga_developer_directory_scans_remain_cancelable() {
        let data = tga_with_developer_tags(u16::MAX);
        let calls = std::cell::Cell::new(0usize);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() >= 20 {
                Err("cancelled during TGA developer directory scan".into())
            } else {
                Ok(())
            }
        };
        assert_eq!(
            tga(&data, &check).unwrap_err(),
            "cancelled during TGA developer directory scan"
        );
    }

    #[test]
    fn tga_original_truecolor_is_not_misidentified_as_a_cursor() {
        let mut bytes = vec![0u8; 18];
        bytes[2] = 2;
        bytes[12] = 2;
        bytes[14] = 1;
        bytes[16] = 24;
        bytes[17] = 32;
        bytes.extend([0, 0, 255, 0, 255, 0]);
        assert!(looks_like_tga(&bytes));
        assert_eq!(
            super::super::decode(&bytes).unwrap().pixels,
            [0xffff0000, 0xff00ff00]
        );
        bytes.push(0);
        assert!(!looks_like_tga(&bytes));
    }

    #[test]
    fn tga_alpha_metadata_and_orientation_are_honored() {
        let mut bytes = vec![0u8; 18];
        bytes[2] = 10;
        bytes[12] = 2;
        bytes[14] = 1;
        bytes[16] = 32;
        bytes[17] = 0x38;
        bytes.extend([1, 0, 0, 64, 128, 0, 255, 0, 255]);
        let extension = bytes.len();
        bytes.extend([0; 495]);
        bytes[extension..extension + 2].copy_from_slice(&495u16.to_le_bytes());
        bytes[extension + 494] = 4;
        bytes.extend_from_slice(&(extension as u32).to_le_bytes());
        bytes.extend([0; 4]);
        bytes.extend_from_slice(TGA_SIGNATURE);
        assert_eq!(
            tga(&bytes, &|| Ok(())).unwrap().pixels,
            [0xff00ff00, 0x80800000]
        );
        bytes[extension + 494] = 0;
        assert_eq!(
            tga(&bytes, &|| Ok(())).unwrap().pixels,
            [0xff00ff00, 0xff400000]
        );
    }

    #[test]
    fn pcx_runs_may_cross_planes_but_not_scanlines() {
        let mut bytes = vec![0u8; 128];
        bytes[0..4].copy_from_slice(&[10, 5, 1, 8]);
        bytes[8] = 1;
        bytes[65] = 3;
        bytes[66] = 2;
        bytes.extend([0xc6, 42]);
        assert_eq!(pcx(&bytes, &|| Ok(())).unwrap().pixels, [0xff2a2a2a; 2]);
        bytes[128] = 0xc7;
        assert!(pcx(&bytes, &|| Ok(())).is_err());
        bytes[128] = 0xc0;
        assert!(pcx(&bytes, &|| Ok(())).is_err());
    }
}
