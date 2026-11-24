//! Original comment token boundary reader shared by document and fragment trees.
//! https://html.spec.whatwg.org/multipage/parsing.html#comment-start-state
// Data is literal (not an entity/reference context). The pending delimiter
// characters at EOF are not part of the emitted token, just as at a close.
pub(super) fn token<'a>(
    source: &'a str,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<(&'a str, usize), String> {
    let bytes = source.as_bytes();
    for (block, chunk) in bytes.chunks(super::RAW_TEXT_CHECK_BYTES).enumerate() {
        if let Some(check) = check { check()?; }
        for (offset, &byte) in chunk.iter().enumerate() {
            if byte != b'>' { continue; }
            let at = block * super::RAW_TEXT_CHECK_BYTES + offset;
            if at == 0 || at == 1 && bytes[0] == b'-' {
                return Ok(("", at + 1));
            }
            if at >= 2 && &bytes[at - 2..at] == b"--" {
                return Ok((&source[..at - 2], at + 1));
            }
            if at >= 3 && &bytes[at - 3..at] == b"--!" {
                return Ok((&source[..at - 3], at + 1));
            }
        }
    }
    let pending = if source.ends_with("--!") { 3 }
        else if source.ends_with("--") { 2 }
        else if source.ends_with('-') { 1 } else { 0 };
    Ok((&source[..source.len() - pending], source.len()))
}

pub(super) fn bogus<'a>(
    source: &'a str,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<(&'a str, usize), String> {
    for (block, chunk) in source.as_bytes().chunks(super::RAW_TEXT_CHECK_BYTES).enumerate() {
        if let Some(check) = check { check()?; }
        if let Some(offset) = chunk.iter().position(|&byte| byte == b'>') {
            let end = block * super::RAW_TEXT_CHECK_BYTES + offset;
            return Ok((&source[..end], end + 1));
        }
    }
    Ok((source, source.len()))
}

pub(super) fn copy_data(
    source: &str,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<String, String> {
    let mut length = source.len();
    for chunk in source.as_bytes().chunks(super::RAW_TEXT_CHECK_BYTES) {
        if let Some(check) = check { check()?; }
        let nulls = chunk.iter().filter(|&&byte| byte == 0).count();
        length = nulls.checked_mul(2).and_then(|bytes| length.checked_add(bytes))
            .ok_or("HTML comment data length overflow.")?;
    }
    let mut text = String::new();
    text.try_reserve_exact(length).map_err(|_| "Could not reserve HTML comment data.")?;
    let mut remaining = source;
    while !remaining.is_empty() {
        if let Some(check) = check { check()?; }
        let end = remaining.floor_char_boundary(remaining.len().min(super::RAW_TEXT_CHECK_BYTES));
        let (chunk, rest) = remaining.split_at(end);
        for (index, part) in chunk.split('\0').enumerate() {
            if index != 0 { text.push('\u{fffd}'); }
            text.push_str(part);
        }
        remaining = rest;
    }
    Ok(text)
}
