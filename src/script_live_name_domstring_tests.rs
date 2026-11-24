//! Live name filters compare actual native DOMString records.
use super::*;
use super::queries::NodeList;
fn fixture()->(Document,Forms,Page){
 let mut doc=crate::html::parse("<!doctype html><html><body><input id=raw name='A�z'><input id=literal name='A�z'><input id=other name=seed><div id=mount></div><template><input name='A�z'></template><svg name='A�z'></svg></body></html>");doc.scripting_enabled=true;
 let n=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("raw")).unwrap();let NodeKind::Element{attrs,..}=&mut doc.nodes[n].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is("name")).unwrap().value=DomString::from_units(&[65,0xd800,122],4096).unwrap();
 let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn live_name_domstring_scalar_queries_skip_raw_native_projection_aliases(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var literal=document.getElementById('literal'),list=document.getElementsByName('A�z');if(list.length!==1||list[0]!==literal||list.item(1)!==null)throw 'actual scalar name';"#);
}
#[test]
fn live_name_domstring_raw_queries_keep_filter_identity_through_live_changes(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var p=document.getElementById('raw'),mount=document.getElementById('mount'),list=document.getElementsByName('A\ud800z');if(list.length!==1||list[0]!==p||list.item(0)!==p||document.getElementsByName('B\udfffz').length!==0)throw 'actual raw name';mount.appendChild(p);if(list.length!==1||list[0]!==p)throw 'live move';p.setAttribute('name','moved');if(list.length!==0||document.getElementsByName('moved')[0]!==p)throw 'live name update';"#);
}
#[test]
fn live_name_domstring_conversion_is_once_abrupt_effects_survive_and_brand_precedes_it(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var p=document.getElementById('raw'),calls=0,list=document.getElementsByName({toString:function(){calls++;p.value='converted';return 'A\ud800z';}});if(calls!==1||list.length!==1||list[0]!==p||p.value!=='converted')throw 'once before live view';var marker={};try{document.getElementsByName({toString:function(){calls++;p.value='abrupt';throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(calls!==2||p.value!=='abrupt')throw 'abrupt effect';try{document.getElementsByName.call({}, {toString:function(){calls++;return 'x';}});throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!==2)throw 'brand before coercion';try{document.getElementsByName(Symbol());throw 'missing symbol';}catch(e){if(!(e instanceof TypeError))throw e;}"#);
}
#[test]
fn live_name_domstring_filter_resource_refusal_and_live_read_recover_atomically(){
 let(mut doc,mut forms,mut page)=fixture();let value=Value::StringUnits(vec![65,0xd800,122]);let retained=page.state.retained;let limit=page.state.retained_limit;let strings=page.state.limits.host.string_bytes;
 for mode in 0..3{page.begin_job();match mode{0=>page.state.copy_work=0,1=>page.state.retained_limit=retained,_=>page.state.limits.host.string_bytes=4};let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_name_list(&value).unwrap_err();assert!(error.contains("work")||error.contains("allowance"),"{error}");assert_eq!(page.state.node_lists.len(),0);assert_eq!(page.state.retained,retained);page.state.retained_limit=limit;page.state.limits.host.string_bytes=strings;}
 page.begin_job();let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_name_list(&value).unwrap()else{panic!("list")};let after=page.state.retained;let NodeList::Name(name)=&page.state.node_lists[0]else{panic!("filter")};assert_eq!(name.raw_units(),Some(&[65,0xd800,122][..]));assert_eq!(after-retained,page.state.node_lists.capacity()*size_of::<NodeList>()+name.capacity());page.state.host_work=0;assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.node_list_get(handle,"length").is_err());assert_eq!(page.state.retained,after);page.begin_job();assert_eq!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.node_list_get(handle,"length").unwrap().to_number(),1.0);
}
