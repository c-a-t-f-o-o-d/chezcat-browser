//! Exact Param value/type/valueType DOMStrings over shared real attributes.
//! https://html.spec.whatwg.org/#htmlparamelement
use super::*;
use crate::javascript::param_metadata_interface as metadata;
pub(super) fn operation(method:&str)->bool{metadata::operation(method).is_some()}
impl Bridge<'_>{
 pub(super) fn param_metadata_member(&self,object:u32,property:&str)->bool{self.param_name_receiver(object)&&metadata::attribute(property).is_some()}
 pub(super) fn param_metadata_get(&mut self,object:u32,property:&str)->Result<Value,String>{
  self.validate_param_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let attribute=metadata::attribute(property).ok_or_else(||"Unsupported Param metadata property.".to_owned())?;
  let value=self.element_attribute_string_get(object,attribute)?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn param_metadata_set(&mut self,object:u32,property:&str,value:&Value)->Result<(),String>{
  self.validate_param_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let attribute=metadata::attribute(property).ok_or_else(||"Unsupported Param metadata property.".to_owned())?;
  let value=self.dom_text(value)?;self.state.copies(attribute.len().saturating_mul(2))?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text(attribute)?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
