//! Canonical PRE width signed-long reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/common-dom-interfaces.html#reflect
use super::*;
pub(super) const GET:&str="\0HTMLPreElement.get.width";
pub(super) const SET:&str="\0HTMLPreElement.set.width";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn pre_width_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLPreElement")}
 pub(super) fn pre_width_member(&self,object:u32,property:&str)->bool{property=="width"&&self.pre_width_receiver(object)}
 pub(super) fn validate_pre_width(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.pre_width_receiver(object){return Err(("TypeError","PRE width metadata requires a genuine HTML preformatted text element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child PRE metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn pre_width_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_pre_width(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.html_long_attribute(object,"width")?.unwrap_or(0);script_text_content::deadline(self.state)?;Ok(Value::Number(f64::from(value)))
 }
 pub(super) fn pre_width_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_pre_width(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let Value::Number(number)=value else{return Err("PRE width setters require number conversion.".into());};
  let value=Value::Number(f64::from(crate::javascript::int32(*number)));let text=self.text(&value)?;self.state.copies(5)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("width")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
