//! Original Headers protocol checks; no networking or site-success claim.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("Headers must not read hosts".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("Headers must not write hosts".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("Headers must not call hosts".into())
    }
}
fn succeeds(source: &str) {
    assert_eq!(
        Runtime::for_test()
            .execute(source, &mut NoHost)
            .unwrap()
            .to_text(),
        "ok"
    );
}
#[test]
fn fetch_headers_real_list_normalization_duplicates_and_cookie_values() {
    succeeds(
        r#"
var h=new Headers([['Z-Last',' \t last \r\n'],['x-Case','one'],['X-CASE','two'],['Set-Cookie','a=1'],['set-cookie','b=2']]);
if(h.get('X-case')!=='one, two'||h.get('none')!==null||!h.has('z-last')||h.get('set-cookie')!=='a=1, b=2')throw 'real list';
if(JSON.stringify(Array.from(h))!=='[["set-cookie","a=1"],["set-cookie","b=2"],["x-case","one, two"],["z-last","last"]]')throw 'sorted combined cookie entries';
if(h.getSetCookie().join('|')!=='a=1|b=2')throw 'separate cookies';h.set('SET-cookie','c=3');if(h.getSetCookie().join('|')!=='c=3')throw 'set replaces all cookies';
var copy=new Headers(h);h.delete('x-case');if(h.has('x-case')||copy.get('x-case')!=='one, two')throw 'copy independence';
if(Object.prototype.toString.call(h)!=='[object Headers]'||h[Symbol.iterator]!==h.entries||Headers.length!==0||Headers.prototype.append.length!==2)throw 'native interface';'ok';
"#,
    );
}
#[test]
fn fetch_headers_conversion_order_and_receiver_validation_precede_mutation() {
    succeeds(
        r#"
var h=new Headers(),calls='',key={toString:function(){calls+='k';return 'Bad Name';}},value={toString:function(){calls+='v';return 'value';}};
try{h.append(key,value);throw 'missing invalid name';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!=='kv'||Array.from(h).length!==0)throw 'Web IDL before validation';
for(var bad of [null,{},Object.create(Headers.prototype)]){calls='';try{Headers.prototype.append.call(bad,key,value);throw 'bad receiver';}catch(e){if(!(e instanceof TypeError))throw e;}if(calls!=='')throw 'brand before conversion';}
var marker={},same=false;try{h.append('x',{toString:function(){throw marker;}});}catch(e){same=e===marker;}if(!same||h.has('x'))throw 'conversion exception';
var late=0;try{new Headers([['bad name','bad'],['next',{toString:function(){late++;return 'next';}}]]);}catch(e){if(!(e instanceof TypeError))throw e;}if(late!==1)throw 'complete init conversion before validation';
h.append('x',{toString:function(){h.set('x','inside');return 'outside';}});if(h.get('x')!=='inside, outside')throw 'post-conversion native list';'ok';
"#,
    );
}
#[test]
fn fetch_headers_record_and_sequence_conversion_close_iterators_on_abrupt_completion() {
    succeeds(
        r#"
var closed=0,innerClosed=0,marker={},source={};source[Symbol.iterator]=function(){return {next:function(){return {done:false,value:{[Symbol.iterator]:function(){return {next:function(){return {done:false,value:{toString:function(){throw marker;}}};},return:function(){innerClosed++;return {};}};}}};},return:function(){closed++;return {};}};};
var same=false;try{new Headers(source);}catch(e){same=e===marker;}if(!same||closed!==1||innerClosed!==1)throw 'nested iterator close';
var count=0;try{new Headers([['x','one',{toString:function(){count++;return 'third';}}]]);}catch(e){if(!(e instanceof TypeError))throw e;}if(count!==1)throw 'whole pair converted';
var record={get Beta(){return ' two ';},alpha:'one'},h=new Headers(record);if(Array.from(h.keys()).join('|')!=='alpha|beta'||h.get('beta')!=='two')throw 'enumerable record';
for(var value of [null,17,true,'x']){var caught=false;try{new Headers(value);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'init object required';}
'Hidden';Object.defineProperty(record,'ignored',{value:Symbol(),enumerable:false});new Headers(record);record[Symbol('bad')]='value';var caught=false;try{new Headers(record);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'symbol record key';'ok';
"#,
    );
}
#[test]
fn fetch_headers_live_sorted_iterators_and_callbacks_see_native_mutations() {
    succeeds(
        r#"
var h=new Headers({b:'two',d:'four'}),i=h.entries();if(i.next().value.join('|')!=='b|two')throw 'first';h.set('a','one');if(i.next().value.join('|')!=='b|two')throw 'insertion before live cursor';h.delete('b');if(i.next().value!==undefined)throw 'live deletion';h.set('z','last');if(i.next().value.join('|')!=='z|last'||!i.next().done)throw 'new value after done';
var seen=[],ctx={};h=new Headers({a:'one'});h.forEach(function(value,key,owner){if(this!==ctx||owner!==h)throw 'callback args';seen.push(key+':'+value);if(key==='a')h.set('b','two');},ctx);if(seen.join('|')!=='a:one|b:two')throw 'live foreach';
var d=Object.getOwnPropertyDescriptor(Headers.prototype,'get');if(!d.enumerable||!d.configurable||!d.writable)throw 'real method descriptor';'ok';
"#,
    );
}
#[test]
fn fetch_headers_invalid_bytes_and_required_arguments_do_not_publish() {
    succeeds(
        r#"
var h=new Headers({x:'old'});for(var pair of [['bad name','v'],['','v'],['x','a\r\nb'],['x','a\0b'],['x','\u0100'],[Symbol('x'),'v'],['x',Symbol('v')]]){var caught=false;try{h.append(pair[0],pair[1]);}catch(e){caught=e instanceof TypeError;}if(!caught||h.get('x')!=='old')throw 'invalid append publication';}
for(var name of ['append','set','get','has','delete']){var caught=false;try{h[name]();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'required arguments';}
h.set('x','\u00ff\tvalue');if(h.get('x')!=='\u00ff\tvalue')throw 'ByteString not ASCII only';h.append('Cookie','private=value');if(h.get('cookie')!=='private=value')throw 'standalone headers no request capability';'ok';
"#,
    );
}

#[test]
fn fetch_headers_retained_failure_is_atomic_and_immutable_guard_is_real() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute("var h=new Headers({x:'old'});", &mut NoHost)
        .unwrap();
    let value = runtime
        .binding_value(runtime.global_environment, "h")
        .unwrap();
    let Value::Object(id) = value else { panic!() };
    let before = runtime.value_bytes;
    let allowance = runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values = before + 16;
    let failure = runtime.headers_builtin(
        Op::Method("append"),
        &[
            Value::String("x".into()),
            Value::String("replacement".repeat(80)),
        ],
        Value::Object(id),
        &mut NoHost,
    );
    assert!(failure.is_err());
    assert_eq!(runtime.value_bytes, before);
    runtime.limits.heap.retained_values = allowance;
    assert_eq!(
        runtime
            .execute(
                "if(h.get('x')!=='old')throw 'partial publication';'ok';",
                &mut NoHost
            )
            .unwrap()
            .to_text(),
        "ok"
    );
    if let Some(Data::List { guard, .. }) = &mut runtime.objects[id].headers {
        *guard = Guard::Immutable;
    }
    assert_eq!(runtime.execute("var called=0,caught=false;try{h.set('x',{toString:function(){called++;return 'new';}});}catch(e){caught=e instanceof TypeError;}if(!caught||called!==1||h.get('x')!=='old')throw 'immutable validates after conversion';'ok';",&mut NoHost).unwrap().to_text(),"ok");
}
#[test]
fn fetch_headers_large_values_follow_paid_storage_without_url_parameter_count_caps() {
    let mut runtime = Runtime::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(runtime.execute("var pairs=[];for(var i=0;i<1100;i++)pairs.push(['x-'+i,'v']);var h=new Headers(pairs);if(Array.from(h.keys()).length!==1100||h.get('x-1099')!=='v')throw 'real entries';'ok';",&mut NoHost).unwrap().to_text(),"ok");
}
