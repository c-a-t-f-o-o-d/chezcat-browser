//! Canonical DATALIST options uses a live exact-namespace HTMLCollection.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-datalist-options
use super::*;
pub(super) const GET:&str="\0HTMLDataListElement.get.options";
impl Bridge<'_>{
 pub(super) fn datalist_options_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLDataListElement")}
 pub(super) fn datalist_options_member(&self,object:u32,property:&str)->bool{property=="options"&&self.datalist_options_receiver(object)}
 pub(super) fn validate_datalist_options(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.datalist_options_receiver(object){return Err(("TypeError","DataList options requires a genuine HTML datalist element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child datalist options requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn datalist_options_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_datalist_options(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.create_datalist_options_collection(object)
 }
}
