//! Native surrounding preserves specified validation and observable mutation order.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=root>A😀B<span>Middle</span>End</div><section id=other><b>Old</b></section><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://surround.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);run(&mut d,&mut f,&mut p,"var root=document.getElementById('root'),first=root.firstChild,middle=root.childNodes[1],last=root.lastChild,other=document.getElementById('other'),wrapper=other.firstChild,r=document.createRange();r.setStart(root,1);r.setEnd(root,2);var surround=r.surroundContents;");(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn range_surround_real_identity_wrapper_replacement_selection_and_exact_text(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var old=wrapper.firstChild,state={},calls=0;middle.author=state;middle.addEventListener('check',function(){calls++;});if(r.surroundContents(wrapper)!==undefined||root.childNodes[1]!==wrapper||wrapper.firstChild!==middle||wrapper.childNodes.length!==1||old.parentNode!==null||other.firstChild!==null||middle.author!==state||r.startContainer!==root||r.startOffset!==1||r.endContainer!==root||r.endOffset!==2||r.collapsed)throw 'native full identity wrapper replacement and selection';middle.dispatchEvent(new Event('check'));if(calls!==1)throw 'selected identity retains listener';
var s=new Range(),w=document.createElement('i');s.setStart(first,1);s.setEnd(first,2);s.surroundContents(w);if(first.data!=='A'||w.firstChild.data!=='\uD83D'||w.nextSibling.data!=='\uDE00B'||w.parentNode!==root||s.startContainer!==root||s.startOffset!==1||s.endOffset!==2)throw 'exact surrogate extraction insertion selection';
var empty=new Range(),em=document.createElement('em');empty.setStart(last,1);empty.collapse(true);empty.surroundContents(em);if(em.firstChild!==null||em.previousSibling.data!=='E'||em.nextSibling.data!=='nd'||empty.collapsed||empty.startContainer!==root||empty.endOffset!==empty.startOffset+1)throw 'collapsed surrounds empty native fragment';
"#);}
#[test]
fn range_surround_partial_nontext_and_invalid_parent_kind_error_precedence(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
r.setStart(first,1);r.setEnd(middle.firstChild,2);var before=root.textContent,caught=false;try{r.surroundContents(document);}catch(e){caught=e instanceof DOMException&&e.name==='InvalidStateError';}if(!caught||root.textContent!==before||wrapper.textContent!=='Old'||r.startContainer!==first||r.startOffset!==1||r.endContainer!==middle.firstChild)throw 'partial non-Text precedence before parent kind';
r.setStart(root,1);r.setEnd(root,2);var count=0;for(var bad of [document,document.doctype,document.createDocumentFragment()]){try{r.surroundContents(bad);}catch(e){if(e instanceof DOMException&&e.name==='InvalidNodeTypeError')count++;}}if(count!==3||root.textContent!==before||middle.parentNode!==root||r.startOffset!==1||r.endOffset!==2)throw 'invalid parent kinds precede extraction';
var comment=document.createComment('abcdef');root.appendChild(comment);r.setStart(comment,1);r.setEnd(root,root.childNodes.length);caught=false;try{r.surroundContents(wrapper);}catch(e){caught=e.name==='InvalidStateError';}if(!caught||comment.data!=='abcdef'||wrapper.textContent!=='Old')throw 'partially contained Comment is non-Text';
"#);}
#[test]
fn range_surround_native_observation_live_fixed_iterator_and_late_character_parent_error(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var old=wrapper.firstChild,watch=new MutationObserver(function(){});watch.observe(document.body,{childList:true,subtree:true});var live=r.cloneRange(),fixed=new StaticRange({startContainer:root,startOffset:1,endContainer:root,endOffset:2}),iterator=document.createNodeIterator(root);while(iterator.nextNode()!==middle){}surround.call(r,wrapper);var records=watch.takeRecords();if(records.length!==5||records[0].target!==root||records[0].removedNodes[0]!==middle||records[1].target!==wrapper||records[1].removedNodes[0]!==old||records[2].target!==other||records[2].removedNodes[0]!==wrapper||records[3].target!==root||records[3].addedNodes[0]!==wrapper||records[4].target!==wrapper||records[4].addedNodes[0]!==middle)throw 'extract clear remove insert append observation order';if(!live.collapsed||live.startContainer!==root||live.startOffset!==1||fixed.endOffset!==2||iterator.referenceNode===middle||r.startContainer!==root||r.startOffset!==1||r.endOffset!==2)throw 'native live fixed iterator and final select';
var leaf=document.createTextNode('Parent'),s=new Range();s.setStart(last,0);s.setEnd(last,1);var caught=false;try{s.surroundContents(leaf);}catch(e){caught=e instanceof DOMException&&e.name==='HierarchyRequestError';}if(!caught||last.data!==''||leaf.parentNode!==root||leaf.data!=='Parent'||!s.toString().includes('Parent'))throw 'CharacterData parent error retains extraction and insertion prefix';
"#);}
#[test]
fn range_surround_native_brands_borrowed_realm_and_initial_refusal_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,method=w.Range.prototype.surroundContents,caught=0;for(var fake of [{},document,new StaticRange({startContainer:root,startOffset:0,endContainer:root,endOffset:0})]){try{method.call(fake,wrapper);}catch(e){if(e instanceof w.TypeError)caught++;}}try{method.call(r,{nodeType:1});}catch(e){if(e instanceof w.TypeError)caught++;}if(caught!==4)throw 'native surround brand realm';method.call(r,wrapper);if(wrapper.firstChild!==middle||wrapper.parentNode!==root||r.startOffset!==1||r.endOffset!==2)throw 'borrowed native surround';");
 for profile in 0..4{let(mut d,mut f,mut p)=fixture();p.begin_job();let before=(p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);let limit=p.state.retained_limit;match profile{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>{p.state.host_work=4096;p.state.execution_deadline=Some(Instant::now());}}assert!(p.runtime.execute("surround.call(r,wrapper)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).is_err());assert_eq!((p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);p.state.retained_limit=limit;run(&mut d,&mut f,&mut p,"if(middle.parentNode!==root||wrapper.parentNode!==other||wrapper.textContent!=='Old')throw 'initial surround refusal preserves source';surround.call(r,wrapper);if(wrapper.firstChild!==middle||wrapper.parentNode!==root||r.endOffset!==2)throw 'fresh native surround recovery';");}
}

#[test]
fn range_surround_captured_origin_loss_refuses_before_native_effects_and_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var foreignSurround=frame.contentWindow.Range.prototype.surroundContents;");let before=(p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end);
 p.set_location(&crate::navigation::address("https://foreign.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();let error=p.runtime.execute("foreignSurround.call(r,wrapper)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin"),"{error}");assert_eq!((p.state.retained,p.state.dom_commits,p.state.ranges[0].start,p.state.ranges[0].end),before);
 p.set_location(&crate::navigation::address("https://surround.test/source",&crate::navigation::Location::home()).unwrap());run(&mut d,&mut f,&mut p,"if(middle.parentNode!==root||wrapper.parentNode!==other||wrapper.textContent!=='Old')throw 'origin refusal before extraction';foreignSurround.call(r,wrapper);if(wrapper.firstChild!==middle||wrapper.parentNode!==root||r.endOffset!==2)throw 'captured origin recovery';");
}

#[test]
fn range_surround_late_copy_refusal_preserves_extraction_clear_and_insertion_prefixes(){
 let mut seen=[false;3];
 for budget in (0..32768).step_by(8){
  let(mut d,mut f,mut p)=fixture();p.begin_job();p.state.copy_work=budget;let result=p.runtime.execute("surround.call(r,wrapper)",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state});if result.is_ok(){continue;}
  p.begin_job();let phase=p.runtime.execute("middle.parentNode!==root&&middle.parentNode!==wrapper?(wrapper.parentNode===root?2:(wrapper.firstChild===null?1:0)):-1",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap();let Value::Number(phase)=phase else{panic!("native phase")};if !(0.0..3.0).contains(&phase){continue;}let phase=phase as usize;if seen[phase]{continue;}seen[phase]=true;
  run(&mut d,&mut f,&mut p,"var savedParent=wrapper.parentNode,savedChild=wrapper.firstChild,recovered=document.createElement('em');r.selectNode(middle);surround.call(r,recovered);if(recovered.firstChild!==middle||r.startContainer!==recovered.parentNode||r.endOffset!==r.startOffset+1||wrapper.parentNode!==savedParent||wrapper.firstChild!==savedChild)throw 'completed surrounding prefixes persist through fresh recovery';");if seen.iter().all(|&v|v){break;}
 }
 assert!(seen.iter().all(|&v|v),"late surrounding prefixes: {seen:?}");
}
