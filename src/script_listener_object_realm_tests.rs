//! Real same-origin child callback objects delivered on an ordinary main target.
use super::*;

fn eval(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
}

fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &forms.controls[&node];
    (
        control.value.text(),
        control.value.cursor,
        control.value.anchor,
        control.value.offset,
        control.dirty_value,
        control.owner,
    )
}

#[test]
fn listener_object_realm_child_eval_callback_uses_live_operation_and_real_main_events() {
    let mut doc = crate::html::parse(
        "<html><body><button id=send type=button>Send</button><input id=neighbor name=neighbor value=original></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://listener-owner.test/source",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    let button = (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some("send"))
        .unwrap();
    let neighbor = (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some("neighbor"))
        .unwrap();
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    let selection = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    selection.cursor = 5;
    selection.anchor = 2;
    selection.offset = 1;
    let before = editor(&forms, neighbor);
    eval(
        &mut page,
        &mut doc,
        &mut forms,
        r#"
var b=document.getElementById('send'),neighbor=document.getElementById('neighbor');
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,childArray=w.Array,parentArray=Array;
var listener=w.eval("var listener={calls:0,lookups:0,expectedTrusted:true}; Object.defineProperty(listener,'handleEvent',{get:function(){this.lookups++;this.getterArray=Array;return this.selected;}}); listener.selected=function(e){if(this!==listener||e.isTrusted!==this.expectedTrusted)throw 'child callback receiver or trust';this.calls++;this.methodArray=Array;this.target=e.currentTarget;this.event=e;};listener;");
if(childArray===Array||listener.calls!==0||listener.lookups!==0)throw 'genuine child prefix';
Object.setPrototypeOf(listener,Object.prototype);
b.addEventListener('click',listener);
if(listener.lookups!==0)throw 'registration got child operation';
"#,
    );
    page.begin_job();
    let first = page.dispatch(&mut doc, &mut forms, button, "click", true);
    assert_eq!(first.errors, 0, "{}", page.report());
    eval(
        &mut page,
        &mut doc,
        &mut forms,
        r#"
if(listener.calls!==1||listener.lookups!==1||listener.getterArray!==childArray||listener.methodArray!==childArray||listener.target!==b)throw 'real child native delivery';
if(Array!==parentArray||Object.getPrototypeOf([])!==parentArray.prototype)throw 'native caller realm leaked';
listener.selected=function(e){
    if(this!==listener||e.isTrusted!==this.expectedTrusted)throw 'parent operation receiver or trust';
    this.calls++;this.methodArray=Array;this.target=e.currentTarget;this.event=e;
};
"#,
    );
    page.begin_job();
    let second = page.dispatch(&mut doc, &mut forms, button, "click", true);
    assert_eq!(second.errors, 0, "{}", page.report());
    eval(
        &mut page,
        &mut doc,
        &mut forms,
        r#"
if(listener.calls!==2||listener.lookups!==2||listener.getterArray!==childArray||listener.methodArray!==parentArray||listener.target!==b)throw 'live foreign operation ownership';
listener.expectedTrusted=false;
var event=document.createEvent('Event');event.initEvent('parcel',true,true);
b.addEventListener('parcel',listener);
if(!b.dispatchEvent(event)||listener.calls!==3||listener.lookups!==3||listener.event!==event||listener.target!==b)throw 'child object synchronous delivery';
if(listener.getterArray!==childArray||listener.methodArray!==parentArray||Array!==parentArray||Object.getPrototypeOf([])!==parentArray.prototype)throw 'synchronous owner or caller realm';
b.removeEventListener('click',listener);b.removeEventListener('parcel',listener);
if(!b.dispatchEvent(event)||listener.calls!==3||listener.lookups!==3)throw 'child object removal';
if(neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.ownerDocument!==document)throw 'neighbor changed';
"#,
    );
    assert_eq!(editor(&forms, neighbor), before);
    assert!(page.state.listeners.is_empty());
    assert!(page.state.synthetic_dispatches.is_empty());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (added, removed, changed) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
}
