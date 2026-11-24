//! Canonical HR color DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/obsolete.html#HTMLHRElement-partial
use super::*;
pub(super) const GET:&str="\0HTMLHRElement.get.color";
pub(super) const SET:&str="\0HTMLHRElement.set.color";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn hr_color_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLHRElement")}
 pub(super) fn hr_color_member(&self,object:u32,property:&str)->bool{property=="color"&&self.hr_color_receiver(object)}
 pub(super) fn validate_hr_color(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.hr_color_receiver(object){return Err(("TypeError","HR color metadata requires a genuine HTML horizontal rule element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child horizontal rule metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn hr_color_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_hr_color(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"color")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn hr_color_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_hr_color(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("color")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
