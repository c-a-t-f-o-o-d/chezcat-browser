//! Original native extraction witnesses preserve moved identities.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root><b data-copy=seed>A😀B</b><span>Middle</span><em>End</em></div><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://extract.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),a=root.firstChild,first=a.firstChild,last=root.lastChild.firstChild,middle=root.childNodes[1],r=document.createRange();r.setStart(first,1);r.setEnd(last,2);var extract=r.extractContents;");(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn range_extract_partial_copies_full_identity_moves_native_owners_and_author_state(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var calls=0,saved={};a.author=saved;a.addEventListener('check',function(){calls++;});middle.author=saved;middle.addEventListener('check',function(){calls+=10;});var result=r.extractContents();if(result.nodeType!==11||result.ownerDocument!==document||result.parentNode!==null||result.childNodes.length!==3||result.textContent!=='😀BMiddleEn'||result.childNodes[1]!==middle||middle.parentNode!==result||middle.author!==saved||root.textContent!=='Ad'||root.childNodes.length!==2)throw 'real fragment copied partials and moved full native node';
if(result.firstChild===a||result.firstChild.firstChild===first||result.firstChild.ownerDocument!==document||result.firstChild.getAttribute('data-copy')!=='seed'||result.firstChild.author!==undefined||a.author!==saved||!r.collapsed||r.startContainer!==root||r.startOffset!==1)throw 'independent partial ancestor and specified collapse';
result.firstChild.dispatchEvent(new Event('check'));if(calls!==0)throw 'partial clone omits listeners';middle.dispatchEvent(new Event('check'));a.dispatchEvent(new Event('check'));if(calls!==11)throw 'moved and original listener identity';result.firstChild.firstChild.data='Changed';if(first.data!=='A'||last.data!=='d')throw 'partial copies independent from source';
"#);}
#[test]
fn range_extract_exact_character_units_collapsed_and_nested_partial_structure(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var t=document.createTextNode('A😀B'),slice=new Range();slice.setStart(t,1);slice.setEnd(t,2);var result=slice.extractContents();if(result.firstChild===t||result.firstChild.data!=='\uD83D'||t.data!=='A\uDE00B'||slice.startContainer!==t||slice.startOffset!==1||!slice.collapsed)throw 'exact surrogate extraction';
var comment=document.createComment('abcdef');slice.setStart(comment,2);slice.setEnd(comment,5);result=slice.extractContents();if(result.firstChild.nodeType!==8||result.firstChild.data!=='cde'||comment.data!=='abf'||slice.startOffset!==2||!slice.collapsed)throw 'comment extraction';
var empty=slice.extractContents();if(empty===result||empty.nodeType!==11||empty.firstChild!==null||empty.ownerDocument!==document)throw 'collapsed distinct native fragment';
var tree=document.createElement('section');tree.innerHTML='<b><i>Start</i><small>Whole</small></b><span>Moved</span><em><strong>Finish</strong></em>';var full=tree.childNodes[1],within=tree.firstChild.lastChild,st=tree.firstChild.firstChild.firstChild,en=tree.lastChild.firstChild.firstChild;slice.setStart(st,2);slice.setEnd(en,2);result=slice.extractContents();if(tree.textContent!=='Stnish'||result.textContent!=='artWholeMovedFi'||result.childNodes[1]!==full||result.firstChild.lastChild!==within||within.parentNode!==result.firstChild||result.firstChild.nodeName!=='B'||result.firstChild.firstChild.nodeName!=='I'||result.lastChild.firstChild.nodeName!=='STRONG'||slice.startContainer!==tree||slice.startOffset!==1||!slice.collapsed)throw 'nested partial clones preserve full subtree identities';
var guarded=new Range(),savedRoot=document.documentElement;guarded.selectNodeContents(document);var before=guarded.endOffset,caught=false;try{guarded.extractContents();}catch(e){caught=e instanceof DOMException&&e.name==='HierarchyRequestError';}if(!caught||document.documentElement!==savedRoot||guarded.startOffset||guarded.endOffset!==before||guarded.collapsed)throw 'contained doctype error before mutation';
"#);}
#[test]
fn range_extract_ordered_observation_live_aliases_fixed_points_and_iterator_reference(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var observer=new MutationObserver(function(){});observer.observe(root,{subtree:true,childList:true,characterData:true,characterDataOldValue:true});var live=r.cloneRange(),fixed=new StaticRange({startContainer:first,startOffset:1,endContainer:last,endOffset:2}),iterator=document.createNodeIterator(root);while(iterator.nextNode()!==middle){}var inside=new Range();inside.selectNodeContents(middle.firstChild);var result=extract.call(r),records=observer.takeRecords();if(records.length!==3||records[0].target!==first||records[0].oldValue!=='A😀B'||records[1].type!=='childList'||records[1].removedNodes[0]!==middle||records[2].target!==last||records[2].oldValue!=='End')throw 'ordered native extraction mutations';if(live.startContainer!==first||live.startOffset!==1||live.endContainer!==last||live.endOffset!==0||inside.startContainer!==root||inside.startOffset!==1||!inside.collapsed||fixed.endContainer!==last||fixed.endOffset!==2||iterator.referenceNode===middle||iterator.nextNode()!==root.lastChild||result.childNodes[1]!==middle)throw 'native live/static/iterator effects';
"#);}
#[test]
fn range_extract_brands_borrowed_realm_and_initial_resource_refusal_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,method=w.Range.prototype.extractContents,caught=0;for(var fake of [{},document,new StaticRange({startContainer:first,startOffset:0,endContainer:first,endOffset:0})]){try{method.call(fake);}catch(e){if(e instanceof w.TypeError)caught++;}}if(caught!==3)throw 'native extract brand realm';var result=method.call(r);if(result.childNodes[1]!==middle||result.ownerDocument!==document||root.textContent!=='Ad')throw 'borrowed native extraction';");
 for profile in 0..4{
  let(mut d,mut f,mut p)=fixture();p.begin_job();let before=(p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);let limit=p.state.retained_limit;
  match profile{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>{p.state.host_work=4096;p.state.execution_deadline=Some(Instant::now());}}
  assert!(p.runtime.execute("extract.call(r)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);p.state.retained_limit=limit;
  run(&mut d,&mut f,&mut p,"if(root.textContent!=='A😀BMiddleEnd'||middle.parentNode!==root)throw 'initial extraction refusal preserves source';var result=extract.call(r);if(result.childNodes[1]!==middle||root.textContent!=='Ad'||!r.collapsed)throw 'fresh native extraction recovery';");
 }
}

#[test]
fn range_extract_native_plan_backing_and_captured_origin_refusal_refund_with_recovery(){
 let(mut d,mut f,mut p)=fixture();p.begin_job();let handle=match p.runtime.execute("r",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap(){Value::Host(node)=>node,_=>panic!("native range")};
 let retained=p.state.retained;let mut bridge=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};let plan=bridge.range_extract_plan(handle).unwrap_or_else(|_|panic!("native extraction plan"));let bytes=plan.steps.capacity()*size_of::<crate::javascript::range_interface::ExtractStep>()+plan.slots.capacity()*size_of::<Option<u32>>();assert_eq!(plan.bytes,bytes);assert_eq!(bridge.state.retained,retained+bytes);drop(plan);bridge.finish_range_extract(bytes);assert_eq!(bridge.state.retained,retained);
 run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,foreignExtract=w.Range.prototype.extractContents;");let before=(p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);
 p.set_location(&crate::navigation::address("https://foreign.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();let error=p.runtime.execute("foreignExtract.call(r)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin"),"{error}");assert_eq!((p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);
 p.set_location(&crate::navigation::address("https://extract.test/source",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,"var result=foreignExtract.call(r);if(result.childNodes[1]!==middle||root.textContent!=='Ad'||!r.collapsed)throw 'native extraction origin recovery';");
}

#[test]
fn range_extract_late_copy_refusal_preserves_committed_data_and_move_prefixes(){
 let mut data_prefix=false;let mut moved_prefix=false;
 for budget in (0..16384).step_by(8){
  let(mut d,mut f,mut p)=fixture();p.begin_job();p.state.copy_work=budget;
  let result=p.runtime.execute("extract.call(r)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state});if result.is_ok(){continue;}
  p.begin_job();let status=p.runtime.execute("first.data==='A' && last.data==='End' && r.collapsed",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();if !matches!(status,Value::Bool(true)){continue;}
  let moved=p.runtime.execute("middle.parentNode!==root",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();let moved=matches!(moved,Value::Bool(true));
  if moved{if moved_prefix{continue;}moved_prefix=true;}else{if data_prefix{continue;}data_prefix=true;}
  run(&mut d,&mut f,&mut p,"var preservedParent=middle.parentNode;if(r.startContainer!==root||r.startOffset!==1)throw 'specified extraction collapse retained';r.setStart(first,1);r.setEnd(last,2);var recovered=extract.call(r);if(root.textContent!=='Ad'||!r.collapsed)throw 'late extraction refusal fresh recovery';if(preservedParent!==root&&middle.parentNode!==preservedParent)throw 'already moved native identity is not rolled back';");
  if data_prefix&&moved_prefix{break;}
 }
 assert!(data_prefix&&moved_prefix,"late refusal witnesses: data={data_prefix} moved={moved_prefix}");
}
