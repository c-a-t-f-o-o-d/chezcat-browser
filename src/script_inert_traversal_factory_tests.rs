//! Genuine inert Document receivers use canonical native traversal factories.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=mount>Main</div></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn inert_traversal_factory_generic_xml_html_parsed_documents_canonical_identity(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var owners=[new Document(),document.implementation.createDocument(null,''),document.implementation.createHTMLDocument('owner'),new DOMParser().parseFromString('<!doctype html><body>Parsed</body>','text/html')];
for(var owner of owners){
 var root=owner.createDocumentFragment(),a=owner.createElementNS('http://www.w3.org/1999/xhtml','span'),b=owner.createElementNS('http://www.w3.org/1999/xhtml','b');a.textContent='A';b.textContent='B';root.appendChild(a);root.appendChild(b);
 var it=owner.createNodeIterator(root,4),walker=owner.createTreeWalker(root,4);
 if(!(it instanceof NodeIterator)||!(walker instanceof TreeWalker)||it.root!==root||walker.root!==root||it.referenceNode!==root||!it.pointerBeforeReferenceNode||walker.currentNode!==root||it.nextNode()!==a.firstChild||walker.nextNode()!==a.firstChild||it.nextNode()!==b.firstChild||walker.nextNode()!==b.firstChild||it.nextNode()!==null||walker.nextNode()!==null||root.ownerDocument!==owner)throw 'canonical inert receiver factories';
 if(owner.createNodeIterator!==document.createNodeIterator||owner.createTreeWalker!==document.createTreeWalker||owner.createNodeIterator(owner,256).nextNode()!==owner||owner.createTreeWalker(owner,256).root!==owner)throw 'canonical shared factories and document root';
}
"#);
}
#[test]
fn inert_traversal_factory_template_appropriate_owners_mixed_roots_and_raw_units(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var parsed=new DOMParser().parseFromString('<!doctype html><body><template id=t><span>Saved</span></template></body>','text/html'),host=parsed.getElementById('t'),content=host.content,owner=content.ownerDocument,text=content.firstChild.firstChild;
var it=owner.createNodeIterator(content,4),walker=parsed.createTreeWalker(content,4);if(it.nextNode()!==text||walker.nextNode()!==text||text.ownerDocument!==owner||it.root!==content||walker.root!==content)throw 'appropriate template owner and independent receiver';
var main=document.getElementById('mount'),mixed=parsed.createNodeIterator(main,4),other=owner.createTreeWalker(main,4);if(mixed.nextNode()!==main.firstChild||other.nextNode()!==main.firstChild||mixed.root!==main||other.root!==main)throw 'receiver need not own native root';
var exact=owner.createDocumentFragment(),raw=owner.createTextNode('\uD800X\uDC00');exact.appendChild(raw);var exactIt=owner.createNodeIterator(exact,4),exactWalker=owner.createTreeWalker(exact,4);if(exactIt.nextNode()!==raw||exactWalker.nextNode()!==raw||raw.data.charCodeAt(0)!==55296||raw.data.charCodeAt(2)!==56320)throw 'exact native units';
var copied=host.cloneNode(true),copyIt=copied.content.ownerDocument.createNodeIterator(copied.content,4);text.data='Changed';if(copyIt.nextNode().data!=='Saved'||it.referenceNode!==text||walker.currentNode!==text)throw 'independent copies and saved source aliases';
"#);assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.has_navigation_request());
}
#[test]
fn inert_traversal_factory_ordered_mask_effects_abrupt_identity_and_genuine_receiver_guards(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var owner=new Document(),root=owner.createDocumentFragment(),text=owner.createTextNode('A');root.appendChild(text);var gets=0,calls=0,filter={get acceptNode(){gets++;return function(n){calls++;return 1;};}},mask={valueOf:function(){root.appendChild(owner.createTextNode('B'));return 4;}};
var it=owner.createNodeIterator(root,mask,filter),walker=owner.createTreeWalker(root,4,filter);if(gets||calls||it.filter!==filter||walker.filter!==filter||it.nextNode()!==text||walker.nextNode()!==text||gets!==2||calls!==2||root.lastChild.data!=='B')throw 'ordered conversion and filter lookup';
var abrupt={},caught=0;for(var factory of [Document.prototype.createNodeIterator,Document.prototype.createTreeWalker]){
 try{factory.call(owner,root,{valueOf:function(){throw abrupt;}},7);}catch(e){if(e===abrupt)caught++;}
 try{factory.call({nodeType:9},root,{valueOf:function(){throw 'brand before conversion';}});}catch(e){if(e instanceof TypeError)caught++;}
 try{factory.call(owner,{},4);}catch(e){if(e instanceof TypeError)caught++;}
}
if(caught!==6||it.nextNode()!==root.lastChild||walker.nextNode()!==root.lastChild)throw 'typed guards and recovery';
"#);
}
