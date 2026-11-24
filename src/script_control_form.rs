//! Canonical readonly form owners share native widget and inert DOM authority.
use super::*;
pub(super) const GETTERS:&[(&str,&str)]=&[
 ("HTMLInputElement","\0HTMLInputElement.get.form"),
 ("HTMLButtonElement","\0HTMLButtonElement.get.form"),
 ("HTMLSelectElement","\0HTMLSelectElement.get.form"),
 ("HTMLTextAreaElement","\0HTMLTextAreaElement.get.form")];
pub(super) fn interface(method:&str)->Option<&'static str>{GETTERS.iter().find_map(|&(name,op)|(op==method).then_some(name))}
impl Bridge<'_>{
 fn control_form_interface(&self,object:u32)->Option<&'static str>{
  self.concrete_html_interface(object).or_else(||self.inert_interface(object)).filter(|name|GETTERS.iter().any(|&(interface,_)|interface==*name))
 }
 pub(super) fn control_form_member(&self,object:u32,property:&str)->bool{property=="form"&&self.control_form_interface(object).is_some()}
 pub(super) fn control_form_method(&self,object:u32)->Option<&'static str>{let name=self.control_form_interface(object)?;GETTERS.iter().find_map(|&(interface,op)|(interface==name).then_some(op))}
 pub(super) fn control_form_matches(&self,object:u32,method:&str)->bool{interface(method).is_some_and(|name|self.control_form_interface(object)==Some(name))}
 pub(super) fn validate_control_form(&self,object:u32,method:&str)->Result<(),(&'static str,String)>{
  if !self.control_form_matches(object,method){return Err(("TypeError","Form owner getter requires its genuine concrete HTML control.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child control form owners require their lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn control_form_get(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_control_form(object,method).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;self.state.rebuild(self.doc)?;
  let result=self.control_form_owner_get(object)?;script_text_content::deadline(self.state)?;Ok(result)
 }
}
