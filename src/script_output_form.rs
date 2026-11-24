//! OUTPUT.form shares the actual native form-owner lookup used by labels.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-output-form
use super::*;
pub(super) const GET:&str="\0HTMLOutputElement.get.form";
impl Bridge<'_>{
 pub(super) fn output_form_member(&self,object:u32,property:&str)->bool{property=="form"&&self.html_output_receiver(object)}
 pub(super) fn validate_output_form(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.html_output_receiver(object){return Err(("TypeError","HTMLOutputElement.form requires a genuine HTML output.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child output form owners require their lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn output_form_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_output_form(object).map_err(|(_,message)|message)?;
  self.state.rebuild(self.doc)?;
  self.control_form_owner_get(object)
 }
}
