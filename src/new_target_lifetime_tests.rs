//! Original identity/root/restoration controls; no captured framework source.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected native read") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected native write") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected native call") }
}

#[test]
fn new_target_arrow_is_the_only_remaining_constructor_edge_and_retires_when_dropped() {
    let mut runtime = Runtime::for_capacity_test();
    // The arguments formal suppresses the implicit callee capture, and a null
    // prototype removes the instance.constructor backlink. The constructor is
    // anonymous, so there is no private named-function binding either.
    let source = "var holder={};holder.ctor=function(arguments){holder.arrow=()=>new.target;};holder.ctor.prototype=null;0;";
    runtime.execute(source, &mut NoHost).unwrap();
    let Value::Object(holder) = runtime.binding_value(0, "holder").unwrap() else { panic!("holder") };
    let Value::Function(target) = runtime.get_property(holder, "ctor").unwrap() else { panic!("ctor") };
    runtime.execute("new holder.ctor();holder.ctor=null;0;", &mut NoHost).unwrap();
    assert!(runtime.mark_persistent_functions(&[], false).unwrap().functions[target]);
    while runtime.function_arena.occupied < MAX_FUNCTIONS * 3 / 4 {
        runtime.native_function(Builtin::IsNaN, "root pressure").unwrap();
    }
    runtime.function_arena.next_collection_at = 0;
    let steps = runtime.steps;
    runtime.collect_function_candidates_at_entry().unwrap();
    assert!(runtime.steps < steps);
    assert!(matches!(runtime.functions[target], Function::Script { .. }));
    // Invoke through the ordinary API, without pinning a returned constructor.
    let Value::Function(arrow) = runtime.get_property(holder, "arrow").unwrap() else { panic!("arrow") };
    let returned = runtime.call_value(Value::Function(arrow), &[], Value::Undefined, &mut NoHost).unwrap();
    assert!(matches!(returned, Value::Function(id) if id == target));
    assert_eq!(runtime.function_source(target).unwrap().to_text(), "function(arguments){holder.arrow=()=>new.target;}");
    runtime.execute("holder.arrow=null;0;", &mut NoHost).unwrap();
    assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[target]);
    runtime.function_arena.next_collection_at = 0;
    runtime.collect_function_candidates_at_entry().unwrap();
    assert!(matches!(runtime.functions[target], Function::Vacant));
    assert!(matches!(runtime.functions[arrow], Function::Vacant));
    assert!(runtime.invocation.is_none());
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn new_target_fatal_quota_restores_context_and_cannot_be_caught_to_continue() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute("var escaped=0;function Busy(){try{while(true){}}catch(e){escaped++;}finally{escaped++;}}function Plain(){return new.target;}0;", &mut NoHost).unwrap();
    runtime.steps = 500;
    let error = runtime.execute("new Busy();escaped++;", &mut NoHost).unwrap_err();
    assert!(error.contains("limit"), "{error}");
    assert!(runtime.invocation_failed_fatally());
    assert!(runtime.invocation.is_none());
    assert_eq!((runtime.call_depth, runtime.eval_depth, runtime.evaluation_frames), (0, 0, 0));
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    assert_eq!(runtime.execute("escaped===0&&Plain()===undefined;", &mut NoHost).unwrap().to_text(), "true");
    assert!(runtime.invocation.is_none());
}

#[test]
fn new_target_dynamic_function_keeps_owner_realm_and_restores_parent_context() {
    let mut runtime = Runtime::for_capacity_test();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let child = runtime.with_intrinsic_realm(Some(realm), |runtime| {
        runtime.function_constructor(&[Value::String("this.target=new.target;this.arrow=()=>new.target;this.owner=Array;".into())], &mut NoHost)
    }).unwrap();
    runtime.define_global("Child", child).unwrap();
    assert_eq!(runtime.execute(r#"
function Parent(){var a=new Child();if(a.target!==Child||a.arrow.call({})!==Child||a.owner===Array)throw 'child construction';if(new.target!==Parent)throw 'parent restoration';this.ok=true;}
if(!(new Parent()).ok)throw 'parent';
var receiver={};Child.call(receiver);if(receiver.target!==undefined||receiver.arrow()!==undefined)throw 'child ordinary call';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
    assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
    assert!(runtime.invocation.is_none());
    assert!(runtime.transient_registration_is_empty());
}
