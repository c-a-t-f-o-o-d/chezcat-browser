//! Canonical Option form follows the qualifying SELECT ancestor's real owner.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-option-form
use super::*;
pub(super) const GET:&str="\0HTMLOptionElement.get.form";
impl Bridge<'_>{
 pub(super) fn option_form_member(&self,object:u32,property:&str)->bool{property=="form"&&self.option_disabled_receiver(object)}
 pub(super) fn validate_option_form(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.option_disabled_receiver(object){return Err(("TypeError","Option form requires a genuine HTML option element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child option form requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn option_form_facts(&self,object:u32)->Option<(crate::html::Namespace,&str)>{
  if let Some(source)=self.inert_import_source(object){return match source{
   script_inert_documents::ImportSource::Element(tag)=>Some((crate::html::Namespace::Html,tag)),
   script_inert_documents::ImportSource::Parsed{node,..}=>match &node.kind{NodeKind::Element{tag,..}=>Some((node.namespace,tag)),_=>None},_=>None,
  };}
  if self.inert_node_type(object).is_some(){return None;}
  let node=self.doc.nodes.get(node_index(object)?)?;match &node.kind{NodeKind::Element{tag,..}=>Some((node.namespace,tag)),_=>None}
 }
 pub(super) fn option_form_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_option_form(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;self.state.rebuild(self.doc)?;
  let Some(select)=self.option_nearest_select(object)?else{return Ok(Value::Null);};
  self.control_form_get(select,"\0HTMLSelectElement.get.form")
 }
 // Callers validate the genuine option, origin and rebuilt owner tree first.
 pub(super) fn option_nearest_select(&mut self,object:u32)->Result<Option<u32>,String>{
  let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(1)).ok_or("Option form tree bound overflow.")?;
  let lookup=((usize::BITS-self.inert_node_count().leading_zeros())as usize).checked_add(9).and_then(|n|n.checked_mul(8)).ok_or("Option form lookup work overflow.")?;
  let mut current=object;let mut seen_group=false;let mut visited=0usize;
  loop{
   visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic option form ancestry.")?;self.state.work(lookup)?;script_text_content::deadline(self.state)?;
   let Some(parent)=self.whole_text_parent(current)?else{return Ok(None);};
   if parent==DOCUMENT||self.inert_node_type(parent)==Some(9){return Ok(None);}
   self.native_node_access(parent).map_err(|(_,message)|message)?;
   let work=self.option_form_facts(parent).map_or(0,|(_,tag)|tag.len());self.state.work(work.saturating_add(1))?;
   if let Some((crate::html::Namespace::Html,tag))=self.option_form_facts(parent){match tag{
    "datalist"|"hr"|"option"=>return Ok(None),
    "optgroup"=>{if seen_group{return Ok(None);}seen_group=true;},
    "select"=>return Ok(Some(parent)),_=>{},
   }}
   current=parent;
  }
 }
}
