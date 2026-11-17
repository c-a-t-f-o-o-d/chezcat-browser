//! Original class cycles, interrupted allocation and parser-capacity controls.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected native read") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected native write") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected native call") }
}

#[test]
fn base_class_repeated_private_cycles_retire_without_losing_identity_or_original_source() {
    let mut runtime = Runtime::for_capacity_test();
    let source = r#"
var kept,last;
for(var i=0;i<700;i++){
 var C=class Self {constructor(value){this.value=value;}read(){return this.value;}static owner(){return Self;}};
 if(i===0)kept=C;last=C;
}
if(kept===last||kept.owner()!==kept||last.owner()!==last||(new kept(17)).read()!==17||(new last(19)).read()!==19)throw 'live class identity';
0;
"#;
    runtime.execute(source, &mut NoHost).unwrap();
    assert!(runtime.function_arena.scope_retired > 1000);
    assert!(runtime.function_arena.environments_retired > 300);
    let program = Rc::downgrade(runtime.programs.last().unwrap());
    for _ in 0..140 {
        runtime.begin_job();
        runtime.execute("0;", &mut NoHost).unwrap();
    }
    assert!(runtime.program_allocation.retired > 0);
    assert!(program.upgrade().is_some());
    runtime.begin_job();
    assert_eq!(runtime.execute("kept.toString();", &mut NoHost).unwrap().to_text(),
        "class Self {constructor(value){this.value=value;}read(){return this.value;}static owner(){return Self;}}");
    assert!(runtime.invocation.is_none());
    assert!(runtime.transient_registration_is_empty());

}

#[test]
fn base_class_capacity_failures_do_not_publish_partial_definitions_and_partial_slots_retire() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute("var target='old';0;", &mut NoHost).unwrap();
    let before = (runtime.function_arena.occupied, runtime.objects.len());
    let functions = runtime.limits.capacity.functions;
    runtime.limits.capacity.functions = before.0 + 1;
    let error = runtime.execute("target=class {first(){}second(){}};", &mut NoHost).unwrap_err();
    assert!(error.contains("Function limit"), "{error}");
    assert_eq!((runtime.function_arena.occupied, runtime.objects.len()), before);
    runtime.limits.capacity.functions = functions;
    runtime.begin_job();
    assert_eq!(runtime.execute("target;", &mut NoHost).unwrap().to_text(), "old");

    let scripts = runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count();
    let properties = runtime.limits.heap.properties;
    runtime.limits.heap.properties = runtime.properties + 4;
    let error = runtime.execute("target=class {method(){return 1;}};", &mut NoHost).unwrap_err();
    assert!(error.contains("property limit"), "{error}");
    runtime.limits.heap.properties = properties;
    runtime.begin_job();
    assert_eq!(runtime.execute("target;", &mut NoHost).unwrap().to_text(), "old");
    assert!(runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count() > scripts);
    // The real partial definitions are candidates, not permanent untracked
    // slots. Lower only the private fixture threshold; do not reset job work.
    runtime.limits.capacity.functions = runtime.function_arena.occupied + 1;
    runtime.function_arena.next_collection_at = 0;
    let steps = runtime.steps;
    runtime.collect_function_candidates_at_entry().unwrap();
    assert!(runtime.steps < steps);
    assert_eq!(runtime.functions.iter().filter(|function| matches!(function, Function::Script { .. })).count(), scripts);
    assert!(runtime.invocation.is_none());
    assert!(runtime.transient_registration_is_empty());

    let mut bounded = Runtime::for_capacity_test();
    bounded.execute("var target='old';0;", &mut NoHost).unwrap();
    bounded.limits.capacity.environments = bounded.environments.len();
    assert!(bounded.free_environments.is_empty());
    let before = bounded.function_arena.occupied;
    let error = bounded.execute("target=class Named {method(){return Named;}};", &mut NoHost).unwrap_err();
    assert!(error.contains("Environment limit"), "{error}");
    assert_eq!(bounded.function_arena.occupied, before);
    bounded.begin_job();
    assert_eq!(bounded.execute("target;", &mut NoHost).unwrap().to_text(), "old");
}

#[test]
fn base_class_ast_and_member_caps_account_for_pending_class_records() {
    let source = "class C {first(){}second(){}third(){}}";
    let mut regexp_work = MAX_REGEXP_WORK;
    let parsed = Parser::parse(source, &mut regexp_work).unwrap();
    assert_eq!(parsed.class_members, 3);
    assert_eq!(parsed.classes.len(), 1);
    let nodes = parsed.node_count();
    let mut capacity = limits::REGRESSION_CAPACITIES;
    capacity.ast_nodes = nodes - 1;
    let fault = Parser::parse_context_with_capacity(source, &mut MAX_REGEXP_WORK.clone(), false, 0,
        capacity, limits::REGRESSION_HEAP).err().expect("complete class exceeds AST capacity");
    assert!(fault.message.contains("AST node limit"), "{fault:?}");
    let mut heap = limits::REGRESSION_HEAP;
    heap.object_properties = 2;
    let fault = Parser::parse_context_with_capacity(source, &mut MAX_REGEXP_WORK.clone(), false, 0,
        limits::REGRESSION_CAPACITIES, heap).err().expect("pending class exceeds member capacity");
    assert!(fault.message.contains("Class member limit"), "{fault:?}");
}
