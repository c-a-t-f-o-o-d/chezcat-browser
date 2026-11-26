//! Native insertion validates hierarchy before the shared Text split transaction.
//! https://dom.spec.whatwg.org/#concept-range-insert
use super::*;
use crate::javascript::range_interface::InsertInfo;
impl Bridge<'_>{
 pub(in crate::scripting) fn range_insert_plan(&mut self,receiver:u32,args:&[Value])->Result<InsertInfo,Error>{
  use Error::{Exception,Fatal};
  let range=*self.range_record(receiver).filter(|r|!r.fixed).ok_or_else(||Exception("TypeError","A genuine live Range is required.".into()))?;
  let Some(Value::Host(incoming))=args.first()else{return Err(Exception("TypeError","Range insertion requires a genuine Node.".into()));};let incoming=*incoming;
  let start=self.range_node(range.start.node).map_err(Fatal)?;
  let split=matches!(self.doc.nodes[start].kind,NodeKind::Text(_)|NodeKind::Cdata(_));
  if incoming==range.start.node||matches!(self.doc.nodes[start].kind,NodeKind::Comment(_)|NodeKind::ProcessingInstruction{..})||split&&self.range_parent(range.start.node).map_err(Fatal)?.is_none(){return Err(Exception("HierarchyRequestError","This Range start cannot receive the inserted Node.".into()));}
  let(parent,reference)=if split{(self.range_parent(range.start.node).map_err(Fatal)?.unwrap(),Some(range.start.node))}else{
   self.state.work(self.doc.nodes[start].children.len()).map_err(Fatal)?;
   let reference=self.doc.nodes[start].children.get(range.start.offset).copied().map(node_handle);(range.start.node,reference)
  };
  let kind=if incoming==DOCUMENT{Some(9)}else if self.attr_receiver(incoming){Some(2)}else if let Some(kind)=self.inert_node_type(incoming){Some(kind)}else if let Some(kind)=self.frame_traversal_node_type(incoming){Some(kind)}else{node_index(incoming).filter(|&node|self.dom_accessible(node)).map(|node|match self.doc.nodes[node].kind{NodeKind::Element{..}=>1,NodeKind::Text(_)=>3,NodeKind::Cdata(_)=>4,NodeKind::ProcessingInstruction{..}=>7,NodeKind::Comment(_)=>8,NodeKind::DocumentType(_)=>10,NodeKind::DocumentFragment=>11})};
  let kind=kind.ok_or_else(||Exception("TypeError","Range insertion requires a genuine Node.".into()))?;
  if !matches!(kind,1|3|4|7|8|11){return Err(Exception("HierarchyRequestError","This Node kind cannot be inserted into a Range parent.".into()));}
  if parent==DOCUMENT{return Err(Exception("NotSupportedError","Range insertion into displayed Document children requires the broader Document mutation producer.".into()));}
  let parent_index=self.range_node(parent).map_err(Fatal)?;
  if !matches!(self.doc.nodes[parent_index].kind,NodeKind::Element{..}|NodeKind::DocumentFragment){return Err(Exception("HierarchyRequestError","The Range insertion parent cannot contain children.".into()));}
  if node_index(incoming).is_some_and(|node|self.dom_accessible(node)){
   if incoming==parent||self.range_ancestor_child(incoming,parent).map_err(Fatal)?.is_some(){return Err(Exception("HierarchyRequestError","Range insertion would create an ancestor cycle.".into()));}
  }else if self.inert_node_type(incoming).is_none(){return Err(Exception("NotSupportedError","Child Node insertion requires its native mutation producer.".into()));}
  if let Some(reference)=reference{if self.range_parent(reference).map_err(Fatal)?!=Some(parent){return Err(Exception("NotFoundError","Range insertion reference is not a child of its parent.".into()));}}
  let values=[Value::Host(incoming),reference.map_or(Value::Null,Value::Host)];
  self.dom_validate_call(parent,"insertBefore",&values).map_err(|(name,message)|Exception(name,message))?;
  Ok(InsertInfo{node:incoming,parent,reference,split:split.then_some((range.start.node,range.start.offset))})
 }
 pub(in crate::scripting) fn range_insert_reference_node(&mut self,reference:Option<u32>,incoming:u32)->Result<Option<u32>,String>{
  if reference!=Some(incoming){return Ok(reference);}self.state.rebuild(self.doc)?;let(parent,index)=self.range_index(incoming)?;let parent=self.range_node(parent)?;Ok(self.doc.nodes[parent].children.get(index+1).copied().map(node_handle))
 }
 pub(in crate::scripting) fn range_insert_new_offset(&mut self,parent:u32,reference:Option<u32>,incoming:u32)->Result<usize,String>{
  self.state.rebuild(self.doc)?;let parent=self.range_node(parent)?;self.state.work(self.doc.nodes[parent].children.len())?;
  let offset=if let Some(reference)=reference{let reference=self.range_node(reference)?;self.doc.nodes[parent].children.iter().position(|&child|child==reference).ok_or("Range insertion reference disappeared.")?}else{self.doc.nodes[parent].children.len()};
  let count=if self.inert_node_type(incoming)==Some(11){self.inert_children(incoming).ok_or("Missing native fragment children.")?.len()}else if let Some(node)=node_index(incoming).filter(|&node|self.dom_accessible(node)){if matches!(self.doc.nodes[node].kind,NodeKind::DocumentFragment){self.doc.nodes[node].children.len()}else{1}}else{1};
  offset.checked_add(count).ok_or_else(||"Range inserted extent overflow.".into())
 }
 pub(in crate::scripting) fn range_insert_end(&mut self,receiver:u32,parent:u32,offset:usize)->Result<(),String>{
  let index=receiver.checked_sub(BASE).ok_or("Invalid Range insertion receiver.")? as usize;
  let record=self.state.ranges.get_mut(index).filter(|r|!r.fixed).ok_or("A live Range insertion receiver is required.")?;if record.start==record.end{record.end=Point{node:parent,offset};}Ok(())
 }
}
