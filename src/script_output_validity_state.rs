//! Shared noncandidate form-control custom errors and live readonly validity views.
use super::*;
pub(super) const GET:&str="\0HTMLOutputElement.get.validity";
pub(super) const SET:&str="\0HTMLOutputElement.setCustomValidity";
pub(super) const FIELD_GET:&str="\0HTMLFieldSetElement.get.validity";
pub(super) const FIELD_SET:&str="\0HTMLFieldSetElement.setCustomValidity";
pub(super) const FLAGS:&[(&str,&str)]=&[
 ("valueMissing","\0ValidityState.get.valueMissing"),
 ("typeMismatch","\0ValidityState.get.typeMismatch"),
 ("patternMismatch","\0ValidityState.get.patternMismatch"),
 ("tooLong","\0ValidityState.get.tooLong"),
 ("tooShort","\0ValidityState.get.tooShort"),
 ("rangeUnderflow","\0ValidityState.get.rangeUnderflow"),
 ("rangeOverflow","\0ValidityState.get.rangeOverflow"),
 ("stepMismatch","\0ValidityState.get.stepMismatch"),
 ("badInput","\0ValidityState.get.badInput"),
 ("customError","\0ValidityState.get.customError"),
 ("valid","\0ValidityState.get.valid")];
const MAIN:u32=0xc500_0000;
const INERT:u32=0xc600_0000;
fn owner(handle:u32)->Option<u32>{
 if (MAIN..MAIN+NODE_HANDLES).contains(&handle){return Some(node_handle((handle-MAIN)as usize));}
 let count=script_inert_documents::ENTRY_END-script_inert_documents::MAIN_IMPLEMENTATION;
 if (INERT+1..INERT+count).contains(&handle){return Some(script_inert_documents::MAIN_IMPLEMENTATION+(handle-INERT));}
 None
}
fn handle(object:u32)->u32{if node_index(object).is_some(){MAIN+(object-NODE_BASE)}else{INERT+(object-script_inert_documents::MAIN_IMPLEMENTATION)}}
pub(super) fn flag(method:&str)->Option<&'static str>{FLAGS.iter().find_map(|&(key,op)|(op==method).then_some(key))}
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET|FIELD_GET|FIELD_SET)||flag(method).is_some()}
impl Bridge<'_>{
 pub(super) fn validity_receiver(&self,object:u32)->bool{owner(object).is_some_and(|o|self.html_output_receiver(o)||self.fieldset_receiver(o))}
 pub(super) fn output_validity_member(&self,object:u32,property:&str)->bool{matches!(property,"validity"|"setCustomValidity")&&(self.html_output_receiver(object)||self.fieldset_receiver(object))}
 pub(super) fn validity_member(&self,object:u32,property:&str)->bool{self.validity_receiver(object)&&FLAGS.iter().any(|&(key,_)|key==property)}
 pub(super) fn validate_output_validity(&self,object:u32,method:&str)->Result<(),(&'static str,String)>{
  if flag(method).is_some(){
   let native=owner(object).filter(|&o|self.html_output_receiver(o)||self.fieldset_receiver(o)).ok_or_else(||("TypeError","ValidityState requires a genuine native validity object.".into()))?;
   if self.fieldset_receiver(native){self.validate_fieldset(native)}else{self.validate_output_metadata(native)}
  }else if matches!(method,FIELD_GET|FIELD_SET){self.validate_fieldset(object)}else{self.validate_output_metadata(object)}
 }
 pub(super) fn output_validity_call(&mut self,object:u32,method:&str,args:&[Value])->Result<Value,String>{
  self.validate_output_validity(object,method).map_err(|(_,s)|s)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  if matches!(method,GET|FIELD_GET){return Ok(Value::Host(handle(object)));}
  if let Some(key)=flag(method){
   let output=owner(object).unwrap();self.state.work(self.state.output_custom_errors.lookup_work())?;
   let custom=self.state.output_custom_errors.get(&output).is_some();
   return Ok(Value::Bool(match key{"customError"=>custom,"valid"=>!custom,_=>false}));
  }
  if !matches!(method,SET|FIELD_SET){return Err("Unknown native validity operation.".into());}
  let value=args.first().ok_or("Custom validity requires one DOMString argument.")?;
  self.state.rebuild(self.doc)?;
  let source=retained_storage::value_bytes(value);
  self.state.replace_storage(0,source,0)?;
  let text:Result<DomString,String>=(||{
   let needed=match value{Value::String(s)=>s.len(),Value::StringUnits(units)=>{
    self.state.work(units.len())?;let(bytes,raw)=DomString::projected_size(units)?;
    bytes.checked_add(if raw{units.len().checked_mul(2).ok_or("Custom error input overflow.")?}else{0}).ok_or("Custom error input overflow.")?
   },_=>0};
   if needed>script_text_content::available(self.state,0)?{return Err("Custom error input copy storage allowance exhausted.".into());}
   let text=self.dom_text(value)?;
   if text.capacity()>script_text_content::available(self.state,0)?{return Err("Custom error input storage allowance exhausted.".into());}
   Ok(text)
  })();self.state.retained-=source;let mut text=text?;
  self.state.work(text.used_bytes())?;self.state.copies(text.used_bytes().checked_mul(2).ok_or("Custom error normalization work overflow.")?)?;
  text.normalize_newlines();script_text_content::deadline(self.state)?;
  let map=&self.state.output_custom_errors;let old=map.get(&object).map_or(0,DomString::capacity);
  if text.logical_bytes()==0{
   let work=map.insert_work(&object);self.state.work(work)?;self.state.copies(work.checked_mul(size_of::<(u32,DomString)>()).ok_or("Custom error removal work overflow.")?)?;script_text_content::deadline(self.state)?;
   self.state.output_custom_errors.remove(&object);self.state.retained-=old;self.state.dom_committed();return Ok(Value::Undefined);
  }
  if old!=0{
   self.state.work(map.lookup_work())?;
   if text.capacity()>script_text_content::available(self.state,0)?{return Err("Custom error replacement storage allowance exhausted.".into());}
   script_text_content::deadline(self.state)?;self.state.replace_storage(old,text.capacity(),size_of::<(u32,DomString)>())?;
   self.state.output_custom_errors.insert(object,text);self.state.dom_committed();return Ok(Value::Undefined);
  }
  let peak=if map.minimum_growth(1)?!=0{map.storage_bytes()}else{0};
  let available=script_text_content::available(self.state,text.capacity().checked_add(peak).ok_or("Custom error joint storage overflow.")?)?;
  if map.minimum_growth(1)?>available{return Err("Custom error index storage allowance exhausted.".into());}
  let(growth,metadata)=map.prepare(1,available)?;let work=map.insert_work(&object);
  let copies=work.checked_add(usize::from(growth.is_some())*map.len()).and_then(|n|n.checked_mul(size_of::<(u32,DomString)>())).ok_or("Custom error index work overflow.")?;
  self.state.work(work)?;self.state.copies(copies)?;script_text_content::deadline(self.state)?;
  self.state.replace_storage(0,text.capacity().checked_add(metadata).ok_or("Custom error retained storage overflow.")?,0)?;
  self.state.output_custom_errors.commit(growth);self.state.output_custom_errors.insert(object,text);self.state.dom_committed();Ok(Value::Undefined)
 }
}
