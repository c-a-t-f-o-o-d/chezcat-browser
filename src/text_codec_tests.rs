//! Original witnesses for the shared codec boundary and bounded resource sink.
use super::*;
use std::cell::Cell;

fn resource(label: &str, bytes: &[u8]) -> String {
    decode(bytes, Encoding::label(label).unwrap(), usize::MAX, &|| Ok(())).unwrap()
}

#[test]
fn codec_all_pinned_labels_preserve_canonical_names_and_exact_normalization() {
    // This inventory reader is deliberately separate from the generated lookup:
    // the pinned JSON is line formatted with ASCII label/name string values.
    let mut in_labels = false;
    let mut labels = Vec::new();
    let mut encodings = 0;
    let mut label_count = 0;
    for line in include_str!("../tools/reference/text-encodings/encodings.json").lines() {
        let line = line.trim();
        if line.starts_with("\"labels\":") {
            assert!(labels.is_empty());
            in_labels = true;
        } else if in_labels && line.starts_with(']') {
            in_labels = false;
        } else if in_labels {
            let value = line.trim_end_matches(',').strip_prefix('"').unwrap().strip_suffix('"').unwrap();
            assert!(value.is_ascii() && !value.contains('\\'));
            labels.push(value);
        } else if let Some(name) = line.strip_prefix("\"name\": \"") {
            let name = name.strip_suffix('"').unwrap().to_ascii_lowercase();
            assert!(!labels.is_empty());
            for label in labels.drain(..) {
                let encoding = Encoding::label(label).unwrap();
                assert_eq!(encoding.name(), name, "{label}");
                assert_eq!(Encoding::label(&format!(" \t\n\u{c}\r{}\r\n\t ", label.to_ascii_uppercase())), Some(encoding));
                label_count += 1;
            }
            encodings += 1;
        }
    }
    assert_eq!((encodings, label_count), (40, 228));
    assert!(!in_labels && labels.is_empty());
    for label in ["", "\u{b}utf-8", "utf-8\u{b}", "\u{a0}utf-8", "utf-8\u{a0}", "utf 8", "utf-8\0", "unrecognized"] {
        assert_eq!(Encoding::label(label), None, "{label:?}");
    }
    assert_eq!(Encoding::label(&format!("{}UTF8{}", " ".repeat(4096), "\t".repeat(4096))), Some(Encoding::UTF8));
    assert_eq!(Encoding::label(&"x".repeat(4096)), None);
    assert_eq!(Encoding::label("windows-1252"), Some(Encoding::WINDOWS1252));
}

#[test]
fn codec_resource_bom_overrides_fallback_and_removes_exactly_one_marker() {
    for (bytes, encoding, expected) in [
        (&b"\xef\xbb\xbfA"[..], Encoding::UTF8, "A"),
        (&b"\xff\xfeA\0"[..], Encoding::UTF16LE, "A"),
        (&b"\xfe\xff\0A"[..], Encoding::UTF16BE, "A"),
        (&b"\xef\xbb\xbf\xef\xbb\xbfA"[..], Encoding::UTF8, "\u{feff}A"),
        (&b"\xff\xfe\xff\xfeA\0"[..], Encoding::UTF16LE, "\u{feff}A"),
        (&b"\xfe\xff\xfe\xff\0A"[..], Encoding::UTF16BE, "\u{feff}A"),
    ] {
        assert_eq!(Encoding::bom(bytes), Some(encoding));
        for fallback in [Encoding::REPLACEMENT, Encoding::WINDOWS1252, Encoding::UTF8] {
            assert_eq!(decode(bytes, fallback, expected.len(), &|| Ok(())).unwrap(), expected);
        }
    }
    for bytes in [&b"\xef\xbb\xbf"[..], &b"\xff\xfe"[..], &b"\xfe\xff"[..]] {
        assert_eq!(decode(bytes, Encoding::REPLACEMENT, 0, &|| Ok(())).unwrap(), "");
    }
    for bytes in [&b""[..], &b"\xef\xbb"[..], &b"\xff"[..], &b"A\xef\xbb\xbf"[..]] {
        assert_eq!(Encoding::bom(bytes), None);
    }
    assert_eq!(resource("replacement", b""), "");
    assert_eq!(resource("replacement", b"\x1b$B<script>never decoded</script>\xff"), "\u{fffd}");
}

#[test]
fn codec_resource_legacy_pairs_and_persistent_designations_share_original_state() {
    for (label, bytes, expected) in [
        ("windows-1252", &b"A\x80\xe9"[..], "A€é"),
        ("windows-1251", &b"\xc6"[..], "Ж"),
        ("big5", &b"\x88\x62\x88\x64"[..], "\u{ca}\u{304}\u{ca}\u{30c}"),
        ("euc-jp", &b"\xa4\xa2"[..], "あ"),
        ("shift_jis", &b"\x82\xa0"[..], "あ"),
        ("iso-2022-jp", &b"\x1b$B$\"\x1b(J\\~\x1b(BZ"[..], "あ¥‾Z"),
        ("gb18030", &b"\x90\x30\x81\x30"[..], "\u{10000}"),
        ("euc-kr", &b"\xb0\xa1"[..], "가"),
        ("x-user-defined", &b"A\x80\xff"[..], "A\u{f780}\u{f7ff}"),
    ] {
        assert_eq!(resource(label, bytes), expected, "{label}");
    }
}

#[test]
fn codec_resource_malformed_sequences_restore_ascii_delimiters() {
    for (label, bytes, expected) in [
        ("utf-8", &b"\xe2\x82\""[..], "\u{fffd}\""),
        ("utf-16le", &b"\0\xd8\"\0"[..], "\u{fffd}\""),
        ("big5", &b"\x81\""[..], "\u{fffd}\""),
        ("shift_jis", &b"\x81\""[..], "\u{fffd}\""),
        ("euc-jp", &b"\x8f\xa1\""[..], "\u{fffd}\""),
        ("euc-kr", &b"\x81["[..], "\u{fffd}["),
        ("gb18030", &b"\x81\x30\x81\""[..], "\u{fffd}0\u{fffd}\""),
        ("iso-2022-jp", &b"\x1b$(Z"[..], "\u{fffd}$(Z"),
    ] {
        assert_eq!(resource(label, bytes), expected, "{label}");
    }
}

fn chunk(decoder: &mut Decoder, bytes: &[u8], stream: bool) -> String {
    let mut input = decoder.pending[..decoder.pending_len].to_vec();
    input.extend_from_slice(bytes);
    decoder.pending_len = 0;
    let mut output = String::new();
    decoder.decode_to(&input, stream, |text| { output.push_str(text); Ok::<_, ()>(()) }).unwrap();
    output
}

#[test]
fn codec_streaming_partitions_keep_prefix_bom_and_iso2022_state() {
    for (label, bytes, expected) in [
        ("utf-8", &b"\xef\xbb\xbfA\xf0\x90\x80\x80\xe2\x82"[..], "A\u{10000}\u{fffd}"),
        ("utf-16le", &b"\xff\xfeA\0\0\xd8\0\xdc\0"[..], "A\u{10000}\u{fffd}"),
        ("utf-16be", &b"\xfe\xff\0A\xd8\0\xdc\0\0"[..], "A\u{10000}\u{fffd}"),
        ("big5", &b"\x88\x62\x81"[..], "\u{ca}\u{304}\u{fffd}"),
        ("euc-jp", &b"\xa4\xa2\x8f\xa1"[..], "あ\u{fffd}"),
        ("iso-2022-jp", &b"\x1b$B$\"\x1b(J\\~\x1b(BZ\x1b$"[..], "あ¥‾Z\u{fffd}$"),
    ] {
        for split in 0..=bytes.len() {
            let mut decoder = Decoder::new(Encoding::label(label).unwrap(), false, false);
            let mut output = chunk(&mut decoder, &bytes[..split], true);
            assert_eq!(chunk(&mut decoder, &[], true), "");
            output.push_str(&chunk(&mut decoder, &bytes[split..], true));
            output.push_str(&chunk(&mut decoder, &[], false));
            assert_eq!(output, expected, "{label}, split {split}");
            assert_eq!(decoder.pending_len, 0);
        }
    }
}

#[test]
fn codec_fatal_errors_and_sink_refusals_preserve_separate_failure_policy() {
    let mut decoder = Decoder::new(Encoding::label("iso-2022-jp").unwrap(), true, true);
    decoder.streaming = true;
    assert_eq!(chunk(&mut decoder, b"\x1b$B", true), "");
    assert_eq!(decoder.decode_to(b"\xff", false, |_| Ok::<_, &str>(())), Err(DecodeError::Invalid));
    assert!(!decoder.streaming);
    assert_eq!(decoder.pending_len, 0);
    decoder.reset();
    assert!(decoder.fatal && decoder.ignore_bom);
    assert_eq!(chunk(&mut decoder, b"\\", false), "\\");

    let mut decoder = Decoder::new(Encoding::label("big5").unwrap(), false, false);
    let mut calls = 0;
    let result = decoder.decode_to(b"\x88\x62Z", false, |text| {
        calls += 1;
        assert_eq!(text, "\u{ca}\u{304}");
        Err("bounded sink refused the complete pair")
    });
    assert_eq!(result, Err(DecodeError::Sink("bounded sink refused the complete pair")));
    assert_eq!(calls, 1);
}

#[test]
fn codec_resource_output_admission_is_exact_and_failure_does_not_poison_recovery() {
    for (label, bytes, expected) in [
        ("utf-8", &b"A\xff"[..], "A\u{fffd}"),
        ("windows-1252", &b"\x80"[..], "€"),
        ("big5", &b"\x88\x62"[..], "\u{ca}\u{304}"),
        ("iso-2022-jp", &b"\x1b$B$\""[..], "あ"),
        ("replacement", &b"unpublished input"[..], "\u{fffd}"),
    ] {
        let encoding = Encoding::label(label).unwrap();
        assert!(decode(bytes, encoding, expected.len() - 1, &|| Ok(())).is_err(), "{label}");
        assert_eq!(decode(bytes, encoding, expected.len(), &|| Ok(())).unwrap(), expected);
        assert_eq!(decode(b"", encoding, 0, &|| Ok(())).unwrap(), "");
    }
    assert_eq!(decode(b"later", Encoding::UTF8, 5, &|| Ok(())).unwrap(), "later");
}

#[test]
fn codec_resource_cancellation_checks_both_passes_before_publication() {
    let input = vec![b'A'; 20_000];
    // These cancellation points reach the start, the size pass, and the output
    // pass. The caller receives only the original refusal, never a prefix.
    for stop in [1, 3, 9] {
        let checks = Cell::new(0);
        let result = decode(&input, Encoding::UTF8, input.len(), &|| {
            checks.set(checks.get() + 1);
            if checks.get() == stop { Err("original cancellation".into()) } else { Ok(()) }
        });
        assert_eq!(result, Err("original cancellation".into()));
        assert_eq!(checks.get(), stop);
    }
    assert_eq!(decode(b"", Encoding::UTF8, 0, &|| Err("empty resource cancelled".into())), Err("empty resource cancelled".into()));
    assert_eq!(decode(b"recovered", Encoding::UTF8, 9, &|| Ok(())).unwrap(), "recovered");
}
