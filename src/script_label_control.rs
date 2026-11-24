//! Associated labels inspect their actual root, namespaces and DOMString IDs.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-label-control
use super::*;
use crate::html::{Attribute,Namespace};
pub(in crate::scripting) const GET:&str="\0HTMLLabelElement.get.control";
impl Bridge<'_>{
 pub(in crate::scripting) fn html_label_control_member(&self,object:u32,property:&str)->bool{property=="control"&&self.html_label_receiver(object)}
 pub(in crate::scripting) fn validate_html_label_control(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.html_label_receiver(object){return Err(("TypeError","HTMLLabelElement.control requires a genuine HTML label.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child label association requires its tree lifecycle hooks.".into()));}
  Ok(())
 }
 fn label_element_facts(&self,object:u32)->Option<(Namespace,&str,&[Attribute])>{
  if let Some(source)=self.inert_import_source(object){return match source{
   script_inert_documents::ImportSource::Element(tag)=>Some((Namespace::Html,tag,self.inert_element_attributes(object)?)),
   script_inert_documents::ImportSource::Parsed{node,..}=>match &node.kind{NodeKind::Element{tag,attrs,..}=>Some((node.namespace,tag,attrs)),_=>None},_=>None,
  };}
  if self.inert_node_type(object).is_some(){return None;}
  let node=self.doc.nodes.get(node_index(object)?)?;
  match &node.kind{NodeKind::Element{tag,attrs,..}=>Some((node.namespace,tag,attrs)),_=>None}
 }
 fn label_attribute_slot(&mut self,object:u32,key:&str)->Result<Option<usize>,String>{
  let Some((_,_,attrs))=self.label_element_facts(object)else{return Ok(None);};
  let work=attrs.iter().try_fold(attrs.len(),|n,a|n.checked_add(a.name.bytes()).ok_or("Label attribute work overflow."))?;
  self.state.work(work)?;
  Ok(self.label_element_facts(object).unwrap().2.iter().position(|a|a.name.html_is(key)))
 }
 fn labelable_element(&mut self,object:u32)->Result<bool,String>{
  let Some((_,tag,_))=self.label_element_facts(object)else{return Ok(false);};
  let work=tag.len().checked_add(1).ok_or("Label element work overflow.")?;self.state.work(work)?;
  let(namespace,tag,_)=self.label_element_facts(object).unwrap();
  if namespace!=Namespace::Html{return Ok(false);}
  match tag{
   "button"|"meter"|"output"|"progress"|"select"|"textarea"=>Ok(true),
   "input"=>{
    let Some(at)=self.label_attribute_slot(object,"type")?else{return Ok(true);};
    self.state.work(7)?;
    Ok(!self.label_element_facts(object).unwrap().2[at].value.code_units().map(|u|if (65..=90).contains(&u){u+32}else{u}).eq("hidden".encode_utf16()))
   },_=>Ok(false),
  }
 }
 fn label_id_matches(&mut self,node:u32,label:u32,at:usize)->Result<bool,String>{
  let Some(id)=self.label_attribute_slot(node,"id")?else{return Ok(false);};
  let left=&self.label_element_facts(node).unwrap().2[id].value;
  let right=&self.label_element_facts(label).unwrap().2[at].value;
  let cost=left.used_bytes().min(right.used_bytes()).checked_add(1).ok_or("Label ID comparison work overflow.")?;
  self.state.work(cost)?;
  Ok(self.label_element_facts(node).unwrap().2[id].value==self.label_element_facts(label).unwrap().2[at].value)
 }
 // Root authority is validated before this physical read walk. Parser records
 // such as doctypes and resource nodes need not expose script handles; they
 // still occupy tree order and their IDs can block a later labelable element.
 fn label_child_count(&self,object:u32)->Result<usize,String>{
  if let Some(children)=self.inert_children(object){return Ok(children.len());}
  let at=if object==DOCUMENT{0}else{node_index(object).ok_or("Missing label tree record.")?};
  Ok(self.doc.nodes.get(at).ok_or("Invalid label tree record.")?.children.len())
 }
 fn label_child_at(&self,object:u32,at:usize)->Result<u32,String>{
  if let Some(children)=self.inert_children(object){return children.get(at).copied().ok_or("Missing inert label tree edge.".into());}
  let node=if object==DOCUMENT{0}else{node_index(object).ok_or("Missing label parent record.")?};
  self.doc.nodes.get(node).and_then(|node|node.children.get(at)).copied().map(node_handle).ok_or("Missing label tree edge.".into())
 }
 fn label_pending_push(&mut self,pending:&mut Vec<u32>,node:u32,available:usize)->Result<(),String>{
  if pending.len()==pending.capacity(){
   let old=pending.capacity().checked_mul(size_of::<u32>()).ok_or("Label traversal storage overflow.")?;
   let free=available.checked_sub(old.checked_mul(2).ok_or("Label traversal peak overflow.")?).ok_or("Label traversal scratch allowance exhausted.")?;
   let(replacement,_)=retained_storage::grow(pending,1,free)?;
   if let Some(mut next)=replacement{self.state.copies(pending.len().checked_mul(size_of::<u32>()).ok_or("Label traversal copy overflow.")?)?;next.append(pending);*pending=next;}
  }
  self.state.copies(size_of::<u32>())?;pending.push(node);Ok(())
 }
 pub(in crate::scripting) fn html_label_control_get(&mut self,object:u32)->Result<Value,String>{
  self.label_control_with_scratch(object,0)
 }
 fn label_control_with_scratch(&mut self,object:u32,other:usize)->Result<Value,String>{
  self.validate_html_label_control(object).map_err(|(_,e)|e)?;
  let lookup=self.state.inert_documents.adopted.lookup_work().checked_add(8).and_then(|n|n.checked_mul(8)).ok_or("Label lookup work overflow.")?;
  self.state.work(lookup)?;self.state.rebuild(self.doc)?;
  let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(1)).ok_or("Label traversal bound overflow.")?;
  let explicit=self.label_attribute_slot(object,"for")?;
  if explicit.is_some_and(|at|self.label_element_facts(object).unwrap().2[at].value.is_empty()){return Ok(Value::Null);}
  let mut root=object;
  if explicit.is_some(){
   let mut ancestors=0usize;
   while root!=DOCUMENT{
    ancestors=ancestors.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic label ancestry.")?;self.state.work(lookup)?;
    let Some(parent)=self.whole_text_parent(root)?else{break;};self.native_node_access(parent).map_err(|(_,e)|e)?;root=parent;
   }
  }
  let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(other).ok_or("Label traversal shared scratch allowance exhausted.")?;
  let mut pending=Vec::new();self.label_pending_push(&mut pending,root,available)?;let mut visited=0usize;
  while let Some(node)=pending.pop(){
   visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic label tree.")?;self.state.work(lookup)?;
   if let Some(at)=explicit{
    if self.label_id_matches(node,object,at)?{if self.labelable_element(node)?{self.native_node_access(node).map_err(|(_,e)|e)?;return Ok(Value::Host(node));}return Ok(Value::Null);}
   }else if node!=object&&self.labelable_element(node)?{self.native_node_access(node).map_err(|(_,e)|e)?;return Ok(Value::Host(node));}
   if self.template_receiver(node){continue;}
   let count=self.label_child_count(node)?;
   for at in (0..count).rev(){self.state.work(lookup)?;let child=self.label_child_at(node,at)?;self.label_pending_push(&mut pending,child,available)?;}
  }
  Ok(Value::Null)
 }
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn label_control_scratch_growth_admits_old_and_replacement_peak(){
  let mut doc=crate::html::parse("<html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);let retained=page.state.retained;let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
  let mut pending=Vec::with_capacity(4);pending.extend([1,2,3,4]);let minimum=4*size_of::<u32>()+5*size_of::<u32>();let copies=b.state.copy_work;
  assert!(b.label_pending_push(&mut pending,5,minimum-1).is_err());assert_eq!(pending,[1,2,3,4]);assert_eq!(pending.capacity(),4);assert_eq!(b.state.retained,retained);
  b.label_pending_push(&mut pending,5,minimum).unwrap();assert_eq!(pending,[1,2,3,4,5]);assert_eq!(pending.capacity(),5);assert_eq!(b.state.copy_work,copies-5*size_of::<u32>());assert_eq!(b.state.retained,retained);
 }
}

#[path="script_label_form.rs"]
pub(in crate::scripting) mod label_form;

#[path="script_input_labels.rs"]
pub(in crate::scripting) mod input_labels;

#[path="script_form_length.rs"]
pub(in crate::scripting) mod form_length;
