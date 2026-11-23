//! Exact UTF-16 property names use the same ordinary descriptors, creation
//! order, reachability and quotas as scalar names. They never enter native
//! string-named operations or become array indices.
use super::*;

impl Runtime {
    pub(super) fn canonical_units_key(&mut self, units: Vec<u16>) -> Result<PropertyKey, Fault> {
        // Canonicalize a well-formed host string before applying the name
        // quota, so its scalar and code-unit forms have the same admission.
        let scalar_bytes = char::decode_utf16(units.iter().copied())
            .try_fold(0usize, |bytes, item| item.map(|ch| bytes.saturating_add(ch.len_utf8())));
        if let Ok(bytes) = scalar_bytes {
            if bytes > self.limits.capacity.property_bytes {
                return Err(Fault::new(format!("Property-name limit reached ({} bytes).", self.limits.capacity.property_bytes)));
            }
            self.work_as(bytes, WorkKind::PropertyName)?;
            let mut text = String::new();
            text.try_reserve_exact(bytes).map_err(|_| Fault::new("Cannot reserve scalar property name."))?;
            text.extend(char::decode_utf16(units).map(|item| item.expect("validated scalar property name")));
            Ok(PropertyKey::String(text))
        } else {
            self.property_name_units(&units)?;
            Ok(PropertyKey::StringUnits(units))
        }
    }

    pub(super) fn property_name_units(&mut self, units: &[u16]) -> Result<(), Fault> {
        let bytes = units.len().checked_mul(2)
            .ok_or_else(|| Fault::new("Property-name byte accounting overflow."))?;
        if bytes > self.limits.capacity.property_bytes {
            return Err(Fault::new(format!("Property-name limit reached ({} bytes).", self.limits.capacity.property_bytes)));
        }
        self.work_as(bytes, WorkKind::PropertyName)
    }

    pub(super) fn owns_units_property(&self, id: usize, units: &[u16]) -> Result<bool, Fault> {
        self.reject_unsupported_proxy_operation(id, "getOwnPropertyDescriptor")?;
        Ok(self.object(id)?.properties.get_units(units).is_some())
    }

    pub(super) fn units_dispatch_owner(
        &mut self, mut object: Ordinary, units: &[u16],
    ) -> Result<Option<usize>, Fault> {
        loop {
            self.step()?;
            let id = self.ordinary_id(object)?;
            let data = self.object(id)?;
            if data.proxy.is_some() || data.properties.get_units(units).is_some() {
                return Ok(Some(id));
            }
            let Some(parent) = data.prototype else { return Ok(None); };
            object = parent;
        }
    }

    pub(super) fn own_units_property(&mut self, id: usize, units: &[u16]) -> Result<Value, Fault> {
        self.step()?;
        self.property_name_units(units)?;
        let Some(property) = self.object(id)?.properties.get_units(units) else {
            return Ok(Value::Undefined);
        };
        if property.accessor.is_some() || property.getter.is_some() {
            return Err(Fault::new("An accessor read requires an execution context."));
        }
        self.work_as(property.value.bytes(), WorkKind::ValueCopy)?;
        Ok(self.objects[id].properties.get_units(units).unwrap().value.as_value()?.clone())
    }

    pub(super) fn get_units_property_with_receiver(
        &mut self, object: Ordinary, units: &[u16], receiver: &Value, host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.property_name_units(units)?;
        self.reject_foreign_window(receiver, host)?;
        let Some(owner) = self.units_dispatch_owner(object, units)? else {
            return Ok(Value::Undefined);
        };
        if self.objects[owner].proxy.is_some() {
            self.work_as(units.len().saturating_mul(2), WorkKind::ValueCopy)?;
            return self.proxy_get(owner, PropertyKey::StringUnits(units.to_vec()), receiver, host);
        }
        let property = self.objects[owner].properties.get_units(units).unwrap();
        if let Some(accessor) = property.accessor {
            let receiver = if accessor.get.is_some() { self.copy_value(receiver)? } else { Value::Undefined };
            return self.call_accessor(accessor.get, receiver, &[], host);
        }
        self.own_units_property(owner, units)
    }

    pub(super) fn units_lookup_base(&mut self, value: &Value, host: &dyn Host) -> Result<Option<Ordinary>, Fault> {
        // The existing opaque-key backing helper revalidates native ownership
        // and never converts a key into a Host method or attribute name.
        if let Some(object) = self.symbol_backing(value, host)? { return Ok(Some(object)); }
        Ok(match value {
            Value::String(_) | Value::StringUnits(_) => self.string_prototype,
            Value::Number(_) => self.number_prototype,
            Value::BigInt(_) => self.big_int_prototype,
            Value::Bool(_) => self.boolean_prototype,
            Value::Symbol(_) => self.symbol_prototype,
            _ => None,
        }.map(Ordinary::Object))
    }

    pub(super) fn read_units_property(
        &mut self, receiver: &Value, units: &[u16], host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        Self::require_property_receiver(receiver)?;
        self.property_name_units(units)?;
        let Some(base) = self.units_lookup_base(receiver, host)? else {
            return Err(Fault::new("UTF-16 properties require an ordinary object, supported primitive or registered native expando store."));
        };
        self.get_units_property_with_receiver(base, units, receiver, host)
    }

    pub(super) fn assign_units_property(
        &mut self, receiver: Value, units: Vec<u16>, value: Value, host: &mut dyn Host,
    ) -> Result<(), Fault> {
        Self::require_property_receiver(&receiver)?;
        self.property_name_units(&units)?;
        let backing = self.symbol_backing(&receiver, host)?;
        let Some(base) = self.units_lookup_base(&receiver, host)? else {
            return Err(Fault::new("UTF-16 properties require an ordinary object, supported primitive or registered native expando store."));
        };
        if let Some(owner) = self.units_dispatch_owner(base, &units)? {
            if self.objects[owner].proxy.is_some() {
                return self.assign_via_proxy(owner, PropertyKey::StringUnits(units), &value, &receiver, host);
            }
            let property = self.objects[owner].properties.get_units(&units).unwrap();
            if let Some(accessor) = property.accessor {
                if accessor.set.is_some() {
                    self.call_accessor(accessor.set, receiver, &[value], host)?;
                    return Ok(());
                }
                return if self.strict { Err(Fault::exception("TypeError", "The property has no setter.")) } else { Ok(()) };
            }
            if property.getter.is_some() || !property.writable {
                return if self.strict { Err(Fault::exception("TypeError", "The property is read-only.")) } else { Ok(()) };
            }
        }
        if let Some(backing) = backing {
            let id = self.ordinary_id(backing)?;
            self.store_units_property(id, units, value, None)
        } else if matches!(receiver, Value::Host(_)) {
            Err(Fault::new("Creating an own UTF-16 property on this native receiver is not implemented."))
        } else if self.strict {
            Err(Fault::exception("TypeError", "A primitive value cannot retain an assigned property."))
        } else { Ok(()) }
    }

    pub(super) fn store_units_property(
        &mut self, id: usize, units: Vec<u16>, value: Value, attributes: Option<(bool, bool, bool)>,
    ) -> Result<(), Fault> {
        self.store_units_property_kind(id, units, value, attributes, None)
    }

    pub(super) fn store_units_accessor_property(
        &mut self, id: usize, units: Vec<u16>, accessor: Accessors, attributes: (bool, bool),
    ) -> Result<(), Fault> {
        self.store_units_property_kind(id, units, Value::Undefined,
            Some((attributes.0, false, attributes.1)), Some(accessor))
    }

    fn store_units_property_kind(
        &mut self, id: usize, units: Vec<u16>, value: Value,
        attributes: Option<(bool, bool, bool)>, accessor: Option<Accessors>,
    ) -> Result<(), Fault> {
        self.step()?;
        self.property_name_units(&units)?;
        let object = self.object(id)?;
        if object.window_named_properties {
            return if self.strict || attributes.is_some() {
                Err(Fault::exception("TypeError", "Cannot define an own property on WindowProperties."))
            } else { Ok(()) };
        }
        let previous = object.properties.get_units(&units);
        if attributes.is_none() && previous.is_some_and(|property| !property.writable) {
            return if self.strict { Err(Fault::exception("TypeError", "The property is read-only.")) } else { Ok(()) };
        }
        if previous.is_none() && !object.extensible {
            return if self.strict || attributes.is_some() {
                Err(Fault::exception("TypeError", "The object is not extensible."))
            } else { Ok(()) };
        }
        let is_new = previous.is_none();
        if is_new && (object.properties.len() + object.symbol_properties.len() >= self.limits.heap.object_properties
            || self.properties >= self.limits.heap.properties) {
            return Err(self.property_limit_fault(object));
        }
        let (order, next_order) = self.property_creation_order(id, previous.map(|p| p.order))?;
        let enumerable = attributes.map_or_else(|| previous.is_none_or(|p| p.enumerable), |a| a.0);
        let writable = attributes.is_none_or(|a| a.1);
        let configurable = attributes.map_or_else(|| previous.is_none_or(|p| p.configurable), |a| a.2);
        let old_bytes = previous.map_or(0, |p| p.value.bytes());
        let new_bytes = value.bytes();
        if new_bytes > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        let key_bytes = if is_new { units.len().checked_mul(2) } else { Some(0) }
            .ok_or_else(|| Fault::new("Property-name byte accounting overflow."))?;
        let retained = self.value_bytes.checked_sub(old_bytes)
            .and_then(|n| n.checked_add(new_bytes)).and_then(|n| n.checked_add(key_bytes))
            .filter(|&n| n <= self.limits.heap.retained_values)
            .ok_or_else(|| Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)))?;
        self.work_as(new_bytes, WorkKind::PropertyStore)?;
        self.commit_property_creation_order(id, next_order);
        let object = &mut self.objects[id];
        object.properties.insert_units(units, Property {
            value: StoredValue::Value(value), getter: None, accessor, order,
            enumerable, writable, configurable,
        });
        object.next_order = next_order;
        self.value_bytes = retained;
        self.properties += usize::from(is_new);
        Ok(())
    }

    pub(super) fn remove_units_property(&mut self, id: usize, units: &[u16]) -> Result<(), Fault> {
        self.step()?;
        self.property_name_units(units)?;
        self.object(id)?;
        if let Some(property) = self.objects[id].properties.remove_units(units) {
            self.value_bytes -= units.len() * 2 + property.value.bytes();
            self.properties -= 1;
        }
        Ok(())
    }
}
