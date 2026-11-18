//! Reflect lookup and assignment keep the target and receiver distinct.
use super::*;

#[cfg(test)]
#[path = "reflect_set_tests.rs"]
mod set_tests;
#[cfg(test)]
#[path = "reflect_get_tests.rs"]
mod tests;

impl Runtime {
    pub(super) fn initialize_reflect(&mut self) -> Result<(), Fault> {
        let object = self.new_object(None)?;
        self.prototype_method(object, "get", Builtin::ReflectGet)?;
        self.prototype_method(object, "set", Builtin::ReflectSet)?;
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

    // These families have virtual keys, aliasing or specialized definitions.
    // Their existing assignment routines can silently refuse in sloppy mode.
    pub(super) fn uses_stored_property_descriptors(&self, id: usize) -> bool {
        let object = &self.objects[id];
        object.length.is_none()
            && object.arguments.is_none()
            && object.regexp.is_none()
            && !matches!(
                &object.primitive,
                Some(primitive_wrappers::Data::String { .. })
            )
            && !matches!(&object.binary, Some(binary_data::Data::Typed(_)))
    }

    // OrdinarySetWithOwnDescriptor's data case. super already owns its value;
    // Reflect copies a borrowed argument only after the receiver accepts it.
    pub(super) fn set_stored_receiver_data(
        &mut self,
        id: usize,
        key: PropertyKey,
        take_value: impl FnOnce(&mut Self) -> Result<Value, Fault>,
    ) -> Result<bool, Fault> {
        match self.stored_descriptor_property(id, &key) {
            Some(property)
                if property.accessor.is_some()
                    || property.getter.is_some()
                    || !property.writable =>
            {
                return Ok(false);
            }
            None if !self.objects[id].extensible => return Ok(false),
            _ => {}
        }
        let value = take_value(self)?;
        match key {
            PropertyKey::String(name) => self.store_property(id, name, value, None)?,
            PropertyKey::Symbol(key) => self.store_symbol_property(id, key, value, None)?,
        }
        Ok(true)
    }

    #[inline(never)]
    pub(super) fn reflect_set(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let target = args.first().unwrap_or(&Value::Undefined);
        let base = Ordinary::from_value(target);
        if let Some(base) = base {
            self.ordinary_id(base)?;
        } else if let Value::Host(handle) = target {
            self.validate_native_access(*handle, host)?;
        } else {
            return Err(Fault::exception(
                "TypeError",
                "Reflect.set requires an object target.",
            ));
        }
        let key = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
        let key = self.property_key(key, host)?;
        let Some(mut current) = base else {
            self.reject_foreign_window(target, host)?;
            return Err(Fault::new(
                "Reflect.set on a native target is not implemented.",
            ));
        };
        let value = args.get(2).unwrap_or(&Value::Undefined);
        let receiver = args.get(3).unwrap_or(target);
        self.reject_foreign_window(receiver, host)?;
        loop {
            self.step()?;
            let id = self.ordinary_id(current)?;
            if !self.uses_stored_property_descriptors(id) {
                return Err(Fault::new(
                    "Reflect.set on this exotic prototype or target is not implemented.",
                ));
            }
            if let Some(property) = self.stored_descriptor_property(id, &key) {
                if let Some(accessor) = property.accessor {
                    let Some(setter) = accessor.set else {
                        return Ok(Value::Bool(false));
                    };
                    let receiver = self.copy_value(receiver)?;
                    self.call_accessor(Some(setter), receiver, std::slice::from_ref(value), host)?;
                    return Ok(Value::Bool(true));
                }
                if property.getter.is_some() || !property.writable {
                    return Ok(Value::Bool(false));
                }
                break;
            }
            let Some(parent) = self.objects[id].prototype else {
                break;
            };
            current = parent;
        }
        if matches!(receiver, Value::Host(_)) {
            return Err(Fault::new(
                "Reflect.set cannot define data on a native receiver.",
            ));
        }
        let Some(receiver) = Ordinary::from_value(receiver) else {
            return Ok(Value::Bool(false));
        };
        let id = self.ordinary_id(receiver)?;
        if !self.uses_stored_property_descriptors(id) {
            return Err(Fault::new(
                "Reflect.set on this exotic data receiver is not implemented.",
            ));
        }
        Ok(Value::Bool(self.set_stored_receiver_data(
            id,
            key,
            |runtime| runtime.copy_value(value),
        )?))
    }
}
