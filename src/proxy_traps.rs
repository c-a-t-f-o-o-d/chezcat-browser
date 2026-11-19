//! Bounded ordinary Proxy [[Get]] and [[Set]], with live trap lookup.
//! https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-get-p-receiver
//! https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots-set-p-v-receiver
use super::*;

impl Runtime {
    /// The constructor admits ordinary non-proxy target/handler identities.
    /// Keep all suspended inputs rooted through trap getters and callbacks.
    #[inline(never)]
    pub(super) fn with_proxy_operation<T>(
        &mut self,
        proxy: usize,
        receiver: &Value,
        value: Option<&Value>,
        action: impl FnOnce(&mut Self, usize, usize) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        self.step()?;
        let data = self.object(proxy)?.proxy.as_ref().ok_or_else(|| {
            Fault::exception("TypeError", "A genuine ordinary Proxy is required.")
        })?;
        let (target, handler) = (data.target, data.handler);
        self.object(target)?;
        self.object(handler)?;
        self.check_native_call_stack("Proxy operation call")?;
        let marker = self.root_marker();
        self.call_depth += 1;
        self.transient_roots.owned_call_depth += 1;
        let result = (|| {
            for id in [proxy, target, handler] {
                self.push_transient_value(&Value::Object(id))?;
            }
            self.push_transient_value(receiver)?;
            if let Some(value) = value {
                self.push_transient_value(value)?;
            }
            action(self, target, handler)
        })();
        self.restore_root_marker(marker);
        self.call_depth -= 1;
        self.transient_roots.owned_call_depth -= 1;
        result
    }

    pub(super) fn proxy_method(
        &mut self,
        handler: usize,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        let method = self.get_ordinary_property_with_receiver(
            Ordinary::Object(handler),
            name,
            &Value::Object(handler),
            host,
        )?;
        match method {
            Value::Undefined | Value::Null => Ok(None),
            Value::Function(_) => Ok(Some(method)),
            _ => Err(Fault::exception(
                "TypeError",
                format!("Proxy {name} trap is not callable."),
            )),
        }
    }

    pub(super) fn proxy_key_value(&mut self, key: &PropertyKey) -> Result<Value, Fault> {
        match key {
            PropertyKey::String(name) => {
                self.property_name(name)?;
                self.work_as(name.len(), WorkKind::ValueCopy)?;
                Ok(Value::String(name.clone()))
            }
            PropertyKey::Symbol(key) => {
                self.symbol(*key)?;
                Ok(Value::Symbol(*key))
            }
        }
    }

    // Frozen data values may be deferred canonical native methods. Realize the
    // actual value before SameValue, preserving flags and never calling a getter.
    fn proxy_materialize_descriptor(
        &mut self,
        target: usize,
        key: &PropertyKey,
    ) -> Result<(), Fault> {
        if let PropertyKey::String(name) = key {
            self.materialize_native_property(target, name)?;
        }
        Ok(())
    }

    pub(super) fn proxy_get(
        &mut self,
        proxy: usize,
        key: PropertyKey,
        receiver: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.with_proxy_operation(proxy, receiver, None, |runtime, target, handler| {
            runtime.reject_foreign_window(receiver, host)?;
            let Some(trap) = runtime.proxy_method(handler, "get", host)? else {
                return match key {
                    PropertyKey::String(name) => runtime.get_ordinary_property_with_receiver(
                        Ordinary::Object(target),
                        &name,
                        receiver,
                        host,
                    ),
                    PropertyKey::Symbol(key) => {
                        runtime.reflect_get_symbol(Ordinary::Object(target), key, receiver, host)
                    }
                };
            };
            runtime.push_transient_value(&trap)?;
            let args = [
                Value::Object(target),
                runtime.proxy_key_value(&key)?,
                runtime.copy_value(receiver)?,
            ];
            let result = runtime.call_value(trap, &args, Value::Object(handler), host)?;
            runtime.push_transient_value(&result)?;
            runtime.step()?;
            // The trap can add or freeze this property. Inspect the descriptor
            // after it returns; an earlier snapshot cannot prove an invariant.
            let property = runtime.stored_descriptor_property(target, &key);
            if property.is_some_and(|property| !property.configurable) {
                if let Some(accessor) = property.and_then(|property| property.accessor) {
                    if accessor.get.is_none() && !matches!(result, Value::Undefined) {
                        return Err(Fault::exception(
                            "TypeError",
                            "Proxy get violates a frozen accessor.",
                        ));
                    }
                } else if property
                    .is_some_and(|property| property.getter.is_none() && !property.writable)
                {
                    runtime.proxy_materialize_descriptor(target, &key)?;
                    let bytes = runtime
                        .stored_descriptor_property(target, &key)
                        .unwrap()
                        .value
                        .bytes();
                    runtime.work(bytes.saturating_add(result.bytes()))?;
                    let expected = runtime
                        .stored_descriptor_property(target, &key)
                        .unwrap()
                        .value
                        .as_value()?;
                    if !prototypes::same_value(expected, &result) {
                        return Err(Fault::exception(
                            "TypeError",
                            "Proxy get violates a frozen data property.",
                        ));
                    }
                }
            }
            Ok(result)
        })
    }

    pub(super) fn proxy_set(
        &mut self,
        proxy: usize,
        key: PropertyKey,
        value: &Value,
        receiver: &Value,
        host: &mut dyn Host,
    ) -> Result<bool, Fault> {
        self.with_proxy_operation(proxy, receiver, Some(value), |runtime, target, handler| {
            runtime.reject_foreign_window(receiver, host)?;
            let Some(trap) = runtime.proxy_method(handler, "set", host)? else {
                return runtime.reflect_set_key(
                    Ordinary::Object(target),
                    key,
                    value,
                    receiver,
                    host,
                );
            };
            runtime.push_transient_value(&trap)?;
            let args = [
                Value::Object(target),
                runtime.proxy_key_value(&key)?,
                runtime.copy_value(value)?,
                runtime.copy_value(receiver)?,
            ];
            let result = runtime.call_value(trap, &args, Value::Object(handler), host)?;
            if !result.truthy() {
                return Ok(false);
            }
            runtime.step()?;
            let property = runtime.stored_descriptor_property(target, &key);
            if property.is_some_and(|property| !property.configurable) {
                if property.is_some_and(|property| {
                    property.getter.is_some()
                        || property
                            .accessor
                            .is_some_and(|accessor| accessor.set.is_none())
                }) {
                    return Err(Fault::exception(
                        "TypeError",
                        "Proxy set violates a frozen accessor.",
                    ));
                }
                if property.is_some_and(|property| {
                    property.accessor.is_none() && property.getter.is_none() && !property.writable
                }) {
                    runtime.proxy_materialize_descriptor(target, &key)?;
                    let bytes = runtime
                        .stored_descriptor_property(target, &key)
                        .unwrap()
                        .value
                        .bytes();
                    runtime.work(bytes.saturating_add(value.bytes()))?;
                    let expected = runtime
                        .stored_descriptor_property(target, &key)
                        .unwrap()
                        .value
                        .as_value()?;
                    if !prototypes::same_value(expected, value) {
                        return Err(Fault::exception(
                            "TypeError",
                            "Proxy set violates a frozen data property.",
                        ));
                    }
                }
            }
            Ok(true)
        })
    }

    /// OrdinarySetWithOwnDescriptor delegates these two internal operations to
    /// the receiver. Their nullish traps forward, preserving a distinct Proxy
    /// receiver's target rather than writing either wrapper or original target.
    pub(super) fn proxy_receiver_set_data(
        &mut self,
        proxy: usize,
        key: PropertyKey,
        value: &Value,
        host: &mut dyn Host,
    ) -> Result<bool, Fault> {
        self.with_proxy_operation(
            proxy,
            &Value::Object(proxy),
            Some(value),
            |runtime, target, handler| {
                runtime.proxy_descriptor_forward(handler, "getOwnPropertyDescriptor", host)?;
                let previous = runtime.stored_descriptor_property(target, &key);
                if previous.is_some_and(|property| {
                    property.accessor.is_some() || property.getter.is_some() || !property.writable
                }) {
                    return Ok(false);
                }
                let create = previous.is_none();
                runtime.proxy_descriptor_forward(handler, "defineProperty", host)?;
                runtime.proxy_define_forwarded_data(target, key, value, create)
            },
        )
    }

    pub(super) fn proxy_descriptor_forward(
        &mut self,
        handler: usize,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if self.proxy_method(handler, name, host)?.is_some() {
            return Err(Fault::new(format!(
                "Proxy {name} trap is not implemented."
            )));
        }
        Ok(())
    }

    // DefineOwnProperty for the finite forwarded data descriptor. The trap
    // lookup above can change the target descriptor, so validate it again here.
    fn proxy_define_forwarded_data(
        &mut self,
        target: usize,
        key: PropertyKey,
        value: &Value,
        create: bool,
    ) -> Result<bool, Fault> {
        self.step()?;
        let previous = self.stored_descriptor_property(target, &key);
        if previous.is_none() && !self.objects[target].extensible {
            return Ok(false);
        }
        let attrs = previous.map(|property| {
            (
                property.enumerable,
                property.writable,
                property.configurable,
            )
        });
        let accessor = previous
            .is_some_and(|property| property.accessor.is_some() || property.getter.is_some());
        if let Some((_, writable, false)) = attrs {
            if create || accessor {
                return Ok(false);
            }
            if !writable {
                self.proxy_materialize_descriptor(target, &key)?;
                let bytes = self
                    .stored_descriptor_property(target, &key)
                    .unwrap()
                    .value
                    .bytes();
                self.work(bytes.saturating_add(value.bytes()))?;
                let previous = self
                    .stored_descriptor_property(target, &key)
                    .unwrap()
                    .value
                    .as_value()?;
                if !prototypes::same_value(previous, value) {
                    return Ok(false);
                }
            }
        }
        let attrs = if create {
            (true, true, true)
        } else {
            attrs
                .map(|(enumerable, writable, configurable)| {
                    (enumerable, !accessor && writable, configurable)
                })
                .unwrap_or((false, false, false))
        };
        let value = self.copy_value(value)?;
        match key {
            PropertyKey::String(name) => self.store_property(target, name, value, Some(attrs))?,
            PropertyKey::Symbol(key) => {
                self.store_symbol_property(target, key, value, Some(attrs))?
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "proxy_traps_tests.rs"]
mod tests;
