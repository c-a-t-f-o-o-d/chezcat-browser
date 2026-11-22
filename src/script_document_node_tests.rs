//! Genuine logical main-Document Node facts; no full Document implementation.
use super::*;

fn fixture_with_profile(source:&str,browser:bool)->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><head></head><body><p id=p>old</p><form id=nodeType name=nodeType></form></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=if browser{Page::new()}else{Page::for_test()};
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/original").unwrap().0),path:None,root:None,fragment:String::new()});
    let out=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original main Document Node identity".into(),source:source.into()}]);
    assert_eq!(out.errors,0,"{}",page.report());(doc,forms,page)
}
fn fixture(source:&str)->(Document,Forms,Page){fixture_with_profile(source,false)}
fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let out=page.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{}",page.report());
}

#[test]
fn document_node_real_root_values_and_canonical_getters_match_existing_node_slots() {
    let (_,_,page)=fixture(r#"
var P=Node.prototype,T=Object.getOwnPropertyDescriptor(P,'nodeType'),N=Object.getOwnPropertyDescriptor(P,'nodeName'),typeGetter=T.get,nameGetter=N.get;
if(typeof typeGetter!=='function'||typeof nameGetter!=='function'||typeGetter.length!==0||nameGetter.length!==0||typeGetter.name!=='get nodeType'||nameGetter.name!=='get nodeName'||T.set!==undefined||N.set!==undefined||!T.enumerable||!T.configurable||!N.enumerable||!N.configurable)throw 'real Node accessor descriptors';
if(Object.getPrototypeOf(typeGetter)!==Function.prototype||Object.getOwnPropertyDescriptor(P,'nodeType').get!==typeGetter)throw 'stable owner function';
if(document.nodeType!==9||document.nodeName!=='#document'||!document.isConnected||!(document instanceof Node))throw 'real logical Document root';
if(document.ownerDocument!==null||document.parentNode!==null||document.parentElement!==null||document.previousSibling!==null||document.nextSibling!==null)throw 'root has no parent owner or siblings';
if(document.firstChild!==document.childNodes[0]||document.lastChild!==document.childNodes[document.childNodes.length-1]||document.firstChild!==document.documentElement)throw 'actual retained child identity';
if(!('nodeType' in document)||!('nodeName' in document)||!('isConnected' in document)||Object.hasOwn(document,'nodeType')||Object.getOwnPropertyDescriptor(document,'nodeName')!==undefined)throw 'inherited metadata presence';
var p=document.getElementById('p'),text=p.childNodes[0],attr=p.getAttributeNode('id'),calls=0,ignored={toString:function(){calls++;throw 'ignored';}};
if(typeGetter.call(document,ignored)!==9||nameGetter.call(document)!=='#document'||typeGetter.call(p)!==1||nameGetter.call(p)!=='P'||typeGetter.call(text)!==3||nameGetter.call(text)!=='#text'||typeGetter.call(attr)!==2||nameGetter.call(attr)!=='id'||calls)throw 'borrowed getters read genuine existing slots';
if(document.getElementById('nodeType')===null||document.nodeType!==9)throw 'named alias does not replace Node identity';
document.title='document-node-ok';
"#);
    assert_eq!(page.state.errors,0);assert!(!page.has_host_tasks());
}

#[test]
fn document_node_getter_brand_readonly_and_real_prototype_changes_remain_distinct() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var T=Object.getOwnPropertyDescriptor(Node.prototype,'nodeType'),N=Object.getOwnPropertyDescriptor(Node.prototype,'nodeName'),typeGetter=T.get,nameGetter=N.get,p=document.getElementById('p'),converted=0,value={valueOf:function(){converted++;return 17;},toString:function(){converted++;return 'changed';}};
document.nodeType=value;document.nodeName=value;document.isConnected=false;document.parentNode=value;
if(converted||document.nodeType!==9||document.nodeName!=='#document'||!document.isConnected||document.parentNode!==null)throw 'readonly writes do not coerce or mutate';
var caught=0;for(var operation of [function(){'use strict';document.nodeType=17;},function(){'use strict';document.nodeName='bad';},function(){'use strict';document.isConnected=false;}])try{operation();}catch(e){if(e instanceof TypeError)caught++;else throw e;}
for(var receiver of [undefined,null,true,1,'x',{},window,document.childNodes,p.style,Node.prototype]){try{typeGetter.call(receiver,value);}catch(e){if(e instanceof TypeError)caught++;else throw e;}try{nameGetter.call(receiver,value);}catch(e){if(e instanceof TypeError)caught++;else throw e;}}
if(caught!==23||converted)throw 'genuine Node brand and ignored arguments';
if(!delete document.nodeType||document.nodeType!==9||Object.hasOwn(document,'nodeType'))throw 'deleting absent own metadata retains inherited slot';
Object.defineProperty(Node.prototype,'nodeType',{value:77,writable:true,enumerable:true,configurable:true});
if(document.nodeType!==77||p.nodeType!==77||typeGetter.call(document)!==9||typeGetter.call(p)!==1)throw 'actual prototype lookup distinct from canonical slot getter';
Object.defineProperty(Node.prototype,'nodeType',T);
Object.defineProperty(p,'nodeType',{value:'own',configurable:true});if(p.nodeType!=='own'||typeGetter.call(p)!==1)throw 'ordinary own shadow cannot change real slot';delete p.nodeType;
if(document.nodeType!==9||p.nodeType!==1||nameGetter.call(document)!=='#document')throw 'prototype and own shadow restoration';
"#);
    let commits=page.state.dom_commits;
    let out=page.console_eval(&mut doc,&mut forms,"document.appendChild(document.createElement('div'));");
    assert_eq!(out.errors,1,"Document mutation remains explicitly unsupported");
    assert_eq!(page.state.dom_commits,commits);
    evaluate(&mut page,&mut doc,&mut forms,"if(document.nodeType!==9||document.nodeName!=='#document'||document.childNodes.length!==1)throw 'metadata failure recovery';");
}

#[test]
fn document_node_saved_getters_keep_actual_holder_and_receiver_owner_guards() {
    let (mut doc,mut forms,mut page)=fixture_with_profile(r#"
var typeGetter=Object.getOwnPropertyDescriptor(Node.prototype,'nodeType').get,nameGetter=Object.getOwnPropertyDescriptor(Node.prototype,'nodeName').get;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,ChildFunction=w.Function;
var childGetter=ChildFunction('return Object.getOwnPropertyDescriptor(Node.prototype,"nodeType").get;')(),marker={entered:0};
var invoke=ChildFunction('getter','node','marker','marker.entered++;return getter.call(node);');
if(Object.getPrototypeOf(childGetter)!==ChildFunction.prototype||childGetter===typeGetter||childGetter.call(document)!==9||invoke(nameGetter,document,marker)!=='#document'||marker.entered!==1)throw 'genuine child owner and same-origin borrowing';
"#,true);
    let commits=page.state.dom_commits;
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["childGetter.call(document);","invoke(typeGetter,document,marker);","invoke(nameGetter,document,marker);"] {
        let out=page.console_eval(&mut doc,&mut forms,source);assert_eq!(out.errors,1,"{source}: {}",page.report());
        assert_eq!(page.state.dom_commits,commits);
        evaluate(&mut page,&mut doc,&mut forms,"if(document.nodeType!==9||document.nodeName!=='#document')throw 'owner refusal changed Document';");
    }
    evaluate(&mut page,&mut doc,&mut forms,"if(marker.entered!==3)throw 'saved child reader must actually enter before guarded native call';");
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/original").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"if(childGetter.call(document)!==9||invoke(nameGetter,document,marker)!=='#document'||marker.entered!==4)throw 'same-origin recovery';");
}

#[test]
fn document_node_brand_precedes_host_work_and_string_refusal_recovers() {
    let (mut doc,mut forms,mut page)=fixture("var typeGetter=Object.getOwnPropertyDescriptor(Node.prototype,'nodeType').get,nameGetter=Object.getOwnPropertyDescriptor(Node.prototype,'nodeName').get;");
    let work=page.state.limits.host_work;page.state.limits.host_work=0;
    evaluate(&mut page,&mut doc,&mut forms,"var caught=false;try{typeGetter.call({});}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'brand must precede host work';");
    let out=page.console_eval(&mut doc,&mut forms,"typeGetter.call(document);");assert_eq!(out.errors,1);
    page.state.limits.host_work=work;
    let bytes=page.state.limits.host.string_bytes;page.state.limits.host.string_bytes=4;
    let out=page.console_eval(&mut doc,&mut forms,"nameGetter.call(document);");assert_eq!(out.errors,1);
    page.state.limits.host.string_bytes=bytes;
    evaluate(&mut page,&mut doc,&mut forms,"if(typeGetter.call(document)!==9||nameGetter.call(document)!=='#document')throw 'bounded refusal recovery';");
}
