//! Exact token mutation workspace, released before the attribute transaction.
use super::*;
use crate::dom_string::{ascii_whitespace_tokens,DomStringRef};

fn insert<'a>(state:&mut State,tokens:&mut Vec<DomStringRef<'a>>,token:DomStringRef<'a>)->Result<(),String>{
    state.work(tokens.len().saturating_add(1))?;
    let cost=tokens.iter().fold(token.logical_bytes().saturating_mul(tokens.len()).saturating_add(tokens.len()),|cost,prior|cost.saturating_add(prior.logical_bytes()));
    state.work(cost)?;
    if !tokens.iter().any(|prior|prior.same_units(token)){tokens.push(token);}
    Ok(())
}
fn units<'a,'b:'a>(tokens:&'a [DomStringRef<'b>])->impl Iterator<Item=u16>+'a {
    tokens.iter().enumerate().flat_map(|(index,token)|std::iter::once(0x20).take(usize::from(index!=0)).chain(token.code_units()))
}

pub(super) fn serialize<'a>(state:&mut State,source:DomStringRef<'a>,args:&'a [Value],add:bool)->Result<DomString,String>{
    state.work(args.len())?;
    let arg_bytes=args.iter().try_fold(0usize,|count,value|count.checked_add(token_list_reads::argument(value)?.logical_bytes()).ok_or_else(||"Token mutation argument work overflow.".to_owned()))?;
    // Admit token counting and traversal before either inspects source units.
    state.work(source.logical_bytes().saturating_mul(2).saturating_add(arg_bytes.saturating_mul(2)).saturating_add(args.len()))?;
    let count=ascii_whitespace_tokens(source).count().checked_add(if add{args.len()}else{0}).ok_or("Token mutation snapshot count overflow.")?;
    let available=state.retained_limit.saturating_sub(state.retained);
    let required=count.checked_mul(size_of::<DomStringRef<'_>>()).filter(|&bytes|bytes<=available).ok_or("Token mutation snapshot retained allowance exhausted.")?;
    state.copies(required)?;
    let mut tokens=Vec::new();tokens.try_reserve_exact(count).map_err(|_|"Cannot reserve token mutation snapshot.")?;
    let metadata=tokens.capacity().checked_mul(size_of::<DomStringRef<'_>>()).filter(|&bytes|bytes<=available).ok_or("Token mutation snapshot returned allocation exceeds allowance.")?;
    for token in ascii_whitespace_tokens(source){
        if !add {
            state.work(token.logical_bytes().saturating_mul(args.len()).saturating_add(arg_bytes).saturating_add(args.len()))?;
            let mut removed=false;
            for value in args {if token.same_units(token_list_reads::argument(value)?){removed=true;break;}}
            if removed{continue;}
        }
        insert(state,&mut tokens,token)?;
    }
    if add{for value in args{insert(state,&mut tokens,token_list_reads::argument(value)?)?;}}
    state.work(tokens.len())?;
    let scan=tokens.iter().fold(tokens.len(),|bytes,token|bytes.saturating_add(token.logical_bytes()));state.work(scan)?;
    let count=tokens.iter().try_fold(tokens.len().saturating_sub(1),|count,token|count.checked_add(token.code_units().count()).ok_or("Token mutation serialized unit count overflow."))?;
    // Projection sizing, UTF-16 staging and the constructor's projection/raw
    // passes all consume this prepaid linear allowance.
    state.work(count.saturating_mul(5))?;
    let mut projection=0usize;let mut raw=false;
    for item in char::decode_utf16(units(&tokens)) {
        let bytes=match item{Ok(ch)=>ch.len_utf8(),Err(_)=>{raw=true;3}};
        projection=projection.checked_add(bytes).ok_or("Token mutation serialized projection overflow.")?;
    }
    let scratch=count.checked_mul(size_of::<u16>()).ok_or("Token mutation scratch byte count overflow.")?;
    let output=projection.checked_add(if raw{scratch}else{0}).ok_or("Token mutation final storage overflow.")?;
    let logical=if raw{scratch}else{projection};
    if logical>state.limits.host.string_bytes{return Err("Token mutation serialization exceeds the host string bound.".into());}
    metadata.checked_add(scratch).and_then(|n|n.checked_add(output)).filter(|&n|n<=available).ok_or("Token mutation snapshot/scratch/output retained allowance exhausted.")?;
    state.copies(scratch.checked_add(output).ok_or("Token mutation serialization copy work overflow.")?)?;
    let mut staged=Vec::new();staged.try_reserve_exact(count).map_err(|_|"Cannot reserve token mutation UTF-16 serialization.")?;
    let scratch=staged.capacity().checked_mul(size_of::<u16>()).filter(|&n|n<=available-metadata).ok_or("Token mutation scratch returned allocation exceeds allowance.")?;
    let remaining=available-metadata-scratch;
    if output>remaining{return Err("Token mutation scratch/output allocation exceeds allowance.".into());}
    staged.extend(units(&tokens));
    DomString::from_units(&staged,remaining)
}
