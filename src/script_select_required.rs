//! Canonical Select required boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-select-required
use super::*;
pub(super) const GET:&str="\0HTMLSelectElement.get.required";
pub(super) const SET:&str="\0HTMLSelectElement.set.required";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn select_required_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLSelectElement")}
 pub(super) fn select_required_member(&self,object:u32,property:&str)->bool{property=="required"&&self.select_required_receiver(object)}
 pub(super) fn validate_select_required(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.select_required_receiver(object){return Err(("TypeError","Select required metadata requires a genuine HTML select element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child select metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn select_required_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_select_required(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("Select attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("required");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn select_required_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_select_required(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("required")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}
