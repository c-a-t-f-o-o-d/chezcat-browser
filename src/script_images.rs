//! Document-owned image request identities. Pixels come only from the loader;
//! the reflected src and the current decoded request are deliberately separate.
use super::*;
use std::collections::VecDeque;

pub(crate) const MAX_IMAGE_REQUESTS: usize = 4096;

#[derive(Clone, Debug)]
pub struct DynamicImage {
    pub node: usize,
    pub generation: u64,
    pub source: String,
    pub base: crate::navigation::Location,
    pub origin: crate::navigation::Location,
    pub policy: crate::csp::PolicySet,
}

#[derive(Default)]
pub(super) struct ImageEntry {
    generation: u64,
    selected: String,
    pub current: String,
    pub pending: bool,
    pub available: bool,
}

#[derive(Default)]
pub(super) struct ImagesState {
    pub entries: HashMap<usize, ImageEntry>,
    pending: VecDeque<DynamicImage>,
    changed: HashSet<usize>,
    removed: HashSet<usize>,
    attempts: usize,
}

pub(super) struct ImageChange { generation: u64, selected: String, request: Option<DynamicImage> }

impl ImagesState {
    pub(super) fn has_changes(&self) -> bool { !self.changed.is_empty() || !self.removed.is_empty() || !self.pending.is_empty() }
}

impl Page {
    pub fn set_initial_images(&mut self, sources: HashMap<usize, String>, present: &HashMap<usize, std::sync::Arc<crate::image::Bitmap>>, pending: bool, enabled: bool) {
        for (node, source) in sources {
            let available = present.contains_key(&node);
            self.state.images.entries.insert(node, ImageEntry {
                selected: source.clone(),
                current: if enabled { source } else { String::new() },
                pending: pending && !available,
                available,
                ..ImageEntry::default()
            });
        }
    }

    pub fn initial_image_current(&self, node: usize) -> bool {
        self.state.images.entries.get(&node).is_none_or(|entry| entry.generation == 0)
    }

    pub fn settle_initial_image(&mut self, node: usize, success: bool) -> bool {
        if !self.initial_image_current(node) { return false; }
        if let Some(entry) = self.state.images.entries.get_mut(&node) {
            entry.pending = false;
            entry.available = success;
            entry.current = entry.selected.clone();
        }
        true
    }

    pub fn take_image_changes(&mut self) -> (HashSet<usize>, HashSet<usize>, VecDeque<DynamicImage>) {
        (std::mem::take(&mut self.state.images.changed), std::mem::take(&mut self.state.images.removed), std::mem::take(&mut self.state.images.pending))
    }

    pub fn current_image_request(&self, request: &DynamicImage) -> bool {
        self.state.images.entries.get(&request.node).is_some_and(|entry| entry.pending && entry.generation == request.generation)
    }

    pub fn settle_dynamic_image(&mut self, request: &DynamicImage, success: bool) -> bool {
        if !self.current_image_request(request) { return false; }
        let entry = self.state.images.entries.get_mut(&request.node).unwrap();
        entry.pending = false;
        entry.available = success;
        entry.current = entry.selected.clone();
        true
    }

    pub fn finish_image_event(&mut self, doc: &mut Document, forms: &mut Forms, node: usize, success: bool) -> Outcome {
        self.begin_job();
        let mut outcome = self.dispatch(doc, forms, node, if success { "load" } else { "error" }, false);
        if self.state.dynamic.load_ready {
            let loading = self.finish_loading(doc, forms);
            outcome.layout_changed |= loading.layout_changed;
            outcome.errors += loading.errors;
        }
        outcome
    }

    pub fn finish_image_changes(&mut self, doc: &mut Document, forms: &mut Forms) -> Outcome {
        if self.state.dynamic.load_ready { self.finish_loading(doc, forms) }
        else { Outcome::default() }
    }

    pub(super) fn clear_image_requests(&mut self) {
        self.state.images.pending.clear();
        self.state.images.changed.clear();
        for entry in self.state.images.entries.values_mut() {
            // Stop leaves the displayed resource intact and invalidates any
            // abandoned completion without manufacturing an error/load event.
            entry.pending = false;
            entry.generation = entry.generation.saturating_add(1);
        }
    }
}

impl Bridge<'_> {
    pub(super) fn validate_plain_image(&mut self, node: usize) -> Result<(), String> {
        self.state.rebuild(self.doc)?;
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html
            || self.state.dom.connected[node] && !self.state.tree.active[node]
            || self.doc.attr(node, "srcset").is_some()
            || self.state.tree.parents[node].is_some_and(|parent| tag(self.doc, parent) == "picture") {
            return Err("Dynamic image source updates for foreign, srcset or picture selection are not implemented.".into());
        }
        Ok(())
    }

    pub(super) fn image_source_change(&mut self, node: usize, value: Option<String>) -> Result<(), String> {
        self.state.rebuild(self.doc)?;
        let eligible = self.state.dom.connected[node] || self.state.images.entries.contains_key(&node);
        if !eligible {
            self.attribute(node, "src", value)?;
            self.state.declined_images = self.state.declined_images.saturating_add(1);
            return Ok(());
        }
        self.validate_plain_image(node)?;
        let change = self.prepare_image_change(node, value.as_deref())?;
        self.attribute(node, "src", value)?;
        self.commit_image_change(node, change);
        Ok(())
    }

    pub(super) fn prepare_image_connections(&mut self, nodes: &[usize], connected: bool) -> Result<Vec<(usize, ImageChange)>, String> {
        if !connected { return Ok(Vec::new()); }
        let candidates: Vec<_> = nodes.iter().copied().filter(|node| tag(self.doc, *node) == "img" && !self.state.images.entries.contains_key(node)).collect();
        let attempts = candidates.iter().filter(|&&node| self.doc.attr(node, "src").is_some()).count();
        self.reserve_image_changes(candidates.len(), attempts)?;
        let mut plans = Vec::new();
        plans.try_reserve(candidates.len()).map_err(|_| "Image connection reservation failed.")?;
        for node in candidates {
            self.validate_plain_image(node)?;
            let source = self.doc.attr(node, "src").map(str::to_owned);
            plans.push((node, self.prepare_image_change(node, source.as_deref())?));
        }
        Ok(plans)
    }

    pub(super) fn reserve_image_changes(&mut self, count: usize, attempts: usize) -> Result<(), String> {
        // Each committed candidate also scans the candidates queued before it.
        let batch_work = count.checked_mul(count.saturating_sub(1)).ok_or("Image connection work overflow.")? / 2;
        self.state.work(batch_work)?;
        if self.state.images.attempts.saturating_add(attempts) > MAX_IMAGE_REQUESTS { return Err("Dynamic image attempt limit reached.".into()); }
        self.state.images.entries.try_reserve(count).map_err(|_| "Image entry reservation failed.")?;
        self.state.images.changed.try_reserve(count).map_err(|_| "Image change reservation failed.")?;
        self.state.images.removed.try_reserve(count).map_err(|_| "Image removal reservation failed.")?;
        self.state.images.pending.try_reserve(count).map_err(|_| "Image request reservation failed.")?;
        Ok(())
    }

    pub(super) fn commit_image_connections(&mut self, plans: Vec<(usize, ImageChange)>) {
        for (node, plan) in plans { self.commit_image_change(node, plan); }
    }

    pub(super) fn prepare_image_change(&mut self, node: usize, source: Option<&str>) -> Result<ImageChange, String> {
        self.state.work(self.state.images.pending.len())?;
        let generation = self.state.images.entries.get(&node).map_or(Some(1), |entry| entry.generation.checked_add(1)).ok_or("Image request generation exhausted.")?;
        if source.is_some() && self.state.images.attempts >= MAX_IMAGE_REQUESTS {
            return Err(format!("Dynamic image attempt limit reached ({MAX_IMAGE_REQUESTS} per document)."));
        }
        self.state.copies(source.map_or(0, str::len).saturating_mul(4).saturating_add(self.state.dynamic.policy.clone_work_bytes()))?;
        self.state.images.entries.try_reserve(1).map_err(|_| "Image entry reservation failed.")?;
        self.state.images.changed.try_reserve(1).map_err(|_| "Image change reservation failed.")?;
        self.state.images.removed.try_reserve(1).map_err(|_| "Image removal reservation failed.")?;
        self.state.images.pending.try_reserve(1).map_err(|_| "Image request reservation failed.")?;
        let origin = self.state.location.as_ref().map(|location| location.source.clone()).unwrap_or_else(crate::navigation::Location::home);
        let base = self.state.dynamic.base.as_ref().unwrap_or(&origin).clone();
        let selected = source.map(|source| {
            if source.trim().is_empty() { String::new() }
            else if crate::image_data::is_data(source) { source.to_owned() }
            else { crate::navigation::resolve(source, &base).map(|target| target.display()).unwrap_or_else(|_| source.to_owned()) }
        }).unwrap_or_default();
        let request = source.map(|source| DynamicImage { node, generation, source: source.to_owned(), base, origin, policy: self.state.dynamic.policy.clone() });
        Ok(ImageChange { generation, selected, request })
    }

    fn commit_image_change(&mut self, node: usize, change: ImageChange) {
        let entry = self.state.images.entries.entry(node).or_default();
        entry.generation = change.generation;
        entry.selected = change.selected;
        entry.pending = change.request.is_some();
        if !entry.available || change.request.is_none() { entry.current = entry.selected.clone(); }
        if change.request.is_none() {
            entry.available = false;
            self.state.images.removed.insert(node);
            if let Some(geometry) = &mut self.state.geometry { geometry.set_image(node, None); }
        }
        self.state.images.changed.insert(node);
        self.state.images.pending.retain(|request| request.node != node);
        if let Some(request) = change.request {
            self.state.images.attempts += 1;
            self.state.images.pending.push_back(request);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_nodes_image_attempt_failure_preserves_identity_payload_and_next_job_recovers() {
        let (mut doc,mut forms,mut page,node)=fixture();
        let outcome=page.console_eval(&mut doc,&mut forms,"var candidate=document.createAttribute('src');candidate.value='/two.ppm';");assert_eq!(outcome.errors,0,"{}",page.report());
        page.state.images.attempts=MAX_IMAGE_REQUESTS;let retained=page.state.retained;let commits=page.state.dom_commits;let count=page.state.attributes.len();
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.call(node_handle(node),"setAttributeNode",&[Value::Host(script_attr::ATTR_BASE)]).unwrap_err();
        assert!(error.contains("image attempt"),"{error}");assert_eq!(doc.attr(node,"src"),Some("/one.ppm"));assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.attributes.len(),count);assert_eq!(page.state.attributes[0].owner,None);assert_eq!(page.state.attributes[0].value,"/two.ppm");assert!(!page.state.images.has_changes());
        page.state.images.attempts=0;page.begin_job();
        let outcome=page.console_eval(&mut doc,&mut forms,"var old=document.getElementById('i').setAttributeNode(candidate);if(old.value!=='/one.ppm'||old.ownerElement!==null||candidate.ownerElement!==document.getElementById('i'))throw 'recovery';");assert_eq!(outcome.errors,0,"{}",page.report());
        assert_eq!(page.take_image_changes().2.len(),1);
    }

    fn fixture() -> (Document, Forms, Page, usize) {
        let mut doc = crate::html::parse("<html><head></head><body><img id=i src='/one.ppm'><picture id=p></picture></body></html>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        page.set_location(&crate::navigation::address("https://example.test/index", &crate::navigation::Location::home()).unwrap());
        page.start(&mut doc, &mut forms, &[]);
        let node = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("i")).unwrap();
        (doc, forms, page, node)
    }

    #[test]
    fn image_attribute_failure_publishes_no_request_or_generation() {
        let (mut doc, mut forms, mut page, node) = fixture();
        page.state.retained_limit = page.state.retained;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let error = bridge.image_source_change(node, Some("/a-much-longer-image-source.ppm".into())).unwrap_err();
        assert!(error.contains("DOM retained"), "{error}");
        assert_eq!(bridge.doc.attr(node, "src"), Some("/one.ppm"));
        assert!(bridge.state.images.entries.is_empty());
        assert!(!bridge.state.images.has_changes());
        assert_eq!(bridge.state.images.attempts, 0);
    }

    #[test]
    fn image_connection_preflight_preserves_tree_on_attempt_failure() {
        let (mut doc, mut forms, mut page, _) = fixture();
        let body = page.state.tree.body.unwrap();
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let Value::Host(handle) = bridge.create_image(&[]).unwrap() else { unreachable!() };
        let node = node_index(handle).unwrap();
        bridge.image_source_change(node, Some("/two.ppm".into())).unwrap();
        bridge.state.images.attempts = MAX_IMAGE_REQUESTS;
        let children = bridge.doc.nodes[body].children.clone();
        let error = bridge.dom_call(node_handle(body), "appendChild", &[Value::Host(handle)]).unwrap_err();
        assert!(error.contains("image attempt"), "{error}");
        assert_eq!(bridge.doc.nodes[body].children, children);
        assert_eq!(bridge.state.tree.parents[node], None);
        assert!(!bridge.state.dom.connected[node]);
        assert!(!bridge.state.images.has_changes());
        assert!(bridge.state.images.entries.is_empty());
    }

    #[test]
    fn image_picture_and_inert_boundaries_reject_before_effects() {
        let (mut doc, mut forms, mut page, node) = fixture();
        let picture = (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("p")).unwrap();
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let old_parent = bridge.state.tree.parents[node];
        assert!(bridge.dom_call(node_handle(picture), "appendChild", &[Value::Host(node_handle(node))]).unwrap_err().contains("picture"));
        assert_eq!(bridge.state.tree.parents[node], old_parent);
        bridge.doc.nodes[node].namespace = crate::html::Namespace::UnknownForeign;
        assert!(bridge.image_source_change(node, Some("/forbidden.ppm".into())).is_err());
        bridge.doc.nodes[node].namespace = crate::html::Namespace::Html;
        bridge.state.tree.active[node] = false;
        assert!(bridge.image_source_change(node, Some("/forbidden.ppm".into())).is_err());
        assert_eq!(bridge.doc.attr(node, "src"), Some("/one.ppm"));
        assert!(!bridge.state.images.has_changes());
    }

#[test]
fn replace_child_image_preparation_failure_preserves_both_parents_and_recovers() {
    let mut doc=crate::html::parse("<div id=p><span id=old>old</span></div><div id=q><img id=incoming src='/original-image.ppm'></div>");
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);
    let mut page=Page::for_test();
    let outcome=page.start(&mut doc,&mut forms,&[]);
    assert_eq!(outcome.errors,0);
    let node=|id:&str|doc.nodes.iter().enumerate().find_map(|(node,_)|(doc.attr(node,"id")==Some(id)).then_some(node)).unwrap();
    let (parent,source,old,incoming)=(node("p"),node("q"),node("old"),node("incoming"));
    let before=(doc.nodes[parent].children.clone(),doc.nodes[source].children.clone(),page.state.dom_commits);
    let attempts=page.state.images.attempts;
    page.state.images.attempts=usize::MAX; // Exhaustion/overflow fixture, no large allocation.
    let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.call(node_handle(parent),"replaceChild",&[Value::Host(node_handle(incoming)),Value::Host(node_handle(old))]).unwrap_err();
    assert!(error.contains("image attempt"),"{error}");
    assert_eq!((doc.nodes[parent].children.clone(),doc.nodes[source].children.clone(),page.state.dom_commits),before);
    assert_eq!(page.state.tree.parents[old],Some(parent));
    assert_eq!(page.state.tree.parents[incoming],Some(source));
    assert!(page.state.dom.connected[old]&&page.state.dom.connected[incoming]);
    assert!(!page.state.images.has_changes()&&page.state.images.entries.is_empty());
    assert_eq!(page.state.images.attempts,usize::MAX);
    page.state.images.attempts=attempts;
    page.begin_job();
    let result=page.console_eval(&mut doc,&mut forms,"if(document.getElementById('p').replaceChild(document.getElementById('incoming'),document.getElementById('old')).id!=='old')throw 'recovery';");
    assert_eq!(result.errors,0,"{}",page.report());
    assert_eq!(doc.nodes[parent].children,[incoming]);
    assert_eq!(page.take_image_changes().2.len(),1);
}

}
