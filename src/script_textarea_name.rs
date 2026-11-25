//! Canonical TEXTAREA name with exact DOMString attributes and native form names.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-fe-name
use super::*;
pub(super) const GET:&str="\0HTMLTextAreaElement.get.name";
pub(super) const SET:&str="\0HTMLTextAreaElement.set.name";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn textarea_name_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTextAreaElement")}
 pub(super) fn textarea_name_member(&self,object:u32,property:&str)->bool{property=="name"&&self.textarea_name_receiver(object)}
 pub(super) fn validate_textarea_name(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.textarea_name_receiver(object){return Err(("TypeError","Textarea name requires a genuine HTML textarea element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child textarea name requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn textarea_name_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_textarea_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_optional_attribute_string_get(object,"name")?.unwrap_or_else(||Value::String(String::new()));script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn textarea_name_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_textarea_name(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(4)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("name")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
 pub(super) fn prepare_textarea_name(&mut self,node:usize,name:&crate::html::AttributeName,value:Option<&DomString>)->Result<Option<forms::ControlNamePlan>,String>{
  if !script_attr::html_textarea_name_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),name){return Ok(None);}
  self.state.work(self.forms.controls.len().saturating_add(1))?;if !self.forms.controls.contains_key(&node){return Ok(None);}
  self.state.work(self.forms.default_scan_work())?;let text=value.map_or("",DomString::as_str);
  self.forms.check_control_name_payload(node,text)?;self.state.copies(text.len())?;
  let available=self.state.retained_limit.saturating_sub(self.state.retained);if text.len()>available{return Err("Native TEXTAREA name retained allowance exhausted.".into());}
  let name=script_clone_node::copy_text(text)?;if name.capacity()>available{return Err("Native TEXTAREA name retained allowance exhausted.".into());}
  script_text_content::deadline(self.state)?;Ok(Some(forms::ControlNamePlan{node,name}))
 }
}
