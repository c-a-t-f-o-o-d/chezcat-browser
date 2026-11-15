//! Original attribute/runtime witnesses. No physical playback or device claim.
use super::*;

fn fixture()->(Document,Forms,Page,usize) {
    let mut doc=crate::html::parse("<html><head></head><body><video id=v src='Original.ivf' controls></video><video id=unbound></video><audio id=audio></audio><p id=p>Original controls reflection</p></body></html>");
    doc.scripting_enabled=true;
    let node=crate::embedded_video::authored_node(&doc).unwrap();
    let mut forms=Forms::new(&doc);let limits=crate::javascript::limits::BROWSER;
    let mut page=Page::with_limits(limits);
    let location=crate::navigation::Location {url:Some(crate::url::parse("https://example.test/media/page").unwrap().0),path:None,root:None,fragment:String::new()};
    page.set_location(&location);page.set_resource_base(&location);
    page.set_embedded_video(Some(crate::embedded_video::Snapshot {
        tasks:crate::media_tasks::shared(limits.host.media_tasks),node,
        retired:Arc::new(AtomicBool::new(false)),current_src:"https://example.test/media/Original.ivf".into(),
        width:64,height:48,duration:Some(Duration::from_secs(3)),position:Duration::from_millis(750),
        observed:Instant::now(),clock:crate::embedded_video::ClockKind::WallTime,
        playing:false,paused:true,load_pending:false,frame_ready:true,error_code:None,
        ended:false,seeking:false,ready:4,network:1,
    }));
    let outcome=page.start(&mut doc,&mut forms,&[]);assert_eq!(outcome.errors,0,"{}",page.report());
    eval(&mut page,&mut doc,&mut forms,"var video=document.getElementById('v'),unbound=document.getElementById('unbound'),audio=document.getElementById('audio'),MP=HTMLMediaElement.prototype,controlDescriptor=Object.getOwnPropertyDescriptor(MP,'controls'),getControls=controlDescriptor.get,setControls=controlDescriptor.set;");
    (doc,forms,page,node)
}
fn eval(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let outcome=page.console_eval(doc,forms,source);assert_eq!(outcome.errors,0,"{source}\n{}",page.report());
}

#[test]
fn media_controls_boolean_conversion_static_brands_and_canonical_shadow_bypass() {
    let(mut doc,mut forms,mut page,node)=fixture();
    let token=page.state.embedded_video.as_ref().unwrap().retired.clone();
    let next=page.state.media_changes.next;
    eval(&mut page,&mut doc,&mut forms,r#"
if(typeof getControls!=='function'||typeof setControls!=='function'||getControls.length!==0||setControls.length!==1||!controlDescriptor.enumerable||!controlDescriptor.configurable)throw 'real Boolean descriptor';
var coerced=0,arg={valueOf:function(){coerced++;throw 'Boolean valueOf';},toString:function(){coerced++;throw 'Boolean toString';}};
for(var value of [undefined,null,false,0,-0,NaN,'']){if(setControls.call(video,value)!==undefined||getControls.call(video)||video.hasAttribute('controls'))throw 'false Boolean reflection';}
for(var value of [true,1,-1,'false',arg,Symbol('controls'),function(){}]){setControls.call(video,value);if(!video.controls||video.getAttribute('controls')!=='')throw 'true Boolean reflection';}
if(coerced)throw 'Boolean must not invoke primitive conversion';
Object.defineProperty(video,'controls',{value:'own-shadow',writable:true,configurable:true});
setControls.call(video,false);if(video.controls!=='own-shadow'||getControls.call(video)||video.hasAttribute('controls'))throw 'native setter bypasses own data shadow';
var reads=0,writes=0;Object.defineProperty(video,'controls',{configurable:true,get:function(){reads++;return 'accessor-shadow';},set:function(){writes++;}});
setControls.call(video,arg);if(reads||writes||!getControls.call(video)||video.getAttribute('controls')!=='')throw 'native setter bypasses accessor shadow';
if(video.controls!=='accessor-shadow'||reads!==1||writes)throw 'ordinary shadow survives';delete video.controls;
for(var item of [unbound,audio,document.createElement('video')]){setControls.call(item,true);if(!getControls.call(item)||!item.controls||item.getAttribute('controls')!=='')throw 'genuine unbound content reflection';item.controls=false;if(getControls.call(item)||item.hasAttribute('controls'))throw 'unbound reflected removal';}
var brands=0;for(var item of [{},document.getElementById('p'),Object.create(HTMLVideoElement.prototype)]){try{setControls.call(item,arg);}catch(e){if(e.name==='TypeError')brands++;else throw e;}}
if(brands!==3||coerced||!video.controls||!video.paused||video.currentTime!==0.75)throw 'brands and existing state';
video.paused=false;if(!video.paused)throw 'other readonly media state changed';
"#);
    assert!(Arc::ptr_eq(&page.state.embedded_video.as_ref().unwrap().retired,&token));
    assert!(!token.load(Ordering::Relaxed));assert_eq!(page.state.media_changes.next,next);
    assert_eq!(doc.attr(node,"src"),Some("Original.ivf"));assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
    page.set_embedded_video(None);
    eval(&mut page,&mut doc,&mut forms,"video.controls=false;if(video.controls||getControls.call(video)||!(video instanceof HTMLVideoElement))throw 'reflection survives absent snapshot';setControls.call(video,true);if(!video.controls)throw 'no player required';");
    assert!(page.state.embedded_video.is_none());assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
}

#[test]
fn media_controls_attribute_and_attached_attr_routes_share_real_presence_and_namespace() {
    let(mut doc,mut forms,mut page,node)=fixture();
    let token=page.state.embedded_video.as_ref().unwrap().retired.clone();
    eval(&mut page,&mut doc,&mut forms,r#"
video.setAttribute('controls','false');var original=video.getAttributeNode('controls');
if(!video.controls||original.value!=='false')throw 'Boolean presence keeps raw value';
original.value='first';original.nodeValue='second';original.textContent='third';
if(!video.controls||video.getAttributeNode('controls')!==original||video.getAttribute('controls')!=='third')throw 'attached Attr writes';
setControls.call(video,true);if(original.value!==''||video.getAttributeNode('controls')!==original)throw 'property keeps attached identity';
video.removeAttribute('controls');if(video.controls||original.ownerElement!==null||original.value!=='')throw 'real detached Attr';
var incoming=document.createAttribute('controls');incoming.value='retained';
if(video.setAttributeNode(incoming)!==null||incoming.ownerElement!==video||!video.controls)throw 'actual Attr adoption';
if(video.attributes.getNamedItem('controls')!==incoming)throw 'actual NamedNodeMap read identity';
"#);
    let commits=page.state.dom_commits;
    let outcome=page.console_eval(&mut doc,&mut forms,"video.attributes.removeNamedItem('controls');");
    assert_eq!(outcome.errors,1,"NamedNodeMap mutation retains its explicit unsupported boundary.");
    assert_eq!(page.state.dom_commits,commits);assert_eq!(doc.attr(node,"controls"),Some("retained"));
    eval(&mut page,&mut doc,&mut forms,r#"
if(video.removeAttributeNode(incoming)!==incoming||video.controls||incoming.ownerElement!==null||incoming.value!=='retained')throw 'existing element Attr removal and recovery';
video.setAttributeNS(null,'controls','ns-null');if(!video.controls)throw 'null namespace controls';video.removeAttributeNS(null,'controls');
video.setAttributeNS('urn:original-controls','m:controls','metadata');var metadata=video.getAttributeNodeNS('urn:original-controls','controls');
metadata.value='still-metadata';if(video.controls||getControls.call(video)||video.getAttribute('m:controls')!=='still-metadata')throw 'namespaced metadata cannot enable controls';
setControls.call(video,true);if(metadata.value!=='still-metadata'||video.getAttribute('controls')!=='')throw 'independent native attribute';
video.removeAttributeNode(video.getAttributeNode('controls'));if(video.controls||metadata.ownerElement!==video)throw 'qualified identity isolation';
video.removeAttributeNS('urn:original-controls','controls');if(metadata.ownerElement!==null||metadata.value!=='still-metadata')throw 'metadata removal';
"#);
    let before=page.state.dom_commits;
    for source in ["video.setAttribute('autoplay','');","video.setAttribute('width','999');","video.toggleAttribute('controls');"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"{source}: {}",page.report());
        assert_eq!(page.state.dom_commits,before);assert_eq!(doc.attr(node,"controls"),None);assert_eq!(doc.attr(node,"autoplay"),None);assert_eq!(doc.attr(node,"width"),None);
    }
    assert!(Arc::ptr_eq(&page.state.embedded_video.as_ref().unwrap().retired,&token));assert!(!token.load(Ordering::Relaxed));
    assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
}

#[test]
fn media_controls_reflection_leaves_accepted_play_promise_and_source_fifo_intact() {
    let(mut doc,mut forms,mut page,_)=fixture();page.notify_user_activation();
    eval(&mut page,&mut doc,&mut forms,"var order='',accepted=video.play();accepted.then(function(){order+='fulfilled;';},function(){throw 'controls rejected play';});if(!(accepted instanceof Promise)||video.paused||order!=='')throw 'actual pending Play';");
    let token=page.state.embedded_video.as_ref().unwrap().retired.clone();
    let shared=page.media_tasks().unwrap();let queued=shared.lock().unwrap().len();
    let pending=page.pending_media_play_count();let next=page.state.media_changes.next;
    assert!(queued>0);
    eval(&mut page,&mut doc,&mut forms,"setControls.call(video,false);video.setAttribute('controls','false');video.getAttributeNode('controls').value='';setControls.call(video,false);if(video.paused||video.seeking||video.currentTime<0.75||order!=='')throw 'controls affected accepted Play';");
    assert_eq!(shared.lock().unwrap().len(),queued);assert_eq!(page.pending_media_play_count(),pending);
    assert_eq!(page.state.media_changes.next,next);assert!(!page.has_media_selection());
    assert!(Arc::ptr_eq(&page.state.embedded_video.as_ref().unwrap().retired,&token));assert!(!token.load(Ordering::Relaxed));
    // This is a queue preservation witness. Physical readiness/Promise dispatch
    // remains the existing controller's responsibility, not this test's claim.
}

#[test]
fn media_controls_owner_realm_saved_accessors_keep_foreign_guards() {
    let(mut doc,mut forms,mut page,node)=fixture();
    eval(&mut page,&mut doc,&mut forms,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
var childControls=Object.getOwnPropertyDescriptor(w.HTMLMediaElement.prototype,'controls'),childGet=childControls.get,childSet=childControls.set,ChildFunction=w.Function;
if(Object.getPrototypeOf(childGet)!==w.Function.prototype||Object.getPrototypeOf(childSet)!==w.Function.prototype)throw 'real accessor owner';
var read=ChildFunction('v','return v.controls;'),invoke=ChildFunction('f','v','return f.call(v,true);');
childSet.call(video,false);if(childGet.call(video)||read(video))throw 'same-origin borrowed reflection';
ChildFunction("HTMLMediaElement=function(){throw 'replaced global';};")();childSet.call(video,true);if(!childGet.call(video))throw 'saved genuine accessors';
"#);
    let token=page.state.embedded_video.as_ref().unwrap().retired.clone();let next=page.state.media_changes.next;
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/parent").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["read(video);","invoke(setControls,video);","childGet.call(video);","childSet.call(video,false);"] {
        let before=page.state.dom_commits;
        let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,1,"{source}: {}",page.report());assert!(page.report().contains("same-origin"),"{}",page.report());
        assert_eq!(page.state.dom_commits,before);assert_eq!(doc.attr(node,"controls"),Some(""));
    }
    assert!(!token.load(Ordering::Relaxed));assert_eq!(page.state.media_changes.next,next);assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
}

#[test]
fn media_controls_checked_refusal_is_atomic_and_recovers_without_source_changes() {
    let(mut doc,mut forms,mut page,node)=fixture();
    eval(&mut page,&mut doc,&mut forms,"video.controls=false;");
    let token=page.state.embedded_video.as_ref().unwrap().retired.clone();let retained=page.state.retained;
    let normal_limit=page.state.retained_limit;page.state.retained_limit=retained+7;
    let before=page.state.dom_commits;
    let outcome=page.console_eval(&mut doc,&mut forms,"setControls.call(video,true);");assert_eq!(outcome.errors,1,"{}",page.report());
    assert!(page.report().contains("retained"),"{}",page.report());assert_eq!(doc.attr(node,"controls"),None);assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,before);
    page.state.retained_limit=normal_limit;
    eval(&mut page,&mut doc,&mut forms,"setControls.call(video,true);if(!video.controls||video.getAttribute('controls')!=='')throw 'reflection recovery';");
    let work=page.state.limits.host_work;page.state.limits.host_work=0;
    let before=page.state.dom_commits;
    let outcome=page.console_eval(&mut doc,&mut forms,"setControls.call(video,false);");assert_eq!(outcome.errors,1,"{}",page.report());assert_eq!(doc.attr(node,"controls"),Some(""));assert_eq!(page.state.dom_commits,before);
    page.state.limits.host_work=work;
    eval(&mut page,&mut doc,&mut forms,"video.controls=false;if(video.controls||video.getAttribute('src')!=='Original.ivf'||!video.paused)throw 'work recovery';");
    assert!(Arc::ptr_eq(&page.state.embedded_video.as_ref().unwrap().retired,&token));assert!(!token.load(Ordering::Relaxed));assert!(!page.has_media_selection());assert!(!page.has_media_tasks());
}
