//! Shared DOM insertion positions and atomic creation of a single adjacent Text node.
use super::*;
use crate::javascript::adjacent_interface::{Destination, Position};

type Error = (&'static str, String);
fn storage(message: impl Into<String>) -> Error {
    ("Error", message.into())
}
fn unsupported(message: impl Into<String>) -> Error {
    ("NotSupportedError", message.into())
}

fn gap<T: PartialEq + Copy>(children: &[T], target: T, pos: Position) -> Result<usize, Error> {
    match pos {
        Position::AfterBegin => Ok(0),
        Position::BeforeEnd => Ok(children.len()),
        Position::BeforeBegin | Position::AfterEnd => {
            let at = children
                .iter()
                .position(|&child| child == target)
                .ok_or_else(|| storage("Adjacent parent has no receiver edge."))?;
            Ok(at + usize::from(pos == Position::AfterEnd))
        }
    }
}

fn neighbors<T: Copy>(
    children: &[T],
    at: usize,
    handle: impl Fn(T) -> u32,
) -> (Option<u32>, Option<u32>) {
    (
        at.checked_sub(1)
            .and_then(|i| children.get(i))
            .copied()
            .map(&handle),
        children.get(at).copied().map(handle),
    )
}

impl Bridge<'_> {
    pub(in crate::scripting) fn adjacent_access(&self, object: u32) -> Result<(), Error> {
        if self.collection_interface_receiver(object) != Some(false) {
            return Err((
                "TypeError",
                "Adjacent insertion requires an Element.".into(),
            ));
        }
        self.native_node_access(object)?;
        if self.is_frame_handle(object) {
            return Err(unsupported("Mutation of an initial blank child element requires child-document lifecycle support."));
        }
        Ok(())
    }

    pub(in crate::scripting) fn resolve_adjacent(
        &mut self,
        target: u32,
        pos: Position,
        text: bool,
    ) -> Result<Option<Destination>, Error> {
        self.adjacent_access(target)?;
        self.state.work(1).map_err(storage)?;
        let outer = matches!(pos, Position::BeforeBegin | Position::AfterEnd);
        let (parent, at, previous, next, inert) =
            if let Some(node) = self.state.inert_documents.node(target) {
                let parent = if outer {
                    let Some(p) = node.parent else {
                        return Ok(None);
                    };
                    p
                } else {
                    target
                };
                let length = self
                    .inert_children(parent)
                    .ok_or_else(|| storage("Missing inert adjacent parent."))?
                    .len();
                if outer {
                    self.state.work(length).map_err(storage)?;
                }
                let children = self.inert_children(parent).unwrap();
                let at = gap(children, target, pos)?;
                let (prev, next) = neighbors(children, at, |n| n);
                (parent, at, prev, next, true)
            } else {
                self.state.rebuild(self.doc).map_err(storage)?;
                let node = self.dom_node(target).map_err(storage)?;
                let parent = if outer {
                    let Some(p) = self.state.tree.parents[node] else {
                        return Ok(None);
                    };
                    p
                } else {
                    node
                };
                if outer {
                    self.state
                        .work(self.doc.nodes[parent].children.len())
                        .map_err(storage)?;
                }
                let children = &self.doc.nodes[parent].children;
                let at = gap(children, node, pos)?;
                let (prev, next) = neighbors(children, at, node_handle);
                (
                    if parent == 0 {
                        DOCUMENT
                    } else {
                        node_handle(parent)
                    },
                    at,
                    prev,
                    next,
                    false,
                )
            };
        let future_text = if !text {
            None
        } else {
            if parent == DOCUMENT || self.inert_document(parent) {
                return Err((
                    "HierarchyRequestError",
                    "A Document cannot have a Text child.".into(),
                ));
            }
            if !inert {
                let index = node_index(parent)
                    .ok_or_else(|| storage("Unknown adjacent insertion parent."))?;
                self.mutation_ancestors(index).map_err(unsupported)?;
                if self.doc.nodes[index].namespace != crate::html::Namespace::Html
                    && !matches!(self.doc.nodes[index].kind, NodeKind::DocumentFragment)
                {
                    return Err(unsupported(
                        "Native foreign Text insertion requires its rendering hooks.",
                    ));
                }
                let mut current = Some(index);
                let mut depth = 0;
                while let Some(node) = current {
                    self.state.work(1).map_err(storage)?;
                    if self.dom_script(node) {
                        return Err(unsupported(
                            "Script insertion requires its resource lifecycle.",
                        ));
                    }
                    depth += 1;
                    if depth + 1 > dom_tree::MAX_DOM_DEPTH {
                        return Err(storage(
                            "Adjacent insertion exceeds the native DOM depth limit.",
                        ));
                    }
                    current = self.state.tree.parents[node];
                }
            }
            if self.can_reserve_inert_leaf() {
                Some(node_handle(self.doc.nodes.len()))
            } else if inert {
                Some(
                    FIRST_ENTRY
                        .checked_add(
                            u32::try_from(self.state.inert_documents.entries.len())
                                .map_err(|_| storage("Inert Text identity overflow."))?,
                        )
                        .filter(|&id| id < ENTRY_END)
                        .ok_or_else(|| storage("Inert Text identity space exhausted."))?,
                )
            } else {
                return Err(storage(
                    "DOM appended-node limit reached during adjacent Text insertion.",
                ));
            }
        };
        Ok(Some(Destination {
            parent,
            gap: at,
            previous,
            next,
            future_text,
        }))
    }

    pub(in crate::scripting) fn publish_adjacent_text(
        &mut self,
        target: u32,
        pos: Position,
        dest: Destination,
        value: &Value,
    ) -> Result<(), Error> {
        if self.resolve_adjacent(target, pos, true)? != Some(dest) {
            return Err(storage(
                "Adjacent insertion topology changed before publication.",
            ));
        }
        let text = self.dom_text(value).map_err(storage)?;
        let available = self
            .state
            .retained_limit
            .saturating_sub(self.state.retained);
        let native = self.can_reserve_inert_leaf();
        let mut minimum = text
            .capacity()
            .checked_add(
                self.main_node_minimum(usize::from(native))
                    .map_err(storage)?,
            )
            .ok_or_else(|| storage("Adjacent Text backing size overflow."))?;
        if let Some(parent) = self.state.inert_documents.node(dest.parent) {
            let owner = parent.owner;
            let arena = &self.state.inert_documents;
            if arena.entries.len() == arena.entries.capacity() {
                minimum = minimum.saturating_add(size_of::<Entry>());
            }
            if native {
                minimum = minimum
                    .saturating_add(size_of::<Node>() + 2 * size_of::<usize>())
                    .saturating_add(arena.adopted.minimum_growth(1).map_err(storage)?);
                if arena.generated.len() == arena.generated.capacity() {
                    minimum = minimum.saturating_add(size_of::<GeneratedNode>());
                }
            }
            let budget = available.checked_sub(minimum).ok_or_else(|| {
                storage("Adjacent inert Text exceeds the joint storage allowance.")
            })?;
            let (growth, bytes) =
                retained_storage::grow(&parent.children, 1, budget).map_err(storage)?;
            let work = (parent.children.len() - dest.gap + 1).saturating_add(if growth.is_some() {
                parent.children.len()
            } else {
                0
            });
            self.state
                .copies(work.saturating_mul(size_of::<u32>()))
                .map_err(storage)?;
            let handle = self
                .append_inert_text(owner, Some(dest.parent), text, bytes)
                .map_err(storage)?;
            debug_assert_eq!(Some(handle), dest.future_text);
            let children = &mut self
                .state
                .inert_documents
                .node_mut(dest.parent)
                .unwrap()
                .children;
            if let Some(mut v) = growth {
                v.append(children);
                *children = v;
            }
            children.insert(dest.gap, handle);
        } else {
            let parent = node_index(dest.parent).unwrap();
            let children = &self.doc.nodes[parent].children;
            let budget = available
                .checked_sub(minimum)
                .ok_or_else(|| storage("Adjacent Text exceeds the joint storage allowance."))?;
            let (growth, bytes) = retained_storage::grow(children, 1, budget).map_err(storage)?;
            let moved = (children.len() - dest.gap + 1).saturating_add(if growth.is_some() {
                children.len()
            } else {
                0
            });
            self.state
                .replace_storage(0, bytes, moved.saturating_mul(size_of::<usize>()))
                .map_err(storage)?;
            let node = self.doc.nodes.len();
            // The native factory is atomic. On failure the unpublished child
            // vector is dropped and its temporary charge is returned.
            if let Err(message) = self.create_dom_node(NodeKind::Text(text)) {
                self.state.retained -= bytes;
                return Err(storage(message));
            }
            debug_assert_eq!(Some(node_handle(node)), dest.future_text);
            let children = &mut self.doc.nodes[parent].children;
            if let Some(mut v) = growth {
                v.append(children);
                *children = v;
            }
            children.insert(dest.gap, node);
            self.state.tree.parents[node] = Some(parent);
            self.state.dom.connected[node] = self.state.dom.connected[parent];
            if self.state.dom.connected[parent] {
                self.state.detached_nodes.remove(&node);
                self.state.layout_changed();
            }
            self.state.dirty = true;
        }
        self.state.ranges_insert_handle(dest.parent, dest.gap, 1);
        self.state.dom_committed();
        Ok(())
    }
}
