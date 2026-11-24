//! Canonical BR clear DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/obsolete.html#HTMLBRElement-partial
use super::*;
pub(super) const GET:&str="\0HTMLBRElement.get.clear";
pub(super) const SET:&str="\0HTMLBRElement.set.clear";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn br_clear_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLBRElement")}
 pub(super) fn br_clear_member(&self,object:u32,property:&str)->bool{property=="clear"&&self.br_clear_receiver(object)}
 pub(super) fn validate_br_clear(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.br_clear_receiver(object){return Err(("TypeError","BR clear metadata requires a genuine HTML line break element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child line break metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn br_clear_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_br_clear(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"clear")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn br_clear_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_br_clear(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("clear")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
