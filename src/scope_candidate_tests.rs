use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected host get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected host set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected host call") }
}

#[test]
fn scope_candidate_comparison_tags_only_successful_direct_anonymous_expression_stores() {
    let mut runtime = Runtime::new();
    runtime.execute("var a=function(){return 1;};let b=()=>2;const c=function(){return 3;};var d;d=function(){return 4;};0;", &mut NoHost).unwrap();
    assert_eq!(runtime.scope_assigned_closures.len(), 4);
    let before = runtime.scope_assigned_closures.clone();
    runtime.execute("var alias=a;var named=function privateName(){};function declaration(){};(function(){});var method={m(){}};var dynamic=Function('return 5;');var logical;logical||=function(){};var [destructured]=[function(){}];0;", &mut NoHost).unwrap();
    assert_eq!(runtime.scope_assigned_closures, before);
    for source in ["'use strict';missing=function(){};", "c=function(){};", "{before=function(){};let before;}"] {
        assert!(runtime.execute(source, &mut NoHost).is_err());
        assert_eq!(runtime.scope_assigned_closures, before);
    }
    assert!(runtime.object_assigned_closures.is_empty());
}

#[test]
fn scope_candidate_comparison_preserves_production_marks_and_reports_dead_binding_cycles() {
    let mut runtime = Runtime::new();
    runtime.execute("function create(){var n=17;var f=function(){return n;};return f;}var kept=create();0;", &mut NoHost).unwrap();
    assert_eq!(runtime.scope_assigned_closures.len(), 1);
    let id = *runtime.scope_assigned_closures.iter().next().unwrap();
    assert!(runtime.mark_scope_candidate_comparison(true).unwrap().functions[id]);
    runtime.execute("kept=null;0;", &mut NoHost).unwrap();
    let before = (runtime.functions.len(), runtime.function_arena.occupied, runtime.bindings,
        runtime.value_bytes, runtime.steps, runtime.copy_bytes, runtime.job_deadline);
    assert!(runtime.mark_root_graph(&[], true, true).unwrap().functions[id]);
    assert!(!runtime.mark_scope_candidate_comparison(true).unwrap().functions[id]);
    assert!(runtime.function_is_occupied(id));
    assert!(runtime.persistent_allocation_report().contains("Production scope retirement remains disabled"));
    assert_eq!((runtime.functions.len(), runtime.function_arena.occupied, runtime.bindings,
        runtime.value_bytes, runtime.steps, runtime.copy_bytes, runtime.job_deadline), before);
}

#[test]
fn scope_candidate_comparison_keeps_permanent_exports_and_new_identity_after_reused_slot() {
    let mut runtime = Runtime::new();
    let exported = runtime.execute("var f=function(){return 23;};f;", &mut NoHost).unwrap();
    let Value::Function(id) = exported else { unreachable!() };
    runtime.execute("f=null;0;", &mut NoHost).unwrap();
    assert!(runtime.mark_scope_candidate_comparison(true).unwrap().functions[id]);
    assert!(!runtime.mark_scope_candidate_comparison(false).unwrap().functions[id]);
    assert_eq!(runtime.invoke(exported, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 23.0);

    let mut runtime = Runtime::new();
    runtime.execute("var holder={};var f;holder.fn=f=function(){return 9;};holder.fn=null;f=null;0;", &mut NoHost).unwrap();
    let id = *runtime.scope_assigned_closures.iter().next().unwrap();
    // Supply the old object class in this allocator-only test; production never
    // retires the new scope class in this checkpoint.
    runtime.object_assigned_closures.insert(id);
    runtime.retire_proven_inactive_candidate(id).unwrap();
    assert!(!runtime.scope_assigned_closures.contains(&id));
    assert!(matches!(runtime.native_function(Builtin::IsNaN, "replacement").unwrap(), Value::Function(found) if found == id));
    assert!(!runtime.scope_assigned_closures.contains(&id));
}
