//! Original production event arena, payload and identity acceptance.
use super::*;
fn page()->(Document,Forms,Page,usize) {
    let mut doc=crate::html::parse("<html><head><title>waiting</title></head><body><button id=b type=button>Run</button></body></html>");
    doc.scripting_enabled=true;
    let node=doc.nodes.iter().enumerate().find_map(|(index,_)|(doc.attr(index,"id")==Some("b")).then_some(index)).unwrap();
    let mut forms=Forms::new(&doc);
    let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let outcome=page.start(&mut doc,&mut forms,&[Script{node:0,label:"Original event storage fixture".into(),source:"var b=document.getElementById('b');".into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    (doc,forms,page,node)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();
    page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
#[test]
fn browser_event_constructor_and_legacy_factory_exceed_former_count_and_dispatch_the_last_identity() {
    let(mut doc,mut forms,mut page,_)=page();
    execute(&mut page,&mut doc,&mut forms,"var first,last,legacy,hits=0;for(var i=0;i<70000;i++){last=new Event('constructed');if(i===0)first=last;}");
    assert_eq!(page.state.created_events.len(),70000);
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<70000;i++)legacy=document.createEvent('Event');legacy.initEvent('legacy',false,false);b.addEventListener('constructed',function(event){if(event!==last||event.isTrusted)throw 'constructor identity';hits++;});b.addEventListener('legacy',function(event){if(event!==legacy||event.isTrusted)throw 'factory identity';hits++;document.title='large event arena delivered';});if(first.type!=='constructed')throw 'first identity';b.dispatchEvent(last);b.dispatchEvent(legacy);");
    assert_eq!(page.state.created_events.len(),140000);
    assert!(page.state.created_events.windows(2).all(|pair|pair[0].handle<pair[1].handle));
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),2.0);
    assert_eq!(doc.title,"large event arena delivered");
}
#[test]
fn browser_event_types_exceed_former_byte_limit_through_constructor_initializer_and_listener_delivery() {
    let(mut doc,mut forms,mut page,_)=page();
    let name=format!("{}Ω","original".repeat(3000));
    assert!(name.len()>16384);
    page.runtime.define_global("largeType",Value::String(name.clone())).unwrap();
    execute(&mut page,&mut doc,&mut forms,"var hits=0;b.addEventListener(largeType,function(event){if(event.type!==largeType)throw 'type bytes changed';hits++;document.title='long event type delivered';});var e=new CustomEvent(largeType,{detail:'real'}),f=document.createEvent('Event');f.initEvent(largeType,false,false);b.dispatchEvent(e);b.dispatchEvent(f);if(e.detail!=='real'||hits!==2)throw 'type delivery';");
    assert_eq!(doc.title,"long event type delivered");
    assert_eq!(page.state.created_events[0].name,name);
    assert_eq!(page.state.created_events[1].name,name);
}
#[test]
fn browser_event_record_storage_refusal_and_empty_rollback_restore_the_exact_owned_allowance() {
    let(mut doc,mut forms,mut page,_)=page();
    let baseline=page.state.retained;
    let next=page.state.next_event;
    let limit=page.state.retained_limit;
    let slot=std::mem::size_of::<CreatedEvent>();
    page.state.retained_limit=baseline+slot;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_base_event(WINDOW,&[Value::String("x".into())],None).is_err());
    assert_eq!(page.state.retained,baseline);
    assert_eq!(page.state.next_event,next);
    assert_eq!(page.state.created_events.capacity(),0);
    page.state.retained_limit=baseline+slot+1;
    let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_base_event(WINDOW,&[Value::String("x".into())],None).unwrap() else {panic!("real Event")};
    assert_eq!(page.state.created_events.capacity(),1);
    assert_eq!(page.state.retained,baseline+slot+1);
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.rollback_base_event(handle).unwrap();
    assert_eq!(page.state.retained,baseline);
    assert_eq!(page.state.created_events.capacity(),0);
    assert_eq!(page.state.next_event,next+1);
    page.state.retained_limit=limit;
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"new Event('recovered').type;").to_text(),"recovered");
}
#[test]
fn browser_event_record_growth_copy_refusal_publishes_no_identity_and_preserves_existing_record() {
    let(mut doc,mut forms,mut page,_)=page();
    let baseline=page.state.retained;
    let limit=page.state.retained_limit;
    let slot=std::mem::size_of::<CreatedEvent>();
    page.state.retained_limit=baseline+slot+1;
    let Value::Host(first)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_base_event(WINDOW,&[Value::String("x".into())],None).unwrap() else {panic!("real Event")};
    let before=(page.state.retained,page.state.next_event,page.state.created_events.capacity());
    page.state.retained_limit=baseline+slot*2+2;
    page.state.copy_work=slot;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_base_event(WINDOW,&[Value::String("y".into())],None).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.copy_work,0);
    assert_eq!((page.state.retained,page.state.next_event,page.state.created_events.capacity()),before);
    assert_eq!(page.state.created_events.len(),1);
    assert_eq!(page.state.created_events[0].handle,first);
    assert_eq!(page.state.created_events[0].name,"x");
    page.state.retained_limit=limit;
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"new Event('fresh').type;").to_text(),"fresh");
}
#[test]
fn browser_event_custom_payload_replacement_refuses_atomically_and_refunds_replaced_owned_capacity() {
    let(mut doc,mut forms,mut page,_)=page();
    let raw=Value::StringUnits(vec![0xda22,0xdf06]);
    let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_custom_event(WINDOW,&[Value::String("old".into())],Some(&[("detail",raw)])).unwrap() else {panic!("real CustomEvent")};
    let before=page.state.retained;
    let old=page.state.created_events[0].payload_bytes();
    assert_eq!(old,7);
    let limit=page.state.retained_limit;
    page.state.retained_limit=before+1;
    let args=[Value::String("larger".into()),Value::Bool(true),Value::Bool(true),Value::String("d".repeat(10000))];
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.created_event_call(handle,"initCustomEvent",&args).unwrap().is_err());
    assert_eq!(page.state.retained,before);
    assert_eq!(page.state.created_events[0].name,"old");
    assert!(matches!(page.state.created_events[0].custom.as_ref(),Some(Value::StringUnits(units)) if units==&[0xda22,0xdf06]));
    page.state.retained_limit=limit;
    let args=[Value::String("new".into()),Value::Bool(false),Value::Bool(false),Value::String("fresh".into())];
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.created_event_call(handle,"initCustomEvent",&args).unwrap().unwrap();
    let new=page.state.created_events[0].payload_bytes();
    assert_eq!(new,8);
    assert_eq!(page.state.retained,before-old+new);
}

#[test]
fn browser_event_bigint_detail_keeps_its_shared_payload_charge_and_replacement_refunds_it() {
    let(mut doc,mut forms,mut page,_)=page();
    let bigint=execute(&mut page,&mut doc,&mut forms,"BigInt('123456789012345678901234567890');");
    let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_custom_event(WINDOW,&[Value::String("big".into())],Some(&[("detail",bigint.clone())])).unwrap() else {panic!("CustomEvent")};
    assert_eq!(page.state.created_events[0].payload_bytes(),3+bigint.bytes());
    let before=page.state.retained;
    let args=[Value::String("big".into()),Value::Bool(false),Value::Bool(false),Value::Null];
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.created_event_call(handle,"initCustomEvent",&args).unwrap().unwrap();
    assert_eq!(page.state.retained,before-bigint.bytes());
}

#[test]
fn browser_abort_reason_reservation_cannot_be_spent_again_by_event_and_dispatch_admission() {
    let(mut doc,mut forms,mut page,_)=page();
    execute(&mut page,&mut doc,&mut forms,"var controller=new AbortController(),signal=controller.signal;");
    page.runtime.define_global("reason",Value::String("r".repeat(10000))).unwrap();
    let baseline=page.state.retained;
    let limit=page.state.retained_limit;
    page.state.retained_limit=baseline+10000+std::mem::size_of::<CreatedEvent>()+5;
    page.begin_job();
    assert!(page.runtime.execute("controller.abort(reason);",&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).is_err());
    assert_eq!(page.state.retained,baseline);
    assert!(page.state.created_events.is_empty());
    assert!(page.state.synthetic_dispatches.is_empty());
    page.state.retained_limit=limit;
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"signal.aborted;").to_text(),"false");
    execute(&mut page,&mut doc,&mut forms,"controller.abort(reason);if(!signal.aborted||signal.reason!==reason)throw 'abort recovery';");
    assert!(page.state.retained<=page.state.retained_limit);
}
