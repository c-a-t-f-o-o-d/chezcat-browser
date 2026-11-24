//! Canonical OL start signed-long reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/common-dom-interfaces.html#reflect
use super::*;
pub(super) const GET:&str="\0HTMLOListElement.get.start";
pub(super) const SET:&str="\0HTMLOListElement.set.start";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn ol_start_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLOListElement")}
 pub(super) fn ol_start_member(&self,object:u32,property:&str)->bool{property=="start"&&self.ol_start_receiver(object)}
 pub(super) fn validate_ol_start(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.ol_start_receiver(object){return Err(("TypeError","OL start metadata requires a genuine HTML ordered list element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child OL metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn ol_start_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_ol_start(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.html_long_attribute(object,"start")?.unwrap_or(1);script_text_content::deadline(self.state)?;Ok(Value::Number(f64::from(value)))
 }
 pub(super) fn ol_start_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_ol_start(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let Value::Number(number)=value else{return Err("OL start setters require number conversion.".into());};
  let value=Value::Number(f64::from(crate::javascript::int32(*number)));let text=self.text(&value)?;self.state.copies(5)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("start")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
