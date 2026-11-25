//! Native OUTPUT reset shares admitted real child-list observations.
use super::*;
impl Runtime{
 pub(crate) fn reset_native_output(&mut self,target:u32,host:&mut dyn Host)->Result<(),String>{
  self.reset_native_output_inner(target,host).map_err(|error|error.message)
 }
 pub(super) fn reset_native_output_inner(&mut self,target:u32,host:&mut dyn Host)->Result<(),Fault>{
  self.validate_native_access(target,host)?;
  self.reject_foreign_window(&Value::Host(target),host)?;
  let value=host.get(target,"defaultValue").map_err(Fault::new)?;
  let value=self.checked_value(value)?;
  let ticket=self.prepare_host_microtask(host,target,"textContent",std::slice::from_ref(&value),true)?;
  let observation=match self.prepare_setter_observation(target,"textContent",&value,None,host){
   Ok((pending,_))=>pending,Err(error)=>{self.finish_host_microtask(host,ticket)?;return Err(error);}
  };
  let result=host.reset_output_text(target,value).map_err(Fault::new);
  let records=self.finish_tree_observation(observation,host);
  let effects=self.drain_native_promise_effects(host);
  if effects.as_ref().is_ok_and(|count|*count!=0){self.defer_host_microtask_after_effects(ticket)?;}
  let finish=self.finish_host_microtask(host,ticket);
  records?;effects?;finish?;result
 }
}
