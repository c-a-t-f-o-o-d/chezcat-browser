//! Canonical Area noHref boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/obsolete.html#HTMLAreaElement-partial
use super::*;
pub(super) const GET:&str="\0HTMLAreaElement.get.noHref";
pub(super) const SET:&str="\0HTMLAreaElement.set.noHref";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn area_no_href_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLAreaElement")}
 pub(super) fn area_no_href_member(&self,object:u32,property:&str)->bool{property=="noHref"&&self.area_no_href_receiver(object)}
 pub(super) fn validate_area_no_href(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.area_no_href_receiver(object){return Err(("TypeError","Area noHref metadata requires a genuine HTML area element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child area metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn area_no_href_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_area_no_href(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("Area attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("nohref");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn area_no_href_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_area_no_href(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(6)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("nohref")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}
