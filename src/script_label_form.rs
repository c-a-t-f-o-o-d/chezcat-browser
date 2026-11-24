//! Labels return the actual associated control's form owner.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-label-form
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#reset-the-form-owner
use super::*;
pub(in crate::scripting) const GET:&str="\0HTMLLabelElement.get.form";
impl Bridge<'_>{
 pub(in crate::scripting) fn html_label_form_member(&self,object:u32,property:&str)->bool{property=="form"&&self.html_label_receiver(object)}
 pub(in crate::scripting) fn validate_html_label_form(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.html_label_receiver(object){return Err(("TypeError","HTMLLabelElement.form requires a genuine HTML label.".into()));}
  self.validate_html_label_control(object)
 }
 fn label_is_html_form(&mut self,object:u32)->Result<bool,String>{
  let Some((_,tag,_))=self.label_element_facts(object)else{return Ok(false);};
  self.state.work(tag.len().checked_add(1).ok_or("Label form name work overflow.")?)?;
  Ok(self.label_element_facts(object).is_some_and(|(namespace,tag,_)|namespace==Namespace::Html&&tag=="form"))
 }
 fn label_checked_form(&mut self,object:u32)->Result<Value,String>{
  if !self.label_is_html_form(object)?{return Ok(Value::Null);}
  self.native_node_access(object).map_err(|(_,e)|e)?;Ok(Value::Host(object))
 }
 pub(in crate::scripting) fn html_label_form_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_html_label_form(object).map_err(|(_,e)|e)?;
  let Value::Host(control)=self.html_label_control_get(object)?else{return Ok(Value::Null);};
  self.control_form_owner_get(control)
 }
 pub(in crate::scripting) fn control_form_owner_get(&mut self,control:u32)->Result<Value,String>{self.control_form_owner_with_tree(control,false)}
 // Counts inspect connected physical main-tree records without exposing their
 // handles. The caller validates its genuine form and origin before entering.
 pub(in crate::scripting) fn control_form_count_owner(&mut self,control:u32)->Result<Value,String>{self.control_form_owner_with_tree(control,true)}
 pub(in crate::scripting) fn form_count_physical_record(&self,object:u32)->bool{self.inert_node_type(object).is_none()&&node_index(object).is_some_and(|n|n<self.doc.nodes.len()&&self.state.dom.connected.get(n)==Some(&true)&&!self.inert_adopted_node(object))}
 pub(in crate::scripting) fn form_count_parent(&self,object:u32)->Result<Option<u32>,String>{if self.form_count_physical_record(object){Ok(self.state.tree.parents.get(node_index(object).unwrap()).copied().flatten().map(|n|if n==0{DOCUMENT}else{node_handle(n)}))}else{self.whole_text_parent(object)}}
 fn control_form_owner_with_tree(&mut self,control:u32,physical:bool)->Result<Value,String>{
  let(_,tag,_)=self.label_element_facts(control).ok_or("Missing label control element.")?;
  self.state.work(tag.len().checked_add(1).ok_or("Label form association work overflow.")?)?;
  if !matches!(self.label_element_facts(control).unwrap().1,"button"|"fieldset"|"input"|"object"|"output"|"select"|"textarea"){return Ok(Value::Null);}
  // Native controls own real parser/lifecycle state. Do not reconstruct or
  // replace their owner from a label's current ancestor or author properties.
  if self.inert_node_type(control).is_none(){if let Some(node)=node_index(control){
   self.state.work((usize::BITS-self.forms.controls.len().leading_zeros())as usize+1)?;
   if let Some(owner)=self.forms.controls.get(&node).map(|field|field.owner){return match owner{Some(owner)=>self.label_checked_form(node_handle(owner)),None=>Ok(Value::Null)};}
  }}
  // Pure inert records and ordinary controls without widgets have no parser
  // association override. Read their actual connected/detached owner tree.
  let lookup=self.state.inert_documents.adopted.lookup_work().checked_add(8).and_then(|n|n.checked_mul(8)).ok_or("Label form lookup work overflow.")?;
  let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(1)).ok_or("Label form tree bound overflow.")?;
  let mut root=control;let mut ancestor=None;let mut count=0usize;
  while root!=DOCUMENT{
   count=count.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic label form ancestry.")?;self.state.work(lookup)?;
   let parent=if physical{self.form_count_parent(root)?}else{self.whole_text_parent(root)?};
   let Some(parent)=parent else{break;};
   if !(physical&&self.form_count_physical_record(parent)){self.native_node_access(parent).map_err(|(_,message)|message)?;}
   if ancestor.is_none()&&self.label_is_html_form(parent)?{ancestor=Some(parent);}
   root=parent;
  }
  let connected=root==DOCUMENT||self.inert_node_type(root)==Some(9);
  let attribute=if connected{self.label_attribute_slot(control,"form")?}else{None};
  let Some(attribute)=attribute else{return match ancestor{Some(form)if physical&&self.form_count_physical_record(form)=>Ok(Value::Host(form)),Some(form)=>self.label_checked_form(form),None=>Ok(Value::Null)};};
  if self.label_element_facts(control).unwrap().2[attribute].value.is_empty(){return Ok(Value::Null);}
  let available=self.state.retained_limit.saturating_sub(self.state.retained);
  let mut pending=Vec::new();self.label_pending_push(&mut pending,root,available)?;let mut visited=0usize;
  while let Some(node)=pending.pop(){
   visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic label form tree.")?;self.state.work(lookup)?;
   if self.label_id_matches(node,control,attribute)?{return if physical&&self.form_count_physical_record(node){Ok(if self.label_is_html_form(node)?{Value::Host(node)}else{Value::Null})}else{self.label_checked_form(node)};}
   if self.template_receiver(node){continue;}
   let count=self.label_child_count(node)?;
   for at in (0..count).rev(){self.state.work(lookup)?;let child=self.label_child_at(node,at)?;self.label_pending_push(&mut pending,child,available)?;}
  }
  Ok(Value::Null)
 }
}
