//! Canonical INPUT formAction USVString URL reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-input-formaction
use super::*;
pub(super) const GET:&str="\0HTMLInputElement.get.formAction";
pub(super) const SET:&str="\0HTMLInputElement.set.formAction";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn input_form_action_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLInputElement")}
 pub(super) fn input_form_action_member(&self,object:u32,property:&str)->bool{property=="formAction"&&self.input_form_action_receiver(object)}
 pub(super) fn validate_input_form_action(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.input_form_action_receiver(object){return Err(("TypeError","Input formAction requires a genuine HTML input element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child input formAction requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn input_form_action_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_input_form_action(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.element_action_url_get(object,"formaction")
 }
 pub(super) fn input_form_action_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_input_form_action(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.form_action_usv(value)?;self.state.copies(10)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("formaction")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value.into()))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value.into()),script_attr::InputPreparation::Pending)}
 }
}
