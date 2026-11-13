//! Original bounded array/call spread over the shared synchronous iterator
//! protocol. Syntax slots remain separate from the expanded values so ordinary
//! expressions, holes and iterable steps keep their observable evaluation order.
//! https://tc39.es/ecma262/#sec-array-initializer-runtime-semantics-arrayaccumulation
//! https://tc39.es/ecma262/#sec-argument-lists-runtime-semantics-argumentlistevaluation

use super::{iterator_protocol, Argument, Fault, Host, Program, Runtime, Value};
#[cfg(test)]
use super::MAX_ARRAY_LENGTH;
use std::rc::Rc;

impl Runtime {
    pub(super) fn spread_iterator(
        &mut self,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<iterator_protocol::Record, Fault> {
        let method = self
            .iterator_method(&value, host)?
            .ok_or_else(|| Fault::exception("TypeError", "Spread requires an iterable value."))?;
        self.iterator_from_method(value, method, host)
    }

    #[cfg(test)]
    pub(super) fn eval_argument_list(
        &mut self,
        program: &Rc<Program>,
        arguments: &[Argument],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Vec<Value>, Fault> {
        let mut values = Vec::with_capacity(arguments.len().min(self.limits.capacity.arguments));
        for argument in arguments {
            if !argument.spread && values.len() == self.limits.capacity.arguments {
                return Err(Fault::new(format!("Call argument limit reached ({}).", self.limits.capacity.arguments)));
            }
            let value = self.eval(program, argument.expression, env, this, host)?;
            if argument.spread {
                self.append_spread_arguments(&mut values, value, host)?;
            } else {
                values.push(value);
            }
        }
        Ok(values)
    }

    // Ordinary nested arguments must not retain iterator/expansion scratch.
    // This helper is entered only after the spread operand has been evaluated.
    #[inline(never)]
    #[cfg(test)]
    fn append_spread_arguments(
        &mut self,
        values: &mut Vec<Value>,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let mut iterator = self.spread_iterator(value, host)?;
        while let Some(value) = self.iterator_next(&mut iterator, host)? {
            // Read terminal done at the boundary: an empty later spread
            // does not add an argument. A yielded excess value is fatal.
            if values.len() == self.limits.capacity.arguments {
                return Err(Fault::new(format!("Call argument limit reached ({}).", self.limits.capacity.arguments)));
            }
            values.push(value);
        }
        Ok(())
    }

    pub(super) fn eval_array_literal(
        &mut self,
        program: &Rc<Program>,
        elements: &[Option<Argument>],
        env: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let object = self.new_object(Some(0))?;
        let mut index = 0;
        for element in elements {
            self.step()?;
            let Some(element) = element else {
                if index == self.limits.heap.array_length {
                    return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                }
                index += 1;
                continue;
            };
            if !element.spread && index == self.limits.heap.array_length {
                return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
            }
            let value = self.eval(program, element.expression, env, this, host)?;
            if element.spread {
                let mut iterator = self.spread_iterator(value, host)?;
                while let Some(value) = self.iterator_next(&mut iterator, host)? {
                    if index == self.limits.heap.array_length {
                        return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                    }
                    // This fresh array is not exposed until evaluation ends.
                    // Define own data properties, bypassing inherited setters.
                    self.store_property(
                        object,
                        index.to_string(),
                        value,
                        Some((true, true, true)),
                    )?;
                    index += 1;
                }
            } else {
                self.store_property(object, index.to_string(), value, Some((true, true, true)))?;
                index += 1;
            }
        }
        // A trailing hole extends length without creating an own property.
        self.set_array_length(object, index)?;
        Ok(Value::Object(object))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No native reads.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No native writes.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No native calls.".into())
        }
        fn allow_eval(&mut self) -> Result<bool, String> {
            Ok(true)
        }
    }

    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn spread_arrays_preserve_holes_native_values_and_own_definitions() {
        succeeds(
            r#"
var a=[,1,...Array(2),,];
if(a.length!==5||Object.hasOwn(a,0)||!Object.hasOwn(a,2)||!Object.hasOwn(a,3)||Object.hasOwn(a,4))throw 'elisions and spread holes';
if(a[2]!==undefined||a[1]!==1)throw 'array values';
if([...new Set([2,2,3]),...new Uint8Array([4,5])].join()!=='2,3,4,5')throw 'native values';
if([...new Map([['key',9]])][0][1]!==9)throw 'map entries';
var points=[...'A😀é'];if(points.length!==3||points[1].length!==2||points.join('')!=='A😀é')throw 'code points';
var original=[1,2];original[Symbol.iterator]=function(){return ['changed'].values();};
if([0,...original,...[],3].join()!=='0,changed,3')throw 'override and empty spread';
var hits=0;Object.defineProperty(Array.prototype,'0',{set:function(){hits++;},configurable:true});
var own=[...[7]];delete Array.prototype[0];
var d=Object.getOwnPropertyDescriptor(own,'0');
if(hits!==0||own[0]!==7||!d.writable||!d.enumerable||!d.configurable)throw 'own definition';
'ok';
"#,
        );
    }

    #[test]
    fn spread_calls_preserve_callee_resolution_and_receivers() {
        succeeds(
            r#"
var order='',obj={n:4,get run(){order+='g';return function(a,b,c){order+='c';return this.n+a+b+c;};}};
var input={};input[Symbol.iterator]=function(){order+='i';Object.defineProperty(obj,'run',{value:function(){throw 'replaced method';},configurable:true});return [2].values();};
function mark(v){order+=v;return v;}
if(obj.run(mark(1),...input,mark(3))!==10||order!=='g1i3c')throw 'callee order';
var key=Symbol('method');obj[key]=function(v){if(this!==obj)throw 'symbol receiver';return v;};
if(obj[key](...[8])!==8)throw 'symbol method';
function plain(a){'use strict';if(this!==undefined)throw 'plain receiver';return a;}
if(plain(...[6])!==6||String.fromCharCode(...[65,66])!=='AB')throw 'plain and native calls';
var bound=function(a,b){return this.n+a+b;}.bind(obj,1);
if(bound(...[2])!==7)throw 'bound receiver';
var iterated=0,items={};items[Symbol.iterator]=function(){iterated++;return [1].values();};
var notCallable={f:3};try{notCallable.f(...items);throw 'missing call error';}catch(e){if(e.name!=='TypeError')throw e;}
if(iterated!==1)throw 'arguments before callable check';
try{null.f(...items);throw 'missing receiver error';}catch(e){if(e.name!=='TypeError')throw e;}
if(iterated!==1)throw 'receiver error before arguments';
'ok';
"#,
        );
    }

    #[test]
    fn spread_constructors_preserve_resolution_bound_arguments_and_native_behavior() {
        succeeds(
            r#"
var order='';function Pair(a,b){order+='c';this.sum=a+b;}
var holder={get C(){order+='g';return Pair;}},items={};
items[Symbol.iterator]=function(){order+='i';holder.C=0;return [2,3].values();};
var pair=new holder.C(...items);
if(pair.sum!==5||!(pair instanceof Pair)||order!=='gic')throw 'constructor resolution';
var Bound=Pair.bind(null,4),other=new Bound(...[5]);
if(other.sum!==9||!(other instanceof Pair)||!(other instanceof Bound))throw 'bound construction';
var sparse=new Array(...[3]);if(sparse.length!==3||Object.hasOwn(sparse,0))throw 'native Array constructor';
if(new Number(...[8]).valueOf()!==8||new Map(...[[['key',9]]]).get('key')!==9)throw 'other native constructors';
var seen=0,flow={};flow[Symbol.iterator]=function(){seen++;return [].values();};
var Arrow=()=>0;try{new Arrow(...flow);throw 'missing new error';}catch(e){if(e.name!=='TypeError')throw e;}
var Invalid=0;try{new Invalid(...flow);throw 'missing invalid error';}catch(e){if(e.name!=='TypeError')throw e;}
if(seen!==2)throw 'new evaluates arguments before constructability';
'ok';
"#,
        );
    }

    #[test]
    fn spread_custom_protocol_caches_next_and_preserves_live_values() {
        succeeds(
            r#"
var log='',input={},it,count=0;
Object.defineProperty(input,Symbol.iterator,{get:function(){log+='g';if(this!==input)throw 'lookup receiver';return function(){
if(this!==input||arguments.length!==0)throw 'iterator receiver';log+='i';
it={};Object.defineProperty(it,'next',{configurable:true,get:function(){log+='n';if(this!==it)throw 'next lookup';return function(){
if(this!==it||arguments.length!==0)throw 'step receiver';count++;Object.defineProperty(it,'next',{value:function(){throw 'uncached next';}});
return {get done(){log+='d';return count>2;},get value(){log+='v';return count;}};
};}});return it;};}});
if([0,...input,3].join()!=='0,1,2,3'||log!=='gindvdvd')throw 'custom read order';
var live=[1,2];Object.defineProperty(live,'0',{get:function(){live.push(3);return 1;},configurable:true});
if([...live].join()!=='1,2,3')throw 'live native length';
function fromArguments(){return [...arguments].join(':');}
if(fromArguments(7,8)!=='7:8')throw 'arguments iterator';
if([...[]].length!==0||(function(){return arguments.length;})(...[],...[])!==0)throw 'empty consumption';
'ok';
"#,
        );
    }

    #[test]
    fn spread_step_failures_preserve_exceptions_and_do_not_close() {
        succeeds(
            r#"
var marker={},closed=0,later=0,called=0;
function make(phase){var input={};Object.defineProperty(input,Symbol.iterator,{get:function(){
if(phase==='lookup')throw marker;return function(){if(phase==='call')throw marker;return {
get next(){if(phase==='nextget')throw marker;return function(){if(phase==='next')throw marker;if(phase==='result')return 0;
return {get done(){if(phase==='done')throw marker;return false;},get value(){throw marker;}};};},
return(){closed++;return {};}
};};}});return input;}
function touch(){later++;return 0;}function Target(){called++;}
for(var phase of ['lookup','call','nextget','next','result','done','value']){
try{[0,...make(phase),touch()];throw 'array missed error';}catch(e){if(phase==='result'?e.name!=='TypeError':e!==marker)throw e;}
try{Target(0,...make(phase),touch());throw 'call missed error';}catch(e){if(phase==='result'?e.name!=='TypeError':e!==marker)throw e;}
try{new Target(0,...make(phase),touch());throw 'new missed error';}catch(e){if(phase==='result'?e.name!=='TypeError':e!==marker)throw e;}
}
if(closed!==0||later!==0||called!==0)throw 'unexpected continuation or cleanup';
var failures=0;for(var bad of [null,undefined,{},3,false,Symbol('bad')]){
try{[...bad];}catch(e){if(e.name==='TypeError')failures++;}}
if(failures!==6)throw 'noniterable validation';
'ok';
"#,
        );
    }

    #[test]
    fn spread_direct_eval_keeps_current_specification_caller_scope() {
        // Node 24 treats an all-spread argument list differently here. Current
        // Function Calls step 6 uses ArgumentListEvaluation for direct eval with
        // no exclusion for spread. Preserve the rule and record that difference.
        succeeds(
            r#"
function localEval(){
var local=7;
if(eval(...['local'])!==7||eval(...[],'local')!==7||eval('local',...[])!==7)throw 'direct eval scope';
if(eval(...[])!==undefined)throw 'empty eval';
eval(...['var added=9;']);if(added!==9)throw 'direct declaration';
return eval(...['local+added','unused']);
}
if(localEval()!==16||typeof added!=='undefined')throw 'caller environment';
'ok';
"#,
        );
    }

    #[test]
    fn spread_argument_and_array_limits_remain_fatal_and_bounded() {
        succeeds(
            r#"
function size(){return arguments.length;}
if(size(...Array(256),...[])!==256||size(1,...Array(255))!==256)throw 'argument boundary';
var dense=[...Array(4096)];if(dense.length!==4096||!Object.hasOwn(dense,4095))throw 'property boundary';
'ok';
"#,
        );
        // Sparse literal slots can reach the logical bound without spending
        // the separate 4096 own-property allowance. The terminal empty spread
        // must still be accepted when exactly 8192 positions are present.
        let holes = ",".repeat(MAX_ARRAY_LENGTH - 2);
        succeeds(&format!(
            "var a=[{holes}...[1,2],...[]];if(a.length!==8192||a[8190]!==1||a[8191]!==2||Object.hasOwn(a,0))throw 'logical array boundary';'ok';"
        ));
        let excess = format!("var caught=false,finalized=false;try{{[{holes}...[1,2,3]];}}catch(e){{caught=true;}}finally{{finalized=true;}}");
        let mut runtime = Runtime::for_test();
        let error = runtime.execute(&excess, &mut NoHost).unwrap_err();
        assert!(error.contains("Array length limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("!caught&&!finalized;", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
        );
        for code in [
            "function f(){called=true;}try{f(...Array(257));}catch(e){caught=true;}finally{finalized=true;}",
            "function F(){called=true;}try{new F(...Array(257));}catch(e){caught=true;}finally{finalized=true;}",
            "try{[...Array(4097)];}catch(e){caught=true;}finally{finalized=true;}",
            "var flow={},item={done:false,value:1};flow[Symbol.iterator]=function(){return{next(){return item;},return(){closed=true;return {};}};};try{(function(){called=true;})(...flow);}catch(e){caught=true;}finally{finalized=true;}",
        ] {
            let mut runtime = Runtime::for_test();
            runtime.execute("var caught=false,finalized=false,called=false,closed=false;", &mut NoHost).unwrap();
            let error = runtime.execute(code, &mut NoHost).unwrap_err();
            assert!(error.contains("limit"), "{error}");
            assert_eq!(runtime.call_depth, 0);
            assert_eq!(runtime.eval_depth, 0);
            runtime.begin_job();
            assert_eq!(runtime.execute("!caught&&!finalized&&!called&&!closed;", &mut NoHost).unwrap().to_text(), "true");
        }
    }

    #[test]
    fn spread_native_handle_gate_precedes_lookup_and_callbacks() {
        for code in [
            "[...native];",
            "(function(){})(...native);",
            "new (function(){})(...native);",
        ] {
            let mut runtime = Runtime::for_test();
            runtime.define_global("native", Value::Host(7)).unwrap();
            runtime.execute("var reads=0;Object.defineProperty(Object.prototype,Symbol.iterator,{get:function(){reads++;return function(){};}});", &mut NoHost).unwrap();
            let error = runtime.execute(code, &mut NoHost).unwrap_err();
            assert!(error.contains("Host-object iteration"), "{error}");
            runtime.begin_job();
            assert_eq!(
                runtime.execute("reads;", &mut NoHost).unwrap().to_text(),
                "0"
            );
        }
    }

    #[test]
    fn spread_recursive_iterator_callbacks_are_bounded_on_small_native_stack() {
        std::thread::Builder::new().name("spread-recursive-iterator".into()).stack_size(1024 * 1024).spawn(|| {
            for (case, code) in [
                "var input={};Object.defineProperty(input,Symbol.iterator,{get:function(){return [...input];}});try{[...input];}catch(e){caught=true;}finally{finalized=true;}",
                "var input={};input[Symbol.iterator]=function(){return{next(){return {done:false,value:[...input]};}};};try{[...input];}catch(e){caught=true;}finally{finalized=true;}",
            ].into_iter().enumerate() {
                eprintln!("spread recursion case {case}: starting");
                let mut runtime=Runtime::for_test();
                runtime.execute("var caught=false,finalized=false;", &mut NoHost).unwrap();
                let error=runtime.execute(code,&mut NoHost).unwrap_err();
                assert!(error.contains("limit"), "{error}");
                assert_eq!(runtime.call_depth,0);
                assert_eq!(runtime.eval_depth,0);
                runtime.begin_job();
                assert_eq!(runtime.execute("!caught&&!finalized;",&mut NoHost).unwrap().to_text(),"true");
            }
        }).unwrap().join().unwrap();
    }
}
