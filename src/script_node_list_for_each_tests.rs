//! Canonical DOMTokenList toggle behavior and native boundaries.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='one two one'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn node_list_for_each_canonical_array_alias_metadata_deletion_overrides_and_generic_calls(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var P=NodeList.prototype,d=Object.getOwnPropertyDescriptor(P,'forEach'),list=document.querySelectorAll('#target'),p=document.getElementById('target');
if(!d||d.value!==Array.prototype.forEach||d.value.name!=='forEach'||d.value.length!==1||!d.enumerable||!d.writable||!d.configurable||Object.prototype.hasOwnProperty.call(list,'forEach'))throw 'canonical NodeList alias';
var each=d.value;delete P.forEach;if(list.forEach!==undefined||'forEach' in list)throw 'ghost native method';var seen=[];each.call(list,function(value,index,source){if(source!==list||index!==0||value!==p)throw 'saved callback';seen.push(value);});if(seen.length!==1)throw 'saved count';
var ordinary={0:'ordinary',length:1};each.call(ordinary,function(value,index,source){if(value!=='ordinary'||index!==0||source!==ordinary)throw 'generic callback';});
P.forEach=function(){return 'shadow';};if(list.forEach()!=='shadow')throw 'prototype override';list.forEach=17;if(list.forEach!==17)throw 'own override';delete list.forEach;P.forEach=each;
if(list.item(0)!==p||list.item(1)!==null)throw 'native item';try{list.item.call({},0);throw 'item brand missing';}catch(e){if(!(e instanceof TypeError))throw e;}
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);if(frame.contentWindow.NodeList.prototype.forEach!==frame.contentWindow.Array.prototype.forEach||frame.contentWindow.NodeList.prototype.forEach===each)throw 'owner alias identity';
"#);}
#[test]
fn node_list_for_each_static_and_live_sources_capture_length_and_observe_current_indices(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var root=document.createElement('div'),a=document.createElement('span'),b=document.createElement('span'),c=document.createElement('span');root.appendChild(a);root.appendChild(b);document.body.appendChild(root);var snapshot=root.querySelectorAll('span'),live=root.childNodes,seen=[];
live.forEach(function(value,index,source){if(source!==live)throw 'live source';seen.push(value);if(index===0)root.appendChild(c);});if(seen.length!==2||seen[0]!==a||seen[1]!==b||live.length!==3||snapshot.length!==2)throw 'captured growth';
seen=[];live.forEach(function(value,index){seen.push(value);if(index===0){root.removeChild(b);root.removeChild(c);}});if(seen.length!==1||seen[0]!==a)throw 'live shrink holes';
Object.defineProperty(snapshot,'length',{value:3,configurable:true});var gets=0;Object.defineProperty(NodeList.prototype,'2',{get:function(){gets++;return c;},configurable:true});seen=[];snapshot.forEach(function(value){seen.push(value);});delete NodeList.prototype[2];if(seen.length!==3||seen[0]!==a||seen[1]!==b||seen[2]!==c||gets!==1)throw 'snapshot retained nodes and inherited hole';
"#);}
#[test]
fn node_list_for_each_effective_length_conversion_precedes_callback_validation_and_abrupt_effects(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=document.querySelectorAll('#target'),order='',marker={};Object.defineProperty(list,'length',{get:function(){order+='g';return {valueOf:function(){order+='n';p.className='effect';return 1;}};},configurable:true});
try{list.forEach({});throw 'missing callback TypeError';}catch(e){if(!(e instanceof TypeError))throw e;}if(order!=='gn'||p.className!=='effect')throw 'length before IsCallable';
order='';var count=0;list.forEach(function(value){if(value!==p)throw 'effective source';count++;});if(order!=='gn'||count!==1)throw 'once effective length';
Object.defineProperty(list,'length',{get:function(){p.className='retained';throw marker;},configurable:true});try{list.forEach({});throw 'missing length abrupt';}catch(e){if(e!==marker)throw e;}if(p.className!=='retained')throw 'length effects lost';delete list.length;
var calls=0;try{list.forEach(function(){calls++;p.className='callback-effect';throw marker;});throw 'missing callback abrupt';}catch(e){if(e!==marker)throw e;}if(calls!==1||p.className!=='callback-effect')throw 'callback effects lost';
"#);}
#[test]
fn node_list_for_each_actual_child_script_caller_checks_origin_before_callbacks(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var list=document.querySelectorAll('#target'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var childEach=frame.contentWindow.Function('list','callback','return list.forEach(callback);'),calls=0;childEach(list,function(){calls++;});if(calls!==1)throw 'same-origin child caller';calls=0;
"#);
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    page.set_location(&crate::navigation::address("https://foreign-node-callback.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("childEach(list,function(){calls++;})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0)throw 'callback before origin guard';list.forEach(function(){calls++;});if(calls!==1)throw 'fresh owner callback';");
}

#[test]
fn node_list_for_each_effective_length_limit_refuses_before_callback_and_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var list=document.querySelectorAll('#target'),calls=0;Object.defineProperty(list,'length',{value:Infinity,writable:true,configurable:true});");
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    page.begin_job();let error=page.runtime.execute("list.forEach(function(){calls++;})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("Native Array callback length limit"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0)throw 'callback before length admission';list.length=1;list.forEach(function(value,index,source){if(value!==document.getElementById('target')||index!==0||source!==list)throw 'fresh actual source';calls++;});if(calls!==1)throw 'fresh length recovery';delete list.length;");
}
