//! Original object-literal accessor installation. Functions retain the existing
//! callable arena, closures and execution budgets; property definitions reuse
//! the ordinary descriptor storage. No accessor runs during construction.
//!
//! ECMA-262 MethodDefinitionEvaluation and SetFunctionName:
//! https://tc39.es/ecma262/multipage/ecmascript-language-functions-and-classes.html#sec-runtime-semantics-methoddefinitionevaluation
//! https://tc39.es/ecma262/multipage/ordinary-and-exotic-objects-behaviours.html#sec-setfunctionname

use super::{Accessors, ExprId, Fault, Ordinary, PropertyKey, Runtime, Value, MAX_STRING_BYTES};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Data,
    Method,
    Getter,
    Setter,
}

#[derive(Clone)]
pub(super) struct Member {
    pub(super) key: ExprId,
    pub(super) value: ExprId,
    pub(super) kind: Kind,
}

impl Runtime {
    pub(super) fn define_literal_member(
        &mut self,
        object: usize,
        key: PropertyKey,
        value: Value,
        kind: Kind,
    ) -> Result<(), Fault> {
        if kind != Kind::Data {
            let Value::Function(function) = value else {
                return Err(Fault::new("Invalid object method definition."));
            };
            self.literal_function_name(function, &key, kind)?;
            if matches!(kind, Kind::Getter | Kind::Setter) {
                // Repeated accessor definitions replace only their own half.
                // A preceding data property starts a fresh accessor pair.
                let previous = match &key {
                    PropertyKey::String(name) => self.objects[object].properties.get(name),
                    PropertyKey::Symbol(symbol) => {
                        self.objects[object].symbol_properties.get(symbol)
                    }
                };
                let mut accessor =
                    previous
                        .and_then(|property| property.accessor)
                        .unwrap_or(Accessors {
                            get: None,
                            set: None,
                        });
                if kind == Kind::Getter {
                    accessor.get = Some(function);
                } else {
                    accessor.set = Some(function);
                }
                return match key {
                    PropertyKey::String(name) => {
                        self.store_accessor_property(object, name, accessor, (true, true))
                    }
                    PropertyKey::Symbol(symbol) => {
                        self.store_symbol_accessor_property(object, symbol, accessor, (true, true))
                    }
                };
            }
        }
        // Literal definitions create data properties, even after an accessor
        // or over an inherited setter. Ordinary assignment would be incorrect.
        match key {
            PropertyKey::String(name) => {
                self.store_property(object, name, value, Some((true, true, true)))
            }
            PropertyKey::Symbol(symbol) => {
                self.store_symbol_property(object, symbol, value, Some((true, true, true)))
            }
        }
    }

    fn literal_function_name(
        &mut self,
        function: usize,
        key: &PropertyKey,
        kind: Kind,
    ) -> Result<(), Fault> {
        let prefix = match kind {
            Kind::Getter => "get ",
            Kind::Setter => "set ",
            Kind::Method => "",
            Kind::Data => unreachable!(),
        };
        let key_name = match key {
            PropertyKey::String(name) => {
                self.work(name.len())?;
                name.clone()
            }
            PropertyKey::Symbol(symbol) => match self.symbol_description(Value::Symbol(*symbol))? {
                Value::Undefined => String::new(),
                Value::String(description) => {
                    if description.len().saturating_add(prefix.len() + 2) > MAX_STRING_BYTES {
                        return Err(Fault::new("String limit reached (128 KiB)."));
                    }
                    self.work(description.len() + 2)?;
                    format!("[{description}]")
                }
                _ => unreachable!(),
            },
        };
        if prefix.len().saturating_add(key_name.len()) > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        self.work(prefix.len() + key_name.len())?;
        let properties = self.ordinary_id(Ordinary::Function(function))?;
        self.store_property(
            properties,
            "name".into(),
            Value::String(format!("{prefix}{key_name}")),
            Some((false, false, true)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::Host;
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn allow_eval(&mut self) -> Result<bool, String> {
            Ok(true)
        }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No host.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No host.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No host.".into())
        }
    }

    fn run(source: &str) {
        Runtime::new().execute(source, &mut NoHost).unwrap();
    }

    #[test]
    fn literal_accessors_merge_override_and_preserve_descriptors() {
        run(r#"
            var calls=0, o={get x(){calls++;return this.n;},set x(v){this.n=v;},n:2};
            var d=Object.getOwnPropertyDescriptor(o,'x');
            if(calls!==0 || !d.enumerable || !d.configurable || 'value' in d || 'writable' in d)throw 'accessor descriptor';
            if(d.get.length!==0 || d.set.length!==1 || d.get.name!=='get x' || d.set.name!=='set x')throw 'metadata';
            if(Object.hasOwn(d.get,'prototype') || Object.hasOwn(d.set,'prototype'))throw 'constructor prototype';
            var constructed=0;try{new d.get();}catch(e){if(e.name==='TypeError')constructed++;}
            try{new d.set();}catch(e){if(e.name==='TypeError')constructed++;}
            if(constructed!==2)throw 'constructible accessor';
            var nd=Object.getOwnPropertyDescriptor(d.get,'name');
            var ld=Object.getOwnPropertyDescriptor(d.set,'length');
            if(nd.enumerable||nd.writable||!nd.configurable||ld.enumerable||ld.writable||!ld.configurable)throw 'metadata descriptors';
            o.x=7;if(o.x!==7 || calls!==1)throw 'get/set pair';
            o={get x(){throw 'obsolete getter';},set x(v){this.n=v;},get x(){return this.n+1;}};
            o.x=8;if(o.x!==9)throw 'getter replaces one half';
            o={set x(v){throw 'obsolete setter';},get x(){return this.n;},set x(v){this.n=v+2;}};
            o.x=8;if(o.x!==10)throw 'setter replaces one half';
            o={get x(){throw 'getter ran';},x:5};d=Object.getOwnPropertyDescriptor(o,'x');
            if(o.x!==5 || !d.writable || !d.enumerable || !d.configurable || 'get' in d)throw 'accessor to data';
            o={x:3,get x(){return 9;}};d=Object.getOwnPropertyDescriptor(o,'x');
            if(o.x!==9 || d.set!==undefined || 'value' in d)throw 'data to accessor';
            o={get x(){return 1;},x:3,set x(v){this.n=v;}};d=Object.getOwnPropertyDescriptor(o,'x');
            if(d.get!==undefined || o.x!==undefined)throw 'data clears old accessor';
            o.x=4;if(o.n!==4)throw 'new setter';
        "#);
    }

    #[test]
    fn literal_accessors_names_keys_receivers_and_closures() {
        run(r#"
            var get=2,set=3,async=4,o={get,set,async,getter(){return this.get;},get(){return 5;},set(v){return v+1;},async(){return 7;}};
            if(o.get()!==5 || o.set(8)!==9 || o.async()!==7 || o.get.name!=='get' || o.set.name!=='set')throw 'ordinary named methods';
            var sym=Symbol('field'),empty=Symbol(''),anonymous=Symbol();
            o={get 'a b'(){return 1;},set 4(v){this.n=v;},get true(){return 2;},get [sym](){return 3;},set [sym](v){this.n=v;},get [empty](){return 4;},get [anonymous](){return 5;}};
            if(o['a b']!==1 || o.true!==2 || o[sym]!==3)throw 'property names';
            o[4]=7;if(o.n!==7)throw 'numeric setter';
            var d=Object.getOwnPropertyDescriptor(o,sym);
            if(d.get.name!=='get [field]' || d.set.name!=='set [field]')throw 'symbol function names';
            if(Object.getOwnPropertyDescriptor(o,empty).get.name!=='get []' || Object.getOwnPropertyDescriptor(o,anonymous).get.name!=='get ')throw 'empty symbol function names';
            o[sym]=9;if(o.n!==9)throw 'symbol setter';
            var child=Object.create(o);child[4]=11;if(child.n!==11 || o.n!==9)throw 'inherited setter receiver';
            function make(n){return {get value(){return n;},set value(v){n=v;}};}
            var a=make(2),b=make(3);a.value=8;if(a.value!==8 || b.value!==3)throw 'closures';
            var getter=Object.getOwnPropertyDescriptor({get value(){return this.n;}},'value').get;
            if(getter.call({n:12})!==12)throw 'borrowed getter';
            if(({get pattern(){return /a+/.test('aaa');}}).pattern!==true)throw 'regexp lexical goal';
        "#);
    }

    #[test]
    fn literal_accessor_computed_keys_follow_order_and_own_definitions() {
        run(r#"
            var order='',key={toString(){order+='k';return 'value';}},sym=Symbol('key');
            var o={get [(order+='g',key)](){order+='G';return this.n;},set [(order+='s',key)](v){order+='S';this.n=v;},n:(order+='n',1)};
            if(order!=='gkskn')throw 'key evaluation order';
            o.value=8;if(o.value!==8 || order!=='gksknSG')throw 'accessor call order';
            var thrown={},seen=false;
            try{o={get [{toString(){throw thrown;}}](){return 1;},later:(order+='bad',2)};}catch(e){seen=e===thrown;}
            if(!seen || order!=='gksknSG')throw 'key abrupt completion';
            Object.defineProperty(Object.prototype,'literalTrap',{set:function(){throw 'inherited setter';},configurable:true});
            Object.defineProperty(Object.prototype,sym,{set:function(){throw 'inherited symbol setter';},configurable:true});
            o={literalTrap:1,[sym]:2};delete Object.prototype.literalTrap;delete Object.prototype[sym];
            if(o.literalTrap!==1 || o[sym]!==2 || !Object.hasOwn(o,'literalTrap'))throw 'own data properties';
            o={get [sym](){throw 'obsolete symbol getter';},[sym]:4};
            if(o[sym]!==4 || !Object.getOwnPropertyDescriptor(o,sym).writable)throw 'symbol data override';
            o={get b(){return 1;},a:2,set b(v){},get 3(){return 3;},get 1(){return 1;}};
            if(Object.keys(o).join(',')!=='1,3,b,a')throw 'property ordering';
            var count=0;Object.getOwnPropertyDescriptors({get x(){count++;return 1;}});
            if(count!==0)throw 'descriptor inspection invoked getter';
        "#);
    }

    #[test]
    fn literal_accessors_propagate_throws_and_observe_strict_calls() {
        run(r#"
            var thrown={},seen=0,o={get x(){throw thrown;},set x(v){throw v;}};
            try{o.x;}catch(e){if(e===thrown)seen++;}
            try{o.x=thrown;}catch(e){if(e===thrown)seen++;}
            if(seen!==2)throw 'exception identity';
            o={get x(){return 1;}};o.x=3;if(o.x!==1)throw 'sloppy missing setter';
            seen=false;try{(function(){'use strict';o.x=3;})();}catch(e){seen=e.name==='TypeError';}
            if(!seen)throw 'strict missing setter';
            var globalReceiver=(function(){return this;})();
            var strict=Object.getOwnPropertyDescriptor({get x(){'use strict';return this;}},'x').get;
            var sloppy=Object.getOwnPropertyDescriptor({get x(){return this;}},'x').get;
            if(strict.call(null)!==null || strict.call(7)!==7 || sloppy.call(null)!==globalReceiver)throw 'this semantics';
            function outer(){'use strict';return Object.getOwnPropertyDescriptor({get x(){return this;}},'x').get;}
            if(outer().call(undefined)!==undefined)throw 'inherited strict mode';
            var x='outer',getter=Object.getOwnPropertyDescriptor({get x(){return x;}},'x').get;
            if(getter()!=='outer')throw 'name must not create lexical binding';
        "#);
    }

    #[test]
    fn literal_accessor_syntax_errors_are_atomic() {
        for source in [
            "({get x(a){return a;}});",
            "({set x(){}});",
            "({set x(a,b){}});",
            "({set x(a,){}});",
            "({set x(...a){}});",
            "({set x([a]){}});",
            "({get x:1});",
            "({get __proto__(){return 1;}});",
            "({get x(){super.x;}});",
            "({async x(){}});",
            "({async [x](){}});",
            "({*x(){}});",
            "({set x(arguments){'use strict';}});",
            "(function(){'use strict';return {set x(eval){}};})();",
            "({set x(v){let v;}});",
        ] {
            let mut runtime = Runtime::new();
            runtime.execute("var untouched=0;", &mut NoHost).unwrap();
            assert!(
                runtime
                    .execute(&format!("untouched=1;{source}"), &mut NoHost)
                    .is_err(),
                "{source}"
            );
            assert_eq!(
                runtime
                    .execute("untouched;", &mut NoHost)
                    .unwrap()
                    .to_text(),
                "0",
                "{source}"
            );
        }
        run(r#"
            var untouched=0,seen=false;
            try{eval('untouched=1;({get x(v){}});');}catch(e){seen=e.name==='SyntaxError';}
            if(!seen || untouched!==0)throw 'catchable arity syntax error';
        "#);
    }

    #[test]
    fn literal_accessor_allocations_and_execution_remain_bounded() {
        let mut runtime = Runtime::new();
        let error = runtime
            .execute(
                "for(var i=0;i<1100;i++){var o={get x(){return i;}};}",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("Function limit"), "{error}");
        let mut runtime = Runtime::new();
        let error = runtime
            .execute("var o={get x(){return this.x;}};o.x;", &mut NoHost)
            .unwrap_err();
        assert!(error.contains("call limit"), "{error}");
        let mut runtime = Runtime::new();
        let symbol = runtime
            .symbol_builtin(
                super::super::symbols::Op::Constructor,
                &[Value::String("x".repeat(MAX_STRING_BYTES))],
                Value::Undefined,
                &mut NoHost,
            )
            .unwrap();
        runtime.define_global("largeKey", symbol).unwrap();
        let error = runtime
            .execute("({get [largeKey](){return 1;}});", &mut NoHost)
            .unwrap_err();
        assert!(error.contains("String limit"), "{error}");
    }
}
