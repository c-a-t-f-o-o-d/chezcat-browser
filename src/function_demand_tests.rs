//! Original allocation-demand/root/pacing regressions; no website code.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
}

fn fixture(free: usize, extra: &str) -> Runtime {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(&format!("var box={{}},tmp;box.keep=function(){{return 71;}};box.dead=function(){{return 73;}};function take(a,b){{return a();}}function tick(){{return 1;}}{extra}0;"), &mut NoHost).unwrap();
    while runtime.function_arena.occupied < MAX_FUNCTIONS - free { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
    runtime.function_arena.next_collection_at = runtime.function_arena.allocations + 10_000;
    runtime
}

#[test]
fn unchanged_near_cap_calls_do_not_repeat_a_completed_no_progress_mark() {
    let mut runtime = fixture(0, "");
    runtime.function_arena.next_collection_at = 0;
    runtime.execute("tick();0;", &mut NoHost).unwrap();
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 0));
    runtime.execute("for(var i=0;i<5000;i++)if(tick()!==1)throw 'call';0;", &mut NoHost).unwrap();
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 0));
    assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
    assert!(runtime.steps > 0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn standalone_literal_at_full_capacity_retries_after_references_clear_without_allocations() {
    let mut runtime = fixture(0, "");
    runtime.execute("box.dead=null;0;", &mut NoHost).unwrap();
    let allocations = runtime.function_arena.allocations;
    let value = runtime.execute("(function fresh(){return 79;});", &mut NoHost).unwrap();
    assert_eq!(runtime.function_arena.allocations, allocations + 1);
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 1));
    assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
    assert_eq!(runtime.invoke(value, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 79.0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn allocation_demand_keeps_candidate_held_only_in_an_earlier_argument() {
    let mut runtime = fixture(0, "");
    assert_eq!(runtime.execute("take((tmp=box.keep,box.keep=null,tmp),(tmp=null,box.dead=null,function fresh(){}));", &mut NoHost).unwrap().to_number(), 71.0);
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 1));
    assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn demand_snapshot_keeps_the_entire_outer_argument_prefix_across_a_nested_call() {
    let setup = "function make(n){var holder={};holder.fn=function(){return n;};return holder.fn;}function inner(a,b){if(a()!==72||typeof b!=='function')throw 'inner';return a;}function outer(a,b){if(a===b||a()!==71||b()!==72)throw 'outer prefix';return 143;}";
    let mut runtime = fixture(2, setup);
    assert_eq!(runtime.execute("box.dead=null;outer(make(71),inner(make(72),function fresh(){}));", &mut NoHost).unwrap().to_number(), 143.0);
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 1));
    assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn allocation_demand_keeps_computed_receiver_key_accessors_and_capture_state() {
    let setup = "var setterHits=0,keyHits=0,seen=0;Object.defineProperty({},'warm',{value:0});function makeReceiver(){var state=97,target={};target.tmp=function(v){setterHits++;seen=v()+state;};Object.defineProperty(target,'slot',{set:target.tmp});delete target.tmp;return target;}function makeKey(){var key={};key.toString=function(){keyHits++;return 'slot';};return key;}";
    let mut runtime = fixture(2, setup);
    runtime.execute("box.dead=null;makeReceiver()[makeKey()]=function rhs(){return 3;};if(setterHits!==1||keyHits!==1||seen!==100)throw 'reference roots/order/state';0;", &mut NoHost).unwrap();
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 1));
    assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn allocation_demand_preserves_inhibitors_and_the_live_function_cap() {
    for source in ["(function stillLive(){});", "box.dead=null;({fn:function inhibited(){}});", "box.dead=null;[function inhibited(){}];"] {
        let mut runtime = fixture(0, "");
        let created = runtime.function_arena.script_created;
        let error = runtime.execute(source, &mut NoHost).unwrap_err();
        assert!(error.contains("Function limit reached (1024)"), "{source}: {error}");
        assert_eq!(runtime.function_arena.script_created, created);
        assert_eq!(runtime.function_arena.retired, 0);
        assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
        assert!(runtime.transient_registration_is_empty());
    }
}

#[test]
fn demand_mark_budget_failure_publishes_no_function_or_private_environment() {
    for expired in [false, true] {
        let mut runtime = fixture(0, "");
        runtime.execute("box.dead=null;0;", &mut NoHost).unwrap();
        let program = runtime.register_program("(function privateName(){return 83;});", false).unwrap();
        let StmtKind::Expr(expression) = program.stmts[program.body[0]].kind else { unreachable!() };
        let before = (runtime.function_arena.allocations, runtime.function_arena.script_created, runtime.value_bytes,
            runtime.bindings, runtime.environments.iter().filter(|env| env.is_some()).count());
        if expired { runtime.job_deadline = Instant::now() - Duration::from_secs(1); } else { runtime.steps = 5; }
        let deadline = runtime.job_deadline;
        assert!(runtime.eval_heap(&program, expression, 0, &Value::Undefined, &mut NoHost).is_err());
        assert_eq!((runtime.function_arena.allocations, runtime.function_arena.script_created, runtime.value_bytes,
            runtime.bindings, runtime.environments.iter().filter(|env| env.is_some()).count()), before);
        assert_eq!(runtime.job_deadline, deadline);
        assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (0, 0));
        assert!(runtime.transient_registration_is_empty());
        assert_eq!(runtime.evaluation_frames, 0);
        runtime.begin_job();
        let function = runtime.eval_heap(&program, expression, 0, &Value::Undefined, &mut NoHost).unwrap();
        assert_eq!(runtime.invoke(function, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 83.0);
    }
}

#[test]
fn demand_snapshot_capacity_failure_restores_owned_frames_before_any_allocation() {
    let mut runtime = fixture(0, "");
    let program = runtime.register_program("(function privateName(){});", false).unwrap();
    let StmtKind::Expr(expression) = program.stmts[program.body[0]].kind else { unreachable!() };
    let before = (runtime.function_arena.allocations, runtime.function_arena.script_created,
        runtime.bindings, runtime.value_bytes, runtime.environments.len());
    let marker = runtime.root_marker();
    while runtime.push_transient_root(function_reachability::Edge::Environment(0)).is_ok() {}
    assert!(runtime.eval_heap(&program, expression, 0, &Value::Undefined, &mut NoHost).is_err());
    assert_eq!((runtime.function_arena.allocations, runtime.function_arena.script_created,
        runtime.bindings, runtime.value_bytes, runtime.environments.len()), before);
    assert_eq!(runtime.evaluation_frames, 0);
    runtime.restore_root_marker(marker);
    assert!(runtime.transient_registration_is_empty());
}
