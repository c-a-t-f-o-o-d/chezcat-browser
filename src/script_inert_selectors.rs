//! Read-only current inert topology adapted to the shared native CSS matcher.
//! No markup roundtrip, resource activation or alternate selector engine.
use super::*;

#[derive(Clone, Copy)]
struct Row {
    handle: u32,
    parent: Option<usize>,
    slot: usize,
    depth: usize,
}
fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    used: &mut usize,
    available: usize,
) -> Result<(), String> {
    let minimum = count
        .checked_mul(size_of::<T>())
        .ok_or("Inert selector backing overflow.")?;
    if minimum > available.saturating_sub(*used) {
        return Err("Inert selector temporary allowance exhausted.".into());
    }
    values
        .try_reserve_exact(count)
        .map_err(|_| "Cannot reserve inert selector view.")?;
    *used = used
        .checked_add(
            values
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or("Inert selector backing overflow.")?,
        )
        .filter(|bytes| *bytes <= available)
        .ok_or("Inert selector temporary allowance exhausted.")?;
    Ok(())
}
impl Bridge<'_> {
    fn inert_query_push<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        used: &mut usize,
        available: usize,
    ) -> Result<(), String> {
        let (growth, added) = retained_storage::grow(values, 1, available.saturating_sub(*used))?;
        self.state.work(1)?;
        if let Some(mut next) = growth {
            self.state.copies(
                values
                    .len()
                    .checked_mul(size_of::<T>())
                    .ok_or("Inert selector movement overflow.")?,
            )?;
            next.append(values);
            *values = next;
        }
        *used = used
            .checked_add(added)
            .ok_or("Inert selector backing overflow.")?;
        values.push(value);
        Ok(())
    }
    fn inert_query_node(&mut self, handle: u32, available: usize) -> Result<(Node, usize), String> {
        let source = self.inert_import_source(handle);
        let minimum = match source {
            Some(ImportSource::Parsed { node, .. }) => copied_node_bytes(node)
                .checked_add(match &node.kind {
                    NodeKind::Element { tag, href, .. } => {
                        tag.len() + href.as_ref().map_or(0, String::len)
                    }
                    _ => 0,
                })
                .ok_or("Inert selector payload overflow.")?,
            Some(ImportSource::Element(name)) => name.len(),
            Some(ImportSource::Text(text)) => text.used_bytes(),
            Some(ImportSource::ProcessingInstruction { target, data }) => target.len() + data.used_bytes(),
            Some(ImportSource::Fragment) => 0,
            None => {
                if self.inert_node_type(handle) == Some(9) {
                    9
                } else {
                    0
                }
            }
        };
        let work = match source {
            Some(ImportSource::Parsed { node, .. }) => {
                if let NodeKind::Element { attrs, .. } = &node.kind {
                    attrs.len() + 1
                } else {
                    1
                }
            }
            _ => 1,
        };
        if minimum > available {
            return Err("Inert selector temporary payload allowance exhausted.".into());
        }
        self.state.work(work)?;
        self.state.copies(minimum)?;
        let (kind, namespace) = match self.inert_import_source(handle) {
            Some(ImportSource::Parsed { node, .. }) => {
                if node.namespace == crate::html::Namespace::UnknownForeign {
                    return Err("Inert selector unknown foreign provenance is unavailable.".into());
                }
                (
                    script_clone_node::copy_node_kind(&node.kind)?,
                    node.namespace,
                )
            }
            Some(ImportSource::Element(name)) => (
                NodeKind::Element {
                    tag: script_clone_node::copy_text(name)?,
                    id: None,
                    href: None,
                    attrs: Vec::new(),
                },
                crate::html::Namespace::Html,
            ),
            Some(ImportSource::Text(text)) => (
                NodeKind::Text(text.try_clone()?),
                crate::html::Namespace::None,
            ),
            Some(ImportSource::ProcessingInstruction { target, data }) => (
                NodeKind::ProcessingInstruction {
                    target: script_clone_node::copy_text(target)?,
                    data: data.try_clone()?,
                },
                crate::html::Namespace::None,
            ),
            Some(ImportSource::Fragment) => {
                (NodeKind::DocumentFragment, crate::html::Namespace::None)
            }
            None if self.inert_node_type(handle) == Some(9) => (
                NodeKind::Element {
                    tag: "#document".into(),
                    id: None,
                    href: None,
                    attrs: Vec::new(),
                },
                crate::html::Namespace::None,
            ),
            // Doctypes are non-elements. Their data has no selector semantics
            // in the supported grammar; no corresponding DOM node is created.
            None if self.inert_node_type(handle) == Some(10) => {
                (NodeKind::DocumentFragment, crate::html::Namespace::None)
            }
            _ => return Err("Inert selector node is unavailable.".into()),
        };
        let node = Node {
            kind,
            namespace,
            children: Vec::new(),
        };
        let extra = match &node.kind {
            NodeKind::Element { tag, href, .. } => {
                tag.capacity() + href.as_ref().map_or(0, String::capacity)
            }
            _ => 0,
        };
        let bytes = node_bytes(&node)
            .checked_add(extra)
            .filter(|bytes| *bytes <= available)
            .ok_or("Inert selector allocated payload allowance exhausted.")?;
        self.state.copies(bytes.saturating_sub(minimum))?;
        Ok((node, bytes))
    }
    pub(in crate::scripting) fn inert_selector_query(
        &mut self,
        object: u32,
        method: &str,
        source: &str,
    ) -> Result<Value, String> {
        if !matches!(self.inert_node_type(object), Some(1 | 9 | 11)) {
            return Err("Inert selector requires an Element, Document or DocumentFragment.".into());
        }
        let mut top = object;
        while let Some(parent) = self
            .state
            .inert_documents
            .node(top)
            .ok_or("Inert selector owner is unavailable.")?
            .parent
        {
            self.state.work(1)?;
            top = parent;
        }
        let available = self
            .state
            .retained_limit
            .saturating_sub(self.state.retained);
        let mut used = 0;
        let mut rows = Vec::new();
        let mut pending = Vec::new();
        self.inert_query_push(
            &mut pending,
            Row {
                handle: top,
                parent: None,
                slot: 0,
                depth: 0,
            },
            &mut used,
            available,
        )?;
        let mut selected = None;
        while let Some(row) = pending.pop() {
            let index = rows.len();
            if row.handle == object {
                selected = Some(index);
            }
            self.inert_query_push(&mut rows, row, &mut used, available)?;
            let length = self
                .inert_children(row.handle)
                .ok_or("Inert selector children unavailable.")?
                .len();
            self.state.work(length)?;
            for slot in (0..length).rev() {
                let child = self.inert_children(row.handle).unwrap()[slot];
                if self
                    .state
                    .inert_documents
                    .node(child)
                    .and_then(|node| node.parent)
                    != Some(row.handle)
                {
                    return Err("Inert selector child topology is inconsistent.".into());
                }
                self.inert_query_push(
                    &mut pending,
                    Row {
                        handle: child,
                        parent: Some(index),
                        slot,
                        depth: row
                            .depth
                            .checked_add(1)
                            .ok_or("Inert selector depth overflow.")?,
                    },
                    &mut used,
                    available,
                )?;
            }
        }
        let root = selected.ok_or("Inert selector scope is unavailable.")?;
        let mut nodes = Vec::new();
        let mut parents = Vec::new();
        let mut slots = Vec::new();
        let mut candidates = Vec::new();
        reserve(&mut nodes, rows.len(), &mut used, available)?;
        reserve(&mut parents, rows.len(), &mut used, available)?;
        reserve(&mut slots, rows.len(), &mut used, available)?;
        reserve(&mut candidates, rows.len(), &mut used, available)?;
        self.state.copies(
            rows.len()
                .checked_mul(
                    size_of::<Node>() + size_of::<Option<usize>>() + 2 * size_of::<usize>(),
                )
                .ok_or("Inert selector initialization overflow.")?,
        )?;
        for row in &rows {
            let (mut node, bytes) =
                self.inert_query_node(row.handle, available.saturating_sub(used))?;
            used = used
                .checked_add(bytes)
                .ok_or("Inert selector payload overflow.")?;
            reserve(
                &mut node.children,
                self.inert_children(row.handle).unwrap().len(),
                &mut used,
                available,
            )?;
            parents.push(row.parent);
            slots.push(row.slot);
            nodes.push(node);
        }
        for (index, row) in rows.iter().enumerate() {
            if let Some(parent) = row.parent {
                if nodes[parent].children.len() != row.slot {
                    return Err("Inert selector sibling topology is inconsistent.".into());
                }
                nodes[parent].children.push(index);
            }
        }
        match method {
            "matches" => candidates.push(root),
            "closest" => {
                let mut current = Some(root);
                while let Some(index) = current {
                    self.state.work(1)?;
                    if self.inert_node_type(rows[index].handle) == Some(1) {
                        candidates.push(index);
                    }
                    current = parents[index];
                }
            }
            _ => {
                for index in root + 1..rows.len() {
                    self.state.work(1)?;
                    if rows[index].depth <= rows[root].depth {
                        break;
                    }
                    if self.inert_node_type(rows[index].handle) == Some(1) {
                        candidates.push(index);
                    }
                }
            }
        }
        let document = Document {
            kind:crate::html::DocumentKind::Html,mode:crate::html::DocumentMode::NoQuirks,
            stylesheet_limits: crate::css::StylesheetLimits::default(),
            media_environment: None,
            title: String::new(),
            content_type: String::new(),
            nodes,
            scripting_enabled: false,
            nonce_unsafe_nodes: HashSet::new(),
            withheld_inline_styles: HashSet::new(),
            native_focus: crate::html::NativeNode::default(),
            native_hover: crate::html::NativeNode::default(),
        };
        let found = crate::css::query_with_topology(
            &document,
            source,
            candidates,
            None,
            &mut self.state.host_work,
            &parents,
            &slots,
            method != "querySelectorAll",
            &mut self.state.copy_work,
        )?;
        if method == "matches" {
            return Ok(Value::Bool(!found.is_empty()));
        }
        if method != "querySelectorAll" {
            return Ok(found
                .first()
                .map_or(Value::Null, |&index| Value::Host(rows[index].handle)));
        }
        let mut handles = Vec::new();
        reserve(&mut handles, found.len(), &mut used, available)?;
        self.state.copies(
            found
                .len()
                .checked_mul(size_of::<u32>())
                .ok_or("Inert selector result copy overflow.")?,
        )?;
        handles.extend(found.iter().map(|&index| rows[index].handle));
        drop(document);
        drop(rows);
        drop(parents);
        drop(slots);
        drop(pending);
        drop(found);
        self.snapshot_query_handles(handles)
    }
}
