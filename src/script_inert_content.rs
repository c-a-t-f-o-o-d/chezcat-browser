//! Shared replacement identity projection and atomic inert text replacement.
use super::*;

impl Bridge<'_> {
    pub(in crate::scripting) fn inert_text_replacement_receiver(&self, object: u32) -> bool {
        if self.inert_node_type(object) == Some(11) {
            return true;
        }
        !self.template_receiver(object)
            && match self.inert_import_source(object) {
                Some(ImportSource::Element(_)) => true,
                Some(ImportSource::Parsed { node, .. }) => {
                    node.namespace == crate::html::Namespace::Html
                        && matches!(node.kind, NodeKind::Element { .. })
                }
                _ => false,
            }
    }
    pub(in crate::scripting) fn existing_template_content(
        &mut self,
        object: u32,
    ) -> Result<Option<u32>, String> {
        self.state
            .work(self.state.inert_documents.template_contents.lookup_work())?;
        Ok(self
            .state
            .inert_documents
            .template_contents
            .get(&object)
            .copied())
    }
    pub(in crate::scripting) fn template_replacement_handles(
        &mut self,
        fragment: u32,
        parsed: &Document,
    ) -> Result<(Vec<u32>, usize), String> {
        let count = parsed
            .nodes
            .len()
            .checked_sub(1)
            .ok_or("Missing checked template root.")?;
        let first = FIRST_ENTRY
            .checked_add(
                u32::try_from(self.state.inert_documents.entries.len())
                    .map_err(|_| "Inert identity overflow.")?,
            )
            .ok_or("Inert identity overflow.")?;
        first
            .checked_add(u32::try_from(count).map_err(|_| "Inert replacement count overflow.")?)
            .filter(|&end| end <= ENTRY_END)
            .ok_or("Inert replacement identity space exhausted.")?;
        let base = self.doc.nodes.len();
        let can_park = count
            <= self
                .state
                .node_limit
                .saturating_sub(base.saturating_add(self.state.blank_frame_nodes))
                .min(((STYLE_BASE - NODE_BASE) as usize).saturating_sub(base));
        let work = parsed.nodes.iter().fold(parsed.nodes.len(), |n, node| {
            n.saturating_add(copied_node_bytes(node))
        });
        self.state.work(work)?;
        self.state.copies(
            parsed
                .nodes
                .len()
                .checked_mul(size_of::<u32>())
                .ok_or("Template identity projection copy overflow.")?,
        )?;
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(parsed.nodes.len())
            .map_err(|_| "Template identity projection reservation failed.")?;
        handles.push(fragment);
        let mut parked = 0;
        for (offset, node) in parsed.nodes.iter().skip(1).enumerate() {
            let handle = if can_park && template_node_can_park(node) {
                let handle = node_handle(base + parked);
                parked += 1;
                handle
            } else {
                first + offset as u32
            };
            handles.push(handle);
        }
        Ok((handles, parked))
    }
    pub(in crate::scripting) fn next_inert_text_identity(&self) -> Result<u32, String> {
        if self.can_reserve_inert_leaf() {
            return Ok(node_handle(self.doc.nodes.len()));
        }
        FIRST_ENTRY
            .checked_add(
                u32::try_from(self.state.inert_documents.entries.len())
                    .map_err(|_| "Inert identity overflow.")?,
            )
            .filter(|&id| id < ENTRY_END)
            .ok_or_else(|| "Inert text identity space exhausted.".into())
    }
    pub(super) fn can_reserve_inert_leaf(&self) -> bool {
        self.doc.nodes.len() < (STYLE_BASE - NODE_BASE) as usize
            && self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes)
                < self.state.node_limit
    }

    // Assign the eventual native identity at creation. The inert route keeps
    // ownership until promotion; no saved handle or observer record is remapped.
    pub(super) fn append_inert_text(
        &mut self, owner: u32, parent: Option<u32>, text: DomString, metadata: usize,
    ) -> Result<u32, String> {
        if !self.can_reserve_inert_leaf() {
            let handle = self.reserve_inert_storage(1, text.capacity(), 0, 0, metadata)?;
            self.state.inert_documents.entries.push(Entry::Node(InertNode {
                kind: Kind::Text(text), owner, parent, children: Vec::new(),
            }));
            return Ok(handle);
        }
        self.append_inert_source_leaf(owner, parent,
            Node { kind: NodeKind::Text(text), namespace: crate::html::Namespace::None, children: Vec::new() }, metadata)
    }

    pub(super) fn append_inert_source_leaf(
        &mut self, owner: u32, parent: Option<u32>, node: Node, metadata: usize,
    ) -> Result<u32, String> {
        let mut payload = node_bytes(&node).checked_add(size_of::<Node>() + 2 * size_of::<usize>())
            .ok_or("Inert leaf source storage overflow.")?;
        if let NodeKind::Element { tag, href, .. } = &node.kind {
            payload = payload.checked_add(tag.capacity()).and_then(|bytes| bytes.checked_add(href.as_ref().map_or(0,String::capacity)))
                .ok_or("Inert element source storage overflow.")?;
        }
        let source = self.state.inert_documents.generated.len();
        let parked = self.can_reserve_inert_leaf() && template_node_can_park(&node);
        let node = Rc::new(node);
        if !parked {
            let handle = self.reserve_inert_storage(1, payload, 1, 0, metadata)?;
            let arena = &mut self.state.inert_documents;
            arena.generated.push(GeneratedNode { node, unsafe_nonce: false });
            arena.entries.push(Entry::Node(InertNode {
                kind: Kind::Parsed { source: ParsedSource::Generated(source), initial_template: None },
                owner, parent, children: Vec::new(),
            }));
            return Ok(handle);
        }
        let handle = node_handle(self.doc.nodes.len());
        let arena = &self.state.inert_documents;
        let minimum = arena.entries.len().checked_add(1)
            .and_then(|n| n.saturating_sub(arena.entries.capacity()).checked_mul(size_of::<Entry>()))
            .and_then(|bytes| arena.generated.len().checked_add(1)
                .and_then(|n| n.saturating_sub(arena.generated.capacity()).checked_mul(size_of::<GeneratedNode>()))
                .and_then(|sources| bytes.checked_add(sources)))
            .ok_or("Inert leaf registry storage overflow.")?;
        let available = self.state.retained_limit.checked_sub(self.state.retained)
            .and_then(|bytes| bytes.checked_sub(payload))
            .and_then(|bytes| bytes.checked_sub(metadata))
            .and_then(|bytes| bytes.checked_sub(minimum))
            .ok_or("Inert leaf joint backing allowance exhausted.")?;
        let native_minimum = self.main_node_minimum(1)?;
        let (index_growth, index_bytes) = self.prepare_adopted_index(std::iter::once(handle),
            available.checked_sub(native_minimum).ok_or("Inert leaf native backing allowance exhausted.")?)?;
        let (node_growth, node_bytes) = self.prepare_main_nodes(1,
            available.checked_sub(index_bytes).ok_or("Inert leaf index allowance exhausted.")?)?;
        let metadata = metadata.checked_add(index_bytes).and_then(|bytes| bytes.checked_add(node_bytes))
            .ok_or("Inert leaf joint storage overflow.")?;
        self.reserve_inert_storage(1, payload, 1, 0, metadata)?;
        self.commit_main_nodes(node_growth);
        let arena = &mut self.state.inert_documents;
        arena.generated.push(GeneratedNode { node, unsafe_nonce: false });
        arena.entries.push(Entry::ParkedNode);
        arena.adopted.commit(index_growth);
        arena.adopted.insert(handle, InertNode {
            kind: Kind::Parsed { source: ParsedSource::Generated(source), initial_template: None },
            owner, parent, children: Vec::new(),
        });
        self.doc.nodes.push(Node { kind: NodeKind::DocumentFragment, namespace: crate::html::Namespace::None, children: Vec::new() });
        self.state.tree.parents.push(None);
        self.state.tree.active.push(false);
        self.state.dom.connected.push(false);
        Ok(handle)
    }
    pub(in crate::scripting) fn replace_inert_text_content(
        &mut self,
        object: u32,
        text: DomString,
    ) -> Result<(), String> {
        if !self.inert_text_replacement_receiver(object) {
            return Err("Inert text replacement requires a supported Element or fragment.".into());
        }
        let root = self
            .state
            .inert_documents
            .node(object)
            .ok_or("Inert text receiver is unavailable.")?;
        let owner = root.owner;
        let old_count = root.children.len();
        let count = usize::from(!text.is_empty());
        self.state.work(old_count.saturating_add(count))?;
        for slot in 0..old_count {
            let child = self.inert_children(object).unwrap()[slot];
            if self
                .state
                .inert_documents
                .node(child)
                .is_none_or(|node| node.parent != Some(object))
            {
                return Err("Inert text child ownership is inconsistent.".into());
            }
        }
        let available = self
            .state
            .retained_limit
            .saturating_sub(self.state.retained);
        let (growth, child_bytes) = retained_storage::capacity(
            &self.state.inert_documents.node(object).unwrap().children,
            count,
            available,
        )?;
        self.state.copies(
            old_count
                .checked_add(count)
                .and_then(|n| n.checked_mul(size_of::<u32>()))
                .ok_or("Inert text child copy overflow.")?,
        )?;
        self.state.prepare_inert_range_changes(2)?;
        let first = if count == 0 {
            self.reserve_inert_storage(0, 0, 0, 0, child_bytes)?;
            None
        } else {
            Some(self.append_inert_text(owner, Some(object), text, child_bytes)?)
        };
        self.state.ranges_remove_inert(object,0,old_count);
        self.state.ranges_insert_handle(object,0,count);
        let arena = &mut self.state.inert_documents;
        for slot in 0..old_count {
            let child = arena.node(object).unwrap().children[slot];
            arena.node_mut(child).unwrap().parent = None;
        }
        let root = arena.node_mut(object).unwrap();
        if let Some(growth) = growth {
            root.children = growth;
        }
        root.children.clear();
        if let Some(first) = first {
            root.children.push(first);
        }
        self.state.dom_committed();
        Ok(())
    }
}
