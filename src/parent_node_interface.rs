//! ParentNode binding conversions precede native DOM construction and mutation.
//! https://dom.spec.whatwg.org/#interface-parentnode
use super::*;
pub(crate) const APPEND:&str="\0ParentNode.append";
pub(crate) const PREPEND:&str="\0ParentNode.prepend";
pub(crate) const REPLACE:&str="\0ParentNode.replaceChildren";
pub(crate) const REPLACE_ALL:&str="\0ParentNode.replaceAllNative";
pub(crate) fn operation(method:&str)->bool{matches!(method,APPEND|PREPEND|REPLACE)}
impl Runtime {
 pub(super) fn define_parent_node_methods(&mut self,prototype:usize,window:u32)->Result<(),Fault>{
  let blocked=self.new_object(None)?;self.objects[blocked].prototype=None;
  for(name,method)in [("append",APPEND),("prepend",PREPEND),("replaceChildren",REPLACE)]{
   self.deferred_host_method(prototype,window,name,method,0)?;
   self.store_property(blocked,name.into(),Value::Bool(true),Some((true,true,true)))?;
  }
  self.store_symbol_property(prototype,self.symbol_unscopables.unwrap(),Value::Object(blocked),Some((false,false,true)))
 }
 fn parent_insertion_check(&mut self,parent:u32,node:u32,host:&mut dyn Host)->Result<(),Fault>{
  host.native_operation_caller(self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(w,_)|w)));
  match host.parent_insertion_validity(parent,node){Ok(())=>Ok(()),Err(("TypeError",message))=>Err(Fault::exception("TypeError",message)),Err((kind,message))=>Err(Fault::thrown(self.native_dom_exception(kind,&message)?))}
 }
 pub(super) fn parent_node_variadic_call(&mut self,object:u32,method:&str,args:&[Value],host:&mut dyn Host)->Result<Value,Fault>{
  let caller=self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(w,_)|w));
  host.native_operation_caller(caller);
  host.validate_call(object,method,&[]).map_err(|(kind,message)|Fault::exception(kind,message))?;
  self.work(args.len().checked_mul(std::mem::size_of::<Value>()).ok_or_else(||Fault::new("ParentNode argument size overflow."))?)?;
  let mut converted=Vec::new();converted.try_reserve_exact(args.len()).map_err(|_|Fault::new("Cannot reserve ParentNode arguments."))?;
  // Web IDL union conversion is completed for the entire variadic list first.
  for value in args {
   if let Value::Host(node)=value{self.validate_native_access(*node,host)?;if host.genuine_node_argument(*node){converted.push(Value::Host(*node));continue;}}
   let value=self.copy_value(value)?;let primitive=self.primitive_with_hint(value,true,host)?;
   let string=if primitive.is_string(){primitive}else{Value::String(primitive.script_text()?)};
   converted.push(self.checked_value(string)?);
  }
  host.native_operation_caller(caller);self.validate_native_access(object,host)?;
  host.validate_call(object,method,&[]).map_err(|(kind,message)|Fault::exception(kind,message))?;
  for value in &converted{if let Value::Host(node)=value{self.validate_native_access(*node,host)?;}}
  host.native_operation_caller(caller);
  let owner=match host.get(object,"ownerDocument").map_err(Fault::new)?{Value::Host(owner)=>owner,Value::Null=>object,_=>return Err(Fault::new("Native ParentNode owner is unavailable."))};
  // All strings become real owner-document Text nodes before any staging move.
  for value in &mut converted{if !matches!(value,Value::Host(_)){*value=self.call_host(owner,"createTextNode",std::slice::from_ref(value),host)?;}}
  let node=if converted.len()==1{converted.pop().unwrap()}else{
   let fragment=self.call_host(owner,"createDocumentFragment",&[],host)?;
   let Value::Host(handle)=fragment else{return Err(Fault::new("Native fragment factory returned no Node."));};
   for value in &converted{let Value::Host(node)=value else{unreachable!()};self.parent_insertion_check(handle,*node,host)?;self.call_host(handle,"appendChild",std::slice::from_ref(value),host)?;}
   fragment
  };
  if method!=REPLACE{let Value::Host(handle)=node else{unreachable!()};self.parent_insertion_check(object,handle,host)?;}
  match method {
   APPEND=>{self.call_host(object,"appendChild",&[node],host)?;},
   PREPEND=>{let first=host.get(object,"firstChild").map_err(Fault::new)?;self.call_host(object,"insertBefore",&[node,first],host)?;},
   REPLACE=>{self.call_host(object,REPLACE_ALL,&[node],host)?;},_=>unreachable!()
  }
  Ok(Value::Undefined)
 }
}
