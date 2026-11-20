//! Exact native keyboard record/payload admission and unpublished rollback.
use super::*;
#[test]
fn keyboard_event_storage_admits_actual_utf16_payload_and_rolls_back_all_owned_capacity(){
    let mut doc=crate::html::parse("<html><body></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    let baseline=page.state.retained;let limit=page.state.retained_limit;let next=page.state.next_event;
    let fields=[("key",Value::StringUnits(vec![0xd800,0x61])),("code",Value::String("Code".into()))];
    let slot=size_of::<CreatedEvent>();let payload=1+4+4;
    page.state.retained_limit=baseline+slot+payload-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_keyboard_event(WINDOW,&[Value::String("x".into())],Some(&fields)).is_err());
    assert_eq!((page.state.retained,page.state.next_event,page.state.created_events.capacity()),(baseline,next,0));assert!(page.state.created_events.is_empty());
    page.begin_job();page.state.retained_limit+=1;
    let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_keyboard_event(WINDOW,&[Value::String("x".into())],Some(&fields)).unwrap() else{panic!("genuine keyboard")};
    let event=&page.state.created_events[0];assert_eq!(event.payload_bytes(),payload);assert_eq!(event.data.keyboard().unwrap().payload_bytes(),8);
    assert_eq!(page.state.retained,baseline+page.state.created_events.capacity()*slot+event.payload_bytes());
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.rollback_base_event(handle).unwrap();
    assert_eq!(page.state.retained,baseline);assert_eq!(page.state.created_events.capacity(),0);assert_eq!(page.state.next_event,next+1);
    page.state.retained_limit=limit;page.begin_job();page.state.copy_work=7;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_keyboard_event(WINDOW,&[Value::String("x".into())],Some(&fields)).is_err());
    assert_eq!(page.state.retained,baseline);assert_eq!(page.state.created_events.capacity(),0);assert_eq!(page.state.next_event,next+1);
    page.begin_job();let fields=[("key",Value::String("original".repeat(3000)))];
    let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.construct_keyboard_event(WINDOW,&[Value::String("recovered".into())],Some(&fields)).unwrap() else{panic!("genuine recovered keyboard")};
    let event=&page.state.created_events[0];assert!(event.data.keyboard().unwrap().key.bytes()>16384);
    assert_eq!(page.state.retained,baseline+page.state.created_events.capacity()*slot+event.payload_bytes());
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.rollback_base_event(handle).unwrap();assert_eq!(page.state.retained,baseline);
}
