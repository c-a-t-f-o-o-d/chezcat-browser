//! Original Number fixed-point conversion and protocol controls.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("No native reads in Number formatting.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("No native writes in Number formatting.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("No native calls in Number formatting.".into())
    }
}

fn succeeds(source: &str) {
    assert_eq!(Runtime::for_capacity_test().execute(source, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn number_to_fixed_original_scalar_interpolation_and_exact_decimal_rounding() {
    succeeds(r#"
if((0.5).toFixed(3)!=='0.500'||(0).toFixed(3)!=='0.000'||(1).toFixed(3)!=='1.000')throw 'scalar midpoint and endpoints';
var cases=[
 [2.5,0,'3'],[-2.5,0,'-3'],[1.25,1,'1.3'],[-1.25,1,'-1.3'],
 [2.55,1,'2.5'],[2.35,1,'2.4'],[1.005,2,'1.00'],[2.675,2,'2.67'],
 [9.999,2,'10.00'],[-0,2,'0.00'],[-0.001,2,'-0.00'],
 [1000000000000000128,0,'1000000000000000128'],
 [999999999999999868928,0,'999999999999999868928']
];
for(var row of cases){if(Number.prototype.toFixed.call(row[0],row[1])!==row[2])throw 'binary rounding '+row[0];}
if((1000000000000000128).toString()===(1000000000000000128).toFixed(0))throw 'shortest decimal is not fixed';
'ok';
"#);
}

#[test]
fn number_to_fixed_precision_extremes_padding_subnormal_and_existing_exponent_model() {
    let mut runtime = Runtime::for_capacity_test();
    for (number, digits, expected) in [
        (0.0, 100, format!("0.{}", "0".repeat(100))),
        (-f64::from_bits(1), 100, format!("-0.{}", "0".repeat(100))),
        (f64::MIN_POSITIVE, 100, format!("0.{}", "0".repeat(100))),
        (1e-100, 100, format!("0.{}1", "0".repeat(99))),
        (0.1, 100, format!("0.1000000000000000055511151231257827021181583404541015625{}", "0".repeat(45))),
        (1e20, 100, format!("100000000000000000000.{}", "0".repeat(100))),
        (f64::from_bits(0.5f64.to_bits() - 1), 0, "0".into()),
        (0.5, 0, "1".into()),
        (f64::from_bits(0.5f64.to_bits() + 1), 0, "1".into()),
    ] {
        runtime.begin_job();
        assert_eq!(runtime.number_to_fixed(number, digits).unwrap().to_text(), expected);
    }
    succeeds(r#"
if((1).toFixed(100).length!==102||(-1).toFixed(100).length!==103)throw 'precision bound';
for(var n of [1e21,-1e21,1e22,1.23456789e30,Number.MAX_VALUE,NaN,Infinity,-Infinity]){
 if(n.toFixed(100)!==String(n))throw 'existing exponential and nonfinite model';
}
if((12).toFixed()!=='12'||(12).toFixed(undefined)!=='12'||(12.5).toFixed(NaN)!=='13'||(12.5).toFixed(-0.9)!=='13'||(12.5).toFixed('1.9')!=='12.5'||(1).toFixed(100.9).length!==102)throw 'ToIntegerOrInfinity';
'ok';
"#);
}

#[test]
fn number_to_fixed_number_brand_precedes_fraction_coercion_and_ignores_author_valueof() {
    succeeds(r#"
var fixed=Number.prototype.toFixed,wrapped=new Number(1.25),hits=0,marker={};
wrapped.valueOf=function(){throw 'receiver coercion';};wrapped.toString=function(){throw 'receiver stringification';};
Object.setPrototypeOf(wrapped,null);
if(fixed.call(wrapped,1)!=='1.3'||fixed.call(Number.prototype,2)!=='0.00')throw 'real number slot';
var fraction={valueOf:function(){hits++;throw marker;}},caught=0;
for(var receiver of [{},Object.create(Number.prototype),new Boolean(true),'1',Symbol('number'),null,undefined,function(){}]){
 try{fixed.call(receiver,fraction);}catch(e){if(e.name!=='TypeError')throw e;caught++;}
}
if(caught!==8||hits!==0)throw 'brand before digits';
var caughtMarker=false;
try{fixed.call(wrapped,fraction);}catch(e){caughtMarker=e===marker;}
if(!caughtMarker||hits!==1)throw 'one fraction conversion/author exception identity';
var order='',digits={};
Object.defineProperty(digits,Symbol.toPrimitive,{get:function(){order+='get,';return function(hint){if(this!==digits||hint!=='number')throw 'hint/receiver';order+='call,';return 2.8;};}});
if(fixed.call(1.25,digits)!=='1.25'||order!=='get,call,')throw 'ordered numeric conversion';
var reentrant={valueOf:function(){if(fixed.call(2.5,0)!=='3')throw 'nested call';return 1;}};
if(fixed.call(1.25,reentrant)!=='1.3')throw 'reentrant conversion';
'ok';
"#);
}

#[test]
fn number_to_fixed_range_and_symbol_errors_precede_nonfinite_and_large_shortcuts() {
    succeeds(r#"
var fixed=Number.prototype.toFixed,count=0;
for(var n of [NaN,Infinity,-Infinity,1e21,-1e21,0]){
 for(var digits of [-1,101,Infinity,-Infinity]){
  try{fixed.call(n,digits);throw 'missing range refusal';}catch(e){if(e.name!=='RangeError')throw e;count++;}
 }
 for(var symbol of [Symbol('digits'),Object(Symbol('boxed digits'))]){
  try{fixed.call(n,symbol);throw 'missing symbol refusal';}catch(e){if(e.name!=='TypeError')throw e;count++;}
 }
}
if(count!==36)throw 'conversion/range matrix';
var marker={},bad={valueOf:function(){throw marker;}},caught=false;
try{fixed.call(Infinity,bad);}catch(e){caught=e===marker;}
if(!caught)throw 'nonfinite does not skip digits conversion';
'ok';
"#);
}

#[test]
fn number_to_fixed_native_metadata_and_observable_prototype_mutations() {
    succeeds(r#"
var prototype=Number.prototype,d=Object.getOwnPropertyDescriptor(prototype,'toFixed'),fixed=prototype.toFixed;
if(d.value!==fixed||!d.writable||!d.configurable||d.enumerable||fixed.name!=='toFixed'||fixed.length!==1)throw 'method descriptor';
if(Object.prototype.hasOwnProperty.call(fixed,'prototype')||Object.prototype.hasOwnProperty.call(fixed,'caller')||Object.prototype.hasOwnProperty.call(fixed,'arguments'))throw 'no own native constructor/frame slots';
if(Function.prototype.toString.call(fixed).indexOf('[native code]')<0)throw 'native method source';
var conversions=0,caught=0,evaluations=0,digits={valueOf:function(){conversions++;return 1;}};
function argument(){evaluations++;return digits;}
try{new fixed(argument());}catch(e){if(e.name==='TypeError')caught++;else throw e;}
if(caught!==1||evaluations!==1||conversions!==0)throw 'argument evaluation before nonconstructor TypeError without numeric conversion';
var marker={},same=false;
function throwingArgument(){evaluations++;throw marker;}
try{new fixed(throwingArgument());}catch(e){same=e===marker;}
if(!same||evaluations!==2||conversions!==0)throw 'argument abrupt completion before nonconstructor check';
prototype.toFixed=function(digits){return this.valueOf()+':'+digits;};
if((2).toFixed(3)!=='2:3'||fixed.call(2,3)!=='2.000')throw 'ordinary replacement/saved method';
delete prototype.toFixed;
if((2).toFixed!==undefined||'toFixed' in prototype||Object.getOwnPropertyDescriptor(prototype,'toFixed')!==undefined)throw 'deletion no ghost fallback';
var seen,gets=0;
Object.defineProperty(prototype,'toFixed',{get:function(){'use strict';seen=this;gets++;return fixed;},configurable:true});
if((2).toFixed(1)!=='2.0'||seen!==2||gets!==1)throw 'ordinary inherited getter receiver';
Object.defineProperty(prototype,'toFixed',d);
var boxed=new Number(2.5);boxed.toFixed=function(){return 'own';};
if(boxed.toFixed()!=='own'||fixed.call(boxed)!=='3'||fixed.bind(boxed,1)()!=='2.5')throw 'own override/native slot/binding';
if(Boolean.prototype.toFixed!==undefined||String.prototype.toFixed!==undefined)throw 'number only';
'ok';
"#);
}

#[test]
fn number_to_fixed_owner_realm_errors_boxed_receivers_and_caller_restoration() {
    let mut runtime = Runtime::for_capacity_test();
    let realm = runtime.allocate_intrinsic_realm().unwrap();
    let values = runtime.with_intrinsic_realm(Some(realm), |runtime| {
        ["Number", "TypeError", "RangeError", "Function"].iter()
            .map(|name| runtime.binding_value(runtime.global_environment, name).map(|value| (*name, value)))
            .collect::<Result<Vec<_>, _>>()
    }).unwrap();
    for (name, value) in values {
        runtime.define_global(&format!("Child{name}"), value).unwrap();
    }
    runtime.execute(r#"
var child=ChildNumber.prototype.toFixed,main=Number.prototype.toFixed;
if(child===main||Object.getPrototypeOf(child)!==ChildFunction.prototype)throw 'owner intrinsic identity';
if(child.call(new Number(1.25),1)!=='1.3'||main.call(new ChildNumber(1.25),1)!=='1.3')throw 'cross-realm genuine slots';
var count=0;
try{child.call({});}catch(e){if(Object.getPrototypeOf(e)===ChildTypeError.prototype)count++;}
try{child.call(0,101);}catch(e){if(Object.getPrototypeOf(e)===ChildRangeError.prototype)count++;}
try{child.call(0,Symbol());}catch(e){if(Object.getPrototypeOf(e)===ChildTypeError.prototype)count++;}
var marker={},fraction={valueOf:function(){throw marker;}};
try{child.call(1,fraction);}catch(e){if(e===marker)count++;}
if(count!==4||main.call(2.5)!=='3'||Object.getPrototypeOf([])!==Array.prototype)throw 'realm restoration';
'ok';
"#, &mut NoHost).unwrap();
    assert_eq!(runtime.active_realm, None);
    assert_eq!(runtime.global_environment, 0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn number_to_fixed_work_output_steps_and_deadline_fail_without_publishing_or_resetting() {
    let mut probe = Runtime::for_capacity_test();
    probe.begin_job();
    let start = (probe.steps, probe.copy_bytes);
    let expected = probe.number_to_fixed(0.1, 100).unwrap().to_text();
    let cost = (start.0 - probe.steps, start.1 - probe.copy_bytes);
    for failure in ["steps", "late-steps", "copy", "output", "string", "deadline"] {
        let mut runtime = Runtime::for_capacity_test();
        runtime.begin_job();
        let before = (runtime.functions.len(), runtime.objects.len(), runtime.environments.len(), runtime.bindings, runtime.value_bytes);
        match failure {
            "steps" => runtime.steps = 0,
            "late-steps" => runtime.steps = cost.0 - 1,
            "copy" => runtime.copy_bytes = 0,
            "output" => runtime.copy_bytes = cost.1 - 1,
            "string" => runtime.limits.capacity.string_bytes = expected.len() - 1,
            "deadline" => runtime.job_deadline = std::time::Instant::now() - std::time::Duration::from_secs(1),
            _ => unreachable!(),
        }
        let deadline = runtime.job_deadline;
        let error = runtime.number_to_fixed(0.1, 100).unwrap_err();
        assert!(error.exception.is_none(), "{failure}");
        assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.environments.len(), runtime.bindings, runtime.value_bytes), before, "{failure}");
        assert_eq!(runtime.job_deadline, deadline);
        assert!(runtime.transient_registration_is_empty());
        if failure == "output" {
            assert_eq!(runtime.copy_bytes, 0);
            assert!(runtime.work_attribution.failure_report().unwrap().contains("category=conversion-output"));
        }
        runtime.begin_job();
        assert_eq!(runtime.number_to_fixed(2.5, 0).unwrap().to_text(), "3");
    }
    let mut runtime = Runtime::for_capacity_test();
    runtime.begin_job();
    runtime.copy_bytes = cost.1;
    assert_eq!(runtime.number_to_fixed(0.1, 100).unwrap().to_text(), expected);
    assert_eq!(runtime.copy_bytes, 0);
    let before = (runtime.objects.len(), runtime.functions.len());
    let error = runtime.primitive_value_method("toFixed", true, Value::Host(41), &[Value::Host(42)], &mut NoHost).unwrap_err();
    assert!(matches!(error.exception.as_deref(), Some(Exception::Error("TypeError"))));
    assert_eq!((runtime.objects.len(), runtime.functions.len()), before);
}

#[test]
fn number_to_fixed_normal_calls_keep_fatal_limits_and_recover_on_a_fresh_job() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute(r#"
var entered=false,caught=false,after=false,fixed=Number.prototype.toFixed;
function format(){
 entered=true;
 try{fixed.call(0.1,100);}catch(e){caught=true;}
 after=true;
}
"#, &mut NoHost).unwrap();
    runtime.begin_job();
    runtime.steps = 1024;
    let error = runtime.execute("format();", &mut NoHost).unwrap_err();
    assert!(error.contains("Execution step limit"), "{error}");
    assert_eq!((runtime.eval_depth, runtime.call_depth, runtime.evaluation_frames), (0, 0, 0));
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    assert_eq!(runtime.execute("entered&&!caught&&!after&&fixed.call(2.5)==='3';", &mut NoHost).unwrap().to_text(), "true");
}
