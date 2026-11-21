//! Shared, immutable computed-property readback from the rendering cascade.
use super::*;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComputedStyleUnavailable;

#[derive(Clone)]
pub(crate) struct ComputedPresentation {pub display:&'static str,pub color:u32,pub visible:bool,pub pointer_events:bool,pub z_index:Option<Rc<z_index::Value>>}
pub(crate) fn display_css(value:Display)->&'static str{match value{Display::None=>"none",Display::Block=>"block",Display::Inline=>"inline",Display::InlineBlock=>"inline-block",Display::InlineFlex=>"inline-flex",Display::Flex=>"flex",Display::Grid=>"grid",Display::Table=>"table",Display::TableRowGroup=>"table-row-group",Display::TableRow=>"table-row",Display::TableCell=>"table-cell"}}

pub(super) fn style_display(style:&Style)->&'static str{match (style.display,style.display_alias){(Display::Block,Some("list-item"))=>"list-item",(Display::TableRowGroup,Some("table-header-group"))=>"table-header-group",(Display::TableRowGroup,Some("table-footer-group"))=>"table-footer-group",_=>display_css(style.display)}}

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
    opacities: HashMap<usize, Option<f64>>,
    touch_actions: HashMap<usize, Option<TouchAction>>,
    font_sizes: HashMap<usize, Option<f32>>,
    presentation:Vec<ComputedPresentation>,
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
        opacities: HashMap<usize, Option<f64>>,
        touch_actions: HashMap<usize, Option<TouchAction>>,
        font_sizes: HashMap<usize, Option<f32>>,
        presentation:Vec<ComputedPresentation>,
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
            opacities,
            touch_actions,
            font_sizes,
            presentation,
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
    pub(crate) fn has_presentation(&self)->bool{self.0.presentation.len()==self.0.eligible.len()}
    pub(crate) fn read_presentation(&self,node:usize)->Result<Option<ComputedPresentation>,ComputedStyleUnavailable>{
        if !*self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)?{return Ok(None);}
        if !self.0.complete{return Err(ComputedStyleUnavailable);}
        self.0.presentation.get(node).cloned().map(Some).ok_or(ComputedStyleUnavailable)
    }
    pub(crate) fn canvas_foreground(&self, node: usize) -> Result<Option<u32>, ComputedStyleUnavailable> {
        if !self.0.complete { return Err(ComputedStyleUnavailable); }
        Ok(self.0.canvas_foregrounds.get(&node).copied())
    }
    pub(crate) fn read_opacity(&self, node: usize) -> Result<Option<f64>, ComputedStyleUnavailable> {
        let eligible = self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)?;
        if !eligible { return Ok(None); }
        if !self.0.complete { return Err(ComputedStyleUnavailable); }
        self.0.opacities.get(&node).copied().unwrap_or(Some(1.0))
            .map(Some).ok_or(ComputedStyleUnavailable)
    }
    pub(crate) fn read_font_size(&self,node:usize)->Result<Option<f32>,ComputedStyleUnavailable> {
        if !*self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)? {return Ok(None);}
        if !self.0.complete {return Err(ComputedStyleUnavailable);}
        self.0.font_sizes.get(&node).copied().unwrap_or(Some(INITIAL_FONT_SIZE)).map(Some).ok_or(ComputedStyleUnavailable)
    }
    pub(crate) fn read_touch_action(&self,node:usize)->Result<Option<TouchAction>,ComputedStyleUnavailable> {
        let eligible=*self.0.eligible.get(node).ok_or(ComputedStyleUnavailable)?;
        if !eligible {return Ok(None);}
        if !self.0.complete {return Err(ComputedStyleUnavailable);}
        self.0.touch_actions.get(&node).copied().unwrap_or(Some(TouchAction::AUTO)).map(Some).ok_or(ComputedStyleUnavailable)
    }
    pub(crate) fn has_opacity_paint(&self) -> bool { !self.0.opacities.is_empty() }
    #[cfg(test)]
    pub(super) fn transform_storage_len(&self) -> usize { self.0.transforms.len() }
}

pub(crate) struct CascadeOutput {
    pub has_hover:bool,
    pub styles: Vec<Style>,
    pub computed_properties: ComputedProperties,
    pub stylesheet_cache: Option<StylesheetCache>,
}

/// Borrow the current script job counters, including on an unsuccessful read.
pub(crate) struct CssBudget<'a> {
    host_work: &'a mut usize,
    copy_work: &'a mut usize,
    deadline: Option<Instant>,
    next_clock_check: usize,
}

pub(super) fn check_css_deadline(deadline: Option<Instant>) -> Result<(), String> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        Err("CSS computation time limit reached.".into())
    } else {
        Ok(())
    }
}

impl<'a> CssBudget<'a> {
    pub(super) fn presentation(&mut self,count:usize)->Result<(),String>{self.work(count)?;self.copies(count.checked_mul(std::mem::size_of::<ComputedPresentation>()).ok_or("Computed presentation backing overflow.")?)}
    pub(super) fn font_size(&mut self,first:bool)->Result<(),String> {
        self.work(1)?;self.copies((if first {6} else {3})*std::mem::size_of::<(usize,Option<f32>)>())
    }
    pub(super) fn touch_action(&mut self,first:bool)->Result<(),String> {
        self.work(1)?;
        self.copies((if first {6} else {3})*std::mem::size_of::<(usize,Option<TouchAction>)>())
    }
    pub(super) fn opacity(&mut self, first: bool) -> Result<(), String> {
        // Sparse hash buckets/control bytes, including the first allocation.
        let entries = if first { 6 } else { 3 };
        self.work(1)?;
        self.copies(entries * std::mem::size_of::<(usize, Option<f64>)>())
    }
    pub(super) fn canvas_foreground(&mut self, first: bool) -> Result<(), String> {
        // Include sparse hash buckets, their control bytes, and initial capacity.
        let entries = if first { 6 } else { 3 };
        self.work(1)?;
        self.copies(entries * std::mem::size_of::<(usize, u32)>())
    }

    pub(crate) fn new(host_work: &'a mut usize, copy_work: &'a mut usize) -> Self {
        let next_clock_check = *host_work;
        Self {
            host_work,
            copy_work,
            deadline: None,
            next_clock_check,
        }
    }

    pub(crate) fn with_deadline(mut self, deadline: Option<Instant>) -> Self {
        self.deadline = deadline;
        self
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub(crate) fn work(&mut self, amount: usize) -> Result<(), String> {
        match self.host_work.checked_sub(amount) {
            Some(left) => {
                *self.host_work = left;
                if self.deadline.is_some() && left <= self.next_clock_check {
                    check_css_deadline(self.deadline)?;
                    self.next_clock_check = left.saturating_sub(4096);
                }
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
    pub(crate) fn source(&mut self, bytes: usize) -> Result<(), String> {
        self.work(bytes.max(1).div_ceil(SOURCE_WORK_CHUNK).saturating_mul(SOURCE_WORK_PER_CHUNK))?;
        self.copies(bytes.saturating_mul(64))
    }
    pub(super) fn document_structure(&mut self, doc: &Document) -> Result<(), String> {
        self.work(doc.nodes.len().saturating_mul(12))?;
        self.copies(doc.nodes.len().saturating_mul(
            std::mem::size_of::<Style>()
                + 128
                + std::mem::size_of::<Option<Position>>()
                + std::mem::size_of::<bool>(),
        ))?;
        for node in &doc.nodes {
            self.work(node.children.len())?;
            if let NodeKind::Element { attrs, .. } = &node.kind {
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
            }
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
