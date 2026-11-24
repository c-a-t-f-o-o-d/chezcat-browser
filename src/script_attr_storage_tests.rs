//! Original Attr backing, owner-index, payload and atomic transfer acceptance.
use super::*;
fn page(markup:&str)->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document(markup,&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let result=page.start(&mut doc,&mut forms,&[]);
    assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn handle(value:Value)->u32{let Value::Host(handle)=value else{panic!("genuine Attr expected")};handle}
#[test]
fn browser_attr_detached_and_attached_identities_exceed_old_count_with_real_payload_and_sameobject() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body>{}</body></html>","<p title=kept></p>".repeat(70000)));
    let nodes:Vec<_>=doc.nodes.iter().enumerate().filter_map(|(id,node)|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="p").then_some(id)).collect();
    let baseline=page.state.retained;let mut name_bytes=0;
    for index in 0..70000{
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        let attr=handle(bridge.create_attribute(&[Value::String(format!("data-{index}"))],false).unwrap());
        assert_eq!(script_attr::index(attr),Some(index));name_bytes+=owned_name_bytes(&bridge.state.attributes[index].name);
    }
    assert_eq!(page.state.retained,baseline+page.state.attributes.capacity()*size_of::<Attr>()+name_bytes);
    for (index,&node) in nodes.iter().enumerate(){
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        let attr=handle(bridge.lookup_attribute_node_at(node,Some(0)).unwrap());
        assert_eq!(script_attr::index(attr),Some(70000+index));
        assert_eq!(handle(bridge.lookup_attribute_node_at(node,Some(0)).unwrap()),attr);
    }
    assert_eq!(page.state.attributes.len(),140000);assert_eq!(page.state.attached_attributes.len(),70000);
    let payload=page.state.attributes.iter().map(|attr|owned_name_bytes(&attr.name)+attr.value.capacity()).sum::<usize>();
    assert_eq!(page.state.retained,baseline+page.state.attributes.capacity()*size_of::<Attr>()+page.state.attached_attributes.capacity()*size_of::<usize>()+payload);
    assert!(page.state.retained<=page.state.retained_limit);
    let first=ATTR_BASE+70000;let last=ATTR_BASE+139999;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(matches!(bridge.attr_get(last,"ownerElement").unwrap(),Value::Host(id) if id==node_handle(*nodes.last().unwrap())));
    bridge.attr_set(first,"value",Value::String("changed".into())).unwrap();
    assert_eq!(bridge.doc.attr(nodes[0],"title"),Some("changed"));
    assert_eq!(bridge.attr_get(last,"value").unwrap().to_text(),"kept");
}
#[test]
fn browser_attr_registry_and_owner_index_exact_bundle_refusal_and_copy_recovery_publish_no_backing() {
    let(mut doc,mut forms,mut page)=page("<html><body><p title=old></p><div title=next></div></body></html>");
    let nodes:Vec<_>=doc.nodes.iter().enumerate().filter_map(|(id,node)|matches!(&node.kind,NodeKind::Element{tag,..} if matches!(tag.as_str(),"p"|"div")).then_some(id)).collect();
    let baseline=page.state.retained;let limit=page.state.retained_limit;
    let bundle=size_of::<Attr>()+size_of::<usize>()+"title".len();
    page.state.retained_limit=baseline+bundle-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.lookup_attribute_node_at(nodes[0],Some(0)).is_err());
    assert_eq!(page.state.retained,baseline);assert_eq!(page.state.attributes.capacity(),0);assert_eq!(page.state.attached_attributes.capacity(),0);
    page.state.retained_limit+=1;page.begin_job();
    let attr=handle(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.lookup_attribute_node_at(nodes[0],Some(0)).unwrap());
    assert_eq!(page.state.retained,baseline+bundle);assert_eq!(page.state.attributes.capacity(),1);assert_eq!(page.state.attached_attributes.capacity(),1);
    page.state.retained_limit=limit;page.begin_job();page.state.copy_work="title".len();
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.lookup_attribute_node_at(nodes[1],Some(0)).unwrap_err().contains("string work limit"));
    assert_eq!((page.state.attributes.len(),page.state.attached_attributes.len(),page.state.retained),(1,1,baseline+bundle));
    assert_eq!(page.state.attributes.capacity(),1);assert_eq!(page.state.attached_attributes.capacity(),1);
    page.begin_job();let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert_eq!(handle(bridge.lookup_attribute_node_at(nodes[0],Some(0)).unwrap()),attr);
    bridge.lookup_attribute_node_at(nodes[1],Some(0)).unwrap();
    assert_eq!(bridge.state.attached_attributes.len(),2);
}
#[test]
fn browser_attr_detach_reserves_actual_value_capacity_and_reattach_releases_detached_backing() {
    let(mut doc,mut forms,mut page)=page("<html><body><p title='a&amp;b'></p></body></html>");
    let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="p")).unwrap();
    let NodeKind::Element{attrs,..}=&doc.nodes[node].kind else{unreachable!()};
    assert_eq!(attrs[0].value.as_str(),"a&b");
    // Keep testing transfer of spare backing without requiring the decoder to
    // retain encoded-input capacity after its exact-size output reservation.
    let mut value=String::with_capacity(64);value.push_str("a&b");
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("title".into()),Some(value)).unwrap();
    let NodeKind::Element{attrs,..}=&doc.nodes[node].kind else{unreachable!()};
    let capacity=attrs[0].value.capacity();assert!(capacity>attrs[0].value.len());
    let attr=handle(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.lookup_attribute_node_at(node,Some(0)).unwrap());
    let before=page.state.retained;let limit=page.state.retained_limit;
    page.state.retained_limit=before-"title".len()-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.remove_attribute_node(node_handle(node),&[Value::Host(attr)]).is_err());
    assert_eq!(doc.attr(node,"title"),Some("a&b"));assert_eq!(page.state.attached_attributes.len(),1);assert_eq!(page.state.retained,before);
    page.state.retained_limit=limit;page.begin_job();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.remove_attribute_node(node_handle(node),&[Value::Host(attr)]).unwrap();
    assert_eq!(bridge.state.retained,before-"title".len());assert!(bridge.state.attached_attributes.is_empty());
    assert_eq!(bridge.state.attributes[0].value.capacity(),capacity);
    bridge.attr_set(attr,"value",Value::String("new".into())).unwrap();
    bridge.set_attribute_node(node_handle(node),&[Value::Host(attr)]).unwrap();
    assert_eq!(bridge.doc.attr(node,"title"),Some("new"));assert_eq!(bridge.state.attributes[0].value.capacity(),0);
    assert_eq!(bridge.state.retained,before-capacity+"new".len());assert_eq!(bridge.state.attached_attributes.as_slice(),[0]);
}
#[test]
fn browser_attr_index_preserves_expanded_names_and_real_replacement_identity() {
    let(mut doc,mut forms,mut page)=page("<html><body><p title=old></p></body></html>");
    page.begin_job();let source="var node=document.querySelector('p'),first=node.getAttributeNode('title'),next=document.createAttribute('title');next.value='new';var old=node.setAttributeNode(next);if(old!==first||old.ownerElement!==null||next.ownerElement!==node||node.getAttributeNode('title')!==next)throw 'replace';node.removeAttributeNode(next);if(next.ownerElement!==null)throw 'detach';node.setAttributeNode(old);var a=document.createAttributeNS('urn:one','a:key'),b=document.createAttributeNS('urn:two','b:key');a.value='one';b.value='two';node.setAttributeNode(a);node.setAttributeNode(b);if(node.getAttributeNodeNS('urn:one','key')!==a||node.getAttributeNodeNS('urn:two','key')!==b||node.getAttribute('title')!=='old')throw 'expanded identity';";
    page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
    assert_eq!(page.state.attached_attributes.len(),3);
    for pair in page.state.attached_attributes.windows(2){
        let left=&page.state.attributes[pair[0]];let right=&page.state.attributes[pair[1]];
        assert!((left.owner,left.name.namespace_uri(),left.name.local())<(right.owner,right.name.namespace_uri(),right.name.local()));
    }
}

#[test]
fn browser_attr_replacement_registry_and_index_growth_fit_exactly_and_refuse_before_native_publication() {
    let(mut doc,mut forms,mut page)=page("<html><body><p title=old></p></body></html>");
    let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="p")).unwrap();
    let baseline=page.state.retained;let limit=page.state.retained_limit;
    page.state.retained_limit=baseline+size_of::<Attr>()+"title".len();
    let incoming=handle(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_attribute(&[Value::String("title".into())],false).unwrap());
    assert_eq!(page.state.attributes.capacity(),1);
    page.state.retained_limit=limit;page.begin_job();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.attr_set(incoming,"value",Value::String("new".into())).unwrap();
    let before=page.state.retained;let bundle=size_of::<Attr>()+size_of::<usize>()+"title".len();
    page.state.retained_limit=before+bundle-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.set_attribute_node(node_handle(node),&[Value::Host(incoming)]).is_err());
    assert_eq!(doc.attr(node,"title"),Some("old"));assert_eq!(page.state.retained,before);
    assert_eq!(page.state.attributes.len(),1);assert_eq!(page.state.attributes.capacity(),1);assert_eq!(page.state.attached_attributes.capacity(),0);
    assert_eq!(page.state.attributes[0].owner,None);assert_eq!(page.state.attributes[0].value,"new");
    page.state.retained_limit+=1;page.begin_job();
    let old=handle(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.set_attribute_node(node_handle(node),&[Value::Host(incoming)]).unwrap());
    assert_eq!(old,ATTR_BASE+1);assert_eq!(doc.attr(node,"title"),Some("new"));
    assert_eq!(page.state.retained,before+bundle);assert_eq!(page.state.attributes.capacity(),2);assert_eq!(page.state.attached_attributes.capacity(),1);
    assert_eq!(page.state.attributes[0].owner.and_then(node_index),Some(node));assert_eq!(page.state.attributes[0].value.capacity(),0);
    assert_eq!(page.state.attributes[1].owner,None);assert_eq!(page.state.attributes[1].value,"old");
}

#[test]
fn browser_attr_owner_index_growth_leaves_exact_native_input_payload_headroom() {
    let(mut doc,mut forms,mut page)=page("<html><body></body></html>");
    page.runtime.execute("var input=document.createElement('input');input.id='fresh';",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
    page.state.rebuild(&doc).unwrap();
    let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="input")).unwrap();
    let attr=handle(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_attribute(&[Value::String("name".into())],false).unwrap());
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.attr_set(attr,"value",Value::String("q".repeat(80))).unwrap();
    let before=page.state.retained;let growth="name".len()+size_of::<usize>()+80;
    page.state.retained_limit=before+growth-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.set_attribute_node(node_handle(node),&[Value::Host(attr)]).is_err());
    assert_eq!(page.state.retained,before);assert!(!forms.controls.contains_key(&node));assert_eq!(doc.attr(node,"name"),None);assert_eq!(page.state.attached_attributes.capacity(),0);
    page.state.retained_limit+=1;page.begin_job();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.set_attribute_node(node_handle(node),&[Value::Host(attr)]).unwrap();
    assert_eq!(page.state.retained,before+growth);assert_eq!(page.state.attached_attributes.capacity(),1);
    assert_eq!(forms.controls[&node].name,"q".repeat(80));assert_eq!(page.state.attributes[0].value.capacity(),0);
}
