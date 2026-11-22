//! Staged UTF-16 edits enter the existing native data setter transaction.
use super::*;
use crate::javascript::{NativeCharacterDataEdit,character_data_interface::{self,Edit}};
impl Bridge<'_>{
 fn character_edit_bounds(&self,object:u32,edit:Edit,args:&[Value])->Result<(usize,usize,usize),(&'static str,String)>{
  let data=self.character_data_source(object).ok_or(("TypeError","CharacterData edit requires a genuine native receiver.".into()))?;let length=data.utf16_len();
  let offset=if matches!(edit,Edit::Append){length}else{character_substring::unsigned_long(args.first()).map_err(|message|("TypeError",message))?};
  let removed=if matches!(edit,Edit::Delete|Edit::Replace){character_substring::unsigned_long(args.get(1)).map_err(|message|("TypeError",message))?}else{0};
  if offset>length{return Err(("IndexSizeError","CharacterData edit offset exceeds current UTF-16 length.".into()));}Ok((length,offset,removed.min(length-offset)))
 }
 pub(in crate::scripting) fn validate_character_edit(&self,object:u32,method:&str,args:&[Value])->Result<(),(&'static str,String)>{let edit=character_data_interface::operation(method).ok_or(("TypeError","Unknown CharacterData edit.".into()))?;self.character_edit_bounds(object,edit,args).map(|_|())}
 pub(in crate::scripting) fn native_character_edit(&mut self,object:u32,method:&str,args:&[Value])->Result<NativeCharacterDataEdit,String>{
  let edit=character_data_interface::operation(method).ok_or("Unknown CharacterData edit.")?;let(length,offset,removed)=self.character_edit_bounds(object,edit,args).map_err(|(_,message)|message)?;
  let incoming=edit.data_index().map(|index|script_attribute_names::converted_name(args.get(index).ok_or(("TypeError","CharacterData edit requires data.".into()))?)).transpose().map_err(|(_,message)|message)?;let inserted=incoming.map_or(0,|data|data.code_units().count());let new_length=length.checked_sub(removed).and_then(|n|n.checked_add(inserted)).ok_or("CharacterData edit length overflow.")?;
  let source=self.character_data_source(object).unwrap();let work=source.used_bytes().checked_mul(3).and_then(|n|incoming.map_or(Some(0),|data|data.logical_bytes().checked_mul(2)).and_then(|m|n.checked_add(m))).and_then(|n|new_length.checked_mul(3).and_then(|m|n.checked_add(m))).ok_or("CharacterData edit work overflow.")?;self.state.work(work)?;
  let units_bytes=new_length.checked_mul(size_of::<u16>()).ok_or("CharacterData edit unit storage overflow.")?;self.state.copies(units_bytes)?;let mut units=Vec::new();units.try_reserve_exact(new_length).map_err(|_|"Cannot reserve CharacterData edit units.")?;let source=self.character_data_source(object).unwrap();units.extend(source.code_units().take(offset));if let Some(incoming)=incoming{units.extend(incoming.code_units());}units.extend(source.code_units().skip(offset+removed));
  let(projection,raw)=DomString::projected_size(&units)?;if (if raw{units_bytes}else{projection})>self.state.limits.host.string_bytes{return Err("CharacterData edit result exceeds the host string allowance.".into());}let storage=projection.checked_add(if raw{units_bytes}else{0}).ok_or("CharacterData edit output storage overflow.")?;self.state.copies(storage)?;
  Ok(NativeCharacterDataEdit{value:Self::dom_string_owned(DomString::from_units(&units,DomString::storage_bound(self.state.limits.host.string_bytes)?)?),offset,removed,inserted})
 }
 pub(in crate::scripting) fn write_character_splice(&mut self,object:u32,value:Value,offset:usize,removed:usize,inserted:usize)->Result<(),String>{
  let old=self.character_data_source(object).ok_or("CharacterData edit receiver disappeared.")?.utf16_len();let current=script_attribute_names::converted_name(&value).map_err(|(_,message)|message)?.code_units().count();
  if offset>old||removed>old-offset||old.checked_sub(removed).and_then(|n|n.checked_add(inserted))!=Some(current){return Err("CharacterData edit proposal no longer matches its actual source.".into());}
  if self.inert_node_type(object).is_some(){let text=self.dom_text(&value)?;self.state.prepare_range_changes(1,0)?;self.write_inert_character_data(object,text)?;self.state.ranges_splice_data(object,offset,removed,inserted);Ok(())}else{if !self.dom_set_with_character_splice(object,"data",&value,Some((offset,removed,inserted)))?{return Err("CharacterData native setter is unavailable.".into());}Ok(())}
 }
}

#[cfg(test)]
#[path="script_character_splice_storage_tests.rs"]
mod storage_tests;
