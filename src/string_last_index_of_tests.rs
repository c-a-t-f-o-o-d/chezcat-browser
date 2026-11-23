//! Original String reverse-search controls. No library or page-specific code.
use super::*;

#[derive(Default)]
struct NoHost {
    gets: usize,
    sets: usize,
    calls: usize,
}
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        self.gets += 1;
        Err("No native reads".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        self.sets += 1;
        Err("No native writes".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        self.calls += 1;
        Err("No native calls".into())
    }
}
fn succeeds(source: &str) {
    Runtime::for_test()
        .execute(source, &mut NoHost::default())
        .unwrap();
}

#[test]
fn string_last_index_of_original_primitive_property_name_and_prefix_search() {
    succeeds(
        r#"
var names={foreground:1,'prefix-mode':2,'suffix-prefix':3},count=0;
for(var name in names){if(typeof name!=='string')throw 'ordinary property-name prefix';count++;}
if(count!==3||'prefix-mode'.indexOf('prefix')!==0||'suffix-prefix'.indexOf('prefix')!==7)
    throw 'existing literal String search controls';
if('prefix-mode'.lastIndexOf('prefix',0)!==0||'suffix-prefix'.lastIndexOf('prefix',0)!==-1
   ||'foreground'.lastIndexOf('prefix',0)!==-1)throw 'primitive String prefix search';
if('ababa'.lastIndexOf('aba')!==2||'ababa'.lastIndexOf('aba',1)!==0)
    throw 'reverse overlapping candidate starts';
"#,
    );
}

#[test]
fn string_last_index_of_position_defaults_clamping_empty_pattern_and_utf16_offsets() {
    succeeds(
        r#"
var f=String.prototype.lastIndexOf,tests=[
['abcabc'.lastIndexOf('abc'),3],['abcabc'.lastIndexOf('abc',undefined),3],
['abcabc'.lastIndexOf('abc',NaN),3],['abcabc'.lastIndexOf('abc',Infinity),3],
['abcabc'.lastIndexOf('abc',-Infinity),0],['abcabc'.lastIndexOf('abc',-10),0],
['abcabc'.lastIndexOf('abc',null),0],['abcabc'.lastIndexOf('abc',false),0],
['abcabc'.lastIndexOf('abc',2.9),0],['abcabc'.lastIndexOf('abc',3.9),3],
['abcabc'.lastIndexOf('bc',-0),-1],['abcabc'.lastIndexOf('bc',true),1],
['abc'.lastIndexOf(''),3],['abc'.lastIndexOf('',NaN),3],['abc'.lastIndexOf('',1.9),1],
['abc'.lastIndexOf('',-Infinity),0],['abc'.lastIndexOf('',Infinity),3],
[''.lastIndexOf(''),0],[''.lastIndexOf('x'),-1],['ab'.lastIndexOf('abc'),-1],
['undefinedxundefined'.lastIndexOf(),10],['anullbnull'.lastIndexOf(null),6],
['A😀B😀C'.lastIndexOf('😀'),4],['A😀B😀C'.lastIndexOf('😀',3),1],
['😀a😀'.lastIndexOf('😀',2),0],['😀a😀'.lastIndexOf('a'),2],
['😀a😀'.lastIndexOf('',Infinity),5],['éé'.lastIndexOf('é'),1],
[f.call(12121,'12'),2],[f.call(true,'r'),1],[f.call(new String('abcabc'),'bc'),4],
[f.call({toString:function(){return 'abcabc';}},'abc'),3]
];for(var i=0;i<tests.length;i++)if(tests[i][0]!==tests[i][1])throw 'case '+i;
var boxed=new String('original');boxed.toString=function(){return 'fixedfixed';};
if(f.call(boxed,'fixed')!==5)throw 'generic boxed conversion override';
"#,
    );
}

#[test]
fn string_last_index_of_coercions_are_live_ordered_once_and_reentrant() {
    succeeds(
        r#"
var f=String.prototype.lastIndexOf,order='',receiverGets=0,searchGets=0,positionGets=0;
var receiver={},search={},position={valueOf:function(){throw 'stale numeric hook';}};
Object.defineProperty(receiver,Symbol.toPrimitive,{get:function(){receiverGets++;return function(h){order+='receiver:'+h+';';return 'ababa';};}});
Object.defineProperty(search,Symbol.toPrimitive,{get:function(){searchGets++;return function(h){
    order+='search:'+h+';';receiver.toString=function(){throw 'receiver was converted twice';};
    Object.defineProperty(position,Symbol.toPrimitive,{get:function(){positionGets++;return function(h){
        order+='position:'+h+';';if(f.call('nested','ed')!==4)throw 'nested saved method';return 2;
    };}});
    String.prototype.lastIndexOf=function(){return 99;};return 'aba';
};}});
if(f.call(receiver,search,position)!==2||order!=='receiver:string;search:string;position:number;')throw order;
if(receiverGets!==1||searchGets!==1||positionGets!==1)throw 'coercion getter count';
if('ababa'.lastIndexOf('aba')!==99||f.call('ababa','aba')!==2)throw 'live prototype mutation versus saved intrinsic';
String.prototype.lastIndexOf=f;
var converted=0;
if('abc'.lastIndexOf({toString:function(){converted++;return '';}},{valueOf:function(){converted++;return NaN;}})!==3||converted!==2)
    throw 'empty pattern still converts search then numeric position';
"#,
    );
}

#[test]
fn string_last_index_of_preserves_abrupt_identity_and_converts_position_before_early_results() {
    succeeds(
        r#"
var f=String.prototype.lastIndexOf,marker={},later=0,caught=false;
var probe={toString:function(){later++;return 'x';}},pos={valueOf:function(){later++;return 0;}};
for(var receiver of [null,undefined,Symbol('receiver')]){
    caught=false;try{f.call(receiver,probe,pos);}catch(e){caught=e instanceof TypeError;}
    if(!caught||later!==0)throw 'invalid receiver touched later arguments';
}
caught=false;try{f.call({toString:function(){throw marker;}},probe,pos);}catch(e){caught=e===marker;}
if(!caught||later!==0)throw 'receiver throw identity';
caught=false;try{f.call('abc',{toString:function(){throw marker;}},pos);}catch(e){caught=e===marker;}
if(!caught||later!==0)throw 'search throw identity';
caught=false;try{f.call('abc',Symbol('needle'),pos);}catch(e){caught=e instanceof TypeError;}
if(!caught||later!==0)throw 'Symbol needle touched position';
caught=false;try{f.call('',{toString:function(){later++;return 'too long';}},{valueOf:function(){later++;throw marker;}});}catch(e){caught=e===marker;}
if(!caught||later!==2)throw 'too-long search skipped position or lost throw identity';
caught=false;try{f.call('','',Symbol('position'));}catch(e){caught=e instanceof TypeError;}
if(!caught)throw 'empty result skipped invalid numeric position';
if(f.call('abc','a',0,{toString:function(){throw 'extra converted';}})!==0)throw 'extra argument';
var regexp=/q/,reads=0;
Object.defineProperty(regexp,Symbol.match,{get:function(){reads++;throw 'IsRegExp must not run';}});
if(f.call('|/q/|',regexp)!==1||reads!==0)throw 'ordinary RegExp ToString without IsRegExp';
"#,
    );
}

#[test]
fn string_last_index_of_native_metadata_and_real_prototype_replace_delete_getter() {
    succeeds(
        r#"
var f=String.prototype.lastIndexOf,d=Object.getOwnPropertyDescriptor(String.prototype,'lastIndexOf');
if(f!==String.prototype.lastIndexOf||f.name!=='lastIndexOf'||f.length!==1||d.value!==f||d.enumerable||!d.writable||!d.configurable)
    throw 'canonical native method metadata';
if(Object.hasOwn(f,'prototype')||Function.prototype.toString.call(f).indexOf('[native code]')<0)throw 'native identity';
var caught=false;try{new f();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'constructible method';
String.prototype.lastIndexOf=function(){return 17;};if('abc'.lastIndexOf('a')!==17)throw 'replacement';
if(!delete String.prototype.lastIndexOf||'lastIndexOf' in String.prototype||'abc'.lastIndexOf!==undefined)throw 'deletion';
var reads=0,actual;
Object.defineProperty(String.prototype,'lastIndexOf',{get:function(){'use strict';reads++;actual=this;return f;},configurable:true});
if('ababa'.lastIndexOf('aba')!==2||reads!==1||actual!=='ababa')throw 'ordinary inherited getter receiver';
Object.defineProperty(String.prototype,'lastIndexOf',{value:f,writable:true,configurable:true});
if('ababa'.lastIndexOf('aba')!==2)throw 'restore';
"#,
    );
}

#[test]
fn string_last_index_of_native_conversion_refusal_precedes_hooks_and_stays_fatal() {
    let mut runtime = Runtime::for_test();
    let mut host = NoHost::default();
    runtime.define_global("native", Value::Host(9)).unwrap();
    runtime.execute("var count=0,caught=false,finalized=false;var probe={toString:function(){count++;return 'x';}};",&mut host).unwrap();
    let error=runtime.execute("try{String.prototype.lastIndexOf.call(native,probe);}catch(e){caught=true;}finally{finalized=true;}",&mut host).unwrap_err();
    assert!(
        error.contains("No conversion permission or hook read was granted"),
        "{error}"
    );
    assert_eq!((host.gets, host.sets, host.calls), (0, 0, 0));
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("count===0&&!caught&&!finalized;", &mut host)
            .unwrap()
            .to_text(),
        "true"
    );
    assert_eq!(
        runtime
            .execute("'ababa'.lastIndexOf('aba');", &mut host)
            .unwrap()
            .to_text(),
        "2"
    );
}

#[test]
fn string_last_index_of_search_work_scalar_caps_and_fresh_job_recovery() {
    use std::time::{Duration, Instant};
    let mut runtime = Runtime::for_test();
    let mut host = NoHost::default();
    let cap = runtime.limits.capacity.string_bytes;
    runtime.limits.capacity.string_bytes = 4;
    let error = runtime
        .string_last_index_of("abc", &[Value::String("longer".into())], &mut host)
        .unwrap_err();
    assert!(error.exception.is_none());
    assert!(
        error.message.contains("String limit reached"),
        "{}",
        error.message
    );
    runtime.limits.capacity.string_bytes = cap;
    runtime.begin_job();
    runtime.constrain_job(
        usize::MAX,
        0,
        usize::MAX,
        Instant::now() + Duration::from_secs(10),
    );
    let error = runtime
        .string_last_index_of("abc", &[Value::String("b".into())], &mut host)
        .unwrap_err();
    assert!(error.exception.is_none());
    assert!(
        error.message.contains("String/copy work limit"),
        "{}",
        error.message
    );
    assert_eq!(runtime.copy_bytes, 0);
    runtime.begin_job();
    runtime.constrain_job(
        usize::MAX,
        256,
        usize::MAX,
        Instant::now() + Duration::from_secs(10),
    );
    let mut calls = 0;
    loop {
        match runtime.string_last_index_of("ababa", &[Value::String("aba".into())], &mut host) {
            Ok(value) => {
                assert_eq!(value.to_text(), "2");
                calls += 1;
                assert!(calls < 64);
            }
            Err(error) => {
                assert!(error.exception.is_none());
                assert!(
                    error.message.contains("String/copy work limit"),
                    "{}",
                    error.message
                );
                break;
            }
        }
    }
    assert!(calls > 0);
    assert_eq!(runtime.copy_bytes, 0);
    // A small repeated-prefix miss must pay each actual candidate comparison.
    runtime.begin_job();
    runtime.constrain_job(
        usize::MAX,
        180,
        usize::MAX,
        Instant::now() + Duration::from_secs(10),
    );
    let error = runtime
        .string_last_index_of(
            "aaaaaaaaaaaaaaaa",
            &[Value::String("aaaaaaab".into())],
            &mut host,
        )
        .unwrap_err();
    assert!(error.exception.is_none());
    assert!(
        error.message.contains("String/copy work limit"),
        "{}",
        error.message
    );
    assert_eq!(runtime.copy_bytes, 0);
    runtime.begin_job();
    runtime.steps = 0;
    let error = runtime
        .string_last_index_of("abc", &[Value::String("b".into())], &mut host)
        .unwrap_err();
    assert!(error.exception.is_none());
    assert!(
        error.message.contains("Execution step limit"),
        "{}",
        error.message
    );
    runtime.begin_job();
    assert_eq!(
        runtime
            .execute("'😀a😀'.lastIndexOf('😀',2);", &mut host)
            .unwrap()
            .to_text(),
        "0"
    );
    assert_eq!((host.gets, host.sets, host.calls), (0, 0, 0));
}

#[test]
fn string_last_index_utf16_exact_receiver_search_and_scalar_pair_halves() {
    succeeds(r#"
var f=String.prototype.lastIndexOf,cases=[
['x\ud800y\ud800','\ud800',undefined,3],
['x\ud800y\ud800','\ud800',2,1],
['\udfffA\ud800','\udfff',undefined,0],
['😀','\ude00',undefined,1],['😀','\ud83d',1,0],
['A\ud800\udc00B\ud800','\ud800',undefined,4],
['A\ud800\udc00B\ud800','\ud800',3,1],
['A\u0000\ud800\u0000','\u0000',undefined,3],
['\ud800','',undefined,1],['\ud800','',-Infinity,0],
['\ud800','\ud800x',Infinity,-1],['\ud800','\ufffd',undefined,-1]
];
for(var c of cases){if(f.call(c[0],c[1],c[2])!==c[3])throw 'exact UTF16 reverse search';}
if('\ud800x\ud800'.lastIndexOf('\ud800')!==2||f.call(new String('😀x'),'\ud83d')!==0)throw 'raw primitive and represented boxed route';
"#);
}

#[test]
fn string_last_index_utf16_ordered_conversion_reentry_and_immutable_capture() {
    succeeds(r#"
var f=String.prototype.lastIndexOf,order='',current='A\ud800B\ud800';
var receiver={toString:function(){order+='R';return current;},valueOf:function(){throw 'receiver hint';}};
var search={toString:function(){order+='S';current='changed';if(f.call('😀','\ude00')!==1)throw 'nested raw search';return '\ud800';},valueOf:function(){throw 'search hint';}};
var position={valueOf:function(){order+='P';current='changed again';return NaN;},toString:function(){throw 'position hint';}};
if(f.call(receiver,search,position)!==3||order!=='RSP'||current!=='changed again')throw 'captured exact values';
order='';if(f.call({toString:function(){order+='R';return '\ud800';}},{toString:function(){order+='S';return ''; }},{valueOf:function(){order+='P';return Infinity;}})!==1||order!=='RSP')throw 'empty still converts';
"#);
}

#[test]
fn string_last_index_utf16_abrupt_search_and_position_keep_native_order() {
    succeeds(r#"
var f=String.prototype.lastIndexOf,marker={},calls=0;
try{f.call('\ud800',{toString:function(){calls++;throw marker;}},{valueOf:function(){throw 'position ran';}});throw 'missing abrupt search';}catch(e){if(e!==marker)throw e;}
if(calls!==1)throw 'search converted twice';
try{f.call('\ud800',{toString:function(){calls++;return '\ud800';}},{valueOf:function(){calls++;throw marker;}});throw 'missing abrupt position';}catch(e){if(e!==marker)throw e;}
if(calls!==3)throw 'position conversion order';
try{f.call(null,{toString:function(){throw 'search on null';}});throw 'missing null receiver';}catch(e){if(!(e instanceof TypeError))throw e;}
if(f.call('\ud800','\ud800')!==0)throw 'abrupt recovery';
"#);
}

#[test]
fn string_last_index_utf16_workspace_refusal_refunds_exact_temporary_backing_and_recovers() {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    let receiver=Value::StringUnits(vec![0xd800,65,0xd800]);let pattern=Value::StringUnits(vec![0xd800]);
    let retained=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    for headroom in [0,1,2,5,7,9] {
        runtime.begin_job();runtime.limits.heap.retained_values=retained+headroom;
        let error=runtime.string_last_index_value(&receiver,&[pattern.clone()],&mut host).unwrap_err();
        assert!(error.exception.is_none());assert!(error.message.contains("workspace limit"),"{headroom}: {}",error.message);
        assert_eq!(runtime.value_bytes,retained,"headroom {headroom}");
    }
    runtime.begin_job();runtime.limits.heap.retained_values=retained+10;
    assert_eq!(runtime.string_last_index_value(&receiver,&[pattern.clone()],&mut host).unwrap().to_text(),"2");
    assert_eq!(runtime.value_bytes,retained);
    runtime.limits.heap.retained_values=limit;runtime.begin_job();
    assert_eq!(runtime.string_last_index_value(&receiver,&[pattern],&mut host).unwrap().to_text(),"2");
    assert_eq!(runtime.value_bytes,retained);assert_eq!((host.gets,host.sets,host.calls),(0,0,0));
}

#[test]
fn string_last_index_utf16_copy_step_and_string_refusals_leave_fresh_search_usable() {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    let receiver=Value::StringUnits(vec![0xd800,65,0xd800]);let pattern=Value::StringUnits(vec![0xd800]);
    let retained=runtime.value_bytes;let cap=runtime.limits.capacity.string_bytes;
    runtime.begin_job();runtime.copy_bytes=0;
    let error=runtime.string_last_index_value(&receiver,&[pattern.clone()],&mut host).unwrap_err();
    assert!(error.message.contains("work limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.begin_job();runtime.steps=0;
    let error=runtime.string_last_index_value(&receiver,&[pattern.clone()],&mut host).unwrap_err();
    assert!(error.message.contains("step limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.begin_job();runtime.limits.capacity.string_bytes=1;
    let error=runtime.string_last_index_value(&receiver,&[pattern.clone()],&mut host).unwrap_err();
    assert!(error.message.contains("String limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.limits.capacity.string_bytes=cap;runtime.begin_job();
    assert_eq!(runtime.string_last_index_value(&receiver,&[pattern],&mut host).unwrap().to_text(),"2");
    assert_eq!(runtime.value_bytes,retained);
}

#[test]
fn string_last_index_utf16_late_workspace_refusal_preserves_author_publications() {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    runtime.execute("var published='before',probe={toString:function(){published='search';return '\\ud800';}},position={valueOf:function(){published='position';return Infinity;}};",&mut host).unwrap();
    let search=runtime.execute("probe;",&mut host).unwrap();let position=runtime.execute("position;",&mut host).unwrap();
    let retained=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values=retained+1024;runtime.begin_job();
    let error=runtime.string_last_index_value(&Value::StringUnits(vec![0xd800;4096]),&[search,position],&mut host).unwrap_err();
    assert!(error.message.contains("workspace limit"),"{}",error.message);
    assert!(runtime.value_bytes<=retained+1024);
    runtime.limits.heap.retained_values=limit;runtime.begin_job();
    assert_eq!(runtime.execute("published;",&mut host).unwrap().to_text(),"position");
    assert_eq!(runtime.execute("'\\ud800x\\ud800'.lastIndexOf('\\ud800');",&mut host).unwrap().to_text(),"2");
}

#[test]
fn string_index_utf16_exact_units_positions_empty_and_scalar_halves() {
    succeeds(r#"
var f=String.prototype.indexOf,cases=[
['A\ud800B\ud800','\ud800',undefined,1],['A\ud800B\ud800','\ud800',2,3],
['😀x😀','\ude00',NaN,1],['😀x😀','\ud83d',1,3],
['\ud800','',Infinity,1],['\ud800','',-Infinity,0],['\ud800','',NaN,0],
['\ud800','\ud800',Infinity,-1],['\ud800','\ud800',-2,0],
['\ud800','\ud800x',0,-1],['\ud800','\ufffd',0,-1],
['A\u0000\ud800\u0000','\u0000',2,3],['ababa','aba',1.9,2],
['undefined',undefined,0,0],['null',null,0,0]
];
for(var c of cases){if(f.call(c[0],c[1],c[2])!==c[3])throw 'exact forward search';}
if('\ud800x'.indexOf('\ud800')!==0||f.call(new String('😀x'),'\ude00')!==1)throw 'represented routes';
if('/a/'.indexOf(/a/)!==0)throw 'regexp is stringified';
"#);
}

#[test]
fn string_index_utf16_ordered_hooks_reentry_and_captured_values() {
    succeeds(r#"
var f=String.prototype.indexOf,order='',current='A\ud800B\ud800';
var receiver={toString:function(){order+='R';return current;},valueOf:function(){throw 'receiver hint';}};
var search={toString:function(){order+='S';current='changed';if(f.call('😀','\ude00')!==1)throw 'nested';return '\ud800';},valueOf:function(){throw 'search hint';}};
var position={valueOf:function(){order+='P';current='again';return 2;},toString:function(){throw 'position hint';}};
if(f.call(receiver,search,position)!==3||order!=='RSP'||current!=='again')throw 'conversion order';
order='';if(f.call({toString:function(){order+='R';return '\ud800';}},{toString:function(){order+='S';return '';}},{valueOf:function(){order+='P';return Infinity;}})!==1||order!=='RSP')throw 'empty conversion';
order='';if(f.call('x',{toString:function(){order+='S';return 'long';}},{valueOf:function(){order+='P';return 0;}})!==-1||order!=='SP')throw 'long conversion';
"#);
}

#[test]
fn string_index_utf16_abrupt_conversion_null_and_fresh_recovery() {
    succeeds(r#"
var f=String.prototype.indexOf,marker={},calls=0;
try{f.call('\ud800',{toString:function(){calls++;throw marker;}},{valueOf:function(){throw 'position ran';}});throw 'missing search throw';}catch(e){if(e!==marker)throw e;}
try{f.call('\ud800',{toString:function(){calls++;return '\ud800';}},{valueOf:function(){calls++;throw marker;}});throw 'missing position throw';}catch(e){if(e!==marker)throw e;}
if(calls!==3)throw 'duplicate conversion';
try{f.call(null,{toString:function(){throw 'search on null';}});throw 'missing null error';}catch(e){if(!(e instanceof TypeError))throw e;}
try{f.call('\ud800',Symbol('x'));throw 'missing symbol error';}catch(e){if(!(e instanceof TypeError))throw e;}
if(f.call('\ud800','\ud800')!==0)throw 'fresh recovery';
"#);
}
#[test]
fn string_index_utf16_workspace_refusal_refunds_exact_temporary_backing_and_recovers() {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    let receiver=Value::StringUnits(vec![0xd800,65,0xd800]);let pattern=Value::StringUnits(vec![0xd800]);
    let retained=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    for headroom in [0,1,2,5,7,9] {
        runtime.begin_job();runtime.limits.heap.retained_values=retained+headroom;
        let error=runtime.string_index_value(&receiver,&[pattern.clone()],false,&mut host).unwrap_err();
        assert!(error.exception.is_none());assert!(error.message.contains("workspace limit"),"{headroom}: {}",error.message);
        assert_eq!(runtime.value_bytes,retained,"headroom {headroom}");
    }
    runtime.begin_job();runtime.limits.heap.retained_values=retained+10;
    assert_eq!(runtime.string_index_value(&receiver,&[pattern.clone()],false,&mut host).unwrap().to_text(),"0");
    assert_eq!(runtime.value_bytes,retained);
    runtime.limits.heap.retained_values=limit;runtime.begin_job();
    assert_eq!(runtime.string_index_value(&receiver,&[pattern],false,&mut host).unwrap().to_text(),"0");
    assert_eq!(runtime.value_bytes,retained);assert_eq!((host.gets,host.sets,host.calls),(0,0,0));
}

#[test]
fn string_index_utf16_copy_step_and_string_refusals_leave_fresh_search_usable() {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    let receiver=Value::StringUnits(vec![0xd800,65,0xd800]);let pattern=Value::StringUnits(vec![0xd800]);
    let retained=runtime.value_bytes;let cap=runtime.limits.capacity.string_bytes;
    runtime.begin_job();runtime.copy_bytes=0;
    let error=runtime.string_index_value(&receiver,&[pattern.clone()],false,&mut host).unwrap_err();
    assert!(error.message.contains("work limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.begin_job();runtime.steps=0;
    let error=runtime.string_index_value(&receiver,&[pattern.clone()],false,&mut host).unwrap_err();
    assert!(error.message.contains("step limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.begin_job();runtime.limits.capacity.string_bytes=1;
    let error=runtime.string_index_value(&receiver,&[pattern.clone()],false,&mut host).unwrap_err();
    assert!(error.message.contains("String limit"));assert_eq!(runtime.value_bytes,retained);
    runtime.limits.capacity.string_bytes=cap;runtime.begin_job();
    assert_eq!(runtime.string_index_value(&receiver,&[pattern],false,&mut host).unwrap().to_text(),"0");
    assert_eq!(runtime.value_bytes,retained);
}

#[test]
fn string_index_utf16_late_workspace_refusal_preserves_author_publications() {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    runtime.execute("var published='before',probe={toString:function(){published='search';return '\\ud800';}},position={valueOf:function(){published='position';return 0;}};",&mut host).unwrap();
    let search=runtime.execute("probe;",&mut host).unwrap();let position=runtime.execute("position;",&mut host).unwrap();
    let retained=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values=retained+1024;runtime.begin_job();
    let error=runtime.string_index_value(&Value::StringUnits(vec![0xd800;4096]),&[search,position],false,&mut host).unwrap_err();
    assert!(error.message.contains("workspace limit"),"{}",error.message);
    assert!(runtime.value_bytes<=retained+1024);
    runtime.limits.heap.retained_values=limit;runtime.begin_job();
    assert_eq!(runtime.execute("published;",&mut host).unwrap().to_text(),"position");
    assert_eq!(runtime.execute("'\\ud800x\\ud800'.indexOf('\\ud800');",&mut host).unwrap().to_text(),"0");
}


#[test]
fn string_index_utf16_empty_and_impossible_ranges_need_no_unit_scratch() {
    let mut runtime=Runtime::for_test();let mut host=NoHost::default();
    let receiver=Value::StringUnits(vec![0xd800,65,0xd800]);
    let retained=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    runtime.limits.heap.retained_values=retained;runtime.begin_job();
    assert_eq!(runtime.string_index_value(&receiver,&[Value::String(String::new()),Value::Number(f64::INFINITY)],false,&mut host).unwrap().to_text(),"3");
    assert_eq!(runtime.value_bytes,retained);
    runtime.limits.heap.retained_values=retained+2;runtime.begin_job();
    assert_eq!(runtime.string_index_value(&receiver,&[Value::StringUnits(vec![0xd800]),Value::Number(f64::INFINITY)],false,&mut host).unwrap().to_text(),"-1");
    assert_eq!(runtime.value_bytes,retained);
    runtime.limits.heap.retained_values=retained+8;runtime.begin_job();
    assert_eq!(runtime.string_index_value(&receiver,&[Value::StringUnits(vec![0xd800;4])],false,&mut host).unwrap().to_text(),"-1");
    assert_eq!(runtime.value_bytes,retained);runtime.limits.heap.retained_values=limit;
}
