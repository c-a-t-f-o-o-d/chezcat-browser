//! Classic script selection uses the preparation-time fallback, response charset
//! and Encoding's BOM override. Stylesheet and document sniffing are separate.
use crate::text_codec::{self, Encoding};

#[path = "script_xhr_mime.rs"]
mod mime;

pub(crate) fn script_fallback(doc: &crate::html::Document, node: usize) -> Encoding {
    doc.dom_attr(node, "charset").and_then(|value| value.scalar().ok()).and_then(Encoding::label).unwrap_or(doc.encoding)
}

pub(crate) fn decode_script(
    bytes: &[u8], content_type: &str, fallback: Encoding, limit: usize,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<String, String> {
    check()?;
    let encoding = if let Some(bom) = Encoding::bom(bytes) {
        bom
    } else {
        mime::parse(content_type)?.as_ref().and_then(|mime| mime.charset())
            .and_then(Encoding::label).unwrap_or(fallback)
    };
    text_codec::decode(bytes, encoding, limit, check)
}

#[cfg(test)]
#[path = "script_encoding_tests.rs"]
mod tests;
