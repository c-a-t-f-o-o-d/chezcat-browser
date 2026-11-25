//! OUTPUT reset follows current native form ownership after reset listeners.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-output-element
use super::*;
impl Bridge<'_>{
 pub(super) fn form_reset_output_owned(&mut self,owner:u32,node:usize)->Result<bool,String>{
  self.state.rebuild(self.doc)?;
  let object=node_handle(node);
  self.native_node_access(object).map_err(|(_,message)|message)?;
  Ok(matches!(self.control_form_owner_get(object)?,Value::Host(form)if form==owner))
 }
 pub(super) fn form_reset_outputs(&mut self,owner:usize)->Result<Vec<usize>,String>{
  self.state.rebuild(self.doc)?;
  if self.doc.nodes.get(owner).is_none_or(|node|node.namespace!=crate::html::Namespace::Html)||tag(self.doc,owner)!="form"{return Err("Reset requires a genuine HTML form.".into());}
  self.native_node_access(node_handle(owner)).map_err(|(_,message)|message)?;
  let mut pending=Vec::new();let mut outputs=Vec::new();
  let mut root=owner;let mut steps=0usize;
  while let Some(parent)=self.state.tree.parents.get(root).copied().flatten(){
   self.state.work(1)?;script_text_content::deadline(self.state)?;
   steps=steps.checked_add(1).filter(|&n|n<=self.doc.nodes.len()).ok_or("Invalid reset root ancestry.")?;root=parent;
  }
  script_text_replacement::push(self.state,&mut pending,root,0)?;
  while let Some(node)=pending.pop(){
   self.state.work(1)?;script_text_content::deadline(self.state)?;
   let other=script_text_replacement::bytes(&[&outputs],0)?;
   for &child in self.doc.nodes[node].children.iter().rev(){script_text_replacement::push(self.state,&mut pending,child,other)?;}
   let object=node_handle(node);
   if !self.html_output_receiver(object){continue;}
   let scratch=script_text_replacement::bytes(&[&pending,&outputs],0)?;
   self.state.replace_storage(0,scratch,0)?;
   let association=self.control_form_owner_get(object);
   self.state.retained-=scratch;
   if matches!(association?,Value::Host(form)if form==node_handle(owner)){
    let other=script_text_replacement::bytes(&[&pending],0)?;
    script_text_replacement::push(self.state,&mut outputs,node,other)?;
   }
  }
  Ok(outputs)
 }
}
pub(crate) fn reset_form(doc:&mut Document,forms:&mut Forms,page:Option<&mut Page>,owner:usize)->Result<Outcome,String>{
 if let Some(page)=page{
  let outputs=Bridge{doc,forms,state:&mut page.state}.form_reset_outputs(owner)?;
  let scratch=script_text_replacement::bytes(&[&outputs],0)?;
  page.state.replace_storage(0,scratch,0)?;
  let result=(||{
   for &node in &outputs{
    let owned=Bridge{doc,forms,state:&mut page.state}.form_reset_output_owned(node_handle(owner),node)?;
    if owned{page.runtime.reset_native_output(node_handle(node),&mut Bridge{doc,forms,state:&mut page.state})?;}
   }
   forms.reset_in_document(doc,owner)
  })();
  page.state.retained-=scratch;
  let before=page.state.errors;
  page.microtask_checkpoint(doc,forms);
  result?;
  Ok(Outcome{prevented:false,layout_changed:std::mem::take(&mut page.state.changed),errors:page.state.errors.saturating_sub(before)})
 }else{
  // Native defaults also reset when scripting is disabled. This short-lived
  // host owns only bounded transaction metadata, with no interpreter or jobs.
  let mut state=State::new();state.retained=document_bytes(doc);
  state.retained_limit=state.retained.checked_add(state.limits.host.retained_bytes).ok_or("Native reset storage overflow.")?;
  state.node_limit=doc.nodes.len().checked_add(state.limits.host.extra_nodes).ok_or("Native reset node limit overflow.")?;
  state.execution_deadline=Instant::now().checked_add(Duration::from_secs(state.limits.job_seconds));
  let mut bridge=Bridge{doc,forms,state:&mut state};
  let outputs=bridge.form_reset_outputs(owner)?;
  let scratch=script_text_replacement::bytes(&[&outputs],0)?;
  bridge.state.replace_storage(0,scratch,0)?;
  for &node in &outputs{
   if !bridge.form_reset_output_owned(node_handle(owner),node)?{continue;}
   let object=node_handle(node);let value=bridge.output_value_get(object,"defaultValue")?;bridge.reset_output_value(object,value)?;
  }
  bridge.forms.reset_in_document(bridge.doc,owner)?;
  Ok(Outcome{layout_changed:state.changed,..Outcome::default()})
 }
}
