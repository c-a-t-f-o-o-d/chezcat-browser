//! Preflight child-list observer records against the native tree transaction.
//! DOM argument, topology, and resource admission remain owned by the mutation.
use super::*;
use crate::javascript::NativeChildListChange;

fn removal(parent: usize, nodes: Vec<u32>, before: Option<u32>, after: Option<u32>) -> NativeChildListChange {
    NativeChildListChange {
        target: if parent == 0 { DOCUMENT } else { node_handle(parent) },
        added: Vec::new(), removed: nodes, previous: before, next: after,
    }
}

impl Bridge<'_> {
    pub(super) fn child_list_change_plan(&mut self, receiver: u32, method: &str, args: &[Value])
        -> Result<Vec<NativeChildListChange>, String>
    {
        if method==xml_documents::CREATE{return self.xml_doctype_adoption_changes(args);}
        if method==DOCUMENT_ADOPT_NODE{return self.adoption_changes(args);}
        if !matches!(method, "appendChild" | "insertBefore" | "removeChild" | "replaceChild") {
            return Ok(Vec::new());
        }
        if method=="replaceChild"&&self.inert_replace_receiver(receiver){return self.inert_replace_changes(receiver,args);}
        if self.inert_mutation_receiver(receiver){return self.inert_child_list_change_plan(receiver,method,args);}
        self.state.rebuild(self.doc)?;
        let Some(parent) = node_index(receiver).filter(|&node| self.dom_accessible(node)) else {
            return Ok(Vec::new());
        };
        let Some(Value::Host(handle)) = args.first() else { return Ok(Vec::new()); };
        if matches!(method,"appendChild"|"insertBefore")&&self.inert_node_type(*handle).is_some_and(|kind|kind!=11){return self.inert_direct_insertion_changes(receiver,method,args);}
        if matches!(method, "appendChild" | "insertBefore") && self.checked_inert_fragment(*handle).is_some() {
            let children = self.inert_children(*handle).unwrap();
            if children.is_empty() { return Ok(Vec::new()); }
            let count = children.len();
            self.state.work(count.saturating_add(self.doc.nodes[parent].children.len()))?;
            self.state.copies(count.saturating_mul(2 * std::mem::size_of::<u32>()))?;
            let added = self.inert_children(*handle).unwrap().to_vec();
            let siblings = &self.doc.nodes[parent].children;
            let position = if let ("insertBefore", Some(Value::Host(reference))) = (method, args.get(1)) {
                let Some(index) = node_index(*reference).and_then(|node| siblings.iter().position(|&child| child == node))
                    else { return Ok(Vec::new()); };
                index
            } else {
                siblings.len()
            };
            let previous = position.checked_sub(1).map(|index| node_handle(siblings[index]));
            let next = siblings.get(position).copied().map(node_handle);
            return Ok(vec![
                NativeChildListChange { target: *handle, added: Vec::new(), removed: added.clone(), previous: None, next: None },
                NativeChildListChange { target: receiver, added, removed: Vec::new(), previous, next },
            ]);
        }
        let Some(source) = node_index(*handle).filter(|&node| self.dom_accessible(node)) else {
            return Ok(Vec::new());
        };
        let reference = match args.get(1) {
            Some(Value::Host(handle)) if matches!(method, "insertBefore" | "replaceChild") => {
                let Some(node) = node_index(*handle).filter(|&node| self.dom_accessible(node)) else {
                    return Ok(Vec::new());
                };
                Some(node)
            }
            _ => None,
        };
        let old_parent = self.state.tree.parents[source];
        if method=="replaceChild"&&reference==Some(source)&&old_parent==Some(parent) {
            let siblings=&self.doc.nodes[parent].children;let length=siblings.len();
            let position=siblings.iter().position(|&node|node==source).ok_or("Self replacement observer child link is unavailable.")?;
            let previous=position.checked_sub(1).and_then(|at|siblings.get(at)).copied().map(node_handle);
            let next=siblings.get(position+1).copied().map(node_handle);
            self.state.work(length)?;self.state.copies(2*std::mem::size_of::<NativeChildListChange>()+2*std::mem::size_of::<u32>())?;
            let mut removed=Vec::new();removed.try_reserve_exact(1).map_err(|_|"Cannot reserve self replacement removal.")?;removed.push(*handle);
            let mut added=Vec::new();added.try_reserve_exact(1).map_err(|_|"Cannot reserve self replacement insertion.")?;added.push(*handle);
            let mut changes=Vec::new();changes.try_reserve_exact(2).map_err(|_|"Cannot reserve self replacement records.")?;
            changes.push(NativeChildListChange{target:receiver,added:Vec::new(),removed,previous,next});
            changes.push(NativeChildListChange{target:receiver,added,removed:Vec::new(),previous,next});
            return Ok(changes);
        }
        if reference == Some(source)
            || reference.is_some_and(|node| self.state.tree.parents[node] != Some(parent))
            || method == "removeChild" && old_parent != Some(parent)
            || method == "replaceChild" && reference.is_none()
        {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        let donor = self.fragment_receiver(*handle) && method != "removeChild";
        let added = if donor {
            let children = &self.doc.nodes[source].children;
            self.state.work(children.len())?;
            self.state.copies(children.len().saturating_mul(std::mem::size_of::<u32>()))?;
            let mut handles = Vec::new();
            handles.try_reserve_exact(children.len()).map_err(|_| "Cannot reserve fragment mutation records.")?;
            handles.extend(children.iter().copied().map(node_handle));
            if !handles.is_empty() {
                records.push(removal(source, handles.clone(), None, None));
            }
            handles
        } else {
            if let Some(old_parent) = old_parent {
                let siblings = &self.doc.nodes[old_parent].children;
                self.state.work(siblings.len())?;
                if let Some(position) = siblings.iter().position(|&node| node == source) {
                    let before = position.checked_sub(1).and_then(|at| siblings.get(at)).copied().map(node_handle);
                    let after = siblings.get(position + 1).copied().map(node_handle);
                    records.push(removal(old_parent, vec![*handle], before, after));
                }
            }
            if method == "removeChild" { return Ok(records); }
            vec![*handle]
        };
        if reference.is_none() && added.is_empty() { return Ok(records); }

        let siblings = &self.doc.nodes[parent].children;
        self.state.work(siblings.len())?;
        let position = reference.and_then(|node| siblings.iter().position(|&child| child == node))
            .unwrap_or(siblings.len());
        let previous = siblings[..position].iter().rev().copied().find(|&node| node != source).map(node_handle);
        let removed = if method == "replaceChild" { reference.map(node_handle).into_iter().collect() }
            else { Vec::new() };
        let next_at = position + usize::from(method == "replaceChild");
        let next = siblings.get(next_at..).and_then(|nodes| nodes.iter().copied().find(|&node| node != source))
            .map(node_handle);
        if !added.is_empty() || !removed.is_empty() {
            records.push(NativeChildListChange { target: receiver, added, removed, previous, next });
        }
        Ok(records)
    }
}
