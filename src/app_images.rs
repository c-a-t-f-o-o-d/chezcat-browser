//! Actual, cancelable image fetch/decode work. Only the App thread commits
//! resources and emits events, after checking document and request identities.
use super::*;
use crate::scripting::DynamicImage;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[cfg(test)]
#[path = "app_image_tests.rs"]
mod tests;

struct Job {
    epoch: u64,
    background: bool,
    request: DynamicImage,
    blocking: Arc<crate::blocking::Policy>,
    network: crate::network_trace::Trace,
    cancel: Arc<AtomicBool>,
    enabled: bool,
}

struct Completion {
    epoch: u64,
    background: bool,
    request: DynamicImage,
    result: Result<crate::image::DecodedImage, String>,
    events: Vec<String>,
}

struct Shared {
    queue: VecDeque<Job>,
    completed: VecDeque<Completion>,
    active: Option<(usize, bool, Arc<AtomicBool>)>,
    stopping: bool,
    wake: loader::Wake,
}

pub(super) struct Worker { state: Arc<(Mutex<Shared>, Condvar)> }

impl Worker {
    fn new(wake: loader::Wake, trust: Arc<crate::trust::Store>, cookies: Arc<crate::cookies::Session>) -> Result<Self, String> {
        let state = Arc::new((Mutex::new(Shared { queue: VecDeque::new(), completed: VecDeque::new(), active: None, stopping: false, wake }), Condvar::new()));
        let shared = state.clone();
        std::thread::Builder::new().name("image-loader".into()).spawn(move || loop {
            let job = {
                let mut guard = shared.0.lock().unwrap();
                while guard.queue.is_empty() && !guard.stopping { guard = shared.1.wait(guard).unwrap(); }
                if guard.stopping { return; }
                let job = guard.queue.pop_front().unwrap();
                guard.active = Some((job.request.node, job.background, job.cancel.clone()));
                job
            };
            let mut events = Vec::new();
            let mut result = fetch(&job, &trust, cookies.clone(), &mut events);
            let wake = {
                let mut guard = shared.0.lock().unwrap();
                guard.active = None;
                if guard.stopping { return; }
                if !job.cancel.load(Ordering::Relaxed) {
                    // Completed assets are separately bounded while the App is
                    // busy. Each individual decode has the decoder's own bound.
                    let used = guard.completed.iter().filter_map(|completion| completion.result.as_ref().ok()).try_fold(0usize, |used, asset| asset.retained_bytes().ok().and_then(|bytes| used.checked_add(bytes))).unwrap_or(usize::MAX);
                    result = result.and_then(|mut asset| { loader::fit_image_budget(&mut asset, used)?; Ok(asset) });
                    guard.completed.push_back(Completion { epoch: job.epoch, background: job.background, request: job.request, result, events });
                }
                guard.wake.clone()
            };
            wake();
        }).map_err(|error| format!("Cannot start image loader: {error}"))?;
        Ok(Self { state })
    }

    fn submit(&self, job: Job) -> Result<(), String> {
        let mut state = self.state.0.lock().unwrap();
        if state.stopping || state.queue.len() + state.completed.len() >= crate::scripting::MAX_IMAGE_REQUESTS {
            return Err("Dynamic image worker queue limit reached.".into());
        }
        // Reserve the admitted job and its future completion before accepting
        // ownership. The worker never needs an infallible completion growth.
        let completions = state.queue.len().checked_add(2).ok_or("Image worker accounting overflow.")?;
        state.completed.try_reserve(completions).map_err(|_| "Cannot reserve image completions.")?;
        state.queue.try_reserve(1).map_err(|_| "Cannot reserve image worker queue.")?;
        state.queue.push_back(job);
        self.state.1.notify_one();
        Ok(())
    }

    pub(super) fn set_wake(&self, wake: loader::Wake) { self.state.0.lock().unwrap().wake = wake; }
    fn poll(&self, backgrounds_only: bool) -> Option<Completion> {
        let (result, wake) = {
            let mut state = self.state.0.lock().unwrap();
            // CSS image publication is independent of scripting. Leave element
            // completions queued while script jobs are paused, preserving their
            // original FIFO when ordinary polling resumes.
            let result = if backgrounds_only {
                state.completed.iter().position(|completion| completion.background)
                    .and_then(|index| state.completed.remove(index))
            } else { state.completed.pop_front() };
            let more = if backgrounds_only { state.completed.iter().any(|completion| completion.background) }
                else { !state.completed.is_empty() };
            (result, more.then(|| state.wake.clone()))
        };
        if let Some(wake) = wake { wake(); }
        result
    }

    fn cancel_node(&self, node: usize, background: bool) {
        let mut state = self.state.0.lock().unwrap();
        if let Some((active, slot, cancel)) = &state.active { if *active == node && *slot == background { cancel.store(true, Ordering::Relaxed); } }
        state.queue.retain(|job| job.request.node != node || job.background != background);
        state.completed.retain(|completion| completion.request.node != node || completion.background != background);
    }

    pub(super) fn cancel(&self) {
        let mut state = self.state.0.lock().unwrap();
        if let Some((_, _, cancel)) = &state.active { cancel.store(true, Ordering::Relaxed); }
        state.queue.clear();
        state.completed.clear();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        self.state.0.lock().unwrap().stopping = true;
        self.state.1.notify_one();
    }
}

fn fetch(job: &Job, trust: &crate::trust::Store, cookies: Arc<crate::cookies::Session>, events: &mut Vec<String>) -> Result<crate::image::DecodedImage, String> {
    let started = std::time::Instant::now();
    let check = || {
        if job.cancel.load(Ordering::Relaxed) { Err("Image request canceled.".into()) }
        else if started.elapsed() > std::time::Duration::from_secs(45) { Err("Image request time budget reached (45 seconds).".into()) }
        else { Ok(()) }
    };
    check()?;
    if !job.enabled { return Err("Images disabled by user.".into()); }
    if job.request.source.trim().is_empty() { return Err("Empty image source.".into()); }
    let bytes = if crate::image_data::is_data(&job.request.source) {
        job.request.policy.allows_embedded_image(usize::MAX)?;
        crate::image_data::prepare(&job.request.source)?.bytes(&check)?
    } else {
        let target = navigation::resolve(&job.request.source, &job.request.base)?;
        job.request.policy.allows_image(usize::MAX, &target)?;
        if let Some(url) = &target.url {
            let origin = job.request.origin.url.as_ref().ok_or("Local pages do not automatically fetch network images.")?;
            if !origin.same_origin(url) { return Err("Cross-origin dynamic image request blocked.".into()); }
            let filter = |url: &crate::url::HttpUrl| {
                check()?;
                job.blocking.check(url)?;
                job.request.policy.allows_image(usize::MAX, &Location { url: Some(url.clone()), path: None, root: None, fragment: String::new() })
            };
            let pool = crate::http::Pool::new(&job.cancel, trust).with_cookie_session(cookies, Some(origin.clone())).with_network_trace(job.network.clone()).with_resource_filter(&filter);
            let response = pool.fetch_kind(url, 8 * 1024 * 1024, Some(origin), crate::http::FetchMode::Normal, if job.background { "BACKGROUND IMAGE" } else { "IMAGE" })?;
            events.extend(response.events);
            if !(200..300).contains(&response.status) { return Err(format!("HTTP {} while fetching a {} image.", response.status, if job.background { "background" } else { "dynamic" })); }
            response.body
        } else {
            if job.request.origin.url.is_some() { return Err("A web document cannot load local files.".into()); }
            navigation::read_limited(target.path.as_ref().ok_or("Image has no local file path.")?, 32 * 1024 * 1024)?
        }
    };
    check()?;
    let mut asset = crate::image::decode_asset_checked(&bytes, &check)?;
    loader::fit_image_budget(&mut asset, 0)?;
    events.push(format!("{} IMAGE node {}: decoded {} x {} pixels", if job.background { "BACKGROUND" } else { "DYNAMIC" }, job.request.node, asset.bitmap.width, asset.bitmap.height));
    Ok(asset)
}

impl App {
    pub(super) fn submit_background_image(&mut self, request: DynamicImage) -> Result<(), String> {
        if self.dynamic_images.is_none() {
            self.dynamic_images = Some(Worker::new(self.wake.clone(), self.trust.clone(), self.cookies.clone())?);
        }
        let job = Job { epoch: self.script_epoch, background: true, request, blocking: Arc::new(crate::blocking::Policy::new(&self.profile.data().settings.blocking)), network: self.network.clone(), cancel: Arc::new(AtomicBool::new(false)), enabled: self.images_enabled };
        self.dynamic_images.as_ref().unwrap().submit(job)
    }

    pub(super) fn cancel_background_image(&self, node: usize) {
        if let Some(worker) = &self.dynamic_images { worker.cancel_node(node, true); }
    }

    pub(super) fn cancel_image_requests(&mut self) {
        if let Some(worker) = &self.dynamic_images { worker.cancel(); }
        self.pending_images.clear();
        let removed = self.scripting.as_mut().map(|page| page.take_image_changes().1).unwrap_or_default();
        let changed = !removed.is_empty();
        for node in removed { self.images.remove(&node); self.animations.remove(&node); self.image_densities.remove(&node); }
        if changed { self.update_script_geometry(); self.reflow(); }
    }

    pub(super) fn process_image_requests(&mut self) -> bool {
        if self.script_jobs_paused() { return false; }
        let Some(page) = &mut self.scripting else { return false; };
        let (changed, removed, requests) = page.take_image_changes();
        let any = !changed.is_empty();
        for node in changed {
            if let Some(worker) = &self.dynamic_images { worker.cancel_node(node, false); }
            self.pending_images.remove(&node);
        }
        let restyle = !removed.is_empty();
        for node in removed { self.images.remove(&node); self.animations.remove(&node); self.image_densities.remove(&node); }
        if restyle { self.update_script_geometry(); self.reflow(); }
        for request in requests {
            if self.dynamic_images.is_none() {
                match Worker::new(self.wake.clone(), self.trust.clone(), self.cookies.clone()) {
                    Ok(worker) => self.dynamic_images = Some(worker),
                    Err(error) => { self.finish_dynamic_image(&request, Err(error)); continue; }
                }
            }
            let node = request.node;
            let generation = request.generation;
            let job = Job { epoch: self.script_epoch, background: false, request: request.clone(), blocking: Arc::new(crate::blocking::Policy::new(&self.profile.data().settings.blocking)), network: self.network.clone(), cancel: Arc::new(AtomicBool::new(false)), enabled: self.images_enabled };
            match self.dynamic_images.as_ref().unwrap().submit(job) {
                Ok(()) => { self.pending_images.insert(node, generation); }
                Err(error) => self.finish_dynamic_image(&request, Err(error)),
            }
        }
        if let Some(page) = &mut self.scripting {
            let outcome = page.finish_image_changes(&mut self.document, &mut self.forms);
            if outcome.layout_changed { self.reflow(); }
        }
        any
    }

    pub(super) fn poll_dynamic_images(&mut self) -> bool {
        let backgrounds_only = self.script_jobs_paused();
        let Some(completion) = self.dynamic_images.as_ref().and_then(|worker| worker.poll(backgrounds_only)) else { return false; };
        if completion.epoch != self.script_epoch { return false; }
        if completion.background {
            self.events.extend(completion.events);
            self.finish_background_image(&completion.request, completion.result);
            return true;
        }
        if self.pending_images.get(&completion.request.node) != Some(&completion.request.generation) { return false; }
        self.pending_images.remove(&completion.request.node);
        if self.scripting.as_ref().is_none_or(|page| !page.current_image_request(&completion.request)) { return false; }
        self.events.extend(completion.events);
        self.finish_dynamic_image(&completion.request, completion.result);
        true
    }

    fn finish_dynamic_image(&mut self, request: &DynamicImage, result: Result<crate::image::DecodedImage, String>) {
        if self.scripting.as_ref().is_none_or(|page| !page.current_image_request(request)) { return; }
        let result = result.and_then(|mut asset| {
            self.fit_document_image(request.node, &mut asset)?;
            Ok(asset)
        });
        let success = result.is_ok();
        self.animations.remove(&request.node);
        match result {
            Ok(asset) => {
                self.images.insert(request.node, asset.bitmap);
                self.image_densities.insert(request.node, 1.0);
                if let Some(animation) = asset.animation { self.install_animations(HashMap::from([(request.node, animation)])); }
                if let Some(notice) = asset.notice { self.events.push(format!("DYNAMIC IMAGE node {}: {notice}", request.node)); }
            }
            Err(error) => {
                self.images.remove(&request.node);
                self.image_densities.remove(&request.node);
                self.events.push(format!("DYNAMIC IMAGE node {}: {error}", request.node));
            }
        }
        self.update_script_geometry();
        self.reflow();
        let page = self.scripting.as_mut().unwrap();
        page.settle_dynamic_image(request, success);
        let outcome = page.finish_image_event(&mut self.document, &mut self.forms, request.node, success);
        if outcome.layout_changed { self.reflow(); }
        // Finish this owned resource task before consuming its queued navigation.
        self.resume_background_loading();
        self.validate_control_focus();
        self.sync_editors();
        self.show_script_errors();
        if self.view == View::Resources { self.refresh_resource_view(); }
    }
}
