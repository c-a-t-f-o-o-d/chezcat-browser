//! Real Page/Runtime Promise/task tests. Queue control acknowledgement below is
//! a handoff witness only; physical playback, Clock and pixels have App tests.
use super::*;
use crate::media_tasks::{Control, Scope};

// These are explicit Page observations, not an audio-device acknowledgement
// emulator. Actual App/worker acceptance and matching pixels have separate tests.
fn sampled_page() -> (Document,Forms,Page,usize) {
    let (doc,forms,mut p,node)=page(false);
    p.state.embedded_video.as_mut().unwrap().clock=crate::embedded_video::ClockKind::AcknowledgedPcm;
    p.notify_user_activation();
    (doc,forms,p,node)
}

#[test]
fn native_media_play_sampled_clock_waits_for_ack_and_frame_once() {
    let(mut doc,mut forms,mut p,_)=sampled_page();
    eval(&mut p,&mut doc,&mut forms,r#"
var order='';for(var name of ['play','waiting','playing'])video.addEventListener(name,function(e){order+=e.type+';';});
var a=video.play(),b=video.play();a.then(function(){order+='a;';},function(){throw 'a rejection';});b.then(function(){order+='b;';},function(){throw 'b rejection';});
if(video.paused||video.readyState!==4||order!=='')throw 'honest admission';
"#);
    assert_eq!(p.pending_media_play_count(),2);
    let owner=scope(&p);assert!(matches!(acknowledge(&p,&owner).as_slice(),[Control::Play {notify_queued:false,..}]));
    assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),1);
    let mut s=p.state.embedded_video.as_ref().unwrap().clone();
    p.set_embedded_video(Some(s.clone()));assert_eq!(p.pending_media_play_count(),2);
    s.playing=true;s.frame_ready=false;p.set_embedded_video(Some(s.clone()));assert_eq!(p.pending_media_play_count(),2);
    s.frame_ready=true;s.seeking=true;p.set_embedded_video(Some(s.clone()));assert_eq!(p.pending_media_play_count(),2);
    s.seeking=false;p.set_embedded_video(Some(s.clone()));assert_eq!(p.pending_media_play_count(),0);
    p.set_embedded_video(Some(s));assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),1);
    eval(&mut p,&mut doc,&mut forms,"if(order!=='play;playing;a;b;')throw 'sample gate '+order;");
    assert!(!p.has_media_tasks());assert_eq!(p.state.media_play.retained_count(),0);
}

#[test]
fn native_media_play_sampled_clock_pause_before_ack_aborts_without_playing() {
    let(mut doc,mut forms,mut p,_)=sampled_page();
    eval(&mut p,&mut doc,&mut forms,r#"
var order='';for(var name of ['play','waiting','playing','timeupdate','pause'])video.addEventListener(name,function(e){order+=e.type+';';});
var a=video.play();a.then(function(){throw 'unacknowledged fulfillment';},function(e){if(e.name!=='AbortError')throw 'reason';order+='abort;';});video.pause();
if(!video.paused||video.readyState!==4||order!=='')throw 'pause projection';
"#);
    let owner=scope(&p);assert!(matches!(acknowledge(&p,&owner).as_slice(),[Control::Play {..},Control::Pause {..}]));
    assert_eq!(p.pending_media_play_count(),0);
    // A late physical sample while logical pause remains true cannot create a
    // new playing batch or rewrite the already captured AbortError.
    let mut s=p.state.embedded_video.as_ref().unwrap().clone();s.playing=true;
    p.set_embedded_video(Some(s));assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),2);
    eval(&mut p,&mut doc,&mut forms,"if(order!=='play;timeupdate;pause;abort;')throw 'pause/ack '+order;");
    assert_eq!(p.state.media_play.retained_count(),0);assert!(!p.has_media_tasks());
}

#[test]
fn native_media_play_sampled_clock_load_aborts_before_new_source_ack() {
    let(mut doc,mut forms,mut p,_)=sampled_page();let old=scope(&p);
    eval(&mut p,&mut doc,&mut forms,r#"
var settled='',playing=0;video.addEventListener('playing',function(){playing++;});
var a=video.play();a.then(function(){throw 'old source fulfilled';},function(e){if(e.name!=='AbortError')throw 'reason';settled='aborted';});video.load();
"#);
    assert!(old.retired.load(Ordering::Relaxed));assert!(p.take_media_selection().is_some());
    eval(&mut p,&mut doc,&mut forms,"if(settled!=='aborted'||playing!==0)throw 'load rejection';");
    let mut fresh=p.state.embedded_video.as_ref().unwrap().clone();
    assert!(!Arc::ptr_eq(&fresh.retired,&old.retired));
    fresh.clock=crate::embedded_video::ClockKind::AcknowledgedPcm;
    fresh.width=64;fresh.height=48;
    fresh.duration=Some(Duration::from_secs(3));fresh.ready=4;fresh.network=1;
    fresh.frame_ready=true;fresh.playing=true;fresh.paused=false;fresh.load_pending=false;
    p.set_embedded_video(Some(fresh.clone()));p.set_embedded_video(Some(fresh));
    dispatch_ok(&mut p,&mut doc,&mut forms);
    eval(&mut p,&mut doc,&mut forms,"if(settled!=='aborted'||playing!==0)throw 'new source rewrote old outcome';");
    assert_eq!(p.state.media_play.retained_count(),0);assert_eq!(p.pending_media_play_count(),0);
}

#[test]
fn native_media_play_sampled_clock_ack_capture_stays_fulfilled_before_pause() {
    let(mut doc,mut forms,mut p,_)=sampled_page();
    eval(&mut p,&mut doc,&mut forms,"var order='';video.addEventListener('playing',function(){order+='playing;';});var a=video.play();a.then(function(){order+='fulfilled;';},function(){throw 'captured outcome';});");
    let owner=scope(&p);acknowledge(&p,&owner);dispatch_ok(&mut p,&mut doc,&mut forms);
    let mut s=p.state.embedded_video.as_ref().unwrap().clone();s.playing=true;
    p.set_embedded_video(Some(s));assert_eq!(p.pending_media_play_count(),0);
    eval(&mut p,&mut doc,&mut forms,"video.pause();if(order!=='')throw 'synchronous settlement';");
    assert!(matches!(acknowledge(&p,&owner).as_slice(),[Control::Pause {..}]));
    assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),2);
    eval(&mut p,&mut doc,&mut forms,"if(order!=='playing;fulfilled;')throw 'captured ack '+order;");
    assert_eq!(p.state.media_play.retained_count(),0);
}

fn page(loading: bool) -> (Document, Forms, Page, usize) {
    let limits=crate::javascript::limits::BROWSER;
    let mut doc=crate::html::parse("<html><body><video id='v' src='scene-intra.ivf'></video><video id='unbound'></video></body></html>");
    doc.scripting_enabled=true;
    let node=crate::embedded_video::authored_node(&doc).unwrap();
    let mut forms=Forms::new(&doc);
    let mut p=Page::with_limits(limits);
    let base=crate::navigation::explicit(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/authored-video.html")).unwrap();
    p.set_location(&base);p.set_resource_base(&base);
    p.set_embedded_video(Some(crate::embedded_video::Snapshot {
        tasks:crate::media_tasks::shared(limits.host.media_tasks),
        node,retired:Arc::new(AtomicBool::new(false)),current_src:"previous".into(),
        width:if loading{0}else{64},height:if loading{0}else{48},
        duration:if loading{None}else{Some(Duration::from_secs(3))},
        position:Duration::ZERO,observed:Instant::now(),playing:false,paused:true,
        clock:crate::embedded_video::ClockKind::WallTime,
        load_pending:loading,frame_ready:!loading,error_code:None,ended:false,seeking:false,
        ready:if loading{0}else{4},network:if loading{2}else{1},
    }));
    let out=p.start(&mut doc,&mut forms,&[]);assert_eq!(out.errors,0,"{}",p.report());
    eval(&mut p,&mut doc,&mut forms,"var video=document.getElementById('v');");
    (doc,forms,p,node)
}
fn eval(p:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let out=p.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{source}\n{}",p.report());
}
fn scope(p:&Page)->Scope {
    let s=p.state.embedded_video.as_ref().unwrap();Scope {node:s.node,retired:s.retired.clone()}
}
// No Player is driven here. Mark only accepted source-bound queue controls as
// acknowledged so real Page native tasks may run under their normal boundary.
fn acknowledge(p:&Page,owner:&Scope)->Vec<Control> {
    let shared=p.media_tasks().unwrap();let mut q=shared.lock().unwrap();let mut controls=Vec::new();
    while let Some((_,actual,control))=q.take_control() {
        assert_eq!(actual.node,owner.node);assert!(Arc::ptr_eq(&actual.retired,&owner.retired));
        assert!(!actual.retired.load(Ordering::Relaxed));controls.push(control);
    }
    controls
}
fn dispatch_initial(p:&mut Page,doc:&mut Document,forms:&mut Forms)->Vec<(MediaTask,Outcome)> {
    let shared=p.media_tasks().unwrap();let budget=shared.lock().unwrap().count_tasks();let mut outcomes=Vec::new();
    for _ in 0..budget {
        let Some((_,owner,task))=shared.lock().unwrap().take_task() else {break;};
        assert!(!owner.retired.load(Ordering::Relaxed));
        let current=scope(p);assert_eq!(owner.node,current.node);assert!(Arc::ptr_eq(&owner.retired,&current.retired));
        let out=p.dispatch_media_task(doc,forms,owner.node,task);outcomes.push((task,out));
    }
    outcomes
}
fn dispatch_ok(p:&mut Page,doc:&mut Document,forms:&mut Forms)->usize {
    let outcomes=dispatch_initial(p,doc,forms);
    for (task,out) in &outcomes {assert_eq!(out.errors,0,"{task:?}: {}",p.report());}
    outcomes.len()
}
fn ready_snapshot(p:&mut Page) {
    let mut s=p.state.embedded_video.as_ref().unwrap().clone();
    s.width=64;s.height=48;s.duration=Some(Duration::from_secs(3));s.load_pending=false;
    s.frame_ready=true;s.ready=4;s.network=1;s.playing=!s.paused;s.observed=Instant::now();
    p.set_embedded_video(Some(s));
}

#[test]
fn native_media_play_real_rejections_brand_arity_and_ignored_arguments() {
    let(mut doc,mut forms,mut p,_)=page(false);let old=p.state.embedded_video.as_ref().unwrap().clone();
    eval(&mut p,&mut doc,&mut forms,r#"
var method=video.play,coercions=0,denied=0,wrong=0,synchronous=0;
if(typeof method!=='function'||method!==video.play||method.length!==0||!('play' in video))throw 'identity/arity';
var ignored={toString:function(){coercions++;throw 'coerced';},valueOf:function(){coercions++;throw 'coerced value';}};
var refusal=video.play(ignored,Symbol('ignored'));
if(!(refusal instanceof Promise)||Object.prototype.toString.call(refusal)!=='[object Promise]')throw 'real denied promise';
refusal.catch(function(e){if(!(e instanceof DOMException)||e.name!=='NotAllowedError'||e.code!==0)throw 'denial type';denied++;});
for(var receiver of [{},document.body,document.getElementById('unbound'),null,undefined,17]){
 var bad;try{bad=method.call(receiver,ignored);}catch(e){synchronous++;}
 if(!(bad instanceof Promise))throw 'brand did not return Promise';
 bad.catch(function(e){if(!(e instanceof TypeError))throw 'brand reason';wrong++;});
}
var saved=method;try{var unbound=saved(ignored);unbound.catch(function(e){if(!(e instanceof TypeError))throw 'unbound reason';wrong++;});}catch(e){synchronous++;}
try{new method(ignored);}catch(e){if(!(e instanceof TypeError))throw e;synchronous++;}
if(coercions!==0||synchronous!==1||!video.paused)throw 'coercion/synchronous brand effects';
"#);
    eval(&mut p,&mut doc,&mut forms,"if(denied!==1||wrong!==7||coercions!==0)throw 'real rejections';");
    let s=p.state.embedded_video.as_ref().unwrap();assert!(Arc::ptr_eq(&s.retired,&old.retired));
    assert_eq!((s.position,s.observed,s.playing,s.paused,s.ready,s.network),(old.position,old.observed,old.playing,old.paused,old.ready,old.network));
    assert!(!p.has_media_tasks()&&!p.has_media_selection()&&!p.has_media_promise_effects());
    assert_eq!(p.state.media_play.retained_count(),0);
}

#[test]
fn native_media_play_first_child_lookup_preserves_owner_intrinsics_after_replacement() {
    let(mut doc,mut forms,mut p,_)=page(false);
    let origin=crate::navigation::Location {url:Some(crate::url::parse("https://example.test/media/page").unwrap().0),path:None,root:None,fragment:String::new()};
    p.set_location(&origin);p.set_resource_base(&origin);
    eval(&mut p,&mut doc,&mut forms,r#"
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow;
var RootFunction=Function,RootPromise=Promise,RootDom=DOMException,ChildFunction=w.Function,ChildPromise=w.Promise,ChildDom=w.DOMException;
var read=ChildFunction('x','return x.play;'),invoke=ChildFunction('fn','x','return fn.call(x);');
ChildFunction('Promise=29;DOMException=31;Function=37;')();Promise=17;DOMException=19;Function=23;
var method=read(video),denied=invoke(method,video),denial=0,fulfilled=0;
if(method!==video.play||Object.getPrototypeOf(method)!==RootFunction.prototype||Object.getPrototypeOf(method)===ChildFunction.prototype)throw 'first read owner Function';
if(Object.getPrototypeOf(denied)!==RootPromise.prototype||denied instanceof ChildPromise)throw 'owner denied Promise';
denied.catch(function(e){if(!(e instanceof RootDom)||e instanceof ChildDom||Object.getPrototypeOf(e)!==RootDom.prototype||e.name!=='NotAllowedError')throw 'owner DOMException';denial++;});
"#);
    p.notify_user_activation();
    eval(&mut p,&mut doc,&mut forms,r#"
if(denial!==1)throw 'deferred owner denial';
var accepted=invoke(method,video);
if(Object.getPrototypeOf(accepted)!==RootPromise.prototype||accepted instanceof ChildPromise||video.paused)throw 'owner accepted Promise';
accepted.then(function(value){if(value!==undefined)throw 'fulfill value';fulfilled++;});
"#);
    let owner=scope(&p);assert!(matches!(acknowledge(&p,&owner).as_slice(),[Control::Play {..}]));
    assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),2);
    eval(&mut p,&mut doc,&mut forms,"if(fulfilled!==1||denial!==1)throw 'owner settlement';");
    assert_eq!(p.state.media_play.retained_count(),0);
}

#[test]
fn native_media_play_distinct_ready_promises_then_pause_keep_captured_fulfillment() {
    let(mut doc,mut forms,mut p,_)=page(false);p.notify_user_activation();
    eval(&mut p,&mut doc,&mut forms,r#"
var order='',coercions=0,ignored={toString:function(){coercions++;throw 'ignored';}};
for(var name of ['play','playing','timeupdate','pause'])video.addEventListener(name,function(e){if(e.target!==video||!e.isTrusted||e.bubbles||e.cancelable)throw 'event';order+=e.type+';';});
var a=video.play(ignored),b=video.play(Symbol('ignored'));
if(a===b||!(a instanceof Promise)||!(b instanceof Promise)||video.paused||order!=='')throw 'distinct/admission';
a.then(function(v){if(v!==undefined)throw 'a value';order+='a;';},function(){throw 'a rejected';});
b.then(function(v){if(v!==undefined)throw 'b value';order+='b;';},function(){throw 'b rejected';});
video.pause();if(!video.paused||coercions!==0||order!=='')throw 'sync pause';
"#);
    let owner=scope(&p);let controls=acknowledge(&p,&owner);
    assert!(matches!(controls.as_slice(),[Control::Play {..},Control::Pause {..}]));
    assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),4);
    eval(&mut p,&mut doc,&mut forms,"if(order!=='play;playing;a;b;timeupdate;pause;')throw 'captured outcome '+order;");
    assert_eq!(p.state.media_play.retained_count(),0);assert!(!p.has_media_tasks());
}

#[test]
fn native_media_play_loading_pause_group_keeps_abort_after_started_load_or_microtask() {
    for mode in 0..3 {
        let(mut doc,mut forms,mut p,_)=page(true);p.notify_user_activation();let owner=scope(&p);
        let reload=match mode {0=>"",1=>"video.load();order+='load;';",_=>"queueMicrotask(function(){video.load();order+='load;';});"};
        eval(&mut p,&mut doc,&mut forms,&format!(r#"
var order='';
video.addEventListener('play',function(){{order+='play;';}});video.addEventListener('waiting',function(){{order+='waiting;';}});
video.addEventListener('timeupdate',function(){{order+='time;';{reload}}});
video.addEventListener('pause',function(){{order+='pause;';if(!video.paused)throw 'tail state';}});
var pending=video.play();pending.then(function(){{throw 'loading fulfilled';}},function(e){{if(!(e instanceof DOMException)||e.name!=='AbortError'||e.code!==20)throw 'abort reason';order+='abort;';}});
video.pause();if(order!==''||!video.paused)throw 'synchronous notifications';
"#));
        assert_eq!(p.pending_media_play_count(),0);
        assert!(matches!(acknowledge(&p,&owner).as_slice(),[Control::Play {..},Control::Pause {..}]));
        assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),3);
        let expected=if mode==0{"play;waiting;time;pause;abort;"}else{"play;waiting;time;load;pause;abort;"};
        eval(&mut p,&mut doc,&mut forms,&format!("if(order!=='{expected}')throw 'group order '+order;"));
        if mode!=0 {assert!(owner.retired.load(Ordering::Relaxed));assert!(p.take_media_selection().is_some());}
        assert_eq!(p.state.media_play.retained_count(),0);assert!(!p.has_media_promise_effects());
    }
}

#[test]
fn native_media_play_load_performs_queued_original_outcomes_before_stable_selection_fifo() {
    let(mut doc,mut forms,mut p,_)=page(true);p.notify_user_activation();
    eval(&mut p,&mut doc,&mut forms,r#"
var order='',events=0;
video.addEventListener('play',function(){events++;});video.addEventListener('playing',function(){events++;});video.addEventListener('pause',function(){events++;});
var a=video.play();a.then(function(){throw 'a should abort';},function(e){if(e.name!=='AbortError'||video.networkState!==3)throw 'queued abort';order+='abort;';});
video.pause();
var b=video.play();b.then(function(){if(video.networkState!==3)throw 'queued fulfillment';order+='fulfill;';},function(){throw 'b rewritten to reject';});
"#);
    // Actual Page observation path: metadata/frame readiness arrives after the
    // second loading request. This acknowledges no decoder or physical frame.
    assert_eq!(p.pending_media_play_count(),1);ready_snapshot(&mut p);assert_eq!(p.pending_media_play_count(),0);
    let owner=scope(&p);
    eval(&mut p,&mut doc,&mut forms,r#"
queueMicrotask(function(){if(video.networkState!==3)throw 'earlier microtask';order+='before;';});
video.load();if(order!==''||video.networkState!==3)throw 'sync load boundary';
queueMicrotask(function(){if(video.networkState!==2)throw 'stable source';order+='after;';});
"#);
    eval(&mut p,&mut doc,&mut forms,"if(order!=='before;abort;fulfill;after;'||events!==0)throw 'load outcome FIFO '+order;");
    assert!(owner.retired.load(Ordering::Relaxed));assert!(p.take_media_selection().is_some());
    assert!(!p.has_media_tasks()&&!p.has_media_promise_effects());assert_eq!(p.state.media_play.retained_count(),0);
}

#[test]
fn native_media_play_producer_fatal_keeps_admitted_control_and_real_pending_promise() {
    let(mut doc,mut forms,mut p,_)=page(true);p.notify_user_activation();let owner=scope(&p);
    let out=p.console_eval(&mut doc,&mut forms,r#"
var stale=0,fresh=0,events='';
for(var name of ['play','waiting','playing'])video.addEventListener(name,function(e){events+=e.type+';';});
var accepted=video.play();accepted.then(function(){stale++;},function(){stale++;});
queueMicrotask(function(){stale++;});document.body.scrollWidth;
"#);
    assert_eq!(out.errors,1,"{}",p.report());assert!(p.runtime.invocation_failed_fatally());
    assert_eq!(p.pending_media_play_count(),1);assert!(!p.runtime.has_microtasks());
    assert!(matches!(acknowledge(&p,&owner).as_slice(),[Control::Play {..}]));
    eval(&mut p,&mut doc,&mut forms,"accepted.then(function(){fresh++;},function(){throw 'accepted control falsely aborted';});");
    ready_snapshot(&mut p);assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),3);
    eval(&mut p,&mut doc,&mut forms,"if(stale!==0||fresh!==1||events!=='play;waiting;playing;')throw 'producer fatal cleanup '+events;");
    assert_eq!(p.state.media_play.retained_count(),0);
}

#[test]
fn native_media_play_ordinary_error_continues_but_started_fatal_defers_native_cleanup() {
    for fatal in [false,true] {
        let(mut doc,mut forms,mut p,_)=page(false);p.notify_user_activation();
        let body=if fatal{"queueMicrotask(function(){stale++;});document.body.scrollWidth;"}else{"queueMicrotask(function(){order+='micro;';});throw Error('ordinary playing listener');"};
        eval(&mut p,&mut doc,&mut forms,&format!(r#"
var order='',stale=0,fulfilled=0,fresh=0;
video.addEventListener('playing',function(){{order+='playing;';{body}}});
var accepted=video.play();accepted.then(function(){{fulfilled++;order+='fulfilled;';}},function(){{throw 'captured play changed outcome';}});
"#));
        let owner=scope(&p);acknowledge(&p,&owner);let outcomes=dispatch_initial(&mut p,&mut doc,&mut forms);
        assert_eq!(outcomes.len(),2);assert_eq!(outcomes.iter().map(|(_,out)|out.errors).sum::<usize>(),1,"{}",p.report());
        assert_eq!(p.runtime.invocation_failed_fatally(),fatal);assert!(!p.runtime.has_microtasks());
        if fatal {
            assert!(p.has_media_promise_effects());
            eval(&mut p,&mut doc,&mut forms,"if(fulfilled!==0||stale!==0||order!=='playing;')throw 'fatal callbacks';accepted.then(function(){fresh++;order+='fresh;';},function(){throw 'false fatal AbortError';});");
            let out=p.run_media_promise_cleanup(&mut doc,&mut forms);assert_eq!(out.errors,0,"{}",p.report());
            eval(&mut p,&mut doc,&mut forms,"if(fresh!==1||fulfilled!==0||stale!==0||order!=='playing;fresh;')throw 'fresh settlement';");
        } else {eval(&mut p,&mut doc,&mut forms,"if(fulfilled!==1||stale!==0||order!=='playing;micro;fulfilled;')throw 'ordinary continuation';");}
        assert!(!p.has_media_promise_effects());assert_eq!(p.state.media_play.retained_count(),0);
    }
}

#[test]
fn native_media_play_small_capacity_atomic_refusal_preserves_state_and_recovers() {
    for limit in [0,1] {
        let(mut doc,mut forms,mut p,_)=page(false);p.notify_user_activation();p.set_media_task_capacity_for_test(limit);
        let old=p.state.embedded_video.as_ref().unwrap().clone();let commits=p.state.dom_commits;
        let out=p.console_eval(&mut doc,&mut forms,"var after=0;video.play();after=1;");
        assert_eq!(out.errors,1,"{}",p.report());assert!(p.runtime.invocation_failed_fatally());
        let s=p.state.embedded_video.as_ref().unwrap();assert!(Arc::ptr_eq(&s.retired,&old.retired));
        assert_eq!((s.paused,s.playing,s.position,s.observed,s.ready,s.network),(old.paused,old.playing,old.position,old.observed,old.ready,old.network));
        assert_eq!(p.state.dom_commits,commits);assert_eq!(p.state.media_play.retained_count(),0);
        assert!(!p.has_media_tasks()&&!p.has_media_selection()&&!p.has_media_promise_effects());
        p.set_media_task_capacity_for_test(4);
        eval(&mut p,&mut doc,&mut forms,"if(after!==0)throw 'fatal continued';var fresh=0;video.play().then(function(){fresh++;});");
        let owner=scope(&p);acknowledge(&p,&owner);assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),2);
        eval(&mut p,&mut doc,&mut forms,"if(fresh!==1)throw 'capacity recovery';");
    }
    let(mut doc,mut forms,mut p,_)=page(true);p.notify_user_activation();p.set_media_task_capacity_for_test(2);
    eval(&mut p,&mut doc,&mut forms,"var a=video.play(),b=video.play();");assert_eq!(p.pending_media_play_count(),2);
    let before=p.state.embedded_video.as_ref().unwrap().clone();
    let out=p.console_eval(&mut doc,&mut forms,"video.play();");assert_eq!(out.errors,1,"{}",p.report());
    assert_eq!(p.pending_media_play_count(),2);assert_eq!(p.state.media_play.retained_count(),2);
    assert_eq!(p.media_tasks().unwrap().lock().unwrap().len(),2);
    let s=p.state.embedded_video.as_ref().unwrap();assert_eq!((s.paused,s.playing,s.position,s.observed),(before.paused,before.playing,before.position,before.observed));
    p.set_media_task_capacity_for_test(4);
    eval(&mut p,&mut doc,&mut forms,"var rejected=0;for(var item of [a,b])item.catch(function(e){if(e.name!=='AbortError')throw e;rejected++;});video.pause();");
    let owner=scope(&p);acknowledge(&p,&owner);assert_eq!(dispatch_ok(&mut p,&mut doc,&mut forms),3);
    eval(&mut p,&mut doc,&mut forms,"if(rejected!==2)throw 'preserved accepted calls';");assert_eq!(p.state.media_play.retained_count(),0);
}
