//! Original shared, allocation-free decoder state and scalar delivery.
//!
//! Algorithms and index data retain the pinned WHATWG Encoding provenance in
//! the existing sibling files. Runtime bindings and document loading supply
//! their own output admission; this module has no VM or platform capability.

#[path = "text_encoding_single_byte.rs"]
mod single_byte;
#[path = "text_encoding_multibyte.rs"]
mod multibyte;
#[path = "text_encoding_iso2022jp.rs"]
mod iso2022jp;
#[cfg(test)]
#[path = "text_codec_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Utf8,
    Utf16Le,
    Utf16Be,
    SingleByte(single_byte::SingleByte),
    Multibyte(multibyte::Encoding),
    UserDefined,
    Replacement,
}

/// An encoding identity without exposing the generated index representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Encoding(Kind);
impl Encoding {
    pub(crate) const UTF8: Self = Self(Kind::Utf8);
    pub(crate) const UTF16LE: Self = Self(Kind::Utf16Le);
    pub(crate) const UTF16BE: Self = Self(Kind::Utf16Be);
    pub(crate) const WINDOWS1252: Self = Self(Kind::SingleByte(single_byte::SingleByte::Windows1252));
    pub(crate) const USER_DEFINED: Self = Self(Kind::UserDefined);
    pub(crate) const REPLACEMENT: Self = Self(Kind::Replacement);

    pub(crate) fn name(self) -> &'static str {
        match self.0 {
            Kind::Utf8 => "utf-8",
            Kind::Utf16Le => "utf-16le",
            Kind::Utf16Be => "utf-16be",
            Kind::SingleByte(encoding) => encoding.name(),
            Kind::Multibyte(encoding) => encoding.name(),
            Kind::UserDefined => "x-user-defined",
            Kind::Replacement => "replacement",
        }
    }

    pub(crate) fn label(label: &str) -> Option<Self> {
        let label = label.trim_matches(|ch| matches!(ch, '\t' | '\n' | '\u{c}' | '\r' | ' '));
        // All 228 pinned labels are ASCII and at most 19 bytes. A bounded
        // stack copy avoids allocating from a caller-provided label. The
        // independent inventory test checks every pinned spelling.
        let mut normalized = [0; 64];
        if label.len() > normalized.len() || !label.is_ascii() { return None; }
        for (out, byte) in normalized.iter_mut().zip(label.bytes()) {
            *out = byte.to_ascii_lowercase();
        }
        let label = std::str::from_utf8(&normalized[..label.len()]).unwrap();
        match label {
            "unicode-1-1-utf-8" | "unicode11utf8" | "unicode20utf8" | "utf-8" | "utf8"
            | "x-unicode20utf8" => Some(Self::UTF8),
            "unicodefffe" | "utf-16be" => Some(Self::UTF16BE),
            "csunicode" | "iso-10646-ucs-2" | "ucs-2" | "unicode" | "unicodefeff" | "utf-16"
            | "utf-16le" => Some(Self::UTF16LE),
            "x-user-defined" => Some(Self::USER_DEFINED),
            "csiso2022kr" | "hz-gb-2312" | "iso-2022-cn" | "iso-2022-cn-ext"
            | "iso-2022-kr" | "replacement" => Some(Self::REPLACEMENT),
            _ => single_byte::SingleByte::from_label(label)
                .map(|encoding| Self(Kind::SingleByte(encoding)))
                .or_else(|| multibyte::Encoding::from_label(label)
                    .map(|encoding| Self(Kind::Multibyte(encoding)))),
        }
    }

    pub(crate) fn bom(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xef, 0xbb, 0xbf]) { Some(Self::UTF8) }
        else if bytes.starts_with(&[0xfe, 0xff]) { Some(Self::UTF16BE) }
        else if bytes.starts_with(&[0xff, 0xfe]) { Some(Self::UTF16LE) }
        else { None }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Decoder {
    pub(crate) encoding: Encoding,
    pub(crate) fatal: bool,
    pub(crate) ignore_bom: bool,
    bom_seen: bool,
    pub(crate) streaming: bool,
    pub(crate) pending: [u8; 4],
    pub(crate) pending_len: usize,
    iso2022jp: iso2022jp::Decoder,
    pub(crate) replacement_error_returned: bool,
}

/// Malformed input and an impossible retained-prefix overflow stay distinct
/// from output admission/cancellation errors owned by the caller.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum DecodeError<E> {
    Invalid,
    State,
    Sink(E),
}

impl Decoder {
    pub(crate) fn new(encoding: Encoding, fatal: bool, ignore_bom: bool) -> Self {
        Self {
            encoding, fatal, ignore_bom, bom_seen: false, streaming: false,
            pending: [0; 4], pending_len: 0, iso2022jp: iso2022jp::Decoder::new(),
            replacement_error_returned: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new(self.encoding, self.fatal, self.ignore_bom);
    }

    /// Decode one prepared input slice. A streaming caller prepends the fixed
    /// pending prefix before its next slice and clears pending_len before this
    /// call. This preserves the VM's existing pre-copy/error-reset boundary.
    /// Each sink call contains one scalar or an inseparable Big5 scalar pair.
    pub(crate) fn decode_to<E>(
        &mut self, bytes: &[u8], stream: bool,
        sink: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), DecodeError<E>> {
        self.decode_checked(bytes, stream, sink, || Ok(()))
    }

    fn decode_checked<E>(
        &mut self, bytes: &[u8], stream: bool,
        mut sink: impl FnMut(&str) -> Result<(), E>,
        mut check: impl FnMut() -> Result<(), E>,
    ) -> Result<(), DecodeError<E>> {
        let decoder = self;
        let mut at = 0;
        let mut next_check = 0;
        while at < bytes.len() {
            if at >= next_check {
                check().map_err(DecodeError::Sink)?;
                next_check = at.saturating_add(4096);
            }
            let scalar = match decoder.encoding.0 {
                Kind::Utf8 => utf8_scalar(&bytes[at..], stream),
                Kind::Utf16Le | Kind::Utf16Be => {
                    utf16_scalar(&bytes[at..], decoder.encoding.0 == Kind::Utf16Le, stream)
                }
                Kind::SingleByte(encoding) => match encoding.decode(bytes[at]) {
                    Some(character) => Scalar::Character(character, 1),
                    None => Scalar::Invalid(1),
                },
                Kind::Multibyte(multibyte::Encoding::Iso2022Jp) => {
                    match decoder.iso2022jp.step(&bytes[at..], stream) {
                        iso2022jp::Step::Character(character, consumed) => Scalar::Character(character, consumed),
                        iso2022jp::Step::Continue(consumed) => Scalar::Continue(consumed),
                        iso2022jp::Step::Invalid(consumed) => Scalar::Invalid(consumed),
                        iso2022jp::Step::Incomplete => Scalar::Incomplete,
                    }
                }
                Kind::Multibyte(encoding) => match encoding.step(&bytes[at..], stream) {
                    multibyte::Step::Character(character, consumed) => {
                        Scalar::Character(character, consumed)
                    }
                    multibyte::Step::Pair(first, second, consumed) => Scalar::Pair(first, second, consumed),
                    multibyte::Step::Invalid(consumed) => Scalar::Invalid(consumed),
                    multibyte::Step::Incomplete => Scalar::Incomplete,
                },
                Kind::UserDefined => {
                    let byte = bytes[at];
                    let value = if byte < 0x80 {
                        u32::from(byte)
                    } else {
                        0xf780 + u32::from(byte - 0x80)
                    };
                    Scalar::Character(char::from_u32(value).unwrap(), 1)
                }
                Kind::Replacement => {
                    // Encoding's replacement decoder emits exactly one error
                    // for a nonempty stream, then finishes without exposing
                    // any following bytes. The flag belongs to this decoder,
                    // including across empty or subsequent streaming chunks.
                    if decoder.replacement_error_returned {
                        Scalar::Continue(bytes.len() - at)
                    } else {
                        decoder.replacement_error_returned = true;
                        Scalar::Invalid(1)
                    }
                }
            };
            let (character, second, consumed) = match scalar {
                Scalar::Incomplete => {
                    let remaining = &bytes[at..];
                    if remaining.len() > decoder.pending.len() {
                        return Err(DecodeError::State);
                    }
                    decoder.pending[..remaining.len()].copy_from_slice(remaining);
                    decoder.pending_len = remaining.len();
                    break;
                }
                Scalar::Continue(consumed) => {
                    at += consumed;
                    continue;
                }
                Scalar::Character(character, consumed) => (character, None, consumed),
                Scalar::Pair(first, second, consumed) => (first, Some(second), consumed),
                Scalar::Invalid(consumed) => {
                    if decoder.fatal {
                        decoder.pending_len = 0;
                        decoder.streaming = false;
                        return Err(DecodeError::Invalid);
                    }
                    ('\u{fffd}', None, consumed)
                }
            };
            at += consumed;
            if matches!(
                decoder.encoding.0,
                Kind::Utf8 | Kind::Utf16Le | Kind::Utf16Be
            ) && !decoder.ignore_bom
                && !decoder.bom_seen
            {
                decoder.bom_seen = true;
                if character == '\u{feff}' {
                    continue;
                }
            }
            // A Big5 pointer can emit two scalars. Admit them together into
            // the unpublished result, including both UTF-8/output costs.
            let mut encoded = [0; 8];
            let mut length = character.encode_utf8(&mut encoded).len();
            if let Some(second) = second {
                length += second.encode_utf8(&mut encoded[length..]).len();
            }
            let text = std::str::from_utf8(&encoded[..length]).unwrap();
            sink(text).map_err(DecodeError::Sink)?;
        }
        Ok(())
    }
}

enum Scalar {
    Character(char, usize),
    Pair(char, char, usize),
    Continue(usize),
    Invalid(usize),
    Incomplete,
}
fn utf8_scalar(bytes: &[u8], stream: bool) -> Scalar {
    let first = bytes[0];
    if first < 128 {
        return Scalar::Character(char::from(first), 1);
    }
    let (count, mut value) = match first {
        0xc2..=0xdf => (2, u32::from(first & 31)),
        0xe0..=0xef => (3, u32::from(first & 15)),
        0xf0..=0xf4 => (4, u32::from(first & 7)),
        _ => return Scalar::Invalid(1),
    };
    for index in 1..count {
        let Some(&byte) = bytes.get(index) else {
            return if stream {
                Scalar::Incomplete
            } else {
                Scalar::Invalid(bytes.len())
            };
        };
        let (lower, upper) = if index == 1 {
            match first {
                0xe0 => (0xa0, 0xbf),
                0xed => (0x80, 0x9f),
                0xf0 => (0x90, 0xbf),
                0xf4 => (0x80, 0x8f),
                _ => (0x80, 0xbf),
            }
        } else {
            (0x80, 0xbf)
        };
        if byte < lower || byte > upper {
            return Scalar::Invalid(index);
        }
        value = (value << 6) | u32::from(byte & 63);
    }
    Scalar::Character(char::from_u32(value).unwrap(), count)
}
fn utf16_scalar(bytes: &[u8], little: bool, stream: bool) -> Scalar {
    if bytes.len() < 2 {
        return if stream {
            Scalar::Incomplete
        } else {
            Scalar::Invalid(1)
        };
    }
    let unit = |at| {
        if little {
            u16::from_le_bytes([bytes[at], bytes[at + 1]])
        } else {
            u16::from_be_bytes([bytes[at], bytes[at + 1]])
        }
    };
    let first = unit(0);
    if (0xd800..=0xdbff).contains(&first) {
        if bytes.len() < 4 {
            return if stream {
                Scalar::Incomplete
            } else {
                Scalar::Invalid(bytes.len())
            };
        }
        let second = unit(2);
        if !(0xdc00..=0xdfff).contains(&second) {
            return Scalar::Invalid(2);
        }
        let scalar = 0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
        Scalar::Character(char::from_u32(scalar).unwrap(), 4)
    } else if (0xdc00..=0xdfff).contains(&first) {
        Scalar::Invalid(2)
    } else {
        Scalar::Character(char::from_u32(u32::from(first)).unwrap(), 2)
    }
}


/// Decode a complete resource using Encoding's BOM override and replacement
/// error policy. The first pass admits the exact UTF-8 length before allocation;
/// the second pass fills only that reserved result. Cancellation is checked in
/// both passes, including state transitions that do not emit a character.
pub(crate) fn decode(
    bytes: &[u8], encoding: Encoding, limit: usize,
    check_cancel: &dyn Fn() -> Result<(), String>,
) -> Result<String, String> {
    check_cancel()?;
    let encoding = Encoding::bom(bytes).unwrap_or(encoding);
    let mut length = 0usize;
    Decoder::new(encoding, false, false).decode_checked(bytes, false, |text| {
        length = length.checked_add(text.len()).filter(|&length| length <= limit)
            .ok_or_else(|| format!("Decoded text exceeds the remaining {limit} byte allowance."))?;
        Ok(())
    }, check_cancel).map_err(resource_error)?;
    check_cancel()?;
    let mut output = String::new();
    output.try_reserve_exact(length).map_err(|_| "Cannot reserve decoded resource text.".to_owned())?;
    Decoder::new(encoding, false, false).decode_checked(bytes, false, |text| {
        // The immutable source and fresh decoder make both passes identical.
        // Keep an explicit check before push_str so allocation cannot grow if
        // that invariant is ever broken by a later codec change.
        if output.len().checked_add(text.len()).is_none_or(|size| size > length) {
            return Err("Decoded resource size changed between passes.".to_owned());
        }
        output.push_str(text);
        Ok(())
    }, check_cancel).map_err(resource_error)?;
    check_cancel()?;
    Ok(output)
}

fn resource_error(error: DecodeError<String>) -> String {
    match error {
        DecodeError::Invalid => "The resource contains invalid encoded input.".into(),
        DecodeError::State => "Text decoder fragment exceeded its fixed state.".into(),
        DecodeError::Sink(error) => error,
    }
}
