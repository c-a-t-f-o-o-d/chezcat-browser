//! Reverse substring search over UTF-16 code units, not UTF-8 byte offsets.
//! https://tc39.es/ecma262/#sec-string.prototype.lastindexof
use super::{bounded_index, Fault, Host, Runtime, Value, WorkKind};

impl Runtime {
    pub(super) fn string_last_index_of(
        &mut self,
        receiver: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // StringMethod already performed RequireObjectCoercible/ToString on
        // the receiver. Keep its value across search/position callbacks.
        self.work_as(
            receiver.len().saturating_mul(3),
            WorkKind::StringMethodInput,
        )?;
        let search = {
            let value = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
            let value = self.primitive_with_hint(value, true, host)?;
            // Existing scalar ToString has a small finite formatting bound;
            // Strings reserve their complete owned copy before conversion.
            let bytes = if let Value::String(text) = &value {
                text.len()
            } else {
                64
            };
            self.work_as(bytes, WorkKind::ConversionOutput)?;
            let Value::String(text) = self.checked_value(Value::String(value.script_text()?))?
            else {
                unreachable!("ToString produces a String");
            };
            text
        };
        // Do not use binary_integer: NaN means search from the end here, not
        // zero. This conversion also runs for empty or too-long patterns.
        let position = self.binary_number(args.get(1).unwrap_or(&Value::Undefined), host)?;
        self.work_as(search.len().saturating_mul(2), WorkKind::StringMethodInput)?;

        // UTF-8 byte length is a finite upper bound on UTF-16 unit count. These
        // two buffers contain only copied scalar data and need no JS GC roots.
        let mut units = Vec::new();
        units
            .try_reserve_exact(receiver.len())
            .map_err(|_| Fault::new("String search input reservation failed."))?;
        units.extend(receiver.encode_utf16());
        let mut needle = Vec::new();
        needle
            .try_reserve_exact(search.len())
            .map_err(|_| Fault::new("String search pattern reservation failed."))?;
        needle.extend(search.encode_utf16());
        let Some(max_start) = units.len().checked_sub(needle.len()) else {
            return Ok(Value::Number(-1.0));
        };
        let start = if position.is_nan() {
            max_start
        } else {
            bounded_index(position, max_start)
        };
        if needle.is_empty() {
            return Ok(Value::Number(start as f64));
        }
        for at in (0..=start).rev() {
            self.step()?;
            // As with the existing forward search, debit a whole comparison
            // before it runs. Repeated prefixes cannot hide quadratic work.
            self.work(needle.len().saturating_mul(2))?;
            if units[at..at + needle.len()] == needle {
                return Ok(Value::Number(at as f64));
            }
        }
        Ok(Value::Number(-1.0))
    }
}
