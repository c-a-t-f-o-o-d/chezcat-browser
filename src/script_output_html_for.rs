//! OUTPUT.htmlFor uses the actual null-namespace for attribute token list.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-output-htmlfor
use super::*;
pub(super) const GET:&str="\0HTMLOutputElement.get.htmlFor";
pub(super) const SET:&str="\0HTMLOutputElement.set.htmlFor";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn html_output_receiver(&self,object:u32)->bool{
  self.concrete_html_interface(object)==Some("HTMLOutputElement")||self.inert_interface(object)==Some("HTMLOutputElement")
 }
 pub(super) fn output_html_for_member(&self,object:u32,property:&str)->bool{property=="htmlFor"&&self.html_output_receiver(object)}
 pub(super) fn validate_output_html_for(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.html_output_receiver(object){return Err(("TypeError","HTMLOutputElement.htmlFor requires a genuine HTML output.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child output token lists require their lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn output_html_for_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_output_html_for(object).map_err(|(_,message)|message)?;
  Ok(Value::Host(script_class_list::output_for_handle(object)))
 }
 pub(super) fn output_for_value_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_output_html_for(object).map_err(|(_,message)|message)?;
  let value=self.dom_text(value)?;self.state.copies(3)?;
  let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("for")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}
  else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
