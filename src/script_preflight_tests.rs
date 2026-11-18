//! Observable call-entry ordering and shared-quota controls. These use real
//! invocations and authored recursion; no test assigns runtime limits or depth.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected native read")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected native write")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected native call")
    }
}

fn assert_clean(runtime: &Runtime) {
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert_eq!(runtime.evaluation_frames, 0);
    assert!(!runtime.strict);
    assert!(runtime.invocation.is_none());
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn script_preflight_invoke_preserves_argument_string_and_callable_error_order() {
    for case in 0..4 {
        let mut runtime = Runtime::for_test();
        let oversized = "x".repeat(runtime.limits.capacity.string_bytes + 1);
        let (args, receiver, expected, fatal) = match case {
            0 => {
                let mut args = vec![Value::Undefined; runtime.limits.capacity.arguments + 1];
                args[0] = Value::String(oversized.clone());
                (args, Value::String(oversized), "Call argument limit", true)
            }
            1 => (vec![Value::String(oversized.clone())], Value::String(oversized), "String limit", true),
            2 => (vec![], Value::String(oversized), "String limit", true),
            _ => (vec![Value::Number(1.0)], Value::Undefined, "not a callable function", false),
        };
        let steps = runtime.steps;
        let error = runtime.invoke(Value::Undefined, &args, receiver, &mut NoHost).unwrap_err();
        assert!(error.contains(expected), "case {case}: {error}");
        assert!(runtime.steps < steps);
        assert_eq!(runtime.invocation_failed_fatally(), fatal, "case {case}");
        let kind = if fatal { "stage=invoke kind=fatal" } else { "stage=invoke kind=TypeError" };
        assert!(runtime.caught_error_report().contains(kind), "case {case}");
        assert_clean(&runtime);
    }
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(r#"
var argumentsEvaluated=0,caught=0,after=0;
try{undefined(argumentsEvaluated++);}catch(e){
 if(e.name!=='TypeError'||!(e instanceof TypeError))throw 'wrong callable error';
 caught++;
}
after++;
argumentsEvaluated===1&&caught===1&&after===1;
"#, &mut NoHost).unwrap().to_text(), "true");
    assert!(!runtime.invocation_failed_fatally());
    assert_clean(&runtime);
}

#[test]
fn script_preflight_real_step_exhaustion_precedes_malformed_call_inputs_without_refill() {
    let mut runtime = Runtime::for_test();
    let error = runtime.execute(r#"
var caught=0,finalized=0,after=0;
try{while(true);}catch(e){caught++;}finally{finalized++;}
after++;
"#, &mut NoHost).unwrap_err();
    assert!(error.contains("Execution step limit"), "{error}");
    assert_eq!(runtime.steps, 0);
    assert!(runtime.invocation_failed_fatally());
    for name in ["caught", "finalized", "after"] {
        assert_eq!(runtime.binding_value(0, name).unwrap().to_number(), 0.0, "{name}");
    }
    assert_clean(&runtime);
    let deadline = runtime.job_deadline;
    let copies = runtime.copy_bytes;
    let args = vec![Value::Undefined; runtime.limits.capacity.arguments + 1];
    let receiver = Value::String("x".repeat(runtime.limits.capacity.string_bytes + 1));
    let error = runtime.invoke(Value::Undefined, &args, receiver, &mut NoHost).unwrap_err();
    assert!(error.contains("Execution step limit"), "{error}");
    assert_eq!(runtime.steps, 0);
    assert_eq!(runtime.job_deadline, deadline);
    assert!(runtime.copy_bytes <= copies);
    assert_clean(&runtime);
    runtime.begin_job();
    assert_eq!(runtime.execute("caught===0&&finalized===0&&after===0&&6*7===42;", &mut NoHost).unwrap().to_text(), "true");
    assert!(!runtime.invocation_failed_fatally());
    assert_clean(&runtime);
}

#[test]
fn script_preflight_ordinary_and_default_recursion_keep_exact_quota_and_fatal_cleanup() {
    for (declaration, expected_bodies) in [
        ("function f(){'use strict';entries++;bodies++;try{return f();}finally{finalized++;}}", MAX_CALL_DEPTH),
        ("function f(value=(entries++,f())){bodies++;}", 0),
    ] {
        let mut runtime = Runtime::for_test();
        let source = format!(r#"
var entries=0,bodies=0,caught=0,finalized=0,after=0;
{declaration}
try{{f();}}catch(e){{caught++;}}finally{{finalized++;}}
after++;
"#);
        let error = runtime.execute(&source, &mut NoHost).unwrap_err();
        assert!(error.contains("Function call limit reached (32)"), "{error}");
        assert_eq!(runtime.binding_value(0, "entries").unwrap().to_number(), MAX_CALL_DEPTH as f64);
        assert_eq!(runtime.binding_value(0, "bodies").unwrap().to_number(), expected_bodies as f64);
        for name in ["caught", "finalized", "after"] {
            assert_eq!(runtime.binding_value(0, name).unwrap().to_number(), 0.0, "{name}");
        }
        assert!(runtime.invocation_failed_fatally());
        assert_clean(&runtime);
        let remaining = (runtime.steps, runtime.copy_bytes, runtime.job_deadline);
        // Runtime's low-level API permits cleanup/recovery work in the same
        // store. Success does not clear the owner's fatal-job admission flag.
        assert_eq!(runtime.execute("function recovery(){'use strict';return 6*7;}recovery();", &mut NoHost).unwrap().to_number(), 42.0);
        assert!(runtime.steps < remaining.0);
        assert!(runtime.copy_bytes <= remaining.1);
        assert_eq!(runtime.job_deadline, remaining.2);
        assert!(runtime.invocation_failed_fatally());
        assert_clean(&runtime);
        runtime.begin_job();
        assert_eq!(runtime.execute("recovery();", &mut NoHost).unwrap().to_number(), 42.0);
        assert!(!runtime.invocation_failed_fatally());
        assert_clean(&runtime);
    }
}

#[test]
fn script_preflight_math_and_class_calls_keep_their_existing_depth_rules() {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(r#"
var entries=0,parameters=0,bodies=0,caught=0;
class ConstructOnly {constructor(value=parameters++){bodies++;}}
function descend(n){
 entries++;
 if(n===1){
  var value=Math.abs(-7);
  try{ConstructOnly();}catch(e){
   if(e.name!=='TypeError'||!(e instanceof TypeError))throw 'class callable error';
   caught++;
  }
  return value;
 }
 return descend(n-1);
}
var result=descend(32);
result===7&&entries===32&&parameters===0&&bodies===0&&caught===1;
"#, &mut NoHost).unwrap().to_text(), "true");
    assert!(!runtime.invocation_failed_fatally());
    assert_clean(&runtime);
}

#[test]
fn script_preflight_bound_target_uses_the_same_ordinary_call_slot() {
    for depth in [31, 32] {
        let mut runtime = Runtime::for_test();
        let source = format!(r#"
var entries=0,targetBodies=0,caught=0,finalized=0,after=0;
var receiver={{tag:'bound'}};
function target(first,second){{'use strict';targetBodies++;return this===receiver&&first===5&&second===7;}}
var bound=target.bind(receiver,5);
function descend(n){{entries++;if(n===1)return bound(7);return descend(n-1);}}
var result;
try{{result=descend({depth});}}catch(e){{caught++;}}finally{{finalized++;}}
after++;
"#);
        if depth == 31 {
            runtime.execute(&source, &mut NoHost).unwrap();
            assert!(runtime.binding_value(0, "result").unwrap().truthy());
            assert_eq!(runtime.binding_value(0, "targetBodies").unwrap().to_number(), 1.0);
            assert_eq!(runtime.binding_value(0, "finalized").unwrap().to_number(), 1.0);
            assert_eq!(runtime.binding_value(0, "after").unwrap().to_number(), 1.0);
            assert!(!runtime.invocation_failed_fatally());
        } else {
            let error = runtime.execute(&source, &mut NoHost).unwrap_err();
            assert!(error.contains("Function call limit reached (32)"), "{error}");
            for name in ["targetBodies", "finalized", "after"] {
                assert_eq!(runtime.binding_value(0, name).unwrap().to_number(), 0.0, "{name}");
            }
            assert!(runtime.invocation_failed_fatally());
        }
        assert_eq!(runtime.binding_value(0, "entries").unwrap().to_number(), f64::from(depth));
        assert_eq!(runtime.binding_value(0, "caught").unwrap().to_number(), 0.0);
        assert_clean(&runtime);
    }
}
