//! Original bounded primitive wrapper objects. Internal slots cannot be forged
//! by setting script properties or changing prototypes. String payloads and
//! virtual code-unit properties are managed by the string_objects module.
//!
//! https://tc39.es/ecma262/#sec-toobject
//! https://tc39.es/ecma262/#sec-number-constructor
//! https://tc39.es/ecma262/#sec-boolean-constructor

use super::{Fault, Host, NativeGetter, Ordinary, Runtime, Value};

#[derive(Clone)]
pub(super) enum Data {
    Number(f64),
    Boolean(bool),
    Symbol(usize),
    String { value: String, length: usize },
}

impl Data {
    fn value(&self) -> Value {
        match self {
            Self::Number(value) => Value::Number(*value),
            Self::Boolean(value) => Value::Bool(*value),
            Self::Symbol(value) => Value::Symbol(*value),
            Self::String { value, .. } => Value::String(value.clone()),
        }
    }

    pub(super) fn tag(&self) -> &'static str {
        match self {
            Self::Number(_) => "Number",
            Self::Boolean(_) => "Boolean",
            Self::Symbol(_) => "Symbol",
            Self::String { .. } => "String",
        }
    }
}

impl Runtime {
    pub(super) fn wrapped_data(&self, value: &Value) -> Option<&Data> {
        let Value::Object(id) = value else {
            return None;
        };
        self.objects.get(*id)?.primitive.as_ref()
    }

    pub(super) fn wrapped_primitive(&mut self, value: &Value) -> Result<Option<Value>, Fault> {
        let bytes = match self.wrapped_data(value) {
            Some(Data::String { value, .. }) => value.len(),
            _ => 0,
        };
        self.work(bytes)?;
        Ok(self.wrapped_data(value).map(Data::value))
    }

    pub(super) fn box_primitive(&mut self, value: Value) -> Result<Value, Fault> {
        let (primitive, prototype) = match value {
            Value::Number(value) => (Data::Number(value), self.number_prototype),
            Value::Bool(value) => (Data::Boolean(value), self.boolean_prototype),
            Value::Symbol(value) => {
                self.symbol(value)?;
                (Data::Symbol(value), self.symbol_prototype)
            }
            Value::String(value) => return self.box_string(value),
            _ => {
                return Err(Fault::exception(
                    "TypeError",
                    "Cannot convert this value to a primitive wrapper.",
                ));
            }
        };
        let object = self.new_object(None)?;
        self.objects[object].primitive = Some(primitive);
        self.objects[object].prototype = prototype.map(Ordinary::Object);
        Ok(Value::Object(object))
    }

    pub(super) fn assign_primitive_property(
        &mut self,
        receiver: Value,
        name: String,
        value: Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.property_name(&name)?;
        let prototype = match &receiver {
            Value::Number(_) => self.number_prototype,
            Value::Bool(_) => self.boolean_prototype,
            Value::Symbol(_) => self.symbol_prototype,
            Value::String(text) => {
                self.work(text.len())?;
                // These immutable own properties shadow prototype setters.
                if name == "length"
                    || super::property_index(&name)
                        .is_some_and(|index| index < text.encode_utf16().count())
                {
                    return if self.strict {
                        Err(Fault::exception(
                            "TypeError",
                            "The string property is read-only.",
                        ))
                    } else {
                        Ok(())
                    };
                }
                self.string_prototype
            }
            _ => return Err(Fault::new("Unsupported primitive property assignment.")),
        };
        if let Some(prototype) = prototype {
            if let Some(owner) = self.property_owner(Ordinary::Object(prototype), &name)? {
                let property = self.objects[owner].properties.get(&name);
                if let Some(setter) = property
                    .and_then(|property| property.accessor)
                    .and_then(|accessor| accessor.set)
                {
                    self.call_accessor(Some(setter), receiver, &[value], host)?;
                    return Ok(());
                }
                if property.is_some_and(|property| {
                    matches!(property.getter, Some(NativeGetter::ArgumentsCallee))
                }) {
                    return Err(Fault::exception(
                        "TypeError",
                        "Strict arguments callee access is forbidden.",
                    ));
                }
            }
        }
        // Ordinary Set cannot create an own property on a primitive receiver.
        // No temporary wrapper allocation or prototype mutation is necessary.
        if self.strict {
            Err(Fault::exception(
                "TypeError",
                "Cannot assign a property on a primitive receiver.",
            ))
        } else {
            Ok(())
        }
    }

    pub(super) fn primitive_value_method(
        &mut self,
        method: &str,
        number: bool,
        receiver: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        // Branding happens before any user-observable radix conversion.
        let receiver = self.wrapped_primitive(&receiver)?.unwrap_or(receiver);
        if !matches!(&receiver, Value::Number(_) if number)
            && !matches!(&receiver, Value::Bool(_) if !number)
        {
            return Err(Fault::exception(
                "TypeError",
                "Primitive value method has an incompatible receiver.",
            ));
        }
        if method == "valueOf" {
            return Ok(receiver);
        }
        // Boolean.toString ignores arguments. Number retains the documented
        // decimal-output subset, but observes ToIntegerOrInfinity on its radix.
        if number {
            let radix = match args.first() {
                None | Some(Value::Undefined) => 10.0,
                Some(value) => {
                    let radix = self.binary_number(value, host)?;
                    if radix.is_nan() {
                        0.0
                    } else {
                        radix.trunc()
                    }
                }
            };
            if !(2.0..=36.0).contains(&radix) {
                return Err(Fault::exception(
                    "RangeError",
                    "Number radix must be from 2 to 36.",
                ));
            }
            let simple =
                matches!(&receiver, Value::Number(value) if !value.is_finite() || *value == 0.0);
            if radix != 10.0 && !simple {
                return Err(Fault::new(
                    "Nondecimal Number string conversion is not implemented.",
                ));
            }
        }
        self.checked_value(Value::String(receiver.script_text()?))
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
            Runtime::new()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn wrappers_construct_scalar_slots_with_stable_intrinsic_prototypes() {
        succeeds(
            r#"
var n=new Number('12'),b=new Boolean(0),s=Symbol('value'),w=Object(s);
if(typeof n!=='object'||n.valueOf()!==12||n.toString()!=='12'||!(n instanceof Number)||!(n instanceof Object))throw 'Number wrapper';
if(typeof b!=='object'||b.valueOf()!==false||b.toString()!=='false'||!Boolean(b)||!(b instanceof Boolean))throw 'Boolean wrapper';
if(typeof w!=='object'||w.valueOf()!==s||!(w instanceof Symbol)||Object.getPrototypeOf(w)!==Symbol.prototype)throw 'Symbol wrapper';
if(Object(n)!==n||new Object(n)!==n||Object(3)===Object(3)||Object(false)===Object(false)||Object(s)===Object(s))throw 'identity';
if(new Object(4).valueOf()!==4||new Object(false).valueOf()!==false||new Object(s).valueOf()!==s)throw 'Object construction';
if(new Number().valueOf()!==0||1/new Number(-0).valueOf()!==-Infinity||new Boolean().valueOf()!==false)throw 'defaults';
if(Object.keys(n).length||Object.getOwnPropertyNames(n).length||Object.getOwnPropertySymbols(w).length)throw 'slots leaked as properties';
var bound=Number.bind(null,'17'),boundValue=new bound();
if(boundValue.valueOf()!==17||!(boundValue instanceof Number)||!(boundValue instanceof bound))throw 'bound constructor';
'ok';
"#,
        );
    }

    #[test]
    fn wrapper_brands_are_internal_and_prototype_slots_are_real() {
        succeeds(
            r#"
if(Number.prototype.valueOf()!==0||Boolean.prototype.valueOf()!==false)throw 'prototype slots';
if(Object.getPrototypeOf(4)!==Number.prototype||Object.getPrototypeOf(false)!==Boolean.prototype)throw 'primitive prototypes';
var n=Object(3),b=Object(true),s=Object(Symbol('tag')),fake=Object.create(Number.prototype),caught=0;
if(Object.prototype.toString.call(n)!=='[object Number]'||Object.prototype.toString.call(b)!=='[object Boolean]'||Object.prototype.toString.call(s)!=='[object Symbol]'||Object.prototype.toString.call(fake)!=='[object Object]')throw 'brand tags';
for(var value of [fake,{},b,Symbol(),null,undefined]){try{Number.prototype.valueOf.call(value);}catch(e){if(e.name==='TypeError')caught++;}}
try{Boolean.prototype.valueOf.call(n);}catch(e){if(e.name==='TypeError')caught++;}
try{Symbol.prototype.valueOf.call(Symbol.prototype);}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==8)throw 'forged brands';
n.value=99;Object.setPrototypeOf(n,null);
if(Number.prototype.valueOf.call(n)!==3||Object.prototype.toString.call(n)!=='[object Number]')throw 'slot survives prototype change';
if(Number.prototype.toString.length!==1||Boolean.prototype.toString.length!==0)throw 'method metadata';
'ok';
"#,
        );
    }

    #[test]
    fn wrappers_box_each_sloppy_call_and_preserve_strict_and_lexical_receivers() {
        succeeds(
            r#"
function receiver(){return this;}function strictReceiver(){'use strict';return this;}
var a=receiver.call(5),b=receiver.apply(5,[]),s=Symbol('receiver');
if(typeof a!=='object'||a===b||a.valueOf()!==5||b.valueOf()!==5)throw 'fresh number receivers';
if(receiver.call(false).valueOf()!==false||receiver.call(s).valueOf()!==s)throw 'other scalar receivers';
if(strictReceiver.call(5)!==5||strictReceiver.call(false)!==false||strictReceiver.call(s)!==s||strictReceiver.call(null)!==null)throw 'strict receivers';
var bound=receiver.bind(5),first=bound();if(bound()===first||first.valueOf()!==5)throw 'bound receiver freshness';
function make(){return ()=>this;}var closure=make.call(false),held=closure();
if(closure.call(8)!==held||held.valueOf()!==false)throw 'lexical boxed receiver';
Number.prototype.fixtureMethod=function(){this.extra=9;return this;};
var boxed=(3).fixtureMethod();delete Number.prototype.fixtureMethod;
if(boxed.valueOf()!==3||boxed.extra!==9||(3).extra!==undefined)throw 'method boxed receiver';
if(Object.prototype.valueOf.call(4).valueOf()!==4||Object.prototype.valueOf.call(false).valueOf()!==false||Object.prototype.valueOf.call(s).valueOf()!==s)throw 'generic valueOf boxing';
'ok';
"#,
        );
    }

    #[test]
    fn wrapper_expandos_descriptors_assign_and_integrity_use_ordinary_storage() {
        succeeds(
            r#"
var key=Symbol('field'),n=Object(6);n.x=1;n[key]=2;
Object.defineProperty(n,'hidden',{value:3});Object.defineProperty(n,'twice',{get:function(){return this.valueOf()*2;},enumerable:true});
if(Object.keys(n).join(',')!=='x,twice'||Object.getOwnPropertyNames(n).join(',')!=='x,hidden,twice'||Object.getOwnPropertySymbols(n)[0]!==key)throw 'own storage';
if(Object.getOwnPropertyDescriptor(n,'hidden').writable||n.twice!==12)throw 'descriptors';
Object.freeze(n);n.x=8;if(n.x!==1||n.valueOf()!==6||!Object.isFrozen(n))throw 'integrity';
var target=Object.assign(4,null,5,true,Symbol(),{a:7,[key]:9});
if(target.valueOf()!==4||target.a!==7||target[key]!==9||Object.getPrototypeOf(target)!==Number.prototype)throw 'assign boxed target';
var copied=Object.assign({},target);if(copied.a!==7||copied[key]!==9)throw 'assign boxed source';
var hits=0,name={toString:function(){hits++;return 'x';}};
if(Object.getOwnPropertyDescriptor(1,name)!==undefined||Object.getOwnPropertyDescriptor(false,name)!==undefined||Object.getOwnPropertyDescriptor(Symbol(),name)!==undefined||hits!==3)throw 'temporary descriptor inspection';
var caught=0;try{Object.defineProperty(1,'x',{value:2});}catch(e){if(e.name==='TypeError')caught++;}
try{Object.assign(null,{x:2});}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2)throw 'invalid destinations';
'ok';
"#,
        );
    }

    #[test]
    fn primitive_assignments_observe_setters_and_strict_failures_without_mutation() {
        succeeds(
            r#"
var n=3,b=false,s=Symbol('write'),caught=0;n.x=1;b.x=1;s.x=1;
if(n.x!==undefined||b.x!==undefined||s.x!==undefined)throw 'primitive retained expando';
for(var value of [n,b,s]){try{(function(){'use strict';this.x=1;}).call(value);}catch(e){if(e.name==='TypeError')caught++;}}
if(caught!==3)throw 'strict primitive write';
var seen,received;
Object.defineProperty(Number.prototype,'fixtureSetter',{set:function(value){'use strict';seen=this;received=value;},configurable:true});
n.fixtureSetter=7;delete Number.prototype.fixtureSetter;
if(seen!==3||received!==7)throw 'setter original primitive';
Object.defineProperty(Boolean.prototype,'fixtureSetter',{set:function(value){seen=this;this.extra=value;},configurable:true});
b.fixtureSetter=8;delete Boolean.prototype.fixtureSetter;
if(typeof seen!=='object'||seen.valueOf()!==false||seen.extra!==8||b.extra!==undefined)throw 'setter sloppy wrapper';
Object.defineProperty(Number.prototype,'fixtureData',{value:4,writable:true,configurable:true});
n.fixtureData=9;if(Number.prototype.fixtureData!==4||n.fixtureData!==4)throw 'prototype data mutation';
delete Number.prototype.fixtureData;
'ok';
"#,
        );
    }

    #[test]
    fn wrapper_conversions_keep_valueof_overrides_boolean_truthiness_and_order() {
        succeeds(
            r#"
var calls=0,marker={},n=new Number({valueOf:function(){calls++;return 8;}});
if(calls!==1||n.valueOf()!==8)throw 'constructor conversion';
var b=new Boolean({valueOf:function(){throw 'Boolean conversion';}});
if(b.valueOf()!==true||Number(Object(false))!==0||String(Object(true))!=='true'||Number(Object(7))!==7)throw 'wrapper conversions';
n.valueOf=function(){return 9;};n.toString=function(){return 'text';};
if(Number(n)!==9||String(n)!=='text'||n+1!==10)throw 'ordinary override ordering';
var caught=false;try{new Number({valueOf:function(){throw marker;}});}catch(e){caught=e===marker;}
if(!caught)throw 'constructor exception';
calls=0;var radix={valueOf:function(){calls++;return 10.9;}};
if(Number.prototype.toString.call(Object(12),radix)!=='12'||calls!==1)throw 'decimal radix conversion';
calls=0;if(Boolean.prototype.toString.call(Object(false),radix)!=='false'||calls!==0)throw 'Boolean ignores arguments';
try{Number.prototype.toString.call({},radix);}catch(e){if(e.name!=='TypeError')throw e;}
if(calls!==0)throw 'brand before radix';
var failures=0;for(var value of [0,1,37,NaN,Infinity,null]){try{Number.prototype.toString.call(n,value);}catch(e){if(e.name==='RangeError')failures++;}}
if(failures!==6||Number.prototype.toString.call(n,undefined)!=='8')throw 'radix validation';
if(Number.prototype.toString.call(Object(NaN),16)!=='NaN'||Number.prototype.toString.call(Object(Infinity),2)!=='Infinity'||Number.prototype.toString.call(Object(-Infinity),36)!=='-Infinity'||Number.prototype.toString.call(Object(-0),2)!=='0')throw 'radix independent values';
'ok';
"#,
        );
    }

    #[test]
    fn wrapper_objects_keep_existing_arena_and_string_limit_boundaries() {
        let mut runtime = Runtime::new();
        for source in [
            "new String('x');",
            "Object('x');",
            "(function(){return this;}).call('x');",
        ] {
            runtime.begin_job();
            let value = runtime.execute(source, &mut NoHost).unwrap();
            assert!(
                matches!(runtime.wrapped_primitive(&value).unwrap(), Some(Value::String(value)) if value == "x")
            );
        }
        let before = runtime.value_bytes;
        while runtime.objects.len() < super::super::MAX_OBJECTS {
            runtime.box_primitive(Value::Number(7.0)).unwrap();
        }
        assert_eq!(runtime.value_bytes, before);
        let error = runtime.box_primitive(Value::Bool(false)).unwrap_err();
        assert!(error.message.contains("Object identity limit"));
        assert_eq!(runtime.objects.len(), super::super::MAX_OBJECTS);
    }
}
