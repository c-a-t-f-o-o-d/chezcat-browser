//! Area URL components use the shared original hyperlink parser.
use super::*;
use crate::javascript::area_url_interface::MEMBERS;
pub(super) use crate::javascript::area_url_interface::operation;
impl Bridge<'_>{
 pub(super) fn area_url_member(&self,object:u32,property:&str)->bool{self.area_href_receiver(object)&&MEMBERS.iter().any(|&(name,_,_)|name==property)}
 pub(super) fn area_url_get(&mut self,object:u32,name:&str)->Result<Value,String>{
  self.validate_area_href(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  let value=self.anchor_reflection_get(object,name)?;script_text_content::deadline(self.state)?;Ok(value)
 }
 pub(super) fn area_url_set(&mut self,object:u32,name:&str,value:&Value)->Result<(),String>{
  self.validate_area_href(object).map_err(|(_,message)|message)?;self.state.work(1)?;script_text_content::deadline(self.state)?;
  if name=="origin"{return Err("Area origin is readonly.".into());}
  self.anchor_reflection_set(object,name,value)
 }
}
