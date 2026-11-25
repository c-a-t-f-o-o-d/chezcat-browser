//! Original SubmitEvent constructor and native form-dispatch witnesses.
use super::*;
fn fixture(source:&str)->(Document,Forms,Page){
 let mut d=crate::html::parse("<html><body><form id=owner><input id=edit name=q value=Seed><button id=send name=go value=chosen>Send</button></form><div id=plain></div><svg id=svg></svg></body></html>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://submit-event.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);let outcome=p.start(&mut d,&mut f,&[Script{node:0,label:"Original SubmitEvent witness".into(),source:source.into()}]);assert_eq!(outcome.errors,0,"{}",p.report());(d,f,p)
}
#[test]
fn submit_event_canonical_constructor_dictionary_and_readonly_submitter(){
 fixture("var C=SubmitEvent,P=C.prototype,button=document.getElementById('send');if(typeof C!=='function'||C.name!=='SubmitEvent'||C.length!==1||Object.getPrototypeOf(C)!==Event||Object.getPrototypeOf(P)!==Event.prototype)throw 'canonical roots';var event=new C('submit',{bubbles:true,cancelable:true,composed:true,submitter:button});if(!(event instanceof C)||!(event instanceof Event)||event.submitter!==button||event.type!=='submit'||!event.bubbles||!event.cancelable||!event.composed||event.isTrusted)throw 'constructor metadata';var descriptor=Object.getOwnPropertyDescriptor(P,'submitter');if(typeof descriptor.get!=='function'||descriptor.set!==undefined||!descriptor.enumerable||!descriptor.configurable||Object.prototype.hasOwnProperty.call(event,'submitter'))throw 'prototype getter';event.submitter=null;if(event.submitter!==button)throw 'readonly';var strict=false;try{(function(){'use strict';event.submitter=null;})();}catch(e){strict=e.name==='TypeError';}if(!strict)throw 'strict readonly';for(var init of [undefined,null,{}, {submitter:null},{submitter:undefined}])if(new C('default',init).submitter!==null)throw 'default null';delete window.SubmitEvent;if(new C('saved',{submitter:button}).submitter!==button)throw 'saved constructor';");
}
#[test]
fn submit_event_native_form_dispatch_has_genuine_interface_and_null_submitter(){
 let(mut d,mut f,mut p)=fixture("var calls=0,form=document.getElementById('owner');form.onsubmit=function(e){if(!(e instanceof SubmitEvent)||!(e instanceof Event)||e.submitter!==null||!e.isTrusted||!e.bubbles||!e.cancelable||e.composed||e.target!==form)throw 'native SubmitEvent';calls++;e.preventDefault();};");
 let owner=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("owner")).unwrap();p.begin_job();let outcome=p.dispatch(&mut d,&mut f,owner,"submit",true);assert!(outcome.prevented);assert_eq!(outcome.errors,0,"{}",p.report());let outcome=p.console_eval(&mut d,&mut f,"if(calls!==1)throw 'native callback';");assert_eq!(outcome.errors,0,"{}",p.report());
}
#[test]
fn submit_event_dictionary_order_genuine_html_brands_and_dispatch(){
 fixture("var order='',button=document.getElementById('send'),init={get bubbles(){order+='b';return true;},get cancelable(){order+='c';return true;},get composed(){order+='o';return false;},get submitter(){order+='s';return button;}},type={toString:function(){order+='t';return 'probe';}},event=new SubmitEvent(type,init);if(order!=='tbcos'||event.submitter!==button)throw 'conversion order '+order;var get=Object.getOwnPropertyDescriptor(SubmitEvent.prototype,'submitter').get;for(var value of [window,document,document.createTextNode('x'),document.getElementById('svg'),{},1,'button']){var denied=false;try{new SubmitEvent('x',{submitter:value});}catch(e){denied=e.name==='TypeError';}if(!denied)throw 'HTMLElement brand';}for(var value of [button,document.getElementById('plain'),document.getElementById('owner')])if(new SubmitEvent('x',{submitter:value}).submitter!==value)throw 'any HTMLElement';for(var value of [new Event('x'),{},button]){var denied=false;try{get.call(value);}catch(e){denied=e.name==='TypeError';}if(!denied)throw 'getter brand';}var form=document.getElementById('owner'),calls=0;form.addEventListener('probe',function(e){if(e!==event||e.submitter!==button||e.isTrusted)throw 'synthetic identity';calls++;e.preventDefault();});if(form.dispatchEvent(event)!==false||calls!==1||event.target!==form||event.submitter!==button)throw 'real dispatch';event.initEvent('renamed');if(event.submitter!==button||event.type!=='renamed')throw 'base init preserves submitter';");
}
#[test]
fn submit_event_inert_and_child_realm_preserve_real_submitter_identity(){
 fixture("var inert=document.implementation.createHTMLDocument('inert'),element=inert.createElement('button'),e=new SubmitEvent('x',{submitter:element});if(e.submitter!==element||element.ownerDocument!==inert)throw 'inert identity';var iframe=document.createElement('iframe');iframe.hidden=true;document.body.appendChild(iframe);var w=iframe.contentWindow,C=w.SubmitEvent,target=w.document.body,event=new C('x',{submitter:target});if(C===SubmitEvent||Object.getPrototypeOf(C)!==w.Event||Object.getPrototypeOf(C.prototype)!==w.Event.prototype||Object.getPrototypeOf(event)!==C.prototype||event.submitter!==target||event instanceof SubmitEvent||!(event instanceof w.Event))throw 'child realm';if(new SubmitEvent('x',{submitter:target}).submitter!==target)throw 'same origin target';");
}
#[test]
fn submit_event_checked_constructor_refusal_preserves_records_and_recovers(){
 for mode in 0..4{
  let(mut d,mut f,mut p)=fixture("");p.begin_job();let retained=p.state.retained;let limit=p.state.retained_limit;let next=p.state.next_event;let count=p.state.created_events.len();
  match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=retained,_=>p.state.execution_deadline=Some(Instant::now())};
  assert!(Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.construct_submit_event(WINDOW,&[Value::String("submission".into())],None).is_err(),"mode {mode}");
  assert_eq!(p.state.retained,retained);assert_eq!(p.state.created_events.len(),count);assert_eq!(p.state.next_event,next);
  p.state.retained_limit=limit;p.state.execution_deadline=None;p.begin_job();let outcome=p.console_eval(&mut d,&mut f,"if(new SubmitEvent('fresh').submitter!==null)throw 'recovery';");assert_eq!(outcome.errors,0,"{}",p.report());
 }
}
#[test]
fn submit_event_native_chosen_reference_and_failed_dispatch_cancel_then_recover(){
 let(mut d,mut f,mut p)=fixture("var calls=0,button=document.getElementById('send');document.getElementById('owner').onsubmit=function(e){if(e.submitter!==button||!(e instanceof SubmitEvent)||!e.isTrusted)throw 'chosen native button';calls++;e.preventDefault();};");
 let owner=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("owner")).unwrap();let button=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("send")).unwrap();
 p.begin_job();p.state.copy_work=0;let retained=p.state.retained;let outcome=p.dispatch_submit(&mut d,&mut f,owner,Some(button));assert!(outcome.prevented&&outcome.errors>0);assert!(p.state.event.is_none());assert_eq!(p.state.retained,retained);
 p.begin_job();let outcome=p.dispatch_submit(&mut d,&mut f,owner,Some(button));assert!(outcome.prevented);assert_eq!(outcome.errors,0,"{}",p.report());let outcome=p.console_eval(&mut d,&mut f,"if(calls!==1)throw 'dispatch recovery';");assert_eq!(outcome.errors,0,"{}",p.report());
}
#[test]
fn submit_event_saved_child_authority_refuses_after_origin_change(){
 let(mut d,mut f,mut p)=fixture("var iframe=document.createElement('iframe');iframe.hidden=true;document.body.appendChild(iframe);var w=iframe.contentWindow,C=w.SubmitEvent,target=w.document.body,event=new SubmitEvent('same',{submitter:target});if(event.submitter!==target)throw 'same origin reference';");
 p.set_location(&crate::navigation::address("https://other-submit.test/",&crate::navigation::Location::home()).unwrap());
 let outcome=p.console_eval(&mut d,&mut f,"var denied=false;try{new SubmitEvent('denied',{submitter:target});}catch(e){denied=e.name==='SecurityError';}if(!denied)throw 'captured target origin';");assert_eq!(outcome.errors,0,"{}",p.report());
 assert!(p.runtime.execute("new C('denied');",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err().contains("Foreign Window access"));
 p.begin_job();let outcome=p.console_eval(&mut d,&mut f,"if(new SubmitEvent('fresh').submitter!==null)throw 'fresh main constructor';");assert_eq!(outcome.errors,0,"{}",p.report());
}
#[test]
fn submit_event_deferred_constructor_preserves_window_descriptors_and_native_identity(){
 let(mut d,mut f,mut p)=fixture("var calls=0;document.getElementById('owner').onsubmit=function(e){var C=e.constructor;if(typeof C!=='function'||C.name!=='SubmitEvent'||Object.getPrototypeOf(e)!==C.prototype||new C('synthetic').submitter!==null||C!==SubmitEvent)throw 'native constructor identity';calls++;e.preventDefault();};");
 let owner=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("owner")).unwrap();p.begin_job();let outcome=p.dispatch_submit(&mut d,&mut f,owner,None);assert!(outcome.prevented);assert_eq!(outcome.errors,0,"{}",p.report());
 let outcome=p.console_eval(&mut d,&mut f,"var descriptor=Object.getOwnPropertyDescriptor(window,'SubmitEvent'),C=SubmitEvent;if(calls!==1||descriptor.value!==C||descriptor.enumerable||!descriptor.writable||!descriptor.configurable||Object.getOwnPropertyNames(window).indexOf('SubmitEvent')<0)throw 'Window descriptor';delete window.SubmitEvent;if('SubmitEvent' in window)throw 'deleted constructor reappeared';window.SubmitEvent=17;if(window.SubmitEvent!==17||new C('saved').constructor!==C)throw 'override or saved identity';");assert_eq!(outcome.errors,0,"{}",p.report());
}
