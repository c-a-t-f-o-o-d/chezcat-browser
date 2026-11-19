//! Bounded allocation attribution for the occupied function-slot guard.
//! Counts and internal helper/native intrinsic names only; no source, Script
//! names, URLs or host/device data is collected.
use super::*;

impl Runtime {
    pub(super) fn function_limit_fault(&self, requested: &'static str) -> Fault {
        let mut kinds = [0usize; 5];
        let mut main = [0usize; 5];
        let mut templates = BTreeMap::new();
        for (index, function) in self.functions.iter().enumerate().take(self.limits.capacity.functions) {
            let kind = match function {
                Function::Vacant => continue,
                Function::Native(_) => 0,
                Function::Script { .. } => 1,
                Function::Host { .. } => 2,
                Function::Bound(_) => 3,
                Function::Promise(_) => 4,
            };
            kinds[kind] += 1;
            if self.function_realms.get(index).is_some_and(Option::is_none) { main[kind] += 1; }
            if let Function::Script { program, template, .. } = function {
                let program_id = self.programs.iter().position(|candidate| Rc::ptr_eq(candidate, program)).unwrap_or(self.limits.capacity.programs);
                *templates.entry((program_id, *template)).or_insert(0usize) += 1;
            }
        }
        let mut repeated: Vec<_> = templates.into_iter().map(|((program, template), count)| (program, template, count)).collect();
        repeated.sort_unstable_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)).then_with(|| left.1.cmp(&right.1)));
        repeated.truncate(3);
        // Classify only three retained templates, using AST metadata. No source
        // text, public names or URLs are read or emitted by this diagnostic.
        let shapes: Vec<_> = repeated.iter().filter_map(|&(program, template, _)| {
            let program = self.programs.get(program)?;
            let syntax = program.templates.get(template)?;
            let kind = if syntax.dynamic_source { "dynamic" }
                else if syntax.arrow { "arrow" }
                else if !syntax.constructible { "method/accessor" }
                else if program.stmts.iter().take(self.limits.capacity.ast_nodes).any(|statement| matches!(&statement.kind, StmtKind::Function(_, id) if *id == template)) { "declaration" }
                else if syntax.name.is_some() { "named-expression" }
                else { "anonymous-expression" };
            Some((kind, syntax.source_start, syntax.source_end.saturating_sub(syntax.source_start)))
        }).collect();
        Fault::new(format!("Function limit reached ({}). Requested={requested}; active-realm={:?}; native/script/host/bound/promise={kinds:?}; main-realm={main:?}; detached-realms={}; top-script-program/template/count={repeated:?}; top-script-kind/offset/bytes={shapes:?}; {}; {}.",
            self.limits.capacity.functions, self.active_realm, self.intrinsic_realms.len(), self.inhibition_diagnostic(), self.microtask_allocation_diagnostic()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_function_guard_attributes_kinds_and_realms_without_allocating_a_callable() {
        let mut runtime = Runtime::for_capacity_test();
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
    fn original_replaced_cells_keep_captured_state_receiver_and_identity_then_reach_cumulative_guard() {
        struct NoHost;
        impl Host for NoHost {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
        }
        let setup = r#"
var order='',slots=[],first,previous,count=0;
var model=Object.create(Object.prototype,{payload:{get:function(){order+='g';return this.read();}}});
function replace(value){
    var state=value,writes=0;
    var cell=Object.create(model);
    cell.write=function(next){order+='w';state=next;writes++;};
    cell.read=function(){order+='r';return state+writes;};
    slots[0]=cell;count++;
    return cell;
}
"#;
        let mut runtime = Runtime::for_capacity_test();
        // Preserve the original cumulative failure as the collection-disabled
        // baseline. The enabled workload has independent semantic tests.
        runtime.function_arena.disable_collection = true;
        runtime.execute(setup, &mut NoHost).unwrap();
        let initial = runtime.functions.len();
        runtime.execute("Promise.resolve().then(function(){for(var i=0;i<64;i++){var current=replace(i);if(i===0)first=current;if(previous&&(previous===current||previous.write===current.write||previous.read===current.read))throw 'identity';order='';current.write(i*2);if(current.payload!==i*2+1||order!=='wgr')throw 'receiver/state/order';previous=current;}if(first===slots[0]||first.payload!==1||slots[0].payload!==127)throw 'retained state';});", &mut NoHost).unwrap();
        runtime.microtask_checkpoint(&mut NoHost).unwrap();
        let scripts = runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
        assert_eq!(scripts, 131); // shared getter + factory + reaction + two per cell
        assert!(runtime.functions.len() >= initial + 128);
        println!("Original replaced cells: 64 cells, 128 distinct stateful methods, retained first/last state and w/g/r order pass; Script identities={scripts}.");

        runtime.begin_job();
        runtime.execute("Promise.resolve().then(function(){for(var i=0;i<2000;i++){var current=replace(i);current.write(i*2);if(current.payload!==i*2+1)throw 'state';}});", &mut NoHost).unwrap();
        let error = runtime.microtask_checkpoint(&mut NoHost).unwrap_err();
        assert!(error.contains("Function limit reached (1024)") && error.contains("microtasks-active=Some(\"reaction\")"), "{error}");
        assert_eq!(runtime.functions.len(), MAX_FUNCTIONS);
        println!("Original replacement workload: {error}");
    }

    #[test]
    fn duplicate_direct_hoists_allocate_only_fresh_survivors() {
        struct NoHost;
        impl Host for NoHost {
            fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
        }
        let mut runtime = Runtime::for_capacity_test();
        let baseline = runtime.functions.len();
        runtime.execute("function factory(){function h(){return 1;}function h(){return 2;}return h;}var a=factory(),b=factory();if(a===b||a()!==2||b()!==2)throw 'identity';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), baseline + 3);
        let before = runtime.functions.len();
        runtime.execute("var c=factory();if(c===a||c===b||c()!==2)throw 'later invocation';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), before + 1);

        let mut runtime = Runtime::for_capacity_test();
        let baseline = runtime.functions.len();
        runtime.execute("function alpha(){return 1;}function beta(){return 2;}function alpha(){return 3;}if(alpha()!==3||beta()!==2)throw 'last body';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), baseline + 2);
        let before = runtime.functions.len();
        runtime.execute("var first=eval('function e(){return 1;}function e(){return 2;}e;');var second=eval('function e(){return 1;}function e(){return 2;}e;');if(first===second||first()!==2||second()!==2)throw 'eval identity';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), before + 2);
        assert_eq!(runtime.functions.len(), runtime.function_realms.len());
        assert_eq!(runtime.functions.len(), runtime.function_properties.len());
        assert_eq!(runtime.functions.len(), runtime.function_initial_names.len());
    }

    #[test]
    fn duplicate_direct_hoists_keep_surviving_host_order_and_complete_syntax_preflight() {
        struct Record(Vec<(String, Value)>);
        impl Host for Record {
            fn owns_window_property(&self, object: Option<u32>, name: &str) -> bool { object == Some(77) && matches!(name, "alpha" | "beta") }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
            fn set(&mut self, object: u32, name: &str, value: Value) -> Result<(), String> { assert_eq!(object, 77); self.0.push((name.into(), value)); Ok(()) }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
        }
        let mut runtime = Runtime::for_capacity_test();
        runtime.bind_browser_global(77).unwrap();
        let mut host = Record(Vec::new());
        runtime.execute("function alpha(){return 1;}function beta(){return 2;}function alpha(){return 3;}", &mut host).unwrap();
        assert_eq!(host.0.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["beta", "alpha"]);
        for (position, expected) in [(0, 2.0), (1, 3.0)] {
            let callback = host.0[position].1.clone();
            assert_eq!(runtime.invoke(callback, &[], Value::Undefined, &mut host).unwrap().to_number(), expected);
        }
        for source in ["function alpha(){var broken=;}function alpha(){return 3;}", "{function alpha(){}function alpha(){}}", "let blocked;function blocked(){}function blocked(){}"] {
            let mut runtime = Runtime::for_capacity_test();
            runtime.bind_browser_global(77).unwrap();
            let functions = runtime.functions.len();
            let mut host = Record(Vec::new());
            assert!(runtime.execute(source, &mut host).is_err(), "{source}");
            assert!(host.0.is_empty());
            assert_eq!(runtime.functions.len(), functions);
        }
    }

    #[test]
    fn duplicate_direct_hoists_original_six_hundred_factory_workload_stays_bounded() {
        struct NoHost;
        impl Host for NoHost {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
        }
        let mut runtime = Runtime::for_capacity_test();
        let baseline = runtime.functions.len();
        runtime.execute("function factory(){function h(){return 1;}function h(){return 2;}return h;}var kept=[];for(var i=0;i<600;i++)kept[i]=factory();if(kept[0]===kept[599]||kept[0]()!==2||kept[599]()!==2)throw 'fresh identity or last body';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), baseline + 601);
        println!("Original duplicate declaration workload: bootstrap={baseline}; fresh survivors=601; total={}; unchanged-cap={MAX_FUNCTIONS}.", runtime.functions.len());
        assert!(runtime.execute("for(var j=0;j<1100;j++)(function(){})();", &mut NoHost).unwrap_err().contains("Function limit reached (1024)"));
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
            let mut runtime = Runtime::with_intrinsic_startup(limits::CAPACITY_REGRESSION, deferred);
            let baseline = runtime.functions.len();
            let result = runtime.execute(source, &mut NoHost);
            let scripts = runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
            if deferred {
                assert_eq!(result.unwrap().to_number(), 900.0);
                assert_eq!(scripts, 900);
                assert_eq!(runtime.functions.len(), baseline + 900);
                assert_eq!(runtime.deferred_intrinsics, intrinsic_realms::ALL_DEFERRED);
            } else {
                let error = result.unwrap_err();
                assert!(error.contains("Function limit reached (1024)"), "{error}");
                assert!(error.contains("anonymous-expression"), "{error}");
                assert_eq!(runtime.functions.len(), MAX_FUNCTIONS);
            }
            println!("Original retained closures: deferred={deferred}; bootstrap={baseline}; scripts={scripts}; final-callables={}; callable-cap={MAX_FUNCTIONS}.", runtime.functions.len());
        }
    }
}
