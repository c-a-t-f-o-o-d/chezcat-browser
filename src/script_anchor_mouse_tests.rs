//! Original semantic controls: native Page input, never synthetic JS dispatch.
use super::*;
fn fixture(source:&str)->(Document,Forms,Page,usize) {
    let mut doc=crate::html::parse("<!doctype html><html><body><div id=parent><a id=link href='/next'>Actual anchor</a></div><p id=other>Other</p></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    page.set_location(&crate::navigation::Location{url:Some(crate::url::parse("https://example.test/start").unwrap().0),path:None,root:None,fragment:String::new()});
    let outcome=page.start(&mut doc,&mut forms,&[Script{node:0,label:"Original native anchor fixture".into(),source:source.into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let node=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("link")).unwrap();(doc,forms,page,node)
}
fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str){let outcome=page.console_eval(doc,forms,source);assert_eq!(outcome.errors,0,"{}",page.report());}
fn input(button:u8)->AnchorMouseInput{AnchorMouseInput{button,ctrl:true,alt:true,meta:true,client_x:12.5,client_y:7.25,screen_x:90.0,screen_y:80.5,..AnchorMouseInput::default()}}
fn fire(page:&mut Page,doc:&mut Document,forms:&mut Forms,node:usize,button:u8)->Outcome{page.begin_job();page.anchor_mouse(doc,forms,node,input(button))}

#[test]
fn native_anchor_primary_uses_actual_mouse_prototype_fields_path_and_checkpoint_cancellation(){
    let(mut doc,mut forms,mut page,node)=fixture(r#"
var a=document.getElementById('link'),p=document.getElementById('parent'),saved,get,log='';
window.addEventListener('click',function(e){log+='W';},true);
p.addEventListener('click',function(e){log+='P';},true);
a.addEventListener('click',function(e){saved=e;get=Object.getOwnPropertyDescriptor(MouseEvent.prototype,'clientX').get;
 if(!(e instanceof MouseEvent)||!(e instanceof UIEvent)||!(e instanceof Event)||Object.getPrototypeOf(e)!==MouseEvent.prototype)throw 'genuine canonical Mouse ancestry';
 if(!e.isTrusted||e.type!=='click'||!e.bubbles||!e.cancelable||!e.composed||e.target!==a||e.currentTarget!==a||e.eventPhase!==2)throw 'native fields';
 if(e.view!==window||e.detail!==1||e.button!==0||e.buttons!==0||e.which!==1||e.relatedTarget!==null)throw 'release fields';
 if(e.clientX!==12.5||e.x!==12.5||e.clientY!==7.25||e.y!==7.25||e.pageX!==12.5||e.pageY!==7.25||e.screenX!==90||e.screenY!==80.5)throw 'actual coordinates';
 if(!e.ctrlKey||e.shiftKey||!e.altKey||!e.metaKey||!e.getModifierState('Control')||e.getModifierState('Shift')||e.getModifierState('CapsLock'))throw 'actual modifiers';
 var expected=[a,p,document.body,document.documentElement,document,window],path=e.composedPath();if(path.length!==expected.length)throw 'path size';for(var i=0;i<path.length;i++)if(path[i]!==expected[i])throw 'path identity';
 log+='A';Promise.resolve().then(function(){log+='M';e.preventDefault();});
});
p.addEventListener('click',function(e){if(!e.defaultPrevented)throw 'checkpoint precedes ancestor';log+='B';});
"#);
    let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert_eq!(outcome.errors,0,"{}",page.report());assert!(outcome.prevented);assert!(page.state.event.is_none());
    evaluate(&mut page,&mut doc,&mut forms,"if(log!=='WPAMB')throw log;var refused=false;try{get.call(saved);}catch(e){refused=e.name==='TypeError';}if(!refused)throw 'expired trusted source';var untrusted=new MouseEvent('x',{clientX:3.5});if(get.call(untrusted)!==3.5||untrusted.isTrusted)throw 'saved canonical getter borrows live actual receiver';");
}

#[test]
fn native_anchor_middle_dispatches_only_auxclick_with_real_handler_cancellation(){
    let(mut doc,mut forms,mut page,node)=fixture(r#"
var a=document.getElementById('link'),clicks=0,aux=0;
a.onclick=function(){clicks++;};a.onauxclick=function(e){aux++;if(e.type!=='auxclick'||!e.isTrusted||e.button!==1||e.which!==2||e.buttons!==0||Object.getPrototypeOf(e)!==MouseEvent.prototype)throw 'middle input';return false;};
"#);
    let outcome=fire(&mut page,&mut doc,&mut forms,node,1);assert_eq!(outcome.errors,0,"{}",page.report());assert!(outcome.prevented);
    evaluate(&mut page,&mut doc,&mut forms,"if(clicks!==0||aux!==1)throw 'actual middle event count';");
}

#[test]
fn native_anchor_initializers_are_dispatch_noops_and_prototype_overrides_remain_real(){
    let(mut doc,mut forms,mut page,node)=fixture(r#"
var a=document.getElementById('link'),get=Object.getOwnPropertyDescriptor(MouseEvent.prototype,'clientX').get,conversions=0;
MouseEvent.prototype.getModifierState=function(name){return name==='actual override';};
a.addEventListener('click',function(e){
 if(!e.getModifierState('actual override'))throw 'prototype method override';
 e.initMouseEvent({toString:function(){conversions++;return 'replaced';}},false,false,window,9,1,2,3,4,false,false,false,false,1,null);
 if(conversions!==1||e.type!=='click'||e.clientX!==12.5||e.button!==0||!e.isTrusted)throw 'native initializer no-op';
 var caught=false;try{e.initMouseEvent('bad',false,false,{},0);}catch(x){caught=x.name==='TypeError';}if(!caught)throw 'real initializer view brand';
 Object.defineProperty(e,'clientX',{value:99,configurable:true});if(e.clientX!==99||get.call(e)!==12.5)throw 'expando does not replace native slot';delete e.clientX;if(e.clientX!==12.5)throw 'canonical getter restored';
});
"#);
    let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert_eq!(outcome.errors,0,"{}",page.report());assert!(!outcome.prevented);
}

#[test]
fn native_anchor_passive_and_ordinary_throw_do_not_cancel_but_path_is_frozen(){
    let(mut doc,mut forms,mut page,node)=fixture(r#"
var a=document.getElementById('link'),p=document.getElementById('parent'),log='';
a.addEventListener('click',function(e){e.preventDefault();log+='p';},{passive:true});
a.addEventListener('click',function(e){p.removeChild(a);log+='t';throw 'ordinary author error';});
p.addEventListener('click',function(e){log+='b';if(e.composedPath()[1]!==p)throw 'frozen original path';});
"#);
    let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert_eq!(outcome.errors,1,"{}",page.report());assert!(!outcome.prevented);
    evaluate(&mut page,&mut doc,&mut forms,"if(log!=='ptb'||a.parentNode!==null)throw 'ordinary throw or path mutation';");
    let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert!(outcome.prevented);assert_eq!(outcome.errors,1);assert!(page.state.event.is_none());
}

#[test]
fn native_anchor_invalid_foreign_nonfinite_and_exhausted_path_fail_closed(){
    let(mut doc,mut forms,mut page,node)=fixture("document.getElementById('link').onclick=function(){document.title='FORBIDDEN';};");
    for bad in [AnchorMouseInput{button:2,..input(0)},AnchorMouseInput{client_x:f64::NAN,..input(0)},AnchorMouseInput{screen_y:f64::INFINITY,..input(0)}]{page.begin_job();let outcome=page.anchor_mouse(&mut doc,&mut forms,node,bad);assert!(outcome.prevented);assert_eq!(outcome.errors,1);}
    let other=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("other")).unwrap();assert!(fire(&mut page,&mut doc,&mut forms,other,0).prevented);
    doc.nodes[node].namespace=crate::html::Namespace::UnknownForeign;assert!(fire(&mut page,&mut doc,&mut forms,node,0).prevented);doc.nodes[node].namespace=crate::html::Namespace::Html;
    page.begin_job();page.state.host_work=0;let outcome=page.anchor_mouse(&mut doc,&mut forms,node,input(0));assert!(outcome.prevented);assert!(outcome.errors>0);
    page.begin_job();page.state.copy_work=0;let outcome=page.anchor_mouse(&mut doc,&mut forms,node,input(0));assert!(outcome.prevented);
    page.begin_job();page.state.next_event=script_class_list::BASE;let outcome=page.anchor_mouse(&mut doc,&mut forms,node,input(0));assert!(outcome.prevented);assert!(outcome.errors>0);
    assert_ne!(doc.title,"FORBIDDEN");assert!(page.state.event.is_none());
}

#[test]
fn native_anchor_fatal_listener_and_microtask_budget_veto_default_and_expire_slot(){
    for source in ["function heavy(){while(true){}}document.getElementById('link').addEventListener('click',heavy);", "function heavy(){Promise.resolve().then(function(){while(true){}});}document.getElementById('link').addEventListener('click',heavy);"] {
        let(mut doc,mut forms,mut page,node)=fixture(source);let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert!(outcome.prevented);assert!(outcome.errors>0);assert!(page.state.event.is_none());
        evaluate(&mut page,&mut doc,&mut forms,"document.getElementById('link').removeEventListener('click',heavy);document.title='recovered';");
        let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert_eq!(outcome.errors,0,"{}",page.report());assert!(!outcome.prevented);
    }
}

#[test]
fn native_anchor_observes_queued_author_navigation_without_consuming_it(){
    let(mut doc,mut forms,mut page,node)=fixture("document.getElementById('link').onclick=function(){Promise.resolve().then(function(){location.href='/author';});};");
    assert!(!page.has_navigation_request());let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert_eq!(outcome.errors,0,"{}",page.report());assert!(page.has_navigation_request());
    assert_eq!(page.take_navigation().unwrap().target.url.unwrap().path,"/author");assert!(!page.has_navigation_request());
}

#[test]
fn native_anchor_script_policy_disabled_has_no_callback_or_synthetic_event(){
    let(mut doc,mut forms,mut page,node)=fixture("document.getElementById('link').onclick=function(){document.title='FORBIDDEN';};");
    doc.scripting_enabled=false;let count=page.state.next_event;let outcome=fire(&mut page,&mut doc,&mut forms,node,0);assert!(!outcome.prevented);assert_eq!(outcome.errors,0);assert_eq!(page.state.next_event,count);assert_ne!(doc.title,"FORBIDDEN");
}
