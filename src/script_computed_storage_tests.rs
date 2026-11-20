//! Original computed declaration backing and live identity acceptance.
use super::*;
fn page()->(Document,Forms,Page,usize,usize){
    let mut doc=crate::html::parse("<html><head><style>.one{--state:first;overflow:hidden}.two{--state:second;overflow:visible}</style></head><body><p class=one>Text</p></body></html>");doc.scripting_enabled=true;
    let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="p")).unwrap();
    let body=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="body")).unwrap();
    let mut forms=Forms::new(&doc);let mut page=Page::new();
    page.set_geometry(crate::script_geometry::Geometry::new(640,480,1.0,&HashMap::new(),HashMap::new(),&crate::fonts::Book::empty()));
    let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());
    (doc,forms,page,node,body)
}
fn create(bridge:&mut Bridge<'_>,node:usize)->u32{let Value::Host(handle)=bridge.create_computed_style(&[Value::Host(node_handle(node))]).unwrap()else{panic!("genuine computed declaration")};handle}
#[test]
fn browser_computed_declaration_capacity_accounts_seventy_thousand_newobjects_and_live_cascade() {
    let(mut doc,mut forms,mut page,node,body)=page();let baseline=page.state.retained;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let mut first=0;let mut last=0;
    for index in 0..70000{last=create(&mut bridge,node);if index==0{first=last;}assert_eq!(script_computed_style::index(last),Some(index));}
    assert_ne!(first,last);assert_eq!(bridge.state.computed_styles.len(),70000);
    assert_eq!(bridge.state.retained,baseline+bridge.state.computed_styles.capacity()*size_of::<Declaration>());
    for handle in [first,last]{assert_eq!(bridge.call(handle,"getPropertyValue",&[Value::String("--state".into())]).unwrap().to_text(),"first");}
    bridge.set(node_handle(node),"className",Value::String("two".into())).unwrap();
    for handle in [first,last]{assert_eq!(bridge.call(handle,"getPropertyValue",&[Value::String("--state".into())]).unwrap().to_text(),"second");}
    bridge.call(node_handle(body),"removeChild",&[Value::Host(node_handle(node))]).unwrap();
    assert_eq!(bridge.call(first,"getPropertyValue",&[Value::String("--state".into())]).unwrap().to_text(),"");
    bridge.call(node_handle(body),"appendChild",&[Value::Host(node_handle(node))]).unwrap();
    assert_eq!(bridge.call(last,"getPropertyValue",&[Value::String("--state".into())]).unwrap().to_text(),"second");
    assert!(bridge.state.retained<=bridge.state.retained_limit);
}
#[test]
fn browser_computed_declaration_exact_slot_copy_failure_and_retry_publish_no_backing() {
    let(mut doc,mut forms,mut page,node,_)=page();let baseline=page.state.retained;let limit=page.state.retained_limit;let slot=size_of::<Declaration>();
    page.state.retained_limit=baseline+slot-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_computed_style(&[Value::Host(node_handle(node))]).is_err());
    assert_eq!((page.state.retained,page.state.computed_styles.len(),page.state.computed_styles.capacity()),(baseline,0,0));
    page.state.retained_limit+=1;page.begin_job();
    let first=create(&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state},node);
    assert_eq!((page.state.retained,page.state.computed_styles.len(),page.state.computed_styles.capacity()),(baseline+slot,1,1));
    page.state.retained_limit=limit;page.begin_job();page.state.copy_work=slot-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_computed_style(&[Value::Host(node_handle(node))]).unwrap_err().contains("string work limit"));
    assert_eq!((page.state.retained,page.state.computed_styles.len(),page.state.computed_styles.capacity()),(baseline+slot,1,1));
    page.begin_job();let second=create(&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state},node);
    assert_ne!(first,second);assert_eq!(page.state.retained,baseline+page.state.computed_styles.capacity()*slot);
}
