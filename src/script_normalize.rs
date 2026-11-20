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
        let mut text_copies = 0usize;
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
            self.state.copies(source.len().checked_mul(size_of::<usize>()).ok_or("Normalize child copy work overflow.")?)?;
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
                            .checked_add(old.capacity())
                            .ok_or("Normalize retained accounting overflow.")?;
                        new_bytes = new_bytes
                            .checked_add(length)
                            .ok_or("Normalize retained accounting overflow.")?;
                        text_copies = text_copies.checked_add(length).ok_or("Normalize text copy work overflow.")?;
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
                if kept.is_empty() {
                    kept = Vec::new();
                } else if kept.capacity() != kept.len() {
                    self.state.copies(kept.len().checked_mul(size_of::<usize>()).ok_or("Normalize compact child copy work overflow.")?)?;
                    let mut compact = Vec::new();
                    compact.try_reserve_exact(kept.len()).map_err(|_| "Normalize compact child reservation failed.")?;
                    compact.append(&mut kept);
                    kept = compact;
                }
                old_bytes = old_bytes.checked_add(child_link_bytes(&self.doc.nodes[parent].children))
                    .ok_or("Normalize old child storage overflow.")?;
                new_bytes = new_bytes.checked_add(child_link_bytes(&kept)).ok_or("Normalize new child storage overflow.")?;
                parents.push((parent, kept));
            }
        }
        if parents.is_empty() {
            return Ok(Value::Undefined);
        }
        let mut retained = self
            .state
            .retained
            .saturating_sub(old_bytes)
            .checked_add(new_bytes)
            .ok_or("Normalize retained accounting overflow.")?;
        if retained > self.state.retained_limit {
            return Err("DOM retained text growth limit reached during normalize.".into());
        }
        self.state.copies(text_copies)?;
        let mut strings = Vec::new();
        strings
            .try_reserve(replacements.len())
            .map_err(|_| "Normalize text storage reservation failed.")?;
        for (keeper, run, length) in replacements {
            let mut text = String::new();
            text.try_reserve_exact(length)
                .map_err(|_| "Normalize text reservation failed.")?;
            retained = retained.checked_add(text.capacity() - length)
                .filter(|bytes| *bytes <= self.state.retained_limit)
                .ok_or("DOM retained text allocation limit reached during normalize.")?;
            for node in run {
                let NodeKind::Text(part) = &self.doc.nodes[node].kind else {
                    unreachable!()
                };
                text.push_str(part);
            }
            strings.push((keeper, text));
        }
        // String copying was charged before allocation; the retained total was
        // validated before staging and no callback can change it in between.
        self.state.prepare_range_changes(removed.len(),1)?;
        if !self.state.ranges.is_empty() {
            let bytes=parents.iter().try_fold(0usize,|total,(parent,_)| {
                self.doc.nodes[*parent].children.iter().try_fold(total,|total,&node|total.checked_add(self.doc.nodes[node].kind.character_data().map_or(0,str::len)))
            }).ok_or("Range normalization work overflow.")?;
            let children=parents.iter().try_fold(0usize,|total,(parent,_)|total.checked_add(self.doc.nodes[*parent].children.len())).ok_or("Range normalization child work overflow.")?;
            self.state.work(bytes.checked_add(children).ok_or("Range normalization scan overflow.")?)?;
            // Every allocation/refusal precedes these infallible live updates.
            for (parent,_) in &parents {
                let mut index=0usize;let mut keeper=None;let mut offset=0usize;
                for &node in &self.doc.nodes[*parent].children {
                    if let NodeKind::Text(text)=&self.doc.nodes[node].kind {
                        let length=text.encode_utf16().count();
                        if let Some(kept)=keeper {
                            self.state.ranges_merge_text(*parent,index,node,kept,offset);offset+=length;
                        } else if text.is_empty() {
                            self.state.ranges_remove(*parent,index,1,&[node]);
                        } else {keeper=Some(node);offset=length;index+=1;}
                    } else {keeper=None;offset=0;index+=1;}
                }
            }
        }
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
