//! Original Page controls for callback interfaces on native and scripted events.
use super::*;

fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<html><body><div id=desk><button id=send type=button>Send</button></div></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = "var b=document.getElementById('send'),desk=document.getElementById('desk');";
    let result = page.start(&mut doc, &mut forms, &[Script {
        node: 0, label: "Original listener objects".into(), source: source.into(),
    }]);
    assert_eq!(result.errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn eval(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    assert_eq!(page.console_eval(doc, forms, source).errors, 0, "{source}\n{}", page.report());
}

fn click(page: &mut Page, doc: &mut Document, forms: &mut Forms) -> Outcome {
    let button = (0..doc.nodes.len()).find(|&id| doc.attr(id, "id") == Some("send")).unwrap();
    page.begin_job();
    page.dispatch(doc, forms, button, "click", true)
}

#[test]
fn listener_object_registration_validates_callback_before_options_without_getting_handle_event() {
    let (mut doc, mut forms, mut page) = fixture();
    eval(&mut page, &mut doc, &mut forms, r#"
        var trace='',lookups=0;
        var type={toString:function(){trace+='T';return 'attendance';}};
        var ticket={get capture(){trace+='C';return false;},get once(){trace+='O';return false;},
            get passive(){trace+='P';return true;},get signal(){trace+='S';}};
        window.addEventListener(type,ticket,ticket);
        if(trace!=='TCOPS')throw 'add did not read the full options once';
    "#);
    assert_eq!(page.state.listeners.len(), 1);
    eval(&mut page, &mut doc, &mut forms, r#"
        window.removeEventListener(type,ticket,ticket);
        if(trace!=='TCOPSTC')throw 'remove did not read capture alone';
        var unread={get handleEvent(){lookups++;throw 'eager operation lookup';}};
        window.addEventListener('unused',unread);
        window.removeEventListener('unused',unread);
        if(lookups)throw 'registration read handleEvent';
        function invalid(callback){
            trace='';
            var caught=0;
            try{window.addEventListener(type,callback,ticket);}catch(e){if(e instanceof TypeError)caught++;}
            try{window.removeEventListener(type,callback,ticket);}catch(e){if(e instanceof TypeError)caught++;}
            if(trace!=='TT'||caught!==2)throw 'invalid callback conversion order';
        }
        invalid(19);invalid(false);invalid('desk');invalid(Symbol('desk'));
        trace='';
        window.addEventListener(type,null,ticket);window.removeEventListener(type,undefined,ticket);
        if(trace!=='TCOPSTC')throw 'nullish callbacks skipped dictionary conversion';
        var marker={},escaped=false;
        try{window.addEventListener(type,unread,{get capture(){throw marker;}});}
        catch(e){escaped=e===marker;}
        if(!escaped||lookups)throw 'abrupt options conversion';
    "#);
    assert!(page.state.listeners.is_empty());
}

#[test]
fn listener_object_native_click_resolves_current_inherited_handler_and_preserves_event_identity() {
    let (mut doc, mut forms, mut page) = fixture();
    eval(&mut page, &mut doc, &mut forms, r#"
        var trace='',lookups=0,seen=null,valid=true;
        desk.addEventListener('click',function(e){seen=e;trace+='C';},true);
        var proto={get handleEvent(){lookups++;if(this!==listener)valid=false;return selected;}};
        var listener=Object.create(proto);
        function inspect(e){
            return e===seen&&e.target===b&&e.currentTarget===b&&e.type==='click'&&e.eventPhase===2&&e.isTrusted;
        }
        function cancel(e){trace+='A';if(this!==listener||!inspect(e))valid=false;e.preventDefault();}
        function replacement(e){trace+='B';if(this!==listener||!inspect(e))valid=false;return false;}
        var selected=cancel;
        b.addEventListener('click',listener);
        b.addEventListener('click',function(e){trace+=e.defaultPrevented?'Y':'N';});
        if(lookups!==0)throw 'premature inherited lookup';
    "#);
    let first = click(&mut page, &mut doc, &mut forms);
    assert_eq!(first.errors, 0, "{}", page.report());
    assert!(first.prevented);
    eval(&mut page, &mut doc, &mut forms,
        "if(trace!=='CAY'||lookups!==1||!valid)throw 'first native delivery';selected=replacement;");
    let second = click(&mut page, &mut doc, &mut forms);
    assert_eq!(second.errors, 0, "{}", page.report());
    assert!(!second.prevented);
    eval(&mut page, &mut doc, &mut forms,
        "if(trace!=='CAYCBN'||lookups!==2||!valid)throw 'live method or listener return value';");
}

#[test]
fn listener_object_synchronous_dispatch_keeps_once_capture_identity_and_snapshot_removals() {
    let (mut doc, mut forms, mut page) = fixture();
    eval(&mut page, &mut doc, &mut forms, r#"
        var event=document.createEvent('Event');event.initEvent('parcel',true,true);
        var trace='',lookups=0,retiredCalls=0;
        var retired={handleEvent:function(){retiredCalls++;}};
        var observer={get handleEvent(){lookups++;return function(e){
            if(this!==observer||e!==event||e.target!==b||e.currentTarget!==desk||e.isTrusted)
                throw 'scripted operation receiver or event';
            trace+=e.eventPhase;
            if(e.eventPhase===1){
                e.preventDefault();
                if(e.defaultPrevented)throw 'duplicate registration changed passive';
            } else {
                desk.removeEventListener('parcel',retired);
            }
        };}};
        desk.addEventListener('parcel',observer,{capture:true,once:true,passive:true});
        desk.addEventListener('parcel',observer,{capture:true,once:false,passive:false});
        desk.addEventListener('parcel',observer,false);
        desk.addEventListener('parcel',retired,false);
        b.addEventListener('parcel',function(e){if(e!==event||this!==b)throw 'function receiver';trace+='T';});
        if(lookups)throw 'eager operation lookup';
    "#);
    assert_eq!(page.state.listeners.len(), 4);
    eval(&mut page, &mut doc, &mut forms, r#"
        if(!b.dispatchEvent(event)||trace!=='1T3'||lookups!==2||retiredCalls!==0)throw 'first dispatch';
        desk.removeEventListener('parcel',observer,true);
        if(!b.dispatchEvent(event)||trace!=='1T3T3'||lookups!==3)throw 'capture removal or once';
        desk.removeEventListener('parcel',observer,false);
        if(!b.dispatchEvent(event)||trace!=='1T3T3T'||lookups!==3||retiredCalls!==0||event.currentTarget!==null)
            throw 'object delivery after removal';
    "#);
    assert_eq!(page.state.listeners.len(), 1);
}

#[test]
fn listener_object_reports_getter_method_and_noncallable_errors_without_skipping_later_listeners() {
    for native in [false, true] {
        let (mut doc, mut forms, mut page) = fixture();
        eval(&mut page, &mut doc, &mut forms, r#"
            var trace='',dispatched=false,synthetic=document.createEvent('Event');synthetic.initEvent('click',true,true);
            b.addEventListener('click',{});
            b.addEventListener('click',{handleEvent:37});
            b.addEventListener('click',{get handleEvent(){trace+='G';throw 'lookup failure';}});
            b.addEventListener('click',{handleEvent:function(){trace+='M';throw 'method failure';}});
            b.addEventListener('click',function(e){trace+='T';if(e.target!==b||e.currentTarget!==b)throw 'lost target';});
            if(trace!=='')throw 'registration ran user code';
        "#);
        let result = if native {
            click(&mut page, &mut doc, &mut forms)
        } else {
            page.console_eval(&mut doc, &mut forms, "dispatched=b.dispatchEvent(synthetic);")
        };
        assert_eq!(result.errors, 4, "native={native}\n{}", page.report());
        assert!(page.state.synthetic_dispatches.is_empty());
        eval(&mut page, &mut doc, &mut forms, "if(trace!=='GMT')throw 'later listener omitted';");
        if !native {
            eval(&mut page, &mut doc, &mut forms,
                "if(!dispatched||synthetic.eventPhase!==0||synthetic.currentTarget!==null)throw 'dispatch cleanup';");
        }
    }
}

#[test]
fn listener_object_fatal_getter_keeps_dispatch_and_microtask_limits() {
    for native in [false, true] {
        let (mut doc, mut forms, mut page) = fixture();
        eval(&mut page, &mut doc, &mut forms, r#"
            var lookups=0,tail=0,queued=0,ev=document.createEvent('Event');ev.initEvent('click',true,true);
            b.addEventListener('click',{get handleEvent(){
                lookups++;queueMicrotask(function(){queued++;});document.body.scrollWidth;
            }},{once:true});
            b.addEventListener('click',function(){tail++;});
        "#);
        let result = if native {
            click(&mut page, &mut doc, &mut forms)
        } else {
            page.console_eval(&mut doc, &mut forms, "b.dispatchEvent(ev);")
        };
        assert_eq!(result.errors, 1, "native={native}\n{}", page.report());
        assert!(!page.runtime.has_microtasks());
        assert!(page.state.synthetic_dispatches.is_empty());
        assert_eq!(page.state.listeners.len(), 1);
        eval(&mut page, &mut doc, &mut forms, r#"
            if(lookups!==1||tail!==0||queued!==0||ev.currentTarget!==null||ev.eventPhase!==0)throw 'fatal cleanup';
            if(!b.dispatchEvent(ev)||tail!==1||lookups!==1||queued!==0)throw 'next job recovery';
        "#);
    }
}

#[test]
fn listener_object_survives_function_slot_pressure_while_only_host_registration_holds_it() {
    let (mut doc, mut forms, mut page) = fixture();
    eval(&mut page, &mut doc, &mut forms, r#"
        var answer='';
        (function(word){
            var receiver={get handleEvent(){return function(e){
                if(this!==receiver||e.target!==b||!e.isTrusted)throw 'lost object or event';
                answer=word;
            };}};
            b.addEventListener('click',receiver,{once:true});
        })('survived');
        function churn(){
            var temporary={};for(var i=0;i<750;i++){temporary.current=function(){return i;};}
        }
    "#);
    // Each call creates 750 fresh closures while the page's function capacity is 1024.
    eval(&mut page, &mut doc, &mut forms, "churn();");
    eval(&mut page, &mut doc, &mut forms, "churn();");
    let result = click(&mut page, &mut doc, &mut forms);
    assert_eq!(result.errors, 0, "{}", page.report());
    eval(&mut page, &mut doc, &mut forms, "if(answer!=='survived')throw 'host-only listener was lost';");
    assert!(page.state.listeners.is_empty());
}

#[test]
fn listener_object_duplicate_at_capacity_consumes_no_slot_and_removal_allows_replacement() {
    let (mut doc, mut forms, mut page) = fixture();
    page.state.limits.host.listeners = 2;
    eval(&mut page, &mut doc, &mut forms, r#"
        var first={},second={},third={};
        b.addEventListener('notice',first);
        b.addEventListener('notice',second);
    "#);
    let serials=page.state.listeners.iter().map(|listener|listener.serial).collect::<Vec<_>>();
    let retained = page.state.retained;
    eval(&mut page, &mut doc, &mut forms, "b.addEventListener('notice',first,{once:true});");
    assert_eq!(page.state.listeners.len(), 2);
    assert_eq!(page.state.listeners.iter().map(|listener|listener.serial).collect::<Vec<_>>(),serials);
    let rejected = page.console_eval(&mut doc, &mut forms, "b.addEventListener('notice',third);");
    assert_eq!(rejected.errors, 1, "{}", page.report());
    assert_eq!(page.state.listeners.iter().map(|listener|listener.serial).collect::<Vec<_>>(),serials);
    assert_eq!(page.state.retained, retained);
    eval(&mut page, &mut doc, &mut forms, "b.removeEventListener('notice',first);b.addEventListener('notice',third);");
    assert_eq!(page.state.listeners.len(), 2);
    assert_eq!(page.state.listeners.iter().map(|listener|listener.serial).collect::<Vec<_>>(),vec![serials[1],serials[0]]);
    assert_eq!(page.state.retained,retained);
}
