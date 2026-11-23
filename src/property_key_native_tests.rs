//! Raw author keys use native expando capability, never native string dispatch.
use super::*;

struct Native {
    enabled: bool,
    owner: u32,
    origin_allowed: bool,
    reads: usize,
    writes: usize,
    calls: usize,
}

impl Native {
    fn new() -> Self {
        Self { enabled: true, owner: 77, origin_allowed: true, reads: 0, writes: 0, calls: 0 }
    }
}

impl Host for Native {
    fn native_access_owner(&self, handle: u32) -> Option<u32> {
        matches!(handle, 42 | 43).then_some(self.owner)
    }
    fn expando_owner(&self, handle: u32) -> Option<u32> {
        (self.enabled && handle == 42).then_some(self.owner)
    }
    fn instance_ancestry(&self, handle: u32) -> Option<NativeAncestry> {
        self.expando_owner(handle).map(|window| NativeAncestry { window, constructor: None })
    }
    fn validate_window_access(&self, _: Option<u32>, _: u32) -> Result<(), String> {
        if self.origin_allowed { Ok(()) } else { Err("same-origin native access refused".into()) }
    }
    fn owns_property(&self, _: u32, name: &str) -> bool {
        matches!(name, "nativeField" | "revoke")
    }
    fn method(&self, handle: u32, name: &str) -> Option<&'static str> {
        (handle == 42 && name == "revoke").then_some("revoke")
    }
    fn own_keys(&mut self, _: u32) -> Result<Vec<String>, String> { Ok(Vec::new()) }
    fn own_data(&mut self, _: u32, _: &str) -> Result<Option<Value>, String> { Ok(None) }
    fn get(&mut self, handle: u32, name: &str) -> Result<Value, String> {
        assert_eq!((handle, name), (42, "nativeField"));
        self.reads += 1;
        Ok(Value::Number(7.0))
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        self.writes += 1;
        panic!("raw author key reached native setter")
    }
    fn call(&mut self, handle: u32, name: &str, _: &[Value]) -> Result<Value, String> {
        assert_eq!((handle, name), (42, "revoke"));
        self.calls += 1;
        self.enabled = false;
        Ok(Value::Undefined)
    }
}

fn fixture() -> (Runtime, Native) {
    let mut runtime = Runtime::for_test();
    runtime.bind_browser_global(77).unwrap();
    runtime.define_global("element", Value::Host(42)).unwrap();
    runtime.define_global("opaque", Value::Host(43)).unwrap();
    (runtime, Native::new())
}

#[test]
fn raw_key_native_expando_uses_exact_ordinary_keys_and_keeps_native_field_dispatch() {
    let (mut runtime, mut host) = fixture();
    runtime.execute(r#"
        var high='\ud800', low='nativeField\udc00', seenThis, reads=0;
        element[high]=17;
        Object.defineProperty(element,low,{get:function(){
            'use strict';reads++;seenThis=this;return 23;
        },enumerable:true,configurable:true});
        element['\ufffd']=29;
        if(element[high]!==17||Reflect.get(element,high)!==17||element[low]!==23||
           seenThis!==element||reads!==1||element['\ufffd']!==29||element.nativeField!==7)
            throw 'native and ordinary identity';
        var keys=Object.keys(element), names=Object.getOwnPropertyNames(element);
        if(keys.length!==3||keys[0]!==high||keys[1]!==low||keys[2]!=='\ufffd'||
           names[0]!==high||names[1]!==low||keys[0].charCodeAt(0)!==55296)
            throw 'exact native expando enumeration';
        if(!(high in element)||!Object.hasOwn(element,high)||!element.propertyIsEnumerable(high)||
           Object.getOwnPropertyDescriptor(element,high).value!==17)throw 'own native expando descriptors';
        var copy=Object.assign({},element);
        if(copy[high]!==17||copy[low]!==23||copy['\ufffd']!==29||reads!==2)
            throw 'native expando assignment';
        if(!delete element[high]||high in element||Object.hasOwn(element,high)||element[high]!==undefined)
            throw 'native expando deletion';
    "#, &mut host).unwrap();
    assert_eq!((host.reads, host.writes, host.calls), (1, 0, 0));
    assert_eq!(runtime.native_expando_owners.get(&42), Some(&77));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn raw_key_native_cached_store_revalidates_expiry_owner_and_origin_on_each_operation() {
    let (mut runtime, mut host) = fixture();
    runtime.execute(r#"
        var raw='\ud800', descriptorReads=0;
        element[raw]=17;
        var descriptor={get value(){descriptorReads++;return 99;}};
        if(typeof Reflect.get!=='function'||typeof Object.entries!=='function')throw 'initialize';
    "#, &mut host).unwrap();
    let id = runtime.host_objects[&42];
    host.enabled = false;
    for source in [
        "element[raw]",
        "Reflect.get(element,raw)",
        "element[raw]=99",
        "raw in element",
        "delete element[raw]",
        "Object.hasOwn(element,raw)",
        "Object.getOwnPropertyDescriptor(element,raw)",
        "Object.defineProperty(element,raw,descriptor)",
        "Object.keys(element)",
        "Object.getOwnPropertyNames(element)",
        "Object.getOwnPropertyDescriptors(element)",
        "Object.entries(element)",
        "Object.assign({},element)",
    ] {
        runtime.begin_job();
        let error = runtime.execute(source, &mut host).unwrap_err();
        assert!(error.contains("no longer accessible"), "{source}: {error}");
        assert_eq!(runtime.host_objects[&42], id);
        assert_eq!(runtime.native_expando_owners.get(&42), Some(&77));
        assert!(runtime.transient_registration_is_empty());
    }
    assert_eq!(runtime.binding_value(runtime.global_environment, "descriptorReads").unwrap().to_number(), 0.0);
    host.enabled = true;
    host.owner = 88;
    runtime.begin_job();
    let error = runtime.execute("element[raw]", &mut host).unwrap_err();
    assert!(error.contains("owner realm changed"), "{error}");
    assert_eq!(runtime.native_expando_owners.get(&42), Some(&77));
    host.owner = 77;
    host.origin_allowed = false;
    runtime.native_access_caller = Some(Some(88));
    runtime.begin_job();
    let error = runtime.execute("element[raw]", &mut host).unwrap_err();
    assert!(error.contains("same-origin"), "{error}");
    runtime.native_access_caller = None;
    host.origin_allowed = true;
    runtime.begin_job();
    assert_eq!(runtime.execute("element[raw]", &mut host).unwrap().to_number(), 17.0);
    assert_eq!((host.reads, host.writes, host.calls), (0, 0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn raw_key_native_revalidates_after_descriptor_getter_and_key_conversion_reentry() {
    let (mut runtime, mut host) = fixture();
    runtime.execute(r#"
        var raw='\ud800', descriptorReads=0, keyReads=0;
        element[raw]=17;
        var descriptor={get value(){descriptorReads++;element.revoke();return 99;}};
        var key={toString:function(){keyReads++;element.revoke();return raw;}};
    "#, &mut host).unwrap();
    runtime.begin_job();
    let error = runtime.execute("Object.defineProperty(element,raw,descriptor)", &mut host).unwrap_err();
    assert!(error.contains("no longer accessible"), "{error}");
    assert_eq!(host.calls, 1);
    assert_eq!(runtime.binding_value(runtime.global_environment, "descriptorReads").unwrap().to_number(), 1.0);
    assert!(runtime.transient_registration_is_empty());
    host.enabled = true;
    runtime.begin_job();
    let error = runtime.execute("element[key]=99", &mut host).unwrap_err();
    assert!(error.contains("no longer accessible"), "{error}");
    assert_eq!(host.calls, 2);
    assert_eq!(runtime.binding_value(runtime.global_environment, "keyReads").unwrap().to_number(), 1.0);
    assert!(runtime.transient_registration_is_empty());
    host.enabled = true;
    runtime.begin_job();
    assert_eq!(runtime.execute("element[raw]", &mut host).unwrap().to_number(), 17.0);
    assert_eq!((host.reads, host.writes), (0, 0));
}

#[test]
fn raw_key_native_opaque_handles_never_gain_an_expando_or_native_dispatch() {
    let (mut runtime, mut host) = fixture();
    runtime.execute("var raw='\\ud800';", &mut host).unwrap();
    for source in [
        "opaque[raw]",
        "opaque[raw]=3",
        "raw in opaque",
        "delete opaque[raw]",
        "Object.defineProperty(opaque,raw,{value:3})",
    ] {
        runtime.begin_job();
        let error = runtime.execute(source, &mut host).unwrap_err();
        assert!(error.contains("native"), "{source}: {error}");
        assert!(!runtime.host_objects.contains_key(&43));
        assert!(!runtime.native_expando_owners.contains_key(&43));
        assert!(runtime.transient_registration_is_empty());
    }
    runtime.begin_job();
    runtime.execute(r#"
        if(Object.hasOwn(opaque,raw)||Object.getOwnPropertyDescriptor(opaque,raw)!==undefined)
            throw 'opaque author property fabricated';
    "#, &mut host).unwrap();
    assert_eq!((host.reads, host.writes, host.calls), (0, 0, 0));
}

#[test]
fn raw_key_native_allocation_and_retained_payload_failures_leave_no_partial_property() {
    let (mut runtime, mut host) = fixture();
    let object_limit = runtime.limits.heap.objects;
    runtime.limits.heap.objects = runtime.objects.len();
    let error = runtime.assign_units_property(
        Value::Host(42), vec![0xd800], Value::Number(17.0), &mut host,
    ).unwrap_err();
    assert!(error.message.contains("limit"), "{error:?}");
    assert!(!runtime.host_objects.contains_key(&42));
    assert!(!runtime.native_expando_owners.contains_key(&42));
    runtime.limits.heap.objects = object_limit;
    runtime.begin_job();
    runtime.assign_units_property(Value::Host(42), vec![0xd800], Value::Number(17.0), &mut host).unwrap();
    let id = runtime.host_objects[&42];
    let before = (runtime.value_bytes, runtime.properties, runtime.objects[id].properties.len(), runtime.objects[id].next_order);
    let retained_limit = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = before.0 + 4; // two key bytes plus three payload bytes require five.
    runtime.begin_job();
    let error = runtime.assign_units_property(
        Value::Host(42), vec![0xdc00], Value::String("abc".into()), &mut host,
    ).unwrap_err();
    assert!(error.message.contains("Retained value limit"), "{error:?}");
    assert_eq!((runtime.value_bytes, runtime.properties, runtime.objects[id].properties.len(), runtime.objects[id].next_order), before);
    assert!(runtime.objects[id].properties.get_units(&[0xdc00]).is_none());
    runtime.limits.heap.retained_values = retained_limit;
    runtime.begin_job();
    runtime.assign_units_property(Value::Host(42), vec![0xdc00], Value::String("abc".into()), &mut host).unwrap();
    assert_eq!(runtime.value_bytes, before.0 + 5);
    assert_eq!(runtime.properties, before.1 + 1);
    runtime.remove_units_property(id, &[0xdc00]).unwrap();
    assert_eq!(runtime.value_bytes, before.0);
    assert_eq!(runtime.properties, before.1);
    assert_eq!((host.reads, host.writes, host.calls), (0, 0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn raw_key_native_supplied_well_formed_units_share_scalar_identity_and_byte_limits() {
    let (mut runtime, mut host) = fixture();
    runtime.limits.capacity.property_bytes = 256;
    let id = runtime.new_object(None).unwrap();
    let object = Value::Object(id);
    for (index, (units, scalar)) in [
        (vec![u16::from(b'a'); 129], "a".repeat(129)),
        (vec![u16::from(b'b'); 256], "b".repeat(256)),
        ([0xd83d, 0xde00].repeat(64), "😀".repeat(64)),
    ].into_iter().enumerate() {
        // Hosts can supply a well-formed StringUnits value. Admission follows
        // its canonical scalar bytes, not the size of its input unit vector.
        let key = runtime.property_key(Value::StringUnits(units.clone()), &mut host).unwrap();
        assert!(matches!(&key, PropertyKey::String(name) if name == &scalar));
        runtime.write_reference(key.reference(object.clone()), Value::Number(11.0), &mut host).unwrap();
        assert_eq!(runtime.read_property(&object, &scalar, &mut host).unwrap().to_number(), 11.0);
        let scalar_key = runtime.property_key(Value::String(scalar.clone()), &mut host).unwrap();
        runtime.write_reference(scalar_key.reference(object.clone()), Value::Number(13.0), &mut host).unwrap();
        let unit_key = runtime.property_key(Value::StringUnits(units.clone()), &mut host).unwrap();
        assert_eq!(runtime.read_reference(&unit_key.reference(object.clone()), &mut host).unwrap().to_number(), 13.0);
        assert_eq!(runtime.objects[id].properties.len(), index + 1, "scalar alias must update the same property");
        assert!(runtime.objects[id].properties.get_units(&units).is_none());
    }
    let before = (runtime.value_bytes, runtime.properties, runtime.objects[id].properties.len(), runtime.objects[id].next_order);
    for value in [
        Value::StringUnits(vec![u16::from(b'c'); 257]),
        Value::String("c".repeat(257)),
        Value::StringUnits([0xd83d, 0xde00].repeat(65)),
        Value::String("😀".repeat(65)),
    ] {
        let error = runtime.property_key(value, &mut host).err().expect("oversized canonical key");
        assert!(error.message.contains("Property-name limit"), "{error:?}");
        assert_eq!((runtime.value_bytes, runtime.properties, runtime.objects[id].properties.len(), runtime.objects[id].next_order), before);
    }
    let raw = runtime.property_key(Value::StringUnits(vec![0xd800; 128]), &mut host).unwrap();
    assert!(matches!(raw, PropertyKey::StringUnits(units) if units == vec![0xd800; 128]));
    let error = runtime.property_key(Value::StringUnits(vec![0xd800; 129]), &mut host)
        .err().expect("oversized malformed key");
    assert!(error.message.contains("Property-name limit"), "{error:?}");
    assert_eq!((host.reads, host.writes, host.calls), (0, 0, 0));
    assert!(runtime.transient_registration_is_empty());
}
