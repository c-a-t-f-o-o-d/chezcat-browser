//! Prepaid borrowed ID/form comparisons preserve explicit-owner mutation guards.
use super::*;
pub(super) fn used(doc:&Document,forms:&Forms,state:&mut State,keys:[Option<&DomString>;2])->Result<bool,String>{
 for &node in forms.controls.keys(){
  let item=doc.nodes.get(node).ok_or("Explicit form control node is unavailable.")?;
  let NodeKind::Element{attrs,..}=&item.kind else{return Err("Explicit form control is not an Element.".into())};
  state.work(attrs.len().saturating_add(1))?;
  for attr in attrs{
   state.work(attr.name.bytes().saturating_add(1))?;
   if !attr.name.html_is("form"){continue;}
   if !attr.value.is_empty(){
    for key in keys.into_iter().flatten(){
     state.work(attr.value.logical_bytes().min(key.logical_bytes()).saturating_add(1))?;
     if &attr.value==key{return Ok(true);}
    }
   }
   break;
  }
 }
 Ok(false)
}
