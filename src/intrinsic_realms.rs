//! Bounded allocation of independent intrinsic roots and detached global scopes.
//!
//! Private realm execution is a prerequisite for real frames, not an iframe API.
//! Window/Document routing must be implemented before publishing a frame. All
//! allocation/work/deadline accounting stays shared.
use super::*;

const MAX_INTRINSIC_REALMS: usize = 8;
pub(super) const BINARY_GLOBALS: &[&str] = &["ArrayBuffer", "DataView", "Int8Array", "Uint8Array", "Uint8ClampedArray",
    "Int16Array", "Uint16Array", "Int32Array", "Uint32Array", "Float16Array", "Float32Array", "Float64Array"];
pub(super) const URL_GLOBALS: &[&str] = &["URL", "URLSearchParams"];
pub(super) const TEXT_GLOBALS: &[&str] = &["TextEncoder", "TextDecoder"];
pub(super) const RECT_GLOBALS: &[&str] = &["DOMRectReadOnly", "DOMRect"];
pub(super) const DOM_EXCEPTION_GLOBALS: &[&str] = &["DOMException"];

macro_rules! roots {
    ($($field:ident: $kind:ty),* $(,)?) => {
        #[derive(Default)]
        pub(super) struct Roots { $(pub(super) $field: $kind,)* }
        impl Roots {
            fn take(runtime: &mut Runtime) -> Self {
                Self { $($field: std::mem::take(&mut runtime.$field),)* }
            }
            fn restore(self, runtime: &mut Runtime) {
                $(runtime.$field = self.$field;)*
            }
        }
    }
}

roots! {
    symbol_prototype: Option<usize>,
    binary_roots: binary_data::Roots,
    collection_roots: collections::Roots,
    text_roots: text_encoding::Roots,
    object_prototype: Option<usize>,
    function_prototype: Option<usize>,
    array_prototype: Option<usize>,
    regexp_prototype: Option<usize>,
    regexp_constructor: Option<usize>,
    date_prototype: Option<usize>,
    url_roots: url_api::Roots,
    rect_roots: rect_api::Roots,
    dom_exception_roots: dom_exception::Roots,
    string_prototype: Option<usize>,
    number_prototype: Option<usize>,
    boolean_prototype: Option<usize>,
    error_prototypes: BTreeMap<&'static str, usize>,
    browser_global: Option<(u32, usize)>,
    native_constructor_prototypes:BTreeMap<(u32,&'static str),usize>,
    global_attributes: BTreeMap<String, (bool, bool, bool)>,
    deferred_global_orders: BTreeMap<&'static str, usize>,
    host_globals: HashSet<String>,
    operation_globals: HashSet<&'static str>,
    arguments_thrower: Option<usize>,
    array_to_string_intrinsic: Option<usize>,
    deferred_intrinsics: u8,
    failed_intrinsics: u8,
}

#[allow(dead_code)] // Not exposed until callee-realm/Window routing is complete.
pub(super) struct IntrinsicRealm {
    pub(super) global_environment: usize,
    roots: Roots,
    promise_roots: (Option<usize>, Option<usize>),
}

pub(super) struct ParkedRoots {
    roots: Roots,
    promise_roots: (Option<usize>, Option<usize>),
}

impl Runtime {
    pub(super) fn native_ancestry_anchor(&self, ancestry:NativeAncestry) -> Result<Ordinary,Fault> {
        let realm = self.window_realms.get(&ancestry.window).copied()
            .ok_or_else(||Fault::new("Native prototype owner realm is not initialized."))?;
        let (object,constructors) = if realm==self.active_realm {
            (self.object_prototype,&self.native_constructor_prototypes)
        } else {
            let roots = if let Some(realm)=realm {
                &self.intrinsic_realms.get(realm).ok_or_else(||Fault::new("Invalid native prototype realm."))?.roots
            } else {
                &self.parked_main_roots.as_ref().ok_or_else(||Fault::new("Main native prototype roots are unavailable."))?.roots
            };
            (roots.object_prototype,&roots.native_constructor_prototypes)
        };
        let id = if let Some(key)=ancestry.constructor {constructors.get(&key).copied()}
            else {object};
        id.map(Ordinary::Object).ok_or_else(||Fault::new("Canonical native prototype anchor is unavailable."))
    }

    pub(super) fn reachability_intrinsic_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        macro_rules! emit {
            ($roots:expr) => {{
                let roots = $roots;
                for id in [roots.symbol_prototype, roots.object_prototype, roots.array_prototype, roots.regexp_prototype,
                    roots.date_prototype, roots.string_prototype, roots.number_prototype, roots.boolean_prototype].into_iter().flatten() { add(Edge::Object(id)); }
                for id in [roots.function_prototype, roots.regexp_constructor, roots.arguments_thrower, roots.array_to_string_intrinsic].into_iter().flatten() { add(Edge::Function(id)); }
                for &id in roots.error_prototypes.values() { add(Edge::Object(id)); }
                if let Some((_, id)) = roots.browser_global { add(Edge::Object(id)); }
                for &id in roots.native_constructor_prototypes.values() { add(Edge::Object(id)); }
                roots.binary_roots.reachability_edges(add);
                roots.collection_roots.reachability_edges(add);
                roots.text_roots.reachability_edges(add);
                roots.url_roots.reachability_edges(add);
                roots.rect_roots.reachability_edges(add);
                roots.dom_exception_roots.reachability_edges(add);
            }};
        }
        emit!(self);
        if let Some(parked) = &self.parked_main_roots {
            emit!(&parked.roots);
            if let Some(id) = parked.promise_roots.0 { add(Edge::Object(id)); }
            if let Some(id) = parked.promise_roots.1 { add(Edge::Function(id)); }
        }
        for realm in &self.intrinsic_realms {
            emit!(&realm.roots);
            if let Some(id) = realm.promise_roots.0 { add(Edge::Object(id)); }
            if let Some(id) = realm.promise_roots.1 { add(Edge::Function(id)); }
        }
    }

    pub(super) fn ensure_global_intrinsics(&mut self, name: &str) -> Result<(), Fault> {
        if self.deferred_intrinsics == 0 && self.failed_intrinsics == 0 { return Ok(()); }
        let (bit, names) = if BINARY_GLOBALS.contains(&name) { (1, BINARY_GLOBALS) }
            else if URL_GLOBALS.contains(&name) { (2, URL_GLOBALS) }
            else if TEXT_GLOBALS.contains(&name) { (4, TEXT_GLOBALS) }
             else if RECT_GLOBALS.contains(&name) { (8, RECT_GLOBALS) }
            else if DOM_EXCEPTION_GLOBALS.contains(&name) { (16, DOM_EXCEPTION_GLOBALS) }
            else { return Ok(()); };
        if self.failed_intrinsics & bit != 0 {
            return Err(Fault::new("Deferred intrinsic family is unavailable after an earlier bounded initialization failure."));
        }
        if self.deferred_intrinsics & bit == 0 { return Ok(()); }
        self.step()?;
        self.deferred_intrinsics &= !bit;
        let result = (|| {
            match bit {
                1 => self.initialize_binary()?,
                2 => self.initialize_url_api()?,
                4 => { self.ensure_global_intrinsics("Uint8Array")?; self.initialize_text_encoding()?; }
                8 => self.initialize_rect_api()?,
                16 => self.initialize_dom_exception()?,
                _ => unreachable!(),
            }
            for &name in names {
                self.work(name.len())?;
                self.global_attributes.insert(name.into(), (false, true, true));
            }
            Ok(())
        })();
        if result.is_err() { self.failed_intrinsics |= bit; }
        result
    }

    pub(super) fn foreign_window(&self, handle: u32, host: &dyn Host) -> bool {
        !self.is_browser_global(handle)
            && (self.window_realms.contains_key(&handle) || host.window_context(handle).is_some())
    }

    pub(super) fn reject_foreign_window(&self, value: &Value, host: &dyn Host) -> Result<(), Fault> {
        if let Value::Host(handle) = value { self.validate_native_access(*handle, host)?; }
        if matches!(value, Value::Host(handle) if self.foreign_window(*handle, host)) {
            return Err(Fault::new("This foreign Window operation is not implemented; only named property reads are supported."));
        }
        Ok(())
    }

    pub(super) fn read_foreign_window(&mut self, handle: u32, name: &str, host: &mut dyn Host) -> Result<Value, Fault> {
        let caller=self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(window,_)|window));
        host.validate_window_access(caller, handle).map_err(Fault::new)?;
        // Native identity/document/location reads do not require allocating the
        // child's ES intrinsics. The host already owns a real child context.
        if host.owns_window_property(Some(handle), name) {
            let value = self.host_get_checked(handle, name, host)?;
            return self.checked_value(value);
        }
        let realm = self.ensure_window_realm(handle,host)?;
        self.with_intrinsic_realm(realm, |runtime| runtime.global_value(name, host))
    }

    pub(super) fn ensure_window_realm(&mut self,handle:u32,host:&mut dyn Host)->Result<Option<usize>,Fault> {
        let caller=self.browser_global.map(|(window,_)|window);
        if caller!=Some(handle) {host.validate_window_access(caller,handle).map_err(Fault::new)?;}
        if let Some(realm) = self.window_realms.get(&handle).copied() {
            Ok(realm)
        } else {
            if self.failed_window_realms.contains(&handle) {
                return Err(Fault::new("Child Window intrinsics are unavailable after an earlier bounded initialization failure."));
            }
            let context = host.window_context(handle).ok_or_else(|| Fault::new("Unknown child Window context."))?;
            let realm = match self.allocate_realm_with_deferred_families(true) {
                Ok(realm) => realm,
                Err(error) => { self.failed_window_realms.insert(handle); return Err(error); }
            };
            let result = self.with_intrinsic_realm(Some(realm), |runtime| {
                for (name, value) in [("window", handle), ("self", handle), ("globalThis", handle),
                    ("document", context.document), ("location", context.location)] {
                    runtime.define_global(name, Value::Host(value)).map_err(Fault::new)?;
                }
                runtime.bind_browser_global(handle).map_err(Fault::new)?;
                if host.event_target_interface(handle) {runtime.define_event_target_interface(handle).map_err(Fault::new)?;}
                if host.event_target_interface(handle) {runtime.define_event_interface(handle).map_err(Fault::new)?;}
                if host.event_target_interface(handle) {runtime.define_custom_event_interface(handle).map_err(Fault::new)?;}
                if host.event_target_interface(handle) {runtime.define_ui_event_interface(handle).map_err(Fault::new)?;}
                if host.event_target_interface(handle) {runtime.define_mouse_event_interface(handle).map_err(Fault::new)?;}
                if let Some(constants)=host.node_interface_constants(handle) {
                    runtime.define_node_interfaces(handle,constants,None).map_err(Fault::new)?;
                    runtime.define_media_interfaces(handle).map_err(Fault::new)?;
                }
                Ok(())
            });
            if let Err(error) = result {
                // The real host context remains. No partial realm is published;
                // failed allocation remains charged under aggregate arena caps.
                self.window_realms.remove(&handle);
                self.failed_window_realms.insert(handle);
                return Err(error);
            }
            Ok(Some(realm))
        }
    }

    pub(super) fn immutable_object_prototype(&self, id: usize) -> bool {
        self.object_prototype == Some(id)
            || self.parked_main_roots.as_ref().is_some_and(|main| main.roots.object_prototype == Some(id))
            || self.intrinsic_realms.iter().any(|realm| realm.roots.object_prototype == Some(id))
    }

    #[allow(dead_code)]
    pub(super) fn allocate_intrinsic_realm(&mut self) -> Result<usize, Fault> {
        self.allocate_realm_with_deferred_families(false)
    }

    fn allocate_realm_with_deferred_families(&mut self, deferred: bool) -> Result<usize, Fault> {
        if self.intrinsic_realms.len() >= MAX_INTRINSIC_REALMS {
            return Err(Fault::new("Intrinsic realm limit reached (8 detached realms)."));
        }
        self.step()?;
        let global_environment = self.new_environment(self.global_environment, true)?;
        self.environment_mut(global_environment).parent = None;
        self.environment_mut(global_environment).captured = true;
        let id = self.intrinsic_realms.len();
        let previous = self.active_realm;
        self.intrinsic_realms.push(IntrinsicRealm {
            global_environment, roots: Roots::default(), promise_roots: (None, None),
        });
        let result = self.with_intrinsic_realm(Some(id), |runtime| {
            runtime.deferred_intrinsics = (if deferred { 7 } else { 0 }) | 8 | 16;
            runtime.initialize_realm_intrinsics()
        });
        if let Err(error) = result {
            // Failed work is not refunded. Unreachable partial arena objects
            // remain bounded by the original aggregate caps; no roots/handles
            // are published. Drop the unexposed global's binding storage.
            self.discard_environment(global_environment);
            self.intrinsic_realms.pop();
            for (function, realm) in self.functions.iter().zip(&mut self.function_realms) {
                if !matches!(function, Function::Vacant) && *realm == Some(id) { *realm = previous; }
            }
            return Err(error);
        }
        Ok(id)
    }

    fn switch_intrinsic_realm(&mut self, target: Option<usize>) -> Result<(), Fault> {
        if target == self.active_realm { return Ok(()); }
        if target.is_some_and(|id| id >= self.intrinsic_realms.len()) {
            return Err(Fault::new("Invalid intrinsic realm handle."));
        }
        let parked = ParkedRoots { roots: Roots::take(self),
            promise_roots: (self.promises.prototype.take(), self.promises.constructor.take()) };
        if let Some(id) = self.active_realm {
            self.intrinsic_realms[id].roots = parked.roots;
            self.intrinsic_realms[id].promise_roots = parked.promise_roots;
        } else {
            self.parked_main_roots = Some(parked);
        }
        let incoming = if let Some(id) = target {
            self.global_environment = self.intrinsic_realms[id].global_environment;
            ParkedRoots { roots: std::mem::take(&mut self.intrinsic_realms[id].roots),
                promise_roots: std::mem::replace(&mut self.intrinsic_realms[id].promise_roots, (None, None)) }
        } else {
            self.global_environment = 0;
            self.parked_main_roots.take().expect("parked main realm")
        };
        incoming.roots.restore(self);
        self.promises.prototype = incoming.promise_roots.0;
        self.promises.constructor = incoming.promise_roots.1;
        self.active_realm = target;
        Ok(())
    }

    #[inline(never)]
    pub(super) fn with_intrinsic_realm<T>(
        &mut self, realm: Option<usize>, action: impl FnOnce(&mut Runtime) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        if realm == self.active_realm { return action(self); }
        let previous = self.active_realm;
        self.switch_intrinsic_realm(realm)?;
        let mut result = action(self);
        let marker = self.inhibit_transient_collection();
        // Materialize only a lazy native Error that crosses a realm boundary,
        // while its originating prototypes are active. Explicit thrown values
        // keep identity; same-realm faults keep the original lazy accounting.
        if let Err(error) = &mut result {
            if let Some(Exception::Error(kind)) = error.exception.as_deref() {
                match self.error_object(kind, Some(error.message.clone())) {
                    Ok(value) => error.exception = Some(Box::new(Exception::Value(value))),
                    Err(fatal) => result = Err(fatal),
                }
            }
        }
        self.switch_intrinsic_realm(previous).expect("restore known caller realm");
        self.restore_transient_marker(marker);
        result
    }

    #[inline(always)]
    pub(super) fn native_getter_in_realm(&mut self, function: &Value, field: NativeGetter, receiver: Value) -> Result<Value, Fault> {
        let marker = self.inhibit_transient_collection();
        let realm = match function { Value::Function(id) => self.function_realms[*id], _ => self.active_realm };
        let result = if realm == self.active_realm { self.native_getter(field, receiver) }
            else { self.with_intrinsic_realm(realm, |runtime| runtime.native_getter(field, receiver)) };
        self.restore_transient_marker(marker);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected host get".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected host set".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected host call".into()) }
    }

    fn constructor(runtime: &Runtime, global: usize, name: &str) -> usize {
        let Some(Value::Function(index)) = runtime.environment(global).bindings[name].value else { panic!("constructor") };
        index
    }

    fn expose_test_realm(runtime: &mut Runtime) -> usize {
        runtime.execute("var realmSecret=91;", &mut NoHost).unwrap();
        let realm = runtime.allocate_intrinsic_realm().unwrap();
        let values = runtime.with_intrinsic_realm(Some(realm), |runtime| {
            runtime.bind_browser_global(77).map_err(Fault::new)?;
            runtime.execute("var realmSecret=17;", &mut NoHost).map_err(Fault::new)?;
            ["Array", "Function", "String", "TypeError", "EvalError", "eval", "Promise", "Uint8Array", "RegExp"]
                .iter().map(|name| runtime.binding_value(runtime.global_environment, name).map(|value| (*name, value)))
                .collect::<Result<Vec<_>, _>>()
        }).unwrap();
        for (name, value) in values {
            let name = if name == "eval" { "ChildEval".into() } else { format!("Child{name}") };
            runtime.define_global(&name, value).unwrap();
        }
        runtime.define_global("ChildWindow", Value::Host(77)).unwrap();
        realm
    }

    #[test]
    fn foreign_constructors_callbacks_closures_and_bound_targets_keep_their_realms() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        runtime.execute(r#"
if(ChildFunction('return realmSecret;')()!==17||realmSecret!==91)throw 'global isolation';
if(ChildFunction('return this;')()!==ChildWindow)throw 'child global this';
if(Object.getPrototypeOf(ChildArray(1,2))!==ChildArray.prototype)throw 'call allocation';
if(Object.getPrototypeOf(new ChildArray(1,2))!==ChildArray.prototype)throw 'construct allocation';
if(Object.getPrototypeOf(Uint8Array.of.call(ChildUint8Array,1))!==ChildUint8Array.prototype||Object.getPrototypeOf(ChildUint8Array.from.call(Uint8Array,[1]))!==Uint8Array.prototype)throw 'typed constructor receiver';
var mapped=ChildArray.prototype.map.call([1],function(){return [Array,[]];});
if(Object.getPrototypeOf(mapped)!==ChildArray.prototype||mapped[0][0]!==Array||Object.getPrototypeOf(mapped[0][1])!==Array.prototype)throw 'callback realm';
if(Object.getPrototypeOf([])!==Array.prototype)throw 'caller restore';
var closure=ChildFunction('return function(){return [realmSecret,Array];};')();
if(closure()[0]!==17||closure()[1]!==ChildArray)throw 'closure realm';
var bound=Function.prototype.bind.call(ChildArray,null,1).bind(null,2);
if(Object.getPrototypeOf(new bound())!==ChildArray.prototype)throw 'bound target realm';
if(ChildFunction('return Symbol.iterator;')()!==Symbol.iterator||ChildFunction('return Symbol.dispose;')()!==Symbol.dispose||ChildFunction('return Symbol.for("realm-key");')()!==Symbol.for('realm-key'))throw 'shared symbols';
'ok';
"#, &mut NoHost).unwrap();
        assert_eq!(runtime.active_realm, None);
        assert_eq!(runtime.global_environment, 0);
        assert_eq!(runtime.function_realms.len(), runtime.functions.len());
        assert_eq!(runtime.function_initial_names.len(), runtime.functions.len());
        runtime.collect_environments();
        assert_eq!(runtime.execute("closure()[0];", &mut NoHost).unwrap().to_number(), 17.0);
    }

    #[test]
    fn foreign_errors_and_direct_indirect_eval_preserve_identity_and_scope() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        assert_eq!(runtime.execute(r#"
var errors=0;
try{ChildString.prototype.charCodeAt.call(null,0);}catch(e){if(Object.getPrototypeOf(e)===ChildTypeError.prototype)errors++;}
try{ChildFunction('return null.x;')();}catch(e){if(Object.getPrototypeOf(e)===ChildTypeError.prototype)errors++;}
try{new ChildArray.prototype.map();throw 'missing nonconstructor';}catch(e){if(Object.getPrototypeOf(e)!==TypeError.prototype)throw 'nonconstructor caller realm';}
var marker={},thrower=ChildFunction('value','throw value;');
try{thrower(marker);}catch(e){if(e===marker)errors++;}
if(errors!==3)throw 'error realm';
function probe(eval){let privateLocal=5;return eval('typeof privateLocal;');}
if(probe(ChildEval)!=='undefined'||probe(eval)!=='number')throw 'eval identity';
ChildEval('var childOnly=27;');
if(typeof childOnly!=='undefined'||ChildFunction('return childOnly;')()!==27)throw 'indirect global';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
    }

    #[test]
    fn borrowed_source_stringification_keeps_foreign_source_and_ignores_names() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        assert_eq!(runtime.execute(r#"
var f=ChildFunction('x','return x;'),read=Function.prototype.toString;
if(read.call(f)!=='function anonymous(x\n) {\nreturn x;\n}')throw 'foreign dynamic source';
var nested=ChildFunction('return function /*child*/ named(){return 17;};')();
var childRead=ChildFunction('return Function.prototype.toString;')();
if(read.call(nested)!=='function /*child*/ named(){return 17;}'||childRead.call(nested)!==read.call(nested))throw 'borrowed source';
Object.defineProperty(nested,'name',{get:function(){throw 'name getter';}});
if(read.call(nested)!=='function /*child*/ named(){return 17;}')throw 'mutable source name';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
        assert_eq!(runtime.function_initial_names.len(), runtime.functions.len());
    }

    #[test]
    fn regexp_call_identity_observes_constructor_and_creation_realm() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        assert_eq!(runtime.execute(r#"
var original=/x/g, NativeRegExp=RegExp, copied=ChildRegExp(original);
if(copied===original||Object.getPrototypeOf(copied)!==ChildRegExp.prototype||copied.source!=='x'||copied.flags!=='g')throw 'foreign regexp';
if(NativeRegExp(original)!==original||ChildRegExp(copied)!==copied)throw 'native identity';
var reads=0;Object.defineProperty(original,'constructor',{get:function(){reads++;return ChildRegExp;},configurable:true});
if(ChildRegExp(original)!==original||reads!==1)throw 'constructor getter';
if(new ChildRegExp(original)===original||reads!==1)throw 'construct must clone without getter';
var marker={};Object.defineProperty(original,'constructor',{get:function(){throw marker;}});
try{ChildRegExp(original);throw 'missing getter exception';}catch(e){if(e!==marker)throw 'getter exception identity';}
RegExp=function(){throw 'mutable global';};
var same=/y/;if(NativeRegExp(same)!==same)throw 'intrinsic identity';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
    }

    #[test]
    fn regexp_input_conversion_preserves_callback_and_builtin_owner_realms() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        assert_eq!(runtime.execute(r#"
var childInput=ChildFunction('var owner=Array,value={};value[Symbol.toPrimitive]=function(h){if(Array!==owner||this!==value||h!=="string")throw "conversion owner";return "child";};return value;')();
var childTest=ChildRegExp.prototype.test,childExec=ChildRegExp.prototype.exec;
if(!/child/.test(childInput)||!childTest.call(/child/,childInput))throw 'borrowed conversion';
var result=childExec.call(/child/,childInput);
if(result[0]!=='child'||Object.getPrototypeOf(result)!==ChildArray.prototype||Array===ChildArray||realmSecret!==91)throw 'exec result owner and caller restore';
var invalid=ChildFunction('return {toString:function(){return Symbol("invalid");}};')(),caught=false;
try{childTest.call(/x/,invalid);}catch(e){caught=Object.getPrototypeOf(e)===ChildTypeError.prototype;}
if(!caught||realmSecret!==91||Object.getPrototypeOf([])!==Array.prototype)throw 'conversion failure owner and restore';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
        assert!(runtime.transient_registration_is_empty());
    }

    #[test]
    fn foreign_jobs_and_compilation_gate_keep_context_and_shared_queue() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        runtime.execute("var observed=0;ChildPromise.resolve(1).then(ChildFunction('return [realmSecret,Array];')).then(function(v){if(v[0]!==17||v[1]!==ChildArray||Array===ChildArray)throw 'job realm';observed=42;});", &mut NoHost).unwrap();
        runtime.execute("ChildEval('var realmJobOK=false;');Promise.resolve({then:ChildFunction('resolve','reject','realmJobOK=Object.getPrototypeOf(resolve)===Function.prototype;resolve(1);')});", &mut NoHost).unwrap();
        assert!(runtime.has_microtasks());
        assert!(runtime.microtask_checkpoint(&mut NoHost).unwrap().is_empty());
        assert_eq!(runtime.binding_value(0, "observed").unwrap().to_number(), 42.0);
        assert_eq!(runtime.execute("ChildFunction('return realmJobOK;')();", &mut NoHost).unwrap().to_text(), "true");
        struct Deny;
        impl Host for Deny {
            fn allow_eval(&mut self) -> Result<bool, String> { Ok(false) }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected host get".into()) }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected host set".into()) }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected host call".into()) }
        }
        assert_eq!(runtime.execute("var blocked=0;try{ChildFunction('return 1;');}catch(e){if(Object.getPrototypeOf(e)===ChildEvalError.prototype)blocked++;}try{ChildEval('1;');}catch(e){if(Object.getPrototypeOf(e)===ChildEvalError.prototype)blocked++;}blocked;", &mut Deny).unwrap().to_number(), 2.0);
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
    }

    #[test]
    fn foreign_call_guard_and_recovery_fit_one_mib_without_resetting_job() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut runtime = Runtime::for_capacity_test();
            expose_test_realm(&mut runtime);
            runtime.execute("var child=ChildFunction('n','callback','if(n>0)return callback(n-1);return 42;');function parent(n){return child(n,parent);}", &mut NoHost).unwrap();
            runtime.begin_job();
            let deadline = runtime.job_deadline;
            assert_eq!(runtime.execute("parent(8);", &mut NoHost).unwrap().to_number(), 42.0);
            assert_eq!(runtime.job_deadline, deadline);
            runtime.begin_job();
            let error = runtime.execute("parent(80);", &mut NoHost).unwrap_err();
            assert!(error.contains("limit"), "{error}");
            assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
            assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
            assert!(runtime.parked_main_roots.is_none());
            runtime.begin_job();
            assert_eq!(runtime.execute("parent(1);", &mut NoHost).unwrap().to_number(), 42.0);
        }).unwrap().join().unwrap();
    }

    #[test]
    fn child_global_declarations_keep_operation_preflight_and_eval_deletion() {
        let mut runtime = Runtime::for_capacity_test();
        let realm = expose_test_realm(&mut runtime);
        runtime.with_intrinsic_realm(Some(realm), |runtime| {
            runtime.define_global_operation("setTimeout", 77, "setTimeout", 1).map_err(Fault::new)?;
            runtime.execute("Object.defineProperty(this,'setTimeout',{configurable:false});", &mut NoHost).map_err(Fault::new)?;
            let program = runtime.register_program("let setTimeout=1;", false)?;
            let error = runtime.prepare(&program, &program.body, runtime.global_environment, true, &mut NoHost).err().expect("restricted child global");
            assert!(matches!(error.exception.as_deref(), Some(Exception::Error("SyntaxError"))));
            assert!(error.message.contains("Lexical binding conflicts with global property"));
            assert_eq!(runtime.execute("var setTimeout=8;setTimeout;", &mut NoHost).unwrap().to_number(), 8.0);
            assert!(!runtime.environment(runtime.global_environment).bindings.contains_key("setTimeout"));
            Ok(())
        }).unwrap();
        assert_eq!(runtime.execute("ChildEval('var removable=5;');if(ChildEval('delete removable;')!==true||ChildEval('typeof removable;')!=='undefined')throw 'deletion';typeof removable;", &mut NoHost).unwrap().to_text(), "undefined");
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
    }

    #[test]
    fn weak_map_keys_and_borrowed_methods_cross_intrinsic_realms() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        let result = runtime.execute(r#"
            var ForeignMap = ChildFunction('return WeakMap;')();
            var foreignKey = ChildFunction('return {};')();
            var localKey = {}, localMap = new WeakMap(), foreignMap = new ForeignMap();
            if (ForeignMap === WeakMap || ForeignMap.prototype === WeakMap.prototype)
                throw 'shared constructors or prototypes';
            if (Object.getPrototypeOf(foreignMap) !== ForeignMap.prototype)
                throw 'wrong instance prototype';
            WeakMap.prototype.set.call(foreignMap, localKey, foreignKey);
            ForeignMap.prototype.set.call(localMap, foreignKey, localKey);
            if (foreignMap.get(localKey) !== foreignKey || localMap.get(foreignKey) !== localKey)
                throw 'cross-realm identity';
            if (foreignMap.has(foreignKey) || localMap.has(localKey))
                throw 'entries leaked between maps';
            try { ForeignMap.prototype.get.call({}, foreignKey); }
            catch (e) { if (Object.getPrototypeOf(e) === ChildTypeError.prototype) 'foreign brand error'; }
        "#, &mut NoHost).unwrap();
        assert_eq!(result.to_text(), "foreign brand error");
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
    }

    #[test]
    fn weak_set_members_and_borrowed_methods_cross_intrinsic_realms() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        let result = runtime.execute(r#"
            var ForeignSet = ChildFunction('return WeakSet;')();
            var foreignKey = ChildFunction('return {};')(), localKey = {};
            var foreignSet = new ForeignSet(), localSet = new WeakSet();
            if (ForeignSet === WeakSet || ForeignSet.prototype === WeakSet.prototype ||
                Object.getPrototypeOf(foreignSet) !== ForeignSet.prototype)
                throw 'wrong realm prototype';
            if (WeakSet.prototype.add.call(foreignSet, localKey) !== foreignSet ||
                ForeignSet.prototype.add.call(localSet, foreignKey) !== localSet ||
                !localSet.has(foreignKey) || !foreignSet.has(localKey) || localSet.has(localKey))
                throw 'cross-realm membership';
            try { ForeignSet.prototype.has.call(new WeakMap(), foreignKey); }
            catch (e) { if (Object.getPrototypeOf(e) === ChildTypeError.prototype) 'foreign brand error'; }
        "#, &mut NoHost).unwrap();
        assert_eq!(result.to_text(), "foreign brand error");
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
    }

    #[test]
    fn detached_intrinsics_are_real_distinct_and_do_not_inherit_parent_mutation() {
        let mut runtime = Runtime::for_capacity_test();
        let parent_object = runtime.object_prototype.unwrap();
        let parent_array = runtime.array_prototype.unwrap();
        let parent_constructor = constructor(&runtime, 0, "Array");
        runtime.set_property(parent_object, "parentOnly".into(), Value::Number(7.0)).unwrap();
        let deadline = runtime.job_deadline;
        let usage = runtime.job_usage();
        let realm = runtime.allocate_intrinsic_realm().unwrap();
        let record = &runtime.intrinsic_realms[realm];
        let global = record.global_environment;
        let object = record.roots.object_prototype.unwrap();
        let array = record.roots.array_prototype.unwrap();
        assert_ne!(object, parent_object);
        assert_ne!(array, parent_array);
        let child_constructor = constructor(&runtime, global, "Array");
        assert_ne!(child_constructor, parent_constructor);
        assert_eq!(runtime.objects[object].prototype, None);
        assert!(!runtime.objects[object].properties.contains_key("parentOnly"));
        assert!(runtime.immutable_object_prototype(object));
        let other = runtime.new_object(None).unwrap();
        assert!(runtime.set_prototype(Ordinary::Object(object), Some(Ordinary::Object(other))).is_err());
        assert_eq!(runtime.objects[object].prototype, None);
        assert_eq!(runtime.object_prototype, Some(parent_object));
        assert_eq!(runtime.array_prototype, Some(parent_array));
        assert_eq!(constructor(&runtime, 0, "Array"), parent_constructor);
        assert_eq!(runtime.job_deadline, deadline);
        assert!(runtime.job_usage().0 > usage.0 && runtime.job_usage().1 > usage.1);
        runtime.collect_environments();
        assert_eq!(constructor(&runtime, global, "Array"), child_constructor);
    }

    #[test]
    fn realm_bootstrap_reuses_agent_symbols_and_preserves_parent_jobs() {
        let mut runtime = Runtime::for_capacity_test();
        let ids = (runtime.symbol_dispose, runtime.symbol_to_primitive, runtime.symbol_iterator, runtime.symbol_species);
        let symbols = runtime.symbols.len();
        let promise_roots = (runtime.promises.prototype, runtime.promises.constructor);
        runtime.execute("var realmJob=0;Promise.resolve().then(function(){realmJob=42;});", &mut NoHost).unwrap();
        assert!(runtime.has_microtasks());
        runtime.allocate_intrinsic_realm().unwrap();
        assert_eq!((runtime.symbol_dispose, runtime.symbol_to_primitive, runtime.symbol_iterator, runtime.symbol_species), ids);
        assert_eq!(runtime.symbols.len(), symbols);
        assert_eq!((runtime.promises.prototype, runtime.promises.constructor), promise_roots);
        assert!(runtime.has_microtasks());
        assert!(runtime.microtask_checkpoint(&mut NoHost).unwrap().is_empty());
        assert_eq!(runtime.binding_value(0, "realmJob").unwrap().to_number(), 42.0);
    }

    #[test]
    fn failed_realm_bootstrap_restores_parent_without_refunding_work_or_time() {
        for extra_objects in [0, 4, 32] {
        let mut runtime = Runtime::for_capacity_test();
        let object = runtime.object_prototype;
        let constructor = constructor(&runtime, 0, "Array");
        let deadline = runtime.job_deadline;
        let usage = runtime.job_usage();
        runtime.limits.heap.objects = runtime.objects.len() + extra_objects;
        assert!(runtime.allocate_intrinsic_realm().is_err());
        assert!(runtime.intrinsic_realms.is_empty());
        assert_eq!(runtime.object_prototype, object);
        assert_eq!(self::constructor(&runtime, 0, "Array"), constructor);
        assert_eq!(runtime.job_deadline, deadline);
        assert!(runtime.job_usage().0 > usage.0);
        assert!(runtime.job_usage().1 >= usage.1);
        runtime.begin_job();
        assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
        }
    }

    #[test]
    fn repeated_realm_bootstrap_is_bounded_on_one_mib_stack() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut runtime = Runtime::for_capacity_test();
            let parent = constructor(&runtime, 0, "Array");
            let deadline = runtime.job_deadline;
            let mut successes = 0;
            let mut rejected = false;
            for _ in 0..=MAX_INTRINSIC_REALMS {
                match runtime.allocate_intrinsic_realm() {
                    Ok(_) => successes += 1,
                    Err(_) => { rejected = true; break; }
                }
            }
            assert!(successes > 0 && successes <= MAX_INTRINSIC_REALMS);
            assert!(rejected);
            assert_eq!(constructor(&runtime, 0, "Array"), parent);
            assert_eq!(runtime.job_deadline, deadline);
            assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
            runtime.collect_environments();
            for realm in &runtime.intrinsic_realms {
                assert!(runtime.environments[realm.global_environment].is_some());
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn deferred_main_families_preserve_declarations_replacements_and_siblings() {
        for (name, sibling) in [("URL", "URLSearchParams"), ("Uint8Array", "ArrayBuffer"), ("TextEncoder", "TextDecoder")] {
            let mut runtime = Runtime::for_capacity_test();
            let source = format!("function {name}(){{return 17;}}if({name}()!==17||typeof {sibling}!=='function')throw 'declaration override';");
            runtime.execute(&source, &mut NoHost).unwrap();
            let mut runtime = Runtime::for_capacity_test();
            runtime.define_global(name, Value::Number(17.0)).unwrap();
            runtime.execute(&format!("if({name}!==17||typeof {sibling}!=='function')throw 'host override';"), &mut NoHost).unwrap();
            runtime.execute(&format!("{name}=23;if({name}!==23||delete {name})throw 'host replacement';"), &mut NoHost).unwrap();
            let mut runtime = Runtime::for_capacity_test();
            runtime.bind_browser_global(77).unwrap();
            runtime.define_global("window", Value::Host(77)).unwrap();
            runtime.execute(&format!("window.{name}=23;if({name}!==23||!delete window.{name}||typeof {name}!=='undefined'||typeof {sibling}!=='function')throw 'no resurrection';"), &mut NoHost).unwrap();
        }
    }

    #[test]
    fn deferred_main_host_registration_allocates_after_family_and_fails_before_capability() {
        struct Record(usize);
        impl Host for Record {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
            fn call(&mut self, object: u32, method: &str, _: &[Value]) -> Result<Value, String> {
                assert_eq!((object, method), (1, "record")); self.0 += 1; Ok(Value::Number(17.0))
            }
        }
        let mut runtime = Runtime::for_capacity_test();
        let value = runtime.define_host_function("URL", 1, "record").unwrap();
        assert!(matches!(value, Value::Function(id) if matches!(runtime.functions[id], Function::Host { object: 1, method: "record", .. })));
        let mut host = Record(0);
        runtime.execute("if(URL()!==17||typeof URLSearchParams!=='function')throw 'registered identity';", &mut host).unwrap();
        assert_eq!(host.0, 1);

        let mut runtime = Runtime::for_capacity_test();
        while runtime.functions.len() < MAX_FUNCTIONS - 2 { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
        let error = runtime.define_host_function("URL", 1, "record").unwrap_err();
        assert!(error.contains("Function limit"), "{error}");
        assert!(!runtime.functions.iter().any(|function| matches!(function, Function::Host { .. })));
        assert_eq!(runtime.functions.len(), runtime.function_properties.len());
        assert_eq!(runtime.functions.len(), runtime.function_realms.len());
        assert_eq!(runtime.functions.len(), runtime.function_initial_names.len());
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.job_usage());
        assert!(runtime.define_host_function("URL", 1, "record").unwrap_err().contains("earlier bounded initialization failure"));
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings, runtime.job_usage()), before);

        let mut runtime = Runtime::for_capacity_test();
        runtime.bind_browser_global(77).unwrap();
        assert!(runtime.define_global_operation("URL", 77, "record", 1).unwrap_err().contains("conflicts"));
        assert!(!runtime.functions.iter().any(|function| matches!(function, Function::Host { .. })));
        runtime.execute("if(typeof URL!=='function'||typeof URLSearchParams!=='function')throw 'collision';", &mut NoHost).unwrap();
    }

    #[test]
    fn deferred_main_window_descriptors_convert_before_activation_and_reselect_after_callbacks() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.bind_browser_global(77).unwrap();
        runtime.define_global("window", Value::Host(77)).unwrap();
        runtime.execute("var define=Object.defineProperty,marker=0,sentinel={};var descriptor={get value(){marker=1;throw sentinel;}};", &mut NoHost).unwrap();
        while runtime.functions.len() < MAX_FUNCTIONS { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
        runtime.execute("try{define(window,'URL',descriptor);throw 'missing';}catch(e){if(e!==sentinel||marker!==1)throw 'conversion order';}", &mut NoHost).unwrap();
        assert_eq!(runtime.deferred_intrinsics, 31);

        let mut runtime = Runtime::for_capacity_test();
        runtime.bind_browser_global(77).unwrap();
        runtime.define_global("window", Value::Host(77)).unwrap();
        runtime.execute("var define=Object.defineProperty;var old=URL;define(window,'URL',{get value(){delete window.URL;return 17;},writable:true,configurable:true});if(URL!==17||typeof URLSearchParams!=='function'||Object.getOwnPropertyDescriptor(window,'URL').value!==17)throw 'reselect';delete window.URL;if(Object.getOwnPropertyDescriptor(window,'URL')!==undefined||typeof URL!=='undefined')throw 'resurrection';", &mut NoHost).unwrap();
    }

    #[test]
    fn deferred_main_direct_binary_calls_initialize_the_receiving_realm() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("if(!Number.isFinite(1))throw 'number';", &mut NoHost).unwrap();
        assert_eq!(runtime.deferred_intrinsics, 31);
        let realm = runtime.allocate_realm_with_deferred_families(true).unwrap();
        let foreign = runtime.with_intrinsic_realm(Some(realm), |runtime| {
            runtime.construct_binary(binary_data::Constructor::Buffer, &[Value::Number(4.0)], &mut NoHost)
        }).unwrap();
        assert_eq!(runtime.deferred_intrinsics, 31);
        let view = runtime.construct_binary(binary_data::Constructor::View, &[foreign.clone()], &mut NoHost).unwrap();
        assert_eq!(runtime.deferred_intrinsics, 30);
        runtime.define_global("view", view).unwrap();
        runtime.execute("if(Object.getPrototypeOf(view)!==DataView.prototype||view.byteLength!==4)throw 'view realm';", &mut NoHost).unwrap();

        let mut runtime = Runtime::for_capacity_test();
        let realm = runtime.allocate_realm_with_deferred_families(true).unwrap();
        let foreign = runtime.with_intrinsic_realm(Some(realm), |runtime| {
            let value = runtime.construct_binary(binary_data::Constructor::Buffer, &[Value::Number(4.0)], &mut NoHost)?;
            let Value::Object(id) = value else { unreachable!() };
            // Select the ordinary default-species path; custom foreign species
            // remain explicitly unsupported by the existing binary subset.
            runtime.set_property(id, "constructor".into(), Value::Undefined)?;
            Ok(value)
        }).unwrap();
        let copied = runtime.binary_builtin(binary_data::Op::BufferMethod("slice"), &[], foreign, &mut NoHost).unwrap();
        runtime.define_global("copied", copied).unwrap();
        runtime.execute("if(Object.getPrototypeOf(copied)!==ArrayBuffer.prototype||copied.byteLength!==4)throw 'copy realm';", &mut NoHost).unwrap();
    }

    #[test]
    fn deferred_main_direct_text_construction_has_real_prototypes_and_binary_output() {
        for encoder in [true, false] {
            let mut runtime = Runtime::for_capacity_test();
            let codec = runtime.construct_text_encoding(encoder, &[], &mut NoHost).unwrap();
            assert_eq!(runtime.deferred_intrinsics, 26);
            assert_eq!(runtime.call_depth, 0);
            runtime.define_global("codec", codec).unwrap();
            let source = if encoder {
                "if(Object.getPrototypeOf(codec)!==TextEncoder.prototype)throw 'encoder prototype';var bytes=codec.encode('A');if(Object.getPrototypeOf(bytes)!==Uint8Array.prototype||bytes[0]!==65)throw 'binary prototype';"
            } else {
                "if(Object.getPrototypeOf(codec)!==TextDecoder.prototype||codec.decode(new Uint8Array([65]))!=='A')throw 'decoder prototype';"
            };
            runtime.execute(source, &mut NoHost).unwrap();
        }
    }

    #[test]
    fn failed_deferred_family_stays_bounded_and_restores_parent() {
        let mut runtime = Runtime::for_capacity_test();
        let realm = runtime.allocate_realm_with_deferred_families(true).unwrap();
        while runtime.functions.len() < MAX_FUNCTIONS - 2 {
            runtime.native_function(Builtin::IsNaN, "guardFixture").unwrap();
        }
        let deadline = runtime.job_deadline;
        let error = runtime.with_intrinsic_realm(Some(realm), |runtime| runtime.ensure_global_intrinsics("URL")).unwrap_err();
        assert!(error.message.contains("Function limit"), "{}", error.message);
        assert_eq!(runtime.job_deadline, deadline);
        let retained = (runtime.functions.len(), runtime.objects.len(), runtime.bindings);
        runtime.begin_job();
        let retry = runtime.with_intrinsic_realm(Some(realm), |runtime| runtime.ensure_global_intrinsics("URLSearchParams")).unwrap_err();
        assert!(retry.message.contains("earlier bounded initialization failure"), "{}", retry.message);
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.bindings), retained);
        assert_eq!((runtime.active_realm, runtime.global_environment), (None, 0));
        assert!(runtime.parked_main_roots.is_none());
        assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
    }

    // Borrowed native concat must reenter each callback's own realm.
    #[test]
    fn string_concat_borrowed_native_reentry_uses_live_hooks_and_restores_realms() {
        let mut runtime = Runtime::for_capacity_test();
        expose_test_realm(&mut runtime);
        runtime.execute(r#"
var mainArray=Array,mainPrototype=Array.prototype;
var borrowed=ChildString.prototype.concat,order='';
if(typeof borrowed!=='function'||borrowed===String.prototype.concat)throw 'distinct realm concat';
if(Object.getPrototypeOf(borrowed)!==ChildFunction.prototype)throw 'native function realm';
var receiver={toString:function(){
    if(Object.getPrototypeOf([])!==mainPrototype)throw 'receiver callback realm';
    order+='r';return 'R';
}};
var later={toString:function(){throw 'obsolete later hook';}};
var first={toString:function(){
    if(Object.getPrototypeOf([])!==mainPrototype)throw 'first callback realm';
    order+='a';
    var retained=(function(text){return function(){return text;};})('B');
    later.toString=function(){
        if(Object.getPrototypeOf([])!==mainPrototype)throw 'replacement callback realm';
        order+='b';return retained();
    };
    var nested=String.prototype.concat.call('n',{toString:function(){
        if(Object.getPrototypeOf([])!==mainPrototype)throw 'nested callback realm';
        order+='n';return 'N';
    }});
    if(nested!=='nN')throw 'native reentry result';
    return 'A';
}};
if(borrowed.call(receiver,first,later)!=='RAB'||order!=='ranb')throw 'live ordered conversion hooks';
if(Array!==mainArray||Object.getPrototypeOf([])!==mainPrototype||realmSecret!==91)throw 'main realm after return';

var childError=false;
try{borrowed.call('prefix',Symbol('not a string'));}
catch(error){childError=error.name==='TypeError'&&Object.getPrototypeOf(error)===ChildTypeError.prototype;}
if(!childError)throw 'native conversion error realm';
if(Object.getPrototypeOf([])!==mainPrototype||realmSecret!==91)throw 'main realm after native error';

var marker={},same=false,lateCalls=0;
try{borrowed.call('prefix',{toString:function(){
    if(Object.getPrototypeOf([])!==mainPrototype)throw 'throwing callback realm';
    throw marker;
}},{toString:function(){lateCalls++;return 'late';}});}
catch(error){same=error===marker;}
if(!same||lateCalls!==0)throw 'explicit throw identity or later conversion';
if(Array!==mainArray||Object.getPrototypeOf([])!==mainPrototype||realmSecret!==91)throw 'main realm after callback error';
'ok';
"#, &mut NoHost).unwrap();
        assert_eq!(runtime.active_realm, None);
        assert_eq!(runtime.global_environment, 0);
        assert!(runtime.parked_main_roots.is_none());
        assert_eq!(runtime.native_access_caller, None);
        assert_eq!(runtime.transient_roots.inhibited, 0);
        assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
        assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
    }
}
