//! Private data-slot deferral for native method identities. No lazy getter or
//! placeholder is exposed through JavaScript Value, descriptors or reflection.
use super::*;


#[derive(Clone, Copy)]
enum State { Pending, Ready(usize), Failed }

pub(super) struct NativeMethod {
    builtin: Builtin,
    name: String,
    realm: Option<usize>,
    state: State,
}

impl Runtime {
    pub(super) fn deferred_native_method(&mut self, owner: usize, name: &str, builtin: Builtin) -> Result<(), Fault> {
        #[cfg(test)]
        if self.limits.test_entry_arenas && self.deferred_native_methods.len() >= self.limits.capacity.functions {
            return Err(Fault::new("Deferred native method metadata limit reached (1024)."));
        }
        let available = self.limits.heap.retained_values.checked_sub(self.value_bytes)
            .and_then(|bytes| bytes.checked_sub(name.len()))
            .ok_or_else(|| Fault::new("Retained native method storage exhausted."))?;
        let growth = super::limits::prepare_retained_growth(&self.deferred_native_methods, 1, available)?;
        let bytes = growth.added + name.len();
        self.work(bytes.saturating_add(if growth.added != 0 { self.deferred_native_methods.len() } else { 0 }))?;
        growth.commit(&mut self.deferred_native_methods);
        let index = self.deferred_native_methods.len();
        self.deferred_native_methods.push(NativeMethod { builtin, name: name.into(), realm: self.active_realm, state: State::Pending });
        self.value_bytes += bytes;
        // There are no callbacks between registration and replacing the private
        // slot. Failed registrations retain only bounded, charged metadata.
        self.store_property(owner, name.into(), Value::Undefined, Some((false, true, true)))?;
        self.objects[owner].properties.get_mut(name).unwrap().value = StoredValue::DeferredNative(index);
        Ok(())
    }

    pub(super) fn materialize_native_property(&mut self, owner: usize, name: &str) -> Result<(), Fault> {
        let Some(StoredValue::DeferredNative(index)) = self.object(owner)?.properties.get(name).map(|property| &property.value) else { return Ok(()); };
        let index = *index;
        let function = match self.deferred_native_methods[index].state {
            State::Ready(function) => function,
            State::Failed => return Err(Fault::new("Native method initialization is unavailable after an earlier bounded failure.")),
            State::Pending => {
                let method = &self.deferred_native_methods[index];
                let (builtin, realm, bytes) = (method.builtin, method.realm, method.name.len());
                self.work(bytes)?;
                let name = self.deferred_native_methods[index].name.clone();
                let result = self.with_intrinsic_realm(realm, |runtime| runtime.native_function(builtin, &name));
                match result {
                    Ok(Value::Function(function)) => {
                        self.deferred_native_methods[index].state = State::Ready(function);
                        function
                    }
                    Ok(_) => unreachable!(),
                    Err(error) => { self.deferred_native_methods[index].state = State::Failed; return Err(error); }
                }
            }
        };
        // Preserve current descriptor flags/order, including freeze before the
        // first read. The conceptual data value is unchanged by realization.
        self.objects[owner].properties.get_mut(name).unwrap().value = StoredValue::Value(Value::Function(function));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected native get".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected native set".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected native call".into()) }
    }

    #[test]
    fn first_method_observation_creates_one_stable_data_value() {
        let mut runtime = Runtime::for_capacity_test();
        println!("Demand-driven baseline: {} callable identities; {} bounded native method records.", runtime.functions.len(), runtime.deferred_native_methods.len());
        runtime.execute("var inspect=Object.getOwnPropertyDescriptor;var p=Array.prototype;", &mut NoHost).unwrap();
        let functions = runtime.functions.len();
        runtime.execute("var a=p.map,b=p.map,d=inspect(p,'map');if(a!==b||a!==d.value||d.get!==undefined||d.enumerable||!d.configurable||!d.writable||a.name!=='map'||a.length!==1)throw 'native data identity';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), functions + 1);
    }

    #[test]
    fn names_presence_freeze_and_flag_updates_do_not_create_callables() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var names=Object.getOwnPropertyNames,freeze=Object.freeze,inspect=Object.getOwnPropertyDescriptor,define=Object.defineProperty;var p=Array.prototype;", &mut NoHost).unwrap();
        let functions = runtime.functions.len();
        runtime.execute("if(!('map' in p)||names(p).indexOf('map')<0)throw 'method presence';define(p,'map',{writable:false});freeze(p);", &mut NoHost).unwrap();
        // indexOf is the only observed method during the presence check.
        assert_eq!(runtime.functions.len(), functions + 1);
        let functions = runtime.functions.len();
        runtime.execute("var method=p.map,d=inspect(p,'map');if(d.value!==method||d.writable||d.configurable||d.enumerable)throw 'frozen data';define(p,'map',{value:method});var blocked=false;try{define(p,'map',{value:0});}catch(e){blocked=e.name==='TypeError';}if(!blocked)throw 'nonconfigurable value';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), functions + 1);
    }

    #[test]
    fn replacement_and_deletion_before_observation_do_not_resurrect_methods() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var inspect=Object.getOwnPropertyDescriptor,define=Object.defineProperty;var p=Array.prototype;", &mut NoHost).unwrap();
        let functions = runtime.functions.len();
        runtime.execute("p.flat=17;if(p.flat!==17)throw 'replacement';delete p.flat;if(p.flat!==undefined||inspect(p,'flat')!==undefined)throw 'resurrection';define(p,'map',{get:undefined,configurable:true});if(p.map!==undefined)throw 'accessor replacement';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), functions);
    }

    #[test]
    fn aliases_preserve_identity_and_first_read_under_one_mib_is_bounded() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut runtime = Runtime::for_capacity_test();
            runtime.execute("if(Array.prototype.values!==Array.prototype[Symbol.iterator]||Set.prototype.keys!==Set.prototype.values||Date.prototype.toGMTString!==Date.prototype.toUTCString)throw 'native alias';function nested(n){if(n>0)return nested(n-1);return String.prototype.trim;}var a=nested(8);if(a!==String.prototype.trim||a.call(' x ')!=='x')throw 'late method';", &mut NoHost).unwrap();
        }).unwrap().join().unwrap();
    }

    #[test]
    fn failed_method_realization_never_exposes_a_placeholder_or_retries_allocation() {
        let mut runtime = Runtime::for_capacity_test();
        while runtime.functions.len() < MAX_FUNCTIONS { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.value_bytes, runtime.job_deadline);
        let error = runtime.execute("Array.prototype.map;", &mut NoHost).unwrap_err();
        assert!(error.contains("Function limit reached"), "{error}");
        assert_eq!(runtime.functions.len(), before.0);
        assert_eq!(runtime.objects.len(), before.1);
        assert_eq!(runtime.job_deadline, before.3);
        runtime.begin_job();
        let error = runtime.execute("Array.prototype.map;", &mut NoHost).unwrap_err();
        assert!(error.contains("earlier bounded failure"), "{error}");
        assert_eq!(runtime.functions.len(), before.0);
        assert_eq!(runtime.objects.len(), before.1);
        assert_eq!(runtime.execute("6*7;", &mut NoHost).unwrap().to_number(), 42.0);
        runtime.execute("Array.prototype.map=17;if(Array.prototype.map!==17)throw 'failed slot replacement';delete Array.prototype.map;if(Array.prototype.map!==undefined)throw 'failed slot resurrection';", &mut NoHost).unwrap();
        assert_eq!(runtime.functions.len(), before.0);
    }

    #[test]
    fn frozen_method_rejects_an_unrelated_value_without_needing_a_callable_slot() {
        let mut runtime = Runtime::for_capacity_test();
        runtime.execute("var freeze=Object.freeze,define=Object.defineProperty;freeze(Array.prototype);", &mut NoHost).unwrap();
        while runtime.functions.len() < MAX_FUNCTIONS { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
        let functions = runtime.functions.len();
        assert_eq!(runtime.execute("var caught=false;try{define(Array.prototype,'map',{value:7});}catch(e){caught=e.name==='TypeError';}caught;", &mut NoHost).unwrap().to_text(), "true");
        assert_eq!(runtime.functions.len(), functions);
    }

    #[test]
    fn observing_all_pending_methods_once_matches_the_eager_surface_without_repeated_allocations() {
        let mut runtime = Runtime::for_capacity_test();
        let before = runtime.functions.len();
        let pending: Vec<_> = runtime.objects.iter().enumerate().flat_map(|(owner, object)| {
            object.properties.iter().filter_map(move |(name, property)| matches!(property.value, StoredValue::DeferredNative(_)).then(|| (owner, name.clone())))
        }).collect();
        let mut identities = Vec::new();
        for (owner, name) in &pending {
            let Value::Function(function) = runtime.own_property(*owner, name).unwrap() else { panic!("real native callable") };
            identities.push(function);
        }
        let after = runtime.functions.len();
        assert_eq!(after, before + pending.len());
        for ((owner, name), expected) in pending.iter().zip(identities) {
            assert!(matches!(runtime.own_property(*owner, name).unwrap(), Value::Function(function) if function == expected));
        }
        assert_eq!(runtime.functions.len(), after);
        println!("Native surface allocation: bootstrap={before}; pending-data-methods={}; after-observing-all={after}; repeated-read-additions=0.", pending.len());
    }

    #[test]
    fn original_sparse_method_workload_allocates_only_observed_identities() {
        let mut runtime = Runtime::for_capacity_test();
        let owner = runtime.new_object(None).unwrap();
        let before = runtime.functions.len();
        let retained = runtime.value_bytes;
        for index in 0..64 { runtime.prototype_method(owner, &format!("original{index}"), Builtin::IsNaN).unwrap(); }
        assert_eq!(runtime.functions.len(), before);
        assert!(runtime.value_bytes >= retained + 64 * std::mem::size_of::<NativeMethod>());
        let mut ids = Vec::new();
        for index in 0..3 {
            let Value::Function(function) = runtime.own_property(owner, &format!("original{index}")).unwrap() else { panic!("callable") };
            ids.push(function);
        }
        assert!(ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2]);
        assert_eq!(runtime.functions.len(), before + 3);
        runtime.set_property(owner, "original3".into(), Value::Number(17.0)).unwrap();
        runtime.remove_property(owner, "original4").unwrap();
        assert_eq!(runtime.own_property(owner, "original3").unwrap().to_number(), 17.0);
        assert!(matches!(runtime.own_property(owner, "original4").unwrap(), Value::Undefined));
        assert_eq!(runtime.functions.len(), before + 3);
        println!("Original sparse workload: registered=64; observed=3; callable-additions=3; unread replacement/deletion additions=0.");
    }
}
