//! Canonical readonly SELECT/TEXTAREA type metadata from actual native state.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-select-type
use super::*;
pub(super) const GETTERS:&[(&str,&str)]=&[("HTMLSelectElement","\0HTMLSelectElement.get.type"),("HTMLTextAreaElement","\0HTMLTextAreaElement.get.type")];
pub(super) fn interface(method:&str)->Option<&'static str>{GETTERS.iter().find_map(|&(name,op)|(op==method).then_some(name))}
impl Bridge<'_>{
 fn control_type_interface(&self,object:u32)->Option<&'static str>{self.concrete_html_interface(object).or_else(||self.inert_interface(object)).filter(|name|GETTERS.iter().any(|&(interface,_)|interface==*name))}
 pub(super) fn control_type_member(&self,object:u32,property:&str)->bool{property=="type"&&self.control_type_interface(object).is_some()}
 pub(super) fn control_type_method(&self,object:u32)->Option<&'static str>{let name=self.control_type_interface(object)?;GETTERS.iter().find_map(|&(interface,op)|(interface==name).then_some(op))}
 pub(super) fn control_type_matches(&self,object:u32,method:&str)->bool{interface(method).is_some_and(|name|self.control_type_interface(object)==Some(name))}
 pub(super) fn validate_control_type(&self,object:u32,method:&str)->Result<(),(&'static str,String)>{
  if !self.control_type_matches(object,method){return Err(("TypeError","Type getter requires its genuine concrete HTML control.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child control type metadata requires its lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn control_type_get(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_control_type(object,method).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=if interface(method)==Some("HTMLTextAreaElement"){"textarea"}else{
   let count=self.element_attribute_source(object).ok_or("Select attribute source is unavailable.")?.len();self.state.work(count)?;
   let mut multiple=false;
   for at in 0..count{let bytes=self.element_attribute_source(object).unwrap()[at].name.bytes();self.state.work(bytes.saturating_add(1))?;if self.element_attribute_source(object).unwrap()[at].name.html_is("multiple"){multiple=true;break;}}
   if multiple{"select-multiple"}else{"select-one"}
  };
  script_text_content::deadline(self.state)?;
  if value.len()>self.state.limits.host.string_bytes{return Err("Control type exceeds the host string allowance.".into());}
  self.state.copies(value.len())?;
  let value=script_clone_node::copy_text(value)?;script_text_content::deadline(self.state)?;Ok(Value::String(value))
 }
}
