//! Original shared-storage, readiness and active-payload acceptance cases.
use super::*;
fn page(source:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><head><title>waiting</title></head><body>Messages</body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let result=page.start(&mut doc,&mut forms,&[Script{node:0,source:source.into(),label:"Original queue fixture".into()}]);
    assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
fn post(page:&mut Page,doc:&mut Document,forms:&mut Forms,value:&Value)->Result<Value,String> {
    Bridge{doc,forms,state:&mut page.state}.message_call(PORT_BASE,POST,std::slice::from_ref(value))
}
fn poll(page:&mut Page,doc:&mut Document,forms:&mut Forms) {
    let result=page.poll_timer_at(doc,forms,Instant::now()+Duration::from_secs(1)).unwrap();
    assert_eq!(result.errors,0,"{}",page.report());
}
#[test]
fn browser_message_queue_exceeds_old_count_and_delivers_real_fifo_after_microtasks() {
    let(mut doc,mut forms,mut page)=page("var ch=new MessageChannel(),hits=0,micro=0;ch.port2.onmessage=function(e){if(e.data!==hits||micro!==hits)throw 'FIFO';hits++;Promise.resolve().then(function(){micro++;});};for(var i=0;i<5000;i++)ch.port1.postMessage(i);");
    assert_eq!(page.state.message_channels.queue.len(),5000);
    let retained=page.state.retained;
    page.state.host_work=0;page.state.copy_work=0;
    for _ in 0..5000 {assert!(page.next_timer_deadline().is_some());}
    assert_eq!(page.state.retained,retained);
    for _ in 0..5000 {poll(&mut page,&mut doc,&mut forms);}
    assert!(page.next_timer_deadline().is_none());
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"if(hits!==5000||micro!==5000)throw 'lost messages';hits;").to_number(),5000.0);
    assert!(page.state.retained<=page.state.retained_limit);
}
#[test]
fn browser_message_readiness_tracks_inactive_prefix_close_and_callback_activation() {
    let(mut doc,mut forms,mut page)=page("var a=new MessageChannel(),b=new MessageChannel(),c=new MessageChannel(),trace='';a.port1.postMessage('a1');b.port1.postMessage('b1');a.port1.postMessage('a2');c.port1.postMessage('c1');b.port1.postMessage('b2');b.port2.onmessage=function(e){trace+=e.data;if(e.data==='b1'){a.port2.onmessage=function(x){trace+=x.data;};c.port2.close();}};");
    assert_eq!(page.state.message_channels.ready,Some(1));
    poll(&mut page,&mut doc,&mut forms);
    assert_eq!(page.state.message_channels.ready,Some(0));
    for _ in 0..3 {poll(&mut page,&mut doc,&mut forms);}
    assert!(page.next_timer_deadline().is_none());
    execute(&mut page,&mut doc,&mut forms,"if(trace!=='b1a1a2b2')throw trace;");
    // Closing the last queued destination releases the reusable deque backing.
    execute(&mut page,&mut doc,&mut forms,"a.port1.postMessage('cancel');a.port2.close();");
    assert_eq!(page.state.message_channels.queue.capacity(),0);
    assert_eq!(page.state.message_channels.ready,None);
}
#[test]
fn browser_message_storage_copy_and_activation_failures_are_atomic_and_recover() {
    let(mut doc,mut forms,mut page)=page("var ch=new MessageChannel();");
    let baseline=page.state.retained;let limit=page.state.retained_limit;let slot=size_of::<Message>();
    let value=Value::StringUnits(vec![0xd800,0x41]);let bytes=4;
    page.state.retained_limit=baseline+slot+bytes-1;
    assert!(post(&mut page,&mut doc,&mut forms,&value).is_err());
    assert_eq!(page.state.retained,baseline);assert_eq!(page.state.message_channels.queue.capacity(),0);
    page.state.retained_limit=baseline+slot+bytes;
    post(&mut page,&mut doc,&mut forms,&value).unwrap();
    assert_eq!(page.state.message_channels.queue.capacity(),1);assert_eq!(page.state.retained,baseline+slot+bytes);
    page.state.retained_limit=limit;
    page.state.copy_work=bytes+slot-1;
    assert!(post(&mut page,&mut doc,&mut forms,&value).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.retained,baseline+slot+bytes);assert_eq!(page.state.message_channels.queue.len(),1);
    page.begin_job();post(&mut page,&mut doc,&mut forms,&value).unwrap();
    // Admission must precede both the start flag and handler publication.
    page.state.host_work=1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.message_call(PORT_BASE+1,SET_MESSAGE,&[Value::Null]).unwrap_err().contains("DOM work limit"));
    assert!(!page.state.message_channels.channels[0][1].started);assert_eq!(page.state.message_channels.ready,None);
    page.begin_job();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.message_call(PORT_BASE+1,START,&[]).unwrap();
    let before=(page.state.retained,page.state.message_channels.queue.len(),page.state.message_channels.ready);
    page.state.copy_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.message_call(PORT_BASE+1,CLOSE,&[]).is_err());
    assert!(!page.state.message_channels.channels[0][1].detached);
    assert_eq!((page.state.retained,page.state.message_channels.queue.len(),page.state.message_channels.ready),before);
    page.clear_message_tasks();assert_eq!(page.state.retained,baseline);assert_eq!(page.state.message_channels.ready,None);
    page.begin_job();post(&mut page,&mut doc,&mut forms,&Value::Number(7.0)).unwrap();
    assert_eq!(page.state.message_channels.ready,Some(0));
    page.clear_message_tasks();assert_eq!(page.state.retained,baseline);
}
#[test]
fn browser_message_active_payload_blocks_callback_growth_then_releases_for_recovery() {
    let(mut doc,mut forms,mut page)=page("var ch=new MessageChannel();ch.port2.onmessage=function(e){if(typeof e.data!=='string')throw 'payload';document.title=candidate;};");
    page.runtime.define_global("candidate",Value::String("t".repeat(12000))).unwrap();
    // Warm callback/native metadata with a small delivery and restore the title.
    post(&mut page,&mut doc,&mut forms,&Value::String("warm".into())).unwrap();poll(&mut page,&mut doc,&mut forms);
    execute(&mut page,&mut doc,&mut forms,"document.title='waiting';");
    post(&mut page,&mut doc,&mut forms,&Value::String("p".repeat(16000))).unwrap();
    let before=page.state.retained;let limit=page.state.retained_limit;
    page.state.retained_limit=before+16000+8000;
    let result=page.poll_timer_at(&mut doc,&mut forms,Instant::now()+Duration::from_secs(1)).unwrap();
    assert_eq!(result.errors,1,"{}",page.report());assert_eq!(doc.title,"waiting");
    assert!(page.state.message_channels.queue.is_empty());
    assert!(page.state.retained<before+16000);
    // A subsequent small payload can use the released active-message allowance.
    post(&mut page,&mut doc,&mut forms,&Value::String("recover".into())).unwrap();poll(&mut page,&mut doc,&mut forms);
    assert_eq!(doc.title.len(),12000);page.state.retained_limit=limit;
}
#[test]
fn browser_message_bigint_and_utf16_data_survive_real_delivery_and_dispatch_refusal_refunds_queue() {
    let(mut doc,mut forms,mut page)=page("var ch=new MessageChannel(),hits=0;ch.port2.onmessage=function(e){if(hits===0&&e.data!==BigInt('123456789012345678901234567890'))throw 'bigint';if(hits===1&&(e.data.length!==2||e.data.charCodeAt(0)!==55296))throw 'UTF16';hits++;};ch.port1.postMessage(BigInt('123456789012345678901234567890'));ch.port1.postMessage('\\ud800A');");
    poll(&mut page,&mut doc,&mut forms);poll(&mut page,&mut doc,&mut forms);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),2.0);
    page.clear_message_tasks();let baseline=page.state.retained;
    post(&mut page,&mut doc,&mut forms,&Value::String("pending".into())).unwrap();
    let work=page.state.limits.host_work;page.state.limits.host_work=0;
    let result=page.poll_timer_at(&mut doc,&mut forms,Instant::now()+Duration::from_secs(1)).unwrap();
    assert_eq!(result.errors,1);assert_eq!(page.state.retained,baseline);
    assert_eq!(page.state.message_channels.queue.capacity(),0);assert_eq!(page.state.message_channels.ready,None);
    page.state.limits.host_work=work;page.begin_job();
    post(&mut page,&mut doc,&mut forms,&Value::Number(3.0)).unwrap();poll(&mut page,&mut doc,&mut forms);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),3.0);
}
