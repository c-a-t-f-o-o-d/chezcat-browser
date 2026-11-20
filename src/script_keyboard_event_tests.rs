//! Original native/constructed keyboard identity, conversion and dispatch controls.
use super::*;
fn fixture(source:&str)->(Document,Forms,Page){
    let mut doc=crate::html::parse("<html><body><div id=parent><input id=field value=original></div></body></html>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    page.set_location(&crate::navigation::address("https://keyboard-event.test/control",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[Script{node:0,label:"Original genuine KeyboardEvent".into(),source:source.into()}]).errors,0,"{}",page.report());(doc,forms,page)
}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{}",page.report());}
#[test]
fn keyboard_event_real_constructor_descriptors_values_utf16_and_modifier_state(){
    let(_,_,page)=fixture(r#"
var C=KeyboardEvent,P=C.prototype,e=new C('keydown',{key:'A',code:'KeyA',location:2,ctrlKey:true,altKey:true,shiftKey:true,metaKey:true,repeat:true,isComposing:true,keyCode:65,charCode:97,view:window,detail:3,bubbles:true,cancelable:true,composed:true,modifierAltGraph:true,modifierCapsLock:true,modifierFn:true,modifierFnLock:true,modifierHyper:true,modifierNumLock:true,modifierScrollLock:true,modifierSuper:true,modifierSymbol:true,modifierSymbolLock:true});
if(typeof C!=='function'||C.name!=='KeyboardEvent'||C.length!==1||Object.getPrototypeOf(C)!==UIEvent||Object.getPrototypeOf(P)!==UIEvent.prototype||Object.getPrototypeOf(e)!==P||e.constructor!==C||!(e instanceof UIEvent)||!(e instanceof Event)||e instanceof MouseEvent||Object.prototype.toString.call(e)!=='[object KeyboardEvent]')throw 'real keyboard ancestry';
if(e.key!=='A'||e.code!=='KeyA'||e.location!==2||!e.repeat||!e.isComposing||e.keyCode!==65||e.charCode!==97||e.which!==97||e.view!==window||e.detail!==3||e.isTrusted||!e.bubbles||!e.cancelable||!e.composed||e.target!==null)throw 'real keyboard slots';
for(var name of ['key','code','location','ctrlKey','altKey','shiftKey','metaKey','repeat','isComposing','keyCode','charCode']){
 var d=Object.getOwnPropertyDescriptor(P,name);if(typeof d.get!=='function'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||Object.hasOwn(e,name))throw 'keyboard getter descriptor '+name;
}
var trust=Object.getOwnPropertyDescriptor(e,'isTrusted');if(!trust||trust.configurable||trust.set!==undefined||trust.get.call(e))throw 'unforgeable keyboard trust';
for(var name of ['Alt','Control','Shift','Meta','AltGraph','CapsLock','Fn','FnLock','Hyper','NumLock','ScrollLock','Super','Symbol','SymbolLock'])if(!e.getModifierState(name))throw 'keyboard modifier '+name;
if(e.getModifierState('Unknown')||e.getModifierState('alt')||P.getModifierState.length!==1)throw 'exact keyboard modifiers';
for(var pair of [['STANDARD',0],['LEFT',1],['RIGHT',2],['NUMPAD',3]]){var name='DOM_KEY_LOCATION_'+pair[0];if(C[name]!==pair[1]||P[name]!==pair[1]||Object.getOwnPropertyDescriptor(C,name).writable||Object.getOwnPropertyDescriptor(P,name).configurable)throw 'keyboard location constants';}
var zero=new C('zero');if(zero.key!==''||zero.code!==''||zero.location||zero.keyCode||zero.charCode||zero.which||zero.repeat||zero.isComposing||zero.ctrlKey||zero.view!==null||zero.detail||zero.isTrusted)throw 'keyboard defaults';
var units='\uD800',u=new C('units',{key:units,code:'x'+units});if(u.key!==units||u.key.charCodeAt(0)!==55296||u.code!=='x'+units)throw 'exact keyboard UTF16';
e.key='changed';if(e.key!=='A')throw 'keyboard readonly';var strict=false;try{(function(){'use strict';e.location=1;})();}catch(x){strict=x.name==='TypeError';}if(!strict)throw 'keyboard strict readonly';
var get=Object.getOwnPropertyDescriptor(P,'key').get,descriptor=Object.getOwnPropertyDescriptor(P,'key');Object.defineProperty(P,'key',{get:function(){return 'override';},configurable:true});if(e.key!=='override'||get.call(e)!=='A')throw 'ordinary keyboard prototype';delete P.key;if(e.key!==undefined||'key' in e)throw 'keyboard ghost getter';Object.defineProperty(P,'key',descriptor);
"#);assert!(page.state.event.is_none());
}
#[test]
fn keyboard_event_conversion_order_brand_errors_factory_and_untrusted_dispatch(){
    let(_,_,_)=fixture(r#"
var C=KeyboardEvent,P=C.prototype,trace='',arg={},marker={};
for(var name of ['bubbles','cancelable','composed','detail','view','altKey','ctrlKey','metaKey','modifierAltGraph','modifierCapsLock','modifierFn','modifierFnLock','modifierHyper','modifierNumLock','modifierScrollLock','modifierSuper','modifierSymbol','modifierSymbolLock','shiftKey','charCode','code','isComposing','key','keyCode','location','repeat']){
 (function(key){Object.defineProperty(arg,key,{get:function(){trace+=key+'|';if(key==='view')return null;if(key==='key'||key==='code')return {toString:function(){trace+='string|';return key;}};if(key==='detail'||key==='charCode'||key==='keyCode'||key==='location')return {valueOf:function(){trace+='number|';return 4294967297;}};return false;}});})(name);
}
var e=new C({toString:function(){trace+='type|';return 'keydown';}},arg);
if(trace!=='type|bubbles|cancelable|composed|detail|number|view|altKey|ctrlKey|metaKey|modifierAltGraph|modifierCapsLock|modifierFn|modifierFnLock|modifierHyper|modifierNumLock|modifierScrollLock|modifierSuper|modifierSymbol|modifierSymbolLock|shiftKey|charCode|number|code|string|isComposing|key|string|keyCode|number|location|number|repeat|'||e.location!==1||e.keyCode!==1||e.charCode!==1||e.detail!==1)throw 'keyboard ordered conversion';
var touched=0,request={toString:function(){touched++;return 'Control';}},get=Object.getOwnPropertyDescriptor(P,'key').get,mod=P.getModifierState;
for(var value of [null,undefined,{},Object.create(P),new Event('base'),new UIEvent('ui'),new MouseEvent('mouse')]){try{get.call(value);throw 'bad getter brand';}catch(x){if(x.name!=='TypeError')throw x;}try{mod.call(value,request);throw 'bad modifier brand';}catch(x){if(x.name!=='TypeError')throw x;}}
if(touched)throw 'brand converted modifier';
try{C(request);throw 'keyboard called without new';}catch(x){if(x.name!=='TypeError')throw x;}
try{new C();throw 'missing keyboard type';}catch(x){if(x.name!=='TypeError')throw x;}
try{new C('bad',{key:Symbol('key')});throw 'symbol keyboard key';}catch(x){if(x.name!=='TypeError')throw x;}
try{new C('bad',{get code(){throw marker;}});throw 'dictionary getter swallowed';}catch(x){if(x!==marker)throw x;}
var f=document.createEvent('kEyBoArDeVeNt');if(!(f instanceof C)||f.type!==''||f.key!==''||f.code!==''||f.location||f.which||f.isTrusted)throw 'real keyboard factory';f.initEvent('keydown',true,true);
var field=document.getElementById('field'),parent=document.getElementById('parent'),calls=0;
parent.addEventListener('keydown',function(event){calls++;if(event!==f||event.isTrusted||event.target!==field||event.currentTarget!==parent||event.key!==''||event.eventPhase!==3)throw 'actual synthetic keyboard dispatch';event.preventDefault();});
if(field.dispatchEvent(f)!==false||calls!==1||!f.defaultPrevented||f.currentTarget!==null||f.eventPhase||field.value!=='original')throw 'untrusted keyboard cancellation/native edit separation';
"#);
}
#[test]
fn keyboard_event_native_ancestry_modifiers_cancellation_and_saved_receiver_expiry(){
    let(mut doc,mut forms,mut page)=fixture(r#"
var field=document.getElementById('field'),saved,get=Object.getOwnPropertyDescriptor(KeyboardEvent.prototype,'key').get,mod=KeyboardEvent.prototype.getModifierState,calls=0;
field.addEventListener('keydown',function(e){calls++;saved=e;
 if(Object.getPrototypeOf(e)!==KeyboardEvent.prototype||!(e instanceof KeyboardEvent)||!(e instanceof UIEvent)||!(e instanceof Event)||e instanceof MouseEvent||!e.isTrusted||e.key!=='A'||e.code!=='KeyA'||e.location!==2||e.which!==65||e.charCode||!e.repeat||e.isComposing||e.view!==window||e.detail)throw 'real native keyboard';
 if(!mod.call(e,'Control')||!mod.call(e,'Shift')||!mod.call(e,'AltGraph')||!mod.call(e,'CapsLock')||!mod.call(e,'NumLock')||!mod.call(e,'ScrollLock')||mod.call(e,'Meta')||mod.call(e,'Fn'))throw 'native modifier snapshot';
 e.note='ordinary';if(Object.getOwnPropertyDescriptor(e,'note').value!=='ordinary'||Object.getOwnPropertyDescriptor(e,'isTrusted').configurable)throw 'real native wrapper';
 e.preventDefault();if(!e.defaultPrevented||!e.isTrusted)throw 'native cancellation';
});
"#);
    let node=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("field")).unwrap();page.begin_job();
    let outcome=page.keyboard(&mut doc,&mut forms,KeyboardInput{down:true,target:Some(node),key:"A".into(),code:"KeyA".into(),location:2,key_code:65,ctrl:true,shift:true,alt:true,repeat:true,caps_lock:true,num_lock:true,scroll_lock:true,alt_graph:true,..KeyboardInput::default()});
    assert_eq!(outcome.errors,0,"{}",page.report());assert!(outcome.prevented);assert!(page.state.event.is_none());
    run(&mut doc,&mut forms,&mut page,r#"
var request={toString:function(){throw 'expired receiver converted';}},rejected=0;
try{get.call(saved);}catch(x){if(x.name==='TypeError')rejected++;}try{mod.call(saved,request);}catch(x){if(x.name==='TypeError')rejected++;}
var e=new KeyboardEvent('separate',{key:'real',ctrlKey:true});if(rejected!==2||calls!==1||get.call(e)!=='real'||!mod.call(e,'Control')||e.isTrusted||field.value!=='original')throw 'saved keyboard receiver isolation';
"#);
}

#[test]
fn keyboard_event_owner_realms_saved_constructor_and_origin_guards(){
    let(mut doc,mut forms,mut page)=fixture(r#"
var C=KeyboardEvent,P=C.prototype,rootGet=Object.getOwnPropertyDescriptor(P,'key').get;
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,Child=w.KeyboardEvent,ChildP=Child.prototype,ChildFunction=w.Function;
var make=ChildFunction('return new KeyboardEvent("child",{key:"child",view:window});'),read=ChildFunction('e','return e.key;');
var child=make(),root=new C('root',{key:'root'}),get=Object.getOwnPropertyDescriptor(ChildP,'key').get;
if(Child===C||ChildP===P||Object.getPrototypeOf(Child)!==w.UIEvent||Object.getPrototypeOf(ChildP)!==w.UIEvent.prototype||Object.getPrototypeOf(child)!==ChildP||Object.getPrototypeOf(get)!==ChildFunction.prototype||child.view!==w||child.isTrusted||read(root)!=='root'||get.call(root)!=='root'||rootGet.call(child)!=='child')throw 'keyboard constructor/getter realm ownership';
KeyboardEvent=function(){throw 'replacement used';};var saved=new C('saved',{key:'saved'});delete window.KeyboardEvent;
if(rootGet.call(saved)!=='saved'||Object.getPrototypeOf(saved)!==P||saved.constructor!==C||!(saved instanceof C))throw 'canonical keyboard roots followed mutable binding';
window.KeyboardEvent=C;
var touched=0,foreignType={toString:function(){touched++;return 'foreign';}},options={get key(){touched++;return 'foreign';}};
var own=ChildFunction('var e=new KeyboardEvent("own",{key:"child",view:window});return e.key==="child"&&e.view===window;');
"#);
    page.set_location(&crate::navigation::address("https://different-keyboard.test/changed",&crate::navigation::Location::home()).unwrap());
    for source in ["new w.KeyboardEvent(foreignType,options);","new Child(foreignType,options);","read(root);","get.call(root);","rootGet.call(child);"]{
        page.clear_console();let out=page.console_eval(&mut doc,&mut forms,source);assert_eq!(out.errors,1,"{source}\n{}",page.report());assert!(page.report().contains("same-origin"),"{source}\n{}",page.report());
        run(&mut doc,&mut forms,&mut page,"if(rootGet.call(root)!=='root'||root.isTrusted||touched)throw 'origin failure converted or damaged root keyboard';");
    }
    run(&mut doc,&mut forms,&mut page,"if(!own()||touched)throw 'captured child realm cannot use its own keyboard interface';");
}
