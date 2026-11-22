//! Original real request state, native notifications, conversions and owners.
use super::*;
fn fixture(source: &str) -> (Document, Forms, Page) {
    let mut doc =
        crate::html::parse("<html><head></head><body><p id=out>original</p></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://xhr-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(
        page.start(
            &mut doc,
            &mut forms,
            &[Script {
                node: 0,
                label: "Original native XMLHttpRequest".into(),
                source: source.into()
            }]
        )
        .errors,
        0,
        "{}",
        page.report()
    );
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    assert_eq!(
        page.console_eval(doc, forms, source).errors,
        0,
        "{}",
        page.report()
    );
}
#[test]
fn xhr_constructor_real_native_state_upload_descriptors_and_saved_roots() {
    fixture(
        r#"
var C=XMLHttpRequest,P=C.prototype,r=new C(),u=r.upload;
if(C.name!=='XMLHttpRequest'||C.length!==0||Object.getPrototypeOf(C)!==XMLHttpRequestEventTarget||Object.getPrototypeOf(P)!==XMLHttpRequestEventTarget.prototype||Object.getPrototypeOf(XMLHttpRequestEventTarget.prototype)!==EventTarget.prototype||Object.getPrototypeOf(r)!==P||r.constructor!==C||!(r instanceof EventTarget)||!(u instanceof XMLHttpRequestUpload)||!(u instanceof XMLHttpRequestEventTarget)||u instanceof C||r.upload!==u)throw 'genuine request ancestry';
if(Object.prototype.toString.call(r)!=='[object XMLHttpRequest]'||Object.prototype.toString.call(u)!=='[object XMLHttpRequestUpload]'||'fetch' in P||r.fetch!==undefined)throw 'real class/optional member';
if(r.readyState!==0||r.status!==0||r.statusText!==''||r.responseURL!==''||r.response!==''||r.responseText!==''||r.responseXML!==null||r.timeout!==0||r.withCredentials||r.responseType!==''||r.onload!==null)throw 'real request defaults';
for(var name of ['readyState','status','statusText','responseURL','responseText','response','upload']){var d=Object.getOwnPropertyDescriptor(P,name);if(!d||typeof d.get!=='function'||d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||Object.hasOwn(r,name))throw 'request readonly descriptor '+name;}
for(var pair of [['UNSENT',0],['OPENED',1],['HEADERS_RECEIVED',2],['LOADING',3],['DONE',4]]){var d=Object.getOwnPropertyDescriptor(P,pair[0]);if(C[pair[0]]!==pair[1]||r[pair[0]]!==pair[1]||d.writable||d.configurable)throw 'request constants';}
var open=P.open;r.note='ordinary';if(r.note!=='ordinary'||Object.keys(r).join(',')!=='note')throw 'request expando';delete P.open;if(r.open!==undefined||'open' in r)throw 'request ghost operation';P.open=open;
try{C();throw 'request constructor without new';}catch(e){if(e.name!=='TypeError')throw e;}for(var B of [XMLHttpRequestEventTarget,XMLHttpRequestUpload]){try{new B();throw 'illegal base constructor';}catch(e){if(e.name!=='TypeError')throw e;}}
delete window.XMLHttpRequest;var saved=new C();if(saved.constructor!==C||saved.readyState!==0)throw 'saved request constructor';
"#,
    );
}
#[test]
fn xhr_open_is_synchronous_branded_reentrant_and_preserves_actual_final_generation() {
    let (_, _, page) = fixture(
        r#"
var r=new XMLHttpRequest(),trace='',events=0;
r.onreadystatechange=function(e){events++;if(!(e instanceof Event)||e instanceof ProgressEvent||e instanceof UIEvent||!e.isTrusted||e.target!==r||e.currentTarget!==r||e.eventPhase!==2||r.readyState!==1)throw 'native open event';trace+='E';};
r.open({toString:function(){trace+='M';return 'POST';}},{toString:function(){trace+='U';return '/native#fragment';}},true);if(trace!=='MUE'||events!==1||r.readyState!==1)throw 'synchronous open conversion/event';
var untouched=0,arg={toString:function(){untouched++;return 'GET';}};try{r.open.call({},arg,arg);throw 'bad receiver';}catch(e){if(e.name!=='TypeError')throw e;}if(untouched)throw 'brand before conversion';
r.open('GET','/again');if(events!==1)throw 'duplicate OPENED event';r=new XMLHttpRequest();var reentered=false;r.onreadystatechange=function(){if(!reentered){reentered=true;r.open('GET','/inner');}};r.open('POST','/outer');if(!reentered||r.readyState!==1)throw 'native reentrant open';
var prior=r.readyState;try{r.open('TRACE','/never');throw 'forbidden method';}catch(e){if(!(e instanceof DOMException)||e.name!=='SecurityError')throw e;}if(r.readyState!==prior)throw 'open refusal changed state';
try{r.open('GET','/never',false);throw 'synchronous unsupported';}catch(e){if(!(e instanceof DOMException)||e.name!=='NotSupportedError')throw e;}
"#,
    );
    assert_eq!(
        page.state.xhr.records[1].url.as_ref().unwrap().path,
        "/inner"
    );
    assert_eq!(
        page.state.xhr.records[1].outgoing.method,
        crate::http::ScriptMethod::Get
    );
    assert!(page.state.synthetic_dispatches.is_empty());
}
#[test]
fn xhr_send_native_progress_binary_snapshot_and_abort_do_not_resurrect_queued_work() {
    let (mut doc, mut forms, mut page) = fixture(
        r#"
var r=new XMLHttpRequest(),saved,log='',bytes=new Uint8Array([9,65,66,8]),view=new Uint8Array(bytes.buffer,1,2);
r.onloadstart=function(e){saved=e;log+='start;';if(!(e instanceof ProgressEvent)||e instanceof UIEvent||!e.isTrusted||e.target!==r||e.loaded||e.total||e.lengthComputable||r.readyState!==1)throw 'native loadstart';};
r.upload.onloadstart=function(e){log+='upload;';if(!(e instanceof ProgressEvent)||!e.isTrusted||e.target!==r.upload)throw 'native upload';};
r.open('POST','/bytes');r.send(view);bytes[1]=90;if(log!=='start;upload;'||r.readyState!==1)throw 'send queues real work';
"#,
    );
    assert_eq!(page.state.xhr.pending.len(), 1);
    let request = page.state.xhr.pending.front().unwrap().clone();
    assert_eq!(request.outgoing.body, b"AB");
    assert!(request.force_preflight);
    assert!(!request
        .outgoing
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type")));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
r.onreadystatechange=function(e){if(r.readyState===4){if(!e.isTrusted||r.status!==0)throw 'native abort ready';log+='ready;';}};
r.onabort=function(e){if(!(e instanceof ProgressEvent)||!e.isTrusted||r.readyState!==4)throw 'native abort';log+='abort;';};r.onloadend=function(e){log+='end;';};r.abort();if(r.readyState!==0||log!=='start;upload;ready;abort;end;')throw log;
r.onloadstart=null;var got=false;r.addEventListener('loadstart',function(e){if(e===saved){got=true;if(e.isTrusted)throw 'redispatch trust';}});r.dispatchEvent(saved);if(!got)throw 'saved native event identity';
"#,
    );
    assert!(request.cancel.load(std::sync::atomic::Ordering::Acquire));
    assert!(!page.state.xhr.records[0].sent);
    assert!(page.state.synthetic_dispatches.is_empty());
}
#[test]
fn xhr_header_body_and_setter_conversion_failures_leave_native_request_state_real() {
    let (mut doc, mut forms, mut page) = fixture(
        r#"
var r=new XMLHttpRequest(),converted=0;r.open('POST','/text');r.setRequestHeader('X-Original',' first ');r.setRequestHeader('x-original',{toString:function(){converted++;return 'second';}});r.setRequestHeader('Cookie','forbidden=not-sent');
try{r.setRequestHeader('bad name','value');throw 'invalid header';}catch(e){if(!(e instanceof DOMException)||e.name!=='SyntaxError')throw e;}
r.timeout={valueOf:function(){converted++;return 4294967297;}};r.withCredentials=true;if(r.timeout!==1||!r.withCredentials)throw 'typed setters';
try{r.responseType='arraybuffer';throw 'unsupported response';}catch(e){if(!(e instanceof DOMException)||e.name!=='NotSupportedError')throw e;}if(r.responseType!=='')throw 'refused response type changed';
r.timeout=0;r.send({toString:function(){converted++;return 'a\uD800b';}});if(converted!==3||r.readyState!==1)throw 'body conversion';
"#,
    );
    let request = page.state.xhr.pending.front().unwrap();
    assert_eq!(
        request.outgoing.body,
        "a\u{fffd}b".replace("\\u{fffd}", "\u{fffd}").as_bytes()
    );
    assert_eq!(request.credentials, crate::http::ScriptCredentials::Include);
    assert_eq!(
        request
            .outgoing
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("x-original"))
            .unwrap()
            .1,
        "first, second"
    );
    assert!(!request
        .outgoing
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("cookie")));
    run(&mut doc,&mut forms,&mut page,"try{r.setRequestHeader('X-Late','denied');throw 'late header';}catch(e){if(!(e instanceof DOMException)||e.name!=='InvalidStateError')throw e;}r.abort();");
}
#[test]
fn progress_event_genuine_constructor_defaults_brand_and_unsigned_fields() {
    fixture(
        r#"
var C=ProgressEvent,P=C.prototype,e=new C('progress',{lengthComputable:true,loaded:4294967297,total:18446744073709551616,bubbles:true});
if(Object.getPrototypeOf(C)!==Event||Object.getPrototypeOf(P)!==Event.prototype||!(e instanceof Event)||e instanceof UIEvent||e.isTrusted||!e.lengthComputable||e.loaded!==4294967297||e.total!==0||!e.bubbles||Object.prototype.toString.call(e)!=='[object ProgressEvent]')throw 'genuine progress';
var get=Object.getOwnPropertyDescriptor(P,'loaded').get;for(var value of [{},new Event('base'),new UIEvent('ui'),Object.create(P)]){try{get.call(value);throw 'progress fake';}catch(e){if(e.name!=='TypeError')throw e;}}
var zero=new C('z');if(zero.loaded||zero.total||zero.lengthComputable)throw 'progress defaults';var negative=new C('n',{loaded:-1,total:Infinity});if(negative.loaded!==18446744073709551616||negative.total)throw 'progress unsigned conversion';
"#,
    );
}

#[test]
fn xhr_owner_realms_saved_getters_and_child_network_refusal_are_genuine() {
    let (mut doc, mut forms, mut page) = fixture(
        r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,C=w.XMLHttpRequest,r=new C(),main=new XMLHttpRequest();
if(C===XMLHttpRequest||Object.getPrototypeOf(C)!==w.XMLHttpRequestEventTarget||Object.getPrototypeOf(r)!==C.prototype||r instanceof XMLHttpRequest||!(r instanceof w.EventTarget)||!(r.upload instanceof w.XMLHttpRequestUpload)||r.readyState!==0)throw 'genuine owner realm';
var get=Object.getOwnPropertyDescriptor(C.prototype,'readyState').get;if(get.call(main)!==0)throw 'same origin borrowed brand';
r.open('GET','/child');try{r.send();throw 'unimplemented child request fabricated';}catch(e){if(!(e instanceof w.DOMException)||e.name!=='NotSupportedError')throw e;}if(r.readyState!==1||r.status)throw 'refused child request changed';
"#,
    );
    assert!(page.state.xhr.pending.is_empty());
    page.set_location(
        &crate::navigation::address(
            "https://other-xhr.test/",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert!(page
        .runtime
        .execute(
            "new C();",
            &mut Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state
            }
        )
        .unwrap_err()
        .contains("Foreign Window access"));
}
#[test]
fn xhr_native_copy_and_snapshot_storage_refusal_preserve_old_state_and_recover() {
    let (mut doc, mut forms, mut page) = fixture(
        "var r=new XMLHttpRequest();r.open('POST','/old');r.setRequestHeader('X-Original','kept');",
    );
    let baseline = page.state.retained;
    let generation = page.state.xhr.records[0].generation;
    let url = page.state.xhr.records[0].url.clone();
    let limit = page.state.limits.capacity.script_storage_bytes;
    page.begin_job();
    page.state.copy_work = 0;
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    assert!(bridge
        .xhr_call(
            0x8e000000,
            "\0XHR.call.open",
            &[
                Value::String("GET".into()),
                Value::String("/refused".into())
            ],
            None
        )
        .is_err());
    assert_eq!(
        (
            page.state.retained,
            page.state.xhr.records[0].generation,
            page.state.xhr.records[0].url.clone()
        ),
        (baseline, generation, url)
    );
    assert_eq!(page.state.xhr.records[0].ready, 1);
    assert!(page.state.synthetic_dispatches.is_empty());
    page.begin_job();
    page.state.limits.capacity.script_storage_bytes = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .xhr_call(
        0x8e000000,
        "\0XHR.call.send",
        &[Value::String("native body".into())],
        None
    )
    .is_err());
    assert!(!page.state.xhr.records[0].sent);
    assert!(page.state.xhr.pending.is_empty());
    assert_eq!(
        page.state
            .xhr
            .used
            .load(std::sync::atomic::Ordering::Acquire),
        0
    );
    assert_eq!(page.state.retained, baseline);
    page.state.limits.capacity.script_storage_bytes = limit;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "r.send('recovered');if(r.readyState!==1)throw 'request recovery';",
    );
    assert_eq!(
        page.state.xhr.pending.front().unwrap().outgoing.body,
        b"recovered"
    );
}
#[test]
fn xhr_reentrant_body_conversion_and_mime_charset_use_the_actual_current_request() {
    let (_, _, page) = fixture(
        r#"
var r=new XMLHttpRequest(),calls=0;r.overrideMimeType('text/plain; charset="windows-1252"');r.open('POST','/outer');r.send({toString:function(){calls++;r.open('POST','/inner');r.setRequestHeader('Content-Type','application/json; charset="windows-1252"');return 'actual';}});if(calls!==1)throw 'body conversion count';
"#,
    );
    let request = page.state.xhr.pending.front().unwrap();
    assert_eq!(request.target.path, "/inner");
    assert_eq!(request.outgoing.body, b"actual");
    assert_eq!(
        request
            .outgoing
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .unwrap()
            .1,
        "application/json;charset=UTF-8"
    );
    assert_eq!(
        page.state.xhr.records[0].mime,
        "text/plain;charset=windows-1252"
    );
}

fn charset_js_string(value: &str) -> String {
    use std::fmt::Write;
    let mut source = String::from("\"");
    for unit in value.encode_utf16() {
        write!(&mut source, "\\u{unit:04x}").unwrap();
    }
    source.push('"');
    source
}

fn start_charset_request(
    doc: &mut Document,
    forms: &mut Forms,
    page: &mut Page,
    override_mime: Option<&str>,
) {
    let override_source = override_mime
        .map(|mime| format!("r.overrideMimeType({});", charset_js_string(mime)))
        .unwrap_or_default();
    run(doc, forms, page, &format!(r#"
var r=new XMLHttpRequest(),charsetTrace='',charsetLoads=0,charsetErrors=0,charsetEnds=0,charsetLoaded=-1,charsetTotal=-1,charsetComputable=false;
r.onreadystatechange=function(e){{if(!e.isTrusted||e.target!==r)throw 'native charset state event';charsetTrace+=r.readyState+';';}};
r.onload=function(e){{if(!e.isTrusted||!(e instanceof ProgressEvent))throw 'native charset load';charsetLoads++;charsetLoaded=e.loaded;charsetTotal=e.total;charsetComputable=e.lengthComputable;charsetTrace+='load;';}};
r.onerror=function(){{charsetErrors++;charsetTrace+='error;';}};
r.onloadend=function(){{charsetEnds++;charsetTrace+='end;';}};
r.open('GET','/charset');r.responseType='text';{override_source}r.send();
"#));
}

fn complete_charset_response(
    doc: &mut Document,
    forms: &mut Forms,
    page: &mut Page,
    content_type: &str,
    bytes: &[u8],
) -> Outcome {
    let request = page.take_native_request().expect("real queued charset request");
    let response = crate::http::Response {
        document_timing: None,
        url: request.target.clone(),
        redirected: false,
        fragment: None,
        status: 200,
        status_text: "Original".into(),
        encoded_body_bytes: bytes.len(),
        protocol: Some("HTTP/1.1"),
        request_is_post: false,
        cookie_same_site: true,
        headers: vec![
            ("Content-Type".into(), content_type.into()),
            ("Content-Length".into(), bytes.len().to_string()),
        ],
        body: bytes.to_vec(),
        events: Vec::new(),
    };
    let retained = response.status_text.capacity()
        + response.body.capacity()
        + response.headers.capacity() * std::mem::size_of::<(String, String)>()
        + response.headers.iter().map(|(name, value)| name.capacity() + value.capacity()).sum::<usize>()
        + response.url.host.capacity()
        + response.url.path.capacity()
        + response.url.query.as_ref().map_or(0, String::capacity);
    let (used, limit) = page.native_request_storage();
    let storage = crate::script_storage::Lease::new(used, retained, limit).unwrap();
    page.complete_native_request(doc, forms, crate::script_request_core::Completion {
        epoch: 0,
        request,
        result: Ok(response),
        _storage: Some(storage),
    })
}

#[test]
fn xhr_charset_response_and_override_precedence_reach_native_completion() {
    let (mut doc, mut forms, mut page) = fixture("");
    let cases: &[(&str, Option<&str>, &[u8], &str)] = &[
        ("text/plain;charset=windows-1252", None, &[65, 0x80], "A€"),
        ("text/plain;charset=windows-1252", Some("text/plain"), &[65, 0x80], "A€"),
        ("text/plain;charset=windows-1252", Some("not a MIME type"), &[65, 0x80], "A€"),
        ("text/plain;charset=utf-8", Some("text/plain;charset=windows-1252"), &[65, 0x80], "A€"),
        ("text/plain;charset=windows-1252", Some("text/plain;charset=utf-8"), &[0xc3, 0xa9], "é"),
        ("text/plain;charset=windows-1252", Some("text/plain;charset=replacement"), b"no source text", "\u{fffd}"),
        ("text/plain;charset=utf-8", Some("text/plain;charset=\" \tSHIFT_JIS \""), &[0x82, 0xa0], "あ"),
        ("text/plain;charset=windows-1252", Some("text/plain;charset=unknown"), &[0xc3, 0xa9], "é"),
        ("text/plain;charset=windows-1252", Some("text/plain;charset=\"\""), &[0xc3, 0xa9], "é"),
        ("text/plain;charset=unknown", None, &[0xc3, 0xa9], "é"),
        ("text/plain;charset=unknown", None, &[0x80, 65], "\u{fffd}A"),
        ("text/plain;charset=\"\u{a0}windows-1252\"", None, &[0xc3, 0xa9], "é"),
        ("not a MIME type", None, &[0xc3, 0xa9], "é"),
        ("application/xml;charset=unknown", None, &[0xc3, 0xa9], "é"),
    ];
    for &(content_type, override_mime, bytes, expected) in cases {
        start_charset_request(&mut doc, &mut forms, &mut page, override_mime);
        let outcome = complete_charset_response(&mut doc, &mut forms, &mut page, content_type, bytes);
        assert_eq!(outcome.errors, 0, "{content_type}, {override_mime:?}: {}", page.report());
        run(&mut doc, &mut forms, &mut page, &format!(r#"
if(r.responseText!=={expected}||r.response!==r.responseText||r.status!==200||r.statusText!=='Original'||r.readyState!==4)throw 'charset native response';
if(r.getResponseHeader('Content-Type')!=={content_type})throw 'override changed response header';
if(charsetTrace!=='1;2;3;4;load;end;'||charsetLoads!==1||charsetErrors!==0||charsetEnds!==1)throw charsetTrace;
if(charsetLoaded!=={length}||charsetTotal!=={length}||!charsetComputable)throw 'charset byte progress';
"#, expected=charset_js_string(expected), content_type=charset_js_string(content_type), length=bytes.len()));
        assert_eq!(page.native_request_pending_count(), 0);
    }
}

#[test]
fn xhr_charset_replacement_and_bom_preserve_response_bytes_events_and_empty_body() {
    let (mut doc, mut forms, mut page) = fixture("");
    for label in ["csiso2022kr", "hz-gb-2312", "iso-2022-cn", "iso-2022-cn-ext", "iso-2022-kr", "replacement"] {
        for &(bytes, expected) in &[
            (&b"<script>not decoded</script>"[..], "\u{fffd}"),
            (&b""[..], ""),
            (&[0xef, 0xbb, 0xbf, 0xc3, 0xa9][..], "é"),
            (&[0xff, 0xfe, 65, 0][..], "A"),
            (&[0xfe, 0xff, 0, 65][..], "A"),
        ] {
            start_charset_request(&mut doc, &mut forms, &mut page, None);
            let content_type = format!("text/plain;charset={label}");
            let outcome = complete_charset_response(&mut doc, &mut forms, &mut page, &content_type, bytes);
            assert_eq!(outcome.errors, 0, "{label}: {}", page.report());
            let trace = if bytes.is_empty() { "1;2;4;load;end;" } else { "1;2;3;4;load;end;" };
            run(&mut doc, &mut forms, &mut page, &format!(r#"
if(r.responseText!=={expected}||r.response!==r.responseText||r.status!==200)throw 'replacement native response';
if(charsetTrace!=={trace:?}||charsetLoads!==1||charsetErrors!==0||charsetEnds!==1)throw charsetTrace;
if(charsetLoaded!=={length}||charsetTotal!=={length}||charsetComputable!=={computable})throw 'replacement progress counts bytes';
"#, expected=charset_js_string(expected), length=bytes.len(), computable=!bytes.is_empty()));
            let received = page.state.xhr.records.last().unwrap().response.as_ref().unwrap();
            assert_eq!(received.received_bytes, bytes.len() as u64);
            assert_eq!(received.text, expected);
            assert!(received.response.body.is_empty());
        }
    }
}

#[test]
fn xhr_charset_output_refusal_never_delivers_success_and_fresh_request_recovers() {
    let (mut doc, mut forms, mut page) = fixture("");
    start_charset_request(&mut doc, &mut forms, &mut page, None);
    let limit = page.state.limits.host.string_bytes;
    page.state.limits.host.string_bytes = 2;
    let outcome = complete_charset_response(&mut doc, &mut forms, &mut page, "text/plain;charset=replacement", b"unpublished");
    page.state.limits.host.string_bytes = limit;
    assert!(outcome.errors > 0, "output refusal must remain recorded");
    assert!(page.report().contains("Native request response storage was refused."));
    assert!(page.state.xhr.records.last().unwrap().response.is_none());
    run(&mut doc, &mut forms, &mut page, "if(charsetLoads!==0||r.status!==0||r.responseText!==''||r.readyState!==4)throw 'refused charset published success';");
    start_charset_request(&mut doc, &mut forms, &mut page, Some("text/plain"));
    let outcome = complete_charset_response(&mut doc, &mut forms, &mut page, "text/plain;charset=windows-1252", &[65, 0x80]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    run(&mut doc, &mut forms, &mut page, "if(r.responseText!=='A\\u20ac'||charsetLoads!==1||charsetErrors!==0||charsetEnds!==1)throw 'fresh charset recovery';");
}
