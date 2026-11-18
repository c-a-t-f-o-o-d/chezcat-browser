use super::tests::{succeeds, NoHost, WindowReceiverHost};
use super::*;

#[test]
fn reflect_set_uses_the_resolved_builtin_and_reports_its_standard_metadata() {
    succeeds(
        r#"
        var set = Reflect.set, target = {}, d = Object.getOwnPropertyDescriptor(Reflect, 'set');
        if (d.value !== set || !d.configurable || !d.writable || d.enumerable ||
            set.name !== 'set' || set.length !== 3 || set.prototype !== undefined ||
            set.toString().indexOf('[native code]') < 0) throw 'set metadata';
        var caught = false;
        try { new set(target, 'x', 1); } catch (e) { caught = e.name === 'TypeError'; }
        if (!caught || 'x' in target) throw 'not constructible';
        var changes = 0;
        function replace() { Reflect.set = function () { changes++; return 19; }; return target; }
        if (Reflect.set(replace(), 'x', 7) !== true || target.x !== 7 || changes !== 0)
            throw 'callee identity across argument evaluation';
        if (Reflect.set(null) !== 19 || changes !== 1) throw 'replacement';
        delete Reflect.set;
        if ('set' in Reflect || set.call(null, target, 'y', 13) !== true || target.y !== 13)
            throw 'captured builtin';
        'ok';
    "#,
    );
}

#[test]
fn reflect_set_data_assignment_keeps_target_receiver_and_descriptor_flags_distinct() {
    succeeds(
        r#"
        'use strict';
        function verify(key) {
            var parent = {}, target = Object.create(parent), recv = {}, inheritedCalls = 0;
            Object.defineProperty(parent, key, {value:1, writable:true});
            Object.defineProperty(recv, key, {value:2, writable:true, enumerable:false, configurable:false});
            if (Reflect.set(target, key, 3, recv) !== true || recv[key] !== 3 ||
                target[key] !== 1 || parent[key] !== 1 || Object.hasOwn(target, key)) throw 'distinct write';
            var d = Object.getOwnPropertyDescriptor(recv, key);
            if (!d.writable || d.enumerable || d.configurable) throw 'updated descriptor flags';
            var unrelated = {};
            Object.defineProperty(unrelated, key, {set:function() { inheritedCalls++; }});
            var fresh = Object.create(unrelated);
            if (Reflect.set(target, key, 4, fresh) !== true || fresh[key] !== 4 || inheritedCalls !== 0)
                throw 'create own data on receiver';
            d = Object.getOwnPropertyDescriptor(fresh, key);
            if (!d.writable || !d.enumerable || !d.configurable) throw 'new descriptor';
            if (Reflect.set(target, key, 5) !== true || target[key] !== 5 || parent[key] !== 1)
                throw 'omitted receiver';
            var closed = Object.preventExtensions({});
            if (Reflect.set(target, key, 6, closed) !== false || Object.hasOwn(closed, key) ||
                Reflect.set(target, key, 6, undefined) !== false || Reflect.set(target, key, 6, 3) !== false)
                throw 'invalid data receivers';
            var ownAccessor = {};
            Object.defineProperty(ownAccessor, key, {set:function() { inheritedCalls++; }});
            if (Reflect.set(target, key, 6, ownAccessor) !== false || inheritedCalls !== 0)
                throw 'receiver-own accessor is not called for a target data property';
            var frozen = {};
            Object.defineProperty(frozen, key, {value:11});
            if (Reflect.set(target, key, 7, frozen) !== false || frozen[key] !== 11 ||
                Reflect.set(frozen, key, 11, recv) !== false || recv[key] !== 3)
                throw 'receiver and target read-only data';
        }
        verify('value'); verify(Symbol('value'));
        var omitted = {};
        if (!Reflect.set(omitted) || !Object.hasOwn(omitted, 'undefined') || omitted.undefined !== undefined)
            throw 'omitted key and value';
        'ok';
    "#,
    );
}

#[test]
fn reflect_set_inherited_setters_receive_exact_values_and_propagate_completions() {
    succeeds(
        r#"
        var seenThis, seenValue, runs = 0, marker = {};
        function verify(key) {
            var parent = {}, target = Object.create(parent), frozenRecv = Object.freeze({});
            function setter(v) { 'use strict'; seenThis = this; seenValue = v; runs++; return false; }
            Object.defineProperty(parent, key, {set:setter, configurable:true});
            var before = runs;
            if (Reflect.set(target, key, marker, frozenRecv) !== true || seenThis !== frozenRecv ||
                seenValue !== marker || runs !== before+1 || Object.hasOwn(frozenRecv, key)) throw 'setter completion';
            if (Reflect.set(target, key, 2, undefined) !== true || seenThis !== undefined || seenValue !== 2)
                throw 'explicit undefined accessor receiver';
            if (Reflect.set(target, key, 3, null) !== true || seenThis !== null ||
                Reflect.set(target, key, 4, 19) !== true || seenThis !== 19) throw 'primitive setter receivers';
            if (Reflect.set(target, key, 5) !== true || seenThis !== target) throw 'default accessor receiver';
            Object.defineProperty(parent, key, {set:function() { throw marker; }});
            var caught = false;
            try { Reflect.set(target, key, 6, frozenRecv); } catch(e) { caught = e === marker; }
            if (!caught) throw 'thrown identity';
            Object.defineProperty(parent, key, {get:function() { throw 'must not read'; },set:undefined});
            if (Reflect.set(target, key, 7) !== false) throw 'missing setter';
        }
        verify('value'); verify(Symbol('value'));
        'ok';
    "#,
    );
}

#[test]
fn reflect_set_validates_target_before_key_and_observes_conversion_once() {
    succeeds(
        r#"
        var order = '', target = {}, receiver = {}, key = {}, marker = {}, symbol = Symbol('slot');
        var value = { toString:function () { throw 'coerced value'; } };
        key[Symbol.toPrimitive] = function(hint) {
            if (hint !== 'string' || this !== key) throw 'conversion receiver';
            order += 'k';
            Object.defineProperty(target, symbol, {set:function(v) {
                if (this !== receiver || v !== value) throw 'wrong setter inputs';
                order += 's';
            }});
            return symbol;
        };
        var invalid = [undefined, null, false, 1, 'text', Symbol('other')], failures = 0;
        for (var i=0; i<invalid.length; i++) {
            try { Reflect.set(invalid[i], key, value, receiver); } catch(e) { if (e.name === 'TypeError') failures++; }
        }
        if (failures !== invalid.length || order !== '') throw 'target validation order';
        if (Reflect.set(target, key, value, receiver) !== true || order !== 'ks') throw 'key conversion order';
        key[Symbol.toPrimitive] = function() { order += 'e'; throw marker; };
        var caught = false;
        try { Reflect.set(target, key, value, receiver); } catch(e) { caught = e === marker; }
        if (!caught || order !== 'kse' || Object.hasOwn(receiver, symbol)) throw 'key exception identity';
        'ok';
    "#,
    );
}

#[test]
fn reflect_set_strict_assignment_and_sloppy_assignment_keep_their_own_failure_policy() {
    succeeds(
        r#"
        var frozen = Object.freeze({x:1}), seen = 0;
        function sloppy() { frozen.x = 2; return Reflect.set(frozen,'x',2); }
        function strict() { 'use strict'; try { frozen.x = 3; }
            catch (e) { if (e.name === 'TypeError') seen++; } return Reflect.set(frozen,'x',3); }
        if (sloppy() !== false || strict() !== false || seen !== 1 || frozen.x !== 1)
            throw 'assignment and internal Set results';
        'ok';
    "#,
    );
}

#[test]
fn reflect_set_retains_assigned_closure_after_collection_and_calls_in_its_home_realm() {
    let mut runtime = Runtime::for_test();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let exports = runtime
        .with_intrinsic_realm(Some(realm), |runtime| {
            ["Reflect", "Function", "TypeError"]
                .into_iter()
                .map(|name| {
                    runtime
                        .binding_value(runtime.global_environment, name)
                        .map(|value| (name, value))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap();
    for (name, value) in exports {
        runtime
            .define_global(&format!("Child{name}"), value)
            .unwrap();
    }
    runtime.execute(r#"
        var saved = ChildReflect.set, target = {}, receiver = {}, scratch = {};
        if (saved === Reflect.set || Object.getPrototypeOf(saved) !== ChildFunction.prototype) throw 'child identity';
        Object.defineProperty(target, 'callback', {set:ChildFunction('v','this.kept=v;this.realm=Function;')});
        (function() { var secret = 43; saved(target, 'callback', function() { return secret; }, receiver); })();
        if (receiver.realm !== ChildFunction) throw 'setter realm';
        var caught = false;
        try { saved(false, 'x', 1); }
        catch(e) { caught = Object.getPrototypeOf(e) === ChildTypeError.prototype; }
        if (!caught) throw 'callee error realm';
        delete ChildReflect.set;
        function pressure() { for(var n=0; n<40; n++) scratch.fn=function() { return n; }; }
    "#, &mut NoHost).unwrap();
    let retired = runtime.function_arena.retired;
    for _ in 0..35 {
        runtime.begin_job();
        runtime.execute("pressure(); 0;", &mut NoHost).unwrap();
        if runtime.function_arena.retired > retired {
            break;
        }
    }
    assert!(runtime.function_arena.retired > retired);
    runtime.collect_environments();
    runtime.begin_job();
    assert_eq!(runtime.execute(
        "if (ChildReflect.set !== undefined || !saved({}, 'x', 1)) throw 'detached identity'; receiver.kept();",
        &mut NoHost,
    ).unwrap().to_number(), 43.0);
    assert_eq!(runtime.active_realm, None);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn reflect_set_foreign_window_receiver_does_not_invoke_string_or_symbol_setters() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("window", Value::Host(77)).unwrap();
    runtime.define_global("foreign", Value::Host(88)).unwrap();
    runtime.bind_browser_global(77).unwrap();
    let mut host = WindowReceiverHost { same_origin: false };
    runtime
        .execute(
            r#"
        var target={}, symbol=Symbol('value'), calls=0, holder={};
        function setter(v) {calls++;}
        Object.defineProperty(target, 'value', {set:setter});
        Object.defineProperty(target, symbol, {set:setter});
        if (typeof Reflect.set !== 'function') throw 'set callable';
    "#,
            &mut host,
        )
        .unwrap();
    for same_origin in [false, true] {
        host.same_origin = same_origin;
        for key in ["'value'", "symbol"] {
            let before = runtime.window_allocation_state_for_test();
            assert!(runtime
                .execute(
                    &format!("Reflect.set(target,{key},holder,foreign);"),
                    &mut host
                )
                .is_err());
            assert_eq!(runtime.window_allocation_state_for_test(), before);
            assert_eq!(
                runtime
                    .binding_value(runtime.global_environment, "calls")
                    .unwrap()
                    .to_number(),
                0.0
            );
            assert!(runtime.transient_registration_is_empty());
            runtime.begin_job();
        }
    }
}

#[test]
fn reflect_set_heap_limit_fails_without_publishing_a_partial_receiver_descriptor() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute(
            "var target={}, recv={keep:9}, key=Symbol('key'); if (typeof Reflect.set !== 'function') throw 'set callable';",
            &mut NoHost,
        )
        .unwrap();
    let capacity = runtime.limits.heap.object_properties;
    runtime.limits.heap.object_properties = 1;
    for source in [
        "Reflect.set(target,'extra',4,recv);",
        "Reflect.set(target,key,5,recv);",
    ] {
        let error = runtime.execute(source, &mut NoHost).unwrap_err();
        assert!(error.contains("property limit"), "{error}");
        runtime.begin_job();
    }
    runtime.limits.heap.object_properties = capacity;
    assert_eq!(
        runtime
            .execute(
                r#"
        if (recv.keep !== 9 || Object.hasOwn(recv, 'extra') || Object.hasOwn(recv, key) ||
            !Reflect.set(target, key, 11, recv)) throw 'transaction or recovery';
        recv[key];
    "#,
                &mut NoHost
            )
            .unwrap()
            .to_number(),
        11.0
    );
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn reflect_set_unsupported_native_and_exotic_data_paths_preserve_original_values() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("native", Value::Host(42)).unwrap();
    runtime
        .execute(
            r#"
        var ordinary={x:1}, array=[8], symbol=Symbol('x');
        var setterCalls=0;
        Object.defineProperty(array, 'writer', {set:function() {setterCalls++;}});
    "#,
            &mut NoHost,
        )
        .unwrap();
    for source in [
        "Reflect.set(native, 'x', 99);",
        "Reflect.set(ordinary, 'x', 99, native);",
        "Reflect.set(array, 'writer', 99);",
        "Reflect.set(ordinary, symbol, 99, array);",
    ] {
        assert!(runtime.execute(source, &mut NoHost).is_err(), "{source}");
        assert!(runtime.transient_registration_is_empty());
        runtime.begin_job();
    }
    assert_eq!(
        runtime
            .execute(
                r#"
        if (setterCalls !== 0 || ordinary.x !== 1 || array.length !== 1 || array[0] !== 8 ||
            Object.hasOwn(array, symbol)) throw 'unsupported writes changed state';
        'ok';
    "#,
                &mut NoHost
            )
            .unwrap()
            .to_text(),
        "ok"
    );
}

#[test]
fn reflect_set_copy_exhaustion_precedes_setter_effects_for_string_and_symbol_keys() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute(
            r#"
        var target={}, key=Symbol('field'), count=0;
        function write(v) {'use strict'; count++;}
        Object.defineProperty(target, 'field', {set:write});
        Object.defineProperty(target, key, {set:write});
    "#,
            &mut NoHost,
        )
        .unwrap();
    let target = runtime
        .binding_value(runtime.global_environment, "target")
        .unwrap();
    let symbol = runtime
        .binding_value(runtime.global_environment, "key")
        .unwrap();
    let receiver = Value::String("r".repeat(8192));
    for key in [Value::String("field".into()), symbol] {
        runtime.begin_job();
        runtime.copy_bytes = 128;
        let error = runtime
            .reflect_set(
                &[target.clone(), key, Value::Undefined, receiver.clone()],
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.message.contains("copy work limit"), "{error:?}");
        assert!(error.exception.is_none());
        assert_eq!(
            runtime
                .binding_value(runtime.global_environment, "count")
                .unwrap()
                .to_number(),
            0.0
        );
        assert!(runtime.transient_registration_is_empty());
    }
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("Reflect.set(target,key,5,'short'); count;", &mut NoHost)
            .unwrap()
            .to_number(),
        1.0
    );
}
