//! Original element attribute record backing and clone/transfer acceptance.
use super::*;
fn page(markup:&str)->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document(markup,&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn element(doc:&Document,name:&str)->usize{doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag==name)).unwrap()}
fn attrs(doc:&Document,node:usize)->&Vec<crate::html::Attribute>{let NodeKind::Element{attrs,..}=&doc.nodes[node].kind else{unreachable!()};attrs}
#[test]
fn browser_element_attribute_backing_exceeds_four_thousand_with_real_names_values_and_live_map() {
    let(mut doc,mut forms,mut page)=page("<html><body><p id=target></p></body></html>");let node=element(&doc,"p");
    let before=page.state.retained;let backing=attrs(&doc,node).capacity()*size_of::<crate::html::Attribute>();
    page.runtime.execute("var target=document.getElementById('target');for(var i=0;i<6000;i++)target.setAttribute('data-k'+i,'v');if(target.attributes.length!==6001||target.getAttribute('data-k5999')!=='v')throw 'real attributes';",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
    let names=(0..6000).map(|index|format!("data-k{index}").len()+1).sum::<usize>();
    assert_eq!(attrs(&doc,node).len(),6001);
    assert_eq!(page.state.retained,before+attrs(&doc,node).capacity()*size_of::<crate::html::Attribute>()-backing+names);
    assert!(page.state.retained<=page.state.retained_limit);
}
#[test]
fn browser_element_attribute_exact_backing_and_copy_refusal_publish_no_vector_or_value() {
    let(mut doc,mut forms,mut page)=page("<html><body><p></p></body></html>");let node=element(&doc,"p");let baseline=page.state.retained;let limit=page.state.retained_limit;
    let bundle=size_of::<crate::html::Attribute>()+"title".len()+1;page.state.retained_limit=baseline+bundle-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("title".into()),Some("x".into())).is_err());
    assert_eq!(attrs(&doc,node).capacity(),0);assert_eq!(page.state.retained,baseline);assert_eq!(doc.attr(node,"title"),None);
    page.state.retained_limit+=1;page.begin_job();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("title".into()),Some("x".into())).unwrap();
    assert_eq!(attrs(&doc,node).capacity(),1);assert_eq!(page.state.retained,baseline+bundle);
    let commits=page.state.dom_commits;page.state.retained_limit=limit;page.begin_job();page.state.copy_work="title".len();
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("role".into()),Some("y".into())).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.dom_commits,commits);assert_eq!(attrs(&doc,node).capacity(),1);assert_eq!(doc.attr(node,"role"),None);assert_eq!(page.state.retained,baseline+bundle);
    page.begin_job();Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.public_attribute(node,crate::html::AttributeName::unqualified("role".into()),Some("y".into())).unwrap();
    assert_eq!(page.state.retained,baseline+attrs(&doc,node).capacity()*size_of::<crate::html::Attribute>()+"title".len()+1+"role".len()+1);
}
#[test]
fn browser_element_attribute_native_identity_index_and_dom_backing_share_one_exact_bundle() {
    let(mut doc,mut forms,mut page)=page("<html><body><p></p></body></html>");let node=element(&doc,"p");
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let Value::Host(attr)=bridge.create_attribute(&[Value::String("title".into())],false).unwrap() else{panic!("genuine Attr")};
    bridge.attr_set(attr,"value",Value::String("x".into())).unwrap();let before=bridge.state.retained;
    let bundle=size_of::<crate::html::Attribute>()+size_of::<usize>()+"title".len();bridge.state.retained_limit=before+bundle-1;
    assert!(bridge.set_attribute_node(node_handle(node),&[Value::Host(attr)]).is_err());
    assert_eq!(bridge.state.retained,before);assert_eq!(attrs(bridge.doc,node).capacity(),0);assert_eq!(bridge.state.attached_attributes.capacity(),0);
    bridge.state.retained_limit+=1;
    assert!(matches!(bridge.set_attribute_node(node_handle(node),&[Value::Host(attr)]).unwrap(),Value::Null));
    assert_eq!(bridge.state.retained,before+bundle);assert_eq!(attrs(bridge.doc,node).capacity(),1);assert_eq!(bridge.state.attached_attributes.capacity(),1);assert_eq!(bridge.doc.attr(node,"title"),Some("x"));
}
#[test]
fn browser_element_attribute_clone_pays_compact_copy_and_fragments_retain_moved_backing() {
    let(mut doc,mut forms,mut page)=page("<html><body><p title=old></p><div></div></body></html>");let node=element(&doc,"p");let mount=element(&doc,"div");let limit=page.state.retained_limit;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for index in 0..18{bridge.public_attribute(node,crate::html::AttributeName::unqualified(format!("data-k{index}")),Some("x".into())).unwrap();}
    for index in 0..18{bridge.public_attribute(node,crate::html::AttributeName::unqualified(format!("data-k{index}")),None).unwrap();}
    assert!(attrs(bridge.doc,node).capacity()>attrs(bridge.doc,node).len());
    let before=bridge.state.retained;let copied=copied_node_bytes(&bridge.doc.nodes[node])+"p".len()+bridge.main_node_minimum(1).unwrap();bridge.state.retained_limit=before+copied;
    let Value::Host(cloned)=bridge.clone_node(node_handle(node),false).unwrap()else{panic!("real clone")};let cloned=node_index(cloned).unwrap();
    assert_eq!(attrs(bridge.doc,cloned).len(),1);assert_eq!(attrs(bridge.doc,cloned).capacity(),1);assert_eq!(bridge.state.retained,before+copied);assert_eq!(bridge.doc.attr(cloned,"title"),Some("old"));
    bridge.state.retained_limit=limit;let before=bridge.state.retained;
    bridge.set(node_handle(mount),"innerHTML",Value::String("<span title=fragment></span>".into())).unwrap();
    let span=element(bridge.doc,"span");assert_eq!(bridge.doc.attr(span,"title"),Some("fragment"));
    assert!(bridge.state.retained-before>=attrs(bridge.doc,span).capacity()*size_of::<crate::html::Attribute>());
}

#[test]
fn regression_element_attribute_count_guard_remains_explicit_and_preserves_existing_values() {
    let mut doc=crate::html::parse("<html><body><p title=old></p></body></html>");doc.scripting_enabled=true;
    let node=element(&doc,"p");let mut forms=Forms::new(&doc);let mut limits=crate::javascript::limits::CAPACITY_REGRESSION;limits.host.element_attributes=1;
    let mut page=Page::with_limits(limits);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    let retained=page.state.retained;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(bridge.public_attribute(node,crate::html::AttributeName::unqualified("role".into()),Some("new".into())).unwrap_err().contains("Element attribute limit reached (1)"));
    assert_eq!(bridge.state.retained,retained);assert_eq!(bridge.doc.attr(node,"title"),Some("old"));assert_eq!(attrs(bridge.doc,node).len(),1);
    bridge.public_attribute(node,crate::html::AttributeName::unqualified("title".into()),Some("kept".into())).unwrap();
    assert_eq!(bridge.doc.attr(node,"title"),Some("kept"));assert_eq!(attrs(bridge.doc,node).len(),1);
}

#[test]
fn browser_element_attribute_record_index_and_native_input_payload_fit_one_joint_bundle() {
    let(mut doc,mut forms,mut page)=page("<html><body></body></html>");
    let Value::Host(input)=page.runtime.execute("document.createElement('input');",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap()else{panic!("genuine fresh input")};let node=node_index(input).unwrap();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let Value::Host(attr)=bridge.create_attribute(&[Value::String("type".into())],false).unwrap()else{panic!("genuine Attr")};bridge.attr_set(attr,"value",Value::String("checkbox".into())).unwrap();
    let before=bridge.state.retained;let growth=size_of::<crate::html::Attribute>()+size_of::<usize>()+"type".len()+"on".len()+2*size_of::<char>();bridge.state.retained_limit=before+growth-1;
    assert!(bridge.set_attribute_node(input,&[Value::Host(attr)]).is_err());
    assert_eq!(bridge.state.retained,before);assert!(!bridge.forms.controls.contains_key(&node));assert_eq!(attrs(bridge.doc,node).capacity(),0);assert_eq!(bridge.state.attached_attributes.capacity(),0);
    bridge.state.retained_limit+=1;
    bridge.set_attribute_node(input,&[Value::Host(attr)]).unwrap();
    assert_eq!(bridge.state.retained,before+growth);assert_eq!(attrs(bridge.doc,node).capacity(),1);assert_eq!(bridge.state.attached_attributes.capacity(),1);assert_eq!(bridge.forms.controls[&node].kind,crate::forms::Kind::Checkbox);
}
