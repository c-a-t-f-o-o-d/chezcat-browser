//! Shared, immutable computed-property readback from the rendering cascade.
use super::*;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComputedStyleUnavailable;

#[derive(Clone)]
pub(crate) struct ComputedProperties(Rc<Snapshot>);
struct Snapshot {
    variables: Vec<custom_properties::Variables>,
    eligible: Vec<bool>,
    complete: bool,
    overflows: Vec<Option<Overflow>>,
    positions: Vec<Option<Position>>,
    transforms: Vec<Option<Transform>>,
    has_transform_geometry: bool,
    canvas_foregrounds: HashMap<usize, u32>,
}
impl ComputedProperties {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        variables: Vec<custom_properties::Variables>,
        eligible: Vec<bool>,
        complete: bool,
        overflows: Vec<Option<Overflow>>,
        positions: Vec<Option<Position>>,
        transforms: Vec<Option<Transform>>,
        has_transform_geometry: bool,
        canvas_foregrounds: HashMap<usize, u32>,
    ) -> Self {
        Self(Rc::new(Snapshot {
            variables,
            eligible,
            complete,
            overflows,
            positions,
            transforms,
            has_transform_geometry,
            canvas_foregrounds,
        }))
    }
    pub(crate) fn read(
        &self,
        node: usize,
        name: &str,
    ) -> Result<Option<&str>, ComputedStyleUnavailable> {
        let eligible = self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)?;
        if !eligible {
            return Ok(None);
        }
        if !self.0.complete {
            return Err(ComputedStyleUnavailable);
        }
        self.0
            .variables
            .get(node)
            .ok_or(ComputedStyleUnavailable)?
            .read(name)
    }
    pub(crate) fn read_overflow(
        &self,
        node: usize,
    ) -> Result<Option<Overflow>, ComputedStyleUnavailable> {
        let eligible = self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)?;
        if !eligible {
            return Ok(None);
        }
        if !self.0.complete {
            return Err(ComputedStyleUnavailable);
        }
        self.0
            .overflows
            .get(node)
            .copied()
            .flatten()
            .map(Some)
            .ok_or(ComputedStyleUnavailable)
    }
    pub(crate) fn read_position(
        &self,
        node: usize,
    ) -> Result<Option<Position>, ComputedStyleUnavailable> {
        let eligible = self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)?;
        if !eligible {
            return Ok(None);
        }
        if !self.0.complete {
            return Err(ComputedStyleUnavailable);
        }
        self.0
            .positions
            .get(node)
            .copied()
            .flatten()
            .map(Some)
            .ok_or(ComputedStyleUnavailable)
    }
    pub(crate) fn read_transform(
        &self,
        node: usize,
    ) -> Result<Option<Transform>, ComputedStyleUnavailable> {
        let eligible = self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)?;
        if !eligible {
            return Ok(None);
        }
        if !self.0.complete {
            return Err(ComputedStyleUnavailable);
        }
        if self.0.transforms.is_empty() {
            return Ok(Some(Transform::None));
        }
        self.0.transforms.get(node).copied().flatten()
            .map(Some).ok_or(ComputedStyleUnavailable)
    }
    pub(crate) fn has_transform_geometry(&self) -> bool {
        self.0.has_transform_geometry
    }
    pub(crate) fn canvas_foreground(&self, node: usize) -> Result<Option<u32>, ComputedStyleUnavailable> {
        if !self.0.complete { return Err(ComputedStyleUnavailable); }
        Ok(self.0.canvas_foregrounds.get(&node).copied())
    }
    #[cfg(test)]
    pub(super) fn transform_storage_len(&self) -> usize { self.0.transforms.len() }
}

pub(crate) struct CascadeOutput {
    pub styles: Vec<Style>,
    pub computed_properties: ComputedProperties,
}

/// Borrow the current script job counters, including on an unsuccessful read.
pub(crate) struct CssBudget<'a> {
    host_work: &'a mut usize,
    copy_work: &'a mut usize,
}
impl<'a> CssBudget<'a> {
    pub(super) fn canvas_foreground(&mut self, first: bool) -> Result<(), String> {
        // Include sparse hash buckets, their control bytes, and initial capacity.
        let entries = if first { 6 } else { 3 };
        self.work(1)?;
        self.copies(entries * std::mem::size_of::<(usize, u32)>())
    }

    pub(crate) fn new(host_work: &'a mut usize, copy_work: &'a mut usize) -> Self {
        Self {
            host_work,
            copy_work,
        }
    }
    pub(crate) fn work(&mut self, amount: usize) -> Result<(), String> {
        match self.host_work.checked_sub(amount) {
            Some(left) => {
                *self.host_work = left;
                Ok(())
            }
            None => {
                *self.host_work = 0;
                Err("CSS computation work limit reached.".into())
            }
        }
    }
    pub(crate) fn copies(&mut self, amount: usize) -> Result<(), String> {
        match self.copy_work.checked_sub(amount) {
            Some(left) => {
                *self.copy_work = left;
                Ok(())
            }
            None => {
                *self.copy_work = 0;
                Err("CSS computation copy limit reached.".into())
            }
        }
    }
    pub(crate) fn remaining_work(&self) -> usize {
        *self.host_work
    }

    pub(crate) fn remaining_copies(&self) -> usize {
        *self.copy_work
    }

    // Parsing is finite: selectors have at most 16 alternatives / 12 parts,
    // custom syntax has at most 32 nested fallback levels, and each declaration
    // is at most 2048 bytes. One parsing work unit covers at most 64 inspected
    // bytes; reserve 512 such passes plus 64 copied/allocated bytes per input
    // byte before admission and parsing. This
    // covers both admission and parse passes, token validation and their Vec /
    // String storage; selector matching and variable expansion are charged
    // separately below. Refused complete sources pay for inspection too.
    pub(super) fn source(&mut self, bytes: usize) -> Result<(), String> {
        self.work(bytes.max(1).div_ceil(64).saturating_mul(512))?;
        self.copies(bytes.saturating_mul(64))
    }
    pub(super) fn document(
        &mut self,
        doc: &Document,
        external: &HashMap<usize, String>,
    ) -> Result<(), String> {
        self.work(doc.nodes.len().saturating_mul(12))?;
        self.copies(doc.nodes.len().saturating_mul(
            std::mem::size_of::<Style>()
                + 128
                + std::mem::size_of::<Option<Position>>()
                + std::mem::size_of::<bool>(),
        ))?;
        for node in &doc.nodes {
            self.work(node.children.len())?;
            if let NodeKind::Element { tag, attrs, .. } = &node.kind {
                self.work(attrs.len())?;
                for attr in attrs {
                    self.work(
                        attr.name
                            .bytes()
                            .saturating_add(attr.value.len())
                            .max(1)
                            .div_ceil(64),
                    )?;
                }
                if tag == "style" {
                    for &child in &node.children {
                        if let Some(crate::html::Node {
                            kind: NodeKind::Text(text),
                            ..
                        }) = doc.nodes.get(child)
                        {
                            self.source(text.len())?;
                        }
                    }
                }
            }
        }
        for source in external.values() {
            self.source(source.len())?;
        }
        Ok(())
    }
    pub(super) fn parent(&mut self, parent: &Style) -> Result<(), String> {
        let bytes = parent
            .font_families
            .iter()
            .fold(0usize, |n, family| {
                n.saturating_add(family.len() + std::mem::size_of::<String>())
            })
            .saturating_add(
                parent
                    .background_image
                    .as_ref()
                    .map_or(0, |image| image.url.len()),
            );
        self.work(bytes.max(1).div_ceil(64))?;
        self.copies(bytes.saturating_mul(3))
    }
    pub(super) fn variables<T>(
        &mut self,
        budget: &mut custom_properties::Budget,
        run: impl FnOnce(&mut custom_properties::Budget) -> T,
    ) -> Result<T, String> {
        let old = budget.remaining();
        let job_work = *self.host_work;
        let job_copies = *self.copy_work;
        let mut operation = budget.operation(job_work, job_copies);
        let result = run(&mut operation);
        let (work, copies, failed_work, failed_copies) = operation.usage();
        self.work(work)?;
        self.copies(copies)?;
        budget.commit_operation(&operation);
        // Preserve the renderer's independent finite allowance. Failed work
        // does not fabricate copy usage, and a successful exact debit is valid.
        if failed_work && job_work <= old.0 || failed_copies && job_copies <= old.1 {
            return Err("CSS computation job limit reached.".into());
        }
        Ok(result)
    }
}
