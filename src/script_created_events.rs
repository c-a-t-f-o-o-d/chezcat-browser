//! Persistent untrusted Event/CustomEvent/UIEvent/MouseEvent/KeyboardEvent state. These records never enter the
//! trusted dispatch slot. Synchronous base/UI dispatch is separate and untrusted.
use super::*;

#[cfg(test)]
const MAX_CREATED_EVENTS: usize = 256;

#[derive(Default)]
struct UiData {
    view: Option<u32>,
    detail: i32,
}

#[derive(Default)]
struct MouseData {
    screen_x: f64,
    screen_y: f64,
    client_x: f64,
    client_y: f64,
    layer_x: i32,
    layer_y: i32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    button: i16,
    buttons: u16,
    modifiers: u16,
    which: u32,
    related: Option<u32>,
}

struct KeyboardData {
    key: Value,
    code: Value,
    location: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    repeat: bool,
    composing: bool,
    key_code: u32,
    char_code: u32,
    modifiers: u16,
}
impl Default for KeyboardData {
    fn default()->Self {Self {key:Value::String(String::new()),code:Value::String(String::new()),location:0,
        ctrl:false,alt:false,shift:false,meta:false,repeat:false,composing:false,key_code:0,char_code:0,modifiers:0}}
}
impl KeyboardData {
    fn payload_bytes(&self)->usize {retained_storage::value_bytes(&self.key)+retained_storage::value_bytes(&self.code)}
    fn which(&self)->u32 {if self.char_code!=0 {self.char_code}else{self.key_code}}
}

const EXTRA_MODIFIERS: &[(&str,&str,u16)] = &[
    ("modifierAltGraph","AltGraph",1),("modifierCapsLock","CapsLock",2),
    ("modifierFn","Fn",4),("modifierFnLock","FnLock",8),
    ("modifierHyper","Hyper",16),("modifierNumLock","NumLock",32),
    ("modifierScrollLock","ScrollLock",64),("modifierSuper","Super",128),
    ("modifierSymbol","Symbol",256),("modifierSymbolLock","SymbolLock",512),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn start(source: &str) -> (Document, Forms, Page, Outcome) {
        let mut doc = crate::html::parse("<html><body><button id=b type=button>Click</button><p id=p title=old>Text</p></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let location = crate::navigation::Location { url: Some(crate::url::parse("https://example.test/page").unwrap().0), path: None, fragment: String::new(), root: None };
        page.set_location(&location);
        let outcome = page.start(&mut doc, &mut forms, &[Script { node: 0, label: "Original constructed-event fixture".into(), source: source.into() }]);
        (doc, forms, page, outcome)
    }

    #[test]
    fn constructed_mouse_coordinates_alias_clients_and_track_css_viewport_scroll() {
        let (mut doc,mut forms,mut page,outcome)=start(r#"
var e=document.createEvent('MouseEvents');
if(e.pageX!==0||e.pageY!==0||e.x!==0||e.y!==0||e.offsetX!==0||e.offsetY!==0||!('pageX' in e))throw 'zero';
e.initMouseEvent('probe',false,false,null,0,900,800,-7,11);
if(e.x!==-7||e.y!==11||e.pageX!==-7||e.pageY!==11||e.offsetX!==-7||e.offsetY!==11)throw 'client aliases';
e.pageX=99;if(e.pageX!==-7)throw 'readonly';
var strict=false;try{(function(){'use strict';e.pageY=99;})();}catch(x){strict=x.name==='TypeError';}if(!strict)throw 'strict readonly';
var base=document.createEvent('Event');if('pageX' in base||base.pageX!==undefined)throw 'brand';
"#);
        assert_eq!(outcome.errors,0,"{}",page.report());
        page.set_geometry(crate::script_geometry::Geometry::new(800,600,2.0,&HashMap::new(),HashMap::new(),&crate::fonts::Book::default()));
        page.set_geometry_scroll(246);
        let outcome=page.console_eval(&mut doc,&mut forms,"if(e.pageY!==134||e.offsetY!==134||e.y!==11||e.pageX!==-7)throw 'current CSS scroll';");
        assert_eq!(outcome.errors,0,"{}",page.report());
        page.set_geometry_scroll(40);
        let outcome=page.console_eval(&mut doc,&mut forms,"if(e.pageY!==31||e.offsetY!==31)throw 'live scroll';e.initMouseEvent('reset');if(e.clientY!==0||e.pageY!==20||e.offsetY!==20)throw 'init reset';");
        assert_eq!(outcome.errors,0,"{}",page.report());
    }

    #[test]
    fn constructed_mouse_and_ui_which_preserve_initialization_and_readonly_brands() {
        let (_,_,page,outcome)=start(r#"
var e=document.createEvent('MouseEvents'),ui=document.createEvent('UIEvent'),base=document.createEvent('Event');
if(e.which!==0||ui.which!==0||!('which' in e)||!('which' in ui)||'which' in base||base.which!==undefined)throw 'factory defaults/brands';
for(var b of [0,1,2,-1]){e.initMouseEvent('x',false,false,null,0,0,0,0,0,false,false,false,false,b);if(e.which!==b+1||e.button!==b)throw 'initialized button';}
e.which=17;if(e.which!==0)throw 'readonly';var caught=false;try{(function(){'use strict';ui.which=4;})();}catch(x){caught=x.name==='TypeError';}if(!caught)throw 'strict readonly';
e.initMouseEvent('middle',false,false,null,0,0,0,0,0,false,false,false,false,1);e.initEvent('renamed');if(e.which!==2)throw 'base init preserves subclass';
ui.initUIEvent('u',true,true,window,7);if(ui.which!==0)throw 'UI has no keyboard or button data';
"#);
        assert_eq!(outcome.errors,0,"{}",page.report());
    }

    #[test]
    fn ui_event_factory_initializers_and_mouse_inheritance_are_real() {
        let (doc,_,page,outcome)=start(r#"
var first=document.createEvent('UIEvent'),second=document.createEvent('uIeVeNtS'),base=document.createEvent('Event');
for(var e of [first,second]){
 if(e.type!==''||e.view!==null||e.detail!==0||e.bubbles||e.cancelable||e.isTrusted||e.target!==null||e.currentTarget!==null)throw 'defaults';
 if(typeof e.initUIEvent!=='function'||e.initUIEvent.length!==1||e.initUIEvent!==e.initUIEvent||'screenX' in e||'initMouseEvent' in e||e.screenX!==undefined)throw 'UI interface';
}
if(first===second||'view' in base||'initUIEvent' in base)throw 'brand/identity';
var stamp=first.timeStamp,truth={valueOf:function(){throw 'boolean conversion';},toString:function(){throw 'boolean conversion';}};
first.initUIEvent('ui',truth,true,window,-4294967297);first.preventDefault();first.cancelBubble=true;
if(first.view!==window||first.detail!==-1||!first.bubbles||!first.cancelable||!first.defaultPrevented||!first.cancelBubble)throw 'initialization';
first.initEvent('base');if(first.view!==window||first.detail!==-1||first.bubbles||first.cancelable||first.defaultPrevented||first.cancelBubble||first.timeStamp!==stamp)throw 'base preserves UI';
first.initUIEvent('reset');if(first.view!==null||first.detail!==0||first.isTrusted||first.timeStamp!==stamp)throw 'UI defaults';
first.view=truth;first.detail=truth;if(first.view!==null||first.detail!==0)throw 'readonly';
var strict=false;try{(function(){'use strict';first.detail=7;})();}catch(e){strict=e.name==='TypeError';}if(!strict)throw 'strict readonly';
var mouse=document.createEvent('MouseEvent');mouse.initMouseEvent('m',false,true,window,2,3,4,5,6,true,false,false,false,1,document.getElementById('p'));
mouse.initUIEvent('ui-mouse',true,true,null,9);
if(mouse.type!=='ui-mouse'||mouse.view!==null||mouse.detail!==9||mouse.screenX!==3||mouse.clientY!==6||!mouse.ctrlKey||mouse.button!==1||mouse.relatedTarget!==document.getElementById('p'))throw 'mouse preservation';
document.title='ui-factory-ok';
"#);
        assert_eq!(outcome.errors,0,"{}",page.report());
        assert_eq!(doc.title,"ui-factory-ok");
    }

    #[test]
    fn ui_event_conversion_order_brands_and_abrupt_completions_are_atomic() {
        let (doc,_,page,outcome)=start(r#"
var e=document.createEvent('UIEvents'),base=document.createEvent('Event'),log='',marker={};e.initUIEvent('old',true,true,window,3);e.preventDefault();
var type={toString:function(){log+='T';return 'outer';}},number={valueOf:function(){log+='D';return 4294967297;}},bad={toString:function(){throw 'view conversion';}};
var caught=false;try{e.initUIEvent(type,false,false,bad,number);}catch(x){caught=x.name==='TypeError';}
if(!caught||log!=='T'||e.type!=='old'||e.view!==window||e.detail!==3||!e.defaultPrevented)throw 'view before detail';
log='';caught=false;try{e.initUIEvent(type,false,false,bad);}catch(x){caught=x.name==='TypeError';}if(!caught||log!=='T')throw 'omitted detail validates view';
log='';e.initUIEvent(type,false,true,undefined,number);if(log!=='TD'||e.view!==null||e.detail!==1)throw 'ordered conversion';
e.initUIEvent('nan',false,false,null,NaN);if(e.detail!==0)throw 'NaN';e.initUIEvent('infinite',false,false,null,Infinity);if(e.detail!==0)throw 'infinity';
e.initUIEvent('fraction',false,false,null,-1.9);if(e.detail!==-1)throw 'fraction';
log='';caught=false;try{e.initUIEvent.call(base,type,false,false,null,number);}catch(x){caught=x.name==='TypeError';}if(!caught||log!=='')throw 'receiver before conversion';
caught=false;try{e.initUIEvent(type,false,false,null,{valueOf:function(){e.initUIEvent('nested',true,true,window,8);throw marker;}});}catch(x){caught=x===marker;}
if(!caught||e.type!=='nested'||e.detail!==8||e.view!==window||!e.bubbles)throw 'nested effects survive outer failure';
var count=0;try{e.initUIEvent();}catch(x){if(x.name==='TypeError')count++;}try{e.initUIEvent(Symbol());}catch(x){if(x.name==='TypeError')count++;}try{e.initUIEvent('x',false,false,null,Symbol());}catch(x){if(x.name==='TypeError')count++;}
if(count!==3||e.type!=='nested')throw 'type/numeric errors';
var long='';for(var i=0;i<257;i++)long+='x';count=0;
try{e.initUIEvent(long,false,false,bad);}catch(x){if(x.name==='TypeError')count++;}
try{e.initUIEvent(long,false,false,null,Symbol());}catch(x){if(x.name==='TypeError')count++;}
try{e.initUIEvent(long);}catch(x){if(x.name==='QuotaExceededError')count++;}
if(count!==3||e.type!=='nested'||e.detail!==8)throw 'conversion before initializer quota';
document.title='ui-conversion-ok';
"#);
        assert_eq!(outcome.errors,0,"{}",page.report());
        assert_eq!(doc.title,"ui-conversion-ok");
    }

    #[test]
    fn ui_event_native_keyboard_dispatch_preserves_trust_and_saved_initializer() {
        let (mut doc,mut forms,mut page,outcome)=start(r#"
var b=document.getElementById('b'),saved,savedInit,log='',ui=document.createEvent('UIEvent');
b.addEventListener('keydown',function(e){saved=e;savedInit=e.initUIEvent;var type=e.type,target=e.target,stamp=e.timeStamp,phase=e.eventPhase;
 if(e.view!==window||e.detail!==0||!e.isTrusted||!('view' in e)||!('detail' in e))throw 'native UI values';
 e.initUIEvent({toString:function(){log+='T';e.preventDefault();return 'changed';}},false,false,window,{valueOf:function(){log+='D';return 99;}});
 if(log!=='TD'||e.type!==type||e.target!==target||e.timeStamp!==stamp||e.eventPhase!==phase||!e.defaultPrevented||!e.isTrusted||e.detail!==0||e.view!==window)throw 'dispatch initializer no-op';
 var rejected=false;try{e.initUIEvent('x',false,false,{}, {valueOf:function(){log+='BAD';return 1;}});}catch(x){rejected=x.name==='TypeError';}if(!rejected||log!=='TD')throw 'dispatch conversion';
 ui.initUIEvent('separate',true,true,window,7);if(ui.isTrusted||ui.detail!==7||e.detail!==0)throw 'trusted isolation';
});
"#);
        assert_eq!(outcome.errors,0,"{}",page.report());
        let button=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("b")).unwrap();
        page.begin_job();
        let outcome=page.keyboard(&mut doc,&mut forms,KeyboardInput{down:true,target:Some(button),key:"a".into(),code:"KeyA".into(),ctrl:false,shift:false,alt:false,meta:false,repeat:false,location:0,key_code:65,..KeyboardInput::default()});
        assert_eq!(outcome.errors,0,"{}",page.report());
        assert!(outcome.prevented);
        let outcome=page.console_eval(&mut doc,&mut forms,"savedInit.call(ui,'borrowed',false,false,null,11);if(ui.type!=='borrowed'||ui.detail!==11||ui.isTrusted)throw 'borrow after source expiry';document.title='ui-dispatch-ok';");
        assert_eq!(outcome.errors,0,"{}",page.report());
        assert_eq!(doc.title,"ui-dispatch-ok");
    }

    #[test]
    fn ui_event_nullable_window_and_origin_denial_precede_numeric_conversion() {
        let (mut doc,mut forms,mut page,outcome)=start(r#"
var e=document.createEvent('UIEvent'),f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow;
e.initUIEvent('child',false,false,w,5);if(e.view!==w||e.detail!==5)throw 'real child Window';
var conversions=0;var detail={valueOf:function(){conversions++;return 6;}};
"#);
        assert_eq!(outcome.errors,0,"{}",page.report());
        page.set_location(&crate::navigation::Location{url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,fragment:String::new(),root:None});
        let outcome=page.console_eval(&mut doc,&mut forms,"var denied=false;try{e.initUIEvent('forbidden',true,true,w,detail);}catch(x){denied=x.name==='SecurityError';}if(!denied||conversions!==0||e.type!=='child'||e.detail!==5)throw 'origin denial';");
        assert_eq!(outcome.errors,0,"{}",page.report());
    }

    #[test]
    fn ui_event_copy_retention_and_creation_failures_publish_no_partial_state() {
        let (mut doc,mut forms,mut page,outcome)=start("var e=document.createEvent('UIEvent');e.initUIEvent('original',true,true,window,8);e.preventDefault();");
        assert_eq!(outcome.errors,0,"{}",page.report());
        let stamp=page.state.created_events[0].timestamp;
        let handle=page.state.created_events[0].handle;
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        let args=[Value::String("longer-original".into()),Value::Bool(false),Value::Bool(false),Value::Null,Value::Number(99.0)];
        bridge.state.copy_work=0;
        assert!(bridge.call(handle,"initUIEvent",&args).is_err());
        bridge.state.copy_work=bridge.state.limits.copy_bytes;
        bridge.state.retained_limit=bridge.state.retained;
        assert!(bridge.call(handle,"initUIEvent",&args).is_err());
        let event=&bridge.state.created_events[0];
        assert_eq!(event.name,"original");assert!(event.prevented&&event.bubbles&&event.cancelable);
        assert_eq!(event.data.ui().unwrap().detail,8);assert_eq!(event.data.ui().unwrap().view,Some(WINDOW));assert_eq!(event.timestamp,stamp);
        let next=bridge.state.next_event;
        bridge.state.limits.host.created_events=bridge.state.created_events.len();
        assert!(bridge.call(DOCUMENT,"createEvent",&[Value::String("UIEvents".into())]).is_err());
        assert_eq!(bridge.state.next_event,next);assert_eq!(bridge.state.created_events.len(),1);
    }

    #[test]
    fn cancel_bubble_uses_persistent_stop_state_without_conversion_or_default_cancellation() {
        let (doc, _, page, outcome) = start(r#"
var e=document.createEvent('Event');e.initEvent('test',true,true);
if(e.cancelBubble||!('cancelBubble' in e))throw 'default';
var calls=0,obj={valueOf:function(){calls++;throw 'conversion';},toString:function(){calls++;throw 'conversion';}};
if((e.cancelBubble=obj)!==obj||!e.cancelBubble||calls||e.defaultPrevented)throw 'truthy state';
e.cancelBubble=false;if(!e.cancelBubble)throw 'false must not clear';
e.initEvent('reset',true,true);if(e.cancelBubble)throw 'reinit';
e.cancelBubble=Symbol();if(!e.cancelBubble)throw 'symbol truthy';
var caught=0;try{e.initEvent(Symbol());}catch(x){if(x.name==='TypeError')caught++;}
if(caught!==1||!e.cancelBubble||e.type!=='reset')throw 'failed init atomic';
e.initEvent('reset');e.stopPropagation();if(!e.cancelBubble)throw 'method alias';
e.initEvent('reset');e.stopImmediatePropagation();e.cancelBubble=0;if(!e.cancelBubble)throw 'immediate alias';
e.initEvent('reset');if(e.cancelBubble||e.defaultPrevented)throw 'final reset';
document.title='cancel-bubble-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "cancel-bubble-ok");
    }

    #[test]
    fn cancel_bubble_on_real_dispatch_keeps_same_phase_and_expires_without_reviving() {
        let (mut doc, mut forms, mut page, outcome) = start(r#"
var b=document.getElementById('b'),saved,order='';
b.addEventListener('click',function(e){saved=e;order+='a';if(e.cancelBubble)throw 'fresh native';e.cancelBubble={valueOf:function(){throw 'conversion';}};e.cancelBubble=false;if(!e.cancelBubble||e.defaultPrevented)throw 'native stop';});
b.addEventListener('click',function(e){order+='b';if(!e.cancelBubble)throw 'same phase';});
document.addEventListener('click',function(){order+='bad';});
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let button = doc.nodes.iter().enumerate().find(|(id, _)| doc.attr(*id, "id") == Some("b")).unwrap().0;
        let outcome = page.dispatch(&mut doc, &mut forms, button, "click", true);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(!outcome.prevented);
        let outcome = page.console_eval(&mut doc, &mut forms, "if(order!=='ab')throw 'order';var e=document.createEvent('Event');e.cancelBubble=true;if(!e.cancelBubble)throw 'created isolation';document.title='native-stop-ok';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "native-stop-ok");
        let outcome = page.console_eval(&mut doc, &mut forms, "saved.cancelBubble=true;");
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert!(page.report().contains("expired"), "{}", page.report());
    }

    #[test]
    fn constructed_event_factory_has_real_defaults_aliases_brands_and_no_dispatch() {
        let (doc, _, page, outcome) = start(r#"
var aliases=['Event','eVeNtS','HTMLEvents','SVGEvents','MouseEvent','mOuSeEvEnTs'],events=[];
for(var name of aliases){var e=document.createEvent(name);events.push(e);
if(e.type!==''||e.target!==null||e.currentTarget!==null||e.bubbles||e.cancelable||e.defaultPrevented||e.composed||e.isTrusted||e.eventPhase!==0||e.timeStamp<0||!Number.isInteger(e.timeStamp))throw 'base defaults';
if(e.NONE!==0||e.CAPTURING_PHASE!==1||e.AT_TARGET!==2||e.BUBBLING_PHASE!==3)throw 'phase constants';}
for(var e of [events[4],events[5]]){if(e.view!==null||e.relatedTarget!==null||e.detail!==0||e.screenX!==0||e.screenY!==0||e.clientX!==0||e.clientY!==0||e.ctrlKey||e.altKey||e.shiftKey||e.metaKey||e.button!==0||e.buttons!==0)throw 'mouse defaults';}
if('view' in events[0]||'initMouseEvent' in events[0]||!('initMouseEvent' in events[4])||events[0]===events[1])throw 'class/identity';
var calls=0,converted=document.createEvent({toString:function(){calls++;return 'MouseEvents';}});if(calls!==1||converted.isTrusted)throw 'factory conversion';
var errors=0;for(var name of ['CompositionEvent','KeyboardEvents','CustomEvents','Event ','unknown']){try{document.createEvent(name);}catch(e){if(e.name==='NotSupportedError')errors++;}}
try{document.createEvent();}catch(e){if(e.name==='TypeError')errors++;}try{document.createEvent(Symbol());}catch(e){if(e.name==='TypeError')errors++;}
if(errors!==7||typeof document.dispatchEvent!=='function'||typeof window.dispatchEvent!=='function'||'dispatchEvent' in events[0])throw 'factory/target capability';
if(document.createEvent!==document.createEvent||converted.initEvent!==converted.initEvent)throw 'method identity';
document.title='created-defaults-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "created-defaults-ok");
        assert_eq!(page.state.created_events.len(), 7);
        assert!(page.state.event.is_none());
    }

    #[test]
    fn constructed_mouse_initializers_wrap_numbers_preserve_base_fields_and_reset_defaults() {
        let (doc, _, page, outcome) = start(r#"
var e=document.createEvent('MouseEvents'),base=document.createEvent('Event'),stamp=e.timeStamp,reads=0;
var truth={valueOf:function(){reads++;throw 'boolean coercion';},toString:function(){reads++;throw 'boolean coercion';}};
e.initMouseEvent('click',truth,true,window,4294967297,-4294967297,NaN,2147483648,1.9,truth,0,truth,'',65535,document.getElementById('p'));
if(e.type!=='click'||!e.bubbles||!e.cancelable||e.isTrusted||e.view!==window||e.detail!==1||e.screenX!==-1||e.screenY!==0||e.clientX!==-2147483648||e.clientY!==1||!e.ctrlKey||e.altKey||!e.shiftKey||e.metaKey||e.button!==-1||e.buttons!==0||e.relatedTarget!==document.getElementById('p')||reads!==0)throw 'mouse values';
e.preventDefault();if(!e.defaultPrevented)throw 'cancel';e.initEvent('base',false,true);
if(e.type!=='base'||e.bubbles||!e.cancelable||e.defaultPrevented||e.view!==window||e.clientY!==1||e.button!==-1||e.timeStamp!==stamp)throw 'base reset/mouse preservation';
e.initMouseEvent('reset');if(e.view!==null||e.relatedTarget!==null||e.detail!==0||e.screenX!==0||e.screenY!==0||e.clientX!==0||e.clientY!==0||e.ctrlKey||e.altKey||e.shiftKey||e.metaKey||e.button!==0||e.buttons!==0||e.bubbles||e.cancelable||e.timeStamp!==stamp)throw 'default reset';
e.preventDefault();if(e.defaultPrevented)throw 'noncancelable';
var effects=0,rejected=false;try{e.initMouseEvent.call(base,{toString:function(){effects++;return 'bad';}});}catch(x){rejected=x.name==='TypeError';}if(!rejected||effects!==0)throw 'Mouse brand';
rejected=false;try{base.initMouseEvent({toString:function(){effects++;return 'bad';}});}catch(x){rejected=x.name==='TypeError';}if(!rejected||effects!==0)throw 'direct Mouse brand';
base.initEvent(null);if(base.type!=='null')throw 'nonnullable null';base.initEvent(undefined);if(base.type!=='undefined')throw 'explicit undefined';
var rhs={valueOf:function(){throw 'readonly coercion';}};if((e.isTrusted=rhs)!==rhs||e.isTrusted)throw 'readonly';
rejected=false;try{(function(){'use strict';e.clientX=99;})();}catch(x){rejected=x.name==='TypeError';}if(!rejected||e.clientX!==0)throw 'strict readonly';
document.title='created-initializers-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "created-initializers-ok");
    }

    #[test]
    fn constructed_mouse_ordered_brands_and_numeric_failures_preserve_callback_effects() {
        let (doc, _, page, outcome) = start(r#"
var e=document.createEvent('MouseEvents'),log='',marker={},type={toString:function(){log+='T';return 'outer';}},bad={};
e.initMouseEvent('old',false,true,null,3,4,5,6,7,false,false,false,false,2,null);e.preventDefault();
function number(i){return {valueOf:function(){log+=i;return i;},toString:function(){throw 'number hint';}};}
var caught=false;try{e.initMouseEvent(type,false,false,bad,number(4),number(5),number(6),number(7),number(8),false,false,false,false,number(13),null);}catch(x){caught=x.name==='TypeError';}
if(!caught||log!=='T'||e.type!=='old'||!e.defaultPrevented||e.clientX!==6)throw 'view before numeric';
log='';caught=false;try{e.initMouseEvent(type,false,false,bad);}catch(x){caught=x.name==='TypeError';}if(!caught||log!=='T')throw 'truncated view';
log='';caught=false;try{e.initMouseEvent(type,false,false,null,number(4),number(5),number(6),number(7),number(8),false,false,false,false,number(13),bad);}catch(x){caught=x.name==='TypeError';}
if(!caught||log!=='T4567813'||e.type!=='old'||!e.defaultPrevented)throw 'related last';
log='';try{e.initMouseEvent(type,false,false,null,number(4),number(5),{valueOf:function(){log+='X';throw marker;}});}catch(x){if(x!==marker)throw x;}
if(log!=='T45X'||e.type!=='old'||!e.defaultPrevented)throw 'numeric throw';
log='';caught=false;try{e.initMouseEvent(type,false,false,null,{valueOf:function(){e.initEvent('nested',true,true);return 4;}},0,{valueOf:function(){throw marker;}});}catch(x){caught=x===marker;}
if(!caught||e.type!=='nested'||!e.bubbles||e.defaultPrevented||e.clientX!==6)throw 'callback side effect rollback';
caught=false;try{e.initMouseEvent({toString:function(){throw marker;}},false,false,bad,number(4));}catch(x){caught=x===marker;}if(!caught)throw 'type throw wins';
caught=false;try{e.initMouseEvent('s',false,false,null,Symbol());}catch(x){caught=x.name==='TypeError';}if(!caught||e.type!=='nested')throw 'numeric Symbol';
var long='',precedence=0;for(var j=0;j<257;j++)long+='x';
try{e.initMouseEvent(long,false,false,bad);}catch(x){if(x.name==='TypeError')precedence++;}
try{e.initMouseEvent(long,false,false,null,0,0,0,0,0,false,false,false,false,0,bad);}catch(x){if(x.name==='TypeError')precedence++;}
try{e.initMouseEvent(long);}catch(x){if(x.name==='QuotaExceededError')precedence++;}
if(precedence!==3||e.type!=='nested')throw 'interface conversion before initializer quota';
document.title='created-order-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "created-order-ok");
    }

    #[test]
    fn constructed_events_do_not_change_trusted_dispatch_and_outlive_expired_native_ids() {
        let (mut doc, mut forms, mut page, outcome) = start(r#"
var first=document.createEvent('MouseEvents'),second=document.createEvent('Event'),saved,inside,savedInit,savedPrevent;
first.initMouseEvent('first',false,true);second.initEvent('second');
var b=document.getElementById('b');b.addEventListener('click',function(native){
saved=native;savedInit=native.initEvent;savedPrevent=native.preventDefault;var type=native.type,target=native.target,phase=native.eventPhase,stamp=native.timeStamp;
inside=document.createEvent('MouseEvents');inside.initMouseEvent('inside',true,true,window,9);inside.preventDefault();inside.stopImmediatePropagation();
if(!native.isTrusted||native.defaultPrevented||native.type!==type||native.target!==target||native.currentTarget!==b||native.eventPhase!==phase||!inside.defaultPrevented||inside.isTrusted)throw 'trusted isolation';
native.initEvent({toString:function(){native.preventDefault();return 'wrong';}},false,false);
if(!native.isTrusted||!native.defaultPrevented||native.type!==type||native.target!==target||native.eventPhase!==phase||native.timeStamp!==stamp)throw 'dispatch-time no-op';
var bad=0;try{native.initEvent();}catch(e){if(e.name==='TypeError')bad++;}try{native.initEvent(Symbol());}catch(e){if(e.name==='TypeError')bad++;}if(bad!==2||native.type!==type||!native.isTrusted||!native.defaultPrevented)throw 'native conversion failures';
inside.initEvent('reset');if(inside.defaultPrevented||inside.type!=='reset'||inside.detail!==9)throw 'separate reinit';
});
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let button = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("b")).unwrap();
        let outcome = page.dispatch(&mut doc, &mut forms, button, "click", true);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(outcome.prevented);
        let saved = page.runtime.execute("saved;", &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }).unwrap();
        let Value::Host(native) = saved else { panic!("expected native event") };
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert!(!bridge.constructed_event_receiver(native));
        assert!(bridge.get(native, "type").unwrap_err().contains("expired"));
        let ids: HashSet<_> = bridge.state.created_events.iter().map(|event| event.handle).collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(&native));
        let outcome = page.console_eval(&mut doc, &mut forms, "if(first.type!=='first'||second.type!=='second'||inside.type!=='reset'||first===second||inside===first)throw 'persistent IDs';var later=document.createEvent('Event');savedInit.call(later,'after',false,true);savedPrevent.call(later);if(later.type!=='after'||!later.defaultPrevented||later.isTrusted)throw 'saved method borrowing';var expired=false;try{savedInit.call(saved,'bad');}catch(e){expired=e.name==='TypeError';}if(!expired)throw 'expired receiver revived';Promise.resolve(first).then(function(e){if(e!==first||e.isTrusted)throw 'Promise event';document.title='created-dispatch-ok';});");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "created-dispatch-ok");
    }

    #[test]
    fn constructed_event_capacity_and_id_overflow_refuse_atomically() {
        let (mut doc, mut forms, mut page, outcome) = start("");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let mut first = 0;
        for i in 0..MAX_CREATED_EVENTS {
            let Value::Host(handle) = bridge.call(DOCUMENT, "createEvent", &[Value::String("MouseEvents".into())]).unwrap() else { panic!("expected event") };
            if i == 0 { first = handle; }
        }
        let before = (bridge.state.next_event, bridge.state.retained, bridge.state.event.is_some());
        assert!(bridge.call(DOCUMENT, "createEvent", &[Value::String("Event".into())]).unwrap_err().contains("256"));
        assert_eq!((bridge.state.next_event, bridge.state.retained, bridge.state.event.is_some()), before);
        assert_eq!(bridge.state.created_events.len(), MAX_CREATED_EVENTS);
        bridge.call(first, "initEvent", &[Value::String("still-live".into())]).unwrap();
        assert!(matches!(bridge.get(first, "type").unwrap(), Value::String(s) if s == "still-live"));
        let (mut doc, mut forms, mut page, _) = start("");
        page.state.next_event = u32::MAX;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert!(bridge.call(DOCUMENT, "createEvent", &[Value::String("Event".into())]).is_err());
        assert_eq!(bridge.state.next_event, u32::MAX);
        assert!(bridge.state.created_events.is_empty());
    }

    #[test]
    fn constructed_mouse_nullable_native_interfaces_preserve_brands_and_origin_checks() {
        let (mut doc, mut forms, mut page, outcome) = start(r#"
var e=document.createEvent('MouseEvents'),p=document.getElementById('p'),text=document.createTextNode('x'),attr=p.getAttributeNode('title');
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentWindow;
e.initMouseEvent('child',false,false,child,0,0,0,0,0,false,false,false,false,0,child.document.body);
if(e.view!==child||e.relatedTarget!==child.document.body)throw 'real child references';
for(var target of [window,document,p,text,attr,child,child.document,child.document.body]){e.initMouseEvent('target',false,false,null,0,0,0,0,0,false,false,false,false,0,target);if(e.relatedTarget!==target)throw 'real target identity';}
var rejected=0,effects=0,duck={valueOf:function(){effects++;return 0;},toString:function(){effects++;return 'x';}};
for(var view of [document,p,attr,e,p.style,duck,false,42,'']){try{e.initMouseEvent('bad',false,false,view);}catch(x){if(x.name==='TypeError')rejected++;}}
for(var target of [e,p.style,document.querySelectorAll('p'),duck,false,42,'']){try{e.initMouseEvent('bad',false,false,null,0,0,0,0,0,false,false,false,false,0,target);}catch(x){if(x.name==='TypeError')rejected++;}}
if(rejected!==16||effects!==0)throw 'interface brand/coercion';
e.initMouseEvent('nullish',false,false,undefined,0,0,0,0,0,false,false,false,false,0,undefined);if(e.view!==null||e.relatedTarget!==null)throw 'undefined optional interfaces';
var prefix=0;document.title='created-interfaces-ok';
"#);
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "created-interfaces-ok");
        let location = crate::navigation::Location { url: Some(crate::url::parse("https://other.test/").unwrap().0), path: None, fragment: String::new(), root: None };
        page.set_location(&location);
        let outcome = page.console_eval(&mut doc, &mut forms, "var denied=false;try{e.initMouseEvent('denied',false,false,child,{valueOf:function(){prefix++;return 0;}});}catch(x){denied=x.name==='SecurityError';}if(!denied||prefix!==0||e.type!=='nullish')throw 'origin before numeric';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }

    #[test]
    fn constructed_event_name_budget_failures_preserve_all_fields_and_refund_successful_shrink() {
        let (mut doc, mut forms, mut page, outcome) = start("var e=document.createEvent('MouseEvents');e.initMouseEvent('original',true,true,null,8);e.preventDefault();");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let handle = page.state.created_events[0].handle;
        let before = page.state.retained;
        let stamp = page.state.created_events[0].timestamp;
        page.state.retained_limit = before;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert!(bridge.call(handle, "initEvent", &[Value::String("longer-than-original".into())]).is_err());
        assert_eq!(bridge.state.retained, before);
        assert_eq!(bridge.state.created_events[0].name, "original");
        assert!(bridge.state.created_events[0].prevented && bridge.state.created_events[0].bubbles && bridge.state.created_events[0].cancelable);
        bridge.state.copy_work = 0;
        assert!(bridge.call(handle, "initEvent", &[Value::String("x".into())]).is_err());
        assert_eq!(bridge.state.created_events[0].name, "original");
        assert!(bridge.state.created_events[0].prevented);
        bridge.state.copy_work = bridge.state.limits.copy_bytes;
        bridge.call(handle, "initEvent", &[Value::String("x".into())]).unwrap();
        assert_eq!(bridge.state.retained, before - "original".len() + 1);
        assert_eq!(bridge.state.created_events[0].timestamp, stamp);
        assert_eq!(bridge.state.created_events[0].data.ui().unwrap().detail, 8);
        assert!(!bridge.state.created_events[0].prevented);
    }
}

// A record has one UI family. Sharing this storage avoids charging every
// base/custom Event for simultaneous mouse and keyboard payloads.
struct AnimationData {name:Value,elapsed:f64,pseudo:Value}
impl AnimationData {fn payload_bytes(&self)->usize{retained_storage::value_bytes(&self.name)+retained_storage::value_bytes(&self.pseudo)}}
struct ErrorEventData {message:Value,filename:Value,error:Value,line:u32,column:u32}
impl ErrorEventData {fn payload_bytes(&self)->usize{retained_storage::value_bytes(&self.message)+retained_storage::value_bytes(&self.filename)+retained_storage::value_bytes(&self.error)}}
struct WheelData {mouse:MouseData,x:f64,y:f64,z:f64,mode:u32,momentum:bool}

enum UiRecord {
    Base,
    Ui(UiData),
    Mouse(UiData,MouseData),
    Wheel(UiData,Vec<WheelData>),
    Keyboard(UiData,KeyboardData),
    Focus(UiData,Option<u32>),
    Submit(Option<u32>),
    Progress(crate::javascript::xhr_interface::Progress),
    Animation(AnimationData),
    Error(Vec<ErrorEventData>),
}
impl UiRecord {
    fn prepare(ui:Option<UiData>,mouse:Option<MouseData>,keyboard:Option<KeyboardData>)->Result<Self,String>{
        Ok(match (ui,mouse,keyboard) {
            (None,None,None)=>Self::Base,(Some(ui),None,None)=>Self::Ui(ui),
            (Some(ui),Some(mouse),None)=>Self::Mouse(ui,mouse),
            (Some(ui),None,Some(keyboard))=>Self::Keyboard(ui,keyboard),
            _=>return Err("Event UI family data is inconsistent.".into()),
        })
    }
    fn ui(&self)->Option<&UiData>{match self {Self::Ui(ui)|Self::Mouse(ui,_)|Self::Wheel(ui,_)|Self::Keyboard(ui,_)|Self::Focus(ui,_)=>Some(ui),Self::Base|Self::Submit(_)|Self::Progress(_)|Self::Animation(_)|Self::Error(_)=>None}}
    fn ui_mut(&mut self)->Option<&mut UiData>{match self {Self::Ui(ui)|Self::Mouse(ui,_)|Self::Wheel(ui,_)|Self::Keyboard(ui,_)|Self::Focus(ui,_)=>Some(ui),Self::Base|Self::Submit(_)|Self::Progress(_)|Self::Animation(_)|Self::Error(_)=>None}}
    fn mouse(&self)->Option<&MouseData>{match self {Self::Mouse(_,mouse)=>Some(mouse),Self::Wheel(_,data)=>Some(&data[0].mouse),_=>None}}
    fn mouse_mut(&mut self)->Option<&mut MouseData>{match self {Self::Mouse(_,mouse)=>Some(mouse),Self::Wheel(_,data)=>Some(&mut data[0].mouse),_=>None}}
    fn wheel(&self)->Option<&WheelData>{if let Self::Wheel(_,data)=self{data.first()}else{None}}
    fn wheel_bytes(&self)->usize{if let Self::Wheel(_,data)=self{data.capacity()*std::mem::size_of::<WheelData>()}else{0}}
    fn error_event(&self)->Option<&ErrorEventData>{if let Self::Error(data)=self{data.first()}else{None}}
    fn error_bytes(&self)->usize{if let Self::Error(data)=self{data.capacity()*std::mem::size_of::<ErrorEventData>()+data[0].payload_bytes()}else{0}}
    fn animation(&self)->Option<&AnimationData>{if let Self::Animation(data)=self{Some(data)}else{None}}
    fn keyboard(&self)->Option<&KeyboardData>{if let Self::Keyboard(_,key)=self{Some(key)}else{None}}
}

pub(super) struct CreatedEvent {
    pub(super) owner: u32,
    pub(super) composed: bool,
    pub(super) handle: u32,
    pub(super) name: String,
    pub(super) bubbles: bool,
    pub(super) cancelable: bool,
    pub(super) prevented: bool,
    pub(super) stopped: bool,
    pub(super) immediate: bool,
    pub(super) target: Option<u32>,
    pub(super) current: Option<u32>,
    pub(super) phase: u8,
    pub(super) dispatching: bool,
    pub(super) initialized: bool,
    pub(super) trusted: bool,
    pub(super) passive: bool,
    timestamp: f64,
    data: UiRecord,
    custom: Option<Value>,
    message: bool,
}

#[cfg(test)]
#[path = "script_event_storage_tests.rs"]
mod storage_tests;

impl CreatedEvent {
    fn payload_bytes(&self)->usize {self.data.error_bytes()+self.data.wheel_bytes()+self.name.capacity()+owned_custom_bytes(self.custom.as_ref())+self.data.keyboard().map_or(0,KeyboardData::payload_bytes)+self.data.animation().map_or(0,AnimationData::payload_bytes)}
}
fn owned_custom_bytes(value:Option<&Value>)->usize {value.map_or(0,retained_storage::value_bytes)}

#[derive(Clone,Copy)]
enum Interface { Base, Custom, Ui, Mouse, Keyboard, Focus }

fn interface(value: Option<&Value>) -> Option<Interface> {
    let Some(Value::String(name)) = value else { return None; };
    if name.eq_ignore_ascii_case("CustomEvent") { Some(Interface::Custom) }
    else if ["MouseEvent", "MouseEvents"].iter().any(|s| name.eq_ignore_ascii_case(s)) { Some(Interface::Mouse) }
    else if name.eq_ignore_ascii_case("KeyboardEvent") {Some(Interface::Keyboard)}
    else if name.eq_ignore_ascii_case("FocusEvent") {Some(Interface::Focus)}
    else if ["UIEvent", "UIEvents"].iter().any(|s| name.eq_ignore_ascii_case(s)) { Some(Interface::Ui) }
    else if ["Event", "Events", "HTMLEvents", "SVGEvents"].iter().any(|s| name.eq_ignore_ascii_case(s)) { Some(Interface::Base) }
    else { None }
}

fn custom_bytes(value: Option<&Value>) -> Result<usize, &'static str> {
    // Shared primitive payload accounting. A valid Vec<u16> cannot have more
    // than usize::MAX / 2 live units; saturation also exceeds the finite cap.
    Ok(value.map_or(0, Value::bytes))
}

fn copy_custom_value(value:&Value)->Result<Value,String> {retained_storage::copy_value(value)}

fn constant(name: &str) -> Option<u8> {
    match name { "NONE" => Some(0), "CAPTURING_PHASE" => Some(1), "AT_TARGET" => Some(2), "BUBBLING_PHASE" => Some(3), _ => None }
}

fn base_property(name: &str) -> bool {
    constant(name).is_some() || matches!(name, "type" | "target" | "currentTarget" | "bubbles" | "cancelable"
        | "defaultPrevented" | "eventPhase" | "timeStamp" | "isTrusted" | "composed" | "cancelBubble"
        | "srcElement" | "returnValue")
}

fn ui_property(name:&str)->bool { matches!(name,"view"|"detail"|"which") }

fn mouse_property(name: &str) -> bool {
    matches!(name, "screenX" | "screenY" | "clientX" | "clientY" | "x" | "y" | "pageX" | "pageY" | "offsetX" | "offsetY"
        | "ctrlKey" | "altKey" | "shiftKey" | "metaKey" | "button" | "buttons" | "relatedTarget" | "layerX" | "layerY")
}

impl Bridge<'_> {
    pub(super) fn event_owner(&self,object:u32)->Option<u32> {
        self.created_event_index(object).map(|id|self.state.created_events[id].owner)
            .or_else(||self.state.event.as_ref().filter(|event|event.handle==object).map(|_|WINDOW))
    }
    pub(super) fn construct_base_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        self.construct_event_record(owner,args,fields,None,None,None,None)
    }
    pub(super) fn ordinary_event_owner(&self,object:u32)->Option<u32> {
        self.created_event_index(object).map(|id|self.state.created_events[id].owner)
            .or_else(||self.state.event.as_ref().filter(|event|event.handle==object).map(|_|WINDOW))
    }
    pub(super) fn ui_view_argument(&self,owner:u32,value:Option<&Value>)->Result<Option<u32>,(&'static str,String)> {
        let Some(value)=value else {return Ok(None)};
        if matches!(value,Value::Null|Value::Undefined) {return Ok(None);}
        let Value::Host(window)=value else {return Err(("TypeError","UIEvent view requires a genuine Window or null.".into()));};
        if !self.event_target_interface(*window) {return Err(("TypeError","UIEvent view requires a genuine Window or null.".into()));}
        if owner!=*window {
            self.frame_window_access(Some(owner),*window).map_err(|message|("SecurityError",message))?;
        }
        Ok(Some(*window))
    }
    pub(super) fn construct_ui_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value));
        let detail=field("detail").map(timer_integer).transpose()?.unwrap_or(0);
        let view=self.ui_view_argument(owner,field("view")).map_err(|(_,message)|message)?;
        self.construct_event_record(owner,args,fields,Some(UiData {view,detail}),None,None,None)
    }
    pub(super) fn mouse_target_argument(&self,owner:u32,value:Option<&Value>)->Result<Option<u32>,(&'static str,String)> {
        let Some(value)=value else {return Ok(None)};
        if matches!(value,Value::Null|Value::Undefined) {return Ok(None);}
        let Value::Host(target)=value else {return Err(("TypeError","Event relatedTarget requires a genuine EventTarget or null.".into()));};
        let target_owner=if self.event_target_interface(*target) {Some(*target)}
            else if let Some(window)=self.standalone_target_owner(*target) {Some(window)}
            else if self.frame_node_interface(*target).is_some() {self.frame_instance_window(*target)}
            else {self.dispatch_receiver(*target).then_some(WINDOW)}
            .ok_or_else(||("TypeError","Event relatedTarget requires a genuine EventTarget or null.".into()))?;
        if owner!=target_owner {self.frame_window_access(Some(owner),target_owner).map_err(|message|("SecurityError",message))?;}
        Ok(Some(*target))
    }
    pub(super) fn construct_mouse_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value));
        let integer=|key|field(key).map(timer_integer).transpose().map(|value|value.unwrap_or(0));
        let coordinate=|key|match field(key) {
            None|Some(Value::Undefined)=>Ok(0.0),
            Some(Value::Number(value)) if value.is_finite()=>Ok(*value),
            _=>Err("MouseEvent coordinates require converted finite doubles.".to_string()),
        };
        let flag=|key|field(key).is_some_and(Value::truthy);
        let ui=UiData {detail:integer("detail")?,view:self.ui_view_argument(owner,field("view")).map_err(|(_,message)|message)?};
        let button=integer("button")? as i16;
        let mut modifiers=0u16;
        for &(name,_,bit) in EXTRA_MODIFIERS {if flag(name) {modifiers|=bit;}}
        let mouse=MouseData {screen_x:coordinate("screenX")?,screen_y:coordinate("screenY")?,
            client_x:coordinate("clientX")?,client_y:coordinate("clientY")?,layer_x:0,layer_y:0,
            ctrl:flag("ctrlKey"),alt:flag("altKey"),shift:flag("shiftKey"),meta:flag("metaKey"),
            // Selected nonnormative UI Events supplemental getter behavior.
            // The separate optional which-init dictionary extension is absent.
            button,buttons:integer("buttons")? as u16,modifiers,which:(i32::from(button)+1) as u32,
            related:self.mouse_target_argument(owner,field("relatedTarget")).map_err(|(_,message)|message)?};
        self.construct_event_record(owner,args,fields,Some(ui),Some(mouse),None,None)
    }
    // Only wheel instances pay for this backing. Keep the shared base-event
    // arena unchanged; admit the actual allocation before publishing a record.
    pub(super) fn construct_wheel_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String>{
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value));
        let number=|key|match field(key){None|Some(Value::Undefined)=>Ok(0.0),Some(Value::Number(n)) if n.is_finite()=>Ok(*n),_=>Err("WheelEvent deltas require converted finite doubles.".to_string())};
        let (x,y,z)=(number("deltaX")?,number("deltaY")?,number("deltaZ")?);
        let mode=field("deltaMode").map(timer_integer).transpose()?.unwrap_or(0) as u32;
        let momentum=field("momentum").is_some_and(Value::truthy);
        let mut data=Vec::new();data.try_reserve_exact(1).map_err(|_|"Could not reserve WheelEvent payload.")?;
        let bytes=data.capacity()*std::mem::size_of::<WheelData>();
        if bytes>self.state.retained_limit.saturating_sub(self.state.retained){return Err("WheelEvent retained payload refused.".into());}
        self.state.retained+=bytes;
        let value=match self.construct_mouse_event(owner,args,fields){Ok(value)=>value,Err(error)=>{self.state.retained-=bytes;return Err(error);}};
        let Value::Host(handle)=value else {unreachable!()};let id=self.created_event_index(handle).unwrap();
        let UiRecord::Mouse(ui,mouse)=std::mem::replace(&mut self.state.created_events[id].data,UiRecord::Base) else {unreachable!()};
        data.push(WheelData {mouse,x,y,z,mode,momentum});self.state.created_events[id].data=UiRecord::Wheel(ui,data);Ok(value)
    }
    pub(super) fn wheel_receiver(&self,handle:u32)->bool{self.created_event_index(handle).is_some_and(|id|self.state.created_events[id].data.wheel().is_some())}
    pub(super) fn wheel_property(&self,object:u32,name:&str)->bool{self.wheel_receiver(object)&&matches!(name,"deltaX"|"deltaY"|"deltaZ"|"deltaMode"|"momentum")}
    pub(super) fn construct_keyboard_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value));
        let integer=|key|field(key).map(timer_integer).transpose().map(|value|value.unwrap_or(0));
        let flag=|key|field(key).is_some_and(Value::truthy);
        let ui=UiData {detail:integer("detail")?,view:self.ui_view_argument(owner,field("view")).map_err(|(_,message)|message)?};
        let empty=Value::String(String::new());
        let key=field("key").filter(|value|!matches!(value,Value::Undefined)).unwrap_or(&empty);
        let code=field("code").filter(|value|!matches!(value,Value::Undefined)).unwrap_or(&empty);
        if !matches!(key,Value::String(_)|Value::StringUnits(_))||!matches!(code,Value::String(_)|Value::StringUnits(_)){return Err("KeyboardEvent key/code require converted DOMStrings.".into());}
        let bytes=key.bytes().checked_add(code.bytes()).ok_or("KeyboardEvent string storage overflow.")?;
        if key.bytes()>self.state.limits.host.string_bytes||code.bytes()>self.state.limits.host.string_bytes
            || bytes>self.state.retained_limit.saturating_sub(self.state.retained){return Err("KeyboardEvent string allowance exceeded.".into());}
        self.state.copies(bytes)?;
        let mut modifiers=0u16;for &(name,_,bit) in EXTRA_MODIFIERS {if flag(name){modifiers|=bit;}}
        let keyboard=KeyboardData {key:copy_custom_value(key)?,code:copy_custom_value(code)?,location:integer("location")? as u32,
            ctrl:flag("ctrlKey"),alt:flag("altKey"),shift:flag("shiftKey"),meta:flag("metaKey"),repeat:flag("repeat"),
            composing:flag("isComposing"),key_code:integer("keyCode")? as u32,char_code:integer("charCode")? as u32,modifiers};
        self.construct_event_record(owner,args,fields,Some(ui),None,Some(keyboard),None)
    }
    pub(super) fn submit_event_argument(&self,owner:u32,value:Option<&Value>)->Result<Option<u32>,(&'static str,String)>{
        let Some(value)=value else{return Ok(None)};
        if matches!(value,Value::Null|Value::Undefined){return Ok(None);}
        let Value::Host(target)=value else{return Err(("TypeError","SubmitEvent submitter requires a genuine HTMLElement or null.".into()));};
        let html=node_index(*target).is_some_and(|node|self.html_class_element(node))
            ||self.inert_interface(*target).is_some_and(|name|name.starts_with("HTML")&&name.ends_with("Element"))
            ||self.frame_concrete_html_interface(*target).is_some()||self.frame_node_interface(*target)==Some("HTMLElement");
        if !html{return Err(("TypeError","SubmitEvent submitter requires a genuine HTMLElement or null.".into()));}
        self.native_node_access(*target)?;
        let target_owner=self.frame_instance_window(*target).unwrap_or(WINDOW);
        if owner!=target_owner{self.frame_window_access(Some(owner),target_owner).map_err(|e|("SecurityError",e))?;}
        Ok(Some(*target))
    }
    pub(super) fn construct_submit_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String>{
        script_text_content::deadline(self.state)?;
        let submitter=self.submit_event_argument(owner,fields.and_then(|f|f.iter().find(|(n,_)|*n=="submitter").map(|(_,v)|v))).map_err(|(_,e)|e)?;
        let value=self.construct_base_event(owner,args,fields)?;
        let Value::Host(handle)=value else{unreachable!()};let id=self.created_event_index(handle).unwrap();
        self.state.created_events[id].data=UiRecord::Submit(submitter);Ok(value)
    }
    pub(super) fn submit_event_receiver(&self,object:u32)->bool{
        self.created_event_index(object).is_some_and(|id|matches!(self.state.created_events[id].data,UiRecord::Submit(_)))
            ||self.state.event.as_ref().is_some_and(|e|e.handle==object&&e.submitter.is_some())
    }
    pub(super) fn submit_event_value(&mut self,object:u32)->Result<Value,String>{
        self.state.work(self.state.event_lookup_work(8))?;
        let submitter=if let Some(id)=self.created_event_index(object){
            let UiRecord::Submit(value)=self.state.created_events[id].data else{return Err("SubmitEvent receiver is invalid.".into())};value
        }else{self.state.event.as_ref().filter(|e|e.handle==object).and_then(|e|e.submitter).ok_or("SubmitEvent has expired.")?};
        if let Some(target)=submitter{self.native_node_access(target).map_err(|(_,e)|e)?;}
        Ok(submitter.map_or(Value::Null,Value::Host))
    }
    pub(super) fn construct_focus_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value));
        let related=self.mouse_target_argument(owner,field("relatedTarget")).map_err(|(_,message)|message)?;
        let detail=field("detail").map(timer_integer).transpose()?.unwrap_or(0);
        let view=self.ui_view_argument(owner,field("view")).map_err(|(_,message)|message)?;
        let value=self.construct_event_record(owner,args,fields,Some(UiData {view,detail}),None,None,None)?;
        // The Focus payload fits the existing UI family record; publication is
        // infallible and does not add storage to every base/custom Event.
        let Value::Host(handle)=value else {unreachable!()};
        let id=self.created_event_index(handle).unwrap();
        self.state.created_events[id].data=UiRecord::Focus(UiData {view,detail},related);
        Ok(value)
    }
    pub(super) fn construct_error_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String>{
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value)).filter(|value|!matches!(value,Value::Undefined));
        let empty=Value::String(String::new());let undefined=Value::Undefined;
        let message=field("message").unwrap_or(&empty);let filename=field("filename").unwrap_or(&empty);let error=field("error").unwrap_or(&undefined);
        if !matches!(message,Value::String(_)|Value::StringUnits(_))||!matches!(filename,Value::String(_)|Value::StringUnits(_)){return Err("ErrorEvent text fields require converted strings.".into());}
        let line=field("lineno").map(timer_integer).transpose()?.unwrap_or(0) as u32;let column=field("colno").map(timer_integer).transpose()?.unwrap_or(0) as u32;
        let filename_bytes=match filename{Value::String(text)=>text.len(),Value::StringUnits(units)=>{self.state.work(units.len().saturating_mul(2))?;std::char::decode_utf16(units.iter().copied()).try_fold(0usize,|bytes,ch|bytes.checked_add(ch.unwrap_or(char::REPLACEMENT_CHARACTER).len_utf8())).ok_or("ErrorEvent filename size overflow.")?},_=>unreachable!()};
        if message.bytes()>self.state.limits.host.string_bytes||filename_bytes>self.state.limits.host.string_bytes||error.bytes()>self.state.limits.host.string_bytes{return Err("ErrorEvent text allowance exceeded.".into());}
        let copies=message.bytes().checked_add(filename_bytes).and_then(|bytes|bytes.checked_add(error.bytes())).ok_or("ErrorEvent payload size overflow.")?;
        if copies>self.state.retained_limit.saturating_sub(self.state.retained){return Err("ErrorEvent retained payload refused.".into());}self.state.copies(copies)?;
        let mut filename_text=String::new();filename_text.try_reserve_exact(filename_bytes).map_err(|_|"Could not reserve ErrorEvent filename.")?;
        match filename{Value::String(text)=>filename_text.push_str(text),Value::StringUnits(units)=>{for ch in std::char::decode_utf16(units.iter().copied()){filename_text.push(ch.unwrap_or(char::REPLACEMENT_CHARACTER));}},_=>unreachable!()}
        let mut data=Vec::new();data.try_reserve_exact(1).map_err(|_|"Could not reserve ErrorEvent record.")?;
        data.push(ErrorEventData{message:copy_custom_value(message)?,filename:Value::String(filename_text),error:copy_custom_value(error)?,line,column});
        let payload=data.capacity().checked_mul(std::mem::size_of::<ErrorEventData>()).and_then(|bytes|bytes.checked_add(data[0].payload_bytes())).ok_or("ErrorEvent backing size overflow.")?;
        if payload>self.state.retained_limit.saturating_sub(self.state.retained){return Err("ErrorEvent retained backing refused.".into());}
        // Prepay only this error instance. No callback or fallible operation
        // occurs between base publication and attaching its reserved payload.
        self.state.retained+=payload;
        let value=match self.construct_base_event(owner,args,fields){Ok(value)=>value,Err(error)=>{self.state.retained-=payload;return Err(error);}};
        let Value::Host(handle)=value else{unreachable!()};let id=self.created_event_index(handle).unwrap();self.state.created_events[id].data=UiRecord::Error(data);Ok(value)
    }
    pub(super) fn error_event_receiver(&self,handle:u32)->bool{self.created_event_index(handle).is_some_and(|id|self.state.created_events[id].data.error_event().is_some())}
    pub(super) fn error_event_property(&self,object:u32,name:&str)->bool{self.error_event_receiver(object)&&matches!(name,"message"|"filename"|"lineno"|"colno"|"error")}
    pub(super) fn construct_animation_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value));
        let empty=Value::String(String::new());
        let name=field("animationName").filter(|value|!matches!(value,Value::Undefined)).unwrap_or(&empty);
        let pseudo=field("pseudoElement").filter(|value|!matches!(value,Value::Undefined)).unwrap_or(&empty);
        if !matches!(name,Value::String(_)|Value::StringUnits(_))||!matches!(pseudo,Value::String(_)|Value::StringUnits(_)) {return Err("AnimationEvent fields require converted DOMStrings.".into());}
        let elapsed=field("elapsedTime").filter(|value|!matches!(value,Value::Undefined)).map_or(0.0,Value::to_number);
        if !elapsed.is_finite(){return Err("AnimationEvent elapsedTime requires a finite double.".into());}
        let bytes=name.bytes().checked_add(pseudo.bytes()).ok_or("AnimationEvent string size overflow.")?;
        if name.bytes()>self.state.limits.host.string_bytes||pseudo.bytes()>self.state.limits.host.string_bytes||bytes>self.state.retained_limit.saturating_sub(self.state.retained){return Err("AnimationEvent string allowance exceeded.".into());}
        self.state.copies(bytes)?;
        let data=AnimationData {name:copy_custom_value(name)?,pseudo:copy_custom_value(pseudo)?,elapsed};
        let payload=data.payload_bytes();if payload>self.state.retained_limit.saturating_sub(self.state.retained){return Err("AnimationEvent retained payload refused.".into());}
        // Pay the additional owned strings before base-record/backing admission.
        // No author callback runs here; failure returns the exact charge.
        self.state.retained+=payload;
        let value=match self.construct_base_event(owner,args,fields) {Ok(value)=>value,Err(error)=>{self.state.retained-=payload;return Err(error);}};
        let Value::Host(handle)=value else {unreachable!()};let id=self.created_event_index(handle).unwrap();
        self.state.created_events[id].data=UiRecord::Animation(data);Ok(value)
    }
    pub(super) fn animation_receiver(&self,handle:u32)->bool{self.created_event_index(handle).is_some_and(|id|self.state.created_events[id].data.animation().is_some())}
    pub(super) fn animation_property(&self,object:u32,name:&str)->bool{self.animation_receiver(object)&&matches!(name,"animationName"|"elapsedTime"|"pseudoElement")}
    pub(super) fn construct_progress_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        let field=|key|fields.and_then(|fields|fields.iter().find(|(name,_)|*name==key).map(|(_,value)|value));
        let integer=|key| {let value=field(key).map_or(0.0,Value::to_number);if !value.is_finite() {0}else{value.trunc().rem_euclid(18446744073709551616.0) as u64}};
        let progress=crate::javascript::xhr_interface::Progress {length_computable:field("lengthComputable").is_some_and(Value::truthy),loaded:integer("loaded"),total:integer("total")};
        let value=self.construct_base_event(owner,args,fields)?;
        let Value::Host(handle)=value else {unreachable!()};let id=self.created_event_index(handle).unwrap();
        self.state.created_events[id].data=UiRecord::Progress(progress);Ok(value)
    }
    pub(super) fn progress_receiver(&self,handle:u32)->bool {self.created_event_index(handle).is_some_and(|id|matches!(self.state.created_events[id].data,UiRecord::Progress(_)))}
    pub(super) fn progress_property(&self,object:u32,name:&str)->bool {self.progress_receiver(object)&&matches!(name,"lengthComputable"|"loaded"|"total")}
    pub(super) fn focus_receiver(&self,handle:u32)->bool {
        self.created_event_index(handle).is_some_and(|id|matches!(self.state.created_events[id].data,UiRecord::Focus(..)))
            || self.state.event.as_ref().is_some_and(|event|event.handle==handle&&event.focus.is_some())
    }
    pub(super) fn keyboard_receiver(&self,handle:u32)->bool {
        self.created_event_index(handle).is_some_and(|id|self.state.created_events[id].data.keyboard().is_some())
            || self.state.event.as_ref().is_some_and(|event|event.handle==handle&&event.keyboard.is_some())
    }
    pub(super) fn keyboard_modifier_state(&mut self,object:u32,args:&[Value])->Result<Value,String> {
        let Some(Value::String(name))=args.first() else {return Err("KeyboardEvent modifier requires a converted DOMString.".into());};
        self.state.work(EXTRA_MODIFIERS.len()+4)?;
        let (ctrl,alt,shift,meta,modifiers)=if let Some(id)=self.created_event_index(object) {
            let key=self.state.created_events[id].data.keyboard().ok_or("KeyboardEvent modifier receiver is invalid.")?;
            (key.ctrl,key.alt,key.shift,key.meta,key.modifiers)
        } else {
            let key=self.state.event.as_ref().filter(|event|event.handle==object).and_then(|event|event.keyboard.as_ref()).ok_or("KeyboardEvent has expired.")?;
            let modifiers=u16::from(key.alt_graph)|u16::from(key.caps_lock)*2
                |u16::from(key.num_lock)*32|u16::from(key.scroll_lock)*64;
            (key.ctrl,key.alt,key.shift,key.meta,modifiers)
        };
        Ok(Value::Bool(match name.as_str(){"Control"=>ctrl,"Alt"=>alt,"Shift"=>shift,"Meta"=>meta,
            _=>EXTRA_MODIFIERS.iter().any(|(_,key,bit)|*key==name.as_str()&&modifiers&*bit!=0)}))
    }
    pub(super) fn construct_custom_event(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>)->Result<Value,String> {
        let default=Value::Null;
        let detail=fields.and_then(|fields|fields.iter().find(|(name,_)|*name=="detail").map(|(_,value)|value))
            .filter(|value|!matches!(value,Value::Undefined)).unwrap_or(&default);
        self.construct_event_record(owner,args,fields,None,None,None,Some(detail))
    }
    fn construct_event_record(&mut self,owner:u32,args:&[Value],fields:Option<&[(&str,Value)]>,ui:Option<UiData>,mouse:Option<MouseData>,keyboard:Option<KeyboardData>,custom:Option<&Value>)->Result<Value,String> {
        if !self.state.enabled||!self.doc.scripting_enabled {return Err("Event construction is disabled.".into());}
        self.state.work(self.state.event_lookup_work(8).saturating_add(1))?;
        let Some(Value::String(source))=args.first() else {return Err("Event requires a converted type.".into());};
        if self.state.event_type_refused(source.len()) {return Err("Event type exceeds the host event-type/string allowance.".into());}
        let detail_bytes=custom_bytes(custom)?;
        if detail_bytes>self.state.limits.host.string_bytes {return Err("CustomEvent detail exceeds the host string bound.".into());}
        let bytes=source.len().checked_add(detail_bytes).ok_or("Event retained text size overflow.")?;
        let keyboard_bytes=keyboard.as_ref().map_or(0,KeyboardData::payload_bytes);
        let required=bytes.checked_add(keyboard_bytes).ok_or("Event keyboard payload overflow.")?;
        if required>self.state.retained_limit.saturating_sub(self.state.retained){return Err("Event retained payload allowance exceeded.".into());}
        self.state.copies(bytes)?;
        if self.state.created_event_limit_reached(){return Err("Constructed Event allowance exceeded.".into());}
        let next=self.state.next_event.checked_add(1).filter(|next|*next<=script_class_list::BASE).ok_or("Event identity space exhausted.")?;
        let mut name=String::new();name.try_reserve_exact(source.len()).map_err(|_|"Could not reserve Event type.")?;name.push_str(source);
        let custom=custom.map(copy_custom_value).transpose()?;
        let payload=name.capacity().checked_add(owned_custom_bytes(custom.as_ref())).and_then(|bytes|bytes.checked_add(keyboard_bytes)).ok_or("Event payload storage overflow.")?;
        self.state.copies(payload.saturating_sub(required))?;
        let (replacement,added)=self.state.prepare_created_event_storage(payload)?;
        let handle=self.state.next_event;
        let timestamp=self.event_timestamp(owner,Instant::now())?;
        let data=UiRecord::prepare(ui,mouse,keyboard)?;
        if let Some(mut records)=replacement {records.append(&mut self.state.created_events);self.state.created_events=records;}
        let flag=|key|fields.is_some_and(|fields|fields.iter().any(|(name,value)|*name==key&&value.truthy()));
        let event=CreatedEvent {owner,composed:flag("composed"),handle,name,
            bubbles:flag("bubbles"),cancelable:flag("cancelable"),prevented:false,stopped:false,immediate:false,
            target:None,current:None,phase:0,dispatching:false,initialized:true,trusted:false,passive:false,
            timestamp,data,custom,message:false};
        self.state.created_events.push(event);
        self.state.retained+=payload+added;
        self.state.next_event=next;
        Ok(Value::Host(handle))
    }
    pub(super) fn rollback_base_event(&mut self,handle:u32)->Result<(),String> {
        let Some(event)=self.state.created_events.last() else {return Err("No unpublished Event record exists.".into());};
        if event.handle!=handle||event.dispatching||event.target.is_some() {return Err("Unpublished Event rollback identity mismatch.".into());}
        let bytes=event.payload_bytes();
        self.state.retained=self.state.retained.checked_sub(bytes).ok_or("Event rollback retention mismatch.")?;
        self.state.created_events.pop();
        if self.state.created_events.is_empty() {
            self.state.retained-=self.state.created_events.capacity()*std::mem::size_of::<CreatedEvent>();
            self.state.created_events=Vec::new();
        }
        // Identity allocation and work remain charged; no capability ID is reused.
        Ok(())
    }
    pub(super) fn set_event_return_value(&mut self,object:u32,value:&Value)->Result<(),String> {
        self.state.work(1)?;
        if !value.truthy() {
            if let Some(id)=self.created_event_index(object) {
                let event=&mut self.state.created_events[id];
                if event.cancelable&&!event.passive {event.prevented=true;}
            } else {
                let event=self.state.event.as_mut().filter(|event|event.handle==object).ok_or("Event has expired.")?;
                if event.cancelable&&!event.passive {event.prevented=true;}
            }
        }
        Ok(())
    }
    pub(super) fn event_native_member(&self,object:u32,name:&str)->bool {
        base_property(name) || self.progress_property(object,name) || self.animation_property(object,name) || self.error_event_property(object,name) || self.wheel_property(object,name) || matches!(name,"constructor"|"__proto__"|"srcElement"|"returnValue"|"composedPath"|"initEvent"|"preventDefault"|"stopPropagation"|"stopImmediatePropagation")
            || self.keyboard_receiver(object) && (keyboard_property(name)||name=="getModifierState")
            || self.custom_receiver(object) && matches!(name,"detail"|"initCustomEvent")
            || self.focus_receiver(object) && name=="relatedTarget"
            || self.submit_event_receiver(object) && name=="submitter"
            || self.ui_receiver(object) && (ui_property(name) || matches!(name,"initUIEvent"|"which"|"sourceCapabilities"))
            || self.mouse_receiver(object) && (mouse_property(name) || matches!(name,"initMouseEvent"|"initUIEvent"|"which"|"x"|"y"|"offsetX"|"offsetY"|"movementX"|"movementY"|"pageX"|"pageY"|"layerX"|"layerY"|"getModifierState"|"sourceCapabilities"))
    }
    pub(super) fn event_factory_enabled(&self) -> bool {
        #[cfg(test)]
        if self.state.disable_event_factory { return false; }
        true
    }

    pub(super) fn created_event_index(&self, handle: u32) -> Option<usize> {
        (EVENT_BASE..script_class_list::BASE).contains(&handle).then(|| self.state.created_events.binary_search_by_key(&handle,|event|event.handle).ok()).flatten()
    }

    pub(super) fn constructed_event_receiver(&self, handle: u32) -> bool {
        self.created_event_index(handle).is_some()
    }

    pub(super) fn event_receiver(&self, handle: u32) -> bool {
        self.created_event_index(handle).is_some() || self.state.event.as_ref().is_some_and(|e| e.handle == handle)
    }

    pub(super) fn mouse_receiver(&self, handle: u32) -> bool {
        self.created_event_index(handle).is_some_and(|id| self.state.created_events[id].data.mouse().is_some())
            || self.state.event.as_ref().is_some_and(|event|event.handle==handle&&event.mouse.is_some())
    }

    pub(super) fn ui_receiver(&self,handle:u32)->bool {
        self.created_event_index(handle).is_some_and(|id|self.state.created_events[id].data.ui().is_some())
            || self.state.event.as_ref().is_some_and(|event|event.handle==handle && (event.keyboard.is_some() || event.mouse.is_some() || event.focus.is_some()))
    }

    pub(super) fn custom_receiver(&self, handle:u32) -> bool {
        self.created_event_index(handle).is_some_and(|id| {
            let event = &self.state.created_events[id];
            event.custom.is_some() && !event.message
        })
    }

    pub(super) fn message_event_receiver(&self, handle: u32) -> bool {
        self.created_event_index(handle).is_some_and(|id| self.state.created_events[id].message)
    }

    pub(super) fn construct_message_event(&mut self, data: &Value) -> Result<u32, String> {
        let Value::Host(handle) = self.construct_event_record(
            WINDOW, &[Value::String("message".into())], None, None, None, None, Some(data),
        )? else { unreachable!() };
        let index = self.created_event_index(handle).unwrap();
        self.state.created_events[index].message = true;
        Ok(handle)
    }

    pub(super) fn message_event_data(&mut self, handle: u32) -> Result<Value, String> {
        let index = self.created_event_index(handle).ok_or("MessageEvent is unavailable.")?;
        let value = self.state.created_events[index].custom.as_ref().ok_or("MessageEvent payload is unavailable.")?;
        self.state.copies(value.bytes())?;
        copy_custom_value(self.state.created_events[index].custom.as_ref().unwrap())
    }

    pub(super) fn created_event_method(&self, object: u32, name: &str) -> Option<&'static str> {
        if !self.event_receiver(object) { return None; }
        match name {
            "initEvent" => Some("initEvent"),
            "initCustomEvent" if self.custom_receiver(object) => Some("initCustomEvent"),
            "initUIEvent" if self.ui_receiver(object) => Some("initUIEvent"),
            "initMouseEvent" if self.mouse_receiver(object) => Some("initMouseEvent"),
            "getModifierState" if self.mouse_receiver(object) => Some("getModifierState"),
            "preventDefault" => Some("preventDefault"),
            "stopPropagation" => Some("stopPropagation"),
            "stopImmediatePropagation" => Some("stopImmediatePropagation"),
            _ => None,
        }
    }

    pub(super) fn created_event_readonly(&self, object: u32, name: &str) -> bool {
        !matches!(name,"cancelBubble"|"returnValue") && (self.progress_property(object,name) || self.animation_property(object,name) || self.error_event_property(object,name) || self.wheel_property(object,name) || (self.focus_receiver(object)&&name=="relatedTarget"||self.submit_event_receiver(object)&&name=="submitter") || self.keyboard_receiver(object)&&keyboard_property(name) || self.created_event_index(object).is_some_and(|id| base_property(name)
            || self.state.created_events[id].data.mouse().is_some() && mouse_property(name)
            || self.custom_receiver(object) && name == "detail")
            || self.ui_receiver(object) && ui_property(name)
            || self.state.event.as_ref().is_some_and(|event|event.handle==object&&event.mouse.is_some()) && mouse_property(name))
    }

    pub(super) fn created_event_has(&mut self, object: u32, name: &str) -> Option<Result<bool, String>> {
        let id = self.created_event_index(object)?;
        Some(self.state.work(self.state.event_lookup_work(8)).map(|()| {
            base_property(name) || self.progress_property(object,name) || self.animation_property(object,name) || self.error_event_property(object,name) || self.wheel_property(object,name) || (self.focus_receiver(object)&&name=="relatedTarget"||self.submit_event_receiver(object)&&name=="submitter") || self.keyboard_receiver(object)&&keyboard_property(name) || self.state.created_events[id].data.mouse().is_some() && mouse_property(name)
                || self.custom_receiver(object) && name == "detail"
                || self.state.created_events[id].data.ui().is_some() && ui_property(name)
                || self.created_event_method(object, name).is_some()
        }))
    }

    pub(super) fn created_event_get(&mut self, object: u32, name: &str) -> Option<Result<Value, String>> {
        let id = self.created_event_index(object)?;
        Some((|| {
            self.state.work(self.state.event_lookup_work(8))?;
            if name == "type" {
                let len = self.state.created_events[id].name.len();
                self.state.copies(len)?;
                let mut copied=String::new();
                copied.try_reserve_exact(len).map_err(|_|"Could not reserve Event type result.")?;
                copied.push_str(&self.state.created_events[id].name);
                return Ok(Value::String(copied));
            }
            if matches!(name,"key"|"code") && self.keyboard_receiver(object) {
                let key=self.state.created_events[id].data.keyboard().unwrap();
                let value=if name=="key"{&key.key}else{&key.code};
                self.state.copies(value.bytes())?;
                return copy_custom_value(if name=="key"{&self.state.created_events[id].data.keyboard().unwrap().key}else{&self.state.created_events[id].data.keyboard().unwrap().code});
            }
            if matches!(name,"message"|"filename"|"error")&&self.error_event_receiver(object){
                let data=self.state.created_events[id].data.error_event().unwrap();let value=match name{"message"=>&data.message,"filename"=>&data.filename,_=>&data.error};self.state.copies(value.bytes())?;
                let data=self.state.created_events[id].data.error_event().unwrap();return copy_custom_value(match name{"message"=>&data.message,"filename"=>&data.filename,_=>&data.error});
            }
            if matches!(name,"animationName"|"pseudoElement")&&self.animation_receiver(object) {
                let data=self.state.created_events[id].data.animation().unwrap();let value=if name=="animationName"{&data.name}else{&data.pseudo};
                self.state.copies(value.bytes())?;
                let data=self.state.created_events[id].data.animation().unwrap();return copy_custom_value(if name=="animationName"{&data.name}else{&data.pseudo});
            }
            if name == "detail" && self.custom_receiver(object) {
                let bytes = custom_bytes(self.state.created_events[id].custom.as_ref())?;
                if bytes > self.state.limits.host.string_bytes {return Err("CustomEvent detail exceeds the host string bound.".into());}
                self.state.copies(bytes)?;
                return copy_custom_value(self.state.created_events[id].custom.as_ref().unwrap());
            }
            let event = &self.state.created_events[id];
            let native = |handle: Option<u32>| handle.map_or(Value::Null, Value::Host);
            Ok(match name {
                "deltaX"|"deltaY"|"deltaZ"|"deltaMode" if event.data.wheel().is_some()=>{let d=event.data.wheel().unwrap();Value::Number(match name{"deltaX"=>d.x,"deltaY"=>d.y,"deltaZ"=>d.z,_=>f64::from(d.mode)})},
                "momentum" if event.data.wheel().is_some()=>Value::Bool(event.data.wheel().unwrap().momentum),
                "lineno"|"colno" if event.data.error_event().is_some()=>{let data=event.data.error_event().unwrap();Value::Number(f64::from(if name=="lineno"{data.line}else{data.column}))},
                "elapsedTime" if event.data.animation().is_some()=>Value::Number(event.data.animation().unwrap().elapsed),
                "lengthComputable" if matches!(event.data,UiRecord::Progress(_)) => {let UiRecord::Progress(data)=event.data else {unreachable!()};Value::Bool(data.length_computable)},
                "loaded"|"total" if matches!(event.data,UiRecord::Progress(_)) => {let UiRecord::Progress(data)=event.data else {unreachable!()};Value::Number(if name=="loaded" {data.loaded}else{data.total} as f64)},
                "relatedTarget" if matches!(event.data,UiRecord::Focus(..)) => {let UiRecord::Focus(_,related)=&event.data else {unreachable!()};native(*related)},
                "target" | "srcElement" => native(event.target),
                "currentTarget" => native(event.current),
                "bubbles" => Value::Bool(event.bubbles),
                "cancelable" => Value::Bool(event.cancelable),
                "defaultPrevented" => Value::Bool(event.prevented),
                "returnValue" => Value::Bool(!event.prevented),
                "cancelBubble" => Value::Bool(event.stopped),
                "eventPhase" => Value::Number(f64::from(event.phase)),
                "timeStamp" => Value::Number(event.timestamp),
                "isTrusted" => Value::Bool(event.trusted),
                "composed" => Value::Bool(event.composed),
                name if constant(name).is_some() => Value::Number(f64::from(constant(name).unwrap())),
                "view" if event.data.ui().is_some() => native(event.data.ui().unwrap().view),
                "detail" if event.data.ui().is_some() => Value::Number(f64::from(event.data.ui().unwrap().detail)),
                "which" if event.data.ui().is_some() => Value::Number(f64::from(event.data.keyboard().map_or_else(||event.data.mouse().map_or(0,|mouse|mouse.which),KeyboardData::which))),
                name if event.data.keyboard().is_some()&&keyboard_property(name) => {
                    let key=event.data.keyboard().unwrap();match name {
                        "location"=>Value::Number(f64::from(key.location)),"keyCode"=>Value::Number(f64::from(key.key_code)),"charCode"=>Value::Number(f64::from(key.char_code)),
                        "ctrlKey"=>Value::Bool(key.ctrl),"altKey"=>Value::Bool(key.alt),"shiftKey"=>Value::Bool(key.shift),"metaKey"=>Value::Bool(key.meta),
                        "repeat"=>Value::Bool(key.repeat),"isComposing"=>Value::Bool(key.composing),_=>unreachable!(),
                    }
                }
                name if event.data.mouse().is_some() && mouse_property(name) => {
                    let mouse = event.data.mouse().unwrap();
                    match name {
                        "relatedTarget" => native(mouse.related),
                        "screenX" => Value::Number(mouse.screen_x), "screenY" => Value::Number(mouse.screen_y),
                        "clientX" | "x" => Value::Number(mouse.client_x), "clientY" | "y" => Value::Number(mouse.client_y),
                        "layerX" => Value::Number(f64::from(mouse.layer_x)), "layerY" => Value::Number(f64::from(mouse.layer_y)),
                        // Constructed events never enter trusted dispatch here.
                        // CSSOM View: page = client + the realm Window's current
                        // scroll; undispatched offset coordinates alias page.
                        // https://drafts.csswg.org/cssom-view/#dom-mouseevent-pagex
                        "pageX" | "offsetX" => Value::Number(0.0+mouse.client_x),
                        "pageY" | "offsetY" => Value::Number(mouse.client_y+if event.owner==WINDOW {
                            self.state.geometry.as_ref().map_or(0.0,|geometry|geometry.css_scroll_y())
                        } else {0.0}),
                        "button" => Value::Number(f64::from(mouse.button)), "buttons" => Value::Number(f64::from(mouse.buttons)),
                        "ctrlKey" => Value::Bool(mouse.ctrl), "altKey" => Value::Bool(mouse.alt),
                        "shiftKey" => Value::Bool(mouse.shift), "metaKey" => Value::Bool(mouse.meta),
                        _ => unreachable!(),
                    }
                }
                "then" | "" => Value::Undefined,
                _ => return Err(format!("Constructed Event property {name:?} is not implemented.")),
            })
        })())
    }

    pub(super) fn set_cancel_bubble(&mut self, object: u32, value: &Value) -> Result<(), String> {
        self.state.work(self.state.event_lookup_work(8))?;
        if let Some(id) = self.created_event_index(object) {
            if value.truthy() { self.state.created_events[id].stopped = true; }
        } else {
            let event = self.state.event.as_mut().filter(|event| event.handle == object)
                .ok_or("Event handle has expired.")?;
            if value.truthy() { event.stopped = true; }
        }
        Ok(())
    }

    pub(super) fn validate_event_prefix(&self, object: u32, method: &str, index: usize, args: &[Value]) -> Result<(), (&'static str, String)> {
        if (method=="initUIEvent"&&self.ui_receiver(object)||method=="initMouseEvent"&&self.mouse_receiver(object))&&index==4 {
            self.ui_view_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(3))?;
        }
        Ok(())
    }

    pub(super) fn validate_created_event_call(&self, object: u32, method: &str, args: &[Value]) -> Result<(), (&'static str, String)> {
        if object == DOCUMENT && method == "createEvent" && self.event_factory_enabled() && interface(args.first()).is_none() {
            return Err(("NotSupportedError", unsupported_event_interface(args.first())));
        }
        if method=="initMouseEvent" && self.mouse_receiver(object) {
            self.ui_view_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(3))?;
            self.mouse_target_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(14))?;
        }
        if method=="initUIEvent" && self.ui_receiver(object) {
            self.ui_view_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(3))?;
        }
        if self.created_event_index(object).is_some() && matches!(method, "initEvent" | "initCustomEvent" | "initUIEvent" | "initMouseEvent") {
            if method == "initCustomEvent" && !self.custom_receiver(object) { return Err(("TypeError","initCustomEvent requires a CustomEvent.".into())); }
            if method=="initUIEvent" && !self.ui_receiver(object) { return Err(("TypeError","initUIEvent requires a UIEvent.".into())); }
            if method == "initMouseEvent" && !self.mouse_receiver(object) {
                return Err(("TypeError", "initMouseEvent requires a MouseEvent.".into()));
            }
            if method == "initMouseEvent" && self.mouse_receiver(object) {
                self.ui_view_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(3))?;
                self.mouse_target_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(14))?;
            }
            if !self.state.created_events[self.created_event_index(object).unwrap()].dispatching && matches!(args.first(), Some(Value::String(name)) if self.state.event_type_refused(name.len())) {
                return Err(("QuotaExceededError",format!("Event type exceeds {} UTF-8 bytes.",self.state.event_type_allowance())));
            }
        }
        Ok(())
    }

    pub(super) fn create_event(&mut self, args: &[Value]) -> Result<Value, String> {
        self.state.work(self.state.event_lookup_work(8).saturating_add(1))?;
        if let Some(Value::String(name))=args.first(){self.state.copies(name.len())?;}
        let interface=interface(args.first()).ok_or_else(||unsupported_event_interface(args.first()))?;
        if self.state.created_event_limit_reached(){return Err("Constructed Event limit reached (256 per document).".into());}
        let next=self.state.next_event.checked_add(1).filter(|next|*next<=script_class_list::BASE).ok_or("Event identity space exhausted.")?;
        let (replacement,added)=self.state.prepare_created_event_storage(0)?;
        if let Some(mut records)=replacement {records.append(&mut self.state.created_events);self.state.created_events=records;}
        let handle = self.state.next_event;
        let event = CreatedEvent { owner:WINDOW,composed:false,handle, name: String::new(), bubbles: false, cancelable: false,
            prevented: false, stopped: false, immediate: false,
            target:None,current:None,phase:0,dispatching:false,initialized:false,trusted:false,passive:false,
            timestamp: self.state.clock_ms(Instant::now()),
            data:match interface {Interface::Base|Interface::Custom=>UiRecord::Base,Interface::Ui=>UiRecord::Ui(UiData::default()),
                Interface::Mouse=>UiRecord::Mouse(UiData::default(),MouseData::default()),
                Interface::Keyboard=>UiRecord::Keyboard(UiData::default(),KeyboardData::default()),
                Interface::Focus=>UiRecord::Focus(UiData::default(),None)},
            custom: matches!(interface,Interface::Custom).then_some(Value::Null), message: false };
        self.state.created_events.push(event);
        self.state.retained+=added;
        self.state.next_event = next;
        Ok(Value::Host(handle))
    }

    pub(super) fn created_event_call(&mut self, object: u32, method: &str, args: &[Value]) -> Option<Result<Value, String>> {
        let id = self.created_event_index(object)?;
        Some((|| {
            self.state.work(self.state.event_lookup_work(8))?;
            match method {
                "initEvent" | "initCustomEvent" | "initUIEvent" | "initMouseEvent" => {
                    if method == "initMouseEvent" && self.state.created_events[id].data.mouse().is_none() {
                        return Err("initMouseEvent requires a MouseEvent.".into());
                    }
                    self.validate_created_event_call(object, method, args).map_err(|(_, message)| message)?;
                    if self.state.created_events[id].dispatching {return Ok(Value::Undefined);}
                    let name = self.text(args.first().ok_or("Event initializer requires a type.")?)?;
                    let number = |index| args.get(index).map(timer_integer).transpose().map(|n| n.unwrap_or(0));
                    let ui=if matches!(method,"initUIEvent"|"initMouseEvent") {
                        let view=self.ui_view_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(3)).map_err(|(_,message)|message)?;
                        Some(UiData {view,detail:number(4)?})
                    } else {None};
                    let mouse = if method == "initMouseEvent" {
                        let previous=self.state.created_events[id].data.mouse().unwrap();
                        Some(MouseData {
                            screen_x: f64::from(number(5)?), screen_y: f64::from(number(6)?),
                            client_x: f64::from(number(7)?), client_y: f64::from(number(8)?),
                            layer_x:previous.layer_x,layer_y:previous.layer_y,
                            ctrl: args.get(9).is_some_and(Value::truthy), alt: args.get(10).is_some_and(Value::truthy),
                            shift: args.get(11).is_some_and(Value::truthy), meta: args.get(12).is_some_and(Value::truthy),
                            button: number(13)? as i16,
                            buttons:previous.buttons,modifiers:previous.modifiers,
                            // Legacy UI Events supplemental information. A
                            // factory-created event is initially zero; the
                            // mouse initializer supplies button + 1.
                            which: (i32::from(number(13)? as i16)+1) as u32,
                            related: self.mouse_target_argument(self.event_owner(object).unwrap_or(WINDOW),args.get(14)).map_err(|(_, message)| message)?,
                        })
                    } else { None };
                    let custom = if method == "initCustomEvent" {
                        let default_detail = Value::Null;
                        let detail = args.get(3).filter(|value| !matches!(value, Value::Undefined)).unwrap_or(&default_detail);
                        let bytes = custom_bytes(Some(detail))?;
                        if bytes > self.state.limits.host.string_bytes {return Err("CustomEvent detail exceeds the host string bound.".into());}
                        let old=self.state.created_events[id].payload_bytes();
                        let estimate=name.capacity().checked_add(bytes).ok_or("CustomEvent retained text size overflow.")?;
                        self.state.copies(estimate)?;
                        let occupied=self.state.retained.checked_sub(old).ok_or("CustomEvent retention mismatch.")?;
                        if estimate>self.state.retained_limit.saturating_sub(occupied){return Err("CustomEvent retained text allowance exceeded.".into());}
                        let detail=copy_custom_value(detail)?;
                        let new=name.capacity().checked_add(owned_custom_bytes(Some(&detail))).ok_or("CustomEvent payload storage overflow.")?;
                        let retained=occupied.checked_add(new).filter(|bytes|*bytes<=self.state.retained_limit).ok_or("CustomEvent retained text allowance exceeded.")?;
                        self.state.copies(new.saturating_sub(estimate))?;
                        // All quota and fallible copying precede the one commit.
                        self.state.retained=retained;
                        Some(detail)
                    } else {
                        self.state.replace_bytes(self.state.created_events[id].name.capacity(), name.capacity())?;
                        None
                    };
                    let event = &mut self.state.created_events[id];
                    event.name = name;
                    event.initialized = true;
                    event.trusted = false;
                    event.bubbles = args.get(1).is_some_and(Value::truthy);
                    event.cancelable = args.get(2).is_some_and(Value::truthy);
                    event.prevented = false; event.stopped = false; event.immediate = false;
                    event.target=None;
                    if let Some(ui)=ui {*event.data.ui_mut().ok_or("UIEvent data is unavailable.")?=ui;}
                    if let Some(mouse) = mouse { *event.data.mouse_mut().ok_or("MouseEvent data is unavailable.")?=mouse; }
                    if let Some(custom) = custom { event.custom = Some(custom); }
                }
                "getModifierState" => {
                    let mouse=self.state.created_events[id].data.mouse().ok_or("getModifierState requires a MouseEvent.")?;
                    let Some(Value::String(name))=args.first() else {return Err("getModifierState requires a converted key.".into());};
                    let active=match name.as_str() {
                        "Alt"=>mouse.alt,"Control"=>mouse.ctrl,"Meta"=>mouse.meta,"Shift"=>mouse.shift,
                        _=>EXTRA_MODIFIERS.iter().any(|(_,key,bit)|*key==name.as_str()&&mouse.modifiers&*bit!=0),
                    };
                    self.state.work(EXTRA_MODIFIERS.len()+4)?;
                    return Ok(Value::Bool(active));
                }
                "preventDefault" => { let event = &mut self.state.created_events[id]; if event.cancelable && !event.passive { event.prevented = true; } }
                "stopPropagation" => { self.state.created_events[id].stopped = true; }
                "stopImmediatePropagation" => { let event = &mut self.state.created_events[id]; event.stopped = true; event.immediate = true; }
                _ => return Err(format!("Constructed Event method {method:?} is not implemented.")),
            }
            Ok(Value::Undefined)
        })())
    }
}

#[cfg(test)]
#[path = "script_event_own_properties_tests.rs"]
mod own_properties_tests;

#[cfg(test)]
#[path = "script_custom_event_interface_tests.rs"]
mod custom_interface_tests;

#[cfg(test)]
#[path = "script_custom_event_tests.rs"]
mod custom_tests;

#[cfg(test)]
#[path = "script_event_base_tests.rs"]
mod base_interface_tests;

#[cfg(test)]
#[path = "script_ui_event_tests.rs"]
mod ui_interface_tests;

#[cfg(test)]
#[path = "script_mouse_event_tests.rs"]
mod mouse_interface_tests;

#[cfg(test)]
#[path = "script_event_opaque_path_tests.rs"]
mod opaque_path_tests;

#[cfg(test)]
#[path="script_keyboard_event_storage_tests.rs"]
mod keyboard_storage_tests;
