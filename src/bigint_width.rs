//! Exact modulo powers of two over canonical decimal BigInts.
//! https://tc39.es/ecma262/#sec-bigint.asintn
//! https://tc39.es/ecma262/#sec-bigint.asuintn
use super::*;

impl Runtime {
    pub(in crate::javascript) fn big_int_width(
        &mut self,
        signed: bool,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let bits = self.binary_integer(args.first(), 0.0, host)?;
        if !(0.0..=9007199254740991.0).contains(&bits) {
            return Err(Fault::exception(
                "RangeError",
                "BigInt width is outside the integer index range.",
            ));
        }
        let value = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
        let primitive = self.primitive_with_hint(value, false, host)?;
        if matches!(primitive, Value::Number(_)) {
            return Err(Fault::exception(
                "TypeError",
                "BigInt width conversion does not accept a Number value.",
            ));
        }
        let Value::BigInt(value) = self.big_int(primitive, host)? else {
            unreachable!()
        };
        let bits = bits as u64;
        if bits == 0 {
            return Ok(Value::BigInt(Arc::from("0")));
        }
        if value.as_ref() == "0" {
            return Ok(Value::BigInt(value));
        }
        let negative = value.starts_with('-');
        let digits = value.trim_start_matches('-');
        // Every decimal digit contributes fewer than four binary bits. A huge
        // width need not materialize a modulus when the answer is unchanged.
        if bits > (digits.len() as u64).saturating_mul(4) && (signed || !negative) {
            return Ok(Value::BigInt(value));
        }
        // For a negative unsigned result beyond the magnitude's bit bound,
        // the output itself has this width. Reject impossible output before
        // reserving its limbs; this is a conservative lower digit bound.
        if negative
            && !signed
            && bits > (digits.len() as u64).saturating_mul(4)
            && bits / 4 > self.limits.capacity.string_bytes as u64
        {
            return Err(Fault::new(
                "BigInt decimal representation exceeds the runtime value limit.",
            ));
        }
        let count = usize::try_from(bits.div_ceil(32))
            .map_err(|_| Fault::new("BigInt limb count is not representable."))?;
        let mut words = Vec::new();
        let mut decimal = Vec::new();
        let mut owned = 0;
        // No author call or VM arena collection occurs after conversion. Both
        // scratch vectors share the live retained-value admission and refund
        // their exact capacities on success, cancellation and every failure.
        let result = self.big_int_width_words(
            signed,
            negative,
            digits,
            bits,
            count,
            &mut words,
            &mut decimal,
            &mut owned,
        );
        drop(words);
        drop(decimal);
        self.value_bytes = self
            .value_bytes
            .checked_sub(owned)
            .expect("owned BigInt workspace charge");
        result
    }

    fn big_int_width_grow(
        &mut self,
        words: &mut Vec<u32>,
        additional: usize,
        owned: &mut usize,
    ) -> Result<(), Fault> {
        let available = self
            .limits
            .heap
            .retained_values
            .checked_sub(self.value_bytes)
            .ok_or_else(|| Fault::new("BigInt workspace storage exhausted."))?;
        let growth = super::super::limits::prepare_retained_growth(words, additional, available)?;
        let movement = if growth.added == 0 {
            0
        } else {
            words
                .len()
                .checked_mul(4)
                .ok_or_else(|| Fault::new("BigInt workspace movement overflow."))?
        };
        self.work_as(
            growth
                .added
                .checked_add(movement)
                .ok_or_else(|| Fault::new("BigInt workspace work overflow."))?,
            WorkKind::NumberScratch,
        )?;
        let next = self
            .value_bytes
            .checked_add(growth.added)
            .ok_or_else(|| Fault::new("BigInt workspace charge overflow."))?;
        let next_owned = owned
            .checked_add(growth.added)
            .ok_or_else(|| Fault::new("BigInt owned workspace overflow."))?;
        growth.commit(words);
        self.value_bytes = next;
        *owned = next_owned;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn big_int_width_words(
        &mut self,
        signed: bool,
        negative: bool,
        digits: &str,
        bits: u64,
        count: usize,
        words: &mut Vec<u32>,
        decimal: &mut Vec<u32>,
        owned: &mut usize,
    ) -> Result<Value, Fault> {
        let first = match digits.len() % 9 {
            0 => 9,
            n => n,
        };
        let mut at = 0;
        while at < digits.len() {
            self.step()?;
            let end = at + if at == 0 { first } else { 9 };
            let mut carry = u64::from(
                digits[at..end]
                    .parse::<u32>()
                    .expect("canonical decimal chunk"),
            );
            let base = 10u64.pow((end - at) as u32);
            for word in words.iter_mut() {
                self.step()?;
                let total = u64::from(*word) * base + carry;
                *word = total as u32;
                carry = total >> 32;
            }
            if carry != 0 && words.len() < count {
                self.big_int_width_grow(words, 1, owned)?;
                words.push(carry as u32);
            }
            at = end;
        }
        if negative {
            self.big_int_width_negate(words, bits, count, owned)?;
        }
        mask(words, bits, count);
        let output_negative =
            signed && words.len() == count && words[count - 1] & (1u32 << ((bits - 1) % 32)) != 0;
        if output_negative {
            self.big_int_width_negate(words, bits, count, owned)?;
        }
        trim(words);
        if words.is_empty() {
            return Ok(Value::BigInt(Arc::from("0")));
        }
        while !words.is_empty() {
            let mut remainder = 0u64;
            for word in words.iter_mut().rev() {
                self.step()?;
                let dividend = (remainder << 32) | u64::from(*word);
                *word = (dividend / DECIMAL_BASE) as u32;
                remainder = dividend % DECIMAL_BASE;
            }
            trim(words);
            self.big_int_width_grow(decimal, 1, owned)?;
            decimal.push(remainder as u32);
        }
        let last = decimal.pop().expect("nonzero magnitude");
        let length = decimal
            .len()
            .checked_mul(9)
            .and_then(|n| n.checked_add(last.ilog10() as usize + 1))
            .and_then(|n| n.checked_add(usize::from(output_negative)))
            .ok_or_else(|| Fault::new("BigInt output length overflow."))?;
        if length > self.limits.capacity.string_bytes {
            return Err(Fault::new(
                "BigInt decimal representation exceeds the runtime value limit.",
            ));
        }
        let available = self
            .limits
            .heap
            .retained_values
            .checked_sub(self.value_bytes)
            .ok_or_else(|| Fault::new("BigInt output storage exhausted."))?;
        let peak = length
            .checked_mul(2)
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .filter(|n| *n <= available)
            .ok_or_else(|| Fault::new("BigInt output storage exhausted."))?;
        self.work_as(peak, WorkKind::ConversionOutput)?;
        let mut output = String::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| Fault::new("Cannot reserve BigInt width output."))?;
        if output
            .capacity()
            .checked_add(length)
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .is_none_or(|n| n > available)
        {
            return Err(Fault::new(
                "BigInt output allocation exceeds storage allowance.",
            ));
        }
        if output_negative {
            output.push('-');
        }
        write!(output, "{last}").expect("String formatting");
        for word in decimal.iter().rev() {
            self.step()?;
            write!(output, "{word:09}").expect("String formatting");
        }
        Ok(Value::BigInt(Arc::from(output)))
    }

    fn big_int_width_negate(
        &mut self,
        words: &mut Vec<u32>,
        bits: u64,
        count: usize,
        owned: &mut usize,
    ) -> Result<(), Fault> {
        self.big_int_width_grow(words, count - words.len(), owned)?;
        self.work_as(
            (count - words.len()).saturating_mul(4),
            WorkKind::NumberScratch,
        )?;
        words.resize(count, 0);
        let mut carry = true;
        for word in words.iter_mut() {
            self.step()?;
            let (next, overflow) = (!*word).overflowing_add(u32::from(carry));
            *word = next;
            carry = overflow;
        }
        mask(words, bits, count);
        Ok(())
    }
}
fn mask(words: &mut [u32], bits: u64, count: usize) {
    if words.len() == count && bits % 32 != 0 {
        words[count - 1] &= (1u32 << (bits % 32)) - 1;
    }
}
fn trim(words: &mut Vec<u32>) {
    while words.last() == Some(&0) {
        words.pop();
    }
}
