//! Template-owner document mutations use actual shared list storage admission.

 use super::*;
 #[test]
 fn template_document_children_joint_storage_copy_refusal_preserve_metadata_and_recover(){
  let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
  let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(xml)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("XML document")};let Value::Host(host)=b.create_expanded_element(xml,&[Value::String(HTML_NAMESPACE.into()),Value::String("template".into())]).unwrap()else{panic!("host")};let Value::Host(content)=b.template_content(host).unwrap()else{panic!("content")};let owner=b.state.inert_documents.node(content).unwrap().owner;let Value::Host(root)=b.create_xml_element(owner,&[Value::String("Root".into())]).unwrap()else{panic!("real root")};
  let before=(b.state.retained,b.state.dom_commits,b.state.inert_documents.entries.len(),b.doc.nodes.len());let limit=b.state.retained_limit;let copies=b.state.copy_work;
  for storage in [true,false]{if storage{b.state.retained_limit=b.state.retained;}else{b.state.copy_work=0;}
   assert!(b.mutate_inert_children(owner,"appendChild",&[Value::Host(root)]).is_err());assert_eq!((b.state.retained,b.state.dom_commits,b.state.inert_documents.entries.len(),b.doc.nodes.len()),before);assert!(b.inert_children(owner).unwrap().is_empty());assert_eq!(b.state.inert_documents.node(root).unwrap().parent,None);assert_eq!(b.inert_document_roots(owner),Some((None,None)));
   b.state.retained_limit=limit;b.state.copy_work=copies;
  }
  b.mutate_inert_children(owner,"appendChild",&[Value::Host(root)]).unwrap();assert_eq!(b.state.retained-before.0,b.state.inert_documents.node(owner).unwrap().children.capacity()*size_of::<u32>());
  let Value::Host(fragment)=b.inert_create_node(owner,"createDocumentFragment",&[]).unwrap()else{panic!("fragment")};let Value::Host(next)=b.create_xml_element(owner,&[Value::String("Next".into())]).unwrap()else{panic!("next root")};b.mutate_inert_children(fragment,"appendChild",&[Value::Host(next)]).unwrap();let before=(b.state.retained,b.state.dom_commits);b.state.copy_work=0;
  assert!(b.replace_inert_child(owner,&[Value::Host(fragment),Value::Host(root)]).is_err());assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.inert_children(owner).unwrap(),[root]);assert_eq!(b.inert_children(fragment).unwrap(),[next]);assert_eq!(b.state.inert_documents.node(next).unwrap().parent,Some(fragment));
  b.state.copy_work=copies;b.replace_inert_child(owner,&[Value::Host(fragment),Value::Host(root)]).unwrap();assert_eq!(b.inert_children(owner).unwrap(),[next]);assert!(b.inert_children(fragment).unwrap().is_empty());assert_eq!(b.inert_document_roots(owner),Some((Some(next),None)));assert_eq!(b.state.inert_documents.node(root).unwrap().parent,None);
 }
