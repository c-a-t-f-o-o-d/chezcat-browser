//! Canonical HTMLLabelElement.htmlFor reflects the actual null-namespace attribute.
//! https://html.spec.whatwg.org/multipage/forms.html#dom-label-htmlfor
use super::*;
pub(super) const GET:&str="\0HTMLLabelElement.get.htmlFor";
pub(super) const SET:&str="\0HTMLLabelElement.set.htmlFor";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn html_label_receiver(&self,object:u32)->bool{
  self.concrete_html_interface(object)==Some("HTMLLabelElement")||self.inert_interface(object)==Some("HTMLLabelElement")
 }
 pub(super) fn html_label_for_member(&self,object:u32,property:&str)->bool{property=="htmlFor"&&self.html_label_receiver(object)}
 pub(super) fn validate_html_label_for(&self,object:u32,setter:bool)->Result<(),(&'static str,String)>{
  if !self.html_label_receiver(object){return Err(("TypeError","HTMLLabelElement.htmlFor requires a genuine HTML label.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){
   self.frame_light_dom_element(object).map_err(|message|("NotSupportedError",message))?;
   if setter{return Err(("NotSupportedError","Initial child label reflection writes require their lifecycle hooks.".into()));}
  }
  Ok(())
 }
 pub(super) fn html_label_for_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_html_label_for(object,false).map_err(|(_,message)|message)?;
  self.element_attribute_string_get(object,"for")
 }
 pub(super) fn html_label_for_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_html_label_for(object,true).map_err(|(_,message)|message)?;
  let value=self.dom_text(value)?;self.state.copies(3)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("for")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}
  else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
