use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        Err("unexpected native read".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        Err("unexpected native write".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        Err("unexpected native call".into())
    }
}

fn execute(runtime: &mut Runtime, source: &str) {
    runtime.execute(source, &mut NoHost).unwrap();
    assert!(runtime.transient_registration_is_empty());
}
fn install(runtime: &mut Runtime, target: &str, handler: &str, name: &str) -> usize {
    let target = runtime
        .binding_value(runtime.global_environment, target)
        .unwrap();
    let handler = runtime
        .binding_value(runtime.global_environment, handler)
        .unwrap();
    let proxy = runtime.new_ordinary_proxy(&target, &handler).unwrap();
    let Value::Object(id) = proxy else {
        panic!("genuine Proxy")
    };
    runtime.define_global(name, proxy).unwrap();
    id
}
fn fixture(source: &str) -> (Runtime, usize) {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, source);
    let proxy = install(&mut runtime, "target", "handler", "proxy");
    (runtime, proxy)
}

#[test]
fn proxy_traps_get_dynamic_inherited_lookup_exact_inputs_and_abrupt_completion() {
    let (mut runtime, _) = fixture(
        r#"
var calls=0,lookups=0,getters=0,key=Symbol('original'),marker={},receiver={label:'receiver'};
var target={get x(){getters++;return this.label;}},base={},handler=Object.create(base);
Object.defineProperty(base,'get',{configurable:true,get:function(){
  if(this!==handler)throw 'trap lookup receiver';
  lookups++;
  return function(t,k,r){
    'use strict';
    if(this!==handler||t!==target||r!==receiver)throw 'trap inputs';
    calls++;return k===key?k:r.label+':'+k;
  };
}});
"#,
    );
    execute(
        &mut runtime,
        r#"
if(Reflect.get(proxy,'x',receiver)!=='receiver:x'||Reflect.get(proxy,key,receiver)!==key||
   lookups!==2||calls!==2||getters!==0)throw 'dynamic get';
Object.defineProperty(base,'get',{value:null,writable:true,configurable:true});
if(Reflect.get(proxy,'x',receiver)!=='receiver'||getters!==1)throw 'null forward receiver';
delete base.get;
if(Reflect.get(proxy,'x',receiver)!=='receiver'||getters!==2)throw 'deleted forward';
handler.get=3;
var caught=false;try{Reflect.get(proxy,'x',receiver);}catch(e){caught=e.name==='TypeError';}
if(!caught||getters!==2)throw 'noncallable trap';
handler.get=function(){throw marker;};
caught=false;try{Reflect.get(proxy,'x',receiver);}catch(e){caught=e===marker;}
if(!caught)throw 'get exception identity';
"#,
    );
}

#[test]
fn proxy_traps_set_dynamic_boolean_exact_inputs_and_assignment_policy() {
    let (mut runtime, proxy) = fixture(
        r#"
var lookups=0,calls=0,answer={},payload={toString:function(){throw 'coerced payload';}},
    receiver=Object.freeze({}),key=Symbol('write'),base={},handler=Object.create(base),target={};
Object.defineProperty(base,'set',{configurable:true,get:function(){
  if(this!==handler)throw 'set lookup receiver';lookups++;
  return function(t,k,v,r){'use strict';
    if(this!==handler||t!==target||k!==key||v!==payload||r!==receiver)throw 'set inputs';
    calls++;return answer;
  };
}});
"#,
    );
    execute(
        &mut runtime,
        r#"
if(Reflect.set(proxy,key,payload,receiver)!==true||lookups!==1||calls!==1||Object.hasOwn(target,key))throw 'truthy set';
answer=0;
if(Reflect.set(proxy,key,payload,receiver)!==false||lookups!==2||calls!==2||Object.hasOwn(target,key))throw 'false set';
Object.defineProperty(base,'set',{value:function(){return false;},writable:true,configurable:true});
proxy.x=9;
var caught=false;try{(function(){'use strict';proxy.x=9;})();}catch(e){caught=e.name==='TypeError';}
if(!caught||Object.hasOwn(target,'x'))throw 'assignment failure policy';
base.set=false;caught=false;
try{Reflect.set(proxy,'x',9);}catch(e){caught=e.name==='TypeError';}
if(!caught)throw 'noncallable set';
"#,
    );
    assert!(runtime.objects[proxy].properties.is_empty());
    assert!(runtime.objects[proxy].symbol_properties.is_empty());
}

#[test]
fn proxy_traps_frozen_data_accessors_and_same_value_nan_signed_zero() {
    let (mut runtime, _) = fixture(
        r#"
var target={},answers={},handler={get:function(t,k){return answers[k];},set:function(){return true;}};
Object.defineProperty(target,'nan',{value:NaN});
Object.defineProperty(target,'positive',{value:0});
Object.defineProperty(target,'negative',{value:-0});
Object.defineProperty(target,'setterOnly',{set:function(){}});
Object.defineProperty(target,'getterOnly',{get:function(){return 13;}});
answers.nan=NaN;answers.positive=0;answers.negative=-0;answers.setterOnly=undefined;
function denied(f){try{f();}catch(e){if(e.name==='TypeError')return true;throw e;}return false;}
"#,
    );
    execute(
        &mut runtime,
        r#"
var n=proxy.nan;
if(n===n||1/proxy.positive!==Infinity||1/proxy.negative!==-Infinity||proxy.setterOnly!==undefined)throw 'SameValue get';
answers.positive=-0;answers.negative=0;answers.setterOnly=7;
if(!denied(function(){return proxy.positive;})||!denied(function(){return proxy.negative;})||
   !denied(function(){return proxy.setterOnly;}))throw 'frozen get invariant';
if(Reflect.set(proxy,'nan',NaN)!==true||Reflect.set(proxy,'positive',0)!==true||
   Reflect.set(proxy,'negative',-0)!==true||Reflect.set(proxy,'setterOnly',7)!==true)throw 'SameValue set';
if(!denied(function(){Reflect.set(proxy,'positive',-0);})||
   !denied(function(){Reflect.set(proxy,'negative',0);})||
   !denied(function(){Reflect.set(proxy,'getterOnly',undefined);}))throw 'frozen set invariant';
handler.set=function(){return false;};
if(Reflect.set(proxy,'positive',-0)!==false)throw 'false result precedes invariant check';
"#,
    );
}

#[test]
fn proxy_traps_invariants_read_descriptors_after_original_callback() {
    let (mut runtime, _) = fixture(
        r#"
var target={},handler={},key=Symbol('post-callback');
function denied(f){try{f();}catch(e){if(e.name==='TypeError')return true;throw e;}return false;}
"#,
    );
    execute(
        &mut runtime,
        r#"
handler.get=function(t,k){Object.defineProperty(t,k,{value:19});return 18;};
if(!denied(function(){return proxy.created;}))throw 'new frozen string after get';
if(!denied(function(){return proxy[key];}))throw 'new frozen symbol after get';
handler.get=function(t,k){Object.defineProperty(t,k,{set:function(){}});return 4;};
if(!denied(function(){return proxy.noGetter;}))throw 'new frozen accessor after get';
handler.set=function(t,k,v){Object.defineProperty(t,k,{value:23});return true;};
if(!denied(function(){Reflect.set(proxy,'setBad',24);})||
   Reflect.set(proxy,'setSame',23)!==true)throw 'new frozen data after set';
handler.set=function(t,k,v){Object.defineProperty(t,k,{get:function(){return v;}});return true;};
if(!denied(function(){Reflect.set(proxy,'noSetter',23);}))throw 'new frozen accessor after set';
"#,
    );
}

#[test]
fn proxy_traps_fallback_uses_distinct_proxy_receiver_target_and_setter_this() {
    let (mut runtime, proxy) = fixture(
        r#"
var target={x:1},handler={},other={},otherHandler={},key=Symbol('data'),seenThis,seenValue,marker={};
var parent={set amount(v){seenThis=this;seenValue=v;this.saved=v;},get amount(){return this.saved;}};
Object.setPrototypeOf(target,parent);
"#,
    );
    let other_proxy = install(&mut runtime, "other", "otherHandler", "otherProxy");
    execute(
        &mut runtime,
        r#"
if(Reflect.set(proxy,'x',9,otherProxy)!==true||target.x!==1||other.x!==9)throw 'distinct receiver data';
var d=Object.getOwnPropertyDescriptor(other,'x');
if(!d.writable||!d.enumerable||!d.configurable)throw 'created descriptor';
if(Reflect.set(proxy,key,13,otherProxy)!==true||other[key]!==13||Object.hasOwn(target,key))throw 'symbol receiver';
if(Reflect.set(proxy,'amount',marker,otherProxy)!==true||seenThis!==otherProxy||seenValue!==marker||
   other.saved!==marker||Reflect.get(proxy,'amount',otherProxy)!==marker)throw 'accessor original receiver';
handler.set=null;
if(Reflect.set(proxy,'own',17)!==true||target.own!==17)throw 'same proxy forwards to original target';
if(Reflect.set(proxy,'x',3,undefined)!==false||Reflect.set(proxy,'x',3,4)!==false)throw 'primitive data receiver';
"#,
    );
    for id in [proxy, other_proxy] {
        assert!(runtime.objects[id].properties.is_empty());
        assert!(runtime.objects[id].symbol_properties.is_empty());
    }
    execute(&mut runtime,"otherHandler.getOwnPropertyDescriptor=function(){throw 'must not invoke unsupported trap';};");
    let error = runtime
        .execute("Reflect.set(proxy,'x',99,otherProxy);", &mut NoHost)
        .unwrap_err();
    assert!(
        error.contains("getOwnPropertyDescriptor trap is not implemented"),
        "{error}"
    );
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    execute(&mut runtime,"delete otherHandler.getOwnPropertyDescriptor;otherHandler.defineProperty=function(){throw 'must not invoke unsupported trap';};");
    let error = runtime
        .execute("Reflect.set(proxy,'x',99,otherProxy);", &mut NoHost)
        .unwrap_err();
    assert!(
        error.contains("defineProperty trap is not implemented"),
        "{error}"
    );
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    execute(
        &mut runtime,
        "if(other.x!==9||target.x!==1)throw 'unsupported trap changed data';",
    );
}

#[test]
fn proxy_traps_forwarded_define_revalidates_mutated_descriptor_without_wrapper_storage() {
    for mode in ["same", "different", "deleted", "accessor"] {
        let (mut runtime, proxy) = fixture(
            r#"
var target={},handler={},other={x:7},otherHandler={},mode;
"#,
        );
        let other_proxy = install(&mut runtime, "other", "otherHandler", "otherProxy");
        runtime
            .define_global("mode", Value::String(mode.into()))
            .unwrap();
        execute(
            &mut runtime,
            r#"
var lookups=0;
Object.defineProperty(otherHandler,'defineProperty',{get:function(){
  lookups++;
  if(mode==='deleted')delete other.x;
  else if(mode==='accessor')Object.defineProperty(other,'x',{get:function(){return 88;},configurable:true});
  else Object.defineProperty(other,'x',{value:7,writable:false,configurable:false});
  return null;
}});
var assigned=Reflect.set(proxy,'x',mode==='different'?8:7,otherProxy);
var d=Object.getOwnPropertyDescriptor(other,'x');
if(lookups!==1||assigned!==(mode!=='different')||other.x!==7||Object.hasOwn(target,'x'))throw 'live define lookup';
if(mode==='deleted'&&(d.writable||d.enumerable||d.configurable))throw 'value-only new descriptor';
if(mode==='accessor'&&(d.get!==undefined||d.writable||!d.enumerable||!d.configurable))throw 'accessor to data';
"#,
        );
        for id in [proxy, other_proxy] {
            assert!(runtime.objects[id].properties.is_empty());
        }
    }
}

#[test]
fn proxy_traps_failed_work_and_depth_restore_roots_without_refill_or_callbacks() {
    let (mut runtime, proxy) = fixture(
        r#"
var target={x:7},calls=0,handler={get get(){calls++;return function(){return 8;};}};
"#,
    );
    let receiver = Value::Object(proxy);
    runtime.begin_job();
    runtime.copy_bytes = 0;
    let error = runtime
        .proxy_get(
            proxy,
            PropertyKey::String("x".into()),
            &receiver,
            &mut NoHost,
        )
        .unwrap_err();
    assert!(error.message.contains("copy work limit"));
    assert_eq!(runtime.copy_bytes, 0);
    assert_eq!(runtime.call_depth, 0);
    assert_eq!(runtime.transient_roots.owned_call_depth, 0);
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    execute(
        &mut runtime,
        "if(calls!==0||target.x!==7)throw 'failed work ran getter';",
    );
    runtime.call_depth = MAX_CALL_DEPTH;
    let error = runtime
        .proxy_get(
            proxy,
            PropertyKey::String("x".into()),
            &receiver,
            &mut NoHost,
        )
        .unwrap_err();
    assert!(error.message.contains("Proxy operation call limit"));
    assert_eq!(runtime.call_depth, MAX_CALL_DEPTH);
    runtime.call_depth = 0;
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    assert_eq!(
        runtime
            .proxy_get(
                proxy,
                PropertyKey::String("x".into()),
                &receiver,
                &mut NoHost
            )
            .unwrap()
            .to_number(),
        8.0
    );
    assert!(runtime.transient_registration_is_empty());
}
