//! Original held-transfer App ownership and wake-routing controls.
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let mut nonce = [0u8;8]; crate::random::fill(&mut nonce).unwrap();
        let token: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("chezcat-shared-download-app-{token}"));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn folder(&self) -> PathBuf { self.0.join("Downloads") }
    fn configure(&self, app: &mut App) {
        let mut data = app.profile.data().clone();
        data.settings.download_folder = Some(self.folder().to_string_lossy().into_owned());
        app.profile.save(data).unwrap();
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

struct Held {
    location: Location,
    observed: mpsc::Receiver<()>,
    release: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Held {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let location = navigation::address(&format!("http://{}/payload.bin", listener.local_addr().unwrap()), &Location::home()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let (seen, observed) = mpsc::channel(); let (release, gate) = mpsc::channel();
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream,_)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "Missing owned download request");
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("Owned download accept failed: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut request = Vec::new(); let mut byte = [0u8;1];
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 4096); stream.read_exact(&mut byte).unwrap(); request.push(byte[0]);
            }
            assert!(request.starts_with(b"GET /payload.bin HTTP/1.1\r\n"));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 7\r\nConnection: close\r\n\r\n").unwrap();
            seen.send(()).unwrap();
            let _ = gate.recv_timeout(Duration::from_secs(5));
            let _ = stream.write_all(b"payload");
        });
        Self { location, observed, release: Some(release), thread: Some(thread) }
    }
    fn wait(&self) { self.observed.recv_timeout(Duration::from_secs(3)).unwrap(); }
    fn release(&mut self) {
        if let Some(release) = self.release.take() { let _ = release.send(()); }
        if let Some(thread) = self.thread.take() { thread.join().unwrap(); }
    }
}
impl Drop for Held { fn drop(&mut self) { self.release(); } }

fn finish_app(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_downloads();
        if !app.downloads_active() { break; }
        assert!(Instant::now() < deadline, "Owned App download did not finish");
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn shared_download_apps_origin_drop_preserves_session_transfer_and_peer_observation() {
    let temp = Temp::new(); let mut held = Held::new(); let shared = downloads::Shared::new();
    let browser_calls = Arc::new(AtomicUsize::new(0)); let page_calls = Arc::new(AtomicUsize::new(0));
    let count = browser_calls.clone();
    shared.set_wake(Some(Arc::new(move || { count.fetch_add(1, Ordering::SeqCst); }))).unwrap();
    let mut first = App::new(); let mut peer = App::new();
    first.attach_shared_downloads(&shared).unwrap(); peer.attach_shared_downloads(&shared).unwrap();
    assert!(first.owned_downloads.is_none() && peer.owned_downloads.is_none());
    temp.configure(&mut first);
    let count = page_calls.clone(); first.set_load_wake(Arc::new(move || { count.fetch_add(1, Ordering::SeqCst); }));
    let count = page_calls.clone(); peer.set_load_wake(Arc::new(move || { count.fetch_add(1, Ordering::SeqCst); }));
    let peer_state = (peer.raw.clone(), peer.location.display(), peer.script_epoch, peer.history.len(), peer.history_index);
    first.download_location(held.location.clone(), None); held.wait();
    assert!(first.downloads_open()); assert!(!peer.downloads_open(), "Popup state must remain App-local");
    let id = first.downloads.entries()[0].id;
    let extra_view = first.downloads.clone(); drop(extra_view); drop(first);
    assert_eq!(shared.active_count().unwrap(), 1);
    held.release();
    let deadline = Instant::now() + Duration::from_secs(5);
    while shared.active_count().unwrap() != 0 {
        shared.poll().unwrap(); assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    assert!(peer.poll_downloads());
    let entry = &peer.downloads.entries()[0];
    assert_eq!(entry.id, id); assert_eq!(entry.status, Status::Complete);
    assert_eq!(std::fs::read(entry.path.as_ref().unwrap()).unwrap(), b"payload");
    assert_eq!((peer.raw.clone(), peer.location.display(), peer.script_epoch, peer.history.len(), peer.history_index), peer_state);
    assert!(!peer.downloads_open());
    assert_eq!(page_calls.load(Ordering::SeqCst), 0, "A page wake retargeted browser-owned downloads");
    assert!(browser_calls.load(Ordering::SeqCst) > 0);
    peer.show_downloads(); assert!(peer.downloads_open());
    assert_eq!(peer.downloads.entries()[0].id, id);
    drop(peer); drop(shared);
}

#[test]
fn shared_download_app_default_authority_survives_construction_and_legacy_wake_retarget() {
    let temp = Temp::new(); let mut held = Held::new(); let mut app = App::new();
    assert!(app.owned_downloads.is_some()); temp.configure(&mut app);
    let marker = Arc::new(()); let weak = Arc::downgrade(&marker); let captured = marker.clone();
    app.set_load_wake(Arc::new(move || { let _ = Arc::strong_count(&captured); })); drop(marker);
    app.download_location(held.location.clone(), None); held.wait();
    let id = app.downloads.entries()[0].id;
    let calls = Arc::new(AtomicUsize::new(0)); let count = calls.clone();
    app.set_load_wake(Arc::new(move || { count.fetch_add(1, Ordering::SeqCst); }));
    // Allow an already-cloned dispatcher callback to return; retargeting does
    // not promise instantaneous recall of a callback already in flight.
    let deadline = Instant::now() + Duration::from_secs(2);
    while weak.upgrade().is_some() { assert!(Instant::now() < deadline); thread::yield_now(); }
    held.release(); finish_app(&mut app);
    let entry = &app.downloads.entries()[0];
    assert_eq!(entry.id, id); assert_eq!(entry.status, Status::Complete);
    assert_eq!(std::fs::read(entry.path.as_ref().unwrap()).unwrap(), b"payload");
    assert!(calls.load(Ordering::SeqCst) > 0);
    drop(app);
}

#[test]
fn shared_download_app_startup_attach_refuses_retained_active_and_completed_work() {
    let temp = Temp::new(); let mut held = Held::new(); let mut app = App::new();
    temp.configure(&mut app); app.download_location(held.location.clone(), None); held.wait();
    let id = app.downloads.entries()[0].id; let target = downloads::Shared::new();
    assert!(app.attach_shared_downloads(&target).unwrap_err().contains("before"));
    assert!(app.owned_downloads.is_some()); assert_eq!(app.downloads.entries()[0].id, id);
    assert_eq!(target.active_count().unwrap(), 0);
    held.release(); finish_app(&mut app);
    let saved = app.downloads.entries()[0].path.clone().unwrap();
    assert_eq!(app.downloads.entries()[0].status, Status::Complete);
    assert!(app.attach_shared_downloads(&target).unwrap_err().contains("before"));
    assert!(app.owned_downloads.is_some()); assert_eq!(app.downloads.entries()[0].id, id);
    assert_eq!(std::fs::read(saved).unwrap(), b"payload");
    assert!(target.view().unwrap().entries().is_empty());
    drop(app); drop(target);
}

#[test]
fn shared_download_app_closed_authority_surfaces_errors_without_recreating_downloads() {
    let temp = Temp::new(); let shared = downloads::Shared::new(); let mut app = App::new();
    app.attach_shared_downloads(&shared).unwrap(); temp.configure(&mut app); drop(shared);
    app.show_downloads(); assert!(app.downloads_ui.error.contains("closed"));
    app.poll_downloads(); assert!(app.status.contains("closed"));
    app.activate_download_action(Action::Clear);
    assert!(app.downloads_ui.error.contains("closed")); assert!(!app.status.starts_with("Cleared"));
    app.activate_download_action(Action::Cancel(1)); assert!(app.downloads_ui.error.contains("closed"));
    let source = temp.0.join("input.bin"); std::fs::write(&source, b"original").unwrap();
    app.download_location(navigation::explicit(&source).unwrap(), None);
    assert!(app.downloads_ui.error.contains("closed")); assert!(app.downloads.entries().is_empty());
    assert!(!temp.folder().exists(), "A closed session reached filesystem admission");
    app.activate_download_action(Action::Close); assert!(!app.downloads_open());
    drop(app);
}
