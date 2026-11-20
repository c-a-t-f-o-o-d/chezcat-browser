//! Cancelable, document-owned dynamic stylesheet fetches. Only the App thread
//! applies CSS and delivers completion events after origin/CSP/MIME checks.

use super::*;
use crate::scripting::DynamicStylesheet;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

struct Job {
    epoch: u64,
    request: DynamicStylesheet,
    blocking: Arc<crate::blocking::Policy>,
    network: crate::network_trace::Trace,
    cancel: Arc<AtomicBool>,
}

struct Completion {
    epoch: u64,
    request: DynamicStylesheet,
    result: Result<(String, Location), String>,
    events: Vec<String>,
}

struct Shared {
    queue: VecDeque<Job>,
    completed: VecDeque<Completion>,
    active: Option<Arc<AtomicBool>>,
    stopping: bool,
    wake: loader::Wake,
    completed_payload: usize,
}

// Every admitted future completion reserves room for a storage-fault message.
const COMPLETION_ERROR_BYTES: usize = 64;
fn worker_storage(state: &Shared) -> usize {
    state.queue.capacity().saturating_mul(std::mem::size_of::<Job>())
        .saturating_add(state.completed.capacity().saturating_mul(std::mem::size_of::<Completion>() + COMPLETION_ERROR_BYTES))
        .saturating_add(state.completed_payload)
}
fn completion_payload(completion: &Completion) -> usize {
    let result = match &completion.result {
        Ok((source, base)) => source.capacity().saturating_add(base.payload_bytes()),
        Err(error) => error.capacity(),
    };
    completion.events.iter().fold(result.saturating_add(completion.events.capacity() * std::mem::size_of::<String>()),
        |bytes, event| bytes.saturating_add(event.capacity())).saturating_sub(COMPLETION_ERROR_BYTES)
}
fn reserve_worker_queue<T>(queue: &mut VecDeque<T>, needed: usize, slot: usize, available: usize) -> Result<(), String> {
    let before = queue.capacity();
    if needed <= before { return Ok(()); }
    let growth = before.max(4).max(needed - before).min(available / slot);
    let capacity = before.checked_add(growth).filter(|capacity| *capacity >= needed)
        .ok_or("Dynamic stylesheet worker storage exhausted.")?;
    let mut replacement = VecDeque::new();
    replacement.try_reserve_exact(capacity).map_err(|_| "Cannot allocate stylesheet worker storage.")?;
    if (replacement.capacity() - before).saturating_mul(slot) > available {
        return Err("Allocated stylesheet worker capacity exceeds its byte allowance.".into());
    }
    replacement.append(queue);
    *queue = replacement;
    Ok(())
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
                completed_payload: 0,
            }),
            Condvar::new(),
        ));
        let shared = state.clone();
        std::thread::Builder::new()
            .name("stylesheet-loader".into())
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
                    // Source retirement still owes one completion to the
                    // document counters. Bulk Stop/navigation takes active
                    // ownership away and resets those counters instead.
                    let publish = guard.active.take()
                        .is_some_and(|active| Arc::ptr_eq(&active, &job.cancel));
                    if guard.stopping {
                        return;
                    }
                    if publish {
                        let mut completion = Completion {
                            epoch: job.epoch,
                            request: job.request,
                            result,
                            events,
                        };
                        let bytes = completion_payload(&completion);
                        if bytes > completion.request.storage_limit.saturating_sub(worker_storage(&guard)) {
                            completion.result = Err("Dynamic stylesheet completion storage exhausted.".into());
                            completion.events = Vec::new();
                        }
                        guard.completed_payload += completion_payload(&completion);
                        guard.completed.push_back(completion);
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
        if state.stopping { return Err("Dynamic stylesheet worker is stopping.".into()); }
        let limit = job.request.storage_limit;
        let available = limit.saturating_sub(worker_storage(&state));
        let needed = state.queue.len().checked_add(1).ok_or("Stylesheet queue size overflow.")?;
        reserve_worker_queue(&mut state.queue, needed, std::mem::size_of::<Job>(), available)?;
        let available = limit.saturating_sub(worker_storage(&state));
        let needed = state.completed.len().checked_add(state.queue.len())
            .and_then(|count| count.checked_add(2)).ok_or("Stylesheet completion size overflow.")?;
        reserve_worker_queue(&mut state.completed, needed, std::mem::size_of::<Completion>() + COMPLETION_ERROR_BYTES, available)?;
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
            if let Some(completion) = &result { state.completed_payload -= completion_payload(completion); }
            let wake = (!state.completed.is_empty()).then(|| state.wake.clone());
            (result, wake)
        };
        if let Some(wake) = wake {
            wake();
        }
        result
    }

    pub(super) fn cancel(&self) {
        let mut state = self.state.0.lock().unwrap();
        if let Some(cancel) = state.active.take() {
            cancel.store(true, Ordering::Release);
        }
        state.queue.clear();
        state.completed.clear();
        state.completed_payload = 0;
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

fn fetch(
    job: &Job,
    trust: &crate::trust::Store,
    cookies: Arc<crate::cookies::Session>,
    events: &mut Vec<String>,
) -> Result<(String, Location), String> {
    if let Some(error) = &job.request.error {
        return Err(error.clone());
    }
    let url = job
        .request
        .target
        .url
        .as_ref()
        .ok_or("Dynamic stylesheet has no HTTP URL.")?;
    let origin = job
        .request
        .origin
        .url
        .as_ref()
        .ok_or("Dynamic stylesheet has no HTTP document origin.")?;
    crate::http::check_stylesheet_request(origin, url, job.request.same_origin_only)?;
    let filter = |target: &crate::url::HttpUrl| {
        job.blocking.check(target)?;
        job.request.policy.allows_external_style(
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
        .with_network_trace(job.network.clone())
        .with_resource_filter(&filter);
    let response = pool.fetch_stylesheet(
        url,
        job.request.limits.sheet_bytes,
        origin,
        crate::http::FetchMode::Normal,
        job.request.same_origin_only,
        &filter,
    ).map_err(|failure| failure.message)?.response;
    let content_type = response.header("content-type").unwrap_or("").to_owned();
    events.extend(response.events);
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "HTTP {} while fetching a dynamic stylesheet.",
            response.status
        ));
    }
    if loader::media_type(&content_type) != "text/css" {
        return Err("Dynamic external stylesheets require a text/css Content-Type.".into());
    }
    let source = loader::decode_text(&response.body, &content_type, false)?;
    job.request.limits.accept(&source,crate::css::StylesheetUsage::default())?;
    events.push(format!(
        "DYNAMIC STYLESHEET node {}: loaded {} bytes from {}",
        job.request.node,
        source.len(),
        response.url.display()
    ));
    Ok((source, Location {
        url: Some(response.url), path: None, root: None, fragment: String::new(),
    }))
}

impl App {
    pub(super) fn cancel_stylesheet_requests(&mut self) {
        if let Some(worker) = &self.dynamic_stylesheets {
            worker.cancel();
        }
        self.pending_stylesheets = 0;
        // Stop cancels fetching, not DOM mutations that already occurred in
        // the document job. Flush removals now so neither stale CSS nor a
        // phantom host task survives until a later poll.
        let removals = self
            .scripting
            .as_mut()
            .map(|page| page.take_stylesheet_removals())
            .unwrap_or_default();
        let mut changed = false;
        for node in removals {
            changed |= self.styles.remove(&node).is_some();
            self.style_bases.remove(&node);
        }
        if changed {
            if let Some(page) = &mut self.scripting { page.set_background_pending(true); }
            self.reflow();
        }
    }

    pub(super) fn process_stylesheet_requests(&mut self) -> bool {
        if !self.scripts_enabled
            || !self.document.scripting_enabled
            || self.resubmission.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| !pending.committed)
        {
            return false;
        }
        let Some(page) = &mut self.scripting else {
            return false;
        };
        let removed = page.take_stylesheet_removals();
        let requests = page.take_dynamic_stylesheets();
        let mut changed = !requests.is_empty();
        let mut restyle = false;
        for node in removed {
            restyle |= self.styles.remove(&node).is_some();
            self.style_bases.remove(&node);
        }
        if restyle {
            if let Some(page) = &mut self.scripting { page.set_background_pending(true); }
            self.reflow();
            changed = true;
        }
        for request in requests {
            if self.dynamic_stylesheets.is_none() {
                match Worker::new(self.wake.clone(), self.trust.clone(), self.cookies.clone()) {
                    Ok(worker) => self.dynamic_stylesheets = Some(worker),
                    Err(error) => {
                        self.finish_dynamic_stylesheet(&request, Err(error));
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
                cancel: request.cancel.clone(),
            };
            match self.dynamic_stylesheets.as_ref().unwrap().submit(job) {
                Ok(()) => self.pending_stylesheets += 1,
                Err(error) => self.finish_dynamic_stylesheet(&request, Err(error)),
            }
        }
        changed
    }

    pub(super) fn poll_dynamic_stylesheets(&mut self) -> bool {
        if !self.scripts_enabled
            || !self.document.scripting_enabled
            || self.resubmission.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| !pending.committed)
        {
            return false;
        }
        let Some(completion) = self.dynamic_stylesheets.as_ref().and_then(Worker::poll) else {
            return false;
        };
        self.accept_dynamic_stylesheet(completion)
    }

    fn accept_dynamic_stylesheet(&mut self, completion: Completion) -> bool {
        if completion.epoch != self.script_epoch {
            return false;
        }
        self.pending_stylesheets = self.pending_stylesheets.saturating_sub(1);
        self.events.extend(completion.events);
        self.finish_dynamic_stylesheet(&completion.request, completion.result);
        true
    }

    fn finish_dynamic_stylesheet(
        &mut self,
        request: &DynamicStylesheet,
        result: Result<(String, Location), String>,
    ) {
        let Some(page) = &mut self.scripting else {
            return;
        };
        let owned = page.accepts_dynamic_stylesheet(&self.document, request);
        let (result, base) = match result {
            Ok((source, base)) => (Ok(source), Some(base)),
            Err(error) => (Err(error), None),
        };
        let outcome = page.complete_dynamic_stylesheet(
            &mut self.document,
            &mut self.forms,
            &mut self.styles,
            request,
            result,
        );
        // Handlers may have committed another LINK mutation. Consume its
        // existing removal before this completion reflows/selects backgrounds.
        let mut removed = false;
        for node in page.take_stylesheet_removals() {
            removed |= self.styles.remove(&node).is_some();
            self.style_bases.remove(&node);
        }
        if removed { page.set_background_pending(true); }
        // Completion can synchronously run author handlers which replace or
        // remove this LINK. Never publish the previous request's base after
        // that mutation, or after a stale/admission-failed completion.
        if owned && page.owns_dynamic_stylesheet(request) {
            if self.styles.contains_key(&request.node) {
                if let Some(base) = base { self.style_bases.insert(request.node, base); }
            } else {
                self.style_bases.remove(&request.node);
            }
        }
        if outcome.layout_changed || removed {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::time::{Duration, Instant};

    struct Site {
        address: SocketAddr,
        stopped: Arc<AtomicBool>,
        requests: Arc<Mutex<Vec<String>>>,
        gate: Arc<(Mutex<bool>, Condvar)>,
        worker: Option<std::thread::JoinHandle<()>>,
    }

    impl Site {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let stopped = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let gate = Arc::new((Mutex::new(false), Condvar::new()));
            let (stop, seen, held) = (stopped.clone(), requests.clone(), gate.clone());
            let worker = std::thread::spawn(move || {
                let mut workers = Vec::new();
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { break };
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let (seen, held) = (seen.clone(), held.clone());
                    workers.push(std::thread::spawn(move || {
                        stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                        stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                        let mut request = Vec::new();
                        let mut byte = [0u8; 1];
                        while !request.ends_with(b"\r\n\r\n") && request.len() < 32768 {
                            if stream.read_exact(&mut byte).is_err() { return }
                            request.push(byte[0]);
                        }
                        let request = String::from_utf8_lossy(&request);
                        let path = request.split_ascii_whitespace().nth(1).unwrap_or("").to_owned();
                        seen.lock().unwrap().push(path.clone());
                        if matches!(path.as_str(),"/large.css"|"/latin.css") {
                            let (body,extra)=if path=="/large.css" {(include_bytes!("../tools/fixtures/large-tail.css.gz").to_vec(),"Content-Encoding: gzip\r\n")} else {
                                let mut body=b"/*".to_vec();body.extend(std::iter::repeat_n(0xa3,1000));body.extend_from_slice(b"*/#target{width:999px}");(body,"")
                            };
                            let encoding=if path=="/latin.css" {"windows-1252"}else{"utf-8"};
                            let header=format!("HTTP/1.1 200 OK\r\nContent-Type: text/css; charset={encoding}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",body.len());
                            let _=stream.write_all(header.as_bytes());let _=stream.write_all(&body);return;
                        }
                        let mut mime = "text/html";
                        let mut status = "200 OK";
                        let mut headers = String::new();
                        let body = match path.as_str() {
                            "/refused-font"=>format!("<style>#target{{font-family:RefusedFace}}</style><style>/*{}*/@font-face{{font-family:RefusedFace;src:url('/forbidden.woff')}}</style><div id=target>Used family</div>","x".repeat(200)),
                            "/initial-large"=>"<head><link rel=stylesheet href=/large.css><link rel=stylesheet href=/large.css></head><body><div id=target>Tail target</div>".to_owned(),
                            "/initial-latin"=>"<head><link rel=stylesheet href=/latin.css></head><body><div id=target>Tail target</div>".to_owned(),
                            "/dynamic-large"=>"<head></head><body><div id=target>Tail target</div><script>var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/large.css';sheet.onload=function(){if(document.getElementById('target').clientWidth!==173)throw 'tail geometry';document.title='Large applied';};sheet.onerror=function(){document.title='Large refused';};document.head.appendChild(sheet);</script>".to_owned(),
                            "/sheet.css" | "/held.css" => {
                                if path == "/held.css" {
                                    let _ = held.1.wait_timeout_while(held.0.lock().unwrap(), Duration::from_secs(4), |open| !*open).unwrap();
                                }
                                mime = "text/css";
                                "#target{display:block;width:123px;height:22px;color:#123456}".to_owned()
                            }
                            "/next.css" => { mime="text/css"; "#target{display:block;width:77px;height:19px}".to_owned() }
                            "/cross-ordinary" => format!("<html><head><title>Waiting</title></head><body><div id=target>Foreign target</div><script>var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='http://localhost:{}/foreign/theme.css';sheet.onload=function(){{if(document.getElementById('target').clientWidth!==147)throw 'foreign CSS geometry';if(('sheet' in sheet)||('cssRules' in sheet)||('styleSheets' in document)||document.styleSheets!==undefined)throw 'foreign CSS source exposed';document.title='Foreign stylesheet applied';}};sheet.onerror=function(){{throw 'ordinary foreign stylesheet refused';}};document.head.appendChild(sheet);</script></body></html>",address.port()),
                            "/foreign/theme.css" => {mime="text/css";"#target{display:block;width:147px;height:23px;background-color:#4521cd}".into()}
                            "/held-redirect.css" => {
                                let _=held.1.wait_timeout_while(held.0.lock().unwrap(),Duration::from_secs(4),|open|!*open).unwrap();
                                status="302 Found";headers=format!("Location: http://localhost:{}/retired-target.css\r\n",address.port());String::new()
                            }
                            "/retired-target.css" => {mime="text/css";"#target{width:999px}".into()}
                            "/base-redirect.css" => {
                                status="302 Found"; headers="Location: /assets/theme.css\r\n".into(); String::new()
                            }
                            "/assets/theme.css" => { mime="text/css"; "#target{display:block;width:131px;height:21px}".to_owned() }
                            "/bad.css" => "<p>This is HTML, not CSS</p>".to_owned(),
                            "/redirect.css" => {
                                status="302 Found";
                                headers=format!("Location: http://localhost:{}/forbidden.css\r\n", address.port());
                                String::new()
                            }
                            "/forbidden.css" => { mime="text/css"; "#target{width:999px}".to_owned() }
                            "/fresh" => "<title>Fresh document</title><p>Fresh</p>".to_owned(),
                            "/held-document" => {
                                let _=held.1.wait_timeout_while(held.0.lock().unwrap(),Duration::from_secs(8),|open|!*open).unwrap();
                                "<!doctype html><title>Held document committed</title><p>No old background or callbacks.</p>".to_owned()
                            }
                            "/startup-background"|"/image-background"|"/script-background" => callback_background_page(&path),
                            "/background-callback.js" => {mime="text/javascript";"var originalBackgroundScript=1;".to_owned()}
                            "/ready.ppm" => {mime="image/x-portable-pixmap";"P6\n1 1\n255\n\x64\0\0".to_owned()}
                            "/error-background" => r#"<!doctype html><html><head><title>Waiting for error background</title></head><body><div id="target" style="display:block;width:64px;height:48px">Pending background</div><script>
var bgWindowLoads=0,bgErrors=0;
window.onload=function(){bgWindowLoads++;if(bgErrors!==1)throw 'stylesheet error must precede window load';document.title='Error background loaded';};
var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='/bad.css';
sheet.onerror=function(){bgErrors++;document.getElementById('target').setAttribute('style','display:block;width:64px;height:48px;background-image:url(/error-background.ppm)');};
document.head.appendChild(sheet);
</script></body></html>"#.to_owned(),
                            "/error-background.ppm" => {
                                let _=held.1.wait_timeout_while(held.0.lock().unwrap(),Duration::from_secs(8),|open|!*open).unwrap();
                                mime="image/x-portable-pixmap";"P6\n1 1\n255\n\x64\0\0".to_owned()
                            }
                            "/blocked" | "/cross" | "/mime" | "/redirect" | "/integrity" => {
                                let href = match path.as_str() {
                                    "/cross" => format!("http://localhost:{}/forbidden.css", address.port()),
                                    "/mime" => "/bad.css".to_owned(),
                                    "/redirect" => "/redirect.css".to_owned(),
                                    _ => "/forbidden.css".to_owned(),
                                };
                                if path == "/blocked" {
                                    headers="Content-Security-Policy: script-src 'unsafe-inline'; style-src 'none'\r\n".into();
                                }
                                format!("<head><title>Waiting</title></head><body><script>var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='{href}';{}sheet.onload=function(){{document.title='UNEXPECTED load';}};sheet.onerror=function(){{document.title='Refused';}};document.head.appendChild(sheet);</script></body>", if path == "/integrity" { "sheet.integrity='sha256-unverified';" } else if matches!(path.as_str(), "/cross" | "/redirect") { "sheet.crossOrigin='anonymous';" } else { "" })
                            }
                            _ => {
                                let href = if path == "/held" { "/held.css" } else { "/sheet.css" };
                                format!(r#"<html><head><title>Waiting</title></head><body><div id="target">Visible target</div><script>
var order='parser';var sheet=document.createElement('link');
sheet.setAttribute('type','text/css');sheet.rel='stylesheet';sheet.media='screen';sheet.href='{href}';
if(sheet.type!=='text/css'||sheet.rel!=='stylesheet'||!('href' in sheet))throw Error('LINK reflection');
sheet.onload=function(event){{
 if(event.target!==sheet||this!==sheet||event.type!=='load')throw Error('LINK event');
 if(document.getElementById('target').clientWidth!==123)throw Error('CSS missing before load');
 order+='-sheet';document.title=order;
}};
sheet.onerror=function(){{throw Error('stylesheet fetch');}};
window.onload=function(){{if(order!=='parser-sheet')throw Error('load order');document.title=order+'-window';}};
document.head.appendChild(sheet);
</script></body></html>"#)
                            }
                        };
                        let response = format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len());
                        let _ = stream.write_all(response.as_bytes());
                    }));
                }
                for worker in workers {
                    worker.join().unwrap();
                }
            });
            Self {
                address,
                stopped,
                requests,
                gate,
                worker: Some(worker),
            }
        }
        fn location(&self, path: &str) -> Location {
            navigation::address(&format!("http://{}{path}", self.address), &Location::home())
                .unwrap()
        }
        fn release(&self) {
            *self.gate.0.lock().unwrap() = true;
            self.gate.1.notify_all();
        }
    }

    impl Drop for Site {
        fn drop(&mut self) {
            self.release();
            self.stopped.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(self.address);
            if let Some(worker) = self.worker.take() {
                worker.join().unwrap();
            }
        }
    }

    fn wait(app: &mut App, condition: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(6);
        while !condition(app) && Instant::now() < deadline {
            app.poll_load();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(condition(app), "{}\n{:?}", app.document.title, app.events);
    }

    fn open(site: &Site, path: &str) -> App {
        let mut app = App::new();
        app.resize(640, 480);
        app.navigate_location(site.location(path), true);
        app
    }

    fn eval(app: &mut App, source: &str) {
        let outcome =
            app.scripting
                .as_mut()
                .unwrap()
                .console_eval(&mut app.document, &mut app.forms, source);
        assert_eq!(
            outcome.errors,
            0,
            "{}",
            app.scripting.as_ref().unwrap().report()
        );
        if outcome.layout_changed {
            app.reflow();
        }
        app.poll_load();
    }

    #[test]
    fn stylesheet_replacement_reuses_current_source_allowance_instead_of_lifetime_work() {
        let site = Site::new();
        let mut app = open(&site, "/home");
        wait(&mut app, |app| !app.is_loading());
        app.document.stylesheet_limits = crate::css::StylesheetLimits {
            sheet_bytes: 128, document_bytes: 192, rules: 8,
        };
        for index in 0..8 {
            let (path, width) = if index % 2 == 0 { ("/next.css", 77) } else { ("/sheet.css", 123) };
            eval(&mut app, &format!("sheet.onload=null;sheet.href='{path}';"));
            wait(&mut app, |app| !app.is_loading());
            assert_eq!(app.styles.len(), 1);
            assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", app.scripting.as_ref().unwrap().report());
            eval(&mut app, &format!("if(document.getElementById('target').clientWidth!=={width})throw 'replacement geometry';"));
        }
    }

    #[test]
    fn dynamic_stylesheet_queue_and_lifetime_exceed_former_count_limit() {
        let site = Site::new();
        let mut app = open(&site, "/home");
        wait(&mut app, |app| !app.is_loading());
        eval(&mut app, "var manyLoads=0;for(var index=0;index<300;index++){var link=document.createElement('link');link.rel='stylesheet';link.href='/sheet.css';link.onload=function(){manyLoads++;};document.head.appendChild(link);}");
        wait(&mut app, |app| !app.is_loading());
        assert_eq!(app.styles.len(), 301);
        assert_eq!(app.pending_stylesheets, 0);
        eval(&mut app, "if(manyLoads!==300)throw 'missing stylesheet completion';");
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path| path.as_str()=="/sheet.css").count(), 301);
    }

    #[test]
    fn stylesheet_worker_completion_byte_refusal_recovers_for_next_real_response() {
        let site = Site::new();
        let mut app = open(&site, "/home");
        wait(&mut app, |app| !app.is_loading());
        let worker = Worker::new(app.wake.clone(), app.trust.clone(), app.cookies.clone()).unwrap();
        for (path, refused) in [("/large.css", true), ("/sheet.css", false)] {
            let page = app.scripting.as_mut().unwrap();
            let outcome = page.console_eval(&mut app.document, &mut app.forms,
                &format!("sheet.onload=null;sheet.href='{path}';"));
            assert_eq!(outcome.errors, 0, "{}", page.report());
            let mut requests = page.take_dynamic_stylesheets();
            assert_eq!(requests.len(), 1);
            let mut request = requests.pop_front().unwrap();
            Arc::get_mut(&mut request).unwrap().storage_limit = 16 * 1024;
            worker.submit(Job { epoch: app.script_epoch, request: request.clone(),
                blocking: Arc::new(crate::blocking::Policy::new(&app.profile.data().settings.blocking)),
                network: app.network.clone(), cancel: request.cancel.clone() }).unwrap();
            let deadline = Instant::now() + Duration::from_secs(6);
            let completion = loop {
                if let Some(completion) = worker.poll() { break completion; }
                assert!(Instant::now() < deadline, "bounded stylesheet response timed out");
                std::thread::sleep(Duration::from_millis(1));
            };
            if refused {
                assert!(completion.result.unwrap_err().contains("completion storage exhausted"));
            } else {
                assert!(completion.result.unwrap().0.contains("width:123px"));
            }
            assert_eq!(worker.state.0.lock().unwrap().completed_payload, 0);
        }
    }

    #[test]
    fn stylesheet_worker_failed_capacity_reservation_preserves_owned_queue() {
        let mut queue = VecDeque::from([7usize, 9]);
        let before = queue.capacity();
        assert!(reserve_worker_queue(&mut queue, before + 1, std::mem::size_of::<usize>(), 0).is_err());
        assert_eq!(queue, [7, 9]);
        assert_eq!(queue.capacity(), before);
        reserve_worker_queue(&mut queue, before + 1, std::mem::size_of::<usize>(), 1024).unwrap();
        assert_eq!(queue, [7, 9]);
    }

    #[test]
    fn dynamic_stylesheet_reflects_applies_before_load_and_keeps_dom_order() {
        let site = Site::new();
        let mut app = open(&site, "/home");
        wait(&mut app, |app| !app.is_loading());
        assert_eq!(
            app.document.title,
            "parser-sheet-window",
            "{}\n{:?}",
            app.scripting.as_ref().unwrap().report(),
            app.events
        );
        assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
        assert_eq!(app.styles.len(), 1);
        assert_eq!(*site.requests.lock().unwrap(), vec!["/home", "/sheet.css"]);
        eval(&mut app, "sheet.disabled=true;");
        assert!(app.styles.is_empty());
        eval(&mut app,"sheet.onload=function(){if(document.getElementById('target').clientWidth!==77)throw Error('replacement CSS');document.title='Replaced';};sheet.href='/next.css';sheet.disabled=false;");
        wait(&mut app, |app| !app.is_loading());
        assert_eq!(app.document.title, "Replaced");
        assert_eq!(app.styles.len(), 1);
        eval(&mut app, "document.head.removeChild(sheet);");
        assert!(app.styles.is_empty());
    }

    #[test]
    fn dynamic_stylesheet_enforces_csp_crossorigin_mode_mime_integrity_and_redirect_origin() {
        for path in ["/blocked", "/cross", "/mime", "/redirect", "/integrity"] {
            let site = Site::new();
            let mut app = open(&site, path);
            wait(&mut app, |app| !app.is_loading());
            assert_eq!(
                app.document.title,
                "Refused",
                "{path}: {}\n{:?}",
                app.scripting.as_ref().unwrap().report(),
                app.events
            );
            assert!(app.styles.is_empty(), "{path}");
            assert!(app.style_bases.is_empty(), "{path}: refused source acquired provenance");
            assert!(!site
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|path| path == "/forbidden.css"));
        }
    }

    #[test]
    fn stop_flushes_a_stylesheet_mutation_before_the_next_poll() {
        let site = Site::new();
        let mut app = open(&site, "/home");
        wait(&mut app, |app| !app.is_loading());
        assert_eq!(app.styles.len(), 1);
        let outcome = app.scripting.as_mut().unwrap().console_eval(
            &mut app.document,
            &mut app.forms,
            "sheet.href='/never-requested.css';",
        );
        assert_eq!(outcome.errors, 0);
        assert!(app.is_loading());
        app.stop_loading();
        assert!(!app.is_loading());
        assert!(app.styles.is_empty());
        app.poll_load();
        assert!(!site
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/never-requested.css"));
    }

    #[test]
    fn dynamic_stylesheet_discards_removed_or_stopped_inflight_completion() {
        for stop in [false, true] {
            let site = Site::new();
            let mut app = open(&site, "/held");
            wait(&mut app, |_| {
                site.requests
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|path| path == "/held.css")
            });
            if stop {
                app.stop_loading();
                app.navigate_location(site.location("/fresh"), true);
            } else {
                eval(
                    &mut app,
                    "window.onload=null;document.head.removeChild(sheet);document.title='Removed';",
                );
            }
            site.release();
            wait(&mut app, |app| !app.is_loading());
            assert!(app.styles.is_empty());
            assert_eq!(
                app.document.title,
                if stop { "Fresh document" } else { "Removed" }
            );
        }
    }

    fn original_large_css()->String {
        String::from_utf8(crate::compression::gzip(include_bytes!("../tools/fixtures/large-tail.css.gz"),8*1024*1024,&||Ok(())).unwrap()).unwrap()
    }
    #[test]
    fn shared_stylesheet_large_gzip_initial_reuse_and_dynamic_apply_the_complete_tail() {
        let site=Site::new();let expected=original_large_css();assert!(expected.len()>1024*1024);
        let mut app=open(&site,"/initial-large");wait(&mut app,|app|!app.is_loading());
        assert_eq!(app.styles.len(),2);assert!(app.styles.values().all(|source|source==&expected));
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/large.css").count(),1);
        let target=app.document.nodes.iter().enumerate().find_map(|(n,_)|(app.document.attr(n,"id")==Some("target")).then_some(n)).unwrap();
        let styles=crate::css::compute(&app.document,&app.styles);assert_eq!(styles[target].width,crate::css::Length::Px(173.0));assert_eq!(styles[target].height,crate::css::Length::Px(29.0));
        assert!(app.draw().pixels.contains(&0x123456));assert!(crate::css::stylesheet_diagnostics(&app.document,&app.styles).is_empty());
        let mut dynamic=open(&site,"/dynamic-large");wait(&mut dynamic,|app|!app.is_loading());
        assert_eq!(dynamic.document.title,"Large applied","{}",dynamic.scripting.as_ref().unwrap().report());
        assert_eq!(dynamic.scripting.as_ref().unwrap().error_count(),0);assert_eq!(dynamic.styles.values().next().unwrap(),&expected);
        assert!(dynamic.draw().pixels.contains(&0x123456));
    }
    #[test]
    fn shared_stylesheet_small_decode_profile_refuses_whole_source_and_later_recovers() {
        let site=Site::new();let mut app=App::new();
        app.stylesheet_limits=crate::css::StylesheetLimits{sheet_bytes:64*1024,document_bytes:256*1024,rules:32};
        app.navigate_location(site.location("/dynamic-large"),true);wait(&mut app,|app|!app.is_loading());
        assert_eq!(app.document.title,"Large refused");assert!(app.styles.is_empty());assert!(!app.draw().pixels.contains(&0x123456));
        eval(&mut app,"sheet.onload=function(){if(document.getElementById('target').clientWidth!==77)throw 'recovery geometry';document.title='Recovered';};sheet.onerror=function(){throw 'recovery source';};sheet.href='/next.css';");
        wait(&mut app,|app|!app.is_loading());assert_eq!(app.document.title,"Recovered");assert_eq!(app.styles.len(),1);
        assert!(app.styles.values().all(|source|source.len()<1024));
    }
    #[test]
    fn shared_stylesheet_checks_decoded_utf8_expansion_after_http_bytes() {
        let site=Site::new();let mut app=App::new();
        app.stylesheet_limits=crate::css::StylesheetLimits{sheet_bytes:1500,document_bytes:8192,rules:16};
        app.navigate_location(site.location("/initial-latin"),true);wait(&mut app,|app|!app.is_loading());
        assert!(app.styles.is_empty());assert!(app.events.iter().any(|event|event.contains("decoded UTF-8")),"{:?}",app.events);
        app.navigate_location(site.location("/home"),true);wait(&mut app,|app|!app.is_loading());
        assert_eq!(app.document.title,"parser-sheet-window");assert_eq!(app.styles.len(),1);
    }

    #[test]
    fn shared_stylesheet_refused_inline_source_cannot_supply_font_fetches() {
        let site=Site::new();let mut app=App::new();
        app.stylesheet_limits=crate::css::StylesheetLimits{sheet_bytes:128,document_bytes:256,rules:16};
        app.navigate_location(site.location("/refused-font"),true);wait(&mut app,|app|!app.is_loading());
        assert!(!site.requests.lock().unwrap().iter().any(|path|path=="/forbidden.woff"));
        assert!(!crate::css::stylesheet_diagnostics(&app.document,&app.styles).is_empty());
    }

    fn queue_base_change(app: &mut App, source: &str) {
        let outcome=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,source);
        assert_eq!(outcome.errors,0,"{}",app.scripting.as_ref().unwrap().report());
        if outcome.layout_changed {app.reflow();}
        app.process_stylesheet_requests();
    }
    fn take_base_completion(app: &App) -> Completion {
        let deadline=Instant::now()+Duration::from_secs(6);
        loop {
            if let Some(completion)=app.dynamic_stylesheets.as_ref().unwrap().poll() {return completion;}
            assert!(Instant::now()<deadline,"real stylesheet completion timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn assert_base(app:&App,node:usize,expected:&Location) {
        let base=app.style_bases.get(&node).expect("accepted stylesheet base");
        assert_eq!(base.url,expected.url);assert!(base.path.is_none()&&base.root.is_none()&&base.fragment.is_empty());
        assert!(app.styles.contains_key(&node));
    }
    #[test]
    fn dynamic_stylesheet_final_redirect_base_survives_application_and_clears_on_removal() {
        let site=Site::new();let mut app=open(&site,"/home");wait(&mut app,|app|!app.is_loading());
        let node=*app.styles.keys().next().unwrap();assert_base(&app,node,&site.location("/sheet.css"));
        queue_base_change(&mut app,"sheet.onload=null;sheet.href='/base-redirect.css';");
        let completion=take_base_completion(&app);
        assert_eq!(completion.result.as_ref().unwrap().1.url,site.location("/assets/theme.css").url);
        assert!(app.accept_dynamic_stylesheet(completion));
        assert_base(&app,node,&site.location("/assets/theme.css"));
        assert!(site.requests.lock().unwrap().iter().any(|path|path=="/base-redirect.css"));
        assert!(site.requests.lock().unwrap().iter().any(|path|path=="/assets/theme.css"));
        eval(&mut app,"sheet.disabled=true;");
        assert!(!app.styles.contains_key(&node)&&!app.style_bases.contains_key(&node));
    }
    #[test]
    fn dynamic_stylesheet_stale_success_cannot_overwrite_new_generation_or_document_base() {
        let site=Site::new();let mut app=open(&site,"/home");wait(&mut app,|app|!app.is_loading());
        let node=*app.styles.keys().next().unwrap();
        queue_base_change(&mut app,"sheet.onload=null;sheet.href='/base-redirect.css';");
        let older=take_base_completion(&app);
        queue_base_change(&mut app,"sheet.href='/next.css';");
        let newer=take_base_completion(&app);assert_ne!(older.request.generation,newer.request.generation);
        assert!(app.accept_dynamic_stylesheet(newer));assert_base(&app,node,&site.location("/next.css"));
        let css=app.styles[&node].clone();assert!(app.accept_dynamic_stylesheet(older));
        assert_base(&app,node,&site.location("/next.css"));assert_eq!(app.styles[&node],css);
        queue_base_change(&mut app,"sheet.href='/base-redirect.css';");let old_document=take_base_completion(&app);
        app.navigate_location(site.location("/fresh"),true);wait(&mut app,|app|!app.is_loading());
        assert!(!app.accept_dynamic_stylesheet(old_document));
        assert!(app.styles.is_empty()&&app.style_bases.is_empty());assert_eq!(app.document.title,"Fresh document");
    }
    #[test]
    fn dynamic_stylesheet_load_callback_replacement_cannot_publish_previous_base() {
        let site=Site::new();let mut app=open(&site,"/home");wait(&mut app,|app|!app.is_loading());
        let node=*app.styles.keys().next().unwrap();
        queue_base_change(&mut app,"sheet.onload=function(){sheet.onload=null;sheet.href='/next.css';};sheet.href='/base-redirect.css';");
        let completion=take_base_completion(&app);assert!(app.accept_dynamic_stylesheet(completion));
        // The genuine load handler has committed a new generation. Its old
        // text/base must be absent before App reflow can select background URLs.
        assert!(!app.styles.contains_key(&node)&&!app.style_bases.contains_key(&node));
        app.process_stylesheet_requests();let next=take_base_completion(&app);
        assert!(app.accept_dynamic_stylesheet(next));assert_base(&app,node,&site.location("/next.css"));
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    }

    #[test]
    fn stylesheet_error_added_background_delays_window_load_until_real_image_completion() {
        let site=Site::new();let mut app=open(&site,"/error-background");
        wait(&mut app,|_|site.requests.lock().unwrap().iter().any(|path|path=="/error-background.ppm"));
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/bad.css").count(),1);
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/error-background.ppm").count(),1);
        assert_eq!(app.pending_backgrounds.len(),1);assert!(app.backgrounds.is_empty());
        assert_eq!(app.document.title,"Waiting for error background","window load ran while the actual image body was held");
        eval(&mut app,"if(bgErrors!==1||bgWindowLoads!==0)throw 'background error-handler load barrier';");
        site.release();wait(&mut app,|app|!app.is_loading());
        assert_eq!(app.document.title,"Error background loaded");
        eval(&mut app,"if(bgErrors!==1||bgWindowLoads!==1)throw 'one actual window load';");
        assert!(app.pending_backgrounds.is_empty());assert_eq!(app.backgrounds.len(),1);
        let bitmap=app.backgrounds.values().next().unwrap();assert_eq!((bitmap.width,bitmap.height),(1,1));assert_eq!(bitmap.pixels,[0xff640000]);
        assert!(app.draw().pixels.contains(&0x640000));
        for _ in 0..4 {app.poll_load();}
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/error-background.ppm").count(),1);
        eval(&mut app,"if(bgWindowLoads!==1)throw 'repeated window load';");
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),1,"only the genuine bad stylesheet MIME error is expected");
    }

    fn callback_background_page(path:&str)->String {
        let trigger=match path {
            "/startup-background"=>"addHeldBackground();",
            "/image-background"=>"var img=document.createElement('img');img.onload=function(){addHeldBackground();};img.src='/ready.ppm';document.body.appendChild(img);",
            "/script-background"=>"var s=document.createElement('script');s.onload=function(){if(originalBackgroundScript!==1)throw 'actual script payload';addHeldBackground();};s.src='/background-callback.js';document.head.appendChild(s);",
            _=>panic!("unknown original background callback fixture"),
        };
        r#"<!doctype html><html><head><title>Waiting for callback background</title></head><body><div id="target" style="display:block;width:64px;height:48px">Pending background</div><script>
var bgWindowLoads=0,bgHandlers=0;
function addHeldBackground(){bgHandlers++;document.getElementById('target').setAttribute('style','display:block;width:64px;height:48px;background-image:url(/error-background.ppm)');}
window.onload=function(){bgWindowLoads++;if(bgHandlers!==1)throw 'callback before window load';document.title='Callback background loaded';};
ORIGINAL_TRIGGER
</script></body></html>"#.replace("ORIGINAL_TRIGGER",trigger)
    }
    fn assert_callback_background_guard(path:&str,preceding:Option<&str>) {
        let site=Site::new();let mut app=open(&site,path);
        wait(&mut app,|_|site.requests.lock().unwrap().iter().any(|request|request=="/error-background.ppm"));
        if let Some(preceding)=preceding {
            assert_eq!(site.requests.lock().unwrap().iter().filter(|request|request.as_str()==preceding).count(),1);
        }
        if path=="/image-background" {
            assert_eq!(app.images.len(),1,"the original image event must follow genuine decoding");
            let bitmap=app.images.values().next().unwrap();assert_eq!((bitmap.width,bitmap.height),(1,1));assert_eq!(bitmap.pixels,[0xff640000]);
        }
        assert_eq!(app.pending_backgrounds.len(),1);assert!(app.backgrounds.is_empty());
        assert_eq!(app.document.title,"Waiting for callback background","{path}: early window load");
        eval(&mut app,"if(bgHandlers!==1||bgWindowLoads!==0)throw 'actual callback background barrier';");
        site.release();wait(&mut app,|app|!app.is_loading());
        assert_eq!(app.document.title,"Callback background loaded");
        eval(&mut app,"if(bgHandlers!==1||bgWindowLoads!==1)throw 'one callback and window load';");
        assert_eq!(app.backgrounds.len(),1);assert!(app.pending_backgrounds.is_empty());
        let bitmap=app.backgrounds.values().next().unwrap();assert_eq!((bitmap.width,bitmap.height),(1,1));assert_eq!(bitmap.pixels,[0xff640000]);
        assert!(app.draw().pixels.contains(&0x640000));
        for _ in 0..4 {app.poll_load();}
        assert_eq!(site.requests.lock().unwrap().iter().filter(|request|request.as_str()=="/error-background.ppm").count(),1);
        eval(&mut app,"if(bgWindowLoads!==1)throw 'repeated callback window load';");
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{path}: {}",app.scripting.as_ref().unwrap().report());
    }
    #[test]
    fn startup_script_added_background_precedes_window_load_after_real_decode() {
        assert_callback_background_guard("/startup-background",None);
    }
    #[test]
    fn stopped_background_wait_never_resumes_abandoned_window_load() {
        let site=Site::new();let mut app=open(&site,"/startup-background");
        wait(&mut app,|_|site.requests.lock().unwrap().iter().any(|path|path=="/error-background.ppm"));
        assert_eq!(app.pending_backgrounds.len(),1);
        eval(&mut app,"if(bgWindowLoads!==0||bgHandlers!==1)throw 'actual pending background before Stop';");
        app.stop_loading();
        // The real source response is still held. Cancellation must not turn
        // its abandoned initial load into a successful window-load callback.
        for _ in 0..4 {app.poll_load();std::thread::sleep(Duration::from_millis(1));}
        assert_eq!(app.document.title,"Waiting for callback background","Stop resumed an abandoned window load before the body was released");
        eval(&mut app,"if(bgWindowLoads!==0||bgHandlers!==1)throw 'abandoned background load resumed after Stop';");
        assert!(app.pending_backgrounds.is_empty()&&app.backgrounds.is_empty());
        site.release();
        let end=Instant::now()+Duration::from_millis(100);
        while Instant::now()<end {app.poll_load();std::thread::sleep(Duration::from_millis(1));}
        app.reflow();let _=app.draw();app.reflow();
        for _ in 0..4 {app.poll_load();}
        assert!(app.pending_backgrounds.is_empty()&&app.backgrounds.is_empty());
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/error-background.ppm").count(),1,"Stop automatically retried its retained binding");
        assert_eq!(app.document.title,"Waiting for callback background");
        eval(&mut app,"if(bgWindowLoads!==0||bgHandlers!==1)throw 'late abandoned window load';");
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    }
    #[test]
    fn dynamic_image_load_handler_background_precedes_window_load_after_real_decode() {
        assert_callback_background_guard("/image-background",Some("/ready.ppm"));
    }

    #[test]
    fn uncommitted_navigation_cannot_resume_old_background_window_load() {
        let site=Site::new();let mut app=open(&site,"/startup-background");
        wait(&mut app,|_|site.requests.lock().unwrap().iter().any(|path|path=="/error-background.ppm"));
        assert_eq!(app.pending_backgrounds.len(),1);
        eval(&mut app,"if(bgWindowLoads!==0||bgHandlers!==1)throw 'old background still pending';");
        app.navigate_location(site.location("/held-document"),true);
        wait(&mut app,|_|site.requests.lock().unwrap().iter().any(|path|path=="/held-document"));
        assert!(app.pending.as_ref().is_some_and(|pending|!pending.committed));
        assert_eq!(app.location.url,site.location("/startup-background").url);
        for _ in 0..4 {app.poll_load();std::thread::sleep(Duration::from_millis(1));}
        assert!(app.pending.as_ref().is_some_and(|pending|!pending.committed));
        assert_eq!(app.document.title,"Waiting for callback background","old window load ran during an uncommitted navigation");
        eval(&mut app,"if(bgWindowLoads!==0||bgHandlers!==1)throw 'old background load resumed during navigation';");
        site.release();wait(&mut app,|app|!app.is_loading());
        assert!(!app.failed,"{}",app.status);assert_eq!(app.document.title,"Held document committed");
        assert_eq!(app.location.url,site.location("/held-document").url);
        assert!(app.backgrounds.is_empty()&&app.pending_backgrounds.is_empty()&&app.background_requests.is_empty());
        let end=Instant::now()+Duration::from_millis(100);
        while Instant::now()<end {app.poll_load();std::thread::sleep(Duration::from_millis(1));}
        assert_eq!(app.document.title,"Held document committed");
        assert!(app.backgrounds.is_empty()&&app.pending_backgrounds.is_empty());
        eval(&mut app,"if(typeof bgWindowLoads!=='undefined'||typeof bgHandlers!=='undefined')throw 'old callback realm survived new document';");
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/error-background.ppm").count(),1);
        assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/held-document").count(),1);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    }
    #[test]
    fn dynamic_script_load_handler_background_precedes_window_load_after_real_decode() {
        assert_callback_background_guard("/script-background",Some("/background-callback.js"));
    }

    #[test]
    fn ordinary_foreign_dynamic_stylesheet_applies_real_geometry_pixels_and_final_base() {
        let site=Site::new();let mut app=open(&site,"/cross-ordinary");wait(&mut app,|app|!app.is_loading());
        assert_eq!(app.document.title,"Foreign stylesheet applied","{}\n{:?}",app.scripting.as_ref().unwrap().report(),app.events);
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);assert_eq!(app.pending_stylesheets,0);assert!(!app.scripting.as_ref().unwrap().has_host_tasks());
        assert_eq!(*site.requests.lock().unwrap(),["/cross-ordinary","/foreign/theme.css"]);
        let node=*app.styles.keys().next().unwrap();let expected=navigation::address(&format!("http://localhost:{}/foreign/theme.css",site.address.port()),&Location::home()).unwrap();assert_base(&app,node,&expected);
        assert!(app.draw().pixels.contains(&0x4521cd));
        // LINK reserves its native sheet getter, which explicitly declines.
        // cssRules is an absent ordinary expando instead: consume its value
        // and require undefined rather than inventing a native CSSOM getter.
        let retained=app.styles[&node].clone();
        let source="var sheetReadCompleted=false;var foreignSheetRead=sheet.sheet;sheetReadCompleted=true;";
        let outcome=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,source);
        assert_eq!(outcome.errors,1,"probe {source:?}: {}",app.scripting.as_ref().unwrap().report());
        assert!(app.scripting.as_ref().unwrap().report().contains("Element property \"sheet\" is not supported."));
        assert_eq!(app.styles[&node],retained);assert_base(&app,node,&expected);
        eval(&mut app,"var foreignRuleRead=sheet.cssRules;if(sheetReadCompleted||foreignSheetRead!==undefined||foreignRuleRead!==undefined||document.styleSheets!==undefined)throw 'foreign stylesheet source/rules exposed';");
        assert_eq!(app.scripting.as_ref().unwrap().error_count(),1,"only the explicit unsupported native sheet read is expected");
        assert_eq!(app.styles[&node],retained);assert_base(&app,node,&expected);
        assert!(app.draw().pixels.contains(&0x4521cd));assert_eq!(app.pending_stylesheets,0);
    }
    #[test]
    fn dynamic_stylesheet_href_retirement_cancels_before_redirect_and_accounts_both_sources() {
        let site=Site::new();let mut app=open(&site,"/home");wait(&mut app,|app|!app.is_loading());
        eval(&mut app,"var replacementLoads=0,replacementErrors=0;sheet.onload=function(){replacementLoads++;};sheet.onerror=function(){replacementErrors++;};sheet.href='/held-redirect.css';");
        wait(&mut app,|_|site.requests.lock().unwrap().iter().any(|path|path=="/held-redirect.css"));
        let old_cancel=app.dynamic_stylesheets.as_ref().unwrap().state.0.lock().unwrap().active.as_ref().unwrap().clone();
        eval(&mut app,"sheet.href='/next.css';");assert!(old_cancel.load(Ordering::Acquire));
        site.release();wait(&mut app,|app|!app.is_loading());
        assert!(!site.requests.lock().unwrap().iter().any(|path|path=="/retired-target.css"));
        assert_eq!(app.pending_stylesheets,0);assert!(!app.scripting.as_ref().unwrap().has_host_tasks());
        let node=*app.styles.keys().next().unwrap();assert_base(&app,node,&site.location("/next.css"));
        eval(&mut app,"if(replacementLoads!==1||replacementErrors!==0||document.getElementById('target').clientWidth!==77)throw 'retired stylesheet task/event/geometry';");
    }
    #[test]
    fn dynamic_stylesheet_stop_discards_old_worker_ownership_before_new_job() {
        let site=Site::new();let mut app=open(&site,"/held");
        wait(&mut app,|_|site.requests.lock().unwrap().iter().any(|path|path=="/held.css"));
        app.stop_loading();assert_eq!(app.pending_stylesheets,0);
        eval(&mut app,"var afterStopLoads=0;window.onload=null;sheet.onload=function(){afterStopLoads++;};sheet.onerror=function(){throw 'new stylesheet after Stop';};sheet.href='/next.css';");
        site.release();wait(&mut app,|app|!app.is_loading());
        assert_eq!(app.pending_stylesheets,0);assert!(!app.scripting.as_ref().unwrap().has_host_tasks());
        eval(&mut app,"if(afterStopLoads!==1||document.getElementById('target').clientWidth!==77)throw 'post-Stop stylesheet accounting';");
    }

}
