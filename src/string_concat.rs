//! Original bounded String.prototype.concat. Receiver coercion is performed
//! once by the existing StringMethod builtin route before entering this helper.
//! https://262.ecma-international.org/16.0/#sec-string.prototype.concat
use super::{Fault, Host, Runtime, Value, WorkKind};

impl Runtime {
    pub(super) fn string_concat(
        &mut self,
        receiver: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let mut output = String::new();
        self.append_string(&mut output, receiver)?;
        self.string_primitive_concat(Value::String(output), args, host)
    }

    pub(super) fn string_primitive_concat(
        &mut self, mut output: Value, args: &[Value], host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        for argument in args {
            // Keep each conversion live and ordered: callbacks can replace a
            // later hook, reenter concat, or throw. Existing conversion paths
            // retain native capability checks, call roots, realms and limits.
            let argument = self.copy_value(argument)?;
            let primitive = self.primitive_with_hint(argument, true, host)?;
            self.work_as(primitive.bytes(), WorkKind::ConversionOutput)?;
            if let Value::String(ref mut text) = output {
                if !matches!(primitive, Value::StringUnits(_)) {
                    self.append_string(text, &primitive.script_text()?)?;
                    continue;
                }
                output = Value::StringUnits(self.string_units_for_value(&output)?);
            }
            let Value::StringUnits(ref mut units) = output else { unreachable!() };
            let more = self.string_units_for_value(&primitive)?;
            if units.len().saturating_add(more.len()) > self.limits.capacity.string_bytes {
                return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
            }
            self.work_as(more.len().saturating_mul(2), WorkKind::ConcatenationScratch)?;
            units.try_reserve(more.len()).map_err(|_| Fault::new("String storage allocation failed."))?;
            units.extend(more);
        }
        if let Value::StringUnits(units) = output {
            self.work_as(units.len().saturating_mul(2), WorkKind::ConversionOutput)?;
            output = super::string_units::canonical(units, self.limits.capacity.string_bytes)?;
        }
        self.checked_value(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct NoHost { gets: usize, sets: usize, calls: usize }
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            self.gets += 1; Err("No native read is allowed.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            self.sets += 1; Err("No native write is allowed.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            self.calls += 1; Err("No native call is allowed.".into())
        }
    }
    fn succeeds(source: &str) {
        Runtime::for_test().execute(source, &mut NoHost::default()).unwrap();
    }

    #[test]
    fn string_concat_uses_real_generic_receivers_varargs_and_primitive_results() {
        succeeds(r#"
var concat=String.prototype.concat;
if(''.concat()!==''||'seed'.concat()!=='seed'||typeof 'seed'.concat()!=='string')throw 'zero arguments';
if('a'.concat('b','c',1,true,null,undefined)!=='abc1truenullundefined')throw 'varargs';
if(concat.call(12,3)!=='123'||concat.call(false,'!')!=='false!')throw 'primitive receiver';
if(concat.call(new String('box'),':','value')!=='box:value')throw 'boxed receiver';
if(concat.call({toString:function(){return 'object';}},'!')!=='object!')throw 'ordinary receiver';
if('array:'.concat([1,2])!=='array:1,2')throw 'ordinary argument conversion';
var unicode='é'.concat('😀','Z');
if(unicode!=='é😀Z'||unicode.length!==4)throw 'Unicode code units';
"#);
    }

    #[test]
    fn string_concat_evaluates_arguments_then_converts_receiver_and_each_argument_once() {
        succeeds(r#"
var concat=String.prototype.concat,order='',receiverGets=0,firstGets=0,secondGets=0;
var receiver={},first={},second={};
Object.defineProperty(receiver,Symbol.toPrimitive,{get:function(){receiverGets++;return function(h){order+='receiver:'+h+';';return 'R';};}});
Object.defineProperty(first,Symbol.toPrimitive,{get:function(){firstGets++;return function(h){order+='first:'+h+';';return 'A';};}});
Object.defineProperty(second,Symbol.toPrimitive,{get:function(){secondGets++;return function(h){order+='second:'+h+';';return 'B';};}});
function evaluated(name,value){order+='eval:'+name+';';return value;}
var result=concat.call(receiver,evaluated('first',first),evaluated('second',second));
if(result!=='RAB'||order!=='eval:first;eval:second;receiver:string;first:string;second:string;')throw order;
if(receiverGets!==1||firstGets!==1||secondGets!==1)throw 'hook read count';
"#);
    }

    #[test]
    fn string_concat_reentry_keeps_outer_prefix_and_observes_later_hook_replacement() {
        succeeds(r#"
var concat=String.prototype.concat,order='',second={toString:function(){throw 'cached later hook';}};
var first={toString:function(){
    order+='first;';
    var nested=concat.call({toString:function(){order+='nested;';return '(';}},')');
    if(nested!=='()')throw 'nested result';
    second.toString=function(){order+='replacement;';return 'B';};
    String.prototype.concat=function(){return 'overridden';};
    return 'A';
}};
var result=concat.call('outer:',first,second);
if(result!=='outer:AB'||order!=='first;nested;replacement;')throw 'outer or hook order';
if('x'.concat('y')!=='overridden'||concat.call('x','y')!=='xy')throw 'saved native identity';
String.prototype.concat=concat;
if('x'.concat('y')!=='xy')throw 'restored native';
"#);
    }

    #[test]
    fn string_concat_preserves_abrupt_identity_and_stops_later_conversions() {
        succeeds(r#"
var concat=String.prototype.concat,conversions=0,marker={},same=false;
var probe={toString:function(){conversions++;return 'probe';}};
for(var receiver of [null,undefined,Symbol('receiver')]){
    var caught=false;try{concat.call(receiver,probe);}catch(error){caught=error.name==='TypeError';}
    if(!caught||conversions!==0)throw 'invalid receiver converted an argument';
}
var brokenReceiver={toString:function(){throw marker;}};
try{concat.call(brokenReceiver,probe);}catch(error){same=error===marker;}
if(!same||conversions!==0)throw 'receiver throw identity';
var broken={toString:function(){throw marker;}};same=false;
try{concat.call('',broken,probe);}catch(error){same=error===marker;}
if(!same||conversions!==0)throw 'argument throw identity';
for(var argument of [Symbol('argument'),{[Symbol.toPrimitive]:function(){return Symbol('returned');}}]){
    var caught=false;try{concat.call('',argument,probe);}catch(error){caught=error.name==='TypeError';}
    if(!caught||conversions!==0)throw 'Symbol argument converted a later value';
}
var failed=false;try{concat.call({toString:function(){return {};},valueOf:function(){return {};}},probe);}catch(error){failed=error.name==='TypeError';}
if(!failed||conversions!==0)throw 'nonprimitive receiver';
"#);
    }

    #[test]
    fn string_concat_has_native_metadata_and_obeys_real_prototype_property_changes() {
        succeeds(r#"
var concat=String.prototype.concat,descriptor=Object.getOwnPropertyDescriptor(String.prototype,'concat');
if(concat!==String.prototype.concat||concat.name!=='concat'||concat.length!==1||descriptor.value!==concat||descriptor.enumerable||!descriptor.writable||!descriptor.configurable)throw 'native metadata';
if(Object.hasOwn(concat,'prototype')||Function.prototype.toString.call(concat).indexOf('[native code]')<0)throw 'native identity';
var caught=false;try{new concat();}catch(error){caught=error.name==='TypeError';}if(!caught)throw 'constructible concat';
String.prototype.concat=function(){return 'replacement';};if('a'.concat('b')!=='replacement')throw 'replacement';
if(!delete String.prototype.concat||'concat' in String.prototype||'a'.concat!==undefined)throw 'deletion';
String.prototype.concat=concat;if('a'.concat('b')!=='ab')throw 'restore';
"#);
    }

    #[test]
    fn string_concat_native_conversion_denial_precedes_hooks_and_remains_fatal() {
        for (source, expected_before) in [
            ("try{String.prototype.concat.call(native,probe);}catch(error){caught=true;}finally{finalized=true;}", 0),
            ("try{''.concat(probe,native,laterProbe);}catch(error){caught=true;}finally{finalized=true;}", 1),
        ] {
            let mut runtime = Runtime::for_test();
            let mut host = NoHost::default();
            runtime.define_global("native", Value::Host(7)).unwrap();
            runtime.execute("var before=0,later=0,caught=false,finalized=false;var probe={toString:function(){before++;return 'before';}},laterProbe={toString:function(){later++;return 'later';}};", &mut host).unwrap();
            let error = runtime.execute(source, &mut host).unwrap_err();
            assert!(error.contains("No conversion permission or hook read was granted"), "{error}");
            assert_eq!((host.gets, host.sets, host.calls), (0, 0, 0));
            runtime.begin_job();
            assert_eq!(runtime.execute("before;", &mut host).unwrap().to_text(), expected_before.to_string());
            assert_eq!(runtime.execute("later===0&&!caught&&!finalized;", &mut host).unwrap().to_text(), "true");
            assert_eq!(runtime.execute("'recovered'.concat(1);", &mut host).unwrap().to_text(), "recovered1");
        }
    }

    #[test]
    fn string_concat_keeps_byte_caps_copy_work_steps_and_fresh_job_recovery() {
        let mut runtime = Runtime::for_test();
        let mut host = NoHost::default();
        let string_limit = runtime.limits.capacity.string_bytes;
        runtime.limits.capacity.string_bytes = 4;
        assert_eq!(runtime.string_concat("é", &[Value::String("é".into())], &mut host).unwrap().to_text(), "éé");
        let error = runtime.string_concat("é", &[Value::String("é!".into())], &mut host).unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.message.contains("String limit reached"), "{}", error.message);
        runtime.limits.capacity.string_bytes = string_limit;
        runtime.begin_job();
        runtime.constrain_job(usize::MAX, 0, usize::MAX, Instant::now() + Duration::from_secs(10));
        let error = runtime.string_concat("abc", &[], &mut host).unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.message.contains("String/copy work limit"), "{}", error.message);
        assert_eq!(runtime.copy_bytes, 0);
        runtime.begin_job();
        runtime.constrain_job(usize::MAX, 96, usize::MAX, Instant::now() + Duration::from_secs(10));
        let mut successful_calls = 0;
        let mut exhausted = false;
        for _ in 0..64 {
            match runtime.string_concat("a", &[Value::String("b".into())], &mut host) {
                Ok(value) => { assert_eq!(value.to_text(), "ab"); successful_calls += 1; }
                Err(error) => {
                    assert!(error.exception.is_none());
                    assert!(error.message.contains("String/copy work limit"), "{}", error.message);
                    exhausted = true;
                    break;
                }
            }
        }
        assert!(exhausted && successful_calls > 0 && successful_calls < 64, "Many small calls must share the same finite allowance");
        assert_eq!(runtime.copy_bytes, 0);
        runtime.begin_job();
        let empty_arguments = vec![Value::String(String::new()); 32];
        runtime.steps = 4;
        let error = runtime.string_concat("", &empty_arguments, &mut host).unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.message.contains("Execution step limit"), "{}", error.message);
        runtime.begin_job();
        assert_eq!(runtime.execute("'é'.concat('😀','Z');", &mut host).unwrap().to_text(), "é😀Z");
        let value = runtime.execute(r#"'\ud800'.concat('x');"#, &mut host).unwrap();
        assert!(matches!(value, Value::StringUnits(units) if units == [0xd800, 120]));
        runtime.begin_job();
        assert_eq!(runtime.execute("'ok'.concat('again');", &mut host).unwrap().to_text(), "okagain");
    }
}
