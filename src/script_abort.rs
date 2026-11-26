//! AbortController and AbortSignal native records share the normal event queue.
use super::*;
use crate::javascript::abort_interface::*;

const CONTROLLER_BASE: u32 = 0x8700_0000;
const SIGNAL_BASE: u32 = 0x8800_0000;
const ARENA: u32 = SIGNAL_BASE - CONTROLLER_BASE;

struct Timeout {due:Option<Instant>}

pub(super) struct Record {
    owner: u32,
    controller: bool,
    reason: Value,
    timeout: Option<Timeout>,
    dependent:bool,
    sources:Vec<u32>,
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

    pub(super) fn listener_signal_aborted(&self,signal:u32)->bool {
        self.abort_record(signal).is_some_and(|(record,is_signal)|is_signal&&!matches!(record.reason,Value::Undefined))
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
        self.state.abort_signals.push(Record{owner,controller,reason,timeout:None,dependent:false,sources:Vec::new()});
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


    pub(super) fn validate_abort_signal_argument(&mut self,value:&Value)->Result<u32,ArgumentError>{
        let Value::Host(signal)=value else{return Err(ArgumentError::TypeError("A genuine AbortSignal is required.".into()));};
        let owner=self.abort_signal_owner(*signal).ok_or_else(||ArgumentError::TypeError("A genuine AbortSignal is required.".into()))?;
        self.window_relationship_access(owner).map_err(ArgumentError::Fatal)?;
        Ok(*signal)
    }
    pub(super) fn create_dependent_signal(&mut self,owner:u32,signals:&[u32])->Result<u32,String>{
        self.window_relationship_access(owner)?;
        if owner!=WINDOW{self.contains_child_document(owner)?;}
        self.state.work(signals.len().saturating_add(1))?;
        for &signal in signals{self.validate_abort_signal_argument(&Value::Host(signal)).map_err(|error|match error{ArgumentError::TypeError(message)|ArgumentError::Fatal(message)=>message})?;}
        if let Some(signal)=signals.iter().copied().find(|signal|self.listener_signal_aborted(*signal)){
            let bytes=self.abort_record(signal).unwrap().0.reason.bytes();self.state.copies(bytes)?;
            let reason=retained_storage::copy_value(&self.abort_record(signal).unwrap().0.reason)?;
            return self.create_abort_record(owner,false,reason);
        }
        let mut roots=Vec::new();
        for &signal in signals{
            let dependent=self.abort_record(signal).unwrap().0.dependent;
            let count=if dependent{self.abort_record(signal).unwrap().0.sources.len()}else{1};
            for index in 0..count{
                let root=if dependent{self.abort_record(signal).unwrap().0.sources[index]}else{signal};
                self.state.work(roots.len().saturating_add(1))?;
                if !roots.contains(&root){self.state.push_dispatch_temporary(&mut roots,root,0)?;}
            }
        }
        let held=roots.capacity().checked_mul(size_of::<u32>()).ok_or("Abort dependency metadata overflow.")?;
        self.state.retained=self.state.retained.checked_add(held).filter(|bytes|*bytes<=self.state.retained_limit).ok_or("Abort dependency storage allowance exhausted.")?;
        let signal=match self.create_abort_record(owner,false,Value::Undefined){Ok(signal)=>signal,Err(error)=>{self.state.retained-=held;return Err(error);}};
        let record=&mut self.state.abort_signals[(signal-SIGNAL_BASE)as usize];record.dependent=true;record.sources=roots;
        Ok(signal)
    }
    pub(super) fn remove_aborted_signal_listeners(&mut self,signal:u32){
        self.state.retained-=self.state.listeners.retain(|listener|listener.options.signal!=Some(signal));
    }
    pub(super) fn commit_abort_dispatch(&mut self,controller:u32,reason:Value)->Result<Vec<Dispatch>,String>{
        let (record,is_signal)=self.abort_record(controller).ok_or("Unknown AbortController.")?;
        if is_signal||!matches!(record.reason,Value::Undefined){return Err("Invalid AbortController cancellation transition.".into());}
        self.commit_signal_abort_dispatch(controller-CONTROLLER_BASE+SIGNAL_BASE,reason)
    }
    fn commit_signal_abort_dispatch(&mut self,signal:u32,reason:Value)->Result<Vec<Dispatch>,String>{
        let (record,is_signal)=self.abort_record(signal).ok_or("Unknown native AbortSignal.")?;
        if !is_signal||!matches!(record.reason,Value::Undefined)||matches!(reason,Value::Undefined){return Err("Invalid native AbortSignal cancellation transition.".into());}
        self.state.work(self.state.abort_signals.len())?;
        let scan=self.state.abort_signals.iter().try_fold(1usize,|sum,record|sum.checked_add(record.sources.len().saturating_add(1))).ok_or("Abort dependency scan overflow.")?;
        self.state.work(scan.checked_mul(2).ok_or("Abort dependency scan overflow.")?)?;
        let count=self.state.abort_signals.iter().filter(|record|matches!(record.reason,Value::Undefined)&&record.sources.contains(&signal)).count().checked_add(1).ok_or("Abort batch size overflow.")?;
        // Every removal scan is admitted before any reason or listener changes.
        self.state.work(self.state.listeners.len().checked_mul(count.checked_mul(2).ok_or("Abort scan overflow.")?).ok_or("Abort scan overflow.")?)?;
        let mut steps=Vec::new();self.state.reserve_dispatch_temporary(&mut steps,count,0)?;
        let metadata=steps.capacity().checked_mul(size_of::<Dispatch>()).ok_or("Abort batch metadata overflow.")?;
        self.state.copies(count.checked_mul(size_of::<Dispatch>()).ok_or("Abort batch copy overflow.")?)?;
        steps.push(Dispatch{token:None,signal,event:0,reason:Value::Undefined});
        for (index,record) in self.state.abort_signals.iter().enumerate(){
            if matches!(record.reason,Value::Undefined)&&record.sources.contains(&signal){steps.push(Dispatch{token:None,signal:SIGNAL_BASE+index as u32,event:0,reason:Value::Undefined});}
        }
        self.state.retained+=metadata;
        let mut reason_bytes=0usize;
        let result=(||->Result<(),String>{
            for step in &mut steps{
                let owner=self.abort_record(step.signal).unwrap().0.owner;
                self.window_relationship_access(owner)?;
                if owner!=WINDOW{self.contains_child_document(owner)?;}
                if retained_storage::value_bytes(&reason)>self.state.retained_limit.saturating_sub(self.state.retained){return Err("AbortSignal reason retention limit reached.".into());}
                self.state.copies(reason.bytes())?;
                let copied=retained_storage::copy_value(&reason)?;let bytes=retained_storage::value_bytes(&copied);
                let retained=self.state.retained.checked_add(bytes).filter(|bytes|*bytes<=self.state.retained_limit).ok_or("AbortSignal reason retention limit reached.")?;
                reason_bytes=reason_bytes.checked_add(bytes).ok_or("Abort reason size overflow.")?;
                self.state.retained=retained;step.reason=copied;
            }
            // Prepare in reverse so the original source is at the top of the
            // checked dispatch stack, followed by dependents in creation order.
            for step in steps.iter_mut().rev(){
                let owner=self.abort_record(step.signal).unwrap().0.owner;
                let Value::Host(event)=self.construct_base_event(owner,&[Value::String("abort".into())],None)? else{unreachable!()};
                step.event=event;
                step.token=Some(self.begin_synthetic_dispatch(step.signal,&[Value::Host(event)])?);
            }
            Ok(())
        })();
        if let Err(error)=result{
            for step in &steps{
                if let Some(token)=step.token{self.finish_synthetic_dispatch(token);}
                if step.event!=0{
                    let index=self.created_event_index(step.event).unwrap();
                    self.state.created_events[index].target=None;
                    self.rollback_base_event(step.event)?;
                }
            }
            self.state.retained-=metadata+reason_bytes;
            return Err(error);
        }
        for step in &mut steps{
            let record=&mut self.state.abort_signals[(step.signal-SIGNAL_BASE)as usize];
            record.reason=std::mem::replace(&mut step.reason,Value::Undefined);
            self.state.retained-=record.sources.capacity()*size_of::<u32>();record.sources=Vec::new();
            let index=self.created_event_index(step.event).unwrap();self.state.created_events[index].trusted=true;
        }
        Ok(steps)
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

impl Bridge<'_>{
    pub(super) fn create_timeout_signal(&mut self,owner:u32,milliseconds:u64)->Result<u32,String>{
        self.window_relationship_access(owner)?;
        if owner!=WINDOW{self.contains_child_document(owner)?;}
        let signal=self.create_abort_record(owner,false,Value::Undefined)?;
        // A valid IDL duration beyond the platform clock remains pending until
        // this native document lifetime ends; it must not wrap or fire early.
        let due=Instant::now().checked_add(std::time::Duration::from_millis(milliseconds));
        self.state.abort_signals[(signal-SIGNAL_BASE)as usize].timeout=Some(Timeout{due});
        if let Some(due)=due{self.state.abort_timeout_deadline=Some(self.state.abort_timeout_deadline.map_or(due,|old|old.min(due)));}
        Ok(signal)
    }
}
impl State{
    fn refresh_abort_timeout_deadline(&mut self){
        self.abort_timeout_deadline=self.abort_signals.iter().filter_map(|record|record.timeout.as_ref().and_then(|timeout|timeout.due)).min();
    }
    pub(super) fn clear_abort_timeouts(&mut self){
        for record in &mut self.abort_signals{record.timeout=None;}
        self.abort_timeout_deadline=None;
    }
}
impl Page{
    #[inline(never)]
    pub(super) fn poll_abort_timeout(&mut self,doc:&mut Document,forms:&mut Forms,now:Instant)->Outcome{
        self.begin_job();let before=self.state.errors;
        let result=(||->Result<(),String>{
            self.state.work(self.state.abort_signals.len().saturating_mul(3).saturating_add(1))?;
            let selected=self.state.abort_signals.iter().enumerate().filter_map(|(index,record)|record.timeout.as_ref().and_then(|timeout|timeout.due).filter(|due|*due<=now).map(|due|(due,index))).min();
            let Some((_,index))=selected else{self.state.refresh_abort_timeout_deadline();return Ok(());};
            let owner=self.state.abort_signals[index].owner;
            self.state.abort_signals[index].timeout=None;
            self.state.refresh_abort_timeout_deadline();
            if owner!=WINDOW{
                let mut bridge=Bridge{doc,forms,state:&mut self.state};
                let closed=bridge.frame_get_with_caller(Some(WINDOW),owner,"closed").ok_or("Timeout owner Window is unavailable.")??;
                if matches!(closed,Value::Bool(true)){return Ok(());}
                bridge.contains_child_document(owner)?;
            }
            let reason=self.runtime.timeout_abort_reason(owner,&mut Bridge{doc,forms,state:&mut self.state})?;
            let token=Bridge{doc,forms,state:&mut self.state}.commit_signal_abort_dispatch(SIGNAL_BASE+index as u32,reason)?;
            self.runtime.invoke_abort_dispatches(token,&mut Bridge{doc,forms,state:&mut self.state})?;
            Ok(())
        })();
        if let Err(error)=result{self.state.clear_abort_timeouts();self.runtime.cancel_microtasks();self.state.error("AbortSignal.timeout",&error);}
        self.microtask_checkpoint(doc,forms);
        Outcome{prevented:false,layout_changed:std::mem::take(&mut self.state.changed),errors:self.state.errors.saturating_sub(before)}
    }
}
