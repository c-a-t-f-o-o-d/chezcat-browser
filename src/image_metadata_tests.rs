use super::*;

const SOURCE: [u32; 6] = [
    0xffff0000, 0x8000ff00, 0x000000ff, 0xffab2510, 0x40f0a0b0, 0xffffffff,
];
const ORDER: [[usize; 6]; 8] = [
    [0, 1, 2, 3, 4, 5],
    [2, 1, 0, 5, 4, 3],
    [5, 4, 3, 2, 1, 0],
    [3, 4, 5, 0, 1, 2],
    [0, 3, 1, 4, 2, 5],
    [3, 0, 4, 1, 5, 2],
    [5, 2, 4, 1, 3, 0],
    [2, 5, 1, 4, 0, 3],
];

fn tiff(value: u16, little: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let word = |n: u16| {
        if little {
            n.to_le_bytes()
        } else {
            n.to_be_bytes()
        }
    };
    let long = |n: u32| {
        if little {
            n.to_le_bytes()
        } else {
            n.to_be_bytes()
        }
    };
    out.extend_from_slice(if little { b"II" } else { b"MM" });
    out.extend(word(42));
    out.extend(long(8));
    out.extend(word(1));
    out.extend(word(0x112));
    out.extend(word(3));
    out.extend(long(1));
    out.extend(word(value));
    out.extend(word(0));
    out.extend(long(0));
    out
}

fn bitmap() -> Bitmap {
    Bitmap {
        width: 3,
        height: 2,
        pixels: SOURCE.to_vec(),
    }
}

fn assert_pixels(bitmap: &Bitmap, source: &[u32], value: u8) {
    let expected_size = if value >= 5 { (2, 3) } else { (3, 2) };
    assert_eq!((bitmap.width, bitmap.height), expected_size);
    assert_eq!(
        bitmap.pixels,
        ORDER[value as usize - 1]
            .iter()
            .map(|&index| source[index])
            .collect::<Vec<_>>()
    );
}

#[test]
fn all_eight_orientations_match_explicit_asymmetric_alpha_geometry() {
    for little in [false, true] {
        for value in 1..=8 {
            let data = tiff(value, little);
            assert_eq!(tiff_orientation(&data), Some(value as u8));
            assert_eq!(webp_orientation(&data), Some(value as u8));
            let mut jpeg = b"Exif\0\0".to_vec();
            jpeg.extend(data);
            assert_eq!(jpeg_orientation(&jpeg), Some(value as u8));
            assert_eq!(webp_orientation(&jpeg), Some(value as u8));
            let output = orient_bitmap(bitmap(), value as u8, &|| Ok(())).unwrap();
            assert_pixels(&output, &SOURCE, value as u8);
        }
    }
}

#[test]
fn tiff_optional_metadata_has_bounded_offsets_counts_types_and_values() {
    for little in [false, true] {
        let valid = tiff(6, little);
        for length in 0..valid.len() {
            assert_eq!(tiff_orientation(&valid[..length]), None);
        }
        for value in [0, 9, 255, 256, u16::MAX] {
            assert_eq!(tiff_orientation(&tiff(value, little)), None);
        }
        let mut bad = valid.clone();
        bad[4..8].copy_from_slice(&[255; 4]);
        assert_eq!(tiff_orientation(&bad), None);
        let mut bad = valid.clone();
        bad[8..10].copy_from_slice(&[255; 2]);
        assert_eq!(tiff_orientation(&bad), None);
        let mut bad = valid.clone();
        bad[12..14].copy_from_slice(&[0; 2]);
        assert_eq!(tiff_orientation(&bad), None);
        let mut bad = valid.clone();
        bad[14..18].copy_from_slice(&[255; 4]);
        assert_eq!(tiff_orientation(&bad), None);
        let mut bad = valid.clone();
        bad[2..4].copy_from_slice(&[0; 2]);
        assert_eq!(tiff_orientation(&bad), None);
        // Next-IFD links (even cycles) are not followed by this bounded subset.
        let mut cycle = valid;
        cycle[22..26].copy_from_slice(if little { &[8, 0, 0, 0] } else { &[0, 0, 0, 8] });
        assert_eq!(tiff_orientation(&cycle), Some(6));
    }
    assert_eq!(tiff_orientation(b"Exif\0\0II*\0"), None);
    assert_eq!(jpeg_orientation(&tiff(1, true)), None);
}

#[test]
fn whole_animation_orientation_preserves_sharing_timing_and_retained_bytes() {
    for orientation in 1..=8 {
        let poster = Arc::new(bitmap());
        let other = Arc::new(Bitmap {
            width: 3,
            height: 2,
            pixels: vec![0xff112233; 6],
        });
        let input = DecodedImage {
            bitmap: poster.clone(),
            animation: Some(Arc::new(Animation {
                frames: vec![
                    ImageFrame {
                        bitmap: poster.clone(),
                        delay_ms: 40,
                    },
                    ImageFrame {
                        bitmap: other,
                        delay_ms: 75,
                    },
                    ImageFrame {
                        bitmap: poster,
                        delay_ms: 90,
                    },
                ],
                plays: Some(2),
            })),
            notice: Some("preserve the decoder notice".into()),
        };
        let original_size = input.retained_bytes().unwrap();
        let output = orient_asset(input, orientation, &|| Ok(())).unwrap();
        assert_pixels(&output.bitmap, &SOURCE, orientation);
        assert_eq!(output.retained_bytes().unwrap(), original_size);
        let animation = output.animation.unwrap();
        assert_eq!(animation.plays, Some(2));
        assert_eq!(
            animation
                .frames
                .iter()
                .map(|f| f.delay_ms)
                .collect::<Vec<_>>(),
            [40, 75, 90]
        );
        assert!(Arc::ptr_eq(&output.bitmap, &animation.frames[0].bitmap));
        assert!(Arc::ptr_eq(&output.bitmap, &animation.frames[2].bitmap));
        assert!(!Arc::ptr_eq(&output.bitmap, &animation.frames[1].bitmap));
        assert_eq!(
            output.notice.as_deref(),
            Some("preserve the decoder notice")
        );
    }
}

#[test]
fn orientation_checks_cancellation_and_rejects_inconsistent_rasters() {
    let checks = std::cell::Cell::new(0);
    let result = orient_bitmap(bitmap(), 6, &|| {
        checks.set(checks.get() + 1);
        if checks.get() > 2 {
            Err("orientation cancelled".into())
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err(), "orientation cancelled");
    assert!(checks.get() <= 3);
    assert!(orient_bitmap(
        Bitmap {
            width: 2,
            height: 2,
            pixels: vec![0]
        },
        6,
        &|| Ok(())
    )
    .is_err());
}

fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], contents: &[u8]) {
    output.extend_from_slice(&(contents.len() as u32).to_be_bytes());
    let start = output.len();
    output.extend_from_slice(kind);
    output.extend_from_slice(contents);
    output.extend_from_slice(&crate::compression::crc32(&output[start..]).to_be_bytes());
}

fn encoded(width: usize, pixels: &[u32]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in pixels.chunks(width) {
        raw.push(0);
        for pixel in row {
            raw.extend_from_slice(&[
                (pixel >> 16) as u8,
                (pixel >> 8) as u8,
                *pixel as u8,
                (pixel >> 24) as u8,
            ]);
        }
    }
    let mut zlib = vec![0x78, 0x01, 0x01];
    zlib.extend_from_slice(&(raw.len() as u16).to_le_bytes());
    zlib.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&crate::compression::adler32(&raw).to_be_bytes());
    zlib
}

fn png(metadata: Option<&[u8]>, animated: bool) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend(3u32.to_be_bytes());
    header.extend(2u32.to_be_bytes());
    header.extend([8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    if let Some(metadata) = metadata {
        chunk(&mut out, b"eXIf", metadata);
    }
    if animated {
        let mut control = Vec::new();
        control.extend(4u32.to_be_bytes());
        control.extend(2u32.to_be_bytes());
        chunk(&mut out, b"acTL", &control);
        frame_control(&mut out, 0, (3, 2, 0, 0), (0, 0));
    }
    chunk(&mut out, b"IDAT", &encoded(3, &SOURCE));
    if animated {
        for (seq, left, top, value, disposal, blend) in [
            (1, 1, 0, 0x80ff0000, 2, 1),
            (3, 2, 1, 0xff00ffff, 1, 0),
            (5, 0, 1, 0x00000000, 0, 1),
        ] {
            frame_control(&mut out, seq, (1, 1, left, top), (disposal, blend));
            let mut data = (seq + 1).to_be_bytes().to_vec();
            data.extend(encoded(1, &[value]));
            chunk(&mut out, b"fdAT", &data);
        }
    }
    chunk(&mut out, b"IEND", &[]);
    out
}

fn frame_control(out: &mut Vec<u8>, seq: u32, rect: (u32, u32, u32, u32), operation: (u8, u8)) {
    let mut control = Vec::new();
    for value in [seq, rect.0, rect.1, rect.2, rect.3] {
        control.extend(value.to_be_bytes());
    }
    control.extend((seq as u16 + 1).to_be_bytes());
    control.extend(10u16.to_be_bytes());
    control.extend([operation.0, operation.1]);
    chunk(out, b"fcTL", &control);
}

#[test]
fn png_and_apng_metadata_orients_after_subframe_disposal_and_blending() {
    let plain = crate::image::decode_asset_checked(&png(None, true), &|| Ok(())).unwrap();
    for little in [false, true] {
        for orientation in 1..=8 {
            let metadata = tiff(orientation, little);
            let still = crate::image::decode(&png(Some(&metadata), false)).unwrap();
            assert_pixels(&still, &SOURCE, orientation as u8);
            let animated =
                crate::image::decode_asset_checked(&png(Some(&metadata), true), &|| Ok(()))
                    .unwrap();
            assert_pixels(&animated.bitmap, &SOURCE, orientation as u8);
            let frames = &animated.animation.as_ref().unwrap().frames;
            let before = &plain.animation.as_ref().unwrap().frames;
            for (before, after) in before.iter().zip(frames) {
                assert_eq!(before.delay_ms, after.delay_ms);
                assert_pixels(&after.bitmap, &before.bitmap.pixels, orientation as u8);
            }
            assert!(Arc::ptr_eq(&animated.bitmap, &frames[0].bitmap));
        }
    }
}

#[test]
fn malformed_png_metadata_is_ignored_and_png_inside_ico_uses_orientation_once() {
    for data in [
        &[][..],
        &b"inert"[..],
        &tiff(9, true)[..],
        &tiff(6, false)[..8],
    ] {
        let decoded = crate::image::decode(&png(Some(data), false)).unwrap();
        assert_pixels(&decoded, &SOURCE, 1);
    }
    let png = png(Some(&tiff(6, true)), false);
    let mut icon = vec![0, 0, 1, 0, 1, 0, 3, 2, 0, 0, 1, 0, 32, 0];
    icon.extend((png.len() as u32).to_le_bytes());
    icon.extend(22u32.to_le_bytes());
    icon.extend(png);
    let decoded = crate::image::decode(&icon).unwrap();
    assert_pixels(&decoded, &SOURCE, 6);
}
