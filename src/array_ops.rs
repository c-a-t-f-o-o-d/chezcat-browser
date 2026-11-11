//! Additional original array operations. Reads recheck inherited indices and
//! holes; writes use Set/Delete with Throw=true. Callback traversal captures
//! length but observes later element changes, with the existing job limits.

use super::{strict_equal, Fault, Host, Ordinary, Runtime, Value, MAX_ARRAY_LENGTH};
use std::cmp::Ordering;

impl Runtime {
    pub(super) fn array_like_length(
        &mut self,
        id: usize,
        host: &mut dyn Host,
    ) -> Result<usize, Fault> {
        if let Some(length) = self.object(id)?.length {
            return Ok(length);
        }
        let length = self.binary_source_length(&Value::Object(id), host)?;
        if length > MAX_ARRAY_LENGTH {
            return Err(Fault::new("Array-like length limit reached (8192)."));
        }
        Ok(length)
    }
    pub(super) fn array_like_join(
        &mut self,
        id: usize,
        length: usize,
        separator: &str,
        host: &mut dyn Host,
    ) -> Result<String, Fault> {
        let mut output = String::new();
        for index in 0..length {
            self.step()?;
            if index != 0 {
                self.append_string(&mut output, separator)?;
            }
            let value = self.get_property_with_host(id, &index.to_string(), host)?;
            if !matches!(value, Value::Null | Value::Undefined) {
                let text = self.primitive_with_hint(value, true, host)?.script_text()?;
                self.append_string(&mut output, &text)?;
            }
        }
        Ok(output)
    }
    fn array_has_index(&mut self, id: usize, index: usize) -> Result<bool, Fault> {
        Ok(self
            .property_owner(Ordinary::Object(id), &index.to_string())?
            .is_some())
    }
    fn array_get_index(
        &mut self,
        id: usize,
        index: usize,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.get_property_with_host(id, &index.to_string(), host)
    }
    fn array_put_index(
        &mut self,
        id: usize,
        index: usize,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let previous = self.strict;
        self.strict = true;
        let result =
            self.assign_ordinary_property(Ordinary::Object(id), index.to_string(), value, host);
        self.strict = previous;
        result
    }
    pub(super) fn array_delete_index(&mut self, id: usize, index: usize) -> Result<(), Fault> {
        let key = index.to_string();
        if self.typed_view(id).is_some_and(|view| index < view.length) {
            return Err(Fault::exception(
                "TypeError",
                "Cannot delete a typed-array element.",
            ));
        }
        if self.objects[id]
            .properties
            .get(&key)
            .is_some_and(|property| !property.configurable)
        {
            return Err(Fault::exception(
                "TypeError",
                "Cannot delete a nonconfigurable array element.",
            ));
        }
        self.remove_property(id, &key)
    }
    pub(super) fn array_length_throw(
        &mut self,
        id: usize,
        length: usize,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let previous = self.strict;
        self.strict = true;
        let result = self.assign_ordinary_property(
            Ordinary::Object(id),
            "length".into(),
            Value::Number(length as f64),
            host,
        );
        self.strict = previous;
        result
    }
    fn array_move_index(
        &mut self,
        id: usize,
        source: usize,
        target: usize,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if self.array_has_index(id, source)? {
            let value = self.array_get_index(id, source, host)?;
            self.array_put_index(id, target, value, host)
        } else {
            self.array_delete_index(id, target)
        }
    }
    pub(super) fn extra_array_method(
        &mut self,
        id: usize,
        length: usize,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        match method {
            "shift" => {
                let first = if length == 0 {
                    Value::Undefined
                } else {
                    self.array_get_index(id, 0, host)?
                };
                for index in 1..length {
                    self.array_move_index(id, index, index - 1, host)?;
                }
                if length > 0 {
                    self.array_delete_index(id, length - 1)?;
                }
                self.array_length_throw(id, length.saturating_sub(1), host)?;
                Ok(first)
            }
            "unshift" => {
                if length.saturating_add(args.len()) > MAX_ARRAY_LENGTH {
                    return Err(Fault::new("Array length limit reached (8192)."));
                }
                if !args.is_empty() {
                    for index in (0..length).rev() {
                        self.array_move_index(id, index, index + args.len(), host)?;
                    }
                    for (index, value) in args.iter().enumerate() {
                        let value = self.copy_value(value)?;
                        self.array_put_index(id, index, value, host)?;
                    }
                }
                self.array_length_throw(id, length + args.len(), host)?;
                Ok(Value::Number((length + args.len()) as f64))
            }
            "splice" | "toSpliced" => {
                self.array_splice(id, length, method == "toSpliced", args, host)
            }
            "at" => {
                let index = self.binary_integer(args.first(), 0.0, host)?;
                let index = if index < 0.0 {
                    length as f64 + index
                } else {
                    index
                };
                if index < 0.0 || index >= length as f64 {
                    Ok(Value::Undefined)
                } else {
                    self.array_get_index(id, index as usize, host)
                }
            }
            "lastIndexOf" => {
                if length == 0 {
                    return Ok(Value::Number(-1.0));
                }
                let needle = args.first().unwrap_or(&Value::Undefined);
                let index = if args.len() < 2 {
                    length as f64 - 1.0
                } else {
                    self.binary_integer(args.get(1), 0.0, host)?
                };
                let index = if index < 0.0 {
                    length as f64 + index
                } else {
                    index
                };
                if index >= 0.0 {
                    for index in (0..=index.min(length as f64 - 1.0) as usize).rev() {
                        if self.array_has_index(id, index)? {
                            let value = self.array_get_index(id, index, host)?;
                            self.work(value.bytes().saturating_add(needle.bytes()))?;
                            if strict_equal(needle, &value) {
                                return Ok(Value::Number(index as f64));
                            }
                        }
                    }
                }
                Ok(Value::Number(-1.0))
            }
            "fill" => {
                let start = self.binary_relative(args.get(1), length, 0, host)?;
                let end = self.binary_relative(args.get(2), length, length, host)?;
                for index in start..end {
                    let value = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                    self.array_put_index(id, index, value, host)?;
                }
                Ok(Value::Object(id))
            }
            "copyWithin" => {
                let target = self.binary_relative(args.first(), length, 0, host)?;
                let start = self.binary_relative(args.get(1), length, 0, host)?;
                let end = self.binary_relative(args.get(2), length, length, host)?;
                let count = end.saturating_sub(start).min(length - target);
                for next in 0..count {
                    let step = if start < target && target < start + count {
                        count - 1 - next
                    } else {
                        next
                    };
                    self.array_move_index(id, start + step, target + step, host)?;
                }
                Ok(Value::Object(id))
            }
            "reverse" => {
                for left in 0..length / 2 {
                    let right = length - 1 - left;
                    let has_left = self.array_has_index(id, left)?;
                    let left_value = if has_left {
                        Some(self.array_get_index(id, left, host)?)
                    } else {
                        None
                    };
                    let has_right = self.array_has_index(id, right)?;
                    let right_value = if has_right {
                        Some(self.array_get_index(id, right, host)?)
                    } else {
                        None
                    };
                    match (left_value, right_value) {
                        (Some(a), Some(b)) => {
                            self.array_put_index(id, left, b, host)?;
                            self.array_put_index(id, right, a, host)?;
                        }
                        (None, Some(b)) => {
                            self.array_put_index(id, left, b, host)?;
                            self.array_delete_index(id, right)?;
                        }
                        (Some(a), None) => {
                            self.array_delete_index(id, left)?;
                            self.array_put_index(id, right, a, host)?;
                        }
                        (None, None) => {}
                    }
                }
                Ok(Value::Object(id))
            }
            "toReversed" | "with" => {
                let replacement = if method == "with" {
                    let index = self.binary_integer(args.first(), 0.0, host)?;
                    let index = if index < 0.0 {
                        length as f64 + index
                    } else {
                        index
                    };
                    if index < 0.0 || index >= length as f64 {
                        return Err(Fault::exception(
                            "RangeError",
                            "Array with index is out of range.",
                        ));
                    }
                    Some(index as usize)
                } else {
                    None
                };
                let result = self.new_object(Some(length))?;
                for index in 0..length {
                    let value = if replacement == Some(index) {
                        self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?
                    } else {
                        self.array_get_index(
                            id,
                            if method == "toReversed" {
                                length - 1 - index
                            } else {
                                index
                            },
                            host,
                        )?
                    };
                    self.set_property(result, index.to_string(), value)?;
                }
                Ok(Value::Object(result))
            }
            "concat" => {
                let result = self.new_object(Some(0))?;
                let mut at = 0;
                for source in std::iter::once(&Value::Object(id)).chain(args) {
                    if let Value::Object(source) = source {
                        if let Some(length) = self.object(*source)?.length {
                            if at + length > MAX_ARRAY_LENGTH {
                                return Err(Fault::new("Array length limit reached (8192)."));
                            }
                            for index in 0..length {
                                if self.array_has_index(*source, index)? {
                                    let value = self.array_get_index(*source, index, host)?;
                                    self.set_property(result, (at + index).to_string(), value)?;
                                }
                            }
                            at += length;
                            continue;
                        }
                    }
                    if at == MAX_ARRAY_LENGTH {
                        return Err(Fault::new("Array length limit reached (8192)."));
                    }
                    let value = self.copy_value(source)?;
                    self.set_property(result, at.to_string(), value)?;
                    at += 1;
                }
                self.set_array_length(result, at)?;
                Ok(Value::Object(result))
            }
            "sort" | "toSorted" => {
                let comparator = args
                    .first()
                    .filter(|value| !matches!(value, Value::Undefined));
                if comparator.is_some_and(|value| !matches!(value, Value::Function(_))) {
                    return Err(Fault::exception(
                        "TypeError",
                        "Array comparator must be callable.",
                    ));
                }
                let mut values = Vec::new();
                for index in 0..length {
                    if method == "toSorted" || self.array_has_index(id, index)? {
                        let value = self.array_get_index(id, index, host)?;
                        self.work(8usize.saturating_add(value.bytes()))?;
                        values.push(value);
                    }
                }
                self.sort_native_values(&mut values, comparator, false, host)?;
                let target = if method == "toSorted" {
                    self.new_object(Some(length))?
                } else {
                    id
                };
                let count = values.len();
                for (index, value) in values.into_iter().enumerate() {
                    self.array_put_index(target, index, value, host)?;
                }
                for index in count..length {
                    self.array_delete_index(target, index)?;
                }
                Ok(Value::Object(target))
            }
            "map" | "filter" | "forEach" | "some" | "every" | "find" | "findIndex" | "findLast"
            | "findLastIndex" | "reduce" | "reduceRight" => {
                self.array_callback(id, length, method, args, host)
            }
            _ => Err(Fault::new("Unsupported array method.")),
        }
    }
    fn array_splice(
        &mut self,
        id: usize,
        length: usize,
        copy: bool,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let start = self.binary_relative(args.first(), length, 0, host)?;
        let deleted = if args.is_empty() {
            0
        } else if args.len() == 1 {
            length - start
        } else {
            self.binary_integer(args.get(1), 0.0, host)?
                .max(0.0)
                .min((length - start) as f64) as usize
        };
        let items = args.get(2..).unwrap_or(&[]);
        let added = items.len();
        let new_length = length - deleted + added;
        if new_length > MAX_ARRAY_LENGTH {
            return Err(Fault::new("Array length limit reached (8192)."));
        }
        if copy {
            let result = self.new_object(Some(new_length))?;
            for index in 0..new_length {
                let value = if index >= start && index < start + added {
                    self.copy_value(&items[index - start])?
                } else {
                    self.array_get_index(
                        id,
                        if index < start {
                            index
                        } else {
                            index + deleted - added
                        },
                        host,
                    )?
                };
                self.set_property(result, index.to_string(), value)?;
            }
            return Ok(Value::Object(result));
        }
        let result = self.new_object(Some(deleted))?;
        for index in 0..deleted {
            if self.array_has_index(id, start + index)? {
                let value = self.array_get_index(id, start + index, host)?;
                self.set_property(result, index.to_string(), value)?;
            }
        }
        if added < deleted {
            for index in start..length - deleted {
                self.array_move_index(id, index + deleted, index + added, host)?;
            }
            for index in (new_length..length).rev() {
                self.array_delete_index(id, index)?;
            }
        } else if added > deleted {
            for index in (start..length - deleted).rev() {
                self.array_move_index(id, index + deleted, index + added, host)?;
            }
        }
        for (index, value) in items.iter().enumerate() {
            let value = self.copy_value(value)?;
            self.array_put_index(id, start + index, value, host)?;
        }
        self.array_length_throw(id, new_length, host)?;
        Ok(Value::Object(result))
    }
    fn array_callback(
        &mut self,
        id: usize,
        length: usize,
        method: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let callback = args.first().unwrap_or(&Value::Undefined);
        if !matches!(callback, Value::Function(_)) {
            return Err(Fault::exception(
                "TypeError",
                "Array callback must be callable.",
            ));
        }
        let reduce = matches!(method, "reduce" | "reduceRight");
        let reverse = matches!(method, "findLast" | "findLastIndex" | "reduceRight");
        let visit_holes = matches!(method, "find" | "findIndex" | "findLast" | "findLastIndex");
        let result = if method == "map" {
            Some(self.new_object(Some(length))?)
        } else if method == "filter" {
            Some(self.new_object(Some(0))?)
        } else {
            None
        };
        let receiver = if reduce {
            Value::Undefined
        } else {
            self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?
        };
        let mut accumulator = if reduce { args.get(1).cloned() } else { None };
        let mut kept = 0;
        for next in 0..length {
            let index = if reverse { length - 1 - next } else { next };
            if !visit_holes && !self.array_has_index(id, index)? {
                continue;
            }
            let value = self.array_get_index(id, index, host)?;
            if reduce && accumulator.is_none() {
                accumulator = Some(value);
                continue;
            }
            let values = if reduce {
                vec![
                    self.copy_value(accumulator.as_ref().unwrap())?,
                    self.copy_value(&value)?,
                    Value::Number(index as f64),
                    Value::Object(id),
                ]
            } else {
                vec![
                    self.copy_value(&value)?,
                    Value::Number(index as f64),
                    Value::Object(id),
                ]
            };
            let output = self.call_value(callback.clone(), &values, receiver.clone(), host)?;
            match method {
                "map" => self.set_property(result.unwrap(), index.to_string(), output)?,
                "filter" if output.truthy() => {
                    self.set_property(result.unwrap(), kept.to_string(), value)?;
                    kept += 1;
                }
                "some" if output.truthy() => return Ok(Value::Bool(true)),
                "every" if !output.truthy() => return Ok(Value::Bool(false)),
                "find" | "findLast" if output.truthy() => return Ok(value),
                "findIndex" | "findLastIndex" if output.truthy() => {
                    return Ok(Value::Number(index as f64))
                }
                "reduce" | "reduceRight" => accumulator = Some(output),
                _ => {}
            }
        }
        Ok(match method {
            "map" | "filter" => Value::Object(result.unwrap()),
            "reduce" | "reduceRight" => accumulator.ok_or_else(|| {
                Fault::exception(
                    "TypeError",
                    "Cannot reduce an empty array without an initial value.",
                )
            })?,
            "every" => Value::Bool(true),
            "some" => Value::Bool(false),
            "findIndex" | "findLastIndex" => Value::Number(-1.0),
            _ => Value::Undefined,
        })
    }
    pub(super) fn sort_native_values(
        &mut self,
        values: &mut [Value],
        comparator: Option<&Value>,
        numeric: bool,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        // Iterative stable merge sort permits fallible script comparators and
        // keeps native stack depth constant. Every temporary copy is charged.
        let mut width = 1usize;
        while width < values.len() {
            self.work(values.len().saturating_mul(std::mem::size_of::<Value>()))?;
            let mut merged = Vec::with_capacity(values.len());
            for start in (0..values.len()).step_by(width * 2) {
                let middle = (start + width).min(values.len());
                let end = (middle + width).min(values.len());
                let (mut left, mut right) = (start, middle);
                while left < middle || right < end {
                    let take_left = right == end
                        || left < middle
                            && self.compare_array_values(
                                &values[left],
                                &values[right],
                                comparator,
                                numeric,
                                host,
                            )? != Ordering::Greater;
                    let index = if take_left {
                        let index = left;
                        left += 1;
                        index
                    } else {
                        let index = right;
                        right += 1;
                        index
                    };
                    merged.push(self.copy_value(&values[index])?);
                }
            }
            for (destination, value) in values.iter_mut().zip(merged) {
                *destination = value;
            }
            width *= 2;
        }
        Ok(())
    }
    fn compare_array_values(
        &mut self,
        a: &Value,
        b: &Value,
        comparator: Option<&Value>,
        numeric: bool,
        host: &mut dyn Host,
    ) -> Result<Ordering, Fault> {
        self.step()?;
        if !numeric {
            if matches!(a, Value::Undefined) {
                return Ok(if matches!(b, Value::Undefined) {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                });
            }
            if matches!(b, Value::Undefined) {
                return Ok(Ordering::Less);
            }
        }
        if let Some(comparator) = comparator {
            let a = self.copy_value(a)?;
            let b = self.copy_value(b)?;
            let value = self.call_value(comparator.clone(), &[a, b], Value::Undefined, host)?;
            let number = self.binary_number(&value, host)?;
            return Ok(if number.is_nan() || number == 0.0 {
                Ordering::Equal
            } else if number < 0.0 {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        if numeric {
            let (Value::Number(a), Value::Number(b)) = (a, b) else {
                return Err(Fault::new("Invalid numeric sort values."));
            };
            return Ok(if a.is_nan() {
                if b.is_nan() {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            } else if b.is_nan() {
                Ordering::Less
            } else if *a == 0.0 && *b == 0.0 {
                b.is_sign_negative().cmp(&a.is_sign_negative())
            } else {
                a.partial_cmp(b).unwrap()
            });
        }
        let a = self.copy_value(a)?;
        let a = self.primitive_with_hint(a, true, host)?;
        self.work(a.bytes())?;
        // Complete ToString on the left before invoking conversion callbacks
        // on the right. A Symbol result is an abrupt completion here.
        let a = a.script_text()?;
        let b = self.copy_value(b)?;
        let b = self.primitive_with_hint(b, true, host)?;
        self.work(b.bytes())?;
        let b = b.script_text()?;
        self.work(a.len().saturating_add(b.len()).saturating_mul(2))?;
        Ok(a.encode_utf16().cmp(b.encode_utf16()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn default_comparison_finishes_left_string_conversion_before_right_callback() {
        for object_left in [false, true] {
            let mut runtime = Runtime::new();
            let mut host = NoHost;
            runtime
                .execute(
                    "var leftCalls=0,rightCalls=0;var key=Symbol('left');",
                    &mut host,
                )
                .unwrap();
            let left = runtime
                .execute(
                    if object_left {
                        "({toString(){leftCalls++;return key;}});"
                    } else {
                        "key;"
                    },
                    &mut host,
                )
                .unwrap();
            let right = runtime
                .execute("({toString(){rightCalls++;return 'right';}});", &mut host)
                .unwrap();
            let fault = runtime
                .compare_array_values(&left, &right, None, false, &mut host)
                .unwrap_err();
            assert!(matches!(
                fault.exception.as_deref(),
                Some(super::super::Exception::Error("TypeError"))
            ));
            assert_eq!(
                runtime
                    .execute("leftCalls+':'+rightCalls;", &mut host)
                    .unwrap()
                    .to_text(),
                if object_left { "1:0" } else { "0:0" }
            );

            // A supplied comparator consumes the values directly. The default
            // string-conversion rule must not reject its Symbol arguments.
            let comparator = runtime
                .execute("(function(){return 0;});", &mut host)
                .unwrap();
            assert_eq!(
                runtime
                    .compare_array_values(&left, &right, Some(&comparator), false, &mut host)
                    .unwrap(),
                Ordering::Equal
            );
            assert_eq!(
                runtime.execute("rightCalls;", &mut host).unwrap().to_text(),
                "0"
            );
        }
    }
}
