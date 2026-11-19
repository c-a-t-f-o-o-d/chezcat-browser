//! Original browser-host regressions; Node cannot validate this native scheduler.
use super::*;

#[test]
fn animation_operation_aliases_descriptors_inheritance_and_readonly_errors() {
    let(mut doc,mut forms,mut page,start)=page(&[]);
    assert_eq!(start.errors,0);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,
        include_str!("../tools/fixtures/animation-global-properties.js")).unwrap().to_text(),"animation-properties-ok");
    assert!(page.state.animation_frames.is_empty());
}

#[test]
fn animation_operation_declarations_and_lexicals_use_ordinary_global_slots() {
    let(_,_,page,start)=page(&[
        "var reads=0,writes=0,last;Object.defineProperty(window,'requestAnimationFrame',{get:function(){reads++;return 13;},set:function(x){writes++;last=x;},configurable:true});",
        "var requestAnimationFrame;",
        "if(reads||writes)throw 'declaration getter';var requestAnimationFrame=42;",
        "if(reads||writes!==1||last!==42||requestAnimationFrame!==13||reads!==1)throw 'initializer';",
        "function requestAnimationFrame(){return 29;}",
        "if(reads!==1||writes!==1||requestAnimationFrame()!==29||Object.getOwnPropertyDescriptor(window,'requestAnimationFrame').configurable)throw 'function declaration';",
    ]);
    assert_eq!(start.errors,0,"{}",page.report());
    let(mut doc,mut forms,mut page,start)=self::page(&[
        "var nativeFrame=requestAnimationFrame;delete window.requestAnimationFrame;",
        "let requestAnimationFrame=23;if(requestAnimationFrame!==23||window.requestAnimationFrame!==undefined)throw 'lexical';",
    ]);
    assert_eq!(start.errors,0,"{}",page.report());
    let error=execute(&mut page,&mut doc,&mut forms,"document.title='wrong';var requestAnimationFrame;").unwrap_err();
    assert!(error.contains("conflict"),"{error}");assert_eq!(doc.title,"waiting");
    let(_,_,page,start)=self::page(&[
        "delete window.cancelAnimationFrame;(0,eval)('var cancelAnimationFrame=11;');if(cancelAnimationFrame!==11||!Object.getOwnPropertyDescriptor(window,'cancelAnimationFrame').configurable)throw 'indirect eval';delete window.cancelAnimationFrame;",
        "let cancelAnimationFrame=31;if(cancelAnimationFrame!==31||window.cancelAnimationFrame!==undefined)throw 'eval deletion';",
    ]);
    assert_eq!(start.errors,0,"{}",page.report());
}

#[test]
fn animation_saved_call_apply_bound_null_undefined_and_callback_this_are_real() {
    let(mut doc,mut forms,mut page,start)=page(&[r#"
var nativeFrame=requestAnimationFrame,nativeCancel=cancelAnimationFrame,count=0,converted=0;
delete window.requestAnimationFrame;delete window.cancelAnimationFrame;
for(var receiver of [undefined,null,window]){
 var id=nativeFrame.call(receiver,function(){throw 'cancelled';});
 nativeCancel.apply(receiver,[{valueOf:function(){converted++;return id;}}]);
 nativeFrame.bind(receiver)(function(){count++;});
}
var object={marker:1};
nativeFrame(function(){'use strict';if(this!==undefined)throw 'strict callback this';count++;});
nativeFrame(function(){if(this!==window)throw 'sloppy callback this';count++;});
nativeFrame((function(){'use strict';if(this!==object)throw 'bound callback this';count++;}).bind(object));
nativeFrame(()=>{if(this!==window)throw 'arrow callback this';count++;});
if(converted!==3)throw 'cancel conversion';
"#]);assert_eq!(start.errors,0,"{}",page.report());
    assert_eq!(page.state.animation_frames.len(),7);poll(&mut page,&mut doc,&mut forms);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"count;").unwrap().to_number(),7.0);
    assert!(page.next_timer_deadline().is_none());
}

#[test]
fn animation_incompatible_receivers_refuse_before_cancellation_conversion() {
    let(_,_,page,start)=page(&[r#"
var frame=requestAnimationFrame,cancel=cancelAnimationFrame,effects=0,catches=0;
var id=frame(function(){});
for(var receiver of [{},1,true,'x',Symbol('x'),document,function(){}]){
 try{frame.call(receiver,function(){});}catch(e){if(e.name!=='TypeError')throw e;catches++;}
 try{cancel.call(receiver,{valueOf:function(){effects++;return id;}});}catch(e){if(e.name!=='TypeError')throw e;catches++;}
}
if(catches!==14||effects!==0)throw 'receiver before coercion';
"#]);assert_eq!(start.errors,0,"{}",page.report());
    assert_eq!(page.state.animation_frames.len(),1);assert_eq!(page.state.next_animation_frame,1);
}

#[test]
fn animation_cancel_conversions_are_ordered_and_full_unsigned_ids_remain_exact() {
    let(mut doc,mut forms,mut page,start)=page(&[r#"
var trace='',callback=function(){},id=requestAnimationFrame(callback),caught=0;
cancelAnimationFrame({valueOf:function(){trace+='v';return {};},toString:function(){trace+='s';return ''+(id+4294967296.75);}});
if(trace!=='vs')throw 'numeric conversion';
try{cancelAnimationFrame(Symbol('x'));}catch(e){if(e.name!=='TypeError')throw e;caught++;}
try{cancelAnimationFrame({valueOf:function(){throw Error('expected');}});}catch(e){if(e.message!=='expected')throw e;caught++;}
if(caught!==2)throw 'conversion errors';
"#]);assert_eq!(start.errors,0,"{}",page.report());assert!(page.state.animation_frames.is_empty());
    page.state.next_animation_frame=u32::MAX-1;
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"var last=requestAnimationFrame(callback);last;").unwrap().to_number(),f64::from(u32::MAX));
    execute(&mut page,&mut doc,&mut forms,"cancelAnimationFrame(-1.9);").unwrap();
    assert!(page.state.animation_frames.is_empty());assert!(page.state.animation_due.is_none());
    let error=execute(&mut page,&mut doc,&mut forms,"requestAnimationFrame(callback);").unwrap_err();
    assert!(error.contains("identifier limit"));assert_eq!(page.state.next_animation_frame,u32::MAX);assert!(page.state.animation_frames.is_empty());
}

#[test]
fn animation_cancel_accessor_is_captured_before_arguments_and_coercion() {
    let(_,_,page,start)=page(&[r#"
var original=cancelAnimationFrame,trace='',id=requestAnimationFrame(function(){throw 'not cancelled';});
Object.defineProperty(window,'cancelAnimationFrame',{configurable:true,get:function(){trace+='g';return original;}});
function argument(){trace+='a';Object.defineProperty(window,'cancelAnimationFrame',{value:function(){throw 'late lookup';},configurable:true});return {valueOf:function(){trace+='v';return id;}};}
window.cancelAnimationFrame(argument());
if(trace!=='gav')throw 'cancel evaluation order';
"#]);assert_eq!(start.errors,0,"{}",page.report());assert!(page.state.animation_frames.is_empty());
}

#[test]
fn animation_microtasks_cancel_live_batch_defer_new_frames_and_report_ordinary_errors() {
    let(mut doc,mut forms,mut page,start)=page(&[r#"
var order='',stamp=-1;
requestAnimationFrame(function(t){stamp=t;order+='a';Promise.resolve().then(function(){order+='m';cancelAnimationFrame(cancelled);requestAnimationFrame(function(next){if(next<stamp)throw 'reversed';order+='r';});});throw Error('expected callback error');});
var cancelled=requestAnimationFrame(function(){throw 'cancelled callback';});
requestAnimationFrame(function(t){if(t!==stamp)throw 'different timestamp';order+='d';});
"#]);assert_eq!(start.errors,0,"{}",page.report());let due=page.state.animation_due.unwrap();
    let result=page.poll_timer_at(&mut doc,&mut forms,due).unwrap();assert_eq!(result.errors,1,"{}",page.report());
    assert!(page.report().contains("expected callback error"));
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"order;").unwrap().to_text(),"amd");
    assert_eq!(page.state.animation_frames.len(),1);assert!(page.poll_timer_at(&mut doc,&mut forms,due).is_none());
    let next=page.state.animation_due.unwrap();let result=page.poll_timer_at(&mut doc,&mut forms,next).unwrap();assert_eq!(result.errors,0,"{}",page.report());
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"order;").unwrap().to_text(),"amdr");
}

#[test]
fn animation_fatal_callback_and_microtask_cancel_frame_work_and_fresh_jobs_recover() {
    for body in [
        "order+='a';queueMicrotask(function(){order+='wrong';});requestAnimationFrame(function(){order+='wrong';});while(true){}",
        "order+='a';requestAnimationFrame(function(){order+='wrong';});queueMicrotask(function(){order+='m';while(true){}});",
    ] {
        // This explicit small job profile also initializes the real intrinsics;
        // callback exhaustion remains far below the normal billion-step policy.
        let mut limits=crate::javascript::limits::BROWSER;limits.steps=50_000;
        let source=format!("var order='';requestAnimationFrame(function(){{{body}}});requestAnimationFrame(function(){{order+='wrong';}});");
        let(mut doc,mut forms,mut page,start)=page_with_policy(&[&source],limits);
        assert_eq!(start.errors,0,"{}",page.report());let due=page.state.animation_due.unwrap();
        let outcome=page.poll_timer_at(&mut doc,&mut forms,due).unwrap();assert_eq!(outcome.errors,1,"{}",page.report());
        assert!(page.state.animation_frames.is_empty());assert!(page.next_timer_deadline().is_none());assert!(!page.runtime.has_microtasks());
        let expected=if body.contains("order+='m'"){ "am" }else{"a"};
        assert_eq!(execute(&mut page,&mut doc,&mut forms,"order;").unwrap().to_text(),expected);
        execute(&mut page,&mut doc,&mut forms,"requestAnimationFrame(function(){order+='r';});").unwrap();poll(&mut page,&mut doc,&mut forms);
        assert_eq!(execute(&mut page,&mut doc,&mut forms,"order;").unwrap().to_text(),format!("{expected}r"));
    }
}

#[test]
fn animation_checkpoint_rejection_scan_shares_frame_work_and_recovers() {
    let mut limits=crate::javascript::limits::BROWSER;limits.steps=50_000;
    let(mut doc,mut forms,mut page,start)=page_with_policy(&["var objects=[],count=0;function callback(){count++;}"],limits);
    assert_eq!(start.errors,0,"{}",page.report());
    // Populate a real retained arena through independent bounded script jobs.
    for _ in 0..16 {execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<128;i++)objects.push({value:i});").unwrap();}
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<64;i++)requestAnimationFrame(callback);").unwrap();
    let due=page.state.animation_due.unwrap();let result=page.poll_timer_at(&mut doc,&mut forms,due).unwrap();
    assert_eq!(result.errors,1,"{}",page.report());assert!(page.report().contains("Microtask checkpoint"));
    let count=execute(&mut page,&mut doc,&mut forms,"count;").unwrap().to_number();assert!(count>0.0 && count<64.0,"{count}");
    assert!(page.state.animation_frames.is_empty());assert!(page.next_timer_deadline().is_none());
    execute(&mut page,&mut doc,&mut forms,"requestAnimationFrame(callback);").unwrap();poll(&mut page,&mut doc,&mut forms);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"count;").unwrap().to_number(),count+1.0);
}

#[test]
fn animation_small_capacity_and_host_work_refusal_are_atomic_and_recover() {
    let(mut doc,mut forms,mut page,start)=page(&["var callback=function(){},first=requestAnimationFrame(callback),second=requestAnimationFrame(callback);"]);
    assert_eq!(start.errors,0);let due=page.state.animation_due;let ids=page.state.animation_frames.iter().map(|f|f.id).collect::<Vec<_>>();
    page.state.limits.host.animation_frames=2;
    assert!(execute(&mut page,&mut doc,&mut forms,"requestAnimationFrame(callback);").unwrap_err().contains("Animation frame limit"));
    assert_eq!(page.state.next_animation_frame,2);assert_eq!(page.state.animation_due,due);
    page.state.limits.host_work=1;
    assert!(execute(&mut page,&mut doc,&mut forms,"cancelAnimationFrame(first);").unwrap_err().contains("DOM work limit"));
    assert_eq!(page.state.animation_frames.iter().map(|f|f.id).collect::<Vec<_>>(),ids);assert_eq!(page.state.animation_due,due);
    page.state.limits.host_work=crate::javascript::limits::BROWSER.host_work;
    execute(&mut page,&mut doc,&mut forms,"cancelAnimationFrame(first);requestAnimationFrame(callback);").unwrap();
    assert_eq!(page.state.animation_frames.iter().map(|f|f.id).collect::<Vec<_>>(),vec![2,3]);poll(&mut page,&mut doc,&mut forms);
    assert!(page.next_timer_deadline().is_none());
    execute(&mut page,&mut doc,&mut forms,"requestAnimationFrame(callback);requestAnimationFrame(callback);").unwrap();
    page.state.limits.host_work=1;let due=page.state.animation_due.unwrap();let outcome=page.poll_timer_at(&mut doc,&mut forms,due).unwrap();
    assert_eq!(outcome.errors,1);assert!(page.state.animation_frames.is_empty());assert!(page.next_timer_deadline().is_none());
}

#[test]
fn animation_generous_normal_queue_drains_in_order_without_snapshot_allocation() {
    let(mut doc,mut forms,mut page,start)=page(&[r#"
var count=0,stamp=-1;
function callback(t){if(stamp<0)stamp=t;else if(t!==stamp)throw 'batch timestamp';count++;}
for(var i=0;i<4096;i++)requestAnimationFrame(callback);
"#]);assert_eq!(start.errors,0,"{}",page.report());assert_eq!(page.state.animation_frames.len(),4096);
    assert!(page.state.animation_frames.capacity()<page.state.limits.host.animation_frames);
    poll(&mut page,&mut doc,&mut forms);assert_eq!(page.state.animation_callbacks,4096);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"count;").unwrap().to_number(),4096.0);
    assert!(page.next_timer_deadline().is_none());
}

#[test]
fn animation_disabled_saved_bound_capabilities_and_stop_cannot_run_pending_work() {
    let(mut doc,mut forms,mut page,start)=page(&["var effects=0,callback=function(){document.title='wrong';},argument={valueOf:function(){effects++;return 1;}};requestAnimationFrame(callback);"]);
    assert_eq!(start.errors,0);
    let frame=execute(&mut page,&mut doc,&mut forms,"requestAnimationFrame.bind(null);").unwrap();
    let cancel=execute(&mut page,&mut doc,&mut forms,"cancelAnimationFrame.bind(undefined);").unwrap();
    let callback=execute(&mut page,&mut doc,&mut forms,"callback;").unwrap();
    let argument=execute(&mut page,&mut doc,&mut forms,"argument;").unwrap();
    for disabled_document in [false,true] {
        page.state.enabled=disabled_document;doc.scripting_enabled=!disabled_document;
        for (function,arg) in [(frame.clone(),callback.clone()),(cancel.clone(),argument.clone())] {
            page.begin_job();let error=page.runtime.invoke(function,&[arg],Value::Undefined,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
            assert!(error.contains("scripting is disabled"),"{error}");assert_eq!(page.state.animation_frames.len(),1);
        }
        assert!(page.poll_timer_at(&mut doc,&mut forms,Instant::now()+Duration::from_secs(1)).is_none());
    }
    page.state.enabled=true;doc.scripting_enabled=true;
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"effects;").unwrap().to_number(),0.0);
    execute(&mut page,&mut doc,&mut forms,"queueMicrotask(function(){document.title='wrong microtask';});").unwrap();
    page.stop_timers("Original animation cancellation test.");assert!(page.state.animation_frames.is_empty());assert!(page.next_timer_deadline().is_none());assert!(!page.runtime.has_microtasks());
    assert!(page.poll_timer_at(&mut doc,&mut forms,Instant::now()+Duration::from_secs(1)).is_none());assert_eq!(doc.title,"waiting");
    execute(&mut page,&mut doc,&mut forms,"requestAnimationFrame(function(){document.title='fresh';});").unwrap();poll(&mut page,&mut doc,&mut forms);assert_eq!(doc.title,"fresh");
}

#[test]
fn animation_globals_are_replaceable_without_hidden_restore_and_keep_real_callbacks() {
    let(mut doc,mut forms,mut page,start)=page(&[r#"
var nativeFrame=requestAnimationFrame,nativeCancel=cancelAnimationFrame,trace='',stamp=-1,conversions=0;
for(var name of ['requestAnimationFrame','cancelAnimationFrame']){var d=Object.getOwnPropertyDescriptor(window,name);if(d.value!==window[name]||!d.writable||!d.enumerable||!d.configurable||d.value.length!==1)throw 'operation descriptor';}
window.requestAnimationFrame=function(callback){trace+='wrap;';return nativeFrame.call(window,function(t){if(this!==window||t<0||t%1!==0)throw 'native callback';stamp=t;trace+='frame;';callback(t);});};
window.cancelAnimationFrame=function(id){trace+='cancel;';return nativeCancel({valueOf:function(){conversions++;return id+4294967296;}});};
var cancelled=requestAnimationFrame(function(){throw 'cancelled';});cancelAnimationFrame(cancelled);
requestAnimationFrame(function(t){trace+='callback;';if(t!==stamp)throw 'timestamp';Promise.resolve().then(function(){trace+='microtask;';document.title=trace;});});
delete window.requestAnimationFrame;delete window.cancelAnimationFrame;
if(typeof requestAnimationFrame!=='undefined'||typeof cancelAnimationFrame!=='undefined'||'requestAnimationFrame' in window||'cancelAnimationFrame' in window)throw 'hidden restore';
Object.defineProperty(window,'requestAnimationFrame',{configurable:true,get:function(){trace+='get;';return nativeFrame;}});
function argument(){trace+='argument;';Object.defineProperty(window,'requestAnimationFrame',{configurable:true,writable:true,value:function(){throw 'late lookup';}});return function(){trace+='captured;';};}
window.requestAnimationFrame(argument());
if(trace!=='wrap;cancel;wrap;get;argument;'||conversions!==1)throw 'lookup order';
"#]);assert_eq!(start.errors,0,"{}",page.report());assert_eq!(page.state.animation_frames.len(),2);assert!(!page.state.native_functions.contains_key("requestAnimationFrame"));assert!(!page.state.native_functions.contains_key("cancelAnimationFrame"));
    poll(&mut page,&mut doc,&mut forms);assert_eq!(page.state.animation_callbacks,2);assert_eq!(doc.title,"wrap;cancel;wrap;get;argument;frame;callback;microtask;");
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"trace;").unwrap().to_text(),"wrap;cancel;wrap;get;argument;frame;callback;microtask;captured;");assert!(page.state.animation_frames.is_empty());
}

#[test]
fn animation_global_conversion_capacity_identity_and_disabled_owner_controls() {
    let(mut doc,mut forms,mut page,start)=page(&[r#"
var errors=0;for(var action of [function(){requestAnimationFrame();},function(){cancelAnimationFrame();},function(){requestAnimationFrame(null);},function(){requestAnimationFrame('bad');}]){try{action();}catch(e){if(e instanceof TypeError)errors++;else throw e;}}if(errors!==4)throw 'IDL errors';
var callback=function(){},nativeFrame=requestAnimationFrame,nativeCancel=cancelAnimationFrame;
var id=requestAnimationFrame(callback);cancelAnimationFrame({valueOf:function(){return id-4294967296;}});
if(cancelAnimationFrame(Infinity)!==undefined)throw 'nonfinite ID';
for(var i=0;i<300;i++)requestAnimationFrame(callback);
"#]);assert_eq!(start.errors,0,"{}",page.report());assert_eq!(page.state.animation_frames.len(),300);assert!(page.state.animation_frames.capacity()<page.state.limits.host.animation_frames);
    let count=page.state.animation_frames.len();let next=page.state.next_animation_frame;page.state.limits.host.animation_frames=count;
    assert!(execute(&mut page,&mut doc,&mut forms,"nativeFrame(callback);").unwrap_err().contains("Animation frame limit"));assert_eq!(page.state.next_animation_frame,next);assert_eq!(page.state.animation_frames.len(),count);
    page.state.limits.host.animation_frames=crate::javascript::limits::BROWSER.host.animation_frames;page.state.next_animation_frame=u32::MAX;
    assert!(execute(&mut page,&mut doc,&mut forms,"nativeFrame(callback);").unwrap_err().contains("identifier limit"));assert_eq!(page.state.animation_frames.len(),count);
    let frame=execute(&mut page,&mut doc,&mut forms,"nativeFrame;").unwrap();let cancel=execute(&mut page,&mut doc,&mut forms,"nativeCancel;").unwrap();let callback=execute(&mut page,&mut doc,&mut forms,"callback;").unwrap();let argument=execute(&mut page,&mut doc,&mut forms,"var effects=0;({valueOf:function(){effects++;return 1;}});").unwrap();
    doc.scripting_enabled=false;
    for (function,arg) in [(frame,callback),(cancel,argument)] {
        let error=page.runtime.invoke(function,&[arg],Value::Host(WINDOW),&mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();assert!(error.contains("scripting is disabled"),"{error}");assert_eq!(page.state.animation_frames.len(),count);
    }
    doc.scripting_enabled=true;assert_eq!(execute(&mut page,&mut doc,&mut forms,"effects;").unwrap().to_number(),0.0);
}

fn page(sources: &[&str]) -> (Document, Forms, Page, Outcome) {
    page_with_policy(sources, crate::javascript::limits::BROWSER)
}

fn page_with_policy(sources: &[&str], limits: crate::javascript::limits::Limits) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse("<html><head><title>waiting</title></head><body><p id=out>Waiting</p></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let scripts: Vec<_> = sources.iter().enumerate().map(|(node, source)| Script {
        node, label: format!("original timer-global fixture {node}"), source: (*source).into(),
    }).collect();
    let mut page = Page::with_limits(limits);
    let outcome = page.start(&mut doc, &mut forms, &scripts);
    (doc, forms, page, outcome)
}

fn execute(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) -> Result<Value, String> {
    page.begin_job();
    page.runtime.execute(source, &mut Bridge { doc, forms, state: &mut page.state })
}

fn poll(page: &mut Page, doc: &mut Document, forms: &mut Forms) {
    let outcome = page.poll_timer_at(doc, forms, Instant::now() + Duration::from_secs(1)).unwrap();
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn timer_globals_have_ordinary_aliases_descriptors_and_no_hidden_restore() {
    let (mut doc, mut forms, mut page, start) = page(&[]);
    assert_eq!(start.errors, 0);
    assert_eq!(execute(&mut page, &mut doc, &mut forms,
        include_str!("../tools/fixtures/timer-global-properties.js")).unwrap().to_text(), "ok");
    assert_eq!(execute(&mut page, &mut doc, &mut forms,
        "setTimeout.length===1&&clearTimeout.length===0&&setTimeout.name==='setTimeout'&&clearTimeout.name==='clearTimeout';"
    ).unwrap().to_text(), "true");
    assert!(page.state.timers.is_empty());
    assert!(!page.state.native_functions.contains_key("setTimeout"));
    assert!(!page.state.native_functions.contains_key("clearTimeout"));
}

#[test]
fn timer_wrappers_delegate_real_callbacks_arguments_cancellation_and_microtasks() {
    let (mut doc, mut forms, mut page, start) = page(&[
        include_str!("../tools/fixtures/timer-wrapper-script.js"),
    ]);
    assert_eq!(start.errors, 0, "{}", page.report());
    assert_eq!(page.state.timers.len(), 1);
    assert_eq!(doc.title, "waiting");
    poll(&mut page, &mut doc, &mut forms);
    assert_eq!(doc.title, "script:wrap:cancel:wrap:timer:microtask:1");
    assert_eq!(page.state.timer_callbacks, 1);
    assert_eq!(page.state.timer_bytes, 0);
    assert!(page.next_timer_deadline().is_none());
    assert!(!page.runtime.has_microtasks());
}

#[test]
fn timer_accessor_lookup_precedes_arguments_and_reads_once() {
    let (mut doc, mut forms, mut page, start) = page(&[r#"
var nativeTimer=setTimeout, trace='';
Object.defineProperty(window,'setTimeout',{
 get:function(){if(this!==window)throw 'receiver';trace+='get;';return nativeTimer;},
 configurable:true
});
function argument(){trace+='arg;';Object.defineProperty(window,'setTimeout',{
 value:function(){throw 'replacement must not be called';},writable:true});
 return function(){document.title=trace;};}
window.setTimeout(argument(),0);
if(trace!=='get;arg;')throw trace;
"#]);
    assert_eq!(start.errors, 0, "{}", page.report());
    assert_eq!(page.state.timers.len(), 1);
    poll(&mut page, &mut doc, &mut forms);
    assert_eq!(doc.title, "get;arg;");
}

#[test]
fn timer_noncallable_and_readonly_replacements_obey_descriptors() {
    let (_, _, page, start) = page(&[r#"
var saved=setTimeout, catches=0;
setTimeout=9;
try{setTimeout(function(){},0);}catch(e){if(e.name!=='TypeError')throw e;catches++;}
try{window.setTimeout(function(){},0);}catch(e){if(e.name!=='TypeError')throw e;catches++;}
Object.defineProperty(window,'setTimeout',{value:saved,writable:false,configurable:false});
setTimeout=9;if(setTimeout!==saved)throw 'sloppy readonly assignment';
try{(function(){'use strict';setTimeout=9;})();}catch(e){if(e.name!=='TypeError')throw e;catches++;}
if(delete window.setTimeout)throw 'deleted nonconfigurable timer';
try{(function(){'use strict';delete window.setTimeout;})();}catch(e){if(e.name!=='TypeError')throw e;catches++;}
try{Object.defineProperty(window,'setTimeout',{value:9});}catch(e){if(e.name!=='TypeError')throw e;catches++;}
if(catches!==5||setTimeout!==saved)throw 'descriptor semantics';
"#]);
    assert_eq!(start.errors, 0, "{}", page.report());
    assert!(page.state.timers.is_empty());
}

#[test]
fn timer_deleted_identifier_can_be_recreated_without_restoring_native_slot() {
    let (_, _, page, start) = page(&[r#"
var nativeTimer=setTimeout, caught=false;
delete window.setTimeout;
try{(function(){'use strict';setTimeout=nativeTimer;})();}catch(e){caught=e.name==='ReferenceError';}
if(!caught||typeof setTimeout!=='undefined'||'setTimeout' in window)throw 'deleted lookup';
setTimeout=function(){return 'replacement';};
if(window.setTimeout!==setTimeout||self.setTimeout()!=='replacement')throw 'recreate';
delete setTimeout;
if(typeof setTimeout!=='undefined'||window.setTimeout!==undefined)throw 'identifier delete';
Object.assign(window,{setTimeout:nativeTimer});
if(setTimeout!==nativeTimer)throw 'assign restore';
"#]);
    assert_eq!(start.errors, 0, "{}", page.report());
    assert!(page.state.timers.is_empty());
}

#[test]
fn timer_var_declarations_do_not_read_or_migrate_accessors() {
    let (mut doc, mut forms, mut page, start) = page(&[r#"
var reads=0,writes=0,last;
Object.defineProperty(window,'setTimeout',{
 get:function(){reads++;return 13;},set:function(x){writes++;last=x;},configurable:true
});
"#, "var setTimeout;", "if(reads!==0||writes!==0)throw 'var invoked accessor';var setTimeout=42;",
"if(reads!==0||writes!==1||last!==42||setTimeout!==13||reads!==1)throw 'initializer path';"]);
    assert_eq!(start.errors, 0, "{}", page.report());
    // A prior var does not turn an existing configurable operation property
    // into a restricted global property (ECMA-262 2026, 16.1.7).
    assert_eq!(execute(&mut page,&mut doc,&mut forms,
        "let setTimeout=1;setTimeout===1&&window.setTimeout===13&&reads===2;"
    ).unwrap().to_text(),"true");
    assert_eq!(doc.title,"waiting");
}

#[test]
fn timer_function_declaration_replaces_configurable_accessor_without_invocation() {
    let (mut doc, mut forms, mut page, start) = page(&[r#"
var reads=0,writes=0;
Object.defineProperty(window,'setTimeout',{get:function(){reads++;},set:function(){writes++;},configurable:true});
"#, "function setTimeout(){return 9;}", r#"
var d=Object.getOwnPropertyDescriptor(window,'setTimeout');
if(reads||writes||setTimeout()!==9||d.value!==setTimeout||!d.writable||!d.enumerable||d.configurable)throw 'function declaration';
"#]);
    assert_eq!(start.errors, 0, "{}", page.report());
    execute(&mut page,&mut doc,&mut forms,
        "Object.defineProperty(window,'setTimeout',{writable:false});").unwrap();
    let error=execute(&mut page,&mut doc,&mut forms,
        "var fresh;function setTimeout(){}document.title='must not run';").unwrap_err();
    // execute() returns Fault::display(), which prints the diagnostic message,
    // not the structured exception name. Check the two contracts separately.
    assert!(
        error.contains("Cannot declare global function or variable \"setTimeout\"."),
        "{error}"
    );
    assert_eq!(doc.title, "waiting");
    assert_eq!(execute(&mut page, &mut doc, &mut forms, r#"
typeof fresh==='undefined'&&!('fresh' in window)&&
setTimeout()===9&&reads===0&&writes===0;
"#).unwrap().to_text(), "true");
    // A global indirect eval uses the same declaration preflight, but exposes
    // the real exception object to JavaScript rather than formatting it.
    assert_eq!(execute(&mut page, &mut doc, &mut forms, r#"
var declarationError='';
try {
    (0,eval)('function setTimeout(){}document.title="must not run";');
} catch(e) { declarationError=e.name; }
declarationError==='TypeError'&&typeof fresh==='undefined'&&
!('fresh' in window)&&setTimeout()===9&&reads===0&&writes===0;
"#).unwrap().to_text(), "true");
    assert_eq!(doc.title, "waiting");
}

#[test]
fn timer_global_lexicals_shadow_the_public_operation_but_not_window() {
    let (mut doc, mut forms, mut page, start) = page(&[
        "window.savedTimer=setTimeout;",
        "let setTimeout=17;",
        "if(setTimeout!==17||window.setTimeout!==window.savedTimer)throw 'lexical shadow';setTimeout=18;",
        "if(setTimeout!==18||self.setTimeout!==window.savedTimer)throw 'lexical assignment';",
    ]);
    assert_eq!(start.errors,0,"{}",page.report());
    let error=execute(&mut page,&mut doc,&mut forms,
        "document.title='must not run';var setTimeout;").unwrap_err();
    assert!(error.contains("conflict"),"{error}");
    assert_eq!(doc.title,"waiting");
    assert_eq!(execute(&mut page,&mut doc,&mut forms,
        "(function(setTimeout){return setTimeout+1;})(3);").unwrap().to_text(),"4");
}

#[test]
fn timer_eval_declarations_and_deletion_keep_global_configurability() {
    let (mut doc, mut forms, mut page, start) = page(&[r#"
delete window.setTimeout;
eval('var setTimeout=19;');
if(setTimeout!==19||!Object.getOwnPropertyDescriptor(window,'setTimeout').configurable)throw 'eval var';
delete setTimeout;
eval.call(null,'function setTimeout(){return 23;}');
if(setTimeout()!==23||!Object.getOwnPropertyDescriptor(window,'setTimeout').configurable)throw 'eval function';
delete setTimeout;
"#, "let setTimeout=27;if(window.setTimeout!==undefined||setTimeout!==27)throw 'delete binding';"]);
    assert_eq!(start.errors,0,"{}",page.report());
    // Member deletion leaves no restricted own property. No hidden native
    // binding or obsolete var-name registry may prevent the following lexical.
    let (mut doc2,mut forms2,mut page2,start2)=self::page(&["var clearTimeout;delete window.clearTimeout;"]);
    assert_eq!(start2.errors,0,"{}",page2.report());
    assert_eq!(execute(&mut page2,&mut doc2,&mut forms2,
        "let clearTimeout=1;clearTimeout===1&&window.clearTimeout===undefined;"
    ).unwrap().to_text(),"true");
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"setTimeout;").unwrap().to_text(),"27");
}

#[test]
fn timer_wrapping_does_not_make_unrelated_native_properties_replaceable() {
    for source in [
        "window.document=9;",
        "Object.defineProperty(window,'location',{value:9});",
        "window.addEventListener=function(){};",
    ] {
        let (doc,_,page,start)=page(&[source]);
        assert_eq!(start.errors,1,"{source}: {}",page.report());
        assert_eq!(doc.title,"waiting");
        assert!(page.state.timers.is_empty());
    }
    let (_,_,page,start)=page(&["if(typeof setInterval!=='undefined'||typeof clearInterval!=='undefined')throw 'fake API';"]);
    assert_eq!(start.errors,0,"{}",page.report());
}

#[test]
fn timer_wrappers_cannot_bypass_scheduler_limits_or_enable_string_timers() {
    let (_,_,page,start)=page_with_policy(&[r#"
var original=setTimeout;
setTimeout=function(){return original.apply(window,arguments);};
for(var n=0;n<65;n++)setTimeout(function(){},60000);
"#], crate::javascript::limits::CAPACITY_REGRESSION);
    assert_eq!(start.errors,1,"{}",page.report());
    assert!(page.report().contains("Timer limit reached (64"));
    assert_eq!(page.state.timers.len(),64);
    let (doc,_,page,start)=self::page(&[r#"
var original=setTimeout;
window.setTimeout=function(){return original.apply(window,arguments);};
try{setTimeout('document.title="bad";',0);}catch(e){document.title='caught';}finally{document.title='finally';}
"#]);
    assert_eq!(start.errors,1,"{}",page.report());
    assert!(page.report().contains("string/eval timers are not supported"));
    assert_eq!(doc.title,"waiting");
    assert!(page.state.timers.is_empty());
}

#[test]
fn saved_timer_capability_is_denied_when_owning_document_is_disabled() {
    let (mut doc,mut forms,mut page,start)=page(&[]);
    assert_eq!(start.errors,0);
    let function=execute(&mut page,&mut doc,&mut forms,"setTimeout;").unwrap();
    let callback=execute(&mut page,&mut doc,&mut forms,"(function(){document.title='wrong';});").unwrap();
    doc.scripting_enabled=false;
    let error=page.runtime.invoke(function,&[callback,Value::Number(0.0)],Value::Host(WINDOW),
        &mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
    assert!(error.contains("scripting is disabled"),"{error}");
    assert!(page.state.timers.is_empty());
    assert_eq!(doc.title,"waiting");
}

#[test]
fn timer_wrappers_keep_fatal_microtask_cancellation_and_fresh_job_recovery() {
    let (mut doc,mut forms,mut page,start)=page(&[
        "var original=setTimeout;setTimeout=function(){return original.apply(window,arguments);};",
    ]);
    assert_eq!(start.errors,0,"{}",page.report());
    page.begin_job();
    page.runtime.execute(
        "queueMicrotask(function(){document.title='wrong';});",
        &mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();
    assert!(page.runtime.has_microtasks());
    page.runtime.constrain_job(200,usize::MAX,usize::MAX,Instant::now()+Duration::from_secs(1));
    let error=page.runtime.execute(
        "while(true){}",
        &mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err();
    assert!(error.contains("Execution step limit"),"{error}");
    assert!(!page.runtime.has_microtasks());
    page.microtask_checkpoint(&mut doc,&mut forms);
    assert_eq!(doc.title,"waiting");
    execute(&mut page,&mut doc,&mut forms,"setTimeout(function(){document.title='recovered';},0);").unwrap();
    poll(&mut page,&mut doc,&mut forms);
    assert_eq!(doc.title,"recovered");
}

#[test]
fn timer_deletion_uses_ordinary_inherited_lookup_without_hidden_capabilities() {
    let (_,_,page,start)=page(&[r#"
var saved=setTimeout;
delete window.setTimeout;
Object.prototype.setTimeout=function(){return 'inherited';};
if(!('setTimeout' in window)||Object.hasOwn(window,'setTimeout')||setTimeout()!=='inherited'||window.setTimeout!==setTimeout)throw 'inherited lookup';
if(!(delete setTimeout)||setTimeout()!=='inherited')throw 'inherited deletion';
setTimeout=7;
if(setTimeout!==7||window.setTimeout!==7||!Object.hasOwn(window,'setTimeout'))throw 'inherited assignment';
delete window.setTimeout;delete Object.prototype.setTimeout;
if(typeof setTimeout!=='undefined'||'setTimeout' in window)throw 'hidden restore';
Object.defineProperty(window,'setTimeout',{value:saved,writable:true,enumerable:true,configurable:false});
"#, "document.title='must not run';let setTimeout=1;"]);
    assert_eq!(start.errors,1,"{}",page.report());
    assert!(page.report().contains("Lexical binding conflicts with global property"));
    assert!(page.state.timers.is_empty());
}

#[test]
fn timer_utf16_arguments_keep_callback_and_value_identities_and_release_on_fire_and_cancel() {
    let (mut doc,mut forms,mut page,start)=page(&[r#"
var raw='\udb60Z',token={},marker=Symbol('original timer'),called=0;
function receive(unit,text,object,symbol,number,boolean){
 if(this!==window||unit!==raw||unit.length!==2||unit.charCodeAt(0)!==0xdb60||unit.charCodeAt(1)!==90||
    text!=='plain'||object!==token||symbol!==marker||number!==17||boolean!==true)throw 'genuine timer arguments';
 called++;document.title='timer argument identities preserved';
}
var first=setTimeout(receive,0,raw,'plain',token,marker,17,true);
"#]);
    assert_eq!(start.errors,0,"{}",page.report());
    let expected=6*std::mem::size_of::<Value>()+4+5;
    assert_eq!(page.state.timer_bytes,expected);
    assert_eq!(page.state.timers.len(),1);
    assert_eq!(page.state.timers[0].bytes,expected);
    let callback=execute(&mut page,&mut doc,&mut forms,"receive;").unwrap();
    assert!(matches!((&page.state.timers[0].callback,&callback),(Value::Function(a),Value::Function(b)) if a==b));
    assert!(matches!(page.state.timers[0].arguments.first(),Some(Value::StringUnits(units)) if units==&[0xdb60,90]));
    poll(&mut page,&mut doc,&mut forms);
    assert_eq!(doc.title,"timer argument identities preserved");
    assert!(page.state.timers.is_empty());assert_eq!(page.state.timer_bytes,0);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"called;").unwrap().to_number(),1.0);
    execute(&mut page,&mut doc,&mut forms,"var cancelId=setTimeout(receive,10000,raw,'plain',token,marker,17,true);").unwrap();
    assert_eq!(page.state.timer_bytes,expected);
    execute(&mut page,&mut doc,&mut forms,"clearTimeout(cancelId);").unwrap();
    assert!(page.state.timers.is_empty());assert_eq!(page.state.timer_bytes,0);
    assert_eq!(page.state.timer_callbacks,1);
}

#[test]
fn timer_utf16_arguments_native_payload_and_copy_boundaries_are_exact_and_atomic() {
    let (mut doc,mut forms,mut page,start)=page(&["function callback(){}"]);
    assert_eq!(start.errors,0,"{}",page.report());
    let callback=execute(&mut page,&mut doc,&mut forms,"callback;").unwrap();
    let arguments=[callback.clone(),Value::Number(0.0),Value::StringUnits(vec![0xd800,0xd800,0xd800]),Value::String("ok".into())];
    let expected=2*std::mem::size_of::<Value>()+6+2;
    page.state.limits.host.timer_bytes=expected-1;
    page.state.copy_work=expected;
    let next=page.state.next_timer;
    assert_eq!(page.state.set_timeout(&arguments).unwrap_err(),"Timer argument memory limit reached (256 KiB per document).");
    assert_eq!(page.state.copy_work,expected);
    assert!(page.state.timers.is_empty());assert_eq!(page.state.timer_bytes,0);assert_eq!(page.state.next_timer,next);
    page.state.limits.host.timer_bytes=expected;
    page.state.copy_work=expected-1;
    let error=page.state.set_timeout(&arguments).unwrap_err();
    assert_eq!(error,format!("DOM string work limit reached ({} MiB per job).",page.state.limits.copy_bytes/(1024*1024)));
    assert_eq!(page.state.copy_work,0);
    assert!(page.state.timers.is_empty());assert_eq!(page.state.timer_bytes,0);assert_eq!(page.state.next_timer,next);
    page.state.copy_work=expected;
    let id=page.state.set_timeout(&arguments).unwrap();
    assert_eq!(page.state.copy_work,0);assert_eq!(page.state.timer_bytes,expected);
    assert_eq!(page.state.timers[0].bytes,expected);
    assert!(matches!(page.state.timers[0].arguments.first(),Some(Value::StringUnits(units)) if units==&[0xd800;3]));
    page.state.clear_timeout(&[id]).unwrap();
    assert!(page.state.timers.is_empty());assert_eq!(page.state.timer_bytes,0);assert_eq!(page.state.copy_work,0);
    // The same exact native allowance also admits ordinary scalar payloads.
    let normal=[callback,Value::Number(0.0),Value::String("plain".into()),Value::String("xyz".into())];
    page.state.copy_work=expected;
    let id=page.state.set_timeout(&normal).unwrap();
    assert_eq!(page.state.copy_work,0);assert_eq!(page.state.timer_bytes,expected);
    page.state.clear_timeout(&[id]).unwrap();
    assert!(page.state.timers.is_empty());assert_eq!(page.state.timer_bytes,0);
}

#[test]
fn timer_utf16_arguments_public_quota_refusal_preserves_an_existing_native_callback() {
    let (mut doc,mut forms,mut page,start)=page(&[r#"
var count=0;
function callback(value){if(value!=='safe')throw 'unchanged ordinary payload';count++;document.title='existing callback survived';}
var original=setTimeout(callback,0,'safe');
"#]);
    assert_eq!(start.errors,0,"{}",page.report());
    let kept=page.state.timer_bytes;let next=page.state.next_timer;
    page.state.limits.host.timer_bytes=kept+std::mem::size_of::<Value>()+2;
    let error=execute(&mut page,&mut doc,&mut forms,r#"setTimeout(callback,0,'\ud800xx');"#).unwrap_err();
    assert!(error.contains("Timer argument memory limit reached (256 KiB per document)."),"{error}");
    assert_eq!(page.state.timers.len(),1);assert_eq!(page.state.timer_bytes,kept);assert_eq!(page.state.next_timer,next);
    assert!(matches!(page.state.timers[0].arguments.as_slice(),[Value::String(text)] if text=="safe"));
    assert_eq!(doc.title,"waiting");
    poll(&mut page,&mut doc,&mut forms);
    assert_eq!(doc.title,"existing callback survived");assert!(page.state.timers.is_empty());assert_eq!(page.state.timer_bytes,0);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"count;").unwrap().to_number(),1.0);
}
