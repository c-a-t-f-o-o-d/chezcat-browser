//! Canonical Option defaultSelected boolean reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-option-defaultselected
use super::*;
pub(super) const GET:&str="\0HTMLOptionElement.get.defaultSelected";
pub(super) const SET:&str="\0HTMLOptionElement.set.defaultSelected";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn option_default_selected_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLOptionElement")}
 pub(super) fn option_default_selected_member(&self,object:u32,property:&str)->bool{property=="defaultSelected"&&self.option_default_selected_receiver(object)}
 pub(super) fn validate_option_default_selected(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.option_default_selected_receiver(object){return Err(("TypeError","Option defaultSelected metadata requires a genuine HTML option element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child option metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn option_default_selected_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_option_default_selected(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("Option attribute source is unavailable.")?.len();self.state.work(count)?;let mut present=false;for at in 0..count{let name=&self.element_attribute_source(object).unwrap()[at].name;let bytes=name.bytes();let matched=name.html_is("selected");self.state.work(bytes.saturating_add(1))?;if matched{present=true;break;}}script_text_content::deadline(self.state)?;Ok(Value::Bool(present))
 }
 pub(super) fn option_default_selected_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_option_default_selected(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.state.copies(8)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("selected")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,value.truthy().then(DomString::new))}else{let node=self.node(object)?;self.public_attribute(node,name,value.truthy().then(String::new))}
 }
}

impl Bridge<'_>{
 pub(super) fn prepare_option_default_selected(&mut self,node:usize,name:&crate::html::AttributeName,value:Option<&DomString>)->Result<Option<forms::OptionDefaultPlan>,String>{
  if !script_attr::html_option_default_selected_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),name){return Ok(None);}
  self.state.work(self.forms.controls.len())?;let mut target=None;
  for (&select,control) in &self.forms.controls{if let Some(state)=&control.select{self.state.work(state.options.len())?;for(option,entry)in state.options.iter().enumerate(){if entry.node==node{target=Some((select,option));}}}}
  script_text_content::deadline(self.state)?;let Some((select,option))=target else{return Ok(None);};
  self.state.work(self.forms.default_scan_work())?;
  let present=value.is_some();let control=&self.forms.controls[&select];let state=control.select.as_ref().unwrap();
  let selected=if state.options[option].dirty_selected||state.options[option].default_selected==present{state.selected}else if present{Some(option)}else if state.selected==Some(option){state.options.iter().position(|entry|!entry.disabled)}else{state.selected.or_else(||state.options.iter().position(|entry|!entry.disabled))};
  let replacement=if selected!=state.selected{
   self.forms.check_select_selection_payload(select,selected)?;
   let entry=selected.map(|index|&state.options[index]);let label=entry.map_or("",|entry|entry.label.as_str());let text=entry.map_or("",|entry|entry.value.as_str());
   let count=text.chars().count();let bytes=count.checked_mul(size_of::<char>()).and_then(|n|n.checked_add(label.len())).ok_or("Native OPTION selection payload overflow.")?;
   self.state.copies(bytes)?;let available=self.state.retained_limit.checked_sub(self.state.retained).ok_or("Native OPTION selection retained allowance exhausted.")?;
   if bytes>available{return Err("Native OPTION selection retained allowance exhausted.".into());}
   let caption=script_clone_node::copy_text(label)?;let mut chars=Vec::new();chars.try_reserve_exact(count).map_err(|_|"Cannot reserve native OPTION selection editor.")?;
   if chars.capacity().checked_mul(size_of::<char>()).and_then(|n|n.checked_add(caption.capacity())).is_none_or(|n|n>available){return Err("Native OPTION selection retained allowance exhausted.".into());}
   chars.extend(text.chars());Some((selected,caption,crate::editor::Editor{chars,cursor:count,anchor:count,offset:0}))
  }else{None};
  script_text_content::deadline(self.state)?;Ok(Some(forms::OptionDefaultPlan{select,option,present,replacement}))
 }
}
