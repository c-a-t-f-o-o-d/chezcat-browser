//! Genuine initial-blank html/body identities and their owner-realm prototypes.
//! No child DOM allocation, navigation or expando storage is introduced.
use super::*;

fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
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
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<!doctype html><title>Original container owners</title><body><main id=main><div id=container></div></main><form id=form><input id=neighbor name=neighbor value=original></form>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = location("https://container-owner.test/source");
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
    let value = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    value.cursor = 5;
    value.anchor = 2;
    value.offset = 1;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,F=w.Function,childDocument=w.document,childHtml=childDocument.documentElement,childBody=childDocument.body;
var stringify=Object.prototype.toString,childStringify=F('return Object.prototype.toString;')(),tag=Symbol.toStringTag;
var read=F('node','return node.toString;'),invoke=F('method','node','return method.call(node);');
"#,
    );
    (doc, forms, page)
}

#[test]
fn container_object_realms_blank_html_body_use_genuine_owner_interfaces_and_intrinsics() {
    let (mut doc, mut forms, mut page) = fixture();
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let neighbor = id(&doc, "neighbor");
    let preserved = editor(&forms, neighbor);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var H=w.HTMLHtmlElement,B=w.HTMLBodyElement,HP=H.prototype,BP=B.prototype;
if(H===HTMLHtmlElement||B===HTMLBodyElement||HP===HTMLHtmlElement.prototype||BP===HTMLBodyElement.prototype)throw 'realm prototype identity';
if(Object.getPrototypeOf(H)!==w.HTMLElement||Object.getPrototypeOf(B)!==w.HTMLElement||Object.getPrototypeOf(HP)!==w.HTMLElement.prototype||Object.getPrototypeOf(BP)!==w.HTMLElement.prototype)throw 'genuine child inheritance';
if(Object.getPrototypeOf(childHtml)!==HP||Object.getPrototypeOf(childBody)!==BP||childHtml.constructor!==H||childBody.constructor!==B)throw 'actual immediate interfaces';
if(!(childHtml instanceof H)||!(childBody instanceof B)||!(childHtml instanceof w.HTMLElement)||!(childBody instanceof w.Element)||childHtml instanceof HTMLHtmlElement||childBody instanceof HTMLBodyElement)throw 'owner interface instanceof';
if(childHtml.toString!==childStringify||childBody.toString!==childStringify||read(document.body)!==stringify||childStringify===stringify)throw 'owner intrinsic lookup';
if(childDocument.head[tag]!==undefined)throw 'unpromoted head acquired a symbol backing';
for(var n of [childHtml,childBody]){
 var expected=n===childHtml?'HTMLHtmlElement':'HTMLBodyElement';
 if(n[tag]!==expected||n.toString()!=='[object '+expected+']'||stringify.call(n)!=='[object '+expected+']'||invoke(stringify,n)!=='[object '+expected+']')throw 'real child tag';
 if(Object.hasOwn(n,'toString')||Object.hasOwn(n,'constructor')||Object.hasOwn(n,tag))throw 'fabricated native own metadata';
}
var HD=Object.getOwnPropertyDescriptor(HP,tag),BD=Object.getOwnPropertyDescriptor(BP,tag);
for(var d of [HD,BD])if(d.writable||d.enumerable||!d.configurable)throw 'IDL tag descriptor';
BP.toString=function(){if(this!==childBody)throw 'child value receiver';return 'author child value';};
if(childBody.toString()!=='author child value')throw 'ordinary value override';delete BP.toString;
if(childBody.toString!==childStringify)throw 'deleted override reveals owner intrinsic';
var constructorDescriptor=Object.getOwnPropertyDescriptor(HP,'constructor');HP.constructor=37;
if(childHtml.constructor!==37)throw 'ordinary constructor value';delete HP.constructor;
if(childHtml.constructor!==w.HTMLElement)throw 'deleted constructor follows actual chain';Object.defineProperty(HP,'constructor',constructorDescriptor);
var childObjectPrototype=w.Object.prototype,stringDescriptor=Object.getOwnPropertyDescriptor(childObjectPrototype,'toString');
delete childObjectPrototype.toString;
if(childBody.toString!==undefined||('toString' in childBody)||childHtml.toString!==undefined||('toString' in childHtml))throw 'deleted intrinsic resurrected';
Object.defineProperty(childObjectPrototype,'toString',stringDescriptor);
F('HTMLBodyElement=17;delete this.HTMLHtmlElement;')();
if(childBody.constructor!==B||childHtml.constructor!==H||childBody.toString()!=='[object HTMLBodyElement]'||childHtml.toString()!=='[object HTMLHtmlElement]')throw 'canonical prototype survives global replacement';
"#,
    );
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(editor(&forms, neighbor), preserved);
    assert_eq!(forms.text_default_value(neighbor).unwrap(), "original");
}

#[test]
fn container_object_realms_prototype_changes_and_cached_reads_revalidate_actual_caller() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var mainBody=document.body,MP=Object.getPrototypeOf(mainBody),CP=Object.getPrototypeOf(childBody),mainReads=0,mainTags=0;
var mainTagDescriptor=Object.getOwnPropertyDescriptor(MP,tag),childTagDescriptor=Object.getOwnPropertyDescriptor(CP,tag);
Object.defineProperty(MP,'toString',{configurable:true,get:function(){if(this!==mainBody)throw 'main receiver';mainReads++;return stringify;}});
Object.defineProperty(MP,tag,{configurable:true,get:function(){if(this!==mainBody)throw 'main tag receiver';mainTags++;return 'OriginalMainContainer';}});
var childState=F(
 "var state={reads:0,tags:0};"+
 "Object.defineProperty(HTMLBodyElement.prototype,'toString',{configurable:true,get:function(){"+
 " if(this!==document.body)throw 'child receiver';state.reads++;return Object.prototype.toString;"+
 "}});"+
 "Object.defineProperty(HTMLBodyElement.prototype,Symbol.toStringTag,{configurable:true,get:function(){"+
 " if(this!==document.body)throw 'child tag receiver';state.tags++;return 'OriginalChildContainer';"+
 "}});"+
 "return state;"
)();
var own=F('return document.body.ownerDocument===document&&document.documentElement.ownerDocument===document&&document.body.previousSibling===document.head;');
if(read(mainBody)!==stringify||invoke(childStringify,mainBody)!=='[object OriginalMainContainer]'||mainReads!==1||mainTags!==1)throw 'same-origin borrowed main';
if(childBody.toString()!=='[object OriginalChildContainer]')throw 'same-origin child overrides';
mainReads=0;mainTags=0;childState.reads=0;childState.tags=0;
"#,
    );
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let neighbor = id(&doc, "neighbor");
    let preserved = editor(&forms, neighbor);
    page.set_location(&location("https://different-container.test/changed"));
    for source in [
        "childBody.toString;",
        "childBody.toString();",
        "childBody[Symbol.toStringTag];",
        "Object.getPrototypeOf(childBody);",
        "childBody.constructor;",
        "read(mainBody);",
        "invoke(stringify,mainBody);",
        "invoke(childStringify,mainBody);",
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
        assert_eq!(format!("{:?}", doc.nodes), nodes);
        assert_eq!(page.state.dom_commits, commits);
        assert_eq!(editor(&forms, neighbor), preserved);
        run(&mut doc, &mut forms, &mut page, "if(!own()||mainReads!==0||mainTags!==0||childState.reads!==0||childState.tags!==0)throw 'refusal ran author metadata or lost owner realm';");
    }
    page.set_location(&location("https://container-owner.test/source"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if(read(mainBody)!==stringify||invoke(childStringify,mainBody)!=='[object OriginalMainContainer]'||mainReads!==1||mainTags!==1||childBody.toString()!=='[object OriginalChildContainer]')throw 'same-origin recovery';
delete MP.toString;delete CP.toString;Object.defineProperty(MP,tag,mainTagDescriptor);Object.defineProperty(CP,tag,childTagDescriptor);
if(mainBody.toString()!=='[object HTMLBodyElement]'||childBody.toString()!=='[object HTMLBodyElement]'||Object.hasOwn(childBody,'toString'))throw 'descriptor restoration';
"#,
    );
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(editor(&forms, neighbor), preserved);
}

#[test]
fn container_object_realms_child_construction_and_own_storage_remain_unimplemented() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, "if(childHtml.toString()!=='[object HTMLHtmlElement]'||childBody.toString()!=='[object HTMLBodyElement]')throw 'admitted concrete child controls';");
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let neighbor = id(&doc, "neighbor");
    let preserved = editor(&forms, neighbor);
    for source in [
        "new w.HTMLBodyElement();",
        "new w.HTMLHtmlElement();",
        "new w.Text('unavailable');",
        "new w.DocumentFragment();",
        "childDocument.createElement('div');",
        "childDocument.createElementNS('http://www.w3.org/1999/xhtml','div');",
        "Object.getPrototypeOf(childDocument.head);",
        "childDocument.head.toString();",
        "childBody.innerHTML='<div>unavailable</div>';",
        "childBody.arbitraryContainerState=17;",
        "childBody.toString=17;",
        "childBody.constructor=17;",
        "childBody.__proto__=null;",
        "childBody.__proto__={};",
        "Object.defineProperty(childBody,'toString',{value:function(){return 'unavailable';}});",
        "Object.defineProperty(childBody,Symbol.toStringTag,{value:'unavailable',configurable:true});",
    ] {
        page.clear_console();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert_eq!(format!("{:?}", doc.nodes), nodes);
        assert_eq!(page.state.dom_commits, commits);
        assert_eq!(editor(&forms, neighbor), preserved);
        run(&mut doc, &mut forms, &mut page, "if(childBody.firstChild!==null||childHtml.firstChild!==childDocument.head||childBody.ownerDocument!==childDocument||childBody.toString()!=='[object HTMLBodyElement]')throw 'unsupported operation changed admitted child';");
    }
}

#[test]
fn container_object_realms_inherited_symbol_presence_deletion_and_getters_keep_native_receiver() {
    let (mut doc, mut forms, mut page) = fixture();
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let neighbor = id(&doc, "neighbor");
    let preserved = editor(&forms, neighbor);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var CP=Object.getPrototypeOf(childBody),tagDescriptor=Object.getOwnPropertyDescriptor(CP,tag);
if(!(tag in childBody)||Object.hasOwn(childBody,tag))throw 'inherited symbol presence';
if(!delete childBody[tag]||!(tag in childBody)||Object.hasOwn(childBody,tag))throw 'delete of absent own symbol';
if(CP[tag]!=='HTMLBodyElement'||childBody[tag]!=='HTMLBodyElement')throw 'delete changed inherited tag';

var tagReads=0,coercions=0,nonString={toString:function(){coercions++;throw 'tag conversion';}};
var nonStringGetter=function(){
 if(this!==childBody)throw 'symbol getter receiver';
 tagReads++;
 return nonString;
};
Object.defineProperty(CP,tag,{configurable:true,get:nonStringGetter});
if(!(tag in childBody)||Object.hasOwn(childBody,tag)||tagReads!==0)throw 'presence invoked inherited getter';
if(childBody[tag]!==nonString||tagReads!==1)throw 'symbol getter result identity';
if(childBody.toString()!=='[object Object]'||tagReads!==2||coercions!==0)throw 'non-string tag must not be coerced';
if(!delete childBody[tag]||tagReads!==2||Object.getOwnPropertyDescriptor(CP,tag).get!==nonStringGetter)throw 'delete changed prototype accessor';

var marker={original:'child tag getter'},throws=0,caught=0;
Object.defineProperty(CP,tag,{configurable:true,get:function(){
 if(this!==childBody)throw 'throwing getter receiver';
 throws++;
 throw marker;
}});
if(!(tag in childBody)||Object.hasOwn(childBody,tag)||!delete childBody[tag]||throws!==0)throw 'presence/delete ran throwing getter';
try{childBody[tag];}catch(error){if(error!==marker)throw error;caught++;}
try{stringify.call(childBody);}catch(error){if(error!==marker)throw error;caught++;}
if(caught!==2||throws!==2||Object.hasOwn(childBody,tag))throw 'throw identity or own storage';
Object.defineProperty(CP,tag,tagDescriptor);
if(childBody.toString()!=='[object HTMLBodyElement]'||!(tag in childBody))throw 'tag descriptor restoration';
"#,
    );
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(editor(&forms, neighbor), preserved);
}

#[test]
fn container_object_realms_inherited_author_setters_retain_rhs_without_own_native_storage() {
    let (mut doc, mut forms, mut page) = fixture();
    let nodes = format!("{:?}", doc.nodes);
    let commits = page.state.dom_commits;
    let neighbor = id(&doc, "neighbor");
    let preserved = editor(&forms, neighbor);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var CP=Object.getPrototypeOf(childBody),tagDescriptor=Object.getOwnPropertyDescriptor(CP,tag);
var rhs={retained:'original child RHS'},received=null,retained=null,stringSets=0,stringGets=0;
Object.defineProperty(CP,'toString',{configurable:true,
 get:function(){stringGets++;return childStringify;},
 set:function(value){received=this;retained=value;stringSets++;}
});
childBody.toString=rhs;
if(stringSets!==1||stringGets!==0||received!==childBody||retained!==rhs)throw 'ordinary string setter inputs';
if(Object.hasOwn(childBody,'toString')||childBody.ownerDocument!==childDocument)throw 'string setter fabricated own storage';
delete CP.toString;
if(childBody.toString!==childStringify)throw 'string setter restoration';

var originalPrototype=childBody.__proto__;
childBody.__proto__=7;
if(childBody.__proto__!==originalPrototype||Object.getPrototypeOf(childBody)!==CP||Object.hasOwn(childBody,'__proto__'))throw 'legacy primitive prototype setter';
var protoSets=0;
Object.defineProperty(CP,'__proto__',{configurable:true,
 get:function(){return CP;},
 set:function(value){received=this;retained=value;protoSets++;}
});
childBody.__proto__=rhs;
if(protoSets!==1||received!==childBody||retained!==rhs||Object.getPrototypeOf(childBody)!==CP||Object.hasOwn(childBody,'__proto__'))throw 'author prototype setter inputs';
delete CP.__proto__;
if(childBody.__proto__!==CP)throw 'legacy prototype accessor restoration';

var coercions=0,readonlyRhs={toString:function(){coercions++;throw 'unwanted tag conversion';}};
childBody[tag]=readonlyRhs;
if(childBody[tag]!=='HTMLBodyElement'||Object.hasOwn(childBody,tag)||coercions!==0)throw 'sloppy inherited readonly tag';
var strictReadonly=0;
try{(function(node,value){'use strict';node[tag]=value;})(childBody,readonlyRhs);}
catch(error){if(error.name!=='TypeError')throw error;strictReadonly++;}
if(strictReadonly!==1||coercions!==0||childBody[tag]!=='HTMLBodyElement')throw 'strict inherited readonly tag';

var symbolSets=0,symbolGets=0;
Object.defineProperty(CP,tag,{configurable:true,
 get:function(){symbolGets++;return 'AuthorChildTag';},
 set:function(value){received=this;retained=value;symbolSets++;}
});
childBody[tag]=rhs;
if(symbolSets!==1||symbolGets!==0||received!==childBody||retained!==rhs||Object.hasOwn(childBody,tag))throw 'symbol setter inputs or storage';
if(childBody.toString()!=='[object AuthorChildTag]'||symbolGets!==1)throw 'symbol accessor result';
delete CP[tag];
if(childBody[tag]!=='HTMLElement'||childBody.toString()!=='[object HTMLElement]')throw 'deleted concrete tag must follow inherited chain';
Object.defineProperty(CP,tag,{value:'WritableChildTag',writable:true,configurable:true});
"#,
    );
    page.clear_console();
    let outcome = page.console_eval(&mut doc, &mut forms, "childBody[tag]=rhs;");
    assert_eq!(outcome.errors, 1, "{}", page.report());
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if(CP[tag]!=='WritableChildTag'||childBody[tag]!=='WritableChildTag'||Object.hasOwn(childBody,tag)||retained!==rhs)throw 'refused data write changed prototype or retained value';
Object.defineProperty(CP,tag,tagDescriptor);
if(childBody.toString()!=='[object HTMLBodyElement]'||Object.getPrototypeOf(childBody)!==CP)throw 'writable tag refusal recovery';
"#,
    );
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(editor(&forms, neighbor), preserved);
}
