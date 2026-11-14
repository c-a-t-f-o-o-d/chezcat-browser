//! Read-only CSSOM client dimensions from the same layout used for painting.
//! Scripts may force synchronous layouts after mutations; repeated reads
//! reuse measurements. No guessed screen dimensions or external renderer.
use std::collections::HashMap;

use crate::fonts::Book;
use crate::forms::{Forms, Kind};
use crate::html::Document;
use crate::layout::{self, ClientBox, Layout};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Limits {pub layouts_per_job:usize}
impl Default for Limits {fn default()->Self{Self {layouts_per_job:1024}}}

pub struct Geometry {
    width: i32,
    height: i32,
    zoom: f32,
    styles: HashMap<usize, String>,
    images: HashMap<usize, (usize, usize)>,
    fonts: Book,
    boxes: Option<Vec<Option<ClientBox>>>,
    scroll_heights:Vec<Option<i32>>,
    viewport_scroll_height:i32,
    viewport_scroll_unavailable:bool,
    rects: Vec<Option<layout::BorderRect>>,
    scroll: i32,
    offset_unavailable: Vec<bool>,
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
            width: width.clamp(1, 16_384),
            height: height.clamp(1, 16_384),
            zoom: valid_zoom(zoom),
            styles: styles.clone(),
            images,
            fonts: fonts.clone(),
            boxes: None,
            scroll_heights:Vec::new(),
            viewport_scroll_height:0,
            viewport_scroll_unavailable:false,
            rects: Vec::new(),
            scroll: 0,
            offset_unavailable: Vec::new(),
            remaining: Limits::default().layouts_per_job,
            limits:Limits::default(),
        }
    }

    pub fn begin_job(&mut self) {
        self.remaining = self.limits.layouts_per_job;
    }
    pub fn with_limits(mut self,limits:Limits)->Self{self.limits=limits;self.remaining=limits.layouts_per_job;self}

    pub fn set_scroll(&mut self, scroll: i32) { self.scroll = scroll.max(0); }

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
        self.boxes = Some(layout.client_boxes.clone());
        self.scroll_heights=layout.scroll_heights.clone();
        self.viewport_scroll_height=layout.viewport_scroll_height;
        self.viewport_scroll_unavailable=layout.viewport_scroll_unavailable;
        self.offset_unavailable = layout.offset_unavailable.clone();
        self.rects = layout.border_rects.clone();
    }

    fn ensure_layout(&mut self, doc: &Document, forms: &Forms, host_work: &mut usize) -> Result<(), String> {
        if self.boxes.is_none() {
            if self.remaining == 0 {
                return Err(format!("DOM geometry layout limit reached ({} rebuilds per job).",self.limits.layouts_per_job));
            }
            if doc.nodes.len() > *host_work {
                *host_work = 0;
                return Err("DOM work limit reached during geometry layout.".into());
            }
            self.remaining -= 1;
            *host_work -= doc.nodes.len();
            let checked = |node| {
                forms
                    .controls
                    .get(&node)
                    .filter(|control| matches!(control.kind, Kind::Checkbox | Kind::Radio))
                    .map(|control| control.checked)
            };
            let layout = layout::build_viewport_with_fonts(
                doc,
                self.width,
                self.height,
                self.zoom,
                &self.styles,
                &self.images,
                Some(&checked),
                &self.fonts,
            );
            self.boxes = Some(layout.client_boxes);
            self.scroll_heights=layout.scroll_heights;
            self.viewport_scroll_height=layout.viewport_scroll_height;
            self.viewport_scroll_unavailable=layout.viewport_scroll_unavailable;
            self.offset_unavailable = layout.offset_unavailable;
            self.rects = layout.border_rects;
        }
        Ok(())
    }

    pub fn bounding_rect(&mut self, doc: &Document, forms: &Forms, node: usize, host_work: &mut usize) -> Result<[f64; 4], String> {
        self.ensure_layout(doc, forms, host_work)?;
        if let Some(rect) = self.rects.get(node).copied().flatten() {
            let zoom = f64::from(self.zoom);
            let y = if rect.fixed { f64::from(rect.y) } else { f64::from(rect.y) - f64::from(self.scroll) };
            return Ok([f64::from(rect.x) / zoom, y / zoom, f64::from(rect.width) / zoom, f64::from(rect.height) / zoom]);
        }
        if self.offset_unavailable.get(node).copied().unwrap_or(true) {
            return Err(format!("Bounding rectangle requires unmeasured inline, replaced, table-row or unsupported layout fragments. Receiver node={node}."));
        }
        Ok([0.0; 4])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read(
        &mut self, doc: &Document, forms: &Forms, node: usize, is_root: bool,
        property: &str, host_work: &mut usize,
    ) -> Result<f64, String> {
        self.ensure_layout(doc, forms, host_work)?;
        if property=="scrollHeight" {
            if is_root &&self.viewport_scroll_unavailable{return Err("Root scroll height requires unsupported viewport overflow propagation.".into());}
            if self.scroll_heights.get(node).copied().flatten().is_none() && self.boxes.as_ref().and_then(|boxes|boxes.get(node)).is_some_and(Option::is_some) {return Err(format!("Scroll height requires unsupported scrolling-area origin geometry. Receiver node={node}."));}
            let pixels=if is_root {self.height.max(self.viewport_scroll_height)}
                else if let Some(height)=self.scroll_heights.get(node).copied().flatten(){height}
                else if self.offset_unavailable.get(node).copied().unwrap_or(true){return Err(format!("Scroll height requires unmeasured layout fragments. Receiver node={node}."));}
                else {0};
            return Ok((f64::from(pixels)/f64::from(self.zoom)).round());
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
