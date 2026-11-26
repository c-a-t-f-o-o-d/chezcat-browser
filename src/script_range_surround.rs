//! Native surrounding preflight precedes extraction and preserves error order.
//! https://dom.spec.whatwg.org/#dom-range-surroundcontents
use super::*;
impl Bridge<'_>{
 pub(in crate::scripting) fn range_surround_append_parent(&mut self,parent:u32)->Result<(),Error>{
  let node=self.range_node(parent).map_err(Error::Fatal)?;
  if !matches!(self.doc.nodes[node].kind,NodeKind::Element{..}|NodeKind::DocumentFragment){return Err(Error::Exception("HierarchyRequestError","The surrounding Node cannot contain the extracted fragment.".into()));}Ok(())
 }
 pub(in crate::scripting) fn range_surround_parent(&mut self,receiver:u32,args:&[Value])->Result<u32,Error>{
  use Error::{Exception,Fatal};
  let range=*self.range_record(receiver).filter(|r|!r.fixed).ok_or_else(||Exception("TypeError","A genuine live Range is required.".into()))?;
  let Some(Value::Host(parent))=args.first()else{return Err(Exception("TypeError","Range surrounding requires a genuine Node.".into()));};let parent=*parent;
  let mut common=range.start.node;
  while common!=range.end.node&&self.range_ancestor_child(common,range.end.node).map_err(Fatal)?.is_none(){common=self.range_parent(common).map_err(Fatal)?.ok_or_else(||Fatal("Range surrounding has no common ancestor.".into()))?;}
  for mut node in [range.start.node,range.end.node]{
   while node!=common{
    let index=self.range_node(node).map_err(Fatal)?;
    if !matches!(self.doc.nodes[index].kind,NodeKind::Text(_)|NodeKind::Cdata(_)){return Err(Exception("InvalidStateError","A non-Text Node is partially contained in this Range.".into()));}
    node=self.range_parent(node).map_err(Fatal)?.ok_or_else(||Fatal("Range surrounding boundary ancestry disappeared.".into()))?;
   }
  }
  if parent==DOCUMENT||self.inert_document(parent)||self.doctype_receiver(parent)||self.fragment_receiver(parent)||matches!(self.inert_node_type(parent),Some(9|10|11))||matches!(self.frame_traversal_node_type(parent),Some(9|10|11)){
   return Err(Exception("InvalidNodeTypeError","A Document, DocumentType or DocumentFragment cannot surround Range contents.".into()));
  }
  Ok(parent)
 }
}
