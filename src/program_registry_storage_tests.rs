//! Original production registry backing, liveness and publication acceptance.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self)->Result<bool,String>{Ok(true)}
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("Unexpected host get")}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("Unexpected host set")}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("Unexpected host call")}
}
#[test]
fn browser_registry_keeps_six_thousand_live_ordinary_and_dynamic_programs_and_real_function_identity() {
    let mut runtime=Runtime::new();let mut ordinary=Vec::new();let mut dynamic=Vec::new();
    for i in 0..6000 {
        runtime.begin_job();ordinary.push(runtime.execute(&format!("(function(){{return {i};}});"),&mut NoHost).unwrap());
    }
    assert!(runtime.programs.len()>4096);
    for i in 0..6000 {
        runtime.begin_job();dynamic.push(runtime.execute(&format!("new Function('return {i};');"),&mut NoHost).unwrap());
    }
    assert!(runtime.programs.iter().filter(|program|program.templates.iter().any(|template|template.dynamic_source)).count()>=6000);
    assert!(runtime.programs.len()>=12000);
    for (i,(ordinary,dynamic)) in ordinary.into_iter().zip(dynamic).enumerate() {
        runtime.begin_job();assert_eq!(runtime.invoke(ordinary,&[],Value::Undefined,&mut NoHost).unwrap().to_number(),i as f64);
        runtime.begin_job();assert_eq!(runtime.invoke(dynamic,&[],Value::Undefined,&mut NoHost).unwrap().to_number(),i as f64);
    }
    assert!(runtime.value_bytes>=runtime.programs.capacity()*std::mem::size_of::<Rc<Program>>());
    assert!(runtime.value_bytes<=runtime.limits.heap.retained_values);
}
#[test]
fn browser_registry_exact_slot_copy_and_retention_refusal_publish_no_program_or_regexp_charge() {
    let mut runtime=Runtime::new();let baseline=runtime.value_bytes;let limit=runtime.limits.heap.retained_values;
    let slot=std::mem::size_of::<Rc<Program>>();runtime.limits.heap.retained_values=baseline+slot-1;
    assert!(runtime.register_program("1;",false).err().unwrap().message.contains("Retained value arena"));
    assert_eq!((runtime.programs.len(),runtime.programs.capacity(),runtime.retained_source,runtime.retained_ast,runtime.retained_regexp,runtime.value_bytes),(0,0,0,0,0,baseline));
    runtime.limits.heap.retained_values+=1;runtime.begin_job();
    let first=runtime.register_program("1;",false).unwrap();
    assert_eq!((runtime.programs.len(),runtime.programs.capacity(),runtime.value_bytes),(1,1,baseline+slot));
    let before=(runtime.programs.len(),runtime.programs.capacity(),runtime.retained_source,runtime.retained_ast,runtime.retained_regexp,runtime.value_bytes);
    runtime.limits.heap.retained_values=limit;runtime.begin_job();runtime.copy_bytes="/a/;".len()+slot-1;
    assert!(runtime.register_program("/a/;",false).err().unwrap().message.contains("work limit"));
    assert_eq!((runtime.programs.len(),runtime.programs.capacity(),runtime.retained_source,runtime.retained_ast,runtime.retained_regexp,runtime.value_bytes),before);
    assert_eq!(runtime.copy_bytes,0);
    runtime.begin_job();let second=runtime.register_program("/a/;",false).unwrap();
    assert_eq!(runtime.programs.len(),2);assert!(runtime.retained_regexp>before.4);
    assert_eq!(runtime.value_bytes,baseline+runtime.programs.capacity()*slot);
    drop((first,second));
}
#[test]
fn browser_registry_retires_dead_programs_on_backing_pressure_and_reuses_paid_capacity() {
    let mut runtime=Runtime::new();let baseline=runtime.value_bytes;
    for _ in 0..6000 {
        runtime.begin_job();assert_eq!(runtime.execute("1+1;",&mut NoHost).unwrap().to_number(),2.0);
    }
    assert!(runtime.program_allocation.retired>=5996);
    assert!(runtime.programs.len()<=runtime.programs.capacity());assert!(runtime.programs.capacity()<=4);
    assert_eq!(runtime.value_bytes,baseline+runtime.programs.capacity()*std::mem::size_of::<Rc<Program>>());
    let capacity=runtime.programs.capacity();let before=runtime.value_bytes;
    runtime.begin_job();let live=runtime.register_program("42;",false).unwrap();
    assert_eq!(runtime.programs.capacity(),capacity);assert_eq!(runtime.value_bytes,before);
    drop(live);
}
