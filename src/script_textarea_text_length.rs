//! Native TEXTAREA textLength over its actual normalized API value.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-textarea-textlength
use super::*;
pub(super) const GET:&str="\0HTMLTextAreaElement.get.textLength";
impl Bridge<'_>{
 pub(super) fn textarea_text_length_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTextAreaElement")}
 pub(super) fn textarea_text_length_member(&self,object:u32,property:&str)->bool{property=="textLength"&&self.textarea_text_length_receiver(object)}
 pub(super) fn validate_textarea_text_length(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.textarea_text_length_receiver(object){return Err(("TypeError","Textarea textLength requires a genuine HTML textarea element.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object)||!node_index(object).is_some_and(|node|self.owned_native_textarea(node)){return Err(("NotSupportedError","Textarea textLength requires admitted native value state.".into()));}Ok(())
 }
 pub(super) fn textarea_text_length_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_textarea_text_length(object).map_err(|(_,message)|message)?;script_text_content::deadline(self.state)?;
  self.state.work(self.forms.controls.len().saturating_add(1))?;let node=self.node(object)?;let count=self.forms.controls[&node].value.chars.len();self.state.work(count)?;
  // Native textarea editors store the API value with LF newlines. Counting
  // UTF-16 units directly avoids constructing or copying its string value.
  let length=self.forms.controls[&node].value.chars.iter().try_fold(0u32,|length,ch|length.checked_add(ch.len_utf16() as u32)).ok_or("Textarea textLength exceeds its unsigned bound.")?;
  script_text_content::deadline(self.state)?;Ok(Value::Number(f64::from(length)))
 }
}
