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
    let value = runtime.execute(source, &mut NoHost).unwrap();
    assert!(runtime.active_array_joins.is_empty());
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    value
}

fn succeeds(source: &str) {
    assert_eq!(run(source).to_text(), "ok");
}

fn scalar_result(source: &str, expected: &str) {
    let Value::String(text) = run(source) else {
        panic!("A fully well-formed join result must use scalar string storage");
    };
    assert_eq!(text, expected);
}

#[test]
fn join_utf16_preserves_raw_values_and_scalar_lookalikes() {
    succeeds(r#"
var high='\ud800',low='\udc00',replacement='\ufffd',spelling='\\ud800';
var result=[high,7,true,replacement,spelling,low,'\u0000'].join('|');
var expected=high+'|7|true|'+replacement+'|'+spelling+'|'+low+'|\u0000';
if(result!==expected||result.charCodeAt(0)!==0xd800)throw 'exact values';
if([high].join('')===replacement||[high].join('')===spelling)throw 'distinct spellings';
'ok';
"#);
}

#[test]
fn join_utf16_raw_separator_converts_once_with_the_string_hint() {
    succeeds(r#"
var high='\ud800',low='\udc00',separator={},reads=0,calls=0;
Object.defineProperty(separator,Symbol.toPrimitive,{get:function(){
    reads++;return function(hint){if(this!==separator||hint!=='string')throw 'separator receiver';calls++;return high+'x'+low;};
}});
var result=[1,2,3].join(separator),text=high+'x'+low;
if(result!=='1'+text+'2'+text+'3'||reads!==1||calls!==1)throw 'separator conversion';
'ok';
"#);
}

#[test]
fn join_utf16_empty_singleton_and_default_separator_keep_their_contracts() {
    succeeds(r#"
var high='\ud800',low='\udc00',calls=0,separator={toString:function(){calls++;return high;}};
if([].join(separator)!==''||calls!==1||[high].join(low)!==high)throw 'zero and one';
if([high,low].join()!==high+','+low||[high,low].join(undefined)!==high+','+low)throw 'default separator';
if([high,low].join(null)!==high+'null'+low)throw 'null separator';
'ok';
"#);
}

#[test]
fn join_utf16_pairs_across_element_boundaries_use_scalar_identity() {
    scalar_result(r#"
var high='\ud800',low='\udc00';
var result=[high,low].join('');
if(result!=='\ud800\udc00'||result.length!==2||result.codePointAt(0)!==0x10000)throw 'paired elements';
result;
"#, "\u{10000}");
}

#[test]
fn join_utf16_pairs_across_both_separator_edges_use_scalar_identity() {
    scalar_result(r#"
var high='\ud800',low='\udc00';
var result=[high,low].join(low+high);
if(result!=='\ud800\udc00\ud800\udc00'||result.length!==4)throw 'paired separator edges';
result;
"#, "\u{10000}\u{10000}");
}

#[test]
fn join_utf16_holes_nullish_and_empty_elements_do_not_break_pairing() {
    scalar_result(r#"
var high='\ud800',low='\udc00',source=[high,,null,undefined,'',low];
var result=source.join('');
if(result!=='\ud800\udc00'||Object.hasOwn(source,1)||source.length!==6)throw 'empty contributions';
result;
"#, "\u{10000}");
}

#[test]
fn join_utf16_interrupted_and_reversed_units_remain_unpaired() {
    succeeds(r#"
var high='\ud800',low='\udc00';
var separated=[high,low].join('|');
if(separated!==high+'|'+low||separated.length!==3)throw 'interrupted pair';
var mixed=[low,high,high,low].join('');
if(mixed!==low+high+'\ud800\udc00'||mixed.charCodeAt(0)!==0xdc00||mixed.charCodeAt(1)!==0xd800)throw 'mixed raw sequence';
'ok';
"#);
}

#[test]
fn join_utf16_nested_arrays_keep_their_own_default_commas() {
    succeeds(r#"
var high='\ud800',low='\udc00',left=[high,low],right=[low,high];
if([left,right].join(';')!==high+','+low+';'+low+','+high)throw 'nested separator';
if(left.toString()!==high+','+low||Array.prototype.toString.call(right)!==low+','+high)throw 'native array toString';
'ok';
"#);
}

#[test]
fn join_utf16_nested_results_can_pair_at_the_parent_boundary() {
    scalar_result(r#"
var high='\ud800',low='\udc00',left=[high],right=[low];
var result=[left,right].join('');
if(result!=='\ud800\udc00'||left[0]!==high||right[0]!==low)throw 'nested pair';
result;
"#, "\u{10000}");
}

#[test]
fn join_utf16_native_well_formed_unit_values_and_separator_canonicalize() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("nativeAscii", Value::StringUnits(vec![0x61])).unwrap();
    runtime.define_global("nativePair", Value::StringUnits(vec![0xd800, 0xdc00])).unwrap();
    runtime.define_global("nativeSeparator", Value::StringUnits(vec![0x2d])).unwrap();
    let value = runtime.execute(r#"
if([nativeAscii].join('')!=='a'||[nativePair].join('')!=='\ud800\udc00')throw 'native canonical values';
var result=[nativeAscii,nativePair].join(nativeSeparator);
if(result!=='a-\ud800\udc00')throw 'native canonical separator';
result;
"#, &mut NoHost).unwrap();
    let Value::String(text) = value else { panic!("Well-formed native units must canonicalize") };
    assert_eq!(text, "a-\u{10000}");
    assert!(runtime.active_array_joins.is_empty());
}

#[test]
fn join_utf16_original_nullish_elements_differ_from_converted_nullish_values() {
    succeeds(r#"
var high='\ud800',calls=0;
var asNull={toString:function(){calls++;return null;}};
var asUndefined={toString:function(){calls++;return undefined;}};
if([null,undefined,asNull,asUndefined].join(high)!==high+high+'null'+high+'undefined'||calls!==2)throw 'nullish conversion';
var array=[];array[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'hint';return undefined;};
if([array].join(high)!=='undefined')throw 'array primitive undefined';
'ok';
"#);
}

#[test]
fn join_utf16_converted_undefined_separator_is_not_the_default_separator() {
    succeeds(r#"
var high='\ud800',low='\udc00',calls=0;
var separator={toString:function(){calls++;return undefined;}};
if([high,low].join(separator)!==high+'undefined'+low||calls!==1)throw 'converted undefined separator';
separator.toString=function(){calls++;return null;};
if([high,low].join(separator)!==high+'null'+low||calls!==2)throw 'converted null separator';
'ok';
"#);
}

#[test]
fn join_utf16_generic_length_separator_and_index_conversion_order_is_live() {
    succeeds(r#"
var high='\ud800',low='\udc00',order='',lengthValue={};
lengthValue[Symbol.toPrimitive]=function(hint){if(hint!=='number')throw 'length hint';order+='N';return 2.9;};
var first={toString:function(){order+='E';return high;}};
var source={get length(){order+='L';return lengthValue;},get 0(){order+='0';return first;},get 1(){order+='1';return low;}};
var separator={toString:function(){order+='S';return '';}};
if(Array.prototype.join.call(source,separator)!=='\ud800\udc00'||order!=='LNS0E1')throw 'generic order';
'ok';
"#);
}

#[test]
fn join_utf16_separator_mutation_retains_the_captured_array_length() {
    succeeds(r#"
var high='\ud800',low='\udc00',shrinking=[high,low,'tail'];
var separator={toString:function(){shrinking.length=1;return '';}};
if(shrinking.join(separator)!==high||shrinking.length!==1)throw 'shrinking length';
var growing=[high];separator.toString=function(){growing.push(low);return '';};
if(growing.join(separator)!==high||growing.length!==2||growing[1]!==low)throw 'growing length';
'ok';
"#);
}

#[test]
fn join_utf16_element_conversion_observes_later_mutations_without_recapturing_length() {
    succeeds(r#"
var high='\ud800',low='\udc00',calls=0,source;
source=[{toString:function(){calls++;delete source[1];source[2]=low;source.push(high);return high;}},'removed','replaced'];
if(source.join('')!=='\ud800\udc00'||calls!==1||source.length!==4||Object.hasOwn(source,1)||source[3]!==high)throw 'live values';
'ok';
"#);
}

#[test]
fn join_utf16_inherited_indices_and_raw_metadata_preserve_real_holes() {
    succeeds(r#"
var high='\ud800',low='\udc00',reads='',parent=Object.create(Array.prototype),source=Array(3);
Object.defineProperty(parent,'0',{get:function(){if(this!==source)throw 'first receiver';reads+='H';return high;}});
Object.defineProperty(parent,'2',{get:function(){if(this!==source)throw 'last receiver';reads+='L';return low;}});
source[high]='metadata';Object.setPrototypeOf(source,parent);
if(source.join('')!=='\ud800\udc00'||reads!=='HL'||Object.hasOwn(source,0)||Object.hasOwn(source,1)||Object.hasOwn(source,2))throw 'inherited values';
if(source[high]!=='metadata')throw 'raw metadata';
'ok';
"#);
}

#[test]
fn join_utf16_proxy_receiver_uses_only_ordered_get_with_the_original_receiver() {
    succeeds(r#"
var high='\ud800',low='\udc00',calls='',target={length:2,0:high,1:'old'},proxy;
proxy=new Proxy(target,{get:function(t,key,receiver){
    if(receiver!==proxy)throw 'proxy receiver';calls+=key+'|';
    if(key==='0')t[1]=low;return t[key];
},has:function(){throw 'unexpected has';},ownKeys:function(){throw 'unexpected ownKeys';}});
if(Array.prototype.join.call(proxy,'')!=='\ud800\udc00'||calls!=='length|0|1|')throw 'proxy gets';
'ok';
"#);
}

#[test]
fn join_utf16_proxy_prototype_cannot_use_the_hole_only_shortcut() {
    succeeds(r#"
var high='\ud800',low='\udc00',source=Array(2),calls='';
var parent=new Proxy({},{get:function(target,key,receiver){
    if(receiver!==source)throw 'inherited proxy receiver';calls+=key;return key==='0'?high:low;
}});
Object.setPrototypeOf(source,parent);
if(Array.prototype.join.call(source,'')!=='\ud800\udc00'||calls!=='01')throw 'synthetic indices';
'ok';
"#);
}

#[test]
fn join_utf16_primitive_hooks_and_lazy_ordinary_fallback_preserve_units() {
    succeeds(r#"
var high='\ud800',low='\udc00',order='',first={},second={};
Object.defineProperty(first,Symbol.toPrimitive,{get:function(){order+='G';return function(hint){if(this!==first||hint!=='string')throw 'exotic hook';order+='C';return high;};}});
Object.defineProperty(first,'toString',{get:function(){throw 'unused ordinary hook';}});
Object.defineProperty(second,'toString',{get:function(){order+='T';return function(){order+='S';return {};};}});
Object.defineProperty(second,'valueOf',{get:function(){order+='V';return function(){if(this!==second)throw 'fallback receiver';order+='F';return low;};}});
if([first,second].join('')!=='\ud800\udc00'||order!=='GCTSVF')throw 'hook order';
'ok';
"#);
}

#[test]
fn join_utf16_callable_and_scalar_box_conversion_hooks_preserve_units() {
    succeeds(r#"
var high='\ud800',low='\udc00',calls='';
function callable(){};callable.toString=function(){if(this!==callable)throw 'callable receiver';calls+='F';return high;};
var bound=callable.bind(null);bound.toString=function(){if(this!==bound)throw 'bound receiver';calls+='B';return low;};
var number=Object(7);number.toString=function(){calls+='N';return high;};
var string=Object('scalar');string[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'box hint';calls+='S';return low;};
if([callable,bound,number,string].join('')!=='\ud800\udc00\ud800\udc00'||calls!=='FBNS')throw 'callable or box';
'ok';
"#);
}

#[test]
fn join_utf16_array_to_string_returns_custom_join_results_unchanged() {
    succeeds(r#"
var high='\ud800',low='\udc00',source=[high],marker={},calls=0,result=marker;
source.join=function(){if(this!==source||arguments.length!==0)throw 'custom join receiver';calls++;return result;};
if(source.toString()!==marker)throw 'object result';
result=high;if(source.toString()!==high)throw 'raw result';
result=7;if(Array.prototype.toString.call(source)!==7||calls!==3)throw 'numeric result';
source.join=0;if(source.toString()!=='[object Array]')throw 'fallback';
var generic=Object.create({join:function(){if(this!==generic)throw 'generic receiver';return low;}});
if(Array.prototype.toString.call(generic)!==low)throw 'generic inherited join';
'ok';
"#);
}

#[test]
fn join_utf16_nested_native_array_shortcuts_keep_lookup_and_fallback_order() {
    succeeds(r#"
var high='\ud800',low='\udc00',child=[high],order='';
Object.defineProperty(child,'toString',{get:function(){order+='T';return Array.prototype.toString;},configurable:true});
Object.defineProperty(child,'join',{get:function(){order+='J';return function(){order+='C';return {};};},configurable:true});
Object.defineProperty(child,'valueOf',{get:function(){order+='V';return function(){order+='F';return low;};},configurable:true});
if([child].join('')!==low||order!=='TJCVF')throw 'nested fallback';
Object.defineProperty(child,'join',{value:7});order='';
if([child].join('')!=='[object Array]'||order!=='T')throw 'noncallable join';
Object.defineProperty(child,'toString',{value:function(){return high;}});
Object.defineProperty(child,'join',{get:function(){throw 'unused join';}});
if([child].join('')!==high)throw 'custom toString';
'ok';
"#);
}

#[test]
fn join_utf16_cycles_and_repeated_child_arrays_keep_exact_results() {
    succeeds(r#"
var high='\ud800',low='\udc00',source=[];source[0]=source;source[1]=high;
if(source.join('')!==high||[source,source].join('')!==','+high+','+high)throw 'self or repeated cycle';
var left=[],right=[];left[0]=right;left[1]=high;right[0]=left;right[1]=low;
if(left.join('')!==','+low+high||right.join('')!==','+high+low)throw 'mutual cycle';
if(source.join('')!==high)throw 'cycle cleanup';
'ok';
"#);
}

#[test]
fn join_utf16_reentrant_joins_keep_the_active_identity_until_conversion_returns() {
    succeeds(r#"
var high='\ud800',low='\udc00',source=[],calls=0;
source[0]={toString:function(){calls++;return source.join('')+low;}};source[1]=high;
if(source.join('')!==low+high||source.join('')!==low+high||calls!==2)throw 'same-array reentry';
var separate=[high,low],nested={toString:function(){return separate.join('')+high;}};
if([nested,low].join('')!=='\ud800\udc00\ud800\udc00')throw 'independent nested join';
'ok';
"#);
}

#[test]
fn join_utf16_abrupt_conversions_keep_thrown_identity_and_prior_effects() {
    succeeds(r#"
var high='\ud800',low='\udc00',marker={},firstCalls=0,secondCalls=0,later=0,source;
source=[{toString:function(){firstCalls++;return high;}},{toString:function(){secondCalls++;source.saved=low;throw marker;}}];
Object.defineProperty(source,'2',{get:function(){later++;return '';},configurable:true});
var caught=false;try{source.join('');}catch(error){caught=error===marker;}
if(!caught||firstCalls!==1||secondCalls!==1||later!==0||source.saved!==low)throw 'abrupt conversion';
source[1]=low;if(source.join('')!=='\ud800\udc00'||firstCalls!==2||later!==1)throw 'recovery';
var reads=0,empty=Array(1);Object.defineProperty(empty,'0',{get:function(){reads++;return high;}});
caught=false;try{empty.join({toString:function(){throw marker;}});}catch(error){caught=error===marker;}
if(!caught||reads!==0)throw 'separator abrupt order';
'ok';
"#);
}

#[test]
fn join_utf16_symbol_and_nonprimitive_conversion_failures_stop_later_reads() {
    succeeds(r#"
var high='\ud800',symbol=Symbol('element'),badSymbol={},badObject={},later=0,caught=0;
badSymbol[Symbol.toPrimitive]=function(){return symbol;};
badObject.toString=function(){return {};};badObject.valueOf=function(){return {};};
for(var value of [symbol,badSymbol,badObject]){
    var source=[value];Object.defineProperty(source,'1',{get:function(){later++;return high;}});
    try{source.join(high);}catch(error){if(error.name==='TypeError')caught++;else throw error;}
}
try{[].join(symbol);}catch(error){if(error.name==='TypeError')caught++;else throw error;}
if(caught!==4||later!==0||[high].join('')!==high)throw 'conversion refusal';
'ok';
"#);
}

struct NativeConversion {
    allow: bool,
    reads: usize,
    calls: usize,
}

impl Host for NativeConversion {
    fn primitive_conversion(&self, _: u32) -> bool { self.allow }

    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        self.reads += 1;
        Ok(Value::Undefined)
    }

    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("Unexpected native write.".into())
    }

    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        self.calls += 1;
        Err("Unexpected native call.".into())
    }
}

fn native_source(allow: bool) -> (Runtime, NativeConversion) {
    let mut runtime = Runtime::for_test();
    let mut host = NativeConversion { allow, reads: 0, calls: 0 };
    runtime.define_global("native", Value::Host(17)).unwrap();
    runtime.bind_host_object(17, &[]).unwrap();
    runtime.execute(r#"
var hookReads=0,hookCalls=0,caught=false,finished=false;
Object.defineProperty(native,Symbol.toPrimitive,{get:function(){
    hookReads++;return function(hint){
        if(this!==native||hint!=='string')throw 'native conversion receiver';
        hookCalls++;return '\ud800';
    };
},configurable:true});
"#, &mut host).unwrap();
    runtime.begin_job();
    (runtime, host)
}

#[test]
fn join_utf16_actual_array_native_elements_remain_denied_before_hooks() {
    let (mut runtime, mut host) = native_source(true);
    let error = runtime.execute(r#"
try{['\ud800',native].join('');}catch(error){caught=true;}finally{finished=true;}
"#, &mut host).unwrap_err();
    assert!(error.contains("cannot implicitly convert host objects"), "{error}");
    assert_eq!((host.reads, host.calls), (0, 0));
    assert!(runtime.active_array_joins.is_empty());
    assert_eq!(runtime.call_depth, 0);
    runtime.begin_job();
    assert_eq!(runtime.execute(r#"
!caught&&!finished&&hookReads===0&&hookCalls===0&&['\ud800','\udc00'].join('')==='\ud800\udc00';
"#, &mut host).unwrap().to_text(), "true");
}

#[test]
fn join_utf16_generic_native_conversion_still_requires_explicit_opt_in() {
    let (mut runtime, mut host) = native_source(false);
    let error = runtime.execute(r#"
try{Array.prototype.join.call({0:native,1:'\udc00',length:2},'');}catch(error){caught=true;}finally{finished=true;}
"#, &mut host).unwrap_err();
    assert!(error.contains("No conversion permission or hook read was granted"), "{error}");
    assert_eq!((host.reads, host.calls), (0, 0));
    runtime.begin_job();
    assert_eq!(runtime.execute("!caught&&!finished&&hookReads===0&&hookCalls===0;", &mut host).unwrap().to_text(), "true");
    host.allow = true;
    runtime.begin_job();
    let value = runtime.execute(r#"
var result=Array.prototype.join.call({0:native,1:'\udc00',length:2},'');
if(result!=='\ud800\udc00'||hookReads!==1||hookCalls!==1)throw 'opted-in raw conversion';
result;
"#, &mut host).unwrap();
    let Value::String(text) = value else { panic!("A repaired native conversion result must canonicalize") };
    assert_eq!(text, "\u{10000}");
    assert_eq!((host.reads, host.calls), (0, 0));
    assert!(runtime.active_array_joins.is_empty());
    assert_eq!(runtime.call_depth, 0);
}
