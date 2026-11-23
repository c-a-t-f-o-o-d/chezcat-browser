//! Original CSS Syntax byte selection. @charset is an exact initial byte
//! signature, distinct from HTML meta and classic-script response selection.
use crate::text_codec::{self, Encoding};

#[path = "script_xhr_mime.rs"]
mod mime;

pub(crate) fn environment(doc: &crate::html::Document, node: usize) -> Encoding {
    doc.attr(node,"charset").and_then(Encoding::label).unwrap_or(doc.encoding)
}

fn declaration(bytes: &[u8]) -> Option<Encoding> {
    let prefix=&bytes[..bytes.len().min(1024)];
    let tail=prefix.strip_prefix(b"@charset \"")?;
    let end=tail.iter().position(|&byte|byte==b'"')?;
    if tail.get(end+1)!=Some(&b';') || !tail[..end].is_ascii() { return None; }
    let encoding=Encoding::label(std::str::from_utf8(&tail[..end]).ok()?)?;
    Some(if matches!(encoding,Encoding::UTF16LE|Encoding::UTF16BE){Encoding::UTF8}else{encoding})
}

pub(crate) fn decode(
    bytes: &[u8], content_type: &str, environment: Option<Encoding>, limit: usize,
    check: &dyn Fn() -> Result<(),String>,
) -> Result<String,String> {
    check()?;
    let encoding=if let Some(bom)=Encoding::bom(bytes) { bom } else {
        mime::parse(content_type)?.as_ref().and_then(|mime|mime.charset()).and_then(Encoding::label)
            .or_else(||declaration(bytes)).or(environment).unwrap_or(Encoding::UTF8)
    };
    text_codec::decode(bytes,encoding,limit,check)
}

#[cfg(test)]
#[path = "stylesheet_encoding_tests.rs"]
mod tests;
