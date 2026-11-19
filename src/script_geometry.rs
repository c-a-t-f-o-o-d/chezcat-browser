//! Read-only CSSOM client dimensions from the same layout used for painting.
//! Scripts may force synchronous layouts after mutations; repeated reads
//! reuse measurements. No guessed screen dimensions or external renderer.
use std::collections::HashMap;
use std::time::Instant;

use crate::fonts::Book;
use crate::forms::{Forms, Kind};
use crate::html::Document;
use crate::layout::{self, ClientBox, Layout};

#[path = "script_geometry_scroll_offsets.rs"]
mod scroll_offsets;
pub(crate) use scroll_offsets::Axis;

#[path = "script_geometry_offset_coordinates.rs"]
mod offset_coordinates;
#[cfg(test)]
#[path="script_geometry_stylesheet_cache_tests.rs"]
mod stylesheet_cache_tests;

fn live_checked(forms: &Forms, node: usize) -> Option<bool> {
    forms.controls.get(&node)
        .filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
        .map(|control| control.checked)
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Limits {pub layouts_per_job:usize}
impl Default for Limits {fn default()->Self{Self {layouts_per_job:1024}}}

pub struct Geometry {
    execution_deadline: Option<Instant>,
    width: i32,
    height: i32,
    zoom: f32,
    styles: HashMap<usize, String>,
    images: HashMap<usize, (usize, usize)>,
    fonts: Book,
    boxes: Option<Vec<Option<ClientBox>>>,
    computed_properties: Option<crate::css::ComputedProperties>,
    stylesheet_cache: Option<crate::css::StylesheetCache>,
    scroll_heights:Vec<Option<i32>>,
    viewport_scroll_height:i32,
    viewport_scroll_unavailable:bool,
    incomplete_grid_nodes: Vec<bool>,
    rects: Vec<Option<layout::BorderRect>>,
    visual_rects: Vec<Option<layout::VisualBorderRect>>,
    visual_unavailable: Vec<bool>,
    transform_translations: Vec<Option<[f64; 2]>>,
    scroll: i32,
    viewport_max_scroll: i32,
    pending_scroll: Option<i32>,
    offset_unavailable: Vec<bool>,
    offset_parents: Vec<layout::OffsetParent>,
    remaining: usize,
    limits:Limits,
}

impl Geometry {
    pub fn new(
        width: i32,
        height: i32,
        zoom: f32,
        styles: &HashMap<usize, String>,
        images: HashMap<usize, (usize, usize)>,
        fonts: &Book,
    ) -> Self {
        Self {
            execution_deadline: None,
            width: width.clamp(1, 16_384),
            height: height.clamp(1, 16_384),
            zoom: valid_zoom(zoom),
            styles: styles.clone(),
            images,
            fonts: fonts.clone(),
            boxes: None,
            computed_properties: None,
            stylesheet_cache: None,
            scroll_heights:Vec::new(),
            viewport_scroll_height:0,
            viewport_scroll_unavailable:false,
            incomplete_grid_nodes: Vec::new(),
            rects: Vec::new(),
            visual_rects: Vec::new(),
            visual_unavailable: Vec::new(),
            transform_translations: Vec::new(),
            scroll: 0,
            viewport_max_scroll: 0,
            pending_scroll: None,
            offset_unavailable: Vec::new(),
            offset_parents: Vec::new(),
            remaining: Limits::default().layouts_per_job,
            limits:Limits::default(),
        }
    }

    pub fn begin_job(&mut self) {
        self.remaining = self.limits.layouts_per_job;
    }

    pub(crate) fn set_execution_deadline(&mut self, deadline: Instant) {
        self.execution_deadline = Some(deadline);
    }
    pub fn with_limits(mut self,limits:Limits)->Self{self.limits=limits;self.remaining=limits.layouts_per_job;self}

    pub fn set_scroll(&mut self, scroll: i32) { self.scroll = scroll.max(0); }

    pub(crate) fn take_scroll_request(&mut self) -> Option<i32> { self.pending_scroll.take() }

    fn update_scroll_range(&mut self, layout_height: i32) {
        self.viewport_max_scroll = layout_height.saturating_sub(self.height).max(0);
        if self.scroll > self.viewport_max_scroll {
            self.scroll = self.viewport_max_scroll;
            self.pending_scroll = Some(self.scroll);
        }
    }

    pub fn css_scroll_y(&self) -> f64 { f64::from(self.scroll)/f64::from(self.zoom) }

    pub fn image_dimensions(&self, node: usize) -> Option<(usize, usize)> {
        self.images.get(&node).copied()
    }

    pub fn set_image(&mut self, node: usize, dimensions: Option<(usize, usize)>) {
        if let Some(dimensions) = dimensions { self.images.insert(node, dimensions); }
        else { self.images.remove(&node); }
        self.invalidate();
    }

    pub fn set_stylesheet(&mut self, node: usize, source: Option<&str>) {
        if let Some(source) = source {
            self.styles.insert(node, source.to_owned());
        } else {
            self.styles.remove(&node);
        }
        self.invalidate();
    }

    pub fn invalidate(&mut self) {
        self.boxes = None;
        self.computed_properties = None;
    }

    /// The document keeps running while a resource/source report is visible.
    /// Update its own context without adopting the report page's measurements.
    pub fn update(
        &mut self,
        width: i32,
        height: i32,
        zoom: f32,
        images: HashMap<usize, (usize, usize)>,
    ) {
        let width = width.clamp(1, 16_384);
        let height = height.clamp(1, 16_384);
        let zoom = valid_zoom(zoom);
        if (self.width, self.height, self.zoom) != (width, height, zoom) || self.images != images {
            self.invalidate();
        }
        self.width = width;
        self.height = height;
        self.zoom = zoom;
        self.images = images;
    }

    /// App has just laid out this document. Reuse that work, including updated
    /// image sizes, window dimensions, form state, and browser zoom.
    pub fn sync(
        &mut self,
        width: i32,
        height: i32,
        zoom: f32,
        images: HashMap<usize, (usize, usize)>,
        layout: &Layout,
    ) {
        self.update(width, height, zoom, images);
        self.update_scroll_range(layout.height);
        self.boxes = Some(layout.client_boxes.clone());
        self.computed_properties = Some(layout.computed_properties.clone());
        self.stylesheet_cache = layout.stylesheet_cache.clone();
        self.scroll_heights=layout.scroll_heights.clone();
        self.viewport_scroll_height=layout.viewport_scroll_height;
        self.viewport_scroll_unavailable=layout.viewport_scroll_unavailable;
        self.offset_unavailable = layout.offset_unavailable.clone();
        self.incomplete_grid_nodes.clone_from(&layout.incomplete_grid_nodes);
        self.offset_parents.clone_from(&layout.offset_parents);
        self.rects = layout.border_rects.clone();
        self.visual_rects.clone_from(&layout.visual_border_rects);
        self.visual_unavailable.clone_from(&layout.visual_unavailable);
        self.transform_translations.clone_from(&layout.transform_translations);
    }

    fn ensure_layout(&mut self, doc: &Document, forms: &Forms, host_work: &mut usize, copy_work: &mut usize) -> Result<(), String> {
        if self.boxes.is_none() {
            if self.remaining == 0 {
                return Err(format!("DOM geometry layout limit reached ({} rebuilds per job).",self.limits.layouts_per_job));
            }
            if doc.nodes.len() > *host_work {
                *host_work = 0;
                return Err("DOM work limit reached during geometry layout.".into());
            }
            *host_work -= doc.nodes.len();
            let mut budget = crate::css::CssBudget::new(host_work, copy_work)
                .with_deadline(self.execution_deadline);
            budget.work(layout::offset_parent_work(doc))?;
            budget.copies(layout::offset_parent_storage(doc.nodes.len()))?;
            let checked = |node| live_checked(forms, node);
            let layout = layout::build_viewport_with_fonts_budgeted_cached(
                doc,
                self.width,
                self.height,
                self.zoom,
                &self.styles,
                &self.images,
                Some(&checked),
                &self.fonts,
                &mut budget,
                self.stylesheet_cache.as_ref(),
            )?;
            self.remaining -= 1;
            self.update_scroll_range(layout.height);
            self.boxes = Some(layout.client_boxes);
            self.computed_properties = Some(layout.computed_properties);
            self.stylesheet_cache = layout.stylesheet_cache;
            self.scroll_heights=layout.scroll_heights;
            self.viewport_scroll_height=layout.viewport_scroll_height;
            self.viewport_scroll_unavailable=layout.viewport_scroll_unavailable;
            self.offset_unavailable = layout.offset_unavailable;
            self.incomplete_grid_nodes = layout.incomplete_grid_nodes;
            self.offset_parents = layout.offset_parents;
            self.rects = layout.border_rects;
            self.visual_rects = layout.visual_border_rects;
            self.visual_unavailable = layout.visual_unavailable;
            self.transform_translations = layout.transform_translations;
        }
        Ok(())
    }

    pub(crate) fn computed_snapshot(
        &mut self,
        doc: &Document,
        forms: &Forms,
        host_work: &mut usize,
        copy_work: &mut usize,
    ) -> Result<&crate::css::ComputedProperties, String> {
        if self.computed_properties.is_none() {
            if self.remaining == 0 {
                return Err(format!("DOM geometry layout limit reached ({} rebuilds per job).", self.limits.layouts_per_job));
            }
            self.remaining -= 1;
            let environment = layout::viewport_style_environment(doc, self.width, self.height, self.zoom);
            let checked = |node| live_checked(forms, node);
            let mut budget = crate::css::CssBudget::new(host_work, copy_work)
                .with_deadline(self.execution_deadline);
            let cascade = crate::css::compute_with_environment_and_properties_budgeted_cached(
                doc, &self.styles, Some(&checked), Some(environment), &mut budget, self.stylesheet_cache.as_ref(),
            )?;
            self.computed_properties = Some(cascade.computed_properties);
            self.stylesheet_cache = cascade.stylesheet_cache;
        }
        *host_work = host_work.checked_sub(1).ok_or("DOM work limit reached during computed style lookup.")?;
        Ok(self.computed_properties.as_ref().expect("computed cascade was installed"))
    }

    pub(crate) fn computed_custom_property(
        &mut self,
        doc: &Document,
        forms: &Forms,
        node: usize,
        name: &str,
        host_work: &mut usize,
        copy_work: &mut usize,
    ) -> Result<Option<String>, String> {
        let snapshot = self.computed_snapshot(doc, forms, host_work, copy_work)?;
        let Some(value) = snapshot.read(node, name)
            .map_err(|_| "Computed style unavailable within the CSS cascade limits.")?
        else {
            return Ok(None);
        };
        if value.len() > *copy_work {
            *copy_work = 0;
            return Err("DOM string work limit reached while copying a computed property.".into());
        }
        *copy_work -= value.len();
        Ok(Some(value.to_owned()))
    }

    pub(crate) fn offset_parent(&mut self, doc: &Document, forms: &Forms, node: usize, work: &mut usize, copies: &mut usize) -> Result<Option<usize>, String> {
        self.ensure_layout(doc, forms, work, copies)?;
        match self.offset_parents.get(node) {
            Some(layout::OffsetParent::Element(parent)) => Ok(Some(*parent)),
            Some(layout::OffsetParent::None) => Ok(None),
            _ => Err("offsetParent requires complete box and containing-block information.".into()),
        }
    }

    pub fn bounding_rect(&mut self, doc: &Document, forms: &Forms, node: usize, host_work: &mut usize, copy_work: &mut usize) -> Result<[f64; 4], String> {
        self.ensure_layout(doc, forms, host_work, copy_work)?;
        if !self.visual_rects.is_empty() {
            if self.visual_unavailable.get(node).copied().unwrap_or(true) {
                return Err(format!("Visual bounding rectangle requires a measured transform and border box. Receiver node={node}."));
            }
            if let Some(rect) = self.visual_rects.get(node).copied().flatten() {
                let zoom = f64::from(self.zoom);
                let y = if rect.fixed { rect.y } else { rect.y - f64::from(self.scroll) };
                return Ok([rect.x / zoom, y / zoom, rect.width / zoom, rect.height / zoom]);
            }
            return Ok([0.0; 4]);
        } else if let Some(rect) = self.rects.get(node).copied().flatten() {
            let zoom = f64::from(self.zoom);
            let y = if rect.fixed { f64::from(rect.y) } else { f64::from(rect.y) - f64::from(self.scroll) };
            return Ok([f64::from(rect.x) / zoom, y / zoom, f64::from(rect.width) / zoom, f64::from(rect.height) / zoom]);
        }
        if self.offset_unavailable.get(node).copied().unwrap_or(true) {
            return Err(format!("Bounding rectangle requires unmeasured inline, replaced, table-row or unsupported layout fragments. Receiver node={node}."));
        }
        Ok([0.0; 4])
    }

    pub(crate) fn resolved_transform(
        &mut self, doc: &Document, forms: &Forms, node: usize,
        host_work: &mut usize, copy_work: &mut usize,
    ) -> Result<Option<[f64; 2]>, String> {
        if self.computed_properties.is_none() {
            self.ensure_layout(doc, forms, host_work, copy_work)?;
        }
        let snapshot = self.computed_properties.as_ref().ok_or("Missing computed style snapshot.")?;
        match snapshot.read_transform(node) {
            Ok(Some(crate::css::Transform::None)) => return Ok(None),
            Ok(Some(crate::css::Transform::Translate {
                x: crate::css::Length::Px(x), y: crate::css::Length::Px(y),
            })) => return Ok(Some([f64::from(x), f64::from(y)])),
            Ok(Some(crate::css::Transform::Translate { .. })) => {}
            _ => return Err("Computed transform unavailable within the CSS cascade limits.".into()),
        }
        self.ensure_layout(doc, forms, host_work, copy_work)?;
        self.transform_translations.get(node).copied().flatten().map(Some)
            .ok_or_else(|| "Computed translation requires a measured reference box.".into())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read(
        &mut self, doc: &Document, forms: &Forms, node: usize, is_root: bool,
        property: &str, host_work: &mut usize, copy_work: &mut usize,
    ) -> Result<f64, String> {
        self.ensure_layout(doc, forms, host_work, copy_work)?;
        if property=="scrollHeight" {
            if is_root &&self.viewport_scroll_unavailable{return Err("Root scroll height requires unsupported viewport overflow propagation.".into());}
            if self.scroll_heights.get(node).copied().flatten().is_none() && self.boxes.as_ref().and_then(|boxes|boxes.get(node)).is_some_and(Option::is_some) {return Err(format!("Scroll height requires unsupported scrolling-area origin geometry. Receiver node={node}."));}
            let pixels=if is_root {self.height.max(self.viewport_scroll_height)}
                else if let Some(height)=self.scroll_heights.get(node).copied().flatten(){height}
                else if self.offset_unavailable.get(node).copied().unwrap_or(true){return Err(format!("Scroll height requires unmeasured layout fragments. Receiver node={node}."));}
                else {0};
            return Ok((f64::from(pixels)/f64::from(self.zoom)).round());
        }

        if self.incomplete_grid_nodes.get(node) == Some(&true) {
            return Err(format!("Client and offset dimensions require completed grid tracks. Receiver node={node}."));
        }
        let client = self
            .boxes
            .as_ref()
            .and_then(|boxes| boxes.get(node))
            .copied()
            .flatten();
        let Some(client) = client else {
            if matches!(property,"offsetWidth"|"offsetHeight") && self.offset_unavailable.get(node).copied().unwrap_or(true) {
                return Err(format!("Offset dimension {property} requires unmeasured inline or unsupported layout fragments. Receiver node={node}."));
            }
            return Ok(0.0);
        };
        let pixels = match property {
            "clientWidth" if is_root => self.width,
            "clientHeight" if is_root => self.height,
            "clientWidth" => client.width,
            "clientHeight" => client.height,
            "clientLeft" => client.left,
            "clientTop" => client.top,
            "offsetWidth" => client.width+client.left+client.right,
            "offsetHeight" => client.height+client.top+client.bottom,
            _ => return Err("Unknown client geometry property.".into()),
        };
        // Layout stores integer surface pixels. Convert back to CSS pixels;
        // small lengths at fractional zoom retain the renderer's rounding.
        Ok((f64::from(pixels) / f64::from(self.zoom)).round())
    }
}

fn valid_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(0.25, 5.0)
    } else {
        1.0
    }
}

#[cfg(test)]
#[test]
fn offset_parent_budget_refusal_preserves_the_available_rebuild() {
    let doc = crate::html::parse("<html><body><div id=outer style='position:relative'><button id=b>B</button></div></body></html>");
    let forms = Forms::new(&doc);
    let id = |name| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(name)).unwrap();
    let mut geometry = Geometry::new(640, 480, 1.0, &HashMap::new(), HashMap::new(), &Book::empty())
        .with_limits(Limits { layouts_per_job: 1 });
    let work_required = doc.nodes.len() + layout::offset_parent_work(&doc);
    let bytes_required = layout::offset_parent_storage(doc.nodes.len());
    let read = |geometry: &mut Geometry, work: &mut usize, copies: &mut usize| {
        geometry.offset_parent(&doc, &forms, id("b"), work, copies)
    };
    for (mut work, mut copies) in [(work_required - 1, bytes_required), (work_required, bytes_required - 1)] {
        assert!(read(&mut geometry, &mut work, &mut copies).is_err());
        assert!(geometry.boxes.is_none());
        assert_eq!(geometry.remaining, 1);
    }
    let (mut work, mut copies) = (work_required, bytes_required);
    assert_eq!(read(&mut geometry, &mut work, &mut copies).unwrap(), Some(id("outer")));
    assert_eq!((work, copies), (0, 0));
    assert_eq!(read(&mut geometry, &mut work, &mut copies).unwrap(), Some(id("outer")));
    assert_eq!(geometry.remaining, 0);
}

#[cfg(test)]
#[test]
fn translated_geometry_budget_refusal_preserves_the_rebuild_and_scroll_state() {
    let doc = crate::html::parse("<html><body style='margin:0'><div id='box' style='width:80px;height:40px;transform:translate(25%,50%)'></div></body></html>");
    let node = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("box")).unwrap();
    let forms = Forms::new(&doc);
    let fresh = || Geometry::new(320, 200, 1.0, &HashMap::new(), HashMap::new(), &Book::empty())
        .with_limits(Limits { layouts_per_job: 1 });
    let read = |g: &mut Geometry, work: &mut usize, copies: &mut usize| {
        g.bounding_rect(&doc, &forms, node, work, copies)
    };
    let mut probe = fresh();
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    assert_eq!(read(&mut probe, &mut work, &mut copies).unwrap(), [20.0, 20.0, 80.0, 40.0]);
    let (needed_work, needed_copies) = (1_000_000 - work, 1_000_000 - copies);
    for (mut work, mut copies) in [(needed_work - 1, needed_copies), (needed_work, needed_copies - 1)] {
        let mut geometry = fresh();
        geometry.set_scroll(17);
        assert!(read(&mut geometry, &mut work, &mut copies).is_err());
        assert!(geometry.boxes.is_none() && geometry.computed_properties.is_none());
        assert!(geometry.visual_rects.is_empty() && geometry.transform_translations.is_empty());
        assert_eq!((geometry.remaining, geometry.scroll, geometry.pending_scroll), (1, 17, None));
        let (mut work, mut copies) = (needed_work, needed_copies);
        assert_eq!(read(&mut geometry, &mut work, &mut copies).unwrap(), [20.0, 20.0, 80.0, 40.0]);
        assert_eq!((work, copies, geometry.remaining, geometry.scroll), (0, 0, 0, 0));
        assert_eq!(geometry.take_scroll_request(), Some(0));
        assert_eq!(read(&mut geometry, &mut work, &mut copies).unwrap(), [20.0, 20.0, 80.0, 40.0]);
    }
}

#[cfg(test)]
#[test]
fn computed_style_geometry_shares_rebuilds_and_only_charges_the_returned_value_on_cache_hits() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet href=theme.css><div id=box></div>");
    let forms = Forms::new(&doc);
    let id = |name| (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap();
    let node = id("box");
    let sheet = id("sheet");
    let sheets = HashMap::from([(sheet, "#box{--token:ready;width:42px}".to_owned())]);
    let mut geometry = Geometry::new(640, 480, 1.0, &sheets, HashMap::new(), &Book::empty())
        .with_limits(Limits { layouts_per_job: 1 });
    let (mut work, mut copies) = (1_000_000, 1_000_000);
    let read = |geometry: &mut Geometry, work: &mut usize, copies: &mut usize| {
        geometry.computed_custom_property(&doc, &forms, node, "--token", work, copies)
    };
    assert_eq!(read(&mut geometry, &mut work, &mut copies).unwrap().as_deref(), Some("ready"));
    assert!(geometry.boxes.is_none());
    assert_eq!(geometry.remaining, 0);
    let (mut lookup, mut exact_copy) = (1, 5);
    assert_eq!(read(&mut geometry, &mut lookup, &mut exact_copy).unwrap().as_deref(), Some("ready"));
    assert_eq!((lookup, exact_copy), (0, 0));
    assert!(geometry.read(&doc, &forms, node, false, "clientWidth", &mut work, &mut copies).is_err());

    geometry.set_stylesheet(sheet, Some("#box{--token:changed;width:65px}"));
    assert!(read(&mut geometry, &mut work, &mut copies).is_err());
    assert!(geometry.computed_properties.is_none());
    geometry.begin_job();
    assert_eq!(geometry.read(&doc, &forms, node, false, "clientWidth", &mut work, &mut copies).unwrap(), 65.0);
    assert_eq!(geometry.remaining, 0);
    assert_eq!(read(&mut geometry, &mut work, &mut copies).unwrap().as_deref(), Some("changed"));
}
