//! ParentNode binding conversions precede native DOM construction and mutation.
//! https://dom.spec.whatwg.org/#interface-parentnode
use super::*;
pub(crate) const APPEND:&str="\0ParentNode.append";
pub(crate) const PREPEND:&str="\0ParentNode.prepend";
pub(crate) const REPLACE:&str="\0ParentNode.replaceChildren";
pub(crate) const REPLACE_ALL:&str="\0ParentNode.replaceAllNative";
pub(crate) fn operation(method:&str)->bool{matches!(method,APPEND|PREPEND|REPLACE)}
impl Runtime {
 pub(super) fn define_node_unscopables(&mut self,prototype:usize,names:&[&str])->Result<(),Fault>{
  let key=self.symbol_unscopables.unwrap();
  let blocked=if let Some(property)=self.objects[prototype].symbol_properties.get(&key){
   let Value::Object(id)=property.value.as_value()? else{return Err(Fault::new("Native unscopables record is unavailable."));};*id
  }else{
   let id=self.new_object(None)?;self.objects[id].prototype=None;
   self.store_symbol_property(prototype,key,Value::Object(id),Some((false,false,true)))?;id
  };
  for name in names{self.store_property(blocked,(*name).into(),Value::Bool(true),Some((true,true,true)))?;}
  Ok(())
 }
 pub(super) fn define_parent_node_methods(&mut self,prototype:usize,window:u32)->Result<(),Fault>{
  for(name,method)in [("append",APPEND),("prepend",PREPEND),("replaceChildren",REPLACE)]{
   self.deferred_host_method(prototype,window,name,method,0)?;
  }
  self.define_node_unscopables(prototype,&["append","prepend","replaceChildren"])
 }
 pub(super) fn parent_insertion_check(&mut self,parent:u32,node:u32,host:&mut dyn Host)->Result<(),Fault>{
  host.native_operation_caller(self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(w,_)|w)));
  match host.parent_insertion_validity(parent,node){Ok(())=>Ok(()),Err(("TypeError",message))=>Err(Fault::exception("TypeError",message)),Err((kind,message))=>Err(Fault::thrown(self.native_dom_exception(kind,&message)?))}
 }
 pub(super) fn pre_insert_variadic_node(&mut self,parent:u32,node:Value,mut reference:Value,host:&mut dyn Host)->Result<Value,Fault>{
  let Value::Host(incoming)=node else{return Err(Fault::new("Variadic pre-insertion requires a native Node."));};
  self.parent_insertion_check(parent,incoming,host)?;
  if let Value::Host(child)=reference{
   self.validate_native_access(child,host)?;
   if !matches!(host.get(child,"parentNode").map_err(Fault::new)?,Value::Host(owner) if owner==parent){
    return Err(Fault::thrown(self.native_dom_exception("NotFoundError","The insertion reference is no longer a child of the original parent.")?));
   }
   // DOM pre-insertion adjusts an incoming node used as its own reference.
   // The shared native move then produces actual removal/insertion records.
   if child==incoming{reference=host.get(incoming,"nextSibling").map_err(Fault::new)?;}
  }
  self.call_host(parent,"insertBefore",&[node,reference],host)
 }
 pub(super) fn convert_node_or_string_arguments(&mut self,args:&[Value],host:&mut dyn Host)->Result<Vec<Value>,Fault>{
  self.work(args.len().checked_mul(std::mem::size_of::<Value>()).ok_or_else(||Fault::new("Variadic node argument size overflow."))?)?;
  let mut converted=Vec::new();converted.try_reserve_exact(args.len()).map_err(|_|Fault::new("Cannot reserve variadic node arguments."))?;
  // Complete the entire Web IDL variadic union conversion before DOM steps.
  for value in args {
   if let Value::Host(node)=value{self.validate_native_access(*node,host)?;if host.genuine_node_argument(*node){converted.push(Value::Host(*node));continue;}}
   let value=self.copy_value(value)?;let primitive=self.primitive_with_hint(value,true,host)?;
   let string=if primitive.is_string(){primitive}else{Value::String(primitive.script_text()?)};
   converted.push(self.checked_value(string)?);
  }
  Ok(converted)
 }
 pub(super) fn recheck_variadic_nodes(&mut self,object:u32,method:&str,args:&[Value],host:&mut dyn Host)->Result<(),Fault>{
  let caller=self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(w,_)|w));
  host.native_operation_caller(caller);self.validate_native_access(object,host)?;
  host.validate_call(object,method,&[]).map_err(|(kind,message)|Fault::exception(kind,message))?;
  for value in args{if let Value::Host(node)=value{self.validate_native_access(*node,host)?;}}
  host.native_operation_caller(caller);Ok(())
 }
 pub(super) fn construct_variadic_node(&mut self,object:u32,mut converted:Vec<Value>,host:&mut dyn Host)->Result<Value,Fault>{
  let owner=match host.get(object,"ownerDocument").map_err(Fault::new)?{Value::Host(owner)=>owner,Value::Null=>object,_=>return Err(Fault::new("Native variadic node owner is unavailable."))};
  // Create all actual owner-document Text nodes before moving any Node.
  for value in &mut converted{if !matches!(value,Value::Host(_)){*value=self.call_host(owner,"createTextNode",std::slice::from_ref(value),host)?;}}
  if converted.len()==1{return Ok(converted.pop().unwrap());}
  let fragment=self.call_host(owner,"createDocumentFragment",&[],host)?;
  let Value::Host(handle)=fragment else{return Err(Fault::new("Native fragment factory returned no Node."));};
  for value in &converted{let Value::Host(node)=value else{unreachable!()};self.parent_insertion_check(handle,*node,host)?;self.call_host(handle,"appendChild",std::slice::from_ref(value),host)?;}
  Ok(fragment)
 }
 pub(super) fn parent_node_variadic_call(&mut self,object:u32,method:&str,args:&[Value],host:&mut dyn Host)->Result<Value,Fault>{
  host.native_operation_caller(self.native_access_caller.unwrap_or_else(||self.browser_global.map(|(w,_)|w)));
  host.validate_call(object,method,&[]).map_err(|(kind,message)|Fault::exception(kind,message))?;
  let converted=self.convert_node_or_string_arguments(args,host)?;
  self.recheck_variadic_nodes(object,method,&converted,host)?;
  let node=self.construct_variadic_node(object,converted,host)?;
  if method!=REPLACE{let Value::Host(handle)=node else{unreachable!()};self.parent_insertion_check(object,handle,host)?;}
  match method {
   APPEND=>{self.call_host(object,"appendChild",&[node],host)?;},
   PREPEND=>{let first=host.get(object,"firstChild").map_err(Fault::new)?;self.pre_insert_variadic_node(object,node,first,host)?;},
   REPLACE=>{self.call_host(object,REPLACE_ALL,&[node],host)?;},_=>unreachable!()
  }
  Ok(Value::Undefined)
 }
}
