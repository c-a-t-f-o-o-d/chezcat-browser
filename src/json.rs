//! Original bounded JSON support. Parsing and serialization use explicit frame
//! stacks and the enclosing Runtime's object, retained-value and work budgets.
//! Revivers, replacers, callable toJSON hooks and lone UTF-16 surrogates remain
//! explicit limitations; they are never silently ignored or evaluated as code.

use super::{number_text, Fault, Host, Runtime, Value, MAX_PARSE_DEPTH, MAX_STRING_BYTES};

struct Reader<'a> {
    source: &'a str,
    at: usize,
}

struct ParseFrame {
    id: usize,
    array: bool,
    next: usize,
    entry: bool,
    allow_end: bool,
}

impl Reader<'_> {
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
        let (root, frame) = self.value(runtime, 0)?;
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
                let (value, child) = self.value(runtime, frames.len() + 1)?;
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
}

struct WriteFrame {
    id: usize,
    entries: Entries,
    wrote: bool,
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

    pub(super) fn json_parse(&mut self, args: &[Value]) -> Result<Value, Fault> {
        if matches!(args.get(1), Some(Value::Function(_))) {
            return Err(Fault::new(
                "JSON.parse reviver functions are not implemented.",
            ));
        }
        let value = args.first().unwrap_or(&Value::Undefined);
        self.work(value.bytes())?;
        let source = value.script_text()?;
        if source.len() > MAX_STRING_BYTES {
            return Err(Fault::new("JSON source limit reached (128 KiB)."));
        }
        self.work(source.len())?;
        let checkpoint = (self.objects.len(), self.properties, self.value_bytes);
        let result = Reader {
            source: &source,
            at: 0,
        }
        .parse(self);
        if result.is_err() {
            // Invalid JSON cannot publish any of these fresh identities. Undo
            // only its allocations; spent execution/copy budgets stay spent.
            self.objects.truncate(checkpoint.0);
            self.properties = checkpoint.1;
            self.value_bytes = checkpoint.2;
        }
        result
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
        host: &mut dyn Host,
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
                if matches!(
                    self.get_property_with_host(id, "toJSON", host)?,
                    Value::Function(_)
                ) {
                    return Err(Fault::new(
                        "JSON.stringify toJSON functions are not implemented.",
                    ));
                }
                let length = self.object(id)?.length;
                let entries = if let Some(length) = length {
                    Entries::Array { next: 0, length }
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

    pub(super) fn json_stringify(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = args.first().unwrap_or(&Value::Undefined);
        if matches!(args.get(1), Some(Value::Function(_)))
            || matches!(args.get(1), Some(Value::Object(id)) if self.object(*id)?.length.is_some())
        {
            return Err(Fault::new(
                "JSON.stringify replacer functions and property lists are not implemented.",
            ));
        }
        let gap = match args.get(2) {
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
        if matches!(
            value,
            Value::Undefined | Value::Function(_) | Value::Symbol(_)
        ) {
            return Ok(Value::Undefined);
        }
        let mut output = String::new();
        let mut frames = Vec::new();
        let value = self.copy_value(value)?;
        self.json_write_value(value, &mut output, &mut frames, host)?;
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
            };
            if let Some(key) = key {
                let value = self.get_property_with_host(frame.id, &key, host)?;
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
                self.json_write_value(value, &mut output, &mut frames, host)?;
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

    #[test]
    fn stringify_omits_symbol_values_and_uses_null_for_array_entries() {
        let result = Runtime::new()
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
        let result = Runtime::new()
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
        let result = Runtime::new()
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
}
