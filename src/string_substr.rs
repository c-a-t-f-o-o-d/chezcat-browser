//! Exact UTF-16 slice, substring and Annex B substring-by-count results.
//! https://tc39.es/ecma262/#sec-string.prototype.slice
use super::{Fault, Host, Runtime, Value, WorkKind};

#[derive(Clone, Copy)]
enum SliceInput<'a> {
    Scalar(&'a str),
    Units(&'a [u16]),
}

impl Runtime {
    pub(super) fn string_slice_value(
        &mut self, value: &Value, method: &str, args: &[Value], host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let input = match value {
            Value::String(text) => SliceInput::Scalar(text),
            Value::StringUnits(units) => SliceInput::Units(units),
            _ => return Err(Fault::new("String slicing requires an already converted string.")),
        };
        self.string_slice_input(input, method, args, host)
    }

    fn string_slice_input(
        &mut self, input: SliceInput<'_>, method: &str, args: &[Value], host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        // The receiver conversion has completed. Keep this immutable value
        // and its native length across both ordered numeric conversions.
        let size = match input {
            SliceInput::Scalar(text) => {
                self.work_as(text.len(), WorkKind::StringMethodInput)?;
                text.encode_utf16().count()
            }
            SliceInput::Units(units) => units.len(),
        };
        let (start, end) = match method {
            "slice" => {
                let start = self.binary_relative(args.first(), size, 0, host)?;
                let end = self.binary_relative(args.get(1), size, size, host)?;
                (start, end.max(start))
            }
            "substring" => {
                let start = self.binary_integer(args.first(), 0.0, host)?.clamp(0.0, size as f64) as usize;
                let end = self.binary_integer(args.get(1), size as f64, host)?.clamp(0.0, size as f64) as usize;
                (start.min(end), start.max(end))
            }
            "substr" => {
                let start = self.binary_relative(args.first(), size, 0, host)?;
                let length = self.binary_integer(args.get(1), size as f64, host)?.clamp(0.0, size as f64) as usize;
                (start, start.saturating_add(length).min(size))
            }
            _ => return Err(Fault::new("Unknown string slicing method.")),
        };
        // Even an empty result follows the end/count conversion above.
        if start == end { return self.checked_value(Value::String(String::new())); }
        let count = end - start;
        match input {
            SliceInput::Scalar(text) => {
                // Skipping a UTF-16 prefix still scans UTF-8. A complete
                // receiver scan bounds each selected-range traversal, while
                // storage is reserved only for the selected final result.
                self.string_slice_output(text.encode_utf16().skip(start).take(count), count, text.len())
            }
            SliceInput::Units(units) => {
                self.string_slice_output(units[start..end].iter().copied(), count, count.saturating_mul(2))
            }
        }
    }

    fn string_slice_workspace_fault(&self) -> Fault {
        Fault::new(format!("String slicing workspace limit reached ({} bytes).", self.limits.heap.retained_values))
    }

    fn string_slice_output<I: Iterator<Item = u16> + Clone>(
        &mut self, units: I, count: usize, scan_bytes: usize,
    ) -> Result<Value, Fault> {
        self.work_as(scan_bytes, WorkKind::StringMethodInput)?;
        let mut scalar = true;
        let mut scalar_bytes = 0usize;
        for decoded in char::decode_utf16(units.clone()) {
            self.step()?;
            let Ok(ch) = decoded else { scalar = false; break; };
            scalar_bytes = scalar_bytes.checked_add(ch.len_utf8()).ok_or_else(|| self.string_limit_fault())?;
        }
        let bytes = if scalar { scalar_bytes } else {
            count.checked_mul(2).ok_or_else(|| self.string_limit_fault())?
        };
        if bytes > self.limits.capacity.string_bytes { return Err(self.string_limit_fault()); }
        let available = self.limits.heap.retained_values.saturating_sub(self.value_bytes);
        if bytes > available { return Err(self.string_slice_workspace_fault()); }
        self.work_as(scan_bytes, WorkKind::StringMethodInput)?;
        self.work_as(bytes, WorkKind::ConversionOutput)?;
        let mut owned = 0usize;
        let result = (|| {
            let value = if scalar {
                let mut text = String::new();
                text.try_reserve_exact(bytes).map_err(|_| Fault::new("String slicing allocation failed."))?;
                let capacity = text.capacity();
                if capacity > available { return Err(self.string_slice_workspace_fault()); }
                self.value_bytes = self.value_bytes.checked_add(capacity).ok_or_else(|| self.string_slice_workspace_fault())?;
                owned = capacity;
                for decoded in char::decode_utf16(units) {
                    self.step()?;
                    text.push(decoded.expect("validated string slice UTF-16"));
                }
                Value::String(text)
            } else {
                let mut output = Vec::new();
                output.try_reserve_exact(count).map_err(|_| Fault::new("String slicing allocation failed."))?;
                let capacity = output.capacity().checked_mul(2).ok_or_else(|| self.string_slice_workspace_fault())?;
                if capacity > available { return Err(self.string_slice_workspace_fault()); }
                self.value_bytes = self.value_bytes.checked_add(capacity).ok_or_else(|| self.string_slice_workspace_fault())?;
                owned = capacity;
                for unit in units {
                    self.step()?;
                    output.push(unit);
                }
                Value::StringUnits(output)
            };
            self.checked_value(value)
        })();
        // Only the result backing belongs to this operation. Callback
        // publications completed before it was reserved and keep their charges.
        self.value_bytes = self.value_bytes.checked_sub(owned)
            .ok_or_else(|| Fault::new("String slicing workspace refund accounting failed."))?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
    }
    #[test]
    fn annex_b_substr_clamps_start_and_count_by_utf16_units() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var s='012345',tests=[
[s.substr(),'012345'],[s.substr(undefined,undefined),'012345'],[s.substr(2,3),'234'],
[s.substr(-2),'45'],[s.substr(-20,3),'012'],[s.substr(20,3),''],
[s.substr(1.9,2.9),'12'],[s.substr(-2.9,1.9),'4'],[s.substr(-0,2),'01'],
[s.substr(NaN,NaN),''],[s.substr(-Infinity,Infinity),'012345'],[s.substr(Infinity,Infinity),''],
[s.substr(2,-Infinity),''],[s.substr(2,-1),''],[s.substr(2,null),''],[s.substr(2,true),'2'],
[s.substr(2,{valueOf:function(){return undefined;}}),''],
['A😀Z'.substr(1,2),'😀'],['A😀Z'.substr(-1,1),'Z'],['A😀Z'.substr(3,Infinity),'Z'],['A😀Z'.substr(2,0),'']
];for(var i=0;i<tests.length;i++)if(tests[i][0]!==tests[i][1])throw 'case '+i;
var f=String.prototype.substr;if(f.call(12345,1,2)!=='23'||f.call(new String('abc'),1)!=='bc')throw 'generic/wrapper';
if(f.call(true,1,2)!=='ru'||f.call({toString:function(){return 'custom';}},2,2)!=='st')throw 'generic string conversion';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn annex_b_substr_converts_receiver_start_length_in_order_even_for_empty_results() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var f=String.prototype.substr,order='',marker={};
var receiver={[Symbol.toPrimitive]:function(h){order+='receiver:'+h+';';return 'abcde';}};
var start={[Symbol.toPrimitive]:function(h){order+='start:'+h+';';return -3;}};
var length={[Symbol.toPrimitive]:function(h){order+='length:'+h+';';return 2;}};
if(f.call(receiver,start,length)!=='cd'||order!=='receiver:string;start:number;length:number;')throw order;
order='';var bad={valueOf:function(){order+='length;';throw marker;}};
var caught=false;try{f.call('',Infinity,bad);}catch(e){caught=e===marker;}
if(!caught||order!=='length;')throw 'empty still converts count';
order='';caught=false;try{f.call(null,start,length);}catch(e){caught=e instanceof TypeError;}
if(!caught||order!=='')throw 'receiver brand first';
order='';caught=false;try{f.call('abc',Symbol('bad'),length);}catch(e){caught=e instanceof TypeError;}
if(!caught||order!=='')throw 'start abrupt completion';
caught=false;try{f.call('',0,Symbol('bad'));}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'length Symbol';
caught=false;try{f.call(Symbol('bad'),0,1);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'receiver Symbol';
if('abc'.substr({valueOf:function(){return undefined;}},undefined)!=='abc')throw 'start undefined conversion';
if('abc'.substr(0,1,{valueOf:function(){throw 'extra converted';}})!=='a')throw 'extra argument';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn annex_b_substr_has_real_native_metadata_and_mutable_prototype_property() {
        let mut runtime=Runtime::for_test();
        runtime.execute(r#"
var f=String.prototype.substr,d=Object.getOwnPropertyDescriptor(String.prototype,'substr');
if(f!==String.prototype.substr||f.name!=='substr'||f.length!==2||d.value!==f||d.enumerable||!d.writable||!d.configurable)throw 'native metadata';
if(Function.prototype.toString.call(f).indexOf('[native code]')<0)throw 'native source';
String.prototype.substr=function(){return 'replacement';};if('abc'.substr(0,1)!=='replacement')throw 'replacement';
if(!delete String.prototype.substr||'substr' in String.prototype||'abc'.substr!==undefined)throw 'deletion';
String.prototype.substr=f;if('abc'.substr(1,1)!=='b')throw 'restore';
"#,&mut NoHost).unwrap();
    }
    #[test]
    fn annex_b_substr_retains_split_pairs_and_keeps_work_refusal_recoverable() {
        let mut runtime=Runtime::for_test();
        let result=runtime.execute("'A😀Z'.substr(1,1);",&mut NoHost).unwrap();
        assert!(matches!(result, Value::StringUnits(units) if units==[0xd83d]));
        runtime.begin_job();
        assert_eq!(runtime.execute("'A😀Z'.substr(1,2);",&mut NoHost).unwrap().to_text(),"😀");
        runtime.constrain_job(usize::MAX,0,usize::MAX,std::time::Instant::now()+std::time::Duration::from_secs(10));
        let error=runtime.string_slice_value(&Value::String("abc".into()),"substr",&[],&mut NoHost).unwrap_err().display();
        assert!(error.contains("String/copy work limit"),"{error}");
        runtime.begin_job();
        assert_eq!(runtime.string_slice_value(&Value::String("abc".into()),"substr",&[Value::Number(1.0)],&mut NoHost).unwrap().to_text(),"bc");
    }
}
