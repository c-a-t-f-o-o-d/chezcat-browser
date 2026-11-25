//! Canonical MAP areas uses a live exact-namespace HTMLCollection.
//! https://html.spec.whatwg.org/multipage/image-maps.html#dom-map-areas
use super::*;
pub(super) const GET:&str="\0HTMLMapElement.get.areas";
impl Bridge<'_>{
 pub(super) fn map_areas_member(&self,object:u32,property:&str)->bool{property=="areas"&&self.map_name_receiver(object)}
 pub(super) fn validate_map_areas(&self,object:u32)->Result<(),(&'static str,String)>{self.validate_map_name(object)}
 pub(super) fn map_areas_get(&mut self,object:u32)->Result<Value,String>{
  self.validate_map_areas(object).map_err(|(_,message)|message)?;
  self.state.work(1)?;script_text_content::deadline(self.state)?;
  self.create_map_areas_collection(object)
 }
}
