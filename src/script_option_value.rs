//! Canonical Option value with exact attributes and HTML-aware text fallback.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-option-value
use super::*;
pub(super) const GET:&str="\0HTMLOptionElement.get.value";
pub(super) const SET:&str="\0HTMLOptionElement.set.value";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn prepare_option_value(&mut self,node:usize,name:&crate::html::AttributeName,value:Option<&DomString>)->Result<Option<forms::OptionValuePlan>,String>{
  if !script_attr::html_option_value_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),name){return Ok(None);}
  self.state.work(self.forms.controls.len())?;let mut target=None;
  for (&select,control) in &self.forms.controls{if let Some(state)=&control.select{
   self.state.work(state.options.len())?;
   for (option,entry) in state.options.iter().enumerate(){if entry.node==node{target=Some((select,option));}}
  }}
  script_text_content::deadline(self.state)?;let Some((select,option))=target else{return Ok(None);};
  let fallback=if value.is_none(){Some(self.html_aware_text_content(node_handle(node))?)}else{None};
  let text=value.or(fallback.as_ref()).map_or("",DomString::as_str);
  self.state.work(text.len())?;
  if text.encode_utf16().count()>forms::MAX_VALUE_UNITS{return Err("Native OPTION value exceeds the 4096 UTF-16 unit limit; no value was changed.".into());}
  self.state.work(self.forms.default_scan_work())?;
  let control=&self.forms.controls[&select];let state=control.select.as_ref().unwrap();let selected=state.selected==Some(option);
  self.forms.check_option_value_payload(select,option,text.len())?;
  let count=if selected{text.chars().count()}else{0};let editor_bytes=count.checked_mul(size_of::<char>()).ok_or("Native OPTION editor overflow.")?;
  let copies=text.len().checked_add(editor_bytes).ok_or("Native OPTION value copy overflow.")?;
  self.state.copies(copies)?;
  let scratch=fallback.as_ref().map_or(0,DomString::capacity);
  let available=self.state.retained_limit.checked_sub(self.state.retained).and_then(|n|n.checked_sub(scratch)).ok_or("Native OPTION value retained allowance exhausted.")?;
  if copies>available{return Err("Native OPTION value retained allowance exhausted.".into());}
  let value=script_clone_node::copy_text(text)?;
  let editor=if selected{
   let mut chars=Vec::new();chars.try_reserve_exact(count).map_err(|_|"Cannot reserve native OPTION editor.")?;
   if chars.capacity().checked_mul(size_of::<char>()).and_then(|n|n.checked_add(value.capacity())).is_none_or(|bytes|bytes>available){return Err("Native OPTION value retained allowance exhausted.".into());}
   chars.extend(text.chars());Some(crate::editor::Editor{chars,cursor:count,anchor:count,offset:0})
  }else{if value.capacity()>available{return Err("Native OPTION value retained allowance exhausted.".into());}None};
  script_text_content::deadline(self.state)?;Ok(Some(forms::OptionValuePlan{select,option,value,editor}))
 }
 pub(super) fn option_value_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLOptionElement")}
 pub(super) fn option_value_member(&self,object:u32,property:&str)->bool{property=="value"&&self.option_value_receiver(object)}
 pub(super) fn validate_option_value(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.option_value_receiver(object){return Err(("TypeError","Option value metadata requires a genuine HTML option element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child option value requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn option_value_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_option_value(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  if let Some(value)=self.element_optional_attribute_string_get(object,"value")?{script_text_content::deadline(self.state)?;return Ok(value);}
  let text=self.html_aware_text_content(object)?;Ok(Self::dom_string_owned(text))
 }
 pub(super) fn option_value_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_option_value(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("value")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
