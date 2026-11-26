//! Original Range content-copy witnesses; source trees remain native.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root><b id=a>A😀B</b><span id=middle data-note=seed>M<i>I</i></span><em id=z>XY</em></div><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();
 d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();
 p.set_location(&crate::navigation::address("https://range-copy.test/source",&crate::navigation::Location::home()).unwrap());
 assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);
 run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),a=document.getElementById('a'),middle=document.getElementById('middle'),z=document.getElementById('z');");
 (d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn range_clone_contents_unsupported_producers_preserve_recovery(){
 let(mut d,mut f,mut p)=fixture();p.begin_job();
 let error=p.runtime.execute("var blockedSvg=document.createElementNS('http://www.w3.org/2000/svg','g');blockedSvg.setAttributeNS('urn:copy','p:note','exact');",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();
 assert!(!error.is_empty());
 run(&mut d,&mut f,&mut p,"var r=document.createRange();r.selectNode(document.getElementById('neighbor'));");
 p.begin_job();let before=(p.state.retained,p.state.inert_documents.backing_bytes(),d.nodes.len(),p.state.dom_commits);
 let error=p.runtime.execute("r.cloneContents();",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();
 assert!(error.contains("native control state"),"{error}");
 assert_eq!((p.state.retained,p.state.inert_documents.backing_bytes(),d.nodes.len(),p.state.dom_commits),before);
 run(&mut d,&mut f,&mut p,"r.selectNode(middle);if(r.cloneContents().firstChild.textContent!=='MI'||middle.textContent!=='MI')throw 'guard recovery';");
}
#[test]
fn range_clone_contents_collapsed_and_split_surrogate_character_data(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),text=a.firstChild;r.setStart(text,2);r.setEnd(text,3);
var descriptor=Object.getOwnPropertyDescriptor(Range.prototype,'cloneContents'),canonical=descriptor.value;
if(typeof canonical!=='function'||canonical.name!=='cloneContents'||canonical.length!==0||descriptor.get!==undefined||!descriptor.writable||!descriptor.configurable||!descriptor.enumerable||canonical!==r.cloneContents)throw 'canonical deferred data method';
var fragment=r.cloneContents(),clone=fragment.firstChild;
if(fragment.nodeType!==11||fragment.ownerDocument!==document||fragment.parentNode!==null||fragment.childNodes.length!==1||clone===text||clone.parentNode!==fragment||clone.ownerDocument!==document||clone.nodeType!==3||clone.nodeValue.length!==1||clone.nodeValue.charCodeAt(0)!==56832)throw 'exact independent partial CharacterData';
if(r.startContainer!==text||r.startOffset!==2||r.endOffset!==3||text.nodeValue!=='A😀B')throw 'source and range changed';
clone.nodeValue='Changed';if(text.nodeValue!=='A😀B')throw 'data aliases';
r.collapse(true);var empty=r.cloneContents();if(empty===fragment||empty.firstChild!==null||empty.childNodes.length||empty.ownerDocument!==document)throw 'collapsed fragment';
var comment=document.createComment('Q😀Z');root.appendChild(comment);r.setStart(comment,1);r.setEnd(comment,3);
var cc=r.cloneContents().firstChild;if(cc.nodeType!==8||cc.nodeValue!=='😀'||comment.nodeValue!=='Q😀Z')throw 'partial comment kind';
delete Range.prototype.cloneContents;if(r.cloneContents!==undefined||canonical.call(r).firstChild.nodeValue!=='😀')throw 'captured method after deletion';
"#);
}
#[test]
fn range_clone_contents_partial_ancestors_whole_nodes_and_independent_attributes(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),text=a.firstChild,last=z.firstChild,calls=0;middle.secret=99;middle.addEventListener('copy-check',function(){calls++;});
r.setStart(text,1);r.setEnd(last,1);var frag=r.cloneContents(),x=frag.childNodes;
if(x.length!==3||x[0].localName!=='b'||x[0].textContent!=='😀B'||x[1].localName!=='span'||x[1].textContent!=='MI'||x[2].localName!=='em'||x[2].textContent!=='X')throw 'partial ancestor structure and order';
if(x[1]===middle||x[1].secret!==undefined||x[1].firstChild===middle.firstChild||x[1].getAttribute('data-note')!=='seed'||x[1].getAttributeNode('data-note').ownerDocument!==document)throw 'native identity attributes and author properties';
var event=document.createEvent('Event');event.initEvent('copy-check',false,false);x[1].dispatchEvent(event);if(calls)throw 'listener copied';
x[1].setAttribute('data-note','copy');if(middle.getAttribute('data-note')!=='seed'||root.childNodes.length!==3||r.startContainer!==text||r.endContainer!==last||r.startOffset!==1||r.endOffset!==1)throw 'source aliases or boundaries changed';
r.selectNode(middle);var whole=r.cloneContents();if(whole.childNodes.length!==1||whole.firstChild.textContent!=='MI'||whole.firstChild===middle)throw 'whole subtree';
"#);
}
#[test]
fn range_clone_contents_svg_and_template_native_owners_and_inactive_payload(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var svg=document.createElementNS('http://www.w3.org/2000/svg','svg'),g=document.createElementNS('http://www.w3.org/2000/svg','g');g.setAttribute('data-note','exact');svg.appendChild(g);root.appendChild(svg);
var r=document.createRange();r.selectNode(svg);var copy=r.cloneContents().firstChild;
if(copy===svg||copy.namespaceURI!=='http://www.w3.org/2000/svg'||copy.firstChild.namespaceURI!=='http://www.w3.org/2000/svg'||copy.firstChild.getAttribute('data-note')!=='exact'||copy.ownerDocument!==document)throw 'namespace and independent owner';
middle.setAttributeNS('urn:copy','p:note','exact');r.selectNode(middle);var named=r.cloneContents().firstChild;if(named.getAttributeNS('urn:copy','note')!=='exact'||named.getAttributeNodeNS('urn:copy','note').ownerDocument!==document)throw 'arbitrary attribute namespace copy';
var template=document.createElement('template');template.innerHTML='<span>Saved</span><template id=n><b>Nested</b></template>';root.appendChild(template);r.selectNode(template);
var tc=r.cloneContents().firstChild;
if(tc===template||tc.ownerDocument!==document||tc.content===template.content||tc.content.ownerDocument!==template.content.ownerDocument||tc.content.firstChild.textContent!=='Saved'||tc.content.lastChild.content.textContent!=='Nested'||tc.childNodes.length||tc.content.isConnected)throw 'template appropriate owner and content roots';
tc.content.firstChild.textContent='Copy';if(template.content.firstChild.textContent!=='Saved')throw 'template payload aliases';
"#);assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
}
#[test]
fn range_clone_contents_hierarchy_errors_preserve_source_and_range(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),doctype=document.doctype,caught=0;r.selectNodeContents(document);
try{r.cloneContents();}catch(e){if(e.name==='HierarchyRequestError')caught++;}
if(r.startContainer!==document||r.startOffset!==0||r.endContainer!==document||doctype.parentNode!==document)throw 'doctype error changed source';
try{r.setStart(doctype,0);}catch(e){if(e.name==='InvalidNodeTypeError')caught++;}
try{r.selectNodeContents(doctype);}catch(e){if(e.name==='InvalidNodeTypeError')caught++;}
try{Range.prototype.cloneContents.call({});}catch(e){if(e instanceof TypeError)caught++;}
if(caught!==4)throw 'range hierarchy and concrete receiver';
r.selectNode(middle);if(r.cloneContents().firstChild.textContent!=='MI')throw 'fresh recovery';
"#);
}
