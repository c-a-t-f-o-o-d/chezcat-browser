//! HTMLInputElement.labels retains a live native NodeList identity.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-lfe-labels
use super::*;
pub(in crate::scripting) const GET:&str="\0HTMLInputElement.get.labels";
impl Bridge<'_>{
 pub(in crate::scripting) fn html_input_labels_receiver(&self,object:u32)->bool{
  self.concrete_html_interface(object)==Some("HTMLInputElement")||self.inert_interface(object)==Some("HTMLInputElement")
 }
 pub(in crate::scripting) fn html_input_labels_member(&self,object:u32,property:&str)->bool{property=="labels"&&self.html_input_labels_receiver(object)}
 pub(in crate::scripting) fn validate_html_input_labels(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.html_input_labels_receiver(object){return Err(("TypeError","HTMLInputElement.labels requires a genuine HTML input.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child input label lists require their tree lifecycle hooks.".into()));}
  Ok(())
 }
 pub(in crate::scripting) fn html_input_labels_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_html_input_labels(object).map_err(|(_,e)|e)?;
  let lookup=self.state.inert_documents.adopted.lookup_work().checked_add(8).and_then(|n|n.checked_mul(8)).ok_or("Input labels lookup work overflow.")?;
  self.state.work(lookup)?;
  if !self.labelable_element(object)?{return Ok(Value::Null);}
  self.create_input_labels_list(object)
 }
 pub(in crate::scripting) fn input_labels_list_nodes(&mut self,object:u32)->Result<Vec<u32>,String>{
  self.validate_html_input_labels(object).map_err(|(_,e)|e)?;
  let lookup=self.state.inert_documents.adopted.lookup_work().checked_add(8).and_then(|n|n.checked_mul(8)).ok_or("Input labels lookup work overflow.")?;
  self.state.work(lookup)?;self.state.rebuild(self.doc)?;
  if !self.labelable_element(object)?{return Ok(Vec::new());}
  let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(1)).ok_or("Input labels tree bound overflow.")?;
  let mut root=object;let mut count=0usize;
  while root!=DOCUMENT{
   count=count.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic input labels ancestry.")?;self.state.work(lookup)?;
   let Some(parent)=self.whole_text_parent(root)?else{break;};self.native_node_access(parent).map_err(|(_,e)|e)?;root=parent;
  }
  let available=self.state.retained_limit.saturating_sub(self.state.retained);
  let mut pending=Vec::new();let mut found=Vec::new();self.label_pending_push(&mut pending,root,available)?;let mut visited=0usize;
  while let Some(node)=pending.pop(){
   visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic input labels tree.")?;self.state.work(lookup)?;
   if self.label_element_facts(node).is_some_and(|(namespace,tag,_)|namespace==Namespace::Html&&tag=="label"){
    if !self.html_label_receiver(node){return Err("Input labels cross an unsupported native label backing.".into());}
    let other=pending.capacity().checked_add(found.capacity()).and_then(|n|n.checked_mul(size_of::<u32>())).ok_or("Input labels scratch peak overflow.")?;
    if matches!(self.label_control_with_scratch(node,other)?,Value::Host(control) if control==object){
     self.native_node_access(node).map_err(|(_,e)|e)?;
     let other=pending.capacity().checked_mul(size_of::<u32>()).ok_or("Input labels pending storage overflow.")?;
     self.label_pending_push(&mut found,node,available.checked_sub(other).ok_or("Input labels result scratch allowance exhausted.")?)?;
    }
   }
   if self.template_receiver(node){continue;}
   let count=self.label_child_count(node)?;
   for at in (0..count).rev(){
    self.state.work(lookup)?;let child=self.label_child_at(node,at)?;
    let other=found.capacity().checked_mul(size_of::<u32>()).ok_or("Input labels result storage overflow.")?;
    self.label_pending_push(&mut pending,child,available.checked_sub(other).ok_or("Input labels traversal scratch allowance exhausted.")?)?;
   }
  }
  Ok(found)
 }
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn input_labels_nested_lookup_admits_outer_scratch_alongside_its_own_stack(){
  let mut doc=crate::html::parse("<html><body><label for=field>Label</label><input id=field></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
  let label=(0..doc.nodes.len()).find(|&n|tag(&doc,n)=="label").unwrap();let input=(0..doc.nodes.len()).find(|&n|tag(&doc,n)=="input").unwrap();let before=format!("{:?}",doc.nodes);let state=(page.state.retained,page.state.dom_commits);page.state.retained_limit=page.state.retained+16;
  assert!(matches!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.label_control_with_scratch(node_handle(label),0).unwrap(),Value::Host(handle) if handle==node_handle(input)));
  page.begin_job();let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.label_control_with_scratch(node_handle(label),16).unwrap_err();assert!(error.contains("allowance"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!((page.state.retained,page.state.dom_commits),state);
 }
}
