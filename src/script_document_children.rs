//! Checked document child ordering shared by represented inert mutations.
use super::*;
#[cfg(test)]
mod storage_tests{
 use super::*;
 #[test]
 fn document_children_joint_list_refusal_and_replacement_copy_refusal_preserve_both_trees(){
  let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
  let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(owner)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("XML document")};let Value::Host(root)=b.create_xml_element(owner,&[Value::String("Root".into())]).unwrap()else{panic!("real root")};
  let before=(b.state.retained,b.state.dom_commits,b.state.inert_documents.entries.len(),b.doc.nodes.len());let limit=b.state.retained_limit;let copies=b.state.copy_work;
  for storage in [true,false]{if storage{b.state.retained_limit=b.state.retained;}else{b.state.copy_work=0;}
   assert!(b.mutate_inert_children(owner,"appendChild",&[Value::Host(root)]).is_err());assert_eq!((b.state.retained,b.state.dom_commits,b.state.inert_documents.entries.len(),b.doc.nodes.len()),before);assert!(b.inert_children(owner).unwrap().is_empty());assert_eq!(b.state.inert_documents.node(root).unwrap().parent,None);let Kind::Document(record)=&b.state.inert_documents.node(owner).unwrap().kind else{unreachable!()};assert_eq!(record.html,None);
   b.state.retained_limit=limit;b.state.copy_work=copies;
  }
  b.mutate_inert_children(owner,"appendChild",&[Value::Host(root)]).unwrap();assert_eq!(b.state.retained-before.0,b.state.inert_documents.node(owner).unwrap().children.capacity()*size_of::<u32>());
  let Value::Host(fragment)=b.inert_create_node(owner,"createDocumentFragment",&[]).unwrap()else{panic!("fragment")};let Value::Host(next)=b.create_xml_element(owner,&[Value::String("Next".into())]).unwrap()else{panic!("next root")};b.mutate_inert_children(fragment,"appendChild",&[Value::Host(next)]).unwrap();let before=(b.state.retained,b.state.dom_commits);b.state.copy_work=0;
  assert!(b.replace_inert_child(owner,&[Value::Host(fragment),Value::Host(root)]).is_err());assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.inert_children(owner).unwrap(),[root]);assert_eq!(b.inert_children(fragment).unwrap(),[next]);assert_eq!(b.state.inert_documents.node(next).unwrap().parent,Some(fragment));
  b.state.copy_work=copies;b.replace_inert_child(owner,&[Value::Host(fragment),Value::Host(root)]).unwrap();assert_eq!(b.inert_children(owner).unwrap(),[next]);assert!(b.inert_children(fragment).unwrap().is_empty());let Kind::Document(record)=&b.state.inert_documents.node(owner).unwrap().kind else{unreachable!()};assert_eq!(record.html,Some(next));assert_eq!(b.state.inert_documents.node(root).unwrap().parent,None);
 }
}
impl Bridge<'_>{
 pub(in crate::scripting) fn inert_document_children_receiver(&self,parent:u32)->bool{
  self.state.inert_documents.node(parent).is_some_and(|node|matches!(node.kind,Kind::Document(_)))
 }
 pub(super) fn validate_document_children(&self,parent:u32,method:&str,args:&[Value])->Result<(),(&'static str,String)>{
  if !self.inert_document_children_receiver(parent)||method=="removeChild"{return Ok(());}
  let Some(Value::Host(incoming))=args.first()else{return Ok(());};
  let children=self.inert_children(parent).unwrap();
  let reference=if matches!(method,"insertBefore"|"replaceChild"){args.get(1).and_then(|v|if let Value::Host(handle)=v{Some(*handle)}else{None})}else{None};
  let insertion=reference.and_then(|reference|children.iter().position(|&child|child==reference)).unwrap_or(children.len());
  let outgoing=if method=="replaceChild"{reference}else{None};
  let reject=||("HierarchyRequestError","Document pre-insertion validity forbids this node, element/doctype count, or order.".into());
  let kind=self.inert_node_type(*incoming);
  if matches!(kind,Some(3|4)){return Err(reject());}
  if matches!(kind,Some(7|8)){return Ok(());}
  let mut element=kind==Some(1);
  if kind==Some(11){
   element=false;
   for &node in self.inert_children(*incoming).unwrap_or(&[]){match self.inert_node_type(node){Some(1) if !element=>element=true,Some(7|8)=>{},_=>return Err(reject())}}
   if !element{return Ok(());}
  }
  let Kind::Document(record)=&self.state.inert_documents.node(parent).unwrap().kind else{unreachable!()};
  // The current pinned DOM excludes only the replaced child. It does not
  // exclude an incoming node already in this document from validity checks.
  if element{
   if record.html.is_some_and(|root|Some(root)!=outgoing){return Err(reject());}
   if let Some(doctype)=record.doctype{
    let at=children.iter().position(|&node|node==doctype).unwrap();
    if reference.is_some()&&(at>insertion||at==insertion&&Some(doctype)!=outgoing){return Err(reject());}
   }
  }else if kind==Some(10){
   if record.doctype.is_some_and(|doctype|Some(doctype)!=outgoing){return Err(reject());}
   if let Some(root)=record.html{
    let at=children.iter().position(|&node|node==root).unwrap();
    if reference.is_some()&&at<insertion||reference.is_none()&&Some(root)!=outgoing{return Err(reject());}
   }
  }else{return Err(reject());}
  Ok(())
 }
 pub(super) fn prepare_document_children_sync(&mut self,parent:u32,added:usize)->Result<(),String>{
  if self.inert_document_children_receiver(parent){
   let count=self.inert_children(parent).unwrap().len().checked_add(added).ok_or("Document metadata traversal count overflow.")?;
   self.state.work(count.checked_mul(self.state.inert_documents.adopted.lookup_work()+1).ok_or("Document metadata traversal work overflow.")?)?;
  }
  Ok(())
 }
 // Called after admission and native edge publication; no allocation or callback.
 pub(super) fn sync_document_children(&mut self,parent:u32){
  if !self.inert_document_children_receiver(parent){return;}
  let mut element=None;let mut doctype=None;
  for &node in self.inert_children(parent).unwrap(){match self.inert_node_type(node){Some(1)=>element=Some(node),Some(10)=>doctype=Some(node),_=>{}}}
  let Kind::Document(record)=&mut self.state.inert_documents.node_mut(parent).unwrap().kind else{unreachable!()};record.html=element;record.doctype=doctype;
 }
}
