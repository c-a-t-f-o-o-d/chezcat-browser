//! Canonical TIME datetime DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-time-element
use super::*;
pub(super) const GET:&str="\0HTMLTimeElement.get.dateTime";
pub(super) const SET:&str="\0HTMLTimeElement.set.dateTime";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn time_datetime_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTimeElement")}
 pub(super) fn time_datetime_member(&self,object:u32,property:&str)->bool{property=="dateTime"&&self.time_datetime_receiver(object)}
 pub(super) fn validate_time_datetime(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.time_datetime_receiver(object){return Err(("TypeError","Time datetime metadata requires a genuine HTML time element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child time metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn time_datetime_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_time_datetime(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"datetime")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn time_datetime_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_time_datetime(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("datetime")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
