//! Bounded selector snapshots and live name-filtered NodeLists. Querying never
//! constructs markup, runs scripts, changes resources or leaves this document.
//! Semantics: https://dom.spec.whatwg.org/#dom-parentnode-queryselectorall
use super::*;

pub(super) enum NodeList {
    Snapshot(Vec<usize>),
    HandleSnapshot(Vec<u32>),
    Name(String),
    // A live SameObject list over this real node's immediate children.
    Children(usize),
    InertChildren(u32),
}

pub(super) fn node_list_index(handle: u32) -> Option<usize> {
    (NODE_LIST_BASE..NODE_LIST_BASE + NODE_LIST_HANDLES as u32)
        .contains(&handle)
        .then(|| (handle - NODE_LIST_BASE) as usize)
}

impl Bridge<'_> {
    pub(super) fn node_list_iteration_member(&self,handle:u32,property:&str)->bool {
        // Iteration members and length use normal ordinary prototype lookup.
        // Existing item/forEach and protected native indices retain their
        // guarded routes; this does not expose other native collections.
        self.node_list_receiver(handle) && matches!(property,"constructor"|"length"|"keys"|"values"|"entries")
    }

    pub(super) fn node_list_receiver(&self, handle: u32) -> bool {
        node_list_index(handle).is_some_and(|index| index < self.state.node_lists.len())
    }

    // Reserve unpublished registry backing alongside the operation's owned
    // payload. No identity, capacity or byte charge changes until commit.
    fn prepare_node_list(&mut self, operation:&str, payload:usize)->Result<(u32,Option<Vec<NodeList>>,usize),String> {
        let length=self.state.node_lists.len();
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&length>=self.state.limits.host.node_lists {
            return Err(format!("DOM NodeList {operation} limit reached ({} per document).",self.state.limits.host.node_lists));
        }
        #[cfg(not(test))]
        let _=operation;
        if length>=NODE_LIST_HANDLES{return Err("DOM NodeList identity space exhausted.".into());}
        let handle=NODE_LIST_BASE.checked_add(u32::try_from(length).map_err(|_|"DOM NodeList identity overflow.")?).ok_or("DOM NodeList identity overflow.")?;
        let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(payload).ok_or("DOM retained NodeList payload allowance exhausted.")?;
        let (replacement,added)=retained_storage::grow(&self.state.node_lists,1,available)?;
        if replacement.is_some(){self.state.copies(length*size_of::<NodeList>())?;}
        Ok((handle,replacement,added))
    }
    fn commit_node_list(&mut self,list:NodeList,replacement:Option<Vec<NodeList>>,added:usize,payload:usize) {
        if let Some(mut next)=replacement{next.append(&mut self.state.node_lists);self.state.node_lists=next;}
        self.state.node_lists.push(list);self.state.retained+=added+payload;
    }

    pub(super) fn snapshot_mutation_node_list(&mut self, nodes: &[u32]) -> Result<u32, String> {
        self.state.work(nodes.len().saturating_add(1))?;
        let mut handles = Vec::new();
        handles.try_reserve_exact(nodes.len()).map_err(|_| "Cannot reserve MutationRecord node identities.")?;
        let payload=handles.capacity()*size_of::<u32>();
        self.state.copies(nodes.len()*size_of::<u32>())?;
        let (handle,replacement,added)=self.prepare_node_list("mutation snapshot",payload)?;
        handles.extend_from_slice(nodes);
        self.commit_node_list(NodeList::HandleSnapshot(handles),replacement,added,payload);
        Ok(handle)
    }

    pub(super) fn snapshot_query_handles(&mut self,nodes:Vec<u32>)->Result<Value,String>{
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&nodes.len()>4096{return Err("DOM selector result limit reached (4096 elements).".into());}
        let references=self.state.node_list_references.checked_add(nodes.len()).ok_or("DOM NodeList retained-reference accounting overflow.")?;
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&references>self.state.limits.host.node_list_references{return Err("DOM NodeList retained-reference limit reached.".into());}
        self.state.work(nodes.len())?;let payload=nodes.capacity()*size_of::<u32>();let (handle,replacement,added)=self.prepare_node_list("inert query snapshot",payload)?;
        self.commit_node_list(NodeList::HandleSnapshot(nodes),replacement,added,payload);self.state.node_list_references=references;Ok(Value::Host(handle))
    }

    pub(super) fn create_child_list(&mut self, object: u32) -> Result<Value, String> {
        let root = if object == DOCUMENT { 0 } else { self.dom_node(object)? };
        self.check_iframe_child_topology(root)?;
        self.state.work(1)?;
        self.state.work(self.state.dom.child_lists.lookup_work())?;
        if let Some(&handle) = self.state.dom.child_lists.get(&root) { return Ok(Value::Host(handle)); }
        let record=if self.state.node_lists.len()==self.state.node_lists.capacity(){size_of::<NodeList>()}else{0};
        let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(record).ok_or("DOM retained NodeList record allowance exhausted.")?;
        let (map,payload)=self.state.dom.child_lists.prepare(1,available)?;
        let work=self.state.dom.child_lists.insert_work(&root);
        self.state.work(work)?;
        let copied=work.saturating_add(if map.is_some(){self.state.dom.child_lists.len()}else{0});
        self.state.copies(self.state.dom.child_lists.movement_bytes(copied))?;
        let (handle,replacement,added)=self.prepare_node_list("identity",payload)?;
        let list = if self.template_receiver(object) {
            // Parsed template members are exposed only through content.
            NodeList::Snapshot(Vec::new())
        } else {
            NodeList::Children(root)
        };
        self.commit_node_list(list,replacement,added,payload);
        self.state.dom.child_lists.commit(map);
        self.state.dom.child_lists.insert(root, handle);
        Ok(Value::Host(handle))
    }

    pub(super) fn create_inert_child_list(&mut self, object: u32) -> Result<Value, String> {
        if self.inert_children(object).is_none() { return Err("Invalid inert childNodes owner.".into()); }
        if let Some(index)=node_index(object){self.state.work(self.state.dom.child_lists.lookup_work())?;if let Some(&saved)=self.state.dom.child_lists.get(&index){return Ok(Value::Host(saved));}}
        self.state.work(1)?;
        self.state.work(self.state.dom.inert_child_lists.lookup_work())?;
        if let Some(&handle) = self.state.dom.inert_child_lists.get(&object) {
            return Ok(Value::Host(handle));
        }
        let record=if self.state.node_lists.len()==self.state.node_lists.capacity(){size_of::<NodeList>()}else{0};
        let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(record).ok_or("DOM retained NodeList record allowance exhausted.")?;
        let (map,payload)=self.state.dom.inert_child_lists.prepare(1,available)?;
        let work=self.state.dom.inert_child_lists.insert_work(&object);
        self.state.work(work)?;
        let copied=work.saturating_add(if map.is_some(){self.state.dom.inert_child_lists.len()}else{0});
        self.state.copies(self.state.dom.inert_child_lists.movement_bytes(copied))?;
        let (handle,replacement,added)=self.prepare_node_list("identity",payload)?;
        self.commit_node_list(NodeList::InertChildren(object),replacement,added,payload);
        self.state.dom.inert_child_lists.commit(map);
        self.state.dom.inert_child_lists.insert(object, handle);
        Ok(Value::Host(handle))
    }

    pub(super) fn create_name_list(&mut self, value: &Value) -> Result<Value,String> {
        let name = self.text(value)?;
        let payload=name.capacity();
        let (handle,replacement,added)=self.prepare_node_list("identity",payload)?;
        self.commit_node_list(NodeList::Name(name),replacement,added,payload);
        Ok(Value::Host(handle))
    }

    fn name_list_nodes(&mut self,index:usize)->Result<Vec<usize>,String> {
        let mut nodes=self.query_candidates(0)?;let mut kept=0;
        for position in 0..nodes.len() {
            let node=nodes[position];self.state.work(1)?;
            if self.doc.nodes[node].namespace!=crate::html::Namespace::Html{continue;}
            let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{unreachable!()};
            self.state.work(attrs.len())?;self.state.copies(attrs.iter().map(|attr|attr.name.bytes()).sum())?;
            let value=self.doc.attr(node,"name");self.state.copies(value.map_or(0,str::len))?;
            let NodeList::Name(name)=&self.state.node_lists[index] else{unreachable!()};
            if value==Some(name.as_str()) {
                if kept!=position{self.state.copies(size_of::<usize>())?;nodes[kept]=node;}
                kept+=1;
            }
        }
        nodes.truncate(kept);Ok(nodes)
    }

    pub(super) fn node_list_get(&mut self, handle: u32, property: &str) -> Result<Value, String> {
        let index = node_list_index(handle).ok_or("Invalid NodeList handle.")?;
        let list = self
            .state
            .node_lists
            .get(index)
            .ok_or("Invalid NodeList handle.")?;
        if let NodeList::Children(root) = list {
            let root = *root;
            if property != "length" && collection_index(property).is_none() { return Ok(Value::Undefined); }
            if self.inert_children(node_handle(root)).is_some(){self.state.work(1)?;let children=self.inert_children(node_handle(root)).unwrap();return Ok(if property=="length"{Value::Number(children.len() as f64)}else{collection_index(property).and_then(|at|children.get(at)).copied().map_or(Value::Undefined,Value::Host)});}
            // A replacement may have appended a new Text since this SameObject
            // list was obtained. Publish current topology/owner metadata before
            // returning its handle, including before cross-realm native access.
            self.state.rebuild(self.doc)?;
            self.state.work(1)?;
            self.check_iframe_child_topology(root)?;
            let children = &self.doc.nodes.get(root).ok_or("Invalid childNodes owner.")?.children;
            if property == "length" { return Ok(Value::Number(children.len() as f64)); }
            return Ok(collection_index(property).and_then(|index| children.get(index))
                .map_or(Value::Undefined, |&node| Value::Host(node_handle(node))));
        }
        if let NodeList::InertChildren(object) = list {
            let object = *object;
            self.state.work(1)?;
            let children = self.inert_children(object).ok_or("Invalid inert childNodes owner.")?;
            return if property == "length" { Ok(Value::Number(children.len() as f64)) }
                else { Ok(collection_index(property).and_then(|index| children.get(index)).copied()
                    .map_or(Value::Undefined, Value::Host)) };
        }
        if matches!(list,NodeList::Name(_)) {
            if property != "length" && collection_index(property).is_none() { return Ok(Value::Undefined); }
            let nodes = self.name_list_nodes(index)?;
            return if property == "length" { Ok(Value::Number(nodes.len() as f64)) }
                else { Ok(collection_index(property).and_then(|index|nodes.get(index)).map_or(Value::Undefined,|&node|Value::Host(node_handle(node)))) };
        }
        if let NodeList::HandleSnapshot(nodes) = list {
            return if property == "length" { Ok(Value::Number(nodes.len() as f64)) }
                else { Ok(collection_index(property).and_then(|index| nodes.get(index)).copied()
                    .map_or(Value::Undefined, Value::Host)) };
        }
        let NodeList::Snapshot(list) = list else { unreachable!() };
        if property == "length" {
            return Ok(Value::Number(list.len() as f64));
        }
        Ok(collection_index(property)
            .and_then(|index| list.get(index))
            .map_or(Value::Undefined, |&node| Value::Host(node_handle(node))))
    }

    pub(super) fn node_list_keys(&mut self, handle: u32) -> Result<Vec<String>, String> {
        let Value::Number(length) = self.node_list_get(handle,"length")? else { unreachable!() };
        let length = length as usize;
        self.state.index_keys(length)
    }

    fn query_push(&mut self,values:&mut Vec<usize>,node:usize,available:usize)->Result<(),String> {
        let used=values.capacity()*size_of::<usize>();
        let (replacement,_)=retained_storage::grow(values,1,available.saturating_sub(used))?;
        if let Some(mut next)=replacement {
            self.state.copies(values.len()*size_of::<usize>())?;
            next.append(values);*values=next;
        }
        values.push(node);Ok(())
    }

    pub(super) fn query_candidates(&mut self,root:usize)->Result<Vec<usize>,String> {
        let available=self.state.retained_limit.saturating_sub(self.state.retained);
        self.query_candidates_with_storage(root,available)
    }
    fn query_candidates_with_storage(&mut self,root:usize,available:usize)->Result<Vec<usize>,String> {
        self.state.rebuild(self.doc)?;
        let mut candidates=Vec::new();
        let mut current=self.doc.nodes[root].children.first().copied();
        while let Some(node)=current {
            self.state.work(1)?;
            let name=tag(self.doc,node);
            let inert=dom_tree::inert_dom_tag(name,self.doc.scripting_enabled);
            let visible=(!inert||dom_tree::exposes_inert_host(name,self.doc.nodes[node].namespace))
                &&(self.state.tree.active.get(node).copied().unwrap_or(false)||self.state.detached_nodes.contains(&node));
            if visible&&matches!(self.doc.nodes[node].kind,NodeKind::Element{..}){self.query_push(&mut candidates,node,available)?;}
            if visible&&!inert {
                if let Some(&child)=self.doc.nodes[node].children.first(){current=Some(child);continue;}
            }
            // Checked parent and sibling slots let DFS climb without a second
            // traversal stack, including detached ordinary subtrees.
            let mut cursor=node;
            current=loop {
                self.state.work(1)?;
                if cursor==root{break None;}
                let Some(parent)=self.state.tree.parents.get(cursor).copied().flatten() else{break None;};
                let slot=self.state.tree.sibling_slots[cursor];
                if let Some(&next)=slot.checked_add(1).and_then(|slot|self.doc.nodes[parent].children.get(slot)){break Some(next);}
                cursor=parent;
            };
        }
        Ok(candidates)
    }

    // Optional retained-result compaction: sparse snapshots should not keep a
    // document-wide candidate allocation when scratch/copy work permits a
    // smaller replacement. Refusal to optimize keeps the already admitted Vec.
    fn compact_query_snapshot(&mut self,mut nodes:Vec<usize>,available:usize)->Result<Vec<usize>,String> {
        if nodes.len()>=nodes.capacity()/2{return Ok(nodes);}
        let bytes=nodes.len()*size_of::<usize>();
        let old=nodes.capacity()*size_of::<usize>();
        let spare=available.saturating_sub(old);
        if bytes>spare||bytes>self.state.copy_work||nodes.len()>self.state.host_work{return Ok(nodes);}
        let mut compact=Vec::new();
        if compact.try_reserve_exact(nodes.len()).is_err()||compact.capacity()*size_of::<usize>()>spare{return Ok(nodes);}
        // Prepay both counters before copying. Deadline refusals unwind both
        // staged buffers; no author callback can change the allowance here.
        self.state.work(nodes.len())?;self.state.copies(bytes)?;
        compact.append(&mut nodes);Ok(compact)
    }

    pub(super) fn selector_query(
        &mut self,
        object: u32,
        method: &str,
        source: &str,
    ) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        let root = if object == DOCUMENT {
            0
        } else if self.fragment_receiver(object) {
            let root=self.dom_node(object)?;self.check_iframe_child_topology(root)?;root
        } else {
            self.node(object)?
        };
        let record=if method=="querySelectorAll"&&self.state.node_lists.len()==self.state.node_lists.capacity(){size_of::<NodeList>()}else{0};
        let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(record).ok_or("DOM retained NodeList record allowance exhausted.")?;
        let candidates = match method {
            "matches" => {let mut candidates=Vec::new();self.query_push(&mut candidates,root,available)?;candidates},
            "closest" => {
                let mut candidates = Vec::new();
                let mut current = Some(root);
                while let Some(node) = current.filter(|&node| node != 0) {
                    self.state.work(1)?;
                    self.query_push(&mut candidates,node,available)?;
                    current = self.state.tree.parents[node];
                }
                candidates
            }
            _ => self.query_candidates_with_storage(root,available)?,
        };
        let checked = |node| {
            self.forms
                .controls
                .get(&node)
                .filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
                .map(|control| control.checked)
        };
        let nodes=crate::css::query_with_topology(self.doc,source,candidates,Some(&checked),&mut self.state.host_work,&self.state.tree.parents,&self.state.tree.sibling_slots,method!="querySelectorAll",&mut self.state.copy_work)?;
        if method=="querySelectorAll" {
            let nodes=self.compact_query_snapshot(nodes,available)?;
            #[cfg(test)]
            if self.state.limits.test_entry_arenas&&nodes.len()>4096{return Err("DOM selector result limit reached (4096 elements).".into());}
            let references = self.state.node_list_references.checked_add(nodes.len())
                .ok_or("DOM NodeList retained-reference accounting overflow.")?;
            #[cfg(test)]
            if self.state.limits.test_entry_arenas&&references > self.state.limits.host.node_list_references {
                return Err(format!("DOM NodeList retained-reference limit reached ({} per document).", self.state.limits.host.node_list_references));
            }
            self.state.work(nodes.len())?;
            self.state.copies(nodes.len()*size_of::<usize>())?;
            let payload=nodes.capacity()*size_of::<usize>();
            let (handle,replacement,added)=self.prepare_node_list("snapshot",payload)?;
            self.commit_node_list(NodeList::Snapshot(nodes),replacement,added,payload);
            self.state.node_list_references = references;
            Ok(Value::Host(handle))
        } else {
            let found=nodes.first().copied();
            if method == "matches" {
                Ok(Value::Bool(found.is_some()))
            } else {
                Ok(found.map_or(Value::Null, |node| Value::Host(node_handle(node))))
            }
        }
    }
}

#[cfg(test)]
#[path = "script_node_list_storage_tests.rs"]
mod storage_tests;

#[cfg(test)]
#[path = "script_query_traversal_tests.rs"]
mod traversal_tests;
