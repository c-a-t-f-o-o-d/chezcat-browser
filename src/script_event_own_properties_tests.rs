//! Original genuine dispatched Event property controls; no framework markers.
use super::*;

fn fixture(source: &str) -> (Document, Forms, Page) {
    fixture_with_limits(source, crate::javascript::limits::CAPACITY_REGRESSION)
}
fn fixture_with_limits(source: &str, limits: crate::javascript::limits::Limits) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><div id=parent><button id=button type=button>Run</button><a id=anchor href=#target>Original link</a></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(limits);
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/page").unwrap().0),
        path: None, root: None, fragment: String::new(),
    });
    let outcome = page.start(&mut doc, &mut forms, &[Script {
        node: 0, label: "Original dispatched Event properties".into(), source: source.into(),
    }]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    // The App opens a fresh native input job after document startup. Preserve
    // initialization allowances while testing the actual callback job limits.
    page.begin_job();
    (doc, forms, page)
}
fn button(doc: &Document) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("button")).unwrap()
}
fn evaluate(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
}
fn dispatch(page: &mut Page, doc: &mut Document, forms: &mut Forms) -> Outcome {
    let target = button(doc);
    page.dispatch(doc, forms, target, "click", true)
}

#[test]
fn dispatched_base_event_fresh_first_inspection_publishes_real_own_slots() {
    for probe in [
        "var names=Object.keys(e);if(names.length!==1||names[0]!=='isTrusted')throw 'fresh keys';",
        "var names=Object.getOwnPropertyNames(e);if(names.length!==1||names[0]!=='isTrusted')throw 'fresh names';",
        "var all=Object.getOwnPropertyDescriptors(e);if(Object.keys(all).join(',')!=='isTrusted'||all.isTrusted.configurable||!all.isTrusted.enumerable||all.isTrusted.set!==undefined||!all.isTrusted.get.call(e))throw 'fresh descriptors';",
        "if(Object.getOwnPropertySymbols(e).length!==0)throw 'fresh symbols';",
        "var d=Object.getOwnPropertyDescriptor(e,'isTrusted');if(d.configurable||!d.enumerable||d.set!==undefined||!d.get.call(e))throw 'fresh descriptor';",
    ] {
        for mouse in [false, true] {
            let name = if mouse { "anchor" } else { "button" };
            let prototype = if mouse { "MouseEvent" } else { "Event" };
            let source = format!("var b=document.getElementById('{name}');b.onclick=function(e){{{probe}if(Object.getPrototypeOf(e)!=={prototype}.prototype||!e.isTrusted||e.type!=='click'||e.target!==b)throw 'real Event';document.title='fresh-event-ok';}};");
            let (mut doc, mut forms, mut page) = fixture(&source);
            let outcome = if mouse {
                let target = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("anchor")).unwrap();
                page.anchor_mouse(&mut doc, &mut forms, target, AnchorMouseInput::default())
            } else { dispatch(&mut page, &mut doc, &mut forms) };
            assert_eq!(outcome.errors, 0, "{probe}; mouse={mouse}\n{}", page.report());
            assert_eq!(doc.title, "fresh-event-ok");
            assert!(!outcome.prevented);
        }
    }
}

#[test]
fn dispatched_base_event_ordinary_properties_preserve_native_slots_path_and_cancellation() {
    let (mut doc, mut forms, mut page) = fixture(r#"
var b=document.getElementById('button'), saved=null, key=Symbol('original-own'), visits=0;
Event.prototype.inheritedProbe=17;
b.addEventListener('click',function(e){
 if(e.arbitraryMarker!==undefined||e[key]!==undefined||'arbitraryMarker' in e||Object.getOwnPropertyDescriptor(e,'arbitraryMarker')!==undefined)throw 'ordinary absence';
 if(!e.isTrusted||e.target!==b||e.currentTarget!==b||e.eventPhase!==2||!e.cancelable||e.defaultPrevented)throw 'native slots';
 var path=e.composedPath();if(path[0]!==b||path[path.length-1]!==window)throw 'native path';
 if(e.inheritedProbe!==17||!('inheritedProbe' in e)||Object.prototype.hasOwnProperty.call(e,'inheritedProbe'))throw 'prototype lookup';
 e.arbitraryMarker='kept';e[key]=29;
 Object.defineProperty(e,'hiddenNote',{value:23,writable:true,configurable:true});
 var d=Object.getOwnPropertyDescriptor(e,'arbitraryMarker');if(d.value!=='kept'||!d.writable||!d.enumerable||!d.configurable)throw 'own descriptor';
 var trusted=Object.getOwnPropertyDescriptor(e,'isTrusted');
 e.type='spoof';e.isTrusted=false;if(e.type!=='click'||!e.isTrusted)throw 'readonly fields';
 var strict=false;try{(function(){'use strict';e.type='spoof';})();}catch(x){strict=x.name==='TypeError';}if(!strict)throw 'strict readonly';
 var blocked=false;try{Object.defineProperty(e,'isTrusted',{value:false});}catch(x){blocked=x.name==='TypeError';}if(!blocked||!trusted.get.call(e)||delete e.isTrusted)throw 'unforgeable trust';
 var fake=Object.create(Event.prototype), bad=0;
 try{trusted.get.call(fake);}catch(x){if(x.name==='TypeError')bad++;}
 try{Event.prototype.preventDefault.call(fake);}catch(x){if(x.name==='TypeError')bad++;}
 if(bad!==2)throw 'forged brand';
 e.cancelBubble=false;e.returnValue=true;Event.prototype.preventDefault.call(e);
 if(!e.defaultPrevented||e.returnValue||e.cancelBubble||e.target!==b||!e.isTrusted)throw 'real cancellation';
 saved=e;visits++;
});
b.addEventListener('click',function(e){
 if(e!==saved||e.arbitraryMarker!=='kept'||e[key]!==29||e.hiddenNote!==23||!e.defaultPrevented)throw 'same-dispatch persistence';
 if(Object.keys(e).join(',')!=='isTrusted,arbitraryMarker'||Object.getOwnPropertyNames(e).join(',')!=='isTrusted,arbitraryMarker,hiddenNote'||Object.getOwnPropertySymbols(e)[0]!==key)throw 'own key sets';
 if(!delete e.arbitraryMarker||!delete e[key]||!delete e.hiddenNote||e.arbitraryMarker!==undefined||e[key]!==undefined||'arbitraryMarker' in e)throw 'ordinary delete';
 if(!delete e.type||e.type!=='click'||Object.keys(e).join(',')!=='isTrusted')throw 'inherited delete';
 visits++;document.title='ordinary-event-ok';
});
"#);
    let outcome = dispatch(&mut page, &mut doc, &mut forms);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(outcome.prevented);
    assert_eq!(doc.title, "ordinary-event-ok");
    evaluate(&mut page, &mut doc, &mut forms, "if(visits!==2)throw 'both real listeners';");
}

#[test]
fn dispatched_base_event_expiry_never_revives_cached_ordinary_properties() {
    let (mut doc, mut forms, mut page) = fixture("var saved=null,key=Symbol('saved'),count=0;document.getElementById('button').onclick=function(e){if(e.marker!==undefined)throw 'new identity';e.marker=++count;e[key]=count;saved=e;document.title='dispatch '+count;};");
    assert_eq!(dispatch(&mut page, &mut doc, &mut forms).errors, 0, "{}", page.report());
    for source in ["saved.marker;", "saved[key];", "Object.keys(saved);", "Object.getOwnPropertyNames(saved);", "Object.getOwnPropertySymbols(saved);", "Object.getOwnPropertyDescriptor(saved,'marker');", "delete saved.marker;", "saved.marker=99;", "saved.preventDefault();"] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("no longer accessible") || page.report().contains("expired"), "{}", page.report());
    }
    let next = page.state.next_event;
    assert_eq!(dispatch(&mut page, &mut doc, &mut forms).errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dispatch 1", "A fatal expiry fault must suppress later dispatch in that same job: {}", page.report());
    assert_eq!(page.state.next_event, next, "Suppressed dispatch cannot allocate a new identity: {}", page.report());
    page.begin_job();
    assert_eq!(dispatch(&mut page, &mut doc, &mut forms).errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dispatch 2", "{}", page.report());
    assert!(page.state.created_events.is_empty(), "Trusted records cannot enter the constructed registry");
}

#[test]
fn dispatched_base_event_fresh_and_cached_foreign_reads_deny_before_author_getters() {
    for cached in [false, true] {
        let prefix = if cached { "e.marker='cached';" } else { "" };
        let source = format!("var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,read=w.Function('e','return e.probe;'),observed=0;Object.defineProperty(Event.prototype,'probe',{{get:function(){{observed++;return 7;}}}});document.getElementById('button').onclick=function(e){{{prefix}read(e);document.title='foreign leaked';}};");
        let (mut doc, mut forms, mut page) = fixture_with_limits(&source,crate::javascript::limits::BROWSER);
        page.set_location(&crate::navigation::Location {
            url: Some(crate::url::parse("https://other.test/").unwrap().0),
            path: None, root: None, fragment: String::new(),
        });
        let outcome = dispatch(&mut page, &mut doc, &mut forms);
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_ne!(doc.title, "foreign leaked");
        evaluate(&mut page, &mut doc, &mut forms, "if(observed!==0)throw 'foreign getter ran';");
    }
}

#[test]
fn dispatched_base_event_real_store_refusal_is_atomic_and_next_event_recovers() {
    let (mut doc, mut forms, mut page) = fixture("document.getElementById('button').onclick=function(e){if(e.marker!==undefined)throw 'failed store leaked';e.marker=31;if(e.marker!==31||!e.isTrusted)throw 'real store';document.title='event-store-ok';};");
    let next = page.state.next_event;
    let allowance = page.runtime.event_property_allowance_for_test(0);
    assert_eq!(dispatch(&mut page, &mut doc, &mut forms).errors, 1, "{}", page.report());
    assert_ne!(doc.title, "event-store-ok", "{}", page.report());
    assert_eq!(page.state.next_event, next + 1, "Failed publication never reuses a native identity: {}", page.report());
    assert!(page.state.event.is_none());
    page.runtime.event_property_allowance_for_test(allowance);
    assert_eq!(dispatch(&mut page, &mut doc, &mut forms).errors, 0, "{}", page.report());
    assert_ne!(doc.title, "event-store-ok", "Restoring capacity cannot revive a fatal job: {}", page.report());
    assert_eq!(page.state.next_event, next + 1, "Suppressed dispatch cannot allocate a new identity: {}", page.report());
    page.begin_job();
    assert_eq!(dispatch(&mut page, &mut doc, &mut forms).errors, 0, "{}", page.report());
    assert_eq!(doc.title, "event-store-ok", "{}", page.report());
}

#[test]
fn dispatched_base_event_expandos_share_copy_and_instruction_limits() {
    for copy_limit in [false, true] {
        let limits = crate::javascript::limits::REGRESSION;
        let source = if copy_limit {
            "var payload='abcdefgh';while(payload.length<8192)payload=payload+payload;document.getElementById('button').onclick=function(e){for(var i=0;i<2048;i++){e.note=payload;var copied=e.note;}document.title='budget escaped';};"
        } else {
            "document.getElementById('button').onclick=function(e){for(var i=0;i<100000;i++){e.note=i;}document.title='budget escaped';};"
        };
        let (mut doc, mut forms, mut page) = fixture_with_limits(source, limits);
        let outcome = dispatch(&mut page, &mut doc, &mut forms);
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_ne!(doc.title, "budget escaped", "{}", page.report());
        assert!(page.report().to_ascii_lowercase().contains(if copy_limit { "copy" } else { "execution step limit" }), "{}", page.report());
        assert!(page.state.event.is_none());
    }
}

#[test]
fn dispatched_keyboard_has_genuine_ordinary_ancestry_but_forged_handles_still_refuse() {
    let (mut doc, mut forms, mut page) = fixture("var seen=0;document.body.addEventListener('keydown',function(e){if(e.key!=='A'||e.code!=='KeyA'||!e.isTrusted||e.type!=='keydown')throw 'native keyboard fields';seen++;});");
    let outcome = page.keyboard(&mut doc, &mut forms, KeyboardInput {
        down: true, key: "A".into(), code: "KeyA".into(), key_code: 0x41, ..KeyboardInput::default()
    });
    assert_eq!(outcome.errors, 0, "{}", page.report());
    evaluate(&mut page, &mut doc, &mut forms, "if(seen!==1)throw 'native keyboard callback';");
    for probe in ["Object.keys(e);", "Object.getPrototypeOf(e);"] {
        let source = format!("document.body.addEventListener('keydown',function(e){{{probe}document.title='keyboard widened';}});");
        let (mut doc, mut forms, mut page) = fixture(&source);
        let outcome = page.keyboard(&mut doc, &mut forms, KeyboardInput {
            down: true, key: "A".into(), code: "KeyA".into(), key_code: 0x41, ..KeyboardInput::default()
        });
        assert_eq!(outcome.errors, 0, "Genuine KeyboardEvent inspection: {probe}\n{}",page.report());
        assert_eq!(doc.title, "keyboard widened");
    }
    page.runtime.define_global("forgedEvent", Value::Host(EVENT_BASE + 10_000)).unwrap();
    let outcome = page.console_eval(&mut doc, &mut forms, "forgedEvent.arbitraryMarker;");
    assert_eq!(outcome.errors, 1);
    assert!(page.report().contains("expired"), "{}", page.report());
}
