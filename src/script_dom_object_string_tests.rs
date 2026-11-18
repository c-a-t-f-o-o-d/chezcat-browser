//! Original DOM string witness for Popper's getWindow node.toString() boundary.
//! No upstream source, fallback stringifier, resource load or renderer stub.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    doc.nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id==name))
        .unwrap()
}
fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let c = &forms.controls[&node];
    (
        c.value.text(),
        c.value.cursor,
        c.value.anchor,
        c.value.offset,
        c.dirty_value,
        c.owner,
    )
}
fn no_effects(page: &mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}

#[test]
fn dom_object_string_original_real_button_list_window_inherited_and_borrowed_calls() {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Original DOM object string</title><body>",
        "<form id=form><button id=toggle type=button>Original toggle</button>",
        "<input id=neighbor name=keep value=original></form>",
        "<ul id=menu><li><button id=choice type=button>Original choice</button></li></ul>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://dom-string.test/original",
        &crate::navigation::Location::home(),
    )
    .unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let (neighbor, toggle, menu) = (id(&doc, "neighbor"), id(&doc, "toggle"), id(&doc, "menu"));
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    let kept = editor(&forms, neighbor);
    assert_eq!(kept.5, Some(id(&doc, "form")));
    let prefix=page.console_eval(&mut doc,&mut forms,r#"
var button=document.getElementById('toggle'),menu=document.getElementById('menu'),choice=document.getElementById('choice'),neighbor=document.getElementById('neighbor');
if(button.nodeType!==1||button.nodeName!=='BUTTON'||button.type!=='button'||menu.nodeType!==1||menu.nodeName!=='UL'||choice.nodeName!=='BUTTON')throw 'real native button/list identities';
if(button!==document.querySelector('#toggle')||menu!==document.querySelector('ul')||choice.parentNode.parentNode!==menu)throw 'real retained DOM topology';
if(button.ownerDocument!==document||menu.ownerDocument!==document||window.document!==document)throw 'real owner Document and Window';
if(neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.getAttribute('value')!=='original'||neighbor.form!==document.getElementById('form'))throw 'edited native neighbor prefix';
document.title='DOM object string positive prefix passed';
"#);
    assert_eq!(prefix.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "DOM object string positive prefix passed");
    assert_eq!(editor(&forms, neighbor), kept);
    no_effects(&mut page);
    let commits = page.state.dom_commits;

    // First new operation deliberately matches getWindow's direct invocation.
    // Callable/tag/borrowed controls follow it, so no alternate primitive path
    // or generic Object call can hide failure at node.toString().
    let outcome=page.console_eval(&mut doc,&mut forms,r#"
var buttonTag=button.toString();
if(buttonTag!=='[object HTMLButtonElement]')throw 'direct genuine button tag';
var listTag=menu.toString(),windowTag=window.toString(),stringify=Object.prototype.toString;
if(listTag!=='[object HTMLUListElement]'||windowTag!=='[object Window]')throw 'direct genuine list/Window tags';
if(typeof button.toString!=='function'||typeof menu.toString!=='function'||button.toString!==stringify||menu.toString!==stringify)throw 'ordinary inherited Object stringifier';
if(Object.hasOwn(button,'toString')||Object.hasOwn(menu,'toString'))throw 'inherited method became own native member';
if(stringify.call(button)!==buttonTag||stringify.call(menu)!==listTag||stringify.call(window)!==windowTag)throw 'borrowed Object stringifier disagreed';
var saved=button.toString;if(saved.call(menu)!==listTag||saved.call(window)!==windowTag||saved.call(choice)!==buttonTag)throw 'inherited stringifier borrowing changed receiver';
if(stringify.call({})!=='[object Object]'||stringify.call(null)!=='[object Null]'||stringify.call(undefined)!=='[object Undefined]')throw 'ordinary borrowed controls';
if(button.ownerDocument.defaultView!==window||menu.ownerDocument!==document||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original')throw 'stringification changed real DOM/native state';
"#);
    assert_eq!(editor(&forms, neighbor), kept);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    assert_eq!(doc.attr(neighbor, "value"), Some("original"));
    assert_eq!(doc.attr(toggle, "type"), Some("button"));
    assert_eq!(doc.attr(menu, "id"), Some("menu"));
    assert_eq!(page.state.dom_commits, commits);
    no_effects(&mut page);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}
