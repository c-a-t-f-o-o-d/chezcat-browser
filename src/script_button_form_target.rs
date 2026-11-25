//! Canonical Button formTarget DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-fs-formtarget
use super::*;
pub(super) const GET:&str="\0HTMLButtonElement.get.formTarget";
pub(super) const SET:&str="\0HTMLButtonElement.set.formTarget";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn button_form_target_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLButtonElement")}
 pub(super) fn button_form_target_member(&self,object:u32,property:&str)->bool{property=="formTarget"&&self.button_form_target_receiver(object)}
 pub(super) fn validate_button_form_target(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.button_form_target_receiver(object){return Err(("TypeError","Form target requires a genuine HTML button element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form target requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn button_form_target_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_button_form_target(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_optional_attribute_string_get(object,"formtarget")?.unwrap_or_else(||Value::String(String::new()));script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn button_form_target_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_button_form_target(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(10)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("formtarget")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
