//! Canonical NodeList item bindings and real node reads.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target>text</p></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://node-item.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn node_list_item_canonical_metadata_overrides_deletion_and_saved_brand(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var P=NodeList.prototype,d=Object.getOwnPropertyDescriptor(P,'item'),list=document.querySelectorAll('#target'),p=document.getElementById('target');
if(!d||typeof d.value!=='function'||d.value.name!=='item'||d.value.length!==1||!d.enumerable||!d.writable||!d.configurable||Object.hasOwn(list,'item'))throw 'canonical item';
var item=d.value;if(list.item!==item||item.call(list,0)!==p||item.call(list,1)!==null)throw 'saved actual nodes';
list.item=function(){return 17;};if(list.item()!==17)throw 'own override';delete list.item;P.item=function(){return 23;};if(list.item()!==23)throw 'prototype override';delete P.item;if(list.item!==undefined||'item' in list)throw 'ghost item';if(item.call(list,0)!==p)throw 'saved after delete';
var n=0;try{item.call({}, {valueOf:function(){n++;return 0;}});throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}if(n!==0)throw 'conversion before brand';
"#);}
#[test]
fn node_list_item_unsigned_long_once_conversion_live_reads_and_abrupt_effects(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),root=document.createElement('div'),a=document.createElement('span'),b=document.createElement('span');root.appendChild(a);root.appendChild(b);var list=root.childNodes,n=0;
if(list.item(undefined)!==a||list.item(NaN)!==a||list.item(Infinity)!==a||list.item(-Infinity)!==a||list.item(4294967296)!==a||list.item(4294967297.9)!==b||list.item(-4294967295)!==b||list.item(-1)!==null)throw 'unsigned long conversion';
var value=list.item({valueOf:function(){n++;root.removeChild(a);return 0;}}, {valueOf:function(){throw 'extra conversion';}});if(n!==1||value!==b)throw 'once conversion then live read';
var marker={};try{list.item({valueOf:function(){p.className='effect';throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(p.className!=='effect')throw 'conversion effect lost';
try{list.item();throw 'missing argument accepted';}catch(e){if(!(e instanceof TypeError))throw e;}try{list.item(Symbol());throw 'symbol accepted';}catch(e){if(!(e instanceof TypeError))throw e;}try{list.item(BigInt(1));throw 'BigInt accepted';}catch(e){if(!(e instanceof TypeError))throw e;}
"#);}
#[test]
fn node_list_item_static_live_inert_and_child_owner_real_identities(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var item=NodeList.prototype.item,p=document.getElementById('target'),snapshot=document.querySelectorAll('#target');p.remove();if(item.call(snapshot,0)!==p)throw 'static retained node';
var inert=document.implementation.createHTMLDocument('inert'),node=inert.createElement('div');inert.body.appendChild(node);if(item.call(inert.body.childNodes,0)!==node)throw 'inert node';
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,child=w.document.body.childNodes,childItem=w.NodeList.prototype.item;if(typeof childItem!=='function'||childItem===item||childItem.name!=='item'||childItem.length!==1||childItem.call(snapshot,0)!==p||item.call(child,0)!==null)throw 'child owner method';
"#);}
#[test]
fn node_list_item_saved_child_method_checks_origin_before_conversion_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var list=document.querySelectorAll('#target'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var item=frame.contentWindow.NodeList.prototype.item,calls=0;item.call(list,{valueOf:function(){calls++;return 0;}});if(calls!==1)throw 'same origin';calls=0;");
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    page.set_location(&crate::navigation::address("https://foreign-item.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("item.call(list,{valueOf:function(){calls++;return 0;}})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://node-item.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0)throw 'conversion before origin';if(item.call(list,0)!==document.getElementById('target'))throw 'fresh recovery';");
}

#[test]
fn node_list_item_native_work_refusal_preserves_nodes_and_fresh_job_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var root=document.getElementById('target'),list=root.childNodes,item=NodeList.prototype.item,first=list[0];");
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    page.begin_job();page.state.host_work=0;let error=page.runtime.execute("item.call(list,0)",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work")||error.contains("allowance"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(item.call(list,0)!==first||first.nodeType!==3||first.data!=='text')throw 'fresh native Text recovery';");
}
