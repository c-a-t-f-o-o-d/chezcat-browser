//! Atomic child movement within one genuine inert owner document.
use super::*;

#[cfg(test)]
#[path = "script_inert_mutation_tests.rs"]
mod tests;

impl Bridge<'_> {
    pub(in crate::scripting) fn inert_mutation_receiver(&self, object:u32)->bool {
        matches!(self.inert_node_type(object),Some(1|11))||self.inert_document_children_receiver(object)
    }

    pub(in crate::scripting) fn validate_inert_mutation(&self, parent:u32, method:&str, args:&[Value])->Result<(),(&'static str,String)> {
        let Some(Value::Host(child))=args.first() else{return Err(("TypeError","Inert child mutation requires a genuine Node.".into()));};
        let arena=&self.state.inert_documents;
        if method!="removeChild"{
            let mut ancestor=Some(parent);let bound=arena.entries.len().saturating_add(arena.adopted.len()).saturating_add(self.doc.nodes.len()).saturating_add(1);
            for _ in 0..bound{let Some(handle)=ancestor else{break;};if handle==*child{return Err(("HierarchyRequestError","Inert insertion would create an ancestor cycle.".into()));}ancestor=arena.node(handle).and_then(|node|node.parent.or_else(||if let Kind::Fragment{template}=node.kind{template}else{None}));}
            if ancestor.is_some(){return Err(("NotSupportedError","Inert ancestor topology is inconsistent.".into()));}
        }
        if method=="insertBefore" {
            match args.get(1) {
                Some(Value::Null|Value::Undefined)=>{},
                Some(Value::Host(reference))=>{
                    if !self.get_root_receiver(*reference){return Err(("TypeError","Inert insertion reference requires a genuine Node.".into()));}
                    if arena.node(*reference).is_none_or(|node|node.parent!=Some(parent)){
                        return Err(("NotFoundError","The reference is not a child of this inert parent.".into()));
                    }
                },
                _=>return Err(("TypeError","Inert insertBefore requires a Node or null reference.".into())),
            }
        }
        if method!="removeChild"&&self.doctype_receiver(*child)&&!self.inert_document_children_receiver(parent){return Err(("HierarchyRequestError","DocumentType requires a Document parent.".into()));}
        let Some(source)=arena.node(*child) else {
            if method=="appendChild" && self.template_fragment_receiver(parent){return self.validate_template_append(parent,args);}
            return Err(("NotSupportedError","Inert child mutation requires a node in the same inert owner document.".into()));
        };
        if method=="removeChild" {
            if source.parent!=Some(parent){return Err(("NotFoundError","The Node is not a child of this inert parent.".into()));}
        }else{
            if !matches!(self.inert_node_type(*child),Some(1|3|4|7|8|11))&&!(self.inert_document_children_receiver(parent)&&self.inert_node_type(*child)==Some(10)) {
                return Err(("HierarchyRequestError","This inert Node cannot be inserted as a child.".into()));
            }
            if source.owner!=arena.node(parent).ok_or(("TypeError","Missing inert parent.".into()))?.owner {
                return Err(("NotSupportedError","Cross-document inert adoption is not implemented.".into()));
            }
        }
        self.validate_document_children(parent,method,args)
    }

    pub(in crate::scripting) fn mutate_inert_children(&mut self,parent:u32,method:&str,args:&[Value])->Result<Value,String> {
        self.validate_inert_mutation(parent,method,args).map_err(|(_,message)|message)?;
        let Value::Host(child)=args[0] else{unreachable!()};
        if self.state.inert_documents.node(child).is_none(){return self.append_template_child(parent,args);}
        let source=self.state.inert_documents.node(child).unwrap();
        let old_parent=source.parent;
        let fragment=matches!(source.kind,Kind::Fragment{..});
        if method!="removeChild"&&!self.inert_document_children_receiver(parent){
            if fragment{let length=self.inert_children(child).unwrap().len();self.state.work(length)?;for slot in 0..length{let root=self.inert_children(child).unwrap()[slot];self.validate_svg_edge(parent,root)?;}}
            else{self.validate_svg_edge(parent,child)?;}
        }

        let reference=if method=="insertBefore"{if let Some(Value::Host(handle))=args.get(1){Some(*handle)}else{None}}else{None};
        if reference==Some(child){return Ok(Value::Host(child));}
        if method=="removeChild" {
            self.prepare_document_children_sync(parent,0)?;
            let children=&self.state.inert_documents.node(parent).unwrap().children;
            self.state.work(children.len())?;
            let slot=self.state.inert_documents.node(parent).unwrap().children.iter().position(|&node|node==child).ok_or("Inert child link is unavailable.")?;
            let moved=self.state.inert_documents.node(parent).unwrap().children.len()-slot-1;
            self.state.copies(moved.checked_mul(size_of::<u32>()).ok_or("Inert removal copy overflow.")?)?;
            self.state.inert_documents.node_mut(parent).unwrap().children.remove(slot);
            self.state.inert_documents.node_mut(child).unwrap().parent=None;
            self.sync_document_children(parent);
            self.state.dom_committed();return Ok(Value::Host(child));
        }
        let mut ancestor=Some(parent);
        while let Some(handle)=ancestor {
            self.state.work(1)?;
            if handle==child{return Err("HierarchyRequestError: Inert insertion would create a cycle.".into());}
            // A template content's host can still live in the main arena.
            ancestor=self.state.inert_documents.node(handle).and_then(|node|
                node.parent.or_else(||if let Kind::Fragment{template}=node.kind{template}else{None}));
        }
        let count=if fragment{self.state.inert_documents.node(child).unwrap().children.len()}else{1};
        self.prepare_document_children_sync(parent,count)?;
        if let Some(old)=old_parent{self.prepare_document_children_sync(old,0)?;}
        self.state.work(count)?;
        if fragment {
            let source=self.state.inert_documents.node(child).unwrap();
            if old_parent.is_some() || source.children.iter().any(|handle|self.state.inert_documents.node(*handle).is_none_or(|node|node.parent!=Some(child)||node.owner!=source.owner)){
                return Err("Inert fragment child ownership is inconsistent.".into());
            }
            if count==0{return Ok(Value::Host(child));}
        }
        let old_slot=if let Some(old)=old_parent {
            let children=&self.state.inert_documents.node(old).ok_or("Inert source parent is unavailable.")?.children;
            self.state.work(children.len())?;
            Some(self.state.inert_documents.node(old).unwrap().children.iter().position(|&handle|handle==child).ok_or("Inert source child link is unavailable.")?)
        }else{None};
        if reference.is_some(){self.state.work(self.state.inert_documents.node(parent).unwrap().children.len())?;}
        let children=&self.state.inert_documents.node(parent).unwrap().children;
        if reference.is_none() && old_parent==Some(parent) && old_slot==children.len().checked_sub(1){return Ok(Value::Host(child));}
        let insertion=if let Some(reference)=reference{children.iter().position(|&handle|handle==reference).ok_or("Inert reference child link is unavailable.")?}else{children.len()};
        let insertion=insertion-usize::from(old_parent==Some(parent)&&old_slot.unwrap()<insertion);
        let tail=children.len()-usize::from(old_parent==Some(parent))-insertion;
        let added=count.checked_sub(usize::from(old_parent==Some(parent))).ok_or("Inert child count overflow.")?;
        let (replacement,bytes)=retained_storage::grow(children,added,self.state.retained_limit.saturating_sub(self.state.retained))?;
        let moved=if replacement.is_some(){children.len()}else{0};
        let removed=old_parent.map_or(0,|old|self.state.inert_documents.node(old).unwrap().children.len()-old_slot.unwrap()-1);
        let writes=count.checked_mul(2).ok_or("Inert insertion initialization overflow.")?;
        self.state.work(moved.saturating_add(removed).saturating_add(writes).saturating_add(tail))?;
        let copies=moved.checked_add(removed).and_then(|n|n.checked_add(writes)).and_then(|n|n.checked_add(tail)).and_then(|n|n.checked_mul(size_of::<u32>())).ok_or("Inert child move copy overflow.")?;
        self.state.replace_storage(0,bytes,copies)?;
        let arena=&mut self.state.inert_documents;
        if let Some(mut next)=replacement{let children=&mut arena.node_mut(parent).unwrap().children;next.append(children);*children=next;}
        if let Some(old)=old_parent{arena.node_mut(old).unwrap().children.remove(old_slot.unwrap());}
        // Growth is already admitted. Shift the destination tail once, rather
        // than repeating a vector insert for every incoming fragment child.
        let children=&mut arena.node_mut(parent).unwrap().children;
        let length=children.len();children.resize(length+count,0);children.copy_within(insertion..length,insertion+count);
        if fragment {
            for index in 0..count {
                let handle=arena.node(child).unwrap().children[index];
                arena.node_mut(handle).unwrap().parent=Some(parent);
                arena.node_mut(parent).unwrap().children[insertion+index]=handle;
            }
            arena.node_mut(child).unwrap().children.clear();
        }else{
            arena.node_mut(child).unwrap().parent=Some(parent);
            arena.node_mut(parent).unwrap().children[insertion]=child;
        }
        self.sync_document_children(parent);if let Some(old)=old_parent{if old!=parent{self.sync_document_children(old);}}
        self.state.dom_committed();Ok(Value::Host(child))
    }
}
