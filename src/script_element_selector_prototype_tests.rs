//! Original canonical Element selector reduction; direct queries must pass first.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn editor_state(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &forms.controls[&node];
    (control.value.text(), control.value.cursor, control.value.anchor,
        control.value.offset, control.dirty_value, control.owner)
}

#[test]
fn element_selector_prototype_original_direct_queries_then_borrowed_canonical_operations() {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Canonical Element selector seed</title><body>",
        "<form id=owner><input id=neighbor name=keep value=original></form>",
        "<span id=outside class=item>Outside</span>",
        "<section id=scope class=item>lead<span id=a class=item>A</span>",
        "<div id=nested><span id=b class=item>B</span></div>tail</section>"
    ));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address(
        "https://element-selector.test/original",
        &crate::navigation::Location::home(),
    ).unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());

    let neighbor = id(&doc, "neighbor");
    let owner = id(&doc, "owner");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let editor = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        editor.cursor = 5;
        editor.anchor = 2;
        editor.offset = 1;
    }
    let kept_editor = editor_state(&forms, neighbor);
    assert_eq!(kept_editor.0, "edited neighbor");
    assert!(kept_editor.4);
    assert_eq!(kept_editor.5, Some(owner));
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");

    // This entire prefix uses the existing direct instance/Document paths.
    // Its title and native-state checks precede every Element.prototype read.
    let direct = page.console_eval(&mut doc, &mut forms, r#"
var scope=document.getElementById('scope'),a=document.getElementById('a'),b=document.getElementById('b');
var outside=document.getElementById('outside'),nested=document.getElementById('nested'),neighbor=document.getElementById('neighbor');
if(scope.parentNode!==document.body||a.parentNode!==scope||b.parentNode!==nested||nested.parentNode!==scope)throw 'direct selector physical parents';
if(scope.ownerDocument!==document||a.ownerDocument!==document||b.ownerDocument!==document)throw 'direct selector document ownership';
if(scope.querySelector('.item')!==a||scope.querySelector('#b')!==b||scope.querySelector('.absent')!==null)throw 'direct Element querySelector identity/order';
var oldList=scope.querySelectorAll('.item'),oldEmpty=scope.querySelectorAll('.absent');
if(oldList.length!==2||oldList[0]!==a||oldList[1]!==b||oldEmpty.length!==0)throw 'direct Element descendant scope/order';
if(document.querySelector('#scope')!==scope||document.querySelector('#neighbor')!==neighbor||document.querySelector('.item')!==outside)throw 'existing Document querySelector';
var documentList=document.querySelectorAll('.item');
if(documentList.length!==4||documentList[0]!==outside||documentList[1]!==scope||documentList[2]!==a||documentList[3]!==b)throw 'existing Document querySelectorAll order';
var third=document.createElement('span');third.id='third';third.className='item';third.textContent='C';
if(scope.appendChild(third)!==third||third.parentNode!==scope||third.ownerDocument!==document)throw 'direct selector ordinary append identity';
var freshList=scope.querySelectorAll('.item'),freshDocumentList=document.querySelectorAll('.item');
if(oldList.length!==2||oldList[0]!==a||oldList[1]!==b||oldEmpty.length!==0||documentList.length!==4)throw 'direct query snapshots must stay static';
if(freshList===oldList||freshList.length!==3||freshList[0]!==a||freshList[1]!==b||freshList[2]!==third)throw 'direct fresh query snapshot';
if(freshDocumentList.length!==5||freshDocumentList[4]!==third||document.getElementById('third')!==third)throw 'Document query follows real append';
if(neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.getAttribute('value')!=='original'||neighbor.form!==document.getElementById('owner'))throw 'direct queries changed neighboring native state';
document.title='Canonical Element selector direct prefix reached';
"#);
    assert_eq!(direct.errors, 0, "Direct positive prefix failed: {}", page.report());
    assert_eq!(doc.title, "Canonical Element selector direct prefix reached");
    assert_eq!(editor_state(&forms, neighbor), kept_editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    assert_eq!(doc.attr(neighbor, "value"), Some("original"));
    let scope = id(&doc, "scope");
    let a = id(&doc, "a");
    let nested = id(&doc, "nested");
    let b = id(&doc, "b");
    let third = id(&doc, "third");
    let kept_children = doc.nodes[scope].children.clone();
    assert_eq!(kept_children.len(), 5);
    assert!(matches!(&doc.nodes[kept_children[0]].kind, NodeKind::Text(value) if value == "lead"));
    assert_eq!(kept_children[1], a);
    assert_eq!(kept_children[2], nested);
    assert!(matches!(&doc.nodes[kept_children[3]].kind, NodeKind::Text(value) if value == "tail"));
    assert_eq!(kept_children[4], third);
    assert_eq!(doc.nodes[nested].children, vec![b]);

    let canonical = page.console_eval(&mut doc, &mut forms, r#"
var allDescriptor=Object.getOwnPropertyDescriptor(Element.prototype,'querySelectorAll');
if(!allDescriptor||typeof allDescriptor.value!=='function')throw 'canonical Element querySelectorAll own operation is missing';
var all=Element.prototype.querySelectorAll;
if(all!==allDescriptor.value||all.name!=='querySelectorAll'||all.length!==1||!allDescriptor.writable||!allDescriptor.enumerable||!allDescriptor.configurable||'get' in allDescriptor||'set' in allDescriptor)throw 'canonical Element querySelectorAll operation descriptor';
var oneDescriptor=Object.getOwnPropertyDescriptor(Element.prototype,'querySelector');
if(!oneDescriptor||typeof oneDescriptor.value!=='function')throw 'canonical Element querySelector own operation is missing';
var one=Element.prototype.querySelector;
if(one!==oneDescriptor.value||one.name!=='querySelector'||one.length!==1||!oneDescriptor.writable||!oneDescriptor.enumerable||!oneDescriptor.configurable||'get' in oneDescriptor||'set' in oneDescriptor)throw 'canonical Element querySelector operation descriptor';
var canonicalList=Element.prototype.querySelectorAll.call(scope,'.item');
if(canonicalList===freshList||canonicalList.length!==3||canonicalList[0]!==a||canonicalList[1]!==b||canonicalList[2]!==third)throw 'borrowed canonical Element querySelectorAll identities';
if(Element.prototype.querySelector.call(scope,'.item')!==a||one.call(scope,'#b')!==b||one.call(scope,'.absent')!==null||all.call(scope,'.absent').length!==0)throw 'borrowed canonical Element querySelector results';
if(oldList.length!==2||documentList.length!==4||freshList.length!==3||freshDocumentList.length!==5||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original')throw 'canonical queries changed retained state';
document.title='Canonical Element selector operations passed';
"#);
    // Even the initial missing-operation failure must preserve physical/native state.
    assert_eq!(doc.nodes[scope].children, kept_children);
    assert_eq!(doc.nodes[nested].children, vec![b]);
    assert_eq!(editor_state(&forms, neighbor), kept_editor);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    assert_eq!(doc.attr(neighbor, "value"), Some("original"));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (changed, removed, pending) = page.take_image_changes();
    assert!(changed.is_empty() && removed.is_empty() && pending.is_empty());
    assert_eq!(canonical.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Canonical Element selector operations passed");
}
