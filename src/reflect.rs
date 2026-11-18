//! Finite Reflect namespace. The lookup target and accessor receiver stay distinct.
//! ECMAScript 28.1.5: validate target, ToPropertyKey, then [[Get]] with receiver.
use super::*;

#[cfg(test)]
#[path = "reflect_get_tests.rs"]
mod tests;

impl Runtime {
    pub(super) fn initialize_reflect(&mut self) -> Result<(), Fault> {
        let object = self.new_object(None)?;
        self.prototype_method(object, "get", Builtin::ReflectGet)?;
        let tag = self
            .symbol_to_string_tag
            .expect("initialized well-known symbols");
        self.store_symbol_property(
            object,
            tag,
            Value::String("Reflect".into()),
            Some((false, false, true)),
        )?;
        self.put_binding(
            self.global_environment,
            "Reflect".into(),
            Some(Value::Object(object)),
            Declaration::Var,
            false,
        )?;
        Ok(())
    }

    #[inline(never)]
    pub(super) fn reflect_get(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let target = args.first().unwrap_or(&Value::Undefined);
        match target {
            Value::Object(id) => {
                self.object(*id)?;
            }
            Value::Function(id) => {
                self.ordinary_id(Ordinary::Function(*id))?;
            }
            Value::Host(handle) => self.validate_native_access(*handle, host)?,
            _ => {
                return Err(Fault::exception(
                    "TypeError",
                    "Reflect.get requires an object target.",
                ))
            }
        }
        let key = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
        let key = self.property_key(key, host)?;
        let receiver = args.get(2).unwrap_or(target);
        if let Value::Host(handle) = target {
            // Author key conversion can change a document's access state.
            self.validate_native_access(*handle, host)?;
            if !strict_equal(target, receiver) {
                // Opaque native slots do not yet expose descriptor/receiver
                // metadata. Never silently substitute target for receiver.
                return Err(Fault::new(
                    "Reflect.get with a different receiver on a native target is not implemented.",
                ));
            }
            return match key {
                PropertyKey::String(name) => self.read_property(target, &name, host),
                PropertyKey::Symbol(key) => {
                    self.get_symbol_property(Value::Host(*handle), key, host)
                }
            };
        }
        let object = Ordinary::from_value(target).expect("validated ordinary target");
        // A borrowed builtin's realm never grants access to its actual native
        // receiver. Preserve the same callback gate as ordinary super reads.
        self.reject_foreign_window(receiver, host)?;
        match key {
            PropertyKey::String(name) => {
                self.get_ordinary_property_with_receiver(object, &name, receiver, host)
            }
            PropertyKey::Symbol(key) => self.reflect_get_symbol(object, key, receiver, host),
        }
    }

    fn reflect_get_symbol(
        &mut self,
        object: Ordinary,
        key: usize,
        receiver: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let Some(owner) = self.symbol_property_owner(object, key)? else {
            return Ok(Value::Undefined);
        };
        let property = &self.objects[owner].symbol_properties[&key];
        if let Some(accessor) = property.accessor {
            let receiver = if accessor.get.is_some() {
                self.copy_value(receiver)?
            } else {
                Value::Undefined
            };
            return self.call_accessor(accessor.get, receiver, &[], host);
        }
        if let Some(getter) = property.getter {
            let function = property.value.as_value()?.clone();
            let receiver = self.copy_value(receiver)?;
            return self.native_getter_in_realm(&function, getter, receiver);
        }
        let bytes = property.value.bytes();
        self.work(bytes)?;
        Ok(self.objects[owner].symbol_properties[&key]
            .value
            .as_value()?
            .clone())
    }
}
