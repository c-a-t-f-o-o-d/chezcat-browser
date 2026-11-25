//! Canonical Option text uses HTML-aware reads and real child replacement.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-option-text
use super::*;
pub(super) const GET:&str="\0HTMLOptionElement.get.text";
pub(super) const SET:&str="\0HTMLOptionElement.set.text";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn option_text_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLOptionElement")}
 pub(super) fn option_text_member(&self,object:u32,property:&str)->bool{property=="text"&&self.option_text_receiver(object)}
 pub(super) fn validate_option_text(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.option_text_receiver(object){return Err(("TypeError","Option text requires a genuine HTML option element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child option text requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn option_text_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_option_text(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let text=self.html_aware_text_content(object)?;Ok(Self::dom_string_owned(text))
 }
 pub(super) fn option_text_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_option_text(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let text=self.dom_text(value)?;
  if self.inert_node_type(object)==Some(1){self.replace_inert_text_content(object,text)}else{let node=self.node(object)?;self.set_text_content(node,text)}
 }
 pub(super) fn prepare_option_content(&mut self,node:usize,text:&DomString)->Result<Option<forms::OptionContentPlan>,String>{
  if self.doc.nodes[node].namespace!=crate::html::Namespace::Html || tag(self.doc,node)!="option"{return Ok(None);}
  self.state.work(self.forms.controls.len())?;let mut target=None;
  for (&select,c) in &self.forms.controls{if let Some(state)=&c.select{self.state.work(state.options.len())?;for(index,entry)in state.options.iter().enumerate(){if entry.node==node{target=Some((select,index));}}}}
  let Some((select,option))=target else{return Ok(None);};
  if let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind{self.state.work(attrs.iter().fold(attrs.len(),|n,a|n.saturating_add(a.name.bytes())))?;}
  let change_value=self.doc.dom_attr(node,"value").is_none();let change_label=self.doc.dom_attr(node,"label").is_none_or(|v|v.is_empty());
  if !change_value&&!change_label{return Ok(None);}
  let normalized=self.html_aware_text_value(text)?;let value_text=normalized.as_str();
  if change_value&&value_text.encode_utf16().count()>forms::MAX_VALUE_UNITS{return Err("Native OPTION text value exceeds the 4096 UTF-16 unit limit; no content was changed.".into());}
  let end=value_text.char_indices().nth(256).map_or(value_text.len(),|(at,_)|at);let label_text=&value_text[..end];
  self.state.work(self.forms.default_scan_work())?;
  self.forms.check_option_content_payload(select,option,change_value.then_some(value_text.len()),change_label.then_some(label_text.len()))?;
  let selected=self.forms.controls[&select].select.as_ref().unwrap().selected==Some(option);
  let count=if selected&&change_value{value_text.chars().count()}else{0};let editor_bytes=count.checked_mul(size_of::<char>()).ok_or("OPTION content editor overflow.")?;
  let copies=(if change_value{value_text.len()}else{0usize}).checked_add(if change_label{label_text.len()}else{0}).and_then(|n|n.checked_add(if change_label&&selected{label_text.len()}else{0})).and_then(|n|n.checked_add(editor_bytes)).ok_or("OPTION content copy overflow.")?;
  self.state.copies(copies)?;
  let available=self.state.retained_limit.checked_sub(self.state.retained).and_then(|n|n.checked_sub(text.capacity())).and_then(|n|n.checked_sub(normalized.capacity())).ok_or("OPTION content scratch allowance exhausted.")?;
  if copies>available{return Err("OPTION content scratch allowance exhausted.".into());}
  let value=if change_value{Some(script_clone_node::copy_text(value_text)?)}else{None};
  let label=if change_label{Some(script_clone_node::copy_text(label_text)?)}else{None};
  let caption=if change_label&&selected{Some(script_clone_node::copy_text(label_text)?)}else{None};
  let mut bytes=value.as_ref().map_or(0,String::capacity).checked_add(label.as_ref().map_or(0,String::capacity)).and_then(|n|n.checked_add(caption.as_ref().map_or(0,String::capacity))).ok_or("OPTION content storage overflow.")?;
  let editor=if change_value&&selected{
   let mut chars=Vec::new();chars.try_reserve_exact(count).map_err(|_|"Cannot reserve OPTION content editor.")?;
   bytes=bytes.checked_add(chars.capacity().checked_mul(size_of::<char>()).ok_or("OPTION content editor backing overflow.")?).ok_or("OPTION content storage overflow.")?;
   if bytes>available{return Err("OPTION content scratch allowance exhausted.".into());}
   chars.extend(value_text.chars());Some(crate::editor::Editor{chars,cursor:count,anchor:count,offset:0})
  }else{None};
  if bytes>available{return Err("OPTION content scratch allowance exhausted.".into());}script_text_content::deadline(self.state)?;
  Ok(Some(forms::OptionContentPlan{select,option,value,label,caption,editor,bytes}))
 }
}
