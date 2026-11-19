//! Original dynamic-compilation/native-stack regressions, no website programs.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected host get".into()) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected host set".into()) }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected host call".into()) }
}

#[test]
fn original_selected_if_branch_native_pressure_on_one_mib_stack() {
    std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
        for braced in [false, true] {
            for depth in [16, 17, 18, 20] {
              for disabled in [true, false] {
                let mut runtime = Runtime::new();
                runtime.transient_roots.disable_if_statement_tail = disabled;
                let code = format!("{}42;{}", "if(true){".repeat(5), "}".repeat(5));
                runtime.define_global("code", Value::String(code)).unwrap();
                let branch = if braced { "{return descend(n-1);}" } else { "return descend(n-1);" };
                let source = format!("function descend(n){{if(n>0){branch}return eval(code);}}descend({depth});");
                let result = runtime.execute(&source, &mut NoHost);
                if disabled && braced && depth >= 17 {
                    let error = result.unwrap_err();
                    assert!(error.contains("Parser nesting limit")
                        || error.contains("Native stack headroom exhausted"), "{error}");
                    println!("Original selected-if baseline: braced={braced}; calls={depth}; {error}");
                } else {
                    assert_eq!(result.unwrap().to_number(), 42.0);
                }
                assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
                runtime.begin_job();
                assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
              }
            }
        }
    }).unwrap().join().unwrap();
}

#[test]
fn selected_if_tail_matches_original_completions_labels_steps_and_fault_locations() {
    for source in [
        "var n=0;outer:if(true)inner:if(true){n=7;break outer;n=99;}if(n!==7)throw 'label';n;",
        "var n=0;outer:if(false)n=99;else inner:if(true){n=11;break outer;}n;",
        "var n=0;while(n<4){if(++n===2)continue;if(n===3)break;}n;",
        "var x=eval('17;if(false)99;');if(x!==undefined)throw 'empty';x;",
        "var x=eval('17;if(true);');if(x!==undefined)throw 'selected empty';x;",
        "function f(){try{if(true)if(false)return 0;else return 23;}finally{if(true);}}f();",
        "var answer=0;try{if(true)if(false);else throw 29;}catch(e){answer=e;}finally{if(false)answer=99;}answer;",
        "var answer=0;if(false);else if(true)try{throw 29;}catch(e){answer=e;}finally{answer++;}answer;",
        "function f(){if(true)try{return 17;}finally{return 29;}return 0;}f();",
        "if(false);else\nif(true)\n missingName();",
    ] {
        for allowance in (0..=120).chain(std::iter::once(10_000)) {
            let mut results = Vec::new();
            for disabled in [true, false] {
                let mut runtime = Runtime::new();
                runtime.transient_roots.disable_if_statement_tail = disabled;
                runtime.steps = allowance;
                let result = runtime.execute(source, &mut NoHost).map(|value| value.to_text());
                results.push((result, runtime.steps));
                assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
                assert!(runtime.transient_registration_is_empty());
            }
            assert_eq!(results[0], results[1], "allowance={allowance}; source={source}");
        }
    }
}

#[test]
fn statement_expression_original_shallow_dynamic_parse_fits_one_mib() {
    std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
        let mut runtime = Runtime::new();
        let source = "function descend(n){if(n>0)return descend(n-1);return eval('6*7;');}descend(20);";
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_number(), 42.0);
        assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
        runtime.begin_job();
        let error = runtime.execute("descend(40);", &mut NoHost).unwrap_err();
        assert!(error.contains("Native stack headroom exhausted")
            || error.contains("Function call limit") || error.contains("Evaluation nesting limit"), "{error}");
        assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
        runtime.begin_job();
        assert_eq!(runtime.execute("descend(1);", &mut NoHost).unwrap().to_number(), 42.0);
    }).unwrap().join().unwrap();
}

#[test]
fn selected_if_pending_metadata_has_an_exact_finite_copy_allowance() {
    for (source, slots) in [("if(true)7;", 1), ("if(false);else if(true)7;", 2)] {
        let charge = slots * std::mem::size_of::<StmtId>();
        for allowance in [0, charge - 1, charge] {
            let mut runtime = Runtime::new();
            let program = runtime.register_program(source, false).unwrap();
            runtime.copy_bytes = allowance;
            let deadline = runtime.job_deadline;
            let result = runtime.statements(&program, &program.body, 0, &Value::Undefined, &mut NoHost);
            if allowance < charge {
                assert!(result.err().unwrap().message.contains("String/copy work limit reached"));
                assert_eq!(runtime.copy_bytes, 0);
            } else {
                let Control::Normal(Some(value)) = result.unwrap() else { panic!("normal completion"); };
                assert_eq!(value.to_number(), 7.0);
                assert_eq!(runtime.copy_bytes, 0);
            }
            assert_eq!(runtime.job_deadline, deadline);
            assert!(runtime.transient_registration_is_empty());
            assert_eq!((runtime.eval_depth, runtime.call_depth), (0, 0));
        }
    }
}

#[test]
fn statement_expression_live_stack_and_recursive_parser_share_original_bound() {
    std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
        let mut first_rejected = None;
        for arrays in 0..=64 {
            let mut runtime = Runtime::new();
            let code = format!("{}42{};", "[".repeat(arrays), "]".repeat(arrays));
            runtime.define_global("code", Value::String(code)).unwrap();
            runtime.begin_job();
            let result = runtime.execute("function descend(n){if(n>0)return descend(n-1);return eval(code);}descend(20);", &mut NoHost);
            match result {
                Ok(_) => assert!(first_rejected.is_none(), "recursive parse admission must be monotonic"),
                Err(error) => {
                    assert!(error.contains("Parser nesting limit reached (64)"), "{error}");
                    first_rejected.get_or_insert(arrays);
                }
            }
            assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
            runtime.begin_job();
            assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
        }
        let first = first_rejected.expect("recursive parser must retain finite rejection");
        assert!(first > 2 && first <= 64, "first rejected depth={first}");
        eprintln!("With 20 live recursive calls, first rejected recursive array depth: {first}");
    }).unwrap().join().unwrap();
}
