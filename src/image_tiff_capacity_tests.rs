use super::*;
use std::cell::Cell;

const HEIGHT: usize = 16_384;
const PLANES: usize = 5;
const OLD_SEGMENT_LIMIT: usize = 65_536;

// Original TIFF 6.0 containers: CMYK plus unassociated alpha, separate planes.
// Tables use the specified SamplesPerPixel * StripsPerImage/TilesPerImage count.
struct Fixture {
    bytes: Vec<u8>,
    little: bool,
    offsets_at: usize,
    lengths_at: usize,
    segments: usize,
}

impl Fixture {
    fn new(little: bool, tile: Option<(usize, usize)>, packed: Option<&[u8]>) -> Self {
        let short = |value: u16| {
            if little { value.to_le_bytes() } else { value.to_be_bytes() }
        };
        let long = |value: u32| {
            if little { value.to_le_bytes() } else { value.to_be_bytes() }
        };
        let segments = PLANES * HEIGHT.div_ceil(tile.map_or(1, |(_, height)| height));
        let field_count = if tile.is_some() { 12 } else { 11 };
        let bits_at = 8 + 2 + field_count * 12 + 4;
        let offsets_at = bits_at + PLANES * 2;
        let lengths_at = offsets_at + segments * 4;
        let payload_at = lengths_at + segments * 4;
        let mut fields = vec![
            (256, 4, 1, 1),
            (257, 4, 1, HEIGHT as u32),
            (258, 3, PLANES as u32, bits_at as u32),
            (259, 3, 1, if packed.is_some() { 32773 } else { 1 }),
            (262, 3, 1, 5),
            (277, 3, 1, PLANES as u32),
            (284, 3, 1, 2),
            (338, 3, 1, 2),
        ];
        if let Some((width, height)) = tile {
            fields.extend([
                (322, 4, 1, width as u32),
                (323, 4, 1, height as u32),
                (324, 4, segments as u32, offsets_at as u32),
                (325, 4, segments as u32, lengths_at as u32),
            ]);
        } else {
            fields.extend([
                (273, 4, segments as u32, offsets_at as u32),
                (278, 4, 1, 1),
                (279, 4, segments as u32, lengths_at as u32),
            ]);
        }
        fields.sort_unstable_by_key(|field| field.0);
        assert_eq!(fields.len(), field_count);
        let mut bytes = if little { b"II".to_vec() } else { b"MM".to_vec() };
        bytes.extend(short(42));
        bytes.extend(long(8));
        bytes.extend(short(field_count as u16));
        for (tag, kind, count, value) in fields {
            bytes.extend(short(tag));
            bytes.extend(short(kind));
            bytes.extend(long(count));
            if kind == 3 && count == 1 {
                bytes.extend(short(value as u16));
                bytes.extend([0; 2]);
            } else {
                bytes.extend(long(value));
            }
        }
        bytes.extend(long(0)); // No subsequent image directory.
        assert_eq!(bytes.len(), bits_at);
        for _ in 0..PLANES {
            bytes.extend(short(8));
        }
        assert_eq!(bytes.len(), offsets_at);
        for index in 0..segments {
            bytes.extend(long((payload_at + if packed.is_some() { 0 } else { index }) as u32));
        }
        assert_eq!(bytes.len(), lengths_at);
        for _ in 0..segments {
            bytes.extend(long(packed.map_or(1, <[u8]>::len) as u32));
        }
        assert_eq!(bytes.len(), payload_at);
        if let Some(payload) = packed {
            bytes.extend(payload);
        } else {
            assert!(tile.is_none());
            for ink in [0, 64, 128, 0, 255] {
                bytes.extend(std::iter::repeat_n(ink, HEIGHT));
            }
            // The entire alpha plane begins beyond the old segment ceiling.
            bytes[payload_at + 4 * HEIGHT] = 73;
            bytes[payload_at + segments - 1] = 201;
            for (plane, ink) in [17, 51, 85].into_iter().enumerate() {
                bytes[payload_at + (plane + 1) * HEIGHT - 1] = ink;
            }
        }
        Self { bytes, little, offsets_at, lengths_at, segments }
    }

    fn put_long(&mut self, at: usize, value: u32) {
        let value = if self.little { value.to_le_bytes() } else { value.to_be_bytes() };
        self.bytes[at..at + 4].copy_from_slice(&value);
    }

    fn field_at(&self, tag: u16) -> usize {
        for index in 0..11 {
            let at = 10 + index * 12;
            let bytes = self.bytes[at..at + 2].try_into().unwrap();
            let actual = if self.little { u16::from_le_bytes(bytes) } else { u16::from_be_bytes(bytes) };
            if actual == tag { return at; }
        }
        panic!("missing fixture tag {tag}");
    }
}

#[test]
fn five_planar_channels_decode_all_81920_strips_in_both_byte_orders() {
    for little in [false, true] {
        let fixture = Fixture::new(little, None, None);
        assert_eq!(fixture.segments, 81_920);
        let image = decode(&fixture.bytes, &|| Ok(())).unwrap();
        assert_eq!((image.width, image.height), (1, HEIGHT));
        assert_eq!(image.pixels.len(), HEIGHT);
        assert_eq!(image.pixels[0], 0x49ffbf7f);
        assert!(image.pixels[1..HEIGHT - 1].iter().all(|&pixel| pixel == 0xffffbf7f));
        assert_eq!(image.pixels[HEIGHT - 1], 0xc9eeccaa);
    }
}

#[test]
fn final_strips_still_require_in_input_extents_and_exact_uncompressed_lengths() {
    for little in [false, true] {
        let mut outside = Fixture::new(little, None, None);
        outside.put_long(outside.offsets_at + (outside.segments - 1) * 4, outside.bytes.len() as u32);
        assert_eq!(decode(&outside.bytes, &|| Ok(())).unwrap_err(),
            "TIFF field or segment is outside the input.");

        let mut truncated = Fixture::new(little, None, None);
        truncated.bytes.pop();
        assert_eq!(decode(&truncated.bytes, &|| Ok(())).unwrap_err(),
            "TIFF field or segment is outside the input.");

        for length in [0, 2] {
            let mut wrong_length = Fixture::new(little, None, None);
            wrong_length.bytes.push(0); // A long final extent is still in the input.
            wrong_length.put_long(wrong_length.lengths_at + (wrong_length.segments - 1) * 4, length);
            assert_eq!(decode(&wrong_length.bytes, &|| Ok(())).unwrap_err(),
                "TIFF uncompressed segment has the wrong byte count.");
        }
    }
}

#[test]
fn segment_table_counts_and_value_admission_remain_enforced() {
    for tag in [273, 279] {
        let mut short = Fixture::new(true, None, None);
        short.put_long(short.field_at(tag) + 4, (short.segments - 1) as u32);
        assert_eq!(decode(&short.bytes, &|| Ok(())).unwrap_err(),
            "TIFF segment tables have the wrong count.");

        let mut excessive = Fixture::new(true, None, None);
        let at = if tag == 273 { excessive.offsets_at } else { excessive.lengths_at };
        // Supply the entire claimed source extent, so this reaches value
        // admission rather than failing an unrelated truncated-input check.
        excessive.bytes.resize(at + (MAX_FIELD_VALUES + 1) * 4, 0);
        excessive.put_long(excessive.field_at(tag) + 4, (MAX_FIELD_VALUES + 1) as u32);
        assert_eq!(decode(&excessive.bytes, &|| Ok(())).unwrap_err(),
            "TIFF field array exceeds its value limit.");
    }
}

#[test]
fn cancellation_remains_effective_after_the_old_segment_ceiling() {
    let mut boundary = Fixture::new(true, None, None);
    boundary.put_long(boundary.offsets_at + OLD_SEGMENT_LIMIT * 4, boundary.bytes.len() as u32);
    let calls = Cell::new(0);
    assert_eq!(decode(&boundary.bytes, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    }).unwrap_err(), "TIFF field or segment is outside the input.");
    // The invalid extent is read only after the first 65,536 strips finish.
    // Calibrate at that point instead of depending on an exact number of
    // cancellation checks inside the individual decompressor and row loops.
    let stop_at = calls.get();
    assert!(stop_at > OLD_SEGMENT_LIMIT);
    calls.set(0);
    let valid = Fixture::new(true, None, None);
    assert_eq!(decode(&valid.bytes, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == stop_at { Err("cancelled after 65536 strips".into()) } else { Ok(()) }
    }).unwrap_err(), "cancelled after 65536 strips");
    assert_eq!(calls.get(), stop_at);
}

#[test]
fn input_segment_and_padded_raster_byte_allowances_remain_enforced() {
    let mut oversized = Fixture::new(true, None, None);
    oversized.bytes.resize(MAX_INPUT_BYTES + 1, 0);
    assert_eq!(decode(&oversized.bytes, &|| Ok(())).unwrap_err(), "TIFF input exceeds 32 MiB.");

    let segment = Fixture::new(true, Some((16_384, 4096)), Some(&[0, 0]));
    assert_eq!(decode(&segment.bytes, &|| Ok(())).unwrap_err(),
        "TIFF segment exceeds the 32 MiB decoded limit.");

    // Reuse a valid compressed tile. The source is small, while padded tile
    // output eventually exceeds the independently retained aggregate allowance.
    let tile: Vec<u8> = std::iter::repeat_n([129, 0], 16_384 * 16 / 128).flatten().collect();
    let raster = Fixture::new(true, Some((16_384, 16)), Some(&tile));
    assert_eq!(decode(&raster.bytes, &|| Ok(())).unwrap_err(),
        "TIFF padded/planar raster exceeds 64 MiB.");
}

#[test]
fn repeated_strip_work_is_charged_after_the_old_segment_ceiling() {
    // A literal followed by legal PackBits no-ops costs 4096 work units per
    // one-byte strip, including decode admission and its row. Reusing its
    // source extent is cheap in bytes but exhausts MAX_WORK after 65,536 strips.
    let mut packed = vec![0, 0];
    packed.extend(std::iter::repeat_n(128, 4092));
    let fixture = Fixture::new(true, None, Some(&packed));
    assert_eq!(decode(&fixture.bytes, &|| Ok(())).unwrap_err(), "TIFF decoding work limit reached.");
}

// Original classic TIFF directories. The nine ordinary fields describe two
// gray pixels; the private fields carry inline metadata that a reader skips.
fn directory_bytes(little: bool, fields: &[(u16, u16, u32, u32)], payload: &[u8]) -> Vec<u8> {
    let short = |value: u16| if little { value.to_le_bytes() } else { value.to_be_bytes() };
    let long = |value: u32| if little { value.to_le_bytes() } else { value.to_be_bytes() };
    let mut bytes = if little { b"II".to_vec() } else { b"MM".to_vec() };
    bytes.extend(short(42));
    bytes.extend(long(8));
    bytes.extend(short(u16::try_from(fields.len()).unwrap()));
    for &(tag, kind, count, value) in fields {
        bytes.extend(short(tag));
        bytes.extend(short(kind));
        bytes.extend(long(count));
        bytes.extend(long(value));
    }
    bytes.extend(long(0));
    bytes.extend(payload);
    bytes
}

fn gray_directory(little: bool, private_fields: usize, reversed: bool) -> Vec<u8> {
    assert!(private_fields <= 32_768);
    let payload_at = 8 + 2 + (9 + private_fields) * 12 + 4;
    let mut fields = vec![
        (256, 4, 1, 2),
        (257, 4, 1, 1),
        (258, 4, 1, 8),
        (259, 4, 1, 1),
        (262, 4, 1, 1),
        (273, 4, 1, payload_at as u32),
        (277, 4, 1, 1),
        (278, 4, 1, 1),
        (279, 4, 1, 2),
    ];
    fields.extend((0..private_fields).map(|index| (32_768 + index as u16, 4, 1, index as u32)));
    if reversed {
        fields.reverse();
    }
    directory_bytes(little, &fields, &[23, 245])
}

fn put_directory_short(bytes: &mut [u8], little: bool, at: usize, value: u16) {
    let encoded = if little { value.to_le_bytes() } else { value.to_be_bytes() };
    bytes[at..at + 2].copy_from_slice(&encoded);
}

fn put_directory_long(bytes: &mut [u8], little: bool, at: usize, value: u32) {
    let encoded = if little { value.to_le_bytes() } else { value.to_be_bytes() };
    bytes[at..at + 4].copy_from_slice(&encoded);
}

#[test]
fn gray_images_with_32777_fields_decode_through_the_public_entry() {
    for little in [false, true] {
        for reversed in [false, true] {
            let bytes = gray_directory(little, 32_768, reversed);
            let bitmap = crate::image::decode(&bytes).unwrap();
            assert_eq!((bitmap.width, bitmap.height), (2, 1));
            assert_eq!(bitmap.pixels, [0xff171717, 0xfff5f5f5]);
        }
    }
}

#[test]
fn directory_count_uses_its_full_u16_representation() {
    for little in [false, true] {
        // This proves directory representation and lookup, not a meaningful
        // image containing every defined TIFF tag. Complete pixels are above.
        let fields: Vec<_> = (1..=u16::MAX)
            .rev()
            .map(|tag| (tag, 4, 1, u32::from(tag) + 5))
            .collect();
        let bytes = directory_bytes(little, &fields, &[]);
        let directory = Directory::parse(&bytes, &|| Ok(())).unwrap();
        assert_eq!(directory.fields.len(), 65_535);
        assert_eq!(directory.scalar(1, None).unwrap(), 6);
        assert_eq!(directory.scalar(32_768, None).unwrap(), 32_773);
        assert_eq!(directory.scalar(u16::MAX, None).unwrap(), 65_540);
        assert_eq!(directory.scalar(0, Some(17)).unwrap(), 17);
    }
}

#[test]
fn large_directories_still_reject_late_duplicates_extents_and_truncation() {
    for little in [false, true] {
        let original = gray_directory(little, 5000, false);
        let last = 10 + (5009 - 1) * 12;

        let mut duplicate = original.clone();
        put_directory_short(&mut duplicate, little, last, 256);
        assert_eq!(crate::image::decode(&duplicate).unwrap_err(), "TIFF directory repeats a field tag.");

        let mut outside = original.clone();
        put_directory_long(&mut outside, little, last + 4, 2);
        put_directory_long(&mut outside, little, last + 8, u32::MAX);
        assert_eq!(crate::image::decode(&outside).unwrap_err(), "TIFF field or segment is outside the input.");

        let mut truncated = original.clone();
        truncated.truncate(last + 12 + 3); // A byte is missing from next-IFD offset.
        assert_eq!(crate::image::decode(&truncated).unwrap_err(), "TIFF field or segment is outside the input.");

        let empty = directory_bytes(little, &[], &[]);
        assert_eq!(crate::image::decode(&empty).unwrap_err(), "TIFF directory is empty.");
    }
}

#[test]
fn large_directories_skip_unknown_types_but_require_supported_consumed_values() {
    for little in [false, true] {
        let mut bytes = gray_directory(little, 5000, false);
        let last = 10 + (5009 - 1) * 12;
        // Its unfamiliar type supplies no interpretable size. Preserve the
        // extension behavior even when its count/offset cannot name a value.
        put_directory_short(&mut bytes, little, last + 2, u16::MAX);
        put_directory_long(&mut bytes, little, last + 4, u32::MAX);
        put_directory_long(&mut bytes, little, last + 8, u32::MAX);
        assert_eq!(crate::image::decode(&bytes).unwrap().pixels, [0xff171717, 0xfff5f5f5]);

        put_directory_short(&mut bytes, little, 10 + 2, u16::MAX); // ImageWidth.
        assert_eq!(crate::image::decode(&bytes).unwrap_err(), "TIFF field 256 has an unsupported value type.");
    }
}

#[test]
fn cancellation_remains_effective_after_4096_directory_fields() {
    let mut boundary = gray_directory(true, 5000, false);
    let late = 10 + 4224 * 12;
    put_directory_long(&mut boundary, true, late + 4, 2);
    put_directory_long(&mut boundary, true, late + 8, u32::MAX);
    let calls = Cell::new(0);
    assert_eq!(Directory::parse(&boundary, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    }).err().unwrap(), "TIFF field or segment is outside the input.");
    let stop_at = calls.get();
    assert!(stop_at > 4096 / 128);
    calls.set(0);
    let valid = gray_directory(true, 5000, false);
    assert_eq!(Directory::parse(&valid, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == stop_at { Err("cancelled after 4096 directory fields".into()) } else { Ok(()) }
    }).err().unwrap(), "cancelled after 4096 directory fields");
    assert_eq!(calls.get(), stop_at);
}
