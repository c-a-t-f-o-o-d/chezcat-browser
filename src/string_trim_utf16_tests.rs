//! Original exact UTF-16 whitespace trimming regressions.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
}
fn succeeds(source:&str){Runtime::for_test().execute(source,&mut NoHost).unwrap();}

#[test]
fn trim_utf16_raw_units_nulls_pairs_and_ecmascript_whitespace() {
    succeeds(r#"
var spaces='\u0009\u000b\u000c\u0020\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u202f\u205f\u3000\ufeff\u000a\u000d\u2028\u2029';
for(var value of ['\ud800','\udfff','A\ud800B','\udc00\ud800','\ud800\u0000\udfff','😀\ud800😀']){
if((spaces+value+spaces).trim()!==value||new String(spaces+value+spaces).trim()!==value)throw 'exact raw trim';}
for(var value of ['\u0085','\u180e','\u200b','\u0000']){if((' '+value+'\ud800'+value+' ').trim()!==value+'\ud800'+value)throw 'non whitespace';}
if(spaces.trim()!==''||(' '+String.fromCharCode(0xd800)+' ').trim()!=='\ud800')throw 'empty and producer';
"#);
}

#[test]
fn trim_utf16_generic_receiver_hooks_captured_native_and_abrupt_identity() {
    succeeds(r#"
var native=String.prototype.trim,log='',receiver={[Symbol.toPrimitive]:function(h){log+=h;String.prototype.trim=function(){return 'replacement';};if(native.call(' x ')!=='x')throw 'nested';return ' \ud800 ';}};
if(native.call(receiver,{valueOf:function(){throw 'extra converted';}})!=='\ud800'||log!=='string'||'a'.trim()!=='replacement')throw 'generic exact trim';
String.prototype.trim=native;
var marker={},bad={toString:function(){throw marker;}};
try{native.call(bad);throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}
for(var value of [null,undefined,Symbol('bad')]){try{native.call(value);throw 'missing TypeError';}catch(e){if(!(e instanceof TypeError))throw e;}}
if(native.call(123)!=='123'||native.call(true)!=='true'||' \udfff '.trim()!=='\udfff')throw 'fresh recovery';
"#);
}

#[test]
fn trim_utf16_exact_keys_json_and_reflected_metadata_consumers() {
    succeeds(r#"
var key=' \ud800 field\udfff ';
var exact=key.trim(),record={};record[exact]=7;var parsed=JSON.parse(JSON.stringify(record)),actual=Reflect.ownKeys(parsed)[0];
if(actual!=='\ud800 field\udfff'||new String(actual).charCodeAt({valueOf:function(){return 0;}})!==0xd800||parsed[actual]!==7)throw 'trimmed key';
"#);
}

#[test]
fn trim_utf16_selected_representation_uses_exact_retained_output_admission() {
    let mut runtime=Runtime::for_test();let retained=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    for (receiver,peak,expected) in [
        (Value::String(" A ".into()),1,Value::String("A".into())),
        (Value::String(" \u{800} ".into()),3,Value::String("\u{800}".into())),
        (Value::String(" 😀 ".into()),4,Value::String("😀".into())),
        (Value::StringUnits(vec![32,0xd800,32]),2,Value::StringUnits(vec![0xd800])),
    ] {
        for headroom in 0..peak {
            runtime.limits.heap.retained_values=retained+headroom;runtime.begin_job();
            let error=runtime.string_trim_value(&receiver).unwrap_err();
            assert!(error.exception.is_none()&&error.message.contains("String trim workspace limit"),"{}",error.message);
            assert_eq!(runtime.value_bytes,retained);
        }
        runtime.limits.heap.retained_values=retained+peak;runtime.begin_job();
        let value=runtime.string_trim_value(&receiver).unwrap();assert!(strict_equal(&value,&expected));
        assert_eq!(runtime.value_bytes,retained);
    }
    runtime.limits.heap.retained_values=retained;runtime.begin_job();
    assert!(matches!(runtime.string_trim_value(&Value::String(" \t\u{feff}".into())).unwrap(),Value::String(text) if text.is_empty()));
    assert_eq!(runtime.value_bytes,retained);runtime.limits.heap.retained_values=limit;
}

#[test]
fn trim_utf16_work_step_string_refusals_refund_and_fresh_recovery() {
    let mut runtime=Runtime::for_test();let retained=runtime.value_bytes;let receiver=Value::StringUnits(vec![32,0xd800,32]);
    runtime.begin_job();runtime.copy_bytes=0;
    let error=runtime.string_trim_value(&receiver).unwrap_err();assert!(error.exception.is_none()&&error.message.contains("work limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.begin_job();runtime.steps=0;
    let error=runtime.string_trim_value(&receiver).unwrap_err();assert!(error.exception.is_none()&&error.message.contains("step limit"));assert_eq!(runtime.value_bytes,retained);
    let cap=runtime.limits.capacity.string_bytes;runtime.limits.capacity.string_bytes=1;runtime.begin_job();
    let error=runtime.string_trim_value(&receiver).unwrap_err();assert!(error.exception.is_none()&&error.message.contains("String limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.limits.capacity.string_bytes=cap;runtime.begin_job();
    assert!(matches!(runtime.string_trim_value(&receiver).unwrap(),Value::StringUnits(units) if units==[0xd800]));assert_eq!(runtime.value_bytes,retained);
}

#[test]
fn trim_utf16_canonical_metadata_preserves_supplementary_and_internal_spacing() {
    succeeds(r#"
var f=String.prototype.trim,d=Object.getOwnPropertyDescriptor(String.prototype,'trim');
if(f.length!==0||f.name!=='trim'||!d.writable||d.enumerable||!d.configurable)throw 'metadata';
for(var value of ['A B','😀','\ud800 \udfff','\ud800\ufeff\udfff']){if(f.call(' '+value+' ')!==value)throw 'internal units';}
if(f.call('  😀  ')!=='😀'||f.call(' \u0085 ')!=='\u0085')throw 'scalar controls';
"#);
}
