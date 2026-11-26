//! Flat native traversal and synchronous reference/candidate removal adjustment.
//! https://dom.spec.whatwg.org/#interface-nodeiterator
use super::*;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) struct Pointer {pub(super) node:u32,pub(super) before:bool}
pub(in crate::javascript) struct RemovalPlan {updates:Vec<(usize,u32,Pointer,Option<Pointer>)>}
impl RemovalPlan {
 pub(in crate::javascript) fn retained_bytes(&self)->usize{std::mem::size_of::<Self>()+self.updates.capacity()*std::mem::size_of::<(usize,u32,Pointer,Option<Pointer>)>()}
}

#[cfg(test)]
mod tests {
 use super::*;
 struct NativeTree {removed:bool,generation:u64,deny_child:bool}
 impl Host for NativeTree {
  fn get(&mut self,id:u32,key:&str)->Result<Value,String>{
   Ok(match (id,key){(1,"firstChild"|"lastChild") if !self.removed=>Value::Host(2),(2,"parentNode") if !self.removed=>Value::Host(1),_=>Value::Null})
  }
  fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected native setter")}
  fn method(&self,id:u32,key:&str)->Option<&'static str>{(id==2&&key=="revoke").then_some("revoke")}
  fn call(&mut self,id:u32,key:&str,_:&[Value])->Result<Value,String>{assert_eq!((id,key),(2,"revoke"));self.deny_child=true;Ok(Value::Undefined)}
  fn tree_walker_document(&self,id:u32)->bool{id==0}
  fn tree_walker_node_type(&self,id:u32)->Option<u32>{matches!(id,1|2).then_some(1)}
  fn mutation_tree_generation(&self)->Option<u64>{Some(self.generation)}
  fn native_access_owner(&self,id:u32)->Option<u32>{(id==2).then_some(9)}
  fn validate_window_access(&self,_:Option<u32>,_:u32)->Result<(),String>{if self.deny_child{Err("denied native child owner".into())}else{Ok(())}}
 }
 fn change()->Vec<NativeChildListChange>{vec![NativeChildListChange{target:1,added:Vec::new(),removed:vec![2],previous:None,next:None}]}
 #[test]
 fn node_iterator_live_prepared_storage_refusal_rollback_commit_and_no_observer_microtask(){
  for profile in 0..4 {
   let mut runtime=Runtime::for_capacity_test();let mut host=NativeTree{removed:false,generation:0,deny_child:false};
   let Value::Object(id)=runtime.create_node_iterator(&[Value::Host(1)],Value::Host(0),&mut host).unwrap() else{panic!("iterator")};
   runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap();runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap();
   let before=(runtime.value_bytes,runtime.node_iterators.states[&id].reference);
   let (limit,copies,steps,deadline)=(runtime.limits.heap.retained_values,runtime.copy_bytes,runtime.steps,runtime.job_deadline);
   match profile{0=>runtime.limits.heap.retained_values=runtime.value_bytes,1=>runtime.copy_bytes=0,2=>runtime.steps=0,_=>{runtime.steps=4096;runtime.job_deadline=Instant::now()-Duration::from_secs(1)}}
   assert!(runtime.prepare_native_child_changes(1,0,change(),&mut host).is_err());
   assert_eq!((runtime.value_bytes,runtime.node_iterators.states[&id].reference),before);
   runtime.limits.heap.retained_values=limit;runtime.copy_bytes=copies;runtime.steps=steps;runtime.job_deadline=deadline;
   let pending=runtime.prepare_native_child_changes(1,0,change(),&mut host).unwrap();assert!(runtime.value_bytes>before.0);
   runtime.finish_tree_observation(pending,&host).unwrap();assert_eq!((runtime.value_bytes,runtime.node_iterators.states[&id].reference),before);
   let pending=runtime.prepare_native_child_changes(1,0,change(),&mut host).unwrap();host.removed=true;host.generation=1;
   runtime.finish_tree_observation(pending,&host).unwrap();let state=runtime.node_iterators.states[&id];
   assert_eq!(runtime.value_bytes,before.0);assert_eq!((state.reference.node,state.reference.before),(1,false));assert!(state.candidate.is_none()&&!state.active);
   assert!(!runtime.has_queued_microtasks());
  }
 }
 #[test]
 fn node_iterator_live_denied_candidate_owner_precedes_callback_and_reference_publication(){
  let mut runtime=Runtime::for_capacity_test();let mut host=NativeTree{removed:false,generation:0,deny_child:false};
  runtime.execute("var calls=0;function filter(){calls++;return 1;}0;",&mut host).unwrap();
  let filter=runtime.binding_value(runtime.global_environment,"filter").unwrap();
  let Value::Object(id)=runtime.create_node_iterator(&[Value::Host(1),Value::Number(1.0),filter],Value::Host(0),&mut host).unwrap() else{panic!("iterator")};
  runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap();host.deny_child=true;
  let error=runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap_err();assert!(error.display().contains("denied native child owner"));
  let state=runtime.node_iterators.states[&id];assert_eq!((state.reference.node,state.reference.before),(1,false));assert!(state.candidate.is_none()&&!state.active);
  assert_eq!(runtime.binding_value(runtime.global_environment,"calls").unwrap().to_number(),1.0);
  host.deny_child=false;let recovered=runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap();assert!(matches!(recovered,Value::Host(2)));
  assert_eq!(runtime.binding_value(runtime.global_environment,"calls").unwrap().to_number(),2.0);
  let mut runtime=Runtime::for_capacity_test();let mut host=NativeTree{removed:false,generation:0,deny_child:false};
  runtime.execute("var calls=0;function filter(n){calls++;if(calls===2)n.revoke();return 1;}0;",&mut host).unwrap();
  let filter=runtime.binding_value(runtime.global_environment,"filter").unwrap();
  let Value::Object(id)=runtime.create_node_iterator(&[Value::Host(1),Value::Number(1.0),filter],Value::Host(0),&mut host).unwrap() else{panic!("iterator")};
  runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap();
  let error=runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap_err();assert!(error.display().contains("denied native child owner"));
  let state=runtime.node_iterators.states[&id];assert_eq!((state.reference.node,state.reference.before),(1,false));assert!(state.candidate.is_none()&&!state.active);
  assert_eq!(runtime.binding_value(runtime.global_environment,"calls").unwrap().to_number(),2.0);
  host.deny_child=false;assert!(matches!(runtime.node_iterator_builtin(Op::Next,&[],Value::Object(id),&mut host).unwrap(),Value::Host(2)));
  assert_eq!(runtime.binding_value(runtime.global_environment,"calls").unwrap().to_number(),3.0);
 }
}
impl Runtime {
 fn iterator_link(&mut self,node:u32,key:&str,host:&mut dyn Host)->Result<Option<u32>,Fault>{
  self.step()?;
  match self.host_get_checked(node,key,host)?{Value::Host(node)=>Ok(Some(node)),Value::Null=>Ok(None),_=>Err(Fault::new("NodeIterator encountered an unrepresented native relationship."))}
 }
 fn iterator_virtual_link(&mut self,node:u32,key:&str,removed:&[u32],host:&mut dyn Host)->Result<Option<u32>,Fault>{
  if key=="parentNode"&&removed.contains(&node){return Ok(None);}
  let mut next=self.iterator_link(node,key,host)?;
  if matches!(key,"firstChild"|"lastChild"|"nextSibling"|"previousSibling"){
   let sibling=if matches!(key,"firstChild"|"nextSibling"){"nextSibling"}else{"previousSibling"};
   while let Some(candidate)=next{if !removed.contains(&candidate){break;}next=self.iterator_link(candidate,sibling,host)?;}
  }
  Ok(next)
 }
 fn iterator_ancestor(&mut self,ancestor:u32,mut node:u32,removed:&[u32],host:&mut dyn Host)->Result<bool,Fault>{
  loop{self.step()?;if node==ancestor{return Ok(true);}let Some(parent)=self.iterator_virtual_link(node,"parentNode",removed,host)? else{return Ok(false)};node=parent;}
 }
 fn iterator_following(&mut self,root:u32,mut node:u32,descend:bool,removed:&[u32],host:&mut dyn Host)->Result<Option<u32>,Fault>{
  if descend{if let Some(child)=self.iterator_virtual_link(node,"firstChild",removed,host)?{return Ok(Some(child));}}
  while node!=root{
   if let Some(sibling)=self.iterator_virtual_link(node,"nextSibling",removed,host)?{return Ok(Some(sibling));}
   let Some(parent)=self.iterator_virtual_link(node,"parentNode",removed,host)? else{return Ok(None)};node=parent;
  }
  Ok(None)
 }
 fn iterator_preceding(&mut self,root:u32,node:u32,removed:&[u32],host:&mut dyn Host)->Result<Option<u32>,Fault>{
  if node==root{return Ok(None);}
  if let Some(mut previous)=self.iterator_virtual_link(node,"previousSibling",removed,host)?{
   while let Some(child)=self.iterator_virtual_link(previous,"lastChild",removed,host)?{previous=child;}return Ok(Some(previous));
  }
  self.iterator_virtual_link(node,"parentNode",removed,host)
 }
 fn iterator_step_pointer(&mut self,root:u32,pointer:Pointer,forward:bool,host:&mut dyn Host)->Result<Option<Pointer>,Fault>{
  self.validate_native_access(pointer.node,host)?;
  if pointer.before==forward{return Ok(Some(Pointer{node:pointer.node,before:!forward}));}
  let node=if forward{self.iterator_following(root,pointer.node,true,&[],host)?}else{self.iterator_preceding(root,pointer.node,&[],host)?};
  Ok(node.map(|node|Pointer{node,before:!forward}))
 }
 fn filter_node_iterator(&mut self,id:usize,node:u32,host:&mut dyn Host)->Result<u16,Fault>{
  let state=self.node_iterators.states[&id];
  if state.active{return Err(Fault::thrown(self.native_dom_exception("InvalidStateError","NodeIterator filter is already active.")?));}
  self.validate_native_access(node,host)?;
  let kind=host.tree_walker_node_type(node).ok_or_else(||Fault::new("NodeIterator encountered an inaccessible native node."))?;
  let bit=kind.checked_sub(1).and_then(|shift|1u32.checked_shl(shift)).ok_or_else(||Fault::new("NodeIterator node type exceeds mask representation."))?;
  if state.mask&bit==0{return Ok(3);}let Some(filter)=state.filter else{return Ok(1)};
  self.node_iterators.states.get_mut(&id).unwrap().active=true;
  let result=(||{
   let callback=filter.value();
   let realm=if let Value::Function(function)=callback{self.function_realms.get(function).copied().unwrap_or(self.active_realm)}else{self.callback_interface_object_realm(&callback,host)?};
   self.with_intrinsic_realm(realm,|runtime|{
    let (function,this)=if matches!(callback,Value::Function(_)){(callback,Value::Undefined)}else{(runtime.read_property(&callback,"acceptNode",host)?,callback)};
    let result=runtime.call_value(function,&[Value::Host(node)],this,host)?;
    Ok(int32(runtime.primitive_with_hint(result,false,host)?.script_number()?) as u16)
   })
  })();
  self.node_iterators.states.get_mut(&id).unwrap().active=false;result
 }
 pub(in crate::javascript) fn traverse_node_iterator(&mut self,id:usize,forward:bool,host:&mut dyn Host)->Result<Value,Fault>{
  let state=self.node_iterators.states[&id];
  // A recursive traversal that cannot reach a candidate returns null without
  // disturbing the outer traversal. Filtering a candidate still rejects reentry.
  if state.active{
   if self.iterator_step_pointer(state.root,state.reference,forward,host)?.is_none(){return Ok(Value::Null);}
   return Err(Fault::thrown(self.native_dom_exception("InvalidStateError","NodeIterator filter is already active.")?));
  }
  self.node_iterators.states.get_mut(&id).unwrap().candidate=Some(state.reference);
  let result=(||{
   loop{
    self.step()?;self.validate_native_access(state.root,host)?;
    let pointer=self.node_iterators.states[&id].candidate.ok_or_else(||Fault::new("NodeIterator candidate was lost."))?;
    let Some(pointer)=self.iterator_step_pointer(state.root,pointer,forward,host)? else{return Ok(Value::Null)};
    self.node_iterators.states.get_mut(&id).unwrap().candidate=Some(pointer);
    let node=pointer.node;
    if self.filter_node_iterator(id,node,host)?==1{
     let adjusted=self.node_iterators.states[&id].candidate.ok_or_else(||Fault::new("NodeIterator candidate was lost after filtering."))?;
     self.validate_native_access(state.root,host)?;self.validate_native_access(node,host)?;self.validate_native_access(adjusted.node,host)?;
     self.node_iterators.states.get_mut(&id).unwrap().reference=adjusted;return Ok(Value::Host(node));
    }
   }
  })();
  self.node_iterators.states.get_mut(&id).unwrap().candidate=None;result
 }
 fn iterator_adjust_pointer(&mut self,root:u32,pointer:Pointer,to_remove:u32,removed:&[u32],host:&mut dyn Host)->Result<Pointer,Fault>{
  if !self.iterator_ancestor(to_remove,pointer.node,removed,host)?||self.iterator_ancestor(to_remove,root,removed,host)?{return Ok(pointer);}
  if pointer.before{if let Some(node)=self.iterator_following(root,to_remove,false,removed,host)?{return Ok(Pointer{node,before:true});}}
  let node=self.iterator_preceding(root,to_remove,removed,host)?.ok_or_else(||Fault::new("NodeIterator removal predecessor is unavailable."))?;
  Ok(Pointer{node,before:false})
 }
 pub(in crate::javascript) fn prepare_iterator_removals<'a,I>(&mut self,changes:I,host:&mut dyn Host)->Result<Option<RemovalPlan>,Fault>
 where I:Iterator<Item=&'a NativeChildListChange>+Clone{
  if !self.node_iterators.has_states(){return Ok(None);}
  let count=changes.clone().try_fold(0usize,|count,change|count.checked_add(change.removed.len())).ok_or_else(||Fault::new("NodeIterator removal count overflow."))?;
  if count==0{return Ok(None);}
  let entries=self.node_iterators.states.len();
  let bytes=entries.checked_mul(std::mem::size_of::<(usize,u32,Pointer,Option<Pointer>)>()).and_then(|bytes|count.checked_mul(std::mem::size_of::<u32>()).and_then(|removed|bytes.checked_add(removed))).ok_or_else(||Fault::new("NodeIterator removal storage overflow."))?;
  if self.value_bytes.checked_add(bytes).is_none_or(|total|total>self.limits.heap.retained_values){return Err(Fault::new("Retained value limit reached for NodeIterator removal preparation."));}
  self.work(bytes)?;let mut updates=Vec::new();updates.try_reserve_exact(entries).map_err(|_|Fault::new("Cannot reserve NodeIterator position proposals."))?;
  updates.extend(self.node_iterators.states.iter().map(|(&id,state)|(id,state.root,state.reference,state.candidate)));
  let mut removed=Vec::new();removed.try_reserve_exact(count).map_err(|_|Fault::new("Cannot reserve NodeIterator virtual removals."))?;
  for change in changes{
   for &node in &change.removed{
    self.step()?;
    for (_,root,reference,candidate) in &mut updates{
     *reference=self.iterator_adjust_pointer(*root,*reference,node,&removed,host)?;
     if let Some(pointer)=candidate{*pointer=self.iterator_adjust_pointer(*root,*pointer,node,&removed,host)?;}
    }
    removed.push(node);
   }
  }
  Ok(Some(RemovalPlan{updates}))
 }
 pub(in crate::javascript) fn commit_iterator_removals(&mut self,plan:RemovalPlan){
  for (id,_,reference,candidate) in plan.updates{
   let state=self.node_iterators.states.get_mut(&id).expect("Prepared NodeIterator identity remains allocated.");
   state.reference=reference;state.candidate=candidate;
  }
 }
}
