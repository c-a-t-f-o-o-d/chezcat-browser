//! Original assignedSlot owner, ordinary-access and finite-refusal controls.
//! Child Text construction stays unavailable. No child navigation or IO.
use super::*;

fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::Location {
        url: Some(crate::url::parse(source).unwrap().0),
        path: None,
        root: None,
        fragment: String::new(),
    }
}
fn fixture() -> (Document, Forms, Page) {
    fixture_page(Page::for_test())
}
fn fixture_page(mut page:Page) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<!doctype html><title>Assigned slot owners</title><body><p id=main>main text</p><form id=f><input id=neighbor name=neighbor value=original></form>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    page.set_location(&location("https://assigned-slot.test/source"));
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    let editor = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    editor.cursor = 5;
    editor.anchor = 2;
    editor.offset = 1;
    (doc, forms, page)
}
fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
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
fn no_effects(page: &mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (added, removed, changed) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    no_effects(page);
}

#[test]
fn assigned_slot_realm_same_origin_saved_text_and_element_getters_revalidate_late_owner() {
    let (mut doc, mut forms, mut page) = fixture_page(Page::with_limits(crate::javascript::limits::BROWSER));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var mainElement=document.getElementById('main'),mainText=mainElement.firstChild;
var getElement=Object.getOwnPropertyDescriptor(Element.prototype,'assignedSlot').get;
var getText=Object.getOwnPropertyDescriptor(Text.prototype,'assignedSlot').get;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,childDoc=w.document,childBody=childDoc.body;
var childElementGet=Object.getOwnPropertyDescriptor(w.Element.prototype,'assignedSlot').get;
var childTextGet=Object.getOwnPropertyDescriptor(w.Text.prototype,'assignedSlot').get;
var childRead=w.Function('node','return node.assignedSlot;');
var borrow=w.Function('getter','node','return getter.call(node);');
var own=w.Function('return window===self&&document.body.assignedSlot===null&&document.body.ownerDocument===document&&document.body.firstChild===null;');
if(mainElement.nodeType!==1||mainText.nodeType!==3||mainText.data!=='main text'||!Text.prototype.isPrototypeOf(mainText))throw 'genuine main Slottable prefix';
if(mainElement.assignedSlot!==null||mainText.assignedSlot!==null||childBody.assignedSlot!==null)throw 'same-origin direct Slottable reads';
if(getElement===childElementGet||getText===childTextGet||childTextGet.name!=='get assignedSlot'||childTextGet.length!==0)throw 'actual per-owner getter identity';
if(getElement.call(childBody)!==null||childElementGet.call(mainElement)!==null||getText.call(mainText)!==null||childTextGet.call(mainText)!==null)throw 'same-origin saved getter borrowing';
if(childRead(mainElement)!==null||childRead(mainText)!==null||borrow(getElement,mainElement)!==null||borrow(getText,mainText)!==null||!own())throw 'same-origin ordinary access and native owner restoration';
var conversions=0,childTextArgument={toString:function(){conversions++;return 'unavailable child text';}},refused=0;
try{new w.Text(childTextArgument);}catch(error){if(error.name!=='NotSupportedError')throw error;refused++;}
if(refused!==1||conversions||childBody.firstChild!==null||!own())throw 'child Text construction boundary';
"#,
    );
    let neighbor = id(&doc, "neighbor");
    let preserved = editor(&forms, neighbor);
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let frames = page.state.blank_frames.len();
    page.set_location(&location("https://different-assigned-slot.test/changed"));
    for source in [
        "childBody.assignedSlot;",
        "getElement.call(childBody);",
        "childElementGet.call(childBody);",
        "childElementGet.call(mainElement);",
        "childTextGet.call(mainText);",
        "childRead(mainElement);",
        "childRead(mainText);",
        "borrow(getElement,mainElement);",
        "borrow(getText,mainText);",
    ] {
        page.clear_console();
        let allocations = page.runtime.window_allocation_state_for_test();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(
            page.report().contains("same-origin"),
            "{source}\n{}",
            page.report()
        );
        assert_eq!(
            page.runtime.window_allocation_state_for_test(),
            allocations,
            "{source}"
        );
        assert_eq!(format!("{:?}", doc.nodes), nodes, "{source}");
        assert_eq!(page.state.dom_commits, commits, "{source}");
        assert_eq!(page.state.blank_frames.len(), frames);
        assert_eq!(editor(&forms, neighbor), preserved);
        assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
        no_effects(&mut page);
        run(&mut doc, &mut forms, &mut page, "if(!own()||window!==self||mainElement.assignedSlot!==null||mainText.assignedSlot!==null||getElement.call(mainElement)!==null||getText.call(mainText)!==null)throw 'main and actual child owner recovery';");
    }
}

#[test]
fn assigned_slot_realm_main_wrong_receivers_and_readonly_access_preserve_native_state() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var mainElement=document.getElementById('main'),mainText=mainElement.firstChild;
var getElement=Object.getOwnPropertyDescriptor(Element.prototype,'assignedSlot').get;
var getText=Object.getOwnPropertyDescriptor(Text.prototype,'assignedSlot').get;
var comment=document.createComment('kept'),fragment=document.createDocumentFragment();
var fakeElement=Object.create(Element.prototype),fakeText=Object.create(Text.prototype),caught=0;
"#,
    );
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
for(var getter of [getElement,getText])for(var receiver of [null,undefined,{},fakeElement,fakeText,document,comment,fragment]){
 try{getter.call(receiver);throw 'wrong assignedSlot receiver accepted';}catch(error){if(error.name!=='TypeError'||!(error instanceof TypeError))throw error;caught++;}
}
if(caught!==16||getElement.call(mainElement)!==null||getText.call(mainText)!==null)throw 'real brand after wrong receivers';
mainElement.assignedSlot=17;mainText.assignedSlot=19;
if(mainElement.assignedSlot!==null||mainText.assignedSlot!==null||Object.hasOwn(mainElement,'assignedSlot')||Object.hasOwn(mainText,'assignedSlot'))throw 'sloppy readonly getter';
var readonly=0;for(var node of [mainElement,mainText])try{(function(target){'use strict';target.assignedSlot=23;})(node);}catch(error){if(error.name!=='TypeError'||!(error instanceof TypeError))throw error;readonly++;}
if(readonly!==2||mainElement.assignedSlot!==null||mainText.assignedSlot!==null||mainText.data!=='main text')throw 'strict readonly state';
"#,
    );
    let neighbor = id(&doc, "neighbor");
    assert_eq!(
        editor(&forms, neighbor),
        ("edited neighbor".into(), 5, 2, 1, true, Some(id(&doc, "f")))
    );
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(doc.title, "Assigned slot owners");
    assert_eq!(doc.attr(neighbor, "value"), Some("original"));
    no_effects(&mut page);
}

#[test]
fn assigned_slot_realm_host_work_failure_is_fatal_without_native_change_and_next_job_recovers() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "var mainElement=document.getElementById('main'),mainText=mainElement.firstChild,caught=0,finalized=0,after=0;if(mainElement.assignedSlot!==null||mainText.assignedSlot!==null)throw 'assigned slot budget prefix';");
    let neighbor = id(&doc, "neighbor");
    let preserved = editor(&forms, neighbor);
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let allocations = page.runtime.window_allocation_state_for_test();
    let allowance = page.state.limits.host_work;
    page.state.limits.host_work = 0;
    page.clear_console();
    let outcome = page.console_eval(
        &mut doc,
        &mut forms,
        "try{mainText.assignedSlot;after++;}catch(error){caught++;}finally{finalized++;}after++;",
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert!(
        page.report().to_ascii_lowercase().contains("work"),
        "{}",
        page.report()
    );
    assert!(page.runtime.invocation_failed_fatally());
    assert_eq!(page.runtime.window_allocation_state_for_test(), allocations);
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(editor(&forms, neighbor), preserved);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    no_effects(&mut page);
    page.state.limits.host_work = allowance;
    run(&mut doc, &mut forms, &mut page, "if(caught||finalized||after||mainElement.assignedSlot!==null||mainText.assignedSlot!==null||window!==self||mainText.data!=='main text')throw 'fatal work containment or next-job recovery';");
    assert!(!page.runtime.invocation_failed_fatally());
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(editor(&forms, neighbor), preserved);
}
