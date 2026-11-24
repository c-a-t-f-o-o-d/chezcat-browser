//! Exact ID selectors over authoritative native attribute values.
use super::*;
fn fixture()->(Document,usize){
    let doc=crate::html::parse("<!doctype html><html><head><style>p{color:#123456}#A�z{color:#aa2233}#plain{background:#654321}#A😺{width:44px}</style></head><body><p id='A�z'></p></body></html>");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("A�z")).unwrap();(doc,node)
}
fn record(doc:&mut Document,node:usize,units:&[u16]){
    let NodeKind::Element{attrs,..}=&mut doc.nodes[node].kind else{unreachable!()};
    attrs.iter_mut().find(|a|a.name.html_is("id")).unwrap().value=crate::dom_string::DomString::from_units(units,4096).unwrap();
}
fn selected(doc:&Document,node:usize,selector:&str)->bool{let mut work=100_000;query_first(doc,selector,&[node],None,&mut work).unwrap().is_some()}
#[test]
fn id_exact_raw_record_does_not_alias_replacement_in_dom_or_cascade(){
    let(mut doc,node)=fixture();record(&mut doc,node,&[65,0xd800,122]);
    assert!(!selected(&doc,node,"#A�z"));assert!(selected(&doc,node,"p:not(#A�z)"));assert!(!selected(&doc,node,"#A�z,#absent"));
    assert_eq!(compute(&doc,&HashMap::new())[node].color,0x123456);
    record(&mut doc,node,&"A�z".encode_utf16().collect::<Vec<_>>());assert!(selected(&doc,node,"#A�z[id='A�z']"));assert_eq!(compute(&doc,&HashMap::new())[node].color,0xaa2233);
}
#[test]
fn id_exact_cached_candidates_follow_authoritative_scalar_and_pair_values(){
    let(mut doc,node)=fixture();let sources=HashMap::new();let first=compute_with_environment_and_properties_cached(&doc,&sources,None,None,None);
    record(&mut doc,node,&"plain".encode_utf16().collect::<Vec<_>>());
    let next=compute_with_environment_and_properties_cached(&doc,&sources,None,None,first.stylesheet_cache.as_ref());
    assert!(selected(&doc,node,"#plain"));assert_eq!(next.styles[node].color,0x123456);assert_eq!(next.styles[node].background,Some(0x654321));
    record(&mut doc,node,&"A😺".encode_utf16().collect::<Vec<_>>());let pair=compute_with_environment_and_properties_cached(&doc,&sources,None,None,next.stylesheet_cache.as_ref());
    assert!(selected(&doc,node,"#A😺"));assert_eq!(pair.styles[node].width,Length::Px(44.0));assert_eq!(pair.styles[node].background,None);
    record(&mut doc,node,&[65,0xdfff,122]);let raw=compute_with_environment_and_properties_cached(&doc,&sources,None,None,pair.stylesheet_cache.as_ref());
    assert_eq!(raw.styles[node].color,0x123456);
}
#[test]
fn id_exact_whole_value_is_not_a_token_list_or_render_projection(){
    let(mut doc,node)=fixture();
    for value in ["plain other","plain\tother","plain\u{b}other","plain\u{a0}other","plain\0other"] {
        record(&mut doc,node,&value.encode_utf16().collect::<Vec<_>>());assert!(!selected(&doc,node,"#plain"));assert!(!selected(&doc,node,"#A�z"));assert_eq!(compute(&doc,&HashMap::new())[node].background,None);
    }
}
#[test]
fn id_exact_query_and_cascade_work_refusal_preserve_record_and_recover(){
    let(mut doc,node)=fixture();record(&mut doc,node,&[65,0xd800,122]);let before=format!("{:?}",doc.nodes);
    let mut work=0;assert!(query_first(&doc,"#A�z",&[node],None,&mut work).is_err());assert_eq!(work,0);
    let(mut work,mut copies)=(0,1_000_000);assert!(compute_with_environment_and_properties_budgeted_cached(&doc,&HashMap::new(),None,None,&mut CssBudget::new(&mut work,&mut copies),None).is_err());assert_eq!(work,0);assert_eq!(format!("{:?}",doc.nodes),before);
    assert!(!selected(&doc,node,"#A�z"));assert_eq!(compute(&doc,&HashMap::new())[node].color,0x123456);
}
