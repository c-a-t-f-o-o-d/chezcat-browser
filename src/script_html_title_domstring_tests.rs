//! Exact HTML title reflection over the real native attribute record.
use super::*;
fn fixture()->(Document,Forms,Page){
    let mut doc=crate::html::parse("<!doctype html><html><body><p id=target title=seed></p></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();let location=crate::navigation::address("https://title-native.test/source",&crate::navigation::Location::home()).unwrap();page.set_location(&location);page.set_resource_base(&location);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap();}
#[test]
fn html_title_domstring_public_producers_keep_exact_attribute_and_clone_identity(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),a=p.getAttributeNode('title'),raw='A\ud800z😺';p.setAttribute('title',raw);if(p.title!==raw||a.value!==raw||p.getAttributeNode('title')!==a)throw 'attribute exact reflection';
p.title='B\udfffz';if(p.getAttribute('title')!=='B\udfffz'||a.value!==p.title)throw 'reflection exact record';a.value=raw;if(p.title!==raw)throw 'Attr producer';
p.setAttributeNS(null,'title','C\ud800z');if(p.title!=='C\ud800z'||p.getAttributeNode('title')!==a)throw 'namespace producer';
var copy=p.cloneNode(false);if(copy.title!=='C\ud800z'||copy.getAttribute('title')!==copy.title||copy===p)throw 'clone exact record';p.title='A�z';if(p.title!=='A�z'||p.title==='A\ud800z')throw 'replacement distinction';
"#);}
#[test]
fn html_title_domstring_canonical_setter_once_conversion_observer_old_values_and_abrupt(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var p=document.getElementById('target'),d=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'title'),calls=0,raw='A\ud800z',observer=new MutationObserver(function(){throw 'not delivered';});observer.observe(p,{attributes:true,attributeOldValue:true});
d.set.call(p,{toString:function(){calls++;return raw;}});if(calls!==1||d.get.call(p)!==raw)throw 'once canonical conversion';p.title='B\udfffz';var records=observer.takeRecords();if(records.length!==2||records[0].oldValue!=='seed'||records[1].oldValue!==raw||records[1].target!==p||records[1].attributeName!=='title')throw 'exact old value';
var marker={};try{p.title={toString:function(){p.setAttribute('data-effect','yes');throw marker;}};throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}if(p.title!=='B\udfffz'||p.getAttribute('data-effect')!=='yes')throw 'conversion effects';
try{d.set.call({}, {toString:function(){calls++;return 'bad';}});throw 'missing brand';}catch(e){if(!(e instanceof TypeError))throw e;}try{p.title=Symbol();throw 'symbol accepted';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!==1||p.title!=='B\udfffz')throw 'brand or symbol mutation';
p.title=null;if(p.title!=='null')throw 'null DOMString';p.title=undefined;if(p.title!=='undefined')throw 'undefined DOMString';
"#);}
#[test]
fn html_title_domstring_inert_created_and_parsed_elements_keep_exact_reflection(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var inert=document.implementation.createHTMLDocument('inert'),p=inert.createElement('p'),raw='A\ud800z';inert.body.appendChild(p);p.setAttribute('title',raw);var a=p.getAttributeNode('title');if(p.title!==raw)throw 'inert attribute read';p.title='B\udfffz';if(a.value!==p.title||p.title!=='B\udfffz')throw 'inert reflection write';
var parsedOwner=new DOMParser().parseFromString('<html><body><p title="seed"></p></body></html>','text/html'),parsed=parsedOwner.body.firstChild;parsed.title=raw;if(parsed.title!==raw||parsed.getAttribute('title')!==raw)throw 'parsed inert title';
"#);}
#[test]
fn html_title_domstring_saved_child_accessor_origin_before_conversion_and_recovery(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var d=Object.getOwnPropertyDescriptor(frame.contentWindow.HTMLElement.prototype,'title'),calls=0;d.set.call(p,{toString:function(){calls++;return 'A\\ud800z';}});if(calls!==1||d.get.call(p)!=='A\\ud800z')throw 'same origin exact';calls=0;");
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;page.set_location(&crate::navigation::address("https://foreign-title.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();let error=page.runtime.execute("d.set.call(p,{toString:function(){calls++;return 'bad';}})",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);
    page.set_location(&crate::navigation::address("https://title-native.test/source",&crate::navigation::Location::home()).unwrap());page.begin_job();run(&mut doc,&mut forms,&mut page,"if(calls!==0||p.title!=='A\\ud800z')throw 'origin guard order';p.title='fresh';if(p.title!=='fresh')throw 'fresh recovery';");
}
#[test]
fn html_title_domstring_native_refusal_is_atomic_and_fresh_jobs_recover(){
    let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,"var p=document.getElementById('target'),a=p.getAttributeNode('title');p.title='A\\ud800z';");
    let before=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;let retained=page.state.retained;let limit=page.state.retained_limit;let strings=page.state.limits.host.string_bytes;
    for mode in 0..4 {
        page.begin_job();match mode{0=>page.state.copy_work=0,1=>page.state.host_work=0,2=>page.state.retained_limit=retained,_=>page.state.limits.host.string_bytes=3};
        let error=page.runtime.execute("p.title='long B\\udfffz'",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("work")||error.contains("allowance")||error.contains("bound"),"{error}");assert_eq!(format!("{:?}",doc.nodes),before);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);page.state.retained_limit=limit;page.state.limits.host.string_bytes=strings;
    }
    page.begin_job();run(&mut doc,&mut forms,&mut page,"if(p.title!=='A\\ud800z'||a.value!==p.title)throw 'refusal changed record';p.title='fresh B\\udfffz';if(a.value!==p.title)throw 'fresh raw record';");
}
