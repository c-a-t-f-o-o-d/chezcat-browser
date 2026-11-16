//! Actual navigation-to-download handoff: the POST is sent once, its received
//! body is saved, and no document/history commit or stale delivery is allowed.
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PAYLOAD: &[u8] = b"received POST bytes\0\xff\x80\r\nunchanged";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    method: String,
    path: String,
    body: Vec<u8>,
}
struct Site {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    fn new(slow: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!(
            "http://127.0.0.1:{}/form",
            listener.local_addr().unwrap().port()
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(!slow));
        let (requests, ending, gate) = (seen.clone(), stop.clone(), release.clone());
        let thread = std::thread::spawn(move || {
            let mut clients = Vec::new();
            while !ending.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // Accepted Winsock sockets inherit the polling listener mode.
                        stream.set_nonblocking(false).unwrap();
                        let (requests, ending, gate) =
                            (requests.clone(), ending.clone(), gate.clone());
                        clients.push(std::thread::spawn(move || {
                            serve(stream, &requests, &ending, &gate)
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("attachment loopback accept: {error}"),
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            url,
            seen,
            stop,
            release,
            thread: Some(thread),
        }
    }
    fn app(&self, folder: &std::path::Path) -> App {
        let mut app = App::new();
        let mut data = app.profile.data().clone();
        data.settings.download_folder = Some(folder.to_string_lossy().into_owned());
        app.profile.save(data).unwrap();
        app.configure_browsing_history(&app.browsing_history.policy_token(),
                crate::history::Policy {
                    enabled: true,
                    ..Default::default()
                },
                crate::history::now(),
            )
            .unwrap();
        app.open_address(&self.url);
        wait_until(|| {
            app.poll_load();
            !app.is_loading()
        });
        assert!(!app.failed, "{}", app.status);
        assert_eq!(app.document.title, "Keep this POST form");
        assert_eq!(app.browsing_history.data().entries.len(), 1);
        app
    }
    fn assert_one_post(&self) {
        assert_eq!(
            *self.seen.lock().unwrap(),
            vec![
                Seen {
                    method: "GET".into(),
                    path: "/form".into(),
                    body: vec![]
                },
                Seen {
                    method: "POST".into(),
                    path: "/save".into(),
                    body: b"q=hello+world&token=unchanged&action=save".to_vec()
                },
            ]
        );
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.release.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            if let Err(failure) = thread.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(failure);
                }
            }
        }
    }
}
fn serve(mut stream: TcpStream, requests: &Mutex<Vec<Seen>>, stop: &AtomicBool, gate: &AtomicBool) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut head = Vec::new();
    let mut byte = [0];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            return;
        }
        head.push(byte[0]);
        assert!(head.len() < 32768);
    }
    let head = String::from_utf8(head).unwrap();
    let mut line = head.split_whitespace();
    let method = line.next().unwrap().to_owned();
    let path = line.next().unwrap().to_owned();
    let count = head
        .lines()
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
        })
        .unwrap_or(0);
    assert!(count < 32768);
    let mut body = vec![0; count];
    stream.read_exact(&mut body).unwrap();
    requests.lock().unwrap().push(Seen {
        method,
        path: path.clone(),
        body,
    });
    let (mime, disposition, body): (&str, &str, &[u8]) = if path == "/form" {
        ("text/html", "", b"<!doctype html><title>Keep this POST form</title><form method=post action=/save><input name=q value='hello world'><input type=hidden name=token value=unchanged><button id=send name=action value=save>Save</button></form>")
    } else {
        while !gate.load(Ordering::Acquire) && !stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(2));
        }
        (
            "application/octet-stream",
            "Content-Disposition: attachment; filename=received.bin\r\n",
            PAYLOAD,
        )
    };
    let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\n{disposition}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    if stream.write_all(head.as_bytes()).is_ok() {
        let _ = stream.write_all(body);
    }
}

struct Folder(std::path::PathBuf);
impl Folder {
    fn new() -> Self {
        let mut nonce = [0; 8];
        crate::random::fill(&mut nonce).unwrap();
        let name: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = std::env::temp_dir().join(format!("chezcat-attachment-{name}"));
        std::fs::create_dir(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(Instant::now() < deadline, "attachment test deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn submit(app: &mut App) {
    let node = app
        .document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some("send")).then_some(node))
        .unwrap();
    app.activate_control(node);
    assert!(app.is_loading());
}
fn count_wakes(app: &mut App) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let signal = count.clone();
    app.set_load_wake(Arc::new(move || {
        signal.fetch_add(1, Ordering::AcqRel);
    }));
    count
}
#[derive(Debug, PartialEq, Eq)]
struct PageState {
    location: String,
    raw: String,
    title: String,
    request: crate::http::Request,
    scroll: i32,
    events: Vec<String>,
    index: usize,
    session_history: Vec<(String, i32, crate::http::Request)>,
    browsing_history: crate::history::Data,
}
fn page_state(app: &App) -> PageState {
    PageState {
        location: app.location.display(),
        raw: app.raw.clone(),
        title: app.document.title.clone(),
        request: app.current_request.clone(),
        scroll: app.scroll,
        events: app.events.clone(),
        index: app.history_index,
        session_history: app
            .history
            .iter()
            .map(|entry| {
                (
                    entry.location.display(),
                    entry.scroll,
                    entry.request.clone(),
                )
            })
            .collect(),
        browsing_history: app.browsing_history.data().clone(),
    }
}

#[test]
fn received_post_attachment_is_saved_once_without_document_or_history_commit() {
    let folder = Folder::new();
    let site = Site::new(false);
    let mut app = site.app(&folder.0.join("Downloads"));
    let before = page_state(&app);
    submit(&mut app);
    wait_until(|| {
        app.poll_load();
        !app.is_loading() && !app.downloads_active()
    });
    site.assert_one_post();
    assert_eq!(page_state(&app), before);
    assert_eq!(app.address.text(), app.location.display());
    assert!(app.downloads_open());
    assert!(!app.failed, "{}", app.status);
    assert!(
        app.status.starts_with("Downloaded received.bin"),
        "{}",
        app.status
    );
    assert_eq!(app.downloads.entries().len(), 1);
    let entry = &app.downloads.entries()[0];
    assert_eq!(entry.status, Status::Complete);
    assert_eq!(entry.bytes, PAYLOAD.len() as u64);
    assert_eq!(
        std::fs::read(entry.path.as_ref().unwrap()).unwrap(),
        PAYLOAD
    );
    assert_eq!(app.network.snapshot().rows.len(), 1);
    assert_eq!(app.network.snapshot().rows[0].method, "POST");
}

#[test]
fn coalesced_attachment_and_progress_preserve_immediate_destination_error() {
    let folder = Folder::new();
    let invalid = folder.0.join("occupied-file");
    std::fs::write(&invalid, b"must not be changed").unwrap();
    let site = Site::new(false);
    let mut app = site.app(&invalid);
    let before = page_state(&app);
    let wakes = count_wakes(&mut app);
    submit(&mut app);
    // Do not poll until progress, Download and completion have all been queued.
    // This reproduces the actual mailbox coalescing that hid download errors.
    wait_until(|| wakes.load(Ordering::Acquire) >= 3);
    assert!(app.poll_load());
    assert!(!app.is_loading());
    assert!(app.status.starts_with("Download: "), "{}", app.status);
    assert!(!app.downloads_ui.error.is_empty());
    assert_eq!(app.status, format!("Download: {}", app.downloads_ui.error));
    assert!(!app.status.contains("Esc stops"));
    assert!(app.downloads_open());
    assert!(!app.failed); // The displayed document itself is still valid.
    assert!(app.downloads.entries().is_empty());
    assert_eq!(std::fs::read(&invalid).unwrap(), b"must not be changed");
    let error = app.status.clone();
    app.poll_load();
    assert_eq!(app.status, error);
    assert_eq!(page_state(&app), before);
    site.assert_one_post();
}

#[test]
fn stopped_attachment_posts_final_trace_wake_without_stale_delivery() {
    let folder = Folder::new();
    let site = Site::new(true);
    let mut app = site.app(&folder.0.join("Downloads"));
    let before = page_state(&app);
    let wakes = count_wakes(&mut app);
    submit(&mut app);
    wait_until(|| site.seen.lock().unwrap().len() == 2);
    app.poll_load();
    app.stop_loading();
    let after_stop = wakes.load(Ordering::Acquire);
    site.release.store(true, Ordering::Release);
    // Stop removed pending and disabled activity ticks. The completed canceled
    // worker must still post one signal to repaint its measured final state.
    wait_until(|| wakes.load(Ordering::Acquire) > after_stop);
    assert!(app.poll_load());
    assert!(!app.is_loading());
    assert!(app.downloads.entries().is_empty());
    assert!(!app.downloads_open());
    assert_eq!(app.status, "Loading stopped.");
    assert_eq!(page_state(&app), before);
    assert_eq!(app.address.text(), app.location.display());
    let trace = app.network.snapshot();
    assert_eq!(trace.rows.len(), 1);
    assert!(trace.rows[0].end.is_some());
    assert_eq!(trace.rows[0].outcome, crate::network_trace::Outcome::Failed);
    site.assert_one_post();
}
