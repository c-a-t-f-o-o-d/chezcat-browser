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
    result: Result<String, String>,
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
        if state.stopping || state.queue.len() + state.completed.len() >= loader::MAX_STYLESHEETS {
            return Err(format!("Dynamic stylesheet worker queue limit reached ({}).", loader::MAX_STYLESHEETS));
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

    pub(super) fn cancel(&self) {
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
) -> Result<String, String> {
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
    if !origin.same_origin(url) {
        return Err("Cross-origin dynamic stylesheet request blocked.".into());
    }
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
    let response = pool.fetch_kind(
        url,
        job.request.limits.sheet_bytes,
        Some(origin),
        crate::http::FetchMode::Normal,
        "STYLESHEET",
    )?;
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
    Ok(source)
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
        }
        if changed {
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
        }
        if restyle {
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
                cancel: Arc::new(AtomicBool::new(false)),
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
        result: Result<String, String>,
    ) {
        let Some(page) = &mut self.scripting else {
            return;
        };
        let outcome = page.complete_dynamic_stylesheet(
            &mut self.document,
            &mut self.forms,
            &mut self.styles,
            request,
            result,
        );
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
                            "/bad.css" => "<p>This is HTML, not CSS</p>".to_owned(),
                            "/redirect.css" => {
                                status="302 Found";
                                headers=format!("Location: http://localhost:{}/forbidden.css\r\n", address.port());
                                String::new()
                            }
                            "/forbidden.css" => { mime="text/css"; "#target{width:999px}".to_owned() }
                            "/fresh" => "<title>Fresh document</title><p>Fresh</p>".to_owned(),
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
                                format!("<head><title>Waiting</title></head><body><script>var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href='{href}';{}sheet.onload=function(){{document.title='UNEXPECTED load';}};sheet.onerror=function(){{document.title='Refused';}};document.head.appendChild(sheet);</script></body>", if path == "/integrity" { "sheet.integrity='sha256-unverified';" } else { "" })
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
    fn dynamic_stylesheet_enforces_csp_origin_mime_integrity_and_redirect_origin() {
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

}
