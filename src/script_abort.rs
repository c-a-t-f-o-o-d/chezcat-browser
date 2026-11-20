//! AbortController and AbortSignal native records share the normal event queue.
use super::*;
use crate::javascript::abort_interface::*;

const CONTROLLER_BASE: u32 = 0x8700_0000;
const SIGNAL_BASE: u32 = 0x8800_0000;
const ARENA: u32 = SIGNAL_BASE - CONTROLLER_BASE;

pub(super) struct Record {
    owner: u32,
    controller: bool,
    reason: Value,
}

impl Bridge<'_> {
    fn abort_record(&self, handle: u32) -> Option<(&Record, bool)> {
        let (base, signal) = if handle >= SIGNAL_BASE {
            (SIGNAL_BASE, true)
        } else {
            (CONTROLLER_BASE, false)
        };
        let index = handle.checked_sub(base).filter(|&index| index < ARENA)? as usize;
        let record = self.state.abort_signals.get(index)?;
        (signal || record.controller).then_some((record, signal))
    }

    pub(super) fn abort_instance(&self, handle: u32) -> Option<(u32, &'static str)> {
        let (record, signal) = self.abort_record(handle)?;
        Some((
            record.owner,
            if signal { SIGNAL_INTERFACE } else { CONTROLLER },
        ))
    }

    pub(super) fn abort_signal_owner(&self, handle: u32) -> Option<u32> {
        let (record, signal) = self.abort_record(handle)?;
        signal.then_some(record.owner)
    }

    pub(super) fn matches_abort_receiver(&self, receiver: u32, method: &str) -> bool {
        let Some((_, interface)) = self.abort_instance(receiver) else {
            return false;
        };
        match method {
            ABORT | SIGNAL => interface == CONTROLLER,
            ABORTED | REASON | THROW | ON_ABORT | SET_ON_ABORT => interface == SIGNAL_INTERFACE,
            _ => false,
        }
    }

    pub(super) fn create_abort_record(
        &mut self,
        owner: u32,
        controller: bool,
        reason: Value,
    ) -> Result<u32, String> {
        if !self.event_target_interface(owner) || !self.state.enabled || !self.doc.scripting_enabled
        {
            return Err("AbortSignal creation requires an active scripting owner.".into());
        }
        self.state.work(1)?;
        if self.event_target_count_refused(1) {
            return Err("Native EventTarget limit reached.".into());
        }
        let index = u32::try_from(self.state.abort_signals.len())
            .ok()
            .filter(|index| *index < ARENA)
            .ok_or("AbortSignal identity space exhausted.")?;
        let bytes=retained_storage::value_bytes(&reason);
        let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(bytes).ok_or("AbortSignal retention limit reached.")?;
        let (replacement,added)=retained_storage::grow(&self.state.abort_signals,1,available)?;
        self.state.copies(reason.bytes())?;
        if let Some(mut signals)=replacement {
            self.state.copies(self.state.abort_signals.len()*size_of::<Record>())?;
            signals.append(&mut self.state.abort_signals);self.state.abort_signals=signals;
        }
        self.state.abort_signals.push(Record{owner,controller,reason});
        self.state.retained+=bytes+added;
        Ok(index
            + if controller {
                CONTROLLER_BASE
            } else {
                SIGNAL_BASE
            })
    }

    pub(super) fn abort_call(
        &mut self,
        receiver: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if method == CONTROLLER {
            return self
                .create_abort_record(receiver, true, Value::Undefined)
                .map(Value::Host);
        }
        if !self.matches_abort_receiver(receiver, method) {
            return Err("Abort operation requires a genuine receiver.".into());
        }
        let (record, _) = self.abort_record(receiver).unwrap();
        match method {
            SIGNAL => Ok(Value::Host(receiver - CONTROLLER_BASE + SIGNAL_BASE)),
            ABORTED => Ok(Value::Bool(!matches!(record.reason, Value::Undefined))),
            REASON => {
                let bytes = record.reason.bytes();
                self.state.copies(bytes)?;
                Ok(self.abort_record(receiver).unwrap().0.reason.clone())
            }
            ON_ABORT => Ok(self.handler(receiver, "onabort")?.unwrap_or(Value::Null)),
            SET_ON_ABORT => {
                let value = args
                    .first()
                    .filter(|value| matches!(value, Value::Function(_)))
                    .cloned()
                    .unwrap_or(Value::Null);
                self.set_handler(receiver, "onabort", value)?;
                Ok(Value::Undefined)
            }
            _ => Err("Unsupported AbortSignal operation.".into()),
        }
    }

    pub(super) fn commit_abort_dispatch(
        &mut self,
        controller: u32,
        reason: Value,
    ) -> Result<usize, String> {
        let (record, is_signal) = self
            .abort_record(controller)
            .ok_or("Unknown AbortController.")?;
        if is_signal
            || !matches!(record.reason, Value::Undefined)
            || matches!(reason, Value::Undefined)
        {
            return Err("Invalid AbortController cancellation transition.".into());
        }
        let owner = record.owner;
        let signal = controller - CONTROLLER_BASE + SIGNAL_BASE;
        let bytes=retained_storage::value_bytes(&reason);
        let retained=self.state.retained.checked_add(bytes).filter(|bytes|*bytes<=self.state.retained_limit)
            .ok_or("AbortSignal reason retention limit reached.")?;
        self.state.copies(reason.bytes())?;
        // Hold the reason across Event construction and dispatch preparation;
        // those reservations must not spend the same remaining allowance.
        self.state.retained=retained;
        let result=(|| {
            let Value::Host(event)=self.construct_base_event(owner,&[Value::String("abort".into())],None)? else {unreachable!()};
            let token=match self.begin_synthetic_dispatch(signal,&[Value::Host(event)]) {
                Ok(token)=>token,
                Err(error)=>{self.rollback_base_event(event)?;return Err(error);}
            };
            self.state.abort_signals[(controller-CONTROLLER_BASE) as usize].reason=reason;
            let entry=self.created_event_index(event).unwrap();
            self.state.created_events[entry].trusted=true;
            Ok(token)
        })();
        if result.is_err(){self.state.retained-=bytes;}
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(source: &str) -> (Document, Forms, Page, Outcome) {
        let mut doc = crate::html::parse("<!doctype html><title>Cancellation fixture</title>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let outcome = page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                label: "Original cancellation fixture".into(),
                source: source.into(),
            }],
        );
        (doc, forms, page, outcome)
    }

    #[test]
    fn script_abort_uses_real_brands_and_dispatches_synchronously_exactly_once() {
        let (doc, _, page, outcome) = start(
            r#"
var c=new AbortController(), s=c.signal, reason={}, events='', received, before;
if(s!==c.signal || s.aborted || s.reason!==undefined || s.throwIfAborted()!==undefined ||
   Object.getPrototypeOf(s)!==AbortSignal.prototype ||
   Object.getPrototypeOf(AbortSignal.prototype)!==EventTarget.prototype ||
   Object.getPrototypeOf(AbortSignal)!==EventTarget ||
   !(s instanceof AbortSignal) || !(s instanceof EventTarget) ||
   !(c instanceof AbortController) || c instanceof EventTarget ||
   Object.prototype.toString.call(s)!=='[object AbortSignal]')throw 'native interfaces';
s.addEventListener('abort',function(e){
    if(this!==s || e.target!==s || e.currentTarget!==s || e.type!=='abort' ||
       e.bubbles || e.cancelable || !e.isTrusted || !s.aborted || s.reason!==reason)
        throw 'abort event';
    received=e;
    events+='listener|';
    c.abort('nested');
},{once:true});
s.onabort=function(e){if(this!==s || e!==received)throw 'handler receiver'; events+='handler|';};
c.abort(reason);
before=events;
c.abort('replacement');
if(before!=='listener|handler|' || events!==before || s.reason!==reason ||
   !received.isTrusted || received.currentTarget!==null)throw 'synchronous first abort';
try{s.throwIfAborted();throw 'missing reason';}catch(e){if(e!==reason)throw e;}
s.onabort=null;
s.dispatchEvent(received);
if(received.isTrusted || events!==before)throw 'author redispatch resets trust';
var defaulted=new AbortController();defaulted.abort(undefined);
if(!(defaulted.signal.reason instanceof DOMException) ||
   defaulted.signal.reason.name!=='AbortError' || defaulted.signal.reason.code!==20)
   throw 'DOMException reason';
var pre=AbortSignal.abort(null), another=AbortSignal.abort();
if(pre===another || !pre.aborted || pre.reason!==null || !(another.reason instanceof DOMException))
   throw 'static signals';
var bad=0;
for(var attempt of [function(){AbortController();},function(){AbortSignal();},function(){new AbortSignal();},
  function(){AbortController.prototype.abort.call({});},
  function(){Object.getOwnPropertyDescriptor(AbortSignal.prototype,'reason').get.call(c);},
  function(){c.addEventListener('abort',function(){});}]){
  try{attempt();}catch(e){if(e.name==='TypeError')bad++;else throw e;}
}
if(bad!==6)throw 'receiver rejection '+bad;
document.title='abort-controller-ok';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "abort-controller-ok");
    }

    #[test]
    fn script_abort_reason_and_event_admission_fail_without_committing_the_signal() {
        let (mut doc, mut forms, mut page, outcome) = start(
            r#"
var c=new AbortController(), s=c.signal, calls=0;
s.addEventListener('abort',function(){calls++;});
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        let limit = page.state.limits.host.created_events;
        let retained = page.state.retained;
        page.state.limits.host.created_events = page.state.created_events.len();
        let outcome = page.console_eval(&mut doc, &mut forms, "c.abort('reason');");
        assert_eq!(outcome.errors, 1);
        assert_eq!(page.state.retained, retained);
        assert!(page.state.synthetic_dispatches.is_empty());
        page.state.limits.host.created_events = limit;
        let outcome = page.console_eval(
            &mut doc,
            &mut forms,
            r#"
if(s.aborted||s.reason!==undefined||calls)throw 'partial transition';
c.abort('recovered');
if(!s.aborted||s.reason!=='recovered'||calls!==1)throw 'retry';
document.title='abort-recovered';
"#,
        );
        assert_eq!(outcome.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "abort-recovered");
    }
}
