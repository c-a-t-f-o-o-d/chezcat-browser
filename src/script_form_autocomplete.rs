//! Canonical FORM autocomplete DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-autocomplete
use super::*;
pub(super) const GET:&str="\0HTMLFormElement.get.autocomplete";
pub(super) const SET:&str="\0HTMLFormElement.set.autocomplete";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn form_autocomplete_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLFormElement")}
 pub(super) fn form_autocomplete_member(&self,object:u32,property:&str)->bool{property=="autocomplete"&&self.form_autocomplete_receiver(object)&&!self.form_named_control(object,property)}
 pub(super) fn validate_form_autocomplete(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.form_autocomplete_receiver(object){return Err(("TypeError","Form autocomplete requires a genuine HTML form element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form autocomplete requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn form_autocomplete_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_form_autocomplete(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"autocomplete")?;
  let off=match &value{Value::String(s)=>{self.state.work(s.len().saturating_add(1))?;s.eq_ignore_ascii_case("off")},Value::StringUnits(u)=>{self.state.work(u.len().saturating_add(1))?;u.iter().copied().map(|u|if(0x41..=0x5a).contains(&u){u+0x20}else{u}).eq("off".encode_utf16())},_=>unreachable!()};
  let word=if off{"off"}else{"on"};if word.len()>self.state.limits.host.string_bytes{return Err("Form autocomplete exceeds the host string allowance.".into());}
  self.state.copies(word.len())?;script_text_content::deadline(self.state)?;Ok(Value::String(script_clone_node::copy_text(word)?))
 }
 pub(super) fn form_autocomplete_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_form_autocomplete(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(13)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("autocomplete")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
