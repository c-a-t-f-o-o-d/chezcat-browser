//! Original parsed-template ownership, copy and atomic publication witnesses.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<!doctype html><body><div id=mount></div><input id=neighbor value=seed></body>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://parsed-templates.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn inactive(p:&mut Page){assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());let(a,b,c)=p.take_image_changes();assert!(a.is_empty()&&b.is_empty()&&c.is_empty());}
#[test]
fn domparser_template_publication_nested_owners_roots_and_empty_siblings(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var parsed=new DOMParser().parseFromString('<!doctype html><html><head></head><body><template id=t>A<span id=inside data-note=kept>😀</span><template id=n><b id=deep>Nested</b></template>Z</template><template id=e></template><p id=outside>Outside</p></body></html>','text/html');
var t=parsed.getElementById('t'),e=parsed.getElementById('e'),content=t.content,owner=content.ownerDocument,n=content.querySelector('#n'),deep=n.content.firstChild;
if(t.ownerDocument!==parsed||owner===parsed||owner===document||owner.nodeType!==9||owner.defaultView!==null||owner.documentElement!==null||content.parentNode!==null||content.isConnected||t.childNodes.length!==0||t.firstChild!==null)throw 'separate appropriate owner';
if(content!==t.content||e.content===content||e.content.ownerDocument!==owner||e.content.childNodes.length!==0||n.ownerDocument!==owner||n.content.ownerDocument!==owner||deep.ownerDocument!==owner||deep.parentNode!==n.content||n.childNodes.length)throw 'nested shared owner and independent roots';
if(content.childNodes.length!==4||content.firstChild.nodeValue!=='A'||content.lastChild.nodeValue!=='Z'||parsed.getElementById('inside')!==null||parsed.querySelector('#deep')!==null||t.querySelector('span')!==null||content.querySelector('#inside').textContent!=='😀'||deep.textContent!=='Nested')throw 'order query root separation';
var attr=content.querySelector('#inside').getAttributeNode('data-note');if(attr.ownerDocument!==owner||attr.value!=='kept'||parsed.body.lastChild.id!=='outside')throw 'attribute owner';
"#);inactive(&mut p);
}
#[test]
fn domparser_template_publication_clone_import_and_saved_aliases(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var parsed=new DOMParser().parseFromString('<body><template id=t><span data-note=seed>Source😀</span><template id=n><b>Nested</b></template></template></body>','text/html'),t=parsed.getElementById('t'),content=t.content,owner=content.ownerDocument,first=content.firstChild,attr=first.getAttributeNode('data-note');
var copy=parsed.cloneNode(true),ct=copy.getElementById('t'),tc=t.cloneNode(true),shallow=t.cloneNode(false);
if(ct===t||ct.ownerDocument!==copy||ct.content.ownerDocument===owner||ct.content.ownerDocument===copy||ct.content.firstChild.textContent!=='Source😀'||ct.content.lastChild.content.ownerDocument!==ct.content.ownerDocument)throw 'document clone appropriate owner';
if(tc.ownerDocument!==parsed||tc.content.ownerDocument!==owner||tc.content===content||tc.content.firstChild===first||shallow.content.childNodes.length||shallow.content.ownerDocument!==owner)throw 'template native clone';
var stamp=document.importNode(content,true),mount=document.getElementById('mount');if(stamp.ownerDocument!==document||stamp.firstChild.ownerDocument!==document||stamp.lastChild.content.ownerDocument===owner)throw 'import destination owners';
mount.appendChild(stamp);if(mount.firstChild.textContent!=='Source😀'||mount.lastChild.content.firstChild.textContent!=='Nested'||content.firstChild!==first||first.getAttributeNode('data-note')!==attr||attr.ownerDocument!==owner||first.ownerDocument!==owner)throw 'independent stamp and aliases';
ct.content.firstChild.textContent='Copy';if(first.textContent!=='Source😀'||mount.firstChild.textContent!=='Source😀')throw 'independent live payload';
"#);inactive(&mut p);
}
#[test]
fn domparser_template_publication_namespace_payload_and_resource_inertness(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var parsed=new DOMParser().parseFromString('<body><template id=t><span data-note="😀">Ω😀</span><script>window.templateEscaped=true;</script><img src="https://inactive-parsed-template.test/a.png"></template></body>','text/html'),t=parsed.getElementById('t'),c=t.content,txt=c.firstChild;
if(txt.namespaceURI!=='http://www.w3.org/1999/xhtml'||txt.textContent!=='Ω😀'||txt.getAttribute('data-note')!=='😀'||txt.ownerDocument!==c.ownerDocument||typeof window.templateEscaped!=='undefined'||parsed.defaultView!==null)throw 'native namespace scalar payload and inactive sources';
"#);inactive(&mut p);
    // Full-document foreign-template parsing retains its existing F05 guard.
    // Publication must not invent a namespace URI from an unsupported record.
    p.begin_job();
    let refused=p.runtime.execute("var foreign=new DOMParser().parseFromString('<body><template id=t><svg><text>guarded</text></svg></template></body>','text/html');foreign.getElementById('t').content.firstChild.namespaceURI;",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();
    assert!(refused.contains("unsupported foreign parser context"),"{refused}");
    inactive(&mut p);
}
#[test]
fn domparser_template_publication_atomic_late_refusal_and_fresh_recovery(){
 for mode in 0..4{
  let(mut d,mut f,mut p)=fixture();let(parsed,dtype)=crate::html::parse_inert_html("<!doctype html><body><template id=t><span>kept</span><template id=n><b>nested</b></template></template></body>",100,&||Ok(())).unwrap();let roots=document_elements(&parsed.nodes).unwrap();let count=parsed.nodes.len()+5;
  let before=(p.state.retained,p.state.inert_documents.entries.len(),p.state.inert_documents.generated.len(),p.state.inert_documents.template_contents.len(),p.state.inert_documents.template_documents.len(),d.nodes.len(),p.state.dom_commits);let limit=p.state.retained_limit;p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  assert!(Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.publish_parsed_document(parsed,"https://parsed-templates.test/refused".into(),roots,dtype,count).is_err(),"mode={mode}");
  assert_eq!((p.state.retained,p.state.inert_documents.entries.len(),p.state.inert_documents.generated.len(),p.state.inert_documents.template_contents.len(),p.state.inert_documents.template_documents.len(),d.nodes.len(),p.state.dom_commits),before,"mode={mode}");
  p.state.retained_limit=limit;run(&mut d,&mut f,&mut p,"var restored=new DOMParser().parseFromString('<body><template id=t><b>Recovered</b></template></body>','text/html');if(restored.getElementById('t').content.firstChild.textContent!=='Recovered')throw 'fresh publication recovery';");inactive(&mut p);
 }
}

#[test]
fn domparser_template_publication_exact_owned_storage_includes_fragments_and_associated_document(){
 let(mut d,mut f,mut p)=fixture();
 let(mut parsed,dtype)=crate::html::parse_inert_html("<!doctype html><body><template id=t><span data-note=owned>😀</span><template id=n><b>Nested</b></template></template><template id=e></template></body>",100,&||Ok(())).unwrap();
 for node in &mut parsed.nodes{if let NodeKind::Element{tag,href,..}=&mut node.kind{if tag=="span"{tag.reserve(128);*href=Some(String::with_capacity(256));}}}
 let roots=document_elements(&parsed.nodes).unwrap();let count=parsed_document_entries(&parsed.nodes).unwrap();
 let mut b=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};
 let before=(b.state.retained,b.template_import_backing(),b.state.inert_documents.generated.len());
 let Value::Host(document)=b.publish_parsed_document(parsed,"https://parsed-templates.test/storage".into(),roots,dtype,count).unwrap()else{panic!("parsed Document")};
 let associated=*b.state.inert_documents.template_documents.get(&document).unwrap();
 let mut payload=0usize;let mut pending=vec![document,associated];let mut fragments=0;
 while let Some(handle)=pending.pop(){
  let node=b.state.inert_documents.node(handle).unwrap();payload+=node.children.capacity()*size_of::<u32>();pending.extend_from_slice(&node.children);
  match &node.kind{Kind::Document(record)=>payload+=record.url.capacity()+record.title.capacity(),Kind::Fragment{..}=>{fragments+=1;assert_eq!(node.owner,associated);assert_eq!(node.parent,None);},_=>{}}
  if let Some(&content)=b.state.inert_documents.template_contents.get(&handle){pending.push(content);}
 }
 assert_eq!(fragments,3);
 for source in b.state.inert_documents.generated.iter().skip(before.2){
  payload+=node_bytes(&source.node)+size_of::<Node>()+2*size_of::<usize>();
  if let NodeKind::Element{tag,href,..}=&source.node.kind{payload+=tag.capacity()+href.as_ref().map_or(0,String::capacity);}
 }
 assert_eq!(b.state.retained-before.0,payload+b.template_import_backing()-before.1);
}
