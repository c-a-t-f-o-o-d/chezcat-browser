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

#[test]
fn inert_attribute_identities_live_map_removal_and_clone_preserve_real_values_and_documents(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var content=document.getElementById('seed').content,e=content.children[0],map=e.attributes,owner=e.ownerDocument;
if(!(map instanceof NamedNodeMap)||Object.prototype.toString.call(map)!=='[object NamedNodeMap]')throw 'genuine map interface';
if(map!==e.attributes||map.length!==5||map[0]!==e.getAttributeNode('id')||map.item(0)!==map[0]||map.item(99)!==null||map[99]!==undefined)throw 'real stable attribute map';
var handler=map.getNamedItem('onclick'),note=map.getNamedItemNS(null,'data-note');
if(handler!==e.getAttributeNode('ONCLICK')||handler.nodeType!==2||handler.name!=='onclick'||handler.value!=='bad()'||handler.ownerElement!==e||handler.ownerDocument!==owner||note.value!=='one'||map.getNamedItem('missing')!==null)throw 'real Attr values';
var snapshot=Array.from(map),clone=e.cloneNode(true);if(snapshot.length!==5||snapshot[4]!==handler||clone.attributes[4]===handler)throw 'attribute snapshot and clone identities';
e.removeAttribute('onclick');
if(map!==e.attributes||map.length!==4||map.getNamedItem('onclick')!==null||handler.ownerElement!==null||handler.ownerDocument!==owner||handler.value!=='bad()'||!clone.hasAttribute('onclick')||note.ownerElement!==e||note.value!=='one')throw 'actual detached Attr';
handler.value='detached';if(handler.value!=='detached'||e.hasAttribute('onclick'))throw 'detached Attr mutation';
"#);
}
#[test]
fn inert_attribute_identities_parked_promotion_retains_map_attr_and_adopts_attached_document(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var template=document.createElement('template');template.innerHTML='<span data-note="kept" on-click="ordinary"><b></b></span>';
var f=template.content,e=f.children[0],map=e.attributes,note=map[0],owner=e.ownerDocument;
if(note.value!=='kept'||note.ownerElement!==e||note.ownerDocument!==owner||map.getNamedItem('data-note')!==note)throw 'parked Attr provenance';
document.getElementById('mount').appendChild(f);
if(e.attributes!==map||map[0]!==note||e.getAttributeNode('data-note')!==note||note.ownerDocument!==document||note.ownerElement!==e||!e.isConnected)throw 'real promotion retains identities';
e.removeAttribute('data-note');if(map.length!==1||map[0].name!=='on-click'||note.ownerElement!==null||note.ownerDocument!==document||note.value!=='kept')throw 'main removal of promoted Attr';
"#);
}
#[test]
fn inert_attribute_identities_lookup_refusal_publishes_no_partial_identity_and_recovers(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,"var inspected=document.getElementById('seed').content.children[0];");
    let Value::Host(owner)=page.runtime.execute("inspected",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap()else{panic!("owner")};
    page.begin_job();let retained=page.state.retained;let limit=page.state.retained_limit;let records=page.state.attributes.len();let index=page.state.attached_attributes.clone();
    page.state.retained_limit=retained;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.lookup_owner_attribute_node_at(owner,Some(0)).is_err());
    assert_eq!(page.state.attributes.len(),records);assert_eq!(page.state.attached_attributes,index);assert_eq!(page.state.retained,retained);
    page.state.retained_limit=limit;page.begin_job();execute(&mut doc,&mut forms,&mut page,"var actual=inspected.attributes[0];if(actual!==inspected.getAttributeNode('id')||actual.ownerElement!==inspected)throw 'identity recovery';");
    let retained=page.state.retained;let attached=page.state.attached_attributes.clone();page.state.retained_limit=retained;page.begin_job();
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.remove_inert_attribute(owner,&Value::String("id".into())).is_err());
    assert_eq!(page.state.retained,retained);assert_eq!(page.state.attached_attributes,attached);assert_eq!(page.state.attributes[records].owner,Some(owner));
    page.state.retained_limit=limit;page.begin_job();execute(&mut doc,&mut forms,&mut page,"inspected.removeAttribute('id');if(actual.ownerElement!==null||actual.value!=='item'||actual.ownerDocument!==inspected.ownerDocument)throw 'detachment recovery';");
    for handle in [0xc100_0000,0xc200_0000,script_named_map::handle(ENTRY_END-1)]{assert!(!Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.named_map_receiver(handle));}

}

#[test]
fn inert_attribute_identities_map_interface_brands_and_ordinary_prototype_changes_are_real(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var e=document.getElementById('seed').content.children[0],map=e.attributes,P=NamedNodeMap.prototype,d=Object.getOwnPropertyDescriptor(P,'length');
var conversions=0;e.attributes={toString:function(){conversions++;return 'ignored';}};map.length={valueOf:function(){conversions++;return 0;}};if(conversions!==0||e.attributes!==map||map.length!==5)throw 'readonly map and owner attributes';
if(d.get.name!=='get length'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||P.item.length!==1||P.getNamedItemNS.length!==2||Object.prototype.hasOwnProperty.call(map,'length'))throw 'map canonical descriptors';
var calls=0,arg={toString:function(){calls++;return 'id';},valueOf:function(){calls++;return 0;}};
for(var method of [d.get,P.item,P.getNamedItem,P.getNamedItemNS]){var caught=false;try{method.call({},arg,arg);}catch(error){caught=error instanceof TypeError;}if(!caught||calls!==0)throw 'map invalid receiver before coercion';}
var caught=false;try{new NamedNodeMap();}catch(error){caught=error instanceof TypeError;}if(!caught)throw 'illegal map constructor';
Object.defineProperty(P,'length',{value:7,configurable:true});if(map.length!==7)throw 'map length replacement';delete P.length;if(map.length!==undefined||'length' in map)throw 'map length resurrection';Object.defineProperty(P,'length',d);
var original=P.item;P.item=function(){return 17;};if(map.item(0)!==17)throw 'map method replacement';delete P.item;if(map.item!==undefined)throw 'map method resurrection';P.item=original;
var iterator=P[Symbol.iterator];P[Symbol.iterator]=function(){return {next:function(){return {done:true};}};};if(Array.from(map).length!==0)throw 'map iterator replacement';P[Symbol.iterator]=iterator;if(Array.from(map)[0]!==map[0])throw 'map iterator restoration';
"#);
}
