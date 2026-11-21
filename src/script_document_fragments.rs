//! Genuine detached DocumentFragment Nodes and one atomic donor-child splice.
//! https://dom.spec.whatwg.org/#interface-documentfragment
//! Fragment insertion never inserts the container or creates a rendering box.
use super::*;

impl Bridge<'_> {
    pub(super) fn fragment_receiver(&self, object:u32)->bool {
        node_index(object).is_some_and(|node|self.doc.nodes.get(node)
            .is_some_and(|item|matches!(item.kind,NodeKind::DocumentFragment)) && self.dom_accessible(node))
    }

    pub(super) fn splice_document_fragment(&mut self,parent:usize,method:&str,args:&[Value])->Result<Value,String> {
        self.state.rebuild(self.doc)?;
        let Some(Value::Host(handle))=args.first() else{return Err("A genuine DocumentFragment is required.".into());};
        if !self.fragment_receiver(*handle) {return Err("A genuine DocumentFragment is required.".into());}
        let donor=self.dom_node(*handle)?;
        if !matches!(self.doc.nodes[parent].kind,NodeKind::Element {..}|NodeKind::DocumentFragment) {
            return Err("HierarchyRequestError: This Node cannot contain children.".into());
        }
        if donor==parent || self.state.tree.parents[donor].is_some() || self.state.dom.connected[donor] {
            return Err("HierarchyRequestError: A fragment container must remain a detached root.".into());
        }
        let old=if method=="replaceChild" {
            let Some(Value::Host(old))=args.get(1) else{return Err("replaceChild requires two Nodes.".into());};
            Some(self.dom_node(*old)?)
        } else {None};
        let reference=if method=="insertBefore" {
            match args.get(1) {Some(Value::Host(reference))=>Some(self.dom_node(*reference)?),Some(Value::Null|Value::Undefined)=>None,
                _=>return Err("insertBefore requires a Node or null reference.".into())}
        } else {old};
        if let Some(reference)=reference {
            self.state.work(self.doc.nodes[parent].children.len())?;
            if self.state.tree.parents[reference]!=Some(parent)
                || self.doc.nodes[parent].children.iter().filter(|&&node|node==reference).count()!=1 {
                return Err("NotFoundError: Reference is not a child of the destination.".into());
            }
        }
        self.mutation_ancestors(parent)?;
        if !matches!(self.doc.nodes[parent].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg) && !self.fragment_receiver(node_handle(parent)) {
            return Err("Fragment insertion requires a known HTML, SVG or fragment container; no tree was changed.".into());
        }
        if matches!(tag(self.doc,parent),"script"|"style"|"link"|"iframe"|"select"|"option"|"optgroup") {
            return Err("Fragment insertion into resource or native-control interiors is not implemented; no tree was changed.".into());
        }
        let root_count=self.doc.nodes[donor].children.len();
        self.state.work(root_count)?;
        self.state.copies(root_count.checked_mul(std::mem::size_of::<usize>()).ok_or("Fragment child copy overflow.")?)?;
        let mut roots=Vec::new();roots.try_reserve(root_count).map_err(|_|"Fragment root reservation failed.")?;
        roots.extend(self.doc.nodes[donor].children.iter().copied());
        let mut incoming=Vec::new();let mut seen=HashSet::new();let mut maximum_depth=0;
        self.state.work(self.doc.nodes.len())?;
        self.state.copies(self.doc.nodes.len().checked_mul(8*std::mem::size_of::<usize>()).ok_or("Fragment staging copy overflow.")?)?;
        seen.try_reserve(self.doc.nodes.len()).map_err(|_|"Fragment topology reservation failed.")?;
        for &root in &roots {
            if root>=self.doc.nodes.len() || self.state.tree.parents[root]!=Some(donor) {return Err("Fragment donor child identity mismatch.".into());}
            let(nodes,depth)=self.mutation_subtree_with_styles(root)?;
            maximum_depth=maximum_depth.max(depth);
            incoming.try_reserve(nodes.len()).map_err(|_|"Fragment descendant reservation failed.")?;
            for node in nodes {
                if !seen.insert(node) {return Err("Fragment donor repeats a Node identity.".into());}
                if node==parent {return Err("HierarchyRequestError: Fragment insertion would create a cycle.".into());}
                if matches!(self.doc.nodes[node].kind,NodeKind::DocumentFragment)
                    || self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign
                    || matches!(tag(self.doc,node),"script"|"link"|"iframe") {
                    return Err("Fragment batch resource, nested-container or foreign insertion is not implemented; no tree was changed.".into());
                }
                incoming.push(node);
            }
        }
        if tag(self.doc,parent)=="picture" && roots.iter().any(|&node|tag(self.doc,node)=="img") {
            return Err("Dynamic picture image selection is not implemented; no tree was changed.".into());
        }
        let mut ancestor=Some(parent);let mut parent_depth:usize=0;
        while let Some(node)=ancestor {
            self.state.work(1)?;parent_depth+=1;
            if node==donor || parent_depth>128 {return Err("HierarchyRequestError: Fragment ancestry is cyclic or too deep.".into());}
            ancestor=self.state.tree.parents[node];
        }
        if parent_depth.saturating_add(maximum_depth)>128 {return Err("Fragment insertion depth limit reached; no tree was changed.".into());}
        let outgoing=if let Some(old)=old {self.mutation_subtree_with_styles(old)?.0} else {Vec::new()};
        if outgoing.iter().any(|&node|self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign
            || matches!(tag(self.doc,node),"script"|"link"|"iframe")) {
            return Err("Fragment replacement cannot retire foreign or script/stylesheet/frame resources; no tree was changed.".into());
        }
        if outgoing.iter().any(|node|seen.contains(node)) {return Err("Fragment source overlaps the replaced subtree.".into());}
        self.validate_button_children(parent,&incoming)?;
        if roots.is_empty() && old.is_none() {return Ok(Value::Host(*handle));}

        let total=self.doc.nodes[parent].children.len().checked_add(roots.len()).ok_or("Fragment destination size overflow.")?;
        self.state.work(total.saturating_add(incoming.len()).saturating_add(outgoing.len()))?;
        let final_count = total.checked_sub(usize::from(old.is_some())).ok_or("Fragment final child count underflow.")?;
        self.state.copies(final_count.checked_mul(std::mem::size_of::<usize>()).ok_or("Fragment destination copy overflow.")?)?;
        let mut children=Vec::new();children.try_reserve_exact(final_count).map_err(|_|"Fragment destination reservation failed.")?;
        let position=reference.and_then(|reference|self.doc.nodes[parent].children.iter().position(|&node|node==reference))
            .unwrap_or(self.doc.nodes[parent].children.len());
        for (index,&node) in self.doc.nodes[parent].children.iter().enumerate() {
            if index==position {children.extend(roots.iter().copied());}
            if Some(node)!=old {children.push(node);}
        }
        if position==self.doc.nodes[parent].children.len() {children.extend(roots.iter().copied());}
        let mut overrides=Vec::new();overrides.try_reserve(roots.len()+usize::from(old.is_some())).map_err(|_|"Fragment owner projection reservation failed.")?;
        for &root in &roots {overrides.push((root,Some(parent)));}
        if let Some(old)=old {overrides.push((old,None));}
        for &root in &roots{self.validate_svg_edge(node_handle(parent),node_handle(root))?;}
        let connected=self.state.dom.connected[parent];
        if incoming.iter().any(|&node|tag(self.doc,node)=="style"){self.validate_style_destination(parent)?;}
        let sheets=self.prepare_style_transfers(&incoming,&outgoing,connected)?;
        let form_plan=self.prepare_select_form_change(None,&overrides)?;
        let images=self.prepare_image_connections(&incoming,connected)?;
        let bytes = child_link_bytes(&children).checked_add(form_plan.retained_bytes).ok_or("Fragment child/native storage overflow.")?;
        self.state.prepare_range_changes(3,incoming.len().saturating_add(outgoing.len()))?;
        self.state.replace_storage(child_link_bytes(&self.doc.nodes[parent].children), bytes, form_plan.retained_bytes)?;
        self.state.ranges_remove(donor,0,roots.len(),&incoming);
        if old.is_some() { self.state.ranges_remove(parent,position,1,&outgoing); }
        self.state.ranges_insert(parent,position,roots.len());
        // All validation, quota checks, allocations and native plans precede
        // the first write. No author callback or fallible work in publication.
        self.doc.nodes[donor].children.clear();
        self.doc.nodes[parent].children=children;
        for &(root,new_parent) in &overrides {self.state.tree.parents[root]=new_parent;}
        for node in outgoing {self.state.dom.connected[node]=false;self.state.detached_nodes.insert(node);}
        for node in incoming {
            self.state.dom.connected[node]=connected;
            if connected {self.state.detached_nodes.remove(&node);} else {self.state.detached_nodes.insert(node);}
        }
        form_plan.commit(self.forms);self.commit_image_connections(images);self.commit_inline_sheet_batch(sheets);
        self.state.dom_committed();self.state.dirty=true;
        if connected {self.state.layout_changed();}
        Ok(Value::Host(old.map_or(*handle,node_handle)))
    }
}

#[cfg(test)]
#[path="script_document_fragment_tests.rs"]
mod tests;
