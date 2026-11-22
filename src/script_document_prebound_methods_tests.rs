//! Actual startup-bound Document methods keep native routing after own storage.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &forms.controls[&node];
    (control.value.text(), control.value.cursor, control.value.anchor,
        control.value.offset, control.dirty_value, control.owner)
}

fn idle(page: &mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (changed, removed, pending) = page.take_image_changes();
    assert!(changed.is_empty() && removed.is_empty() && pending.is_empty());
}

fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    idle(page);
}

#[test]
fn document_own_properties_prebound_native_methods_keep_identity_and_refuse_ordinary_replacement() {
    let mut doc = crate::html::parse("<!doctype html><title>Prebound Document seed</title><body><form id=known name=preboundForm><input id=neighbor name=neighbor value=original></form><p id=content>Original content</p>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    let control = forms.controls.get_mut(&neighbor).unwrap();
    control.value.cursor = 5;
    control.value.anchor = 2;
    control.value.offset = 1;
    let preserved = editor(&forms, neighbor);
    assert!(preserved.4);
    assert_eq!(preserved.5, Some(id(&doc, "known")));
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    let mut page = Page::for_test();
    let location = crate::navigation::address("https://document-prebound.test/source", &crate::navigation::Location::home()).unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
    run(&mut doc, &mut forms, &mut page, r#"
var names=['getElementById','querySelector','querySelectorAll','createElement','createElementNS','createTextNode','createComment','createDocumentFragment'],saved={};
for(var name of names){if(typeof document[name]!=='function')throw 'missing prebound native method';saved[name]=document[name];}
var body=document.body,form=document.getElementById('known'),neighbor=document.getElementById('neighbor'),content=document.getElementById('content');
if(document.preboundForm!==form||document.title!=='Prebound Document seed'||body.nodeName!=='BODY')throw 'native read prefix';
if(saved.getElementById.call(document,'content')!==content||Document.prototype.getElementsByTagName.call(document,'p').item(0)!==content||saved.querySelector.call(document,'#content')!==content||saved.querySelectorAll.call(document,'p').item(0)!==content)throw 'genuine prebound query operations';
var element=saved.createElement.call(document,'div'),namespaced=saved.createElementNS.call(document,'http://www.w3.org/1999/xhtml','span'),text=saved.createTextNode.call(document,'literal'),comment=saved.createComment.call(document,'original comment'),fragment=saved.createDocumentFragment.call(document);
if(element.nodeName!=='DIV'||namespaced.nodeName!=='SPAN'||namespaced.namespaceURI!=='http://www.w3.org/1999/xhtml'||text.nodeType!==3||text.nodeValue!=='literal'||comment.nodeType!==8||comment.nodeValue!=='original comment'||fragment.nodeType!==11||fragment.ownerDocument!==document||fragment.childNodes.length!==0)throw 'genuine prebound creation operations';
document.ordinaryMethodState={kept:41};
var replacementCalls=0,replacement=function(){replacementCalls++;throw 'replacement callback must not run';},replacementDescriptor={value:replacement,writable:true,enumerable:true,configurable:true};
for(var methodName of ['getElementById','getElementsByTagName','getElementsByClassName']){
 var canonical=Document.prototype[methodName];if(document[methodName]!==canonical)throw 'canonical inherited Document method';
 document[methodName]=replacement;if(document[methodName]!==replacement)throw 'ordinary inherited method override';
 if(!delete document[methodName]||document[methodName]!==canonical)throw 'deletion restores prototype method';
 Object.defineProperty(document,methodName,replacementDescriptor);if(document[methodName]!==replacement)throw 'ordinary inherited method descriptor';
 if(!delete document[methodName]||document[methodName]!==canonical)throw 'descriptor deletion restores prototype';
}
// These existing Node/EventTarget methods remain ordinary inherited methods.
for(var eventName of ['addEventListener','removeEventListener','dispatchEvent']){
 var canonical=document[eventName];document[eventName]=replacement;
 if(document[eventName]!==replacement)throw 'ordinary event method override';
 if(!delete document[eventName]||document[eventName]!==canonical)throw 'event method deletion restores canonical';
}
if(replacementCalls||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.form!==form)throw 'prepared replacement/editor prefix';
"#);
    let nodes = format!("{:?}", doc.nodes);
    assert_eq!(editor(&forms, neighbor), preserved);
    for name in ["querySelector", "querySelectorAll", "createElement", "createElementNS", "createTextNode", "createComment", "createDocumentFragment"] {
        for source in [
            format!("document.{name}=replacement;"),
            format!("Object.defineProperty(document,'{name}',replacementDescriptor);"),
        ] {
            page.clear_console();
            let outcome = page.console_eval(&mut doc, &mut forms, &source);
            assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
            assert_eq!(format!("{:?}", doc.nodes), nodes, "{source}");
            assert_eq!(doc.title, "Prebound Document seed", "{source}");
            assert_eq!(editor(&forms, neighbor), preserved, "{source}");
            assert_eq!(forms.text_default_value(neighbor).unwrap(), "original", "{source}");
            assert_eq!(forms.controls.len(), 1, "{source}");
            idle(&mut page);
            run(&mut doc, &mut forms, &mut page, r#"
for(var name of names)if(document[name]!==saved[name])throw 'native method identity changed';
if(replacementCalls||document.ordinaryMethodState.kept!==41||document.body!==body||document.title!=='Prebound Document seed'||document.preboundForm!==form||document.getElementById('content')!==content||content.textContent!=='Original content'||neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.form!==form)throw 'native refusal effects or recovery';
"#);
            assert_eq!(format!("{:?}", doc.nodes), nodes, "recovery after {source}");
        }
    }
}
