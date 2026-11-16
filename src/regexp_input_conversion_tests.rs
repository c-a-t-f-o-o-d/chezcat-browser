//! Original finite RegExp exec/test input conversion controls; no library replay.
use super::*;

#[derive(Default)]
struct NoHost { reads: usize }
impl Host for NoHost {
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{self.reads+=1;Err("Unexpected native read".into())}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("Unexpected native write".into())}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("Unexpected native call".into())}
}
fn succeeds(source:&str) {
    let mut runtime=Runtime::for_capacity_test();
    assert_eq!(runtime.execute(source,&mut NoHost::default()).unwrap().to_text(),"ok");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn regexp_input_conversion_uses_actual_function_source_and_public_overrides() {
    succeeds(r#"
function original(){return 1;}
if(!/^function original/.test(original)||!RegExp.prototype.test.call(/^function original/,original))throw 'script Function input';
if(!/\[native code\]/.test(Array)||!/\[native code\]/.test(original.bind(null)))throw 'actual native and bound Function input';
var nameReads=0;Object.defineProperty(Array,'name',{get:function(){nameReads++;throw 'public name getter';},configurable:true});
if(!/\[native code\]/.test(Array)||nameReads)throw 'native source does not read mutable name';
var calls=0;original.toString=function(){if(this!==original)throw 'function receiver';calls++;return 'override';};
if(!/^override$/.test(original)||calls!==1||!/^function original/.test(Function.prototype.toString.call(original)))throw 'ordinary function conversion override';
if(!/^undefined$/.test()||/^undefined$/.exec()[0]!=='undefined'||!/^null$/.test(null)||/^null$/.exec(null)[0]!=='null')throw 'missing and null ToString';
'ok';
"#);
}

#[test]
fn regexp_input_conversion_calls_once_with_string_hint_and_ordered_fallback() {
    succeeds(r#"
var gets=0,calls=0,value={};Object.defineProperty(value,Symbol.toPrimitive,{get:function(){gets++;return function(h){if(this!==value||h!=='string'||arguments.length!==1)throw 'hint receiver arity';calls++;return 'needle';};}});
if(!/needle/.test(value)||gets!==1||calls!==1)throw 'test ToPrimitive once';
var match=RegExp.prototype.exec.call(/needle/,value);if(match[0]!=='needle'||match.input!=='needle'||gets!==2||calls!==2)throw 'exec ToPrimitive once and actual input';
var order='',fallback={};fallback[Symbol.toPrimitive]=null;
Object.defineProperty(fallback,'toString',{get:function(){order+='s';return function(){if(this!==fallback)throw 'fallback receiver';order+='S';return {};};}});
Object.defineProperty(fallback,'valueOf',{get:function(){order+='v';return function(){order+='V';return 'needle';};}});
if(!/needle/.test(fallback)||order!=='sSvV')throw 'string-preferred ordinary order';
'ok';
"#);
}

#[test]
fn regexp_input_conversion_preserves_abrupt_identity_symbols_and_brand_order() {
    succeeds(r#"
var marker={},r=/x/g;r.lastIndex=1;var count=0;
var getter={};Object.defineProperty(getter,Symbol.toPrimitive,{get:function(){count++;throw marker;}});
var method={};method[Symbol.toPrimitive]=function(h){count++;throw marker;};
var ordinary={toString:function(){count++;throw marker;}};
for(var value of [getter,method,ordinary]){var caught=false;try{r.test(value);}catch(e){caught=e===marker;}if(!caught||r.lastIndex!==1)throw 'author abrupt identity before matching state';}
if(count!==3)throw 'abrupt callbacks once';
var errors=0,converted=0,input={toString:function(){converted++;return 'x';}};
try{RegExp.prototype.exec.call({},input);}catch(e){if(e instanceof TypeError)errors++;else throw e;}
try{RegExp.prototype.test.call(null,input);}catch(e){if(e instanceof TypeError)errors++;else throw e;}
if(errors!==2||converted)throw 'invalid receiver before input conversion';
for(var operation of [function(){r.test(Symbol('x'));},function(){r.exec(Symbol('x'));},function(){var value={};value[Symbol.toPrimitive]=function(){return Symbol('x');};r.test(value);}]){try{operation();}catch(e){if(e instanceof TypeError)errors++;else throw e;}}
if(errors!==5||r.lastIndex!==1)throw 'Symbol ToString failure does not run matcher';
'ok';
"#);
}

#[test]
fn regexp_input_conversion_reentrancy_precedes_actual_last_index_read() {
    succeeds(r#"
var r=/a/g,conversions=0,input={toString:function(){conversions++;r.lastIndex=1;return 'ba';}};
var match=r.exec(input);if(conversions!==1||match.index!==1||match[0]!=='a'||match.input!=='ba'||r.lastIndex!==2)throw 'conversion before lastIndex';
var nested=/x/g,value={};value[Symbol.toPrimitive]=function(h){if(h!=='string')throw 'nested hint';var inner=nested.exec('x');if(inner.index!==0)throw 'real reentrant match';nested.lastIndex=0;return 'x';};
if(!RegExp.prototype.test.call(nested,value)||nested.lastIndex!==1)throw 'same-regexp reentry uses current state';
'ok';
"#);
}

#[test]
fn regexp_input_conversion_keeps_native_capability_denial_and_recovers() {
    let mut runtime=Runtime::for_capacity_test();let mut host=NoHost::default();
    runtime.define_global("opaque",Value::Host(42)).unwrap();
    runtime.execute("var r=/x/g;",&mut host).unwrap();
    let error=runtime.execute("r.test(opaque);",&mut host).unwrap_err();
    assert!(error.contains("Native host object property-key conversion"),"{error}");
    assert_eq!(host.reads,0);
    assert_eq!(runtime.call_depth,0);assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    assert_eq!(runtime.execute("r.lastIndex===0&&r.test('x');",&mut host).unwrap().to_text(),"true");
}

#[test]
fn regexp_input_conversion_budget_failure_does_not_reset_job_or_run_matcher() {
    let mut runtime=Runtime::for_capacity_test();let mut host=NoHost::default();
    runtime.execute("var r=/x/g,input={toString:function(){return 'xxxxxxxx';}};",&mut host).unwrap();
    let Value::Object(regex)=runtime.execute("r;",&mut host).unwrap() else {panic!("real regexp")};
    let input=runtime.execute("input;",&mut host).unwrap();
    runtime.begin_job();runtime.copy_bytes=0;let deadline=runtime.job_deadline;
    let error=runtime.regexp_method(regex,"test",&[input],&mut host).unwrap_err();
    assert!(error.exception.is_none(),"budget exhaustion remains fatal");
    assert!(error.message.contains("String/copy work limit"),"{}",error.message);
    assert_eq!(runtime.copy_bytes,0);assert_eq!(runtime.job_deadline,deadline);
    assert_eq!(runtime.call_depth,0);assert!(runtime.transient_registration_is_empty());
    // The exhausted job must refuse even diagnostic property-copy work.
    // A fresh job resets work allowances, not the persistent RegExp state.
    runtime.begin_job();
    assert!(matches!(runtime.get_property(regex,"lastIndex").unwrap(),Value::Number(value) if value==0.0));
    assert_eq!(runtime.execute("r.test('x');",&mut host).unwrap().to_text(),"true");
}
