// Original independent String slot accounting assertions. Development only:
// check_string_retention_review_041.py injects this module into a temporary
// source copy; production code is never edited by the review.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("No host reads.".into()) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("No host writes.".into()) }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("No host calls.".into()) }
}

#[test]
fn independent_string_payload_uses_utf8_bytes_and_virtual_keys_allocate_no_slots() {
    let mut runtime = Runtime::new();
    let before = runtime.value_bytes;
    let properties = runtime.properties;
    let objects = runtime.objects.len();
    let Value::Object(id) = runtime.box_string("é😀".into()).unwrap() else { panic!("box required") };
    assert_eq!(runtime.value_bytes, before + "é😀".len() + "length".len());
    assert_eq!(runtime.string_object_length(id), Some(3));
    assert_eq!(runtime.properties, properties + 1);
    assert_eq!(runtime.objects.len(), objects + 1);
    assert_eq!(runtime.objects[id].properties.len(), 1);
    let retained = runtime.value_bytes;
    assert_eq!(runtime.own_string_keys(id, false).unwrap(), ["0", "1", "2", "length"]);
    assert_eq!(runtime.own_property(id, "0").unwrap().to_text(), "é");
    assert_eq!(runtime.value_bytes, retained);
    assert_eq!(runtime.properties, properties + 1);
    assert!(runtime.owns_property(id, "1").unwrap());
    assert!(runtime.owns_property(id, "2").unwrap());
    assert!(!runtime.owns_property(id, "3").unwrap());
}

#[test]
fn independent_string_retained_preflight_is_exact_for_multibyte_payloads() {
    let mut runtime = Runtime::new();
    let needed = "é😀".len() + "length".len();
    runtime.value_bytes = MAX_RETAINED_VALUES - needed + 1;
    let before = runtime.value_bytes;
    let properties = runtime.properties;
    let objects = runtime.objects.len();
    let error = runtime.box_string("é😀".into()).unwrap_err();
    assert!(error.message.contains("Retained value limit"));
    assert_eq!(runtime.value_bytes, before);
    assert_eq!(runtime.properties, properties);
    assert_eq!(runtime.objects.len(), objects);
    runtime.value_bytes = MAX_RETAINED_VALUES - needed;
    runtime.box_string("é😀".into()).unwrap();
    assert_eq!(runtime.value_bytes, MAX_RETAINED_VALUES);
    assert_eq!(runtime.properties, properties + 1);
    assert_eq!(runtime.objects.len(), objects + 1);
}

#[test]
fn independent_string_property_replacement_and_deletion_preserve_slot_accounting() {
    let mut runtime = Runtime::new();
    let Value::Object(id) = runtime.box_string("A😀Z".into()).unwrap() else { panic!("box required") };
    let retained = runtime.value_bytes;
    let properties = runtime.properties;
    runtime.set_property(id, "extra".into(), Value::String("held".into())).unwrap();
    assert_eq!(runtime.value_bytes, retained + "extra".len() + "held".len());
    runtime.set_property(id, "extra".into(), Value::String("é".into())).unwrap();
    assert_eq!(runtime.value_bytes, retained + "extra".len() + "é".len());
    runtime.remove_property(id, "extra").unwrap();
    assert_eq!(runtime.value_bytes, retained);
    assert_eq!(runtime.properties, properties);
    runtime.set_property(id, "0".into(), Value::String("wrong".into())).unwrap();
    assert_eq!(runtime.value_bytes, retained);
    assert_eq!(runtime.properties, properties);
    assert_eq!(runtime.own_property(id, "0").unwrap().to_text(), "A");
    assert_eq!(runtime.delete_reference(Reference::Property(Value::Object(id), "1".into()), &mut NoHost).unwrap().to_text(), "false");
    assert_eq!(runtime.value_bytes, retained);
    let bytes = runtime.copy_bytes;
    assert_eq!(runtime.wrapped_primitive(&Value::Object(id)).unwrap().unwrap().to_text(), "A😀Z");
    assert_eq!(runtime.copy_bytes, bytes - "A😀Z".len());
    assert_eq!(runtime.value_bytes, retained);
}

#[test]
fn independent_string_large_box_and_failed_unbox_stay_bounded() {
    let mut runtime = Runtime::new();
    let retained = runtime.value_bytes;
    let properties = runtime.properties;
    let Value::Object(id) = runtime.box_string("x".repeat(MAX_STRING_BYTES)).unwrap() else { panic!("box required") };
    assert_eq!(runtime.value_bytes, retained + MAX_STRING_BYTES + "length".len());
    assert_eq!(runtime.properties, properties + 1);
    assert_eq!(runtime.own_property(id, &(MAX_STRING_BYTES - 1).to_string()).unwrap().to_text(), "x");
    assert!(runtime.own_string_keys(id, true).unwrap_err().exception.is_none());
    let retained = runtime.value_bytes;
    let objects = runtime.objects.len();
    runtime.copy_bytes = MAX_STRING_BYTES - 1;
    let error = runtime.wrapped_primitive(&Value::Object(id)).unwrap_err();
    assert!(error.message.contains("work limit"));
    assert!(error.exception.is_none());
    assert_eq!(runtime.value_bytes, retained);
    assert_eq!(runtime.objects.len(), objects);
}
