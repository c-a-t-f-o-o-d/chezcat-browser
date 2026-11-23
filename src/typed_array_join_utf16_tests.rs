//! Original exact-separator, conversion, and resource checks for typed-array join.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("Typed-array join must not read a native host");
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("Typed-array join must not write a native host");
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("Typed-array join must not call a native host");
    }
}

fn typed(runtime: &mut Runtime, kind: Kind, values: &[f64]) -> Value {
    let id = runtime.new_typed(kind, values.len()).unwrap();
    let view = runtime.typed_view(id).unwrap();
    for (index, value) in values.iter().enumerate() {
        runtime.typed_write(view, index, *value).unwrap();
    }
    Value::Object(id)
}

fn join(runtime: &mut Runtime, value: &Value, separator: Value) -> Result<Value, Fault> {
    runtime.typed_method("join", value.clone(), &[separator], &mut NoHost)
}

fn high() -> Value { Value::StringUnits(vec![0xd800]) }

fn assert_raw(value: &Value, expected: &[u16]) {
    let Value::StringUnits(units) = value else { panic!("Expected exact raw joined output, got {value:?}") };
    assert_eq!(units, expected);
}

fn assert_scalar(value: &Value, expected: &str) {
    let Value::String(text) = value else { panic!("Expected canonical scalar joined output, got {value:?}") };
    assert_eq!(text, expected);
}

fn assert_idle(runtime: &Runtime) {
    assert!(runtime.active_array_joins.is_empty());
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert!(runtime.transient_registration_is_empty());
}

fn heap_state(runtime: &Runtime) -> (usize, usize, usize, usize) {
    (runtime.objects.len(), runtime.properties, runtime.value_bytes, runtime.retained_binary)
}

fn fatal(fault: Fault, expected: &str) {
    assert!(fault.exception.is_none(), "Expected a fatal resource refusal: {fault:?}");
    assert!(fault.message.contains(expected), "{fault:?}");
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    assert_idle(&runtime);
}

#[test]
fn typed_join_utf16_preserves_unpaired_mixed_and_nul_separators_exactly() {
    let mut runtime = Runtime::for_test();
    let value = typed(&mut runtime, Kind::U8, &[1.0, 2.0, 3.0]);
    let before = heap_state(&runtime);
    for separator in [
        vec![0xd800], vec![0xdbff], vec![0xdc00], vec![0xdfff],
        vec![0, 0xdfff], vec![0xd800, 0x61, 0xdc00],
        vec![0xd83d, 0xde00, 0xd800], vec![0xdfff, 0xd800, 0xdc00, 0],
    ] {
        let mut expected = vec![0x31];
        expected.extend_from_slice(&separator);
        expected.push(0x32);
        expected.extend_from_slice(&separator);
        expected.push(0x33);
        assert_raw(&join(&mut runtime, &value, Value::StringUnits(separator)).unwrap(), &expected);
        assert_eq!(heap_state(&runtime), before);
        assert_idle(&runtime);
    }
}

#[test]
fn typed_join_utf16_valid_pairs_and_scalar_separators_keep_scalar_identity() {
    let mut runtime = Runtime::for_test();
    let value = typed(&mut runtime, Kind::U16, &[1.0, 2.0]);
    for separator in [Value::String("😀".into()), Value::StringUnits(vec![0xd83d, 0xde00])] {
        assert_scalar(&join(&mut runtime, &value, separator).unwrap(), "1😀2");
    }
    assert_scalar(&join(&mut runtime, &value, Value::String(String::new())).unwrap(), "12");
    assert_scalar(&join(&mut runtime, &value, Value::String("\0".into())).unwrap(), "1\02");
    assert_idle(&runtime);
}

#[test]
fn typed_join_utf16_empty_and_singleton_still_convert_once_and_return_scalar_text() {
    succeeds(r#"
var calls=0,separator={};
separator[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'hint';calls++;return '\ud800';};
if(new Uint8Array(0).join(separator)!==''||new Int16Array([17]).join(separator)!=='17'||calls!==2)
    throw 'unused separator conversion';
var marker={},thrown=false,failed={toString:function(){calls++;throw marker;}};
try{new Uint8Array(0).join(failed);}catch(error){thrown=error===marker;}
if(!thrown||calls!==3)throw 'empty abrupt conversion';
var symbolCaught=false;
try{new Uint8Array(0).join(Symbol());}catch(error){symbolCaught=error.name==='TypeError';}
if(!symbolCaught)throw 'empty Symbol conversion';
'ok';
"#);
    let mut runtime = Runtime::for_test();
    let empty = typed(&mut runtime, Kind::U8, &[]);
    let singleton = typed(&mut runtime, Kind::I16, &[17.0]);
    runtime.limits.capacity.string_bytes = 2;
    assert_scalar(&join(&mut runtime, &empty, high()).unwrap(), "");
    assert_scalar(&join(&mut runtime, &singleton, high()).unwrap(), "17");
    assert_idle(&runtime);
}

#[test]
fn typed_join_utf16_brand_validation_precedes_separator_conversion() {
    succeeds(r#"
var calls=0,separator={toString:function(){calls++;return '\ud800';}};
var values=[null,undefined,{},[1,2],new DataView(new ArrayBuffer(4))];
for(var i=0;i<values.length;i++){
    var caught=false;
    try{Uint8Array.prototype.join.call(values[i],separator);}catch(error){caught=error.name==='TypeError';}
    if(!caught)throw 'typed brand';
}
if(calls!==0)throw 'conversion before brand';
'ok';
"#);
}

#[test]
fn typed_join_utf16_uses_internal_length_and_observes_separator_mutation_through_aliases() {
    succeeds(r#"
var buffer=new ArrayBuffer(4),alias=new Uint8Array(buffer),view=new DataView(buffer);
alias.set([8,3,4,9]);
var value=new Uint8Array(buffer,1,2),lengthReads=0,calls=0,separator={};
Object.defineProperty(value,'length',{get:function(){lengthReads++;throw 'ordinary length';}});
separator[Symbol.toPrimitive]=function(hint){
    if(hint!=='string'||this!==separator)throw 'conversion receiver or hint';
    calls++;view.setUint8(1,7);alias[2]=6;return '\udfff';
};
if(value.join(separator)!=='7\udfff6'||calls!==1||lengthReads!==0||alias[0]!==8||alias[3]!==9)
    throw 'captured view or live backing';
'ok';
"#);
}

#[test]
fn typed_join_utf16_ordinary_conversion_is_ordered_once_and_keeps_converted_nullish_text() {
    succeeds(r#"
var value=new Uint8Array([1,2]),trace=[],separator={
    toString:function(){trace.push('string');return {};},
    valueOf:function(){trace.push('value');return '\ud800';}
};
if(value.join(separator)!=='1\ud8002'||trace.length!==2||trace[0]!=='string'||trace[1]!=='value')
    throw 'ordinary conversion order';
var converted={};converted[Symbol.toPrimitive]=function(h){if(h!=='string')throw 'hint';return null;};
if(value.join(converted)!=='1null2')throw 'converted null';
converted[Symbol.toPrimitive]=function(){return undefined;};
if(value.join(converted)!=='1undefined2')throw 'converted undefined';
converted[Symbol.toPrimitive]=function(){return Symbol();};
var caught=false;try{value.join(converted);}catch(error){caught=error.name==='TypeError';}
if(!caught)throw 'converted Symbol';
'ok';
"#);
}

#[test]
fn typed_join_utf16_separator_reentry_uses_the_same_live_view_without_array_cycle_suppression() {
    succeeds(r#"
var value=new Uint8Array([1,2]),calls=0,inner,separator={};
separator[Symbol.toPrimitive]=function(hint){
    if(hint!=='string')throw 'hint';
    calls++;inner=value.join('\udfff');value[0]=3;return '\ud800';
};
if(value.join(separator)!=='3\ud8002'||inner!=='1\udfff2'||calls!==1)throw 'typed reentry';
'ok';
"#);
}

#[test]
fn typed_join_utf16_all_numeric_kinds_keep_their_actual_element_and_special_float_spelling() {
    for (kind, expected) in [
        (Kind::I8, "0|-1|1"), (Kind::U8, "0|255|1"), (Kind::U8Clamped, "0|255|2"),
        (Kind::I16, "0|255|1"), (Kind::U16, "0|255|1"),
        (Kind::I32, "0|255|1"), (Kind::U32, "0|255|1"),
        (Kind::F16, "0|255|1.5"), (Kind::F32, "0|255|1.5"), (Kind::F64, "0|255|1.5"),
    ] {
        let mut runtime = Runtime::for_test();
        let value = typed(&mut runtime, kind, &[-0.0, 255.0, 1.5]);
        let expected = expected.encode_utf16().map(|unit| if unit == 0x7c { 0xd800 } else { unit }).collect::<Vec<_>>();
        assert_raw(&join(&mut runtime, &value, high()).unwrap(), &expected);
        assert_idle(&runtime);
    }
    for kind in [Kind::F16, Kind::F32, Kind::F64] {
        let mut runtime = Runtime::for_test();
        let value = typed(&mut runtime, kind, &[-0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY]);
        let expected = "0|NaN|Infinity|-Infinity".encode_utf16()
            .map(|unit| if unit == 0x7c { 0xdfff } else { unit }).collect::<Vec<_>>();
        assert_raw(&join(&mut runtime, &value, Value::StringUnits(vec![0xdfff])).unwrap(), &expected);
    }
}

#[test]
fn typed_join_utf16_default_custom_to_string_and_generic_controls_remain_consistent() {
    succeeds(r#"
var value=new Uint8Array([1,2]);
if(value.join()!=='1,2'||value.join(undefined)!=='1,2'||value.toString()!=='1,2')throw 'default spelling';
if(value.join(null)!=='1null2'||value.join(true)!=='1true2'||value.join(7)!=='172')throw 'primitive separator';
if(Array.prototype.join.call(value,'\ud800')!==value.join('\ud800'))throw 'generic parity';
if(Uint8Array.prototype.toString!==Array.prototype.toString)throw 'canonical toString';
var calls=0;value.join=function(){if(this!==value)throw 'toString receiver';calls++;return '\udfff';};
if(value.toString()!=='\udfff'||calls!==1)throw 'custom join';
value.join=0;
if(value.toString()!=='[object Uint8Array]')throw 'noncallable join fallback';
'ok';
"#);
}

#[test]
fn typed_join_utf16_scalar_and_raw_output_caps_refuse_one_byte_less_without_retaining_workspace() {
    for raw in [false, true] {
        let mut runtime = Runtime::for_test();
        let value = typed(&mut runtime, Kind::U8, &[1.0, 2.0]);
        let separator = if raw { high() } else { Value::String("😀".into()) };
        runtime.limits.capacity.string_bytes = 6;
        let before = heap_state(&runtime);
        let output = join(&mut runtime, &value, separator.clone()).unwrap();
        if raw { assert_raw(&output, &[0x31, 0xd800, 0x32]); }
        else { assert_scalar(&output, "1😀2"); }
        assert_eq!(output.bytes(), 6);
        assert_eq!(heap_state(&runtime), before);
        runtime.limits.capacity.string_bytes = 5;
        fatal(join(&mut runtime, &value, separator.clone()).unwrap_err(), "String limit");
        assert_eq!(heap_state(&runtime), before);
        assert_idle(&runtime);
        runtime.limits.capacity.string_bytes = 6;
        runtime.begin_job();
        assert!(join(&mut runtime, &value, separator).is_ok());
        assert_eq!(heap_state(&runtime), before);
    }
}

fn budget_view() -> (Runtime, Value) {
    let mut runtime = Runtime::for_test();
    let value = typed(&mut runtime, Kind::U8, &[255.0; 16]);
    runtime.begin_job();
    (runtime, value)
}

#[test]
fn typed_join_utf16_copy_and_step_exhaustion_release_owned_storage_and_recover() {
    let (mut measured, value) = budget_view();
    let initial = (measured.copy_bytes, measured.steps);
    let expected = join(&mut measured, &value, high()).unwrap();
    let used = (initial.0 - measured.copy_bytes, initial.1 - measured.steps);
    assert!(used.0 > 1 && used.1 > 1);
    for copies in [true, false] {
        let (mut runtime, value) = budget_view();
        let before = heap_state(&runtime);
        let deadline = runtime.job_deadline;
        if copies { runtime.copy_bytes = used.0 - 1; }
        else { runtime.steps = used.1 - 1; }
        fatal(join(&mut runtime, &value, high()).unwrap_err(),
            if copies { "String/copy work limit" } else { "Execution step limit" });
        if copies { assert_eq!(runtime.copy_bytes, 0); }
        else { assert_eq!(runtime.steps, 0); }
        assert_eq!(runtime.job_deadline, deadline);
        assert_eq!(heap_state(&runtime), before);
        assert_idle(&runtime);
        runtime.begin_job();
        let Value::StringUnits(expected) = &expected else { panic!("Raw separator must remain exact") };
        assert_raw(&join(&mut runtime, &value, high()).unwrap(), expected);
        assert_eq!(heap_state(&runtime), before);
    }
}

#[test]
fn typed_join_utf16_output_workspace_shares_retained_storage_and_refunds_refusal() {
    let (mut runtime, value) = budget_view();
    let before = heap_state(&runtime);
    let original_limit = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = runtime.value_bytes;
    fatal(join(&mut runtime, &value, high()).unwrap_err(), "Array join workspace limit");
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
    runtime.limits.heap.retained_values = original_limit;
    runtime.begin_job();
    assert!(matches!(join(&mut runtime, &value, high()).unwrap(), Value::StringUnits(_)));
    assert_eq!(heap_state(&runtime), before);
    assert_idle(&runtime);
}

#[test]
fn typed_join_utf16_late_refusal_preserves_separator_publications_and_buffer_mutation() {
    let mut runtime = Runtime::for_test();
    runtime.execute(r#"
var typedJoinSaved={},typedJoinCalls=0,typedJoinArray=new Uint8Array([255,255,255,255]);
var typedJoinSeparator={toString:function(){
    typedJoinCalls++;typedJoinSaved['\ud800']='\udfff';typedJoinArray[0]=9;return '\ud800';
}};
"#, &mut NoHost).unwrap();
    let value = runtime.binding_value(runtime.global_environment, "typedJoinArray").unwrap();
    let separator = runtime.binding_value(runtime.global_environment, "typedJoinSeparator").unwrap();
    let Value::Object(saved) = runtime.binding_value(runtime.global_environment, "typedJoinSaved").unwrap()
        else { panic!("Expected publication holder") };
    let expected = [0x39, 0xd800, 0x32, 0x35, 0x35, 0xd800, 0x32, 0x35, 0x35, 0xd800, 0x32, 0x35, 0x35];
    let original_limit = runtime.limits.capacity.string_bytes;
    runtime.limits.capacity.string_bytes = expected.len() * 2 - 1;
    let retained = runtime.value_bytes;
    fatal(join(&mut runtime, &value, separator).unwrap_err(), "String limit");
    assert_raw(&runtime.own_units_property(saved, &[0xd800]).unwrap(), &[0xdfff]);
    assert_eq!(runtime.binding_value(runtime.global_environment, "typedJoinCalls").unwrap().to_number(), 1.0);
    let Value::Object(id) = value else { panic!("Expected typed view") };
    assert_eq!(runtime.typed_read(runtime.typed_view(id).unwrap(), 0).unwrap(), 9.0);
    assert!(runtime.value_bytes >= retained + 4, "Published raw key/value bytes must survive workspace cleanup");
    assert_idle(&runtime);
    runtime.limits.capacity.string_bytes = original_limit;
    runtime.begin_job();
    assert_raw(&join(&mut runtime, &Value::Object(id), high()).unwrap(), &expected);
    assert_eq!(runtime.binding_value(runtime.global_environment, "typedJoinCalls").unwrap().to_number(), 1.0);
    assert_idle(&runtime);
}
