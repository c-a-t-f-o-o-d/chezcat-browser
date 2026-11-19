//! Normalize real Text sibling runs. All validation, accounting and allocation
//! precedes commit; removed arena nodes retain their identities and data.
//! https://dom.spec.whatwg.org/#dom-node-normalize
use super::*;

impl Bridge<'_> {
    pub(super) fn normalize_node(&mut self, object: u32) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        if self.is_frame_handle(object) || !self.namespace_receiver(object) {
            return Err("normalize requires an accessible main-document Node.".into());
        }
        if self.attr_receiver(object) {
            return Ok(Value::Undefined);
        }
        let root = if object == DOCUMENT {
            0
        } else {
            self.dom_node(object)?
        };
        // The receiver itself is excluded from the descendant traversal.
        if self.doc.nodes[root].kind.character_data().is_some() {
            return Ok(Value::Undefined);
        }
        let count = self.doc.nodes.len();
        self.state.work(count)?;
        let mut seen = vec![false; count];
        let mut pending = vec![root];
        let mut parents: Vec<(usize, Vec<usize>)> = Vec::new();
        let mut replacements: Vec<(usize, Vec<usize>, usize)> = Vec::new();
        let mut removed = Vec::new();
        let mut old_bytes = 0usize;
        let mut new_bytes = 0usize;
        let mut connected = false;
        while let Some(parent) = pending.pop() {
            self.state.work(1)?;
            if parent >= count || seen[parent] {
                return Err("Invalid or cyclic normalization tree.".into());
            }
            seen[parent] = true;
            let source = &self.doc.nodes[parent].children;
            self.state.work(source.len())?;
            self.state.copies(
                source
                    .len()
                    .checked_mul(std::mem::size_of::<usize>())
                    .ok_or("Normalize child accounting overflow.")?,
            )?;
            let children = source.clone();
            let mut kept = Vec::new();
            kept.try_reserve(children.len())
                .map_err(|_| "Normalize child storage reservation failed.")?;
            let mut index = 0;
            let before = removed.len();
            while index < children.len() {
                self.state.work(1)?;
                let child = children[index];
                if child >= count {
                    return Err("Invalid normalization child index.".into());
                }
                if !matches!(self.doc.nodes[child].kind, NodeKind::Text(_)) {
                    kept.push(child);
                    pending.push(child);
                    index += 1;
                    continue;
                }
                let start = index;
                let mut keeper = None;
                let mut length = 0usize;
                while index < children.len() {
                    let node = children[index];
                    let Some(item) = self.doc.nodes.get(node) else {
                        return Err("Invalid normalization text index.".into());
                    };
                    let NodeKind::Text(text) = &item.kind else {
                        break;
                    };
                    self.state.work(1)?;
                    if !text.is_empty() && keeper.is_none() {
                        keeper = Some(node);
                    }
                    length = length
                        .checked_add(text.len())
                        .ok_or("Normalize text size overflow.")?;
                    index += 1;
                }
                if let Some(keeper) = keeper {
                    kept.push(keeper);
                    let NodeKind::Text(old) = &self.doc.nodes[keeper].kind else {
                        unreachable!()
                    };
                    if length != old.len() {
                        if length > self.state.limits.host.string_bytes {
                            return Err("Normalized Text exceeds the host string bound.".into());
                        }
                        old_bytes = old_bytes
                            .checked_add(old.len())
                            .ok_or("Normalize retained accounting overflow.")?;
                        new_bytes = new_bytes
                            .checked_add(length)
                            .ok_or("Normalize retained accounting overflow.")?;
                        replacements.push((keeper, children[start..index].to_vec(), length));
                    }
                }
                for &node in &children[start..index] {
                    if Some(node) != keeper {
                        removed.push(node);
                    }
                }
            }
            if removed.len() != before {
                if parent != 0 && self.doc.nodes[parent].namespace != crate::html::Namespace::Html {
                    return Err(
                        "Normalization requires known attribute/tree namespace provenance.".into(),
                    );
                }
                self.mutation_ancestors(parent)?;
                connected |= self.state.dom.connected[parent];
                parents.push((parent, kept));
            }
        }
        if parents.is_empty() {
            return Ok(Value::Undefined);
        }
        let retained = self
            .state
            .retained
            .saturating_sub(old_bytes)
            .checked_add(new_bytes)
            .ok_or("Normalize retained accounting overflow.")?;
        if retained > self.state.retained_limit {
            return Err("DOM retained text growth limit reached during normalize.".into());
        }
        self.state.copies(new_bytes)?;
        let mut strings = Vec::new();
        strings
            .try_reserve(replacements.len())
            .map_err(|_| "Normalize text storage reservation failed.")?;
        for (keeper, run, length) in replacements {
            let mut text = String::new();
            text.try_reserve_exact(length)
                .map_err(|_| "Normalize text reservation failed.")?;
            for node in run {
                let NodeKind::Text(part) = &self.doc.nodes[node].kind else {
                    unreachable!()
                };
                text.push_str(part);
            }
            strings.push((keeper, text));
        }
        self.state
            .detached_nodes
            .try_reserve(removed.len())
            .map_err(|_| "Normalize detached-node reservation failed.")?;
        // String copying was charged before allocation; the retained total was
        // validated before staging and no callback can change it in between.
        self.state.retained = retained;
        // No fallible operation remains after this point.
        for (keeper, text) in strings {
            self.doc.nodes[keeper].kind = NodeKind::Text(text);
        }
        for (parent, children) in parents {
            self.doc.nodes[parent].children = children;
        }
        for node in removed {
            self.state.detached_nodes.insert(node);
            self.state.dom.connected[node] = false;
            self.state.tree.parents[node] = None;
        }
        self.state.dom_committed();
        self.state.dirty = true;
        if connected {
            self.state.layout_changed();
        }
        Ok(Value::Undefined)
    }
}
