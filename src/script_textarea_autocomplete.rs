//! Canonical TEXTAREA autocomplete DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-textarea-element
use super::*;
pub(super) const GET:&str="\0HTMLTextAreaElement.get.autocomplete";
pub(super) const SET:&str="\0HTMLTextAreaElement.set.autocomplete";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn textarea_autocomplete_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLTextAreaElement")}
 pub(super) fn textarea_autocomplete_member(&self,object:u32,property:&str)->bool{property=="autocomplete"&&self.textarea_autocomplete_receiver(object)}
 pub(super) fn validate_textarea_autocomplete(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.textarea_autocomplete_receiver(object){return Err(("TypeError","Textarea autocomplete requires a genuine HTML textarea element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child textarea autocomplete requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn textarea_autocomplete_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_textarea_autocomplete(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let source=self.element_attribute_string_get(object,"autocomplete")?;
  let view=match &source{Value::String(s)=>crate::dom_string::DomStringRef::Scalar(s),Value::StringUnits(u)=>crate::dom_string::DomStringRef::Raw(u),_=>unreachable!()};
  self.state.work(view.logical_bytes().saturating_mul(64).saturating_add(1))?;
  let plan=Autofill::parse(view);let bytes=plan.bytes()?;
  if bytes>self.state.limits.host.string_bytes{return Err("Autocomplete exceeds the host string allowance.".into());}
  self.state.copies(bytes)?;script_text_content::deadline(self.state)?;
  let value=plan.value()?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn textarea_autocomplete_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_textarea_autocomplete(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(13)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("autocomplete")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}

// The expectation mantle exposes an empty IDL value for missing/invalid hints,
// independently of the containing form's internal autofill state.
use crate::dom_string::{DomStringRef,ascii_whitespace_tokens};
struct Autofill<'a>{tokens:[Option<DomStringRef<'a>>;5],len:usize,raw:bool}
fn lower(unit:u16)->u16{if (0x41..=0x5a).contains(&unit){unit+0x20}else{unit}}
fn is(token:DomStringRef<'_>,word:&str)->bool{token.code_units().map(lower).eq(word.bytes().map(u16::from))}
fn one_of(token:DomStringRef<'_>,words:&str)->bool{words.split(' ').any(|word|is(token,word))}
impl<'a> Autofill<'a>{
 fn parse(source:DomStringRef<'a>)->Self{
  let mut result=Self{tokens:[None;5],len:0,raw:matches!(source,DomStringRef::Raw(_))};
  for token in ascii_whitespace_tokens(source){if result.len==5{return Self{len:0,..result};}result.tokens[result.len]=Some(token);result.len+=1;}
  if result.len==0{return result;}
  let at=|n:usize|result.tokens[n].unwrap();let mut end=result.len;
  if is(at(end-1),"webauthn"){end-=1;if end==0{return result;}}
  let field=at(end-1);end-=1;
  if one_of(field,"on off"){if result.len!=1{result.len=0;}return result;}
  let contact=one_of(field,"tel tel-country-code tel-national tel-area-code tel-local tel-local-prefix tel-local-suffix tel-extension email impp");
  if !contact&&!one_of(field,"name honorific-prefix given-name additional-name family-name honorific-suffix nickname organization-title username new-password current-password one-time-code organization street-address address-line1 address-line2 address-line3 address-level4 address-level3 address-level2 address-level1 country country-name postal-code cc-name cc-given-name cc-additional-name cc-family-name cc-number cc-exp cc-exp-month cc-exp-year cc-csc cc-type transaction-currency transaction-amount language bday bday-day bday-month bday-year sex url photo"){result.len=0;return result;}
  if contact&&end>0&&one_of(at(end-1),"home work mobile fax pager"){end-=1;}
  if end>0&&one_of(at(end-1),"shipping billing"){end-=1;}
  if end==1&&at(0).code_units().map(lower).take(8).eq("section-".bytes().map(u16::from)){end=0;}
  if end!=0{result.len=0;}result
 }
 fn bytes(&self)->Result<usize,String>{
  self.tokens[..self.len].iter().try_fold(self.len.saturating_sub(1)*if self.raw{2}else{1},|sum,token|sum.checked_add(token.unwrap().logical_bytes()).ok_or("Autocomplete size overflow.".into()))
 }
 fn value(&self)->Result<Value,String>{
  let bytes=self.bytes()?;
  if self.len==0{return Ok(Value::String(String::new()));}
  if self.raw{let mut units=Vec::new();units.try_reserve_exact(bytes/2).map_err(|_|"Cannot reserve autocomplete units.")?;for (i,token) in self.tokens[..self.len].iter().enumerate(){if i>0{units.push(32);}units.extend(token.unwrap().code_units().map(lower));}Ok(Value::StringUnits(units))}
  else{let mut text=String::new();text.try_reserve_exact(bytes).map_err(|_|"Cannot reserve autocomplete text.")?;for (i,token) in self.tokens[..self.len].iter().enumerate(){if i>0{text.push(' ');}let DomStringRef::Scalar(s)=token.unwrap()else{unreachable!()};text.extend(s.chars().map(|ch|ch.to_ascii_lowercase()));}Ok(Value::String(text))}
 }
}
