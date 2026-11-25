//! Canonical MAP name DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/image-maps.html#the-map-element
use super::*;
pub(super) const GET:&str="\0HTMLMapElement.get.name";
pub(super) const SET:&str="\0HTMLMapElement.set.name";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn map_name_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLMapElement")}
 pub(super) fn map_name_member(&self,object:u32,property:&str)->bool{property=="name"&&self.map_name_receiver(object)}
 pub(super) fn validate_map_name(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.map_name_receiver(object){return Err(("TypeError","Map name metadata requires a genuine HTML map element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child map metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn map_name_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_map_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"name")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn map_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_map_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("name")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
