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
