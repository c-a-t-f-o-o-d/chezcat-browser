//! Original native XML document factory. Parsing XML bytes is owned by F08.
use super::*;
pub(in crate::scripting) const CREATE:&str="\0DOMImplementation.createDocument";
impl Bridge<'_>{
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
            return Err(("NotSupportedError","Document-root doctype adoption is not implemented; no document was created.".into()));
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
        let count=2+usize::from(root.is_some());
        let first=FIRST_ENTRY.checked_add(u32::try_from(self.state.inert_documents.entries.len()).map_err(|_|"XML document identity overflow.")?).ok_or("XML document identity overflow.")?;
        let root_handle=root.as_ref().map(|_|first+2);
        let mut children=Vec::new();children.try_reserve_exact(usize::from(root.is_some())).map_err(|_|"Cannot reserve XML document child links.")?;
        if let Some(handle)=root_handle{children.push(handle);}
        let url=script_clone_node::copy_text("about:blank")?;
        let mut payload=url.capacity().checked_add(children.capacity()*size_of::<u32>()).ok_or("XML document payload overflow.")?;
        let generated_base=self.state.inert_documents.generated.len();
        let generated=if let Some(node)=root{
            let NodeKind::Element{tag,href,..}=&node.kind else{unreachable!()};
            payload=payload.checked_add(node_bytes(&node)).and_then(|n|n.checked_add(tag.capacity())).and_then(|n|n.checked_add(href.as_ref().map_or(0,String::capacity))).and_then(|n|n.checked_add(size_of::<Node>()+2*size_of::<usize>())).ok_or("XML root source payload overflow.")?;
            Some(GeneratedNode{node:Rc::new(node),unsafe_nonce:false})
        }else{None};
        let mut entries=Vec::new();entries.try_reserve_exact(count).map_err(|_|"Cannot reserve staged XML identities.")?;
        entries.push(Entry::Node(InertNode{kind:Kind::Document(DocumentNodes{kind:crate::html::DocumentKind::Xml,mode:crate::html::DocumentMode::NoQuirks,content_type,implementation:first+1,doctype:None,url,html:root_handle,head:None,body:None,title:String::new()}),owner:first,parent:None,children}));
        entries.push(Entry::Implementation);
        if root_handle.is_some(){entries.push(Entry::Node(InertNode{kind:Kind::Parsed{source:ParsedSource::Generated(generated_base),initial_template:None},owner:first,parent:Some(first),children:Vec::new()}));}
        let admitted=self.reserve_inert_entries(count,payload,usize::from(generated.is_some()))?;debug_assert_eq!(admitted,first);
        if let Some(source)=generated{self.state.inert_documents.generated.push(source);}
        self.state.inert_documents.entries.extend(entries);Ok(Value::Host(first))
    }
}
