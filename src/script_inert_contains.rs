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

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn node_equality_inert_contains_work_refusal_preserves_tree_and_recovers(){
  let mut doc=crate::html::parse("<html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);assert_eq!(page.console_eval(&mut doc,&mut forms,"var owner=new Document(),a=owner.createElement('div'),b=owner.createTextNode('native');owner.appendChild(a);a.appendChild(b);").errors,0);
  let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};let Value::Host(a)=page.runtime.execute("a",&mut bridge).unwrap()else{panic!("a")};let Value::Host(b)=page.runtime.execute("b",&mut bridge).unwrap()else{panic!("b")};let before=(bridge.state.retained,bridge.state.dom_commits,bridge.state.inert_documents.entries.len(),bridge.state.inert_documents.node(b).unwrap().parent);let work=bridge.state.host_work;bridge.state.host_work=1;assert!(bridge.node_contains(a,&[Value::Host(b)]).unwrap_err().contains("work limit"));assert_eq!((bridge.state.retained,bridge.state.dom_commits,bridge.state.inert_documents.entries.len(),bridge.state.inert_documents.node(b).unwrap().parent),before);bridge.state.host_work=work;assert!(matches!(bridge.node_contains(a,&[Value::Host(b)]).unwrap(),Value::Bool(true)));assert_eq!((bridge.state.retained,bridge.state.dom_commits,bridge.state.inert_documents.entries.len(),bridge.state.inert_documents.node(b).unwrap().parent),before);
 }
}
