//! Original main-Document own descriptors and existing native/named fallbacks.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<!doctype html><title>Document descriptor seed</title><body><form id=known name=fixtureForm><input id=field name=field value=original></form><p id=content>Original</p>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::address("https://document-descriptor.test/source", &crate::navigation::Location::home()).unwrap();
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert!(page.take_image_changes().2.is_empty());
}

#[test]
fn document_own_properties_empty_numeric_symbol_keys_keep_real_order_and_readdition() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
document.fixtureFirst=1;document[9]='nine';document[2]='two';document['']='empty';document.fixtureLast=3;
var ownKeys=['2','9','fixtureFirst','','fixtureLast'];
function selected(values){return values.filter(function(value){return ownKeys.indexOf(value)!==-1;}).join('|');}
if(selected(Object.keys(document))!=='2|9|fixtureFirst||fixtureLast')throw 'own numeric/string order';
var key=Symbol('document state'),state={kept:true};document[key]=state;
var descriptor=Object.getOwnPropertyDescriptor(document,key);
if(document[key]!==state||descriptor.value!==state||!descriptor.writable||!descriptor.enumerable||!descriptor.configurable)throw 'symbol descriptor';
var symbols=Object.getOwnPropertySymbols(document);if(symbols.length!==1||symbols[0]!==key)throw 'symbol identity/order';
if(!Object.hasOwn(document,'')||!('' in document)||document['']!=='empty')throw 'empty own key';
if(!delete document.fixtureFirst||Object.hasOwn(document,'fixtureFirst')||'fixtureFirst' in document)throw 'delete string own key';
document.fixtureFirst=4;
if(selected(Object.getOwnPropertyNames(document))!=='2|9||fixtureLast|fixtureFirst')throw 'delete/readd order';
if(!delete document[key]||Object.getOwnPropertySymbols(document).length||document[key]!==undefined)throw 'delete own symbol';
if(!delete document['']||Object.hasOwn(document,'')||'' in document||document['']!==undefined)throw 'delete empty own key';
document.title='Document own key order passed';
"#);
    assert_eq!(doc.title, "Document own key order passed");
}

#[test]
fn document_own_properties_accessors_inspection_and_assign_keep_document_receiver() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var getterCalls=0,setterCalls=0,seen=null,record={count:1};document.fixtureRecord=record;
var getter=function(){getterCalls++;seen=this;return this.fixtureRecord.count+1;};
var setter=function(value){setterCalls++;seen=this;this.fixtureRecord.count=value;};
Object.defineProperty(document,'fixtureAccessor',{get:getter,set:setter,enumerable:true,configurable:true});
Object.defineProperty(document,'fixtureHidden',{value:17,writable:true,configurable:true});
var descriptor=Object.getOwnPropertyDescriptor(document,'fixtureAccessor');
if(descriptor.get!==getter||descriptor.set!==setter||!descriptor.enumerable||!descriptor.configurable||'value' in descriptor||getterCalls||setterCalls)throw 'inspection invoked accessor';
if(!Object.hasOwn(document,'fixtureAccessor')||!('fixtureAccessor' in document)||Object.keys(document).indexOf('fixtureAccessor')===-1||getterCalls||setterCalls)throw 'own/has/keys invoked accessor';
if(document.fixtureAccessor!==2||seen!==document||getterCalls!==1)throw 'getter receiver';
document.fixtureAccessor=8;if(record.count!==8||seen!==document||setterCalls!==1)throw 'setter receiver';
var key=Symbol('copy');document[key]=record;
var copied=Object.assign({},document);
if(copied.fixtureRecord!==record||copied.fixtureAccessor!==9||copied[key]!==record||Object.hasOwn(copied,'fixtureHidden')||getterCalls!==2||setterCalls!==1||seen!==document)throw 'copy real enumerable own values';
if(!delete document.fixtureAccessor||Object.getOwnPropertyDescriptor(document,'fixtureAccessor')!==undefined||document.fixtureAccessor!==undefined||getterCalls!==2)throw 'accessor deletion';
Object.defineProperties(document,{fixtureMethod:{value:function(){return this===document;},enumerable:true,configurable:true}});
if(document.fixtureMethod()!==true)throw 'own method actual receiver';
document.title='Document accessor receiver passed';
"#);
    assert_eq!(doc.title, "Document accessor receiver passed");
}

#[test]
fn document_own_properties_data_flags_obey_strict_assignment_and_deletion() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
Object.defineProperty(document,'fixtureFixed',{value:7});
var descriptor=Object.getOwnPropertyDescriptor(document,'fixtureFixed');
if(descriptor.value!==7||descriptor.writable||descriptor.enumerable||descriptor.configurable)throw 'default data flags';
document.fixtureFixed=8;
if(document.fixtureFixed!==7||delete document.fixtureFixed)throw 'sloppy readonly/nonconfigurable';
var errors=0;
try{(function(){'use strict';document.fixtureFixed=9;})();}catch(e){if(e.name==='TypeError')errors++;else throw e;}
try{(function(){'use strict';delete document.fixtureFixed;})();}catch(e){if(e.name==='TypeError')errors++;else throw e;}
try{Object.defineProperty(document,'fixtureFixed',{value:10});}catch(e){if(e.name==='TypeError')errors++;else throw e;}
if(errors!==3||document.fixtureFixed!==7||Object.keys(document).indexOf('fixtureFixed')!==-1||Object.getOwnPropertyNames(document).indexOf('fixtureFixed')===-1)throw 'fixed data contract';
Object.defineProperty(document,'fixtureMutable',{value:11,writable:true,enumerable:true,configurable:true});
Object.defineProperty(document,'fixtureMutable',{value:13,enumerable:false});
descriptor=Object.getOwnPropertyDescriptor(document,'fixtureMutable');
if(descriptor.value!==13||!descriptor.writable||descriptor.enumerable||!descriptor.configurable)throw 'partial descriptor preserves flags';
if(!delete document.fixtureMutable||Object.hasOwn(document,'fixtureMutable'))throw 'configurable data deletion';
document.title='Document data flags passed';
"#);
    assert_eq!(doc.title, "Document data flags passed");
}

#[test]
fn document_own_properties_first_descriptor_symbol_inspection_and_assign_paths_agree() {
    for source in [
        "Object.defineProperty(document,'firstData',{value:19,enumerable:true,configurable:true});if(document.firstData!==19||Object.getOwnPropertyDescriptor(document,'firstData').value!==19)throw 'first descriptor';",
        "var key=Symbol('first');Object.defineProperty(document,key,{value:23,enumerable:true,configurable:true});if(document[key]!==23||Object.getOwnPropertySymbols(document)[0]!==key)throw 'first symbol descriptor';",
        "if(Object.keys(document).indexOf('missingOwn')!==-1||Object.getOwnPropertyNames(document).indexOf('missingOwn')!==-1||Object.getOwnPropertySymbols(document).length||Object.getOwnPropertyDescriptor(document,'missingOwn')!==undefined||Object.hasOwn(document,'missingOwn')||'missingOwn' in document)throw 'fresh inspection';document.laterOwn=29;if(document.laterOwn!==29)throw 'after fresh inspection';",
        "Object.defineProperties(document,{});var state={number:31};if(Object.assign(document,{assignedState:state})!==document||document.assignedState!==state)throw 'first assign';",
    ] {
        let (mut doc, mut forms, mut page) = fixture();
        run(&mut doc, &mut forms, &mut page, source);
        assert_eq!(doc.title, "Document descriptor seed");
    }
}

#[test]
fn document_own_properties_named_form_fallback_survives_undefined_data_and_accessor_shadows() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var nativeForm=document.getElementById('known');
if(document.fixtureForm!==nativeForm||!('fixtureForm' in document))throw 'original named form fallback';
document.fixtureForm=undefined;
if(document.fixtureForm!==undefined||!Object.hasOwn(document,'fixtureForm')||!('fixtureForm' in document))throw 'own undefined must shadow named form';
if(!delete document.fixtureForm||document.fixtureForm!==nativeForm||!('fixtureForm' in document))throw 'delete restores named form';
var calls=0;Object.defineProperty(document,'fixtureForm',{get:function(){calls++;if(this!==document)throw 'shadow getter receiver';return undefined;},enumerable:true,configurable:true});
var descriptor=Object.getOwnPropertyDescriptor(document,'fixtureForm');
if(typeof descriptor.get!=='function'||calls)throw 'shadow inspection invoked getter';
if(document.fixtureForm!==undefined||calls!==1)throw 'own accessor must shadow named form';
if(!delete document.fixtureForm||document.fixtureForm!==nativeForm||calls!==1)throw 'accessor deletion restores named form';
document.fixtureForm={ordinary:true};if(document.fixtureForm===nativeForm||!document.fixtureForm.ordinary)throw 'ordinary state shadow';
delete document.fixtureForm;
if(document.fixtureForm!==nativeForm||document.body!==nativeForm.parentNode)throw 'native fallback identity after shadows';
document.title='Document named fallback passed';
"#);
    assert_eq!(doc.title, "Document named fallback passed");
}

#[test]
fn document_own_properties_keep_existing_native_document_routing_and_refusals() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var nativeBody=document.body,nativeCreate=document.createElement,nativeRoot=document.getRootNode;
document.fixtureState={kept:37};document.title='Document native title kept';
if(document.body!==nativeBody||nativeBody.nodeName!=='BODY'||document.nodeType!==9||document.nodeName!=='#document'||document.getRootNode()!==document||nativeRoot.call(document)!==document)throw 'native Document facts/root';
document.nodeType=1;
var errors=0;try{(function(){'use strict';document.nodeType=2;})();}catch(e){if(e.name==='TypeError')errors++;else throw e;}
if(errors!==1||document.nodeType!==9||document.createElement!==nativeCreate||document.getRootNode!==nativeRoot||document.createElement('div').nodeName!=='DIV')throw 'readonly/native methods';
var rootShadow=function(){return 'ordinary root shadow';};document.getRootNode=rootShadow;
if(document.getRootNode!==rootShadow||document.getRootNode()!=='ordinary root shadow'||nativeRoot.call(document)!==document)throw 'Node method ordinary own override versus canonical call';
if(!delete document.getRootNode||document.getRootNode!==nativeRoot||document.getRootNode()!==document)throw 'Node method deletion restores canonical prototype';
"#);
    for source in [
        "Object.defineProperty(document,'title',{value:'fake title'});",
        "Object.defineProperty(document,'body',{value:{fake:true}});",
        "Object.defineProperty(document,'createElement',{value:function(){return null;}});",
        "document.body=null;",
    ] {
        page.clear_console();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        run(&mut doc, &mut forms, &mut page, "if(document.title!=='Document native title kept'||document.body!==nativeBody||document.createElement!==nativeCreate||document.getRootNode!==nativeRoot||document.getRootNode()!==document||document.fixtureState.kept!==37)throw 'native refusal changed state';");
    }
    assert_eq!(doc.title, "Document native title kept");
}
