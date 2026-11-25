//! Canonical hyperlink stringifiers call the internal href getter.
use super::*;
pub(super) const AREA:&str="\0HTMLAreaElement.toString";
pub(super) const ANCHOR:&str="\0HTMLAnchorElement.toString";
pub(super) fn operation(method:&str)->bool{matches!(method,AREA|ANCHOR)}
impl Bridge<'_>{
 pub(super) fn hyperlink_stringifier_receiver(&self,object:u32,method:&str)->bool{
  match method{AREA=>self.area_href_receiver(object),ANCHOR=>self.native_anchor(object),_=>false}
 }
 pub(super) fn validate_hyperlink_stringifier(&self,object:u32,method:&str)->Result<(),(&'static str,String)>{
  if !self.hyperlink_stringifier_receiver(object,method){return Err(("TypeError","Hyperlink stringification requires its genuine HTML element interface.".into()));}
  if method==AREA{return self.validate_area_href(object);}
  self.native_node_access(object)
 }
 pub(super) fn hyperlink_stringify(&mut self,object:u32,method:&str)->Result<Value,String>{
  self.validate_hyperlink_stringifier(object,method).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.anchor_reflection_get(object,"href")?;script_text_content::deadline(self.state)?;Ok(value)
 }
}
