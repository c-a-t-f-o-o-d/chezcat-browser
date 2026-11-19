//! Original no-await async-call controls. Suspension is a separate checkpoint.
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

fn succeeds(runtime: &mut Runtime, source: &str) {
    assert_eq!(
        runtime.execute(source, &mut NoHost).unwrap().to_text(),
        "ok"
    );
}

fn checkpoint(runtime: &mut Runtime) {
    let diagnostics = runtime.microtask_checkpoint(&mut NoHost).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn for_await_async_iterator_suspends_steps_and_closes_before_finally() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', reads=0, steps=0, close, delivered, wrapped=Promise.resolve(17);
var iterator={return:function(){
    if(this!==iterator || arguments.length!==0)throw 'close receiver';
    trace+='close|';
    return new Promise(function(resolve){close=resolve;});
}};
Object.defineProperty(iterator,'next',{get:function(){
    reads++;
    return function(){
        if(this!==iterator || arguments.length!==0)throw 'next receiver';
        trace+='next|';
        steps++;
        return Promise.resolve({done:false,value:steps===1?wrapped:23});
    };
}});
var source={[Symbol.asyncIterator]:function(){if(this!==source)throw 'method receiver';return iterator;}};
async function walk(){
    try {
        for await (const value of source) {
            if(steps===1){if(value!==wrapped)throw 'native async values stay intact';trace+='first|';continue;}
            if(value!==23)throw 'second value';
            trace+='second|';
            break;
        }
        trace+='after|';
    } finally { trace+='finally|'; }
    return 42;
}
walk().then(function(value){delivered=value;trace+='resolved|';});
if(trace!=='next|'||reads!==1||delivered!==undefined)throw 'step did not suspend';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='next|first|next|second|close|'||reads!==1||delivered!==undefined)throw trace;close({});'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='next|first|next|second|close|after|finally|resolved|'||delivered!==42)throw trace;'ok';");
}

#[test]
fn for_await_async_iterator_preserves_throw_and_skips_closing_failed_step() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var fault={}, stepFault={}, recorded='', closes=0;
function source(failing){return {
    [Symbol.asyncIterator]:function(){return this;},
    next:function(){return failing?Promise.reject(stepFault):Promise.resolve({value:1,done:false});},
    return:function(){closes++;return Promise.reject('close failure');}
};}
async function original(){try{for await(let value of source(false)){throw fault;}}
    catch(error){if(error===fault)recorded+='original|';else throw error;}}
async function badStep(){try{for await(let value of source(true)){throw 'unreachable';}}
    catch(error){if(error===stepFault)recorded+='step|';else throw error;}}
async function badClose(){try{for await(let value of source(false)){break;}}
    catch(error){if(error==='close failure')recorded+='close|';else throw error;}}
original();badStep();badClose();
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(recorded!=='step|original|close|'||closes!==2)throw recorded;'ok';");
}

#[test]
fn async_function_execution_prefix_this_arguments_defaults_and_lexical_arrow() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var trace='',defaults=0,receiver={base:3};
async function ordinary(value=(defaults++,2),...rest){
    trace+='ordinary|';
    if(this!==receiver||arguments.length!==2||arguments[0]!==5||arguments[1]!==7||rest.length!==1)throw 'ordinary call context';
    var original=value;value=19;
    if(arguments[0]!==5)throw 'non-simple parameters must keep unmapped arguments';
    return this.base+original+rest[0];
}
function factory(seed){
    var owner=this,args=arguments;
    return async value=>{
        trace+='arrow|';
        if(this!==owner||arguments!==args||arguments[0]!==11)throw 'lexical async arrow context';
        return this.base+arguments[0]+value;
    };
}
var expression=async function named(value){trace+='expression|';return value+1;};
async function defaulted(value=(defaults++,4)){return value;}
var first=ordinary.call(receiver,5,7),arrow=factory.call(receiver,11);
var second=arrow.call({base:99},2),third=expression(8),fourth=defaulted(undefined);
trace+='caller|';
first.then(function(value){if(value!==15)throw 'ordinary result';trace+='first|';});
second.then(function(value){if(value!==16)throw 'arrow result';trace+='second|';});
third.then(function(value){if(value!==9)throw 'expression result';trace+='third|';});
fourth.then(function(value){if(value!==4)throw 'default result';trace+='fourth|';});
if(trace!=='ordinary|arrow|expression|caller|'||defaults!==1)throw 'synchronous body or premature observer';
var refused=false;
try{new ordinary();}catch(error){if(error instanceof TypeError)refused=true;else throw error;}
if(!refused||trace!=='ordinary|arrow|expression|caller|')throw 'async function constructed';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(trace!=='ordinary|arrow|expression|caller|first|second|third|fourth|')throw 'observer FIFO';'ok';",
    );
}

#[test]
fn async_function_execution_real_distinct_promises_and_return_assimilation() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var NativePromise=Promise,trace='',observed=0;
async function pass(value){return value;}
async function fallthrough(){}
async function emptyReturn(){return;}
var original=NativePromise.resolve(5),wrapped=pass(original),first=pass(3),second=pass(3);
var implicit=fallthrough(),explicit=emptyReturn();
for(var promise of [wrapped,first,second,implicit,explicit]){
    if(Object.getPrototypeOf(promise)!==NativePromise.prototype||NativePromise.resolve(promise)!==promise)throw 'genuine intrinsic Promise';
}
if(wrapped===original||first===second)throw 'async calls must create distinct Promises';
wrapped.then(function(value){if(value!==5)throw 'Promise adoption';observed+=1;});
first.then(function(value){if(value!==3)throw 'primitive return';observed+=2;});
implicit.then(function(value){if(value!==undefined)throw 'fallthrough';observed+=4;});
explicit.then(function(value){if(value!==undefined)throw 'bare return';observed+=8;});
var thenable={};
Object.defineProperty(thenable,'then',{get:function(){
    trace+='get|';
    return function(resolve,reject){
        if(this!==thenable)throw 'thenable receiver';
        trace+='then|';resolve(23);reject('ignored second settlement');throw 'ignored after resolution';
    };
}});
async function assimilate(value){trace+='body|';return value;}
var adopted=assimilate(thenable);
trace+='caller|';
NativePromise.prototype.then.call(adopted,function(value){
    if(value!==23)throw 'thenable fulfillment';trace+='resolved:23|';observed+=16;
});
NativePromise.resolve().then(function(){trace+='ordinary|';});
var getterMarker={},broken={};
Object.defineProperty(broken,'then',{get:function(){throw getterMarker;}});
var getterFailure=pass(broken);
getterFailure.catch(function(reason){if(reason!==getterMarker)throw 'then getter rejection identity';observed+=32;});
if(trace!=='body|get|caller|'||observed!==0)throw 'assimilation synchronous getter or deferred call';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(trace!=='body|get|caller|then|ordinary|resolved:23|'||observed!==63)throw 'assimilation jobs or completion';'ok';",
    );
}

#[test]
fn async_function_execution_body_and_parameter_throws_reject_identity_in_fifo_order() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var trace='',entered=0,parameterMarker={},bodyMarker={};
Promise.resolve().then(function(){trace+='before|';});
function explode(){trace+='default|';throw parameterMarker;}
async function failParameter(value=explode()){entered++;return value;}
async function failBody(){trace+='body|';throw bodyMarker;}
var first,second;
try{first=failParameter();second=failBody();}catch(error){throw 'author exception escaped async call';}
trace+='caller|';
first.catch(function(reason){if(reason!==parameterMarker)throw 'parameter reason identity';trace+='parameter-rejected|';});
second.then(function(){throw 'body unexpectedly fulfilled';},function(reason){
    if(reason!==bodyMarker)throw 'body reason identity';trace+='body-rejected|';
});
Promise.resolve().then(function(){trace+='after|';});
if(entered!==0||trace!=='default|body|caller|')throw 'parameter/body synchronous order';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(trace!=='default|body|caller|before|parameter-rejected|body-rejected|after|'||entered!==0)throw 'rejection observer FIFO';'ok';",
    );
}

#[test]
fn async_function_execution_promises_use_function_owner_intrinsics_and_restore_caller() {
    let mut runtime = Runtime::for_test();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let values = runtime
        .with_intrinsic_realm(Some(realm), |runtime| {
            runtime
                .execute(
                    r#"
var SavedPromise=Promise,SavedArray=Array;
async function foreign(value){return [value,Array];}
Promise=function(){throw 'mutable child Promise global used';};
"#,
                    &mut NoHost,
                )
                .map_err(Fault::new)?;
            ["foreign", "SavedPromise", "SavedArray"]
                .iter()
                .map(|name| runtime.binding_value(runtime.global_environment, name))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap();
    for (name, value) in ["foreign", "ChildPromise", "ChildArray"]
        .into_iter()
        .zip(values)
    {
        runtime.define_global(name, value).unwrap();
    }
    succeeds(
        &mut runtime,
        r#"
var SavedPromise=Promise,SavedArray=Array,marker={},observed=0;
async function local(value){return value;}
Promise=function(){throw 'mutable parent Promise global used';};
var own=local(17),other=foreign(marker);
if(Object.getPrototypeOf(own)!==SavedPromise.prototype||SavedPromise.resolve(own)!==own)throw 'parent intrinsic Promise';
if(Object.getPrototypeOf(other)!==ChildPromise.prototype||ChildPromise.resolve(other)!==other||other instanceof SavedPromise)throw 'owner intrinsic Promise';
own.then(function(value){if(value!==17||Array!==SavedArray)throw 'parent completion realm';observed++;});
other.then(function(value){
    if(value[0]!==marker||value[1]!==ChildArray||Object.getPrototypeOf(value)!==ChildArray.prototype)throw 'foreign body realm';
    if(Array!==SavedArray||Object.getPrototypeOf([])!==SavedArray.prototype)throw 'observer caller realm';
    observed+=2;
});
if(observed!==0)throw 'foreign observer ran synchronously';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(observed!==3||Array!==SavedArray||Object.getPrototypeOf([])!==SavedArray.prototype)throw 'caller restore';'ok';",
    );
    assert_eq!(
        (runtime.active_realm, runtime.global_environment),
        (None, 0)
    );
}

#[test]
fn async_function_execution_unhandled_rejection_reports_once_then_keeps_late_identity() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var marker={},late=0;
async function fail(){throw marker;}
var rejected=fail();
if(Object.getPrototypeOf(rejected)!==Promise.prototype)throw 'rejected Promise brand';
'ok';
"#,
    );
    let diagnostics = runtime.microtask_checkpoint(&mut NoHost).unwrap();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("Unhandled Promise rejection: "));
    assert!(runtime
        .microtask_checkpoint(&mut NoHost)
        .unwrap()
        .is_empty());
    runtime.begin_job();
    succeeds(
        &mut runtime,
        "rejected.catch(function(reason){if(reason!==marker)throw 'late reason identity';late++;});if(late!==0)throw 'synchronous catch';'ok';",
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(late!==1)throw 'missing late rejection handler';'ok';",
    );
}

#[test]
fn async_function_execution_fatal_body_and_parameter_limits_do_not_become_rejections() {
    for target in ["failBody", "failParameter"] {
        let mut runtime = Runtime::for_test();
        succeeds(
            &mut runtime,
            r#"
var caught=0,finalized=0,entered=0,bodyStarted=0,parameterStarted=0,tail=0,after=0,published=null;
async function failBody(){
    bodyStarted++;
    try{while(true){}}catch(error){caught++;}finally{finalized++;}
}
function burn(){parameterStarted++;while(true){}}
async function failParameter(value=burn()){entered++;return value;}
Promise.resolve().then(function(){tail++;});
'ok';
"#,
        );
        runtime.begin_job();
        runtime.constrain_job(4096, usize::MAX, usize::MAX, runtime.job_deadline);
        let source = format!(
            "try{{published={target}();}}catch(error){{caught+=10;}}finally{{finalized+=10;}}after=1;"
        );
        let error = runtime.execute(&source, &mut NoHost).unwrap_err();
        assert!(error.contains("limit"), "{target}: {error}");
        assert!(runtime.job_failed_fatally);
        assert!(!runtime.has_microtasks());
        assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
        assert_eq!(
            (runtime.active_realm, runtime.global_environment),
            (None, 0)
        );
        assert!(runtime.transient_registration_is_empty());
        runtime.begin_job();
        succeeds(
            &mut runtime,
            "if(caught!==0||finalized!==0||entered!==0||tail!==0||after!==0||published!==null)throw 'fatal async completion became ordinary rejection';'ok';",
        );
        let (body_started, parameter_started) = if target == "failBody" { (1, 0) } else { (0, 1) };
        succeeds(
            &mut runtime,
            &format!(
                "if(bodyStarted!=={body_started}||parameterStarted!=={parameter_started})throw 'budget failed before intended async path';'ok';"
            ),
        );
        checkpoint(&mut runtime);
    }
}
