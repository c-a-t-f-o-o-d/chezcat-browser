//! OUTPUT current descendant text and its native default-value override.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-output-element
use super::*;
pub(super) const VALUE_GET:&str="\0HTMLOutputElement.get.value";
pub(super) const VALUE_SET:&str="\0HTMLOutputElement.set.value";
pub(super) const DEFAULT_GET:&str="\0HTMLOutputElement.get.defaultValue";
pub(super) const DEFAULT_SET:&str="\0HTMLOutputElement.set.defaultValue";
pub(super) fn operation(method:&str)->Option<(&'static str,bool)>{match method{
 VALUE_GET=>Some(("value",false)),VALUE_SET=>Some(("value",true)),
 DEFAULT_GET=>Some(("defaultValue",false)),DEFAULT_SET=>Some(("defaultValue",true)),_=>None,
}}
impl Bridge<'_>{
 pub(super) fn output_value_member(&self,object:u32,property:&str)->bool{matches!(property,"value"|"defaultValue")&&self.html_output_receiver(object)}
 pub(super) fn validate_output_value(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.html_output_receiver(object){return Err(("TypeError","OUTPUT value requires a genuine HTML output.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child output values require their lifecycle hooks.".into()));}
  Ok(())
 }
 fn output_current_text(&mut self,object:u32)->Result<DomString,String>{
  if self.inert_node_type(object).is_some(){self.inert_text_content_string(object)}
  else{let node=self.dom_node(object)?;self.text_content(node)}
 }
 fn output_replace_text(&mut self,object:u32,text:DomString)->Result<(),String>{
  if self.inert_node_type(object).is_some(){self.replace_inert_text_content(object,text)}
  else{let node=self.dom_node(object)?;self.set_text_content(node,text)}
 }
 pub(super) fn output_value_get(&mut self,object:u32,property:&str)->Result<Value,String>{
  self.validate_output_value(object).map_err(|(_,message)|message)?;
  self.state.work(self.state.output_defaults.lookup_work().checked_add(1).ok_or("OUTPUT default lookup work overflow.")?)?;
  script_text_content::deadline(self.state)?;
  if property=="defaultValue" {
   if let Some(value)=self.state.output_defaults.get(&object){
    if value.logical_bytes()>self.state.limits.host.string_bytes{return Err("OUTPUT default value exceeds the host string allowance.".into());}
    let needed=value.used_bytes();
    let available=script_text_content::available(self.state,0)?;
    if needed>available{return Err("OUTPUT default value copy storage allowance exhausted.".into());}
    self.state.copies(needed.checked_mul(2).ok_or("OUTPUT default copy work overflow.")?)?;
    let copied=self.state.output_defaults.get(&object).unwrap().try_clone()?;
    if copied.capacity()>available{return Err("OUTPUT default value copy storage allowance exhausted.".into());}
    script_text_content::deadline(self.state)?;
    return Ok(Self::dom_string_owned(copied));
   }
  }else if property!="value"{return Err("Unknown OUTPUT value property.".into());}
  Ok(Self::dom_string_owned(self.output_current_text(object)?))
 }
 pub(super) fn output_value_set(&mut self,object:u32,property:&str,value:&Value)->Result<(),String>{
  self.validate_output_value(object).map_err(|(_,message)|message)?;
  self.state.rebuild(self.doc)?;
  self.state.work(self.state.output_defaults.lookup_work().checked_add(1).ok_or("OUTPUT default lookup work overflow.")?)?;
  script_text_content::deadline(self.state)?;
  let text=self.dom_text(value)?;
  let existing=self.state.output_defaults.get(&object).map(DomString::capacity);
  if property=="defaultValue" {
   if let Some(old)=existing {
    let bytes=text.capacity();
    if bytes>script_text_content::available(self.state,0)?{return Err("OUTPUT default replacement storage allowance exhausted.".into());}
    self.state.copies(size_of::<(u32,DomString)>())?;
    script_text_content::deadline(self.state)?;
    self.state.replace_storage(old,bytes,0)?;
    self.state.output_defaults.insert(object,text);
    self.state.dom_committed();
    return Ok(());
   }
   return self.output_replace_text(object,text);
  }
  if property!="value"{return Err("Unknown OUTPUT value property.".into());}
  if existing.is_some(){return self.output_replace_text(object,text);}
  // The incoming replacement and current default coexist during the read.
  let input=text.capacity();
  self.state.replace_storage(0,input,0)?;
  let current=self.output_current_text(object);
  self.state.retained-=input;
  let current=current?;
  let map=&self.state.output_defaults;
  let growing=map.minimum_growth(1)?!=0;
  let peak=if growing{map.storage_bytes()}else{0};
  let available=script_text_content::available(self.state,input.checked_add(current.capacity()).and_then(|bytes|bytes.checked_add(peak)).ok_or("OUTPUT default joint storage overflow.")?)?;
  let minimum=map.minimum_growth(1)?;
  if minimum>available{return Err("OUTPUT default index storage allowance exhausted.".into());}
  let (growth,metadata)=map.prepare(1,available)?;
  let work=map.insert_work(&object);
  let shifted=work.saturating_sub(map.lookup_work());
  let copied=shifted.checked_add(usize::from(growth.is_some())*map.len()).and_then(|n|n.checked_add(1))
   .and_then(|n|n.checked_mul(size_of::<(u32,DomString)>())).ok_or("OUTPUT default index copy overflow.")?;
  self.state.work(work)?;self.state.copies(copied)?;
  script_text_content::deadline(self.state)?;
  let reserved=current.capacity().checked_add(metadata).and_then(|bytes|bytes.checked_add(peak)).ok_or("OUTPUT default retained storage overflow.")?;
  // Reserve the unpublished authority while the shared text transaction runs.
  // Failure leaves both children and the sparse override index unchanged.
  self.state.replace_storage(0,reserved,0)?;
  if let Err(error)=self.output_replace_text(object,text){self.state.retained-=reserved;return Err(error);}
  self.state.output_defaults.commit(growth);
  self.state.output_defaults.insert(object,current);
  self.state.retained-=peak;
  Ok(())
 }

 pub(super) fn reset_output_value(&mut self,object:u32,value:Value)->Result<(),String>{
  self.validate_output_value(object).map_err(|(_,message)|message)?;
  let lookup=self.state.output_defaults.lookup_work();
  self.state.work(lookup.checked_mul(2).and_then(|work|work.checked_add(1)).ok_or("OUTPUT reset lookup overflow.")?)?;
  let old=self.state.output_defaults.get(&object).map(DomString::capacity);
  if old.is_some(){
   self.state.work(self.state.output_defaults.len())?;
   let shifted=self.state.output_defaults.retention_movement(|key|*key!=object);
   self.state.copies(shifted.checked_mul(size_of::<(u32,DomString)>()).ok_or("OUTPUT reset index movement overflow.")?)?;
  }
  script_text_content::deadline(self.state)?;
  // The native getter result coexists with its DOMString conversion and the
  // unpublished replacement. Admit that source backing throughout the write.
  let source=retained_storage::value_bytes(&value);
  self.state.replace_storage(0,source,0)?;
  let result=(||{let text=self.dom_text(&value)?;self.output_replace_text(object,text)})();
  self.state.retained-=source;
  result?;
  if let Some(bytes)=old{self.state.output_defaults.remove(&object);self.state.retained-=bytes;}
  Ok(())
 }

}
