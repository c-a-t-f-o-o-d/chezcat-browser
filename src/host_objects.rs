//! Bounded ordinary expandos on explicitly selected native objects and live
//! named-property reflection. Host handles never acquire arbitrary capability.

use super::{Fault, Function, Host, Ordinary, Runtime, Value};

impl Runtime {
    pub fn bind_host_object(
        &mut self,
        handle: u32,
        methods: &[(&str, Value)],
    ) -> Result<(), String> {
        for (_, value) in methods { self.pin_exported_value(value); }
        if self.host_objects.contains_key(&handle) {
            return Err("Native object already has an expando store.".into());
        }
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            let prototype = self.new_object(None)?;
            for (name, value) in methods {
                self.set_property(prototype, (*name).into(), value.clone())?;
            }
            let object = self.new_object(None)?;
            self.objects[object].prototype = Some(Ordinary::Object(prototype));
            self.host_objects.insert(handle, object);
            Ok(())
        })();
        self.restore_transient_marker(marker);
        result.map_err(Fault::display)
    }

    pub fn define_host_constructor(
        &mut self,
        name: &str,
        object: u32,
        method: &'static str,
    ) -> Result<Value, String> {
        let value = self.define_host_function(name, object, method)?;
        if let Some(Function::Host { constructible, .. }) = self.functions.last_mut() {
            *constructible = true;
        }
        let Value::Function(index) = value else {
            unreachable!();
        };
        let prototype = self.new_object(None).map_err(Fault::display)?;
        self.store_property(
            prototype,
            "constructor".into(),
            Value::Function(index),
            Some((false, true, true)),
        )
        .map_err(Fault::display)?;
        self.store_property(
            self.function_properties[index],
            "prototype".into(),
            Value::Object(prototype),
            Some((false, false, false)),
        )
        .map_err(Fault::display)?;
        Ok(value)
    }

    pub(super) fn host_ordinary(
        &self,
        handle: u32,
        name: &str,
        host: &dyn Host,
    ) -> Option<Ordinary> {
        (!host.owns_property(handle, name))
            .then(|| self.host_objects.get(&handle).copied())
            .flatten()
            .map(Ordinary::Object)
    }

    pub(super) fn host_read(
        &mut self,
        handle: u32,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.foreign_window(handle, host) {
            return self.read_foreign_window(handle, name, host);
        }
        if let Some(object) = self.host_ordinary(handle, name, host) {
            return self.get_ordinary_property_with_receiver(
                object,
                name,
                &Value::Host(handle),
                host,
            );
        }
        if let Some(method) = host.method(handle, name) {
            if let Some(value) = self.host_method_values.get(&(handle, method)).cloned() {
                return self.copy_value(&value);
            }
            let value = self.host_method(handle, method).map_err(Fault::new)?;
            self.host_method_values
                .insert((handle, method), value.clone());
            return Ok(value);
        }
        let value = host.get(handle, name).map_err(Fault::new)?;
        if matches!(value, Value::Undefined)
            && host.ordinary_prototype(handle)
            && !host.has(handle, name).map_err(Fault::new)?
        {
            return self.get_ordinary_property_with_receiver(
                Ordinary::Object(self.object_prototype.unwrap()),
                name,
                &Value::Host(handle),
                host,
            );
        }
        self.checked_value(value)
    }

    pub(super) fn host_write(
        &mut self,
        handle: u32,
        name: String,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        if let Some(object) = self.host_ordinary(handle, &name, host) {
            return self.assign_ordinary_property_with_receiver(
                object,
                name,
                value,
                Value::Host(handle),
                host,
            );
        }
        if host.readonly_property(handle, &name) {
            return if self.strict {
                Err(Fault::exception(
                    "TypeError",
                    format!("Native property {name:?} is read-only."),
                ))
            } else {
                Ok(())
            };
        }
        let value = if host.string_setter(handle, &name) {
            let primitive = self.primitive_with_hint(value, true, host)?;
            self.work(primitive.bytes())?;
            self.checked_value(Value::String(primitive.script_text()?))?
        } else if host.number_setter(handle, &name) {
            let primitive = self.primitive_with_hint(value, false, host)?;
            self.work(primitive.bytes())?;
            Value::Number(primitive.script_number()?)
        } else {
            self.checked_value(value)?
        };
        self.pin_exported_value(&value);
        host.validate_set(handle, &name, &value)
            .map_err(|(kind, message)| Fault::exception(kind, message))?;
        host.set(handle, &name, value).map_err(Fault::new)
    }

    pub(super) fn host_keys(
        &mut self,
        handle: u32,
        inherited: bool,
        host: &mut dyn Host,
    ) -> Result<Vec<String>, Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        let keys = if let Some(&id) = self.host_objects.get(&handle) {
            if inherited {
                self.inherited_keys(Ordinary::Object(id))?
            } else {
                self.enumerable_keys(id)?
            }
        } else {
            host.own_keys(handle).map_err(Fault::new)?
        };
        if keys.len() > self.limits.heap.object_properties {
            return Err(Fault::new(
                format!("Property enumeration limit reached ({} keys).", self.limits.heap.object_properties),
            ));
        }
        for key in &keys {
            self.property_name(key)?;
        }
        Ok(keys)
    }

    pub(super) fn host_owned(
        &mut self,
        handle: u32,
        name: &str,
        enumerable: bool,
        host: &mut dyn Host,
    ) -> Result<bool, Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        if self.is_browser_global(handle) && self.is_operation_global(name) {
            let (_, id) = self.browser_global.unwrap();
            return if enumerable {
                self.own_enumerable(id, name)
            } else {
                self.owns_property(id, name)
            };
        }
        if let Some(object) = self.host_ordinary(handle, name, host) {
            let id = self.ordinary_id(object)?;
            return if enumerable {
                self.own_enumerable(id, name)
            } else {
                self.owns_property(id, name)
            };
        }
        Ok(host.own_data(handle, name).map_err(Fault::new)?.is_some())
    }

    pub(super) fn host_descriptor(
        &mut self,
        handle: u32,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.reject_foreign_window(&Value::Host(handle), host)?;
        if let Some(object) = self.host_ordinary(handle, name, host) {
            return self.own_descriptor(object, name);
        }
        let Some(value) = host.own_data(handle, name).map_err(Fault::new)? else {
            return Ok(Value::Undefined);
        };
        let id = self.new_object(None)?;
        for (key, value) in [
            ("value", value),
            (
                "writable",
                Value::Bool(!host.readonly_property(handle, name)),
            ),
            ("enumerable", Value::Bool(true)),
            ("configurable", Value::Bool(true)),
        ] {
            self.set_property(id, key.into(), value)?;
        }
        Ok(Value::Object(id))
    }
}
