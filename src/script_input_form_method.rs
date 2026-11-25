//! Canonical INPUT formMethod known-value DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-input-formmethod
use super::*;
pub(super) const GET:&str="\0HTMLInputElement.get.formMethod";
pub(super) const SET:&str="\0HTMLInputElement.set.formMethod";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn input_form_method_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLInputElement")}
 pub(super) fn input_form_method_member(&self,object:u32,property:&str)->bool{property=="formMethod"&&self.input_form_method_receiver(object)}
 pub(super) fn validate_input_form_method(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.input_form_method_receiver(object){return Err(("TypeError","Input formMethod requires a genuine HTML input element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child input formMethod requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn input_form_method_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_input_form_method(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.element_form_method_get(object,"formmethod",true)
 }
 pub(super) fn input_form_method_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_input_form_method(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(10)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("formmethod")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
