//! Original allocation-free XML encoding declaration selection for XHR text.
//! This selects a shared codec; it neither parses XML nor loads entities.
use crate::text_codec::Encoding;

fn space(byte: u8) -> bool { matches!(byte, b' ' | b'\t' | b'\r' | b'\n') }

pub(super) fn determine(bytes: &[u8]) -> Option<&'static str> {
    if let Some(encoding) = Encoding::bom(bytes) { return Some(encoding.name()); }
    // XML's initial code-unit signatures can identify UTF-16 without a BOM.
    if bytes.starts_with(&[0, b'<', 0, b'?']) { return Some("utf-16be"); }
    if bytes.starts_with(&[b'<', 0, b'?', 0]) { return Some("utf-16le"); }
    if !bytes.starts_with(b"<?xml") || !bytes.get(5).is_some_and(|byte| space(*byte)) {
        return None;
    }
    // Borrow only the initial declaration, including arbitrarily long admitted
    // XML whitespace. There is no HTML/CSS-style 1024-byte prescan cutoff.
    let end = bytes.windows(2).position(|pair| pair == b"?>")?;
    let declaration = &bytes[..end];
    let mut at = 5;
    let mut version = false;
    while at < declaration.len() {
        let before = at;
        while declaration.get(at).is_some_and(|byte| space(*byte)) { at += 1; }
        if before == at || at == declaration.len() { return None; }
        let start = at;
        while declaration.get(at).is_some_and(u8::is_ascii_alphabetic) { at += 1; }
        let name = &declaration[start..at];
        while declaration.get(at).is_some_and(|byte| space(*byte)) { at += 1; }
        if declaration.get(at) != Some(&b'=') { return None; }
        at += 1;
        while declaration.get(at).is_some_and(|byte| space(*byte)) { at += 1; }
        let quote = *declaration.get(at)?;
        if !matches!(quote, b'\'' | b'"') { return None; }
        at += 1;
        let start = at;
        while declaration.get(at).is_some_and(|byte| *byte != quote) { at += 1; }
        if at == declaration.len() { return None; }
        let value = &declaration[start..at];
        at += 1;
        match name {
            b"version" if !version => {
                if value != b"1.0" && value != b"1.1" { return None; }
                version = true;
            }
            b"encoding" => {
                if !value.first().is_some_and(u8::is_ascii_alphabetic)
                    || !value.iter().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')) {
                    return None;
                }
                return std::str::from_utf8(value).ok().and_then(Encoding::label).map(Encoding::name);
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xhr_xml_declarations_require_initial_exact_names_quotes_and_encoding_names() {
        for source in [
            "<?xml version='1.0' encoding='Shift_JIS'?>",
            "<?xml\tversion = \"1.1\"\r\nencoding = 'sHiFt_JiS' ?>",
            "<?xml encoding='Shift_JIS'?>",
        ] { assert_eq!(determine(source.as_bytes()), Some("shift_jis")); }
        for source in [
            " <?xml version='1.0' encoding='Shift_JIS'?>",
            "<!-- <?xml encoding='Shift_JIS'?> -->",
            "<?XML version='1.0' encoding='Shift_JIS'?>",
            "<?xml-stylesheet encoding='Shift_JIS'?>",
            "<?xml version='1.0' Encoding='Shift_JIS'?>",
            "<?xml version='1.0' encoding=Shift_JIS?>",
            "<?xml version='1.0' encoding='Shift_JIS\"?>",
            "<?xml version='1.0' encoding=' Shift_JIS'?>",
            "<?xml version='1.0' encoding='unknown'?>",
            "<?xml version='encoding=Shift_JIS'?>",
            "<?xml version='1.0' standalone='yes' encoding='Shift_JIS'?>",
            "<?xml version='1.0' encoding='Shift_JIS'",
        ] { assert_eq!(determine(source.as_bytes()), None, "{source}"); }
        assert_eq!(determine(b"<?xml encoding='windows-1252'?>"), Some("windows-1252"));
        assert_eq!(determine(b"<?xml encoding='replacement'?>"), Some("replacement"));
        assert_eq!(determine(b"\xef\xbb\xbf<?xml encoding='Shift_JIS'?>"), Some("utf-8"));
    }

    #[test]
    fn xhr_xml_long_declaration_scan_has_work_admission_and_fresh_recovery() {
        use super::super::Runtime;
        let source = format!("<?xml version='1.0'{}encoding='windows-1252'?><r>\u{81}</r>", " ".repeat(4096));
        let bytes = source.as_bytes();
        let mut runtime = Runtime::with_limits(crate::javascript::limits::BROWSER);
        let retained = runtime.value_bytes;
        for allowance in [0, bytes.len()-1] {
            runtime.begin_job(); runtime.copy_bytes = allowance;
            assert!(runtime.xml_request_text_encoding(bytes).unwrap_err().contains("work limit"));
            assert_eq!(runtime.value_bytes, retained);
        }
        runtime.begin_job();
        assert_eq!(runtime.xml_request_text_encoding(bytes).unwrap(), Some("windows-1252"));
        assert_eq!(runtime.value_bytes, retained);
        assert_eq!(runtime.xml_request_text_encoding(b"<r/>").unwrap(), None);
    }
}
