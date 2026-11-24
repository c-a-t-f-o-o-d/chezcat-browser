//! Canonical Element.id over actual native attribute records.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut doc=crate::html::parse("<!doctype html><html><body><p id=target></p><form id=form><input name=id></form><svg id=shape></svg></body></html>");doc.scripting_enabled=true;
 let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://id-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn element_id_accessor_canonical_descriptor_brand_conversion_and_shadowing(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),d=Object.getOwnPropertyDescriptor(Element.prototype,'id'),calls=0;
if(!d||!d.enumerable||!d.configurable||d.get.name!=='get id'||d.set.name!=='set id'||d.get.length!==0||d.set.length!==1||Object.hasOwn(p,'id'))throw 'canonical descriptor';
d.set.call(p,{toString:function(){calls++;return 'A😺';}});if(calls!==1||d.get.call(p)!=='A😺'||p.id!=='A😺'||document.getElementById('A😺')!==p)throw 'converted scalar';
try{d.set.call({}, {toString:function(){calls++;return 'bad';}});throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}try{d.get.call(document);throw 'document brand';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!==1)throw 'conversion before brand';
try{p.id=Symbol();throw 'symbol accepted';}catch(e){if(!(e instanceof TypeError))throw e;}
var observer=new MutationObserver(function(){});observer.observe(p,{attributes:true,attributeOldValue:true});d.set.call(p,'observed');var records=observer.takeRecords();if(records.length!==1||records[0].oldValue!=='A😺'||records[0].target!==p||records[0].attributeName!=='id')throw 'main observation';observer.disconnect();
p.id=null;if(p.id!=='null')throw 'null';p.id=undefined;if(p.id!=='undefined')throw 'undefined';
var f=document.getElementById('form');if(f.id!==f.getElementsByTagName('input')[0]||d.get.call(f)!=='form')throw 'named form precedence';
var svg=document.getElementById('shape');d.set.call(svg,'shape2');if(d.get.call(svg)!=='shape2'||svg.id!=='shape2')throw 'SVG inherited id';
Object.defineProperty(Element.prototype,'id',{get:function(){return 'author';},configurable:true});if(p.id!=='author'||d.get.call(p)!=='undefined')throw 'prototype shadow';delete Element.prototype.id;if(p.id!==undefined||d.get.call(p)!=='undefined')throw 'deleted prototype';
"#);}
#[test]
fn element_id_accessor_main_raw_record_keeps_exact_units_and_literal_distinction(){
 let(mut doc,mut forms,mut page)=fixture();let target=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let NodeKind::Element{attrs,..}=&mut doc.nodes[target].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is("id")).unwrap().value=DomString::from_units(&[65,0xd800,122],4096).unwrap();
 run(&mut doc,&mut forms,&mut page,r#"var p=document.getElementsByTagName('p')[0],d=Object.getOwnPropertyDescriptor(Element.prototype,'id');if(p.id!=='A\ud800z'||d.get.call(p)!==p.getAttribute('id')||p.id==='A�z'||document.getElementById(p.id)!==p||document.getElementById('A�z')!==null)throw 'exact native record';p.id='A�z';if(p.id!=='A�z'||document.getElementById('A�z')!==p)throw 'scalar recovery';"#);
}
#[test]
fn element_id_accessor_inert_raw_setters_keep_attr_identity_observation_and_lookup(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=Object.getOwnPropertyDescriptor(Element.prototype,'id'),owner=document.implementation.createHTMLDocument('inert'),p=owner.createElement('p');owner.body.appendChild(p);p.setAttribute('id','seed');var a=p.getAttributeNode('id'),observer=new MutationObserver(function(){});observer.observe(p,{attributes:true,attributeOldValue:true});
p.id='A\ud800z';if(p.id!=='A\ud800z'||a.value!==p.id||owner.getElementById(p.id)!==p||owner.getElementById('A�z')!==null)throw 'inert exact';d.set.call(p,'B\udfffz');if(a.value!==p.id||d.get.call(p)!=='B\udfffz')throw 'canonical inert';var records=observer.takeRecords();if(records.length!==2||records[0].oldValue!=='seed'||records[1].oldValue!=='A\ud800z'||records[0].target!==p)throw 'observer exact';
var parsed=new DOMParser().parseFromString('<p id=parsed></p>','text/html'),q=parsed.body.firstChild;q.id='C\ud800z';if(d.get.call(q)!==q.getAttribute('id')||parsed.getElementById(q.id)!==q)throw 'parsed exact';
var marker={},calls=0;try{p.id={toString:function(){calls++;p.setAttribute('data-effect','yes');throw marker;}};throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(calls!==1||p.id!=='B\udfffz'||p.getAttribute('data-effect')!=='yes')throw 'abrupt effects';
"#);}
#[test]
fn element_id_accessor_saved_child_origin_before_conversion_and_recovery(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var d=Object.getOwnPropertyDescriptor(frame.contentWindow.Element.prototype,'id'),calls=0;d.set.call(p,{toString:function(){calls++;return 'same';}});if(calls!==1||d.get.call(p)!=='same')throw 'same origin';calls=0;var child=frame.contentDocument.body;if(d.get.call(child)!==''||child.id!=='')throw 'actual child read';");
 let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;page.set_location(&crate::navigation::address("https://foreign-id.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("d.set.call(p,{toString:function(){calls++;return 'bad';}})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
 page.set_location(&crate::navigation::address("https://id-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0||d.get.call(p)!=='same')throw 'origin order';p.id='fresh';if(p.id!=='fresh')throw 'fresh recovery';");
}

#[test]
fn element_id_accessor_native_get_limits_are_atomic_and_main_raw_writes_still_refuse(){
 let(mut doc,mut forms,mut page)=fixture();let target=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("target")).unwrap();let NodeKind::Element{attrs,..}=&mut doc.nodes[target].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is("id")).unwrap().value=DomString::from_units(&[65,0xd800,122],4096).unwrap();
 let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;let limit=page.state.limits.host.string_bytes;
 for mode in 0..3{page.begin_job();match mode{0=>page.state.host_work=0,1=>page.state.copy_work=0,_=>page.state.limits.host.string_bytes=4};let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.element_id_get(node_handle(target)).unwrap_err();assert!(error.contains("work")||error.contains("allowance"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);page.state.limits.host.string_bytes=limit;}
 page.begin_job();let value=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.element_id_get(node_handle(target)).unwrap();assert!(matches!(value,Value::StringUnits(units) if units==[65,0xd800,122]));
 run(&mut doc,&mut forms,&mut page,"var p=document.getElementsByTagName('p')[0],calls=0;");
 let error=page.runtime.execute(r#"p.id={toString:function(){calls++;p.setAttribute('data-effect','yes');return 'B\udfffz';}}"#,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("Unpaired attribute"),"{error}");
 page.begin_job();run(&mut doc,&mut forms,&mut page,r#"if(calls!==1||p.id!=='A\ud800z'||p.getAttribute('data-effect')!=='yes')throw 'guard and effects';p.id='fresh';if(p.id!=='fresh')throw 'fresh scalar';"#);
}

#[test]
fn element_id_accessor_inert_observer_and_record_refusal_publish_nothing_and_recover(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"var owner=document.implementation.createHTMLDocument('inert'),p=owner.createElement('p');owner.body.appendChild(p);p.id='A\ud800z';var a=p.getAttributeNode('id'),observer=new MutationObserver(function(){});observer.observe(p,{attributes:true,attributeOldValue:true});"#);
 let commits=page.state.dom_commits;let retained=page.state.retained;let limit=page.state.retained_limit;let strings=page.state.limits.host.string_bytes;
 for mode in 0..4{page.begin_job();match mode{0=>page.state.host_work=0,1=>page.state.copy_work=0,2=>page.state.retained_limit=retained,_=>page.state.limits.host.string_bytes=4};let error=page.runtime.execute(r#"p.id='long B\udfffz';"#,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work")||error.contains("allowance")||error.contains("bound"),"{error}");assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);page.state.retained_limit=limit;page.state.limits.host.string_bytes=strings;}
 page.begin_job();run(&mut doc,&mut forms,&mut page,r#"if(p.id!=='A\ud800z'||a.value!==p.id||observer.takeRecords().length!==0)throw 'refusal changed record';p.id='fresh B\udfffz';var records=observer.takeRecords();if(records.length!==1||records[0].oldValue!=='A\ud800z'||a.value!==p.id)throw 'fresh notification';"#);
}
