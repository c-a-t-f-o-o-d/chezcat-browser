//! Exact HTMLCollection named lookup through real native collections.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><div id=host><img id='A�z'><img id=literal name='A�z'></div></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://named-collection.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn collection_named_exact_public_inert_id_name_order_and_live_updates(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createHTMLDocument('owner'),a=d.createElement('p'),b=d.createElement('p'),c=d.createElement('p'),raw='A\ud800z';
d.body.appendChild(a);d.body.appendChild(b);d.body.appendChild(c);a.setAttribute('id',raw);b.setAttribute('name','A�z');c.setAttribute('id','A�z');var list=d.body.children,tag=d.getElementsByTagName('p'),saved=HTMLCollection.prototype.namedItem;
if(list.namedItem(raw)!==a||tag.namedItem(raw)!==a||saved.call(list,{toString:function(){return raw;}})!==a)throw 'exact raw ID';
if(list.namedItem('A�z')!==b||list['A�z']!==b||tag.namedItem('A�z')!==b)throw 'first exact ID or HTML name';
a.getAttributeNode('id').value='changed';c.setAttribute('name',raw);if(list.namedItem(raw)!==c||list.namedItem('changed')!==a)throw 'live records';
a.setAttributeNS(null,'id',raw);if(list.namedItem(raw)!==a)throw 'first exact update';d.body.removeChild(a);if(list.namedItem(raw)!==c||list.namedItem('')!==null||list.namedItem('absent')!==null)throw 'live removal and misses';
"#);}
#[test]
fn collection_named_exact_saved_conversion_brands_and_abrupt_effects(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createHTMLDocument('owner'),p=d.createElement('p');d.body.appendChild(p);var list=d.body.children,f=HTMLCollection.prototype.namedItem,n=0,raw='A\udfffz';
if(f.name!=='namedItem'||f.length!==1)throw 'canonical metadata';
if(f.call(list,{toString:function(){n++;p.setAttribute('id',raw);return raw;}})!==p||n!==1)throw 'once conversion before live read';
var marker={},caught=false;try{f.call(list,{toString:function(){p.setAttribute('id','changed');throw marker;}});}catch(e){caught=e===marker;}if(!caught||list.namedItem('changed')!==p)throw 'abrupt author effect';
var count=0;try{f.call({}, {toString:function(){n++;return raw;}});}catch(e){if(e instanceof TypeError)count++;}try{f.call(list,Symbol());}catch(e){if(e instanceof TypeError)count++;}try{f.call(list);}catch(e){if(e instanceof TypeError)count++;}if(count!==3||n!==1)throw 'brand arity symbol';
p.setAttribute('id','null');if(f.call(list,null)!==p)throw 'null conversion';p.setAttribute('id','undefined');if(f.call(list,undefined)!==p)throw 'undefined conversion';
"#);}
#[test]
fn collection_named_exact_main_and_image_native_raw_records_skip_projection_aliases(){
    let(mut doc,mut forms,mut page)=fixture();let image=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("A�z")).unwrap();
    let NodeKind::Element{attrs,..}=&mut doc.nodes[image].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is("id")).unwrap().value=DomString::from_units(&[65,0xd800,122],4096).unwrap();
    run(&mut doc,&mut forms,&mut page,r#"
var first=document.images[0],literal=document.getElementById('literal'),raw='A\ud800z',list=document.getElementById('host').children,f=HTMLCollection.prototype.namedItem;
if(list.namedItem(raw)!==first||document.getElementsByTagName('img').namedItem(raw)!==first||document.images.namedItem(raw)!==first||f.call(document.images,raw)!==first)throw 'actual raw node';
if(list.namedItem('A�z')!==literal||list['A�z']!==literal||document.images.namedItem('A�z')!==literal||document.images['A�z']!==literal)throw 'literal name skips raw alias';
"#);}
#[test]
fn collection_named_exact_saved_child_origin_precedes_conversion_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var f=frame.contentWindow.HTMLCollection.prototype.namedItem,d=document.implementation.createHTMLDocument('owner'),p=d.createElement('p'),raw='A\\ud800z',calls=0;d.body.appendChild(p);p.setAttribute('id',raw);var list=d.body.children;if(f.call(list,raw)!==p)throw 'same origin borrowed';");
    page.set_location(&crate::navigation::address("https://foreign-collection.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    let error=page.runtime.execute("f.call(list,{toString:function(){calls++;return raw;}})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(format!("{:?}",doc.nodes),nodes);assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://named-collection.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0||f.call(list,raw)!==p)throw 'fresh origin recovery';");
}
#[test]
fn collection_named_exact_native_work_copy_refusal_preserves_records_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var d=document.implementation.createHTMLDocument('owner'),p=d.createElement('p'),raw='A\\ud800z';d.body.appendChild(p);p.setAttribute('id',raw);var list=d.body.children;");
    let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;
    for mode in 0..2{page.begin_job();if mode==0{page.state.host_work=0;}else{page.state.copy_work=0;}let error=page.runtime.execute("list.namedItem(raw)",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work")||error.contains("allowance"),"{error}");assert_eq!(format!("{:?}",doc.nodes),nodes);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);}
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(list.namedItem(raw)!==p||p.getAttribute('id')!==raw)throw 'fresh exact recovery';");
}

#[test]
fn collection_named_exact_borrowed_match_needs_no_copy_headroom_and_keeps_key_limit(){
    let(mut doc,mut forms,mut page)=fixture();let image=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("A�z")).unwrap();let units=[65,0xd800,122];
    let NodeKind::Element{attrs,..}=&mut doc.nodes[image].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is("id")).unwrap().value=DomString::from_units(&units,4096).unwrap();
    let nodes=format!("{:?}",doc.nodes);let retained=page.state.retained;let limit=page.state.retained_limit;
    {let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};bridge.state.host_work=0;
    assert!(bridge.collection_element_named_match(node_handle(image),crate::dom_string::DomStringRef::Raw(&units)).unwrap_err().contains("work"));
    bridge.state.host_work=bridge.state.limits.host_work;bridge.state.copy_work=0;bridge.state.retained_limit=retained;
    assert!(bridge.collection_element_named_match(node_handle(image),crate::dom_string::DomStringRef::Raw(&units)).unwrap());assert_eq!(bridge.state.retained,retained);}
    assert_eq!(format!("{:?}",doc.nodes),nodes);page.state.retained_limit=limit;page.begin_job();
    run(&mut doc,&mut forms,&mut page,"var d=document.implementation.createHTMLDocument('owner'),p=d.createElement('p'),raw='A\\ud800z';d.body.appendChild(p);p.setAttribute('id',raw);var list=d.body.children;");
    let strings=page.state.limits.host.string_bytes;
    for source in ["list.namedItem(raw)","document.images.namedItem(raw)"]{page.begin_job();page.state.limits.host.string_bytes=4;let error=page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("string allowance"),"{error}");page.state.limits.host.string_bytes=strings;}
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(list.namedItem(raw)!==p||document.images.namedItem(raw)!==document.images[0])throw 'fresh bounded lookup';");
}
