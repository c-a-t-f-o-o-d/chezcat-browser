//! Canonical native textarea defaultValue shares checked textContent replacement.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-textarea-defaultvalue
use super::*;
pub(super) const GET:&str="\0HTMLTextAreaElement.get.defaultValue";
pub(super) const SET:&str="\0HTMLTextAreaElement.set.defaultValue";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn textarea_default_value_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTextAreaElement")}
 pub(super) fn textarea_default_value_member(&self,object:u32,property:&str)->bool{property=="defaultValue"&&self.textarea_default_value_receiver(object)}
 pub(super) fn validate_textarea_default_value(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.textarea_default_value_receiver(object){return Err(("TypeError","Textarea defaultValue requires a genuine HTML textarea element.".into()));}self.native_node_access(object)?;
  if self.is_frame_handle(object)||!node_index(object).is_some_and(|node|self.owned_native_textarea(node)){return Err(("NotSupportedError","Textarea defaultValue requires admitted native default state.".into()));}Ok(())
 }
 pub(super) fn textarea_default_value_get(&mut self,object:u32)->Result<Value,String>{self.validate_textarea_default_value(object).map_err(|(_,s)|s)?;let node=self.node(object)?;Ok(Self::dom_string_owned(self.text_content(node)?))}
 pub(super) fn textarea_default_value_set(&mut self,object:u32,value:&Value)->Result<(),String>{self.validate_textarea_default_value(object).map_err(|(_,s)|s)?;script_text_content::deadline(self.state)?;let text=self.dom_text(value)?;let node=self.node(object)?;self.set_text_content(node,text)}
 pub(super) fn prepare_textarea_default_content(&mut self,node:usize,text:&DomString)->Result<Option<forms::TextareaDefaultPlan>,String>{
  if self.doc.nodes[node].namespace!=crate::html::Namespace::Html||tag(self.doc,node)!="textarea"{return Ok(None);}
  if !self.owned_native_textarea(node){return Err("Textarea default requires admitted native default state.".into());}
  if text.raw_units().is_some(){return Err("Native textarea defaults cannot represent unpaired UTF-16 units.".into());}
  self.state.work(self.forms.default_scan_work().saturating_add(text.len().saturating_mul(8)).saturating_add(self.forms.controls.len()))?;
  self.state.copies(text.len().checked_mul(5).ok_or("Textarea default copy work overflow.")?)?;
  let available=script_text_content::available(self.state,text.capacity())?;
  let plan=self.forms.prepare_textarea_default(node,text.as_str(),available)?;script_text_content::deadline(self.state)?;Ok(Some(plan))
 }
}
