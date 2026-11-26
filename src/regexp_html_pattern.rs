//! Original HTML pattern compilation through the shared RegExp engine.
//! https://html.spec.whatwg.org/multipage/input.html#the-pattern-attribute
use super::regexp::{Error,Regex};
fn spend(counter:&mut usize,count:usize,message:&'static str)->Result<(),String>{
    if count>*counter{*counter=0;return Err(message.into());}*counter-=count;Ok(())
}

pub(crate) fn html_pattern_matches(pattern:&[u16],input:&[u16],work:&mut usize,copies:&mut usize,mut available:usize)->Result<Option<bool>,String>{
    // Validate standalone syntax before adding anchors: a closing parenthesis
    // in an invalid attribute must not escape the noncapturing wrapper.
    let standalone=match Regex::compile_units(pattern,"v",work){
        Ok(re)=>re,
        Err(Error::Syntax(_))=>return Ok(None),
        Err(error)=>return Err(error.message().into()),
    };
    if standalone.retained_bytes>available{return Err("HTML pattern storage allowance exhausted.".into());}
    drop(standalone);
    let count=pattern.len().checked_add(6).ok_or("HTML pattern source size overflow.")?;
    let bytes=count.checked_mul(std::mem::size_of::<u16>()).ok_or("HTML pattern source storage overflow.")?;
    if bytes>available{return Err("HTML pattern storage allowance exhausted.".into());}
    spend(copies,bytes,"DOM string work limit reached while compiling an HTML pattern.")?;
    spend(work,count,"Regular-expression work limit reached.")?;
    let mut anchored=Vec::new();anchored.try_reserve_exact(count).map_err(|_|"Could not allocate HTML pattern source.")?;
    let storage=anchored.capacity().checked_mul(std::mem::size_of::<u16>()).ok_or("HTML pattern allocation overflow.")?;
    available=available.checked_sub(storage).ok_or("HTML pattern allocation exceeds storage allowance.")?;
    anchored.extend("^(?:".encode_utf16());anchored.extend_from_slice(pattern);anchored.extend(")$".encode_utf16());
    let re=Regex::compile_units(&anchored,"v",work).map_err(|error|error.message().to_owned())?;
    available=available.checked_sub(re.retained_bytes).ok_or("HTML pattern storage allowance exhausted.")?;
    let matched=re.find(input,0,true,work).map_err(|error|error.message().to_owned())?;
    if matched.as_ref().is_some_and(|m|m.retained_bytes()>available){return Err("HTML pattern result storage allowance exhausted.".into());}
    Ok(Some(matched.is_some()))
}

#[cfg(test)]
mod tests{
 use super::*;
 fn matches(pattern:&str,input:&str)->Option<bool>{let mut copies=usize::MAX;html_pattern_matches(&pattern.encode_utf16().collect::<Vec<_>>(),&input.encode_utf16().collect::<Vec<_>>(),&mut 20_000_000,&mut copies,usize::MAX).unwrap()}
 #[test]
 fn native_pattern_validation_shared_engine_standalone_and_anchors(){
  for pattern in ["[","a)|(?:b","(?-:a)"]{assert_eq!(matches(pattern,"z"),None);}
  for(pattern,input,yes)in [("a|b","ab",false),("a|b","a",true),("(?m:a$)","a\nb",false),("(?s:.)","\n",true),("(?i:a)b","AB",false),("(?i:a)b","Ab",true),("","",true),("","x",false)]{assert_eq!(matches(pattern,input),Some(yes),"{pattern} {input:?}");}
 }
 #[test]
 fn native_pattern_validation_shared_engine_refusal_and_fresh_recovery(){
  let mut copies=usize::MAX;
  assert!(html_pattern_matches(&[97],&[97],&mut 0,&mut copies,usize::MAX).is_err());
  assert!(html_pattern_matches(&[97],&[97],&mut 1_000_000,&mut 0,usize::MAX).is_err());
  assert!(html_pattern_matches(&[97],&[97],&mut 1_000_000,&mut copies,0).is_err());
  assert_eq!(matches("(?i:𐐀)","𐐨"),Some(true));
 }
}
