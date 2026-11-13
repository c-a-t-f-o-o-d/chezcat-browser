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
fn statement_expression_original_shallow_dynamic_parse_fits_one_mib() {
    std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
        let mut runtime = Runtime::new();
        let source = "function descend(n){if(n>0)return descend(n-1);return eval('6*7;');}descend(20);";
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_number(), 42.0);
        assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
        runtime.begin_job();
        let error = runtime.execute("descend(40);", &mut NoHost).unwrap_err();
        assert!(error.contains("Function call limit") || error.contains("Evaluation nesting limit"), "{error}");
        assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
        runtime.begin_job();
        assert_eq!(runtime.execute("descend(1);", &mut NoHost).unwrap().to_number(), 42.0);
    }).unwrap().join().unwrap();
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
