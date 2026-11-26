//! Native fixed child-document endpoint snapshots.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=mount></div><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://iterator.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}


#[test]
fn static_range_child_documents_elements_mixed_roots_and_invalid_fixed_offsets(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,cd=w.document,main=document.getElementById('mount');main.textContent='Main';
for(var node of [cd,cd.documentElement,cd.head,cd.body]){var fixed=new StaticRange({startContainer:node,startOffset:4294967295,endContainer:node,endOffset:0});if(fixed.startContainer!==node||fixed.endContainer!==node||fixed.startOffset!==4294967295||fixed.endOffset!==0||fixed.collapsed||!(fixed instanceof StaticRange)||fixed instanceof w.StaticRange)throw 'genuine child fixed invalid snapshot';}
var second=document.createElement('iframe');second.hidden=true;document.body.appendChild(second);var other=second.contentDocument,foreign=new w.StaticRange({startContainer:cd.body,startOffset:2,endContainer:other.head,endOffset:3});if(!(foreign instanceof w.StaticRange)||foreign instanceof StaticRange||foreign.startContainer!==cd.body||foreign.endContainer!==other.head||foreign.startContainer.ownerDocument!==cd||foreign.endContainer.ownerDocument!==other)throw 'constructor realm and independent child roots';
var inert=new Document(),text=inert.createTextNode('Saved'),mixed=new StaticRange({startContainer:cd.body,startOffset:9,endContainer:text,endOffset:1}),source=main.firstChild,displayed=new w.StaticRange({startContainer:source,startOffset:1,endContainer:cd,endOffset:999});source.insertData(0,'X');main.appendChild(document.createElement('b'));text.data='Changed';if(mixed.startContainer!==cd.body||mixed.endContainer!==text||mixed.endOffset!==1||displayed.startContainer!==source||displayed.startOffset!==1||displayed.endContainer!==cd||displayed.endOffset!==999)throw 'fixed native identities across source mutations';
"#);}
#[test]
fn static_range_child_ordered_dictionary_typed_guards_and_native_origin_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,cd=w.document,seen=[],init={get endContainer(){seen.push('ec');return cd.body;},get endOffset(){seen.push('eo');return {valueOf:function(){seen.push('en');return -1;}};},get startContainer(){seen.push('sc');return cd;},get startOffset(){seen.push('so');return {valueOf:function(){seen.push('sn');return 2;}};}},fixed=new w.StaticRange(init);if(seen.join(',')!=='ec,eo,en,sc,so,sn'||fixed.startContainer!==cd||fixed.startOffset!==2||fixed.endOffset!==4294967295)throw 'ordered native child dictionary';
var caught=0,abrupt={};try{new w.StaticRange({endContainer:cd,endOffset:{valueOf:function(){throw abrupt;}},startContainer:cd,startOffset:0});}catch(e){if(e===abrupt)caught++;}try{new w.StaticRange({endContainer:{nodeType:9},endOffset:0,startContainer:cd,startOffset:0});}catch(e){if(e instanceof w.TypeError)caught++;}try{new w.StaticRange({endContainer:document.createAttribute('x'),endOffset:0,startContainer:cd,startOffset:0});}catch(e){if(e instanceof w.DOMException&&e.name==='InvalidNodeTypeError')caught++;}
var live=document.createRange();live.setStart(cd.body,0);if(live.startContainer!==cd.body||live.endContainer!==cd.body||live.startOffset||live.endOffset||!live.collapsed||caught!==3)throw 'native fixed type guards and live child boundary';
var savedBody=cd.body,gets=0,query={get endContainer(){gets++;return savedBody;},get endOffset(){gets++;return 0;},get startContainer(){gets++;return cd;},get startOffset(){gets++;return 0;}};
"#);
 p.set_location(&crate::navigation::address("https://foreign.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();let before=p.state.ranges.len();let error=p.runtime.execute("new StaticRange(query);",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin"),"{error}");assert_eq!(p.state.ranges.len(),before);
 p.set_location(&crate::navigation::address("https://iterator.test/source",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,"if(gets!==4)throw 'author dictionary effects survive authority refusal';var recovered=new StaticRange({startContainer:cd,startOffset:0,endContainer:cd.body,endOffset:0});if(recovered.startContainer!==cd||fixed.endContainer!==cd.body)throw 'fresh child snapshot recovery';");
}
#[test]
fn static_range_child_native_storage_copy_work_deadline_refusal_and_recovery(){
 for profile in 0..4{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var cd=frame.contentDocument,node=cd.body,warm=new StaticRange({startContainer:node,startOffset:99,endContainer:document,endOffset:9});");
  p.begin_job();let Value::Host(node)=p.runtime.execute("node",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap() else{panic!("native child")};let fields=[("endContainer",Value::Host(DOCUMENT)),("endOffset",Value::Number(9.0)),("startContainer",Value::Host(node)),("startOffset",Value::Number(99.0))];let mut bridge=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};bridge.validate_static_range_fields(Some(&fields)).unwrap();
  while bridge.state.ranges.len()<bridge.state.ranges.capacity(){bridge.create_static_range(WINDOW,Some(&fields)).unwrap();}
  let before=(bridge.state.retained,bridge.state.ranges.len(),bridge.state.ranges.capacity(),bridge.doc.nodes.len(),bridge.state.dom_commits);let limit=bridge.state.retained_limit;match profile{0=>bridge.state.host_work=0,1=>bridge.state.copy_work=0,2=>bridge.state.retained_limit=bridge.state.retained,_=>{bridge.state.host_work=4096;bridge.state.execution_deadline=Some(Instant::now());}}
  assert!(bridge.create_static_range(WINDOW,Some(&fields)).is_err());assert_eq!((bridge.state.retained,bridge.state.ranges.len(),bridge.state.ranges.capacity(),bridge.doc.nodes.len(),bridge.state.dom_commits),before);bridge.state.retained_limit=limit;
  run(&mut d,&mut f,&mut p,"var recovered=new StaticRange({startContainer:node,startOffset:99,endContainer:document,endOffset:9});if(recovered.startContainer!==node||recovered.startOffset!==99||recovered.endContainer!==document||node.ownerDocument!==cd)throw 'child snapshot quota recovery';");
 }
}
