//! Original state, restoration and split witnesses for ISO-2022-JP.
use super::*;

struct Chunks {
    decoder: Decoder,
    pending: Vec<u8>,
}

impl Chunks {
    fn new() -> Self {
        Self { decoder: Decoder::new(), pending: Vec::new() }
    }

    fn feed(&mut self, bytes: &[u8], stream: bool) -> String {
        self.pending.extend_from_slice(bytes);
        let mut output = String::new();
        let mut at = 0;
        while at < self.pending.len() {
            let consumed = match self.decoder.step(&self.pending[at..], stream) {
                Step::Character(scalar, consumed) => { output.push(scalar); consumed }
                Step::Invalid(consumed) => { output.push('\u{fffd}'); consumed }
                Step::Continue(consumed) => consumed,
                Step::Incomplete => { assert!(stream); break; }
            };
            assert!(consumed > 0 && consumed <= self.pending.len() - at);
            at += consumed;
        }
        self.pending.drain(..at);
        assert!(self.pending.len() <= 2);
        if !stream { assert!(self.pending.is_empty()); }
        output
    }
}

fn output(bytes: &[u8]) -> String {
    Chunks::new().feed(bytes, false)
}

fn splits(bytes: &[u8], expected: &str) {
    assert_eq!(output(bytes), expected, "input {bytes:?}");
    for split in 0..=bytes.len() {
        let mut chunks = Chunks::new();
        let mut actual = chunks.feed(&bytes[..split], true);
        assert!(chunks.feed(&[], true).is_empty());
        actual.push_str(&chunks.feed(&bytes[split..], true));
        actual.push_str(&chunks.feed(&[], false));
        assert_eq!(actual, expected, "input {bytes:?}, split {split}");
    }
    let mut chunks = Chunks::new();
    let mut actual = String::new();
    for byte in bytes {
        actual.push_str(&chunks.feed(&[*byte], true));
        assert!(chunks.feed(&[], true).is_empty());
    }
    actual.push_str(&chunks.feed(&[], false));
    assert_eq!(actual, expected, "single-byte chunks {bytes:?}");
}

#[test]
fn iso2022jp_modes_roman_katakana_jis_and_designation_boundaries() {
    splits(b"A\x1b(J\\~\x1b(I!_\x1b$BF|K\\\x1b(BZ", "A¥‾｡ﾟ日本Z");
    splits(b"\x1b$@F|\x1b(B", "日");
    splits(b"\x1b(B", "");
    splits(b"\x1b(J", "");
    splits(b"\x1b(I", "");
    splits(b"\x1b$B", "");
    splits(b"\x1b$@", "");
    splits(b"\x1b(J\\\x1b(B\x1b(J\\\x1b(B", "¥\u{fffd}¥");
}

#[test]
fn iso2022jp_every_ascii_roman_and_katakana_byte_follows_its_mode() {
    for byte in 0..=255u8 {
        if byte == 0x1b { continue; }
        for (designation, mode) in [(&b"\x1b(B"[..], Mode::Ascii),
            (&b"\x1b(J"[..], Mode::Roman), (&b"\x1b(I"[..], Mode::Katakana)] {
            let expected = match (mode, byte) {
                (Mode::Roman, 0x5c) => '¥',
                (Mode::Roman, 0x7e) => '‾',
                (Mode::Ascii | Mode::Roman, 0..=0x7f) if !matches!(byte, 0x0e | 0x0f) => char::from(byte),
                (Mode::Katakana, 0x21..=0x5f) => char::from_u32(0xff61 + u32::from(byte - 0x21)).unwrap(),
                _ => '\u{fffd}',
            };
            let mut bytes = designation.to_vec();
            bytes.push(byte);
            assert_eq!(output(&bytes), expected.to_string(), "{mode:?}, {byte:02x}");
        }
    }
}

#[test]
fn iso2022jp_jis_grid_matches_independently_parsed_normative_index() {
    let mut table = vec![None; 94 * 94];
    for line in include_str!("../tools/reference/multibyte-encodings/index-jis0208.txt").split('\n') {
        if line.is_empty() || line.starts_with('#') { continue; }
        let mut columns = line.split('\t');
        let pointer: usize = columns.next().unwrap().trim().parse().unwrap();
        let code = u32::from_str_radix(columns.next().unwrap().strip_prefix("0x").unwrap(), 16).unwrap();
        if pointer < table.len() { table[pointer] = char::from_u32(code); }
    }
    for (pointer, scalar) in table.into_iter().enumerate() {
        let bytes = [0x1b, 0x24, 0x42, (pointer / 94) as u8 + 0x21, (pointer % 94) as u8 + 0x21];
        assert_eq!(output(&bytes), scalar.unwrap_or('\u{fffd}').to_string(), "JIS pointer {pointer}");
    }
}

#[test]
fn iso2022jp_repeated_designations_and_error_recovery_keep_output_mode() {
    let designations: &[&[u8]] = &[b"\x1b(B", b"\x1b(J", b"\x1b(I", b"\x1b$@", b"\x1b$B"];
    for first in designations {
        for second in designations {
            let mut bytes = first.to_vec(); bytes.extend_from_slice(second);
            splits(&bytes, "\u{fffd}");
        }
    }
    splits(b"\x1b(J\x1b(I!", "\u{fffd}｡");
    splits(b"\x1b$B\x1b(B\"<", "\u{fffd}\"<");
    splits(b"\x1b(B\x0e\x1b(B", "\u{fffd}");
    splits(b"\x1b$B!\x1b(B\"<", "\u{fffd}\"<");
    splits(b"\x1b$B!\x00\x1b(BZ", "\u{fffd}Z");
    splits(b"\x1b$B!\xff\x1b(BZ", "\u{fffd}Z");
    splits(b"\x1b(J\x1b(\\", "\u{fffd}(¥");
    splits(b"\x1b(I\x1b(\\", "\u{fffd}ｨﾜ");
}

#[test]
fn iso2022jp_incomplete_and_invalid_escape_restores_bytes_and_handles_eof() {
    for (bytes, expected) in [
        (&b"\x1b"[..], "\u{fffd}"),
        (&b"\x1b("[..], "\u{fffd}("),
        (&b"\x1b$"[..], "\u{fffd}$"),
        (&b"\x1bX\"<"[..], "\u{fffd}X\"<"),
        (&b"\x1b(\"<"[..], "\u{fffd}(\"<"),
        (&b"\x1b\x1b(BZ"[..], "\u{fffd}Z"),
        (&b"\x1b(J\x1b"[..], "\u{fffd}"),
        (&b"\x1b(I\x1b("[..], "\u{fffd}ｨ"),
        (&b"\x1b$B!"[..], "\u{fffd}"),
        (&b"\x1b$B\x1b("[..], "\u{fffd}\u{fffd}"),
        (&b"\x1b$B!\x1b"[..], "\u{fffd}\u{fffd}"),
    ] { splits(bytes, expected); }
    // All malformed third bytes are restored, including control delimiters;
    // the prior ASCII mode, not the failed designation, interprets them.
    for leading in [0x24, 0x28] {
        for last in 0..=255u8 {
            if matches!((leading, last), (0x28, 0x42 | 0x4a | 0x49) | (0x24, 0x40 | 0x42)) { continue; }
            let mut expected = String::from("\u{fffd}");
            expected.push(char::from(leading));
            expected.push(if last <= 0x7f && !matches!(last, 0x0e | 0x0f | 0x1b) { char::from(last) } else { '\u{fffd}' });
            splits(&[0x1b, leading, last], &expected);
        }
    }
}
