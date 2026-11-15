// Original, bounded actual App/HTTP regressions; included inside embedded tests.
mod network_media_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::{Condvar, Mutex};
    use std::thread::{self, JoinHandle};

    const IVF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tools/fixtures/scene-intra.ivf"));
    const AUTHORED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tools/fixtures/authored-video.html"));
    struct Site {
        address: SocketAddr,
        stopped: Arc<AtomicBool>,
        gate: Arc<(Mutex<bool>, Condvar)>,
        held: Arc<AtomicBool>,
        requests: Arc<Mutex<Vec<String>>>,
        thread: Option<JoinHandle<()>>,
    }
    impl Site {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let stopped = Arc::new(AtomicBool::new(false));
            let gate = Arc::new((Mutex::new(false), Condvar::new()));
            let held = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let (stop, release, waiting, log) = (stopped.clone(), gate.clone(), held.clone(), requests.clone());
            let thread = thread::spawn(move || {
                let mut children = Vec::new();
                let mut failure = None;
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if children.len() == 64 { failure = Some("connection allowance reached".to_owned()); break; }
                            let (stop, gate, held, log) = (stop.clone(), release.clone(), waiting.clone(), log.clone());
                            children.push(thread::spawn(move || serve(stream, address.port(), stop, gate, held, log)));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(1)),
                        Err(error) => { failure = Some(format!("accept: {error}")); break; }
                    }
                }
                stop.store(true, Ordering::Relaxed);
                *release.0.lock().unwrap() = true; release.1.notify_all();
                let mut clean = true;
                for child in children { clean &= child.join().is_ok(); }
                assert!(clean, "owned media fixture handler panicked");
                assert!(failure.is_none(), "media fixture listener: {failure:?}");
            });
            Self { address, stopped, gate, held, requests, thread: Some(thread) }
        }
        fn url(&self, path: &str) -> String { format!("http://{}{path}", self.address) }
        fn release(&self) { *self.gate.0.lock().unwrap() = true; self.gate.1.notify_all(); }
        fn count(&self, path: &str) -> usize { self.requests.lock().unwrap().iter().filter(|request| request.as_str() == path).count() }
        fn open(&self, path: &str) -> App {
            let mut app = App::new(); app.scripts_enabled = true; app.images_enabled = false;
            app.open_address(&self.url(path)); app
        }
        fn wait_held(&self, app: &mut App) {
            until(app, |app| self.held.load(Ordering::Relaxed)
                && app.embedded_video.as_ref().is_some_and(|video| video.plan.is_some()));
            assert!(app.media.is_none());
        }
    }
    impl Drop for Site {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::Relaxed); self.release();
            if let Some(thread) = self.thread.take() {
                let joined = thread.join();
                if !std::thread::panicking() { assert!(joined.is_ok(), "owned media fixture listener panicked"); }
            }
        }
    }
    fn until(app: &mut App, ready: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(6);
        while !ready(app) {
            assert!(Instant::now() < deadline, "media fixture did not settle: {}", app.status);
            app.poll_load(); app.poll_page_timer(); thread::sleep(Duration::from_millis(1));
        }
    }
    fn denied_page(source: &str, attributes: &str, policy: &str) -> String {
        format!("<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\"><title>Network refusal fixture</title></head><body><video id=\"authored-video\" src=\"{source}\" {attributes} controls></video><script>var video=document.getElementById('authored-video'),mediaEvents=[];video.addEventListener('error',function(){{mediaEvents.push('error');}});</script></body></html>")
    }
    fn serve(mut stream: TcpStream, port: u16, stop: Arc<AtomicBool>, gate: Arc<(Mutex<bool>, Condvar)>, held: Arc<AtomicBool>, log: Arc<Mutex<Vec<String>>>) {
        // Accepted sockets inherit the nonblocking listener mode on Windows.
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
        stream.set_write_timeout(Some(Duration::from_millis(250))).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut header = Vec::new(); let mut chunk = [0u8; 512];
        while !header.windows(4).any(|part| part == b"\r\n\r\n") {
            if stop.load(Ordering::Relaxed) || Instant::now() >= deadline || header.len() >= 16384 { return; }
            match stream.read(&mut chunk) {
                Ok(0) => return,
                Ok(count) => header.extend_from_slice(&chunk[..count]),
                Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
                Err(_) => return,
            }
        }
        let header = String::from_utf8_lossy(&header);
        let Some(path) = header.lines().next().and_then(|line| line.strip_prefix("GET ")).and_then(|line| line.split_once(' ').map(|pair| pair.0.to_owned())) else { return; };
        log.lock().unwrap().push(path.clone());
        let (mut status, mut mime, mut extra) = (200, "text/html", String::new());
        let body = match path.as_str() {
            "/page" => AUTHORED.as_bytes().to_vec(),
            "/redirect-page" => AUTHORED.replace("src=\"scene-intra.ivf\"", "src=\"redirect.ivf\"").into_bytes(),
            "/slow-page" => AUTHORED.replace("src=\"scene-intra.ivf\"", "src=\"slow.ivf\"").into_bytes(),
            "/mime-page" => denied_page("bad-mime.ivf", "", "media-src 'self'").into_bytes(),
            "/partial-page" => denied_page("partial.ivf", "", "media-src 'self'").into_bytes(),
            "/csp-page" => denied_page("csp-target.ivf", "", "media-src 'none'").into_bytes(),
            "/cross-page" => denied_page(&format!("http://localhost:{port}/cross-target.ivf"), "", "media-src *").into_bytes(),
            "/attribute-page" => denied_page("attribute-target.ivf", "crossorigin=\"anonymous\"", "media-src 'self'").into_bytes(),
            "/next" => b"<!doctype html><title>Network next document</title><p>No video in this document.</p>".to_vec(),
            "/redirect.ivf" => { status = 302; extra = "Location: /scene-intra.ivf\r\n".into(); Vec::new() }
            "/bad-mime.ivf" => { mime = "text/plain"; IVF.to_vec() }
            "/partial.ivf" => { status = 206; mime = "video/x-ivf"; extra = format!("Content-Range: bytes 0-{}/{}\r\n", IVF.len()-1, IVF.len()); IVF.to_vec() }
            "/scene-intra.ivf" | "/slow.ivf" | "/csp-target.ivf" | "/cross-target.ivf" | "/attribute-target.ivf" => { mime = "video/x-ivf"; IVF.to_vec() }
            _ => { status = 404; b"Original fixture route absent".to_vec() }
        };
        if write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n", body.len()).is_err() { return; }
        if path == "/slow.ivf" {
            if stream.write_all(&body[..32]).is_err() { return; }
            held.store(true, Ordering::Relaxed);
            let end = Instant::now() + Duration::from_secs(8);
            let mut released = gate.0.lock().unwrap();
            while !*released && !stop.load(Ordering::Relaxed) && Instant::now() < end {
                released = gate.1.wait_timeout(released, Duration::from_millis(20)).unwrap().0;
            }
            if !*released || stop.load(Ordering::Relaxed) { return; }
            let _ = stream.write_all(&body[32..]);
        } else { let _ = stream.write_all(&body); }
    }

    #[test]
    fn network_media_full_get_redirect_real_play_pause_seek_and_reference_pixels() {
        for path in ["/page", "/redirect-page"] {
            let site = Site::new(); let mut app = site.open(path); app.wait_for_load();
            assert!(!app.failed, "{}", app.status); assert!(app.media.is_some()); assert_reference_frame(&app, 0);
            eval(&mut app, "if(video.readyState!==4||video.networkState!==1||video.duration!==3||video.videoWidth!==64||video.videoHeight!==48||!mediaWindowLoaded)throw 'HTTP metadata';var networkSettles=0,networkPlay='pending';");
            activate_media_document(&mut app);
            eval(&mut app, "var networkPromise=video.play();if(!(networkPromise instanceof Promise)||video.paused)throw 'HTTP genuine play';networkPromise.then(function(){networkSettles++;networkPlay='fulfilled';},function(e){throw e;});");
            app.flush_embedded_video(); run(&mut app, Duration::from_millis(650));
            eval(&mut app, "if(networkPlay!=='fulfilled'||networkSettles!==1||video.currentTime<0.5)throw 'HTTP actual playback';video.pause();");
            app.flush_embedded_video(); settle(&mut app);
            let position = app.embedded_video_snapshot().unwrap().position; let frame = app.media.as_ref().unwrap().displayed;
            assert!(frame >= 1); assert_reference_frame(&app, frame); run(&mut app, Duration::from_millis(80));
            assert_eq!(app.embedded_video_snapshot().unwrap().position, position);
            eval(&mut app, "video.currentTime=2.2;"); app.flush_embedded_video(); settle(&mut app);
            assert_reference_frame(&app, 4);
            eval(&mut app, "if(!video.paused||video.seeking||video.currentTime!==2.2)throw 'HTTP settled seek';");
            assert_eq!(site.count("/scene-intra.ivf"), 1); assert_eq!(site.count("/redirect.ivf"), usize::from(path == "/redirect-page"));
        }
    }

    #[test]
    fn network_media_transport_refusals_and_policy_denials_do_not_install_a_player() {
        for (page, target, contacts) in [("/mime-page","/bad-mime.ivf",1),("/partial-page","/partial.ivf",1),
            ("/csp-page","/csp-target.ivf",0),("/cross-page","/cross-target.ivf",0),("/attribute-page","/attribute-target.ivf",0)] {
            let site = Site::new(); let mut app = site.open(page); app.wait_for_load();
            assert!(!app.failed, "document fetch failed: {}", app.status); assert!(app.media.is_none(), "{page}");
            assert!(app.embedded_video.as_ref().is_some_and(|video| video.error.is_some()), "{page}");
            assert_eq!(site.count(target), contacts, "{page}: policy denial must precede target contact");
            assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
            run(&mut app, Duration::from_millis(80)); assert_eq!(site.count(target), contacts, "automatic media retry: {page}");
        }
    }

    #[test]
    fn network_media_held_body_stop_replacement_navigation_and_real_late_result_are_owned() {
        for action in ["stop", "replace", "navigate"] {
            let site = Site::new(); let mut app = site.open("/slow-page"); site.wait_held(&mut app);
            let old = app.embedded_video.as_ref().unwrap().plan.clone().unwrap();
            match action {
                "stop" => app.stop_loading(),
                "replace" => { eval(&mut app, "video.src='scene-intra.ivf';"); app.process_media_selection(); }
                _ => app.open_address(&site.url("/next")),
            }
            site.release(); app.wait_for_load(); run(&mut app, Duration::from_millis(100));
            assert_eq!(site.count("/slow.ivf"), 1, "abandoned source retried");
            assert!(!app.embedded_video.as_ref().and_then(|video| video.plan.as_ref()).is_some_and(|plan| Arc::ptr_eq(plan, &old)));
            match action {
                "stop" => {
                    assert!(app.media.is_none()); run(&mut app, Duration::from_millis(80)); assert_eq!(site.count("/slow.ivf"),1);
                    eval(&mut app, "video.load();"); app.wait_for_load(); assert!(app.media.is_some()); assert_reference_frame(&app,0);
                    assert_eq!(site.count("/slow.ivf"),2,"only explicit load starts recovery");
                }
                "replace" => { assert_reference_frame(&app,0); assert_eq!(site.count("/scene-intra.ivf"),1); }
                _ => { assert_eq!(app.document.title,"Network next document"); assert!(app.media.is_none() && app.embedded_video.is_none() && app.images.is_empty()); }
            }
        }
        let site = Site::new(); let mut app = site.open("/slow-page"); site.wait_held(&mut app); site.release();
        let late = take_decoded_source(&app); // Actual full HTTP body + original decoder, not a fabricated completion.
        app.open_address(&site.url("/next")); app.wait_for_load();
        assert!(!app.accept_embedded_video(late)); run(&mut app,Duration::from_millis(80));
        assert_eq!(app.document.title,"Network next document"); assert!(app.media.is_none() && app.embedded_video.is_none() && app.images.is_empty());
    }
}
