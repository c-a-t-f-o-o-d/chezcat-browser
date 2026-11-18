use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        panic!("unexpected host read")
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        panic!("unexpected host write")
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        panic!("unexpected host call")
    }
}

#[test]
fn proxy_dispatch_constructor_identity_metadata_and_required_new() {
    Runtime::for_test().execute(r#"
        var P=Proxy, first={}, second={}, count=0;
        if(P.name!=='Proxy'||P.length!==2||Object.hasOwn(P,'prototype'))throw 'constructor metadata';
        try{P(first,second);}catch(e){if(e.name==='TypeError')count++;}
        try{new P(null,second);}catch(e){if(e.name==='TypeError')count++;}
        try{new P(first);}catch(e){if(e.name==='TypeError')count++;}
        if(count!==3)throw 'constructor errors';
        var p=new P(first,second), other=new P(first,second), cache=new WeakMap();
        cache.set(first,p);cache.set(p,other);
        if(other===p||p===first||cache.get(first)!==p||cache.get(p)!==other)throw 'identity';
        Proxy=function(){throw 'replacement';};
        var retained=new P(first,second);
        if(typeof retained!=='object'||retained===p)throw 'saved constructor';
    "#, &mut NoHost).unwrap();
}

#[test]
fn proxy_dispatch_methods_and_prototype_walk_keep_actual_receiver_and_key() {
    Runtime::for_test()
        .execute(
            r#"
        var key=Symbol('method'), calls='', receiver, raw={}, h={};
        function method(){return this;}
        raw.method=method;raw[key]=method;
        var p=new Proxy(raw,h), child=Object.create(p);
        h.get=function(t,k,r){
            if(t!==raw||r!==receiver||this!==h)throw 'get identity';
            calls+=typeof k==='symbol'?'s':'n';return t[k];
        };
        h.set=function(t,k,v,r){
            if(t!==raw||r!==receiver||this!==h)throw 'set identity';
            calls+=typeof k==='symbol'?'S':'N';return true;
        };
        receiver=p;
        if(p.method()!==p||p[key]()!==p||p?.method()!==p)throw 'method receiver';
        receiver=child;
        if(child.method()!==child||Reflect.get(child,key,child)!==method)throw 'inherited read';
        child.x=3;child[Symbol('field')]=9;
        if(calls!=='nsnnsNS'||Object.hasOwn(raw,'x')||Object.hasOwn(child,'x'))throw calls;
    "#,
            &mut NoHost,
        )
        .unwrap();
}
