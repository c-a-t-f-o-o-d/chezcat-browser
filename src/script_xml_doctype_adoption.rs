//! The factory moves the actual leaf; its payload is never copied or reidentified.
use super::*;
pub(super) struct Adoption{
 pub(super) handle:u32,
 pub(super) native:Option<usize>,
 parent:Option<u32>,
 slot:Option<usize>,
}

#[cfg(test)]
mod storage_tests{
 use super::*;
 #[test]
 fn xml_doctype_adoption_late_copy_refusal_publishes_no_owner_index_or_document(){
  let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
  let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let index=bridge.doc.nodes[0].children[0];let handle=node_handle(index);
  let before=(bridge.state.retained,bridge.state.inert_documents.backing_bytes(),bridge.state.inert_documents.entries.len(),bridge.state.inert_documents.adopted.len(),bridge.state.dom_commits,bridge.doc.nodes[0].children.clone());
  // Admit removal movement and the unpublished index row, then deny the
  // document entries/payload copy. Earlier source state must remain visible.
  bridge.state.copy_work=size_of::<usize>()+size_of::<(u32,InertNode)>();
  assert!(bridge.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new()),Value::Host(handle)]).is_err());
  assert_eq!((bridge.state.retained,bridge.state.inert_documents.backing_bytes(),bridge.state.inert_documents.entries.len(),bridge.state.inert_documents.adopted.len(),bridge.state.dom_commits,bridge.doc.nodes[0].children.clone()),before);
  assert_eq!(bridge.state.tree.parents[index],Some(0));assert!(bridge.dom_accessible(index));
  bridge.state.copy_work=bridge.state.limits.copy_bytes;
  let Value::Host(owner)=bridge.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new()),Value::Host(handle)]).unwrap()else{panic!("recovered XML")};assert_eq!(bridge.state.inert_documents.node(handle).unwrap().owner,owner);
 }
 #[test]
 fn xml_doctype_adoption_charges_index_and_keeps_owned_payload_without_copying(){
  let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
  let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
  let Value::Host(handle)=bridge.create_document_type(MAIN_IMPLEMENTATION,&[Value::String("Root".into()),Value::String("original".repeat(100)),Value::String("source".repeat(100))]).unwrap()else{panic!("real doctype")};
  let index=node_index(handle).unwrap();let original=bridge.doc.nodes[index].kind.character_data();assert!(original.is_none());
  let NodeKind::DocumentType(data)=&bridge.doc.nodes[index].kind else{unreachable!()};let pointer=data.public_id.as_str().as_ptr();
  let before=(bridge.state.retained,bridge.state.inert_documents.backing_bytes(),bridge.doc.nodes.len());
  let Value::Host(owner)=bridge.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new()),Value::Host(handle)]).unwrap()else{panic!("real XML")};
  let arena=&bridge.state.inert_documents;let record=arena.node(owner).unwrap();let Kind::Document(data)=&record.kind else{unreachable!()};
  let payload=data.url.capacity()+record.children.capacity()*size_of::<u32>();
  assert_eq!(bridge.state.retained-before.0,payload+arena.backing_bytes()-before.1);assert_eq!(bridge.doc.nodes.len(),before.2);
  let NodeKind::DocumentType(data)=&bridge.doc.nodes[index].kind else{unreachable!()};assert_eq!(data.public_id.as_str().as_ptr(),pointer);assert_eq!(data.public_id,"original".repeat(100));
  let Kind::Parsed{source:ParsedSource::Loaded(actual),..}=arena.node(handle).unwrap().kind else{panic!("same native source")};assert_eq!(actual,index);assert_eq!(arena.node(handle).unwrap().owner,owner);
 }
}
impl Bridge<'_>{
 pub(super) fn prepare_xml_doctype_adoption(&mut self,handle:u32)->Result<Adoption,String>{
  self.state.rebuild(self.doc)?;
  let(native,parent)=if let Some(node)=self.state.inert_documents.node(handle){
   if self.inert_node_type(handle)!=Some(10)||!node.children.is_empty(){return Err("XML adoption requires a genuine leaf DocumentType.".into());}
   (None,node.parent)
  }else{
   let index=node_index(handle).filter(|&index|self.dom_accessible(index)&&matches!(self.doc.nodes[index].kind,NodeKind::DocumentType(_))).ok_or("XML adoption requires an accessible DocumentType.")?;
   if !self.doc.nodes[index].children.is_empty(){return Err("DocumentType unexpectedly has children.".into());}
   let parent=self.state.tree.parents[index];
   if parent.is_some_and(|parent|parent!=0){return Err("DocumentType has an invalid native parent.".into());}
   (Some(index),parent.map(|_|DOCUMENT))
  };
  let slot=if let Some(parent)=parent{
   let children=if parent==DOCUMENT{&self.doc.nodes[0].children}else{
    let owner=self.state.inert_documents.node(parent).ok_or("Missing doctype source parent.")?;
    if !matches!(owner.kind,Kind::Document(_)){return Err("DocumentType source parent must be a Document.".into());}
    self.state.work(owner.children.len())?;
    let children=self.inert_children(parent).unwrap();
    let slot=children.iter().position(|&child|child==handle).ok_or("Missing inert doctype child link.")?;
    self.state.copies((children.len()-slot-1).checked_mul(size_of::<u32>()).ok_or("Doctype source removal copy overflow.")?)?;
    return Ok(Adoption{handle,native,parent:Some(parent),slot:Some(slot)});
   };
   self.state.work(children.len())?;
   let index=native.ok_or("Main doctype source lost its native identity.")?;
   let children=&self.doc.nodes[0].children;
   let slot=children.iter().position(|&child|child==index).ok_or("Missing native doctype child link.")?;
   self.state.copies((children.len()-slot-1).checked_mul(size_of::<usize>()).ok_or("Native doctype source removal copy overflow.")?)?;
   self.state.prepare_range_changes(1,1)?;Some(slot)
  }else{None};
  Ok(Adoption{handle,native,parent,slot})
 }
 pub(super) fn commit_xml_doctype_adoption(&mut self,plan:Adoption,owner:u32,growth:Option<Vec<(u32,InertNode)>>){
  if let Some(parent)=plan.parent{
   let slot=plan.slot.unwrap();
   if parent==DOCUMENT{
    let index=plan.native.unwrap();self.state.ranges_remove(0,slot,1,&[index]);self.doc.nodes[0].children.remove(slot);self.state.dirty=true;
   }else{
    let parent=self.state.inert_documents.node_mut(parent).unwrap();parent.children.remove(slot);
    let Kind::Document(record)=&mut parent.kind else{unreachable!()};record.doctype=None;
   }
  }
  let arena=&mut self.state.inert_documents;
  if let Some(index)=plan.native{
   arena.adopted.commit(growth);arena.adopted.insert(plan.handle,InertNode{kind:Kind::Parsed{source:ParsedSource::Loaded(index),initial_template:None},owner,parent:Some(owner),children:Vec::new()});
   self.state.tree.parents[index]=None;self.state.tree.active[index]=false;self.state.dom.connected[index]=false;
  }else{
   let node=arena.node_mut(plan.handle).unwrap();node.owner=owner;node.parent=Some(owner);
  }
  self.state.dom_committed();
 }
 pub(in crate::scripting) fn xml_doctype_adoption_changes(&mut self,args:&[Value])->Result<Vec<crate::javascript::NativeChildListChange>,String>{
  let Some(Value::Host(handle))=args.get(2)else{return Ok(Vec::new());};
  let plan=self.prepare_xml_doctype_adoption(*handle)?;
  let Some(parent)=plan.parent else{return Ok(Vec::new());};let slot=plan.slot.unwrap();
  self.state.copies(size_of::<crate::javascript::NativeChildListChange>()+size_of::<u32>())?;
  let(previous,next)=if parent==DOCUMENT{let children=&self.doc.nodes[0].children;(slot.checked_sub(1).map(|i|node_handle(children[i])),children.get(slot+1).copied().map(node_handle))}else{let children=self.inert_children(parent).unwrap();(slot.checked_sub(1).map(|i|children[i]),children.get(slot+1).copied())};
  let mut removed=Vec::new();removed.try_reserve_exact(1).map_err(|_|"Cannot reserve adopted doctype observation.")?;removed.push(*handle);
  let mut changes=Vec::new();changes.try_reserve_exact(1).map_err(|_|"Cannot reserve doctype removal record.")?;changes.push(crate::javascript::NativeChildListChange{target:parent,added:Vec::new(),removed,previous,next});Ok(changes)
 }
}
