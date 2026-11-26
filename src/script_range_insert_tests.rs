//! Original native insertion and pre-split hierarchy witnesses.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root>A😀B</div><section id=other><b>Moved</b></section><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://insert.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),text=root.firstChild,other=document.getElementById('other'),moved=other.firstChild,r=document.createRange();r.setStart(text,2);r.collapse(true);var insert=r.insertNode;");(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn range_insert_exact_split_native_moves_collapsed_end_and_fragment_splicing(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var state={},calls=0;moved.author=state;moved.addEventListener('check',function(){calls++;});if(r.insertNode(moved)!==undefined||text.data!=='A\uD83D'||root.childNodes.length!==3||root.childNodes[1]!==moved||moved.nextSibling.data!=='\uDE00B'||other.firstChild!==null||moved.author!==state||r.startContainer!==text||r.startOffset!==2||r.endContainer!==root||r.endOffset!==2||r.collapsed)throw 'exact native split/move/collapsed-end';moved.dispatchEvent(new Event('check'));if(calls!==1)throw 'moved listener retained';
var fragment=document.createDocumentFragment(),a=document.createElement('i'),b=document.createTextNode('Tail');a.textContent='First';fragment.appendChild(a);fragment.appendChild(b);var before=root.childNodes[2],s=new Range();s.setStart(root,2);s.collapse(true);s.insertNode(fragment);if(fragment.firstChild!==null||root.childNodes[2]!==a||root.childNodes[3]!==b||root.childNodes[4]!==before||s.startContainer!==root||s.startOffset!==2||s.endContainer!==root||s.endOffset!==4)throw 'genuine fragment splicing and inserted extent';
var empty=document.createDocumentFragment(),atText=new Range();atText.setStart(b,2);atText.collapse(true);atText.insertNode(empty);if(b.data!=='Ta'||b.nextSibling.data!=='il'||atText.startContainer!==b||atText.startOffset!==2||atText.endContainer!==root||atText.endOffset!==4||atText.collapsed)throw 'empty fragment still splits Text and updates collapsed end';
"#);}
#[test]
fn range_insert_existing_reference_detachment_noncollapsed_and_native_hierarchy_precedes_split(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var first=document.createElement('i'),second=document.createElement('em'),third=document.createElement('strong');first.textContent='One';second.textContent='Two';third.textContent='Three';other.appendChild(first);other.appendChild(second);other.appendChild(third);var s=new Range();s.setStart(other,2);s.collapse(true);s.insertNode(second);if(other.childNodes[2]!==second||other.childNodes[3]!==third||s.startOffset!==2||s.endOffset!==3)throw 'incoming equals reference adjusts before removal';
s.setStart(other,1);s.setEnd(other,4);s.insertNode(third);if(other.childNodes[1]!==third||s.startOffset!==1||s.endOffset!==4)throw 'moving existing node preserves noncollapsed live boundaries';
var cases=[root,text,document,document.doctype,document.createAttribute('x')],caught=0,before=text.data,children=root.childNodes.length;for(var node of cases){try{r.insertNode(node);}catch(e){if(e instanceof DOMException&&e.name==='HierarchyRequestError')caught++;}}if(caught!==5||text.data!==before||root.childNodes.length!==children||!r.collapsed||r.startContainer!==text||r.startOffset!==2)throw 'hierarchy before split has no source effects';
var detached=document.createTextNode('Detached'),comment=document.createComment('Comment');for(var node of [detached,comment]){s.setStart(node,1);s.collapse(true);var failed=false;try{s.insertNode(document.createElement('b'));}catch(e){failed=e.name==='HierarchyRequestError';}if(!failed||node.data!==(node===detached?'Detached':'Comment')||!s.collapsed)throw 'invalid start kind or detached Text before split';}
"#);}
#[test]
fn range_insert_observers_other_live_ranges_static_snapshots_and_iterator_references(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var observer=new MutationObserver(function(){});observer.observe(document.body,{subtree:true,childList:true,characterData:true,characterDataOldValue:true});var live=r.cloneRange(),fixed=new StaticRange({startContainer:text,startOffset:2,endContainer:text,endOffset:4}),iterator=document.createNodeIterator(other);iterator.nextNode();iterator.nextNode();insert.call(r,moved);var records=observer.takeRecords();if(records.length!==4||records[0].type!=='childList'||records[0].target!==root||records[0].addedNodes[0]!==moved.nextSibling||records[1].type!=='characterData'||records[1].target!==text||records[1].oldValue!=='A😀B'||records[2].target!==other||records[2].removedNodes[0]!==moved||records[3].target!==root||records[3].addedNodes[0]!==moved)throw 'ordered split/remove/insert native records';if(live.startContainer!==text||live.endContainer!==text||live.startOffset!==2||!live.collapsed||fixed.endContainer!==text||fixed.endOffset!==4||iterator.referenceNode!==other||iterator.nextNode()!==null)throw 'real live/static/iterator integration';
"#);}
#[test]
fn range_insert_native_brands_borrowed_realm_and_resource_refusal_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,method=w.Range.prototype.insertNode,caught=0;for(var fake of [{},document,new StaticRange({startContainer:text,startOffset:0,endContainer:text,endOffset:0})]){try{method.call(fake,moved);}catch(e){if(e instanceof w.TypeError)caught++;}}try{method.call(r,{nodeType:1});}catch(e){if(e instanceof w.TypeError)caught++;}if(caught!==4)throw 'native method and argument brands';method.call(r,moved);if(moved.parentNode!==root||r.endContainer!==root||r.endOffset!==2)throw 'borrowed native insertion';");
 for profile in 0..4{let(mut d,mut f,mut p)=fixture();p.begin_job();let before=(p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);let limit=p.state.retained_limit;match profile{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>{p.state.host_work=4096;p.state.execution_deadline=Some(Instant::now());}}assert!(p.runtime.execute("insert.call(r,moved)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);p.state.retained_limit=limit;run(&mut d,&mut f,&mut p,r#"if(text.data!=='A😀B'||moved.parentNode!==other||!r.collapsed)throw 'initial insertion refusal preserves source';insert.call(r,moved);if(moved.parentNode!==root||text.data!=='A\uD83D'||r.endOffset!==2)throw 'native insertion fresh recovery';"#);}
}

#[test]
fn range_insert_captured_origin_loss_refuses_before_effects_and_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var foreignInsert=frame.contentWindow.Range.prototype.insertNode;");let before=(p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);
 p.set_location(&crate::navigation::address("https://foreign.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();let error=p.runtime.execute("foreignInsert.call(r,moved)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin"),"{error}");assert_eq!((p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);
 p.set_location(&crate::navigation::address("https://insert.test/source",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,"if(text.data!=='A😀B'||moved.parentNode!==other||!r.collapsed)throw 'origin refusal before split';foreignInsert.call(r,moved);if(moved.parentNode!==root||r.endContainer!==root||r.endOffset!==2)throw 'captured origin recovery';");
}

#[test]
fn range_insert_inert_promotion_cdata_split_and_preflight_representation_refusal(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var owner=document.implementation.createHTMLDocument('source'),incoming=owner.createElement('span');incoming.textContent='Inert';owner.body.appendChild(incoming);var state={};incoming.author=state;r.insertNode(incoming);if(incoming.ownerDocument!==document||incoming.parentNode!==root||incoming.author!==state||owner.body.firstChild!==null||root.childNodes[1]!==incoming||r.endOffset!==2)throw 'native inert promotion retains identity';
var xml=document.implementation.createDocument(null,''),cd=xml.createCDATASection('C😀D');root.appendChild(cd);var s=new Range();s.setStart(cd,2);s.collapse(true);s.insertNode(document.createComment('inserted'));if(cd.nodeType!==4||cd.data!=='C\uD83D'||cd.nextSibling.nodeType!==8||cd.nextSibling.nextSibling.nodeType!==3||cd.nextSibling.nextSibling.data!=='\uDE00D'||s.endContainer!==root||s.endOffset!==5)throw 'promoted CDATA split creates genuine Text tail';
var unsupported=xml.createElement('ordinary'),before=cd.data,count=root.childNodes.length;s.setStart(cd,1);s.collapse(true);var caught=false;try{s.insertNode(unsupported);}catch(e){caught=e instanceof DOMException&&e.name==='NotSupportedError';}if(!caught||cd.data!==before||root.childNodes.length!==count||unsupported.ownerDocument!==xml||unsupported.parentNode!==null||!s.collapsed)throw 'unsupported source producer refuses before split';
"#);}

#[test]
fn range_insert_late_copy_refusal_preserves_split_and_detachment_prefixes(){
 let mut split_prefix=false;let mut detached_prefix=false;
 for budget in (0..16384).step_by(8){
  let(mut d,mut f,mut p)=fixture();p.begin_job();p.state.copy_work=budget;let result=p.runtime.execute("insert.call(r,moved)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state});if result.is_ok(){continue;}
  p.begin_job();let split=p.runtime.execute(r#"text.data==='A\uD83D' && text.nextSibling.data==='\uDE00B' && r.collapsed"#,&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();if !matches!(split,Value::Bool(true)){continue;}
  let detached=matches!(p.runtime.execute("moved.parentNode===null",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap(),Value::Bool(true));if detached{if detached_prefix{continue;}detached_prefix=true;}else{if split_prefix{continue;}split_prefix=true;}
  run(&mut d,&mut f,&mut p,"var tail=text.nextSibling;r.setStart(root,1);r.collapse(true);insert.call(r,moved);if(moved.parentNode!==root||root.childNodes[1]!==moved||moved.nextSibling!==tail||r.endContainer!==root||r.endOffset!==2)throw 'completed split and removal prefixes permit fresh insertion recovery';");if split_prefix&&detached_prefix{break;}
 }
 assert!(split_prefix&&detached_prefix,"late refusal witnesses: split={split_prefix} detached={detached_prefix}");
}
