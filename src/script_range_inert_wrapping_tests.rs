//! Genuine inert insertion and surrounding share native transactions.
use super::*;
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root>Main</div></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://inert-wrap.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);run(&mut d,&mut f,&mut p,"var owner=new Document(),root=owner.createElement('div'),text=owner.createTextNode('A😀B'),other=owner.createElement('div'),moved=owner.createElement('b');root.appendChild(text);moved.textContent='X';other.appendChild(moved);var r=owner.createRange();r.setStart(text,2);r.collapse(true);var insert=r.insertNode,surround=r.surroundContents;");(d,f,p)}
#[test]
fn range_inert_wrapping_insert_split_fragments_moves_self_reference_and_extent(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var fixed=new StaticRange({startContainer:text,startOffset:2,endContainer:text,endOffset:4}),live=r.cloneRange();r.insertNode(moved);if(text.data!=='A\uD83D'||moved.parentNode!==root||other.firstChild!==null||moved.nextSibling.data!=='\uDE00B'||moved.nextSibling.nodeType!==3||r.startContainer!==text||r.startOffset!==2||r.endContainer!==root||r.endOffset!==2||r.toString()!=='X'||fixed.endContainer!==text||fixed.endOffset!==4||live.startContainer!==text||live.startOffset!==2||!live.collapsed)throw 'native inert split move and actual range extent';
var fragment=owner.createDocumentFragment(),i=owner.createElement('i'),em=owner.createElement('em');i.textContent='I';em.textContent='M';fragment.appendChild(i);fragment.appendChild(em);r.setStart(root,2);r.collapse(true);r.insertNode(fragment);if(fragment.firstChild!==null||root.childNodes[2]!==i||root.childNodes[3]!==em||r.startContainer!==root||r.startOffset!==2||r.endOffset!==4||r.toString()!=='IM')throw 'native inert fragment transfer and count';r.setStart(root,2);r.collapse(true);r.insertNode(i);if(root.childNodes[2]!==i||r.endOffset!==3||r.toString()!=='I')throw 'incoming equals actual reference';r.setStart(root,0);r.collapse(true);r.insertNode(em);if(root.firstChild!==em||r.endOffset!==1||r.toString()!=='M')throw 'same-parent native move';
var cd=owner.createCDATASection('C😀D');root.appendChild(cd);r.setStart(cd,2);r.collapse(true);r.insertNode(owner.createComment('inserted'));if(cd.nodeType!==4||cd.data!=='C\uD83D'||cd.nextSibling.nodeType!==8||cd.nextSibling.nextSibling.nodeType!==3||cd.nextSibling.nextSibling.data!=='\uDE00D')throw 'actual inert CDATA split with Text tail';
"#);}
#[test]
fn range_inert_wrapping_insert_hierarchy_before_split_and_actual_document_metadata(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var caught=false;try{r.insertNode(root);}catch(e){caught=e instanceof DOMException&&e.name==='HierarchyRequestError';}if(!caught||text.data!=='A😀B'||root.childNodes.length!==1||!r.collapsed)throw 'cycle refuses before native split';for(var bad of [owner,owner.createAttribute('x')]){caught=false;try{r.insertNode(bad);}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||text.data!=='A😀B')throw 'invalid node kind before native split';}
var standalone=owner.createTextNode('Detached'),point=owner.createRange();point.setStart(standalone,1);caught=false;try{point.insertNode(moved);}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||moved.parentNode!==other||standalone.data!=='Detached')throw 'detached inert Text hierarchy';
var doc=new Document(),docRange=doc.createRange(),element=doc.createElement('Root');docRange.insertNode(element);if(doc.documentElement!==element||docRange.startContainer!==doc||docRange.startOffset||docRange.endOffset!==1)throw 'actual inert Document insertion and metadata';var type=doc.implementation.createDocumentType('Root','','');docRange.setStart(doc,0);docRange.collapse(true);docRange.insertNode(type);if(doc.doctype!==type||doc.firstChild!==type||docRange.endOffset!==1)throw 'actual inert Document Doctype insertion';caught=false;docRange.setStart(doc,2);docRange.collapse(true);try{docRange.insertNode(doc.createElement('Other'));}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||doc.documentElement!==element||doc.childNodes.length!==2)throw 'native Document count and order validation';
"#);}
#[test]
fn range_inert_wrapping_surround_identity_clear_extract_insert_append_and_select(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var parsed=new DOMParser().parseFromString('<body><div>A😀B</div><div><b>X</b></div></body>','text/html');owner=parsed;root=parsed.body.firstChild;text=root.firstChild;other=root.nextSibling;moved=other.firstChild;r=owner.createRange();
var saved={},calls=0;moved.author=saved;moved.addEventListener('check',function(){calls++;});var discarded=moved.firstChild;r.setStart(text,1);r.setEnd(text,3);var obs=new MutationObserver(function(){throw 'drained';});obs.observe(root,{subtree:true,childList:true,characterData:true,characterDataOldValue:true});var donor=new MutationObserver(function(){throw 'drained';});donor.observe(moved,{childList:true});var fixed=new StaticRange({startContainer:text,startOffset:1,endContainer:text,endOffset:3});r.surroundContents(moved);if(root.textContent!=='A😀B'||root.childNodes[1]!==moved||text.data!=='A'||moved.textContent!=='😀'||moved.firstChild===text||moved.author!==saved||moved.ownerDocument!==owner||discarded.parentNode!==null||other.firstChild!==null||r.startContainer!==root||r.startOffset!==1||r.endOffset!==2||r.toString()!=='😀'||fixed.endContainer!==text||fixed.endOffset!==3)throw 'native inert ordered surround transaction and final selection';moved.dispatchEvent(new Event('check'));if(calls!==1)throw 'wrapper actual listener identity';var records=obs.takeRecords(),changes=donor.takeRecords();if(records.length!==5||records[4].target!==moved||records[4].addedNodes[0]!==moved.firstChild||records[0].target!==text||records[0].oldValue!=='A😀B'||records[1].type!=='childList'||records[1].target!==root||records[1].addedNodes[0]!==root.lastChild||records[2].type!=='characterData'||records[2].target!==text||records[2].oldValue!=='AB'||records[3].addedNodes[0]!==moved||changes.length!==2||changes[0].removedNodes[0]!==discarded||changes[1].addedNodes[0]!==moved.firstChild)throw 'real extraction clear split insertion append observations';
"#);}
#[test]
fn range_inert_wrapping_surround_error_precedence_and_late_character_parent_prefix(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var b=owner.createElement('b');b.textContent='End';root.appendChild(b);r.setStart(text,1);r.setEnd(b.firstChild,1);var caught=false;try{r.surroundContents(owner);}catch(e){caught=e instanceof DOMException&&e.name==='InvalidStateError';}if(!caught||text.data!=='A😀B'||b.textContent!=='End')throw 'partial nonText before bad wrapper kind';r.setStart(text,1);r.setEnd(text,3);caught=false;try{r.surroundContents(owner.createDocumentFragment());}catch(e){caught=e.name==='InvalidNodeTypeError';}if(!caught||text.data!=='A😀B')throw 'invalid wrapper kind before extraction';var wrapper=owner.createTextNode('Wrapper');caught=false;try{r.surroundContents(wrapper);}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught||text.data!=='A'||wrapper.parentNode!==root||wrapper.data!=='Wrapper'||r.startContainer!==text||r.startOffset!==1||r.endContainer!==root||r.endOffset!==2)throw 'late CharacterData append error preserves extraction insertion prefix';
"#);}
#[test]
fn range_inert_wrapping_resource_refusal_before_split_and_fresh_native_recovery(){
 for profile in 0..4{let(mut d,mut f,mut p)=fixture();p.begin_job();let before=(p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);let limit=p.state.retained_limit;match profile{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>{p.state.host_work=4096;p.state.execution_deadline=Some(Instant::now());}}assert!(p.runtime.execute("insert.call(r,moved)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);p.state.retained_limit=limit;run(&mut d,&mut f,&mut p,"if(text.data!=='A😀B'||moved.parentNode!==other||!r.collapsed)throw 'initial inert insertion refusal';insert.call(r,moved);if(moved.parentNode!==root||r.endOffset!==2)throw 'fresh inert insertion recovery';");}
}

#[test]
fn range_inert_wrapping_insert_captured_origin_loss_refuses_before_effects_and_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var foreignInsert=frame.contentWindow.Range.prototype.insertNode;");let before=(p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);
 p.set_location(&crate::navigation::address("https://foreign.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();let error=p.runtime.execute("foreignInsert.call(r,moved)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin"),"{error}");assert_eq!((p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);
 p.set_location(&crate::navigation::address("https://inert-wrap.test/source",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,"if(text.data!=='A😀B'||moved.parentNode!==other||!r.collapsed)throw 'origin refusal before split';foreignInsert.call(r,moved);if(moved.parentNode!==root||r.endContainer!==root||r.endOffset!==2)throw 'captured origin recovery';");
}

#[test]
fn range_inert_wrapping_insert_late_copy_refusal_preserves_split_and_detachment_prefixes(){
 let mut split_prefix=false;let mut detached_prefix=false;
 for budget in (0..16384).step_by(8){
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"for(var i=0;i<8;i++)other.appendChild(owner.createComment('following sibling'));");p.begin_job();p.state.copy_work=budget;let result=p.runtime.execute("insert.call(r,moved)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state});if result.is_ok(){continue;}
  p.begin_job();let split=p.runtime.execute(r#"text.data==='A\uD83D' && text.nextSibling.data==='\uDE00B' && r.collapsed"#,&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();if !matches!(split,Value::Bool(true)){continue;}
  let detached=matches!(p.runtime.execute("moved.parentNode===null",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap(),Value::Bool(true));if detached{if detached_prefix{continue;}detached_prefix=true;}else{if split_prefix{continue;}split_prefix=true;}
  run(&mut d,&mut f,&mut p,"var tail=text.nextSibling;r.setStart(root,1);r.collapse(true);insert.call(r,moved);if(moved.parentNode!==root||root.childNodes[1]!==moved||moved.nextSibling!==tail||r.endContainer!==root||r.endOffset!==2)throw 'completed split and removal prefixes permit fresh insertion recovery';");if split_prefix&&detached_prefix{break;}
 }
 assert!(split_prefix&&detached_prefix,"late refusal witnesses: split={split_prefix} detached={detached_prefix}");
}

#[test]
fn range_inert_wrapping_surround_late_copy_refusal_keeps_extraction_and_insertion_prefixes(){
 let mut extracted=false;let mut inserted=false;
 for budget in (0..16384).step_by(8){
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"r.setStart(text,1);r.setEnd(text,3);");p.begin_job();p.state.copy_work=budget;
  if p.runtime.execute("surround.call(r,moved)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_ok(){continue;}
  p.begin_job();let data=p.runtime.execute("text.data==='AB' && moved.parentNode===other",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();let node=p.runtime.execute("text.data==='A' && moved.parentNode===root && moved.firstChild===null",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();
  let data=matches!(data,Value::Bool(true));let node=matches!(node,Value::Bool(true));if !data&&!node{continue;}if data&&extracted||node&&inserted{continue;}extracted|=data;inserted|=node;
  run(&mut d,&mut f,&mut p,"var before=root.textContent;r.setStart(root,0);r.setEnd(root,root.childNodes.length);var recovered=owner.createElement('em');r.surroundContents(recovered);if(root.childNodes.length!==1||root.firstChild!==recovered||recovered.textContent!==before||r.startContainer!==root||r.startOffset||r.endOffset!==1)throw 'fresh native wrapping retains actual committed prefixes';");
  if extracted&&inserted{break;}
 }assert!(extracted&&inserted,"inert surrounding late prefixes extraction={extracted} insertion={inserted}");
}

#[test]
fn range_inert_wrapping_generic_element_clear_preserves_native_observations_and_range_updates(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var generic=owner.createElement('ordinary'),child=owner.createTextNode('\uD800A\uDC00');generic.appendChild(child);var points=owner.createRange();points.setStart(child,1);points.setEnd(child,3);var fixed=new StaticRange({startContainer:child,startOffset:1,endContainer:child,endOffset:3}),watch=new MutationObserver(function(){throw 'drained';});watch.observe(generic,{childList:true});generic.textContent='\uD800X\uDC00';var records=watch.takeRecords();if(generic.namespaceURI!==null||generic.firstChild.data.length!==3||generic.firstChild.data.charCodeAt(0)!==55296||generic.firstChild.data.charCodeAt(2)!==56320||generic.firstChild.ownerDocument!==owner||child.parentNode!==null||points.startContainer!==generic||points.startOffset||points.endOffset||fixed.endContainer!==child||fixed.endOffset!==3||records.length!==1||records[0].removedNodes[0]!==child||records[0].addedNodes[0]!==generic.firstChild)throw 'real namespace-null element replacement and observations';generic.textContent=null;if(generic.firstChild!==null||watch.takeRecords().length!==1)throw 'actual generic wrapper clear';
"#);assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.has_navigation_request());
}
