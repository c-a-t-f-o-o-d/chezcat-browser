//! Joint unpublished clone identity projection and backing admission.
use super::*;

impl Bridge<'_> {
    pub(super) fn publish_inert_clone(
        &mut self, first: u32, mut staged: Vec<Entry>, generated: Option<GeneratedNode>,
        mut payload: usize, mut links: Vec<(u32,u32)>, reserve_slots: bool,
    ) -> Result<Value,String> {
        let base = self.doc.nodes.len();
        let source_base = self.state.inert_documents.generated.len();
        let mut sources = Vec::new();
        sources.try_reserve_exact(staged.len().checked_add(1).ok_or("Clone source count overflow.")?)
            .map_err(|_|"Cannot reserve clone source plan.")?;
        if let Some(source) = generated { sources.push(source); }
        let mut handles = Vec::new();
        handles.try_reserve_exact(staged.len()).map_err(|_|"Cannot reserve clone identity projection.")?;
        let slots = self.state.node_limit.saturating_sub(base.saturating_add(self.state.blank_frame_nodes))
            .min(((STYLE_BASE-NODE_BASE) as usize).saturating_sub(base));
        let mut parked = 0usize;
        for (offset, entry) in staged.iter().enumerate() {
            self.state.work(1)?;
            let Entry::Node(node) = entry else { unreachable!() };
            let safe = match &node.kind {
                Kind::Parsed { source, .. } => {
                    let raw = match *source {
                        ParsedSource::Generated(index) if index >= source_base => sources.get(index-source_base).map(|source| &*source.node),
                        source => self.parsed_source(source).map(|(node,_)|node),
                    };
                    raw.is_some_and(template_node_can_park)
                },
                Kind::Text(_) => true,
                Kind::Element(tag) => tag=="style" || script_clone_node::ordinary_clone_element(tag),
                _ => false,
            };
            let handle = if reserve_slots && safe && parked < slots {
                let handle = node_handle(base+parked); parked+=1; handle
            } else { first.checked_add(u32::try_from(offset).map_err(|_|"Clone identity count overflow.")?).ok_or("Clone identity overflow.")? };
            handles.push(handle);
        }
        let project = |handle:u32| -> Result<u32,String> {
            handle.checked_sub(first).and_then(|offset|handles.get(offset as usize)).copied()
                .ok_or("Clone edge escaped the unpublished tree.".into())
        };
        let mut parked_nodes = Vec::new();
        parked_nodes.try_reserve_exact(parked).map_err(|_|"Cannot reserve cloned native owner plan.")?;
        for (offset, entry) in staged.iter_mut().enumerate() {
            let Entry::Node(node) = entry else { unreachable!() };
            self.state.work(node.children.len().saturating_add(1))?;
            self.state.copies(node.children.len().checked_mul(size_of::<u32>()).ok_or("Clone edge projection overflow.")?)?;
            node.parent = node.parent.map(&project).transpose()?;
            for child in &mut node.children { *child=project(*child)?; }
            if let Kind::Fragment { template } = &mut node.kind {
                *template=template.map(&project).transpose()?;
            }
            if handles[offset] >= FIRST_ENTRY { continue; }
            if let Kind::Parsed {source:ParsedSource::Loaded(index),initial_template}=node.kind {
                // A loaded source can only own its original native slot.
                // A cloned slot needs an independent immutable generated
                // snapshot, including nonce provenance and owned capacity.
                let raw=&self.doc.nodes[index];
                let mut cost=copied_node_bytes(raw);
                if let NodeKind::Element {tag,href,attrs,..}=&raw.kind {
                    self.state.work(attrs.len())?;
                    cost=cost.checked_add(tag.len()).and_then(|n|n.checked_add(href.as_ref().map_or(0,String::len))).ok_or("Loaded clone source copy overflow.")?;
                }
                self.state.copies(cost)?;
                let raw=&self.doc.nodes[index];
                let snapshot=Node {kind:script_clone_node::copy_node_kind(&raw.kind)?,namespace:raw.namespace,children:Vec::new()};
                let mut bytes=node_bytes(&snapshot).checked_add(size_of::<Node>()+2*size_of::<usize>()).ok_or("Loaded clone source storage overflow.")?;
                if let NodeKind::Element {tag,href,..}=&snapshot.kind {
                    bytes=bytes.checked_add(tag.capacity()).and_then(|n|n.checked_add(href.as_ref().map_or(0,String::capacity))).ok_or("Loaded clone element storage overflow.")?;
                }
                payload=payload.checked_add(bytes).ok_or("Loaded clone payload overflow.")?;
                node.kind=Kind::Parsed {source:ParsedSource::Generated(source_base+sources.len()),initial_template};
                sources.push(GeneratedNode {node:Rc::new(snapshot),unsafe_nonce:self.doc.nonce_unsafe_nodes.contains(&index)});
            }
            if !matches!(node.kind,Kind::Parsed { .. }) {
                let kind = match std::mem::replace(&mut node.kind,Kind::Doctype) {
                    Kind::Text(text)=>NodeKind::Text(text),
                    Kind::Element(tag)=>NodeKind::Element { tag,id:None,href:None,attrs:Vec::new() },
                    _=>unreachable!(),
                };
                let namespace = if matches!(kind,NodeKind::Element { .. }) {crate::html::Namespace::Html} else {crate::html::Namespace::None};
                node.kind=Kind::Parsed {source:ParsedSource::Generated(source_base+sources.len()),initial_template:None};
                sources.push(GeneratedNode {node:Rc::new(Node {kind,namespace,children:Vec::new()}),unsafe_nonce:false});
                payload=payload.checked_add(size_of::<Node>()+2*size_of::<usize>()).ok_or("Clone generated source storage overflow.")?;
            }
            let Entry::Node(node)=std::mem::replace(entry,Entry::ParkedNode) else {unreachable!()};
            parked_nodes.push((handles[offset],node));
        }
        for (host,fragment) in &mut links { *host=project(*host)?; *fragment=project(*fragment)?; }
        let arena=&self.state.inert_documents;
        let entry_min=arena.entries.len().checked_add(staged.len()).and_then(|n|n.saturating_sub(arena.entries.capacity()).checked_mul(size_of::<Entry>())).ok_or("Clone entry backing overflow.")?;
        let source_min=arena.generated.len().checked_add(sources.len()).and_then(|n|n.saturating_sub(arena.generated.capacity()).checked_mul(size_of::<GeneratedNode>())).ok_or("Clone source backing overflow.")?;
        let content_min=arena.template_contents.minimum_growth(links.len())?;
        let available=self.state.retained_limit.checked_sub(self.state.retained)
            .and_then(|n|n.checked_sub(payload)).and_then(|n|n.checked_sub(entry_min))
            .and_then(|n|n.checked_sub(source_min)).and_then(|n|n.checked_sub(content_min))
            .ok_or("Clone joint backing allowance exhausted.")?;
        let node_min=self.main_node_minimum(parked)?;
        let (index_growth,index_bytes)=self.prepare_adopted_index(parked_nodes.iter().map(|entry|entry.0),
            available.checked_sub(node_min).ok_or("Clone native backing allowance exhausted.")?)?;
        let (node_growth,node_bytes)=self.prepare_main_nodes(parked,
            available.checked_sub(index_bytes).ok_or("Clone owner index allowance exhausted.")?)?;
        let metadata=index_bytes.checked_add(node_bytes).ok_or("Clone joint metadata overflow.")?;
        let (committed,indexes)=self.prepare_template_indexes(staged.len(),payload,sources.len(),None,&links,metadata)?;
        debug_assert_eq!(committed,first);
        self.commit_main_nodes(node_growth);
        let arena=&mut self.state.inert_documents;
        arena.generated.extend(sources);
        arena.entries.extend(staged);
        arena.adopted.commit(index_growth);
        for (key,node) in parked_nodes {arena.adopted.insert(key,node);}
        arena.template_documents.commit(indexes.documents);
        arena.template_contents.commit(indexes.contents);
        for (key,value) in links {arena.template_contents.insert(key,value);}
        for _ in 0..parked {
            self.doc.nodes.push(Node {namespace:crate::html::Namespace::None,kind:NodeKind::DocumentFragment,children:Vec::new()});
            self.state.tree.parents.push(None);self.state.tree.active.push(false);self.state.dom.connected.push(false);
        }
        Ok(Value::Host(handles[0]))
    }
}
