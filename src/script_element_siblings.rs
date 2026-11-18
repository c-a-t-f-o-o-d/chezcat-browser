//! Finite NonDocumentTypeChildNode reads over real, guarded sibling vectors.
//! https://dom.spec.whatwg.org/#interface-nondocumenttypechildnode
use super::*;

#[derive(Clone,Copy,PartialEq,Eq)]
pub(super) enum Family { Element, CharacterData }

pub(super) fn property(name:&str)->bool {
    matches!(name,"nextElementSibling"|"previousElementSibling")
}

pub(super) fn operation(name:&str)->Option<(Family,bool)> {
    Some(match name {
        "\0Element.get.nextElementSibling"=>(Family::Element,true),
        "\0Element.get.previousElementSibling"=>(Family::Element,false),
        "\0CharacterData.get.nextElementSibling"=>(Family::CharacterData,true),
        "\0CharacterData.get.previousElementSibling"=>(Family::CharacterData,false),
        _=>return None,
    })
}

impl Bridge<'_> {
    pub(super) fn element_sibling_family(&self,object:u32)->Option<Family> {
        if self.frame_element_sibling_receiver(object) {return Some(Family::Element);}
        let node=node_index(object).filter(|&node|self.dom_accessible(node))?;
        match self.doc.nodes.get(node)?.kind {
            NodeKind::Element {..}=>Some(Family::Element),
            NodeKind::Text(_)|NodeKind::Comment(_)=>Some(Family::CharacterData),
            NodeKind::DocumentFragment=>None,
        }
    }

    pub(super) fn element_sibling_member(&self,object:u32,name:&str)->bool {
        property(name)&&self.element_sibling_family(object).is_some()
            && !self.form_named_control(object,name)
    }

    // Existing ownership/inert admission remains authoritative. A physical
    // ancestry containing an unsupported foreign or inaccessible node cannot
    // become traversable merely because this operation returns no text.
    fn element_sibling_ancestry(&mut self,node:usize)->Result<(),String> {
        let mut current=node;
        for _ in 0..self.doc.nodes.len() {
            self.state.work(1)?;
            let item=self.doc.nodes.get(current).ok_or("Invalid element-sibling ancestry index.")?;
            if current==0 {return Ok(());}
            if item.namespace==crate::html::Namespace::UnknownForeign
                || matches!(item.kind,NodeKind::Element {..}) && item.namespace!=crate::html::Namespace::Html
                || !self.dom_accessible(current) {
                return Err("Element sibling ancestry is inert or has unknown namespace provenance.".into());
            }
            if current!=node && matches!(item.kind,NodeKind::Text(_)|NodeKind::Comment(_)) {
                return Err("CharacterData cannot own an element-sibling subtree.".into());
            }
            let parent=self.state.tree.parents.get(current).ok_or("Invalid element-sibling parent index.")?;
            let Some(parent)=*parent else {return Ok(());};
            current=parent;
        }
        Err("Cyclic element-sibling ancestry.".into())
    }

    pub(super) fn element_sibling(&mut self,object:u32,next:bool)->Result<Value,String> {
        if self.is_frame_handle(object) {return self.frame_element_sibling(object,next);}
        let node=self.dom_node(object)?;
        if self.element_sibling_family(object).is_none() {
            return Err("Element sibling access requires an Element or CharacterData node.".into());
        }
        self.element_sibling_ancestry(node)?;
        let parent=*self.state.tree.parents.get(node).ok_or("Invalid element-sibling receiver index.")?;
        let Some(parent)=parent else {return Ok(Value::Null);};
        let siblings=&self.doc.nodes.get(parent).ok_or("Invalid element-sibling parent.")?.children;
        self.state.work(siblings.len().saturating_mul(2))?;
        let mut position=None;
        for (at,&child) in siblings.iter().enumerate() {
            if child>=self.doc.nodes.len() || self.state.tree.parents.get(child)!=Some(&Some(parent)) {
                return Err("Element sibling parent/child identity mismatch.".into());
            }
            if child==node {
                if position.replace(at).is_some() {return Err("Repeated element-sibling receiver.".into());}
            }
        }
        let at=position.ok_or("Element sibling receiver is absent from its parent.")?;
        let count=if next {siblings.len()-at-1}else{at};
        for offset in 0..count {
            let child=siblings[if next {at+1+offset}else{at-1-offset}];
            let item=&self.doc.nodes[child];
            if item.namespace==crate::html::Namespace::UnknownForeign
                || matches!(item.kind,NodeKind::Element {..}) && item.namespace!=crate::html::Namespace::Html
                || !self.dom_accessible(child) {
                return Err("Element sibling is inert or has unknown namespace provenance.".into());
            }
            match item.kind {
                NodeKind::Element {..}=>return Ok(Value::Host(node_handle(child))),
                NodeKind::Text(_)|NodeKind::Comment(_)=>{},
                NodeKind::DocumentFragment=>return Err("A DocumentFragment cannot be a published sibling.".into()),
            }
        }
        Ok(Value::Null)
    }
}
