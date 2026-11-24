//! Canonical LI value signed-long reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/common-dom-interfaces.html#reflect
use super::*;
pub(super) const GET:&str="\0HTMLLIElement.get.value";
pub(super) const SET:&str="\0HTMLLIElement.set.value";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn li_value_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLLIElement")}
 pub(super) fn li_value_member(&self,object:u32,property:&str)->bool{property=="value"&&self.li_value_receiver(object)}
 pub(super) fn validate_li_value(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.li_value_receiver(object){return Err(("TypeError","LI value metadata requires a genuine HTML list item element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child LI metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn html_long_attribute(&mut self,object:u32,name:&str)->Result<Option<i32>,String>{
  let count=self.element_attribute_source(object).ok_or("Integer attribute source is unavailable.")?.len();self.state.work(count)?;
  for at in 0..count{
   let bytes=self.element_attribute_source(object).unwrap()[at].name.bytes();self.state.work(bytes.saturating_add(1))?;
   if self.element_attribute_source(object).unwrap()[at].name.html_is(name){
    let bytes=self.element_attribute_source(object).unwrap()[at].value.used_bytes();self.state.work(bytes.saturating_add(1))?;
    // Scalar projection preserves the ASCII integer prefix, including its end
    // at any unpaired UTF-16 code unit. Actual attribute storage is untouched.
    return Ok(crate::decimal_number::html_long(self.element_attribute_source(object).unwrap()[at].value.as_str()));
   }
  }Ok(None)
 }
 pub(super) fn li_value_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_li_value(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.html_long_attribute(object,"value")?.unwrap_or(0);script_text_content::deadline(self.state)?;Ok(Value::Number(f64::from(value)))
 }
 pub(super) fn li_value_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_li_value(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let Value::Number(number)=value else{return Err("LI value setters require number conversion.".into());};
  let value=Value::Number(f64::from(crate::javascript::int32(*number)));let text=self.text(&value)?;self.state.copies(5)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("value")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
