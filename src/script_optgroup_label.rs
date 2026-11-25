//! Canonical OPTGROUP label DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-optgroup-label
use super::*;
pub(super) const GET:&str="\0HTMLOptGroupElement.get.label";
pub(super) const SET:&str="\0HTMLOptGroupElement.set.label";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
pub(super) struct LabelPlan{select:usize,row:usize,label:String}
impl LabelPlan{
 pub(super) fn commit(self,forms:&mut Forms){
  let state=forms.controls.get_mut(&self.select).and_then(|c|c.select.as_mut()).expect("Prepared SELECT group identity");
  let forms::SelectRow::Group{label,..}=&mut state.rows[self.row] else{unreachable!("Prepared SELECT group row")};*label=self.label;
 }
}
impl Bridge<'_>{
 pub(super) fn prepare_optgroup_label(&mut self,node:usize,name:&crate::html::AttributeName,value:Option<&DomString>)->Result<Option<LabelPlan>,String>{
  if !script_attr::html_optgroup_label_attribute(self.doc.nodes[node].namespace,tag(self.doc,node),name){return Ok(None);}
  self.state.work(self.forms.controls.len())?;
  let mut target=None;
  for (&select,control) in &self.forms.controls{if let Some(state)=&control.select{
   self.state.work(state.rows.len())?;
   for (row,entry) in state.rows.iter().enumerate(){if matches!(entry,forms::SelectRow::Group{node:group,..} if *group==node){target=Some((select,row));}}
  }}
  script_text_content::deadline(self.state)?;
  let Some((select,row))=target else{return Ok(None);};
  // Keep the existing native SELECT display bound; the DOM value stays exact.
  let text=value.map_or("",DomString::as_str);let end=text.char_indices().nth(256).map_or(text.len(),|(offset,_)|offset);let text=&text[..end];
  self.state.work(self.forms.default_scan_work())?;
  let forms::SelectRow::Group{label:old,..}=&self.forms.controls[&select].select.as_ref().unwrap().rows[row] else{unreachable!()};
  self.forms.check_select_label_payload(old.len(),text.len())?;
  self.state.copies(text.len())?;
  if self.state.retained.checked_add(text.len()).is_none_or(|bytes|bytes>self.state.retained_limit){return Err("SELECT group label retained allowance exhausted.".into());}
  // Existing native rows use their finite Forms payload bound. Do not debit
  // uncharged parser storage or accumulate a DOM charge on every replacement.
  let label=script_clone_node::copy_text(text)?;script_text_content::deadline(self.state)?;
  Ok(Some(LabelPlan{select,row,label}))
 }
 pub(super) fn optgroup_label_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLOptGroupElement")}
 pub(super) fn optgroup_label_member(&self,object:u32,property:&str)->bool{property=="label"&&self.optgroup_label_receiver(object)}
 pub(super) fn validate_optgroup_label(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.optgroup_label_receiver(object){return Err(("TypeError","OptGroup label metadata requires a genuine HTML optgroup element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child optgroup metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn optgroup_label_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_optgroup_label(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"label")?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn optgroup_label_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_optgroup_label(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("label")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
