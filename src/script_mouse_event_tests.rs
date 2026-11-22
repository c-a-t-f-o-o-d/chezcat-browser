//! Original finite MouseEvent Page/Runtime controls; no service or device IO.
use super::*;

fn fixture(source: &str) -> (Document, Forms, Page) {
    fixture_with_limits(source,crate::javascript::limits::CAPACITY_REGRESSION)
}
fn fixture_with_limits(source:&str,limits:crate::javascript::limits::Limits)->(Document,Forms,Page){
    let mut doc = crate::html::parse("<html><head></head><body><div id=parent><button id=button type=button>Run</button></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(limits);
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/page").unwrap().0),
        path: None, root: None, fragment: String::new(),
    });
    let outcome = page.start(&mut doc, &mut forms, &[Script {
        node: 0, label: "Original genuine MouseEvent control".into(), source: source.into(),
    }]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn evaluate(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn mouse_event_genuine_interface_slots_descriptors_modifiers_and_ordinary_overrides() {
    let (mut doc, mut forms, mut page) = fixture(r#"
var C=MouseEvent,P=C.prototype,b=document.getElementById('button');
var e=new C('mouse-real',{bubbles:true,cancelable:true,composed:true,view:window,detail:3,
 screenX:7.5,screenY:-8.25,clientX:1.5,clientY:-2.75,button:1,buttons:5,relatedTarget:b,
 altKey:true,ctrlKey:true,metaKey:true,shiftKey:true,modifierAltGraph:true,modifierCapsLock:true,
 modifierFn:true,modifierFnLock:true,modifierHyper:true,modifierNumLock:true,modifierScrollLock:true,
 modifierSuper:true,modifierSymbol:true,modifierSymbolLock:true});
if(typeof C!=='function'||C.name!=='MouseEvent'||C.length!==1||Object.getPrototypeOf(C)!==UIEvent||Object.getPrototypeOf(P)!==UIEvent.prototype)throw 'genuine Mouse interface inheritance';
if(Object.getPrototypeOf(e)!==P||e.constructor!==C||!(e instanceof C)||!(e instanceof UIEvent)||!(e instanceof Event)||Object.prototype.toString.call(e)!=='[object MouseEvent]')throw 'genuine Mouse instance chain';
if(e.type!=='mouse-real'||e.view!==window||e.detail!==3||!e.bubbles||!e.cancelable||!e.composed||e.isTrusted||e.target!==null||e.currentTarget!==null||e.defaultPrevented||e.screenX!==7.5||e.screenY!==-8.25||e.clientX!==1.5||e.clientY!==-2.75||e.button!==1||e.buttons!==5||e.relatedTarget!==b||e.which!==2||e.layerX!==0||e.layerY!==0)throw 'actual Mouse constructor slots';
for(var key of ['screenX','screenY','clientX','clientY','button','buttons','relatedTarget','ctrlKey','altKey','shiftKey','metaKey']){
 var d=Object.getOwnPropertyDescriptor(P,key);if(typeof d.get!=='function'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||Object.prototype.hasOwnProperty.call(e,key))throw 'canonical readonly Mouse getter '+key;
}
var trust=Object.getOwnPropertyDescriptor(e,'isTrusted'),init=Object.getOwnPropertyDescriptor(P,'initMouseEvent'),modifier=Object.getOwnPropertyDescriptor(P,'getModifierState');
if(typeof trust.get!=='function'||!trust.enumerable||trust.configurable||trust.set!==undefined||trust.get.call(e)||Object.getOwnPropertyDescriptor(P,'isTrusted')!==undefined)throw 'genuine own unforgeable trust';
if(init.value!==e.initMouseEvent||init.value.length!==1||!init.writable||!init.enumerable||!init.configurable||modifier.value!==e.getModifierState||modifier.value.length!==1||!modifier.writable||!modifier.enumerable||!modifier.configurable)throw 'canonical Mouse methods';
for(var name of ['Alt','Control','Meta','Shift','AltGraph','CapsLock','Fn','FnLock','Hyper','NumLock','ScrollLock','Super','Symbol','SymbolLock'])if(!e.getModifierState(name))throw 'retained real modifier '+name;
for(var name of ['alt','CTRL','Unknown',''])if(e.getModifierState(name))throw 'modifier names remain exact';
var conversions=0;if(!e.getModifierState({toString:function(){conversions++;return 'AltGraph';}})||conversions!==1)throw 'modifier DOMString conversion';
var get=Object.getOwnPropertyDescriptor(P,'clientX').get;
e.clientX=99;e.buttons=0;e.relatedTarget=null;if(e.clientX!==1.5||e.buttons!==5||e.relatedTarget!==b)throw 'readonly sloppy setters';
var strict=false;try{(function(){'use strict';e.clientX=99;})();}catch(x){strict=x.name==='TypeError';}if(!strict)throw 'readonly strict setter';
Object.defineProperty(e,'clientX',{value:'own',configurable:true});if(e.clientX!=='own'||get.call(e)!==1.5)throw 'ordinary shadow retains real slot';delete e.clientX;
var saved=P.getModifierState,overrides=0;P.getModifierState=function(){overrides++;return 'ordinary';};
if(e.getModifierState('Alt')!=='ordinary'||overrides!==1)throw 'prototype override observed';P.getModifierState=saved;
e.getModifierState=function(){return 'own';};if(e.getModifierState('Alt')!=='own')throw 'own method observed';delete e.getModifierState;if(e.getModifierState!==saved||!e.getModifierState('Alt'))throw 'delete restores canonical method';
var refused=0,base=new Event('base'),ui=new UIEvent('ui'),custom=document.createEvent('CustomEvent');
for(var invalid of [base,ui,custom,{},Object.create(P),null,3]){try{get.call(invalid);}catch(x){if(x.name==='TypeError')refused++;}}
if(refused!==7||base instanceof C||ui instanceof C||custom instanceof C)throw 'genuine Mouse getter brand';
"#);
    evaluate(&mut page, &mut doc, &mut forms, "if(e.clientX!==1.5||e.buttons!==5||!e.getModifierState('FnLock')||e.isTrusted)throw 'actual Mouse slots survive next entry';");
}

#[test]
fn mouse_event_dictionary_order_numeric_defaults_finiteness_and_reentrant_throw_are_real() {
    let (mut doc, mut forms, mut page) = fixture(r#"
var log=[],proto={},options=Object.create(proto),b=document.getElementById('button');
var names=['bubbles','cancelable','composed','detail','view','altKey','ctrlKey','metaKey','modifierAltGraph','modifierCapsLock','modifierFn','modifierFnLock','modifierHyper','modifierNumLock','modifierScrollLock','modifierSuper','modifierSymbol','modifierSymbolLock','shiftKey','button','buttons','clientX','clientY','relatedTarget','screenX','screenY'];
var numbers={detail:4294967297,button:65537,buttons:65537,clientX:-0.75,clientY:2.5,screenX:9.5,screenY:-3.25};
function install(key){Object.defineProperty(proto,key,{get:function(){if(this!==options)throw 'inherited dictionary receiver';log.push(key);if(key==='view')return window;if(key==='relatedTarget')return b;if(Object.prototype.hasOwnProperty.call(numbers,key))return {valueOf:function(){log.push('number:'+key);return numbers[key];}};return {valueOf:function(){throw 'boolean coercion';},toString:function(){throw 'boolean text coercion';}};}});}
for(var key of names)install(key);
var e=new MouseEvent({toString:function(){log.push('type');return 'ordered';}},options);
if(log.join(',')!=='type,bubbles,cancelable,composed,detail,number:detail,view,altKey,ctrlKey,metaKey,modifierAltGraph,modifierCapsLock,modifierFn,modifierFnLock,modifierHyper,modifierNumLock,modifierScrollLock,modifierSuper,modifierSymbol,modifierSymbolLock,shiftKey,button,number:button,buttons,number:buttons,clientX,number:clientX,clientY,number:clientY,relatedTarget,screenX,number:screenX,screenY,number:screenY')throw 'actual inherited dictionary interleaving';
if(e.detail!==1||e.button!==1||e.buttons!==1||e.clientX!==-0.75||e.clientY!==2.5||e.screenX!==9.5||e.screenY!==-3.25||e.relatedTarget!==b||!e.getModifierState('SymbolLock'))throw 'actual converted slots';
for(var init of [undefined,null]){var d=new MouseEvent('defaults',init);if(d.view!==null||d.detail!==0||d.button!==0||d.buttons!==0||d.which!==1||d.clientX!==0||d.clientY!==0||d.screenX!==0||d.screenY!==0||d.relatedTarget!==null||d.getModifierState('AltGraph')||d.composed)throw 'genuine constructor defaults';}
var undefineds=new MouseEvent('undefined',{screenX:undefined,screenY:undefined,clientX:undefined,clientY:undefined,button:undefined,buttons:undefined});if(undefineds.screenX!==0||undefineds.screenY!==0||undefineds.clientX!==0||undefineds.clientY!==0||undefineds.button!==0||undefineds.buttons!==0)throw 'undefined dictionary defaults';
var wide=new MouseEvent('wide',{clientX:-0,clientY:4294967297.5,screenX:-4294967297.5,screenY:1e100});if(1/wide.clientX!==-Infinity||1/wide.x!==-Infinity||wide.clientY!==4294967297.5||wide.y!==4294967297.5||wide.screenX!==-4294967297.5||wide.screenY!==1e100)throw 'finite double range and signed zero are not narrowed to legacy LONG';
for(var pair of [[32768,-32768],[-32769,32767],[65537,1],[NaN,0],[Infinity,0]]){var n=new MouseEvent('short',{button:pair[0]});if(n.button!==pair[1])throw 'IDL short modulo';}
for(var pair of [[65537,1],[-1,65535],[NaN,0],[Infinity,0]]){var n=new MouseEvent('unsigned',{buttons:pair[0]});if(n.buttons!==pair[1])throw 'IDL unsigned short modulo';}
var refused=0,early=0;try{MouseEvent({toString:function(){early++;return 'call';}});}catch(x){if(x.name==='TypeError')refused++;}try{new MouseEvent();}catch(x){if(x.name==='TypeError')refused++;}try{new MouseEvent(Symbol());}catch(x){if(x.name==='TypeError')refused++;}if(refused!==3||early)throw 'constructor-only and required type before effects';
for(var value of [NaN,Infinity,-Infinity]){var reads='',caught=false;try{new MouseEvent('finite',{get clientX(){reads+='X';return value;},get clientY(){reads+='Y';return 1;}});}catch(x){caught=x.name==='TypeError';}if(!caught||reads!=='X')throw 'finite double before later member Get';}
for(var key of ['screenX','screenY','clientX','clientY'])for(var value of [NaN,Infinity,-Infinity]){var invalid={};invalid[key]=value;var caught=false;try{new MouseEvent('finite-coordinate',invalid);}catch(x){caught=x.name==='TypeError';}if(!caught)throw 'every explicit coordinate uses finite double '+key;}
var reads='',symbol=false;try{new MouseEvent('symbol',{get clientX(){reads+='X';return Symbol();},get clientY(){reads+='Y';return 0;}});}catch(x){symbol=x.name==='TypeError';}if(!symbol||reads!=='X')throw 'Symbol numeric conversion precedes later Get';
var marker={},nested,tail=0;
"#);
    let count = page.state.created_events.len();
    evaluate(&mut page, &mut doc, &mut forms, r#"
var caught=false;try{new MouseEvent('unpublished',{clientX:{valueOf:function(){nested=new MouseEvent('nested-real',{clientX:9.5,buttons:3});throw marker;}},get clientY(){tail++;return 1;}});}catch(x){caught=x===marker;}
if(!caught||tail||nested.type!=='nested-real'||nested.clientX!==9.5||nested.buttons!==3)throw 'reentrant constructor effects and thrown identity';
"#);
    assert_eq!(page.state.created_events.len(), count + 1);
    assert!(page.state.created_events.iter().all(|event| event.name != "unpublished"));
}

#[test]
fn mouse_event_real_window_target_brands_and_origin_refusal_precede_later_members() {
    let (mut doc, mut forms, mut page) = fixture(r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
var b=document.getElementById('button'),text=document.createTextNode('original'),attr=document.createAttribute('title'),target=new EventTarget();
for(var related of [window,w,document,w.document,b,text,attr,target,null,undefined]){var e=new MouseEvent('related',{relatedTarget:related});if(e.relatedTarget!==(related===undefined?null:related))throw 'genuine nullable EventTarget';}
for(var view of [window,w,null,undefined]){var e=new MouseEvent('view',{view:view});if(e.view!==(view===undefined?null:view))throw 'genuine nullable Window';}
var duck={valueOf:function(){throw 'duck numeric';},toString:function(){throw 'duck string';}},reads='',caught=false;
try{new MouseEvent('duck',{get relatedTarget(){reads+='R';return duck;},get screenX(){reads+='X';return 1;}});}catch(x){caught=x.name==='TypeError';}if(!caught||reads!=='R')throw 'target brand before screen Get and without duck coercion';
"#);
    let count = page.state.created_events.len();
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, root: None, fragment: String::new(),
    });
    evaluate(&mut page, &mut doc, &mut forms, r#"
var reads='',denied=false;try{new MouseEvent('foreign-target',{get relatedTarget(){reads+='R';return w;},get screenX(){reads+='X';return 1;}});}catch(x){denied=x.name==='SecurityError';}if(!denied||reads!=='R')throw 'foreign target refusal before later screen Get';
reads='';denied=false;try{new MouseEvent('foreign-view',{get detail(){reads+='D';return 3;},get view(){reads+='V';return w;},get altKey(){reads+='A';return true;}});}catch(x){denied=x.name==='SecurityError';}if(!denied||reads!=='DV')throw 'foreign view refusal before modifiers';
"#);
    assert_eq!(page.state.created_events.len(), count);
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/page").unwrap().0), path: None, root: None, fragment: String::new(),
    });
    evaluate(&mut page, &mut doc, &mut forms, "var recovered=new MouseEvent('recovered',{view:w,relatedTarget:w.document,screenX:2.5});if(recovered.view!==w||recovered.relatedTarget!==w.document||recovered.screenX!==2.5)throw 'same-origin target recovery';");
}

#[test]
fn mouse_event_canonical_owner_realms_saved_foreign_operations_and_owner_clock_are_real() {
    let (mut doc, mut forms, mut page) = fixture_with_limits("",crate::javascript::limits::BROWSER);
    page.state.clock_origin = Instant::now().checked_sub(Duration::from_secs(3600)).unwrap();
    evaluate(&mut page, &mut doc, &mut forms, r#"
var C=MouseEvent,P=C.prototype,RootFunction=Function;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,ChildC=w.MouseEvent,ChildP=ChildC.prototype,ChildFunction=w.Function;
var main=new C('main',{clientX:11.5,buttons:3}),child=new ChildC('child',{view:w,relatedTarget:w,clientX:13.5});
if(ChildC===C||ChildP===P||Object.getPrototypeOf(ChildC)!==w.UIEvent||Object.getPrototypeOf(ChildP)!==w.UIEvent.prototype||Object.getPrototypeOf(main)!==P||Object.getPrototypeOf(child)!==ChildP||child instanceof C||main instanceof ChildC)throw 'independent canonical Mouse owner roots';
if(main.timeStamp-child.timeStamp<3000000||child.timeStamp<0||main.timeStamp%1||child.timeStamp%1)throw 'actual owner timestamp origin';
var init=main.initMouseEvent,get=Object.getOwnPropertyDescriptor(P,'clientX').get;
var read=ChildFunction('e','return e.initMouseEvent;'),savedGet=ChildFunction('e','g','return g.call(e);'),savedInit=ChildFunction('e','f','t','return f.call(e,t);');
if(read(main)!==init||savedGet(main,get)!==11.5||Object.getPrototypeOf(init)!==RootFunction.prototype)throw 'first child lookup keeps method owner';
var childOwn=ChildFunction('selfWindow','var e=new MouseEvent("own-child",{view:selfWindow,relatedTarget:selfWindow,clientX:17.5});return e instanceof MouseEvent&&e.view===selfWindow&&e.relatedTarget===selfWindow&&e.clientX===17.5;');
var conversions=0,type={toString:function(){conversions++;return 'changed';}},options={get clientX(){conversions++;return 19.5;}};
"#);
    let count = page.state.created_events.len();
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, root: None, fragment: String::new(),
    });
    for source in ["new w.MouseEvent(type,options);", "new ChildC(type,options);", "read(main);", "savedGet(main,get);", "savedInit(main,init,type);"] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(page.state.created_events.len(), count);
        evaluate(&mut page, &mut doc, &mut forms, "if(conversions||main.type!=='main'||main.clientX!==11.5||main.buttons!==3)throw 'foreign refusal before coercion and mutation';");
    }
    evaluate(&mut page, &mut doc, &mut forms, "if(main.clientX!==11.5||!childOwn(w)||conversions)throw 'captured child owner governs its own Window arguments';");
    assert_eq!(page.state.created_events.len(), count + 1);
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/page").unwrap().0), path: None, root: None, fragment: String::new(),
    });
    evaluate(&mut page, &mut doc, &mut forms, r#"
var recovered=new ChildC(type,options);if(conversions!==2||recovered.clientX!==19.5||Object.getPrototypeOf(recovered)!==ChildP||read(main)!==init||savedGet(main,get)!==11.5)throw 'same-origin saved operation recovery';
ChildFunction('MouseEvent=17;')();MouseEvent=23;
if(Object.getPrototypeOf(new ChildC('saved-child'))!==ChildP||Object.getPrototypeOf(new C('saved-main'))!==P)throw 'global replacement preserves canonical roots';
delete window.MouseEvent;if(typeof MouseEvent!=='undefined'||Object.getPrototypeOf(new C('deleted-global'))!==P)throw 'deleted constructor leaves canonical capability';
"#);
    evaluate(&mut page, &mut doc, &mut forms, "if(typeof MouseEvent!=='undefined'||Object.getPrototypeOf(new C('after-entry'))!==P)throw 'no hidden native restoration after deletion';");
}

#[test]
fn mouse_event_factory_and_legacy_long_initializer_preserve_constructor_only_fields() {
    let (_doc, _forms, page) = fixture(r#"
var P=MouseEvent.prototype,b=document.getElementById('button'),f=document.createEvent('mOuSeEvEnTs');
if(Object.getPrototypeOf(f)!==P||!(f instanceof MouseEvent)||!(f instanceof UIEvent)||!(f instanceof Event)||f.type!==''||f.which!==0||f.button!==0||f.buttons!==0||f.clientX!==0||f.clientY!==0||f.layerX!==0||f.layerY!==0||f.getModifierState('FnLock'))throw 'genuine uninitialized factory defaults';
var initialized=false;try{b.dispatchEvent(f);}catch(x){initialized=x.name==='InvalidStateError';}if(!initialized)throw 'factory initialization state';
var e=new MouseEvent('constructed',{composed:true,view:window,detail:7,screenX:1.5,clientX:2.75,buttons:5,button:1,modifierAltGraph:true,modifierCapsLock:true,modifierFnLock:true,ctrlKey:true,altKey:true,shiftKey:true,metaKey:true,relatedTarget:b}),stamp=e.timeStamp;
var log='';P.initMouseEvent.call(e,{toString:function(){log+='T';return 'legacy';}},false,true,null,{valueOf:function(){log+='D';return 4294967297;}},{valueOf:function(){log+='X';return 4294967297;}},-4294967297,3.9,-4.9,false,false,true,false,2,null);
if(log!=='TDX'||e.type!=='legacy'||e.view!==null||e.detail!==1||e.screenX!==1||e.screenY!==-1||e.clientX!==3||e.clientY!==-4||e.button!==2||e.which!==3||e.relatedTarget!==null||e.ctrlKey||e.altKey||!e.shiftKey||e.metaKey||e.buttons!==5||!e.getModifierState('AltGraph')||!e.getModifierState('CapsLock')||!e.getModifierState('FnLock')||e.getModifierState('Control')||!e.getModifierState('Shift')||e.isTrusted||!e.composed||e.timeStamp!==stamp||e.layerX!==0||e.layerY!==0)throw 'legacy LONG coercion preserves constructor-only state';
UIEvent.prototype.initUIEvent.call(e,'ui-reset',true,true,window,9);if(e.detail!==9||e.view!==window||e.clientX!==3||e.button!==2||e.buttons!==5||e.which!==3||!e.getModifierState('FnLock'))throw 'borrowed UI init preserves Mouse fields';
Event.prototype.initEvent.call(e,'base-reset');if(e.detail!==9||e.clientX!==3||e.buttons!==5||e.which!==3||!e.getModifierState('FnLock'))throw 'base init preserves all Mouse state';
f.initMouseEvent('factory-ready',false,false,null,0,1.75,2.75,3.75,4.75,false,false,false,false,0);if(f.which!==1||f.screenX!==1||f.clientX!==3||f.buttons!==0||f.getModifierState('AltGraph'))throw 'factory legacy defaults and integer coordinates';
"#);
    assert!(page.state.created_events.iter().all(|event| event.initialized));
}

#[test]
fn mouse_event_dispatch_boundary_and_native_keyboard_brand_remain_explicit() {
    let (mut doc, mut forms, mut page) = fixture(r#"
var b=document.getElementById('button'),mouse=new MouseEvent('mouse',{clientX:2.5}),ui=new UIEvent('ui');
var get=Object.getOwnPropertyDescriptor(MouseEvent.prototype,'clientX').get,mod=MouseEvent.prototype.getModifierState;
var denied=false;try{b.dispatchEvent(mouse);}catch(x){denied=x.name==='NotSupportedError';}if(!denied||mouse.target!==null||mouse.currentTarget!==null||mouse.eventPhase!==0||mouse.isTrusted||mouse.composedPath().length)throw 'Mouse dispatch stays explicitly unsupported and clean';
if(typeof KeyboardEvent!=='function'||typeof CustomEvent!=='function'||ui instanceof MouseEvent)throw 'deferred constructor and brand boundaries';
var keys=0;b.addEventListener('keydown',function(event){
 keys++;if(!event.isTrusted||!(event instanceof UIEvent)||!(event instanceof Event)||event instanceof MouseEvent||event.view!==window||event.detail!==0||event.which!==65)throw 'genuine keyboard UI ancestry without Mouse fabrication';
 var refused=0;try{get.call(event);}catch(x){if(x.name==='TypeError')refused++;}try{mod.call(event,'Control');}catch(x){if(x.name==='TypeError')refused++;}if(refused!==2)throw 'Mouse getters and methods reject native keyboard';
 Event.prototype.preventDefault.call(event);
});
"#);
    let node = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("button")).unwrap();
    page.begin_job();
    let outcome = page.keyboard(&mut doc, &mut forms, KeyboardInput {
        down: true, target: Some(node), key: "a".into(), code: "KeyA".into(), key_code: 65,
        ..KeyboardInput::default()
    });
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert!(outcome.prevented);
    evaluate(&mut page, &mut doc, &mut forms, "if(keys!==1||mouse.clientX!==2.5||mouse.isTrusted)throw 'native input and Mouse state remain separate';");
}

#[test]
fn mouse_event_small_profiles_work_and_real_store_failure_preserve_prior_records_and_ids() {
    let (mut doc, mut forms, mut page) = fixture("var C=MouseEvent,kept=new C('kept',{clientX:7.5,buttons:3}),factoryKept=document.createEvent('MouseEvent');factoryKept.initMouseEvent('factory-kept');");
    let count = page.state.created_events.len();
    let retained = page.state.retained;
    let normal_capacity = page.state.limits.host.created_events;
    page.state.limits.host.created_events = count;
    let next = page.state.next_event;
    let outcome = page.console_eval(&mut doc, &mut forms, "new C('capacity-refused');");
    assert_eq!(outcome.errors, 1);
    assert_eq!(page.state.created_events.len(), count);
    assert_eq!(page.state.retained, retained);
    assert_eq!(page.state.next_event, next);
    page.state.limits.host.created_events = normal_capacity;
    let normal_type = page.state.limits.host.event_type_bytes;
    page.state.limits.host.event_type_bytes = 3;
    let outcome = page.console_eval(&mut doc, &mut forms, "new C('type-refused');");
    assert_eq!(outcome.errors, 1);
    assert_eq!(page.state.created_events.len(), count);
    assert_eq!(page.state.retained, retained);
    page.state.limits.host.event_type_bytes = normal_type;
    let normal_retained = page.state.retained_limit;
    page.state.retained_limit = retained;
    let outcome = page.console_eval(&mut doc, &mut forms, "new C('retained-refused');");
    assert_eq!(outcome.errors, 1);
    assert_eq!(page.state.created_events.len(), count);
    assert_eq!(page.state.retained, retained);
    page.state.retained_limit = normal_retained;
    let normal_work = page.state.limits.host_work;
    page.state.limits.host_work = 1;
    let next = page.state.next_event;
    for source in ["new C('work-refused');", "document.createEvent('MouseEvent');"] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1);
        assert_eq!(page.state.created_events.len(), count);
        assert_eq!(page.state.retained, retained);
        assert_eq!(page.state.next_event, next);
    }
    page.state.limits.host_work = normal_work;
    for source in ["new C('store-unpublished');", "document.createEvent('MouseEvent');"] {
        let next = page.state.next_event;
        let normal = page.runtime.event_property_allowance_for_test(0);
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "Real own isTrusted accessor storage must refuse.");
        assert_eq!(page.state.created_events.len(), count);
        assert_eq!(page.state.retained, retained);
        assert_eq!(page.state.next_event, next + 1, "Withdrawn capabilities must not be reused.");
        page.runtime.event_property_allowance_for_test(normal);
    }
    evaluate(&mut page, &mut doc, &mut forms, "var recovered=new C('recovered',{clientX:11.5,buttons:5}),factory=document.createEvent('MouseEvent');factory.initMouseEvent('factory-recovered');if(kept.clientX!==7.5||kept.buttons!==3||factoryKept.type!=='factory-kept'||recovered.clientX!==11.5||recovered.buttons!==5||Object.getPrototypeOf(recovered)!==C.prototype||Object.getPrototypeOf(factory)!==C.prototype||recovered.isTrusted||factory.isTrusted)throw 'actual profile and store recovery';");
    assert_eq!(page.state.created_events.len(), count + 2);
    page.state.next_event = script_class_list::BASE;
    let exhausted = page.state.next_event;
    let outcome = page.console_eval(&mut doc, &mut forms, "new C('identity-refused');");
    assert_eq!(outcome.errors, 1);
    assert_eq!(page.state.created_events.len(), count + 2);
    assert_eq!(page.state.next_event, exhausted);
}
