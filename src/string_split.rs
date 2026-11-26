//! Exact UTF-16 string splitting for primitive separators and native RegExp.
use super::{int32, regexp, Fault, Runtime, Value, WorkKind};

impl Runtime {
    fn split_charge(&mut self, bytes: usize, owned: &mut usize) -> Result<(), Fault> {
        let next = self.value_bytes.checked_add(bytes).filter(|&n| n <= self.limits.heap.retained_values)
            .ok_or_else(|| Fault::new("String split workspace limit reached."))?;
        let total = owned.checked_add(bytes).ok_or_else(|| Fault::new("String split accounting overflow."))?;
        self.value_bytes = next; *owned = total; Ok(())
    }
    fn split_refund(&mut self, bytes: usize, owned: &mut usize) -> Result<(), Fault> {
        self.value_bytes = self.value_bytes.checked_sub(bytes).ok_or_else(|| Fault::new("String split refund failed."))?;
        *owned = owned.checked_sub(bytes).ok_or_else(|| Fault::new("String split accounting refund failed."))?;
        Ok(())
    }
    fn split_match_backing(found: &regexp::Match) -> Result<usize, Fault> {
        let mut held = found.captures.capacity().checked_mul(std::mem::size_of::<Option<(usize,usize)>>())
            .and_then(|n| found.named_captures.capacity().checked_mul(std::mem::size_of::<(String,Option<(usize,usize)>)>()).and_then(|m| n.checked_add(m)))
            .ok_or_else(|| Fault::new("String split capture backing overflow."))?;
        for (name, _) in &found.named_captures { held = held.checked_add(name.capacity()).ok_or_else(|| Fault::new("String split name backing overflow."))?; }
        Ok(held)
    }
    fn split_units(&mut self, value: &Value, owned: &mut usize) -> Result<Vec<u16>, Fault> {
        self.work_as(value.bytes().saturating_mul(3), WorkKind::StringMethodInput)?;
        let converted;
        let mut converted_bytes = 0;
        let value = match value {
            Value::String(_) | Value::StringUnits(_) => value,
            _ => { converted = Value::String(value.script_text()?); if let Value::String(s) = &converted { converted_bytes = s.capacity(); } self.split_charge(converted_bytes, owned)?; &converted }
        };
        let count = match value { Value::String(s) => s.encode_utf16().count(), Value::StringUnits(s) => s.len(), _ => unreachable!() };
        let bytes = count.checked_mul(2).ok_or_else(|| Fault::new("String split input size overflow."))?;
        self.work_as(bytes, WorkKind::StringMethodInput)?;
        self.split_charge(bytes, owned)?;
        let mut units = Vec::new();
        units.try_reserve_exact(count).map_err(|_| Fault::new("String split input allocation failed."))?;
        let actual = units.capacity().checked_mul(2).ok_or_else(|| Fault::new("String split input allocation overflow."))?;
        if actual > bytes { self.split_charge(actual - bytes, owned)?; }
        match value {
            Value::String(s) => for unit in s.encode_utf16() { self.step()?; units.push(unit); },
            Value::StringUnits(s) => { self.step()?; units.extend_from_slice(s); },
            _ => unreachable!(),
        }
        self.split_refund(converted_bytes, owned)?;
        Ok(units)
    }
    fn split_piece(&mut self, units: &[u16], owned: &mut usize) -> Result<Value, Fault> {
        self.work_as(units.len().saturating_mul(2), WorkKind::StringMethodInput)?;
        let mut bytes = 0usize;
        for character in char::decode_utf16(units.iter().copied()) {
            self.step()?;
            let Ok(character) = character else {
                bytes = units.len().checked_mul(2).ok_or_else(|| Fault::new("String split fragment size overflow."))?;
                break;
            };
            bytes = bytes.checked_add(character.len_utf8()).ok_or_else(|| Fault::new("String split fragment size overflow."))?;
        }
        self.split_charge(bytes, owned)?;
        let value = self.string_from_units(units)?;
        let actual = match &value {
            Value::String(s) => s.capacity(),
            Value::StringUnits(s) => s.capacity().checked_mul(2).ok_or_else(|| Fault::new("String split fragment allocation overflow."))?,
            _ => unreachable!(),
        };
        if actual > bytes { self.split_charge(actual - bytes, owned)?; }
        else if actual < bytes { self.split_refund(bytes - actual, owned)?; }
        Ok(value)
    }
    fn split_push(&mut self, pieces: &mut Vec<Value>, value: Value, limit: usize, owned: &mut usize) -> Result<bool, Fault> {
        self.step()?;
        if pieces.len() >= self.limits.heap.array_length {
            return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
        }
        if pieces.len() == pieces.capacity() {
            let old = pieces.capacity();
            let growth = old.max(1).min(limit - pieces.len()).min(self.limits.heap.array_length - pieces.len());
            let bytes = growth.checked_mul(std::mem::size_of::<Value>()).ok_or_else(|| Fault::new("String split pieces size overflow."))?;
            self.split_charge(bytes, owned)?;
            pieces.try_reserve_exact(growth).map_err(|_| Fault::new("String split pieces allocation failed."))?;
            let actual = (pieces.capacity() - old).checked_mul(std::mem::size_of::<Value>()).ok_or_else(|| Fault::new("String split pieces allocation overflow."))?;
            if actual > bytes { self.split_charge(actual - bytes, owned)?; }
        }
        pieces.push(value); Ok(pieces.len() >= limit)
    }
    pub(super) fn string_split_value(&mut self, receiver: &Value, args: &[Value]) -> Result<Value, Fault> {
        let mut owned = 0;
        let result = (|| {
            self.work_as(receiver.bytes().saturating_mul(3), WorkKind::StringMethodInput)?;
            let separator = args.first().unwrap_or(&Value::Undefined);
            let expression = match separator { Value::Object(id) => self.object(*id)?.regexp.clone(), _ => None };
            if expression.is_none() { separator.primitive()?; }
            let limit_value = args.get(1).unwrap_or(&Value::Undefined);
            limit_value.primitive()?;
            self.work(limit_value.bytes().saturating_mul(2))?;
            let limit = if matches!(limit_value, Value::Undefined) { u32::MAX as usize }
                else { int32(limit_value.script_number()?) as u32 as usize };
            let mut pieces = Vec::new();
            if limit == 0 { return self.array_from_values(&pieces); }
            let input = self.split_units(receiver, &mut owned)?;
            if matches!(separator, Value::Undefined) {
                let value = self.split_piece(&input, &mut owned)?;
                self.split_push(&mut pieces, value, limit, &mut owned)?;
                return self.array_from_values(&pieces);
            }
            if let Some(expression) = expression {
                if input.is_empty() {
                    if let Some(found) = expression.find(&input, 0, true, &mut self.regexp_work).map_err(Self::regexp_fault)? {
                        let held = Self::split_match_backing(&found)?;
                        self.split_charge(held, &mut owned)?;
                        self.split_refund(held, &mut owned)?;
                    } else {
                        let value = self.split_piece(&[], &mut owned)?;
                        self.split_push(&mut pieces, value, limit, &mut owned)?;
                    }
                    return self.array_from_values(&pieces);
                }
                let mut previous = 0; let mut at = 0;
                while at < input.len() {
                    self.step()?;
                    let Some(found) = expression.find(&input, at, true, &mut self.regexp_work).map_err(Self::regexp_fault)? else {
                        at = regexp::advance_string_index(&input, at, expression.flags.unicode); continue;
                    };
                    let held = Self::split_match_backing(&found)?;
                    self.split_charge(held, &mut owned)?;
                    if found.end() == previous {
                        self.split_refund(held, &mut owned)?;
                        at = regexp::advance_string_index(&input, at, expression.flags.unicode); continue;
                    }
                    let value = self.split_piece(&input[previous..at], &mut owned)?;
                    if self.split_push(&mut pieces, value, limit, &mut owned)? { return self.array_from_values(&pieces); }
                    for capture in found.captures.iter().skip(1) {
                        let value = if let Some((start,end)) = capture { self.split_piece(&input[*start..*end], &mut owned)? } else { Value::Undefined };
                        if self.split_push(&mut pieces, value, limit, &mut owned)? { return self.array_from_values(&pieces); }
                    }
                    previous = found.end(); at = previous;
                    self.split_refund(held, &mut owned)?;
                }
                let value = self.split_piece(&input[previous..], &mut owned)?;
                self.split_push(&mut pieces, value, limit, &mut owned)?;
            } else {
                let needle = self.split_units(separator, &mut owned)?;
                if needle.is_empty() {
                    for &unit in &input {
                        let value = self.split_piece(&[unit], &mut owned)?;
                        if self.split_push(&mut pieces, value, limit, &mut owned)? { break; }
                    }
                } else {
                    let mut previous = 0; let mut at = 0;
                    while at <= input.len() && needle.len() <= input.len() - at {
                        self.step()?; self.work(needle.len().saturating_mul(2))?;
                        if input[at..at+needle.len()] == needle {
                            let value = self.split_piece(&input[previous..at], &mut owned)?;
                            if self.split_push(&mut pieces, value, limit, &mut owned)? { return self.array_from_values(&pieces); }
                            at += needle.len(); previous = at;
                        } else { at += 1; }
                    }
                    let value = self.split_piece(&input[previous..], &mut owned)?;
                    self.split_push(&mut pieces, value, limit, &mut owned)?;
                }
            }
            self.array_from_values(&pieces)
        })();
        self.split_refund(owned, &mut owned)?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("Unexpected native read".into())}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("Unexpected native write".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("Unexpected native call".into())}
    }
    fn succeeds(source:&str){assert_eq!(Runtime::for_test().execute(source,&mut NoHost).unwrap().to_text(),"ok");}
    #[test]
    fn utf16_string_split_literal_separators_preserve_all_units_and_limits(){
        succeeds(r#"
var parts='😀'.split('');
if(parts.length!==2||parts[0].charCodeAt(0)!==0xd83d||parts[1].charCodeAt(0)!==0xde00)throw 'empty units';
parts='a\ud800b\ud800'.split('\ud800');if(parts.join('|')!=='a|b|')throw 'isolated separator';
parts='😀'.split('\ud83d');if(parts.length!==2||parts[0]!==''||parts[1].charCodeAt(0)!==0xde00)throw 'half scalar separator';
parts='\ud800x'.split();if(parts.length!==1||parts[0].charCodeAt(0)!==0xd800)throw 'undefined separator';
if('😀'.split('',1)[0].charCodeAt(0)!==0xd83d||'😀'.split('',0).length!==0||''.split('').length!==0)throw 'empty limit';
if('a\ud800b'.split('\ud800',-1).join('|')!=='a|b'||'a\ud800b'.split('\ud800',1)[0]!=='a')throw 'uint32 limit';
'ok';
"#);
    }
    #[test]
    fn utf16_string_split_native_regex_preserves_captures_unicode_progress_and_last_index(){
        succeeds(r#"
var re=/\ud800/gu;re.lastIndex=19;
var parts='a\ud800b😀c'.split(re);if(parts.join('|')!=='a|b😀c'||re.lastIndex!==19)throw 'native separator';
parts='a\ud800b'.split(/(\ud800)|(x)/u);
if(parts.length!==4||parts[0]!=='a'||parts[1].charCodeAt(0)!==0xd800||parts[2]!==undefined||parts[3]!=='b')throw 'capture and unmatched';
parts='😀'.split(/(?:)/);if(parts.length!==2||parts[0].charCodeAt(0)!==0xd83d||parts[1].charCodeAt(0)!==0xde00)throw 'classic empty progress';
if('😀A'.split(/(?:)/u).join('|')!=='😀|A'||''.split(/(?:)/u).length!==0||''.split(/x/u).length!==1)throw 'Unicode empty progress';
parts='😀'.split(/(\ud83d)/);if(parts.length!==3||parts[1].charCodeAt(0)!==0xd83d||parts[2].charCodeAt(0)!==0xde00)throw 'classic captures';
if('a\ud800b'.split(/(\ud800)/u,2).length!==2)throw 'capture limit';
'ok';
"#);
    }
    #[test]
    fn utf16_string_split_workspace_refunds_on_retained_and_step_refusal(){
        let mut runtime=Runtime::for_test();let mut host=NoHost;
        runtime.execute("var preservedSplitValue=7;",&mut host).unwrap();
        let input=Value::StringUnits(vec![97,0xd800,98]);let args=[Value::StringUnits(vec![0xd800])];
        let baseline=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
        for headroom in [0,9+std::mem::size_of::<Value>()-1] {
            runtime.begin_job();runtime.limits.heap.retained_values=baseline+headroom;
            let error=runtime.string_split_value(&input,&args).unwrap_err();
            assert!(error.message.contains("String split workspace limit"),"{}",error.message);
            assert_eq!(runtime.value_bytes,baseline);
        }
        runtime.limits.heap.retained_values=limit;runtime.begin_job();runtime.steps=0;
        let error=runtime.string_split_value(&input,&args).unwrap_err();
        assert!(error.message.contains("Execution step limit"),"{}",error.message);assert_eq!(runtime.value_bytes,baseline);
        runtime.begin_job();
        assert_eq!(runtime.execute(r"preservedSplitValue===7&&'a\ud800b'.split('\ud800').join('|')==='a|b'?'ok':'bad'",&mut host).unwrap().to_text(),"ok");
    }
    #[test]
    fn utf16_string_split_copy_deadline_string_and_array_refusals_recover(){
        use std::time::{Duration,Instant};
        let mut runtime=Runtime::for_test();let input=Value::StringUnits(vec![97,0xd800,98]);let args=[Value::String(String::new())];
        let baseline=runtime.value_bytes;
        runtime.constrain_job(usize::MAX,0,usize::MAX,Instant::now()+Duration::from_secs(10));
        let error=runtime.string_split_value(&input,&args).unwrap_err();assert!(error.message.contains("String/copy work limit"));assert_eq!(runtime.value_bytes,baseline);
        runtime.begin_job();runtime.constrain_job(usize::MAX,usize::MAX,usize::MAX,Instant::now()-Duration::from_secs(1));runtime.steps=256;
        let error=runtime.string_split_value(&input,&args).unwrap_err();assert!(error.message.contains("time")||error.message.contains("deadline"),"{}",error.message);assert_eq!(runtime.value_bytes,baseline);
        runtime.begin_job();let old_string=runtime.limits.capacity.string_bytes;runtime.limits.capacity.string_bytes=1;
        let error=runtime.string_split_value(&input,&args).unwrap_err();assert!(error.message.contains("String limit reached"),"{}",error.message);assert_eq!(runtime.value_bytes,baseline);
        runtime.limits.capacity.string_bytes=old_string;runtime.begin_job();let old_array=runtime.limits.heap.array_length;runtime.limits.heap.array_length=2;
        let error=runtime.string_split_value(&input,&args).unwrap_err();assert!(error.message.contains("Array length limit"),"{}",error.message);assert_eq!(runtime.value_bytes,baseline);
        runtime.limits.heap.array_length=old_array;runtime.begin_job();
        assert_eq!(runtime.execute(r"'a\ud800b'.split('').length===3?'ok':'bad'",&mut NoHost).unwrap().to_text(),"ok");
    }
    #[test]
    fn utf16_string_split_primitive_conversion_and_unsupported_protocols_stay_explicit(){
        succeeds(r#"
if('anullb'.split(null).join('|')!=='a|b'||'atrueb'.split(true).join('|')!=='a|b'||'a12b'.split(12).join('|')!=='a|b')throw 'primitive separator';
if('a,b,c'.split(',',4294967298).join('|')!=='a|b'||'a,b'.split(',',NaN).length!==0)throw 'uint32 conversion';
var caught=null;try{'abc'.split(Symbol('x'));}catch(e){caught=e;}if(!(caught instanceof TypeError))throw 'symbol separator';
'ok';
"#);
        let mut runtime=Runtime::for_test();let error=runtime.execute("'a'.split({toString:function(){return 'a';}})",&mut NoHost).unwrap_err();
        assert!(error.contains("Implicit object conversion is not implemented."),"{error}");
        assert_eq!(runtime.execute(r"'\ud800'.split('').length===1?'ok':'bad'",&mut NoHost).unwrap().to_text(),"ok");
    }

    #[test]
    fn utf16_string_split_regex_work_refusal_preserves_last_index_and_recovery(){
        let mut runtime=Runtime::for_test();
        let expression=runtime.execute(r"var splitRe=/(\ud800)/u;splitRe.lastIndex=7;splitRe;",&mut NoHost).unwrap();
        let baseline=runtime.value_bytes;runtime.regexp_work=0;
        let error=runtime.string_split_value(&Value::StringUnits(vec![97,0xd800,98]),&[expression]).unwrap_err();
        assert!(error.message.contains("Regular-expression work limit"),"{}",error.message);assert_eq!(runtime.value_bytes,baseline);
        runtime.begin_job();
        assert_eq!(runtime.execute(r"splitRe.lastIndex===7&&'a\ud800b'.split(splitRe)[1].charCodeAt(0)===0xd800?'ok':'bad'",&mut NoHost).unwrap().to_text(),"ok");
        let empty=runtime.execute("/(?:)/u",&mut NoHost).unwrap();
        let baseline=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
        runtime.limits.heap.retained_values=baseline;
        let error=runtime.string_split_value(&Value::String(String::new()),&[empty]).unwrap_err();
        assert!(error.message.contains("String split workspace limit"),"{}",error.message);assert_eq!(runtime.value_bytes,baseline);
        runtime.limits.heap.retained_values=limit;runtime.begin_job();
        assert_eq!(runtime.execute("''.split(/(?:)/u).length;",&mut NoHost).unwrap().to_text(),"0");
    }

}
