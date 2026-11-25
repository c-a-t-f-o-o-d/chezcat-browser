//! Canonical PARAM name DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/#htmlparamelement
use super::*;
pub(super) const GET:&str="\0HTMLParamElement.get.name";
pub(super) const SET:&str="\0HTMLParamElement.set.name";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn param_name_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLParamElement")}
 pub(super) fn param_name_member(&self,object:u32,property:&str)->bool{property=="name"&&self.param_name_receiver(object)}
 pub(super) fn validate_param_name(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.param_name_receiver(object){return Err(("TypeError","Param metadata requires a genuine HTML param element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child param metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn param_name_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_param_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"name")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn param_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_param_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("name")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
