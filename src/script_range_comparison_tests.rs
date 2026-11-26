//! Native range query ordering and source-preserving comparison witnesses.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root><b id=a>A😀B</b><span id=b>Next</span></div></body></html>",&||Ok(())).unwrap();
 d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();
 assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);
 run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),text=a.firstChild,r=document.createRange();r.setStart(text,1);r.setEnd(text,3);");(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn range_comparison_native_node_guards_and_same_origin_method_realms(){
 let(mut d,mut f,mut p)=fixture();
 p.set_location(&crate::navigation::address("https://range-query.test/start",&crate::navigation::Location::home()).unwrap());
 run(&mut d,&mut f,&mut p,r#"
var attr=document.createAttribute('title'),inert=new Document(),node=inert.createElement('div'),seen=0,caught=0;
try{r.comparePoint(attr,{valueOf:function(){seen++;return 0;}});}catch(e){if(e instanceof DOMException&&e.name==='WrongDocumentError')caught++;}
if(r.isPointInRange(node,0)||r.intersectsNode(node))throw 'genuine inert nodes have independent native roots';
if(caught!==1||seen!==1||r.startOffset!==1||r.endOffset!==3)throw 'genuine native node root guards';
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentWindow;
var read=child.Function('range','node','return range.comparePoint(node,2)===0&&range.intersectsNode(node);');
if(!read(r,text))throw 'same origin caller';
try{child.Range.prototype.comparePoint.call(r,text,999);}catch(e){if(e instanceof child.DOMException&&!(e instanceof DOMException)&&e.name==='IndexSizeError')caught++;}
if(caught!==2)throw 'called method exception realm';
"#);
 p.set_location(&crate::navigation::address("https://range-query-other.test/changed",&crate::navigation::Location::home()).unwrap());
 let result=p.console_eval(&mut d,&mut f,"read(r,text);");
 assert_eq!(result.errors,1,"{}",p.report());assert!(p.report().contains("origin"),"{}",p.report());
 run(&mut d,&mut f,&mut p,"if(r.comparePoint(text,2)!==0||r.startOffset!==1||r.endOffset!==3)throw 'main caller recovery';");
}
#[test]
fn range_comparison_modes_points_exact_units_and_live_boundaries(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var s=document.createRange();s.setStart(text,0);s.setEnd(text,2);
if(r.compareBoundaryPoints(0,s)!==1||r.compareBoundaryPoints(1,s)!==1||r.compareBoundaryPoints(2,s)!==1||r.compareBoundaryPoints(3,s)!==-1)throw 'four mode endpoint pairs';
if(r.compareBoundaryPoints(65536,s)!==1||r.compareBoundaryPoints(-65533,s)!==-1)throw 'unsigned short mode';
if(r.comparePoint(text,0)!==-1||r.comparePoint(text,1)!==0||r.comparePoint(text,2)!==0||r.comparePoint(text,3)!==0||r.comparePoint(text,4)!==1)throw 'exact unit point ordering';
if(!r.isPointInRange(text,1)||!r.isPointInRange(text,2)||!r.isPointInRange(text,3)||r.isPointInRange(text,0)||r.isPointInRange(text,4))throw 'inclusive point boundaries';
if(r.startContainer!==text||r.startOffset!==1||r.endOffset!==3||text.nodeValue!=='A😀B')throw 'queries mutated source';
text.insertData(0,'X');if(r.startOffset!==2||r.endOffset!==4||r.comparePoint(text,1)!==-1||!r.isPointInRange(text,3))throw 'current live points';
"#);
}
#[test]
fn range_comparison_intersections_roots_and_strict_node_edges(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var detached=document.createElement('div');detached.appendChild(document.createTextNode('Other'));
if(!r.intersectsNode(a)||!r.intersectsNode(root)||!r.intersectsNode(document)||r.intersectsNode(b)||r.intersectsNode(detached)||r.isPointInRange(detached,999))throw 'roots and ancestor intersections';
r.selectNode(a);if(!r.intersectsNode(a)||r.intersectsNode(b))throw 'touching sibling edges';
r.setStart(root,1);r.collapse(true);if(r.intersectsNode(a)||r.intersectsNode(b)||!r.intersectsNode(root))throw 'collapsed strict edges';
r.selectNodeContents(detached);if(!r.intersectsNode(detached)||r.intersectsNode(root)||!r.isPointInRange(detached,0))throw 'same detached root';
"#);
}
#[test]
fn range_comparison_typed_errors_root_precedence_and_required_arguments(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var other=document.createElement('div'),s=document.createRange();s.selectNodeContents(other);var count=0;
function check(name,call){try{call();}catch(e){if(e.name===name){count++;return;}}throw name;}
check('WrongDocumentError',function(){r.compareBoundaryPoints(0,s);});
check('NotSupportedError',function(){r.compareBoundaryPoints(4,s);});
check('WrongDocumentError',function(){r.comparePoint(other,999);});
check('IndexSizeError',function(){r.comparePoint(text,5);});
check('IndexSizeError',function(){r.isPointInRange(text,-1);});
check('InvalidNodeTypeError',function(){r.comparePoint(document.doctype,0);});
check('InvalidNodeTypeError',function(){r.isPointInRange(document.doctype,0);});
check('TypeError',function(){r.compareBoundaryPoints(0,{});});
check('TypeError',function(){r.comparePoint(text);});
check('TypeError',function(){r.isPointInRange();});
check('TypeError',function(){r.intersectsNode();});
if(count!==11||r.startContainer!==text||r.startOffset!==1||r.endOffset!==3)throw 'error count or mutation';
"#);
}
#[test]
fn range_comparison_conversion_order_reentry_and_canonical_data_methods(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var seen=0,source=r.cloneRange(),offset={valueOf:function(){seen++;r.setStart(text,0);return 0;}};
if(r.comparePoint(text,offset)!==0||seen!==1||r.startOffset!==0)throw 'conversion reentry must precede snapshot';
var mode={valueOf:function(){seen++;return 4;}},caught=0;
try{r.compareBoundaryPoints(mode,{});}catch(e){if(e instanceof TypeError)caught++;}
if(seen!==2||caught!==1)throw 'source brand after mode conversion before algorithm';
try{Range.prototype.comparePoint.call({},text,{valueOf:function(){seen++;return 0;}});}catch(e){if(e instanceof TypeError)caught++;}
try{r.comparePoint({},{valueOf:function(){seen++;return 0;}});}catch(e){if(e instanceof TypeError)caught++;}
if(seen!==2||caught!==3)throw 'receiver and node brand before offset conversion';
var sentinel={};try{r.comparePoint(text,{valueOf:function(){throw sentinel;}});}catch(e){if(e===sentinel)caught++;}
var desc=Object.getOwnPropertyDescriptor(Range.prototype,'comparePoint'),saved=desc.value;
if(desc.get!==undefined||!desc.writable||!desc.configurable||!desc.enumerable||saved.name!=='comparePoint'||saved.length!==2||saved!==r.comparePoint||caught!==4)throw 'ordinary canonical data method';
delete Range.prototype.comparePoint;if(r.comparePoint!==undefined||saved.call(r,text,1)!==0)throw 'captured method after deletion';
"#);
}

#[test]
fn range_comparison_work_deadline_storage_and_fresh_recovery(){
 for profile in 0..4 {
  let(mut d,mut f,mut p)=fixture();p.begin_job();
  let Value::Host(receiver)=p.runtime.execute("r",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap() else {panic!("Range")};
  let record=p.state.ranges[0];
  let before=(p.state.retained,p.state.inert_documents.backing_bytes(),d.nodes.len(),p.state.dom_commits,p.state.ranges.len());
  let limit=p.state.retained_limit;
  match profile {0=>p.state.host_work=0,1=>p.state.host_work=3,2=>{p.state.host_work=4096;p.state.execution_deadline=Some(Instant::now());},_=>{p.state.retained_limit=p.state.retained;p.state.copy_work=0;}}
  let args=[Value::Host(record.start.node),Value::Number(2.0)];
  let mut bridge=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};
  let result=bridge.validate_range_call(receiver,crate::javascript::range_interface::COMPARE_POINT,&args).map_err(|_|"preflight".to_string()).and_then(|()|bridge.range_call(receiver,crate::javascript::range_interface::COMPARE_POINT,&args));
  if profile==3 {assert!(matches!(result,Ok(Value::Number(0.0))));}else{assert!(result.is_err());}
  assert_eq!((p.state.retained,p.state.inert_documents.backing_bytes(),d.nodes.len(),p.state.dom_commits,p.state.ranges.len()),before);
  assert_eq!((p.state.ranges[0].start,p.state.ranges[0].end),(record.start,record.end));
  p.state.retained_limit=limit;
  run(&mut d,&mut f,&mut p,"if(r.comparePoint(text,2)!==0||!r.isPointInRange(text,2)||text.nodeValue!=='A😀B')throw 'query fresh recovery';");
 }
}
