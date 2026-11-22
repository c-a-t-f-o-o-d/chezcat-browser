//! Native names and namespace values read their actual owned records.
use super::*;
use crate::html::{DocumentKind,ExpandedElementName,Namespace};
use crate::html::element_names::{self,NamespaceRef};

pub(in crate::scripting) const DEFAULT_METHOD:&str="\0Node.isDefaultNamespace";
pub(in crate::scripting) fn property(name:&str)->bool{matches!(name,"tagName"|"nodeName"|"localName"|"namespaceURI"|"prefix")}
fn result_bytes(class:Namespace,tag:&str,names:&[ExpandedElementName],property:&str)->Result<usize,String>{
    let namespace=element_names::namespace(class,names)?;
    Ok(match property {
        "namespaceURI"=>match namespace{NamespaceRef::Null=>0,NamespaceRef::Static(uri)=>uri.len(),NamespaceRef::Owned(uri)=>uri.logical_bytes()},
        "prefix"=>element_names::prefix(class,names)?.map_or(0,str::len),
        "localName"=>tag.len(),
        _=>tag.len().checked_add(element_names::prefix(class,names)?.map_or(0,|p|p.len()+1)).ok_or("Qualified element name size overflow.")?,
    })
}
fn copy_value(class:Namespace,tag:&str,names:&[ExpandedElementName],property:&str,kind:DocumentKind)->Result<Value,String>{
    match property {
        "namespaceURI"=>return match element_names::namespace(class,names)?{NamespaceRef::Null=>Ok(Value::Null),NamespaceRef::Static(uri)=>Ok(Value::String(script_clone_node::copy_text(uri)?)),NamespaceRef::Owned(uri)=>Bridge::dom_string_copy(uri)},
        "prefix"=>return element_names::prefix(class,names)?.map_or(Ok(Value::Null),|prefix|script_clone_node::copy_text(prefix).map(Value::String)),
        "localName"=>return script_clone_node::copy_text(tag).map(Value::String),
        _=>{},
    }
    let prefix=element_names::prefix(class,names)?;
    let bytes=result_bytes(class,tag,names,property)?;
    let mut value=String::new();value.try_reserve_exact(bytes).map_err(|_|"Cannot reserve qualified element name.")?;
    let uppercase=class==Namespace::Html&&kind==DocumentKind::Html;
    if let Some(prefix)=prefix{if uppercase{value.extend(prefix.chars().map(|ch|ch.to_ascii_uppercase()));}else{value.push_str(prefix);}value.push(':');}
    if uppercase{value.extend(tag.chars().map(|ch|ch.to_ascii_uppercase()));}else{value.push_str(tag);}
    Ok(Value::String(value))
}
impl Bridge<'_>{
    pub(in crate::scripting) fn create_expanded_element(&mut self,owner:u32,args:&[Value])->Result<Value,String>{
        let namespace=args.first().ok_or("createElementNS requires two arguments.")?;
        let qualified=self.text(args.get(1).ok_or("createElementNS requires two arguments.")?)?;
        script_attribute_names::validate_element_namespace_value(namespace,&qualified).map_err(|(_,message)|message)?;
        if args.get(2).is_some_and(|v|!matches!(v,Value::Null|Value::Undefined)){return Err("Element creation options/custom registries are not implemented; no node was created.".into());}
        let namespace=if matches!(namespace,Value::Null|Value::Undefined){None}else{Some(self.dom_text(namespace)?)};
        let(tag,expanded_name,class)=element_names::prepare(namespace,qualified)?;
        if class==Namespace::Svg&&!crate::html::svg_dom_name(&tag){return Err("SVG creation requires a supported passive shape/container; no node was created.".into());}
        if owner==DOCUMENT&&class==Namespace::Html&&tag=="img"{return self.create_image_named(&[],expanded_name);}
        let node=Node{kind:NodeKind::Element{tag,expanded_name,id:None,href:None,attrs:Vec::new()},namespace:class,children:Vec::new()};
        if owner==DOCUMENT&&class!=Namespace::Other{return self.create_dom_node_ns(node.kind,class);}
        self.append_inert_source_leaf(owner,None,node,0).map(Value::Host)
    }

    pub(in crate::scripting) fn element_name_get(&mut self,node:usize,property:&str)->Result<Value,String>{
        if self.doc.nodes[node].namespace==Namespace::UnknownForeign{return Err(self.element_naming_error(node));}
        let NodeKind::Element{tag,expanded_name,..}=&self.doc.nodes[node].kind else{return Err("Native element names require an Element.".into());};
        let class=self.doc.nodes[node].namespace;let bytes=result_bytes(class,tag,expanded_name,property)?;
        if bytes>self.state.limits.host.string_bytes{return Err("Native element name exceeds the host string allowance.".into());}
        self.state.work(bytes)?;self.state.copies(bytes)?;
        copy_value(class,tag,expanded_name,property,self.doc.kind)
    }
    fn inert_name_parts(&self,object:u32)->Result<(Namespace,&str,&[ExpandedElementName]),String>{
        let node=self.state.inert_documents.node(object).ok_or("Missing native named element.")?;
        match &node.kind {
            Kind::Element(tag)=>Ok((Namespace::Html,tag,&[])),
            Kind::Parsed{source,..}=>{
                let source=self.parsed_source(*source).ok_or("Missing native named source.")?.0;
                let NodeKind::Element{tag,expanded_name,..}=&source.kind else{return Err("Native names require an Element.".into());};
                Ok((source.namespace,tag,expanded_name))
            },
            _=>Err("Native names require an Element.".into()),
        }
    }
    pub(in crate::scripting) fn inert_element_name_get(&mut self,object:u32,property:&str)->Result<Value,String>{
        let owner=self.state.inert_documents.node(object).ok_or("Missing element owner.")?.owner;
        let kind=if owner==DOCUMENT{self.doc.kind}else{match self.state.inert_documents.node(owner).map(|node|&node.kind){Some(Kind::Document(doc))=>doc.kind,_=>DocumentKind::Html}};
        let(class,tag,names)=self.inert_name_parts(object)?;let bytes=result_bytes(class,tag,names,property)?;
        if bytes>self.state.limits.host.string_bytes{return Err("Native element name exceeds the host string allowance.".into());}
        self.state.work(bytes)?;self.state.copies(bytes)?;
        let(class,tag,names)=self.inert_name_parts(object)?;copy_value(class,tag,names,property,kind)
    }
}

fn namespace_matches(namespace:NamespaceRef<'_>,query:&Value)->bool{
    let empty=matches!(query,Value::Null|Value::Undefined)||matches!(query,Value::String(s) if s.is_empty())||matches!(query,Value::StringUnits(s) if s.is_empty());
    match namespace {
        NamespaceRef::Null=>empty,
        NamespaceRef::Static(uri)=>match query{Value::String(s)=>s==uri,Value::StringUnits(s)=>s.iter().copied().eq(uri.encode_utf16()),_=>false},
        NamespaceRef::Owned(uri) if uri.logical_bytes()==0=>empty,
        NamespaceRef::Owned(uri)=>match query{Value::String(s)=>uri.code_units().eq(s.encode_utf16()),Value::StringUnits(s)=>uri.code_units().eq(s.iter().copied()),_=>false},
    }
}
fn namespace_bytes(namespace:NamespaceRef<'_>)->usize{match namespace{NamespaceRef::Null=>0,NamespaceRef::Static(s)=>s.len(),NamespaceRef::Owned(s)=>s.logical_bytes()}}
impl Bridge<'_>{
    fn namespace_parent_element(&self,object:u32)->Option<u32>{
        let parent=if let Some(node)=self.state.inert_documents.node(object){node.parent}else{let node=node_index(object)?;self.state.tree.parents.get(node).copied().flatten().filter(|&p|p!=0).map(node_handle)}?;
        (self.inert_node_type(parent)==Some(1)||node_index(parent).is_some_and(|node|matches!(self.doc.nodes.get(node).map(|n|&n.kind),Some(NodeKind::Element{..})))).then_some(parent)
    }
    fn namespace_start(&self,object:u32)->Result<Option<u32>,String>{
        if self.attr_receiver(object){return Ok(self.state.attributes[script_attr::index(object).unwrap()].owner);}
        if object==DOCUMENT{return Ok(self.doc.nodes[0].children.iter().copied().find(|&node|matches!(self.doc.nodes[node].kind,NodeKind::Element{..})).map(node_handle));}
        if let Some(node)=self.state.inert_documents.node(object){return Ok(match &node.kind{Kind::Document(doc)=>Some(doc.html),Kind::TemplateDocument{..}|Kind::Doctype(_)|Kind::Fragment{..}=>None,_=>if self.inert_node_type(object)==Some(1){Some(object)}else{self.namespace_parent_element(object)}});}
        let node=node_index(object).filter(|&node|self.dom_accessible(node)).ok_or("Namespace lookup requires a genuine accessible Node.")?;
        Ok(match self.doc.nodes[node].kind{NodeKind::Element{..}=>Some(object),NodeKind::DocumentType(_)|NodeKind::DocumentFragment=>None,_=>self.namespace_parent_element(object)})
    }
    fn namespace_facts(&self,object:u32)->Result<(Namespace,&[ExpandedElementName],&[crate::html::Attribute]),String>{
        if self.state.inert_documents.node(object).is_some(){
            let(class,_,names)=self.inert_name_parts(object)?;let attrs=self.inert_element_attributes(object).ok_or("Missing namespace attribute source.")?;return Ok((class,names,attrs));
        }
        let node=node_index(object).filter(|&node|node<self.doc.nodes.len()).ok_or("Missing namespace Element.")?;
        let NodeKind::Element{expanded_name,attrs,..}=&self.doc.nodes[node].kind else{return Err("Namespace ancestry requires an Element.".into());};
        Ok((self.doc.nodes[node].namespace,expanded_name,attrs))
    }
    pub(in crate::scripting) fn native_is_default_namespace(&mut self,object:u32,args:&[Value])->Result<Value,String>{
        let query=args.first().ok_or("isDefaultNamespace requires one argument.")?;
        if !matches!(query,Value::String(_)|Value::StringUnits(_)|Value::Null|Value::Undefined){return Err("Namespace query must be a converted nullable DOMString.".into());}
        if let Some(result)=self.frame_default_namespace(object){return result.map(|uri|Value::Bool(namespace_matches(uri.map_or(NamespaceRef::Null,NamespaceRef::Static),query)));}
        self.state.rebuild(self.doc)?;
        if object==DOCUMENT{self.state.work(self.doc.nodes[0].children.len())?;}
        let mut current=self.namespace_start(object)?;
        let maximum=self.doc.nodes.len().checked_add(self.state.inert_documents.entries.len()).and_then(|n|n.checked_add(self.state.inert_documents.adopted.len())).and_then(|n|n.checked_add(1)).ok_or("Namespace ancestry bound overflow.")?;
        for _ in 0..maximum {
            let Some(element)=current else{return Ok(Value::Bool(namespace_matches(NamespaceRef::Null,query)));};
            self.state.work(1)?;
            if let Some(result)=self.frame_default_namespace(element){return result.map(|uri|Value::Bool(namespace_matches(uri.map_or(NamespaceRef::Null,NamespaceRef::Static),query)));}
            let(class,names,attrs)=self.namespace_facts(element)?;
            let bytes=namespace_bytes(element_names::namespace(class,names)?).saturating_add(attrs.iter().fold(attrs.len(),|n,a|n.saturating_add(a.name.bytes()).saturating_add(a.value.logical_bytes()))).saturating_add(query.bytes());
            self.state.work(bytes)?;
            let(class,names,attrs)=self.namespace_facts(element)?;
            let namespace=element_names::namespace(class,names)?;
            if element_names::prefix(class,names)?.is_none()&&!matches!(namespace,NamespaceRef::Null){return Ok(Value::Bool(namespace_matches(namespace,query)));}
            if let Some(declaration)=attrs.iter().find(|attr|attr.name.namespace_uri()==Some("http://www.w3.org/2000/xmlns/")&&attr.name.prefix().is_none()&&attr.name.local()=="xmlns"){
                return Ok(Value::Bool(namespace_matches(NamespaceRef::Owned(&declaration.value),query)));
            }
            current=self.namespace_parent_element(element);
        }
        Err("Cyclic namespace ancestry.".into())
    }
}
