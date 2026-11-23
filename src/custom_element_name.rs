/// HTML's autonomous custom-element names start with an ASCII lowercase
/// letter and use the DOM local-name grammar.
/// https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name
pub(crate) fn valid(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name.contains('-')
        && !name.bytes().any(|b| matches!(b, b'A'..=b'Z' | 0 | b'\t' | b'\n' | 0x0c | b'\r' | b' ' | b'/' | b'>'))
        && !matches!(name, "annotation-xml" | "color-profile" | "font-face" | "font-face-src"
            | "font-face-uri" | "font-face-format" | "font-face-name" | "missing-glyph")
}
