//! Original buffered Response and Body protocol checks.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("Response must not read native hosts".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("Response must not write native hosts".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("Response must not call native hosts".into())
    }
}
fn succeeds(source: &str, after: &str) {
    let mut runtime = Runtime::for_test();
    runtime.execute(source, &mut NoHost).unwrap();
    assert!(runtime
        .microtask_checkpoint(&mut NoHost)
        .unwrap()
        .is_empty());
    assert_eq!(runtime.execute(after, &mut NoHost).unwrap().to_text(), "ok");
}
#[test]
fn fetch_response_real_status_headers_readonly_slots_and_error_guard() {
    succeeds(
        r#"
var input=new Headers([['X-Native','one'],['Set-Cookie','private=authored']]),r=new Response(null,{status:201.8,statusText:'Created',headers:input});
if(r.status!==201||!r.ok||r.statusText!=='Created'||r.type!=='default'||r.url!==''||r.redirected||r.body!==null||r.bodyUsed||r.headers===input||r.headers.get('x-native')!=='one'||r.headers.has('set-cookie'))throw 'real metadata and response header guard';
r.headers.append('Set-Cookie','still=private');r.headers.set('x-native','two');if(r.headers.getSetCookie().length||input.get('x-native')!=='one'||r.headers.get('x-native')!=='two')throw 'native response headers independence';
if(Object.keys(r).length||Object.hasOwn(r,'status')||Object.prototype.toString.call(r)!=='[object Response]'||Response.length!==0||r.clone.length!==0||r.text.length!==0)throw 'native descriptors';
r.status=400;if(r.status!==201)throw 'readonly';var error=Response.error();if(error.status!==0||error.ok||error.type!=='error'||error.body!==null||error.bodyUsed)throw 'network error metadata';
var calls=0,caught=false;try{error.headers.append('x',{toString:function(){calls++;return 'v';}});}catch(e){caught=e instanceof TypeError;}if(!caught||calls!==1||error.headers.has('x'))throw 'immutable native headers';
if(new Response(null,{status:65736}).status!==200)throw 'Web IDL unsigned short';
for(var status of [0,199,600,NaN,Infinity]){caught=false;try{new Response(null,{status:status});}catch(e){caught=e instanceof RangeError;}if(!caught)throw 'status range';}
for(var status of [204,205,304]){caught=false;try{new Response('',{status:status});}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'non-null empty body';}
for(var text of ['bad\r\ntext','\u007f','\u0100']){caught=false;try{new Response(null,{statusText:text});}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'reason phrase bytes';}
"#,
        "'ok';",
    );
}
#[test]
fn fetch_response_body_clone_single_use_null_body_and_intrinsic_promises() {
    succeeds(
        r#"
var r=Response.json({items:[1,2],real:true}),copy=r.clone(),count=0,OriginalPromise=Promise;
if(r.headers.get('content-type')!=='application/json'||copy.headers===r.headers)throw 'real JSON type and clone';copy.headers.set('x-copy','yes');if(r.headers.has('x-copy'))throw 'clone headers independence';
Promise=function(){throw 'replaceable global Promise must not be called';};
var promise=r.json();if(!(promise instanceof OriginalPromise)||!r.bodyUsed||copy.bodyUsed)throw 'real native Promise and use state';promise.then(function(data){if(!data.real||data.items[1]!==2)throw 'actual JSON body';count++;});
copy.text().then(function(text){if(text!=='{"items":[1,2],"real":true}')throw 'clone actual bytes';count++;});r.text().then(function(){throw 'duplicate consumption succeeded';},function(e){if(!(e instanceof TypeError))throw e;count++;});
var caught=false;try{r.clone();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'used clone';
var empty=new Response();empty.text().then(function(text){if(text!==''||empty.bodyUsed)throw 'null body consumption';count++;});empty.text().then(function(text){if(text!==''||empty.bodyUsed)throw 'repeat null body';count++;});
Response.prototype.text.call({}).catch(function(e){if(!(e instanceof TypeError))throw e;count++;});new Response('{').json().catch(function(e){if(!(e instanceof SyntaxError))throw e;count++;});
"#,
        "if(count!==7)throw 'queued results';'ok';",
    );
}
#[test]
fn fetch_response_binary_snapshot_ranges_utf8_bom_and_urlencoded_extraction_are_real() {
    succeeds(
        r#"
var source=new Uint8Array([10,65,0,255]),r=new Response(new DataView(source.buffer,1,2)),copy=r.clone(),count=0;source[1]=9;
if(r.headers.has('content-type'))throw 'binary no fabricated type';r.arrayBuffer().then(function(buffer){var bytes=new Uint8Array(buffer);if(bytes.length!==2||bytes[0]!==65||bytes[1]!==0)throw 'actual copied range';bytes[0]=99;count++;});
copy.bytes().then(function(bytes){if(!(bytes instanceof Uint8Array)||bytes[0]!==65||bytes[1]!==0)throw 'independent binary clone';count++;});
new Response(new Uint8Array([255,254,65,0])).text().then(function(text){if(text!=='\ufffd\ufffdA\0')throw 'body UTF8 never UTF16 sniff';count++;});
new Response(new Uint8Array([239,187,191,123,34,120,34,58,49,125])).json().then(function(data){if(data.x!==1)throw 'UTF8 BOM';count++;});
new Response('\ud800x').text().then(function(text){if(text!=='\ufffdx')throw 'USV replacement';count++;});
var params=new URLSearchParams({a:'before'});params.toString=function(){throw 'author serialization override';};var encoded=new Response(params,{get headers(){params.set('a','after value');return undefined;}});if(encoded.headers.get('content-type')!=='application/x-www-form-urlencoded;charset=UTF-8')throw 'native form type';encoded.text().then(function(text){if(text!=='a=after+value')throw 'extract internal post-dictionary params';count++;});
"#,
        "if(count!==6)throw 'binary results';'ok';",
    );
}
#[test]
fn fetch_response_idl_conversion_precedes_validation_and_buffer_extraction() {
    succeeds(
        r#"
var order='',body={toString:function(){order+='b';return 'body';}},r=new Response(body,{get headers(){order+='h';return {'X-Value':{toString:function(){order+='i';return 'value';}}};},get status(){order+='s';return {valueOf:function(){order+='n';return 200;}};},get statusText(){order+='t';return {toString:function(){order+='v';return 'OK';}};}});
if(order!=='bhisntv'||r.headers.get('x-value')!=='value')throw 'IDL conversion order';
var late=0,caught=false;try{new Response('x',{headers:[['bad name','v']],get statusText(){late++;return 'OK';}});}catch(e){caught=e instanceof TypeError;}if(!caught||late!==1)throw 'whole dictionary before header validation';
var source=new Uint8Array([1,2]),count=0,r=new Response(source,{get status(){source[0]=9;return 200;}});r.bytes().then(function(bytes){if(bytes[0]!==9)throw 'extract BufferSource after dictionary';count++;});
"#,
        "if(count!==1)throw 'buffer extraction result';'ok';",
    );
}
#[test]
fn fetch_response_body_promises_use_real_thenable_resolution_and_queued_jobs() {
    succeeds(r#"
var gets=0,jobs=0,result='';Object.defineProperty(Object.prototype,'then',{configurable:true,get:function(){gets++;return function(resolve){jobs++;resolve('assimilated');};}});
var promise=new Response('{"real":true}').json();if(gets!==1||jobs!==0)throw 'synchronous then getter, queued invocation';promise.then(function(value){result=value;});
"#,"delete Object.prototype.then;if(gets!==1||jobs!==1||result!=='assimilated')throw 'native resolution';'ok';");
}
#[test]
fn fetch_response_unimplemented_stream_refuses_without_claiming_a_null_body() {
    let mut runtime = Runtime::for_test();
    let error = runtime
        .execute("var r=new Response('real body');r.body;", &mut NoHost)
        .unwrap_err();
    assert!(
        error.contains("ReadableStream access is not implemented"),
        "{error}"
    );
    assert_eq!(
        runtime
            .execute(
                "if(r.bodyUsed)throw 'stream refusal consumed body';'ok';",
                &mut NoHost
            )
            .unwrap()
            .to_text(),
        "ok"
    );
    runtime.execute("r.text();", &mut NoHost).unwrap();
    assert!(runtime
        .microtask_checkpoint(&mut NoHost)
        .unwrap()
        .is_empty());
    let error = runtime.execute("r.body;", &mut NoHost).unwrap_err();
    assert!(
        error.contains("ReadableStream access is not implemented"),
        "{error}"
    );
}

#[test]
fn fetch_response_resource_refusal_is_fatal_and_never_a_successful_or_rejected_promise() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute("var r=new Response(new Uint8Array(4096));", &mut NoHost)
        .unwrap();
    let Value::Object(id) = runtime
        .binding_value(runtime.global_environment, "r")
        .unwrap()
    else {
        panic!()
    };
    let promises = runtime
        .objects
        .iter()
        .filter(|o| o.promise.is_some())
        .count();
    let limit = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = runtime.value_bytes + 16;
    let fault = runtime
        .response_builtin(
            Op::Consume("arrayBuffer"),
            &[],
            Value::Object(id),
            &mut NoHost,
        )
        .unwrap_err();
    assert!(fault.exception.is_none());
    assert_eq!(
        runtime
            .objects
            .iter()
            .filter(|o| o.promise.is_some())
            .count(),
        promises
    );
    runtime.limits.heap.retained_values = limit;
    assert_eq!(runtime.execute("if(!r.bodyUsed||r.status!==200)throw 'real failed consumption state';new Response('new job').text().then(function(text){if(text!=='new job')throw 'recovery';});'ok';",&mut NoHost).unwrap().to_text(),"ok");
    assert!(runtime
        .microtask_checkpoint(&mut NoHost)
        .unwrap()
        .is_empty());
}
