//! Canonical SubmitEvent interface over original native event records.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#the-submitevent-interface
use super::{Fault,Function,Ordinary,Runtime,Value};
impl Runtime{
 pub fn define_submit_event_interface(&mut self,window:u32)->Result<(),String>{
  let marker=self.inhibit_transient_collection();
  let result=(||{
   let parent=*self.native_constructor_prototypes.get(&(window,"Event")).ok_or_else(||Fault::new("SubmitEvent requires the canonical Event interface."))?;
   let constructor=self.objects[parent].properties.get("constructor").ok_or_else(||Fault::new("Canonical Event constructor is unavailable."))?.value.as_value()?.clone();
   let Value::Function(base)=constructor else{return Err(Fault::new("Canonical Event constructor is invalid."))};
   let allocation=self.allocate_host_function_with_length(window,"SubmitEvent",1).map_err(Fault::new)?;
   if let Some(Function::Host{constructible,..})=self.functions.get_mut(allocation.id){*constructible=true;}
   let constructor=Value::Function(allocation.id);
   self.objects[self.function_properties[allocation.id]].prototype=Some(Ordinary::Function(base));
   let prototype=self.new_object(None)?;self.objects[prototype].prototype=Some(Ordinary::Object(parent));
   self.store_property(prototype,"constructor".into(),constructor.clone(),Some((false,true,true)))?;
   self.store_property(self.function_properties[allocation.id],"prototype".into(),Value::Object(prototype),Some((false,false,false)))?;
   self.define_host_attribute(prototype,window,"submitter","\0SubmitEvent.get.submitter",None)?;
   self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("SubmitEvent".into()),Some((false,false,true)))?;
   self.native_constructor_prototypes.insert((window,"SubmitEvent"),prototype);
   self.put_binding(self.global_environment,"SubmitEvent".into(),Some(constructor),super::Declaration::Var,false)?;
   self.global_attributes.insert("SubmitEvent".into(),(false,true,true));Ok(())
  })();self.restore_transient_marker(marker);result.map_err(Fault::display)
 }
}
