//! Original base Event Page/Runtime regression tests; no service I/O.
use super::*;

#[test]
fn base_event_factory_store_failure_preserves_previous_events_and_consumes_only_new_identity() {
    let (mut doc,mut forms,mut page)=fixture("var kept=new Event('kept'),factoryKept=document.createEvent('Event');factoryKept.initEvent('factory-kept');");
    let count=page.state.created_events.len();
    let retained=page.state.retained;
    let next=page.state.next_event;
    let normal=page.runtime.event_property_allowance_for_test(0);
    let outcome=page.console_eval(&mut doc,&mut forms,"document.createEvent('Event');");
    assert_eq!(outcome.errors,1);
    assert_eq!(page.state.created_events.len(),count);
    assert_eq!(page.state.retained,retained);
    assert_eq!(page.state.next_event,next+1);
    page.runtime.event_property_allowance_for_test(normal);
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=document.createEvent('Event');recovered.initEvent('factory-recovered');if(kept.type!=='kept'||factoryKept.type!=='factory-kept'||recovered.type!=='factory-recovered'||recovered.isTrusted)throw 'real factory store rollback recovery';");
}

#[test]
fn base_event_saved_getter_and_initializer_refuse_foreign_reader_before_type_coercion() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var main=new Event('kept'),get=Object.getOwnPropertyDescriptor(Event.prototype,'type').get,init=main.initEvent;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var ChildFunction=frame.contentWindow.Function,read=ChildFunction('e','return e.initEvent;');
var getSaved=ChildFunction('e','g','return g.call(e);'),initSaved=ChildFunction('e','f','t','return f.call(e,t);');
var conversions=0,type={toString:function(){conversions++;return 'changed';}};
if(read(main)!==init||getSaved(main,get)!=='kept')throw 'real same-origin cached getter and method';
"#);
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["read(main);","getSaved(main,get);","initSaved(main,init,type);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"Foreign fresh/cached reads and saved invocation must decline.");
        evaluate(&mut page,&mut doc,&mut forms,"if(conversions!==0||main.type!=='kept')throw 'foreign native access preceded type conversion and mutation';");
    }
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"initSaved(main,init,type);if(conversions!==1||getSaved(main,get)!=='changed'||read(main)!==init)throw 'same-origin native recovery';");
}

#[test]
fn base_event_native_composition_comes_from_click_keyboard_and_load_producers() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var P=Event.prototype,getComposed=Object.getOwnPropertyDescriptor(P,'composed').get;
var button=document.getElementById('button'),clicks=0,keys=0,loads=0,domLoads=0;
document.addEventListener('DOMContentLoaded',function(e){if(!e.isTrusted||e.composed||getComposed.call(e))throw 'real noncomposed DOMContentLoaded';domLoads++;});
window.addEventListener('load',function(e){if(!e.isTrusted||e.composed||getComposed.call(e))throw 'real noncomposed load';loads++;});
button.addEventListener('click',function(e){if(!e.isTrusted||!e.composed||!getComposed.call(e))throw 'real composed native click';clicks++;});
function key(e){if(!e.isTrusted||!e.composed||!getComposed.call(e))throw 'real composed native key';keys++;}
button.addEventListener('keydown',key);button.addEventListener('keyup',key);
var created=new Event('click'),selected=new Event('load',{composed:true}),legacy=document.createEvent('Event');
legacy.initEvent('click',true,true);
if(created.composed||legacy.composed||!selected.composed)throw 'event names must not fabricate composed state';
"#);
    let node=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("button")).unwrap();
    page.begin_job();
    let outcome=page.dispatch(&mut doc,&mut forms,node,"click",true);
    assert_eq!(outcome.errors,0,"{}",page.report());
    for down in [true,false] {
        page.begin_job();
        let outcome=page.keyboard(&mut doc,&mut forms,KeyboardInput {down,target:Some(node),key:"a".into(),code:"KeyA".into(),key_code:65,..KeyboardInput::default()});
        assert_eq!(outcome.errors,0,"{}",page.report());
    }
    evaluate(&mut page,&mut doc,&mut forms,"if(clicks!==1||keys!==2||loads!==1||domLoads!==1||created.composed||legacy.composed||!selected.composed)throw 'actual producer composition and created-event isolation';");
}

#[test]
fn base_event_normal_profile_admits_real_events_beyond_small_regression_boundaries() {
    let mut doc=crate::html::parse("<html><body></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let outcome=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original generous real Event admission".into(),source:r#"
var longType='';for(var i=0;i<1024;i++)longType+='x';
var first=new Event(longType,{composed:true}),last;
for(var i=0;i<300;i++)last=new Event('event-'+i,{cancelable:true});
if(first.type!==longType||!first.composed||last.type!=='event-299'||last.isTrusted||Object.getPrototypeOf(last)!==Event.prototype)throw 'real generous Event default';
"#.into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(page.state.created_events.len(),301);
    assert_eq!(page.state.created_events[0].name.len(),1024);
}

#[test]
fn base_event_ordinary_prototype_ancestry_is_real_and_native_mutation_stays_explicit() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var P=Event.prototype,e=new Event('actual-chain'),root=Object.prototype;
var ancestor={inherited:83};Object.setPrototypeOf(P,ancestor);
if(e.inherited!==83||!(e instanceof Event)||!(e instanceof Object)||!ancestor.isPrototypeOf(e))throw 'actual stored ordinary parent chain';
Object.setPrototypeOf(P,null);
if(!(e instanceof Event)||e instanceof Object||Object.prototype.isPrototypeOf.call(root,e))throw 'actual detached ordinary prototype parent';
Object.setPrototypeOf(P,root);
if(!(e instanceof Object)||e.type!=='actual-chain')throw 'ordinary chain restored';
"#);
    let outcome=page.console_eval(&mut doc,&mut forms,"Object.setPrototypeOf(e,{});");
    assert_eq!(outcome.errors,1,"Native Host prototype mutation remains the existing explicit unsupported boundary.");
    evaluate(&mut page,&mut doc,&mut forms,"if(Object.getPrototypeOf(e)!==P||!(e instanceof Event)||e.type!=='actual-chain')throw 'refused native mutation must preserve real chain';");
}

#[test]
fn base_event_child_timestamp_uses_its_real_context_origin_not_parent_origin() {
    let (mut doc,mut forms,mut page)=fixture("");
    // Distinct real monotonic origins without sleeping or inventing event time.
    page.state.clock_origin=Instant::now().checked_sub(Duration::from_secs(3600)).unwrap();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var child=new frame.contentWindow.Event('child-clock'),parent=new Event('parent-clock');
if(parent.timeStamp-child.timeStamp<3000000||child.timeStamp<0||parent.timeStamp%1||child.timeStamp%1)throw 'actual owner context origin and existing millisecond coarsening';
var stamp=child.timeStamp;Event.prototype.initEvent.call(child,'preserved-clock');
if(child.timeStamp!==stamp)throw 'initialization must preserve timestamp';
"#);
}

#[test]
fn base_event_unpublished_store_failure_rolls_back_real_record_without_reusing_id() {
    let (mut doc,mut forms,mut page)=fixture("var kept=new Event('kept');");
    let count=page.state.created_events.len();
    let retained=page.state.retained;
    let next=page.state.next_event;
    let normal=page.runtime.event_property_allowance_for_test(0);
    let outcome=page.console_eval(&mut doc,&mut forms,"new Event('unpublished');");
    assert_eq!(outcome.errors,1,"Real per-instance isTrusted accessor storage must refuse under the small profile.");
    assert_eq!(page.state.created_events.len(),count,"No failed hidden Event record survives.");
    assert_eq!(page.state.retained,retained);
    assert_eq!(page.state.next_event,next+1,"A withdrawn unpublished capability ID is never reused.");
    page.runtime.event_property_allowance_for_test(normal);
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=new Event('recovered');if(kept.type!=='kept'||recovered.type!=='recovered'||recovered.isTrusted||Object.getPrototypeOf(recovered)!==Event.prototype)throw 'real store recovery';");
    assert_eq!(page.state.created_events.len(),count+1);
}

fn fixture(source: &str) -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<html><head></head><body><div id=parent><button id=button type=button>Run</button></div></body></html>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::for_test();
    page.set_location(&crate::navigation::Location {
        url:Some(crate::url::parse("https://example.test/page").unwrap().0),
        path:None,root:None,fragment:String::new(),
    });
    let outcome=page.start(&mut doc,&mut forms,&[Script {
        node:0,label:"Original genuine base Event control".into(),source:source.into(),
    }]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    (doc,forms,page)
}

fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let outcome=page.console_eval(doc,forms,source);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn base_event_real_constructor_descriptors_unforgeable_and_ordinary_overrides() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var C=Event,P=C.prototype,e=new C('original',{bubbles:true,cancelable:true,composed:true});
if(typeof C!=='function'||C.length!==1||C.name!=='Event'||Object.getPrototypeOf(C)!==Function.prototype||Object.getPrototypeOf(P)!==Object.prototype)throw 'real interface ancestry';
if(Object.getPrototypeOf(e)!==P||e.constructor!==C||!(e instanceof C)||Object.prototype.toString.call(e)!=='[object Event]')throw 'actual instance provenance';
if(e.type!=='original'||!e.bubbles||!e.cancelable||!e.composed||e.isTrusted||e.target!==null||e.srcElement!==null||e.currentTarget!==null||e.eventPhase!==0||e.defaultPrevented||e.cancelBubble||!e.returnValue)throw 'real initial slots';
var global=Object.getOwnPropertyDescriptor(window,'Event'),prototype=Object.getOwnPropertyDescriptor(C,'prototype'),constructor=Object.getOwnPropertyDescriptor(P,'constructor');
if(global.enumerable||!global.writable||!global.configurable||prototype.writable||prototype.enumerable||prototype.configurable||constructor.enumerable||!constructor.writable||!constructor.configurable)throw 'interface descriptors';
var trust=Object.getOwnPropertyDescriptor(e,'isTrusted');
if(typeof trust.get!=='function'||trust.get.length!==0||trust.set!==undefined||!trust.enumerable||trust.configurable||Object.getOwnPropertyDescriptor(P,'isTrusted')!==undefined||Object.keys(e).join(',')!=='isTrusted')throw 'own LegacyUnforgeable getter';
for(var key of ['type','target','srcElement','currentTarget','eventPhase','bubbles','cancelable','defaultPrevented','composed','timeStamp']){
 var d=Object.getOwnPropertyDescriptor(P,key);
 if(typeof d.get!=='function'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||Object.prototype.hasOwnProperty.call(e,key))throw 'real inherited readonly getter '+key;
}
for(var key of ['cancelBubble','returnValue']){var d=Object.getOwnPropertyDescriptor(P,key);if(typeof d.get!=='function'||typeof d.set!=='function'||d.set.length!==1||!d.enumerable||!d.configurable)throw 'real inherited setter';}
for(var key of ['stopPropagation','stopImmediatePropagation','preventDefault','composedPath','initEvent']){
 var d=Object.getOwnPropertyDescriptor(P,key);if(typeof d.value!=='function'||!d.writable||!d.enumerable||!d.configurable||d.value.length!==(key==='initEvent'?1:0)||e[key]!==P[key])throw 'canonical method '+key;
}
for(var item of [['NONE',0],['CAPTURING_PHASE',1],['AT_TARGET',2],['BUBBLING_PHASE',3]])for(var target of [C,P]){var d=Object.getOwnPropertyDescriptor(target,item[0]);if(d.value!==item[1]||d.writable||!d.enumerable||d.configurable)throw 'real phase constant';}
var caught=0;try{Object.defineProperty(e,'isTrusted',{value:true});}catch(x){if(x.name==='TypeError')caught++;}try{(function(){'use strict';e.isTrusted=true;})();}catch(x){if(x.name==='TypeError')caught++;}if(caught!==2||e.isTrusted||delete e.isTrusted)throw 'unforgeable refusal';
e.type='ignored';caught=false;try{(function(){'use strict';e.type='forbidden';})();}catch(x){caught=x.name==='TypeError';}if(!caught||e.type!=='original')throw 'readonly assignment';
var typeGetter=Object.getOwnPropertyDescriptor(P,'type').get;
Object.defineProperty(e,'type',{value:'own-shadow',writable:true,configurable:true});
if(e.type!=='own-shadow'||typeGetter.call(e)!=='original')throw 'ordinary own shadow does not mutate slot';
var actualDispatch=0,button=document.getElementById('button');
button.addEventListener('original',function(actual){if(actual!==e||actual.type!=='own-shadow'||typeGetter.call(actual)!=='original')throw 'dispatch reads actual slots without invoking shadow';actualDispatch++;});
if(!button.dispatchEvent(e)||actualDispatch!==1)throw 'real dispatch type must remain internal';delete e.type;
var saved=P.preventDefault,calls=0;P.preventDefault=function(){calls++;return this;};
if(e.preventDefault()!==e||calls!==1||e.defaultPrevented)throw 'prototype override observed';P.preventDefault=saved;
e.preventDefault=function(){return 'own';};if(e.preventDefault()!=='own')throw 'own override';delete e.preventDefault;
if(e.preventDefault!==saved)throw 'own deletion restored prototype';saved.call(e);if(!e.defaultPrevented)throw 'restored genuine method';
"#);
    evaluate(&mut page,&mut doc,&mut forms,"if(e.type!=='original'||!e.composed||!e.defaultPrevented)throw 'slots survived entry';");
    assert_eq!(page.state.created_events.len(),1);
    assert!(page.state.created_events[0].initialized);
}

#[test]
fn base_event_constructor_conversion_order_defaults_and_reentrant_throw_are_atomic() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var log='',truth={valueOf:function(){throw 'boolean numeric conversion';},toString:function(){throw 'boolean string conversion';}};
var type={toString:function(){log+='T';return 'ordered';}},proto={};
Object.defineProperty(proto,'bubbles',{get:function(){if(this!==dict)throw 'dictionary getter receiver';log+='B';return truth;}});
Object.defineProperty(proto,'cancelable',{get:function(){log+='C';return truth;}});
Object.defineProperty(proto,'composed',{get:function(){log+='O';return truth;}});
Object.defineProperty(proto,'ignored',{get:function(){throw 'unknown dictionary member';}});
var dict=Object.create(proto),e=new Event(type,dict,{toString:function(){throw 'unused third argument';}});
if(log!=='TBCO'||e.type!=='ordered'||!e.bubbles||!e.cancelable||!e.composed)throw 'ordered actual conversions';
for(var init of [undefined,null]){var d=new Event('defaults',init);if(d.bubbles||d.cancelable||d.composed||d.isTrusted)throw 'dictionary defaults';}
if(new Event(undefined).type!=='undefined'||new Event(null).type!=='null')throw 'required DOMString values';
var catches=0,conversions=0,badType={toString:function(){conversions++;return 'x';}};
try{Event(badType,dict);}catch(x){if(x.name==='TypeError')catches++;}
try{new Event();}catch(x){if(x.name==='TypeError')catches++;}
try{new Event(Symbol(),{get bubbles(){conversions++;return false;}});}catch(x){if(x.name==='TypeError')catches++;}
for(var init of [true,false,0,1,'wrong',Symbol()]){try{new Event('invalid',init);}catch(x){if(x.name==='TypeError')catches++;}}
if(catches!==9||conversions!==0)throw 'constructor and dictionary refusal before side effects';
var marker={},nested,tail=0;
"#);
    let before=page.state.created_events.len();
    evaluate(&mut page,&mut doc,&mut forms,r#"
var failure=false;
try{new Event('unpublished',{
 get bubbles(){nested=new Event('nested-real',{composed:true});return true;},
 get cancelable(){throw marker;},
 get composed(){tail++;return true;}
});}catch(x){failure=x===marker;}
if(!failure||tail||nested.type!=='nested-real'||!nested.composed)throw 'reentrant conversion effects and thrown identity';
"#);
    assert_eq!(page.state.created_events.len(),before+1,"Only the real nested constructor may publish.");
    assert!(page.state.created_events.iter().all(|event|event.name!="unpublished"));
}

#[test]
fn base_event_saved_main_and_child_constructors_keep_canonical_owner_realms() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var RootEvent=Event,RootPrototype=Event.prototype,RootFunction=Function,RootObject=Object;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
var ChildEvent=w.Event,ChildPrototype=ChildEvent.prototype,ChildFunction=w.Function,ChildObject=w.Object;
if(ChildEvent===RootEvent||ChildPrototype===RootPrototype||Object.getPrototypeOf(ChildEvent)!==ChildFunction.prototype||Object.getPrototypeOf(ChildPrototype)!==ChildObject.prototype)throw 'distinct canonical child roots';
var child=new ChildEvent('child',{composed:true}),main=new RootEvent('main');
if(Object.getPrototypeOf(child)!==ChildPrototype||!(child instanceof ChildEvent)||child instanceof RootEvent||Object.getPrototypeOf(main)!==RootPrototype||main instanceof ChildEvent)throw 'owner instance roots';
var read=ChildFunction('e','return e.initEvent;'),method=read(main);
if(Object.getPrototypeOf(method)!==RootFunction.prototype||method!==RootPrototype.initEvent)throw 'owner first child method read';
ChildFunction('Event=17;Function=19;')();Event=23;
var childAgain=new ChildEvent('saved-child'),mainAgain=new RootEvent('saved-main');
if(Object.getPrototypeOf(childAgain)!==ChildPrototype||Object.getPrototypeOf(mainAgain)!==RootPrototype||childAgain.constructor!==ChildEvent||mainAgain.constructor!==RootEvent)throw 'saved constructors ignore replaced globals';
delete window.Event;if(typeof Event!=='undefined'||Object.getPrototypeOf(new RootEvent('after-delete'))!==RootPrototype)throw 'deleted global does not replace canonical roots';
"#);
    evaluate(&mut page,&mut doc,&mut forms,"if(typeof Event!=='undefined'||Object.getPrototypeOf(new RootEvent('after-entry'))!==RootPrototype)throw 'deleted global and saved canonical root survive next entry';method.call(main,'borrowed-after-entry',true,true);if(main.type!=='borrowed-after-entry'||!main.bubbles||main.isTrusted||child.type!=='child')throw 'real owner method recovery';");
}

#[test]
fn base_event_foreign_constructor_lookup_and_saved_invocation_refuse_before_coercion() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,ChildEvent=w.Event,ChildPrototype=ChildEvent.prototype;
var converted=0,type={toString:function(){converted++;return 'blocked';}},init={get bubbles(){converted++;return true;},get cancelable(){converted++;return true;},get composed(){converted++;return true;}};
"#);
    let count=page.state.created_events.len();
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["new w.Event(type,init);","new ChildEvent(type,init);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"{}",page.report());
        assert_eq!(page.state.created_events.len(),count);
        evaluate(&mut page,&mut doc,&mut forms,"if(converted!==0)throw 'foreign constructor coerced arguments';");
    }
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/page").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=new ChildEvent(type,init);if(converted!==4||recovered.type!=='blocked'||!recovered.bubbles||!recovered.composed||Object.getPrototypeOf(recovered)!==ChildPrototype)throw 'same-origin constructor recovery';");
}

#[test]
fn base_event_borrowed_brands_cover_real_opaque_subclasses_and_native_keyboard() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var base=new Event('base'),P=Event.prototype,getType=Object.getOwnPropertyDescriptor(P,'type').get,getTrust=Object.getOwnPropertyDescriptor(base,'isTrusted').get;
var ui=document.createEvent('UIEvent'),mouse=document.createEvent('MouseEvent'),custom=document.createEvent('CustomEvent'),payload={value:79};
ui.initUIEvent('ui',true,true,window,3);mouse.initMouseEvent('mouse',true,true,window,4,1,2,5,7);custom.initCustomEvent('custom',true,true,payload);
for(var e of [ui,mouse,custom]){if(!(e instanceof Event)||!Event.prototype.isPrototypeOf(e)||getType.call(e)!==e.type||getTrust.call(e))throw 'genuine inherited Event brand';P.preventDefault.call(e);if(!e.defaultPrevented)throw 'borrowed base action';}
P.initEvent.call(mouse,'mouse-renamed',false,false);if(mouse.type!=='mouse-renamed'||mouse.clientX!==5||mouse.detail!==4)throw 'base init preserves subclass slots';
if(custom.detail!==payload||typeof UIEvent!=='function'||typeof MouseEvent!=='function'||typeof CustomEvent!=='undefined')throw 'real UI/Mouse and explicit deferred constructor families';
var falseBrands=0;for(var invalid of [{},Object.create(P),null,undefined,3]){try{getType.call(invalid);}catch(x){if(x.name==='TypeError')falseBrands++;}}
if(falseBrands!==5)throw 'prototype does not fabricate Event brand';
var button=document.getElementById('button'),nativeCalls=0;
button.addEventListener('keydown',function(e){
 nativeCalls++;if(getType.call(e)!=='keydown'||!getTrust.call(e)||!(e instanceof Event))throw 'genuine native base brand';
 var type=e.type,stamp=e.timeStamp,conversions=0;
 P.initEvent.call(e,{toString:function(){conversions++;return 'ignored';}},false,false);
 if(conversions!==1||e.type!==type||e.timeStamp!==stamp||!e.isTrusted)throw 'native dispatch initialization no-op';
 P.preventDefault.call(e);if(!e.defaultPrevented)throw 'native borrowed cancellation';
});
"#);
    evaluate(&mut page,&mut doc,&mut forms,"if(Object.getPrototypeOf(ui)!==UIEvent.prototype||!(ui instanceof UIEvent))throw 'genuine UIEvent factory interface';");
    evaluate(&mut page,&mut doc,&mut forms,"if(Object.getPrototypeOf(mouse)!==MouseEvent.prototype||!(mouse instanceof MouseEvent))throw 'genuine MouseEvent factory interface';");
    for source in ["Object.getPrototypeOf(custom);"] {
        let outcome=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(outcome.errors,1,"Deferred immediate subclass prototypes stay explicitly opaque.");
    }
    let node=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("button")).unwrap();
    page.begin_job();
    let outcome=page.keyboard(&mut doc,&mut forms,KeyboardInput {down:true,target:Some(node),key:"a".into(),code:"KeyA".into(),key_code:65,..KeyboardInput::default()});
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert!(outcome.prevented);
    evaluate(&mut page,&mut doc,&mut forms,"if(nativeCalls!==1||custom.detail!==payload||mouse.type!=='mouse-renamed')throw 'trusted dispatch and subclass isolation';");
}

#[test]
fn base_event_real_composed_path_aliases_cancellation_and_factory_initialization() {
    let (_doc,_forms,page)=fixture(r#"
var b=document.getElementById('button'),p=document.getElementById('parent'),e=new Event('',{bubbles:true,cancelable:true,composed:true}),factory=document.createEvent('Event');
var invalid=false;try{b.dispatchEvent(factory);}catch(x){invalid=x.name==='InvalidStateError';}if(!invalid||!b.dispatchEvent(e)||e.type!=='')throw 'factory versus initialized empty constructor';
factory.initEvent('factory');if(!b.dispatchEvent(factory)||Object.getPrototypeOf(factory)!==Event.prototype)throw 'real base factory prototype';
var expected=[b,p,document.body,document.documentElement,document,window],log='';
e.initEvent('path',true,true);var stamp=e.timeStamp;if(!e.composed||e.composedPath().length)throw 'init preserves composed and empty outside dispatch';
function actualPath(event){var first=event.composedPath();if(!Array.isArray(first)||first.length!==expected.length)throw 'actual complete path';for(var i=0;i<expected.length;i++)if(first[i]!==expected[i])throw 'actual path identity';first[0]=window;if(event.composedPath()[0]!==b)throw 'fresh path snapshot';}
p.addEventListener('path',function(event){actualPath(event);if(event.srcElement!==b||event.target!==b||event.currentTarget!==p||event.eventPhase!==1||event.defaultPrevented)throw 'capture aliases';event.preventDefault();if(event.defaultPrevented)throw 'passive cancellation';log+='c';},{capture:true,passive:true});
b.addEventListener('path',function(event){actualPath(event);event.returnValue=false;if(event.returnValue||!event.defaultPrevented||event.isTrusted)throw 'genuine returnValue cancellation';event.returnValue=true;if(event.returnValue)throw 'returnValue cannot clear cancellation';log+='t';});
p.addEventListener('path',function(event){actualPath(event);log+='b';});
if(b.dispatchEvent(e)!==false||log!=='ctb'||e.target!==b||e.srcElement!==b||e.currentTarget!==null||e.eventPhase!==0||e.composedPath().length)throw 'dispatch result cleanup';
e.initEvent('reset');if(!e.composed||e.defaultPrevented||!e.returnValue||e.target!==null||e.srcElement!==null||e.timeStamp!==stamp)throw 'real init reset preserves constructor composed/time';
e.cancelBubble=true;e.cancelBubble=false;if(!e.cancelBubble)throw 'false cancelBubble must not clear';e.initEvent('final');if(e.cancelBubble)throw 'init resets propagation';
"#);
    assert!(page.state.synthetic_dispatches.is_empty());
    assert!(page.state.created_events.iter().all(|event|!event.dispatching));
}

#[test]
fn base_event_ordinary_listener_errors_continue_but_fatal_jobs_cancel_stale_tail() {
    let (mut doc,mut forms,mut page)=fixture(r#"
var b=document.getElementById('button'),ordinary=new Event('ordinary',{cancelable:true}),log='',caught=false;
b.addEventListener('ordinary',function(){log+='a';queueMicrotask(function(){log+='m';});throw Error('original ordinary listener');});
b.addEventListener('ordinary',function(){log+='b';});
"#);
    let outcome=page.console_eval(&mut doc,&mut forms,"try{b.dispatchEvent(ordinary);}catch(x){caught=true;}log+='s';document.title='ordinary caller completed';");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(doc.title,"ordinary caller completed");
    assert!(page.state.synthetic_dispatches.is_empty());
    evaluate(&mut page,&mut doc,&mut forms,r#"
if(caught||log!=='absm'||ordinary.currentTarget!==null||ordinary.eventPhase!==0||ordinary.composedPath().length)throw 'ordinary exception ordering and cleanup';
var fatal=new Event('fatal'),stale=0,tail=0;
function fail(){queueMicrotask(function(){stale++;});document.body.scrollWidth;}
b.addEventListener('fatal',fail);b.addEventListener('fatal',function(){tail++;});
"#);
    let outcome=page.console_eval(&mut doc,&mut forms,"b.dispatchEvent(fatal);");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert!(page.state.synthetic_dispatches.is_empty());
    assert!(page.state.created_events.iter().all(|event|!event.dispatching&&!event.passive&&event.current.is_none()&&event.phase==0));
    evaluate(&mut page,&mut doc,&mut forms,"if(stale||tail||fatal.composedPath().length||fatal.currentTarget!==null||fatal.eventPhase!==0)throw 'fatal cleanup without stale callbacks';fatal.initEvent('recovered');if(!b.dispatchEvent(fatal)||fatal.isTrusted)throw 'fresh job dispatch recovery';");
}

#[test]
fn base_event_small_policy_admission_retained_and_identity_exhaustion_are_atomic() {
    let (mut doc,mut forms,mut page)=fixture("var kept=new Event('kept',{composed:true});");
    let count=page.state.created_events.len();
    let retained=page.state.retained;
    let normal_capacity=page.state.limits.host.created_events;
    page.state.limits.host.created_events=count;
    let before_id=page.state.next_event;
    let outcome=page.console_eval(&mut doc,&mut forms,"new Event('capacity-refused');");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(page.state.created_events.len(),count);
    assert_eq!(page.state.retained,retained);
    assert!(page.state.next_event>=before_id,"Unpublished allocation rollback must not rewind issued IDs.");
    page.state.limits.host.created_events=normal_capacity;
    let normal_type=page.state.limits.host.event_type_bytes;
    page.state.limits.host.event_type_bytes=3;
    let outcome=page.console_eval(&mut doc,&mut forms,"new Event('type-refused');");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(page.state.created_events.len(),count);
    assert_eq!(page.state.retained,retained);
    page.state.limits.host.event_type_bytes=normal_type;
    let normal_retained=page.state.retained_limit;
    page.state.retained_limit=retained;
    let outcome=page.console_eval(&mut doc,&mut forms,"new Event('retained-refused');");
    assert_eq!(outcome.errors,1,"{}",page.report());
    assert_eq!(page.state.created_events.len(),count);
    assert_eq!(page.state.retained,retained);
    page.state.retained_limit=normal_retained;
    evaluate(&mut page,&mut doc,&mut forms,"var recovered=new Event('recovered',{composed:true});if(kept.type!=='kept'||!kept.composed||recovered.type!=='recovered'||recovered.isTrusted||Object.getPrototypeOf(recovered)!==Event.prototype)throw 'real quota recovery';");
    assert_eq!(page.state.created_events.len(),count+1);
    let (mut overflow_doc,mut overflow_forms,mut overflow_page)=fixture("var kept=new Event('overflow-kept');");
    let overflow_count=overflow_page.state.created_events.len();
    let overflow_retained=overflow_page.state.retained;
    overflow_page.state.next_event=script_class_list::BASE;
    let exhausted=overflow_page.state.next_event;
    let outcome=overflow_page.console_eval(&mut overflow_doc,&mut overflow_forms,"new Event('identity-refused');");
    assert_eq!(outcome.errors,1,"{}",overflow_page.report());
    assert_eq!(overflow_page.state.created_events.len(),overflow_count);
    assert_eq!(overflow_page.state.retained,overflow_retained);
    assert_eq!(overflow_page.state.next_event,exhausted);
    // Exhaustion is terminal for this document, never repaired by rewinding IDs.
    let (_fresh_doc,_fresh_forms,fresh_page)=fixture("var fresh=new Event('fresh-document');if(fresh.type!=='fresh-document'||fresh.isTrusted)throw 'fresh document recovery';");
    assert_eq!(fresh_page.state.created_events.len(),1);
}
