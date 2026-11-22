//! Generic document admission accounts real headers and payload.
use super::*;
#[test]
fn generic_document_constructor_exact_storage_refusal_and_recovery_are_atomic(){
 let mut doc=crate::html::parse("<html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);page.begin_job();let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
 let before=(b.state.retained,b.state.inert_documents.backing_bytes(),b.state.inert_documents.entries.len(),b.doc.nodes.len());let limit=b.state.retained_limit;let copies=b.state.copy_work;
 for storage in [true,false]{if storage{b.state.retained_limit=before.0;}else{b.state.copy_work=0;}assert!(b.create_generic_document().is_err());assert_eq!((b.state.retained,b.state.inert_documents.backing_bytes(),b.state.inert_documents.entries.len(),b.doc.nodes.len()),before);b.state.retained_limit=limit;b.state.copy_work=copies;}
 let Value::Host(document)=b.create_generic_document().unwrap()else{panic!("genuine document")};let record=b.state.inert_documents.node(document).unwrap();assert_eq!(record.owner,document);let Kind::Document(record)=&record.kind else{panic!("document record")};assert!(!record.xml_interface);assert_eq!(record.kind,crate::html::DocumentKind::Xml);assert_eq!(b.state.retained-before.0,record.url.capacity()+b.state.inert_documents.backing_bytes()-before.1);assert_eq!(b.state.inert_documents.entries.len(),before.2+2);assert_eq!(b.doc.nodes.len(),before.3);
}
