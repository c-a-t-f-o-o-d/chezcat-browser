//! Environment-driven picture/srcset requests on the existing image worker.
//! The initial document loader settles first. Displayed pixels and their density
//! remain a single resource until a replacement has decoded and fits the page.
use super::*;
use crate::image_selection::{Environment, Selector};
use crate::scripting::DynamicImage;
use std::collections::VecDeque;

const SELECTION_WORK: usize = 64 * 1024 * 1024;
const REQUEST_COPY_BYTES: usize = 16 * 1024 * 1024;

struct Displayed {
    source: String,
    density: f64,
    failed: Option<String>,
}

struct Pending {
    generation: u64,
    source: String,
    density: f64,
}

pub(super) struct State {
    nodes: Vec<usize>,
    displayed: HashMap<usize, Displayed>,
    requests: HashMap<usize, Pending>,
    loads: VecDeque<(usize, u64)>,
    environment: [u64; 3],
    attempts: usize,
    generation: u64,
    selection_work: usize,
    request_bytes: usize,
}

fn environment_key(environment: Environment) -> [u64; 3] {
    [
        environment.viewport_width.to_bits(),
        environment.viewport_height.to_bits(),
        environment.density.to_bits(),
    ]
}

impl Default for State {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            displayed: HashMap::new(),
            requests: HashMap::new(),
            loads: VecDeque::new(),
            environment: [0; 3],
            attempts: 0,
            generation: 0,
            selection_work: SELECTION_WORK,
            request_bytes: REQUEST_COPY_BYTES,
        }
    }
}

impl State {
    pub(super) fn new(
        nodes: Vec<usize>,
        environment: Environment,
        sources: &HashMap<usize, String>,
        densities: &HashMap<usize, f64>,
    ) -> Self {
        let displayed = nodes
            .iter()
            .map(|&node| {
                (
                    node,
                    Displayed {
                        source: sources.get(&node).cloned().unwrap_or_default(),
                        density: densities.get(&node).copied().unwrap_or(1.0),
                        failed: None,
                    },
                )
            })
            .collect();
        Self {
            nodes,
            displayed,
            environment: environment_key(environment),
            ..Self::default()
        }
    }

    pub(super) fn pending(&self) -> bool {
        !self.requests.is_empty() || !self.loads.is_empty()
    }
}

/// Conservatively precharge the selector's tree, candidate comparisons and
/// attribute scans. Both per-operation parsing limits and this document total
/// apply; exhaustion keeps the last displayed resource and emits one diagnostic.
fn selection_cost(doc: &Document) -> Option<usize> {
    let mut cost = doc.nodes.len().checked_mul(4)?;
    for item in &doc.nodes {
        cost = cost.checked_add(item.children.len().checked_mul(4)?)?;
        if let html::NodeKind::Element { tag, attrs, .. } = &item.kind {
            if !matches!(tag.as_str(), "img" | "source") {
                continue;
            }
            cost = cost.checked_add(attrs.len().checked_mul(16)?)?;
            for attr in attrs {
                if ["src", "srcset", "sizes", "media", "type"]
                    .iter()
                    .any(|name| attr.name.html_is(name))
                {
                    cost = cost.checked_add(attr.value.len().checked_add(1)?.checked_mul(128)?)?;
                }
            }
        }
    }
    Some(cost)
}

impl App {
    pub(super) fn image_selection_environment(&self) -> Environment {
        Environment {
            viewport_width: f64::from(
                ((self.width - 12).max(1) as f32 / self.zoom)
                    .round()
                    .max(1.0) as u32,
            ),
            viewport_height: f64::from(
                (self.viewport_height().max(1) as f32 / self.zoom)
                    .round()
                    .max(1.0) as u32,
            ),
            density: f64::from((self.zoom * 1000.0).round().max(1.0) as u32) / 1000.0,
        }
    }

    pub(super) fn cancel_responsive_node(&mut self, node: usize) {
        self.cancel_responsive_worker(node);
        self.responsive_images
            .loads
            .retain(|&(target, _)| target != node);
        if let Some(pending) = self.responsive_images.requests.remove(&node) {
            if let Some(page) = &mut self.scripting {
                page.cancel_environment_image(node, pending.generation);
            }
        }
    }

    pub(super) fn invalidate_responsive_source(&mut self, node: usize) {
        if let Some(displayed) = self.responsive_images.displayed.get_mut(&node) {
            // A plain-image mutation may follow a move out of picture. Its
            // replacement pixels must not be mistaken for the old selected URL.
            displayed.source.clear();
            displayed.failed = None;
        }
    }

    pub(super) fn cancel_responsive_images(&mut self) {
        self.responsive_images.loads.clear();
        let requests = std::mem::take(&mut self.responsive_images.requests);
        for (node, pending) in requests {
            if let Some(page) = &mut self.scripting {
                page.cancel_environment_image(node, pending.generation);
            }
        }
        // Stop coalesces the current environment too; polling must not restart
        // the abandoned request until a later actual environment change.
        self.responsive_images.environment = environment_key(self.image_selection_environment());
    }

    pub(super) fn process_responsive_images(&mut self) -> bool {
        if self.pending.is_some()
            || self.resubmission.is_some()
            || !self.images_enabled
            || self.responsive_images.nodes.is_empty()
        {
            return false;
        }
        let environment = self.image_selection_environment();
        let key = environment_key(environment);
        if key == self.responsive_images.environment {
            return false;
        }
        self.responsive_images.environment = key;
        if self.responsive_images.selection_work == 0 {
            return false;
        }
        let Some(cost) = selection_cost(&self.document)
            .filter(|&cost| cost <= self.responsive_images.selection_work)
        else {
            if self.responsive_images.selection_work != 0 {
                self.events.push("RESPONSIVE IMAGE: document selection work allowance exhausted; displayed images retained.".into());
                self.responsive_images.selection_work = 0;
            }
            return false;
        };
        self.responsive_images.selection_work -= cost;
        if let Some(page) = &mut self.scripting {
            if let Err(error) = page.begin_environment_images(&self.document) {
                self.events.push(format!("RESPONSIVE IMAGE: {error}"));
                return false;
            }
        }
        // Parsed no-JS nodes cannot move. With scripting, the existing DOM
        // index supplies actual connection and active-subtree provenance.
        let inert = loader::inert_nodes(&self.document, self.document.scripting_enabled);
        let selector = Selector::new(&self.document, environment);
        let mut choices = Vec::new();
        if choices
            .try_reserve(self.responsive_images.nodes.len())
            .is_err()
        {
            self.events
                .push("RESPONSIVE IMAGE: could not reserve selected choices.".into());
            return false;
        }
        for &node in &self.responsive_images.nodes {
            let eligible = inert.get(node) == Some(&false)
                && self
                    .scripting
                    .as_ref()
                    .is_none_or(|page| page.environment_image_node_active(node))
                && self
                    .document
                    .nodes
                    .get(node)
                    .is_some_and(|item| item.namespace == html::Namespace::Html)
                && selector.environment_sensitive(node);
            choices.push((
                node,
                if eligible {
                    selector.select(node).image
                } else {
                    None
                },
            ));
        }
        drop(selector);
        let mut changed = false;
        let mut geometry_changed = false;
        for (node, choice) in choices {
            let Some(choice) = choice else {
                self.cancel_responsive_node(node);
                self.responsive_images
                    .displayed
                    .get_mut(&node)
                    .unwrap()
                    .failed = None;
                continue;
            };
            let source = if crate::image_data::is_data(&choice.url) {
                choice.url
            } else {
                match navigation::resolve(&choice.url, &self.base_location) {
                    Ok(target) => target.display(),
                    Err(error) => {
                        self.responsive_images
                            .displayed
                            .get_mut(&node)
                            .unwrap()
                            .failed = None;
                        self.events
                            .push(format!("RESPONSIVE IMAGE node {node}: {error}"));
                        continue;
                    }
                }
            };
            // Suppress repeated work for the current failed choice, rather
            // than blacklisting its URL for this document's lifetime. Leaving
            // it also permits a later return to retry a recovered resource.
            let displayed = self.responsive_images.displayed.get_mut(&node).unwrap();
            if displayed
                .failed
                .as_deref()
                .is_some_and(|failed| failed != source)
            {
                displayed.failed = None;
            }
            if self.pending_images.contains_key(&node) {
                self.cancel_responsive_node(node);
                continue;
            }
            let displayed = &self.responsive_images.displayed[&node];
            if displayed.source == source && self.images.contains_key(&node) {
                let density_changed = displayed.density != choice.density;
                if density_changed || self.responsive_images.requests.contains_key(&node) {
                    self.cancel_responsive_node(node);
                }
                self.responsive_images
                    .displayed
                    .get_mut(&node)
                    .unwrap()
                    .failed = None;
                if density_changed {
                    let load = if let Some(page) = &mut self.scripting {
                        if self.responsive_images.loads.try_reserve(1).is_err() {
                            self.events.push(
                                "RESPONSIVE IMAGE: could not reserve a cached-resource load task."
                                    .into(),
                            );
                            continue;
                        }
                        match page.prepare_environment_image(node, &source, false) {
                            Ok(Some(generation)) => {
                                page.settle_environment_image(node, generation, true);
                                Some(generation)
                            }
                            Ok(None) => continue,
                            Err(error) => {
                                self.events
                                    .push(format!("RESPONSIVE IMAGE node {node}: {error}"));
                                continue;
                            }
                        }
                    } else {
                        None
                    };
                    self.image_densities.insert(node, choice.density);
                    self.responsive_images
                        .displayed
                        .get_mut(&node)
                        .unwrap()
                        .density = choice.density;
                    if let Some(generation) = load {
                        self.responsive_images.loads.push_back((node, generation));
                    }
                    geometry_changed = true;
                }
                continue;
            }
            if let Some(pending) = self.responsive_images.requests.get_mut(&node) {
                if pending.source == source {
                    pending.density = choice.density;
                    continue;
                }
            }
            self.cancel_responsive_node(node);
            if self.responsive_images.displayed[&node].failed.as_deref() == Some(source.as_str()) {
                continue;
            }
            let bytes = source
                .len()
                .saturating_mul(3)
                .saturating_add(self.document_policy.clone_work_bytes().saturating_mul(2))
                .saturating_add(self.base_location.display().len().saturating_mul(2))
                .saturating_add(self.location.display().len().saturating_mul(2));
            if self.responsive_images.attempts >= crate::scripting::MAX_IMAGE_REQUESTS
                || bytes > self.responsive_images.request_bytes
            {
                if self.responsive_images.attempts != usize::MAX {
                    self.events.push("RESPONSIVE IMAGE: document request allowance exhausted; displayed images retained.".into());
                    self.responsive_images.attempts = usize::MAX;
                }
                continue;
            }
            if self.responsive_images.requests.try_reserve(1).is_err() {
                self.events
                    .push("RESPONSIVE IMAGE: could not reserve a request.".into());
                continue;
            }
            let generation = if let Some(page) = &mut self.scripting {
                match page.prepare_environment_image(node, &source, true) {
                    Ok(Some(generation)) => generation,
                    Ok(None) => continue,
                    Err(error) => {
                        self.events
                            .push(format!("RESPONSIVE IMAGE node {node}: {error}"));
                        continue;
                    }
                }
            } else {
                let Some(generation) = self.responsive_images.generation.checked_add(1) else {
                    continue;
                };
                self.responsive_images.generation = generation;
                generation
            };
            self.responsive_images.attempts += 1;
            self.responsive_images.request_bytes -= bytes;
            self.responsive_images.requests.insert(
                node,
                Pending {
                    generation,
                    source: source.clone(),
                    density: choice.density,
                },
            );
            let request = DynamicImage {
                node,
                generation,
                source,
                base: self.base_location.clone(),
                origin: self.location.clone(),
                policy: self.document_policy.clone(),
            };
            if let Err(error) = self.submit_responsive_image(request.clone()) {
                self.finish_responsive_image(&request, Err(error));
            }
            changed = true;
        }
        if geometry_changed {
            self.update_script_geometry();
            self.reflow();
        }
        changed || geometry_changed
    }

    pub(super) fn finish_responsive_image(
        &mut self,
        request: &DynamicImage,
        result: Result<crate::image::DecodedImage, String>,
    ) -> bool {
        if !self.responsive_image_current(request) {
            return false;
        }
        let pending = self
            .responsive_images
            .requests
            .remove(&request.node)
            .unwrap();
        if self
            .scripting
            .as_ref()
            .is_some_and(|page| !page.environment_image_current(request.node, request.generation))
        {
            if let Some(page) = &mut self.scripting {
                page.cancel_environment_image(request.node, request.generation);
            }
            return true;
        }
        let result = result.and_then(|mut asset| {
            self.fit_document_image(request.node, &mut asset)?;
            Ok(asset)
        });
        let success = result.is_ok();
        match result {
            Ok(asset) => {
                self.animations.remove(&request.node);
                self.images.insert(request.node, asset.bitmap);
                self.image_densities.insert(request.node, pending.density);
                if let Some(animation) = asset.animation {
                    self.install_animations(HashMap::from([(request.node, animation)]));
                }
                if let Some(notice) = asset.notice {
                    self.events
                        .push(format!("RESPONSIVE IMAGE node {}: {notice}", request.node));
                }
                let displayed = self
                    .responsive_images
                    .displayed
                    .get_mut(&request.node)
                    .unwrap();
                displayed.source = pending.source;
                displayed.density = pending.density;
                displayed.failed = None;
            }
            Err(error) => {
                self.events
                    .push(format!("RESPONSIVE IMAGE node {}: {error}", request.node));
                self.responsive_images
                    .displayed
                    .get_mut(&request.node)
                    .unwrap()
                    .failed = Some(pending.source);
            }
        }
        if let Some(page) = &mut self.scripting {
            page.settle_environment_image(request.node, request.generation, success);
        }
        if success {
            self.update_script_geometry();
            self.reflow();
        }
        if success && self.scripting.is_some() {
            if self.responsive_images.loads.try_reserve(1).is_ok() {
                self.responsive_images
                    .loads
                    .push_back((request.node, request.generation));
            } else {
                self.events
                    .push("RESPONSIVE IMAGE: could not reserve a resource load task.".into());
            }
        }
        self.validate_control_focus();
        self.sync_editors();
        true
    }

    pub(super) fn poll_responsive_image_events(&mut self) -> bool {
        if self.script_jobs_paused() {
            return false;
        }
        let Some((node, generation)) = self.responsive_images.loads.pop_front() else {
            return false;
        };
        if self
            .scripting
            .as_ref()
            .is_some_and(|page| page.environment_image_loaded(node, generation))
        {
            let outcome = self.scripting.as_mut().unwrap().finish_image_event(
                &mut self.document,
                &mut self.forms,
                node,
                true,
            );
            if outcome.layout_changed {
                self.reflow();
            }
            self.resume_background_loading();
            self.show_script_errors();
            self.validate_control_focus();
            self.sync_editors();
        }
        true
    }

    pub(super) fn responsive_image_current(&self, request: &DynamicImage) -> bool {
        self.responsive_images
            .requests
            .get(&request.node)
            .is_some_and(|pending| {
                pending.generation == request.generation && pending.source == request.source
            })
    }
}
