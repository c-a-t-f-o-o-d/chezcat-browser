use crate::javascript::{Host, Runtime, Value};

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

#[test]
fn proxy_own_descriptor_predicates_preserve_flags_symbols_and_accessor_identity() {
    Runtime::for_test().execute(r#"
        var key=Symbol('own'), proto={inherited:1}, raw=Object.create(proto), p=new Proxy(raw,{});
        var reads=0, value={}, get=function(){reads++;return value;}, set=function(){throw 'setter';};
        Object.defineProperty(raw,'hidden',{get:get,set:set,enumerable:false,configurable:true});
        Object.defineProperty(raw,key,{value:value,enumerable:true,writable:false,configurable:false});
        var has=Object.prototype.hasOwnProperty, enumerable=Object.prototype.propertyIsEnumerable;
        if(!has.call(p,'hidden')||!Object.hasOwn(p,key)||has.call(p,'inherited')||
           Object.hasOwn(p,'absent')||enumerable.call(p,'hidden')||!enumerable.call(p,key))throw 'predicate';
        var a=Object.getOwnPropertyDescriptor(p,'hidden'), b=Object.getOwnPropertyDescriptor(p,key);
        if(a.get!==get||a.set!==set||a.enumerable||!a.configurable||Object.hasOwn(a,'value'))throw 'accessor';
        if(b.value!==value||b.writable||!b.enumerable||b.configurable||Object.getOwnPropertyDescriptor(p,'inherited')!==undefined)throw 'data';
        b.value=99;
        if(raw[key]!==value||reads!==0)throw 'descriptor alias or getter';
    "#, &mut NoHost).unwrap();
}

#[test]
fn proxy_own_descriptor_converts_key_then_resolves_live_inherited_handler_method() {
    Runtime::for_test().execute(r#"
        var raw={before:1}, parent={}, handler=Object.create(parent), p=new Proxy(raw,handler);
        var order='', count=0, key={toString:function(){order+='k';return 'before';}};
        Object.defineProperty(parent,'getOwnPropertyDescriptor',{configurable:true,get:function(){
            if(this!==handler)throw 'trap getter receiver';order+='t';count++;
            if(count===1)delete raw.before; else raw.before=42;
            return count===1?null:undefined;
        }});
        if(Object.hasOwn(p,key)||order!=='kt')throw 'read before deletion';
        if(Object.getOwnPropertyDescriptor(p,key).value!==42||order!=='ktkt')throw 'stale descriptor';
        delete parent.getOwnPropertyDescriptor;
        var marker={},caught=false;
        Object.defineProperty(parent,'getOwnPropertyDescriptor',{configurable:true,get:function(){throw marker;}});
        try{Object.prototype.propertyIsEnumerable.call(p,key);}catch(e){caught=e===marker;}
        if(!caught||order!=='ktktk')throw 'abrupt trap lookup';
        delete parent.getOwnPropertyDescriptor;parent.getOwnPropertyDescriptor=2;caught=false;
        try{Object.hasOwn(p,'before');}catch(e){caught=e.name==='TypeError';}
        if(!caught||raw.before!==42)throw 'invalid method';
    "#, &mut NoHost).unwrap();
}

#[test]
fn proxy_own_descriptor_boolean_queries_work_at_the_object_limit() {
    let mut runtime = Runtime::for_test();
    runtime
        .execute("var raw={x:1}, p=new Proxy(raw,{});", &mut NoHost)
        .unwrap();
    let proxy = runtime
        .binding_value(runtime.global_environment, "p")
        .unwrap();
    let key = Value::String("x".into());
    let ceiling = runtime.limits.heap.objects;
    let count = runtime.objects.len();
    runtime.limits.heap.objects = count;
    runtime.begin_job();
    for method in ["hasOwnProperty", "propertyIsEnumerable"] {
        let answer = runtime
            .object_method(
                method,
                proxy.clone(),
                std::slice::from_ref(&key),
                &mut NoHost,
            )
            .unwrap();
        assert!(matches!(answer, Value::Bool(true)));
        assert_eq!(runtime.objects.len(), count);
    }
    let error = runtime
        .object_static(
            "getOwnPropertyDescriptor",
            &[proxy.clone(), key.clone()],
            &mut NoHost,
        )
        .unwrap_err();
    assert!(
        error.exception.is_none() && error.message.contains("Object identity limit"),
        "{error:?}"
    );
    assert!(runtime.transient_registration_is_empty());
    assert_eq!(runtime.objects.len(), count);
    runtime.limits.heap.objects = ceiling;
    runtime.begin_job();
    let result = runtime
        .object_static("getOwnPropertyDescriptor", &[proxy, key], &mut NoHost)
        .unwrap();
    assert!(matches!(result, Value::Object(_)));
}
