//! Inert snapshot endpoints retain native identities and appropriate owners.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root>Main</div></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn static_range_inert_documents_fragments_character_kinds_and_exact_fixed_points(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var owner=new Document(),fragment=owner.createDocumentFragment(),text=owner.createTextNode('\uD800X\uDC00'),cdata=owner.createCDATASection('\uD800X\uDC00'),comment=owner.createComment('\uD800X\uDC00'),pi=owner.createProcessingInstruction('p','\uD800X\uDC00');
for(var node of [text,cdata,comment,pi]){fragment.appendChild(node);var fixed=new StaticRange({startContainer:node,startOffset:1,endContainer:node,endOffset:2});if(fixed.startContainer!==node||fixed.endContainer!==node||fixed.startOffset!==1||fixed.endOffset!==2||fixed.startContainer.ownerDocument!==owner||node.nodeValue.charCodeAt(0)!==55296||node.nodeValue.charCodeAt(2)!==56320)throw 'native inert exact identity';node.insertData(0,'Y');if(fixed.startOffset!==1||fixed.endOffset!==2||fixed.startContainer!==node||node.nodeValue.charCodeAt(1)!==55296)throw 'inert splice changes fixed point';}
var roots=new StaticRange({startContainer:owner,startOffset:999,endContainer:fragment,endOffset:999});fragment.appendChild(owner.createTextNode('Tail'));if(roots.startContainer!==owner||roots.endContainer!==fragment||roots.startOffset!==999||roots.endOffset!==999)throw 'inert document and fragment invalid snapshot';
var element=owner.createElementNS('http://www.w3.org/1999/xhtml','div');fragment.appendChild(element);var elements=new StaticRange({startContainer:element,startOffset:99,endContainer:fragment,endOffset:99});element.appendChild(text);if(elements.startContainer!==element||elements.startOffset!==99||elements.endContainer!==fragment||elements.endOffset!==99||element.ownerDocument!==owner||element.namespaceURI!=='http://www.w3.org/1999/xhtml')throw 'inert element identity and native children';
var main=document.getElementById('root').firstChild,mixed=new StaticRange({startContainer:main,startOffset:1,endContainer:text,endOffset:2});main.insertData(0,'X');if(mixed.startOffset!==1||mixed.endOffset!==2||mixed.startContainer!==main||mixed.endContainer!==text)throw 'mixed root snapshot';
"#);assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.has_navigation_request());
}
#[test]
fn static_range_inert_template_owners_clones_imports_and_live_range_guard(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var parsed=new DOMParser().parseFromString('<!doctype html><body><template id=t><span>Saved</span><template><b>Nested</b></template></template></body>','text/html'),host=parsed.getElementById('t'),content=host.content,owner=content.ownerDocument,first=content.firstChild.firstChild,last=content.lastChild.content.firstChild.firstChild;
var fixed=new StaticRange({startContainer:first,startOffset:1,endContainer:last,endOffset:2}),whole=new StaticRange({startContainer:content,startOffset:0,endContainer:parsed,endOffset:9});
var copy=host.cloneNode(true),imported=document.importNode(host,true);first.nodeValue='Changed';
if(fixed.startContainer!==first||fixed.endContainer!==last||fixed.startOffset!==1||fixed.endOffset!==2||first.ownerDocument!==owner||last.ownerDocument!==owner||whole.startContainer!==content||whole.endContainer!==parsed||whole.endOffset!==9)throw 'appropriate owners and separate native roots';
if(copy.content.firstChild.textContent!=='Saved'||imported.content.firstChild.textContent!=='Saved'||copy.content.firstChild.firstChild===first||imported.content.ownerDocument===owner||imported.ownerDocument!==document||content.ownerDocument!==owner)throw 'independent copies and snapshot source aliases';
var caught=0,live=document.createRange();try{live.setStart(first,0);}catch(e){if(e.name==='NotSupportedError')caught++;}
var attr=owner.createAttribute('title');try{new StaticRange({startContainer:first,startOffset:0,endContainer:attr,endOffset:0});}catch(e){if(e.name==='InvalidNodeTypeError')caught++;}
if(parsed.doctype.nodeType!==10)throw 'genuine inert doctype producer';try{new StaticRange({startContainer:first,startOffset:0,endContainer:parsed.doctype,endOffset:0});}catch(e){if(e instanceof DOMException&&e.name==='InvalidNodeTypeError')caught++;}
if(caught!==3||fixed.startContainer!==first||first.nodeValue!=='Changed')throw 'live and invalid endpoint guards';
"#);
}
#[test]
fn static_range_inert_storage_copy_work_deadline_refusal_and_native_recovery(){
 for profile in 0..4 {
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var owner=new Document(),node=owner.createTextNode('Saved'),warm=new StaticRange({startContainer:node,startOffset:1,endContainer:document,endOffset:2});");
  p.begin_job();let Value::Host(node)=p.runtime.execute("node",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap() else {panic!("inert text")};
  let fields=[("endContainer",Value::Host(DOCUMENT)),("endOffset",Value::Number(2.0)),("startContainer",Value::Host(node)),("startOffset",Value::Number(1.0))];
  let mut bridge=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};
  bridge.validate_static_range_fields(Some(&fields)).unwrap();
  while bridge.state.ranges.len()<bridge.state.ranges.capacity(){bridge.create_static_range(WINDOW,Some(&fields)).unwrap();}
  let before=(bridge.state.retained,bridge.state.ranges.len(),bridge.state.ranges.capacity(),bridge.doc.nodes.len(),bridge.state.dom_commits);let limit=bridge.state.retained_limit;
  match profile {0=>bridge.state.host_work=0,1=>bridge.state.copy_work=0,2=>bridge.state.retained_limit=bridge.state.retained,_=>{bridge.state.host_work=4096;bridge.state.execution_deadline=Some(Instant::now());}}
  assert!(bridge.create_static_range(WINDOW,Some(&fields)).is_err());
  assert_eq!((bridge.state.retained,bridge.state.ranges.len(),bridge.state.ranges.capacity(),bridge.doc.nodes.len(),bridge.state.dom_commits),before);
  bridge.state.retained_limit=limit;
  run(&mut d,&mut f,&mut p,"var recovered=new StaticRange({startContainer:node,startOffset:1,endContainer:document,endOffset:2});if(recovered.startContainer!==node||recovered.endContainer!==document||recovered.startOffset!==1||node.nodeValue!=='Saved'||node.ownerDocument!==owner)throw 'inert native quota recovery';");
 }
}
