//! FORM.relList uses the actual null-namespace rel attribute token list.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-rellist
use super::*;
pub(super) const GET:&str="\0HTMLFormElement.get.relList";
pub(super) const SET:&str="\0HTMLFormElement.set.relList";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn form_rel_list_member(&self,object:u32,property:&str)->bool{property=="relList"&&self.form_rel_receiver(object)&&!self.form_named_control(object,property)}
 pub(super) fn validate_form_rel_list(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.form_rel_receiver(object){return Err(("TypeError","HTMLFormElement.relList requires a genuine HTML form.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form token lists require their lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn form_rel_list_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_form_rel_list(object).map_err(|(_,message)|message)?;
  Ok(Value::Host(script_class_list::area_rel_handle(object)))
 }
}
