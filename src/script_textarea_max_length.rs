//! Canonical TEXTAREA nonnegative signed-long maximum and native edit bound.
//! https://html.spec.whatwg.org/multipage/common-dom-interfaces.html#reflect
use super::*;
pub(super) const GET:&str="\0HTMLTextAreaElement.get.maxLength";
pub(super) const SET:&str="\0HTMLTextAreaElement.set.maxLength";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn textarea_max_length_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTextAreaElement")}
 pub(super) fn textarea_max_length_member(&self,object:u32,property:&str)->bool{property=="maxLength"&&self.textarea_max_length_receiver(object)}
 pub(super) fn validate_textarea_max_length(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.textarea_max_length_receiver(object){return Err(("TypeError","Textarea maxLength requires a genuine HTML textarea element.".into()));}self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child textarea maxLength requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn validate_textarea_max_length_assignment(&self,object:u32,value:&Value)->Result<(),(&'static str,String)>{
  self.validate_textarea_max_length(object)?;if let Value::Number(n)=value{if crate::javascript::int32(*n)<0{return Err(("IndexSizeError","Textarea maxLength cannot be negative.".into()));}}Ok(())
 }
 pub(super) fn textarea_max_length_get(&mut self,object:u32)->Result<Value,String>{self.validate_textarea_max_length(object).map_err(|(_,s)|s)?;self.state.work(1)?;script_text_content::deadline(self.state)?;let value=self.html_long_attribute(object,"maxlength")?.filter(|value|*value>=0).unwrap_or(-1);script_text_content::deadline(self.state)?;Ok(Value::Number(f64::from(value)))}
 pub(super) fn textarea_max_length_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_textarea_max_length_assignment(object,value).map_err(|(_,s)|s)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let Value::Number(n)=value else{return Err("Textarea maxLength requires number conversion.".into());};let text=self.text(&Value::Number(f64::from(crate::javascript::int32(*n))))?;self.state.copies(9)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("maxlength")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
