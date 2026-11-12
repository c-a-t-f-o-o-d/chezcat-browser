//! Original insertion-ordered Map/Set and native iterator support. Hash lookup
//! preserves identity/SameValueZero; monotonically ordered live entries keep
//! deletion/reinsertion observable to active iterators without tombstone growth.
//! Collection entries share the runtime's retained-string and execution budgets.

use super::{
    Builtin, Declaration, Fault, Host, NativeGetter, Ordinary, Runtime, Value, MAX_CALL_DEPTH,
    MAX_STRING_BYTES,
};
use std::collections::{BTreeMap, HashMap};

const MAX_ENTRIES: usize = 4096;
const MAX_DOCUMENT_ENTRIES: usize = 16384;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Kind {
    Map,
    Set,
}
impl Kind {
    fn name(self) -> &'static str {
        if self == Self::Map {
            "Map"
        } else {
            "Set"
        }
    }
}
#[derive(Eq, PartialEq, Hash)]
enum Key {
    Undefined,
    Null,
    Bool(bool),
    Number(u64),
    String(String),
    Symbol(usize),
    Object(usize),
    Function(usize),
    Host(u32),
}
struct Entry {
    key: Value,
    value: Value,
}
pub(super) struct Data {
    pub kind: Kind,
    lookup: HashMap<Key, usize>,
    ordered: BTreeMap<usize, Entry>,
    sequence: usize,
}
impl Data {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            lookup: HashMap::new(),
            ordered: BTreeMap::new(),
            sequence: 0,
        }
    }
    pub(super) fn tag(&self) -> &'static str {
        self.kind.name()
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
pub(super) struct IteratorData {
    pub family: Family,
    source: Option<usize>,
    next: usize,
    mode: Mode,
    text: Option<String>,
}
impl IteratorData {
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
    array_iterator: Option<usize>,
    map_iterator: Option<usize>,
    set_iterator: Option<usize>,
    string_iterator: Option<usize>,
    array_values: Option<usize>,
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
                0,
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
        let iterator_prototype = self.new_object(None)?;
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
        for (name, mode) in [
            ("keys", Mode::Keys),
            ("values", Mode::Values),
            ("entries", Mode::Entries),
        ] {
            self.prototype_method(array, name, Builtin::Collection(Op::ArrayIterator(mode)))?;
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
            Value::String(value) => Key::String(value.clone()),
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
        let value = if kind == Kind::Set {
            Value::Undefined
        } else {
            self.copy_value(value)?
        };
        let key_bytes = if existing.is_none() {
            key.bytes().saturating_mul(2)
        } else {
            0
        };
        let retained = self.value_bytes - old_bytes + key_bytes + value.bytes();
        if key.bytes() > MAX_STRING_BYTES || value.bytes() > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
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
            if self.collection(id, kind)?.ordered.len() >= MAX_ENTRIES
                || self.collection_entries >= MAX_DOCUMENT_ENTRIES
            {
                return Err(Fault::new(
                    "Collection entry limit reached (4096 per collection / 16384 per document).",
                ));
            }
            let key = match key {
                Value::Number(number) if *number == 0.0 => Value::Number(0.0),
                _ => self.copy_value(key)?,
            };
            let data = self.objects[id].collection.as_mut().unwrap();
            let order = data.sequence;
            data.sequence = order
                .checked_add(1)
                .ok_or_else(|| Fault::new("Collection insertion order limit reached."))?;
            data.lookup.insert(lookup, order);
            data.ordered.insert(order, Entry { key, value });
            self.collection_entries += 1;
        }
        self.value_bytes = retained;
        Ok(())
    }
    fn collection_delete(&mut self, id: usize, kind: Kind, key: &Value) -> Result<Value, Fault> {
        let key = self.collection_key(key)?;
        self.collection(id, kind)?;
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
        self.work(length.saturating_mul(8))?;
        let data = self.objects[id].collection.as_mut().unwrap();
        self.value_bytes -= data
            .ordered
            .values()
            .map(|entry| entry.key.bytes() * 2 + entry.value.bytes())
            .sum::<usize>();
        self.collection_entries -= length;
        data.lookup.clear();
        data.ordered.clear();
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

    pub(super) fn new_string_iterator(&mut self, text: String) -> Result<usize, Fault> {
        if text.len() > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
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
            Family::Array => {
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
            Family::Map | Family::Set => {
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
    pub(super) fn construct_collection(
        &mut self,
        kind: Kind,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = self.construct_collection_inner(kind, args, host);
        self.call_depth -= 1;
        result
    }
    fn construct_collection_inner(
        &mut self,
        kind: Kind,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let id = self.new_object(None)?;
        self.objects[id].collection = Some(Data::new(kind));
        self.objects[id].prototype = if kind == Kind::Map {
            self.collection_roots.map
        } else {
            self.collection_roots.set
        }
        .map(Ordinary::Object);
        let source = args.first().unwrap_or(&Value::Undefined);
        if matches!(source, Value::Null | Value::Undefined) {
            return Ok(Value::Object(id));
        }
        let adder =
            self.get_property_with_host(id, if kind == Kind::Map { "set" } else { "add" }, host)?;
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
                let values = if kind == Kind::Map {
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
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
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
                let Value::Object(source) = this else {
                    return Err(Fault::new(
                        "Array iterator methods currently require an ordinary object.",
                    ));
                };
                self.new_native_iterator(source, Family::Array, mode)
                    .map(Value::Object)
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
        assert_eq!(runtime.value_bytes, baseline + 11);
        assert_eq!(runtime.collection_entries, 1);
        runtime
            .collection_put(
                id,
                Kind::Map,
                &Value::String("key".into()),
                &Value::String("v".into()),
            )
            .unwrap();
        assert_eq!(runtime.value_bytes, baseline + 7);
        runtime
            .collection_delete(id, Kind::Map, &Value::String("key".into()))
            .unwrap();
        assert_eq!(runtime.value_bytes, baseline);
        assert_eq!(runtime.collection_entries, 0);
        let text = Value::String("x".repeat(MAX_STRING_BYTES));
        runtime.collection_put(id, Kind::Map, &text, &text).unwrap();
        assert_eq!(runtime.value_bytes, baseline + 3 * MAX_STRING_BYTES);
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
