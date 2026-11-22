//! NonElementParentNode ID lookup reads exact native attributes/tree order.
//! https://dom.spec.whatwg.org/#dom-nonelementparentnode-getelementbyid
//! Separate operation brands follow https://webidl.spec.whatwg.org/#es-operations.
use super::*;
use crate::dom_string::DomStringRef;
pub(in crate::scripting) const DOCUMENT_METHOD:&str="\0Document.getElementById";
pub(in crate::scripting) const FRAGMENT_METHOD:&str="\0DocumentFragment.getElementById";
pub(in crate::scripting) fn operation(method:&str)->Option<bool>{match method{DOCUMENT_METHOD=>Some(true),FRAGMENT_METHOD=>Some(false),_=>None}}
impl Bridge<'_>{
 pub(in crate::scripting) fn identifier_lookup_receiver(&self,object:u32,document:Option<bool>)->bool{
  let kind=self.inert_node_type(object);
  let doc=object==DOCUMENT||kind==Some(9)||self.frame_contains_node(object).is_some_and(|(_,node)|node==0);
  let fragment=kind==Some(11)||node_index(object).is_some_and(|node|self.dom_accessible(node)&&matches!(self.doc.nodes[node].kind,NodeKind::DocumentFragment));
  match document{Some(true)=>doc,Some(false)=>fragment,None=>doc||fragment}
 }
 pub(in crate::scripting) fn validate_identifier_lookup(&self,object:u32,document:bool)->Result<(),(&'static str,String)>{
  if !self.identifier_lookup_receiver(object,Some(document)){return Err(("TypeError","getElementById requires its genuine Document or DocumentFragment receiver.".into()));}
  self.native_node_access(object)
 }
 fn identifier_attributes(&self,object:u32)->Result<Option<&[crate::html::Attribute]>,String>{
  if let Some((owner,node))=self.frame_contains_node(object){return Ok(match &self.contains_child_document(owner)?.nodes[node].kind{NodeKind::Element{attrs,..}=>Some(attrs),_=>None});}
  Ok(self.element_attribute_source(object))
 }
 fn identifier_matches(&mut self,object:u32,query:DomStringRef<'_>)->Result<bool,String>{
  let Some(attrs)=self.identifier_attributes(object)?else{return Ok(false);};
  let work=attrs.iter().try_fold(attrs.len(),|n,a|n.checked_add(a.name.bytes()).ok_or("Identifier attribute lookup work overflow."))?;self.state.work(work)?;
  let Some(at)=self.identifier_attributes(object)?.unwrap().iter().position(|attr|attr.name.html_is("id"))else{return Ok(false);};
  let value=&self.identifier_attributes(object)?.unwrap()[at].value;let cost=value.used_bytes().min(query.logical_bytes()).checked_add(1).ok_or("Identifier value comparison work overflow.")?;self.state.work(cost)?;
  Ok(self.identifier_attributes(object)?.unwrap()[at].value.code_units().eq(query.code_units()))
 }
 pub(in crate::scripting) fn native_identifier_lookup(&mut self,object:u32,args:&[Value],document:bool)->Result<Value,String>{
  self.validate_identifier_lookup(object,document).map_err(|(_,e)|e)?;
  let query=match args.first(){Some(Value::String(s))=>DomStringRef::Scalar(s),Some(Value::StringUnits(units))=>DomStringRef::Raw(units),_=>return Err("getElementById requires its converted DOMString argument.".into())};
  self.state.work(self.state.inert_documents.adopted.lookup_work().checked_mul(4).and_then(|n|n.checked_add(1)).ok_or("Identifier receiver lookup work overflow.")?)?;
  if query.is_empty(){return Ok(Value::Null);}
  if object==DOCUMENT{
   self.state.rebuild(self.doc)?;
   // Retain the existing logarithmic index for ordinary native queries. Verify
   // its actual value: a raw-unit projection must not hide a later real match.
   if let DomStringRef::Scalar(id)=query{if id.len()<=self.state.limits.host.string_bytes{
    let Some(node)=self.state.document_id(self.doc,id)?else{return Ok(Value::Null);};
    if self.identifier_matches(node_handle(node),query)?{return Ok(Value::Host(node_handle(node)));}
   }}
   // Native order already excludes unsupported parser/lifecycle backings and
   // disconnected roots. Raw/large queries avoid scalar ID projection aliases.
   for at in 0..self.state.tree.order.len(){self.state.work(1)?;let index=self.state.tree.order[at];if index==0{continue;}let node=node_handle(index);if self.identifier_matches(node,query)?{return Ok(Value::Host(node));}}
   return Ok(Value::Null);
  }
  if self.inert_node_type(object).is_none()&&!self.is_frame_handle(object){self.state.rebuild(self.doc)?;}
  let mut stack=self.equality_children(object)?;stack.reverse();let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(self.state.blank_frame_nodes)).and_then(|n|n.checked_add(1)).ok_or("Identifier traversal bound overflow.")?;let mut visited=0usize;
  while let Some(node)=stack.pop(){visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic identifier tree.")?;self.state.work(1)?;self.native_node_access(node).map_err(|(_,e)|e)?;
   if self.identifier_matches(node,query)?{return Ok(Value::Host(node));}
   let children=self.equality_children(node)?;self.state.copies(children.len().checked_mul(size_of::<u32>()).ok_or("Identifier traversal scratch overflow.")?)?;stack.try_reserve(children.len()).map_err(|_|"Cannot reserve identifier traversal stack.")?;stack.extend(children.into_iter().rev());
  }
  Ok(Value::Null)
 }
}
