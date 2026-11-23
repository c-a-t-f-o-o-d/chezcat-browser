//! Bounded original String exotic objects.
//!
//! The immutable payload owns an exact scalar or raw UTF-16 string, charged to retained values.
//! Indexed own properties are virtual; boxing a long string does not allocate
//! thousands of property slots. Length and enumeration count UTF-16 code units.
//! Indexed values preserve individual code units, including unpaired surrogates.
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
        let bytes = value.bytes();
        self.work_as(bytes, WorkKind::CharacterScan)?;
        let Some(Data::String { value, .. }) = self.objects[id].primitive.as_ref() else {
            unreachable!()
        };
        match value {
            Value::String(text)=>Ok(text.encode_utf16().nth(index).unwrap()),
            Value::StringUnits(units)=>Ok(units[index]),
            _=>unreachable!("String slot contains a non-string"),
        }
    }

    pub(super) fn string_object_index_value(
        &mut self,
        id: usize,
        index: usize,
    ) -> Result<Value, Fault> {
        let unit = self.string_object_unit(id, index)?;
        self.string_from_units(&[unit])
    }

    pub(super) fn string_object_index_matches(
        &mut self,
        id: usize,
        index: usize,
        candidate: &Value,
    ) -> Result<bool, Fault> {
        // Immutable descriptor compatibility compares the exact single unit
        // without coercion or materializing a temporary indexed value.
        if !candidate.is_string(){return Ok(false);}
        self.work(candidate.bytes())?;
        let unit=match candidate {
            Value::String(text)=>{
                let mut units=text.encode_utf16();let first=units.next();
                if units.next().is_some(){return Ok(false);}first
            },
            Value::StringUnits(units) if units.len()==1=>Some(units[0]),
            _=>None,
        };
        let Some(unit)=unit else{return Ok(false);};
        Ok(unit==self.string_object_unit(id,index)?)
    }

    pub(super) fn initialize_string_object(
        &mut self,
        id: usize,
        value: String,
    ) -> Result<(), Fault> {
        self.initialize_string_value(id,Value::String(value))
    }

    fn initialize_string_value(&mut self,id:usize,value:Value)->Result<(),Fault> {
        if !value.is_string(){return Err(Fault::new("Invalid String object payload."));}
        if value.bytes() > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        if self
            .value_bytes
            .saturating_add(value.bytes())
            .saturating_add("length".len())
            > self.limits.heap.retained_values
        {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        self.work_as(value.bytes(), WorkKind::LengthScan)?;
        let length=value.string_length().expect("validated String payload");

        self.store_property(
            id,
            "length".into(),
            Value::Number(length as f64),
            Some((false, false, false)),
        )?;
        self.value_bytes += value.bytes();
        self.objects[id].primitive = Some(Data::String { value, length });
        Ok(())
    }

    pub(super) fn box_string(&mut self, value: String) -> Result<Value, Fault> {
        self.box_string_value(Value::String(value))
    }

    pub(super) fn box_string_value(&mut self,value:Value)->Result<Value,Fault> {
        if !value.is_string(){return Err(Fault::new("Invalid String object payload."));}
        // Validate before allocating an object; no partial object is exposed.
        if value.bytes() > self.limits.capacity.string_bytes {
            return Err(Fault::new(format!("String limit reached ({} KiB).", self.limits.capacity.string_bytes / 1024)));
        }
        if self
            .value_bytes
            .saturating_add(value.bytes())
            .saturating_add("length".len())
            > self.limits.heap.retained_values
        {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        let id = self.new_object(None)?;
        self.initialize_string_value(id, value)?;
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
            if primitive.is_string(){primitive}else{Value::String(primitive.script_text()?)}
        } else {
            Value::String(String::new())
        };
        self.box_string_value(value)
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
    fn string_object_resource_faults_remain_fatal_and_surrogate_indices_are_exact() {
        for source in [
            "let b=new String('x');try{for(var i=0;i<5000;i++)new String('x');}catch(e){throw 'caught';}finally{throw 'finally';}",
        ] {
            let error=Runtime::for_test().execute(source,&mut NoHost).unwrap_err();
            let diagnostic=error.lines().next().unwrap();
            assert!(!diagnostic.contains("caught")&&!diagnostic.contains("finally"), "{error}");
            assert!(error.contains("Object identity limit"), "{error}");
        }
        succeeds("if(new String('😀')[0]!=='\\ud83d'||new String('😀')[1]!=='\\ude00')throw 'exact indexed surrogate';'ok';");
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


#[cfg(test)]
mod raw_box_tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
    }
    fn succeeds(source:&str){assert_eq!(Runtime::for_test().execute(source,&mut NoHost).unwrap().to_text(),"ok");}

    #[test]
    fn string_box_utf16_constructor_object_routes_exact_slot_and_ordered_hooks() {
        succeeds(r#"
var raw='A\ud800B\udfff',b=new String(raw),o=Object(raw),n=new Object(raw);
for(var box of [b,o,n]){if(typeof box!=='object'||!(box instanceof String)||box.valueOf()!==raw||box.toString()!==raw||String(box)!==raw||box.length!==4)throw 'exact slot';}
if(Object(b)!==b||o===n)throw 'identity';
var order='',arg={toString:function(){order+='S';return raw;},valueOf:function(){throw 'wrong hint';}};
if(new String(arg).valueOf()!==raw||order!=='S')throw 'ordered construction';
var marker={};try{new String({toString:function(){throw marker;}});throw 'missing abrupt';}catch(e){if(e!==marker)throw e;}
try{new String(Symbol('x'));throw 'accepted symbol';}catch(e){if(!(e instanceof TypeError))throw e;}
try{String.prototype.valueOf.call(Object.create(String.prototype));throw 'forged slot';}catch(e){if(!(e instanceof TypeError))throw e;}
var B=String.bind(null,raw);if(new B().valueOf()!==raw)throw 'bound constructor';
'ok';
"#);
    }

    #[test]
    fn string_box_utf16_virtual_units_and_immutable_descriptor_compatibility() {
        succeeds(r#"
for(var b of [new String('😀'),new String('\ud83d\ude00'),new String('\ud800\udfff')]){
if(b.length!==2||b[0].length!==1||b[1].length!==1||b[0].charCodeAt(0)!==b.charCodeAt(0)||b[1].charCodeAt(0)!==b.charCodeAt(1))throw 'exact virtual units';
for(var i=0;i<2;i++){var d=Object.getOwnPropertyDescriptor(b,''+i);if(d.value!==b[i]||d.writable||!d.enumerable||d.configurable)throw 'descriptor';Object.defineProperty(b,i,{value:b[i]});}
try{Object.defineProperty(b,0,{value:'\ufffd'});throw 'accepted replacement';}catch(e){if(!(e instanceof TypeError))throw e;}
if(Object.getOwnPropertyNames(b).join()!=='0,1,length'||Object.keys(b).join()!=='0,1')throw 'virtual enumeration';
Object.freeze(b);if(!Object.isFrozen(b))throw 'freeze';
}
'ok';
"#);
    }

    #[test]
    fn string_box_utf16_json_keys_sloppy_receiver_and_exact_search_consumers() {
        succeeds(r#"
var raw='A\ud800B\udfff',b=new String(raw),o={};o[b]=7;
if(Object.keys(o)[0]!==raw||o[raw]!==7||JSON.parse(JSON.stringify(o))[raw]!==7)throw 'boxed key';
if(JSON.parse(JSON.stringify({x:b})).x!==raw||JSON.parse(JSON.stringify(o,[b]))[raw]!==7)throw 'JSON consumers';
if((function(){return this instanceof String&&this.valueOf()===raw;}).call(raw)!==true)throw 'sloppy boxing';
if(b.indexOf('\ud800')!==1||b.lastIndexOf('\udfff')!==3||b.slice(1,2)!=='\ud800')throw 'exact methods';
var gap=new String('\ud800'),formatted=JSON.stringify({x:1},null,gap);if(formatted.charCodeAt(2)!==55296)throw 'exact gap';
'ok';
"#);
    }
}

#[cfg(test)]
mod raw_box_boundary_tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
    }
    #[test]
    fn string_box_utf16_payload_admission_is_exact_before_object_allocation() {
        let mut runtime=Runtime::for_test();
        let before=runtime.value_bytes;let objects=runtime.objects.len();let properties=runtime.properties;
        let cap=runtime.limits.capacity.string_bytes;let limit=runtime.limits.heap.retained_values;
        runtime.limits.capacity.string_bytes=3;
        let error=runtime.box_string_value(Value::StringUnits(vec![0xd800,0xdfff])).unwrap_err();
        assert!(error.exception.is_none()&&error.message.contains("String limit"));
        assert_eq!((runtime.value_bytes,runtime.objects.len(),runtime.properties),(before,objects,properties));
        runtime.limits.capacity.string_bytes=cap;runtime.limits.heap.retained_values=before+9;
        let error=runtime.box_string_value(Value::StringUnits(vec![0xd800,0xdfff])).unwrap_err();
        assert!(error.exception.is_none()&&error.message.contains("Retained value limit"));
        assert_eq!((runtime.value_bytes,runtime.objects.len(),runtime.properties),(before,objects,properties));
        runtime.limits.heap.retained_values=before+10;runtime.begin_job();
        let boxed=runtime.box_string_value(Value::StringUnits(vec![0xd800,0xdfff])).unwrap();
        assert_eq!((runtime.value_bytes,runtime.objects.len(),runtime.properties),(before+10,objects+1,properties+1));
        assert!(matches!(runtime.wrapped_primitive(&boxed).unwrap(),Some(Value::StringUnits(units)) if units==[0xd800,0xdfff]));
        runtime.limits.heap.retained_values=limit;
    }

    #[test]
    fn string_box_utf16_large_payload_uses_virtual_indices_and_no_property_fanout() {
        let mut runtime=Runtime::for_test();let before=runtime.value_bytes;let properties=runtime.properties;
        let Value::Object(id)=runtime.box_string_value(Value::StringUnits(vec![0xd800;32768])).unwrap()else{panic!("String object")};
        assert_eq!(runtime.value_bytes,before+65536+6);assert_eq!(runtime.properties,properties+1);
        assert_eq!(runtime.objects[id].properties.len(),1);assert_eq!(runtime.string_object_length(id),Some(32768));
        assert!(matches!(runtime.string_object_index_value(id,32767).unwrap(),Value::StringUnits(units) if units==[0xd800]));
        let error=runtime.own_string_keys(id,true).unwrap_err();assert!(error.message.contains("enumeration limit"));
    }

    #[test]
    fn string_box_utf16_index_output_and_unboxing_work_refuse_then_recover() {
        let mut runtime=Runtime::for_test();let boxed=runtime.box_string_value(Value::StringUnits(vec![0xd800,65])).unwrap();
        let Value::Object(id)=boxed else{panic!("String object")};let retained=runtime.value_bytes;
        let cap=runtime.limits.capacity.string_bytes;runtime.limits.capacity.string_bytes=1;runtime.begin_job();
        let error=runtime.string_object_index_value(id,0).unwrap_err();assert!(error.exception.is_none()&&error.message.contains("String limit"));
        assert_eq!(runtime.value_bytes,retained);
        // Descriptor compatibility compares a unit directly, without output.
        assert!(runtime.string_object_index_matches(id,0,&Value::StringUnits(vec![0xd800])).unwrap());
        assert!(!runtime.string_object_index_matches(id,0,&Value::String("\u{fffd}".into())).unwrap());
        runtime.limits.capacity.string_bytes=cap;runtime.begin_job();runtime.copy_bytes=0;
        let error=runtime.wrapped_primitive(&Value::Object(id)).unwrap_err();assert!(error.message.contains("work limit"));
        assert_eq!(runtime.value_bytes,retained);runtime.begin_job();runtime.copy_bytes=0;
        let error=runtime.string_object_index_value(id,0).unwrap_err();assert!(error.message.contains("work limit"));
        assert_eq!(runtime.value_bytes,retained);runtime.begin_job();
        assert!(matches!(runtime.string_object_index_value(id,0).unwrap(),Value::StringUnits(units) if units==[0xd800]));
        assert!(matches!(runtime.wrapped_primitive(&Value::Object(id)).unwrap(),Some(Value::StringUnits(units)) if units==[0xd800,65]));
        assert_eq!(runtime.value_bytes,retained);
    }
}
