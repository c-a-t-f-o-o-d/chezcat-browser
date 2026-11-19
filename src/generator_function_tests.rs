//! Original generator suspended-start and completed-state controls.
use super::*;

#[derive(Default)]
struct NoHost {
    allow_eval: bool,
    eval_windows: Vec<Option<u32>>,
}

impl Host for NoHost {
    fn allow_eval_for_window(&mut self, window: Option<u32>) -> Result<bool, String> {
        self.eval_windows.push(window);
        Ok(self.allow_eval)
    }

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

#[test]
fn generator_yield_if_preserves_selected_block_and_evaluates_each_test_once() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', read;
function test(label,result){trace+=label+'|';return result;}
function relay(value){trace+='call:'+value+'|';return value;}
function* item(){yield 4;return 7;}
function* run(flag){
    let value=0;
    try{
        if(test('primary',flag)){
            let seen=2, inner=relay(yield* item()), result=seen+inner;
            read=()=>result;
            value=result;
        }else if(test('alternate',true)){
            value=relay(yield 9);
        }else{throw 'unselected block';}
    }finally{trace+='finally|';}
    return value;
}
var yes=run(true);
if(yes.next().value!==4||trace!=='primary|'||read!==undefined)throw 'selected block prefix';
if(yes.next().value!==9||read()!==9||trace!=='primary|call:7|finally|')
    throw 'block binding or repeated test';
var no=run(false);
if(no.next().value!==9||!trace.endsWith('primary|alternate|'))throw 'alternate selection';
if(no.next(10).value!==10||trace!=='primary|call:7|finally|primary|alternate|call:10|finally|')
    throw 'alternate completion';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_declarators_resume_in_order_without_initializing_the_suffix_early() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', read, reason={}, caught;
function note(label,value){trace+=label+'|';return value;}
function relay(value){if(value===reason)throw value;trace+='call:'+value+'|';return value;}
function* sequence(){
    read=()=>last;
    let first=note('first',1), second=relay(yield first), third=note('third',second+1), last=relay(yield third);
    return first+second+third+last;
}
function* failure(){
    try{var first=note('before',1), middle=relay(yield first), last=note('unreachable',2);}
    catch(error){caught=error;}
    return last;
}
var working=sequence(), denied=false;
if(working.next().value!==1||trace!=='first|')throw 'prefix declaration order';
try{read();}catch(error){if(error.name!=='ReferenceError')throw error;denied=true;}
if(!denied||working.next(3).value!==4||trace!=='first|call:3|third|')
    throw 'first resumption ran a later initializer';
var result=working.next(6);
if(!result.done||result.value!==14||read()!==6||trace!=='first|call:3|third|call:6|')
    throw 'declaration ran twice or initialized the wrong binding';
var broken=failure();
if(broken.next().value!==1||broken.next(reason).value!==undefined||caught!==reason||
    !trace.endsWith('call:6|before|'))throw 'failed initializer ran its suffix';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_lexical_for_of_keeps_iterator_and_unwinds_in_order() {
    let mut runtime = Runtime::for_capacity_test();
    let source = r#"
var trace='', iterator, readers=[], fault={};
function make(fail){
    return {[Symbol.iterator](){
        var index=0;
        iterator={
            next(){
                trace+='next|';
                if(fail&&index===1)throw fault;
                if(index===2)return {done:true};
                return {done:false,value:++index};
            },
            return(){trace+='close|';return {};}
        };
        return iterator;
    }};
}
function pass(value){return value;}
function* feed(fail){
    try{
        for(let value of make(fail)){
            const read=()=>value;
            readers.push(read);
            try{pass(yield read());}
            finally{trace+='inner:'+read()+'|';}
        }
    }finally{trace+='outer|';}
}
var full=feed(false);
if(full.next().value!==1||trace!=='next|')throw 'initial step';
iterator.next=function(){throw 'next was read again';};
iterator=null;
var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;
if(full.next().value!==2||!full.next().done||readers[0]()!==1||readers[1]()!==2||
    trace!=='next|inner:1|next|inner:2|next|outer|')throw 'iterator or lexical order';
trace='';
var early=feed(false);
if(early.next().value!==1||early.return(9).value!==9||
    trace!=='next|inner:1|close|outer|')throw 'abrupt cleanup order';
trace='';
var broken=feed(true), caught=false;
broken.next();
try{broken.next();}catch(error){if(error!==fault)throw error;caught=true;}
if(!caught||!broken.next().done||trace!=='next|inner:1|next|outer|')throw 'step failure closed iterator';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert!(runtime.function_arena.retired > 0);
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_classic_for_resumes_iteration_and_unwinds_cleanup() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', readers=[];
function mark(label,value){trace+=label+'|';return value;}
function relay(value){return mark('call:'+value,value);}
function* each(value){yield value;return value+10;}
function* loop(){
    let index=0;
    try{
        for(;mark('test:'+index,index<2);index=mark('update:'+index,index+1)){
            let value=relay(yield* each(index));
            readers.push(()=>value);
        }
        return index;
    }finally{trace+='finally|';}
}
var active=loop();
if(active.next().value!==0||trace!=='test:0|')throw 'loop prefix';
if(active.next().value!==1||trace!=='test:0|call:10|update:0|test:1|')throw 'first iteration';
var done=active.next();
if(!done.done||done.value!==2||readers[0]()!==10||readers[1]()!==11||
    !trace.endsWith('call:11|update:1|test:2|finally|'))throw 'iteration scopes or exit';
trace='';
var stopped=loop();
stopped.next();
if(stopped.return(9).value!==9||trace!=='test:0|finally|')throw 'early return ran update';
function* open(){
    try{for(;;){let value=relay(yield 4);return value;}}
    finally{trace+='open-finally|';}
}
var infinite=open();
if(infinite.next().value!==4||infinite.next(7).value!==7||
    !trace.endsWith('call:7|open-finally|'))throw 'loop without condition';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_conditional_initializer_selects_one_branch_and_keeps_tdz() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', read, marker={};
function mark(label,value){trace+=label+'|';return value;}
function relay(value){trace+='call|';return value;}
function* items(){yield 5;return {value:marker};}
function* run(flag){
    try{
        let before=mark('before',1), {value}=mark('choice',flag)
            ? relay(yield* items()) : mark('other',{value:marker}),
            after=mark('after',2);
        return before+after===3&&value===marker;
    }finally{trace+='finally|';}
}
function* captures(flag){
    read=()=>value;
    let value=flag ? relay(yield 6) : marker;
    return value;
}
var selected=run(true);
if(selected.next().value!==5||trace!=='before|choice|')throw 'selected initializer prefix';
if(selected.next().value!==true||trace!=='before|choice|call|after|finally|')
    throw 'selected initializer order or destructuring';
trace='';
var skipped=run(false);
if(skipped.next().value!==true||trace!=='before|choice|other|after|finally|')
    throw 'unchosen yield suspended';
var reference=captures(true), tdz=false;
if(reference.next().value!==6)throw 'missing yield';
try{read();}catch(error){if(error.name!=='ReferenceError')throw error;tdz=true;}
if(!tdz||reference.next(marker).value!==marker||read()!==marker)throw 'incorrect initialization';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_in_return_sequence_preserves_value_and_cleanup() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', chosen={}, replacement={};
function relay(value){trace+='call:'+value+'|';}
function finish(){trace+='suffix|';return chosen;}
function* inner(){try{yield 3;return 4;}finally{trace+='inner|';}}
function* sequence(override){
    try{return (relay(yield* inner()), trace+='between|', finish());}
    finally{trace+='outer|';if(override)return replacement;}
}
var first=sequence(false);
if(first.next().value!==3||trace!=='')throw 'return suffix ran before suspension';
var done=first.next();
if(!done.done||done.value!==chosen||trace!=='inner|call:4|between|suffix|outer|')
    throw 'return sequence completion';
trace='';
var overridden=sequence(true);
overridden.next();
if(overridden.next().value!==replacement||!trace.endsWith('suffix|outer|'))
    throw 'finally did not replace computed return';
trace='';
var cancelled=sequence(false);
cancelled.next();
var interrupted=cancelled.return(9);
if(!interrupted.done||interrupted.value!==9||trace!=='inner|outer|')
    throw 'interrupted return sequence ran its suffix';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_respects_short_circuit_expression_statements() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='';
function read(value){trace+='read|';return value;}
function relay(value){trace+='call:'+value+'|';}
function* inner(value){trace+='inner:'+value+'|';yield value;return value*10;}
function* run(andLeft, orLeft, nullLeft){
    try{
        let first=yielded();
        function yielded(){return 1;}
        if(first){
            read(andLeft) && relay(yield* inner(1));
            read(orLeft) || relay(yield 2);
            (read(nullLeft) && 3) ?? relay(yield* inner(3));
        }
    }finally{trace+='finally|';}
}
var skipped=run(0,true,false);
if(!skipped.next().done||trace!=='read|read|read|finally|')throw 'short-circuited operands ran';
trace='';
var selected=run({},0,null);
if(selected.next().value!==1||trace!=='read|inner:1|')throw 'and prefix';
if(selected.next().value!==2||trace!=='read|inner:1|call:10|read|')throw 'or prefix';
if(selected.next(7).value!==3||!trace.endsWith('call:7|read|inner:3|'))throw 'nullish prefix';
if(!selected.next().done||!trace.endsWith('call:30|finally|'))throw 'nullish completion';
trace='';
var cancelled=run(true,false,null);
cancelled.next();
if(cancelled.return(9).value!==9||trace!=='read|inner:1|finally|')throw 'abrupt short-circuit continuation';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_branch_resumes_only_the_selected_sequence() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='';
function choose(flag){trace+='choose|';return flag;}
function relay(value){trace+='call:'+value+'|';return value;}
function* items(value){trace+='in:'+value+'|';yield value;return value*10;}
function* run(flag){
    try{
        var base=relay(yield* items(1));
        var selected=0;
        choose(flag) ? (
            selected=relay(yield* items(2)),
            trace+='between|',
            selected=relay(yield* items(3))
        ) : selected=relay(yield* items(4));
        return base+selected;
    }finally{trace+='finally|';}
}
var yes=run(true);
if(yes.next().value!==1||yes.next().value!==2||trace!=='in:1|call:10|choose|in:2|')
    throw 'selected branch prefix';
if(yes.next().value!==3||!trace.endsWith('call:20|between|in:3|'))throw 'sequence order';
var complete=yes.next();
if(!complete.done||complete.value!==40||!trace.endsWith('call:30|finally|')||trace.includes('in:4|'))
    throw 'completed wrong branch';
trace='';
var no=run(false);
if(no.next().value!==1||no.next().value!==4||no.next().value!==50||
    trace!=='in:1|call:10|choose|in:4|call:40|finally|')throw 'alternate or duplicate condition';
trace='';
var cancelled=run(true);
cancelled.next();cancelled.next();
if(cancelled.return(99).value!==99||!cancelled.next().done||
    trace!=='in:1|call:10|choose|in:2|finally|')throw 'cancelled sequence ran its suffix';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_declaration_initializes_hoisted_and_lexical_bindings_on_resume() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var readVar, readConst, failure={}, caught, finalized=0;
function relay(value){if(value===failure)throw value;return value;}
function* inner(){yield 2;return 7;}
function* variable(){
    readVar=()=>value;
    try{var value=relay(yield* inner());}
    finally{finalized++;}
    return value;
}
function* lexical(){
    readConst=()=>value;
    const {value}=relay(yield 3);
    return value;
}
function* failed(){
    try{var value=relay(yield 4);}
    catch(error){caught=error;}
    return value;
}
var first=variable();
if(first.next().value!==2||readVar()!==undefined||finalized)throw 'hoisted var prefix';
var result=first.next();
if(!result.done||result.value!==7||readVar()!==7||finalized!==1)throw 'var initializer';
var second=lexical(), denied=false;
if(second.next().value!==3)throw 'lexical suspension';
try{readConst();}catch(error){if(error.name!=='ReferenceError')throw error;denied=true;}
if(!denied||second.next({value:9}).value!==9||readConst()!==9)throw 'const pattern or TDZ';
var third=failed();
third.next();
if(third.next(failure).value!==undefined||caught!==failure)throw 'failed initializer changed binding';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_assignment_keeps_the_original_reference_and_skips_failed_writes() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', writes=0, reason={}, caught;
var original={set value(value){if(this!==original)throw 'wrong setter receiver';trace+='write:'+value+'|';writes++;}};
var receiver=original;
function getReceiver(){trace+='receiver|';return receiver;}
function getKey(){trace+='key|';return {toString(){trace+='convert|';return 'value';}};}
function relay(value){trace+='call:'+value+'|';if(value===reason)throw reason;return value+1;}
function* input(){yield 2;return 7;}
function* property(){
    try{getReceiver()[getKey()]=relay(yield* input());}
    finally{trace+='finally|';}
}
function* local(){
    let result=3;
    try{result=relay(yield);}
    catch(error){caught=error;}
    return result;
}
var pending=property();
if(pending.next().value!==2||trace!=='receiver|key|'||writes)throw 'assignment target order';
receiver={set value(value){throw 'repeated receiver lookup';}};
if(!pending.next().done||trace!=='receiver|key|call:7|convert|write:8|finally|'||writes!==1)
    throw 'resumed assignment or result';
var variable=local();
variable.next();
if(variable.next(9).value!==10)throw 'identifier assignment';
var failed=local();
failed.next();
if(failed.next(reason).value!==3||caught!==reason||writes!==1)
    throw 'failed call overwrote assignment target';
var cancelled=local();
cancelled.next();
if(cancelled.return(11).value!==11||!cancelled.next().done||writes!==1)
    throw 'return executed pending assignment';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_try_catch_resumes_after_handling_a_suspended_throw() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', injected={}, fromBody={}, replacement={}, saved;
function* work(label,mode){
    try{
        try{
            let inner=5;
            yield inner;
            trace+=label+':continued|';
            if(mode==='body')throw fromBody;
        }catch(error){
            trace+=label+':catch|';
            saved=()=>error;
            if(mode==='replace')throw replacement;
        }
        yield 'after';
        return 7;
    }finally{trace+=label+':finally|';}
}
var first=work('a','injected');
if(first.next().value!==5||trace!=='')throw 'initial suspension';
var handled=first.throw(injected);
if(handled.done||handled.value!=='after'||trace!=='a:catch|'||saved()!==injected)
    throw 'catch did not resume the generator';
if(first.next().value!==7||!first.next().done||trace!=='a:catch|a:finally|')
    throw 'continuation or duplicate cleanup';
var second=work('b','body');
second.next();
if(second.next().value!=='after'||saved()!==fromBody||
    !trace.endsWith('b:continued|b:catch|'))throw 'body throw or catch scope';
var closed=second.return(10);
if(!closed.done||closed.value!==10||!trace.endsWith('b:finally|'))throw 'post-catch closure';
var bypass=work('c','injected');
bypass.next();
if(bypass.return(11).value!==11||!trace.endsWith('b:finally|c:finally|'))
    throw 'return entered catch';
var changed=work('d','replace'), received;
changed.next();
try{changed.throw(injected);}catch(error){received=error;}
if(received!==replacement||!trace.endsWith('d:catch|d:finally|')||!changed.next().done)
    throw 'catch replacement or cleanup';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_call_argument_retains_callee_and_uses_the_resumed_value() {
    let mut runtime = Runtime::for_capacity_test();
    let source = r#"
var trace='', delegated, plain, reason={}, caught;
function makeOriginal(){return function(value){trace+='call:'+value+'|';return 100;};}
function other(){throw 'callee looked up after suspension';}
var target=makeOriginal();
function* inner(){yield 2;return 8;}
function* invoke(){
    try{target(yield* inner());}
    finally{trace+='finally|';}
    return 'tail';
}
function* accept(){target(yield 5);}
delegated=invoke();
var first=delegated.next();
if(first.done||first.value!==2||trace!=='')throw 'pending delegation';
target=other;
var trash={};for(var index=0;index<900;index++)trash.fn=function(){return index;};trash=null;
var last=delegated.next(1);
if(!last.done||last.value!=='tail'||trace!=='call:8|finally|')throw 'delegate return value or callee';
target=makeOriginal();
plain=accept();
if(plain.next().value!==5)throw 'bare call yield';
target=other;
if(!plain.next(9).done||trace!=='call:8|finally|call:9|')throw 'sent value or repeated target lookup';
var cancelled=invoke();
cancelled.next();
if(cancelled.return(11).value!==11||!cancelled.next().done)throw 'delegate return closed wrong result';
var failed=invoke();
failed.next();
try{failed.throw(reason);}catch(error){caught=error;}
if(caught!==reason||trace!=='call:8|finally|call:9|finally|finally|'||!failed.next().done)
    throw 'abrupt interruption entered caller or skipped cleanup';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert!(runtime.function_arena.retired > 0);
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_call_argument_preserves_direct_eval_policy_and_scope() {
    let mut runtime = Runtime::for_test();
    let mut host = NoHost::default();
    assert_eq!(runtime.execute(r#"
var visible=0, refused=false;
function* invoke(){let local=17;eval(yield);return visible;}
var blocked=invoke();
if(blocked.next().value!==undefined)throw 'bare yield';
try{blocked.next('visible=local');}catch(error){if(error.name!=='EvalError')throw error;refused=true;}
if(!refused||visible||!blocked.next().done)throw 'eval policy or completion';
'ok';
"#, &mut host).unwrap().to_text(), "ok");
    assert_eq!(host.eval_windows, [None]);
    host.allow_eval = true;
    assert_eq!(runtime.execute(r#"
var allowed=invoke();
allowed.next();
if(allowed.next('visible=local').value!==17||visible!==17)throw 'resumed direct scope';
'ok';
"#, &mut host).unwrap().to_text(), "ok");
    assert_eq!(host.eval_windows, [None, None]);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_try_finally_keeps_scopes_and_unwinds_on_injected_completion() {
    let mut runtime = Runtime::for_capacity_test();
    let source = r#"
var trace='', marker={}, saw;
function* scoped(label,source){
    try{
        let visible=label;
        try{
            let inside=40;
            yield inside;
            yield* source;
            trace+=visible+':body'+inside+'|';
        }finally{trace+=visible+':inner|';}
    }finally{trace+=label+':outer|';}
    trace+=label+':after|';
    return 'done';
}
function iterable(){
    var used=false;
    var iterator={
        next(){if(used)return {done:true};used=true;return {done:false,value:9};},
        return(value){trace+='delegate:close|';return {done:true,value};}
    };
    return {[Symbol.iterator](){return iterator;}};
}
var exhausted=scoped('a',iterable());
if(exhausted.next().value!==40)throw 'first suspension';
var trash={};for(var index=0;index<900;index++)trash.fn=function(){return index;};trash=null;
if(exhausted.next().value!==9||trace!=='')throw 'unexpected cleanup';
if(exhausted.next().value!=='done'||trace!=='a:body40|a:inner|a:outer|a:after|')
    throw 'normal finally order or state';
var closed=scoped('b',iterable());
closed.next();closed.next();
var returned=closed.return(marker);
if(!returned.done||returned.value!==marker||!trace.endsWith('delegate:close|b:inner|b:outer|')||
    !closed.next().done)throw 'return, delegate close or cleanup order';
var throwing=scoped('c',iterable());
throwing.next();
try{throwing.throw(marker);}catch(error){saw=error;}
if(saw!==marker||!trace.endsWith('c:inner|c:outer|')||!throwing.next().done)
    throw 'throw unwinding or repeated body';
function* replace(){try{yield 2;}finally{return 7;}}
var replacement=replace();
replacement.next();
if(replacement.throw(marker).value!==7)throw 'finally did not replace exception';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert!(runtime.function_arena.retired > 0);
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_delegation_forwards_results_and_completions() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var marker={}, reads=0, tail=0;
function source(mode){
    var args=[], gets=0, closes=0, received, index=0;
    var first={done:false,get value(){reads++;return 10;}};
    var returned={done:false,value:'held return'};
    var iterator={
        get next(){
            gets++;
            return function(value){
                if(this!==iterator||arguments.length!==1)throw 'next receiver or arity';
                args.push(value);
                if(index++===0)return first;
                return {done:true,get value(){reads++;return 'done';}};
            };
        }
    };
    if(mode!=='no return')iterator.return=function(value){
        if(this!==iterator)throw 'return receiver';
        closes++;
        if(mode==='no throw'&&arguments.length!==0)throw 'close passed an argument';
        received=value;
        if(mode==='return yield')return returned;
        return {done:true,value:'closed'};
    };
    if(mode==='handles throw')iterator.throw=function(value){
        if(this!==iterator||arguments.length!==1)throw 'throw receiver or arity';
        received=value;
        return {done:true,value:'handled'};
    };
    return {
        [Symbol.iterator](){return iterator;},
        first,returned,iterator,args,
        get gets(){return gets;},get closes(){return closes;},get received(){return received;}
    };
}
function* outer(input){yield* input;tail++;return 'tail';}
var normal=source('normal'), drive=outer(normal);
if(normal.gets||drive.next(1)!==normal.first||reads||normal.args[0]!==undefined)
    throw 'first result identity or premature read';
Object.defineProperty(normal.iterator,'next',{value:function(){throw 'next read again';}});
var completion=drive.next(4);
if(!completion.done||completion.value!=='tail'||normal.args[1]!==4||normal.gets!==1||reads!==1)
    throw 'next forwarding, cached method or completion';
var hold=source('return yield'), resumed=outer(hold);
resumed.next();
if(resumed.return(7)!==hold.returned||hold.received!==7||hold.closes!==1||
    resumed.next(8).value!=='tail'||hold.args[1]!==8)throw 'return result did not suspend';
var recovery=source('handles throw'), catcher=outer(recovery);
catcher.next();
if(catcher.throw(marker).value!=='tail'||recovery.received!==marker||recovery.closes)
    throw 'forwarded throw completion';
var missing=source('no throw'), closed=outer(missing), caught=false;
closed.next();
try{closed.throw(marker);}catch(error){if(error.name!=='TypeError')throw error;caught=true;}
if(!caught||missing.closes!==1||!closed.next().done)throw 'missing throw did not close';
var absent=source('no return'), stopping=outer(absent);
stopping.next();
var stopped=stopping.return(marker);
if(!stopped.done||stopped.value!==marker||!stopping.next().done||tail!==3||reads!==2)
    throw 'missing return, unexpected tail, or value read';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_statements_resume_without_replaying_prefixes_or_running_abandoned_bodies() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', reason={}, returned={}, seen;
function* walk(input){
    let stored=input+1;
    trace+='start|';
    yield stored;
    trace+='middle|';
    yield
    trace+='finish|';
    return stored+1;
}
var iterator=walk(4);
if(trace!=='')throw 'eager generator body';
var first=iterator.next('ignored');
if(first.done!==false||first.value!==5||trace!=='start|')throw 'first yield';
var second=iterator.next(100);
if(second.done!==false||second.value!==undefined||trace!=='start|middle|')throw 'bare yield or replay';
var last=iterator.next();
if(last.done!==true||last.value!==6||iterator.next().value!==undefined||trace!=='start|middle|finish|')
    throw 'return from resumed body';
var stopped=walk(9);
stopped.next();
var exit=stopped.return(returned);
var thrown=walk(10);
thrown.next();
try{thrown.throw(reason);}catch(error){seen=error;}
if(exit.done!==true||exit.value!==returned||stopped.next().value!==undefined||
    seen!==reason||thrown.next().done!==true||trace!=='start|middle|finish|start|start|')
    throw 'abrupt exit resumed a paused body';
function fail(){throw reason;}
function* failure(){yield fail();}
var failing=failure();
seen=null;
try{failing.next();}catch(error){seen=error;}
if(seen!==reason||failing.next().done!==true)throw 'failure while evaluating yielded value';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_yield_statements_keep_live_scopes_and_close_on_allocation_failure() {
    let mut runtime = Runtime::for_capacity_test();
    let mut host = NoHost::default();
    assert_eq!(runtime.execute(r#"
var tail=0;
function* staged(value){
    let read=()=>value;
    yield 1;
    yield read();
    yield 3;
    tail++;
}
var paused=staged(35);
if(paused.next().value!==1)throw 'first suspension';
var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;
'ok';
"#, &mut host).unwrap().to_text(), "ok");
    assert!(runtime.function_arena.retired > 0);
    runtime.begin_job();
    assert_eq!(runtime.execute("paused.next().value;", &mut host).unwrap().to_number(), 35.0);
    let object_limit = runtime.limits.heap.objects;
    runtime.limits.heap.objects = runtime.objects.len();
    let failure = runtime.execute("paused.next();", &mut host).unwrap_err();
    assert!(failure.contains("Object identity limit"), "{failure}");
    runtime.limits.heap.objects = object_limit;
    runtime.begin_job();
    assert_eq!(runtime.execute("if(!paused.next().done||tail)throw 'replayed failed yield';'ok';", &mut host)
        .unwrap().to_text(), "ok");
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_function_start_is_lazy_but_initializes_parameters_at_call_time() {
    let mut runtime = Runtime::for_test();
    let source = r#"
var trace='', reason={}, receiver={base:5}, handle, catches=0;
function initial(){trace+='param|';return 3;}
function* make(value=initial()){
    trace+='body|';
    try{return this.base+value;}
    finally{trace+='finally|';}
}
var expression=function* named(){return 11;};
var generator=make.call(receiver);
if(trace!=='param|'||expression.name!=='named'||Object.getPrototypeOf(generator)!==make.prototype||
    generator[Symbol.iterator]()!==generator||Object.prototype.toString.call(generator)!=='[object Generator]')
    throw 'generator identity or eager body';
var first=generator.next('unused'), end=generator.next();
if(!first.done||first.value!==8||!end.done||end.value!==undefined||trace!=='param|body|finally|')
    throw 'generator first and completed next';
var skipped=make.call(receiver), stop=skipped.return(9);
if(!stop.done||stop.value!==9||skipped.next().value!==undefined||trace!=='param|body|finally|param|')
    throw 'return ran suspended-start body';
var thrown=make.call(receiver,12);
try{thrown.throw(reason);}catch(error){if(error!==reason)throw error;catches++;}
if(thrown.next().value!==undefined||!thrown.next().done||trace!=='param|body|finally|param|')
    throw 'throw ran suspended-start body';
try{new make();}catch(error){if(error.name!=='TypeError')throw error;catches++;}
try{generator.next.call({});}catch(error){if(error.name!=='TypeError')throw error;catches++;}
function* reentrant(){
    try{handle.next();}catch(error){if(error.name!=='TypeError')throw error;return 13;}
    throw 'accepted reentrant next';
}
handle=reentrant();
if(handle.next().value!==13||catches!==3||expression().next().value!==11)
    throw 'reentrant, brand or constructibility';
'ok';
"#;
    assert_eq!(runtime.execute(source, &mut NoHost::default()).unwrap().to_text(), "ok");
    assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_function_start_keeps_foreign_body_realm_and_dynamic_csp() {
    let mut runtime = Runtime::for_test();
    let mut host = NoHost::default();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let foreign = runtime.with_intrinsic_realm(Some(realm), |runtime| {
        runtime.bind_browser_global(77).map_err(Fault::new)?;
        runtime.execute("var secret=17;function* foreign(){return Array;}", &mut host)
            .map_err(Fault::new)?;
        ["foreign", "Array"].into_iter().map(|name| runtime.binding_value(runtime.global_environment, name))
            .collect::<Result<Vec<_>, _>>()
    }).unwrap();
    for (name, value) in ["foreign", "ChildArray"].into_iter().zip(foreign) {
        runtime.define_global(name, value).unwrap();
    }
    assert_eq!(runtime.execute(r#"
function* local(){}
var localNext=local().next, child=foreign(), Generator=Object.getPrototypeOf(foreign).constructor;
var result=localNext.call(child);
if(!result.done||result.value!==ChildArray||Object.getPrototypeOf(result)!==Object.prototype)
    throw 'generator and result realms';
var blocked=false;
try{Generator('return secret;');}catch(error){if(error.name!=='EvalError')throw error;blocked=true;}
if(!blocked)throw 'dynamic generator bypassed CSP';
'ok';
"#, &mut host).unwrap().to_text(), "ok");
    assert_eq!(host.eval_windows, [Some(77)]);
    host.allow_eval = true;
    assert_eq!(runtime.execute(r#"
var built=new Generator('return secret;');
if(Object.getPrototypeOf(built)!==Object.getPrototypeOf(foreign)||
    built.toString()!=='function* anonymous(\n) {\nreturn secret;\n}'||
    built().next().value!==17)throw 'dynamic generator owner or source';
'ok';
"#, &mut host).unwrap().to_text(), "ok");
    assert_eq!(host.eval_windows, [Some(77), Some(77)]);
    assert_eq!((runtime.active_realm, runtime.global_environment, runtime.call_depth), (None, 0, 0));
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn generator_function_start_preserves_prepared_scopes_through_collection() {
    let mut runtime = Runtime::for_capacity_test();
    let mut host = NoHost::default();
    assert_eq!(runtime.execute(r#"
function makeReceiver(){return {value:19};}
function* work(read=()=>this.value){return read();}
var pending=work.call(makeReceiver());
var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;
'ok';
"#, &mut host).unwrap().to_text(), "ok");
    assert!(runtime.function_arena.retired > 0);
    let Value::Object(generator) = runtime.binding_value(runtime.global_environment, "pending").unwrap()
        else { panic!("missing generator") };
    assert!(matches!(runtime.objects[generator].generator, Some(generator_functions::State::SuspendedStart(_))));
    runtime.begin_job();
    assert_eq!(runtime.execute("pending.next().value;", &mut host).unwrap().to_number(), 19.0);
    assert!(matches!(runtime.objects[generator].generator, Some(generator_functions::State::Completed)));
    assert!(runtime.transient_registration_is_empty());
}
