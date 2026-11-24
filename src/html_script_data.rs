//! HTML script-data boundary scanning over preprocessed UTF-8 input.
//!
//! The state machine identifies the appropriate end tag without interpreting
//! script syntax. All preceding source characters remain literal; the caller
//! replaces NULLs when copying the retained text. Only equality with the fixed
//! word "script" is needed from the specification's temporary name buffer.
//! https://html.spec.whatwg.org/multipage/parsing.html#script-data-state

#[derive(Clone, Copy)]
enum State {
    Data,
    LessThan,
    EscapeStart,
    EscapeStartDash,
    Escaped,
    EscapedDash,
    EscapedDashDash,
    EscapedLessThan,
    DoubleEscapeStart,
    DoubleEscaped,
    DoubleEscapedDash,
    DoubleEscapedDashDash,
    DoubleEscapedLessThan,
    DoubleEscapeEnd,
}

fn name_delimiter(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | 0x0c | b'\r' | b' ' | b'/' | b'>')
}

fn appropriate_end(bytes: &[u8], slash: usize) -> bool {
    let remaining = &bytes[slash + 1..];
    remaining.get(..6).is_some_and(|name| name.eq_ignore_ascii_case(b"script"))
        && remaining.get(6).is_some_and(|&byte| name_delimiter(byte))
}

fn advance_name(prefix: Option<usize>, byte: u8) -> Option<usize> {
    prefix.filter(|&index| b"script".get(index) == Some(&byte.to_ascii_lowercase()))
        .map(|index| index + 1)
}

/// Return the opening '<' of the first appropriate script end tag, or None
/// at EOF. Candidate names are fixed-size lookahead; arbitrary other names
/// remain literal and do not allocate a temporary buffer.
pub(super) fn end(
    input: &str,
    start: usize,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<Option<usize>, String> {
    let bytes = input.as_bytes();
    let mut state = State::Data;
    let mut position = start;
    let mut opening = start;
    let mut next_check = start;
    let mut prefix = Some(0usize);
    while position < bytes.len() {
        if position >= next_check {
            if let Some(check) = check { check()?; }
            next_check = position.saturating_add(super::RAW_TEXT_CHECK_BYTES);
        }
        let byte = bytes[position];
        match state {
            State::Data => {
                if byte == b'<' { opening = position; state = State::LessThan; }
            }
            State::LessThan => match byte {
                b'/' => {
                    if appropriate_end(bytes, position) { return Ok(Some(opening)); }
                    // Failed end-tag names contain only ASCII letters until
                    // their fallback character; scanning them in Data is
                    // equivalent and preserves their original source spelling.
                    state = State::Data;
                }
                b'!' => state = State::EscapeStart,
                _ => { state = State::Data; continue; }
            },
            State::EscapeStart => {
                if byte == b'-' { state = State::EscapeStartDash; }
                else { state = State::Data; continue; }
            }
            State::EscapeStartDash => {
                if byte == b'-' { state = State::EscapedDashDash; }
                else { state = State::Data; continue; }
            }
            State::Escaped => match byte {
                b'-' => state = State::EscapedDash,
                b'<' => { opening = position; state = State::EscapedLessThan; }
                _ => {}
            },
            State::EscapedDash => match byte {
                b'-' => state = State::EscapedDashDash,
                b'<' => { opening = position; state = State::EscapedLessThan; }
                _ => state = State::Escaped,
            },
            State::EscapedDashDash => match byte {
                b'-' => {}
                b'<' => { opening = position; state = State::EscapedLessThan; }
                b'>' => state = State::Data,
                _ => state = State::Escaped,
            },
            State::EscapedLessThan => match byte {
                b'/' => {
                    if appropriate_end(bytes, position) { return Ok(Some(opening)); }
                    state = State::Escaped;
                }
                byte if byte.is_ascii_alphabetic() => {
                    prefix = Some(0);
                    state = State::DoubleEscapeStart;
                    continue;
                }
                _ => { state = State::Escaped; continue; }
            },
            State::DoubleEscapeStart => {
                if byte.is_ascii_alphabetic() { prefix = advance_name(prefix, byte); }
                else if name_delimiter(byte) {
                    state = if prefix == Some(6) { State::DoubleEscaped } else { State::Escaped };
                } else { state = State::Escaped; continue; }
            }
            State::DoubleEscaped => match byte {
                b'-' => state = State::DoubleEscapedDash,
                b'<' => state = State::DoubleEscapedLessThan,
                _ => {}
            },
            State::DoubleEscapedDash => match byte {
                b'-' => state = State::DoubleEscapedDashDash,
                b'<' => state = State::DoubleEscapedLessThan,
                _ => state = State::DoubleEscaped,
            },
            State::DoubleEscapedDashDash => match byte {
                b'-' => {}
                b'<' => state = State::DoubleEscapedLessThan,
                b'>' => state = State::Data,
                _ => state = State::DoubleEscaped,
            },
            State::DoubleEscapedLessThan => {
                if byte == b'/' { prefix = Some(0); state = State::DoubleEscapeEnd; }
                else { state = State::DoubleEscaped; continue; }
            }
            State::DoubleEscapeEnd => {
                if byte.is_ascii_alphabetic() { prefix = advance_name(prefix, byte); }
                else if name_delimiter(byte) {
                    state = if prefix == Some(6) { State::Escaped } else { State::DoubleEscaped };
                } else { state = State::DoubleEscaped; continue; }
            }
        }
        position += 1;
    }
    if let Some(check) = check { check()?; }
    Ok(None)
}
