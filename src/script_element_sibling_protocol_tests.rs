//! Original canonical descriptors, brands and live physical-sibling controls.
use super::*;

fn fixture()->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<!doctype html><title>Sibling protocol</title><body><div id=first><span id=a>A</span>middle<span id=b>B</span>tail<span id=c>C</span></div><div id=second></div><form id=f><input id=neighbor name=neighbor value=original></form>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    let location=crate::navigation::address("https://element-sibling.test/source",&crate::navigation::Location::home()).unwrap();
    page.set_location(&location);page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    (doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let out=page.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{source}\n{}",page.report());
    assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
}
fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some(name)).unwrap()}

#[test]
fn element_siblings_canonical_interfaces_descriptors_and_declaring_family_brands() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var a=document.getElementById('a'),b=document.getElementById('b'),text=a.nextSibling,comment=document.createComment('real'),fragment=document.createDocumentFragment(),attr=document.createAttribute('original');
if(!(text instanceof CharacterData)||!(comment instanceof CharacterData)||!(text instanceof Node)||text instanceof Element||a instanceof CharacterData)throw 'genuine CharacterData suffix';
if(Object.getPrototypeOf(CharacterData.prototype)!==Node.prototype||Object.getPrototypeOf(CharacterData)!==Node||CharacterData.prototype.constructor!==CharacterData)throw 'canonical CharacterData parent';
var failures=0,touched=0,fake={get nodeType(){touched++;return 1;}};
for(var name of ['nextElementSibling','previousElementSibling']){
 var e=Object.getOwnPropertyDescriptor(Element.prototype,name),d=Object.getOwnPropertyDescriptor(CharacterData.prototype,name);
 for(var descriptor of [e,d]){
  if(typeof descriptor.get!=='function'||descriptor.get.name!=='get '+name||descriptor.get.length!==0||descriptor.set!==undefined||!descriptor.enumerable||!descriptor.configurable||'value' in descriptor)throw 'canonical sibling accessor descriptor';
  try{new descriptor.get();throw 'getter was constructor';}catch(error){if(error.name!=='TypeError')throw error;failures++;}
 }
 if(Object.getOwnPropertyDescriptor(Node.prototype,name)!==undefined||Object.hasOwn(a,name)||Object.hasOwn(text,name))throw 'wrong descriptor placement';
 for(var receiver of [document,fragment,attr,window,fake,text,comment])try{e.get.call(receiver);throw 'Element getter wrong brand';}catch(error){if(error.name!=='TypeError')throw error;failures++;}
 for(var receiver of [document,fragment,attr,window,fake,a])try{d.get.call(receiver);throw 'CharacterData getter wrong brand';}catch(error){if(error.name!=='TypeError')throw error;failures++;}
}
try{new CharacterData();throw 'CharacterData constructor exposed';}catch(error){if(error.name!=='TypeError')throw error;failures++;}
try{CharacterData();throw 'CharacterData callable factory exposed';}catch(error){if(error.name!=='TypeError')throw error;failures++;}
if(failures!==32||touched||'nextElementSibling' in document||'previousElementSibling' in document||'nextElementSibling' in fragment||'previousElementSibling' in fragment)throw 'brand/noninclusion boundary';
if(a.nextElementSibling!==b||text.previousElementSibling!==a||text.nextElementSibling!==b||comment.nextElementSibling!==null||comment.previousElementSibling!==null)throw 'canonical getter results';
"#);
}

#[test]
fn element_siblings_readonly_element_own_shadow_and_real_prototype_override_deletion() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var a=document.getElementById('a'),b=document.getElementById('b'),text=a.nextSibling;
var e=Object.getOwnPropertyDescriptor(Element.prototype,'nextElementSibling'),d=Object.getOwnPropertyDescriptor(CharacterData.prototype,'nextElementSibling');
a.nextElementSibling=17;text.nextElementSibling=19;
if(a.nextElementSibling!==b||text.nextElementSibling!==b||Object.hasOwn(a,'nextElementSibling')||Object.hasOwn(text,'nextElementSibling'))throw 'sloppy readonly';
var errors=0;for(var target of [a,text])try{(function(value){'use strict';value.nextElementSibling=23;})(target);}catch(error){if(error.name!=='TypeError')throw error;errors++;}
if(errors!==2)throw 'strict readonly';
var state={ordinary:true};Object.defineProperty(a,'nextElementSibling',{value:state,writable:true,enumerable:true,configurable:true});
if(a.nextElementSibling!==state||e.get.call(a)!==b||!Object.hasOwn(a,'nextElementSibling'))throw 'Element own shadow versus native getter';
if(!delete a.nextElementSibling||a.nextElementSibling!==b)throw 'Element own deletion';
var calls=0;Object.defineProperty(Element.prototype,'nextElementSibling',{get:function(){calls++;if(this!==a)throw 'override receiver';return state;},configurable:true});
if(a.nextElementSibling!==state||calls!==1||text.nextElementSibling!==b||e.get.call(a)!==b)throw 'live Element prototype override';
delete Element.prototype.nextElementSibling;
if(a.nextElementSibling!==undefined||'nextElementSibling' in a||text.nextElementSibling!==b)throw 'ghost Element getter after deletion';
Object.defineProperty(Element.prototype,'nextElementSibling',e);
Object.defineProperty(CharacterData.prototype,'nextElementSibling',{value:state,writable:true,configurable:true});
if(text.nextElementSibling!==state||a.nextElementSibling!==b||d.get.call(text)!==b)throw 'live CharacterData prototype override';
delete CharacterData.prototype.nextElementSibling;
if(text.nextElementSibling!==undefined||'nextElementSibling' in text)throw 'ghost CharacterData getter after deletion';
Object.defineProperty(CharacterData.prototype,'nextElementSibling',d);
if(a.nextElementSibling!==b||text.nextElementSibling!==b||calls!==1)throw 'canonical descriptor restoration';
"#);
}

#[test]
fn element_siblings_follow_moves_detached_subtrees_and_real_fragment_splices_without_editor_reset() {
    let(mut doc,mut forms,mut page)=fixture();let neighbor=id(&doc,"neighbor");
    forms.script_set_value(neighbor,"edited neighbor").unwrap();
    forms.controls.get_mut(&neighbor).unwrap().value.cursor=5;
    forms.controls.get_mut(&neighbor).unwrap().value.anchor=2;
    let old=&forms.controls[&neighbor];let before=(old.value.text(),old.value.cursor,old.value.anchor,old.value.offset,old.dirty_value,old.owner);
    run(&mut doc,&mut forms,&mut page,r#"
var first=document.getElementById('first'),second=document.getElementById('second'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),text=a.nextSibling,marker=document.createComment('moving');
first.insertBefore(marker,b);
if(a.nextElementSibling!==b||marker.previousElementSibling!==a||marker.nextElementSibling!==b)throw 'initial live siblings';
first.removeChild(b);if(a.nextElementSibling!==c||b.nextElementSibling!==null||b.previousElementSibling!==null||marker.nextElementSibling!==c)throw 'retired standalone siblings';
second.appendChild(b);second.appendChild(a);
if(b.nextElementSibling!==a||a.previousElementSibling!==b||marker.previousElementSibling!==null||marker.nextElementSibling!==c)throw 'whole-node move live order';
document.body.removeChild(first);
if(first.isConnected||c.isConnected||text.nextElementSibling!==c||marker.previousElementSibling!==null||c.previousElementSibling!==null||c.ownerDocument!==document)throw 'detached subtree physical siblings';
document.body.appendChild(first);
if(!first.isConnected||!marker.isConnected||text.nextElementSibling!==c)throw 'reconnected physical siblings';
var donor=document.createDocumentFragment(),lead=document.createTextNode('lead'),fresh=document.createElement('span'),end=document.createComment('end');
donor.appendChild(lead);donor.appendChild(fresh);donor.appendChild(end);
if(lead.nextElementSibling!==fresh||end.previousElementSibling!==fresh||fresh.previousElementSibling!==null||fresh.nextElementSibling!==null)throw 'genuine fragment topology';
second.appendChild(donor);
if(donor.childNodes.length||fresh.parentNode!==second||a.nextElementSibling!==fresh||fresh.previousElementSibling!==a||lead.previousElementSibling!==a||end.previousElementSibling!==fresh||end.nextElementSibling!==null)throw 'fragment donor splice identities';
if(document.getElementById('neighbor').value!=='edited neighbor'||document.getElementById('neighbor').defaultValue!=='original')throw 'neighbor native state';
"#);
    let now=&forms.controls[&neighbor];assert_eq!((now.value.text(),now.value.cursor,now.value.anchor,now.value.offset,now.dirty_value,now.owner),before);
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");
}

#[test]
fn element_siblings_keep_named_form_controls_separate_from_saved_canonical_getters() {
    let mut doc=crate::html::parse("<!doctype html><body><span id=before></span><form id=f><input id=next name=nextElementSibling value=next><input id=previous name=previousElementSibling value=previous></form><span id=after></span>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    run(&mut doc,&mut forms,&mut page,r#"
var form=document.getElementById('f'),next=document.getElementById('next'),previous=document.getElementById('previous');
var getNext=Object.getOwnPropertyDescriptor(Element.prototype,'nextElementSibling').get,getPrevious=Object.getOwnPropertyDescriptor(Element.prototype,'previousElementSibling').get;
if(form.nextElementSibling!==next||form.previousElementSibling!==previous||next.form!==form||previous.form!==form||next.value!=='next'||previous.value!=='previous')throw 'named form control precedence';
if(getNext.call(form)!==document.getElementById('after')||getPrevious.call(form)!==document.getElementById('before'))throw 'saved getter uses actual form sibling tree';
if(form.nextElementSibling!==next||form.previousElementSibling!==previous||next.defaultValue!=='next'||previous.defaultValue!=='previous')throw 'native controls changed after sibling reads';
"#);
    assert_eq!(forms.controls.len(),2);
}
