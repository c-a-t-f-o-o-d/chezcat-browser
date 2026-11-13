//! Original bounded ToPrimitive protocol. Lookup and calls share the existing
//! property/function arenas and job limits. Native handles retain their explicit
//! conversion capability check before any symbol or ordinary property read.
//! https://tc39.es/ecma262/multipage/abstract-operations.html#sec-toprimitive

use super::{Fault, Host, Ordinary, Runtime, Value, MAX_CALL_DEPTH};

#[derive(Clone, Copy)]
pub(super) enum Hint {
    Default,
    Number,
    String,
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
    fn conversion_protocol_preserves_hints_receiver_and_operator_order() {
        succeeds(
            r#"
var hints=[],o={};o[Symbol.toPrimitive]=function(hint){
if(this!==o||arguments.length!==1)throw 'receiver or arity';hints.push(hint);return hint==='string'?'field':6;};
if(o+1!==7||1+o!==7||!(o==6)||!(6==o))throw 'default conversions';
if(+o!==6||o-1!==5||!(o<7)||!(7>o))throw 'number conversions';
if(String(o)!=='field'||({field:9})[o]!==9)throw 'string conversions';
if(hints.join(',')!=='default,default,default,default,number,number,number,number,string,string')throw 'hint sequence';
var count=hints.length;if(o==null||o===6||!Boolean(o)||hints.length!==count)throw 'unneeded conversion';
var order='',a={},b={};a[Symbol.toPrimitive]=function(h){order+='a'+h;return 2;};
b[Symbol.toPrimitive]=function(h){order+='b'+h;return 3;};
if(a+b!==5||order!=='adefaultbdefault')throw 'left to right';
order='';try{Symbol('left')-b;}catch(e){if(e.name!=='TypeError')throw e;}
if(order!=='')throw 'numeric left rejection';
try{Symbol('left')+b;}catch(e){if(e.name!=='TypeError')throw e;}
if(order!=='bdefault')throw 'addition converts both';
'ok';
"#,
        );
    }

    #[test]
    fn conversion_protocol_reads_once_falls_back_and_propagates_errors() {
        succeeds(
            r#"
var reads=0,calls=0,parent={},child=Object.create(parent);
Object.defineProperty(parent,Symbol.toPrimitive,{get:function(){
if(this!==child)throw 'lookup receiver';reads++;return function(h){if(this!==child)throw 'call receiver';calls++;return 8;};},configurable:true});
if(Number(child)!==8||reads!==1||calls!==1)throw 'lookup once';
var order='',fallback={valueOf(){order+='v';return 3;},toString(){order+='s';return 'field';}};
fallback[Symbol.toPrimitive]=null;
if(+fallback!==3||String(fallback)!=='field'||order!=='vs')throw 'null fallback';
Object.defineProperty(fallback,Symbol.toPrimitive,{get:function(){order+='g';this.valueOf=function(){order+='n';return 4;};return undefined;}});
if(+fallback!==4||order!=='vsgn')throw 'live fallback';
var marker={},bad={},caught=0;
Object.defineProperty(bad,Symbol.toPrimitive,{get:function(){throw marker;},configurable:true});
try{String(bad);}catch(e){if(e===marker)caught++;}
Object.defineProperty(bad,Symbol.toPrimitive,{value:7});
try{+bad;}catch(e){if(e.name==='TypeError')caught++;}
Object.defineProperty(bad,Symbol.toPrimitive,{value:function(){throw marker;}});
try{+bad;}catch(e){if(e===marker)caught++;}
Object.defineProperty(bad,Symbol.toPrimitive,{value:function(){return {};}});
try{+bad;}catch(e){if(e.name==='TypeError')caught++;}
Object.defineProperty(bad,Symbol.toPrimitive,{value:function(){return function(){};}});
try{String(bad);}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==5)throw 'abrupt completions';
'ok';
"#,
        );
    }

    #[test]
    fn conversion_protocol_symbol_intrinsic_and_wrapper_branding() {
        succeeds(
            r#"
var key=Symbol.toPrimitive,d=Object.getOwnPropertyDescriptor(Symbol,'toPrimitive');
if(typeof key!=='symbol'||key.description!=='Symbol.toPrimitive'||d.writable||d.enumerable||d.configurable)throw 'key descriptor';
var hook=Symbol.prototype[key],hd=Object.getOwnPropertyDescriptor(Symbol.prototype,key);
if(hook.name!=='[Symbol.toPrimitive]'||hook.length!==1||hd.writable||hd.enumerable||!hd.configurable)throw 'hook metadata';
var s=Symbol('token'),boxed=Object(s),poison={toString(){throw 'hint conversion';}};
if(hook.call(s,poison)!==s||hook.call(boxed,poison)!==s||boxed.valueOf()!==s||boxed.toString()!=='Symbol(token)'||boxed.description!=='token')throw 'Symbol branding';
var o={};o[boxed]=7;if(o[s]!==7||Object.getOwnPropertySymbols(o)[0]!==s)throw 'boxed key';
var returns={};returns[key]=function(h){return s;};o[returns]=8;
if(o[s]!==8||!(returns==s))throw 'Symbol result';
var caught=0;for(var action of [()=>String(boxed),()=>Number(boxed),()=>String(returns),()=>Number(returns),()=>hook.call(Symbol.prototype),()=>hook.call(Object.create(boxed)),()=>hook.call({})]){
try{action();}catch(e){if(e.name==='TypeError')caught++;}}
if(caught!==7)throw 'invalid Symbol conversion';
Symbol.toPrimitive=Symbol('replacement');if(Symbol.toPrimitive!==key||delete Symbol.toPrimitive)throw 'immutable key';
'ok';
"#,
        );
        assert_eq!(
            Runtime::for_test()
                .execute("typeof Symbol.iterator;", &mut NoHost)
                .unwrap()
                .to_text(),
            "symbol"
        );
    }

    #[test]
    fn conversion_protocol_apply_length_observes_hooks_and_index_order() {
        succeeds(
            r#"
var order='',length={};
length[Symbol.toPrimitive]=function(hint){if(hint!=='number'||this!==length)throw 'length hint';order+='c';return 2.9;};
var list={get length(){order+='l';return length;},get 0(){order+='a';this[1]='changed';return 'first';},1:'second'};
function read(a,b){order+='f';if(arguments.length!==2)throw 'length truncation';return a+':'+b;}
if(read.apply(null,list)!=='first:changed'||order!=='lcaf')throw 'apply order';
function callable(a,b){};callable[0]='function';callable[1]='source';
if(read.apply(null,callable)!=='function:source')throw 'callable array-like';
var marker={},touched=0,broken={get length(){return {valueOf(){throw marker;}};},get 0(){touched++;}};
try{read.apply(null,broken);throw 'missed length error';}catch(e){if(e!==marker)throw e;}
if(touched!==0)throw 'early index read';
var caught=0;for(var candidate of [4,true,'text',Symbol('source')]){
try{read.apply(null,candidate);}catch(e){if(e.name==='TypeError')caught++;}}
try{read.apply(null,{length:Symbol('length')});}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==5)throw 'array-like validation';
'ok';
"#,
        );
    }

    #[test]
    fn conversion_protocol_apply_converted_length_keeps_argument_bound() {
        let mut runtime = Runtime::for_test();
        runtime
            .execute(
                "var converted=0,indexReads=0,called=false,caught=false,finalized=false;",
                &mut NoHost,
            )
            .unwrap();
        let error = runtime
            .execute(
                r#"
function target(){called=true;}
var length={};length[Symbol.toPrimitive]=function(h){converted++;return 257;};
var source={length:length,get 0(){indexReads++;return 1;}};
try{target.apply(null,source);}catch(e){caught=true;}finally{finalized=true;}
"#,
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("Call argument limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute(
                    "converted===1&&indexReads===0&&!called&&!caught&&!finalized;",
                    &mut NoHost
                )
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn conversion_protocol_native_callers_and_cycle_aware_array_join() {
        succeeds(
            r#"
var hints=[],o={};o[Symbol.toPrimitive]=function(h){hints.push(h);return h==='string'?'7':65;};
if(Math.floor(o)!==65||String.fromCharCode(o)!=='A'||parseInt(o,10)!==7||new Uint8Array([o])[0]!==65||[o].join('|')!=='7')throw 'native callers';
if(hints.join(',')!=='number,number,string,number,string')throw 'native hints';
var a=[1,2,3],calls=0,n={};n[Symbol.toPrimitive]=function(h){if(h!=='number')throw h;calls++;return 1;};
Object.defineProperty(a,'length',{value:n});if(calls!==2||a.length!==1||1 in a)throw 'array length double conversion';
var boxed=new Number(8);boxed[Symbol.toPrimitive]=function(h){return h==='string'?'eight':4;};
if([boxed].join()!=='eight'||Number(boxed)!==4||[new Number(3),new Boolean(false)].join('|')!=='3|false')throw 'wrapper join';
var cycle=[];cycle.push(cycle);if(cycle.join('!')!=='')throw 'plain array cycle';
cycle[Symbol.toPrimitive]=function(h){if(h!=='string')throw h;return 'cycle';};
if([cycle].join()!=='cycle')throw 'array exotic conversion';
'ok';
"#,
        );
    }

    #[test]
    fn conversion_protocol_primitive_symbol_setters_use_original_receiver() {
        succeeds(
            r#"
var key=Symbol('setter'),symbol=Symbol('receiver'),seen=[];
function setter(value){'use strict';seen.push(typeof this);seen.push(this===value);}
Object.defineProperty(Number.prototype,key,{set:setter,configurable:true});
Object.defineProperty(Boolean.prototype,key,{set:setter,configurable:true});
Object.defineProperty(Symbol.prototype,key,{set:setter,configurable:true});
(7)[key]=7;(false)[key]=false;symbol[key]=symbol;
delete Number.prototype[key];delete Boolean.prototype[key];delete Symbol.prototype[key];
if(seen.join('|')!=='number|true|boolean|true|symbol|true')throw 'original primitive receivers';
var caught=0;for(var value of [7,false,symbol]){
value[key]=1;if(value[key]!==undefined)throw 'retained primitive property';
try{(function(){'use strict';value[key]=2;})();}catch(e){if(e.name==='TypeError')caught++;}}
if(caught!==3)throw 'strict primitive assignments';
'ok';
"#,
        );
    }

    #[test]
    fn conversion_protocol_keeps_native_capability_gate_before_hook_lookup() {
        #[derive(Default)]
        struct Guard {
            allow: bool,
            reads: usize,
        }
        impl Host for Guard {
            fn primitive_conversion(&self, _: u32) -> bool {
                self.allow
            }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                self.reads += 1;
                Ok(Value::Undefined)
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                Err("No native writes.".into())
            }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
                Err("No native calls.".into())
            }
        }
        let mut runtime = Runtime::for_test();
        runtime.define_global("native", Value::Host(1)).unwrap();
        runtime.define_global("opaque", Value::Host(2)).unwrap();
        runtime.bind_host_object(1, &[]).unwrap();
        let mut host = Guard::default();
        runtime.execute("var gets=0,calls=0;Object.defineProperty(native,Symbol.toPrimitive,{get:function(){gets++;return function(h){calls++;if(this!==native||h!=='string')throw 'native receiver';return 'native text';};},configurable:true});", &mut host).unwrap();
        runtime.begin_job();
        let error = runtime.execute("String(native);", &mut host).unwrap_err();
        assert!(error.contains("Native host object property-key"), "{error}");
        assert_eq!(host.reads, 0);
        runtime.begin_job();
        assert_eq!(
            runtime.execute("gets+calls;", &mut host).unwrap().to_text(),
            "0"
        );
        host.allow = true;
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("String(native);", &mut host)
                .unwrap()
                .to_text(),
            "native text"
        );
        assert_eq!(host.reads, 0);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("gets===1&&calls===1;", &mut host)
                .unwrap()
                .to_text(),
            "true"
        );
        runtime.execute("Object.defineProperty(native,Symbol.toPrimitive,{value:function(){return opaque;}});var result='';try{String(native);}catch(e){result=e.name;}result;", &mut host).map(|value| assert_eq!(value.to_text(), "TypeError")).unwrap();
        assert_eq!(host.reads, 0);
    }

    #[test]
    fn conversion_denial_attributes_only_numeric_handle_and_fixed_hint_without_reads() {
        let mut runtime = Runtime::for_test();
        for hint in [Hint::Default, Hint::Number, Hint::String] {
            let before = (runtime.functions.len(), runtime.objects.len(), runtime.environments.len(), runtime.copy_bytes, runtime.steps, runtime.job_deadline);
            let fault = runtime.primitive_with_preference(Value::Host(1234), hint, &mut NoHost).unwrap_err();
            assert!(fault.exception.is_none());
            assert!(fault.message.contains(&format!("Native handle=1234; hint={}", hint.text())), "{}", fault.message);
            assert_eq!((runtime.functions.len(), runtime.objects.len(), runtime.environments.len(), runtime.copy_bytes, runtime.steps, runtime.job_deadline), (before.0, before.1, before.2, before.3, before.4 - 1, before.5));
        }
    }

    #[test]
    fn conversion_protocol_recursion_and_native_array_hook_cycles_are_bounded() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            for source in [
                "var o={};o[Symbol.toPrimitive]=function(){return +o;};try{+o;}catch(e){caught=true;}finally{finalized=true;}",
                "var o={};Object.defineProperty(o,Symbol.toPrimitive,{get:function(){return +o;}});try{+o;}catch(e){caught=true;}finally{finalized=true;}",
            ] {
                let mut runtime = Runtime::for_test();
                runtime.execute("var caught=false,finalized=false;", &mut NoHost).unwrap();
                let error = runtime.execute(source, &mut NoHost).unwrap_err();
                assert!(error.contains("limit"), "{error}");
                assert_eq!(runtime.call_depth, 0);
                assert_eq!(runtime.eval_depth, 0);
                runtime.begin_job();
                assert_eq!(runtime.execute("!caught&&!finalized;", &mut NoHost).unwrap().to_text(), "true");
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn conversion_protocol_native_array_hook_cycle_uses_shared_join_guard() {
        // The third original program from the fatal recursion test is retained
        // verbatim. Native join now recognizes this active array through the
        // Symbol.toPrimitive callback; the two genuine recursion programs above
        // remain fatal. Node returns normally and executes the finally block.
        let mut runtime = Runtime::for_test();
        runtime
            .execute("var caught=false,finalized=false;", &mut NoHost)
            .unwrap();
        runtime.execute("var o=[];o[0]=o;o[Symbol.toPrimitive]=Array.prototype.join;try{[o].join();}catch(e){caught=true;}finally{finalized=true;}", &mut NoHost).unwrap();
        assert_eq!(
            runtime
                .execute("!caught&&finalized&&[o].join()==='';", &mut NoHost)
                .unwrap()
                .to_text(),
            "true"
        );
        assert!(runtime.active_array_joins.is_empty());
    }
}

impl Hint {
    fn text(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Number => "number",
            Self::String => "string",
        }
    }
}

// Keep formatting scratch out of successful recursive conversion activations.
#[cold]
#[inline(never)]
fn native_conversion_denied(object: u32, hint: Hint) -> Fault {
    let message = if matches!(hint, Hint::String) {
        "Native host object property-key conversion is not implemented."
    } else {
        "Implicit native host object conversion is not implemented."
    };
    Fault::new(format!("{message} Native handle={object}; hint={}. No conversion permission or hook read was granted.", hint.text()))
}

impl Runtime {
    pub(super) fn primitive_with_preference(
        &mut self,
        value: Value,
        hint: Hint,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if let Value::Host(object) = value {
            if !host.primitive_conversion(object) {
                return Err(native_conversion_denied(object, hint));
            }
        } else if Ordinary::from_value(&value).is_none() {
            return Ok(value);
        }
        if let Some(primitive) = self.primitive_exotic(value.clone(), hint, host)? {
            return Ok(primitive);
        }
        self.primitive_ordinary(value, matches!(hint, Hint::String), host)
    }

    /// Called only after the host capability check above or for an ordinary
    /// array element. Symbol reads cannot create a native expando backing store.
    pub(super) fn primitive_exotic(
        &mut self,
        value: Value,
        hint: Hint,
        host: &mut dyn Host,
    ) -> Result<Option<Value>, Fault> {
        self.step()?;
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Object conversion call limit reached (32)."));
        }
        let marker = self.inhibit_transient_collection();
        self.call_depth += 1;
        let result = (|| {
            let Some(key) = self.symbol_to_primitive else {
                return Ok(None);
            };
            let method = self.get_symbol_property(value.clone(), key, host)?;
            if matches!(method, Value::Undefined | Value::Null) {
                return Ok(None);
            }
            if !matches!(method, Value::Function(_)) {
                return Err(Fault::exception(
                    "TypeError",
                    "Symbol.toPrimitive must be callable.",
                ));
            }
            let primitive =
                self.call_value(method, &[Value::String(hint.text().into())], value, host)?;
            if matches!(
                primitive,
                Value::Object(_) | Value::Function(_) | Value::Host(_)
            ) {
                return Err(Fault::exception(
                    "TypeError",
                    "Symbol.toPrimitive must return a primitive value.",
                ));
            }
            self.checked_value(primitive).map(Some)
        })();
        self.call_depth -= 1;
        self.restore_transient_marker(marker);
        result
    }

    pub(super) fn primitive_ordinary(
        &mut self,
        value: Value,
        string_hint: bool,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // Native methods can themselves convert values (for example Error's
        // name). Count the ordinary path too, including cycles with no hook.
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Object conversion call limit reached (32)."));
        }
        let marker = self.inhibit_transient_collection();
        self.call_depth += 1;
        let result = self.primitive_methods(value, string_hint, host);
        self.call_depth -= 1;
        self.restore_transient_marker(marker);
        result
    }
}
