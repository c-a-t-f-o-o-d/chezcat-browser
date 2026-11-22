//! Native contiguous Text/CDATA concatenation, without mutation or flattening.
use super::*;
pub(in crate::scripting) const GETTER:&str="\0Text.get.wholeText";
impl Bridge<'_>{
 pub(in crate::scripting) fn text_node_receiver(&self,object:u32)->bool{
  if let Some(kind)=self.inert_node_type(object){return matches!(kind,3|4);}
  node_index(object).is_some_and(|node|self.dom_accessible(node)&&matches!(self.doc.nodes[node].kind,NodeKind::Text(_)|NodeKind::Cdata(_)))
 }
 fn whole_text_parent(&self,object:u32)->Result<Option<u32>,String>{
  if let Some(node)=self.state.inert_documents.node(object){return Ok(node.parent);}
  let node=node_index(object).filter(|&node|self.dom_accessible(node)).ok_or("Missing wholeText node.")?;
  Ok(self.state.tree.parents.get(node).copied().flatten().map(|node|if node==0{DOCUMENT}else{node_handle(node)}))
 }
 fn whole_text_child_count(&self,parent:u32)->Result<usize,String>{
  if let Some(children)=self.inert_children(parent){return Ok(children.len());}
  let node=if parent==DOCUMENT{0}else{node_index(parent).filter(|&node|self.dom_accessible(node)).ok_or("Missing wholeText parent.")?};Ok(self.doc.nodes[node].children.len())
 }
 fn whole_text_child(&self,parent:u32,index:usize)->Option<u32>{
  if let Some(children)=self.inert_children(parent){return children.get(index).copied();}
  let node=if parent==DOCUMENT{0}else{node_index(parent).filter(|&node|self.dom_accessible(node))?};self.doc.nodes.get(node)?.children.get(index).copied().map(node_handle)
 }
 fn whole_text_singleton(&mut self,object:u32)->Result<Value,String>{
  let data=self.character_data_source(object).ok_or("Missing wholeText data.")?;let logical=data.logical_bytes();let work=data.used_bytes().checked_add(1).ok_or("wholeText singleton work overflow.")?;if logical>self.state.limits.host.string_bytes{return Err("wholeText result exceeds the host string allowance.".into());}self.state.work(work)?;self.state.copies(logical)?;Self::dom_string_copy(self.character_data_source(object).unwrap())
 }
 fn whole_text_parts<'a>(&'a self,handles:&[u32])->Result<Vec<&'a DomString>,String>{
  let mut parts=Vec::new();parts.try_reserve_exact(handles.len()).map_err(|_|"Cannot reserve wholeText references.")?;for &handle in handles{parts.push(self.character_data_source(handle).ok_or("wholeText source disappeared.")?);}Ok(parts)
 }
 pub(in crate::scripting) fn native_whole_text(&mut self,object:u32)->Result<Value,String>{
  if !self.text_node_receiver(object){return Err("wholeText requires a genuine Text or CDATA node.".into());}self.state.rebuild(self.doc)?;
  let Some(parent)=self.whole_text_parent(object)?else{return self.whole_text_singleton(object);};let count=self.whole_text_child_count(parent)?;
  let lookup=self.state.inert_documents.adopted.lookup_work().checked_add(2).and_then(|n|n.checked_mul(8)).ok_or("wholeText lookup work overflow.")?;self.state.work(count.checked_mul(lookup).ok_or("wholeText sibling work overflow.")?)?;
  let position=(0..count).find(|&index|self.whole_text_child(parent,index)==Some(object)).ok_or("wholeText parent edge is inconsistent.")?;let mut start=position;let mut end=position+1;
  while start>0&&self.whole_text_child(parent,start-1).is_some_and(|node|self.text_node_receiver(node)){start-=1;}
  while end<count&&self.whole_text_child(parent,end).is_some_and(|node|self.text_node_receiver(node)){end+=1;}
  let count=end-start;if count==1{return self.whole_text_singleton(object);}
  let metadata=count.checked_mul(size_of::<u32>()+2*size_of::<&DomString>()).ok_or("wholeText reference storage overflow.")?;self.state.copies(metadata)?;
  let mut handles=Vec::new();handles.try_reserve_exact(count).map_err(|_|"Cannot reserve wholeText source handles.")?;for index in start..end{handles.push(self.whole_text_child(parent,index).ok_or("wholeText sibling disappeared.")?);}
  let input=handles.iter().try_fold(0usize,|n,&node|n.checked_add(self.character_data_source(node).ok_or("wholeText source disappeared.")?.used_bytes()).ok_or("wholeText input work overflow."))?;self.state.work(input.checked_mul(4).ok_or("wholeText data work overflow.")?)?;
  let(logical,copy)={let parts=self.whole_text_parts(&handles)?;(DomString::concat_size(&parts)?.0,DomString::concat_copy_work(&parts)?)};if logical>self.state.limits.host.string_bytes{return Err("wholeText result exceeds the host string allowance.".into());}self.state.copies(copy)?;
  let parts=self.whole_text_parts(&handles)?;Ok(Self::dom_string_owned(DomString::concat(&parts,DomString::storage_bound(self.state.limits.host.string_bytes)?)?))
 }
}

#[cfg(test)]
#[path="script_text_whole_storage_tests.rs"]
mod storage_tests;
