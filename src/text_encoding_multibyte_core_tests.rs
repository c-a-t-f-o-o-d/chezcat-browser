//! Original codec witnesses selected before implementation execution.
//! Expected mappings come from the retained pinned standard text, not the
//! generated Rust tables. Queue-oracle tests use a stateful restoration model
//! distinct from the production prefix/consumed-count implementation.

use super::{Encoding, Step};
use std::collections::VecDeque;

const GB_INDEX: &str = include_str!("../tools/reference/multibyte-encodings/index-gb18030.txt");
const KR_INDEX: &str = include_str!("../tools/reference/multibyte-encodings/index-euc-kr.txt");
const GB_RANGES: &str =
    include_str!("../tools/reference/multibyte-encodings/index-gb18030-ranges.txt");
const ENCODINGS: [Encoding; 3] = [Encoding::Gbk, Encoding::Gb18030, Encoding::EucKr];

fn rows(source: &str) -> Vec<(u32, u32)> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut fields = line.split_whitespace();
            let pointer = fields.next().unwrap().parse().unwrap();
            let scalar = fields.next().unwrap().strip_prefix("0x").unwrap();
            Some((pointer, u32::from_str_radix(scalar, 16).unwrap()))
        })
        .collect()
}

fn raw_index(source: &str, expected_count: usize) -> Vec<Option<char>> {
    raw_index_sized(source, expected_count, 23_940)
}

fn raw_index_sized(source: &str, expected_count: usize, size: usize) -> Vec<Option<char>> {
    let mut index = vec![None; size];
    let raw = rows(source);
    assert_eq!(raw.len(), expected_count);
    for (pointer, scalar) in raw {
        assert!(index[pointer as usize]
            .replace(char::from_u32(scalar).unwrap())
            .is_none());
    }
    index
}

struct Oracle {
    gb: Vec<Option<char>>,
    kr: Vec<Option<char>>,
    ranges: Vec<(u32, u32)>,
}

impl Oracle {
    fn new() -> Self {
        let ranges = rows(GB_RANGES);
        assert_eq!(ranges.len(), 207);
        assert_eq!(ranges.first(), Some(&(0, 0x80)));
        assert_eq!(ranges.last(), Some(&(189_000, 0x1_0000)));
        assert!(ranges.windows(2).all(|p| p[0].0 < p[1].0 && p[0].1 < p[1].1));
        Self {
            gb: raw_index(GB_INDEX, 23_940),
            kr: raw_index(KR_INDEX, 17_048),
            ranges,
        }
    }

    fn range(&self, pointer: u32) -> Option<char> {
        if pointer > 1_237_575 || (pointer > 39_419 && pointer < 189_000) {
            return None;
        }
        if pointer == 7_457 {
            return Some('\u{e7c7}');
        }
        let &(start, scalar) = self.ranges.iter().rev().find(|&&(p, _)| p <= pointer)?;
        char::from_u32(scalar + pointer - start)
    }

    fn decode(&self, encoding: Encoding, bytes: &[u8]) -> Vec<char> {
        if encoding == Encoding::EucKr {
            self.korean(bytes)
        } else {
            self.chinese(bytes)
        }
    }

    fn chinese(&self, bytes: &[u8]) -> Vec<char> {
        let mut queue: VecDeque<u8> = bytes.iter().copied().collect();
        let mut first = None;
        let mut second = None;
        let mut third = None;
        let mut output = Vec::new();
        loop {
            let Some(byte) = queue.pop_front() else {
                if first.is_some() {
                    output.push('\u{fffd}');
                }
                return output;
            };
            if let Some(saved_third) = third.take() {
                let saved_first = first.take().unwrap();
                let saved_second = second.take().unwrap();
                if (0x30..=0x39).contains(&byte) {
                    let pointer = (u32::from(saved_first) - 0x81) * 10 * 126 * 10
                        + (u32::from(saved_second) - 0x30) * 126 * 10
                        + (u32::from(saved_third) - 0x81) * 10
                        + u32::from(byte) - 0x30;
                    output.push(self.range(pointer).unwrap_or('\u{fffd}'));
                } else {
                    queue.push_front(byte);
                    queue.push_front(saved_third);
                    queue.push_front(saved_second);
                    output.push('\u{fffd}');
                }
                continue;
            }
            if let Some(saved_second) = second {
                if (0x81..=0xfe).contains(&byte) {
                    third = Some(byte);
                } else {
                    first = None;
                    second = None;
                    queue.push_front(byte);
                    queue.push_front(saved_second);
                    output.push('\u{fffd}');
                }
                continue;
            }
            if let Some(saved_first) = first {
                if (0x30..=0x39).contains(&byte) {
                    second = Some(byte);
                    continue;
                }
                first = None;
                let mapped = if (0x40..=0x7e).contains(&byte) || (0x80..=0xfe).contains(&byte) {
                    let offset = if byte < 0x7f { 0x40 } else { 0x41 };
                    self.gb[(usize::from(saved_first) - 0x81) * 190 + usize::from(byte - offset)]
                } else {
                    None
                };
                match mapped {
                    Some(scalar) => output.push(scalar),
                    None => {
                        if byte < 0x80 {
                            queue.push_front(byte);
                        }
                        output.push('\u{fffd}');
                    }
                }
                continue;
            }
            match byte {
                0..=0x7f => output.push(char::from(byte)),
                0x80 => output.push('\u{20ac}'),
                0x81..=0xfe => first = Some(byte),
                _ => output.push('\u{fffd}'),
            }
        }
    }

    fn korean(&self, bytes: &[u8]) -> Vec<char> {
        let mut queue: VecDeque<u8> = bytes.iter().copied().collect();
        let mut lead = None;
        let mut output = Vec::new();
        loop {
            let Some(byte) = queue.pop_front() else {
                if lead.is_some() {
                    output.push('\u{fffd}');
                }
                return output;
            };
            if let Some(saved_lead) = lead.take() {
                let mapped = if (0x41..=0xfe).contains(&byte) {
                    self.kr[(usize::from(saved_lead) - 0x81) * 190 + usize::from(byte - 0x41)]
                } else {
                    None
                };
                match mapped {
                    Some(scalar) => output.push(scalar),
                    None => {
                        if byte < 0x80 {
                            queue.push_front(byte);
                        }
                        output.push('\u{fffd}');
                    }
                }
            } else if byte < 0x80 {
                output.push(char::from(byte));
            } else if (0x81..=0xfe).contains(&byte) {
                lead = Some(byte);
            } else {
                output.push('\u{fffd}');
            }
        }
    }
}

struct ChunkDecoder {
    encoding: Encoding,
    pending: Vec<u8>,
}

impl ChunkDecoder {
    fn new(encoding: Encoding) -> Self {
        Self { encoding, pending: Vec::new() }
    }

    fn feed(&mut self, bytes: &[u8], stream: bool) -> Vec<char> {
        self.pending.extend_from_slice(bytes);
        let mut output = Vec::new();
        let mut consumed = 0;
        while consumed < self.pending.len() {
            let remaining = &self.pending[consumed..];
            let count = match self.encoding.step(remaining, stream) {
                Step::Character(scalar, count) => {
                    output.push(scalar);
                    count
                }
                Step::Pair(first, second, count) => {
                    output.extend([first, second]);
                    count
                }
                Step::Invalid(count) => {
                    output.push('\u{fffd}');
                    count
                }
                Step::Incomplete => {
                    assert!(stream, "EOF must settle any nonempty prefix");
                    break;
                }
            };
            assert!(count > 0 && count <= remaining.len(), "invalid forward progress");
            consumed += count;
        }
        self.pending.drain(..consumed);
        let maximum = match self.encoding {
            Encoding::Gbk | Encoding::Gb18030 => 3,
            Encoding::EucJp => 2,
            Encoding::Big5 | Encoding::ShiftJis | Encoding::EucKr => 1,
            Encoding::Iso2022Jp => unreachable!("stateful decoder has its own tests"),
        };
        assert!(self.pending.len() <= maximum, "too much retained input");
        if !stream {
            assert!(self.pending.is_empty());
        }
        output
    }
}

fn core_output(encoding: Encoding, bytes: &[u8]) -> Vec<char> {
    ChunkDecoder::new(encoding).feed(bytes, false)
}

fn four_bytes(pointer: u32) -> [u8; 4] {
    assert!(pointer < 126 * 10 * 126 * 10);
    [
        (pointer / 12_600) as u8 + 0x81,
        ((pointer / 1_260) % 10) as u8 + 0x30,
        ((pointer / 10) % 126) as u8 + 0x81,
        (pointer % 10) as u8 + 0x30,
    ]
}

#[test]
fn multibyte_canonical_names_aliases_and_empty_prefix() {
    let groups: &[(Encoding, &str, &[&str])] = &[
        (Encoding::Gbk, "gbk", &[
            "chinese", "csgb2312", "csiso58gb231280", "gb2312", "gb_2312", "gb_2312-80",
            "gbk", "iso-ir-58", "x-gbk",
        ]),
        (Encoding::Gb18030, "gb18030", &["gb18030"]),
        (Encoding::Big5, "big5", &["big5", "big5-hkscs", "cn-big5", "csbig5", "x-x-big5"]),
        (Encoding::EucJp, "euc-jp", &["cseucpkdfmtjapanese", "euc-jp", "x-euc-jp"]),
        (Encoding::ShiftJis, "shift_jis", &[
            "csshiftjis", "ms932", "ms_kanji", "shift-jis", "shift_jis", "sjis",
            "windows-31j", "x-sjis",
        ]),
        (Encoding::EucKr, "euc-kr", &[
            "cseuckr", "csksc56011987", "euc-kr", "iso-ir-149", "korean", "ks_c_5601-1987",
            "ks_c_5601-1989", "ksc5601", "ksc_5601", "windows-949",
        ]),
    ];
    for &(encoding, name, aliases) in groups {
        assert_eq!(encoding.name(), name);
        for label in aliases {
            assert_eq!(Encoding::from_label(label), Some(encoding), "{label}");
        }
        assert_eq!(encoding.step(&[], true), Step::Incomplete);
        assert_eq!(encoding.step(&[], false), Step::Incomplete);
    }
    assert_eq!(Encoding::Iso2022Jp.name(), "iso-2022-jp");
    for label in ["csiso2022jp", "iso-2022-jp"] {
        assert_eq!(Encoding::from_label(label), Some(Encoding::Iso2022Jp));
    }
    for label in ["GBK", " gbk", "euc-kr\n", "utf-8", "cp932", "big5-extra", "gb18030-extra", ""] {
        assert_eq!(Encoding::from_label(label), None, "{label:?}");
    }
}

#[test]
fn multibyte_every_single_byte_has_exact_stream_and_eof_behavior() {
    for encoding in ENCODINGS {
        for byte in 0u8..=255 {
            for stream in [false, true] {
                let expected = match byte {
                    0..=0x7f => Step::Character(char::from(byte), 1),
                    0x80 if encoding != Encoding::EucKr => Step::Character('\u{20ac}', 1),
                    0x81..=0xfe if stream => Step::Incomplete,
                    _ => Step::Invalid(1),
                };
                assert_eq!(encoding.step(&[byte], stream), expected, "{encoding:?} {byte:x} {stream}");
            }
        }
    }
}

#[test]
fn multibyte_every_two_byte_index_position_matches_pinned_raw_data() {
    let oracle = Oracle::new();
    for pointer in 0..23_940 {
        let lead = (pointer / 190) as u8 + 0x81;
        let remainder = (pointer % 190) as u8;
        let gb_trail = remainder + if remainder < 63 { 0x40 } else { 0x41 };
        let kr_trail = remainder + 0x41;
        for encoding in ENCODINGS {
            let (trail, mapped) = if encoding == Encoding::EucKr {
                (kr_trail, oracle.kr[pointer])
            } else {
                (gb_trail, oracle.gb[pointer])
            };
            let expected = match mapped {
                Some(scalar) => Step::Character(scalar, 2),
                None => Step::Invalid(if trail < 0x80 { 1 } else { 2 }),
            };
            for stream in [false, true] {
                assert_eq!(encoding.step(&[lead, trail], stream), expected,
                    "{encoding:?} pointer {pointer}, stream {stream}");
            }
        }
    }
}

#[test]
fn multibyte_all_byte_pairs_match_independent_restore_queue_oracle() {
    let oracle = Oracle::new();
    for encoding in ENCODINGS {
        for first in 0u8..=255 {
            for second in 0u8..=255 {
                let bytes = [first, second];
                assert_eq!(core_output(encoding, &bytes), oracle.decode(encoding, &bytes),
                    "{encoding:?} {bytes:x?}");
            }
        }
    }
}

#[test]
fn multibyte_every_gb18030_four_byte_pointer_matches_pinned_raw_ranges() {
    let oracle = Oracle::new();
    let mut range = 0;
    let mut mapped = 0;
    for pointer in 0..1_587_600 {
        while range + 1 < oracle.ranges.len() && oracle.ranges[range + 1].0 <= pointer {
            range += 1;
        }
        // A monotone walk over parsed raw rows is independent of the core's
        // binary search over the generated table.
        let expected = if (39_420..189_000).contains(&pointer) || pointer > 1_237_575 {
            Step::Invalid(4)
        } else {
            let (start, scalar) = oracle.ranges[range];
            let scalar = if pointer == 7_457 { 0xe7c7 } else { scalar + pointer - start };
            mapped += 1;
            Step::Character(char::from_u32(scalar).unwrap(), 4)
        };
        let bytes = four_bytes(pointer);
        for encoding in [Encoding::Gbk, Encoding::Gb18030] {
            assert_eq!(encoding.step(&bytes, false), expected, "{encoding:?} pointer {pointer}");
            assert_eq!(encoding.step(&bytes, true), expected, "{encoding:?} pointer {pointer}, stream");
        }
    }
    assert_eq!(mapped, 1_087_996);
}

#[test]
fn multibyte_recovery_eof_range_and_compatibility_anchors() {
    for encoding in [Encoding::Gbk, Encoding::Gb18030] {
        assert_eq!(encoding.step(&[0x81, 0x30], false), Step::Invalid(2));
        assert_eq!(encoding.step(&[0x81, 0x30], true), Step::Incomplete);
        assert_eq!(encoding.step(&[0x81, 0x30, 0x81], false), Step::Invalid(3));
        assert_eq!(encoding.step(&[0x81, 0x30, 0x81], true), Step::Incomplete);
        assert_eq!(encoding.step(&[0x81, 0x30, 0x22], true), Step::Invalid(1));
        assert_eq!(encoding.step(&[0x81, 0x30, 0x81, 0x22], true), Step::Invalid(1));
        assert_eq!(encoding.step(&[0xa3, 0xa0], false), Step::Character('\u{3000}', 2));
        assert_eq!(encoding.step(&[0x81, 0xff], false), Step::Invalid(2));
        assert_eq!(encoding.step(&[0x81, 0x22], false), Step::Invalid(1));
        for (pointer, scalar) in [(0, '\u{80}'), (7_457, '\u{e7c7}'), (39_419, '\u{ffff}'),
            (189_000, '\u{10000}'), (1_237_575, '\u{10ffff}')] {
            assert_eq!(encoding.step(&four_bytes(pointer), false), Step::Character(scalar, 4));
        }
        for pointer in [39_420, 188_999, 1_237_576, 1_587_599] {
            assert_eq!(encoding.step(&four_bytes(pointer), false), Step::Invalid(4));
        }
    }
    assert_eq!(Encoding::EucKr.step(&[0x81, 0x5b], false), Step::Invalid(1));
    assert_eq!(Encoding::EucKr.step(&[0x81, 0x7f], false), Step::Invalid(1));
    assert_eq!(Encoding::EucKr.step(&[0x81, 0x80], false), Step::Invalid(2));
    assert_eq!(Encoding::EucKr.step(&[0x81, 0xff], false), Step::Invalid(2));
    assert_eq!(Encoding::EucKr.step(&[0x81, 0x22], false), Step::Invalid(1));
    assert_eq!(core_output(Encoding::Gb18030, &[0x81, 0x30, 0x81, 0x22]),
        vec!['\u{fffd}', '0', '\u{fffd}', '"']);
    assert_eq!(core_output(Encoding::EucKr, &[0x81, 0x5b]), vec!['\u{fffd}', '[']);
}

#[test]
fn multibyte_all_third_and_fourth_bytes_preserve_queue_restoration() {
    let oracle = Oracle::new();
    for lead in [0x81, 0xae, 0xfe] {
        for digit in [0x30, 0x35, 0x39] {
            for byte in 0u8..=255 {
                let prefix = [lead, digit, byte];
                for encoding in [Encoding::Gbk, Encoding::Gb18030] {
                    let (stream, end) = if (0x81..=0xfe).contains(&byte) {
                        (Step::Incomplete, Step::Invalid(3))
                    } else {
                        (Step::Invalid(1), Step::Invalid(1))
                    };
                    assert_eq!(encoding.step(&prefix, true), stream, "{prefix:x?}");
                    assert_eq!(encoding.step(&prefix, false), end, "{prefix:x?}");
                    assert_eq!(core_output(encoding, &prefix), oracle.decode(encoding, &prefix));
                }
                for third in [0x81, 0xb0, 0xfe] {
                    let prefix = [lead, digit, third, byte];
                    for encoding in [Encoding::Gbk, Encoding::Gb18030] {
                        if !(0x30..=0x39).contains(&byte) {
                            assert_eq!(encoding.step(&prefix, true), Step::Invalid(1), "{prefix:x?}");
                        }
                        assert_eq!(core_output(encoding, &prefix), oracle.decode(encoding, &prefix),
                            "{encoding:?} {prefix:x?}");
                    }
                }
            }
        }
    }
}

#[test]
fn multibyte_every_stream_split_and_eof_reset_preserves_output() {
    let oracle = Oracle::new();
    let cases: &[&[u8]] = &[
        &[], &[0], &[0x81], &[0x81, 0x30], &[0x81, 0x30, 0x81],
        &[b'A', 0x81, 0x40, 0x80, b'Z'],
        &[0xb0, 0xa1, 0xc7, 0xd1, 0x81, 0x41, b'!'],
        &[0x81, 0x30, 0x81, 0x30, b'A', 0xa3, 0xa0],
        &[0x81, 0x30, 0x22, 0x81, 0x30, 0x81, 0x22],
        &[0x81, 0x30, 0x81, 0x81, 0x30, 0x81, 0x30],
        &[0x81, 0x5b, 0x81, 0x7f, 0x81, 0x80, 0x81, 0xff],
        &[0xff, 0x80, 0xfe, 0xff, 0x81, 0x22, 0x81],
        &[0x84, 0x31, 0xa5, 0x30, 0x90, 0x30, 0x81, 0x30],
        &[0xe3, 0x32, 0x9a, 0x35, 0xe3, 0x32, 0x9a, 0x36],
    ];
    for encoding in ENCODINGS {
        for bytes in cases {
            let expected = oracle.decode(encoding, bytes);
            for first in 0..=bytes.len() {
                for second in first..=bytes.len() {
                    let mut decoder = ChunkDecoder::new(encoding);
                    let mut output = decoder.feed(&bytes[..first], true);
                    output.extend(decoder.feed(&bytes[first..second], true));
                    output.extend(decoder.feed(&bytes[second..], true));
                    output.extend(decoder.feed(&[], false));
                    assert_eq!(output, expected, "{encoding:?} {bytes:x?} splits {first}/{second}");
                    assert!(decoder.feed(&[], false).is_empty());
                    assert_eq!(decoder.feed(b"reset", false), vec!['r', 'e', 's', 'e', 't']);
                }
            }
            let mut decoder = ChunkDecoder::new(encoding);
            let mut output = Vec::new();
            for byte in *bytes {
                output.extend(decoder.feed(&[*byte], true));
            }
            output.extend(decoder.feed(&[], false));
            assert_eq!(output, expected, "{encoding:?} one-byte chunks {bytes:x?}");
        }
    }
    for encoding in [Encoding::Gbk, Encoding::Gb18030] {
        let mut decoder = ChunkDecoder::new(encoding);
        assert!(decoder.feed(&[0x81, 0x30], true).is_empty());
        assert_eq!(decoder.feed(&[0x22], true), vec!['\u{fffd}', '0', '"']);
        assert!(decoder.feed(&[0x81, 0x30, 0x81], true).is_empty());
        assert_eq!(decoder.feed(&[], false), vec!['\u{fffd}']);
        assert_eq!(decoder.feed(&[0x30], false), vec!['0']);
    }
    let mut decoder = ChunkDecoder::new(Encoding::EucKr);
    assert!(decoder.feed(&[0x81], true).is_empty());
    assert_eq!(decoder.feed(&[0x5b], true), vec!['\u{fffd}', '[']);
}


const BIG5_INDEX: &str = include_str!("../tools/reference/multibyte-encodings/index-big5.txt");
const JIS0208_INDEX: &str = include_str!("../tools/reference/multibyte-encodings/index-jis0208.txt");
const JIS0212_INDEX: &str = include_str!("../tools/reference/multibyte-encodings/index-jis0212.txt");
const NEW_PREFIX_ENCODINGS: [Encoding; 3] = [Encoding::Big5, Encoding::EucJp, Encoding::ShiftJis];

struct EastAsianOracle {
    big5: Vec<Option<char>>,
    jis0208: Vec<Option<char>>,
    jis0212: Vec<Option<char>>,
}

impl EastAsianOracle {
    fn new() -> Self {
        Self {
            big5: raw_index_sized(BIG5_INDEX, 18_590, 126 * 157),
            jis0208: raw_index_sized(JIS0208_INDEX, 7_724, 60 * 188),
            jis0212: raw_index_sized(JIS0212_INDEX, 6_067, 94 * 94),
        }
    }

    fn decode(&self, encoding: Encoding, bytes: &[u8]) -> Vec<char> {
        let mut queue: VecDeque<u8> = bytes.iter().copied().collect();
        let mut leading = None;
        let mut jis0212 = false;
        let mut output = Vec::new();
        loop {
            let Some(byte) = queue.pop_front() else {
                if leading.is_some() {
                    output.push('\u{fffd}');
                }
                return output;
            };
            if let Some(lead) = leading.take() {
                if encoding == Encoding::EucJp {
                    if lead == 0x8e && (0xa1..=0xdf).contains(&byte) {
                        output.push(char::from_u32(0xff61 + u32::from(byte) - 0xa1).unwrap());
                        continue;
                    }
                    if lead == 0x8f && (0xa1..=0xfe).contains(&byte) {
                        leading = Some(byte);
                        jis0212 = true;
                        continue;
                    }
                }
                let mapped = match encoding {
                    Encoding::Big5 => {
                        if (0x40..=0x7e).contains(&byte) || (0xa1..=0xfe).contains(&byte) {
                            let pointer = (usize::from(lead) - 0x81) * 157
                                + usize::from(byte) - if byte < 0x7f { 0x40 } else { 0x62 };
                            let pair = match pointer {
                                1133 => Some(['\u{ca}', '\u{304}']),
                                1135 => Some(['\u{ca}', '\u{30c}']),
                                1164 => Some(['\u{ea}', '\u{304}']),
                                1166 => Some(['\u{ea}', '\u{30c}']),
                                _ => None,
                            };
                            if let Some(pair) = pair {
                                output.extend(pair);
                                continue;
                            }
                            self.big5[pointer]
                        } else {
                            None
                        }
                    }
                    Encoding::EucJp => {
                        let selected = if jis0212 { &self.jis0212 } else { &self.jis0208 };
                        jis0212 = false;
                        if (0xa1..=0xfe).contains(&lead) && (0xa1..=0xfe).contains(&byte) {
                            selected[(usize::from(lead) - 0xa1) * 94 + usize::from(byte) - 0xa1]
                        } else {
                            None
                        }
                    }
                    Encoding::ShiftJis => {
                        if (0x40..=0x7e).contains(&byte) || (0x80..=0xfc).contains(&byte) {
                            let pointer = (usize::from(lead) - if lead < 0xa0 { 0x81 } else { 0xc1 })
                                * 188 + usize::from(byte) - if byte < 0x7f { 0x40 } else { 0x41 };
                            if (8836..=10715).contains(&pointer) {
                                char::from_u32(0xe000 + pointer as u32 - 8836)
                            } else {
                                self.jis0208[pointer]
                            }
                        } else {
                            None
                        }
                    }
                    _ => unreachable!(),
                };
                if let Some(scalar) = mapped {
                    output.push(scalar);
                } else {
                    if byte.is_ascii() {
                        queue.push_front(byte);
                    }
                    output.push('\u{fffd}');
                }
            } else if byte.is_ascii() || (encoding == Encoding::ShiftJis && byte == 0x80) {
                output.push(char::from(byte));
            } else if encoding == Encoding::ShiftJis && (0xa1..=0xdf).contains(&byte) {
                output.push(char::from_u32(0xff61 + u32::from(byte) - 0xa1).unwrap());
            } else {
                let is_lead = match encoding {
                    Encoding::Big5 => (0x81..=0xfe).contains(&byte),
                    Encoding::EucJp => byte == 0x8e || byte == 0x8f || (0xa1..=0xfe).contains(&byte),
                    Encoding::ShiftJis => (0x81..=0x9f).contains(&byte) || (0xe0..=0xfc).contains(&byte),
                    _ => unreachable!(),
                };
                if is_lead {
                    leading = Some(byte);
                } else {
                    output.push('\u{fffd}');
                }
            }
        }
    }
}

fn big5_bytes(pointer: usize) -> [u8; 2] {
    let remainder = pointer % 157;
    [(pointer / 157 + 0x81) as u8,
        (remainder + if remainder < 63 { 0x40 } else { 0x62 }) as u8]
}

fn shift_jis_bytes(pointer: usize) -> [u8; 2] {
    let lead = pointer / 188;
    let trail = pointer % 188;
    [(lead + if lead < 31 { 0x81 } else { 0xc1 }) as u8,
        (trail + if trail < 63 { 0x40 } else { 0x41 }) as u8]
}

#[test]
fn multibyte_big5_every_pointer_preserves_pinned_data_and_two_scalar_exceptions() {
    let oracle = EastAsianOracle::new();
    let mut supplementary = 0;
    for pointer in 0..126 * 157 {
        let bytes = big5_bytes(pointer);
        let expected = match pointer {
            1133 => Step::Pair('\u{ca}', '\u{304}', 2),
            1135 => Step::Pair('\u{ca}', '\u{30c}', 2),
            1164 => Step::Pair('\u{ea}', '\u{304}', 2),
            1166 => Step::Pair('\u{ea}', '\u{30c}', 2),
            _ => match oracle.big5[pointer] {
                Some(scalar) => {
                    supplementary += usize::from(u32::from(scalar) > 0xffff);
                    Step::Character(scalar, 2)
                }
                None => Step::Invalid(if bytes[1].is_ascii() { 1 } else { 2 }),
            },
        };
        for stream in [false, true] {
            assert_eq!(Encoding::Big5.step(&bytes, stream), expected,
                "Big5 pointer {pointer}, stream {stream}");
        }
    }
    assert_eq!(supplementary, 1713);
    assert_eq!(Encoding::Big5.step(&big5_bytes(947), false), Step::Character('\u{27267}', 2));
}

#[test]
fn multibyte_euc_jp_both_planes_match_every_pinned_index_position_and_hole() {
    let oracle = EastAsianOracle::new();
    for pointer in 0..94 * 94 {
        let lead = (pointer / 94 + 0xa1) as u8;
        let trail = (pointer % 94 + 0xa1) as u8;
        for (bytes, mapped) in [
            (vec![lead, trail], oracle.jis0208[pointer]),
            (vec![0x8f, lead, trail], oracle.jis0212[pointer]),
        ] {
            let expected = match mapped {
                Some(scalar) => Step::Character(scalar, bytes.len()),
                None => Step::Invalid(bytes.len()),
            };
            for stream in [false, true] {
                assert_eq!(Encoding::EucJp.step(&bytes, stream), expected,
                    "EUC-JP pointer {pointer}, {bytes:x?}, stream {stream}");
            }
        }
    }
}

#[test]
fn multibyte_shift_jis_every_pointer_covers_pinned_index_holes_and_eudc() {
    let oracle = EastAsianOracle::new();
    for pointer in 0..60 * 188 {
        let bytes = shift_jis_bytes(pointer);
        let scalar = if (8836..=10715).contains(&pointer) {
            char::from_u32(0xe000 + pointer as u32 - 8836)
        } else {
            oracle.jis0208[pointer]
        };
        let expected = match scalar {
            Some(scalar) => Step::Character(scalar, 2),
            None => Step::Invalid(if bytes[1].is_ascii() { 1 } else { 2 }),
        };
        for stream in [false, true] {
            assert_eq!(Encoding::ShiftJis.step(&bytes, stream), expected,
                "Shift_JIS pointer {pointer}, stream {stream}");
        }
    }
    assert_eq!(Encoding::ShiftJis.step(&[0xf0, 0x40], false), Step::Character('\u{e000}', 2));
    assert_eq!(Encoding::ShiftJis.step(&[0xf9, 0xfc], false), Step::Character('\u{e757}', 2));
}

#[test]
fn multibyte_big5_japanese_every_single_byte_preserves_ascii_halfwidth_and_eof() {
    for encoding in NEW_PREFIX_ENCODINGS {
        for byte in 0u8..=255 {
            let is_lead = match encoding {
                Encoding::Big5 => (0x81..=0xfe).contains(&byte),
                Encoding::EucJp => byte == 0x8e || byte == 0x8f || (0xa1..=0xfe).contains(&byte),
                Encoding::ShiftJis => (0x81..=0x9f).contains(&byte) || (0xe0..=0xfc).contains(&byte),
                _ => unreachable!(),
            };
            for stream in [false, true] {
                let expected = if byte.is_ascii() || (encoding == Encoding::ShiftJis && byte == 0x80) {
                    Step::Character(char::from(byte), 1)
                } else if encoding == Encoding::ShiftJis && (0xa1..=0xdf).contains(&byte) {
                    Step::Character(char::from_u32(0xff61 + u32::from(byte) - 0xa1).unwrap(), 1)
                } else if is_lead && stream {
                    Step::Incomplete
                } else {
                    Step::Invalid(1)
                };
                assert_eq!(encoding.step(&[byte], stream), expected,
                    "{encoding:?} byte {byte:x}, stream {stream}");
            }
        }
    }
    for byte in 0xa1..=0xdf {
        assert_eq!(Encoding::EucJp.step(&[0x8e, byte], true),
            Step::Character(char::from_u32(0xff61 + u32::from(byte) - 0xa1).unwrap(), 2));
    }
}

#[test]
fn multibyte_big5_japanese_all_byte_pairs_match_stateful_restoration_oracle() {
    let oracle = EastAsianOracle::new();
    for encoding in NEW_PREFIX_ENCODINGS {
        for first in 0u8..=255 {
            for second in 0u8..=255 {
                let bytes = [first, second];
                assert_eq!(core_output(encoding, &bytes), oracle.decode(encoding, &bytes),
                    "{encoding:?} {bytes:x?}");
            }
        }
    }
}

#[test]
fn multibyte_euc_jp_every_introducer_tail_restores_only_ascii_and_resets_plane() {
    let oracle = EastAsianOracle::new();
    for introducer in [0x8e, 0x8f] {
        for second in 0u8..=255 {
            for third in 0u8..=255 {
                let bytes = [introducer, second, third, 0xa1, 0xa1];
                assert_eq!(core_output(Encoding::EucJp, &bytes), oracle.decode(Encoding::EucJp, &bytes),
                    "{bytes:x?}");
            }
        }
    }
    assert_eq!(core_output(Encoding::EucJp, &[0x8f, 0xa1, b'"']), vec!['\u{fffd}', '"']);
    assert_eq!(core_output(Encoding::EucJp, &[0x8f, b'"']), vec!['\u{fffd}', '"']);
    assert_eq!(core_output(Encoding::EucJp, &[0x8e, 0xe0]), vec!['\u{fffd}']);
}

#[test]
fn multibyte_big5_japanese_all_chunk_splits_and_empty_eof_preserve_output() {
    let oracle = EastAsianOracle::new();
    let cases: &[&[u8]] = &[
        &[], &[0], &[0x80], &[0x81], &[0x8e], &[0x8f], &[0x8f, 0xa1],
        &[0x8f, 0xa1, b'"', 0xa1, 0xa1],
        &[0x8f, 0xa2, 0xaf, 0x8e, 0xa1, 0x8e, 0xdf, b'Z'],
        &[0x8f, 0xa2, 0x8f, 0xa2, 0xaf, b'!'],
        &[0x81, 0x22, 0x81, 0x7f, 0x81, 0x80, 0x81, 0xff],
        &[0x87, 0x45, 0x88, 0x62, 0x88, 0x64, 0x88, 0xa3, 0x88, 0xa5],
        &[0xf0, 0x40, 0xf9, 0xfc, 0xfa, 0x40, 0xfc, 0xfc, 0xff],
        &[0x8f, 0xfe, 0xfe, 0x8f, 0xa2, 0xaf, 0x8f, 0xa2],
    ];
    for encoding in NEW_PREFIX_ENCODINGS {
        for bytes in cases {
            let expected = oracle.decode(encoding, bytes);
            for first in 0..=bytes.len() {
                for second in first..=bytes.len() {
                    let mut decoder = ChunkDecoder::new(encoding);
                    let mut output = decoder.feed(&bytes[..first], true);
                    output.extend(decoder.feed(&bytes[first..second], true));
                    output.extend(decoder.feed(&bytes[second..], true));
                    output.extend(decoder.feed(&[], false));
                    assert_eq!(output, expected, "{encoding:?} {bytes:x?}, splits {first}/{second}");
                    assert!(decoder.feed(&[], false).is_empty());
                    assert_eq!(decoder.feed(b"reset", false), vec!['r', 'e', 's', 'e', 't']);
                }
            }
            let mut decoder = ChunkDecoder::new(encoding);
            let mut output = Vec::new();
            for byte in *bytes {
                output.extend(decoder.feed(&[*byte], true));
            }
            output.extend(decoder.feed(&[], false));
            assert_eq!(output, expected, "{encoding:?} one-byte chunks {bytes:x?}");
        }
    }
    let mut decoder = ChunkDecoder::new(Encoding::EucJp);
    assert!(decoder.feed(&[0x8f, 0xa1], true).is_empty());
    assert_eq!(decoder.feed(&[], false), vec!['\u{fffd}']);
    assert_eq!(decoder.feed(&[0xa1, 0xa1], false), vec!['\u{3000}']);
}
