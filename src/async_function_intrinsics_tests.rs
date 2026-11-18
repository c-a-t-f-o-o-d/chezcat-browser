//! Original intrinsic/constructor controls; async execution is tested separately.
use super::*;

#[derive(Default)]
struct TestHost {
    denied: bool,
    policy_windows: Vec<Option<u32>>,
}

impl Host for TestHost {
    fn allow_eval_for_window(&mut self, window: Option<u32>) -> Result<bool, String> {
        self.policy_windows.push(window);
        Ok(!self.denied)
    }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("Unexpected native read.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("Unexpected native write.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("Unexpected native call.".into())
    }
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_capacity_test();
    assert_eq!(
        runtime
            .execute(source, &mut TestHost::default())
            .unwrap()
            .to_text(),
        "ok"
    );
}

#[test]
fn async_function_intrinsics_real_identity_descriptors_and_nonconstructible_instances() {
    succeeds(
        r#"
async function declared(a,b=2) { return a+b; }
var expression=async function named(x) { return x; };
var arrow=async x=>x;
var ap=Object.getPrototypeOf(declared),af=ap.constructor;
if(typeof AsyncFunction!=='undefined'||typeof af!=='function'||typeof ap!=='object')throw 'intrinsic kinds';
if(Object.getPrototypeOf(ap)!==Function.prototype||Object.getPrototypeOf(af)!==Function)throw 'intrinsic inheritance';
if(Object.getPrototypeOf(expression)!==ap||Object.getPrototypeOf(arrow)!==ap)throw 'one realm prototype';
if(!(declared instanceof Function)||declared.name!=='declared'||declared.length!==1)throw 'callable metadata';
if(af.name!=='AsyncFunction'||af.length!==1||af.prototype!==ap)throw 'constructor metadata';
if(Object.getOwnPropertyNames(ap).join(',')!=='constructor')throw 'prototype string keys';
var c=Object.getOwnPropertyDescriptor(ap,'constructor'),p=Object.getOwnPropertyDescriptor(af,'prototype');
var t=Object.getOwnPropertyDescriptor(ap,Symbol.toStringTag);
if(c.value!==af||c.writable||c.enumerable||!c.configurable)throw 'constructor descriptor';
if(p.value!==ap||p.writable||p.enumerable||p.configurable)throw 'prototype descriptor';
if(t.value!=='AsyncFunction'||t.writable||t.enumerable||!t.configurable)throw 'tag descriptor';
if(Object.getOwnPropertySymbols(ap).length!==1||Object.getOwnPropertySymbols(ap)[0]!==Symbol.toStringTag)throw 'symbol identity';
for(var f of [declared,expression,arrow]){
 if(Object.hasOwn(f,'prototype')||Object.hasOwn(f,Symbol.toStringTag))throw 'instance own properties';
 if(Object.prototype.toString.call(f)!=='[object AsyncFunction]')throw 'instance tag';
 var denied=false;try{new f();}catch(e){denied=e.name==='TypeError';}if(!denied)throw 'async constructor';
}
if(Object.prototype.toString.call(ap)!=='[object AsyncFunction]')throw 'ordinary prototype tag';
var notCallable=false;try{Function.prototype.toString.call(ap);}catch(e){notCallable=e.name==='TypeError';}
if(!notCallable)throw 'prototype must not be callable';
ap.constructor=1;ap[Symbol.toStringTag]='changed';
if(ap.constructor!==af||ap[Symbol.toStringTag]!=='AsyncFunction')throw 'readonly inherited metadata';
Object.defineProperty(expression,Symbol.toStringTag,{value:'Original',configurable:true});
if(Object.prototype.toString.call(expression)!=='[object Original]')throw 'own tag override';
delete expression[Symbol.toStringTag];delete ap[Symbol.toStringTag];
if(Object.prototype.toString.call(expression)!=='[object Function]')throw 'tag deletion fallback';
delete ap.constructor;if(ap.constructor!==Function)throw 'constructor deletion lookup';
'ok';
"#,
    );
}

#[test]
fn async_function_intrinsics_exact_source_dynamic_constructor_call_new_and_bind() {
    succeeds(
        r#"
async function /* original */ exact (x) { /* body */return x; }
if(exact.toString()!=='async function /* original */ exact (x) { /* body */return x; }')throw 'retained source';
var af=Object.getPrototypeOf(exact).constructor,ap=af.prototype;
if(af.toString()!=='function AsyncFunction() { [native code] }')throw 'native source';
var f=af('a /* parameter */','b','return a+b;');
if(f.name!=='anonymous'||f.length!==2||Object.getPrototypeOf(f)!==ap||Object.hasOwn(f,'prototype'))throw 'dynamic metadata';
if(f.toString()!=='async function anonymous(a /* parameter */,b\n) {\nreturn a+b;\n}')throw 'dynamic exact source';
var constructed=new af('x','return x;');
var bound=af.bind(null,'x'),viaBound=new bound('return x;');
if(Object.getPrototypeOf(constructed)!==ap||Object.getPrototypeOf(viaBound)!==ap)throw 'constructor call paths';
if(af().toString()!=='async function anonymous(\n) {\n\n}')throw 'empty dynamic source';
if(Function('return 1;').toString()!=='function anonymous(\n) {\nreturn 1;\n}')throw 'ordinary source control';
'ok';
"#,
    );
}

#[test]
fn async_function_intrinsics_dynamic_fragments_validate_independently_without_execution() {
    succeeds(
        r#"
var af=Object.getPrototypeOf(async function(){}).constructor,effects=0,caught=0;
for(var pair of [
 ['a/*','*/return 1;'],['a) { effects=1; } //','return 2;'],['a','} effects=2; {'],
 ['a) { } ); effects=3; (async function(','return 4;'],['a','}); effects=5; (async function(){'],
 ['a/* */) { effects=6; } /*','*/'],['','effects=7;return )'],['a=1','"use strict";return a;']
]){
 try{af(pair[0],pair[1]);}catch(e){if(e.name==='SyntaxError')caught++;else throw e;}
}
if(caught!==8||effects!==0)throw 'fragment boundary or early error';
var f=af('a// trailing parameter comment','return a;// trailing body comment');
if(f.length!==1||Object.getPrototypeOf(f)!==af.prototype)throw 'valid fragment comments';
var marker={},log='';
function text(mark,value){return {toString:function(){log+=mark;return value;}};}
try{af(text('p','a)'),text('b','return )'));}catch(e){if(e.name!=='SyntaxError')throw e;}
if(log!=='pb')throw 'all source converted before syntax';
log='';var identity=false;
try{af({toString:function(){log+='x';throw marker;}},text('y','return 1;'));}catch(e){identity=e===marker;}
if(!identity||log!=='x')throw 'conversion abrupt identity';
var symbolDenied=false;try{af(Symbol());}catch(e){symbolDenied=e.name==='TypeError';}
if(!symbolDenied)throw 'symbol source conversion';
'ok';
"#,
    );
}

#[test]
fn async_function_intrinsics_dynamic_policy_uses_existing_conversion_and_refusal_order() {
    let mut runtime = Runtime::for_capacity_test();
    let mut host = TestHost {
        denied: true,
        ..TestHost::default()
    };
    runtime
        .execute(
            r#"
var af=Object.getPrototypeOf(async function(){}).constructor,log='';
var p={toString:function(){log+='p';return 'a)';}};
var b={toString:function(){log+='b';return 'return )';}};
"#,
            &mut host,
        )
        .unwrap();
    let programs = runtime.programs.len();
    let functions = runtime.function_arena.occupied;
    assert_eq!(
        runtime
            .execute(
                "var error='';try{af(p,b);}catch(e){error=e.name;}log+':'+error;",
                &mut host
            )
            .unwrap()
            .to_text(),
        "pb:EvalError"
    );
    assert_eq!(host.policy_windows, [None]);
    assert_eq!(runtime.programs.len(), programs + 1);
    assert_eq!(runtime.function_arena.occupied, functions);
    assert_eq!(runtime.call_depth, 0);
}

#[test]
fn async_function_intrinsics_lazy_canonical_parent_and_failed_initialization_are_bounded() {
    let mut runtime = Runtime::for_capacity_test();
    let mut host = TestHost::default();
    assert!(runtime.async_function_roots.prototype.is_none());
    let functions = runtime.function_arena.occupied;
    runtime
        .execute("function ordinary(){return 1;}0;", &mut host)
        .unwrap();
    assert_eq!(runtime.function_arena.occupied, functions + 1);
    assert!(runtime.async_function_roots.prototype.is_none());
    assert_eq!(runtime.execute(r#"
var originalFunction=Function;delete Function.prototype.constructor;Function=17;
var af=Object.getPrototypeOf(async function(){}).constructor;
if(Object.getPrototypeOf(af)!==originalFunction||Object.getPrototypeOf(af.prototype)!==originalFunction.prototype)throw 'canonical parent';
'ok';
"#, &mut host).unwrap().to_text(), "ok");

    let mut exhausted = Runtime::for_capacity_test();
    let capacity = exhausted.limits.capacity.functions;
    exhausted.limits.capacity.functions = exhausted.function_arena.occupied;
    assert!(exhausted
        .async_function_prototype()
        .unwrap_err()
        .message
        .contains("Function limit reached"));
    let objects = exhausted.objects.len();
    exhausted.limits.capacity.functions = capacity;
    assert!(exhausted
        .async_function_prototype()
        .unwrap_err()
        .message
        .contains("bounded initialization failure"));
    assert_eq!(exhausted.objects.len(), objects);
    assert!(exhausted.async_function_roots.prototype.is_none());
    assert!(exhausted.async_function_roots.constructor.is_none());
}

#[test]
fn async_function_intrinsics_per_realm_dynamic_scope_policy_and_roots() {
    let mut runtime = Runtime::for_capacity_test();
    let mut host = TestHost::default();
    runtime
        .execute(
            "var secret=91;async function main(){}var mainPrototype=Object.getPrototypeOf(main);",
            &mut host,
        )
        .unwrap();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let (constructor, prototype) = runtime
        .with_intrinsic_realm(Some(realm), |runtime| {
            runtime.bind_browser_global(77).map_err(Fault::new)?;
            runtime
                .execute("var secret=17;async function child(){}", &mut host)
                .map_err(Fault::new)?;
            Ok((
                runtime.async_function_roots.constructor.unwrap(),
                runtime.async_function_roots.prototype.unwrap(),
            ))
        })
        .unwrap();
    runtime
        .define_global("ChildAsyncFunction", Value::Function(constructor))
        .unwrap();
    runtime
        .define_global("ChildAsyncPrototype", Value::Object(prototype))
        .unwrap();
    assert_eq!(runtime.execute(r#"
if(mainPrototype===ChildAsyncPrototype||Object.getPrototypeOf(ChildAsyncPrototype)===Function.prototype)throw 'distinct realm prototypes';
var fromChild=ChildAsyncFunction('return secret;');
if(Object.getPrototypeOf(fromChild)!==ChildAsyncPrototype)throw 'dynamic owner prototype';
if(fromChild.toString()!=='async function anonymous(\n) {\nreturn secret;\n}')throw 'child retained source';
var childObserved=0;fromChild().then(function(value){childObserved=value;});
delete ChildAsyncPrototype.constructor;ChildAsyncFunction=null;
'ok';
"#, &mut host).unwrap().to_text(), "ok");
    assert_eq!(host.policy_windows, [Some(77)]);
    assert!(runtime.microtask_checkpoint(&mut host).unwrap().is_empty());
    assert_eq!(
        runtime
            .execute("childObserved;", &mut host)
            .unwrap()
            .to_number(),
        17.0
    );
    let mut roots = Vec::new();
    runtime.reachability_intrinsic_edges(&mut |edge| roots.push(edge));
    assert!(roots.iter().any(
        |edge| matches!(edge, function_reachability::Edge::Function(id) if *id == constructor)
    ));
    assert!(matches!(
        runtime.functions[constructor],
        Function::Native(Builtin::AsyncFunction)
    ));
    runtime
        .with_intrinsic_realm(Some(realm), |runtime| {
            assert_eq!(runtime.async_function_roots.constructor, Some(constructor));
            assert_eq!(runtime.async_function_prototype()?, prototype);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        (
            runtime.active_realm,
            runtime.global_environment,
            runtime.call_depth
        ),
        (None, 0, 0)
    );
    assert!(runtime.transient_registration_is_empty());
}
