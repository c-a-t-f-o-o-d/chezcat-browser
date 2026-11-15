//! Original legacy CustomEvent Page/Runtime/dispatch controls; no service IO.
use super::*;

fn page(source: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><head></head><body><div id=parent><button id=button type=button>Run</button></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/page").unwrap().0),
        path: None,
        root: None,
        fragment: String::new(),
    };
    page.set_location(&location);
    let outcome = page.start(
        &mut doc,
        &mut forms,
        &[Script {
            node: 0,
            label: "Original legacy CustomEvent control".into(),
            source: source.into(),
        }],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    (doc, forms, page)
}
fn evaluate(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn legacy_custom_event_defaults_any_detail_identity_readonly_and_receiver_brands() {
    let (mut doc, mut forms, mut page) = page(
        r#"
var e=document.createEvent('cUsToMeVeNt'),other=document.createEvent('CustomEvent'),base=document.createEvent('Event'),ui=document.createEvent('UIEvent');
if(e.type!==''||e.detail!==null||e.target!==null||e.bubbles||e.cancelable||e.isTrusted||e.composed||!('detail' in e)||'view' in e||'which' in e||'initUIEvent' in e||base.detail!==undefined||ui.detail!==0)throw 'factory slots';
if(typeof e.initCustomEvent!=='function'||e.initCustomEvent!==e.initCustomEvent||e.initCustomEvent.length!==1||typeof CustomEvent!=='function'||!('CustomEvent' in window)||Object.getPrototypeOf(e)!==CustomEvent.prototype||!(e instanceof CustomEvent))throw 'method/constructor policy';
var payload={value:7},symbol=Symbol('detail'),callable=function(){return 19;};
for(var value of [payload,callable,symbol,document.body,window,'Original string',0,false,null]){
 e.initCustomEvent('actual',true,true,value);if(e.detail!==value||e.isTrusted||!e.bubbles||!e.cancelable)throw 'actual any identity';
}
e.initCustomEvent('nan',false,false,NaN);if(!isNaN(e.detail))throw 'actual NaN';
e.initCustomEvent('default');if(e.detail!==null||e.bubbles||e.cancelable)throw 'missing defaults';
e.initCustomEvent('undefined',undefined,undefined,undefined);if(e.detail!==null)throw 'undefined uses default';
e.initCustomEvent('retained',false,true,payload);e.detail=99;if(e.detail!==payload)throw 'readonly detail';
var strict=false;try{(function(){'use strict';e.detail=0;})();}catch(x){strict=x.name==='TypeError';}if(!strict)throw 'strict readonly';
e.initEvent('base-reset',false,false);if(e.detail!==payload||e.type!=='base-reset')throw 'base init preserves subclass detail';
var init=e.initCustomEvent,conversions=0,type={toString:function(){conversions++;return 'borrowed';}},bad=0;
for(var receiver of [{},base,ui,null,undefined]){try{init.call(receiver,type);}catch(x){if(x.name==='TypeError')bad++;}}
try{new init(type);}catch(x){if(x.name==='TypeError')bad++;}if(bad!==6||conversions!==0)throw 'receiver/new before conversion';
init.call(other,type,true,true,payload);if(other.detail!==payload||other.type!=='borrowed'||conversions!==1)throw 'genuine borrowing';
var plural=false;try{document.createEvent('CustomEvents');}catch(x){plural=x.name==='NotSupportedError';}if(!plural)throw 'no invented plural alias';
"#,
    );
    evaluate(&mut page, &mut doc, &mut forms, "if(e.detail!==payload||callable()!==19||other.detail!==payload)throw 'fresh-job identity';");
}

#[test]
fn legacy_custom_event_real_propagation_cancellation_and_initialized_empty_type() {
    let (_, _, page) = page(
        r#"
var b=document.getElementById('button'),ancestor=document.getElementById('parent'),e=document.createEvent('CustomEvent'),payload={value:31},order='';
var uninitialized=0;try{b.dispatchEvent(e);}catch(x){if(x.name==='InvalidStateError')uninitialized++;}if(uninitialized!==1)throw 'factory initialized flag';
e.initCustomEvent('route',true,true,payload);var stamp=e.timeStamp;
ancestor.addEventListener('route',function(event){if(event!==e||event.detail!==payload||event.target!==b||event.currentTarget!==ancestor||event.isTrusted)throw 'capture';order+='capture;';},true);
b.addEventListener('route',function(event){if(event.detail.value!==31||event.eventPhase!==2)throw 'at target';event.detail.value=37;event.preventDefault();order+='target;';});
ancestor.addEventListener('route',function(event){if(event.detail.value!==37||event.eventPhase!==3||!event.defaultPrevented)throw 'bubble';order+='bubble;';});
if(b.dispatchEvent(e)!==false||order!=='capture;target;bubble;'||e.detail!==payload||e.target!==b||e.currentTarget!==null||e.eventPhase!==0||e.timeStamp!==stamp||!e.defaultPrevented)throw 'dispatch result/cleanup';
e.initCustomEvent('',false,false,payload);if(e.defaultPrevented||e.target!==null||e.timeStamp!==stamp||!b.dispatchEvent(e)||e.target!==b)throw 'initialized empty custom type';
var base=document.createEvent('Event');base.initEvent('');if(!b.dispatchEvent(base)||base.target!==b)throw 'initialized empty base type';
e.initCustomEvent('standalone',false,false,payload);var target=new EventTarget(),calls=0;
target.addEventListener('standalone',function(event){if(this!==target||event.detail!==payload||event.target!==target)throw 'standalone';calls++;});
if(!target.dispatchEvent(e)||calls!==1||e.detail!==payload)throw 'standalone dispatch';
"#,
    );
    assert!(page.state.synthetic_dispatches.is_empty());
}

#[test]
fn legacy_custom_event_conversion_is_atomic_and_inflight_init_keeps_conversion_effects() {
    let (_, _, _) = page(
        r#"
var b=document.getElementById('button'),e=document.createEvent('CustomEvent'),kept={value:1},nested={value:2},outer={value:3},marker={},log='';
e.initCustomEvent('kept',true,true,kept);e.preventDefault();
var caught=false;try{e.initCustomEvent({toString:function(){throw marker;}},false,false,outer);}catch(x){caught=x===marker;}
if(!caught||e.type!=='kept'||e.detail!==kept||!e.defaultPrevented)throw 'conversion failure partial state';
caught=false;try{e.initCustomEvent({toString:function(){e.initCustomEvent('nested',false,true,nested);throw marker;}},false,false,outer);}catch(x){caught=x===marker;}
if(!caught||e.type!=='nested'||e.detail!==nested||e.defaultPrevented)throw 'nested conversion effects';
var truth={valueOf:function(){throw 'boolean valueOf';},toString:function(){throw 'boolean toString';}};
var detail={valueOf:function(){throw 'detail valueOf';},toString:function(){throw 'detail toString';}};
e.initCustomEvent('guard',truth,truth,detail);if(e.detail!==detail||!e.bubbles||!e.cancelable)throw 'boolean/any conversion';
b.addEventListener('guard',function(event){
 var stamp=event.timeStamp,target=event.target,phase=event.eventPhase;
 event.initCustomEvent({toString:function(){log+='type;';event.preventDefault();detail.changed=7;return 'ignored';}},false,false,outer);
 if(log!=='type;'||event.type!=='guard'||event.detail!==detail||event.target!==target||event.eventPhase!==phase||event.timeStamp!==stamp||!event.defaultPrevented||detail.changed!==7)throw 'dispatch no-op/conversion effects';
 var failed=0;try{event.initCustomEvent();}catch(x){if(x.name==='TypeError')failed++;}try{event.initCustomEvent(Symbol());}catch(x){if(x.name==='TypeError')failed++;}if(failed!==2||event.detail!==detail||event.type!=='guard')throw 'dispatch argument conversion';
});
if(b.dispatchEvent(e)!==false||e.detail!==detail||log!=='type;')throw 'guard dispatch result';
"#,
    );
}

#[test]
fn legacy_custom_event_first_child_read_uses_owner_method_and_preserves_child_detail_realm() {
    let (mut doc, mut forms, mut page) = page(
        r#"
var e=document.createEvent('CustomEvent'),other=document.createEvent('CustomEvent'),base=document.createEvent('Event'),ui=document.createEvent('UIEvent'),mouse=document.createEvent('MouseEvent');
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
var RootFunction=Function,ChildFunction=w.Function,ChildObject=w.Object;
var read=ChildFunction('event','name','return event[name];'),invoke=ChildFunction('fn','event','value','return fn.call(event,"child-call",true,true,value);');
var childPayload=ChildFunction('return {number:41,run:function(){return 43;}};')();
ChildFunction('Function=17;')();Function=19;
var init=read(e,'initCustomEvent');
if(Object.getPrototypeOf(init)!==RootFunction.prototype||Object.getPrototypeOf(init)===ChildFunction.prototype||init!==e.initCustomEvent)throw 'owner first-read Function';
for(var item of [[base,'initEvent'],[ui,'initUIEvent'],[mouse,'initMouseEvent']]){var method=read(item[0],item[1]);if(Object.getPrototypeOf(method)!==RootFunction.prototype||Object.getPrototypeOf(method)===ChildFunction.prototype)throw 'initializer family owner';}
invoke(init,other,childPayload);
if(other.detail!==childPayload||Object.getPrototypeOf(other.detail)!==ChildObject.prototype||other.detail.run()!==43)throw 'real child detail identity';
(function(){var only={run:function(){return 47;}};e.initCustomEvent('retained',false,false,only);})();
"#,
    );
    evaluate(&mut page, &mut doc, &mut forms, "if(e.detail.run()!==47||other.detail!==childPayload||other.detail.run()!==43)throw 'exported detail graph survived entry';");
    let events = page.state.created_events.len();
    let outcome = page.console_eval(
        &mut doc,
        &mut forms,
        "w.document.createEvent('CustomEvent');",
    );
    assert_eq!(
        outcome.errors, 1,
        "Child document factories remain outside the current read-only child policy."
    );
    assert_eq!(page.state.created_events.len(), events);
    evaluate(&mut page, &mut doc, &mut forms, "var conversions=0;try{document.createEvent.call(w.document,{toString:function(){conversions++;return 'CustomEvent';}});}catch(x){if(x.name!=='TypeError')throw x;}if(conversions||other.detail!==childPayload)throw 'foreign Document factory brand';");
}

#[test]
fn legacy_custom_event_initializer_family_cached_and_fresh_child_reads_both_enforce_owner_origin() {
    let (mut doc, mut forms, mut page) = page(
        r#"
var cases=[
 [document.createEvent('Event'),document.createEvent('Event'),'initEvent'],
 [document.createEvent('CustomEvent'),document.createEvent('CustomEvent'),'initCustomEvent'],
 [document.createEvent('UIEvent'),document.createEvent('UIEvent'),'initUIEvent'],
 [document.createEvent('MouseEvent'),document.createEvent('MouseEvent'),'initMouseEvent']
],saved=[],RootFunction=Function;
for(var item of cases){var method=item[1][item[2]];if(Object.getPrototypeOf(method)!==RootFunction.prototype)throw 'cached owner Function';saved.push(method);}
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
var read=w.Function('probe','event','name','probe.entered++;var method=event[name];probe.escaped++;return method;');
var probe={entered:0,escaped:0};
"#,
    );
    let count = page.state.created_events.len();
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://other.test/").unwrap().0),
        path: None,
        root: None,
        fragment: String::new(),
    });
    let mut entered = 0;
    for family in 0..4 {
        for slot in 0..2 {
            let source = format!("read(probe,cases[{family}][{slot}],cases[{family}][2]);");
            let outcome = page.console_eval(&mut doc, &mut forms, &source);
            assert_eq!(
                outcome.errors,
                1,
                "Foreign {} lookup escaped owner validation for family {family}: {}",
                if slot == 0 { "fresh" } else { "cached" },
                page.report()
            );
            entered += 1;
            evaluate(&mut page,&mut doc,&mut forms,&format!("if(probe.entered!=={entered}||probe.escaped!==0)throw 'reader must enter then stop exactly at foreign native lookup';"));
            assert_eq!(page.state.created_events.len(), count);
        }
    }
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/page").unwrap().0),
        path: None,
        root: None,
        fragment: String::new(),
    });
    evaluate(&mut page,&mut doc,&mut forms,"for(var index=0;index<cases.length;index++){if(read(probe,cases[index][1],cases[index][2])!==saved[index])throw 'same-origin cached identity recovery';}if(probe.entered!==12||probe.escaped!==4)throw 'restored actual child reads';");
}

#[test]
fn legacy_custom_event_text_retention_copy_quota_creation_refusal_and_recovery_are_atomic() {
    let (mut doc, mut forms, mut page) = page("var e=document.createEvent('CustomEvent');e.initCustomEvent('old',true,true,'abc');e.preventDefault();");
    let retained = page.state.retained;
    let handle = page.state.created_events[0].handle;
    let stamp = page.state.created_events[0].timestamp;
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    let args = [
        Value::String("longer".into()),
        Value::Bool(false),
        Value::Bool(false),
        Value::String("longer-detail".into()),
    ];
    bridge.state.copy_work = 0;
    assert!(bridge.call(handle, "initCustomEvent", &args).is_err());
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.state.retained_limit = retained;
    assert!(bridge.call(handle, "initCustomEvent", &args).is_err());
    assert_eq!(bridge.state.retained, retained);
    let event = &bridge.state.created_events[0];
    assert_eq!(event.name, "old");
    assert!(event.initialized && event.bubbles && event.cancelable && event.prevented);
    assert!(matches!(&event.custom,Some(Value::String(value)) if value=="abc"));
    assert_eq!(event.timestamp, stamp);
    bridge
        .call(
            handle,
            "initCustomEvent",
            &[
                Value::String("x".into()),
                Value::Bool(false),
                Value::Bool(false),
                Value::String("z".into()),
            ],
        )
        .unwrap();
    assert_eq!(bridge.state.retained, retained - 6 + 2);
    let next = bridge.state.next_event;
    let capacity = bridge.state.limits.host.created_events;
    bridge.state.limits.host.created_events = 1;
    assert!(bridge
        .call(
            DOCUMENT,
            "createEvent",
            &[Value::String("CustomEvent".into())]
        )
        .is_err());
    assert_eq!(bridge.state.next_event, next);
    assert_eq!(bridge.state.created_events.len(), 1);
    bridge.state.limits.host.created_events = capacity;
    bridge.state.next_event = script_class_list::BASE;
    assert!(bridge
        .call(
            DOCUMENT,
            "createEvent",
            &[Value::String("CustomEvent".into())]
        )
        .is_err());
    assert_eq!(bridge.state.created_events.len(), 1);
    bridge.state.next_event = next;
    bridge
        .call(
            DOCUMENT,
            "createEvent",
            &[Value::String("CustomEvent".into())],
        )
        .unwrap();
    assert_eq!(bridge.state.created_events.len(), 2);
    drop(bridge);
    evaluate(
        &mut page,
        &mut doc,
        &mut forms,
        "if(e.type!=='x'||e.detail!=='z'||e.defaultPrevented)throw 'fresh entry recovery';",
    );
}

#[test]
fn legacy_custom_event_keeps_real_prototype_and_saved_native_after_error() {
    let (mut doc,mut forms,mut page)=page("var e=document.createEvent('CustomEvent');e.initCustomEvent('kept',false,false,{value:53});var saved=e.initCustomEvent;");
    evaluate(&mut page,&mut doc,&mut forms,"if(Object.getPrototypeOf(e)!==CustomEvent.prototype||!(e instanceof CustomEvent))throw 'genuine CustomEvent prototype';");
    let outcome=page.console_eval(&mut doc,&mut forms,"document.body.scrollWidth;");
    assert_eq!(outcome.errors,1,"Retain a real independent native-capability failure before saved-method recovery.");
    evaluate(&mut page,&mut doc,&mut forms,"saved.call(e,'recovered',false,false,e.detail);if(e.type!=='recovered'||e.detail.value!==53)throw 'saved native recovery';");
}

#[test]
fn legacy_custom_event_fatal_listener_cleans_dispatch_without_losing_detail_or_running_stale_jobs()
{
    let (mut doc, mut forms, mut page) = page(
        r#"
var b=document.getElementById('button'),e=document.createEvent('CustomEvent'),payload={calls:0},tail=0,stale=0;
e.initCustomEvent('fatal',true,true,payload);
b.addEventListener('fatal',function(event){event.detail.calls++;queueMicrotask(function(){stale++;});document.body.scrollWidth;});
b.addEventListener('fatal',function(){tail++;});
"#,
    );
    let outcome = page.console_eval(&mut doc, &mut forms, "b.dispatchEvent(e);");
    assert_eq!(outcome.errors, 1);
    assert!(page.state.synthetic_dispatches.is_empty());
    assert!(!page.state.created_events[0].dispatching);
    evaluate(&mut page,&mut doc,&mut forms,"if(e.detail!==payload||payload.calls!==1||tail!==0||stale!==0||e.eventPhase!==0||e.currentTarget!==null)throw 'fatal cleanup';e.initCustomEvent('fresh',false,false,payload);if(!b.dispatchEvent(e)||e.detail!==payload)throw 'fresh dispatch recovery';");
}
