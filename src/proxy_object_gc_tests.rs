//! Private record/GC controls. Public constructor and trap tests are separate.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected host get")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected host set")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected host call")
    }
}
fn object(runtime: &Runtime, name: &str) -> usize {
    let Value::Object(id) = runtime.binding_value_unmetered_for_proxy_test(name) else {
        panic!("object fixture")
    };
    id
}

impl Runtime {
    // Do not export-pin an internal record merely to give it a script binding.
    fn binding_value_unmetered_for_proxy_test(&self, name: &str) -> Value {
        self.environment(self.global_environment).bindings[name]
            .value
            .as_ref()
            .unwrap()
            .clone()
    }
}

fn bind_proxy(runtime: &mut Runtime, target: Value, handler: Value) -> usize {
    let proxy = runtime.new_ordinary_proxy(&target, &handler).unwrap();
    let Value::Object(id) = proxy else {
        panic!("proxy record")
    };
    runtime
        .store_binding(runtime.global_environment, "proxy", proxy, false)
        .unwrap();
    id
}
fn slots(runtime: &Runtime, id: usize) -> usize {
    runtime.objects[id]
        .collection
        .as_ref()
        .unwrap()
        .weak_slot_count()
}

#[test]
fn proxy_gc_record_allocation_is_distinct_charged_and_accepts_real_class_handler() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute(
            "class Handler { get(){return 7;} }var target={},handler=new Handler(),proxy;",
            &mut NoHost,
        )
        .unwrap();
    let target = object(&runtime, "target");
    let handler = object(&runtime, "handler");
    let before = (
        runtime.objects.len(),
        runtime.functions.len(),
        runtime.properties,
        runtime.value_bytes,
        runtime.copy_bytes,
    );
    let first = runtime
        .new_ordinary_proxy(&Value::Object(target), &Value::Object(handler))
        .unwrap();
    let second = runtime
        .new_ordinary_proxy(&Value::Object(target), &Value::Object(handler))
        .unwrap();
    let (Value::Object(first), Value::Object(second)) = (first, second) else {
        panic!("ordinary record identity")
    };
    assert_eq!((first, second), (before.0, before.0 + 1));
    assert_ne!(first, target);
    assert_ne!(first, handler);
    let record = proxy_objects::ProxyData { target, handler };
    for id in [first, second] {
        let object = &runtime.objects[id];
        assert_eq!(object.proxy, Some(record));
        assert!(
            object.prototype.is_none(),
            "visible prototype must use the real proxy algorithm"
        );
        assert!(
            object.length.is_none()
                && object.properties.is_empty()
                && object.symbol_properties.is_empty()
        );
        assert!(
            !Runtime::plain_array_storage(object),
            "proxy is not a plain-storage fallback"
        );
    }
    let bytes = std::mem::size_of::<proxy_objects::ProxyData>() * 2;
    assert_eq!(
        (
            runtime.objects.len(),
            runtime.functions.len(),
            runtime.properties,
            runtime.value_bytes,
            runtime.copy_bytes
        ),
        (
            before.0 + 2,
            before.1,
            before.2,
            before.3 + bytes,
            before.4 - bytes
        )
    );
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn proxy_gc_allocation_guards_do_not_coerce_endpoints_or_publish_failed_records() {
    let mut runtime = Runtime::for_test();
    runtime.execute(r#"
        var effects=0,target={},handler={},proxy,callback=function(){return 1;},symbol=Symbol('invalid');
        Object.defineProperty(target,'constructor',{get:function(){effects++;throw 'author classification';}});
        var exotic=[[],new Date(0),/x/,new Map(),new WeakMap(),new Uint8Array(1),new String('x')];
    "#,&mut NoHost).unwrap();
    let target = Value::Object(object(&runtime, "target"));
    let handler = Value::Object(object(&runtime, "handler"));
    let mut rejected = vec![
        Value::Undefined,
        Value::Null,
        Value::Bool(true),
        Value::Number(1.0),
        Value::String("x".into()),
        Value::Host(19),
        Value::Object(usize::MAX),
        runtime.binding_value_unmetered_for_proxy_test("symbol"),
        runtime.binding_value_unmetered_for_proxy_test("callback"),
    ];
    let array = object(&runtime, "exotic");
    for index in 0..7 {
        rejected.push(runtime.own_property(array, &index.to_string()).unwrap());
    }
    let existing = runtime.new_ordinary_proxy(&target, &handler).unwrap();
    rejected.push(existing);
    let before = (
        runtime.objects.len(),
        runtime.functions.len(),
        runtime.properties,
        runtime.value_bytes,
    );
    for invalid in rejected {
        assert!(runtime.new_ordinary_proxy(&invalid, &handler).is_err());
        assert!(runtime.new_ordinary_proxy(&target, &invalid).is_err());
        assert_eq!(
            (
                runtime.objects.len(),
                runtime.functions.len(),
                runtime.properties,
                runtime.value_bytes
            ),
            before
        );
    }
    assert_eq!(
        runtime
            .binding_value_unmetered_for_proxy_test("effects")
            .to_number(),
        0.0
    );
    for mode in ["objects", "retained", "copy", "steps"] {
        runtime.begin_job();
        let heap = runtime.limits.heap;
        match mode {
            "objects" => runtime.limits.heap.objects = runtime.objects.len(),
            "retained" => runtime.limits.heap.retained_values = runtime.value_bytes,
            "copy" => runtime.copy_bytes = 0,
            _ => runtime.steps = 0,
        }
        assert!(
            runtime.new_ordinary_proxy(&target, &handler).is_err(),
            "{mode}"
        );
        assert_eq!(
            (
                runtime.objects.len(),
                runtime.functions.len(),
                runtime.properties,
                runtime.value_bytes
            ),
            before,
            "{mode}"
        );
        runtime.limits.heap = heap;
    }
    runtime.begin_job();
    assert!(matches!(
        runtime.new_ordinary_proxy(&target, &handler).unwrap(),
        Value::Object(_)
    ));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn proxy_gc_rooted_record_retains_target_handler_prototypes_and_callbacks() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(r#"
        var calls=0,proxy;
        var target=(function(n){var object={};object.fn=function(){calls++;return n;};return object;})(71);
        var handler=(function(n){class Handler{get(){calls++;return n;}}return new Handler();})(73);
    "#,&mut NoHost).unwrap();
    let target = object(&runtime, "target");
    let handler = object(&runtime, "handler");
    let Value::Function(target_fn) = runtime
        .get_property_with_host(target, "fn", &mut NoHost)
        .unwrap()
    else {
        panic!("target callback")
    };
    let Value::Function(handler_fn) = runtime
        .get_property_with_host(handler, "get", &mut NoHost)
        .unwrap()
    else {
        panic!("handler callback")
    };
    let proxy = bind_proxy(&mut runtime, Value::Object(target), Value::Object(handler));
    runtime
        .execute("target=null;handler=null;0;", &mut NoHost)
        .unwrap();
    for graph in [
        runtime.mark_root_graph(&[], true, true).unwrap(),
        runtime.mark_scope_candidate_comparison(true).unwrap(),
        runtime.mark_persistent_functions(&[], true).unwrap(),
    ] {
        assert!(graph.objects[proxy] && graph.objects[target] && graph.objects[handler]);
        assert!(graph.functions[target_fn] && graph.functions[handler_fn]);
    }
    assert_eq!(
        runtime
            .binding_value_unmetered_for_proxy_test("calls")
            .to_number(),
        0.0
    );
    assert!(runtime.retire_proven_inactive_candidate(target_fn).is_err());
    assert!(runtime.function_is_occupied(target_fn) && runtime.function_is_occupied(handler_fn));
    assert_eq!(
        runtime
            .call_value(
                Value::Function(target_fn),
                &[],
                Value::Object(target),
                &mut NoHost
            )
            .unwrap()
            .to_number(),
        71.0
    );
    assert_eq!(
        runtime
            .call_value(
                Value::Function(handler_fn),
                &[],
                Value::Object(handler),
                &mut NoHost
            )
            .unwrap()
            .to_number(),
        73.0
    );
    assert!(runtime.transient_registration_is_empty());
}

fn weak_cycle() -> (Runtime, usize, usize, usize, usize, usize, usize) {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(r#"
        var map=new WeakMap(),members=new WeakSet(),proxy;
        function make(){var key={},target={back:key},handler={};handler.get=function(){return key;};return [key,target,handler];}
        var parts=make();
    "#,&mut NoHost).unwrap();
    let parts = object(&runtime, "parts");
    let Value::Object(key) = runtime.own_property(parts, "0").unwrap() else {
        panic!("weak key")
    };
    let target = runtime.own_property(parts, "1").unwrap();
    let handler = runtime.own_property(parts, "2").unwrap();
    let Value::Object(target_id) = target else {
        panic!("target")
    };
    let Value::Object(handler_id) = handler else {
        panic!("handler")
    };
    let Value::Function(callback) = runtime.own_property(handler_id, "get").unwrap() else {
        panic!("callback")
    };
    let proxy = bind_proxy(&mut runtime, target, handler);
    runtime
        .execute(
            "map.set(parts[0],proxy);members.add(proxy);parts=null;proxy=null;0;",
            &mut NoHost,
        )
        .unwrap();
    let map = object(&runtime, "map");
    let members = object(&runtime, "members");
    (runtime, map, members, key, proxy, target_id, callback)
}

#[test]
fn proxy_gc_weak_value_back_to_key_cycle_does_not_seed_record_or_callbacks() {
    let (mut runtime, map, members, key, proxy, target, callback) = weak_cycle();
    assert_eq!((slots(&runtime, map), slots(&runtime, members)), (1, 1));
    for graph in [
        runtime.mark_root_graph(&[], true, true).unwrap(),
        runtime.mark_scope_candidate_comparison(true).unwrap(),
        runtime.mark_persistent_functions(&[], true).unwrap(),
    ] {
        assert!(
            !graph.objects[key]
                && !graph.objects[proxy]
                && !graph.objects[target]
                && !graph.functions[callback],
            "weak value-to-key cycle cannot seed itself"
        );
    }
    let retained = runtime.value_bytes;
    runtime.retire_proven_inactive_candidate(callback).unwrap();
    assert_eq!(
        (
            slots(&runtime, map),
            slots(&runtime, members),
            runtime.collection_entries
        ),
        (0, 0, 0)
    );
    assert!(!runtime.function_is_occupied(callback));
    assert!(runtime.value_bytes <= retained);
    // The append-only record keeps its own fixed metadata charge. Dead weak
    // entries must be removed before a callback slot can be reused.
    assert!(runtime.objects[proxy].proxy.is_some());
    let replacement = runtime
        .native_function(Builtin::IsNaN, "replacement")
        .unwrap();
    assert!(matches!(replacement,Value::Function(id) if id==callback));
    assert_eq!((slots(&runtime, map), slots(&runtime, members)), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn proxy_gc_metered_edges_and_weak_sweep_failure_preserve_previous_state() {
    let mut runtime = Runtime::for_capacity_test();
    runtime
        .execute("var target={},handler={},proxy;", &mut NoHost)
        .unwrap();
    let target = object(&runtime, "target");
    let handler = object(&runtime, "handler");
    let proxy = bind_proxy(&mut runtime, Value::Object(target), Value::Object(handler));
    let mut meter = Meter {
        remaining: Some(usize::MAX),
        deadline: runtime.job_deadline,
    };
    runtime
        .trace_root_graph_metered(&[], true, true, false, &mut meter)
        .unwrap();
    let with_record = usize::MAX - meter.remaining.unwrap();
    let record = runtime.objects[proxy].proxy.take();
    let mut meter = Meter {
        remaining: Some(usize::MAX),
        deadline: runtime.job_deadline,
    };
    runtime
        .trace_root_graph_metered(&[], true, true, false, &mut meter)
        .unwrap();
    runtime.objects[proxy].proxy = record;
    assert_eq!(
        with_record,
        usize::MAX - meter.remaining.unwrap() + 2,
        "two fixed endpoint edges; proxy-free roots keep previous charges"
    );

    let (mut runtime, map, members, _, proxy, _, callback) = weak_cycle();
    let before = (
        runtime.value_bytes,
        runtime.collection_entries,
        runtime.function_arena.occupied,
        runtime.bindings,
    );
    runtime.steps = 0;
    assert!(runtime.retire_proven_inactive_candidate(callback).is_err());
    assert_eq!(
        (
            runtime.value_bytes,
            runtime.collection_entries,
            runtime.function_arena.occupied,
            runtime.bindings
        ),
        before
    );
    assert_eq!((slots(&runtime, map), slots(&runtime, members)), (1, 1));
    assert!(runtime.function_is_occupied(callback) && runtime.objects[proxy].proxy.is_some());
    runtime.begin_job();
    runtime.retire_proven_inactive_candidate(callback).unwrap();
    assert_eq!((slots(&runtime, map), slots(&runtime, members)), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}
