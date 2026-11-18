//! Reachability and retirement controls over real WeakMap and WeakSet storage.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected host get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected host set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected host call") }
}
fn object(runtime: &Runtime, name: &str) -> usize {
    let Some(Value::Object(id)) = runtime.environment(0).bindings[name].value else { panic!("object fixture") };
    id
}
fn function(runtime: &Runtime, name: &str) -> usize {
    let Some(Value::Function(id)) = runtime.environment(0).bindings[name].value else { panic!("function fixture") };
    id
}
fn slots(runtime: &Runtime, id: usize) -> usize {
    runtime.objects[id].collection.as_ref().unwrap().weak_slot_count()
}

#[test]
fn weak_map_gc_fixed_point_requires_reached_map_and_independently_reached_key() {
    let mut runtime = Runtime::new();
    runtime.execute(r#"
var start={},middle={},holder={};
holder.kept=function(){return 73;};holder.dropped=function(){return 79;};
var saved=holder.kept,deadValue=holder.dropped;
var later=new WeakMap(),earlier=new WeakMap();
later.set(middle,saved);earlier.set(start,middle);
var deadMap=new WeakMap();
deadMap.set(start,deadValue);holder=null;0;
"#, &mut NoHost).unwrap();
    let saved = function(&runtime, "saved");
    let dead_value = function(&runtime, "deadValue");
    let dead_map = object(&runtime, "deadMap");
    let middle = object(&runtime, "middle");
    runtime.execute("middle=null;saved=null;deadMap=null;deadValue=null;0;", &mut NoHost).unwrap();
    let usage = (runtime.steps, runtime.copy_bytes, runtime.value_bytes, runtime.job_deadline);
    // Allocation order makes the first map's key appear only after the second
    // map activates its value. One forward scan cannot mark the saved closure.
    for graph in [runtime.mark_root_graph(&[], true, true).unwrap(),
                  runtime.mark_scope_candidate_comparison(true).unwrap(),
                  runtime.mark_persistent_functions(&[], true).unwrap()] {
        assert!(graph.objects[middle] && graph.functions[saved]);
        assert!(!graph.objects[dead_map] && !graph.functions[dead_value]);
    }
    assert_eq!((runtime.steps, runtime.copy_bytes, runtime.value_bytes, runtime.job_deadline), usage);
    assert_eq!(slots(&runtime, dead_map), 1, "mark-only audit cannot sweep");
    assert_eq!(runtime.execute("later.get(earlier.get(start))();", &mut NoHost).unwrap().to_number(), 73.0);
}

#[test]
fn weak_map_gc_plans_only_maps_that_have_entries() {
    let mut runtime = Runtime::new();
    runtime.execute(r#"
        var maps = [], key = {};
        for (var i = 0; i < 40; i++) maps.push(new WeakMap());
        var kept = maps[17];
        kept.set(key, 'middle');
        maps[21].set(key, 'removed');
        maps[21].delete(key);
        0;
    "#, &mut NoHost).unwrap();
    let graph = runtime.mark_persistent_functions(&[], true).unwrap();
    assert_eq!(graph.weak_collections, [object(&runtime, "kept")]);
}

#[test]
fn weak_set_membership_tracks_liveness_after_map_values_are_traced() {
    let mut runtime = Runtime::new();
    runtime.execute(r#"
        var key = {}, member = {}, kept = new WeakSet([member]), gone = new WeakSet([key]);
        var map = new WeakMap(), holder = {};
        map.set(key, member);
        holder.temp = function() { return 1; };
        kept.add(holder.temp);
        0;
    "#, &mut NoHost).unwrap();
    let kept = object(&runtime, "kept");
    let gone = object(&runtime, "gone");
    let member = object(&runtime, "member");
    let Value::Function(temp) = runtime.own_property(object(&runtime, "holder"), "temp").unwrap() else {
        panic!("expected function member");
    };
    runtime.execute("member=null;gone=null;holder.temp=null;0;", &mut NoHost).unwrap();
    let graph = runtime.mark_scope_candidate_comparison(true).unwrap();
    assert!(graph.objects[member]);
    assert!(!graph.objects[gone] && !graph.functions[temp]);
    let before = runtime.collection_entries;
    let steps = runtime.steps;
    runtime.steps = 0;
    assert!(runtime.retire_proven_inactive_candidate(temp).is_err());
    assert!(runtime.function_is_occupied(temp));
    assert_eq!((slots(&runtime, kept), slots(&runtime, gone)), (2, 1));
    assert_eq!(runtime.collection_entries, before);
    runtime.steps = steps;
    runtime.retire_proven_inactive_candidate(temp).unwrap();
    assert_eq!(runtime.collection_entries, before - 2);
    assert_eq!((slots(&runtime, kept), slots(&runtime, gone)), (1, 0));
    let replacement = runtime.native_function(Builtin::IsNaN, "replacement").unwrap();
    assert!(matches!(replacement, Value::Function(id) if id == temp));
    runtime.define_global("replacement", replacement).unwrap();
    assert_eq!(runtime.execute("kept.has(map.get(key)) && !kept.has(replacement);",
        &mut NoHost).unwrap().to_text(), "true");
}

#[test]
fn weak_map_gc_dead_cycles_and_function_key_reuse_are_swept_before_publication() {
    let mut runtime = Runtime::new();
    runtime.execute(r#"
var map=new WeakMap(),holder={};
holder.key=function(){return 11;};map.set(holder.key,'original payload');0;
"#, &mut NoHost).unwrap();
    let map = object(&runtime, "map");
    let key = match runtime.own_property(object(&runtime, "holder"), "key").unwrap() {
        Value::Function(id) => id, _ => panic!("function key"),
    };
    runtime.execute("holder.key=null;0;", &mut NoHost).unwrap();
    let retained = runtime.value_bytes;
    runtime.retire_proven_inactive_candidate(key).unwrap();
    assert_eq!(slots(&runtime, map), 0);
    assert_eq!(runtime.collection_entries, 0);
    assert!(runtime.value_bytes <= retained - "original payload".len());
    let replacement = runtime.native_function(Builtin::IsNaN, "replacement").unwrap();
    assert!(matches!(replacement, Value::Function(id) if id == key), "actual arena reuse");
    runtime.define_global("replacement", replacement).unwrap();
    assert_eq!(runtime.execute("!map.has(replacement)&&map.get(replacement)===undefined;", &mut NoHost).unwrap().to_text(), "true");

    runtime.execute(r#"
function cycle(){var key={},cell={};cell.fn=function(){return key;};map.set(key,cell.fn);}
cycle();0;
"#, &mut NoHost).unwrap();
    let candidate = *runtime.object_assigned_closures.iter().next().unwrap();
    let graph = runtime.mark_scope_candidate_comparison(true).unwrap();
    assert!(!graph.functions[candidate], "value-to-key cycle cannot seed itself");
    runtime.retire_proven_inactive_candidate(candidate).unwrap();
    assert_eq!(slots(&runtime, map), 0);
    assert!(!runtime.function_is_occupied(candidate));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn weak_map_gc_active_argument_and_export_pins_keep_weak_values() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static AUDITS: AtomicUsize = AtomicUsize::new(0);
    fn observe(runtime: &Runtime, function: usize, _: &[Value], _: &Value) {
        let StoredValue::Value(Value::String(name)) = &runtime.objects[runtime.function_properties[function]].properties["name"].value else { return; };
        if name != "checkpoint" { return; }
        let graph = runtime.mark_audited_active_roots(false).unwrap();
        let map = object(runtime, "map");
        for (key, value) in runtime.objects[map].collection.as_ref().unwrap().weak_entries() {
            assert!(graph.weak_key_live(key));
            let Value::Function(id) = value else { panic!("weak value fixture") };
            assert!(graph.functions[*id]);
        }
        AUDITS.fetch_add(1, Ordering::Relaxed);
    }
    AUDITS.store(0, Ordering::Relaxed);
    let mut runtime = Runtime::new();
    runtime.execute(r#"
var map=new WeakMap(),key={},saved=function(){return 83;};map.set(key,saved);saved=null;
function checkpoint(){return 0;}function receive(arg){key=null;checkpoint();return map.get(arg)();}0;
"#, &mut NoHost).unwrap();
    runtime.transient_roots.script_input_observer = Some(observe);
    assert_eq!(runtime.execute("receive(key);", &mut NoHost).unwrap().to_number(), 83.0);
    runtime.transient_roots.script_input_observer = None;
    assert_eq!(AUDITS.load(Ordering::Relaxed), 1);
    assert!(runtime.transient_registration_is_empty());

    runtime.execute("key={};saved=function(){return 89;};map.set(key,saved);saved=null;0;", &mut NoHost).unwrap();
    let exported = runtime.execute("key;", &mut NoHost).unwrap();
    let Value::Object(key) = exported else { panic!("exported key") };
    let map = object(&runtime, "map");
    let saved = runtime.objects[map].collection.as_ref().unwrap().weak_entries()
        .find_map(|(candidate, value)| match (candidate, value) {
            (collections::WeakKey::Object(id), Value::Function(function)) if id == key => Some(*function),
            _ => None,
        }).unwrap();
    runtime.execute("key=null;0;", &mut NoHost).unwrap();
    assert!(runtime.mark_persistent_functions(&[], true).unwrap().functions[saved]);
    assert!(!runtime.mark_persistent_functions(&[], false).unwrap().functions[saved]);
}

#[test]
fn weak_map_gc_host_and_symbol_keys_conservatively_retain_without_native_storage() {
    let mut runtime = Runtime::new();
    runtime.define_global("native", Value::Host(7)).unwrap();
    runtime.execute(r#"
var map=new WeakMap(),sym=Symbol('original'),a=function(){return 97;},b=function(){return 101;};
map.set(native,a);map.set(sym,b);0;
"#, &mut NoHost).unwrap();
    let a = function(&runtime, "a");
    let b = function(&runtime, "b");
    let map = object(&runtime, "map");
    let host_stores = runtime.host_objects.len();
    runtime.execute("native=null;sym=null;a=null;b=null;0;", &mut NoHost).unwrap();
    let graph = runtime.mark_persistent_functions(&[], false).unwrap();
    assert!(graph.functions[a] && graph.functions[b]);
    assert_eq!(runtime.host_objects.len(), host_stores);
    assert_eq!(slots(&runtime, map), 2);
    assert!(runtime.transient_registration_is_empty());
}

fn pressured() -> (Runtime, usize) {
    let mut runtime = Runtime::for_capacity_test();
    runtime.transient_roots.disable_function_literal_collection = true;
    runtime.execute(r#"
var map=new WeakMap();
function pressure(){var key={},cell={};for(var i=0;i<750;i++)cell.fn=function(){return i;};map.set(key,'dead payload');}
pressure();0;
"#, &mut NoHost).unwrap();
    let map = object(&runtime, "map");
    assert_eq!(runtime.function_arena.collections, 0);
    assert!(runtime.function_arena.occupied >= MAX_FUNCTIONS * 3 / 4);
    (runtime, map)
}

#[test]
fn weak_map_gc_preflight_failures_are_atomic_and_spend_existing_allowance() {
    let (mut probe, map) = pressured();
    let steps = probe.steps;
    probe.collect_function_candidates_at_entry().unwrap();
    let cost = steps - probe.steps;
    assert_eq!(slots(&probe, map), 0);
    assert!(probe.function_arena.retired >= 749);
    for failure in ["steps", "late-steps", "deadline", "refund", "invalid-root"] {
        let (mut runtime, map) = pressured();
        if failure == "steps" { runtime.steps = 0; }
        if failure == "late-steps" { runtime.steps = cost - 1; }
        if failure == "deadline" { runtime.job_deadline = Instant::now() - Duration::from_secs(1); }
        if failure == "refund" { runtime.value_bytes = 0; }
        if failure == "invalid-root" { runtime.exported_function_roots.insert(usize::MAX); }
        let before = (runtime.function_arena.occupied, runtime.function_arena.collections,
            runtime.collection_entries, runtime.value_bytes, runtime.bindings, runtime.job_deadline,
            runtime.environments.iter().filter(|env| env.is_some()).count());
        assert!(runtime.collect_function_candidates_at_entry().is_err(), "{failure}");
        assert_eq!((runtime.function_arena.occupied, runtime.function_arena.collections,
            runtime.collection_entries, runtime.value_bytes, runtime.bindings, runtime.job_deadline,
            runtime.environments.iter().filter(|env| env.is_some()).count()), before, "{failure}");
        assert_eq!(slots(&runtime, map), 1, "{failure}");
        assert!(runtime.transient_registration_is_empty());
        if matches!(failure, "steps" | "late-steps" | "deadline") { assert_eq!(runtime.steps, 0); }
    }
}
