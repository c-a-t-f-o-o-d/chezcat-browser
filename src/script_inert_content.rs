//! Shared replacement identity projection and atomic inert text replacement.
use super::*;

impl Bridge<'_> {
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
        FIRST_ENTRY
            .checked_add(
                u32::try_from(self.state.inert_documents.entries.len())
                    .map_err(|_| "Inert identity overflow.")?,
            )
            .filter(|&id| id < ENTRY_END)
            .ok_or_else(|| "Inert text identity space exhausted.".into())
    }
    pub(in crate::scripting) fn replace_inert_text_content(
        &mut self,
        object: u32,
        text: String,
    ) -> Result<(), String> {
        if !matches!(self.inert_node_type(object), Some(1 | 11)) || self.template_receiver(object) {
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
        let payload = if count == 0 { 0 } else { text.capacity() };
        let first = self.reserve_inert_storage(count, payload, 0, 0, child_bytes)?;
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
        if count != 0 {
            root.children.push(first);
            arena.entries.push(Entry::Node(InertNode {
                kind: Kind::Text(text),
                owner,
                parent: Some(object),
                children: Vec::new(),
            }));
        }
        self.state.dom_committed();
        Ok(())
    }
}
