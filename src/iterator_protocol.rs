//! Original synchronous iterator protocol over the existing bounded runtime.
//! The record caches next once, while return is looked up only on early exit.
//! Native steps avoid creating a result object that cannot be observed; explicit
//! next calls and overridden next methods retain normal observable objects.
//! https://tc39.es/ecma262/#sec-getiteratorfrommethod
//! https://tc39.es/ecma262/#sec-iteratorstepvalue
//! https://tc39.es/ecma262/#sec-iteratorclose

use super::{Builtin, Fault, Function, Host, Ordinary, Runtime, Value, MAX_CALL_DEPTH};
use super::function_reachability::Edge;
use super::generator_functions::Op as GeneratorOp;

pub(super) struct Record {
    iterator: Value,
    next: Value,
    done: bool,
}

pub(super) enum Delegated {
    Yield(Value),
    Done(Value),
    Return(Value),
}

impl Record {
    pub(super) fn retained_bytes(&self) -> usize {
        self.iterator.bytes().saturating_add(self.next.bytes())
    }

    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(Edge)) {
        Edge::value(&self.iterator, add);
        Edge::value(&self.next, add);
    }
}

#[derive(Clone, Copy)]
pub(super) enum Op {
    Self_,
    String,
}

impl Runtime {
    pub(super) fn iterator_method(
        &mut self,
        source: &Value,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        // Approval and origin validation precede even an observable iterator
        // getter. Unrelated native handles keep the explicit refusal.
        if let Value::Host(handle) = source {
            self.validate_indexed_host_iteration(*handle, None, host)?;
        }
        let receiver = self.copy_value(source)?;
        let method = self.get_symbol_property(receiver, self.symbol_iterator.unwrap(), host)?;
        match method {
            Value::Undefined | Value::Null => Ok(None),
            Value::Function(_) => Ok(Some(method)),
            _ => Err(Fault::exception(
                "TypeError",
                "Iterator method is not callable.",
            )),
        }
    }

    pub(super) fn iterator_from_method(
        &mut self,
        source: Value,
        method: Value,
        host: &mut dyn Host,
    ) -> Result<Record, Fault> {
        let marker = self.inhibit_transient_collection();
        let result = self.iterator_from_method_unpublished(source, method, host);
        self.restore_transient_marker(marker);
        result
    }

    fn iterator_from_method_unpublished(
        &mut self,
        source: Value,
        method: Value,
        host: &mut dyn Host,
    ) -> Result<Record, Fault> {
        if let Value::Host(handle) = &source {
            self.validate_indexed_host_iteration(*handle, None, host)?;
        }
        let iterator = self.call_value(method, &[], source, host)?;
        let object = self.iterator_ordinary(&iterator)?;
        let next = self.get_ordinary_property_with_host(object, "next", host)?;
        Ok(Record {
            iterator,
            next,
            done: false,
        })
    }

    fn iterator_ordinary(&self, value: &Value) -> Result<Ordinary, Fault> {
        if matches!(value, Value::Host(_)) {
            return Err(Fault::new(
                "Host-object iterator results are not implemented.",
            ));
        }
        Ordinary::from_value(value).ok_or_else(|| {
            Fault::exception("TypeError", "Iterator operation must return an object.")
        })
    }

    pub(super) fn iterator_next(
        &mut self,
        record: &mut Record,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        self.iterator_advance(record, true, host)
    }

    pub(super) fn iterator_skip(
        &mut self,
        record: &mut Record,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.iterator_advance(record, false, host).map(|_| ())
    }

    fn iterator_advance(
        &mut self,
        record: &mut Record,
        read_value: bool,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        // Includes the optimized native-next path, which bypasses call_value,
        // and custom result objects retained across done/value getters.
        let marker = self.inhibit_transient_collection();
        let result = self.iterator_advance_unpublished(record, read_value, host);
        self.restore_transient_marker(marker);
        result
    }

    fn iterator_advance_unpublished(
        &mut self,
        record: &mut Record,
        read_value: bool,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        self.step()?;
        if record.done {
            return Ok(None);
        }
        let result = (|| {
            if let (Value::Object(id), Value::Function(next)) = (&record.iterator, &record.next) {
                if let Some(Function::Native(Builtin::Collection(super::collections::Op::Next(
                    family,
                )))) = self.functions.get(*next)
                {
                    let family = *family;
                    if self
                        .object(*id)?
                        .iterator
                        .as_ref()
                        .is_some_and(|iterator| iterator.family == family)
                    {
                        // The genuine native next result has only data fields.
                        // Reading its done/value immediately cannot expose its
                        // identity, so keep the previous native-loop allocation
                        // behavior instead of spending an object for each step.
                        if self.call_depth >= MAX_CALL_DEPTH {
                            return Err(Fault::new("Function call limit reached (32)."));
                        }
                        self.call_depth += 1;
                        let result = self.next_native_iterator(*id, host);
                        self.call_depth -= 1;
                        return result;
                    }
                }
            }
            let next = self.copy_value(&record.next)?;
            let receiver = self.copy_value(&record.iterator)?;
            let result = self.call_value(next, &[], receiver, host)?;
            let object = self.iterator_ordinary(&result)?;
            let done = self.get_ordinary_property_with_host(object, "done", host)?;
            if done.truthy() {
                return Ok(None);
            }
            if read_value {
                self.get_ordinary_property_with_host(object, "value", host)
                    .map(Some)
            } else {
                // An elision advances the iterator without reading a custom
                // iterator result's observable value getter.
                Ok(Some(Value::Undefined))
            }
        })();
        // A failed next call/result/done/value read is not followed by return.
        if !matches!(&result, Ok(Some(_))) {
            record.done = true;
        }
        result
    }

    pub(super) fn iterator_close<T>(
        &mut self,
        record: &mut Record,
        completion: Result<T, Fault>,
        host: &mut dyn Host,
    ) -> Result<T, Fault> {
        // The generic completion can own an unpublished Value as well as the
        // iterator record. Preserve it across live return lookup and callbacks.
        let marker = self.inhibit_transient_collection();
        let result = self.iterator_close_unpublished(record, completion, host);
        self.restore_transient_marker(marker);
        result
    }

    fn iterator_control_method(
        &mut self, record: &Record, name: &str, host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        let object = self.iterator_ordinary(&record.iterator)?;
        let method = self.get_ordinary_property_with_host(object, name, host)?;
        match method {
            Value::Null | Value::Undefined => Ok(None),
            Value::Function(_) => Ok(Some(method)),
            _ => Err(Fault::exception("TypeError", format!("Iterator {name} is not callable."))),
        }
    }

    #[inline(never)]
    pub(super) fn delegate_iterator(
        &mut self, record: &mut Record, operation: GeneratorOp, input: &Value, host: &mut dyn Host,
    ) -> Result<Delegated, Fault> {
        // The record and original result object remain borrowed across
        // IteratorComplete/IteratorValue getters and iterator callbacks.
        let marker = self.inhibit_transient_collection();
        let result = (|| {
            self.step()?;
            let method = match operation {
                GeneratorOp::Next => self.copy_value(&record.next)?,
                GeneratorOp::Return => {
                    let Some(method) = self.iterator_control_method(record, "return", host)? else {
                        record.done = true;
                        return self.copy_value(input).map(Delegated::Return);
                    };
                    method
                }
                GeneratorOp::Throw => {
                    let Some(method) = self.iterator_control_method(record, "throw", host)? else {
                        self.iterator_close_unpublished(record, Ok(()), host)?;
                        return Err(Fault::exception("TypeError", "Delegated iterator has no throw method."));
                    };
                    method
                }
            };
            let receiver = self.copy_value(&record.iterator)?;
            let input = self.copy_value(input)?;
            let result = self.call_value(method, &[input], receiver, host)?;
            let object = self.iterator_ordinary(&result)?;
            if !self.get_ordinary_property_with_host(object, "done", host)?.truthy() {
                return Ok(Delegated::Yield(result));
            }
            record.done = true;
            let value = self.get_ordinary_property_with_host(object, "value", host)?;
            if matches!(operation, GeneratorOp::Return) { Ok(Delegated::Return(value)) }
            else { Ok(Delegated::Done(value)) }
        })();
        self.restore_transient_marker(marker);
        result
    }

    fn iterator_close_unpublished<T>(
        &mut self,
        record: &mut Record,
        completion: Result<T, Fault>,
        host: &mut dyn Host,
    ) -> Result<T, Fault> {
        // Resource/capability failures are fatal throughout ChezCat. Running
        // cleanup after them could extend the exhausted job's allowance.
        if record.done
            || completion
                .as_ref()
                .is_err_and(|error| error.exception.is_none())
        {
            return completion;
        }
        record.done = true;
        let closed = (|| {
            let Some(method) = self.iterator_control_method(record, "return", host)? else {
                return Ok(());
            };
            let receiver = self.copy_value(&record.iterator)?;
            let result = self.call_value(method, &[], receiver, host)?;
            // No property of the returned object is read, including native
            // handles. Merely returning an existing handle grants no access.
            if !matches!(
                result,
                Value::Object(_) | Value::Function(_) | Value::Host(_)
            ) {
                return Err(Fault::exception(
                    "TypeError",
                    "Iterator return must return an object.",
                ));
            }
            Ok(())
        })();
        // A fatal cleanup fault remains fatal even when the original completion
        // was a script throw. Semantic close failures lose to an existing throw.
        if closed
            .as_ref()
            .is_err_and(|error| error.exception.is_none())
        {
            return Err(closed.err().unwrap());
        }
        match completion {
            Err(error) => Err(error),
            Ok(value) => closed.map(|()| value),
        }
    }

    pub(super) fn iterator_builtin(
        &mut self,
        operation: Op,
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = (|| match operation {
            Op::Self_ => self.copy_value(&this),
            Op::String => {
                Self::require_property_receiver(&this)?;
                let primitive = self.primitive_with_hint(this, true, host)?;
                self.work(primitive.bytes())?;
                let text = primitive.script_text()?;
                self.new_string_iterator(text).map(Value::Object)
            }
        })();
        self.call_depth -= 1;
        result
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
    fn iterator_protocol_intrinsic_aliases_descriptors_and_native_receivers() {
        succeeds(
            r#"
var k=Symbol.iterator,d=Object.getOwnPropertyDescriptor(Symbol,'iterator');
if(typeof k!=='symbol'||k.description!=='Symbol.iterator'||d.writable||d.enumerable||d.configurable)throw 'key descriptor';
if(Array.prototype[k]!==Array.prototype.values||Map.prototype[k]!==Map.prototype.entries||Set.prototype[k]!==Set.prototype.values||Set.prototype.keys!==Set.prototype.values)throw 'native aliases';
if(Uint8Array.prototype[k]!==Uint8Array.prototype.values||Array.prototype[k].name!=='values'||Array.prototype[k].length!==0)throw 'array metadata';
var it=[2].values(),proto=Object.getPrototypeOf(Object.getPrototypeOf(it));
if(it[k]()!==it||proto[k].call(9)!==9||proto[k].name!=='[Symbol.iterator]'||proto[k].length!==0)throw 'iterator identity';
var pd=Object.getOwnPropertyDescriptor(proto,k);if(pd.enumerable||!pd.writable||!pd.configurable)throw 'iterator descriptor';
var err='';try{it.next.call({});}catch(e){err=e.name;}if(err!=='TypeError')throw 'brand';
function args(a){var old=Array.prototype.values;Array.prototype.values=function(){throw 'replacement';};try{var d=Object.getOwnPropertyDescriptor(arguments,k);if(d.value!==old||d.enumerable||!d.writable||!d.configurable)throw 'arguments alias';var n=0;for(var x of arguments)n+=x;return n;}finally{Array.prototype.values=old;}}
if(args(3,4)!==7)throw 'arguments iteration';
'ok';
"#,
        );
    }

    #[test]
    fn iterator_protocol_caches_next_and_orders_done_before_value() {
        succeeds(
            r#"
var order='',steps=0,iter={};
Object.defineProperty(iter,'next',{get:function(){order+='n';return function(){if(this!==iter||arguments.length!==0)throw 'next receiver';steps++;if(steps===1)Object.defineProperty(iter,'next',{value:function(){throw 'reread';}});return {get done(){order+='d';return steps>2;},get value(){order+='v';return steps;}};};},configurable:true});
var items={};Object.defineProperty(items,Symbol.iterator,{get:function(){order+='g';return function(){if(this!==items||arguments.length!==0)throw 'method receiver';order+='i';return iter;};}});
var total=0;for(var x of items)total+=x;
if(total!==3||order!=='gindvdvd')throw order;
var functionResult=function(){};functionResult.done=true;
var functionIterator=function(){};functionIterator.next=function(){return functionResult;};
var callableItems={};callableItems[Symbol.iterator]=function(){return functionIterator;};for(var x of callableItems)throw 'done function';
'ok';
"#,
        );
    }

    #[test]
    fn iterator_protocol_closes_break_return_outer_continue_and_target_throws() {
        succeeds(
            r#"
var closes='',count=0;function values(label){var it={next(){return {done:false,value:++count};},return(){closes+=label;return {};}};it[Symbol.iterator]=function(){return this;};return it;}
for(var x of values('b')){if(x===1)continue;break;}
function stop(){for(var x of values('r'))return x;}stop();
var rounds=0;outer:for(var o of values('o')){for(var i of values('i')){rounds++;if(rounds===1)continue outer;break outer;}}
var marker={},target={set x(v){throw marker;}};try{for(target.x of values('t')){throw 'body';}}catch(e){if(e!==marker)throw 'target exception';}
if(closes!=='briiot')throw closes;
'ok';
"#,
        );
    }

    #[test]
    fn iterator_protocol_close_error_precedence_and_live_return_lookup() {
        succeeds(
            r#"
var original={},closing={},count=0;
function create(mode){var it={next(){return {done:false,value:1};}};it[Symbol.iterator]=function(){return this;};Object.defineProperty(it,'return',{get:function(){count++;if(mode===0)throw closing;if(mode===1)return function(){throw closing;};if(mode===2)return function(){return 1;};return 7;}});return it;}
for(var mode=0;mode<4;mode++){
try{for(var x of create(mode))throw original;}catch(e){if(e!==original)throw 'throw precedence';}
try{for(var x of create(mode))break;}catch(e){if(mode<2?e!==closing:e.name!=='TypeError')throw 'break precedence';}
function exit(){for(var x of create(mode))return 7;}try{exit();throw 'no close error';}catch(e){if(mode<2?e!==closing:e.name!=='TypeError')throw 'return precedence';}
}
if(count!==12)throw 'close count';
var it={next(){return {value:1,done:false};}};it[Symbol.iterator]=function(){return this;};var seen='';
for(var x of it){it.return=function(){if(this!==it||arguments.length!==0)throw 'return receiver';seen='live';return function(){};};break;}
if(seen!=='live')throw 'return was cached';
'ok';
"#,
        );
    }

    #[test]
    fn iterator_protocol_step_and_lookup_failures_do_not_close() {
        succeeds(
            r#"
var marker={},closes=0;
function check(mode){var it={return(){closes++;return {};}};it[Symbol.iterator]=function(){return this;};
Object.defineProperty(it,'next',{get:function(){if(mode===0)throw marker;if(mode===1)return 3;return function(){if(mode===2)throw marker;if(mode===3)return 4;if(mode===4)return {get done(){throw marker;}};return {done:false,get value(){throw marker;}};};}});
try{for(var x of it)throw 'body';throw 'missing failure';}catch(e){if(mode===1||mode===3){if(e.name!=='TypeError')throw 'step type';}else if(e!==marker)throw 'step identity';}}
for(var mode=0;mode<6;mode++)check(mode);
var bad={};bad[Symbol.iterator]=function(){return 3;};try{for(var x of bad){};throw 'accepted primitive iterator';}catch(e){if(e.name!=='TypeError')throw e;}
var done={next(){return {done:true,get value(){throw 'value read on done';}};},return(){closes++;return {};}};done[Symbol.iterator]=function(){return this;};for(var x of done)throw 'done body';
if(closes!==0)throw 'closed failed step';
'ok';
"#,
        );
    }

    #[test]
    fn iterator_protocol_native_live_iteration_overrides_and_string_code_points() {
        succeeds(
            r#"
var a=[1,,3],got='';for(var x of a){got+=String(x)+',';if(a.length===3)a.push(4);}
if(got!=='1,undefined,3,4,')throw got;
var it;var aLike={length:2,get 0(){return it.next().value;},1:9};it=Array.prototype.values.call(aLike);
if(it.next().value!==9||!it.next().done)throw 'reentrant advance';
var single={next(){return {done:false,value:8};}};single[Symbol.iterator]=function(){return this;};
a[Symbol.iterator]=function(){return single;};var first=0;for(var x of a){first=x;break;}if(first!==8)throw 'array override';
var m=new Map([[1,2]]);m[Symbol.iterator]=function(){return [7].values();};if(new Set(m).has(7)!==true)throw 'collection override';
var t=new Uint8Array([1,2]);t[Symbol.iterator]=function(){return [6,5].values();};var seen='';for(var x of t)seen+=x;if(seen!=='65'||Uint8Array.from(t).join()!=='6,5')throw 'typed override';
var points='',lengths='';for(var ch of 'aé𐀀'){points+=ch;lengths+=ch.length;}
if(points!=='aé𐀀'||lengths!=='112')throw 'string points';
var si=String.prototype[Symbol.iterator].call({toString(){return 'xy';}});if(si[Symbol.iterator]()!==si||si.next().value!=='x'||si.next().value!=='y'||!si.next().done||!si.next().done)throw 'string iterator';
'ok';
"#,
        );
    }

    #[test]
    fn iterator_protocol_collection_consumers_close_entry_and_adder_errors() {
        succeeds(
            r#"
var closes=0,marker={};function items(v){var it={next(){return {done:false,value:v};},return(){closes++;return {};}};it[Symbol.iterator]=function(){return this;};return it;}
try{new Map(items(3));}catch(e){if(e.name!=='TypeError')throw e;}
try{new Map(items({get 0(){throw marker;}}));}catch(e){if(e!==marker)throw e;}
var old=Map.prototype.set;Map.prototype.set=function(){throw marker;};try{new Map(items([1,2]));}catch(e){if(e!==marker)throw e;}Map.prototype.set=old;
var old=Set.prototype.add;Set.prototype.add=function(){throw marker;};try{new Set(items(1));}catch(e){if(e!==marker)throw e;}Set.prototype.add=old;
if(closes!==4)throw 'collection close';
var custom={};custom[Symbol.iterator]=function(){return [3,4].values();};if(new Uint8Array(custom).join()!=='3,4'||Uint8Array.from(custom,function(v,i){return v+i;}).join()!=='3,5')throw 'typed custom consumption';
'ok';
"#,
        );
    }

    #[test]
    fn iterator_protocol_native_steps_preserve_object_budget_and_fatal_exit() {
        succeeds("var count=0;for(var x of Array(6000))count++;if(count!==6000)throw count;'ok';");
        let mut runtime = Runtime::for_test();
        let error=runtime.execute(r#"
var closed=false,caught=false,finalized=false,result={done:false,value:1};
var it={next(){return result;},return(){closed=true;return {};}};it[Symbol.iterator]=function(){return this;};
try{for(var x of it){}}catch(e){caught=true;}finally{finalized=true;}
"#,&mut NoHost).unwrap_err();
        assert!(
            error.contains("limit") || error.contains("budget"),
            "{error}"
        );
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("closed+':'+caught+':'+finalized;", &mut NoHost)
                .unwrap()
                .to_text(),
            "false:false:false"
        );
        assert_eq!(runtime.call_depth, 0);
        assert_eq!(runtime.eval_depth, 0);
    }

    #[test]
    fn iterator_protocol_rejects_native_handles_before_property_callbacks() {
        let mut runtime = Runtime::for_test();
        runtime.define_global("native", Value::Host(7)).unwrap();
        let error=runtime.execute("var reads=0;Object.defineProperty(Object.prototype,Symbol.iterator,{get:function(){reads++;return function(){};}});for(var x of native){};",&mut NoHost).unwrap_err();
        assert!(error.contains("Host-object iteration"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime.execute("reads;", &mut NoHost).unwrap().to_text(),
            "0"
        );
    }

    #[test]
    fn iterator_protocol_fatal_recursive_lookup_and_native_string_conversion_are_bounded() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            for code in [
                "var obj={};Object.defineProperty(obj,Symbol.iterator,{get:function(){for(var x of obj){}return undefined;}});try{for(var x of obj){}}catch(e){caught=true;}finally{finalized=true;}",
                "var obj={toString:String.prototype[Symbol.iterator]};try{String.prototype[Symbol.iterator].call(obj);}catch(e){caught=true;}finally{finalized=true;}",
            ] {
                let mut runtime=Runtime::for_test();
                let source=format!("var caught=false,finalized=false;{code}");
                let error=runtime.execute(&source,&mut NoHost).unwrap_err();
                assert!(error.contains("limit"),"{error}");
                assert_eq!(runtime.call_depth,0);
                assert_eq!(runtime.eval_depth,0);
                runtime.begin_job();
                assert_eq!(runtime.execute("caught+':'+finalized;",&mut NoHost).unwrap().to_text(),"false:false");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn iterator_protocol_fatal_return_exhaustion_overrides_body_throw() {
        let mut runtime = Runtime::for_test();
        let error=runtime.execute(r#"
var closed=false,caught=false,finalized=false,marker={};
var it={next(){return {done:false,value:1};},return(){closed=true;while(true){};}};it[Symbol.iterator]=function(){return this;};
try{for(var x of it){throw marker;}}catch(e){caught=true;}finally{finalized=true;}
"#,&mut NoHost).unwrap_err();
        assert!(
            error.contains("limit") || error.contains("budget"),
            "{error}"
        );
        assert_eq!(runtime.call_depth, 0);
        assert_eq!(runtime.eval_depth, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("closed+':'+caught+':'+finalized;", &mut NoHost)
                .unwrap()
                .to_text(),
            "true:false:false"
        );
    }
}
