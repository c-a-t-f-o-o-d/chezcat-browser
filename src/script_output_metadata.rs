//! Canonical OUTPUT name reflection and fixed readonly type metadata.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-output-element
use super::*;
pub(super) const NAME_GET:&str="\0HTMLOutputElement.get.name";
pub(super) const NAME_SET:&str="\0HTMLOutputElement.set.name";
pub(super) const TYPE_GET:&str="\0HTMLOutputElement.get.type";
pub(super) fn operation(method:&str)->bool{matches!(method,NAME_GET|NAME_SET|TYPE_GET)}
impl Bridge<'_>{
 pub(super) fn output_metadata_member(&self,object:u32,property:&str)->bool{matches!(property,"name"|"type")&&self.html_output_receiver(object)}
 pub(super) fn output_name_member(&self,object:u32,property:&str)->bool{property=="name"&&self.html_output_receiver(object)}
 pub(super) fn validate_output_metadata(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.html_output_receiver(object){return Err(("TypeError","OUTPUT metadata requires a genuine HTML output.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child output metadata requires its lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn output_metadata_get(&mut self,object:u32,property:&str)->Result<Value,String>{
  self.validate_output_metadata(object).map_err(|(_,message)|message)?;
  if property=="name"{return self.element_attribute_string_get(object,"name");}
  if property!="type"{return Err("Unknown OUTPUT metadata property.".into());}
  self.state.work(1)?;
  if self.state.limits.host.string_bytes<6{return Err("OUTPUT type exceeds the host string allowance.".into());}
  self.state.copies(6)?;Ok(Value::String("output".into()))
 }
 pub(super) fn output_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_output_metadata(object).map_err(|(_,message)|message)?;
  let value=self.dom_text(value)?;self.state.copies(4)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("name")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}
  else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
