//! Page/API pause controls. Physical Clock/worker publication has App tests.
use super::*;

fn page() -> (Document, Forms, Page, usize) {
    let mut doc=crate::html::parse("<html><body><video id='v' src='scene-intra.ivf'></video><video id='unbound'></video></body></html>");
    doc.scripting_enabled=true;
    let node=crate::embedded_video::authored_node(&doc).unwrap();
    let mut forms=Forms::new(&doc);
    let mut p=Page::with_limits(crate::javascript::limits::BROWSER);
    let base=crate::navigation::explicit(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/authored-video.html")).unwrap();
    p.set_location(&base);p.set_resource_base(&base);
    p.set_embedded_video(Some(crate::embedded_video::Snapshot {
        tasks:crate::media_tasks::shared(crate::javascript::limits::BROWSER.host.media_tasks),
        node,retired:Arc::new(AtomicBool::new(false)),current_src:"previous".into(),
        width:64,height:48,duration:Some(Duration::from_secs(3)),
        position:Duration::from_millis(750),observed:Instant::now(),
        playing:true,paused:false,load_pending:false,ended:false,seeking:false,ready:4,network:1,
    }));
    let out=p.start(&mut doc,&mut forms,&[]);assert_eq!(out.errors,0,"{}",p.report());
    eval(&mut p,&mut doc,&mut forms,"var video=document.getElementById('v');");
    (doc,forms,p,node)
}
fn eval(p:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let out=p.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{source}\n{}",p.report());
}
// Acknowledge only the Page/App handoff; this does not claim a physical pause.
fn task(p:&mut Page,doc:&mut Document,forms:&mut Forms)->(PauseRequest,Outcome) {
    assert!(p.media_pause().is_some_and(|r|!r.applied));
    p.mark_media_pause_applied();
    let request=p.take_media_pause_task().unwrap();assert!(!request.retired.load(Ordering::Relaxed));
    let out=p.dispatch_media_task(doc,forms,request.node,MediaTask::Pause);
    (request,out)
}
fn unchanged(p:&Page,old:&crate::embedded_video::Snapshot) {
    let s=p.state.embedded_video.as_ref().unwrap();
    assert!(Arc::ptr_eq(&s.retired,&old.retired));assert!(!s.retired.load(Ordering::Relaxed));
    assert_eq!((s.position,s.observed,s.playing,s.paused),(old.position,old.observed,old.playing,old.paused));
    assert_eq!((s.duration,s.ready,s.network),(old.duration,old.ready,old.network));
    assert_eq!(s.current_src,old.current_src);
    assert!(p.media_pause().is_none());assert!(!p.has_media_selection());assert!(!p.runtime.has_microtasks());
}

#[test]
fn native_media_pause_identity_brand_ignored_arguments_and_frozen_idempotence() {
    let(mut doc,mut forms,mut p,node)=page();
    let old=p.state.embedded_video.as_ref().unwrap().clone();let commits=p.state.dom_commits;let next=p.state.media_changes.next;
    eval(&mut p,&mut doc,&mut forms,r#"
var method=video.pause,attribute=video.getAttributeNode('src'),raw=attribute.value,coercions=0;
if(typeof method!=='function'||method!==video.pause||method.length!==0||!('pause' in video))throw 'method identity';
var ignored={toString:function(){coercions++;throw Error('coerced');},valueOf:function(){coercions++;throw Error('coerced value');}},brands=0;
for(var receiver of [{},document.body,document.getElementById('unbound'),null,undefined,17]){
 try{method.call(receiver,ignored);}catch(e){if(e instanceof TypeError)brands++;else throw e;}
}
try{var saved=method;saved(ignored);}catch(e){if(e instanceof TypeError)brands++;else throw e;}
try{new method(ignored);}catch(e){if(e instanceof TypeError)brands++;else throw e;}
if(brands!==8||coercions!==0||video.paused)throw 'brand effects';
if(video.pause(ignored,Symbol('ignored'))!==undefined)throw 'result';
var frozen=video.currentTime;
if(!video.paused||frozen<0.75||frozen>3||video.currentTime!==frozen||video.seeking||video.ended)throw 'sync frozen state';
if(method.call(video,ignored)!==undefined||method.apply(video,[ignored])!==undefined||method.bind(video,ignored)()!==undefined)throw 'saved native';
if(coercions!==0||video.currentTime!==frozen||video.getAttributeNode('src')!==attribute||attribute.value!==raw||video.currentSrc!=='previous'||video.duration!==3||video.readyState!==4||video.networkState!==1)throw 'retention/idempotence';
"#);
    let r=p.media_pause().unwrap();assert_eq!(r.node,node);assert!(Arc::ptr_eq(&r.retired,&old.retired));
    assert_eq!(r.position,old.position_at(r.issued));assert!(!r.applied);
    let s=p.state.embedded_video.as_ref().unwrap();assert!(s.paused&&!s.playing);
    assert_eq!((s.position,s.observed),(r.position,r.issued));
    assert_eq!(s.position_at(r.issued+Duration::from_secs(100)),r.position);
    assert_eq!(p.state.media_changes.next,next+1);assert_eq!(p.state.dom_commits,commits);
    assert_eq!(doc.attr(node,"src"),Some("scene-intra.ivf"));
}

#[test]
fn native_media_pause_small_policy_ticket_atomicity_and_recovery() {
    for overflow in [false,true] {
        let(mut doc,mut forms,mut p,node)=page();let old=p.state.embedded_video.as_ref().unwrap().clone();
        if overflow{p.state.media_changes.next=u64::MAX;}else{p.state.limits.host.media_tasks=0;}
        let next=p.state.media_changes.next;let commits=p.state.dom_commits;
        let out=p.console_eval(&mut doc,&mut forms,"video.pause();");assert_eq!(out.errors,1,"{}",p.report());
        unchanged(&p,&old);assert_eq!(p.state.media_changes.next,next);assert_eq!(p.state.dom_commits,commits);
        assert_eq!(doc.attr(node,"src"),Some("scene-intra.ivf"));
        if overflow {
            // No counter rewind: this exhausted document stays exhausted.
            let out=p.console_eval(&mut doc,&mut forms,"video.pause();");assert_eq!(out.errors,1);unchanged(&p,&old);
            let(mut d,mut f,mut fresh,_)=self::page();eval(&mut fresh,&mut d,&mut f,"video.pause();if(!video.paused)throw 'new document recovery';");
            assert!(fresh.media_pause().is_some());
        }else{
            p.state.limits.host.media_tasks=1;eval(&mut p,&mut doc,&mut forms,"video.pause();if(!video.paused)throw 'policy recovery';");assert!(p.media_pause().is_some());
        }
    }
}

#[test]
fn native_media_pause_empty_invokes_actual_reserved_selection() {
    let(mut doc,mut forms,mut p,_)=page();let s=p.state.embedded_video.as_mut().unwrap();let old=s.retired.clone();
    s.network=0;s.ready=0;s.width=0;s.height=0;s.duration=None;s.position=Duration::ZERO;s.playing=false;s.paused=true;
    eval(&mut p,&mut doc,&mut forms,r#"
var attr=video.getAttributeNode('src'),raw=attr.value,order='';
queueMicrotask(function(){if(video.networkState!==3||!video.paused)throw 'EMPTY sync reset';order+='before;';});
if(video.pause()!==undefined||!video.paused||video.currentTime!==0||video.getAttributeNode('src')!==attr||attr.value!==raw)throw 'EMPTY pause';
queueMicrotask(function(){if(video.networkState!==2||video.currentSrc.indexOf('scene-intra.ivf')<0)throw 'stable selection';order+='after;';});
"#);
    assert!(old.load(Ordering::Relaxed));assert!(p.media_pause().is_none());
    let selection=p.take_media_selection().unwrap();assert_eq!(selection.raw.as_deref(),Some("scene-intra.ivf"));
    assert!(selection.uri.as_ref().is_some_and(|uri|uri.contains("scene-intra.ivf")));
    assert!(!selection.abort&&!selection.emptied&&!selection.timeupdate);
    eval(&mut p,&mut doc,&mut forms,"if(order!=='before;after;')throw 'real selection FIFO';");
}

#[test]
fn native_media_pause_started_pair_keeps_tail_after_listener_or_microtask_load() {
    for micro in [false,true] {
        let(mut doc,mut forms,mut p,_)=page();
        let change=if micro{"queueMicrotask(function(){order+='micro;';reload();});order+='listener;'"}else{"reload();"};
        eval(&mut p,&mut doc,&mut forms,&format!(r#"
var order='',attribute=video.getAttributeNode('src'),raw=attribute.value;
function reload(){{
 video.load();if(video.readyState!==0||video.currentTime!==0||!video.paused||!isNaN(video.duration))throw 'load reset';
 order+='reset;';queueMicrotask(function(){{if(video.networkState!==2||video.getAttributeNode('src')!==attribute||attribute.value!==raw)throw 'stable load';order+='after;';}});
}}
video.addEventListener('timeupdate',function(e){{if(e.target!==video||!e.isTrusted||e.cancelable||e.bubbles)throw 'identity';order+='time;';{change}}});
video.addEventListener('pause',function(e){{if(e.target!==video||!e.isTrusted||e.cancelable||e.bubbles||video.readyState!==0||video.currentTime!==0||!video.paused||!isNaN(video.duration)||video.networkState!==2)throw 'started tail state';order+='pause;';}});
video.pause();if(order!=='')throw 'synchronous events';
"#));
        let(request,out)=task(&mut p,&mut doc,&mut forms);assert_eq!(out.errors,0,"{}",p.report());
        assert!(request.retired.load(Ordering::Relaxed));
        let selection=p.take_media_selection().unwrap();assert_eq!(selection.raw.as_deref(),Some("scene-intra.ivf"));
        assert!(!Arc::ptr_eq(&request.retired,&selection.retired));
        let expected=if micro{"time;listener;micro;reset;after;pause;"}else{"time;reset;after;pause;"};
        eval(&mut p,&mut doc,&mut forms,&format!("if(order!=='{expected}')throw 'started order '+order;"));
        assert!(p.media_pause().is_none());
    }
}

#[test]
fn native_media_pause_ordinary_error_continues_but_fatal_job_stops_tail_and_recovers() {
    // ordinary Error; capability-fatal callback; capability-fatal microtask.
    for mode in 0..3 {
        let(mut doc,mut forms,mut p,_)=page();
        let body=match mode {
            0=>"queueMicrotask(function(){order+='micro;';});throw Error('ordinary listener');",
            1=>"queueMicrotask(function(){stale++;});document.body.scrollWidth;",
            _=>"queueMicrotask(function(){order+='fatal;';queueMicrotask(function(){stale++;});document.body.scrollWidth;});",
        };
        eval(&mut p,&mut doc,&mut forms,&format!(r#"
var order='',pauses=0,stale=0;
function listener(){{order+='time;';{body}}}
function pauseListener(){{pauses++;order+='pause;';}}
video.addEventListener('timeupdate',listener);video.addEventListener('pause',pauseListener);video.pause();
"#));
        let(_,out)=task(&mut p,&mut doc,&mut forms);assert_eq!(out.errors,1,"{}",p.report());
        assert_eq!(p.runtime.invocation_failed_fatally(),mode!=0);assert!(!p.runtime.has_microtasks());
        let(expected,pauses)=match mode{0=>("time;micro;pause;",1),1=>("time;",0),_=>("time;fatal;",0)};
        eval(&mut p,&mut doc,&mut forms,&format!("if(order!=='{expected}'||pauses!=={pauses}||stale!==0)throw 'unfinished tail/stale jobs';video.removeEventListener('timeupdate',listener);video.removeEventListener('pause',pauseListener);"));
        let s=p.state.embedded_video.as_mut().unwrap();s.paused=false;s.playing=true;s.position=Duration::from_secs(1);s.observed=Instant::now();
        eval(&mut p,&mut doc,&mut forms,"var fresh=0;video.addEventListener('pause',function(){fresh++;});video.pause();");
        let(_,out)=task(&mut p,&mut doc,&mut forms);assert_eq!(out.errors,0,"{}",p.report());
        eval(&mut p,&mut doc,&mut forms,"if(fresh!==1||stale!==0)throw 'fresh task recovery';");
    }
}

#[test]
fn native_media_pause_source_retirement_discards_real_unstarted_request() {
    let(mut doc,mut forms,mut p,_)=page();
    eval(&mut p,&mut doc,&mut forms,"var events=0;video.addEventListener('timeupdate',function(){events++;});video.addEventListener('pause',function(){events++;});video.pause();");
    let old=p.media_pause().unwrap().retired.clone();
    eval(&mut p,&mut doc,&mut forms,"video.load();if(video.readyState!==0||!video.paused||video.currentTime!==0)throw 'load reset';");
    assert!(old.load(Ordering::Relaxed));assert!(p.media_pause().is_none());
    p.mark_media_pause_applied();assert!(p.take_media_pause_task().is_none());
    assert!(p.take_media_selection().is_some());
    eval(&mut p,&mut doc,&mut forms,"if(events!==0)throw 'unstarted pair escaped';");
}

#[test]
fn native_media_pause_saved_operation_respects_disabled_and_foreign_callers() {
    let(mut doc,mut forms,mut p,_)=page();
    eval(&mut p,&mut doc,&mut forms,"var savedPause=video.pause;");
    let old=p.state.embedded_video.as_ref().unwrap().clone();
    p.state.enabled=false;
    // The normal Page entry refuses disabled documents entirely. Invoke the
    // initialized runtime directly here to verify the saved native's defensive
    // gate as well, without bypassing the real native method implementation.
    let refused=p.console_eval(&mut doc,&mut forms,"savedPause.call(video);");
    assert_eq!(refused.errors,1);
    p.begin_job();
    let result=p.runtime.execute("var refusal='';try{savedPause.call(video);}catch(e){refusal=e.name;}if(refusal!=='InvalidStateError')throw 'disabled saved operation';",
        &mut Bridge {doc:&mut doc,forms:&mut forms,state:&mut p.state});
    assert!(result.is_ok(),"{result:?}");
    unchanged(&p,&old);
    p.state.enabled=true;
    let web=crate::navigation::address("https://example.test/page",&crate::navigation::Location::home()).unwrap();
    p.set_location(&web);
    eval(&mut p,&mut doc,&mut forms,"var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var foreignCall=f.contentWindow.Function('fn','v','return fn.call(v);');");
    p.set_location(&crate::navigation::address("https://other.test/page",&web).unwrap());
    let out=p.console_eval(&mut doc,&mut forms,"foreignCall(savedPause,video);");
    assert_eq!(out.errors,1,"{}",p.report());
    unchanged(&p,&old);
    eval(&mut p,&mut doc,&mut forms,"savedPause.call(video);if(!video.paused)throw 'native owner recovery';");
    assert!(p.media_pause().is_some());
}

#[test]
fn native_media_current_time_is_official_and_stable_within_job_and_microtasks() {
    let(mut doc,mut forms,mut p,_)=page();
    // Real elapsed physical time advances between jobs. The script itself and
    // its queued checkpoint see one official sample without relying on sleeps.
    eval(&mut p,&mut doc,&mut forms,"var official=video.currentTime;for(var i=0;i<1000;i++){if(video.currentTime!==official)throw 'moving official position';}queueMicrotask(function(){if(video.currentTime!==official)throw 'moving checkpoint position';});");
    let before=p.state.media_changes.official_position;
    let snapshot=p.state.embedded_video.as_mut().unwrap();
    snapshot.position=Duration::from_millis(1750);
    snapshot.observed=Instant::now();
    eval(&mut p,&mut doc,&mut forms,"if(video.currentTime<1.75||video.currentTime===official)throw 'new stable state did not refresh';video.pause();var frozenOfficial=video.currentTime;if(!video.paused||video.currentTime!==frozenOfficial)throw 'pause official update';");
    assert!(p.state.media_changes.official_position>before);
    assert_eq!(p.state.media_changes.official_position,p.media_pause().unwrap().position.as_secs_f64());
}

#[test]
fn native_media_time_finite_conversion_projects_a_real_source_bound_seek_control() {
    let(mut doc,mut forms,mut p,node)=page();
    let source=p.state.embedded_video.as_ref().unwrap().retired.clone();
    eval(&mut p,&mut doc,&mut forms,"var attr=video.getAttributeNode('src'),raw=attr.value;video.currentTime={valueOf:function(){return 1.25;}};if(video.currentTime!==1.25||!video.seeking||video.paused||video.getAttributeNode('src')!==attr||attr.value!==raw)throw 'seek projection/identity';");
    let tasks=p.media_tasks().unwrap();let mut tasks=tasks.lock().unwrap();
    assert_eq!(tasks.count_tasks(),0,"seeking/completion tasks are not fabricated at setter issue");
    let(_,scope,control)=tasks.take_control().unwrap();
    assert_eq!(scope.node,node);assert!(Arc::ptr_eq(&scope.retired,&source));
    assert!(matches!(control,crate::media_tasks::Control::Seek {seconds} if seconds==1.25));
    assert!(!tasks.has_pending());
}

#[test]
fn native_media_time_nonfinite_or_throwing_conversion_and_admission_are_atomic() {
    for expression in ["NaN","Infinity","-Infinity","Symbol('time')","({valueOf:function(){throw Error('conversion');}})"] {
        let(mut doc,mut forms,mut p,_)=page();
        let old=p.state.embedded_video.as_ref().unwrap().clone();let next=p.state.media_changes.next;
        eval(&mut p,&mut doc,&mut forms,&format!("var caught=0;try{{video.currentTime={expression};}}catch(e){{caught++;}}if(caught!==1||video.seeking||video.paused)throw 'conversion refusal';"));
        unchanged(&p,&old);assert_eq!(p.state.media_changes.next,next);
    }
    for overflow in [false,true] {
        let(mut doc,mut forms,mut p,_)=page();let old=p.state.embedded_video.as_ref().unwrap().clone();
        if overflow {p.state.media_changes.next=u64::MAX;} else {p.state.limits.host.media_tasks=0;}
        let out=p.console_eval(&mut doc,&mut forms,"video.currentTime=2;");
        assert_eq!(out.errors,1,"{}",p.report());unchanged(&p,&old);
    }
}

#[test]
fn native_media_time_default_start_is_signed_and_metadata_resets_both_signs() {
    for seconds in [-2.5,0.0,2.5,f64::MAX] {
        let(mut doc,mut forms,mut p,_)=page();
        let s=p.state.embedded_video.as_mut().unwrap();s.ready=0;s.duration=None;s.position=Duration::ZERO;s.playing=false;s.paused=true;
        let source=s.retired.clone();
        eval(&mut p,&mut doc,&mut forms,&format!("video.currentTime={seconds};if(video.currentTime!=={seconds}||video.seeking||!video.paused)throw 'signed default';"));
        assert!(!p.media_tasks().unwrap().lock().unwrap().has_pending());
        let default=p.take_media_default_start(&source);
        assert_eq!(default,if seconds>0.0 {Some(seconds)}else{None});
        assert_eq!(p.state.media_changes.default_start,0.0);
    }
    let(mut doc,mut forms,mut p,_)=page();
    eval(&mut p,&mut doc,&mut forms,"video.currentTime={valueOf:function(){video.load();return 1.5;}};if(video.currentTime!==1.5||video.readyState!==0||video.seeking)throw 'conversion source reentry';");
    assert_eq!(p.state.media_changes.default_start,1.5);
    assert!(p.take_media_selection().is_some());
    assert!(!p.media_tasks().unwrap().lock().unwrap().has_pending());
}
