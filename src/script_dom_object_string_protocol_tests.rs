//! Bounded ordinary-property and owner controls for the concrete BUTTON/UL slice.
//! These derive real prototypes from real elements; no new global constructor
//! API, child DOM storage, implicit URL stringifier or framework shim is assumed.
use super::*;

fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
}
fn id(doc: &Document, name: &str) -> usize {
    doc.nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id==name))
        .unwrap()
}
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Original DOM string protocol</title><body>",
        "<form id=form><button id=button type=button>Original button</button>",
        "<input id=neighbor name=keep value=original></form>",
        "<ul id=list><li>Original item</li></ul><div id=opaque>Retained opaque element</div>",
        "<a id=anchor href='../destination?q=one#part'>Original link</a>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = location("https://dom-string.test/path/page");
    page.set_location(&source);
    page.set_resource_base(&source);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    run(&mut doc,&mut forms,&mut page,"var button=document.getElementById('button'),list=document.getElementById('list'),neighbor=document.getElementById('neighbor'),stringify=Object.prototype.toString,tag=Symbol.toStringTag;");
    (doc, forms, page)
}
fn kept(doc: &Document, forms: &Forms, page: &mut Page) {
    let node = id(doc, "neighbor");
    let c = &forms.controls[&node];
    assert_eq!(
        (
            c.value.text(),
            c.value.cursor,
            c.value.anchor,
            c.value.offset,
            c.dirty_value,
            c.owner
        ),
        (
            "edited neighbor".into(),
            5,
            2,
            1,
            true,
            Some(id(doc, "form"))
        )
    );
    assert_eq!(forms.text_default_value(node).unwrap(), "original");
    assert_eq!(doc.attr(node, "value"), Some("original"));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    kept(doc, forms, page);
}

#[test]
fn dom_object_string_protocol_concrete_prototypes_tag_descriptors_and_ordinary_overrides() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var BP=Object.getPrototypeOf(button),UP=Object.getPrototypeOf(list),BD=Object.getOwnPropertyDescriptor(BP,tag),UD=Object.getOwnPropertyDescriptor(UP,tag),OD=Object.getOwnPropertyDescriptor(Object.prototype,'toString');
if(BP===UP||Object.getPrototypeOf(BP)!==HTMLElement.prototype||Object.getPrototypeOf(UP)!==HTMLElement.prototype)throw 'distinct real concrete prototype suffixes';
if(HTMLButtonElement.prototype!==BP||HTMLUListElement.prototype!==UP||button.constructor!==HTMLButtonElement||list.constructor!==HTMLUListElement)throw 'concrete constructor identity';
if(!(button instanceof HTMLButtonElement)||list instanceof HTMLButtonElement||!(list instanceof HTMLUListElement))throw 'concrete instance identity';
for(var C of [HTMLButtonElement,HTMLUListElement]){try{new C();throw 'illegal construction succeeded';}catch(e){if(!(e instanceof TypeError))throw e;}}
for(var pair of [[BD,'HTMLButtonElement'],[UD,'HTMLUListElement']]){var d=pair[0];if(!d||d.value!==pair[1]||d.writable||d.enumerable||!d.configurable||d.get!==undefined||d.set!==undefined)throw 'standard concrete tag descriptor';}
if(Object.getOwnPropertyDescriptor(BP,'toString')!==undefined||Object.getOwnPropertyDescriptor(UP,'toString')!==undefined||Object.hasOwn(button,'toString')||Object.hasOwn(list,'toString'))throw 'Object stringifier must be inherited';
if(typeof stringify!=='function'||stringify.name!=='toString'||stringify.length!==0||Object.getPrototypeOf(stringify)!==Function.prototype||OD.value!==stringify||!OD.writable||OD.enumerable||!OD.configurable)throw 'ordinary canonical Object method';
if(button.toString!==stringify||list.toString!==stringify||button[tag]!=='HTMLButtonElement'||list[tag]!=='HTMLUListElement')throw 'canonical method/tag identity';
button[tag]='ignored';list[tag]='ignored';if(button[tag]!==BD.value||list[tag]!==UD.value||Object.hasOwn(button,tag)||Object.hasOwn(list,tag))throw 'inherited readonly tags';
var own=function(){return this===button?'own-button':'wrong';};button.toString=own;
if(button.toString()!=='own-button'||Object.getOwnPropertyDescriptor(button,'toString').value!==own||list.toString!==stringify||stringify.call(button)!=='[object HTMLButtonElement]')throw 'ordinary own stringifier shadow';
delete button.toString;if(button.toString!==stringify||!delete button.toString||button.toString()!=='[object HTMLButtonElement]')throw 'own deletion restores actual inheritance';
BP.toString=function(){return this===button?'prototype-button':'wrong';};if(button.toString()!=='prototype-button'||list.toString!==stringify)throw 'concrete prototype override leaked';
delete BP.toString;if(button.toString!==stringify)throw 'concrete prototype deletion bypassed Object method';
var tagReads=0;Object.defineProperty(button,tag,{configurable:true,get:function(){tagReads++;return 'AuthorButton';}});
if(button.toString()!=='[object AuthorButton]'||stringify.call(button)!=='[object AuthorButton]'||tagReads!==2||list.toString()!=='[object HTMLUListElement]')throw 'observable own tag getter';
delete button[tag];if(button.toString()!=='[object HTMLButtonElement]')throw 'own tag deletion';
delete BP[tag];if(button.toString()!=='[object HTMLElement]'||button[tag]!=='HTMLElement'||list[tag]!=='HTMLUListElement')throw 'concrete tag deletion must expose parent tag';
Object.defineProperty(BP,tag,BD);if(button.toString()!=='[object HTMLButtonElement]')throw 'concrete descriptor restoration';
var marker={},caught=false;Object.defineProperty(list,tag,{configurable:true,get:function(){throw marker;}});
try{stringify.call(list);}catch(e){caught=e===marker;}if(!caught)throw 'tag getter abrupt value changed';delete list[tag];
try{new stringify();throw 'Object method constructed';}catch(e){if(!(e instanceof TypeError))throw e;}
"#,
    );
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_getter_before_arguments_real_receiver_and_abrupt_order() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var BP=Object.getPrototypeOf(button),order='',marker={};
Object.defineProperty(BP,'toString',{configurable:true,get:function(){order+='G';if(this!==button)throw 'getter receiver';return function(value){order+='C';if(this!==button||value!==37)throw 'call receiver/argument';return 'observed';};}});
function argument(){order+='A';return 37;}
if(button.toString(argument())!=='observed'||order!=='GAC')throw 'inherited getter/argument/call order';
order='';Object.defineProperty(BP,'toString',{configurable:true,get:function(){order+='G';throw marker;}});
var caught=false;try{button.toString(argument());}catch(e){caught=e===marker;}if(!caught||order!=='G')throw 'abrupt member Get must precede argument';
delete BP.toString;order='';Object.defineProperty(button,'toString',{configurable:true,get:function(){order+='G';return stringify;}});
if(button.toString(argument())!=='[object HTMLButtonElement]'||order!=='GA')throw 'canonical builtin ignored args or lookup order';
order='';Object.defineProperty(button,'toString',{configurable:true,get:function(){order+='G';return 17;}});
try{button.toString(argument());throw 'noncallable accepted';}catch(e){if(!(e instanceof TypeError))throw e;}if(order!=='GA')throw 'noncallable member did not evaluate argument';
delete button.toString;if(button.toString!==stringify||list.toString!==stringify)throw 'getter deletion left ghost method';
"#,
    );
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_window_tag_override_delete_readonly_and_location_stringifier() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if(window.toString!==stringify||window.toString()!=='[object Window]'||window[tag]!=='Window')throw 'ordinary real Window stringifier/tag';
var originalWindowTag=Object.getOwnPropertyDescriptor(window,tag),tagReads=0;
if(originalWindowTag!==undefined||!(tag in window))throw 'Window class string must be inherited';
window[tag]='ignored';if(window[tag]!=='Window'||window.toString()!=='[object Window]')throw 'readonly Window tag assignment';
Object.defineProperty(window,tag,{configurable:true,get:function(){tagReads++;return 'AuthorWindow';}});
if(window.toString()!=='[object AuthorWindow]'||stringify.call(window)!=='[object AuthorWindow]'||tagReads!==2)throw 'Window own tag getter ignored';
delete window[tag];
if(window[tag]!=='Window'||window.toString()!=='[object Window]')throw 'Window override deletion/default restoration';
var anchor=document.getElementById('anchor'),savedLocation=location.toString;
if(typeof savedLocation!=='function'||savedLocation===stringify||savedLocation.call(location)!==location.href||location.toString()!=='https://dom-string.test/path/page')throw 'existing native Location stringifier changed';
if(anchor.href!=='https://dom-string.test/destination?q=one#part'||anchor.getAttribute('href')!=='../destination?q=one#part')throw 'existing anchor projection/raw attribute changed';
"#,
    );
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_same_owner_method_identity_and_cached_foreign_guards() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow,F=w.Function;
var childStringify=F('return Object.prototype.toString;')(),childRead=F('node','return node.toString;'),childInvoke=F('method','node','return method.call(node);'),own=F('return window===self&&document.body.nodeType===1;');
if(childStringify===stringify||childRead(button)!==stringify||childInvoke(childStringify,button)!=='[object HTMLButtonElement]'||childInvoke(stringify,list)!=='[object HTMLUListElement]')throw 'real per-realm Object intrinsic and target owner lookup';
var reads=0,tags=0,BP=Object.getPrototypeOf(button);
Object.defineProperty(BP,'toString',{configurable:true,get:function(){reads++;return stringify;}});
Object.defineProperty(button,tag,{configurable:true,get:function(){tags++;return 'OwnerButton';}});
"#,
    );
    let commits = page.state.dom_commits;
    page.set_location(&location("https://other-dom-string.test/changed"));
    for source in [
        "childRead(button);",
        "childInvoke(stringify,button);",
        "childInvoke(childStringify,button);",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        kept(&doc, &forms, &mut page);
        run(&mut doc,&mut forms,&mut page,"if(!own()||reads!==0||tags!==0||neighbor.value!=='edited neighbor')throw 'refusal ran getter or lost owner context';");
    }
    page.set_location(&location("https://dom-string.test/path/page"));
    run(&mut doc,&mut forms,&mut page,"if(childRead(button)!==stringify||childInvoke(childStringify,button)!=='[object OwnerButton]'||reads!==1||tags!==1)throw 'same-origin recovery';delete BP.toString;delete button[tag];if(button.toString()!=='[object HTMLButtonElement]')throw 'restored owner tag';");
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_unrelated_opaque_types_and_shared_copy_failure_recover() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(&mut doc,&mut forms,&mut page,"var opaque=document.getElementById('opaque');if(button.toString()!=='[object HTMLButtonElement]'||list.toString()!=='[object HTMLUListElement]')throw 'admitted concrete controls';");
    for source in [
        "Object.getPrototypeOf(opaque);",
        "opaque[Symbol.toStringTag];",
        "stringify.call(opaque);",
        "opaque.toString();",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        kept(&doc, &forms, &mut page);
    }
    run(&mut doc,&mut forms,&mut page,"var longTag='x',budgetGets=0;while(longTag.length<2048)longTag+=longTag;if(longTag.length!==2048)throw 'tag fixture length';Object.defineProperty(button,tag,{configurable:true,get:function(){budgetGets++;return longTag;}});");
    page.begin_job();
    page.runtime.constrain_job(
        usize::MAX, 512, usize::MAX, Instant::now() + Duration::from_secs(10),
    );
    let fault = page.runtime.execute(
        "button.toString();",
        &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state },
    ).unwrap_err();
    assert!(fault.contains("String/copy work limit reached"), "{fault}");
    assert!(page.runtime.invocation_failed_fatally());
    assert_eq!(page.state.dom_commits, commits);
    kept(&doc, &forms, &mut page);
    run(&mut doc,&mut forms,&mut page,"if(budgetGets!==1||button.toString()!=='[object '+longTag+']'||budgetGets!==2)throw 'shared copy bound or later job recovery';delete button[tag];if(button.toString()!=='[object HTMLButtonElement]')throw 'tag recovery';");
    assert_eq!(page.state.dom_commits, commits);
}
