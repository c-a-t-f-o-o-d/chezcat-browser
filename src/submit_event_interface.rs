//! Canonical SubmitEvent interface over original native event records.
//! https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#the-submitevent-interface
use super::{Fault,Ordinary,Runtime,Value};
impl Runtime{
 pub fn define_submit_event_interface(&mut self,window:u32)->Result<(),String>{
  let marker=self.inhibit_transient_collection();
  let result=(||{
   let parent=*self.native_constructor_prototypes.get(&(window,"Event")).ok_or_else(||Fault::new("SubmitEvent requires the canonical Event interface."))?;
   let constructor=self.objects[parent].properties.get("constructor").ok_or_else(||Fault::new("Canonical Event constructor is unavailable."))?.value.as_value()?.clone();
   let Value::Function(base)=constructor else{return Err(Fault::new("Canonical Event constructor is invalid."))};
   let prototype=self.new_object(None)?;self.objects[prototype].prototype=Some(Ordinary::Object(parent));
   self.defer_submit_event_constructor(window,prototype,base)?;
   self.define_host_attribute(prototype,window,"submitter","\0SubmitEvent.get.submitter",None)?;
   self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("SubmitEvent".into()),Some((false,false,true)))?;
   self.native_constructor_prototypes.insert((window,"SubmitEvent"),prototype);
   self.global_attributes.insert("SubmitEvent".into(),(false,true,true));Ok(())
  })();self.restore_transient_marker(marker);result.map_err(Fault::display)
 }
}
