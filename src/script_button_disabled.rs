//! Canonical Button disabled boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-fe-disabled
use super::*;
pub(super) const GET:&str="\0HTMLButtonElement.get.disabled";
pub(super) const SET:&str="\0HTMLButtonElement.set.disabled";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn button_disabled_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLButtonElement")}
 pub(super) fn button_disabled_member(&self,object:u32,property:&str)->bool{property=="disabled"&&self.button_disabled_receiver(object)}
 pub(super) fn validate_button_disabled(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.button_disabled_receiver(object){return Err(("TypeError","Button disabled metadata requires a genuine HTML button element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child button metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn button_disabled_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_button_disabled(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("Select attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("disabled");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn button_disabled_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_button_disabled(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("disabled")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}
