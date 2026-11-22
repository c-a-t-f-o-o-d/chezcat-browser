//! Original native API witnesses for Big5 and the Japanese decoder families.
//! Expected characters come from the pinned indexes and Encoding algorithms.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected native read") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected native write") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected native call") }
}

fn runtime() -> Runtime { Runtime::with_limits(super::super::limits::BROWSER) }

fn succeeds(source: &str) {
    assert_eq!(runtime().execute(source, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn east_asian_all_eighteen_labels_keep_their_canonical_identity() {
    let source = format!(
        "var groups={};var count=0,labelCount=0;\
         for(var group of groups){{for(var encoding of group.encodings){{\
           var name=encoding.name.toLowerCase();\
           if(name!=='big5'&&name!=='euc-jp'&&name!=='iso-2022-jp'&&name!=='shift_jis')continue;\
           count++;for(var label of encoding.labels){{\
             labelCount++;var d=new TextDecoder(' \\t'+label.toUpperCase()+'\\n\\r\\f ');\
             if(d.encoding!==name||d.fatal||d.ignoreBOM)throw 'decoder identity '+label;\
             if(d.decode(new Uint8Array([65]))!=='A')throw 'decoder alias '+label;\
           }}\
         }}}}\
         if(count!==4||labelCount!==18)throw 'selected label inventory';'ok';",
        include_str!("../tools/reference/multibyte-encodings/encodings.json")
    );
    succeeds(&source);
}

#[test]
fn east_asian_known_scalars_pairs_and_supplementary_utf16_are_exact() {
    succeeds(r#"
var big5=new TextDecoder('big5-hkscs');
if(big5.decode(new Uint8Array([164,164,164,229]))!=='\u4e2d\u6587')throw 'Big5 words';
var pairs=big5.decode(new Uint8Array([136,98,136,100,136,163,136,165]));
if(pairs!=='\u00ca\u0304\u00ca\u030c\u00ea\u0304\u00ea\u030c'||pairs.length!==8)throw 'Big5 scalar pairs';
var supplementary=big5.decode(new Uint8Array([135,69]));
if(supplementary!=='\ud85c\ude67'||supplementary.length!==2||supplementary.charCodeAt(0)!==0xd85c||supplementary.charCodeAt(1)!==0xde67)throw 'Big5 supplementary units';
var japanese=new TextDecoder('shift_jis');
if(japanese.decode(new Uint8Array([0,65,128,161,223,130,160,240,64,249,252]))!=='\u0000A\u0080\uff61\uff9f\u3042\ue000\ue757')throw 'Shift_JIS direct and private scalars';
if(new TextDecoder('euc-jp').decode(new Uint8Array([65,142,161,142,223,164,162,143,162,175]))!=='A\uff61\uff9f\u3042\u02d8')throw 'EUC-JP planes';
'ok';
"#);
}

#[test]
fn east_asian_every_split_empty_chunk_and_byte_chunk_preserves_decoding() {
    succeeds(r#"
var cases=[
  ['big5',[65,164,164,136,98,135,69,34],'A\u4e2d\u00ca\u0304\ud85c\ude67"'],
  ['shift_jis',[65,128,161,130,160,240,64,249,252,34],'A\u0080\uff61\u3042\ue000\ue757"'],
  ['euc-jp',[65,142,161,164,162,143,162,175,34],'A\uff61\u3042\u02d8"'],
  ['iso-2022-jp',[65,27,40,74,92,126,27,40,73,33,95,27,36,66,36,34,27,40,66,34],'A\u00a5\u203e\uff61\uff9f\u3042"'],
  ['iso-2022-jp',[27,36,64,36,34],'\u3042']
];
for(var test of cases){
  var bytes=new Uint8Array(test[1]);
  for(var split=0;split<=bytes.length;split++){
    var d=new TextDecoder(test[0]),result=d.decode(undefined,{stream:true});
    result+=d.decode(bytes.subarray(0,split),{stream:true});
    result+=d.decode(new Uint8Array(0),{stream:true});
    result+=d.decode(bytes.subarray(split),{stream:true});
    result+=d.decode(undefined,{stream:true});result+=d.decode();
    if(result!==test[2]||d.decode()!=='')throw 'split '+test[0]+' '+split;
    if(d.decode(new Uint8Array([92]))!=='\\')throw 'finished decoder mode reset';
  }
  var d=new TextDecoder(test[0]),result='';
  for(var i=0;i<bytes.length;i++){
    result+=d.decode(bytes.subarray(i,i+1),{stream:true});
    if(d.decode(undefined,{stream:true})!=='')throw 'empty chunk flushed prefix';
  }
  if(result+d.decode()!==test[2])throw 'byte chunks '+test[0];
}
'ok';
"#);
}

#[test]
fn east_asian_pending_bytes_are_copied_and_instances_keep_independent_modes() {
    succeeds(r#"
for(var test of [
  ['big5',[136],[98],'\u00ca\u0304'],
  ['big5',[135],[69],'\ud85c\ude67'],
  ['shift_jis',[130],[160],'\u3042'],
  ['euc-jp',[143,162],[175],'\u02d8'],
  ['iso-2022-jp',[27,40],[74,92],'\u00a5']
]){
  var prefix=new Uint8Array(test[1]),d=new TextDecoder(test[0]);
  if(d.decode(prefix,{stream:true})!=='')throw 'prefix output';
  prefix.fill(65);
  if(d.decode(new Uint8Array(test[2]))!==test[3])throw 'borrowed prefix';
  if(d.decode(new Uint8Array([92]))!=='\\'||d.decode()!=='')throw 'decoder reset';
}
var left=new TextDecoder('iso-2022-jp'),right=new TextDecoder('iso-2022-jp');
if(left.decode(new Uint8Array([27,40,74]),{stream:true})!==''||right.decode(new Uint8Array([27,40,73]),{stream:true})!=='')throw 'designation output';
if(right.decode(new Uint8Array([33]))!=='\uff61'||left.decode(new Uint8Array([92]))!=='\u00a5')throw 'shared ISO mode';
'ok';
"#);
}

#[test]
fn east_asian_malformed_and_unmapped_sequences_restore_ascii_delimiters() {
    succeeds(r#"
var cases=[
  ['big5',[129,34,60],'\ufffd"<'],
  ['big5',[129,64,34],'\ufffd@"'],
  ['big5',[129,161,34],'\ufffd"'],
  ['big5',[128,255,34],'\ufffd\ufffd"'],
  ['shift_jis',[129,34,60],'\ufffd"<'],
  ['shift_jis',[129,127,34],'\ufffd\u007f"'],
  ['shift_jis',[252,252,34],'\ufffd"'],
  ['euc-jp',[164,34,60],'\ufffd"<'],
  ['euc-jp',[142,34,60],'\ufffd"<'],
  ['euc-jp',[143,162,34,60],'\ufffd"<'],
  ['euc-jp',[143,34,60],'\ufffd"<'],
  ['euc-jp',[143,161,161,34],'\ufffd"']
];
for(var test of cases){
  var bytes=new Uint8Array(test[1]);
  for(var split=0;split<=bytes.length;split++){
    var d=new TextDecoder(test[0]);
    if(d.decode(bytes.subarray(0,split),{stream:true})+d.decode(bytes.subarray(split))!==test[2])throw 'malformed '+test[0]+' '+split;
  }
}
'ok';
"#);
}

#[test]
fn east_asian_iso_designations_repeats_escaped_errors_and_eof_are_observable() {
    succeeds(r#"
var cases=[
  [[65,14,15,128,66],'A\ufffd\ufffd\ufffdB'],
  [[27,40,66,27,40,74,92],'\ufffd\u00a5'],
  [[27,40,66,27,40,74,27,40,73,33],'\ufffd\ufffd\uff61'],
  [[27,40,74,92,27,40,74,126],'\u00a5\u203e'],
  [[27,88,34],'\ufffdX"'],
  [[27,40,88,34],'\ufffd(X"'],
  [[27,40,74,92,27,40,88,92,126],'\u00a5\ufffd(X\u00a5\u203e'],
  [[27,36,66,36,27,40,66,34],'\ufffd"'],
  [[27],'\ufffd'],
  [[27,40],'\ufffd('],
  [[27,36],'\ufffd$'],
  [[27,40,74],''],
  [[27,36,66,36],'\ufffd'],
  [[27,36,66,36,34,27,40],'\u3042\ufffd\ufffd']
];
for(var test of cases){
  var bytes=new Uint8Array(test[0]);
  for(var split=0;split<=bytes.length;split++){
    var d=new TextDecoder('iso-2022-jp'),result=d.decode(bytes.subarray(0,split),{stream:true});
    result+=d.decode(undefined,{stream:true});result+=d.decode(bytes.subarray(split),{stream:true});
    if(result+d.decode()!==test[1]||d.decode()!=='')throw 'ISO state or EOF '+split;
  }
}
'ok';
"#);
}

#[test]
fn east_asian_fatal_errors_reset_all_state_without_returning_partial_output() {
    succeeds(r#"
function fatal(f){try{f();}catch(e){if(e.name==='TypeError')return;throw e;}throw 'missing fatal TypeError';}
for(var test of [
  ['big5',[135]],['shift_jis',[130]],['euc-jp',[143]],['euc-jp',[143,162]],
  ['iso-2022-jp',[27]],['iso-2022-jp',[27,40]],['iso-2022-jp',[27,36,66,36]]
]){
  var d=new TextDecoder(test[0],{fatal:true}),returned='unchanged';
  if(d.decode(new Uint8Array(test[1]),{stream:true})!=='')throw 'fatal pending output';
  fatal(function(){returned=d.decode();});
  if(returned!=='unchanged'||!d.fatal||d.decode(new Uint8Array([92]),{stream:true})!=='\\'||d.decode()!=='')throw 'fatal EOF reset';
}
for(var test of [
  ['big5',[65,129,34]],['shift_jis',[65,129,34]],['euc-jp',[65,143,162,34]],
  ['iso-2022-jp',[65,27,40,74,92,27,40,66,27,40,74,92]]
]){
  var d=new TextDecoder(test[0],{fatal:true}),returned='unchanged';
  fatal(function(){returned=d.decode(new Uint8Array(test[1]),{stream:true});});
  if(returned!=='unchanged'||d.decode(new Uint8Array([92]),{stream:true})!=='\\'||d.decode()!=='')throw 'fatal input reset';
}
'ok';
"#);
}

#[test]
fn east_asian_explicit_decoders_treat_unicode_bom_bytes_as_legacy_data() {
    succeeds(r#"
for(var test of [
  ['big5','\u569c\u6fa4'],['shift_jis','\ufffd\uff7fA'],
  ['euc-jp','\u93e4\ufffdA'],['iso-2022-jp','\ufffd\ufffd\ufffdA']
]){
  var bytes=new Uint8Array([239,187,191,65]);
  for(var ignore of [false,true]){
    var d=new TextDecoder(test[0],{ignoreBOM:ignore});
    if(d.decode(bytes)!==test[1]||d.ignoreBOM!==ignore)throw 'explicit legacy BOM data';
    for(var split=0;split<=bytes.length;split++){
      if(d.decode(bytes.subarray(0,split),{stream:true})+d.decode(bytes.subarray(split))!==test[1])throw 'split BOM bytes';
    }
  }
}
'ok';
"#);
}

#[test]
fn east_asian_views_option_reentry_and_native_brands_use_real_decoder_state() {
    succeeds(r#"
function rejects(f,name){try{f();}catch(e){if(e.name===name)return;throw e;}throw 'missing '+name;}
var bytes=new Uint8Array([0,164,164,0]),d=new TextDecoder('big5');
if(d.decode(bytes.subarray(1,3))!=='\u4e2d'||d.decode(new DataView(bytes.buffer,1,2))!=='\u4e2d')throw 'view extent';
var seen=0,options={get stream(){seen++;bytes[2]=229;if(d.decode(new Uint8Array([65]))!=='A')throw 'nested decoder';return true;}};
if(d.decode(bytes.subarray(1,3),options)!=='\u6587'||seen!==1||d.decode()!=='')throw 'input copy after options';
if(bytes[0]!==0||bytes[3]!==0)throw 'input mutation';
var stateful=new TextDecoder('iso-2022-jp'),options={get stream(){if(stateful.decode(new Uint8Array([27,40,74]),{stream:true})!=='')throw 'nested designation';return false;}};
if(stateful.decode(new Uint8Array([92]),options)!=='\u00a5'||stateful.decode(new Uint8Array([92]))!=='\\')throw 'state after option reentry';
if(new TextDecoder('euc-jp').decode(new Uint8Array([143,162,175]).buffer)!=='\u02d8')throw 'ArrayBuffer input';
var log=[],label={toString:function(){log.push('label');return 'SHIFT_JIS';}},ctorOptions={get fatal(){log.push('fatal');return true;},get ignoreBOM(){log.push('ignoreBOM');return true;}};
var strict=new TextDecoder(label,ctorOptions);
if(log.join(',')!=='label,fatal,ignoreBOM'||strict.encoding!=='shift_jis'||!strict.fatal||!strict.ignoreBOM)throw 'constructor dictionary order';
rejects(function(){TextDecoder('big5');},'TypeError');
rejects(function(){TextDecoder.prototype.decode.call({},new Uint8Array([65]));},'TypeError');
for(var property of ['encoding','fatal','ignoreBOM'])rejects(function(){Object.getOwnPropertyDescriptor(TextDecoder.prototype,property).get.call({});},'TypeError');
for(var label of ['\vbig5','euc-jp\u00a0','replacement','iso-2022-cn'])rejects(function(){new TextDecoder(label);},'RangeError');
rejects(function(){strict.decode(null);},'TypeError');rejects(function(){strict.decode([65]);},'TypeError');
'ok';
"#);
}

#[test]
fn east_asian_shared_request_decoding_retains_bom_override_and_fetch_utf8() {
    let mut engine = runtime();
    let cases: &[(&str, &[u8], &str)] = &[
        ("big5-hkscs", &[136, 98, 135, 69], "\u{ca}\u{304}\u{27267}"),
        ("windows-31j", &[128, 130, 160, 240, 64], "\u{80}あ\u{e000}"),
        ("euc-jp", &[143, 162, 175, 164, 162], "\u{2d8}あ"),
        ("iso-2022-jp", &[27, 40, 74, 92, 27, 40, 66, 65], "¥A"),
    ];
    for &(name, bytes, expected) in cases {
        engine.begin_job();
        assert_eq!(engine.decode_request_text(bytes, name).unwrap(), expected);
        assert_eq!(engine.decode_request_text(bytes, &format!("\t {name}\n")).unwrap(), expected);
        assert_eq!(engine.decode_request_text(&[0xef, 0xbb, 0xbf, 0xc3, 0xa9], name).unwrap(), "é");
        assert_eq!(engine.decode_request_text(&[0xff, 0xfe, 65, 0], name).unwrap(), "A");
        assert_eq!(engine.decode_request_text(&[0xfe, 0xff, 0, 65], name).unwrap(), "A");
    }
    assert_eq!(engine.decode_request_text(&[27, 40], "iso-2022-jp").unwrap(), "\u{fffd}(");
    assert_eq!(engine.decode_request_text(&[129, 34], "big5").unwrap(), "\u{fffd}\"");
    assert_eq!(engine.decode_body_utf8(&[164, 164]).unwrap(), "\u{fffd}\u{fffd}");
    assert_eq!(engine.decode_body_utf8(&[27, 36, 66, 36, 34]).unwrap(), "\u{1b}$B$\"");
    assert_eq!(engine.decode_request_text(&[65], "replacement").unwrap(), "\u{fffd}");
}

#[test]
fn east_asian_work_and_output_refusal_publish_no_partial_string_and_recover() {
    let mut engine = runtime();
    let retained = engine.value_bytes;
    let original = engine.limits.capacity.string_bytes;
    let cases: &[(&str, &[u8], &str)] = &[
        ("big5", &[65, 136, 98], "A\u{ca}\u{304}"),
        ("shift_jis", &[65, 130, 160], "Aあ"),
        ("euc-jp", &[65, 143, 162, 175], "A\u{2d8}"),
        ("iso-2022-jp", &[27, 40, 74, 92, 65], "¥A"),
    ];
    for &(name, bytes, expected) in cases {
        for allowance in [0, bytes.len() - 1, bytes.len(), bytes.len() + 1] {
            engine.begin_job();engine.copy_bytes = allowance;
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
        engine.limits.capacity.string_bytes = original;engine.begin_job();
        assert_eq!(engine.decode_request_text(bytes, name).unwrap(), expected);
    }
}

#[test]
fn east_asian_retained_decoder_resource_refusal_resets_failed_output_but_preserves_precopy_state() {
    for (name, prefix, bytes, next, allowance) in [
        ("big5", "[136]", "[98,65]", "[65]", 3),
        ("iso-2022-jp", "[27,40,74]", "[92,65]", "[92]", 2),
    ] {
        let mut engine = runtime();
        let setup = format!("var d=new TextDecoder('{name}');var input=new Uint8Array({bytes});var next=new Uint8Array({next});var options={{stream:true}};d.decode(new Uint8Array({prefix}),options);d;");
        let Value::Object(decoder) = engine.execute(&setup, &mut NoHost).unwrap() else { panic!("native decoder object"); };
        let Value::Object(input) = engine.execute("input;", &mut NoHost).unwrap() else { panic!("native typed array"); };
        let Value::Object(next) = engine.execute("next;", &mut NoHost).unwrap() else { panic!("native typed array"); };
        let Value::Object(options) = engine.execute("options;", &mut NoHost).unwrap() else { panic!("ordinary options object"); };
        let retained = engine.value_bytes;
        let original = engine.limits.capacity.string_bytes;
        engine.limits.capacity.string_bytes = allowance;engine.begin_job();
        let error = engine.text_encoding_builtin(Op::Decode, &[Value::Object(input), Value::Object(options)], Value::Object(decoder), &mut NoHost).unwrap_err();
        assert!(error.display().contains("String limit"), "{name}");
        assert_eq!(engine.value_bytes, retained);
        engine.limits.capacity.string_bytes = original;engine.begin_job();
        let fresh = engine.text_encoding_builtin(Op::Decode, &[Value::Object(next)], Value::Object(decoder), &mut NoHost).unwrap();
        assert_eq!(fresh.to_text(), if name == "big5" { "A" } else { "\\" });

        // A refusal before copying input has not consumed the saved prefix or
        // completed designation. It leaves that previous native state intact.
        engine.execute(&format!("d.decode(new Uint8Array({prefix}),{{stream:true}});"), &mut NoHost).unwrap();
        engine.begin_job();engine.copy_bytes = 0;
        assert!(engine.text_encoding_builtin(Op::Decode, &[Value::Object(input)], Value::Object(decoder), &mut NoHost).is_err());
        engine.begin_job();
        let continued = engine.text_encoding_builtin(Op::Decode, &[Value::Object(input)], Value::Object(decoder), &mut NoHost).unwrap();
        assert_eq!(continued.to_text(), if name == "big5" { "\u{ca}\u{304}A" } else { "¥A" });
    }
}
