//! Canonical Form novalidate boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/forms.html#attr-form-novalidate
use super::*;
pub(super) const GET:&str="\0HTMLFormElement.get.noValidate";
pub(super) const SET:&str="\0HTMLFormElement.set.noValidate";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn form_no_validate_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLFormElement")}
 pub(super) fn form_no_validate_member(&self,object:u32,property:&str)->bool{property=="noValidate"&&self.form_no_validate_receiver(object)&&!self.form_named_control(object,property)}
 pub(super) fn validate_form_no_validate(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.form_no_validate_receiver(object){return Err(("TypeError","Form novalidate metadata requires a genuine HTML form element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn form_no_validate_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_form_no_validate(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("Form attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("novalidate");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn form_no_validate_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_form_no_validate(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(10)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("novalidate")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}
