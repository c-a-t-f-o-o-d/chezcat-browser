//! Original bounded whitespace trimming with exact UTF-16 output.
//! https://tc39.es/ecma262/#sec-string.prototype.trim
use super::{js_space,Fault,Runtime,Value,WorkKind};

impl Runtime {
    pub(super) fn string_trim_value(&mut self,receiver:&Value)->Result<Value,Fault> {
        self.step()?;
        // Generic receiver conversion has completed. No argument conversion
        // or author call belongs to whitespace selection or output copying.
        self.work_as(receiver.bytes().saturating_mul(3),WorkKind::StringMethodInput)?;
        match receiver {
            Value::String(text)=>{
                let mut selected=text.as_str();
                while !selected.is_empty() {
                    self.step()?;self.work_as(4,WorkKind::CharacterScan)?;
                    let ch=selected.chars().next().expect("remaining scalar");
                    if !js_space(ch){break;}selected=&selected[ch.len_utf8()..];
                }
                while !selected.is_empty() {
                    self.step()?;self.work_as(4,WorkKind::CharacterScan)?;
                    let ch=selected.chars().next_back().expect("remaining scalar");
                    if !js_space(ch){break;}selected=&selected[..selected.len()-ch.len_utf8()];
                }
                self.string_units_output(selected.encode_utf16(),selected.encode_utf16().count(),selected.len(),"String trim")
            }
            Value::StringUnits(units)=>{
                let (mut start,mut end)=(0,units.len());
                while start<end {
                    self.step()?;self.work_as(2,WorkKind::CharacterScan)?;
                    if !char::from_u32(u32::from(units[start])).is_some_and(js_space){break;}start+=1;
                }
                while start<end {
                    self.step()?;self.work_as(2,WorkKind::CharacterScan)?;
                    if !char::from_u32(u32::from(units[end-1])).is_some_and(js_space){break;}end-=1;
                }
                // Surrogates and null units are never whitespace. Keep every
                // selected unit; do not repair or replace unpaired values.
                self.string_units_output(units[start..end].iter().copied(),end-start,(end-start).saturating_mul(2),"String trim")
            }
            _=>Err(Fault::new("String trim requires an already converted string.")),
        }
    }
}
