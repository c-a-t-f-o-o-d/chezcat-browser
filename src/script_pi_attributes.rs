//! Original XML pseudo-attribute parsing and ordered native PI state.
//! https://www.w3.org/TR/2010/REC-xml-stylesheet-20101028/#dt-parsing
use super::*;
use crate::javascript::processing_instruction_interface::{self as interface,Prepared};
pub(super) struct Attribute{pub name:DomString,pub value:DomString}
pub(super) struct Record{pub object:u32,pub attributes:Vec<Attribute>,pub ready:bool}
pub(super) struct Pending{object:u32,index:usize,attributes:Vec<Attribute>,map_bytes:usize,data_bytes:usize,committed:bool}
fn bytes(attrs:&Vec<Attribute>)->usize{attrs.capacity()*size_of::<Attribute>()+attrs.iter().map(|a|a.name.capacity()+a.value.capacity()).sum::<usize>()}

// Temporary and persistent native allocations share the current retained
// allowance. Counters are copied back even when construction is refused.
struct Budget{available:usize,held:usize,work:usize,copies:usize,deadline:Option<Instant>,string:usize}
impl Budget{
 fn work(&mut self,n:usize)->Result<(),String>{if self.deadline.is_some_and(|d|Instant::now()>=d){return Err("PI computation time limit reached.".into());}self.work=self.work.checked_sub(n).ok_or("PI native work allowance exhausted.")?;Ok(())}
 fn copy(&mut self,n:usize)->Result<(),String>{self.work(1)?;self.copies=self.copies.checked_sub(n).ok_or("PI native copy allowance exhausted.")?;Ok(())}
 fn hold(&mut self,n:usize)->Result<(),String>{self.held=self.held.checked_add(n).filter(|n|*n<=self.available).ok_or("PI retained native storage allowance exhausted.")?;Ok(())}
 fn reserve<T>(&mut self,v:&mut Vec<T>,additional:usize)->Result<(),String>{let required=v.len().checked_add(additional).ok_or("PI vector size overflow.")?;if required<=v.capacity(){return Ok(());}let old=v.capacity()*size_of::<T>();let maximum=v.capacity().saturating_add(self.available.saturating_sub(self.held)/size_of::<T>());let capacity=required.max(v.capacity().saturating_mul(2).min(maximum));let target=capacity.checked_mul(size_of::<T>()).ok_or("PI vector storage overflow.")?;if target-old>self.available.saturating_sub(self.held){return Err("PI retained vector allowance exhausted.".into());}self.copy(v.len().checked_mul(size_of::<T>()).ok_or("PI vector move overflow.")?)?;v.try_reserve_exact(capacity-v.len()).map_err(|_|"Cannot reserve PI native vector.")?;self.hold(v.capacity()*size_of::<T>()-old)}
 fn units(&mut self,source:crate::dom_string::DomStringRef<'_>)->Result<Vec<u16>,String>{self.work(source.logical_bytes())?;let count=source.code_units().count();self.work(count)?;self.copy(count.checked_mul(2).ok_or("PI units overflow.")?)?;let mut out=Vec::new();self.reserve(&mut out,count)?;out.extend(source.code_units());Ok(out)}
 fn text(&mut self,units:&[u16])->Result<DomString,String>{self.work(units.len())?;let(projection,raw)=DomString::projected_size(units)?;let logical=if raw{units.len().checked_mul(2).ok_or("PI string overflow.")?}else{projection};if logical>self.string{return Err("PI string exceeds native string allowance.".into());}let count=projection.checked_add(if raw{logical}else{0}).ok_or("PI string storage overflow.")?;self.copy(count)?;if count>self.available.saturating_sub(self.held){return Err("PI string retained allowance exhausted.".into());}let out=DomString::from_units(units,self.available-self.held)?;self.hold(out.capacity())?;Ok(out)}
 fn clone_text(&mut self,value:&DomString)->Result<DomString,String>{self.work(value.utf16_len())?;self.copy(value.capacity())?;if value.capacity()>self.available.saturating_sub(self.held){return Err("PI copied string retained allowance exhausted.".into());}let out=value.try_clone()?;self.hold(out.capacity())?;Ok(out)}
 fn push(&mut self,out:&mut Vec<u16>,units:&[u16])->Result<(),String>{self.work(units.len())?;self.copy(units.len().checked_mul(2).ok_or("PI output overflow.")?)?;self.reserve(out,units.len())?;out.extend_from_slice(units);Ok(())}
}
fn space(u:u16)->bool{matches!(u,9|10|13|32)}
fn xml_char(u:u32)->bool{matches!(u,9|10|13|0x20..=0xd7ff|0xe000..=0xfffd|0x10000..=0x10ffff)}
fn reference(input:&[u16])->Option<u32>{
 for(name,value)in[("amp",38),("lt",60),("gt",62),("quot",34),("apos",39)]{if input.iter().copied().eq(name.encode_utf16()){return Some(value);}}
 if input.first()!=Some(&35){return None;}let(hex,digits)=if input.get(1)==Some(&120){(true,&input[2..])}else{(false,&input[1..])};if digits.is_empty(){return None;}let mut result=0u32;for &u in digits{let digit=match u{48..=57=>(u-48)as u32,65..=70 if hex=>(u-65+10)as u32,97..=102 if hex=>(u-97+10)as u32,_=>return None};result=result.checked_mul(if hex{16}else{10})?.checked_add(digit)?;}xml_char(result).then_some(result)
}
fn parse(input:&[u16],b:&mut Budget)->Result<Vec<Attribute>,String>{
 let mut attrs=Vec::new();let mut pos=0;
 // Malformed input discards the complete map, never a parsed prefix.
 macro_rules! malformed{()=>{return Ok(Vec::new())}}
 while pos<input.len(){b.work(1)?;while pos<input.len()&&space(input[pos]){pos+=1;}if pos==input.len(){break;}let start=pos;
  while pos<input.len()&&!space(input[pos])&&input[pos]!=61{pos+=1;}
  let name=&input[start..pos];if !script_attribute_names::valid_xml_name(crate::dom_string::DomStringRef::Raw(name)){malformed!();}
  while pos<input.len()&&space(input[pos]){pos+=1;}if input.get(pos)!=Some(&61){malformed!();}pos+=1;while pos<input.len()&&space(input[pos]){pos+=1;}
  let Some(&quote @ (34|39))=input.get(pos)else{malformed!();};pos+=1;let mut value=Vec::new();
  while pos<input.len()&&input[pos]!=quote{b.work(1)?;let u=input[pos];pos+=1;if u==60{malformed!();}if u==38{let start=pos;while pos<input.len()&&input[pos]!=59{b.work(1)?;pos+=1;}if pos==input.len(){malformed!();}let Some(cp)=reference(&input[start..pos])else{malformed!();};pos+=1;let mut encoded=[0;2];b.push(&mut value,char::from_u32(cp).unwrap().encode_utf16(&mut encoded))?;}else{b.push(&mut value,&[u])?;}}
  if input.get(pos)!=Some(&quote){malformed!();}pos+=1;if pos<input.len()&&!space(input[pos]){malformed!();}
  b.work(attrs.len().checked_mul(name.len().saturating_add(1)).ok_or("PI duplicate-name scan overflow.")?)?;
  if attrs.iter().any(|a:&Attribute|a.name.view().same_units(crate::dom_string::DomStringRef::Raw(name))){malformed!();}
  let name=b.text(name)?;let text=b.text(&value)?;b.held-=value.capacity()*2;drop(value);b.reserve(&mut attrs,1)?;attrs.push(Attribute{name,value:text});
 }Ok(attrs)
}
fn serialize(attrs:&[Attribute],b:&mut Budget)->Result<DomString,String>{let mut out=Vec::new();for(i,a)in attrs.iter().enumerate(){if i!=0{b.push(&mut out,&[32])?;}for u in a.name.code_units(){b.push(&mut out,&[u])?;}b.push(&mut out,&[61,34])?;for u in a.value.code_units(){match u{38=>b.push(&mut out,&[38,97,109,112,59])?,60=>b.push(&mut out,&[38,108,116,59])?,62=>b.push(&mut out,&[38,103,116,59])?,34=>b.push(&mut out,&[38,113,117,111,116,59])?,_=>b.push(&mut out,&[u])?,}}b.push(&mut out,&[34])?;}b.text(&out)}
impl State{
 pub(in crate::scripting) fn prepare_pi_invalidation(&mut self)->Result<(),String>{self.work(self.pi_attributes.len().saturating_add(1))}
 pub(in crate::scripting) fn invalidate_pi_attributes(&mut self,object:u32){if let Some(pending)=self.pi_pending.as_mut().filter(|p|p.object==object){pending.committed=true;}if let Some(record)=self.pi_attributes.iter_mut().find(|r|r.object==object){self.retained-=bytes(&record.attributes);record.attributes=Vec::new();record.ready=false;}}
}
impl Bridge<'_>{
 pub(in crate::scripting) fn pi_receiver(&self,object:u32)->bool{self.inert_node_type(object)==Some(7)||node_index(object).filter(|&n|self.dom_accessible(n)).and_then(|n|self.doc.nodes.get(n)).is_some_and(|n|matches!(n.kind,NodeKind::ProcessingInstruction{..}))}
 pub(in crate::scripting) fn validate_pi_operation(&self,object:u32,method:&str,args:&[Value])->Result<(),(&'static str,String)>{if !self.pi_receiver(object){return Err(("TypeError","ProcessingInstruction method requires a genuine supported PI.".into()));}if matches!(interface::operation(method),Some("setAttribute"|"toggleAttribute")){let name=script_attribute_names::converted_name(args.first().ok_or(("TypeError","Missing PI name.".into()))?)?;if !script_attribute_names::valid_attribute_name_dom(name){return Err(("InvalidCharacterError","Invalid ProcessingInstruction attribute local name.".into()));}}Ok(())}
 fn pi_build<T>(&mut self,f:impl FnOnce(&Self,&mut Budget)->Result<T,String>)->Result<T,String>{let mut b=Budget{available:self.state.retained_limit.saturating_sub(self.state.retained),held:0,work:self.state.host_work,copies:self.state.copy_work,deadline:self.state.execution_deadline,string:self.state.limits.host.string_bytes};let result=f(self,&mut b);self.state.host_work=b.work;self.state.copy_work=b.copies;result}
 fn pi_index(&mut self,object:u32)->Result<usize,String>{
  if !self.pi_receiver(object){return Err("ProcessingInstruction receiver is unavailable.".into());}self.state.work(self.state.pi_attributes.len().saturating_add(1))?;
  let index=self.state.pi_attributes.iter().position(|r|r.object==object);if let Some(index)=index{if self.state.pi_attributes[index].ready{return Ok(index);}}
  let attrs=self.pi_build(|this,b|{let source=this.character_data_source(object).ok_or("PI source is unavailable.")?;let input=b.units(source.view())?;parse(&input,b)})?;let payload=bytes(&attrs);
  let growth=if index.is_none(){retained_storage::grow(&self.state.pi_attributes,1,self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(payload).ok_or("PI map retained allowance exhausted.")?)?}else{(None,0)};
  if let Some(mut next)=growth.0{next.append(&mut self.state.pi_attributes);self.state.pi_attributes=next;}self.state.retained+=payload+growth.1;
  if let Some(index)=index{self.state.pi_attributes[index].attributes=attrs;self.state.pi_attributes[index].ready=true;Ok(index)}else{let index=self.state.pi_attributes.len();self.state.pi_attributes.push(Record{object,attributes:attrs,ready:true});Ok(index)}
 }
 pub(in crate::scripting) fn prepare_pi(&mut self,object:u32,method:&str,args:&[Value])->Result<Prepared,String>{
  self.validate_pi_operation(object,method,args).map_err(|(_,m)|m)?;if self.state.pi_pending.is_some(){return Err("Another PI write is pending.".into());}let index=self.pi_index(object)?;let operation=interface::operation(method).ok_or("Unknown PI operation.")?;
  if operation=="hasAttributes"{return Ok(Prepared::Result(Value::Bool(!self.state.pi_attributes[index].attributes.is_empty())));}
  if operation=="getAttributeNames"{return self.pi_build(|this,b|{let attrs=&this.state.pi_attributes[index].attributes;let mut names=Vec::new();b.reserve(&mut names,attrs.len())?;for a in attrs{names.push(b.clone_text(&a.name)?);}Ok(Prepared::Names(names))});}
  let name=script_attribute_names::converted_name(&args[0]).map_err(|(_,m)|m)?;let attrs=&self.state.pi_attributes[index].attributes;self.state.work(attrs.len().checked_mul(name.code_units().count().saturating_add(1)).ok_or("PI name lookup overflow.")?)?;let position=self.state.pi_attributes[index].attributes.iter().position(|a|a.name.view().same_units(name));
  if operation=="hasAttribute"{return Ok(Prepared::Result(Value::Bool(position.is_some())));}
  if operation=="getAttribute"{return self.pi_build(|this,b|Ok(Prepared::Result(if let Some(pos)=position{Self::dom_string_owned(b.clone_text(&this.state.pi_attributes[index].attributes[pos].value)?)}else{Value::Null})));}
  let force=args.get(1).map(Value::truthy);let result=if operation=="toggleAttribute"{Value::Bool(if position.is_some(){force==Some(true)}else{force!=Some(false)})}else{Value::Undefined};
  if operation=="removeAttribute"&&position.is_none()||operation=="toggleAttribute"&&(position.is_some()&&force==Some(true)||position.is_none()&&force==Some(false)){return Ok(Prepared::Result(result));}
  let(attributes,data)=self.pi_build(|this,b|{let attrs=&this.state.pi_attributes[index].attributes;let mut next=Vec::new();b.reserve(&mut next,attrs.len().checked_add(usize::from(position.is_none())).ok_or("PI attribute count overflow.")?)?;
   for(i,a)in attrs.iter().enumerate(){b.work(1)?;if Some(i)==position&&operation!="setAttribute"{continue;}let name=b.clone_text(&a.name)?;let value=if Some(i)==position&&operation=="setAttribute"{let units=b.units(script_attribute_names::converted_name(&args[1]).map_err(|(_,m)|m)?)?;let out=b.text(&units)?;b.held-=units.capacity()*2;drop(units);out}else{b.clone_text(&a.value)?};next.push(Attribute{name,value});}
   if position.is_none(){let units=b.units(name)?;let name=b.text(&units)?;b.held-=units.capacity()*2;drop(units);let value=if operation=="setAttribute"{let units=b.units(script_attribute_names::converted_name(&args[1]).map_err(|(_,m)|m)?)?;let value=b.text(&units)?;b.held-=units.capacity()*2;drop(units);value}else{DomString::new()};next.push(Attribute{name,value});}let data=serialize(&next,b)?;Ok((next,data))})?;
  let map_bytes=bytes(&attributes);let data_bytes=data.capacity();self.state.retained=self.state.retained.checked_add(map_bytes).and_then(|n|n.checked_add(data_bytes)).filter(|n|*n<=self.state.retained_limit).ok_or("PI staged write retained allowance exhausted.")?;self.state.pi_pending=Some(Pending{object,index,attributes,map_bytes,data_bytes,committed:false});Ok(Prepared::Write{data:Self::dom_string_owned(data),result})
 }
 pub(in crate::scripting) fn finish_pi(&mut self,object:u32,committed:bool){if let Some(pending)=self.state.pi_pending.take(){debug_assert_eq!(pending.object,object);self.state.retained-=pending.data_bytes;if committed||pending.committed{let record=&mut self.state.pi_attributes[pending.index];self.state.retained-=bytes(&record.attributes);record.attributes=pending.attributes;record.ready=true;}else{self.state.retained-=pending.map_bytes;}}}
}
