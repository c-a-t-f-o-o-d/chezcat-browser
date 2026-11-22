//! Canonical CharacterData substring reads actual UTF-16 units without mutation.
use super::*;
pub(in crate::scripting) const METHOD:&str="\0CharacterData.substringData";
fn unsigned_long(value:Option<&Value>)->Result<usize,String>{let Some(Value::Number(value))=value else{return Err("CharacterData offset/count requires converted Numbers.".into());};Ok(if !value.is_finite()||*value==0.0{0}else{value.trunc().rem_euclid(4294967296.0) as u32 as usize})}
impl Bridge<'_>{
 fn substring_data_source(&self,object:u32)->Option<&DomString>{
  if let Some(node)=self.state.inert_documents.node(object){return match &node.kind{Kind::Text(data)|Kind::ProcessingInstruction{data,..}=>Some(data),Kind::Parsed{source,..}=>self.parsed_source(*source)?.0.kind.character_dom_string(),_=>None};}
  let node=node_index(object).filter(|&node|self.dom_accessible(node))?;self.doc.nodes.get(node)?.kind.character_dom_string()
 }
 pub(in crate::scripting) fn substring_data_receiver(&self,object:u32)->bool{self.substring_data_source(object).is_some()}
 pub(in crate::scripting) fn validate_substring_data(&self,object:u32,args:&[Value])->Result<(),(&'static str,String)>{
  let data=self.substring_data_source(object).ok_or(("TypeError","substringData requires a genuine CharacterData node.".into()))?;
  let offset=unsigned_long(args.first()).map_err(|message|("TypeError",message))?;unsigned_long(args.get(1)).map_err(|message|("TypeError",message))?;
  if offset>data.utf16_len(){return Err(("IndexSizeError","substringData offset exceeds current UTF-16 length.".into()));}Ok(())
 }
 pub(in crate::scripting) fn native_substring_data(&mut self,object:u32,args:&[Value])->Result<Value,String>{
  self.validate_substring_data(object,args).map_err(|(_,message)|message)?;let offset=unsigned_long(args.first())?;let count=unsigned_long(args.get(1))?;
  let data=self.substring_data_source(object).ok_or("Missing CharacterData source.")?;let length=data.utf16_len();let count=count.min(length-offset);
  let work=data.used_bytes().checked_mul(3).and_then(|n|count.checked_mul(3).and_then(|c|n.checked_add(c))).and_then(|n|n.checked_add(1)).ok_or("CharacterData substring work overflow.")?;self.state.work(work)?;
  let units_bytes=count.checked_mul(size_of::<u16>()).ok_or("CharacterData substring unit storage overflow.")?;self.state.copies(units_bytes)?;let mut units=Vec::new();units.try_reserve_exact(count).map_err(|_|"Cannot reserve CharacterData substring units.")?;
  units.extend(self.substring_data_source(object).unwrap().code_units().skip(offset).take(count));
  let(projection,raw)=DomString::projected_size(&units)?;let logical=if raw{units_bytes}else{projection};if logical>self.state.limits.host.string_bytes{return Err("CharacterData substring result exceeds the host string allowance.".into());}
  let storage=projection.checked_add(if raw{units_bytes}else{0}).ok_or("CharacterData substring output storage overflow.")?;self.state.copies(storage)?;
  Ok(Self::dom_string_owned(DomString::from_units(&units,DomString::storage_bound(self.state.limits.host.string_bytes)?)?))
 }
}
