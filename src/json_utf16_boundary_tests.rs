//! Original allocation, source, and callback boundary checks for exact JSON strings.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("JSON boundary checks must not read a native host");
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("JSON boundary checks must not write a native host");
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("JSON boundary checks must not call a native host");
    }
}

fn heap_state(runtime: &Runtime) -> (usize, usize, usize) {
    (runtime.objects.len(), runtime.properties, runtime.value_bytes)
}

fn raw_source(prefix: &str, units: &[u16], suffix: &str) -> Value {
    let mut text: Vec<u16> = prefix.encode_utf16().collect();
    text.extend_from_slice(units);
    text.extend(suffix.encode_utf16());
    Value::StringUnits(text)
}

fn object_id(value: Value) -> usize {
    let Value::Object(id) = value else { panic!("Expected a parsed object") };
    id
}

fn assert_units(value: &Value, expected: &[u16]) {
    let Value::StringUnits(units) = value else {
        panic!("Expected an exact unpaired string, got {value:?}")
    };
    assert_eq!(units, expected);
}

fn fatal(fault: Fault, expected: &str) {
    assert!(fault.exception.is_none(), "Expected a fatal resource refusal: {}", fault.display());
    assert!(fault.message.contains(expected), "{}", fault.display());
}

fn execute_ok(runtime: &mut Runtime, source: &str) {
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn json_utf16_decoded_raw_storage_checks_the_converted_prefix_at_the_exact_cap() {
    let mut runtime = Runtime::for_test();
    runtime.limits.capacity.string_bytes = 48;
    let before = heap_state(&runtime);
    let fit = Value::String(format!("\"{}\\ud800\"", "a".repeat(23)));
    let value = runtime.json_parse(std::slice::from_ref(&fit)).unwrap();
    let mut expected = vec![b'a' as u16; 23];
    expected.push(0xd800);
    assert_units(&value, &expected);
    assert_eq!(value.bytes(), 48);
    assert_eq!(heap_state(&runtime), before);

    let oversized = Value::String(format!("\"{}\\ud800\"", "a".repeat(24)));
    fatal(runtime.json_parse(&[oversized]).unwrap_err(), "String limit");
    assert_eq!(heap_state(&runtime), before);
    runtime.begin_job();
    assert_units(&runtime.json_parse(&[fit]).unwrap(), &expected);
}

#[test]
fn json_utf16_pending_valid_pair_keeps_scalar_admission_without_prefix_doubling() {
    let mut runtime = Runtime::for_test();
    runtime.limits.capacity.string_bytes = 48;
    let source = format!("\"{}\\ud83d\\ude00\"", "a".repeat(33));
    assert_eq!(source.len(), 47);
    let value = runtime.json_parse(&[Value::String(source)]).unwrap();
    let Value::String(text) = value else { panic!("A well-formed pair must stay scalar") };
    assert_eq!(text, format!("{}😀", "a".repeat(33)));
    assert_eq!(text.len(), 37);
    // A temporary UTF-16 copy of this prefix would exceed the same byte cap.
    assert!(text.encode_utf16().count() * 2 > runtime.limits.capacity.string_bytes);
}

#[test]
fn json_utf16_raw_source_admission_counts_bytes_separately_from_positions() {
    let mut runtime = Runtime::for_test();
    let source = raw_source("\"", &[0xd800], "\"");
    assert_eq!(source.bytes(), 6);
    let before = heap_state(&runtime);
    runtime.limits.capacity.string_bytes = 5;
    fatal(runtime.json_parse(std::slice::from_ref(&source)).unwrap_err(), "JSON source limit");
    assert_eq!(heap_state(&runtime), before);
    runtime.begin_job();
    runtime.limits.capacity.string_bytes = 6;
    assert_units(&runtime.json_parse(&[source]).unwrap(), &[0xd800]);
    assert_eq!(heap_state(&runtime), before);
}

#[test]
fn json_utf16_raw_property_names_share_the_exact_name_byte_cap() {
    let mut runtime = Runtime::for_test();
    runtime.limits.capacity.property_bytes = 4;
    let value = runtime.json_parse(&[Value::String(r#"{"a\ud800":7}"#.into())]).unwrap();
    let id = object_id(value);
    assert_eq!(runtime.own_units_property(id, &[b'a' as u16, 0xd800]).unwrap().to_text(), "7");
    let before = heap_state(&runtime);
    fatal(runtime.json_parse(&[Value::String(r#"{"ab\ud800":9}"#.into())]).unwrap_err(),
        "Property-name limit");
    assert_eq!(heap_state(&runtime), before);
    runtime.begin_job();
    let value = runtime.json_parse(&[raw_source("{\"a", &[0xd800], "\":11}")]).unwrap();
    let id = object_id(value);
    assert_eq!(runtime.own_units_property(id, &[b'a' as u16, 0xd800]).unwrap().to_text(), "11");
}

#[test]
fn json_utf16_malformed_suffix_rolls_back_raw_allocations_without_refunding_work() {
    let mut runtime = Runtime::for_test();
    let before = heap_state(&runtime);
    let initial_work = runtime.copy_bytes;
    let initial_steps = runtime.steps;
    let inputs = [
        Value::String(r#"{"\ud800":"\udfff","child":[1,{"\ud801":2}],}"#.into()),
        raw_source("{\"", &[0xd800], "\":\"\\udfff\",\"child\":[1,{\"\\ud801\":2}],}"),
    ];
    for input in &inputs {
        let fault = runtime.json_parse(std::slice::from_ref(input)).unwrap_err();
        assert!(matches!(fault.exception.as_deref(),
            Some(super::super::Exception::Error("SyntaxError"))),
            "Expected a catchable syntax error: {}", fault.display());
        assert_eq!(heap_state(&runtime), before);
        assert_eq!(runtime.json_scratch_bytes, 0);
    }
    assert!(runtime.copy_bytes < initial_work);
    assert!(runtime.steps < initial_steps);
    runtime.begin_job();
    let value = runtime.json_parse(&[Value::String(r#"{"\ud800":"\udfff"}"#.into())]).unwrap();
    let id = object_id(value);
    assert_units(&runtime.own_units_property(id, &[0xd800]).unwrap(), &[0xdfff]);
}

#[test]
fn json_utf16_document_property_refusal_releases_all_new_raw_properties() {
    let mut runtime = Runtime::for_test();
    let before = heap_state(&runtime);
    let original_limit = runtime.limits.heap.properties;
    runtime.limits.heap.properties = runtime.properties + 1;
    let source = Value::String(r#"{"\ud800":"\udfff","\ud801":"\udc00"}"#.into());
    let work = runtime.copy_bytes;
    fatal(runtime.json_parse(std::slice::from_ref(&source)).unwrap_err(), "Object property limit");
    assert_eq!(heap_state(&runtime), before);
    assert!(runtime.copy_bytes < work);
    runtime.limits.heap.properties = original_limit;
    runtime.begin_job();
    let id = object_id(runtime.json_parse(&[source]).unwrap());
    assert_units(&runtime.own_units_property(id, &[0xd800]).unwrap(), &[0xdfff]);
    assert_units(&runtime.own_units_property(id, &[0xd801]).unwrap(), &[0xdc00]);
    assert_eq!(runtime.properties, before.1 + 2);
}

#[test]
fn json_utf16_retained_key_and_value_bytes_refuse_atomically_and_recover() {
    let mut runtime = Runtime::for_test();
    let before = heap_state(&runtime);
    let original_limit = runtime.limits.heap.retained_values;
    // One two-byte key and one two-byte value fit. A second pair does not.
    runtime.limits.heap.retained_values = runtime.value_bytes + 4;
    let source = Value::String(r#"{"\ud800":"\udfff","\ud801":"\udc00"}"#.into());
    fatal(runtime.json_parse(std::slice::from_ref(&source)).unwrap_err(), "Retained value limit");
    assert_eq!(heap_state(&runtime), before);
    runtime.limits.heap.retained_values = original_limit;
    runtime.begin_job();
    let id = object_id(runtime.json_parse(&[source]).unwrap());
    assert_units(&runtime.own_units_property(id, &[0xd800]).unwrap(), &[0xdfff]);
    assert_units(&runtime.own_units_property(id, &[0xd801]).unwrap(), &[0xdc00]);
    assert_eq!(runtime.value_bytes, before.2 + 8);
}

#[test]
fn json_utf16_late_copy_refusal_restores_allocations_but_keeps_the_spent_budget() {
    let source = Value::String(r#"{"\ud800":"\udfff","nested":[{"\ud801":"value"}]}"#.into());
    let mut measured = Runtime::for_test();
    let initial_work = measured.copy_bytes;
    measured.json_parse(std::slice::from_ref(&source)).unwrap();
    let required_work = initial_work - measured.copy_bytes;
    assert!(required_work > source.bytes());

    let mut runtime = Runtime::for_test();
    let before = heap_state(&runtime);
    let steps = runtime.steps;
    runtime.copy_bytes = required_work - 1;
    fatal(runtime.json_parse(std::slice::from_ref(&source)).unwrap_err(), "String/copy work limit");
    assert_eq!(runtime.copy_bytes, 0);
    assert!(runtime.steps < steps);
    assert_eq!(heap_state(&runtime), before);
    assert_eq!(runtime.json_scratch_bytes, 0);
    runtime.begin_job();
    let id = object_id(runtime.json_parse(&[source]).unwrap());
    assert_units(&runtime.own_units_property(id, &[0xd800]).unwrap(), &[0xdfff]);
}

#[test]
fn json_utf16_duplicate_snapshots_replace_old_payload_and_keep_the_last_token() {
    let single = r#"{"\ud800":"\udfff","later":1}"#;
    let repeated = r#"{"\ud800":"a longer previous value","later":1,"\ud800":"\udfff"}"#;
    let mut first = Runtime::for_test();
    let (_, first_snapshots, first_bytes) = first.json_parse_source(Source::Scalar(single), true).unwrap();
    let mut second = Runtime::for_test();
    let (value, snapshots, bytes) = second.json_parse_source(Source::Scalar(repeated), true).unwrap();
    let id = object_id(value);
    assert_eq!(bytes, first_bytes, "A duplicate key must release its previous snapshot payload charge");
    assert_eq!(snapshots.len(), first_snapshots.len());
    assert_eq!(snapshots.values().map(|holder| holder.len()).sum::<usize>(), 3);
    let snapshot = snapshots.get(&id).unwrap().get(&StringKey::Units(vec![0xd800])).unwrap();
    assert_units(&snapshot.value, &[0xdfff]);
    assert_eq!(&repeated[snapshot.start..snapshot.end], r#""\udfff""#);
    assert_eq!(snapshot.start, repeated.rfind(r#""\udfff""#).unwrap());
    assert_eq!(second.enumerable_keys(id).unwrap(), vec![
        StringKey::Units(vec![0xd800]), StringKey::Scalar("later".into()),
    ]);
    assert_eq!(second.value_bytes, first.value_bytes);
}

#[test]
fn json_utf16_snapshot_refusal_restores_outer_scratch_and_allows_a_later_parse() {
    let mut runtime = Runtime::for_test();
    let callback = runtime.execute("function keep(key,value){return value;} keep;", &mut NoHost).unwrap();
    let before = heap_state(&runtime);
    let occupied = runtime.limits.heap.retained_values - 1;
    runtime.json_scratch_bytes = occupied;
    let source = raw_source("\"", &[0xd800], "\"");
    fatal(runtime.json_parse_builtin(&[source.clone(), callback.clone()], &mut NoHost).unwrap_err(),
        "JSON reviver snapshot limit");
    assert_eq!(runtime.json_scratch_bytes, occupied);
    assert_eq!(heap_state(&runtime), before);
    assert_eq!(runtime.call_depth, 0);
    assert!(runtime.transient_registration_is_empty());
    runtime.json_scratch_bytes = 0;
    runtime.begin_job();
    assert_units(&runtime.json_parse_builtin(&[source, callback], &mut NoHost).unwrap(), &[0xd800]);
    assert_eq!(runtime.json_scratch_bytes, 0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn json_utf16_nested_revivers_preserve_source_tokens_and_restore_outer_scratch() {
    let mut runtime = Runtime::for_test();
    let occupied = 127;
    runtime.json_scratch_bytes = occupied;
    execute_ok(&mut runtime, r#"
var high='\ud800',low='\udfff',outerText='{"'+high+'":"'+low+'"}',outerCalls=0,innerCalls=0;
var value=JSON.parse(outerText,function(key,value,context){
    outerCalls++;
    if(key===high){
        if(this[key]!==low||context.source!=='"'+low+'"')throw 'outer raw source';
        var inner=JSON.parse('{"\\ud801":"\\udc00"}',function(k,v,c){
            innerCalls++;
            if(k==='\ud801'&&(this[k]!==v||c.source!=='"\\udc00"'))throw 'inner escaped source';
            return v;
        });
        if(inner['\ud801']!=='\udc00'||context.source!=='"'+low+'"')throw 'nested source lifetime';
    }
    return value;
});
if(value[high]!==low||outerCalls!==2||innerCalls!==2)throw 'nested raw result';
'ok';
"#);
    assert_eq!(runtime.json_scratch_bytes, occupied);
}

#[test]
fn json_utf16_source_conversion_publications_survive_later_syntax_errors() {
    let mut runtime = Runtime::for_test();
    execute_ok(&mut runtime, r#"
var key='\ud800',raw='\udfff',saved,calls=0,reviverCalls=0,caught=false,argument={};
argument.toString=function(){
    calls++;saved={};saved[key]=raw;
    return '{"'+key+'":"'+raw+'",}';
};
argument.valueOf=function(){throw 'unexpected second conversion';};
try{JSON.parse(argument,function(k,v){reviverCalls++;return v;});}
catch(error){caught=error.name==='SyntaxError';}
if(!caught||calls!==1||reviverCalls!==0||saved[key]!==raw)throw 'published conversion state';
if(Object.keys(saved).length!==1||Object.keys(saved)[0]!==key)throw 'published raw key';
if(JSON.parse('"\\ud801"')!=='\ud801')throw 'parse recovery';
'ok';
"#);
    assert_eq!(runtime.json_scratch_bytes, 0);
}

#[test]
fn json_utf16_reviver_publications_survive_abrupt_completion_and_keep_raw_children() {
    let mut runtime = Runtime::for_test();
    let occupied = 91;
    runtime.json_scratch_bytes = occupied;
    execute_ok(&mut runtime, r#"
var key='\ud800',raw='\udfff',saved,marker={},seen=0,caught=false;
var text='{"'+key+'":"'+raw+'","child":{"\\ud801":"\\udc00"}}';
try{
    JSON.parse(text,function(k,v,c){
        seen++;
        if(k===key){
            saved=this;
            if(c.source!=='"'+raw+'"')throw 'source before publication';
            throw marker;
        }
        return v;
    });
}catch(error){caught=error===marker;}
if(!caught||seen!==1||saved[key]!==raw||saved.child['\ud801']!=='\udc00')throw 'published parsed holder';
saved[key]='\ud802';
if(saved[key]!=='\ud802'||JSON.parse(JSON.stringify(saved))[key]!=='\ud802')throw 'published holder recovery';
'ok';
"#);
    assert_eq!(runtime.json_scratch_bytes, occupied);
}

#[test]
fn json_utf16_raw_source_values_and_keys_admit_final_units_without_transient_scalar_expansion() {
    let mut runtime = Runtime::for_test();
    runtime.limits.capacity.string_bytes = 256;
    let mut units = vec![0x0800; 100];
    units.push(0xd800);
    let source = raw_source("\"", &units, "\"");
    assert_eq!(source.bytes(), 206);
    let value = runtime.json_parse(&[source]).unwrap();
    assert_units(&value, &units);
    assert_eq!(value.bytes(), 202);

    let source = raw_source("{\"", &units, "\":7}");
    assert_eq!(source.bytes(), 214);
    let id = object_id(runtime.json_parse(&[source]).unwrap());
    assert_eq!(runtime.own_units_property(id, &units).unwrap().to_text(), "7");
    assert_eq!(runtime.enumerable_keys(id).unwrap(), vec![StringKey::Units(units)]);
}

#[test]
fn json_utf16_well_formed_raw_source_applies_the_final_scalar_output_cap() {
    let mut runtime = Runtime::for_test();
    let source = raw_source("\"", &[0x0800; 100], "\"");
    assert_eq!(source.bytes(), 204);
    let before = heap_state(&runtime);
    runtime.limits.capacity.string_bytes = 256;
    fatal(runtime.json_parse(std::slice::from_ref(&source)).unwrap_err(), "String limit");
    assert_eq!(heap_state(&runtime), before);

    runtime.begin_job();
    runtime.limits.capacity.string_bytes = 300;
    let value = runtime.json_parse(&[source]).unwrap();
    let Value::String(text) = value else { panic!("Well-formed raw source must canonicalize to scalar text") };
    assert_eq!(text, "\u{0800}".repeat(100));
    assert_eq!(text.len(), 300);
    assert_eq!(heap_state(&runtime), before);

    let mut paired = vec![b'a' as u16; 32];
    paired.extend_from_slice(&[0xd83d, 0xde00]);
    let source = raw_source("\"", &paired, "\"");
    assert_eq!(source.bytes(), 72);
    runtime.limits.capacity.string_bytes = 72;
    let value = runtime.json_parse(&[source]).unwrap();
    let Value::String(text) = value else { panic!("A paired raw source must have scalar identity") };
    assert_eq!(text, format!("{}😀", "a".repeat(32)));
    assert_eq!(text.len(), 36);
}
