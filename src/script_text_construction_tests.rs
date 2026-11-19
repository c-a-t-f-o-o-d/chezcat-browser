//! Main Text construction, interface inheritance and Slottable getter behavior.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<!doctype html><body><span id=element>Parsed</span><form id=form><input id=field name=assignedSlot value=kept></form>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = crate::navigation::address(
        "https://text-construction.test/main",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&source);
    page.set_resource_base(&source);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}

#[test]
fn text_construction_original_canonical_main_interface_defaults_and_genuine_nodes() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var T=Text,P=T.prototype,parsed=document.getElementById('element').firstChild,a=new T(),b=new T(undefined),c=new T(null),made=document.createTextNode('Factory');
if(typeof T!=='function'||T.name!=='Text'||T.length!==0||Object.getPrototypeOf(T)!==CharacterData||Object.getPrototypeOf(P)!==CharacterData.prototype||P.constructor!==T)throw 'canonical Text interface ancestry';
if(Object.getPrototypeOf(a)!==P||Object.getPrototypeOf(parsed)!==P||Object.getPrototypeOf(made)!==P||!(a instanceof T)||!(a instanceof CharacterData)||!(a instanceof Node)||a instanceof Element)throw 'genuine constructor/factory/parsed Text identity';
if(a===b||b===c||a.data!==''||b.data!==''||c.data!=='null'||c.nodeValue!=='null'||c.textContent!=='null'||made.data!=='Factory')throw 'optional undefined default versus null DOMString';
for(var node of [a,b,c,made]){if(node.nodeType!==3||node.nodeName!=='#text'||node.ownerDocument!==document||node.parentNode!==null||node.isConnected||node.assignedSlot!==null)throw 'real detached Text facts';}
document.body.appendChild(c);if(c.parentNode!==document.body||!c.isConnected||c.ownerDocument!==document||c.data!=='null')throw 'real created node attachment';
"#,
    );
}

#[test]
fn text_construction_original_domstring_order_abrupt_no_node_and_existing_string_limit() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var T=Text,order='',primitive={};primitive[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'wrong primitive hint';order+='S';return 37;};
var a=new T({toString:function(){order+='T';return 'chosen';},valueOf:function(){throw 'string hint skipped toString';}}),b=new T({toString:function(){order+='t';return {};},valueOf:function(){order+='v';return 9;}}),c=new T(primitive),extra=new T('kept',{toString:function(){throw 'extra argument converted';}});
if(a.data!=='chosen'||b.data!=='9'||c.data!=='37'||extra.data!=='kept'||order!=='TtvS')throw 'normal DOMString primitive precedence';
var hits=0,marker={},failed={toString:function(){hits++;throw marker;}};
"#,
    );
    run(&mut doc,&mut forms,&mut page,"var undefinedHits=0,convertedUndefined=new T({toString:function(){undefinedHits++;return undefined;}});if(convertedUndefined.data!=='undefined'||undefinedHits!==1)throw 'object result undefined is a DOMString, not the optional argument default';");
    let before = (
        doc.nodes.len(),
        page.state.dom.created.len(),
        page.state.retained,
        page.state.dom_commits,
    );
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var caught=false;try{new T(failed);}catch(e){caught=e===marker;}if(!caught||hits!==1)throw 'same abrupt conversion value';
try{new T(Symbol());throw 'Symbol converted';}catch(e){if(!(e instanceof TypeError))throw e;}
try{T(failed);throw 'Text called without new';}catch(e){if(!(e instanceof TypeError))throw e;}if(hits!==1)throw 'constructor-required failure converted data';
"#,
    );
    assert_eq!(
        (
            doc.nodes.len(),
            page.state.dom.created.len(),
            page.state.retained,
            page.state.dom_commits
        ),
        before,
        "failed conversions published a native node"
    );
    let limit = page.state.limits.host.string_bytes;
    page.state.limits.host.string_bytes = 8;
    let out = page.console_eval(
        &mut doc,
        &mut forms,
        "new T({toString:function(){hits++;return '123456789';}});",
    );
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(
        (
            doc.nodes.len(),
            page.state.dom.created.len(),
            page.state.retained,
            page.state.dom_commits
        ),
        before,
        "over-limit conversion published a native node"
    );
    page.state.limits.host.string_bytes = limit;
    run(&mut doc,&mut forms,&mut page,"if(hits!==2||new T('recovered').data!=='recovered'||a.data!=='chosen')throw 'bounded constructor did not recover';");
}

#[test]
fn text_construction_original_distinct_slottable_getter_brands_mutation_and_named_form() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var P=Text.prototype,D=Object.getOwnPropertyDescriptor(P,'assignedSlot'),E=Object.getOwnPropertyDescriptor(Element.prototype,'assignedSlot'),text=new Text('kept'),element=document.getElementById('element'),comment=document.createComment('kept'),fragment=document.createDocumentFragment();
if(!D||typeof D.get!=='function'||D.get===E.get||D.get.name!=='get assignedSlot'||D.get.length!==0||D.set!==undefined||!D.enumerable||!D.configurable||D.value!==undefined||Object.getOwnPropertyDescriptor(CharacterData.prototype,'assignedSlot')!==undefined)throw 'distinct readonly Text mixin descriptor';
if(D.get.call(text)!==null||E.get.call(element)!==null)throw 'correct concrete getter brands';
var rejected=0;for(var receiver of [null,undefined,{},P,element,comment,fragment,document]){try{D.get.call(receiver);}catch(e){if(e instanceof TypeError)rejected++;else throw e;}}if(rejected!==8)throw 'Text getter accepted non-Text';
try{E.get.call(text);throw 'Element getter accepted Text';}catch(e){if(!(e instanceof TypeError))throw e;}
var hits=0;Object.defineProperty(P,'assignedSlot',{configurable:true,get:function(){hits++;return 'author';}});
if(text.assignedSlot!=='author'||element.firstChild.assignedSlot!=='author'||hits!==2||D.get.call(text)!==null||element.assignedSlot!==null||comment.assignedSlot!==undefined)throw 'prototype override changed native brand or unrelated Nodes';
delete P.assignedSlot;if('assignedSlot' in text||text.assignedSlot!==undefined||element.assignedSlot!==null)throw 'deleted Text getter ghost';Object.defineProperty(P,'assignedSlot',D);
if(text.assignedSlot!==null||Object.hasOwn(text,'assignedSlot'))throw 'descriptor restoration';
var form=document.getElementById('form'),field=document.getElementById('field');if(form.assignedSlot!==field||field.form!==form||E.get.call(form)!==null||field.value!=='kept')throw 'legacy form named-control precedence';
"#,
    );
}

#[test]
fn text_construction_replacement_setters_keep_receiver_value_and_native_state() {
    let mut doc=crate::html::parse("<!doctype html><body><span id=span>Parsed</span><slot id=slot name=chosen><span id=fallback>Fallback</span></slot><form id=owner><input id=neighbor value=original></form>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = crate::navigation::address(
        "https://text-construction.test/setters",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&source);
    page.set_resource_base(&source);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let find = |name: &str| {
        (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some(name))
            .unwrap()
    };
    let (neighbor, owner, slot) = (find("neighbor"), find("owner"), find("slot"));
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    let native = |forms: &Forms| {
        let c = &forms.controls[&neighbor];
        (
            c.value.text(),
            c.value.cursor,
            c.value.anchor,
            c.value.offset,
            c.dirty_value,
            c.owner,
        )
    };
    let before = native(&forms);
    assert_eq!(before.5, Some(owner));
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var span=document.getElementById('span'),slot=document.getElementById('slot'),text=span.firstChild,TP=Text.prototype,EP=Element.prototype,TD=Object.getOwnPropertyDescriptor(TP,'assignedSlot'),ED=Object.getOwnPropertyDescriptor(EP,'assignedSlot');
if(text.nodeType!==3||text.data!=='Parsed'||text.ownerDocument!==document||slot.nodeName!=='SLOT'||slot.getAttribute('name')!=='chosen'||text.assignedSlot!==null||slot.assignedSlot!==null)throw 'actual Text/light SLOT positives';
var value={kept:37},textCalls=0,slotCalls=0,textReceiver,textValue,slotReceiver,slotValue;
Object.defineProperty(TP,'assignedSlot',{configurable:true,set:function(input){textCalls++;textReceiver=this;textValue=input;}});
Object.defineProperty(EP,'assignedSlot',{configurable:true,set:function(input){slotCalls++;slotReceiver=this;slotValue=input;}});
text.assignedSlot=value;
if(textCalls!==1||textReceiver!==text||textValue!==value||Object.hasOwn(text,'assignedSlot'))throw 'actual Text replacement setter bypassed';
slot.assignedSlot=value;
if(slotCalls!==1||slotReceiver!==slot||slotValue!==value||Object.hasOwn(slot,'assignedSlot'))throw 'ordinary SLOT replacement setter bypassed or created own value';
if(Object.getOwnPropertyDescriptor(TP,'assignedSlot').get!==TD.get||Object.getOwnPropertyDescriptor(EP,'assignedSlot').get!==ED.get||text.assignedSlot!==null||slot.assignedSlot!==null)throw 'adding a setter replaced the existing getter';
Object.defineProperty(TP,'assignedSlot',{get:undefined});Object.defineProperty(EP,'assignedSlot',{get:undefined});
if(text.assignedSlot!==undefined||slot.assignedSlot!==undefined)throw 'explicit setter-only descriptor returned a value';
if(TD.get.call(text)!==null||ED.get.call(slot)!==null||text.data!=='Parsed'||slot.getAttribute('name')!=='chosen'||document.getElementById('neighbor').value!=='edited neighbor')throw 'author setter changed native slots/raw/editor';
Object.defineProperty(TP,'assignedSlot',TD);Object.defineProperty(EP,'assignedSlot',ED);
if(text.assignedSlot!==null||slot.assignedSlot!==null||textCalls!==1||slotCalls!==1)throw 'canonical getter restoration';
delete EP.assignedSlot;span.assignedSlot=value;
var own=Object.getOwnPropertyDescriptor(span,'assignedSlot');if(!own||own.value!==value||!own.writable||!own.enumerable||!own.configurable||span.assignedSlot!==value||ED.get.call(span)!==null)throw 'backed SPAN ordinary value after getter deletion';
Object.defineProperty(EP,'assignedSlot',ED);if(span.assignedSlot!==value||ED.get.call(span)!==null)throw 'restored prototype ignored ordinary own value';delete span.assignedSlot;
if(span.assignedSlot!==null||text.assignedSlot!==null||slot.assignedSlot!==null||text.data!=='Parsed'||text.parentNode!==span)throw 'canonical state after own deletion';
"#,
    );
    assert_eq!(native(&forms), before);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    assert_eq!(doc.attr(neighbor, "value"), Some("original"));
    assert_eq!(doc.attr(slot, "name"), Some("chosen"));
    assert_eq!(page.state.dom_commits, commits);
    run(&mut doc, &mut forms, &mut page,
        "delete TP.assignedSlot; Object.defineProperty(EP,'assignedSlot',{value:23,writable:true,configurable:true});");
    let out = page.console_eval(&mut doc, &mut forms, "text.assignedSlot=41;");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert!(page.report().contains("own data property"), "{}", page.report());
    run(&mut doc, &mut forms, &mut page, r#"
slot.assignedSlot=41;
var slotOwn=Object.getOwnPropertyDescriptor(slot,'assignedSlot');
if(!slotOwn||slotOwn.value!==41||!slotOwn.writable||!slotOwn.enumerable||!slotOwn.configurable||
   Object.hasOwn(text,'assignedSlot')||Object.hasOwn(TP,'assignedSlot')||EP.assignedSlot!==23)throw 'SLOT ordinary write changed native Text or interfaces';
delete slot.assignedSlot;
Object.defineProperty(TP,'assignedSlot',TD);Object.defineProperty(EP,'assignedSlot',ED);
if(slot.assignedSlot!==null||text.assignedSlot!==null)throw 'native getter restoration';
"#);
    run(&mut doc, &mut forms, &mut page, "if(text.unrelatedTextProperty!==undefined)throw 'unexpected Text field';");
    let out = page.console_eval(&mut doc, &mut forms, "text.unrelatedTextProperty=17;");
    assert_eq!(out.errors, 1, "{}", page.report());
    run(&mut doc,&mut forms,&mut page,"if(text.unrelatedTextProperty!==undefined||text.assignedSlot!==null||slot.assignedSlot!==null||text.data!=='Parsed'||document.getElementById('neighbor').value!=='edited neighbor')throw 'unrelated Text property refusal changed native state';");
    assert_eq!(native(&forms), before);
    assert_eq!(page.state.dom_commits, commits);
}
