//! Native PROGRESS numeric metadata over actual null-namespace attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-progress-element
use super::*;
pub(super) const VALUE_GET:&str="\0HTMLProgressElement.get.value";
pub(super) const VALUE_SET:&str="\0HTMLProgressElement.set.value";
pub(super) const MAX_GET:&str="\0HTMLProgressElement.get.max";
pub(super) const MAX_SET:&str="\0HTMLProgressElement.set.max";
pub(super) const POSITION_GET:&str="\0HTMLProgressElement.get.position";
pub(super) fn operation(method:&str)->bool{matches!(method,VALUE_GET|VALUE_SET|MAX_GET|MAX_SET|POSITION_GET)}
impl Bridge<'_>{
 pub(super) fn progress_numeric_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLProgressElement")}
 pub(super) fn progress_numeric_member(&self,object:u32,property:&str)->bool{matches!(property,"value"|"max"|"position")&&self.progress_numeric_receiver(object)}
 pub(super) fn progress_numeric_attribute(&self,object:u32,property:&str)->Option<&'static str>{if !self.progress_numeric_receiver(object){return None;}match property{"value"=>Some("value"),"max"=>Some("max"),_=>None}}
 pub(super) fn validate_progress_numeric(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.progress_numeric_receiver(object){return Err(("TypeError","Progress numeric metadata requires a genuine HTML progress element.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child progress metadata requires its lifecycle hooks.".into()));}
  Ok(())
 }
 fn progress_number_attribute(&mut self,object:u32,name:&str)->Result<(bool,Option<f64>),String>{
  let count=self.element_attribute_source(object).ok_or("Progress attribute source is unavailable.")?.len();self.state.work(count)?;
  for at in 0..count{
   let bytes=self.element_attribute_source(object).unwrap()[at].name.bytes();self.state.work(bytes.saturating_add(1))?;
   if self.element_attribute_source(object).unwrap()[at].name.html_is(name){
    let bytes=self.element_attribute_source(object).unwrap()[at].value.used_bytes();self.state.work(bytes.saturating_add(1))?;
    return Ok((true,crate::decimal_number::html_float(self.element_attribute_source(object).unwrap()[at].value.as_str())));
   }
  }
  Ok((false,None))
 }
 fn progress_maximum(&mut self,object:u32)->Result<f64,String>{Ok(self.progress_number_attribute(object,"max")?.1.filter(|n|*n>0.0).unwrap_or(1.0))}
 pub(super) fn progress_numeric_get(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_progress_numeric(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let number=if method==MAX_GET{self.progress_maximum(object)?}else if matches!(method,VALUE_GET|POSITION_GET){
   let(present,value)=self.progress_number_attribute(object,"value")?;
   if !present{if method==POSITION_GET{-1.0}else{0.0}}else{let max=self.progress_maximum(object)?;let value=value.unwrap_or(0.0).max(0.0).min(max);if method==POSITION_GET{value/max}else{value}}
  }else{return Err("Unknown progress numeric getter.".into());};
  script_text_content::deadline(self.state)?;Ok(Value::Number(number))
 }
 pub(super) fn progress_numeric_set(&mut self,object:u32,property:&str,value:&Value)->Result<(),String>{
  self.validate_progress_numeric(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let Value::Number(number)=value else{return Err("Progress numeric setters require number conversion.".into());};
  if !number.is_finite(){return Err("Progress numeric setters require finite doubles.".into());}
  if property=="max"&&*number<=0.0{return Ok(());}
  let key=match property{"value"=>"value","max"=>"max",_=>return Err("Unknown progress numeric setter.".into())};
  let text=self.text(value)?;self.state.copies(key.len())?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text(key)?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
