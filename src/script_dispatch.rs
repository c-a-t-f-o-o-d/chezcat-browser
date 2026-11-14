//! Synchronous, untrusted Event/UIEvent delivery through the current JS job.
//! No activation, native input, network access or microtask checkpoint is added.
use super::*;

pub(super) struct Dispatch {
    event: usize,
    name: String,
    visits: Vec<(u32, bool, u8)>,
    visit: usize,
    snapshot: Vec<u32>,
    listener: usize,
    property: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn start(source: &str) -> (Document, Forms, Page, Outcome) {
        let mut doc = crate::html::parse(
            "<html><body><div id=p><button id=b type=button>Click</button></div></body></html>",
        );
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let outcome = page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                label: "Original synchronous dispatch fixture".into(),
                source: source.into(),
            }],
        );
        (doc, forms, page, outcome)
    }
    #[test]
    fn synchronous_dispatch_real_identity_order_cancel_and_persistent_fields() {
        let (doc, _, page, outcome) = start(
            r#"
var b=document.getElementById('b'),p=document.getElementById('p'),e=document.createEvent('Event'),log='',stamp=e.timeStamp;
e.initEvent('probe',true,true);
function record(mark,target,phase){return function(x){if(x!==e||x.target!==b||x.currentTarget!==target||this!==target||x.eventPhase!==phase||x.isTrusted)throw 'identity/phase/trust';log+=mark;};}
window.addEventListener('probe',record('w',window,1),true);document.addEventListener('probe',record('d',document,1),true);p.addEventListener('probe',record('p',p,1),true);b.addEventListener('probe',record('c',b,2),true);b.addEventListener('probe',record('t',b,2));p.addEventListener('probe',record('P',p,3));document.addEventListener('probe',record('D',document,3));window.addEventListener('probe',record('W',window,3));
b.addEventListener('probe',function(x){x.preventDefault();x.initEvent('ignored',false,false);if(x.type!=='probe'||!x.defaultPrevented)throw 'dispatch initializer';});
if(b.dispatchEvent(e)!==false||log!=='wdpctPDW'||e.target!==b||e.currentTarget!==null||e.eventPhase!==0||e.cancelBubble||e.timeStamp!==stamp||!e.defaultPrevented)throw 'completed state';
e.initEvent('next',false,false);if(e.target!==null||e.defaultPrevented||!b.dispatchEvent(e)||e.target!==b)throw 'reuse';document.title='synchronous-order-ok';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "synchronous-order-ok");
        assert!(page.state.synthetic_dispatches.is_empty());
    }
    #[test]
    fn synchronous_dispatch_snapshots_once_passive_removal_and_property_cancellation() {
        let (doc, _, page, outcome) = start(
            r#"
var b=document.getElementById('b'),e=document.createEvent('Event'),log='';e.initEvent('x',false,true);
function removed(){throw 'removed listener';}function later(){log+='C';}
b.addEventListener('x',function(x){log+='A';b.removeEventListener('x',removed);b.addEventListener('x',later);x.preventDefault();},{once:true,passive:true});b.addEventListener('x',removed);b.addEventListener('x',function(){log+='D';});
if(!b.dispatchEvent(e)||e.defaultPrevented||log!=='AD')throw 'first snapshot/passive';if(!b.dispatchEvent(e)||log!=='ADDC')throw 'once/later snapshot';
e.initEvent('click',true,true);b.onclick=function(){return false;};if(b.dispatchEvent(e)!==false||!e.defaultPrevented)throw 'on property cancellation';
document.title='snapshot-ok';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "snapshot-ok");
    }
    #[test]
    fn synchronous_dispatch_nested_invalid_reentry_detached_paths_and_stopping() {
        let (doc, _, page, outcome) = start(
            r#"
var p=document.createElement('div'),b=document.createElement('span');p.appendChild(b);var outer=document.createEvent('Event'),inner=document.createEvent('UIEvent'),log='';outer.initEvent('outer',true,true);inner.initUIEvent('inner',true,true,window,7);
document.addEventListener('outer',function(){throw 'detached reached document';});p.addEventListener('inner',function(e){if(e!==inner||e.detail!==7||e.target!==p)throw 'nested UI';log+='I';});
b.addEventListener('outer',function(e){var failed=false;try{b.dispatchEvent(e);}catch(x){failed=x.name==='InvalidStateError';}if(!failed)throw 'reentry';p.dispatchEvent(inner);if(e.target!==b||e.currentTarget!==b||e.eventPhase!==2||e.isTrusted)throw 'outer context';log+='A';e.stopPropagation();});b.addEventListener('outer',function(){log+='B';});p.addEventListener('outer',function(){throw 'stopped ancestor';});
if(!b.dispatchEvent(outer)||log!=='IAB'||outer.cancelBubble||inner.currentTarget!==null)throw 'nested/detached';
outer.initEvent('immediate',false,false);b.addEventListener('immediate',function(e){log+='X';e.stopImmediatePropagation();});b.addEventListener('immediate',function(){throw 'immediate';});b.dispatchEvent(outer);if(log!=='IABX')throw 'immediate stop';document.title='nested-ok';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "nested-ok");
    }
    #[test]
    fn synchronous_dispatch_brand_empty_mouse_boundary_borrowing_and_no_coercion() {
        let (doc, _, page, outcome) = start(
            r#"
var b=document.getElementById('b'),e=document.createEvent('Event'),mouse=document.createEvent('MouseEvent'),conversions=0,count=0,invalid=0;
for(var bad of [undefined,null,{},7,{toString:function(){conversions++;return 'x';}}]){try{b.dispatchEvent(bad);}catch(x){if(x.name==='TypeError')count++;}}
try{b.dispatchEvent();}catch(x){if(x.name==='TypeError')count++;}try{b.dispatchEvent(e);}catch(x){if(x.name==='InvalidStateError')invalid++;}
mouse.initMouseEvent('click',true,true);var fired=0;b.onclick=function(){fired++;};try{b.dispatchEvent(mouse);}catch(x){if(x.name==='NotSupportedError')invalid++;}
e.initEvent('borrowed');if(count!==6||invalid!==2||conversions||fired||e.target!==null)throw 'preflight';if(!b.dispatchEvent.call(document,e)||e.target!==document)throw 'Document borrowing';if(!document.dispatchEvent.call(window,e)||e.target!==window)throw 'Window borrowing';if(b.dispatchEvent.length!==1||b.dispatchEvent!==b.dispatchEvent)throw 'method identity';document.title='preflight-ok';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "preflight-ok");
    }
    #[test]
    fn synchronous_dispatch_exceptions_report_but_do_not_escape_and_promises_wait() {
        let (mut doc, mut forms, mut page, outcome) = start(
            r#"
var b=document.getElementById('b'),e=document.createEvent('Event'),log='';e.initEvent('x');b.addEventListener('x',function(){log+='a';Promise.resolve().then(function(){log+='p';});throw 'original listener error';});b.addEventListener('x',function(){log+='b';});
if(!b.dispatchEvent(e)||log!=='ab')throw 'listener escape/checkpoint';log+='s';document.title='caller-completed';
"#,
        );
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.title, "caller-completed");
        assert!(page.report().contains("Synthetic event listener"));
        let outcome=page.console_eval(&mut doc,&mut forms,"if(log!=='absp'||e.currentTarget!==null||e.eventPhase!==0)throw 'microtask ordering/cleanup';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
    #[test]
    fn synchronous_dispatch_fatal_budget_failure_cleans_up_and_valid_later_job_recovers() {
        let(mut doc,mut forms,mut page,outcome)=start("var b=document.getElementById('b'),e=document.createEvent('Event');e.initEvent('x');function heavy(){while(true){}}b.addEventListener('x',heavy);");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let outcome = page.console_eval(&mut doc, &mut forms, "b.dispatchEvent(e);");
        assert_eq!(outcome.errors, 1);
        assert!(page.state.synthetic_dispatches.is_empty());
        let event = &page.state.created_events[0];
        assert!(
            !event.dispatching && event.current.is_none() && event.phase == 0 && !event.passive
        );
        let outcome=page.console_eval(&mut doc,&mut forms,"b.removeEventListener('x',heavy);if(!b.dispatchEvent(e))throw 'recovery';document.title='dispatch-recovered';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "dispatch-recovered");
    }
    #[test]
    fn synchronous_dispatch_freezes_path_but_observes_listener_removal_and_addition() {
        let (doc, _, page, outcome) = start(
            r#"
var p=document.createElement('div'),b=document.createElement('span');document.body.appendChild(p);p.appendChild(b);var e=document.createEvent('Event'),log='';e.initEvent('frozen',true,false);
p.addEventListener('frozen',function(){p.removeChild(b);log+='C';b.addEventListener('frozen',function(){log+='T';},{once:true});},true);p.addEventListener('frozen',function(){log+='P';});document.addEventListener('frozen',function(){log+='D';});b.dispatchEvent(e);
if(log!=='CTPD'||b.parentNode!==null||e.target!==b)throw 'path was not frozen';document.title='frozen-path-ok';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "frozen-path-ok");
    }
    #[test]
    fn synchronous_dispatch_nested_inside_native_input_preserves_trusted_cancellation() {
        let (mut doc, mut forms, mut page, outcome) = start(
            r#"
var b=document.getElementById('b'),inner=document.createEvent('Event');inner.initEvent('inside',true,true);
document.addEventListener('inside',function(e){if(e!==inner||e.isTrusted)throw 'inner trust';e.preventDefault();});
b.addEventListener('keydown',function(e){var phase=e.eventPhase,stamp=e.timeStamp;if(!e.isTrusted||b.dispatchEvent(inner)!==false||e.eventPhase!==phase||e.timeStamp!==stamp||e.target!==b||e.currentTarget!==b||e.defaultPrevented)throw 'native slot changed';e.preventDefault();document.title='native-preserved';});
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let node = (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some("b"))
            .unwrap();
        page.begin_job();
        let outcome = page.keyboard(
            &mut doc,
            &mut forms,
            KeyboardInput {
                down: true,
                target: Some(node),
                key: "a".into(),
                code: "KeyA".into(),
                key_code: 65,
                ..KeyboardInput::default()
            },
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert!(outcome.prevented);
        assert_eq!(doc.title, "native-preserved");
    }
    #[test]
    fn synchronous_dispatch_host_budget_failure_cleans_flags_and_initializer_converts_before_noop()
    {
        let (mut doc, mut forms, mut page, outcome) = start(
            r#"
var b=document.getElementById('b'),e=document.createEvent('UIEvent'),log='';e.initUIEvent('x',true,true,window,3);b.addEventListener('x',function(event){event.initUIEvent({toString:function(){log+='T';return 'ignored';}},false,false,null,{valueOf:function(){log+='N';return 9;}});if(event.type!=='x'||event.detail!==3||event.view!==window||log!=='TN')throw 'initializer no-op conversion';});b.dispatchEvent(e);
var budgetEvent=document.createEvent('Event');budgetEvent.initEvent('budget');b.addEventListener('budget',function(event){for(var i=0;i<1000;i++){var sample=event.defaultPrevented;}});
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let old = page.state.limits.host_work;
        page.state.limits.host_work = 128;
        let outcome = page.console_eval(&mut doc, &mut forms, "b.dispatchEvent(budgetEvent);");
        assert_eq!(outcome.errors, 1);
        assert!(page.state.synthetic_dispatches.is_empty());
        assert!(!page.state.created_events[1].dispatching);
        assert!(
            page.state.created_events[1].target.is_some(),
            "failure must occur during listener work"
        );
        page.state.limits.host_work = old;
        let outcome = page.console_eval(
            &mut doc,
            &mut forms,
            "e.initEvent('recovery');if(!b.dispatchEvent(e))throw 'host recovery';",
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
    #[test]
    fn synchronous_dispatch_nested_calls_refuse_at_runtime_guard_on_native_stack() {
        // Explicit measured production reserve for nested interpreter/host
        // callbacks. The old 1 MiB witness overflow is retained as evidence.
        std::thread::Builder::new().stack_size(8*1024*1024).spawn(|| {
            let(mut doc,mut forms,mut page,outcome)=start("var b=document.getElementById('b'),first=document.createEvent('Event');first.initEvent('recursive');function nested(){var next=document.createEvent('Event');next.initEvent('recursive');b.dispatchEvent(next);}b.addEventListener('recursive',nested);");assert_eq!(outcome.errors,0,"{}",page.report());
            let outcome=page.console_eval(&mut doc,&mut forms,"b.dispatchEvent(first);");assert_eq!(outcome.errors,1,"{}",page.report());assert!(page.report().contains("call limit")||page.report().contains("evaluation nesting"),"{}",page.report());assert!(page.state.synthetic_dispatches.is_empty());assert!(page.state.created_events.iter().all(|event|!event.dispatching&&event.current.is_none()&&event.phase==0));
            let outcome=page.console_eval(&mut doc,&mut forms,"b.removeEventListener('recursive',nested);if(!b.dispatchEvent(first))throw 'nested recovery';");assert_eq!(outcome.errors,0,"{}",page.report());
        }).unwrap().join().unwrap();
    }
    #[test]
    fn synchronous_dispatch_event_validation_scan_is_charged_before_publication() {
        let(mut doc,mut forms,mut page,outcome)=start("var b=document.getElementById('b');for(var i=0;i<32;i++){document.createEvent('Event');}var last=document.createEvent('Event');last.initEvent('small-profile');");
        assert_eq!(outcome.errors, 0, "{}", page.report());
        page.state.limits.host_work = 100;
        let outcome = page.console_eval(&mut doc, &mut forms, "b.dispatchEvent(last);");
        assert_eq!(outcome.errors, 1);
        assert!(page.state.synthetic_dispatches.is_empty());
        assert!(page.state.created_events[32].target.is_none());
        assert!(!page.state.created_events[32].dispatching);
    }
}

impl Bridge<'_> {
    pub(super) fn dispatch_receiver(&self, object: u32) -> bool {
        matches!(object, WINDOW | DOCUMENT) || self.attr_element_receiver(object)
    }
    pub(super) fn validate_dispatch(
        &self,
        object: u32,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if !self.dispatch_receiver(object) {
            return Err((
                "TypeError",
                "dispatchEvent requires a supported EventTarget.".into(),
            ));
        }
        let Some(Value::Host(handle)) = args.first() else {
            return Err((
                "TypeError",
                "dispatchEvent requires a genuine Event.".into(),
            ));
        };
        let Some(id) = self.created_event_index(*handle) else {
            if self
                .state
                .event
                .as_ref()
                .is_some_and(|event| event.handle == *handle)
            {
                return Err((
                    "InvalidStateError",
                    "An event already being dispatched cannot be redispatched.".into(),
                ));
            }
            if (EVENT_BASE..self.state.next_event).contains(handle) {
                return Err((
                    "NotSupportedError",
                    "Expired native input events cannot be redispatched in this subset.".into(),
                ));
            }
            return Err((
                "TypeError",
                "dispatchEvent requires a genuine Event.".into(),
            ));
        };
        let event = &self.state.created_events[id];
        if event.dispatching || event.name.is_empty() {
            return Err((
                "InvalidStateError",
                "Event type is empty/uninitialized or the event is already being dispatched."
                    .into(),
            ));
        }
        // A MouseEvent click requires HTML activation/rollback; other dispatched
        // mouse coordinates also need live target-relative geometry. Do not
        // fabricate either behavior. Factory/initializer support is unchanged.
        if self.mouse_receiver(*handle) {
            return Err(("NotSupportedError","MouseEvent dispatch/activation is not implemented; base Event/UIEvent dispatch is supported.".into()));
        }
        Ok(())
    }
    pub(super) fn begin_synthetic_dispatch(
        &mut self,
        object: u32,
        args: &[Value],
    ) -> Result<usize, String> {
        self.validate_dispatch(object, args)
            .map_err(|(_, message)| message)?;
        self.state.rebuild(self.doc)?;
        let Value::Host(handle) = args[0] else {
            unreachable!()
        };
        let id = self.created_event_index(handle).unwrap();
        self.state
            .copies(self.state.created_events[id].name.len())?;
        let name = self.state.created_events[id].name.clone();
        let bubbles = self.state.created_events[id].bubbles;
        let mut path = Vec::new();
        path.try_reserve(2)
            .map_err(|_| "Could not reserve the event path.")?;
        path.push(object);
        if object == DOCUMENT {
            if name != "load" {
                path.push(WINDOW);
            }
        } else if object != WINDOW {
            let node = self.node(object)?;
            let mut parent = self.state.tree.parents[node];
            while let Some(node) = parent {
                self.state.work(1)?;
                if node != 0 {
                    path.try_reserve(1)
                        .map_err(|_| "Could not grow the event path.")?;
                    path.push(node_handle(node));
                }
                parent = self.state.tree.parents[node];
            }
            if self.state.dom.connected.get(node).copied().unwrap_or(false) {
                path.try_reserve(2)
                    .map_err(|_| "Could not finish the event path.")?;
                path.push(DOCUMENT);
                if name != "load" {
                    path.push(WINDOW);
                }
            }
        }
        let mut visits = Vec::new();
        visits
            .try_reserve(
                path.len()
                    .checked_mul(2)
                    .ok_or("Event path length overflow.")?,
            )
            .map_err(|_| "Could not reserve event propagation phases.")?;
        visits.extend(path.iter().skip(1).rev().map(|&target| (target, true, 1)));
        visits.extend([(object, true, 2), (object, false, 2)]);
        if bubbles {
            visits.extend(path.iter().skip(1).map(|&target| (target, false, 3)));
        }
        self.state
            .synthetic_dispatches
            .try_reserve(1)
            .map_err(|_| "Could not grow the nested event dispatch stack.")?;
        let token = self.state.synthetic_dispatches.len();
        self.state.synthetic_dispatches.push(Dispatch {
            event: id,
            name,
            visits,
            visit: 0,
            snapshot: Vec::new(),
            listener: 0,
            property: false,
        });
        let event = &mut self.state.created_events[id];
        event.dispatching = true;
        event.target = Some(object);
        event.current = None;
        event.phase = 0;
        event.passive = false;
        Ok(token)
    }
    pub(super) fn next_synthetic_callback(
        &mut self,
        token: usize,
    ) -> Result<Option<crate::javascript::EventCallback>, String> {
        if token.checked_add(1) != Some(self.state.synthetic_dispatches.len()) {
            return Err("Event dispatch stack mismatch.".into());
        }
        loop {
            self.state.work(
                self.state
                    .listeners
                    .len()
                    .saturating_mul(2)
                    .saturating_add(1),
            )?;
            let frame = &mut self.state.synthetic_dispatches[token];
            let event = &mut self.state.created_events[frame.event];
            event.passive = false;
            if event.immediate {
                return Ok(None);
            }
            if let Some(&serial) = frame.snapshot.get(frame.listener) {
                frame.listener += 1;
                let Some((function, target, options, property)) = self
                    .state
                    .listeners
                    .iter()
                    .find(|listener| listener.serial == serial)
                    .map(|listener| {
                        (
                            listener.function.clone(),
                            listener.target,
                            listener.options,
                            listener.property,
                        )
                    })
                else {
                    continue;
                };
                if options.once {
                    self.state
                        .listeners
                        .retain(|candidate| candidate.serial != serial);
                }
                frame.property = property;
                event.passive = options.passive;
                return Ok(Some(crate::javascript::EventCallback {
                    function,
                    target,
                    event: event.handle,
                }));
            }
            if event.stopped {
                return Ok(None);
            }
            let Some(&(target, capture, phase)) = frame.visits.get(frame.visit) else {
                return Ok(None);
            };
            frame.visit += 1;
            event.current = Some(target);
            event.phase = phase;
            frame.snapshot.clear();
            frame.listener = 0;
            self.state.work(self.state.listeners.len())?;
            let frame = &mut self.state.synthetic_dispatches[token];
            for listener in &self.state.listeners {
                if listener.target == target
                    && listener.event == frame.name
                    && listener.options.capture == capture
                {
                    frame
                        .snapshot
                        .try_reserve(1)
                        .map_err(|_| "Could not grow the event listener snapshot.")?;
                    frame.snapshot.push(listener.serial);
                }
            }
        }
    }
    pub(super) fn synthetic_callback_result(&mut self, token: usize, returned_false: bool) {
        if let Some(frame) = self.state.synthetic_dispatches.get(token) {
            let event = &mut self.state.created_events[frame.event];
            if returned_false && frame.property && event.cancelable && !event.passive {
                event.prevented = true;
            }
        }
    }
    pub(super) fn synthetic_callback_error(&mut self, token: usize, message: &str) {
        if self.state.synthetic_dispatches.get(token).is_some() {
            self.state.error("Synthetic event listener", message);
        }
    }
    pub(super) fn finish_synthetic_dispatch(&mut self, token: usize) -> bool {
        if token.checked_add(1) != Some(self.state.synthetic_dispatches.len()) {
            return false;
        }
        let frame = self.state.synthetic_dispatches.pop().unwrap();
        let event = &mut self.state.created_events[frame.event];
        event.current = None;
        event.phase = 0;
        event.dispatching = false;
        event.passive = false;
        event.stopped = false;
        event.immediate = false;
        !event.prevented
    }
}
