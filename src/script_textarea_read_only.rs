//! Canonical Textarea readOnly boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-textarea-read-only
use super::*;
pub(super) const GET:&str="\0HTMLTextAreaElement.get.readOnly";
pub(super) const SET:&str="\0HTMLTextAreaElement.set.readOnly";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn textarea_read_only_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTextAreaElement")}
 pub(super) fn textarea_read_only_member(&self,object:u32,property:&str)->bool{property=="readOnly"&&self.textarea_read_only_receiver(object)}
 pub(super) fn validate_textarea_read_only(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.textarea_read_only_receiver(object){return Err(("TypeError","Textarea readOnly metadata requires a genuine HTML textarea element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child textarea metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn textarea_read_only_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_textarea_read_only(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("Textarea attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("readonly");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn textarea_read_only_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_textarea_read_only(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("readonly")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}
