//! Original primitive native-read controls. No child navigation, rendering or IO.
use super::*;

fn start(source:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><head></head><body><p>Original caller-read fixture</p></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::for_test();
    page.set_location(&location("https://example.test/parent/page"));
    let source=format!("var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow;{source}");
    let outcome=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original caller-aware native reads".into(),source}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    (doc,forms,page)
}

fn location(source:&str)->crate::navigation::Location {
    crate::navigation::Location {url:Some(crate::url::parse(source).unwrap().0),path:None,root:None,fragment:String::new()}
}

fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let outcome=page.console_eval(doc,forms,source);
    assert_eq!(outcome.errors,0,"{source}: {}",page.report());
}

fn diverge(page:&mut Page) {page.set_location(&location("https://other.test/changed"));}

#[test]
fn child_native_reads_follow_actual_own_script_window_and_document_after_divergence() {
    let(mut doc,mut forms,mut page)=start(r#"
var rootWindow=window,rootDoc=document,rootLocation=location,ChildFunction=w.Function;
var readOwn=ChildFunction('return window===self&&window===globalThis&&window.document===document&&document.defaultView===window&&document.body.nodeType===1&&document.body.nodeName==="BODY"&&document.body.ownerDocument===document&&document.documentElement.nodeName==="HTML"&&document.head.nodeName==="HEAD"&&document.location===location&&location.href==="about:blank"&&location.protocol==="about:"&&window.length===0&&window.origin==="https://example.test";');
if(!readOwn()||w.window!==w||w.document.defaultView!==w||w.document.body.nodeName!=='BODY'||w.location.href!=='about:blank')throw 'same-origin control';
var directUI=ChildFunction('var event=new UIEvent("child-owned",{detail:37,view:window});return event.detail===37&&event.view===window&&event instanceof UIEvent;');
"#);
    diverge(&mut page);
    evaluate(&mut page,&mut doc,&mut forms,"if(!readOwn()||!directUI())throw 'real own-child reads and UI view';if(window!==rootWindow||document!==rootDoc||location!==rootLocation||document.body.nodeName!=='BODY')throw 'main owner restored';");
    assert_eq!(page.state.blank_frames.len(),1);
}

#[test]
fn child_native_foreign_window_document_and_location_reads_stay_refused_and_recover() {
    let(mut doc,mut forms,mut page)=start(r#"
var childDoc=w.document,childBody=childDoc.body,childLocation=w.location,rootDoc=document,rootLocation=location;
var childRead=w.Function('value','field','return value[field];'),own=w.Function('return window===self&&document.body.nodeType===1&&location.href==="about:blank";');
if(childRead(rootDoc,'body')!==document.body||childRead(rootLocation,'href')!==location.href)throw 'same-origin foreign owner control';
"#);
    diverge(&mut page);
    for source in ["w.window;","w.document;","childDoc.body;","childBody.nodeType;","childLocation.href;","childRead(rootDoc,'body');","childRead(rootLocation,'href');"] {
        // Compare each denied operation with its immediate entry state. The
        // legitimate recovery below can materialize a main Node backing store.
        let allocations=page.runtime.window_allocation_state_for_test();
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"{source}: {}",page.report());
        assert!(page.report().contains("same-origin"),"{source}: {}",page.report());
        assert_eq!(page.runtime.window_allocation_state_for_test(),allocations,"refused read allocated: {source}");
        evaluate(&mut page,&mut doc,&mut forms,"if(!own()||document.body.nodeType!==1||window!==self)throw 'later owner recovery';");
    }
}

#[test]
fn child_native_super_receiver_refuses_foreign_window_before_base_callbacks_and_recovers() {
    let(mut doc,mut forms,mut page)=start(r#"
var reads=0,writes=0,calls=0;
class Base {
 get value(){reads++;return this;}
 set value(input){writes++;}
 method(){calls++;return this;}
}
class Derived extends Base {
 read(){return super.value;}
 write(input){super.value=input;}
 invoke(){return super.method();}
}
var read=Derived.prototype.read,write=Derived.prototype.write,invoke=Derived.prototype.invoke;
if(read.call(window)!==window||invoke.call(window)!==window)throw 'own-window borrowed super receiver';
write.call(window,17);
var own=w.Function('return window===self&&document.body.nodeType===1&&location.href==="about:blank";');
"#);
    // Foreign Window object operations remain a capability refusal even before
    // origin divergence; only the existing named native reads are supported.
    for changed_origin in [false,true] {
        if changed_origin {diverge(&mut page);}
        let refusal=if changed_origin {"requires a verified same-origin web context"}
            else {"foreign Window operation is not implemented"};
        for source in ["read.call(w);","write.call(w,19);","invoke.call(w);"] {
            page.clear_console();
            let allocations=page.runtime.window_allocation_state_for_test();
            let outcome=page.console_eval(&mut doc,&mut forms,source);
            assert_eq!(outcome.errors,1,"{source}: {}",page.report());
            assert!(page.report().contains(refusal),"{source}: {}",page.report());
            assert_eq!(page.runtime.window_allocation_state_for_test(),allocations,"refused super receiver allocated: {source}");
            evaluate(&mut page,&mut doc,&mut forms,"if(reads!==1||writes!==1||calls!==1||!own()||window!==self||document.body.nodeType!==1)throw 'super refusal effects or owner recovery';");
        }
    }
}

#[test]
fn child_native_event_fresh_cached_saved_guards_precede_coercion_after_divergence() {
    let(mut doc,mut forms,mut page)=start(r#"
var event=new UIEvent('kept',{detail:29,view:window}),get=Object.getOwnPropertyDescriptor(UIEvent.prototype,'detail').get,init=event.initUIEvent;
var ChildC=w.UIEvent,ChildFunction=w.Function;
var read=ChildFunction('e','return e.initUIEvent;'),getSaved=ChildFunction('e','g','return g.call(e);'),initSaved=ChildFunction('e','f','t','return f.call(e,t);');
var converted=0,type={toString:function(){converted++;return 'bad';}},options={get detail(){return {valueOf:function(){converted++;return 31;}};},get view(){converted++;return window;}};
var own=ChildFunction('var e=new UIEvent("own",{view:window});return e.view===window&&document.body.nodeType===1;');
"#);
    diverge(&mut page);
    let count=page.state.created_events.len();
    let next=page.state.next_event;
    let allocations=page.runtime.window_allocation_state_for_test();
    for source in ["new w.UIEvent(type,options);","new ChildC(type,options);","read(event);","getSaved(event,get);","initSaved(event,init,type);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"{source}: {}",page.report());
        assert!(page.report().contains("same-origin"),"{source}: {}",page.report());
        assert_eq!(page.state.created_events.len(),count);
        assert_eq!(page.state.next_event,next);
        assert_eq!(page.runtime.window_allocation_state_for_test(),allocations);
        evaluate(&mut page,&mut doc,&mut forms,"if(converted||event.type!=='kept'||event.detail!==29||event.view!==window)throw 'foreign attempt changed slots or coerced';");
    }
    evaluate(&mut page,&mut doc,&mut forms,"if(!own())throw 'own constructor after all foreign failures';");
}

#[test]
fn child_native_borrowed_native_getter_reentry_and_throw_restore_actual_script_owners() {
    let(mut doc,mut forms,mut page)=start(r#"
var rootWindow=window,rootDoc=document,rootMap=Array.prototype.map,marker={};
function parentRead(){return window===rootWindow&&document===rootDoc&&document.body.nodeType===1&&location.href==='https://other.test/changed';}
var nested=w.Function('map','parentRead','marker',`
 var own=window,log='',source={get length(){log+='L';if(window!==own||document.defaultView!==own)throw 'length owner';return 1;},get 0(){log+='I';if(location.href!=='about:blank')throw 'index owner';return document.body.nodeType;}};
 var result=map.call(source,function(value){log+='C';if(window!==own||self!==own||!parentRead())throw 'nested script owner';return value+1;});
 if(log!=='LIC'||result[0]!==2||window!==own)throw 'borrowed native/getter/callback order';
 var rejected={get length(){if(window!==own||document.body.nodeType!==1)throw 'throwing getter owner';throw marker;}};
 try{map.call(rejected,function(){throw 'unreachable callback';});throw 'missing throw';}catch(e){if(e!==marker)throw e;}
 return window===own&&document.body.ownerDocument===document&&parentRead();
`);
"#);
    diverge(&mut page);
    evaluate(&mut page,&mut doc,&mut forms,"if(!nested(rootMap,parentRead,marker)||!parentRead())throw 'borrowed native restoration';");
    evaluate(&mut page,&mut doc,&mut forms,"if(!nested(rootMap,parentRead,marker)||window!==rootWindow||document!==rootDoc)throw 'independent job restoration';");
}

#[test]
fn child_native_read_preserves_policy_and_capability_refusals_and_following_jobs() {
    let(mut doc,mut forms,mut page)=start(r#"
var compile=w.Function('try{Function("return window;");throw "missing eval policy";}catch(e){if(Object.getPrototypeOf(e)!==EvalError.prototype)throw e;}return window===self&&document.body.nodeType===1;');
var unsupported=w.Function('return window.frames;'),own=w.Function('return window===self&&location.href==="about:blank";');
"#);
    page.state.blank_frames[0].policy.add_header("script-src 'unsafe-inline'");
    diverge(&mut page);
    evaluate(&mut page,&mut doc,&mut forms,"if(!compile())throw 'inherited child policy bypass';");
    let outcome=page.console_eval(&mut doc,&mut forms,"unsupported();");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert!(page.report().contains("not implemented"),"{}",page.report());
    evaluate(&mut page,&mut doc,&mut forms,"if(!own()||window!==self||document.body.nodeType!==1)throw 'recovery after capability refusal';");
}

#[test]
fn child_native_explicit_host_read_keeps_internal_default_and_single_work_charge() {
    let(mut doc,mut forms,mut page)=start("");
    diverge(&mut page);
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.state.host_work=10;
    assert!(matches!(bridge.get_with_caller(Some(window(0)),window(0),"window"),Ok(Value::Host(handle)) if handle==window(0)));
    assert_eq!(bridge.state.host_work,9);
    assert!(bridge.get(window(0),"window").is_err());
    assert_eq!(bridge.state.host_work,8);
    assert!(bridge.get_with_caller(Some(WINDOW),window(0),"window").is_err());
    assert_eq!(bridge.state.host_work,7);
    bridge.state.host_work=0;
    assert!(bridge.get_with_caller(Some(window(0)),window(0),"window").is_err());
    bridge.state.host_work=1;
    assert!(matches!(bridge.get_with_caller(Some(window(0)),window(0),"window"),Ok(Value::Host(handle)) if handle==window(0)));
    assert_eq!(bridge.state.host_work,0);
}
