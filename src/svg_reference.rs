//! Borrowed local SVG references shared by DOM admission and the rasterizer.
pub(crate) fn fragment(text:&str)->Option<&str>{text.strip_prefix('#')}

pub(crate) fn effect(text:&str)->Result<Option<&str>,&'static str>{
    let value=text.trim();
    if value=="none"{return Ok(None);}
    let mut inner=value.strip_prefix("url(").and_then(|value|value.strip_suffix(')'))
        .ok_or("SVG clipping and masking support one local url(#id) reference only.")?.trim();
    if inner.starts_with(['\'', '"']) {
        let quote=inner.as_bytes()[0] as char;
        inner=inner[1..].strip_suffix(quote).ok_or("Unclosed quoted SVG effect reference.")?;
    }
    let id=fragment(inner).filter(|id|!id.is_empty()&&!id.contains(|ch:char|ch.is_whitespace()||matches!(ch,'\''|'"'|'('|')')))
        .ok_or("SVG clipping and masking cannot fetch external references.")?;
    Ok(Some(id))
}
