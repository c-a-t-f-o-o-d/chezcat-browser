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
