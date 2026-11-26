//! Native state-preserving move; no removal/insertion lifecycle publication.
//! https://dom.spec.whatwg.org/#concept-node-move-ext
use super::*;
impl Bridge<'_>{
 fn state_move_parent(&self,node:u32)->Option<u32>{
  if self.inert_node_type(node).is_some(){return self.state.inert_documents.node(node).and_then(|n|n.parent);}
  if node==DOCUMENT{return None;}
  node_index(node).and_then(|n|self.state.tree.parents.get(n).copied().flatten()).map(|n|if n==0{DOCUMENT}else{node_handle(n)})
 }
 fn state_move_root(&self,mut node:u32)->Result<u32,(&'static str,String)>{
  for _ in 0..self.doc.nodes.len().saturating_add(self.state.inert_documents.entries.len()).saturating_add(self.state.inert_documents.adopted.len()).saturating_add(1){
   let Some(parent)=self.state_move_parent(node)else{return Ok(node);};node=parent;
  }
  Err(("NotSupportedError","Native move root topology is inconsistent.".into()))
 }
 pub(in crate::scripting) fn state_move_arguments(&self,args:&[Value])->Result<[Value;2],String>{
  let Some(Value::Host(node))=args.first()else{return Err("Native move requires a Node.".into());};
  let mut reference=match args.get(1){Some(Value::Host(n))=>Some(*n),Some(Value::Null|Value::Undefined)=>None,_=>return Err("Native move requires a Node or null reference.".into())};
  if reference==Some(*node){reference=if let Some(inert)=self.state.inert_documents.node(*node){inert.parent.and_then(|p|self.inert_children(p)).and_then(|siblings|siblings.iter().position(|n|n==node).and_then(|at|siblings.get(at+1))).copied()}else{node_index(*node).and_then(|n|self.state.tree.parents.get(n).copied().flatten().and_then(|p|self.doc.nodes[p].children.iter().position(|&c|c==n).and_then(|at|self.doc.nodes[p].children.get(at+1).copied()))).map(node_handle)};}
  Ok([Value::Host(*node),reference.map_or(Value::Null,Value::Host)])
 }
 pub(in crate::scripting) fn validate_state_preserving_move(&self,parent:u32,args:&[Value])->Result<(),(&'static str,String)>{
  if !self.parent_node_receiver(parent){return Err(("TypeError","moveBefore requires a genuine ParentNode.".into()));}
  self.native_node_access(parent)?;
  let Some(Value::Host(node))=args.first()else{return Err(("TypeError","moveBefore requires a Node argument.".into()));};
  if !self.get_root_receiver(*node){return Err(("TypeError","moveBefore requires a genuine Node argument.".into()));}
  self.native_node_access(*node)?;
  match args.get(1){Some(Value::Host(child))=>{if !self.get_root_receiver(*child){return Err(("TypeError","moveBefore reference requires a genuine Node.".into()));}self.native_node_access(*child)?;},Some(Value::Null|Value::Undefined)=>{},_=>return Err(("TypeError","moveBefore requires an explicit nullable Node reference.".into()))}
  if self.is_frame_handle(parent)||self.is_frame_handle(*node){return Err(("NotSupportedError","Child document moving steps are not implemented.".into()));}
  let hierarchy=||("HierarchyRequestError","moveBefore violates native move eligibility.".into());
  if self.state_move_root(parent)?!=self.state_move_root(*node)?{return Err(hierarchy());}
  if self.inert_mutation_receiver(parent){self.parent_inert_insertion_cycle(parent,*node)?;}else{
   let mut ancestor=Some(parent);for _ in 0..=self.doc.nodes.len(){let Some(at)=ancestor else{break;};if at==*node{return Err(hierarchy());}ancestor=self.state_move_parent(at);}
  }
  let adjusted=self.state_move_arguments(args).map_err(|message|("TypeError",message))?;
  if let Value::Host(reference)=adjusted[1]{if self.state_move_parent(reference)!=Some(parent){return Err(("NotFoundError","The move reference is not a child of this parent.".into()));}}
  if !matches!(self.tree_walker_node_type(*node),Some(1|3|4|7|8)){return Err(hierarchy());}
  if parent==DOCUMENT||self.inert_document_children_receiver(parent){
   if matches!(self.tree_walker_node_type(*node),Some(3|4)){return Err(hierarchy());}
   if self.tree_walker_node_type(*node)==Some(1){
    let element=if parent==DOCUMENT{self.doc.nodes[0].children.iter().any(|&n|matches!(self.doc.nodes[n].kind,NodeKind::Element{..}))}else{self.inert_children(parent).unwrap().iter().any(|&n|self.inert_node_type(n)==Some(1))};if element{return Err(hierarchy());}
   }
   if parent==DOCUMENT{return Err(("NotSupportedError","Displayed Document moving steps are not implemented.".into()));}
  }
  if self.state_move_parent(*node).is_none(){return Err(hierarchy());}
  if self.inert_mutation_receiver(parent){return self.validate_inert_mutation(parent,"insertBefore",&adjusted);}
  Ok(())
 }
 pub(in crate::scripting) fn state_preserving_move(&mut self,parent:u32,args:&[Value])->Result<Value,String>{
  self.validate_state_preserving_move(parent,args).map_err(|(_,message)|message)?;
  let adjusted=self.state_move_arguments(args)?;
  if self.inert_mutation_receiver(parent){self.mutate_inert_children(parent,parent_node_interface::MOVE,&adjusted)?;return Ok(Value::Undefined);}
  let Value::Host(handle)=adjusted[0]else{unreachable!()};let child=node_index(handle).ok_or("Native moving node index unavailable.")?;let parent=node_index(parent).ok_or("Native move parent index unavailable.")?;
  let old=self.state.tree.parents[child].ok_or("Native move old parent unavailable.")?;
  self.text_child_mutation_ancestors(parent,child)?;self.text_child_mutation_ancestors(old,child)?;
  let(descendants,depth)=self.checked_mutation_subtree(child,false,false)?;
  let mut ancestor=Some(parent);let mut parent_depth=0usize;
  while let Some(node)=ancestor{self.state.work(1)?;parent_depth=parent_depth.checked_add(1).ok_or("Native move depth overflow.")?;ancestor=self.state.tree.parents[node];}
  if parent_depth.saturating_add(depth)>dom_tree::MAX_DOM_DEPTH{return Err("Native move exceeds represented DOM depth.".into());}
  self.validate_svg_edge(node_handle(parent),handle)?;self.validate_button_children(parent,&descendants)?;
  let plan=self.prepare_select_form_change(None,&[(child,Some(parent))])?;
  let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(plan.retained_bytes).ok_or("Native move control storage exhausted.")?;
  let(growth,bytes)=if old==parent{(None,0)}else{retained_storage::grow(&self.doc.nodes[parent].children,1,available)?};
  let work=self.doc.nodes[old].children.len().checked_add(self.doc.nodes[parent].children.len()).and_then(|n|n.checked_add(descendants.len())).ok_or("Native move work overflow.")?;self.state.work(work)?;
  let old_slot=self.doc.nodes[old].children.iter().position(|&n|n==child).ok_or("Native move source link missing.")?;
  let reference=if let Value::Host(reference)=adjusted[1]{Some(node_index(reference).ok_or("Native move reference index missing.")?)}else{None};
  let at=if let Some(reference)=reference{self.doc.nodes[parent].children.iter().position(|&n|n==reference).ok_or("Native move reference link missing.")?}else{self.doc.nodes[parent].children.len()};
  let at=at-usize::from(old==parent&&old_slot<at);
  let copies=work.checked_add(if growth.is_some(){self.doc.nodes[parent].children.len()}else{0}).and_then(|n|n.checked_mul(size_of::<usize>())).and_then(|n|n.checked_add(plan.retained_bytes)).ok_or("Native move copy overflow.")?;
  self.state.prepare_range_changes(2,descendants.len())?;
  self.state.replace_storage(0,bytes.checked_add(plan.retained_bytes).ok_or("Native move storage overflow.")?,copies)?;
  // All admission is complete. Preserve connected state, frame contexts,
  // loaded resources and editor identities; publish only topology and metadata.
  self.state.ranges_remove(old,old_slot,1,&descendants);
  if let Some(mut next)=growth{next.append(&mut self.doc.nodes[parent].children);self.doc.nodes[parent].children=next;}
  self.doc.nodes[old].children.remove(old_slot);self.state.ranges_insert(parent,at,1);self.doc.nodes[parent].children.insert(at,child);self.state.tree.parents[child]=Some(parent);
  plan.commit(self.forms);self.state.dom_committed();self.state.dirty=true;if self.state.dom.connected[child]{self.state.layout_changed();}
  Ok(Value::Undefined)
 }
}

#[cfg(test)]
#[path="script_noscript_text_move_before_tests.rs"]
mod noscript_text_move_before_tests;
