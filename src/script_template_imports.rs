//! Appropriate-owner template content for real main-document template copies.
use super::*;
pub(in crate::scripting) struct TemplateImportCheckpoint {
    entries: usize,
    generated: usize,
    backing: usize,
    retained: usize,
}
impl Bridge<'_> {
    pub(in crate::scripting) fn template_import_checkpoint(&self) -> TemplateImportCheckpoint {
        let arena = &self.state.inert_documents;
        TemplateImportCheckpoint {
            entries: arena.entries.len(),
            generated: arena.generated.len(),
            backing: arena.backing_bytes(),
            retained: self.state.retained,
        }
    }
    pub(in crate::scripting) fn rollback_template_import(
        &mut self,
        checkpoint: TemplateImportCheckpoint,
    ) {
        let first = FIRST_ENTRY + checkpoint.entries as u32;
        let arena = &mut self.state.inert_documents;
        arena.entries.truncate(checkpoint.entries);
        arena.generated.truncate(checkpoint.generated);
        arena
            .template_contents
            .retain_entries(|_, &value| value < first);
        arena
            .template_documents
            .retain_entries(|_, &value| value < first);
        // Payload disappears; fallibly acquired reusable backing remains owned.
        self.state.retained = checkpoint.retained + (arena.backing_bytes() - checkpoint.backing);
    }
    fn main_template_owner(&mut self) -> Result<u32, String> {
        self.state
            .work(self.state.inert_documents.template_documents.lookup_work())?;
        if let Some(&owner) = self.state.inert_documents.template_documents.get(&DOCUMENT) {
            return Ok(owner);
        }
        let first = FIRST_ENTRY + self.state.inert_documents.entries.len() as u32;
        let (committed, indexes) =
            self.prepare_template_indexes(2, 0, 0, Some((DOCUMENT, first)), &[])?;
        debug_assert_eq!(first, committed);
        let arena = &mut self.state.inert_documents;
        arena.entries.push(Entry::Node(InertNode {
            kind: Kind::TemplateDocument {
                implementation: first + 1,
            },
            owner: first,
            parent: None,
            children: Vec::new(),
        }));
        arena.entries.push(Entry::Implementation);
        arena.template_documents.commit(indexes.documents);
        arena.template_contents.commit(indexes.contents);
        arena.template_documents.insert(DOCUMENT, first);
        Ok(first)
    }
    pub(in crate::scripting) fn prepare_main_template_contents(
        &mut self,
        copies: &[(u32, u32)],
        deep: bool,
    ) -> Result<(), String> {
        if copies.is_empty() {
            return Ok(());
        }
        let owner = self.main_template_owner()?;
        let mut links = Vec::new();
        links
            .try_reserve_exact(copies.len())
            .map_err(|_| "Cannot reserve imported template links.")?;
        for &(source, destination) in copies {
            if !deep {
                let root = self.reserve_inert_entries(1, 0, 0)?;
                self.state
                    .inert_documents
                    .entries
                    .push(Entry::Node(InertNode {
                        kind: Kind::Fragment {
                            template: Some(destination),
                        },
                        owner,
                        parent: None,
                        children: Vec::new(),
                    }));
                links.push((destination, root));
                continue;
            }
            let Value::Host(fragment) = self.template_content(source)? else {
                unreachable!()
            };
            let start = self.state.inert_documents.entries.len();
            let Value::Host(cloned) = self.clone_inert_tree(fragment, deep)? else {
                unreachable!()
            };
            let end = self.state.inert_documents.entries.len();
            self.state.work(end - start)?;
            let arena = &mut self.state.inert_documents;
            for entry in &mut arena.entries[start..end] {
                if let Entry::Node(node) = entry {
                    node.owner = owner;
                }
            }
            let root = arena
                .node_mut(cloned)
                .ok_or("Imported template content disappeared.")?;
            root.kind = Kind::Fragment {
                template: Some(destination),
            };
            root.parent = None;
            links.push((destination, cloned));
        }
        let arena = &self.state.inert_documents;
        let length = arena
            .template_contents
            .len()
            .checked_add(links.len())
            .ok_or("Imported template link count overflow.")?;
        if links.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || links
                .iter()
                .any(|(key, _)| arena.template_contents.contains_key(key))
        {
            return Err("Imported template links require distinct unpublished hosts.".into());
        }
        let sorting = length
            .checked_mul((usize::BITS - length.leading_zeros()) as usize + 1)
            .ok_or("Imported template index sort overflow.")?;
        self.state.work(sorting)?;
        self.state.copies(
            sorting
                .checked_mul(size_of::<(u32, u32)>())
                .ok_or("Imported template index copy overflow.")?,
        )?;
        let available = self
            .state
            .retained_limit
            .saturating_sub(self.state.retained);
        let (growth, added) = self
            .state
            .inert_documents
            .template_contents
            .prepare(links.len(), available)?;
        let moved = if growth.is_some() {
            self.state.inert_documents.template_contents.len() * size_of::<(u32, u32)>()
        } else {
            0
        };
        self.state.replace_storage(0, added, moved)?;
        let arena = &mut self.state.inert_documents;
        arena.template_contents.commit(growth);
        arena.template_contents.append_and_sort(links.len(), links);
        Ok(())
    }
}

#[cfg(test)]
#[path = "script_template_main_import_tests.rs"]
mod tests;
