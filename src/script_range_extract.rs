//! Iterative native extraction plan; fully selected nodes move without copying.
//! https://dom.spec.whatwg.org/#concept-range-extract
use super::*;
use crate::javascript::range_interface::{ExtractPlan,ExtractStep};
enum Task {Visit{node:usize,parent:usize},Append{parent:usize,child:usize}}
impl Bridge<'_>{
 fn extraction_scratch(&mut self,plan:&ExtractPlan,pending:&Vec<Task>,extra:usize)->Result<usize,String>{
  self.state.work(1)?;
  let bytes=plan.steps.capacity().checked_mul(size_of::<ExtractStep>())
   .and_then(|n|plan.slots.capacity().checked_mul(size_of::<Option<u32>>()).and_then(|m|n.checked_add(m)))
   .and_then(|n|pending.capacity().checked_mul(size_of::<Task>()).and_then(|m|n.checked_add(m)))
   .and_then(|n|n.checked_add(extra)).ok_or("Range extraction scratch overflow.")?;
  if bytes>self.state.retained_limit.saturating_sub(self.state.retained){return Err("Range extraction scratch allowance exhausted.".into());}Ok(bytes)
 }
 fn extraction_step(&mut self,plan:&mut ExtractPlan,pending:&Vec<Task>,step:ExtractStep)->Result<(),String>{
  self.extraction_scratch(plan,pending,size_of::<ExtractStep>())?;plan.steps.try_reserve_exact(1).map_err(|_|"Cannot reserve extraction steps.")?;self.extraction_scratch(plan,pending,0)?;plan.steps.push(step);Ok(())
 }
 fn extraction_slot(&mut self,plan:&mut ExtractPlan,pending:&Vec<Task>)->Result<usize,String>{
  self.extraction_scratch(plan,pending,size_of::<Option<u32>>())?;plan.slots.try_reserve_exact(1).map_err(|_|"Cannot reserve extraction identities.")?;self.extraction_scratch(plan,pending,0)?;let slot=plan.slots.len();plan.slots.push(None);Ok(slot)
 }
 pub(in crate::scripting) fn range_extract_plan(&mut self,receiver:u32)->Result<ExtractPlan,Error>{
  use Error::{Exception,Fatal};
  let range=*self.range_record(receiver).filter(|r|!r.fixed).ok_or_else(||Exception("TypeError","A genuine live Range is required.".into()))?;
  let start=self.range_node(range.start.node).map_err(Fatal)?;let end=self.range_node(range.end.node).map_err(Fatal)?;
  let mut plan=ExtractPlan{steps:Vec::new(),slots:Vec::new(),bytes:0};let mut pending=Vec::new();
  let root=self.extraction_slot(&mut plan,&pending).map_err(Fatal)?;
  self.extraction_step(&mut plan,&pending,ExtractStep::Fragment{document:DOCUMENT,slot:root}).map_err(Fatal)?;
  if range.start!=range.end{
   if start==end&&self.doc.nodes[start].kind.character_dom_string().is_some(){
    let slot=self.extraction_slot(&mut plan,&pending).map_err(Fatal)?;let count=range.end.offset-range.start.offset;
    self.extraction_step(&mut plan,&pending,ExtractStep::Slice{source:range.start.node,slot,offset:range.start.offset,count}).map_err(Fatal)?;
    self.extraction_step(&mut plan,&pending,ExtractStep::Append{parent:root,child:slot}).map_err(Fatal)?;
    self.extraction_step(&mut plan,&pending,ExtractStep::Data{node:range.start.node,offset:range.start.offset,count}).map_err(Fatal)?;
   }else{
    let mut common=range.start.node;
    while common!=range.end.node&&self.range_ancestor_child(common,range.end.node).map_err(Fatal)?.is_none(){common=self.range_parent(common).map_err(Fatal)?.ok_or_else(||Fatal("Range extraction has no common ancestor.".into()))?;}
    let collapse=if common==range.start.node{range.start}else{let child=self.range_ancestor_child(common,range.start.node).map_err(Fatal)?.ok_or_else(||Fatal("Range extraction ancestry disappeared.".into()))?;let(parent,index)=self.range_index(child).map_err(Fatal)?;Point{node:parent,offset:index+1}};
    self.extraction_step(&mut plan,&pending,ExtractStep::Collapse{node:collapse.node,offset:collapse.offset}).map_err(Fatal)?;
    let common=self.range_node(common).map_err(Fatal)?;
    self.extraction_children(common,root,&mut plan,&mut pending).map_err(Fatal)?;
    while let Some(task)=pending.pop(){
     self.extraction_scratch(&plan,&pending,0).map_err(Fatal)?;
     let(node,parent)=match task{Task::Append{parent,child}=>{self.extraction_step(&mut plan,&pending,ExtractStep::Append{parent,child}).map_err(Fatal)?;continue;},Task::Visit{node,parent}=>(node,parent)};
     let source=node_handle(node);let native_parent=self.state.tree.parents.get(node).copied().flatten().ok_or_else(||Fatal("Range extraction source lost its parent.".into()))?;
     self.state.work(self.doc.nodes[native_parent].children.len()).map_err(Fatal)?;
     let index=self.doc.nodes[native_parent].children.iter().position(|&child|child==node).ok_or_else(||Fatal("Range extraction topology is inconsistent.".into()))?;
     let owner=if native_parent==0{DOCUMENT}else{node_handle(native_parent)};let before=Point{node:owner,offset:index};let after=Point{node:owner,offset:index+1};
     if matches!(self.range_compare(after,range.start).map_err(Fatal)?,Some(Ordering::Less|Ordering::Equal))||matches!(self.range_compare(before,range.end).map_err(Fatal)?,Some(Ordering::Greater|Ordering::Equal)){continue;}
     let full=matches!(self.range_compare(range.start,before).map_err(Fatal)?,Some(Ordering::Less|Ordering::Equal))&&matches!(self.range_compare(after,range.end).map_err(Fatal)?,Some(Ordering::Less|Ordering::Equal));
     if full{
      if matches!(self.doc.nodes[node].kind,NodeKind::DocumentType(_)){return Err(Exception("HierarchyRequestError","An extracted fragment cannot contain a DocumentType.".into()));}
      if owner==DOCUMENT{return Err(Exception("NotSupportedError","Extraction of displayed Document children requires the broader Document mutation producer.".into()));}
      self.extraction_step(&mut plan,&pending,ExtractStep::Move{parent,node:source}).map_err(Fatal)?;continue;
     }
     self.range_node(source).map_err(Fatal)?;
     let slot=self.extraction_slot(&mut plan,&pending).map_err(Fatal)?;
     if let Some(text)=self.doc.nodes[node].kind.character_dom_string(){
      let start=if source==range.start.node{range.start.offset}else{0};let end=if source==range.end.node{range.end.offset}else{text.utf16_len()};let count=end-start;
      self.extraction_step(&mut plan,&pending,ExtractStep::Slice{source,slot,offset:start,count}).map_err(Fatal)?;
      self.extraction_step(&mut plan,&pending,ExtractStep::Append{parent,child:slot}).map_err(Fatal)?;
      self.extraction_step(&mut plan,&pending,ExtractStep::Data{node:source,offset:start,count}).map_err(Fatal)?;
     }else{
      self.extraction_step(&mut plan,&pending,ExtractStep::Clone{source,slot}).map_err(Fatal)?;
      self.extraction_step(&mut plan,&pending,ExtractStep::Append{parent,child:slot}).map_err(Fatal)?;
      let fragment=self.extraction_slot(&mut plan,&pending).map_err(Fatal)?;
      self.extraction_step(&mut plan,&pending,ExtractStep::Fragment{document:DOCUMENT,slot:fragment}).map_err(Fatal)?;
      self.extraction_scratch(&plan,&pending,size_of::<Task>()).map_err(Fatal)?;pending.try_reserve_exact(1).map_err(|_|Fatal("Cannot reserve extraction continuation.".into()))?;pending.push(Task::Append{parent:slot,child:fragment});
      self.extraction_children(node,fragment,&mut plan,&mut pending).map_err(Fatal)?;
     }
    }
   }
  }
  self.extraction_scratch(&plan,&pending,0).map_err(Fatal)?;
  plan.bytes=plan.steps.capacity()*size_of::<ExtractStep>()+plan.slots.capacity()*size_of::<Option<u32>>();self.state.copies(plan.bytes).map_err(Fatal)?;self.state.retained+=plan.bytes;Ok(plan)
 }
 fn extraction_children(&mut self,node:usize,parent:usize,plan:&mut ExtractPlan,pending:&mut Vec<Task>)->Result<(),String>{
  let count=self.doc.nodes[node].children.len();self.state.work(count)?;let bytes=count.checked_mul(size_of::<Task>()).ok_or("Range extraction traversal overflow.")?;
  self.extraction_scratch(plan,pending,bytes)?;pending.try_reserve_exact(count).map_err(|_|"Cannot reserve extraction child traversal.")?;pending.extend(self.doc.nodes[node].children.iter().rev().map(|&node|Task::Visit{node,parent}));Ok(())
 }
}
