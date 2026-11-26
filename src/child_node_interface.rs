//! Native ChildNode operations retain viable references across staging moves.
//! https://dom.spec.whatwg.org/#interface-childnode
use super::*;
pub(crate) const BEFORE:&str="\0ChildNode.before";
pub(crate) const AFTER:&str="\0ChildNode.after";
pub(crate) const REPLACE:&str="\0ChildNode.replaceWith";
pub(crate) fn operation(method:&str)->bool{matches!(method,BEFORE|AFTER|REPLACE)}
impl Runtime{
 pub(super) fn define_child_node_methods(&mut self,prototype:usize,window:u32)->Result<(),Fault>{
  for(name,method)in [("before",BEFORE),("after",AFTER),("replaceWith",REPLACE)]{self.deferred_host_method(prototype,window,name,method,0)?;}
  self.define_node_unscopables(prototype,&["before","after","replaceWith","remove"])
 }
 pub(super) fn child_node_variadic_call(&mut self,object:u32,method:&str,args:&[Value],host:&mut dyn Host)->Result<Value,Fault>{
  host.native_operation_caller(self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(w,_)|w)));
  host.validate_call(object,method,&[]).map_err(|(kind,message)|Fault::exception(kind,message))?;
  let converted=self.convert_node_or_string_arguments(args,host)?;
  self.recheck_variadic_nodes(object,method,&converted,host)?;
  // Bindings convert even for detached receivers; the DOM algorithm then
  // returns before creating Text nodes or a staging fragment.
  let parent=host.get(object,"parentNode").map_err(Fault::new)?;
  let Value::Host(parent)=parent else{if matches!(parent,Value::Null){return Ok(Value::Undefined);}return Err(Fault::new("Native ChildNode parent is unavailable."));};
  self.validate_native_access(parent,host)?;
  let direction=if method==BEFORE{"previousSibling"}else{"nextSibling"};
  let mut viable=host.get(object,direction).map_err(Fault::new)?;
  loop{
   self.work(converted.len().saturating_add(1))?;
   let Value::Host(sibling)=viable else{if !matches!(viable,Value::Null){return Err(Fault::new("Native ChildNode sibling is unavailable."));}break;};
   if !converted.iter().any(|v|matches!(v,Value::Host(node) if *node==sibling)){break;}
   viable=host.get(sibling,direction).map_err(Fault::new)?;
  }
  let node=self.construct_variadic_node(object,converted,host)?;
  if method==BEFORE{
   viable=match viable{Value::Null=>host.get(parent,"firstChild").map_err(Fault::new)?,Value::Host(previous)=>host.get(previous,"nextSibling").map_err(Fault::new)?,_=>unreachable!()};
  }
  let replace=method==REPLACE&&matches!(host.get(object,"parentNode").map_err(Fault::new)?,Value::Host(current) if current==parent);
  if replace{self.call_host(parent,"replaceChild",&[node,Value::Host(object)],host)?;}
  else{self.pre_insert_variadic_node(parent,node,viable,host)?;}
  Ok(Value::Undefined)
 }
}
