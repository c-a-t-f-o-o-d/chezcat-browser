//! Real element-only ParentNode facts without changing ownership or activation.
//! https://dom.spec.whatwg.org/#interface-parentnode
use super::*;
pub(super) fn member(name:&str)->bool{crate::javascript::PARENT_NODE_GETTERS.iter().any(|&(property,_)|property==name)}
pub(super) fn operation(name:&str)->Option<&'static str>{crate::javascript::PARENT_NODE_GETTERS.iter().find_map(|&(property,method)|(method==name).then_some(property))}
fn value(property:&str,first:Option<u32>,last:Option<u32>,count:usize)->Value {
    if property=="childElementCount" {Value::Number(count as f64)}
    else {if property=="firstElementChild"{first}else{last}.map_or(Value::Null,Value::Host)}
}
impl Bridge<'_> {
    pub(super) fn parent_node_receiver(&self,object:u32)->bool {
        if let Some((_,node))=self.frame_contains_node(object){return node==0||self.frame_element_sibling_receiver(object);}
        if object==DOCUMENT{return true;}
        if let Some(kind)=self.inert_node_type(object){return matches!(kind,1|9|11);}
        node_index(object).is_some_and(|node|node!=0&&self.dom_accessible(node)&&self.doc.nodes.get(node)
            .is_some_and(|node|matches!(node.kind,NodeKind::Element{..}|NodeKind::DocumentFragment)))
    }
    pub(super) fn parent_node_get(&mut self,object:u32,property:&str)->Result<Value,String> {
        if !member(property)||!self.parent_node_receiver(object){return Err("ParentNode getter requires a genuine accessible parent node.".into());}
        if property=="children"{if let Some((owner,_))=self.frame_contains_node(object){self.contains_child_document(owner)?;}return self.create_children_collection(object);}
        let(mut first,mut last,mut count)=(None,None,0usize);
        if let Some((owner,node))=self.frame_contains_node(object){
            let length=self.contains_child_document(owner)?.nodes[node].children.len();
            self.state.work(length.saturating_add(1))?;
            let document=self.contains_child_document(owner)?;
            for &child in &document.nodes[node].children {
                if matches!(document.nodes[child].kind,NodeKind::Element{..}) {
                    let handle=self.frame_child_node_handle(owner,child)?;
                    if first.is_none(){first=Some(handle);}last=Some(handle);count+=1;
                }
            }
        }else if self.template_receiver(object){
            // A template element's descendants belong to its content fragment.
            self.state.work(1)?;
        }else if let Some(children)=self.inert_children(object){
            let length=children.len();self.state.work(length.saturating_add(1))?;
            for &child in self.inert_children(object).ok_or("Inert ParentNode is unavailable.")? {
                if self.inert_node_type(child)==Some(1){
                    if first.is_none(){first=Some(child);}last=Some(child);count+=1;
                }
            }
        }else{
            self.state.rebuild(self.doc)?;
            let root=if object==DOCUMENT{0}else{self.dom_node(object)?};
            self.check_iframe_child_topology(root)?;
            self.state.work(self.doc.nodes[root].children.len().saturating_add(1))?;
            for &child in &self.doc.nodes[root].children {
                if matches!(self.doc.nodes[child].kind,NodeKind::Element{..}){
                    let handle=node_handle(child);if first.is_none(){first=Some(handle);}last=Some(handle);count+=1;
                }
            }
        }
        Ok(value(property,first,last,count))
    }
}

impl Bridge<'_> {
    // A temporary view is admitted against the same host allowance as queries;
    // the live collection itself stores only the stable owner handle.
    pub(super) fn parent_element_nodes(&mut self,object:u32)->Result<Vec<u32>,String> {
        if !self.parent_node_receiver(object){return Err("Children collection owner is unavailable.".into());}
        let length=if let Some((owner,node))=self.frame_contains_node(object){self.contains_child_document(owner)?.nodes[node].children.len()}
            else if self.template_receiver(object){0}
            else if let Some(children)=self.inert_children(object){children.len()}
            else {self.state.rebuild(self.doc)?;let root=if object==DOCUMENT{0}else{self.dom_node(object)?};self.check_iframe_child_topology(root)?;self.doc.nodes[root].children.len()};
        self.state.work(length.saturating_add(1))?;
        self.state.copies(length.saturating_mul(size_of::<u32>()))?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        length.checked_mul(size_of::<u32>()).filter(|bytes|*bytes<=available).ok_or("DOM children view scratch allowance exhausted.")?;
        let mut nodes=Vec::new();nodes.try_reserve_exact(length).map_err(|_|"Could not reserve children view.")?;
        if nodes.capacity()*size_of::<u32>()>available{return Err("DOM children view allocation exceeds allowance.".into());}
        if let Some((owner,node))=self.frame_contains_node(object){
            for &child in &self.contains_child_document(owner)?.nodes[node].children {
                if matches!(self.contains_child_document(owner)?.nodes[child].kind,NodeKind::Element{..}) {
                    nodes.push(self.frame_child_node_handle(owner,child)?);
                }
            }
        }else if self.template_receiver(object){}
        else if let Some(children)=self.inert_children(object){
            for &child in children {if self.inert_node_type(child)==Some(1){nodes.push(child);}}
        }else{
            let root=if object==DOCUMENT{0}else{self.dom_node(object)?};
            for &child in &self.doc.nodes[root].children {if matches!(self.doc.nodes[child].kind,NodeKind::Element{..}){nodes.push(node_handle(child));}}
        }
        Ok(nodes)
    }
}
