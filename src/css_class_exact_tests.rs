//! Exact class-selector consumers over authoritative native attribute records.
use super::*;
fn fixture()->(Document,usize){
    let doc=crate::html::parse("<!doctype html><html><head><style>p{color:#123456}.A�z{color:#aa2233}.plain{background:#654321}.A😺{width:44px}</style></head><body><p id=target class=plain></p></body></html>");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();(doc,node)
}
fn record(doc:&mut Document,node:usize,units:&[u16]){
    let NodeKind::Element{attrs,..}=&mut doc.nodes[node].kind else{unreachable!()};
    attrs.iter_mut().find(|a|a.name.html_is("class")).unwrap().value=crate::dom_string::DomString::from_units(units,4096).unwrap();
}
fn selected(doc:&Document,node:usize,selector:&str)->bool{let mut work=100_000;query_first(doc,selector,&[node],None,&mut work).unwrap().is_some()}
#[test]
fn class_exact_raw_record_does_not_alias_replacement_in_dom_or_cascade(){
    let(mut doc,node)=fixture();record(&mut doc,node,&[65,0xd800,122]);
    assert!(!selected(&doc,node,".A�z"));assert!(selected(&doc,node,":not(.A�z)"));
    let styles=compute(&doc,&HashMap::new());assert_eq!(styles[node].color,0x123456);
    record(&mut doc,node,&"A�z".encode_utf16().collect::<Vec<_>>());assert!(selected(&doc,node,".A�z"));assert_eq!(compute(&doc,&HashMap::new())[node].color,0xaa2233);
}
#[test]
fn class_exact_scalar_vertical_tab_and_non_ascii_space_are_not_delimiters(){
    let(mut doc,node)=fixture();
    for value in ["plain\u{b}other","plain\u{a0}other","plain\0other"]{
        record(&mut doc,node,&value.encode_utf16().collect::<Vec<_>>());assert!(!selected(&doc,node,".plain"),"{value:?}");assert_eq!(compute(&doc,&HashMap::new())[node].background,None,"{value:?}");
    }
    for whitespace in [' ','\t','\n','\u{c}','\r']{let value=format!("before{whitespace}plain{whitespace}after");record(&mut doc,node,&value.encode_utf16().collect::<Vec<_>>());assert!(selected(&doc,node,".plain"));assert_eq!(compute(&doc,&HashMap::new())[node].background,Some(0x654321));}
}
#[test]
fn class_exact_raw_record_cached_candidates_keep_scalar_tokens_pairs_and_cascade_order(){
    let(mut doc,node)=fixture();let sources=HashMap::new();let first=compute_with_environment_and_properties_cached(&doc,&sources,None,None,None);
    let mut units:Vec<_>="A".encode_utf16().chain([0xdfff]).chain("z plain A😺".encode_utf16()).collect();
    record(&mut doc,node,&units);
    let next=compute_with_environment_and_properties_cached(&doc,&sources,None,None,first.stylesheet_cache.as_ref());
    assert!(selected(&doc,node,".A😺"),"direct pair selector; class={:?}",doc.dom_attr(node,"class"));
    assert_eq!(next.styles[node].color,0x123456);assert_eq!(next.styles[node].background,Some(0x654321));assert_eq!(next.styles[node].width,Length::Px(44.0));
    assert!(selected(&doc,node,".plain.A😺:not(.A�z)"));assert!(!selected(&doc,node,".A�z,.absent"));
    units="A�z".encode_utf16().collect();record(&mut doc,node,&units);
    let final_result=compute_with_environment_and_properties_cached(&doc,&sources,None,None,next.stylesheet_cache.as_ref());
    assert_eq!(final_result.styles[node].color,0xaa2233);assert_eq!(final_result.styles[node].background,None);assert!(!selected(&doc,node,".plain"));
}
#[test]
fn class_exact_raw_record_mixed_null_tokens_keep_exact_attribute_selector_agreement(){
    let(mut doc,node)=fixture();record(&mut doc,node,&[0xd800,32,65,0xfffd,122,32,0x70,0x6c,0x61,0x69,0x6e,0xb,0x78]);
    assert!(selected(&doc,node,".A�z"));assert!(selected(&doc,node,"[class~='A�z']"));assert!(!selected(&doc,node,".plain"));assert!(!selected(&doc,node,"[class~='plain']"));
    assert_eq!(compute(&doc,&HashMap::new())[node].color,0xaa2233);
}
#[test]
fn class_exact_query_work_refusal_preserves_native_record_and_recovers(){
    let(mut doc,node)=fixture();let units:Vec<_>="plain ".encode_utf16().chain([0xd800]).chain(" tail".encode_utf16()).collect();record(&mut doc,node,&units);let before=format!("{:?}",doc.nodes);
    let mut work=0;assert!(query_first(&doc,".plain",&[node],None,&mut work).is_err());assert_eq!(work,0);assert_eq!(format!("{:?}",doc.nodes),before);
    let(mut work,mut copies)=(0,1_000_000);assert!(compute_with_environment_and_properties_budgeted_cached(&doc,&HashMap::new(),None,None,&mut CssBudget::new(&mut work,&mut copies),None).is_err());assert_eq!(work,0);assert_eq!(format!("{:?}",doc.nodes),before);
    assert!(selected(&doc,node,".plain"));assert_eq!(compute(&doc,&HashMap::new())[node].background,Some(0x654321));
}
