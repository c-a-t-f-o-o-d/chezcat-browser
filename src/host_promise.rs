//! Genuine Promise-returning host operations and bounded native settlements.
//! No script helper, thenable shortcut or capability-failure-to-rejection path.
use super::*;

impl Runtime {
    pub(super) fn drain_native_promise_effects(&mut self,host:&mut dyn Host)->Result<usize,Fault> {
        let mut count=0usize;
        while let Some((promise,rejection))=host.peek_native_promise_effect() {
            self.step()?;
            if !self.native_promise_pending(&promise).map_err(Fault::new)? {
                host.finish_native_promise_effect();
                count=count.checked_add(1).ok_or_else(||Fault::new("Native settlement count overflow."))?;
                continue;
            }
            let outcome=if let Some((name,message))=rejection {
                self.work(message.len())?;
                let mut retained=String::new();
                retained.try_reserve(message.len()).map_err(|_|Fault::new("Could not retain native Promise rejection."))?;
                retained.push_str(message);
                NativePromiseOutcome::RejectDom {name,message:retained}
            } else {NativePromiseOutcome::Fulfill};
            // The host keeps the front token until this state transition has
            // completed. A later retry observes already-settled state first.
            self.settle_native_promise(&promise,outcome).map_err(Fault::new)?;
            host.finish_native_promise_effect();
            count=count.checked_add(1).ok_or_else(||Fault::new("Native settlement count overflow."))?;
        }
        Ok(count)
    }

    pub(crate) fn process_native_promise_effects(&mut self,host:&mut dyn Host)->Result<usize,String> {
        self.flush_native_microtask_cancellations(host);
        let result=self.drain_native_promise_effects(host);
        if result.as_ref().is_err_and(|fault|fault.exception.is_none()) {
            self.job_failed_fatally=true;self.cancel_microtasks();
        }
        self.flush_native_microtask_cancellations(host);
        result.map_err(|fault|self.display_fault(fault))
    }

    pub(super) fn call_host_promise(&mut self,mut object:u32,method:&str,receiver_required:bool,
        this:Value,host:&mut dyn Host)->Result<Value,Fault> {
        let marker=self.inhibit_transient_collection();
        let result=(|| {
            self.validate_native_access(object,host)?;
            if receiver_required {
                match this {
                    Value::Host(receiver) if host.accepts_receiver(object,method,receiver)=>object=receiver,
                    _=>return Err(Fault::exception("TypeError","Native method called with an incompatible receiver.")),
                }
            }
            self.validate_native_access(object,host)?;
            self.check_native_call_stack("Function call")?;
            let (promise,value)=self.new_native_promise()?;
            match host.validate_promise_operation(object,method) {
                Ok(())=>{},
                Err(NativePromiseFailure::Dom(name,message))=>{
                    self.settle_native_promise(&promise,NativePromiseOutcome::RejectDom {name,message:message.into()}).map_err(Fault::new)?;
                    return Ok(value);
                },
                Err(NativePromiseFailure::Fatal(message))=>return Err(Fault::new(message)),
            }
            let ticket=self.prepare_host_microtask(host,object,method,&[],false)?;
            host.native_operation_caller(self.native_access_caller
                .unwrap_or_else(||self.browser_global.map(|(window,_)|window)));
            self.call_depth+=1;
            let accepted=host.call_promise_operation(object,method,promise.clone());
            self.call_depth-=1;
            if self.drain_native_promise_effects(host)?!=0 {
                self.defer_host_microtask_after_effects(ticket)?;
            }
            self.finish_host_microtask(host,ticket)?;
            match accepted {
                Ok(())=>Ok(value),
                Err(NativePromiseFailure::Dom(name,message))=>{
                    self.settle_native_promise(&promise,NativePromiseOutcome::RejectDom {name,message:message.into()}).map_err(Fault::new)?;
                    Ok(value)
                },
                Err(NativePromiseFailure::Fatal(message))=>Err(Fault::new(message)),
            }
        })();
        self.restore_transient_marker(marker);
        let result=match result {Ok(value)=>Ok(value),Err(fault)=>self.rejected_native_operation(fault)};
        self.checked_value(result?)
    }
}
