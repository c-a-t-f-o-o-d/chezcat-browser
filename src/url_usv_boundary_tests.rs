//! Original resource and capability boundary checks for URL USVString conversion.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("URL conversion must not read a native host");
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("URL conversion must not write a native host");
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("URL conversion must not call a native host");
    }
}

fn initialized_runtime() -> Runtime {
    let mut runtime = Runtime::for_test();
    runtime.binding_value(runtime.global_environment, "URL").unwrap();
    runtime.binding_value(runtime.global_environment, "URLSearchParams").unwrap();
    runtime
}

fn object_id(value: Value) -> usize {
    let Value::Object(id) = value else { panic!("Expected a genuine URL object") };
    id
}

fn attached_runtime() -> (Runtime, usize, usize) {
    let mut runtime = initialized_runtime();
    let value = runtime.url_construct(
        Kind::Url,
        &[Value::String("https://example.org/a?x=old#kept".into())],
        &mut NoHost,
    ).unwrap();
    let url = object_id(value);
    let params = runtime.url_record(url).unwrap().1;
    (runtime, url, params)
}

#[derive(Debug, PartialEq, Eq)]
struct SlotState {
    href: String,
    pairs: Vec<Pair>,
    params: usize,
    owner: Option<usize>,
    slot_bytes: usize,
    retained: usize,
    objects: usize,
    properties: usize,
}

fn slot_state(runtime: &Runtime, url: usize, params: usize) -> SlotState {
    let (value, attached) = runtime.url_record(url).unwrap();
    let (pairs, owner) = runtime.params_data(params).unwrap();
    SlotState {
        href: value.href(),
        pairs: pairs.clone(),
        params: attached,
        owner,
        slot_bytes: runtime.url_data(url).unwrap().bytes()
            + runtime.url_data(params).unwrap().bytes(),
        retained: runtime.value_bytes,
        objects: runtime.objects.len(),
        properties: runtime.properties,
    }
}

fn fatal(fault: Fault, expected: &str) {
    assert!(fault.exception.is_none(), "Expected a fatal limit/refusal: {}", fault.display());
    assert!(fault.message.contains(expected), "{}", fault.display());
}

#[test]
fn usv_conversion_charges_input_scan_and_expanded_output_before_allocation() {
    let mut runtime = initialized_runtime();
    let units = [0xd800, b'a' as u16, 0xdc00, 0xd83d, 0xde00];
    let expected = "\u{fffd}a\u{fffd}\u{1f600}";
    let conversion_work = units.len() * 2 + expected.len();
    let before = (runtime.value_bytes, runtime.objects.len(), runtime.properties);
    for budget in [units.len() * 2 - 1, conversion_work - 1] {
        runtime.begin_job();
        runtime.copy_bytes = budget;
        fatal(runtime.url_usv_units(&units).unwrap_err(), "String/copy work limit");
        assert_eq!(runtime.copy_bytes, 0);
        assert_eq!((runtime.value_bytes, runtime.objects.len(), runtime.properties), before);
    }
    runtime.begin_job();
    runtime.copy_bytes = conversion_work;
    assert_eq!(runtime.url_usv_units(&units).unwrap(), expected);
    assert_eq!(runtime.copy_bytes, 0);

    runtime.begin_job();
    runtime.copy_bytes = units.len() * 2 + conversion_work;
    assert_eq!(runtime.url_string(&Value::StringUnits(units.to_vec()), &mut NoHost).unwrap(), expected);
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!((runtime.value_bytes, runtime.objects.len(), runtime.properties), before);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn usv_conversion_applies_string_cap_to_scalar_output_not_raw_unit_storage() {
    let mut runtime = initialized_runtime();
    let original_limit = runtime.limits.capacity.string_bytes;
    runtime.limits.capacity.string_bytes = 2;
    fatal(runtime.url_string(&Value::StringUnits(vec![0xd800]), &mut NoHost).unwrap_err(), "String limit");
    runtime.begin_job();
    runtime.limits.capacity.string_bytes = 3;
    assert_eq!(runtime.url_string(&Value::StringUnits(vec![0xd800]), &mut NoHost).unwrap(), "\u{fffd}");
    fatal(runtime.url_usv_units(&[0xd83d, 0xde00]).unwrap_err(), "String limit");
    runtime.begin_job();
    runtime.limits.capacity.string_bytes = 4;
    assert_eq!(runtime.url_usv_units(&[0xd83d, 0xde00]).unwrap(), "\u{1f600}");
    // Valid raw units can occupy more internal bytes than their scalar output.
    assert_eq!(runtime.url_usv_units(&[b'a' as u16; 4]).unwrap(), "aaaa");
    runtime.limits.capacity.string_bytes = original_limit;
    runtime.begin_job();
    assert_eq!(runtime.url_usv_units(&[0xdc00]).unwrap(), "\u{fffd}");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn usv_copy_refusal_preserves_attached_slots_and_recovers_in_a_fresh_job() {
    let (mut runtime, url, params) = attached_runtime();
    let before = slot_state(&runtime, url, params);
    let input = Value::StringUnits(vec![0xd800]);
    runtime.copy_bytes = input.bytes();
    fatal(runtime.url_builtin(Op::Set("search"), &[input.clone()], Value::Object(url), &mut NoHost)
        .unwrap_err(), "String/copy work limit");
    assert_eq!(slot_state(&runtime, url, params), before);
    assert_eq!(runtime.call_depth, 0);
    assert!(runtime.transient_registration_is_empty());

    runtime.begin_job();
    runtime.url_builtin(Op::Set("search"), &[input], Value::Object(url), &mut NoHost).unwrap();
    let after = slot_state(&runtime, url, params);
    assert_eq!(after.href, "https://example.org/a?%EF%BF%BD#kept");
    assert_eq!(after.pairs, vec![("\u{fffd}".into(), String::new())]);
    assert_eq!((after.params, after.owner), (params, Some(url)));
    assert_eq!((after.objects, after.properties), (before.objects, before.properties));
    assert_eq!(after.retained, before.retained - before.slot_bytes + after.slot_bytes);
}

#[test]
fn usv_percent_expansion_cannot_overflow_url_or_attached_query_limits() {
    let (mut runtime, url, params) = attached_runtime();
    let before = slot_state(&runtime, url, params);
    // Three UTF-8 bytes become nine percent-encoded bytes for each replacement.
    let oversized = Value::StringUnits(vec![0xd800; MAX_URL_BYTES / 9 + 1]);
    for attached_query in [false, true] {
        runtime.begin_job();
        let result = if attached_query {
            runtime.url_builtin(Op::Params("append"), &[Value::String("new".into()), oversized.clone()],
                Value::Object(params), &mut NoHost)
        } else {
            runtime.url_builtin(Op::Set("pathname"), &[oversized.clone()], Value::Object(url), &mut NoHost)
        };
        fatal(result.unwrap_err(), "URL length limit");
        assert_eq!(slot_state(&runtime, url, params), before);
        assert_eq!(runtime.call_depth, 0);
        assert!(runtime.transient_registration_is_empty());
    }
    runtime.begin_job();
    runtime.url_builtin(Op::Params("append"),
        &[Value::String("new".into()), Value::StringUnits(vec![0xdc00])],
        Value::Object(params), &mut NoHost).unwrap();
    let after = slot_state(&runtime, url, params);
    assert_eq!(after.href, "https://example.org/a?x=old&new=%EF%BF%BD#kept");
    assert_eq!(after.pairs[1], ("new".into(), "\u{fffd}".into()));
    assert_eq!((after.params, after.owner), (params, Some(url)));
    assert_eq!(after.retained, before.retained - before.slot_bytes + after.slot_bytes);
}

#[test]
fn usv_retained_value_refusal_is_atomic_across_url_and_query_slots() {
    let (mut runtime, url, params) = attached_runtime();
    let before = slot_state(&runtime, url, params);
    let old_limit = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = before.retained;
    let args = [Value::String("new".into()), Value::StringUnits(vec![0xd800, 0xdc00, 0xd800])];
    fatal(runtime.url_builtin(Op::Params("append"), &args, Value::Object(params), &mut NoHost)
        .unwrap_err(), "Retained value limit");
    assert_eq!(slot_state(&runtime, url, params), before);
    assert_eq!(runtime.call_depth, 0);
    assert!(runtime.transient_registration_is_empty());

    runtime.limits.heap.retained_values = old_limit;
    runtime.begin_job();
    runtime.url_builtin(Op::Params("append"), &args, Value::Object(params), &mut NoHost).unwrap();
    let after = slot_state(&runtime, url, params);
    assert_eq!(after.pairs[1], ("new".into(), "\u{10000}\u{fffd}".into()));
    assert_eq!((after.params, after.owner), (params, Some(url)));
    assert_eq!((after.objects, after.properties), (before.objects, before.properties));
    assert!(after.slot_bytes > before.slot_bytes);
    assert_eq!(after.retained, before.retained - before.slot_bytes + after.slot_bytes);
}

#[test]
fn usv_query_decoded_and_serialized_byte_limits_remain_separate() {
    let mut runtime = initialized_runtime();
    let before = (runtime.value_bytes, runtime.objects.len(), runtime.properties);
    let count = MAX_PARAMS_BYTES / 3;
    fatal(runtime.url_construct(Kind::Params,
        &[Value::StringUnits(vec![0xd800; count + 1])], &mut NoHost).unwrap_err(),
        "URLSearchParams byte limit");
    assert_eq!((runtime.value_bytes, runtime.objects.len(), runtime.properties), before);
    assert_eq!(runtime.call_depth, 0);

    runtime.begin_job();
    let value = runtime.url_construct(Kind::Params,
        &[Value::StringUnits(vec![0xd800; count])], &mut NoHost).unwrap();
    let params = object_id(value);
    let (pairs, owner) = runtime.params_data(params).unwrap();
    assert_eq!(owner, None);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0.len(), count * 3);
    assert!(pairs[0].0.chars().all(|ch| ch == '\u{fffd}'));
    let retained = runtime.value_bytes;
    let storage = runtime.url_data(params).unwrap().bytes();
    // Admitted decoded text need not fit the separately bounded serialization.
    fatal(runtime.params_method(params, "toString", &[], &mut NoHost).unwrap_err(),
        "URLSearchParams byte limit");
    assert_eq!(runtime.value_bytes, retained);
    assert_eq!(runtime.url_data(params).unwrap().bytes(), storage);
    assert_eq!(runtime.params_data(params).unwrap().0[0].0.len(), count * 3);
    runtime.begin_job();
    runtime.params_method(params, "delete", &[Value::StringUnits(vec![0xd800; count])], &mut NoHost).unwrap();
    assert_eq!(runtime.params_method(params, "toString", &[], &mut NoHost).unwrap().to_text(), "");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn usv_argument_callbacks_run_only_after_native_receiver_brand_checks() {
    let mut runtime = initialized_runtime();
    let result = runtime.execute(r#"
var reads=0,calls=0,arg={},fake=Object.create(URLSearchParams.prototype);
Object.defineProperty(arg,Symbol.toPrimitive,{get:function(){
    reads++;return function(hint){if(hint!=='string'||this!==arg)throw 'conversion protocol';calls++;return '\ud800';};
}});
var setter=Object.getOwnPropertyDescriptor(URL.prototype,'href').set;
var actions=[
    function(){URLSearchParams.prototype.append.call(fake,arg,arg);},
    function(){URLSearchParams.prototype.set.call(fake,arg,arg);},
    function(){URLSearchParams.prototype.get.call(fake,arg);},
    function(){URLSearchParams.prototype.has.call(fake,arg,arg);},
    function(){URLSearchParams.prototype.delete.call(fake,arg,arg);},
    function(){setter.call(Object.create(URL.prototype),arg);}
];
var caught=0;for(var action of actions){try{action();}catch(e){if(e.name==='TypeError')caught++;else throw e;}}
if(caught!==6||reads!==0||calls!==0)throw 'brand check ordering';
var p=new URLSearchParams();p.append(arg,arg);
if(reads!==2||calls!==2||p.get('\ufffd')!=='\ufffd')throw 'genuine conversion once per argument';
'ok';
"#, &mut NoHost).unwrap();
    assert_eq!(result.to_text(), "ok");
    for op in [Op::Set("href"), Op::Params("append")] {
        let budget = runtime.copy_bytes;
        let error = runtime.url_builtin(op, &[Value::StringUnits(vec![0xd800])],
            Value::Host(77), &mut NoHost).unwrap_err();
        assert!(error.exception.is_some());
        assert!(error.message.contains("incompatible receiver"));
        assert_eq!(runtime.copy_bytes, budget);
        assert_eq!(runtime.call_depth, 0);
    }
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn usv_conversion_preserves_native_host_idna_and_scheme_refusals() {
    let (mut runtime, url, params) = attached_runtime();
    let before = slot_state(&runtime, url, params);
    fatal(runtime.url_construct(Kind::Params, &[Value::Host(77)], &mut NoHost).unwrap_err(),
        "Native-host URLSearchParams construction");
    fatal(runtime.url_construct(Kind::Url, &[Value::Host(77)], &mut NoHost).unwrap_err(),
        "No conversion permission or hook read was granted");
    for parse in [false, true] {
        for (prefix, suffix, message) in [
            ("https://", ".test/", "IDNA"),
            ("file:///", "", "only HTTP(S)"),
        ] {
            let mut units: Vec<u16> = prefix.encode_utf16().collect();
            units.push(0xd800);
            units.extend(suffix.encode_utf16());
            fatal(runtime.url_builtin(Op::Static(parse), &[Value::StringUnits(units)],
                Value::Undefined, &mut NoHost).unwrap_err(), message);
            assert_eq!(slot_state(&runtime, url, params), before);
        }
    }
    fatal(runtime.url_builtin(Op::Set("hostname"), &[Value::StringUnits(vec![0xdc00])],
        Value::Object(url), &mut NoHost).unwrap_err(), "IDNA");
    assert_eq!(slot_state(&runtime, url, params), before);
    assert_eq!(runtime.call_depth, 0);
    runtime.begin_job();
    runtime.url_builtin(Op::Set("pathname"), &[Value::StringUnits(vec![0xdc00])],
        Value::Object(url), &mut NoHost).unwrap();
    assert_eq!(runtime.url_record(url).unwrap().0.href(), "https://example.org/%EF%BF%BD?x=old#kept");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn usv_record_collision_at_pair_limit_replaces_without_adding_an_entry() {
    let mut runtime = initialized_runtime();
    let record = runtime.new_object(None).unwrap();
    for index in 0..MAX_PAIRS - 1 {
        runtime.set_property(record, format!("k{index}"), Value::String("v".into())).unwrap();
    }
    runtime.store_units_property(record, vec![0xd800], Value::String("first".into()), None).unwrap();
    runtime.store_units_property(record, vec![0xdc00], Value::String("last".into()), None).unwrap();
    runtime.begin_job();
    let before = (runtime.value_bytes, runtime.objects.len(), runtime.properties);
    let pairs = runtime.params_input(&Value::Object(record), &mut NoHost).unwrap();
    assert_eq!(pairs.len(), MAX_PAIRS);
    assert_eq!(pairs[MAX_PAIRS - 1], ("\u{fffd}".into(), "last".into()));
    assert_eq!(pairs[0], ("k0".into(), "v".into()));
    assert_eq!((runtime.value_bytes, runtime.objects.len(), runtime.properties), before);

    runtime.begin_job();
    runtime.set_property(record, "distinct".into(), Value::String("one too many".into())).unwrap();
    let before_failure = (runtime.value_bytes, runtime.objects.len(), runtime.properties);
    fatal(runtime.params_input(&Value::Object(record), &mut NoHost).unwrap_err(),
        "URLSearchParams entry limit");
    assert_eq!((runtime.value_bytes, runtime.objects.len(), runtime.properties), before_failure);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn usv_record_collision_replaces_byte_charge_but_distinct_names_share_the_cap() {
    let mut runtime = initialized_runtime();
    let record = runtime.new_object(None).unwrap();
    runtime.store_units_property(record, vec![0xd800],
        Value::String("a".repeat(40 * 1024)), None).unwrap();
    runtime.store_units_property(record, vec![0xdc00],
        Value::String("b".repeat(40 * 1024)), None).unwrap();
    runtime.begin_job();
    let before = (runtime.value_bytes, runtime.objects.len(), runtime.properties);
    let native_slots = runtime.objects.iter().filter(|object| object.url.is_some()).count();
    let pairs = runtime.params_input(&Value::Object(record), &mut NoHost).unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("\u{fffd}".into(), "b".repeat(40 * 1024)));
    assert_eq!((runtime.value_bytes, runtime.objects.len(), runtime.properties), before);
    assert_eq!(runtime.objects.iter().filter(|object| object.url.is_some()).count(), native_slots);

    runtime.set_property(record, "distinct".into(), Value::String("c".repeat(32 * 1024))).unwrap();
    runtime.begin_job();
    let before_failure = (runtime.value_bytes, runtime.objects.len(), runtime.properties);
    fatal(runtime.params_input(&Value::Object(record), &mut NoHost).unwrap_err(),
        "URLSearchParams byte limit");
    assert_eq!((runtime.value_bytes, runtime.objects.len(), runtime.properties), before_failure);
    assert_eq!(runtime.objects.iter().filter(|object| object.url.is_some()).count(), native_slots);
    assert!(runtime.transient_registration_is_empty());
}
