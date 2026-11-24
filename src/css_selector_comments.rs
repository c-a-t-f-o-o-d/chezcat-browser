//! Comment boundaries shared by selectors and stylesheet structure.

pub(super) fn end(bytes: &[u8], position: usize) -> Option<usize> {
    if !bytes.get(position..)?.starts_with(b"/*") {
        return None;
    }
    let mut at = position + 2;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'*' && bytes.get(at + 1) == Some(&b'/') {
            return Some(at + 2);
        }
        at += 1;
    }
    Some(bytes.len())
}

pub(super) fn skip(bytes: &[u8], position: &mut usize) -> bool {
    let start = *position;
    while let Some(end) = end(bytes, *position) {
        *position = end;
    }
    *position != start
}

// Preserve the stylesheet parser's existing spelling allowance without copying
// comment payloads or using their virtual space as a selector combinator. Raw
// source bytes remain subject to the caller's stylesheet/query admission.
pub(super) fn within_limit(source: &str, maximum: usize, trim_outer: bool) -> bool {
    let bytes = source.as_bytes();
    let mut position = 0;
    let mut length = 0;
    let mut pending_space = 0;
    let mut quote = 0;
    let mut escaped = false;
    while position < bytes.len() {
        if quote == 0 {
            if let Some(end) = end(bytes, position) {
                position = end;
                if trim_outer {
                    if length != 0 { pending_space += 1; }
                } else {
                    length += 1;
                }
                if length > maximum { return false; }
                continue;
            }
        }
        let byte = bytes[position];
        position += 1;
        if trim_outer && quote == 0 && byte.is_ascii_whitespace() {
            if length != 0 { pending_space += 1; }
        } else {
            length += pending_space + 1;
            pending_space = 0;
        }
        if quote != 0 {
            if escaped { escaped = false; }
            else if byte == b'\\' { escaped = true; }
            else if byte == quote { quote = 0; }
        } else if matches!(byte, b'\'' | b'"') {
            quote = byte;
        }
        if length > maximum { return false; }
    }
    true
}
