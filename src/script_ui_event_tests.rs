//! Original finite UIEvent Page/Runtime controls; no service or device IO.
use super::*;

#[test]
fn ui_event_shared_registry_work_refusal_is_prepublication_and_recovers() {
    let (mut doc,mut forms,mut page)=fixture("var kept=new UIEvent('kept',{detail:17}),factory=document.createEvent('UIEvent');factory.initUIEvent('factory-kept');");
    let count=page.state.created_events.len();
    let retained=page.state.retained;
    let next=page.state.next_event;
    let normal=page.state.limits.host_work;
    page.state.limits.host_work=1;
    for source in ["new UIEvent('work-refused');","document.createEvent('UIEvent');"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1);
        assert_eq!(page.state.created_events.len(),count);
        assert_eq!(page.state.retained,retained);
        assert_eq!(page.state.next_event,next);
    }
    page.state.limits.host_work=normal;
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=new UIEvent('work-recovered',{detail:19});if(kept.detail!==17||factory.type!=='factory-kept'||recovered.detail!==19||recovered.isTrusted)throw 'actual registry-work admission recovery';");
}

fn fixture(source:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse("<html><head></head><body><div id=parent><button id=button type=button>Run</button></div></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::for_test();
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    let outcome=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original genuine UIEvent control".into(),source:source.into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    (doc,forms,page)
}

fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let outcome=page.console_eval(doc,forms,source);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn ui_event_real_constructor_prototype_fields_ordinary_overrides_and_dispatch() {
    let (doc,_forms,page)=fixture(r#"
var C=UIEvent,P=C.prototype,b=document.getElementById('button'),viewReads=0,ignoredReads=0;
var e=new C('ui-real',{bubbles:true,cancelable:true,composed:true,detail:7,view:window,get which(){ignoredReads++;throw 'optional which extension must not be read';}});
if(ignoredReads||typeof C!=='function'||C.name!=='UIEvent'||C.length!==1||Object.getPrototypeOf(C)!==Event||Object.getPrototypeOf(P)!==Event.prototype)throw 'real inherited interface';
if(Object.getPrototypeOf(e)!==P||e.constructor!==C||!(e instanceof C)||!(e instanceof Event)||Object.prototype.toString.call(e)!=='[object UIEvent]')throw 'real UI instance ancestry';
if(e.type!=='ui-real'||!e.bubbles||!e.cancelable||!e.composed||e.view!==window||e.detail!==7||e.which!==0||e.isTrusted||e.target!==null)throw 'real constructor slots';
var d=new UIEvent('defaults');if(d.view!==null||d.detail!==0||d.which!==0||d.bubbles||d.cancelable||d.composed)throw 'actual defaults';
for(var name of ['view','detail','which']){
 var desc=Object.getOwnPropertyDescriptor(P,name);if(typeof desc.get!=='function'||desc.get.length!==0||desc.set!==undefined||!desc.enumerable||!desc.configurable||Object.prototype.hasOwnProperty.call(e,name))throw 'UI inherited getter '+name;
}
var trust=Object.getOwnPropertyDescriptor(e,'isTrusted'),method=Object.getOwnPropertyDescriptor(P,'initUIEvent');
if(typeof trust.get!=='function'||!trust.enumerable||trust.configurable||trust.set!==undefined||Object.getOwnPropertyDescriptor(P,'isTrusted')!==undefined)throw 'inherited-family own unforgeable';
if(method.value!==e.initUIEvent||method.value.length!==1||!method.writable||!method.enumerable||!method.configurable||e.preventDefault!==Event.prototype.preventDefault)throw 'canonical methods';
e.view=null;e.detail=91;e.which=37;if(e.view!==window||e.detail!==7||e.which!==0)throw 'readonly sloppy setters';
var errors=0;for(var name of ['view','detail','which']){try{(function(){'use strict';e[name]=13;})();}catch(x){if(x.name==='TypeError')errors++;}}if(errors!==3)throw 'readonly strict setters';
var get=Object.getOwnPropertyDescriptor(P,'detail').get;
Object.defineProperty(e,'detail',{value:'own',configurable:true});if(e.detail!=='own'||get.call(e)!==7)throw 'ordinary UI shadow without slot mutation';delete e.detail;
var init=P.initUIEvent,overrides=0;P.initUIEvent=function(){overrides++;return this;};
if(e.initUIEvent('ignored')!==e||overrides!==1||e.type!=='ui-real')throw 'prototype override observed';P.initUIEvent=init;
e.initUIEvent=function(){return 'own';};if(e.initUIEvent()!=='own')throw 'own method shadow';delete e.initUIEvent;if(e.initUIEvent!==init)throw 'delete restores canonical UI method';
var calls=0;b.addEventListener('ui-real',function(event){calls++;if(event!==e||event.detail!==7||event.view!==window||event.isTrusted||event.target!==b)throw 'actual UI delivery';event.preventDefault();});
if(b.dispatchEvent(e)!==false||calls!==1||!e.defaultPrevented||e.currentTarget!==null||e.eventPhase!==0||e.composedPath().length)throw 'actual dispatch/cancel cleanup';
document.title='real-ui-constructor-dispatch';
"#);
    assert_eq!(doc.title,"real-ui-constructor-dispatch");
    assert!(page.state.created_events.iter().all(|event|event.initialized&&!event.dispatching));
}

#[test]
fn ui_event_dictionary_get_and_conversion_order_defaults_numeric_and_reentry_are_real() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var log='',init={
 get bubbles(){log+='B';return true;},
 get cancelable(){log+='C';return true;},
 get composed(){log+='O';return true;},
 get detail(){log+='D';return {valueOf:function(){log+='N';return 4294967297;}};},
 get view(){log+='V';return window;}
};
var e=new UIEvent({toString:function(){log+='T';return 'ordered';}},init);
if(log!=='TBCODNV'||e.detail!==1||e.view!==window||!e.composed)throw 'IDL member conversion occurs before next Get';
var refused=0,early=0;
try{UIEvent({toString:function(){early++;return 'call';}});}catch(x){if(x.name==='TypeError')refused++;}
try{new UIEvent();}catch(x){if(x.name==='TypeError')refused++;}
try{new UIEvent(Symbol(),{get detail(){early++;return 3;}});}catch(x){if(x.name==='TypeError')refused++;}
for(var badInit of [true,7,'wrong',Symbol()]){try{new UIEvent('invalid',badInit);}catch(x){if(x.name==='TypeError')refused++;}}
if(refused!==7||early!==0)throw 'real constructor and dictionary negative brands';
for(var init of [undefined,null]){var d=new UIEvent('defaults',init);if(d.detail!==0||d.view!==null||d.which!==0||d.bubbles||d.cancelable||d.composed)throw 'dictionary defaults';}
for(var item of [[NaN,0],[Infinity,0],[-Infinity,0],[-1.9,-1],[4294967297,1],[-4294967297,-1],[2147483648,-2147483648]]){var n=new UIEvent('number',{detail:item[0]});if(n.detail!==item[1])throw 'IDL long conversion';}
var stopped='',symbol=false;try{new UIEvent('symbol',{get detail(){stopped+='D';return Symbol();},get view(){stopped+='V';return window;}});}catch(x){symbol=x.name==='TypeError';}if(!symbol||stopped!=='D')throw 'Symbol conversion precedes view Get';
var inherited={},inheritedInit=Object.create(inherited),reads='';
Object.defineProperty(inherited,'detail',{get:function(){if(this!==inheritedInit)throw 'dictionary receiver';reads+='D';return 9;}});
Object.defineProperty(inherited,'view',{get:function(){reads+='V';return null;}});
var fromInherited=new UIEvent('inherited',inheritedInit);if(reads!=='DV'||fromInherited.detail!==9||fromInherited.view!==null)throw 'inherited member Get';
var marker={},nested,tail=0;
"#);
    let before=page.state.created_events.len();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var caught=false;
try{new UIEvent('unpublished',{get detail(){return {valueOf:function(){nested=new UIEvent('nested-real',{detail:17,view:window});throw marker;}};},get view(){tail++;return window;}});}catch(x){caught=x===marker;}
if(!caught||tail||nested.type!=='nested-real'||nested.detail!==17||nested.view!==window)throw 'reentrant numeric effects and thrown identity';
"#);
    assert_eq!(page.state.created_events.len(),before+1,"Only the actual nested UIEvent may publish.");
    assert!(page.state.created_events.iter().all(|event|event.name!="unpublished"));
}

#[test]
fn ui_event_window_brand_origin_and_legacy_positional_initializer_are_distinct() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
for(var view of [window,w,null,undefined]){var e=new UIEvent('view',{detail:3,view:view});if(e.view!==(view===undefined?null:view)||e.detail!==3)throw 'genuine nullable Window';}
var log='',bad={toString:function(){throw 'duck string conversion';},valueOf:function(){throw 'duck numeric conversion';}},rejected=false;
try{new UIEvent('duck',{get detail(){log+='D';return {valueOf:function(){log+='N';return 7;}};},get view(){log+='V';return bad;}});}catch(x){rejected=x.name==='TypeError';}
if(!rejected||log!=='DNV')throw 'view brand validation after detail conversion, without duck coercion';
var f=document.createEvent('UIEvent'),b=document.getElementById('button'),uninitialized=false;
if(Object.getPrototypeOf(f)!==UIEvent.prototype||f.view!==null||f.detail!==0||f.which!==0||f.type!==''||f.isTrusted)throw 'genuine legacy UI factory';
try{b.dispatchEvent(f);}catch(x){uninitialized=x.name==='InvalidStateError';}if(!uninitialized)throw 'factory must remain initially uninitialized';
log='';rejected=false;try{f.initUIEvent({toString:function(){log+='T';return 'bad';}},false,false,bad,{valueOf:function(){log+='N';return 3;}});}catch(x){rejected=x.name==='TypeError';}
if(!rejected||log!=='T'||f.type!=='')throw 'legacy positional view check precedes detail coercion';
log='';f.initUIEvent({toString:function(){log+='T';return 'legacy';}},true,true,w,{valueOf:function(){log+='N';return -4294967297;}});
if(log!=='TN'||f.view!==w||f.detail!==-1||f.which!==0||!b.dispatchEvent(f))throw 'actual legacy positional UI initialization';
var composed=new UIEvent('composed',{composed:true,detail:7,view:window}),stamp=composed.timeStamp;
composed.initUIEvent('preserved',false,false,null,4);if(!composed.composed||composed.timeStamp!==stamp||composed.detail!==4||composed.view!==null)throw 'legacy init preserves constructor composed and timestamp';
var converted=0,foreignInit={get detail(){converted++;return {valueOf:function(){converted++;return 5;}};},get view(){converted++;return w;}};
"#);
    let count=page.state.created_events.len();
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"var denied=false;try{new UIEvent('foreign-view',foreignInit);}catch(x){denied=x.name==='SecurityError';}if(!denied||converted!==3)throw 'retained view origin policy after ordered earlier member conversions';");
    assert_eq!(page.state.created_events.len(),count);
}

#[test]
fn ui_event_canonical_owner_realms_saved_globals_and_child_clock_are_genuine() {
    let (mut doc,mut forms,mut page)=fixture("");
    page.state.clock_origin=Instant::now().checked_sub(Duration::from_secs(3600)).unwrap();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var C=UIEvent,P=C.prototype,RootFunction=Function,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,ChildC=w.UIEvent,ChildP=ChildC.prototype,ChildEvent=w.Event,ChildFunction=w.Function,ChildObject=w.Object;
if(Object.getPrototypeOf(C)!==Event||Object.getPrototypeOf(ChildC)!==ChildEvent||Object.getPrototypeOf(P)!==Event.prototype||Object.getPrototypeOf(ChildP)!==ChildEvent.prototype||Object.getPrototypeOf(ChildP)===Event.prototype)throw 'real realm constructor/prototype inheritance';
var child=new ChildC('child-clock',{view:w,detail:11}),main=new C('main-clock',{view:window,detail:13});
if(Object.getPrototypeOf(child)!==ChildP||child.constructor!==ChildC||!(child instanceof ChildC)||!(child instanceof ChildEvent)||child instanceof C||Object.getPrototypeOf(main)!==P)throw 'owner instance roots';
if(main.timeStamp-child.timeStamp<3000000||child.timeStamp<0||main.timeStamp%1||child.timeStamp%1)throw 'actual child context timestamp origin';
var read=ChildFunction('event','return event.initUIEvent;'),method=read(main),get=Object.getOwnPropertyDescriptor(P,'detail').get;
if(method!==P.initUIEvent||Object.getPrototypeOf(method)!==RootFunction.prototype||Object.getPrototypeOf(get)!==RootFunction.prototype)throw 'owner first child lookup and getter realm';
var stamp=child.timeStamp;method.call(child,'borrowed',true,true,w,19);
if(child.detail!==19||child.view!==w||child.timeStamp!==stamp||child.isTrusted)throw 'real borrowed UI initializer';
ChildFunction('UIEvent=17;Function=19;')();UIEvent=23;
var childAgain=new ChildC('saved-child'),mainAgain=new C('saved-main');
if(Object.getPrototypeOf(childAgain)!==ChildP||Object.getPrototypeOf(mainAgain)!==P||childAgain.constructor!==ChildC||mainAgain.constructor!==C)throw 'saved constructors use canonical roots after replacement';
delete window.UIEvent;if(typeof UIEvent!=='undefined'||Object.getPrototypeOf(new C('deleted-global'))!==P)throw 'global deletion cannot alter canonical interface';
"#);
    evaluate(&mut page,&mut doc,&mut forms,"if(typeof UIEvent!=='undefined'||Object.getPrototypeOf(new C('deleted-next-job'))!==P||Object.getPrototypeOf(new ChildC('saved-child-next-job'))!==ChildP)throw 'deleted globals and saved canonical UI roots persist across jobs';");
}

#[test]
fn ui_event_foreign_fresh_cached_constructors_getters_and_methods_stop_before_coercion() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var main=new UIEvent('kept',{detail:29,view:window}),get=Object.getOwnPropertyDescriptor(UIEvent.prototype,'detail').get,init=main.initUIEvent;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,ChildC=w.UIEvent,ChildP=ChildC.prototype,ChildFunction=w.Function;
var read=ChildFunction('e','return e.initUIEvent;'),getSaved=ChildFunction('e','g','return g.call(e);'),initSaved=ChildFunction('e','f','t','return f.call(e,t);');
var childOwn=ChildFunction('childView','var event=new UIEvent("child-owned",{detail:37,view:childView});return event.detail===37&&event.view===childView&&event instanceof UIEvent;');
if(read(main)!==init||getSaved(main,get)!==29)throw 'same-origin real cached access';
var converted=0,type={toString:function(){converted++;return 'converted';}},options={get detail(){return {valueOf:function(){converted++;return 31;}};},get view(){converted++;return window;}};
"#);
    let count=page.state.created_events.len();
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["new w.UIEvent(type,options);","new ChildC(type,options);","read(main);","getSaved(main,get);","initSaved(main,init,type);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"{}",page.report());
        assert_eq!(page.state.created_events.len(),count);
        evaluate(&mut page,&mut doc,&mut forms,"if(converted||main.type!=='kept'||main.detail!==29||main.view!==window)throw 'foreign refusal precedes conversion and event mutation';");
    }
    // The main getter first leaves a genuine main-owned native operation behind.
    // The child constructs using its real Window captured while same-origin;
    // the view check must use its constructor owner, not that stale native caller.
    // Own child native-global reads after parent-origin changes are a separate
    // existing frame_get limitation, outside this UI constructor assertion.
    evaluate(&mut page,&mut doc,&mut forms,"if(main.detail!==29||!childOwn(w)||converted)throw 'child constructor owner must govern its view policy';");
    assert_eq!(page.state.created_events.len(),count+1);
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=new ChildC(type,options);if(converted!==3||recovered.detail!==31||recovered.view!==window||Object.getPrototypeOf(recovered)!==ChildP||read(main)!==init||getSaved(main,get)!==29)throw 'actual same-origin recovery and saved canonical identity';");
}

#[test]
fn ui_event_real_mouse_and_native_keyboard_support_ui_ancestry_and_borrowing() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var P=UIEvent.prototype,getView=Object.getOwnPropertyDescriptor(P,'view').get,getDetail=Object.getOwnPropertyDescriptor(P,'detail').get,getWhich=Object.getOwnPropertyDescriptor(P,'which').get;
var mouse=document.createEvent('MouseEvent');mouse.initMouseEvent('mouse',true,true,window,4,1,2,5,7,false,false,false,false,1);
if(!(mouse instanceof UIEvent)||!(mouse instanceof Event)||!P.isPrototypeOf(mouse)||getView.call(mouse)!==window||getDetail.call(mouse)!==4||getWhich.call(mouse)!==2)throw 'actual Mouse UI ancestry and slots';
P.initUIEvent.call(mouse,'borrowed',false,false,null,9);if(mouse.clientX!==5||mouse.clientY!==7||mouse.button!==1||mouse.which!==2||mouse.detail!==9||mouse.view!==null)throw 'borrowed UI method preserves Mouse state';
var base=new Event('base'),custom=document.createEvent('CustomEvent');custom.initCustomEvent('custom');
if(base instanceof UIEvent||custom instanceof UIEvent||typeof MouseEvent!=='function'||typeof KeyboardEvent!=='function')throw 'real Mouse and deferred classes with genuine negative brands';
var refused=0;for(var invalid of [base,custom,{},Object.create(P),null,3]){try{getView.call(invalid);}catch(x){if(x.name==='TypeError')refused++;}}if(refused!==6)throw 'real UI getter brand';
var b=document.getElementById('button'),native=0;
b.addEventListener('keydown',function(e){
 native++;if(!(e instanceof UIEvent)||!(e instanceof Event)||getView.call(e)!==window||getDetail.call(e)!==0||getWhich.call(e)!==65||!e.isTrusted)throw 'actual keyboard UI state';
 var stamp=e.timeStamp,kind=e.type;P.initUIEvent.call(e,'ignored',false,false,null,17);
 if(e.type!==kind||e.timeStamp!==stamp||e.detail!==0||e.view!==window||!e.isTrusted)throw 'native initializer no-op';Event.prototype.preventDefault.call(e);
});
var dispatchRefused=false;try{b.dispatchEvent(mouse);}catch(x){dispatchRefused=x.name==='NotSupportedError';}if(!dispatchRefused)throw 'Mouse dispatch remains explicit deferred boundary';
"#);
    evaluate(&mut page,&mut doc,&mut forms,"if(Object.getPrototypeOf(mouse)!==MouseEvent.prototype||!(mouse instanceof MouseEvent))throw 'genuine MouseEvent factory interface';");
    let node=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("button")).unwrap();
    page.begin_job();
    let outcome=page.keyboard(&mut doc,&mut forms,KeyboardInput {down:true,target:Some(node),key:"a".into(),code:"KeyA".into(),key_code:65,..KeyboardInput::default()});
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert!(outcome.prevented);
    evaluate(&mut page,&mut doc,&mut forms,"if(native!==1||mouse.clientX!==5||mouse.detail!==9)throw 'native and constructed UI isolation';");
}

#[test]
fn ui_event_small_profiles_and_actual_postcreate_accessor_failure_roll_back_and_recover() {
    let (mut doc,mut forms,mut page)=fixture("var C=UIEvent,kept=new C('kept',{detail:41,view:window}),factoryKept=document.createEvent('UIEvent');factoryKept.initUIEvent('factory-kept',false,false,window,43);");
    let count=page.state.created_events.len();
    let retained=page.state.retained;
    let capacity=page.state.limits.host.created_events;
    page.state.limits.host.created_events=count;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('capacity-refused');");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(page.state.created_events.len(),count);
    assert_eq!(page.state.retained,retained);
    page.state.limits.host.created_events=capacity;
    let type_limit=page.state.limits.host.event_type_bytes;
    page.state.limits.host.event_type_bytes=3;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('type-refused');");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(page.state.created_events.len(),count);
    assert_eq!(page.state.retained,retained);
    page.state.limits.host.event_type_bytes=type_limit;
    let retained_limit=page.state.retained_limit;
    page.state.retained_limit=retained;
    let outcome=page.console_eval(&mut doc,&mut forms,"new C('retained-refused');");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(page.state.created_events.len(),count);
    assert_eq!(page.state.retained,retained);
    page.state.retained_limit=retained_limit;
    for source in ["new C('store-unpublished');","document.createEvent('UIEvent');"] {
        let next=page.state.next_event;
        let normal=page.runtime.event_property_allowance_for_test(0);
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"Actual new UI instance unforgeable accessor storage must refuse.");
        assert_eq!(page.state.created_events.len(),count,"No hidden UI event survives failed publication.");
        assert_eq!(page.state.retained,retained);
        assert_eq!(page.state.next_event,next+1,"The withdrawn new identity must not be reused.");
        page.runtime.event_property_allowance_for_test(normal);
    }
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=new C('recovered',{detail:47,view:window}),factory=document.createEvent('UIEvent');factory.initUIEvent('factory-recovered',true,true,null,53);if(kept.detail!==41||kept.view!==window||factoryKept.detail!==43||recovered.detail!==47||recovered.view!==window||factory.detail!==53||factory.view!==null||Object.getPrototypeOf(recovered)!==C.prototype||Object.getPrototypeOf(factory)!==C.prototype||recovered.isTrusted||factory.isTrusted)throw 'real storage and profile recovery';");
    assert_eq!(page.state.created_events.len(),count+2);
}
