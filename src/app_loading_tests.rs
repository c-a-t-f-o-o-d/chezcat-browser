//! Real App navigation against a peer that withholds image bytes until released.
use super::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const MARKUP: &str = r##"<!doctype html><html><head><title>Before startup</title>
<link rel="stylesheet" href="/style.css"></head><body>
<h1>Styled and interactive</h1><input id="entry" value="original">
<button id="change" type="button">Change counter</button>
<p id="state">not ready</p><p id="timer">not run</p><p id="loaded">not loaded</p>
<a href="#tail">Tail</a><img id="image" src="/held.ppm"><p id="tail">Tail anchor</p>
<script>
var count=1; var loads=0;
document.title='Interactive page';
document.getElementById('change').onclick=function(){count+=5;};
document.addEventListener('DOMContentLoaded',function(){
  document.getElementById('state').textContent=document.readyState;
});
setTimeout(function(){document.getElementById('timer').textContent='timer-'+count;},0);
window.addEventListener('load',function(){
  loads+=1;
  document.title='Loaded '+count;
  document.getElementById('loaded').textContent=document.readyState+'-'+loads+'-'+document.getElementById('entry').value;
});
</script></body></html>"##;

struct Peer {
    address: SocketAddr,
    gate: Arc<(Mutex<bool>, Condvar)>,
    stopped: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Peer {
    fn new() -> Self {
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
                workers.push(std::thread::spawn(move || {
                    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut bytes = Vec::new();
                    let mut byte = [0u8; 1];
                    while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 32768 {
                        if stream.read_exact(&mut byte).is_err() { return; }
                        bytes.push(byte[0]);
                    }
                    let request = String::from_utf8_lossy(&bytes);
                    let first = request.lines().next().unwrap_or("");
                    let mut words = first.split_whitespace();
                    let method = words.next().unwrap_or("").to_owned();
                    let path = words.next().unwrap_or("").to_owned();
                    let length = request.lines().filter_map(|line| line.split_once(':'))
                        .find(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
                        .and_then(|(_, value)| value.trim().parse::<usize>().ok()).unwrap_or(0);
                    if length > crate::http::MAX_FORM_BYTES { return; }
                    let mut body = vec![0; length];
                    if stream.read_exact(&mut body).is_err() { return; }
                    requests.lock().unwrap().push((method, path.clone()));
                    if path == "/post-redirect" {
                        let _ = stream.write_all(b"HTTP/1.1 303 See Other\r\nLocation: /page\r\nContent-Length: 0\r\n\r\n");
                        return;
                    }
                    let (content_type, body) = if path == "/held.ppm" {
                        let open = gate.0.lock().unwrap();
                        let _ = gate.1.wait_timeout_while(open, Duration::from_secs(5), |open| !*open).unwrap();
                        let mut image = b"P6\n1 1800\n255\n".to_vec();
                        image.extend(std::iter::repeat_n([32, 96, 224], 1800).flatten());
                        ("image/x-portable-pixmap", image)
                    } else if path == "/style.css" {
                        ("text/css", b"body { background:#eafbee; } h1 { color:#123456; }".to_vec())
                    } else if path == "/fresh" {
                        ("text/html", b"<title>Fresh document</title><p id='fresh'>Replacement</p>".to_vec())
                    } else if path == "/hide" {
                        ("text/html", MARKUP.replace("loads+=1;", "loads+=1;document.getElementById('entry').style.display='none';").into_bytes())
                    } else if path == "/geometry" {
                        ("text/html", b"<html><body><img id='picture' src='/held.ppm'><script>var first=document.getElementById('picture').clientHeight;window.onload=function(){document.title=document.getElementById('picture').clientHeight+','+document.documentElement.clientWidth;};</script></body></html>".to_vec())
                    } else if path == "/location" {
                        ("text/html", b"<html><body><img src='/held.ppm'><script>var saved=location,stringify=location.toString;document.title=saved.href;window.onload=function(){if(saved!==document.location || stringify!==location.toString)throw Error('location identity');document.title=stringify.call(saved)+'|'+document.URL+'|'+document.documentURI;};</script></body></html>".to_vec())
                    } else {
                        ("text/html", MARKUP.as_bytes().to_vec())
                    };
                    let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n", body.len());
                    let _ = stream.write_all(headers.as_bytes()).and_then(|_| stream.write_all(&body));
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
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.release();
        self.stopped.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn until(app: &mut App, condition: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition(app) {
        app.poll_load();
        assert!(Instant::now() < deadline, "{}", app.status);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn id(app: &App, value: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(value)).then_some(node))
        .unwrap()
}
fn text(app: &App, value: &str) -> String {
    let mut stack = vec![id(app, value)];
    let mut result = String::new();
    while let Some(node) = stack.pop() {
        if let html::NodeKind::Text(value) = &app.document.nodes[node].kind {
            result.push_str(value);
        }
        stack.extend(app.document.nodes[node].children.iter().rev().copied());
    }
    result
}
fn displayed(app: &App) -> bool {
    app.pending
        .as_ref()
        .is_some_and(|pending| pending.committed)
}

#[test]
fn load_event_location_keeps_fragment_history_changes_while_images_finish() {
    let peer = Peer::new();
    let mut app = App::new();
    let first = peer.url("/location#first");
    let next = peer.url("/location#next%20part");
    app.open_address(&first);
    until(&mut app, displayed);
    assert_eq!(
        app.document.title,
        first,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    app.open_address(&next);
    assert!(displayed(&app));
    app.history_move(-1);
    assert!(displayed(&app));
    app.history_move(1);
    assert!(displayed(&app));
    app.show_resources();
    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert_eq!(app.location.display(), next);
    assert_eq!(app.document.title, vec![next; 3].join("|"));
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}

#[test]
fn load_handler_geometry_uses_finished_images_while_resource_report_is_open() {
    let peer = Peer::new();
    let mut app = App::new();
    app.open_address(&peer.url("/geometry"));
    until(&mut app, displayed);
    assert!(app.images.is_empty());
    app.show_resources();
    app.resize(900, 600);
    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert_eq!(app.document.title, "1800,888");
    assert_eq!(
        app.scripting.as_ref().unwrap().error_count(),
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
}

#[test]
fn styled_document_accepts_input_and_timers_before_images_then_fires_load_once() {
    let peer = Peer::new();
    let mut app = App::new();
    app.browsing_history
        .configure(
            crate::history::Policy {
                enabled: true,
                ..Default::default()
            },
            crate::history::now(),
        )
        .unwrap();
    app.open_address(&peer.url("/page"));
    until(&mut app, displayed);
    assert!(app.is_loading() && app.images.is_empty());
    assert_eq!(app.document.title, "Interactive page");
    assert_eq!(app.browsing_history.data().entries.len(), 1);
    assert_eq!(
        app.browsing_history.data().entries[0].url,
        peer.url("/page")
    );
    assert_eq!(text(&app, "state"), "interactive");
    assert_eq!(text(&app, "loaded"), "not loaded");
    assert_eq!(app.history.len(), 2);
    assert!(
        app.draw()
            .pixels
            .iter()
            .filter(|pixel| **pixel & 0x00ffffff == 0xeafbee)
            .count()
            > 1000
    );
    app.activate_control(id(&app, "change"));
    let entry = id(&app, "entry");
    app.focus_control(entry);
    app.key(0x41, true, false, false);
    app.paste("edited while loading");
    assert!(app.poll_script_timer());
    assert_eq!(text(&app, "timer"), "timer-6");
    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert_eq!(app.images.len(), 1);
    assert_eq!(app.document.title, "Loaded 6");
    assert_eq!(text(&app, "loaded"), "complete-1-edited while loading");
    assert_eq!(text(&app, "timer"), "timer-6");
    assert!(app.focus == Focus::Control(entry));
    assert_eq!(
        app.forms.controls[&entry].value.text(),
        "edited while loading"
    );
    assert_eq!(app.history.len(), 2);
    app.poll_load();
    assert_eq!(text(&app, "loaded"), "complete-1-edited while loading");
    assert!(app
        .scripting
        .as_ref()
        .unwrap()
        .report()
        .contains("1 script(s)"));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
    assert_eq!(app.browsing_history.data().entries.len(), 1);
}

#[test]
fn fragment_and_history_navigation_keep_the_loading_document_and_restore_the_target() {
    let peer = Peer::new();
    let mut app = App::new();
    app.open_address(&peer.url("/page"));
    until(&mut app, displayed);
    let request = app.pending.as_ref().unwrap().id;
    app.open_address(&peer.url("/page#tail"));
    assert_eq!(app.pending.as_ref().unwrap().id, request);
    assert_eq!(app.location.fragment, "tail");
    app.history_move(-1);
    assert!(app.location.fragment.is_empty());
    assert_eq!(app.pending.as_ref().unwrap().id, request);
    app.history_move(1);
    assert_eq!(app.location.fragment, "tail");
    app.follow("#tail");
    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert!(
        app.scroll > 1000,
        "fragment was left at its pre-image position: {}",
        app.scroll
    );
    assert_eq!(app.history.len(), 3);
    assert_eq!(app.images.len(), 1);
    let docs = peer
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, path)| path == "/page")
        .count();
    assert_eq!(docs, 1);
}

#[test]
fn stop_retains_the_interactive_page_and_replacement_discards_late_images() {
    let peer = Peer::new();
    let mut app = App::new();
    app.open_address(&peer.url("/page"));
    until(&mut app, displayed);
    app.stop_loading();
    assert!(!app.is_loading());
    assert_eq!(app.document.title, "Interactive page");
    assert!(app.poll_script_timer());
    assert_eq!(text(&app, "loaded"), "not loaded");
    app.open_address(&peer.url("/fresh"));
    until(&mut app, |app| !app.is_loading());
    peer.release();
    assert_eq!(app.document.title, "Fresh document");
    assert!(app.images.is_empty());
    assert_eq!(app.history.len(), 3);
    app.poll_load();
    assert_eq!(app.document.title, "Fresh document");
    assert!(app.images.is_empty() && app.next_script_timer().is_none());
}

#[test]
fn reload_of_a_displayed_post_redirect_uses_the_final_get_without_resubmission() {
    let peer = Peer::new();
    let mut app = App::new();
    app.browsing_history
        .configure(
            crate::history::Policy {
                enabled: true,
                ..Default::default()
            },
            crate::history::now(),
        )
        .unwrap();
    let location = navigation::resolve(&peer.url("/post-redirect"), &Location::home()).unwrap();
    app.start_network_request(
        location,
        NavigationKind::New,
        crate::http::FetchMode::Normal,
        crate::http::Request::FormPost {
            body: b"q=one".to_vec(),
            origin: None,
        },
    );
    until(&mut app, displayed);
    assert_eq!(app.browsing_history.data().entries.len(), 1);
    assert_eq!(
        app.browsing_history.data().entries[0].url,
        peer.url("/page")
    );
    assert!(!app.current_request.is_post());
    assert!(!app.pending.as_ref().unwrap().request.is_post());
    app.reload();
    assert!(app.resubmission.is_none());
    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert_eq!(app.history.len(), 2);
    assert_eq!(app.browsing_history.data().entries.len(), 2);
    assert!(app
        .browsing_history
        .data()
        .entries
        .iter()
        .all(|entry| entry.url == peer.url("/page")));
    assert!(peer
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, path)| path == "/post-redirect")
        .all(|(method, _)| method == "POST"));
    assert_eq!(
        peer.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(method, _)| method == "POST")
            .count(),
        1
    );
}

#[test]
fn resource_updates_preserve_address_and_find_editing() {
    let peer = Peer::new();
    let mut app = App::new();
    app.open_address(&peer.url("/page"));
    until(&mut app, displayed);
    app.show_resources();
    assert!(app.view == View::Resources);
    app.key(0x46, true, false, false);
    app.paste("DISPLAY READY");
    assert!(app.focus == Focus::Find);
    assert!(app.poll_script_timer());
    assert!(app.focus == Focus::Find);
    assert_eq!(app.find.text(), "DISPLAY READY");
    app.key(0x4c, true, false, false);
    app.paste("https://typing.example/path");
    let page_scroll = app.page_scroll;
    app.scroll_by(100);
    let report_scroll = app.scroll;
    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert!(app.view == View::Resources && app.focus == Focus::Address);
    assert_eq!(app.address.text(), "https://typing.example/path");
    assert_eq!(app.find.text(), "DISPLAY READY");
    assert_eq!(app.page_scroll, page_scroll);
    assert_eq!(app.scroll, report_scroll);
}

#[test]
fn deferred_load_cannot_leave_keyboard_focus_in_a_hidden_control() {
    let peer = Peer::new();
    let mut app = App::new();
    app.open_address(&peer.url("/hide"));
    until(&mut app, displayed);
    let entry = id(&app, "entry");
    app.focus_control(entry);
    assert!(app.focus == Focus::Control(entry));
    peer.release();
    until(&mut app, |app| !app.is_loading());
    assert!(app.focus == Focus::Page);
    assert!(!app
        .page
        .controls
        .iter()
        .any(|control| control.node == entry));
    app.text_input('x');
    assert_eq!(app.forms.controls[&entry].value.text(), "original");
}
