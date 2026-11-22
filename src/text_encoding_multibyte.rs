//! Original allocation-free Chinese, Korean and Japanese prefix decoders.
//!
//! Algorithms follow WHATWG Encoding at
//! 7d2e3e08e11f57ba1b080390b490d53648c6f69c. Only the separately attributed
//! standard index data is generated; no browser/codec implementation is used.
//! The caller retains an incomplete prefix and owns error/BOM/output policy.

#[path = "text_encoding_multibyte_data.rs"]
mod data;

#[cfg(test)]
#[path = "text_encoding_multibyte_core_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Encoding {
    Gbk,
    Gb18030,
    Big5,
    EucJp,
    Iso2022Jp,
    ShiftJis,
    EucKr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Step {
    Character(char, usize),
    Pair(char, char, usize),
    Invalid(usize),
    Incomplete,
}

impl Encoding {
    // Encoding's ASCII whitespace/case normalization belongs to the caller.
    pub(super) fn from_label(label: &str) -> Option<Self> {
        match data::canonical_name(label)? {
            "gbk" => Some(Self::Gbk),
            "gb18030" => Some(Self::Gb18030),
            "big5" => Some(Self::Big5),
            "euc-jp" => Some(Self::EucJp),
            "iso-2022-jp" => Some(Self::Iso2022Jp),
            "shift_jis" => Some(Self::ShiftJis),
            "euc-kr" => Some(Self::EucKr),
            _ => None,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Gbk => "gbk",
            Self::Gb18030 => "gb18030",
            Self::Big5 => "big5",
            Self::EucJp => "euc-jp",
            Self::Iso2022Jp => "iso-2022-jp",
            Self::ShiftJis => "shift_jis",
            Self::EucKr => "euc-kr",
        }
    }

    /// Decode the first complete scalar, Big5 scalar pair or error from the prefix.
    ///
    /// A consumed count smaller than the candidate sequence restores its
    /// remaining bytes for the next call, as required by Encoding's I/O queue.
    /// For nonempty input, Incomplete is possible only while streaming and
    /// retains at most three bytes. An empty input has no step to produce.
    pub(super) fn step(self, bytes: &[u8], stream: bool) -> Step {
        match self {
            // GBK has its own canonical identity but uses the gb18030 decoder,
            // including its four-byte sequences and single-byte euro sign.
            Self::Gbk | Self::Gb18030 => gb18030_step(bytes, stream),
            Self::Big5 => big5_step(bytes, stream),
            Self::EucJp => euc_jp_step(bytes, stream),
            Self::ShiftJis => shift_jis_step(bytes, stream),
            Self::EucKr => euc_kr_step(bytes, stream),
            // The runtime routes this identity through its persistent decoder.
            Self::Iso2022Jp => unreachable!("ISO-2022-JP requires its persistent decoder state"),
        }
    }
}

pub(super) fn jis0208_index(pointer: usize) -> Option<char> {
    data::jis0208_index(pointer)
}

fn unfinished(length: usize, stream: bool) -> Step {
    if stream {
        Step::Incomplete
    } else {
        // EOF consumes one incomplete sequence, including any saved digits.
        Step::Invalid(length)
    }
}

fn gb18030_step(bytes: &[u8], stream: bool) -> Step {
    let Some(&first) = bytes.first() else {
        return Step::Incomplete;
    };
    match first {
        0x00..=0x7f => return Step::Character(char::from(first), 1),
        0x80 => return Step::Character('\u{20ac}', 1),
        0x81..=0xfe => {}
        _ => return Step::Invalid(1),
    }
    let Some(&second) = bytes.get(1) else {
        return unfinished(1, stream);
    };

    if (0x30..=0x39).contains(&second) {
        let Some(&third) = bytes.get(2) else {
            return unfinished(2, stream);
        };
        if !(0x81..=0xfe).contains(&third) {
            // Restore the second byte and current byte in that order.
            return Step::Invalid(1);
        }
        let Some(&fourth) = bytes.get(3) else {
            return unfinished(3, stream);
        };
        if !(0x30..=0x39).contains(&fourth) {
            // Restore second, third and current byte; only the lead is bad.
            return Step::Invalid(1);
        }
        let pointer = u32::from(first - 0x81) * 12_600
            + u32::from(second - 0x30) * 1_260
            + u32::from(third - 0x81) * 10
            + u32::from(fourth - 0x30);
        return match gb18030_range(pointer) {
            Some(scalar) => Step::Character(scalar, 4),
            None => Step::Invalid(4),
        };
    }

    if (0x40..=0x7e).contains(&second) || (0x80..=0xfe).contains(&second) {
        let offset = if second < 0x7f { 0x40 } else { 0x41 };
        let pointer = usize::from(first - 0x81) * 190 + usize::from(second - offset);
        if let Some(scalar) = data::gb18030_index(pointer) {
            return Step::Character(scalar, 2);
        }
    }
    // Restore an invalid ASCII trail so it cannot swallow a delimiter.
    Step::Invalid(if second.is_ascii() { 1 } else { 2 })
}

fn gb18030_range(pointer: u32) -> Option<char> {
    if (39_420..189_000).contains(&pointer) || pointer > 1_237_575 {
        return None;
    }
    if pointer == 7_457 {
        return Some('\u{e7c7}');
    }
    let upper = data::GB18030_RANGES.partition_point(|&(start, _)| start <= pointer);
    let &(start, code_point) = data::GB18030_RANGES.get(upper.checked_sub(1)?)?;
    char::from_u32(code_point.checked_add(pointer - start)?)
}

fn euc_kr_step(bytes: &[u8], stream: bool) -> Step {
    let Some(&lead) = bytes.first() else {
        return Step::Incomplete;
    };
    if lead.is_ascii() {
        return Step::Character(char::from(lead), 1);
    }
    if !(0x81..=0xfe).contains(&lead) {
        return Step::Invalid(1);
    }
    let Some(&trail) = bytes.get(1) else {
        return unfinished(1, stream);
    };
    if (0x41..=0xfe).contains(&trail) {
        let pointer = usize::from(lead - 0x81) * 190 + usize::from(trail - 0x41);
        if let Some(scalar) = data::euc_kr_index(pointer) {
            return Step::Character(scalar, 2);
        }
    }
    // Undefined index positions also restore ASCII, even inside 0x41..=0x7f.
    Step::Invalid(if trail.is_ascii() { 1 } else { 2 })
}

fn big5_step(bytes: &[u8], stream: bool) -> Step {
    let Some(&lead) = bytes.first() else {
        return Step::Incomplete;
    };
    if lead.is_ascii() {
        return Step::Character(char::from(lead), 1);
    }
    if !(0x81..=0xfe).contains(&lead) {
        return Step::Invalid(1);
    }
    let Some(&trail) = bytes.get(1) else {
        return unfinished(1, stream);
    };
    if (0x40..=0x7e).contains(&trail) || (0xa1..=0xfe).contains(&trail) {
        let offset = if trail < 0x7f { 0x40 } else { 0x62 };
        let pointer = usize::from(lead - 0x81) * 157 + usize::from(trail - offset);
        match pointer {
            1133 => return Step::Pair('\u{ca}', '\u{304}', 2),
            1135 => return Step::Pair('\u{ca}', '\u{30c}', 2),
            1164 => return Step::Pair('\u{ea}', '\u{304}', 2),
            1166 => return Step::Pair('\u{ea}', '\u{30c}', 2),
            _ => {}
        }
        if let Some(scalar) = data::big5_index(pointer) {
            return Step::Character(scalar, 2);
        }
    }
    Step::Invalid(if trail.is_ascii() { 1 } else { 2 })
}

fn euc_jp_step(bytes: &[u8], stream: bool) -> Step {
    let Some(&first) = bytes.first() else {
        return Step::Incomplete;
    };
    if first.is_ascii() {
        return Step::Character(char::from(first), 1);
    }
    if first != 0x8e && first != 0x8f && !(0xa1..=0xfe).contains(&first) {
        return Step::Invalid(1);
    }
    let Some(&second) = bytes.get(1) else {
        return unfinished(1, stream);
    };
    if first == 0x8e && (0xa1..=0xdf).contains(&second) {
        return Step::Character(char::from_u32(0xff61 + u32::from(second - 0xa1)).unwrap(), 2);
    }
    let (lead, trail, consumed, jis0212) = if first == 0x8f && (0xa1..=0xfe).contains(&second) {
        let Some(&third) = bytes.get(2) else {
            return unfinished(2, stream);
        };
        (second, third, 3, true)
    } else {
        (first, second, 2, false)
    };
    if (0xa1..=0xfe).contains(&lead) && (0xa1..=0xfe).contains(&trail) {
        let pointer = usize::from(lead - 0xa1) * 94 + usize::from(trail - 0xa1);
        let scalar = if jis0212 {
            data::jis0212_index(pointer)
        } else {
            data::jis0208_index(pointer)
        };
        if let Some(scalar) = scalar {
            return Step::Character(scalar, consumed);
        }
    }
    // In the three-byte plane, the introducer and lead belong to one failed
    // sequence. Only an ASCII trailing byte returns to the input queue.
    Step::Invalid(if trail.is_ascii() { consumed - 1 } else { consumed })
}

fn shift_jis_step(bytes: &[u8], stream: bool) -> Step {
    let Some(&lead) = bytes.first() else {
        return Step::Incomplete;
    };
    if lead <= 0x80 {
        return Step::Character(char::from(lead), 1);
    }
    if (0xa1..=0xdf).contains(&lead) {
        return Step::Character(char::from_u32(0xff61 + u32::from(lead - 0xa1)).unwrap(), 1);
    }
    if !(0x81..=0x9f).contains(&lead) && !(0xe0..=0xfc).contains(&lead) {
        return Step::Invalid(1);
    }
    let Some(&trail) = bytes.get(1) else {
        return unfinished(1, stream);
    };
    if (0x40..=0x7e).contains(&trail) || (0x80..=0xfc).contains(&trail) {
        let offset = if trail < 0x7f { 0x40 } else { 0x41 };
        let leading_offset = if lead < 0xa0 { 0x81 } else { 0xc1 };
        let pointer = usize::from(lead - leading_offset) * 188 + usize::from(trail - offset);
        if (8836..=10715).contains(&pointer) {
            return Step::Character(char::from_u32(0xe000 + (pointer - 8836) as u32).unwrap(), 2);
        }
        if let Some(scalar) = data::jis0208_index(pointer) {
            return Step::Character(scalar, 2);
        }
    }
    Step::Invalid(if trail.is_ascii() { 1 } else { 2 })
}
