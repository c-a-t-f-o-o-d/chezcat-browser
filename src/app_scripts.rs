//! App-owned execution of document requests. JavaScript never holds a network
//! socket or destroys its own realm. One cancelable worker fetches bounded
//! external classic scripts; completions return to the normal App thread.

use super::*;
use crate::scripting::DynamicScript;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool,AtomicUsize, Ordering};
use crate::script_storage::Lease;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

struct Job {
    epoch: u64,
    request: DynamicScript,
    blocking: Arc<crate::blocking::Policy>,
    network: crate::network_trace::Trace,
    cancel: Arc<AtomicBool>,
    _storage:Option<Lease>,
}

struct Completion {
    epoch: u64,
    request: DynamicScript,
    result: Result<loader::Script,ScriptFailure>,
    events: Vec<String>,
    _storage:Option<Lease>,
}

struct Shared {
    queue: VecDeque<Job>,
    completed: VecDeque<Completion>,
    payload_used:Arc<AtomicUsize>,
    active: Option<Arc<AtomicBool>>,
    stopping: bool,
    wake: loader::Wake,
}

#[derive(Debug)]
enum ScriptFailure {Message(String),Storage}
impl ScriptFailure {
    fn into_message(self)->String{match self{Self::Message(error)=>error,Self::Storage=>"Dynamic script completion storage exhausted.".into()}}
}
fn worker_storage(state:&Shared)->usize {
    state.queue.capacity().saturating_mul(std::mem::size_of::<Job>())
        .saturating_add(state.completed.capacity().saturating_mul(std::mem::size_of::<Completion>()))
        .saturating_add(state.payload_used.load(Ordering::Acquire))
}
fn completion_payload(completion:&Completion)->usize {
    let result=match &completion.result{Ok(script)=>script.payload_bytes(),Err(ScriptFailure::Message(error))=>error.capacity(),Err(ScriptFailure::Storage)=>0};
    completion.events.iter().fold(result.saturating_add(completion.events.capacity()*std::mem::size_of::<String>()),|bytes,event|bytes.saturating_add(event.capacity()))
}
fn reserve_worker_queue<T>(queue:&mut VecDeque<T>,needed:usize,available:usize)->Result<(),String> {
    let before=queue.capacity();if needed<=before{return Ok(());}
    let slot=std::mem::size_of::<T>();let growth=before.max(4).max(needed-before).min(available/slot);
    let capacity=before.checked_add(growth).filter(|capacity|*capacity>=needed).ok_or("Dynamic script worker storage exhausted.")?;
    let mut next=VecDeque::new();next.try_reserve_exact(capacity).map_err(|_|"Could not reserve dynamic script worker queue.")?;
    if next.capacity().saturating_sub(before).saturating_mul(slot)>available{return Err("Allocated script worker backing exceeds allowance.".into());}
    next.append(queue);*queue=next;Ok(())
}
impl Shared {
    fn complete(&mut self,mut completion:Completion,limit:usize) {
        // Submit reserves every future completion slot. Storage refusal uses an
        // inline typed fault, requiring no allocation when the budget is full.
        assert!(self.completed.len()<self.completed.capacity(),"Admitted script completion requires a reserved slot");
        let metadata=self.queue.capacity()*std::mem::size_of::<Job>()+self.completed.capacity()*std::mem::size_of::<Completion>();
        match Lease::new(self.payload_used.clone(),completion_payload(&completion),limit.saturating_sub(metadata)) {
            Ok(storage)=>completion._storage=Some(storage),
            Err(_)=>{completion.result=Err(ScriptFailure::Storage);completion.events=Vec::new();}
        }
        self.completed.push_back(completion);
    }
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
                payload_used:Arc::new(AtomicUsize::new(0)),
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
                let (job, source_limit) = {
                    let mut guard = shared.0.lock().unwrap();
                    while guard.queue.is_empty() && !guard.stopping {
                        guard = shared.1.wait(guard).unwrap();
                    }
                    if guard.stopping {
                        return;
                    }
                    let job = guard.queue.pop_front().unwrap();
                    guard.active = Some(job.cancel.clone());
                    let remaining=job.request.storage_limit.saturating_sub(worker_storage(&guard));
                    let source_limit = job.request.source_limit.min(remaining);
                    (job, source_limit)
                };
                let mut events = Vec::new();
                let result = fetch(&job, source_limit, &trust, cookies.clone(), &mut events);
                let wake = {
                    let mut guard = shared.0.lock().unwrap();
                    guard.active = None;
                    if guard.stopping {
                        return;
                    }
                    if !job.cancel.load(Ordering::Relaxed) {
                        let limit=job.request.storage_limit;
                        guard.complete(Completion {
                            epoch: job.epoch,
                            request: job.request,
                            result:result.map_err(ScriptFailure::Message),
                            events,
                            _storage:None,
                        }, limit);
                    }
                    guard.wake.clone()
                };
                wake();
            })
            .map_err(|error| format!("Cannot start script loader: {error}"))?;
        Ok(Self { state })
    }

    fn submit(&self,mut job:Job)->Result<(),String> {
        let mut state = self.state.0.lock().unwrap();
        if state.stopping{return Err("Dynamic script worker is stopping.".into());}
        let limit=job.request.storage_limit;
        let metadata=state.queue.capacity()*std::mem::size_of::<Job>()+state.completed.capacity()*std::mem::size_of::<Completion>();
        let payload=job.blocking.owned_bytes().saturating_add(4*std::mem::size_of::<usize>()+std::mem::size_of::<AtomicBool>());
        job._storage=Some(Lease::new(state.payload_used.clone(),payload,limit.saturating_sub(metadata))?);
        let completions=state.completed.len().checked_add(state.queue.len()).and_then(|count|count.checked_add(usize::from(state.active.is_some()))).and_then(|count|count.checked_add(1)).ok_or("Dynamic script completion size overflow.")?;
        let minimum=completions.saturating_sub(state.completed.capacity()).checked_mul(std::mem::size_of::<Completion>()).ok_or("Dynamic script completion storage overflow.")?;
        let available=limit.saturating_sub(worker_storage(&state)).checked_sub(minimum).ok_or("Dynamic script completion backing exhausted.")?;
        let needed=state.queue.len().checked_add(1).ok_or("Dynamic script queue size overflow.")?;
        reserve_worker_queue(&mut state.queue,needed,available)?;
        let available=limit.saturating_sub(worker_storage(&state));
        reserve_worker_queue(&mut state.completed,completions,available)?;
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
        if let Some(cancel)=state.active.take(){cancel.store(true,Ordering::Relaxed);}
        state.queue=VecDeque::new();
        state.completed=VecDeque::new();
    }
}

#[cfg(test)]
impl Worker {
    pub(crate) fn retirement_probe(&self) -> impl Fn() -> bool + 'static {
        let state = Arc::downgrade(&self.state);
        move || state.upgrade().is_none()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        self.state.0.lock().unwrap().stopping = true;
        self.state.1.notify_one();
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    fn request()->DynamicScript {
        Arc::new(crate::scripting::ScriptRequest{node:1,target:Location::home(),origin:Location::home(),nonce:None,
            policy:Arc::new(crate::csp::PolicySet::default()),same_origin_only:false,fallback_encoding:crate::text_codec::Encoding::UTF8,
            source_limit:crate::javascript::limits::BROWSER_CAPACITIES.source_bytes,error:None,
            storage_limit:crate::javascript::limits::BROWSER_CAPACITIES.script_storage_bytes,
            _storage:Lease::default(),_policy_storage:Arc::new(Lease::default())})
    }
    fn completion(source:&str)->Completion {
        Completion{epoch:1,request:request(),result:Ok(loader::Script{node:1,source:source.into(),label:"bounded completion".into()}),events:Vec::new(),_storage:None}
    }
    #[test]
    fn completed_script_payload_is_bounded_and_stays_charged_across_ui_poll_and_cancel() {
        let worker=Worker{state:Arc::new((Mutex::new(Shared{queue:VecDeque::new(),completed:VecDeque::with_capacity(4),payload_used:Arc::new(AtomicUsize::new(0)),active:None,stopping:false,wake:Arc::new(||{})}),Condvar::new()))};
        let first=completion("abc");let first_bytes=completion_payload(&first);let second=completion("12345");let second_bytes=completion_payload(&second);
        let limit={let mut state=worker.state.0.lock().unwrap();let limit=worker_storage(&state)+first_bytes+second_bytes;
            state.complete(first,limit);state.complete(second,limit);state.complete(completion("overflow"),limit);
            assert_eq!(state.payload_used.load(Ordering::Acquire),first_bytes+second_bytes);assert_eq!(state.completed.len(),3);
            assert!(matches!(state.completed[2].result,Err(ScriptFailure::Storage)));limit};
        let active=worker.poll().unwrap();assert_eq!(active.result.as_ref().unwrap().source,"abc");
        assert_eq!(worker.state.0.lock().unwrap().payload_used.load(Ordering::Acquire),first_bytes+second_bytes);
        worker.cancel();assert_eq!(worker.state.0.lock().unwrap().payload_used.load(Ordering::Acquire),first_bytes);
        drop(active);assert_eq!(worker.state.0.lock().unwrap().payload_used.load(Ordering::Acquire),0);
        let mut state=worker.state.0.lock().unwrap();let available=limit.saturating_sub(worker_storage(&state));reserve_worker_queue(&mut state.completed,1,available).unwrap();
        state.complete(completion("recovered"),limit);assert!(state.completed[0].result.is_ok());
    }
    fn worker()->Worker {
        Worker{state:Arc::new((Mutex::new(Shared{queue:VecDeque::new(),completed:VecDeque::new(),payload_used:Arc::new(AtomicUsize::new(0)),active:None,stopping:false,wake:Arc::new(||{})}),Condvar::new()))}
    }
    fn job(request:DynamicScript,epoch:u64)->Job {
        Job{epoch,request,blocking:Arc::new(crate::blocking::Policy::default()),network:crate::network_trace::Trace::default(),cancel:Arc::new(AtomicBool::new(false)),_storage:None}
    }
    #[test]
    fn dynamic_script_encoded_input_can_exceed_decoded_allowance_and_refusal_recovers() {
        use std::io::{Read,Write};
        use std::net::TcpListener;
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let url=crate::url::parse(&format!("http://{}/source.js",listener.local_addr().unwrap())).unwrap().0;
        let server=std::thread::spawn(move||{
            for _ in 0..3 {
                let (mut stream,_)=listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut head=Vec::new();let mut byte=[0];
                while !head.ends_with(b"\r\n\r\n"){assert_eq!(stream.read(&mut byte).unwrap(),1);head.push(byte[0]);assert!(head.len()<16384);}
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: 6\r\nConnection: close\r\n\r\n\xff\xfe1\0;\0").unwrap();
            }
        });
        let mut request=request();
        let location=Location{url:Some(url),path:None,root:None,fragment:String::new()};
        {let request=Arc::get_mut(&mut request).unwrap();request.target=location.clone();request.origin=location.clone();}
        let job=job(request,1);let trust=crate::trust::Store::unconfigured();
        let cookies=Arc::new(crate::cookies::Session::default());
        for limit in [2,1,2] {
            let result=fetch(&job,limit,&trust,cookies.clone(),&mut Vec::new());
            if limit==1 { assert!(result.err().unwrap().contains("Decoded text exceeds")); }
            else {let script=result.unwrap();assert_eq!(script.source,"1;");assert_eq!(script.label,location.display());}
        }
        server.join().unwrap();
    }
    #[test]
    fn dynamic_script_worker_exceeds_old_queue_count_and_preserves_fifo_with_actual_owned_storage() {
        let worker=worker();let request=request();let limit=request.storage_limit;
        for epoch in 0..6000{worker.submit(job(request.clone(),epoch)).unwrap();}
        {let mut state=worker.state.0.lock().unwrap();assert_eq!(state.queue.len(),6000);assert!(state.completed.capacity()>=6000);
            while let Some(job)=state.queue.pop_front(){let source=job.epoch.to_string();state.complete(Completion{epoch:job.epoch,request:job.request,result:Ok(loader::Script{node:1,source,label:String::new()}),events:Vec::new(),_storage:None},limit);}
            assert!(worker_storage(&state)<=limit);}
        for epoch in 0..6000{let completion=worker.poll().unwrap();assert_eq!(completion.epoch,epoch);assert_eq!(completion.result.as_ref().unwrap().source,epoch.to_string());}
        assert_eq!(worker.state.0.lock().unwrap().payload_used.load(Ordering::Acquire),0);
        worker.cancel();assert_eq!(worker_storage(&worker.state.0.lock().unwrap()),0);
    }
    #[test]
    fn dynamic_script_worker_one_complete_job_and_completion_fit_exactly_and_refusal_keeps_admitted_job() {
        let worker=worker();let mut request=request();let candidate=job(request.clone(),1);
        let payload=candidate.blocking.owned_bytes()+4*std::mem::size_of::<usize>()+std::mem::size_of::<AtomicBool>();drop(candidate);
        let exact=payload+std::mem::size_of::<Job>()+std::mem::size_of::<Completion>();
        Arc::get_mut(&mut request).unwrap().storage_limit=exact-1;
        assert!(worker.submit(job(request.clone(),1)).is_err());assert_eq!(worker_storage(&worker.state.0.lock().unwrap()),0);
        Arc::get_mut(&mut request).unwrap().storage_limit=exact;
        worker.submit(job(request.clone(),1)).unwrap();
        {let state=worker.state.0.lock().unwrap();assert_eq!(state.queue.len(),1);assert_eq!(state.queue.capacity(),1);assert_eq!(state.completed.capacity(),1);assert_eq!(worker_storage(&state),exact);}
        assert!(worker.submit(job(request.clone(),2)).is_err());assert_eq!(worker.state.0.lock().unwrap().queue.len(),1);
        worker.cancel();assert_eq!(worker_storage(&worker.state.0.lock().unwrap()),0);
    }
}

fn fetch(
    job: &Job,
    source_limit: usize,
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
    let wire_limit = crate::javascript::limits::BROWSER_CAPACITIES.source_bytes;
    let response = pool.fetch_classic_script(
        url,
        wire_limit,
        wire_limit,
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
    let source = loader::decode_script(&response.body, &content_type, job.request.fallback_encoding, source_limit, &|| {
        if job.cancel.load(Ordering::Relaxed) { Err("Script loading canceled.".into()) } else { Ok(()) }
    })?;
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
        self.apply_history_replacement();
        self.cancel_stylesheet_requests();
        self.cancel_image_requests();
        self.cancel_background_requests();
        if let Some(worker) = &self.dynamic_scripts {
            worker.cancel();
        }
        self.script_requests=None;
        self.dynamic_pending = 0;
        if let Some(page) = &mut self.scripting {
            page.cancel_media_selection();
            page.clear_host_requests();
        }
    }

    pub(super) fn script_jobs_paused(&self) -> bool {
        !self.scripts_enabled
            || !self.document.scripting_enabled
            || self.resubmission.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| !pending.committed)
    }

    /// A completed page event has drained its observer/Promise jobs. Apply its
    /// latest focus request against current layout before the next native edit.
    pub(super) fn apply_script_focus(&mut self)->bool {
        if self.script_jobs_paused(){return false;}
        let Some(node)=self.scripting.as_mut().and_then(|page|page.take_focus_request()) else{return false;};
        self.focus_control(node);
        true
    }
    pub(super) fn process_script_requests(&mut self) -> bool {
        self.apply_media_pause();
        let effects=self.poll_media_promise_cleanup();
        self.process_script_requests_after_cleanup() | effects | self.process_native_script_requests()
    }
    fn process_script_requests_after_cleanup(&mut self) -> bool {
        if self.script_jobs_paused() {
            return false;
        }
        let history_changed = self.apply_history_replacement();
        let scrolled = self.apply_script_scroll();
        let Some(page) = &mut self.scripting else {
            return false;
        };
        let focus = page.take_focus_request();
        let navigation = page.take_navigation();
        let requests = page.take_dynamic_scripts();
        let changed = history_changed || scrolled || focus.is_some() || navigation.is_some() || !requests.is_empty();
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
                _storage:None,
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
        let _active_storage=completion._storage;
        self.events.extend(completion.events);
        self.finish_dynamic_script(&completion.request,completion.result.map_err(ScriptFailure::into_message));
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
        // Finish this owned resource task before consuming its queued navigation.
        self.resume_background_loading();
        self.validate_control_focus();
        self.sync_editors();
        self.show_script_errors();
        if self.view == View::Resources {
            self.refresh_resource_view();
        }
    }
}
