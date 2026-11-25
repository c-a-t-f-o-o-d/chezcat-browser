//! Canonical BUTTON value DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element
use super::*;
pub(super) const GET:&str="\0HTMLButtonElement.get.value";
pub(super) const SET:&str="\0HTMLButtonElement.set.value";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn button_value_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLButtonElement")}
 pub(super) fn button_value_member(&self,object:u32,property:&str)->bool{property=="value"&&self.button_value_receiver(object)}
 pub(super) fn validate_button_value(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.button_value_receiver(object){return Err(("TypeError","Button value requires a genuine HTML button element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child button value requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn button_value_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_button_value(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"value")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn button_value_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_button_value(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(6)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("value")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
