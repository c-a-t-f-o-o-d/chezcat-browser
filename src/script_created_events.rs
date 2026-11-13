//! Persistent untrusted Event/MouseEvent state. These records never enter the
//! trusted dispatch slot and provide no synthetic dispatch or activation path.
use super::*;

#[cfg(test)]
const MAX_CREATED_EVENTS: usize = 256;

#[derive(Default)]
struct MouseData {
    view: Option<u32>,
    detail: i32,
    screen_x: i32,
    screen_y: i32,
    client_x: i32,
    client_y: i32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    button: i16,
    related: Option<u32>,
}

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
    fn constructed_event_factory_has_real_defaults_aliases_brands_and_no_dispatch() {
        let (doc, _, page, outcome) = start(r#"
var aliases=['Event','eVeNtS','HTMLEvents','SVGEvents','MouseEvent','mOuSeEvEnTs'],events=[];
for(var name of aliases){var e=document.createEvent(name);events.push(e);
if(e.type!==''||e.target!==null||e.currentTarget!==null||e.bubbles||e.cancelable||e.defaultPrevented||e.composed||e.isTrusted||e.eventPhase!==0||e.timeStamp<0||!Number.isInteger(e.timeStamp))throw 'base defaults';
if(e.NONE!==0||e.CAPTURING_PHASE!==1||e.AT_TARGET!==2||e.BUBBLING_PHASE!==3)throw 'phase constants';}
for(var e of [events[4],events[5]]){if(e.view!==null||e.relatedTarget!==null||e.detail!==0||e.screenX!==0||e.screenY!==0||e.clientX!==0||e.clientY!==0||e.ctrlKey||e.altKey||e.shiftKey||e.metaKey||e.button!==0||e.buttons!==0)throw 'mouse defaults';}
if('view' in events[0]||'initMouseEvent' in events[0]||!('initMouseEvent' in events[4])||events[0]===events[1])throw 'class/identity';
var calls=0,converted=document.createEvent({toString:function(){calls++;return 'MouseEvents';}});if(calls!==1||converted.isTrusted)throw 'factory conversion';
var errors=0;for(var name of ['UIEvent','KeyboardEvent','CustomEvent','Event ','unknown']){try{document.createEvent(name);}catch(e){if(e.name==='NotSupportedError')errors++;}}
try{document.createEvent();}catch(e){if(e.name==='TypeError')errors++;}try{document.createEvent(Symbol());}catch(e){if(e.name==='TypeError')errors++;}
if(errors!==7||'dispatchEvent' in document||'dispatchEvent' in window||'dispatchEvent' in events[0])throw 'false capability';
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
        assert_eq!(bridge.state.created_events[0].mouse.as_ref().unwrap().detail, 8);
        assert!(!bridge.state.created_events[0].prevented);
    }
}

pub(super) struct CreatedEvent {
    handle: u32,
    name: String,
    bubbles: bool,
    cancelable: bool,
    prevented: bool,
    stopped: bool,
    immediate: bool,
    timestamp: f64,
    mouse: Option<MouseData>,
}

fn interface(value: Option<&Value>) -> Option<bool> {
    let Some(Value::String(name)) = value else { return None; };
    if ["MouseEvent", "MouseEvents"].iter().any(|s| name.eq_ignore_ascii_case(s)) { Some(true) }
    else if ["Event", "Events", "HTMLEvents", "SVGEvents"].iter().any(|s| name.eq_ignore_ascii_case(s)) { Some(false) }
    else { None }
}

fn constant(name: &str) -> Option<u8> {
    match name { "NONE" => Some(0), "CAPTURING_PHASE" => Some(1), "AT_TARGET" => Some(2), "BUBBLING_PHASE" => Some(3), _ => None }
}

fn base_property(name: &str) -> bool {
    constant(name).is_some() || matches!(name, "type" | "target" | "currentTarget" | "bubbles" | "cancelable"
        | "defaultPrevented" | "eventPhase" | "timeStamp" | "isTrusted" | "composed")
}

fn mouse_property(name: &str) -> bool {
    matches!(name, "view" | "detail" | "screenX" | "screenY" | "clientX" | "clientY"
        | "ctrlKey" | "altKey" | "shiftKey" | "metaKey" | "button" | "buttons" | "relatedTarget")
}

impl Bridge<'_> {
    pub(super) fn event_factory_enabled(&self) -> bool {
        #[cfg(test)]
        if self.state.disable_event_factory { return false; }
        true
    }

    fn created_event_index(&self, handle: u32) -> Option<usize> {
        (handle >= EVENT_BASE).then(|| self.state.created_events.iter().position(|event| event.handle == handle)).flatten()
    }

    pub(super) fn constructed_event_receiver(&self, handle: u32) -> bool {
        self.created_event_index(handle).is_some()
    }

    pub(super) fn event_receiver(&self, handle: u32) -> bool {
        self.created_event_index(handle).is_some() || self.state.event.as_ref().is_some_and(|e| e.handle == handle)
    }

    pub(super) fn mouse_receiver(&self, handle: u32) -> bool {
        self.created_event_index(handle).is_some_and(|id| self.state.created_events[id].mouse.is_some())
    }

    pub(super) fn created_event_method(&self, object: u32, name: &str) -> Option<&'static str> {
        if !self.event_receiver(object) { return None; }
        match name {
            "initEvent" => Some("initEvent"),
            "initMouseEvent" if self.mouse_receiver(object) => Some("initMouseEvent"),
            "preventDefault" => Some("preventDefault"),
            "stopPropagation" => Some("stopPropagation"),
            "stopImmediatePropagation" => Some("stopImmediatePropagation"),
            _ => None,
        }
    }

    pub(super) fn created_event_readonly(&self, object: u32, name: &str) -> bool {
        self.created_event_index(object).is_some_and(|id| base_property(name)
            || self.state.created_events[id].mouse.is_some() && mouse_property(name))
    }

    pub(super) fn created_event_has(&mut self, object: u32, name: &str) -> Option<Result<bool, String>> {
        let id = self.created_event_index(object)?;
        Some(self.state.work(self.state.created_events.len()).map(|()| {
            base_property(name) || self.state.created_events[id].mouse.is_some() && mouse_property(name)
                || self.created_event_method(object, name).is_some()
        }))
    }

    pub(super) fn created_event_get(&mut self, object: u32, name: &str) -> Option<Result<Value, String>> {
        let id = self.created_event_index(object)?;
        Some((|| {
            self.state.work(self.state.created_events.len())?;
            if name == "type" {
                let len = self.state.created_events[id].name.len();
                self.state.copies(len)?;
                return Ok(Value::String(self.state.created_events[id].name.clone()));
            }
            let event = &self.state.created_events[id];
            let native = |handle: Option<u32>| handle.map_or(Value::Null, Value::Host);
            Ok(match name {
                "target" | "currentTarget" => Value::Null,
                "bubbles" => Value::Bool(event.bubbles),
                "cancelable" => Value::Bool(event.cancelable),
                "defaultPrevented" => Value::Bool(event.prevented),
                "eventPhase" => Value::Number(0.0),
                "timeStamp" => Value::Number(event.timestamp),
                "isTrusted" | "composed" => Value::Bool(false),
                name if constant(name).is_some() => Value::Number(f64::from(constant(name).unwrap())),
                name if event.mouse.is_some() && mouse_property(name) => {
                    let mouse = event.mouse.as_ref().unwrap();
                    match name {
                        "view" => native(mouse.view), "relatedTarget" => native(mouse.related),
                        "detail" => Value::Number(f64::from(mouse.detail)),
                        "screenX" => Value::Number(f64::from(mouse.screen_x)), "screenY" => Value::Number(f64::from(mouse.screen_y)),
                        "clientX" => Value::Number(f64::from(mouse.client_x)), "clientY" => Value::Number(f64::from(mouse.client_y)),
                        "button" => Value::Number(f64::from(mouse.button)), "buttons" => Value::Number(0.0),
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

    fn mouse_interface_argument(&self, value: Option<&Value>, window_only: bool) -> Result<Option<u32>, (&'static str, String)> {
        let Some(value) = value else { return Ok(None); };
        if matches!(value, Value::Null | Value::Undefined) { return Ok(None); }
        let Value::Host(handle) = value else { return Err(("TypeError", "MouseEvent requires a nullable native interface reference.".into())); };
        if *handle == WINDOW { return Ok(Some(*handle)); }
        if self.frame_context(*handle).is_some() {
            self.frame_window_access(Some(WINDOW), *handle).map_err(|message| ("SecurityError", message))?;
            return Ok(Some(*handle));
        }
        if !window_only && self.namespace_receiver(*handle) { return Ok(Some(*handle)); }
        Err(("TypeError", if window_only { "MouseEvent view must be a Window or null." } else { "MouseEvent relatedTarget must be an EventTarget or null." }.into()))
    }

    pub(super) fn validate_event_prefix(&self, object: u32, method: &str, index: usize, args: &[Value]) -> Result<(), (&'static str, String)> {
        if method == "initMouseEvent" && self.mouse_receiver(object) && index == 4 {
            self.mouse_interface_argument(args.get(3), true)?;
        }
        Ok(())
    }

    pub(super) fn validate_created_event_call(&self, object: u32, method: &str, args: &[Value]) -> Result<(), (&'static str, String)> {
        if object == DOCUMENT && method == "createEvent" && self.event_factory_enabled() && interface(args.first()).is_none() {
            return Err(("NotSupportedError", unsupported_event_interface(args.first())));
        }
        if self.created_event_index(object).is_some() && matches!(method, "initEvent" | "initMouseEvent") {
            if method == "initMouseEvent" && !self.mouse_receiver(object) {
                return Err(("TypeError", "initMouseEvent requires a MouseEvent.".into()));
            }
            if method == "initMouseEvent" && self.mouse_receiver(object) {
                self.mouse_interface_argument(args.get(3), true)?;
                self.mouse_interface_argument(args.get(14), false)?;
            }
            if matches!(args.first(), Some(Value::String(name)) if name.len() > self.state.limits.host.event_type_bytes) {
                return Err(("QuotaExceededError", "Event type exceeds 256 UTF-8 bytes.".into()));
            }
        }
        Ok(())
    }

    pub(super) fn create_event(&mut self, args: &[Value]) -> Result<Value, String> {
        if let Some(Value::String(name)) = args.first() { self.state.copies(name.len())?; }
        let mouse = interface(args.first()).ok_or_else(|| unsupported_event_interface(args.first()))?;
        if self.state.created_events.len() >= self.state.limits.host.created_events { return Err("Constructed Event limit reached (256 per document).".into()); }
        let next = self.state.next_event.checked_add(1).ok_or("Event identity space exhausted.")?;
        self.state.created_events.try_reserve(1).map_err(|_| "Constructed Event storage reservation failed.")?;
        let handle = self.state.next_event;
        let event = CreatedEvent { handle, name: String::new(), bubbles: false, cancelable: false,
            prevented: false, stopped: false, immediate: false,
            timestamp: self.state.clock_ms(Instant::now()),
            mouse: mouse.then(MouseData::default) };
        self.state.created_events.push(event);
        self.state.next_event = next;
        Ok(Value::Host(handle))
    }

    pub(super) fn created_event_call(&mut self, object: u32, method: &str, args: &[Value]) -> Option<Result<Value, String>> {
        let id = self.created_event_index(object)?;
        Some((|| {
            self.state.work(self.state.created_events.len())?;
            match method {
                "initEvent" | "initMouseEvent" => {
                    if method == "initMouseEvent" && self.state.created_events[id].mouse.is_none() {
                        return Err("initMouseEvent requires a MouseEvent.".into());
                    }
                    self.validate_created_event_call(object, method, args).map_err(|(_, message)| message)?;
                    let name = self.text(args.first().ok_or("Event initializer requires a type.")?)?;
                    let number = |index| args.get(index).map(timer_integer).transpose().map(|n| n.unwrap_or(0));
                    let mouse = if method == "initMouseEvent" {
                        Some(MouseData {
                            view: self.mouse_interface_argument(args.get(3), true).map_err(|(_, message)| message)?,
                            detail: number(4)?, screen_x: number(5)?, screen_y: number(6)?, client_x: number(7)?, client_y: number(8)?,
                            ctrl: args.get(9).is_some_and(Value::truthy), alt: args.get(10).is_some_and(Value::truthy),
                            shift: args.get(11).is_some_and(Value::truthy), meta: args.get(12).is_some_and(Value::truthy),
                            button: number(13)? as i16,
                            related: self.mouse_interface_argument(args.get(14), false).map_err(|(_, message)| message)?,
                        })
                    } else { None };
                    self.state.replace_bytes(self.state.created_events[id].name.len(), name.len())?;
                    let event = &mut self.state.created_events[id];
                    event.name = name;
                    event.bubbles = args.get(1).is_some_and(Value::truthy);
                    event.cancelable = args.get(2).is_some_and(Value::truthy);
                    event.prevented = false; event.stopped = false; event.immediate = false;
                    if let Some(mouse) = mouse { event.mouse = Some(mouse); }
                }
                "preventDefault" => { let event = &mut self.state.created_events[id]; if event.cancelable { event.prevented = true; } }
                "stopPropagation" => { self.state.created_events[id].stopped = true; }
                "stopImmediatePropagation" => { let event = &mut self.state.created_events[id]; event.stopped = true; event.immediate = true; }
                _ => return Err(format!("Constructed Event method {method:?} is not implemented.")),
            }
            Ok(Value::Undefined)
        })())
    }
}
