//! Internal methods shared by the supported ordinary-object Proxy operations.
//! https://tc39.es/ecma262/#sec-proxy-object-internal-methods-and-internal-slots
use super::{Fault, Host, Ordinary, PropertyKey, Runtime, Value};

impl Runtime {
    // An unsupported internal method must stop before any ordinary algorithm
    // can enumerate, return a descriptor from, or change the wrapper's storage.
    pub(super) fn reject_unsupported_proxy_operation(
        &self,
        id: usize,
        operation: &str,
    ) -> Result<(), Fault> {
        if self.object(id)?.proxy.is_some() {
            return Err(Fault::new(format!("Proxy {operation} is not implemented.")));
        }
        Ok(())
    }

    pub(super) fn prototype_with_host(
        &mut self,
        object: Ordinary,
        keep_alive: &Value,
        host: &mut dyn Host,
    ) -> Result<Option<Ordinary>, Fault> {
        let id = self.ordinary_id(object)?;
        if self.objects[id].proxy.is_none() {
            return Ok(self.objects[id].prototype);
        }
        self.with_proxy_operation(id, keep_alive, None, |runtime, target, handler| {
            let Some(trap) = runtime.proxy_method(handler, "getPrototypeOf", host)? else {
                return runtime.ordinary_prototype(Ordinary::Object(target));
            };
            runtime.push_transient_value(&trap)?;
            let result =
                runtime.call_value(trap, &[Value::Object(target)], Value::Object(handler), host)?;
            runtime.push_transient_value(&result)?;
            runtime.step()?;
            let prototype = match result {
                Value::Null => None,
                Value::Object(id) => Some(Ordinary::Object(id)),
                Value::Function(id) => Some(Ordinary::Function(id)),
                Value::Host(_) => {
                    return Err(Fault::new(
                        "Native host results of Proxy getPrototypeOf are not implemented.",
                    ))
                }
                _ => {
                    return Err(Fault::exception(
                        "TypeError",
                        "Proxy getPrototypeOf must return an object or null.",
                    ))
                }
            };
            let target = runtime.object(target)?;
            if !target.extensible && target.prototype != prototype {
                return Err(Fault::exception(
                    "TypeError",
                    "Proxy getPrototypeOf disagrees with a nonextensible target.",
                ));
            }
            Ok(prototype)
        })
    }

    pub(super) fn has_ordinary_key(
        &mut self,
        object: Ordinary,
        key: &PropertyKey,
        host: &mut dyn Host,
    ) -> Result<bool, Fault> {
        let owner = match key {
            PropertyKey::String(name) => self.property_dispatch_owner(object, name)?,
            PropertyKey::Symbol(symbol) => self.symbol_dispatch_owner(object, *symbol)?,
        };
        let Some(owner) = owner else { return Ok(false) };
        if self.objects[owner].proxy.is_none() {
            return Ok(true);
        }
        self.with_proxy_operation(
            owner,
            &Value::Undefined,
            None,
            |runtime, target, handler| {
                let Some(trap) = runtime.proxy_method(handler, "has", host)? else {
                    return runtime.has_ordinary_key(Ordinary::Object(target), key, host);
                };
                runtime.push_transient_value(&trap)?;
                let args = [Value::Object(target), runtime.proxy_key_value(key)?];
                let present = runtime
                    .call_value(trap, &args, Value::Object(handler), host)?
                    .truthy();
                runtime.step()?;
                if !present {
                    if let Some(own) = runtime.stored_descriptor_property(target, key) {
                        if !own.configurable || !runtime.objects[target].extensible {
                            return Err(Fault::exception(
                                "TypeError",
                                "Proxy has cannot hide this own target property.",
                            ));
                        }
                    }
                }
                Ok(present)
            },
        )
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn proxy_operations_prototype_trap_is_live_and_respects_newly_fixed_target() {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
            function A(){} function B(){}
            var base=A.prototype, alt=B.prototype, target=Object.create(base);
            var log='', parent={}, h=Object.create(parent), p=new Proxy(target,h);
            if(Object.getPrototypeOf(p)!==base || p.__proto__!==base || !(p instanceof A))throw 'forward';
            Object.defineProperty(parent,'getPrototypeOf',{configurable:true,get:function(){
                if(this!==h)throw 'getter receiver';log+='g';
                return function(t){if(this!==h||t!==target)throw 'trap args';log+='t';return alt;};
            }});
            if(Object.getPrototypeOf(p)!==alt || !(p instanceof B) ||
                !Object.prototype.isPrototypeOf.call(alt,p) || log!=='gtgtgt')throw 'live lineage';
            var child={};Object.setPrototypeOf(child,p);
            if(Object.getPrototypeOf(child)!==p || log!=='gtgtgt' || !(child instanceof B))throw 'indirect lineage';
            delete parent.getPrototypeOf;
            var marker={}, caught=0;
            h.getPrototypeOf=function(){throw marker;};
            try{Object.getPrototypeOf(p);}catch(e){if(e===marker)caught++;}
            h.getPrototypeOf=1;
            try{Object.getPrototypeOf(p);}catch(e){if(e.name==='TypeError')caught++;}
            h.getPrototypeOf=function(){return 1;};
            try{Object.getPrototypeOf(p);}catch(e){if(e.name==='TypeError')caught++;}
            h.getPrototypeOf=function(t){Object.preventExtensions(t);return null;};
            try{Object.getPrototypeOf(p);}catch(e){if(e.name==='TypeError')caught++;}
            h.getPrototypeOf=function(){return base;};
            if(caught!==4 || Object.getPrototypeOf(p)!==base)throw 'proxy prototype invariant';
            h.getPrototypeOf=null;
            if(Object.getPrototypeOf(p)!==base)throw 'null forwarding';
        "#, &mut NoHost).unwrap();
        assert!(runtime.transient_registration_is_empty());
    }

    #[test]
    fn proxy_operations_has_uses_real_keys_receivers_and_post_trap_invariants() {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
            var k=Symbol('k'), t=Object.create({inherited:1}), hp={}, h=Object.create(hp);
            t[k]=4;t.x=1;var p=new Proxy(t,h), child=Object.create(p), log='';
            if(!(k in p)||!('inherited' in child)||('missing' in p))throw 'fallback';
            var observed;
            Object.defineProperty(hp,'has',{configurable:true,get:function(){
                if(this!==h)throw 'getter this';log+='g';return function(target,key){
                    if(this!==h||target!==t)throw 'trap args';log+='t';observed=key;
                    return key===k ? {} : 0;
                };
            }});
            if(!(k in child)||observed!==k||('x' in p)||observed!=='x'||log!=='gtgt')throw 'has trap';
            delete hp.has;
            var marker={},caught=0;
            h.has=function(){throw marker;};try{'x' in p;}catch(e){if(e===marker)caught++;}
            h.has=3;try{'x' in p;}catch(e){if(e.name==='TypeError')caught++;}
            h.has=function(target,key){Object.defineProperty(target,key,{configurable:false});return false;};
            try{k in p;}catch(e){if(e.name==='TypeError')caught++;}
            h.has=function(target){Object.preventExtensions(target);return false;};
            try{'x' in child;}catch(e){if(e.name==='TypeError')caught++;}
            if(('inherited' in p)||caught!==4)throw 'has invariant';
            h.has=null;if(!(k in p))throw 'null forwarding';
        "#, &mut NoHost).unwrap();
        assert!(runtime.transient_registration_is_empty());
    }

    #[test]
    fn proxy_operations_array_join_checks_inherited_synthetic_indices() {
        Runtime::for_test().execute(r#"
            var a=Array(2), calls='';
            var p=new Proxy({}, {get:function(t,k,r){
                if(r!==a)throw 'wrong receiver';
                calls+=k;return k==='0'?'left':'right';
            }});
            Object.setPrototypeOf(a,p);
            if(Array.prototype.join.call(a,'|')!=='left|right'||calls!=='01')throw 'inherited proxy indices';
        "#, &mut NoHost).unwrap();
    }

    #[test]
    fn proxy_operations_unsupported_paths_never_observe_or_mutate_wrapper_storage() {
        let mut runtime = Runtime::for_test();
        runtime
            .execute(
                "var k=Symbol(), t={x:1,0:7,length:1}, h={}, p=new Proxy(t,h);t[k]=3;",
                &mut NoHost,
            )
            .unwrap();
        for (expression, operation) in [
            ("Object.keys(p)", "ownKeys"),
            ("Object.getOwnPropertyNames(p)", "ownKeys"),
            ("Object.getOwnPropertySymbols(p)", "ownKeys"),
            ("Object.getOwnPropertyDescriptors(p)", "ownKeys"),
            ("Object.values(p)", "ownKeys"),
            ("Object.entries(p)", "ownKeys"),
            ("Object.assign({},p)", "ownKeys"),
            ("var copy={...p}", "ownKeys"),
            ("for(var key in p){}", "ownKeys"),
            ("JSON.stringify(p)", "ownKeys"),
            ("Object.hasOwn(p,'x')", "getOwnPropertyDescriptor"),
            (
                "Object.prototype.propertyIsEnumerable.call(p,k)",
                "getOwnPropertyDescriptor",
            ),
            (
                "Object.getOwnPropertyDescriptor(p,'x')",
                "getOwnPropertyDescriptor",
            ),
            (
                "Object.getOwnPropertyDescriptor(p,k)",
                "getOwnPropertyDescriptor",
            ),
            ("Object.defineProperty(p,'x',{value:5})", "defineProperty"),
            (
                "Object.defineProperties(p,{newKey:{value:1}})",
                "defineProperty",
            ),
            ("Object.defineProperty(p,k,{value:5})", "defineProperty"),
            ("Object.preventExtensions(p)", "preventExtensions"),
            ("Object.freeze(p)", "freeze"),
            ("Object.seal(p)", "seal"),
            ("Object.isSealed(p)", "isSealed"),
            ("Object.isFrozen(p)", "isFrozen"),
            ("Object.isExtensible(p)", "isExtensible"),
            ("Object.setPrototypeOf(p,null)", "setPrototypeOf"),
            ("delete p.x", "deleteProperty"),
            ("delete p[k]", "deleteProperty"),
            ("Array.prototype.pop.call(p)", "deleteProperty"),
        ] {
            runtime.begin_job();
            let error = runtime.execute(expression, &mut NoHost).unwrap_err();
            assert!(
                error.contains(&format!("Proxy {operation}")),
                "{expression}: {error}"
            );
            assert!(runtime.transient_registration_is_empty(), "{expression}");
        }
        runtime.begin_job();
        runtime.execute(r#"
            if(t.x!==1||t[k]!==3||t[0]!==7||t.length!==1||!Object.isExtensible(t)||
                Object.getPrototypeOf(t)!==Object.prototype||Object.hasOwn(t,'newKey'))throw 'target changed';
            if(p.valueOf()!==p||Object.is(p,t)||Array.isArray(p))throw 'identity';
            if(JSON.stringify(p,['x'])!=='{"x":1}')throw 'listed get';
        "#, &mut NoHost).unwrap();
        let id = runtime
            .objects
            .iter()
            .position(|object| object.proxy.is_some())
            .unwrap();
        let wrapper = &runtime.objects[id];
        assert!(wrapper.properties.is_empty() && wrapper.symbol_properties.is_empty());
        assert!(wrapper.extensible && wrapper.prototype.is_none());
    }
}
