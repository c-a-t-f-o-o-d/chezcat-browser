//! Canonical Option label with exact attributes and HTML-aware text fallback.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-option-label
use super::*;
pub(super) const GET:&str="\0HTMLOptionElement.get.label";
pub(super) const SET:&str="\0HTMLOptionElement.set.label";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
pub(super) struct LabelPlan{select:usize,option:usize,label:String,caption:String}
impl LabelPlan{
 pub(super) fn commit(self,forms:&mut Forms){
  let control=forms.controls.get_mut(&self.select).expect("Prepared SELECT option owner");
  control.select.as_mut().expect("Prepared SELECT state").options[self.option].label=self.label;
  control.caption=self.caption;
 }
}
impl Bridge<'_>{
 pub(super) fn prepare_option_label(&mut self,node:usize,name:&crate::html::AttributeName,value:Option<&DomString>)->Result<Option<LabelPlan>,String>{
  if !script_attr::html_option_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),name){return Ok(None);}
  self.state.work(self.forms.controls.len())?;let mut target=None;
  for (&select,control) in &self.forms.controls{if let Some(state)=&control.select{
   self.state.work(state.options.len())?;
   for (option,entry) in state.options.iter().enumerate(){if entry.node==node{target=Some((select,option));}}
  }}
  script_text_content::deadline(self.state)?;let Some((select,option))=target else{return Ok(None);};
  // The IDL returns a present empty attribute; the display label falls back
  // for both empty and missing attributes, preserving existing SELECT bounds.
  let fallback=if value.is_none_or(|v|v.is_empty()){Some(self.html_aware_text_content(node_handle(node))?)}else{None};
  let text=value.filter(|v|!v.is_empty()).or(fallback.as_ref()).map_or("",DomString::as_str);
  let end=text.char_indices().nth(256).map_or(text.len(),|(offset,_)|offset);let text=&text[..end];
  self.state.work(self.forms.default_scan_work())?;
  let state=self.forms.controls[&select].select.as_ref().unwrap();
  self.forms.check_select_label_payload(state.options[option].label.len(),text.len())?;
  // The control paints its currently selected option, independent of the
  // longest-label sizing helper. An unselected label must not replace it.
  let caption=state.selected.and_then(|selected|state.options.get(selected).map(|entry|if selected==option{text}else{entry.label.as_str()})).unwrap_or("");
  let copies=text.len().checked_add(caption.len()).ok_or("SELECT option label copy overflow.")?;
  self.state.copies(copies)?;
  let scratch=fallback.as_ref().map_or(0,DomString::capacity);
  if self.state.retained.checked_add(scratch).and_then(|n|n.checked_add(copies)).is_none_or(|bytes|bytes>self.state.retained_limit){return Err("SELECT option label retained allowance exhausted.".into());}
  let label=script_clone_node::copy_text(text)?;let caption=script_clone_node::copy_text(caption)?;
  script_text_content::deadline(self.state)?;Ok(Some(LabelPlan{select,option,label,caption}))
 }
 pub(super) fn option_label_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLOptionElement")}
 pub(super) fn option_label_member(&self,object:u32,property:&str)->bool{property=="label"&&self.option_label_receiver(object)}
 pub(super) fn validate_option_label(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.option_label_receiver(object){return Err(("TypeError","Option label metadata requires a genuine HTML option element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child option metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn option_label_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_option_label(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  if let Some(value)=self.element_optional_attribute_string_get(object,"label")?{script_text_content::deadline(self.state)?;return Ok(value);}
  let text=self.html_aware_text_content(object)?;Ok(Self::dom_string_owned(text))
 }
 pub(super) fn option_label_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_option_label(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("label")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
