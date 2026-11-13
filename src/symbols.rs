//! Original bounded symbol primitives and property keys. Identities live for
//! one document, just like the existing object/function arenas. Descriptions
//! and registry keys share the retained-value and copy-work budgets.
//!
//! Symbol.dispose is a stable key; it does not run cleanup automatically.
//! Symbol.iterator supports the bounded native/custom iterator protocol. Other
//! unsupported well-known protocols are not advertised. Number/Boolean/Symbol boxing uses explicit
//! primitive slots; Symbol.prototype itself has no boxed Symbol value.

use super::{
    Accessors, Builtin, Fault, Host, NativeGetter, Ordinary, Property, Runtime, Value,
    MAX_STRING_BYTES,
};
#[cfg(test)]
use super::{MAX_OBJECT_PROPERTIES, MAX_PROPERTIES, MAX_RETAINED_VALUES};

const MAX_SYMBOLS: usize = 4096;

pub(super) struct Data {
    description: Option<String>,
    registered: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor,
    For,
    KeyFor,
    Description,
    ToString,
    ValueOf,
    ToPrimitive,
}

impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Constructor | Self::Description | Self::ToString | Self::ValueOf => 0,
            Self::For | Self::KeyFor | Self::ToPrimitive => 1,
        }
    }
}

impl Runtime {
    pub(super) fn symbol(&self, id: usize) -> Result<&Data, Fault> {
        self.symbols
            .get(id)
            .ok_or_else(|| Fault::new("Invalid symbol identity."))
    }

    fn new_symbol(
        &mut self,
        description: Option<String>,
        registered: bool,
    ) -> Result<Value, Fault> {
        self.step()?;
        if self.symbols.len() >= MAX_SYMBOLS {
            return Err(Fault::new(
                "Symbol identity limit reached (4096 per document).",
            ));
        }
        let bytes = description.as_ref().map_or(0, String::len);
        if bytes > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        // A registry entry owns a second copy of its description as a key.
        let retained = self
            .value_bytes
            .saturating_add(bytes * if registered { 2 } else { 1 });
        if retained > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work(bytes * if registered { 2 } else { 1 })?;
        let id = self.symbols.len();
        if registered {
            self.symbol_registry
                .insert(description.as_ref().unwrap().clone(), id);
        }
        self.symbols.push(Data {
            description,
            registered,
        });
        self.value_bytes = retained;
        Ok(Value::Symbol(id))
    }

    pub(super) fn initialize_symbols(&mut self) -> Result<(), Fault> {
        let prototype = self.new_object(None)?;
        self.symbol_prototype = Some(prototype);
        self.constructor_prototype("Symbol", Value::Object(prototype))?;
        self.prototype_method(prototype, "toString", Builtin::Symbol(Op::ToString))?;
        self.prototype_method(prototype, "valueOf", Builtin::Symbol(Op::ValueOf))?;
        self.native_accessor(
            prototype,
            "description",
            Builtin::Symbol(Op::Description),
            NativeGetter::SymbolDescription,
        )?;
        let constructor = self.binding_value(self.global_environment, "Symbol")?;
        let owner = self.ordinary_id(Ordinary::from_value(&constructor).unwrap())?;
        self.prototype_method(owner, "for", Builtin::Symbol(Op::For))?;
        self.prototype_method(owner, "keyFor", Builtin::Symbol(Op::KeyFor))?;
        let dispose = if let Some(key) = self.symbol_dispose { Value::Symbol(key) } else {
            let value = self.new_symbol(Some("Symbol.dispose".into()), false)?;
            let Value::Symbol(key) = value else { unreachable!() };
            self.symbol_dispose = Some(key);
            value
        };
        self.store_property(
            owner,
            "dispose".into(),
            dispose,
            Some((false, false, false)),
        )?;
        let key = if let Some(key) = self.symbol_to_primitive { key } else {
            let Value::Symbol(key) = self.new_symbol(Some("Symbol.toPrimitive".into()), false)? else { unreachable!() };
            key
        };
        self.symbol_to_primitive = Some(key);
        self.store_property(
            owner,
            "toPrimitive".into(),
            Value::Symbol(key),
            Some((false, false, false)),
        )?;
        let method =
            self.native_function(Builtin::Symbol(Op::ToPrimitive), "[Symbol.toPrimitive]")?;
        self.store_symbol_property(prototype, key, method, Some((false, false, true)))?;
        let key = if let Some(key) = self.symbol_iterator { key } else {
            let Value::Symbol(key) = self.new_symbol(Some("Symbol.iterator".into()), false)? else { unreachable!() };
            key
        };
        self.symbol_iterator = Some(key);
        self.store_property(
            owner,
            "iterator".into(),
            Value::Symbol(key),
            Some((false, false, false)),
        )?;
        let key = if let Some(key) = self.symbol_species { key } else {
            let Value::Symbol(key) = self.new_symbol(Some("Symbol.species".into()), false)? else { unreachable!() };
            key
        };
        self.symbol_species = Some(key);
        self.store_property(
            owner,
            "species".into(),
            Value::Symbol(key),
            Some((false, false, false)),
        )
    }

    fn symbol_receiver(&self, receiver: &Value) -> Result<usize, Fault> {
        let id = match receiver {
            Value::Symbol(id) => Some(*id),
            _ => match self.wrapped_data(receiver) {
                Some(super::primitive_wrappers::Data::Symbol(id)) => Some(*id),
                _ => None,
            },
        };
        if let Some(id) = id {
            self.symbol(id)?;
            Ok(id)
        } else {
            Err(Fault::exception(
                "TypeError",
                "Symbol method has an incompatible receiver.",
            ))
        }
    }

    pub(super) fn symbol_description(&mut self, receiver: Value) -> Result<Value, Fault> {
        let id = self.symbol_receiver(&receiver)?;
        let bytes = self.symbol(id)?.description.as_ref().map_or(0, String::len);
        self.work(bytes)?;
        Ok(self
            .symbol(id)?
            .description
            .as_ref()
            .map_or(Value::Undefined, |text| Value::String(text.clone())))
    }

    pub(super) fn symbol_text(&mut self, id: usize) -> Result<Value, Fault> {
        let bytes = self.symbol(id)?.description.as_ref().map_or(0, String::len);
        if bytes.saturating_add(8) > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        self.work(bytes + 8)?;
        Ok(Value::String(format!(
            "Symbol({})",
            self.symbol(id)?.description.as_deref().unwrap_or("")
        )))
    }

    pub(super) fn symbol_builtin(
        &mut self,
        operation: Op,
        args: &[Value],
        receiver: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        match operation {
            Op::Constructor | Op::For => {
                let value = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                let description =
                    if matches!(operation, Op::Constructor) && matches!(value, Value::Undefined) {
                        None
                    } else {
                        let primitive = self.primitive_with_hint(value, true, host)?;
                        self.work(primitive.bytes())?;
                        Some(primitive.script_text()?)
                    };
                if matches!(operation, Op::For) {
                    let key = description.as_ref().unwrap();
                    // Charge a conservative bound for comparisons in the tree.
                    self.work(key.len().saturating_mul(13))?;
                    if let Some(id) = self.symbol_registry.get(key) {
                        return Ok(Value::Symbol(*id));
                    }
                }
                self.new_symbol(description, matches!(operation, Op::For))
            }
            Op::KeyFor => {
                let Some(Value::Symbol(id)) = args.first() else {
                    return Err(Fault::exception(
                        "TypeError",
                        "Symbol.keyFor requires a Symbol.",
                    ));
                };
                if self.symbol(*id)?.registered {
                    self.symbol_description(Value::Symbol(*id))
                } else {
                    Ok(Value::Undefined)
                }
            }
            Op::Description => self.symbol_description(receiver),
            Op::ToString | Op::ValueOf | Op::ToPrimitive => {
                let id = self.symbol_receiver(&receiver)?;
                if matches!(operation, Op::ToString) {
                    self.symbol_text(id)
                } else {
                    Ok(Value::Symbol(id))
                }
            }
        }
    }

    /// Only already registered native expando stores can hold symbol keys.
    /// No key is converted to a native property name or passed to Host::set.
    pub(super) fn symbol_backing(&self, value: &Value, host: &dyn Host) -> Result<Option<Ordinary>, Fault> {
        self.reject_foreign_window(value, host)?;
        if let Some(ordinary) = Ordinary::from_value(value) {
            self.ordinary_id(ordinary)?;
            return Ok(Some(ordinary));
        }
        if let Value::Host(handle) = value {
            if let Some((global, properties)) = self.browser_global {
                if global == *handle {
                    return Ok(Some(Ordinary::Object(properties)));
                }
            }
            return Ok(self.host_objects.get(handle).copied().map(Ordinary::Object));
        }
        Ok(None)
    }

    pub(super) fn symbol_property_owner(
        &mut self,
        mut object: Ordinary,
        key: usize,
    ) -> Result<Option<usize>, Fault> {
        self.symbol(key)?;
        loop {
            self.step()?;
            let id = self.ordinary_id(object)?;
            let data = self.object(id)?;
            if data.symbol_properties.contains_key(&key) {
                return Ok(Some(id));
            }
            let Some(parent) = data.prototype else {
                return Ok(None);
            };
            object = parent;
        }
    }

    pub(super) fn get_symbol_property(
        &mut self,
        receiver: Value,
        key: usize,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        Self::require_property_receiver(&receiver)?;
        self.symbol(key)?;
        let object = match self.symbol_backing(&receiver, host)? {
            Some(object) => Some(object),
            None => match receiver {
                Value::Symbol(_) => self.symbol_prototype.map(Ordinary::Object),
                Value::String(_) => self.string_prototype.map(Ordinary::Object),
                Value::Number(_) => self.number_prototype.map(Ordinary::Object),
                Value::Bool(_) => self.boolean_prototype.map(Ordinary::Object),
                _ => None,
            },
        };
        let Some(object) = object else {
            return Ok(Value::Undefined);
        };
        let Some(owner) = self.symbol_property_owner(object, key)? else {
            return Ok(Value::Undefined);
        };
        let property = &self.objects[owner].symbol_properties[&key];
        if let Some(accessor) = property.accessor {
            return self.call_accessor(accessor.get, receiver, &[], host);
        }
        if let Some(getter) = property.getter {
            let function = property.value.clone();
            return self.native_getter_in_realm(&function, getter, receiver);
        }
        let bytes = property.value.bytes();
        self.work(bytes)?;
        Ok(self.objects[owner].symbol_properties[&key].value.clone())
    }

    pub(super) fn symbol_string_property(
        &mut self,
        symbol: usize,
        name: &str,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.symbol(symbol)?;
        self.property_name(name)?;
        let prototype = Ordinary::Object(self.symbol_prototype.unwrap());
        self.get_ordinary_property_with_receiver(prototype, name, &Value::Symbol(symbol), host)
    }

    pub(super) fn assign_symbol_property(
        &mut self,
        receiver: Value,
        key: usize,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        Self::require_property_receiver(&receiver)?;
        let backing = self.symbol_backing(&receiver, host)?;
        let object = match backing {
            Some(object) => object,
            None => match receiver {
                Value::String(_) => Ordinary::Object(self.string_prototype.unwrap()),
                Value::Number(_) => Ordinary::Object(self.number_prototype.unwrap()),
                Value::Bool(_) => Ordinary::Object(self.boolean_prototype.unwrap()),
                Value::Symbol(_) => Ordinary::Object(self.symbol_prototype.unwrap()),
                _ => return Err(Fault::new(
                    "Symbol properties require an ordinary object, supported primitive or registered native expando store.",
                )),
            },
        };
        if let Some(owner) = self.symbol_property_owner(object, key)? {
            let property = &self.objects[owner].symbol_properties[&key];
            if let Some(accessor) = property.accessor {
                if accessor.set.is_some() {
                    self.call_accessor(accessor.set, receiver, &[value], host)?;
                    return Ok(());
                }
                return if self.strict {
                    Err(Fault::exception("TypeError", "The accessor has no setter."))
                } else {
                    Ok(())
                };
            }
            if property.getter.is_some() || !property.writable {
                return if self.strict {
                    Err(Fault::exception(
                        "TypeError",
                        "The symbol property is read-only.",
                    ))
                } else {
                    Ok(())
                };
            }
        }
        if let Some(object) = backing {
            let target = self.ordinary_id(object)?;
            self.store_symbol_property(target, key, value, None)
        } else if self.strict {
            Err(Fault::exception(
                "TypeError",
                "A primitive value cannot retain an assigned property.",
            ))
        } else {
            Ok(())
        }
    }

    pub(super) fn store_symbol_property(
        &mut self,
        id: usize,
        key: usize,
        value: Value,
        attributes: Option<(bool, bool, bool)>,
    ) -> Result<(), Fault> {
        self.store_symbol_property_kind(id, key, value, attributes, None)
    }

    pub(super) fn store_symbol_accessor_property(
        &mut self,
        id: usize,
        key: usize,
        accessor: Accessors,
        attributes: (bool, bool),
    ) -> Result<(), Fault> {
        self.store_symbol_property_kind(
            id,
            key,
            Value::Undefined,
            Some((attributes.0, false, attributes.1)),
            Some(accessor),
        )
    }

    fn store_symbol_property_kind(
        &mut self,
        id: usize,
        key: usize,
        value: Value,
        attributes: Option<(bool, bool, bool)>,
        accessor: Option<Accessors>,
    ) -> Result<(), Fault> {
        self.step()?;
        self.symbol(key)?;
        let object = self.object(id)?;
        let previous = object.symbol_properties.get(&key);
        if attributes.is_none() && previous.is_some_and(|property| !property.writable) {
            return if self.strict {
                Err(Fault::exception(
                    "TypeError",
                    "The symbol property is read-only.",
                ))
            } else {
                Ok(())
            };
        }
        let is_new = previous.is_none();
        if is_new && !object.extensible {
            return if self.strict || attributes.is_some() {
                Err(Fault::exception(
                    "TypeError",
                    "The object is not extensible.",
                ))
            } else {
                Ok(())
            };
        }
        if is_new
            && (object.properties.len() + object.symbol_properties.len() >= self.limits.heap.object_properties
                || self.properties >= self.limits.heap.properties)
        {
            return Err(self.property_limit_fault(object));
        }
        let order = previous.map_or(object.next_order, |property| property.order);
        let enumerable = attributes.map_or_else(
            || previous.is_none_or(|property| property.enumerable),
            |attrs| attrs.0,
        );
        let writable = attributes.is_none_or(|attrs| attrs.1);
        let configurable = attributes.map_or_else(
            || previous.is_none_or(|property| property.configurable),
            |attrs| attrs.2,
        );
        let next_order = object
            .next_order
            .checked_add(usize::from(is_new))
            .ok_or_else(|| Fault::new("Object property order limit reached."))?;
        let bytes = value.bytes();
        if bytes > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        let retained =
            self.value_bytes - previous.map_or(0, |property| property.value.bytes()) + bytes;
        if retained > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work(bytes)?;
        self.objects[id].symbol_properties.insert(
            key,
            Property {
                value,
                getter: None,
                accessor,
                order,
                enumerable,
                writable,
                configurable,
            },
        );
        self.objects[id].next_order = next_order;
        self.value_bytes = retained;
        self.properties += usize::from(is_new);
        Ok(())
    }

    pub(super) fn remove_symbol_property(&mut self, id: usize, key: usize) -> Result<(), Fault> {
        self.step()?;
        self.symbol(key)?;
        self.object(id)?;
        if let Some(property) = self.objects[id].symbol_properties.remove(&key) {
            self.value_bytes -= property.value.bytes();
            self.properties -= 1;
        }
        Ok(())
    }

    pub(super) fn own_symbol_keys(
        &mut self,
        id: usize,
        enumerable_only: bool,
    ) -> Result<Vec<usize>, Fault> {
        let count = self.object(id)?.symbol_properties.len();
        self.work(count.saturating_mul(std::mem::size_of::<(usize, usize)>()))?;
        let mut keys = self.objects[id]
            .symbol_properties
            .iter()
            .filter(|(_, property)| !enumerable_only || property.enumerable)
            .map(|(&key, property)| (property.order, key))
            .collect::<Vec<_>>();
        keys.sort_unstable_by_key(|&(order, _)| order);
        Ok(keys.into_iter().map(|(_, key)| key).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("Unexpected native read.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Unexpected native write.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("Unexpected native call.".into())
        }
    }

    fn fresh(runtime: &mut Runtime) -> usize {
        let Value::Symbol(id) = runtime.new_symbol(None, false).unwrap() else {
            unreachable!()
        };
        id
    }

    #[test]
    fn string_and_symbol_properties_share_object_limit_and_retained_storage() {
        let mut runtime = Runtime::for_test();
        let object = runtime.new_object(None).unwrap();
        let key = fresh(&mut runtime);
        for index in 0..MAX_OBJECT_PROPERTIES - 1 {
            runtime
                .set_property(object, index.to_string(), Value::Number(1.0))
                .unwrap();
        }
        let before = runtime.value_bytes;
        runtime
            .store_symbol_property(object, key, Value::String("held".into()), None)
            .unwrap();
        assert_eq!(runtime.value_bytes, before + 4);
        let other = fresh(&mut runtime);
        let count = runtime.properties;
        let retained = runtime.value_bytes;
        for error in [
            runtime
                .set_property(object, "extra".into(), Value::Number(2.0))
                .unwrap_err(),
            runtime
                .store_symbol_property(object, other, Value::Number(2.0), None)
                .unwrap_err(),
        ] {
            assert!(error.display().contains("Object property limit"));
        }
        assert_eq!(runtime.properties, count);
        assert_eq!(runtime.value_bytes, retained);
        runtime
            .store_symbol_property(object, key, Value::String("replaced".into()), None)
            .unwrap();
        assert_eq!(runtime.value_bytes, before + 8);
        runtime.remove_symbol_property(object, key).unwrap();
        assert_eq!(runtime.value_bytes, before);
        assert_eq!(runtime.properties, count - 1);
        runtime
            .set_property(object, "extra".into(), Value::Number(2.0))
            .unwrap();
    }

    #[test]
    fn symbol_properties_cannot_bypass_document_property_limit() {
        let mut runtime = Runtime::for_test();
        let keys: Vec<_> = (0..(MAX_OBJECT_PROPERTIES - 1)
            .min(MAX_SYMBOLS - runtime.symbols.len()))
            .map(|_| fresh(&mut runtime))
            .collect();
        let mut object = runtime.new_object(None).unwrap();
        let mut index = 0;
        while runtime.properties < MAX_PROPERTIES {
            if index == keys.len() {
                object = runtime.new_object(None).unwrap();
                index = 0;
            }
            runtime
                .store_symbol_property(object, keys[index], Value::Number(0.0), None)
                .unwrap();
            index += 1;
        }
        let empty = runtime.new_object(None).unwrap();
        assert!(runtime
            .store_symbol_property(empty, keys[0], Value::Null, None)
            .unwrap_err()
            .display()
            .contains("Object property limit"));
        assert!(runtime
            .set_property(empty, "plain".into(), Value::Null)
            .unwrap_err()
            .display()
            .contains("Object property limit"));
        assert_eq!(runtime.properties, MAX_PROPERTIES);
        runtime
            .remove_symbol_property(object, keys[index - 1])
            .unwrap();
        runtime
            .set_property(empty, "plain".into(), Value::Null)
            .unwrap();
        assert_eq!(runtime.properties, MAX_PROPERTIES);
    }

    #[test]
    fn symbol_descriptions_and_registry_share_retained_value_budget() {
        let mut runtime = Runtime::for_test();
        let initial = runtime.value_bytes;
        runtime.new_symbol(Some("plain".into()), false).unwrap();
        runtime.new_symbol(Some("registered".into()), true).unwrap();
        assert_eq!(runtime.value_bytes, initial + 5 + 2 * 10);
        while runtime.value_bytes + MAX_STRING_BYTES <= MAX_RETAINED_VALUES {
            runtime
                .new_symbol(Some("x".repeat(MAX_STRING_BYTES)), false)
                .unwrap();
        }
        let count = runtime.symbols.len();
        let retained = runtime.value_bytes;
        let registry = runtime.symbol_registry.len();
        let error = runtime
            .new_symbol(Some("y".repeat(MAX_STRING_BYTES)), true)
            .unwrap_err();
        assert!(error.display().contains("Retained value limit"));
        assert_eq!(runtime.symbols.len(), count);
        assert_eq!(runtime.value_bytes, retained);
        assert_eq!(runtime.symbol_registry.len(), registry);
    }

    #[test]
    fn symbol_identity_limit_persists_across_jobs_and_cannot_be_caught() {
        let mut runtime = Runtime::for_test();
        runtime
            .execute(
                "var saved=Symbol.for('saved'); var caught=false,finalized=false;",
                &mut NoHost,
            )
            .unwrap();
        while runtime.symbols.len() < MAX_SYMBOLS {
            fresh(&mut runtime);
        }
        let error = runtime
            .execute(
                "try{Symbol();}catch(e){caught=true;}finally{finalized=true;}",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("Symbol identity limit reached"));
        assert_eq!(runtime.execute("!caught&&!finalized&&saved===Symbol.for('saved')&&saved.description==='saved';", &mut NoHost).unwrap().to_text(), "true");
        let mut next_document = Runtime::for_test();
        assert_eq!(
            next_document
                .execute("typeof Symbol()", &mut NoHost)
                .unwrap()
                .to_text(),
            "symbol"
        );
    }
}
