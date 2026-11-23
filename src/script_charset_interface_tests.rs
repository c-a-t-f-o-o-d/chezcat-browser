//! Canonical charset accessors preserve native brands, conversion and caller authority.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<!doctype html><head><script id=s type=application/x-inert charset=seed></script><link id=l charset=link-seed></head><body><div id=other></div></body>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(&crate::navigation::address("https://charset.test/",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}

fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    assert_eq!(page.console_eval(doc,forms,source).errors,0,"{}",page.report());
}

#[test]
fn charset_interface_descriptors_exact_values_and_author_shadows() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var s=document.getElementById('s'),l=document.getElementById('l');
for(var entry of [[s,HTMLScriptElement.prototype,'seed'],[l,HTMLLinkElement.prototype,'link-seed']]){
var node=entry[0],proto=entry[1],saved=Object.getOwnPropertyDescriptor(proto,'charset');
if(!saved||!saved.enumerable||!saved.configurable||saved.get.length!==0||saved.set.length!==1||saved.get.name!=='get charset'||saved.set.name!=='set charset'||Object.getPrototypeOf(node)!==proto||saved.get.call(node)!==entry[2])throw 'canonical charset descriptor';
var attr=node.getAttributeNode('charset'),observer=new MutationObserver(function(){});observer.observe(node,{attributes:true,attributeOldValue:true,attributeFilter:['charset']});
var calls=0;saved.set.call(node,{toString:function(){calls++;node.setAttribute('charset','during');return '\ud800\ufffd\udfff';}});
var records=observer.takeRecords();if(calls!==1||node.charset!=='\ud800\ufffd\udfff'||attr.value!==node.charset||records.length!==2||records[0].oldValue!==entry[2]||records[1].oldValue!=='during')throw 'exact accessor storage and records';
Object.defineProperty(node,'charset',{value:'shadow',writable:true,configurable:true});node.charset='author';saved.set.call(node,'native');
if(node.charset!=='author'||saved.get.call(node)!=='native'||attr.value!=='native')throw 'own descriptor precedence';delete node.charset;
Object.defineProperty(proto,'charset',{value:'prototype shadow',configurable:true});if(node.charset!=='prototype shadow'||saved.get.call(node)!=='native')throw 'prototype descriptor precedence';Object.defineProperty(proto,'charset',saved);
if(node.charset!=='native')throw 'restored accessor';
observer.disconnect();
}
"#);
}

#[test]
fn charset_interface_receiver_brand_precedes_domstring_conversion() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var s=document.getElementById('s'),l=document.getElementById('l'),other=document.getElementById('other');
for(var entry of [[HTMLScriptElement.prototype,s,l],[HTMLLinkElement.prototype,l,s]]){
var saved=Object.getOwnPropertyDescriptor(entry[0],'charset');if(!saved)throw 'missing charset descriptor';var calls=0,value={toString:function(){calls++;return 'converted';}};
for(var wrong of [null,{},other,entry[2]]){var bad=false;try{saved.set.call(wrong,value);}catch(e){bad=e instanceof TypeError;}if(!bad||calls)throw 'setter brand before conversion';bad=false;try{saved.get.call(wrong);}catch(e){bad=e instanceof TypeError;}if(!bad)throw 'getter brand';}
var node=entry[1];saved.set.call(node,null);if(node.charset!=='null')throw 'null DOMString';saved.set.call(node);if(node.charset!=='undefined')throw 'omitted DOMString';
var bad=false;try{saved.set.call(node,Symbol('charset'));}catch(e){bad=e instanceof TypeError;}if(!bad||node.charset!=='undefined')throw 'symbol conversion';
saved.get.call(node,value);if(calls)throw 'getter ignores extra argument';
}
"#);
}

#[test]
fn charset_interface_saved_child_caller_origin_precedes_value_conversion() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);
var child=f.contentWindow,s=document.getElementById('s'),l=document.getElementById('l'),calls=0;
var putScript=child.Function('node','value',"Object.getOwnPropertyDescriptor(HTMLScriptElement.prototype,'charset').set.call(node,value);"),putLink=child.Function('node','value',"Object.getOwnPropertyDescriptor(HTMLLinkElement.prototype,'charset').set.call(node,value);");
var value={toString:function(){calls++;return '\ud800';}};putScript(s,value);putLink(l,value);if(calls!==2||s.charset!=='\ud800'||l.charset!=='\ud800')throw 'same origin exact accessors';
"#);
    page.set_location(&crate::navigation::address("https://other-charset.test/",&crate::navigation::Location::home()).unwrap());
    for operation in ["putScript(s,value);","putLink(l,value);"] {
        assert_eq!(page.console_eval(&mut doc,&mut forms,operation).errors,1);
        assert!(page.report().contains("same-origin"),"{}",page.report());
    }
    run(&mut doc,&mut forms,&mut page,"if(calls!==2||s.charset!=='\\ud800'||l.charset!=='\\ud800')throw 'refused conversion changed metadata';");
}


#[test]
fn charset_interface_genuine_inert_owner_and_foreign_brand_contracts() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var xml=document.implementation.createDocument(null,'Root'),calls=0,value={toString:function(){calls++;return 'changed';}};
for(var entry of [['script',HTMLScriptElement],['link',HTMLLinkElement]]){
var C=entry[1],saved=Object.getOwnPropertyDescriptor(C.prototype,'charset'),inert=xml.createElementNS('http://www.w3.org/1999/xhtml',entry[0]),foreign=xml.createElementNS('urn:outside',entry[0]);
if(!(inert instanceof C)||foreign instanceof C||Object.getPrototypeOf(C.prototype)!==HTMLElement.prototype)throw 'real namespace interface';
var bad=false;try{new C();}catch(e){bad=e instanceof TypeError;}if(!bad)throw 'illegal interface constructor';
var before=calls;saved.set.call(inert,value);if(calls!==before+1||inert.getAttribute('charset')!=='changed'||saved.get.call(inert)!=='changed')throw 'genuine inert owner storage';
bad=false;try{saved.set.call(foreign,value);}catch(e){bad=e instanceof TypeError;}if(!bad||calls!==before+1)throw 'foreign brand before conversion';
}
"#);
}

#[test]
fn charset_interface_native_wrapper_storage_and_output_refusal_recover() {
    let (mut doc,mut forms,mut page)=fixture();
    let script=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("s")).unwrap();
    let old=doc.dom_attr(script,"charset").unwrap().capacity();
    let raw=Value::StringUnits(vec![0xd800,0xfffd,0xdc00]);
    let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let object=node_handle(script);let before=(b.state.retained,b.state.dom_commits);let limit=b.state.retained_limit;
    b.state.retained_limit=b.state.retained;
    assert!(b.charset_reflection_set(object,&raw).unwrap_err().contains("backing allowance"));
    assert_eq!((b.state.retained,b.state.dom_commits),before);assert_eq!(b.doc.attr(script,"charset"),Some("seed"));
    b.state.retained_limit=limit;b.state.copy_work=0;
    assert!(b.charset_reflection_set(object,&raw).unwrap_err().contains("string work limit"));
    assert_eq!((b.state.retained,b.state.dom_commits),before);
    b.state.copy_work=b.state.limits.copy_bytes;b.charset_reflection_set(object,&raw).unwrap();
    let current=b.doc.dom_attr(script,"charset").unwrap().capacity();
    assert_eq!(b.state.retained,before.0-old+current);
    let bytes=b.state.limits.host.string_bytes;b.state.limits.host.string_bytes=5;
    assert!(b.charset_reflection_get(object).unwrap_err().contains("allowance"));
    b.state.limits.host.string_bytes=bytes;b.state.copy_work=0;
    assert!(b.charset_reflection_get(object).unwrap_err().contains("string work limit"));
    b.state.copy_work=b.state.limits.copy_bytes;
    assert!(matches!(b.charset_reflection_get(object).unwrap(),Value::StringUnits(units) if units==[0xd800,0xfffd,0xdc00]));
    assert_eq!(b.state.retained,before.0-old+current);assert!(b.state.dynamic.pending.is_empty());assert!(b.state.stylesheets.pending.is_empty());
}
