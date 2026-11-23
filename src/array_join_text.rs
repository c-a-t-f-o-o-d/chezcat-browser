//! Exact joined strings with bounded provisional storage. A later unpaired
//! unit can select UTF-16 storage; adjacent halves can instead repair a pair.
//! Admit the final representation only when the complete output is known.
use super::{Fault, Runtime, Value};
use super::super::WorkKind;

enum Buffer {
    Scalar { text: String, units: usize },
    Units(Vec<u16>),
}

pub(super) struct JoinOutput {
    buffer: Buffer,
    owned: usize,
}

impl JoinOutput {
    pub(super) fn new() -> Self {
        Self { buffer: Buffer::Scalar { text: String::new(), units: 0 }, owned: 0 }
    }

    fn workspace_fault(runtime: &Runtime) -> Fault {
        Fault::new(format!("Array join workspace limit reached ({} bytes).", runtime.limits.heap.retained_values))
    }

    fn charge_growth(runtime: &mut Runtime, owned: &mut usize, old: usize, new: usize) -> Result<(), Fault> {
        let added = new.checked_sub(old).ok_or_else(|| Self::workspace_fault(runtime))?;
        let total = runtime.value_bytes.checked_add(added)
            .filter(|&total| total <= runtime.limits.heap.retained_values)
            .ok_or_else(|| Self::workspace_fault(runtime))?;
        *owned = owned.checked_add(added).ok_or_else(|| Self::workspace_fault(runtime))?;
        runtime.value_bytes = total;
        Ok(())
    }

    fn refund(runtime: &mut Runtime, owned: &mut usize, bytes: usize) -> Result<(), Fault> {
        let values = runtime.value_bytes.checked_sub(bytes);
        let remaining = owned.checked_sub(bytes);
        if let (Some(values), Some(remaining)) = (values, remaining) {
            runtime.value_bytes = values;
            *owned = remaining;
            Ok(())
        } else {
            Err(Fault::new("Array join workspace refund accounting failed."))
        }
    }

    fn reserve_scalar(
        runtime: &mut Runtime, text: &mut String, owned: &mut usize,
        required: usize, maximum: usize,
    ) -> Result<(), Fault> {
        if required <= text.capacity() { return Ok(()); }
        let old = text.capacity();
        let available = runtime.limits.heap.retained_values.saturating_sub(runtime.value_bytes);
        let maximum = maximum.min(old.saturating_add(available));
        if required > maximum { return Err(Self::workspace_fault(runtime)); }
        let target = required.max(old.saturating_mul(2).min(maximum));
        runtime.work(text.len())?;
        text.try_reserve_exact(target - text.len())
            .map_err(|_| Fault::new("Unable to allocate Array join text."))?;
        if text.capacity() > maximum { return Err(Self::workspace_fault(runtime)); }
        Self::charge_growth(runtime, owned, old, text.capacity())
    }

    fn reserve_units(
        runtime: &mut Runtime, units: &mut Vec<u16>, owned: &mut usize, required: usize,
    ) -> Result<(), Fault> {
        // Every final encoding needs at least one byte per code unit. This
        // provisional count bound admits ASCII and pairs that repair later,
        // while physical backing is separately charged to retained storage.
        if required > runtime.limits.capacity.string_bytes { return Err(runtime.string_limit_fault()); }
        if required <= units.capacity() { return Ok(()); }
        let old = units.capacity();
        let available = runtime.limits.heap.retained_values.saturating_sub(runtime.value_bytes);
        let maximum = runtime.limits.capacity.string_bytes.min(old.saturating_add(available / 2));
        if required > maximum { return Err(Self::workspace_fault(runtime)); }
        let target = required.max(old.saturating_mul(2).min(maximum));
        runtime.work(units.len().saturating_mul(2))?;
        units.try_reserve_exact(target - units.len())
            .map_err(|_| Fault::new("Unable to allocate Array join units."))?;
        if units.capacity() > maximum { return Err(Self::workspace_fault(runtime)); }
        let old_bytes = old.checked_mul(2).ok_or_else(|| Self::workspace_fault(runtime))?;
        let new_bytes = units.capacity().checked_mul(2).ok_or_else(|| Self::workspace_fault(runtime))?;
        Self::charge_growth(runtime, owned, old_bytes, new_bytes)
    }

    pub(super) fn append(&mut self, runtime: &mut Runtime, value: &Value) -> Result<(), Fault> {
        match value {
            Value::String(text) => self.append_scalar(runtime, text),
            Value::StringUnits(units) => self.append_units(runtime, units),
            _ => Err(Fault::new("Array join output requires an already converted string.")),
        }
    }

    fn append_scalar(&mut self, runtime: &mut Runtime, added: &str) -> Result<(), Fault> {
        if added.is_empty() { return Ok(()); }
        runtime.work_as(added.len(), WorkKind::CharacterScan)?;
        let count = added.encode_utf16().count();
        match &mut self.buffer {
            Buffer::Scalar { text, units } => {
                let required = text.len().checked_add(added.len()).ok_or_else(|| runtime.string_limit_fault())?;
                let next_units = units.checked_add(count).ok_or_else(|| runtime.string_limit_fault())?;
                let limit = runtime.limits.capacity.string_bytes;
                if required > limit && next_units > limit / 2 { return Err(runtime.string_limit_fault()); }
                // A scalar prefix may occupy three UTF-8 bytes per unit yet
                // later become an admissible two-byte-per-unit raw string.
                let maximum = limit.max((limit / 2).saturating_mul(3));
                runtime.work_as(added.len(), WorkKind::ConversionOutput)?;
                Self::reserve_scalar(runtime, text, &mut self.owned, required, maximum)?;
                text.push_str(added);
                *units = next_units;
            }
            Buffer::Units(units) => {
                let required = units.len().checked_add(count).ok_or_else(|| runtime.string_limit_fault())?;
                runtime.work_as(count.saturating_mul(2), WorkKind::ConversionOutput)?;
                Self::reserve_units(runtime, units, &mut self.owned, required)?;
                units.extend(added.encode_utf16());
            }
        }
        Ok(())
    }

    fn append_units(&mut self, runtime: &mut Runtime, added: &[u16]) -> Result<(), Fault> {
        if added.is_empty() { return Ok(()); }
        if let Buffer::Scalar { text, units } = &self.buffer {
            let required = units.checked_add(added.len()).ok_or_else(|| runtime.string_limit_fault())?;
            if required > runtime.limits.capacity.string_bytes { return Err(runtime.string_limit_fault()); }
            runtime.work_as(text.len().saturating_add(required.saturating_mul(2)), WorkKind::ConversionOutput)?;
            let mut output = Vec::new();
            // Keep both allocations charged until conversion drops the old
            // scalar backing. No callback allocation is credited here.
            Self::reserve_units(runtime, &mut output, &mut self.owned, required)?;
            output.extend(text.encode_utf16());
            output.extend_from_slice(added);
            let old = text.capacity();
            self.buffer = Buffer::Units(output);
            Self::refund(runtime, &mut self.owned, old)?;
        } else if let Buffer::Units(units) = &mut self.buffer {
            let required = units.len().checked_add(added.len()).ok_or_else(|| runtime.string_limit_fault())?;
            runtime.work_as(added.len().saturating_mul(2), WorkKind::ConversionOutput)?;
            Self::reserve_units(runtime, units, &mut self.owned, required)?;
            units.extend_from_slice(added);
        }
        Ok(())
    }

    pub(super) fn finish(&mut self, runtime: &mut Runtime) -> Result<Value, Fault> {
        match &mut self.buffer {
            Buffer::Scalar { text, .. } => {
                if text.len() > runtime.limits.capacity.string_bytes { return Err(runtime.string_limit_fault()); }
                Ok(Value::String(std::mem::take(text)))
            }
            Buffer::Units(units) => {
                runtime.work_as(units.len().saturating_mul(2), WorkKind::CharacterScan)?;
                let mut bytes = 0usize;
                for decoded in char::decode_utf16(units.iter().copied()) {
                    let Ok(ch) = decoded else {
                        if units.len() > runtime.limits.capacity.string_bytes / 2 {
                            return Err(runtime.string_limit_fault());
                        }
                        return Ok(Value::StringUnits(std::mem::take(units)));
                    };
                    bytes = bytes.checked_add(ch.len_utf8()).ok_or_else(|| runtime.string_limit_fault())?;
                }
                if bytes > runtime.limits.capacity.string_bytes { return Err(runtime.string_limit_fault()); }
                runtime.work_as(bytes, WorkKind::ConversionOutput)?;
                let mut text = String::new();
                Self::reserve_scalar(runtime, &mut text, &mut self.owned, bytes, runtime.limits.capacity.string_bytes)?;
                text.extend(char::decode_utf16(units.iter().copied()).map(|ch| ch.expect("validated joined UTF-16")));
                let old = units.capacity().checked_mul(2).ok_or_else(|| Self::workspace_fault(runtime))?;
                drop(std::mem::take(units));
                Self::refund(runtime, &mut self.owned, old)?;
                Ok(Value::String(text))
            }
        }
    }

    pub(super) fn release(&mut self, runtime: &mut Runtime) -> Result<(), Fault> {
        self.buffer = Buffer::Scalar { text: String::new(), units: 0 };
        let owned = self.owned;
        // Returned payloads leave temporary ownership; arena stores retain
        // their normal value charges. Failures drop all unfinished backing.
        Self::refund(runtime, &mut self.owned, owned)
    }

    pub(super) fn repeat(runtime: &mut Runtime, value: &Value, count: usize) -> Result<Value, Fault> {
        let mut output = Self::new();
        let result = (|| {
            if count == 0 || matches!(value, Value::String(text) if text.is_empty())
                || matches!(value, Value::StringUnits(units) if units.is_empty())
            {
                runtime.work(0)?;
                return Ok(Value::String(String::new()));
            }
            match value {
                Value::String(added) => {
                    let bytes = added.len().checked_mul(count).ok_or_else(|| runtime.string_limit_fault())?;
                    if bytes > runtime.limits.capacity.string_bytes { return Err(runtime.string_limit_fault()); }
                    runtime.work(bytes)?;
                    let Buffer::Scalar { text, .. } = &mut output.buffer else { unreachable!() };
                    Self::reserve_scalar(runtime, text, &mut output.owned, bytes, runtime.limits.capacity.string_bytes)?;
                    for _ in 0..count { text.push_str(added); }
                }
                Value::StringUnits(added) => {
                    let length = added.len().checked_mul(count).ok_or_else(|| runtime.string_limit_fault())?;
                    if length > runtime.limits.capacity.string_bytes { return Err(runtime.string_limit_fault()); }
                    runtime.work(length.saturating_mul(2))?;
                    output.buffer = Buffer::Units(Vec::new());
                    let Buffer::Units(units) = &mut output.buffer else { unreachable!() };
                    Self::reserve_units(runtime, units, &mut output.owned, length)?;
                    for _ in 0..count { units.extend_from_slice(added); }
                }
                _ => return Err(Fault::new("Array join separator requires an already converted string.")),
            }
            output.finish(runtime)
        })();
        output.release(runtime)?;
        result
    }
}
