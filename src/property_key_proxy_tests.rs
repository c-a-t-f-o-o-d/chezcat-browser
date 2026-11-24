//! Exact UTF-16 keys across supported Reflect and ordinary Proxy operations.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected native get")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected native set")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected native call")
    }
}

fn execute(runtime: &mut Runtime, source: &str) {
    runtime.execute(source, &mut NoHost).unwrap();
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn raw_key_reflect_get_preserves_units_conversion_order_and_accessor_receiver() {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, r#"
        var high='\ud800', low='\udc00', replacement='\ufffd', parent={}, receiver={};
        var target=Object.create(parent), reads=0, order='', key={};
        Object.defineProperty(parent,high,{get:function(){'use strict';reads++;return this;}});
        parent[low]=17;parent[replacement]=23;
        if(Reflect.get(target,high,receiver)!==receiver || Reflect.get(target,high,null)!==null ||
           Reflect.get(target,low,receiver)!==17 || Reflect.get(target,replacement)!==23 || reads!==2)
            throw 'raw lookup or exact receiver';
        key[Symbol.toPrimitive]=function(hint){
            if(this!==key||hint!=='string')throw 'key conversion inputs';order+='k';return high;
        };
        var caught=false;
        try{Reflect.get(null,key);}catch(e){caught=e.name==='TypeError';}
        if(!caught||order!==''||reads!==2)throw 'target validation precedes conversion';
        if(Reflect.get(target,key,receiver)!==receiver||order!=='k'||reads!==3)
            throw 'single key conversion';
        var symbol=Symbol('other');parent[symbol]=29;
        if(Reflect.get(target,symbol)!==29||Reflect.get(target,low)!==17)throw 'symbol separation';
    "#);
}

#[test]
fn raw_key_reflect_set_keeps_target_receiver_flags_and_setter_semantics() {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, r#"
        var key='\ud800', parent={}, target=Object.create(parent), receiver={};
        Object.defineProperty(parent,key,{value:1,writable:true});
        Object.defineProperty(receiver,key,{value:2,writable:true});
        if(!Reflect.set(target,key,3,receiver)||receiver[key]!==3||target[key]!==1||
           Object.hasOwn(target,key))throw 'distinct receiver data';
        var d=Object.getOwnPropertyDescriptor(receiver,key);
        if(!d.writable||d.enumerable||d.configurable)throw 'receiver flags';
        var fresh={}, closed=Object.preventExtensions({});
        if(!Reflect.set(target,key,4,fresh)||fresh[key]!==4)throw 'new receiver data';
        d=Object.getOwnPropertyDescriptor(fresh,key);
        if(!d.writable||!d.enumerable||!d.configurable)throw 'new receiver flags';
        if(Reflect.set(target,key,5,closed)!==false||Object.hasOwn(closed,key)||
           Reflect.set(target,key,5,undefined)!==false||Reflect.set(target,key,5,3)!==false)
            throw 'invalid receivers';
        var frozen={};Object.defineProperty(frozen,key,{value:7});
        if(Reflect.set(frozen,key,7,receiver)!==false||receiver[key]!==3)throw 'frozen target';
        var setterKey='\udc00', calls=0, seenThis, seenValue, value={};
        Object.defineProperty(parent,setterKey,{set:function(v){
            'use strict';calls++;seenThis=this;seenValue=v;
        }});
        if(!Reflect.set(target,setterKey,value,null)||calls!==1||seenThis!==null||seenValue!==value||
           Object.hasOwn(target,setterKey))throw 'exact setter inputs';
    "#);
}

#[test]
fn raw_key_proxy_get_set_and_has_receive_original_units_and_live_handler() {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, r#"
        var high='\ud800', low='\udc00', target={}, base={}, handler=Object.create(base);
        var receiver={}, payload={}, lookups=0, gets=0, sets=0, observed;
        target[high]=11;target[low]=13;
        Object.defineProperty(base,'get',{get:function(){
            if(this!==handler)throw 'trap lookup receiver';lookups++;
            return function(t,k,r){
                'use strict';
                if(this!==handler||t!==target||r!==receiver||typeof k!=='string'||k.length!==1)
                    throw 'get inputs';
                gets++;observed=k;return k;
            };
        }});
        handler.set=function(t,k,v,r){
            'use strict';
            if(this!==handler||t!==target||k!==high||k.charCodeAt(0)!==55296||v!==payload||r!==receiver)
                throw 'set inputs';
            sets++;return true;
        };
        handler.has=function(t,k){
            'use strict';if(this!==handler||t!==target)throw 'has inputs';observed=k;return k===low;
        };
        var proxy=new Proxy(target,handler);
        if(Reflect.get(proxy,high,receiver)!==high||observed.charCodeAt(0)!==55296||
           Reflect.get(proxy,low,receiver)!==low||observed.charCodeAt(0)!==56320||lookups!==2||gets!==2)
            throw 'exact get keys';
        if(!Reflect.set(proxy,high,payload,receiver)||sets!==1||target[high]!==11||Object.hasOwn(receiver,high))
            throw 'truthy trap does not write target';
        if(!(low in proxy)||observed!==low||(high in proxy)||observed!==high)throw 'exact has keys';
    "#);
}

#[test]
fn raw_key_proxy_nullish_fallback_uses_the_distinct_receiver_target() {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, r#"
        var key='\ud800', setterKey='\udc00', target={}, other={}, handler={}, otherHandler={};
        target[key]=1;
        var proxy=new Proxy(target,handler), otherProxy=new Proxy(other,otherHandler);
        if(!Reflect.set(proxy,key,9,otherProxy)||target[key]!==1||other[key]!==9)
            throw 'forwarded receiver target';
        var d=Object.getOwnPropertyDescriptor(other,key);
        if(!d.writable||!d.enumerable||!d.configurable)throw 'forwarded new descriptor';
        Object.defineProperty(other,key,{enumerable:false,configurable:false});
        if(!Reflect.set(proxy,key,10,otherProxy)||other[key]!==10)throw 'forwarded update';
        d=Object.getOwnPropertyDescriptor(other,key);
        if(d.enumerable||d.configurable||!d.writable)throw 'forwarded preserved descriptor';
        var parent={}, seenThis, seenValue, payload={};
        Object.defineProperty(parent,setterKey,{
            get:function(){'use strict';return this.saved;},
            set:function(v){'use strict';seenThis=this;seenValue=v;this.saved=v;}
        });
        Object.setPrototypeOf(target,parent);
        if(!Reflect.set(proxy,setterKey,payload,otherProxy)||seenThis!==otherProxy||seenValue!==payload||
           other.saved!==payload||Reflect.get(proxy,setterKey,otherProxy)!==payload)
            throw 'forwarded accessor receiver';
        handler.set=null;
        if(!Reflect.set(proxy,key,12)||target[key]!==12||other[key]!==10)throw 'null trap forwarding';
    "#);
}

#[test]
fn raw_key_proxy_frozen_data_and_accessor_invariants_use_exact_descriptors() {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, r#"
        var high='\ud800', low='\udc00', setterOnly='s\ud800', getterOnly='g\ud800';
        var target={}, answer=0, handler={get:function(){return answer;},set:function(){return true;}};
        Object.defineProperty(target,high,{value:-0});
        Object.defineProperty(target,low,{value:NaN});
        Object.defineProperty(target,setterOnly,{set:function(){}});
        Object.defineProperty(target,getterOnly,{get:function(){return 1;}});
        var proxy=new Proxy(target,handler);
        function denied(f){try{f();}catch(e){if(e.name==='TypeError')return true;throw e;}return false;}
        if(!denied(function(){return proxy[high];}))throw 'signed-zero get invariant';
        answer=-0;if(1/proxy[high]!==-Infinity)throw 'same signed zero';
        answer=NaN;var result=proxy[low];if(result===result)throw 'same NaN';
        answer=1;if(!denied(function(){return proxy[setterOnly];}))throw 'missing getter invariant';
        answer=undefined;if(proxy[setterOnly]!==undefined)throw 'undefined getter result';
        if(!Reflect.set(proxy,high,-0)||!Reflect.set(proxy,low,NaN)||
           !Reflect.set(proxy,setterOnly,17))throw 'allowed frozen results';
        if(!denied(function(){Reflect.set(proxy,high,0);})||
           !denied(function(){Reflect.set(proxy,getterOnly,1);}))throw 'frozen set invariant';
        handler.set=function(){return false;};
        if(Reflect.set(proxy,high,0)!==false)throw 'false trap result';
    "#);
}

#[test]
fn raw_key_proxy_get_set_and_has_recheck_post_trap_descriptor_changes() {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, r#"
        var target={}, handler={}, proxy=new Proxy(target,handler);
        function denied(f){try{f();}catch(e){if(e.name==='TypeError')return true;throw e;}return false;}
        handler.get=function(t,k){Object.defineProperty(t,k,{value:19});return 18;};
        if(!denied(function(){return proxy['\ud800'];}))throw 'new frozen get property';
        handler.set=function(t,k){Object.defineProperty(t,k,{value:23});return true;};
        if(!denied(function(){Reflect.set(proxy,'\udc00',24);})||
           Reflect.set(proxy,'s\ud800',23)!==true)throw 'new frozen set property';
        handler.has=function(t,k){Object.defineProperty(t,k,{value:31});return false;};
        if(!denied(function(){return 'h\ud800' in proxy;}))throw 'new frozen has property';
        var extensible={}, hasHandler={has:function(t){Object.preventExtensions(t);return false;}};
        extensible['\ud800']=37;
        var fixed=new Proxy(extensible,hasHandler);
        if(!denied(function(){return '\ud800' in fixed;}))throw 'new nonextensible has target';
        delete handler.has;
        if(!('\ud800' in proxy)||!('\udc00' in proxy)||('missing\ud800' in proxy))throw 'has fallback';
    "#);
}

#[test]
fn raw_key_proxy_preserves_unsupported_operations_and_cleans_up_after_refusal() {
    let mut runtime = Runtime::for_test();
    execute(&mut runtime, r#"
        var raw='\ud800', target={}, calls=0;target[raw]=7;
        function unsupported(){calls++;throw 'unsupported trap invoked';}
        var handler={ownKeys:unsupported,deleteProperty:unsupported,defineProperty:unsupported,
                     getOwnPropertyDescriptor:unsupported};
        var proxy=new Proxy(target,handler);
    "#);
    for (source, operation) in [
        ("Object.keys(proxy)", "ownKeys"),
        ("Object.getOwnPropertyNames(proxy)", "ownKeys"),
        ("Reflect.ownKeys(proxy)", "ownKeys"),
        ("Object.hasOwn(proxy,raw)", "getOwnPropertyDescriptor"),
        ("Object.defineProperty(proxy,raw,{value:99})", "defineProperty"),
        ("delete proxy[raw]", "deleteProperty"),
    ] {
        runtime.begin_job();
        let error = runtime.execute(source, &mut NoHost).unwrap_err();
        assert!(error.contains(&format!("Proxy {operation}")), "{source}: {error}");
        assert!(runtime.transient_registration_is_empty(), "{source}");
    }
    runtime.begin_job();
    execute(&mut runtime, r#"
        if(calls!==0||target[raw]!==7||Object.getOwnPropertyNames(target).length!==1||
           typeof Reflect.ownKeys!=='function'||Reflect.ownKeys(target)[0]!==raw)
            throw 'unsupported path changed state or surface';
        delete handler.getOwnPropertyDescriptor;
        if(!Object.hasOwn(proxy,raw)||Object.getOwnPropertyDescriptor(proxy,raw).value!==7)
            throw 'supported descriptor fallback';
    "#);
}

struct RevokingHost {
    allowed: bool,
    calls: usize,
}
impl Host for RevokingHost {
    fn method(&self, handle: u32, name: &str) -> Option<&'static str> {
        (handle == 42 && name == "close").then_some("close")
    }
    fn native_access_owner(&self, handle: u32) -> Option<u32> {
        match handle {
            42 => Some(77),
            88 => Some(88),
            _ => None,
        }
    }
    fn window_context(&self, handle: u32) -> Option<WindowContext> {
        (handle == 88).then_some(WindowContext { document: 89, location: 90 })
    }
    fn validate_window_access(&self, _: Option<u32>, _: u32) -> Result<(), String> {
        if self.allowed { Ok(()) } else { Err("same-origin access refused".into()) }
    }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("raw key reached native get")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("raw key reached native set")
    }
    fn call(&mut self, handle: u32, name: &str, _: &[Value]) -> Result<Value, String> {
        assert_eq!((handle, name), (42, "close"));
        self.calls += 1;
        self.allowed = false;
        Ok(Value::Undefined)
    }
}

#[test]
fn raw_key_reflect_native_access_and_receiver_refusals_precede_host_effects() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("native", Value::Host(42)).unwrap();
    runtime.define_global("foreign", Value::Host(88)).unwrap();
    let mut host = RevokingHost { allowed: true, calls: 0 };
    runtime.execute(r#"
        var raw='\ud800', effects=0, callbacks=0, target={}, key={toString:function(){
            effects++;native.close();return raw;
        }};
        Object.defineProperty(target,raw,{
            get:function(){callbacks++;return this;},set:function(){callbacks++;}
        });
    "#, &mut host).unwrap();
    for (source, message) in [
        ("Reflect.get(native,raw,{})", "different receiver"),
        ("Reflect.set(native,raw,1)", "native target"),
        ("Reflect.set({},raw,1,native)", "native receiver"),
        ("Reflect.get(target,raw,foreign)", "foreign Window"),
        ("Reflect.set(target,raw,1,foreign)", "foreign Window"),
    ] {
        runtime.begin_job();
        let state = (runtime.host_objects.len(), runtime.native_expando_owners.len());
        let error = runtime.execute(source, &mut host).unwrap_err();
        assert!(error.contains(message), "{source}: {error}");
        assert_eq!((runtime.host_objects.len(), runtime.native_expando_owners.len()), state);
        assert!(runtime.transient_registration_is_empty());
    }
    assert_eq!(host.calls, 0);
    runtime.begin_job();
    let error = runtime.execute("Reflect.get(native,key)", &mut host).unwrap_err();
    assert!(error.contains("same-origin"), "{error}");
    assert_eq!(host.calls, 1);
    assert_eq!(runtime.binding_value(runtime.global_environment, "effects").unwrap().to_number(), 1.0);
    assert_eq!(runtime.binding_value(runtime.global_environment, "callbacks").unwrap().to_number(), 0.0);
    runtime.begin_job();
    let error = runtime.execute("Reflect.get(native,key)", &mut host).unwrap_err();
    assert!(error.contains("same-origin"), "{error}");
    assert_eq!(host.calls, 1, "inaccessible target must fail before key conversion");
    assert_eq!(runtime.binding_value(runtime.global_environment, "effects").unwrap().to_number(), 1.0);
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn raw_key_reflect_resource_failure_is_atomic_and_exotic_set_stays_refused() {
    let mut runtime = Runtime::for_test();
    runtime.define_global("native", Value::Host(42)).unwrap();
    execute(&mut runtime, r#"
        var raw='\ud800', target={}, receiver={keep:9}, array=[8], calls=0;
        Object.defineProperty(array,raw,{set:function(){calls++;}});
        if(typeof Reflect.set!=='function')throw 'Reflect initialization';
    "#);
    let limit = runtime.limits.heap.object_properties;
    runtime.limits.heap.object_properties = 1;
    let error = runtime.execute("Reflect.set(target,raw,4,receiver)", &mut NoHost).unwrap_err();
    assert!(error.contains("property limit"), "{error}");
    assert!(runtime.transient_registration_is_empty());
    runtime.limits.heap.object_properties = limit;
    for source in ["Reflect.set(array,raw,1)", "Reflect.set(target,raw,1,array)"] {
        runtime.begin_job();
        let error = runtime.execute(source, &mut NoHost).unwrap_err();
        assert!(error.contains("exotic"), "{source}: {error}");
        assert!(runtime.transient_registration_is_empty());
    }
    runtime.begin_job();
    execute(&mut runtime, r#"
        if(receiver.keep!==9||Object.hasOwn(receiver,raw)||Object.hasOwn(target,raw)||
           array.length!==1||array[0]!==8||calls!==0)throw 'refusal changed state';
        if(!Reflect.set(target,raw,11,receiver)||receiver[raw]!==11||Object.hasOwn(target,raw))
            throw 'recovery';
    "#);
}

#[test]
fn raw_key_proxy_argument_copy_and_reflect_setter_copy_obey_work_limits() {
    let mut runtime = Runtime::for_test();
    let units = vec![0xd800, 0x61, 0xdc00];
    runtime.begin_job();
    let before = runtime.copy_bytes;
    runtime.property_name_units(&units).unwrap();
    let name_work = before - runtime.copy_bytes;
    runtime.begin_job();
    runtime.copy_bytes = name_work + units.len() * 2 - 1;
    let error = runtime.proxy_key_value(&PropertyKey::StringUnits(units.clone())).unwrap_err();
    assert!(error.exception.is_none() && error.message.contains("copy work limit"), "{error:?}");
    runtime.begin_job();
    runtime.copy_bytes = name_work + units.len() * 2;
    assert!(matches!(runtime.proxy_key_value(&PropertyKey::StringUnits(units.clone())).unwrap(),
        Value::StringUnits(value) if value == units));
    assert_eq!(runtime.copy_bytes, 0);
    runtime.begin_job();
    execute(&mut runtime, r#"
        var raw='\ud800', target={}, calls=0;
        Object.defineProperty(target,raw,{set:function(){calls++;}});
    "#);
    let target = runtime.binding_value(runtime.global_environment, "target").unwrap();
    runtime.begin_job();
    runtime.copy_bytes = 128;
    let error = runtime.reflect_set(&[
        target, Value::StringUnits(vec![0xd800]), Value::Undefined, Value::String("r".repeat(8192)),
    ], &mut NoHost).unwrap_err();
    assert!(error.exception.is_none() && error.message.contains("copy work limit"), "{error:?}");
    assert_eq!(runtime.binding_value(runtime.global_environment, "calls").unwrap().to_number(), 0.0);
    assert!(runtime.transient_registration_is_empty());
    runtime.begin_job();
    execute(&mut runtime, "if(!Reflect.set(target,raw,5,'short')||calls!==1)throw 'setter recovery';");
}
