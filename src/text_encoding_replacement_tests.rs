//! Original replacement-decoder and API-boundary witnesses against pinned Encoding.
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

fn pinned_replacement_labels() -> Vec<String> {
    let source = format!(
        "var groups={};var labels=[],count=0;\
         for(var group of groups){{for(var encoding of group.encodings){{\
           if(encoding.name==='replacement'){{\
             count++;for(var label of encoding.labels)labels.push(label);\
           }}\
         }}}}\
         if(count!==1)throw 'replacement encoding inventory';labels.join('|');",
        include_str!("../tools/reference/text-encodings/encodings.json")
    );
    let labels: Vec<String> = runtime()
        .execute(&source, &mut NoHost)
        .unwrap()
        .to_text()
        .split('|')
        .map(str::to_owned)
        .collect();
    assert_eq!(labels, [
        "csiso2022kr", "hz-gb-2312", "iso-2022-cn", "iso-2022-cn-ext",
        "iso-2022-kr", "replacement",
    ]);
    labels
}

#[test]
fn replacement_all_six_pinned_labels_use_exact_encoding_normalization() {
    let mut engine = runtime();
    for label in pinned_replacement_labels() {
        for normalized in [label.clone(), label.to_ascii_uppercase(),
            format!("\t\n\u{c}\r {} \r\u{c}\n\t", label.to_ascii_uppercase())] {
            assert_eq!(Runtime::request_text_encoding_label(&normalized), Some("replacement"));
            engine.begin_job();
            assert_eq!(engine.decode_request_text(b"visible bytes", &normalized).unwrap(), "\u{fffd}");
        }
        for invalid in [format!("\u{b}{label}"), format!("{label}\u{b}"),
            format!("\u{a0}{label}"), format!("{label}\u{a0}"), format!("{label} extra")] {
            assert_eq!(Runtime::request_text_encoding_label(&invalid), None, "{invalid:?}");
            engine.begin_job();
            assert!(engine.decode_request_text(b"visible bytes", &invalid).is_err(), "{invalid:?}");
        }
    }
    assert_eq!(Runtime::request_text_encoding_label("\t UTF8 \r"), Some("utf-8"));
    assert_eq!(Runtime::request_text_encoding_label("\n ASCII \u{c}"), Some("windows-1252"));
    for unknown in ["", "not-an-encoding", "iso-2022-jp-2", "iso 2022 kr"] {
        assert_eq!(Runtime::request_text_encoding_label(unknown), None);
    }
}

#[test]
fn replacement_empty_and_nonempty_inputs_do_not_leak_source_bytes() {
    let mut engine = runtime();
    assert_eq!(engine.decode_request_text(&[], "replacement").unwrap(), "");
    for byte in 0u8..=255 {
        engine.begin_job();
        assert_eq!(engine.decode_request_text(&[byte], "replacement").unwrap(), "\u{fffd}", "byte {byte:02x}");
    }
    let cases: &[&[u8]] = &[
        b"ordinary ASCII", b"\0\0\0", b"<script>visible()</script>\"'&",
        b"\x1b$B$\"\x1b(B<", b"~{abcd~}\"<", b"\x80\xff\xc0\xaf\"<",
        b"\xef\xbb", b"\xfe", b"\xff", b"A\xef\xbb\xbfB",
    ];
    for bytes in cases {
        engine.begin_job();
        assert_eq!(engine.decode_request_text(bytes, "iso-2022-kr").unwrap(), "\u{fffd}", "{bytes:x?}");
    }
    engine.begin_job();
    assert_eq!(engine.decode_request_text(&vec![b'<'; 4096], "hz-gb-2312").unwrap(), "\u{fffd}");
}

#[test]
fn replacement_unicode_bom_override_strips_only_the_authoritative_bom() {
    let cases: &[(&[u8], &str)] = &[
        (&[0xef, 0xbb, 0xbf], ""),
        (&[0xff, 0xfe], ""),
        (&[0xfe, 0xff], ""),
        (&[0xef, 0xbb, 0xbf, 0xc3, 0xa9], "é"),
        (&[0xff, 0xfe, 0xe9, 0], "é"),
        (&[0xfe, 0xff, 0, 0xe9], "é"),
        (&[0xef, 0xbb, 0xbf, 0xef, 0xbb, 0xbf, b'A'], "\u{feff}A"),
        (&[0xff, 0xfe, 0xff, 0xfe, b'A', 0], "\u{feff}A"),
        (&[0xfe, 0xff, 0xfe, 0xff, 0, b'A'], "\u{feff}A"),
        (&[0xef, 0xbb, 0xbf, 0xff, b'<'], "\u{fffd}<"),
        (&[0xff, 0xfe, 0, 0xdc, b'A', 0], "\u{fffd}A"),
        (&[0xfe, 0xff, 0xdc, 0, 0, b'A'], "\u{fffd}A"),
    ];
    let mut engine = runtime();
    for label in pinned_replacement_labels() {
        for &(bytes, expected) in cases {
            engine.begin_job();
            assert_eq!(engine.decode_request_text(bytes, &label).unwrap(), expected, "{label}: {bytes:x?}");
        }
    }
    for bytes in [&[0xef][..], &[0xef, 0xbb], &[0xfe], &[0xff],
        &[b'A', 0xff, 0xfe, b'B', 0], &[0, 0xfe, 0xff, 0, b'A']] {
        engine.begin_job();
        assert_eq!(engine.decode_request_text(bytes, "replacement").unwrap(), "\u{fffd}");
    }
}

#[test]
fn replacement_textdecoder_rejects_every_recognized_alias() {
    let source = format!(r#"
var labels='{}'.split('|');
function range(f){{try{{f();}}catch(e){{if(e.name==='RangeError')return;throw e;}}throw 'missing RangeError';}}
for(var label of labels){{
  for(var fatal of [false,true]){{for(var bom of [false,true]){{
    range(function(){{new TextDecoder(label,{{fatal:fatal,ignoreBOM:bom}});}});
    range(function(){{new TextDecoder(' \t'+label.toUpperCase()+'\n\r\f ',{{fatal:fatal,ignoreBOM:bom}});}});
  }}}}
}}
var d=new TextDecoder();
if(d.encoding!=='utf-8'||d.decode(new Uint8Array([65]))!=='A')throw 'subsequent UTF-8 constructor';
'ok';
"#, pinned_replacement_labels().join("|"));
    succeeds(&source);
}

#[test]
fn replacement_constructor_conversion_order_reentry_and_abrupt_completion() {
    succeeds(r#"
var log=[],label={toString:function(){log.push('label');return 'ISO-2022-CN';}},options={
  get fatal(){log.push('fatal');if(new TextDecoder('utf-8').decode(new Uint8Array([65]))!=='A')throw 'fatal reentry';return true;},
  get ignoreBOM(){log.push('ignoreBOM');if(new TextDecoder('big5').decode(new Uint8Array([164,164]))!=='\u4e2d')throw 'ignoreBOM reentry';return true;}
};
var returned='unchanged';
try{returned=new TextDecoder(label,options);throw 'missing replacement rejection';}catch(e){if(e.name!=='RangeError')throw e;}
if(returned!=='unchanged'||log.join(',')!=='label,fatal,ignoreBOM')throw 'ordered replacement conversion';
var marker={};log=[];
label.toString=function(){log.push('label');throw marker;};
try{new TextDecoder(label,options);throw 'missing label failure';}catch(e){if(e!==marker)throw e;}
if(log.join(',')!=='label')throw 'options after abrupt label';
label.toString=function(){log.push('label');return 'replacement';};
log=[];options={get fatal(){log.push('fatal');throw marker;},get ignoreBOM(){log.push('ignoreBOM');return false;}};
try{new TextDecoder(label,options);throw 'missing fatal getter failure';}catch(e){if(e!==marker)throw e;}
if(log.join(',')!=='label,fatal')throw 'ignoreBOM after abrupt fatal';
log=[];options={get fatal(){log.push('fatal');return false;},get ignoreBOM(){log.push('ignoreBOM');throw marker;}};
try{new TextDecoder(label,options);throw 'missing ignoreBOM getter failure';}catch(e){if(e!==marker)throw e;}
if(log.join(',')!=='label,fatal,ignoreBOM')throw 'abrupt ignoreBOM precedence';
try{new TextDecoder('replacement',1);throw 'missing invalid dictionary rejection';}catch(e){if(e.name!=='TypeError')throw e;}
if(new TextDecoder().decode(new Uint8Array([65]))!=='A')throw 'constructor recovery';
'ok';
"#);
}

#[test]
fn replacement_work_and_output_refusal_preserves_accounting_and_recovers() {
    let mut engine = runtime();
    let retained = engine.value_bytes;
    let capacity = engine.limits.capacity.string_bytes;
    let bytes = vec![b'<'; 4096];
    for allowance in [0, bytes.len() - 1, bytes.len(), bytes.len() + 2] {
        engine.begin_job();
        engine.copy_bytes = allowance;
        let error = engine.decode_request_text(&bytes, "replacement").unwrap_err();
        assert!(error.contains("work limit"), "allowance {allowance}: {error}");
        assert_eq!(engine.value_bytes, retained);
        engine.begin_job();
        assert_eq!(engine.decode_request_text(&bytes, "replacement").unwrap(), "\u{fffd}");
    }
    for allowance in [0, 1, 2] {
        engine.limits.capacity.string_bytes = allowance;
        engine.begin_job();
        let error = engine.decode_request_text(&bytes, "replacement").unwrap_err();
        assert!(error.contains("String limit"), "allowance {allowance}: {error}");
        assert_eq!(engine.value_bytes, retained);
        engine.begin_job();
        assert_eq!(engine.decode_request_text(&[], "replacement").unwrap(), "");
    }
    engine.limits.capacity.string_bytes = 3;
    engine.begin_job();
    assert_eq!(engine.decode_request_text(&bytes, "replacement").unwrap(), "\u{fffd}");
    assert_eq!(engine.value_bytes, retained);
    engine.limits.capacity.string_bytes = capacity;
    engine.begin_job();
    assert_eq!(engine.decode_request_text(b"recovered", "utf-8").unwrap(), "recovered");
}

#[test]
fn replacement_request_instances_and_existing_decoders_keep_independent_state() {
    let mut engine = runtime();
    assert_eq!(engine.execute(r#"
var d=new TextDecoder('utf-8');
if(d.decode(new Uint8Array([226]),{stream:true})!=='')throw 'UTF-8 pending prefix';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
    for label in pinned_replacement_labels() {
        engine.begin_job();
        assert_eq!(engine.decode_request_text(b"first", &label).unwrap(), "\u{fffd}");
        assert_eq!(engine.decode_request_text(&[], &label).unwrap(), "");
        assert_eq!(engine.decode_request_text(b"second", &label).unwrap(), "\u{fffd}");
        assert_eq!(engine.decode_request_text(&[0xc0], "windows-1251").unwrap(), "А");
        assert_eq!(engine.decode_request_text(b"third", &label).unwrap(), "\u{fffd}");
    }
    assert_eq!(engine.execute(r#"
if(d.decode(new Uint8Array([130,172]))!=='\u20ac')throw 'replacement request changed public decoder';
if(d.decode(new Uint8Array([65]))!=='A')throw 'public decoder recovery';
'ok';
"#, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn replacement_fetch_body_decoding_remains_utf8() {
    let mut engine = runtime();
    let payload = b"\x1b$B$\"\x1b(B<script>text</script>\"&";
    engine.begin_job();
    assert_eq!(engine.decode_request_text(payload, "replacement").unwrap(), "\u{fffd}");
    assert_eq!(engine.decode_body_utf8(payload).unwrap().as_bytes(), payload);
    for (bytes, expected) in [
        (&[0xff, b'A'][..], "\u{fffd}A"),
        (&[0xef, 0xbb, 0xbf, 0xc3, 0xa9][..], "é"),
        (&[0xef, 0xbb, 0xbf, 0xef, 0xbb, 0xbf, b'A'][..], "\u{feff}A"),
        (&[0xff, 0xfe, b'A', 0][..], "\u{fffd}\u{fffd}A\0"),
        (&[][..], ""),
    ] {
        engine.begin_job();
        assert_eq!(engine.decode_body_utf8(bytes).unwrap(), expected);
    }
}

#[test]
fn replacement_incremental_core_emits_once_per_decoder() {
    let decoder = || Decoder::new(Encoding::REPLACEMENT, false, false);
    let mut engine = runtime();
    let bytes = b"\x1b$B\"<script>data</script>\xff";
    for split in 0..=bytes.len() {
        engine.begin_job();
        let mut state = decoder();
        assert_eq!(engine.decode_text_bytes(&mut state, &[], true).unwrap(), "");
        assert!(!state.replacement_error_returned);
        let mut output = engine.decode_text_bytes(&mut state, &bytes[..split], true).unwrap();
        assert_eq!(engine.decode_text_bytes(&mut state, &[], true).unwrap(), "");
        output.push_str(&engine.decode_text_bytes(&mut state, &bytes[split..], true).unwrap());
        output.push_str(&engine.decode_text_bytes(&mut state, &[], false).unwrap());
        assert_eq!(output, "\u{fffd}", "split {split}");
        assert!(state.replacement_error_returned);
        assert_eq!(state.pending_len, 0);
        assert_eq!(engine.decode_text_bytes(&mut state, b"later bytes", true).unwrap(), "");
    }
    engine.begin_job();
    let mut state = decoder();
    assert_eq!(engine.decode_text_bytes(&mut state, &[], false).unwrap(), "");
    assert!(!state.replacement_error_returned);
    let mut output = String::new();
    for byte in bytes {
        output.push_str(&engine.decode_text_bytes(&mut state, &[*byte], true).unwrap());
    }
    assert_eq!(output, "\u{fffd}");
    assert_eq!(engine.decode_text_bytes(&mut state, &[], false).unwrap(), "");
    let mut independent = decoder();
    assert_eq!(engine.decode_text_bytes(&mut independent, b"fresh bytes", false).unwrap(), "\u{fffd}");
}
