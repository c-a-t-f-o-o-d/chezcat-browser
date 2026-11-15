// Original actual App/CORS regressions; include inside app_embedded_video::tests.
mod cors_app_media_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::Mutex;
    use std::thread::{self, JoinHandle};

    const IVF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tools/fixtures/scene-intra.ivf"));
    const AUTHORED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tools/fixtures/authored-video.html"));
    #[derive(Clone, Debug)]
    struct Request { path: String, origin: Option<String>, cookie: Option<String> }
    struct Reply { mime: &'static str, headers: String, body: Vec<u8> }
    struct Site {
        address: SocketAddr,
        stopped: Arc<AtomicBool>,
        requests: Arc<Mutex<Vec<Request>>>,
        worker: Option<JoinHandle<()>>,
    }
    impl Site {
        fn spawn(listener: TcpListener, routes: Vec<(&'static str, Reply)>) -> Self {
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let stopped = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let (stop, log) = (stopped.clone(), requests.clone());
            let worker = thread::spawn(move || {
                let mut contacts = 0;
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            contacts += 1; assert!(contacts <= 64, "original CORS fixture connection allowance");
                            // One owned thread per origin; each request has finite
                            // read/write bounds, and no detached handler survives Drop.
                            serve(stream, &routes, &stop, &log);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(1)),
                        Err(error) => panic!("original CORS fixture accept: {error}"),
                    }
                }
            });
            Self { address, stopped, requests, worker: Some(worker) }
        }
        fn url(&self, path: &str) -> String { format!("http://{}{path}", self.address) }
        fn requests(&self, path: &str) -> Vec<Request> {
            self.requests.lock().unwrap().iter().filter(|request| request.path == path).cloned().collect()
        }
    }
    impl Drop for Site {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::Relaxed);
            if let Some(worker) = self.worker.take() {
                let result = worker.join();
                if !thread::panicking() { assert!(result.is_ok(), "owned CORS fixture thread panicked"); }
            }
        }
    }
    fn header(request: &str, name: &str) -> Option<String> {
        request.lines().skip(1).filter_map(|line| line.split_once(':'))
            .find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.trim().to_owned())
    }
    fn has_cookie(request: &Request, pair: &str) -> bool {
        request.cookie.as_deref().unwrap_or("").split(';').any(|value| value.trim() == pair)
    }
    fn serve(mut stream: TcpStream, routes: &[(&str, Reply)], stopped: &AtomicBool, log: &Mutex<Vec<Request>>) {
        // Winsock accepts inherit the listener's nonblocking flag.
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
        stream.set_write_timeout(Some(Duration::from_millis(250))).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut bytes = Vec::new(); let mut chunk = [0u8; 512];
        while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
            if stopped.load(Ordering::Relaxed) || Instant::now() >= deadline || bytes.len() >= 16384 { return; }
            match stream.read(&mut chunk) {
                Ok(0) => return,
                Ok(count) => bytes.extend_from_slice(&chunk[..count]),
                Err(error) if matches!(error.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => continue,
                Err(_) => return,
            }
        }
        let text = String::from_utf8_lossy(&bytes);
        let Some(path) = text.lines().next().and_then(|line| line.strip_prefix("GET "))
            .and_then(|line| line.split_once(' ').map(|pair| pair.0.to_owned())) else { return; };
        log.lock().unwrap().push(Request { path: path.clone(), origin: header(&text, "Origin"), cookie: header(&text, "Cookie") });
        let reply = routes.iter().find(|(route, _)| *route == path).map(|(_, reply)| reply);
        let missing = Reply { mime: "text/plain", headers: String::new(), body: b"Original CORS fixture route absent".to_vec() };
        let selected = reply.unwrap_or(&missing); let status = if reply.is_some() { 200 } else { 404 };
        if write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n", selected.mime, selected.body.len(), selected.headers).is_ok() {
            let _ = stream.write_all(&selected.body);
        }
    }

    struct Sites { document: Site, media: Site }
    impl Sites {
        fn new(attributes: &str, policy: &str, allowed: bool, cors: impl FnOnce(&str) -> String) -> Self {
            // Both origins are real distinct loopback ports on the same host:
            // cross-origin, but SameSite=Strict cookies remain same-site.
            let document = TcpListener::bind("127.0.0.1:0").unwrap();
            let media = TcpListener::bind("127.0.0.1:0").unwrap();
            let origin = format!("http://{}", document.local_addr().unwrap());
            let source = format!("http://{}/clip.ivf", media.local_addr().unwrap());
            let page = if allowed {
                AUTHORED.replace("media-src 'self'", policy)
                    .replace("src=\"scene-intra.ivf\"", &format!("src=\"{source}\" {attributes}"))
            } else {
                // A denial fixture observes only actual error events; the
                // success fixture's metadata/window-load assertions stay strict.
                format!("<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\"><title>CORS source refusal</title><video id=\"authored-video\" src=\"{source}\" {attributes} controls></video><script>var video=document.getElementById('authored-video'),mediaEvents=[];video.addEventListener('error',function(){{mediaEvents.push('error');}});</script>")
            };
            let document = Site::spawn(document, vec![
                ("/page", Reply { mime: "text/html", headers: "Set-Cookie: document_seed=present; Path=/; SameSite=Strict; HttpOnly\r\n".into(), body: page.into_bytes() }),
                ("/after", Reply { mime: "text/html", headers: String::new(), body: b"<!doctype html><title>CORS cookie witness</title><p>No video.</p>".to_vec() }),
            ]);
            let media = Site::spawn(media, vec![("/clip.ivf", Reply { mime: "video/x-ivf", headers: format!("{}Set-Cookie: media_response=stored; Path=/; SameSite=Strict; HttpOnly\r\n", cors(&origin)), body: IVF.to_vec() })]);
            Self { document, media }
        }
        fn open(&self) -> App {
            let mut app = App::new(); app.scripts_enabled = true; app.images_enabled = false;
            app.open_address(&self.document.url("/page")); app.wait_for_load();
            assert!(!app.failed, "original CORS document failed: {}", app.status); app
        }
        fn witness_cookies(&self, app: &mut App, stored: bool) {
            app.open_address(&self.document.url("/after")); app.wait_for_load();
            assert!(!app.failed); assert_eq!(app.document.title, "CORS cookie witness");
            let requests = self.document.requests("/after"); assert_eq!(requests.len(), 1);
            assert!(has_cookie(&requests[0], "document_seed=present"));
            assert_eq!(has_cookie(&requests[0], "media_response=stored"), stored, "actual subsequent request cookie storage");
        }
    }
    fn actual_playback(app: &mut App) {
        assert!(app.media.is_some()); assert_reference_frame(app, 0);
        eval(app, "if(video.readyState!==4||video.duration!==3||video.videoWidth!==64||video.videoHeight!==48||!mediaWindowLoaded)throw 'CORS metadata';var corsPlay='pending',corsSettles=0;");
        activate_media_document(app);
        eval(app, "var corsPromise=video.play();if(!(corsPromise instanceof Promise)||video.paused)throw 'CORS genuine play';corsPromise.then(function(){corsPlay='fulfilled';corsSettles++;},function(e){throw e;});");
        app.flush_embedded_video(); run(app, Duration::from_millis(650));
        eval(app, "if(corsPlay!=='fulfilled'||corsSettles!==1||video.currentTime<0.5)throw 'CORS actual playback';video.pause();");
        app.flush_embedded_video(); settle(app);
        let position = app.embedded_video_snapshot().unwrap().position;
        let frame = app.media.as_ref().unwrap().displayed;
        assert!(frame >= 1); assert_reference_frame(app, frame);
        run(app, Duration::from_millis(80)); assert_eq!(app.embedded_video_snapshot().unwrap().position, position);
        eval(app, "video.currentTime=2.2;"); app.flush_embedded_video(); settle(app); assert_reference_frame(app, 4);
        eval(app, "if(!video.paused||video.seeking||video.currentTime!==2.2)throw 'CORS settled seek';");
    }

    #[test]
    fn network_media_authored_cors_modes_use_real_frames_play_and_cookie_policy() {
        for credentialed in [false, true] {
            let attributes = if credentialed { "crossorigin=\"use-credentials\"" } else { "crossorigin=\"anonymous\"" };
            let sites = Sites::new(attributes, "media-src *", true, |origin| format!("Access-Control-Allow-Origin: {origin}\r\n{}", if credentialed { "Access-Control-Allow-Credentials: true\r\n" } else { "" }));
            let mut app = sites.open(); actual_playback(&mut app);
            let requests = sites.media.requests("/clip.ivf"); assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].origin.as_deref(), Some(sites.document.url("").as_str()));
            assert_eq!(has_cookie(&requests[0], "document_seed=present"), credentialed);
            if !credentialed { assert!(requests[0].cookie.is_none(), "anonymous cross-origin media sent a Cookie header"); }
            sites.witness_cookies(&mut app, credentialed);
        }
    }

    #[test]
    fn network_media_authored_cors_failures_and_csp_have_exact_source_contacts() {
        // Credentialed HTTP cookies are processed before a later CORS failure;
        // rejection must not fabricate a cookie rollback. Anonymous omits them.
        for case in ["missing", "wildcard-credential", "duplicate", "csp"] {
            let credentialed = case == "wildcard-credential";
            let sites = Sites::new(if credentialed { "crossorigin=\"use-credentials\"" } else { "crossorigin=\"anonymous\"" },
                if case == "csp" { "media-src 'none'" } else { "media-src *" }, false, |origin| match case {
                    "missing" => String::new(),
                    "wildcard-credential" => "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Credentials: true\r\n".into(),
                    "duplicate" => format!("Access-Control-Allow-Origin: {origin}\r\nAccess-Control-Allow-Origin: {origin}\r\n"),
                    _ => format!("Access-Control-Allow-Origin: {origin}\r\n"),
                });
            let mut app = sites.open(); assert!(app.media.is_none(), "{case}");
            assert!(app.embedded_video.as_ref().is_some_and(|video| video.error.is_some()), "{case}");
            eval(&mut app, "if(mediaEvents.indexOf('error')<0)throw 'CORS actual error event';");
            let requests = sites.media.requests("/clip.ivf"); assert_eq!(requests.len(), usize::from(case != "csp"), "{case}: target contacts");
            if let Some(request) = requests.first() {
                assert_eq!(request.origin.as_deref(), Some(sites.document.url("").as_str()));
                assert_eq!(has_cookie(request, "document_seed=present"), credentialed, "{case}");
            }
            run(&mut app, Duration::from_millis(80));
            assert_eq!(sites.media.requests("/clip.ivf").len(), requests.len(), "{case}: automatic retry");
            sites.witness_cookies(&mut app, credentialed);
        }
    }
}
