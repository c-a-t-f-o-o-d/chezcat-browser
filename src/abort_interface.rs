//! Browser-owned cancellation, using the same EventTarget path as DOM events.
//! https://dom.spec.whatwg.org/#aborting-ongoing-activities
use super::*;

pub(crate) const CONTROLLER: &str = "AbortController";
pub(crate) const SIGNAL_INTERFACE: &str = "AbortSignal";
pub(crate) const SIGNAL: &str = "\0AbortController.signal";
pub(crate) const ABORT: &str = "\0AbortController.abort";
pub(crate) const ABORTED: &str = "\0AbortSignal.aborted";
pub(crate) const REASON: &str = "\0AbortSignal.reason";
pub(crate) const THROW: &str = "\0AbortSignal.throwIfAborted";
pub(crate) const STATIC_ABORT: &str = "\0AbortSignal.static.abort";
pub(crate) const ANY: &str = "\0AbortSignal.static.any";
pub(crate) const TIMEOUT: &str = "\0AbortSignal.static.timeout";
pub(crate) const ON_ABORT: &str = "\0AbortSignal.onabort";
pub(crate) const SET_ON_ABORT: &str = "\0AbortSignal.set.onabort";

pub(crate) fn operation(method: &str) -> bool {
    matches!(
        method,
        CONTROLLER
            | SIGNAL_INTERFACE
            | SIGNAL
            | ABORT
            | ABORTED
            | REASON
            | THROW
            | STATIC_ABORT
            | TIMEOUT
            | ANY
            | ON_ABORT
            | SET_ON_ABORT
    )
}

impl Runtime {
    fn signal_abort_reason(&mut self, value: Option<&Value>) -> Result<Value, Fault> {
        match value {
            None | Some(Value::Undefined) => {
                self.native_dom_exception("AbortError", "This operation was aborted")
            }
            Some(value) => self.copy_value(value),
        }
    }

    #[inline(never)]
    pub(super) fn abort_native_operation(
        &mut self,
        receiver: u32,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Option<Result<Value, Fault>> {
        if !matches!(method, ABORT | THROW | STATIC_ABORT | TIMEOUT | ANY) {
            return None;
        }
        Some((|| match method {
            ABORT => {
                let Value::Host(signal) = host.call(receiver, SIGNAL, &[]).map_err(Fault::new)?
                else {
                    return Err(Fault::new("Controller has no admitted signal."));
                };
                if host
                    .call(signal, ABORTED, &[])
                    .map_err(Fault::new)?
                    .truthy()
                {
                    return Ok(Value::Undefined);
                }
                let reason = self.signal_abort_reason(args.first())?;
                self.pin_exported_value(&reason);
                let token = host
                    .begin_abort_dispatch(receiver, reason)
                    .map_err(Fault::new)?;
                self.dispatch_abort_events(token, host)?;
                Ok(Value::Undefined)
            }
            THROW => {
                let reason = host.call(receiver, REASON, &[]).map_err(Fault::new)?;
                if matches!(reason, Value::Undefined) {
                    Ok(Value::Undefined)
                } else {
                    Err(Fault::thrown(reason))
                }
            }
            ANY => self.dependent_abort_signal(receiver,args.first().unwrap_or(&Value::Undefined),host),
            TIMEOUT => {
                let number=args.first().unwrap_or(&Value::Undefined).script_number()?;
                let integer=number.trunc();
                if !number.is_finite() || integer<0.0 || integer>9007199254740991.0 {
                    return Err(Fault::exception("TypeError","Timeout requires a nonnegative safe integer millisecond count."));
                }
                host.new_timeout_signal(receiver,integer as u64).map(Value::Host).map_err(Fault::new)
            }
            STATIC_ABORT => {
                let reason = self.signal_abort_reason(args.first())?;
                self.pin_exported_value(&reason);
                host.new_aborted_signal(receiver, reason)
                    .map(Value::Host)
                    .map_err(Fault::new)
            }
            _ => unreachable!(),
        })())
    }
}

impl Runtime {
    pub(crate) fn timeout_abort_reason(&mut self,owner:u32,host:&mut dyn Host)->Result<Value,String>{
        let realm=self.ensure_window_realm(owner,host).map_err(Fault::display)?;
        self.with_intrinsic_realm(realm,|runtime|{
            let reason=runtime.native_dom_exception("TimeoutError","The operation timed out")?;
            runtime.pin_exported_value(&reason);Ok(reason)
        }).map_err(Fault::display)
    }
}


pub(crate) enum ArgumentError {TypeError(String),Fatal(String)}
pub(crate) struct Dispatch {
    pub(crate) token:Option<usize>,
    pub(crate) signal:u32,
    pub(crate) event:u32,
    pub(crate) reason:Value,
}
impl Runtime {
    fn dependent_abort_signal(&mut self,owner:u32,source:&Value,host:&mut dyn Host)->Result<Value,Fault>{
        if Ordinary::from_value(source).is_none()&&!matches!(source,Value::Host(_)){
            return Err(Fault::exception("TypeError","AbortSignal.any requires an iterable object."));
        }
        let caller=self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(window,_)|window));
        let method=self.iterator_method(source,host)?.ok_or_else(||Fault::exception("TypeError","AbortSignal.any requires an iterable object."))?;
        let receiver=self.copy_value(source)?;
        let mut iterator=self.iterator_from_method(receiver,method,host)?;
        let mut signals=Vec::new();
        while let Some(value)=self.iterator_next(&mut iterator,host)?{
            host.native_operation_caller(caller);
            let signal=host.abort_signal_argument(&value).map_err(|error|match error{
                ArgumentError::TypeError(message)=>Fault::exception("TypeError",message),
                ArgumentError::Fatal(message)=>Fault::new(message),
            })?;
            self.work(std::mem::size_of::<u32>())?;
            signals.try_reserve(1).map_err(|_|Fault::new("Could not reserve abort signal sequence."))?;
            signals.push(signal);
        }
        host.native_operation_caller(caller);
        host.new_dependent_signal(owner,&signals).map(Value::Host).map_err(Fault::new)
    }
    fn dispatch_abort_events(&mut self,steps:Vec<Dispatch>,host:&mut dyn Host)->Result<(),Fault>{
        let mut result=Ok(());
        for (index,step) in steps.iter().enumerate(){
            host.abort_dispatch_step(step.signal,step.event);
            if let Err(error)=self.dispatch_native_event(step.token.unwrap(),host){
                for pending in &steps[index+1..]{host.finish_event_dispatch(pending.token.unwrap());}
                result=Err(error);break;
            }
        }
        host.finish_abort_dispatches(steps);result
    }
    pub(crate) fn invoke_abort_dispatches(&mut self,steps:Vec<Dispatch>,host:&mut dyn Host)->Result<(),String>{
        self.flush_native_microtask_cancellations(host);
        let result=self.dispatch_abort_events(steps,host).map(|()|Value::Undefined);
        self.finish_invocation(result,host).map(|_|())
    }
}
