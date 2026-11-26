//! Native NodeIterator factory and reference state.
//! https://dom.spec.whatwg.org/#interface-nodeiterator
use super::*;
use std::collections::HashMap;
#[path="node_iterator_live.rs"]
pub(super) mod live;
use live::Pointer;

#[derive(Clone,Copy)]
pub(super) enum Op { Interface,Create,Root,Reference,Before,Mask,Filter,Detach,Next,Previous }
impl Op { pub(super) fn arity(self)->usize {usize::from(matches!(self,Self::Create))} }

#[derive(Clone,Copy)]
enum Filter { Object(usize),Function(usize),Host(u32) }
impl Filter {
 fn value(self)->Value {match self {Self::Object(id)=>Value::Object(id),Self::Function(id)=>Value::Function(id),Self::Host(id)=>Value::Host(id)}}
}
#[derive(Clone,Copy)]
struct IteratorState { root:u32,reference:Pointer,candidate:Option<Pointer>,active:bool,mask:u32,filter:Option<Filter> }
#[derive(Default)]
pub(super) struct Arena {pub(super) prototype:Option<usize>,states:HashMap<usize,IteratorState>}
impl Arena {
 pub(super) fn has_states(&self)->bool{!self.states.is_empty()}
 pub(super) fn filter_value(&self,id:usize)->Option<Value>{self.states.get(&id).and_then(|state|state.filter).map(Filter::value)}
}
impl Runtime {
 pub(crate) fn define_node_iterator(&mut self,window:u32)->Result<(),String>{
  let marker=self.inhibit_transient_collection();
  let result=self.define_node_iterator_interface(window);
  self.restore_transient_marker(marker);result.map_err(Fault::display)
 }
 fn define_node_iterator_interface(&mut self,window:u32)->Result<(),Fault>{
  let prototype=self.new_object(None)?;
  let constructor=self.native_function(Builtin::NodeIterator(Op::Interface),"NodeIterator")?;
  self.put_binding(self.global_environment,"NodeIterator".into(),Some(constructor),Declaration::Var,false)?;
  self.global_attributes.insert("NodeIterator".into(),(false,true,true));
  self.constructor_prototype("NodeIterator",Value::Object(prototype))?;
  self.store_symbol_property(prototype,self.symbol_to_string_tag.unwrap(),Value::String("NodeIterator".into()),Some((false,false,true)))?;
  self.native_constructor_prototypes.insert((window,"NodeIterator"),prototype);
  if self.node_iterators.prototype.is_none(){self.node_iterators.prototype=Some(prototype);}
  for (name,op) in [("root",Op::Root),("referenceNode",Op::Reference),("pointerBeforeReferenceNode",Op::Before),("whatToShow",Op::Mask),("filter",Op::Filter)] {
   let Value::Function(get)=self.native_function(Builtin::NodeIterator(op),&format!("get {name}"))? else {unreachable!()};
   self.store_accessor_property(prototype,name.into(),Accessors{get:Some(get),set:None},(true,true))?;
  }
  for (name,op) in [("nextNode",Op::Next),("previousNode",Op::Previous),("detach",Op::Detach)] {
   self.deferred_native_method(prototype,name,Builtin::NodeIterator(op))?;
  }
  let document=*self.native_constructor_prototypes.get(&(window,"Document")).ok_or_else(||Fault::new("Document must precede NodeIterator."))?;
  self.deferred_native_method(document,"createNodeIterator",Builtin::NodeIterator(Op::Create))?;
  Ok(())
 }
 pub(super) fn node_iterator_builtin(&mut self,op:Op,args:&[Value],this:Value,host:&mut dyn Host)->Result<Value,Fault>{
  match op {Op::Interface=>return Err(Fault::exception("TypeError","Illegal NodeIterator constructor.")),Op::Create=>return self.create_node_iterator(args,this,host),_=>{}}
  let Value::Object(id)=this else {return Err(Self::node_iterator_brand_error())};
  let state=self.node_iterators.states.get(&id).copied().ok_or_else(Self::node_iterator_brand_error)?;
  self.validate_native_access(state.root,host)?;
  match op {
   Op::Root=>Ok(Value::Host(state.root)),
   Op::Reference=>{self.validate_native_access(state.reference.node,host)?;Ok(Value::Host(state.reference.node))},
   Op::Before=>Ok(Value::Bool(state.reference.before)),
   Op::Mask=>Ok(Value::Number(f64::from(state.mask))),
   Op::Filter=>Ok(state.filter.map_or(Value::Null,Filter::value)),
   Op::Detach=>Ok(Value::Undefined),
   Op::Next|Op::Previous=>self.traverse_node_iterator(id,matches!(op,Op::Next),host),
   _=>unreachable!()
  }
 }
 fn node_iterator_brand_error()->Fault{Fault::exception("TypeError","Incompatible NodeIterator receiver.")}
 fn create_node_iterator(&mut self,args:&[Value],this:Value,host:&mut dyn Host)->Result<Value,Fault>{
  let Value::Host(document)=this else {return Err(Fault::exception("TypeError","createNodeIterator requires its Document receiver."))};
  self.validate_native_access(document,host)?;
  if !host.tree_walker_document(document){return Err(Fault::exception("TypeError","createNodeIterator requires its Document receiver."))}
  let Some(Value::Host(root))=args.first() else {return Err(Fault::exception("TypeError","NodeIterator requires a Node root."))};
  let root=*root;
  self.validate_native_access(root,host)?;
  if host.tree_walker_node_type(root).is_none(){return Err(Fault::exception("TypeError","NodeIterator requires an accessible Node root."))}
  let mask=if let Some(value)=args.get(1).filter(|value|!matches!(value,Value::Undefined)) {
   let value=self.copy_value(value)?;int32(self.primitive_with_hint(value,false,host)?.script_number()?) as u32
  }else{u32::MAX};
  let filter=match args.get(2){
   None|Some(Value::Null|Value::Undefined)=>None,
   Some(Value::Object(id))=>Some(Filter::Object(*id)),Some(Value::Function(id))=>Some(Filter::Function(*id)),Some(Value::Host(id))=>Some(Filter::Host(*id)),
   _=>return Err(Fault::exception("TypeError","NodeIterator filter must be an object or null."))
  };
  // Numeric conversion can run author code. Recheck captured native authority
  // before publishing the stored state and its externally visible identity.
  self.validate_native_access(document,host)?;self.validate_native_access(root,host)?;
  if !host.tree_walker_document(document)||host.tree_walker_node_type(root).is_none(){return Err(Fault::exception("TypeError","NodeIterator native inputs are no longer accessible."))}
  let bytes=std::mem::size_of::<IteratorState>();
  let retained=self.value_bytes.checked_add(bytes).filter(|&total|total<=self.limits.heap.retained_values).ok_or_else(||Fault::new("Retained value limit reached for NodeIterator state."))?;
  self.work(bytes)?;
  self.node_iterators.states.try_reserve(1).map_err(|_|Fault::new("Cannot reserve NodeIterator state."))?;
  let id=self.new_object(None)?;
  let prototype=if let Some((window,_))=self.browser_global{self.native_constructor_prototypes.get(&(window,"NodeIterator")).copied()}else{self.node_iterators.prototype};
  self.objects[id].prototype=prototype.map(Ordinary::Object);
  self.node_iterators.states.insert(id,IteratorState{root,reference:Pointer{node:root,before:true},candidate:None,active:false,mask,filter});self.value_bytes=retained;
  Ok(Value::Object(id))
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 struct NoHost;
 impl Host for NoHost {
  fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("unexpected native read")}
  fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected native write")}
  fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("unexpected native call")}
  fn tree_walker_document(&self,id:u32)->bool{id==0}
  fn tree_walker_node_type(&self,id:u32)->Option<u32>{(id==1).then_some(1)}
 }
 #[test]
 fn node_iterator_state_exact_record_refusal_and_fresh_recovery(){
  for profile in 0..5 {
   let mut runtime=Runtime::for_capacity_test();
   let before=(runtime.value_bytes,runtime.objects.len(),runtime.node_iterators.states.len());
   let retained=runtime.limits.heap.retained_values;let objects=runtime.limits.heap.objects;let steps=runtime.steps;let copies=runtime.copy_bytes;let deadline=runtime.job_deadline;
   match profile {0=>runtime.limits.heap.retained_values=runtime.value_bytes,1=>runtime.copy_bytes=0,2=>runtime.steps=0,3=>runtime.limits.heap.objects=runtime.objects.len(),_=>{runtime.steps=4096;runtime.job_deadline=Instant::now()-Duration::from_secs(1)}}
   assert!(runtime.create_node_iterator(&[Value::Host(1)],Value::Host(0),&mut NoHost).is_err());
   assert_eq!((runtime.value_bytes,runtime.objects.len(),runtime.node_iterators.states.len()),before);
   runtime.limits.heap.retained_values=retained;runtime.limits.heap.objects=objects;runtime.steps=steps;runtime.copy_bytes=copies;runtime.job_deadline=deadline;
   let Value::Object(id)=runtime.create_node_iterator(&[Value::Host(1)],Value::Host(0),&mut NoHost).unwrap() else {panic!("native iterator")};
   assert_eq!(runtime.value_bytes-before.0,std::mem::size_of::<IteratorState>());
   assert_eq!(runtime.node_iterators.states.len(),before.2+1);
   let state=runtime.node_iterators.states[&id];assert_eq!((state.root,state.reference.node,state.reference.before,state.mask),(1,1,true,u32::MAX));
  }
 }
 #[test]
 fn node_iterator_state_filter_reachable_only_through_reachable_native_identity(){
  let mut runtime=Runtime::for_capacity_test();
  runtime.execute("function make(n){var box={};box.fn=function(){return n;};return box.fn;}var filter=make(37);0;",&mut NoHost).unwrap();
  let Value::Function(filter)=runtime.binding_value(runtime.global_environment,"filter").unwrap() else {panic!("function filter")};
  let held=runtime.create_node_iterator(&[Value::Host(1),Value::Number(1.0),Value::Function(filter)],Value::Host(0),&mut NoHost).unwrap();
  runtime.put_binding(runtime.global_environment,"held".into(),Some(held),Declaration::Var,true).unwrap();
  runtime.execute("filter=null;0;",&mut NoHost).unwrap();
  assert!(runtime.retire_proven_inactive_candidate(filter).unwrap_err().display().contains("reachable"));
  runtime.execute("held=null;0;",&mut NoHost).unwrap();
  runtime.retire_proven_inactive_candidate(filter).unwrap();assert!(!runtime.function_is_occupied(filter));
 }
 #[test]
 fn node_iterator_state_direct_return_closure_retention_matches_existing_walker(){
  let mut outcomes=Vec::new();
  for iterator in [false,true] {
   let mut runtime=Runtime::for_capacity_test();
   runtime.execute("function make(n){return function(){return n;};}var filter=make(37);0;",&mut NoHost).unwrap();
   let Value::Function(filter)=runtime.binding_value(runtime.global_environment,"filter").unwrap() else {panic!("function filter")};
   let args=[Value::Host(1),Value::Number(1.0),Value::Function(filter)];
   let held=if iterator {runtime.create_node_iterator(&args,Value::Host(0),&mut NoHost)} else {runtime.tree_walker_builtin(tree_walker::Op::Create,&args,Value::Host(0),&mut NoHost)}.unwrap();
   runtime.put_binding(runtime.global_environment,"held".into(),Some(held),Declaration::Var,true).unwrap();
   runtime.execute("filter=null;held=null;0;",&mut NoHost).unwrap();
   outcomes.push(runtime.retire_proven_inactive_candidate(filter).is_err());
  }
  assert_eq!(outcomes,vec![true,true],"direct-return closure retains the same existing conservative root");
 }
 #[test]
 fn node_iterator_state_author_conversion_revoked_root_authority_prevents_publication(){
  struct Guard{denied:bool}
  impl Host for Guard{
   fn get(&mut self,_:u32,_:&str)->Result<Value,String>{Ok(Value::Undefined)}
   fn method(&self,id:u32,key:&str)->Option<&'static str>{(id==1&&key=="revoke").then_some("revoke")}
   fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("unexpected native write")}
   fn call(&mut self,id:u32,key:&str,_:&[Value])->Result<Value,String>{assert_eq!((id,key),(1,"revoke"));self.denied=true;Ok(Value::Undefined)}
   fn tree_walker_document(&self,id:u32)->bool{id==0}
   fn tree_walker_node_type(&self,id:u32)->Option<u32>{(id==1).then_some(1)}
   fn native_access_owner(&self,id:u32)->Option<u32>{(id==1).then_some(9)}
   fn validate_window_access(&self,_:Option<u32>,_:u32)->Result<(),String>{if self.denied{Err("denied native owner".into())}else{Ok(())}}
  }
  let mut runtime=Runtime::for_capacity_test();let mut host=Guard{denied:false};
  runtime.put_binding(runtime.global_environment,"root".into(),Some(Value::Host(1)),Declaration::Var,true).unwrap();
  runtime.execute("var conversions=0;var mask={valueOf:function(){conversions++;root.revoke();return 1;}};0;",&mut host).unwrap();
  let mask=runtime.binding_value(runtime.global_environment,"mask").unwrap();let before=(runtime.value_bytes,runtime.objects.len(),runtime.node_iterators.states.len());
  let error=runtime.create_node_iterator(&[Value::Host(1),mask],Value::Host(0),&mut host).unwrap_err();
  assert!(error.display().contains("denied native owner"));assert_eq!(runtime.node_iterators.states.len(),before.2);
  assert_eq!(runtime.binding_value(runtime.global_environment,"conversions").unwrap().to_number(),1.0);
  host.denied=false;runtime.create_node_iterator(&[Value::Host(1)],Value::Host(0),&mut host).unwrap();
 }
}
