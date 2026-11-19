//! Original finite await controls: owned suspension, resumption and refusal.
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
fn await_execution_member_read_uses_original_receiver_and_evaluates_key_after_resume() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, reason={}, desired={}, received, caught=0;
var ready=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var member=Symbol('read'), marker={};
var receiver={marker,get [member](){
    if(this.marker!==marker)throw 'getter receiver';
    trace+='getter|';return desired;
}};
function key(){
    trace+='key|';
    return {[Symbol.toPrimitive](hint){if(hint!=='string')throw 'key hint';trace+='convert|';return member;}};
}
function relay(input){if(input!==4)throw 'awaited argument';trace+='relay|';var value=receiver;receiver=null;return value;}
async function read(){return relay(await ready)[key()];}
async function skip(){
    try { return (await failed)[key()]; }
    catch(error){if(error!==reason)throw error;caught++;}
}
async function throws(){
    var holder={get value(){throw reason;}};
    try {return (await Promise.resolve(holder)).value;}
    catch(error){if(error!==reason)throw error;caught++;}
}
read().then(function(value){received=value;});
skip();
throws();
if(trace!==''||received!==undefined||caught)throw 'read before suspension';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!==''||caught!==1)throw 'static getter exception';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);reject(reason);if(trace!=='')throw 'synchronous read';'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='relay|key|convert|getter|'||received!==desired||caught!==2||receiver!==null)throw 'member result, order or rejection';'ok';");
}

#[test]
fn await_execution_logical_right_operands_keep_short_circuit_values_and_order() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, token={}, result=0, same=false;
var gate=new Promise(function(resolve){release=resolve;});
function mark(value,label){trace+=label+'|';return value;}
function never(){throw 'skipped operand ran';}
function invoke(value){trace+='call:'+value+'|';return value;}
async function run(){
    try {
        mark(false,'skip-and') && absent(await never()),
        mark(true,'take-and') && invoke(await gate);
        let object=mark(token,'skip-or') || absent(await never()),
            zero=mark(0,'skip-nullish') ?? absent(await never()),
            value=mark(0,'take-or') || invoke(await Promise.resolve(2));
        if(object!==token||zero!==0)throw 'left value';
        return mark(null,'take-nullish') ?? await Promise.resolve(value+1);
    } finally { trace+='finally|'; }
}
async function preserved(){return token || invoke(await never());}
run().then(function(value){result=value;});
preserved().then(function(value){same=value===token;});
if(trace!=='skip-and|take-and|'||result||same)throw 'synchronous prefix';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(!same||result||trace!=='skip-and|take-and|')throw 'short-circuit branch suspended';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);if(result)throw 'synchronous resolution';'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(result!==3||trace!=='skip-and|take-and|call:4|skip-or|skip-nullish|take-or|call:2|take-nullish|finally|')
    throw 'logical evaluation order or value';
'ok';
"#);
}

#[test]
fn await_execution_declarators_initialize_in_order_and_leave_later_bindings_in_tdz() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', openFirst, openSecond, reject, reason={}, result=0, caught=0;
var first=new Promise(function(resolve){openFirst=resolve;});
var second=new Promise(function(resolve){openSecond=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var retained, neverInitialized;
function receive(value){trace+='received|';return value+1;}
async function work(){
    var hoisted, seed=(trace+='seed|',1), start=await first, after=start+seed;
    const middle=receive(await second), Klass=class{}, end=(trace+='end|',middle+after);
    let empty, final=empty===undefined&&hoisted===undefined&&Klass.name==='Klass'?end:0;
    return final;
}
async function stop(){
    try {
        let done=(retained=()=>done,neverInitialized=()=>later,3),
            broken=await failed, later=(trace+='late|',4);
    } catch(error) { if(error!==reason)throw error; caught++; }
}
work().then(function(value){result=value;});
stop();
if(trace!=='seed|'||retained()!==3||result||caught)throw 'prefix declaration';
var tdz=false;try{neverInitialized();}catch(error){tdz=error.name==='ReferenceError';}
if(!tdz)throw 'early initialization';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "openFirst(2);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='seed|'||retained()!==3||caught!==1||result)throw 'between declarations';
var tdz=false;try{neverInitialized();}catch(error){tdz=error.name==='ReferenceError';}
if(!tdz)throw 'rejected declaration initialized a later binding';
'ok';
"#);
    runtime.begin_job();
    succeeds(&mut runtime, "openSecond(5);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='seed|received|end|'||result!==9)throw 'declaration order or result';'ok';");
}

#[test]
fn await_execution_sequence_keeps_prefix_suffix_and_final_value_in_order() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', releaseFirst, releaseSecond, reject, reason={}, final=0, caught=0;
var first=new Promise(function(resolve){releaseFirst=resolve;});
var second=new Promise(function(resolve){releaseSecond=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
function receive(value){trace+='received:'+value+'|';return 99;}
async function run(){
    try {
        let result=(trace+='before|',receive(await first),(trace+='between|',await second),trace+='after|',7);
        return (trace+='return|', await Promise.resolve(result));
    } finally { trace+='finally|'; }
}
async function skip(){
    try { receive(await failed),trace+='late|'; }
    catch(e){if(e!==reason)throw e;caught++;}
}
run().then(function(value){final=value;});
skip();
if(trace!=='before|'||final||caught)throw 'prefix timing';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "releaseFirst(4);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='before|received:4|between|'||final||caught!==1)throw 'pause or rejection';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "releaseSecond(8);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='before|received:4|between|after|return|finally|'||final!==7)throw 'order or final value';'ok';");
}

#[test]
fn await_execution_try_finally_keeps_completion_order_identity_and_roots() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, reason={}, replacement={};
var ready=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var recorded=0, rejected=0, replaced=0, recovered=0;
function plus(value){trace+='call|';return value+3;}
async function keep(){
    try {
        let result=plus(await ready);
        return function(){return result;};
    } finally {
        trace+='kept|';
        var trash={};for(var i=0;i<900;i++)trash.callback=function(){return i;};
    }
}
async function preserve(){
    try { try { await failed; } finally { trace+='inner|'; } }
    finally { trace+='outer|'; }
}
async function handle(){
    try { await failed; }
    catch(e){if(e!==reason)throw e;trace+='catch|';}
    finally { trace+='handled|'; }
    return 8;
}
async function replaceRejection(){
    try { return await failed; } finally { return replacement; }
}
async function replaceReturn(){
    try { return await ready; } finally { throw replacement; }
}
async function normal(){
    try { await ready;trace+='body|'; } finally { trace+='normal|'; }
    trace+='after|';
}
keep().then(function(fn){recorded=fn();});
preserve().catch(function(error){if(error!==reason)throw error;rejected++;});
handle().then(function(value){recovered=value;});
replaceRejection().then(function(value){if(value!==replacement)throw 'return replacement';replaced++;});
replaceReturn().catch(function(error){if(error!==replacement)throw error;replaced++;});
normal();
if(trace!==''||recorded||rejected||replaced||recovered)throw 'finally started before settlement';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);reject(reason);if(trace!=='')throw 'finally ran synchronously';'ok';");
    checkpoint(&mut runtime);
    assert!(runtime.function_arena.retired > 0);
    succeeds(&mut runtime, r#"
if(trace!=='call|kept|body|normal|after|inner|outer|catch|handled|'||recorded!==7||rejected!==1||replaced!==2||recovered!==8)
    throw 'completion order, replacement or retention';
'ok';
"#);
}

#[test]
fn await_execution_call_argument_keeps_the_resolved_function_through_collection() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(&mut runtime, r#"
var trace='', release, observed=0, rejected=0, reason={};
var gate=new Promise(function(resolve){release=resolve;});
async function work(seed){
    var dispatch=function(value){'use strict';if(this!==undefined)throw 'receiver';trace+='call|';return seed+value;};
    function input(){trace+='operand|';dispatch=function(){throw 'callee read twice';};return gate;}
    var result=dispatch(await input());
    return result;
}
async function rejectCall(){
    function unused(){throw 'called on rejection';}
    try{return unused(await Promise.reject(reason));}catch(e){if(e!==reason)throw e;rejected++;}
}
work(3).then(function(value){observed=value;});
rejectCall();
if(trace!=='operand|'||observed||rejected)throw 'synchronous invocation';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;'ok';");
    assert!(runtime.function_arena.retired > 0);
    succeeds(&mut runtime, "if(trace!=='operand|'||observed||rejected!==1)throw 'pending call or rejection';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);if(observed)throw 'synchronous resumption';'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='operand|call|'||observed!==7)throw 'call replay or lost callee';'ok';");
}

#[test]
fn await_execution_pending_gates_resume_across_jobs_in_fifo_without_replay() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var trace='',prefix=0,middle=0,suffix=0,observed=0,releaseFirst,releaseSecond;
var firstGate=new Promise(function(resolve){releaseFirst=resolve;});
var secondGate=new Promise(function(resolve){releaseSecond=resolve;});
async function work(){
    prefix++;trace+='start|';
    let first=await firstGate;
    middle++;trace+='first:'+first+'|';
    await secondGate;
    suffix++;trace+='second|';
    return first+3;
}
var pending=work();trace+='caller|';
pending.then(function(value){if(value!==8)throw 'await result';observed++;trace+='fulfilled:8|';});
Promise.resolve().then(function(){trace+='outside|';});
if(trace!=='start|caller|'||prefix!==1||middle!==0||suffix!==0||observed!==0)throw 'pending synchronous prefix';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(trace!=='start|caller|outside|'||prefix!==1||middle!==0||suffix!==0||observed!==0)throw 'pending checkpoint advanced';'ok';",
    );
    runtime.begin_job();
    succeeds(
        &mut runtime,
        r#"
Promise.resolve().then(function(){trace+='before-first|';});
releaseFirst(5);trace+='released-first|';
Promise.resolve().then(function(){trace+='after-first|';});
if(middle!==0||suffix!==0||observed!==0)throw 'settlement resumed synchronously';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(trace!=='start|caller|outside|released-first|before-first|first:5|after-first|'||prefix!==1||middle!==1||suffix!==0||observed!==0)throw 'first continuation or FIFO';'ok';",
    );
    runtime.begin_job();
    succeeds(
        &mut runtime,
        r#"
Promise.resolve().then(function(){trace+='before-second|';});
releaseSecond(9);trace+='released-second|';
Promise.resolve().then(function(){trace+='after-second|';});
if(suffix!==0||observed!==0)throw 'second settlement resumed synchronously';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(trace!=='start|caller|outside|released-first|before-first|first:5|after-first|released-second|before-second|second|after-second|fulfilled:8|'||prefix!==1||middle!==1||suffix!==1||observed!==1)throw 'second continuation replay or FIFO';'ok';",
    );
}

#[test]
fn await_execution_immediate_thenable_and_return_await_are_real_deferred_jobs() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var trace='',prefix=0,immediate=0,thenCalls=0,resumed=0,result=0,thenable={};
Object.defineProperty(thenable,'then',{get:function(){
    trace+='get|';
    return function(resolve,reject){
        if(this!==thenable)throw 'await thenable receiver';
        thenCalls++;trace+='then|';resolve(11);reject('ignored');resolve(99);
    };
}});
async function work(){
    prefix++;trace+='before|';
    await 7;
    immediate++;trace+='immediate|';
    var value=await thenable;
    resumed++;trace+='resumed:'+value+'|';
    return await Promise.resolve(value+2);
}
var pending=work();trace+='caller|';
pending.then(function(value){result=value;trace+='fulfilled:'+value+'|';});
Promise.resolve().then(function(){trace+='outside|';});
if(trace!=='before|caller|'||prefix!==1||immediate!==0||thenCalls!==0||resumed!==0||result!==0)throw 'immediate await failed to suspend';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(trace!=='before|caller|immediate|get|outside|then|resumed:11|fulfilled:13|'||prefix!==1||immediate!==1||thenCalls!==1||resumed!==1||result!==13)throw 'await assimilation order, value or replay';'ok';",
    );
}

#[test]
fn await_execution_branches_sync_catches_and_rejected_return_keep_reason_identity() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var prefix=0,selects=0,after=0,catches=0,observed=0,operandGets=0,operandCaught=0;
var rejectionMarker={},bodyMarker={},operandMarker={},rejectFirst,resolveSecond;
var firstGate=new Promise(function(resolve,reject){rejectFirst=reject;});
var secondGate=new Promise(function(resolve){resolveSecond=resolve;});
function choose(mode){selects++;return mode;}
async function flow(mode,gate){
    prefix++;
    try{
        if(choose(mode)){
            await gate;
            throw 'rejected branch unexpectedly resumed normally';
        }else{
            const value=await gate;
            if(value!==7)throw 'fulfilled branch value';
            after++;throw bodyMarker;
        }
    }catch(reason){
        if(reason!==(mode?rejectionMarker:bodyMarker))throw 'catch identity';
        catches++;return reason;
    }
}
async function relay(gate){return await gate;}
var explosive={};
Object.defineProperty(explosive,'value',{get:function(){operandGets++;throw operandMarker;}});
async function operand(){
    try{await explosive.value;}catch(reason){
        if(reason!==operandMarker)throw 'operand identity';operandCaught++;return reason;
    }
}
var first=flow(true,firstGate),second=flow(false,secondGate),propagated=relay(firstGate),synchronous=operand();
first.then(function(reason){if(reason!==rejectionMarker)throw 'fulfilled catch result';observed+=1;});
second.then(function(reason){if(reason!==bodyMarker)throw 'body catch result';observed+=2;});
propagated.catch(function(reason){if(reason!==rejectionMarker)throw 'return-await rejection identity';observed+=4;});
synchronous.then(function(reason){if(reason!==operandMarker)throw 'operand catch result';observed+=8;});
if(prefix!==2||selects!==2||after!==0||catches!==0||observed!==0||operandGets!==1||operandCaught!==1)throw 'branch or synchronous operand prefix';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(observed!==8||prefix!==2||selects!==2||after!==0||catches!==0)throw 'pending branches advanced';'ok';",
    );
    runtime.begin_job();
    succeeds(
        &mut runtime,
        "rejectFirst(rejectionMarker);resolveSecond(7);if(catches!==0||after!==0)throw 'synchronous rejection resume';'ok';",
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(observed!==15||prefix!==2||selects!==2||after!==1||catches!==2||operandGets!==1||operandCaught!==1)throw 'resumed catch, branch replay or reason identity';'ok';",
    );
}

#[test]
fn await_execution_pending_activation_keeps_unexported_closure_and_lexical_arrow_through_gc() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(
        &mut runtime,
        r#"
var owner={base:3},release,prefix=0,recovered=null,total=0;
var gate=new Promise(function(resolve){release=resolve;});
function factory(gate,seed){
    var originalThis=this,originalArguments=arguments;
    return async value=>{
        prefix++;
        var state=seed+value,holder={};
        holder.fn=function retained(){state++;return state;};
        var kept=holder.fn;holder.fn=null;holder=null;
        const delta=await gate;
        if(this!==originalThis||arguments!==originalArguments||arguments[0]!==gate||arguments[1]!==seed)throw 'resumed lexical context';
        if(kept()!==26)throw 'suspended closure value';
        return {callback:kept,total:this.base+delta+state};
    };
}
var arrow=factory.call(owner,gate,20),pending=arrow.call({base:99},5);arrow=null;
pending.then(function(value){recovered=value.callback;total=value.total;});
if(prefix!==1||recovered!==null||total!==0)throw 'pending closure prefix';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    runtime.begin_job();
    let deadline = runtime.job_deadline;
    succeeds(
        &mut runtime,
        "owner.base=7;var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;'ok';",
    );
    assert!(runtime.function_arena.retired > 0);
    assert_eq!(runtime.job_deadline, deadline);
    succeeds(
        &mut runtime,
        "if(prefix!==1||recovered!==null||total!==0)throw 'collection resumed or replayed work';'ok';",
    );
    runtime.begin_job();
    succeeds(&mut runtime, "release(3);'ok';");
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(prefix!==1||total!==36||typeof recovered!=='function'||recovered()!==27||recovered()!==28)throw 'lost activation closure, lexical context or state';'ok';",
    );
}

#[test]
fn await_execution_foreign_resume_uses_owner_environment_and_restores_caller_realm() {
    let mut runtime = Runtime::for_test();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let values = runtime
        .with_intrinsic_realm(Some(realm), |runtime| {
            runtime
                .execute(
                    r#"
var realmValue=17,OwnerArray=Array;
async function foreign(gate){
    const value=await gate;
    return [Array,realmValue,value];
}
"#,
                    &mut NoHost,
                )
                .map_err(Fault::new)?;
            ["foreign", "OwnerArray"]
                .iter()
                .map(|name| runtime.binding_value(runtime.global_environment, name))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap();
    for (name, value) in ["foreign", "ChildArray"].into_iter().zip(values) {
        runtime.define_global(name, value).unwrap();
    }
    succeeds(
        &mut runtime,
        r#"
var realmValue=91,ParentArray=Array,marker={},release,observed=0;
var gate=new Promise(function(resolve){release=resolve;});
foreign(gate).then(function(value){
    if(value[0]!==ChildArray||value[1]!==17||value[2]!==marker||Object.getPrototypeOf(value)!==ChildArray.prototype)throw 'resume owner realm';
    if(realmValue!==91||Array!==ParentArray||Object.getPrototypeOf([])!==ParentArray.prototype)throw 'observer realm';
    observed++;
});
if(observed!==0||realmValue!==91||Array!==ParentArray)throw 'caller realm before checkpoint';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(
        &mut runtime,
        "release(marker);if(observed!==0||realmValue!==91||Array!==ParentArray)throw 'caller realm after release';'ok';",
    );
    checkpoint(&mut runtime);
    succeeds(
        &mut runtime,
        "if(observed!==1||realmValue!==91||Array!==ParentArray)throw 'caller realm after resume';'ok';",
    );
    assert_eq!(
        (runtime.active_realm, runtime.global_environment),
        (None, 0)
    );
}

#[test]
fn await_execution_cancellation_drops_pending_and_queued_resumes_without_revival() {
    for queued in [false, true] {
        let mut runtime = Runtime::for_test();
        succeeds(
            &mut runtime,
            r#"
var release,prefix=0,resumed=0,observed=0,fresh=0;
var gate=new Promise(function(resolve){release=resolve;});
async function work(){prefix++;await gate;resumed++;return 5;}
var pending=work();pending.then(function(){observed++;});
if(prefix!==1||resumed!==0||observed!==0)throw 'cancel prefix';
'ok';
"#,
        );
        if queued {
            succeeds(&mut runtime, "release(1);'ok';");
            assert!(runtime.has_microtasks());
        }
        runtime.cancel_microtasks();
        assert!(!runtime.has_microtasks());
        runtime.begin_job();
        if !queued {
            succeeds(&mut runtime, "release(1);'ok';");
        }
        checkpoint(&mut runtime);
        succeeds(
            &mut runtime,
            r#"
if(prefix!==1||resumed!==0||observed!==0)throw 'cancelled continuation revived';
async function next(){await 0;fresh++;}
next();
if(fresh!==0)throw 'fresh await did not suspend';
'ok';
"#,
        );
        checkpoint(&mut runtime);
        succeeds(
            &mut runtime,
            "if(prefix!==1||resumed!==0||observed!==0||fresh!==1)throw 'cancel isolation or fresh recovery';'ok';",
        );
    }
}

#[test]
fn await_execution_resumed_fatal_budget_keeps_shared_allowance_and_cancels_tail() {
    let mut runtime = Runtime::for_test();
    succeeds(
        &mut runtime,
        r#"
var release,prefix=0,entered=0,caught=0,finalized=0,observed=0,tail=0;
var gate=new Promise(function(resolve){release=resolve;});
async function work(){
    prefix++;
    try{await gate;entered++;while(true){}}catch(error){caught++;}finally{finalized++;}
}
var pending=work();pending.catch(function(){observed++;});
if(prefix!==1||entered!==0)throw 'fatal pending prefix';
'ok';
"#,
    );
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(
        &mut runtime,
        "release(1);Promise.resolve().then(function(){tail++;});'ok';",
    );
    runtime.constrain_job(4096, usize::MAX, usize::MAX, runtime.job_deadline);
    let deadline = runtime.job_deadline;
    let allowance = runtime.job_allowance;
    let error = runtime.microtask_checkpoint(&mut NoHost).unwrap_err();
    assert!(error.contains("limit"), "{error}");
    assert!(runtime.job_failed_fatally);
    assert!(!runtime.has_microtasks());
    assert_eq!(runtime.job_deadline, deadline);
    assert_eq!(runtime.job_allowance, allowance);
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert_eq!(
        (runtime.active_realm, runtime.global_environment),
        (None, 0)
    );
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    succeeds(
        &mut runtime,
        "if(prefix!==1||entered!==1||caught!==0||finalized!==0||observed!==0||tail!==0)throw 'fatal resume became catchable, replayed or ran tail';'ok';",
    );
    checkpoint(&mut runtime);
}

#[test]
fn await_execution_unsupported_positions_reject_before_effects_even_in_dead_code() {
    for body in [
        "if(false){consume(1,await p);}",
        "if(await p){}",
        "return await p+1;",
        "let first=await p,second=1+await p;",
        "while(false){await p;}",
        "try{}catch(reason){await p;}",
        "try{}finally{await p;}",
        "var values=[await p];",
        "var value={entry:await p};",
        "await (await p);",
    ] {
        let mut runtime = Runtime::for_test();
        let source = format!("var effect=1;async function unsupported(p){{{body}}}effect=2;");
        let error = runtime.execute(&source, &mut NoHost).unwrap_err();
        assert!(
            error.contains("Await in this expression or statement position")
                && error.contains("not implemented"),
            "{body}: {error}"
        );
        runtime.begin_job();
        succeeds(
            &mut runtime,
            "if(typeof effect!=='undefined'||typeof unsupported!=='undefined')throw 'unsupported placement had effects';'ok';",
        );
        assert!(!runtime.has_microtasks());
        assert!(runtime.transient_registration_is_empty());
    }
}
