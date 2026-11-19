//! ECMAScript IdentifierName. A keyword production also requires an unescaped spelling.
use super::{Fault, TokenKind};

#[path = "unicode_identifier_ranges.rs"]
mod ranges;

fn contains(ranges: &[(u32, u32)], ch: char) -> bool {
    let code = ch as u32;
    let after = ranges.partition_point(|&(lo, _)| lo <= code);
    after != 0 && code <= ranges[after - 1].1
}

pub(super) fn start(ch: char) -> bool {
    if ch.is_ascii() {
        ch.is_ascii_alphabetic() || matches!(ch, '$' | '_')
    } else {
        contains(ranges::START, ch)
    }
}

pub(super) fn part(ch: char) -> bool {
    if ch.is_ascii() {
        ch.is_ascii_alphanumeric() || matches!(ch, '$' | '_')
    } else {
        matches!(ch, '\u{200c}' | '\u{200d}')
            || contains(ranges::START, ch)
            || contains(ranges::CONTINUE_ONLY, ch)
    }
}

fn escape(source: &str, at: &mut usize) -> Result<char, Fault> {
    let slash = *at;
    let bytes = source.as_bytes();
    let invalid = || Fault::exception("SyntaxError", "Invalid Unicode identifier escape.").at(source, slash);
    if bytes.get(slash + 1) != Some(&b'u') {
        return Err(invalid());
    }
    let mut pos = slash + 2;
    let braced = bytes.get(pos) == Some(&b'{');
    if braced {
        pos += 1;
    }
    let first = pos;
    let mut value = 0;
    while braced || pos - first < 4 {
        let Some(digit) = bytes.get(pos).and_then(|byte| char::from(*byte).to_digit(16)) else {
            break;
        };
        value = value * 16 + digit;
        if value > 0x10ffff {
            return Err(invalid());
        }
        pos += 1;
    }
    if braced {
        if pos == first || bytes.get(pos) != Some(&b'}') {
            return Err(invalid());
        }
        pos += 1;
    } else if pos - first != 4 {
        return Err(invalid());
    }
    let ch = char::from_u32(value).ok_or_else(invalid)?;
    *at = pos;
    Ok(ch)
}

pub(super) fn read(source: &str, at: &mut usize) -> Result<TokenKind, Fault> {
    let first = *at;
    let mut literal_start = first;
    let mut decoded: Option<String> = None;
    while let Some(ch) = source[*at..].chars().next() {
        if ch == '\\' {
            let slash = *at;
            let value = escape(source, at)?;
            let allowed = if slash == first { start(value) } else { part(value) };
            if !allowed {
                return Err(Fault::exception("SyntaxError", "Escaped character is invalid here in an identifier.").at(source, slash));
            }
            let name = decoded.get_or_insert_with(String::new);
            name.push_str(&source[literal_start..slash]);
            name.push(value);
            literal_start = *at;
        } else if part(ch) {
            // The caller checks the unescaped first character.
            *at += ch.len_utf8();
        } else {
            break;
        }
    }
    Ok(if let Some(mut name) = decoded {
        name.push_str(&source[literal_start..*at]);
        TokenKind::EscapedWord(name)
    } else {
        TokenKind::Word(source[first..*at].to_owned())
    })
}
