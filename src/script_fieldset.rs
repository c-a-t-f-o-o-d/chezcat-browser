//! Canonical FIELDSET identity and readonly owner/type metadata.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-fieldset-element
use super::*;
pub(super) const FORM:&str="\0HTMLFieldSetElement.get.form";
pub(super) const TYPE:&str="\0HTMLFieldSetElement.get.type";
pub(super) fn operation(method:&str)->bool{matches!(method,FORM|TYPE)}
impl Bridge<'_>{
 pub(super) fn fieldset_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLFieldSetElement")}
 pub(super) fn fieldset_member(&self,object:u32,property:&str)->bool{matches!(property,"form"|"type")&&self.fieldset_receiver(object)}
 pub(super) fn validate_fieldset(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.fieldset_receiver(object){return Err(("TypeError","Fieldset metadata requires a genuine HTML fieldset.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child fieldset metadata requires its lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn fieldset_get(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_fieldset(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=match method{
   FORM=>{self.state.rebuild(self.doc)?;self.control_form_owner_get(object)?},
   TYPE=>{if self.state.limits.host.string_bytes<8{return Err("Fieldset type exceeds the host string allowance.".into());}self.state.copies(8)?;Value::String(script_clone_node::copy_text("fieldset")?)},
   _=>return Err("Unknown fieldset metadata operation.".into())
  };
  script_text_content::deadline(self.state)?;Ok(value)
 }
}
