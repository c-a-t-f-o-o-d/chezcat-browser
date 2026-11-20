//! Original dynamic request ownership, admission, execution and cancellation.
use super::*;
fn page()->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><head></head><body>Original script storage</body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(&crate::navigation::Location{url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let result=page.console_eval(doc,forms,source);assert_eq!(result.errors,0,"{}",page.report());
}
#[test]
fn browser_dynamic_scripts_exceed_former_count_and_lifetime_bytes_with_real_execution_and_transferred_ownership() {
    let(mut doc,mut forms,mut page)=page();page.state.dynamic.script_count=usize::MAX;page.state.dynamic.source_bytes=usize::MAX;
    execute(&mut page,&mut doc,&mut forms,"var hits=0;for(var i=0;i<6000;i++){var script=document.createElement('script');script.src='/original.js';document.head.appendChild(script);}");
    let used=page.state.dynamic.pending.used.clone();let mut batch=page.take_dynamic_scripts();assert_eq!(batch.len(),6000);
    let queue_bytes=batch.capacity()*size_of::<DynamicScript>();assert!(used.load(Ordering::Acquire)>=queue_bytes);
    assert!(Arc::ptr_eq(&batch[0].policy,&batch[5999].policy));let saved=batch[0].clone();
    for request in batch.iter(){assert!(request.error.is_none());assert_eq!(request.source_limit,page.state.limits.capacity.source_bytes);}
    while let Some(request)=batch.pop_front(){let result=page.complete_dynamic_script(&mut doc,&mut forms,&request,Ok(Script{node:request.node,source:"hits++;".into(),label:"Original completed script".into()}));assert_eq!(result.errors,0,"{}",page.report());}
    execute(&mut page,&mut doc,&mut forms,"if(hits!==6000)throw 'real queued execution';");assert_eq!(page.state.dynamic.in_flight,0);
    assert!(used.load(Ordering::Acquire)>queue_bytes);drop(page);assert!(used.load(Ordering::Acquire)>queue_bytes);
    drop(saved);assert_eq!(used.load(Ordering::Acquire),queue_bytes);drop(batch);assert_eq!(used.load(Ordering::Acquire),0);
}
#[test]
fn browser_dynamic_script_quota_and_copy_refusal_publish_no_started_bit_or_request_and_recover() {
    let(mut doc,mut forms,mut page)=page();
    execute(&mut page,&mut doc,&mut forms,"var candidate=document.createElement('script');candidate.id='candidate';candidate.src='/candidate.js';");
    let used=page.state.dynamic.pending.used.clone();let limit=page.state.limits.host.retained_bytes;page.state.limits.host.retained_bytes=0;
    let result=page.console_eval(&mut doc,&mut forms,"document.head.appendChild(candidate);");assert_eq!(result.errors,1);
    assert_eq!(used.load(Ordering::Acquire),0);assert!(page.state.dynamic.pending.is_empty());assert!(page.state.dynamic.started.is_empty());assert_eq!(page.state.dynamic.in_flight,0);
    let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id=="candidate")).unwrap();
    page.state.limits.host.retained_bytes=limit;page.begin_job();page.state.copy_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.queue_dynamic_script(node).unwrap_err().contains("string work limit"));
    assert_eq!(used.load(Ordering::Acquire),0);assert!(page.state.dynamic.started.is_empty());assert_eq!(page.state.dynamic.in_flight,0);
    page.begin_job();Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.queue_dynamic_script(node).unwrap();assert_eq!(page.state.dynamic.pending.len(),1);
    let request=page.take_dynamic_scripts().pop_front().unwrap();let held=used.load(Ordering::Acquire);assert!(held>0);
    let clone=request.clone();page.clear_host_requests();assert_eq!(page.state.dynamic.in_flight,0);drop(page);assert!(used.load(Ordering::Acquire)>0);
    drop(request);assert!(used.load(Ordering::Acquire)>0);drop(clone);assert_eq!(used.load(Ordering::Acquire),0);
}
