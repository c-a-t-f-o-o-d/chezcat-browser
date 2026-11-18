//! Independently implemented image decoders using only Rust's standard library.
//! PNG supports every standard sample depth/color type, transparency and Adam7.
//! The PNG pixel values are kept as straight alpha; color profiles are not applied.
//! Compression follows the PNG specification and RFC 1950 / RFC 1951.
//! JPEG uses our own bounded Huffman/DCT decoder in jpeg.rs.
//! GIF, APNG and WebP can return bounded, fully composited animation frames.
//! BMP/ICO/CUR, static SVG, VP8/VP8L WebP, TIFF, QOI and Netpbm use original modules.
//! JPEG, PNG/APNG and WebP honor bounded Exif orientation; color management is separate.

#[path = "image_metadata.rs"]
pub(crate) mod metadata;

#[path = "image_bmp.rs"]
mod dib;
#[path = "gif.rs"]
mod gif;
#[path = "image_legacy.rs"]
mod legacy;
#[path = "image_png.rs"]
mod png;
#[path = "image_simple.rs"]
mod simple;
#[path = "image_svg.rs"]
mod svg;
#[path = "image_tiff.rs"]
mod tiff;
#[path = "image_webp.rs"]
mod webp;
pub(crate) use webp::VideoDecoder as Vp8VideoDecoder;

#[cfg(test)]
use crate::compression::adler32;
#[cfg(test)]
use crate::compression::crc32;

pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_PIXELS: usize = 4 * 1024 * 1024;
pub const MAX_ANIMATION_PIXELS: usize = 16 * 1024 * 1024;
pub const MAX_ANIMATION_FRAMES: usize = 256;
const MAX_DIMENSION: usize = 16_384;

type ImageResult<T> = Result<T, String>;

#[derive(Debug)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    /// Straight-alpha pixels, 0xAARRGGBB.
    pub pixels: Vec<u32>,
}

/// Straight-alpha source-over. Work in integer premultiplied numerators, then
/// unpremultiply only once; fully transparent colors never leak into the result.
pub(crate) fn source_over(source: u32, target: u32) -> u32 {
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

/// A fully composited frame. Pixels are immutable and shared with every use of
/// the same resource. Delays come from the file; the player imposes a 20ms floor.
#[derive(Debug)]
pub struct ImageFrame {
    pub bitmap: std::sync::Arc<Bitmap>,
    pub delay_ms: u32,
}

#[derive(Debug)]
pub struct Animation {
    pub frames: Vec<ImageFrame>,
    /// Total plays, not additional repeats. None means indefinite repetition.
    pub plays: Option<u32>,
}

#[derive(Debug)]
pub struct DecodedImage {
    pub bitmap: std::sync::Arc<Bitmap>,
    pub animation: Option<std::sync::Arc<Animation>>,
    /// An explicit static fallback, never a suppressed malformed-image error.
    pub notice: Option<String>,
}

impl DecodedImage {
    pub fn still(bitmap: Bitmap) -> Self {
        Self {
            bitmap: std::sync::Arc::new(bitmap),
            animation: None,
            notice: None,
        }
    }

    pub fn retained_bytes(&self) -> ImageResult<usize> {
        let count = dimensions(self.bitmap.width, self.bitmap.height)?;
        if self.bitmap.pixels.len() != count {
            return Err("Image pixel count does not match its dimensions.".into());
        }
        let mut pixels = count;
        let mut seen = std::collections::HashSet::new();
        seen.insert(std::sync::Arc::as_ptr(&self.bitmap));
        if let Some(animation) = &self.animation {
            if animation.frames.is_empty()
                || animation.frames.len() > MAX_ANIMATION_FRAMES
                || animation.plays == Some(0)
            {
                return Err("Invalid image animation frame count or play count.".into());
            }
            for frame in &animation.frames {
                if frame.bitmap.width != self.bitmap.width
                    || frame.bitmap.height != self.bitmap.height
                    || frame.bitmap.pixels.len() != count
                {
                    return Err("Image animation frames must match their canvas.".into());
                }
                if seen.insert(std::sync::Arc::as_ptr(&frame.bitmap)) {
                    pixels = pixels
                        .checked_add(count)
                        .filter(|&n| n <= MAX_ANIMATION_PIXELS)
                        .ok_or("Image animation exceeds its retained pixel limit.")?;
                }
            }
        }
        Ok(pixels * 4)
    }
}

pub fn decode_asset_checked(
    data: &[u8],
    check: &dyn Fn() -> ImageResult<()>,
) -> ImageResult<DecodedImage> {
    check()?;
    if data.len() > MAX_INPUT_BYTES {
        return Err("Image input exceeds the 32 MiB limit.".into());
    }
    let decoded = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        png::decode_asset(data, check)?
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        gif::decode_asset(data, check)?
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        webp::decode_asset(data, check)?
    } else {
        DecodedImage::still(decode_checked(data, check)?)
    };
    decoded.retained_bytes()?;
    check()?;
    Ok(decoded)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn decode(data: &[u8]) -> Result<Bitmap, String> {
    decode_checked(data, &|| Ok(()))
}

pub fn decode_checked(
    data: &[u8],
    check: &dyn Fn() -> Result<(), String>,
) -> Result<Bitmap, String> {
    check()?;
    if data.len() > MAX_INPUT_BYTES {
        return Err("Image input exceeds the 32 MiB limit.".into());
    }
    let result = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        png::decode(data, check)
    } else if data.starts_with(b"\xff\xd8") {
        crate::jpeg::decode(data, check)
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        gif::decode(data, check)
    } else if data.starts_with(b"BM") {
        dib::bmp(data, check)
    } else if matches!(
        data.get(..4),
        Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
    ) {
        tiff::decode(data, check)
    } else if data.starts_with(&[0, 0, 1, 0])
        || (data.starts_with(&[0, 0, 2, 0])
            // Raw TGA true-color headers share the CUR prefix, but clear
            // these color-map bytes; a real CUR directory has a nonzero count.
            && !(data.get(4..8) == Some(&[0, 0, 0, 0]) && legacy::looks_like_tga(data)))
    {
        dib::icon(data, check)
    } else if data.first() == Some(&b'P') && data.get(1).is_some_and(|b| (b'1'..=b'7').contains(b))
    {
        simple::netpbm(data, check)
    } else if data.starts_with(b"qoif") {
        simple::qoi(data, check)
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        webp::decode(data, check)
    } else if svg::looks_like(data) {
        svg::decode(data, check)
    } else if legacy::looks_like_pcx(data) {
        legacy::pcx(data, check)
    } else if legacy::looks_like_tga(data) {
        legacy::tga(data, check)
    } else {
        Err(
            "Unsupported image format. AVIF, HEIF/HEIC and JPEG XL codecs are not implemented. Supported families include JPEG, PNG/APNG, GIF, BMP, ICO/CUR, static SVG, WebP, TIFF, QOI, Netpbm, TGA and PCX."
                .into(),
        )
    };
    check()?;
    result
}

fn dimensions(width: usize, height: usize) -> ImageResult<usize> {
    dimensions_with_limit(width,height,MAX_PIXELS)
}

fn dimensions_with_limit(width:usize,height:usize,maximum:usize)->ImageResult<usize> {
    let count = width
        .checked_mul(height)
        .ok_or("Image dimensions overflow.")?;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || count > maximum
    {
        return Err(if maximum==MAX_PIXELS {"Image exceeds the 4-megapixel or 16384-pixel dimension limit.".into()}else{format!("Image exceeds the {maximum}-pixel or {MAX_DIMENSION}-pixel dimension allowance.")});
    }
    Ok(count)
}

/// Raw intra VP8 frames share the original codec, with a host-selected pixel
/// allowance. No WebP/animation container is involved in the video path.
pub(crate) fn decode_vp8_frame_checked(data:&[u8],maximum_pixels:usize,check:&dyn Fn()->ImageResult<()>)->ImageResult<Bitmap> {
    webp::decode_vp8_frame(data,maximum_pixels,check)
}

fn reserved<T>(count: usize) -> ImageResult<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| "Not enough memory for the image.")?;
    Ok(output)
}

fn be32(data: &[u8]) -> u32 {
    u32::from_be_bytes([data[0], data[1], data[2], data[3]])
}

fn le32(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

fn le16(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

fn pixel(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    (u32::from(alpha) << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

#[cfg(test)]
fn inflate_zlib(data: &[u8], expected: usize) -> ImageResult<Vec<u8>> {
    png::inflate(data, expected, &|| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(kind: &[u8; 4], body: &[u8], output: &mut Vec<u8>) {
        output.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let start = output.len();
        output.extend_from_slice(kind);
        output.extend_from_slice(body);
        output.extend_from_slice(&crc32(&output[start..]).to_be_bytes());
    }

    fn stored(raw: &[u8]) -> Vec<u8> {
        assert!(raw.len() <= 65535);
        let mut data = vec![0x78, 0x01, 0x01];
        let length = raw.len() as u16;
        data.extend_from_slice(&length.to_le_bytes());
        data.extend_from_slice(&(!length).to_le_bytes());
        data.extend_from_slice(raw);
        data.extend_from_slice(&adler32(raw).to_be_bytes());
        data
    }

    fn png(
        width: u32,
        height: u32,
        color: u8,
        raw: &[u8],
        extras: &[(&[u8; 4], &[u8])],
    ) -> Vec<u8> {
        let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = width.to_be_bytes().to_vec();
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, color, 0, 0, 0]);
        chunk(b"IHDR", &header, &mut data);
        for (kind, body) in extras {
            chunk(kind, body, &mut data);
        }
        chunk(b"IDAT", &stored(raw), &mut data);
        chunk(b"IEND", &[], &mut data);
        data
    }

    fn png_depth(
        width: u32,
        height: u32,
        depth: u8,
        color: u8,
        interlace: u8,
        raw: &[u8],
        extras: &[(&[u8; 4], &[u8])],
    ) -> Vec<u8> {
        let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = width.to_be_bytes().to_vec();
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[depth, color, 0, 0, interlace]);
        chunk(b"IHDR", &header, &mut data);
        for (kind, body) in extras {
            chunk(kind, body, &mut data);
        }
        chunk(b"IDAT", &stored(raw), &mut data);
        chunk(b"IEND", &[], &mut data);
        data
    }

    fn dib_info(
        width: u32,
        height: i32,
        depth: u16,
        compression: u32,
        palette: &[u8],
        raw: &[u8],
    ) -> Vec<u8> {
        let mut data = vec![0u8; 40];
        data[..4].copy_from_slice(&40u32.to_le_bytes());
        data[4..8].copy_from_slice(&width.to_le_bytes());
        data[8..12].copy_from_slice(&height.to_le_bytes());
        data[12..14].copy_from_slice(&1u16.to_le_bytes());
        data[14..16].copy_from_slice(&depth.to_le_bytes());
        data[16..20].copy_from_slice(&compression.to_le_bytes());
        if depth <= 8 {
            data[32..36].copy_from_slice(&(palette.len() as u32 / 4).to_le_bytes());
        }
        data.extend_from_slice(palette);
        data.extend_from_slice(raw);
        data
    }

    fn bmp_file(dib: &[u8], offset: usize) -> Vec<u8> {
        let mut data = vec![0u8; 14];
        data[..2].copy_from_slice(b"BM");
        data[2..6].copy_from_slice(&((dib.len() + 14) as u32).to_le_bytes());
        data[10..14].copy_from_slice(&((offset + 14) as u32).to_le_bytes());
        data.extend_from_slice(dib);
        data
    }

    fn icon_file(kind: u16, width: u8, height: u8, depth: u16, payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0, 0];
        data.extend_from_slice(&kind.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&[width, height, 0, 0]);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&depth.to_le_bytes());
        data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        data.extend_from_slice(&22u32.to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn decodes_all_png_filters_and_straight_alpha() {
        let raw = [
            0, 10, 20, 30, 40, 50, 60, 70, 80, 1, 11, 22, 33, 44, 44, 44, 44, 44, 2, 1, 2, 3, 4, 5,
            6, 7, 8, 3, 14, 18, 22, 26, 20, 19, 18, 17, 4, 5, 5, 5, 5, 5, 5, 5, 5,
        ];
        let decoded = decode(&png(2, 5, 6, &raw, &[])).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 5));
        assert_eq!(
            decoded.pixels,
            vec![
                pixel(10, 20, 30, 40),
                pixel(50, 60, 70, 80),
                pixel(11, 22, 33, 44),
                pixel(55, 66, 77, 88),
                pixel(12, 24, 36, 48),
                pixel(60, 72, 84, 96),
                pixel(20, 30, 40, 50),
                pixel(60, 70, 80, 90),
                pixel(25, 35, 45, 55),
                pixel(65, 75, 85, 95),
            ]
        );
    }

    #[test]
    fn decodes_grayscale_rgb_palette_and_transparency() {
        let gray = decode(&png(2, 1, 0, &[0, 7, 8], &[(b"tRNS", &[0, 7])])).unwrap();
        assert_eq!(gray.pixels, vec![pixel(7, 7, 7, 0), pixel(8, 8, 8, 255)]);
        let rgb = decode(&png(
            1,
            1,
            2,
            &[0, 10, 20, 30],
            &[(b"tRNS", &[0, 10, 0, 20, 0, 30])],
        ))
        .unwrap();
        assert_eq!(rgb.pixels, vec![pixel(10, 20, 30, 0)]);
        let gray_alpha = decode(&png(1, 1, 4, &[0, 7, 128], &[])).unwrap();
        assert_eq!(gray_alpha.pixels, vec![pixel(7, 7, 7, 128)]);
        let indexed = decode(&png(
            2,
            1,
            3,
            &[0, 0, 1],
            &[(b"PLTE", &[255, 0, 0, 0, 255, 0]), (b"tRNS", &[64])],
        ))
        .unwrap();
        assert_eq!(
            indexed.pixels,
            vec![pixel(255, 0, 0, 64), pixel(0, 255, 0, 255)]
        );
        assert!(decode(&png(1, 1, 3, &[0, 2], &[(b"PLTE", &[255, 0, 0])])).is_err());
    }

    #[test]
    fn inflates_stored_fixed_and_dynamic_streams_with_checksums() {
        let mut expected: Vec<u8> = (0..4096).map(|n| (n % 23) as u8).collect();
        expected.extend_from_slice(&b"abcabcabc".repeat(128));
        // Generated from the explicit input above with a separate development
        // tool; decoding these constants needs no external runtime library.
        let fixed = hex("78016360646266616563e7e0e4e2e6e1e5e3171014121611156318151e151e151e151e151e151e151e151e151e151e151e151e151e151e151e3ec28949c9a368148da2513420080022b1690a");
        let dynamic = hex("789cedc8c70980400000b0b37bf672ba9fbaff0e8e214820af842c2fcaaa6edad8f5c338cdcbbaede93883d65a6badb5d65a6badffd3d7fd007ce20522b1690a");
        for compressed in [stored(&expected), fixed, dynamic] {
            assert_eq!(inflate_zlib(&compressed, expected.len()).unwrap(), expected);
            assert!(inflate_zlib(&compressed, 4).is_err());
        }
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        let mut bad = stored(b"valid");
        *bad.last_mut().unwrap() ^= 1;
        assert!(inflate_zlib(&bad, 5).is_err());
    }

    #[test]
    fn rejects_truncation_invalid_trees_and_excessive_dimensions() {
        let valid = png(1, 1, 6, &[0, 1, 2, 3, 255], &[]);
        for end in 0..valid.len() {
            assert!(decode(&valid[..end]).is_err());
        }
        let mut corrupt = valid.clone();
        corrupt[29] ^= 1;
        assert!(decode(&corrupt).is_err());
        assert!(decode(&png(20000, 1, 6, &[], &[])).is_err());
        assert!(decode(&png(4096, 4096, 6, &[], &[])).is_err());
        assert!(decode(&png(1, 1, 6, &[0, 1, 2, 3, 4], &[(b"ABCD", &[])])).is_err());
        let overrun = png(1, 1, 6, &[0, 1, 2, 3, 4, 5, 6, 7], &[]);
        assert!(decode(&overrun).is_err());
    }

    #[test]
    fn decodes_padded_bottom_up_bmp_and_ppm_samples() {
        let mut bmp = vec![0u8; 70];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&70u32.to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&2u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&2u32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
        bmp[54..].copy_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0]);
        assert_eq!(
            decode(&bmp).unwrap().pixels,
            vec![0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffffff]
        );
        let plain = decode(b"P3\n# comment\r1 1\n15\n15 0 7").unwrap();
        assert_eq!(plain.pixels, vec![pixel(255, 0, 119, 255)]);
        let binary = decode(b"P6\n1 1\n255\r\x0a\x14\x1e").unwrap();
        assert_eq!(binary.pixels, vec![pixel(10, 20, 30, 255)]);
    }

    #[test]
    fn malformed_compressed_data_never_escapes_output_bounds() {
        let mut state = 93u32;
        for length in 0..256 {
            let mut compressed = vec![0x78, 0x01];
            for _ in 0..length {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                compressed.push(state as u8);
            }
            compressed.extend_from_slice(&[0, 0, 0, 1]);
            if let Ok(output) = inflate_zlib(&compressed, 16) {
                assert_eq!(output.len(), 16);
            }
        }
    }
    #[test]
    fn png_packed_samples_and_full_precision_transparency() {
        let packed = decode(&png_depth(
            5,
            1,
            2,
            0,
            0,
            &[0, 0b00011011, 0],
            &[(b"tRNS", &[0, 2])],
        ))
        .unwrap();
        assert_eq!(
            packed.pixels,
            vec![
                pixel(0, 0, 0, 255),
                pixel(85, 85, 85, 255),
                pixel(170, 170, 170, 0),
                pixel(255, 255, 255, 255),
                pixel(0, 0, 0, 255)
            ]
        );
        let indexed = decode(&png_depth(
            3,
            1,
            1,
            3,
            0,
            &[0, 0b10100000],
            &[(b"PLTE", &[1, 2, 3, 4, 5, 6]), (b"tRNS", &[0, 128])],
        ))
        .unwrap();
        assert_eq!(
            indexed.pixels,
            vec![pixel(4, 5, 6, 128), pixel(1, 2, 3, 0), pixel(4, 5, 6, 128)]
        );
        let gray = decode(&png_depth(
            2,
            1,
            16,
            0,
            0,
            &[0, 0x12, 0x34, 0x12, 0x35],
            &[(b"tRNS", &[0x12, 0x34])],
        ))
        .unwrap();
        assert_eq!(
            gray.pixels,
            vec![pixel(18, 18, 18, 0), pixel(18, 18, 18, 255)]
        );
        assert!(decode(&png_depth(1, 1, 8, 0, 0, &[0, 7], &[(b"tRNS", &[1, 7])])).is_err());
        assert!(decode(&png_depth(1, 1, 1, 3, 0, &[0, 0], &[(b"PLTE", &[0; 9])])).is_err());
    }

    #[test]
    fn png_adam7_handles_empty_passes_and_resets_filter_history() {
        // Three-by-two grayscale image: Adam7's nonempty passes are 1,4,6,7.
        // Use Up on every row, so no pass may read the previous pass as above.
        let bitmap = decode(&png_depth(
            3,
            2,
            8,
            0,
            1,
            &[2, 10, 2, 30, 2, 20, 2, 40, 50, 60],
            &[],
        ))
        .unwrap();
        assert_eq!(
            bitmap.pixels,
            [10, 20, 30, 40, 50, 60].map(|v| pixel(v, v, v, 255))
        );
        let one = decode(&png_depth(
            1,
            1,
            16,
            6,
            1,
            &[
                0,
                0xffffu16.to_be_bytes()[0],
                255,
                0,
                0,
                0x80,
                0x80,
                0x40,
                0x40,
            ],
            &[],
        ))
        .unwrap();
        assert_eq!(one.pixels, vec![pixel(255, 0, 128, 64)]);
        assert!(decode(&png_depth(3, 2, 8, 0, 1, &[0, 1, 2, 3, 4, 5, 6], &[])).is_err());
    }

    #[test]
    fn png_and_bmp_call_the_cancel_guard_during_decoding() {
        use std::cell::Cell;
        let source = png(4, 16, 6, &[0; 16 * 17], &[]);
        let calls = Cell::new(0);
        let error = decode_checked(&source, &|| {
            calls.set(calls.get() + 1);
            if calls.get() > 10 {
                Err("canceled".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "canceled");
        let source = bmp_file(&dib_info(4, 16, 24, 0, &[], &[0; 4 * 16 * 3]), 40);
        calls.set(0);
        assert_eq!(
            decode_checked(&source, &|| {
                calls.set(calls.get() + 1);
                if calls.get() > 5 {
                    Err("canceled".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err(),
            "canceled"
        );
    }

    #[test]
    fn bmp_palette_top_down_rgb555_and_bitfields() {
        let palette = [0, 0, 0, 0, 0, 0, 255, 0, 0, 255, 0, 0];
        let bmp = bmp_file(
            &dib_info(3, -1, 4, 0, &palette, &[0x12, 0x00, 0, 0]),
            40 + palette.len(),
        );
        assert_eq!(
            decode(&bmp).unwrap().pixels,
            vec![0xffff0000, 0xff00ff00, 0xff000000]
        );
        let rgb555 = bmp_file(&dib_info(2, 1, 16, 0, &[], &[0x00, 0x7c, 0xe0, 0x03]), 40);
        assert_eq!(
            decode(&rgb555).unwrap().pixels,
            vec![0xffff0000, 0xff00ff00]
        );
        let mut masks = Vec::new();
        for mask in [0xf800u32, 0x07e0, 0x001f] {
            masks.extend_from_slice(&mask.to_le_bytes());
        }
        let fields = bmp_file(&dib_info(2, 1, 16, 3, &masks, &[0, 0xf8, 0xe0, 7]), 52);
        assert_eq!(
            decode(&fields).unwrap().pixels,
            vec![0xffff0000, 0xff00ff00]
        );
        masks[4..8].copy_from_slice(&0xf800u32.to_le_bytes());
        assert!(decode(&bmp_file(&dib_info(2, 1, 16, 3, &masks, &[0; 4]), 52)).is_err());
    }

    #[test]
    fn bmp_rle_runs_literals_deltas_padding_and_end_markers() {
        let palette = [0, 0, 0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0];
        let raw = [0, 4, 1, 2, 3, 0, 0, 0, 0, 2, 1, 0, 3, 2, 0, 1];
        let bmp = bmp_file(&dib_info(4, 2, 8, 1, &palette, &raw), 40 + palette.len());
        assert_eq!(
            decode(&bmp).unwrap().pixels,
            vec![
                0xff000000, 0xff00ff00, 0xff00ff00, 0xff00ff00, 0xffff0000, 0xff00ff00, 0xff0000ff,
                0xff000000
            ]
        );
        let four = bmp_file(
            &dib_info(5, 1, 4, 2, &palette, &[0, 5, 0x12, 0x30, 0x10, 0, 0, 1]),
            40 + palette.len(),
        );
        assert_eq!(
            decode(&four).unwrap().pixels,
            vec![0xffff0000, 0xff00ff00, 0xff0000ff, 0xff000000, 0xffff0000]
        );
        for bad in [
            &[6, 1, 0, 1][..],
            &[0, 2, 0, 2, 0, 1],
            &[4, 1],
            &[0, 4, 1, 2],
        ] {
            assert!(decode(&bmp_file(
                &dib_info(4, 1, 8, 1, &palette, bad),
                40 + palette.len()
            ))
            .is_err());
        }
    }

    #[test]
    fn icon_png_dib_alpha_legacy_masks_and_cursor_hotspots() {
        let png = png(2, 1, 6, &[0, 255, 0, 0, 64, 0, 255, 0, 128], &[]);
        assert_eq!(
            decode(&icon_file(1, 2, 1, 32, &png)).unwrap().pixels,
            vec![pixel(255, 0, 0, 64), pixel(0, 255, 0, 128)]
        );
        let alpha = dib_info(
            2,
            2,
            32,
            0,
            &[],
            &[0, 0, 255, 64, 0, 255, 0, 128, 0xff, 0, 0, 0],
        );
        assert_eq!(
            decode(&icon_file(1, 2, 1, 32, &alpha)).unwrap().pixels,
            vec![pixel(255, 0, 0, 64), pixel(0, 255, 0, 128)]
        );
        let legacy = dib_info(
            2,
            2,
            32,
            0,
            &[],
            &[0, 0, 255, 0, 0, 255, 0, 0, 0x40, 0, 0, 0],
        );
        assert_eq!(
            decode(&icon_file(1, 2, 1, 32, &legacy)).unwrap().pixels,
            vec![pixel(255, 0, 0, 255), pixel(0, 255, 0, 0)]
        );
        let cursor = icon_file(2, 2, 1, 0, &legacy);
        assert_eq!(
            decode(&cursor).unwrap().pixels,
            decode(&icon_file(1, 2, 1, 32, &legacy)).unwrap().pixels
        );
        assert!(decode(&icon_file(2, 2, 1, 1, &legacy)).is_err());
        assert!(decode(&icon_file(1, 1, 1, 32, &png)).is_err());
        for length in 0..alpha.len() {
            assert!(decode(&icon_file(1, 2, 1, 32, &alpha[..length])).is_err());
        }
    }
}
