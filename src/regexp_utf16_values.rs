//! RegExp consumes and returns the VM's actual UTF-16 string values. The
//! matcher, captures and lastIndex share the existing code-unit engine.
use super::*;

impl Runtime {
    pub(super) fn regexp_source_value(&mut self, expression: &regexp::Regex) -> Result<Value, Fault> {
        if let Some(units) = &expression.source_units {
            self.string_from_units(units)
        } else {
            self.checked_value(Value::String(expression.source.clone()))
        }
    }

    pub(super) fn regexp_factory(&mut self, args: &[Value]) -> Result<Value, Fault> {
        enum Pattern { Scalar(String), Units(Vec<u16>) }
        let pattern = args.first().unwrap_or(&Value::Undefined);
        let flags = args.get(1).unwrap_or(&Value::Undefined);
        let existing = match pattern {
            Value::Object(id) => self.object(*id)?.regexp.clone(),
            _ => None,
        };
        let pattern = if let Some(existing) = &existing {
            if let Some(units) = &existing.source_units {
                self.work_as(units.len().saturating_mul(2), WorkKind::ValueCopy)?;
                Pattern::Units(units.clone())
            } else {
                Pattern::Scalar(existing.source.clone())
            }
        } else if matches!(pattern, Value::Undefined) {
            Pattern::Scalar(String::new())
        } else if matches!(pattern, Value::StringUnits(_)) {
            Pattern::Units(self.string_units_for_value(pattern)?)
        } else {
            pattern.primitive()?;
            self.work(pattern.bytes())?;
            Pattern::Scalar(pattern.script_text()?)
        };
        let flags = if matches!(flags, Value::Undefined) {
            existing.as_ref().map_or(String::new(), |regex| regex.flags.text())
        } else {
            flags.primitive()?;
            self.work(flags.bytes())?;
            if matches!(flags, Value::StringUnits(_)) {
                return Err(Fault::exception("SyntaxError", "Invalid regular-expression flag."));
            }
            flags.script_text()?
        };
        let expression = match pattern {
            Pattern::Scalar(pattern) => {
                self.work(pattern.len().saturating_add(flags.len()))?;
                regexp::Regex::compile(&pattern, &flags, &mut self.regexp_work)
            }
            Pattern::Units(pattern) => {
                self.work(pattern.len().saturating_mul(2).saturating_add(flags.len()))?;
                regexp::Regex::compile_units(&pattern, &flags, &mut self.regexp_work)
            }
        }.map_err(Self::regexp_fault)?;
        let expression = Rc::new(expression);
        self.retain_regexp(expression.retained_bytes)?;
        self.regexp_object(expression)
    }

    // Result material comes from the matched code units, including halves of
    // a scalar pair, rather than a repaired scalar approximation.
    fn regexp_array_units(&mut self, found: &regexp::Match, input: &[u16], original: impl FnOnce() -> Value) -> Result<Value, Fault> {
        let mut values = Vec::with_capacity(found.captures.len());
        for capture in &found.captures {
            values.push(if let Some((start, end)) = capture {
                self.string_from_units(&input[*start..*end])?
            } else { Value::Undefined });
        }
        let Value::Object(id) = self.array_from_values(&values)? else { unreachable!() };
        self.set_property_throw(id, "index".into(), Value::Number(found.start() as f64))?;
        self.set_property_throw(id, "input".into(), original())?;
        let groups = self.regexp_named_groups(found, input)?;
        self.set_property_throw(id, "groups".into(), groups)?;
        Ok(Value::Object(id))
    }

    pub(super) fn regexp_method(&mut self, id: usize, method: &str, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        if method == "toString" {
            let expression = self.object(id)?.regexp.clone().unwrap();
            if let Some(source) = &expression.source_units {
                let flags = expression.flags.text();
                let count = source.len().saturating_add(flags.len()).saturating_add(2);
                self.work_as(count.saturating_mul(2), WorkKind::ConversionOutput)?;
                if count.saturating_mul(2) > self.limits.capacity.string_bytes {
                    return Err(Fault::new("RegExp string exceeds the string limit."));
                }
                let mut units = Vec::with_capacity(count);
                units.push(b'/' as u16);
                units.extend_from_slice(source);
                units.push(b'/' as u16);
                units.extend(flags.encode_utf16());
                return self.string_from_units(&units);
            }
            return self.checked_value(Value::String(format!("/{}/{}", expression.source, expression.flags.text())));
        }
        // Genuine author conversion precedes the live lastIndex read.
        let value = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
        let value = self.primitive_with_hint(value, true, host)?;
        self.work_as(value.bytes().saturating_mul(3), WorkKind::StringMethodInput)?;
        let original = if value.is_string() { value } else { Value::String(value.script_text()?) };
        match original {
            Value::String(text) => {
                let input: Vec<_> = text.encode_utf16().collect();
                self.regexp_exec_or_test(id, method, &input, || Value::String(text))
            }
            Value::StringUnits(units) => {
                self.regexp_exec_or_test(id, method, &units, || Value::StringUnits(units.clone()))
            }
            _ => unreachable!(),
        }
    }

    fn regexp_exec_or_test(
        &mut self, id: usize, method: &str, input: &[u16], original: impl FnOnce() -> Value,
    ) -> Result<Value, Fault> {
        let found = self.regexp_run(id, input)?;
        if method == "test" { return Ok(Value::Bool(found.is_some())); }
        match found {
            Some(found) => self.regexp_array_units(&found, input, original),
            None => Ok(Value::Null),
        }
    }

    pub(super) fn regexp_string_method(&mut self, text: &str, method: &str, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        if method=="replace" {return self.string_replace_value(Value::String(text.to_owned()),args,false,host);}
        self.work_as(text.len().saturating_mul(3), WorkKind::StringMethodInput)?;
        let input: Vec<_> = text.encode_utf16().collect();
        self.regexp_search_or_match_units(&input, method, args, || Value::String(text.to_owned()))
    }

    pub(super) fn regexp_string_method_units(&mut self, input: &[u16], method: &str, args: &[Value], _host: &mut dyn Host) -> Result<Value, Fault> {
        self.work_as(input.len().saturating_mul(3), WorkKind::StringMethodInput)?;
        if method == "replace" {
            return Err(Fault::new("String replacement containing unpaired UTF-16 units is not implemented."));
        }
        self.regexp_search_or_match_units(input, method, args, || Value::StringUnits(input.to_vec()))
    }

    fn regexp_search_or_match_units(&mut self, input: &[u16], method: &str, args: &[Value], original: impl FnOnce() -> Value) -> Result<Value, Fault> {
        let pattern = args.first().unwrap_or(&Value::Undefined);
        let id = match pattern {
            Value::Object(id) if self.object(*id)?.regexp.is_some() => *id,
            _ => {
                let Value::Object(id) = self.regexp_factory(std::slice::from_ref(pattern))? else { unreachable!() };
                id
            }
        };
        let expression = self.object(id)?.regexp.clone().unwrap();
        if method == "search" {
            let found = expression.find(input, 0, expression.flags.sticky, &mut self.regexp_work).map_err(Self::regexp_fault)?;
            return Ok(Value::Number(found.as_ref().map_or(-1.0, |found| found.start() as f64)));
        }
        if !expression.flags.global {
            return if let Some(found) = self.regexp_run(id, input)? {
                self.regexp_array_units(&found, input, original)
            } else { Ok(Value::Null) };
        }
        self.set_property_throw(id, "lastIndex".into(), Value::Number(0.0))?;
        let mut matches = Vec::new();
        while let Some(found) = self.regexp_run(id, input)? {
            if matches.len() == self.limits.heap.array_length {
                return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
            }
            matches.push(self.string_from_units(&input[found.start()..found.end()])?);
            if found.start() == found.end() {
                let next = regexp::advance_string_index(input, found.end(), expression.flags.unicode);
                self.set_property_throw(id, "lastIndex".into(), Value::Number(next as f64))?;
            }
        }
        if matches.is_empty() { Ok(Value::Null) } else { self.array_from_values(&matches) }
    }
}

#[cfg(test)]
#[path = "regexp_utf16_value_tests.rs"]
mod tests;
