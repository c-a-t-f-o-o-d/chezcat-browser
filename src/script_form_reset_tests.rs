//! Original FORM.reset() method and synchronous native reset behavior.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<html><body><form id=form><input id=edit name=q value=Seed><input id=check type=checkbox checked><input id=r1 type=radio name=pick checked><input id=r2 type=radio name=pick><textarea id=area>Area</textarea><select id=select><option value=a selected>A</option><option value=b>B</option></select><button id=button type=button value=kept>Caption</button></form><form id=other></form><input id=neighbor value=neighbor></body></html>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://form-reset.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn form_reset_canonical_method_descriptor_and_actual_brand(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.getElementById('form'),P=HTMLFormElement.prototype,m=Object.getOwnPropertyDescriptor(P,'reset');if(!m||m.value.name!=='reset'||m.value.length!==0||!m.enumerable||!m.writable||!m.configurable||x.reset!==m.value||Object.hasOwn(x,'reset'))throw 'canonical reset';for(var bad of [{},document,document.getElementById('edit'),Object.create(P)]){try{m.value.call(bad);throw 'missing reset brand';}catch(e){if(!(e instanceof TypeError))throw e;}}");}
#[test]
fn form_reset_synchronous_trusted_cancelable_event_then_native_values(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.getElementById('form'),edit=document.getElementById('edit'),area=document.getElementById('area'),calls=0,received;edit.value='dirty';area.value='dirty area';x.onreset=function(e){if(!e.isTrusted||!e.bubbles||!e.cancelable||e.target!==x||e.currentTarget!==x||edit.value!=='dirty'||area.value!=='dirty area')throw 'reset event ordering';calls++;received=e;};if(x.reset()!==undefined||calls!==1||edit.value!=='Seed'||area.value!=='Area'||received.currentTarget!==null||received.eventPhase!==0||!received.isTrusted)throw 'synchronous native reset';");}

#[test]
fn form_reset_cancellation_recursion_bubbling_and_listener_exceptions(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,r#"var x=document.getElementById('form'),edit=document.getElementById('edit'),log='',event;
 edit.value='dirty';document.addEventListener('reset',function(e){if(e.target===x)log+='c';},true);
 x.onreset=function(e){log+='f';event=e;x.reset();return false;};document.addEventListener('reset',function(e){if(e.target===x)log+='b';});
 x.reset();if(log!=='cfb'||edit.value!=='dirty'||!event.defaultPrevented)throw 'canceled recursion';
 x.onreset=function(e){e.preventDefault();};x.reset();if(edit.value!=='dirty')throw 'preventDefault';
 x.onreset=null;function broken(){throw 'listener exception';}x.addEventListener('reset',broken);x.reset();if(edit.value!=='Seed')throw 'listener exception stopped reset';x.removeEventListener('reset',broken);"#);
 assert_eq!(p.state.errors,1);assert!(p.state.synthetic_dispatches.is_empty());
}
#[test]
fn form_reset_current_defaults_flags_radio_order_and_neighbor_selection(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,r#"var x=document.getElementById('form'),edit=document.getElementById('edit'),area=document.getElementById('area'),check=document.getElementById('check'),r1=document.getElementById('r1'),r2=document.getElementById('r2'),select=document.getElementById('select'),neighbor=document.getElementById('neighbor');edit.value='dirty';area.value='dirty';check.checked=false;r2.checked=true;select.selectedIndex=1;neighbor.value='outside';
 x.onreset=function(){edit.defaultValue='New';area.defaultValue='New area';r2.defaultChecked=true;x.insertBefore(r2,r1);document.getElementById('button').value='current';};x.reset();
 if(edit.value!=='New'||area.value!=='New area'||!check.checked||!r1.checked||r2.checked||select.selectedIndex!==0||neighbor.value!=='outside'||document.getElementById('button').value!=='current')throw 'defaults and current order';"#);
 for id in ["edit","area","check","r1","r2"]{let n=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some(id)).unwrap();let c=&f.controls[&n];assert!(!c.dirty_value&&!c.dirty_checked,"{id}");assert_eq!((c.value.cursor,c.value.anchor,c.value.offset),(c.value.chars.len(),c.value.chars.len(),0));}
 let s=f.controls.values().find_map(|c|c.select.as_ref()).unwrap();assert!(s.options.iter().all(|o|!o.dirty_selected));
}
#[test]
fn form_reset_callbacks_reassociate_controls_and_nested_other_form(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,r#"var x=document.getElementById('form'),other=document.getElementById('other'),edit=document.getElementById('edit'),neighbor=document.getElementById('neighbor'),calls=0;edit.value='preserved';neighbor.value='reset me';other.onreset=function(){calls++;};x.onreset=function(){other.appendChild(edit);x.appendChild(neighbor);other.reset();edit.value='after nested';};x.reset();if(calls!==1||edit.value!=='after nested'||edit.form!==other||neighbor.value!=='neighbor'||neighbor.form!==x)throw 'current owner and nested form';"#);
}
#[test]
fn form_reset_saved_method_named_mask_and_initial_child_refusal(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,r#"var x=document.getElementById('form'),edit=document.getElementById('edit'),P=HTMLFormElement.prototype,reset=P.reset,field=document.createElement('input');field.name='reset';x.appendChild(field);if(x.reset!==field)throw 'named masking';edit.value='dirty';reset.call(x);if(edit.value!=='Seed')throw 'saved masked reset';P.reset=function(){return 7;};field.name='other';if(x.reset()!==7)throw 'author override';delete P.reset;if(x.reset!==undefined)throw 'deleted canonical';edit.value='again';reset.call(x);if(edit.value!=='Seed')throw 'saved canonical';P.reset=reset;var inert=document.implementation.createHTMLDocument('').createElement('form');try{reset.call(inert);throw 'inert fake success';}catch(e){if(!(e instanceof DOMException)||e.name!=='NotSupportedError')throw e;}"#);
}
#[test]
fn form_reset_saved_child_authority_rejects_foreign_origin_and_recovers(){
 let(mut d,mut f,mut p)=fixture();
 run(&mut d,&mut f,&mut p,"var x=document.getElementById('form'),edit=document.getElementById('edit'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var reset=frame.contentWindow.HTMLFormElement.prototype.reset,calls=0;x.onreset=function(){calls++;};edit.value='dirty';");
 p.set_location(&crate::navigation::address("https://foreign-reset.test/",&crate::navigation::Location::home()).unwrap());p.begin_job();
 let error=p.runtime.execute("reset.call(x);",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");
 p.set_location(&crate::navigation::address("https://form-reset.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();run(&mut d,&mut f,&mut p,"if(calls||edit.value!=='dirty')throw 'foreign reset mutation';reset.call(x);if(calls!==1||edit.value!=='Seed')throw 'authority recovery';");
}
#[test]
fn form_reset_checked_preparation_refuses_without_partial_native_state_and_recovers(){
 for mode in 0..4{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.getElementById('form');document.getElementById('edit').value='dirty';document.getElementById('area').value='dirty area';");
  let Value::Host(form)=p.runtime.execute("x;",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap()else{panic!("form")};
  let before=f.controls.iter().map(|(&n,c)|(n,c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.checked,c.dirty_checked,c.caption.clone())).collect::<Vec<_>>();
  let limit=p.state.retained_limit;p.begin_job();match mode{0=>p.state.copy_work=0,1=>p.state.host_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  assert!(Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.prepare_native_form_reset(form).is_err(),"mode {mode}");
  assert_eq!(f.controls.iter().map(|(&n,c)|(n,c.value.text(),c.value.cursor,c.value.anchor,c.value.offset,c.dirty_value,c.checked,c.dirty_checked,c.caption.clone())).collect::<Vec<_>>(),before);
  p.state.retained_limit=limit;p.state.execution_deadline=None;p.begin_job();run(&mut d,&mut f,&mut p,"x.reset();if(document.getElementById('edit').value!=='Seed'||document.getElementById('area').value!=='Area')throw 'reset fresh recovery';");
 }
}
#[test]
fn form_reset_fatal_listener_unwinds_lock_keeps_native_values_and_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.getElementById('form'),edit=document.getElementById('edit'),captured;edit.value='dirty';function heavy(e){captured=e;while(true){}}x.addEventListener('reset',heavy);");
 let out=p.console_eval(&mut d,&mut f,"x.reset();");assert_eq!(out.errors,1);assert!(p.state.synthetic_dispatches.is_empty());assert!(p.state.created_events.iter().all(|e|!e.dispatching&&e.current.is_none()&&e.phase==0));
 let out=p.console_eval(&mut d,&mut f,"if(edit.value!=='dirty'||captured.currentTarget!==null||captured.eventPhase!==0)throw 'fatal partial reset';x.removeEventListener('reset',heavy);x.reset();if(edit.value!=='Seed')throw 'lock recovery';");assert_eq!(out.errors,0,"{}",p.report());
}
#[test]
fn form_reset_owned_output_restores_defaults_and_preserves_callback_side_effects(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var x=document.getElementById('form'),edit=document.getElementById('edit'),output=document.createElement('output');x.appendChild(output);output.defaultValue='default';output.value='dirty';edit.value='dirty';x.onreset=function(){x.setAttribute('data-side','kept');};");
 p.begin_job();run(&mut d,&mut f,&mut p,"x.reset();if(edit.value!=='Seed'||output.value!=='default'||x.getAttribute('data-side')!=='kept')throw 'reset side effects';output.value='outside';document.body.appendChild(output);edit.value='dirty';x.reset();if(edit.value!=='Seed'||output.value!=='outside')throw 'unrelated output isolation';");assert!(p.state.synthetic_dispatches.is_empty()&&p.state.form_reset_pending.is_none());
}

#[test]
fn form_reset_staging_storage_sweep_and_dispatch_preparation_rollback(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"document.getElementById('edit').value='dirty';document.getElementById('area').value='dirty area';");
 let owner=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("form")).unwrap();
 let before=f.controls.iter().map(|(&n,c)|(n,c.value.text(),c.dirty_value,c.checked,c.caption.clone())).collect::<Vec<_>>();
 let(mut refused,mut admitted)=(0,0);
 for available in (0..8192).step_by(37){
  match forms::ResetPlan::prepare(&f,&d,owner,available){Ok(_)=>admitted+=1,Err(_)=>refused+=1}
  assert_eq!(f.controls.iter().map(|(&n,c)|(n,c.value.text(),c.dirty_value,c.checked,c.caption.clone())).collect::<Vec<_>>(),before);
 }
 assert!(refused>2&&admitted>2);
 for copies in [0,1,4,8,16,32,64,128,256,512]{
  let(mut d,mut f,mut p)=fixture();p.begin_job();p.state.copy_work=copies;
  let result=Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.begin_native_form_reset(node_handle(owner));
  match result{Err(_)=>assert!(p.state.created_events.is_empty()),Ok(Some(token))=>{Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}.finish_synthetic_dispatch(token);},Ok(None)=>panic!("unexpected lock")}
  assert!(p.state.synthetic_dispatches.is_empty());p.begin_job();run(&mut d,&mut f,&mut p,"document.getElementById('form').reset();");
 }
}
