//! Author Event chains preserve concrete native interfaces and dispatch state.
use super::*;
fn fixture()->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><body><input id=field><button id=button type=button>Run</button></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(&crate::navigation::address("https://event-prototype.test/source",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());
}
#[test]
fn event_author_prototype_concrete_families_keep_native_brand_trust_dispatch_and_overrides() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var button=document.getElementById('button'),calls=0,getType=Object.getOwnPropertyDescriptor(Event.prototype,'type').get,getTarget=Object.getOwnPropertyDescriptor(Event.prototype,'target').get;
button.addEventListener('family',function(e){calls++;if(e.alias!=='family'||e.isTrusted||e.currentTarget!==button||getTarget.call(e)!==button)throw 'real author dispatch';e.preventDefault();});
var cases=[[new Event('family',{cancelable:true}),Event],[new UIEvent('family',{cancelable:true,detail:7,view:window}),UIEvent],[new CustomEvent('family',{cancelable:true,detail:17}),CustomEvent],[new MouseEvent('family',{cancelable:true,clientX:23}),MouseEvent],[new WheelEvent('family',{cancelable:true,deltaY:3}),WheelEvent],[new KeyboardEvent('family',{cancelable:true,key:'A'}),KeyboardEvent],[new FocusEvent('family',{cancelable:true,relatedTarget:button}),FocusEvent],[new ProgressEvent('family',{cancelable:true,loaded:2,total:3}),ProgressEvent],[new AnimationEvent('family',{cancelable:true,animationName:'original'}),AnimationEvent],[new ErrorEvent('family',{cancelable:true,message:'original'}),ErrorEvent]];
for(var pair of cases){var e=pair[0],C=pair[1],native=Object.getPrototypeOf(e),P=Object.create(native,{alias:{get:function(){return this.type;}}});e.state={node:e};e.__proto__=P;if(Object.getPrototypeOf(e)!==P||e.__proto__!==P||!(e instanceof C)||!(e instanceof Event)||e.alias!=='family'||e.state.node!==e||getType.call(e)!=='family')throw 'concrete author chain';var trust=Object.getOwnPropertyDescriptor(e,'isTrusted');if(!trust||trust.configurable||trust.set!==undefined||e.isTrusted)throw 'real unforgeable trust';if(e instanceof MouseEvent){var refused=false;try{button.dispatchEvent(e);}catch(error){refused=error.name==='NotSupportedError';}if(!refused||getTarget.call(e)!==null||e.defaultPrevented)throw 'real mouse activation refusal';e.preventDefault();}else if(button.dispatchEvent(e)!==false||!e.defaultPrevented||e.currentTarget!==null||e.eventPhase!==0||e.composedPath().length)throw 'native cleanup';Object.defineProperty(P,'target',{configurable:true,get:function(){return 'author target';}});if(e.target!=='author target'||getTarget.call(e)!==(e instanceof MouseEvent?null:button))throw 'ordinary retargeting versus native fact';Object.setPrototypeOf(e,native);if(e.alias!==undefined||e.target!==(e instanceof MouseEvent?null:button)||e.state.node!==e||!(e instanceof C))throw 'restore native chain';}
if(calls!==cases.length-2||cases[1][0].detail!==7||cases[1][0].view!==window||cases[2][0].detail!==17||cases[3][0].clientX!==23||cases[4][0].deltaY!==3||cases[5][0].key!=='A'||cases[6][0].relatedTarget!==button||cases[7][0].loaded!==2||cases[8][0].animationName!=='original'||cases[9][0].message!=='original')throw 'real subclass slots';
"#);
}
#[test]
fn event_author_prototype_invalid_interface_and_proxy_chains_preserve_native_state() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var e=new FocusEvent('native',{relatedTarget:document.getElementById('button')}),P=Object.create(FocusEvent.prototype),traps=0,proxy=new Proxy(P,{getPrototypeOf:function(){traps++;return FocusEvent.prototype;}}),refused=0;Object.setPrototypeOf(e,P);
for(var invalid of [null,Object.prototype,Event.prototype,UIEvent.prototype,MouseEvent.prototype,proxy,Object.create(proxy)]){try{Object.setPrototypeOf(e,invalid);}catch(error){if(error instanceof TypeError)refused++;else throw error;}}
if(refused!==7||traps||Object.getPrototypeOf(e)!==P||e.type!=='native'||e.relatedTarget!==document.getElementById('button')||e.isTrusted)throw 'concrete native anchor guard';var bad=0;try{Object.defineProperty(e,'isTrusted',{value:true});}catch(error){if(error instanceof TypeError)bad++;else throw error;}if(bad!==1||e.isTrusted)throw 'author chain forged trust';var getter=Object.getOwnPropertyDescriptor(FocusEvent.prototype,'relatedTarget').get;try{getter.call(new Event('wrong'));}catch(error){if(error instanceof TypeError)bad++;else throw error;}if(bad!==2)throw 'native subclass brand';
"#);
}
#[test]
fn event_author_prototype_foreign_saved_operations_validate_before_author_getters() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var e=new FocusEvent('native'),hits=0,state={},P=Object.create(FocusEvent.prototype,{binding:{get:function(){hits++;return state;}}});Object.setPrototypeOf(e,P);var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,read=w.Function('e','return e.binding;'),set=w.Function('e','p','return Object.setPrototypeOf(e,p);'),inspect=w.Function('e','return Object.getPrototypeOf(e);');if(read(e)!==state||hits!==1||set(e,P)!==e||inspect(e)!==P)throw 'same origin event operations';
"#);
    page.set_location(&crate::navigation::address("https://other-event-prototype.test/",&crate::navigation::Location::home()).unwrap());
    for source in ["read(e);","set(e,P);","inspect(e);"] {
        assert_eq!(page.console_eval(&mut doc,&mut forms,source).errors,1,"{source}: {}",page.report());
        assert!(page.report().contains("origin")||page.report().contains("Security"));
    }
    run(&mut doc,&mut forms,&mut page,"if(hits!==1||Object.getPrototypeOf(e)!==P||e.type!=='native'||e.isTrusted)throw 'denied foreign event effects';");
}
