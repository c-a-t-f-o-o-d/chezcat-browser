//! Original capacity and storage acceptance for all four native target stores.
use super::*;
fn page()->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><head><title>waiting</title></head><body>Targets</body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());
    (doc,forms,page)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
#[test]
fn browser_target_registries_exceed_former_shared_count_and_keep_real_dispatch_abort_media_and_messages() {
    let(mut doc,mut forms,mut page)=page();
    execute(&mut page,&mut doc,&mut forms,"var target,controller,channel,media,hits=0;for(var i=0;i<70000;i++)target=new EventTarget();");
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<70000;i++)controller=new AbortController();");
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<70000;i++)channel=new MessageChannel();");
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<70000;i++)media=matchMedia('all');if(media.media!=='all'||!media.matches||!(media instanceof EventTarget))throw 'media registry';target.addEventListener('notice',function(){hits++;});target.dispatchEvent(new Event('notice'));controller.signal.addEventListener('abort',function(){hits++;});controller.abort('real reason');if(!controller.signal.aborted||controller.signal.reason!=='real reason')throw 'signal registry';channel.port2.onmessage=function(event){if(event.data!=='real data')throw 'message data';hits++;document.title='large target registries delivered';};channel.port1.postMessage('real data');");
    assert_eq!(page.state.standalone_targets.len(),70000);
    assert_eq!(page.state.abort_signals.len(),70000);
    assert_eq!(page.state.message_channels.port_count(),140000);
    assert_eq!(page.state.media_queries.len(),70000);
    let result=page.poll_timer_at(&mut doc,&mut forms,Instant::now()+Duration::from_secs(1)).unwrap();
    assert_eq!(result.errors,0,"{}",page.report());
    assert_eq!(execute(&mut page,&mut doc,&mut forms,"hits;").to_number(),3.0);
    assert_eq!(doc.title,"large target registries delivered");
    assert!(page.state.retained<=page.state.retained_limit);
}
#[derive(Clone,Copy,Debug)]
enum Kind {Target,Abort,Channel,Media}
fn create(kind:Kind,doc:&mut Document,forms:&mut Forms,page:&mut Page)->Result<Value,String> {
    let mut bridge=Bridge{doc,forms,state:&mut page.state};
    match kind {
        Kind::Target=>bridge.create_standalone_target(WINDOW),
        Kind::Abort=>bridge.create_abort_record(WINDOW,true,Value::Undefined).map(Value::Host),
        Kind::Channel=>bridge.message_call(WINDOW,crate::javascript::message_channel_interface::CHANNEL,&[]),
        Kind::Media=>bridge.create_media_query(&[Value::String("all".into())]),
    }
}
fn facts(page:&Page,kind:Kind)->(usize,usize,usize) {
    match kind {
        Kind::Target=>(page.state.standalone_targets.len(),page.state.standalone_targets.capacity(),size_of::<u32>()),
        Kind::Abort=>(page.state.abort_signals.len(),page.state.abort_signals.capacity(),size_of::<script_abort::Record>()),
        Kind::Channel=>page.state.message_channels.registry_storage(),
        Kind::Media=>(page.state.media_queries.len(),page.state.media_queries.capacity(),size_of::<String>()),
    }
}
#[test]
fn browser_target_registry_storage_and_growth_copy_refusal_preserve_each_previous_store() {
    for kind in [Kind::Target,Kind::Abort,Kind::Channel,Kind::Media] {
        let(mut doc,mut forms,mut page)=page();let baseline=page.state.retained;let limit=page.state.retained_limit;
        page.state.retained_limit=baseline;
        assert!(create(kind,&mut doc,&mut forms,&mut page).is_err(),"{kind:?}");
        assert_eq!(page.state.retained,baseline,"{kind:?}");assert_eq!(facts(&page,kind).0,0);assert_eq!(facts(&page,kind).1,0);
        page.state.retained_limit=limit;page.begin_job();
        create(kind,&mut doc,&mut forms,&mut page).unwrap();
        while facts(&page,kind).0<facts(&page,kind).1 {create(kind,&mut doc,&mut forms,&mut page).unwrap();}
        let before=(page.state.retained,facts(&page,kind));
        let extra=if matches!(kind,Kind::Media){9}else{0};
        page.state.copy_work=before.1.0*before.1.2+extra-1;
        assert!(create(kind,&mut doc,&mut forms,&mut page).unwrap_err().contains("string work limit"),"{kind:?}");
        assert_eq!((page.state.retained,facts(&page,kind)),before,"{kind:?}");assert_eq!(page.state.copy_work,0);
        page.begin_job();create(kind,&mut doc,&mut forms,&mut page).unwrap();assert_eq!(facts(&page,kind).0,before.1.0+1);
    }
}
#[test]
fn browser_target_single_complete_record_fits_without_a_forced_geometric_allocation() {
    for kind in [Kind::Target,Kind::Abort,Kind::Channel] {
        let(mut doc,mut forms,mut page)=page();let baseline=page.state.retained;let slot=facts(&page,kind).2;
        page.state.retained_limit=baseline+slot-1;
        assert!(create(kind,&mut doc,&mut forms,&mut page).is_err(),"{kind:?}");
        assert_eq!(facts(&page,kind).1,0);
        page.state.retained_limit=baseline+slot;
        create(kind,&mut doc,&mut forms,&mut page).unwrap();
        assert_eq!(facts(&page,kind).1,1,"{kind:?}");assert_eq!(page.state.retained,baseline+slot);
    }
}
