//! Bounded allocation attribution for the existing cumulative function guard.
//! Counts only; no source, function names, URLs or host/device data is collected.
use super::*;

impl Runtime {
    pub(super) fn function_limit_fault(&self, requested: &'static str) -> Fault {
        let mut kinds = [0usize; 5];
        let mut main = [0usize; 5];
        for (index, function) in self.functions.iter().enumerate().take(MAX_FUNCTIONS) {
            let kind = match function {
                Function::Native(_) => 0,
                Function::Script { .. } => 1,
                Function::Host { .. } => 2,
                Function::Bound(_) => 3,
                Function::Promise(_) => 4,
            };
            kinds[kind] += 1;
            if self.function_realms.get(index).is_some_and(Option::is_none) { main[kind] += 1; }
        }
        Fault::new(format!("Function limit reached (1024). Requested={requested}; active-realm={:?}; native/script/host/bound/promise={kinds:?}; main-realm={main:?}; detached-realms={}.",
            self.active_realm, self.intrinsic_realms.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_function_guard_attributes_kinds_and_realms_without_allocating_a_callable() {
        let mut runtime = Runtime::new();
        let realm = runtime.allocate_intrinsic_realm().unwrap();
        runtime.with_intrinsic_realm(Some(realm), |runtime| {
            while runtime.functions.len() < MAX_FUNCTIONS { runtime.native_function(Builtin::IsNaN, "fixture")?; }
            let before = (runtime.functions.len(), runtime.objects.len(), runtime.job_usage(), runtime.job_deadline);
            let fault = runtime.native_function(Builtin::IsNaN, "fixture").unwrap_err();
            assert!(fault.message.contains("Requested=native") && fault.message.contains("active-realm=Some(0)") && fault.message.contains("main-realm="), "{}", fault.message);
            assert!(fault.exception.is_none());
            assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.job_usage(), runtime.job_deadline), before);
            println!("{}", fault.message);
            Ok(())
        }).unwrap();
    }
}
