//! AREA.relList uses the actual null-namespace rel attribute token list.
//! https://html.spec.whatwg.org/multipage/image-maps.html#dom-area-rellist
use super::*;
pub(super) const GET:&str="\0HTMLAreaElement.get.relList";
pub(super) const SET:&str="\0HTMLAreaElement.set.relList";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn area_rel_list_member(&self,object:u32,property:&str)->bool{property=="relList"&&self.area_rel_receiver(object)}
 pub(super) fn validate_area_rel_list(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.area_rel_receiver(object){return Err(("TypeError","HTMLAreaElement.relList requires a genuine HTML area.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child area token lists require their lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn area_rel_list_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_area_rel_list(object).map_err(|(_,message)|message)?;
  Ok(Value::Host(script_class_list::area_rel_handle(object)))
 }
}
