//! Original AnimationEvent construction, coercion, owner and storage checks.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<html><body><div id=outer><div id=target>Original</div></div></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://animation-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn animation_event_genuine_constructor_defaults_fields_brand_and_saved_roots() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var A=AnimationEvent,e=new A('animationend',{animationName:'appear',elapsedTime:1.25,pseudoElement:'::before',bubbles:true,cancelable:true,composed:true});
if(Object.getPrototypeOf(A)!==Event||Object.getPrototypeOf(A.prototype)!==Event.prototype||Object.getPrototypeOf(e)!==A.prototype||e.constructor!==A||!(e instanceof Event)||!(e instanceof A)||e instanceof UIEvent||Object.prototype.toString.call(e)!=='[object AnimationEvent]')throw 'genuine native ancestry';
if(e.animationName!=='appear'||e.elapsedTime!==1.25||e.pseudoElement!=='::before'||!e.bubbles||!e.cancelable||!e.composed||e.isTrusted)throw 'genuine retained fields';
var d=new A('animationstart');if(d.animationName!==''||d.elapsedTime!==0||d.pseudoElement!==''||d.bubbles||d.cancelable||d.composed||d.isTrusted)throw 'real defaults';
var get=Object.getOwnPropertyDescriptor(A.prototype,'animationName').get,pd=Object.getOwnPropertyDescriptor(A,'prototype'),gd=Object.getOwnPropertyDescriptor(window,'AnimationEvent');
if(pd.writable||pd.configurable||pd.enumerable||!gd.writable||!gd.configurable||gd.enumerable||A.length!==1||get.name!=='get animationName'||get.length!==0)throw 'canonical descriptors';
var field=Object.getOwnPropertyDescriptor(A.prototype,'elapsedTime');if(field.set!==undefined||!field.enumerable||!field.configurable)throw 'readonly field descriptor';
var readonly=false;try{(function(){'use strict';e.elapsedTime=9;})();}catch(x){readonly=x instanceof TypeError;}if(!readonly||e.elapsedTime!==1.25)throw 'readonly actual field';
var bad=0;for(var f of [function(){A('bad');},function(){new A();},function(){get.call({});},function(){get.call(new Event('bad'));},function(){new A('bad',3);},function(){document.createEvent('AnimationEvent');}]){try{f();}catch(x){bad++;}}if(bad!==6)throw 'constructor/brand/factory refusal';
var unpaired=new A('units',{animationName:'\ud800',pseudoElement:'\udc00',elapsedTime:-0});
if(unpaired.animationName.charCodeAt(0)!==0xd800||unpaired.pseudoElement.charCodeAt(0)!==0xdc00||!Object.is(unpaired.elapsedTime,-0))throw 'exact DOMString and double';
A.prototype.extra=17;e.extra=23;if(e.extra!==23||!delete e.extra||e.extra!==17)throw 'ordinary inherited own state';
var descriptor=Object.getOwnPropertyDescriptor(A.prototype,'animationName');delete A.prototype.animationName;if(e.animationName!==undefined)throw 'deleted getter resurrected';Object.defineProperty(A.prototype,'animationName',descriptor);
delete window.AnimationEvent;if(typeof AnimationEvent!=='undefined'||!(e instanceof A)||get.call(e)!=='appear')throw 'saved native roots';window.AnimationEvent=17;if(!(d instanceof A))throw 'global replacement';
"#,
    );
}
#[test]
fn animation_event_webidl_dictionary_order_finite_double_and_conversion_failures() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var order='',init={get bubbles(){order+='b';return true;},get cancelable(){order+='c';return false;},get composed(){order+='o';return true;},get animationName(){order+='a';return {toString:function(){order+='A';return 'name';}};},get elapsedTime(){order+='e';return {valueOf:function(){order+='E';return -2.5;}};},get pseudoElement(){order+='p';return {toString:function(){order+='P';return '::after';}};}};
var e=new AnimationEvent({toString:function(){order+='T';return 'end';}},init);
if(order!=='TbcoaAeEpP'||e.animationName!=='name'||e.elapsedTime!==-2.5||e.pseudoElement!=='::after')throw 'ordered WebIDL fields';
var later=0,bad=0;for(var v of [NaN,Infinity,-Infinity,Symbol('s'),BigInt(1)]){try{new AnimationEvent('bad',{elapsedTime:v,get pseudoElement(){later++;return 'late';}});}catch(x){if(!(x instanceof TypeError))throw x;bad++;}}if(bad!==5||later!==0)throw 'finite double rejects before next member';
var marker={},caught=false;try{new AnimationEvent('bad',{animationName:{toString:function(){throw marker;}}});}catch(x){caught=x===marker;}if(!caught)throw 'conversion exception identity';
"#,
    );
}
#[test]
fn animation_event_synthetic_dispatch_uses_real_event_target_and_base_state() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var target=document.getElementById('target'),outer=document.getElementById('outer'),order='';
outer.addEventListener('animationend',function(e){if(e.target!==target||e.currentTarget!==outer||e.eventPhase!==3||!(e instanceof AnimationEvent)||e.animationName!=='appear'||e.elapsedTime!==2||e.isTrusted)throw 'native synthetic event plan';order+='outer';});
target.addEventListener('animationend',function(e){if(e.currentTarget!==target||e.eventPhase!==2||e.pseudoElement!=='::before')throw 'native target phase';e.preventDefault();order+='target;';});
var e=new AnimationEvent('animationend',{animationName:'appear',elapsedTime:2,pseudoElement:'::before',bubbles:true,cancelable:true});
if(target.dispatchEvent(e)!==false||order!=='target;outer'||!e.defaultPrevented||e.currentTarget!==null||e.eventPhase!==0||e.target!==target||e.isTrusted)throw 'real cancellation/expiry';
e.initEvent('reset',false,false);if(e.type!=='reset'||e.defaultPrevented||e.animationName!=='appear'||e.elapsedTime!==2||e.pseudoElement!=='::before')throw 'base reinitialization changed subclass fields';
"#,
    );
}
#[test]
fn animation_event_child_owner_and_saved_native_access_obey_origin_guards() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,A=w.AnimationEvent;
var e=new A('child',{animationName:'child-name'}),get=Object.getOwnPropertyDescriptor(A.prototype,'animationName').get;
if(A===AnimationEvent||e instanceof AnimationEvent||!(e instanceof A)||!(e instanceof w.Event)||Object.getPrototypeOf(A.prototype)!==w.Event.prototype||get.call(e)!=='child-name')throw 'native child owner';
window.AnimationEvent=17;if(!(e instanceof A)||get.call(e)!=='child-name')throw 'saved child roots';
"#,
    );
    page.set_location(
        &crate::navigation::address(
            "https://foreign-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    let out = page.console_eval(&mut doc, &mut forms, "get.call(e);");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert!(
        page.report().contains("origin") || page.report().contains("Security"),
        "{}",
        page.report()
    );
}
#[test]
fn animation_event_payload_and_base_backing_refusal_leave_no_partial_record_and_recover() {
    let (mut doc, mut forms, mut page) = fixture();
    let before = page.state.retained;
    let count = page.state.created_events.len();
    let old_limit = page.state.retained_limit;
    let old_copies = page.state.copy_work;
    let fields = [
        ("animationName", Value::String("name".into())),
        ("pseudoElement", Value::String("::before".into())),
        ("elapsedTime", Value::Number(1.0)),
    ];
    for allowance in [0, 12] {
        page.state.retained_limit = before + allowance;
        let result = Bridge {
            doc: &mut doc,
            forms: &mut forms,
            state: &mut page.state,
        }
        .construct_animation_event(
            WINDOW,
            &[Value::String("animationend".into())],
            Some(&fields),
        );
        assert!(result.is_err());
        assert_eq!(page.state.retained, before);
        assert_eq!(page.state.created_events.len(), count);
    }
    page.state.retained_limit = old_limit;
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .construct_animation_event(
        WINDOW,
        &[Value::String("animationend".into())],
        Some(&fields)
    )
    .is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(page.state.created_events.len(), count);
    page.state.copy_work = old_copies;
    run(&mut doc,&mut forms,&mut page,"var recovered=new AnimationEvent('animationend',{animationName:'name',elapsedTime:1,pseudoElement:'::before'});if(recovered.animationName!=='name'||recovered.elapsedTime!==1||recovered.pseudoElement!=='::before')throw 'native recovery';");
}
