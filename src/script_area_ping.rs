//! Canonical AREA ping USVString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/image-maps.html#the-area-element
use super::*;
pub(super) const GET:&str="\0HTMLAreaElement.get.ping";
pub(super) const SET:&str="\0HTMLAreaElement.set.ping";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 fn area_ping_usv(&mut self,value:&Value)->Result<String,String>{
  let Value::StringUnits(units)=value else{return self.text(value)};
  self.state.work(units.len().checked_mul(2).ok_or("Area ping USVString work overflow.")?)?;
  let(bytes,_)=DomString::projected_size(units)?;
  if bytes>self.state.limits.host.string_bytes{return Err("Area ping USVString exceeds the host string allowance.".into());}
  self.state.copies(bytes)?;
  let mut text=String::new();text.try_reserve_exact(bytes).map_err(|_|"Cannot reserve area ping USVString.")?;
  text.extend(char::decode_utf16(units.iter().copied()).map(|v|v.unwrap_or(char::REPLACEMENT_CHARACTER)));
  Ok(text)
 }

 pub(super) fn area_ping_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLAreaElement")}
 pub(super) fn area_ping_member(&self,object:u32,property:&str)->bool{property=="ping"&&self.area_ping_receiver(object)}
 pub(super) fn validate_area_ping(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.area_ping_receiver(object){return Err(("TypeError","Area ping metadata requires a genuine HTML area element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child area metadata requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn area_ping_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_area_ping(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.element_attribute_string_get(object,"ping")?;let value=self.area_ping_usv(&value)?;script_text_content::deadline(self.state)?;Ok(Value::String(value))
 }
 pub(super) fn area_ping_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_area_ping(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.area_ping_usv(value)?;self.state.copies(4)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("ping")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value.into()))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value.into()),script_attr::InputPreparation::Pending)}
 }
}
