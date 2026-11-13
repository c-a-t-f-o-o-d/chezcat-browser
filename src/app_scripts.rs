//! App-owned execution of document requests. JavaScript never holds a network
//! socket or destroys its own realm. One cancelable worker fetches bounded
//! external classic scripts; completions return to the normal App thread.

use super::*;
use crate::scripting::DynamicScript;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

struct Job {
    epoch: u64,
    request: DynamicScript,
    blocking: Arc<crate::blocking::Policy>,
    network: crate::network_trace::Trace,
    cancel: Arc<AtomicBool>,
}

struct Completion {
    epoch: u64,
    request: DynamicScript,
    result: Result<loader::Script, String>,
    events: Vec<String>,
}

struct Shared {
    queue: VecDeque<Job>,
    completed: VecDeque<Completion>,
    active: Option<Arc<AtomicBool>>,
    stopping: bool,
    wake: loader::Wake,
}

pub(super) struct Worker {
    state: Arc<(Mutex<Shared>, Condvar)>,
}

impl Worker {
    fn new(
        wake: loader::Wake,
        trust: Arc<crate::trust::Store>,
        cookies: Arc<crate::cookies::Session>,
    ) -> Result<Self, String> {
        let state = Arc::new((
            Mutex::new(Shared {
                queue: VecDeque::new(),
                completed: VecDeque::new(),
                active: None,
                stopping: false,
                wake,
            }),
            Condvar::new(),
        ));
        let shared = state.clone();
        std::thread::Builder::new()
            .name("script-loader".into())
            .spawn(move || loop {
                let job = {
                    let mut guard = shared.0.lock().unwrap();
                    while guard.queue.is_empty() && !guard.stopping {
                        guard = shared.1.wait(guard).unwrap();
                    }
                    if guard.stopping {
                        return;
                    }
                    let job = guard.queue.pop_front().unwrap();
                    guard.active = Some(job.cancel.clone());
                    job
                };
                let mut events = Vec::new();
                let result = fetch(&job, &trust, cookies.clone(), &mut events);
                let wake = {
                    let mut guard = shared.0.lock().unwrap();
                    guard.active = None;
                    if guard.stopping {
                        return;
                    }
                    if !job.cancel.load(Ordering::Relaxed) {
                        guard.completed.push_back(Completion {
                            epoch: job.epoch,
                            request: job.request,
                            result,
                            events,
                        });
                    }
                    guard.wake.clone()
                };
                wake();
            })
            .map_err(|error| format!("Cannot start script loader: {error}"))?;
        Ok(Self { state })
    }

    fn submit(&self, job: Job) -> Result<(), String> {
        let mut state = self.state.0.lock().unwrap();
        if state.stopping || state.queue.len() + state.completed.len() >= loader::MAX_SCRIPTS {
            return Err(format!("Dynamic script worker queue limit reached ({}).", loader::MAX_SCRIPTS));
        }
        state.queue.push_back(job);
        self.state.1.notify_one();
        Ok(())
    }

    pub(super) fn set_wake(&self, wake: loader::Wake) {
        self.state.0.lock().unwrap().wake = wake;
    }

    fn poll(&self) -> Option<Completion> {
        let (result, wake) = {
            let mut state = self.state.0.lock().unwrap();
            let result = state.completed.pop_front();
            let wake = (!state.completed.is_empty()).then(|| state.wake.clone());
            (result, wake)
        };
        if let Some(wake) = wake {
            wake();
        }
        result
    }

    fn cancel(&self) {
        let mut state = self.state.0.lock().unwrap();
        if let Some(cancel) = &state.active {
            cancel.store(true, Ordering::Relaxed);
        }
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

fn fetch(
    job: &Job,
    trust: &crate::trust::Store,
    cookies: Arc<crate::cookies::Session>,
    events: &mut Vec<String>,
) -> Result<loader::Script, String> {
    if let Some(error) = &job.request.error {
        return Err(error.clone());
    }
    let url = job
        .request
        .target
        .url
        .as_ref()
        .ok_or("Dynamic script has no HTTP URL.")?;
    let origin = job
        .request
        .origin
        .url
        .as_ref()
        .ok_or("Dynamic script has no HTTP document origin.")?;
    crate::http::check_classic_script_mode(origin, url, job.request.same_origin_only)?;
    let filter = |target: &crate::url::HttpUrl| {
        job.blocking.check(target)?;
        // Apply CSP to every redirected destination before any DNS or request,
        // not only after the final response has already been transmitted.
        job.request.policy.allows_external(
            job.request.node,
            job.request.nonce.as_deref(),
            &Location {
                url: Some(target.clone()),
                path: None,
                root: None,
                fragment: String::new(),
            },
        )
    };
    let pool = crate::http::Pool::new(&job.cancel, trust)
        .with_cookie_session(cookies, Some(origin.clone()))
        .with_network_trace(job.network.clone());
    let response = pool.fetch_classic_script(
        url,
        loader::MAX_SCRIPT_BYTES,
        origin,
        crate::http::FetchMode::Normal,
        job.request.same_origin_only,
        &filter,
    )?;
    let content_type = response.header("content-type").unwrap_or("").to_owned();
    events.extend(response.events);
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "HTTP {} while fetching a dynamic script.",
            response.status
        ));
    }
    if !loader::javascript_mime(&loader::media_type(&content_type)) {
        return Err("Dynamic external scripts require a JavaScript Content-Type.".into());
    }
    let source = loader::decode_text(&response.body, &content_type, false)?;
    if source.len() > loader::MAX_SCRIPT_BYTES {
        return Err("Decoded dynamic script source exceeds 512 KiB.".into());
    }
    events.push(format!(
        "DYNAMIC SCRIPT node {}: loaded {} bytes from {}",
        job.request.node,
        source.len(),
        response.url.display()
    ));
    Ok(loader::Script {
        node: job.request.node,
        source,
        label: response.url.display(),
    })
}

impl App {
    pub(super) fn cancel_script_requests(&mut self) {
        self.cancel_stylesheet_requests();
        if let Some(worker) = &self.dynamic_scripts {
            worker.cancel();
        }
        self.dynamic_pending = 0;
        if let Some(page) = &mut self.scripting {
            page.clear_host_requests();
        }
    }

    fn script_jobs_paused(&self) -> bool {
        !self.scripts_enabled
            || !self.document.scripting_enabled
            || self.resubmission.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| !pending.committed)
    }

    pub(super) fn process_script_requests(&mut self) -> bool {
        if self.script_jobs_paused() {
            return false;
        }
        let Some(page) = &mut self.scripting else {
            return false;
        };
        let focus = page.take_focus_request();
        let navigation = page.take_navigation();
        let requests = page.take_dynamic_scripts();
        let changed = focus.is_some() || navigation.is_some() || !requests.is_empty();
        if let Some(node) = focus {
            self.focus_control(node);
        }
        if let Some(request) = navigation {
            if self.script_navigation_since.elapsed() >= Duration::from_secs(10) {
                self.script_navigation_since = Instant::now();
                self.script_navigation_count = 0;
            }
            if self.script_navigation_count >= 8 {
                if let Some(page) = &mut self.scripting {
                    page.host_error(
                        "Script navigation",
                        "Automatic navigation limit reached (8 within 10 seconds).",
                    );
                }
                self.show_script_errors();
            } else {
                self.script_navigation_count += 1;
                let fragment_only = !request.reload
                    && request.form_policy.is_none()
                    && request.target.url == self.location.url
                    && request.fragment_navigation;
                if fragment_only {
                    self.navigate_location_mode(request.target, true, request.replace);
                } else if request.reload && self.current_request.is_post() {
                    self.ask_resubmission(
                        request.target,
                        self.current_request.clone(),
                        NavigationKind::ScriptReplace(Some(self.scroll)),
                        crate::http::FetchMode::Normal,
                    );
                    return true;
                } else {
                    let kind = if request.replace {
                        NavigationKind::ScriptReplace(request.reload.then_some(self.scroll))
                    } else {
                        NavigationKind::ScriptNew
                    };
                    self.start_network_request_with_policy(
                        request.target,
                        kind,
                        crate::http::FetchMode::Normal,
                        request.request,
                        request.form_policy,
                    );
                    return true;
                }
            }
        }
        for request in requests {
            if self.dynamic_scripts.is_none() {
                match Worker::new(self.wake.clone(), self.trust.clone(), self.cookies.clone()) {
                    Ok(worker) => self.dynamic_scripts = Some(worker),
                    Err(error) => {
                        self.finish_dynamic_script(&request, Err(error));
                        continue;
                    }
                }
            }
            let job = Job {
                epoch: self.script_epoch,
                request: request.clone(),
                blocking: Arc::new(crate::blocking::Policy::new(
                    &self.profile.data().settings.blocking,
                )),
                network: self.network.clone(),
                cancel: Arc::new(AtomicBool::new(false)),
            };
            match self.dynamic_scripts.as_ref().unwrap().submit(job) {
                Ok(()) => self.dynamic_pending += 1,
                Err(error) => self.finish_dynamic_script(&request, Err(error)),
            }
        }
        changed
    }

    pub(super) fn poll_dynamic_scripts(&mut self) -> bool {
        if self.script_jobs_paused() {
            return false;
        }
        let Some(completion) = self.dynamic_scripts.as_ref().and_then(Worker::poll) else {
            return false;
        };
        if completion.epoch != self.script_epoch {
            return false;
        }
        self.dynamic_pending = self.dynamic_pending.saturating_sub(1);
        self.events.extend(completion.events);
        self.finish_dynamic_script(&completion.request, completion.result);
        true
    }

    fn finish_dynamic_script(
        &mut self,
        request: &DynamicScript,
        result: Result<loader::Script, String>,
    ) {
        let Some(page) = &mut self.scripting else {
            return;
        };
        let outcome =
            page.complete_dynamic_script(&mut self.document, &mut self.forms, request, result);
        if outcome.layout_changed {
            self.reflow();
        }
        self.validate_control_focus();
        self.sync_editors();
        self.show_script_errors();
        if self.view == View::Resources {
            self.refresh_resource_view();
        }
    }
}
