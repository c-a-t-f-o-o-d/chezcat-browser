//! Token operations over immutable inert Element attribute sources. Holding an Rc
//! view copies no attribute payload and adds no allocation or token cache.
use super::*;
use crate::dom_string::DomStringRef;

enum Source<'a>{Empty,Loaded(&'a [crate::html::Attribute]),Generated(Rc<Node>)}
impl Source<'_>{
 fn attrs(&self)->Result<&[crate::html::Attribute],String>{match self{
  Self::Empty=>Ok(&[]),Self::Loaded(attrs)=>Ok(attrs),
  Self::Generated(node)=>match &node.kind{NodeKind::Element{attrs,..}=>Ok(attrs),_=>Err("Inert token source is not an Element.".into())},
 }}
 fn value(&self,attribute:&str)->Result<(DomStringRef<'_>,bool),String>{let found=self.attrs()?.iter().find(|a|a.name.html_is(attribute));Ok((found.map(|a|a.value.view()).unwrap_or(DomStringRef::Scalar("")),found.is_none()))}
}
// The returned lifetime belongs only to the displayed document. Generated
// sources retain a scoped Rc, so metering can mutate State without copying them.
fn source<'a>(doc:&'a Document,arena:&Arena,owner:u32)->Result<Source<'a>,String>{
 match arena.node(owner).ok_or("Inert token owner is unavailable.")?.kind{
  Kind::Element(_)=>Ok(Source::Empty),
  Kind::Parsed{source:ParsedSource::Loaded(index),..}=>match &doc.nodes.get(index).ok_or("Inert token source is unavailable.")?.kind{NodeKind::Element{attrs,..}=>Ok(Source::Loaded(attrs)),_=>Err("Inert token source is not an Element.".into())},
  Kind::Parsed{source:ParsedSource::Generated(index),..}=>Ok(Source::Generated(Rc::clone(&arena.generated.get(index).ok_or("Inert token snapshot is unavailable.")?.node))),
  _=>Err("Inert token owner is not an Element.".into()),
 }
}
impl Bridge<'_>{
 pub(in crate::scripting) fn inert_class_element_receiver(&self,owner:u32)->bool{
  self.inert_node_type(owner)==Some(1)
 }
 fn inert_token_owner(&mut self,object:u32)->Result<u32,String>{
  let owner=script_class_list::element(object).filter(|&owner|self.inert_class_element_receiver(owner)).ok_or("Inert Element token list is unavailable.")?;
  if !matches!(script_class_list::attribute(object),Some("class"|"for"|"rel"))||!self.class_list_receiver(object){return Err("Inert Element token attribute is unavailable.".into());}
  self.native_node_access(owner).map_err(|(_,message)|message)?;
  self.state.work(1)?;self.state.copies(size_of::<usize>())?;Ok(owner)
 }
 fn inert_token_read(&mut self,object:u32,read:token_list_reads::Read<'_>)->Result<Value,String>{
  let owner=self.inert_token_owner(object)?;
  let source=source(self.doc,&self.state.inert_documents,owner)?;
  token_list_reads::read_attributes(self.state,source.attrs()?,script_class_list::attribute(object).ok_or("Inert token attribute is unavailable.")?,read)
 }
 pub(in crate::scripting) fn inert_class_list_set(&mut self,object:u32,property:&str,value:&Value)->Result<(),String>{
  let owner=self.inert_token_owner(object)?;
  if property!="value"{return Err("DOMTokenList property write is not implemented.".into());}
  match script_class_list::attribute(object){Some("for")=>self.output_for_value_set(owner,value),Some("rel")=>if self.form_rel_receiver(owner){self.form_rel_set(owner,value)}else{self.area_rel_set(owner,value)},_=>self.element_class_name_set(owner,value)}
 }
 pub(in crate::scripting) fn inert_class_list_get(&mut self,object:u32,property:&str)->Result<Value,String>{
  if property=="value"{let owner=self.inert_token_owner(object)?;return self.element_attribute_string_get(owner,script_class_list::attribute(object).ok_or("Inert token attribute is unavailable.")?);}
  if property=="length"{return self.inert_token_read(object,token_list_reads::Read::Length);}
  if let Some(index)=collection_index(property){return self.inert_token_read(object,token_list_reads::Read::Index(index,true));}
  self.inert_token_owner(object)?;
  if matches!(property,"entries"|"keys"|"values"){return Err(format!("DOMTokenList operation {property:?} is not implemented."));}
  Ok(Value::Undefined)
 }
 pub(in crate::scripting) fn inert_class_list_call(&mut self,object:u32,method:&str,args:&[Value])->Result<Value,String>{
  let owner=self.inert_token_owner(object)?;
  self.class_list_validate(object,method,args).map_err(|(_,message)|message)?;
  if method=="supports"{
   let token=token_list_reads::argument(args.first().ok_or("supports requires a converted DOMString token.")?)?;
   self.state.work(token.logical_bytes())?;
   // Area/form relation processing is not implemented, so no processing token is advertised.
   return Ok(Value::Bool(false));
  }
  if method=="toString"{return self.element_attribute_string_get(owner,script_class_list::attribute(object).ok_or("Inert token attribute is unavailable.")?);}
  if method=="item"{let value=args.first().ok_or("item requires an index.")?.to_number();let index=if value.is_finite(){value.trunc().rem_euclid(4_294_967_296.0)as usize}else{0};return self.inert_token_read(object,token_list_reads::Read::Index(index,false));}
  if matches!(method,"contains"|"toggle"|"replace"){
   let token=token_list_reads::argument(args.first().ok_or("Token operation requires a converted DOMString.")?)?;
   let present=self.inert_token_read(object,token_list_reads::Read::Contains(token))?;
   if method=="contains"{return Ok(present);}
   let Value::Bool(present)=present else{unreachable!()};
   if method=="replace"&&!present{return Ok(Value::Bool(false));}
   if method=="toggle"{let desired=args.get(1).map_or(!present,Value::truthy);if desired!=present{self.inert_class_list_call(object,if desired{"add"}else{"remove"},&args[..1])?;}return Ok(Value::Bool(desired));}
  }
  if !matches!(method,"add"|"remove"|"replace"){return Err(format!("DOMTokenList method {method:?} is not implemented."));}
  let(value,absent)={
   let source=source(self.doc,&self.state.inert_documents,owner)?;
   let(source,absent)=source.value(script_class_list::attribute(object).ok_or("Inert token attribute is unavailable.")?)?;
   let value=if method=="replace"{token_list_mutation::replace(self.state,source,&args[..2])?}else{token_list_mutation::serialize(self.state,source,args,method=="add")?};
   (value,absent)
  };
  if !value.is_empty()||!absent{
   let attribute=script_class_list::attribute(object).ok_or("Inert token attribute is unavailable.")?;
   self.state.copies(attribute.len())?;
   let name=crate::html::AttributeName::unqualified(script_clone_node::copy_text(attribute)?);
   self.write_inert_dom_attribute_ns(owner,name,Some(value))?;
  }
  Ok(if method=="replace"{Value::Bool(true)}else{Value::Undefined})
 }
}
