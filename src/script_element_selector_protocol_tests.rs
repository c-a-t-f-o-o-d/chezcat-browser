//! Original Element operation protocol, coercion and real-tree controls.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse(concat!(
        "<!doctype html><title>Element selector protocol</title><body>",
        "<form id=f><input id=neighbor name=keep value=original>",
        "<input id=named-one name=querySelector value=one><input id=named-all name=querySelectorAll value=all></form>",
        "<div id=root><span id=a class=item>A</span>middle<div id=nested><span id=b class=item>B</span></div></div>",
        "<div id=destination><span id=c class=item>C</span></div>"
    ));
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    let location=crate::navigation::address("https://element-selector.test/protocol",&crate::navigation::Location::home()).unwrap();
    page.set_location(&location);page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0,"{}",page.report());
    (doc,forms,page)
}

fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    let out=page.console_eval(doc,forms,source);
    assert_eq!(out.errors,0,"{source}\n{}",page.report());
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());
    let(a,b,c)=page.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());
}

#[test]
fn element_selector_prototype_nonconstructible_real_element_brand_precedes_string_conversion() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a');
var one=Element.prototype.querySelector,all=Element.prototype.querySelectorAll,touched=0,rejected=0;
var request={toString:function(){touched++;return '.item';}};
var fake={get nodeType(){touched++;return 1;},get ownerDocument(){touched++;return document;}};
var receivers=[undefined,null,true,3,'text',Symbol('receiver'),fake,Object.create(Element.prototype),document,window,document.createDocumentFragment(),document.createTextNode('text'),document.createComment('comment'),document.createAttribute('attr')];
for(var fn of [one,all]){
 for(var receiver of receivers){try{fn.call(receiver,request);throw 'accepted wrong Element brand';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}}
 try{new fn(request);throw 'Element selector operation was a constructor';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
}
if(touched||rejected!==receivers.length*2+2)throw 'brand/constructor validation converted a selector or inspected a duck';
if(root.querySelector!==one||root.querySelectorAll!==all||Object.hasOwn(root,'querySelector')||Object.hasOwn(root,'querySelectorAll'))throw 'Element operations are not canonical inherited functions';
if(one.call(root,'.item')!==a||all.call(root,'.item').length!==2||Object.hasOwn(Node.prototype,'querySelector')||Object.hasOwn(Node.prototype,'querySelectorAll'))throw 'declaring interface or valid receiver result';
"#);
}

#[test]
fn element_selector_prototype_required_argument_domstring_effects_and_selector_error_order() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b');
var one=Element.prototype.querySelector,all=Element.prototype.querySelectorAll,trace='',rejected=0;
for(var fn of [one,all]){
 try{fn.call(root);throw 'missing required selector accepted';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
 try{fn.call(root,Symbol('selector'));throw 'Symbol selector accepted';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
}
var exotic={};exotic[Symbol.toPrimitive]=function(hint){trace+=hint;return '#a';};
if(one.call(root,exotic)!==a||trace!=='string')throw 'DOMString exotic conversion hint/order';
trace='';var ordinary={toString:function(){trace+='t';return {};},valueOf:function(){trace+='v';return '#b';}};
var list=all.call(root,ordinary);if(trace!=='tv'||list.length!==1||list[0]!==b)throw 'DOMString ordinary conversion order';
var symbolObject={};symbolObject[Symbol.toPrimitive]=function(hint){trace+='s';return Symbol('converted');};
try{one.call(root,symbolObject);throw 'object Symbol conversion accepted';}catch(error){if(error.name!=='TypeError')throw error;rejected++;}
var sentinel={original:true},thrown={toString:function(){trace+='e';throw sentinel;}};
try{all.call(root,thrown);throw 'selector conversion exception swallowed';}catch(error){if(error!==sentinel)throw error;rejected++;}
var invalid={toString:function(){trace+='i';return '[';}};
try{one.call(root,invalid);throw 'invalid selector accepted';}catch(error){if(error.name!=='SyntaxError')throw error;rejected++;}
var ignored={toString:function(){throw 'extra argument was converted';}};
if(one.call(root,'#a',ignored)!==a||one.call(root,undefined)!==null||one.call(root,null)!==null||all.call(root,false).length!==0)throw 'explicit primitive conversion or extra argument';
if(trace!=='tvsei'||rejected!==7||one.call(root,'#a')!==a)throw 'conversion/error ordering or recovery';
"#);
}

#[test]
fn element_selector_prototype_reentrant_conversion_queries_current_tree_and_keeps_static_snapshots_editor() {
    let(mut doc,mut forms,mut page)=fixture();
    let neighbor=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("neighbor")).unwrap();
    forms.script_set_value(neighbor,"edited neighbor").unwrap();
    {let e=&mut forms.controls.get_mut(&neighbor).unwrap().value;e.cursor=5;e.anchor=2;e.offset=1;}
    let old=&forms.controls[&neighbor];
    let kept=(old.value.text(),old.value.cursor,old.value.anchor,old.value.offset,old.dirty_value,old.owner);
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),destination=document.getElementById('destination');
var one=Element.prototype.querySelector,all=Element.prototype.querySelectorAll,old=all.call(root,'.item'),conversions=0;
var request={toString:function(){conversions++;if(one.call(destination,'.item')!==c)throw 'nested selector conversion';root.appendChild(c);return '.item';}};
var fresh=all.call(root,request);
if(conversions!==1||old.length!==2||old[0]!==a||old[1]!==b||fresh.length!==3||fresh[0]!==a||fresh[1]!==b||fresh[2]!==c||c.parentNode!==root)throw 'query did not use post-conversion topology or old snapshot changed';
var remove={toString:function(){conversions++;root.removeChild(a);return '.item';}};
if(one.call(root,remove)!==b||a.parentNode!==null||a.isConnected||fresh[0]!==a||old[0]!==a||old.length!==2||fresh.length!==3||all.call(root,'.item').length!==2||conversions!==2)throw 'removed identity or static snapshot after conversion';
if(document.getElementById('neighbor').value!=='edited neighbor'||document.getElementById('neighbor').defaultValue!=='original')throw 'selector conversion reset native input';
"#);
    let now=&forms.controls[&neighbor];
    assert_eq!((now.value.text(),now.value.cursor,now.value.anchor,now.value.offset,now.dirty_value,now.owner),kept);
    assert_eq!(forms.text_default_value(neighbor).unwrap(),"original");
    assert_eq!(doc.attr(neighbor,"value"),Some("original"));
}

#[test]
fn element_selector_prototype_own_shadow_live_override_delete_and_saved_native_function() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),state={original:true};
for(var name of ['querySelector','querySelectorAll']){
 var descriptor=Object.getOwnPropertyDescriptor(Element.prototype,name),saved=descriptor.value,calls=0;
 root[name]=function(selector){calls++;if(this!==root||selector!=='original')throw 'own operation receiver/argument';return state;};
 if(!Object.hasOwn(root,name)||root[name]('original')!==state||calls!==1)throw 'own Element selector shadow';
 var result=saved.call(root,'.item');if(name==='querySelector'?result!==a:result.length!==2||result[0]!==a)throw 'saved native function followed own shadow';
 if(!delete root[name]||root[name]!==saved||Object.hasOwn(root,name))throw 'own selector deletion did not restore prototype';
 Object.defineProperty(Element.prototype,name,{get:function(){calls++;if(this!==root)throw 'live prototype getter receiver';return function(selector){if(this!==root||selector!=='replacement')throw 'replacement method receiver';return state;};},configurable:true});
 if(root[name]('replacement')!==state||calls!==2)throw 'live selector prototype override';
 result=saved.call(root,'.item');if(name==='querySelector'?result!==a:result.length!==2)throw 'saved native function followed prototype override';
 if(!delete Element.prototype[name]||root[name]!==undefined||name in root)throw 'deleted selector has ghost native fallback';
 Object.defineProperty(Element.prototype,name,descriptor);
 if(root[name]!==saved)throw 'canonical selector descriptor restoration';
}
if(root.querySelector('.item')!==a||root.querySelectorAll('.item').length!==2)throw 'restored canonical selectors';
"#);
}

#[test]
fn element_selector_prototype_form_named_controls_keep_precedence_over_canonical_borrowing() {
    let(mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var form=document.getElementById('f'),neighbor=document.getElementById('neighbor'),namedOne=document.getElementById('named-one'),namedAll=document.getElementById('named-all');
var one=Element.prototype.querySelector,all=Element.prototype.querySelectorAll;
if(form.querySelector!==namedOne||form.querySelectorAll!==namedAll)throw 'native form named-control precedence';
if(one.call(form,'input')!==neighbor||all.call(form,'input').length!==3)throw 'canonical borrowed selectors cannot query genuine form';
for(var name of ['querySelector','querySelectorAll']){
 var descriptor=Object.getOwnPropertyDescriptor(Element.prototype,name),expected=name==='querySelector'?namedOne:namedAll;
 delete Element.prototype[name];
 if(form[name]!==expected||!(name in form))throw 'prototype deletion hid actual named form control';
 Object.defineProperty(Element.prototype,name,descriptor);
 if(form[name]!==expected)throw 'prototype restoration overrode named form control';
}
if(form.querySelector!==namedOne||form.querySelectorAll!==namedAll||neighbor.value!=='original'||neighbor.defaultValue!=='original'||namedOne.form!==form||namedAll.form!==form)throw 'form query altered control state or owner';
"#);
}
