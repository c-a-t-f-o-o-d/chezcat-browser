//! Original native XML document factory. Parsing XML bytes is owned by F08.
use super::*;
#[path="script_xml_doctype_adoption.rs"]
mod doctype_adoption;
pub(in crate::scripting) const CREATE:&str="\0DOMImplementation.createDocument";
impl Bridge<'_>{
    fn document_element_named(&self,element:u32,namespace:crate::html::Namespace,name:&str)->bool{
        match self.inert_import_source(element){Some(ImportSource::Element(tag))=>namespace==crate::html::Namespace::Html&&tag==name,Some(ImportSource::Parsed{node,..})=>node.namespace==namespace&&matches!(&node.kind,NodeKind::Element{tag,..} if tag==name),_=>false}
    }
    pub(in crate::scripting) fn inert_document_structure_get(&mut self,document:u32,property:&str)->Result<Value,String>{
        let Kind::Document(record)=&self.state.inert_documents.node(document).ok_or("Missing document record.")?.kind else{return Ok(Value::Null);};
        let Some(root)=record.html else{return Ok(Value::Null);};
        if !self.document_element_named(root,crate::html::Namespace::Html,"html"){return Ok(Value::Null);}
        let children=self.inert_children(root).ok_or("Missing document root children.")?;self.state.work(children.len())?;
        let children=self.inert_children(root).unwrap();
        Ok(children.iter().copied().find(|&child|if property=="head"{self.document_element_named(child,crate::html::Namespace::Html,"head")}else{self.document_element_named(child,crate::html::Namespace::Html,"body")||self.document_element_named(child,crate::html::Namespace::Html,"frameset")}).map_or(Value::Null,Value::Host))
    }
    pub(in crate::scripting) fn inert_document_title_get(&mut self,document:u32)->Result<Value,String>{
        let Kind::Document(record)=&self.state.inert_documents.node(document).ok_or("Missing document record.")?.kind else{return Ok(Value::String(String::new()));};
        let Some(root)=record.html else{return Ok(Value::String(String::new()));};
        let mut title=None;
        if self.document_element_named(root,crate::html::Namespace::Svg,"svg"){
            let children=self.inert_children(root).ok_or("Missing SVG document children.")?;self.state.work(children.len())?;
            title=self.inert_children(root).unwrap().iter().copied().find(|&child|self.document_element_named(child,crate::html::Namespace::Svg,"title"));
        }else{
            let mut pending=vec![root];let mut visited=0usize;
            let maximum=self.state.inert_documents.entries.len().saturating_add(self.state.inert_documents.adopted.len());
            while let Some(node)=pending.pop(){
                self.state.work(1)?;visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Cyclic document title topology.")?;
                if self.document_element_named(node,crate::html::Namespace::Html,"title"){title=Some(node);break;}
                let children=self.inert_children(node).ok_or("Missing title traversal node.")?;let count=children.len();
                self.state.work(count)?;let children=self.inert_children(node).unwrap();pending.try_reserve(count).map_err(|_|"Cannot reserve document title traversal.")?;pending.extend(children.iter().rev().copied());
            }
        }
        let Some(title)=title else{return Ok(Value::String(String::new()));};
        let children=self.inert_children(title).ok_or("Missing title child nodes.")?;let mut handles=Vec::new();handles.try_reserve_exact(children.len()).map_err(|_|"Cannot reserve title text references.")?;
        handles.extend(children.iter().copied().filter(|&node|matches!(self.inert_node_type(node),Some(3|4))));
        let (bytes,count)={let parts=self.inert_dom_text_parts(&handles)?;(parts.iter().try_fold(0usize,|n,p|n.checked_add(p.used_bytes()).ok_or("Title input size overflow."))?,parts.iter().try_fold(0usize,|n,p|n.checked_add(p.utf16_len()).ok_or("Title unit count overflow."))?)};
        self.state.work(bytes)?;self.state.copies(count.checked_mul(2).ok_or("Title temporary size overflow.")?)?;
        let parts=self.inert_dom_text_parts(&handles)?;let mut units=Vec::new();units.try_reserve_exact(count).map_err(|_|"Cannot reserve title code units.")?;let mut whitespace=false;
        for unit in parts.iter().flat_map(|part|part.code_units()){
            if matches!(unit,0x09|0x0a|0x0c|0x0d|0x20){whitespace=!units.is_empty();continue;}
            if whitespace{units.push(0x20);whitespace=false;}units.push(unit);
        }
        let(projection,unpaired)=DomString::projected_size(&units)?;let logical=if unpaired{units.len().checked_mul(2).ok_or("Title result size overflow.")?}else{projection};
        if logical>self.state.limits.host.string_bytes{return Err("Document title exceeds the host string allowance.".into());}
        self.state.copies(projection.checked_add(if unpaired{logical}else{0}).ok_or("Title copy size overflow.")?)?;
        DomString::from_units(&units,DomString::storage_bound(self.state.limits.host.string_bytes)?).map(Self::dom_string_owned)
    }
    pub(in crate::scripting) fn create_xml_element(&mut self,owner:u32,args:&[Value])->Result<Value,String>{
        let name=self.text(args.first().ok_or("createElement requires a local name.")?)?;
        if !script_attribute_names::valid_element_local_name(&name){return Err("Invalid XML document element local name.".into());}
        if args.get(1).is_some_and(|value|!matches!(value,Value::Null|Value::Undefined)){return Err("Element creation options are not implemented; no node was created.".into());}
        let mut expanded_name=Vec::new();expanded_name.try_reserve_exact(1).map_err(|_|"Cannot reserve null element namespace record.")?;
        expanded_name.push(crate::html::ExpandedElementName{namespace:crate::html::ElementNamespaceValue::Null,prefix:None});
        self.append_inert_source_leaf(owner,None,Node{kind:NodeKind::Element{tag:name,expanded_name,id:None,href:None,attrs:Vec::new()},namespace:crate::html::Namespace::Other,children:Vec::new()},0).map(Value::Host)
    }

    pub(in crate::scripting) fn validate_xml_document_factory(&self,args:&[Value])->Result<(),(&'static str,String)>{
        if let Some(doctype)=args.get(2).filter(|v|!matches!(v,Value::Null|Value::Undefined)){
            if !matches!(doctype,Value::Host(handle) if self.doctype_receiver(*handle)){return Err(("TypeError","createDocument requires a DocumentType or null.".into()));}
        }
        if let (Some(namespace),Some(Value::String(name)))=(args.first(),args.get(1)){
            if !name.is_empty(){script_attribute_names::validate_element_namespace_value(namespace,name)?;}
        }
        Ok(())
    }
    pub(in crate::scripting) fn create_xml_document(&mut self,implementation:u32,args:&[Value])->Result<Value,String>{
        if !self.inert_implementation(implementation){return Err("createDocument requires a genuine DOMImplementation.".into());}
        self.validate_xml_document_factory(args).map_err(|(_,message)|message)?;
        let namespace=args.first().ok_or("createDocument requires two arguments.")?;
        let qualified=self.text(args.get(1).ok_or("createDocument requires two arguments.")?)?;
        let namespace=if matches!(namespace,Value::Null|Value::Undefined){None}else{Some(self.dom_text(namespace)?)};
        let class=match &namespace{Some(uri) if uri=="http://www.w3.org/1999/xhtml"=>crate::html::Namespace::Html,Some(uri) if uri=="http://www.w3.org/2000/svg"=>crate::html::Namespace::Svg,_=>crate::html::Namespace::Other};
        let content_type=match class{crate::html::Namespace::Html=>"application/xhtml+xml",crate::html::Namespace::Svg=>"image/svg+xml",_=>"application/xml"};
        let root=if qualified.is_empty(){None}else{
            let(tag,expanded_name,namespace)=crate::html::element_names::prepare(namespace,qualified)?;
            Some(Node{kind:NodeKind::Element{tag,expanded_name,id:None,href:None,attrs:Vec::new()},namespace,children:Vec::new()})
        };
        let doctype=args.get(2).and_then(|value|if let Value::Host(handle)=value{Some(*handle)}else{None});
        let adoption=doctype.map(|handle|self.prepare_xml_doctype_adoption(handle)).transpose()?;
        // Root, Document and implementation remain unpublished until the
        // whole factory succeeds. Reuse the real leaf reservation before any
        // caller can save its identity; no returned handle is ever remapped.
        let checkpoint=self.template_import_checkpoint();
        let result=(||{
            let first=FIRST_ENTRY.checked_add(u32::try_from(self.state.inert_documents.entries.len()).map_err(|_|"XML document identity overflow.")?).and_then(|handle|handle.checked_add(u32::from(root.is_some()))).ok_or("XML document handle overflow.")?;
            let root_handle=if let Some(node)=root{Some(self.append_inert_source_leaf(first,Some(first),node,0)?)}else{None};
            let mut children=Vec::new();children.try_reserve_exact(usize::from(root_handle.is_some())+usize::from(doctype.is_some())).map_err(|_|"Cannot reserve XML document child links.")?;
            if let Some(handle)=doctype{children.push(handle);}if let Some(handle)=root_handle{children.push(handle);}
            let url=script_clone_node::copy_text("about:blank")?;let payload=url.capacity().checked_add(children.capacity()*size_of::<u32>()).ok_or("XML document payload overflow.")?;
            let native=adoption.as_ref().and_then(|plan|plan.native);
            let(index_growth,index_bytes)=if let Some(handle)=doctype.filter(|_|native.is_some()){
                self.prepare_adopted_index(std::iter::once(handle),self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(payload).ok_or("XML adoption payload allowance exhausted.")?)?
            }else{(None,0)};
            let mut entries=Vec::new();entries.try_reserve_exact(2).map_err(|_|"Cannot reserve staged XML document identities.")?;
            entries.push(Entry::Node(InertNode{kind:Kind::Document(DocumentNodes{kind:crate::html::DocumentKind::Xml,mode:crate::html::DocumentMode::NoQuirks,content_type,implementation:first+1,doctype,url,base_url_override:None,html:root_handle,head:None,body:None,title:String::new()}),owner:first,parent:None,children}));entries.push(Entry::Implementation);
            let admitted=self.reserve_inert_entries(2,payload.checked_add(index_bytes).ok_or("XML adoption storage overflow.")?,0)?;debug_assert_eq!(admitted,first);
            self.state.inert_documents.entries.extend(entries);
            if let Some(plan)=adoption{self.commit_xml_doctype_adoption(plan,first,index_growth);}
            Ok(Value::Host(first))
        })();
        if result.is_err(){self.rollback_template_import(checkpoint);}result
    }
}

#[cfg(test)]
mod storage_tests{
    use super::*;
    fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse("<!doctype html><html><head></head><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
    #[test]
    fn xml_factory_root_identity_late_header_refusal_rolls_back_unpublished_root_and_keeps_paid_backing(){
        let root_cost={let(mut doc,mut forms,mut page)=fixture();let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let before=b.state.retained;let Value::Host(owner)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::String(HTML_NAMESPACE.into()),Value::String("span".into())]).unwrap()else{panic!("measured factory")};let node=b.state.inert_documents.node(owner).unwrap();let Kind::Document(record)=&node.kind else{unreachable!()};assert!(node_index(record.html.unwrap()).is_some());b.state.retained-before-record.url.capacity()-node.children.capacity()*size_of::<u32>()};
        let(mut doc,mut forms,mut page)=fixture();let dtype=doc.nodes[0].children[0];let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let original=b.doc.nodes[0].children.clone();let before=(b.state.retained,b.template_import_backing(),b.doc.nodes.len(),b.state.inert_documents.entries.len(),b.state.inert_documents.generated.len(),b.state.inert_documents.adopted.len(),b.state.dom_commits);let limit=b.state.retained_limit;let copies=b.state.copy_work;
        b.state.retained_limit=b.state.retained+root_cost;assert!(b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::String(HTML_NAMESPACE.into()),Value::String("span".into()),Value::Host(node_handle(dtype))]).is_err());
        assert_eq!((b.doc.nodes.len(),b.state.inert_documents.entries.len(),b.state.inert_documents.generated.len(),b.state.inert_documents.adopted.len(),b.state.dom_commits),(before.2,before.3,before.4,before.5,before.6));assert_eq!(b.doc.nodes[0].children,original);assert_eq!(b.state.tree.parents[dtype],Some(0));assert!(!b.inert_adopted_node(node_handle(dtype)));assert!(b.template_import_backing()>before.1,"late root reservation acquired actual backing");assert_eq!(b.state.retained,before.0+b.template_import_backing()-before.1,"only acquired backing survives rollback");
        b.state.retained_limit=limit;let rollback=(b.state.retained,b.template_import_backing(),b.doc.nodes.len(),b.state.inert_documents.entries.len());b.state.copy_work=0;assert!(b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::String(HTML_NAMESPACE.into()),Value::String("span".into())]).is_err());assert_eq!((b.state.retained,b.template_import_backing(),b.doc.nodes.len(),b.state.inert_documents.entries.len()),rollback);
        b.state.copy_work=copies;let Value::Host(owner)=b.create_xml_document(MAIN_IMPLEMENTATION,&[Value::String(HTML_NAMESPACE.into()),Value::String("span".into()),Value::Host(node_handle(dtype))]).unwrap()else{panic!("fresh recovery")};let Kind::Document(record)=&b.state.inert_documents.node(owner).unwrap().kind else{unreachable!()};let root=record.html.unwrap();assert!(node_index(root).is_some());assert_eq!(record.doctype,Some(node_handle(dtype)));assert_eq!(b.inert_children(owner).unwrap(),[node_handle(dtype),root]);assert_eq!(b.state.inert_documents.node(root).unwrap().owner,owner);assert_eq!(b.state.inert_documents.node(node_handle(dtype)).unwrap().owner,owner);
    }
    #[test]
    fn native_xml_document_exact_owned_payload_and_entry_source_backing_are_admitted(){
        let mut doc=crate::html::parse("<!doctype html><html><head></head><body></body></html>");doc.scripting_enabled=true;
        let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        let backing=|bridge:&Bridge<'_>|bridge.template_import_backing();
        for (namespace,name) in [(Value::Null,""),(Value::String("urn:original".into()),"p:Mixed"),(Value::String("http://www.w3.org/1999/xhtml".into()),"h:html")]{
            let before=bridge.state.retained;let previous=backing(&bridge);
            let Value::Host(owner)=bridge.create_xml_document(MAIN_IMPLEMENTATION,&[namespace,Value::String(name.into()),Value::Null]).unwrap()else{panic!("real XML document")};
            let arena=&bridge.state.inert_documents;let Kind::Document(record)=&arena.node(owner).unwrap().kind else{panic!("owned document record")};
            let mut payload=record.url.capacity()+record.title.capacity()+arena.node(owner).unwrap().children.capacity()*size_of::<u32>();
            if let Some(root)=record.html{
                let Kind::Parsed{source,..}=arena.node(root).unwrap().kind else{panic!("owned root source")};let node=bridge.parsed_source(source).unwrap().0;
                let NodeKind::Element{tag,href,..}=&node.kind else{panic!("actual root")};
                payload+=node_bytes(node)+tag.capacity()+href.as_ref().map_or(0,String::capacity)+size_of::<Node>()+2*size_of::<usize>();
                assert_eq!(arena.node(root).unwrap().owner,owner);
            }
            assert_eq!(bridge.state.retained-before,payload+backing(&bridge)-previous,"{name}");
        }
    }
}
