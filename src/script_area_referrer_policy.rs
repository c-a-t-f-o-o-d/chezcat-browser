//! Canonical AREA referrerPolicy DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/image-maps.html#the-area-element
use super::*;
pub(super) const GET:&str="\0HTMLAreaElement.get.referrerPolicy";
pub(super) const SET:&str="\0HTMLAreaElement.set.referrerPolicy";
const POLICIES:&[&str]=&["no-referrer","no-referrer-when-downgrade","same-origin","origin","strict-origin","origin-when-cross-origin","strict-origin-when-cross-origin","unsafe-url"];
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn area_referrer_policy_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLAreaElement")}
 pub(super) fn area_referrer_policy_member(&self,object:u32,property:&str)->bool{property=="referrerPolicy"&&self.area_referrer_policy_receiver(object)}
 pub(super) fn validate_area_referrer_policy(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.area_referrer_policy_receiver(object){return Err(("TypeError","Area referrerPolicy metadata requires a genuine HTML area element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child area metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn area_referrer_policy_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_area_referrer_policy(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let raw=self.element_attribute_string_get(object,"referrerpolicy")?;
  let keyword=if let Value::String(text)=&raw{self.state.work(text.len().saturating_mul(POLICIES.len()))?;POLICIES.iter().copied().find(|keyword|text.eq_ignore_ascii_case(keyword)).unwrap_or("")}else{""};
  let value=self.string(keyword)?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn area_referrer_policy_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_area_referrer_policy(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(14)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("referrerpolicy")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
