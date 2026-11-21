//! Original FocusEvent identity, conversion, realm and native transition checks.
use super::*;
fn fixture(source: &str) -> (Document, Forms, Page) {
    fixture_page(source,Page::for_test())
}
fn fixture_page(source:&str,mut page:Page)->(Document,Forms,Page){
    let mut doc=crate::html::parse("<html><body><div id=parent><input id=a value=original><textarea id=b>second</textarea></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    page.set_location(
        &crate::navigation::address(
            "https://focus-event.test/control",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(
        page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                label: "Original genuine FocusEvent".into(),
                source: source.into()
            }]
        )
        .errors,
        0,
        "{}",
        page.report()
    );
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(
        page.console_eval(doc, forms, source).errors,
        0,
        "{}",
        page.report()
    );
}
#[test]
fn focus_event_constructor_real_slots_descriptors_and_ordinary_prototypes() {
    fixture(
        r#"
var C=FocusEvent,P=C.prototype,a=document.getElementById('a'),e=new C('focusout',{bubbles:true,cancelable:true,composed:true,view:window,detail:4294967297,relatedTarget:a});
if(C.name!=='FocusEvent'||C.length!==1||Object.getPrototypeOf(C)!==UIEvent||Object.getPrototypeOf(P)!==UIEvent.prototype||Object.getPrototypeOf(e)!==P||e.constructor!==C||!(e instanceof C)||!(e instanceof UIEvent)||!(e instanceof Event)||e instanceof MouseEvent||e instanceof KeyboardEvent||Object.prototype.toString.call(e)!=='[object FocusEvent]')throw 'real focus ancestry';
if(e.relatedTarget!==a||e.view!==window||e.detail!==1||e.which!==0||e.isTrusted||e.target!==null||!e.bubbles||!e.cancelable||!e.composed)throw 'real focus slots';
var d=Object.getOwnPropertyDescriptor(P,'relatedTarget'),get=d.get;if(get.name!=='get relatedTarget'||get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||Object.hasOwn(e,'relatedTarget')||Object.getOwnPropertyDescriptor(e,'isTrusted').configurable)throw 'focus descriptors';
var zero=new C('focus');if(zero.relatedTarget!==null||zero.view!==null||zero.detail||zero.which||zero.bubbles||zero.cancelable||zero.composed||zero.isTrusted)throw 'focus defaults';
e.relatedTarget=null;if(e.relatedTarget!==a)throw 'focus readonly';try{(function(){'use strict';e.relatedTarget=null;})();throw 'strict focus write';}catch(x){if(x.name!=='TypeError')throw x;}
Object.defineProperty(P,'relatedTarget',{get:function(){return 'override';},configurable:true});if(e.relatedTarget!=='override'||get.call(e)!==a)throw 'focus prototype override';delete P.relatedTarget;if(e.relatedTarget!==undefined||'relatedTarget' in e)throw 'focus ghost getter';Object.defineProperty(P,'relatedTarget',d);
delete window.FocusEvent;var saved=new C('saved',{relatedTarget:document});if(saved.relatedTarget!==document||saved.constructor!==C)throw 'saved canonical focus';
"#,
    );
}
#[test]
fn focus_event_ordered_conversion_brands_factory_and_synthetic_native_separation() {
    fixture(
        r#"
var C=FocusEvent,P=C.prototype,a=document.getElementById('a'),b=document.getElementById('b'),trace='',arg={};
for(var name of ['bubbles','cancelable','composed','detail','view','relatedTarget']){(function(key){Object.defineProperty(arg,key,{get:function(){trace+=key+'|';if(key==='detail')return {valueOf:function(){trace+='number|';return -1;}};if(key==='view')return window;if(key==='relatedTarget')return a;return true;}});})(name);}
var e=new C({toString:function(){trace+='type|';return 'focusout';}},arg);if(trace!=='type|bubbles|cancelable|composed|detail|number|view|relatedTarget|'||e.detail!==-1||e.relatedTarget!==a)throw 'ordered focus conversion';
var get=Object.getOwnPropertyDescriptor(P,'relatedTarget').get,count=0,request={toString:function(){count++;return 'focus';}};
for(var value of [null,undefined,{},Object.create(P),new Event('e'),new UIEvent('u'),new MouseEvent('m'),new KeyboardEvent('k')]){try{get.call(value);throw 'focus bad brand';}catch(x){if(x.name!=='TypeError')throw x;}}
try{C(request);throw 'focus without new';}catch(x){if(x.name!=='TypeError')throw x;}if(count)throw 'focus call converted';
for(var target of [false,42,'',{},Object.create(EventTarget.prototype),new Event('bad'),a.style]){try{new C('bad',{relatedTarget:target});throw 'focus duck target';}catch(x){if(x.name!=='TypeError')throw x;}}
for(var target of [window,document,a,a.firstChild,document.createTextNode('text'),new EventTarget()]){var f=new C('probe',{relatedTarget:target});if(f.relatedTarget!==target)throw 'real related target';}
var marker={},seen=0;try{new C('bad',{get relatedTarget(){throw marker;}});}catch(x){if(x===marker)seen++;}try{new C();}catch(x){if(x.name==='TypeError')seen++;}try{new C('bad',{detail:Symbol()});}catch(x){if(x.name==='TypeError')seen++;}if(seen!==3)throw 'focus errors';
var f=document.createEvent('fOcUsEvEnT');if(!(f instanceof C)||f.type!==''||f.relatedTarget!==null||f.view!==null||f.isTrusted||'initFocusEvent' in f)throw 'focus factory';f.initUIEvent('focusout',true,true,window,7);
var calls=0;document.getElementById('parent').addEventListener('focusout',function(event){calls++;if(event!==f||event.isTrusted||event.target!==b||event.relatedTarget!==null||event.eventPhase!==3)throw 'synthetic focus';event.preventDefault();});
if(b.dispatchEvent(f)!==false||calls!==1||!f.defaultPrevented||f.currentTarget!==null||f.eventPhase||a.value!=='original'||b.value!=='second')throw 'synthetic native focus separation';
"#,
    );
}
#[test]
fn focus_event_native_transition_flags_targets_bubbling_expiry_and_failure_recovery() {
    let (mut doc, mut forms, mut page) = fixture(
        r#"
var a=document.getElementById('a'),b=document.getElementById('b'),log='',saved,get=Object.getOwnPropertyDescriptor(FocusEvent.prototype,'relatedTarget').get;
for(var name of ['blur','focusout','focus','focusin']){(function(type){document.addEventListener(type,function(e){saved=e;log+=type+':'+e.target.id+':'+(e.relatedTarget?e.relatedTarget.id:'null')+';';if(!(e instanceof FocusEvent)||!(e instanceof UIEvent)||!e.isTrusted||e.view!==window||e.detail||e.which||e.cancelable||!e.composed||e.bubbles!==(type==='focusin'||type==='focusout')||e.relatedTarget!==(e.target===a?b:a))throw 'native focus slots';e.preventDefault();if(e.defaultPrevented)throw 'uncancelable focus';},true);})(name);}
"#,
    );
    let a = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("a"))
        .unwrap();
    let b = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("b"))
        .unwrap();
    page.begin_job();
    let out = page.focus_transition(&mut doc, &mut forms, Some(a), Some(b));
    assert_eq!(out.errors, 0, "{}", page.report());
    assert!(!out.prevented);
    assert!(page.state.event.is_none());
    run(&mut doc,&mut forms,&mut page,"if(log!=='blur:a:b;focusout:a:b;focus:b:a;focusin:b:a;')throw log;var denied=false;try{get.call(saved);}catch(x){denied=x.name==='TypeError';}if(!denied)throw 'expired focus';");
    page.begin_job();
    page.state.copy_work = 0;
    let before = page.state.retained;
    let out = page.focus_transition(&mut doc, &mut forms, Some(b), Some(a));
    assert!(out.errors > 0);
    assert!(page.state.event.is_none());
    assert_eq!(page.state.retained, before);
    page.begin_job();
    let out = page.focus_transition(&mut doc, &mut forms, Some(a), Some(b));
    assert_eq!(out.errors, 0, "{}", page.report());
}
#[test]
fn focus_event_child_realm_genuine_targets_and_saved_owner() {
    let (mut doc, mut forms, mut page) = fixture_page(
        r#"
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow,C=w.FocusEvent;
var e=new C('focus',{view:w,relatedTarget:w.document.body});if(C===FocusEvent||Object.getPrototypeOf(C)!==w.UIEvent||Object.getPrototypeOf(C.prototype)!==w.UIEvent.prototype||Object.getPrototypeOf(e)!==C.prototype||e.relatedTarget!==w.document.body||e.view!==w||e instanceof FocusEvent||!(e instanceof w.UIEvent)||Object.getOwnPropertyDescriptor(e,'isTrusted').configurable)throw 'focus owner realm';
var main=new FocusEvent('focus',{relatedTarget:w.document.body});if(main.relatedTarget!==w.document.body)throw 'same origin focus target';
"#,
        Page::with_limits(crate::javascript::limits::BROWSER),
    );
    run(&mut doc,&mut forms,&mut page,"var e=new FocusEvent('x',{relatedTarget:undefined});if(e.relatedTarget!==null)throw 'undefined focus target';");
    page.set_location(
        &crate::navigation::address(
            "https://other-focus.test/",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    run(&mut doc,&mut forms,&mut page,"var denied=0;try{new FocusEvent('denied',{relatedTarget:w});}catch(x){if(x.name==='SecurityError')denied++;}if(denied!==1)throw 'focus owner origin changed';");
    // Saved foreign native constructor access uses the existing fatal origin
    // guard, rather than weakening that guard into a catchable constructor error.
    assert!(page.runtime.execute("new C('denied');",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap_err().contains("Foreign Window access"));

}

#[test]
fn focus_event_constructor_copy_and_retained_refusal_are_atomic_and_recover() {
    let (mut doc, mut forms, mut page) = fixture("");
    let baseline = page.state.retained;
    let limit = page.state.retained_limit;
    let next = page.state.next_event;
    let fields = [("relatedTarget", Value::Host(DOCUMENT))];
    page.state.retained_limit =
        baseline + std::mem::size_of::<script_created_events::CreatedEvent>();
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .construct_focus_event(WINDOW, &[Value::String("x".into())], Some(&fields))
    .is_err());
    assert_eq!(
        (
            page.state.retained,
            page.state.next_event,
            page.state.created_events.capacity()
        ),
        (baseline, next, 0)
    );
    page.state.retained_limit = limit;
    page.begin_job();
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .construct_focus_event(WINDOW, &[Value::String("x".into())], Some(&fields))
    .is_err());
    assert_eq!(
        (
            page.state.retained,
            page.state.next_event,
            page.state.created_events.capacity()
        ),
        (baseline, next, 0)
    );
    page.begin_job();
    let Value::Host(handle) = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .construct_focus_event(WINDOW, &[Value::String("x".into())], Some(&fields))
    .unwrap() else {
        panic!("native focus")
    };
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .focus_receiver(handle));
    Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .rollback_base_event(handle)
    .unwrap();
    assert_eq!(page.state.retained, baseline);
    assert_eq!(page.state.created_events.capacity(), 0);
    assert_eq!(page.state.next_event, next + 1);
    // Runtime wrapper admission failure must also undo the newly made record.
    let allowance = page.runtime.event_property_allowance_for_test(0);
    assert!(page
        .runtime
        .execute(
            "new FocusEvent('wrapper',{relatedTarget:document});",
            &mut Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state
            }
        )
        .is_err());
    page.runtime.event_property_allowance_for_test(allowance);
    assert!(page.state.created_events.is_empty());
    assert_eq!(page.state.created_events.capacity(), 0);
    assert_eq!(page.state.retained, baseline);
    run(&mut doc,&mut forms,&mut page,"var f=new FocusEvent('recovered',{relatedTarget:document});if(f.relatedTarget!==document||f.isTrusted)throw 'focus recovery';");
}
#[test]
fn focus_event_native_callback_mutation_drops_disconnected_targets_without_resurrection() {
    let (mut doc, mut forms, mut page) = fixture(
        r#"
var a=document.getElementById('a'),b=document.getElementById('b'),log='';
a.onblur=function(e){if(!(e instanceof FocusEvent)||e.relatedTarget!==b)throw 'real blur handler';log+='blur;';b.parentNode.removeChild(b);};
a.addEventListener('focusout',function(e){if(e.relatedTarget!==null)throw 'removed related target';log+='out;';});
b.addEventListener('focus',function(){throw 'disconnected native focus';});
"#,
    );
    let a = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("a"))
        .unwrap();
    let b = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("b"))
        .unwrap();
    page.begin_job();
    let out = page.focus_transition(&mut doc, &mut forms, Some(a), Some(b));
    assert_eq!(out.errors, 0, "{}", page.report());
    assert!(page.state.event.is_none());
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if(log!=='blur;out;'||b.isConnected)throw 'focus callback mutation '+log+' connected='+b.isConnected;",
    );
}
