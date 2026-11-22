//! Original loaded-document charset selection and bounded HTML byte prescan.
//! HTML source: 0e292f402d0c3dd17fe00c1bb2ef3a3c94cd8bce, encoding sniffing,
//! prescan/get-an-attribute/get-an-XML-encoding and meta charset extraction.
//! The committed document retains its initial decoder identity. Parser restarts
//! and CSS/script selection have separate entry paths.
use crate::text_codec::{self, Encoding};

// Reuse the original native MIME algorithm; HTML meta content has a different
// extraction algorithm below and must not be parsed as an HTTP MIME header.
#[path = "script_xhr_mime.rs"]
mod mime;

#[cfg(test)]
#[path = "document_encoding_tests.rs"]
mod tests;

pub(super) fn decode(
    bytes: &[u8],
    content_type: &str,
    html: bool,
    output_limit: usize,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<(String, Encoding), String> {
    check()?;
    let encoding = select(bytes, content_type, html)?;
    let text = text_codec::decode(bytes, encoding, output_limit, check)?;
    Ok((text, encoding))
}

fn select(bytes: &[u8], content_type: &str, html: bool) -> Result<Encoding, String> {
    if let Some(encoding) = Encoding::bom(bytes) {
        return Ok(encoding);
    }
    if let Some(encoding) = mime::parse(content_type)?
        .as_ref()
        .and_then(|mime| mime.charset())
        .and_then(Encoding::label)
    {
        return Ok(encoding);
    }
    if html {
        if let Some(encoding) = prescan(bytes) {
            return Ok(encoding);
        }
    }
    // Preserve the loader's existing deterministic fallback. It is a fallback
    // policy, not an assertion that the document supplied an encoding.
    Ok(if std::str::from_utf8(bytes).is_ok() {
        Encoding::UTF8
    } else {
        Encoding::WINDOWS1252
    })
}

fn space(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

fn label(bytes: &[u8]) -> Option<Encoding> {
    // All Encoding labels are ASCII. Isomorphic decoding of non-ASCII bytes
    // cannot produce a supported label; no lossy conversion belongs here.
    bytes.is_ascii().then(|| std::str::from_utf8(bytes).unwrap())
        .and_then(Encoding::label)
}

fn utf16_to_utf8(encoding: Encoding) -> Encoding {
    if matches!(encoding, Encoding::UTF16LE | Encoding::UTF16BE) {
        Encoding::UTF8
    } else {
        encoding
    }
}

fn prescan(input: &[u8]) -> Option<Encoding> {
    // Every pointer, including attribute and XML-declaration pointers, shares
    // this end condition. A declaration cut off here is not a complete one.
    let bytes = &input[..input.len().min(1024)];
    scan(bytes).or_else(|| xml_encoding(bytes))
}

fn scan(bytes: &[u8]) -> Option<Encoding> {
    if bytes.starts_with(b"<\0?\0x\0") {
        return Some(Encoding::UTF16LE);
    }
    if bytes.starts_with(b"\0<\0?\0x") {
        return Some(Encoding::UTF16BE);
    }
    let mut at = 0;
    while at < bytes.len() {
        let rest = &bytes[at..];
        if rest.starts_with(b"<!--") {
            // The opening '--' may also be the first two bytes of the end.
            at += 2 + rest[2..].windows(3).position(|part| part == b"-->")? + 2;
        } else if rest.len() >= 6 && rest[..5].eq_ignore_ascii_case(b"<meta")
            && (space(rest[5]) || rest[5] == b'/')
        {
            at += 5;
            let mut seen = 0u8;
            let mut got_pragma = false;
            let mut need_pragma = None;
            // None is the algorithm's null; Some(None) is a failed label.
            let mut charset: Option<Option<Encoding>> = None;
            while let Some((name, value)) = attribute(bytes, &mut at).ok()? {
                let bit = if name.eq_ignore_ascii_case(b"http-equiv") { 1 }
                    else if name.eq_ignore_ascii_case(b"content") { 2 }
                    else if name.eq_ignore_ascii_case(b"charset") { 4 }
                    else { continue };
                if seen & bit != 0 {
                    continue;
                }
                seen |= bit;
                match bit {
                    1 => got_pragma = value.eq_ignore_ascii_case(b"content-type"),
                    2 => {
                        if charset.is_none() {
                            if let Some(encoding) = meta_content_encoding(value) {
                                charset = Some(Some(encoding));
                                need_pragma = Some(true);
                            }
                        }
                    }
                    4 => {
                        charset = Some(label(value));
                        need_pragma = Some(false);
                    }
                    _ => unreachable!(),
                }
            }
            if need_pragma.is_some_and(|need| !need || got_pragma) {
                if let Some(Some(encoding)) = charset {
                    let encoding = utf16_to_utf8(encoding);
                    return Some(if encoding == Encoding::USER_DEFINED {
                        Encoding::WINDOWS1252
                    } else {
                        encoding
                    });
                }
            }
        } else if rest.first() == Some(&b'<')
            && rest.get(if rest.get(1) == Some(&b'/') { 2 } else { 1 })
                .is_some_and(u8::is_ascii_alphabetic)
        {
            while !space(*bytes.get(at)?) && bytes[at] != b'>' {
                at += 1;
            }
            while attribute(bytes, &mut at).ok()?.is_some() {}
        } else if rest.starts_with(b"<!") || rest.starts_with(b"</") || rest.starts_with(b"<?") {
            at += rest.iter().position(|&byte| byte == b'>')?;
        }
        at += 1;
    }
    None
}

type Attribute<'a> = (&'a [u8], &'a [u8]);

// Err denotes the shared prescan end condition, distinct from a tag-ending >.
fn attribute<'a>(bytes: &'a [u8], at: &mut usize) -> Result<Option<Attribute<'a>>, ()> {
    while space(*bytes.get(*at).ok_or(())?) || bytes[*at] == b'/' {
        *at += 1;
    }
    if bytes[*at] == b'>' {
        return Ok(None);
    }
    let start = *at;
    let end;
    loop {
        let byte = *bytes.get(*at).ok_or(())?;
        if byte == b'=' && *at > start {
            end = *at;
            *at += 1;
            break;
        }
        if space(byte) {
            end = *at;
            while space(*bytes.get(*at).ok_or(())?) {
                *at += 1;
            }
            if bytes[*at] != b'=' {
                return Ok(Some((&bytes[start..end], &[])));
            }
            *at += 1;
            break;
        }
        if matches!(byte, b'/' | b'>') {
            return Ok(Some((&bytes[start..*at], &[])));
        }
        *at += 1;
    }
    while space(*bytes.get(*at).ok_or(())?) {
        *at += 1;
    }
    let quote = bytes[*at];
    if matches!(quote, b'\'' | b'"') {
        *at += 1;
        let value = *at;
        while *bytes.get(*at).ok_or(())? != quote {
            *at += 1;
        }
        let result = (&bytes[start..end], &bytes[value..*at]);
        *at += 1;
        return Ok(Some(result));
    }
    let value = *at;
    while !space(*bytes.get(*at).ok_or(())?) && bytes[*at] != b'>' {
        *at += 1;
    }
    Ok(Some((&bytes[start..end], &bytes[value..*at])))
}

fn meta_content_encoding(bytes: &[u8]) -> Option<Encoding> {
    let mut at = 0;
    loop {
        at += bytes.get(at..)?.windows(7)
            .position(|part| part.eq_ignore_ascii_case(b"charset"))? + 7;
        while bytes.get(at).is_some_and(|&byte| space(byte)) {
            at += 1;
        }
        if *bytes.get(at)? != b'=' {
            continue;
        }
        at += 1;
        while bytes.get(at).is_some_and(|&byte| space(byte)) {
            at += 1;
        }
        let first = *bytes.get(at)?;
        if matches!(first, b'\'' | b'"') {
            at += 1;
            let length = bytes[at..].iter().position(|&byte| byte == first)?;
            return label(&bytes[at..at + length]);
        }
        let length = bytes[at..].iter().position(|&byte| space(byte) || byte == b';')
            .unwrap_or(bytes.len() - at);
        return label(&bytes[at..at + length]);
    }
}

fn xml_encoding(bytes: &[u8]) -> Option<Encoding> {
    if !bytes.starts_with(b"<?xml") {
        return None;
    }
    let end = bytes.iter().position(|&byte| byte == b'>')?;
    let mut at = bytes[..end].windows(8).position(|part| part == b"encoding")? + 8;
    while bytes.get(at).is_some_and(|&byte| byte <= 0x20) {
        at += 1;
    }
    if *bytes.get(at)? != b'=' {
        return None;
    }
    at += 1;
    while bytes.get(at).is_some_and(|&byte| byte <= 0x20) {
        at += 1;
    }
    let quote = *bytes.get(at)?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    at += 1;
    let length = bytes[at..].iter().position(|&byte| byte == quote)?;
    let value = &bytes[at..at + length];
    if value.iter().any(|&byte| byte <= 0x20) {
        return None;
    }
    label(value).map(utf16_to_utf8)
}
