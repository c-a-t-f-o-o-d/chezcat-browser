//! Original character-position conversion over captured exact strings.
//! https://tc39.es/ecma262/#sec-string.prototype.charat
//! https://tc39.es/ecma262/#sec-string.prototype.charcodeat
//! https://tc39.es/ecma262/#sec-string.prototype.codepointat
use super::{Fault,Host,Runtime,Value};

impl Runtime {
    pub(super) fn string_character_value(
        &mut self,receiver:&Value,method:&str,args:&[Value],host:&mut dyn Host,
    )->Result<Value,Fault> {
        self.step()?;
        // Receiver conversion already completed on the native method route.
        // Keep that immutable scalar/raw value while the numeric hook runs;
        // conversion is mandatory even for empty and out-of-range receivers.
        let position=self.binary_number(args.first().unwrap_or(&Value::Undefined),host)?;
        let position=[Value::Number(position)];
        // Existing consumers own exact unit results, pre-cast range checks,
        // scalar scan charging, raw indexing and output admission. Passing the
        // converted number cannot invoke another author hook.
        match receiver {
            Value::String(text)=>self.string_character(text,method,&position),
            Value::StringUnits(units)=>self.raw_string_character(units,method,&position),
            _=>Err(Fault::new("Character access requires an already converted string.")),
        }
    }
}
