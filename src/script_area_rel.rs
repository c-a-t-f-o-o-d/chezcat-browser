//! Canonical AREA rel DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/image-maps.html#the-area-element
use super::*;
pub(super) const GET:&str="\0HTMLAreaElement.get.rel";
pub(super) const SET:&str="\0HTMLAreaElement.set.rel";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn area_rel_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLAreaElement")}
 pub(super) fn area_rel_member(&self,object:u32,property:&str)->bool{property=="rel"&&self.area_rel_receiver(object)}
 pub(super) fn validate_area_rel(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.area_rel_receiver(object){return Err(("TypeError","Area rel metadata requires a genuine HTML area element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child area metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn area_rel_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_area_rel(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"rel")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn area_rel_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_area_rel(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(3)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("rel")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
