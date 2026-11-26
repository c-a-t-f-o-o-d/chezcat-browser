//! Original selected native deletion witnesses.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root><b>A😀B</b><span>Middle</span><em>End</em></div><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://delete.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),first=root.firstChild.firstChild,last=root.lastChild.firstChild,middle=root.childNodes[1],r=document.createRange();r.setStart(first,1);r.setEnd(last,2);var remove=r.deleteContents;");(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn range_delete_partial_ancestors_exact_units_and_detached_character_kinds(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
if(r.deleteContents()!==undefined||root.textContent!=='Ad'||root.childNodes.length!==2||middle.parentNode!==null||first.data!=='A'||last.data!=='d'||!r.collapsed||r.startContainer!==root||r.startOffset!==1)throw 'partial native structure and collapse';
var t=document.createTextNode('A😀B'),slice=new Range();slice.setStart(t,1);slice.setEnd(t,2);slice.deleteContents();if(t.data!=='A\uDE00B'||!slice.collapsed||slice.startContainer!==t||slice.startOffset!==1)throw 'surrogate exact deletion';
var comment=document.createComment('abcdef');slice.setStart(comment,2);slice.setEnd(comment,5);slice.deleteContents();if(comment.data!=='abf'||slice.startOffset!==2||!slice.collapsed)throw 'comment selected deletion';
var detached=document.createElement('section');detached.innerHTML='<b>One</b><i>Two</i><em>Three</em>';var saved=detached.firstChild;slice.selectNodeContents(detached);slice.deleteContents();if(detached.childNodes.length||saved.parentNode!==null||!slice.collapsed||slice.startContainer!==detached||slice.startOffset)throw 'whole detached native children';
var empty=document.createRange(),before=root.innerHTML;empty.setStart(first,1);empty.collapse(true);empty.deleteContents();if(root.innerHTML!==before||empty.startContainer!==first||empty.startOffset!==1)throw 'collapsed no mutation';
var nested=document.createElement('section');nested.innerHTML='<b>Start</b><i><strong>Whole</strong><small>Tree</small></i><em>Finish</em>';var branch=nested.childNodes[1],leaf=branch.firstChild,finish=nested.lastChild.firstChild;slice.setStart(nested,1);slice.setEnd(finish,2);slice.deleteContents();if(nested.textContent!=='Startnish'||branch.parentNode!==null||leaf.parentNode!==branch||slice.startContainer!==nested||slice.startOffset!==1||!slice.collapsed)throw 'ancestor start and topmost removed subtree';
slice.setStart(nested.firstChild.firstChild,2);slice.setEnd(nested,2);slice.deleteContents();if(nested.textContent!=='St'||nested.childNodes.length!==1||slice.startContainer!==nested||slice.startOffset!==1||!slice.collapsed)throw 'ancestor end and computed collapse';
"#);}
#[test]
fn range_delete_observer_order_live_aliases_static_snapshots_and_iterator_removal(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var observer=new MutationObserver(function(){});observer.observe(root,{subtree:true,childList:true,characterData:true,characterDataOldValue:true});var live=r.cloneRange(),fixed=new StaticRange({startContainer:first,startOffset:1,endContainer:last,endOffset:2}),iterator=document.createNodeIterator(root);while(iterator.nextNode()!==middle){}var inside=document.createRange();inside.selectNodeContents(middle.firstChild);
remove.call(r);var records=observer.takeRecords();if(records.length!==3||records[0].type!=='characterData'||records[0].target!==first||records[0].oldValue!=='A😀B'||records[1].type!=='childList'||records[1].target!==root||records[1].removedNodes.length!==1||records[1].removedNodes[0]!==middle||records[1].previousSibling!==root.firstChild||records[1].nextSibling!==root.lastChild||records[2].target!==last||records[2].oldValue!=='End')throw 'ordered real mutation records';
if(live.startContainer!==first||live.startOffset!==1||live.endContainer!==last||live.endOffset!==0||live.toString()!==''||inside.startContainer!==root||inside.startOffset!==1||!inside.collapsed)throw 'other live ranges follow actual native changes';
if(fixed.startContainer!==first||fixed.startOffset!==1||fixed.endContainer!==last||fixed.endOffset!==2||iterator.referenceNode===middle||iterator.nextNode()!==root.lastChild)throw 'fixed snapshots and actual iterator adjustment';
"#);}
#[test]
fn range_delete_native_brand_realms_and_resource_refusal_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,method=w.Range.prototype.deleteContents,caught=0;for(var fake of [{},document,new StaticRange({startContainer:first,startOffset:0,endContainer:first,endOffset:0})]){try{method.call(fake);}catch(e){if(e instanceof w.TypeError)caught++;}}if(caught!==3)throw 'method realm native brands';var foreign=new Range();foreign.setStart(first,1);foreign.setEnd(first,3);method.call(foreign);if(first.data!=='AB'||foreign.startOffset!==1||!foreign.collapsed)throw 'borrowed realm real deletion';
var guarded=new Range(),savedRoot=document.documentElement;guarded.selectNodeContents(document);var savedEnd=guarded.endOffset,refused=false;try{guarded.deleteContents();}catch(e){refused=e instanceof DOMException&&e.name==='NotSupportedError';}if(!refused||document.documentElement!==savedRoot||guarded.startContainer!==document||guarded.startOffset!==0||guarded.endOffset!==savedEnd||guarded.collapsed)throw 'displayed Document children retain typed producer guard before effects';
"#);
 for profile in 0..4{
  let(mut d,mut f,mut p)=fixture();p.begin_job();let before=(p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);let limit=p.state.retained_limit;
  match profile{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>{p.state.host_work=4096;p.state.execution_deadline=Some(Instant::now());}}
  assert!(p.runtime.execute("remove.call(r)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);p.state.retained_limit=limit;
  run(&mut d,&mut f,&mut p,"if(root.textContent!=='A😀BMiddleEnd'||middle.parentNode!==root)throw 'refused preflight preserves native source';remove.call(r);if(root.textContent!=='Ad'||!r.collapsed)throw 'fresh deletion recovery';");
 }
}

#[test]
fn range_delete_captured_foreign_method_denies_native_origin_loss_and_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,foreignRemove=w.Range.prototype.deleteContents;");
 let before=(p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);
 p.set_location(&crate::navigation::address("https://foreign.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();let error=p.runtime.execute("foreignRemove.call(r)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin"),"{error}");assert_eq!((p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);
 p.set_location(&crate::navigation::address("https://delete.test/source",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,"if(root.textContent!=='A😀BMiddleEnd')throw 'native origin refusal preserves source';foreignRemove.call(r);if(root.textContent!=='Ad'||!r.collapsed)throw 'captured method native recovery';");
}

#[test]
fn range_delete_late_data_refusal_preserves_specified_collapse_and_fresh_recovery(){
 let(mut d,mut f,mut p)=fixture();p.begin_job();let before=p.state.dom_commits;let retained=p.state.retained;
 let handle=match p.runtime.execute("r",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap(){Value::Host(handle)=>handle,_=>panic!("native range")};
 let plan=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.range_delete_plan(handle).unwrap_or_else(|_|panic!("native plan"));let copied=plan.capacity()*size_of::<crate::javascript::range_interface::DeleteStep>();assert_eq!(p.state.retained,retained+copied);p.state.retained-=copied;drop(plan);p.begin_job();p.state.copy_work=copied;
 assert!(p.runtime.execute("remove.call(r)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!(p.state.dom_commits,before);assert_eq!(p.state.retained,retained);
 run(&mut d,&mut f,&mut p,"if(root.textContent!=='A😀BMiddleEnd'||!r.collapsed||r.startContainer!==root||r.startOffset!==1)throw 'specified collapse survives later refused data step';r.setStart(first,1);r.setEnd(last,2);remove.call(r);if(root.textContent!=='Ad'||!r.collapsed)throw 'late data refusal recovery';");
}

#[test]
fn range_delete_late_removal_refusal_keeps_committed_data_prefix_and_live_state(){
 let(mut md,mut mf,mut mp)=fixture();mp.begin_job();let copies=mp.state.copy_work;mp.runtime.execute("first.deleteData(1,3)",&mut Bridge{doc:&mut md,forms:&mut mf,state:&mut mp.state}).unwrap();let prefix=copies-mp.state.copy_work;assert!(prefix>0);
 let(mut d,mut f,mut p)=fixture();p.begin_job();let handle=match p.runtime.execute("r",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap(){Value::Host(handle)=>handle,_=>panic!("native range")};let before=p.state.dom_commits;
 let plan=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.range_delete_plan(handle).unwrap_or_else(|_|panic!("native plan"));let bytes=plan.capacity()*size_of::<crate::javascript::range_interface::DeleteStep>();p.state.retained-=bytes;drop(plan);p.begin_job();p.state.copy_work=bytes+prefix;
 assert!(p.runtime.execute("remove.call(r)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!(p.state.dom_commits,before.map(|generation|generation+1));
 run(&mut d,&mut f,&mut p,"if(first.data!=='A'||middle.parentNode!==root||last.data!=='End'||!r.collapsed||r.startContainer!==root||r.startOffset!==1)throw 'committed data prefix survives later removal refusal';r.setStart(first,1);r.setEnd(last,2);remove.call(r);if(root.textContent!=='Ad'||!r.collapsed)throw 'late removal refusal fresh recovery';");
}

#[test]
fn range_delete_ignores_unselected_resource_subtrees_and_preserves_native_owners(){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root><section id=before><style>#before{color:red}</style></section><b id=start>Start</b><i id=middle>Middle</i><em id=end>Finish</em><section id=after><style>#after{color:blue}</style></section></div></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);
 run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),before=document.getElementById('before'),after=document.getElementById('after'),first=document.getElementById('start').firstChild,last=document.getElementById('end').firstChild,middle=document.getElementById('middle'),r=document.createRange();r.setStart(first,1);r.setEnd(last,2);r.deleteContents();if(first.data!=='S'||last.data!=='nish'||middle.parentNode!==null||root.firstChild!==before||root.lastChild!==after||before.ownerDocument!==document||after.ownerDocument!==document||!r.collapsed||r.startContainer!==root||r.startOffset!==2)throw 'unselected resource branches preserved';");
}
