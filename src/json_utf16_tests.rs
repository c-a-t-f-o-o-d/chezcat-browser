//! Original JSON UTF-16 value, property-name, and reviver regressions.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("JSON parsing must not read a native host");
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("JSON parsing must not write a native host");
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("JSON parsing must not call a native host");
    }
}

fn units(value: Value) -> Vec<u16> {
    match value {
        Value::String(text) => text.encode_utf16().collect(),
        Value::StringUnits(units) => units,
        other => panic!("Expected a JSON string, got {other:?}"),
    }
}

fn parsed_units(source: &str) -> Vec<u16> {
    units(Runtime::for_test().json_parse_builtin(
        &[Value::String(source.into())], &mut NoHost,
    ).unwrap())
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    assert_eq!(runtime.json_scratch_bytes, 0);
    assert!(runtime.transient_registration_is_empty());
}

fn raw_source_succeeds(source: Vec<u16>, script: &str) {
    let mut runtime = Runtime::for_test();
    runtime.define_global("rawJson", Value::StringUnits(source)).unwrap();
    assert_eq!(runtime.execute(script, &mut NoHost).unwrap().to_text(), "ok");
    assert_eq!(runtime.json_scratch_bytes, 0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn json_utf16_parse_preserves_each_lone_surrogate_boundary() {
    for (source, expected) in [
        (r#""\ud800""#, 0xd800),
        (r#""\uDBFF""#, 0xdbff),
        (r#""\udc00""#, 0xdc00),
        (r#""\uDFFF""#, 0xdfff),
    ] {
        assert_eq!(parsed_units(source), vec![expected], "{source}");
    }
}

#[test]
fn json_utf16_parse_keeps_valid_pairs_among_unpaired_units() {
    assert_eq!(parsed_units(r#""a\ud800\udc00\ud800b\udfff\udbff\udfff""#),
        vec![0x61, 0xd800, 0xdc00, 0xd800, 0x62, 0xdfff, 0xdbff, 0xdfff]);
    assert_eq!(parsed_units(r#""\ud800\ud800\udc00\udc00""#),
        vec![0xd800, 0xd800, 0xdc00, 0xdc00]);
}

#[test]
fn json_utf16_interrupted_pairs_do_not_consume_the_following_escape() {
    let cases: &[(&str, &[u16])] = &[
        (r#""\ud800\u0041""#, &[0xd800, 0x41]),
        (r#""\ud800\n""#, &[0xd800, 0x0a]),
        (r#""\ud800\\u0041""#, &[0xd800, 0x5c, 0x75, 0x30, 0x30, 0x34, 0x31]),
        (r#""\ud800\"""#, &[0xd800, 0x22]),
        (r#""\ud800\/""#, &[0xd800, 0x2f]),
        (r#""\ud800\uD800\uDC00""#, &[0xd800, 0xd800, 0xdc00]),
    ];
    for (source, expected) in cases {
        assert_eq!(parsed_units(source), *expected, "{source}");
    }
}

#[test]
fn json_utf16_controls_non_ascii_and_surrogates_share_one_string() {
    assert_eq!(parsed_units(r#""A\u0000\u001f\b\f\n\r\t\/\"\\é中\ud83d\ude00\ud800""#),
        vec![0x41, 0, 0x1f, 8, 12, 10, 13, 9, 0x2f, 0x22, 0x5c,
            0xe9, 0x4e2d, 0xd83d, 0xde00, 0xd800]);
}

#[test]
fn json_utf16_stringify_parse_roundtrips_exact_string_units() {
    let mut runtime = Runtime::for_test();
    for source in [
        r#""\ud800""#, r#""\udfff""#,
        r#""\ud800\udc00\ud800a\udfff""#,
        r#""\u0000é\ud800\n\\ud800""#,
    ] {
        let original = runtime.json_parse_builtin(&[Value::String(source.into())], &mut NoHost).unwrap();
        let expected = units(original.clone());
        let serialized = runtime.json_stringify(&[original], &mut NoHost).unwrap();
        let parsed = runtime.json_parse_builtin(&[serialized], &mut NoHost).unwrap();
        assert_eq!(units(parsed), expected, "{source}");
    }
    assert_eq!(runtime.json_scratch_bytes, 0);
}

#[test]
fn json_utf16_keys_distinguish_surrogates_replacement_and_literal_backslash() {
    succeeds(r#"
var high='\ud800',low='\udfff',replacement='\ufffd',literal='\\ud800';
var value=JSON.parse('{"\\ud800":1,"\\udfff":2,"�":3,"\\\\ud800":4,"x\\ud800":5}');
var keys=Object.keys(value);
if(keys.length!==5||keys[0]!==high||keys[1]!==low||keys[2]!==replacement||keys[3]!==literal||keys[4]!=='x'+high)throw 'exact key order';
if(value[high]!==1||value[low]!==2||value[replacement]!==3||value[literal]!==4||value['x'+high]!==5)throw 'key alias';
if(Reflect.get(value,high)!==1||!Object.hasOwn(value,low)||Object.getOwnPropertyDescriptor(value,high).value!==1)throw 'ordinary key access';
if(JSON.stringify(value)!=='{"\\ud800":1,"\\udfff":2,"�":3,"\\\\ud800":4,"x\\ud800":5}')throw 'key serialization';
var round=JSON.parse(JSON.stringify(value)),roundKeys=Object.keys(round);
if(roundKeys.length!==keys.length)throw 'roundtrip key count';
for(var i=0;i<keys.length;i++){if(roundKeys[i]!==keys[i]||round[keys[i]]!==value[keys[i]])throw 'key roundtrip';}
'ok';
"#);
}

#[test]
fn json_utf16_duplicate_raw_keys_replace_value_at_the_first_position() {
    succeeds(r#"
var high='\ud800',low='\udc00';
var value=JSON.parse('{"\\uD800":"first","middle":2,"\\ud800":"last","\\uDC00":3,"\\udc00":4}');
var keys=Object.keys(value);
if(keys.length!==3||keys[0]!==high||keys[1]!=='middle'||keys[2]!==low)throw 'duplicate order';
if(value[high]!=='last'||value[low]!==4||JSON.stringify(value)!=='{"\\ud800":"last","middle":2,"\\udc00":4}')throw 'duplicate values';
'ok';
"#);
}

#[test]
fn json_utf16_well_formed_literal_and_escaped_keys_share_canonical_identity() {
    succeeds(r#"
var value=JSON.parse('{"a":1,"\\u0061":2,"😀":3,"\\uD83D\\uDE00":4,"é":5,"\\u00e9":6}');
var keys=Object.keys(value);
if(keys.length!==3||keys[0]!=='a'||keys[1]!=='😀'||keys[2]!=='é')throw 'canonical key order';
if(value.a!==2||value['😀']!==4||value['é']!==6||JSON.stringify(value)!=='{"a":2,"😀":4,"é":6}')throw 'canonical duplicates';
'ok';
"#);
}

#[test]
fn json_utf16_parse_creates_own_data_without_prototype_or_setter_effects() {
    succeeds(r#"
var high='\ud800',writes=0,value;
Object.defineProperty(Object.prototype,high,{set:function(){writes++;},configurable:true});
Object.defineProperty(Object.prototype,'jsonSetterProbe',{set:function(){writes++;},configurable:true});
try{value=JSON.parse('{"__proto__":{"jsonPolluted":true},"\\ud800":7,"jsonSetterProbe":8}');}
finally{delete Object.prototype[high];delete Object.prototype.jsonSetterProbe;}
if(writes!==0||Object.getPrototypeOf(value)!==Object.prototype||Object.hasOwn(Object.prototype,'jsonPolluted'))throw 'prototype side effect';
if(!Object.hasOwn(value,'__proto__')||value.__proto__.jsonPolluted!==true||value[high]!==7||value.jsonSetterProbe!==8)throw 'own data';
for(var key of ['__proto__',high,'jsonSetterProbe']){var d=Object.getOwnPropertyDescriptor(value,key);if(!d.writable||!d.enumerable||!d.configurable||'get' in d||'set' in d)throw 'data descriptor';}
'ok';
"#);
}

#[test]
fn json_utf16_reviver_has_exact_keys_real_holders_and_postorder() {
    succeeds(r#"
var high='\ud800',low='\udfff',keys=[],holders=[],contexts=[],rootHolder;
var result=JSON.parse('{"outer":{"\\ud800":1,"\\udfff":2},"\\ud800":3}',function(key,value,context){
    if(arguments.length!==3||this[key]!==value||Object.getPrototypeOf(context)!==Object.prototype)throw 'reviver protocol';
    keys.push(key);holders.push(this);contexts.push(context);if(key==='')rootHolder=this;return value;
});
if(keys.length!==5||keys[0]!==high||keys[1]!==low||keys[2]!=='outer'||keys[3]!==high||keys[4]!=='')throw 'postorder keys';
if(holders[0]!==result.outer||holders[1]!==result.outer||holders[2]!==result||holders[3]!==result||rootHolder['']!==result)throw 'holders';
if(contexts[0].source!=='1'||contexts[1].source!=='2'||contexts[3].source!=='3'||Object.hasOwn(contexts[2],'source')||Object.hasOwn(contexts[4],'source'))throw 'contexts';
'ok';
"#);
}

#[test]
fn json_utf16_reviver_deletion_and_root_replacement_preserve_exact_keys() {
    succeeds(r#"
var high='\ud800',low='\udfff',visits=0;
var value=JSON.parse('{"\\ud800":10,"\\udfff":20}',function(key,value){
    if(key===high){visits++;return undefined;}if(key===low){visits++;return value+1;}return value;
});
if(visits!==2||Object.hasOwn(value,high)||high in value||value[low]!==21||JSON.stringify(value)!=='{"\\udfff":21}')throw 'exact deletion';
var roots=0;
if(JSON.parse('"\\ud800"',function(key,value){roots++;if(key!==''||value!==high||this['']!==value)throw 'root holder';return undefined;})!==undefined||roots!==1)throw 'root deletion';
if(JSON.parse('{"\\udfff":1}',function(key,value){return key===''?low:value;})!==low)throw 'root replacement';
'ok';
"#);
}

#[test]
fn json_utf16_reviver_ignores_false_descriptor_definitions_and_deletions() {
    succeeds(r#"
var high='\ud800',low='\udfff',removed='x\ud800';
var value=JSON.parse('{"\\ud800":1,"\\udfff":2,"x\\ud800":3}',function(key,value){
    if(key===high){Object.defineProperty(this,key,{value:7,configurable:false});return 9;}
    if(key===low){Object.defineProperty(this,key,{value:8,configurable:false});return undefined;}
    if(key===removed){delete this[key];Object.preventExtensions(this);return 10;}
    return value;
});
if(value[high]!==7||value[low]!==8||Object.hasOwn(value,removed)||Object.isExtensible(value))throw 'definition rejection';
if(Object.getOwnPropertyDescriptor(value,high).configurable||Object.getOwnPropertyDescriptor(value,low).configurable)throw 'descriptor changed';
'ok';
"#);
}

#[test]
fn json_utf16_reviver_source_uses_exact_last_duplicate_token() {
    succeeds(r#"
var high='\ud800',low='\udfff',seen=[],contexts=[];
var result=JSON.parse('{"\\ud800":"\\udfff","tail":true,"\\uD800":"\\uDFFF"}',function(key,value,context){
    if(key===high){if(value!==low||context.source!=='"\\uDFFF"')throw 'last source token';seen.push(key);contexts.push(context);}
    if(key==='tail'){if(context.source!=='true')throw 'tail source';seen.push(key);contexts.push(context);}
    if(key===''&&Object.hasOwn(context,'source'))throw 'object source';return value;
});
if(seen.length!==2||seen[0]!==high||seen[1]!=='tail'||result[high]!==low||contexts[0]===contexts[1])throw 'duplicate visit';
var d=Object.getOwnPropertyDescriptor(contexts[0],'source');
if(!d.writable||!d.enumerable||!d.configurable||d.value!=='"\\uDFFF"')throw 'source descriptor';
'ok';
"#);
}

#[test]
fn json_utf16_reviver_source_preserves_escape_spelling_and_token_ranges() {
    succeeds(r#"
var expected=['"\\ud800"','"\\uDFFF"','"\\ud800\\n"','"é\\ud800"','"\\ud83d\\ude00"'],seen=0;
var result=JSON.parse(' \r\n["\\ud800","\\uDFFF","\\ud800\\n","é\\ud800","\\ud83d\\ude00"] \t',function(key,value,context){
    if(key!==''){if(context.source!==expected[+key]||this[key]!==value)throw 'exact token';seen++;}
    else if(Object.hasOwn(context,'source'))throw 'array source';return value;
});
if(seen!==5||result[0]!=='\ud800'||result[1]!=='\udfff'||result[2]!=='\ud800\n'||result[3]!=='é\ud800'||result[4]!=='😀')throw 'decoded values';
'ok';
"#);
}

#[test]
fn json_utf16_reviver_omits_source_after_a_primitive_value_changes() {
    succeeds(r#"
var high='\ud800',low='\udfff',seen=0;
var result=JSON.parse('{"first":0,"\\ud800":"\\udfff"}',function(key,value,context){
    if(key==='first')this[high]='\ufffd';
    if(key===high){seen++;if(value!=='\ufffd'||value===low||Object.hasOwn(context,'source'))throw 'modified primitive source';}
    return value;
});
if(seen!==1||result[high]!=='\ufffd')throw 'live value';
'ok';
"#);
}

#[test]
fn json_utf16_reviver_omits_descendant_source_after_ancestor_replacement() {
    succeeds(r#"
var high='\ud800',low='\udfff',replacement={},seen=0;replacement[high]=low;
var result=JSON.parse('{"first":0,"child":{"\\ud800":"\\udfff"}}',function(key,value,context){
    if(key==='first')this.child=replacement;
    if(key===high){seen++;if(this!==replacement||value!==low||Object.hasOwn(context,'source'))throw 'replaced ancestor source';}
    return value;
});
if(seen!==1||result.child!==replacement||result.child[high]!==low)throw 'replacement identity';
'ok';
"#);
}

#[test]
fn json_utf16_reviver_visits_later_added_keys_without_source_but_keeps_root_snapshot() {
    succeeds(r#"
var high='\ud800',low='\udfff',root,child,added=0,original=0;
var result=JSON.parse('{"first":0,"child":{"\\udfff":"\\ud800"}}',function(key,value,context){
    if(key==='first'){root=this;child=this.child;child[high]=5;root[high]=6;}
    if(key===high){added++;if(this!==child||value!==5||Object.hasOwn(context,'source'))throw 'added key context';}
    if(key===low){original++;if(this!==child||value!==high||context.source!=='"\\ud800"')throw 'original child source';}
    return value;
});
if(added!==1||original!==1||result!==root||result[high]!==6||result.child[high]!==5)throw 'key snapshot timing';
'ok';
"#);
}

#[test]
fn json_utf16_source_conversion_is_once_and_nested_parsing_preserves_snapshots() {
    succeeds(r#"
var reads=0,calls=0,visited=0,source={toString:function(){throw 'unexpected fallback';}};
Object.defineProperty(source,Symbol.toPrimitive,{get:function(){reads++;return function(hint){
    calls++;if(this!==source||hint!=='string'||JSON.parse('"\\udfff"')!=='\udfff')throw 'source conversion';
    return '{"\\ud800":"\\udfff"}';
};}});
var result=JSON.parse(source,function(key,value,context){
    if(key==='\ud800'){
        visited++;if(context.source!=='"\\udfff"')throw 'outer snapshot';
        if(JSON.parse(JSON.stringify(value))!==value||context.source!=='"\\udfff"')throw 'nested parsing';
    }
    return value;
});
if(reads!==1||calls!==1||visited!==1||result['\ud800']!=='\udfff')throw 'conversion count';
'ok';
"#);
}

#[test]
fn json_utf16_source_and_reviver_throws_preserve_identity_and_published_holders() {
    succeeds(r#"
var marker={},source={},calls=0,revived=0,caught=0,saved;
source[Symbol.toPrimitive]=function(){calls++;throw marker;};
try{JSON.parse(source,function(key,value){revived++;return value;});}catch(e){if(e===marker)caught++;else throw e;}
try{JSON.parse('{"\\ud800":"\\udfff","later":1}',function(key,value){
    if(key==='\ud800'){saved=this;throw marker;}return value;
});}catch(e){if(e===marker)caught++;else throw e;}
if(caught!==2||calls!==1||revived!==0||saved['\ud800']!=='\udfff'||saved.later!==1)throw 'throw identity or published holder';
'ok';
"#);
}

#[test]
fn json_utf16_raw_source_accepts_literal_units_without_replacement() {
    let cases: &[&[u16]] = &[
        &[0xd800], &[0xdfff], &[0xd800, 0xdc00],
        &[0x61, 0xd800, 0x62, 0xdc00, 0xd83d, 0xde00],
    ];
    for expected in cases {
        let mut source = vec![0x22];
        source.extend_from_slice(expected);
        source.push(0x22);
        let mut runtime = Runtime::for_test();
        let value = runtime.json_parse_builtin(&[Value::StringUnits(source)], &mut NoHost).unwrap();
        assert_eq!(units(value), *expected);
    }
}

#[test]
fn json_utf16_raw_source_pairs_can_cross_raw_and_escape_boundaries() {
    let cases: &[(&[u16], &str, &[u16], &[u16])] = &[
        (&[0xd800], "\\udc00", &[], &[0xd800, 0xdc00]),
        (&[], "\\ud800", &[0xdc00], &[0xd800, 0xdc00]),
        (&[0xd800], "\\n", &[], &[0xd800, 0x0a]),
        (&[0xd800], "\\u0041", &[], &[0xd800, 0x41]),
        (&[0xd800], "\\\\udc00", &[], &[0xd800, 0x5c, 0x75, 0x64, 0x63, 0x30, 0x30]),
        (&[0xd800], "\\ud800", &[0xdc00], &[0xd800, 0xd800, 0xdc00]),
    ];
    for (prefix, escaped, suffix, expected) in cases {
        let mut source = vec![0x22];
        source.extend_from_slice(prefix);
        source.extend(escaped.encode_utf16());
        source.extend_from_slice(suffix);
        source.push(0x22);
        let mut runtime = Runtime::for_test();
        let value = runtime.json_parse_builtin(&[Value::StringUnits(source)], &mut NoHost).unwrap();
        assert_eq!(units(value), *expected);
    }
}

#[test]
fn json_utf16_raw_source_duplicate_key_uses_last_literal_token_and_first_position() {
    let mut source: Vec<u16> = "{\"\\ud800\":\"\\udfff\",\"middle\":0,\"".encode_utf16().collect();
    source.push(0xd800);
    source.extend("\":\"".encode_utf16());
    source.push(0xdfff);
    source.extend("\"}".encode_utf16());
    raw_source_succeeds(source, r#"
var high='\ud800',low='\udfff',seen=[];
var value=JSON.parse(rawJson,function(key,value,context){
    if(key===high){if(value!==low||context.source!=='"'+low+'"'||context.source.charCodeAt(1)!==57343)throw 'last raw token';seen.push(key);}
    if(key==='middle'){if(context.source!=='0')throw 'middle token';seen.push(key);}
    return value;
});
var keys=Object.keys(value);
if(keys.length!==2||keys[0]!==high||keys[1]!=='middle'||seen.length!==2||seen[0]!==high||seen[1]!=='middle')throw 'raw duplicate order';
if(value[high]!==low||Object.hasOwn(value,'\ufffd')||Object.hasOwn(value,'\\ud800'))throw 'raw key identity';
'ok';
"#);
}

#[test]
fn json_utf16_raw_source_context_ranges_preserve_astral_and_unpaired_source_units() {
    let mut source: Vec<u16> = " \t[\"😀\",\"".encode_utf16().collect();
    source.push(0xd800);
    source.extend("\",1.20e+2,\"".encode_utf16());
    source.push(0xdfff);
    source.extend("\\n\"] \r".encode_utf16());
    raw_source_succeeds(source, r#"
var high='\ud800',low='\udfff',expected=['"😀"','"'+high+'"','1.20e+2','"'+low+'\\n"'],seen=0;
var result=JSON.parse(rawJson,function(key,value,context){
    if(key!==''){if(context.source!==expected[+key]||this[key]!==value)throw 'raw source range';seen++;}
    else if(Object.hasOwn(context,'source'))throw 'array context';return value;
});
if(seen!==4||result[0]!=='😀'||result[1]!==high||result[2]!==120||result[3]!==low+'\n')throw 'raw decoded values';
'ok';
"#);
}

#[test]
fn json_utf16_raw_source_conversion_and_reentrant_reviver_keep_exact_root_context() {
    raw_source_succeeds(vec![0x22, 0xd800, 0x22], r#"
var reads=0,calls=0,visits=0,source={toString:function(){throw 'fallback';}};
Object.defineProperty(source,Symbol.toPrimitive,{get:function(){reads++;return function(hint){
    calls++;if(this!==source||hint!=='string'||JSON.parse('"\\udfff"')!=='\udfff')throw 'raw conversion protocol';return rawJson;
};}});
var result=JSON.parse(source,function(key,value,context){
    visits++;if(key!==''||value!=='\ud800'||this['']!==value||context.source!==rawJson)throw 'raw root context';
    if(JSON.parse(JSON.stringify(value))!==value||context.source!==rawJson)throw 'raw recursive context';
    return value;
});
if(result!=='\ud800'||reads!==1||calls!==1||visits!==1)throw 'raw source conversion count';
'ok';
"#);
}

#[test]
fn json_utf16_malformed_escape_after_high_surrogate_is_catchable_before_reviving() {
    for source in [
        r#""\ud800\u12G4""#,
        r#""\ud800\u12""#,
        r#""\ud800\u"#,
        r#""\ud800\x41""#,
        r#""\ud800\u{41}""#,
        "\"\\ud800\\",
    ] {
        let mut runtime = Runtime::for_test();
        runtime.define_global("invalidJson", Value::String(source.into())).unwrap();
        let value = runtime.execute(r#"
var calls=0,caught=false;
try{JSON.parse(invalidJson,function(key,value){calls++;return value;});}
catch(e){if(e.name!=='SyntaxError')throw e;caught=true;}
if(!caught||calls!==0)throw 'malformed JSON reached reviver';
'ok';
"#, &mut NoHost).unwrap();
        assert_eq!(value.to_text(), "ok", "{source}");
        assert_eq!(runtime.json_scratch_bytes, 0);
    }
    assert_eq!(parsed_units(r#""\ud800""#), vec![0xd800]);
}

#[test]
fn json_utf16_raw_source_controls_and_non_ascii_syntax_are_catchable_errors() {
    let mut cases: Vec<Vec<u16>> = (0..=0x1f).map(|control| vec![0x22, control, 0x22]).collect();
    cases.extend([
        vec![0x0122, 0x61, 0x22], // Narrowing to a byte would invent a quote.
        vec![0x015b, 0x31, 0x5d], // Narrowing would invent an array opener.
        vec![0x017b, 0x22, 0x61, 0x22, 0x3a, 0x31, 0x7d],
        vec![0xd800], vec![0xdc00],
        vec![0x00a0, 0x31], vec![0x31, 0x00a0],
        vec![0x31, 0xd800], vec![0x31, 0xdc00],
    ]);
    for source in cases {
        let mut runtime = Runtime::for_test();
        let fault = runtime.json_parse_builtin(
            &[Value::StringUnits(source.clone())], &mut NoHost,
        ).unwrap_err();
        assert!(matches!(fault.exception.as_deref(), Some(super::super::Exception::Error("SyntaxError"))),
            "Source {source:?} did not produce a catchable SyntaxError: {fault:?}");
        assert_eq!(runtime.json_scratch_bytes, 0);
    }
}

#[test]
fn json_utf16_raw_source_valid_grammar_keeps_numbers_literals_and_canonical_key_order() {
    let mut source: Vec<u16> = concat!(
        " \t\r\n{\"10\":null,\"2\":true,\"1\":false,\"n\":-0,\"e\":1.25e+2,",
        "\"a\":[null,true,false,0,-1,2E-1],\"😀\":1,\"\\ud83d\\ude00\":2,\"s\":\"",
    ).encode_utf16().collect();
    source.push(0xd800);
    source.extend("\"} \t\r\n".encode_utf16());
    raw_source_succeeds(source, r#"
var value=JSON.parse(rawJson),keys=Object.keys(value);
if(keys.join('|')!=='1|2|10|n|e|a|😀|s')throw 'numeric or canonical key order';
if(value[1]!==false||value[2]!==true||value[10]!==null||1/value.n!==-Infinity||value.e!==125)throw 'literal or number grammar';
if(!Array.isArray(value.a)||value.a.length!==6||JSON.stringify(value.a)!=='[null,true,false,0,-1,0.2]')throw 'array grammar';
if(value['😀']!==2||value.s!=='\ud800'||JSON.stringify(value.s)!=='"\\ud800"')throw 'raw string or canonical duplicate';
'ok';
"#);
}
