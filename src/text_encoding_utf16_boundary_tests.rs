//! Original exact-string conversion, source-storage, and encoder refusal checks.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("Unexpected native read") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("Unexpected native write") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("Unexpected native call") }
}

fn prepared(destination_length: usize) -> (Runtime, Value, usize) {
    let mut runtime = Runtime::for_test();
    let encoder = runtime.construct_text_encoding(true, &[], &mut NoHost).unwrap();
    let destination = runtime.new_typed(BinaryKind::U8, destination_length).unwrap();
    let view = runtime.typed_view(destination).unwrap();
    runtime.buffer_bytes_mut(view.buffer).unwrap().fill(0xcc);
    runtime.begin_job();
    (runtime, encoder, destination)
}

fn encode(runtime: &mut Runtime, encoder: &Value, source: &Value) -> Result<Value, Fault> {
    runtime.text_encoding_builtin(Op::Encode, std::slice::from_ref(source), encoder.clone(), &mut NoHost)
}

fn encode_into(runtime: &mut Runtime, encoder: &Value, source: &Value, destination: Value) -> Result<Value, Fault> {
    runtime.text_encoding_builtin(Op::EncodeInto, &[source.clone(), destination], encoder.clone(), &mut NoHost)
}

fn bytes(runtime: &Runtime, value: &Value) -> Vec<u8> {
    let (buffer, offset, length) = runtime.binary_source_range(value).unwrap();
    runtime.buffer_bytes(buffer).unwrap()[offset..offset + length].to_vec()
}

fn counts(runtime: &Runtime, result: Value) -> (usize, usize) {
    let Value::Object(id) = result else { panic!("Expected encodeInto result") };
    let number = |name: &str| runtime.objects[id].properties[name].value.as_value().unwrap().to_number() as usize;
    (number("read"), number("written"))
}

fn nonbinary_bytes(runtime: &Runtime) -> usize { runtime.value_bytes - runtime.retained_binary }

fn idle(runtime: &Runtime) {
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert!(runtime.transient_registration_is_empty());
}

fn fatal(error: Fault, message: &str) {
    assert!(error.exception.is_none(), "Expected resource refusal: {error:?}");
    assert!(error.message.contains(message), "{error:?}");
}

fn exception(error: &Fault, name: &str) {
    assert!(matches!(error.exception.as_deref(), Some(super::super::Exception::Error(actual)) if *actual == name), "{error:?}");
}

fn binding(runtime: &mut Runtime, name: &str) -> Value {
    runtime.binding_value(runtime.global_environment, name).unwrap()
}

#[test]
fn codec_utf16_encoder_admits_exact_replacement_utf8_size() {
    for (source, expected) in [
        (Value::StringUnits(vec![0xd800]), vec![0xef, 0xbf, 0xbd]),
        (Value::StringUnits(vec![0xdc00]), vec![0xef, 0xbf, 0xbd]),
        (Value::StringUnits(vec![0x0800, 0xd800]), vec![0xe0, 0xa0, 0x80, 0xef, 0xbf, 0xbd]),
    ] {
        let (mut runtime, encoder, _) = prepared(0);
        let baseline = nonbinary_bytes(&runtime);
        runtime.limits.capacity.string_bytes = expected.len() - 1;
        let retained = runtime.retained_binary;
        fatal(encode(&mut runtime, &encoder, &source).unwrap_err(), "String limit");
        assert_eq!(runtime.retained_binary, retained);
        assert_eq!(nonbinary_bytes(&runtime), baseline);
        idle(&runtime);
        runtime.begin_job();
        runtime.limits.capacity.string_bytes = expected.len();
        let output = encode(&mut runtime, &encoder, &source).unwrap();
        assert_eq!(bytes(&runtime, &output), expected);
        assert_eq!(nonbinary_bytes(&runtime), baseline);
        idle(&runtime);
    }
}

#[test]
fn codec_utf16_full_source_admission_precedes_destination_brand_and_short_buffer() {
    for destination_valid in [false, true] {
        let (mut runtime, encoder, destination) = prepared(0);
        let source = Value::StringUnits(vec![0xd800]);
        let before = runtime.value_bytes;
        let target = if destination_valid { Value::Object(destination) } else { Value::Null };
        runtime.limits.capacity.string_bytes = 2;
        fatal(encode_into(&mut runtime, &encoder, &source, target.clone()).unwrap_err(), "String limit");
        assert_eq!(runtime.value_bytes, before);
        idle(&runtime);
        runtime.begin_job();
        runtime.limits.capacity.string_bytes = 3;
        let result = encode_into(&mut runtime, &encoder, &source, target);
        if destination_valid {
            assert_eq!(counts(&runtime, result.unwrap()), (0, 0));
        } else {
            exception(&result.unwrap_err(), "TypeError");
            assert_eq!(runtime.value_bytes, before);
        }
        idle(&runtime);
    }
}

fn spare_source(raw: bool) -> (Value, usize) {
    if raw {
        let mut units = Vec::with_capacity(512);
        units.push(0xd800);
        let capacity = units.capacity() * 2;
        (Value::StringUnits(units), capacity)
    } else {
        let mut text = String::with_capacity(1024);
        text.push('A');
        let capacity = text.capacity();
        (Value::String(text), capacity)
    }
}

#[test]
fn codec_utf16_source_ownership_charges_actual_capacity_and_refunds_exactly() {
    for raw in [false, true] {
        let (mut runtime, _, _) = prepared(0);
        let before = runtime.value_bytes;
        let (source, capacity) = spare_source(raw);
        assert!(capacity > source.bytes());
        runtime.limits.heap.retained_values = before + capacity - 1;
        fatal(runtime.text_hold_string(source).unwrap_err(), "Text codec input workspace limit");
        assert_eq!(runtime.value_bytes, before);
        runtime.limits.heap.retained_values = before + capacity;
        let (source, _) = spare_source(raw);
        let held = runtime.text_hold_string(source).unwrap();
        assert_eq!(held.owned, capacity);
        assert_eq!(runtime.value_bytes, before + capacity);
        runtime.text_release_string(held).unwrap();
        assert_eq!(runtime.value_bytes, before);
        idle(&runtime);
    }
}

#[test]
fn codec_utf16_input_and_encoded_buffer_share_retained_admission() {
    let (mut runtime, encoder, _) = prepared(0);
    let source = Value::StringUnits(vec![0xd800; 32]);
    let before = runtime.value_bytes;
    let binary = runtime.retained_binary;
    runtime.limits.heap.retained_values = before + 64 + 96 - 1;
    fatal(encode(&mut runtime, &encoder, &source).unwrap_err(), "Retained value limit");
    assert_eq!(runtime.value_bytes, before);
    assert_eq!(runtime.retained_binary, binary);
    idle(&runtime);
    runtime.begin_job();
    runtime.limits.heap.retained_values += 1;
    let result = encode(&mut runtime, &encoder, &source).unwrap();
    assert_eq!(bytes(&runtime, &result), [0xef, 0xbf, 0xbd].repeat(32));
    assert_eq!(runtime.value_bytes - before, runtime.retained_binary - binary);
    idle(&runtime);
}

#[test]
fn codec_utf16_short_destination_does_not_allocate_expanded_scalar_source() {
    let (mut runtime, encoder, destination) = prepared(0);
    let source = Value::StringUnits(vec![0xd800; 1024]);
    let before = runtime.value_bytes;
    let binary = runtime.retained_binary;
    runtime.limits.capacity.string_bytes = 3072;
    runtime.limits.heap.retained_values = before + source.bytes() + 128;
    let result = encode_into(&mut runtime, &encoder, &source, Value::Object(destination)).unwrap();
    assert_eq!(counts(&runtime, result), (0, 0));
    assert_eq!(runtime.retained_binary, binary);
    assert!(runtime.value_bytes < before + 128, "Only the returned result fields may remain retained");
    idle(&runtime);
}

fn budget_source() -> Value { Value::StringUnits(vec![b'A' as u16, 0xd800, b'B' as u16]) }

#[test]
fn codec_utf16_encoder_copy_refusal_releases_source_and_recovers() {
    let source = budget_source();
    let (mut measured, encoder, _) = prepared(0);
    let initial = measured.copy_bytes;
    encode(&mut measured, &encoder, &source).unwrap();
    let required = initial - measured.copy_bytes;
    assert!(required > 1);
    let (mut runtime, encoder, _) = prepared(0);
    let baseline = nonbinary_bytes(&runtime);
    let deadline = runtime.job_deadline;
    runtime.copy_bytes = required - 1;
    fatal(encode(&mut runtime, &encoder, &source).unwrap_err(), "String/copy work limit");
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!(runtime.job_deadline, deadline);
    assert_eq!(nonbinary_bytes(&runtime), baseline);
    idle(&runtime);
    runtime.begin_job();
    runtime.copy_bytes = required;
    let result = encode(&mut runtime, &encoder, &source).unwrap();
    assert_eq!(bytes(&runtime, &result), [65, 0xef, 0xbf, 0xbd, 66]);
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!(nonbinary_bytes(&runtime), baseline);
    idle(&runtime);
}

#[test]
fn codec_utf16_encoder_step_refusal_releases_source_and_recovers() {
    let source = budget_source();
    let (mut measured, encoder, _) = prepared(0);
    let initial = measured.steps;
    encode(&mut measured, &encoder, &source).unwrap();
    let required = initial - measured.steps;
    assert!(required > 1);
    let (mut runtime, encoder, _) = prepared(0);
    let baseline = nonbinary_bytes(&runtime);
    runtime.steps = required - 1;
    fatal(encode(&mut runtime, &encoder, &source).unwrap_err(), "Execution step limit");
    assert_eq!(runtime.steps, 0);
    assert_eq!(nonbinary_bytes(&runtime), baseline);
    idle(&runtime);
    runtime.begin_job();
    runtime.steps = required;
    let result = encode(&mut runtime, &encoder, &source).unwrap();
    assert_eq!(bytes(&runtime, &result), [65, 0xef, 0xbf, 0xbd, 66]);
    assert_eq!(runtime.steps, 0);
    assert_eq!(nonbinary_bytes(&runtime), baseline);
    idle(&runtime);
}

#[test]
fn codec_utf16_encode_into_work_refusals_keep_only_complete_committed_scalars() {
    let source = budget_source();
    let expected = [65, 0xef, 0xbf, 0xbd, 66];
    let (mut measured, encoder, destination) = prepared(expected.len() + 2);
    let initial = measured.copy_bytes;
    encode_into(&mut measured, &encoder, &source, Value::Object(destination)).unwrap();
    let required = initial - measured.copy_bytes;
    let mut partial = false;
    for allowance in 0..required {
        let (mut runtime, encoder, destination) = prepared(expected.len() + 2);
        let before = runtime.value_bytes;
        runtime.copy_bytes = allowance;
        fatal(encode_into(&mut runtime, &encoder, &source, Value::Object(destination)).unwrap_err(), "String/copy work limit");
        let actual = bytes(&runtime, &Value::Object(destination));
        let written = actual.iter().position(|&byte| byte == 0xcc).unwrap();
        assert!([0, 1, 4, 5].contains(&written), "Partial UTF-8 scalar at allowance {allowance}: {actual:x?}");
        assert_eq!(&actual[..written], &expected[..written]);
        assert!(actual[written..].iter().all(|&byte| byte == 0xcc));
        if written < expected.len() {
            assert_eq!(runtime.value_bytes, before, "Source backing leaked before result allocation");
        }
        idle(&runtime);
        if !partial && written > 0 && written < expected.len() {
            partial = true;
            runtime.begin_job();
            let result = encode_into(&mut runtime, &encoder, &source, Value::Object(destination)).unwrap();
            assert_eq!(counts(&runtime, result), (3, 5));
            assert_eq!(bytes(&runtime, &Value::Object(destination)), [65, 0xef, 0xbf, 0xbd, 66, 0xcc, 0xcc]);
            idle(&runtime);
        }
    }
    assert!(partial, "The measured allowances must reach an intermediate committed prefix");
}

#[test]
fn codec_utf16_encode_into_late_result_refusal_preserves_written_bytes() {
    let (mut runtime, encoder, destination) = prepared(7);
    let source = budget_source();
    let before = runtime.value_bytes;
    let object_limit = runtime.limits.heap.objects;
    runtime.limits.heap.objects = runtime.objects.len();
    let error = encode_into(&mut runtime, &encoder, &source, Value::Object(destination)).unwrap_err();
    fatal(error, "Object identity limit");
    assert_eq!(bytes(&runtime, &Value::Object(destination)), [65, 0xef, 0xbf, 0xbd, 66, 0xcc, 0xcc]);
    assert_eq!(runtime.value_bytes, before);
    idle(&runtime);
    runtime.limits.heap.objects = object_limit;
    runtime.begin_job();
    let result = encode_into(&mut runtime, &encoder, &source, Value::Object(destination)).unwrap();
    assert_eq!(counts(&runtime, result), (3, 5));
    idle(&runtime);
}

#[test]
fn codec_utf16_source_workspace_refusal_preserves_conversion_and_precedes_bad_destination() {
    let (mut runtime, encoder, destination) = prepared(0);
    runtime.execute(r#"
var codecCalls=0;
var codecSource={toString:function(){codecCalls++;return '\ud800';}};
"#, &mut NoHost).unwrap();
    let source = binding(&mut runtime, "codecSource");
    encode_into(&mut runtime, &encoder, &source, Value::Object(destination)).unwrap();
    let before = runtime.value_bytes;
    let limit = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = before;
    fatal(encode_into(&mut runtime, &encoder, &source, Value::Null).unwrap_err(), "Text codec input workspace limit");
    assert_eq!(binding(&mut runtime, "codecCalls").to_number(), 2.0);
    assert_eq!(runtime.value_bytes, before);
    idle(&runtime);
    runtime.limits.heap.retained_values = limit;
    runtime.begin_job();
    exception(&encode_into(&mut runtime, &encoder, &source, Value::Null).unwrap_err(), "TypeError");
    assert_eq!(binding(&mut runtime, "codecCalls").to_number(), 3.0);
    assert_eq!(runtime.value_bytes, before);
    idle(&runtime);
}

#[test]
fn codec_utf16_decoder_label_release_preserves_nested_dictionary_publications() {
    let prepare_options = || {
        let (mut runtime, _, _) = prepared(0);
        runtime.execute(r#"
var codecSaved={first:null,second:null},codecCalls=0;
var codecOptions={
 get fatal(){codecCalls++;codecSaved.first=new TextEncoder().encode('\ud800');return false;},
 get ignoreBOM(){codecCalls++;codecSaved.second=new TextEncoder().encode('X');return false;}
};
"#, &mut NoHost).unwrap();
        let options = binding(&mut runtime, "codecOptions");
        let Value::Object(saved) = binding(&mut runtime, "codecSaved") else { unreachable!() };
        (runtime, options, saved)
    };
    let (mut runtime, options, saved) = prepare_options();
    let (mut control, control_options, control_saved) = prepare_options();
    assert_eq!((runtime.value_bytes, runtime.retained_binary), (control.value_bytes, control.retained_binary));
    let binary = runtime.retained_binary;
    let error = runtime.construct_text_encoding(false,
        &[Value::StringUnits(vec![0xd800]), options], &mut NoHost).unwrap_err();
    exception(&error, "RangeError");
    // Run identical getters at constructor entry depth, without a held label.
    // Native method realization and callback arena storage remain legitimate.
    control.call_depth += 1;
    let options_result = control.text_options(Some(&control_options), &["fatal", "ignoreBOM"], &mut NoHost);
    control.call_depth -= 1;
    assert_eq!(options_result.unwrap(), vec![false, false]);
    for (runtime, saved) in [(&mut runtime, saved), (&mut control, control_saved)] {
        assert_eq!(binding(runtime, "codecCalls").to_number(), 2.0);
        let first = runtime.own_property(saved, "first").unwrap();
        let second = runtime.own_property(saved, "second").unwrap();
        assert_eq!(bytes(runtime, &first), [0xef, 0xbf, 0xbd]);
        assert_eq!(bytes(runtime, &second), [b'X']);
        assert_eq!(runtime.retained_binary - binary, 4);
        idle(runtime);
    }
    assert_eq!((runtime.value_bytes, runtime.retained_binary), (control.value_bytes, control.retained_binary),
        "Only captured label storage may be refunded; callback publications and metadata must remain retained");
}

#[test]
fn codec_utf16_abrupt_dictionary_conversion_refunds_label_and_keeps_author_effects() {
    let (mut runtime, _, _) = prepared(0);
    runtime.execute(r#"
var codecSaved={},codecCalls=0,codecLater=0,codecMarker={};
var codecOptions={
 get fatal(){codecCalls++;codecSaved['\ud800']='\udfff';throw codecMarker;},
 get ignoreBOM(){codecLater++;return false;}
};
"#, &mut NoHost).unwrap();
    let options = binding(&mut runtime, "codecOptions");
    let Value::Object(saved) = binding(&mut runtime, "codecSaved") else { unreachable!() };
    let Value::Object(marker) = binding(&mut runtime, "codecMarker") else { unreachable!() };
    for attempt in 0..2 {
        let before = runtime.value_bytes;
        let error = runtime.construct_text_encoding(false,
            &[Value::StringUnits(vec![0xd800; 64]), options.clone()], &mut NoHost).unwrap_err();
        assert!(matches!(error.exception.as_deref(),
            Some(super::super::Exception::Value(Value::Object(id))) if *id == marker), "{error:?}");
        assert_eq!(binding(&mut runtime, "codecCalls").to_number(), (attempt + 1) as f64);
        assert_eq!(binding(&mut runtime, "codecLater").to_number(), 0.0);
        let value = runtime.own_units_property(saved, &[0xd800]).unwrap();
        assert!(matches!(value, Value::StringUnits(ref units) if units == &[0xdfff]));
        if attempt == 1 { assert_eq!(runtime.value_bytes, before, "Repeated publication must not leak captured label storage"); }
        idle(&runtime);
        runtime.begin_job();
    }
}
