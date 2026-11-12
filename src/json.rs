//! Original bounded JSON support. Parsing and serialization use explicit frame
//! stacks and the enclosing Runtime's object, retained-value and work budgets.
//! Callback walks retain source snapshots and real holders without recursive
//! native traversal. Lone UTF-16 surrogates remain an explicit limitation.

use super::{
    number_text, property_index, strict_equal, Fault, Host, Ordinary, PropertyKey, Reference,
    Runtime, Value, MAX_OBJECT_PROPERTIES, MAX_PARSE_DEPTH, MAX_RETAINED_VALUES, MAX_STRING_BYTES,
};
use std::collections::{BTreeMap, BTreeSet};

struct ParseSnapshot {
    value: Value,
    start: usize,
    end: usize,
}

type Snapshots = BTreeMap<(usize, String), ParseSnapshot>;

struct Reader<'a> {
    source: &'a str,
    at: usize,
    snapshots: Option<Snapshots>,
    snapshot_bytes: usize,
}

struct ParseFrame {
    id: usize,
    array: bool,
    next: usize,
    entry: bool,
    allow_end: bool,
}

impl Reader<'_> {
    fn snapshot(
        &mut self,
        runtime: &mut Runtime,
        holder: usize,
        key: &str,
        value: &Value,
        start: usize,
    ) -> Result<(), Fault> {
        let Some(snapshots) = self.snapshots.as_mut() else {
            return Ok(());
        };
        let key = (holder, key.to_owned());
        let old = snapshots.get(&key).map_or(0, |old| {
            std::mem::size_of::<((usize, String), ParseSnapshot)>()
                + 3 * std::mem::size_of::<usize>()
                + key.1.len()
                + old.value.bytes()
        });
        let bytes = std::mem::size_of::<((usize, String), ParseSnapshot)>()
            + 3 * std::mem::size_of::<usize>()
            + key.1.len()
            + value.bytes();
        let retained = self
            .snapshot_bytes
            .saturating_sub(old)
            .saturating_add(bytes);
        if retained > MAX_RETAINED_VALUES {
            return Err(Fault::new("JSON reviver snapshot limit reached (2 MiB)."));
        }
        runtime.work(bytes)?;
        let value = runtime.copy_value(value)?;
        snapshots.insert(
            key,
            ParseSnapshot {
                value,
                start,
                end: self.at,
            },
        );
        self.snapshot_bytes = retained;
        Ok(())
    }

    fn byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn syntax(&self, message: &str) -> Fault {
        Fault::exception(
            "SyntaxError",
            format!("JSON at byte {}: {message}", self.at),
        )
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.byte() == Some(byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn hex_unit(&mut self) -> Result<u16, Fault> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = match self.byte() {
                Some(b'0'..=b'9') => self.byte().unwrap() - b'0',
                Some(b'a'..=b'f') => self.byte().unwrap() - b'a' + 10,
                Some(b'A'..=b'F') => self.byte().unwrap() - b'A' + 10,
                _ => return Err(self.syntax("Expected four hexadecimal escape digits.")),
            };
            self.at += 1;
            value = value * 16 + digit as u16;
        }
        Ok(value)
    }

    fn string(&mut self, runtime: &mut Runtime) -> Result<String, Fault> {
        runtime.step()?;
        if !self.take(b'"') {
            return Err(self.syntax("Expected a quoted string."));
        }
        let mut output = String::new();
        loop {
            let ch = match self.byte() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(output);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let escaped = self
                        .byte()
                        .ok_or_else(|| self.syntax("Unterminated escape."))?;
                    self.at += 1;
                    match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let high = self.hex_unit()?;
                            let code = if (0xd800..=0xdbff).contains(&high) {
                                if !self.source[self.at..].starts_with("\\u") {
                                    return Err(Fault::new(
                                        "JSON lone UTF-16 surrogates are not implemented.",
                                    ));
                                }
                                self.at += 2;
                                let low = self.hex_unit()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err(Fault::new(
                                        "JSON lone UTF-16 surrogates are not implemented.",
                                    ));
                                }
                                0x10000 + ((high as u32 - 0xd800) << 10) + low as u32 - 0xdc00
                            } else if (0xdc00..=0xdfff).contains(&high) {
                                return Err(Fault::new(
                                    "JSON lone UTF-16 surrogates are not implemented.",
                                ));
                            } else {
                                high as u32
                            };
                            char::from_u32(code).unwrap()
                        }
                        _ => return Err(self.syntax("Invalid string escape.")),
                    }
                }
                Some(0..=31) => return Err(self.syntax("Unescaped control character in string.")),
                Some(_) => {
                    let ch = self.source[self.at..].chars().next().unwrap();
                    self.at += ch.len_utf8();
                    ch
                }
                None => return Err(self.syntax("Unterminated string.")),
            };
            runtime.append_string(&mut output, ch.encode_utf8(&mut [0; 4]))?;
        }
    }

    fn number(&mut self) -> Result<Value, Fault> {
        let start = self.at;
        self.take(b'-');
        match self.byte() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => {
                self.at += 1;
                while matches!(self.byte(), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            _ => return Err(self.syntax("Expected a JSON number.")),
        }
        if self.take(b'.') {
            let digits = self.at;
            while matches!(self.byte(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
            if digits == self.at {
                return Err(self.syntax("A decimal point needs following digits."));
            }
        }
        if matches!(self.byte(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.byte(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            let digits = self.at;
            while matches!(self.byte(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
            if digits == self.at {
                return Err(self.syntax("An exponent needs digits."));
            }
        }
        self.source[start..self.at]
            .parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.syntax("Invalid JSON number."))
    }

    fn value(
        &mut self,
        runtime: &mut Runtime,
        depth: usize,
    ) -> Result<(Value, Option<ParseFrame>), Fault> {
        runtime.step()?;
        self.space();
        let value = match self.byte() {
            Some(b'[' | b'{') => {
                runtime.json_depth(depth + 1)?;
                let array = self.take(b'[');
                if !array {
                    self.at += 1;
                }
                let id = runtime.new_object(if array { Some(0) } else { None })?;
                return Ok((
                    Value::Object(id),
                    Some(ParseFrame {
                        id,
                        array,
                        next: 0,
                        entry: true,
                        allow_end: true,
                    }),
                ));
            }
            Some(b'"') => Value::String(self.string(runtime)?),
            Some(b'-' | b'0'..=b'9') => self.number()?,
            Some(b'n') if self.source[self.at..].starts_with("null") => {
                self.at += 4;
                Value::Null
            }
            Some(b't') if self.source[self.at..].starts_with("true") => {
                self.at += 4;
                Value::Bool(true)
            }
            Some(b'f') if self.source[self.at..].starts_with("false") => {
                self.at += 5;
                Value::Bool(false)
            }
            _ => return Err(self.syntax("Expected a JSON value.")),
        };
        Ok((value, None))
    }

    fn parse(&mut self, runtime: &mut Runtime) -> Result<Value, Fault> {
        self.space();
        let start = self.at;
        let (root, frame) = self.value(runtime, 0)?;
        self.snapshot(runtime, usize::MAX, "", &root, start)?;
        let mut frames = Vec::new();
        frames.extend(frame);
        while let Some(mut frame) = frames.pop() {
            runtime.step()?;
            self.space();
            let end = if frame.array { b']' } else { b'}' };
            if frame.entry {
                if frame.allow_end && self.take(end) {
                    continue;
                }
                let key = if frame.array {
                    let key = frame.next.to_string();
                    frame.next += 1;
                    key
                } else {
                    let key = self.string(runtime)?;
                    runtime.property_name(&key)?;
                    self.space();
                    if !self.take(b':') {
                        return Err(self.syntax("Expected a colon after the property name."));
                    }
                    key
                };
                self.space();
                let start = self.at;
                let (value, child) = self.value(runtime, frames.len() + 1)?;
                self.snapshot(runtime, frame.id, &key, &value, start)?;
                runtime.set_property(frame.id, key, value)?;
                frame.entry = false;
                frames.push(frame);
                frames.extend(child);
            } else if self.take(end) {
                continue;
            } else if self.take(b',') {
                frame.entry = true;
                frame.allow_end = false;
                frames.push(frame);
            } else {
                return Err(self.syntax("Expected a comma or the end of the container."));
            }
        }
        self.space();
        if self.at != self.source.len() {
            return Err(self.syntax("Unexpected text after the JSON value."));
        }
        Ok(root)
    }
}

enum Entries {
    Array { next: usize, length: usize },
    Object(std::vec::IntoIter<String>),
    Listed { next: usize },
}

struct WriteFrame {
    id: usize,
    entries: Entries,
    wrote: bool,
}

struct ReviveFrame {
    holder: Value,
    key: String,
    value: Value,
    context: usize,
    entries: Option<Entries>,
    snapshot_matches: bool,
}

fn json_same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => {
            a.is_nan() && b.is_nan() || a.to_bits() == b.to_bits()
        }
        _ => strict_equal(left, right),
    }
}

impl Runtime {
    fn json_depth(&self, depth: usize) -> Result<(), Fault> {
        if self.eval_depth.saturating_add(depth) > MAX_PARSE_DEPTH {
            Err(Fault::new(
                "JSON combined evaluation/nesting limit reached (64).",
            ))
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(super) fn json_parse(&mut self, args: &[Value]) -> Result<Value, Fault> {
        // Host-free parsing is retained for the original internal allocation
        // assertions. The public builtin below performs observable conversion
        // and callback processing with its caller's host.
        let value = args.first().unwrap_or(&Value::Undefined);
        self.work(value.bytes())?;
        let source = value.script_text()?;
        self.json_parse_source(&source, false)
            .map(|(value, _)| value)
    }

    fn json_parse_source(
        &mut self,
        source: &str,
        capture: bool,
    ) -> Result<(Value, Snapshots), Fault> {
        if source.len() > MAX_STRING_BYTES {
            return Err(Fault::new("JSON source limit reached (128 KiB)."));
        }
        self.work(source.len())?;
        let checkpoint = (self.objects.len(), self.properties, self.value_bytes);
        let mut reader = Reader {
            source,
            at: 0,
            snapshots: capture.then(BTreeMap::new),
            snapshot_bytes: 0,
        };
        let result = reader.parse(self);
        if result.is_err() {
            // Invalid JSON cannot publish any of these fresh identities. Undo
            // only its allocations; spent execution/copy budgets stay spent.
            self.objects.truncate(checkpoint.0);
            self.properties = checkpoint.1;
            self.value_bytes = checkpoint.2;
        }
        result.map(|value| (value, reader.snapshots.unwrap_or_default()))
    }

    pub(super) fn json_parse_builtin(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let source = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        let source = self
            .primitive_with_hint(source, true, host)?
            .script_text()?;
        let callback = args
            .get(1)
            .filter(|value| matches!(value, Value::Function(_)));
        let (root, snapshots) = self.json_parse_source(&source, callback.is_some())?;
        let Some(callback) = callback else {
            return Ok(root);
        };
        let holder = self.new_object(None)?;
        self.store_property(holder, "".into(), root, Some((true, true, true)))?;
        let first = self.json_revive_frame(
            Value::Object(holder),
            String::new(),
            snapshots.get(&(usize::MAX, String::new())),
            &source,
            host,
        )?;
        let mut frames = vec![first];
        loop {
            self.step()?;
            let frame = frames.last_mut().unwrap();
            let key = match frame.entries.as_mut() {
                Some(Entries::Array { next, length }) if *next < *length => {
                    let key = next.to_string();
                    *next += 1;
                    Some(key)
                }
                Some(Entries::Object(keys)) => keys.next(),
                _ => None,
            };
            if let Some(key) = key {
                let holder = frame.value.clone();
                let id = self.ordinary_id(self.ordinary_required(&holder)?)?;
                let snapshot = if frame.snapshot_matches {
                    self.work(key.len())?;
                    snapshots.get(&(id, key.clone()))
                } else {
                    None
                };
                self.json_depth(frames.len() + 1)?;
                let child = self.json_revive_frame(holder, key, snapshot, &source, host)?;
                frames.push(child);
                continue;
            }
            let frame = frames.pop().unwrap();
            let key_value = Value::String(frame.key.clone());
            let result = self.call_value(
                callback.clone(),
                &[key_value, frame.value, Value::Object(frame.context)],
                frame.holder.clone(),
                host,
            )?;
            if frames.is_empty() {
                return Ok(result);
            }
            self.json_revive_property(&frame.holder, frame.key, result, host)?;
        }
    }

    fn json_revive_frame(
        &mut self,
        holder: Value,
        key: String,
        snapshot: Option<&ParseSnapshot>,
        source: &str,
        host: &mut dyn Host,
    ) -> Result<ReviveFrame, Fault> {
        self.work(key.len())?;
        let value = self.read_reference(&Reference::Property(holder.clone(), key.clone()), host)?;
        if matches!(value, Value::Host(_)) {
            return Err(Fault::new(
                "JSON reviver traversal of native host objects is not implemented.",
            ));
        }
        let context = self.new_object(None)?;
        let snapshot_matches =
            snapshot.is_some_and(|snapshot| json_same_value(&snapshot.value, &value));
        if snapshot_matches && !matches!(value, Value::Object(_) | Value::Function(_)) {
            let snapshot = snapshot.unwrap();
            let text = &source[snapshot.start..snapshot.end];
            self.work(text.len())?;
            self.store_property(
                context,
                "source".into(),
                Value::String(text.into()),
                Some((true, true, true)),
            )?;
        }
        let entries = if let Some(object) = Ordinary::from_value(&value) {
            let id = self.ordinary_id(object)?;
            Some(if let Some(length) = self.object(id)?.length {
                Entries::Array { next: 0, length }
            } else {
                Entries::Object(self.enumerable_keys(id)?.into_iter())
            })
        } else {
            None
        };
        Ok(ReviveFrame {
            holder,
            key,
            value,
            context,
            entries,
            snapshot_matches,
        })
    }

    fn json_revive_property(
        &mut self,
        holder: &Value,
        key: String,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if matches!(value, Value::Undefined) {
            let previous = self.strict;
            self.strict = false;
            let result = self.delete_reference(Reference::Property(holder.clone(), key), host);
            self.strict = previous;
            return result.map(|_| ());
        }
        let id = self.ordinary_id(self.ordinary_required(holder)?)?;
        // InternalizeJSONProperty ignores a false definition result. Check
        // descriptor rejection before the throwing general-purpose helper;
        // abrupt typed-value conversions must still propagate unchanged.
        if let Some(index) = self.typed_numeric_key(id, &key)? {
            if !self.typed_index_valid(id, index) {
                return Ok(());
            }
        } else {
            if self.objects[id]
                .properties
                .get(&key)
                .is_some_and(|property| !property.configurable)
                || !self.objects[id].properties.contains_key(&key) && !self.objects[id].extensible
            {
                return Ok(());
            }
            if let Some(length) = self.objects[id].length {
                if key == "length"
                    || property_index(&key).is_some_and(|index| index >= length)
                        && !self.objects[id].length_writable
                {
                    return Ok(());
                }
            }
        }
        self.create_data_property(holder, PropertyKey::String(key), value, host)
    }

    fn json_quote(&mut self, output: &mut String, text: &str) -> Result<(), Fault> {
        self.append_string(output, "\"")?;
        for ch in text.chars() {
            let escaped = match ch {
                '"' => Some("\\\""),
                '\\' => Some("\\\\"),
                '\u{8}' => Some("\\b"),
                '\u{c}' => Some("\\f"),
                '\n' => Some("\\n"),
                '\r' => Some("\\r"),
                '\t' => Some("\\t"),
                _ => None,
            };
            if let Some(escaped) = escaped {
                self.append_string(output, escaped)?;
            } else if ch < '\u{20}' {
                self.append_string(output, &format!("\\u{:04x}", ch as u32))?;
            } else {
                self.append_string(output, ch.encode_utf8(&mut [0; 4]))?;
            }
        }
        self.append_string(output, "\"")
    }

    fn json_indent(&mut self, output: &mut String, gap: &str, depth: usize) -> Result<(), Fault> {
        if !gap.is_empty() {
            self.append_string(output, "\n")?;
            for _ in 0..depth {
                self.append_string(output, gap)?;
            }
        }
        Ok(())
    }

    fn json_write_value(
        &mut self,
        value: Value,
        output: &mut String,
        frames: &mut Vec<WriteFrame>,
        listed: bool,
    ) -> Result<(), Fault> {
        self.step()?;
        match value {
            Value::Null | Value::Undefined | Value::Function(_) | Value::Symbol(_) => {
                self.append_string(output, "null")
            }
            Value::Bool(value) => self.append_string(output, if value { "true" } else { "false" }),
            Value::Number(value) => {
                let text = if value.is_finite() {
                    number_text(value)
                } else {
                    "null".into()
                };
                self.append_string(output, &text)
            }
            Value::String(value) => self.json_quote(output, &value),
            Value::Host(_) => Err(Fault::new(
                "JSON serialization of host objects is not implemented.",
            )),
            Value::Object(id) => {
                self.json_depth(frames.len() + 1)?;
                if frames.iter().any(|frame| frame.id == id) {
                    return Err(Fault::exception(
                        "TypeError",
                        "JSON.stringify cannot serialize a circular object.",
                    ));
                }
                let length = self.object(id)?.length;
                let entries = if let Some(length) = length {
                    Entries::Array { next: 0, length }
                } else if listed {
                    Entries::Listed { next: 0 }
                } else {
                    // JSON visits enumerable string keys only. Symbol-keyed
                    // properties are not read, even when their values cycle.
                    Entries::Object(self.enumerable_keys(id)?.into_iter())
                };
                self.append_string(output, if length.is_some() { "[" } else { "{" })?;
                frames.push(WriteFrame {
                    id,
                    entries,
                    wrote: false,
                });
                Ok(())
            }
        }
    }

    fn json_property_value(
        &mut self,
        holder: &Value,
        key: &str,
        replacer: Option<&Value>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.work(key.len())?;
        let mut value =
            self.read_reference(&Reference::Property(holder.clone(), key.into()), host)?;
        if matches!(value, Value::Host(_)) {
            return Err(Fault::new(
                "JSON serialization of host objects is not implemented.",
            ));
        }
        if matches!(value, Value::Object(_) | Value::Function(_)) {
            let hook =
                self.read_reference(&Reference::Property(value.clone(), "toJSON".into()), host)?;
            if matches!(hook, Value::Function(_)) {
                value = self.call_value(hook, &[Value::String(key.into())], value, host)?;
            }
        }
        if let Some(replacer) = replacer {
            value = self.call_value(
                replacer.clone(),
                &[Value::String(key.into()), value],
                holder.clone(),
                host,
            )?;
        }
        // Hooks run before wrapper conversion. A replacement object's toJSON
        // is not called a second time for the same property.
        match self.wrapped_data(&value) {
            Some(super::primitive_wrappers::Data::Number(_)) => {
                Ok(Value::Number(self.binary_number(&value, host)?))
            }
            Some(super::primitive_wrappers::Data::Boolean(value)) => Ok(Value::Bool(*value)),
            Some(super::primitive_wrappers::Data::String { .. }) => {
                let primitive = self.primitive_with_hint(value, true, host)?;
                self.checked_value(Value::String(primitive.script_text()?))
            }
            _ => Ok(value),
        }
    }

    fn json_property_list(
        &mut self,
        value: Option<&Value>,
        host: &mut dyn Host,
    ) -> Result<Option<Vec<String>>, Fault> {
        let Some(Value::Object(id)) = value else {
            return Ok(None);
        };
        let Some(length) = self.object(*id)?.length else {
            return Ok(None);
        };
        let mut keys = Vec::new();
        let mut seen = BTreeSet::new();
        let mut bytes = 0usize;
        for index in 0..length {
            self.step()?;
            let value = self.get_property_with_host(*id, &index.to_string(), host)?;
            let key = match value {
                Value::String(text) => Some(text),
                Value::Number(number) => Some(number_text(number)),
                value
                    if matches!(
                        self.wrapped_data(&value),
                        Some(
                            super::primitive_wrappers::Data::Number(_)
                                | super::primitive_wrappers::Data::String { .. }
                        )
                    ) =>
                {
                    Some(self.primitive_with_hint(value, true, host)?.script_text()?)
                }
                _ => None,
            };
            if let Some(key) = key {
                self.property_name(&key)?;
                self.work(key.len())?;
                if !seen.contains(&key) {
                    if keys.len() == MAX_OBJECT_PROPERTIES {
                        return Err(Fault::new("JSON property list limit reached (4096)."));
                    }
                    let added = key.len() * 2
                        + 2 * std::mem::size_of::<String>()
                        + 3 * std::mem::size_of::<usize>();
                    bytes = bytes.saturating_add(added);
                    if bytes > MAX_RETAINED_VALUES {
                        return Err(Fault::new(
                            "JSON property list snapshot limit reached (2 MiB).",
                        ));
                    }
                    self.work(added)?;
                    seen.insert(key.clone());
                    keys.push(key);
                }
            }
        }
        Ok(Some(keys))
    }

    pub(super) fn json_stringify(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = args.first().unwrap_or(&Value::Undefined);
        let replacer = args
            .get(1)
            .filter(|value| matches!(value, Value::Function(_)));
        let property_list = self.json_property_list(args.get(1), host)?;
        let converted_space = match args.get(2) {
            Some(value)
                if matches!(
                    self.wrapped_data(value),
                    Some(super::primitive_wrappers::Data::Number(_))
                ) =>
            {
                Some(Value::Number(self.binary_number(value, host)?))
            }
            Some(value)
                if matches!(
                    self.wrapped_data(value),
                    Some(super::primitive_wrappers::Data::String { .. })
                ) =>
            {
                let value = self.copy_value(value)?;
                let value = self.primitive_with_hint(value, true, host)?;
                Some(self.checked_value(Value::String(value.script_text()?))?)
            }
            _ => None,
        };
        let gap = match converted_space.as_ref().or(args.get(2)) {
            Some(Value::Number(value)) => " ".repeat(if value.is_nan() {
                0
            } else {
                value.clamp(0.0, 10.0) as usize
            }),
            Some(Value::String(value)) => {
                let mut units = 0;
                let mut gap = String::new();
                for ch in value.chars() {
                    if units == 10 {
                        break;
                    }
                    units += ch.len_utf16();
                    if units > 10 {
                        return Err(Fault::new("JSON indentation ending in a lone UTF-16 surrogate is not implemented."));
                    }
                    gap.push(ch);
                }
                self.work(gap.len())?;
                gap
            }
            _ => String::new(),
        };
        let holder = self.new_object(None)?;
        let value = self.copy_value(value)?;
        self.store_property(holder, "".into(), value, Some((true, true, true)))?;
        let value = self.json_property_value(&Value::Object(holder), "", replacer, host)?;
        if matches!(
            value,
            Value::Undefined | Value::Function(_) | Value::Symbol(_)
        ) {
            return Ok(Value::Undefined);
        }
        let mut output = String::new();
        let mut frames = Vec::new();
        self.json_write_value(value, &mut output, &mut frames, property_list.is_some())?;
        while let Some(mut frame) = frames.pop() {
            self.step()?;
            let array = matches!(frame.entries, Entries::Array { .. });
            let key = match &mut frame.entries {
                Entries::Array { next, length } => {
                    if *next == *length {
                        None
                    } else {
                        let key = next.to_string();
                        *next += 1;
                        Some(key)
                    }
                }
                Entries::Object(keys) => keys.next(),
                Entries::Listed { next } => {
                    let keys = property_list.as_ref().unwrap();
                    if let Some(key) = keys.get(*next) {
                        self.work(key.len())?;
                        *next += 1;
                        Some(key.clone())
                    } else {
                        None
                    }
                }
            };
            if let Some(key) = key {
                let value =
                    self.json_property_value(&Value::Object(frame.id), &key, replacer, host)?;
                if !array
                    && matches!(
                        value,
                        Value::Undefined | Value::Function(_) | Value::Symbol(_)
                    )
                {
                    frames.push(frame);
                    continue;
                }
                if frame.wrote {
                    self.append_string(&mut output, ",")?;
                }
                frame.wrote = true;
                self.json_indent(&mut output, &gap, frames.len() + 1)?;
                if !array {
                    self.json_quote(&mut output, &key)?;
                    self.append_string(&mut output, if gap.is_empty() { ":" } else { ": " })?;
                }
                frames.push(frame);
                self.json_write_value(value, &mut output, &mut frames, property_list.is_some())?;
            } else {
                if frame.wrote {
                    self.json_indent(&mut output, &gap, frames.len())?;
                }
                self.append_string(&mut output, if array { "]" } else { "}" })?;
            }
        }
        self.checked_value(Value::String(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::javascript::Host;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No host object is exposed in this test.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No host object is exposed in this test.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No host object is exposed in this test.".into())
        }
    }

    fn callback_succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn json_callbacks_reviver_postorder_holders_deletion_and_root() {
        callback_succeeds(
            r#"
var order=[],root;
var value=JSON.parse('{"b":[1,2],"a":3}',function(key,value,context){
if(arguments.length!==3||Object.getPrototypeOf(context)!==Object.prototype)throw 'context';
order.push(key+':'+(Array.isArray(this)?'array':'object'));
if(key==='0')return undefined;if(key==='1')return value+5;if(key==='a')return undefined;
if(key==='')root=this;return value;});
if(order.join('|')!=='0:array|1:array|b:object|a:object|:object')throw 'postorder';
if(root['']!==value||JSON.stringify(value)!=='{"b":[null,7]}'||Object.hasOwn(value.b,0))throw 'holder and deletion';
if(JSON.parse('2',function(k,v){return k===''?v+4:v;})!==6||JSON.parse('2',function(){})!==undefined)throw 'root';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_reviver_source_context_keeps_exact_last_parse_tokens() {
        callback_succeeds(
            r#"
var sources=[],contexts=[];
var result=JSON.parse('{"n":1.20e+2,"s":"a\\nb","z":-0,"n":5,"a":[true,null]}',function(k,v,c){contexts.push(c);sources.push(k+':'+Object.hasOwn(c,'source')+':'+c.source);return v;});
if(sources.join('|')!=='n:true:5|s:true:"a\\nb"|z:true:-0|0:true:true|1:true:null|a:false:undefined|:false:undefined')throw sources.join('|');
if(contexts[0]===contexts[1]||1/result.z!==-Infinity)throw 'fresh context or negative zero';
var raw='';JSON.parse(' 1.20e+2 ',function(k,v,c){raw=c.source;return v;});if(raw!=='1.20e+2')throw 'source range';
var d=Object.getOwnPropertyDescriptor(contexts[0],'source');if(!d.writable||!d.enumerable||!d.configurable)throw 'context descriptor';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_reviver_observes_mutation_and_ignores_definition_false() {
        callback_succeeds(
            r#"
var contexts='';
var result=JSON.parse('{"first":1,"later":2,"child":{"x":3}}',function(k,v,c){
if(k==='first'){this.later=9;this.child={x:3};this.added=4;Object.defineProperty(this,'first',{value:8,configurable:false});return undefined;}
contexts+=k+':'+Object.hasOwn(c,'source')+'|';return v;});
if(JSON.stringify(result)!=='{"first":8,"later":9,"child":{"x":3},"added":4}')throw 'mutation';
if(contexts!=='later:false|x:false|child:false|:false|')throw contexts;
var array=JSON.parse('[1,2]',function(k,v){if(k==='0'){Object.freeze(this);return 8;}return v;});
if(JSON.stringify(array)!=='[1,2]')throw 'frozen definitions';
var seen='';JSON.parse('{"a":1,"b":-0}',function(k,v,c){if(k==='a')this.b=0;if(k==='b')seen=Object.hasOwn(c,'source');return v;});
if(seen!==false)throw 'SameValue negative zero';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_replacer_and_tojson_keep_holder_and_order() {
        callback_succeeds(
            r#"
var order=[],leaf={toJSON:function(key){order.push('j:'+key+':'+(this===leaf));return {v:2};}},root={a:leaf,b:3};
var output=JSON.stringify(root,function(key,value){order.push('r:'+key+':'+(this===root));if(key==='v')return value+5;if(key==='b')return undefined;return value;});
if(output!=='{"a":{"v":7}}'||order.join('|')!=='r::false|j:a:true|r:a:true|r:v:false|r:b:true')throw 'hook order';
if(JSON.stringify(undefined,function(){return 4;})!=='4')throw 'undefined replacement';
var f=function(){};f.toJSON=function(k){return k+'function';};if(JSON.stringify({f:f})!=='{"f":"ffunction"}')throw 'function toJSON';
var fresh={toJSON(){throw 'second toJSON';}};if(JSON.stringify(1,function(k,v){return k===''?fresh:v;})!=='{}')throw 'replacer object';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_property_lists_keep_order_nonenum_and_inherited_keys() {
        callback_succeeds(
            r#"
var parent={inherited:7},o=Object.create(parent);o.b=2;o.a=1;o[2]=9;Object.defineProperty(o,'hidden',{value:4});
var boxed=Object(2),calls=0;boxed.toString=function(){calls++;return '2';};
var output=JSON.stringify(o,['b','a','b',boxed,'hidden','inherited',true,{},Symbol()]);
if(output!=='{"b":2,"a":1,"2":9,"hidden":4,"inherited":7}'||calls!==1)throw 'property list';
if(JSON.stringify({a:{b:2,a:1},b:3},['b','a'])!=='{"b":3,"a":{"b":2,"a":1}}')throw 'nested list';
if(JSON.stringify([1,2],['1'])!=='[1,2]')throw 'array list ignored';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_live_getters_omissions_cycles_and_thrown_identity() {
        callback_succeeds(
            r#"
var log='',o={};Object.defineProperty(o,'a',{enumerable:true,get:function(){log+='a';delete o.b;o.c=3;return 1;}});o.b=2;
if(JSON.stringify(o)!=='{"a":1}'||log!=='a')throw 'live keys';
if(JSON.stringify({s:Symbol(),f:function(){},u:undefined,a:[Symbol(),function(){},undefined]})!=='{"a":[null,null,null]}')throw 'omissions';
var a={};a.self=a;if(JSON.stringify(a,function(k,v){return k==='self'?'stopped':v;})!=='{"self":"stopped"}')throw 'cycle filtered';
var marker={},calls=0,caught=false;try{JSON.stringify({a:1,b:2},function(k,v){calls++;if(k==='a')throw marker;return v;});}catch(e){caught=e===marker;}
if(!caught||calls!==2)throw 'replacer throw';
var cyclic=false;try{JSON.stringify(1,function(k,v){return a;});}catch(e){cyclic=e.name==='TypeError';}if(!cyclic)throw 'replacement cycle';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_conversion_order_and_recursive_calls_preserve_values() {
        callback_succeeds(
            r#"
var order='',text={toString(){order+='s';return '{"x":2}';}};
if(JSON.parse(text,function(k,v){order+=k;return v;}).x!==2||order!=='sx')throw 'parse conversion';
order='';var list=[];Object.defineProperty(list,0,{get:function(){order+='l';return 'a';}});list.length=1;
var gap=Object(2);gap.valueOf=function(){order+='g';return 2;};var value={toJSON:function(){order+='j';return {a:1};}};
if(JSON.stringify(value,list,gap)!=='{\n  "a": 1\n}'||order!=='lgj')throw 'list space hook order';
if(JSON.stringify({a:1},function(k,v){if(k==='a')return JSON.parse(JSON.stringify(v+1));return v;})!=='{"a":2}')throw 'reentrant callbacks';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_never_roll_back_published_objects() {
        callback_succeeds(
            r#"
var saved,marker={},caught=false;
try{JSON.parse('{"a":1,"b":{"x":2}}',function(k,v){if(k==='a'){saved=this;throw marker;}return v;});}catch(e){caught=e===marker;}
if(!caught||saved.a!==1||saved.b.x!==2)throw 'published parsed object';
caught=false;try{JSON.parse({toString(){saved={keep:9};return '{bad';}});}catch(e){caught=e.name==='SyntaxError';}
if(!caught||saved.keep!==9)throw 'source conversion publication';
'ok';
"#,
        );
    }

    #[test]
    fn json_callbacks_created_cycles_and_hook_recursion_remain_bounded() {
        for source in [
            "try{JSON.parse('{\"a\":1,\"b\":{}}',function(k,v){if(k==='a')this.b=this;return v;});}catch(e){caught=true;}finally{finished=true;}",
            "var a={toJSON(){return JSON.stringify(this);}};try{JSON.stringify(a);}catch(e){caught=true;}finally{finished=true;}",
        ] {
            let mut runtime=Runtime::for_test();
            runtime.execute("var caught=false,finished=false;",&mut NoHost).unwrap();
            let error=runtime.execute(source,&mut NoHost).unwrap_err();
            assert!(error.contains("limit"),"{error}");
            assert_eq!(runtime.call_depth,0);assert_eq!(runtime.eval_depth,0);
            runtime.begin_job();
            assert_eq!(runtime.execute("!caught&&!finished;",&mut NoHost).unwrap().to_text(),"true");
        }
    }

    #[test]
    fn stringify_omits_symbol_values_and_uses_null_for_array_entries() {
        let result = Runtime::for_test()
            .execute(
                r#"var symbol=Symbol("value");
                [JSON.stringify(symbol)===undefined,
                 JSON.stringify({left:1,skipped:symbol,right:2}),
                 JSON.stringify([symbol,undefined,{skipped:symbol,kept:3}]),
                 JSON.stringify({kept:4},symbol,symbol)].join("|");"#,
                &mut NoHost,
            )
            .unwrap();
        assert_eq!(
            result.to_text(),
            "true|{\"left\":1,\"right\":2}|[null,null,{\"kept\":3}]|{\"kept\":4}"
        );
    }

    #[test]
    fn stringify_ignores_symbol_keys_without_reading_their_values() {
        let result = Runtime::for_test()
            .execute(
                r#"var symbol=Symbol("hidden");
                var object={"Symbol(hidden)":5,visible:7};
                object[symbol]=object;
                object[Symbol("hidden")]=[object];
                JSON.stringify(object);"#,
                &mut NoHost,
            )
            .unwrap();
        assert_eq!(result.to_text(), "{\"Symbol(hidden)\":5,\"visible\":7}");
    }

    #[test]
    fn parse_symbol_source_throws_a_catchable_type_error() {
        let result = Runtime::for_test()
            .execute(
                r#"var result="missing";
                try {JSON.parse(Symbol("source"));}
                catch(error) {result=error.name;}
                result;"#,
                &mut NoHost,
            )
            .unwrap();
        assert_eq!(result.to_text(), "TypeError");
    }

    #[test]
    fn stringify_unboxes_numeric_and_boolean_wrappers_but_keeps_symbol_objects() {
        let result = Runtime::for_test().execute(r#"
var n=new Number(7),b=new Boolean(false),s=Object(Symbol('field'));
n.self=n;b.self=b;s.field=3;
if(JSON.stringify(n)!=='7'||JSON.stringify(b)!=='false'||JSON.stringify(s)!=='{"field":3}')throw 'wrapper values';
if(JSON.stringify([n,b,s,new Number(NaN),new Number(Infinity),new Number(-0)])!=='[7,false,{"field":3},null,null,0]')throw 'array wrappers';
if(JSON.stringify({number:n,boolean:b})!=='{"number":7,"boolean":false}')throw 'object wrappers';
if(JSON.stringify(Number.prototype)!=='0'||JSON.stringify(Boolean.prototype)!=='false')throw 'intrinsic slots';
var read=0;Object.defineProperty(n,'toJSON',{get:function(){read++;return undefined;}});
Object.defineProperty(b,'toJSON',{get:function(){read++;return undefined;}});
if(JSON.stringify([n,b])!=='[7,false]'||read!==2)throw 'toJSON lookup';
s.self=s;var caught=false;try{JSON.stringify(s);}catch(e){caught=e.name==='TypeError';}
if(!caught)throw 'symbol wrapper cycle';
'ok';
"#, &mut NoHost).unwrap();
        assert_eq!(result.to_text(), "ok");
    }

    #[test]
    fn stringify_wrapper_conversion_observes_hooks_and_preserves_exception_order() {
        let result = Runtime::for_test().execute(r#"
var n=Object(3),b=Object(false),s=Object(Symbol('field')),log='';
Object.defineProperty(n,'toJSON',{get:function(){log+='j';return 0;},configurable:true});
n[Symbol.toPrimitive]=function(hint){if(this!==n)throw 'receiver';log+=hint;return '9';};
b[Symbol.toPrimitive]=function(){throw 'Boolean hook';};b.valueOf=function(){throw 'Boolean valueOf';};
Object.defineProperty(s,Symbol.toPrimitive,{value:function(){throw 'Symbol hook';}});s.kept=2;
if(JSON.stringify([n,b,s])!=='[9,false,{"kept":2}]'||log!=='jnumber')throw 'unboxing policy';
var marker={},caught=0;
n[Symbol.toPrimitive]=function(){throw marker;};
try{JSON.stringify(n);}catch(e){if(e===marker)caught++;}
Object.defineProperty(n,'toJSON',{get:function(){throw marker;},configurable:true});
try{JSON.stringify(n);}catch(e){if(e===marker)caught++;}
if(caught!==2)throw 'exception identity';
'ok';
"#, &mut NoHost).unwrap();
        assert_eq!(result.to_text(), "ok");
    }

    #[test]
    fn stringify_boxed_numeric_indentation_converts_before_omitted_root() {
        let result = Runtime::for_test()
            .execute(
                r#"
var gap=Object(2),log='',value={a:1};
gap[Symbol.toPrimitive]=function(hint){if(this!==gap)throw 'gap receiver';log+=hint;return 2.8;};
if(JSON.stringify(value,null,gap)!=='{\n  "a": 1\n}'||log!=='number')throw 'number gap';
var ignored=Object(true);ignored[Symbol.toPrimitive]=function(){throw 'ignored gap';};
if(JSON.stringify(value,null,ignored)!=='{"a":1}')throw 'Boolean gap';
if(JSON.stringify(value,null,new Number(Infinity))!=='{\n          "a": 1\n}')throw 'gap clamp';
if(JSON.stringify(value,null,new Number(NaN))!=='{"a":1}')throw 'NaN gap';
var marker={},seen=false;gap[Symbol.toPrimitive]=function(){throw marker;};
try{JSON.stringify(undefined,null,gap);}catch(e){seen=e===marker;}
if(!seen)throw 'gap conversion before omitted root';
'ok';
"#,
                &mut NoHost,
            )
            .unwrap();
        assert_eq!(result.to_text(), "ok");
    }
}
