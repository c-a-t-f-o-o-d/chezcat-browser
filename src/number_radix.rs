//! Original bounded binary64 -> positional radix formatter.
//! Exact rounding intervals; no decimal parsing, float remainder or engine helper.
//! https://tc39.es/ecma262/2025/multipage/ecmascript-data-types-and-values.html#sec-numeric-types-number-tostring
use super::{Fault, Runtime, Value, WorkKind, MAX_STRING_BYTES};
use std::cmp::Ordering;

const LIMBS: usize = 36; // 1152 bits; intermediates are below 36 * 2^1076.
const MAX_DIGITS: usize = 128;
const MAX_TEXT: usize = 1100;
const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

struct Natural {
    words: [u32; LIMBS],
    len: usize,
}

fn bound() -> Fault {
    Fault::new("Bounded Number radix arithmetic exceeded its binary64 storage.")
}

impl Natural {
    fn shifted(value: u64, shift: usize, runtime: &mut Runtime) -> Result<Self, Fault> {
        runtime.step()?;
        runtime.work(std::mem::size_of::<Self>())?;
        let mut out = Self {
            words: [0; LIMBS],
            len: 0,
        };
        let mut index = shift / 32;
        let mut bits = (value as u128) << (shift % 32);
        while bits != 0 {
            runtime.step()?;
            if index >= LIMBS {
                return Err(bound());
            }
            out.words[index] = bits as u32;
            bits >>= 32;
            index += 1;
            out.len = index;
        }
        Ok(out)
    }

    fn copy(&self, runtime: &mut Runtime) -> Result<Self, Fault> {
        runtime.step()?;
        runtime.work(std::mem::size_of::<Self>())?;
        Ok(Self {
            words: self.words,
            len: self.len,
        })
    }

    fn compare(&self, other: &Self, runtime: &mut Runtime) -> Result<Ordering, Fault> {
        runtime.step()?;
        if self.len != other.len {
            return Ok(self.len.cmp(&other.len));
        }
        for index in (0..self.len).rev() {
            runtime.step()?;
            let order = self.words[index].cmp(&other.words[index]);
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        Ok(Ordering::Equal)
    }

    fn multiply(&mut self, multiplier: u32, runtime: &mut Runtime) -> Result<(), Fault> {
        let mut carry = 0u64;
        for index in 0..self.len {
            runtime.step()?;
            let value = self.words[index] as u64 * multiplier as u64 + carry;
            self.words[index] = value as u32;
            carry = value >> 32;
        }
        if carry != 0 {
            runtime.step()?;
            if self.len == LIMBS {
                return Err(bound());
            }
            self.words[self.len] = carry as u32;
            self.len += 1;
        }
        Ok(())
    }

    // Precondition self >= other, established by the exact quotient/interval checks.
    fn subtract(&mut self, other: &Self, runtime: &mut Runtime) -> Result<(), Fault> {
        let mut borrow = 0u64;
        for index in 0..self.len {
            runtime.step()?;
            let sub = other.words[index] as u64 + borrow;
            let old = self.words[index] as u64;
            self.words[index] = old.wrapping_sub(sub) as u32;
            borrow = u64::from(old < sub);
        }
        if borrow != 0 {
            return Err(bound());
        }
        while self.len != 0 && self.words[self.len - 1] == 0 {
            runtime.step()?;
            self.len -= 1;
        }
        Ok(())
    }
}

impl Runtime {
    #[inline(never)]
    pub(super) fn number_radix_string(&mut self, value: f64, radix: u32) -> Result<Value, Fault> {
        if !(2..=36).contains(&radix) || radix == 10 || !value.is_finite() || value == 0.0 {
            return Err(Fault::new(
                "Invalid finite nondecimal Number conversion request.",
            ));
        }
        let bits = value.abs().to_bits();
        let exponent_bits = ((bits >> 52) & 2047) as i32;
        let fraction = bits & ((1u64 << 52) - 1);
        let significand = if exponent_bits == 0 {
            fraction
        } else {
            fraction | (1u64 << 52)
        };
        let exponent = if exponent_bits == 0 {
            -1074
        } else {
            exponent_bits - 1075
        };
        let inclusive = significand & 1 == 0;
        // At an exact normal power of two above MIN_NORMAL, the preceding gap
        // is half the following gap. Subnormal spacing is uniform.
        let lower = if fraction == 0 && exponent_bits > 1 {
            1
        } else {
            2
        };
        let binary_shift = exponent - 2;
        let numerator_shift = binary_shift.max(0) as usize;
        let denominator_shift = (-binary_shift).max(0) as usize;
        let mut remainder = Natural::shifted(significand * 4, numerator_shift, self)?;
        let mut low_margin = Natural::shifted(lower, numerator_shift, self)?;
        let mut high_margin = Natural::shifted(2, numerator_shift, self)?;
        let mut denominator = Natural::shifted(1, denominator_shift, self)?;
        let mut point = 1i32;
        if remainder.compare(&denominator, self)? == Ordering::Less {
            while remainder.compare(&denominator, self)? == Ordering::Less {
                self.step()?;
                if point <= -(MAX_TEXT as i32) {
                    return Err(bound());
                }
                remainder.multiply(radix, self)?;
                low_margin.multiply(radix, self)?;
                high_margin.multiply(radix, self)?;
                point -= 1;
            }
        } else {
            loop {
                self.step()?;
                let mut next = denominator.copy(self)?;
                next.multiply(radix, self)?;
                if remainder.compare(&next, self)? == Ordering::Less {
                    break;
                }
                if point >= MAX_TEXT as i32 {
                    return Err(bound());
                }
                denominator = next;
                point += 1;
            }
        }

        self.work(MAX_DIGITS)?;
        let mut digits = [0u8; MAX_DIGITS];
        let mut count = 0usize;
        let mut coefficient_parity = 0u32;
        loop {
            self.step()?;
            if count == MAX_DIGITS {
                return Err(bound());
            }
            let mut digit = 0u32;
            while remainder.compare(&denominator, self)? != Ordering::Less {
                remainder.subtract(&denominator, self)?;
                digit += 1;
                if digit >= radix {
                    return Err(bound());
                }
            }
            digits[count] = digit as u8;
            count += 1;
            coefficient_parity = (coefficient_parity * (radix & 1) + digit) & 1;
            let low = remainder.compare(&low_margin, self)?;
            let low_ok = low == Ordering::Less || inclusive && low == Ordering::Equal;
            let mut distance = denominator.copy(self)?;
            distance.subtract(&remainder, self)?;
            let high = distance.compare(&high_margin, self)?;
            let high_ok = high == Ordering::Less || inclusive && high == Ordering::Equal;
            if low_ok || high_ok {
                let round_up = if !low_ok {
                    true
                } else if !high_ok {
                    false
                } else {
                    let mut twice = remainder.copy(self)?;
                    twice.multiply(2, self)?;
                    match twice.compare(&denominator, self)? {
                        Ordering::Less => false,
                        Ordering::Greater => true,
                        Ordering::Equal => coefficient_parity != 0,
                    }
                };
                if round_up {
                    let mut cursor = count;
                    loop {
                        self.step()?;
                        if cursor == 0 {
                            if count == MAX_DIGITS {
                                return Err(bound());
                            }
                            digits[0] = 1;
                            digits[count] = 0;
                            count += 1;
                            point += 1;
                            break;
                        }
                        cursor -= 1;
                        if digits[cursor] as u32 + 1 < radix {
                            digits[cursor] += 1;
                            break;
                        }
                        digits[cursor] = 0;
                    }
                }
                break;
            }
            remainder.multiply(radix, self)?;
            low_margin.multiply(radix, self)?;
            high_margin.multiply(radix, self)?;
        }
        while count > 1 && digits[count - 1] == 0 {
            self.step()?;
            count -= 1;
        }
        let unsigned = if point <= 0 {
            2 + (-point) as usize + count
        } else if point as usize >= count {
            point as usize
        } else {
            count + 1
        };
        let length = unsigned + usize::from(value.is_sign_negative());
        if length > MAX_TEXT || length > MAX_STRING_BYTES {
            return Err(bound());
        }
        // Charge output exactly once, before allocation. Arithmetic scratch
        // copies were charged separately and executed work is never refunded.
        self.work_as(length, WorkKind::ConversionOutput)?;
        for _ in 0..length {
            self.step()?;
        }
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| Fault::new("Unable to allocate bounded Number radix output."))?;
        if value.is_sign_negative() {
            text.push('-');
        }
        if point <= 0 {
            text.push_str("0.");
            for _ in 0..-point {
                text.push('0');
            }
        }
        for (index, &digit) in digits[..count].iter().enumerate() {
            if point > 0 && index == point as usize {
                text.push('.');
            }
            text.push(DIGITS[digit as usize] as char);
        }
        if point > count as i32 {
            for _ in count as i32..point {
                text.push('0');
            }
        }
        debug_assert_eq!(text.len(), length);
        Ok(Value::String(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl super::super::Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("unexpected get")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("unexpected set")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("unexpected call")
        }
    }

    #[test]
    fn radix_method_preserves_brands_coercion_order_special_values_and_errors() {
        let mut runtime = Runtime::new();
        runtime.execute(r#"
var seen='',hits=0,marker={},wrapped=Object(255);
wrapped.valueOf=function(){throw 'receiver valueOf';};
wrapped[Symbol.toPrimitive]=function(){throw 'receiver primitive';};
var radix={get [Symbol.toPrimitive](){seen+='get:';return function(hint){seen+=hint;hits++;return 16.9;};}};
if(Number.prototype.toString.call(wrapped,radix)!=='ff'||seen!=='get:number'||hits!==1)throw 'radix order';
seen='';hits=0;try{Number.prototype.toString.call({},radix);throw 'missing brand';}catch(e){if(e.name!=='TypeError')throw e;}
if(seen!==''||hits!==0)throw 'brand before radix';
var thrown=false;try{(12).toString({valueOf:function(){throw marker;}});}catch(e){thrown=e===marker;}
if(!thrown)throw 'radix throw identity';
var errors=0;for(var bad of [0,1,37,NaN,Infinity,-Infinity,null,4294967312]){
for(var n of [NaN,Infinity,-Infinity,-0,255]){try{Number.prototype.toString.call(n,bad);}catch(e){if(e.name==='RangeError')errors++;else throw e;}}}
if(errors!==40)throw 'invalid special radix';
try{(12).toString(Symbol());throw 'missing symbol';}catch(e){if(e.name!=='TypeError')throw e;}
if((255).toString(16.9)!=='ff'||(35).toString(36)!=='z'||(36).toString(36)!=='10'||(-255).toString(16)!=='-ff')throw 'integers';
if((10.5).toString(2)!=='1010.1'||(0.125).toString(16)!=='0.2'||(0.5).toString(36)!=='0.i'||(1/3).toString(3)!=='0.1')throw 'fractions';
if((255).toString(undefined)!=='255'||(255).toString(10)!=='255')throw 'decimal unchanged';
if(NaN.toString(16)!=='NaN'||Infinity.toString(2)!=='Infinity'||(-Infinity).toString(36)!=='-Infinity'||(-0).toString(2)!=='0')throw 'special strings';
hits=0;var ignored={valueOf:function(){hits++;throw 'ignored argument';}};
if(Boolean.prototype.toString.call(false,ignored)!=='false'||Number.prototype.valueOf.call(12,ignored)!==12||hits!==0)throw 'ignored arguments';
'ok';
"#, &mut NoHost).unwrap();
    }

    #[test]
    fn extreme_exact_binary_and_hex_results_have_bounded_fixed_notation() {
        let mut runtime = Runtime::new();
        for (number, radix, expected) in [
            (f64::MAX, 2, "1".repeat(53) + &"0".repeat(971)),
            (
                f64::from_bits(1),
                2,
                "0.".to_owned() + &"0".repeat(1073) + "1",
            ),
            (
                f64::from_bits(1),
                16,
                "0.".to_owned() + &"0".repeat(268) + "4",
            ),
            (
                f64::MIN_POSITIVE,
                16,
                "0.".to_owned() + &"0".repeat(255) + "4",
            ),
            (2.0f64.powi(100), 16, "1".to_owned() + &"0".repeat(25)),
            (1e21, 36, "5v1j4f4ds7a000".to_owned()),
        ] {
            runtime.begin_job();
            let Value::String(actual) = runtime.number_radix_string(number, radix).unwrap() else {
                unreachable!()
            };
            assert_eq!(actual, expected);
            assert!(actual.len() <= MAX_TEXT && !actual.contains('+'));
        }
    }

    #[test]
    fn incompatible_string_wrapper_brand_does_not_copy_payload_before_type_error() {
        let mut runtime = Runtime::new();
        let receiver = runtime.box_string("payload".into()).unwrap();
        runtime.copy_bytes = 0;
        let error = runtime
            .primitive_value_method(
                "toString",
                true,
                receiver,
                &[Value::Number(16.0)],
                &mut NoHost,
            )
            .unwrap_err();
        assert!(matches!(
            error.exception.as_deref(),
            Some(super::super::Exception::Error("TypeError"))
        ));
        assert_eq!(runtime.copy_bytes, 0);
    }

    #[test]
    fn radix_arithmetic_and_output_budgets_fail_without_arena_effects_or_deadline_reset() {
        let mut probe = Runtime::new();
        probe.begin_job();
        let initial = (probe.steps, probe.copy_bytes);
        let Value::String(text) = probe.number_radix_string(0.1, 3).unwrap() else {
            unreachable!()
        };
        let cost = (initial.0 - probe.steps, initial.1 - probe.copy_bytes);
        for failure in ["steps", "late-steps", "copy", "output", "deadline"] {
            let mut runtime = Runtime::new();
            runtime.begin_job();
            let before = (
                runtime.functions.len(),
                runtime.objects.len(),
                runtime.environments.len(),
                runtime.bindings,
                runtime.value_bytes,
            );
            match failure {
                "steps" => runtime.steps = 0,
                "late-steps" => runtime.steps = cost.0 - 1,
                "copy" => runtime.copy_bytes = 0,
                "output" => runtime.copy_bytes = cost.1 - 1,
                "deadline" => {
                    runtime.job_deadline =
                        std::time::Instant::now() - std::time::Duration::from_secs(1)
                }
                _ => unreachable!(),
            }
            let deadline = runtime.job_deadline;
            let error = runtime.number_radix_string(0.1, 3).unwrap_err();
            assert!(error.exception.is_none());
            assert_eq!(
                (
                    runtime.functions.len(),
                    runtime.objects.len(),
                    runtime.environments.len(),
                    runtime.bindings,
                    runtime.value_bytes
                ),
                before
            );
            assert_eq!(runtime.job_deadline, deadline);
            if matches!(failure, "steps" | "late-steps" | "deadline") {
                assert_eq!(runtime.steps, 0);
            } else {
                assert_eq!(runtime.copy_bytes, 0);
            }
            if failure == "output" {
                let report = runtime.work_attribution.failure_report().unwrap();
                assert!(
                    report.contains(&format!(
                        "category=conversion-output; requested={}; remaining-before={}",
                        text.len(),
                        text.len() - 1
                    )),
                    "{report}"
                );
            }
            assert!(runtime.transient_registration_is_empty());
            runtime.begin_job();
            assert_eq!(runtime.number_radix_string(7.0, 16).unwrap().to_text(), "7");
        }
        let mut runtime = Runtime::new();
        runtime.begin_job();
        runtime.copy_bytes = cost.1;
        assert_eq!(runtime.number_radix_string(0.1, 3).unwrap().to_text(), text);
        assert_eq!(runtime.copy_bytes, 0);
        assert!(runtime.work(1).is_err());
        let report = runtime.work_attribution.failure_report().unwrap();
        assert!(
            report.contains(&format!(
                "conversion-output: bytes={}; calls=1.",
                text.len()
            )),
            "{report}"
        );
    }

    #[test]
    fn original_radix_conversion_and_recovery_fit_one_mib_with_live_script_calls() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut runtime = Runtime::new();
            assert_eq!(runtime.execute("function descend(n){if(n>0)return descend(n-1);return (255).toString(16);}descend(20);", &mut NoHost).unwrap().to_text(), "ff");
            assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
            runtime.begin_job();
            let text = runtime.number_radix_string(-f64::from_bits(1), 2).unwrap().to_text();
            assert_eq!(text.len(), 1077);
            assert!(text.starts_with("-0.") && text.ends_with('1'));
            assert!(runtime.transient_registration_is_empty());
        }).unwrap().join().unwrap();
    }
    #[test]
    fn all_radices_match_original_independent_exact_interval_witnesses() {
        let mut runtime = Runtime::new();
        let before = (
            runtime.functions.len(),
            runtime.objects.len(),
            runtime.bindings,
            runtime.value_bytes,
        );
        let mut count = 0;
        for line in include_str!("../tools/reference/number-radix-056/expected.tsv").lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut fields = line.split('\t');
            let bits = u64::from_str_radix(fields.next().unwrap(), 16).unwrap();
            let radix = fields.next().unwrap().parse::<u32>().unwrap();
            let expected = fields.next().unwrap();
            assert!(fields.next().is_none());
            runtime.begin_job();
            let Value::String(actual) = runtime
                .number_radix_string(f64::from_bits(bits), radix)
                .unwrap()
            else {
                unreachable!()
            };
            assert_eq!(actual, expected, "bits={bits:016x}; radix={radix}");
            count += 1;
        }
        assert!(count > 9_000);
        assert_eq!(
            (
                runtime.functions.len(),
                runtime.objects.len(),
                runtime.bindings,
                runtime.value_bytes
            ),
            before
        );
    }
}
