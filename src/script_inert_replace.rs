//! Atomic replacement inside supported genuine inert-owner trees.
//! https://dom.spec.whatwg.org/#concept-node-replace
use super::*;
use crate::javascript::NativeChildListChange;
impl Bridge<'_> {
    pub(in crate::scripting) fn inert_replace_receiver(&self, parent: u32) -> bool {
        self.inert_node_type(parent).is_some()
    }
    pub(in crate::scripting) fn validate_inert_replace(
        &self,
        parent: u32,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if args.len() < 2
            || !args[..2]
                .iter()
                .all(|v| matches!(v,Value::Host(h) if self.get_root_receiver(*h)))
        {
            return Err((
                "TypeError",
                "replaceChild requires two genuine Node arguments.".into(),
            ));
        }
        match self.inert_node_type(parent) {
            Some(1 | 11) => {}
            Some(9) => {
                return Err((
                    "NotSupportedError",
                    "Replacing inert Document structure is not implemented.".into(),
                ))
            }
            _ => {
                return Err((
                    "HierarchyRequestError",
                    "This inert Node cannot contain children.".into(),
                ))
            }
        }
        let Value::Host(incoming) = args[0] else {
            unreachable!()
        };
        let Value::Host(outgoing) = args[1] else {
            unreachable!()
        };
        let arena = &self.state.inert_documents;
        let mut ancestor = Some(parent);
        let bound = arena
            .entries
            .len()
            .saturating_add(arena.adopted.len())
            .saturating_add(self.doc.nodes.len())
            .saturating_add(1);
        for _ in 0..bound {
            let Some(handle) = ancestor else {
                break;
            };
            if handle == incoming {
                return Err((
                    "HierarchyRequestError",
                    "Inert replacement would create a cycle.".into(),
                ));
            }
            ancestor = arena.node(handle).and_then(|n| {
                n.parent.or_else(|| {
                    if let Kind::Fragment { template } = n.kind {
                        template
                    } else {
                        None
                    }
                })
            });
        }
        if ancestor.is_some() {
            return Err((
                "NotSupportedError",
                "Inert ancestor topology is inconsistent.".into(),
            ));
        }
        if arena
            .node(outgoing)
            .is_none_or(|n| n.parent != Some(parent))
        {
            return Err((
                "NotFoundError",
                "The replaced Node is not a child of this inert parent.".into(),
            ));
        }
        if self
            .tree_walker_node_type(incoming)
            .is_some_and(|kind| !matches!(kind, 1 | 3 | 4 | 7 | 8 | 11))
        {
            return Err((
                "HierarchyRequestError",
                "This Node cannot be inserted into an inert Element or fragment.".into(),
            ));
        }
        let Some(source) = arena.node(incoming) else {
            return Err(("NotSupportedError","Replacement requires the same represented inert owner; cross-owner adoption is unavailable.".into()));
        };
        let owner = arena.node(parent).unwrap().owner;
        if source.owner != owner {
            return Err((
                "NotSupportedError",
                "Cross-document inert replacement adoption is not implemented.".into(),
            ));
        }
        for handle in [incoming, outgoing] {
            let n = arena.node(handle).unwrap();
            if arena.node(n.owner).is_some_and(|owner|matches!(&owner.kind,Kind::Document(nodes) if [nodes.html,nodes.head,nodes.body].contains(&Some(handle)))){return Err(("NotSupportedError","Replacing or moving inert document structure is not implemented.".into()));}
        }
        Ok(())
    }
    pub(in crate::scripting) fn replace_inert_child(
        &mut self,
        parent: u32,
        args: &[Value],
    ) -> Result<Value, String> {
        self.validate_inert_replace(parent, args)
            .map_err(|(_, m)| m)?;
        let Value::Host(incoming) = args[0] else {
            unreachable!()
        };
        let Value::Host(outgoing) = args[1] else {
            unreachable!()
        };
        let source = self.state.inert_documents.node(incoming).unwrap();
        let old_parent = source.parent;
        let fragment = self.inert_node_type(incoming) == Some(11);
        let owner = source.owner;
        let count = if fragment { source.children.len() } else { 1 };
        self.state.work(
            self.state
                .inert_documents
                .node(parent)
                .unwrap()
                .children
                .len()
                .saturating_add(old_parent.map_or(0, |p| {
                    self.state
                        .inert_documents
                        .node(p)
                        .map_or(0, |n| n.children.len())
                }))
                .saturating_add(count),
        )?;
        let mut ancestor = Some(parent);
        while let Some(handle) = ancestor {
            self.state.work(1)?;
            ancestor = self.state.inert_documents.node(handle).and_then(|n| {
                n.parent.or_else(|| {
                    if let Kind::Fragment { template } = n.kind {
                        template
                    } else {
                        None
                    }
                })
            });
        }
        if fragment {
            if old_parent.is_some() {
                return Err("Inert replacement fragment must have no parent.".into());
            }
            for at in 0..count {
                let root = self.inert_children(incoming).unwrap()[at];
                if self
                    .state
                    .inert_documents
                    .node(root)
                    .is_none_or(|n| n.parent != Some(incoming) || n.owner != owner)
                {
                    return Err("Inert replacement fragment ownership is inconsistent.".into());
                }
                self.validate_svg_edge(parent, root)?;
            }
        } else {
            self.validate_svg_edge(parent, incoming)?;
        }
        let siblings = &self.state.inert_documents.node(parent).unwrap().children;
        let original = siblings.len();
        let out_at = siblings
            .iter()
            .position(|h| *h == outgoing)
            .ok_or("Inert replacement child link is unavailable.")?;
        let source_at = if let Some(source) = old_parent {
            Some(
                self.inert_children(source)
                    .ok_or("Inert replacement source is unavailable.")?
                    .iter()
                    .position(|h| *h == incoming)
                    .ok_or("Inert replacement source link is unavailable.")?,
            )
        } else {
            None
        };
        let same_parent = old_parent == Some(parent);
        let remove_old = incoming != outgoing;
        let insertion = out_at
            .checked_sub(usize::from(same_parent && source_at.unwrap() < out_at))
            .ok_or("Inert replacement position underflow.")?;
        let after = original
            .checked_sub(usize::from(same_parent))
            .and_then(|n| n.checked_sub(usize::from(remove_old)))
            .ok_or("Inert replacement child length underflow.")?;
        let final_len = after
            .checked_add(count)
            .ok_or("Inert replacement child length overflow.")?;
        let tail = after
            .checked_sub(insertion)
            .ok_or("Inert replacement insertion is unavailable.")?;
        let old_shift = if remove_old {
            original - usize::from(same_parent) - insertion - 1
        } else {
            0
        };
        let source_shift = old_parent.map_or(0, |p| {
            self.inert_children(p).unwrap().len() - source_at.unwrap() - 1
        });
        let (replacement, storage) = retained_storage::grow(
            siblings,
            final_len.saturating_sub(original),
            self.state
                .retained_limit
                .saturating_sub(self.state.retained),
        )?;
        let copied = if replacement.is_some() { original } else { 0 };
        let slots = copied
            .checked_add(source_shift)
            .and_then(|n| n.checked_add(old_shift))
            .and_then(|n| n.checked_add(tail))
            .and_then(|n| n.checked_add(count.checked_mul(3)?))
            .and_then(|n| n.checked_add(2))
            .ok_or("Inert replacement copy size overflow.")?;
        self.state.work(slots)?;
        self.state.replace_storage(
            0,
            storage,
            slots
                .checked_mul(size_of::<u32>())
                .ok_or("Inert replacement copy bytes overflow.")?,
        )?;
        // Both list growth and every move/parent write are admitted. Publish
        // without callbacks or fallible work between source and destination.
        let arena = &mut self.state.inert_documents;
        if let Some(mut next) = replacement {
            let list = &mut arena.node_mut(parent).unwrap().children;
            next.append(list);
            *list = next;
        }
        if let Some(source) = old_parent {
            arena
                .node_mut(source)
                .unwrap()
                .children
                .remove(source_at.unwrap());
        }
        if remove_old {
            arena.node_mut(parent).unwrap().children.remove(insertion);
        }
        arena.node_mut(outgoing).unwrap().parent = None;
        let children = &mut arena.node_mut(parent).unwrap().children;
        let length = children.len();
        children.resize(final_len, 0);
        children.copy_within(insertion..length, insertion + count);
        if fragment {
            for at in 0..count {
                let root = arena.node(incoming).unwrap().children[at];
                arena.node_mut(root).unwrap().parent = Some(parent);
                arena.node_mut(parent).unwrap().children[insertion + at] = root;
            }
            arena.node_mut(incoming).unwrap().children.clear();
        } else {
            arena.node_mut(incoming).unwrap().parent = Some(parent);
            arena.node_mut(parent).unwrap().children[insertion] = incoming;
        }
        self.state.dom_committed();
        Ok(Value::Host(outgoing))
    }
    pub(in crate::scripting) fn inert_replace_changes(
        &mut self,
        parent: u32,
        args: &[Value],
    ) -> Result<Vec<NativeChildListChange>, String> {
        let [Value::Host(incoming), Value::Host(outgoing), ..] = args else {
            return Ok(Vec::new());
        };
        self.validate_inert_replace(parent, args)
            .map_err(|(_, m)| m)?;
        let fragment = self.inert_node_type(*incoming) == Some(11);
        let source = self.inert_observation_parent(*incoming);
        let siblings = self
            .inert_children(parent)
            .ok_or("Inert replacement observer parent is unavailable.")?;
        let length = siblings.len();
        let at = siblings
            .iter()
            .position(|h| h == outgoing)
            .ok_or("Inert replacement observer child is unavailable.")?;
        let previous = at.checked_sub(1).and_then(|i| siblings.get(i)).copied();
        let mut next = siblings.get(at + 1).copied();
        if next == Some(*incoming) {
            next = siblings.get(at + 2).copied();
        }
        let count = if fragment {
            self.inert_children(*incoming)
                .ok_or("Inert replacement observer fragment is unavailable.")?
                .len()
        } else {
            1
        };
        self.state.work(
            length.saturating_add(count).saturating_add(
                source.map_or(0, |p| self.inert_children(p).map_or(0, |c| c.len())),
            ),
        )?;
        let bytes = count
            .checked_mul(2 * size_of::<u32>())
            .and_then(|n| {
                n.checked_add(2 * size_of::<NativeChildListChange>() + 2 * size_of::<u32>())
            })
            .ok_or("Inert replacement observer size overflow.")?;
        self.state.copies(bytes)?;
        let mut added = Vec::new();
        added
            .try_reserve_exact(count)
            .map_err(|_| "Cannot reserve inert replacement added nodes.")?;
        if fragment {
            added.extend_from_slice(self.inert_children(*incoming).unwrap());
        } else {
            added.push(*incoming);
        }
        let mut changes = Vec::new();
        changes
            .try_reserve_exact(2)
            .map_err(|_| "Cannot reserve inert replacement records.")?;
        if fragment && count != 0 {
            let mut removed = Vec::new();
            removed
                .try_reserve_exact(count)
                .map_err(|_| "Cannot reserve inert fragment removal snapshot.")?;
            removed.extend_from_slice(&added);
            changes.push(NativeChildListChange {
                target: *incoming,
                added: Vec::new(),
                removed,
                previous: None,
                next: None,
            });
        } else if !fragment {
            if let Some(source) = source {
                let siblings = self.inert_children(source).unwrap();
                let i = siblings
                    .iter()
                    .position(|h| h == incoming)
                    .ok_or("Inert observer replacement source link is unavailable.")?;
                let previous = i.checked_sub(1).and_then(|i| siblings.get(i)).copied();
                let next = siblings.get(i + 1).copied();
                changes.push(NativeChildListChange {
                    target: source,
                    added: Vec::new(),
                    removed: vec![*incoming],
                    previous,
                    next,
                });
            }
        }
        changes.push(NativeChildListChange {
            target: parent,
            added,
            removed: if incoming == outgoing {
                Vec::new()
            } else {
                vec![*outgoing]
            },
            previous,
            next,
        });
        Ok(changes)
    }
}

#[cfg(test)]
#[path = "script_inert_replace_tests.rs"]
mod tests;
