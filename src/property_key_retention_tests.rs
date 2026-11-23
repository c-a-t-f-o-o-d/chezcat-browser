//! Exact-key storage, bounded accounting, and strong-edge regression witnesses.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected native get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected native set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected native call") }
}

fn allocation_state(runtime: &Runtime, id: usize) -> (usize, usize, usize, usize) {
    (runtime.properties, runtime.value_bytes, runtime.objects[id].next_order,
        runtime.objects[id].properties.len())
}

#[test]
fn utf16_key_payload_updates_and_deletions_refund_exact_bytes_without_reusing_order() {
    let mut runtime = Runtime::for_test();
    let id = runtime.new_object(None).unwrap();
    let base = (runtime.properties, runtime.value_bytes);
    let key = [0xd800, 0x41];
    runtime.store_units_property(id, key.to_vec(), Value::String("payload".into()), None).unwrap();
    assert_eq!((runtime.properties, runtime.value_bytes), (base.0 + 1, base.1 + 4 + 7));
    let first_order = runtime.objects[id].properties.get_units(&key).unwrap().order;
    runtime.store_units_property(id, key.to_vec(), Value::StringUnits(vec![0xdc00, 0x78]), None).unwrap();
    assert_eq!((runtime.properties, runtime.value_bytes), (base.0 + 1, base.1 + 4 + 4));
    assert_eq!(runtime.objects[id].properties.get_units(&key).unwrap().order, first_order);
    runtime.remove_units_property(id, &key).unwrap();
    assert_eq!((runtime.properties, runtime.value_bytes), base);
    assert!(runtime.objects[id].properties.is_empty());
    runtime.store_units_property(id, key.to_vec(), Value::Number(9.0), None).unwrap();
    assert_eq!((runtime.properties, runtime.value_bytes), (base.0 + 1, base.1 + 4));
    assert!(runtime.objects[id].properties.get_units(&key).unwrap().order > first_order);
}

#[test]
fn utf16_keys_share_scalar_and_symbol_property_caps_and_failure_is_atomic() {
    let mut runtime = Runtime::for_test();
    let Value::Symbol(symbol) = runtime.execute("Symbol('private-description');", &mut NoHost).unwrap() else {
        panic!("expected symbol");
    };
    let id = runtime.new_object(None).unwrap();
    runtime.limits.heap.object_properties = 3;
    runtime.set_property(id, "scalar".into(), Value::Number(1.0)).unwrap();
    runtime.store_units_property(id, vec![0xd800], Value::Number(2.0), None).unwrap();
    runtime.store_symbol_property(id, symbol, Value::Number(3.0), None).unwrap();
    let before = allocation_state(&runtime, id);
    let error = runtime.store_units_property(id, vec![0xdc00], Value::Number(4.0), None).unwrap_err();
    assert!(error.message.contains("Object property limit"));
    assert_eq!(allocation_state(&runtime, id), before);
    assert!(!error.message.contains("private-description"));
    runtime.store_units_property(id, vec![0xd800], Value::Number(5.0), None).unwrap();
    assert_eq!(allocation_state(&runtime, id), before);
    runtime.remove_property(id, "scalar").unwrap();
    runtime.store_units_property(id, vec![0xdc00], Value::Number(6.0), None).unwrap();
    assert_eq!(runtime.properties, before.0);
    assert_eq!(runtime.objects[id].properties.len(), 2);
    assert_eq!(runtime.objects[id].symbol_properties.len(), 1);
}

#[test]
fn utf16_key_name_payload_and_copy_refusals_leave_storage_and_order_unchanged() {
    let mut runtime = Runtime::for_test();
    let id = runtime.new_object(None).unwrap();
    runtime.limits.capacity.property_bytes = 4;
    runtime.store_units_property(id, vec![0xd800, 0x41], Value::Number(1.0), None).unwrap();
    let before = allocation_state(&runtime, id);
    let error = runtime.store_units_property(id, vec![0xdc00, 0x41, 0x42], Value::Number(2.0), None).unwrap_err();
    assert!(error.message.contains("Property-name limit"));
    assert_eq!(allocation_state(&runtime, id), before);
    runtime.limits.heap.retained_values = runtime.value_bytes + 3;
    let error = runtime.store_units_property(id, vec![0xdc00], Value::String("ab".into()), None).unwrap_err();
    assert!(error.message.contains("Retained value limit"));
    assert_eq!(allocation_state(&runtime, id), before);
    runtime.limits.heap.retained_values += 16;
    runtime.copy_bytes = 2; // Admission consumes the key's two bytes first.
    let error = runtime.store_units_property(id, vec![0xdc00], Value::String("ab".into()), None).unwrap_err();
    assert!(error.message.contains("String/copy work limit"));
    assert_eq!(allocation_state(&runtime, id), before);
}

#[test]
fn utf16_key_enumeration_merges_creation_order_and_nonenumerable_shadows() {
    let mut runtime = Runtime::for_test();
    let id = runtime.new_object(None).unwrap();
    runtime.set_property(id, "alpha".into(), Value::Number(0.0)).unwrap();
    runtime.store_units_property(id, vec![0xd800], Value::Number(1.0), None).unwrap();
    runtime.set_property(id, "9".into(), Value::Number(9.0)).unwrap();
    runtime.set_property(id, "2".into(), Value::Number(2.0)).unwrap();
    runtime.set_property(id, "\u{fffd}".into(), Value::Number(3.0)).unwrap();
    runtime.store_units_property(id, vec![0xdc00], Value::Number(4.0), None).unwrap();
    assert_eq!(runtime.own_string_keys(id, false).unwrap(), vec![
        "2".into(), "9".into(), "alpha".into(), StringKey::Units(vec![0xd800]),
        "\u{fffd}".into(), StringKey::Units(vec![0xdc00]),
    ]);
    runtime.remove_units_property(id, &[0xd800]).unwrap();
    runtime.store_units_property(id, vec![0xd800], Value::Number(5.0), None).unwrap();
    assert_eq!(runtime.own_string_keys(id, false).unwrap().last(), Some(&StringKey::Units(vec![0xd800])));

    let parent = runtime.new_object(None).unwrap();
    let child = runtime.new_object(None).unwrap();
    runtime.objects[child].prototype = Some(Ordinary::Object(parent));
    runtime.store_units_property(parent, vec![0xd800], Value::Number(1.0), None).unwrap();
    runtime.store_units_property(parent, vec![0xdc00], Value::Number(2.0), None).unwrap();
    runtime.store_units_property(child, vec![0xd800], Value::Number(3.0), Some((false, true, true))).unwrap();
    runtime.set_property(child, "own".into(), Value::Number(4.0)).unwrap();
    assert_eq!(runtime.inherited_keys(Ordinary::Object(child)).unwrap(),
        vec![StringKey::Scalar("own".into()), StringKey::Units(vec![0xdc00])]);
}

#[test]
fn utf16_array_properties_survive_length_truncation_and_participate_in_freeze_and_seal() {
    let mut runtime = Runtime::for_test();
    let id = runtime.new_object(Some(5)).unwrap();
    let key = [0xd800, 0x34];
    runtime.set_property(id, "0".into(), Value::Number(10.0)).unwrap();
    runtime.set_property(id, "4".into(), Value::Number(14.0)).unwrap();
    runtime.store_units_property(id, key.to_vec(), Value::String("kept".into()), None).unwrap();
    let before = (runtime.properties, runtime.value_bytes);
    runtime.set_array_length(id, 0).unwrap();
    assert_eq!((runtime.properties, runtime.value_bytes), (before.0 - 2, before.1 - 2));
    assert_eq!(runtime.objects[id].length, Some(0));
    assert_eq!(runtime.own_units_property(id, &key).unwrap().to_text(), "kept");
    runtime.object_integrity("freeze", &Value::Object(id)).unwrap();
    assert!(runtime.object_integrity("isFrozen", &Value::Object(id)).unwrap().truthy());
    let property = runtime.objects[id].properties.get_units(&key).unwrap();
    assert!(!property.writable && !property.configurable);
    let before = allocation_state(&runtime, id);
    runtime.strict = true;
    assert!(runtime.store_units_property(id, key.to_vec(), Value::Number(7.0), None).is_err());
    assert_eq!(allocation_state(&runtime, id), before);
    runtime.strict = false;

    let sealed = runtime.new_object(None).unwrap();
    runtime.store_units_property(sealed, vec![0xdc00], Value::Number(1.0), None).unwrap();
    runtime.object_integrity("seal", &Value::Object(sealed)).unwrap();
    assert!(runtime.object_integrity("isSealed", &Value::Object(sealed)).unwrap().truthy());
    assert!(!runtime.object_integrity("isFrozen", &Value::Object(sealed)).unwrap().truthy());
    runtime.store_units_property(sealed, vec![0xdc00], Value::Number(2.0), None).unwrap();
    assert_eq!(runtime.own_units_property(sealed, &[0xdc00]).unwrap().to_number(), 2.0);
    assert!(!runtime.objects[sealed].properties.get_units(&[0xdc00]).unwrap().configurable);
}

#[test]
fn utf16_data_and_accessor_edges_retain_closures_and_scopes_until_deleted() {
    for container in ["{}", "[]"] {
        let mut runtime = Runtime::for_capacity_test();
        let source = format!(r#"
var holder={container};
(function(seed){{
    holder['\ud800']=function(){{return seed;}};
    Object.defineProperty(holder,'\udc00',{{
        get:function(){{return seed+1;}},
        set:function(value){{seed=value;}},
        enumerable:true,configurable:true
    }});
}})(41);0;
"#);
        runtime.execute(&source, &mut NoHost).unwrap();
        let Value::Object(holder) = runtime.binding_value(runtime.global_environment, "holder").unwrap() else {
            panic!("expected holder");
        };
        let Value::Function(data) = runtime.own_units_property(holder, &[0xd800]).unwrap() else {
            panic!("expected closure");
        };
        let accessor = runtime.objects[holder].properties.get_units(&[0xdc00]).unwrap().accessor.unwrap();
        let functions = [data, accessor.get.unwrap(), accessor.set.unwrap()];
        let environments: Vec<_> = functions.iter().map(|&id| {
            let Function::Script { environment, .. } = &runtime.functions[id] else { panic!("expected script closure"); };
            *environment
        }).collect();
        let before = (runtime.properties, runtime.value_bytes, runtime.job_usage());
        let graph = runtime.mark_persistent_functions(&[], false).unwrap();
        assert!(graph.objects[holder]);
        assert!(functions.iter().all(|&id| graph.functions[id]));
        assert!(environments.iter().all(|&id| graph.environments[id]));
        assert_eq!((runtime.properties, runtime.value_bytes, runtime.job_usage()), before);
        runtime.execute(r#"
if(holder['\ud800']()!==41||holder['\udc00']!==42)throw 'raw closure state';
holder['\udc00']=90;
if(holder['\ud800']()!==90||holder['\udc00']!==91)throw 'raw shared scope';
delete holder['\ud800'];delete holder['\udc00'];0;
"#, &mut NoHost).unwrap();
        let graph = runtime.mark_persistent_functions(&[], false).unwrap();
        assert!(functions.iter().all(|&id| !graph.functions[id]));
        assert!(environments.iter().all(|&id| !graph.environments[id]));
        assert!(runtime.object_assigned_closures.contains(&data));
        runtime.retire_proven_inactive_candidate(data).unwrap();
        assert!(!runtime.function_is_occupied(data));
        assert!(runtime.transient_registration_is_empty());
    }
}
