//! Original bounded consumers of the shared synchronous iterator protocol.
//! Construction, mapping and property definition keep their observable order;
//! abrupt consumers close an active iterator through the shared completion rules.
//! https://tc39.es/ecma262/#sec-array.from
//! https://tc39.es/ecma262/#sec-object.fromentries

use super::{
    Builtin, Fault, Function, Host, PropertyKey, Reference, Runtime, Value, MAX_CALL_DEPTH,
};

#[derive(Clone, Copy)]
pub(super) enum Op {
    ArrayFrom,
    ObjectFromEntries,
}

impl Op {
    pub(super) fn arity(self) -> usize {
        1
    }
}

impl Runtime {
    pub(super) fn initialize_iterable_builtins(&mut self) -> Result<(), Fault> {
        for (constructor, name, operation) in [
            ("Array", "from", Op::ArrayFrom),
            ("Object", "fromEntries", Op::ObjectFromEntries),
        ] {
            let constructor = self.binding_value(self.global_environment, constructor)?;
            let owner = self.ordinary_id(self.ordinary_required(&constructor)?)?;
            self.prototype_method(owner, name, Builtin::Iterable(operation))?;
        }
        Ok(())
    }

    pub(super) fn iterable_builtin(
        &mut self,
        operation: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("Function call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = match operation {
            Op::ArrayFrom => self.array_from(args, this, host),
            Op::ObjectFromEntries => {
                self.object_from_entries(args.first().unwrap_or(&Value::Undefined), host)
            }
        };
        self.call_depth -= 1;
        result
    }

    pub(super) fn from_constructor(&mut self, value: &Value) -> Result<bool, Fault> {
        let Value::Function(mut index) = *value else {
            return Ok(false);
        };
        loop {
            self.step()?;
            match self.functions.get(index) {
                Some(Function::Bound(bound)) => index = bound.target,
                Some(Function::Script {
                    program, template, ..
                }) => {
                    return Ok(program.templates[*template].constructible);
                }
                Some(Function::Host { constructible, .. }) => return Ok(*constructible),
                Some(Function::Native(
                    Builtin::Object
                    | Builtin::Proxy
                    | Builtin::Date(super::date::Op::Constructor)
                    | Builtin::Url(super::url_api::Op::Constructor(_))
                    | Builtin::Rect(super::rect_api::Op::Constructor(_))
                    | Builtin::DomException(super::dom_exception::Op::Constructor)
                    | Builtin::Promise(
                        super::promises::Op::Constructor | super::promises::Op::AggregateError,
                    )
                    | Builtin::Array
                    | Builtin::Number
                    | Builtin::Boolean
                    | Builtin::String
                    | Builtin::Function
                    | Builtin::RegExp
                    | Builtin::Error(_)
                    | Builtin::Collection(super::collections::Op::Constructor(_))
                    | Builtin::Binary(super::binary_data::Op::Constructor(_))
                    | Builtin::TextEncoding(super::text_encoding::Op::Constructor(_)),
                )) => return Ok(true),
                Some(Function::Vacant) | None => return Err(Fault::new("Invalid function handle.")),
                Some(_) => return Ok(false),
            }
        }
    }

    fn array_from_target(
        &mut self,
        constructor: &Value,
        length: Option<usize>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let target = if self.from_constructor(constructor)? {
            let arguments = length.map(|length| [Value::Number(length as f64)]);
            self.construct_value(
                constructor.clone(),
                arguments.as_ref().map_or(&[][..], |a| &a[..]),
                host,
            )?
        } else {
            Value::Object(self.new_object(Some(length.unwrap_or(0)))?)
        };
        // A script constructor may return a native browser handle. Generic
        // data-property creation must not turn it into a new host capability.
        self.ordinary_required(&target)?;
        Ok(target)
    }

    fn array_from_map(
        &mut self,
        mapper: &Value,
        receiver: &Value,
        value: Value,
        index: usize,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if matches!(mapper, Value::Undefined) {
            return Ok(value);
        }
        let receiver = self.copy_value(receiver)?;
        self.call_value(
            mapper.clone(),
            &[value, Value::Number(index as f64)],
            receiver,
            host,
        )
    }

    fn array_from_length(
        &mut self,
        target: &Value,
        length: usize,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        // Set with Throw=true, including an ordinary constructor result's
        // inherited length setter. A completed iterator is not closed here.
        let strict = self.strict;
        self.strict = true;
        let result = self.write_reference(
            Reference::Property(target.clone(), "length".into()),
            Value::Number(length as f64),
            host,
        );
        self.strict = strict;
        result
    }

    fn array_from(
        &mut self,
        args: &[Value],
        constructor: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let source = args.first().unwrap_or(&Value::Undefined);
        let mapper = args.get(1).unwrap_or(&Value::Undefined);
        let receiver = args.get(2).unwrap_or(&Value::Undefined);
        if !matches!(mapper, Value::Undefined | Value::Function(_)) {
            return Err(Fault::exception(
                "TypeError",
                "Array.from mapper is not callable.",
            ));
        }
        let method = self.iterator_method(source, host)?;
        if let Some(method) = method {
            // GetMethod precedes construction; invocation uses that captured
            // method after construction, even if the constructor changes it.
            let target = self.array_from_target(&constructor, None, host)?;
            let source = self.copy_value(source)?;
            let mut iterator = self.iterator_from_method(source, method, host)?;
            let mut index = 0;
            loop {
                self.step()?;
                let Some(value) = self.iterator_next(&mut iterator, host)? else {
                    self.array_from_length(&target, index, host)?;
                    return Ok(target);
                };
                if index >= self.limits.heap.array_length {
                    return Err(Fault::new(format!("Array.from length limit reached ({}).", self.limits.heap.array_length)));
                }
                let mapped = self.array_from_map(mapper, receiver, value, index, host);
                let defined = mapped.and_then(|value| {
                    self.create_data_property(
                        &target,
                        PropertyKey::String(index.to_string()),
                        value,
                        host,
                    )
                });
                if let Err(error) = defined {
                    return self.iterator_close(&mut iterator, Err(error), host);
                }
                index += 1;
            }
        }

        // Array-like scalar sources use genuine wrappers so inherited length
        // and index accessors receive the object. String boxing remains outside
        // the documented subset; ordinary string iterators do not need boxing.
        let source = match source {
            Value::Object(_) | Value::Function(_) => self.copy_value(source)?,
            Value::Number(_) | Value::Bool(_) | Value::Symbol(_) | Value::String(_) => {
                let source = self.copy_value(source)?;
                self.box_primitive(source)?
            }
            _ => {
                self.ordinary_required(source)?;
                unreachable!()
            }
        };
        let length = self.binary_source_length(&source, host)?;
        if length > self.limits.heap.array_length {
            return Err(Fault::new(format!("Array.from length limit reached ({}).", self.limits.heap.array_length)));
        }
        let target = self.array_from_target(&constructor, Some(length), host)?;
        let source = self.ordinary_required(&source)?;
        for index in 0..length {
            self.step()?;
            let key = index.to_string();
            let value = self.get_ordinary_property_with_host(source, &key, host)?;
            let value = self.array_from_map(mapper, receiver, value, index, host)?;
            self.create_data_property(&target, PropertyKey::String(key), value, host)?;
        }
        self.array_from_length(&target, length, host)?;
        Ok(target)
    }

    fn object_from_entries(&mut self, source: &Value, host: &mut dyn Host) -> Result<Value, Fault> {
        Self::require_property_receiver(source)?;
        let target = Value::Object(self.new_object(None)?);
        let method = self.iterator_method(source, host)?.ok_or_else(|| {
            Fault::exception("TypeError", "Object.fromEntries input is not iterable.")
        })?;
        let source = self.copy_value(source)?;
        let mut iterator = self.iterator_from_method(source, method, host)?;
        while let Some(entry) = self.iterator_next(&mut iterator, host)? {
            let defined = (|| {
                let entry = self.ordinary_required(&entry)?;
                let key = self.get_ordinary_property_with_host(entry, "0", host)?;
                let value = self.get_ordinary_property_with_host(entry, "1", host)?;
                let key = self.property_key(key, host)?;
                self.create_data_property(&target, key, value, host)
            })();
            if let Err(error) = defined {
                return self.iterator_close(&mut iterator, Err(error), host);
            }
        }
        Ok(target)
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
    fn iterable_builtins_array_from_native_and_array_like_values() {
        succeeds(
            r#"
var sparse=Array(3);sparse[1]=4;
var dense=Array.from(sparse,function(v,i){return v===undefined?'hole'+i:v;});
if(dense.join('|')!=='hole0|4|hole2'||!Object.hasOwn(dense,'0'))throw 'holes';
if(Array.from('A😀').join('|')!=='A|😀')throw 'Unicode iterator';
if(Array.from(new Set([3,2,3])).join(',')!=='3,2')throw 'Set';
if(Array.from(new Map([['x',2],['y',3]])).map(function(e){return e.join(':');}).join(',')!=='x:2,y:3')throw 'Map';
if(Array.from(new Uint8Array([7,8])).join(',')!=='7,8')throw 'typed';
var receiver={base:5};var mapped=Array.from({0:2,2:4,length:3},function(v,i){if(this!==receiver)throw 'mapper receiver';return v===undefined?'empty':this.base+v+i;},receiver);
if(mapped.join('|')!=='7|empty|11')throw 'array-like';
if(Array.from(4).length||Array.from(false).length||Array.from(Symbol('s')).length)throw 'scalar';
if(Array.from.length!==1||Array.from.name!=='from'||Object.fromEntries.length!==1||Object.fromEntries.name!=='fromEntries')throw 'metadata';
var d=Object.getOwnPropertyDescriptor(Array,'from');
if(d.enumerable||!d.writable||!d.configurable)throw 'descriptor';
var errors=0;for(var x of [null,undefined]){try{Array.from(x);}catch(e){if(e.name==='TypeError')errors++;}}
if(errors!==2)throw 'nullish';

'ok';
"#,
        );
    }

    #[test]
    fn iterable_builtins_array_from_captures_method_before_constructor() {
        succeeds(
            r#"
var trace='',index=0,input={},iter={};
Object.defineProperty(iter,'next',{get:function(){trace+='N';return function(){trace+='X';var n=index++;return {get done(){trace+='D';return n>0;},get value(){trace+='V';return 6;}};};}});
Object.defineProperty(input,Symbol.iterator,{configurable:true,get:function(){trace+='I';return function(){trace+='M';if(this!==input)throw 'iterator receiver';return iter;};}});
function C(){trace+='C';Object.defineProperty(input,Symbol.iterator,{value:function(){throw 'new method';}});}
Object.defineProperty(C.prototype,'length',{set:function(n){trace+='L';this.total=n;}});
var result=Array.from.call(C,input,function(v,i){trace+='F';return v+i;});
if(trace!=='ICMNXDVFXDL'||result[0]!==6||result.total!==1||!(result instanceof C))throw trace;
var touched=0;var bad={get [Symbol.iterator](){touched++;throw 'read';}};
try{Array.from(bad,null);throw 'accepted mapper';}catch(e){if(e.name!=='TypeError')throw e;}
if(touched!==0)throw 'mapper validation order';

'ok';
"#,
        );
    }

    #[test]
    fn iterable_builtins_array_from_constructors_and_scalar_receivers() {
        succeeds(
            r#"
var trace='',input={0:'a',1:'b',get length(){trace+='L';return {[Symbol.toPrimitive]:function(h){trace+=h;return 2.8;}};}};
function C(n){trace+='C'+n;this.constructorLength=n;}
var result=Array.from.call(C,input);
if(trace!=='LnumberC2'||result.constructorLength!==2||result.length!==2||result[1]!=='b')throw trace;
function Bound(prefix,n){this.prefix=prefix;this.n=n;}
var bound=Bound.bind(null,'kept'),made=Array.from.call(bound,{length:2,0:7,1:8});
if(made.prefix!=='kept'||made.n!==2||made.length!==2||!(made instanceof Bound))throw 'bound constructor';
var arrow=()=>{};
if(!Array.isArray(Array.from.call(arrow,{length:1,0:4})))throw 'nonconstructor fallback';
Object.defineProperty(Number.prototype,'length',{get:function(){'use strict';if(typeof this!=='object'||this.valueOf()!==7)throw 'length receiver';return 1;}});
Object.defineProperty(Number.prototype,'0',{get:function(){'use strict';if(typeof this!=='object'||this.valueOf()!==7)throw 'index receiver';return 9;}});
if(Array.from(7).join()!=='9')throw 'boxed scalar';
var list=[1,2];list[Symbol.iterator]=null;
if(Array.from(list).join()!=='1,2')throw 'array-like fallback';

'ok';
"#,
        );
    }

    #[test]
    fn iterable_builtins_array_from_closes_only_active_consumers() {
        succeeds(
            r#"
var closed=0,marker={kind:'mapper'};
function make(mode){var n=0;var iter={next:function(){return {done:n++>0,value:5};},return:function(){closed++;if(mode==='throw')throw 'close';if(mode==='primitive')return 1;return {};}};return {[Symbol.iterator]:function(){return iter;}};}
for(var mode of ['normal','throw','primitive']){closed=0;try{Array.from(make(mode),function(){throw marker;});throw 'accepted mapping';}catch(e){if(e!==marker)throw 'exception precedence';}if(closed!==1)throw 'mapper close';}
closed=0;function Locked(){return Object.preventExtensions({});}
try{Array.from.call(Locked,make('normal'));throw 'defined locked';}catch(e){if(e.name!=='TypeError')throw e;}
if(closed!==1)throw 'definition close';
for(var phase of ['next','done','value']){closed=0;var iter={next:function(){if(phase==='next')throw marker;return {get done(){if(phase==='done')throw marker;return false;},get value(){throw marker;}};},return:function(){closed++;return {};}};
try{Array.from({[Symbol.iterator]:function(){return iter;}});throw 'accepted iterator';}catch(e){if(e!==marker)throw e;}if(closed!==0)throw 'step closed';}
closed=0;function LengthFail(){}Object.defineProperty(LengthFail.prototype,'length',{set:function(){throw marker;}});
try{Array.from.call(LengthFail,make('normal'));throw 'accepted length';}catch(e){if(e!==marker)throw e;}
if(closed!==0)throw 'completed iterator closed';

'ok';
"#,
        );
    }

    #[test]
    fn iterable_builtins_from_entries_reads_pairs_before_key_conversion() {
        succeeds(
            r#"
var trace='',key={[Symbol.toPrimitive]:function(h){trace+='K'+h;return 'answer';}},entry={get 0(){trace+='0';return key;},get 1(){trace+='1';return 7;}};
var setters=0;Object.defineProperty(Object.prototype,'answer',{set:function(){setters++;},configurable:true});
var symbol=Symbol('key'),result=Object.fromEntries([entry,['a',1],[symbol,5],['a',9]]);
if(trace!=='01Kstring'||setters!==0||result.answer!==7||result.a!==9||result[symbol]!==5)throw trace;
var d=Object.getOwnPropertyDescriptor(result,'answer');if(!d.enumerable||!d.writable||!d.configurable||d.get!==undefined)throw 'own data';
if(Object.keys(result).join()!=='answer,a'||Object.getOwnPropertySymbols(result)[0]!==symbol)throw 'keys';
function pair(){}pair[0]='function';pair[1]=11;
var ignored=false;var other=Object.fromEntries.call(function(){ignored=true;},[pair]);
if(ignored||other.function!==11||Object.getPrototypeOf(other)!==Object.prototype)throw 'generic entry or receiver';

'ok';
"#,
        );
    }

    #[test]
    fn iterable_builtins_from_entries_closes_entry_and_conversion_failures() {
        succeeds(
            r#"
var closed=0,marker={kind:'entry'},phase='primitive';
function make(entry){var once=false;return {[Symbol.iterator]:function(){return {next:function(){if(phase==='next')throw marker;return {done:once,get value(){once=true;return entry;}};},return:function(){closed++;throw 'close';}};}};}
try{Object.fromEntries(make(3));throw 'accepted primitive';}catch(e){if(e.name!=='TypeError')throw e;}
if(closed!==1)throw 'primitive close';
for(phase of ['zero','one','key']){closed=0;var entry={get 0(){if(phase==='zero')throw marker;return {[Symbol.toPrimitive]:function(){throw marker;}};},get 1(){if(phase==='one')throw marker;return 5;}};
try{Object.fromEntries(make(entry));throw 'accepted entry';}catch(e){if(e!==marker)throw 'entry precedence';}if(closed!==1)throw 'entry close';}
closed=0;phase='next';try{Object.fromEntries(make([]));throw 'accepted next';}catch(e){if(e!==marker)throw e;}
if(closed!==0)throw 'next closed';
var invalid=0;for(var value of [null,undefined,3,{},'x']){try{Object.fromEntries(value);}catch(e){if(e.name==='TypeError')invalid++;}}
if(invalid!==5)throw 'input validation';

'ok';
"#,
        );
    }

    #[test]
    fn iterable_builtins_scalar_descriptor_sources_have_no_own_keys() {
        succeeds(
            r#"
var calls=0;Object.defineProperty(Number.prototype,'x',{enumerable:true,get:function(){calls++;throw 'inherited descriptor';}});
var target={kept:4};
if(Object.defineProperties(target,5)!==target||Object.defineProperties(target,false)!==target||Object.defineProperties(target,Symbol('d'))!==target)throw 'identity';
var prototype={p:8};var created=Object.create(prototype,7);
if(Object.getPrototypeOf(created)!==prototype||Object.keys(created).length||calls||target.kept!==4)throw 'empty own sources';
var count=0;for(var bad of [null,undefined]){try{Object.defineProperties(target,bad);}catch(e){if(e.name==='TypeError')count++;}}
try{Object.defineProperties(3,5);}catch(e){if(e.name==='TypeError')count++;}
if(count!==3)throw 'object requirements';

'ok';
"#,
        );
    }

    #[test]
    fn iterable_builtins_keep_shared_limits_and_native_capability_gates() {
        for operation in ["Array.from", "Object.fromEntries"] {
            let mut runtime = Runtime::for_test();
            let source = format!(
                "var caught=false,finalized=false,closed=0;\
                 var pair=['repeat',1],step={{done:false,value:{}}};\
                 var input={{[Symbol.iterator]:function(){{return {{next:function(){{return step;}},\
                 return:function(){{closed++;return {{}};}}}};}}}};\
                 try{{{}(input);}}catch(e){{caught=true;}}finally{{finalized=true;}}",
                if operation == "Array.from" { "1" } else { "pair" },
                operation,
            );
            let error = runtime.execute(&source, &mut NoHost).unwrap_err();
            assert!(error.to_string().contains("limit"), "{error}");
            assert_eq!(runtime.call_depth, 0);
            assert!(runtime.objects.len() <= super::super::MAX_OBJECTS);
            assert!(runtime.properties <= super::super::MAX_PROPERTIES);
            runtime.begin_job();
            assert_eq!(
                runtime
                    .execute("caught+':'+finalized+':'+closed;", &mut NoHost)
                    .unwrap()
                    .to_text(),
                "false:false:0"
            );
        }
        let mut runtime = Runtime::for_test();
        let error = runtime
            .execute(
                "var entered=0;function C(){entered++;}Array.from.call(C,{length:8193});",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.to_string().contains("8192"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime.execute("entered;", &mut NoHost).unwrap().to_text(),
            "0"
        );

        struct GuardedHost(usize);
        impl Host for GuardedHost {
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                self.0 += 1;
                Ok(Value::Undefined)
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                self.0 += 1;
                Ok(())
            }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
                self.0 += 1;
                Ok(Value::Undefined)
            }
        }
        for operation in [Op::ArrayFrom, Op::ObjectFromEntries] {
            let mut runtime = Runtime::for_test();
            let mut host = GuardedHost(0);
            let error = runtime
                .iterable_builtin(operation, &[Value::Host(77)], Value::Undefined, &mut host)
                .unwrap_err();
            assert!(error.exception.is_none());
            assert_eq!(host.0, 0);
            assert_eq!(runtime.call_depth, 0);
        }
    }

    #[test]
    fn from_entries_proto_key_is_ordinary_own_data_without_changing_chain() {
        let value = Runtime::for_test().execute(r#"
var parent=Object.prototype, value={marker:1};
var object=Object.fromEntries([['__proto__',value]]);
var descriptor=Object.getOwnPropertyDescriptor(object,'__proto__');
if(Object.getPrototypeOf(object)!==parent||object.__proto__!==value||!Object.prototype.hasOwnProperty.call(object,'__proto__'))throw 'ordinary fromEntries key';
if(!descriptor.writable||!descriptor.enumerable||!descriptor.configurable||descriptor.value!==value||descriptor.get!==undefined)throw 'real data descriptor';
'from-entries-key-ok';
"#, &mut NoHost).unwrap();
        assert_eq!(value.to_text(), "from-entries-key-ok");
    }

    #[test]
    fn string_arraylike_fallback_retains_the_original_supported_program() {
        // Exact earlier rejection input, now enabled by String boxing.
        let mut runtime = Runtime::for_test();
        let Value::Object(array) = runtime
            .execute(
                "String.prototype[Symbol.iterator]=undefined;Array.from('x');",
                &mut NoHost,
            )
            .unwrap()
        else {
            panic!("Array.from must return an array");
        };
        assert_eq!(runtime.objects[array].length, Some(1));
        assert_eq!(runtime.own_property(array, "0").unwrap().to_text(), "x");
    }
}
