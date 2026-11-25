//! Canonical INPUT name DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-fe-name
use super::*;
pub(super) const GET:&str="\0HTMLInputElement.get.name";
pub(super) const SET:&str="\0HTMLInputElement.set.name";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn input_name_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLInputElement")}
 pub(super) fn input_name_member(&self,object:u32,property:&str)->bool{property=="name"&&self.input_name_receiver(object)}
 pub(super) fn validate_input_name(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.input_name_receiver(object){return Err(("TypeError","Input name requires a genuine HTML input element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child form name requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn input_name_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_input_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_optional_attribute_string_get(object,"name")?.unwrap_or_else(||Value::String(String::new()));script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn input_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_input_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(4)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("name")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}

impl Bridge<'_>{
 pub(super) fn prepare_input_control_name(&mut self,node:usize,name:&crate::html::AttributeName,value:Option<&DomString>)->Result<Option<forms::ControlNamePlan>,String>{
  if self.doc.nodes[node].namespace!=crate::html::Namespace::Html||tag(self.doc,node)!="input"||!name.html_is("name"){return Ok(None);}
  let Some(control)=self.forms.controls.get(&node)else{return Ok(None);};
  // Text and checkable INPUT names retain their established plans, including
  // radio-group synchronization. Other bound INPUT kinds need a name plan too.
  if matches!(control.kind,Kind::Text|Kind::Checkbox|Kind::Radio){return Ok(None);}
  self.state.work(self.forms.controls.len().saturating_add(1))?;self.state.work(self.forms.default_scan_work())?;
  let text=value.map_or("",DomString::as_str);self.forms.check_control_name_payload(node,text)?;self.state.copies(text.len())?;
  let available=self.state.retained_limit.saturating_sub(self.state.retained);if text.len()>available{return Err("Native INPUT name retained allowance exhausted.".into());}
  let name=script_clone_node::copy_text(text)?;if name.capacity()>available{return Err("Native INPUT name retained allowance exhausted.".into());}
  script_text_content::deadline(self.state)?;Ok(Some(forms::ControlNamePlan{node,name}))
 }
}
