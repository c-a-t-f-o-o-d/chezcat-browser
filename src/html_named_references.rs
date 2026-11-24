//! Original HTML character-reference decoding over pinned normative name data.
//! The caller chooses Data/RCDATA or attribute-value context. Replacements are
//! emitted directly and cannot reenter markup or character-reference parsing.

#[path = "html_named_references_data.rs"]
mod data;

const CANCEL_INTERVAL: usize = 4096;
type Cancel<'a> = Option<&'a dyn Fn() -> Result<(), String>>;

enum Replacement {
    Named(&'static str),
    Numeric(char),
}

pub(super) fn decode(input: &str, attribute: bool, check: Cancel<'_>) -> Result<String, String> {
    let mut length = 0usize;
    walk(input, attribute, check, |part| {
        length = length.checked_add(part.len()).ok_or("Decoded HTML reference length overflow.")?;
        Ok(())
    })?;
    let mut output = String::new();
    output.try_reserve_exact(length).map_err(|_| "Cannot reserve decoded HTML reference text.")?;
    walk(input, attribute, check, |part| {
        output.push_str(part);
        Ok(())
    })?;
    debug_assert_eq!(output.len(), length);
    Ok(output)
}

fn poll(check: Cancel<'_>) -> Result<(), String> {
    if let Some(check) = check { check()?; }
    Ok(())
}

fn walk(
    input: &str, attribute: bool, check: Cancel<'_>,
    mut emit: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    poll(check)?;
    let bytes = input.as_bytes();
    let mut position = 0usize;
    let mut literal = 0usize;
    let mut next_check = CANCEL_INTERVAL;
    while position < bytes.len() {
        if position >= next_check {
            poll(check)?;
            next_check = position.saturating_add(CANCEL_INTERVAL);
        }
        if bytes[position] == b'&' {
            if let Some((replacement, consumed)) = reference(&input[position + 1..], attribute, check)? {
                emit(&input[literal..position])?;
                match replacement {
                    Replacement::Named(value) => emit(value)?,
                    Replacement::Numeric(value) => {
                        let mut encoded = [0; 4];
                        emit(value.encode_utf8(&mut encoded))?;
                    }
                }
                position += 1 + consumed;
                literal = position;
                continue;
            }
        }
        position += 1;
    }
    emit(&input[literal..])?;
    poll(check)
}

fn reference(input: &str, attribute: bool, check: Cancel<'_>) -> Result<Option<(Replacement, usize)>, String> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'#') {
        return Ok(named(input, attribute).map(|(value, consumed)| (Replacement::Named(value), consumed)));
    }
    let mut position = 1;
    let radix = if matches!(bytes.get(position), Some(b'x' | b'X')) {
        position += 1;
        16
    } else { 10 };
    let first_digit = position;
    let mut number = 0u32;
    while let Some(digit) = bytes.get(position).and_then(|byte| char::from(*byte).to_digit(radix)) {
        if position % CANCEL_INTERVAL == 0 { poll(check)?; }
        number = number.saturating_mul(radix).saturating_add(digit);
        position += 1;
    }
    if position == first_digit { return Ok(None); }
    if bytes.get(position) == Some(&b';') { position += 1; }
    Ok(Some((Replacement::Numeric(numeric_character(number)), position)))
}

fn named(input: &str, attribute: bool) -> Option<(&'static str, usize)> {
    let bytes = input.as_bytes();
    let mut matched = None;
    for (index, &byte) in bytes.iter().take(data::MAX_NAME_BYTES).enumerate() {
        if !byte.is_ascii_alphanumeric() && byte != b';' { break; }
        let end = index + 1;
        if let Ok(entry) = data::ENTITIES.binary_search_by(|entry| entry.0.cmp(&input[..end])) {
            matched = Some((data::ENTITIES[entry].1, end, byte == b';'));
        }
        if byte == b';' { break; }
    }
    let (value, consumed, semicolon) = matched?;
    if attribute && !semicolon
        && bytes.get(consumed).is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'=') {
        return None;
    }
    Some((value, consumed))
}

fn numeric_character(number: u32) -> char {
    // HTML numeric-reference corrections do not replace undefined C1 controls.
    let corrected = match number {
        0 => 0xfffd,
        0x80 => 0x20ac,
        0x82 => 0x201a,
        0x83 => 0x0192,
        0x84 => 0x201e,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02c6,
        0x89 => 0x2030,
        0x8a => 0x0160,
        0x8b => 0x2039,
        0x8c => 0x0152,
        0x8e => 0x017d,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201c,
        0x94 => 0x201d,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02dc,
        0x99 => 0x2122,
        0x9a => 0x0161,
        0x9b => 0x203a,
        0x9c => 0x0153,
        0x9e => 0x017e,
        0x9f => 0x0178,
        _ => number,
    };
    char::from_u32(corrected).unwrap_or('\u{fffd}')
}
