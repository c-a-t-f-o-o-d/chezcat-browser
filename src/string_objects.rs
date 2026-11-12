//! Bounded original String exotic objects.
//!
//! The immutable payload owns one UTF-8 string, charged to retained values.
//! Indexed own properties are virtual; boxing a long string does not allocate
//! thousands of property slots. Length and enumeration count UTF-16 code units.
//! Reading an isolated surrogate retains the existing explicit string boundary.
//! https://tc39.es/ecma262/#sec-string-exotic-objects
//! https://tc39.es/ecma262/#sec-string-constructor

use super::{primitive_wrappers::Data, Fault, Host, Ordinary, Runtime, Value, WorkKind};

impl Runtime {
    pub(super) fn string_object_length(&self, id: usize) -> Option<usize> {
        match self.objects.get(id)?.primitive.as_ref()? {
            Data::String { length, .. } => Some(*length),
            _ => None,
        }
    }

    pub(super) fn string_object_index(&self, id: usize, name: &str) -> Option<usize> {
        let index = super::property_index(name)?;
        (index < self.string_object_length(id)?).then_some(index)
    }

    pub(super) fn string_object_unit(&mut self, id: usize, index: usize) -> Result<u16, Fault> {
        let Some(Data::String { value, length }) = self.objects[id].primitive.as_ref() else {
            return Err(Fault::new("Invalid String object slot."));
        };
        if index >= *length {
            return Err(Fault::new("Invalid String object index."));
        }
        let bytes = value.len();
        self.work_as(bytes, WorkKind::CharacterScan)?;
        let Some(Data::String { value, .. }) = self.objects[id].primitive.as_ref() else {
            unreachable!()
        };
        Ok(value.encode_utf16().nth(index).unwrap())
    }

    pub(super) fn string_object_index_value(
        &mut self,
        id: usize,
        index: usize,
    ) -> Result<Value, Fault> {
        let unit = self.string_object_unit(id, index)?;
        self.units_string(&[unit]).map(Value::String)
    }

    pub(super) fn string_object_index_matches(
        &mut self,
        id: usize,
        index: usize,
        candidate: &Value,
    ) -> Result<bool, Fault> {
        // Descriptor compatibility does not coerce its value, and does not
        // need to materialize an unsupported isolated surrogate.
        let Value::String(candidate) = candidate else {
            return Ok(false);
        };
        self.work(candidate.len())?;
        let mut units = candidate.encode_utf16();
        let first = units.next();
        if first.is_none() || units.next().is_some() {
            return Ok(false);
        }
        Ok(first == Some(self.string_object_unit(id, index)?))
    }

    pub(super) fn initialize_string_object(
        &mut self,
        id: usize,
        value: String,
    ) -> Result<(), Fault> {
        if value.len() > super::MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        if self
            .value_bytes
            .saturating_add(value.len())
            .saturating_add("length".len())
            > super::MAX_RETAINED_VALUES
        {
            return Err(Fault::new("Retained value limit reached (2 MiB)."));
        }
        self.work_as(value.len(), WorkKind::LengthScan)?;
        let length = value.encode_utf16().count();
        self.store_property(
            id,
            "length".into(),
            Value::Number(length as f64),
            Some((false, false, false)),
        )?;
        self.value_bytes += value.len();
        self.objects[id].primitive = Some(Data::String { value, length });
        Ok(())
    }

    pub(super) fn box_string(&mut self, value: String) -> Result<Value, Fault> {
        // Validate before allocating an object; no partial object is exposed.
        if value.len() > super::MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        if self
            .value_bytes
            .saturating_add(value.len())
            .saturating_add("length".len())
            > super::MAX_RETAINED_VALUES
        {
            return Err(Fault::new("Retained value limit reached (2 MiB)."));
        }
        let id = self.new_object(None)?;
        self.initialize_string_object(id, value)?;
        self.objects[id].prototype = self.string_prototype.map(Ordinary::Object);
        Ok(Value::Object(id))
    }

    pub(super) fn construct_string(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = if let Some(value) = args.first() {
            let value = self.copy_value(value)?;
            // Only String() as a function has the Symbol description exception.
            let primitive = self.primitive_with_hint(value, true, host)?;
            self.work_as(primitive.bytes(), WorkKind::ConversionOutput)?;
            primitive.script_text()?
        } else {
            String::new()
        };
        self.box_string(value)
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
    fn string_slots_are_unforgeable_and_constructor_conversion_is_ordered() {
        succeeds(
            r#"
let b=new String('abc');
if(typeof b!=='object'||b.valueOf()!=='abc'||b.toString()!=='abc'||!(b instanceof String))throw 'boxing';
if(Object(b)!==b||new Object(b)!==b||Object('a')===Object('a'))throw 'identity';
if(String.prototype.valueOf()!==''||String.prototype.length!==0)throw 'prototype';
let log='',v={toString(){log+='s';return {};},valueOf(){log+='v';return 17;}};
if(new String(v).valueOf()!=='17'||log!=='sv')throw 'conversion';
let rejected=0;try{new String(Symbol('x'));}catch(e){if(e.name==='TypeError')rejected++;}
try{String.prototype.valueOf.call(Object.create(String.prototype));}catch(e){if(e.name==='TypeError')rejected++;}
if(rejected!==2||String(Symbol('x'))!=='Symbol(x)')throw 'branding';
let B=String.bind(null,'bound');if(new B().valueOf()!=='bound')throw 'bound constructor';
'ok';
"#,
        );
    }

    #[test]
    fn virtual_string_indices_have_exact_immutable_descriptors() {
        succeeds(
            r#"
let b=new String('ab'),d=Object.getOwnPropertyDescriptor(b,0),l=Object.getOwnPropertyDescriptor(b,'length');
if(d.value!=='a'||d.writable||!d.enumerable||d.configurable||'get' in d)throw 'index descriptor';
if(l.value!==2||l.writable||l.enumerable||l.configurable)throw 'length descriptor';
Object.defineProperty(b,0,{value:'a',writable:false,enumerable:true,configurable:false});
let n=0;for(let d of [{value:'z'},{writable:true},{enumerable:false},{configurable:true},{get(){return 'a';}}]){try{Object.defineProperty(b,0,d);}catch(e){if(e.name==='TypeError')n++;}}
if(n!==5||b[0]!=='a'||delete b[0])throw 'index mutation';
b[0]='z';b.length=10;if(b[0]!=='a'||b.length!==2)throw 'sloppy writes';
try{(function(){'use strict';b[0]='z';})();throw 'accepted';}catch(e){if(e.name!=='TypeError')throw e;}
'ok';
"#,
        );
    }

    #[test]
    fn string_indices_enumerate_before_other_keys_and_shadow_prototypes() {
        succeeds(
            r#"
let b=new String('ab');b[3]='d';b['01']='leading';b.extra=7;let s=Symbol('s');b[s]=8;
if(Object.getOwnPropertyNames(b).join()!=='0,1,3,length,01,extra')throw 'names';
if(Object.keys(b).join()!=='0,1,3,01,extra'||Object.getOwnPropertySymbols(b)[0]!==s)throw 'keys';
let child=Object.create(b);child[0]='z';if(child[0]!=='a'||child.hasOwnProperty(0))throw 'inherited immutable index';
let keys=[];for(let k in child)keys.push(k);if(keys.join()!=='0,1,3,01,extra')throw 'for-in';
if(JSON.stringify({...b})!=='{"0":"a","1":"b","3":"d","01":"leading","extra":7}')throw 'spread';
'ok';
"#,
        );
    }

    #[test]
    fn astral_string_objects_support_length_iteration_and_descriptor_validation() {
        succeeds(
            r#"
let b=new String('A😀Z');
if(b.length!==4||b[0]!=='A'||b[3]!=='Z'||b.valueOf()!=='A😀Z')throw 'UTF-16 length';
if(b.charCodeAt(1)!==55357||b.charCodeAt(2)!==56832||Array.from(b).join('|')!=='A|😀|Z')throw 'Unicode methods';
if(Object.getOwnPropertyNames(b).join()!=='0,1,2,3,length')throw 'virtual keys';
Object.defineProperty(b,1,{writable:false,enumerable:true,configurable:false});
try{Object.defineProperty(b,1,{value:'x'});throw 'accepted';}catch(e){if(e.name!=='TypeError')throw e;}
Object.preventExtensions(b);if(!Object.isFrozen(b))throw 'immutable virtual indices';
'ok';
"#,
        );
    }

    #[test]
    fn string_wrappers_unbox_after_json_hooks_and_observe_overrides() {
        succeeds(
            r#"
let b=new String('raw'),log='';b.toJSON=function(k){log+='j'+k;return this;};b.toString=function(){log+='s';return 'changed';};
let result=JSON.stringify({a:b},function(k,v){if(k==='a')log+='r';return v;});
if(result!=='{"a":"changed"}'||log!=='jars')throw 'JSON hook order';
let key=new String('a');key.toString=function(){return 'b';};
if(JSON.stringify({a:1,b:2},[key])!=='{"b":2}')throw 'property list';
let gap=new String('..');gap.toString=function(){return '--';};
if(JSON.stringify({a:1},null,gap)!=='{\n--"a": 1\n}')throw 'indentation';
'ok';
"#,
        );
    }

    #[test]
    fn primitive_string_to_object_and_sloppy_this_are_supported() {
        succeeds(
            r#"
if(Object.keys('ab').join()!=='0,1'||Object.getOwnPropertyDescriptor('ab',1).value!=='b')throw 'Object APIs';
if(JSON.stringify(Object.assign({},'ab'))!=='{"0":"a","1":"b"}')throw 'assign source';
let assigned=Object.assign('ab',{x:3});if(assigned.valueOf()!=='ab'||assigned.x!==3)throw 'assign target';
if((function(){return this instanceof String&&this.valueOf()==='ab';}).call('ab')!==true)throw 'sloppy receiver';
if((function(){'use strict';return typeof this;}).call('ab')!=='string')throw 'strict receiver';
let log='';Object.defineProperty(String.prototype,'field',{set(v){'use strict';log+=typeof this+':'+v;}});'ab'.field=3;
if(log!=='string:3')throw 'primitive inherited setter';
'ok';
"#,
        );
    }

    #[test]
    fn string_payload_accounting_is_exact_and_indices_are_not_allocated() {
        let mut runtime = Runtime::for_test();
        let before = runtime.value_bytes;
        let properties = runtime.properties;
        let Value::Object(id) = runtime.box_string("x".repeat(65536)).unwrap() else {
            unreachable!()
        };
        assert_eq!(runtime.value_bytes, before + 65536 + "length".len());
        assert_eq!(runtime.properties, properties + 1);
        assert_eq!(runtime.objects[id].properties.len(), 1);
        assert_eq!(runtime.string_object_length(id), Some(65536));
        assert_eq!(runtime.own_property(id, "65535").unwrap().to_text(), "x");
        assert!(runtime
            .own_string_keys(id, true)
            .unwrap_err()
            .message
            .contains("enumeration limit"));
        runtime.value_bytes = super::super::MAX_RETAINED_VALUES;
        let objects = runtime.objects.len();
        assert!(runtime
            .box_string("a".into())
            .unwrap_err()
            .message
            .contains("Retained value limit"));
        assert_eq!(runtime.objects.len(), objects);
        runtime.value_bytes = super::super::MAX_RETAINED_VALUES - 8;
        assert!(runtime
            .box_string("four".into())
            .unwrap_err()
            .message
            .contains("Retained value limit"));
        assert_eq!(runtime.objects.len(), objects);
        assert_eq!(runtime.value_bytes, super::super::MAX_RETAINED_VALUES - 8);
        runtime.value_bytes = super::super::MAX_RETAINED_VALUES - 10;
        runtime.box_string("four".into()).unwrap();
        assert_eq!(runtime.value_bytes, super::super::MAX_RETAINED_VALUES);
    }

    #[test]
    fn string_object_resource_faults_and_surrogate_faults_remain_fatal() {
        for source in [
            "try{new String('😀')[0];}catch(e){throw 'caught';}finally{throw 'finally';}",
            "let b=new String('x');try{for(var i=0;i<5000;i++)new String('x');}catch(e){throw 'caught';}finally{throw 'finally';}",
        ] {
            let error=Runtime::for_test().execute(source,&mut NoHost).unwrap_err();
            let diagnostic=error.lines().next().unwrap();
            assert!(!diagnostic.contains("caught")&&!diagnostic.contains("finally"), "{error}");
            assert!(error.contains("lone UTF-16")||error.contains("Object identity limit"), "{error}");
        }
        let mut runtime = Runtime::for_test();
        let boxed = runtime.box_string("work".into()).unwrap();
        runtime.copy_bytes = 0;
        assert!(runtime
            .wrapped_primitive(&boxed)
            .unwrap_err()
            .message
            .contains("work limit"));
    }

    #[test]
    fn string_payload_retention_remains_bounded_across_fresh_jobs() {
        let mut runtime = Runtime::for_test();
        let mut failures = 0;
        for _ in 0..40 {
            runtime.begin_job();
            match runtime.box_string("x".repeat(65536)) {
                Ok(_) => assert!(runtime.value_bytes <= super::super::MAX_RETAINED_VALUES),
                Err(error) => {
                    assert!(error.message.contains("Retained value limit"));
                    failures += 1;
                    break;
                }
            }
        }
        assert_eq!(failures, 1);
    }

    #[test]
    fn primitive_string_symbol_writes_and_descriptor_sources_use_to_object() {
        succeeds(
            r#"
let key=Symbol('key'),log='';
Object.defineProperty(String.prototype,key,{set(v){'use strict';log+=typeof this+':'+this+':'+v;},configurable:true});
'ab'[key]=7;if(log!=='string:ab:7')throw 'symbol setter';
delete String.prototype[key];'ab'[key]=8;if('ab'[key]!==undefined)throw 'primitive symbol retention';
try{(function(){'use strict';'ab'[key]=8;})();throw 'accepted';}catch(e){if(e.name!=='TypeError')throw e;}
let boxed=new String('ab');if(Object.defineProperties(boxed,'')!==boxed)throw 'empty descriptors';
let created=Object.create(null,'');if(Object.getPrototypeOf(created)!==null||Object.keys(created).length)throw 'empty creation descriptors';
try{Object.defineProperties(boxed,'x');throw 'accepted';}catch(e){if(e.name!=='TypeError')throw e;}
if(!Object.is(NaN,NaN)||Object.is(0,-0)||!Object.is(-0,-0)||Object.is({},{}))throw 'SameValue';
'ok';
"#,
        );
    }
}
