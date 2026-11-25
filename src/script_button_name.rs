//! Canonical BUTTON name DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-fe-name
use super::*;
pub(super) const GET:&str="\0HTMLButtonElement.get.name";
pub(super) const SET:&str="\0HTMLButtonElement.set.name";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn button_name_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLButtonElement")}
 pub(super) fn button_name_member(&self,object:u32,property:&str)->bool{property=="name"&&self.button_name_receiver(object)}
 pub(super) fn validate_button_name(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.button_name_receiver(object){return Err(("TypeError","Button name requires a genuine HTML button element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form name requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn button_name_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_button_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_optional_attribute_string_get(object,"name")?.unwrap_or_else(||Value::String(String::new()));script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn button_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_button_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(4)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("name")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
