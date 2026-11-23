//! Native Headers values for the shared request/response adapters.
//! https://fetch.spec.whatwg.org/#headers-class
//! No network I/O or script emulation occurs in this value layer.
use super::function_reachability::Edge;
use super::{Builtin, Declaration, Fault, Host, Ordinary, Reference, Runtime, Value};
pub(super) type Pair = (String, String);
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Guard {
    None,
    Response,
    Immutable,
}
#[derive(Default)]
pub(super) struct Roots {
    prototype: Option<usize>,
    iterator: Option<usize>,
}
impl Roots {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        for id in [self.prototype, self.iterator].into_iter().flatten() {
            add(Edge::Object(id));
        }
    }
}
pub(super) enum Data {
    List {
        pairs: Vec<Pair>,
        guard: Guard,
    },
    Iterator {
        source: usize,
        next: usize,
        kind: &'static str,
    },
}
impl Data {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        if let Self::Iterator { source, .. } = self {
            add(Edge::Object(*source));
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor,
    Method(&'static str),
    Next,
}
impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Method("append" | "set") => 2,
            Self::Method("get" | "has" | "delete" | "forEach") => 1,
            _ => 0,
        }
    }
}
pub(super) fn bytes(pairs: &Vec<Pair>) -> Result<usize, Fault> {
    pairs.iter().try_fold(
        pairs
            .capacity()
            .checked_mul(std::mem::size_of::<Pair>())
            .ok_or_else(|| Fault::new("Headers backing overflow."))?,
        |n, (k, v)| {
            n.checked_add(k.capacity())
                .and_then(|n| n.checked_add(v.capacity()))
                .ok_or_else(|| Fault::new("Headers payload overflow."))
        },
    )
}
fn invalid(message: &str) -> Fault {
    Fault::exception("TypeError", message)
}
impl Runtime {
    pub(super) fn initialize_headers_api(&mut self) -> Result<(), Fault> {
        let prototype = self.new_object(None)?;
        self.headers_roots.prototype = Some(prototype);
        let ctor = self.native_function(Builtin::Headers(Op::Constructor), "Headers")?;
        self.put_binding(
            self.global_environment,
            "Headers".into(),
            Some(ctor),
            Declaration::Var,
            false,
        )?;
        self.constructor_prototype("Headers", Value::Object(prototype))?;
        for name in [
            "append",
            "delete",
            "get",
            "getSetCookie",
            "has",
            "set",
            "keys",
            "values",
            "entries",
            "forEach",
        ] {
            let f = self.native_function(Builtin::Headers(Op::Method(name)), name)?;
            if name == "entries" {
                self.store_symbol_property(
                    prototype,
                    self.symbol_iterator.unwrap(),
                    f.clone(),
                    Some((true, true, true)),
                )?;
            }
            self.store_property(prototype, name.into(), f, Some((true, true, true)))?;
        }
        self.store_symbol_property(
            prototype,
            self.symbol_to_string_tag.unwrap(),
            Value::String("Headers".into()),
            Some((false, false, true)),
        )?;
        let iterator = self.new_object(None)?;
        self.objects[iterator].prototype = self
            .collection_roots
            .iterator_prototype
            .map(Ordinary::Object);
        self.headers_roots.iterator = Some(iterator);
        let next = self.native_function(Builtin::Headers(Op::Next), "next")?;
        self.store_property(iterator, "next".into(), next, Some((true, true, true)))?;
        self.store_symbol_property(
            iterator,
            self.symbol_to_string_tag.unwrap(),
            Value::String("Headers Iterator".into()),
            Some((false, false, true)),
        )?;
        Ok(())
    }
    fn headers_room(&self, additional: usize) -> Result<usize, Fault> {
        self.value_bytes
            .checked_add(additional)
            .filter(|n| *n <= self.limits.heap.retained_values)
            .ok_or_else(|| Fault::new("Headers retained storage allowance exhausted."))
    }
    /// Charge owned conversion scratch across user callbacks and unwind it on
    /// both success and failure. Nested conversions share the ordinary ledger.
    pub(super) fn headers_hold<T>(
        &mut self,
        held: usize,
        run: impl FnOnce(&mut Self) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        self.value_bytes = self.headers_room(held)?;
        let result = run(self);
        self.value_bytes = self
            .value_bytes
            .checked_sub(held)
            .ok_or_else(|| Fault::new("Headers scratch ownership mismatch."))?;
        result
    }
    fn headers_copy(&mut self, text: &str) -> Result<String, Fault> {
        self.work(text.len())?;
        self.headers_room(text.len())?;
        let mut copy = String::new();
        copy.try_reserve_exact(text.len())
            .map_err(|_| Fault::new("Could not reserve Headers text."))?;
        copy.push_str(text);
        Ok(copy)
    }
    pub(super) fn headers_byte_string(
        &mut self,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<String, Fault> {
        let primitive = self.primitive_with_hint(value, true, host)?;
        self.work(primitive.bytes())?;
        let text = match primitive {
            Value::StringUnits(units) => {
                if units.iter().any(|u| *u > 255) {
                    return Err(invalid("Headers ByteString contains a value above255."));
                }
                let mut text = String::new();
                self.headers_room(units.len().saturating_mul(2))?;
                text.try_reserve_exact(units.len().saturating_mul(2))
                    .map_err(|_| Fault::new("Could not reserve Headers ByteString."))?;
                for u in units.iter() {
                    text.push(char::from_u32(u32::from(*u)).unwrap());
                }
                text
            }
            value => value.script_text()?,
        };
        if text.chars().any(|c| u32::from(c) > 255) {
            return Err(invalid("Headers ByteString contains a value above255."));
        }
        self.headers_room(text.capacity())?;
        Ok(text)
    }
    fn headers_push(&mut self, pairs: &mut Vec<Pair>, pair: Pair) -> Result<(), Fault> {
        let need = bytes(pairs)?
            .checked_add(std::mem::size_of::<Pair>())
            .and_then(|n| n.checked_add(pair.0.capacity()))
            .and_then(|n| n.checked_add(pair.1.capacity()))
            .ok_or_else(|| Fault::new("Headers growth overflow."))?;
        self.headers_room(need)?;
        self.work(pairs.len().saturating_mul(std::mem::size_of::<Pair>()))?;
        pairs
            .try_reserve_exact(1)
            .map_err(|_| Fault::new("Could not reserve Headers entries."))?;
        pairs.push(pair);
        self.headers_room(bytes(pairs)?)?;
        Ok(())
    }
    fn headers_pair(&mut self, item: Value, host: &mut dyn Host) -> Result<Pair, Fault> {
        if Ordinary::from_value(&item).is_none() {
            return Err(invalid("Headers pair must be an iterable object."));
        }
        let method = self
            .iterator_method(&item, host)?
            .ok_or_else(|| invalid("Headers pair must be iterable."))?;
        let mut iterator = self.iterator_from_method(item, method, host)?;
        let mut first: Option<String> = None;
        let mut second: Option<String> = None;
        let mut count = 0usize;
        loop {
            let held = first
                .as_ref()
                .map_or(0, String::capacity)
                .checked_add(second.as_ref().map_or(0, String::capacity))
                .ok_or_else(|| Fault::new("Headers pair scratch overflow."))?;
            let converted = self.headers_hold(held, |runtime| {
                runtime
                    .iterator_next(&mut iterator, host)?
                    .map(|value| runtime.headers_byte_string(value, host))
                    .transpose()
            });
            match converted {
                Ok(Some(value)) => {
                    if count == 0 {
                        first = Some(value);
                    } else if count == 1 {
                        second = Some(value);
                    }
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| Fault::new("Headers pair length overflow."))?;
                }
                Ok(None) => break,
                Err(fault) => return self.iterator_close(&mut iterator, Err(fault), host),
            }
        }
        if count != 2 {
            return Err(invalid("Headers pairs require exactly two values."));
        }
        Ok((first.unwrap(), second.unwrap()))
    }
    pub(super) fn headers_input(
        &mut self,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<Vec<Pair>, Fault> {
        if matches!(value, Value::Undefined) {
            return Ok(Vec::new());
        }
        let Some(ordinary) = Ordinary::from_value(&value) else {
            return if matches!(value, Value::Host(_)) {
                Err(Fault::new(
                    "Native-host HeadersInit enumeration is unavailable.",
                ))
            } else {
                Err(invalid("HeadersInit requires an object."))
            };
        };
        if let Some(method) = self.iterator_method(&value, host)? {
            let mut iterator = self.iterator_from_method(value, method, host)?;
            let mut pairs = Vec::new();
            loop {
                let converted = self.headers_hold(bytes(&pairs)?, |runtime| {
                    runtime
                        .iterator_next(&mut iterator, host)?
                        .map(|item| runtime.headers_pair(item, host))
                        .transpose()
                });
                let pair = match converted {
                    Ok(Some(pair)) => pair,
                    Ok(None) => return Ok(pairs),
                    Err(fault) => return self.iterator_close(&mut iterator, Err(fault), host),
                };
                if let Err(fault) = self.headers_push(&mut pairs, pair) {
                    return self.iterator_close(&mut iterator, Err(fault), host);
                }
            }
        }
        let object = self.ordinary_id(ordinary)?;
        let keys = self.own_string_keys(object, false)?;
        let symbols = self.own_symbol_keys(object, false)?;
        let mut pairs = Vec::new();
        for key in keys {
            self.step()?;
            if !self.own_enumerable(object, &key)? {
                continue;
            }
            let pair = self.headers_hold(bytes(&pairs)?, |runtime| {
                let key_value = runtime.headers_copy(&key)?;
                let name = runtime.headers_byte_string(Value::String(key_value), host)?;
                let text = runtime.headers_hold(name.capacity(), |runtime| {
                    let receiver = runtime.copy_value(&value)?;
                    let v = runtime.read_reference(&Reference::Property(receiver, key), host)?;
                    runtime.headers_byte_string(v, host)
                })?;
                Ok((name, text))
            })?;
            self.headers_push(&mut pairs, pair)?;
        }
        for key in symbols {
            self.step()?;
            if self.objects[object]
                .symbol_properties
                .get(&key)
                .is_some_and(|p| p.enumerable)
            {
                return Err(invalid("Headers record keys cannot be Symbols."));
            }
        }
        Ok(pairs)
    }
    fn headers_normalize(&mut self, mut name: String, value: String) -> Result<Pair, Fault> {
        self.work(name.len().saturating_add(value.len()))?;
        if !crate::http_token::valid(&name) {
            return Err(invalid("Invalid HTTP header name."));
        }
        let value = value.trim_matches(['\t', ' ', '\r', '\n']);
        if value.bytes().any(|b| matches!(b, 0 | b'\r' | b'\n')) {
            return Err(invalid("Invalid HTTP header value."));
        }
        name.make_ascii_lowercase();
        Ok((name, self.headers_copy(value)?))
    }
    fn headers_insert(
        &mut self,
        pairs: &mut Vec<Pair>,
        pair: Pair,
        set: bool,
    ) -> Result<(), Fault> {
        self.work(bytes(pairs)?)?;
        let start = pairs.partition_point(|(name, _)| name < &pair.0);
        let end = pairs.partition_point(|(name, _)| name <= &pair.0);
        if set {
            pairs.drain(start..end);
        } else if start != end && pair.0 != "set-cookie" {
            let additional = pair
                .1
                .len()
                .checked_add(2)
                .ok_or_else(|| Fault::new("Headers value growth overflow."))?;
            self.headers_room(
                bytes(pairs)?
                    .checked_add(additional)
                    .ok_or_else(|| Fault::new("Headers value storage overflow."))?,
            )?;
            pairs[start]
                .1
                .try_reserve_exact(additional)
                .map_err(|_| Fault::new("Could not combine Headers values."))?;
            pairs[start].1.push_str(", ");
            pairs[start].1.push_str(&pair.1);
            self.headers_room(bytes(pairs)?)?;
            return Ok(());
        }
        let at = if set { start } else { end };
        self.headers_push(pairs, pair)?;
        let last = pairs.len() - 1;
        pairs[at..=last].rotate_right(1);
        Ok(())
    }
    pub(super) fn headers_construct(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let input = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        let input = self.headers_input(input, host)?;
        self.headers_from_converted(input, Guard::None)
    }
    pub(super) fn headers_from_converted(
        &mut self,
        input: Vec<Pair>,
        guard: Guard,
    ) -> Result<Value, Fault> {
        let held = bytes(&input)?;
        self.headers_hold(held, |runtime| {
            let mut pairs = Vec::new();
            // Web IDL converts the whole HeadersInit before validating headers.
            for (name, value) in input {
                let pair = runtime.headers_normalize(name, value)?;
                if guard != Guard::Response
                    || !matches!(pair.0.as_str(), "set-cookie" | "set-cookie2")
                {
                    runtime.headers_insert(&mut pairs, pair, false)?;
                }
            }
            runtime.ensure_global_intrinsics("Headers")?;
            let retained = runtime.headers_room(bytes(&pairs)?)?;
            let id = runtime.new_object(None)?;
            runtime.objects[id].headers = Some(Data::List { pairs, guard });
            runtime.objects[id].prototype = runtime.headers_roots.prototype.map(Ordinary::Object);
            runtime.value_bytes = retained;
            Ok(Value::Object(id))
        })
    }

    pub(super) fn headers_clone_native(&mut self, id: usize) -> Result<Value, Fault> {
        self.headers_id(&Value::Object(id), false)?;
        let guard = match self.objects[id].headers.as_ref().unwrap() {
            Data::List { guard, .. } => *guard,
            _ => unreachable!(),
        };
        let mut pairs = Vec::new();
        let mut index = 0usize;
        while let Some(pair) = self.headers_pair_copy(id, index)? {
            self.headers_push(&mut pairs, pair)?;
            index += 1;
        }
        self.headers_from_converted(pairs, guard)
    }
    fn headers_id(&self, this: &Value, iterator: bool) -> Result<usize, Fault> {
        if let Value::Object(id) = this {
            let data = self.object(*id)?.headers.as_ref();
            if matches!(
                (data, iterator),
                (Some(Data::List { .. }), false) | (Some(Data::Iterator { .. }), true)
            ) {
                return Ok(*id);
            }
        }
        Err(invalid("Headers operation has an incompatible receiver."))
    }
    fn headers_pairs(&self, id: usize) -> &Vec<Pair> {
        match self.objects[id].headers.as_ref().unwrap() {
            Data::List { pairs, .. } => pairs,
            _ => unreachable!(),
        }
    }
    fn headers_pair_copy(&mut self, id: usize, index: usize) -> Result<Option<Pair>, Fault> {
        self.step()?;
        let Some((name, value)) = self.headers_pairs(id).get(index) else {
            return Ok(None);
        };
        let needed = name.len().saturating_add(value.len());
        self.work(needed)?;
        self.headers_room(needed)?;
        let (name, value) = &self.headers_pairs(id)[index];
        let mut a = String::new();
        let mut b = String::new();
        a.try_reserve_exact(name.len())
            .map_err(|_| Fault::new("Could not copy Headers key."))?;
        b.try_reserve_exact(value.len())
            .map_err(|_| Fault::new("Could not copy Headers value."))?;
        a.push_str(name);
        b.push_str(value);
        Ok(Some((a, b)))
    }
    pub(super) fn headers_builtin(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if matches!(operation, Op::Constructor) {
            return Err(invalid("Headers requires new."));
        }
        if matches!(operation, Op::Next) {
            let id = self.headers_id(&this, true)?;
            let (source, index, kind) = match self.objects[id].headers.as_ref().unwrap() {
                Data::Iterator { source, next, kind } => (*source, *next, *kind),
                _ => unreachable!(),
            };
            let pair = self.headers_pair_copy(source, index)?;
            let done = pair.is_none();
            let value = if let Some((name, value)) = pair {
                if let Some(Data::Iterator { next, .. }) = &mut self.objects[id].headers {
                    *next = next
                        .checked_add(1)
                        .ok_or_else(|| Fault::new("Headers iterator index overflow."))?;
                }
                match kind {
                    "keys" => Value::String(name),
                    "values" => Value::String(value),
                    _ => {
                        let array = self.new_object(Some(2))?;
                        self.set_property(array, "0".into(), Value::String(name))?;
                        self.set_property(array, "1".into(), Value::String(value))?;
                        Value::Object(array)
                    }
                }
            } else {
                Value::Undefined
            };
            let result = self.new_object(None)?;
            self.set_property(result, "value".into(), value)?;
            self.set_property(result, "done".into(), Value::Bool(done))?;
            return Ok(Value::Object(result));
        }
        let id = self.headers_id(&this, false)?;
        let Op::Method(name) = operation else {
            unreachable!()
        };
        if matches!(name, "entries" | "keys" | "values") {
            let iterator = self.new_object(None)?;
            self.objects[iterator].headers = Some(Data::Iterator {
                source: id,
                next: 0,
                kind: name,
            });
            self.objects[iterator].prototype = self.headers_roots.iterator.map(Ordinary::Object);
            return Ok(Value::Object(iterator));
        }
        if name == "forEach" {
            let callback = args.first().unwrap_or(&Value::Undefined);
            if !matches!(callback, Value::Function(_)) {
                return Err(invalid("Headers forEach callback must be callable."));
            }
            let callback = self.copy_value(callback)?;
            let mut index = 0usize;
            while let Some((name, value)) = self.headers_pair_copy(id, index)? {
                let receiver = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
                self.call_value(
                    callback.clone(),
                    &[Value::String(value), Value::String(name), Value::Object(id)],
                    receiver,
                    host,
                )?;
                index = index
                    .checked_add(1)
                    .ok_or_else(|| Fault::new("Headers callback index overflow."))?;
            }
            return Ok(Value::Undefined);
        }
        if name == "getSetCookie" {
            self.work(self.headers_pairs(id).len())?;
            let count = self
                .headers_pairs(id)
                .iter()
                .filter(|(k, _)| k == "set-cookie")
                .count();
            let array = self.new_object(Some(count))?;
            let mut out = 0usize;
            let mut index = 0usize;
            while let Some((k, v)) = self.headers_pair_copy(id, index)? {
                if k == "set-cookie" {
                    self.set_property(array, out.to_string(), Value::String(v))?;
                    out += 1;
                }
                index += 1;
            }
            return Ok(Value::Object(array));
        }
        if args.len() < operation.arity() {
            return Err(invalid("Missing required Headers argument."));
        }
        let first = self.copy_value(&args[0])?;
        let key = self.headers_byte_string(first, host)?;
        let value = if matches!(name, "append" | "set") {
            self.headers_hold(key.capacity(), |runtime| {
                let value = runtime.copy_value(&args[1])?;
                runtime.headers_byte_string(value, host)
            })?
        } else {
            String::new()
        };
        let (key, value) = self.headers_normalize(key, value)?;
        self.work(bytes(self.headers_pairs(id))?)?;
        if name == "has" {
            return Ok(Value::Bool(
                self.headers_pairs(id).iter().any(|(k, _)| k == &key),
            ));
        }
        if name == "get" {
            let mut found = false;
            let mut text = String::new();
            let mut index = 0usize;
            while let Some((k, v)) = self.headers_pair_copy(id, index)? {
                if k == key {
                    if found {
                        self.append_string(&mut text, ", ")?;
                    }
                    self.append_string(&mut text, &v)?;
                    found = true;
                }
                index += 1;
            }
            return Ok(if found {
                Value::String(text)
            } else {
                Value::Null
            });
        }
        if matches!(
            self.objects[id].headers,
            Some(Data::List {
                guard: Guard::Immutable,
                ..
            })
        ) {
            return Err(invalid("Headers are immutable."));
        }
        if matches!(
            self.objects[id].headers,
            Some(Data::List {
                guard: Guard::Response,
                ..
            })
        ) && matches!(key.as_str(), "set-cookie" | "set-cookie2")
        {
            return Ok(Value::Undefined);
        }
        let old = bytes(self.headers_pairs(id))?;
        self.headers_room(old)?;
        let mut pairs = Vec::new();
        let mut index = 0usize;
        while let Some(pair) = self.headers_pair_copy(id, index)? {
            self.headers_push(&mut pairs, pair)?;
            index += 1;
        }
        if name == "delete" {
            pairs.retain(|(k, _)| k != &key);
        } else {
            self.headers_insert(&mut pairs, (key, value), name == "set")?;
        }
        let new = bytes(&pairs)?;
        let total = self
            .value_bytes
            .checked_sub(old)
            .and_then(|n| n.checked_add(new))
            .filter(|n| *n <= self.limits.heap.retained_values)
            .ok_or_else(|| Fault::new("Headers retained replacement allowance exhausted."))?;
        if let Some(Data::List { pairs: current, .. }) = &mut self.objects[id].headers {
            *current = pairs;
        }
        self.value_bytes = total;
        Ok(Value::Undefined)
    }
}

#[cfg(test)]
#[path = "headers_api_tests.rs"]
mod tests;
