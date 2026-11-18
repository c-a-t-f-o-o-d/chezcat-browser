//! Original Document own-store authority, policy and finite-name controls.
use super::*;

fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::Location { url: Some(crate::url::parse(source).unwrap().0),
        path: None, root: None, fragment: String::new() }
}

fn start(source: &str, child: bool, limits: crate::javascript::limits::Limits)
    -> (Document, Forms, Page)
{
    let mut doc = crate::html::parse("<!doctype html><title>Document security seed</title><body><form id=f><input id=neighbor name=neighbor value=original></form><p>Original document security fixture</p>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let neighbor = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("neighbor")).unwrap();
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    forms.controls.get_mut(&neighbor).unwrap().value.cursor = 5;
    forms.controls.get_mut(&neighbor).unwrap().value.anchor = 2;
    let mut page = Page::with_limits(limits);
    page.set_location(&location("https://document-own.test/parent/page"));
    let source = if child {
        format!("var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow;{source}")
    } else { source.into() };
    let outcome = page.start(&mut doc, &mut forms, &[Script {
        node: 0, label: "Original Document authority fixture".into(), source,
    }]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn run(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}: {}", page.report());
    assert!(!page.has_navigation_request());
}

fn denied(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str, message: &str) {
    page.clear_console();
    let allocations = page.runtime.window_allocation_state_for_test();
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 1, "{source}: {}", page.report());
    assert!(page.report().contains(message), "{source}: {}", page.report());
    assert_eq!(page.runtime.window_allocation_state_for_test(), allocations,
        "Refused operation allocated native backing: {source}");
    assert!(!page.has_navigation_request());
}

#[test]
fn document_own_properties_cached_operations_revalidate_real_child_origin_before_callbacks() {
    let (mut doc, mut forms, mut page) = start(r#"
var rootWindow=window,cachedRoot=document,getterCalls=0,setterCalls=0,setterValue=0,seen=null;
document.securityState={count:31};var state=document.securityState,ownSymbol=Symbol('document-own');document[ownSymbol]=71;
Object.defineProperty(document,'guardedState',{get:function(){getterCalls++;seen=this;return state;},set:function(value){setterCalls++;setterValue=value;seen=this;},enumerable:true,configurable:true});
var ChildFunction=w.Function;
var childRead=ChildFunction('d','key','return d[key];');
var childWrite=ChildFunction('d','key','value','d[key]=value;return value;');
var childDescriptor=ChildFunction('d','key','return Object.getOwnPropertyDescriptor(d,key);');
var childHasOwn=ChildFunction('d','key','return Object.hasOwn(d,key);');
var childIn=ChildFunction('d','key','return key in d;');
var childDelete=ChildFunction('d','key','return delete d[key];');
var childDefine=ChildFunction('d','key','descriptor','return Object.defineProperty(d,key,descriptor);');
var childKeys=ChildFunction('d','return Object.keys(d);');
var childNames=ChildFunction('d','return Object.getOwnPropertyNames(d);');
var childSymbols=ChildFunction('d','return Object.getOwnPropertySymbols(d);');
var childOwn=ChildFunction('return window===self&&document.body.nodeType===1&&location.href==="about:blank";');
var dataDescriptor={value:23,writable:true,enumerable:true,configurable:true};
if(childRead(cachedRoot,'securityState')!==state||childRead(cachedRoot,'guardedState')!==state)throw 'same-origin cached read';
childWrite(cachedRoot,'guardedState',99);
var descriptor=childDescriptor(cachedRoot,'guardedState');
if(typeof descriptor.get!=='function'||typeof descriptor.set!=='function'||getterCalls!==1||setterCalls!==1||setterValue!==99||seen!==cachedRoot)throw 'same-origin accessor/inspection';
if(!childHasOwn(cachedRoot,'securityState')||!childIn(cachedRoot,'securityState')||!childDelete(cachedRoot,'absent'))throw 'same-origin membership/delete';
if(childDefine(cachedRoot,'temporaryDefined',dataDescriptor)!==cachedRoot||cachedRoot.temporaryDefined!==23)throw 'same-origin definition';
delete cachedRoot.temporaryDefined;
if(childKeys(cachedRoot).indexOf('securityState')<0||childNames(cachedRoot).indexOf('guardedState')<0||childSymbols(cachedRoot)[0]!==ownSymbol||childRead(cachedRoot,ownSymbol)!==71)throw 'same-origin own inspection';
if(!childOwn()||document.body.nodeType!==1)throw 'native owner prefix';
"#, true, crate::javascript::limits::CAPACITY_REGRESSION);
    page.set_location(&location("https://other-document.test/changed"));
    for source in [
        "childRead(cachedRoot,'securityState');",
        "childRead(cachedRoot,'guardedState');",
        "childWrite(cachedRoot,'guardedState',101);",
        "childDescriptor(cachedRoot,'guardedState');",
        "childHasOwn(cachedRoot,'securityState');",
        "childIn(cachedRoot,'securityState');",
        "childWrite(cachedRoot,'freshDenied',101);",
        "childDelete(cachedRoot,'securityState');",
        "childDefine(cachedRoot,'freshDenied',dataDescriptor);",
        "childKeys(cachedRoot);",
        "childNames(cachedRoot);",
        "childSymbols(cachedRoot);",
        "childRead(cachedRoot,ownSymbol);",
        "childWrite(cachedRoot,ownSymbol,101);",
        "childDelete(cachedRoot,ownSymbol);",
        "childDefine(cachedRoot,ownSymbol,dataDescriptor);",
    ] {
        denied(&mut page, &mut doc, &mut forms, source, "same-origin");
        run(&mut page, &mut doc, &mut forms, r#"
if(getterCalls!==1||setterCalls!==1||setterValue!==99||seen!==document||state.count!==31||document.securityState!==state||document[ownSymbol]!==71||Object.hasOwn(document,'freshDenied'))throw 'foreign operation entered callbacks or changed own state';
if(!childOwn()||window!==rootWindow||window!==self||document.body.nodeType!==1)throw 'later genuine owner recovery';
"#);
    }
}

#[test]
fn document_own_properties_do_not_admit_child_documents_or_bypass_child_compilation_policy() {
    let (mut doc, mut forms, mut page) = start(r#"
document.policyState={count:37};var savedState=document.policyState;
var ChildFunction=w.Function,childDocument=w.document;
var childWrite=ChildFunction('document.childOwnProbe=41;');
var childOwn=ChildFunction('return window===self&&document.body.nodeType===1&&location.href==="about:blank";');
var childCompile=ChildFunction('source','try{Function(source);return false;}catch(error){if(error.name!=="EvalError")throw error;return window===self&&document.body.nodeType===1;}');
if(childDocument.body.nodeType!==1||!childOwn()||document.policyState!==savedState)throw 'real child/native prefix';
"#, true, crate::javascript::limits::CAPACITY_REGRESSION);
    for changed_origin in [false, true] {
        if changed_origin { page.set_location(&location("https://other-document.test/changed")); }
        denied(&mut page, &mut doc, &mut forms, "childWrite();",
            "Blank child native writes and navigation are not implemented");
        run(&mut page, &mut doc, &mut forms,
            "if(!childOwn()||document.policyState!==savedState||savedState.count!==37||window!==self||document.body.nodeType!==1)throw 'owner recovery after unsupported child write';");
    }
    page.state.blank_frames[0].policy.add_header("script-src 'unsafe-inline'");
    run(&mut page, &mut doc, &mut forms,
        "if(!childCompile('document.childOwnProbe=43;')||!childOwn()||document.policyState!==savedState||savedState.count!==37)throw 'Document own state bypassed child CSP';");
}

#[test]
fn document_own_properties_name_limit_is_fatal_atomic_and_next_job_recovers() {
    let mut limits = crate::javascript::limits::CAPACITY_REGRESSION;
    limits.capacity.property_bytes = 64;
    let (mut doc, mut forms, mut page) = start(r#"
document.stableState={kept:true};var original=document.stableState,caught=0,finalized=0,after=0;
if(document.title!=='Document security seed'||document.body.nodeName!=='BODY')throw 'native quota prefix';
"#, false, limits);
    let neighbor = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("neighbor")).unwrap();
    let editor = &forms.controls[&neighbor];
    let before = (editor.value.text(), editor.value.cursor, editor.value.anchor,
        editor.value.offset, editor.dirty_value, editor.owner);
    let nodes = doc.nodes.len();
    let long_key = "q".repeat(65);
    let source = format!("try{{document['{long_key}']=47;after++;}}catch(error){{caught++;}}finally{{finalized++;}}after++;");
    page.clear_console();
    let outcome = page.console_eval(&mut doc, &mut forms, &source);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert!(page.report().contains("Property-name limit"), "{}", page.report());
    assert!(page.runtime.invocation_failed_fatally());
    assert_eq!(doc.nodes.len(), nodes);
    let editor = &forms.controls[&neighbor];
    assert_eq!((editor.value.text(), editor.value.cursor, editor.value.anchor,
        editor.value.offset, editor.dirty_value, editor.owner), before);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
    run(&mut page, &mut doc, &mut forms, r#"
if(caught||finalized||after||document.stableState!==original||!original.kept||Object.keys(document).join(',')!=='stableState')throw 'fatal property key changed state or entered user handlers';
document.recoveredState=53;
if(document.recoveredState!==53||document.body.nodeName!=='BODY'||document.getElementById('neighbor').value!=='edited neighbor')throw 'following own/native job';
"#);
}
