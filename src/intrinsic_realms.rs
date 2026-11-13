//! Bounded allocation of independent intrinsic roots and detached global scopes.
//!
//! This is a prerequisite for real frames, not an exposed iframe API. Child
//! function execution/Window routing must be implemented before publishing these
//! roots to page code. All allocation/work/deadline accounting stays shared.
use super::*;

const MAX_INTRINSIC_REALMS: usize = 8;

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
    date_prototype: Option<usize>,
    url_roots: url_api::Roots,
    string_prototype: Option<usize>,
    number_prototype: Option<usize>,
    boolean_prototype: Option<usize>,
    error_prototypes: BTreeMap<&'static str, usize>,
    browser_global: Option<(u32, usize)>,
    global_attributes: BTreeMap<String, (bool, bool, bool)>,
    host_globals: HashSet<String>,
    operation_globals: HashSet<&'static str>,
    arguments_thrower: Option<usize>,
}

#[allow(dead_code)] // Not exposed until callee-realm/Window routing is complete.
pub(super) struct IntrinsicRealm {
    pub(super) global_environment: usize,
    roots: Roots,
    promise_roots: (Option<usize>, Option<usize>),
    function_range: std::ops::Range<usize>,
}

impl Runtime {
    pub(super) fn immutable_object_prototype(&self, id: usize) -> bool {
        self.object_prototype == Some(id)
            || self.intrinsic_realms.iter().any(|realm| realm.roots.object_prototype == Some(id))
    }

    #[allow(dead_code)]
    pub(super) fn allocate_intrinsic_realm(&mut self) -> Result<usize, Fault> {
        if self.intrinsic_realms.len() >= MAX_INTRINSIC_REALMS {
            return Err(Fault::new("Intrinsic realm limit reached (8 detached realms)."));
        }
        self.step()?;
        // Reserve the global environment first. Bootstrap cannot run page/host
        // code, create script closures or allocate lexical environments. Slot 0
        // is borrowed only during this synchronous intrinsic bootstrap, then
        // its exact original Environment is restored before any return.
        let global_environment = self.new_environment(0, true)?;
        let mut empty = self.environments[global_environment].take().unwrap();
        empty.parent = None;
        empty.captured = true;
        let parent_environment = self.environments[0].replace(empty);
        let parent_roots = Roots::take(self);
        let parent_promises = (self.promises.prototype.take(), self.promises.constructor.take());
        let start = self.functions.len();
        let result = self.initialize_realm_intrinsics();
        let roots = Roots::take(self);
        let promise_roots = (self.promises.prototype.take(), self.promises.constructor.take());
        let child_environment = std::mem::replace(&mut self.environments[0], parent_environment);
        self.environments[global_environment] = child_environment;
        parent_roots.restore(self);
        self.promises.prototype = parent_promises.0;
        self.promises.constructor = parent_promises.1;
        if let Err(error) = result {
            // Failed work is not refunded. Unreachable partial arena objects
            // remain bounded by the original aggregate caps; no roots/handles
            // are published. Drop the unexposed global's binding storage.
            self.discard_environment(global_environment);
            return Err(error);
        }
        let id = self.intrinsic_realms.len();
        self.intrinsic_realms.push(IntrinsicRealm {
            global_environment, roots, promise_roots,
            function_range: start..self.functions.len(),
        });
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected host get".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected host set".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected host call".into()) }
    }

    fn constructor(runtime: &Runtime, global: usize, name: &str) -> usize {
        let Some(Value::Function(index)) = runtime.environment(global).bindings[name].value else { panic!("constructor") };
        index
    }

    #[test]
    fn detached_intrinsics_are_real_distinct_and_do_not_inherit_parent_mutation() {
        let mut runtime = Runtime::new();
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
        let mut runtime = Runtime::new();
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
        let mut runtime = Runtime::new();
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
            let mut runtime = Runtime::new();
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
}
