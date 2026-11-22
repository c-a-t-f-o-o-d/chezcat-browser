//! Original native API witnesses for the selected Chinese and Korean decoders.
//! Mapping-table exhaustiveness is checked separately against the pinned data.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected native read")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected native write")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected native call")
    }
}

fn runtime() -> Runtime {
    Runtime::with_limits(super::super::limits::BROWSER)
}

fn succeeds(source: &str) {
    assert_eq!(runtime().execute(source, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn multibyte_every_selected_label_keeps_its_canonical_decoder_identity() {
    let source = format!(
        "var groups={};var count=0,labelCount=0;\
         for(var group of groups){{for(var encoding of group.encodings){{\
           var name=encoding.name.toLowerCase();\
           if(name!=='gbk'&&name!=='gb18030'&&name!=='euc-kr')continue;\
           count++;for(var label of encoding.labels){{\
             labelCount++;var d=new TextDecoder(' \\t'+label.toUpperCase()+'\\n\\r\\f ');\
             if(d.encoding!==name||d.fatal||d.ignoreBOM)throw 'canonical decoder '+label;\
             if(d.decode(new Uint8Array([65]))!=='A')throw 'label decode '+label;\
           }}\
         }}}}\
         if(count!==3||labelCount!==20)throw 'selected label inventory changed';'ok';",
        include_str!("../tools/reference/multibyte-encodings/encodings.json")
    );
    succeeds(&source);
}

#[test]
fn multibyte_known_characters_and_supplementary_code_units_are_preserved() {
    // These selected scalars are fixed witnesses from the attributed indexes
    // and the gb18030 ranges algorithm, not output from the decoder under test.
    succeeds(r#"
for(var name of ['gbk','gb18030']){
  var d=new TextDecoder(name);
  if(d.decode(new Uint8Array([0,65,128,214,208,206,196]))!=='\u0000A\u20ac\u4e2d\u6587')throw 'Chinese scalars';
  var s=d.decode(new Uint8Array([129,48,129,48,144,48,129,48,227,50,154,53]));
  if(s!=='\u0080\ud800\udc00\udbff\udfff'||s.length!==5)throw 'supplementary string';
  if(s.charCodeAt(1)!==0xd800||s.charCodeAt(2)!==0xdc00||s.charCodeAt(3)!==0xdbff||s.charCodeAt(4)!==0xdfff)throw 'UTF-16 code units';
  if(d.decode(new Uint8Array([129,53,244,55]))!=='\ue7c7')throw 'range exception';
  if(d.encoding!==name)throw 'distinct GBK identity';
}
var korean=new TextDecoder('windows-949');
if(korean.decode(new Uint8Array([65,176,161,129,65,34]))!=='A\uac00\uac02"'||korean.encoding!=='euc-kr')throw 'Korean and extension scalars';
'ok';
"#);
}

#[test]
fn multibyte_every_selected_stream_split_and_empty_chunk_retains_state() {
    succeeds(r#"
var cases=[
  ['gbk',[65],'A'],
  ['gbk',[128],'\u20ac'],
  ['gbk',[214,208],'\u4e2d'],
  ['gbk',[144,48,129,48],'\ud800\udc00'],
  ['gb18030',[129,48,129,48],'\u0080'],
  ['gb18030',[129,53,244,55],'\ue7c7'],
  ['gb18030',[65,214,208,144,48,129,48,34],'A\u4e2d\ud800\udc00"'],
  ['euc-kr',[65],'A'],
  ['euc-kr',[176,161],'\uac00'],
  ['euc-kr',[129,65],'\uac02'],
  ['euc-kr',[65,176,161,129,65,34],'A\uac00\uac02"']
];
for(var test of cases){
  var bytes=new Uint8Array(test[1]);
  for(var split=0;split<=bytes.length;split++){
    var d=new TextDecoder(test[0]),result=d.decode(undefined,{stream:true});
    result+=d.decode(bytes.subarray(0,split),{stream:true});
    result+=d.decode(new Uint8Array(0),{stream:true});
    result+=d.decode(bytes.subarray(split),{stream:true});
    result+=d.decode(undefined,{stream:true});
    result+=d.decode();
    if(result!==test[2]||d.decode()!=='')throw 'split '+test[0]+' '+split;
  }
  var single=new TextDecoder(test[0]),result='';
  for(var i=0;i<bytes.length;i++){
    result+=single.decode(bytes.subarray(i,i+1),{stream:true});
    if(single.decode(undefined,{stream:true})!=='')throw 'empty chunk flushed prefix';
  }
  if(result+single.decode()!==test[2])throw 'one byte at a time';
}
'ok';
"#);
}

#[test]
fn multibyte_pending_prefix_is_copied_and_decoder_instances_are_independent() {
    succeeds(r#"
for(var test of [
  ['gbk',[214],[208],'\u4e2d'],
  ['gb18030',[144,48,129],[48],'\ud800\udc00'],
  ['euc-kr',[129],[65],'\uac02']
]){
  var prefix=new Uint8Array(test[1]),d=new TextDecoder(test[0]);
  if(d.decode(prefix,{stream:true})!=='')throw 'incomplete prefix output';
  prefix.fill(65);
  if(d.decode(new Uint8Array(test[2]))!==test[3])throw 'borrowed prefix bytes';
  if(d.decode(new Uint8Array([65]))!=='A'||d.decode()!=='')throw 'completed decoder reset';
}
var left=new TextDecoder('gb18030'),right=new TextDecoder('gb18030');
if(left.decode(new Uint8Array([144,48]),{stream:true})!==''||right.decode(new Uint8Array([214]),{stream:true})!=='')throw 'independent prefixes';
if(right.decode(new Uint8Array([208]))!=='\u4e2d'||left.decode(new Uint8Array([129,48]))!=='\ud800\udc00')throw 'shared decoder state';
'ok';
"#);
}

#[test]
fn multibyte_malformed_and_unmapped_input_restores_ascii_delimiters() {
    succeeds(r#"
var chinese=[
  [[129,34,60],'\ufffd"<'],
  [[129,48,34,60],'\ufffd0"<'],
  [[129,48,129,34,60],'\ufffd0\ufffd"<'],
  [[129,48,129,64],'\ufffd0\u4e02'],
  [[129,255,34],'\ufffd"'],
  [[132,49,165,48,34],'\ufffd"'],
  [[227,50,154,54,34],'\ufffd"']
];
for(var name of ['gbk','gb18030']){
  for(var test of chinese){
    var bytes=new Uint8Array(test[0]),d=new TextDecoder(name);
    if(d.decode(bytes)!==test[1])throw 'Chinese malformed input';
    if(d.decode(bytes.subarray(0,1),{stream:true})+d.decode(bytes.subarray(1))!==test[1])throw 'Chinese malformed stream';
  }
}
for(var test of [
  [[129,34,60],'\ufffd"<'],
  [[129,91,34],'\ufffd["'],
  [[129,128,34],'\ufffd"'],
  [[128,255,34],'\ufffd\ufffd"']
]){
  var bytes=new Uint8Array(test[0]),d=new TextDecoder('euc-kr');
  if(d.decode(bytes)!==test[1])throw 'Korean malformed or unmapped input';
  if(d.decode(bytes.subarray(0,1),{stream:true})+d.decode(bytes.subarray(1))!==test[1])throw 'Korean malformed stream';
}
'ok';
"#);
}

#[test]
fn multibyte_final_eof_and_fatal_errors_recover_without_partial_return() {
    succeeds(r#"
function fatal(f){try{f();}catch(e){if(e.name==='TypeError')return;throw e;}throw 'missing fatal TypeError';}
for(var test of [
  ['gbk',[129]],['gbk',[129,48]],['gbk',[129,48,129]],
  ['gb18030',[129]],['gb18030',[129,48]],['gb18030',[129,48,129]],
  ['euc-kr',[176]]
]){
  var bytes=new Uint8Array(test[1]),d=new TextDecoder(test[0]);
  if(d.decode(bytes)!=='\ufffd')throw 'final incomplete sequence';
  if(d.decode(bytes,{stream:true})!==''||d.decode(undefined,{stream:true})!=='')throw 'premature EOF';
  if(d.decode()!=='\ufffd'||d.decode()!==''||d.decode(new Uint8Array([65]))!=='A')throw 'EOF flush and reset';
  var strict=new TextDecoder(test[0],{fatal:true});
  if(strict.decode(bytes,{stream:true})!=='')throw 'fatal prefix';
  var returned='unchanged';
  fatal(function(){returned=strict.decode();});
  if(returned!=='unchanged'||!strict.fatal||strict.decode(new Uint8Array([65]),{stream:true})!=='A'||strict.decode()!=='')throw 'fatal EOF recovery';
}
for(var name of ['gbk','gb18030','euc-kr']){
  var strict=new TextDecoder(name,{fatal:true}),returned='unchanged';
  fatal(function(){returned=strict.decode(new Uint8Array([65,129,34]),{stream:true});});
  if(returned!=='unchanged'||strict.decode(new Uint8Array([66]))!=='B')throw 'partial fatal output or stale prefix';
}
'ok';
"#);
}

#[test]
fn multibyte_explicit_legacy_decoder_treats_bom_bytes_as_data() {
    // EF BB and BF 41 are two ordinary pairs in these indexes. TextDecoder
    // does not invoke the separate request decoder's BOM override.
    // The ranges entry (39116, U+FE6C) gives pointer 39263 for U+FEFF:
    // 84 31 95 33. Even an actual BOM scalar remains legacy-decoded data.
    succeeds(r#"
for(var test of [['gbk','\u9518\u7dfc'],['gb18030','\u9518\u7dfc'],['euc-kr','\u7664\ud49e']]){
  var bytes=new Uint8Array([239,187,191,65]),d=new TextDecoder(test[0]);
  if(d.decode(bytes)!==test[1]||new TextDecoder(test[0],{ignoreBOM:true}).decode(bytes)!==test[1])throw 'legacy BOM bytes';
  for(var split=0;split<=bytes.length;split++){
    if(d.decode(bytes.subarray(0,split),{stream:true})+d.decode(bytes.subarray(split))!==test[1])throw 'split legacy BOM bytes';
  }
}
for(var name of ['gbk','gb18030']){
  var bytes=new Uint8Array([132,49,149,51,65]);
  for(var ignore of [false,true]){
    var d=new TextDecoder(name,{ignoreBOM:ignore});
    if(d.decode(bytes)!=='\ufeffA')throw 'legacy decoded BOM scalar';
    for(var split=0;split<=bytes.length;split++){
      if(d.decode(bytes.subarray(0,split),{stream:true})+d.decode(bytes.subarray(split))!=='\ufeffA')throw 'split legacy decoded BOM scalar';
    }
  }
}
'ok';
"#);
}

#[test]
fn multibyte_buffer_views_option_reentry_and_native_brands_are_real() {
    succeeds(r#"
function rejects(f,name){try{f();}catch(e){if(e.name===name)return;throw e;}throw 'missing '+name;}
var bytes=new Uint8Array([0,214,208,0]),d=new TextDecoder('gbk');
if(d.decode(bytes.subarray(1,3))!=='\u4e2d'||d.decode(new DataView(bytes.buffer,1,2))!=='\u4e2d')throw 'view extent';
var seen=0,options={get stream(){seen++;bytes[1]=206;bytes[2]=196;if(d.decode(new Uint8Array([65]))!=='A')throw 'nested decoder';return true;}};
if(d.decode(bytes.subarray(1,3),options)!=='\u6587'||seen!==1||d.decode()!=='')throw 'input copy after options';
if(bytes[0]!==0||bytes[3]!==0)throw 'input mutation';
var stateful=new TextDecoder('gb18030'),options={get stream(){if(stateful.decode(new Uint8Array([214]),{stream:true})!=='')throw 'nested prefix';return false;}};
if(stateful.decode(new Uint8Array([208]),options)!=='\u4e2d')throw 'state after option reentry';
var whole=new Uint8Array([176,161]);
if(new TextDecoder('euc-kr').decode(whole.buffer)!=='\uac00')throw 'ArrayBuffer input';
var log=[],label={toString:function(){log.push('label');return 'GBK';}},ctorOptions={get fatal(){log.push('fatal');return true;},get ignoreBOM(){log.push('ignoreBOM');return true;}};
var strict=new TextDecoder(label,ctorOptions);
if(log.join(',')!=='label,fatal,ignoreBOM'||strict.encoding!=='gbk'||!strict.fatal||!strict.ignoreBOM)throw 'constructor dictionary order';
rejects(function(){TextDecoder('gb18030');},'TypeError');
rejects(function(){TextDecoder.prototype.decode.call({},new Uint8Array([65]));},'TypeError');
for(var property of ['encoding','fatal','ignoreBOM']){
  rejects(function(){Object.getOwnPropertyDescriptor(TextDecoder.prototype,property).get.call({});},'TypeError');
}
rejects(function(){new TextDecoder('\vgbk');},'RangeError');
rejects(function(){new TextDecoder('gb18030\u00a0');},'RangeError');
rejects(function(){strict.decode(null);},'TypeError');
rejects(function(){strict.decode([65]);},'TypeError');
'ok';
"#);
}

#[test]
fn multibyte_shared_request_text_retains_bom_override_and_utf8_body_rules() {
    let mut engine = runtime();
    for name in ["gbk", "gb18030"] {
        engine.begin_job();
        assert_eq!(
            engine.decode_request_text(&[0xd6, 0xd0, 0x90, 0x30, 0x81, 0x30], name).unwrap(),
            "中\u{10000}"
        );
        assert_eq!(engine.decode_request_text(&[0x81, 0x22], name).unwrap(), "\u{fffd}\"");
        assert_eq!(engine.decode_request_text(&[0x81, 0x30], name).unwrap(), "\u{fffd}");
    }
    assert_eq!(
        engine.decode_request_text(&[0xb0, 0xa1, 0x81, 0x41], "windows-949").unwrap(),
        "가갂"
    );
    assert_eq!(
        engine.decode_request_text(&[0x81, 0x5b, 0x22], "euc-kr").unwrap(),
        "\u{fffd}[\""
    );
    for name in ["gbk", "gb18030", "euc-kr"] {
        engine.begin_job();
        assert_eq!(
            engine.decode_request_text(&[0xef, 0xbb, 0xbf, 0xc3, 0xa9], name).unwrap(),
            "é"
        );
        assert_eq!(engine.decode_request_text(&[0xff, 0xfe, 65, 0], name).unwrap(), "A");
        assert_eq!(engine.decode_request_text(&[0xfe, 0xff, 0, 65], name).unwrap(), "A");
    }
    // XHR lowercases extracted MIME labels but preserves quoted whitespace.
    // The shared decoder must remove exactly Encoding's ASCII whitespace.
    let labels: &[(&str, &[u8], &str)] = &[
        ("gbk", &[0xd6, 0xd0], "中"),
        ("gb18030", &[0x90, 0x30, 0x81, 0x30], "\u{10000}"),
        ("euc-kr", &[0xb0, 0xa1], "가"),
        ("cp1251", &[0xc0], "А"),
    ];
    for &(name, bytes, expected) in labels {
        engine.begin_job();
        let spaced = format!("\t\n\u{c}\r {name} \r\u{c}\n\t");
        assert_eq!(engine.decode_request_text(bytes, &spaced).unwrap(), expected);
        for invalid in [
            format!("\u{b}{name}"),
            format!("{name}\u{b}"),
            format!("\u{a0}{name}"),
            format!("{name}\u{a0}"),
        ] {
            assert!(
                engine.decode_request_text(bytes, &invalid).is_err(),
                "non-ASCII-whitespace label {invalid:?}"
            );
        }
    }
    assert_eq!(engine.decode_body_utf8(&[0xd6, 0xd0]).unwrap(), "\u{fffd}\u{fffd}");
    assert!(engine.decode_request_text(&[65], "shift_jis").is_err());
}

#[test]
fn multibyte_work_and_output_refusal_publish_no_partial_string_and_recover() {
    let mut engine = runtime();
    let retained = engine.value_bytes;
    let original = engine.limits.capacity.string_bytes;
    let cases: &[(&str, &[u8], &str)] = &[
        ("gbk", &[65, 0x90, 0x30, 0x81, 0x30], "A\u{10000}"),
        ("gb18030", &[65, 0x90, 0x30, 0x81, 0x30], "A\u{10000}"),
        ("euc-kr", &[65, 0xb0, 0xa1], "A가"),
    ];
    for &(name, bytes, expected) in cases {
        // Cover refusal before input copy, before output, and after an ASCII
        // prefix has already been appended to the unpublished local string.
        for allowance in [0, bytes.len() - 1, bytes.len(), bytes.len() + 1] {
            engine.begin_job();
            engine.copy_bytes = allowance;
            let error = engine.decode_request_text(bytes, name).unwrap_err();
            assert!(error.contains("work limit"), "{name}, allowance {allowance}: {error}");
            assert_eq!(engine.value_bytes, retained);
            engine.begin_job();
            assert_eq!(engine.decode_request_text(bytes, name).unwrap(), expected);
        }
        engine.limits.capacity.string_bytes = expected.len() - 1;
        engine.begin_job();
        let error = engine.decode_request_text(bytes, name).unwrap_err();
        assert!(error.contains("String limit"), "{name}: {error}");
        assert_eq!(engine.value_bytes, retained);
        engine.limits.capacity.string_bytes = original;
        engine.begin_job();
        assert_eq!(engine.decode_request_text(bytes, name).unwrap(), expected);
    }
}
