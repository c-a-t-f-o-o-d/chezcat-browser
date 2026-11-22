//! Inclusive containment follows ordinary inert parent links, never owners.
use super::*;
impl Bridge<'_>{
 pub(in crate::scripting) fn inert_contains_ancestry(&mut self,mut node:u32,sought:Option<u32>)->Result<(u32,bool),String>{
  let mut found=false;
  for _ in 0..self.inert_node_count(){
   self.state.work(self.state.inert_documents.adopted.lookup_work().checked_mul(2).and_then(|n|n.checked_add(1)).ok_or("Inert containment lookup work overflow.")?)?;
   let entry=self.state.inert_documents.node(node).ok_or("Missing inert containment Node.")?;
   found|=sought==Some(node);
   let Some(parent)=entry.parent else{return Ok((node,found));};
   let children=&self.state.inert_documents.node(parent).ok_or("Missing inert containment parent.")?.children;
   self.state.work(children.len())?;
   let children=&self.state.inert_documents.node(parent).unwrap().children;
   if children.iter().filter(|&&child|child==node).count()!=1{return Err("Invalid inert containment parent/child edge.".into());}
   node=parent;
  }
  Err("Cyclic inert containment ancestry.".into())
 }
}
