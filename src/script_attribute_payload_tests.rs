//! Original actual attribute payload, reflected ID and transfer admission.
use super::*;
fn page(markup:&str)->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document(markup,&||Ok(())).unwrap();doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn element(doc:&Document,tagname:&str)->usize{doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag==tagname)).unwrap()}
fn attribute<'a>(doc:&'a Document,node:usize,key:&str)->&'a crate::html::Attribute{let NodeKind::Element{attrs,..}=&doc.nodes[node].kind else{unreachable!()};attrs.iter().find(|attr|attr.name.qualified==key).unwrap()}
fn reserved(text:&str,capacity:usize)->String{let mut value=String::with_capacity(capacity);value.push_str(text);value}
#[test]
fn browser_attribute_decoded_capacity_and_id_reflection_refund_actual_owned_payload() {
    let(mut doc,mut forms,mut page)=page("<html><body><p id='a&amp;b' title='c&amp;d'></p></body></html>");let node=element(&doc,"p");
    let title=attribute(&doc,node,"title");let title_cap=title.value.capacity();assert!(title_cap>title.value.len());
    let before=page.state.retained;page.state.retained_limit=before-title_cap+1-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("title".into()),Some("z".into())).is_err());
    assert_eq!(page.state.retained,before);assert_eq!(doc.attr(node,"title"),Some("c&d"));
    page.state.retained_limit+=1;page.begin_job();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("title".into()),Some("z".into())).unwrap();
    assert_eq!(page.state.retained,before-title_cap+1);
    let old_value=attribute(&doc,node,"id").value.capacity();let NodeKind::Element{id,..}=&doc.nodes[node].kind else{unreachable!()};let old_id=id.as_ref().unwrap().capacity();let before=page.state.retained;
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("id".into()),Some("xy".into())).unwrap();
    assert_eq!(page.state.retained,before-old_value-old_id+4);assert_eq!(doc.attr(node,"id"),Some("xy"));
    let NodeKind::Element{id,..}=&doc.nodes[node].kind else{unreachable!()};assert_eq!(id.as_deref(),Some("xy"));
}
#[test]
fn browser_attribute_reserved_payload_is_memory_while_copy_work_tracks_text_and_records() {
    let(mut doc,mut forms,mut page)=page("<html><body><p></p></body></html>");let node=element(&doc,"p");let before=page.state.retained;
    let work=size_of::<crate::html::Attribute>()+"data-key".len()+"old".len();page.state.copy_work=work-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified(reserved("data-key",64)),Some(reserved("old",128))).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.retained,before);assert_eq!(doc.attr(node,"data-key"),None);
    page.begin_job();page.state.copy_work=work;
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified(reserved("data-key",64)),Some(reserved("old",128))).unwrap();
    assert_eq!(page.state.copy_work,0);let attr=attribute(&doc,node,"data-key");assert_eq!((attr.name.qualified.capacity(),attr.value.capacity()),(64,128));
    let NodeKind::Element{attrs,..}=&doc.nodes[node].kind else{unreachable!()};assert_eq!(page.state.retained,before+attrs.capacity()*size_of::<crate::html::Attribute>()+192);
    page.begin_job();let before=page.state.retained;
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("data-key".into()),Some("z".into())).unwrap();
    assert_eq!(page.state.retained,before-127);assert_eq!(attribute(&doc,node,"data-key").name.qualified.capacity(),64);
}
#[test]
fn browser_attribute_wide_qname_refund_funds_exact_native_identity_transfer() {
    let(mut doc,mut forms,mut page)=page("<html><body><p></p></body></html>");let node=element(&doc,"p");let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.public_attribute(node,crate::html::AttributeName::unqualified(reserved("data-key",64)),Some("z".into())).unwrap();
    let Value::Host(incoming)=bridge.create_attribute(&[Value::String("data-key".into())],false).unwrap()else{panic!("real Attr")};bridge.attr_set(incoming,"value",Value::String("new".into())).unwrap();
    // Release old DOM QName/value (64+1) and incoming value (3); retain
    // new DOM QName/value (8+3), displaced QName/value (8+1), and index (8).
    let before=bridge.state.retained;let final_bytes=before-40;bridge.state.retained_limit=final_bytes-1;
    assert!(bridge.set_attribute_node(node_handle(node),&[Value::Host(incoming)]).is_err());
    assert_eq!(bridge.state.retained,before);assert_eq!(attribute(bridge.doc,node,"data-key").name.qualified.capacity(),64);assert_eq!(bridge.doc.attr(node,"data-key"),Some("z"));
    bridge.state.retained_limit+=1;
    let Value::Host(old)=bridge.set_attribute_node(node_handle(node),&[Value::Host(incoming)]).unwrap()else{panic!("displaced Attr")};
    assert_eq!(bridge.state.retained,final_bytes);assert_eq!(bridge.state.attached_attributes.capacity(),1);assert_eq!(attribute(bridge.doc,node,"data-key").name.qualified.capacity(),8);assert_eq!(bridge.doc.attr(node,"data-key"),Some("new"));
    assert_eq!(bridge.get(old,"value").unwrap().to_text(),"z");assert!(matches!(bridge.get(old,"ownerElement").unwrap(),Value::Null));assert_eq!(bridge.state.attributes[script_attr::index(incoming).unwrap()].value.capacity(),0);
}
#[test]
fn browser_attribute_fragment_retains_decoded_capacity_and_clone_pays_only_compact_copy() {
    let(mut doc,mut forms,mut page)=page("<html><body><div></div></body></html>");let mount=element(&doc,"div");let before=page.state.retained;
    let old_nodes=doc.nodes.capacity();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};bridge.set(node_handle(mount),"innerHTML",Value::String("<span data-key='a&amp;b'></span>".into())).unwrap();let node=element(bridge.doc,"span");
    let attr=attribute(bridge.doc,node,"data-key");assert!(attr.value.capacity()>attr.value.len());let NodeKind::Element{attrs,..}=&bridge.doc.nodes[node].kind else{unreachable!()};
    assert_eq!(bridge.state.retained-before,attrs.capacity()*size_of::<crate::html::Attribute>()+attr.name.owned_bytes()+attr.value.capacity()+"span".len()+child_link_bytes(&bridge.doc.nodes[mount].children)+(bridge.doc.nodes.capacity()-old_nodes)*size_of::<Node>());
    let before=bridge.state.retained;let copied=size_of::<crate::html::Attribute>()+"data-key".len()+"a&b".len()+"span".len();bridge.state.retained_limit=before+copied;
    let Value::Host(cloned)=bridge.clone_node(node_handle(node),false).unwrap()else{panic!("real clone")};let cloned=node_index(cloned).unwrap();assert_eq!(bridge.state.retained,before+copied);assert_eq!(bridge.doc.attr(cloned,"data-key"),Some("a&b"));assert_eq!(attribute(bridge.doc,cloned,"data-key").value.capacity(),3);
}
