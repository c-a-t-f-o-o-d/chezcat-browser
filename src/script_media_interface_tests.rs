//! Original Page/Runtime interface controls. Queue acknowledgement is a handoff
//! witness only; this module never claims physical playback, decode or device IO.
use super::*;

fn fixture()->(Document,Forms,Page,usize) {
    let mut doc=crate::html::parse("<html><head></head><body><video id=v src='Scene-First.ivf' controls width=160 height=120></video><video id=unbound></video><audio id=audio></audio><p id=p>Original media interface fixture</p></body></html>");
    doc.scripting_enabled=true;
    let node=crate::embedded_video::authored_node(&doc).unwrap();
    let mut forms=Forms::new(&doc);
    let limits=crate::javascript::limits::BROWSER;
    let mut page=Page::with_limits(limits);
    let source=crate::navigation::Location {url:Some(crate::url::parse("https://example.test/media/page").unwrap().0),path:None,root:None,fragment:String::new()};
    page.set_location(&source);page.set_resource_base(&source);
    page.set_embedded_video(Some(crate::embedded_video::Snapshot {
        tasks:crate::media_tasks::shared(limits.host.media_tasks),node,
        retired:Arc::new(AtomicBool::new(false)),current_src:"https://example.test/media/Selected.ivf".into(),
        width:64,height:48,duration:Some(Duration::from_secs(3)),position:Duration::from_millis(750),
        observed:Instant::now(),clock:crate::embedded_video::ClockKind::WallTime,
        playing:false,paused:true,load_pending:false,frame_ready:true,error_code:None,
        ended:false,seeking:false,ready:4,network:1,
    }));
    let outcome=page.start(&mut doc,&mut forms,&[]);assert_eq!(outcome.errors,0,"{}",page.report());
    evaluate(&mut page,&mut doc,&mut forms,"var video=document.getElementById('v'),unbound=document.getElementById('unbound'),M=HTMLMediaElement,V=HTMLVideoElement,MP=M.prototype,VP=V.prototype;");
    (doc,forms,page,node)
}

fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let outcome=page.console_eval(doc,forms,source);
    assert_eq!(outcome.errors,0,"{source}\n{}",page.report());
}

#[test]
fn media_interface_real_roots_descriptors_constants_and_illegal_constructors() {
    let(mut doc,mut forms,mut page,_)=fixture();
    evaluate(&mut page,&mut doc,&mut forms,r#"
if(typeof M!=='function'||typeof V!=='function'||M.name!=='HTMLMediaElement'||V.name!=='HTMLVideoElement'||M.length!==0||V.length!==0)throw 'genuine interface objects';
if(Object.getPrototypeOf(M)!==HTMLElement||Object.getPrototypeOf(V)!==M||Object.getPrototypeOf(MP)!==HTMLElement.prototype||Object.getPrototypeOf(VP)!==MP)throw 'real interface inheritance';
if(Object.getPrototypeOf(video)!==VP||video.constructor!==V||!(video instanceof V)||!(video instanceof M)||!(video instanceof HTMLElement)||!(video instanceof EventTarget))throw 'real bound element ancestry';
if(Object.prototype.toString.call(video)!=='[object HTMLVideoElement]')throw 'real Video tag';
for(var entry of [['NETWORK_EMPTY',0],['NETWORK_IDLE',1],['NETWORK_LOADING',2],['NETWORK_NO_SOURCE',3],['HAVE_NOTHING',0],['HAVE_METADATA',1],['HAVE_CURRENT_DATA',2],['HAVE_FUTURE_DATA',3],['HAVE_ENOUGH_DATA',4]]){
 var name=entry[0],value=entry[1];for(var holder of [M,MP]){var d=Object.getOwnPropertyDescriptor(holder,name);if(!d||d.value!==value||!d.enumerable||d.writable||d.configurable)throw 'real constant '+name;}
 if(video[name]!==value)throw 'inherited constant';
}
for(var name of ['load','pause','play']){var d=Object.getOwnPropertyDescriptor(MP,name);if(typeof d.value!=='function'||d.value!==video[name]||d.value.length!==0||!d.enumerable||!d.writable||!d.configurable||Object.prototype.hasOwnProperty.call(video,name)||Object.getOwnPropertyDescriptor(VP,name)!==undefined)throw 'canonical Media method '+name;}
for(var name of ['currentSrc','duration','paused','ended','seeking','readyState','networkState']){var d=Object.getOwnPropertyDescriptor(MP,name);if(typeof d.get!=='function'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||Object.prototype.hasOwnProperty.call(video,name))throw 'canonical Media getter '+name;}
for(var name of ['src','currentTime']){var d=Object.getOwnPropertyDescriptor(MP,name);if(typeof d.get!=='function'||typeof d.set!=='function'||d.get.length!==0||d.set.length!==1||!d.enumerable||!d.configurable)throw 'existing mutable attribute '+name;}
for(var name of ['videoWidth','videoHeight']){var d=Object.getOwnPropertyDescriptor(VP,name);if(typeof d.get!=='function'||d.set!==undefined||!d.enumerable||!d.configurable||Object.getOwnPropertyDescriptor(MP,name)!==undefined)throw 'canonical Video getter '+name;}
var effects=0,arg={toString:function(){effects++;throw 'illegal constructor coerced';}},rejected=0;
for(var action of [function(){M(arg);},function(){V(arg);},function(){new M(arg);},function(){new V(arg);}]){try{action();}catch(e){if(e.name==='TypeError')rejected++;else throw e;}}
if(rejected!==4||effects)throw 'illegal native HTML constructors';
if(Object.getOwnPropertyDescriptor(MP,'canPlayType')!==undefined||Object.getOwnPropertyDescriptor(MP,'volume')!==undefined||Object.getOwnPropertyDescriptor(VP,'width')!==undefined||Object.getOwnPropertyDescriptor(VP,'height')!==undefined||video.getAttribute('width')!=='160')throw 'deferred members must not alias existing fields';
"#);
    assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
}

#[test]
fn media_interface_getters_follow_real_snapshot_and_ordinary_shadows_leave_slots_intact() {
    let(mut doc,mut forms,mut page,_)=fixture();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var getTime=Object.getOwnPropertyDescriptor(MP,'currentTime').get,getPaused=Object.getOwnPropertyDescriptor(MP,'paused').get,getWidth=Object.getOwnPropertyDescriptor(VP,'videoWidth').get;
if(video.src!=='https://example.test/media/Scene-First.ivf'||video.currentSrc!=='https://example.test/media/Selected.ivf'||!video.controls||video.currentTime!==0.75||video.duration!==3||video.videoWidth!==64||video.videoHeight!==48||!video.paused||video.seeking||video.ended||video.readyState!==4||video.networkState!==1)throw 'actual initial slots';
var rejected=0,forged=Object.create(VP);for(var action of [function(){getTime.call({});},function(){getWidth.call(document.body);},function(){getPaused.call(forged);}]){try{action();}catch(e){if(e.name==='TypeError')rejected++;else throw e;}}if(rejected!==3)throw 'prototype ancestry cannot forge native brand';
video.paused=false;if(!video.paused)throw 'readonly sloppy getter';var strict=0;try{(function(){'use strict';video.paused=false;})();}catch(e){if(e.name==='TypeError')strict++;}if(strict!==1)throw 'readonly strict getter';
Object.defineProperty(video,'paused',{value:'own',configurable:true});if(video.paused!=='own'||getPaused.call(video)!==true)throw 'ordinary shadow versus native slots';delete video.paused;
video.originalExpando={real:17};if(video.originalExpando.real!==17)throw 'real ordinary VIDEO expando';delete video.originalExpando;
var savedPause=MP.pause,calls=0;MP.pause=function(){calls++;return this;};if(video.pause()!==video||calls!==1)throw 'ordinary inherited override';MP.pause=savedPause;
video.pause=function(){return 'own';};if(video.pause()!=='own')throw 'ordinary method shadow';delete video.pause;if(video.pause!==savedPause)throw 'delete restores inherited canonical method';
"#);
    let mut snapshot=page.state.embedded_video.as_ref().unwrap().clone();
    snapshot.position=Duration::from_millis(1250);snapshot.width=128;snapshot.height=96;snapshot.duration=Some(Duration::from_secs(4));snapshot.ready=2;
    page.set_embedded_video(Some(snapshot));
    evaluate(&mut page,&mut doc,&mut forms,"if(getTime.call(video)!==1.25||getWidth.call(video)!==128||video.videoHeight!==96||video.duration!==4||video.readyState!==2||!video.paused||Object.getPrototypeOf(video)!==VP)throw 'getters must read new actual snapshot';");
}

#[test]
fn media_interface_canonical_pause_load_and_play_reuse_real_controls_and_promises() {
    let(mut doc,mut forms,mut page,node)=fixture();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var coerced=0,ignored={valueOf:function(){coerced++;throw 'ignored';},toString:function(){coerced++;throw 'ignored';}},rejected=0,sync=0;
var denied=MP.play.call(video,ignored),bad;
try{bad=MP.play.call({},ignored);}catch(e){sync++;}
if(Object.getPrototypeOf(denied)!==Promise.prototype||Object.getPrototypeOf(bad)!==Promise.prototype)throw 'actual Promise results';
denied.then(function(){throw 'untrusted Play fulfilled';},function(e){if(!(e instanceof DOMException)||e.name!=='NotAllowedError')throw e;rejected++;});
bad.then(function(){throw 'wrong receiver Play fulfilled';},function(e){if(!(e instanceof TypeError))throw e;rejected++;});
var brands=0;for(var action of [function(){MP.pause.call({});},function(){MP.load.call(Object.create(VP));},function(){new MP.play();}]){try{action();}catch(e){if(e.name==='TypeError')brands++;else throw e;}}
if(sync||coerced||brands!==3||!video.paused)throw 'real method admission and ignored args';
"#);
    evaluate(&mut page,&mut doc,&mut forms,"if(rejected!==2||coerced)throw 'actual queued Promise rejection';");
    assert!(!page.has_media_tasks());
    let mut snapshot=page.state.embedded_video.as_ref().unwrap().clone();snapshot.playing=true;snapshot.paused=false;page.set_embedded_video(Some(snapshot));
    evaluate(&mut page,&mut doc,&mut forms,"var order='';video.addEventListener('timeupdate',function(){order+='timeupdate;';});video.addEventListener('pause',function(){order+='pause;';});if(MP.pause.call(video,ignored)!==undefined||!video.paused||video.currentTime<0.75||video.currentTime>3||coerced)throw 'real pause issue';");
    let request=page.media_pause().unwrap();assert!(!request.applied);
    // Page/App handoff only. Physical Clock/device acceptance is separate.
    page.mark_media_pause_applied();let task=page.take_media_pause_task().unwrap();assert_eq!(task.node,node);
    let outcome=page.dispatch_media_task(&mut doc,&mut forms,node,MediaTask::Pause);assert_eq!(outcome.errors,0,"{}",page.report());
    evaluate(&mut page,&mut doc,&mut forms,"if(order!=='timeupdate;pause;')throw 'real grouped pause dispatch';MP.load.call(video,ignored);if(!video.paused||video.readyState!==0||!isNaN(video.duration)||coerced)throw 'real synchronous load reset';");
    assert!(request.retired.load(Ordering::Relaxed));assert!(page.take_media_selection().is_some());
}

#[test]
fn media_interface_extracted_src_setter_preserves_raw_attribute_and_stable_selection_fifo() {
    let(mut doc,mut forms,mut page,node)=fixture();
    let old=page.state.embedded_video.as_ref().unwrap().retired.clone();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var setTime=Object.getOwnPropertyDescriptor(MP,'currentTime').set,getTime=Object.getOwnPropertyDescriptor(MP,'currentTime').get,effects=0,number={valueOf:function(){effects++;return 1.25;}},refused=0;
try{setTime.call({},number);}catch(e){if(e.name==='TypeError')refused++;else throw e;}
if(refused!==1||effects)throw 'time setter brand before conversion';
Object.defineProperty(video,'currentTime',{value:'own-time',writable:true,configurable:true});
var timeResult=setTime.call(video,number);
if(timeResult!==undefined||effects!==1||getTime.call(video)!==1.25||video.currentTime!=='own-time'||!video.seeking)throw 'saved native time setter bypasses own data shadow';
delete video.currentTime;
if(timeResult!==undefined||effects!==1||video.currentTime!==1.25||!video.seeking)throw 'actual canonical finite-time setter';
"#);
    let shared=page.media_tasks().unwrap();
    let control=shared.lock().unwrap().take_control().unwrap();
    assert_eq!(control.1.node,node);assert!(Arc::ptr_eq(&control.1.retired,&old));
    assert!(matches!(control.2,crate::media_tasks::Control::Seek {seconds} if seconds==1.25));
    // Inspect accepted Page control only, without claiming seek/frame completion.
    evaluate(&mut page,&mut doc,&mut forms,r#"
var setSrc=Object.getOwnPropertyDescriptor(MP,'src').set,getSrc=Object.getOwnPropertyDescriptor(MP,'src').get,attr=video.getAttributeNode('src'),order='',srcReads=0,srcWrites=0;
Object.defineProperty(video,'src',{configurable:true,get:function(){srcReads++;return 'own-src';},set:function(){srcWrites++;}});
queueMicrotask(function(){if(video.networkState!==3||video.currentSrc!=='https://example.test/media/Selected.ivf')throw 'before source selection';order+='before;';});
var srcResult=setSrc.call(video,'Mixed-Replacement.ivf');
if(srcResult!==undefined||srcReads||srcWrites||getSrc.call(video)!=='https://example.test/media/Mixed-Replacement.ivf'||attr.value!=='Mixed-Replacement.ivf')throw 'saved native source setter bypasses own accessor shadow';
if(video.src!=='own-src'||srcReads!==1||srcWrites)throw 'own source accessor remains intact';delete video.src;
if(srcResult!==undefined||video.src!=='https://example.test/media/Mixed-Replacement.ivf'||attr.value!=='Mixed-Replacement.ivf'||video.getAttributeNode('src')!==attr||!video.paused||video.readyState!==0||video.networkState!==3)throw 'real extracted setter reset';
queueMicrotask(function(){if(video.networkState!==2||video.currentSrc!=='https://example.test/media/Mixed-Replacement.ivf')throw 'after source selection';order+='after;';});
"#);
    assert!(old.load(Ordering::Relaxed));assert_eq!(doc.attr(node,"src"),Some("Mixed-Replacement.ivf"));
    let selected=page.take_media_selection().unwrap();assert_eq!(selected.raw.as_deref(),Some("Mixed-Replacement.ivf"));
    evaluate(&mut page,&mut doc,&mut forms,"if(order!=='before;after;')throw 'stable setter FIFO';var marker={},before=video.getAttribute('src');try{setSrc.call(video,{toString:function(){throw marker;}});throw 'missing abrupt completion';}catch(e){if(e!==marker)throw e;}if(video.getAttribute('src')!==before)throw 'failed conversion changed source';");
    assert!(!page.has_media_selection());assert!(!page.state.embedded_video.as_ref().unwrap().retired.load(Ordering::Relaxed));
}

#[test]
fn media_interface_owner_realms_saved_holders_and_foreign_reads_keep_real_guards() {
    let(mut doc,mut forms,mut page,node)=fixture();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,ChildM=w.HTMLMediaElement,ChildV=w.HTMLVideoElement,ChildP=w.Promise,ChildD=w.DOMException,ChildFunction=w.Function;
if(ChildM===M||ChildV===V||Object.getPrototypeOf(ChildM)!==w.HTMLElement||Object.getPrototypeOf(ChildV)!==ChildM||Object.getPrototypeOf(ChildV.prototype)!==ChildM.prototype)throw 'real owner interface roots';
var read=ChildFunction('v','return v.play;'),invoke=ChildFunction('f','v','return f.call(v);'),get=Object.getOwnPropertyDescriptor(MP,'currentTime').get;
if(read(video)!==MP.play||Object.getPrototypeOf(read(video))!==Function.prototype)throw 'first lookup uses actual element owner function';
var childPlay=ChildM.prototype.play,childPause=ChildM.prototype.pause,childGet=Object.getOwnPropertyDescriptor(ChildM.prototype,'currentTime').get,childSetter=Object.getOwnPropertyDescriptor(ChildM.prototype,'src').set,childRejected=0;ChildFunction("Promise=function(){throw 'replaced child Promise';};DOMException=function(){throw 'replaced child DOMException';};")();
var promise=childPlay.call(video);if(Object.getPrototypeOf(promise)!==ChildP.prototype)throw 'saved child canonical operation realm';promise.then(function(){throw 'untrusted borrowed Play fulfilled';},function(e){if(Object.getPrototypeOf(e)!==ChildD.prototype||e.name!=='NotAllowedError')throw e;childRejected++;});
var rootPlay=MP.play,rootLoad=MP.load,converted=0,arg={toString:function(){converted++;return 'unused';}},unsupported=0,unsupportedPlay=0;
for(var operation of [childPause,ChildM.prototype.load]){try{operation.call(unbound,arg);throw 'unbound control succeeded';}catch(e){if(Object.getPrototypeOf(e)!==ChildD.prototype||e.name!=='NotSupportedError'||e.code!==9)throw e;unsupported++;}}
var unsupportedPromise=childPlay.call(unbound,arg);if(Object.getPrototypeOf(unsupportedPromise)!==ChildP.prototype)throw 'unavailable binding Promise owner';unsupportedPromise.catch(function(e){if(Object.getPrototypeOf(e)!==ChildD.prototype||e.name!=='NotSupportedError'||e.code!==9)throw e;unsupportedPlay++;});
if(unsupported!==2||converted)throw 'unavailable binding conversion or synchronous controls';
HTMLMediaElement=function(){throw 'replacement Media';};delete window.HTMLVideoElement;
if(Object.getPrototypeOf(video)!==VP||video.play!==rootPlay||!(video instanceof V)||V.prototype!==VP||M.prototype!==MP)throw 'global replacement must not replace canonical roots';
"#);
    evaluate(&mut page,&mut doc,&mut forms,"if(childRejected!==1||unsupportedPlay!==1||converted)throw 'actual child rejections';");
    let token=page.state.embedded_video.as_ref().unwrap().retired.clone();let next=page.state.media_changes.next;
    let raw=doc.attr(node,"src").unwrap().to_owned();
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/parent").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["read(video);","invoke(rootLoad,video);","invoke(get,video);","childPlay.call(video,arg);","childPause.call(video,arg);","childGet.call(video);","childSetter.call(video,arg);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"{source}: {}",page.report());
        assert!(page.report().contains("same-origin"),"{}",page.report());
        assert!(!token.load(Ordering::Relaxed));assert_eq!(page.state.media_changes.next,next);assert!(!page.has_media_selection());
        assert_eq!(doc.attr(node,"src"),Some(raw.as_str()));assert!(!page.has_media_tasks());
        evaluate(&mut page,&mut doc,&mut forms,"if(converted||video.play!==rootPlay||Object.getPrototypeOf(video)!==VP||!video.paused)throw 'foreign failure changed main source or roots';");
    }
}

#[test]
fn media_interface_genuine_unbound_class_never_fabricates_player_or_loses_identity() {
    let(mut doc,mut forms,mut page,node)=fixture();
    evaluate(&mut page,&mut doc,&mut forms,"var created=document.createElement('video'),upper=document.createElementNS('http://www.w3.org/1999/xhtml','VIDEO'),audio=document.getElementById('audio');if(!(unbound instanceof V)||!(created instanceof V)||!(unbound instanceof M)||upper instanceof V||Object.getPrototypeOf(unbound)!==VP||Object.getPrototypeOf(created)!==VP||!(audio instanceof M)||audio instanceof V)throw 'real static media/VIDEO brands';");
    let old=page.state.embedded_video.as_ref().unwrap().clone();
    for source in ["Object.getOwnPropertyDescriptor(MP,'currentTime').get.call(unbound);","MP.load.call(created);","MP.pause.call(unbound);","Object.getOwnPropertyDescriptor(MP,'duration').get.call(audio);","MP.pause.call(audio);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"{source}: {}",page.report());
        assert!(page.report().contains("not implemented")||page.report().contains("unsupported")||page.report().contains("binding"),"{}",page.report());
        let actual=page.state.embedded_video.as_ref().unwrap();assert_eq!(actual.node,node);assert!(Arc::ptr_eq(&actual.retired,&old.retired));assert!(!actual.retired.load(Ordering::Relaxed));
        assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
    }
    page.set_embedded_video(None);
    evaluate(&mut page,&mut doc,&mut forms,"if(!(video instanceof V)||!(video instanceof M)||Object.getPrototypeOf(video)!==VP||video.constructor!==V||Object.getPrototypeOf(unbound)!==VP)throw 'class identity must not depend on player snapshot';");
}

#[test]
fn media_interface_native_getter_work_refusal_leaves_source_and_recovers() {
    let(mut doc,mut forms,mut page,node)=fixture();
    evaluate(&mut page,&mut doc,&mut forms,"var nativeTime=Object.getOwnPropertyDescriptor(MP,'currentTime').get;");
    let token=page.state.embedded_video.as_ref().unwrap().retired.clone();
    let next=page.state.media_changes.next;
    let normal=page.state.limits.host_work;
    page.state.limits.host_work=1;
    let outcome=page.console_eval(&mut doc,&mut forms,"nativeTime.call(video);nativeTime.call(video);");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert!(page.report().contains("DOM work limit"),"{}",page.report());
    assert_eq!(page.state.media_changes.next,next);
    let snapshot=page.state.embedded_video.as_ref().unwrap();
    assert_eq!(snapshot.node,node);assert!(Arc::ptr_eq(&snapshot.retired,&token));
    assert!(!token.load(Ordering::Relaxed));
    assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
    page.state.limits.host_work=normal;
    evaluate(&mut page,&mut doc,&mut forms,"if(nativeTime.call(video)!==0.75||Object.getPrototypeOf(video)!==VP||!video.paused)throw 'native getter recovery';");
}
