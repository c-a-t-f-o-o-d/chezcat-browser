//! Original UTF-16 ordinary property-key regression tests.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("Ordinary property keys must not read a native object")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("Ordinary property keys must not write a native object")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("Ordinary property keys must not call a native object")
    }
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn property_key_utf16_optional_calls_and_primitive_exotic_receivers_keep_exact_keys() {
    succeeds(
        r#"
        var key = '\ud800', object = {}, calls = 0;
        object[key] = function() { calls++; return this; };
        if (object?.[key]() !== object || object[key]?.() !== object || calls !== 2)
            throw 'optional raw method receiver';
        if (null?.[key]() !== undefined || object['\ud801']?.() !== undefined || calls !== 2)
            throw 'optional raw short circuit';
        String.prototype[key] = 7;
        Number.prototype[key] = 11;
        Boolean.prototype[key] = 13;
        if ('x'[key] !== 7 || (2)[key] !== 11 || true[key] !== 13)
            throw 'primitive raw prototype read';
        'x'[key] = 17;
        if ('x'[key] !== 7) throw 'primitive raw assignment persisted';
        var boxed = Object('x'), typed = new Uint8Array(2);
        boxed[key] = 19;
        typed[key] = 23;
        typed[0] = 5;
        if (boxed[key] !== 19 || boxed[0] !== 'x' || boxed.length !== 1 ||
            typed[key] !== 23 || typed[0] !== 5 || typed.length !== 2)
            throw 'exotic raw properties changed native indices';
        var names = Object.keys(typed);
        if (names.length !== 3 || names[0] !== '0' || names[1] !== '1' || names[2] !== key)
            throw 'typed raw enumeration';
        if (!delete boxed[key] || boxed[key] !== 7 || !delete typed[key] || typed[key] !== undefined)
            throw 'exotic raw deletion';
        delete String.prototype[key];
        delete Number.prototype[key];
        delete Boolean.prototype[key];
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_distinct_code_units_do_not_alias_scalar_or_escaped_spellings() {
    succeeds(
        r#"
        var keys = ['\ud800', '\ud801', '\udc00', '\udfff', 'A\ud800B',
                    '\ufffd', 'A\ufffdB', '\ud800\udc00', '\udc00\ud800',
                    '\\ud800', '\0\ud800', '\0\\ud800'];
        var object = Object.create(null);
        for (var i = 0; i < keys.length; i++) {
            if (keys[i] in object) throw 'distinct input already present';
            object[keys[i]] = i + 1;
        }
        if (Object.keys(object).length !== keys.length) throw 'distinct key count';
        for (var i = 0; i < keys.length; i++) {
            if (object[keys[i]] !== i + 1 || !(keys[i] in object) ||
                !Object.prototype.hasOwnProperty.call(object, keys[i])) throw 'exact stored key';
        }
        var high = '\ud83d', low = '\ude00', paired = high + low;
        object[paired] = 71;
        if (object['😀'] !== 71 || object['\ud83d\ude00'] !== 71 ||
            high in object || low in object) throw 'paired and unpaired identities';
        object[high] = 73;
        object[low] = 79;
        if (object['😀'] !== 71 || object[high] !== 73 || object[low] !== 79 ||
            object['\ufffd'] !== 6) throw 'surrogate insertion changed another key';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_membership_shadow_delete_and_null_prototype_keep_exact_keys() {
    succeeds(
        r#"
        var key = '\ud800', replacement = '\ufffd', parent = {};
        parent[key] = 5;
        parent[replacement] = 11;
        var child = Object.create(parent);
        if (!(key in child) || Object.hasOwn(child, key) || child[key] !== 5)
            throw 'inherited exact key';
        child[key] = 7;
        if (!Object.hasOwn(child, key) || child[key] !== 7 || parent[key] !== 5 ||
            child[replacement] !== 11) throw 'own shadow';
        if (!delete child[key] || child[key] !== 5 || !(key in child) ||
            Object.hasOwn(child, key)) throw 'delete exposes exact prototype key';
        if (!delete parent[key] || key in child || child[key] !== undefined ||
            child[replacement] !== 11) throw 'prototype deletion';
        var dictionary = Object.create(null);
        dictionary[key] = undefined;
        if (!(key in dictionary) || !Object.hasOwn(dictionary, key) ||
            replacement in dictionary || !delete dictionary[key] || key in dictionary)
            throw 'undefined own value and absent key differ';
        if (!delete dictionary[key]) throw 'deleting absent exact key';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_data_and_accessor_descriptors_preserve_flags_and_receivers() {
    succeeds(
        r#"
        var data = '\ud800', access = 'get\udfff', object = {}, writes = 0, reads = 0;
        Object.defineProperty(object, data, {
            value: 3, writable: true, enumerable: true, configurable: true
        });
        var descriptor = Object.getOwnPropertyDescriptor(object, data);
        if (descriptor.value !== 3 || !descriptor.writable || !descriptor.enumerable ||
            !descriptor.configurable || descriptor.get !== undefined ||
            Object.getOwnPropertyDescriptor(object, '\ufffd') !== undefined)
            throw 'exact data descriptor';
        var receiver = Object.create(object), saved = 0;
        function get() { reads++; if (this !== receiver) throw 'getter receiver'; return saved; }
        function set(value) { writes++; if (this !== receiver) throw 'setter receiver'; saved = value; }
        Object.defineProperty(object, access, {
            get: get, set: set, enumerable: false, configurable: true
        });
        receiver[access] = 17;
        if (receiver[access] !== 17 || reads !== 1 || writes !== 1 ||
            Object.hasOwn(receiver, access)) throw 'inherited exact accessor';
        descriptor = Object.getOwnPropertyDescriptor(object, access);
        if (descriptor.get !== get || descriptor.set !== set || descriptor.enumerable ||
            !descriptor.configurable || descriptor.value !== undefined ||
            descriptor.writable !== undefined) throw 'exact accessor descriptor';
        Object.defineProperty(object, data, {value: 19, writable: false});
        if (object[data] !== 19 || Object.getOwnPropertyDescriptor(object, data).writable ||
            !object.propertyIsEnumerable(data)) throw 'data descriptor update';
        if (!delete object[access] || Object.getOwnPropertyDescriptor(object, access) !== undefined)
            throw 'accessor delete';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_enumeration_keeps_index_order_string_insertion_order_and_symbols() {
    succeeds(
        r#"
        function same(actual, expected) {
            if (actual.length !== expected.length) throw 'enumeration length';
            for (var i = 0; i < expected.length; i++)
                if (actual[i] !== expected[i]) throw 'enumeration key identity or order';
        }
        var high = '\ud800', low = '\udfff', hidden = 'hidden\ud800';
        var first = Symbol('slot'), second = Symbol('slot'), object = {};
        object.z = 1;
        object[low] = 2;
        object['10'] = 10;
        object[high] = 3;
        object['2'] = 20;
        Object.defineProperty(object, hidden, {value: 4, configurable: true});
        object[first] = 31;
        object.a = 5;
        object[second] = 37;
        same(Object.keys(object), ['2', '10', 'z', low, high, 'a']);
        same(Object.getOwnPropertyNames(object), ['2', '10', 'z', low, high, hidden, 'a']);
        same(Object.getOwnPropertySymbols(object), [first, second]);
        same(Object.values(object), [20, 10, 1, 2, 3, 5]);
        var entries = Object.entries(object);
        same(entries[3], [low, 2]);
        same(entries[4], [high, 3]);
        if (Object.keys(object)[4].charCodeAt(0) !== 0xd800 ||
            Object.getOwnPropertyNames(object)[5].charCodeAt(6) !== 0xd800)
            throw 'enumeration replaced code units';
        var iterated = [];
        for (var key in object) iterated.push(key);
        same(iterated, ['2', '10', 'z', low, high, 'a']);
        delete object[low];
        object[low] = 41;
        same(Object.keys(object), ['2', '10', 'z', high, 'a', low]);
        same(Object.getOwnPropertyNames(object), ['2', '10', 'z', high, hidden, 'a', low]);
        same(Object.getOwnPropertySymbols(object), [first, second]);
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_reflect_get_set_keep_lookup_target_and_receiver_distinct() {
    succeeds(
        r#"
        var key = '\ud800', accessor = 'access\udfff', parent = {}, receiver = {}, calls = 0;
        Object.defineProperty(parent, key, {value: 5, writable: true});
        var child = Object.create(parent);
        if (!Reflect.set(child, key, 11, receiver) || receiver[key] !== 11 ||
            child[key] !== 5 || Object.hasOwn(child, key)) throw 'Reflect exact data receiver';
        Object.defineProperty(parent, accessor, {
            get: function() { calls++; return this; },
            set: function(value) { calls++; this.seen = value; }, configurable: true
        });
        if (Reflect.get(child, accessor, receiver) !== receiver ||
            !Reflect.set(child, accessor, 17, receiver) || receiver.seen !== 17 || calls !== 2 ||
            Object.hasOwn(receiver, accessor)) throw 'Reflect exact accessor receiver';
        if (Reflect.get(child, '\ufffd') !== undefined ||
            Reflect.get(child, 'access\ufffd') !== undefined) throw 'Reflect scalar collision';
        var symbol = Symbol('slot');
        parent[symbol] = 23;
        if (Reflect.get(child, symbol) !== 23 || !Reflect.set(child, symbol, 29, receiver) ||
            receiver[symbol] !== 29 || parent[symbol] !== 23 || receiver[key] !== 11)
            throw 'Reflect symbol and string domains';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_computed_and_quoted_literal_keys_are_real_ordinary_properties() {
    succeeds(
        r#"
        var high = '\ud800', low = '\udfff', log = '', key = {};
        key[Symbol.toPrimitive] = function(hint) {
            if (hint !== 'string' || this !== key) throw 'literal key conversion';
            log += 'K'; return high;
        };
        var object = {before: (log += 'A', 1), [key]: (log += 'V', 2), '\udfff': 3,
                      after: (log += 'B', 4), ['\ufffd']: 5};
        if (log !== 'AKVB' || object[high] !== 2 || object[low] !== 3 ||
            object['\ufffd'] !== 5 || Object.getPrototypeOf(object) !== Object.prototype)
            throw 'literal order or key identity';
        var names = Object.getOwnPropertyNames(object);
        if (names.length !== 5 || names[0] !== 'before' || names[1] !== high ||
            names[2] !== low || names[3] !== 'after' || names[4] !== '\ufffd')
            throw 'literal property order';
        var descriptor = Object.getOwnPropertyDescriptor(object, high);
        if (!descriptor.writable || !descriptor.enumerable || !descriptor.configurable)
            throw 'literal data descriptor';
        var repeated = {[high]: 7, ['\ud800']: 11};
        if (repeated[high] !== 11 || Object.keys(repeated).length !== 1)
            throw 'same units create one property';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_assign_from_entries_and_enumerable_getters_copy_exact_names() {
    succeeds(
        r#"
        var high = '\ud800', low = '\udfff', symbol = Symbol('slot'), log = '', source = {};
        source[high] = 3;
        Object.defineProperty(source, low, {
            get: function() { log += 'g'; return 5; }, enumerable: true, configurable: true
        });
        Object.defineProperty(source, 'hidden\ud800', {value: 7});
        source['\ufffd'] = 11;
        source[symbol] = 13;
        var target = Object.assign({}, source);
        if (log !== 'g' || target[high] !== 3 || target[low] !== 5 || target['\ufffd'] !== 11 ||
            target[symbol] !== 13 || 'hidden\ud800' in target) throw 'assign copies real keys';
        var descriptor = Object.getOwnPropertyDescriptor(target, low);
        if (descriptor.value !== 5 || !descriptor.writable || !descriptor.enumerable ||
            !descriptor.configurable || descriptor.get !== undefined) throw 'assign creates data';
        var rebuilt = Object.fromEntries([[high, 17], [low, 19], [symbol, 23],
                                          ['\ufffd', 29], [high, 31]]);
        if (rebuilt[high] !== 31 || rebuilt[low] !== 19 || rebuilt['\ufffd'] !== 29 ||
            rebuilt[symbol] !== 23) throw 'fromEntries exact and repeated keys';
        var keys = Object.keys(rebuilt);
        if (keys.length !== 3 || keys[0] !== high || keys[1] !== low || keys[2] !== '\ufffd' ||
            Object.getOwnPropertySymbols(rebuilt)[0] !== symbol) throw 'fromEntries order';
        var roundtrip = Object.fromEntries(Object.entries(rebuilt));
        if (roundtrip[high] !== 31 || roundtrip[low] !== 19 || roundtrip['\ufffd'] !== 29 ||
            Object.getOwnPropertySymbols(roundtrip).length !== 0) throw 'entries roundtrip';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_conversion_callbacks_run_once_in_existing_operation_order() {
    succeeds(
        r#"
        var raw = '\ud800', key = {}, object = {}, log = '';
        key[Symbol.toPrimitive] = function(hint) {
            if (this !== key || hint !== 'string') throw 'property-key hint or receiver';
            log += 'K'; return raw;
        };
        object[key] = (log += 'R', 3);
        if (log !== 'RK') throw 'assignment conversion order';
        log = '';
        if (object[key] !== 3 || log !== 'K') throw 'single read conversion';
        log = '';
        if (!(key in object) || log !== 'K') throw 'single membership conversion';
        log = '';
        Object.defineProperty(object, key, {
            get value() { log += 'D'; return 7; },
            writable: true, enumerable: true, configurable: true
        });
        if (log !== 'KD' || object[raw] !== 7) throw 'descriptor conversion follows key';
        log = '';
        if (Object.getOwnPropertyDescriptor(object, key).value !== 7 || log !== 'K')
            throw 'single descriptor-key conversion';
        log = '';
        if (!delete object[key] || log !== 'K' || raw in object) throw 'single delete conversion';
        var fallback = {toString: function() {
            log += 'T'; this.valueOf = function() { log += 'V'; return raw; }; return {};
        }};
        log = '';
        object[fallback] = 11;
        if (log !== 'TV' || object[raw] !== 11) throw 'current fallback methods and raw result';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_key_exceptions_preserve_properties_and_allow_conversion_recovery() {
    succeeds(
        r#"
        var raw = '\ud800', object = {}, marker = {}, key = {}, after = false;
        object[raw] = 3;
        key[Symbol.toPrimitive] = function() { throw marker; };
        var caught = false;
        try { Object.defineProperty(object, key, {value: 7}); after = true; }
        catch(error) { caught = error === marker; }
        if (!caught || after || object[raw] !== 3 || Object.keys(object).length !== 1)
            throw 'key exception preserves object';
        key[Symbol.toPrimitive] = function() { return {}; };
        caught = false;
        try { object[key]; } catch(error) { caught = error.name === 'TypeError'; }
        if (!caught || object[raw] !== 3 || object['\ufffd'] !== undefined)
            throw 'nonprimitive conversion result';
        key[Symbol.toPrimitive] = function() { return raw; };
        if (object[key] !== 3) throw 'conversion recovery';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_nonwritable_nonconfigurable_and_nonextensible_rules_still_apply() {
    succeeds(
        r#"
        var raw = '\ud800', object = {}, strictErrors = 0;
        Object.defineProperty(object, raw, {value: 3, enumerable: true});
        object[raw] = 5;
        if (object[raw] !== 3 || delete object[raw] || Reflect.set(object, raw, 7))
            throw 'read-only key behavior';
        function strictWrite() { 'use strict'; object[raw] = 11; }
        function strictDelete() { 'use strict'; delete object[raw]; }
        try { strictWrite(); } catch(error) { if (error.name === 'TypeError') strictErrors++; }
        try { strictDelete(); } catch(error) { if (error.name === 'TypeError') strictErrors++; }
        if (strictErrors !== 2 || object[raw] !== 3) throw 'strict key failures';
        var closed = Object.preventExtensions({});
        if (Reflect.set(closed, raw, 13) || Object.hasOwn(closed, raw) ||
            Object.getOwnPropertyNames(closed).length !== 0) throw 'closed object raw key';
        var writable = {};
        writable[raw] = 17;
        Object.freeze(writable);
        var descriptor = Object.getOwnPropertyDescriptor(writable, raw);
        if (!Object.isFrozen(writable) || descriptor.writable || descriptor.configurable ||
            !descriptor.enumerable || Reflect.set(writable, raw, 19)) throw 'freeze sees raw key';
        'ok';
        "#,
    );
}

#[test]
fn property_key_utf16_raw_and_scalar_names_share_object_property_admission() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute("var o = {}, k = '\\ud800'; o.a = 1;", &mut NoHost)
        .unwrap();
    let property_limit = runtime.limits.heap.object_properties;
    runtime.limits.heap.object_properties = 1;
    let error = runtime.execute("o[k] = 2;", &mut NoHost).unwrap_err();
    assert!(error.contains("Object property limit"), "{error}");
    assert!(runtime.transient_registration_is_empty());
    runtime.limits.heap.object_properties = property_limit;
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute(
                r#"
                if (o.a !== 1 || k in o || Object.keys(o).join(',') !== 'a')
                    throw 'refusal published a raw key';
                o[k] = 2;
                if (o[k] !== 2 || o['\ufffd'] !== undefined || Object.keys(o).length !== 2)
                    throw 'fresh job recovery';
                'ok';
                "#,
                &mut NoHost,
            )
            .unwrap()
            .to_text(),
        "ok"
    );
    assert!(runtime.transient_registration_is_empty());
}
