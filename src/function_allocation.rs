//! Bounded allocation attribution for the existing cumulative function guard.
//! Counts only; no source, function names, URLs or host/device data is collected.
use super::*;

impl Runtime {
    pub(super) fn function_limit_fault(&self, requested: &'static str) -> Fault {
        let mut kinds = [0usize; 5];
        let mut main = [0usize; 5];
        let mut templates = BTreeMap::new();
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
            if let Function::Script { program, template, .. } = function {
                let program_id = self.programs.iter().position(|candidate| Rc::ptr_eq(candidate, program)).unwrap_or(MAX_PROGRAMS);
                *templates.entry((program_id, *template)).or_insert(0usize) += 1;
            }
        }
        let mut repeated: Vec<_> = templates.into_iter().map(|((program, template), count)| (program, template, count)).collect();
        repeated.sort_unstable_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)).then_with(|| left.1.cmp(&right.1)));
        repeated.truncate(3);
        Fault::new(format!("Function limit reached (1024). Requested={requested}; active-realm={:?}; native/script/host/bound/promise={kinds:?}; main-realm={main:?}; detached-realms={}; top-script-program/template/count={repeated:?}.",
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

    #[test]
    fn original_retained_closure_workload_measures_unused_main_family_cost() {
        struct NoHost;
        impl Host for NoHost {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected host get".into()) }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected host set".into()) }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected host call".into()) }
        }
        let source = "var kept=[];for(var i=0;i<900;i++)kept[i]=function(){return 1;};if(kept[0]===kept[899]||kept[0]()!==1||kept[899]()!==1)throw 'identity';kept.length;";
        for deferred in [false, true] {
            let mut runtime = Runtime::with_intrinsic_startup(limits::BROWSER, deferred);
            let baseline = runtime.functions.len();
            let result = runtime.execute(source, &mut NoHost);
            let scripts = runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
            if deferred {
                assert_eq!(result.unwrap().to_number(), 900.0);
                assert_eq!(scripts, 900);
                assert_eq!(runtime.functions.len(), baseline + 900);
                assert_eq!(runtime.deferred_intrinsics, 7);
            } else {
                let error = result.unwrap_err();
                assert!(error.contains("Function limit reached (1024)"), "{error}");
                assert_eq!(runtime.functions.len(), MAX_FUNCTIONS);
            }
            println!("Original retained closures: deferred={deferred}; bootstrap={baseline}; scripts={scripts}; final-callables={}; callable-cap={MAX_FUNCTIONS}.", runtime.functions.len());
        }
    }
}
