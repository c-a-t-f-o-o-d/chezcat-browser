//! Original comment token boundary reader shared by document and fragment trees.
//! https://html.spec.whatwg.org/multipage/parsing.html#comment-start-state
// Data is literal (not an entity/reference context). The pending delimiter
// characters at EOF are not part of the emitted token, just as at a close.
pub(super) fn token(source:&str)->(&str,usize){
    if source.starts_with('>'){return ("",1);}
    if source.starts_with("->"){return ("",2);}
    let normal=source.find("-->").map(|end|(end,3));
    let bang=source.find("--!>").map(|end|(end,4));
    let closed=match(normal,bang){(Some(a),Some(b))=>Some(if a.0<b.0{a}else{b}),(a,None)|(None,a)=>a};
    if let Some((end,delimiter))=closed{return (&source[..end],end+delimiter);}
    let pending=if source.ends_with("--!"){3}else if source.ends_with("--"){2}else if source.ends_with('-'){1}else{0};
    (&source[..source.len()-pending],source.len())
}

pub(super) fn copy_data(source:&str)->Result<String,String>{
    let nulls=source.bytes().filter(|&byte|byte==0).count();
    let length=nulls.checked_mul(2).and_then(|bytes|bytes.checked_add(source.len())).ok_or("HTML comment data length overflow.")?;
    let mut text=String::new();text.try_reserve_exact(length).map_err(|_|"Could not reserve HTML comment data.")?;
    if nulls==0{text.push_str(source);}else{for character in source.chars(){text.push(if character=='\0'{'\u{fffd}'}else{character});}}
    Ok(text)
}
