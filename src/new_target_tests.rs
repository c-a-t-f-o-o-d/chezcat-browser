//! Original invocation-context controls; no class or framework dependency.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("Unexpected host get.".into()) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("Unexpected host set.".into()) }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("Unexpected host call.".into()) }
}

fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
}

#[test]
fn new_target_distinguishes_construction_ordinary_calls_and_parameter_defaults() {
    succeeds(r#"
var observations=[];
function Probe(value=new.target){observations.push(new.target);this.value=value;}
var ordinary={};
if(Probe.call(ordinary)!==undefined||ordinary.value!==undefined||observations[0]!==undefined)throw 'ordinary default';
var instance=new Probe();
if(instance.value!==Probe||observations[1]!==Probe||Object.getPrototypeOf(instance)!==Probe.prototype)throw 'constructed default';
Probe.call(instance,17);
if(instance.value!==17||observations[2]!==undefined)throw 'object this is not construction';
var explicit=new Probe(23);
if(explicit.value!==23||observations[3]!==Probe)throw 'explicit argument';
function Returning(value=new.target){return value;}
if(Returning()!==undefined||new Returning()!==Returning)throw 'callable constructor result';
function Strict(){'use strict';return new.target;}
if(Strict.call({})!==undefined||new Strict()!==Strict)throw 'strict invocation';
'ok';
"#);
}

#[test]
fn new_target_bound_constructors_substitute_each_bound_identity_and_ignore_bound_this() {
    succeeds(r#"
function Target(first,second,observed=new.target){this.first=first;this.second=second;this.observed=observed;}
var ignored={untouched:true},bound=Target.bind(ignored,'first'),twice=bound.bind({other:true},'second');
var one=new bound('one'),two=new twice();
if(one.first!=='first'||one.second!=='one'||one.observed!==Target||Object.getPrototypeOf(one)!==Target.prototype)throw 'single bound construction';
if(two.first!=='first'||two.second!=='second'||two.observed!==Target||Object.getPrototypeOf(two)!==Target.prototype)throw 'nested bound construction';
if(ignored.first!==undefined||ignored.observed!==undefined||!ignored.untouched)throw 'bound this used by construction';
if(twice()!==undefined||ignored.first!=='first'||ignored.second!=='second'||ignored.observed!==undefined)throw 'ordinary bound call';
var replacement={};
Target.call(replacement,'call','value');
if(replacement.observed!==undefined||replacement.first!=='call')throw 'ordinary forwarding';
'ok';
"#);
}

#[test]
fn new_target_arrows_capture_lexically_across_nested_bodies_defaults_call_and_bind() {
    succeeds(r#"
function Factory(parameter=()=>new.target){
 this.parameter=parameter;this.concise=()=>new.target;this.block=()=>{return new.target;};
 this.nested=()=>()=>new.target;this.defaulted=(value=new.target)=>value;
}
var constructed=new Factory(),ordinary={};Factory.call(ordinary);
for(var arrow of [constructed.parameter,constructed.concise,constructed.block,constructed.nested(),constructed.defaulted]){
 if(arrow()!==Factory||arrow.call({})!==Factory||arrow.bind({})()!==Factory)throw 'constructed lexical capture';
}
for(var arrow of [ordinary.parameter,ordinary.concise,ordinary.block,ordinary.nested(),ordinary.defaulted]){
 if(arrow()!==undefined||arrow.call(constructed)!==undefined||arrow.bind(constructed)()!==undefined)throw 'ordinary lexical capture';
}
if(constructed.defaulted(31)!==31)throw 'explicit arrow argument';
var rejected=0;try{new constructed.concise();}catch(e){if(e.name==='TypeError')rejected++;}
if(rejected!==1)throw 'arrow became constructor';
'ok';
"#);
}

#[test]
fn new_target_nested_calls_getters_and_abrupt_default_evaluation_restore_outer_context() {
    succeeds(r#"
var effects=[];
function Plain(){effects.push(new.target);return new.target;}
function Inner(){effects.push(new.target);throw 'inner abrupt';}
function FailDefault(value=PlainThenThrow()){throw 'body reached';}
function PlainThenThrow(){if(new.target!==undefined)throw 'default call inherited target';throw 'default abrupt';}
function Outer(){
 if(new.target!==Outer||Plain()!==undefined||new.target!==Outer)throw 'ordinary nested call';
 try{new Inner();}catch(e){if(e!=='inner abrupt')throw e;}
 if(new.target!==Outer)throw 'constructor throw leaked';
 try{new FailDefault();}catch(e){if(e!=='default abrupt')throw e;}
 if(new.target!==Outer)throw 'default throw leaked';
 var object={};Object.defineProperty(object,'value',{get:function(){if(new.target!==undefined)throw 'getter inherited target';return 37;}});
 if(object.value!==37||new.target!==Outer)throw 'getter context leaked';
 this.capture=()=>new.target;
}
var result=new Outer();
if(effects.length!==2||effects[0]!==undefined||effects[1]!==Inner||result.capture()!==Outer||Plain()!==undefined)throw 'restored identities';
'ok';
"#);
}

#[test]
fn new_target_direct_eval_inherits_function_defaults_and_lexical_arrows() {
    succeeds(r#"
function Probe(value=eval('new.target')){
 if(eval('new.target')!==new.target||eval("'use strict';new.target")!==new.target)throw 'direct eval identity';
 this.value=value;this.fromEval=eval('()=>new.target');this.fromArrow=()=>eval('new.target');
 this.nestedEval=eval("()=>eval('new.target')");
}
var constructed=new Probe(),ordinary={};Probe.call(ordinary);
if(constructed.value!==Probe||ordinary.value!==undefined)throw 'eval parameter context';
for(var arrow of [constructed.fromEval,constructed.fromArrow,constructed.nestedEval])if(arrow()!==Probe)throw 'constructed eval capture';
for(var arrow of [ordinary.fromEval,ordinary.fromArrow,ordinary.nestedEval])if(arrow()!==undefined)throw 'ordinary eval capture';
var dynamic=Function('value=new.target','return value;');
if(dynamic()!==undefined||new dynamic()!==dynamic)throw 'dynamic ordinary function context';
'ok';
"#);
}

#[test]
fn new_target_indirect_eval_and_top_level_arrow_eval_reject_without_leaking_constructor_context() {
    succeeds(r#"
var effects=0,topArrow=()=>eval('effects++;new.target');
function Outer(){
 var rejected=0,indirect=eval;
 for(var run of [function(){return indirect('effects++;new.target');},function(){return (0,eval)('effects++;new.target');},topArrow]){
  try{run();}catch(e){if(e.name==='SyntaxError')rejected++;else throw e;}
  if(new.target!==Outer)throw 'eval rejection changed outer target';
 }
 if(rejected!==3||effects!==0)throw 'outside lexical permission';
 if(eval('new.target')!==Outer)throw 'direct context not restored';
 this.arrow=eval('()=>new.target');
}
var outer=new Outer();if(outer.arrow()!==Outer)throw 'capture after rejected eval';
var rejected=0;try{eval('effects++;new.target');}catch(e){if(e.name==='SyntaxError')rejected++;}
if(rejected!==1||effects!==0)throw 'top-level direct eval permission';
'ok';
"#);
}

#[test]
fn new_target_early_errors_precede_all_program_side_effects_and_reject_invalid_targets() {
    for source in [
        "effects++;new.target;",
        "effects++;()=>new.target;",
        "effects++;()=>{return new.target;};",
        "effects++;(value=new.target)=>value;",
        "effects++;function Outer(){return new.target;}var bad=()=>new.target;",
        "effects++;function Outer(){return ()=>new.target;}var bad=()=>{return new.target;};",
        "effects++;function Invalid(){new.target=1;}",
        "effects++;function Invalid(){new.target++;}",
        "effects++;function Invalid(){return new.other;}",
    ] {
        let mut regexp_work = MAX_REGEXP_WORK;
        let fault = Parser::parse(source, &mut regexp_work).err().expect("invalid new.target context");
        assert!(matches!(fault.exception.as_deref(), Some(Exception::Error("SyntaxError"))),
            "{source}: {fault:?}");
        let mut runtime = Runtime::for_test();
        runtime.execute("var effects=0;", &mut NoHost).unwrap();
        let error = runtime.execute(source, &mut NoHost).unwrap_err();
        // Top-level display preserves source/message, not the exception name.
        // Verify the parser's typed fault above and eval's author-visible type
        // below instead of depending on a diagnostic formatting convention.
        assert!(!runtime.invocation_failed_fatally(), "{source}: {error}");
        runtime.begin_job();
        assert_eq!(runtime.execute("effects;", &mut NoHost).unwrap().to_text(), "0", "{source}");
        runtime.define_global("invalidSource", Value::String(source.into())).unwrap();
        assert_eq!(runtime.execute("var caught;try{eval(invalidSource);}catch(e){caught=e.name;}caught==='SyntaxError'&&effects===0;", &mut NoHost)
            .unwrap().to_text(), "true", "{source}");
    }
}

#[test]
fn new_target_source_reflection_preserves_exact_original_function_and_arrow_text() {
    for (source, expected) in [
        ("function /*original é*/ F (value = new /*gap*/ . target) {\r\n return new.target;\r\n}\r\nF.toString();", "function /*original é*/ F (value = new /*gap*/ . target) {\r\n return new.target;\r\n}"),
        ("function F(){return (value = new.target) => /*arrow*/ new.target;}new F().toString();", "(value = new.target) => /*arrow*/ new.target"),
        ("function F(){return ()=>{ /*block*/ return new.target; };}new F().toString();", "()=>{ /*block*/ return new.target; }"),
    ] {
        let mut runtime = Runtime::for_test();
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), expected, "{source}");
    }
}
