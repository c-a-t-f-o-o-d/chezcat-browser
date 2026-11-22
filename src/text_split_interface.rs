//! Canonical splitText transaction: staged native storage and ordered records.
use super::*;
pub(crate) const METHOD:&str="\0Text.splitText";
impl Runtime {
 pub(super) fn split_native_text(&mut self,target:u32,args:&[Value],host:&mut dyn Host)->Result<Value,Fault>{
  self.validate_native_access(target,host)?;self.reject_foreign_window(&Value::Host(target),host)?;
  let info=host.prepare_text_split(target,args).map_err(|(kind,message)|if matches!(kind,"IndexSizeError"|"HierarchyRequestError"|"NotSupportedError"){self.native_dom_exception(kind,&message).map(Fault::thrown).unwrap_or_else(|error|error)}else{Fault::new(message)})?;
  let ticket=match self.prepare_host_microtask(host,target,METHOD,args,false){Ok(ticket)=>ticket,Err(error)=>{host.abort_text_split(target);return Err(error);}};
  let mut tree=None;let mut data=None;
  let result=(||->Result<Value,Fault>{
   if let Some(generation)=host.mutation_tree_generation(){let parent=info.changes.first().map_or(target,|change|change.target);tree=self.prepare_native_child_changes(parent,generation,info.changes,host)?;}
   data=self.prepare_character_data_write(target,"data",host)?;
   let node=host.commit_text_split(target).map_err(Fault::new)?;
   if node!=info.node{return Err(Fault::new("splitText changed its admitted native identity."));}Ok(Value::Host(node))
  })();
  host.abort_text_split(target);
  let tree_result=self.finish_tree_observation(tree,host);let data_result=self.finish_character_data_write(data,"data",host);
  let effects=self.drain_native_promise_effects(host);if effects.as_ref().is_ok_and(|count|*count!=0){self.defer_host_microtask_after_effects(ticket)?;}
  let ticket_result=self.finish_host_microtask(host,ticket);
  tree_result?;data_result?;effects?;ticket_result?;result
 }
}
