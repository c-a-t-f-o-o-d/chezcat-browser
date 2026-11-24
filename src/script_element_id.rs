//! Canonical Element.id reflection over authoritative native code units.
//! https://dom.spec.whatwg.org/#dom-element-id
use super::*;
pub(super) const GET:&str="\0Element.get.id";
pub(super) const SET:&str="\0Element.set.id";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn element_id_member(&self,object:u32,property:&str)->bool{
  property=="id"&&self.canonical_selector_receiver(object)&&!self.form_named_control(object,property)
 }
 pub(super) fn validate_element_id(&self,object:u32,setter:bool)->Result<(),(&'static str,String)>{
  if !self.canonical_selector_receiver(object){return Err(("TypeError","Element.id requires a genuine Element.".into()));}
  self.native_node_access(object)?;
  if setter&&self.is_frame_handle(object){return Err(("NotSupportedError","Initial child Element.id writes require their lifecycle hooks.".into()));}
  Ok(())
 }
 fn element_id_attributes(&self,object:u32)->Result<&[crate::html::Attribute],String>{
  if let Some((owner,node))=self.frame_contains_node(object){
   let NodeKind::Element{attrs,..}=&self.contains_child_document(owner)?.nodes[node].kind else{return Err("Child id source is not an Element.".into())};return Ok(attrs);
  }
  self.element_attribute_source(object).ok_or("Element.id attribute source is unavailable.".into())
 }
 pub(super) fn element_id_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_element_id(object,false).map_err(|(_,message)|message)?;
  self.element_attribute_string_get(object,"id")
 }
 // The caller validates the genuine Element and captured native authority first.
 pub(super) fn element_attribute_string_get(&mut self,object:u32,key:&str)->Result<Value,String>{
  let count=self.element_id_attributes(object)?.len();self.state.work(count.saturating_add(1))?;
  let mut found=None;
  for at in 0..count{
   let bytes=self.element_id_attributes(object)?[at].name.bytes();self.state.work(bytes.saturating_add(1))?;
   if self.element_id_attributes(object)?[at].name.html_is(key){found=Some(at);break;}
  }
  let Some(at)=found else{return Ok(Value::String(String::new()))};
  let bytes=self.element_id_attributes(object)?[at].value.logical_bytes();self.state.work(bytes)?;
  if bytes>self.state.limits.host.string_bytes{return Err(if key=="id"{"Element.id exceeds the host string allowance."}else{"Element.className exceeds the host string allowance."}.into());}
  self.state.copies(bytes)?;Self::dom_string_copy(&self.element_id_attributes(object)?[at].value)
 }
 pub(super) fn element_id_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_element_id(object,true).map_err(|(_,message)|message)?;
  let value=self.dom_text(value)?;self.state.copies(2)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("id")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}
  else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
