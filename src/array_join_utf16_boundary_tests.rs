//! Original exact-string, workspace, and capability boundaries for Array join.
use super::*;
use std::cell::{Cell, RefCell};

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("Join boundary checks must not read a native host");
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("Join boundary checks must not write a native host");
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("Join boundary checks must not call a native host");
    }
}

fn object_id(value: Value) -> usize {
    let Value::Object(id) = value else { panic!("Expected an ordinary object") };
    id
}

fn array(runtime: &mut Runtime, values: &[Value]) -> usize {
    object_id(runtime.array_from_values(values).unwrap())
}

fn empty_separator() -> Value { Value::String(String::new()) }

fn heap_state(runtime: &Runtime) -> (usize, usize, usize) {
    (runtime.objects.len(), runtime.properties, runtime.value_bytes)
}

fn assert_idle(runtime: &Runtime) {
    assert!(runtime.active_array_joins.is_empty());
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert!(runtime.transient_registration_is_empty());
}

fn assert_units(value: &Value, expected: &[u16]) {
    let Value::StringUnits(units) = value else { panic!("Expected exact raw units, got {value:?}") };
    assert_eq!(units, expected);
}

fn assert_scalar(value: &Value, expected: &str) {
    let Value::String(text) = value else { panic!("Expected canonical scalar text, got {value:?}") };
    assert_eq!(text, expected);
}

fn fatal(fault: Fault, message: &str) {
    assert!(fault.exception.is_none(), "Expected a fatal refusal: {fault:?}");
    assert!(fault.message.contains(message), "{fault:?}");
}

fn workspace_refusal(fault: Fault) {
    assert!(fault.exception.is_none(), "Workspace refusal must remain fatal: {fault:?}");
    assert!(fault.message.contains("workspace") || fault.message.contains("storage")
        || fault.message.contains("Retained") || fault.message.contains("retained"), "{fault:?}");
}

#[test]
fn join_utf16_scalar_prefix_expansion_does_not_reject_a_fitting_final_raw_result() {
    let mut runtime = Runtime::for_test();
    let piece = "\u{0800}".repeat(50);
    assert_eq!(piece.len(), 150);
    let id = array(&mut runtime, &[
        Value::String(piece.clone()), Value::String(piece), Value::StringUnits(vec![0xd800]),
    ]);
    runtime.limits.capacity.string_bytes = 256;
    let before = heap_state(&runtime);
    let value = runtime.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap();
    let mut expected = vec![0x0800; 100];
    expected.push(0xd800);
    assert_units(&value, &expected);
    assert_eq!(value.bytes(), 202);
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
}

#[test]
fn join_utf16_pending_raw_prefix_can_repair_across_an_empty_contribution() {
    let mut runtime = Runtime::for_test();
    let id = array(&mut runtime, &[
        Value::String("a".repeat(32)), Value::StringUnits(vec![0xd83d]),
        Value::Null, Value::StringUnits(vec![0xde00]),
    ]);
    runtime.limits.capacity.string_bytes = 48;
    let before = heap_state(&runtime);
    let value = runtime.array_join(id, 4, &empty_separator(), &mut NoHost).unwrap();
    assert_scalar(&value, &format!("{}😀", "a".repeat(32)));
    assert_eq!(value.bytes(), 36);
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
}

#[test]
fn join_utf16_final_raw_byte_limit_admits_exact_fit_and_refunds_refusal() {
    let mut runtime = Runtime::for_test();
    let fit = array(&mut runtime, &[Value::String("a".repeat(23)), Value::StringUnits(vec![0xd800])]);
    let oversized = array(&mut runtime, &[Value::String("a".repeat(24)), Value::StringUnits(vec![0xd800])]);
    runtime.limits.capacity.string_bytes = 48;
    let before = heap_state(&runtime);
    let value = runtime.array_join(fit, 2, &empty_separator(), &mut NoHost).unwrap();
    let mut expected = vec![b'a' as u16; 23];
    expected.push(0xd800);
    assert_units(&value, &expected);
    assert_eq!(value.bytes(), 48);
    assert_eq!(heap_state(&runtime), before);
    fatal(runtime.array_join(oversized, 2, &empty_separator(), &mut NoHost).unwrap_err(), "String limit");
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.begin_job();
    assert_units(&runtime.array_join(fit, 2, &empty_separator(), &mut NoHost).unwrap(), &expected);
    assert_eq!(heap_state(&runtime), before);
}

#[test]
fn join_utf16_final_scalar_byte_limit_uses_utf8_after_canonicalization() {
    let mut runtime = Runtime::for_test();
    let piece = "\u{0800}".repeat(50);
    let id = array(&mut runtime, &[Value::String(piece.clone()), Value::String(piece)]);
    let before = heap_state(&runtime);
    runtime.limits.capacity.string_bytes = 256;
    fatal(runtime.array_join(id, 2, &empty_separator(), &mut NoHost).unwrap_err(), "String limit");
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.begin_job();
    runtime.limits.capacity.string_bytes = 300;
    let value = runtime.array_join(id, 2, &empty_separator(), &mut NoHost).unwrap();
    assert_scalar(&value, &"\u{0800}".repeat(100));
    assert_eq!(value.bytes(), 300);
    assert_eq!(heap_state(&runtime), before);
}

#[test]
fn join_utf16_sparse_raw_repetition_preserves_pairs_at_repeat_boundaries() {
    let mut runtime = Runtime::for_test();
    let id = runtime.new_object(Some(4)).unwrap();
    let separator = Value::StringUnits(vec![0xdc00, 0xd800]);
    let before = heap_state(&runtime);
    runtime.limits.capacity.string_bytes = 12;
    let value = runtime.array_join(id, 4, &separator, &mut NoHost).unwrap();
    assert_units(&value, &[0xdc00, 0xd800, 0xdc00, 0xd800, 0xdc00, 0xd800]);
    assert_eq!(value.bytes(), 12);
    assert_eq!(heap_state(&runtime), before);
    runtime.limits.capacity.string_bytes = 11;
    fatal(runtime.array_join(id, 4, &separator, &mut NoHost).unwrap_err(), "String limit");
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.begin_job();
    runtime.limits.capacity.string_bytes = 12;
    assert_units(&runtime.array_join(id, 4, &separator, &mut NoHost).unwrap(),
        &[0xdc00, 0xd800, 0xdc00, 0xd800, 0xdc00, 0xd800]);
}

#[test]
fn join_utf16_sparse_empty_results_remain_scalar_without_visiting_every_hole() {
    let mut runtime = Runtime::for_test();
    let holes = runtime.new_object(Some(8192)).unwrap();
    let zero = runtime.new_object(Some(0)).unwrap();
    let one = runtime.new_object(Some(1)).unwrap();
    let before = heap_state(&runtime);
    let steps = runtime.steps;
    assert_scalar(&runtime.array_join(holes, 8192, &empty_separator(), &mut NoHost).unwrap(), "");
    assert!(steps - runtime.steps < 8192 / 2, "Hole-only repetition must retain its bounded fast path");
    let raw = Value::StringUnits(vec![0xd800]);
    assert_scalar(&runtime.array_join(zero, 0, &raw, &mut NoHost).unwrap(), "");
    assert_scalar(&runtime.array_join(one, 1, &raw, &mut NoHost).unwrap(), "");
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
}

fn budget_array() -> (Runtime, usize) {
    let mut runtime = Runtime::for_test();
    let id = array(&mut runtime, &[
        Value::String("prefix".repeat(8)), Value::StringUnits(vec![0xd800]),
        Value::String("tail".repeat(8)),
    ]);
    runtime.begin_job();
    (runtime, id)
}

#[test]
fn join_utf16_late_copy_refusal_refunds_workspace_without_refunding_work() {
    let (mut measured, id) = budget_array();
    let initial = measured.copy_bytes;
    measured.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap();
    let required = initial - measured.copy_bytes;
    assert!(required > 1);
    let (mut runtime, id) = budget_array();
    let before = heap_state(&runtime);
    let deadline = runtime.job_deadline;
    runtime.copy_bytes = required - 1;
    fatal(runtime.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap_err(), "String/copy work limit");
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!(runtime.job_deadline, deadline);
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.begin_job();
    let value = runtime.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap();
    let mut expected: Vec<u16> = "prefix".repeat(8).encode_utf16().collect();
    expected.push(0xd800);
    expected.extend("tail".repeat(8).encode_utf16());
    assert_units(&value, &expected);
    assert_eq!(heap_state(&runtime), before);
}

#[test]
fn join_utf16_late_step_refusal_clears_cycle_state_and_owned_storage() {
    let (mut measured, id) = budget_array();
    let initial = measured.steps;
    measured.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap();
    let required = initial - measured.steps;
    assert!(required > 1);
    let (mut runtime, id) = budget_array();
    let before = heap_state(&runtime);
    let work = runtime.copy_bytes;
    runtime.steps = required - 1;
    fatal(runtime.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap_err(), "Execution step limit");
    assert_eq!(runtime.steps, 0);
    assert!(runtime.copy_bytes < work);
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.begin_job();
    assert!(runtime.array_join(id, 3, &empty_separator(), &mut NoHost).is_ok());
    assert_eq!(heap_state(&runtime), before);
}

#[test]
fn join_utf16_actual_array_workspace_respects_existing_retained_capacity() {
    let (mut runtime, id) = budget_array();
    let before = heap_state(&runtime);
    let original_limit = runtime.limits.heap.retained_values;
    let work = runtime.copy_bytes;
    runtime.limits.heap.retained_values = runtime.value_bytes;
    workspace_refusal(runtime.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap_err());
    assert_eq!(heap_state(&runtime), before);
    assert!(runtime.copy_bytes < work);
    assert_idle(&runtime);
    runtime.limits.heap.retained_values = original_limit;
    runtime.begin_job();
    assert!(runtime.array_join(id, 3, &empty_separator(), &mut NoHost).is_ok());
    assert_eq!(heap_state(&runtime), before);
}

#[test]
fn join_utf16_generic_array_like_workspace_and_final_raw_cap_share_the_same_guards() {
    let mut runtime = Runtime::for_test();
    let id = runtime.new_object(None).unwrap();
    runtime.set_property(id, "length".into(), Value::Number(3.0)).unwrap();
    runtime.set_property(id, "0".into(), Value::StringUnits(vec![0xd800])).unwrap();
    runtime.set_property(id, "1".into(), Value::String("a".into())).unwrap();
    runtime.set_property(id, "2".into(), Value::Null).unwrap();
    let separator = Value::StringUnits(vec![0xdc00]);
    let before = heap_state(&runtime);
    let original_limit = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = runtime.value_bytes;
    workspace_refusal(runtime.array_like_join(id, 3, &separator, &mut NoHost).unwrap_err());
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.limits.heap.retained_values = original_limit;
    runtime.limits.capacity.string_bytes = 8;
    runtime.begin_job();
    assert_units(&runtime.array_like_join(id, 3, &separator, &mut NoHost).unwrap(),
        &[0xd800, 0xdc00, b'a' as u16, 0xdc00]);
    assert_eq!(heap_state(&runtime), before);
    runtime.limits.capacity.string_bytes = 7;
    fatal(runtime.array_like_join(id, 3, &separator, &mut NoHost).unwrap_err(), "String limit");
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
}

thread_local! {
    static JOIN_IDS: Cell<(usize, usize)> = const { Cell::new((usize::MAX, usize::MAX)) };
    static JOIN_OBSERVATIONS: RefCell<Vec<(&'static str, usize, bool, bool)>> = const { RefCell::new(Vec::new()) };
}

fn inspect_nested_join_workspace(runtime: &Runtime, callee: usize, _: &[Value], _: &Value) {
    let name = runtime.objects[runtime.function_properties[callee]].properties["name"].value.as_value().unwrap();
    let Value::String(name) = name else { return; };
    let phase = match name.as_str() {
        "joinOuterProbe" => "outer",
        "joinInnerProbe" => "inner",
        "joinAfterProbe" => "after",
        _ => return,
    };
    let (outer, inner) = JOIN_IDS.with(Cell::get);
    JOIN_OBSERVATIONS.with(|observations| observations.borrow_mut().push((
        phase, runtime.value_bytes, runtime.active_array_joins.contains(&outer),
        runtime.active_array_joins.contains(&inner),
    )));
}

#[test]
fn join_utf16_nested_callbacks_keep_outer_and_inner_workspace_charged_together() {
    let mut runtime = Runtime::for_test();
    let prefix = "p".repeat(32 * 1024);
    runtime.define_global("joinPrefix", Value::String(prefix.clone())).unwrap();
    runtime.execute(r#"
function joinOuterProbe(){}function joinInnerProbe(){}function joinAfterProbe(){}
var joinInner=[joinPrefix,{toString:function(){joinInnerProbe();return '\ud800';}}];
var joinOuter=[joinPrefix,{toString:function(){joinOuterProbe();joinInner.join('');joinAfterProbe();return '';}}];
"#, &mut NoHost).unwrap();
    let outer = object_id(runtime.binding_value(runtime.global_environment, "joinOuter").unwrap());
    let inner = object_id(runtime.binding_value(runtime.global_environment, "joinInner").unwrap());
    // Materialize the same native methods before measuring active workspace.
    assert_scalar(&runtime.array_join(outer, 2, &empty_separator(), &mut NoHost).unwrap(), &prefix);
    runtime.begin_job();
    let before = runtime.value_bytes;
    JOIN_IDS.with(|ids| ids.set((outer, inner)));
    JOIN_OBSERVATIONS.with(|observations| observations.borrow_mut().clear());
    runtime.transient_roots.script_input_observer = Some(inspect_nested_join_workspace);
    let value = runtime.array_join(outer, 2, &empty_separator(), &mut NoHost).unwrap();
    runtime.transient_roots.script_input_observer = None;
    assert_scalar(&value, &prefix);
    let observations = JOIN_OBSERVATIONS.with(|observations| observations.borrow().clone());
    assert_eq!(observations.len(), 3);
    assert_eq!((observations[0].0, observations[0].2, observations[0].3), ("outer", true, false));
    assert_eq!((observations[1].0, observations[1].2, observations[1].3), ("inner", true, true));
    assert_eq!((observations[2].0, observations[2].2, observations[2].3), ("after", true, false));
    assert!(observations[0].1 >= before + prefix.len());
    assert!(observations[1].1 >= before + 2 * prefix.len());
    assert!(observations[2].1 >= before + prefix.len());
    assert!(runtime.value_bytes < observations[2].1, "Returning must release the outer join's owned storage");
    assert_idle(&runtime);
}

#[test]
fn join_utf16_callback_publications_survive_throw_while_workspace_is_refunded() {
    let mut runtime = Runtime::for_test();
    runtime.execute(r#"
var joinSaved={},joinMarker={},joinLater=0;
var joinSource=['prefix',{toString:function(){joinSaved['\ud800']='\udfff';throw joinMarker;}},
    {toString:function(){joinLater++;return 'tail';}}];
"#, &mut NoHost).unwrap();
    let id = object_id(runtime.binding_value(runtime.global_environment, "joinSource").unwrap());
    let saved = object_id(runtime.binding_value(runtime.global_environment, "joinSaved").unwrap());
    let marker = object_id(runtime.binding_value(runtime.global_environment, "joinMarker").unwrap());
    let retained = runtime.value_bytes;
    let fault = runtime.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap_err();
    assert!(matches!(fault.exception.as_deref(),
        Some(super::super::Exception::Value(Value::Object(value))) if *value == marker), "{fault:?}");
    assert_units(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
    assert_eq!(runtime.binding_value(runtime.global_environment, "joinLater").unwrap().to_number(), 0.0);
    assert!(runtime.value_bytes >= retained + 4, "Published key/value bytes must not be rolled back with workspace");
    assert_idle(&runtime);
    runtime.begin_job();
    runtime.set_property(id, "1".into(), Value::StringUnits(vec![0xd800])).unwrap();
    let value = runtime.array_join(id, 3, &empty_separator(), &mut NoHost).unwrap();
    let mut expected: Vec<u16> = "prefix".encode_utf16().collect();
    expected.push(0xd800);
    expected.extend("tail".encode_utf16());
    assert_units(&value, &expected);
    assert_eq!(runtime.binding_value(runtime.global_environment, "joinLater").unwrap().to_number(), 1.0);
    assert_idle(&runtime);
}

#[test]
fn join_utf16_actual_array_native_refusal_precedes_permission_and_conversion_hooks() {
    struct Native { permissions: Cell<usize>, reads: usize, calls: usize }
    impl Host for Native {
        fn primitive_conversion(&self, _: u32) -> bool {
            self.permissions.set(self.permissions.get() + 1);
            true
        }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            self.reads += 1;
            Ok(Value::Undefined)
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("Join must not write a native host")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            self.calls += 1;
            Ok(Value::String("native".into()))
        }
    }
    let mut runtime = Runtime::for_test();
    let mut native = Native { permissions: Cell::new(0), reads: 0, calls: 0 };
    let id = array(&mut runtime, &[Value::StringUnits(vec![0xd83d]), Value::Host(17)]);
    let before = heap_state(&runtime);
    fatal(runtime.array_join(id, 2, &empty_separator(), &mut native).unwrap_err(),
        "cannot implicitly convert host objects");
    assert_eq!((native.permissions.get(), native.reads, native.calls), (0, 0, 0));
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.begin_job();
    runtime.set_property(id, "1".into(), Value::StringUnits(vec![0xde00])).unwrap();
    assert_scalar(&runtime.array_join(id, 2, &empty_separator(), &mut NoHost).unwrap(), "😀");
    assert_idle(&runtime);
}

#[test]
fn join_utf16_fatal_output_refusal_skips_page_cleanup_and_preserves_the_source() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("joinPrefix", Value::String("a".repeat(128))).unwrap();
    runtime.execute("var joinCaught=false,joinFinalized=false,joinSource=[joinPrefix,'\\ud800'];", &mut NoHost).unwrap();
    runtime.limits.capacity.string_bytes = 256;
    let error = runtime.execute(
        "try{joinSource.join('');}catch(error){joinCaught=true;}finally{joinFinalized=true;}",
        &mut NoHost,
    ).unwrap_err();
    assert!(error.contains("String limit"), "{error}");
    assert_idle(&runtime);
    runtime.begin_job();
    assert_eq!(runtime.execute(
        "!joinCaught&&!joinFinalized&&joinSource.length===2&&joinSource[0]===joinPrefix&&joinSource[1]==='\\ud800';",
        &mut NoHost,
    ).unwrap().to_text(), "true");
    assert_idle(&runtime);
}

#[test]
fn join_utf16_nested_raw_arrays_keep_supported_depth_and_clear_refused_nesting() {
    let mut runtime = Runtime::for_test();
    let mut value = Value::StringUnits(vec![0xd800]);
    for _ in 0..24 { value = runtime.array_from_values(&[value]).unwrap(); }
    let shallow = object_id(value.clone());
    assert_units(&runtime.array_join(shallow, 1, &empty_separator(), &mut NoHost).unwrap(), &[0xd800]);
    assert_idle(&runtime);
    for _ in 24..40 { value = runtime.array_from_values(&[value]).unwrap(); }
    let deep = object_id(value);
    let before = heap_state(&runtime);
    fatal(runtime.array_join(deep, 1, &empty_separator(), &mut NoHost).unwrap_err(), "nesting limit");
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.begin_job();
    assert_units(&runtime.array_join(shallow, 1, &empty_separator(), &mut NoHost).unwrap(), &[0xd800]);
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
}
