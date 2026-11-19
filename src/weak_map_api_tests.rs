use super::*;
use std::cell::Cell;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected native property read")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected native property write")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected native method call")
    }
}

fn succeeds(script: &str) {
    let mut runtime = Runtime::for_test();
    let result = runtime.execute(script, &mut NoHost).unwrap();
    assert!(matches!(result, Value::String(ref text) if text == "ok"));
}

#[test]
fn weak_map_api_canonical_surface_brands_descriptors_and_tag_deletion() {
    succeeds(
        r#"
        var map = new WeakMap();
        if (WeakMap.name !== 'WeakMap' || WeakMap.length !== 0) throw 'constructor metadata';
        if (Object.getPrototypeOf(map) !== WeakMap.prototype ||
            Object.getPrototypeOf(WeakMap.prototype) !== Object.prototype ||
            WeakMap.prototype.constructor !== WeakMap) throw 'prototype identity';
        var cp = Object.getOwnPropertyDescriptor(WeakMap, 'prototype');
        if (cp.writable || cp.enumerable || cp.configurable) throw 'constructor prototype descriptor';
        var names = ['delete', 'get', 'has', 'set'];
        for (var i = 0; i < names.length; i++) {
            var name = names[i];
            var d = Object.getOwnPropertyDescriptor(WeakMap.prototype, name);
            if (typeof d.value !== 'function' || !d.writable || d.enumerable ||
                !d.configurable || d.value.name !== name ||
                d.value.length !== (name === 'set' ? 2 : 1) ||
                Object.hasOwn(d.value, 'prototype')) throw 'method descriptor';
        }
        if (Object.keys(map).length || Object.keys(WeakMap.prototype).length) throw 'enumerability';
        var absent = ['size', 'clear', 'keys', 'values', 'entries', 'forEach'];
        for (var i = 0; i < absent.length; i++) {
            if (absent[i] in map || Object.hasOwn(WeakMap.prototype, absent[i])) throw 'key enumeration surface';
        }
        if (Symbol.iterator in map) throw 'weak map iterator';
        var caught = 0;
        try { WeakMap(); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { new WeakMap.prototype.set(); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { WeakMap.prototype.get.call(WeakMap.prototype, {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { WeakMap.prototype.has.call(Object.create(WeakMap.prototype), {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { Map.prototype.get.call(map, {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { WeakMap.prototype.get.call(new Map(), {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        if (caught !== 6) throw 'required construction and brands';
        var tag = Object.getOwnPropertyDescriptor(WeakMap.prototype, Symbol.toStringTag);
        if (tag.value !== 'WeakMap' || tag.writable || tag.enumerable || !tag.configurable ||
            Object.prototype.toString.call(map) !== '[object WeakMap]' ||
            Object.prototype.toString.call(WeakMap.prototype) !== '[object WeakMap]') throw 'tag descriptor';
        Object.defineProperty(map, Symbol.toStringTag, { value: 'Custom', configurable: true });
        if (Object.prototype.toString.call(map) !== '[object Custom]') throw 'own tag';
        delete map[Symbol.toStringTag];
        delete WeakMap.prototype[Symbol.toStringTag];
        if (Object.prototype.toString.call(map) !== '[object Object]') throw 'deleted tag fallback';
        Object.defineProperty(WeakMap.prototype, Symbol.toStringTag, tag);
        if (Object.prototype.toString.call(map) !== '[object WeakMap]') throw 'tag restoration';
        'ok';
    "#,
    );
}

#[test]
fn weak_map_api_keys_values_and_noncoercing_invalid_key_results() {
    succeeds(
        r#"
        var effects = 0, map = new WeakMap();
        var key = { get toString() { effects++; throw 'key coercion'; },
                    get valueOf() { effects++; throw 'key coercion'; } };
        var other = {}, value = { get toString() { effects++; throw 'value coercion'; } };
        function fn() {}
        if (map.set(key, value) !== map || !map.has(key) || map.get(key) !== value ||
            map.has(other) || map.get(other) !== undefined) throw 'object identity';
        map.set(fn, key);
        if (map.get(fn) !== key || map.get(key) !== value) throw 'function identity';
        map.set(key, undefined);
        if (!map.has(key) || map.get(key) !== undefined) throw 'undefined value presence';
        map.set(key, value);
        var symbol = Symbol('key'), sameDescription = Symbol('key');
        map.set(symbol, fn).set(Symbol.iterator, other);
        if (map.get(symbol) !== fn || map.has(sameDescription) ||
            map.get(Symbol.iterator) !== other) throw 'unregistered symbol identity';
        var invalid = [undefined, null, false, 1, NaN, 'key', Symbol.for('registered')];
        for (var i = 0; i < invalid.length; i++) {
            var k = invalid[i], rejected = false;
            if (map.get(k) !== undefined || map.has(k) || map.delete(k)) throw 'invalid query result';
            try { map.set(k, value); } catch (e) {
                if (e.name !== 'TypeError') throw e;
                rejected = true;
            }
            if (!rejected) throw 'invalid set accepted';
        }
        if (!map.delete(key) || map.delete(key) || map.get(key) !== undefined ||
            map.has(key) || map.get(fn) !== key || effects !== 0) throw 'delete or coercion';
        'ok';
    "#,
    );
}

#[test]
fn weak_map_api_constructor_observes_adder_then_iterable_then_pair_reads() {
    succeeds(
        r#"
        var saved = Object.getOwnPropertyDescriptor(WeakMap.prototype, 'set');
        var order = '', key = {}, value = {};
        Object.defineProperty(WeakMap.prototype, 'set', {
            configurable: true,
            get() {
                order += 'a';
                    return function(k, v) {
                        order += 's';
                        var nested = new WeakMap(null);
                        saved.value.call(nested, k, v);
                        if (nested.get(k) !== v) throw 'adder reentry identity';
                        return saved.value.call(this, k, v);
                    };
            }
        });
        new WeakMap(); new WeakMap(undefined); new WeakMap(null);
        if (order !== '') throw 'nullish source touched adder';
        var source = {};
        source[Symbol.iterator] = function() {
            order += 'i';
            var done = false;
            return { next() {
                order += 'n';
                if (done) return { done: true };
                done = true;
                return { done: false, value: {
                    get 0() { order += 'k'; return key; },
                    get 1() { order += 'v'; return value; }
                } };
            } };
        };
        var map = new WeakMap(source);
        if (order !== 'ainkvsn' || map.get(key) !== value) throw 'constructor order';
        Object.defineProperty(WeakMap.prototype, 'set', { value: 1, configurable: true });
        order = '';
        var caught = false;
        try { new WeakMap(source); } catch (e) { if (e.name !== 'TypeError') throw e; caught = true; }
        if (!caught || order !== '') throw 'adder validation order';
        Object.defineProperty(WeakMap.prototype, 'set', saved);
        var both = new WeakMap([[key, 1], [key, 2], [value, 3]]);
        if (both.get(key) !== 2 || both.get(value) !== 3) throw 'iterable replacement';
        'ok';
    "#,
    );
}

#[test]
fn weak_map_api_constructor_closes_abrupt_pair_and_adder_completions() {
    succeeds(
        r#"
        var closes = 0, marker = {}, secondary = {}, key = {};
        function items(value) {
            var it = {
                next() { return { done: false, value: value }; },
                return() { closes++; throw secondary; }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        }
        var caught = 0;
        try { new WeakMap(items(3)); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { new WeakMap(items({ get 0() { throw marker; } })); } catch (e) { if (e !== marker) throw e; caught++; }
        try { new WeakMap(items({ 0: key, get 1() { throw marker; } })); } catch (e) { if (e !== marker) throw e; caught++; }
        try { new WeakMap(items([1, 2])); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        var saved = WeakMap.prototype.set;
        WeakMap.prototype.set = function() { throw marker; };
        try { new WeakMap(items([key, 2])); } catch (e) { if (e !== marker) throw e; caught++; }
        WeakMap.prototype.set = saved;
        if (caught !== 5 || closes !== 5) throw 'abrupt iterator close';
        'ok';
    "#,
    );
}

#[test]
fn weak_map_api_native_key_owner_is_revalidated_without_expando_or_coercion() {
    struct OwnerHost {
        allow: Cell<bool>,
        checks: Cell<usize>,
    }
    impl Host for OwnerHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("key property read")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("key property write")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("key method call")
        }
        fn native_access_owner(&self, _: u32) -> Option<u32> {
            Some(2)
        }
        fn validate_window_access(&self, caller: Option<u32>, owner: u32) -> Result<(), String> {
            assert_eq!((caller, owner), (Some(1), 2));
            self.checks.set(self.checks.get() + 1);
            if self.allow.get() {
                Ok(())
            } else {
                Err("native owner denied".into())
            }
        }
    }
    let mut runtime = Runtime::for_test();
    let mut host = OwnerHost {
        allow: Cell::new(true),
        checks: Cell::new(0),
    };
    runtime.native_access_caller = Some(Some(1));
    let map = runtime
        .construct_collection(Kind::WeakMap, &[], None, &mut host)
        .unwrap();
    let native_key = Value::Host(50);
    let before = (
        runtime.host_objects.len(),
        runtime.native_expando_owners.len(),
    );
    runtime
        .weak_map_method(
            "set",
            &[native_key.clone(), Value::String("kept".into())],
            map.clone(),
            &mut host,
        )
        .unwrap();
    assert!(
        matches!(runtime.weak_map_method("get", &[native_key.clone()], map.clone(), &mut host).unwrap(), Value::String(ref text) if text == "kept")
    );
    let entries = runtime.collection_entries;
    let bytes = runtime.value_bytes;
    host.allow.set(false);
    for method in ["set", "get", "has", "delete"] {
        assert!(runtime
            .weak_map_method(
                method,
                &[native_key.clone(), Value::Null],
                map.clone(),
                &mut host
            )
            .is_err());
        assert_eq!(
            (runtime.collection_entries, runtime.value_bytes),
            (entries, bytes)
        );
    }
    let checks = host.checks.get();
    assert!(runtime
        .weak_map_method("get", &[native_key], Value::Null, &mut host)
        .is_err());
    assert_eq!(host.checks.get(), checks);
    assert_eq!(
        (
            runtime.host_objects.len(),
            runtime.native_expando_owners.len()
        ),
        before
    );
    host.allow.set(true);
    assert!(
        matches!(runtime.weak_map_method("get", &[Value::Host(50)], map, &mut host).unwrap(), Value::String(ref text) if text == "kept")
    );
}

#[test]
fn weak_map_api_retained_copy_and_entry_caps_preserve_prior_values() {
    let mut runtime = Runtime::for_test();
    let map = runtime
        .construct_collection(Kind::WeakMap, &[], None, &mut NoHost)
        .unwrap();
    let key = Value::Object(runtime.new_object(None).unwrap());
    let baseline = runtime.value_bytes;
    runtime
        .weak_map_method(
            "set",
            &[key.clone(), Value::String("value".into())],
            map.clone(),
            &mut NoHost,
        )
        .unwrap();
    assert_eq!(
        (runtime.collection_entries, runtime.value_bytes),
        (1, baseline + 5)
    );
    runtime
        .weak_map_method(
            "set",
            &[key.clone(), Value::String("v".into())],
            map.clone(),
            &mut NoHost,
        )
        .unwrap();
    assert_eq!(
        (runtime.collection_entries, runtime.value_bytes),
        (1, baseline + 1)
    );
    runtime.copy_bytes = 2;
    assert!(runtime
        .weak_map_method(
            "set",
            &[key.clone(), Value::String("long".into())],
            map.clone(),
            &mut NoHost
        )
        .is_err());
    runtime.begin_job();
    let retained_limit = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = baseline + 2;
    assert!(runtime
        .weak_map_method(
            "set",
            &[key.clone(), Value::String("long".into())],
            map.clone(),
            &mut NoHost
        )
        .is_err());
    runtime.limits.heap.retained_values = retained_limit;
    assert!(
        matches!(runtime.weak_map_method("get", &[key.clone()], map.clone(), &mut NoHost).unwrap(), Value::String(ref text) if text == "v")
    );
    assert_eq!(
        (runtime.collection_entries, runtime.value_bytes),
        (1, baseline + 1)
    );
    runtime
        .weak_map_method("delete", &[key], map.clone(), &mut NoHost)
        .unwrap();
    assert_eq!(
        (runtime.collection_entries, runtime.value_bytes),
        (0, baseline)
    );
    for index in 0..MAX_ENTRIES {
        if index % 256 == 0 {
            runtime.begin_job();
        }
        runtime
            .weak_map_method(
                "set",
                &[Value::Host(index as u32), Value::Null],
                map.clone(),
                &mut NoHost,
            )
            .unwrap();
    }
    runtime.begin_job();
    assert!(runtime
        .weak_map_method(
            "set",
            &[Value::Host(MAX_ENTRIES as u32), Value::Null],
            map.clone(),
            &mut NoHost
        )
        .is_err());
    assert_eq!(runtime.collection_entries, MAX_ENTRIES);
    runtime
        .weak_map_method(
            "set",
            &[Value::Host(0), Value::Bool(true)],
            map.clone(),
            &mut NoHost,
        )
        .unwrap();
    runtime
        .weak_map_method("delete", &[Value::Host(1)], map.clone(), &mut NoHost)
        .unwrap();
    runtime
        .weak_map_method(
            "set",
            &[Value::Host(MAX_ENTRIES as u32), Value::Null],
            map.clone(),
            &mut NoHost,
        )
        .unwrap();
    assert_eq!(runtime.collection_entries, MAX_ENTRIES);
}

#[test]
fn weak_map_api_shares_document_entry_limit_with_existing_strong_maps() {
    let mut runtime = Runtime::for_test();
    let mut maps = Vec::new();
    for _ in 0..MAX_DOCUMENT_ENTRIES / MAX_ENTRIES {
        let map = runtime
            .construct_collection(Kind::WeakMap, &[], None, &mut NoHost)
            .unwrap();
        for index in 0..MAX_ENTRIES {
            if index % 256 == 0 {
                runtime.begin_job();
            }
            runtime
                .weak_map_method(
                    "set",
                    &[Value::Host(index as u32), Value::Null],
                    map.clone(),
                    &mut NoHost,
                )
                .unwrap();
        }
        maps.push(map);
    }
    let strong = runtime.new_object(None).unwrap();
    runtime.objects[strong].collection = Some(Data::new(Kind::Map));
    assert!(runtime
        .collection_put(strong, Kind::Map, &Value::Number(1.0), &Value::Null)
        .is_err());
    let another = runtime
        .construct_collection(Kind::WeakMap, &[], None, &mut NoHost)
        .unwrap();
    assert!(runtime
        .weak_map_method(
            "set",
            &[Value::Host(1), Value::Null],
            another.clone(),
            &mut NoHost
        )
        .is_err());
    runtime
        .weak_map_method("delete", &[Value::Host(0)], maps.remove(0), &mut NoHost)
        .unwrap();
    runtime
        .weak_map_method("set", &[Value::Host(1), Value::Null], another, &mut NoHost)
        .unwrap();
    assert_eq!(runtime.collection_entries, MAX_DOCUMENT_ENTRIES);
}
