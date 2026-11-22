//! Actual filter backing and raw-unit payload subscription admission/refund.
use super::*;
struct Tree;
impl Host for Tree{
 fn get(&mut self,_object:u32,_property:&str)->Result<Value,String>{Err("unused tree getter".into())}
 fn set(&mut self,_object:u32,_property:&str,_value:Value)->Result<(),String>{Err("unused tree setter".into())}
 fn call(&mut self,_object:u32,_method:&str,_args:&[Value])->Result<Value,String>{Err("unused tree call".into())}
}
fn registration(wide:bool)->Registration{let mut names=Vec::new();names.try_reserve_exact(if wide{8}else{1}).unwrap();if wide{names.push(DomString::from_units(&[0x64,0x61,0x74,0x61,0x2d,0xd800],128).unwrap());let mut scalar=String::with_capacity(128);scalar.push_str("data-�");names.push(scalar.into());}else{names.push("data-next".into());}Registration{observer:0,character_data:false,child_list:false,attributes:true,subtree:false,old_value:false,attribute_old_value:true,attribute_filter:Some(names)}}
#[test]
fn attribute_filter_units_actual_backing_refusal_replacement_credit_and_disconnect_refund(){
 let mut runtime=Runtime::with_limits(limits::BROWSER);runtime.mutations.observers.insert(0,Observer{callback:Value::Undefined,targets:Vec::new(),pending:Vec::new(),scheduled:false});let base=runtime.value_bytes;let request=registration(true);let filter=request.attribute_filter.as_ref().unwrap();let expected=filter.capacity()*std::mem::size_of::<DomString>()+filter.iter().map(DomString::capacity).sum::<usize>();assert_eq!(request.retained_bytes(),expected);assert_ne!(filter[0],filter[1]);runtime.limits.heap.retained_values=base+expected-1;assert!(runtime.install_mutation_registration(0,&1,request).is_err());assert_eq!(runtime.value_bytes,base);assert_eq!(runtime.mutations.registrations,0);assert!(runtime.mutations.targets.is_empty()&&runtime.mutations.observers[&0].targets.is_empty());
 runtime.limits.heap.retained_values=base+expected;runtime.install_mutation_registration(0,&1,registration(true)).unwrap();assert_eq!(runtime.value_bytes,base+expected);assert_eq!(runtime.mutations.registrations,1);let before=runtime.value_bytes;let smaller=registration(false);let size=smaller.retained_bytes();runtime.limits.heap.retained_values=before;runtime.install_mutation_registration(0,&1,smaller).unwrap();assert_eq!(runtime.value_bytes,base+size);assert_eq!(runtime.mutations.registrations,1);let names=runtime.mutations.targets[&1][0].attribute_filter.as_ref().unwrap();assert_eq!(names.len(),1);assert_eq!(names[0],"data-next");runtime.mutation_observer_builtin(Op::Disconnect,&[],Value::Object(0),&mut Tree).unwrap();assert_eq!(runtime.value_bytes,base);assert_eq!(runtime.mutations.registrations,0);assert!(runtime.mutations.targets.is_empty());
}
