//! Original bounded Function.prototype.bind support. Arena links retain callable
//! identity; invocation and construction flatten binding chains iteratively.
//! https://tc39.es/ecma262/#sec-function.prototype.bind
//! https://tc39.es/ecma262/#sec-bound-function-exotic-objects

use super::{
    Fault, Function, Host, Ordinary, Runtime, Value, MAX_ARGUMENTS, MAX_FUNCTIONS,
    MAX_STRING_BYTES,
};
#[cfg(test)]
use super::MAX_RETAINED_VALUES;

pub(super) struct Bound {
    pub(super) target: usize,
    receiver: Value,
    arguments: Vec<Value>,
}

impl Bound {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        add(Edge::Function(self.target));
        Edge::value(&self.receiver, add);
        for value in &self.arguments { Edge::value(value, add); }
    }
}

impl Runtime {
    pub(super) fn bind_function(
        &mut self,
        target: usize,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if !self.function_is_occupied(target) {
            return Err(Fault::new("Invalid function handle."));
        }
        if self.function_arena.occupied >= MAX_FUNCTIONS {
            return Err(self.function_limit_fault("bound"));
        }
        let receiver = args.first().unwrap_or(&Value::Undefined);
        let arguments = args.get(1..).unwrap_or(&[]);
        if arguments.len() > MAX_ARGUMENTS {
            return Err(Fault::new("Call argument limit reached (256)."));
        }
        // Charge both captured strings and the argument slots themselves. Bound
        // arguments cannot become an unaccounted arena of primitive values.
        let retained = std::mem::size_of::<Value>()
            .saturating_mul(arguments.len() + 1)
            .saturating_add(receiver.bytes())
            .saturating_add(arguments.iter().map(Value::bytes).sum::<usize>());
        if self.value_bytes.saturating_add(retained) > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        if receiver.bytes() > MAX_STRING_BYTES
            || arguments
                .iter()
                .any(|value| value.bytes() > MAX_STRING_BYTES)
        {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        self.work(retained)?;
        let receiver = receiver.clone();
        let arguments = arguments.to_vec();
        let prototype = self.ordinary_prototype(Ordinary::Function(target))?;

        // Only an own length participates. Read length before name, preserving
        // accessor side effects and abrupt completions without coercing either.
        let owner = self.ordinary_id(Ordinary::Function(target))?;
        let length = if self.objects[owner].properties.contains_key("length") {
            match self.get_ordinary_property_with_host(
                Ordinary::Function(target),
                "length",
                host,
            )? {
                Value::Number(value) if value == f64::INFINITY => f64::INFINITY,
                Value::Number(value) if value.is_finite() => {
                    (value.trunc() - arguments.len() as f64).max(0.0)
                }
                _ => 0.0,
            }
        } else {
            0.0
        };
        let name =
            self.get_ordinary_property_with_host(Ordinary::Function(target), "name", host)?;
        let mut bound_name = String::from("bound ");
        if let Value::String(name) = name {
            self.append_string(&mut bound_name, &name)?;
        }
        // User callbacks above may allocate functions or retain values.
        if self.function_arena.occupied >= MAX_FUNCTIONS {
            return Err(self.function_limit_fault("bound"));
        }
        if self.value_bytes.saturating_add(retained) > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        let allocation = self.insert_function_slot(Function::Bound(Bound {
            target,
            receiver,
            arguments,
        }))?;
        let index = allocation.id;
        self.value_bytes += retained;
        self.initialize_function_properties(allocation, &bound_name, 0, false)?;
        let properties = self.function_properties[index];
        self.objects[properties].prototype = prototype;
        self.store_property(
            properties,
            "length".into(),
            Value::Number(length),
            Some((false, false, true)),
        )?;
        Ok(Value::Function(index))
    }

    pub(super) fn bound_arguments(
        &mut self,
        mut index: usize,
        args: &[Value],
        bind_receiver: bool,
    ) -> Result<(usize, Vec<Value>, Value), Fault> {
        if args.len() > MAX_ARGUMENTS {
            return Err(Fault::new("Call argument limit reached (256)."));
        }
        self.work(std::mem::size_of_val(args))?;
        let mut arguments = Vec::with_capacity(args.len());
        for value in args {
            arguments.push(self.copy_value(value)?);
        }
        let mut receiver = Value::Undefined;
        loop {
            self.step()?;
            let Some(Function::Bound(bound)) = self.functions.get(index) else {
                break;
            };
            let total = bound.arguments.len().saturating_add(arguments.len());
            if total > MAX_ARGUMENTS {
                return Err(Fault::new("Call argument limit reached (256)."));
            }
            let work = std::mem::size_of::<Value>()
                .saturating_mul(total)
                .saturating_add(bound.arguments.iter().map(Value::bytes).sum::<usize>())
                .saturating_add(if bind_receiver {
                    bound.receiver.bytes()
                } else {
                    0
                });
            self.work(work)?;
            let Function::Bound(bound) = &self.functions[index] else {
                unreachable!()
            };
            let mut combined = Vec::with_capacity(total);
            combined.extend(bound.arguments.iter().cloned());
            combined.append(&mut arguments);
            arguments = combined;
            if bind_receiver {
                receiver = bound.receiver.clone();
            }
            index = bound.target;
        }
        Ok((index, arguments, receiver))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No native objects.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No native objects.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No native objects.".into())
        }
    }

    fn run(source: &str) -> Value {
        Runtime::for_test().execute(source, &mut NoHost).unwrap()
    }

    #[test]
    fn bind_captures_identity_arguments_and_earliest_receiver() {
        let result = run(r#"
            function add(a,b,c){'use strict';return this.n+a+b+c;}
            var receiver={n:1}, other={n:100};
            var first=add.bind(receiver,2), second=first.bind(other,3);
            receiver.n=10;
            if(second(4)!==19 || second.call(other,5)!==20 || second.apply(null,[6])!==21)throw 'bound arguments';
            var array=[1,2,3],pop=Array.prototype.pop.bind(array);
            if(pop()!==3 || pop.call([])!==2 || array.length!==1)throw 'bound native';
            var arrow=(()=>this).bind(receiver);
            if(arrow()!==this)throw 'lexical receiver';
            var maker=function(x){return function(y){return x+y;}.bind(null,3);};
            if(maker(7)()!==10)throw 'bound closure';
            'ok';
        "#);
        assert_eq!(result.to_text(), "ok");
    }

    #[test]
    fn bind_constructs_original_targets_and_delegates_instanceof() {
        run(r#"
            function Item(a,b){this.sum=a+b;}
            var first=Item.bind({sum:100},2), second=first.bind(null,3);
            if(first.hasOwnProperty('prototype'))throw 'bound prototype';
            first.prototype={wrong:1};
            var item=new second();
            if(item.sum!==5 || !(item instanceof Item) || !(item instanceof first) || !(item instanceof second))throw 'construction';
            if(Object.getPrototypeOf(item)!==Item.prototype)throw 'target prototype';
            Item.prototype={changed:1};
            if((new first(4)).changed!==1 || item instanceof first)throw 'live target prototype';
            function Returns(){return {marker:1};}
            if((new (Returns.bind(null))()).marker!==1)throw 'returned object';
            var arrow=(()=>1).bind(null), caught=false;
            try{new arrow();}catch(e){caught=e.name==='TypeError';}
            if(!caught)throw 'bound arrow constructible';
            var array=new (Array.bind(null,2,3))();
            if(array.length!==2 || array[0]!==2 || !(array instanceof Array))throw 'native construction';
        "#);
    }

    #[test]
    fn bind_metadata_observes_order_and_snapshots_target_prototype() {
        run(r#"
            function target(a,b,c){}
            var original=Object.getPrototypeOf(target), custom={}, order='';
            Object.defineProperty(target,'length',{get:function(){order+='l';Object.setPrototypeOf(target,custom);return 3.8;},configurable:true});
            Object.defineProperty(target,'name',{get:function(){order+='n';return 'test';},configurable:true});
            var bound=Function.prototype.bind.call(target,null,1);
            if(order!=='ln' || bound.name!=='bound test' || bound.length!==2 || Object.getPrototypeOf(bound)!==original)throw 'metadata';
            var d=Object.getOwnPropertyDescriptor(bound,'length');
            if(d.writable||d.enumerable||!d.configurable)throw 'length descriptor';
            d=Object.getOwnPropertyDescriptor(bound,'name');
            if(d.writable||d.enumerable||!d.configurable)throw 'name descriptor';
            if(Function.prototype.toString.call(bound)!=='function () { [native code] }')throw 'bound source';
            delete target.length;
            Object.defineProperty(custom,'length',{get:function(){throw 'inherited length';}});
            if(Function.prototype.bind.call(target,null).length!==0)throw 'own length only';
            Object.defineProperty(target,'length',{value:Infinity,configurable:true});
            if(Function.prototype.bind.call(target,null,1).length!==Infinity)throw 'infinite length';
            var marker={}, caught=false;
            Object.defineProperty(target,'length',{get:function(){throw marker;},configurable:true});
            try{Function.prototype.bind.call(target,null);}catch(e){caught=e===marker;}
            if(!caught)throw 'getter exception';
        "#);
    }

    #[test]
    fn bind_preserves_native_receiver_checks_and_indirect_eval_policy() {
        struct Guard;
        impl Host for Guard {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                Ok(Value::Undefined)
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                Err("forbidden".into())
            }
            fn call(&mut self, object: u32, method: &str, _: &[Value]) -> Result<Value, String> {
                assert_eq!((object, method), (7, "checked"));
                Ok(Value::Number(7.0))
            }
            fn allow_eval(&mut self) -> Result<bool, String> {
                Ok(false)
            }
        }
        let mut runtime = Runtime::for_test();
        let method = runtime.host_function(7, "checked").unwrap();
        let Value::Function(id) = method else {
            unreachable!()
        };
        if let Function::Host {
            receiver_required, ..
        } = &mut runtime.functions[id]
        {
            *receiver_required = true;
        }
        runtime
            .define_global("method", Value::Function(id))
            .unwrap();
        runtime.define_global("native", Value::Host(7)).unwrap();
        runtime
            .execute(
                r#"
            if(method.bind(native)()!==7)throw 'native binding';
            var wrong=false, denied=false;
            try{method.bind({})();}catch(e){wrong=e.name==='TypeError';}
            try{eval.bind(null)('1');}catch(e){denied=e.name==='EvalError';}
            if(!wrong || !denied)throw 'capability checks';
        "#,
                &mut Guard,
            )
            .unwrap();
    }

    #[test]
    fn bind_chains_share_argument_retention_and_execution_bounds() {
        let mut runtime = Runtime::for_test();
        runtime.execute("function target(){return arguments.length;} var bound=target; for(var i=0;i<400;i++)bound=bound.bind(null); if(bound(1,2)!==2)throw 'chain';",&mut NoHost).unwrap();
        runtime.begin_job();
        let target = runtime.execute("target;", &mut NoHost).unwrap();
        let Value::Function(id) = target else {
            unreachable!()
        };
        let mut prefix = vec![Value::Null];
        prefix.extend((0..255).map(|_| Value::Number(1.0)));
        let bound = runtime.bind_function(id, &prefix, &mut NoHost).unwrap();
        assert_eq!(
            runtime
                .invoke(
                    bound.clone(),
                    &[Value::Number(1.0)],
                    Value::Undefined,
                    &mut NoHost
                )
                .unwrap()
                .to_text(),
            "256"
        );
        assert!(runtime
            .invoke(
                bound,
                &[Value::Null, Value::Null],
                Value::Undefined,
                &mut NoHost
            )
            .unwrap_err()
            .contains("Call argument limit"));
        runtime.begin_job();
        runtime.value_bytes = MAX_RETAINED_VALUES;
        assert!(runtime
            .bind_function(id, &[], &mut NoHost)
            .unwrap_err()
            .message
            .contains("Retained value limit"));
        runtime.value_bytes = 0;
        runtime.steps = 0;
        assert!(runtime
            .bind_function(id, &[], &mut NoHost)
            .unwrap_err()
            .message
            .contains("Execution step limit"));
    }
}
