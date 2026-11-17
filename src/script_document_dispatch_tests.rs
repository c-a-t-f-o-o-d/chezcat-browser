//! Ordinary missing/undefined/function calls on the real main Document.
use super::*;

#[test]
fn document_own_properties_absent_undefined_and_function_calls_use_ordinary_dispatch() {
    let mut doc=crate::html::parse("<!doctype html><title>Document dispatch seed</title><body><p>Original dispatch fixture</p>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    let outcome=page.console_eval(&mut doc,&mut forms,r#"
var body=document.body,args=0,conversions=0,entered=0,rejected=0;
var request={toString:function(){conversions++;return 'not converted';}};
function checkMissing(error,name){
 if(error.name!=='TypeError'||!(error instanceof TypeError)||error.message.indexOf('Actual value type: undefined.')===-1||error.message.indexOf('Attempted member "'+name+'".')===-1)throw error;
 rejected++;
}
if(document.fixtureAbsentCall!==undefined||'fixtureAbsentCall' in document||Object.hasOwn(document,'fixtureAbsentCall'))throw 'arbitrary missing member exposed';
try{document.fixtureAbsentCall((args++,request));throw 'absent member was callable';}catch(error){checkMissing(error,'fixtureAbsentCall');}
document.fixtureUndefinedCall=undefined;
if(!Object.hasOwn(document,'fixtureUndefinedCall')||!('fixtureUndefinedCall' in document))throw 'own undefined disappeared';
try{document.fixtureUndefinedCall((args++,request));throw 'undefined member was callable';}catch(error){checkMissing(error,'fixtureUndefinedCall');}
document.fixtureOwnCall=function(value){entered++;if(this!==document)throw 'ordinary function receiver';return value;};
if(document.fixtureOwnCall((args++,request))!==request||entered!==1)throw 'own function result/identity';
if(!delete document.fixtureOwnCall||document.fixtureOwnCall!==undefined||Object.hasOwn(document,'fixtureOwnCall')||'fixtureOwnCall' in document)throw 'own function deletion';
try{document.fixtureOwnCall((args++,request));throw 'deleted member was callable';}catch(error){checkMissing(error,'fixtureOwnCall');}
if(args!==4||conversions||rejected!==3||entered!==1||document.body!==body||'matches' in document||'closest' in document||document.matches!==undefined||document.closest!==undefined)throw 'ordinary call order/coercion/interface boundary';
document.title='Document ordinary dispatch passed';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"Document ordinary dispatch passed");
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let(changed,removed,pending)=page.take_image_changes();
    assert!(changed.is_empty()&&removed.is_empty()&&pending.is_empty());
}
