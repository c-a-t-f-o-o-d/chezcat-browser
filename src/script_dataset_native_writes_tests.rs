//! Dataset named mutations use native attribute conversion and observation.
use super::*;
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->Value{p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()}
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><p data-first-name='old'></p></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}

#[test]
fn dataset_native_writes_name_errors_follow_value_conversion(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var node=document.querySelector('p'),data=node.dataset,watch=new MutationObserver(function(){});watch.observe(node,{attributes:true});for(var key of ['x-y','-a','a-bC']){var hits=0,error;try{data[key]={toString:function(){hits++;return 'value';}};}catch(e){error=e;}if(hits!==1||!(error instanceof DOMException)||error.name!=='SyntaxError')throw 'named setter SyntaxError after value conversion';}for(var key of ['a b','a/b','a=b','a>b','a'+String.fromCharCode(0)]){var hits=0,error;try{data[key]={toString:function(){hits++;return 'value';}};}catch(e){error=e;}if(hits!==1||!(error instanceof DOMException)||error.name!=='InvalidCharacterError')throw 'native attribute local-name error';}if(watch.takeRecords().length||Object.keys(data).join(',')!=='firstName'||data.firstName!=='old')throw 'invalid writes must not mutate';
"#);}

#[test]
fn dataset_native_writes_abrupt_conversion_identity_and_author_effect(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var node=document.querySelector('p'),data=node.dataset,marker={},watch=new MutationObserver(function(){});watch.observe(node,{attributes:true,attributeOldValue:true});var error;try{data['a-b']={toString:function(){node.setAttribute('data-author','effect');throw marker;}};}catch(e){error=e;}var r=watch.takeRecords();if(error!==marker||data.author!=='effect'||r.length!==1||r[0].target!==node||r[0].attributeName!=='data-author')throw 'same abrupt value and actual author mutation';
"#);}

#[test]
fn dataset_native_writes_raw_values_keep_live_attr_identity_and_descriptors(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var node=document.querySelector('p'),data=node.dataset,attr=node.getAttributeNode('data-first-name'),raw='a'+String.fromCharCode(55296)+String.fromCharCode(0)+String.fromCharCode(56320);data.firstName=raw;var desc=Object.getOwnPropertyDescriptor(data,'firstName');if(data!==node.dataset||data.firstName!==raw||node.getAttribute('data-first-name')!==raw||attr!==node.getAttributeNode('data-first-name')||attr.value!==raw||desc.value!==raw||!desc.writable||!desc.enumerable||!desc.configurable)throw 'native exact UTF16 attribute backing';Object.defineProperty(data,'secondName',{value:raw});if(node.getAttribute('data-second-name')!==raw||Object.keys(data).join(',')!=='firstName,secondName')throw 'defineProperty named setter';
"#);}

#[test]
fn dataset_native_writes_observers_receive_actual_owner_names_and_raw_old_values(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var node=document.querySelector('p'),data=node.dataset,watch=new MutationObserver(function(){}),filtered=new MutationObserver(function(){});watch.observe(node,{attributes:true,attributeOldValue:true});filtered.observe(document.body,{attributes:true,subtree:true,attributeFilter:['data-first-name'],attributeOldValue:true});var raw='x'+String.fromCharCode(55296);data.firstName=raw;data.firstName=raw;data.secondName='second';Object.defineProperty(data,'firstName',{value:'last'});var r=watch.takeRecords(),f=filtered.takeRecords();if(r.length!==4||f.length!==3)throw 'all native dataset writes observed';for(var item of r){if(item.type!=='attributes'||item.target!==node||item.attributeNamespace!==null)throw 'real Element observer target';}if(r[0].attributeName!=='data-first-name'||r[0].oldValue!=='old'||r[1].oldValue!==raw||r[2].attributeName!=='data-second-name'||r[2].oldValue!==null||r[3].oldValue!==raw||f[2].oldValue!==raw)throw 'native names and exact previous values';
"#);}

#[test]
fn dataset_native_writes_delete_uses_supported_key_and_native_removal_records(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var node=document.querySelector('p'),data=node.dataset,attr=node.getAttributeNode('data-first-name'),watch=new MutationObserver(function(){});node.setAttribute('data-x--y','keep');watch.observe(node,{attributes:true,attributeOldValue:true});if(!delete data['x-y']||data['x-Y']!=='keep'||watch.takeRecords().length)throw 'unsupported name must not alias';if(!delete data.firstName||attr.ownerElement!==null||node.hasAttribute('data-first-name')||Object.hasOwn(data,'firstName'))throw 'actual native removal';var r=watch.takeRecords();if(r.length!==1||r[0].target!==node||r[0].attributeName!=='data-first-name'||r[0].oldValue!=='old')throw 'native removal record';delete data.firstName;delete data.missing;if(watch.takeRecords().length)throw 'absent delete has no mutation';data.firstName='new';if(Object.keys(data).join(',')!=='x-Y,firstName')throw 'live reinsert order';
"#);}

#[test]
fn dataset_native_writes_refusal_preserves_attribute_and_pending_records_then_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var node=document.querySelector('p'),data=node.dataset,watch=new MutationObserver(function(){});watch.observe(node,{attributes:true,attributeOldValue:true});");
 let limit=p.state.retained_limit;let commits=p.state.dom_commits;p.state.retained_limit=p.state.retained;
 for (source,work_refusal) in [("data.firstName='refused';",false),("delete data.firstName;",true)]{
  p.begin_job();if work_refusal{p.state.retained_limit=limit;p.state.copy_work=0;}
  let error=p.runtime.execute(source,&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains(if work_refusal{"string work"}else{"retained"}),"{error}");assert_eq!(p.state.dom_commits,commits);
  run(&mut d,&mut f,&mut p,"if(data.firstName!=='old'||watch.takeRecords().length)throw 'refusal must preserve native attribute and pending records';");
 }p.state.retained_limit=limit;
 run(&mut d,&mut f,&mut p,"data.firstName='recovered';delete data.firstName;var r=watch.takeRecords();if(r.length!==2||r[0].oldValue!=='old'||r[1].oldValue!=='recovered'||node.hasAttribute('data-first-name'))throw 'actual write delete recovery';");
}

#[test]
fn dataset_native_writes_exact_ascii_expansion_and_raw_delete_ignore_method_shadow(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
 var node=document.querySelector('p'),data=node.dataset;for(var pair of [['','data-'],['ABC','data--a-b-c'],['x-Y','data-x--y'],['x-9','data-x-9'],['a.b:c','data-a.b:c']]){data[pair[0]]='mapped';if(node.getAttribute(pair[1])!=='mapped'||data[pair[0]]!=='mapped')throw 'exact ASCII mapping';}
 var raw='raw'+String.fromCharCode(55296);data.firstName=raw;var attr=node.getAttributeNode('data-first-name'),watch=new MutationObserver(function(){});watch.observe(node,{attributes:true,attributeOldValue:true});node.removeAttribute=function(){throw 'author shadow';};delete data.firstName;var r=watch.takeRecords();if(r.length!==1||r[0].oldValue!==raw||r[0].target!==node||attr.ownerElement!==null||data.firstName!==undefined)throw 'actual raw native deletion ignores author method shadow';
 "#);
}
