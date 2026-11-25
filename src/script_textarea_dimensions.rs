//! Canonical TEXTAREA positive unsigned dimensions over actual attributes.
//! https://html.spec.whatwg.org/multipage/common-dom-interfaces.html#reflect
use super::*;
pub(super) fn operation(method:&str)->Option<(&'static str,bool)>{match method{
 "\0HTMLTextAreaElement.get.cols"=>Some(("cols",false)),"\0HTMLTextAreaElement.set.cols"=>Some(("cols",true)),
 "\0HTMLTextAreaElement.get.rows"=>Some(("rows",false)),"\0HTMLTextAreaElement.set.rows"=>Some(("rows",true)),_=>None}}
impl Bridge<'_>{
 pub(super) fn textarea_dimensions_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTextAreaElement")}
 pub(super) fn textarea_dimensions_member(&self,object:u32,property:&str)->bool{matches!(property,"cols"|"rows")&&self.textarea_dimensions_receiver(object)}
 pub(super) fn validate_textarea_dimensions(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.textarea_dimensions_receiver(object){return Err(("TypeError","Textarea dimensions require a genuine HTML textarea element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child textarea dimensions require their lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn textarea_dimensions_get(&mut self,object:u32,property:&str)->Result<Value,String>{
  self.validate_textarea_dimensions(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let default=match property{"cols"=>20,"rows"=>2,_=>return Err("Unknown textarea dimension.".into())};
  let count=self.element_attribute_source(object).ok_or("Textarea dimension attribute source is unavailable.")?.len();self.state.work(count)?;let mut value=default;
  for at in 0..count{let attr=&self.element_attribute_source(object).unwrap()[at];let bytes=attr.name.bytes();let matched=attr.name.html_is(property);self.state.work(bytes.saturating_add(1))?;if matched{let attr=&self.element_attribute_source(object).unwrap()[at];let bytes=attr.value.used_bytes();self.state.work(bytes.saturating_add(1))?;value=crate::decimal_number::html_positive_with_fallback(self.element_attribute_source(object).unwrap()[at].value.as_str(),default);break;}}
  script_text_content::deadline(self.state)?;Ok(Value::Number(f64::from(value)))
 }
 pub(super) fn textarea_dimensions_set(&mut self,object:u32,property:&str,value:&Value)->Result<(),String>{
  self.validate_textarea_dimensions(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let default=match property{"cols"=>20,"rows"=>2,_=>return Err("Unknown textarea dimension.".into())};
  let Value::Number(number)=value else{return Err("Textarea dimension setters require number conversion.".into());};
  let number=if !number.is_finite()||*number==0.0{0}else{number.trunc().rem_euclid(4294967296.0) as u32};let number=if (1..=2147483647).contains(&number){number}else{default};
  let text=self.text(&Value::Number(f64::from(number)))?;self.state.copies(property.len())?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text(property)?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(DomString::from(text)))}else{let node=self.node(object)?;self.public_attribute(node,name,Some(text))}
 }
}
