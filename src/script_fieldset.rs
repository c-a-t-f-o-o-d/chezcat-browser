//! Canonical FIELDSET identity and readonly owner/type metadata.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-fieldset-element
use super::*;
pub(super) const FORM:&str="\0HTMLFieldSetElement.get.form";
pub(super) const TYPE:&str="\0HTMLFieldSetElement.get.type";
pub(super) const NAME_GET:&str="\0HTMLFieldSetElement.get.name";
pub(super) const NAME_SET:&str="\0HTMLFieldSetElement.set.name";
pub(super) fn operation(method:&str)->bool{matches!(method,FORM|TYPE|NAME_GET|NAME_SET)}
impl Bridge<'_>{
 pub(super) fn fieldset_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLFieldSetElement")}
 pub(super) fn fieldset_member(&self,object:u32,property:&str)->bool{matches!(property,"form"|"type"|"name")&&self.fieldset_receiver(object)}
 pub(super) fn fieldset_name_member(&self,object:u32,property:&str)->bool{property=="name"&&self.fieldset_receiver(object)}
 pub(super) fn validate_fieldset(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.fieldset_receiver(object){return Err(("TypeError","Fieldset metadata requires a genuine HTML fieldset.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child fieldset metadata requires its lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn fieldset_get(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_fieldset(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=match method{
   NAME_GET=>self.element_attribute_string_get(object,"name")?,
   FORM=>{self.state.rebuild(self.doc)?;self.control_form_owner_get(object)?},
   TYPE=>{if self.state.limits.host.string_bytes<8{return Err("Fieldset type exceeds the host string allowance.".into());}self.state.copies(8)?;Value::String(script_clone_node::copy_text("fieldset")?)},
   _=>return Err("Unknown fieldset metadata operation.".into())
  };
  script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn fieldset_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_fieldset(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(4)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("name")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}
  else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
