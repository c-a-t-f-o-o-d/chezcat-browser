//! Canonical Select disabled boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-select-disabled
use super::*;
pub(super) const GET:&str="\0HTMLSelectElement.get.disabled";
pub(super) const SET:&str="\0HTMLSelectElement.set.disabled";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn select_disabled_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLSelectElement")}
 pub(super) fn select_disabled_member(&self,object:u32,property:&str)->bool{property=="disabled"&&self.select_disabled_receiver(object)}
 pub(super) fn validate_select_disabled(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.select_disabled_receiver(object){return Err(("TypeError","Select disabled metadata requires a genuine HTML select element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child select metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn select_disabled_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_select_disabled(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("Select attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("disabled");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn select_disabled_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_select_disabled(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("disabled")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}
