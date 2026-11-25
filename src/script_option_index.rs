//! Canonical Option index counts the actual qualifying SELECT option list.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-option-index
use super::*;
pub(super) const GET:&str="\0HTMLOptionElement.get.index";
impl Bridge<'_>{
 pub(super) fn option_index_member(&self,object:u32,property:&str)->bool{property=="index"&&self.option_disabled_receiver(object)}
 pub(super) fn validate_option_index(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.option_disabled_receiver(object){return Err(("TypeError","Option index requires a genuine HTML option element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child option index requires its lifecycle hooks.".into()));}Ok(())
 }
 // The validated SELECT authorizes physical reads of its actual descendants.
 // Resource and non-element records need not expose public script handles.
 fn option_index_children(&self,object:u32)->Result<usize,String>{
  if let Some(children)=self.inert_children(object){return Ok(children.len());}
  let node=node_index(object).and_then(|n|self.doc.nodes.get(n)).ok_or("Missing option list parent.")?;Ok(node.children.len())
 }
 fn option_index_child(&self,object:u32,at:usize)->Result<u32,String>{
  if let Some(children)=self.inert_children(object){return children.get(at).copied().ok_or("Missing inert option list edge.".into());}
  node_index(object).and_then(|n|self.doc.nodes.get(n)).and_then(|node|node.children.get(at)).copied().map(node_handle).ok_or("Missing native option list edge.".into())
 }
 fn option_index_push(&mut self,pending:&mut Vec<(u32,bool)>,node:u32,group:bool,available:usize)->Result<(),String>{
  if pending.len()==pending.capacity(){
   let old=pending.capacity().checked_mul(size_of::<(u32,bool)>()).ok_or("Option list scratch overflow.")?;
   let free=available.checked_sub(old.checked_mul(2).ok_or("Option list peak overflow.")?).ok_or("Option list scratch allowance exhausted.")?;
   let(replacement,_)=retained_storage::grow(pending,1,free)?;
   if let Some(mut next)=replacement{self.state.copies(pending.len().checked_mul(size_of::<(u32,bool)>()).ok_or("Option list copy overflow.")?)?;next.append(pending);*pending=next;}
  }
  self.state.copies(size_of::<(u32,bool)>())?;pending.push((node,group));Ok(())
 }
 pub(super) fn option_index_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_option_index(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;self.state.rebuild(self.doc)?;
  let Some(select)=self.option_nearest_select(object)?else{return Ok(Value::Number(0.0));};
  let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(1)).ok_or("Option list tree bound overflow.")?;
  let lookup=((usize::BITS-self.inert_node_count().leading_zeros())as usize).checked_add(9).and_then(|n|n.checked_mul(8)).ok_or("Option list lookup overflow.")?;
  let available=self.state.retained_limit.saturating_sub(self.state.retained);let mut pending=Vec::new();let mut visited=0usize;let mut index=0usize;
  let count=self.option_index_children(select)?;for at in (0..count).rev(){self.state.work(lookup)?;let child=self.option_index_child(select,at)?;self.option_index_push(&mut pending,child,false,available)?;}
  while let Some((node,mut group))=pending.pop(){
   visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic option list.")?;self.state.work(lookup)?;script_text_content::deadline(self.state)?;
   let work=self.option_form_facts(node).map_or(0,|(_,tag)|tag.len());self.state.work(work.saturating_add(1))?;
   if let Some((crate::html::Namespace::Html,tag))=self.option_form_facts(node){match tag{
    "option"=>{if node==object{return Ok(Value::Number(index as f64));}index=index.checked_add(1).filter(|&n|n<=i32::MAX as usize).ok_or("Option index exceeds its integer bound.")?;continue;},
    "select"|"hr"|"datalist"|"template"=>continue,
    "optgroup"=>{if group{continue;}group=true;},_=>{},
   }}
   let count=self.option_index_children(node)?;for at in (0..count).rev(){self.state.work(lookup)?;let child=self.option_index_child(node,at)?;self.option_index_push(&mut pending,child,group,available)?;}
  }
  Ok(Value::Number(0.0))
 }
}
