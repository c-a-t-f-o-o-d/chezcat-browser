//! Canonical BUTTON type DOMString reflection over actual attributes.
//! https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element
use super::*;
pub(super) const GET:&str="\0HTMLButtonElement.get.type";
pub(super) const SET:&str="\0HTMLButtonElement.set.type";
pub(super) fn operation(method:&str)->bool{matches!(method,GET|SET)}
impl Bridge<'_>{
 pub(super) fn button_type_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLButtonElement")}
 pub(super) fn button_type_member(&self,object:u32,property:&str)->bool{property=="type"&&self.button_type_receiver(object)}
 pub(super) fn validate_button_type(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.button_type_receiver(object){return Err(("TypeError","Button type requires a genuine HTML button element.".into()));}
  self.native_node_access(object)?;if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child button type requires its lifecycle hooks.".into()));}Ok(())
 }
 pub(super) fn button_type_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_button_type(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let count=self.element_attribute_source(object).ok_or("BUTTON type attribute source unavailable.")?.len();self.state.work(count)?;
  let mut explicit=None;let mut command=false;
  for at in 0..count{
   let attr=&self.element_attribute_source(object).unwrap()[at];let bytes=attr.name.bytes().saturating_add(attr.value.logical_bytes());self.state.work(bytes.saturating_add(1))?;
   let attr=&self.element_attribute_source(object).unwrap()[at];
   if attr.name.html_is("type"){explicit=["submit","reset","button"].into_iter().find(|kind|attr.value.as_str().eq_ignore_ascii_case(kind));}
   if attr.name.html_is("command")||attr.name.html_is("commandfor"){command=true;}
  }
  let select_parent=if self.inert_node_type(object)==Some(1){self.inert_observation_parent(object).is_some_and(|parent|self.inert_interface(parent)==Some("HTMLSelectElement"))}else{
   self.state.rebuild(self.doc)?;let node=self.node(object)?;self.state.tree.parents.get(node).copied().flatten().is_some_and(|parent|self.doc.nodes[parent].namespace==crate::html::Namespace::Html&&tag(self.doc,parent)=="select")
  };
  script_text_content::deadline(self.state)?;self.string(explicit.unwrap_or(if command||select_parent{"button"}else{"submit"}))
 }
 pub(super) fn button_type_set(&mut self,object:u32,value:&Value)->Result<(),String>{
  self.validate_button_type(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.dom_text(value)?;self.state.copies(5)?;let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text("type")?);
  if self.inert_node_type(object)==Some(1){self.write_inert_dom_attribute_ns(object,name,Some(value))}else{let node=self.node(object)?;self.public_dom_attribute(node,name,Some(value),script_attr::InputPreparation::Pending)}
 }
}
