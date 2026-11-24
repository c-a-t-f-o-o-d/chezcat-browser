//! Original exact character consumers and numeric conversion ordering.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
}
fn succeeds(source:&str){Runtime::for_test().execute(source,&mut NoHost).unwrap();}

#[test]
fn character_position_scalar_raw_and_boxed_numeric_hooks() {
    succeeds(r#"
var cases=[['A😀Z',1,'\ud83d',0xd83d,0x1f600],['A😀Z',2,'\ude00',0xde00,0xde00],['A\ud800Z',1,'\ud800',0xd800,0xd800],['A\udfffZ',1,'\udfff',0xdfff,0xdfff]];
for(var c of cases){for(var receiver of [c[0],new String(c[0])]){var calls=0,p={valueOf:function(){calls++;return c[1];},toString:function(){throw 'wrong numeric hint';}};
if(receiver.charAt(p)!==c[2]||receiver.charCodeAt(p)!==c[3]||receiver.codePointAt(p)!==c[4]||calls!==3)throw 'exact character conversion';}}
"#);
}

#[test]
fn character_position_receiver_precedes_once_only_numeric_hook_and_capture() {
    succeeds(r#"
for(var method of ['charAt','charCodeAt','codePointAt']){var f=String.prototype[method],order='',text='A\ud800Z';
var receiver={[Symbol.toPrimitive]:function(h){order+='R'+h+';';return text;}},position={[Symbol.toPrimitive]:function(h){order+='P'+h+';';text='changed';if('nested'.charAt(1)!=='e')throw 'nested';return 1;}};
var value=f.call(receiver,position,{valueOf:function(){throw 'extra';}});
if(order!=='Rstring;Pnumber;'||value!==(method==='charAt'?'\ud800':0xd800))throw 'ordered captured receiver';}
"#);
}

#[test]
fn character_position_empty_range_abrupt_type_errors_and_recovery() {
    succeeds(r#"
for(var method of ['charAt','charCodeAt','codePointAt']){var f=String.prototype[method],calls=0,marker={};
try{f.call('',{valueOf:function(){calls++;throw marker;}});throw 'missing throw';}catch(e){if(e!==marker)throw e;}
if(calls!==1)throw 'empty conversion';
try{f.call(null,{valueOf:function(){throw 'late hook';}});throw 'missing null error';}catch(e){if(!(e instanceof TypeError))throw e;}
for(var bad of [Symbol('position'),BigInt(1)]){try{f.call('A\ud800',bad);throw 'missing numeric error';}catch(e){if(!(e instanceof TypeError))throw e;}}
var miss=f.call('A\ud800',{valueOf:function(){calls++;return Infinity;}});
if(method==='charAt'?miss!=='':method==='codePointAt'?miss!==undefined:!isNaN(miss))throw 'out of range';
var first=f.call('A\ud800',{valueOf:function(){return undefined;}});
if(first!==(method==='charAt'?'A':65)||calls!==2)throw 'undefined numeric default';
if(f.call('A\ud800',{valueOf:function(){return 1.9;}})!==(method==='charAt'?'\ud800':0xd800))throw 'recovery';}
"#);
}

#[test]
fn character_position_work_step_and_exact_output_refusal_recover() {
    let mut runtime=Runtime::for_test();let mut host=NoHost;
    let receiver=Value::StringUnits(vec![0xd800,65]);let args=[Value::Number(0.0)];
    let retained=runtime.value_bytes;
    runtime.begin_job();runtime.copy_bytes=0;
    let error=runtime.string_character_value(&receiver,"charAt",&args,&mut host).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("work limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.begin_job();runtime.steps=0;
    let error=runtime.string_character_value(&receiver,"charCodeAt",&args,&mut host).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("step limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.begin_job();let limit=runtime.limits.capacity.string_bytes;runtime.limits.capacity.string_bytes=1;
    let error=runtime.string_character_value(&receiver,"charAt",&args,&mut host).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("String limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.limits.capacity.string_bytes=limit;runtime.begin_job();
    assert!(matches!(runtime.string_character_value(&receiver,"charAt",&args,&mut host).unwrap(),Value::StringUnits(units) if units==[0xd800]));
    assert_eq!(runtime.value_bytes,retained);
}

#[test]
fn character_position_late_exact_output_refusal_retains_author_mutation() {
    let mut runtime=Runtime::for_test();let mut host=NoHost;
    let position=runtime.execute("var publications=0,position={valueOf:function(){publications++;return 0;}};position;",&mut host).unwrap();
    let limit=runtime.limits.capacity.string_bytes;runtime.limits.capacity.string_bytes=1;runtime.begin_job();
    let receiver=Value::StringUnits(vec![0xd800,65]);
    let error=runtime.string_character_value(&receiver,"charAt",&[position.clone()],&mut host).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("String limit"),"{}",error.message);
    runtime.limits.capacity.string_bytes=limit;runtime.begin_job();
    assert_eq!(runtime.execute("publications;",&mut host).unwrap().to_text(),"1");
    runtime.begin_job();
    assert!(matches!(runtime.string_character_value(&receiver,"charAt",&[position],&mut host).unwrap(),Value::StringUnits(units) if units==[0xd800]));
    runtime.begin_job();assert_eq!(runtime.execute("publications;",&mut host).unwrap().to_text(),"2");
}

#[test]
fn character_position_canonical_capture_and_numeric_fallback() {
    succeeds(r#"
for(var method of ['charAt','charCodeAt','codePointAt']){var f=String.prototype[method],descriptor=Object.getOwnPropertyDescriptor(String.prototype,method);
if(f.length!==1||f.name!==method||!descriptor.writable||descriptor.enumerable||!descriptor.configurable)throw 'metadata';
var order='',p={valueOf:function(){order+='V';String.prototype[method]=function(){return 'replacement';};return {};},toString:function(){order+='S';return '1';}};
if(f.call('A\ud800',p)!==(method==='charAt'?'\ud800':0xd800)||order!=='VS'||'abc'[method](0)!=='replacement')throw 'captured native';
String.prototype[method]=f;
for(var n of [undefined,NaN,-0,null,false,'0']){if(f.call('A\ud800',n)!==(method==='charAt'?'A':65))throw 'numeric zero';}
for(var n of [-1,-Infinity,Infinity]){var value=f.call('A\ud800',n);if(method==='charAt'?value!=='':method==='codePointAt'?value!==undefined:!isNaN(value))throw 'numeric range';}}
"#);
}
