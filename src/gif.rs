//! Original bounded GIF87a/GIF89a decoder; no external decoder is used.
//! Format reference: <https://www.w3.org/Graphics/GIF/spec-gif89a.txt>.
//!
//! Static callers retain the first image; animated callers receive composited
//! frames with delays, looping and disposal. Transparent samples preserve the
//! current canvas. Application extensions are bounded data, never executed.
//! Compatibility is limited to zero-byte padding after LZW end-of-information,
//! and a missing/partial end code when a real data terminator follows a complete
//! raster. Truncated blocks, short/long rasters and missing trailers still fail.

use crate::image::{
    Animation, Bitmap, DecodedImage, ImageFrame, MAX_ANIMATION_PIXELS, MAX_INPUT_BYTES, MAX_PIXELS,
};
use std::sync::Arc;

type Result<T> = std::result::Result<T, String>;
type Check<'a> = &'a dyn Fn() -> Result<()>;
const MAX_DIMENSION: usize = 16_384;
const MAX_IMAGES: usize = 256;
const MAX_BLOCKS: usize = 65_536;
const MAX_WORK: usize = 32 * 1024 * 1024;
const MAX_ANIMATION_WORK: usize = 128 * 1024 * 1024;

struct Work<'a> {
    remaining: usize,
    until_check: usize,
    check: Check<'a>,
}

impl Work<'_> {
    fn spend(&mut self, count: usize) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or("GIF decoding work limit reached.")?;
        if count >= self.until_check {
            (self.check)()?;
            self.until_check = 1024;
        } else {
            self.until_check -= count;
        }
        Ok(())
    }
}

struct Reader<'a> {
    data: &'a [u8],
    position: usize,
    blocks: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .filter(|&end| end <= self.data.len())
            .ok_or("Truncated GIF block.")?;
        let data = &self.data[self.position..end];
        self.position = end;
        Ok(data)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn block(&mut self, work: &mut Work<'_>) -> Result<&'a [u8]> {
        self.blocks += 1;
        if self.blocks > MAX_BLOCKS {
            return Err("GIF sub-block limit reached (65536).".into());
        }
        let length = usize::from(self.byte()?);
        work.spend(1 + length)?;
        self.bytes(length)
    }

    fn skip_blocks(&mut self, work: &mut Work<'_>) -> Result<()> {
        while !self.block(work)?.is_empty() {}
        Ok(())
    }

    fn palette(&mut self, size: usize, work: &mut Work<'_>) -> Result<Vec<u32>> {
        work.spend(size)?;
        let bytes = self.bytes(size * 3)?;
        Ok(bytes
            .as_chunks::<3>()
            .0
            .iter()
            .map(|rgb| {
                0xff00_0000
                    | (u32::from(rgb[0]) << 16)
                    | (u32::from(rgb[1]) << 8)
                    | u32::from(rgb[2])
            })
            .collect())
    }
}

fn word(bytes: &[u8]) -> usize {
    usize::from(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn dimensions(width: usize, height: usize) -> Result<usize> {
    let count = width
        .checked_mul(height)
        .ok_or("GIF dimensions overflow.")?;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || count > MAX_PIXELS
    {
        return Err("GIF exceeds the 4-megapixel or 16384-pixel dimension limit.".into());
    }
    Ok(count)
}

#[derive(Clone, Copy, Default)]
struct Control {
    transparent: Option<u8>,
    delay_ms: u32,
    disposal: u8,
    user_input: bool,
}

/// Reads LSB-first codes through the original GIF sub-block boundaries without
/// concatenating the compressed bytes into another allocation.
struct Bits<'a, 'b> {
    reader: &'b mut Reader<'a>,
    block: &'a [u8],
    offset: usize,
    value: u32,
    count: u8,
}

impl Bits<'_, '_> {
    fn code(&mut self, width: u8, work: &mut Work<'_>) -> Result<Option<usize>> {
        while self.count < width {
            if self.offset == self.block.len() {
                self.block = self.reader.block(work)?;
                self.offset = 0;
                if self.block.is_empty() {
                    // A real zero-length sub-block, unlike raw EOF or a short
                    // declared block, can terminate a complete legacy raster.
                    return Ok(None);
                }
            }
            self.value |= u32::from(self.block[self.offset]) << self.count;
            self.offset += 1;
            self.count += 8;
        }
        let value = self.value & ((1 << width) - 1);
        self.value >>= width;
        self.count -= width;
        Ok(Some(value as usize))
    }

    fn finish(&mut self, work: &mut Work<'_>) -> Result<()> {
        // Unused bits in the byte containing EOI need no particular value.
        // Some older encoders also flush whole zero bytes. Accept only those
        // bytes, retaining checked sub-block lengths, work/cancel limits and
        // the required zero-length terminator; arbitrary tail data still fails.
        if self.block[self.offset..].iter().any(|&byte| byte != 0) {
            return Err("Extra GIF image data follows the LZW end code.".into());
        }
        loop {
            let block = self.reader.block(work)?;
            if block.is_empty() {
                return Ok(());
            }
            if block.iter().any(|&byte| byte != 0) {
                return Err("Extra GIF image data follows the LZW end code.".into());
            }
        }
    }
}

struct Raster<'a> {
    pixels: Option<&'a mut [u32]>,
    palette: &'a [u32],
    transparent: Option<u8>,
    screen_width: usize,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    x: usize,
    row: usize,
    pass: usize,
    interlaced: bool,
    written: usize,
}

impl Raster<'_> {
    fn write(&mut self, index: u8) -> Result<()> {
        if self.written >= self.width * self.height {
            return Err("GIF LZW output exceeds its image dimensions.".into());
        }
        // A transparent sample leaves the display unchanged; it has no color
        // lookup. Optimizing encoders may use the first index after the table
        // for this purpose while all visible samples still require an entry.
        if self.transparent != Some(index) {
            let &color = self
                .palette
                .get(usize::from(index))
                .ok_or("GIF pixel index is outside its active color table.")?;
            if let Some(pixels) = &mut self.pixels {
                let position = (self.top + self.row) * self.screen_width + self.left + self.x;
                pixels[position] = color;
            }
        }
        self.written += 1;
        self.x += 1;
        if self.x == self.width {
            self.x = 0;
            if self.interlaced {
                const STARTS: [usize; 4] = [0, 4, 2, 1];
                const STEPS: [usize; 4] = [8, 8, 4, 2];
                self.row += STEPS[self.pass];
                while self.row >= self.height && self.pass < 3 {
                    self.pass += 1;
                    self.row = STARTS[self.pass];
                }
            } else {
                self.row += 1;
            }
        }
        Ok(())
    }
}

fn lzw(reader: &mut Reader<'_>, raster: &mut Raster<'_>, work: &mut Work<'_>) -> Result<()> {
    let minimum = reader.byte()?;
    if !(2..=8).contains(&minimum) {
        return Err("GIF LZW minimum code size must be between 2 and 8.".into());
    }
    let clear = 1usize << minimum;
    let end = clear + 1;
    let mut next = end + 1;
    let mut width = minimum + 1;
    let mut previous: Option<usize> = None;
    // Entries only point to an older entry. No reset-time clearing is needed:
    // the active `next` boundary makes the old dictionary unreachable.
    let mut prefixes = [0u16; 4096];
    let mut suffixes = [0u8; 4096];
    let mut stack = [0u8; 4096];
    let mut bits = Bits {
        reader,
        block: &[],
        offset: 0,
        value: 0,
        count: 0,
    };
    loop {
        work.spend(1)?;
        let Some(code) = bits.code(width, work)? else {
            // The zero-length data terminator has already been consumed. This
            // tolerates an omitted/partial EOI only after every pixel was
            // decoded; it never accepts raw EOF or suppresses decoder errors.
            return if raster.written == raster.width * raster.height {
                Ok(())
            } else {
                Err("GIF LZW end code is missing before all image pixels were decoded.".into())
            };
        };
        if code == clear {
            width = minimum + 1;
            next = end + 1;
            previous = None;
            continue;
        }
        if code == end {
            if raster.written != raster.width * raster.height {
                return Err("GIF LZW output is shorter than its image dimensions.".into());
            }
            return bits.finish(work);
        }
        if code > next || (code == next && previous.is_none()) {
            return Err("GIF LZW refers to an undefined dictionary entry.".into());
        }
        let special = code == next;
        let mut entry = if special { previous.unwrap() } else { code };
        let mut length = 0;
        while entry >= clear {
            if entry <= end || entry >= next || length >= stack.len() {
                return Err("Invalid GIF LZW dictionary chain.".into());
            }
            work.spend(1)?;
            stack[length] = suffixes[entry];
            length += 1;
            entry = usize::from(prefixes[entry]);
        }
        let first = entry as u8;
        if length == stack.len() {
            return Err("GIF LZW dictionary expansion exceeds its limit.".into());
        }
        stack[length] = first;
        length += 1;
        work.spend(length + usize::from(special))?;
        for &index in stack[..length].iter().rev() {
            raster.write(index)?;
        }
        if special {
            raster.write(first)?;
        }
        if let Some(old) = previous {
            if next < 4096 {
                prefixes[next] = old as u16;
                suffixes[next] = first;
                next += 1;
                if next == 1usize << width && width < 12 {
                    width += 1;
                }
            }
        }
        previous = Some(code);
    }
}

pub(super) fn decode(data: &[u8], check: Check<'_>) -> Result<Bitmap> {
    let image = decode_internal(data, check, false)?;
    Arc::try_unwrap(image.bitmap)
        .map_err(|_| "GIF static decoder unexpectedly retained its output.".into())
}

pub(super) fn decode_asset(data: &[u8], check: Check<'_>) -> Result<DecodedImage> {
    decode_internal(data, check, true)
}

fn copy_pixels(source: &[u32], work: &mut Work<'_>) -> Result<Vec<u32>> {
    let mut target = Vec::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| "Not enough memory for GIF frame pixels.")?;
    for part in source.chunks(4096) {
        work.spend(part.len())?;
        target.extend_from_slice(part);
    }
    Ok(target)
}

fn decode_internal(data: &[u8], check: Check<'_>, animated: bool) -> Result<DecodedImage> {
    check()?;
    if data.len() > MAX_INPUT_BYTES {
        return Err("GIF input exceeds the 32 MiB limit.".into());
    }
    let mut reader = Reader {
        data,
        position: 0,
        blocks: 0,
    };
    if !matches!(reader.bytes(6)?, b"GIF87a" | b"GIF89a") {
        return Err("Invalid GIF signature or version.".into());
    }
    let descriptor = reader.bytes(7)?;
    let width = word(descriptor);
    let height = word(&descriptor[2..]);
    let count = dimensions(width, height)?;
    let mut work = Work {
        remaining: if animated {
            MAX_ANIMATION_WORK
        } else {
            MAX_WORK
        },
        until_check: 1,
        check,
    };
    let global = if descriptor[4] & 0x80 != 0 {
        reader.palette(1 << ((descriptor[4] & 7) + 1), &mut work)?
    } else {
        Vec::new()
    };
    let background = if global.is_empty() {
        0
    } else {
        *global
            .get(usize::from(descriptor[5]))
            .ok_or("GIF background index is outside its global color table.")?
    };
    let mut pixels: Option<Vec<u32>> = None;
    let mut poster: Option<Arc<Bitmap>> = None;
    let mut frames = Vec::new();
    let mut retain_frames = animated;
    let mut notice = None;
    let mut plays = Some(1);
    let mut seen_loop = false;
    let mut control: Option<Control> = None;
    let mut images = 0;
    loop {
        work.spend(1)?;
        match reader.byte()? {
            0x3b => {
                if reader.position != data.len() || control.is_some() {
                    return Err("GIF has trailing data or an unused graphic control block.".into());
                }
                check()?;
                return Ok(DecodedImage {
                    bitmap: poster.ok_or("GIF contains no image.")?,
                    animation: if retain_frames && frames.len() > 1 {
                        Some(Arc::new(Animation { frames, plays }))
                    } else {
                        None
                    },
                    notice,
                });
            }
            0x21 => match reader.byte()? {
                0xf9 => {
                    if control.is_some() || reader.byte()? != 4 {
                        return Err("Invalid or duplicate GIF graphic control block.".into());
                    }
                    let fields = reader.bytes(4)?;
                    if fields[0] & 0xe0 != 0 || (fields[0] >> 2) & 7 > 3 {
                        return Err(
                            "GIF graphic control has reserved bits or disposal method.".into()
                        );
                    }
                    control = Some(Control {
                        transparent: (fields[0] & 1 != 0).then_some(fields[3]),
                        delay_ms: word(&fields[1..]) as u32 * 10,
                        disposal: (fields[0] >> 2) & 7,
                        user_input: fields[0] & 2 != 0,
                    });
                    if reader.byte()? != 0 {
                        return Err("GIF graphic control terminator is missing.".into());
                    }
                }
                0xfe => reader.skip_blocks(&mut work)?,
                0xff => {
                    if reader.byte()? != 11 {
                        return Err("GIF application header must contain 11 bytes.".into());
                    }
                    let identifier = reader.bytes(11)?;
                    let loop_extension =
                        animated && matches!(identifier, b"NETSCAPE2.0" | b"ANIMEXTS1.0");
                    loop {
                        let block = reader.block(&mut work)?;
                        if block.is_empty() {
                            break;
                        }
                        if loop_extension && block[0] == 1 {
                            if block.len() != 3 || seen_loop {
                                return Err("Invalid or duplicate GIF loop extension.".into());
                            }
                            let loops = word(&block[1..]) as u32;
                            plays = if loops == 0 { None } else { Some(loops + 1) };
                            seen_loop = true;
                        }
                    }
                }
                0x01 => return Err("GIF plain-text graphics are not implemented.".into()),
                _ => return Err("Unsupported GIF extension type.".into()),
            },
            0x2c => {
                images += 1;
                if images > MAX_IMAGES {
                    return Err("GIF image count exceeds 256.".into());
                }
                let descriptor = reader.bytes(9)?;
                let left = word(descriptor);
                let top = word(&descriptor[2..]);
                let image_width = word(&descriptor[4..]);
                let image_height = word(&descriptor[6..]);
                dimensions(image_width, image_height)?;
                if left + image_width > width || top + image_height > height {
                    return Err("GIF image rectangle extends outside the logical screen.".into());
                }
                let packed = descriptor[8];
                if packed & 0x18 != 0 {
                    return Err("GIF image descriptor contains reserved bits.".into());
                }
                let local = if packed & 0x80 != 0 {
                    reader.palette(1 << ((packed & 7) + 1), &mut work)?
                } else {
                    Vec::new()
                };
                let palette = if local.is_empty() { &global } else { &local };
                if palette.is_empty() {
                    return Err("GIF image has no color table.".into());
                }
                let control = control.take().unwrap_or_default();
                if animated && control.user_input {
                    return Err("GIF user-input waits are not implemented.".into());
                }
                if retain_frames
                    && count
                        .checked_mul(images)
                        .is_none_or(|n| n > MAX_ANIMATION_PIXELS)
                {
                    retain_frames = false;
                    frames.clear();
                    notice = Some("GIF animation exceeds the retained-frame pixel limit; showing its first frame after validating all frames.".into());
                }
                if images == 1 {
                    let color = control.transparent.map_or(background, |index| {
                        palette.get(usize::from(index)).copied().unwrap_or(0) & 0x00ff_ffff
                    });
                    let mut canvas = Vec::new();
                    canvas
                        .try_reserve_exact(count)
                        .map_err(|_| "Not enough memory for GIF pixels.")?;
                    // Chunked initialization keeps cancellation responsive even
                    // when a tiny image asks for a large logical screen.
                    while canvas.len() < count {
                        let length = (count - canvas.len()).min(4096);
                        work.spend(length)?;
                        canvas.resize(canvas.len() + length, color);
                    }
                    pixels = Some(canvas);
                }
                let mut restore = Vec::new();
                if animated && control.disposal == 3 {
                    restore
                        .try_reserve_exact(image_width * image_height)
                        .map_err(|_| "Not enough memory for GIF restore pixels.")?;
                    let canvas = pixels.as_ref().unwrap();
                    for row in top..top + image_height {
                        work.spend(image_width)?;
                        restore.extend_from_slice(
                            &canvas[row * width + left..row * width + left + image_width],
                        );
                    }
                }
                let mut raster = Raster {
                    pixels: if animated || images == 1 {
                        pixels.as_deref_mut()
                    } else {
                        None
                    },
                    palette,
                    transparent: control.transparent,
                    screen_width: width,
                    left,
                    top,
                    width: image_width,
                    height: image_height,
                    x: 0,
                    row: 0,
                    pass: 0,
                    interlaced: packed & 0x40 != 0,
                    written: 0,
                };
                lzw(&mut reader, &mut raster, &mut work)?;
                if animated {
                    if retain_frames || images == 1 {
                        let frame = Arc::new(Bitmap {
                            width,
                            height,
                            pixels: copy_pixels(pixels.as_ref().unwrap(), &mut work)?,
                        });
                        if images == 1 {
                            poster = Some(frame.clone());
                        }
                        if retain_frames {
                            frames.push(ImageFrame {
                                bitmap: frame,
                                delay_ms: control.delay_ms,
                            });
                        }
                    }
                    let canvas = pixels.as_mut().unwrap();
                    if control.disposal == 2 || control.disposal == 3 {
                        let clear = control.transparent.map_or(background, |index| {
                            palette.get(usize::from(index)).copied().unwrap_or(0) & 0x00ff_ffff
                        });
                        for row in 0..image_height {
                            work.spend(image_width)?;
                            let destination = &mut canvas[(top + row) * width + left
                                ..(top + row) * width + left + image_width];
                            if control.disposal == 2 {
                                destination.fill(clear);
                            } else {
                                destination.copy_from_slice(
                                    &restore[row * image_width..(row + 1) * image_width],
                                );
                            }
                        }
                    }
                } else if images == 1 {
                    poster = Some(Arc::new(Bitmap {
                        width,
                        height,
                        pixels: pixels.take().unwrap(),
                    }));
                }
            }
            _ => return Err("Invalid GIF block introducer or missing trailer.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // Original small fixtures with explicit three-bit LZW codes. Frequent
    // clears deliberately keep that width fixed; the independent checker
    // exercises every width transition and a full dictionary against Pillow.
    fn fixture(width: u16, height: u16, codes: &[u8], transparent: Option<u8>) -> Vec<u8> {
        let mut data = b"GIF89a".to_vec();
        data.extend_from_slice(&width.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data.extend_from_slice(&[
            0x81, 0, 0, 17, 33, 49, 210, 80, 40, 20, 180, 90, 50, 90, 210,
        ]);
        if let Some(index) = transparent {
            data.extend_from_slice(&[0x21, 0xf9, 4, 1, 0, 0, index, 0]);
        }
        data.extend_from_slice(&[0x2c, 0, 0, 0, 0]);
        data.extend_from_slice(&width.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data.extend_from_slice(&[0, 2]);
        let mut packed = Vec::new();
        let mut bits = 0;
        let mut buffer = 0u16;
        for &code in codes {
            buffer |= u16::from(code) << bits;
            bits += 3;
            if bits >= 8 {
                packed.push(buffer as u8);
                buffer >>= 8;
                bits -= 8;
            }
        }
        if bits > 0 {
            packed.push(buffer as u8);
        }
        for part in packed.chunks(255) {
            data.push(part.len() as u8);
            data.extend_from_slice(part);
        }
        data.extend_from_slice(&[0, 0x3b]);
        data
    }

    #[test]
    fn first_image_and_transparent_indices_reach_the_real_dispatcher() {
        let data = fixture(2, 2, &[4, 0, 4, 1, 4, 2, 4, 3, 5], None);
        let bitmap = crate::image::decode(&data).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (2, 2));
        assert_eq!(
            bitmap.pixels,
            [0xff112131, 0xffd25028, 0xff14b45a, 0xff325ad2]
        );
        let transparent = fixture(2, 1, &[4, 0, 4, 1, 5], Some(0));
        assert_eq!(
            decode(&transparent, &|| Ok(())).unwrap().pixels,
            [0x00112131, 0xffd25028]
        );
        let mut gif87 = data;
        gif87[3..6].copy_from_slice(b"87a");
        assert!(decode(&gif87, &|| Ok(())).is_ok());
    }

    #[test]
    fn transparent_samples_do_not_require_a_palette_color() {
        // The two-entry color table does not contain index 3, but a sample of
        // that index is still meaningful when its sole effect is transparency.
        let mut data = fixture(2, 1, &[4, 3, 4, 1, 5], Some(3));
        data[10] = 0x80;
        data.drain(19..25);
        assert_eq!(decode(&data, &|| Ok(())).unwrap().pixels, [0, 0xffd25028]);
        assert_eq!(
            decode_asset(&data, &|| Ok(())).unwrap().bitmap.pixels,
            [0, 0xffd25028]
        );
        // Clearing the transparency flag makes exactly the same sample invalid.
        data[22] = 0;
        assert!(decode_asset(&data, &|| Ok(()))
            .unwrap_err()
            .contains("outside its active color table"));
    }

    #[test]
    fn lzw_special_entry_clears_and_invalid_codes_are_distinct() {
        let special = fixture(3, 1, &[4, 0, 6, 5], None);
        assert_eq!(
            decode(&special, &|| Ok(())).unwrap().pixels,
            [0xff112131; 3]
        );
        let repeated_clear = fixture(1, 1, &[4, 4, 4, 1, 5], None);
        assert_eq!(
            decode(&repeated_clear, &|| Ok(())).unwrap().pixels,
            [0xffd25028]
        );
        for codes in [&[4, 6, 5][..], &[4, 0, 7, 5], &[4, 5], &[4, 0, 1, 5]] {
            assert!(
                decode(&fixture(1, 1, codes, None), &|| Ok(())).is_err(),
                "{codes:?}"
            );
        }
    }

    #[test]
    fn legacy_image_termination_requires_complete_pixels_and_checked_framing() {
        let complete = fixture(1, 1, &[4, 0, 5], None);
        // One whole zero byte in the EOI sub-block, as emitted by some older
        // encoders. Byte 36 is this original fixture's sole sub-block size.
        let mut padded = complete.clone();
        let terminator = padded.len() - 2;
        padded.insert(terminator, 0);
        padded[36] += 1;
        assert_eq!(decode(&padded, &|| Ok(())).unwrap().pixels, [0xff112131]);
        let mut separate = complete[..complete.len() - 2].to_vec();
        separate.extend_from_slice(&[3, 0, 0, 0, 0, 0x3b]);
        assert_eq!(decode(&separate, &|| Ok(())).unwrap().pixels, [0xff112131]);

        let no_end = fixture(1, 1, &[4, 0], None);
        assert_eq!(decode(&no_end, &|| Ok(())).unwrap().pixels, [0xff112131]);
        let mut partial_end = no_end.clone();
        partial_end[37] |= 0x40; // Only the low two bits of the three-bit EOI.
        assert_eq!(
            decode(&partial_end, &|| Ok(())).unwrap().pixels,
            [0xff112131]
        );

        padded[terminator] = 1;
        assert!(decode(&padded, &|| Ok(()))
            .unwrap_err()
            .contains("Extra GIF"));
        separate[complete.len() - 1] = 1;
        assert!(decode(&separate, &|| Ok(()))
            .unwrap_err()
            .contains("Extra GIF"));
        assert!(decode(&fixture(2, 1, &[4, 0], None), &|| Ok(())).is_err());
        assert!(decode(&fixture(1, 1, &[4, 0, 7], None), &|| Ok(())).is_err());
        assert!(decode(&fixture(1, 1, &[4, 0, 0], None), &|| Ok(())).is_err());
        for data in [&complete, &no_end, &partial_end] {
            for removed in [1, 2, 3] {
                assert!(decode(&data[..data.len() - removed], &|| Ok(())).is_err());
            }
        }
        let mut short_block = complete[..complete.len() - 2].to_vec();
        short_block.extend_from_slice(&[5, 0, 0]);
        assert!(decode(&short_block, &|| Ok(()))
            .unwrap_err()
            .contains("Truncated"));
    }

    #[test]
    fn legacy_padding_keeps_subblock_limits_and_cancellation() {
        let complete = fixture(1, 1, &[4, 0, 5], None);
        let mut padded = complete[..complete.len() - 2].to_vec();
        for _ in 0..40 {
            padded.push(255);
            padded.extend_from_slice(&[0; 255]);
        }
        padded.extend_from_slice(&[0, 0x3b]);
        let calls = Cell::new(0);
        let error = decode(&padded, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 6 {
                Err("cancelled in padding".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "cancelled in padding");
        let mut blocks = complete[..complete.len() - 2].to_vec();
        for _ in 0..MAX_BLOCKS {
            blocks.extend_from_slice(&[1, 0]);
        }
        blocks.extend_from_slice(&[0, 0x3b]);
        assert!(decode(&blocks, &|| Ok(()))
            .unwrap_err()
            .contains("sub-block limit"));
    }

    #[test]
    fn interlaced_passes_cover_short_and_odd_height_images_exactly() {
        for height in 1..20u16 {
            let mut codes = Vec::new();
            for (start, step) in [(0, 8), (4, 8), (2, 4), (1, 2)] {
                for row in (start..height).step_by(step) {
                    codes.extend_from_slice(&[4, (row % 4) as u8]);
                }
            }
            codes.push(5);
            let mut data = fixture(1, height, &codes, None);
            data[34] = 0x40;
            let colors = [0xff112131, 0xffd25028, 0xff14b45a, 0xff325ad2];
            let bitmap = decode(&data, &|| Ok(())).unwrap();
            assert_eq!(
                bitmap.pixels,
                (0..height)
                    .map(|row| colors[usize::from(row % 4)])
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn image_rectangles_use_the_bounded_logical_screen_and_background() {
        let mut data = fixture(1, 1, &[4, 0, 5], None);
        data[6..10].copy_from_slice(&[3, 0, 3, 0]);
        data[11] = 2;
        data[26] = 1;
        data[28] = 1;
        let bitmap = decode(&data, &|| Ok(())).unwrap();
        let mut expected = vec![0xff14b45a; 9];
        expected[4] = 0xff112131;
        assert_eq!(bitmap.pixels, expected);
        data[26] = 3;
        assert!(decode(&data, &|| Ok(()))
            .unwrap_err()
            .contains("outside the logical screen"));
        data[6..10].copy_from_slice(&[0, 0, 0, 0]);
        assert!(decode(&data, &|| Ok(()))
            .unwrap_err()
            .contains("dimension limit"));
    }

    #[test]
    fn first_frame_is_retained_but_later_frames_and_the_trailer_are_validated() {
        let first = fixture(1, 1, &[4, 0, 5], None);
        let second = fixture(1, 1, &[4, 1, 5], None);
        let mut animation = first[..first.len() - 1].to_vec();
        animation.extend_from_slice(&second[25..]);
        assert_eq!(decode(&animation, &|| Ok(())).unwrap().pixels, [0xff112131]);
        for length in 0..animation.len() {
            assert!(
                decode(&animation[..length], &|| Ok(())).is_err(),
                "accepted prefix {length}"
            );
        }
        let mut invalid_later = first[..first.len() - 1].to_vec();
        invalid_later.extend_from_slice(&fixture(1, 1, &[4, 7, 5], None)[25..]);
        assert!(decode(&invalid_later, &|| Ok(()))
            .unwrap_err()
            .contains("undefined"));
        let mut many = first[..25].to_vec();
        for _ in 0..257 {
            many.extend_from_slice(&first[25..first.len() - 1]);
        }
        many.push(0x3b);
        assert!(decode(&many, &|| Ok(()))
            .unwrap_err()
            .contains("exceeds 256"));
    }

    #[test]
    fn metadata_scope_and_cancellation_are_bounded() {
        let mut data = fixture(1, 1, &[4, 0, 5], Some(0));
        data.splice(33..33, [0x21, 0xfe, 3, b'a', b'b', b'c', 0]);
        assert_eq!(decode(&data, &|| Ok(())).unwrap().pixels[0] >> 24, 0);
        let calls = Cell::new(0);
        let mut large = fixture(1, 1, &[4, 0, 5], None);
        large[6..10].copy_from_slice(&[0, 8, 0, 8]);
        let error = decode(&large, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 4 {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "cancelled");
        assert_eq!(calls.get(), 4);
        let valid = fixture(1, 1, &[4, 0, 5], None);
        let mut blocks = valid[..25].to_vec();
        blocks.extend_from_slice(&[0x21, 0xfe]);
        for _ in 0..=MAX_BLOCKS {
            blocks.extend_from_slice(&[1, 0]);
        }
        blocks.push(0);
        blocks.extend_from_slice(&valid[25..]);
        assert!(decode(&blocks, &|| Ok(()))
            .unwrap_err()
            .contains("sub-block limit"));
    }
}
