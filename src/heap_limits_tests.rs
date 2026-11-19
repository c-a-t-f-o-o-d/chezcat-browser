//! Actual-default capacity witnesses and explicit small-profile boundary tests.
//! These tests never change native stack sizes or the browser's work budgets.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("No native reads in heap fixture.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("No native writes in heap fixture.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("No native calls in heap fixture.".into())
    }
}

#[test]
fn heap_profile_defaults_and_resets_are_explicit() {
    let mut runtime = Runtime::new();
    assert_eq!(runtime.limits.heap, limits::BROWSER_HEAP);
    assert_eq!(Runtime::default().limits.heap, limits::BROWSER_HEAP);
    assert_eq!(Runtime::for_test().limits.heap, limits::REGRESSION_HEAP);
    assert_eq!(limits::BROWSER_HEAP.object_properties, 1_048_576);
    assert_eq!(limits::BROWSER_HEAP.properties, 16_777_216);
    assert_eq!(limits::BROWSER_HEAP.objects, 1_048_576);
    assert_eq!(limits::BROWSER_HEAP.array_length, 16_777_216);
    assert_eq!(limits::BROWSER_HEAP.retained_values, 512 * 1024 * 1024);
    let counts = (runtime.objects.len(), runtime.properties, runtime.value_bytes);
    assert!(counts.0 < 4096 && counts.1 < 4096 && counts.2 < 2 * 1024 * 1024);
    runtime.constrain_job(1, 1, 1, Instant::now() + Duration::from_secs(1));
    runtime.begin_job();
    assert_eq!(runtime.limits.heap, limits::BROWSER_HEAP);
    assert_eq!((runtime.objects.len(), runtime.properties, runtime.value_bytes), counts);
    assert_eq!(runtime.limits.test_call_depth, None);
}

#[test]
fn heap_browser_array_and_enumeration_exceed_the_old_ceiling() {
    let mut runtime = Runtime::new();
    runtime.begin_job();
    let result = runtime.execute(include_str!("../tools/fixtures/heap-capacity.js"), &mut NoHost).unwrap();
    assert_eq!(result.to_text(), "ok");
    assert!(runtime.properties > limits::REGRESSION_HEAP.properties);
    assert!(runtime.properties < runtime.limits.heap.properties);
}

#[test]
fn heap_browser_object_and_payload_capacity_exceed_old_document_caps() {
    let mut runtime = Runtime::new();
    runtime.begin_job();
    // Actual allocations, not simulated counts: more objects and retained
    // payload than the old defaults. No single string exceeds its old bound.
    while runtime.objects.len() <= limits::REGRESSION_HEAP.objects {
        runtime.new_object(None).unwrap();
    }
    let holder = runtime.new_object(None).unwrap();
    for i in 0..20 {
        runtime.set_property(holder, i.to_string(), Value::String("x".repeat(MAX_STRING_BYTES))).unwrap();
    }
    assert!(runtime.value_bytes > limits::REGRESSION_HEAP.retained_values);
    assert!(runtime.value_bytes < runtime.limits.heap.retained_values);
    let bytes = runtime.value_bytes;
    runtime.begin_job();
    assert_eq!(runtime.value_bytes, bytes);
    assert_eq!(runtime.get_property(holder, "19").unwrap().to_text().len(), MAX_STRING_BYTES);
}

#[test]
fn heap_string_and_symbol_properties_share_atomic_capacity_and_redaction() {
    let mut runtime = Runtime::new();
    let Value::Symbol(symbol) = runtime.execute("Symbol('private-description');", &mut NoHost).unwrap() else {
        panic!("Expected a symbol identity");
    };
    let id = runtime.new_object(None).unwrap();
    runtime.limits.heap.object_properties = 3;
    runtime.set_property(id, "private-name".into(), Value::String("private-value".into())).unwrap();
    runtime.set_property(id, "second".into(), Value::Number(2.0)).unwrap();
    runtime.store_symbol_property(id, symbol, Value::Number(3.0), None).unwrap();
    let before = (runtime.properties, runtime.value_bytes, runtime.objects[id].next_order);
    let error = runtime.set_property(id, "rejected-private-name".into(), Value::Number(4.0)).unwrap_err();
    assert!(error.exception.is_none());
    assert!(error.message.contains("3 per object / 16777216 per document"));
    assert!(error.message.contains("Target properties: 2 string, 1 symbol"));
    for secret in ["private-name", "private-value", "private-description"] {
        assert!(!error.message.contains(secret));
    }
    assert!(error.message.len() < 512);
    assert_eq!((runtime.properties, runtime.value_bytes, runtime.objects[id].next_order), before);
    // Updates at capacity remain legal. Removal returns a slot, but no work.
    runtime.store_symbol_property(id, symbol, Value::Number(5.0), None).unwrap();
    let work = runtime.copy_bytes;
    runtime.remove_property(id, "second").unwrap();
    assert!(runtime.copy_bytes <= work);
    runtime.set_property(id, "replacement".into(), Value::Number(6.0)).unwrap();
    assert_eq!(runtime.properties, before.0);
}

#[test]
fn heap_document_cap_is_independent_from_target_and_released_on_delete() {
    let mut runtime = Runtime::new();
    let first = runtime.new_object(None).unwrap();
    let second = runtime.new_object(None).unwrap();
    runtime.limits.heap.properties = runtime.properties + 1;
    runtime.set_property(first, "one".into(), Value::Number(1.0)).unwrap();
    let before = (runtime.properties, runtime.value_bytes);
    let error = runtime.set_property(second, "two".into(), Value::Number(2.0)).unwrap_err();
    assert!(error.message.contains("Object property limit reached"));
    assert!(error.message.contains("Target properties: 0 string, 0 symbol"));
    assert!(runtime.objects[second].properties.is_empty());
    assert_eq!((runtime.properties, runtime.value_bytes), before);
    runtime.remove_property(first, "one").unwrap();
    runtime.set_property(second, "two".into(), Value::Number(2.0)).unwrap();
    assert_eq!(runtime.properties, runtime.limits.heap.properties);
}

#[test]
fn heap_sparse_maximum_does_not_allocate_holes_and_overflow_is_atomic() {
    let mut runtime = Runtime::new();
    let cap = runtime.limits.heap.array_length;
    let before = runtime.properties;
    let id = runtime.new_object(Some(cap)).unwrap();
    assert_eq!(runtime.properties, before);
    runtime.set_property(id, (cap - 1).to_string(), Value::Number(7.0)).unwrap();
    assert_eq!(runtime.objects[id].properties.len(), 1);
    let retained = runtime.value_bytes;
    assert!(runtime.set_property(id, cap.to_string(), Value::Number(8.0)).unwrap_err().message.contains(&cap.to_string()));
    assert_eq!(runtime.objects[id].length, Some(cap));
    assert_eq!(runtime.value_bytes, retained);
    assert!(runtime.apply_array_length(id, Some(cap + 1), None, true).is_err());
    assert_eq!(runtime.objects[id].length, Some(cap));
    assert!(runtime.apply_array_length(id, Some(0), None, true).unwrap());
    assert!(runtime.objects[id].properties.is_empty());
    assert_eq!(runtime.properties, before);
}

#[test]
fn heap_payload_and_object_limits_remain_finite_and_do_not_reserve_storage() {
    let mut runtime = Runtime::new();
    let id = runtime.new_object(None).unwrap();
    runtime.limits.heap.retained_values = runtime.value_bytes + 4;
    runtime.set_property(id, "a".into(), Value::String("123".into())).unwrap();
    let before = (runtime.properties, runtime.value_bytes);
    let error = runtime.set_property(id, "b".into(), Value::Number(0.0)).unwrap_err();
    assert!(error.message.contains("Retained value limit reached"));
    assert!(error.exception.is_none());
    assert_eq!((runtime.properties, runtime.value_bytes), before);
    runtime.set_property(id, "a".into(), Value::String("".into())).unwrap();
    assert_eq!(runtime.value_bytes, before.1 - 3);
    runtime.limits.heap.objects = runtime.objects.len();
    let count = runtime.objects.len();
    assert!(runtime.new_object(None).unwrap_err().message.contains("Object identity limit reached"));
    assert_eq!(runtime.objects.len(), count);
}

#[test]
fn heap_property_fault_cancels_microtasks_without_catch_finally_or_late_effects() {
    let mut runtime = Runtime::new();
    runtime.execute(concat!(
        "var target={},caught=false,finished=false,later=false;",
        "Promise.resolve().then(function(){try{target.x=1;}catch(e){caught=true;}",
        "finally{finished=true;}});",
        "Promise.resolve().then(function(){later=true;});"
    ), &mut NoHost).unwrap();
    let original = runtime.limits.heap.properties;
    runtime.limits.heap.properties = runtime.properties;
    let error = runtime.microtask_checkpoint(&mut NoHost).unwrap_err();
    assert!(error.contains("Object property limit reached"), "{error}");
    assert!(!runtime.has_microtasks());
    runtime.limits.heap.properties = original;
    runtime.begin_job();
    assert_eq!(runtime.execute("!caught&&!finished&&!later&&!target.hasOwnProperty('x');", &mut NoHost).unwrap().to_text(), "true");
    assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
    assert_eq!(runtime.execute("target.x=9;target.x;", &mut NoHost).unwrap().to_text(), "9");
}

#[test]
fn heap_old_small_profile_still_exercises_the_original_property_guard() {
    let mut runtime = Runtime::for_test();
    let error = runtime.execute("var a=[];for(var i=0;i<=4096;i++)a[i]=i;", &mut NoHost).unwrap_err();
    assert!(error.contains("4096 per object / 16384 per document"), "{error}");
    assert_eq!(runtime.limits.heap, limits::REGRESSION_HEAP);
    runtime.begin_job();
    assert_eq!(runtime.execute("a.length===4096&&!a.hasOwnProperty(4096);", &mut NoHost).unwrap().to_text(), "true");
}
