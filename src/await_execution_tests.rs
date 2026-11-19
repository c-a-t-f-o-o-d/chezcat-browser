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
fn await_execution_negated_logical_expression_keeps_short_circuit_and_branch_order() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var release, reject, trace='', selected, skipped, doubled, caught, reason={};
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
function input(label,promise){trace+='operand:'+label+'|';return promise;}
function mark(value){trace+='mark|';return value;}
function fallback(label){trace+='fallback:'+label+'|';return 0;}
async function choose(enabled,promise,label){
    try{
        if(!(enabled&&mark(await input(label,promise))||fallback(label))){
            trace+='yes:'+label+'|';
            return 'yes';
        }
        trace+='no:'+label+'|';
        return 'no';
    }catch(error){caught=error;}
    finally{trace+='finally:'+label+'|';}
}
async function twice(){return !!(true&&mark(await gate));}
choose(true,gate,'go').then(value=>selected=value);
choose(false,gate,'skip').then(value=>skipped=value);
choose(true,failed,'reject');
twice().then(value=>doubled=value);
if(trace!=='operand:go|fallback:skip|yes:skip|finally:skip|operand:reject|'||
    selected!==undefined||skipped!==undefined||doubled!==undefined||caught!==undefined)
    throw 'condition prefix or inactive operand';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(skipped!=='yes'||selected!==undefined||doubled!==undefined)throw 'premature branch';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release({});reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(selected!=='no'||doubled!==true||caught!==reason||
    trace!=='operand:go|fallback:skip|yes:skip|finally:skip|operand:reject|mark|no:go|finally:go|mark|finally:reject|')
    throw 'negation, branch or rejection';
'ok';
"#);
}

#[test]
fn await_execution_relational_comparison_waits_for_both_operands_before_conversion() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var release, finish, reject, trace='', numberOrder, stringOrder, rejected, reason={};
var leftGate=new Promise(function(resolve){release=resolve;});
var rightGate=new Promise(function(resolve){finish=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var value={get reading(){
    trace+='reading|';
    return {valueOf(){trace+='convert-left|';return 6;}};
}};
function relay(result){trace+='left|';return result;}
function right(result){trace+='right|';return {valueOf(){trace+='convert-right|';return result;}};}
async function order(){return relay(await leftGate).reading>right(await rightGate);}
async function lexical(){return (await Promise.resolve('07'))<'1';}
async function stop(){
    try{return value.reading>=right(await failed);}
    catch(error){rejected=error;}
}
order().then(result=>numberOrder=result);
lexical().then(result=>stringOrder=result);
stop();
if(trace!=='reading|'||numberOrder!==undefined||stringOrder!==undefined||rejected!==undefined)
    throw 'comparison prefix';
'ok';
"#);
    runtime.begin_job();
    succeeds(&mut runtime, "release(value);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(stringOrder!==true||rejected!==reason||numberOrder!==undefined||trace!=='reading|left|reading|')throw 'conversion before right operand';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "finish(5);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(numberOrder!==true||trace!=='reading|left|reading|right|convert-left|convert-right|')
    throw 'relational conversion order or result';
'ok';
"#);
}

#[test]
fn await_execution_strict_comparisons_retain_left_values_through_later_suspension() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', start, release, reject, right, nested, caught, reason={};
var first=new Promise(function(resolve){start=resolve;});
var next=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var token={valueOf(){throw 'strict comparison invoked valueOf';}};
function left(label,value){trace+=label+'|';return value;}
function mark(value){trace+='mark|';return value;}
function relay(value){trace+='right|';return value;}
async function awaitRight(){return left('first',token)===relay(await next);}
async function awaitBoth(){return mark(await first)===(left('middle','6')!==relay(await next));}
async function stop(){
    try{return left('failed',token)===relay(await failed);}
    catch(error){caught=error;}
}
awaitRight().then(value=>right=value);
awaitBoth().then(value=>nested=value);
stop();
if(trace!=='first|failed|'||right!==undefined||nested!==undefined||caught!==undefined)
    throw 'left operand did not run before right suspension';
'ok';
"#);
    runtime.begin_job();
    succeeds(&mut runtime, "start(true);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='first|failed|mark|middle|'||right!==undefined||nested!==undefined)throw 'nested evaluation order';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(token);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='first|failed|mark|middle|right|right|'||right!==true||nested!==true||caught!==reason)
    throw 'comparison results, repeated prefix or rejection';
'ok';
"#);
}

#[test]
fn await_execution_composed_access_reads_selected_receivers_once() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, finish, hit, miss, reason={}, caught, token={};
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var next=new Promise(function(resolve){finish=resolve;});
var selected={get ready(){if(this!==selected)throw 'ready receiver';trace+='ready|';return true;}};
var node={get inner(){if(this!==node)throw 'inner receiver';trace+='inner|';return selected;}};
function relay(value){trace+='relay|';return value;}
function nil(){trace+='nil|';return null;}
function key(){trace+='key|';return 'inner';}
function fallback(){trace+='fallback|';return {get ready(){trace+='miss-ready|';return 0;}};}
async function run(input){
    return (((relay(await input)??nil())?.[key()]??fallback()).ready&&relay(await next));
}
run(gate).then(value=>hit=value);
run(Promise.resolve(null)).then(value=>miss=value);
run(failed).catch(error=>caught=error);
if(trace!==''||hit!==undefined||miss!==undefined||caught!==undefined)throw 'read before fulfillment';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(miss!==0||trace!=='relay|nil|fallback|miss-ready|')throw 'short circuits';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(node);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(hit!==undefined||caught!==reason||trace!=='relay|nil|fallback|miss-ready|relay|key|inner|ready|')
    throw 'selected receiver, property key or rejection';
finish(token);
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(hit!==token||!trace.endsWith('relay|key|inner|ready|relay|'))throw 'resumed logical result';'ok';");
}

#[test]
fn await_execution_strict_comparison_runs_the_right_operand_only_after_the_left() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, eq, neq, different, finalCount=0, caught;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var thing={valueOf(){throw 'strict comparison coerced its operand';}}, reason={};
function relay(value){trace+='left|';return value;}
function right(label,value){trace+=label+'|';return value;}
async function equal(){return relay(await gate)===right('equal',thing);}
async function unequal(){return relay(await gate)!==right('unequal',{});}
async function types(){return (true ? await Promise.resolve(7) : 0)===right('types','7');}
async function stop(){
    try{return relay(await failed)!==right('unreachable',thing);}
    catch(error){caught=error;}
    finally{finalCount++;}
}
equal().then(value=>eq=value);
unequal().then(value=>neq=value);
types().then(value=>different=value);
stop();
if(trace!==''||eq!==undefined||neq!==undefined||different!==undefined||finalCount)
    throw 'comparison ran before fulfillment';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(different!==false||trace!=='types|')throw 'compared different types as equal';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(thing);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(eq!==true||neq!==true||caught!==reason||finalCount!==1||trace!=='types|left|equal|left|unequal|')
    throw 'strict equality, ordering or rejection';
'ok';
"#);
}

#[test]
fn await_execution_for_of_continue_unwinds_inner_scopes_and_preserves_target_iterator() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, clean, result;
var gate=new Promise(function(resolve){release=resolve;});
var cleanup=new Promise(function(resolve){clean=resolve;});
function sequence(label,values){
    return {[Symbol.iterator](){
        let index=0;
        return {
            next(){
                trace+=label+':next'+index+'|';
                return index<values.length ? {done:false,value:values[index++]} : {done:true};
            },
            return(){trace+=label+':close|';return {};}
        };
    }};
}
async function run(){
    outer: for(let item of sequence('outer',[1,2,3,4])){
        if(item===1){
            for(let child of sequence('inner',[8,9])){
                try{
                    await gate;
                    continue outer;
                }finally{
                    trace+='cleanup:start|';
                    await cleanup;
                    trace+='cleanup:'+item+':'+child+'|';
                }
            }
            throw 'continue reached the rest of the outer body';
        }
        await gate;
        if(item===2)continue;
        trace+='break:'+item+'|';
        break;
    }
    return 'done';
}
run().then(value=>result=value);
if(trace!=='outer:next0|inner:next0|'||result!==undefined)throw 'iterators did not suspend';
'ok';
"#);
    runtime.begin_job();
    succeeds(&mut runtime, "release();'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='outer:next0|inner:next0|cleanup:start|'||result!==undefined)throw 'jump escaped suspended cleanup';
clean();
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='outer:next0|inner:next0|cleanup:start|cleanup:1:8|inner:close|outer:next1|outer:next2|break:3|outer:close|'||
    result!=='done')throw 'iterator jump ordering or close';
'ok';
"#);
}

#[test]
fn await_execution_switch_resumes_fallthrough_and_break_in_one_case_scope() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, result, fallback, tdz=false, retained;
var gate=new Promise(function(resolve){release=resolve;});
function choose(tag,visible){trace+='disc:'+tag+':'+visible+'|';return tag;}
function check(tag){trace+='test:'+tag+'|';return tag;}
async function run(tag){
    let visible='outer';
    switch(choose(tag,visible)){
        default:trace+='default|';
        case check('a'):
            let visible='inner';
            retained=()=>visible;
            await gate;
            trace+=visible+'|';
        case check('b'):
            trace+='fall:'+visible+'|';
            break;
        case check('c'):
            throw 'fell past break';
    }
    return visible;
}
async function temporal(){
    try{
        switch(0){case name:let name=0;await gate;}
    }catch(error){if(error.name!=='ReferenceError')throw error;tdz=true;}
}
run('a').then(value=>result=value);
temporal();
if(trace!=='disc:a:outer|test:a|'||result!==undefined||!tdz||retained()!=='inner')
    throw 'initial selection or scope';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "release();'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(result!=='outer'||trace!=='disc:a:outer|test:a|inner|fall:inner|'||retained()!=='inner')
    throw 'fallthrough or break replayed selection';
run('miss').then(value=>fallback=value);
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(fallback!=='outer'||!trace.endsWith('disc:miss:outer|test:a|test:b|test:c|default|inner|fall:inner|'))
    throw 'default selected before later case tests';
'ok';
"#);
}

#[test]
fn await_execution_for_of_body_retains_the_iterator_and_closes_on_abrupt_exit() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(&mut runtime, r#"
var release, reject, normal, returned, rejected, nativeSum, tdz=false, trace='', saved=[], counts={};
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var reason={}, shared=[9];
function values(label){
    var stats=counts[label]={gets:0,steps:0,closes:0};
    return {[Symbol.iterator](){
        var at=0;
        var iterator={
            get next(){
                stats.gets++;
                return function(){
                    if(this!==iterator)throw 'wrong next receiver';
                    stats.steps++;
                    return at<2 ? {done:false,value:++at} : {done:true};
                };
            },
            return(){
                if(this!==iterator)throw 'wrong return receiver';
                stats.closes++;
                trace+=label+':close|';
                if(label==='throw')throw 'ignored close exception';
                return {};
            }
        };
        return iterator;
    }};
}
async function run(label){
    try{
        for(let item of values(label)){
            if(label==='normal')saved.push(()=>item);
            try{
                await (label==='throw' ? failed : gate);
            }finally{
                trace+=label+':inner'+item+'|';
            }
            if(label==='return')return item;
        }
        return 'done';
    }finally{trace+=label+':outer|';}
}
async function uninitialized(){
    try{for(let shared of shared){await gate;}}
    catch(error){if(error.name!=='ReferenceError')throw error;tdz=true;}
}
async function nativeValues(){
    let sum=0;
    for(const [item] of [[2],[3]]){await gate;sum+=item;}
    return sum;
}
run('normal').then(value=>normal=value);
run('return').then(value=>returned=value);
run('throw').catch(error=>rejected=error);
uninitialized();
nativeValues().then(value=>nativeSum=value);
if(trace!==''||!tdz||counts.normal.gets!==1||counts.normal.steps!==1||
    counts.return.steps!==1||counts.throw.steps!==1)throw 'for-of prefix';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;'ok';");
    assert!(runtime.function_arena.retired > 0);
    runtime.begin_job();
    succeeds(&mut runtime, "release();reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(normal!=='done'||returned!==1||rejected!==reason||nativeSum!==5||saved[0]()!==1||saved[1]()!==2||
    counts.normal.gets!==1||counts.return.gets!==1||counts.throw.gets!==1||
    counts.normal.steps!==3||counts.normal.closes!==0||
    counts.return.steps!==1||counts.return.closes!==1||counts.throw.steps!==1||counts.throw.closes!==1||
    !trace.includes('return:inner1|return:close|return:outer|')||
    !trace.includes('throw:inner1|throw:close|throw:outer|'))throw 'iteration, close, or callback order';
'ok';
"#);
}

#[test]
fn await_execution_for_body_runs_test_body_and_update_in_order() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, result, tail, refused, reason={}, saved=[];
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
function test(index){trace+='test'+index+'|';return index<2;}
function next(index){trace+='update'+index+'|';return index+1;}
async function count(){
    let index=0;
    for(;test(index);index=next(index)){
        let snapshot=index;
        saved.push(()=>snapshot);
        trace+='enter'+snapshot+'|';
        await gate;
        trace+='leave'+snapshot+'|';
    }
    for(;false;) {await failed;throw 'unreachable';}
    return index;
}
async function stop(){
    try{for(;;){await failed;return 'unreachable';}}
    finally{tail='done';}
}
async function returnEarly(){
    for(;; next(9)){return await gate;}
}
count().then(value=>result=value);
stop().catch(error=>refused=error);
returnEarly().then(value=>tail+=value);
if(trace!=='test0|enter0|'||result!==undefined||tail!==undefined||refused!==undefined)
    throw 'loop ran past an await';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "reject(reason);release(7);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='test0|enter0|leave0|update0|test1|enter1|leave1|update1|test2|'||
    result!==2||tail!=='done7'||refused!==reason||saved[0]()!==0||saved[1]()!==1)
    throw 'loop ordering, scope or abrupt exit';
'ok';
"#);
}

#[test]
fn await_execution_suspending_finally_preserves_or_overrides_the_incoming_completion() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, retained, fallback, escaped, rejected, normal;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var replacement={marker:'replacement'};
function fresh(marker){return function(){return marker;};}
async function keep(){
    try{
        try{return fresh('return');}
        finally{let local='inner|';await gate;trace+=local;}
    }finally{trace+='outer|';}
}
async function keepThrow(){
    try{throw fresh('throw');}
    finally{await gate;trace+='throw|';}
}
async function replaceThrow(){
    try{throw 'stale exception';}
    finally{await gate;return fresh('override');}
}
async function replaceReturn(){
    try{return 'stale return';}
    finally{await failed;throw 'unreachable';}
}
async function follow(){
    try{trace+='body|';}
    finally{await gate;trace+='cleanup|';}
    trace+='after|';
    return 7;
}
keep().then(value=>retained=value);
keepThrow().catch(error=>escaped=error);
replaceThrow().then(value=>fallback=value);
replaceReturn().catch(error=>rejected=error);
follow().then(value=>normal=value);
if(trace!=='body|'||retained||fallback||escaped||rejected||normal)throw 'finally did not suspend';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;'ok';");
    assert!(runtime.function_arena.retired > 0);
    runtime.begin_job();
    succeeds(&mut runtime, "release();reject(replacement);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='body|inner|outer|throw|cleanup|after|'||retained()!=='return'||escaped()!=='throw'||
    fallback()!=='override'||rejected!==replacement||normal!==7)throw 'lost completion, cleanup or scope';
'ok';
"#);
}

#[test]
fn await_execution_logical_not_resumes_the_operand_before_choosing_a_branch() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, reason={}, branch, doubled, catches=0;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var object={valueOf(){throw 'unexpected coercion';},toString(){throw 'unexpected conversion';}};
function relay(value){trace+='call|';return value;}
async function choose(value){
    try{
        if(!relay(await value)){trace+='true|';return 'true';}
        trace+='false|';
        return 'false';
    }catch(error){if(error!==reason)throw error;catches++;}
    finally{trace+='finally|';}
}
async function twice(){let item={answer:!!await gate};return item.answer;}
choose(gate).then(value=>branch=value);
choose(failed);
twice().then(value=>doubled=value);
if(trace!==''||branch!==undefined||doubled!==undefined||catches)throw 'side effect before fulfillment';
'ok';
"#);
    runtime.begin_job();
    succeeds(&mut runtime, "release(object);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='call|false|finally|finally|'||branch!=='false'||doubled!==true||catches!==1)
    throw 'logical not order, result or rejection';
choose(Promise.resolve(0)).then(value=>branch=value);
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(branch!=='true'||!trace.endsWith('call|true|finally|'))throw 'falsey operand';'ok';");
}

#[test]
fn await_execution_single_property_literal_keeps_the_converted_key_and_own_definition() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, token={}, reason={}, result, plain, caught=0, setters=0;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var name=Symbol('original'), changing=name;
Object.defineProperty(Object.prototype,name,{set(value){setters++;},configurable:true});
function key(){
    trace+='key|';
    return {[Symbol.toPrimitive](hint){if(hint!=='string')throw 'key hint';trace+='convert|';return changing;}};
}
function input(){trace+='operand|';changing=Symbol('replacement');return gate;}
function relay(value){trace+='relay|';return value;}
async function make(){return {[key()]:relay(await input())};}
async function staticKey(){let item={entry:await Promise.resolve(11)};return item.entry;}
async function skip(){
    try{return {entry:relay(await failed)};}catch(error){if(error!==reason)throw error;caught++;}
}
make().then(value=>result=value);
staticKey().then(value=>plain=value);
skip();
if(trace!=='key|convert|operand|'||result!==undefined||plain!==undefined||caught||setters)
    throw 'computed key did not finish before suspension';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(plain!==11||result!==undefined||caught||setters)throw 'pending and static literal results';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "reject(reason);release(token);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
var descriptor=Object.getOwnPropertyDescriptor(result,name);
if(trace!=='key|convert|operand|relay|'||caught!==1||setters||Object.hasOwn(result,changing)||
    Object.getPrototypeOf(result)!==Object.prototype||descriptor.value!==token||
    !descriptor.enumerable||!descriptor.writable||!descriptor.configurable)throw 'literal key, descriptor or rejection';
'ok';
"#);
}

#[test]
fn await_execution_first_member_argument_resumes_trailing_arguments_in_order() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, reason={}, result=0, skipped=0, invalid=0;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
function mark(label,value){trace+=label+'|';return value;}
var owner={base:5,get run(){
    trace+='getter|';
    return function(first,second,third,fourth){
        if(this!==owner)throw 'receiver';trace+='call|';return this.base+first+second+third+fourth;
    };
}};
function relay(value){trace+='relay|';return value+1;}
function never(){throw 'argument evaluated after rejection';}
var missing={run:7};
async function work(){return owner.run(relay(await gate),mark('later',2),...mark('spread',[3,4]));}
async function skip(){
    try{return owner.run(await failed,never());}catch(error){if(error!==reason)throw error;skipped++;}
}
async function bad(){
    try{return missing.run(await Promise.resolve(0),mark('bad-argument',1));}
    catch(error){if(error.name!=='TypeError')throw error;invalid++;}
}
work().then(value=>result=value);
skip();bad();
if(trace!=='getter|getter|'||result||skipped||invalid)throw 'arguments ran before suspension';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='getter|getter|bad-argument|'||invalid!==1||skipped||result)throw 'callability or pending call';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "reject(reason);release(4);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='getter|getter|bad-argument|relay|later|spread|call|'||result!==19||skipped!==1)throw 'argument order or result';'ok';");
}

#[test]
fn await_execution_first_member_argument_shares_argument_limit_and_cleanup() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, &format!(r#"
var ran=0, argumentsToSpread=new Array({MAX_ARGUMENTS}).fill(1), receiver={{run(){{ran++;}}}};
async function work(){{return receiver.run(await Promise.resolve(1),...argumentsToSpread);}}
work();
'ok';
"#));
    let error = runtime.microtask_checkpoint(&mut NoHost).unwrap_err();
    assert!(error.contains("Call argument limit"), "{error}");
    assert!(runtime.transient_registration_is_empty());
    assert_eq!(runtime.evaluation_argument_slots, 0);
    runtime.begin_job();
    succeeds(&mut runtime, "if(ran)throw 'overlimit call ran';'ok';");
}

#[test]
fn await_execution_resumed_method_receiver_precedes_key_and_arguments() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reason={}, result=0, invalid=0, rejected=0;
var gate=new Promise(function(resolve){release=resolve;});
var holder={base:5,get run(){
    trace+='getter|';
    return function(first,second){if(this!==holder)throw 'receiver';trace+='invoke|';return this.base+first+second;};
}};
function receiver(value){if(value!==3)throw 'receiver argument';trace+='receiver|';return holder;}
function key(){trace+='key|';return {toString(){trace+='convert|';return 'run';}};}
function mark(label,value){trace+=label+'|';return value;}
function never(){throw 'arguments evaluated on failed receiver';}
async function work(){return receiver(await gate)[key()](mark('argument',2),...mark('spread',[3]));}
async function bad(){
    try {return (await Promise.resolve(null))[mark('null-key','run')](never());}
    catch(error){if(error.name!=='TypeError')throw error;invalid++;}
}
async function skip(){
    try {return (await Promise.reject(reason))[key()](never());}
    catch(error){if(error!==reason)throw error;rejected++;}
}
work().then(value=>result=value);
bad();skip();
if(trace!==''||result||invalid||rejected)throw 'method evaluated before its receiver';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='null-key|'||invalid!==1||rejected!==1||result)throw 'invalid or rejected receiver';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(3);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='null-key|receiver|key|convert|getter|argument|spread|invoke|'||result!==10)throw 'method receiver, key or argument order';'ok';");
}

#[test]
fn await_execution_assignment_of_branch_values_keeps_references_and_completion() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', releaseYes, releaseNo, releaseWrite, yesResult, noResult, token={}, reason={}, stored=0;
var yes=new Promise(function(resolve){releaseYes=resolve;});
var no=new Promise(function(resolve){releaseNo=resolve;});
var write=new Promise(function(resolve){releaseWrite=resolve;});
function mark(label,value){trace+=label+'|';return value;}
function chooseYes(value){return mark('yes-value',value);}
function chooseNo(value){return mark('no-value',value);}
function never(){throw 'unselected expression';}
function key(label){return mark('key:'+label,{toString(){return mark('convert:'+label,'value');}});}
var outer={set value(value){stored=mark('outer-write',value);}};
var inner={set value(value){mark('inner-write',value);}};
async function choose(flag,source,label){
    let selected;
    if(selected=mark('test:'+label,flag) ? chooseYes(await source) : chooseNo(await source))
        mark('truthy:'+label,selected);
    else mark('falsy:'+label,selected);
    return selected;
}
async function assign(){
    try { outer[key('skipped')]=(true ? await Promise.reject(reason) : never()); }
    catch(error) {
        if(error!==reason)throw error;
        mark('caught',error);
        outer[key('outer')]=(false ? never() : (inner[key('inner')]=(false ? never() : await write)));
    }
}
choose(true,yes,'a').then(value=>yesResult=value);
choose(false,no,'b').then(value=>noResult=value);
assign();
if(trace!=='test:a|test:b|key:skipped|'||stored)throw 'assignment prefix';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='test:a|test:b|key:skipped|caught|key:outer|key:inner|'||stored)throw 'exception or captured targets';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "releaseYes(0);releaseNo(token);releaseWrite(5);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='test:a|test:b|key:skipped|caught|key:outer|key:inner|yes-value|falsy:a|no-value|truthy:b|convert:inner|inner-write|convert:outer|outer-write|'||yesResult!==0||noResult!==token||stored!==5)throw 'assignment value, target order or branch';'ok';");
}

#[test]
fn await_execution_catch_body_retains_bindings_and_unwinds_through_outer_finally() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(&mut runtime, r#"
var trace='', tag={}, fault={code:5}, replacement={}, finish, reject, kept, recovered=0, normal=0, escaped=0;
var continuation=new Promise(function(resolve){finish=resolve;});
var failing=new Promise(function(resolve,fail){reject=fail;});
async function use(start,next,label){
    let code='outer';
    try {
        try { await start;trace+='normal:'+label+'|'; }
        catch({code,note=tag}) {
            trace+='caught:'+label+'|';
            let check=()=>note;
            if(label==='a')kept=check;
            let result=await next;
            if(check()!==tag||note!==tag)throw 'catch environment';
            trace+='resumed:'+label+'|';
            return code+result;
        }
        return 3;
    } finally {
        if(code!=='outer')throw 'catch scope escaped';
        trace+='finally:'+label+'|';
    }
}
use(Promise.reject(fault),continuation,'a').then(value=>recovered=value);
use(Promise.reject(fault),failing,'b').catch(error=>{if(error===replacement)escaped++;});
use(Promise.resolve(0),continuation,'c').then(value=>normal=value);
if(trace!==''||recovered||normal||escaped)throw 'catch before suspension';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='caught:a|caught:b|normal:c|finally:c|'||kept()!==tag||normal!==3||recovered||escaped)throw 'catch entry or normal branch';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;'ok';");
    assert!(runtime.function_arena.retired > 0);
    runtime.begin_job();
    succeeds(&mut runtime, "finish(4);reject(replacement);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='caught:a|caught:b|normal:c|finally:c|resumed:a|finally:a|finally:b|'||recovered!==9||escaped!==1||kept()!==tag)throw 'catch scope or outward rejection';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, r#"
var scope='outer', fallthrough;
async function fallsThrough(){
    try {throw 1;}
    catch(scope){await Promise.resolve(0);if(scope!==1)throw 'lost catch parameter';}
    fallthrough=scope;
}
fallsThrough();
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(fallthrough!=='outer')throw 'catch scope was not released';'ok';");
}

#[test]
fn await_execution_property_assignment_keeps_receiver_and_key_until_the_write() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, reason={}, marker={}, result=0, stored=0, caught=0;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var replacement={set slot(value){throw 'replacement receiver';}};
var current={marker,set slot(value){
    if(this.marker!==marker)throw 'setter receiver';
    trace+='setter|';stored=value;
}};
function receiver(){trace+='receiver|';return current;}
function key(label){
    trace+='key:'+label+'|';
    return {[Symbol.toPrimitive](hint){
        if(hint!=='string'||label!=='write')throw 'unexpected key coercion';
        trace+='convert|';return 'slot';
    }};
}
function input(){trace+='operand|';current=replacement;return gate;}
function relay(value){trace+='relay|';return value+1;}
async function work(){return receiver()[key('write')]=relay(await input());}
async function skip(){
    try { replacement[key('skip')]=await failed; }
    catch(error){if(error!==reason)throw error;caught++;}
}
work().then(value=>result=value);
skip();
if(trace!=='receiver|key:write|operand|key:skip|'||stored||result||caught)throw 'property reference timing';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;'ok';");
    assert!(runtime.function_arena.retired > 0);
    runtime.begin_job();
    succeeds(&mut runtime, "reject(reason);release(4);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='receiver|key:write|operand|key:skip|relay|convert|setter|'||stored!==5||result!==5||caught!==1)throw 'write order, reference identity or rejection';'ok';");
}

#[test]
fn await_execution_resumed_callee_evaluates_followup_arguments_in_order() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reason={}, result=0, invalid=0, rejected=0;
var gate=new Promise(function(resolve){release=resolve;});
function mark(label,value){trace+=label+'|';return value;}
function factory(seed){
    trace+='factory|';
    return function(first,second,third){
        'use strict';if(this!==undefined)throw 'function-valued call receiver';
        return {value:mark('invoke',seed+first+second+third)};
    };
}
function never(){throw 'argument evaluated after rejection';}
async function work(){
    return factory(await gate)(mark('first',1),...mark('spread',[2,3]))?.value;
}
async function bad(){
    try {return (await Promise.resolve(7))(mark('bad-argument',1));}
    catch(error){if(error.name!=='TypeError')throw error;invalid++;}
}
async function skip(){
    try {return (await Promise.reject(reason))(never());}
    catch(error){if(error!==reason)throw error;rejected++;}
}
work().then(value=>result=value);
bad();skip();
if(trace!==''||result||invalid||rejected)throw 'follow-up call before suspension';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='bad-argument|'||invalid!==1||rejected!==1||result)throw 'callability or rejection order';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='bad-argument|factory|first|spread|invoke|'||result!==10)throw 'follow-up argument or call result';'ok';");
}

#[test]
fn await_execution_conditional_values_evaluate_only_the_selected_arms() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, token={}, reason={}, direct, suspended, yes, no, mixed, rejected=0;
var gate=new Promise(function(resolve){release=resolve;});
function mark(label,value){trace+=label+'|';return value;}
function relay(value){return mark('alternate',value);}
function never(){throw 'unselected arm';}
async function choose(flag){
    let value=mark('test',flag) ? mark('primary',token) : relay(await gate);
    return value;
}
async function fromGate(input){
    return (await input) ? mark('test-true','yes') : mark('test-false','no');
}
async function nested(){
    return ((await Promise.resolve(0)) ? never() : token) && 'kept';
}
async function fails(){
    try {return (await Promise.reject(reason)) ? never() : never();}
    finally{trace+='finally|';}
}
choose(true).then(value=>direct=value);
choose(false).then(value=>suspended=value);
fromGate(Promise.resolve(true)).then(value=>yes=value);
fromGate(Promise.resolve(false)).then(value=>no=value);
nested().then(value=>mixed=value);
fails().catch(error=>{if(error===reason)rejected++;});
if(trace!=='test|primary|test|'||direct!==undefined||suspended!==undefined)throw 'conditional prefix';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='test|primary|test|test-true|test-false|finally|'||direct!==token||suspended!==undefined||yes!=='yes'||no!=='no'||mixed!=='kept'||rejected!==1)throw 'selected arm or rejection';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(7);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='test|primary|test|test-true|test-false|finally|alternate|'||suspended!==7)throw 'alternate arm or result';'ok';");
}

#[test]
fn await_execution_nested_logical_values_choose_only_the_required_operands() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', releaseYes, releaseZero, token={}, yesResult, zeroResult, skipResult, mixedResult, kept;
var yes=new Promise(function(resolve){releaseYes=resolve;});
var zero=new Promise(function(resolve){releaseZero=resolve;});
function mark(label,value){trace+=label+'|';return value;}
function receive(pair){return mark('received:'+pair[0],pair[1]);}
async function choose(flag,gate,label){
    return mark('check:'+label,flag) && receive(await gate) &&
        mark('later:'+label,true) && mark('last:'+label,token);
}
async function mixed(){
    return ((await Promise.resolve(null)) ?? ((await Promise.resolve(false)) && mark('wrong',token)))
        || 'fallback';
}
async function preserve(){return (await Promise.resolve(0)) ?? mark('wrong nullish',token);}
choose(false,yes,'skip').then(value=>skipResult=value);
choose(true,yes,'yes').then(value=>yesResult=value);
choose(true,zero,'zero').then(value=>zeroResult=value);
mixed().then(value=>mixedResult=value);
preserve().then(value=>kept=value);
if(trace!=='check:skip|check:yes|check:zero|')throw 'logical prefix';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(skipResult!==false||mixedResult!=='fallback'||kept!==0||yesResult!==undefined||zeroResult!==undefined)throw 'skipped or mixed branch';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "releaseZero(['zero',0]);releaseYes(['yes',true]);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='check:skip|check:yes|check:zero|received:zero|received:yes|later:yes|last:yes|'||zeroResult!==0||yesResult!==token)throw 'logical result or suffix order';'ok';");
}

#[test]
fn await_execution_member_call_retains_receiver_and_getter_result_before_nested_await() {
    let mut runtime = Runtime::for_capacity_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, reason={}, result=0, caught=0, invalid=0;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
var marker={}, original={base:5,marker,get run(){
    trace+='getter|';
    return function(value){if(this.marker!==marker)throw 'receiver';trace+='method|';return this.base+value;};
}};
function owner(){trace+='receiver|';return original;}
function key(){trace+='key|';return {toString(){trace+='convert|';return 'run';}};}
function replace(){trace+='operand|';Object.defineProperty(original,'run',{value:function(){throw 'getter repeated';}});original=null;return gate;}
function relay(value){trace+='relay|';return value+1;}
function never(){throw 'evaluated argument for invalid receiver';}
async function work(){var result=owner()[key()](relay(await replace()));return result;}
async function skips(){
    let object={get run(){trace+='reject-get|';return function(){throw 'invoked on rejection';};}};
    try {return object.run(await failed);}catch(error){if(error!==reason)throw error;caught++;}
}
async function early(){
    let absent=null;
    try {return absent.run(await never());}catch(error){if(error.name!=='TypeError')throw error;invalid++;}
}
work().then(function(value){result=value;});
skips();early();
if(trace!=='receiver|key|convert|getter|operand|reject-get|'||result||caught||invalid!==1)
    throw 'pre-suspension method resolution';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;'ok';");
    assert!(runtime.function_arena.retired > 0);
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='receiver|key|convert|getter|operand|reject-get|relay|method|'||result!==10||caught!==1||invalid!==1)throw 'member call order, collection or rejection';'ok';");
}

#[test]
fn await_execution_optional_chain_keeps_receivers_and_skips_nullish_suffixes() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, result=0, skipped=0, reason={}, rejected=0;
var gate=new Promise(function(resolve){release=resolve;});
var holder={base:5,get method(){
    trace+='getter|';
    return function(input){if(this!==holder)throw 'receiver';trace+='call|';return {value:this.base+input};};
}};
function relay(input){if(input!==4)throw 'awaited argument';trace+='relay|';return holder;}
function key(){trace+='key|';return 'method';}
function argument(){trace+='argument|';return 2;}
async function work(){return relay(await gate)?.method(argument()).value;}
async function optional(source){return (await source)?.[key()]?.(argument()).value;}
work().then(function(value){result=value;});
optional(Promise.resolve(null)).then(function(value){if(value===undefined)skipped++;});
optional(Promise.resolve({})).then(function(value){if(value===undefined)skipped++;});
optional(Promise.reject(reason)).catch(function(error){if(error===reason)rejected++;});
if(trace!==''||result||skipped||rejected)throw 'chain before suspension';
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='key|'||skipped!==2||rejected!==1||result)throw 'nullish or rejected suffix';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);if(trace!=='key|')throw 'synchronous chain';'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='key|relay|getter|argument|call|'||result!==7)throw 'chain order or receiver';'ok';");
}

#[test]
fn await_execution_name_assignment_writes_after_fulfillment_and_skips_rejection() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', release, reject, reason={}, result=0, caught=0, retained=0;
var gate=new Promise(function(resolve){release=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
function relay(value){trace+='relay|';return value+1;}
async function work(){
    let target=1;
    target=relay(await gate);
    try { target=await failed; }catch(error){if(error!==reason)throw error;caught++;}
    try { const locked=2;locked=relay(await Promise.resolve(8)); }
    catch(error){if(error.name!=='TypeError')throw error;caught++;}
    retained=target;
    return target=await Promise.resolve(target+3);
}
work().then(function(value){result=value;});
if(trace!==''||result||caught||retained)throw 'write before suspension';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "release(4);if(trace!=='')throw 'synchronous write';'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='relay|'||result||caught||retained)throw 'first assignment';'ok';");
    runtime.begin_job();
    succeeds(&mut runtime, "reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(trace!=='relay|relay|'||result!==8||caught!==2||retained!==5)throw 'assignment result or error order';'ok';");
}

#[test]
fn await_execution_if_selects_one_branch_after_the_condition_finishes() {
    let mut runtime = Runtime::for_test();
    succeeds(&mut runtime, r#"
var trace='', releaseYes, releaseNo, reject, reason={}, caught=0;
var yes=new Promise(function(resolve){releaseYes=resolve;});
var no=new Promise(function(resolve){releaseNo=resolve;});
var failed=new Promise(function(resolve,fail){reject=fail;});
function check(flag,label){trace+='check:'+label+'|';return flag;}
function wrap(value){trace+='read:'+value+'|';return {ready:value};}
async function choose(flag,gate,label){
    try {
        if(check(flag,label) && wrap(await gate).ready)trace+='yes:'+label+'|';
        else trace+='no:'+label+'|';
        if(flag && await Promise.resolve(false))throw 'false condition without else';
        trace+='after:'+label+'|';
    } finally { trace+='finally:'+label+'|'; }
}
choose(false,yes,'skip');
choose(true,yes,'a');
choose(true,no,'b');
choose(true,failed,'c').catch(function(error){if(error!==reason)throw error;caught++;});
if(trace!=='check:skip|no:skip|after:skip|finally:skip|check:a|check:b|check:c|'||caught)
    throw 'condition prefix';
'ok';
"#);
    checkpoint(&mut runtime);
    runtime.begin_job();
    succeeds(&mut runtime, "releaseYes(true);releaseNo(false);reject(reason);'ok';");
    checkpoint(&mut runtime);
    succeeds(&mut runtime, r#"
if(trace!=='check:skip|no:skip|after:skip|finally:skip|check:a|check:b|check:c|read:true|yes:a|read:false|no:b|finally:c|after:a|finally:a|after:b|finally:b|'||caught!==1)
    throw 'branch choice, rejection or continuation order';
'ok';
"#);
    runtime.begin_job();
    succeeds(&mut runtime, r#"
var tail=0;
async function commaCondition(){
    if((await Promise.resolve(true),0))throw 'used the awaited value';
    else tail++;
    if((await Promise.resolve(false),2))tail++;
}
commaCondition();
'ok';
"#);
    checkpoint(&mut runtime);
    succeeds(&mut runtime, "if(tail!==2)throw 'condition discarded the final operand';'ok';");
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
        "if(1+await p){}",
        "return await p+1;",
        "let first=await p,second=1+await p;",
        "while(false){await p;}",
        "try{}catch(reason){await p;}finally{}",
        "try{}catch{}finally{await p;}",
        "var values=[await p];",
        "var value={entry:await p,another:1};",
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
