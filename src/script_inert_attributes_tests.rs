//! Original connected inert attribute preparation and transfer cases.
use super::*;
fn page()->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document("<html><body><div id='mount'></div><template id='seed'><span id='item' data-note='one' on-click='ordinary' on-keydown='keys' onclick='bad()'><b></b></span></template></body></html>",&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn execute(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){let outcome=page.console_eval(doc,forms,source);assert_eq!(outcome.errors,0,"{}",page.report());}
#[test]
fn inert_attributes_names_removal_clone_import_and_live_collections_keep_real_source_owners(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var content=document.getElementById('seed').content,element=content.children[0],view=element.children,clone=element.cloneNode(true);
var names=element.getAttributeNames();if(!Array.isArray(names)||names.join(',')!=='id,data-note,on-click,on-keydown,onclick')throw 'ordered qualified names';
if(!element.hasAttribute('ONCLICK')||element.getAttribute('on-click')!=='ordinary')throw 'real attribute lookup';
element.removeAttribute({toString:function(){return 'ONCLICK';}});element.removeAttribute('id');element.removeAttribute('missing');
if(element.hasAttribute('onclick')||element.id!==''||element.getAttribute('id')!==null||element.getAttributeNames().join(',')!=='data-note,on-click,on-keydown'||names.length!==5)throw 'actual removal and snapshot';
if(!clone.hasAttribute('onclick')||clone.getAttribute('id')!=='item'||clone.children.length!==1||element.children!==view||view.length!==1)throw 'copy source and topology identity';
var imported=document.importNode(content,true),copy=imported.children[0];
if(copy.getAttribute('on-click')!=='ordinary'||copy.getAttribute('on-keydown')!=='keys'||copy.getAttribute('onclick')!==null||copy.ownerDocument!==document||element.ownerDocument===document)throw 'declarative data transfer';
document.getElementById('mount').appendChild(imported);if(!copy.isConnected||content.children[0]!==element||view[0]!==element.firstElementChild)throw 'real connection';
var target=document.createElement('div');target.innerHTML='<span only="data" on-click="declarative" on-keydown="keys"></span>';var child=target.firstElementChild;
if(child.getAttributeNames().join(',')!=='only,on-click,on-keydown'||child.cloneNode(true).getAttribute('only')!=='data')throw 'ordinary names across innerHTML clone';
"#);
}
#[test]
fn inert_attributes_names_and_removal_brand_prototype_and_noop_contracts(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var e=document.getElementById('seed').content.children[0],P=Element.prototype,d=Object.getOwnPropertyDescriptor(P,'getAttributeNames'),method=d.value;
if(method.name!=='getAttributeNames'||method.length!==0||!d.enumerable||!d.writable||!d.configurable)throw 'name method descriptor';
for(var value of [{},document,document.createDocumentFragment(),document.createTextNode('x')]){var caught=false;try{method.call(value);}catch(error){caught=error instanceof TypeError;}if(!caught)throw 'name method brand';}
var conversions=0;method.call(e,{toString:function(){conversions++;throw 'extra';}});if(conversions!==0)throw 'extra argument coercion';
P.getAttributeNames=function(){return ['replacement'];};if(e.getAttributeNames()[0]!=='replacement')throw 'ordinary override';delete P.getAttributeNames;if(e.getAttributeNames!==undefined)throw 'deleted name method';Object.defineProperty(P,'getAttributeNames',d);
var caught=false;try{e.removeAttribute();}catch(error){caught=error instanceof TypeError;}if(!caught||!e.hasAttribute('onclick'))throw 'required name';
var fresh=e.ownerDocument.createElement('p');if(fresh.getAttributeNames().length!==0||fresh.hasAttribute('none'))throw 'fresh empty attributes';fresh.removeAttribute('none');
"#);
}
#[test]
fn inert_attributes_source_copy_refusal_preserves_identity_storage_and_recovers(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,"var inspected=document.getElementById('seed').content.children[0];");
    let Value::Host(handle)=page.runtime.execute("inspected",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap()else{panic!("node")};
    page.begin_job();let before=page.state.retained;let generated=page.state.inert_documents.generated.len();let backing=page.state.inert_documents.backing_bytes();let limit=page.state.retained_limit;
    page.state.retained_limit=before;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.attribute_names(handle).is_err());
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.remove_inert_attribute(handle,&Value::String("onclick".into())).is_err());
    assert_eq!(page.state.retained,before);assert_eq!(page.state.inert_documents.generated.len(),generated);
    page.state.retained_limit=limit;page.begin_job();page.state.copy_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.remove_inert_attribute(handle,&Value::String("onclick".into())).is_err());
    assert_eq!(page.state.retained,before);assert_eq!(page.state.inert_documents.generated.len(),generated);
    page.begin_job();execute(&mut doc,&mut forms,&mut page,"if(!inspected.hasAttribute('onclick'))throw 'refusal changed source';inspected.removeAttribute('onclick');if(inspected.hasAttribute('onclick'))throw 'recovery';");
    assert_eq!(page.state.inert_documents.generated.len(),generated+1);
    let node=&page.state.inert_documents.generated[generated].node;
    let NodeKind::Element{tag,href,..}=&node.kind else{panic!("element source")};
    let payload=node_bytes(node)+tag.capacity()+href.as_ref().map_or(0,String::capacity)+size_of::<Node>()+2*size_of::<usize>();
    assert_eq!(page.state.retained,before+payload+page.state.inert_documents.backing_bytes()-backing);
}
#[test]
fn inert_attributes_handler_classification_uses_exact_native_names(){
    use crate::html::AttributeName;
    for name in ["onclick","onkeydown","onpointerdown","onanimationend"] {assert!(script_element_properties::event_handler_content_attribute("span",&AttributeName::unqualified(name.into())));}
    for name in ["on-click","on-keydown","only","onunknown"] {assert!(!script_element_properties::event_handler_content_attribute("span",&AttributeName::unqualified(name.into())));}
    assert!(script_element_properties::event_handler_content_attribute("body",&AttributeName::unqualified("onhashchange".into())));
    assert!(!script_element_properties::event_handler_content_attribute("span",&AttributeName::unqualified("onhashchange".into())));
}
