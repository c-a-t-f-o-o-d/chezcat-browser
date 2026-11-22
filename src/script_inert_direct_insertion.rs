//! Direct sources use the existing checked root sink, never a fabricated fragment.
use super::*;
#[cfg(test)]
mod storage_tests{
 use super::*;
 #[test]
 fn inert_direct_insertion_refusal_preserves_document_root_and_success_allocates_no_fragment_identity(){
  let mut doc=crate::html::parse("<!doctype html><html><body><div id=mount><span>one</span><span>two</span></div></body></html>");doc.scripting_enabled=true;let mount=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="mount")).unwrap();let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
  let Value::Host(owner)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::String(HTML_NAMESPACE.into()),Value::String("html".into())]).unwrap()else{panic!("source document")};let Value::Host(source)=b.create_expanded_element(owner,&[Value::String(HTML_NAMESPACE.into()),Value::String("div".into())]).unwrap()else{panic!("source leaf")};let Kind::Document(record)=&b.state.inert_documents.node(owner).unwrap().kind else{unreachable!()};let previous=record.html.unwrap();b.replace_inert_child(owner,&[Value::Host(source),Value::Host(previous)]).unwrap();
  let before=(b.state.retained,b.state.dom_commits,b.state.inert_documents.entries.len(),b.state.inert_documents.generated.len(),b.doc.nodes.len(),b.doc.nodes[mount].children.clone());let limit=b.state.retained_limit;let copies=b.state.copy_work;
  for storage in [true,false]{if storage{b.state.retained_limit=b.state.retained;}else{b.state.copy_work=0;}assert!(b.call(node_handle(mount),"appendChild",&[Value::Host(source)]).is_err());assert_eq!((b.state.retained,b.state.dom_commits,b.state.inert_documents.entries.len(),b.state.inert_documents.generated.len(),b.doc.nodes.len(),b.doc.nodes[mount].children.clone()),before);let Kind::Document(record)=&b.state.inert_documents.node(owner).unwrap().kind else{unreachable!()};assert_eq!(record.html,Some(source));assert_eq!(b.state.inert_documents.node(source).unwrap().parent,Some(owner));assert_eq!(b.state.inert_documents.node(source).unwrap().owner,owner);b.state.retained_limit=limit;b.state.copy_work=copies;}
  assert!(matches!(b.call(node_handle(mount),"appendChild",&[Value::Host(source)]).unwrap(),Value::Host(handle) if handle==source));assert_eq!(b.state.inert_documents.entries.len(),before.2);assert_eq!(b.state.inert_documents.generated.len(),before.3);assert_eq!(b.doc.nodes.len(),before.4);assert!(!b.inert_adopted_node(source));assert_eq!(b.state.tree.parents[node_index(source).unwrap()],Some(mount));let Kind::Document(record)=&b.state.inert_documents.node(owner).unwrap().kind else{unreachable!()};assert_eq!(record.html,None);assert!(b.inert_children(owner).unwrap().is_empty());
 }
}
impl Bridge<'_>{
 pub(in crate::scripting) fn validate_inert_direct_insertion(&self,receiver:u32,source:u32,method:&str,args:&[Value])->Result<(),(&'static str,String)>{
  let parent=node_index(receiver).filter(|&node|self.dom_accessible(node)).ok_or(("TypeError","Inert insertion destination is unavailable.".into()))?;
  if !matches!(self.doc.nodes[parent].kind,NodeKind::Element{..}|NodeKind::DocumentFragment){return Err(("HierarchyRequestError","This Node cannot contain children.".into()));}
  if !matches!(self.inert_node_type(source),Some(1|3|4|7|8)){return Err(("HierarchyRequestError","This Node cannot be inserted under an Element or fragment.".into()));}
  if self.doc.nodes[parent].namespace!=crate::html::Namespace::Html&&!self.fragment_receiver(receiver)||matches!(tag(self.doc,parent),"script"|"link"){return Err(("NotSupportedError","Direct inert insertion requires an ordinary HTML or native fragment destination.".into()));}
  if method=="insertBefore"{match args.get(1){Some(Value::Null|Value::Undefined)=>{},Some(Value::Host(reference)) if self.get_root_receiver(*reference)=>{if !node_index(*reference).is_some_and(|node|self.dom_accessible(node)&&self.state.tree.parents[node]==Some(parent)){return Err(("NotFoundError","The reference Node is not a child of this destination.".into()));}},_=>return Err(("TypeError","insertBefore requires a Node or null reference.".into()))}}
  let owner=self.state.inert_documents.node(source).ok_or(("TypeError","Missing inert insertion source.".into()))?.owner;
  let mut pending=Vec::new();pending.try_reserve_exact(1).map_err(|_|("NotSupportedError","Cannot reserve direct insertion validation.".into()))?;pending.push(source);let mut visited=0usize;let maximum=self.inert_node_count();
  while let Some(handle)=pending.pop(){
   visited=visited.checked_add(1).filter(|&count|count<=maximum).ok_or(("NotSupportedError","Direct insertion topology is inconsistent.".into()))?;
   let(node,raw,_)=self.parked_template_source(handle).ok_or(("NotSupportedError","Direct insertion requires an existing reserved native identity; this producer is not materialized.".into()))?;
   if node.owner!=owner{return Err(("NotSupportedError","Direct insertion source owners are inconsistent.".into()));}
   let raw=self.parsed_source(raw).ok_or(("NotSupportedError","Direct insertion source payload is unavailable.".into()))?.0;
   if !template_node_can_park(raw){return Err(("NotSupportedError","Direct insertion requires unsupported activation capabilities or namespace provenance.".into()));}
   pending.try_reserve(node.children.len()).map_err(|_|("NotSupportedError","Cannot reserve direct insertion descendants.".into()))?;pending.extend(node.children.iter().rev().copied());
  }
  Ok(())
 }
 pub(in crate::scripting) fn inert_direct_insertion_changes(&mut self,receiver:u32,method:&str,args:&[Value])->Result<Vec<crate::javascript::NativeChildListChange>,String>{
  let Some(Value::Host(source))=args.first()else{return Ok(Vec::new());};self.validate_inert_direct_insertion(receiver,*source,method,args).map_err(|(_,message)|message)?;
  let parent=node_index(receiver).unwrap();let reference=if method=="insertBefore"{if let Some(Value::Host(reference))=args.get(1){node_index(*reference)}else{None}}else{None};
  let siblings=&self.doc.nodes[parent].children;let length=siblings.len();let at=reference.and_then(|node|siblings.iter().position(|&child|child==node)).unwrap_or(length);let previous=at.checked_sub(1).map(|index|node_handle(siblings[index]));let next=siblings.get(at).copied().map(node_handle);
  self.state.work(length)?;self.state.copies(2*size_of::<crate::javascript::NativeChildListChange>()+2*size_of::<u32>())?;
  let mut changes=Vec::new();changes.try_reserve_exact(2).map_err(|_|"Cannot reserve direct insertion observations.")?;
  if let Some(old)=self.state.inert_documents.node(*source).unwrap().parent{let children=self.inert_children(old).ok_or("Direct insertion observer source parent is unavailable.")?;let length=children.len();let at=children.iter().position(|node|node==source).ok_or("Direct insertion observer source link is unavailable.")?;let previous=at.checked_sub(1).map(|index|children[index]);let next=children.get(at+1).copied();self.state.work(length)?;let mut removed=Vec::new();removed.try_reserve_exact(1).map_err(|_|"Cannot reserve direct removal identity.")?;removed.push(*source);changes.push(crate::javascript::NativeChildListChange{target:old,added:Vec::new(),removed,previous,next});}
  let mut added=Vec::new();added.try_reserve_exact(1).map_err(|_|"Cannot reserve direct insertion identity.")?;added.push(*source);changes.push(crate::javascript::NativeChildListChange{target:receiver,added,removed:Vec::new(),previous,next});Ok(changes)
 }
}
