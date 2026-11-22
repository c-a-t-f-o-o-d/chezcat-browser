//! Original API and resource witnesses against the pinned WHATWG index data.
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

macro_rules! index {
    ($name:literal, $file:literal) => {
        (
            $name,
            include_str!(concat!("../tools/reference/text-encodings/", $file)),
        )
    };
}

// The oracle reads the attributed source data, independently of the generated
// Rust table or its enum/label implementation. These are encoding names, not
// a declaration that document charset selection is implemented.
const INDEXES: &[(&str, &str)] = &[
    index!("ibm866", "index-ibm866.txt"),
    index!("iso-8859-2", "index-iso-8859-2.txt"),
    index!("iso-8859-3", "index-iso-8859-3.txt"),
    index!("iso-8859-4", "index-iso-8859-4.txt"),
    index!("iso-8859-5", "index-iso-8859-5.txt"),
    index!("iso-8859-6", "index-iso-8859-6.txt"),
    index!("iso-8859-7", "index-iso-8859-7.txt"),
    index!("iso-8859-8", "index-iso-8859-8.txt"),
    index!("iso-8859-8-i", "index-iso-8859-8.txt"),
    index!("iso-8859-10", "index-iso-8859-10.txt"),
    index!("iso-8859-13", "index-iso-8859-13.txt"),
    index!("iso-8859-14", "index-iso-8859-14.txt"),
    index!("iso-8859-15", "index-iso-8859-15.txt"),
    index!("iso-8859-16", "index-iso-8859-16.txt"),
    index!("koi8-r", "index-koi8-r.txt"),
    index!("koi8-u", "index-koi8-u.txt"),
    index!("macintosh", "index-macintosh.txt"),
    index!("windows-874", "index-windows-874.txt"),
    index!("windows-1250", "index-windows-1250.txt"),
    index!("windows-1251", "index-windows-1251.txt"),
    index!("windows-1252", "index-windows-1252.txt"),
    index!("windows-1253", "index-windows-1253.txt"),
    index!("windows-1254", "index-windows-1254.txt"),
    index!("windows-1255", "index-windows-1255.txt"),
    index!("windows-1256", "index-windows-1256.txt"),
    index!("windows-1257", "index-windows-1257.txt"),
    index!("windows-1258", "index-windows-1258.txt"),
    index!("x-mac-cyrillic", "index-x-mac-cyrillic.txt"),
];

fn expected_index(input: &str) -> [Option<char>; 128] {
    let mut expected = [None; 128];
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let pointer = fields.next().unwrap().parse::<usize>().unwrap();
        let scalar = fields.next().unwrap().strip_prefix("0x").unwrap();
        let value = char::from_u32(u32::from_str_radix(scalar, 16).unwrap()).unwrap();
        assert!(pointer < expected.len(), "invalid normative pointer");
        assert!(
            expected[pointer].replace(value).is_none(),
            "duplicate normative pointer"
        );
    }
    expected
}

fn expected_bytes(index: &[Option<char>; 128], bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            if byte < 128 {
                char::from(byte)
            } else {
                index[usize::from(byte - 128)].unwrap_or('\u{fffd}')
            }
        })
        .collect()
}

#[test]
fn single_byte_all_names_and_all_bytes_match_pinned_normative_indexes() {
    assert_eq!(INDEXES.len(), 28);
    let bytes: Vec<u8> = (0..=255).collect();
    let mut engine = runtime();
    for &(name, source) in INDEXES {
        let expected = expected_bytes(&expected_index(source), &bytes);
        engine.begin_job();
        let script = format!(
            "var a=new Uint8Array(256);for(var i=0;i<256;i++)a[i]=i;\
             var d=new TextDecoder('{name}');\
             if(d.encoding!=='{name}'||d.fatal||d.ignoreBOM)throw 'decoder metadata';\
             var full=d.decode(a),s=new TextDecoder('{name}');\
             var split=s.decode(a.subarray(0,128),{{stream:true}})+\
                 s.decode(a.subarray(128),{{stream:true}})+s.decode();\
             if(full!==split||s.decode()!=='')throw 'stream or flush changed single-byte data';full;"
        );
        assert_eq!(
            engine.execute(&script, &mut NoHost).unwrap().to_text(),
            expected,
            "{name}"
        );
    }
}

#[test]
fn single_byte_every_pinned_label_has_the_correct_distinct_canonical_name() {
    let source = format!(
        "var groups={};var count=0;\
         for(var group of groups){{\
           if(group.heading!=='Legacy single-byte encodings')continue;\
           for(var encoding of group.encodings){{\
             count++;for(var label of encoding.labels){{\
               var d=new TextDecoder(' \\t'+label.toUpperCase()+'\\n\\r\\f ');\
               if(d.encoding!==encoding.name.toLowerCase())throw 'wrong canonical label '+label;\
             }}\
           }}\
         }}\
         if(count!==28)throw 'incomplete normative family';\
         if(new TextDecoder('logical').encoding!=='iso-8859-8-i'||\
            new TextDecoder('visual').encoding!=='iso-8859-8')throw 'distinct Hebrew names';'ok';",
        include_str!("../tools/reference/text-encodings/encodings.json")
    );
    succeeds(&source);
}

#[test]
fn single_byte_undefined_mappings_replace_or_throw_without_masking_ascii() {
    let mut families_with_holes = 0;
    for &(name, source) in INDEXES {
        let index = expected_index(source);
        let Some(hole) = index.iter().position(Option::is_none) else {
            continue;
        };
        families_with_holes += 1;
        succeeds(&format!(
            r#"
var a=new Uint8Array([66,{byte},34,67]);
if(new TextDecoder('{name}').decode(a)!=='B\ufffd"C')throw 'replacement or delimiter';
var d=new TextDecoder('{name}',{{fatal:true}}),returned='unchanged',caught=false;
try{{returned=d.decode(a,{{stream:true}});}}catch(e){{if(e.name!=='TypeError')throw e;caught=true;}}
if(!caught||returned!=='unchanged'||!d.fatal)throw 'fatal output';
if(d.decode(new Uint8Array([65]),{{stream:true}})!=='A'||d.decode()!=='')throw 'fatal recovery';
'ok';
"#,
            byte = hole + 128
        ));
    }
    assert!(
        families_with_holes > 0,
        "the undefined-pointer path must be exercised"
    );
}

#[test]
fn single_byte_bom_bytes_are_data_while_unicode_bom_behavior_is_retained() {
    for &(name, source) in INDEXES {
        let expected = expected_bytes(&expected_index(source), &[0xef, 0xbb, 0xbf, 65]);
        let script = format!(
            "var a=new Uint8Array([239,187,191,65]),d=new TextDecoder('{name}');\
             var output=d.decode(a);\
             if(new TextDecoder('{name}',{{ignoreBOM:true}}).decode(a)!==output)throw 'legacy BOM option';\
             if(d.decode(a.subarray(0,1),{{stream:true}})+d.decode(a.subarray(1))!==output)throw 'legacy BOM stream';output;"
        );
        assert_eq!(
            runtime().execute(&script, &mut NoHost).unwrap().to_text(),
            expected,
            "{name}"
        );
    }
    succeeds(r#"
for(var pair of [['utf-8',[239,187,191,65]],['utf-16le',[255,254,65,0]],['utf-16be',[254,255,0,65]]]){
  var a=new Uint8Array(pair[1]),d=new TextDecoder(pair[0]);
  if(d.decode(a)!=='A'||d.decode(a)!=='A')throw 'Unicode BOM reset';
  if(new TextDecoder(pair[0],{ignoreBOM:true}).decode(a)!=='\ufeffA')throw 'Unicode BOM retention';
  if(d.decode(a.subarray(0,1),{stream:true})+d.decode(a.subarray(1))!=='A')throw 'split Unicode BOM';
}
'ok';
"#);
}

#[test]
fn single_byte_x_user_defined_preserves_ascii_and_maps_every_high_byte() {
    succeeds(r#"
var a=new Uint8Array(256);for(var i=0;i<256;i++)a[i]=i;
var d=new TextDecoder(' X-USER-DEFINED ',{fatal:true}),s=d.decode(a);
if(d.encoding!=='x-user-defined'||!d.fatal||s.length!==256)throw 'x-user-defined metadata';
for(var i=0;i<256;i++)if(s.charCodeAt(i)!==(i<128?i:0xf700+i))throw 'private-use mapping '+i;
if(d.decode(a.subarray(0,129),{stream:true})+d.decode(a.subarray(129))+d.decode()!==s)throw 'private-use streaming';
var bom=new Uint8Array([239,187,191]);
if(d.decode(bom).length!==3||d.decode(bom)!==new TextDecoder('x-user-defined',{ignoreBOM:true}).decode(bom))throw 'private-use BOM bytes';
'ok';
"#);
}

#[test]
fn single_byte_label_validation_options_order_and_native_brands_remain_real() {
    succeeds(r#"
function rejects(f,name){try{f();}catch(e){if(e.name===name)return;throw e;}throw 'missing '+name;}
for(var label of ['',null,'unknown','replacement','iso-2022-cn','hz-gb-2312','\vwindows-1251','windows-1251\u00a0']){
  rejects(function(){new TextDecoder(label);},'RangeError');
}
var log=[],label={toString:function(){log.push('label');return 'cp1251';}},options={
  get fatal(){log.push('fatal');return false;},
  get ignoreBOM(){log.push('ignoreBOM');if(new TextDecoder('koi8-r').decode(new Uint8Array([65]))!=='A')throw 'reentry';return true;}
};
var d=new TextDecoder(label,options);
if(log.join(',')!=='label,fatal,ignoreBOM'||d.encoding!=='windows-1251'||d.fatal||!d.ignoreBOM)throw 'ordered options';
log=[];label.toString=function(){log.push('label');return 'unknown';};
rejects(function(){new TextDecoder(label,options);},'RangeError');
if(log.join(',')!=='label,fatal,ignoreBOM')throw 'invalid-label dictionary order';
log=[];var marker={};label.toString=function(){throw marker;};
try{new TextDecoder(label,options);throw 'missing abrupt conversion';}catch(e){if(e!==marker)throw e;}
if(log.length!==0)throw 'options after abrupt label';
rejects(function(){TextDecoder('windows-1251');},'TypeError');
rejects(function(){TextDecoder.prototype.decode.call({},new Uint8Array([65]));},'TypeError');
rejects(function(){Object.getOwnPropertyDescriptor(TextDecoder.prototype,'encoding').get.call({});},'TypeError');
rejects(function(){new TextDecoder('windows-1251',1);},'TypeError');
rejects(function(){d.decode(null);},'TypeError');
if(d.decode()!==''||d.decode(undefined)!=='')throw 'empty input';
'ok';
"#);
}

#[test]
fn single_byte_buffer_views_and_option_reentry_use_real_backing() {
    succeeds(r#"
var bytes=new Uint8Array([0,192,224,0]),d=new TextDecoder('windows-1251');
if(d.decode(bytes.subarray(1,3))!=='\u0410\u0430'||d.decode(new DataView(bytes.buffer,1,2))!=='\u0410\u0430')throw 'view extent';
var seen=0,options={get stream(){seen++;bytes[1]=193;if(d.decode(new Uint8Array([65]))!=='A')throw 'nested decoder';return true;}};
if(d.decode(bytes.subarray(1,3),options)!=='\u0411\u0430'||seen!==1||d.decode()!=='')throw 'conversion and input snapshot order';
if(bytes[0]!==0||bytes[3]!==0)throw 'input write';
'ok';
"#);
}

#[test]
fn single_byte_request_text_uses_the_same_indexes_and_retains_bom_override() {
    let bytes: Vec<u8> = (0..=255).collect();
    let mut engine = runtime();
    for &(name, source) in INDEXES {
        engine.begin_job();
        assert_eq!(
            engine.decode_request_text(&bytes, name).unwrap(),
            expected_bytes(&expected_index(source), &bytes),
            "{name}"
        );
    }
    engine.begin_job();
    assert_eq!(
        engine
            .decode_request_text(&[0xef, 0xbb, 0xbf, 0xc3, 0xa9], "windows-1251")
            .unwrap(),
        "é"
    );
    assert_eq!(
        engine
            .decode_request_text(&[0xff, 0xfe, 65, 0], "koi8-r")
            .unwrap(),
        "A"
    );
    assert_eq!(
        engine
            .decode_request_text(&[0xfe, 0xff, 0, 65], "ibm866")
            .unwrap(),
        "A"
    );
    assert_eq!(
        engine
            .decode_body_utf8(&[0xef, 0xbb, 0xbf, 0xc3, 0xa9])
            .unwrap(),
        "é"
    );
    assert_eq!(engine.decode_request_text(&[65], "replacement").unwrap(), "\u{fffd}");
}

#[test]
fn single_byte_work_and_output_refusal_do_not_publish_partial_strings_and_recover() {
    let mut engine = runtime();
    let retained = engine.value_bytes;
    for allowance in [0, 1, 2] {
        engine.begin_job();
        engine.copy_bytes = allowance;
        assert!(
            engine.decode_request_text(&[0x80], "windows-1252").is_err(),
            "copy allowance {allowance}"
        );
        assert_eq!(engine.value_bytes, retained);
        engine.begin_job();
        assert_eq!(
            engine.decode_request_text(&[0x80], "windows-1252").unwrap(),
            "€"
        );
    }
    let original = engine.limits.capacity.string_bytes;
    engine.limits.capacity.string_bytes = 1;
    engine.begin_job();
    assert!(engine.decode_request_text(&[65, 0x80], "windows-1252").is_err());
    assert_eq!(engine.value_bytes, retained);
    engine.limits.capacity.string_bytes = original;
    engine.begin_job();
    assert_eq!(
        engine.decode_request_text(&[65, 0x80], "windows-1252").unwrap(),
        "A€"
    );
}
