//! Exact integer conversions over canonical signed decimal values.
//! https://tc39.es/ecma262/#sec-bigint-constructor
//! https://tc39.es/ecma262/#sec-islessthan
use super::{Builtin, Fault, Host, Runtime, Value, WorkKind, js_space, primitive_wrappers::Data};
use std::{cmp::Ordering, fmt::Write, sync::Arc};

const DECIMAL_BASE: u64 = 1_000_000_000;

pub(super) fn compare(left: &str, right: &str) -> Ordering {
    match (left.strip_prefix('-'), right.strip_prefix('-')) {
        (Some(left), Some(right)) => unsigned_compare(right, left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => unsigned_compare(left, right),
    }
}

fn unsigned_compare(left: &str, right: &str) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

fn compare_number(integer: &str, number: f64) -> Option<Ordering> {
    if number.is_nan() {
        return None;
    }
    if number.is_infinite() {
        return Some(if number.is_sign_positive() {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let whole = number.trunc();
    let digits = if whole == 0.0 {
        "0".to_owned()
    } else {
        format!("{whole:.0}")
    };
    let order = compare(integer, &digits);
    Some(if order.is_eq() && number != whole {
        if number.is_sign_negative() {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    } else {
        order
    })
}

impl Runtime {
    pub(super) fn initialize_big_int(&mut self) -> Result<(), Fault> {
        let prototype = self.new_object(None)?;
        self.big_int_prototype = Some(prototype);
        self.constructor_prototype("BigInt", Value::Object(prototype))?;
        self.store_symbol_property(
            prototype,
            self.symbol_to_string_tag
                .expect("well-known symbols initialized"),
            Value::String("BigInt".into()),
            Some((false, false, true)),
        )?;
        self.prototype_method(prototype, "toString", Builtin::BigIntMethod(true))?;
        self.prototype_method(prototype, "valueOf", Builtin::BigIntMethod(false))
    }

    pub(super) fn big_int_method(
        &mut self,
        to_string: bool,
        receiver: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = match receiver {
            Value::BigInt(value) => value,
            other => match self.wrapped_data(&other) {
                Some(Data::BigInt(value)) => value.clone(),
                _ => {
                    return Err(Fault::exception(
                        "TypeError",
                        "BigInt method requires a BigInt receiver.",
                    ));
                }
            },
        };
        if !to_string {
            return Ok(Value::BigInt(value));
        }
        let radix = match args.first() {
            None | Some(Value::Undefined) => 10.0,
            Some(radix) => self.binary_number(radix, host)?.trunc(),
        };
        if !(2.0..=36.0).contains(&radix) {
            return Err(Fault::exception(
                "RangeError",
                "BigInt radix must be between 2 and 36.",
            ));
        }
        self.work_as(value.len(), WorkKind::ConversionOutput)?;
        if radix == 10.0 {
            return Ok(Value::String(value.to_string()));
        }
        self.big_int_text_with_radix(&value, radix as u32)
    }

    fn big_int_text_with_radix(&mut self, value: &str, radix: u32) -> Result<Value, Fault> {
        if value == "0" {
            return Ok(Value::String("0".into()));
        }
        let digits = value.strip_prefix('-').unwrap_or(value);
        let count = digits.len().div_ceil(9);
        self.work_as(
            count.saturating_mul(size_of::<u32>()),
            WorkKind::NumberScratch,
        )?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|_| Fault::new("Cannot reserve BigInt conversion storage."))?;
        for chunk in digits.as_bytes().rchunks(9) {
            let chunk = std::str::from_utf8(chunk).expect("ASCII decimal digits");
            words.push(chunk.parse::<u32>().expect("canonical decimal digits"));
        }
        let mut output = String::new();
        while !words.is_empty() {
            let mut remainder = 0;
            for word in words.iter_mut().rev() {
                self.step()?;
                let dividend = remainder * DECIMAL_BASE + u64::from(*word);
                *word = (dividend / u64::from(radix)) as u32;
                remainder = dividend % u64::from(radix);
            }
            if words.last() == Some(&0) {
                words.pop();
            }
            if output.len() >= self.limits.capacity.string_bytes {
                return Err(Fault::new(
                    "BigInt string conversion exceeds the runtime value limit.",
                ));
            }
            self.work_as(1, WorkKind::ConversionOutput)?;
            output
                .try_reserve(1)
                .map_err(|_| Fault::new("Cannot reserve BigInt string conversion output."))?;
            output.push(char::from_digit(remainder as u32, radix).expect("remainder within radix"));
        }
        if value.starts_with('-') {
            if output.len() >= self.limits.capacity.string_bytes {
                return Err(Fault::new(
                    "BigInt string conversion exceeds the runtime value limit.",
                ));
            }
            output
                .try_reserve(1)
                .map_err(|_| Fault::new("Cannot reserve BigInt string conversion output."))?;
            output.push('-');
        }
        self.work_as(output.len(), WorkKind::ConversionOutput)?;
        Ok(Value::String(
            output.bytes().rev().map(char::from).collect(),
        ))
    }

    pub(super) fn big_int(&mut self, value: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let value = self.primitive_with_hint(value, false, host)?;
        self.work_as(value.bytes(), WorkKind::NumberScratch)?;
        let text = match value {
            Value::BigInt(_) => return Ok(value),
            Value::Bool(false) => "0".into(),
            Value::Bool(true) => "1".into(),
            Value::Number(number) if number == 0.0 => "0".into(),
            Value::Number(number) if number.is_finite() && number.fract() == 0.0 => {
                format!("{number:.0}")
            }
            Value::Number(_) => {
                return Err(Fault::exception(
                    "RangeError",
                    "BigInt requires a finite integral Number.",
                ));
            }
            Value::String(text) => self.parse_big_int(&text)?.ok_or_else(invalid_string)?,
            Value::StringUnits(_) => return Err(invalid_string()),
            _ => {
                return Err(Fault::exception(
                    "TypeError",
                    "The value cannot be converted to BigInt.",
                ));
            }
        };
        if text.len() > self.limits.capacity.string_bytes {
            return Err(Fault::new(
                "BigInt decimal representation exceeds the runtime value limit.",
            ));
        }
        self.work_as(text.len(), WorkKind::NumberScratch)?;
        Ok(Value::BigInt(Arc::from(text)))
    }

    pub(super) fn big_int_order(
        &mut self,
        left: &Value,
        right: &Value,
    ) -> Result<Option<Ordering>, Fault> {
        self.work_as(
            left.bytes().saturating_add(right.bytes()),
            WorkKind::BinaryInspection,
        )?;
        match (left, right) {
            (Value::BigInt(left), Value::BigInt(right)) => Ok(Some(compare(left, right))),
            (Value::BigInt(integer), other) => self.big_int_order_with(integer, other),
            (other, Value::BigInt(integer)) => Ok(self
                .big_int_order_with(integer, other)?
                .map(Ordering::reverse)),
            _ => unreachable!("comparison requires a BigInt"),
        }
    }

    fn big_int_order_with(
        &mut self,
        integer: &str,
        other: &Value,
    ) -> Result<Option<Ordering>, Fault> {
        match other {
            Value::String(text) => Ok(self
                .parse_big_int(text)?
                .map(|other| compare(integer, &other))),
            Value::StringUnits(_) => Ok(None),
            _ => Ok(compare_number(integer, other.script_number()?)),
        }
    }

    fn parse_big_int(&mut self, source: &str) -> Result<Option<String>, Fault> {
        self.work_as(source.len(), WorkKind::NumberScratch)?;
        let text = source.trim_matches(js_space);
        if text.is_empty() {
            return Ok(Some("0".into()));
        }
        for (prefix, radix) in [
            ("0x", 16),
            ("0X", 16),
            ("0o", 8),
            ("0O", 8),
            ("0b", 2),
            ("0B", 2),
        ] {
            if let Some(digits) = text.strip_prefix(prefix) {
                return self.big_int_positional(digits, radix);
            }
        }
        let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Ok(None);
        }
        let digits = digits.trim_start_matches('0');
        Ok(Some(if digits.is_empty() {
            "0".into()
        } else if text.starts_with('-') {
            format!("-{digits}")
        } else {
            digits.to_owned()
        }))
    }

    fn big_int_positional(&mut self, text: &str, radix: u32) -> Result<Option<String>, Fault> {
        if text.is_empty() || !text.bytes().all(|byte| (byte as char).is_digit(radix)) {
            return Ok(None);
        }
        // Keep decimal chunks for conversion from a power-of-two radix. No
        // rounding or conversion through a floating-point accumulator.
        let mut words = Vec::<u32>::new();
        let width = match radix {
            16 => 7,
            8 => 9,
            _ => 28,
        };
        let text = text.trim_start_matches('0');
        let first = match text.len() % width {
            0 => width,
            short => short,
        };
        let mut at = 0;
        while at < text.len() {
            self.step()?;
            let end = at + if at == 0 { first } else { width };
            let mut carry =
                u64::from(u32::from_str_radix(&text[at..end], radix).expect("validated digits"));
            let base = u64::from(radix.pow((end - at) as u32));
            for word in &mut words {
                self.step()?;
                let total = u64::from(*word) * base + carry;
                *word = (total % DECIMAL_BASE) as u32;
                carry = total / DECIMAL_BASE;
            }
            if carry != 0 {
                self.work_as(std::mem::size_of::<u32>() * 2, WorkKind::NumberScratch)?;
                words
                    .try_reserve(1)
                    .map_err(|_| Fault::new("Cannot reserve BigInt conversion storage."))?;
                words.push(carry as u32);
            }
            at = end;
        }
        let Some(last) = words.pop() else {
            return Ok(Some("0".into()));
        };
        let capacity = (words.len() + 1).saturating_mul(9);
        if capacity.saturating_sub(8) > self.limits.capacity.string_bytes {
            return Err(Fault::new(
                "BigInt decimal representation exceeds the runtime value limit.",
            ));
        }
        self.work_as(capacity, WorkKind::NumberScratch)?;
        let mut result = last.to_string();
        result
            .try_reserve(capacity.saturating_sub(result.len()))
            .map_err(|_| Fault::new("Cannot reserve BigInt decimal representation."))?;
        for word in words.into_iter().rev() {
            write!(result, "{word:09}").expect("String formatting cannot fail");
        }
        if result.len() > self.limits.capacity.string_bytes {
            return Err(Fault::new(
                "BigInt decimal representation exceeds the runtime value limit.",
            ));
        }
        Ok(Some(result))
    }
}

fn invalid_string() -> Fault {
    Fault::exception("SyntaxError", "The string does not represent a BigInt.")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;

    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("unexpected native read")
        }

        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("unexpected native write")
        }

        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("unexpected native call")
        }
    }

    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn big_int_construction_and_comparison_preserve_exact_integer_values() {
        succeeds(
            r#"
var min = BigInt(Number.MIN_SAFE_INTEGER), max = BigInt(Number.MAX_SAFE_INTEGER);
if (typeof min !== 'bigint' || String(min) !== '-9007199254740991') throw 'min';
if (String(max) !== '9007199254740991' || Object.is(min, max)) throw 'max';
if (String(BigInt(1000000000000000100)) !== '1000000000000000128') throw 'exact binary64';
var large = BigInt('0x20000000000001');
if (large.toString() !== '9007199254740993' || large.toString(16) !== '20000000000001') throw 'large';
if (large != '9007199254740993' || large == 9007199254740992 || !(large > 9007199254740992)) throw 'mixed';
if (large === '9007199254740993' || large < 'wrong' || large >= 'wrong') throw 'string compare';
if (!(BigInt(-2) > -2.5 && -2.5 < BigInt(-2) && BigInt(2) < 2.5)) throw 'fraction';
if (BigInt(' -000 ') || !BigInt(true) || BigInt('') !== BigInt(false)) throw 'zero';
if (!Object.is(BigInt(true), BigInt('0o1')) || Number(large) !== 9007199254740992) throw 'Number';
if (BigInt('0b101111').toString(36) !== '1b' || BigInt(-31).toString(16) !== '-1f') throw 'radix';
if (BigInt({valueOf: function(){return max;}}) !== max) throw 'coercion';
'ok';
"#,
        );
    }

    #[test]
    fn big_int_wrappers_and_protocols_use_a_distinct_primitive() {
        succeeds(
            r#"
var value = BigInt(7), wrapped = Object(value);
if (typeof wrapped !== 'object' || wrapped.valueOf() !== value ||
    Object.getPrototypeOf(value) !== BigInt.prototype || BigInt.prototype.constructor !== BigInt)
    throw 'wrapper';
if (Object.prototype.toString.call(value) !== '[object BigInt]' ||
    Object.prototype.toString.call(wrapped) !== '[object BigInt]') throw 'tag';
var keys = new Map();
keys.set(value, 'bigint'); keys.set(7, 'number');
if (keys.get(7) !== 'number' || keys.get(BigInt('7')) !== 'bigint') throw 'map key';
function rejects(fn, name) {
    try {fn();} catch(e) { if (e.name === name) return; throw e; }
    throw name + ' missing';
}
rejects(function(){BigInt('-0x1');}, 'SyntaxError');
rejects(function(){BigInt(0.5);}, 'RangeError');
rejects(function(){BigInt(Infinity);}, 'RangeError');
rejects(function(){BigInt(null);}, 'TypeError');
rejects(function(){new BigInt(7);}, 'TypeError');
rejects(function(){value + 1;}, 'TypeError');
rejects(function(){value.toString(1);}, 'RangeError');
rejects(function(){BigInt.prototype.valueOf();}, 'TypeError');
rejects(function(){JSON.stringify(wrapped);}, 'TypeError');
BigInt.prototype.toJSON = function(){return String(this);};
if (JSON.stringify(value) !== '"7"') throw 'JSON hook';
'ok';
"#,
        );
    }
}
