//! Finite native WeakMap/WeakSet operations over the identity collection store.
//! Weak entries are not strong graph edges. The collector separately reaches
//! values from independently live keys, then removes dead entries before any
//! callable slot can be reused. Host and symbol arenas remain conservative.
//!
//! https://tc39.es/ecma262/#sec-weakmap-objects
//! https://tc39.es/ecma262/#sec-weakset-objects
//! https://tc39.es/ecma262/#sec-canbeheldweakly

use super::{Builtin, Data, Declaration, Fault, Host, Key, Kind, Op, Ordinary, Runtime, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum WeakKey {
    Object(usize),
    Function(usize),
    Host(u32),
    Symbol(usize),
}

impl WeakKey {
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Object(id) => Some(Self::Object(*id)),
            Value::Function(id) => Some(Self::Function(*id)),
            Value::Host(id) => Some(Self::Host(*id)),
            Value::Symbol(id) => Some(Self::Symbol(*id)),
            _ => None,
        }
    }

    fn lookup(self) -> Key {
        match self {
            Self::Object(id) => Key::Object(id),
            Self::Function(id) => Key::Function(id),
            Self::Host(id) => Key::Host(id),
            Self::Symbol(id) => Key::Symbol(id),
        }
    }
}

impl Data {
    pub(in super::super) fn is_weak_map(&self) -> bool {
        self.kind == Kind::WeakMap
    }

    pub(in super::super) fn is_weak_collection(&self) -> bool {
        self.kind.is_weak()
    }

    /// No tombstones or hash-capacity walk: each occupied ordered slot is one
    /// bounded visit, with at most one hash removal during the commit pass.
    pub(in super::super) fn weak_slot_count(&self) -> usize {
        if self.is_weak_collection() {
            self.ordered.len()
        } else {
            0
        }
    }

    pub(in super::super) fn weak_entries(&self) -> impl Iterator<Item = (WeakKey, &Value)> {
        let weak = self.is_weak_collection();
        self.ordered.values().filter_map(move |entry| {
            if weak {
                WeakKey::from_value(&entry.key).map(|key| (key, &entry.value))
            } else {
                None
            }
        })
    }

    /// Predicate true means remove. The collector prepays these visits and
    /// validates the exact global entry/string refunds before committing.
    pub(in super::super) fn weak_removal_totals(
        &self,
        remove: &impl Fn(WeakKey) -> bool,
    ) -> (usize, usize) {
        let mut entries = 0;
        let mut bytes = 0;
        for (key, value) in self.weak_entries() {
            if remove(key) {
                entries += 1;
                bytes += value.bytes();
            }
        }
        (entries, bytes)
    }

    /// Allocation-free, infallible publication after the caller's preflight.
    /// Keys carry no retained string bytes; only the stored value is refunded.
    pub(in super::super) fn remove_weak_entries(
        &mut self,
        remove: &impl Fn(WeakKey) -> bool,
    ) -> (usize, usize) {
        if !self.is_weak_collection() {
            return (0, 0);
        }
        let mut entries = 0;
        let mut bytes = 0;
        let lookup = &mut self.lookup;
        self.ordered.retain(|_, entry| {
            let Some(key) = WeakKey::from_value(&entry.key) else {
                return true;
            };
            if !remove(key) {
                return true;
            }
            lookup.remove(&key.lookup());
            entries += 1;
            bytes += entry.value.bytes();
            false
        });
        (entries, bytes)
    }
}

impl Runtime {
    pub(super) fn initialize_weak_collections(&mut self) -> Result<(), Fault> {
        for kind in [Kind::WeakMap, Kind::WeakSet] {
            self.initialize_weak_collection(kind)?;
        }
        Ok(())
    }

    fn initialize_weak_collection(&mut self, kind: Kind) -> Result<(), Fault> {
        if !kind.is_weak() {
            return Err(Fault::new("Invalid weak collection intrinsic kind."));
        }
        let prototype = self.new_object(None)?;
        if kind == Kind::WeakMap {
            self.collection_roots.weak_map = Some(prototype);
        } else {
            self.collection_roots.weak_set = Some(prototype);
        }
        let constructor =
            self.native_function(Builtin::Collection(Op::Constructor(kind)), kind.name())?;
        self.put_binding(
            self.global_environment,
            kind.name().into(),
            Some(constructor),
            Declaration::Var,
            false,
        )?;
        self.constructor_prototype(kind.name(), Value::Object(prototype))?;
        let methods: &[&'static str] = if kind == Kind::WeakMap {
            &["delete", "get", "has", "set"]
        } else {
            &["add", "delete", "has"]
        };
        for &name in methods {
            self.prototype_method(prototype, name, Builtin::Collection(Op::Method(kind, name)))?;
        }
        self.store_symbol_property(
            prototype,
            self.symbol_to_string_tag.unwrap(),
            Value::String(kind.name().into()),
            Some((false, false, true)),
        )
    }

    /// This check performs no primitive conversion, author property lookup or
    /// expando allocation. Native identities retain the existing owner guard.
    fn valid_weak_key(&self, key: &Value, host: &dyn Host) -> Result<bool, Fault> {
        match key {
            Value::Object(id) => {
                self.object(*id)?;
                Ok(true)
            }
            Value::Function(id) => {
                self.ordinary_id(Ordinary::Function(*id))?;
                Ok(true)
            }
            Value::Host(handle) => {
                self.validate_native_access(*handle, host)?;
                Ok(true)
            }
            Value::Symbol(id) => self.weak_symbol_key(*id),
            _ => Ok(false),
        }
    }

    #[cfg(test)]
    pub(super) fn weak_map_method(
        &mut self,
        method: &'static str,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.weak_collection_method(Kind::WeakMap, method, args, this, host)
    }

    pub(super) fn weak_collection_method(
        &mut self,
        kind: Kind,
        method: &'static str,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        // Brand is checked before key classification or native-owner callbacks.
        let id = self.collection_receiver(&this, kind)?;
        let key = args.first().unwrap_or(&Value::Undefined);
        if !self.valid_weak_key(key, host)? {
            return match method {
                "get" => Ok(Value::Undefined),
                "has" | "delete" => Ok(Value::Bool(false)),
                "set" => Err(Fault::exception("TypeError", "Invalid WeakMap key.")),
                "add" => Err(Fault::exception("TypeError", "Invalid WeakSet value.")),
                _ => Err(Fault::new("Unsupported weak collection method.")),
            };
        }
        match method {
            "set" | "add" => {
                self.collection_put(id, kind, key, args.get(1).unwrap_or(&Value::Undefined))?;
                Ok(this)
            }
            "delete" => self.collection_delete(id, kind, key),
            "get" | "has" => {
                let lookup = self.collection_key(key)?;
                let order = self.collection(id, kind)?.lookup.get(&lookup).copied();
                if method == "has" {
                    return Ok(Value::Bool(order.is_some()));
                }
                let Some(order) = order else {
                    return Ok(Value::Undefined);
                };
                let bytes = self.collection(id, kind)?.ordered[&order].value.bytes();
                self.work(bytes)?;
                Ok(self.collection(id, kind)?.ordered[&order].value.clone())
            }
            _ => Err(Fault::new("Unsupported weak collection method.")),
        }
    }
}
