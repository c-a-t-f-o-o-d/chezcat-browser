//! Native replace-all admission, records and one checked publication.
//! https://dom.spec.whatwg.org/#concept-node-replace-all
use super::*;
use crate::javascript::NativeChildListChange;
impl Bridge<'_>{
 pub(in crate::scripting) fn parent_inert_insertion_cycle(&self,parent:u32,node:u32)->Result<(),(&'static str,String)>{
  // The final shared append/insertBefore call validates document ordering
  // against its actual reference. This common check only rejects cycles.
  let arena=&self.state.inert_documents;
  let mut ancestor=Some(parent);let bound=arena.entries.len().saturating_add(arena.adopted.len()).saturating_add(self.doc.nodes.len()).saturating_add(1);
  for _ in 0..bound{let Some(handle)=ancestor else{break;};if handle==node{return Err(("HierarchyRequestError","Insertion would create an inert ancestor cycle.".into()));}ancestor=arena.node(handle).and_then(|n|n.parent.or_else(||if let Kind::Fragment{template}=n.kind{template}else{None}));}
  if ancestor.is_some(){return Err(("NotSupportedError","Inert insertion topology is inconsistent.".into()));}
  Ok(())
 }

 pub(in crate::scripting) fn validate_parent_replace_all(&self,parent:u32,args:&[Value])->Result<(),(&'static str,String)>{
  if !self.parent_node_receiver(parent){return Err(("TypeError","Replacement requires a genuine ParentNode.".into()));}
  self.native_node_access(parent)?;
  let Some(Value::Host(node))=args.first()else{return Err(("TypeError","Native replacement requires a Node.".into()));};
  self.native_node_access(*node)?;
  let hierarchy=||("HierarchyRequestError","Replacement violates native node hierarchy.".into());
  if *node==parent||!self.get_root_receiver(*node){return Err(hierarchy());}
  if !matches!(self.tree_walker_node_type(*node),Some(1|3|4|7|8|10|11)){return Err(hierarchy());}
  if self.frame_contains_node(parent).is_some()||parent==DOCUMENT{return Err(("NotSupportedError","Displayed and child Document replacement is not implemented.".into()));}
  if self.inert_mutation_receiver(parent){
   let arena=&self.state.inert_documents;let owner=arena.node(parent).unwrap().owner;
   let Some(source)=arena.node(*node)else{return Err(("NotSupportedError","Cross-owner replacement adoption is not implemented.".into()));};
   if source.owner!=owner{return Err(("NotSupportedError","Cross-owner replacement adoption is not implemented.".into()));}
   let mut ancestor=Some(parent);let bound=arena.entries.len().saturating_add(arena.adopted.len()).saturating_add(self.doc.nodes.len()).saturating_add(1);
   for _ in 0..bound{let Some(handle)=ancestor else{break;};if handle==*node{return Err(hierarchy());}ancestor=arena.node(handle).and_then(|n|n.parent.or_else(||if let Kind::Fragment{template}=n.kind{template}else{None}));}
   if ancestor.is_some(){return Err(hierarchy());}
   let document=self.inert_document_children_receiver(parent);let fragment=self.inert_node_type(*node)==Some(11);
   let roots=if fragment{self.inert_children(*node).unwrap()}else{std::slice::from_ref(node)};
   let(mut element,mut doctype)=(false,false);
   for root in roots{match self.inert_node_type(*root){Some(1) if !document||!element=>element=true,Some(10) if document&&!doctype&&!element=>doctype=true,Some(7|8)=>{},Some(3|4) if !document=>{},_=>return Err(hierarchy())}}
   return Ok(());
  }
  if self.inert_node_type(*node).is_some(){return Err(("NotSupportedError","Cross-owner replacement adoption is not implemented.".into()));}
  let mut ancestor=node_index(parent);
  for _ in 0..=self.doc.nodes.len(){let Some(at)=ancestor else{break;};if node_handle(at)==*node{return Err(hierarchy());}ancestor=self.state.tree.parents.get(at).copied().flatten();}
  self.dom_validate_call(parent,"appendChild",args)
 }
 fn parent_replacement_roots(&mut self,parent:u32,node:u32)->Result<(Vec<u32>,Vec<u32>),String>{
  let inert=self.inert_mutation_receiver(parent);let fragment=self.tree_walker_node_type(node)==Some(11);
  let count=if inert{self.inert_children(parent).unwrap().len()}else{self.doc.nodes[node_index(parent).ok_or("Main replacement node index is unavailable.")?].children.len()};
  let added=if fragment{if inert{self.inert_children(node).unwrap().len()}else{self.doc.nodes[node_index(node).ok_or("Main replacement node index is unavailable.")?].children.len()}}else{1};
  self.state.work(count.saturating_add(added))?;
  self.state.copies(count.checked_add(added.checked_mul(2).ok_or("Replacement record overflow.")?).and_then(|n|n.checked_mul(size_of::<u32>())).ok_or("Replacement record overflow.")?)?;
  let scratch=count.checked_add(added).and_then(|n|n.checked_mul(size_of::<u32>())).ok_or("Replacement list scratch size overflow.")?;
  if scratch>self.state.retained_limit.saturating_sub(self.state.retained){return Err("Replacement list scratch exceeds the native retained allowance.".into());}
  let mut old=Vec::new();old.try_reserve_exact(count).map_err(|_|"Cannot reserve replaced nodes.")?;
  let mut roots=Vec::new();roots.try_reserve_exact(added).map_err(|_|"Cannot reserve added nodes.")?;
  let actual=old.capacity().checked_add(roots.capacity()).and_then(|n|n.checked_mul(size_of::<u32>())).ok_or("Replacement allocated list size overflow.")?;
  if actual>self.state.retained_limit.saturating_sub(self.state.retained){return Err("Replacement allocated lists exceed the native retained allowance.".into());}
  if inert{old.extend_from_slice(self.inert_children(parent).unwrap());if fragment{roots.extend_from_slice(self.inert_children(node).unwrap());}else{roots.push(node);}}
  else{old.extend(self.doc.nodes[node_index(parent).ok_or("Main replacement node index is unavailable.")?].children.iter().copied().map(node_handle));if fragment{roots.extend(self.doc.nodes[node_index(node).ok_or("Main replacement node index is unavailable.")?].children.iter().copied().map(node_handle));}else{roots.push(node);}}
  Ok((old,roots))
 }
 pub(in crate::scripting) fn parent_replace_all_changes(&mut self,parent:u32,args:&[Value])->Result<Vec<NativeChildListChange>,String>{
  self.validate_parent_replace_all(parent,args).map_err(|(_,m)|m)?;self.state.rebuild(self.doc)?;
  let Value::Host(node)=args[0]else{unreachable!()};let (old,roots)=self.parent_replacement_roots(parent,node)?;
  let inert=self.inert_mutation_receiver(parent);let fragment=self.tree_walker_node_type(node)==Some(11);
  let source=if fragment{Some(node)}else if inert{self.inert_observation_parent(node)}else{self.state.tree.parents[node_index(node).ok_or("Main replacement node index is unavailable.")?].map(node_handle)};
  let mut changes=Vec::new();changes.try_reserve_exact(2).map_err(|_|"Cannot reserve replacement records.")?;
  if let Some(source)=source.filter(|source|fragment||*source!=parent){
   let(previous,next)=if fragment{(None,None)}else if inert{let c=self.inert_children(source).unwrap();let at=c.iter().position(|h|*h==node).ok_or("Missing replacement source link.")?;(at.checked_sub(1).map(|i|c[i]),c.get(at+1).copied())}else{let c=&self.doc.nodes[node_index(source).ok_or("Main replacement node index is unavailable.")?].children;let at=c.iter().position(|&n|node_handle(n)==node).ok_or("Missing replacement source link.")?;(at.checked_sub(1).map(|i|node_handle(c[i])),c.get(at+1).copied().map(node_handle))};
   if !roots.is_empty(){let mut removed=Vec::new();removed.try_reserve_exact(roots.len()).map_err(|_|"Cannot reserve replacement source record.")?;removed.extend_from_slice(&roots);changes.push(NativeChildListChange{target:source,added:Vec::new(),removed,previous,next});}
  }
  if !old.is_empty()||!roots.is_empty(){changes.push(NativeChildListChange{target:parent,added:roots,removed:old,previous:None,next:None});}
  Ok(changes)
 }
 pub(in crate::scripting) fn parent_replace_all(&mut self,parent:u32,args:&[Value])->Result<Value,String>{
  self.validate_parent_replace_all(parent,args).map_err(|(_,m)|m)?;
  let Value::Host(node)=args[0]else{unreachable!()};
  if !self.inert_mutation_receiver(parent){let parent=node_index(parent).ok_or("Main replacement node index is unavailable.")?;return self.splice_document_fragment(parent,parent_node_interface::REPLACE_ALL,args);}
  let(old,roots)=self.parent_replacement_roots(parent,node)?;let fragment=self.inert_node_type(node)==Some(11);
  let source=if fragment{Some(node)}else{self.inert_observation_parent(node)};
  let source_at=if !fragment{source.and_then(|source|self.inert_children(source).unwrap().iter().position(|h|*h==node))}else{None};
  self.prepare_document_children_sync(parent,roots.len())?;
  if let Some(source)=source{self.prepare_document_children_sync(source,0)?;}
  let bytes=roots.capacity().checked_mul(size_of::<u32>()).ok_or("Replacement native list size overflow.")?;
  self.state.prepare_inert_range_changes(3)?;
  let old_bytes=self.state.inert_documents.node(parent).unwrap().children.capacity()*size_of::<u32>();
  self.state.replace_storage(old_bytes,bytes,bytes)?;
  // The old list is removed before insertion. This also moves endpoints from
  // an incoming descendant out of a removed subtree, including self reuse.
  self.state.ranges_remove_inert(parent,0,old.len());
  if fragment{self.state.ranges_remove_inert(node,0,roots.len());}
  else if let Some(source)=source.filter(|source|*source!=parent){self.state.ranges_remove_inert(source,source_at.unwrap(),1);}
  self.state.ranges_insert_handle(parent,0,roots.len());
  if fragment{self.state.inert_documents.node_mut(node).unwrap().children.clear();}
  else if let Some(source)=source.filter(|source|*source!=parent){self.state.inert_documents.node_mut(source).unwrap().children.remove(source_at.unwrap());}
  for root in old{self.state.inert_documents.node_mut(root).unwrap().parent=None;}
  for &root in &roots{self.state.inert_documents.node_mut(root).unwrap().parent=Some(parent);}
  self.state.inert_documents.node_mut(parent).unwrap().children=roots;
  self.sync_document_children(parent);if let Some(source)=source.filter(|source|*source!=parent){self.sync_document_children(source);}
  self.state.dom_committed();Ok(Value::Undefined)
 }
}

#[cfg(test)]
#[path="script_replace_all_source_ancestry_tests.rs"]
mod replace_all_source_ancestry_tests;
