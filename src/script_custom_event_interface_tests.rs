//! Original genuine CustomEvent Page/Runtime controls. No service or device IO.
use super::*;

fn fixture(source:&str)->(Document,Forms,Page) {
    fixture_with_limits(source,crate::javascript::limits::CAPACITY_REGRESSION)
}
fn fixture_with_limits(source:&str,limits:crate::javascript::limits::Limits)->(Document,Forms,Page){
    let mut doc=crate::html::parse("<html><head></head><body><div id=parent><button id=button type=button>Run</button></div></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::with_limits(limits);
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    let outcome=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original real CustomEvent interface".into(),source:source.into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    (doc,forms,page)
}
fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let outcome=page.console_eval(doc,forms,source);
    assert_eq!(outcome.errors,0,"{source}\n{}",page.report());
}

#[test]
fn custom_event_interface_real_roots_any_detail_brands_and_ordinary_overrides() {
    let(mut doc,mut forms,mut page)=fixture(r#"
var C=CustomEvent,P=C.prototype,payload={number:17},event=new C('real',{bubbles:true,cancelable:true,composed:true,detail:payload});
if(typeof C!=='function'||C.name!=='CustomEvent'||C.length!==1||Object.getPrototypeOf(C)!==Event||Object.getPrototypeOf(P)!==Event.prototype)throw 'real CustomEvent roots';
if(Object.getPrototypeOf(event)!==P||event.constructor!==C||!(event instanceof C)||!(event instanceof Event)||event instanceof UIEvent||Object.prototype.toString.call(event)!=='[object CustomEvent]')throw 'real CustomEvent identity';
if(event.type!=='real'||event.detail!==payload||!event.bubbles||!event.cancelable||!event.composed||event.isTrusted||event.target!==null||event.currentTarget!==null||event.eventPhase!==0)throw 'real event slots';
var desc=Object.getOwnPropertyDescriptor(P,'detail'),init=Object.getOwnPropertyDescriptor(P,'initCustomEvent'),trust=Object.getOwnPropertyDescriptor(event,'isTrusted');
if(typeof desc.get!=='function'||desc.get.length!==0||desc.set!==undefined||!desc.enumerable||!desc.configurable||Object.prototype.hasOwnProperty.call(event,'detail'))throw 'real readonly inherited detail accessor';
if(init.value!==event.initCustomEvent||init.value.length!==1||!init.writable||!init.enumerable||!init.configurable||event.preventDefault!==Event.prototype.preventDefault)throw 'canonical initializer';
if(typeof trust.get!=='function'||!trust.enumerable||trust.configurable||trust.set!==undefined||Object.getOwnPropertyDescriptor(P,'isTrusted')!==undefined)throw 'actual unforgeable trust';
var get=desc.get,coerced=0,type={toString:function(){coerced++;return 'unused';}},bad=0;
for(var value of [{},Object.create(P),new Event('base'),new UIEvent('ui'),new MouseEvent('mouse'),null,undefined,1]){
 try{get.call(value);}catch(e){if(e.name==='TypeError')bad++;else throw e;}
 try{init.value.call(value,type);}catch(e){if(e.name==='TypeError')bad++;else throw e;}
}
try{new init.value(type);}catch(e){if(e.name==='TypeError')bad++;else throw e;}
if(bad!==17||coerced)throw 'genuine brands before coercion and nonconstructor initializer';
event.detail=91;if(event.detail!==payload)throw 'readonly sloppy detail';var strict=0;try{(function(){'use strict';event.detail=91;})();}catch(e){if(e.name==='TypeError')strict++;}if(strict!==1)throw 'readonly strict detail';
Object.defineProperty(event,'detail',{value:'own',configurable:true});if(event.detail!=='own'||get.call(event)!==payload)throw 'ordinary shadow versus actual any slot';delete event.detail;
var original=P.initCustomEvent,calls=0;P.initCustomEvent=function(){calls++;return this;};if(event.initCustomEvent('ignored')!==event||calls!==1||event.type!=='real')throw 'real prototype override';P.initCustomEvent=original;
event.initCustomEvent=function(){return 'own';};if(event.initCustomEvent()!=='own')throw 'own method override';delete event.initCustomEvent;
var values=[payload,function(){return 19;},Symbol('any'),document.body,window,'Original any string',0,false,null,NaN];
for(var value of values){var e=new C('any',{detail:value});if(value!==value?!isNaN(e.detail):e.detail!==value)throw 'actual any identity';}
document.title='real-custom-interface';
"#);
    assert_eq!(doc.title,"real-custom-interface");
    evaluate(&mut page,&mut doc,&mut forms,"if(event.detail!==payload||event.initCustomEvent!==original||values[1]()!==19)throw 'real retained detail and native identity across entries';");
}

#[test]
fn custom_event_interface_dictionary_order_defaults_and_reentrant_throw_are_atomic() {
    let(mut doc,mut forms,mut page)=fixture(r#"
var C=CustomEvent,log='',never=0,detail={toString:function(){never++;throw 'any coerced';},valueOf:function(){never++;throw 'any coerced';}};
var prototype={get bubbles(){log+='B';return true;},get cancelable(){log+='C';return true;},get composed(){log+='O';return true;}},dictionary=Object.create(prototype);
Object.defineProperty(dictionary,'detail',{get:function(){log+='D';return detail;}});
var event=new C({toString:function(){log+='T';return 'ordered';}},dictionary);
if(log!=='TBCOD'||never||event.detail!==detail||event.type!=='ordered'||!event.bubbles||!event.cancelable||!event.composed)throw 'type then inherited EventInit then unconverted any';
for(var init of [undefined,null,{}, {detail:undefined}]){var e=new C('default',init);if(e.detail!==null||e.bubbles||e.cancelable||e.composed||e.isTrusted)throw 'real dictionary defaults';}
if(new C(undefined).type!=='undefined'||new C(null).type!=='null')throw 'required DOMString values';
var refused=0,effects=0,type={toString:function(){effects++;return 'bad';}};
try{C(type,dictionary);}catch(e){if(e.name==='TypeError')refused++;else throw e;}
try{new C();}catch(e){if(e.name==='TypeError')refused++;else throw e;}
try{new C(Symbol(),{get detail(){effects++;return null;}});}catch(e){if(e.name==='TypeError')refused++;else throw e;}
for(var init of [true,false,0,1,'wrong',Symbol()]){try{new C('bad',init);}catch(e){if(e.name==='TypeError')refused++;else throw e;}}
if(refused!==9||effects)throw 'constructor refusal before later getters';
var marker={},nested,tail=0;
"#);
    let count=page.state.created_events.len();let next=page.state.next_event;
    evaluate(&mut page,&mut doc,&mut forms,r#"
var caught=false;
try{new C('outer-unpublished',{get detail(){nested=new C('nested',{detail:{value:23}});throw marker;},get z(){tail++;return 1;}});}catch(e){caught=e===marker;}
if(!caught||tail||nested.type!=='nested'||nested.detail.value!==23||nested.isTrusted)throw 'valid nested effect and abrupt identity';
"#);
    assert_eq!(page.state.created_events.len(),count+1);
    assert_eq!(page.state.next_event,next+1);
    assert!(page.state.created_events.iter().all(|event|event.name!="outer-unpublished"));
}

#[test]
fn custom_event_interface_constructor_and_factory_keep_real_dispatch_and_initializer_state() {
    let(mut doc,mut forms,mut page)=fixture(r#"
var C=CustomEvent,P=C.prototype,button=document.getElementById('button'),parent=document.getElementById('parent'),payload={value:29},event=new C('route',{bubbles:true,cancelable:true,composed:true,detail:payload}),factory=document.createEvent('cUsToMeVeNt'),log='',converted=0;
if(Object.getPrototypeOf(factory)!==P||!(factory instanceof C)||factory.detail!==null||factory.type!==''||factory.isTrusted)throw 'real legacy factory family';
var uninitialized=0;try{button.dispatchEvent(factory);}catch(e){if(e.name==='InvalidStateError')uninitialized++;}if(uninitialized!==1)throw 'factory stays genuinely uninitialized';
if(!button.dispatchEvent(new C('')))throw 'initialized empty constructor event';
parent.addEventListener('route',function(e){if(e!==event||e.detail!==payload||e.eventPhase!==1||e.isTrusted)throw 'real capture';log+='capture;';},true);
button.addEventListener('route',function(e){
 var stamp=e.timeStamp;e.initCustomEvent({toString:function(){converted++;e.preventDefault();payload.value=31;return 'ignored';}},false,false,{value:0});
 if(e.type!=='route'||e.detail!==payload||!e.composed||e.timeStamp!==stamp||!e.defaultPrevented)throw 'inflight init no-op after actual conversions';
 var path=e.composedPath();if(path[0]!==button||path[path.length-1]!==window)throw 'actual path';
 log+='target;';
});
parent.addEventListener('route',function(e){if(e.detail.value!==31||e.eventPhase!==3||!e.defaultPrevented)throw 'real bubble';log+='bubble;';});
if(button.dispatchEvent(event)!==false||log!=='capture;target;bubble;'||converted!==1||event.currentTarget!==null||event.eventPhase!==0||event.composedPath().length)throw 'dispatch cancellation and cleanup';
var stamp=event.timeStamp,second={value:37};P.initCustomEvent.call(event,'reset',false,false,second);
if(event.detail!==second||event.type!=='reset'||event.target!==null||event.defaultPrevented||!event.composed||event.timeStamp!==stamp)throw 'legacy init actual reset preserves timestamp/composed';
Event.prototype.initEvent.call(event,'base-reset');if(event.detail!==second||!event.composed)throw 'base initialization preserves subclass detail';
factory.initCustomEvent('standalone',false,false,payload);var target=new EventTarget(),calls=0;
target.addEventListener('standalone',function(e){if(e!==factory||e.detail!==payload||e.target!==target||this!==target)throw 'real standalone target';calls++;});
if(!target.dispatchEvent(factory)||calls!==1||factory.detail!==payload)throw 'legacy standalone dispatch';
"#);
    assert!(page.state.synthetic_dispatches.is_empty());
    assert!(page.state.created_events.iter().all(|event|!event.dispatching));
    evaluate(&mut page,&mut doc,&mut forms,"if(event.detail!==second||factory.detail!==payload||event.type!=='base-reset')throw 'retained real dispatch state';");
}

#[test]
fn custom_event_interface_main_child_owners_and_any_foreign_references_keep_security_boundaries() {
    let(mut doc,mut forms,mut page)=fixture_with_limits(r#"
var RootC=CustomEvent,RootP=RootC.prototype,RootFunction=Function,main=new RootC('main',{detail:{value:41}});
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,ChildC=w.CustomEvent,ChildP=ChildC.prototype,ChildFunction=w.Function,ChildObject=w.Object;
if(ChildC===RootC||Object.getPrototypeOf(ChildC)!==w.Event||Object.getPrototypeOf(ChildP)!==w.Event.prototype)throw 'genuine child roots';
var childPayload=ChildFunction('return {value:43,run:function(){return 47;}};')(),child=new ChildC('child',{detail:childPayload});
if(Object.getPrototypeOf(child)!==ChildP||!(child instanceof ChildC)||child instanceof RootC||child.detail!==childPayload||Object.getPrototypeOf(child.detail)!==ChildObject.prototype)throw 'actual cross-realm any and owner instance';
var read=ChildFunction('event','return event.initCustomEvent;'),getterRead=ChildFunction('event','return event.detail;'),call=ChildFunction('fn','event','type','return fn.call(event,type);');
var init=read(main),get=Object.getOwnPropertyDescriptor(RootP,'detail').get,childGet=Object.getOwnPropertyDescriptor(ChildP,'detail').get,childInit=ChildP.initCustomEvent;
if(init!==RootP.initCustomEvent||Object.getPrototypeOf(init)!==RootFunction.prototype||getterRead(main)!==main.detail)throw 'first child lookup keeps actual owner Function';
var own=ChildFunction('C','var e=new C("own",{detail:window});return e.detail===window&&e instanceof C&&document.body.nodeType===1;');
ChildFunction('CustomEvent=17;Function=19;')();CustomEvent=23;delete window.CustomEvent;
if(typeof CustomEvent!=='undefined'||Object.getPrototypeOf(new RootC('saved-main'))!==RootP||Object.getPrototypeOf(new ChildC('saved-child'))!==ChildP)throw 'saved canonical roots ignore globals';
var converted=0,type={toString:function(){converted++;return 'forbidden';}},dictionary={get detail(){converted++;return null;}};
"#,crate::javascript::limits::BROWSER);
    let count=page.state.created_events.len();let next=page.state.next_event;
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/parent").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["new w.CustomEvent(type,dictionary);","new ChildC(type,dictionary);","read(main);","getterRead(main);","call(get,main,type);","call(init,main,type);","childGet.call(main);","childInit.call(main,type);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"{source}: {}",page.report());
        assert!(page.report().contains("same-origin"),"{}",page.report());
        assert_eq!(page.state.created_events.len(),count);assert_eq!(page.state.next_event,next);
        evaluate(&mut page,&mut doc,&mut forms,"if(converted||main.type!=='main'||main.detail.value!==41)throw 'foreign operation before coercion or mutation';");
    }
    evaluate(&mut page,&mut doc,&mut forms,"if(!own(ChildC))throw 'real child constructor after foreign failures';var envelope=new RootC('foreign-reference',{detail:w});if(envelope.detail!==w||typeof CustomEvent!=='undefined')throw 'any identity must not invent eager origin filtering';");
    let outcome=page.console_eval(&mut doc,&mut forms,"envelope.detail.document;");assert_eq!(outcome.errors,1,"The stored any reference grants no new native read authority.");
    evaluate(&mut page,&mut doc,&mut forms,"if(main.detail.value!==41||envelope.detail!==w||Object.getPrototypeOf(main)!==RootP)throw 'genuine main recovery';");
}

#[test]
fn custom_event_interface_combined_text_capacity_identity_and_work_refusals_recover() {
    let(mut doc,mut forms,mut page)=fixture("var C=CustomEvent,kept=new C('kept',{detail:'abc'});");
    let count=page.state.created_events.len();let retained=page.state.retained;let next=page.state.next_event;
    let normal_retained=page.state.retained_limit;
    page.state.retained_limit=retained+3;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('N',{detail:'XYZ'});");assert_eq!(outcome.errors,1);
    assert_eq!(page.state.retained,retained);assert_eq!(page.state.created_events.len(),count);assert_eq!(page.state.next_event,next);
    page.state.retained_limit=normal_retained;
    let string_limit=page.state.limits.host.string_bytes;page.state.limits.host.string_bytes=3;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('N',{detail:'four'});");assert_eq!(outcome.errors,1);
    assert_eq!(page.state.retained,retained);assert_eq!(page.state.next_event,next);
    page.state.limits.host.string_bytes=string_limit;
    let type_limit=page.state.limits.host.event_type_bytes;page.state.limits.host.event_type_bytes=1;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('too-long',{detail:null});");assert_eq!(outcome.errors,1);
    assert_eq!(page.state.created_events.len(),count);assert_eq!(page.state.next_event,next);
    page.state.limits.host.event_type_bytes=type_limit;
    let capacity=page.state.limits.host.created_events;page.state.limits.host.created_events=count;
    for source in ["new C('capacity');","document.createEvent('CustomEvent');"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1);
        assert_eq!(page.state.created_events.len(),count);assert_eq!(page.state.retained,retained);assert_eq!(page.state.next_event,next);
    }
    page.state.limits.host.created_events=capacity;
    page.state.next_event=script_class_list::BASE;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('overflow');");assert_eq!(outcome.errors,1);
    assert_eq!(page.state.created_events.len(),count);assert_eq!(page.state.next_event,script_class_list::BASE);
    page.state.next_event=next;
    let work=page.state.limits.host_work;page.state.limits.host_work=1;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('work');");assert_eq!(outcome.errors,1);
    assert_eq!(page.state.created_events.len(),count);assert_eq!(page.state.retained,retained);assert_eq!(page.state.next_event,next);
    page.state.limits.host_work=work;
    evaluate(&mut page,&mut doc,&mut forms,"var combined=new C('N',{detail:'XYZ'});if(kept.detail!=='abc'||combined.detail!=='XYZ'||combined.type!=='N')throw 'real combined-budget recovery';");
    assert_eq!(page.state.retained,retained+4);
}

#[test]
fn custom_event_interface_unpublished_store_failure_restores_both_strings_without_id_reuse() {
    let(mut doc,mut forms,mut page)=fixture("var C=CustomEvent,kept=new C('kept',{detail:'abc'}),factoryKept=document.createEvent('CustomEvent'),savedInit={detail:'withdrawn-detail'};factoryKept.initCustomEvent('factory-kept',false,false,{value:53});");
    let count=page.state.created_events.len();let retained=page.state.retained;
    for source in ["new C('withdrawn-type',savedInit);","document.createEvent('CustomEvent');"] {
        let next=page.state.next_event;let normal=page.runtime.event_property_allowance_for_test(0);
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"Actual native isTrusted accessor installation must fail.");
        assert_eq!(page.state.created_events.len(),count,"No unpublished Custom record survives.");
        assert_eq!(page.state.retained,retained,"Both unpublished type and detail retained bytes are withdrawn.");
        assert_eq!(page.state.next_event,next+1,"Withdrawn IDs remain consumed.");
        page.runtime.event_property_allowance_for_test(normal);
    }
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=new C('recovered',{detail:'real'}),factory=document.createEvent('CustomEvent');factory.initCustomEvent('factory',true,true,{value:59});if(kept.detail!=='abc'||factoryKept.detail.value!==53||recovered.detail!=='real'||factory.detail.value!==59||Object.getPrototypeOf(recovered)!==C.prototype||Object.getPrototypeOf(factory)!==C.prototype||recovered.isTrusted||factory.isTrusted)throw 'actual storage recovery';");
    assert_eq!(page.state.created_events.len(),count+2);
}

#[test]
fn custom_event_utf16_detail_preserves_code_units_and_releases_the_old_payload() {
    let (mut doc, mut forms, mut page) = fixture("");
    let retained = page.state.retained;
    evaluate(&mut page, &mut doc, &mut forms, r#"
var raw='\uda22x\udf06', e=new CustomEvent('a',{detail:raw});
var f=document.createEvent('CustomEvent'); f.initCustomEvent('b',false,false,raw);
if(e.detail!==raw || f.detail!==e.detail || f.detail.length!==3 ||
   e.detail.charCodeAt(0)!==0xda22 || e.detail.charCodeAt(1)!==120 ||
   e.detail.charCodeAt(2)!==0xdf06)throw 'lost detail code units';
"#);
    let backing=page.state.created_events.capacity()*std::mem::size_of::<CreatedEvent>();
    assert_eq!(page.state.retained, retained + backing + 2 * (1 + 6));
    evaluate(&mut page, &mut doc, &mut forms,
        "f.initCustomEvent('z',false,false,'x'); if(f.detail!=='x'||e.detail!==raw)throw 'detail replacement';");
    assert_eq!(page.state.retained, retained + backing + 7 + 2);
}

#[test]
fn custom_event_utf16_detail_checks_copy_string_retention_and_rollback_boundaries() {
    let (mut doc, mut forms, mut page) = fixture("var raw='\\uda22x\\udf06', init={detail:raw}, e=new CustomEvent('e',init); e.initCustomEvent('e',true,true,raw); e.preventDefault();");
    let retained = page.state.retained;
    let count = page.state.created_events.len();
    let next = page.state.next_event;
    let handle = page.state.created_events[0].handle;
    {
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let normal_limit = bridge.state.limits.host.string_bytes;
        bridge.state.limits.host.string_bytes = 5;
        assert!(bridge.created_event_get(handle, "detail").unwrap().is_err());
        bridge.state.limits.host.string_bytes = 6;
        bridge.state.copy_work = 5;
        assert!(bridge.created_event_get(handle, "detail").unwrap().is_err());
        bridge.state.copy_work = 6;
        let detail = bridge.created_event_get(handle, "detail").unwrap().unwrap();
        assert!(matches!(&detail, Value::StringUnits(units) if units.as_slice() == [0xda22, 120, 0xdf06]));
        assert_eq!(bridge.state.copy_work, 0);
        bridge.state.copy_work = 6;
        let constructor = bridge.construct_custom_event(WINDOW, &[Value::String("x".into())], Some(&[("detail", detail)]));
        assert!(constructor.is_err());
        assert_eq!((bridge.state.retained, bridge.state.created_events.len(), bridge.state.next_event), (retained, count, next));
        bridge.state.limits.host.string_bytes = normal_limit;
    }
    let limit = page.state.limits.host.string_bytes;
    page.state.limits.host.string_bytes = 5;
    let failed = page.console_eval(&mut doc, &mut forms, "new CustomEvent('x',{detail:raw});");
    assert_eq!(failed.errors, 1, "{}", page.report());
    assert!(page.report().contains("CustomEvent detail exceeds the host string bound"), "{}", page.report());
    page.state.limits.host.string_bytes = limit;
    let normal_retained = page.state.retained_limit;
    page.state.retained_limit = retained;
    let failed = page.console_eval(&mut doc, &mut forms, "e.initCustomEvent('long',false,false,raw);");
    assert_eq!(failed.errors, 1, "{}", page.report());
    assert_eq!((page.state.retained, page.state.created_events.len(), page.state.next_event), (retained, count, next));
    page.state.retained_limit = normal_retained;
    let allowance = page.runtime.event_property_allowance_for_test(0);
    let failed = page.console_eval(&mut doc, &mut forms, "new CustomEvent('x',init);");
    assert_eq!(failed.errors, 1);
    assert_eq!((page.state.retained, page.state.created_events.len(), page.state.next_event), (retained, count, next + 1));
    page.runtime.event_property_allowance_for_test(allowance);
    evaluate(&mut page, &mut doc, &mut forms,
        "if(e.detail!==raw||e.type!=='e'||!e.bubbles||!e.cancelable||!e.defaultPrevented)throw 'event changed on failure';");
}

#[test]
fn custom_event_interface_ordinary_and_fatal_listener_paths_keep_real_detail_and_job_cleanup() {
    for fatal in [false,true] {
        let(mut doc,mut forms,mut page)=fixture("var C=CustomEvent,b=document.getElementById('button'),payload={calls:0},event=new C('run',{detail:payload}),tail=0,queued=0;");
        let source=if fatal {
            "b.addEventListener('run',function(e){e.detail.calls++;queueMicrotask(function(){queued++;});document.body.scrollWidth;});b.addEventListener('run',function(){tail++;});"
        } else {
            "b.addEventListener('run',function(e){e.detail.calls++;queueMicrotask(function(){queued++;});throw 17;});b.addEventListener('run',function(){tail++;});"
        };
        evaluate(&mut page,&mut doc,&mut forms,source);
        let outcome=page.console_eval(&mut doc,&mut forms,"b.dispatchEvent(event);");assert_eq!(outcome.errors,1);
        assert!(page.state.synthetic_dispatches.is_empty());assert!(page.state.created_events.iter().all(|event|!event.dispatching));
        assert!(!page.runtime.has_microtasks());
        let expected=if fatal {0}else{1};
        evaluate(&mut page,&mut doc,&mut forms,&format!("if(event.detail!==payload||payload.calls!==1||tail!=={expected}||queued!=={expected}||event.currentTarget!==null||event.eventPhase!==0)throw 'real callback completion cleanup';var fresh=new C('fresh',{{detail:payload}});if(fresh.detail!==payload||Object.getPrototypeOf(fresh)!==C.prototype)throw 'new-job constructor recovery';"));
    }
}
