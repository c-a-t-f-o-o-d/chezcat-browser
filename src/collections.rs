//! Original insertion-ordered Map/Set and native iterator support. Hash lookup
//! preserves identity/SameValueZero; monotonically ordered live entries keep
//! deletion/reinsertion observable to active iterators without tombstone growth.
//! Entry payload and owned order/lookup capacity share retained-value storage.
//! Deletion compacts live order slots; clear releases the owned backing stores.

#[cfg(test)]
use super::MAX_STRING_BYTES;
use super::{
    Builtin, Declaration, Fault, Host, NativeGetter, Ordinary, Runtime, Value,
};
use std::collections::HashMap;

#[path = "weak_map.rs"]
mod weak_map;
pub(super) use weak_map::WeakKey;
#[cfg(test)]
#[path = "weak_map_api_tests.rs"]
mod weak_map_api_tests;
#[cfg(test)]
#[path = "weak_set_api_tests.rs"]
mod weak_set_api_tests;

#[cfg(test)]
const MAX_ENTRIES: usize = 4096;
#[cfg(test)]
const MAX_DOCUMENT_ENTRIES: usize = 16384;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Kind {
    Map,
    Set,
    WeakMap,
    WeakSet,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Map => "Map",
            Self::Set => "Set",
            Self::WeakMap => "WeakMap",
            Self::WeakSet => "WeakSet",
        }
    }

    fn is_weak(self) -> bool {
        matches!(self, Self::WeakMap | Self::WeakSet)
    }

    fn is_set(self) -> bool {
        matches!(self, Self::Set | Self::WeakSet)
    }
}
#[derive(Eq, PartialEq, Hash)]
enum Key {
    Undefined,
    Null,
    Bool(bool),
    Number(u64),
    BigInt(std::sync::Arc<str>),
    String(String),
    StringUnits(Vec<u16>),
    Symbol(usize),
    Object(usize),
    Function(usize),
    Host(u32),
}
struct Entry {
    key: Value,
    value: Value,
}
// Sequence numbers stay monotonic while deleted slots are compacted. Native
// iterators seek by sequence, so compaction never skips a later insertion.
#[derive(Default)]
struct OrderedEntries(Vec<(usize, Entry)>);
impl OrderedEntries {
    fn len(&self) -> usize { self.0.len() }
    #[cfg(test)]
    fn is_empty(&self) -> bool { self.0.is_empty() }
    fn values(&self) -> impl Iterator<Item=&Entry> { self.0.iter().map(|(_, entry)| entry) }
    fn get_mut(&mut self, order: &usize) -> Option<&mut Entry> {
        self.0.binary_search_by_key(order, |(order, _)| *order).ok().map(|index| &mut self.0[index].1)
    }
    fn remove(&mut self, order: &usize) -> Option<Entry> {
        self.0.binary_search_by_key(order, |(order, _)| *order).ok().map(|index| self.0.remove(index).1)
    }
    fn insert(&mut self, order: usize, entry: Entry) { self.0.push((order, entry)); }
    fn retain(&mut self, mut keep: impl FnMut(&usize, &Entry)->bool) { self.0.retain(|(order, entry)| keep(order, entry)); }
    fn range(&self, range: std::ops::RangeFrom<usize>) -> impl Iterator<Item=(&usize, &Entry)> {
        let index = self.0.partition_point(|(order, _)| *order < range.start);
        self.0[index..].iter().map(|(order, entry)| (order, entry))
    }
}
impl std::ops::Index<&usize> for OrderedEntries {
    type Output = Entry;
    fn index(&self, order: &usize) -> &Entry {
        &self.0[self.0.binary_search_by_key(order, |(order, _)| *order).expect("live collection sequence")].1
    }
}

pub(super) struct Data {
    pub kind: Kind,
    lookup: HashMap<Key, usize>,
    lookup_storage_capacity: usize,
    ordered: OrderedEntries,
    sequence: usize,
}
impl Data {
    pub(super) fn reachability_cost(&self) -> usize {
        if self.is_weak_collection() { 0 } else { self.ordered.len() * 2 }
    }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        // Weak values are activated only by the collector's ephemeron fixpoint;
        // neither side of a weak entry is an unconditional strong edge.
        if self.is_weak_collection() { return; }
        for entry in self.ordered.values() { Edge::value(&entry.key, add); Edge::value(&entry.value, add); }
    }

    fn new(kind: Kind) -> Self {
        Self {
            kind,
            lookup: HashMap::new(),
            lookup_storage_capacity: 0,
            ordered: OrderedEntries::default(),
            sequence: 0,
        }
    }
    fn storage_bytes(&self) -> usize {
        self.lookup_storage_capacity * std::mem::size_of::<(Key, usize)>()
            + self.ordered.0.capacity() * std::mem::size_of::<(usize, Entry)>()
    }

    /// Prepare both replacements before moving either live store. Refusal
    /// leaves entries, capacities and sequence ownership unchanged.
    fn reserve_entry(&mut self, available: usize) -> Result<usize, Fault> {
        let mut lookup = None;
        let mut ordered = None;
        if self.lookup.len() == self.lookup.capacity() {
            let mut replacement = HashMap::new();
            let before = self.lookup_storage_capacity;
            let capacity = if self.lookup.len() < before { before } else {
                let growth = before.max(4).min(available / std::mem::size_of::<(Key, usize)>());
                if growth == 0 { return Err(Fault::new("Collection retained storage exhausted.")); }
                before.checked_add(growth).ok_or_else(|| Fault::new("Collection storage size overflow."))?
            };
            replacement.try_reserve(capacity)
                .map_err(|_| Fault::new("Cannot allocate collection lookup storage."))?;
            lookup = Some(replacement);
        }
        if self.ordered.len() == self.ordered.0.capacity() {
            let mut replacement = Vec::new();
            let before = self.ordered.0.capacity();
            let growth = before.max(4).min(available / std::mem::size_of::<(usize, Entry)>());
            if growth == 0 { return Err(Fault::new("Collection retained storage exhausted.")); }
            replacement.try_reserve_exact(before.checked_add(growth)
                .ok_or_else(|| Fault::new("Collection storage size overflow."))?)
                .map_err(|_| Fault::new("Cannot allocate collection order storage."))?;
            ordered = Some(replacement);
        }
        let added_lookup = lookup.as_ref().map_or(0, |next| next.capacity() - self.lookup_storage_capacity)
            .checked_mul(std::mem::size_of::<(Key, usize)>()).ok_or_else(|| Fault::new("Collection storage size overflow."))?;
        let added_ordered = ordered.as_ref().map_or(0, |next| next.capacity() - self.ordered.0.capacity())
            .checked_mul(std::mem::size_of::<(usize, Entry)>()).ok_or_else(|| Fault::new("Collection storage size overflow."))?;
        let added = added_lookup.checked_add(added_ordered).filter(|added| *added <= available)
            .ok_or_else(|| Fault::new("Collection retained storage exhausted."))?;
        if let Some(mut next) = lookup { next.extend(self.lookup.drain()); self.lookup_storage_capacity = next.capacity(); self.lookup = next; }
        if let Some(mut next) = ordered { next.append(&mut self.ordered.0); self.ordered.0 = next; }
        Ok(added)
    }

    pub(super) fn tag(&self) -> &'static str {
        // Weak collection branding comes from ordinary @@toStringTag descriptors.
        // Deleting that descriptor restores Object.prototype.toString's tag.
        if self.is_weak_collection() { "Object" } else { self.kind.name() }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Family {
    Array,
    Map,
    Set,
    String,
}
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Keys,
    Values,
    Entries,
}
#[derive(Clone, Copy)]
enum IteratorSource {
    Ordinary(usize),
    IndexedHost { handle: u32, owner: u32 },
}
pub(super) struct IteratorData {
    pub family: Family,
    source: Option<IteratorSource>,
    next: usize,
    mode: Mode,
    text: Option<String>,
}
impl IteratorData {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        if let Some(IteratorSource::Ordinary(id)) = self.source { add(super::function_reachability::Edge::Object(id)); }
        // Approved Host sources retain their existing append-only native list.
        // Native expando stores are already conservative reachability roots.
    }

    pub(super) fn tag(&self) -> &'static str {
        match self.family {
            Family::Array => "Array Iterator",
            Family::Map => "Map Iterator",
            Family::Set => "Set Iterator",
            Family::String => "String Iterator",
        }
    }
}
#[derive(Default)]
pub(super) struct Roots {
    map: Option<usize>,
    set: Option<usize>,
    weak_map: Option<usize>,
    weak_set: Option<usize>,
    pub(super) iterator_prototype: Option<usize>,
    array_iterator: Option<usize>,
    map_iterator: Option<usize>,
    set_iterator: Option<usize>,
    string_iterator: Option<usize>,
    array_keys: Option<usize>,
    array_values: Option<usize>,
    array_entries: Option<usize>,
    array_for_each: Option<usize>,
}

impl Roots {
    fn prototype(&self, kind: Kind) -> Option<Ordinary> {
        match kind {
            Kind::Map => self.map,
            Kind::Set => self.set,
            Kind::WeakMap => self.weak_map,
            Kind::WeakSet => self.weak_set,
        }.map(Ordinary::Object)
    }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        for id in [self.map, self.set, self.weak_map, self.weak_set, self.iterator_prototype, self.array_iterator, self.map_iterator, self.set_iterator, self.string_iterator].into_iter().flatten() { add(Edge::Object(id)); }
        for id in [self.array_keys,self.array_values,self.array_entries,self.array_for_each].into_iter().flatten() { add(Edge::Function(id)); }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor(Kind),
    Method(Kind, &'static str),
    Size(Kind),
    ArrayIterator(Mode),
    Next(Family),
}
impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Constructor(_) | Self::Size(_) | Self::ArrayIterator(_) | Self::Next(_) => 0,
            Self::Method(_, "set") => 2,
            Self::Method(_, "clear" | "keys" | "values" | "entries") => 0,
            _ => 1,
        }
    }
}

impl Runtime {
    pub(super) fn initialize_collections(&mut self) -> Result<(), Fault> {
        for kind in [Kind::Map, Kind::Set] {
            let prototype = self.new_object(None)?;
            if kind == Kind::Map {
                self.collection_roots.map = Some(prototype);
            } else {
                self.collection_roots.set = Some(prototype);
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
            self.native_accessor(
                prototype,
                "size",
                Builtin::Collection(Op::Size(kind)),
                NativeGetter::CollectionSize(kind),
            )?;
            for name in [
                "has", "delete", "clear", "forEach", "keys", "values", "entries",
            ] {
                self.prototype_method(
                    prototype,
                    name,
                    Builtin::Collection(Op::Method(kind, name)),
                )?;
            }
            if kind == Kind::Map {
                for name in ["get", "set"] {
                    self.prototype_method(
                        prototype,
                        name,
                        Builtin::Collection(Op::Method(kind, name)),
                    )?;
                }
            } else {
                self.prototype_method(
                    prototype,
                    "add",
                    Builtin::Collection(Op::Method(kind, "add")),
                )?;
                let values = self.get_property(prototype, "values")?;
                self.store_property(prototype, "keys".into(), values, Some((false, true, true)))?;
            }
        }
        self.initialize_weak_collections()?;
        let iterator_prototype = self.new_object(None)?;
        self.collection_roots.iterator_prototype = Some(iterator_prototype);
        let identity = self.native_function(
            Builtin::Iterator(super::iterator_protocol::Op::Self_),
            "[Symbol.iterator]",
        )?;
        self.store_symbol_property(
            iterator_prototype,
            self.symbol_iterator.unwrap(),
            identity,
            Some((false, true, true)),
        )?;
        for family in [Family::Array, Family::Map, Family::Set, Family::String] {
            let prototype = self.new_object(None)?;
            match family {
                Family::Array => self.collection_roots.array_iterator = Some(prototype),
                Family::Map => self.collection_roots.map_iterator = Some(prototype),
                Family::Set => self.collection_roots.set_iterator = Some(prototype),
                Family::String => self.collection_roots.string_iterator = Some(prototype),
            }
            self.objects[prototype].prototype = Some(Ordinary::Object(iterator_prototype));
            self.prototype_method(prototype, "next", Builtin::Collection(Op::Next(family)))?;
        }
        let array = self.array_prototype.unwrap();
        let Value::Function(each)=self.get_property(array,"forEach")? else{unreachable!()};
        self.collection_roots.array_for_each=Some(each);
        for (name, mode) in [
            ("keys", Mode::Keys),
            ("values", Mode::Values),
            ("entries", Mode::Entries),
        ] {
            self.prototype_method(array, name, Builtin::Collection(Op::ArrayIterator(mode)))?;
            let Value::Function(id)=self.get_property(array,name)? else {unreachable!()};
            match mode {
                Mode::Keys=>self.collection_roots.array_keys=Some(id),
                Mode::Values=>self.collection_roots.array_values=Some(id),
                Mode::Entries=>self.collection_roots.array_entries=Some(id),
            }
        }
        let values = self.get_property(array, "values")?;
        let Value::Function(function) = values else {
            unreachable!()
        };
        self.collection_roots.array_values = Some(function);
        self.store_symbol_property(
            array,
            self.symbol_iterator.unwrap(),
            values,
            Some((false, true, true)),
        )?;
        for (prototype, name) in [
            (self.collection_roots.map.unwrap(), "entries"),
            (self.collection_roots.set.unwrap(), "values"),
        ] {
            let method = self.get_property(prototype, name)?;
            self.store_symbol_property(
                prototype,
                self.symbol_iterator.unwrap(),
                method,
                Some((false, true, true)),
            )?;
        }
        let string = self.native_function(
            Builtin::Iterator(super::iterator_protocol::Op::String),
            "[Symbol.iterator]",
        )?;
        self.store_symbol_property(
            self.string_prototype.unwrap(),
            self.symbol_iterator.unwrap(),
            string,
            Some((false, true, true)),
        )?;
        Ok(())
    }
    fn collection(&self, id: usize, kind: Kind) -> Result<&Data, Fault> {
        self.object(id)?
            .collection
            .as_ref()
            .filter(|data| data.kind == kind)
            .ok_or_else(|| {
                Fault::exception(
                    "TypeError",
                    "Collection method has an incompatible receiver.",
                )
            })
    }
    fn collection_receiver(&self, this: &Value, kind: Kind) -> Result<usize, Fault> {
        let Value::Object(id) = this else {
            return Err(Fault::exception(
                "TypeError",
                "Collection method has an incompatible receiver.",
            ));
        };
        self.collection(*id, kind)?;
        Ok(*id)
    }
    pub(super) fn collection_size(&mut self, this: Value, kind: Kind) -> Result<Value, Fault> {
        self.step()?;
        let id = self.collection_receiver(&this, kind)?;
        Ok(Value::Number(
            self.collection(id, kind)?.ordered.len() as f64
        ))
    }
    fn collection_key(&mut self, value: &Value) -> Result<Key, Fault> {
        self.step()?;
        self.work(value.bytes())?;
        Ok(match value {
            Value::Undefined => Key::Undefined,
            Value::Null => Key::Null,
            Value::Bool(value) => Key::Bool(*value),
            Value::Number(value) => Key::Number(if value.is_nan() {
                f64::NAN.to_bits()
            } else if *value == 0.0 {
                0
            } else {
                value.to_bits()
            }),
            Value::BigInt(value) => Key::BigInt(value.clone()),
            Value::String(value) => Key::String(value.clone()),
            Value::StringUnits(units) => Key::StringUnits(units.clone()),
            Value::Symbol(id) => Key::Symbol(*id),
            Value::Object(id) => Key::Object(*id),
            Value::Function(id) => Key::Function(*id),
            Value::Host(id) => Key::Host(*id),
        })
    }
    fn collection_put(
        &mut self,
        id: usize,
        kind: Kind,
        key: &Value,
        value: &Value,
    ) -> Result<(), Fault> {
        let lookup = self.collection_key(key)?;
        let existing = self.collection(id, kind)?.lookup.get(&lookup).copied();
        let old_bytes = existing.map_or(0, |order| {
            self.collection(id, kind).unwrap().ordered[&order]
                .value
                .bytes()
        });
        let value = if kind.is_set() {
            Value::Undefined
        } else {
            self.copy_value(value)?
        };
        let key_bytes = if existing.is_none() {
            key.bytes().saturating_mul(2)
        } else {
            0
        };
        let mut retained = self.value_bytes.checked_sub(old_bytes)
            .and_then(|bytes| bytes.checked_add(key_bytes))
            .and_then(|bytes| bytes.checked_add(value.bytes()))
            .ok_or_else(|| Fault::new("Collection retained storage size overflow."))?;
        if key.bytes() > self.limits.capacity.string_bytes || value.bytes() > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        if retained > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        if let Some(order) = existing {
            self.objects[id]
                .collection
                .as_mut()
                .unwrap()
                .ordered
                .get_mut(&order)
                .unwrap()
                .value = value;
        } else {
            #[cfg(test)]
            if self.limits.test_collection_entries.is_some_and(|(single, document)|
                self.collection(id, kind).unwrap().ordered.len() >= single || self.collection_entries >= document)
            {
                return Err(Fault::new(
                    "Collection entry limit reached (4096 per collection / 16384 per document).",
                ));
            }
            let key = match key {
                Value::Number(number) if *number == 0.0 => Value::Number(0.0),
                _ => self.copy_value(key)?,
            };
            let data = self.collection(id, kind)?;
            let order = data.sequence;
            let sequence = order.checked_add(1).ok_or_else(|| Fault::new("Collection insertion order limit reached."))?;
            let count = self.collection_entries.checked_add(1).ok_or_else(|| Fault::new("Collection entry counter overflow."))?;
            if data.lookup.len() == data.lookup.capacity() || data.ordered.len() == data.ordered.0.capacity() {
                self.work(data.lookup_storage_capacity.saturating_add(data.ordered.len().saturating_mul(4)))?;
            }
            let data = self.objects[id].collection.as_mut().unwrap();
            retained += data.reserve_entry(self.limits.heap.retained_values - retained)?;
            data.sequence = sequence;
            data.lookup.insert(lookup, order);
            data.ordered.insert(order, Entry { key, value });
            self.collection_entries = count;
        }
        self.value_bytes = retained;
        Ok(())
    }
    fn collection_delete(&mut self, id: usize, kind: Kind, key: &Value) -> Result<Value, Fault> {
        let key = self.collection_key(key)?;
        let data = self.collection(id, kind)?;
        if data.lookup.contains_key(&key) { self.work(data.ordered.len())?; }
        let data = self.objects[id].collection.as_mut().unwrap();
        if let Some(order) = data.lookup.remove(&key) {
            let entry = data.ordered.remove(&order).unwrap();
            self.value_bytes -= entry.key.bytes() * 2 + entry.value.bytes();
            self.collection_entries -= 1;
            Ok(Value::Bool(true))
        } else {
            Ok(Value::Bool(false))
        }
    }
    fn collection_clear(&mut self, id: usize, kind: Kind) -> Result<Value, Fault> {
        let data = self.collection(id, kind)?;
        let length = data.ordered.len();
        self.work(length.saturating_mul(8).saturating_add(data.lookup_storage_capacity))?;
        let data = self.objects[id].collection.as_mut().unwrap();
        self.value_bytes -= data
            .ordered
            .values()
            .map(|entry| entry.key.bytes() * 2 + entry.value.bytes())
            .sum::<usize>();
        self.collection_entries -= length;
        self.value_bytes -= data.storage_bytes();
        data.lookup = HashMap::new();
        data.lookup_storage_capacity = 0;
        data.ordered = OrderedEntries::default();
        // Do not reset sequence: an existing iterator must see later additions.
        Ok(Value::Undefined)
    }
    fn next_collection_entry(
        &mut self,
        id: usize,
        next: usize,
    ) -> Result<Option<(usize, Value, Value)>, Fault> {
        self.step()?;
        let data = self
            .object(id)?
            .collection
            .as_ref()
            .ok_or_else(|| Fault::new("Invalid collection iterator source."))?;
        let Some((&order, entry)) = data.ordered.range(next..).next() else {
            return Ok(None);
        };
        let bytes = entry.key.bytes().saturating_add(entry.value.bytes());
        self.work(bytes)?;
        let entry = &self.objects[id].collection.as_ref().unwrap().ordered[&order];
        Ok(Some((order, entry.key.clone(), entry.value.clone())))
    }
    pub(super) fn new_native_iterator(
        &mut self,
        source: usize,
        family: Family,
        mode: Mode,
    ) -> Result<usize, Fault> {
        self.new_native_iterator_source(IteratorSource::Ordinary(source), family, mode)
    }

    fn new_native_iterator_source(&mut self, source: IteratorSource, family: Family, mode: Mode)
        -> Result<usize, Fault>
    {
        let id = self.new_object(None)?;
        self.objects[id].iterator = Some(IteratorData {
            family,
            source: Some(source),
            next: 0,
            mode,
            text: None,
        });
        self.objects[id].prototype = match family {
            Family::Array => self.collection_roots.array_iterator,
            Family::Map => self.collection_roots.map_iterator,
            Family::Set => self.collection_roots.set_iterator,
            Family::String => self.collection_roots.string_iterator,
        }
        .map(Ordinary::Object);
        Ok(id)
    }
    pub(super) fn arguments_iterator_method(&self) -> Value {
        Value::Function(self.collection_roots.array_values.unwrap())
    }
    pub(super) fn array_for_each_intrinsic(&self)->Value{Value::Function(self.collection_roots.array_for_each.unwrap())}

    pub(super) fn array_iterator_intrinsic(&self,mode:Mode)->Value {
        Value::Function(match mode {
            Mode::Keys=>self.collection_roots.array_keys,
            Mode::Values=>self.collection_roots.array_values,
            Mode::Entries=>self.collection_roots.array_entries,
        }.unwrap())
    }

    pub(super) fn new_string_iterator(&mut self, text: String) -> Result<usize, Fault> {
        if text.len() > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        if self.value_bytes.saturating_add(text.len()) > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work(text.len())?;
        let id = self.new_object(None)?;
        self.value_bytes += text.len();
        self.objects[id].iterator = Some(IteratorData {
            family: Family::String,
            source: None,
            next: 0,
            mode: Mode::Values,
            text: Some(text),
        });
        self.objects[id].prototype = self.collection_roots.string_iterator.map(Ordinary::Object);
        Ok(id)
    }

    pub(super) fn next_native_iterator(
        &mut self,
        id: usize,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        self.step()?;
        let iterator =
            self.object(id)?.iterator.as_ref().ok_or_else(|| {
                Fault::exception("TypeError", "An iterator receiver is required.")
            })?;
        let (family, source, next, mode) = (
            iterator.family,
            iterator.source,
            iterator.next,
            iterator.mode,
        );
        if family == Family::String {
            let Some(text) = &iterator.text else {
                return Ok(None);
            };
            let Some(ch) = text[next..].chars().next() else {
                let bytes = text.len();
                self.objects[id].iterator.as_mut().unwrap().text = None;
                self.value_bytes -= bytes;
                return Ok(None);
            };
            self.work(ch.len_utf8())?;
            self.objects[id].iterator.as_mut().unwrap().next += ch.len_utf8();
            return Ok(Some(Value::String(ch.to_string())));
        }
        let Some(source) = source else {
            return Ok(None);
        };
        let pair = match family {
            Family::Array => match source {
                IteratorSource::IndexedHost { handle, owner } =>
                    self.next_indexed_host_pair(id, handle, owner, next, mode, host)?,
                IteratorSource::Ordinary(source) => {
                let view = self.typed_view(source);
                let length = if let Some(view) = view {
                    view.length
                } else {
                    self.array_like_length(source, host)?
                };
                if next >= length {
                    None
                } else {
                    // Commit the advance before an indexed getter can reenter
                    // this iterator or throw; the next call must not repeat it.
                    self.objects[id].iterator.as_mut().unwrap().next = next + 1;
                    let key = Value::Number(next as f64);
                    let value = if matches!(mode, Mode::Keys) {
                        Value::Undefined
                    } else if let Some(view) = view {
                        Value::Number(self.typed_read(view, next)?)
                    } else {
                        self.get_property_with_host(source, &next.to_string(), host)?
                    };
                    Some((key, value))
                }
                }
            },
            Family::Map | Family::Set => {
                let IteratorSource::Ordinary(source) = source else {
                    return Err(Fault::new("Invalid native Map/Set iterator source."));
                };
                if let Some((order, key, value)) = self.next_collection_entry(source, next)? {
                    self.objects[id].iterator.as_mut().unwrap().next = order + 1;
                    Some((key, value))
                } else {
                    None
                }
            }
            Family::String => unreachable!(),
        };
        let Some((key, value)) = pair else {
            self.objects[id].iterator.as_mut().unwrap().source = None;
            return Ok(None);
        };
        Ok(Some(match mode {
            Mode::Keys => key,
            Mode::Values if family == Family::Set => key,
            Mode::Values => value,
            Mode::Entries => {
                let value = if family == Family::Set {
                    self.copy_value(&key)?
                } else {
                    value
                };
                self.array_from_values(&[key, value])?
            }
        }))
    }
    // Keep native-source validation and indexed reads out of ordinary recursive
    // call frames. No list is copied and no allowance is reset here.
    #[inline(never)]
    pub(super) fn validate_indexed_host_iteration(
        &mut self, handle: u32, expected_owner: Option<u32>, host: &mut dyn Host,
    ) -> Result<u32, Fault> {
        let owner=host.indexed_iteration_owner(handle)
            .ok_or_else(||Fault::new("Host-object iteration is not implemented."))?;
        if expected_owner.is_some_and(|expected|expected!=owner)
            || host.native_access_owner(handle)!=Some(owner) {
            return Err(Fault::new("Native indexed iterator source owner changed or expired."));
        }
        self.validate_native_access(handle,host)?;
        Ok(owner)
    }

    #[inline(never)]
    fn new_indexed_host_iterator(&mut self, handle:u32, mode:Mode, host:&mut dyn Host)
        -> Result<usize,Fault>
    {
        let owner=self.validate_indexed_host_iteration(handle,None,host)?;
        // Array values/keys/entries read length only when next is requested.
        self.new_native_iterator_source(IteratorSource::IndexedHost {handle,owner},Family::Array,mode)
    }

    #[inline(never)]
    fn next_indexed_host_pair(
        &mut self, iterator:usize, handle:u32, owner:u32, next:usize,
        mode:Mode, host:&mut dyn Host,
    ) -> Result<Option<(Value,Value)>,Fault> {
        self.validate_indexed_host_iteration(handle,Some(owner),host)?;
        let length_value=self.host_read(handle,"length",host)?;
        // Same ToLength conversion as the existing ordinary array-like path,
        // including observable primitive conversion and Symbol rejection.
        let number=self.binary_number(&length_value,host)?;
        let length=if number.is_nan()||number<=0.0 {0.0}
            else {number.trunc().min(9007199254740991.0)};
        if length>self.limits.heap.array_length as f64 {
            return Err(Fault::new(format!("Native indexed iterator length limit reached ({}).",self.limits.heap.array_length)));
        }
        if next>=length as usize {return Ok(None);}
        self.validate_indexed_host_iteration(handle,Some(owner),host)?;
        // Commit the cursor before an indexed read can throw or reenter.
        self.objects[iterator].iterator.as_mut().unwrap().next=next+1;
        let value=if matches!(mode,Mode::Keys) {Value::Undefined}
            else {self.host_read(handle,&next.to_string(),host)?};
        Ok(Some((Value::Number(next as f64),value)))
    }

    pub(super) fn construct_collection(
        &mut self,
        kind: Kind,
        args: &[Value],
        new_target: Option<usize>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.check_native_call_stack("Function call")?;
        self.call_depth += 1;
        let result = self.construct_collection_inner(kind, args, new_target, host);
        self.call_depth -= 1;
        result
    }
    fn construct_collection_inner(
        &mut self,
        kind: Kind,
        args: &[Value],
        new_target: Option<usize>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let prototype = self.builtin_instance_prototype(new_target, |runtime| runtime.collection_roots.prototype(kind), host)?;
        let weak_count = if kind.is_weak() {
            Some(self.weak_collection_count.checked_add(1)
                .ok_or_else(|| Fault::new("Weak collection identity count overflow."))?)
        } else { None };
        let id = self.new_object(None)?;
        self.objects[id].collection = Some(Data::new(kind));
        // Native construction inhibits transient collection through iterator
        // getters/adders/close. Objects remain append-only even if these throw.
        if let Some(count) = weak_count { self.weak_collection_count = count; }
        self.objects[id].prototype = prototype;
        let source = args.first().unwrap_or(&Value::Undefined);
        if matches!(source, Value::Null | Value::Undefined) {
            return Ok(Value::Object(id));
        }
        let adder =
            self.get_property_with_host(id, if kind.is_set() { "add" } else { "set" }, host)?;
        if !matches!(adder, Value::Function(_)) {
            return Err(Fault::exception(
                "TypeError",
                "Collection adder is not callable.",
            ));
        }
        let source = self.copy_value(source)?;
        let mut iterator = self.loop_iterator(source, true, host)?;
        while let Some(value) = self.loop_next(&mut iterator, host)? {
            let completion = (|| {
                let values = if !kind.is_set() {
                    let object = self.ordinary_required(&value)?;
                    vec![
                        self.get_ordinary_property_with_host(object, "0", host)?,
                        self.get_ordinary_property_with_host(object, "1", host)?,
                    ]
                } else {
                    vec![value]
                };
                self.call_value(adder.clone(), &values, Value::Object(id), host)
            })();
            if let Err(error) = completion {
                return self.loop_close(&mut iterator, Err(error), host);
            }
        }
        Ok(Value::Object(id))
    }
    pub(super) fn collection_builtin(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.check_native_call_stack("Function call")?;
        self.call_depth += 1;
        let result = self.collection_builtin_inner(operation, args, this, host);
        self.call_depth -= 1;
        result
    }
    fn collection_builtin_inner(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match operation {
            Op::Constructor(_) => Err(Fault::exception(
                "TypeError",
                "Collection constructors require new.",
            )),
            Op::Size(kind) => self.collection_size(this, kind),
            Op::ArrayIterator(mode) => {
                match this {
                    Value::Object(source)=>self.new_native_iterator(source,Family::Array,mode),
                    Value::Host(handle)=>self.new_indexed_host_iterator(handle,mode,host),
                    _=>Err(Fault::new("Array iterator methods currently require an ordinary object or approved native indexed collection.")),
                }.map(Value::Object)
            }
            Op::Next(family) => {
                let Value::Object(id) = this else {
                    return Err(Fault::exception(
                        "TypeError",
                        "Iterator next requires an iterator receiver.",
                    ));
                };
                if self
                    .object(id)?
                    .iterator
                    .as_ref()
                    .is_none_or(|iterator| iterator.family != family)
                {
                    return Err(Fault::exception(
                        "TypeError",
                        "Iterator next has an incompatible receiver.",
                    ));
                }
                let value = self.next_native_iterator(id, host)?;
                let result = self.new_object(None)?;
                let done = value.is_none();
                self.set_property(result, "value".into(), value.unwrap_or(Value::Undefined))?;
                self.set_property(result, "done".into(), Value::Bool(done))?;
                Ok(Value::Object(result))
            }
            Op::Method(kind, method) => {
                if kind.is_weak() {
                    return self.weak_collection_method(kind, method, args, this, host);
                }
                let id = self.collection_receiver(&this, kind)?;
                let key = args.first().unwrap_or(&Value::Undefined);
                match method {
                    "set" | "add" => {
                        self.collection_put(
                            id,
                            kind,
                            key,
                            args.get(1).unwrap_or(&Value::Undefined),
                        )?;
                        Ok(this)
                    }
                    "has" | "get" => {
                        let lookup = self.collection_key(key)?;
                        let order = self.collection(id, kind)?.lookup.get(&lookup).copied();
                        if method == "has" {
                            return Ok(Value::Bool(order.is_some()));
                        }
                        if let Some(order) = order {
                            let bytes = self.collection(id, kind)?.ordered[&order].value.bytes();
                            self.work(bytes)?;
                            Ok(self.collection(id, kind)?.ordered[&order].value.clone())
                        } else {
                            Ok(Value::Undefined)
                        }
                    }
                    "delete" => self.collection_delete(id, kind, key),
                    "clear" => self.collection_clear(id, kind),
                    "keys" | "values" | "entries" => {
                        let mode = match method {
                            "keys" => Mode::Keys,
                            "values" => Mode::Values,
                            _ => Mode::Entries,
                        };
                        self.new_native_iterator(
                            id,
                            if kind == Kind::Map {
                                Family::Map
                            } else {
                                Family::Set
                            },
                            mode,
                        )
                        .map(Value::Object)
                    }
                    "forEach" => {
                        let callback = key;
                        if !matches!(callback, Value::Function(_)) {
                            return Err(Fault::exception(
                                "TypeError",
                                "Collection callback must be callable.",
                            ));
                        }
                        let receiver = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
                        let mut next = 0;
                        while let Some((order, key, value)) =
                            self.next_collection_entry(id, next)?
                        {
                            next = order + 1;
                            let value = if kind == Kind::Set {
                                self.copy_value(&key)?
                            } else {
                                value
                            };
                            self.call_value(
                                callback.clone(),
                                &[value, key, this.clone()],
                                receiver.clone(),
                                host,
                            )?;
                        }
                        Ok(Value::Undefined)
                    }
                    _ => Err(Fault::new("Unsupported collection method.")),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn new_map(runtime: &mut Runtime) -> usize {
        let id = runtime.new_object(None).unwrap();
        runtime.objects[id].collection = Some(Data::new(Kind::Map));
        id
    }
    #[test]
    fn collection_metadata_refusal_is_atomic_and_delete_clear_recover_storage() {
        let mut runtime = Runtime::new();
        let id = new_map(&mut runtime);
        let baseline = runtime.value_bytes;
        for key in 0..8 {
            runtime.collection_put(id, Kind::Map, &Value::Number(key as f64), &Value::Null).unwrap();
        }
        let before = runtime.value_bytes;
        let capacity = runtime.collection(id, Kind::Map).unwrap().storage_bytes();
        assert_eq!(before, baseline + capacity);
        let limit = runtime.limits.heap.retained_values;
        runtime.limits.heap.retained_values = before;
        let error = runtime.collection_put(id, Kind::Map, &Value::Number(8.0), &Value::Null).unwrap_err();
        assert!(error.display().contains("Collection retained storage exhausted"));
        assert_eq!(runtime.value_bytes, before);
        assert_eq!(runtime.collection_entries, 8);
        assert_eq!(runtime.collection(id, Kind::Map).unwrap().storage_bytes(), capacity);
        runtime.collection_delete(id, Kind::Map, &Value::Number(0.0)).unwrap();
        runtime.collection_put(id, Kind::Map, &Value::Number(8.0), &Value::Null).unwrap();
        assert_eq!(runtime.value_bytes, before);
        runtime.limits.heap.retained_values = limit;
        runtime.collection_put(id, Kind::Map, &Value::Number(9.0), &Value::Null).unwrap();
        assert!(runtime.value_bytes > before);
        runtime.collection_clear(id, Kind::Map).unwrap();
        assert_eq!(runtime.value_bytes, baseline);
        assert_eq!(runtime.collection_entries, 0);
    }

    #[test]
    fn collection_delete_reinsert_churn_keeps_backing_reservation_stable() {
        let mut runtime = Runtime::new();
        let id = new_map(&mut runtime);
        for key in 0..256 { runtime.collection_put(id, Kind::Map, &Value::Number(key as f64), &Value::Null).unwrap(); }
        let before = runtime.value_bytes;
        runtime.limits.heap.retained_values = before;
        for key in 0..10_000 {
            runtime.collection_delete(id, Kind::Map, &Value::Number(key as f64)).unwrap();
            runtime.collection_put(id, Kind::Map, &Value::Number((key + 256) as f64), &Value::Null).unwrap();
        }
        assert_eq!(runtime.collection_entries, 256);
        assert_eq!(runtime.value_bytes, before);
        assert!(matches!(runtime.next_collection_entry(id, 0).unwrap().unwrap().1, Value::Number(value) if value == 10_000.0));
    }

    #[test]
    fn collection_entry_and_retained_string_budgets_release_on_delete_and_clear() {
        let mut runtime = Runtime::for_test();
        let id = new_map(&mut runtime);
        let baseline = runtime.value_bytes;
        runtime
            .collection_put(
                id,
                Kind::Map,
                &Value::String("key".into()),
                &Value::String("value".into()),
            )
            .unwrap();
        let metadata = runtime.collection(id, Kind::Map).unwrap().storage_bytes();
        assert_eq!(runtime.value_bytes, baseline + metadata + 11);
        assert_eq!(runtime.collection_entries, 1);
        runtime
            .collection_put(
                id,
                Kind::Map,
                &Value::String("key".into()),
                &Value::String("v".into()),
            )
            .unwrap();
        assert_eq!(runtime.value_bytes, baseline + metadata + 7);
        runtime
            .collection_delete(id, Kind::Map, &Value::String("key".into()))
            .unwrap();
        assert_eq!(runtime.value_bytes, baseline + metadata);
        assert_eq!(runtime.collection_entries, 0);
        let text = Value::String("x".repeat(MAX_STRING_BYTES));
        runtime.collection_put(id, Kind::Map, &text, &text).unwrap();
        assert_eq!(runtime.value_bytes, baseline + metadata + 3 * MAX_STRING_BYTES);
        runtime.collection_clear(id, Kind::Map).unwrap();
        assert_eq!(runtime.value_bytes, baseline);
        assert_eq!(runtime.collection_entries, 0);
    }
    #[test]
    fn live_entry_caps_do_not_accumulate_deleted_slots() {
        let mut runtime = Runtime::for_test();
        let id = new_map(&mut runtime);
        for index in 0..MAX_ENTRIES * 2 {
            if index % 256 == 0 {
                runtime.begin_job();
            }
            runtime
                .collection_put(id, Kind::Map, &Value::Number(index as f64), &Value::Null)
                .unwrap();
            runtime
                .collection_delete(id, Kind::Map, &Value::Number(index as f64))
                .unwrap();
        }
        assert!(runtime.collection(id, Kind::Map).unwrap().lookup.is_empty());
        assert!(runtime
            .collection(id, Kind::Map)
            .unwrap()
            .ordered
            .is_empty());
        assert_eq!(runtime.collection_entries, 0);
        runtime.begin_job();
        for index in 0..MAX_ENTRIES {
            runtime
                .collection_put(id, Kind::Map, &Value::Number(index as f64), &Value::Null)
                .unwrap();
        }
        let before = runtime.value_bytes;
        assert!(runtime
            .collection_put(
                id,
                Kind::Map,
                &Value::Number(MAX_ENTRIES as f64),
                &Value::String("new".into())
            )
            .unwrap_err()
            .display()
            .contains("Collection entry limit"));
        assert_eq!(runtime.value_bytes, before);
        assert_eq!(runtime.collection_entries, MAX_ENTRIES);
        // Updating an existing key remains permitted when the collection is full.
        runtime
            .collection_put(id, Kind::Map, &Value::Number(0.0), &Value::Number(3.0))
            .unwrap();
    }
    #[test]
    fn all_collections_share_one_document_entry_cap() {
        let mut runtime = Runtime::for_test();
        runtime.limits.heap.retained_values = super::super::limits::BROWSER_HEAP.retained_values;
        for _ in 0..MAX_DOCUMENT_ENTRIES / MAX_ENTRIES {
            runtime.begin_job();
            let id = new_map(&mut runtime);
            for index in 0..MAX_ENTRIES {
                runtime
                    .collection_put(id, Kind::Map, &Value::Number(index as f64), &Value::Null)
                    .unwrap();
            }
        }
        runtime.begin_job();
        let id = new_map(&mut runtime);
        assert!(runtime
            .collection_put(id, Kind::Map, &Value::Number(0.0), &Value::Null)
            .unwrap_err()
            .display()
            .contains("Collection entry limit"));
        assert_eq!(runtime.collection_entries, MAX_DOCUMENT_ENTRIES);
    }

    #[test]
    fn map_symbol_keys_preserve_identity_updates_and_reinsertion_order() {
        let mut runtime = Runtime::for_test();
        let id = new_map(&mut runtime);
        let first = Value::Symbol(0);
        let second = Value::Symbol(1);
        for (key, value) in [
            (&first, Value::Number(10.0)),
            (&second, Value::Number(20.0)),
            (&first, Value::Number(30.0)),
        ] {
            runtime.collection_put(id, Kind::Map, key, &value).unwrap();
        }
        let (order, key, value) = runtime.next_collection_entry(id, 0).unwrap().unwrap();
        assert!(matches!(key, Value::Symbol(0)));
        assert!(matches!(value, Value::Number(30.0)));
        let (_, key, value) = runtime
            .next_collection_entry(id, order + 1)
            .unwrap()
            .unwrap();
        assert!(matches!(key, Value::Symbol(1)));
        assert!(matches!(value, Value::Number(20.0)));
        assert_eq!(runtime.collection_entries, 2);

        assert!(matches!(
            runtime.collection_delete(id, Kind::Map, &first).unwrap(),
            Value::Bool(true)
        ));
        runtime
            .collection_put(id, Kind::Map, &first, &Value::Symbol(1))
            .unwrap();
        let (order, key, _) = runtime.next_collection_entry(id, 0).unwrap().unwrap();
        assert!(matches!(key, Value::Symbol(1)));
        let (_, key, value) = runtime
            .next_collection_entry(id, order + 1)
            .unwrap()
            .unwrap();
        assert!(matches!(key, Value::Symbol(0)));
        assert!(matches!(value, Value::Symbol(1)));
    }

    #[test]
    fn set_symbol_keys_are_distinct_from_other_key_types() {
        let mut runtime = Runtime::for_test();
        let id = runtime.new_object(None).unwrap();
        runtime.objects[id].collection = Some(Data::new(Kind::Set));
        for key in [
            Value::Symbol(0),
            Value::Symbol(1),
            Value::Symbol(0),
            Value::Number(0.0),
            Value::Number(-0.0),
            Value::String("Symbol()".into()),
            Value::Undefined,
            Value::Number(f64::NAN),
            Value::Number(-f64::NAN),
        ] {
            runtime
                .collection_put(id, Kind::Set, &key, &Value::Undefined)
                .unwrap();
        }
        assert_eq!(runtime.collection(id, Kind::Set).unwrap().ordered.len(), 6);
        assert_eq!(runtime.collection_entries, 6);
        runtime
            .collection_delete(id, Kind::Set, &Value::Symbol(0))
            .unwrap();
        assert!(runtime
            .collection(id, Kind::Set)
            .unwrap()
            .lookup
            .contains_key(&Key::Symbol(1)));
        assert!(runtime
            .collection(id, Kind::Set)
            .unwrap()
            .lookup
            .contains_key(&Key::Number(0)));
        runtime.collection_clear(id, Kind::Set).unwrap();
        assert_eq!(runtime.collection_entries, 0);
    }
}
