//! Active Window key snapshots use the actual realm's bindings/property store.
//! Other Window operations retain their existing foreign-context restrictions.
use super::{Fault, Host, Ordinary, Runtime, StringKey, WorkKind};
use std::collections::BTreeSet;

impl Runtime {
    #[cfg(test)]
    pub(crate) fn window_allocation_state_for_test(&self) -> (usize, usize, usize, bool) {
        (self.functions.len(), self.objects.len(), self.bindings, self.transient_registration_is_empty())
    }

    pub(super) fn reserve_deferred_global_orders(&mut self, names: &'static [&'static str]) -> Result<(), Fault> {
        let next = self.environment(self.global_environment).next_order;
        let end = next.checked_add(names.len()).ok_or_else(|| Fault::new("Deferred global creation order overflow."))?;
        self.work_as(names.iter().map(|name| name.len()).sum(), WorkKind::PropertyEnumeration)?;
        for (offset, &name) in names.iter().enumerate() {
            self.deferred_global_orders.insert(name, next + offset);
        }
        self.environment_mut(self.global_environment).next_order = end;
        Ok(())
    }

    pub(super) fn property_creation_order(&self, id: usize, old: Option<usize>) -> Result<(usize, usize), Fault> {
        let next = if self.browser_global.is_some_and(|(_, properties)| properties == id) {
            self.environment(self.global_environment).next_order
        } else { self.object(id)?.next_order };
        Ok((old.unwrap_or(next), next.checked_add(usize::from(old.is_none()))
            .ok_or_else(|| Fault::new("Object property order limit reached."))?))
    }

    pub(super) fn commit_property_creation_order(&mut self, id: usize, next: usize) {
        if self.browser_global.is_some_and(|(_, properties)| properties == id) {
            self.environment_mut(self.global_environment).next_order = next;
        }
    }

    pub(super) fn window_own_enumerable(&mut self, name: &str, host: &mut dyn Host) -> Result<Option<bool>, Fault> {
        let (handle, properties) = self.browser_global.expect("active Window");
        let native = host.window_properties(handle).map_err(Fault::new)?;
        self.ensure_global_intrinsics(name)?;
        self.step()?;
        self.work_as(name.len(), WorkKind::PropertyEnumeration)?;
        if let Some((_, enumerable)) = native.iter().find(|(key, _)| *key == name) {
            return Ok(Some(*enumerable));
        }
        if self.global_binding(name) {
            return Ok(Some(self.global_attributes.get(name).is_none_or(|attributes| attributes.0)));
        }
        Ok(self.objects[properties].properties.get(name).map(|property| property.enumerable))
    }

    pub(super) fn window_enumerable(&mut self, name: &str, host: &mut dyn Host) -> Result<bool, Fault> {
        if let Some(enumerable) = self.window_own_enumerable(name, host)? { return Ok(enumerable); }
        let (_, properties) = self.browser_global.unwrap();
        match self.property_owner(Ordinary::Object(properties), name)? {
            Some(owner) => self.own_enumerable(owner, name),
            None => Ok(false),
        }
    }

    pub(super) fn window_keys(&mut self, inherited: bool, enumerable_only: bool, host: &mut dyn Host) -> Result<Vec<StringKey>, Fault> {
        let (handle, properties) = self.browser_global.expect("active Window");
        let native = host.window_properties(handle).map_err(Fault::new)?;
        // Deferred families are real nonenumerable own globals. Materialize
        // before snapshotting; deleted initialized names are never recreated.
        for family in ["Uint8Array", "URL", "TextEncoder", "DOMRect", "ImageData"] { self.ensure_global_intrinsics(family)?; }
        self.ensure_param_constructor()?;
        self.ensure_datalist_constructor()?;
        let environment = self.environment(self.global_environment);
        let raw_count = environment.bindings.len().checked_add(self.objects[properties].properties.len())
            .and_then(|n| n.checked_add(native.len())).ok_or_else(|| Fault::new("Window key accounting overflow."))?;
        let bytes = environment.bindings.keys()
            .fold(0usize, |total, name| total.saturating_add(name.len()))
            .saturating_add(self.objects[properties].properties.keys()
                .fold(0usize, |total, name| total.saturating_add(name.byte_len())))
            .saturating_add(native.iter().map(|(name, _)| name.len()).sum::<usize>());
        self.work_as(bytes.saturating_mul(2), WorkKind::PropertyEnumeration)?;
        for _ in 0..raw_count { self.step()?; }
        let mut entries: Vec<(StringKey, (usize, usize), bool)> = Vec::new();
        entries.try_reserve(raw_count).map_err(|_| Fault::new("Unable to allocate Window key snapshot."))?;
        for (order, &(name, enumerable)) in native.iter().enumerate() {
            entries.push((name.into(), (0, order), enumerable));
        }
        for (name, binding) in &self.environment(self.global_environment).bindings {
            if self.global_binding(name) {
                entries.push((name.clone().into(), (1, binding.order), self.global_attributes.get(name).is_none_or(|attributes| attributes.0)));
            }
        }
        for (name, property) in &self.objects[properties].properties {
            entries.push((name.to_owned(), (1, property.order), property.enumerable));
        }
        entries.sort_by(|left, right| match (left.0.index(), right.0.index()) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => left.1.cmp(&right.1),
        });
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        for (name, _, enumerable) in entries {
            if !seen.insert(name.clone()) { continue; }
            if seen.len() > self.limits.heap.object_properties {
                return Err(Fault::new("Window property enumeration limit reached."));
            }
            if !enumerable_only || enumerable { result.push(name); }
        }
        if inherited {
            for name in self.inherited_keys(Ordinary::Object(properties))? {
                if seen.insert(name.clone()) { result.push(name); }
                if seen.len() > self.limits.heap.object_properties {
                    return Err(Fault::new("Window property enumeration limit reached."));
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Value, limits};
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("unexpected native getter".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("unexpected native setter".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("unexpected native call".into()) }
    }
    fn runtime() -> Runtime {
        let mut runtime = Runtime::with_limits(limits::BROWSER);
        runtime.define_global("window", Value::Host(17)).unwrap();
        runtime.bind_browser_global(17).unwrap();
        runtime
    }

    #[test]
    fn window_keys_share_creation_order_across_bindings_properties_and_migration() {
        let mut runtime = runtime();
        for source in [
            "var key_a=1;let key_lexical=9;const key_const=8;window.key_b=2;window.key_c=3;window[10]=10;window[2]=2;",
            "var key_b;function key_c(){}window.key_d=4;window.key_e=5;delete window.key_d;window.key_d=6;",
            "var names=Object.keys(window);if(names[0]!=='2'||names[1]!=='10')throw 'integer order';if(names.filter(function(n){return n.indexOf('key_')===0;}).join(',')!=='key_a,key_b,key_c,key_e,key_d')throw 'creation order';var all=Object.getOwnPropertyNames(window);for(var n of ['undefined','NaN','Infinity','Uint8Array','URL','TextEncoder','DOMRectReadOnly','DOMRect']){if(all.indexOf(n)<0||all.indexOf(n)>all.indexOf('key_a')||names.indexOf(n)>=0)throw 'intrinsic metadata/order';}if(all.indexOf('key_lexical')>=0||all.indexOf('key_const')>=0)throw 'lexical leakage';",
        ] { runtime.begin_job(); runtime.execute(source, &mut NoHost).unwrap(); }
    }

    #[test]
    fn window_keys_recheck_presence_and_flags_and_hide_nonenumerable_own_shadows() {
        let mut runtime = runtime();
        runtime.execute(r#"
Object.prototype.key_inherited=1;Object.prototype.key_hidden=2;
Object.defineProperty(window,'key_hidden',{value:3,enumerable:false,configurable:true});
window.key_first=1;window.key_second=2;var seen=[];
for(var key in window){if(key==='key_first')delete window.key_second;if(key.indexOf('key_')===0)seen.push(key);}
if(seen.join(',')!=='key_first,key_inherited')throw 'for-in snapshot/metadata';
if(Object.keys(window).indexOf('key_hidden')>=0||Object.getOwnPropertyNames(window).indexOf('key_hidden')<0)throw 'all-own distinction';
if(!Object.hasOwn(window,'key_hidden')||window.propertyIsEnumerable('key_hidden'))throw 'own metadata';
delete Object.prototype.key_inherited;delete Object.prototype.key_hidden;
"#, &mut NoHost).unwrap();
    }

    #[test]
    fn window_keys_never_resurrect_deleted_operations_or_call_snapshot_getters() {
        let mut runtime = runtime();
        runtime.define_global_operation("setTimeout", 17, "setTimeout", 1).unwrap();
        runtime.execute(r#"
delete window.setTimeout;var calls=0;
Object.defineProperty(window,'key_getter',{enumerable:true,configurable:true,get:function(){calls++;return 17;}});
var keys=Object.keys(window),all=Object.getOwnPropertyNames(window);
if(calls||keys.indexOf('setTimeout')>=0||all.indexOf('setTimeout')>=0||Object.hasOwn(window,'setTimeout'))throw 'snapshot/deletion';
Object.defineProperty(window,'key_later',{value:18,enumerable:false,configurable:true});
Object.defineProperty(window,'key_getter',{get:function(){Object.defineProperty(window,'key_later',{enumerable:true});return 17;}});
var copied=Object.assign({},window);if(copied.key_later!==18)throw 'assign snapshots all own then rechecks';
"#, &mut NoHost).unwrap();
    }

    #[test]
    fn window_order_overflow_and_key_work_failure_leave_existing_state_unchanged() {
        let mut runtime = runtime();
        runtime.execute("var keep=7;window.saved=9;", &mut NoHost).unwrap();
        let (_, properties) = runtime.browser_global.unwrap();
        let old = (runtime.environment(runtime.global_environment).next_order, runtime.objects[properties].next_order);
        runtime.environment_mut(runtime.global_environment).next_order = usize::MAX;
        assert!(runtime.put_binding(runtime.global_environment, "overflow".into(), Some(Value::Number(1.0)), super::super::Declaration::Var, false).is_err());
        assert!(runtime.set_property(properties, "overflow".into(), Value::Number(1.0)).is_err());
        assert!(!runtime.objects[properties].properties.contains_key("overflow"));
        runtime.environment_mut(runtime.global_environment).next_order = old.0;
        assert_eq!(runtime.objects[properties].next_order, old.1);
        runtime.begin_job();runtime.copy_bytes=0;
        assert!(runtime.window_keys(false, true, &mut NoHost).is_err());
        runtime.begin_job();
        assert_eq!(runtime.execute("keep+window.saved;", &mut NoHost).unwrap().to_number(), 16.0);
    }
}
