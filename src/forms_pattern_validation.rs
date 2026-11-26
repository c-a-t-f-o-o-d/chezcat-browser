//! Checked native text-input pattern matching; no author callbacks or cache.
use super::*;
fn spend(work:&mut usize,count:usize)->Result<(),String>{
 if count>*work{*work=0;return Err("Regular-expression work limit reached.".into());}*work-=count;Ok(())
}
fn reserve(values:&mut Vec<u16>,count:usize,work:&mut usize,copies:&mut usize,available:&mut usize)->Result<(),String>{
 let bytes=count.checked_mul(std::mem::size_of::<u16>()).ok_or("HTML pattern scratch storage overflow.")?;
 if bytes>*available{return Err("HTML pattern storage allowance exhausted.".into());}
 if bytes>*copies{*copies=0;return Err("DOM string work limit reached while preparing an HTML pattern.".into());}*copies-=bytes;spend(work,count)?;
 values.try_reserve_exact(count).map_err(|_|"Could not allocate HTML pattern scratch.")?;
 let storage=values.capacity().checked_mul(std::mem::size_of::<u16>()).ok_or("HTML pattern scratch allocation overflow.")?;
 *available=available.checked_sub(storage).ok_or("HTML pattern scratch allocation exceeds storage allowance.")?;Ok(())
}
impl Forms{
 pub(crate) fn pattern_mismatch(&self,doc:&Document,node:usize,work:&mut usize,copies:&mut usize,mut available:usize)->Result<bool,String>{
  let Some(control)=self.controls.get(&node).filter(|c|c.kind==Kind::Text&&tag(doc,node)=="input"&&!c.value.chars.is_empty())else{return Ok(false)};
  let Some(pattern)=doc.dom_attr(node,"pattern")else{return Ok(false)};
  // Metadata inspection and both UTF-16 copies spend the caller's job budget.
  spend(work,pattern.logical_bytes().checked_add(control.value.chars.len()).ok_or("HTML pattern work overflow.")?)?;
  let count=control.value.chars.iter().try_fold(0usize,|n,ch|n.checked_add(ch.len_utf16()).ok_or("HTML pattern input size overflow."))?;
  let mut source=Vec::new();let mut input=Vec::new();reserve(&mut source,pattern.utf16_len(),work,copies,&mut available)?;reserve(&mut input,count,work,copies,&mut available)?;
  source.extend(pattern.code_units());
  for ch in &control.value.chars{let mut units=[0u16;2];input.extend_from_slice(ch.encode_utf16(&mut units));}
  Ok(crate::javascript::html_pattern_matches(&source,&input,work,copies,available)?==Some(false))
 }
}
