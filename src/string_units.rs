//! ECMAScript strings that cannot be stored in a Rust UTF-8 String.
use super::{Fault, Runtime, TokenKind, Value, WorkKind};

pub(super) enum LiteralBuffer {
    Scalar(String),
    Units(Vec<u16>),
}

fn size_error(limit: usize) -> Fault {
    Fault::new(format!("String limit reached ({} KiB).", limit / 1024))
}

impl LiteralBuffer {
    pub(super) fn new() -> Self {
        Self::Scalar(String::new())
    }

    pub(super) fn push(&mut self, ch: char) {
        match self {
            Self::Scalar(text) => text.push(ch),
            Self::Units(units) => units.extend_from_slice(ch.encode_utf16(&mut [0; 2])),
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Scalar(text) => text.len(),
            Self::Units(units) => units.len().saturating_mul(2),
        }
    }

    pub(super) fn push_unpaired(&mut self, unit: u16, limit: usize) -> Result<(), Fault> {
        if let Self::Scalar(text) = self {
            let count = text.encode_utf16().count().saturating_add(1);
            if count.saturating_mul(2) > limit {
                return Err(size_error(limit));
            }
            let mut units = Vec::new();
            units
                .try_reserve_exact(count)
                .map_err(|_| Fault::new("String storage allocation failed."))?;
            units.extend(text.encode_utf16());
            *self = Self::Units(units);
        }
        if let Self::Units(units) = self {
            if units.len() >= limit / 2 {
                return Err(size_error(limit));
            }
            units.push(unit);
        }
        Ok(())
    }

    pub(super) fn finish(self, limit: usize) -> Result<TokenKind, Fault> {
        match self {
            Self::Scalar(text) => Ok(TokenKind::String(text)),
            Self::Units(units) => match canonical(units, limit)? {
                Value::String(s) => Ok(TokenKind::String(s)),
                Value::StringUnits(u) => Ok(TokenKind::StringUnits(u)),
                _ => unreachable!(),
            },
        }
    }
}

// A lone-code-unit string never compares equal to well-formed UTF-8. Joining
// two strings can repair a surrogate pair; canonicalize after composition.
fn scalar_bytes(units: &[u16], limit: usize) -> Result<Option<usize>, Fault> {
    let mut bytes = 0usize;
    for c in char::decode_utf16(units.iter().copied()) {
        let Ok(c) = c else {
            return if units.len().saturating_mul(2) <= limit {
                Ok(None)
            } else {
                Err(size_error(limit))
            };
        };
        bytes = bytes.saturating_add(c.len_utf8());
    }
    if bytes > limit {
        return Err(size_error(limit));
    }
    Ok(Some(bytes))
}

fn scalar_value(units: &[u16], bytes: usize) -> Result<Value, Fault> {
    let mut text = String::new();
    text.try_reserve_exact(bytes)
        .map_err(|_| Fault::new("String storage allocation failed."))?;
    text.extend(char::decode_utf16(units.iter().copied()).map(|c| c.expect("validated UTF-16")));
    Ok(Value::String(text))
}

pub(super) fn canonical(units: Vec<u16>, limit: usize) -> Result<Value, Fault> {
    match scalar_bytes(&units, limit)? {
        Some(bytes) => scalar_value(&units, bytes),
        None => Ok(Value::StringUnits(units)),
    }
}

impl Value {
    // Concatenating adjacent native Text nodes can repair a surrogate pair.
    pub(crate) fn from_host_string_units(
        units: Vec<u16>, limit: usize, admit_copy: impl FnOnce(usize) -> Result<(), String>,
    ) -> Result<Self, String> {
        match scalar_bytes(&units, limit).map_err(|fault| fault.message)? {
            Some(bytes) => {
                admit_copy(bytes)?;
                scalar_value(&units, bytes).map_err(|fault| fault.message)
            }
            None => Ok(Self::StringUnits(units)),
        }
    }

    pub(super) fn is_string(&self) -> bool {
        matches!(self, Self::String(_) | Self::StringUnits(_))
    }

    pub(super) fn string_length(&self) -> Option<usize> {
        match self {
            Self::String(text) => Some(text.encode_utf16().count()),
            Self::StringUnits(units) => Some(units.len()),
            _ => None,
        }
    }

    pub(crate) fn scalar_text(&self) -> Result<String, String> {
        if matches!(self, Self::StringUnits(_)) {
            return Err("This operation requires text without unpaired UTF-16 surrogates.".into());
        }
        Ok(self.to_text())
    }
}

impl Runtime {
    pub(super) fn string_from_units(&mut self, units: &[u16]) -> Result<Value, Fault> {
        let bytes = units.len().saturating_mul(2);
        if units.len() > self.limits.capacity.string_bytes {
            return Err(size_error(self.limits.capacity.string_bytes));
        }
        self.work_as(bytes, WorkKind::ConversionOutput)?;
        if let Some(size) = scalar_bytes(units, self.limits.capacity.string_bytes)? {
            return scalar_value(units, size);
        }
        let mut copy = Vec::new();
        copy.try_reserve_exact(units.len())
            .map_err(|_| Fault::new("String storage allocation failed."))?;
        copy.extend_from_slice(units);
        Ok(Value::StringUnits(copy))
    }

    pub(super) fn string_units_for_value(&mut self, value: &Value) -> Result<Vec<u16>, Fault> {
        self.work_as(value.bytes().saturating_mul(3), WorkKind::ConversionOutput)?;
        if let Value::StringUnits(units) = value {
            let mut copy = Vec::new();
            copy.try_reserve_exact(units.len())
                .map_err(|_| Fault::new("String storage allocation failed."))?;
            copy.extend_from_slice(units);
            return Ok(copy);
        }
        let converted;
        let scalar = if let Value::String(s) = value {
            s
        } else {
            converted = value.script_text()?;
            &converted
        };
        let mut units = Vec::new();
        units
            .try_reserve_exact(scalar.encode_utf16().count())
            .map_err(|_| Fault::new("String storage allocation failed."))?;
        units.extend(scalar.encode_utf16());
        Ok(units)
    }

    pub(super) fn concat_unit_values(
        &mut self,
        first: &Value,
        second: &Value,
    ) -> Result<Value, Fault> {
        let mut units = self.string_units_for_value(first)?;
        let other = self.string_units_for_value(second)?;
        let count = units.len().saturating_add(other.len());
        if count > self.limits.capacity.string_bytes {
            return Err(size_error(self.limits.capacity.string_bytes));
        }
        self.work_as(count.saturating_mul(2), WorkKind::ConcatenationScratch)?;
        units
            .try_reserve(other.len())
            .map_err(|_| Fault::new("String storage allocation failed."))?;
        units.extend(other);
        let value = canonical(units, self.limits.capacity.string_bytes)?;
        self.checked_value(value)
    }

    pub(super) fn raw_string_character(
        &mut self,
        units: &[u16],
        method: &str,
        args: &[Value],
    ) -> Result<Value, Fault> {
        let position = args.first().unwrap_or(&Value::Undefined);
        position.primitive()?;
        self.work_as(
            position.bytes().saturating_mul(2),
            WorkKind::ConversionOutput,
        )?;
        let number = position.script_number()?;
        let index = if number.is_nan() { 0.0 } else { number.trunc() };
        if index < 0.0 || index >= units.len() as f64 {
            return Ok(match method {
                "charAt" => Value::String(String::new()),
                "codePointAt" => Value::Undefined,
                _ => Value::Number(f64::NAN),
            });
        }
        let index = index as usize;
        self.work_as(2, WorkKind::CharacterScan)?;
        let first = units[index];
        if method == "charAt" {
            return self.string_from_units(&units[index..=index]);
        }
        if method == "codePointAt" && (0xd800..=0xdbff).contains(&first) {
            if let Some(&low) = units
                .get(index + 1)
                .filter(|low| (0xdc00..=0xdfff).contains(*low))
            {
                let code = 0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(low) - 0xdc00;
                return Ok(Value::Number(f64::from(code)));
            }
        }
        Ok(Value::Number(f64::from(first)))
    }
}
