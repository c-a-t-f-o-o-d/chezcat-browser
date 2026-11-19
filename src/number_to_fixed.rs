//! Original bounded binary64 fixed-point conversion.
//!
//! Compute round(abs(x) * 10^f) using the exact binary significand, not a
//! floating multiplication or the shortest decimal representation. A discarded
//! half bit rounds upward, as required even when the integral part is even.
//! https://tc39.es/ecma262/#sec-number.prototype.tofixed

use super::{Fault, Runtime, Value, WorkKind};

// For abs(x) < 10^21 and f <= 100 the scaled integer needs at most 403 bits.
// The pre-shift product significand * 5^f is smaller still (286 bits).
const LIMBS: usize = 14;
const MAX_DIGITS: usize = 122;

struct Coefficient {
    words: [u32; LIMBS],
    len: usize,
}

fn bound() -> Fault {
    Fault::new("Bounded Number fixed-point arithmetic exceeded its binary64 storage.")
}

impl Coefficient {
    fn new(value: u64) -> Self {
        let mut words = [0; LIMBS];
        words[0] = value as u32;
        words[1] = (value >> 32) as u32;
        let len = if words[1] != 0 { 2 } else { usize::from(words[0] != 0) };
        Self { words, len }
    }

    fn multiply_five(&mut self, runtime: &mut Runtime) -> Result<(), Fault> {
        let mut carry = 0u64;
        for word in &mut self.words[..self.len] {
            runtime.step()?;
            let value = u64::from(*word) * 5 + carry;
            *word = value as u32;
            carry = value >> 32;
        }
        if carry != 0 {
            runtime.step()?;
            if self.len == LIMBS { return Err(bound()); }
            self.words[self.len] = carry as u32;
            self.len += 1;
        }
        Ok(())
    }

    fn shift_left(&mut self, shift: usize, runtime: &mut Runtime) -> Result<(), Fault> {
        if self.len == 0 || shift == 0 { return Ok(()); }
        let whole = shift / 32;
        let part = shift % 32;
        let high = usize::from(part != 0 && self.words[self.len - 1] >> (32 - part) != 0);
        let length = self.len.checked_add(whole).and_then(|n| n.checked_add(high)).ok_or_else(bound)?;
        if length > LIMBS { return Err(bound()); }
        for index in (0..length).rev() {
            runtime.step()?;
            let source = index.checked_sub(whole);
            let low = source.filter(|&i| i < self.len).map_or(0, |i| self.words[i] << part);
            let high = if part == 0 { 0 } else {
                source.and_then(|i| i.checked_sub(1)).filter(|&i| i < self.len)
                    .map_or(0, |i| self.words[i] >> (32 - part))
            };
            self.words[index] = low | high;
        }
        self.len = length;
        Ok(())
    }

    fn shift_right_rounded(&mut self, shift: usize, runtime: &mut Runtime) -> Result<(), Fault> {
        if shift == 0 { return Ok(()); }
        runtime.step()?;
        let half = shift - 1;
        let round_up = half / 32 < self.len && self.words[half / 32] & (1 << (half % 32)) != 0;
        let whole = shift / 32;
        let part = shift % 32;
        let length = self.len.saturating_sub(whole);
        for index in 0..length {
            runtime.step()?;
            let source = index + whole;
            let low = self.words[source] >> part;
            let high = if part != 0 && source + 1 < self.len {
                self.words[source + 1] << (32 - part)
            } else { 0 };
            self.words[index] = low | high;
        }
        self.len = length;
        self.normalize(runtime)?;
        if round_up {
            let mut index = 0;
            loop {
                runtime.step()?;
                if index == self.len {
                    if index == LIMBS { return Err(bound()); }
                    self.words[index] = 1;
                    self.len += 1;
                    break;
                }
                let (value, carry) = self.words[index].overflowing_add(1);
                self.words[index] = value;
                if !carry { break; }
                index += 1;
            }
        }
        Ok(())
    }

    fn normalize(&mut self, runtime: &mut Runtime) -> Result<(), Fault> {
        while self.len != 0 && self.words[self.len - 1] == 0 {
            runtime.step()?;
            self.len -= 1;
        }
        Ok(())
    }

    fn decimal_digit(&mut self, runtime: &mut Runtime) -> Result<u8, Fault> {
        let mut remainder = 0u64;
        for index in (0..self.len).rev() {
            runtime.step()?;
            let value = (remainder << 32) | u64::from(self.words[index]);
            self.words[index] = (value / 10) as u32;
            remainder = value % 10;
        }
        self.normalize(runtime)?;
        Ok(b'0' + remainder as u8)
    }
}

impl Runtime {
    #[inline(never)]
    pub(super) fn number_to_fixed(&mut self, value: f64, digits: usize) -> Result<Value, Fault> {
        self.step()?;
        if digits > 100 { return Err(bound()); }
        // Reuse the established shortest-decimal model for this specified
        // fallback. Its result is bounded by binary64's decimal exponent.
        if !value.is_finite() || value.abs() >= 1e21 {
            return self.checked_value(Value::String(super::number_text(value)));
        }
        self.work_as(std::mem::size_of::<Coefficient>() + MAX_DIGITS, WorkKind::NumberScratch)?;
        let bits = value.abs().to_bits();
        let exponent = ((bits >> 52) & 2047) as i32;
        let fraction = bits & ((1u64 << 52) - 1);
        let significand = if exponent == 0 { fraction } else { fraction | (1u64 << 52) };
        let exponent = if exponent == 0 { -1074 } else { exponent - 1075 };
        let mut coefficient = Coefficient::new(significand);
        for _ in 0..digits {
            self.step()?;
            coefficient.multiply_five(self)?;
        }
        let shift = exponent + digits as i32;
        if shift < 0 {
            coefficient.shift_right_rounded((-shift) as usize, self)?;
        } else {
            coefficient.shift_left(shift as usize, self)?;
        }
        let mut reversed = [0; MAX_DIGITS];
        let mut count = 0;
        loop {
            self.step()?;
            if count == MAX_DIGITS { return Err(bound()); }
            reversed[count] = coefficient.decimal_digit(self)?;
            count += 1;
            if coefficient.len == 0 { break; }
        }
        // Negative zero has no sign; a negative nonzero rounded to zero does.
        let negative = value < 0.0;
        let width = count.max(digits + 1);
        let length = width + usize::from(digits != 0) + usize::from(negative);
        if length > self.limits.capacity.string_bytes {
            return Err(Fault::new("Number fixed-point output exceeds the string limit."));
        }
        self.work_as(length, WorkKind::ConversionOutput)?;
        // Refuse before output allocation if the exact emission cannot finish.
        for _ in 0..length { self.step()?; }
        let mut text = String::new();
        text.try_reserve_exact(length).map_err(|_| Fault::new("Unable to allocate bounded Number fixed-point output."))?;
        if negative { text.push('-'); }
        for index in (0..width).rev() {
            if digits != 0 && index + 1 == digits { text.push('.'); }
            text.push(if index < count { reversed[index] as char } else { '0' });
        }
        debug_assert_eq!(text.len(), length);
        Ok(Value::String(text))
    }
}
