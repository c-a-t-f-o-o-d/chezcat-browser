
//! Original exact own-key reflection controls.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native reads".into())}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native writes".into())}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native calls".into())}
}
fn succeeds(source:&str){assert_eq!(Runtime::for_test().execute(source,&mut NoHost).unwrap().to_text(),"ok");}

#[test]
fn reflect_own_keys_native_metadata_captured_identity_and_author_mutation() {
    succeeds(r#"
var f=Reflect.ownKeys,d=Object.getOwnPropertyDescriptor(Reflect,'ownKeys');
if(typeof f!=='function'||f.name!=='ownKeys'||f.length!==1||f.prototype!==undefined||f.toString().indexOf('[native code]')<0)throw 'native metadata';
if(d.value!==f||!d.writable||!d.configurable||d.enumerable)throw 'method descriptor';
var rejected=false;try{new f({});}catch(e){if(e instanceof TypeError)rejected=true;}if(!rejected)throw 'constructible';
Reflect.ownKeys=7;if(Reflect.ownKeys!==7||f({x:1})[0]!=='x')throw 'captured override';
delete Reflect.ownKeys;if(Reflect.ownKeys!==undefined||'ownKeys' in Reflect)throw 'resurrection';
Object.defineProperty(Reflect,'ownKeys',d);if(Reflect.ownKeys!==f)throw 'restore';
'ok';
"#);
}

#[test]
fn reflect_own_keys_exact_raw_hidden_strings_symbols_and_creation_order() {
    succeeds(r#"
var o={},s=Symbol('first'),t=Symbol('last'),calls=0;
o.z=1;o[10]=1;o[2]=1;o['\ud800']=1;o['\ufffd']=1;o[s]=1;
Object.defineProperty(o,'hidden',{get:function(){calls++;throw 'getter';},enumerable:false});
Object.defineProperty(o,'\udfff',{value:3,enumerable:false});o[t]=1;
delete o.z;o.z=2;
var keys=Reflect.ownKeys(o);
if(keys.length!==9||keys[0]!=='2'||keys[1]!=='10'||keys[2]!=='\ud800'||keys[3]!=='\ufffd'||keys[4]!=='hidden'||keys[5]!=='\udfff'||keys[6]!=='z'||keys[7]!==s||keys[8]!==t||calls!==0)throw 'exact ordered snapshot';
delete o['\ud800'];o.newKey=1;if(keys[2]!=='\ud800'||keys.length!==9)throw 'snapshot identity';
'ok';
"#);
}

#[test]
fn reflect_own_keys_object_only_no_coercion_and_virtual_exotic_keys() {
    succeeds(r#"
var errors=0;for(var value of [undefined,null,true,1,'x','\ud800',Symbol('x')]){try{Reflect.ownKeys(value);}catch(e){if(e instanceof TypeError)errors++;else throw e;}}
if(errors!==7)throw 'object target';
var o={toString:function(){throw 'coerced target';}};if(Reflect.ownKeys(o)[0]!=='toString')throw 'object snapshot';
var a=[];a[2]=1;a.extra=1;if(Reflect.ownKeys(a).join()!=='2,length,extra')throw 'array own length';
var b=new String('\ud800x');if(Reflect.ownKeys(b).join()!=='0,1,length')throw 'String virtual indices';
if(Reflect.ownKeys(new Uint8Array([7,8])).join()!=='0,1')throw 'typed indices';
function sample(a){}if(Reflect.ownKeys(sample).join()!==Object.getOwnPropertyNames(sample).join())throw 'function keys';
'ok';
"#);
}

#[test]
fn reflect_own_keys_combined_quota_array_and_identity_refusals_recover() {
    let mut runtime=Runtime::for_test();let mut host=NoHost;
    let target=runtime.execute("var sym=Symbol('key'),record={};record['\\ud800']=7;record[sym]=8;record;",&mut host).unwrap();
    let before=runtime.objects.len();let cap=runtime.limits.heap.object_properties;
    runtime.limits.heap.object_properties=1;runtime.begin_job();
    let error=runtime.reflect_own_keys(&[target.clone()],&mut host).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("enumeration allowance"));assert_eq!(runtime.objects.len(),before);
    runtime.limits.heap.object_properties=cap;let arrays=runtime.limits.heap.array_length;
    runtime.limits.heap.array_length=1;runtime.begin_job();
    let error=runtime.reflect_own_keys(&[target.clone()],&mut host).unwrap_err();
    assert!(error.message.contains("Array length limit"));assert_eq!(runtime.objects.len(),before);
    runtime.limits.heap.array_length=arrays;let objects=runtime.limits.heap.objects;
    runtime.limits.heap.objects=before;runtime.begin_job();
    let error=runtime.reflect_own_keys(&[target.clone()],&mut host).unwrap_err();
    assert!(error.message.contains("Object identity limit"));assert_eq!(runtime.objects.len(),before);
    runtime.limits.heap.objects=objects;runtime.begin_job();
    let Value::Object(result)=runtime.reflect_own_keys(&[target],&mut host).unwrap()else{panic!("array")};
    assert_eq!(runtime.objects[result].length,Some(2));
    assert!(matches!(runtime.own_property(result,"0").unwrap(),Value::StringUnits(units) if units==[0xd800]));
    assert!(matches!(runtime.own_property(result,"1").unwrap(),Value::Symbol(_)));
}

#[test]
fn reflect_own_keys_output_copy_step_refusal_preserves_target_and_recovers() {
    let mut runtime=Runtime::for_test();let mut host=NoHost;
    let target=runtime.execute("var record={};record['\\ud800']=7;record;",&mut host).unwrap();
    let retained=runtime.value_bytes;let cap=runtime.limits.capacity.string_bytes;let limit=runtime.limits.heap.retained_values;
    runtime.limits.capacity.string_bytes=1;runtime.begin_job();
    let error=runtime.reflect_own_keys(&[target.clone()],&mut host).unwrap_err();assert!(error.message.contains("String limit"));
    runtime.limits.capacity.string_bytes=cap;runtime.limits.heap.retained_values=retained;runtime.begin_job();
    let error=runtime.reflect_own_keys(&[target.clone()],&mut host).unwrap_err();assert!(error.message.contains("Retained value limit"));
    runtime.limits.heap.retained_values=limit;runtime.begin_job();runtime.copy_bytes=0;
    let error=runtime.reflect_own_keys(&[target.clone()],&mut host).unwrap_err();assert!(error.message.contains("work limit"));
    runtime.begin_job();runtime.steps=0;
    let error=runtime.reflect_own_keys(&[target.clone()],&mut host).unwrap_err();assert!(error.message.contains("step limit"));
    runtime.begin_job();
    let Value::Object(result)=runtime.reflect_own_keys(&[target],&mut host).unwrap()else{panic!("array")};
    assert!(matches!(runtime.own_property(result,"0").unwrap(),Value::StringUnits(units) if units==[0xd800]));
    runtime.begin_job();assert_eq!(runtime.execute("record['\\ud800'];",&mut host).unwrap().to_text(),"7");
}

#[test]
fn reflect_own_keys_result_ignores_inherited_setters_and_retains_proxy_refusal() {
    succeeds(r#"
var intercepted=0,keys;
Object.defineProperty(Array.prototype,'0',{set:function(){intercepted++;throw 'inherited setter';},configurable:true});
try{keys=Reflect.ownKeys({key:1});if(keys[0]!=='key'||intercepted!==0)throw 'array construction';}finally{delete Array.prototype[0];}
'ok';
"#);
    let mut runtime=Runtime::for_test();let mut host=NoHost;
    let target=runtime.execute("var ownTrapCalls=0;var proxy=new Proxy({x:1},{ownKeys:function(){ownTrapCalls++;throw 'trap';}});proxy;",&mut host).unwrap();
    let error=runtime.reflect_own_keys(&[target],&mut host).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("Proxy ownKeys is not implemented"));
    runtime.begin_job();assert_eq!(runtime.execute("ownTrapCalls;",&mut host).unwrap().to_text(),"0");
}

#[test]
fn reflect_own_keys_native_origin_refusal_precedes_host_keys_and_backing() {
    struct Denied;
    impl Host for Denied {
        fn native_access_owner(&self,handle:u32)->Option<u32>{assert_eq!(handle,42);Some(88)}
        fn validate_window_access(&self,_:Option<u32>,owner:u32)->Result<(),String>{assert_eq!(owner,88);Err("Own-key native owner denied".into())}
        fn own_keys(&mut self,_:u32)->Result<Vec<String>,String>{panic!("keys before owner validation")}
        fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("native getter")}
        fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("native setter")}
        fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("native call")}
    }
    let mut runtime=Runtime::for_test();let before=runtime.objects.len();
    let error=runtime.reflect_own_keys(&[Value::Host(42)],&mut Denied).unwrap_err();
    assert!(error.exception.is_none()&&error.message.contains("native owner denied"));
    assert_eq!(runtime.objects.len(),before);assert!(!runtime.host_objects.contains_key(&42));
}
