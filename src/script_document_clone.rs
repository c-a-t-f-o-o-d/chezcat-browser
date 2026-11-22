//! Owned inert Document copies join the existing unpublished clone reservation.
use super::*;
#[cfg(test)]
mod storage_tests{
 use super::*;
 #[test]
 fn document_clone_exact_header_storage_modes_and_without_native_slots_keep_real_records(){
  let mut doc=crate::html::parse("<!doctype html><html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
  let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(empty)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::Null,Value::String(String::new())]).unwrap()else{panic!("empty XML")};let before=(b.state.retained,b.state.inert_documents.backing_bytes(),b.doc.nodes.len());
  let Value::Host(copy)=b.clone_inert_document(empty,false,true).unwrap()else{panic!("real copy")};let Kind::Document(record)=&b.state.inert_documents.node(copy).unwrap().kind else{unreachable!()};let payload=record.url.capacity()+record.base_url_override.as_ref().unwrap().capacity();assert_eq!(record.kind,crate::html::DocumentKind::Xml);assert_eq!(record.mode,crate::html::DocumentMode::NoQuirks);assert_eq!(b.state.retained-before.0,payload+b.state.inert_documents.backing_bytes()-before.1);assert_eq!(b.doc.nodes.len(),before.2);
  let Value::Host(parsed)=b.parse_inert_document(&[Value::String("<!doctype html PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\" \"original\"><html><head></head><body></body></html>".into()),Value::String("text/html".into())]).unwrap()else{panic!("parsed document")};let Value::Host(parsed_copy)=b.clone_inert_document(parsed,true,true).unwrap()else{panic!("mode copy")};for handle in [parsed,parsed_copy]{let Kind::Document(record)=&b.state.inert_documents.node(handle).unwrap().kind else{unreachable!()};assert_eq!(record.mode,crate::html::DocumentMode::LimitedQuirks);assert_eq!(record.kind,crate::html::DocumentKind::Html);}
  let Value::Host(xml)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::String("urn:original".into()),Value::String("Root".into())]).unwrap()else{panic!("XML")};let Value::Host(part)=b.create_cdata_section(xml,&[Value::String("data".into())]).unwrap()else{panic!("CDATA")};let Kind::Document(record)=&b.state.inert_documents.node(xml).unwrap().kind else{unreachable!()};let root=record.html.unwrap();b.mutate_inert_children(root,"appendChild",&[Value::Host(part)]).unwrap();b.state.node_limit=b.doc.nodes.len();let nodes=b.doc.nodes.len();
  let Value::Host(copy)=b.clone_inert_document(xml,true,true).unwrap()else{panic!("actual inert fallback")};let root=b.inert_children(copy).unwrap()[0];let part=b.inert_children(root).unwrap()[0];assert_eq!(b.doc.nodes.len(),nodes);assert!(node_index(part).is_none());assert_eq!(b.inert_node_type(part),Some(4));assert_eq!(b.state.inert_documents.node(root).unwrap().owner,copy);assert_eq!(b.state.inert_documents.node(part).unwrap().owner,copy);assert_eq!(b.inert_text_value(part).unwrap(),"data");
  let before=(b.state.retained,b.state.inert_documents.entries.len(),b.state.inert_documents.generated.len(),b.state.inert_documents.adopted.len(),b.doc.nodes.len());b.state.copy_work=0;assert!(b.clone_inert_document(xml,true,true).is_err());assert_eq!((b.state.retained,b.state.inert_documents.entries.len(),b.state.inert_documents.generated.len(),b.state.inert_documents.adopted.len(),b.doc.nodes.len()),before);b.state.copy_work=b.state.limits.copy_bytes;assert!(b.clone_inert_document(xml,true,true).is_ok());
 }
}
impl Bridge<'_>{
 pub(in crate::scripting) fn validate_document_clone(&self,object:u32,deep:bool)->Result<(),(&'static str,String)>{
  if !deep||!self.inert_document_children_receiver(object){return Ok(());}
  let failure=|message:&str|("NotSupportedError",message.to_owned());
  let mut pending=Vec::new();pending.try_reserve_exact(1).map_err(|_|failure("Cannot reserve document clone validation root."))?;pending.push(object);
  let maximum=self.state.inert_documents.entries.len().saturating_add(self.state.inert_documents.adopted.len());let mut visited=0usize;
  while let Some(node)=pending.pop(){
   visited=visited.checked_add(1).filter(|&count|count<=maximum).ok_or_else(||failure("Document clone source topology is inconsistent."))?;
   if self.template_receiver(node){return Err(failure("Document cloning of template contents requires separate cloned template-owner records; no document was published."));}
   if matches!(self.inert_import_source(node),Some(ImportSource::Parsed{node:Node{namespace:crate::html::Namespace::UnknownForeign,..},..})){return Err(failure("Document cloning requires known namespace provenance; no document was published."));}
   let children=self.inert_children(node).ok_or_else(||failure("Document clone validation node is unavailable."))?;
   pending.try_reserve(children.len()).map_err(|_|failure("Cannot reserve document clone validation traversal."))?;pending.extend(children.iter().rev().copied());
  }
  Ok(())
 }
 pub(super) fn first_inert_base_element(&mut self,owner:u32)->Result<Option<u32>,String>{
  self.state.copies(size_of::<u32>())?;
  let mut pending=Vec::new();pending.try_reserve_exact(1).map_err(|_|"Cannot reserve base traversal root.")?;pending.push(owner);
  let maximum=self.state.inert_documents.entries.len().saturating_add(self.state.inert_documents.adopted.len());let mut visited=0usize;
  while let Some(handle)=pending.pop(){
   visited=visited.checked_add(1).filter(|&count|count<=maximum).ok_or("Cyclic inert base topology.")?;self.state.work(1)?;
   if matches!(self.inert_import_source(handle),Some(ImportSource::Parsed{node:Node{namespace:crate::html::Namespace::UnknownForeign,..},..})){return Err("Inert base lookup requires known namespace provenance.".into());}
   let html_name=match self.inert_import_source(handle){Some(ImportSource::Element(name))=>Some(name),Some(ImportSource::Parsed{node:Node{kind:NodeKind::Element{tag,..},namespace:crate::html::Namespace::Html,..},..})=>Some(tag.as_str()),_=>None};
   let(is_base,is_template)=(html_name==Some("base"),html_name==Some("template"));
   if is_base{self.state.work(self.inert_element_attributes(handle).map_or(0,|attrs|attrs.len()))?;}
   if is_base&&self.inert_element_attributes(handle).is_some_and(|attrs|attrs.iter().any(|attr|attr.name.html_is("href"))){return Ok(Some(handle));}
   if is_template{continue;}
   let children=self.inert_children(handle).ok_or("Inert base traversal node is missing.")?;let count=children.len();self.state.work(count)?;self.state.copies(count.checked_mul(size_of::<u32>()).ok_or("Inert base traversal copy overflow.")?)?;
   let children=self.inert_children(handle).unwrap();pending.try_reserve(count).map_err(|_|"Cannot reserve base traversal children.")?;pending.extend(children.iter().rev().copied());
  }
  Ok(None)
 }
 pub(super) fn clone_inert_document(&mut self,object:u32,deep:bool,reserve_slots:bool)->Result<Value,String>{
  let Value::String(base)=self.inert_node_base_uri(object)?else{return Err("Document clone base URL is unavailable.".into());};
  let(kind,xml_interface,mode,content_type,url)=match &self.state.inert_documents.node(object).ok_or("Missing source Document.")?.kind{Kind::Document(record)=>{let bytes=record.url.len();let(kind,interface,mode,mime)=(record.kind,record.xml_interface,record.mode,record.content_type);self.state.copies(bytes)?;let Kind::Document(record)=&self.state.inert_documents.node(object).unwrap().kind else{unreachable!()};(kind,interface,mode,mime,script_clone_node::copy_text(&record.url)?)},Kind::TemplateDocument{kind,..}=>{let kind=*kind;self.state.copies("about:blank".len())?;(kind,false,crate::html::DocumentMode::NoQuirks,"application/xml",script_clone_node::copy_text("about:blank")?)},_=>return Err("Document clone requires a represented Document.".into())};
  let remaining=self.inert_remaining_entries();if remaining<2{return Err("Document clone identity backing allowance exhausted.".into());}
  let first=FIRST_ENTRY.checked_add(u32::try_from(self.state.inert_documents.entries.len()).map_err(|_|"Document clone identity overflow.")?).ok_or("Document clone handle overflow.")?;
  let initial=if deep{self.inert_children(object).unwrap().len()}else{0};if initial>remaining-2{return Err("Document clone child identity allowance exhausted.".into());}
  let mut pending=Vec::new();pending.try_reserve_exact(initial).map_err(|_|"Cannot reserve document clone traversal.")?;
  let mut children=Vec::new();children.try_reserve_exact(initial).map_err(|_|"Cannot reserve document clone root links.")?;
  self.state.work(initial)?;self.state.copies(initial.checked_mul(2*size_of::<u32>()).ok_or("Document clone child initialization overflow.")?)?;
  for (offset,&child) in self.inert_children(object).unwrap().iter().take(initial).enumerate(){pending.push((child,first,object));children.push(first+2+offset as u32);}
  let mut payload=url.capacity().checked_add(base.capacity()).and_then(|n|n.checked_add(children.capacity()*size_of::<u32>())).ok_or("Document clone payload overflow.")?;
  let mut staged=Vec::new();staged.try_reserve_exact(2).map_err(|_|"Cannot reserve document clone identities.")?;
  staged.push(Entry::Node(InertNode{kind:Kind::Document(DocumentNodes{xml_interface,kind,mode,content_type,url,base_url_override:Some(base),implementation:first+1,doctype:None,html:None,head:None,body:None,title:String::new()}),owner:first,parent:None,children}));staged.push(Entry::Implementation);
  let mut at=0usize;
  while let Some(&(source,parent,source_parent))=pending.get(at){
   let node=self.state.inert_documents.node(source).filter(|node|node.owner==object&&node.parent==Some(source_parent)).ok_or("Document clone source owner or edge is inconsistent.")?;
   let count=node.children.len();let copy=match &node.kind{Kind::Element(name)=>name.len(),Kind::Text(data)=>data.used_bytes(),Kind::Doctype(data)=>data.used_bytes(),Kind::ProcessingInstruction{target,data}=>target.len().checked_add(data.used_bytes()).ok_or("Document clone character size overflow.")?,_=>0};
   self.state.work(count.saturating_add(1))?;self.state.copies(copy.checked_add(count.checked_mul(2*size_of::<u32>()).ok_or("Document clone edges overflow.")?).ok_or("Document clone copy size overflow.")?)?;
   if self.template_receiver(source){return Err("Document cloning of template contents requires separate cloned template-owner records; no document was published.".into());}
   if count>remaining-2-pending.len(){return Err("Document clone descendant identity allowance exhausted.".into());}
   pending.try_reserve(count).map_err(|_|"Cannot reserve document clone descendant traversal.")?;staged.try_reserve(1).map_err(|_|"Cannot reserve document clone descendant record.")?;
   let handle=first+2+at as u32;let child_start=first+2+pending.len() as u32;
   let node=self.state.inert_documents.node(source).unwrap();let mut children=Vec::new();children.try_reserve_exact(count).map_err(|_|"Cannot reserve cloned child links.")?;children.extend(child_start..child_start+count as u32);pending.extend(node.children.iter().map(|&child|(child,handle,source)));
   let kind=match &node.kind{
    Kind::Element(name)=>Kind::Element(script_clone_node::copy_text(name)?),Kind::Text(data)=>Kind::Text(data.try_clone()?),Kind::Doctype(data)=>Kind::Doctype(data.try_clone()?),Kind::ProcessingInstruction{target,data}=>Kind::ProcessingInstruction{target:script_clone_node::copy_text(target)?,data:data.try_clone()?},Kind::Parsed{source,..}=>Kind::Parsed{source:*source,initial_template:None},_=>return Err("Unsupported node kind in Document clone source.".into()),
   };
   let owned=match &kind{Kind::Element(name)=>name.capacity(),Kind::Text(data)=>data.capacity(),Kind::Doctype(data)=>data.capacity(),Kind::ProcessingInstruction{target,data}=>target.capacity().checked_add(data.capacity()).ok_or("Document clone owned character size overflow.")?,_=>0};
   payload=payload.checked_add(owned).and_then(|n|n.checked_add(children.capacity()*size_of::<u32>())).ok_or("Document clone descendant payload overflow.")?;
   if parent==first{let node_type=self.inert_node_type(source);let Entry::Node(doc)=&mut staged[0]else{unreachable!()};let Kind::Document(record)=&mut doc.kind else{unreachable!()};match node_type{Some(1)=>record.html=Some(handle),Some(10)=>record.doctype=Some(handle),_=>{}}}
   staged.push(Entry::Node(InertNode{kind,owner:first,parent:Some(parent),children}));at+=1;
  }
  self.publish_inert_clone(first,staged,None,payload,Vec::new(),reserve_slots)
 }
}
