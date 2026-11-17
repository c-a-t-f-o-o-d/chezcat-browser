//! Original reduced controls for the finite static colon prototype setter.
//! Colon prototype setters stay distinct from ordinary own __proto__ data keys.
use super::*;
use super::super::Host;

#[derive(Default)]
struct GuardHost { gets: usize, writes: usize, calls: usize }
impl Host for GuardHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        self.gets += 1; Err("No native read is permitted by this control.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        self.writes += 1; Err("No native write is permitted by this control.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        self.calls += 1; Err("No native call is permitted by this control.".into())
    }
}
fn run(source: &str) -> Value {
    Runtime::for_test().execute(source, &mut GuardHost::default()).unwrap()
}

#[test]
fn literal_prototype_null_retains_real_ordinary_fields_without_an_own_special_key() {
    let mut runtime = Runtime::for_test();
    let value = runtime.execute(r#"
        var object={__proto__:null,marker:7};
        if(Object.getPrototypeOf(object)!==null||object.marker!==7||object.toString!==undefined)throw 'null prototype';
        if(Object.keys(object).join()!=='marker'||Object.getOwnPropertyNames(object).join()!=='marker')throw 'own keys';
        Object.freeze(object);
        var descriptor=Object.getOwnPropertyDescriptor(object,'marker');
        if(!Object.isFrozen(object)||descriptor.writable||descriptor.configurable||!descriptor.enumerable)throw 'real freeze';
        if(Object.getPrototypeOf(object)!==null)throw 'freeze changed prototype';
        object;
    "#, &mut GuardHost::default()).unwrap();
    let Value::Object(object) = value else { panic!("Expected a genuine ordinary object"); };
    assert_eq!(runtime.objects[object].prototype, None);
    assert!(!runtime.objects[object].properties.contains_key("__proto__"));
}

#[test]
fn literal_prototype_object_and_function_keep_identity_and_never_coerce_the_rhs() {
    run(r#"
        var writes=0,parent={
            get inherited(){return this.own;},
            set own(value){writes++;},
            toString:function(){throw 'object coerced';},
            valueOf:function(){throw 'object coerced';}
        };
        var child={__proto__:parent,own:9};
        if(Object.getPrototypeOf(child)!==parent||child.inherited!==9||writes!==0)throw 'ordinary identity or inherited setter';
        function Parent(){}Parent.marker=11;
        var callableChild={'__proto__':Parent};
        if(Object.getPrototypeOf(callableChild)!==Parent||callableChild.marker!==11)throw 'function prototype identity';
        var unnamed={__proto__:function(){}};
        if(Object.getPrototypeOf(unnamed).name!=='')throw 'prototype RHS inferred a property name';
        var nested={__proto__:{__proto__:null,marker:13}};
        if(nested.marker!==13||Object.getPrototypeOf(Object.getPrototypeOf(nested))!==null)throw 'independent nested literals';
        var cycle=false;
        try{Object.setPrototypeOf(parent,child);}catch(error){cycle=error.name==='TypeError';}
        if(!cycle||Object.getPrototypeOf(child)!==parent||Object.getPrototypeOf(parent)!==Object.prototype)throw 'shared cycle guard';
    "#);
}

#[test]
fn literal_prototype_primitive_rhs_is_evaluated_and_ignored_without_property_creation() {
    run(r#"
        var values=[undefined,false,true,0,-1,NaN,Infinity,'text',Symbol('ordinary input')],seen=0;
        for(var index=0;index<values.length;index++){
            var object={__proto__:(seen++,values[index]),marker:index};
            if(Object.getPrototypeOf(object)!==Object.prototype||object.marker!==index)throw 'primitive prototype changed';
            if(Object.getOwnPropertyNames(object).join()!=='marker')throw 'ignored primitive became own key';
        }
        if(seen!==values.length)throw 'primitive RHS not evaluated once';
    "#);
}

#[test]
fn literal_prototype_rhs_runs_once_in_source_order_and_preserves_abrupt_identity() {
    run(r#"
        var log='',count=0,parent={};
        function rhs(){log+='P';count++;return parent;}
        var object={before:(log+='A',1),__proto__:rhs(),after:(log+='B',2)};
        if(log!=='APB'||count!==1||Object.getPrototypeOf(object)!==parent)throw 'evaluation order';
        var marker={},same=false;
        try{({before:(log+='C',1),__proto__:(function(){log+='T';throw marker;})(),after:(log+='D',2)});}
        catch(error){same=error===marker;}
        if(!same||log!=='APBCT')throw 'abrupt identity or later property ran';
    "#);
}

#[test]
fn literal_prototype_duplicate_static_setters_are_early_syntax_errors_without_effects() {
    for literal in [
        "({__proto__:null,'__proto__':{}})",
        "({'__proto__':0,__proto__:null})",
        r#"({"\u005f_proto__":false,'__proto__':{}})"#,
    ] {
        let mut runtime = Runtime::for_test();
        let mut host = GuardHost::default();
        runtime.execute("var untouched=0;", &mut host).unwrap();
        let error = runtime.execute(&format!("untouched=1;{literal};"), &mut host).unwrap_err();
        assert!(error.contains("Duplicate __proto__ prototype setter"), "{error}");
        runtime.begin_job();
        assert_eq!(runtime.execute("untouched;", &mut host).unwrap().to_text(), "0");
    }
    run(r#"
        var effects=0,caught=false;
        try{eval("effects=1;({__proto__:null,'__proto__':0});");}
        catch(error){caught=error.name==='SyntaxError';}
        if(!caught||effects!==0)throw 'duplicate eval was not an early SyntaxError';
    "#);
}

#[test]
fn literal_prototype_native_host_refusal_keeps_prior_effects_without_host_operations() {
    let mut runtime = Runtime::for_test();
    let mut host = GuardHost::default();
    runtime.define_global("native", Value::Host(7)).unwrap();
    runtime.execute("var before=false,later=false,caught=false,finalized=false;", &mut host).unwrap();
    let error = runtime.execute("try{({a:(before=true),__proto__:native,b:(later=true)});}catch(error){caught=true;}finally{finalized=true;}", &mut host).unwrap_err();
    assert!(error.contains("Native host object prototypes"), "{error}");
    assert_eq!((host.gets, host.writes, host.calls), (0, 0, 0));
    runtime.begin_job();
    assert_eq!(runtime.execute("before&&!later&&!caught&&!finalized;", &mut host).unwrap().to_text(), "true");
    assert_eq!(runtime.execute("({__proto__:null,answer:4}).answer;", &mut host).unwrap().to_text(), "4");
}

#[test]
fn literal_prototype_name_and_member_caps_still_refuse_before_effects_then_recover() {
    let mut runtime = Runtime::for_test();
    let mut host = GuardHost::default();
    runtime.execute("var untouched=0;", &mut host).unwrap();
    let name_limit = runtime.limits.capacity.property_bytes;
    runtime.limits.capacity.property_bytes = 8;
    let error = runtime.execute("untouched=1;({__proto__:null});", &mut host).unwrap_err();
    assert!(error.contains("Property-name limit"), "{error}");
    runtime.limits.capacity.property_bytes = name_limit;
    runtime.begin_job();
    assert_eq!(runtime.execute("untouched;", &mut host).unwrap().to_text(), "0");
    let member_limit = runtime.limits.heap.object_properties;
    runtime.limits.heap.object_properties = 2;
    let error = runtime.execute("untouched=1;({__proto__:null,a:1,b:2});", &mut host).unwrap_err();
    assert!(error.contains("Object property limit"), "{error}");
    runtime.limits.heap.object_properties = member_limit;
    runtime.begin_job();
    assert_eq!(runtime.execute("untouched+({__proto__:null,answer:4}).answer;", &mut host).unwrap().to_text(), "4");
}

#[test]
fn literal_prototype_walk_work_limit_is_atomic_and_a_fresh_job_recovers() {
    let mut runtime = Runtime::for_test();
    let mut host = GuardHost::default();
    let parent = runtime.execute("var base=null;for(var index=0;index<64;index++)base={__proto__:base};base;", &mut host).unwrap();
    let object = runtime.new_object(None).unwrap();
    let original = runtime.objects[object].prototype;
    runtime.steps = 8;
    let error = runtime.define_literal_prototype(object, &parent).unwrap_err();
    assert!(error.exception.is_none());
    assert!(error.message.contains("limit"), "{}", error.message);
    assert_eq!(runtime.objects[object].prototype, original, "Failed chain walk must not publish a partial link");
    runtime.begin_job();
    runtime.define_literal_prototype(object, &parent).unwrap();
    assert_eq!(runtime.objects[object].prototype, Ordinary::from_value(&parent));
    assert_eq!(runtime.execute("({__proto__:null,answer:4}).answer;", &mut host).unwrap().to_text(), "4");
}

#[test]
fn literal_prototype_deferred_shorthand_methods_and_accessors_still_refuse_explicitly() {
    for source in [
        "var __proto__=1;({__proto__});",
        "({__proto__(){}});",
        "({get __proto__(){return 1;}});",
        "({set __proto__(value){}});",
    ] {
        let mut runtime = Runtime::for_test();
        let error = runtime.execute(source, &mut GuardHost::default()).unwrap_err();
        assert!(error.contains("prototype"), "{source}: {error}");
        runtime.begin_job();
        assert_eq!(runtime.execute("({__proto__:null,answer:4}).answer;", &mut GuardHost::default()).unwrap().to_text(), "4");
    }
}

#[test]
fn literal_prototype_computed_data_keys_descriptors_and_legacy_access_are_ordinary() {
    let mut runtime = Runtime::for_test();
    let mut host = GuardHost::default();
    runtime.execute("var keys=0,rhs=0,later=0;", &mut host).unwrap();
    runtime.execute("var object={[(keys++,'__proto__')]:(rhs++,1),after:(later++,2)};if(object.__proto__!==1||Object.getPrototypeOf(object)!==Object.prototype||Object.getOwnPropertyDescriptor(object,'__proto__').value!==1)throw 'computed own data';", &mut host).unwrap();
    assert_eq!(runtime.execute("keys+':'+rhs+':'+later;", &mut host).unwrap().to_text(), "1:1:1");
    assert_eq!(runtime.execute(r#"var ordinary={};if(!('__proto__' in ordinary)||ordinary.__proto__!==Object.prototype)throw 'legacy lookup';Object.defineProperty(ordinary,'__proto__',{value:7});if(ordinary.__proto__!==7||Object.getPrototypeOf(ordinary)!==Object.prototype)throw 'own descriptor';var json=JSON.parse('{"__proto__":null}');if(json.__proto__!==null||!Object.prototype.hasOwnProperty.call(json,'__proto__')||Object.getPrototypeOf(json)!==Object.prototype)throw 'JSON own data';'ok';"#, &mut host).unwrap().to_text(), "ok");
}
