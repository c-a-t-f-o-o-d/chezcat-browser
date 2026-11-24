//! Canonical HR noShade boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/obsolete.html#HTMLHRElement-partial
use super::*;
pub(super) const GET:&str="\0HTMLHRElement.get.noShade";
pub(super) const SET:&str="\0HTMLHRElement.set.noShade";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn hr_no_shade_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLHRElement")}
 pub(super) fn hr_no_shade_member(&self,object:u32,property:&str)->bool{property=="noShade"&&self.hr_no_shade_receiver(object)}
 pub(super) fn validate_hr_no_shade(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.hr_no_shade_receiver(object){return Err(("TypeError","HR noShade metadata requires a genuine HTML horizontal rule element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child HR metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn hr_no_shade_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_hr_no_shade(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("HR attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("noshade");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn hr_no_shade_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_hr_no_shade(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(7)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("noshade")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}
