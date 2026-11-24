//! Character data retained after the selected HTML insertion rules run.

pub(super) fn without_nulls(
    text: String,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<String, String> {
    let mut retained = text.len();
    for chunk in text.as_bytes().chunks(super::RAW_TEXT_CHECK_BYTES) {
        check()?;
        retained -= chunk.iter().filter(|&&byte| byte == 0).count();
    }
    if retained == text.len() {
        return Ok(text);
    }
    let mut output = String::new();
    output
        .try_reserve_exact(retained)
        .map_err(|_| "Cannot reserve HTML character data.")?;
    let mut remaining = text.as_str();
    while !remaining.is_empty() {
        check()?;
        let end = remaining.floor_char_boundary(remaining.len().min(super::RAW_TEXT_CHECK_BYTES));
        let (chunk, rest) = remaining.split_at(end);
        for part in chunk.split('\0') {
            output.push_str(part);
        }
        remaining = rest;
    }
    Ok(output)
}

#[cfg(test)]
#[path = "html_data_character_scan_tests.rs"]
mod tests;
