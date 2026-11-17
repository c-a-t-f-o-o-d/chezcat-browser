//! Original Annex B Object.prototype.__proto__ controls, independent of libraries.
use super::*;

#[derive(Default)]
struct GuardHost { calls: usize, foreign: bool }
impl Host for GuardHost {
    fn native_access_owner(&self, object: u32) -> Option<u32> {
        (self.foreign && object == 42).then_some(9)
    }
    fn validate_window_access(&self, _: Option<u32>, _: u32) -> Result<(), String> {
        Err("Original foreign native access refusal.".into())
    }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
        self.calls += 1; Err("Original native lookup refusal.".into())
    }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
        self.calls += 1; Err("Original native write refusal.".into())
    }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
        self.calls += 1; Err("Original native call refusal.".into())
    }
}
fn succeeds(source: &str) {
    let mut runtime = Runtime::for_test();
    assert_eq!(runtime.execute(source, &mut GuardHost::default()).unwrap().to_text(), "ok");
}

#[test]
fn legacy_proto_real_accessor_descriptor_getter_and_membership() {
    succeeds(r#"
var d=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__');
if(d.enumerable||!d.configurable||d.value!==undefined||typeof d.get!=='function'||typeof d.set!=='function'||d.get.length!==0||d.set.length!==1||d.get.name!=='get __proto__'||d.set.name!=='set __proto__')throw 'real descriptor';
if(!('__proto__' in {})||({}).__proto__!==Object.prototype||d.get.call(Object.prototype)!==null)throw 'ordinary root';
for(var value of [1,true,'text',Symbol('original')])if(d.get.call(value)!==Object.getPrototypeOf(value))throw 'ToObject';
var caught=0;for(var value of [null,undefined]){try{d.get.call(value);}catch(e){if(e.name==='TypeError')caught++;}}
try{new d.get();}catch(e){if(e.name==='TypeError')caught++;}try{new d.set();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==4)throw 'getter coercibility and nonconstructor';'ok';
"#);
}

#[test]
fn legacy_proto_setter_changes_real_array_chain_and_canonical_operations() {
    succeeds(r#"
var parent=Object.create(Array.prototype);parent.marker=19;
var array=[2,3],set=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__').set;
if(set.call(array,parent)!==undefined||array.__proto__!==parent||array.marker!==19||!Array.isArray(array)||array.push(5)!==3||array.join(',')!=='2,3,5')throw 'actual array chain';
var get=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__').get;
Object.getPrototypeOf=function(){throw 'overridden static used';};Object.setPrototypeOf=function(){throw 'overridden static used';};
var target={};if(set.call(target,parent)!==undefined||get.call(target)!==parent)throw 'private canonical operations';
var first={marker:7};target.__proto__=first;if(target.marker!==7||get.call(target)!==first)throw 'ordinary assignment';
'ok';
"#);
}

#[test]
fn legacy_proto_own_data_null_dictionary_computed_and_json_are_ordinary_keys() {
    succeeds(r#"
var dictionary=Object.create(null);if('__proto__' in dictionary||dictionary.__proto__!==undefined)throw 'absent null key';
dictionary.__proto__=13;if(dictionary.__proto__!==13||Object.getPrototypeOf(dictionary)!==null||!Object.prototype.hasOwnProperty.call(dictionary,'__proto__'))throw 'null own data';
var computed={['__proto__']:17};if(computed.__proto__!==17||Object.getPrototypeOf(computed)!==Object.prototype||Object.keys(computed).join(',')!=='__proto__')throw 'computed data';
var object={};Object.defineProperty(object,'__proto__',{value:23,writable:true,enumerable:true,configurable:true});object.__proto__=29;
if(object.__proto__!==29||Object.getPrototypeOf(object)!==Object.prototype||!delete object.__proto__||object.__proto__!==Object.prototype)throw 'own shadow and delete';
var json=JSON.parse('{"__proto__":{"marker":31}}');if(json.__proto__.marker!==31||Object.getPrototypeOf(json)!==Object.prototype||!Object.prototype.hasOwnProperty.call(json,'__proto__'))throw 'JSON data';
var literal={__proto__:null,answer:37};if(Object.getPrototypeOf(literal)!==null||'__proto__' in literal||literal.answer!==37)throw 'literal colon remains distinct';
'ok';
"#);
}

#[test]
fn legacy_proto_setter_order_ignores_primitives_without_author_coercion() {
    succeeds(r#"
var d=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__'),target={},effects=0;
var prototype={toString:function(){effects++;throw 'coercion';},valueOf:function(){effects++;throw 'coercion';}};
for(var value of [undefined,1,true,'ignored',Symbol('ignored')])if(d.set.call(target,value)!==undefined||d.get.call(target)!==Object.prototype)throw 'ignored primitive RHS';
for(var receiver of [1,true,'text',Symbol('receiver')])if(d.set.call(receiver,prototype)!==undefined)throw 'primitive receiver';
var caught=0;for(var receiver of [null,undefined]){try{d.set.call(receiver,17);}catch(e){if(e.name==='TypeError')caught++;}}
if(caught!==2||effects!==0)throw 'coercibility precedes ignored RHS';
if(d.set.call(target,prototype)!==undefined||d.get.call(target)!==prototype||effects!==0)throw 'object prototype is not coerced';
'ok';
"#);
}

#[test]
fn legacy_proto_cycles_extensibility_accessor_deletion_and_replacement_are_real() {
    succeeds(r#"
var d=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__'),first={},child=Object.create(first),caught=0;
try{first.__proto__=child;}catch(e){if(e.name==='TypeError')caught++;}if(Object.getPrototypeOf(first)!==Object.prototype)throw 'cycle mutated';
Object.preventExtensions(first);if(d.set.call(first,Object.prototype)!==undefined)throw 'same prototype no-op';
try{d.set.call(first,null);}catch(e){if(e.name==='TypeError')caught++;}
try{d.set.call(Object.prototype,{});}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3||d.set.call(Object.prototype,null)!==undefined)throw 'extensibility/immutable root';
if(!delete Object.prototype.__proto__||'__proto__' in {})throw 'configurable accessor delete';
var ordinary={};ordinary.__proto__=41;if(ordinary.__proto__!==41||Object.getPrototypeOf(ordinary)!==Object.prototype)throw 'no magic after delete';
Object.defineProperty(Object.prototype,'__proto__',{value:43,writable:true,configurable:true});
var replacement={};replacement.__proto__=47;if(replacement.__proto__!==47||Object.getPrototypeOf(replacement)!==Object.prototype)throw 'ordinary replacement';
'ok';
"#);
}

#[test]
fn legacy_proto_each_realm_has_distinct_accessor_and_immutable_object_root() {
    let mut runtime = Runtime::for_test();
    let mut host = GuardHost::default();
    let main = runtime.object_prototype.unwrap();
    let child = runtime.allocate_intrinsic_realm().unwrap();
    let (child_root, child_get, child_set) = runtime.with_intrinsic_realm(Some(child), |runtime| {
        let result = runtime.execute("Object.getOwnPropertyDescriptor(Object.prototype,'__proto__');", &mut host).map_err(Fault::new)?;
        let Value::Object(id) = result else { panic!("descriptor") };
        let get = runtime.get_property(id, "get")?;
        let set = runtime.get_property(id, "set")?;
        Ok((runtime.object_prototype.unwrap(), get, set))
    }).unwrap();
    runtime.define_global("childRoot", Value::Object(child_root)).unwrap();
    runtime.define_global("childGet", child_get).unwrap();
    runtime.define_global("childSet", child_set).unwrap();
    assert_eq!(runtime.execute("var d=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__');if(d.get===childGet||d.set===childSet||childGet.call(childRoot)!==null)throw 'independent roots';var caught=0;try{d.set.call(childRoot,{});}catch(e){if(e.name==='TypeError')caught++;}try{childSet.call(Object.prototype,{});}catch(e){if(e.name==='TypeError')caught++;}if(caught!==2||childSet.call(childRoot,null)!==undefined)throw 'every immutable root';'ok';", &mut host).unwrap().to_text(), "ok");
    assert_eq!(runtime.objects[main].prototype, None);
    assert_eq!(runtime.objects[child_root].prototype, None);
}

#[test]
fn legacy_proto_native_and_foreign_capabilities_refuse_before_host_or_author_effects() {
    for foreign in [false, true] {
        for source in [
            "get.call(native);",
            "set.call(native,{});",
            "var target={};set.call(target,native);",
        ] {
            let mut runtime = Runtime::for_test();
            let mut host = GuardHost { foreign, ..GuardHost::default() };
            runtime.define_global("native", Value::Host(42)).unwrap();
            runtime.execute("var d=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__'),get=d.get,set=d.set,effects=0;", &mut host).unwrap();
            let error = runtime.execute(&format!("try{{{source}}}catch(e){{effects++;}}finally{{effects++;}}"), &mut host).unwrap_err();
            assert!(error.contains("native") || error.contains("Native"), "{source}: {error}");
            assert_eq!(host.calls, 0, "Native refusal precedes Host reads/writes/calls");
            runtime.begin_job();
            assert_eq!(runtime.execute("effects;", &mut host).unwrap().to_number(), 0.0, "Fatal capability errors cannot run catch/finally");
        }
    }
}

#[test]
fn legacy_proto_property_and_chain_work_limits_remain_fatal_and_atomic() {
    let mut runtime = Runtime::for_test();
    let mut host = GuardHost::default();
    runtime.execute("var get=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__').get,target={},parent=null;for(var i=0;i<64;i++)parent={__proto__:parent};", &mut host).unwrap();
    let properties = runtime.limits.capacity.property_bytes;
    runtime.limits.capacity.property_bytes = 8;
    let error = runtime.execute("target.__proto__;", &mut host).unwrap_err();
    assert!(error.contains("Property-name limit"), "{error}");
    runtime.limits.capacity.property_bytes = properties;
    let steps = runtime.limits.steps;
    runtime.limits.steps = 32;
    // Limits select the next job; begin_job copies them into the live counters.
    runtime.begin_job();
    let error = runtime.execute("target.__proto__=parent;", &mut host).unwrap_err();
    assert!(error.contains("step limit"), "{error}");
    runtime.limits.steps = steps;
    runtime.begin_job();
    assert_eq!(runtime.execute("get.call(target)===Object.prototype;", &mut host).unwrap().to_text(), "true");
    runtime.execute("var dictionary=Object.create(null);dictionary.keep=7;", &mut host).unwrap();
    let own_properties = runtime.limits.heap.object_properties;
    runtime.limits.heap.object_properties = 1;
    // This dictionary has no inherited setter: __proto__ must grow a genuine
    // own data property and therefore consume the ordinary property allowance.
    let error = runtime.execute("dictionary.__proto__=parent;", &mut host).unwrap_err();
    assert!(error.to_ascii_lowercase().contains("property"), "{error}");
    runtime.limits.heap.object_properties = own_properties;
    runtime.begin_job();
    assert_eq!(runtime.execute("Object.getPrototypeOf(dictionary)===null&&dictionary.keep===7&&!('__proto__' in dictionary)&&Object.keys(dictionary).join(',')==='keep';", &mut host).unwrap().to_text(), "true");
    assert_eq!(runtime.execute("dictionary.__proto__=parent;Object.getPrototypeOf(dictionary)===null&&Object.getOwnPropertyDescriptor(dictionary,'__proto__').value===parent;", &mut host).unwrap().to_text(), "true");
}
