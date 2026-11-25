//! Synchronous FORM.reset integrates native OUTPUT child transactions.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<html><body><form id=owner><output id=first>First</output><input id=edit value=Seed><output id=second>Second</output></form><output id=external form=owner>External</output><form id=other><output id=unrelated>Other</output></form><div id=mount></div></body></html>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://form-reset-output.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn form_reset_output_synchronous_real_children_observers_and_native_editors(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,r#"var form=document.getElementById('owner'),first=document.getElementById('first'),second=document.getElementById('second'),external=document.getElementById('external'),unrelated=document.getElementById('unrelated'),edit=document.getElementById('edit');first.value='dirty';second.value='dirty';external.value='dirty';unrelated.value='kept';edit.value='dirty';var old=first.firstChild,list=first.childNodes,range=new Range(),watch=new MutationObserver(function(){});range.selectNodeContents(old);watch.observe(document.body,{childList:true,subtree:true});form.reset();var records=watch.takeRecords();if(first.value!=='First'||first.defaultValue!=='First'||second.value!=='Second'||external.value!=='External'||unrelated.value!=='kept'||edit.value!=='Seed'||list!==first.childNodes||old.parentNode!==null||list.length!==1||range.startContainer!==first||!range.collapsed||records.length!==3||records[0].target!==first||records[1].target!==second||records[2].target!==external||records[0].removedNodes[0]!==old||records[0].addedNodes[0]!==first.firstChild)throw 'synchronous reset transactions';first.defaultValue='next';if(first.value!=='next')throw 'reset cleared default override';"#);
}
#[test]
fn form_reset_output_selection_follows_event_owner_and_default_changes(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,r#"var form=document.getElementById('owner'),first=document.getElementById('first'),second=document.getElementById('second'),external=document.getElementById('external'),other=document.getElementById('other'),edit=document.getElementById('edit');first.value='dirty';second.value='dirty second';external.value='dirty external';edit.value='dirty';form.onreset=function(e){if(!e.isTrusted||first.value!=='dirty'||edit.value!=='dirty')throw 'before reset';first.defaultValue='A\ud800z';other.appendChild(second);external.setAttribute('form','other');};form.reset();if(first.value!=='A\ud800z'||second.value!=='dirty second'||external.value!=='dirty external'||edit.value!=='Seed')throw 'current owner defaults';"#);
}

#[test]
fn form_reset_output_cancel_recursion_and_observers_run_after_method_returns(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,"var form=document.getElementById('owner'),first=document.getElementById('first'),log='',cancel=true,calls=0;first.value='dirty';var watch=new MutationObserver(function(records){if(records.length!==3)throw 'ordered batch';log+='m';});watch.observe(document.body,{childList:true,subtree:true});form.onreset=function(e){calls++;log+='r';form.reset();if(cancel)e.preventDefault();};form.reset();if(calls!==1||log!=='r'||first.value!=='dirty'||watch.takeRecords().length)throw 'canceled outputs';cancel=false;log='';form.reset();log+='a';if(calls!==2||first.value!=='First'||log!=='ra')throw 'synchronous output before jobs';");
 p.microtask_checkpoint(&mut d,&mut f);run(&mut d,&mut f,&mut p,"if(log!=='ram')throw 'observer checkpoint order';");assert!(p.state.form_reset_pending.is_none());
}
#[test]
fn form_reset_output_detached_native_form_and_nested_output_current_owner(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,r#"var detached=document.createElement('form'),output=document.createElement('output');detached.appendChild(output);output.textContent='A\ud800z';output.value='dirty';detached.reset();if(output.value!=='A\ud800z'||output.defaultValue!==output.value||output.form!==detached||detached.isConnected)throw 'detached native reset';
 var form=document.getElementById('owner'),first=document.getElementById('first'),nested=document.createElement('output');first.value='outer current';first.appendChild(nested);nested.defaultValue='nested default';nested.value='nested current';form.reset();if(nested.value!=='nested current'||nested.form!==null)throw 'detached nested output reset';"#);
}
#[test]
fn form_reset_output_pending_preparation_refusal_and_unbudgeted_cleanup(){
 for mode in 0..5{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var form=document.getElementById('owner'),first=document.getElementById('first');first.value='dirty';document.getElementById('edit').value='dirty';");
  let Value::Host(form)=p.runtime.execute("form;",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap()else{panic!("form")};
  let before=format!("{:?}",d.nodes);let controls=f.controls.iter().map(|(&n,c)|(n,c.value.text(),c.dirty_value)).collect::<Vec<_>>();let retained=p.state.retained;let limit=p.state.retained_limit;
  p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,3=>p.state.retained_limit=p.state.retained+1,_=>p.state.execution_deadline=Some(Instant::now())};
  assert!(Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.prepare_native_form_reset(form).is_err(),"mode {mode}");
  Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.finish_native_form_reset(form);
  assert!(p.state.form_reset_pending.is_none());assert_eq!(p.state.retained,retained);assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(f.controls.iter().map(|(&n,c)|(n,c.value.text(),c.dirty_value)).collect::<Vec<_>>(),controls);
  p.state.retained_limit=limit;p.state.execution_deadline=None;p.begin_job();
  Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.prepare_native_form_reset(form).unwrap();assert!(p.state.retained>retained);
  p.state.host_work=0;p.state.copy_work=0;p.state.execution_deadline=Some(Instant::now());
  Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.finish_native_form_reset(form);assert!(p.state.form_reset_pending.is_none());assert_eq!(p.state.retained,retained);
  p.state.execution_deadline=None;p.begin_job();run(&mut d,&mut f,&mut p,"form.reset();if(first.value!=='First'||document.getElementById('edit').value!=='Seed')throw 'pending recovery';");
 }
}
#[test]
fn form_reset_output_runtime_failure_after_plan_preparation_cleans_and_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var form=document.getElementById('owner'),first=document.getElementById('first'),edit=document.getElementById('edit');first.defaultValue='lengthy default value';first.value='dirty';edit.value='dirty';var old=first.firstChild,watch=new MutationObserver(function(){});watch.observe(first,{childList:true});");
 let strings=p.state.limits.host.string_bytes;p.state.limits.host.string_bytes=8;p.begin_job();
 let error=p.runtime.execute("form.reset();",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("OUTPUT default value"),"{error}");assert!(p.state.form_reset_pending.is_none()&&p.state.synthetic_dispatches.is_empty());
 p.state.limits.host.string_bytes=strings;p.begin_job();run(&mut d,&mut f,&mut p,"if(first.value!=='dirty'||first.firstChild!==old||edit.value!=='dirty'||watch.takeRecords().length)throw 'failed output changed native state';form.reset();if(first.value!=='lengthy default value'||edit.value!=='Seed'||watch.takeRecords().length!==1)throw 'runtime cleanup recovery';");
}

#[test]
fn form_reset_output_later_failure_keeps_prior_output_step_and_native_editors_unpublished(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var form=document.getElementById('owner'),first=document.getElementById('first'),second=document.getElementById('second'),external=document.getElementById('external'),edit=document.getElementById('edit');first.value='dirty first';second.defaultValue='lengthy second default';second.value='dirty second';external.value='dirty external';edit.value='dirty editor';var old=first.firstChild,watch=new MutationObserver(function(){});watch.observe(document.body,{childList:true,subtree:true});");
 let strings=p.state.limits.host.string_bytes;p.state.limits.host.string_bytes=8;p.begin_job();let error=p.runtime.execute("form.reset();",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("OUTPUT default value"),"{error}");assert!(p.state.form_reset_pending.is_none());
 p.state.limits.host.string_bytes=strings;p.begin_job();run(&mut d,&mut f,&mut p,"var records=watch.takeRecords();if(first.value!=='First'||second.value!=='dirty second'||external.value!=='dirty external'||edit.value!=='dirty editor'||first.firstChild===old||old.parentNode!==null||records.length!==0)throw 'per-output failure boundary '+first.value+','+second.value+','+external.value+','+edit.value+','+records.length;form.reset();if(second.value!=='lengthy second default'||external.value!=='External'||edit.value!=='Seed'||watch.takeRecords().length!==3)throw 'later failure recovery';");
}

#[test]
fn form_reset_output_native_reset_adapter_rechecks_nested_output_owner(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var first=document.getElementById('first'),nested=document.createElement('output'),edit=document.getElementById('edit'),order=[];first.value='outer current';first.appendChild(nested);nested.defaultValue='nested default';nested.value='nested current';edit.value='dirty';var watch=new MutationObserver(function(records){for(var record of records)order.push(record.target.id);});watch.observe(document.body,{childList:true,subtree:true});");
 let owner=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("owner")).unwrap();p.begin_job();reset_form(&mut d,&mut f,Some(&mut p),owner).unwrap();run(&mut d,&mut f,&mut p,"if(first.value!=='First'||nested.value!=='nested current'||nested.form!==null||edit.value!=='Seed'||order.join(',')!=='first,second,external')throw 'native current owner reset';");
}
