//! Original real-Document ordinary-property controls; no framework source.
use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &forms.controls[&node];
    (control.value.text(), control.value.cursor, control.value.anchor,
        control.value.offset, control.dirty_value, control.owner)
}

#[test]
fn document_own_properties_original_numeric_state_and_descriptors_preserve_native_neighbors() {
    let mut doc = crate::html::parse("<!doctype html><title>Document native seed</title><body><form id=known name=ordinaryFixtureForm><input id=neighbor name=neighbor value=original></form><p id=original>Original body</p>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    let control = forms.controls.get_mut(&neighbor).unwrap();
    control.value.cursor = 5;
    control.value.anchor = 2;
    control.value.offset = 1;
    let before = editor(&forms, neighbor);
    assert!(before.4);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    let mut page = Page::for_test();
    let base = crate::navigation::address("https://document-own.test/source", &crate::navigation::Location::home()).unwrap();
    page.set_location(&base);
    page.set_resource_base(&base);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
    let prefix = page.console_eval(&mut doc, &mut forms, r#"
var nativeBody=document.body,nativeForm=document.getElementById('known'),nativeNeighbor=document.getElementById('neighbor');
if(document.title!=='Document native seed'||nativeBody.nodeName!=='BODY'||document.ordinaryFixtureForm!==nativeForm)throw 'native Document prefix';
if(nativeNeighbor.value!=='edited neighbor'||nativeNeighbor.defaultValue!=='original'||nativeNeighbor.form!==nativeForm)throw 'native editor prefix';
document.title='Document native prefix reached';
"#);
    assert_eq!(prefix.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Document native prefix reached");
    assert_eq!(editor(&forms, neighbor), before);

    let outcome = page.console_eval(&mut doc, &mut forms, r#"
document.fixtureCounter=0;
document.fixtureCounter++;
var retained={count:document.fixtureCounter,owner:document};document.fixtureState=retained;
var descriptor=Object.getOwnPropertyDescriptor(document,'fixtureCounter');
if(document.fixtureCounter!==1||document.fixtureState!==retained||document.fixtureState.owner!==document||retained.count!==1)throw 'ordinary Document numeric/state identity';
if(!descriptor||descriptor.value!==1||!descriptor.writable||!descriptor.enumerable||!descriptor.configurable||!Object.hasOwn(document,'fixtureCounter')||!('fixtureCounter' in document))throw 'ordinary Document descriptor';
if(document.body!==nativeBody||document.ordinaryFixtureForm!==nativeForm||nativeNeighbor.value!=='edited neighbor'||nativeNeighbor.defaultValue!=='original')throw 'native neighbors after ordinary properties';
document.title='Document ordinary properties passed';
"#);
    assert_eq!(editor(&forms, neighbor), before);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    assert_eq!(forms.controls.len(), 1);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Document ordinary properties passed");
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}
