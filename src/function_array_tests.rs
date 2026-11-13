//! Original array-root controls; candidate factories do not capture their arrays.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
}

const FACTORY: &str = "var reads=0;function candidate(n){var box={};box.fn=function(){reads++;return n;};return box.fn;}function array(n){return [candidate(n)];}";
const PRESSURE: &str = "var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;0;";

fn compared(runtime: &Runtime, exports: bool) -> Graph {
    runtime.trace_root_graph_array_policy(&[], exports, true, false, false,
        &mut Meter { remaining: None, deadline: runtime.job_deadline }).unwrap()
}

#[test]
fn sparse_symbol_accessor_and_prototype_array_edges_survive_mark_and_collection() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(FACTORY, &mut NoHost).unwrap();
    runtime.execute("var holder=[],key=Symbol('slot');Object.defineProperty(holder,'1000',{value:candidate(11)});Object.defineProperty(holder,'read',{get:candidate(13)});holder[key]=candidate(17);var proto={fn:candidate(19)};Object.setPrototypeOf(holder,proto);proto=null;0;", &mut NoHost).unwrap();
    let ids: Vec<_> = runtime.object_assigned_closures.iter().copied().collect();
    assert_eq!(ids.len(), 4);
    let graph = compared(&runtime, true);
    assert!(ids.iter().all(|&id| graph.functions[id]));
    assert_eq!(runtime.binding_value(0, "reads").unwrap().to_number(), 0.0);
    runtime.execute(PRESSURE, &mut NoHost).unwrap();
    assert!(runtime.function_arena.retired > 500);
    assert!(ids.iter().all(|&id| runtime.function_is_occupied(id)));
    assert_eq!(runtime.binding_value(0, "reads").unwrap().to_number(), 0.0);
    assert_eq!(runtime.execute("if(holder.length!==1001||holder[999]!==undefined)throw 'sparse';holder[1000]()+holder.read+holder[key]()+holder.fn();", &mut NoHost).unwrap().to_number(), 60.0);
    assert_eq!(runtime.binding_value(0, "reads").unwrap().to_number(), 4.0);
}

#[test]
fn array_api_pin_remains_a_root_after_all_script_names_disappear() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(FACTORY, &mut NoHost).unwrap();
    let exported = runtime.execute("var held=array(83);held;", &mut NoHost).unwrap();
    let id = *runtime.object_assigned_closures.iter().next().unwrap();
    runtime.execute("held=null;0;", &mut NoHost).unwrap();
    assert!(compared(&runtime, true).functions[id]);
    assert!(!compared(&runtime, false).functions[id]);
    runtime.execute(PRESSURE, &mut NoHost).unwrap();
    assert!(runtime.function_arena.retired > 500);
    runtime.define_global("recovered", exported).unwrap();
    assert_eq!(runtime.execute("recovered[0]();", &mut NoHost).unwrap().to_number(), 83.0);
}

#[test]
fn array_edges_in_iterators_promises_bound_values_and_pending_arguments_remain_strong() {
    for (setup, probe) in [
        ("var holder=array(91).values();", "holder.next().value();"),
        ("var holder=Promise.resolve(array(91));", "var result=0;holder.then(function(a){result=a[0]();});0;"),
        ("var holder=(function(){return this[0]();}).bind(array(91));", "holder();"),
        ("var holder=(function(a){return a[0]();}).bind(null,array(91));", "holder();"),
        ("var holder=(function(a){'use strict';a=null;return ()=>arguments[0];})(array(91));", "holder()[0]();"),
        ("var holder=(function(a){var args=arguments;a=array(91);return args;})(null);", "holder[0][0]();"),
        ("var holder=new Map();holder.set(array(91),17);", "holder.keys().next().value[0]();"),
        ("var holder=Promise.all([array(91)]);", "var result=0;holder.then(function(a){result=a[0][0]();});0;"),
    ] {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute(FACTORY, &mut NoHost).unwrap();
        runtime.execute(&format!("{setup}0;"), &mut NoHost).unwrap();
        let id = *runtime.object_assigned_closures.iter().next().unwrap();
        assert!(compared(&runtime, true).functions[id], "{setup}");
        runtime.execute(PRESSURE, &mut NoHost).unwrap();
        assert!(runtime.function_arena.retired > 500, "{setup}");
        assert!(runtime.function_is_occupied(id), "{setup}");
        let result = runtime.execute(probe, &mut NoHost).unwrap();
        if setup.contains("Promise.") {
            assert!(runtime.microtask_checkpoint(&mut NoHost).unwrap().is_empty());
            assert_eq!(runtime.execute("result;", &mut NoHost).unwrap().to_number(), 91.0);
        } else { assert_eq!(result.to_number(), 91.0, "{setup}"); }
        assert!(runtime.transient_registration_is_empty());
    }
}

#[test]
fn active_array_argument_roots_are_proven_without_array_seeds_or_api_pins() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CHECKED: AtomicUsize = AtomicUsize::new(0);
    fn observe(runtime: &Runtime, function: usize, args: &[Value], _: &Value) {
        let name = runtime.objects[runtime.function_properties[function]].properties["name"].value.as_value().unwrap();
        if !matches!(name, Value::String(name) if name == "observe") { return; }
        assert!(runtime.active_roots_are_audited());
        let Value::Object(array) = args[0] else { panic!("array argument") };
        let StoredValue::Value(Value::Function(candidate)) = runtime.objects[array].properties["0"].value else { panic!("candidate") };
        assert!(runtime.object_assigned_closures.contains(&candidate));
        let graph = runtime.trace_root_graph_array_policy(&[], false, true, true, false,
            &mut Meter { remaining: None, deadline: runtime.job_deadline }).unwrap();
        assert!(graph.objects[array] && graph.functions[candidate]);
        let without = runtime.trace_root_graph_array_policy(&[], false, true, false, false,
            &mut Meter { remaining: None, deadline: runtime.job_deadline }).unwrap();
        assert!(!without.functions[candidate]);
        CHECKED.fetch_add(1, Ordering::Relaxed);
    }
    let mut runtime = Runtime::for_capacity_test();
    runtime.transient_roots.script_input_observer = Some(observe);
    runtime.execute(FACTORY, &mut NoHost).unwrap();
    assert_eq!(runtime.execute("function observe(a){return a[0]();}observe(array(91));", &mut NoHost).unwrap().to_number(), 91.0);
    assert_eq!(CHECKED.load(Ordering::Relaxed), 1);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn original_discarded_array_cell_cycles_reclaim_functions_without_losing_live_state() {
    let mut runtime = Runtime::for_capacity_test();
    let deadline = runtime.job_deadline;
    runtime.execute("function build(n){var state=n,c={},a=[c];c.write=function(v){state=v;};c.read=function(){return state;};c.back=a;return a;}var first,previous;for(var i=0;i<2000;i++){var current=build(i);if(i===0)first=current;if(previous&&(previous===current||previous[0].write===current[0].write||previous[0].read===current[0].read))throw 'identity';current[0].write(i*2);if(current[0].read()!==i*2)throw 'state';previous=current;}if(first[0].read()!==0||current[0].read()!==3998||first[0].back!==first||current[0].back!==current)throw 'live cycles';0;", &mut NoHost).unwrap();
    assert!(runtime.function_arena.retired > 3000);
    assert!(runtime.function_arena.environments_retired > 1500);
    assert!(runtime.function_arena.script_created >= 4001);
    assert!(runtime.function_arena.occupied < MAX_FUNCTIONS);
    assert!(runtime.objects.len() > 12_000); // Object/array slots are still append-only.
    assert_eq!(runtime.job_deadline, deadline);
    assert!(runtime.transient_registration_is_empty());
}

fn full_array_fixture(free: usize, extra: &str) -> Runtime {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(FACTORY, &mut NoHost).unwrap();
    runtime.execute(&format!("var dead={{}};dead.fn=function(){{return 0;}};{extra}0;"), &mut NoHost).unwrap();
    while runtime.function_arena.occupied < MAX_FUNCTIONS - free { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
    runtime.function_arena.next_collection_at = runtime.function_arena.allocations + 10_000;
    runtime
}

#[test]
fn full_cap_demand_keeps_array_prefix_receiver_and_effective_this() {
    for (free, extra, source, expected) in [
        (1, "function take(a,b){return a[0]();}", "take(array(71),(dead.fn=null,function fresh(){}));", 71.0),
        (2, "function inner(a,b){return a;}function outer(a,b){if(a===b)throw 'identity';return a[0]()+b[0]();}", "outer(array(71),inner(array(72),(dead.fn=null,function fresh(){})));", 143.0),
        (1, "function method(fresh){return this[0]();}function target(n){var a=array(n);a.run=method;return a;}", "target(73).run((dead.fn=null,function fresh(){}));", 73.0),
        (1, "function method(){dead.fn=null;var fresh=function(){};return this[0]();}function target(n){var a=array(n);a.run=method;return a;}", "target(75).run();", 75.0),
    ] {
        let mut runtime = full_array_fixture(free, extra);
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_number(), expected, "{source}");
        assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 1));
        assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
        assert!(runtime.transient_registration_is_empty());
    }
}

#[test]
fn pending_return_and_throw_arrays_survive_collection_in_finally() {
    for source in [
        "function returning(){try{return array(71);}finally{pressure();}}if(returning()[0]()!==71)throw 'pending return';0;",
        "try{try{throw array(73);}finally{pressure();}}catch(e){if(e[0]()!==73)throw 'pending throw';}0;",
    ] {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute(FACTORY, &mut NoHost).unwrap();
        runtime.execute("function pressure(){var trash={};for(var i=0;i<750;i++)trash.fn=function(){return i;};}0;", &mut NoHost).unwrap();
        runtime.execute(source, &mut NoHost).unwrap();
        assert!(runtime.function_arena.retired > 500);
        assert!(runtime.transient_registration_is_empty());
    }
}
