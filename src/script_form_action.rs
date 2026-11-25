//! Canonical FORM action USVString URL reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-fs-action
use super::*;
pub(super) const GET:&str="\0HTMLFormElement.get.action";
pub(super) const SET:&str="\0HTMLFormElement.set.action";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn form_action_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLFormElement")}
 pub(super) fn form_action_member(&self,object:u32,property:&str)->bool{property=="action"&&self.form_action_receiver(object)&&!self.form_named_control(object,property)}
 pub(super) fn validate_form_action(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.form_action_receiver(object){return Err(("TypeError","Form action requires a genuine HTML form element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form action requires its lifecycle hooks.".into()));}Ok(())
 }

 fn form_action_usv(&mut self,value:&Value)->Result<String,String>{
  let Value::StringUnits(units)=value else{return self.text(value)};
  self.state.work(units.len().checked_mul(2).ok_or("Form action scalar work overflow.")?)?;
  let(bytes,_)=DomString::projected_size(units)?;
  if bytes>self.state.limits.host.string_bytes{return Err("Form action exceeds the host string allowance.".into());}
  self.state.copies(bytes)?;let mut text=String::new();text.try_reserve_exact(bytes).map_err(|_|"Cannot reserve form action scalar string.")?;
  text.extend(char::decode_utf16(units.iter().copied()).map(|v|v.unwrap_or(char::REPLACEMENT_CHARACTER)));Ok(text)
 }
 pub(super) fn form_action_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_form_action(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"action")?;let raw=self.form_action_usv(&value)?;
  if raw.is_empty(){return if self.inert_node_type(object).is_some(){self.inert_node_document_url(object)}else{self.location_property("href")};}
  let Value::String(base)=self.node_base_uri_get(object)? else{return Err("Form action requires a scalar document base URL.".into())};
  let cost=dom_compat::url_copy_cost(&raw,None)?.checked_add(base.len().checked_mul(20).ok_or("Form action URL accounting overflow.")?).ok_or("Form action URL accounting overflow.")?;
  self.state.work(cost)?;self.state.copies(cost)?;
  // Reuse lexical local reflection only for a native document with its actual
  // Location record. This path does not check existence or grant file access.
  let local=raw.trim_matches(|c|c<='\u{20}').split_once(':').is_none_or(|(scheme,_)|scheme.eq_ignore_ascii_case("file"));
  let resolved=if self.inert_node_type(object).is_none()&&base.starts_with("file:")&&local{
   let location=self.state.dynamic.base.as_ref().or_else(||self.state.location.as_ref().map(|v|&v.source)).ok_or("Missing form action base.")?;
   crate::navigation::selection_url(&raw,location).ok()
  }else{crate::javascript::anchor_url_project(&raw,Some(&base),"href")?};
  script_text_content::deadline(self.state)?;self.string(resolved.as_deref().unwrap_or(&raw))
 }
 pub(super) fn form_action_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_form_action(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.form_action_usv(value)?;self.state.copies(6)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("action")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value.into()))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value.into()),script_attr::InputPreparation::Pending)}
 }
}
