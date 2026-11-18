use super::*;

struct NoHost;
impl Host for NoHost {
    fn has(&mut self, handle: u32, _: &str) -> Result<bool, String> {
        assert_eq!(handle, 77, "unexpected native presence check");
        // Like the real Window adapter, this synthetic Window has no native
        // fallback property after the VM removes an ordinary global binding.
        Ok(false)
    }
    fn allow_eval(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected native get")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected native set")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected native call")
    }
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(
        runtime.execute(source, &mut NoHost).unwrap().to_text(),
        "ok"
    );
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn reflect_get_namespace_descriptors_native_identity_and_author_mutation() {
    succeeds(
        r#"
        if (typeof Reflect !== 'object' || Object.getPrototypeOf(Reflect) !== Object.prototype)
            throw 'ordinary namespace';
        var get = Reflect.get;
        if (get.name !== 'get' || get.length !== 2 || get.prototype !== undefined ||
            get !== Reflect.get || get.toString().indexOf('[native code]') < 0)
            throw 'native identity';
        var method = Object.getOwnPropertyDescriptor(Reflect, 'get');
        if (method.value !== get || !method.writable || !method.configurable || method.enumerable ||
            method.get !== undefined || method.set !== undefined) throw 'method descriptor';
        var name = Object.getOwnPropertyDescriptor(get, 'name');
        var length = Object.getOwnPropertyDescriptor(get, 'length');
        if (name.writable || name.enumerable || !name.configurable ||
            length.writable || length.enumerable || !length.configurable) throw 'metadata descriptors';
        var tag = Object.getOwnPropertyDescriptor(Reflect, Symbol.toStringTag);
        if (tag.value !== 'Reflect' || tag.writable || tag.enumerable || !tag.configurable ||
            Object.prototype.toString.call(Reflect) !== '[object Reflect]') throw 'namespace tag';
        if (Reflect.set !== undefined || Reflect.construct !== undefined || Reflect.ownKeys !== undefined)
            throw 'unimplemented surface';
        var failures = 0;
        try { new get({}, 'x'); } catch (e) { if (e.name === 'TypeError') failures++; }
        try { new Reflect(); } catch (e) { if (e.name === 'TypeError') failures++; }
        try { Reflect(); } catch (e) { if (e.name === 'TypeError') failures++; }
        if (failures !== 3) throw 'call and construct restrictions';
        Reflect.get = 19;
        if (Reflect.get !== 19 || get({x: 7}, 'x') !== 7) throw 'replace and capture';
        delete Reflect.get;
        if ('get' in Reflect || Reflect.get !== undefined ||
            Object.getOwnPropertyDescriptor(Reflect, 'get') !== undefined) throw 'deleted method resurrected';
        Object.defineProperty(Reflect, 'get', method);
        delete Reflect[Symbol.toStringTag];
        if (Object.prototype.toString.call(Reflect) !== '[object Object]') throw 'deleted tag';
        Object.defineProperty(Reflect, Symbol.toStringTag, tag);
        if (Reflect.get !== get) throw 'canonical restoration';
        'ok';
    "#,
    );
}

#[test]
fn reflect_get_distinct_lookup_target_and_exact_accessor_receiver() {
    succeeds(
        r#"
        var parent = { inherited: 23 }, target = Object.create(parent), seen = [], reads = 0;
        Object.defineProperty(parent, 'value', {
            get: function () { 'use strict'; reads++; seen.push(this); return this; }, configurable: true
        });
        var receiver = { inherited: 99 };
        if (Reflect.get(target, 'inherited', receiver) !== 23 || reads) throw 'data lookup uses target';
        if (Reflect.get(target, 'value') !== target || Reflect.get(target, 'value', receiver) !== receiver ||
            Reflect.get(target, 'value', undefined) !== undefined ||
            Reflect.get(target, 'value', null) !== null || Reflect.get(target, 'value', 17) !== 17 ||
            Reflect.get(target, 'value', 'original') !== 'original') throw 'exact receiver';
        if (reads !== 6 || seen[0] !== target || seen[1] !== receiver || seen[2] !== undefined)
            throw 'accessor once';
        Object.defineProperty(target, 'setterOnly', { set: function () { throw 'unexpected setter'; } });
        if (Reflect.get(target, 'setterOnly', receiver) !== undefined ||
            Reflect.get(target, 'missing', null) !== undefined) throw 'missing getter';
        var key = Symbol('key'), other = Symbol('key');
        parent[key] = 41;
        receiver[key] = 90;
        if (Reflect.get(target, key, receiver) !== 41 || Reflect.get(target, other) !== undefined)
            throw 'symbol lookup identity';
        Object.defineProperty(parent, key, {
            get: function () { 'use strict'; return this; }, configurable: true
        });
        if (Reflect.get(target, key, receiver) !== receiver || Reflect.get(target, key, undefined) !== undefined)
            throw 'symbol receiver';
        var fn = function () {};
        Object.setPrototypeOf(fn, parent);
        if (Reflect.get(fn, 'value', receiver) !== receiver || Reflect.get(fn, key, null) !== null)
            throw 'callable target ancestry';
        'ok';
    "#,
    );
}

#[test]
fn reflect_get_target_validation_and_single_observable_property_key_conversion() {
    succeeds(
        r#"
        var effects = '', target = { value: 1 }, receiver = {}, marker = {};
        var key = {};
        key[Symbol.toPrimitive] = function (hint) {
            if (this !== key || hint !== 'string') throw 'key conversion receiver or hint';
            effects += 'k';
            Object.defineProperty(target, 'value', {
                get: function () { effects += 'g'; return this; }, configurable: true
            });
            return 'value';
        };
        var invalid = [undefined, null, true, 1, 'text', Symbol('invalid')], errors = 0;
        for (var i = 0; i < invalid.length; i++) {
            try { Reflect.get(invalid[i], key); } catch (e) { if (e.name === 'TypeError') errors++; }
        }
        if (errors !== invalid.length || effects !== '') throw 'validate before conversion';
        if (Reflect.get(target, key, receiver) !== receiver || effects !== 'kg') throw 'convert before lookup once';
        var symbol = Symbol('result');
        target[symbol] = 13;
        key[Symbol.toPrimitive] = function (hint) { effects += 's'; return symbol; };
        if (Reflect.get(target, key) !== 13 || effects !== 'kgs') throw 'symbol conversion preserved';
        key[Symbol.toPrimitive] = function () { throw marker; };
        try { Reflect.get(target, key); throw 'missing throw'; } catch (e) { if (e !== marker) throw e; }
        key[Symbol.toPrimitive] = function () { return {}; };
        var rejected = false;
        try { Reflect.get(target, key); } catch (e) { rejected = e.name === 'TypeError'; }
        if (!rejected || effects !== 'kgs') throw 'failed conversion';
        if (Reflect.get({undefined: 5}) !== 5 || Reflect.get({'7': 8}, 7) !== 8)
            throw 'missing and numeric key conversion';
        var fallback = { toString: function () { effects += 't'; return {}; },
            valueOf: function () { effects += 'v'; return 'plain'; } };
        if (Reflect.get({plain: 11}, fallback) !== 11 || effects !== 'kgstv') throw 'ordinary conversion order';
        'ok';
    "#,
    );
}

#[test]
fn reflect_get_existing_native_getters_exotics_and_brands_keep_receiver_semantics() {
    succeeds(
        r#"
        var first = new Map([[{}, 1]]), second = new Map([[{}, 1], [{}, 2]]);
        if (Reflect.get(Map.prototype, 'size', first) !== 1 ||
            Reflect.get(Map.prototype, 'size', second) !== 2) throw 'native accessor receiver';
        var caught = false;
        try { Reflect.get(Map.prototype, 'size', {}); } catch (e) { caught = e.name === 'TypeError'; }
        if (!caught) throw 'native accessor brand';
        if (Reflect.get([9, 8], 'length', null) !== 2 || Reflect.get([9, 8], '0', {}) !== 9 ||
            Reflect.get(new String('hi'), '1', {}) !== 'i') throw 'indexed data properties';
        var bytes = new Uint8Array([3, 4]);
        if (Reflect.get(bytes, '0', {}) !== 3 || Reflect.get(bytes, '99', {}) !== undefined)
            throw 'integer-indexed lookup';
        function mapped(value) { arguments[0] = 27; return Reflect.get(arguments, '0', {}); }
        if (mapped(1) !== 27) throw 'mapped arguments';
        'ok';
    "#,
    );
}

#[test]
fn reflect_get_namespace_global_descriptor_replacement_deletion_and_roots() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("window", Value::Host(77)).unwrap();
    runtime.bind_browser_global(77).unwrap();
    assert_eq!(runtime.execute(r#"
        var descriptor = Object.getOwnPropertyDescriptor(window, 'Reflect'), kept = Reflect, saved = Reflect.get;
        if (descriptor.value !== kept || descriptor.enumerable || !descriptor.writable || !descriptor.configurable)
            throw 'global descriptor';
        window.Reflect = 11;
        if (Reflect !== 11 || saved({value: 3}, 'value') !== 3) throw 'global replacement';
        delete window.Reflect;
        if ('Reflect' in window || typeof Reflect !== 'undefined' ||
            Object.getOwnPropertyDescriptor(window, 'Reflect') !== undefined) throw 'global deletion';
        Object.defineProperty(window, 'Reflect', descriptor);
        (function () {
            var secret = 43, object = {};
            Object.defineProperty(object, 'value', {get: function () { return secret; }});
            kept.reader = function () { return saved(object, 'value'); };
        })();
        var scratch = {};
        function pressure() {
            for (var n = 0; n < 40; n++) scratch.reader = function () { return n; };
        }
        if (Reflect !== kept || kept.reader() !== 43) throw 'retained namespace and accessor';
        'ok';
    "#, &mut NoHost).unwrap().to_text(), "ok");
    // Displaced readers are collectible. Use normal separate jobs, leaving
    // the saved namespace, its reader and the captured getter reachable.
    let retired = runtime.function_arena.retired;
    for _ in 0..35 {
        runtime.begin_job();
        runtime.execute("pressure(); 0;", &mut NoHost).unwrap();
        if runtime.function_arena.retired > retired { break; }
    }
    assert!(runtime.function_arena.retired > retired);
    runtime.collect_environments();
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("kept.reader();", &mut NoHost)
            .unwrap()
            .to_number(),
        43.0
    );
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn reflect_get_per_realm_identity_callback_owner_and_error_prototypes() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute("Object.prototype.parentOnly = 31;", &mut NoHost)
        .unwrap();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let exports = runtime
        .with_intrinsic_realm(Some(realm), |runtime| {
            ["Reflect", "Object", "Function", "TypeError", "Array"]
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
    assert_eq!(runtime.execute(r#"
        var get = Reflect.get, childGet = ChildReflect.get;
        if (ChildReflect === Reflect || childGet === get ||
            Object.getPrototypeOf(ChildReflect) !== ChildObject.prototype ||
            Object.getPrototypeOf(childGet) !== ChildFunction.prototype ||
            ChildReflect.parentOnly !== undefined || Reflect.parentOnly !== 31) throw 'realm namespace roots';
        var child = ChildFunction('return { get value() { if (Array !== this.expected) throw "getter realm"; return this; } };')();
        var receiver = { expected: ChildArray };
        if (get(child, 'value', receiver) !== receiver || childGet(child, 'value', receiver) !== receiver)
            throw 'accessor defining realm';
        var errors = 0;
        try { childGet(null, 'value'); } catch (e) { if (Object.getPrototypeOf(e) === ChildTypeError.prototype) errors++; }
        try { get(null, 'value'); } catch (e) { if (Object.getPrototypeOf(e) === TypeError.prototype) errors++; }
        if (errors !== 2) throw 'builtin error realm';
        ChildReflect.get = 17;
        delete ChildReflect.get;
        if (ChildReflect.get !== undefined || Reflect.get !== get || childGet({value: 29}, 'value') !== 29)
            throw 'independent mutation and captured method';
        if (Object.getPrototypeOf([]) !== Array.prototype) throw 'caller realm restoration';
        'ok';
    "#, &mut NoHost).unwrap().to_text(), "ok");
    assert_eq!(
        (runtime.active_realm, runtime.global_environment),
        (None, 0)
    );
    assert!(runtime.transient_registration_is_empty());
}

struct Native {
    allowed: bool,
    gets: usize,
}
impl Host for Native {
    fn method(&self, handle: u32, key: &str) -> Option<&'static str> {
        (handle == 42 && key == "close").then_some("close")
    }
    fn native_access_owner(&self, handle: u32) -> Option<u32> {
        (handle == 42).then_some(77)
    }
    fn validate_window_access(&self, _: Option<u32>, _: u32) -> Result<(), String> {
        if self.allowed {
            Ok(())
        } else {
            Err("same-origin native read refused".into())
        }
    }
    fn get(&mut self, handle: u32, key: &str) -> Result<Value, String> {
        assert_eq!((handle, key), (42, "value"));
        self.gets += 1;
        Ok(Value::Number(37.0))
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected native set")
    }
    fn call(&mut self, handle: u32, method: &str, _: &[Value]) -> Result<Value, String> {
        assert_eq!((handle, method), (42, "close"));
        self.allowed = false;
        Ok(Value::Undefined)
    }
}

#[test]
fn reflect_get_native_default_receiver_access_revalidation_and_finite_substitution_boundary() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("native", Value::Host(42)).unwrap();
    let mut host = Native {
        allowed: true,
        gets: 0,
    };
    assert_eq!(
        runtime
            .execute(
                r#"
        var effects = 0, key = {toString: function () { effects++; return 'value'; }};
        if (Reflect.get(native, 'value') !== 37 || Reflect.get(native, 'value', native) !== 37)
            throw 'default native access';
        'ok';
    "#,
                &mut host
            )
            .unwrap()
            .to_text(),
        "ok"
    );
    assert_eq!(host.gets, 2);
    let state = (
        runtime.host_objects.len(),
        runtime.native_expando_owners.len(),
        host.gets,
    );
    let error = runtime
        .execute("Reflect.get(native, key, {});", &mut host)
        .unwrap_err();
    assert!(error.contains("different receiver"), "{error}");
    assert_eq!(
        (
            runtime.host_objects.len(),
            runtime.native_expando_owners.len(),
            host.gets
        ),
        state
    );
    assert_eq!(
        runtime
            .binding_value(runtime.global_environment, "effects")
            .unwrap()
            .to_number(),
        1.0
    );
    host.allowed = false;
    let error = runtime
        .execute("Reflect.get(native, key);", &mut host)
        .unwrap_err();
    assert!(error.contains("same-origin"), "{error}");
    assert_eq!(
        runtime
            .binding_value(runtime.global_environment, "effects")
            .unwrap()
            .to_number(),
        1.0
    );
    assert_eq!(host.gets, 2);
    host.allowed = true;
    runtime.begin_job();
    runtime
        .execute(
            "key.toString = function () { effects++; native.close(); return 'value'; };",
            &mut host,
        )
        .unwrap();
    let error = runtime
        .execute("Reflect.get(native, key);", &mut host)
        .unwrap_err();
    assert!(error.contains("same-origin"), "{error}");
    assert_eq!(
        runtime
            .binding_value(runtime.global_environment, "effects")
            .unwrap()
            .to_number(),
        2.0
    );
    assert_eq!(host.gets, 2);
    host.allowed = true;
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("Reflect.get(native, 'value');", &mut host)
            .unwrap()
            .to_number(),
        37.0
    );
    assert_eq!(host.gets, 3);
    assert!(runtime.transient_registration_is_empty());
}

struct WindowReceiverHost {
    same_origin: bool,
}
impl Host for WindowReceiverHost {
    fn native_access_owner(&self, handle: u32) -> Option<u32> {
        matches!(handle, 77 | 88).then_some(handle)
    }
    fn window_context(&self, handle: u32) -> Option<WindowContext> {
        (handle == 88).then_some(WindowContext {
            document: 89,
            location: 90,
        })
    }
    fn validate_window_access(&self, caller: Option<u32>, window: u32) -> Result<(), String> {
        assert_eq!((caller, window), (Some(77), 88));
        if self.same_origin {
            Ok(())
        } else {
            Err("same-origin receiver access refused".into())
        }
    }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected native get")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected native set")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected native call")
    }
}

#[test]
fn reflect_get_foreign_native_receiver_guards_precede_string_and_symbol_getters() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("window", Value::Host(77)).unwrap();
    runtime.define_global("foreign", Value::Host(88)).unwrap();
    runtime.bind_browser_global(77).unwrap();
    let mut host = WindowReceiverHost { same_origin: true };
    runtime
        .execute(
            r#"
        var get = Reflect.get, target = {}, symbol = Symbol('receiver'), getterRuns = 0;
        function getter() { 'use strict'; getterRuns++; return this; }
        Object.defineProperty(target, 'value', {get: getter});
        Object.defineProperty(target, symbol, {get: getter});
        if (get(target, 'value', window) !== window || get(target, symbol, window) !== window)
            throw 'own Window receiver';
    "#,
            &mut host,
        )
        .unwrap();
    for same_origin in [true, false] {
        host.same_origin = same_origin;
        for source in [
            "get(target, 'value', foreign);",
            "get(target, symbol, foreign);",
        ] {
            let allocations = runtime.window_allocation_state_for_test();
            let stores = (
                runtime.host_objects.len(),
                runtime.native_expando_owners.len(),
                runtime.intrinsic_realms.len(),
            );
            let before = runtime
                .binding_value(runtime.global_environment, "getterRuns")
                .unwrap()
                .to_number();
            let error = runtime.execute(source, &mut host).unwrap_err();
            let expected = if same_origin {
                "foreign Window operation is not implemented"
            } else {
                "same-origin"
            };
            assert!(error.contains(expected), "{source}: {error}");
            assert_eq!(runtime.window_allocation_state_for_test(), allocations);
            assert_eq!(
                (
                    runtime.host_objects.len(),
                    runtime.native_expando_owners.len(),
                    runtime.intrinsic_realms.len()
                ),
                stores
            );
            assert_eq!(
                runtime
                    .binding_value(runtime.global_environment, "getterRuns")
                    .unwrap()
                    .to_number(),
                before
            );
            assert_eq!(
                (runtime.active_realm, runtime.native_access_caller),
                (None, None)
            );
            runtime.begin_job();
            assert_eq!(
                runtime
                    .execute("get(target, 'value', window) === window;", &mut host)
                    .unwrap()
                    .to_text(),
                "true"
            );
            assert!(runtime.transient_registration_is_empty());
        }
    }
}

#[test]
fn reflect_get_copy_work_and_instruction_failures_do_not_run_getter_or_publish_state() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute(
            r#"
        var target = {}, symbol = Symbol('key'), getterRuns = 0;
        function getter() { 'use strict'; getterRuns++; return this; }
        Object.defineProperty(target, 'value', {get: getter});
        Object.defineProperty(target, symbol, {get: getter});
        target.data = 7;
    "#,
            &mut NoHost,
        )
        .unwrap();
    let target = runtime
        .binding_value(runtime.global_environment, "target")
        .unwrap();
    let symbol = runtime
        .binding_value(runtime.global_environment, "symbol")
        .unwrap();
    let receiver = Value::String("A".repeat(8192));
    for key in [Value::String("value".into()), symbol] {
        runtime.begin_job();
        runtime.copy_bytes = 128;
        let error = runtime
            .reflect_get(&[target.clone(), key, receiver.clone()], &mut NoHost)
            .unwrap_err();
        assert!(error.message.contains("copy work limit"), "{error:?}");
        assert!(error.exception.is_none());
        assert_eq!(
            runtime
                .binding_value(runtime.global_environment, "getterRuns")
                .unwrap()
                .to_number(),
            0.0
        );
        assert!(runtime.transient_registration_is_empty());
    }
    runtime.begin_job();
    runtime.copy_bytes = 128;
    assert_eq!(
        runtime
            .reflect_get(
                &[target.clone(), Value::String("data".into()), receiver],
                &mut NoHost
            )
            .unwrap()
            .to_number(),
        7.0
    );
    runtime.begin_job();
    runtime.steps = 0;
    assert!(runtime
        .reflect_get(&[target, Value::String("value".into())], &mut NoHost)
        .is_err());
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute(
                "if (getterRuns !== 0) throw 'failed read ran getter'; Reflect.get({x: 19}, 'x');",
                &mut NoHost
            )
            .unwrap()
            .to_number(),
        19.0
    );
}
