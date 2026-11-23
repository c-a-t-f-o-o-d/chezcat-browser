//! Original bounded JSON support. Parsing and serialization use explicit frame
//! stacks and the enclosing Runtime's object, retained-value and work budgets.
//! Callback walks retain source snapshots and real holders without recursive
//! native traversal. Parsing preserves exact UTF-16 strings, keys and source
//! tokens; serialization escapes unpaired units in quoted data and preserves
//! exact indentation code units.

use super::{
    number_text, property_index, strict_equal, Fault, Host, Ordinary, PropertyKey, Reference,
    Runtime, StringKey, Value, WorkKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

struct ParseSnapshot {
    value: Value,
    start: usize,
    end: usize,
}

type Snapshots = BTreeMap<usize, BTreeMap<StringKey, ParseSnapshot>>;

// Positions use the source's native representation. Grammar is ASCII, but
// strings and reviver source tokens must retain every UTF-16 code unit.
#[derive(Clone, Copy)]
enum Source<'a> {
    Scalar(&'a str),
    Units(&'a [u16]),
}

impl Source<'_> {
    fn len(self) -> usize {
        match self { Self::Scalar(text) => text.len(), Self::Units(units) => units.len() }
    }

    fn bytes(self) -> usize {
        match self { Self::Scalar(text) => text.len(), Self::Units(units) => units.len().saturating_mul(2) }
    }

    fn position_kind(self) -> &'static str {
        match self { Self::Scalar(_) => "byte", Self::Units(_) => "code unit" }
    }

    fn grammar_byte(self, at: usize) -> Option<u8> {
        // 0xff is an invalid grammar byte, not EOF. Never narrow a raw unit
        // into an ASCII delimiter (for example, U+0122 into a quote).
        match self {
            Self::Scalar(text) => text.as_bytes().get(at).copied(),
            Self::Units(units) => units.get(at).map(|&unit| {
                if unit <= 0x7f { unit as u8 } else { 0xff }
            }),
        }
    }

    fn starts_with(self, at: usize, word: &[u8]) -> bool {
        word.iter().enumerate().all(|(index, &byte)| self.grammar_byte(at + index) == Some(byte))
    }

    fn slice_value(self, runtime: &mut Runtime, start: usize, end: usize) -> Result<Value, Fault> {
        match self {
            Self::Scalar(text) => {
                let text = &text[start..end];
                runtime.work(text.len())?;
                let mut copy = String::new();
                copy.try_reserve_exact(text.len()).map_err(|_| Fault::new("Unable to allocate JSON source token."))?;
                copy.push_str(text);
                Ok(Value::String(copy))
            }
            Self::Units(units) => runtime.string_from_units(&units[start..end]),
        }
    }
}

enum StringBuffer {
    Scalar(String),
    Units(Vec<u16>),
}

struct ParsedString {
    buffer: StringBuffer,
    high: Option<u16>,
    unpaired: bool,
}

impl ParsedString {
    fn new(raw_source: bool) -> Self {
        Self {
            // A raw source can contain a BMP prefix whose UTF-8 expansion
            // exceeds the limit, followed by an unpaired unit that makes the
            // final, smaller UTF-16 representation necessary. Its admitted
            // source bytes bound the decoded units until the full token is known.
            buffer: if raw_source { StringBuffer::Units(Vec::new()) } else { StringBuffer::Scalar(String::new()) },
            high: None,
            unpaired: false,
        }
    }

    fn reserve_units(runtime: &mut Runtime, units: &mut Vec<u16>, added: usize) -> Result<(), Fault> {
        let limit = runtime.limits.capacity.string_bytes / 2;
        let required = units.len().saturating_add(added);
        if required > limit { return Err(runtime.string_limit_fault()); }
        runtime.work_as(added.saturating_mul(2), WorkKind::ConversionOutput)?;
        if required > units.capacity() {
            runtime.work(units.len().saturating_mul(2))?;
            let target = required.max(units.capacity().saturating_mul(2).min(limit));
            units.try_reserve_exact(target - units.len())
                .map_err(|_| Fault::new("Unable to allocate JSON string units."))?;
            if units.capacity() > limit { return Err(runtime.string_limit_fault()); }
        }
        Ok(())
    }

    fn append_char(&mut self, runtime: &mut Runtime, ch: char) -> Result<(), Fault> {
        match &mut self.buffer {
            StringBuffer::Scalar(text) => {
                let limit = runtime.limits.capacity.string_bytes;
                let required = text.len().saturating_add(ch.len_utf8());
                if required > limit { return Err(runtime.string_limit_fault()); }
                runtime.work_as(ch.len_utf8(), WorkKind::ConversionOutput)?;
                if required > text.capacity() {
                    runtime.work(text.len())?;
                    let target = required.max(text.capacity().saturating_mul(2).min(limit));
                    text.try_reserve_exact(target - text.len())
                        .map_err(|_| Fault::new("Unable to allocate JSON string."))?;
                    if text.capacity() > limit { return Err(runtime.string_limit_fault()); }
                }
                text.push(ch);
            }
            StringBuffer::Units(units) => {
                let mut buffer = [0; 2];
                let added = ch.encode_utf16(&mut buffer);
                Self::reserve_units(runtime, units, added.len())?;
                units.extend_from_slice(added);
            }
        }
        Ok(())
    }

    fn append_unpaired(&mut self, runtime: &mut Runtime, unit: u16) -> Result<(), Fault> {
        self.unpaired = true;
        if let StringBuffer::Scalar(text) = &self.buffer {
            runtime.work(text.len())?;
            let count = text.encode_utf16().count().saturating_add(1);
            if count > runtime.limits.capacity.string_bytes / 2 { return Err(runtime.string_limit_fault()); }
            runtime.work_as(count.saturating_mul(2), WorkKind::ConversionOutput)?;
            let mut units = Vec::new();
            units.try_reserve_exact(count)
                .map_err(|_| Fault::new("Unable to allocate JSON string units."))?;
            if units.capacity() > runtime.limits.capacity.string_bytes / 2 {
                return Err(runtime.string_limit_fault());
            }
            units.extend(text.encode_utf16());
            units.push(unit);
            self.buffer = StringBuffer::Units(units);
        } else if let StringBuffer::Units(units) = &mut self.buffer {
            Self::reserve_units(runtime, units, 1)?;
            units.push(unit);
        }
        Ok(())
    }

    fn unit(&mut self, runtime: &mut Runtime, unit: u16) -> Result<(), Fault> {
        if let Some(high) = self.high.take() {
            if (0xdc00..=0xdfff).contains(&unit) {
                let code = 0x10000 + ((high as u32 - 0xd800) << 10) + unit as u32 - 0xdc00;
                return self.append_char(runtime, char::from_u32(code).unwrap());
            }
            self.append_unpaired(runtime, high)?;
        }
        if (0xd800..=0xdbff).contains(&unit) {
            self.high = Some(unit);
            Ok(())
        } else if (0xdc00..=0xdfff).contains(&unit) {
            self.append_unpaired(runtime, unit)
        } else {
            self.append_char(runtime, char::from_u32(unit as u32).unwrap())
        }
    }

    fn scalar(&mut self, runtime: &mut Runtime, ch: char) -> Result<(), Fault> {
        if let Some(high) = self.high.take() { self.append_unpaired(runtime, high)?; }
        self.append_char(runtime, ch)
    }

    fn finish(mut self, runtime: &mut Runtime) -> Result<Value, Fault> {
        if let Some(high) = self.high.take() { self.append_unpaired(runtime, high)?; }
        match self.buffer {
            StringBuffer::Scalar(text) => Ok(Value::String(text)),
            StringBuffer::Units(units) if self.unpaired => Ok(Value::StringUnits(units)),
            // Fully well-formed raw-source tokens use the same canonical
            // scalar identity and output-byte admission as other strings.
            StringBuffer::Units(units) => runtime.string_from_units(&units),
        }
    }
}

struct Reader<'a> {
    source: Source<'a>,
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

fn reserve_json_frames<T>(
    frames: &mut Vec<T>,
    additional: usize,
    budget: usize,
    other_bytes: usize,
    operation: &str,
) -> Result<(), Fault> {
    let required = frames.len().saturating_add(additional);
    if required <= frames.capacity() {
        return Ok(());
    }
    let max_frames = budget.saturating_sub(other_bytes) / size_of::<T>();
    let exhausted = || Fault::new(format!("JSON {operation} scratch limit reached ({budget} bytes)."));
    if required > max_frames {
        return Err(exhausted());
    }
    let target = required.max(frames.capacity().saturating_mul(2).min(max_frames));
    frames.try_reserve_exact(target - frames.len()).map_err(|_| {
        Fault::new(format!("Unable to allocate JSON {operation} frames."))
    })?;
    if frames.capacity() > max_frames {
        return Err(exhausted());
    }
    Ok(())
}

fn key_storage(keys: &Vec<StringKey>) -> usize {
    keys.iter().fold(keys.capacity() * size_of::<StringKey>(), |bytes, key| {
        bytes.saturating_add(key.capacity_bytes())
    })
}

impl Reader<'_> {
    fn snapshot(
        &mut self,
        runtime: &mut Runtime,
        holder: usize,
        key: &StringKey,
        value: &Value,
        start: usize,
        frame_bytes: usize,
    ) -> Result<(), Fault> {
        let Some(snapshots) = self.snapshots.as_mut() else {
            return Ok(());
        };
        let entries = snapshots.get(&holder);
        let previous = entries.and_then(|entries| entries.get_key_value(key));
        let entry_bytes = size_of::<(StringKey, ParseSnapshot)>() + 3 * size_of::<usize>();
        let holder_bytes = if entries.is_none() {
            size_of::<(usize, BTreeMap<StringKey, ParseSnapshot>)>() + 3 * size_of::<usize>()
        } else { 0 };
        let old = previous.map_or(0, |(key, old)| entry_bytes + key.capacity_bytes() + old.value.bytes());
        let key_bytes = previous.map_or(key.byte_len(), |(key, _)| key.capacity_bytes());
        let bytes = entry_bytes.saturating_add(key_bytes).saturating_add(value.bytes());
        let mut retained = self
            .snapshot_bytes
            .saturating_sub(old)
            .saturating_add(bytes)
            .saturating_add(holder_bytes);
        let scratch = frame_bytes.saturating_add(runtime.json_scratch_bytes);
        let budget = runtime.limits.heap.retained_values;
        let exhausted = || Fault::new(format!("JSON reviver snapshot limit reached ({budget} bytes)."));
        if retained > budget.saturating_sub(scratch) { return Err(exhausted()); }
        runtime.work(bytes.saturating_add(holder_bytes))?;
        let value = runtime.copy_value(value)?;
        let snapshot = ParseSnapshot { value, start, end: self.at };
        if let Some(previous) = snapshots.get_mut(&holder).and_then(|entries| entries.get_mut(key)) {
            // Replacing a duplicate retains the original key allocation, as
            // well as the parsed object's original property creation order.
            *previous = snapshot;
        } else {
            let owned = match key {
                StringKey::Scalar(name) => {
                    let mut copy = String::new();
                    copy.try_reserve_exact(name.len()).map_err(|_| Fault::new("Unable to allocate JSON snapshot key."))?;
                    copy.push_str(name);
                    StringKey::Scalar(copy)
                }
                StringKey::Units(units) => {
                    let mut copy = Vec::new();
                    copy.try_reserve_exact(units.len()).map_err(|_| Fault::new("Unable to allocate JSON snapshot key."))?;
                    copy.extend_from_slice(units);
                    StringKey::Units(copy)
                }
            };
            retained = retained.saturating_add(owned.capacity_bytes().saturating_sub(key_bytes));
            if retained > budget.saturating_sub(scratch) { return Err(exhausted()); }
            snapshots.entry(holder).or_default().insert(owned, snapshot);
        }
        self.snapshot_bytes = retained;
        Ok(())
    }

    fn byte(&self) -> Option<u8> {
        self.source.grammar_byte(self.at)
    }

    fn space(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn syntax(&self, message: &str) -> Fault {
        Fault::exception(
            "SyntaxError",
            format!("JSON at {} {}: {message}", self.source.position_kind(), self.at),
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

    fn string(&mut self, runtime: &mut Runtime) -> Result<Value, Fault> {
        runtime.step()?;
        if !self.take(b'"') {
            return Err(self.syntax("Expected a quoted string."));
        }
        let mut output = ParsedString::new(matches!(self.source, Source::Units(_)));
        loop {
            match self.byte() {
                Some(b'"') => {
                    self.at += 1;
                    return output.finish(runtime);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let escaped = self
                        .byte()
                        .ok_or_else(|| self.syntax("Unterminated escape."))?;
                    self.at += 1;
                    let unit = match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            output.unit(runtime, self.hex_unit()?)?;
                            continue;
                        }
                        _ => return Err(self.syntax("Invalid string escape.")),
                    };
                    output.unit(runtime, unit as u16)?;
                }
                Some(0..=31) => return Err(self.syntax("Unescaped control character in string.")),
                Some(_) => {
                    match self.source {
                        Source::Scalar(text) => {
                            let ch = text[self.at..].chars().next().unwrap();
                            self.at += ch.len_utf8();
                            output.scalar(runtime, ch)?;
                        }
                        Source::Units(units) => {
                            let unit = units[self.at];
                            self.at += 1;
                            output.unit(runtime, unit)?;
                        }
                    }
                }
                None => return Err(self.syntax("Unterminated string.")),
            }
        }
    }

    fn number(&mut self, runtime: &mut Runtime) -> Result<Value, Fault> {
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
        // Only a validated ASCII number token needs a scalar copy for f64
        // conversion; the surrounding raw source is never replaced or lost.
        let copied;
        let text = match self.source {
            Source::Scalar(text) => &text[start..self.at],
            Source::Units(units) => {
                let units = &units[start..self.at];
                runtime.work(units.len())?;
                let mut text = String::new();
                text.try_reserve_exact(units.len()).map_err(|_| Fault::new("Unable to allocate JSON number token."))?;
                text.extend(units.iter().map(|&unit| char::from_u32(unit as u32).unwrap()));
                copied = text;
                &copied
            }
        };
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.syntax("Invalid JSON number."))
    }

    fn value(
        &mut self,
        runtime: &mut Runtime,
    ) -> Result<(Value, Option<ParseFrame>), Fault> {
        runtime.step()?;
        self.space();
        let value = match self.byte() {
            Some(b'[' | b'{') => {
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
            Some(b'"') => self.string(runtime)?,
            Some(b'-' | b'0'..=b'9') => self.number(runtime)?,
            Some(b'n') if self.source.starts_with(self.at, b"null") => {
                self.at += 4;
                Value::Null
            }
            Some(b't') if self.source.starts_with(self.at, b"true") => {
                self.at += 4;
                Value::Bool(true)
            }
            Some(b'f') if self.source.starts_with(self.at, b"false") => {
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
        let (root, frame) = self.value(runtime)?;
        let mut frames = Vec::new();
        self.reserve_frames(runtime, &mut frames, usize::from(frame.is_some()))?;
        self.snapshot(
            runtime, usize::MAX, &StringKey::Scalar(String::new()), &root, start, frames.capacity() * size_of::<ParseFrame>(),
        )?;
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
                    StringKey::Scalar(key)
                } else {
                    let key = match self.string(runtime)? {
                        Value::String(name) => {
                            runtime.property_name(&name)?;
                            StringKey::Scalar(name)
                        }
                        Value::StringUnits(units) => {
                            runtime.property_name_units(&units)?;
                            StringKey::Units(units)
                        }
                        _ => unreachable!("a JSON string token"),
                    };
                    self.space();
                    if !self.take(b':') {
                        return Err(self.syntax("Expected a colon after the property name."));
                    }
                    key
                };
                self.space();
                let start = self.at;
                let (value, child) = self.value(runtime)?;
                self.reserve_frames(runtime, &mut frames, 1 + usize::from(child.is_some()))?;
                self.snapshot(
                    runtime, frame.id, &key, &value, start, frames.capacity() * size_of::<ParseFrame>(),
                )?;
                match key {
                    StringKey::Scalar(name) => runtime.store_property(frame.id, name, value, Some((true, true, true)))?,
                    StringKey::Units(units) => runtime.store_units_property(frame.id, units, value, Some((true, true, true)))?,
                }
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

    fn reserve_frames(
        &self,
        runtime: &Runtime,
        frames: &mut Vec<ParseFrame>,
        additional: usize,
    ) -> Result<(), Fault> {
        reserve_json_frames(
            frames,
            additional,
            runtime.limits.heap.retained_values,
            runtime.json_scratch_bytes.saturating_add(self.snapshot_bytes),
            "parse",
        )
    }
}

enum Entries {
    Array { next: usize, length: usize },
    Object(std::vec::IntoIter<StringKey>),
    Listed { next: usize },
}

struct WriteFrame {
    id: usize,
    entries: Entries,
    wrote: bool,
    scratch_bytes: usize,
}

enum Gap {
    Scalar { bytes: [u8; 30], length: usize },
    Units { units: [u16; 10], length: usize },
}

impl Gap {
    fn from_value(runtime: &mut Runtime, value: Option<&Value>) -> Result<Self, Fault> {
        let mut units = [0; 10];
        let mut length = 0;
        match value {
            Some(Value::Number(value)) => {
                length = if value.is_nan() { 0 } else { value.clamp(0.0, 10.0) as usize };
                units[..length].fill(0x20);
            }
            Some(Value::String(text)) => {
                for unit in text.encode_utf16().take(units.len()) {
                    units[length] = unit;
                    length += 1;
                }
            }
            Some(Value::StringUnits(text)) => {
                length = text.len().min(units.len());
                units[..length].copy_from_slice(&text[..length]);
            }
            _ => (),
        }
        runtime.work_as(length * 2, WorkKind::CharacterScan)?;
        let mut bytes = [0; 30];
        let mut scalar_length = 0;
        for character in char::decode_utf16(units[..length].iter().copied()) {
            let Ok(character) = character else { return Ok(Self::Units { units, length }); };
            let mut encoded = [0; 4];
            let text = character.encode_utf8(&mut encoded).as_bytes();
            bytes[scalar_length..scalar_length + text.len()].copy_from_slice(text);
            scalar_length += text.len();
        }
        Ok(Self::Scalar { bytes, length: scalar_length })
    }

    fn is_empty(&self) -> bool {
        match self { Self::Scalar { length, .. } | Self::Units { length, .. } => *length == 0 }
    }

    fn append(&self, runtime: &mut Runtime, output: &mut WriteOutput) -> Result<(), Fault> {
        match self {
            Self::Scalar { bytes, length } => output.append_scalar(
                runtime, std::str::from_utf8(&bytes[..*length]).expect("encoded JSON indentation"),
            ),
            Self::Units { units, length } => output.append_raw_gap(runtime, &units[..*length]),
        }
    }
}

struct WriteOutput {
    buffer: StringBuffer,
}

impl Default for WriteOutput {
    fn default() -> Self { Self { buffer: StringBuffer::Scalar(String::new()) } }
}

impl WriteOutput {
    fn scratch_fault(runtime: &Runtime) -> Fault {
        Fault::new(format!("JSON stringify output scratch limit reached ({} bytes).", runtime.limits.heap.retained_values))
    }

    fn reserve_scalar(runtime: &mut Runtime, output: &mut String, required: usize) -> Result<(), Fault> {
        let limit = runtime.limits.capacity.string_bytes;
        if required > limit { return Err(runtime.string_limit_fault()); }
        if required <= output.capacity() { return Ok(()); }
        let old = output.capacity();
        let available = runtime.limits.heap.retained_values.saturating_sub(runtime.json_scratch_bytes);
        let maximum = limit.min(old.saturating_add(available));
        if required > maximum { return Err(Self::scratch_fault(runtime)); }
        let target = required.max(old.saturating_mul(2).min(maximum));
        runtime.work(output.len())?;
        output.try_reserve_exact(target - output.len())
            .map_err(|_| Fault::new("Unable to allocate JSON serialization text."))?;
        if output.capacity() > maximum { return Err(Self::scratch_fault(runtime)); }
        runtime.charge_json_scratch(output.capacity() - old, "stringify output")
    }

    fn reserve_units(runtime: &mut Runtime, output: &mut Vec<u16>, required: usize) -> Result<(), Fault> {
        let limit = runtime.limits.capacity.string_bytes / 2;
        if required > limit { return Err(runtime.string_limit_fault()); }
        if required <= output.capacity() { return Ok(()); }
        let old = output.capacity();
        let available = runtime.limits.heap.retained_values.saturating_sub(runtime.json_scratch_bytes);
        let maximum = limit.min(old.saturating_add(available / 2));
        if required > maximum { return Err(Self::scratch_fault(runtime)); }
        let target = required.max(old.saturating_mul(2).min(maximum));
        runtime.work(output.len().saturating_mul(2))?;
        output.try_reserve_exact(target - output.len())
            .map_err(|_| Fault::new("Unable to allocate JSON serialization units."))?;
        if output.capacity() > maximum { return Err(Self::scratch_fault(runtime)); }
        runtime.charge_json_scratch((output.capacity() - old) * 2, "stringify output")
    }

    fn append_scalar(&mut self, runtime: &mut Runtime, added: &str) -> Result<(), Fault> {
        match &mut self.buffer {
            StringBuffer::Scalar(output) => {
                let required = output.len().checked_add(added.len()).ok_or_else(|| runtime.string_limit_fault())?;
                if required > runtime.limits.capacity.string_bytes { return Err(runtime.string_limit_fault()); }
                runtime.work_as(added.len(), WorkKind::ConversionOutput)?;
                Self::reserve_scalar(runtime, output, required)?;
                output.push_str(added);
            }
            StringBuffer::Units(output) => {
                runtime.work_as(added.len(), WorkKind::CharacterScan)?;
                let count = added.encode_utf16().count();
                let required = output.len().checked_add(count).ok_or_else(|| runtime.string_limit_fault())?;
                if required > runtime.limits.capacity.string_bytes / 2 { return Err(runtime.string_limit_fault()); }
                runtime.work_as(count.saturating_mul(2), WorkKind::ConversionOutput)?;
                Self::reserve_units(runtime, output, required)?;
                output.extend(added.encode_utf16());
            }
        }
        Ok(())
    }

    fn append_raw_gap(&mut self, runtime: &mut Runtime, added: &[u16]) -> Result<(), Fault> {
        if let StringBuffer::Scalar(previous) = &self.buffer {
            runtime.work_as(previous.len(), WorkKind::CharacterScan)?;
            let required = previous.encode_utf16().count().checked_add(added.len())
                .ok_or_else(|| runtime.string_limit_fault())?;
            if required > runtime.limits.capacity.string_bytes / 2 { return Err(runtime.string_limit_fault()); }
            runtime.work_as(required.saturating_mul(2), WorkKind::ConversionOutput)?;
            let mut output = Vec::new();
            // Both allocations remain reserved until promotion drops the
            // scalar prefix. Nested JSON callbacks retain this scratch charge.
            Self::reserve_units(runtime, &mut output, required)?;
            output.extend(previous.encode_utf16());
            output.extend_from_slice(added);
            let old = previous.capacity();
            self.buffer = StringBuffer::Units(output);
            runtime.json_scratch_bytes -= old;
        } else if let StringBuffer::Units(output) = &mut self.buffer {
            let required = output.len().checked_add(added.len()).ok_or_else(|| runtime.string_limit_fault())?;
            if required > runtime.limits.capacity.string_bytes / 2 { return Err(runtime.string_limit_fault()); }
            runtime.work_as(added.len().saturating_mul(2), WorkKind::ConversionOutput)?;
            Self::reserve_units(runtime, output, required)?;
            output.extend_from_slice(added);
        }
        Ok(())
    }

    fn into_value(self) -> Value {
        match self.buffer {
            StringBuffer::Scalar(output) => Value::String(output),
            // Only an emitted raw gap selects this representation. JSON
            // syntax surrounds each indent, so at least one unpaired unit
            // remains even when adjacent gap repetitions repair a pair.
            StringBuffer::Units(output) => Value::StringUnits(output),
        }
    }
}

#[derive(Default)]
struct Writer {
    output: WriteOutput,
    frames: Vec<WriteFrame>,
    ancestors: BTreeSet<usize>,
}

struct ReviveFrame {
    holder: Value,
    key: StringKey,
    value: Value,
    context: usize,
    entries: Option<Entries>,
    snapshot_matches: bool,
    scratch_bytes: usize,
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
    fn charge_json_scratch(&mut self, bytes: usize, operation: &str) -> Result<(), Fault> {
        let budget = self.limits.heap.retained_values;
        let total = self.json_scratch_bytes.saturating_add(bytes);
        if total > budget {
            return Err(Fault::new(format!("JSON {operation} scratch limit reached ({budget} bytes).")));
        }
        self.json_scratch_bytes = total;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn json_parse(&mut self, args: &[Value]) -> Result<Value, Fault> {
        // Host-free parsing is retained for the original internal allocation
        // assertions. The public builtin below performs observable conversion
        // and callback processing with its caller's host.
        let value = args.first().unwrap_or(&Value::Undefined);
        self.work(value.bytes())?;
        let scalar;
        let source = match value {
            Value::String(text) => Source::Scalar(text),
            Value::StringUnits(units) => Source::Units(units),
            _ => {
                scalar = value.script_text()?;
                Source::Scalar(&scalar)
            }
        };
        self.json_parse_source(source, false)
            .map(|(value, _, _)| value)
    }

    fn json_parse_source(
        &mut self,
        source: Source<'_>,
        capture: bool,
    ) -> Result<(Value, Snapshots, usize), Fault> {
        let max_source = self.limits.capacity.string_bytes;
        if source.bytes() > max_source {
            return Err(Fault::new(format!("JSON source limit reached ({max_source} bytes).")));
        }
        self.work(source.bytes())?;
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
        result.map(|value| (value, reader.snapshots.unwrap_or_default(), reader.snapshot_bytes))
    }

    pub(super) fn json_parse_builtin(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let outer_scratch = self.json_scratch_bytes;
        let result = self.json_parse_builtin_value(args, host);
        self.json_scratch_bytes = outer_scratch;
        result
    }

    fn json_parse_builtin_value(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let source = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        let source = self.primitive_with_hint(source, true, host)?;
        let source_value = match source {
            Value::String(_) | Value::StringUnits(_) => source,
            _ => Value::String(source.script_text()?),
        };
        let source = match &source_value {
            Value::String(text) => Source::Scalar(text),
            Value::StringUnits(units) => Source::Units(units),
            _ => unreachable!("a JSON source string"),
        };
        let callback = args
            .get(1)
            .filter(|value| matches!(value, Value::Function(_)));
        let (root, snapshots, snapshot_bytes) = self.json_parse_source(source, callback.is_some())?;
        let Some(callback) = callback else {
            return Ok(root);
        };
        self.charge_json_scratch(snapshot_bytes, "reviver")?;
        let holder = self.new_object(None)?;
        self.store_property(holder, "".into(), root, Some((true, true, true)))?;
        let first = self.json_revive_frame(
            Value::Object(holder),
            StringKey::Scalar(String::new()),
            snapshots.get(&usize::MAX).and_then(|entries| entries.get(&StringKey::Scalar(String::new()))),
            source,
            host,
        )?;
        let mut frames = Vec::new();
        self.json_push_revive_frame(&mut frames, first)?;
        loop {
            self.step()?;
            let frame = frames.last_mut().unwrap();
            let key = match frame.entries.as_mut() {
                Some(Entries::Array { next, length }) if *next < *length => {
                    let key = StringKey::Scalar(next.to_string());
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
                    self.work(key.byte_len())?;
                    snapshots.get(&id).and_then(|entries| entries.get(&key))
                } else {
                    None
                };
                let child = self.json_revive_frame(holder, key, snapshot, source, host)?;
                self.json_push_revive_frame(&mut frames, child)?;
                continue;
            }
            let frame = frames.pop().unwrap();
            let key_value = frame.key.clone().into_value();
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
            self.json_scratch_bytes -= frame.scratch_bytes;
        }
    }

    fn json_push_revive_frame(
        &mut self,
        frames: &mut Vec<ReviveFrame>,
        frame: ReviveFrame,
    ) -> Result<(), Fault> {
        self.charge_json_scratch(frame.scratch_bytes, "reviver")?;
        let previous_frame_bytes = frames.capacity() * size_of::<ReviveFrame>();
        reserve_json_frames(
            frames, 1, self.limits.heap.retained_values,
            self.json_scratch_bytes - previous_frame_bytes, "reviver",
        )?;
        self.json_scratch_bytes += frames.capacity() * size_of::<ReviveFrame>() - previous_frame_bytes;
        frames.push(frame);
        Ok(())
    }

    fn json_revive_frame(
        &mut self,
        holder: Value,
        key: StringKey,
        snapshot: Option<&ParseSnapshot>,
        source: Source<'_>,
        host: &mut dyn Host,
    ) -> Result<ReviveFrame, Fault> {
        self.work(key.byte_len())?;
        let value = self.read_reference(&key.clone().into_property_key().reference(holder.clone()), host)?;
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
            let text = source.slice_value(self, snapshot.start, snapshot.end)?;
            self.store_property(
                context,
                "source".into(),
                text,
                Some((true, true, true)),
            )?;
        }
        let mut scratch_bytes = key.capacity_bytes().saturating_add(value.bytes());
        let entries = if let Some(object) = Ordinary::from_value(&value) {
            let id = self.ordinary_id(object)?;
            Some(if let Some(length) = self.object(id)?.length {
                Entries::Array { next: 0, length }
            } else {
                let keys = self.enumerable_keys(id)?;
                scratch_bytes = scratch_bytes.saturating_add(key_storage(&keys));
                Entries::Object(keys.into_iter())
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
            scratch_bytes,
        })
    }

    fn json_revive_property(
        &mut self,
        holder: &Value,
        key: StringKey,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if matches!(value, Value::Undefined) {
            let previous = self.strict;
            self.strict = false;
            let result = self.delete_reference(key.into_property_key().reference(holder.clone()), host);
            self.strict = previous;
            return result.map(|_| ());
        }
        let id = self.ordinary_id(self.ordinary_required(holder)?)?;
        // InternalizeJSONProperty ignores a false definition result. Check
        // descriptor rejection before the throwing general-purpose helper;
        // abrupt typed-value conversions must still propagate unchanged.
        match &key {
            StringKey::Scalar(name) => {
                if let Some(index) = self.typed_numeric_key(id, name)? {
                    if !self.typed_index_valid(id, index) {
                        return Ok(());
                    }
                } else {
                    if self.objects[id]
                        .properties
                        .get(name)
                        .is_some_and(|property| !property.configurable)
                        || !self.objects[id].properties.contains_key(name) && !self.objects[id].extensible
                    {
                        return Ok(());
                    }
                    if let Some(length) = self.objects[id].length {
                        if name == "length"
                            || property_index(name).is_some_and(|index| index >= length)
                                && !self.objects[id].length_writable
                        {
                            return Ok(());
                        }
                    }
                }
            }
            StringKey::Units(units) => {
                let previous = self.objects[id].properties.get_units(units);
                if previous.is_some_and(|property| !property.configurable)
                    || previous.is_none() && !self.objects[id].extensible
                {
                    return Ok(());
                }
            }
        }
        self.create_data_property(holder, key.into_property_key(), value, host)
    }

    fn json_quote(&mut self, output: &mut WriteOutput, text: &str) -> Result<(), Fault> {
        self.json_quote_chars(output, text.chars().map(Ok))
    }

    fn json_quote_key(&mut self, output: &mut WriteOutput, key: &StringKey) -> Result<(), Fault> {
        match key {
            StringKey::Scalar(name) => self.json_quote(output, name),
            StringKey::Units(units) => self.json_quote_chars(output, char::decode_utf16(units.iter().copied())),
        }
    }

    fn json_quote_chars(&mut self, output: &mut WriteOutput, chars: impl Iterator<Item=Result<char, std::char::DecodeUtf16Error>>) -> Result<(), Fault> {
        output.append_scalar(self, "\"")?;
        for character in chars {
            let ch = match character {
                Ok(ch) => ch,
                Err(error) => {
                    output.append_scalar(self, &format!("\\u{:04x}", error.unpaired_surrogate()))?;
                    continue;
                }
            };
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
                output.append_scalar(self, escaped)?;
            } else if ch < '\u{20}' {
                output.append_scalar(self, &format!("\\u{:04x}", ch as u32))?;
            } else {
                output.append_scalar(self, ch.encode_utf8(&mut [0; 4]))?;
            }
        }
        output.append_scalar(self, "\"")
    }

    fn json_indent(&mut self, output: &mut WriteOutput, gap: &Gap, depth: usize) -> Result<(), Fault> {
        if !gap.is_empty() {
            output.append_scalar(self, "\n")?;
            for _ in 0..depth {
                gap.append(self, output)?;
            }
        }
        Ok(())
    }

    fn json_write_value(
        &mut self,
        value: Value,
        writer: &mut Writer,
        listed: bool,
    ) -> Result<(), Fault> {
        self.step()?;
        let Writer { output, frames, ancestors } = writer;
        match value {
            Value::Null | Value::Undefined | Value::Function(_) | Value::Symbol(_) => {
                output.append_scalar(self, "null")
            }
            Value::Bool(value) => output.append_scalar(self, if value { "true" } else { "false" }),
            Value::Number(value) => {
                let text = if value.is_finite() {
                    number_text(value)
                } else {
                    "null".into()
                };
                output.append_scalar(self, &text)
            }
            Value::String(value) => self.json_quote(output, &value),
            Value::BigInt(_) => Err(Fault::exception("TypeError", "JSON.stringify cannot serialize a BigInt.")),
            Value::StringUnits(units) => self.json_quote_chars(output, char::decode_utf16(units)),
            Value::Host(_) => Err(Fault::new(
                "JSON serialization of host objects is not implemented.",
            )),
            Value::Object(id) => {
                if ancestors.contains(&id) {
                    return Err(Fault::exception(
                        "TypeError",
                        "JSON.stringify cannot serialize a circular object.",
                    ));
                }
                let length = self.object(id)?.length;
                // Account for the ancestor key and its tree bookkeeping.
                let mut scratch_bytes = 4 * size_of::<usize>();
                let entries = if let Some(length) = length {
                    Entries::Array { next: 0, length }
                } else if listed {
                    Entries::Listed { next: 0 }
                } else {
                    // JSON visits enumerable string keys only. Symbol-keyed
                    // properties are not read, even when their values cycle.
                    let keys = self.enumerable_keys(id)?;
                    scratch_bytes = scratch_bytes.saturating_add(key_storage(&keys));
                    Entries::Object(keys.into_iter())
                };
                self.charge_json_scratch(scratch_bytes, "stringify")?;
                let previous_frame_bytes = frames.capacity() * size_of::<WriteFrame>();
                reserve_json_frames(
                    frames, 1, self.limits.heap.retained_values,
                    self.json_scratch_bytes - previous_frame_bytes, "stringify",
                )?;
                self.json_scratch_bytes += frames.capacity() * size_of::<WriteFrame>() - previous_frame_bytes;
                output.append_scalar(self, if length.is_some() { "[" } else { "{" })?;
                ancestors.insert(id);
                frames.push(WriteFrame {
                    id,
                    entries,
                    wrote: false,
                    scratch_bytes,
                });
                Ok(())
            }
        }
    }

    fn json_property_value(
        &mut self,
        holder: &Value,
        key: &StringKey,
        replacer: Option<&Value>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.work(key.byte_len())?;
        let mut value =
            self.read_reference(&key.clone().into_property_key().reference(holder.clone()), host)?;
        if matches!(value, Value::Host(_)) {
            return Err(Fault::new(
                "JSON serialization of host objects is not implemented.",
            ));
        }
        if matches!(value, Value::Object(_) | Value::Function(_) | Value::BigInt(_)) {
            let hook =
                self.read_reference(&Reference::Property(value.clone(), "toJSON".into()), host)?;
            if matches!(hook, Value::Function(_)) {
                value = self.call_value(hook, &[key.clone().into_value()], value, host)?;
            }
        }
        if let Some(replacer) = replacer {
            value = self.call_value(
                replacer.clone(),
                &[key.clone().into_value(), value],
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
            Some(super::primitive_wrappers::Data::BigInt(value)) => Ok(Value::BigInt(value.clone())),
            Some(super::primitive_wrappers::Data::String { .. }) => {
                let primitive = self.primitive_with_hint(value, true, host)?;
                self.json_string_primitive(primitive)
            }
            _ => Ok(value),
        }
    }

    fn json_string_primitive(&mut self, value: Value) -> Result<Value, Fault> {
        let value = match value {
            Value::String(_) | Value::StringUnits(_) => value,
            value => Value::String(value.script_text()?),
        };
        self.checked_value(value)
    }

    fn json_property_list(
        &mut self,
        value: Option<&Value>,
        host: &mut dyn Host,
    ) -> Result<Option<Vec<StringKey>>, Fault> {
        let Some(Value::Object(id)) = value else {
            return Ok(None);
        };
        let Some(length) = self.object(*id)?.length else {
            return Ok(None);
        };
        let mut keys = Vec::new();
        let mut seen = BTreeSet::new();
        let outer_scratch = self.json_scratch_bytes;
        for index in 0..length {
            self.step()?;
            let value = self.get_property_with_host(*id, &index.to_string(), host)?;
            let primitive = match value {
                value @ (Value::String(_) | Value::StringUnits(_) | Value::Number(_)) => Some(value),
                value
                    if matches!(
                        self.wrapped_data(&value),
                        Some(
                            super::primitive_wrappers::Data::Number(_)
                                | super::primitive_wrappers::Data::String { .. }
                        )
                    ) =>
                {
                    Some(self.primitive_with_hint(value, true, host)?)
                }
                _ => None,
            };
            if let Some(primitive) = primitive {
                let key = match primitive {
                    // Native values may use units even when they are entirely
                    // well formed. Share ordinary-key normalization before
                    // both duplicate detection and property-name admission.
                    Value::StringUnits(units) => match self.canonical_units_key(units)? {
                        PropertyKey::String(name) => StringKey::Scalar(name),
                        PropertyKey::StringUnits(units) => StringKey::Units(units),
                        PropertyKey::Symbol(_) => unreachable!(),
                    },
                    value => {
                        let name = if let Value::String(name) = value { name } else { value.script_text()? };
                        self.property_name(&name)?;
                        StringKey::Scalar(name)
                    }
                };
                self.work(key.byte_len())?;
                if !seen.contains(&key) {
                    if keys.len() == self.limits.heap.object_properties {
                        return Err(Fault::new(format!("JSON property list limit reached ({}).", self.limits.heap.object_properties)));
                    }
                    let added = key.capacity_bytes().saturating_mul(2)
                        + 2 * std::mem::size_of::<StringKey>()
                        + 3 * std::mem::size_of::<usize>();
                    self.charge_json_scratch(added, "property list snapshot")?;
                    self.work(added)?;
                    seen.insert(key.clone());
                    keys.push(key);
                }
            }
        }
        self.json_scratch_bytes = outer_scratch;
        Ok(Some(keys))
    }

    pub(super) fn json_stringify(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let outer_scratch = self.json_scratch_bytes;
        let result = self.json_stringify_value(args, host);
        self.json_scratch_bytes = outer_scratch;
        result
    }

    fn json_stringify_value(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = args.first().unwrap_or(&Value::Undefined);
        let replacer = args
            .get(1)
            .filter(|value| matches!(value, Value::Function(_)));
        let property_list = self.json_property_list(args.get(1), host)?;
        if let Some(keys) = &property_list {
            self.charge_json_scratch(key_storage(keys), "stringify")?;
        }
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
                Some(self.json_string_primitive(value)?)
            }
            _ => None,
        };
        let gap = Gap::from_value(self, converted_space.as_ref().or(args.get(2)))?;
        let holder = self.new_object(None)?;
        let value = self.copy_value(value)?;
        self.store_property(holder, "".into(), value, Some((true, true, true)))?;
        let value = self.json_property_value(&Value::Object(holder), &StringKey::Scalar(String::new()), replacer, host)?;
        if matches!(
            value,
            Value::Undefined | Value::Function(_) | Value::Symbol(_)
        ) {
            return Ok(Value::Undefined);
        }
        let mut writer = Writer::default();
        self.json_write_value(value, &mut writer, property_list.is_some())?;
        while let Some(mut frame) = writer.frames.pop() {
            let output = &mut writer.output;
            self.step()?;
            let array = matches!(frame.entries, Entries::Array { .. });
            let key = match &mut frame.entries {
                Entries::Array { next, length } => {
                    if *next == *length {
                        None
                    } else {
                        let key = StringKey::Scalar(next.to_string());
                        *next += 1;
                        Some(key)
                    }
                }
                Entries::Object(keys) => keys.next(),
                Entries::Listed { next } => {
                    let keys = property_list.as_ref().unwrap();
                    if let Some(key) = keys.get(*next) {
                        self.work(key.byte_len())?;
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
                    writer.frames.push(frame);
                    continue;
                }
                if frame.wrote {
                    output.append_scalar(self, ",")?;
                }
                frame.wrote = true;
                self.json_indent(output, &gap, writer.frames.len() + 1)?;
                if !array {
                    self.json_quote_key(output, &key)?;
                    output.append_scalar(self, if gap.is_empty() { ":" } else { ": " })?;
                }
                writer.frames.push(frame);
                self.json_write_value(value, &mut writer, property_list.is_some())?;
            } else {
                if frame.wrote {
                    self.json_indent(output, &gap, writer.frames.len())?;
                }
                output.append_scalar(self, if array { "]" } else { "}" })?;
                writer.ancestors.remove(&frame.id);
                self.json_scratch_bytes -= frame.scratch_bytes;
            }
        }
        self.checked_value(writer.output.into_value())
    }
}

#[cfg(test)]
#[path = "json_utf16_tests.rs"]
mod utf16_tests;

#[cfg(test)]
#[path = "json_utf16_boundary_tests.rs"]
mod utf16_boundary_tests;

#[cfg(test)]
#[path = "json_stringify_utf16_tests.rs"]
mod json_stringify_utf16_tests;

#[cfg(test)]
#[path = "json_stringify_utf16_boundary_tests.rs"]
mod json_stringify_utf16_boundary_tests;

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
    fn json_callbacks_share_scratch_with_the_caller_and_restore_it_after_failure() {
        let mut runtime = Runtime::for_test();
        let occupied = runtime.limits.heap.retained_values - 1;
        runtime.json_scratch_bytes = occupied;
        for (source, expected) in [
            ("JSON.parse('[]');", "JSON parse scratch limit"),
            ("JSON.stringify([]);", "JSON stringify scratch limit"),
        ] {
            let error = runtime.execute(source, &mut NoHost).unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert_eq!(runtime.json_scratch_bytes, occupied);
            runtime.begin_job();
        }
        runtime.json_scratch_bytes = 0;
        let result = runtime.execute("JSON.stringify(JSON.parse('[1]'));", &mut NoHost).unwrap();
        assert_eq!(result.to_text(), "[1]");
        assert_eq!(runtime.json_scratch_bytes, 0);
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
    fn stringify_raw_utf16_keys_escape_exact_units_and_keep_mixed_creation_order() {
        callback_succeeds(r#"
var high='\ud800',low='\udfff',object={},symbol=Symbol('hidden');
object.z=1;
object[high]=2;
object['2']=3;
object['\ufffd']=4;
object[low]=5;
object['x\ud800\udc00\udfff']=6;
object['\\ud800']=7;
object[symbol]=object;
Object.defineProperty(object,'hidden\ud800',{value:8});
if(JSON.stringify(object)!=='{"2":3,"z":1,"\\ud800":2,"�":4,"\\udfff":5,"x𐀀\\udfff":6,"\\\\ud800":7}')
    throw 'raw keys or mixed order';
delete object[high];object[high]=9;
if(JSON.stringify(object)!=='{"2":3,"z":1,"�":4,"\\udfff":5,"x𐀀\\udfff":6,"\\\\ud800":7,"\\ud800":9}')
    throw 'raw reinsertion order';
var controls={};controls['\0\ud800']='\udfff';
if(JSON.stringify(controls)!=='{"\\u0000\\ud800":"\\udfff"}')throw 'raw control key and value';
'ok';
"#);
    }

    #[test]
    fn stringify_raw_utf16_callbacks_and_property_lists_preserve_keys_and_live_reads() {
        callback_succeeds(r#"
var high='\ud800',low='\udfff',root={},trace=[];
var leaf={toJSON:function(key){if(this!==leaf||key!==high)throw 'raw toJSON key';trace.push('json');return 11;}};
root[high]=leaf;
Object.defineProperty(root,low,{enumerable:true,get:function(){trace.push('get');delete root.later;root.added=31;return 17;}});
root.later=19;
root['\ufffd']=23;
var output=JSON.stringify(root,function(key,value){
    if(key===''){trace.push('root');return value;}
    if(this!==root)throw 'raw replacer holder';
    trace.push(key);
    if(key===high&&value!==11)throw 'toJSON before raw replacer';
    return value;
});
if(output!=='{"\\ud800":11,"\\udfff":17,"�":23}'||trace.length!==7||
    trace[0]!=='root'||trace[1]!=='json'||trace[2]!==high||trace[3]!=='get'||
    trace[4]!==low||trace[5]!=='later'||trace[6]!=='\ufffd')throw 'raw callback order';
var parent={};parent[low]=37;
var listed=Object.create(parent);Object.defineProperty(listed,high,{value:41});listed['\ufffd']=43;
var boxed=Object('key'),calls=0;
boxed.toString=function(){calls++;return high;};
if(JSON.stringify(listed,[low,boxed,high,'\ufffd',low,Symbol('ignored')])!=='{"\\udfff":37,"\\ud800":41,"�":43}'||calls!==1)
    throw 'raw property-list order or duplicate identity';
'ok';
"#);
    }

    #[test]
    fn stringify_property_lists_canonicalize_native_scalar_units_and_boxed_conversion_results() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("nativeName", Value::StringUnits(vec![0x61])).unwrap();
        runtime.define_global("nativePair", Value::StringUnits(vec![0xd800, 0xdc00])).unwrap();
        let value = runtime.execute(r#"
var assigned={};assigned[nativeName]=7;
if(JSON.stringify(assigned,[nativeName])!=='{"a":7}')throw 'native scalar name';
var source={},trace='';
Object.defineProperty(source,'a',{get:function(){trace+='a';return 1;}});
Object.defineProperty(source,'\ud800\udc00',{get:function(){trace+='p';return 2;}});
var stringBox=Object('unused'),numberBox=Object(3);
stringBox.toString=function(){trace+='s';return nativeName;};
numberBox[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'hint';trace+='n';return nativePair;};
var output=JSON.stringify(source,[nativeName,'a',stringBox,nativePair,'\ud800\udc00',numberBox,nativeName]);
if(output!=='{"a":1,"𐀀":2}'||trace!=='snap')throw 'canonical list identity or conversion order';
'ok';
"#, &mut NoHost).unwrap();
        assert_eq!(value.to_text(), "ok");
    }

    #[test]
    fn stringify_native_scalar_unit_property_lists_share_the_scalar_name_byte_limit() {
        let mut runtime = Runtime::for_test();
        let capacity = runtime.limits.capacity.property_bytes;
        runtime.define_global("nativeBoundary", Value::StringUnits(vec![0x61; capacity])).unwrap();
        runtime.define_global("nativeOverflow", Value::StringUnits(vec![0x61; capacity + 1])).unwrap();
        let value = runtime.execute(
            "var boundary={};boundary[nativeBoundary]=7;JSON.stringify(boundary,[nativeBoundary]);",
            &mut NoHost,
        ).unwrap();
        assert_eq!(value.to_text(), format!("{{\"{}\":7}}", "a".repeat(capacity)));
        runtime.begin_job();
        let error = runtime.execute("JSON.stringify({},[nativeOverflow]);", &mut NoHost).unwrap_err();
        assert!(error.contains("Property-name limit"), "{error}");
    }

    #[test]
    fn json_reviver_raw_utf16_keys_added_to_later_holders_have_exact_callbacks_without_source() {
        callback_succeeds(r#"
var high='\ud800',low='\udfff',removed='remove\ud800',visited=[];
var value=JSON.parse('{"first":1,"later":{}}',function(key,value,context){
    if(key==='first'){this.later[high]=2;this.later[low]=3;this.later[removed]=4;}
    if(key===high||key===low||key===removed){
        visited.push(key);
        if(Object.hasOwn(context,'source')||this[key]!==value)throw 'raw reviver context or holder';
        if(key===high)return 9;
        if(key===low){Object.defineProperty(this,key,{value:7,configurable:false});return 11;}
        return undefined;
    }
    return value;
});
if(visited.length!==3||visited[0]!==high||visited[1]!==low||visited[2]!==removed||
    value.later[high]!==9||value.later[low]!==7||removed in value.later)throw 'raw reviver definitions';
if(JSON.stringify(value)!=='{"first":1,"later":{"\\ud800":9,"\\udfff":7}}')throw 'raw revived output';
'ok';
"#);
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
