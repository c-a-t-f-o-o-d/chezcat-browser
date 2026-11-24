//! Canonical Element.classList over the existing admitted native token lists.
//! https://dom.spec.whatwg.org/#dom-element-classlist
use super::*;
pub(super) const GET:&str="\0Element.get.classList";
pub(super) const SET:&str="\0Element.set.classList";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn element_class_list_member(&self,object:u32,property:&str)->bool{
  property=="classList"&&self.canonical_selector_receiver(object)&&!self.form_named_control(object,property)
 }
 pub(super) fn validate_element_class_list(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.canonical_selector_receiver(object){return Err(("TypeError","Element.classList requires a genuine Element.".into()));}
  self.native_node_access(object)
 }
 pub(super) fn element_class_list_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_element_class_list(object).map_err(|(_,message)|message)?;
  if !self.class_list_element_receiver(object)&&!self.inert_class_element_receiver(object){return Err("Element.classList requires supported native token-list owner storage.".into());}
  Ok(Value::Host(script_class_list::handle(object)))
 }
}
