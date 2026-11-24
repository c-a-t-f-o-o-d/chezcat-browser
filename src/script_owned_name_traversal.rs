//! Owner-bound live names use real native links, namespaces and code units.
//! https://html.spec.whatwg.org/multipage/dom.html#dom-document-getelementsbyname
use super::*;
use crate::scripting::queries::NodeList;
use crate::html::{Attribute,Namespace};
impl Bridge<'_>{
    fn name_facts(&self,node:u32)->Result<Option<(Namespace,&[Attribute])>,String>{
        if let Some((owner,at))=self.frame_contains_node(node){let record=&self.contains_child_document(owner)?.nodes[at];return Ok(match &record.kind{NodeKind::Element{attrs,..}=>Some((record.namespace,attrs)),_=>None});}
        if let Some(source)=self.inert_import_source(node){return Ok(match source{
            script_inert_documents::ImportSource::Element(_)=>Some((Namespace::Html,self.inert_element_attributes(node).ok_or("Missing native name attributes.")?)),
            script_inert_documents::ImportSource::Parsed{node,..}=>match &node.kind{NodeKind::Element{attrs,..}=>Some((node.namespace,attrs)),_=>None},_=>None,
        });}
        if self.inert_node_type(node).is_some(){return Ok(None);}
        let record=self.doc.nodes.get(node_index(node).ok_or("Missing native name node.")?).ok_or("Invalid native name node.")?;
        Ok(match &record.kind{NodeKind::Element{attrs,..}=>Some((record.namespace,attrs)),_=>None})
    }
    fn rooted_name_matches(&mut self,index:usize,node:u32)->Result<bool,String>{
        let Some((namespace,attrs))=self.name_facts(node)?else{return Ok(false);};if namespace!=Namespace::Html{return Ok(false);}
        let work=attrs.iter().try_fold(attrs.len(),|cost,attr|cost.checked_add(attr.name.bytes()).ok_or("Name attribute scan work overflow."))?;self.state.work(work)?;
        let attrs=self.name_facts(node)?.unwrap().1;let Some(at)=attrs.iter().position(|attr|attr.name.html_is("name"))else{return Ok(false);};
        let NodeList::RootedName{name,..}=&self.state.node_lists[index]else{unreachable!()};let work=attrs[at].value.logical_bytes().min(name.logical_bytes()).checked_add(1).ok_or("Name comparison work overflow.")?;self.state.work(work)?;
        let NodeList::RootedName{name,..}=&self.state.node_lists[index]else{unreachable!()};Ok(self.name_facts(node)?.unwrap().1[at].value==*name)
    }
    fn name_child_count(&self,node:u32)->Result<usize,String>{
        if let Some((owner,at))=self.frame_contains_node(node){return Ok(self.contains_child_document(owner)?.nodes[at].children.len());}
        self.whole_text_child_count(node)
    }
    fn name_child_at(&self,node:u32,at:usize)->Result<u32,String>{
        if let Some((owner,index))=self.frame_contains_node(node){let child=*self.contains_child_document(owner)?.nodes[index].children.get(at).ok_or("Missing child name-query link.")?;return self.frame_child_node_handle(owner,child);}
        self.whole_text_child(node,at).ok_or("Missing name-query child.").map_err(Into::into)
    }
    fn name_push(&mut self,values:&mut Vec<u32>,node:u32,other:usize,available:usize)->Result<(),String>{
        if values.len()==values.capacity(){
            // During replacement both old and new backings are alive. Admit
            // that peak alongside the other scratch vector before moving data.
            let old=values.capacity().checked_mul(size_of::<u32>()).ok_or("Name scratch size overflow.")?;
            let free=available.checked_sub(other).and_then(|n|n.checked_sub(old.checked_mul(2)?)).ok_or("Name traversal scratch allowance exhausted.")?;
            let(replacement,_)=retained_storage::grow(values,1,free)?;
            if let Some(mut next)=replacement{self.state.copies(values.len().checked_mul(size_of::<u32>()).ok_or("Name scratch copy overflow.")?)?;next.append(values);*values=next;}
        }
        self.state.copies(size_of::<u32>())?;values.push(node);Ok(())
    }
    pub(in crate::scripting) fn rooted_name_list_nodes(&mut self,index:usize)->Result<Vec<u32>,String>{
        let NodeList::RootedName{root,..}=self.state.node_lists[index]else{return Err("Missing owner-bound name list.".into());};
        self.validate_name_list(root).map_err(|(_,e)|e)?;
        self.state.work(1)?;self.state.rebuild(self.doc)?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        let maximum=self.doc.nodes.len().checked_add(self.inert_node_count()).and_then(|n|n.checked_add(self.state.blank_frame_nodes)).and_then(|n|n.checked_add(1)).ok_or("Name traversal bound overflow.")?;
        let lookup=self.state.inert_documents.adopted.lookup_work().checked_add(8).and_then(|n|n.checked_mul(8)).ok_or("Name traversal lookup work overflow.")?;
        let mut pending=Vec::new();let mut found=Vec::new();let mut visited=0usize;
        self.state.work(lookup)?;let count=self.name_child_count(root)?;
        for at in (0..count).rev(){self.state.work(lookup)?;let child=self.name_child_at(root,at)?;self.name_push(&mut pending,child,0,available)?;}
        while let Some(node)=pending.pop(){
            visited=visited.checked_add(1).filter(|&n|n<=maximum).ok_or("Invalid or cyclic name-query tree.")?;self.state.work(lookup)?;self.native_node_access(node).map_err(|(_,e)|e)?;
            if self.rooted_name_matches(index,node)?{let other=pending.capacity().checked_mul(size_of::<u32>()).ok_or("Name scratch overflow.")?;self.name_push(&mut found,node,other,available)?;}
            if self.template_receiver(node){continue;}
            let count=self.name_child_count(node)?;
            for at in (0..count).rev(){self.state.work(lookup)?;let child=self.name_child_at(node,at)?;let other=found.capacity().checked_mul(size_of::<u32>()).ok_or("Name scratch overflow.")?;self.name_push(&mut pending,child,other,available)?;}
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn owned_name_list_scratch_growth_admits_old_and_replacement_backing_peak(){
  let mut doc=crate::html::parse("<html><body></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);let retained=page.state.retained;let mut b=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
  let mut pending=Vec::with_capacity(4);pending.extend([1,2,3,4]);let mut other=Vec::<u32>::with_capacity(1);other.push(8);let other_bytes=other.capacity()*size_of::<u32>();let minimum=other_bytes+4*size_of::<u32>()+5*size_of::<u32>();let copies=b.state.copy_work;
  assert!(b.name_push(&mut pending,5,other_bytes,minimum-1).is_err());assert_eq!(pending,[1,2,3,4]);assert_eq!(pending.capacity(),4);assert_eq!(b.state.retained,retained);
  b.name_push(&mut pending,5,other_bytes,minimum).unwrap();assert_eq!(pending,[1,2,3,4,5]);assert_eq!(pending.capacity(),5);assert_eq!(b.state.copy_work,copies-5*size_of::<u32>());assert_eq!(b.state.retained,retained);assert_eq!(other,[8]);
 }
}
