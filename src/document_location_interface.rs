//! A Document's unforgeable location accessors share the defining realm's
//! functions. Each call resolves the actual receiver's current browsing context.
use super::*;

pub(crate) const GET: &str = "\0Document.get.location";
pub(crate) const SET: &str = "\0Document.set.location";
const STORAGE: &str = "\0Document.location own descriptor";

impl Runtime {
    pub(super) fn document_location_slot(
        &mut self,
        handle: u32,
        host: &mut dyn Host,
    ) -> Result<Option<Ordinary>, Fault> {
        let Some(owner) = host.document_location_owner(handle) else {
            return Ok(None);
        };
        self.validate_native_access(handle, host)?;
        let caller = self
            .native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(window, _)| window));
        host.document_location_access(caller, handle)
            .map_err(Fault::new)?;
        let realm = self.ensure_window_realm(owner, host)?;
        self.with_intrinsic_realm(realm, |runtime| {
            if let Some(&id) = runtime.native_constructor_prototypes.get(&(owner, STORAGE)) {
                return Ok(Some(Ordinary::Object(id)));
            }
            let id = runtime.new_object(None)?;
            if let Err(error) = runtime.define_host_attribute(id, owner, "location", GET, Some(SET))
            {
                debug_assert_eq!(runtime.objects.len(), id + 1);
                debug_assert!(runtime.objects[id].properties.is_empty());
                runtime.objects.pop();
                return Err(error);
            }
            runtime.objects[id]
                .properties
                .get_mut("location")
                .unwrap()
                .configurable = false;
            runtime
                .native_constructor_prototypes
                .insert((owner, STORAGE), id);
            Ok(Some(Ordinary::Object(id)))
        })
    }

    pub(super) fn document_own_string_keys(
        &mut self,
        handle: u32,
        enumerable_only: bool,
        host: &mut dyn Host,
    ) -> Result<Vec<String>, Fault> {
        self.validate_native_access(handle, host)?;
        let caller = self
            .native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(window, _)| window));
        host.document_location_access(caller, handle)
            .map_err(Fault::new)?;
        let mut keys = if let Some(&object) = self.host_objects.get(&handle) {
            self.own_string_keys(object, enumerable_only)?
        } else {
            Vec::new()
        };
        if keys.len() >= self.limits.heap.object_properties {
            return Err(Fault::new(
                "Document own key count exceeds the property allowance.",
            ));
        }
        self.work(keys.len().saturating_add("location".len()))?;
        let at = keys
            .iter()
            .take_while(|k| property_index(k).is_some())
            .count();
        keys.try_reserve(1)
            .map_err(|_| Fault::new("Cannot reserve Document location key."))?;
        keys.insert(at, "location".into());
        Ok(keys)
    }

    pub(super) fn document_location_call(
        &mut self,
        handle: u32,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let caller = self
            .native_access_caller
            .unwrap_or_else(|| self.browser_global.map(|(w, _)| w));
        let location = host
            .document_location_value(caller, handle)
            .map_err(Fault::new)?;
        if method == GET {
            return Ok(location);
        }
        let Value::Host(target) = location else {
            return Err(Fault::exception(
                "TypeError",
                "A Document without a browsing context has no Location to assign.",
            ));
        };
        let value = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        self.host_set_native(target, "href", value, host)?;
        Ok(Value::Undefined)
    }
}
