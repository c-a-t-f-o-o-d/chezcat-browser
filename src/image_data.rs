//! Bounded data: image resources. This module decodes bytes, never documents or
//! executable code. Reference: WHATWG Fetch, data: URL processor; Infra base64.
const MAX_URL_BYTES: usize = 8 * 1024 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

pub struct Prepared {
    url: String,
    start: usize,
    pub mime: String,
    base64: bool,
}

pub fn is_data(value: &str) -> bool {
    let mut bytes = value
        .trim_matches(|c| c <= '\u{20}')
        .bytes()
        .filter(|b| !matches!(b, b'\t' | b'\n' | b'\r'));
    b"data:".iter().all(|expected| {
        bytes
            .next()
            .is_some_and(|b| b.eq_ignore_ascii_case(expected))
    })
}

pub fn prepare(value: &str) -> Result<Prepared, String> {
    if value.len() > MAX_URL_BYTES {
        return Err("Embedded image URL exceeds 8 MiB.".into());
    }
    let url: String = value
        .trim_matches(|c| c <= '\u{20}')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    if !is_data(&url) {
        return Err("Expected a data: image URL.".into());
    }
    let url = url.split('#').next().unwrap().to_owned();
    let comma = url
        .find(',')
        .ok_or("Embedded image URL has no comma separator.")?;
    let mut header = url[5..comma].trim_matches(|c: char| c.is_ascii_whitespace());
    if header.len() > 1024 {
        return Err("Embedded image media header exceeds 1 KiB.".into());
    }
    let mut base64 = false;
    if header
        .get(header.len().saturating_sub(6)..)
        .is_some_and(|s| s.eq_ignore_ascii_case("base64"))
    {
        let prefix = header[..header.len() - 6].trim_end_matches(' ');
        if let Some(prefix) = prefix.strip_suffix(';') {
            base64 = true;
            header = prefix;
        }
    }
    let mime = header
        .split(';')
        .next()
        .unwrap()
        .trim()
        .to_ascii_lowercase();
    if !crate::image_selection::supported_mime(&mime) {
        return Err("Embedded resource needs a supported image media type.".into());
    }
    Ok(Prepared {
        url,
        start: comma + 1,
        mime,
        base64,
    })
}

impl Prepared {
    pub fn key(&self) -> [u8; 32] {
        crate::crypto_hash::sha256(self.url.as_bytes())
    }
    pub fn label(&self) -> String {
        format!("data:{} (embedded image; payload omitted)", self.mime)
    }
    pub fn bytes(&self, check: &dyn Fn() -> Result<(), String>) -> Result<Vec<u8>, String> {
        check()?;
        let source = &self.url.as_bytes()[self.start..];
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(source.len().min(MAX_BODY_BYTES))
            .map_err(|_| "Not enough memory for the embedded image.")?;
        let mut at = 0;
        let mut next_check = 0;
        while at < source.len() {
            if at >= next_check {
                check()?;
                next_check = at + 4096;
            }
            if bytes.len() == MAX_BODY_BYTES {
                return Err("Embedded image body exceeds 8 MiB.".into());
            }
            if source[at] == b'%' && at + 2 < source.len() {
                if let (Some(a), Some(b)) = (hex(source[at + 1]), hex(source[at + 2])) {
                    bytes.push(a * 16 + b);
                    at += 3;
                    continue;
                }
            }
            bytes.push(source[at]);
            at += 1;
        }
        if self.base64 {
            bytes = forgiving_base64(&bytes, check)?;
        }
        check()?;
        Ok(bytes)
    }
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
fn ascii_whitespace(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn forgiving_base64(
    data: &[u8],
    check: &dyn Fn() -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let mut compact = Vec::new();
    compact
        .try_reserve_exact(data.len())
        .map_err(|_| "Not enough memory for embedded base64.")?;
    for (i, &b) in data.iter().enumerate() {
        if i & 4095 == 0 {
            check()?;
        }
        if !ascii_whitespace(b) {
            compact.push(b);
        }
    }
    if compact.len().is_multiple_of(4) {
        for _ in 0..2 {
            if compact.last() == Some(&b'=') {
                compact.pop();
            } else {
                break;
            }
        }
    }
    if compact.len() % 4 == 1 {
        return Err("Invalid embedded-image base64 length.".into());
    }
    let mut out = Vec::new();
    out.try_reserve_exact(compact.len() * 3 / 4)
        .map_err(|_| "Not enough memory for embedded-image bytes.")?;
    let mut bits = 0u32;
    let mut accumulator = 0u32;
    for (i, &b) in compact.iter().enumerate() {
        if i & 4095 == 0 {
            check()?;
        }
        let value = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("Invalid embedded-image base64 character or padding.".into()),
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percent_binary_and_forgiving_base64_follow_data_url_boundaries() {
        let decode = |value: &str| prepare(value).unwrap().bytes(&|| Ok(())).unwrap();
        assert_eq!(
            decode("data:image/png,%00%FF+%25%zz#ignored"),
            b"\x00\xff+%%zz"
        );
        assert_eq!(
            decode(" DATA:image/png; charset=UTF-8; BaSe64,AP8rJQ==#ignored "),
            b"\0\xff+%"
        );
        assert_eq!(decode("data:image/png;base64,%54%51%3d%3d"), b"M");
        for body in ["TQ==", "TQ", "TR", "T Q\n=\t=", "TQ%0C=="] {
            assert_eq!(decode(&format!("data:image/png;base64,{body}")), b"M");
        }
        for body in ["A", "TQ=", "=TQ=", "TQ===", "TQ-_"] {
            assert!(prepare(&format!("data:image/png;base64,{body}"))
                .unwrap()
                .bytes(&|| Ok(()))
                .is_err());
        }
        assert_eq!(
            decode("data:image/svg+xml,%3Csvg%3E%23%3C/svg%3E"),
            b"<svg>#</svg>"
        );
        assert!(is_data(" \tDA\nTA:image/png,x"));
        assert_eq!(decode(" \tDA\nTA:image/png,x"), b"x");
        assert!(prepare("data:text/html,%3Cscript%3E").is_err());
        assert!(prepare("data:,plain").is_err());
        assert!(prepare("data:image/png").is_err());
    }
    #[test]
    fn embedded_resources_have_bounded_headers_input_and_cancelable_decoding() {
        assert!(prepare(&format!("data:image/png;{},x", "a".repeat(1100))).is_err());
        assert!(prepare(&"x".repeat(MAX_URL_BYTES + 1)).is_err());
        let asset = prepare(&format!("data:image/svg+xml,{}", "a".repeat(10000))).unwrap();
        let count = std::cell::Cell::new(0);
        assert_eq!(
            asset
                .bytes(&|| {
                    count.set(count.get() + 1);
                    if count.get() > 2 {
                        Err("cancelled".into())
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err(),
            "cancelled"
        );
        assert!(!asset.label().contains("aaaa"));
        assert_eq!(
            prepare("data:image/png,x#one").unwrap().key(),
            prepare("data:image/png,x#two").unwrap().key()
        );
    }
}
