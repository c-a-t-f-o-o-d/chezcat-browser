//! Original HTML character-reference decoding over pinned normative name data.
//! The caller chooses Data/RCDATA or attribute-value context. Replacements are
//! emitted directly and cannot reenter markup or character-reference parsing.

#[path = "html_named_references_data.rs"]
mod data;

const CANCEL_INTERVAL: usize = 4096;
type Cancel<'a> = Option<&'a dyn Fn() -> Result<(), String>>;

enum Replacement {
    Named(&'static str),
    Numeric(char),
}

pub(super) fn decode(input: &str, attribute: bool, check: Cancel<'_>) -> Result<String, String> {
    let mut length = 0usize;
    walk(input, attribute, check, |part| {
        length = length.checked_add(part.len()).ok_or("Decoded HTML reference length overflow.")?;
        Ok(())
    })?;
    let mut output = String::new();
    output.try_reserve_exact(length).map_err(|_| "Cannot reserve decoded HTML reference text.")?;
    walk(input, attribute, check, |part| {
        output.push_str(part);
        Ok(())
    })?;
    debug_assert_eq!(output.len(), length);
    Ok(output)
}

fn poll(check: Cancel<'_>) -> Result<(), String> {
    if let Some(check) = check { check()?; }
    Ok(())
}

fn walk(
    input: &str, attribute: bool, check: Cancel<'_>,
    mut emit: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    poll(check)?;
    let bytes = input.as_bytes();
    let mut position = 0usize;
    let mut literal = 0usize;
    let mut next_check = CANCEL_INTERVAL;
    while position < bytes.len() {
        if position >= next_check {
            poll(check)?;
            next_check = position.saturating_add(CANCEL_INTERVAL);
        }
        if bytes[position] == b'&' {
            if let Some((replacement, consumed)) = reference(&input[position + 1..], attribute, check)? {
                emit(&input[literal..position])?;
                match replacement {
                    Replacement::Named(value) => emit(value)?,
                    Replacement::Numeric(value) => {
                        let mut encoded = [0; 4];
                        emit(value.encode_utf8(&mut encoded))?;
                    }
                }
                position += 1 + consumed;
                literal = position;
                continue;
            }
        }
        position += 1;
    }
    emit(&input[literal..])?;
    poll(check)
}

#[derive(Clone,Copy)]
enum ReferenceInput<'a>{Bytes(&'a [u8]),Units(&'a [u16])}
impl ReferenceInput<'_>{
    // Character-reference syntax and the attribute ambiguity exception are
    // ASCII. Other code units terminate a candidate without changing them.
    fn get(self,index:usize)->Option<u8>{match self{Self::Bytes(bytes)=>bytes.get(index).copied(),Self::Units(units)=>units.get(index).and_then(|&unit|u8::try_from(unit).ok())}}
}

fn reference(input:&str,attribute:bool,check:Cancel<'_>)->Result<Option<(Replacement,usize)>,String>{
    reference_input(ReferenceInput::Bytes(input.as_bytes()),attribute,check)
}

fn reference_input(input:ReferenceInput<'_>,attribute:bool,check:Cancel<'_>)->Result<Option<(Replacement,usize)>,String>{
    if input.get(0)!=Some(b'#'){return Ok(named(input,attribute).map(|(value,consumed)|(Replacement::Named(value),consumed)));}
    let mut position=1;
    let radix=if matches!(input.get(position),Some(b'x'|b'X')){position+=1;16}else{10};
    let first_digit=position;let mut number=0u32;
    while let Some(digit)=input.get(position).and_then(|byte|char::from(byte).to_digit(radix)){
        if position%CANCEL_INTERVAL==0{poll(check)?;}
        number=number.saturating_mul(radix).saturating_add(digit);position+=1;
    }
    if position==first_digit{return Ok(None);}
    if input.get(position)==Some(b';'){position+=1;}
    Ok(Some((Replacement::Numeric(numeric_character(number)),position)))
}

fn named(input:ReferenceInput<'_>,attribute:bool)->Option<(&'static str,usize)>{
    let mut prefix=[0u8;data::MAX_NAME_BYTES];let mut matched=None;
    for index in 0..data::MAX_NAME_BYTES{
        let Some(byte)=input.get(index)else{break;};
        if !byte.is_ascii_alphanumeric()&&byte!=b';'{break;}
        prefix[index]=byte;let end=index+1;
        if let Ok(entry)=data::ENTITIES.binary_search_by(|entry|entry.0.as_bytes().cmp(&prefix[..end])){
            matched=Some((data::ENTITIES[entry].1,end,byte==b';'));
        }
        if byte==b';'{break;}
    }
    let(value,consumed,semicolon)=matched?;
    if attribute&&!semicolon&&input.get(consumed).is_some_and(|byte|byte.is_ascii_alphanumeric()||byte==b'='){return None;}
    Some((value,consumed))
}

// Exact-unit emission shares all lexical matching and numeric correction with
// scalar input. Replacements are emitted once and never reenter the tokenizer.
pub(super) fn decode_units(input:&[u16],attribute:bool,available:usize,check:Cancel<'_>)->Result<crate::dom_string::DomString,String>{
    let mut length=0usize;
    walk_units(input,attribute,check,|part|{length=length.checked_add(part.len()).ok_or("Decoded HTML unit length overflow.")?;Ok(())})?;
    let minimum=length.checked_mul(std::mem::size_of::<u16>()).ok_or("Decoded HTML unit storage overflow.")?;
    if minimum>available{return Err("Decoded HTML unit storage allowance exhausted.".into());}
    let mut output=Vec::new();output.try_reserve_exact(length).map_err(|_|"Cannot reserve decoded HTML units.")?;
    let temporary=output.capacity().checked_mul(std::mem::size_of::<u16>()).ok_or("Decoded HTML unit backing overflow.")?;
    let remaining=available.checked_sub(temporary).ok_or("Decoded HTML temporary allowance exhausted.")?;
    walk_units(input,attribute,check,|part|{output.extend_from_slice(part);Ok(())})?;
    debug_assert_eq!(output.len(),length);
    crate::dom_string::DomString::from_units(&output,remaining)
}

fn walk_units(input:&[u16],attribute:bool,check:Cancel<'_>,mut emit:impl FnMut(&[u16])->Result<(),String>)->Result<(),String>{
    poll(check)?;let(mut position,mut literal,mut next_check)=(0,0,CANCEL_INTERVAL);
    while position<input.len(){
        if position>=next_check{poll(check)?;next_check=position.saturating_add(CANCEL_INTERVAL);}
        if input[position]==u16::from(b'&'){
            if let Some((replacement,consumed))=reference_input(ReferenceInput::Units(&input[position+1..]),attribute,check)?{
                emit(&input[literal..position])?;
                match replacement{
                    Replacement::Named(value)=>{for unit in value.encode_utf16(){emit(&[unit])?;}},
                    Replacement::Numeric(value)=>{let mut encoded=[0u16;2];emit(value.encode_utf16(&mut encoded))?;},
                }
                position+=1+consumed;literal=position;continue;
            }
        }
        position+=1;
    }
    emit(&input[literal..])?;poll(check)
}

fn numeric_character(number: u32) -> char {
    // HTML numeric-reference corrections do not replace undefined C1 controls.
    let corrected = match number {
        0 => 0xfffd,
        0x80 => 0x20ac,
        0x82 => 0x201a,
        0x83 => 0x0192,
        0x84 => 0x201e,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02c6,
        0x89 => 0x2030,
        0x8a => 0x0160,
        0x8b => 0x2039,
        0x8c => 0x0152,
        0x8e => 0x017d,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201c,
        0x94 => 0x201d,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02dc,
        0x99 => 0x2122,
        0x9a => 0x0161,
        0x9b => 0x203a,
        0x9c => 0x0153,
        0x9e => 0x017e,
        0x9f => 0x0178,
        _ => number,
    };
    char::from_u32(corrected).unwrap_or('\u{fffd}')
}


#[cfg(test)]
mod exact_units_tests{
 use super::*;
 fn units(value:&crate::dom_string::DomString)->Vec<u16>{value.raw_units().map(|v|v.to_vec()).unwrap_or_else(||value.as_str().encode_utf16().collect())}
 fn source(value:&str)->Vec<u16>{let mut v=vec![0xdfff];v.extend(value.encode_utf16());v}
 #[test]
 fn range_fragment_rcdata_every_pinned_named_reference_exact_units(){
  for &(name,value) in data::ENTITIES{
   let mut input=source(&format!("&{name}"));input.push(0xd800);
   let mut expected=source(value);expected.push(0xd800);
   for attribute in [false,true]{assert_eq!(units(&decode_units(&input,attribute,usize::MAX,None).unwrap()),expected,"{name} attribute={attribute}");}
  }
 }
 #[test]
 fn range_fragment_rcdata_numeric_longest_match_ambiguity_and_no_retokenization(){
  let mut input=source("&not");input.push(0xd800);input.extend("in;&amp=&#xD800;&#x110000;&#0;&#x80;&#129;&#13;&amp;lt;&Afr;".encode_utf16());
  let mut expected=source("\u{ac}");expected.push(0xd800);expected.extend("in;&=\u{fffd}\u{fffd}\u{fffd}\u{20ac}\u{81}\r&lt;\u{1d504}".encode_utf16());
  assert_eq!(units(&decode_units(&input,false,usize::MAX,None).unwrap()),expected);
  for (input,expected) in [("&notit;&amp=","&notit;&amp="),("&not;it;&amp;=","\u{ac}it;&="),("&amp;lt;","&lt;")]{
   assert_eq!(units(&decode_units(&source(input),true,usize::MAX,None).unwrap()),source(expected));
  }
  let mut boundary=source("&amp");boundary.push(0xd800);let mut expected=source("&");expected.push(0xd800);
  assert_eq!(units(&decode_units(&boundary,true,usize::MAX,None).unwrap()),expected);
 }
 #[test]
 fn range_fragment_rcdata_actual_joint_backing_budget_cancellation_and_recovery(){
  let input=source("&NotEqualTilde;&Afr;&#13;");
  let decoded=decode_units(&input,false,usize::MAX,None).unwrap();
  let exact=units(&decoded).len()*2+decoded.capacity();
  assert!(decode_units(&input,false,exact-1,None).is_err());
  assert_eq!(units(&decode_units(&input,false,exact,None).unwrap()),units(&decoded));
  let input=source(&format!("&#{}65;","0".repeat(20_000)));
  let calls=std::cell::Cell::new(0);let cancel=||{calls.set(calls.get()+1);if calls.get()==2{Err("cancelled exact reference".into())}else{Ok(())}};
  assert!(decode_units(&input,false,usize::MAX,Some(&cancel)).unwrap_err().contains("cancelled exact reference"));
  assert_eq!(calls.get(),2);
  assert_eq!(units(&decode_units(&input,false,usize::MAX,None).unwrap()),source("A"));
 }
}
