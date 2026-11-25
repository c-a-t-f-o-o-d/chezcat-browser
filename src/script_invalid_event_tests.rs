//! Native interactive required-field snapshots, events and checked recovery.
use super::*;
fn fixture(html:&str,source:&str)->(Document,Forms,Page){
 let mut d=crate::html::parse(html);d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://invalid.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);let outcome=p.start(&mut d,&mut f,&[Script{node:0,label:"Native required-field witness".into(),source:source.into()}]);assert_eq!(outcome.errors,0,"{}",p.report());(d,f,p)
}
fn id(d:&Document,name:&str)->usize{(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some(name)).unwrap()}
fn eval(d:&mut Document,f:&mut Forms,p:&mut Page,source:&str){let out=p.console_eval(d,f,source);assert_eq!(out.errors,0,"{}",p.report());}
const HTML:&str="<html><body><form id=owner action=https://invalid.test/submit><input id=first name=a required><input id=second name=b required><button id=send name=go>Send</button></form><input id=external name=c required form=owner></body></html>";
#[test]
fn native_invalid_handler_property_identity_removal_and_inline_return_false(){
 let(mut d,mut f,mut p)=fixture("<html><body><form id=owner><input id=field name=q required oninvalid=\"document.title+='inline';return false\"></form></body></html>","var field=document.getElementById('field'),original=field.oninvalid;if(typeof original!=='function')throw 'inline callable';var replacement=function(){document.title+='replacement';return false;};field.oninvalid=replacement;if(field.oninvalid!==replacement)throw 'handler identity';field.oninvalid=null;if(field.oninvalid!==null)throw 'handler removal';field.oninvalid=original;");
 let owner=id(&d,"owner");p.begin_job();let(out,invalid,unhandled)=p.validate_native_submission(&mut d,&mut f,owner,None).unwrap();assert!(invalid&&unhandled.is_none());assert_eq!(out.errors,0,"{}",p.report());assert_eq!(d.title,"inline");
}
#[test]
fn native_invalid_tree_order_capture_target_trust_and_nonbubbling(){
 let(mut d,mut f,mut p)=fixture(HTML,"var owner=document.getElementById('owner'),a=document.getElementById('first'),calls='',phases='';owner.appendChild(a);document.addEventListener('invalid',function(e){if(!(e instanceof Event)||!e.isTrusted||!e.cancelable||e.bubbles||e.composed||e.eventPhase!==1)throw 'metadata';calls+=e.target.id+',';e.preventDefault();},true);owner.addEventListener('invalid',function(){throw 'bubble';});a.oninvalid=function(e){if(e.target!==a||e.currentTarget!==a||e.eventPhase!==2||!e.defaultPrevented)throw 'target';phases+='a';};");
 let owner=id(&d,"owner");let send=id(&d,"send");p.begin_job();let(out,invalid,unhandled)=p.validate_native_submission(&mut d,&mut f,owner,Some(send)).unwrap();assert!(invalid&&unhandled.is_none());assert_eq!(out.errors,0,"{}",p.report());assert!(p.state.event.is_none());
 eval(&mut d,&mut f,&mut p,"if(calls!=='second,first,external,'||phases!=='a')throw 'tree order '+calls;");
}
#[test]
fn native_invalid_snapshot_survives_repair_removal_and_new_required_fields(){
 let(mut d,mut f,mut p)=fixture(HTML,"var a=document.getElementById('first'),b=document.getElementById('second'),c=document.getElementById('external'),calls='';a.oninvalid=function(e){calls+='a';a.value='fixed';b.value='fixed';b.remove();c.required=false;var fresh=document.createElement('input');fresh.required=true;document.getElementById('owner').appendChild(fresh);e.preventDefault();};b.oninvalid=function(e){if(e.target!==b||b.isConnected||!e.isTrusted)throw 'saved detached candidate';calls+='b';e.preventDefault();};c.oninvalid=function(e){calls+='c';e.preventDefault();};");
 let owner=id(&d,"owner");p.begin_job();let(out,invalid,unhandled)=p.validate_native_submission(&mut d,&mut f,owner,None).unwrap();assert!(invalid&&unhandled.is_none());assert_eq!(out.errors,0,"{}",p.report());eval(&mut d,&mut f,&mut p,"if(calls!=='abc')throw 'snapshot '+calls;");
 p.begin_job();let(_,invalid,unhandled)=p.validate_native_submission(&mut d,&mut f,owner,None).unwrap();assert!(invalid&&unhandled.is_some());
}
#[test]
fn native_invalid_shared_required_candidates_and_validation_overrides(){
 let html="<html><body><form id=owner><input name=disabled required disabled><input name=readonly required readonly><input type=hidden required><fieldset disabled><input required></fieldset><input id=check type=checkbox required><input id=radio type=radio name=r required><input id=peer type=radio name=r><select id=select name=s required><option value=''>Choose</option><option value=x>X</select><textarea id=text name=t required></textarea><button id=send formnovalidate>Send</button></form></body></html>";
 let(mut d,mut f,mut p)=fixture(html,"");let owner=id(&d,"owner");let snapshot=f.validation_snapshot(&d,owner,None,usize::MAX).unwrap();assert_eq!(snapshot.nodes,[id(&d,"check"),id(&d,"radio"),id(&d,"peer"),id(&d,"select"),id(&d,"text")]);assert!(f.validate_submission(&d,owner,None).is_err());let send=id(&d,"send");p.begin_job();let(_,invalid,_)=p.validate_native_submission(&mut d,&mut f,owner,Some(send)).unwrap();assert!(!invalid);
 eval(&mut d,&mut f,&mut p,"var form=document.getElementById('owner');form.noValidate=true;");p.begin_job();assert!(!p.validate_native_submission(&mut d,&mut f,owner,None).unwrap().1);
 eval(&mut d,&mut f,&mut p,"form.noValidate=false;document.getElementById('check').checked=true;document.getElementById('peer').checked=true;document.getElementById('select').value='x';document.getElementById('text').value='done';");assert!(f.validate_submission(&d,owner,None).is_ok());assert!(f.validation_snapshot(&d,owner,None,usize::MAX).unwrap().nodes.is_empty());
}
#[test]
fn native_invalid_snapshot_refusals_leave_no_event_or_storage_then_recover(){
 for mode in 0..5{
  let(mut d,mut f,mut p)=fixture(HTML,"var count=0;document.addEventListener('invalid',function(e){count++;e.preventDefault();},true);");let owner=id(&d,"owner");p.begin_job();let retained=p.state.retained;let limit=p.state.retained_limit;let next=p.state.next_event;let peak=f.validation_snapshot(&d,owner,None,usize::MAX).unwrap().peak_bytes;
  match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=retained,3=>p.state.execution_deadline=Some(Instant::now()),_=>p.state.copy_work=peak};
  assert!(p.validate_native_submission(&mut d,&mut f,owner,None).is_err(),"mode {mode}");assert!(p.state.event.is_none());assert_eq!(p.state.retained,retained,"mode {mode}");if mode<4{assert_eq!(p.state.next_event,next);}
  p.state.retained_limit=limit;p.begin_job();let(out,invalid,unhandled)=p.validate_native_submission(&mut d,&mut f,owner,None).unwrap();assert!(invalid&&unhandled.is_none());assert_eq!(out.errors,0,"{}",p.report());eval(&mut d,&mut f,&mut p,"if(count!==3)throw 'fresh recovery '+count;");
 }
}
#[test]
fn native_invalid_listener_exception_continues_and_reports_unhandled(){
 let(mut d,mut f,mut p)=fixture(HTML,"var calls='';document.getElementById('first').oninvalid=function(){calls+='a';throw 'author exception';};document.getElementById('second').oninvalid=function(e){calls+='b';e.preventDefault();};document.getElementById('external').oninvalid=function(e){calls+='c';e.preventDefault();};");let owner=id(&d,"owner");let first=id(&d,"first");p.begin_job();let(out,invalid,unhandled)=p.validate_native_submission(&mut d,&mut f,owner,None).unwrap();assert!(invalid);assert_eq!(unhandled,Some(first));assert_eq!(out.errors,1);eval(&mut d,&mut f,&mut p,"if(calls!=='abc')throw 'continue '+calls;");
}
#[test]
fn native_invalid_fatal_listener_preserves_committed_value_and_refunds_snapshot(){
 let(mut d,mut f,mut p)=fixture(HTML,"var calls=0;document.getElementById('first').oninvalid=function(){calls++;document.getElementById('first').value='committed';for(var i=0;i<10000;i++){document.getElementById('first');}};");let owner=id(&d,"owner");let first=id(&d,"first");p.begin_job();p.state.host_work=d.nodes.len()*2+f.controls.len()*f.controls.len()+1000;
 assert!(p.validate_native_submission(&mut d,&mut f,owner,None).is_err());assert!(p.state.event.is_none());assert_eq!(f.controls[&first].value.text(),"committed");assert!(p.runtime.invocation_failed_fatally());
 p.begin_job();eval(&mut d,&mut f,&mut p,"if(calls!==1)throw 'committed listener';document.getElementById('first').oninvalid=null;document.getElementById('second').value='fixed';document.getElementById('external').value='fixed';");
 let retained=p.state.retained;for _ in 0..3{p.begin_job();let(out,invalid,unhandled)=p.validate_native_submission(&mut d,&mut f,owner,None).unwrap();assert!(!invalid&&unhandled.is_none());assert_eq!(out.errors,0);assert_eq!(p.state.retained,retained);}
}
#[test]
fn native_invalid_completed_validation_serializes_later_submit_listener_changes(){
 let(mut d,mut f,mut p)=fixture(HTML,"document.getElementById('first').value='a';document.getElementById('second').value='b';document.getElementById('external').value='c';document.getElementById('owner').onsubmit=function(e){if(!(e instanceof SubmitEvent))throw 'submit family';document.getElementById('first').value='';};");let owner=id(&d,"owner");let send=id(&d,"send");p.begin_job();assert!(!p.validate_native_submission(&mut d,&mut f,owner,Some(send)).unwrap().1);let out=p.dispatch_submit(&mut d,&mut f,owner,Some(send));assert!(!out.prevented);assert_eq!(out.errors,0);let location=crate::navigation::address("https://invalid.test/source",&crate::navigation::Location::home()).unwrap();let submission=f.validated_submit_request(&d,owner,Some(send),&location,&location).unwrap();assert!(submission.location.display().contains("a=&b=b"));assert!(f.validate_submission(&d,owner,Some(send)).is_err());
}
