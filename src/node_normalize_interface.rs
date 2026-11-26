//! Native normalization admission and ordered observer records.
use super::*;
pub(crate) const METHOD:&str="\0Node.normalize";
impl Runtime{
 pub(super) fn normalize_native_node(&mut self,target:u32,host:&mut dyn Host)->Result<Value,Fault>{
  self.validate_native_access(target,host)?;self.reject_foreign_window(&Value::Host(target),host)?;
  let info=host.prepare_node_normalize(target).map_err(Fault::new)?;
  let ticket=match self.prepare_host_microtask(host,target,METHOD,&[],false){Ok(ticket)=>ticket,Err(error)=>{host.abort_node_normalize(target);return Err(error);}};
  let mut pending=Vec::new();let result=(||->Result<Value,Fault>{
   pending.try_reserve_exact(info.events.len().checked_add(1).ok_or_else(||Fault::new("Normalization observation count overflow."))?).map_err(|_|Fault::new("Cannot reserve normalization observations."))?;
   if let Some(generation)=host.mutation_tree_generation(){let change=self.prepare_normalize_iterator_changes(target,generation,&info.events,host)?;pending.push((change,true));}
   for event in info.events{match event{NativeNormalizeEvent::Data(node)=>{let change=self.prepare_character_data_write(node,"data",host)?;pending.push((change,false));},NativeNormalizeEvent::Child(change)=>{let node=change.target;let mut changes=Vec::new();changes.try_reserve_exact(1).map_err(|_|Fault::new("Cannot reserve normalization removal record."))?;changes.push(change);let change=if let Some(generation)=host.mutation_tree_generation(){self.prepare_normalize_child_changes(node,generation,changes,host)?}else{None};pending.push((change,true));}}}
   host.commit_node_normalize(target).map_err(Fault::new)?;Ok(Value::Undefined)
  })();host.abort_node_normalize(target);let mut cleanup=None;
  for(change,tree)in pending{let finish=if tree{self.finish_tree_observation(change,host)}else{self.finish_character_data_write(change,"data",host)};if let Err(error)=finish{if cleanup.is_none(){cleanup=Some(error);}}}
  let effects=self.drain_native_promise_effects(host);if effects.as_ref().is_ok_and(|count|*count!=0){self.defer_host_microtask_after_effects(ticket)?;}let finish=self.finish_host_microtask(host,ticket);if let Some(error)=cleanup{return Err(error);}effects?;finish?;result
 }
}
