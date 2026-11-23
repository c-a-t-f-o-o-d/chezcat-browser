//! Original selected-result storage, work, and callback boundaries for strings.
use super::*;

const METHODS: [&str; 3] = ["slice", "substring", "substr"];

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("String slicing boundaries must not read a native host");
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("String slicing boundaries must not write a native host");
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("String slicing boundaries must not call a native host");
    }
}

fn span(runtime: &mut Runtime, source: &Value, method: &str, start: usize, end: usize) -> Result<Value, Fault> {
    let second = if method == "substr" { end - start } else { end };
    runtime.string_slice_value(source, method,
        &[Value::Number(start as f64), Value::Number(second as f64)], &mut NoHost)
}

fn scalar(value: &Value, expected: &str) {
    let Value::String(text) = value else { panic!("Expected canonical scalar result: {value:?}") };
    assert_eq!(text, expected);
}

fn units(value: &Value, expected: &[u16]) {
    let Value::StringUnits(actual) = value else { panic!("Expected exact raw result: {value:?}") };
    assert_eq!(actual, expected);
}

fn heap(runtime: &Runtime) -> (usize, usize, usize) {
    (runtime.objects.len(), runtime.properties, runtime.value_bytes)
}

fn idle(runtime: &Runtime) {
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert!(runtime.transient_registration_is_empty());
}

fn fatal(error: Fault, message: &str) {
    assert!(error.exception.is_none(), "Expected a resource refusal: {error:?}");
    assert!(error.message.contains(message), "{error:?}");
}

fn binding(runtime: &mut Runtime, name: &str) -> Value {
    runtime.binding_value(runtime.global_environment, name).unwrap()
}

#[test]
fn slicing_utf16_paired_scalar_result_uses_its_exact_utf8_byte_limit() {
    for method in METHODS {
        let mut runtime = Runtime::for_test();
        let source = Value::String("A😀Z".into());
        let before = heap(&runtime);
        runtime.limits.capacity.string_bytes = 4;
        let output = span(&mut runtime, &source, method, 1, 3).unwrap();
        scalar(&output, "😀");
        assert_eq!(output.bytes(), 4);
        assert_eq!(heap(&runtime), before);
        runtime.limits.capacity.string_bytes = 3;
        fatal(span(&mut runtime, &source, method, 1, 3).unwrap_err(), "String limit");
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
        runtime.begin_job();
        runtime.limits.capacity.string_bytes = 4;
        scalar(&span(&mut runtime, &source, method, 1, 3).unwrap(), "😀");
        idle(&runtime);
    }
}

#[test]
fn slicing_utf16_split_pair_results_use_two_raw_bytes_and_recover() {
    for method in METHODS {
        for (start, expected) in [(1, 0xd83d), (2, 0xde00)] {
            let mut runtime = Runtime::for_test();
            let source = Value::String("A😀Z".into());
            let before = heap(&runtime);
            runtime.limits.capacity.string_bytes = 2;
            units(&span(&mut runtime, &source, method, start, start + 1).unwrap(), &[expected]);
            assert_eq!(heap(&runtime), before);
            runtime.limits.capacity.string_bytes = 1;
            fatal(span(&mut runtime, &source, method, start, start + 1).unwrap_err(), "String limit");
            assert_eq!(heap(&runtime), before);
            idle(&runtime);
            runtime.begin_job();
            runtime.limits.capacity.string_bytes = 2;
            units(&span(&mut runtime, &source, method, start, start + 1).unwrap(), &[expected]);
            idle(&runtime);
        }
    }
}

#[test]
fn slicing_utf16_raw_bmp_output_does_not_require_hypothetical_utf8_storage() {
    let mut expected = vec![0x0800; 64];
    expected.push(0xd800);
    let mut input = vec![0xdc00];
    input.extend_from_slice(&expected);
    input.push(0xdc00);
    let source = Value::StringUnits(input);
    for method in METHODS {
        let mut runtime = Runtime::for_test();
        let before = heap(&runtime);
        runtime.limits.capacity.string_bytes = expected.len() * 2;
        assert!(64 * 3 > runtime.limits.capacity.string_bytes);
        let result = span(&mut runtime, &source, method, 1, 66).unwrap();
        units(&result, &expected);
        assert_eq!(result.bytes(), 130);
        assert_eq!(heap(&runtime), before);
        runtime.limits.capacity.string_bytes -= 1;
        fatal(span(&mut runtime, &source, method, 1, 66).unwrap_err(), "String limit");
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
    }
}

#[test]
fn slicing_utf16_well_formed_selection_from_raw_input_uses_canonical_scalar_admission() {
    let mut input = vec![0x0800; 48];
    input.push(0xd800);
    let source = Value::StringUnits(input);
    for method in METHODS {
        let mut runtime = Runtime::for_test();
        let before = heap(&runtime);
        runtime.limits.capacity.string_bytes = 144;
        scalar(&span(&mut runtime, &source, method, 0, 48).unwrap(), &"\u{0800}".repeat(48));
        runtime.limits.capacity.string_bytes = 143;
        fatal(span(&mut runtime, &source, method, 0, 48).unwrap_err(), "String limit");
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
        runtime.begin_job();
        runtime.limits.capacity.string_bytes = 144;
        scalar(&span(&mut runtime, &source, method, 0, 48).unwrap(), &"\u{0800}".repeat(48));
        assert_eq!(heap(&runtime), before);
    }
}

#[test]
fn slicing_utf16_unused_raw_suffix_needs_neither_storage_nor_copy_work() {
    for method in METHODS {
        let mut charges = Vec::new();
        for suffix in [1, 16_384] {
            let mut runtime = Runtime::for_test();
            let mut input = vec![b'a' as u16];
            input.extend(std::iter::repeat_n(0xd800, suffix));
            let source = Value::StringUnits(input);
            let before = heap(&runtime);
            runtime.limits.capacity.string_bytes = 1;
            runtime.limits.heap.retained_values = runtime.value_bytes + 32;
            let initial = runtime.copy_bytes;
            scalar(&span(&mut runtime, &source, method, 0, 1).unwrap(), "a");
            charges.push(initial - runtime.copy_bytes);
            assert_eq!(heap(&runtime), before);
            idle(&runtime);
        }
        assert!(charges[0] > 0);
        assert_eq!(charges[0], charges[1], "Unused raw suffix must not be copied or scanned for {method}");
    }
}

#[test]
fn slicing_utf16_scalar_receiver_needs_only_selected_result_workspace() {
    for method in METHODS {
        let mut runtime = Runtime::for_test();
        let source = Value::String(format!("A{}", "😀".repeat(4096)));
        let before = heap(&runtime);
        runtime.limits.capacity.string_bytes = 1;
        runtime.limits.heap.retained_values = runtime.value_bytes + 32;
        let initial = runtime.copy_bytes;
        scalar(&span(&mut runtime, &source, method, 0, 1).unwrap(), "A");
        assert!(initial - runtime.copy_bytes >= source.bytes(), "Scalar length scan must charge input work");
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
    }
}

#[test]
fn slicing_utf16_empty_selection_needs_no_result_storage() {
    for method in METHODS {
        for source in [Value::String("😀tail".into()), Value::StringUnits(vec![0xd800; 1024])] {
            let mut runtime = Runtime::for_test();
            let before = heap(&runtime);
            runtime.limits.capacity.string_bytes = 0;
            runtime.limits.heap.retained_values = runtime.value_bytes;
            scalar(&span(&mut runtime, &source, method, 0, 0).unwrap(), "");
            assert_eq!(heap(&runtime), before);
            idle(&runtime);
        }
    }
}

fn budget_source() -> Value {
    let mut input: Vec<u16> = "selected output ".repeat(16).encode_utf16().collect();
    input.push(0xd800);
    Value::StringUnits(input)
}

#[test]
fn slicing_utf16_late_copy_refusal_refunds_only_output_workspace() {
    let source = budget_source();
    let count = source.string_length().unwrap();
    for method in METHODS {
        let mut measured = Runtime::for_test();
        let initial = measured.copy_bytes;
        span(&mut measured, &source, method, 0, count).unwrap();
        let required = initial - measured.copy_bytes;
        assert!(required > 1);
        let mut runtime = Runtime::for_test();
        let before = heap(&runtime);
        let deadline = runtime.job_deadline;
        runtime.copy_bytes = required - 1;
        fatal(span(&mut runtime, &source, method, 0, count).unwrap_err(), "String/copy work limit");
        assert_eq!(runtime.copy_bytes, 0);
        assert_eq!(runtime.job_deadline, deadline);
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
        runtime.begin_job();
        runtime.copy_bytes = required;
        let result = span(&mut runtime, &source, method, 0, count).unwrap();
        let Value::StringUnits(expected) = &source else { unreachable!() };
        units(&result, expected);
        assert_eq!(runtime.copy_bytes, 0);
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
    }
}

#[test]
fn slicing_utf16_late_step_refusal_restores_workspace_and_allows_recovery() {
    let source = budget_source();
    let count = source.string_length().unwrap();
    for method in METHODS {
        let mut measured = Runtime::for_test();
        let initial = measured.steps;
        span(&mut measured, &source, method, 0, count).unwrap();
        let required = initial - measured.steps;
        assert!(required > 2);
        let mut runtime = Runtime::for_test();
        let before = heap(&runtime);
        let work = runtime.copy_bytes;
        runtime.steps = required - 1;
        fatal(span(&mut runtime, &source, method, 0, count).unwrap_err(), "Execution step limit");
        assert_eq!(runtime.steps, 0);
        assert!(runtime.copy_bytes < work);
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
        runtime.begin_job();
        runtime.steps = required;
        let result = span(&mut runtime, &source, method, 0, count).unwrap();
        let Value::StringUnits(expected) = &source else { unreachable!() };
        units(&result, expected);
        assert_eq!(runtime.steps, 0);
        assert_eq!(heap(&runtime), before);
        idle(&runtime);
    }
}

#[test]
fn slicing_utf16_result_workspace_respects_existing_retained_values() {
    for method in METHODS {
        for source in [Value::String("scalar output".into()), budget_source()] {
            let mut runtime = Runtime::for_test();
            let before = heap(&runtime);
            let original_limit = runtime.limits.heap.retained_values;
            let count = source.string_length().unwrap();
            runtime.limits.heap.retained_values = runtime.value_bytes;
            fatal(span(&mut runtime, &source, method, 0, count).unwrap_err(), "String slicing workspace limit");
            assert_eq!(heap(&runtime), before);
            idle(&runtime);
            runtime.limits.heap.retained_values = original_limit;
            runtime.begin_job();
            assert!(span(&mut runtime, &source, method, 0, count).is_ok());
            assert_eq!(heap(&runtime), before);
            idle(&runtime);
        }
    }
}

#[test]
fn slicing_utf16_output_refusal_preserves_argument_conversion_publications() {
    for method in METHODS {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
var slicingSaved={},slicingCalls=0;
var slicingStart={valueOf:function(){slicingCalls++;slicingSaved['\ud800']='\udfff';return 0;}};
var slicingEnd={valueOf:function(){slicingCalls++;return 65;}};
"#, &mut NoHost).unwrap();
        let start = binding(&mut runtime, "slicingStart");
        let end = binding(&mut runtime, "slicingEnd");
        let Value::Object(saved) = binding(&mut runtime, "slicingSaved") else { unreachable!() };
        let mut input = vec![0x0800; 64];
        input.push(0xd800);
        let source = Value::StringUnits(input.clone());
        let retained = runtime.value_bytes;
        runtime.limits.capacity.string_bytes = 128;
        fatal(runtime.string_slice_value(&source, method, &[start.clone(), end.clone()], &mut NoHost).unwrap_err(), "String limit");
        assert_eq!(binding(&mut runtime, "slicingCalls").to_number(), 2.0);
        units(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
        assert!(runtime.value_bytes >= retained + 4, "Published key/value storage must survive output refusal");
        idle(&runtime);
        runtime.begin_job();
        runtime.limits.capacity.string_bytes = 130;
        units(&runtime.string_slice_value(&source, method, &[start, end], &mut NoHost).unwrap(), &input);
        assert_eq!(binding(&mut runtime, "slicingCalls").to_number(), 4.0);
        units(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
        idle(&runtime);
    }
}

#[test]
fn slicing_utf16_nested_argument_calls_preserve_the_outer_receiver_and_publications() {
    for method in METHODS {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
var slicingOuter='\ud800A\udfff',slicingSaved={},slicingCalls=0,slicingMarker={};
var slicingStart={valueOf:function(){
    if('\ud800AB'.slice(1,2)!=='A')throw 'nested scalar slice';
    if('Q😀Z'.substring(2,3).charCodeAt(0)!==0xde00)throw 'nested raw substring';
    var caught=false;try{'\ud800'.substr(0,{valueOf:function(){throw slicingMarker;}});}
    catch(e){caught=e===slicingMarker;}if(!caught)throw 'nested abrupt conversion';
    slicingOuter='changed';slicingSaved['\ud800']='\udfff';slicingCalls++;return 0;
}};
var slicingEnd={valueOf:function(){slicingCalls++;return 2;}};
"#, &mut NoHost).unwrap();
        let source = binding(&mut runtime, "slicingOuter");
        let start = binding(&mut runtime, "slicingStart");
        let end = binding(&mut runtime, "slicingEnd");
        let Value::Object(saved) = binding(&mut runtime, "slicingSaved") else { unreachable!() };
        units(&runtime.string_slice_value(&source, method, &[start, end], &mut NoHost).unwrap(), &[0xd800, b'A' as u16]);
        scalar(&binding(&mut runtime, "slicingOuter"), "changed");
        assert_eq!(binding(&mut runtime, "slicingCalls").to_number(), 2.0);
        units(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
        idle(&runtime);
    }
}
