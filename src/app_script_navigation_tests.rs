//! Actual App/HTTP regressions for external-script completion, session cookies,
//! deferred Location navigation and cancellation. All peers are loopback-only.
use super::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const HOME: &str = r#"<!doctype html><html><head><title>Home document</title></head><body>
<form name="f"><input name="q" value="test"></form>
<script>
if(document.cookie!=='')throw Error('HttpOnly cookie exposed at startup');
if(document.f.q.value!=='test')throw Error('named form/control lookup');
var stages='parser';
window.onload=function(){
  if(stages!=='parser,boot,script-load')throw Error('window load order: '+stages);
  document.title='Home loaded';
};
var boot=document.createElement('script');
boot.src='/boot.js';
boot.onload=function(event){
  if(this!==boot || event.target!==boot || event.type!=='load')throw Error('load event');
  if(stages!=='parser,boot')throw Error('script load order: '+stages);
  stages+=',script-load';
  document.title='Boot loaded';
};
boot.onerror=function(){throw Error('boot fetch failed');};
document.head.appendChild(boot);
</script></body></html>"#;

const BOOT: &str = r#"
if(stages!=='parser')throw Error('bootstrap order: '+stages);
stages+=',boot';
if(document.cookie!=='')throw Error('HttpOnly cookie exposed to external script');
document.cookie='client=ready; Path=/';
if(document.cookie!=='client=ready')throw Error('document cookie write/read');
location.replace('/search?q='+encodeURIComponent(document.f.q.value));
"#;

const SEARCH: &str = r#"<!doctype html><html><head><title>Search response</title></head>
<body><h1>Local test results</h1><script>
if(document.cookie!=='client=ready')throw Error('search cookie visibility');
document.title='Results for '+location.search;
</script></body></html>"#;

const HELD: &str = r#"<!doctype html><html><head><title>Waiting for script</title></head>
<body><script>var held=document.createElement('script');held.src='/held.js';
held.onload=function(){document.title='STALE load handler';};
held.onerror=function(){document.title='STALE error handler';};
document.head.appendChild(held);</script></body></html>"#;

const FRESH: &str = r#"<!doctype html><html><head><title>Fresh pending</title></head>
<body><script>var fresh=document.createElement('script');fresh.src='/fresh.js';
fresh.onload=function(){document.title='Fresh document';};
fresh.onerror=function(){throw Error('fresh script fetch failed');};
document.head.appendChild(fresh);</script></body></html>"#;

const HASH: &str = r#"<!doctype html><html><head><title>Hash pending</title></head>
<body><script>
var scriptRuns=0;
location.hash='#kept';
var first=document.createElement('script');first.src='/hash.js';
first.onerror=function(){throw Error('first hash script failed');};
document.head.appendChild(first);
</script></body></html>"#;

const HASH_SCRIPT: &str = r#"
if(location.hash!=='#kept' || ++scriptRuns!==1)throw Error('first hash execution');
location.hash='#changed';
var second=document.createElement('script');second.src='/after-hash.js';
second.onload=function(){
  if(scriptRuns!==2)throw Error('hash script count');
  document.title='Hash finished';
};
second.onerror=function(){throw Error('second hash script failed');};
document.head.appendChild(second);
"#;

#[derive(Clone, Debug)]
struct Request {
    method: String,
    path: String,
    cookie: String,
}

struct Peer {
    address: SocketAddr,
    gate: Arc<(Mutex<bool>, Condvar)>,
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<Request>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Peer {
    fn new(other_origin: Option<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (server_gate, server_stopped, server_requests) =
            (gate.clone(), stopped.clone(), requests.clone());
        let worker = std::thread::spawn(move || {
            let mut workers = Vec::new();
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                if server_stopped.load(Ordering::Relaxed) {
                    break;
                }
                let gate = server_gate.clone();
                let requests = server_requests.clone();
                let other_origin = other_origin.clone();
                workers.push(std::thread::spawn(move || {
                    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut bytes = Vec::new();
                    let mut byte = [0u8; 1];
                    while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 32768 {
                        if stream.read_exact(&mut byte).is_err() { return; }
                        bytes.push(byte[0]);
                    }
                    if !bytes.ends_with(b"\r\n\r\n") { return; }
                    let header = String::from_utf8_lossy(&bytes);
                    let mut first = header.lines().next().unwrap_or("").split_whitespace();
                    let method = first.next().unwrap_or("").to_owned();
                    let path = first.next().unwrap_or("").to_owned();
                    let cookie = header.lines().filter_map(|line| line.split_once(':'))
                        .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
                        .map_or_else(String::new, |(_, value)| value.trim().to_owned());
                    requests.lock().unwrap().push(Request { method, path: path.clone(), cookie });
                    if path=="/timing-held" {
                        let body="<html><body><script>document.title='STALE timing';</script></body></html>";
                        let header=format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());let _=stream.write_all(header.as_bytes());
                        let open=gate.0.lock().unwrap();let _=gate.1.wait_timeout_while(open,Duration::from_secs(10),|open|!*open).unwrap();let _=stream.write_all(body.as_bytes());return;
                    }
                    if matches!(path.as_str(), "/search?q=test" | "/held.js"|"/timing-held") {
                        let open = gate.0.lock().unwrap();
                        let _ = gate.1.wait_timeout_while(open, Duration::from_secs(10), |open| !*open).unwrap();
                    }
                    let (mut status, mut mime, mut extra) = (200, "text/html", String::new());
                    let body = match path.as_str() {
                        "/timed"=>{std::thread::sleep(Duration::from_millis(15));"<html><head><title>Timing pending</title></head><body><script>var savedTiming=performance.timing,savedStamp=savedTiming.responseStart;if(savedTiming.navigationStart<=0||savedTiming.requestStart<savedTiming.navigationStart||savedStamp<savedTiming.requestStart||savedTiming.responseEnd<savedStamp||savedTiming.responseEnd>Date.now()||performance.now()+savedTiming.navigationStart+1<savedTiming.responseEnd)throw Error('actual timing unavailable before startup');document.title='Timing ready';</script></body></html>".into()},
                        "/timing-held"=>"<html><body><script>document.title='STALE timing';</script></body></html>".into(),
                        "/timing-fail"=>{status=204;String::new()},
                        "/home" => {
                            extra.push_str("Set-Cookie: sid=server; HttpOnly; Path=/\r\n");
                            HOME.to_owned()
                        }
                        "/boot.js" => { mime = "text/javascript"; BOOT.to_owned() }
                        "/search?q=test" => SEARCH.to_owned(),
                        "/csp" => {
                            extra.push_str("Content-Security-Policy: script-src 'nonce-Allowed'\r\n");
                            blocked_page("/forbidden.js", "CSP blocked")
                        }
                        "/cross-origin" => blocked_page(&format!("{}/forbidden-direct.js", other_origin.as_ref().unwrap()), "Origin blocked"),
                        "/redirect" => blocked_page("/redirect.js", "Redirect blocked"),
                        "/redirect.js" => {
                            status = 302;
                            extra = format!("Location: {}/forbidden-redirect.js\r\n", other_origin.as_ref().unwrap());
                            String::new()
                        }
                        "/held" => HELD.to_owned(),
                        "/held.js" => {
                            mime = "text/javascript";
                            "document.title='STALE script';document.cookie='late=bad; Path=/';location.assign('/escaped');".into()
                        }
                        "/fresh" => FRESH.to_owned(),
                        "/fresh.js" => {
                            mime = "text/javascript";
                            "if(document.cookie!=='')throw Error('stale script changed cookies');document.title='Fresh script ran';".into()
                        }
                        "/hash" => HASH.to_owned(),
                        "/hash.js" => {
                            mime = "text/javascript";
                            HASH_SCRIPT.to_owned()
                        }
                        "/after-hash.js" => {
                            mime = "text/javascript";
                            "if(location.hash!=='#changed' || ++scriptRuns!==2)throw Error('second hash execution');location.hash='#changed';".into()
                        }
                        _ => {
                            mime = "text/javascript";
                            "document.title='FORBIDDEN request executed';".into()
                        }
                    };
                    let response = format!("HTTP/1.1 {status} Test\r\nContent-Type: {mime};charset=utf-8\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = stream.write_all(response.as_bytes());
                }));
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            address,
            gate,
            stopped,
            requests,
            worker: Some(worker),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn release(&self) {
        *self.gate.0.lock().unwrap() = true;
        self.gate.1.notify_all();
    }

    fn request(&self, path: &str) -> Option<Request> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .find(|request| request.path == path)
            .cloned()
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.release();
        self.stopped.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn blocked_page(source: &str, title: &str) -> String {
    format!(
        r#"<!doctype html><html><head><title>Waiting for refusal</title></head>
<body><script nonce="Allowed">
var blocked=document.createElement('script');blocked.src={source:?};
blocked.onerror=function(event){{
  if(this!==blocked || event.target!==blocked || event.type!=='error')throw Error('error event');
  document.title={title:?};
}};
blocked.onload=function(){{document.title='Unexpected script load';}};
document.head.appendChild(blocked);
</script></body></html>"#
    )
}

fn until(app: &mut App, condition: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition(app) {
        app.poll_load();
        app.poll_script_timer();
        assert!(Instant::now() < deadline, "{}\n{}", app.status, report(app));
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn report(app: &App) -> String {
    app.scripting
        .as_ref()
        .map_or_else(String::new, |page| page.report())
}

fn no_script_errors(app: &App) {
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        report(app)
    );
}

#[test]
fn navigation_timing_reaches_actual_startup_and_only_matching_document_commit() {
    let peer=Peer::new(None);let mut app=App::new();app.open_address(&peer.url("/timed"));app.wait_for_load();assert!(!app.failed,"{}",app.status);assert_eq!(app.document.title,"Timing ready","{}",report(&app));no_script_errors(&app);
    let timing_line=|app:&App|report(app).lines().find(|l|l.starts_with("Measured document timing")).unwrap().to_owned();let original=timing_line(&app);
    app.open_address(&peer.url("/timed#same-document"));app.wait_for_load();assert_eq!(timing_line(&app),original);
    let outcome=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,"if(performance.timing!==savedTiming||savedTiming.responseStart!==savedStamp)throw 'same-document record changed';");assert_eq!(outcome.errors,0,"{}",report(&app));
    app.open_address(&peer.url("/timing-fail"));app.wait_for_load();assert_eq!(app.document.title,"Timing ready");assert_eq!(timing_line(&app),original);
    app.open_address(&peer.url("/timing-held"));until(&mut app,|app|app.network.snapshot().rows.iter().any(|r|r.url.ends_with("/timing-held")&&r.status==Some(200)&&r.http_bytes>0));assert_eq!(timing_line(&app),original);app.stop_loading();peer.release();app.wait_for_load();assert_eq!(app.document.title,"Timing ready");assert_eq!(timing_line(&app),original);
    app.reload();app.wait_for_load();assert!(!app.failed,"{}",app.status);assert_eq!(app.document.title,"Timing ready");no_script_errors(&app);assert_ne!(timing_line(&app),original);
}

#[test]
fn external_bootstrap_sends_session_cookies_and_replaces_history_after_real_response() {
    let peer = Peer::new(None);
    let mut app = App::new();
    app.open_address(&peer.url("/home"));
    until(&mut app, |_| peer.request("/search?q=test").is_some());
    // The response is still withheld: inspect the executing old realm before
    // navigation commits and its diagnostics are legitimately discarded.
    no_script_errors(&app);
    assert_eq!(app.document.title, "Home loaded");
    assert_eq!(app.location.display(), peer.url("/home"));
    assert_eq!(app.history.len(), 2);
    let request = peer.request("/search?q=test").unwrap();
    assert_eq!(request.method, "GET");
    assert_eq!(request.cookie, "sid=server; client=ready");
    assert_eq!(peer.request("/boot.js").unwrap().cookie, "sid=server");
    assert!(peer.request("/home").unwrap().cookie.is_empty());

    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert_eq!(app.document.title, "Results for ?q=test");
    assert_eq!(app.location.display(), peer.url("/search?q=test"));
    no_script_errors(&app);
    assert_eq!(
        app.history.len(),
        2,
        "replace must not append a third entry"
    );
    assert_eq!(app.history_index, 1);
    assert_eq!(
        app.history[1].location.display(),
        peer.url("/search?q=test")
    );
    assert!(peer
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|request| request.method == "GET"));
}

#[test]
fn dynamic_script_refusals_fire_real_error_events_without_forbidden_requests() {
    let sink = Peer::new(None);
    let peer = Peer::new(Some(sink.url("")));
    for (path, title, diagnostic) in [
        ("/csp", "CSP blocked", "Content Security Policy"),
        ("/cross-origin", "Origin blocked", "HTTPS document"),
        ("/redirect", "Redirect blocked", "origin"),
    ] {
        let mut app = App::new();
        app.open_address(&peer.url(path));
        until(&mut app, |app| !app.is_loading());
        assert_eq!(app.document.title, title, "{}", report(&app));
        // One fetch refusal is recorded; a broken error handler would add a
        // second error instead of completing this observable event path.
        assert_eq!(
            app.scripting.as_ref().unwrap().error_count(),
            1,
            "{}",
            report(&app)
        );
        assert!(report(&app).contains(diagnostic), "{}", report(&app));
    }
    assert!(peer.request("/forbidden.js").is_none());
    assert!(peer.request("/redirect.js").is_some());
    let requests = sink.requests.lock().unwrap();
    assert!(requests.is_empty(), "{requests:?}");
}

#[test]
fn navigation_and_stop_discard_inflight_and_queued_script_completions() {
    for mode in ["navigate", "stop", "queued"] {
        let peer = Peer::new(None);
        let mut app = App::new();
        app.open_address(&peer.url("/held"));
        until(&mut app, |app| {
            app.pending.is_none() && peer.request("/held.js").is_some()
        });
        assert_eq!(app.document.title, "Waiting for script");
        assert!(app.is_loading());
        no_script_errors(&app);
        if mode == "stop" {
            app.stop_loading();
            assert!(!app.is_loading());
            assert_eq!(app.document.title, "Waiting for script");
        } else if mode == "queued" {
            // Loader is idle; the dynamic worker's wake means its actual HTTP
            // completion is queued. Do not poll App before starting navigation.
            let completed = Arc::new(AtomicBool::new(false));
            let signal = completed.clone();
            app.set_load_wake(Arc::new(move || signal.store(true, Ordering::Release)));
            peer.release();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !completed.load(Ordering::Acquire) {
                assert!(
                    Instant::now() < deadline,
                    "dynamic completion did not arrive"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(app.document.title, "Waiting for script");
        }
        app.open_address(&peer.url("/fresh"));
        until(&mut app, |app| app.location.display() == peer.url("/fresh"));
        peer.release();
        // A later external script runs on the same single worker. Its load
        // event provides an ordering barrier after the old canceled job.
        until(&mut app, |app| !app.is_loading());
        assert_eq!(
            app.document.title,
            "Fresh document",
            "mode {mode}: {}",
            report(&app)
        );
        assert_eq!(app.location.display(), peer.url("/fresh"));
        assert!(peer.request("/fresh.js").is_some());
        assert!(peer.request("/escaped").is_none());
        assert!(app
            .cookies
            .document(app.location.url.as_ref().unwrap())
            .is_empty());
        no_script_errors(&app);
    }
}

#[test]
fn unchanged_hash_is_inert_and_fragment_navigation_preserves_external_requests() {
    let peer = Peer::new(None);
    let mut app = App::new();
    app.open_address(&peer.url("/hash#kept"));
    until(&mut app, |app| !app.is_loading());
    no_script_errors(&app);
    assert_eq!(app.document.title, "Hash finished", "{}", report(&app));
    assert_eq!(app.location.display(), peer.url("/hash#changed"));
    // The unactivated fragment change while scripts are still loading replaces
    // this entry. Assigning already-current hashes remains inert.
    assert_eq!(app.history.len(), 2);
    assert_eq!(app.history_index, 1);
    assert_eq!(app.history[1].location.display(), peer.url("/hash#changed"));
    let outcome = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document, &mut app.forms, "location.hash='#later';");
    assert_eq!(outcome.errors, 0, "{}", report(&app));
    app.process_script_requests();
    assert_eq!(app.history.len(), 3);
    assert_eq!(app.history_index, 2);
    assert_eq!(app.history[1].location.display(), peer.url("/hash#changed"));
    assert_eq!(app.history[2].location.display(), peer.url("/hash#later"));
    let requests = peer.requests.lock().unwrap();
    for path in ["/hash", "/hash.js", "/after-hash.js"] {
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.path == path)
                .count(),
            1,
            "{requests:?}"
        );
    }
    assert_eq!(requests.len(), 3, "{requests:?}");
}

#[test]
fn native_document_activation_excludes_chrome_reserved_keys_and_keyup() {
    for (input, replace) in [("none", true), ("chrome", true), ("pointer", false),
        ("keydown", false), ("reserved", true), ("keyup", true), ("escape", true)] {
        let peer = Peer::new(None);
        let mut app = App::new();
        app.open_address(&peer.url("/held"));
        until(&mut app, |app| app.scripting.is_some()
            && app.document.title == "Waiting for script");
        // The held external script keeps this genuine document incomplete.
        // Chrome interaction cannot give its Window transient activation.
        app.focus = Focus::Page;
        match input {
            "chrome" => { app.pointer_down(10, 10, false); }
            "pointer" => { app.pointer_down(200, app.content_top() + 80, false); }
            "keydown" => { app.key(0x0d, false, false, false); }
            "reserved" => { app.key(0x4c, true, false, false); }
            "keyup" => { app.key_event(crate::scripting::KeyboardInput {
                down: false, key: "Enter".into(), code: "Enter".into(), key_code: 0x0d,
                ..crate::scripting::KeyboardInput::default()
            }); }
            "escape" => { app.key(0x1b, false, false, false); }
            _ => {}
        }
        let result = app.scripting.as_mut().unwrap().console_eval(
            &mut app.document, &mut app.forms, "location.assign('/destination');");
        assert_eq!(result.errors, 0, "{input}: {}", report(&app));
        let request = app.scripting.as_mut().unwrap().take_navigation().unwrap();
        assert_eq!(request.replace, replace, "{input}");
        app.stop_loading();
        peer.release();
    }
}
