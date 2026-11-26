//! Native insertion validates hierarchy before the shared Text split transaction.
//! https://dom.spec.whatwg.org/#concept-range-insert
use super::*;
use crate::javascript::range_interface::InsertInfo;
impl Bridge<'_>{
 pub(in crate::scripting) fn range_insert_plan(&mut self,receiver:u32,args:&[Value])->Result<InsertInfo,Error>{
  use Error::{Exception,Fatal};
  let range=*self.range_record(receiver).filter(|r|!r.fixed).ok_or_else(||Exception("TypeError","A genuine live Range is required.".into()))?;
  let Some(Value::Host(incoming))=args.first()else{return Err(Exception("TypeError","Range insertion requires a genuine Node.".into()));};let incoming=*incoming;
  if self.range_attr_document(range.start.node).is_some(){return Err(Exception("HierarchyRequestError","An Attr cannot contain inserted children.".into()));}
  let start_kind=self.range_native_type(range.start.node).ok_or_else(||Fatal("Range insertion start is unavailable.".into()))?;
  let split=matches!(start_kind,3|4);
  if incoming==range.start.node||matches!(start_kind,7|8)||split&&self.range_parent(range.start.node).map_err(Fatal)?.is_none(){return Err(Exception("HierarchyRequestError","This Range start cannot receive the inserted Node.".into()));}
  let(parent,reference)=if split{(self.range_parent(range.start.node).map_err(Fatal)?.unwrap(),Some(range.start.node))}else{
   if self.inert_node_type(range.start.node).is_some(){
    let count=self.inert_children(range.start.node).ok_or_else(||Fatal("Inert Range insertion child links are unavailable.".into()))?.len();self.state.work(count).map_err(Fatal)?;
    (range.start.node,self.inert_children(range.start.node).unwrap().get(range.start.offset).copied())
   }else{
    let start=self.range_node(range.start.node).map_err(Fatal)?;self.state.work(self.doc.nodes[start].children.len()).map_err(Fatal)?;
    let reference=self.doc.nodes[start].children.get(range.start.offset).copied().map(node_handle);(range.start.node,reference)
   }
  };
  let kind=self.range_native_type(incoming);
  let kind=kind.ok_or_else(||Exception("TypeError","Range insertion requires a genuine Node.".into()))?;
  if !matches!(kind,1|3|4|7|8|11)&&!(kind==10&&self.inert_node_type(parent)==Some(9)){return Err(Exception("HierarchyRequestError","This Node kind cannot be inserted into a Range parent.".into()));}
  if parent==DOCUMENT{return Err(Exception("NotSupportedError","Range insertion into displayed Document children requires the broader Document mutation producer.".into()));}
  if !matches!(self.range_native_type(parent),Some(1|9|11)){return Err(Exception("HierarchyRequestError","The Range insertion parent cannot contain children.".into()));}
  if node_index(incoming).is_some_and(|node|self.dom_accessible(node))||self.inert_node_type(incoming).is_some(){
   if incoming==parent||self.range_ancestor_child(incoming,parent).map_err(Fatal)?.is_some(){return Err(Exception("HierarchyRequestError","Range insertion would create an ancestor cycle.".into()));}
  }else{return Err(Exception("NotSupportedError","Child Node insertion requires its native mutation producer.".into()));}
  if let Some(reference)=reference{if self.range_parent(reference).map_err(Fatal)?!=Some(parent){return Err(Exception("NotFoundError","Range insertion reference is not a child of its parent.".into()));}}
  let values=[Value::Host(incoming),reference.map_or(Value::Null,Value::Host)];
  if self.inert_mutation_receiver(parent){self.validate_inert_mutation(parent,"insertBefore",&values).map_err(|(name,message)|Exception(name,message))?;}
  else{self.dom_validate_call(parent,"insertBefore",&values).map_err(|(name,message)|Exception(name,message))?;}
  self.range_insert_main_ancestry(parent,incoming).map_err(Fatal)?;
  Ok(InsertInfo{node:incoming,parent,reference,split:split.then_some((range.start.node,range.start.offset))})
 }
 // Range insertion may split its start Text and explicitly detach the incoming
 // Node before the ordinary insert transaction. Check native ancestry first.
 fn range_insert_main_ancestry(&mut self,parent:u32,incoming:u32)->Result<(),String>{
  if self.inert_mutation_receiver(parent){return Ok(());}
  self.state.rebuild(self.doc)?;
  let parent=self.range_node(parent)?;
  let mut passive=self.range_native_type(incoming)==Some(3);
  if let Some(node)=node_index(incoming).filter(|&node|self.dom_accessible(node)){
   if matches!(self.doc.nodes[node].kind,NodeKind::DocumentFragment){
    passive=true;
    for &child in &self.doc.nodes[node].children{
     self.state.work(1)?;
     let child=self.doc.nodes.get(child).ok_or("Range insertion fragment child is unavailable.")?;
     if child.namespace!=crate::html::Namespace::None||!matches!(child.kind,NodeKind::Text(_))||!child.children.is_empty(){passive=false;break;}
    }
   }else if passive{
    passive=self.doc.nodes[node].namespace==crate::html::Namespace::None&&self.doc.nodes[node].children.is_empty();
   }
   if let Some(source)=self.state.tree.parents[node]{self.text_child_mutation_ancestors(source,node)?;}
  }else if self.inert_node_type(incoming)==Some(11){
   passive=true;
   let count=self.inert_children(incoming).ok_or("Range insertion inert fragment children are unavailable.")?.len();
   for index in 0..count{
    self.state.work(1)?;
    let child=self.inert_children(incoming).unwrap()[index];
    if self.inert_node_type(child)!=Some(3){passive=false;break;}
   }
  }
  if passive{self.passive_text_mutation_ancestors(parent)}else{self.mutation_ancestors(parent)}
 }
 pub(in crate::scripting) fn range_insert_reference_node(&mut self,reference:Option<u32>,incoming:u32)->Result<Option<u32>,String>{
  if reference!=Some(incoming){return Ok(reference);}self.state.rebuild(self.doc)?;let(parent,index)=self.range_index(incoming)?;
  if self.inert_node_type(parent).is_some(){return Ok(self.inert_children(parent).ok_or("Inert Range insertion reference links are unavailable.")?.get(index+1).copied());}
  let parent=self.range_node(parent)?;Ok(self.doc.nodes[parent].children.get(index+1).copied().map(node_handle))
 }
 pub(in crate::scripting) fn range_insert_new_offset(&mut self,parent:u32,reference:Option<u32>,incoming:u32)->Result<usize,String>{
  self.state.rebuild(self.doc)?;
  let offset=if self.inert_node_type(parent).is_some(){
   let count=self.inert_children(parent).ok_or("Inert Range insertion parent links are unavailable.")?.len();self.state.work(count)?;
   if let Some(reference)=reference{self.inert_children(parent).unwrap().iter().position(|&child|child==reference).ok_or("Inert Range insertion reference disappeared.")?}else{count}
  }else{
   let parent=self.range_node(parent)?;self.state.work(self.doc.nodes[parent].children.len())?;
   if let Some(reference)=reference{let reference=self.range_node(reference)?;self.doc.nodes[parent].children.iter().position(|&child|child==reference).ok_or("Range insertion reference disappeared.")?}else{self.doc.nodes[parent].children.len()}
  };
  let count=if self.inert_node_type(incoming)==Some(11){self.inert_children(incoming).ok_or("Missing native fragment children.")?.len()}else if let Some(node)=node_index(incoming).filter(|&node|self.dom_accessible(node)){if matches!(self.doc.nodes[node].kind,NodeKind::DocumentFragment){self.doc.nodes[node].children.len()}else{1}}else{1};
  offset.checked_add(count).ok_or_else(||"Range inserted extent overflow.".into())
 }
 pub(in crate::scripting) fn range_insert_end(&mut self,receiver:u32,parent:u32,offset:usize)->Result<(),String>{
  let index=receiver.checked_sub(BASE).ok_or("Invalid Range insertion receiver.")? as usize;
  let record=self.state.ranges.get_mut(index).filter(|r|!r.fixed).ok_or("A live Range insertion receiver is required.")?;if record.start==record.end{record.end=Point{node:parent,offset};}Ok(())
 }
}
