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
fn weak_set_api_distinct_canonical_surface_brands_and_tag_deletion() {
    succeeds(
        r#"
        var set = new WeakSet();
        if (WeakSet.name !== 'WeakSet' || WeakSet.length !== 0 ||
            Object.getPrototypeOf(set) !== WeakSet.prototype ||
            Object.getPrototypeOf(WeakSet.prototype) !== Object.prototype ||
            WeakSet.prototype.constructor !== WeakSet ||
            WeakSet.prototype === WeakMap.prototype || WeakSet.prototype === Set.prototype) {
            throw 'constructor and prototype identity';
        }
        var cp = Object.getOwnPropertyDescriptor(WeakSet, 'prototype');
        if (cp.writable || cp.enumerable || cp.configurable) throw 'constructor prototype descriptor';
        var names = ['add', 'delete', 'has'];
        for (var i = 0; i < names.length; i++) {
            var name = names[i], d = Object.getOwnPropertyDescriptor(WeakSet.prototype, name);
            if (typeof d.value !== 'function' || d.value.name !== name || d.value.length !== 1 ||
                !d.writable || d.enumerable || !d.configurable || Object.hasOwn(d.value, 'prototype')) {
                throw 'method descriptor';
            }
        }
        var absent = ['size', 'clear', 'get', 'set', 'keys', 'values', 'entries', 'forEach'];
        for (var i = 0; i < absent.length; i++) {
            if (absent[i] in set) throw 'unsupported observable surface';
        }
        if (Symbol.iterator in set || Object.keys(set).length || Object.keys(WeakSet.prototype).length) {
            throw 'enumeration';
        }
        var caught = 0;
        try { WeakSet(); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { new WeakSet.prototype.add(); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { WeakSet.prototype.has.call(WeakSet.prototype, {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { WeakSet.prototype.add.call(Object.create(WeakSet.prototype), {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { WeakSet.prototype.has.call(new WeakMap(), {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { WeakMap.prototype.has.call(set, {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { Set.prototype.has.call(set, {}); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        if (caught !== 7) throw 'distinct internal brands';
        var tag = Object.getOwnPropertyDescriptor(WeakSet.prototype, Symbol.toStringTag);
        if (tag.value !== 'WeakSet' || tag.writable || tag.enumerable || !tag.configurable ||
            Object.prototype.toString.call(set) !== '[object WeakSet]' ||
            Object.prototype.toString.call(WeakSet.prototype) !== '[object WeakSet]') throw 'tag';
        Object.defineProperty(set, Symbol.toStringTag, { value: 'Custom', configurable: true });
        if (Object.prototype.toString.call(set) !== '[object Custom]') throw 'own tag';
        delete set[Symbol.toStringTag];
        delete WeakSet.prototype[Symbol.toStringTag];
        if (Object.prototype.toString.call(set) !== '[object Object]' ||
            Object.prototype.toString.call(new WeakMap()) !== '[object WeakMap]') throw 'independent deleted tag';
        Object.defineProperty(WeakSet.prototype, Symbol.toStringTag, tag);
        if (Object.prototype.toString.call(set) !== '[object WeakSet]') throw 'restored tag';
        'ok';
    "#,
    );
}

#[test]
fn weak_set_api_noncoercing_membership_symbols_and_set_like_iterable_values() {
    succeeds(
        r#"
        var effects = 0, set = new WeakSet();
        var key = { get toString() { effects++; throw 'key conversion'; },
                    get valueOf() { effects++; throw 'key conversion'; } };
        key[Symbol.toPrimitive] = function() { effects++; throw 'key conversion'; };
        var other = {}, ignored = { get toString() { effects++; throw 'extra argument conversion'; } };
        function fn() {}
        var symbol = Symbol('same'), distinct = Symbol('same');
        if (set.add(key, ignored) !== set || set.add(key) !== set || !set.has(key) || set.has(other)) {
            throw 'object identity and add return';
        }
        set.add(fn).add(symbol).add(Symbol.iterator);
        if (!set.has(fn) || !set.has(symbol) || set.has(distinct) || !set.has(Symbol.iterator)) throw 'key families';
        var invalid = [undefined, null, false, 1, NaN, 'key', Symbol.for('registered')];
        for (var i = 0; i < invalid.length; i++) {
            var value = invalid[i], caught = false;
            if (set.has(value) || set.delete(value)) throw 'invalid membership';
            try { set.add(value); } catch (e) { if (e.name !== 'TypeError') throw e; caught = true; }
            if (!caught) throw 'invalid add accepted';
        }
        if (!set.delete(key) || set.delete(key) || set.has(key) || !set.has(fn) || effects !== 0) throw 'delete and conversion';
        var pair = [other];
        var fromValues = new WeakSet([pair, fn, symbol, pair]);
        if (!fromValues.has(pair) || fromValues.has(other) || !fromValues.has(fn) || !fromValues.has(symbol)) {
            throw 'iterable values must not be destructured as pairs';
        }
        if (Object.getOwnPropertyNames(fromValues).length || Object.keys(Object.assign({}, fromValues)).length) {
            throw 'stored keys must not become properties';
        }
        var count = 0;
        for (var name in fromValues) count++;
        if (count !== 0) throw 'for-in keys';
        var map = new WeakMap([[other, pair]]), ordinary = new Set([1, 2]);
        if (map.get(other) !== pair || ordinary.size !== 2 || !ordinary.has(1)) throw 'neighbor collections';
        'ok';
    "#,
    );
}

#[test]
fn weak_set_api_constructor_observes_add_once_without_pair_property_reads() {
    succeeds(
        r#"
        var saved = Object.getOwnPropertyDescriptor(WeakSet.prototype, 'add');
        var order = '', key = { get 0() { throw 'pair index0'; }, get 1() { throw 'pair index1'; } };
        Object.defineProperty(WeakSet.prototype, 'add', {
            configurable: true,
            get() {
                order += 'a';
                return function(value) {
                    order += 's';
                    var nested = new WeakSet(null);
                    saved.value.call(nested, value);
                    if (!nested.has(value)) throw 'adder reentry';
                    return saved.value.call(this, value);
                };
            }
        });
        new WeakSet(); new WeakSet(undefined); new WeakSet(null);
        if (order !== '') throw 'nullish source read adder';
        var source = {};
        source[Symbol.iterator] = function() {
            order += 'i';
            var done = false;
            return { next() {
                order += 'n';
                if (done) return { done: true };
                done = true;
                return { done: false, get value() { order += 'v'; return key; } };
            } };
        };
        var set = new WeakSet(source);
        if (order !== 'ainvsn' || !set.has(key)) throw 'constructor order';
        Object.defineProperty(WeakSet.prototype, 'add', { value: 1, configurable: true });
        order = '';
        var caught = false;
        try { new WeakSet(source); } catch (e) { if (e.name !== 'TypeError') throw e; caught = true; }
        if (!caught || order !== '') throw 'adder callable check precedes iterator';
        Object.defineProperty(WeakSet.prototype, 'add', saved);
        if (new WeakSet([]).has(key)) throw 'empty iterable';
        'ok';
    "#,
    );
}

#[test]
fn weak_set_api_constructor_closes_invalid_values_and_adder_throw() {
    succeeds(
        r#"
        var closes = 0, iterations = 0, marker = {}, secondary = {}, key = {};
        function items(value) {
            var iterator = {
                next() { return { done: false, value: value }; },
                return() { closes++; throw secondary; }
            };
            iterator[Symbol.iterator] = function() { iterations++; return this; };
            return iterator;
        }
        var caught = 0;
        try { new WeakSet(items(1)); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        try { new WeakSet(items(Symbol.for('registered'))); } catch (e) { if (e.name !== 'TypeError') throw e; caught++; }
        var saved = Object.getOwnPropertyDescriptor(WeakSet.prototype, 'add');
        WeakSet.prototype.add = function() { throw marker; };
        try { new WeakSet(items(key)); } catch (e) { if (e !== marker) throw e; caught++; }
        Object.defineProperty(WeakSet.prototype, 'add', { configurable: true, get() { throw marker; } });
        try { new WeakSet(items(key)); } catch (e) { if (e !== marker) throw e; caught++; }
        Object.defineProperty(WeakSet.prototype, 'add', saved);
        if (caught !== 4 || closes !== 3 || iterations !== 3) throw 'close and acquisition order';
        'ok';
    "#,
    );
}

#[test]
fn weak_set_api_stores_undefined_and_shares_exact_entry_budgets() {
    let mut runtime = Runtime::for_test();
    let set = runtime
        .construct_collection(Kind::WeakSet, &[], &mut NoHost)
        .unwrap();
    let Value::Object(id) = set.clone() else {
        unreachable!()
    };
    let baseline = runtime.value_bytes;
    runtime.copy_bytes = 0;
    runtime
        .weak_collection_method(
            Kind::WeakSet,
            "add",
            &[Value::Host(0), Value::String("ignored".into())],
            set.clone(),
            &mut NoHost,
        )
        .unwrap();
    runtime
        .weak_collection_method(
            Kind::WeakSet,
            "add",
            &[Value::Host(0)],
            set.clone(),
            &mut NoHost,
        )
        .unwrap();
    assert_eq!(
        (runtime.collection_entries, runtime.value_bytes),
        (1, baseline)
    );
    assert!(runtime.objects[id]
        .collection
        .as_ref()
        .unwrap()
        .ordered
        .values()
        .all(|entry| matches!(entry.value, Value::Undefined)));
    runtime
        .weak_collection_method(
            Kind::WeakSet,
            "delete",
            &[Value::Host(0)],
            set.clone(),
            &mut NoHost,
        )
        .unwrap();
    assert_eq!(
        (runtime.collection_entries, runtime.value_bytes),
        (0, baseline)
    );
    for kind in [Kind::WeakSet, Kind::Map, Kind::WeakMap, Kind::Set] {
        runtime.begin_job();
        let target = if kind == Kind::WeakSet {
            set.clone()
        } else {
            runtime
                .construct_collection(kind, &[], &mut NoHost)
                .unwrap()
        };
        for index in 0..MAX_ENTRIES {
            if index % 256 == 0 {
                runtime.begin_job();
            }
            let method = if kind.is_set() { "add" } else { "set" };
            runtime
                .collection_builtin(
                    Op::Method(kind, method),
                    &[Value::Host(index as u32), Value::Null],
                    target.clone(),
                    &mut NoHost,
                )
                .unwrap();
        }
    }
    assert_eq!(runtime.collection_entries, MAX_DOCUMENT_ENTRIES);
    runtime.begin_job();
    assert!(runtime
        .weak_collection_method(
            Kind::WeakSet,
            "add",
            &[Value::Host(MAX_ENTRIES as u32)],
            set.clone(),
            &mut NoHost
        )
        .is_err());
    runtime
        .weak_collection_method(
            Kind::WeakSet,
            "add",
            &[Value::Host(0)],
            set.clone(),
            &mut NoHost,
        )
        .unwrap();
    let another = runtime
        .construct_collection(Kind::WeakSet, &[], &mut NoHost)
        .unwrap();
    assert!(runtime
        .weak_collection_method(
            Kind::WeakSet,
            "add",
            &[Value::Host(0)],
            another.clone(),
            &mut NoHost
        )
        .is_err());
    runtime
        .weak_collection_method(Kind::WeakSet, "delete", &[Value::Host(1)], set, &mut NoHost)
        .unwrap();
    runtime
        .weak_collection_method(
            Kind::WeakSet,
            "add",
            &[Value::Host(0)],
            another,
            &mut NoHost,
        )
        .unwrap();
    assert_eq!(runtime.collection_entries, MAX_DOCUMENT_ENTRIES);
    assert_eq!(runtime.value_bytes, baseline);
}

#[test]
fn weak_set_api_native_key_owner_and_brand_checks_precede_any_key_effects() {
    struct OwnerHost {
        allow: Cell<bool>,
        checks: Cell<usize>,
    }
    impl Host for OwnerHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("native key read")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("native key write")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("native key call")
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
    let set = runtime
        .construct_collection(Kind::WeakSet, &[], &mut host)
        .unwrap();
    let stores = (
        runtime.host_objects.len(),
        runtime.native_expando_owners.len(),
    );
    runtime
        .weak_collection_method(
            Kind::WeakSet,
            "add",
            &[Value::Host(50)],
            set.clone(),
            &mut host,
        )
        .unwrap();
    assert!(matches!(
        runtime
            .weak_collection_method(
                Kind::WeakSet,
                "has",
                &[Value::Host(50)],
                set.clone(),
                &mut host
            )
            .unwrap(),
        Value::Bool(true)
    ));
    let prior = (runtime.collection_entries, runtime.value_bytes);
    host.allow.set(false);
    for method in ["add", "has", "delete"] {
        assert!(runtime
            .weak_collection_method(
                Kind::WeakSet,
                method,
                &[Value::Host(50)],
                set.clone(),
                &mut host
            )
            .is_err());
        assert_eq!((runtime.collection_entries, runtime.value_bytes), prior);
    }
    let checks = host.checks.get();
    let map = runtime
        .construct_collection(Kind::WeakMap, &[], &mut host)
        .unwrap();
    assert!(runtime
        .weak_collection_method(Kind::WeakSet, "has", &[Value::Host(50)], map, &mut host)
        .is_err());
    assert_eq!(host.checks.get(), checks);
    assert_eq!(
        (
            runtime.host_objects.len(),
            runtime.native_expando_owners.len()
        ),
        stores
    );
    host.allow.set(true);
    assert!(matches!(
        runtime
            .weak_collection_method(Kind::WeakSet, "has", &[Value::Host(50)], set, &mut host)
            .unwrap(),
        Value::Bool(true)
    ));
}
