//! Original PAM capacity fixtures, through the production image entry point.
use super::*;
use std::cell::Cell;

fn long_comment() -> Vec<u8> {
    let mut data = b"P7\n#".to_vec();
    data.resize(data.len() + 131_072, b'x');
    data.push(b'\n');
    data
}

fn decode(data: &[u8]) -> std::sync::Arc<Bitmap> {
    let result = crate::image::decode_asset_checked(data, &|| Ok(())).unwrap();
    assert!(result.animation.is_none());
    result.bitmap
}

fn refusal(data: &[u8]) -> String {
    crate::image::decode_asset_checked(data, &|| Ok(())).unwrap_err()
}

#[test]
fn pam_long_comment_preserves_exact_raster_boundary_and_pixels() {
    let mut data = long_comment();
    data.extend_from_slice(
        b"WIDTH 2\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
    );
    // LF, space and '#' are samples after ENDHDR, not more header text.
    data.extend_from_slice(&[10, 32, 35, 128, 200, 150, 100, 255]);
    let bitmap = decode(&data);
    assert_eq!((bitmap.width, bitmap.height), (2, 1));
    assert_eq!(bitmap.pixels, [0x800a2023, 0xffc89664]);

    // As with the other Netpbm variants, only the first image is displayed.
    data.extend_from_slice(b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 1\nENDHDR\n\0");
    assert_eq!(decode(&data).pixels, bitmap.pixels);
}

#[test]
fn pam_many_header_lines_preserve_high_depth_alpha() {
    let mut data = b"P7\r\n".to_vec();
    for _ in 0..20_000 {
        data.extend_from_slice(b"# comment\r\n \t\x0b\x0c\r\n");
    }
    data.extend_from_slice(
        b"WIDTH 2\r\nHEIGHT 1\r\nDEPTH 2\r\nMAXVAL 65535\r\nTUPLTYPE GRAYSCALE_ALPHA\r\nENDHDR\r\n",
    );
    data.extend_from_slice(&[255, 255, 128, 0, 128, 0, 255, 255]);
    let bitmap = decode(&data);
    assert_eq!((bitmap.width, bitmap.height), (2, 1));
    assert_eq!(bitmap.pixels, [0x80ffffff, 0xff808080]);
}

#[test]
fn pam_long_supported_fields_need_no_header_sized_metadata_copy() {
    let mut data = b"P7\nWIDTH ".to_vec();
    data.resize(data.len() + 70_000, b'0');
    data.extend_from_slice(b"1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE");
    data.resize(data.len() + 70_000, b' ');
    data.extend_from_slice(b"RGB_ALPHA");
    data.resize(data.len() + 70_000, b'\t');
    data.extend_from_slice(b"\nENDHDR\n\x01\x02\x03\x04");
    assert_eq!(decode(&data).pixels, [0x04010203]);
}

#[test]
fn pam_unsupported_tuple_text_and_multiline_types_stay_explicit() {
    let mut data = long_comment();
    data.extend_from_slice(b"WIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE ");
    for _ in 0..40_000 {
        data.extend_from_slice(b"custom ");
    }
    data.extend_from_slice(b"\nENDHDR\n\0\0\0\0");
    assert!(refusal(&data).contains("Unsupported or inconsistent PAM tuple type"));

    for tuples in [
        b"TUPLTYPE RGB_ALPHA\nTUPLTYPE RGB_ALPHA\n".as_slice(),
        b"TUPLTYPE RGB\nTUPLTYPE _ALPHA\n",
        b"TUPLTYPE RGB  _ALPHA\n",
    ] {
        let mut data = long_comment();
        data.extend_from_slice(b"WIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\n");
        data.extend_from_slice(tuples);
        data.extend_from_slice(b"ENDHDR\n\0\0\0\0");
        assert!(refusal(&data).contains("Unsupported or inconsistent PAM tuple type"));
    }
}

#[test]
fn pam_late_malformed_fields_ascii_and_raster_still_refuse() {
    let header = b"WIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\n";
    for (tail, expected) in [
        (b"WIDTH 1\nENDHDR\n\0".as_slice(), "Duplicate or invalid PAM header field"),
        (b"OTHER 1\nENDHDR\n\0", "Unsupported PAM header field OTHER"),
        (b"TOOLONGKEY 1\nENDHDR\n\0", "field name exceeds eight"),
        (b"TUPLTYPE \t\nENDHDR\n\0", "tuple type is empty"),
        (b"ENDHDR extra\n\0", "Invalid PAM ENDHDR"),
        (b"ENDHDR", "Truncated PAM header"),
        (b"ENDHDR\n", "Truncated PAM raster"),
        (b"# non-ascii \xff\nENDHDR\n\0", "header must be ASCII"),
    ] {
        let mut data = long_comment();
        data.extend_from_slice(header);
        data.extend_from_slice(tail);
        let error = refusal(&data);
        assert!(error.contains(expected), "expected {expected}; got {error}");
    }
    for invalid in [b"WIDTH +1\n".as_slice(), b"WIDTH 1x\n", b"WIDTH 1 2\n"] {
        let mut data = long_comment();
        data.extend_from_slice(invalid);
        assert!(refusal(&data).contains("PAM header"));
    }
}

#[test]
fn pam_cancellation_reaches_past_old_header_boundary_without_newline() {
    let mut data = long_comment();
    data.pop(); // One unfinished long line must remain interruptible.
    let calls = Cell::new(0);
    let error = crate::image::decode_asset_checked(&data, &|| {
        calls.set(calls.get() + 1);
        if calls.get() >= 22 {
            Err("cancelled after long PAM header prefix".into())
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    // Both public entry checks and the signature take three calls. Eighteen
    // 4096-byte chunks precede cancellation, beyond the former 64 KiB ceiling;
    // decode_checked makes one final cancellation poll while returning.
    assert_eq!(calls.get(), 23);
    assert_eq!(error, "cancelled after long PAM header prefix");

    let mut many = b"P7\n".to_vec();
    for _ in 0..25_000 {
        many.extend_from_slice(b"# x\n");
    }
    let calls = Cell::new(0);
    let error = crate::image::decode_asset_checked(&many, &|| {
        calls.set(calls.get() + 1);
        if calls.get() >= 20_000 {
            Err("cancelled during many PAM lines".into())
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(calls.get(), 20_001);
    assert_eq!(error, "cancelled during many PAM lines");
}

#[test]
fn pam_keeps_image_admission_dimensions_samples_and_depth_guards() {
    let mut oversized = b"P7\n#".to_vec();
    oversized.resize(crate::image::MAX_INPUT_BYTES + 1, b'x');
    assert!(refusal(&oversized).contains("32 MiB"));
    for (fields, expected) in [
        (b"WIDTH 0\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\n".as_slice(), "dimension"),
        (b"WIDTH 16385\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\n", "dimension"),
        (b"WIDTH 2049\nHEIGHT 2049\nDEPTH 1\nMAXVAL 255\n", "pixel"),
        (b"WIDTH 1\nHEIGHT 1\nDEPTH 5\nMAXVAL 255\n", "one through four"),
        (b"WIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 65536\n", "up to 16 bits"),
        (b"WIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 15\n", "sample exceeds"),
    ] {
        let mut data = long_comment();
        data.extend_from_slice(fields);
        data.extend_from_slice(b"ENDHDR\n\x10");
        let error = refusal(&data);
        assert!(error.contains(expected), "expected {expected}; got {error}");
    }
}
