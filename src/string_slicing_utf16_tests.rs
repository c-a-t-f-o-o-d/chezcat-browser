use super::*;

struct NoHost;

impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("No native object is exposed in this test.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("No native object is exposed in this test.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("No native object is exposed in this test.".into())
    }
}

fn run(source: &str) -> Value {
    let mut runtime = Runtime::for_test();
    let result = runtime.execute(source, &mut NoHost).unwrap();
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    result
}

fn succeeds(source: &str) {
    assert_eq!(run(source).to_text(), "ok");
}

fn scalar(source: &str, expected: &str) {
    let Value::String(text) = run(source) else {
        panic!("A well-formed selected range must use scalar storage");
    };
    assert_eq!(text, expected);
}

fn raw(source: &str, expected: &[u16]) {
    let Value::StringUnits(units) = run(source) else {
        panic!("An unpaired selected range must retain exact code units");
    };
    assert_eq!(units, expected);
}

#[test]
fn string_slicing_utf16_retains_isolates_embedded_null_and_original_spelling() {
    for source in [
        r#"'x\ud800\u0000\udc00y'.slice(1,4);"#,
        r#"'x\ud800\u0000\udc00y'.substring(4,1);"#,
        r#"'x\ud800\u0000\udc00y'.substr(1,3);"#,
    ] {
        raw(source, &[0xd800, 0, 0xdc00]);
    }
    succeeds(r#"
var s='x\ud800\u0000\udc00y',methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var m=methods[i],result=s[m](1,m==='substr'?3:4);
  if(result==='\ufffd\u0000\ufffd'||result==='\\ud800\u0000\\udc00')throw 'projection or spelling';
  if(s.length!==5||s.charCodeAt(1)!==0xd800||s.charCodeAt(3)!==0xdc00)throw 'source mutation';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_splits_scalar_pairs_at_either_code_unit() {
    for source in [
        r#"'A\ud83d\ude00Z'.slice(1,2);"#,
        r#"'A\ud83d\ude00Z'.substring(2,1);"#,
        r#"'A\ud83d\ude00Z'.substr(1,1);"#,
    ] {
        raw(source, &[0xd83d]);
    }
    for source in [
        r#"'A\ud83d\ude00Z'.slice(2,3);"#,
        r#"'A\ud83d\ude00Z'.substring(3,2);"#,
        r#"'A\ud83d\ude00Z'.substr(-2,1);"#,
    ] {
        raw(source, &[0xde00]);
    }
}

#[test]
fn string_slicing_utf16_removing_isolates_canonicalizes_pair_scalar_and_empty_results() {
    for source in [
        r#"'h\ud800A\ud83d\ude00B\udfffz'.slice(3,5);"#,
        r#"'h\ud800A\ud83d\ude00B\udfffz'.substring(5,3);"#,
        r#"'h\ud800A\ud83d\ude00B\udfffz'.substr(3,2);"#,
    ] {
        scalar(source, "😀");
    }
    for source in [
        r#"'h\ud800A\udfffz'.slice(2,3);"#,
        r#"'h\ud800A\udfffz'.substring(3,2);"#,
        r#"'h\ud800A\udfffz'.substr(2,1);"#,
    ] {
        scalar(source, "A");
    }
    for source in [
        r#"'\ud800'.slice(1,0);"#,
        r#"'\ud800'.substring(1,1);"#,
        r#"'\ud800'.substr(0,0);"#,
    ] {
        scalar(source, "");
    }
}

#[test]
fn string_slicing_utf16_native_well_formed_unit_storage_uses_scalar_result_identity() {
    for method in ["slice", "substring", "substr"] {
        for (units, expected) in [
            (vec![0x61, 0], "a\0"),
            (vec![0xd83d, 0xde00], "😀"),
            (Vec::new(), ""),
        ] {
            let mut runtime = Runtime::for_test();
            runtime.define_global("source", Value::StringUnits(units)).unwrap();
            let result = runtime.execute(&format!("source.{method}(0);"), &mut NoHost).unwrap();
            let Value::String(text) = result else {
                panic!("Well-formed native code units must canonicalize");
            };
            assert_eq!(text, expected);
        }
    }
}

#[test]
fn string_slicing_utf16_slice_clamps_relative_indices_after_integer_conversion() {
    succeeds(r#"
var s='0\ud8002\udc004',cases=[
 [s.slice(),s],[s.slice(undefined,undefined),s],[s.slice(-2),'\udc004'],
 [s.slice(-20,3),'0\ud8002'],[s.slice(20,3),''],[s.slice(3,1),''],
 [s.slice(1.9,3.9),'\ud8002'],[s.slice(-3.9,-1.9),'2\udc00'],[s.slice(-0,2),'0\ud800'],
 [s.slice(NaN,NaN),''],[s.slice(NaN,Infinity),s],[s.slice(-Infinity,Infinity),s],
 [s.slice(Infinity,Infinity),''],[s.slice(1,-Infinity),''],[s.slice(1,null),''],
 [s.slice(null,true),'0'],[s.slice('1','3'),'\ud8002'],[s.slice(1e100),'' ],
 [s.slice(-1e100,1e100),s],[s.slice(0,-0),'']
];
for(var i=0;i<cases.length;i++)if(cases[i][0]!==cases[i][1])throw 'slice index '+i;
'ok';
"#);
}

#[test]
fn string_slicing_utf16_substring_clamps_then_swaps_indices() {
    succeeds(r#"
var s='0\ud8002\udc004',cases=[
 [s.substring(),s],[s.substring(undefined,undefined),s],[s.substring(-2),s],
 [s.substring(-20,3),'0\ud8002'],[s.substring(20,3),'\udc004'],[s.substring(3,1),'\ud8002'],
 [s.substring(1.9,3.9),'\ud8002'],[s.substring(-3.9,-1.9),''],[s.substring(-0,2),'0\ud800'],
 [s.substring(NaN,NaN),''],[s.substring(NaN,Infinity),s],[s.substring(-Infinity,Infinity),s],
 [s.substring(Infinity,Infinity),''],[s.substring(1,-Infinity),'0'],[s.substring(1,null),'0'],
 [s.substring(null,true),'0'],[s.substring('1','3'),'\ud8002'],[s.substring(1e100),'' ],
 [s.substring(-1e100,1e100),s],[s.substring(0,-0),'']
];
for(var i=0;i<cases.length;i++)if(cases[i][0]!==cases[i][1])throw 'substring index '+i;
'ok';
"#);
}

#[test]
fn string_slicing_utf16_substr_keeps_relative_start_and_nonnegative_count() {
    succeeds(r#"
var s='0\ud8002\udc004',cases=[
 [s.substr(),s],[s.substr(undefined,undefined),s],[s.substr(-2),'\udc004'],
 [s.substr(-20,3),'0\ud8002'],[s.substr(20,3),''],[s.substr(3,1),'\udc00'],
 [s.substr(1.9,2.9),'\ud8002'],[s.substr(-3.9,2.9),'2\udc00'],[s.substr(-0,2),'0\ud800'],
 [s.substr(NaN,NaN),''],[s.substr(NaN,Infinity),s],[s.substr(-Infinity,Infinity),s],
 [s.substr(Infinity,Infinity),''],[s.substr(1,-Infinity),''],[s.substr(1,null),''],
 [s.substr(1,-1),''],[s.substr(null,true),'0'],[s.substr('1','2'),'\ud8002'],
 [s.substr(1e100),'' ],[s.substr(-1e100,1e100),s],[s.substr(0,-0),'']
];
for(var i=0;i<cases.length;i++)if(cases[i][0]!==cases[i][1])throw 'substr index '+i;
'ok';
"#);
}

#[test]
fn string_slicing_utf16_generic_primitives_and_boxed_scalar_receivers_use_string_conversion() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var name=methods[i],f=String.prototype[name],end=name==='substr'?1:2;
  if(f.call(12345,1,end)!=='2'||f.call(true,1,end)!=='r'||f.call(BigInt('12345'),1,end)!=='2')throw 'generic primitive';
  var boxed=new String('A\ud83d\ude00Z'),result=f.call(boxed,1,end);
  if(result.length!==1||result.charCodeAt(0)!==0xd83d)throw 'boxed scalar';
  var calls=0;
  boxed.toString=function(){calls++;return 'A\ud800Z';};
  if(f.call(boxed,1,end)!=='\ud800'||calls!==1)throw 'boxed override';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_receiver_and_index_exotic_hooks_run_once_in_order() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var name=methods[i],f=String.prototype[name],order='',receiver={},start={},end={};
  Object.defineProperty(receiver,Symbol.toPrimitive,{get:function(){order+='R';return function(hint){if(this!==receiver||hint!=='string')throw 'receiver hint';order+='r';return 'A\ud800Z';};}});
  Object.defineProperty(start,Symbol.toPrimitive,{get:function(){order+='S';return function(hint){if(this!==start||hint!=='number')throw 'start hint';order+='s';return 1;};}});
  Object.defineProperty(end,Symbol.toPrimitive,{get:function(){order+='E';return function(hint){if(this!==end||hint!=='number')throw 'end hint';order+='e';return name==='substr'?1:2;};}});
  if(f.call(receiver,start,end)!=='\ud800'||order!=='RrSsEe')throw 'conversion order '+name+':'+order;
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_ordinary_conversion_fallbacks_use_string_and_number_hints() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var name=methods[i],f=String.prototype[name],order='';
  var receiver={toString:function(){order+='r';return {};},valueOf:function(){order+='R';return 'A\ud800Z';}};
  var start={valueOf:function(){order+='s';return {};},toString:function(){order+='S';return '1';}};
  var end={valueOf:function(){order+='e';return name==='substr'?1:2;},toString:function(){throw 'unexpected fallback';}};
  if(f.call(receiver,start,end)!=='\ud800'||order!=='rRsSe')throw 'ordinary hints '+order;
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_live_index_hooks_see_mutations_but_keep_the_captured_source() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var name=methods[i],f=String.prototype[name],order='',source='A\ud800Z';
  var receiver={toString:function(){order+='r';return source;}};
  var end={valueOf:function(){throw 'stale end';}};
  var start={valueOf:function(){order+='s';source='changed';receiver.toString=function(){throw 'second receiver conversion';};end.valueOf=function(){order+='e';return name==='substr'?1:2;};return 1;}};
  if(f.call(receiver,start,end)!=='\ud800'||order!=='rse'||source!=='changed')throw 'live hooks and snapshot';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_nullish_and_symbol_receivers_refuse_before_index_conversion() {
    succeeds(r#"
var methods=['slice','substring','substr'],receivers=[null,undefined,Symbol('receiver')];
for(var i=0;i<methods.length;i++)for(var j=0;j<receivers.length;j++){
  var calls=0,index={valueOf:function(){calls++;return 0;}},caught=false;
  try{String.prototype[methods[i]].call(receivers[j],index,index);}catch(e){caught=e instanceof TypeError;}
  if(!caught||calls!==0)throw 'receiver refusal order';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_abrupt_receiver_and_index_hooks_preserve_identity_and_effects() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var f=String.prototype[methods[i]],marker={},order='',caught=false;
  var receiver={toString:function(){order+='r';throw marker;}};
  var start={valueOf:function(){order+='s';return 1;}},end={valueOf:function(){order+='e';return 2;}};
  try{f.call(receiver,start,end);}catch(e){caught=e===marker;}
  if(!caught||order!=='r')throw 'receiver throw identity';
  order='';caught=false;receiver.toString=function(){order+='r';return 'A\ud800Z';};
  start.valueOf=function(){order+='s';throw marker;};
  try{f.call(receiver,start,end);}catch(e){caught=e===marker;}
  if(!caught||order!=='rs')throw 'start throw identity';
  order='';caught=false;start.valueOf=function(){order+='s';return 1;};
  end.valueOf=function(){order+='e';throw marker;};
  try{f.call(receiver,start,end);}catch(e){caught=e===marker;}
  if(!caught||order!=='rse')throw 'end throw identity';
  if(f.call('A\ud800Z',0,1)!=='A')throw 'subsequent call recovery';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_empty_ranges_still_convert_both_arguments() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var name=methods[i],f=String.prototype[name],order='';
  var start={valueOf:function(){order+='s';return Infinity;}};
  var end={valueOf:function(){order+='e';return name==='substr'?0:Infinity;}};
  if(f.call('',start,end)!==''||order!=='se')throw 'empty source conversions';
  order='';if(f.call('\ud800',start,end)!==''||order!=='se')throw 'empty selected range conversions';
  var marker={},caught=false;order='';
  end.valueOf=function(){order+='e';throw marker;};
  try{f.call('',start,end);}catch(e){caught=e===marker;}
  if(!caught||order!=='se')throw 'empty range abrupt completion';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_original_undefined_defaults_differ_from_converted_undefined() {
    succeeds(r#"
var source='A\ud800Z',converted={valueOf:function(){return undefined;}};
if(source.slice(1,undefined)!=='\ud800Z'||source.slice(1,converted)!=='')throw 'slice omitted end';
if(source.substring(1,undefined)!=='\ud800Z'||source.substring(1,converted)!=='A')throw 'substring omitted end';
if(source.substr(1,undefined)!=='\ud800Z'||source.substr(1,converted)!=='')throw 'substr omitted count';
if(source.slice(converted,1)!=='A'||source.substring(converted,1)!=='A'||source.substr(converted,1)!=='A')throw 'converted start';
var extra={valueOf:function(){throw 'extra argument converted';}};
if(source.slice(1,2,extra)!=='\ud800'||source.substring(1,2,extra)!=='\ud800'||source.substr(1,1,extra)!=='\ud800')throw 'extra argument';
'ok';
"#);
}

#[test]
fn string_slicing_utf16_symbol_and_bigint_indices_are_catchable_type_errors() {
    succeeds(r#"
var methods=['slice','substring','substr'],invalid=[Symbol('index'),BigInt('1')];
for(var i=0;i<methods.length;i++)for(var j=0;j<invalid.length;j++){
  var f=String.prototype[methods[i]],calls=0,end={valueOf:function(){calls++;return 1;}},caught=false;
  try{f.call('\ud800',invalid[j],end);}catch(e){caught=e instanceof TypeError;}
  if(!caught||calls!==0)throw 'invalid start';
  caught=false;try{f.call('',0,invalid[j]);}catch(e){caught=e instanceof TypeError;}
  if(!caught)throw 'invalid end';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_reentrant_conversion_preserves_each_call_source_and_indices() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var name=methods[i],f=String.prototype[name],order='';
  var receiver={toString:function(){order+='r';if('\udc00a'.slice(0,1)!=='\udc00')throw 'receiver reentry';return 'A\ud800Z';}};
  var start={valueOf:function(){order+='s';if('x\udc00'.substring(1)!=='\udc00')throw 'start reentry';return 1;}};
  var end={valueOf:function(){order+='e';if('\ud800y'.substr(0,1)!=='\ud800')throw 'end reentry';return name==='substr'?1:2;}};
  if(f.call(receiver,start,end)!=='\ud800'||order!=='rse')throw 'outer state after reentry';
}
'ok';
"#);
}

#[test]
fn string_slicing_utf16_intrinsic_metadata_and_prototype_overrides_remain_ordinary() {
    succeeds(r#"
var methods=['slice','substring','substr'];
for(var i=0;i<methods.length;i++){
  var name=methods[i],f=String.prototype[name],d=Object.getOwnPropertyDescriptor(String.prototype,name);
  if(f.name!==name||f.length!==2||d.value!==f||d.enumerable||!d.writable||!d.configurable)throw 'method metadata';
  if(Function.prototype.toString.call(f).indexOf('[native code]')<0)throw 'native source';
  String.prototype[name]=function(){'use strict';if(this!=='\ud800')throw 'override receiver';return 'replacement';};
  if('\ud800'[name]()!=='replacement')throw 'prototype override';
  if(!delete String.prototype[name]||'\ud800'[name]!==undefined)throw 'prototype deletion';
  String.prototype[name]=f;
  if('\ud800'[name](0,1)!=='\ud800')throw 'prototype restore';
}
'ok';
"#);
}
