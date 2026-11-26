//! Native Symbol storage validates ownership and keeps allocation failures atomic.
use super::*;
struct Native{owner:Option<u32>,deny:bool}
impl Host for Native{
 fn symbol_property_owner(&self,object:u32)->Option<u32>{if object==42{self.owner}else{None}}
 fn instance_ancestry(&self,object:u32)->Option<super::super::NativeAncestry>{self.symbol_property_owner(object).map(|window|super::super::NativeAncestry{window,constructor:None})}
 fn validate_window_access(&self,_caller:Option<u32>,_target:u32)->Result<(),String>{if self.deny{Err("Native Symbol authority denied".into())}else{Ok(())}}
 fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Err("No native read".into())}
 fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{Err("No native write".into())}
 fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{Err("No native call".into())}
}
#[test]
fn dataset_symbol_properties_allocation_and_retained_refusal_publish_no_store_and_recover(){
 let mut r=Runtime::for_test();r.bind_browser_global(77).unwrap();let host=Native{owner:Some(77),deny:false};
 let objects=r.limits.heap.objects;let retained=r.limits.heap.retained_values;let before=r.objects.len();
 r.limits.heap.retained_values=r.value_bytes;
 let error=r.symbol_backing(&Value::Host(42),&host).unwrap_err();assert!(error.message.contains("Retained"),"{}",error.message);assert!(r.native_symbol_backings.is_empty());assert_eq!(r.objects.len(),before);
 r.limits.heap.retained_values=retained;r.limits.heap.objects=before;
 let error=r.symbol_backing(&Value::Host(42),&host).unwrap_err();assert!(error.message.contains("limit"),"{}",error.message);assert!(r.native_symbol_backings.is_empty());assert_eq!(r.objects.len(),before);
 r.limits.heap.objects=objects;let store=r.symbol_backing(&Value::Host(42),&host).unwrap().unwrap();let id=r.ordinary_id(store).unwrap();assert_eq!(r.native_symbol_backings.len(),1);assert!(!r.host_objects.contains_key(&42));assert!(r.objects[id].properties.is_empty());assert_eq!(r.symbol_backing(&Value::Host(42),&host).unwrap(),Some(store));
}
#[test]
fn dataset_symbol_properties_cached_capability_owner_and_access_revalidated(){
 let mut r=Runtime::for_test();r.bind_browser_global(77).unwrap();let mut host=Native{owner:Some(77),deny:false};r.define_global("data",Value::Host(42)).unwrap();r.execute("var key=Symbol();data[key]=17;",&mut host).unwrap();
 host.owner=None;for source in ["data[key];","key in data;","Object.getOwnPropertySymbols(data);","Object.getOwnPropertyDescriptor(data,key);","delete data[key];"]{r.begin_job();let error=r.execute(source,&mut host).unwrap_err();assert!(error.contains("expired"),"{source}: {error}");}
 host.owner=Some(88);host.deny=true;r.begin_job();let error=r.execute("data[key];",&mut host).unwrap_err();assert!(error.contains("authority denied"),"{error}");
 host.deny=false;r.begin_job();let error=r.execute("data[key];",&mut host).unwrap_err();assert!(error.contains("owner realm changed"),"{error}");
 host.owner=Some(77);r.begin_job();assert_eq!(r.execute("data[key];",&mut host).unwrap().to_number(),17.0);assert!(!r.host_objects.contains_key(&42));
}

#[test]
fn dataset_symbol_properties_value_storage_copy_and_deadline_refusals_preserve_existing_symbol(){
 let mut r=Runtime::for_test();r.bind_browser_global(77).unwrap();let mut host=Native{owner:Some(77),deny:false};r.define_global("data",Value::Host(42)).unwrap();r.execute("var key=Symbol();data[key]=17;",&mut host).unwrap();
 let backing=r.symbol_backing(&Value::Host(42),&host).unwrap().unwrap();let id=r.ordinary_id(backing).unwrap();let key=*r.objects[id].symbol_properties.keys().next().unwrap();let retained=r.limits.heap.retained_values;
 r.limits.heap.retained_values=r.value_bytes;r.begin_job();let error=r.assign_symbol_property(Value::Host(42),key,Value::String("x".repeat(16000)),&mut host).unwrap_err();assert!(error.message.to_lowercase().contains("retained"),"{}",error.message);assert_eq!(r.objects[id].symbol_properties[&key].value.as_value().unwrap().to_number(),17.0);
 r.limits.heap.retained_values=retained;
 for deadline in [false,true]{r.begin_job();if deadline{r.steps=256;r.job_deadline=std::time::Instant::now();}else{r.copy_bytes=0;}
  let error=r.assign_symbol_property(Value::Host(42),key,Value::Number(19.0),&mut host).unwrap_err();assert!(error.message.contains(if deadline{"Execution time"}else{"String/copy work"}),"{}",error.message);assert_eq!(r.objects[id].symbol_properties[&key].value.as_value().unwrap().to_number(),17.0);assert_eq!(r.native_symbol_backings.len(),1);assert!(!r.host_objects.contains_key(&42));
 }
 r.begin_job();r.assign_symbol_property(Value::Host(42),key,Value::Number(23.0),&mut host).unwrap();assert_eq!(r.get_symbol_property(Value::Host(42),key,&mut host).unwrap().to_number(),23.0);
}
