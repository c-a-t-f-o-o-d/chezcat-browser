//! Original output, scratch, and callback boundaries for exact JSON indentation.
use super::*;
use std::cell::RefCell;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("JSON output boundary checks must not read a native host");
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("JSON output boundary checks must not write a native host");
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("JSON output boundary checks must not call a native host");
    }
}

fn object_id(value: Value) -> usize {
    let Value::Object(id) = value else { panic!("Expected an ordinary object") };
    id
}

fn raw_gap() -> Value { Value::StringUnits(vec![0xd800]) }

fn stringify(runtime: &mut Runtime, value: &Value, gap: Value) -> Result<Value, Fault> {
    runtime.json_stringify(&[value.clone(), Value::Undefined, gap], &mut NoHost)
}

fn assert_raw(value: &Value, expected: &[u16]) {
    let Value::StringUnits(units) = value else { panic!("Expected exact raw output, got {value:?}") };
    assert_eq!(units, expected);
}

fn assert_scalar(value: &Value, expected: &str) {
    let Value::String(text) = value else { panic!("Expected canonical scalar output, got {value:?}") };
    assert_eq!(text, expected);
}

fn assert_idle(runtime: &Runtime, scratch: usize) {
    assert_eq!(runtime.json_scratch_bytes, scratch);
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert!(runtime.transient_registration_is_empty());
}

fn fatal(fault: Fault, message: &str) {
    assert!(fault.exception.is_none(), "Expected a fatal resource refusal: {fault:?}");
    assert!(fault.message.contains(message), "{fault:?}");
}

fn number_array() -> (Runtime, Value) {
    let mut runtime = Runtime::for_test();
    let value = runtime.array_from_values(&[Value::Number(7.0)]).unwrap();
    assert_scalar(&runtime.json_stringify(std::slice::from_ref(&value), &mut NoHost).unwrap(), "[7]");
    runtime.begin_job();
    (runtime, value)
}

#[test]
fn stringify_utf16_raw_output_admits_the_exact_byte_cap_and_recovers_after_one_less() {
    let (mut runtime, value) = number_array();
    let expected = [0x5b, 0x0a, 0xd800, 0x37, 0x0a, 0x5d];
    runtime.limits.capacity.string_bytes = expected.len() * 2;
    let output = stringify(&mut runtime, &value, raw_gap()).unwrap();
    assert_raw(&output, &expected);
    assert_eq!(output.bytes(), runtime.limits.capacity.string_bytes);
    assert_idle(&runtime, 0);

    runtime.limits.capacity.string_bytes -= 1;
    fatal(stringify(&mut runtime, &value, raw_gap()).unwrap_err(), "String limit");
    assert_idle(&runtime, 0);
    runtime.begin_job();
    runtime.limits.capacity.string_bytes += 1;
    assert_raw(&stringify(&mut runtime, &value, raw_gap()).unwrap(), &expected);
    assert_idle(&runtime, 0);
}

#[test]
fn stringify_utf16_paired_gap_uses_the_exact_scalar_byte_cap() {
    for gap in [Value::String("😀".into()), Value::StringUnits(vec![0xd83d, 0xde00])] {
        let (mut runtime, value) = number_array();
        let expected = "[\n😀7\n]";
        assert_eq!(expected.len(), 9);
        runtime.limits.capacity.string_bytes = expected.len();
        assert_scalar(&stringify(&mut runtime, &value, gap.clone()).unwrap(), expected);
        assert_idle(&runtime, 0);
        runtime.limits.capacity.string_bytes -= 1;
        fatal(stringify(&mut runtime, &value, gap.clone()).unwrap_err(), "String limit");
        assert_idle(&runtime, 0);
        runtime.begin_job();
        runtime.limits.capacity.string_bytes += 1;
        assert_scalar(&stringify(&mut runtime, &value, gap).unwrap(), expected);
    }
}

#[test]
fn stringify_utf16_raw_output_admits_bmp_payloads_without_hypothetical_utf8_expansion() {
    let mut runtime = Runtime::for_test();
    let part = "\u{0800}".repeat(50);
    let value = runtime.array_from_values(&[Value::String(part.clone()), Value::String(part.clone())]).unwrap();
    let mut expected = vec![0x5b, 0x0a, 0xd800, 0x22];
    expected.extend([0x0800; 50]);
    expected.extend("\",\n".encode_utf16());
    expected.extend([0xd800, 0x22]);
    expected.extend([0x0800; 50]);
    expected.extend("\"\n]".encode_utf16());
    assert_eq!(expected.len() * 2, 224);
    runtime.limits.capacity.string_bytes = 256;
    assert!(part.len() < runtime.limits.capacity.string_bytes);
    assert!(2 * part.len() > runtime.limits.capacity.string_bytes);
    let output = stringify(&mut runtime, &value, raw_gap()).unwrap();
    assert_raw(&output, &expected);
    assert_eq!(output.bytes(), 224);
    assert_idle(&runtime, 0);
}

#[test]
fn stringify_utf16_raw_output_applies_the_same_final_admission_to_bmp_keys() {
    let mut runtime = Runtime::for_test();
    let id = runtime.new_object(None).unwrap();
    let first = "\u{0800}".repeat(40);
    let second = "\u{0801}".repeat(40);
    runtime.set_property(id, first.clone(), Value::Number(1.0)).unwrap();
    runtime.set_property(id, second.clone(), Value::Number(2.0)).unwrap();
    let mut expected = vec![0x7b, 0x0a, 0xd800, 0x22];
    expected.extend([0x0800; 40]);
    expected.extend("\": 1,\n".encode_utf16());
    expected.extend([0xd800, 0x22]);
    expected.extend([0x0801; 40]);
    expected.extend("\": 2\n}".encode_utf16());
    runtime.limits.capacity.string_bytes = expected.len() * 2;
    assert!(first.len() + second.len() > runtime.limits.capacity.string_bytes);
    let output = stringify(&mut runtime, &Value::Object(id), raw_gap()).unwrap();
    assert_raw(&output, &expected);
    assert_eq!(output.bytes(), runtime.limits.capacity.string_bytes);
    assert_idle(&runtime, 0);
}

#[test]
fn stringify_utf16_unused_raw_gap_does_not_double_scalar_root_or_empty_output() {
    for case in ["number", "boolean", "null", "string", "array", "object", "undefined"] {
        let mut runtime = Runtime::for_test();
        let (value, expected, limit) = match case {
            "number" => (Value::Number(0.0), Some("0"), 2),
            "boolean" => (Value::Bool(true), Some("true"), 4),
            "null" => (Value::Null, Some("null"), 4),
            "string" => (Value::String("ab".into()), Some("\"ab\""), 4),
            "array" => (runtime.array_from_values(&[]).unwrap(), Some("[]"), 2),
            "object" => (Value::Object(runtime.new_object(None).unwrap()), Some("{}"), 2),
            _ => (Value::Undefined, None, 2),
        };
        runtime.json_stringify(std::slice::from_ref(&value), &mut NoHost).unwrap();
        runtime.limits.capacity.string_bytes = limit;
        let output = stringify(&mut runtime, &value, raw_gap()).unwrap();
        if let Some(expected) = expected {
            assert_scalar(&output, expected);
            assert_eq!(output.bytes(), expected.len());
        } else {
            assert!(matches!(output, Value::Undefined));
        }
        assert_idle(&runtime, 0);
    }
}

#[test]
fn stringify_utf16_scalar_output_reserves_scratch_even_when_raw_gap_is_unused() {
    let mut runtime = Runtime::for_test();
    let occupied = runtime.limits.heap.retained_values - 3;
    runtime.json_scratch_bytes = occupied;
    fatal(stringify(&mut runtime, &Value::Bool(true), raw_gap()).unwrap_err(),
        "JSON stringify output scratch limit");
    assert_idle(&runtime, occupied);
    runtime.json_scratch_bytes = 0;
    runtime.begin_job();
    assert_scalar(&stringify(&mut runtime, &Value::Bool(true), raw_gap()).unwrap(), "true");
    assert_idle(&runtime, 0);
}

#[test]
fn stringify_utf16_container_refusal_preserves_the_callers_existing_scratch() {
    let (mut runtime, value) = number_array();
    let occupied = runtime.limits.heap.retained_values - 1;
    runtime.json_scratch_bytes = occupied;
    fatal(stringify(&mut runtime, &value, raw_gap()).unwrap_err(), "JSON stringify");
    assert_idle(&runtime, occupied);
    runtime.json_scratch_bytes = 0;
    runtime.begin_job();
    assert_raw(&stringify(&mut runtime, &value, raw_gap()).unwrap(),
        &[0x5b, 0x0a, 0xd800, 0x37, 0x0a, 0x5d]);
    assert_idle(&runtime, 0);
}

fn budget_source() -> (Runtime, Value) {
    let mut runtime = Runtime::for_test();
    let value = runtime.array_from_values(&[
        Value::String("prefix".repeat(8)), Value::String("tail".repeat(8)),
    ]).unwrap();
    runtime.begin_job();
    (runtime, value)
}

fn expected_budget_output() -> Vec<u16> {
    let mut expected = vec![0x5b, 0x0a, 0xd800, 0x22];
    expected.extend("prefix".repeat(8).encode_utf16());
    expected.extend("\",\n".encode_utf16());
    expected.extend([0xd800, 0x22]);
    expected.extend("tail".repeat(8).encode_utf16());
    expected.extend("\"\n]".encode_utf16());
    expected
}

#[test]
fn stringify_utf16_late_copy_refusal_releases_scratch_without_refunding_work() {
    let (mut measured, value) = budget_source();
    let initial = measured.copy_bytes;
    stringify(&mut measured, &value, raw_gap()).unwrap();
    let required = initial - measured.copy_bytes;
    assert!(required > 1);

    let (mut runtime, value) = budget_source();
    let occupied = 73;
    runtime.json_scratch_bytes = occupied;
    runtime.copy_bytes = required - 1;
    let deadline = runtime.job_deadline;
    fatal(stringify(&mut runtime, &value, raw_gap()).unwrap_err(), "String/copy work limit");
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!(runtime.job_deadline, deadline);
    assert_idle(&runtime, occupied);
    runtime.begin_job();
    assert_raw(&stringify(&mut runtime, &value, raw_gap()).unwrap(), &expected_budget_output());
    assert_idle(&runtime, occupied);
}

#[test]
fn stringify_utf16_late_step_refusal_releases_scratch_and_allows_a_later_call() {
    let (mut measured, value) = budget_source();
    let initial = measured.steps;
    stringify(&mut measured, &value, raw_gap()).unwrap();
    let required = initial - measured.steps;
    assert!(required > 1);

    let (mut runtime, value) = budget_source();
    let occupied = 73;
    runtime.json_scratch_bytes = occupied;
    runtime.steps = required - 1;
    let work = runtime.copy_bytes;
    fatal(stringify(&mut runtime, &value, raw_gap()).unwrap_err(), "Execution step limit");
    assert_eq!(runtime.steps, 0);
    assert!(runtime.copy_bytes < work);
    assert_idle(&runtime, occupied);
    runtime.begin_job();
    assert_raw(&stringify(&mut runtime, &value, raw_gap()).unwrap(), &expected_budget_output());
    assert_idle(&runtime, occupied);
}

thread_local! {
    static SCRATCH_OBSERVATIONS: RefCell<Vec<(&'static str, usize)>> = const { RefCell::new(Vec::new()) };
}

fn inspect_stringify_scratch(runtime: &Runtime, callee: usize, _: &[Value], _: &Value) {
    let name = runtime.objects[runtime.function_properties[callee]].properties["name"].value.as_value().unwrap();
    let Value::String(name) = name else { return; };
    let phase = match name.as_str() {
        "stringifyOuterProbe" => "outer",
        "stringifyInnerProbe" => "inner",
        "stringifyAfterProbe" => "after",
        "stringifyFailedProbe" => "failed",
        _ => return,
    };
    SCRATCH_OBSERVATIONS.with(|values| values.borrow_mut().push((phase, runtime.json_scratch_bytes)));
}

#[test]
fn stringify_utf16_nested_callbacks_share_output_scratch_and_restore_it_after_inner_failure() {
    let mut runtime = Runtime::for_test();
    let prefix = "p".repeat(4096);
    runtime.define_global("stringifyPrefix", Value::String(prefix.clone())).unwrap();
    runtime.execute(r#"
function stringifyOuterProbe(){}function stringifyInnerProbe(){}
function stringifyAfterProbe(){}function stringifyFailedProbe(){}
var stringifyCycle={};stringifyCycle.self=stringifyCycle;
var stringifyInner=[stringifyPrefix,{toJSON:function(){stringifyInnerProbe();return 9;}}];
var stringifyOuter=[stringifyPrefix,{toJSON:function(){
    stringifyOuterProbe();
    JSON.stringify(stringifyInner,undefined,'\ud800');
    stringifyAfterProbe();
    var caught=false;
    try{JSON.stringify([stringifyPrefix,stringifyCycle],undefined,'\ud800');}
    catch(error){caught=error.name==='TypeError';}
    if(!caught)throw 'nested cycle completion';
    stringifyFailedProbe();return 7;
}}];
"#, &mut NoHost).unwrap();
    let outer = runtime.binding_value(runtime.global_environment, "stringifyOuter").unwrap();
    stringify(&mut runtime, &outer, raw_gap()).unwrap();
    runtime.begin_job();
    let occupied = 73;
    runtime.json_scratch_bytes = occupied;
    SCRATCH_OBSERVATIONS.with(|values| values.borrow_mut().clear());
    runtime.transient_roots.script_input_observer = Some(inspect_stringify_scratch);
    let result = stringify(&mut runtime, &outer, raw_gap());
    runtime.transient_roots.script_input_observer = None;
    let output = result.unwrap();
    let mut expected = vec![0x5b, 0x0a, 0xd800, 0x22];
    expected.extend(prefix.encode_utf16());
    expected.extend("\",\n".encode_utf16());
    expected.extend([0xd800, 0x37, 0x0a, 0x5d]);
    assert_raw(&output, &expected);
    let seen = SCRATCH_OBSERVATIONS.with(|values| values.borrow().clone());
    assert_eq!(seen.iter().map(|entry| entry.0).collect::<Vec<_>>(),
        vec!["outer", "inner", "after", "failed"]);
    assert!(seen[0].1 >= occupied + 2 * prefix.len(), "Outer raw output must remain charged during its callback");
    assert!(seen[1].1 >= seen[0].1 + 2 * prefix.len(), "Nested raw output must share its caller's scratch budget");
    assert_eq!(seen[2].1, seen[0].1, "Successful inner serialization must release only its own scratch");
    assert_eq!(seen[3].1, seen[0].1, "A caught inner cycle must preserve the outer output charge");
    assert_idle(&runtime, occupied);
}

#[test]
fn stringify_utf16_callback_publications_survive_throw_and_output_scratch_unwind() {
    let mut runtime = Runtime::for_test();
    runtime.execute(r#"
var stringifySaved={},stringifyMarker={},stringifyLater=0;
var stringifySource=['prefix',{toJSON:function(){
    stringifySaved['\ud800']='\udfff';throw stringifyMarker;
}},{toJSON:function(){stringifyLater++;return 1;}}];
"#, &mut NoHost).unwrap();
    let source = runtime.binding_value(runtime.global_environment, "stringifySource").unwrap();
    let saved = object_id(runtime.binding_value(runtime.global_environment, "stringifySaved").unwrap());
    let marker = object_id(runtime.binding_value(runtime.global_environment, "stringifyMarker").unwrap());
    let retained = runtime.value_bytes;
    let occupied = 91;
    runtime.json_scratch_bytes = occupied;
    let fault = stringify(&mut runtime, &source, raw_gap()).unwrap_err();
    assert!(matches!(fault.exception.as_deref(),
        Some(super::super::Exception::Value(Value::Object(id))) if *id == marker), "{fault:?}");
    assert_raw(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
    assert_eq!(runtime.binding_value(runtime.global_environment, "stringifyLater").unwrap().to_number(), 0.0);
    assert!(runtime.value_bytes >= retained + 4, "Published raw key/value bytes must survive output cleanup");
    assert_idle(&runtime, occupied);
    runtime.begin_job();
    runtime.set_property(object_id(source.clone()), "1".into(), Value::Number(0.0)).unwrap();
    assert!(stringify(&mut runtime, &source, raw_gap()).is_ok());
    assert_eq!(runtime.binding_value(runtime.global_environment, "stringifyLater").unwrap().to_number(), 1.0);
    assert_raw(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
    assert_idle(&runtime, occupied);
}

#[test]
fn stringify_utf16_late_output_limit_keeps_observable_callback_publications() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("stringifyTail", Value::String("tail".repeat(16))).unwrap();
    runtime.execute(r#"
var stringifySaved={},stringifyCalls=0;
var stringifySource=['prefix',{toJSON:function(){
    stringifyCalls++;stringifySaved['\ud800']='\udfff';return 9;
}},stringifyTail];
"#, &mut NoHost).unwrap();
    let source = runtime.binding_value(runtime.global_environment, "stringifySource").unwrap();
    let saved = object_id(runtime.binding_value(runtime.global_environment, "stringifySaved").unwrap());
    let original_limit = runtime.limits.capacity.string_bytes;
    let retained = runtime.value_bytes;
    runtime.limits.capacity.string_bytes = 80;
    let occupied = 91;
    runtime.json_scratch_bytes = occupied;
    fatal(stringify(&mut runtime, &source, raw_gap()).unwrap_err(), "String limit");
    assert_eq!(runtime.binding_value(runtime.global_environment, "stringifyCalls").unwrap().to_number(), 1.0);
    assert_raw(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
    assert!(runtime.value_bytes >= retained + 4);
    assert_idle(&runtime, occupied);
    runtime.limits.capacity.string_bytes = original_limit;
    runtime.begin_job();
    assert!(stringify(&mut runtime, &source, raw_gap()).is_ok());
    assert_eq!(runtime.binding_value(runtime.global_environment, "stringifyCalls").unwrap().to_number(), 2.0);
    assert_idle(&runtime, occupied);
}

#[test]
fn stringify_utf16_late_scratch_refusal_keeps_publications_and_restores_the_outer_charge() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("stringifyPrefix", Value::String("p".repeat(512))).unwrap();
    runtime.define_global("stringifyTail", Value::String("t".repeat(4096))).unwrap();
    runtime.execute(r#"
var stringifySaved={},stringifyCalls=0;
var stringifySource=[stringifyPrefix,{toJSON:function(){
    stringifyCalls++;stringifySaved['\ud800']='\udfff';return stringifyTail;
}}];
"#, &mut NoHost).unwrap();
    let source = runtime.binding_value(runtime.global_environment, "stringifySource").unwrap();
    let saved = object_id(runtime.binding_value(runtime.global_environment, "stringifySaved").unwrap());
    let occupied = runtime.limits.heap.retained_values - 4096;
    runtime.json_scratch_bytes = occupied;
    fatal(stringify(&mut runtime, &source, raw_gap()).unwrap_err(), "JSON stringify output scratch limit");
    assert_eq!(runtime.binding_value(runtime.global_environment, "stringifyCalls").unwrap().to_number(), 1.0);
    assert_raw(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
    assert_idle(&runtime, occupied);
    runtime.json_scratch_bytes = 0;
    runtime.begin_job();
    assert!(stringify(&mut runtime, &source, raw_gap()).is_ok());
    assert_eq!(runtime.binding_value(runtime.global_environment, "stringifyCalls").unwrap().to_number(), 2.0);
    assert_idle(&runtime, 0);
}
