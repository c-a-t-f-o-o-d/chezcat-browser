//! Native HTML title content and its displayed, whitespace-normalized document title.
use super::*;

impl Bridge<'_> {
    pub(super) fn title_receiver(&self, object: u32) -> bool {
        if self.inert_node_type(object).is_some() {
            self.inert_interface(object) == Some("HTMLTitleElement")
        } else {
            self.attr_element_receiver(object)
                && node_index(object).is_some_and(|node| {
                    self.doc.nodes[node].namespace == crate::html::Namespace::Html
                        && tag(self.doc, node) == "title"
                })
        }
    }

    pub(super) fn title_child_text(
        &mut self,
        object: u32,
        normalize: bool,
    ) -> Result<DomString, String> {
        self.native_node_access(object)
            .map_err(|(_, message)| message)?;
        if self.inert_node_type(object).is_none() {
            self.state.rebuild(self.doc)?;
        }
        let n = if let Some(children) = self.inert_children(object) {
            children.len()
        } else {
            self.doc.nodes[self.dom_node_index_for_title(object)?]
                .children
                .len()
        };
        self.state.work(n)?;
        self.state.copies(
            n.checked_mul(size_of::<u32>())
                .ok_or("Title child storage overflow.")?,
        )?;
        let mut children = Vec::new();
        children
            .try_reserve_exact(n)
            .map_err(|_| "Cannot reserve title child handles.")?;
        if let Some(nodes) = self.inert_children(object) {
            children.extend(
                nodes
                    .iter()
                    .copied()
                    .filter(|&node| matches!(self.inert_node_type(node), Some(3 | 4))),
            );
        } else {
            let node = self.dom_node_index_for_title(object)?;
            children.extend(
                self.doc.nodes[node]
                    .children
                    .iter()
                    .copied()
                    .filter(|&n| {
                        matches!(
                            self.doc.nodes[n].kind,
                            NodeKind::Text(_) | NodeKind::Cdata(_)
                        )
                    })
                    .map(node_handle),
            );
        }
        let input = children.iter().try_fold(0usize, |sum, &node| {
            sum.checked_add(
                self.character_data_source(node)
                    .ok_or("Title Text source disappeared.")?
                    .logical_bytes(),
            )
            .ok_or("Title input size overflow.")
        })?;
        self.state.work(input)?;
        if normalize {
            let count = children.iter().try_fold(0usize, |sum, &node| {
                sum.checked_add(self.character_data_source(node).unwrap().utf16_len())
                    .ok_or("Title unit count overflow.")
            })?;
            self.state
                .copies(count.checked_mul(2).ok_or("Title unit storage overflow.")?)?;
            let units = normalized_units(
                children
                    .iter()
                    .flat_map(|&node| self.character_data_source(node).unwrap().code_units()),
                count,
            )?;
            return self.finish_normalized_title(&units);
        }
        let refs = children
            .len()
            .checked_mul(size_of::<&DomString>())
            .ok_or("Title part size overflow.")?;
        self.state.copies(refs.saturating_mul(2))?;
        let (logical, copy) = {
            let parts = self.title_parts(&children)?;
            (
                DomString::concat_size(&parts)?.0,
                DomString::concat_copy_work(&parts)?,
            )
        };
        let limit = self.state.limits.host.string_bytes;
        if logical > limit {
            return Err("Title text exceeds the host string allowance.".into());
        }
        self.state.copies(copy)?;
        DomString::concat(
            &self.title_parts(&children)?,
            DomString::storage_bound(limit)?,
        )
    }

    fn dom_node_index_for_title(&self, object: u32) -> Result<usize, String> {
        node_index(object)
            .filter(|&n| self.dom_accessible(n))
            .ok_or_else(|| "Invalid native title node.".into())
    }

    fn title_parts<'a>(&'a self, children: &[u32]) -> Result<Vec<&'a DomString>, String> {
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(children.len())
            .map_err(|_| "Cannot reserve title text references.")?;
        for &node in children {
            parts.push(
                self.character_data_source(node)
                    .ok_or("Missing title Text child.")?,
            );
        }
        Ok(parts)
    }

    pub(super) fn normalized_title(&mut self, value: &DomString) -> Result<DomString, String> {
        self.state.work(value.logical_bytes())?;
        let count = value.utf16_len();
        self.state
            .copies(count.checked_mul(2).ok_or("Title unit count overflow.")?)?;
        let units = normalized_units(value.code_units(), count)?;
        self.finish_normalized_title(&units)
    }

    fn finish_normalized_title(&mut self, units: &[u16]) -> Result<DomString, String> {
        let (bytes, raw) = DomString::projected_size(units)?;
        let logical = if raw { units.len() * 2 } else { bytes };
        if logical > self.state.limits.host.string_bytes {
            return Err("Document title exceeds the host string allowance.".into());
        }
        self.state.copies(
            bytes
                .checked_add(if raw { logical } else { 0 })
                .ok_or("Title copy overflow.")?,
        )?;
        DomString::from_units(
            units,
            DomString::storage_bound(self.state.limits.host.string_bytes)?,
        )
    }

    pub(super) fn set_title_text(&mut self, object: u32, value: &Value) -> Result<(), String> {
        self.native_node_access(object)
            .map_err(|(_, message)| message)?;
        let text = self.dom_text(value)?;
        if self.inert_node_type(object) == Some(1) {
            return self.replace_inert_text_content(object, text);
        }
        let node = self.dom_node(object)?;
        self.set_text_content(node, text)
    }
}

fn normalized_units(input: impl Iterator<Item = u16>, count: usize) -> Result<Vec<u16>, String> {
    let mut units = Vec::new();
    units
        .try_reserve_exact(count)
        .map_err(|_| "Cannot reserve normalized title units.")?;
    let mut space = false;
    for unit in input {
        if matches!(unit, 9 | 10 | 12 | 13 | 32) {
            space = !units.is_empty();
            continue;
        }
        if space {
            units.push(32);
            space = false;
        }
        units.push(unit);
    }
    Ok(units)
}
