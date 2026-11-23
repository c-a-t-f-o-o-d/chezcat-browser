//! Original regressions for HTTP(S) protocol parser no-ops and hyperlink projection.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("URL protocol operations must not read a native host")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("URL protocol operations must not write a native host")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("URL protocol operations must not call a native host")
    }
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    let result = runtime.execute(source, &mut NoHost).unwrap();
    assert!(matches!(&result, Value::Bool(true)), "{}", result.to_text());
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn url_protocol_non_special_names_are_noops_and_preserve_query_spelling() {
    succeeds(r#"
var u=new URL('https://a.test:8443/p?x=1#f'),q=u.searchParams;u.protocol='HTTPS1:';u.href==='https://a.test:8443/p?x=1#f'&&u.searchParams===q;
"#);
    succeeds(r#"
var u=new URL('https://a.test/p');u.protocol='data:text/plain,x';u.href==='https://a.test/p';
"#);
    succeeds(r#"
var inputs=['custom','HTTPS1:','http+ext','http.foo','http-foo','data:text/plain,x','mailto:user@example.test','javascript:void(0)','ftp+ext:','filex:','ws+extension:'];
var u=new URL('https://u:p@EXAMPLE.test:8443/a?x=a%20b&tilde=~&slash=%2f&x=two#f'),q=u.searchParams;
for(var i=0;i<inputs.length;i++){
    u.protocol=inputs[i];
    if(u.href!=='https://u:p@example.test:8443/a?x=a%20b&tilde=~&slash=%2f&x=two#f'||u.searchParams!==q||q.getAll('x').join('|')!=='a b|two')throw 'non-special protocol changed URL or query';
}
true;
"#);
}

#[test]
fn url_protocol_file_guard_uses_credentials_or_non_null_port() {
    succeeds(r#"
var u=new URL('https://u:p@a.test/p');u.protocol='FiLe:';u.href==='https://u:p@a.test/p';
"#);
    succeeds(r#"
var u=new URL('https://a.test:81/p');u.protocol='file';u.href==='https://a.test:81/p';
"#);
    succeeds(r#"
var inputs=['https://u@a.test/p?x=%7e#f','https://:p@a.test/p?x=%7e#f','https://u:p@a.test:443/p?x=%7e#f','https://a.test:0/p?x=%7e#f','http://a.test:443/p?x=%7e#f','https://a.test:80/p?x=%7e#f'];
for(var i=0;i<inputs.length;i++){
    var u=new URL(inputs[i]),before=u.href,q=u.searchParams;
    u.protocol='FiLe';u.protocol='\tf\ri\nle:ignored';
    if(u.href!==before||u.searchParams!==q||q.get('x')!=='~')throw 'file guard changed URL';
}
true;
"#);
}

#[test]
fn url_protocol_invalid_and_raw_inputs_are_noops() {
    succeeds(r#"
var u=new URL('https://a.test/p');u.protocol='3x:';u.href==='https://a.test/p';
"#);
    succeeds(r#"
var u=new URL('https://a.test/p'),s='';u.protocol={[Symbol.toPrimitive]:function(h){s+=h;return '\ud800:';}};u.href==='https://a.test/p'&&s==='string';
"#);
    succeeds(r#"
var inputs=['','3http:',' https:','http :','http/','http\0:','\0http:','\ud800:','\udfff:','ht\ud800tp:','\ud83d\ude00:','http😀:','é:'];
var u=new URL('https://a.test:81/p?x=%2f&space=a%20b#f'),q=u.searchParams;
for(var i=0;i<inputs.length;i++){
    u.protocol=inputs[i];
    if(u.href!=='https://a.test:81/p?x=%2f&space=a%20b#f'||u.searchParams!==q)throw 'invalid protocol changed URL';
}
true;
"#);
}

#[test]
fn url_protocol_http_transitions_normalize_default_ports_only() {
    succeeds(r#"
var u=new URL('https://a.test:80/p');u.protocol='http:';u.href==='http://a.test/p';
"#);
    succeeds(r#"
var rows=[
 ['https://a.test:80/p?x=a%20b&z=%2f#f','hTtP:ignored','http://a.test/p?x=a%20b&z=%2f#f'],
 ['http://a.test:443/p?x=a%20b&z=%2f#f','HtTpS::::','https://a.test/p?x=a%20b&z=%2f#f'],
 ['https://a.test:8443/p?x=a%20b&z=%2f#f','http','http://a.test:8443/p?x=a%20b&z=%2f#f'],
 ['http://a.test/p?x=~#','\th\rt\ntps:','https://a.test/p?x=~#'],
 ['https://a.test:81/p?x=~#','HtTpS:\ud800','https://a.test:81/p?x=~#'],
 ['https://a.test:443/p?x=~#','https','https://a.test/p?x=~#']
];
for(var i=0;i<rows.length;i++){
    var u=new URL(rows[i][0]),q=u.searchParams;
    u.protocol=rows[i][1];
    if(u.href!==rows[i][2]||u.searchParams!==q)throw 'http transition or query spelling';
}
true;
"#);
}

#[test]
fn url_protocol_non_special_primitive_conversion_is_noop() {
    succeeds(r#"
var u=new URL('https://a.test/p?x=%2f#f'),q=u.searchParams;
var inputs=[undefined,null,true,false,0,NaN,Infinity,BigInt('9'),new String('mailto')];
for(var i=0;i<inputs.length;i++){
    u.protocol=inputs[i];
    if(u.href!=='https://a.test/p?x=%2f#f'||u.searchParams!==q)throw 'primitive protocol conversion';
}
true;
"#);
}

#[test]
fn url_protocol_conversion_getter_runs_once_before_live_noop() {
    succeeds(r#"
var u=new URL('https://a.test/p'),n=0;u.protocol={toString:function(){n++;u.pathname='/changed';return 'mailto';}};u.href==='https://a.test/changed'&&n===1;
"#);
    succeeds(r#"
var u=new URL('https://old.test/old?old=1'),q=u.searchParams,trace='',reads=0,calls=0,arg={};
Object.defineProperty(arg,Symbol.toPrimitive,{get:function(){
    trace+='G';reads++;u.href='http://next.test:81/live?x=a%20b&z=%2f#live';
    return function(hint){if(this!==arg||hint!=='string')throw 'conversion receiver or hint';trace+='C';calls++;u.password='p';return 'MaIlTo:\ud800';};
}});
Object.defineProperty(arg,'toString',{get:function(){throw 'ordinary conversion after exotic';}});
u.protocol=arg;
trace==='GC'&&reads===1&&calls===1&&u.href==='http://:p@next.test:81/live?x=a%20b&z=%2f#live'&&u.searchParams===q&&q.get('x')==='a b'&&q.get('z')==='/'&&!q.has('old');
"#);
}

#[test]
fn url_protocol_file_guard_observes_reentrant_credentials_and_port() {
    // Web IDL converts the assigned value before the setter reads the current URL.
    // https://webidl.spec.whatwg.org/#es-attributes
    succeeds(r#"
var mutations=[function(u){u.username='live';},function(u){u.password='live';},function(u){u.port='0';},function(u){u.href='http://u:p@b.test:81/live?x=%2f#z';}];
var expected=['https://live@a.test/p?x=%7e#f','https://:live@a.test/p?x=%7e#f','https://a.test:0/p?x=%7e#f','http://u:p@b.test:81/live?x=%2f#z'];
for(var i=0;i<mutations.length;i++){
    var u=new URL('https://a.test/p?x=%7e#f'),q=u.searchParams,calls=0,arg={};
    arg.toString=function(){calls++;mutations[i](u);return 'file';};
    u.protocol=arg;
    if(calls!==1||u.href!==expected[i]||u.searchParams!==q)throw 'file guard read stale URL';
}
true;
"#);
}

#[test]
fn url_protocol_http_transition_observes_reentrant_live_record() {
    // Web IDL converts the assigned value before the setter reads the current URL.
    // https://webidl.spec.whatwg.org/#es-attributes
    succeeds(r#"
var u=new URL('https://old.test:8443/old?old=1'),q=u.searchParams,calls=0,arg={};
arg.toString=function(){calls++;u.href='https://next.test:80/live?x=%2f&space=a%20b#f';return 'hTtP:\udfff';};
u.protocol=arg;
calls===1&&u.href==='http://next.test/live?x=%2f&space=a%20b#f'&&u.searchParams===q&&q.get('x')==='/'&&q.get('space')==='a b'&&!q.has('old');
"#);
}

#[test]
fn url_protocol_abrupt_conversion_preserves_side_effects_and_identity() {
    succeeds(r#"
var u=new URL('https://a.test:81/p?x=%2f#f'),q=u.searchParams,marker={},seen=false,arg={};
Object.defineProperty(arg,Symbol.toPrimitive,{get:function(){u.pathname='/getter';throw marker;}});
try{u.protocol=arg;}catch(e){seen=e===marker;}
if(!seen||u.href!=='https://a.test:81/getter?x=%2f#f'||u.searchParams!==q)throw 'getter abrupt effects';
seen=false;arg={};arg[Symbol.toPrimitive]=function(hint){u.href='https://other.test:81/live?raw=%2f#f';throw marker;};
try{u.protocol=arg;}catch(e){seen=e===marker;}
if(!seen||u.href!=='https://other.test:81/live?raw=%2f#f'||u.searchParams!==q||q.get('raw')!=='/')throw 'hook abrupt effects';
seen=false;var trace='';
arg={toString:function(){trace+='T';u.hash='after';return {};},valueOf:function(){trace+='V';return {};}};
try{u.protocol=arg;}catch(e){seen=e.name==='TypeError';}
if(!seen||trace!=='TV'||u.href!=='https://other.test:81/live?raw=%2f#after'||u.searchParams!==q)throw 'ordinary failure effects';
seen=false;try{u.protocol=Symbol('http');}catch(e){seen=e.name==='TypeError';}
seen&&u.href==='https://other.test:81/live?raw=%2f#after'&&u.searchParams===q;
"#);
}

#[test]
fn url_protocol_ordinary_fallback_keeps_conversion_order() {
    succeeds(r#"
var u=new URL('https://a.test/p?x=%2f#f'),q=u.searchParams,trace='',arg={};
Object.defineProperty(arg,'toString',{get:function(){trace+='G';return function(){if(this!==arg)throw 'toString receiver';trace+='T';u.pathname='/toString';return {};};}});
Object.defineProperty(arg,'valueOf',{get:function(){trace+='H';return function(){if(this!==arg)throw 'valueOf receiver';trace+='V';u.hash='valueOf';return 'CuStOm:\ud800';};}});
u.protocol=arg;
trace==='GTHV'&&u.href==='https://a.test/toString?x=%2f#valueOf'&&u.searchParams===q;
"#);
}

#[test]
fn url_protocol_brand_rejection_precedes_conversion() {
    succeeds(r#"
var u=new URL('https://a.test/p?x=%2f#f'),q=u.searchParams,setter=Object.getOwnPropertyDescriptor(URL.prototype,'protocol').set,reads=0,arg={};
Object.defineProperty(arg,Symbol.toPrimitive,{get:function(){reads++;return function(){throw 'unexpected conversion';};}});
var receivers=[null,undefined,{},Object.create(URL.prototype),Object.create(u)];
for(var i=0;i<receivers.length;i++){
    var rejected=false;try{setter.call(receivers[i],arg);}catch(e){rejected=e.name==='TypeError';}
    if(!rejected||reads!==0)throw 'URL brand before conversion';
}
setter.call(u);
u.href==='https://a.test/p?x=%2f#f'&&u.searchParams===q&&reads===0;
"#);
}

#[test]
fn url_protocol_real_special_transitions_keep_finite_capability_errors() {
    for source in [
        r#"var u=new URL("https://a.test/p?x=%2f#f");u.protocol="ftp";u.href;"#,
        r#"var u=new URL("https://a.test/p?x=%2f#f");u.protocol="wS:";u.href;"#,
        r#"var u=new URL("https://a.test/p?x=%2f#f");u.protocol="WSS";u.href;"#,
        r#"var u=new URL("https://a.test/p?x=%2f#f");u.protocol="file";u.href;"#,
        r#"var u=new URL("https://a.test:443/p?x=%2f#f");u.protocol="file";u.href;"#,
        r#"var u=new URL("http://a.test:80/p?x=%2f#f");u.protocol="file";u.href;"#,
        r#"var u=new URL("https://:@a.test/p?x=%2f#f");u.protocol="file";u.href;"#,
        r#"var u=new URL('https://u:p@a.test:81/p?x=%2f#f');u.protocol='ftp';u.href;"#,
    ] {
        let mut runtime = Runtime::for_test();
        let error = runtime.execute(source, &mut NoHost).unwrap_err();
        assert!(error.contains("URL values support only HTTP(S) schemes."), "{error}");
        assert!(runtime.transient_registration_is_empty());
    }
}

#[test]
fn url_protocol_capability_failure_keeps_only_conversion_side_effects() {
    let mut runtime = Runtime::for_test();
    let error = runtime.execute(r#"
var u=new URL('https://old:pw@a.test:81/old?x=%2f#f'),q=u.searchParams,calls=0;
u.protocol={toString:function(){
    calls++;u.username='';u.password='';u.port='';u.pathname='/live';
    return 'file';
}};
"#, &mut NoHost).unwrap_err();
    assert!(error.contains("URL values support only HTTP(S) schemes."), "{error}");
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    let result = runtime.execute(r#"
calls===1&&u.href==='https://a.test/live?x=%2f#f'&&u.searchParams===q&&q.get('x')==='/';
"#, &mut NoHost).unwrap();
    assert!(matches!(&result, Value::Bool(true)), "{}", result.to_text());
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn anchor_protocol_noop_reflects_serialized_href() {
    // The hyperlink protocol setter updates href after parsing even when the
    // scheme is unchanged. This checks the pure helper used by its host bridge.
    // https://html.spec.whatwg.org/multipage/links.html#dom-hyperlink-protocol
    let raw = "../raw?x=a%20b&tilde=~#f";
    let base = Some("https://EXAMPLE.test:443/a/page");
    for input in ["mailto:", "3http:", "", "\u{fffd}:"] {
        assert_eq!(
            mutate(raw, base, "protocol", input).unwrap().as_deref(),
            Some("https://example.test/raw?x=a%20b&tilde=~#f"),
            "{input:?}",
        );
    }
    for (raw, base, input, expected) in [
        (
            " HTTPS://u:p@EXAMPLE.test:443/a/../b?x=%7e#f ",
            None,
            "file",
            "https://u:p@example.test/b?x=%7e#f",
        ),
        (
            "../raw?x=%2f#f",
            Some("https://EXAMPLE.test:81/a/page"),
            "file",
            "https://example.test:81/raw?x=%2f#f",
        ),
        (
            "https://EXAMPLE.test:443/a/../b?x=%2f#f",
            Some("file:///C:/fixture/page.html"),
            "data:text/plain,x",
            "https://example.test/b?x=%2f#f",
        ),
    ] {
        assert_eq!(
            mutate(raw, base, "protocol", input).unwrap().as_deref(),
            Some(expected),
            "{raw} {input}",
        );
    }
}

#[test]
fn anchor_protocol_noop_requires_parsed_original_and_preserves_errors() {
    assert_eq!(mutate("https://[bad/", None, "protocol", "data").unwrap(), None);
    assert_eq!(mutate("relative-without-base", None, "protocol", "1http").unwrap(), None);
    for input in ["ftp", "file"] {
        let error = mutate("../raw", Some("https://example.test/a/page"), "protocol", input)
            .unwrap_err();
        assert!(error.contains("URL values support only HTTP(S) schemes."), "{error}");
    }
    let too_long = "x".repeat(MAX_URL_BYTES + 1);
    for raw in ["https://example.test/a", "https://[bad/"] {
        assert_eq!(
            mutate(raw, None, "protocol", &too_long).unwrap_err(),
            "URL length limit reached (16 KiB).",
            "{raw}",
        );
    }
}
