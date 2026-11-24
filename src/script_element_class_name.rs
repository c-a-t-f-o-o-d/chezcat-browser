//! Canonical Element.className reflection over authoritative native code units.
//! https://dom.spec.whatwg.org/#dom-element-classname
use super::*;
pub(super) const GET:&str="\0Element.get.className";
pub(super) const SET:&str="\0Element.set.className";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn element_class_name_member(&self,object:u32,property:&str)->bool{
  property=="className"&&self.canonical_selector_receiver(object)&&!self.form_named_control(object,property)
 }
 pub(super) fn validate_element_class_name(&self,object:u32,setter:bool)->Result<(),(&'static str,String)>{
  if !self.canonical_selector_receiver(object){return Err(("TypeError","Element.className requires a genuine Element.".into()));}
  self.native_node_access(object)?;
  if setter&&self.is_frame_handle(object){return Err(("NotSupportedError","Initial child Element.className writes require their lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn element_class_name_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_element_class_name(object,false).map_err(|(_,message)|message)?;
  self.element_attribute_string_get(object,"class")
 }
 pub(super) fn element_class_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_element_class_name(object,true).map_err(|(_,message)|message)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("class")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}
  else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
