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

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
}

#[test]
fn text_utf16_encoder_replaces_each_isolate_and_keeps_pairs_and_nulls() {
    succeeds(r#"
var encoder=new TextEncoder(),cases=[
 ['\ud800','239,191,189'],['\udfff','239,191,189'],
 ['\ud800\ud800','239,191,189,239,191,189'],['\udc00\ud800','239,191,189,239,191,189'],
 ['\ud800A\udc00','239,191,189,65,239,191,189'],
 ['\ud83d\ude00','240,159,152,128'],['\udbff\udfff','244,143,191,191'],
 ['\udc00\ud83d\ude00\ud800','239,191,189,240,159,152,128,239,191,189'],
 ['\u0000\ud800\u0000','0,239,191,189,0'],['\ufffd','239,191,189'],
 ['\\ud800','92,117,100,56,48,48'],['Aé中','65,195,169,228,184,173']
];
for(var i=0;i<cases.length;i++){
  var result=encoder.encode(cases[i][0]);
  if(result.join(',')!==cases[i][1]||!(result instanceof Uint8Array))throw 'UTF-8 bytes '+i;
}
if(cases[0][0].charCodeAt(0)!==0xd800||cases[4][0].charCodeAt(2)!==0xdc00)throw 'source mutation';
'ok';
"#);
}

#[test]
fn text_utf16_encoder_defaults_and_ordinary_primitive_strings_are_distinct() {
    succeeds(r#"
var encoder=new TextEncoder();
if(encoder.encode().length!==0||encoder.encode(undefined).length!==0||encoder.encode('').length!==0)throw 'default source';
var cases=[[null,'110,117,108,108'],[false,'102,97,108,115,101'],[0,'48'],[BigInt('12'),'49,50']];
for(var i=0;i<cases.length;i++)if(encoder.encode(cases[i][0]).join(',')!==cases[i][1])throw 'primitive source '+i;
var converted={toString:function(){return undefined;}};
if(encoder.encode(converted).join(',')!=='117,110,100,101,102,105,110,101,100')throw 'converted undefined';
var a=encoder.encode('\ud800'),b=encoder.encode('\ud800');
if(a===b||a.buffer===b.buffer)throw 'fresh outputs';a[0]=0;
if(b[0]!==239)throw 'independent outputs';
var unused={toString:function(){throw 'constructor argument converted';}};
if(new TextEncoder(unused).encoding!=='utf-8')throw 'encoder constructor';
'ok';
"#);
}

#[test]
fn text_utf16_encoder_exotic_string_conversion_runs_once() {
    succeeds(r#"
var encoder=new TextEncoder(),source={},order='';
Object.defineProperty(source,Symbol.toPrimitive,{get:function(){order+='G';return function(hint){if(this!==source||hint!=='string')throw 'string hint';order+='C';return '\ud800A\udc00';};}});
if(encoder.encode(source).join(',')!=='239,191,189,65,239,191,189'||order!=='GC')throw 'once-only conversion';
'ok';
"#);
}

#[test]
fn text_utf16_encoder_ordinary_fallback_and_boxed_scalar_override_keep_raw_results() {
    succeeds(r#"
var encoder=new TextEncoder(),order='';
var source={toString:function(){order+='s';return {};},valueOf:function(){order+='v';return '\ud800';}};
if(encoder.encode(source).join(',')!=='239,191,189'||order!=='sv')throw 'ordinary string hint';
var boxed=new String('ordinary'),calls=0;
boxed.toString=function(){calls++;return '\udc00';};
if(encoder.encode(boxed).join(',')!=='239,191,189'||calls!==1)throw 'boxed override';
if(encoder.encode(['\ud800','\udc00']).join(',')!=='239,191,189,44,239,191,189')throw 'array conversion';
'ok';
"#);
}

#[test]
fn text_utf16_encoder_conversion_failures_keep_thrown_identity_and_recover() {
    succeeds(r#"
var encoder=new TextEncoder(),marker={},order='',source={},caught=false;
Object.defineProperty(source,Symbol.toPrimitive,{get:function(){order+='g';throw marker;}});
try{encoder.encode(source);}catch(e){caught=e===marker;}
if(!caught||order!=='g')throw 'getter failure';
source={toString:function(){order+='s';throw marker;},valueOf:function(){order+='v';return 'unused';}};
caught=false;try{encoder.encode(source);}catch(e){caught=e===marker;}
if(!caught||order!=='gs')throw 'method failure';
for(var value of [Symbol('source'),{toString:function(){return {};},valueOf:function(){return {};}}]){
  caught=false;try{encoder.encode(value);}catch(e){caught=e instanceof TypeError;}
  if(!caught)throw 'conversion TypeError';
}
if(encoder.encode('\ud800').join(',')!=='239,191,189')throw 'subsequent call';
'ok';
"#);
}

#[test]
fn text_utf16_encoder_reentry_uses_independent_conversion_and_output_state() {
    succeeds(r#"
var encoder=new TextEncoder(),calls=0,inner;
var source={toString:function(){calls++;inner=encoder.encode('\udc00A');return '\ud800B';}};
var outer=encoder.encode(source);
if(calls!==1||inner.join(',')!=='239,191,189,65'||outer.join(',')!=='239,191,189,66'||inner===outer)throw 'reentry';
'ok';
"#);
}

#[test]
fn text_utf16_encode_into_whole_scalars_have_exact_source_unit_progress() {
    succeeds(r#"
var encoder=new TextEncoder(),source='A\ud800B\udc00\ud83d\ude00\u0000';
var expected=[65,239,191,189,66,239,191,189,240,159,152,128,0];
var reads=[0,1,1,1,2,3,3,3,4,4,4,4,6,7,7],writes=[0,1,1,1,4,5,5,5,8,8,8,8,12,13,13];
for(var length=0;length<reads.length;length++){
  var output=new Uint8Array(length);
  for(var i=0;i<length;i++)output[i]=170;
  var result=encoder.encodeInto(source,output);
  if(result.read!==reads[length]||result.written!==writes[length])throw 'progress '+length;
  for(var i=0;i<length;i++)if(output[i]!== (i<result.written?expected[i]:170))throw 'whole scalar bytes '+length+':'+i;
}
if(source.length!==7||source.charCodeAt(1)!==0xd800||source.charCodeAt(3)!==0xdc00)throw 'original source units';
'ok';
"#);
}

#[test]
fn text_utf16_encode_into_subviews_respect_offsets_and_existing_aliases() {
    succeeds(r#"
var encoder=new TextEncoder(),buffer=new ArrayBuffer(12),whole=new Uint8Array(buffer),view=new Uint8Array(buffer,2,7),alias=new DataView(buffer);
for(var i=0;i<whole.length;i++)whole[i]=170;
var result=encoder.encodeInto('\ud800\ud83d\ude00Z',view);
if(result.read!==3||result.written!==7)throw 'subview progress';
if(whole.join(',')!=='170,170,239,191,189,240,159,152,128,170,170,170')throw 'subview extent';
if(alias.getUint8(2)!==239||alias.getUint8(8)!==128||alias.getUint8(9)!==170)throw 'alias observation';
'ok';
"#);
}

#[test]
fn text_utf16_encode_into_explicit_nullish_sources_are_required_string_values() {
    succeeds(r#"
var encoder=new TextEncoder(),output=new Uint8Array(9),result=encoder.encodeInto(undefined,output);
if(result.read!==9||result.written!==9||output.join(',')!=='117,110,100,101,102,105,110,101,100')throw 'explicit undefined';
output=new Uint8Array(5);result=encoder.encodeInto(null,output);
if(result.read!==4||result.written!==4||output.join(',')!=='110,117,108,108,0')throw 'null source';
result=encoder.encodeInto('',output);
if(result.read!==0||result.written!==0||output.join(',')!=='110,117,108,108,0')throw 'empty source';
'ok';
"#);
}

#[test]
fn text_utf16_encode_into_source_hooks_precede_destination_validation() {
    succeeds(r#"
var encoder=new TextEncoder(),calls=0,source={toString:function(){calls++;return '\ud800';}};
var invalid=[{},new Uint16Array(4),new Int8Array(4),new Uint8ClampedArray(4),new ArrayBuffer(4),new DataView(new ArrayBuffer(4)),null,undefined];
for(var i=0;i<invalid.length;i++){
  var caught=false;
  try{encoder.encodeInto(source,invalid[i]);}catch(e){caught=e instanceof TypeError;}
  if(!caught||calls!==i+1)throw 'source before destination '+i;
}
var output=new Uint8Array(3),result=encoder.encodeInto(source,output);
if(calls!==invalid.length+1||result.read!==1||result.written!==3||output.join(',')!=='239,191,189')throw 'valid destination';
'ok';
"#);
}

#[test]
fn text_utf16_encode_into_receiver_and_arity_checks_precede_source_hooks() {
    succeeds(r#"
var encoder=new TextEncoder(),calls=0,source={toString:function(){calls++;return '\ud800';}},output=new Uint8Array(3);
var receivers=[{},null,undefined,new TextDecoder(),Object.create(encoder)];
for(var i=0;i<receivers.length;i++)for(var name of ['encode','encodeInto']){
  var caught=false;
  try{TextEncoder.prototype[name].call(receivers[i],source,output);}catch(e){caught=e instanceof TypeError;}
  if(!caught||calls!==0)throw 'receiver before source';
}
for(var args of [[],[source]]){
  var caught=false;try{encoder.encodeInto.apply(encoder,args);}catch(e){caught=e instanceof TypeError;}
  if(!caught||calls!==0)throw 'required source and destination';
}
if(output.join(',')!=='0,0,0')throw 'no destination mutation';
'ok';
"#);
}

#[test]
fn text_utf16_encode_into_reentrant_source_conversion_retains_live_alias_writes() {
    succeeds(r#"
var encoder=new TextEncoder(),whole=new Uint8Array([9,9,9,9,9,9]),view=new Uint8Array(whole.buffer,1,4),calls=0,source={};
Object.defineProperty(source,Symbol.toPrimitive,{get:function(){calls++;return function(hint){
  if(this!==source||hint!=='string')throw 'source hook';
  var inner=encoder.encodeInto('X',view);
  if(inner.read!==1||inner.written!==1||whole[1]!==88)throw 'inner write';
  new DataView(whole.buffer).setUint8(4,77);
  return '\ud800A';
};}});
var result=encoder.encodeInto(source,view);
if(calls!==1||result.read!==2||result.written!==4||whole.join(',')!=='9,239,191,189,65,9')throw 'outer live write';
'ok';
"#);
}

#[test]
fn text_utf16_encode_into_abrupt_source_keeps_prior_author_mutations() {
    succeeds(r#"
var encoder=new TextEncoder(),output=new Uint8Array([9,9,9]),marker={},calls=0;
var source={toString:function(){calls++;output[0]=7;throw marker;}};
var caught=false;try{encoder.encodeInto(source,output);}catch(e){caught=e===marker;}
if(!caught||calls!==1||output.join(',')!=='7,9,9')throw 'source exception effects';
caught=false;try{encoder.encodeInto(Symbol('source'),output);}catch(e){caught=e instanceof TypeError;}
if(!caught||output.join(',')!=='7,9,9')throw 'symbol input';
var result=encoder.encodeInto('\ud800',output);
if(result.read!==1||result.written!==3||output.join(',')!=='239,191,189')throw 'subsequent encoding';
'ok';
"#);
}

#[test]
fn text_utf16_decoder_unknown_raw_labels_throw_catchable_range_errors() {
    succeeds(r#"
var labels=['\ud800','\udc00','utf-8\ud800','\ud800utf-8','\ud83d\ude00','\ufffd','utf-8\u0000'];
for(var i=0;i<labels.length;i++){
  var caught=false;try{new TextDecoder(labels[i]);}catch(e){caught=e instanceof RangeError;}
  if(!caught)throw 'unknown DOMString label '+i;
}
if(new TextDecoder(' \tUtF-8\r\n').encoding!=='utf-8'||new TextDecoder(undefined).encoding!=='utf-8')throw 'valid label controls';
var converted={toString:function(){return undefined;}},caught=false;
try{new TextDecoder(converted);}catch(e){caught=e instanceof RangeError;}
if(!caught)throw 'converted undefined label is not default';
if(new TextDecoder().decode(new Uint8Array([65]))!=='A')throw 'constructor recovery';
'ok';
"#);
}

#[test]
fn text_utf16_decoder_raw_label_conversion_precedes_ordered_options_and_range_error() {
    succeeds(r#"
var labels=['\ud800','utf-8\udc00','\ud83d\ude00'];
for(var i=0;i<labels.length;i++){
  var label={},order='',returned=false;
  Object.defineProperty(label,Symbol.toPrimitive,{get:function(){order+='G';return function(hint){if(this!==label||hint!=='string')throw 'label hint';order+='L';return labels[i];};}});
  var options={get fatal(){order+='F';if(new TextDecoder('utf-8').decode(new Uint8Array([65]))!=='A')throw 'options reentry';return false;},get ignoreBOM(){order+='B';return true;}};
  var caught=false;try{returned=new TextDecoder(label,options);}catch(e){caught=e instanceof RangeError;order+='R';}
  if(!caught||returned!==false||order!=='GLFBR')throw 'label and dictionary order '+order;
}
'ok';
"#);
}

#[test]
fn text_utf16_decoder_raw_label_options_throws_keep_identity_and_prior_effects() {
    succeeds(r#"
var marker={},order='',caught=false;
var first={get fatal(){order+='F';throw marker;},get ignoreBOM(){order+='B';return false;}};
try{new TextDecoder('\ud800',first);}catch(e){caught=e===marker;}
if(!caught||order!=='F')throw 'first dictionary throw';
order='';caught=false;
var second={get fatal(){order+='F';return false;},get ignoreBOM(){order+='B';throw marker;}};
try{new TextDecoder('\udc00',second);}catch(e){caught=e===marker;}
if(!caught||order!=='FB')throw 'second dictionary throw';
if(new TextDecoder().encoding!=='utf-8')throw 'constructor recovery';
'ok';
"#);
}

#[test]
fn text_utf16_decoder_label_abrupt_conversion_prevents_dictionary_reads() {
    succeeds(r#"
var marker={},order='',label={},options={get fatal(){order+='F';return false;},get ignoreBOM(){order+='B';return false;}},caught=false;
Object.defineProperty(label,Symbol.toPrimitive,{get:function(){order+='G';throw marker;}});
try{new TextDecoder(label,options);}catch(e){caught=e===marker;}
if(!caught||order!=='G')throw 'label getter throw';
label={toString:function(){order+='L';throw marker;},valueOf:function(){order+='V';return 'utf-8';}};
caught=false;try{new TextDecoder(label,options);}catch(e){caught=e===marker;}
if(!caught||order!=='GL')throw 'label call throw';
caught=false;try{new TextDecoder(Symbol('label'),options);}catch(e){caught=e instanceof TypeError;}
if(!caught||order!=='GL')throw 'symbol label';
'ok';
"#);
}

#[test]
fn text_utf16_decoder_dictionary_type_check_precedes_unknown_label_validation() {
    succeeds(r#"
var invalid=[1,'options',false,Symbol('options')],calls=0,label={toString:function(){calls++;return '\ud800';}};
for(var i=0;i<invalid.length;i++){
  var caught=false;try{new TextDecoder(label,invalid[i]);}catch(e){caught=e instanceof TypeError;}
  if(!caught||calls!==i+1)throw 'dictionary type check before label validation';
}
for(var options of [null,undefined]){
  var caught=false;try{new TextDecoder('\ud800',options);}catch(e){caught=e instanceof RangeError;}
  if(!caught)throw 'nullish dictionary';
}
'ok';
"#);
}

#[test]
fn text_utf16_decoder_native_well_formed_units_match_ascii_labels_exactly() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("nativeLabel", Value::StringUnits(" \tUtF-8\r\n".encode_utf16().collect())).unwrap();
    runtime.define_global("nativePair", Value::StringUnits(vec![0xd83d, 0xde00])).unwrap();
    assert_eq!(runtime.execute(r#"
var order='',decoder=new TextDecoder(nativeLabel,{get fatal(){order+='F';return true;},get ignoreBOM(){order+='B';return true;}});
if(decoder.encoding!=='utf-8'||!decoder.fatal||!decoder.ignoreBOM||order!=='FB')throw 'native ASCII DOMString';
order='';var caught=false;
try{new TextDecoder(nativePair,{get fatal(){order+='F';return false;},get ignoreBOM(){order+='B';return false;}});}catch(e){caught=e instanceof RangeError;}
if(!caught||order!=='FB')throw 'native non-ASCII DOMString';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn text_utf16_encoder_native_well_formed_units_and_empty_units_use_scalar_values() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("nativeAscii", Value::StringUnits(vec![65, 0])).unwrap();
    runtime.define_global("nativePair", Value::StringUnits(vec![0xd83d, 0xde00])).unwrap();
    runtime.define_global("nativeEmpty", Value::StringUnits(Vec::new())).unwrap();
    assert_eq!(runtime.execute(r#"
var encoder=new TextEncoder();
if(encoder.encode(nativeAscii).join(',')!=='65,0'||encoder.encode(nativePair).join(',')!=='240,159,152,128'||encoder.encode(nativeEmpty).length!==0)throw 'native unit input';
var output=new Uint8Array(4),result=encoder.encodeInto(nativePair,output);
if(result.read!==2||result.written!==4||output.join(',')!=='240,159,152,128')throw 'native pair progress';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
}
