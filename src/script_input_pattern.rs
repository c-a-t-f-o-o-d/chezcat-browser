//! Canonical INPUT pattern DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/input.html#htmlinputelement
use super::*;
pub(super) const GET:&str="\0HTMLInputElement.get.pattern";
pub(super) const SET:&str="\0HTMLInputElement.set.pattern";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn input_pattern_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLInputElement")}
 pub(super) fn input_pattern_member(&self,object:u32,property:&str)->bool{property=="pattern"&&self.input_pattern_receiver(object)}
 pub(super) fn validate_input_pattern(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.input_pattern_receiver(object){return Err(("TypeError","Input pattern requires a genuine HTML input element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child input pattern requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn input_pattern_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_input_pattern(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"pattern")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn input_pattern_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_input_pattern(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("pattern")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
