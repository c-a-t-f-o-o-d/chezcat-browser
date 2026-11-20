//! Admission against the native thread's actual stack reservation.
//! https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentthreadstacklimits
use super::{Fault, Runtime};

impl Runtime {
    pub(super) fn check_native_evaluation_stack(&self) -> Result<(), Fault> {
        #[cfg(test)]
        if let Some(limit) = self.limits.test_native_evaluation_depth {
            return if self.eval_depth >= limit {
                Err(Fault::new(format!("Evaluation nesting limit reached ({limit}).")))
            } else {
                Ok(())
            };
        }

        #[cfg(windows)]
        {
            check_windows_stack("evaluator entry")
        }
        #[cfg(not(windows))]
        {
            if self.eval_depth >= super::MAX_EVAL_DEPTH {
                return Err(Fault::new(format!("Evaluation nesting limit reached on an unmeasured native stack ({}).", super::MAX_EVAL_DEPTH)));
            }
            Ok(())
        }
    }

    pub(super) fn check_native_call_stack(&self, operation: &'static str) -> Result<(), Fault> {
        #[cfg(test)]
        if let Some(limit) = self.limits.test_call_depth {
            return if self.call_depth >= limit {
                Err(Fault::new(format!("{operation} limit reached ({limit}).")))
            } else {
                Ok(())
            };
        }

        #[cfg(windows)]
        {
            check_windows_stack(operation)
        }
        #[cfg(not(windows))]
        {
            if self.call_depth >= super::MAX_CALL_DEPTH {
                return Err(Fault::new(format!("{operation} limit reached on an unmeasured native stack ({}).", super::MAX_CALL_DEPTH)));
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
pub(super) fn check_windows_stack(operation: &'static str) -> Result<(), Fault> {
    let (available, headroom) = current_stack().ok_or_else(||
        Fault::new("Native Windows stack bounds are unavailable for this execution thread."))?;
    if available < headroom {
        return Err(Fault::new(format!(
            "Native stack headroom exhausted during {operation}: {available} bytes available; {headroom} bytes reserved for guarded evaluator work and unwind."
        )));
    }
    Ok(())
}

#[cfg(windows)]
#[inline(never)]
fn current_stack() -> Option<(usize, usize)> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let mut low = 0usize;
    let mut high = 0usize;
    // Both output pointers are live pointer-sized values on this thread.
    unsafe { GetCurrentThreadStackLimits(&mut low, &mut high); }
    let position = std::ptr::addr_of!(low) as usize;
    if !(low..high).contains(&position) { return None; }
    let stack = high - low;
    // The guarded evaluator and parser still contain native-stack work. Keep
    // up to the established one-MiB stress-test envelope available for that
    // work and unwinding, including when entered through a callback.
    let headroom = (1024 * 1024).min(stack / 3);
    Some((position - low, headroom))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use super::super::{Host, Value};

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("No fixture host read.".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("No fixture host write.".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("No fixture host call.".into()) }
    }

    #[inline(never)]
    fn toward_floor(runtime: &Runtime, entered: usize) -> (usize, Fault) {
        let mut frame = [0u8; 16 * 1024];
        std::hint::black_box(&mut frame);
        let result = match runtime.check_native_call_stack("test caller") {
            Err(fault) => (entered, fault),
            Ok(()) => toward_floor(runtime, entered + 1),
        };
        std::hint::black_box(&frame);
        result
    }

    #[test]
    fn browser_native_call_stack_stops_before_the_real_one_mib_stack_floor() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let runtime = Runtime::new();
            let (entered, fault) = toward_floor(&runtime, 0);
            assert!(entered > 8, "entered {entered}");
            assert!(fault.message.contains("Native stack headroom exhausted"), "{}", fault.message);
            assert!(runtime.check_native_call_stack("after unwind").is_ok());
        }).unwrap().join().unwrap();
    }

    #[test]
    fn browser_native_call_stack_runs_finite_script_and_accessor_recursion_past_regression_depth() {
        std::thread::Builder::new().stack_size(8 * 1024 * 1024).spawn(|| {
            let mut runtime = Runtime::new();
            let result = runtime.execute(r#"
function descend(level){
    if(level===0)return Object.getPrototypeOf({})===Object.prototype?1:0;
    return descend(level-1)+1;
}
var cursor=0;
var entry={get depth(){
    if(++cursor===48)return 1;
    return entry.depth+1;
}};
descend(64)+entry.depth;
"#, &mut NoHost);
            assert_eq!(result.unwrap().to_number(), 113.0);
            assert_eq!(runtime.call_depth, 0);
            assert!(runtime.transient_registration_is_empty());
        }).unwrap().join().unwrap();
    }
}
