//! Static native boundary snapshots and ordered dictionary conversion.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root><b id=a>A😀B</b><span id=b>Next</span></div></body></html>",&||Ok(())).unwrap();
 d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);
 run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),text=a.firstChild;");(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn static_range_canonical_hierarchy_brands_and_permitted_invalid_snapshots(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var s=new StaticRange({startContainer:text,startOffset:3,endContainer:text,endOffset:1});
if(!(s instanceof StaticRange)||!(s instanceof NodeRange)||!(s instanceof AbstractRange)||s instanceof Range||Object.getPrototypeOf(StaticRange.prototype)!==NodeRange.prototype||Object.prototype.toString.call(s)!=='[object StaticRange]'||StaticRange.length!==1)throw 'canonical hierarchy';
if(s.startContainer!==text||s.endContainer!==text||s.startOffset!==3||s.endOffset!==1||s.collapsed||s.setStart!==undefined)throw 'reversed snapshot';
var other=document.createElement('div'),split=new StaticRange({startContainer:text,startOffset:-1,endContainer:other,endOffset:4294967297});
if(split.startContainer!==text||split.endContainer!==other||split.startOffset!==4294967295||split.endOffset!==1||split.collapsed)throw 'different roots and converted offsets';
var empty=new StaticRange({startContainer:text,startOffset:999,endContainer:text,endOffset:999});if(!empty.collapsed||empty.startOffset!==999)throw 'invalid but collapsed';
var caught=0,seen=0;try{Range.prototype.setStart.call(s,text,{valueOf:function(){seen++;return 0;}});}catch(e){if(e instanceof TypeError)caught++;}
try{Range.prototype.comparePoint.call(s,text,{valueOf:function(){seen++;return 0;}});}catch(e){if(e instanceof TypeError)caught++;}
try{Range.prototype.cloneRange.call(s);}catch(e){if(e instanceof TypeError)caught++;}
try{Object.getOwnPropertyDescriptor(AbstractRange.prototype,'startOffset').get.call({});}catch(e){if(e instanceof TypeError)caught++;}
if(caught!==4||seen||s.startOffset!==3)throw 'distinct live range brands';
var method=Object.getOwnPropertyDescriptor(Range.prototype,'cloneRange');
if(method.get!==undefined||!method.enumerable||!method.writable||!method.configurable||method.value.length!==0||method.value.name!=='cloneRange')throw 'canonical deferred live method';
var live=document.createRange();live.setStart(text,1);live.setEnd(text,3);var saved=method.value;
delete Range.prototype.cloneRange;if(live.cloneRange!==undefined||saved.call(live).endOffset!==3)throw 'saved deferred live method';
"#);
}
#[test]
fn static_range_exact_snapshots_ignore_real_character_and_tree_mutations(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var fixed=new StaticRange({startContainer:text,startOffset:2,endContainer:text,endOffset:3}),live=document.createRange();live.setStart(text,2);live.setEnd(text,3);
text.insertData(0,'X');if(live.startOffset!==3||fixed.startOffset!==2||fixed.endOffset!==3||fixed.startContainer!==text)throw 'splice updates fixed range';
var tail=text.splitText(2);a.normalize();if(fixed.startContainer!==text||fixed.endContainer!==text||fixed.startOffset!==2||fixed.endOffset!==3)throw 'split normalize updates fixed range';
root.removeChild(a);if(fixed.startContainer!==text||fixed.endContainer!==text||fixed.startOffset!==2||fixed.endOffset!==3||live.startContainer!==root)throw 'removal updates fixed range';
root.appendChild(a);a.textContent='Changed';if(fixed.startContainer!==text||fixed.endContainer!==text||fixed.startOffset!==2||fixed.endOffset!==3||text.parentNode!==null)throw 'replacement updates saved identities';
"#);
}
#[test]
fn static_range_required_dictionary_order_conversions_and_original_failures(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var log='',init={};Object.defineProperty(init,'endContainer',{get:function(){log+='e';return text;}});
Object.defineProperty(init,'endOffset',{get:function(){log+='o';return {valueOf:function(){log+='O';text.insertData(0,'X');return 3;}};}});
Object.defineProperty(init,'startContainer',{get:function(){log+='s';return text;}});
Object.defineProperty(init,'startOffset',{get:function(){log+='i';return {valueOf:function(){log+='I';return 1;}};}});
var s=new StaticRange(init);if(log!=='eoOsiI'||s.startOffset!==1||s.endOffset!==3||text.nodeValue!=='XA😀B')throw 'ordered captured members';
var sentinel={},caught=0;try{new StaticRange({get endContainer(){throw sentinel;}});}catch(e){if(e===sentinel)caught++;}
try{new StaticRange({endContainer:text,get endOffset(){return {valueOf:function(){throw sentinel;}};},get startContainer(){throw 'late getter';}});}catch(e){if(e===sentinel)caught++;}
var seen=0;try{new StaticRange({get endOffset(){seen++;return 0;}});}catch(e){if(e instanceof TypeError)caught++;}
try{new StaticRange({endContainer:{},get endOffset(){seen++;return 0;}});}catch(e){if(e instanceof TypeError)caught++;}
if(caught!==4||seen)throw 'required member and brand ordering';
var inherited=Object.create({endContainer:text,endOffset:Infinity,startContainer:text,startOffset:NaN});if(!new StaticRange(inherited).collapsed)throw 'inherited dictionary and numeric defaults';
"#);
}
#[test]
fn static_range_typed_invalid_node_errors_after_dictionary_conversion(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var attr=document.createAttribute('title'),caught=0;
for(var invalid of [attr,document.doctype]){var seen=0,init={endContainer:invalid,endOffset:0,startContainer:text,get startOffset(){seen++;return 0;}};try{new StaticRange(init);}catch(e){if(e instanceof DOMException&&e.name==='InvalidNodeTypeError'&&seen===1)caught++;}}
for(var invalid of [undefined,null,17,'x']){try{new StaticRange(invalid);}catch(e){if(e instanceof TypeError)caught++;}}
try{StaticRange({endContainer:text,endOffset:0,startContainer:text,startOffset:0});}catch(e){if(e instanceof TypeError)caught++;}
try{new StaticRange();}catch(e){if(e instanceof TypeError)caught++;}
if(caught!==8)throw 'constructor errors';
var fresh=new StaticRange({endContainer:text,endOffset:3,startContainer:text,startOffset:1});if(fresh.startContainer!==text||fresh.endOffset!==3)throw 'fresh recovery';
"#);
}

#[test]
fn static_range_native_storage_refusal_cancellation_and_exact_recovery(){
 for profile in 0..5 {
  let(mut d,mut f,mut p)=fixture();p.begin_job();
  let Value::Host(text)=p.runtime.execute("text",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap() else {panic!("Text")};
  let fields=[("endContainer",Value::Host(text)),("endOffset",Value::Number(3.0)),("startContainer",Value::Host(text)),("startOffset",Value::Number(1.0))];
  let mut bridge=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state};
  bridge.create_static_range(WINDOW,Some(&fields)).unwrap();
  while bridge.state.ranges.len()<bridge.state.ranges.capacity(){bridge.create_static_range(WINDOW,Some(&fields)).unwrap();}
  let before=(bridge.state.retained,bridge.state.ranges.len(),bridge.state.ranges.capacity(),bridge.doc.nodes.len(),bridge.state.dom_commits);
  let limit=bridge.state.retained_limit;
  match profile {0=>bridge.state.host_work=0,1=>bridge.state.copy_work=0,2=>bridge.state.retained_limit=bridge.state.retained,3=>{bridge.state.host_work=4096;bridge.state.execution_deadline=Some(Instant::now());},_=>{}}
  let result=bridge.create_static_range(WINDOW,Some(&fields));
  if profile==4 {
   let Value::Host(handle)=result.unwrap() else {panic!("native fixed range")};
   let record=bridge.range_record(handle).unwrap();assert!(record.fixed);assert_eq!((record.start.offset,record.end.offset),(1,3));
   assert_eq!(bridge.state.retained-before.0,(bridge.state.ranges.capacity()-before.2)*size_of::<script_ranges::Record>());
  } else {
   assert!(result.is_err());assert_eq!((bridge.state.retained,bridge.state.ranges.len(),bridge.state.ranges.capacity(),bridge.doc.nodes.len(),bridge.state.dom_commits),before);
  }
  bridge.state.retained_limit=limit;
  run(&mut d,&mut f,&mut p,"var recovered=new StaticRange({startContainer:text,startOffset:1,endContainer:text,endOffset:3});if(recovered.startContainer!==text||recovered.endOffset!==3||text.nodeValue!=='A😀B')throw 'fixed range recovery';");
 }
}
#[test]
fn static_range_native_owner_realms_and_captured_dictionary_node_authority(){
 let(mut d,mut f,mut p)=fixture();p.set_location(&crate::navigation::address("https://static-range.test/start",&crate::navigation::Location::home()).unwrap());
 run(&mut d,&mut f,&mut p,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentWindow,foreignDocument=frame.contentDocument;
var owned=new child.StaticRange({startContainer:text,startOffset:1,endContainer:text,endOffset:3});
if(!(owned instanceof child.StaticRange)||owned instanceof StaticRange||owned.startContainer!==text||owned.endOffset!==3)throw 'constructor owner realm';
var caught=0;try{new child.StaticRange({startContainer:text,startOffset:0,endContainer:document.doctype,endOffset:0});}catch(e){if(e instanceof child.DOMException&&!(e instanceof DOMException)&&e.name==='InvalidNodeTypeError')caught++;}
try{new StaticRange({startContainer:foreignDocument,startOffset:0,endContainer:text,endOffset:0});}catch(e){if(e.name==='NotSupportedError')caught++;}
if(caught!==2)throw 'method realm and genuine unsupported endpoint';
var getters=0,query={get endContainer(){getters++;return text;},get endOffset(){getters++;return 0;},get startContainer(){getters++;return foreignDocument;},get startOffset(){getters++;return 0;}};
"#);
 p.set_location(&crate::navigation::address("https://static-range-other.test/changed",&crate::navigation::Location::home()).unwrap());
 let before=p.state.ranges.len();let result=p.console_eval(&mut d,&mut f,"new StaticRange(query);");
 assert_eq!(result.errors,1,"{}",p.report());assert!(p.report().contains("origin"),"{}",p.report());assert_eq!(p.state.ranges.len(),before);
 run(&mut d,&mut f,&mut p,"if(getters!==4)throw 'dictionary effects before native access';var recovered=new StaticRange({startContainer:text,startOffset:0,endContainer:text,endOffset:0});if(!recovered.collapsed)throw 'owner recovery';");
}
