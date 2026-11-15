//! Original native loopback transport checks, not App/pixel or public-site proof.
use super::*;
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

struct Site {
    origin: HttpUrl,
    routes: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    requests: Arc<Mutex<Vec<String>>>,
    contacts: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Site {
    fn new(host: &str) -> Self {
        // Every socket binds to the supported loopback address. The localhost
        // spelling supplies a separate exact host/site without127.0.0.2.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = crate::url::parse(&format!("http://{host}:{}/document", listener.local_addr().unwrap().port())).unwrap().0;
        let routes = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
        let served = routes.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let contacts = Arc::new(AtomicUsize::new(0));
        let contacted = contacts.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let stopping = done.load(Ordering::Acquire) || Instant::now() >= deadline;
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if stopping { break; }
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("owned stylesheet listener failed: {error}"),
                };
                contacted.fetch_add(1, Ordering::Relaxed);
                // Accepted Winsock sockets can inherit listener nonblocking.
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
                stream.set_write_timeout(Some(Duration::from_millis(200))).unwrap();
                let read_deadline = Instant::now() + Duration::from_secs(2);
                let mut head = Vec::new();
                while !head.ends_with(b"\r\n\r\n") && head.len() < MAX_HEADER_BYTES && Instant::now() < read_deadline {
                    let mut byte = [0];
                    match stream.read(&mut byte) {
                        Ok(0) => break,
                        Ok(_) => head.push(byte[0]),
                        Err(error) if matches!(error.kind(), io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {},
                        Err(_) => break,
                    }
                }
                let request = String::from_utf8(head).unwrap();
                let path = request.split_whitespace().nth(1).unwrap_or("");
                let wire = served.lock().unwrap().iter().find(|(key, _)| key == path).map(|(_, wire)| wire.clone())
                    .unwrap_or_else(|| b"HTTP/1.1 404 Missing\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec());
                seen.lock().unwrap().push(request);
                let _ = stream.write_all(&wire);
            }
        });
        Self { origin, routes, requests, contacts, stop, worker: Some(worker) }
    }

    fn url(&self, path: &str) -> HttpUrl { self.origin.resolve(path).unwrap().0 }
    fn put(&self, path: &str, status: u16, headers: &str, body: &[u8]) {
        let mut wire = format!("HTTP/1.1 {status} Original\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", body.len()).into_bytes();
        wire.extend_from_slice(body);
        self.wire(path, wire);
    }
    fn wire(&self, path: &str, wire: Vec<u8>) { self.routes.lock().unwrap().push((path.into(), wire)); }
    fn jump(&self, path: &str, to: &HttpUrl, headers: &str) {
        self.put(path, 302, &format!("Location: {}\r\n{headers}", to.display()), b"");
    }
    fn take(&self) -> Vec<String> { std::mem::take(&mut *self.requests.lock().unwrap()) }
    fn finish(&mut self) -> usize {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() { worker.join().unwrap(); }
        self.contacts.load(Ordering::Relaxed)
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            if let Err(failure) = worker.join() {
                if !thread::panicking() { std::panic::resume_unwind(failure); }
            }
        }
    }
}

fn allow(_: &HttpUrl) -> Result<(), String> { Ok(()) }
const CSS: &[u8] = b"#target { width:137px; color:#13579b } #tail { height:29px }";

#[test]
fn stylesheet_native_foreign_get_includes_same_site_cookies_and_retains_hops() {
    let first = Site::new("127.0.0.1");
    let second = Site::new("127.0.0.1");
    second.put("/assets/final.css", 200, "Content-Type: text/css; charset=utf-8\r\n", CSS);
    first.jump("/jump", &second.url("/assets/final.css"), "");
    let cookies = Arc::new(crate::cookies::Session::default());
    cookies.set_http(&second.origin, "fixture=kept; Path=/; SameSite=Strict").unwrap();
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust).with_cookie_session(cookies, Some(first.origin.clone()));
    let result = pool.fetch_stylesheet(&first.url("/jump"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap();
    assert!(result.cross_origin);
    assert_eq!(result.response.body, CSS);
    assert_eq!(result.response.url, second.url("/assets/final.css"));
    assert_eq!(result.hops, [first.url("/jump"), second.url("/assets/final.css")]);
    let request = second.take().pop().unwrap();
    assert!(request.starts_with("GET /assets/final.css HTTP/1.1\r\n"));
    assert!(request.contains("\r\nCookie: fixture=kept\r\n"));
    assert!(!request.contains("\r\nOrigin:"), "unmarked no-CORS GET must not invent a CORS header");
}

#[test]
fn stylesheet_native_filter_element_guard_mixed_content_and_marked_mode_precede_contact() {
    let first = Site::new("127.0.0.1");
    let mut forbidden = Site::new("localhost");
    forbidden.put("/sheet.css", 200, "Content-Type: text/css\r\n", CSS);
    first.jump("/jump", &forbidden.url("/sheet.css"), "");
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let deny = |target: &HttpUrl| if target.host == "localhost" { Err("original stylesheet policy refusal".into()) } else { Ok(()) };
    let trace = network_trace::Trace::default();
    let pool = Pool::new(&cancel, &trust).with_resource_filter(&deny).with_network_trace(trace.clone());
    assert!(pool.fetch_stylesheet(&forbidden.url("/sheet.css"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap_err().message.contains("policy refusal"));
    assert!(pool.fetch_stylesheet(&first.url("/jump"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap_err().message.contains("policy refusal"));
    let plain = Pool::new(&cancel, &trust);
    assert!(plain.fetch_stylesheet(&first.url("/jump"), 4096, &first.origin, FetchMode::Normal, false, &deny).unwrap_err().message.contains("policy refusal"));
    assert!(plain.fetch_stylesheet(&forbidden.url("/sheet.css"), 4096, &first.origin, FetchMode::Normal, true, &allow).unwrap_err().message.contains("CORS-mode"));
    let mut secure = first.origin.clone(); secure.secure = true;
    assert!(plain.fetch_stylesheet(&forbidden.url("/sheet.css"), 4096, &secure, FetchMode::Normal, false, &allow).unwrap_err().message.contains("unencrypted"));
    assert!(trace.snapshot().rows.iter().filter(|row| row.outcome == network_trace::Outcome::Blocked).count() >= 2);
    // Drain already queued accepts before the zero-contact assertion.
    assert_eq!(forbidden.finish(), 0);
}

#[test]
fn stylesheet_native_foreign_mime_is_unambiguous_and_taint_survives_return_home() {
    let first = Site::new("127.0.0.1");
    let second = Site::new("localhost");
    first.put("/missing.css", 200, "", CSS);
    second.put("/missing.css", 200, "", CSS);
    second.put("/wrong.css", 200, "Content-Type: text/html\r\n", b"<p>not CSS</p>");
    second.put("/duplicate.css", 200, "Content-Type: text/css\r\nContent-Type: text/plain\r\n", CSS);
    second.put("/mixedcase.css", 200, "Content-Type: TeXt/CsS; charset=utf-8\r\n", CSS);
    first.jump("/out", &second.url("/back"), "");
    second.jump("/back", &first.url("/missing.css"), "");
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured(); let pool = Pool::new(&cancel, &trust);
    assert!(!pool.fetch_stylesheet(&first.url("/missing.css"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap().cross_origin);
    for path in ["/missing.css", "/wrong.css", "/duplicate.css"] {
        let error = pool.fetch_stylesheet(&second.url(path), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap_err();
        assert!(error.message.contains("Content-Type") || error.message.contains("content-type"), "{error}");
    }
    assert_eq!(pool.fetch_stylesheet(&second.url("/mixedcase.css"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap().response.body, CSS);
    let error = pool.fetch_stylesheet(&first.url("/out"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap_err();
    assert!(error.message.contains("Cross-origin stylesheets require"), "return-home must retain foreign taint: {error}");
}

#[test]
fn stylesheet_native_corp_cookie_effects_precede_refusal_and_redirect_following() {
    let first = Site::new("127.0.0.1");
    let second = Site::new("127.0.0.1");
    let mut untouched = Site::new("localhost");
    second.put("/blocked.css", 200, "Content-Type: text/css\r\nCross-Origin-Resource-Policy: same-origin\r\nSet-Cookie: accepted=kept; SameSite=Strict; Path=/\r\n", CSS);
    second.put("/site.css", 200, "Content-Type: text/css\r\nCross-Origin-Resource-Policy: same-site\r\n", CSS);
    second.put("/case.css", 200, "Content-Type: text/css\r\nCross-Origin-Resource-Policy: SAME-ORIGIN\r\n", CSS);
    second.put("/duplicate.css", 200, "Content-Type: text/css\r\nCross-Origin-Resource-Policy: same-origin\r\nCross-Origin-Resource-Policy: same-site\r\n", CSS);
    second.jump("/blocked-jump", &untouched.url("/never.css"), "Cross-Origin-Resource-Policy: same-origin\r\n");
    let cookies = Arc::new(crate::cookies::Session::default());
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust).with_cookie_session(cookies.clone(), Some(first.origin.clone()));
    assert!(pool.fetch_stylesheet(&second.url("/blocked.css"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap_err().message.contains("same-origin blocked"));
    assert!(cookies.request_header(&second.origin, true, false).contains("accepted=kept"));
    for path in ["/site.css", "/case.css", "/duplicate.css"] {
        pool.fetch_stylesheet(&second.url(path), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap();
    }
    assert!(pool.fetch_stylesheet(&second.url("/site.css"), 4096, &untouched.origin, FetchMode::Normal, false, &allow).unwrap_err().message.contains("same-site blocked"));
    assert!(pool.fetch_stylesheet(&second.url("/blocked-jump"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap_err().message.contains("same-origin blocked"));
    assert_eq!(untouched.finish(), 0);
}

#[test]
fn stylesheet_native_cross_site_cookies_and_redirect_history_remain_restricted() {
    let first = Site::new("127.0.0.1");
    let second = Site::new("localhost");
    first.jump("/out", &second.url("/back"), "");
    second.jump("/back", &first.url("/final.css"), "Set-Cookie: foreign=new; SameSite=Strict; Path=/\r\nSet-Cookie: old=; Max-Age=0; SameSite=Strict; Path=/\r\nSet-Cookie: fresh=new; Path=/\r\n");
    first.put("/final.css", 200, "Content-Type: text/css\r\n", CSS);
    let cookies = Arc::new(crate::cookies::Session::default());
    cookies.set_http(&first.origin, "home=held; SameSite=Strict; Path=/").unwrap();
    cookies.set_http(&second.origin, "old=held; SameSite=Strict; Path=/").unwrap();
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust).with_cookie_session(cookies.clone(), Some(first.origin.clone()));
    let result = pool.fetch_stylesheet(&first.url("/out"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap();
    assert!(result.cross_origin);
    assert_eq!(result.hops, [first.url("/out"), second.url("/back"), first.url("/final.css")]);
    let home = first.take();
    assert_eq!(home.len(), 2);
    assert!(home[0].contains("Cookie: home=held"));
    assert!(!home[1].contains("\r\nCookie:"), "A→B→A must not restore Strict cookies");
    assert!(!second.take()[0].contains("\r\nCookie:"));
    assert_eq!(cookies.request_header(&second.origin, true, false), "old=held", "cross-site responses must not create/delete non-None cookies");
}

#[test]
fn stylesheet_https_none_secure_cookie_context_is_include_without_opening_network() {
    // Pure native state/store control, not a TLS or HTTPS socket journey.
    let origin = crate::url::parse("https://document.example.test/").unwrap().0;
    let target = crate::url::parse("https://assets.other.test/sheet.css").unwrap().0;
    let cookies = crate::cookies::Session::default();
    cookies.set_http(&target, "eligible=held; Secure; SameSite=None; Path=/").unwrap();
    cookies.set_http(&target, "strict=hidden; Secure; SameSite=Strict; Path=/").unwrap();
    let mut state = State::new(&origin, false, 4096); state.prepare(&target).unwrap();
    assert!(!state.same_site(&target));
    assert_eq!(cookies.request_header(&target, state.same_site(&target), false), "eligible=held");
    cookies.set_http_subresource(&target, "new=accepted; Secure; SameSite=None; Path=/", state.same_site(&target)).unwrap();
    assert!(cookies.set_http_subresource(&target, "strict=changed; Secure; SameSite=Strict; Path=/", state.same_site(&target)).is_err());
    assert!(cookies.request_header(&target, false, false).contains("new=accepted"));
}

fn stored_gzip(body: &[u8]) -> Vec<u8> {
    assert!(body.len() <= u16::MAX as usize);
    let mut wire = vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 1];
    wire.extend_from_slice(&(body.len() as u16).to_le_bytes());
    wire.extend_from_slice(&(!(body.len() as u16)).to_le_bytes());
    wire.extend_from_slice(body);
    let mut crc = u32::MAX;
    for &byte in body {
        crc ^= u32::from(byte);
        for _ in 0..8 { crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1)); }
    }
    wire.extend_from_slice(&(!crc).to_le_bytes());
    wire.extend_from_slice(&(body.len() as u32).to_le_bytes());
    wire
}

#[test]
fn stylesheet_native_complete_framing_encoding_status_and_small_limit_refuse_publication() {
    let site = Site::new("127.0.0.1");
    let origin = Site::new("localhost");
    site.put("/gzip.css", 200, "Content-Type: text/css\r\nContent-Encoding: gzip\r\n", &stored_gzip(CSS));
    site.put("/partial.css", 206, "Content-Type: text/css\r\nContent-Range: bytes 0-2/100\r\n", CSS);
    site.put("/range.css", 200, "Content-Type: text/css\r\nContent-Range: bytes 0-2/100\r\n", CSS);
    site.put("/missing.css", 404, "Content-Type: text/css\r\n", CSS);
    site.wire("/truncated.css", b"HTTP/1.1 200 Original\r\nContent-Type: text/css\r\nContent-Length: 100\r\nConnection: close\r\n\r\nshort".to_vec());
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured(); let pool = Pool::new(&cancel, &trust);
    assert_eq!(pool.fetch_stylesheet(&site.url("/gzip.css"), 4096, &origin.origin, FetchMode::Normal, false, &allow).unwrap().response.body, CSS);
    assert!(pool.fetch_stylesheet(&site.url("/gzip.css"), CSS.len() - 1, &origin.origin, FetchMode::Normal, false, &allow).is_err());
    for path in ["/partial.css", "/range.css", "/missing.css", "/truncated.css"] {
        assert!(pool.fetch_stylesheet(&site.url(path), 4096, &origin.origin, FetchMode::Normal, false, &allow).is_err(), "{path}");
    }
}

#[test]
fn stylesheet_native_cancellation_before_redirect_contact_recovers_without_retry() {
    let first = Site::new("127.0.0.1");
    let mut second = Site::new("localhost");
    second.put("/sheet.css", 200, "Content-Type: text/css\r\n", CSS);
    first.jump("/jump", &second.url("/sheet.css"), "");
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured();
    let cancel_next = |target: &HttpUrl| {
        if target.host == "localhost" { cancel.store(true, Ordering::Release); }
        Ok(())
    };
    let pool = Pool::new(&cancel, &trust);
    assert!(pool.fetch_stylesheet(&first.url("/jump"), 4096, &first.origin, FetchMode::Normal, false, &cancel_next).unwrap_err().message.contains("cancelled"));
    assert!(second.take().is_empty());
    cancel.store(false, Ordering::Release);
    assert_eq!(pool.fetch_stylesheet(&first.url("/jump"), 4096, &first.origin, FetchMode::Normal, false, &allow).unwrap().response.body, CSS);
    assert_eq!(second.take().len(), 1, "only the explicit fresh recovery request may arrive");
    assert_eq!(second.finish(), 1);
}

#[test]
fn stylesheet_native_provenance_stays_within_existing_http_redirect_bound() {
    let site = Site::new("127.0.0.1");
    for index in 0..MAX_REDIRECTS { site.jump(&format!("/{index}"), &site.url(&format!("/{}", index + 1)), ""); }
    site.put(&format!("/{MAX_REDIRECTS}"), 200, "Content-Type: text/css\r\n", CSS);
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured(); let pool = Pool::new(&cancel, &trust);
    let result = pool.fetch_stylesheet(&site.url("/0"), 4096, &site.origin, FetchMode::Normal, false, &allow).unwrap();
    assert_eq!(result.hops.len(), MAX_REDIRECTS + 1);
    for (index, hop) in result.hops.iter().enumerate() { assert_eq!(*hop, site.url(&format!("/{index}"))); }
    assert!(!result.cross_origin);
}

#[test]
fn stylesheet_native_rejected_complete_bodies_retain_exact_page_budget_charges() {
    let page = Site::new("127.0.0.1");
    let server = Site::new("127.0.0.1");
    server.put("/wrong-mime.css", 200, "Content-Type: text/html\r\n", CSS);
    server.put("/corp.css", 200, "Content-Type: text/css\r\nCross-Origin-Resource-Policy: same-origin\r\n", CSS);
    server.put("/status.css", 404, "Content-Type: text/css\r\n", CSS);
    server.put("/later.css", 200, "Content-Type: text/css\r\n", CSS);
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust);
    let budget = CSS.len() * 3;
    let mut charged = 0usize;
    for path in ["/wrong-mime.css", "/corp.css", "/status.css"] {
        let remaining = budget.checked_sub(charged).unwrap();
        let error = pool.fetch_stylesheet(&server.url(path), remaining.min(4096), &page.origin,
            FetchMode::Normal, false, &allow).unwrap_err();
        assert_eq!(error.received_bytes, CSS.len(), "{path}: {error}");
        charged = charged.checked_add(error.received_bytes).unwrap();
    }
    assert_eq!(charged, budget, "Complete rejected payloads cannot disappear from native accounting");
    let requests = server.take();
    assert_eq!(requests.len(), 3);
    assert!(!requests.iter().any(|request| request.starts_with("GET /later.css ")));
    // Root's actual parser integration separately exercises refusing the next
    // LINK after charging these typed failures; this is its native HTTP seam.
}

#[test]
fn stylesheet_native_redirect_body_skip_and_final_failure_accounting_are_truthful() {
    let page = Site::new("127.0.0.1");
    let server = Site::new("127.0.0.1");
    server.put("/good.css", 200, "Content-Type: text/css\r\n", CSS);
    server.put("/bad.css", 200, "Content-Type: text/html\r\n", CSS);
    let redirect_body = vec![b'x'; 8192];
    page.put("/good-jump", 302, &format!("Location: {}\r\n", server.url("/good.css").display()), &redirect_body);
    page.put("/bad-jump", 302, &format!("Location: {}\r\n", server.url("/bad.css").display()), &redirect_body);
    page.put("/corp-jump", 302, &format!("Location: {}\r\nCross-Origin-Resource-Policy: same-origin\r\n", server.url("/good.css").display()), &redirect_body);
    let cancel = AtomicBool::new(false); let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust);
    let result = pool.fetch_stylesheet(&page.url("/good-jump"), CSS.len(), &page.origin, FetchMode::Normal, false, &allow).unwrap();
    assert_eq!(result.received_bytes, CSS.len());
    assert_eq!(result.response.body, CSS);
    assert_eq!(result.hops.len(), 2);
    let failure = pool.fetch_stylesheet(&page.url("/bad-jump"), CSS.len(), &page.origin, FetchMode::Normal, false, &allow).unwrap_err();
    assert_eq!(failure.received_bytes, CSS.len());
    assert!(failure.message.contains("Content-Type"));
    // A redirect's body is deliberately not decoded/downloaded as CSS. Do not
    // fabricate charges from its declared Content-Length or prefetched wire.
    let foreign_page = Site::new("localhost");
    let failure = pool.fetch_stylesheet(&page.url("/corp-jump"), CSS.len(), &foreign_page.origin, FetchMode::Normal, false, &allow).unwrap_err();
    assert_eq!(failure.received_bytes, 0);
    assert!(failure.message.contains("same-origin blocked"));
}

#[test]
fn stylesheet_accounting_bounds_remaining_hop_allowance_and_checks_overflow() {
    // State arithmetic control, not invented network/transport observations.
    let origin = crate::url::parse("https://document.example.test/").unwrap().0;
    let mut state = State::new(&origin, false, 10);
    assert_eq!(state.remaining_bytes(), 10);
    state.received(4).unwrap();
    assert_eq!(state.remaining_bytes(), 6);
    state.received(6).unwrap();
    assert_eq!(state.remaining_bytes(), 0);
    assert!(state.received(1).unwrap_err().contains("aggregate body allowance"));
    assert_eq!(state.failure("observed rejection".into()).received_bytes, 11);
    let mut state = State::new(&origin, false, usize::MAX);
    state.received(usize::MAX).unwrap();
    assert!(state.received(1).unwrap_err().contains("overflow"));
    assert_eq!(state.failure("overflow".into()).received_bytes, usize::MAX);
}

#[test]
fn stylesheet_completed_body_charge_precedes_post_read_cancel_and_deadline() {
    // Deterministic original framing-parser/handoff boundary control. This does
    // not claim to reproduce a scheduling race or a real socket-retention fault.
    let origin = crate::url::parse("https://document.example.test/").unwrap().0;
    let mut wire = format!("HTTP/1.1 200 Original\r\nContent-Length: {}\r\nContent-Type: text/css\r\nConnection: keep-alive\r\n\r\n", CSS.len()).into_bytes();
    wire.extend_from_slice(CSS);
    for cancelled in [true, false] {
        let cancel = AtomicBool::new(false);
        let message = read_response(&wire[..], 4096, &cancel,
            Instant::now() + Duration::from_secs(2), true).unwrap();
        assert_eq!(message.body, CSS);
        assert!(message.reusable);
        let mut state = State::new(&origin, false, 4096);
        let deadline = if cancelled {
            cancel.store(true, Ordering::Release);
            Instant::now() + Duration::from_secs(2)
        } else { Instant::now() };
        let calls = std::cell::Cell::new(0);
        let error = complete_response(Some(&mut state), message.body.len(), || {
            calls.set(calls.get() + 1);
            check(&cancel, deadline)
        }).unwrap_err();
        assert_eq!(calls.get(), 1);
        assert!(error.contains(if cancelled { "cancelled" } else { "timed out" }), "{error}");
        let failure = state.failure(error);
        assert_eq!(failure.received_bytes, CSS.len(), "Known completed data must survive failed handoff");
        assert_eq!(state.remaining_bytes(), 4096 - CSS.len());
    }
}
