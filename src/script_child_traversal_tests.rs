//! Native child traversal factories, roots and method realms.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=mount></div><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://iterator.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}

#[test]
fn child_traversal_native_roots_masks_flat_filters_and_root_boundaries(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,cd=w.document,html=cd.documentElement,head=cd.head,body=cd.body;
var it=document.createNodeIterator(cd,1),walker=document.createTreeWalker(cd,1);if(it.nextNode()!==html||it.nextNode()!==head||it.nextNode()!==body||it.nextNode()!==null||it.previousNode()!==body||it.previousNode()!==head||it.previousNode()!==html||it.previousNode()!==null)throw 'native child preorder and reverse';
if(walker.firstChild()!==html||walker.firstChild()!==head||walker.nextSibling()!==body||walker.previousSibling()!==head||walker.parentNode()!==html||walker.parentNode()!==null)throw 'child walker root mask';
var skip=document.createNodeIterator(cd,1,function(n){return n===html?2:1;}),reject=document.createTreeWalker(cd,1,function(n){return n===html?2:1;});if(skip.nextNode()!==head||reject.nextNode()!==null)throw 'flat iterator versus pruned walker';
if(document.createNodeIterator(body,4294967295).nextNode()!==body||document.createTreeWalker(body,4294967295).nextNode()!==null||document.createNodeIterator(cd,256).nextNode()!==cd)throw 'root inclusion and document brand';
"#);}
#[test]
fn child_traversal_canonical_method_realm_prototypes_and_mixed_owner_roots(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,cd=w.document;
if(w.TreeWalker===TreeWalker||w.NodeIterator===NodeIterator||w.NodeFilter===NodeFilter||w.NodeFilter.SHOW_ELEMENT!==1||w.NodeFilter.FILTER_REJECT!==2||cd.createTreeWalker!==w.Document.prototype.createTreeWalker||cd.createNodeIterator!==w.Document.prototype.createNodeIterator)throw 'canonical child factories';
var childIt=cd.createNodeIterator(cd,1),childWalker=cd.createTreeWalker(cd,1);if(!(childIt instanceof w.NodeIterator)||childIt instanceof NodeIterator||!(childWalker instanceof w.TreeWalker)||childWalker instanceof TreeWalker||childIt.nextNode()!==cd.documentElement||childWalker.nextNode()!==cd.documentElement)throw 'child method allocation realm';
var borrowed=w.Document.prototype.createNodeIterator.call(document,document.body,1),parent=Document.prototype.createTreeWalker.call(cd,cd,1);if(!(borrowed instanceof w.NodeIterator)||!(parent instanceof TreeWalker)||borrowed.root!==document.body||parent.nextNode()!==cd.documentElement)throw 'method realm differs from receiver and root';
var inert=new Document(),root=inert.createDocumentFragment(),text=inert.createTextNode('saved');root.appendChild(text);var mixed=cd.createNodeIterator(root,4);if(!(mixed instanceof w.NodeIterator)||mixed.nextNode()!==text)throw 'child factory genuine inert root';
var missing=w.eval('({})'),bad=cd.createNodeIterator(cd,1,missing),caught=false;try{bad.nextNode();}catch(e){caught=e instanceof w.TypeError;}if(!caught||bad.referenceNode!==cd||!bad.pointerBeforeReferenceNode)throw 'child callback error realm';
"#);}
#[test]
fn child_traversal_original_abrupt_brand_guards_and_native_origin_refusal_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var cd=frame.contentDocument,abrupt={},caught=0;
for(var factory of [Document.prototype.createNodeIterator,Document.prototype.createTreeWalker]){try{factory.call(cd,cd,{valueOf:function(){throw abrupt;}});}catch(e){if(e===abrupt)caught++;}try{factory.call({nodeType:9},cd,{valueOf:function(){throw 'bad receiver';}});}catch(e){if(e instanceof TypeError)caught++;}}
if(caught!==4)throw 'brand order abrupt identity';var it=document.createNodeIterator(cd,1);if(it.nextNode()!==cd.documentElement)throw 'recovery cursor';
"#);
 let location=crate::navigation::address("https://iterator.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&crate::navigation::address("https://foreign.test/source",&crate::navigation::Location::home()).unwrap());
 p.begin_job();let denied=p.runtime.execute("it.nextNode();",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state});assert!(denied.is_err());
 p.set_location(&location);run(&mut d,&mut f,&mut p,"if(it.referenceNode!==cd.documentElement||it.nextNode()!==cd.head)throw 'origin refusal must preserve cursor';");
}
