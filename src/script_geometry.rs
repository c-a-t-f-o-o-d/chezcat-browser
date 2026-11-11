//! Read-only CSSOM client dimensions from the same layout used for painting.
//! A script may force a few synchronous layouts after mutations; repeated reads
//! reuse measurements. No guessed screen dimensions or external renderer.
use std::collections::HashMap;

use crate::fonts::Book;
use crate::forms::{Forms, Kind};
use crate::html::Document;
use crate::layout::{self, ClientBox, Layout};

const MAX_LAYOUTS_PER_JOB: usize = 4;

pub struct Geometry {
    width: i32,
    height: i32,
    zoom: f32,
    styles: HashMap<usize, String>,
    images: HashMap<usize, (usize, usize)>,
    fonts: Book,
    boxes: Option<Vec<Option<ClientBox>>>,
    remaining: usize,
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
            remaining: MAX_LAYOUTS_PER_JOB,
        }
    }

    pub fn begin_job(&mut self) {
        self.remaining = MAX_LAYOUTS_PER_JOB;
    }

    pub fn image_dimensions(&self, node: usize) -> Option<(usize, usize)> {
        self.images.get(&node).copied()
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
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read(
        &mut self,
        doc: &Document,
        forms: &Forms,
        node: usize,
        is_root: bool,
        property: &str,
        host_work: &mut usize,
    ) -> Result<f64, String> {
        if self.boxes.is_none() {
            if self.remaining == 0 {
                return Err("DOM geometry layout limit reached (4 rebuilds per job).".into());
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
        }
        let client = self
            .boxes
            .as_ref()
            .and_then(|boxes| boxes.get(node))
            .copied()
            .flatten();
        let Some(client) = client else {
            return Ok(0.0);
        };
        let pixels = match property {
            "clientWidth" if is_root => self.width,
            "clientHeight" if is_root => self.height,
            "clientWidth" => client.width,
            "clientHeight" => client.height,
            "clientLeft" => client.left,
            "clientTop" => client.top,
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
