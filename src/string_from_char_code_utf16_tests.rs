
//! Original String.fromCharCode exact-unit producer controls.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
}
fn succeeds(source:&str){Runtime::for_test().execute(source,&mut NoHost).unwrap();}

#[test]
fn from_char_code_utf16_exact_raw_null_pair_and_numeric_unit_sequences() {
    succeeds(r#"
var f=String.fromCharCode,cases=[[[0xd800],'\ud800'],[[0xdc00],'\udc00'],[[0xd800,65,0xdc00],'\ud800A\udc00'],[[0xdc00,0xd800],'\udc00\ud800'],[[0xd83d,0xde00],'😀'],[[0,0xd800,0],'\u0000\ud800\u0000'],[[0x1d800,-10240],'\ud800\ud800']];
for(var c of cases){var value=f.apply(null,c[0]);if(value!==c[1]||value.length!==c[1].length)throw 'exact units';}
if(f()!==''||f(65,NaN,Infinity,-Infinity,undefined)!=='A\u0000\u0000\u0000\u0000')throw 'defaults';
"#);
}

#[test]
fn from_char_code_utf16_ordered_nested_conversion_and_abrupt_identity() {
    succeeds(r#"
var f=String.fromCharCode,order='',marker={};
var a={valueOf:function(){order+='A';if(f(0xdc00)!=='\udc00')throw 'nested';return 0xd800;},toString:function(){throw 'wrong numeric hint';}};
var b={valueOf:function(){order+='B';return 65;}};
if(f(a,b)!=='\ud800A'||order!=='AB')throw 'ordered raw result';
order='';try{f({valueOf:function(){order+='A';return 0xd800;}},{valueOf:function(){order+='B';throw marker;}},{valueOf:function(){throw 'late conversion';}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}
if(order!=='AB'||f(0xd800)!=='\ud800')throw 'abrupt recovery';
"#);
}

#[test]
fn from_char_code_utf16_exact_key_box_json_and_search_consumers() {
    succeeds(r#"
var raw=String.fromCharCode(65,0xd800,66,0xdfff),o={};o[raw]=7;
var parsed=JSON.parse(JSON.stringify(o)),keys=Reflect.ownKeys(parsed),b=new String(keys[0]);
if(keys[0]!==raw||parsed[b]!==7||b.indexOf('\ud800')!==1||b.lastIndexOf('\udfff')!==3||b.slice(1,2)!=='\ud800')throw 'produced exact key';
"#);
}

#[test]
fn from_char_code_utf16_staging_and_representation_peak_share_retained_admission() {
    let mut runtime=Runtime::for_test();let retained=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    for (args,peak,raw) in [
        (vec![Value::Number(65.0)],3,false),
        (vec![Value::Number(0xd800 as f64)],4,true),
        (vec![Value::Number(0x800 as f64)],5,false),
        (vec![Value::Number(0xd83d as f64),Value::Number(0xde00 as f64)],8,false),
    ] {
        for headroom in 0..peak {
            runtime.limits.heap.retained_values=retained+headroom;runtime.begin_job();
            let error=runtime.string_from_char_code(&args,&mut NoHost).unwrap_err();
            assert!(error.exception.is_none()&&error.message.contains("workspace limit"),"{headroom}/{peak}: {}",error.message);
            assert_eq!(runtime.value_bytes,retained);
        }
        runtime.limits.heap.retained_values=retained+peak;runtime.begin_job();
        let value=runtime.string_from_char_code(&args,&mut NoHost).unwrap();
        assert_eq!(matches!(value,Value::StringUnits(_)),raw);assert_eq!(runtime.value_bytes,retained);
    }
    runtime.limits.heap.retained_values=retained;runtime.begin_job();
    assert!(matches!(runtime.string_from_char_code(&[],&mut NoHost).unwrap(),Value::String(text) if text.is_empty()));
    assert_eq!(runtime.value_bytes,retained);runtime.limits.heap.retained_values=limit;
}

#[test]
fn from_char_code_utf16_string_work_step_and_argument_refusals_recover() {
    let mut runtime=Runtime::for_test();let retained=runtime.value_bytes;
    let args=[Value::Number(0xd800 as f64)];let cap=runtime.limits.capacity.string_bytes;
    runtime.limits.capacity.string_bytes=1;runtime.begin_job();
    let error=runtime.string_from_char_code(&args,&mut NoHost).unwrap_err();assert!(error.message.contains("String limit"));
    assert_eq!(runtime.value_bytes,retained);runtime.limits.capacity.string_bytes=cap;
    runtime.begin_job();runtime.copy_bytes=0;
    let error=runtime.string_from_char_code(&args,&mut NoHost).unwrap_err();assert!(error.message.contains("work limit"));
    assert_eq!(runtime.value_bytes,retained);runtime.begin_job();runtime.steps=0;
    let error=runtime.string_from_char_code(&args,&mut NoHost).unwrap_err();assert!(error.message.contains("step limit"));
    assert_eq!(runtime.value_bytes,retained);let arguments=runtime.limits.capacity.arguments;
    runtime.limits.capacity.arguments=0;runtime.begin_job();
    let error=runtime.string_from_char_code(&args,&mut NoHost).unwrap_err();assert!(error.message.contains("argument limit"));
    assert_eq!(runtime.value_bytes,retained);runtime.limits.capacity.arguments=arguments;runtime.begin_job();
    assert!(matches!(runtime.string_from_char_code(&args,&mut NoHost).unwrap(),Value::StringUnits(units) if units==[0xd800]));
    assert_eq!(runtime.value_bytes,retained);
}

#[test]
fn from_char_code_utf16_late_output_refusal_preserves_numeric_hook_publication() {
    let mut runtime=Runtime::for_test();let mut host=NoHost;
    let probe=runtime.execute("var publications=0,probe={valueOf:function(){publications++;return 0xd800;}};probe;",&mut host).unwrap();
    // Leave callback arena storage available: this control reaches final
    // output admission after the author hook, unlike the separate peak test.
    let limit=runtime.limits.capacity.string_bytes;
    runtime.limits.capacity.string_bytes=2;runtime.begin_job();
    let error=runtime.string_from_char_code(&[probe.clone(),Value::Number(65.0)],&mut host).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("String limit"),"{}",error.message);
    runtime.limits.capacity.string_bytes=limit;runtime.begin_job();
    assert_eq!(runtime.execute("publications;",&mut host).unwrap().to_text(),"1");
    runtime.begin_job();
    assert!(matches!(runtime.string_from_char_code(&[probe,Value::Number(65.0)],&mut host).unwrap(),Value::StringUnits(units) if units==[0xd800,65]));
    runtime.begin_job();assert_eq!(runtime.execute("publications;",&mut host).unwrap().to_text(),"2");
}
