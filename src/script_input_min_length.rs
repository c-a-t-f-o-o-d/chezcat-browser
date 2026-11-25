//! Canonical INPUT nonnegative signed-long minimum independent of native edit bounds.
//! https://html.spec.whatwg.org/multipage/common-dom-interfaces.html#reflect
use super::*;
pub(super) const GET:&str="\0HTMLInputElement.get.minLength";
pub(super) const SET:&str="\0HTMLInputElement.set.minLength";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn input_min_length_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLInputElement")}
 pub(super) fn input_min_length_member(&self,object:u32,property:&str)->bool{property=="minLength"&&self.input_min_length_receiver(object)}
 pub(super) fn validate_input_min_length(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.input_min_length_receiver(object){return Err(("TypeError","Input minLength requires a genuine HTML input element.".into()));}self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child input minLength requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn validate_input_min_length_assignment(&self,object:u32,value:&Value)->Result<(),(&'static str,String)>{
  self.validate_input_min_length(object)?;if let Value::Number(n)=value{if crate::javascript::int32(*n)<0{return Err(("IndexSizeError","Input minLength cannot be negative.".into()));}}Ok(())
 }
 pub(super) fn input_min_length_get(&mut self,object:u32)->Result<Value,String>{self.validate_input_min_length(object).map_err(|(_,s)|s)?;self.state.work(1)?;script_text_content::deadline(self.state)?;let value=self.html_long_attribute(object,"minlength")?.filter(|value|*value>=0).unwrap_or(-1);script_text_content::deadline(self.state)?;Ok(Value::Number(f64::from(value)))}
 pub(super) fn input_min_length_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_input_min_length_assignment(object,value).map_err(|(_,s)|s)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let Value::Number(n)=value else{return Err("Input minLength requires number conversion.".into());};let text=self.text(&Value::Number(f64::from(crate::javascript::int32(*n))))?;self.state.copies(9)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("minlength")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
