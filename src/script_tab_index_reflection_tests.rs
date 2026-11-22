//! Global HTML tabindex data, signed-long IDL and genuine native focus order.
use super::*;
fn fixture()->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><body><original-tooltip id=host></original-tooltip><input id=field value=seed><button id=button>Run</button><svg><g id=foreign></g></svg></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);
    let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(&crate::navigation::address("https://tab-index.test/source",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str) {
    assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());
}
#[test]
fn tab_index_reflection_native_descriptor_signed_long_and_attribute_transactions() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var host=document.getElementById('host'),field=document.getElementById('field'),d=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'tabIndex'),obs=new MutationObserver(function(){});if(!d||!d.enumerable||!d.configurable||d.get.length||d.set.length!==1||Object.hasOwn(host,'tabIndex')||host.tabIndex!==-1||field.tabIndex!==0)throw 'real inherited descriptor';
host.setAttribute('TABINDEX',' +3tail');var attr=host.getAttributeNode('tabindex');if(host.tabIndex!==3||attr.value!==' +3tail')throw 'HTML integer prefix';obs.observe(host,{attributes:true,attributeOldValue:true});var calls=0;d.set.call(host,{valueOf:function(){calls++;return 4294967297;},toString:function(){throw 'wrong numeric hint';}});var records=obs.takeRecords();if(calls!==1||host.tabIndex!==1||attr.value!=='1'||records.length!==1||records[0].target!==host||records[0].attributeName!=='tabindex'||records[0].attributeNamespace!==null||records[0].oldValue!==' +3tail')throw 'actual reflected transaction';
for(var pair of [[-4294967297,-1],[2147483648,-2147483648],[NaN,0],[Infinity,0],[-1.9,-1],[null,0],[undefined,0],[' +2tail',0]]){d.set.call(host,pair[0]);if(host.tabIndex!==pair[1]||attr.value!==String(pair[1]))throw 'signed long conversion';obs.takeRecords();}
for(var bad of [Symbol('x'),BigInt(1)]){var caught=false;try{d.set.call(host,bad);}catch(e){caught=e instanceof TypeError;}if(!caught||attr.value!=='0'||obs.takeRecords().length)throw 'failed numeric conversion';}
attr.value='-7tail';if(host.tabIndex!==-7||host.getAttributeNode('tabindex')!==attr)throw 'saved Attr mutation';host.removeAttribute('tabindex');if(host.tabIndex!==-1||attr.ownerElement!==null||attr.value!=='-7tail')throw 'native removal';host.setAttributeNS('urn:original','tabindex','9');if(host.tabIndex!==-1||host.getAttributeNS('urn:original','tabindex')!=='9')throw 'namespace does not grant focus data';
var reads=0;Object.defineProperty(host,'tabIndex',{configurable:true,get:function(){reads++;return 37;},set:function(v){d.set.call(this,v);}});host.tabIndex=-1;if(host.tabIndex!==37||d.get.call(host)!==-1||host.getAttributeNS(null,'tabindex')!=='-1'||host.getAttribute('tabindex')!=='9'||reads!==1)throw 'author override versus native getter';delete host.tabIndex;if(host.tabIndex!==-1)throw 'native reflection restore';field.tabIndex=2;if(field.tabIndex!==2||field.getAttribute('tabindex')!=='2')throw 'native control property';
"#);
    let field=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("field")).unwrap();
    assert_eq!(forms.controls[&field].tab_index,2);
}
#[test]
fn tab_index_reflection_inert_clone_import_promotion_and_summary_defaults() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var t=document.createElement('template');t.innerHTML='<original-tooltip><span>native</span></original-tooltip>';var root=t.content.firstChild,owner=root.ownerDocument,d=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'tabIndex');root.tabIndex=-1;var attr=root.getAttributeNode('tabindex'),copy=root.cloneNode(true),imported=document.importNode(root,true);if(root.tabIndex!==-1||attr.value!=='-1'||copy.tabIndex!==-1||copy.getAttributeNode('tabindex')===attr||copy.ownerDocument!==owner||imported.ownerDocument!==document||imported.tabIndex!==-1)throw 'real copy attributes';document.body.appendChild(t.content);if(root.ownerDocument!==document||root.getAttributeNode('tabindex')!==attr||d.get.call(root)!==-1)throw 'saved promoted attribute';d.set.call(root,3);if(root.tabIndex!==3||attr.value!=='3'||copy.tabIndex!==-1)throw 'independent promoted data';
var details=document.createElement('template');details.innerHTML='<details><summary id="first">First</summary><summary id="second">Second</summary></details>';var parent=details.content.firstChild,first=parent.firstChild,second=first.nextSibling;if(first.tabIndex!==0||second.tabIndex!==-1)throw 'inert summary defaults';parent.removeChild(first);if(first.tabIndex!==-1||second.tabIndex!==0)throw 'actual inert physical order';var made=owner.createElement('input');if(made.tabIndex!==0)throw 'inert control default';
"#);
}
#[test]
fn tab_index_reflection_brands_child_write_and_storage_refusal_keep_native_state() {
    let (mut doc,mut forms,mut page)=fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var host=document.getElementById('host'),field=document.getElementById('field'),d=Object.getOwnPropertyDescriptor(HTMLElement.prototype,'tabIndex'),calls=0,bad={valueOf:function(){calls++;return 3;}},frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentDocument.body;
for(var target of [document,document.createTextNode('x'),document.getElementById('foreign'),Object.create(HTMLElement.prototype),child]){var caught=false;try{d.set.call(target,bad);}catch(e){caught=e.name==='TypeError'||e.name==='NotSupportedError';}if(!caught||calls)throw 'brand and readonly child before conversion';}
if(d.get.call(child)!==-1)throw 'readonly actual child';field.tabIndex=1;var obs=new MutationObserver(function(){});obs.observe(field,{attributes:true,attributeOldValue:true});var attr=field.getAttributeNode('tabindex');
"#);
    let field=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("field")).unwrap();
    let retained=page.state.retained_limit;page.state.retained_limit=page.state.retained;
    let before=(doc.nodes.len(),page.state.dom_commits,forms.controls[&field].tab_index);
    assert_eq!(page.console_eval(&mut doc,&mut forms,"field.tabIndex=4294967295;").errors,1);
    assert_eq!((doc.nodes.len(),page.state.dom_commits,forms.controls[&field].tab_index),before);
    page.state.retained_limit=retained;
    run(&mut doc,&mut forms,&mut page,"if(field.tabIndex!==1||attr.value!=='1'||obs.takeRecords().length)throw 'failed tabindex publication';field.setAttribute('tabindex',' -1tail');if(field.tabIndex!==-1||attr.value!==' -1tail')throw 'fresh job recovery';");
    assert_eq!(forms.controls[&field].tab_index,-1);
}
