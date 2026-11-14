//! Original browser-host regressions; Node cannot validate this native scheduler.
use super::*;

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
        "window.requestAnimationFrame=function(){};",
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
