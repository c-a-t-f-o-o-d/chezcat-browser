//! Canonical OL type DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/grouping-content.html#the-ol-element
use super::*;
pub(super) const GET:&str="\0HTMLOListElement.get.type";
pub(super) const SET:&str="\0HTMLOListElement.set.type";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn ol_type_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLOListElement")}
 pub(super) fn ol_type_member(&self,object:u32,property:&str)->bool{property=="type"&&self.ol_type_receiver(object)}
 pub(super) fn validate_ol_type(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.ol_type_receiver(object){return Err(("TypeError","OL type metadata requires a genuine HTML ordered list element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child OL metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn ol_type_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_ol_type(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"type")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn ol_type_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_ol_type(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("type")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
