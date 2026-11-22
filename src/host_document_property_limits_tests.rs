//! Shared VM-store budgets/roots after actual Bridge admission tests elsewhere.
//! This small Host fixture exposes no DOM operations or new native capability.
use super::super::*;

struct Native;
impl Host for Native {
    fn expando_owner(&self, handle:u32)->Option<u32> {(handle==42).then_some(77)}
    fn native_access_owner(&self, handle:u32)->Option<u32> {self.expando_owner(handle)}
    fn instance_ancestry(&self,handle:u32)->Option<NativeAncestry> {
        self.expando_owner(handle).map(|window|NativeAncestry {window,constructor:None})
    }
    fn owns_property(&self,_:u32,name:&str)->bool {name=="nativeField"}
    fn ordinary_native_fallback(&self,handle:u32,name:&str)->bool {handle==42&&name!="nativeField"}
    fn get(&mut self,_:u32,name:&str)->Result<Value,String> {
        Ok(if name=="nativeField" {Value::Number(7.0)} else {Value::Undefined})
    }
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String> {panic!("unexpected native write")}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String> {panic!("unexpected native call")}
}

fn fixture(capacity:bool)->Runtime {
    let mut runtime=if capacity {Runtime::for_capacity_test()}else{Runtime::for_test()};
    runtime.bind_browser_global(77).unwrap();
    runtime.define_global("document",Value::Host(42)).unwrap();
    runtime.execute("var caught=0,finalized=0,after=0;0;",&mut Native).unwrap();
    runtime
}

#[test]
fn deferred_host_attribute_preserves_descriptor_identity_and_retries_a_late_refusal() {
    let mut runtime = fixture(false);
    let prototype = runtime.new_object(None).unwrap();
    let initial_functions = runtime.function_arena.occupied;
    runtime.define_host_attribute(prototype, 77, "example", "read", Some("write")).unwrap();
    assert_eq!(runtime.function_arena.occupied, initial_functions);
    let objects = runtime.objects.len();
    let properties = runtime.properties;
    let payload = |r: &Runtime| r.value_bytes - r.retained_arena_bytes();
    let before = payload(&runtime);
    let previous = runtime.event_property_allowance_for_test(1);
    // length fits on the new function, but its name does not. Repeating
    // this failed read must leave the same property and identity available.
    for _ in 0..2 {
        assert!(runtime.materialize_host_accessor(prototype, "example", false).is_err());
        assert_eq!((runtime.function_arena.occupied, runtime.objects.len(), runtime.properties),
                   (initial_functions, objects, properties));
        assert_eq!(payload(&runtime), before);
    }
    runtime.event_property_allowance_for_test(previous);
    runtime.materialize_host_accessor(prototype, "example", false).unwrap();
    let getter = runtime.objects[prototype].properties["example"].accessor.unwrap().get.unwrap();
    assert_eq!(runtime.function_arena.occupied, initial_functions + 1);
    runtime.define_global("samplePrototype", Value::Object(prototype)).unwrap();
    runtime.execute(r#"
        Object.freeze(samplePrototype);
        var a = Object.getOwnPropertyDescriptor(samplePrototype, 'example');
        var b = Object.getOwnPropertyDescriptor(samplePrototype, 'example');
        if(a.get !== b.get || a.set !== b.set || a.get === a.set || a.configurable || !a.enumerable) throw 'identity or flags';
        if(a.get.name !== 'get example' || a.set.name !== 'set example' || a.get.length !== 0 || a.set.length !== 1) throw 'accessor functions';
    "#, &mut Native).unwrap();
    assert_eq!(runtime.objects[prototype].properties["example"].accessor.unwrap().get, Some(getter));
}

#[test]
fn deferred_host_attribute_replacement_and_deletion_do_not_allocate_unused_functions() {
    let mut runtime = fixture(false);
    let prototype = runtime.new_object(None).unwrap();
    for name in ["removed", "replaced", "flags"] {
        runtime.define_host_attribute(prototype, 77, name, "read", Some("write")).unwrap();
    }
    runtime.define_global("samplePrototype", Value::Object(prototype)).unwrap();
    runtime.execute("Object.keys({}); Object.defineProperty({}, 'warm', {value:1});", &mut Native).unwrap();
    let functions = runtime.function_arena.occupied;
    runtime.execute(r#"
        var keys = Object.keys(samplePrototype);
        if(!('removed' in samplePrototype) || keys.length !== 3 || keys[0] !== 'removed' || keys[1] !== 'replaced' || keys[2] !== 'flags') throw 'keys';
        delete samplePrototype.removed;
        Object.defineProperty(samplePrototype, 'replaced', {value: 42});
        Object.defineProperty(samplePrototype, 'flags', {enumerable:false, configurable:false});
        if('removed' in samplePrototype || samplePrototype.replaced !== 42) throw 'replacement';
    "#, &mut Native).unwrap();
    assert_eq!(runtime.function_arena.occupied, functions);
    let property = &runtime.objects[prototype].properties["flags"];
    assert!(!property.enumerable && !property.configurable);
    assert!(matches!(property.value, StoredValue::DeferredHostAttribute(_)));
}

fn fatal(runtime:&mut Runtime,source:&str,message:&str) {
    let error=runtime.execute(source,&mut Native).unwrap_err();
    assert!(error.contains(message),"{error}");
    assert!(runtime.invocation_failed_fatally());
    for name in ["caught","finalized","after"] {
        assert_eq!(runtime.binding_value(runtime.global_environment,name).unwrap().to_number(),0.0,"{name}");
    }
    assert!(runtime.invocation.is_none());
    assert!(runtime.transient_registration_is_empty());
}

#[test]
fn document_own_properties_vm_store_creation_failure_is_unpublished_and_recovers() {
    let mut runtime=fixture(false);
    let objects=runtime.objects.len();let limit=runtime.limits.heap.objects;
    runtime.limits.heap.objects=objects;
    fatal(&mut runtime,"try{document.fresh=1;after++;}catch(e){caught++;}finally{finalized++;}after++;","Object identity limit");
    assert_eq!(runtime.objects.len(),objects);
    assert!(!runtime.host_objects.contains_key(&42));
    assert!(!runtime.native_expando_owners.contains_key(&42));
    runtime.limits.heap.objects=limit;runtime.begin_job();
    assert_eq!(runtime.execute("if(document.fresh!==undefined)throw 'failed store published';document.fresh=17;document.fresh+document.nativeField;",&mut Native).unwrap().to_number(),24.0);
}

#[test]
fn document_own_properties_vm_property_and_retained_growth_fail_before_publication() {
    for mode in 0..3 {
        let mut runtime=fixture(false);
        runtime.execute("try{}catch(e){}finally{}document.keep='old';",&mut Native).unwrap();
        let id=runtime.host_objects[&42];
        let source=if mode==2 {"document.keep='larger retained value';"} else {"document.overflow=19;"};
        let callback=runtime.execute(&format!("(function(){{try{{{source}after++;}}catch(e){{caught++;}}finally{{finalized++;}}after++;}});"),&mut Native).unwrap();
        let before=(runtime.properties,runtime.value_bytes,runtime.objects[id].properties.len());
        let limits=runtime.limits.heap;
        let message=match mode {
            0=>{runtime.limits.heap.object_properties=before.2;"property limit"},
            1=>{runtime.limits.heap.properties=before.0;"property limit"},
            _=>{runtime.limits.heap.retained_values=before.1;"Retained value limit"},
        };
        let error=runtime.invoke(callback,&[],Value::Undefined,&mut Native).unwrap_err();
        assert!(error.contains(message),"{error}");
        assert!(runtime.invocation_failed_fatally());
        for name in ["caught","finalized","after"] {assert_eq!(runtime.binding_value(runtime.global_environment,name).unwrap().to_number(),0.0,"{name}");}
        assert!(runtime.invocation.is_none());assert!(runtime.transient_registration_is_empty());
        assert_eq!((runtime.properties,runtime.value_bytes,runtime.objects[id].properties.len()),before);
        assert_eq!(runtime.get_property(id,"keep").unwrap().to_text(),"old");
        assert!(!runtime.objects[id].properties.contains_key("overflow"));
        runtime.limits.heap=limits;runtime.begin_job();
        assert_eq!(runtime.execute("if(document.keep!=='old'||document.nativeField!==7)throw 'quota changed old state';document.recovered=23;document.recovered;",&mut Native).unwrap().to_number(),23.0);
    }
}

#[test]
fn document_own_properties_vm_store_copy_and_instruction_limits_keep_old_value() {
    for copies in [true,false] {
        let mut runtime=fixture(false);
        runtime.execute("document.keep='old';",&mut Native).unwrap();
        let before=(runtime.properties,runtime.value_bytes);
        if copies {runtime.copy_bytes=0;}else{runtime.steps=0;}
        let error=runtime.host_write(42,"keep".into(),Value::String("new payload".into()),&mut Native).unwrap_err();
        assert!(error.exception.is_none(),"{error:?}");
        assert!(error.message.contains("limit"),"{error:?}");
        assert_eq!((runtime.properties,runtime.value_bytes),before);
        assert!(runtime.transient_registration_is_empty());
        runtime.begin_job();
        assert_eq!(runtime.execute("document.keep;",&mut Native).unwrap().to_text(),"old");
    }
}

#[test]
fn document_own_properties_vm_store_traces_retained_closure_and_state_during_collection() {
    let mut runtime=fixture(true);
    runtime.execute(r#"
(function(seed){document.reader=function(){return seed;};document.state={kept:seed+1};})(43);
function pressure(){var trash;for(var n=0;n<1600;n++)trash=function(){return n;};}
pressure();
if(document.reader()!==43||document.state.kept!==44||document.nativeField!==7)throw 'own-store roots lost';
0;
"#,&mut Native).unwrap();
    assert!(runtime.function_arena.collections>0);
    assert!(runtime.function_arena.scope_retired>0);
    runtime.begin_job();
    assert_eq!(runtime.execute("document.reader()+document.state.kept;",&mut Native).unwrap().to_number(),87.0);
    assert!(runtime.invocation.is_none());
    assert!(runtime.transient_registration_is_empty());
}

#[derive(Default)]
struct ReadHost { fallback: bool, gets: Vec<String>, writes: usize, calls: usize }
impl Host for ReadHost {
    fn owns_property(&self, _:u32, _:&str)->bool { false }
    fn ordinary_native_fallback(&self, handle:u32, _:&str)->bool {handle==1&&self.fallback}
    fn get(&mut self, handle:u32, name:&str)->Result<Value,String> {
        assert_eq!(handle,1);
        self.gets.push(name.into());
        Ok(if name=="nativeNamed" {Value::Number(17.0)} else {Value::Undefined})
    }
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String> {
        self.writes+=1;Err("Unexpected native write.".into())
    }
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String> {
        self.calls+=1;Err("Unexpected native call.".into())
    }
}

fn read_fixture(fallback:bool)->(Runtime,ReadHost) {
    let mut runtime=Runtime::for_test();
    runtime.bind_host_object(1,&[]).unwrap();
    runtime.define_global("native",Value::Host(1)).unwrap();
    (runtime,ReadHost {fallback,..ReadHost::default()})
}

#[test]
fn document_own_properties_native_getter_recursion_uses_the_shared_guard_on_one_mib_and_recovers() {
    std::thread::Builder::new().stack_size(1024*1024).spawn(|| {
        for fallback in [false,true] {
            let (mut runtime,mut host)=read_fixture(fallback);
            runtime.execute("var entries=0,caught=0,finalized=0,after=0;Object.defineProperty(native,'x',{configurable:true,get:function(){entries++;return this.x;}});0;",&mut host).unwrap();
            runtime.begin_job();
            let error=runtime.execute("try{native.x;}catch(e){caught++;}finally{finalized++;}after++;",&mut host).unwrap_err();
            assert!(error.contains("Accessor call limit reached (32)"),"fallback={fallback}: {error}");
            assert!(runtime.invocation_failed_fatally());
            // Each real accessor and its Script callback use the same quota.
            assert_eq!(runtime.binding_value(runtime.global_environment,"entries").unwrap().to_number(),(MAX_CALL_DEPTH/2) as f64);
            for name in ["caught","finalized","after"] {
                assert_eq!(runtime.binding_value(runtime.global_environment,name).unwrap().to_number(),0.0,"{name}");
            }
            assert!(host.gets.is_empty(),"An own accessor must bypass native fallback");
            assert_eq!((host.writes,host.calls),(0,0));
            assert_eq!((runtime.call_depth,runtime.eval_depth,runtime.evaluation_frames),(0,0,0));
            assert!(runtime.invocation.is_none());
            assert!(runtime.transient_registration_is_empty());
            assert!(!runtime.strict);
            runtime.begin_job();
            assert_eq!(runtime.execute("Object.defineProperty(native,'x',{value:7});native.x*6;",&mut host).unwrap().to_number(),42.0);
            assert!(!runtime.invocation_failed_fatally());
            assert!(host.gets.is_empty());
            assert_eq!((host.writes,host.calls),(0,0));
            assert_eq!((runtime.call_depth,runtime.eval_depth,runtime.evaluation_frames),(0,0,0));
            assert!(runtime.invocation.is_none());
            assert!(runtime.transient_registration_is_empty());
        }
    }).unwrap().join().unwrap();
}

#[test]
fn document_own_properties_native_named_fallback_reads_once_before_inherited_accessors_and_keeps_receiver() {
    for fallback in [false,true] {
        let (mut runtime,mut host)=read_fixture(fallback);
        assert_eq!(runtime.execute(r#"
var inherited=0,named=0,own=0;
Object.defineProperty(Object.prototype,'inheritedProbe',{configurable:true,get:function(){inherited++;if(this!==native)throw 'inherited receiver';return 23;}});
Object.defineProperty(Object.prototype,'nativeNamed',{configurable:true,get:function(){named++;if(this!==native)throw 'named receiver';return 31;}});
native.inheritedProbe;
"#,&mut host).unwrap().to_number(),23.0);
        assert_eq!(runtime.binding_value(runtime.global_environment,"inherited").unwrap().to_number(),1.0);
        let first=if fallback {vec!["inheritedProbe"]} else {Vec::new()};
        assert_eq!(host.gets,first);
        assert_eq!(runtime.execute("native.nativeNamed;",&mut host).unwrap().to_number(),if fallback {17.0}else{31.0});
        assert_eq!(runtime.binding_value(runtime.global_environment,"named").unwrap().to_number(),if fallback {0.0}else{1.0});
        let before=host.gets.clone();
        runtime.execute(r#"
Object.defineProperty(native,'nativeNamed',{value:undefined,configurable:true});
if(native.nativeNamed!==undefined)throw 'own undefined lost';
Object.defineProperty(native,'nativeNamed',{configurable:true,get:function(){own++;if(this!==native)throw 'own receiver';return undefined;}});
if(native.nativeNamed!==undefined||own!==1)throw 'own accessor lost';0;
"#,&mut host).unwrap();
        assert_eq!(host.gets,before,"Own undefined/accessors must not call native fallback");
        assert_eq!(runtime.execute("delete native.nativeNamed;native.nativeNamed;",&mut host).unwrap().to_number(),if fallback {17.0}else{31.0});
        let expected=if fallback {vec!["inheritedProbe","nativeNamed","nativeNamed"]}else{Vec::new()};
        assert_eq!(host.gets,expected);
        assert_eq!((host.writes,host.calls),(0,0));
        assert_eq!((runtime.call_depth,runtime.eval_depth,runtime.evaluation_frames),(0,0,0));
        assert!(runtime.invocation.is_none());
        assert!(runtime.transient_registration_is_empty());
    }
}
