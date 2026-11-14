//! Original scope-only workloads; no captured website program.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
}

const SETUP: &str = "function checkpoint(){}function pressure(){var trash;for(var p=0;p<750;p++)trash=function(){return p;};}function make(n){var f=function(){return n;};return f;}";

fn entry_runtime() -> Runtime {
    let mut runtime = Runtime::for_capacity_test();
    runtime.transient_roots.disable_function_literal_collection = true;
    runtime
}

fn fixture(free: usize, extra: &str) -> Runtime {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(&format!("var keep=function(){{return 71;}},dead=function(){{return 73;}},tmp;function take(a,b){{return a();}}function tick(){{return 1;}}{extra}0;"), &mut NoHost).unwrap();
    assert!(runtime.object_assigned_closures.is_empty());
    while runtime.function_arena.occupied < MAX_FUNCTIONS - free { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
    runtime.function_arena.next_collection_at = runtime.function_arena.allocations + 10000;
    runtime
}

#[test]
fn scope_collection_original_two_thousand_factories_preserve_state_identity_and_source() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(r#"
function make(n){var f=function(delta){n+=delta;return n;};return f;}
var first,previous,current;for(var i=0;i<2000;i++){
current=make(i);if(i===0)first=current;
if(previous===current||current(2)!==i+2||current(3)!==i+5)throw 'identity/state';previous=current;
}if(first(0)!==5||current(0)!==2004||first.toString()!=='function(delta){n+=delta;return n;}')throw 'retained identity/source';0;
"#, &mut NoHost).unwrap();
    assert!(runtime.object_assigned_closures.is_empty());
    assert!(runtime.function_arena.scope_retired > 0);
    assert!(runtime.objects.len() > 3000);
    println!("Default2000factory phase: scope retired={}; collections={}; occupied={}.", runtime.function_arena.scope_retired, runtime.function_arena.collections, runtime.function_arena.occupied);
    // Force the next audited proof after quiescence, retaining the same workload
    // and retirement assertions rather than assuming a last automatic pass.
    while runtime.function_arena.occupied < MAX_FUNCTIONS * 3 / 4 {
        runtime.native_function(Builtin::IsNaN, "quiescent pressure").unwrap();
    }
    runtime.function_arena.next_collection_at = 0;
    runtime.collect_function_candidates_at_entry().unwrap();
    assert!(runtime.function_arena.scope_retired > 1500);
    assert!(runtime.function_arena.environments_retired > 1500);
    assert!(runtime.function_arena.script_created >= 2001);
    assert!(runtime.function_arena.occupied < MAX_FUNCTIONS);
    assert!(runtime.objects.len() > 3000); // Object slots still append.
    assert_eq!(runtime.execute("first(0)===5&&current(0)===2004;", &mut NoHost).unwrap().to_text(), "true");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn scope_collection_protects_unexported_saved_returns_throws_and_eval_completions() {
    for source in [
        "function returning(){try{return make(71);}finally{pressure();checkpoint();}}if(returning()()!==71)throw 'return';0;",
        "try{try{throw make(72);}finally{pressure();checkpoint();}}catch(e){if(e()!==72)throw 'throw';}0;",
        "var result=eval('make(73);var ignored=(pressure(),checkpoint());');if(result()!==73)throw 'list completion';0;",
        "var result=eval('var i=0;while(i++<1||(pressure(),checkpoint(),false)){make(74);}');if(result()!==74)throw 'loop completion';0;",
        "var result=eval('for(var i=0;i<1;(pressure(),checkpoint())){i++;make(75);}');if(result()!==75)throw 'for completion';0;",
    ] {
        for disabled in [true, false] {
            let mut runtime = entry_runtime();
            runtime.function_arena.disable_scope_collection = disabled;
            runtime.execute(SETUP, &mut NoHost).unwrap();
            runtime.execute(source, &mut NoHost).unwrap();
            assert!(runtime.object_assigned_closures.is_empty());
            if disabled { assert_eq!(runtime.function_arena.retired, 0); }
            else { assert!(runtime.function_arena.scope_retired >= 749, "{source}"); }
            assert!(runtime.transient_registration_is_empty());
        }
    }
}

#[test]
fn scope_collection_full_demand_protects_earlier_and_nested_argument_prefixes() {
    let mut runtime = fixture(0, "");
    assert_eq!(runtime.execute("take((tmp=keep,keep=null,tmp),(tmp=null,dead=null,function fresh(){}));", &mut NoHost).unwrap().to_number(), 71.0);
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.scope_retired), (1, 1));
    assert!(runtime.transient_registration_is_empty());

    let setup = "function make(n){var f=function(){return n;};return f;}function inner(a,b){if(a()!==72||typeof b!=='function')throw 'inner';return a;}function outer(a,b){if(a===b||a()!==71||b()!==72)throw 'outer';return 143;}";
    let mut runtime = fixture(2, setup);
    assert_eq!(runtime.execute("dead=null;outer(make(71),inner(make(72),function fresh(){}));", &mut NoHost).unwrap().to_number(), 143.0);
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.scope_retired), (1, 1));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn scope_collection_full_demand_protects_computed_receiver_key_and_setter() {
    let setup = "var setterHits=0,keyHits=0,seen=0;Object.defineProperty({},'warm',{value:0});function makeReceiver(){var state=97;var setter=function(v){setterHits++;seen=v()+state;};var target={};Object.defineProperty(target,'slot',{set:setter});return target;}function makeKey(){var conversion=function(){keyHits++;return 'slot';};return {toString:conversion};}";
    let mut runtime = fixture(2, setup);
    runtime.execute("dead=null;makeReceiver()[makeKey()]=function rhs(){return 3;};if(setterHits!==1||keyHits!==1||seen!==100)throw 'reference/state/order';0;", &mut NoHost).unwrap();
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.scope_retired), (1, 1));
    assert_eq!(runtime.object_assigned_closures.len(), 1);
    let rhs = *runtime.object_assigned_closures.iter().next().unwrap();
    let Function::Script { program, template, .. } = &runtime.functions[rhs] else { unreachable!() };
    assert_eq!(program.templates[*template].name.as_deref(), Some("rhs"));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn scope_collection_keeps_api_pins_after_aliases_clear_without_vacuous_object_roots() {
    let mut runtime = entry_runtime();
    runtime.execute(SETUP, &mut NoHost).unwrap();
    let function = runtime.execute("make(81);", &mut NoHost).unwrap();
    let object = runtime.execute("var box={fn:make(83)};box;", &mut NoHost).unwrap();
    runtime.execute("box=null;0;", &mut NoHost).unwrap();
    let graph = runtime.mark_scope_candidate_comparison(false).unwrap();
    let Value::Function(function_id) = function else { unreachable!() };
    assert!(!graph.functions[function_id]);
    assert!(runtime.scope_assigned_closures.contains(&function_id));
    runtime.execute("pressure();checkpoint();0;", &mut NoHost).unwrap();
    assert!(runtime.function_arena.scope_retired >= 749);
    assert_eq!(runtime.invoke(function, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 81.0);
    runtime.define_global("recovered", object).unwrap();
    assert_eq!(runtime.execute("recovered.fn();", &mut NoHost).unwrap().to_number(), 83.0);
}

#[test]
fn scope_collection_keeps_lexical_receiver_constructor_backlinks_and_strict_eval() {
    for source in [
        "function build(){'use strict';var f=()=>this.value;return f;}var kept=build.call({value:97});pressure();checkpoint();if(kept()!==97)throw 'lexical receiver';0;",
        "var C=function(v){this.value=v;};var instance=new C(99);C=null;pressure();checkpoint();if(new instance.constructor(101).value!==101||instance.value!==99)throw 'constructor backlink';0;",
        "function build(){'use strict';return eval('var f=function(){return 103;};f;');}function returning(){try{return build();}finally{pressure();checkpoint();}}if(returning()()!==103)throw 'strict eval';0;",
    ] {
        let mut runtime = entry_runtime();
        runtime.execute(SETUP, &mut NoHost).unwrap();
        runtime.execute(source, &mut NoHost).unwrap();
        assert!(runtime.object_assigned_closures.is_empty());
        assert!(runtime.function_arena.scope_retired >= 749, "{source}");
        assert!(runtime.transient_registration_is_empty());
    }
}

fn ready() -> Runtime {
    let mut runtime = entry_runtime();
    runtime.execute(SETUP, &mut NoHost).unwrap();
    runtime.execute("pressure('retained payload');0;", &mut NoHost).unwrap();
    assert_eq!(runtime.function_arena.collections, 0);
    assert!(runtime.function_arena.occupied >= MAX_FUNCTIONS * 3 / 4);
    runtime
}

#[test]
fn scope_collection_failed_mark_preflight_and_deadline_retire_nothing() {
    let mut probe = ready();
    let steps = probe.steps;
    probe.collect_function_candidates_at_entry().unwrap();
    let cost = steps - probe.steps;
    for failure in ["zero", "steps", "late", "time", "invalid", "refund"] {
        let mut runtime = ready();
        if failure == "refund" { runtime.value_bytes = 0; }
        let before = (runtime.function_arena.occupied, runtime.bindings, runtime.value_bytes,
            runtime.environments.iter().filter(|env| env.is_some()).count(), runtime.scope_assigned_closures.clone());
        match failure {
            "zero" => runtime.steps = 0,
            "steps" => runtime.steps = 5,
            "late" => runtime.steps = cost - 1,
            "time" => runtime.job_deadline = Instant::now() - Duration::from_secs(1),
            "invalid" => { runtime.exported_function_roots.insert(usize::MAX); }
            _ => {}
        }
        let deadline = runtime.job_deadline;
        assert!(runtime.collect_function_candidates_at_entry().is_err());
        assert_eq!((runtime.function_arena.occupied, runtime.bindings, runtime.value_bytes,
            runtime.environments.iter().filter(|env| env.is_some()).count(), runtime.scope_assigned_closures.clone()), before, "{failure}");
        assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired, runtime.function_arena.scope_retired), (0, 0, 0));
        assert_eq!(runtime.job_deadline, deadline);
    }
}

#[test]
fn scope_collection_keeps_live_capacity_and_inhibitors_unchanged() {
    for source in ["(function stillLive(){});", "dead=null;({fn:function inhibited(){}});", "dead=null;[function inhibited(){}];"] {
        let mut runtime = fixture(0, "");
        let before = runtime.function_arena.script_created;
        let error = runtime.execute(source, &mut NoHost).unwrap_err();
        assert!(error.contains("Function limit reached (1024)"), "{error}");
        assert_eq!(runtime.function_arena.script_created, before);
        assert_eq!(runtime.function_arena.scope_retired, 0);
        assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
        assert!(runtime.transient_registration_is_empty());
    }
}

#[test]
fn scope_collection_full_demand_retries_after_only_transient_references_disappear() {
    let mut runtime = fixture(0, "");
    runtime.function_arena.next_collection_at = 0;
    let allocations = runtime.function_arena.allocations;
    let function = runtime.execute("tick((tmp=keep,keep=null,tmp),(tmp=null));(function fresh(){return 79;});", &mut NoHost).unwrap();
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.scope_retired), (2, 1));
    assert_eq!(runtime.function_arena.allocations, allocations + 1);
    assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
    assert_eq!(runtime.invoke(function, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 79.0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn scope_collection_keeps_foreign_realm_globals_and_clears_reused_slot_provenance() {
    let mut runtime = entry_runtime();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    runtime.with_intrinsic_realm(Some(realm), |runtime| {
        runtime.execute("var childSaved=function(){return 43;};0;", &mut NoHost).map_err(Fault::new)?;
        Ok(())
    }).unwrap();
    let child = *runtime.scope_assigned_closures.iter().next().unwrap();
    runtime.execute(&SETUP.replace("p<750", "p<pressureLimit"), &mut NoHost).unwrap();
    let count = MAX_FUNCTIONS * 3 / 4 + 8 - runtime.function_arena.occupied;
    assert!(count > 0);
    runtime.define_global("pressureLimit", Value::Number(count as f64)).unwrap();
    runtime.execute("pressure();checkpoint();0;", &mut NoHost).unwrap();
    assert!(runtime.function_arena.scope_retired >= count - 1);
    assert!(runtime.function_is_occupied(child));
    assert_eq!(runtime.function_realms[child], Some(realm));
    runtime.with_intrinsic_realm(Some(realm), |runtime| {
        assert_eq!(runtime.execute("childSaved();", &mut NoHost).map_err(Fault::new)?.to_number(), 43.0);
        runtime.execute("childSaved=null;0;", &mut NoHost).map_err(Fault::new)?;
        Ok(())
    }).unwrap();
    assert!(!runtime.mark_scope_candidate_comparison(true).unwrap().functions[child]);
    while runtime.function_arena.occupied < MAX_FUNCTIONS * 3 / 4 {
        runtime.native_function(Builtin::IsNaN, "realm pressure").unwrap();
    }
    runtime.function_arena.next_collection_at = 0;
    runtime.collect_function_candidates_at_entry().unwrap();
    assert!(!runtime.function_is_occupied(child));
    assert!(!runtime.scope_assigned_closures.contains(&child));
    assert!(matches!(runtime.native_function(Builtin::IsNaN, "replacement").unwrap(), Value::Function(id) if id == child));
    assert_eq!(runtime.function_realms[child], None);
    assert!(!runtime.scope_assigned_closures.contains(&child));
}

#[test]
fn scope_collection_full_demand_at_twenty_live_script_calls_fits_one_mib_stack() {
    std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
        let mut runtime = fixture(0, "function descend(n){if(n>0)return descend(n-1);dead=null;return (function fresh(){return 79;});}");
        let deadline = runtime.job_deadline;
        let function = runtime.execute("descend(20);", &mut NoHost).unwrap();
        assert_eq!((runtime.function_arena.collections, runtime.function_arena.scope_retired), (1, 1));
        assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
        assert_eq!(runtime.invoke(function, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 79.0);
        assert_eq!(runtime.job_deadline, deadline);
        assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
        assert!(runtime.transient_registration_is_empty());
    }).unwrap().join().unwrap();
}
