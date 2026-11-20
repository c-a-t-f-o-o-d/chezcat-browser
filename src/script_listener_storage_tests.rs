//! Original native and synchronous listener-capacity/lifecycle acceptance.
use super::*;
fn page(source:&str)->(Document,Forms,Page,usize) {
    let mut doc=crate::html::parse("<html><head><title>waiting</title></head><body><button id=b type=button>Run</button></body></html>");
    doc.scripting_enabled=true;
    let node=doc.nodes.iter().enumerate().find_map(|(index,_)|(doc.attr(index,"id")==Some("b")).then_some(index)).unwrap();
    let mut forms=Forms::new(&doc);
    let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let outcome=page.start(&mut doc,&mut forms,&[Script{node:0,label:"Original listener storage acceptance".into(),source:source.into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    (doc,forms,page,node)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();
    page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
fn click(page:&mut Page,doc:&mut Document,forms:&mut Forms,node:usize) {
    page.begin_job();
    let outcome=page.dispatch(doc,forms,node,"click",true);
    assert_eq!(outcome.errors,0,"{}",page.report());
}
#[test]
fn browser_listener_capacity_exceeds_former_count_with_real_native_and_synchronous_delivery() {
    let(mut doc,mut forms,mut page,node)=page("var b=document.getElementById('b'),hits=0;function observed(){hits++;if(hits===5000||hits===10000)document.title='delivered '+hits;}for(var i=0;i<5000;i++)b.addEventListener('click',{handleEvent:observed});var event=document.createEvent('Event');event.initEvent('click',false,false);");
    assert_eq!(page.state.listeners.len(),5000);
    click(&mut page,&mut doc,&mut forms,node);
    assert_eq!(doc.title,"delivered 5000");
    execute(&mut page,&mut doc,&mut forms,"b.dispatchEvent(event);");
    assert_eq!(doc.title,"delivered 10000");
    assert!(page.state.synthetic_dispatches.is_empty());
    assert!(page.state.event.is_none());
}
#[test]
fn browser_listener_storage_refusal_preserves_registered_callback_and_reuses_paid_slots() {
    let(mut doc,mut forms,mut page,node)=page("var b=document.getElementById('b');function kept(){document.title='kept callback';}");
    let callback=execute(&mut page,&mut doc,&mut forms,"kept;");
    let baseline=page.state.retained;
    let limit=page.state.retained_limit;
    let bundle=std::mem::size_of::<Listener>()+std::mem::size_of::<Option<usize>>()+std::mem::size_of::<usize>();
    let options=ListenerOptions::default();
    page.state.retained_limit=baseline+bundle+5-1;
    assert!(page.state.add_listener(node_handle(node),"click",callback.clone(),false,options).is_err());
    assert_eq!(page.state.retained,baseline);
    assert_eq!(page.state.listeners.storage_bytes(),0);
    assert_eq!(page.state.listeners.slot_counts(),(0,0));
    page.state.retained_limit=baseline+bundle+5;
    page.state.add_listener(node_handle(node),"click",callback.clone(),false,options).unwrap();
    assert_eq!(page.state.listeners.storage_bytes(),bundle);
    assert_eq!(page.state.retained,baseline+bundle+5);
    let first=page.state.listeners[0].serial;
    page.state.retained_limit=page.state.retained+bundle+6-1;
    assert!(page.state.add_listener(node_handle(node),"notice",callback.clone(),false,options).is_err());
    assert_eq!(page.state.listeners.len(),1);
    assert_eq!(page.state.listeners[0].serial,first);
    assert_eq!(page.state.listeners.slot_counts(),(1,0));
    assert_eq!(page.state.retained,baseline+bundle+5);
    for _ in 0..2048 {
        page.state.retain_listeners(|_|false).unwrap();
        assert_eq!(page.state.retained,baseline+bundle);
        page.state.add_listener(node_handle(node),"click",callback.clone(),false,options).unwrap();
        assert_eq!(page.state.listeners[0].serial,first);
        assert_eq!(page.state.listeners.slot_counts(),(1,0));
    }
    page.state.retained_limit=limit;
    click(&mut page,&mut doc,&mut forms,node);
    assert_eq!(doc.title,"kept callback");
}
#[test]
fn browser_listener_removed_snapshot_slots_do_not_alias_new_callbacks_in_native_or_script_dispatch() {
    for synthetic in [false,true] {
        let(mut doc,mut forms,mut page,node)=page("var b=document.getElementById('b'),log='';function later(){log+='B';}function added(){log+='C';}function first(){log+='A';b.removeEventListener('click',later);b.addEventListener('click',added);}b.addEventListener('click',first,{once:true});b.addEventListener('click',later);var event=document.createEvent('Event');event.initEvent('click',false,false);");
        if synthetic {execute(&mut page,&mut doc,&mut forms,"b.dispatchEvent(event);");}else{click(&mut page,&mut doc,&mut forms,node);}
        assert_eq!(execute(&mut page,&mut doc,&mut forms,"log;").to_text(),"A");
        assert_eq!(page.state.listeners.len(),1);
        assert_eq!(page.state.listeners.slot_counts(),(3,2));
        if synthetic {execute(&mut page,&mut doc,&mut forms,"b.dispatchEvent(event);");}else{click(&mut page,&mut doc,&mut forms,node);}
        assert_eq!(execute(&mut page,&mut doc,&mut forms,"log;").to_text(),"AC");
    }
}
#[test]
fn browser_listener_snapshot_storage_refusal_unwinds_dispatch_and_fresh_delivery_recovers() {
    let(mut doc,mut forms,mut page,node)=page("var b=document.getElementById('b'),hits=0;function observed(){hits++;}b.addEventListener('click',observed);var event=document.createEvent('Event');event.initEvent('click',false,false);");
    let event=execute(&mut page,&mut doc,&mut forms,"event;");
    page.begin_job();
    let token=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.begin_synthetic_dispatch(node_handle(node),&[event]).unwrap();
    let active=page.state.retained;
    let payload=page.state.synthetic_dispatches[token].storage_bytes();
    let limit=page.state.retained_limit;
    page.state.retained_limit=active+std::mem::size_of::<usize>()-1;
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.next_synthetic_callback(token).err().expect("snapshot storage refusal");
    assert!(error.contains("retained host metadata"),"{error}");
    assert_eq!(page.state.retained,active);
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.finish_synthetic_dispatch(token));
    assert_eq!(page.state.retained,active-payload);
    assert!(page.state.synthetic_dispatches.is_empty());
    page.state.retained_limit=limit;
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),0.0);
    execute(&mut page,&mut doc,&mut forms,"b.dispatchEvent(event);");
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),1.0);
}
#[test]
fn browser_listener_native_dispatch_metadata_refusal_preserves_registration_and_fresh_job_recovers() {
    let(mut doc,mut forms,mut page,node)=page("var b=document.getElementById('b'),hits=0;b.addEventListener('click',function(){hits++;});");
    let retained=page.state.retained;
    let limit=page.state.retained_limit;
    let handles=page.state.next_event;
    page.state.retained_limit=retained;
    page.begin_job();
    let outcome=page.dispatch(&mut doc,&mut forms,node,"click",true);
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(page.state.retained,retained);
    assert_eq!(page.state.next_event,handles);
    assert_eq!(page.state.listeners.len(),1);
    assert!(page.state.event.is_none());
    page.state.retained_limit=limit;
    click(&mut page,&mut doc,&mut forms,node);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),1.0);
}
#[test]
fn browser_listener_snapshot_copy_failure_preserves_storage_and_real_delivery_recovers() {
    let(mut doc,mut forms,mut page,node)=page("var b=document.getElementById('b'),hits=0;b.addEventListener('click',function(){hits++;});");
    // Refuse publication after reservation when the snapshot copy has no work
    // allowance; the original registration remains eligible for real delivery.
    let baseline=page.state.retained;
    page.state.copy_work=0;
    assert!(page.state.listener_snapshot(node_handle(node),"click",false).is_err());
    assert_eq!(page.state.retained,baseline);
    assert_eq!(page.state.copy_work,0);
    click(&mut page,&mut doc,&mut forms,node);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),1.0);
}

#[test]
fn browser_listener_native_snapshot_failure_refunds_event_storage_and_keeps_committed_timer() {
    let(mut doc,mut forms,mut page,node)=page("var b=document.getElementById('b'),hits=0;function noop(){}window.addEventListener('click',function(){setTimeout(noop,60000,large);},true);b.addEventListener('click',function(){hits++;});b.addEventListener('click',function(){hits++;});");
    page.runtime.define_global("large",Value::String("s".repeat(60000))).unwrap();
    let path=page.event_path(&doc,node_handle(node)).unwrap();
    let mut visits=Vec::<(u32,bool,u8)>::new();visits.try_reserve_exact(path.len()*2).unwrap();
    let mut name=String::new();name.try_reserve_exact(5).unwrap();
    let native=path.capacity()*std::mem::size_of::<u32>()+visits.capacity()*std::mem::size_of::<(u32,bool,u8)>()+name.capacity();
    drop((path,visits,name));
    let baseline=page.state.retained;
    let limit=page.state.retained_limit;
    let payload=60000+std::mem::size_of::<Value>();
    page.state.retained_limit=baseline+native+std::mem::size_of::<usize>()+payload+std::mem::size_of::<Timer>()+7;
    page.begin_job();
    let outcome=page.dispatch(&mut doc,&mut forms,node,"click",true);
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert!(page.report().contains("retained host metadata"),"{}",page.report());
    assert!(page.state.event.is_none());
    assert_eq!(page.state.timers.len(),1);
    assert_eq!(page.state.timer_bytes,payload);
    assert_eq!(page.state.retained,baseline+payload+std::mem::size_of::<Timer>());
    page.state.retained_limit=limit;
    page.state.clear_timers();
    assert_eq!(page.state.retained,baseline);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),0.0);
    click(&mut page,&mut doc,&mut forms,node);
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),2.0);
}
