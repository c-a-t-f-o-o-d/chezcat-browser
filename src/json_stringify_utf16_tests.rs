//! Exact indentation and boxed-string conversion behavior for JSON serialization.
use super::*;

struct NoHost;

impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("JSON serialization must not read a native host");
    }

    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("JSON serialization must not write a native host");
    }

    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("JSON serialization must not call a native host");
    }
}

fn assert_idle(runtime: &Runtime) {
    assert_eq!(runtime.json_scratch_bytes, 0);
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.eval_depth, 0);
    assert!(runtime.transient_registration_is_empty());
}

fn run(source: &str) -> Value {
    let mut runtime = Runtime::for_test();
    let value = runtime.execute(source, &mut NoHost).unwrap();
    assert_idle(&runtime);
    value
}

fn succeeds(source: &str) {
    assert_eq!(run(source).to_text(), "ok");
}

#[test]
fn stringify_utf16_raw_gaps_preserve_exact_units_and_scalar_lookalikes() {
    succeeds(r#"
var high='\ud800',low='\udfff',replacement='\ufffd',spelling='\\ud800';
for(var gap of [high,low,replacement,spelling]){
    var text=JSON.stringify({a:1},null,gap);
    if(text!=='{\n'+gap+'"a": 1\n}')throw 'literal gap identity';
}
var highText=JSON.stringify({a:1},null,high);
if(highText.charCodeAt(2)!==0xd800||highText===JSON.stringify({a:1},null,replacement)||
    highText===JSON.stringify({a:1},null,spelling))throw 'raw gap aliases';
'ok';
"#);
    let Value::StringUnits(units) = run(r#"JSON.stringify({a:1},null,'\udfff');"#) else {
        panic!("A used unpaired gap must remain an exact raw string");
    };
    let mut expected: Vec<u16> = "{\n".encode_utf16().collect();
    expected.push(0xdfff);
    expected.extend("\"a\": 1\n}".encode_utf16());
    assert_eq!(units, expected);
}

#[test]
fn stringify_utf16_gap_truncation_counts_units_and_keeps_a_split_pair() {
    succeeds(r#"
var pair='\ud83d\ude00';
if(JSON.stringify({a:1},null,pair+pair+pair+pair+pair+'ignored')!==
    '{\n'+pair+pair+pair+pair+pair+'"a": 1\n}')throw 'supplementary unit count';
var split='123456789'+pair+'ignored';
if(JSON.stringify({a:1},null,split)!=='{\n123456789\ud83d"a": 1\n}')throw 'split tenth unit';
var mixed=pair+pair+pair+pair+'a'+pair;
if(JSON.stringify([1],null,mixed)!=='[\n'+pair+pair+pair+pair+'a\ud83d1\n]')throw 'mixed split';
'ok';
"#);
}

#[test]
fn stringify_utf16_discarded_raw_suffix_does_not_taint_scalar_output() {
    let Value::String(text) = run(r#"
var result=JSON.stringify({a:1},null,'1234567890\ud800\udfff');
if(result!=='{\n1234567890"a": 1\n}')throw 'discarded raw suffix';
result;
"#) else { panic!("An unused raw suffix must not select raw output storage") };
    assert_eq!(text, "{\n1234567890\"a\": 1\n}");
}

#[test]
fn stringify_utf16_native_well_formed_gap_units_use_scalar_output() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("nativeGap", Value::StringUnits(vec![0xd83d, 0xde00])).unwrap();
    let value = runtime.execute(r#"
var result=JSON.stringify({a:1},null,nativeGap);
if(result!=='{\n😀"a": 1\n}')throw 'native pair gap';
result;
"#, &mut NoHost).unwrap();
    let Value::String(text) = value else { panic!("A well-formed result must use scalar storage") };
    assert_eq!(text, "{\n😀\"a\": 1\n}");
    assert_idle(&runtime);
}

#[test]
fn stringify_utf16_unused_gap_leaves_primitive_empty_and_omitted_roots_unchanged() {
    succeeds(r#"
var gap='\ud800';
if(JSON.stringify(1,null,gap)!=='1'||JSON.stringify(null,null,gap)!=='null'||
    JSON.stringify(true,null,gap)!=='true'||JSON.stringify(gap,null,gap)!=='"\\ud800"')throw 'primitive root';
if(JSON.stringify({},null,gap)!=='{}'||JSON.stringify([],null,gap)!=='[]'||
    JSON.stringify({a:undefined,b:function(){},c:Symbol()},null,gap)!=='{}')throw 'empty root';
if(JSON.stringify(undefined,null,gap)!==undefined||JSON.stringify(function(){},null,gap)!==undefined||
    JSON.stringify(Symbol(),null,gap)!==undefined||JSON.stringify({a:1},function(){return undefined;},gap)!==undefined)
    throw 'omitted root';
var box=Object('unused'),calls=0;box.toString=function(){calls++;return gap;};
if(JSON.stringify(undefined,null,box)!==undefined||JSON.stringify({},null,box)!=='{}'||
    JSON.stringify([],null,box)!=='[]'||JSON.stringify(1,null,box)!=='1'||calls!==4)throw 'unused boxed gap conversion';
'ok';
"#);
    for source in [
        r#"JSON.stringify({},null,'\ud800');"#,
        r#"JSON.stringify('raw\udfff',null,'\ud800');"#,
    ] {
        assert!(matches!(run(source), Value::String(_)), "Unused gap must keep scalar storage: {source}");
    }
}

#[test]
fn stringify_utf16_nested_arrays_and_objects_restore_the_parent_indentation() {
    succeeds(r#"
var gap='\udfff\ud800';
var value={a:[1,{b:2},[],{}],z:3};
var expected='{\n'+gap+'"a": [\n'+gap+gap+'1,\n'+gap+gap+'{\n'+gap+gap+gap+'"b": 2\n'+
    gap+gap+'},\n'+gap+gap+'[],\n'+gap+gap+'{}\n'+gap+'],\n'+gap+'"z": 3\n}';
if(JSON.stringify(value,null,gap)!==expected)throw 'nested indentation restoration';
if(JSON.stringify([undefined,null,Symbol(),function(){}],null,gap)!==
    '[\n'+gap+'null,\n'+gap+'null,\n'+gap+'null,\n'+gap+'null\n]')throw 'array omissions';
'ok';
"#);
}

#[test]
fn stringify_utf16_escapes_names_and_values_while_emitting_gap_units_literally() {
    succeeds(r#"
var high='\ud800',low='\udfff',value={};value[high]=low;value['literal\\ud800']=high;
var result=JSON.stringify(value,null,high);
if(result!=='{\n'+high+'"\\ud800": "\\udfff",\n'+high+'"literal\\\\ud800": "\\ud800"\n}')
    throw 'escaped names and values';
var gap='"\\'+low;
if(JSON.stringify({a:1},null,gap)!=='{\n'+gap+'"a": 1\n}')throw 'literal slash and quote gap';
'ok';
"#);
}

#[test]
fn stringify_utf16_gap_keeps_null_units_and_the_following_unpaired_units() {
    succeeds(r#"
var gap='"\\\u0000\udfff';
var result=JSON.stringify({a:1},null,gap);
if(result!=='{\n'+gap+'"a": 1\n}'||result.charCodeAt(4)!==0||result.charCodeAt(5)!==0xdfff)
    throw 'literal null and following gap units';
'ok';
"#);
}

#[test]
fn stringify_utf16_boxed_gap_converts_once_after_the_property_list() {
    succeeds(r#"
var high='\ud800',order='',list=[],key=Object('unused'),gap=Object('unused'),reads=0,calls=0;
key.toString=function(){order+='K';return 'a';};
Object.defineProperty(list,'0',{get:function(){order+='L';return key;}});
Object.defineProperty(gap,Symbol.toPrimitive,{get:function(){
    order+='G';reads++;return function(hint){if(this!==gap||hint!=='string')throw 'gap hint';order+='S';calls++;return high;};
}});
var root={toJSON:function(key){if(this!==root||key!=='')throw 'root hook';order+='T';
    return {get a(){order+='V';return 1;}};
}};
if(JSON.stringify(root,list,gap)!=='{\n'+high+'"a": 1\n}'||order!=='LKGSTV'||reads!==1||calls!==1)
    throw 'list gap and serialization order';
'ok';
"#);
}

#[test]
fn stringify_utf16_boxed_gap_ordinary_conversion_has_string_hint_fallback_order() {
    succeeds(r#"
var low='\udfff',order='',gap=Object('unused');
Object.defineProperty(gap,'toString',{get:function(){order+='T';return function(){
    if(this!==gap)throw 'toString receiver';order+='S';return {};
};}});
Object.defineProperty(gap,'valueOf',{get:function(){order+='V';return function(){
    if(this!==gap)throw 'valueOf receiver';order+='F';return low;
};}});
if(JSON.stringify({a:1},null,gap)!=='{\n'+low+'"a": 1\n}'||order!=='TSVF')throw 'ordinary gap conversion';
'ok';
"#);
}

#[test]
fn stringify_utf16_boxed_value_converts_after_to_json_and_replacer_once() {
    let Value::String(text) = run(r#"
var value=Object('unused'),order='',reads=0,calls=0;
Object.defineProperty(value,'toJSON',{get:function(){order+='G';return function(key){
    if(this!==value||key!=='')throw 'boxed toJSON receiver';order+='J';return value;
};}});
Object.defineProperty(value,Symbol.toPrimitive,{get:function(){order+='P';reads++;return function(hint){
    if(this!==value||hint!=='string')throw 'boxed string hint';order+='S';calls++;return '\udfff';
};}});
Object.defineProperty(value,'ignored',{enumerable:true,get:function(){throw 'boxed properties are not serialized';}});
var result=JSON.stringify(value,function(key,current){
    if(key!==''||current!==value||this['']!==value)throw 'boxed replacer';order+='R';return current;
},'\ud800');
if(result!=='"\\udfff"'||order!=='GJRPS'||reads!==1||calls!==1)throw 'boxed value conversion order';
result;
"#) else { panic!("Escaped boxed raw text without indentation must remain scalar") };
    assert_eq!(text, "\"\\udfff\"");
}

#[test]
fn stringify_utf16_replacer_returned_box_unboxes_without_a_second_to_json() {
    succeeds(r#"
var high='\ud800',low='\udfff',box=Object('unused'),calls=0,keys='';
box.toJSON=function(){throw 'second toJSON call';};
box.toString=function(){if(this!==box)throw 'replacement box receiver';calls++;return low;};
var result=JSON.stringify({a:1},function(key,value){keys+=key+'|';return key==='a'?box:value;},high);
if(result!=='{\n'+high+'"a": "\\udfff"\n}'||calls!==1||keys!=='|a|')throw 'replacement box conversion';
'ok';
"#);
}

#[test]
fn stringify_utf16_box_conversion_keeps_to_string_rules_and_ignores_nonbox_gap_hooks() {
    succeeds(r#"
var ignored={toString:function(){throw 'ordinary gap conversion';}};
ignored[Symbol.toPrimitive]=function(){throw 'ordinary gap exotic conversion';};
if(JSON.stringify({a:1},null,ignored)!=='{"a":1}')throw 'ordinary gap ignored';
var gap=Object('unused'),value=Object('unused');
gap[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'gap hint';return undefined;};
value[Symbol.toPrimitive]=function(hint){if(hint!=='string')throw 'value hint';return null;};
if(JSON.stringify({a:1},null,gap)!=='{\nundefined"a": 1\n}'||JSON.stringify(value)!=='"null"')throw 'converted primitive spellings';
var bad=Object('unused'),caught=0;bad[Symbol.toPrimitive]=function(){return Symbol();};
try{JSON.stringify({a:1},null,bad);}catch(error){if(error.name==='TypeError')caught++;else throw error;}
try{JSON.stringify(bad);}catch(error){if(error.name==='TypeError')caught++;else throw error;}
bad[Symbol.toPrimitive]=function(){return {};};
try{JSON.stringify(bad);}catch(error){if(error.name==='TypeError')caught++;else throw error;}
if(caught!==3)throw 'conversion TypeErrors';
'ok';
"#);
}

#[test]
fn stringify_utf16_recursive_calls_keep_outer_and_inner_gaps_independent() {
    succeeds(r#"
var high='\ud800',low='\udfff',gap=Object('unused'),converted=0,innerGap,innerValue;
gap.toString=function(){converted++;innerGap=JSON.stringify({b:2},null,low);return high;};
var result=JSON.stringify({a:1,b:2},function(key,value){
    if(key==='a')innerValue=JSON.stringify([3],null,low);return value;
},gap);
if(result!=='{\n'+high+'"a": 1,\n'+high+'"b": 2\n}'||converted!==1||
    innerGap!=='{\n'+low+'"b": 2\n}'||innerValue!=='[\n'+low+'3\n]')throw 'reentrant indentation';
'ok';
"#);
}

#[test]
fn stringify_utf16_abrupt_gap_conversion_preserves_mutation_and_stops_value_hooks() {
    succeeds(r#"
var high='\ud800',low='\udfff',saved={},marker={},gap=Object('unused'),list=[],listReads=0,valueCalls=0,caught=false;
Object.defineProperty(list,'0',{get:function(){listReads++;return 'a';}});
gap.toString=function(){saved[high]=low;throw marker;};
var root={toJSON:function(){valueCalls++;return {a:1};}};
try{JSON.stringify(root,list,gap);}catch(error){caught=error===marker;}
if(!caught||listReads!==1||valueCalls!==0||saved[high]!==low)throw 'gap abrupt effects';
if(JSON.stringify(root,['a'],high)!=='{\n'+high+'"a": 1\n}'||valueCalls!==1||saved[high]!==low)throw 'gap recovery';
'ok';
"#);
}

#[test]
fn stringify_utf16_abrupt_box_conversion_preserves_prior_writes_and_stops_later_gets() {
    succeeds(r#"
var high='\ud800',low='\udfff',saved={},marker={},first=Object('first'),second=Object('second'),calls='',later=0,caught=false;
first.toString=function(){calls+='F';saved.first=high;return high;};
second.toString=function(){calls+='S';saved[high]=low;throw marker;};
var root={first:first,second:second};Object.defineProperty(root,'later',{enumerable:true,get:function(){later++;return 3;}});
try{JSON.stringify(root,null,low);}catch(error){caught=error===marker;}
if(!caught||calls!=='FS'||later!==0||saved.first!==high||saved[high]!==low)throw 'boxed abrupt effects';
root.second='ok';
if(JSON.stringify(root,null,low)!=='{\n'+low+'"first": "\\ud800",\n'+low+'"second": "ok",\n'+low+'"later": 3\n}'||
    calls!=='FSF'||later!==1||saved[high]!==low)throw 'boxed conversion recovery';
'ok';
"#);
}
