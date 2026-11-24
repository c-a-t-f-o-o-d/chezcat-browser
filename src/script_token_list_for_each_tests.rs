//! Canonical DOMTokenList toggle behavior and native boundaries.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target class='one two one'></p><iframe id=frame sandbox='allow-scripts'></iframe></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn token_list_for_each_canonical_array_alias_descriptors_deletion_and_generic_receiver(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var P=DOMTokenList.prototype,d=Object.getOwnPropertyDescriptor(P,'forEach'),list=document.getElementById('target').classList;
if(!d||d.value!==Array.prototype.forEach||d.value.name!=='forEach'||d.value.length!==1||!d.writable||!d.enumerable||!d.configurable||Object.prototype.hasOwnProperty.call(list,'forEach'))throw 'canonical alias';
var each=d.value;delete P.forEach;if(list.forEach!==undefined||'forEach' in list)throw 'deleted alias resurrected';var seen='';each.call(list,function(v){seen+=v+'|';});if(seen!=='one|two|')throw 'saved native iteration';
each.call({0:'ordinary',length:1},function(v,i,o){if(v!=='ordinary'||i!==0||o.length!==1)throw 'generic array receiver';});
P.forEach=function(){return 'shadow';};if(list.forEach()!=='shadow')throw 'prototype shadow';list.forEach=17;if(list.forEach!==17)throw 'own shadow';delete list.forEach;P.forEach=each;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);if(frame.contentWindow.DOMTokenList.prototype.forEach!==frame.contentWindow.Array.prototype.forEach||frame.contentWindow.DOMTokenList.prototype.forEach===each)throw 'owner intrinsic identity';
"#);}
#[test]
fn token_list_for_each_raw_callback_identity_this_and_captured_length_with_live_reads(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList,receiver={},seen=[];p.className='A\ud800z A�z A\ud800z';
if(list.forEach(function(v,i,s){'use strict';if(this!==receiver||s!==list)throw 'callback receiver';seen.push(v);},receiver)!==undefined||seen.length!==2||seen[0]!=='A\ud800z'||seen[1]!=='A�z')throw 'exact callback values';
p.className='a b c';seen=[];list.forEach(function(v,i){seen.push(v);if(i===0)p.className='u v extra tail';});if(seen.join('|')!=='a|v|extra')throw 'captured length live growth';
p.className='a b c';seen=[];list.forEach(function(v,i){seen.push(v);if(i===0)p.className='u';});if(seen.join('|')!=='a')throw 'live shrink holes';
"#);}
#[test]
fn token_list_for_each_inherited_holes_abrupt_identity_and_reentrant_calls(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList,seen=[],marker={},gets=0;p.className='a b';
Object.defineProperty(DOMTokenList.prototype,'1',{get:function(){gets++;return 'fallback';},configurable:true});list.forEach(function(v,i){seen.push(v);if(i===0)p.className='u';});delete DOMTokenList.prototype[1];if(seen.join('|')!=='a|fallback'||gets!==1)throw 'HasProperty and inherited Get';
p.className='A\ud800z next';seen=[];try{list.forEach(function(v){seen.push(v);p.className='effect';throw marker;});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(seen.length!==1||seen[0]!=='A\ud800z'||p.className!=='effect')throw 'abrupt author effects';
p.className='a b';var nested='';list.forEach(function(v,i){if(i===0)list.forEach(function(x){nested+=x;});});if(nested!=='ab')throw 'nested callback iteration';
"#);}
#[test]
fn token_list_for_each_native_admission_refusal_keeps_record_and_fresh_job_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target'),calls=0;p.className='A\\ud800z next';");
    let before=format!("{:?}",doc.nodes);let retained=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;
    for mode in 0..3 {
        page.begin_job();if mode==0{page.state.copy_work=0;}else if mode==1{page.state.host_work=0;}else{page.state.retained_limit=retained;}
        let error=page.runtime.execute("p.classList.forEach(function(){calls++;})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work limit")||error.contains("allowance"),"{error}");
        assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);page.state.retained_limit=limit;
    }
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0)throw 'callback before admission';var seen=[];p.classList.forEach(function(v){seen.push(v);});if(seen.length!==2||seen[0]!=='A\\ud800z'||seen[1]!=='next')throw 'fresh callback recovery';");
}

#[test]
fn token_list_for_each_shared_array_path_accepts_real_indexed_collections_and_checks_callback(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),sources=[document.querySelectorAll('#target'),document.body.children,p.attributes];
for(var s of sources){var count=0;Array.prototype.forEach.call(s,function(value,index,source){if(source!==s||index!==count||value!==s[index])throw 'shared indexed callback';count++;});if(count!==s.length)throw 'shared native count';}
var empty=document.createElement('p').classList;try{empty.forEach({});throw 'invalid callback skipped for empty';}catch(e){if(!(e instanceof TypeError))throw e;}
try{Array.prototype.forEach.call(null,function(){});throw 'null receiver';}catch(e){if(!(e instanceof TypeError))throw e;}
"#);}
#[test]
fn token_list_for_each_actual_child_script_caller_is_rechecked_before_callback(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),list=p.classList,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var childEach=frame.contentWindow.Function('list','callback','return Array.prototype.forEach.call(list,callback);'),calls=0;
childEach(list,function(){calls++;});if(calls!==2)throw 'same-origin child Script caller';calls=0;
"#);
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    page.set_location(&crate::navigation::address("https://foreign-callback.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("childEach(list,function(){calls++;})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://token-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0)throw 'callback before caller validation';childEach(list,function(){calls++;});if(calls!==2)throw 'fresh same-origin callback recovery';");
}
#[test]
fn token_list_for_each_cumulative_step_refusal_keeps_author_effects_and_fresh_job_recovers(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target'),calls=0,words=[];for(var i=0;i<100;i++)words.push('token'+i);p.className=words.join(' ');function consume(){calls++;}");
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    page.begin_job();page.runtime.constrain_job(300,usize::MAX,usize::MAX,std::time::Instant::now()+std::time::Duration::from_secs(30));
    let error=page.runtime.execute("p.classList.forEach(consume)",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("step")||error.contains("execution limit"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls<=0||calls>=100)throw 'iteration reset callback allowance or erased progress';var prior=calls;p.classList.forEach(consume);if(calls!==prior+100)throw 'fresh cumulative callback recovery';");
}
