//! Native NodeIterator factory and initial reference-state witnesses.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root>Main</div></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn node_iterator_state_canonical_factory_brands_descriptors_and_identity(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),made=document.createNodeIterator(root);
if(!(made instanceof NodeIterator)||Object.getPrototypeOf(made)!==NodeIterator.prototype||Object.prototype.toString.call(made)!=='[object NodeIterator]'||NodeIterator.length!==0||document.createNodeIterator.length!==1)throw 'canonical iterator';
if(made.root!==root||made.referenceNode!==root||!made.pointerBeforeReferenceNode||made.whatToShow!==4294967295||made.filter!==null)throw 'initial reference tuple';
var names=['root','referenceNode','pointerBeforeReferenceNode','whatToShow','filter'];for(var name of names){var desc=Object.getOwnPropertyDescriptor(NodeIterator.prototype,name);if(typeof desc.get!=='function'||desc.set!==undefined||!desc.enumerable||!desc.configurable)throw 'readonly native descriptor';}
var caught=0;try{NodeIterator();}catch(e){if(e instanceof TypeError)caught++;}try{new NodeIterator();}catch(e){if(e instanceof TypeError)caught++;}
try{Document.prototype.createNodeIterator.call({},root);}catch(e){if(e instanceof TypeError)caught++;}
try{Document.prototype.createNodeIterator.call(document,{});}catch(e){if(e instanceof TypeError)caught++;}
try{Object.getOwnPropertyDescriptor(NodeIterator.prototype,'root').get.call({});}catch(e){if(e instanceof TypeError)caught++;}
if(caught!==5)throw 'genuine iterator brands';var detach=made.detach;if(detach.call(made)!==undefined||made.root!==root||made.referenceNode!==root||!made.pointerBeforeReferenceNode)throw 'detach is a no-op';
var guarded=0;try{made.nextNode();}catch(e){if(e instanceof DOMException&&e.name==='NotSupportedError')guarded++;}try{made.previousNode();}catch(e){if(e instanceof DOMException&&e.name==='NotSupportedError')guarded++;}if(guarded!==2||made.referenceNode!==root||!made.pointerBeforeReferenceNode)throw 'unimplemented cursor must refuse explicitly';
"#);
}
#[test]
fn node_iterator_state_mask_conversion_filter_identity_and_original_abrupt(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),calls=0,filter={get acceptNode(){throw 'creation must not resolve callback';}},mask={valueOf:function(){calls++;root.firstChild.insertData(0,'X');return -1.9;}};
var iterator=document.createNodeIterator(root,mask,filter);if(calls!==1||iterator.whatToShow!==4294967295||iterator.filter!==filter||iterator.root!==root||root.textContent!=='XMain')throw 'ordered native conversion and identity';
function fn(){throw 'creation must not call function';}var callable=document.createNodeIterator(root,NaN,fn);if(callable.whatToShow!==0||callable.filter!==fn)throw 'function callback identity';
var wrapped=new Proxy(filter,{}),proxied=document.createNodeIterator(root,4294967300,wrapped);if(proxied.whatToShow!==4||proxied.filter!==wrapped)throw 'object proxy identity';
var abrupt={},caught=0;try{document.createNodeIterator(root,{valueOf:function(){throw abrupt;}},7);}catch(e){if(e===abrupt)caught++;}
try{document.createNodeIterator(root,1,7);}catch(e){if(e instanceof TypeError)caught++;}
try{document.createNodeIterator(root,Symbol('mask'),filter);}catch(e){if(e instanceof TypeError)caught++;}
var poisoned={valueOf:function(){throw 'brand must precede conversion';}};try{Document.prototype.createNodeIterator.call({},root,poisoned);}catch(e){if(e instanceof TypeError)caught++;}
if(caught!==4||iterator.referenceNode!==root||!iterator.pointerBeforeReferenceNode||iterator.detach()!==undefined)throw 'native conversion failure and saved reference';
"#);
}
#[test]
fn node_iterator_state_inert_template_raw_units_and_stable_native_roots(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var owner=new Document(),fragment=owner.createDocumentFragment(),text=owner.createTextNode('\uD800X\uDC00');fragment.appendChild(text);
var roots=[document,document.getElementById('root'),owner,fragment,text,owner.createComment('c'),owner.createCDATASection('x'),owner.createProcessingInstruction('p','x'),owner.createAttribute('title')];
for(var root of roots){var it=document.createNodeIterator(root);if(it.root!==root||it.referenceNode!==root||!it.pointerBeforeReferenceNode)throw 'genuine node root';}
var template=document.createElement('template');template.innerHTML='<span>Seed</span>';var content=template.content,held=document.createNodeIterator(content,4,null),copy=template.cloneNode(true);
content.firstChild.textContent='Changed';if(held.root!==content||held.referenceNode!==content||!held.pointerBeforeReferenceNode||held.root.ownerDocument===document||copy.content===content||copy.content.firstChild.textContent!=='Seed')throw 'appropriate inert owner and copy independence';
var exact=document.createNodeIterator(text);text.insertData(0,'Y');if(exact.root!==text||exact.referenceNode!==text||text.data.charCodeAt(1)!==55296||text.data.charCodeAt(3)!==56320||text.ownerDocument!==owner)throw 'raw units and stable native aliases';
"#);assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.has_navigation_request());
}
