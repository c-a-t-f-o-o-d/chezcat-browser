//! Canonical LEGEND form metadata uses the actual direct parent FIELDSET.
//! https://html.spec.whatwg.org/multipage/form-elements.html#dom-legend-form
use super::*;
pub(super) const GET:&str="\0HTMLLegendElement.get.form";
impl Bridge<'_>{
 pub(super) fn legend_receiver(&self,object:u32)->bool{self.concrete_html_interface(object).or_else(||self.inert_interface(object))==Some("HTMLLegendElement")}
 pub(super) fn legend_form_member(&self,object:u32,property:&str)->bool{property=="form"&&self.legend_receiver(object)}
 pub(super) fn validate_legend_form(&self,object:u32)->Result<(),(&'static str,String)>{
  if !self.legend_receiver(object){return Err(("TypeError","Legend form requires a genuine HTML legend.".into()));}
  self.native_node_access(object)?;
  if self.is_frame_handle(object){return Err(("NotSupportedError","Initial child legend metadata requires its lifecycle hooks.".into()));}
  Ok(())
 }
 pub(super) fn legend_form_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_legend_form(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;self.state.rebuild(self.doc)?;
  let Some(parent)=self.whole_text_parent(object)?else{return Ok(Value::Null);};
  self.native_node_access(parent).map_err(|(_,message)|message)?;
  if !self.fieldset_receiver(parent){return Ok(Value::Null);}
  self.fieldset_get(parent,script_fieldset::FORM)
 }
}
