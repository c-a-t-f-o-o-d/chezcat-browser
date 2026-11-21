//! Original owned loopback request/CORS/policy/body/refusal checks.
use super::*;
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
struct Site {
    origin: HttpUrl,
    routes: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    seen: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Site {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = crate::url::parse(&format!(
            "http://127.0.0.1:{}/document",
            listener.local_addr().unwrap().port()
        ))
        .unwrap()
        .0;
        let routes = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
        let served = routes.clone();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let requests = seen.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !done.load(Ordering::Acquire) && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("owned script listener: {e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut head = Vec::new();
                let mut byte = [0];
                while !head.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    head.push(byte[0]);
                    assert!(head.len() <= MAX_HEADER_BYTES);
                }
                let head = String::from_utf8(head).unwrap();
                let size = head
                    .lines()
                    .find_map(|line| {
                        line.split_once(':')
                            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                assert!(size < 2 * 1024 * 1024);
                let mut body = vec![0; size];
                stream.read_exact(&mut body).unwrap();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_owned();
                requests.lock().unwrap().push((head, body));
                let routes = served.lock().unwrap();
                let wire = routes
                    .iter()
                    .find(|(key, _)| *key == path)
                    .map(|(_, wire)| wire.as_slice())
                    .unwrap_or(
                        b"HTTP/1.1 404 Missing\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                let _ = stream.write_all(wire);
            }
        });
        Self {
            origin,
            routes,
            seen,
            stop,
            worker: Some(worker),
        }
    }
    fn url(&self, path: &str) -> HttpUrl {
        self.origin.resolve(path).unwrap().0
    }
    fn serialized(&self) -> String {
        format!("http://{}", self.origin.host_header())
    }
    fn put(&self, path: &str, status: u16, headers: &str, body: &[u8]) {
        let mut wire = format!(
            "HTTP/1.1 {status} Fixture\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        wire.extend_from_slice(body);
        self.routes.lock().unwrap().push((path.into(), wire));
    }
    fn count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            if let Err(error) = worker.join() {
                if !thread::panicking() {
                    std::panic::resume_unwind(error);
                }
            }
        }
    }
}
fn policy(origin: &HttpUrl, text: &str) -> crate::csp::PolicySet {
    let mut policy = crate::csp::PolicySet::new(&crate::navigation::Location {
        url: Some(origin.clone()),
        path: None,
        fragment: String::new(),
        root: None,
    });
    policy.add_header(text);
    policy
}
fn allow(_: &HttpUrl) -> Result<(), String> {
    Ok(())
}
fn req(method: ScriptMethod, headers: &[(&str, &str)], body: &[u8]) -> ScriptRequest {
    ScriptRequest {
        method,
        headers: headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        body: body.to_vec(),
    }
}
fn field<'a>(head: &'a str, key: &str) -> Option<&'a str> {
    head.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.trim())
    })
}
#[test]
fn script_request_real_post_non_success_body_headers_and_large_payload_keep_form_limits_separate() {
    let site = Site::new();
    site.put("/post",418,"Content-Type: application/json\r\nX-Private: visible\r\nSet-Cookie: script_http=kept; Path=/\r\n",b"{\"actual\":true}");
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let cookies = Arc::new(crate::cookies::Session::default());
    let pool =
        Pool::new(&cancel, &trust).with_cookie_session(cookies.clone(), Some(site.origin.clone()));
    let body = "{\"original\":\"".to_owned() + &"x".repeat(140000) + "\"}";
    let request = req(
        ScriptMethod::Post,
        &[
            ("Content-Type", "application/json"),
            ("X-Original", "live"),
            ("Accept", "application/json"),
        ],
        body.as_bytes(),
    );
    let response = pool
        .fetch_script_request(
            &site.url("/post"),
            &request,
            4096,
            &site.origin,
            ScriptCredentials::SameOrigin,
            &policy(&site.origin, "connect-src 'self'"),
            &allow,
        )
        .unwrap();
    assert_eq!(response.status, 418);
    assert_eq!(response.body, b"{\"actual\":true}");
    assert_eq!(response.header("x-private"), Some("visible"));
    assert_eq!(response.header("set-cookie"), None);
    assert!(cookies.document(&site.origin).contains("script_http=kept"));
    let seen = site.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].0.starts_with("POST /post HTTP/1.1"));
    assert_eq!(field(&seen[0].0, "Content-Type"), Some("application/json"));
    assert_eq!(
        field(&seen[0].0, "Origin"),
        Some(site.serialized().as_str())
    );
    assert_eq!(seen[0].0.matches("Accept:").count(), 1);
    assert_eq!(seen[0].1, body.as_bytes());
    assert!(super::super::request(
        &site.url("/post"),
        &Request::FormPost {
            body: body.into_bytes(),
            origin: Some(site.origin.clone())
        },
        FetchMode::Normal
    )
    .unwrap_err()
    .contains("128 KiB"));
}
#[test]
fn script_request_real_cors_filters_headers_and_obeys_credential_modes() {
    let a = Site::new();
    let b = Site::new();
    let headers=format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: true\r\nAccess-Control-Expose-Headers: X-Visible\r\nX-Visible: kept\r\nX-Secret: hidden\r\nContent-Type: text/plain\r\nSet-Cookie: server=real; Path=/\r\n",a.serialized());
    b.put("/data", 200, &headers, b"real foreign body");
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let cookies = Arc::new(crate::cookies::Session::default());
    cookies
        .set_http(&b.origin, "fixture=kept; Path=/; SameSite=Strict")
        .unwrap();
    let pool =
        Pool::new(&cancel, &trust).with_cookie_session(cookies.clone(), Some(a.origin.clone()));
    let request = req(ScriptMethod::Get, &[], b"");
    let p = policy(&a.origin, "connect-src *");
    for credentials in [
        ScriptCredentials::SameOrigin,
        ScriptCredentials::Include,
        ScriptCredentials::Omit,
    ] {
        let response = pool
            .fetch_script_request(
                &b.url("/data"),
                &request,
                4096,
                &a.origin,
                credentials,
                &p,
                &allow,
            )
            .unwrap();
        assert_eq!(response.body, b"real foreign body");
        assert_eq!(response.header("x-visible"), Some("kept"));
        assert_eq!(response.header("x-secret"), None);
        assert_eq!(response.header("set-cookie"), None);
        assert_eq!(response.header("access-control-allow-origin"), None);
    }
    let seen = b.seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    for (index, (head, _)) in seen.iter().enumerate() {
        assert_eq!(field(head, "Origin"), Some(a.serialized().as_str()));
        assert_eq!(field(head, "Cookie").is_some(), index == 1);
    }
}
#[test]
fn script_request_cors_denial_never_exposes_response_and_preflight_refusal_precedes_contact() {
    let a = Site::new();
    let b = Site::new();
    b.put("/missing", 200, "X-Secret: private\r\n", b"unreadable");
    b.put(
        "/wild",
        200,
        "Access-Control-Allow-Origin: *\r\n",
        b"unreadable",
    );
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust);
    let p = policy(&a.origin, "connect-src *");
    let get = req(ScriptMethod::Get, &[], b"");
    assert!(pool
        .fetch_script_request(
            &b.url("/missing"),
            &get,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &p,
            &allow
        )
        .unwrap_err()
        .contains("CORS"));
    assert!(pool
        .fetch_script_request(
            &b.url("/wild"),
            &get,
            4096,
            &a.origin,
            ScriptCredentials::Include,
            &p,
            &allow
        )
        .unwrap_err()
        .contains("wildcard"));
    let before = b.count();
    for request in [
        req(ScriptMethod::Put, &[], b"write"),
        req(
            ScriptMethod::Post,
            &[("Content-Type", "application/json")],
            b"{}",
        ),
        req(ScriptMethod::Get, &[("X-Private", "secret")], b""),
    ] {
        assert!(pool
            .fetch_script_request(
                &b.url("/denied"),
                &request,
                4096,
                &a.origin,
                ScriptCredentials::Include,
                &p,
                &allow
            )
            .unwrap_err()
            .contains("preflight"));
    }
    assert_eq!(b.count(), before);
}
#[test]
fn script_request_connect_default_meta_redirect_blocking_mixed_content_and_cancel_precede_bytes() {
    let a = Site::new();
    let b = Site::new();
    a.put(
        "/jump",
        302,
        &format!("Location: {}\r\n", b.url("/foreign").display()),
        b"",
    );
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust);
    let request = req(ScriptMethod::Get, &[], b"");
    for text in ["default-src 'none'", "connect-src 'none'; default-src *"] {
        assert!(pool
            .fetch_script_request(
                &a.url("/denied"),
                &request,
                4096,
                &a.origin,
                ScriptCredentials::SameOrigin,
                &policy(&a.origin, text),
                &allow
            )
            .unwrap_err()
            .contains("connect-src"));
    }
    assert_eq!(a.count(), 0);
    let mut meta = policy(&a.origin, "connect-src *");
    meta.add_meta("connect-src 'none'", 999);
    assert!(pool
        .fetch_script_request(
            &a.url("/denied"),
            &request,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &meta,
            &allow
        )
        .unwrap_err()
        .contains("connect-src"));
    assert_eq!(a.count(), 0);
    assert!(pool
        .fetch_script_request(
            &a.url("/jump"),
            &request,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &policy(&a.origin, "connect-src 'self'"),
            &allow
        )
        .unwrap_err()
        .contains("connect-src"));
    assert_eq!(a.count(), 1);
    assert_eq!(b.count(), 0);
    let deny = |_: &HttpUrl| Err("owned request-blocking refusal".into());
    assert!(pool
        .fetch_script_request(
            &a.url("/denied"),
            &request,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &policy(&a.origin, "connect-src *"),
            &deny
        )
        .unwrap_err()
        .contains("request-blocking"));
    assert_eq!(a.count(), 1);
    let mut secure = a.origin.clone();
    secure.secure = true;
    assert!(pool
        .fetch_script_request(
            &a.url("/denied"),
            &request,
            4096,
            &secure,
            ScriptCredentials::SameOrigin,
            &policy(&secure, "connect-src *"),
            &allow
        )
        .unwrap_err()
        .contains("unencrypted"));
    assert_eq!(a.count(), 1);
    cancel.store(true, Ordering::Release);
    assert!(pool
        .fetch_script_request(
            &a.url("/denied"),
            &request,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &policy(&a.origin, "connect-src *"),
            &allow
        )
        .is_err());
    assert_eq!(a.count(), 1);
}
#[test]
fn script_request_real_redirect_rewrites_post_body_and_keeps_other_native_method_bytes() {
    let a = Site::new();
    a.put("/jump", 303, "Location: /final\r\n", b"");
    a.put("/final", 200, "Content-Type: text/plain\r\n", b"real final");
    a.put("/put", 201, "", b"stored");
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust);
    let p = policy(&a.origin, "connect-src 'self'");
    let request = req(
        ScriptMethod::Post,
        &[("Content-Type", "text/plain"), ("X-Original", "kept")],
        b"actual body",
    );
    let response = pool
        .fetch_script_request(
            &a.url("/jump"),
            &request,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &p,
            &allow,
        )
        .unwrap();
    assert!(response.redirected);
    assert!(!response.request_is_post);
    assert_eq!(response.body, b"real final");
    let request = req(
        ScriptMethod::Put,
        &[("Content-Type", "application/json")],
        b"{\"write\":true}",
    );
    assert_eq!(
        pool.fetch_script_request(
            &a.url("/put"),
            &request,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &p,
            &allow
        )
        .unwrap()
        .status,
        201
    );
    let seen = a.seen.lock().unwrap();
    assert!(seen[0].0.starts_with("POST /jump"));
    assert_eq!(seen[0].1, b"actual body");
    assert!(seen[1].0.starts_with("GET /final"));
    assert_eq!(field(&seen[1].0, "Content-Type"), None);
    assert_eq!(field(&seen[1].0, "Content-Length"), None);
    assert!(seen[1].1.is_empty());
    assert_eq!(field(&seen[1].0, "X-Original"), Some("kept"));
    assert!(seen[2].0.starts_with("PUT /put"));
    assert_eq!(seen[2].1, b"{\"write\":true}");
}
#[test]
fn script_request_header_method_validation_and_response_wildcard_never_leak_cookie() {
    for method in ["", "G ET", "GET\r\nPOST"] {
        assert_eq!(ScriptMethod::parse(method).unwrap_err().0, "SyntaxError");
    }
    for method in ["CONNECT", "trace", "Track"] {
        assert_eq!(ScriptMethod::parse(method).unwrap_err().0, "SecurityError");
    }
    assert_eq!(ScriptMethod::parse("post").unwrap(), ScriptMethod::Post);
    for name in [
        "Cookie",
        "Host",
        "Origin",
        "Content-Length",
        "Sec-Fetch-Site",
        "Proxy-Authorization",
        "X-HTTP-Method-Override",
    ] {
        assert!(forbidden_header(name));
        assert!(validate_header(name, "secret").is_err());
    }
    assert!(validate_header("X-Original", "good\r\nCookie: injected").is_err());
    assert!(validate_header("bad name", "x").is_err());
    let mut headers = vec![
        ("Set-Cookie".into(), "private=1".into()),
        ("X-Visible".into(), "real".into()),
        ("Access-Control-Expose-Headers".into(), "*".into()),
    ];
    filter_response_headers(&mut headers, true, ScriptCredentials::SameOrigin);
    assert!(!headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("set-cookie")));
    assert!(headers.iter().any(|(name, _)| name == "X-Visible"));
}

#[test]
fn script_request_head_uses_real_bodyless_framing_and_native_reason_phrase() {
    let a = Site::new();
    a.routes.lock().unwrap().push(("/head".into(),b"HTTP/1.1 503 Caf\xe9\r\nContent-Length: 8192\r\nContent-Encoding: br\r\nConnection: close\r\n\r\n".to_vec()));
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let pool = Pool::new(&cancel, &trust);
    let request = req(ScriptMethod::Head, &[], b"");
    let response = pool
        .fetch_script_request(
            &a.url("/head"),
            &request,
            4096,
            &a.origin,
            ScriptCredentials::SameOrigin,
            &policy(&a.origin, "connect-src 'self'"),
            &allow,
        )
        .unwrap();
    assert_eq!(response.status, 503);
    assert_eq!(response.status_text, "Caf\u{e9}");
    assert_eq!(response.encoded_body_bytes, 0);
    assert!(response.body.is_empty());
    assert_eq!(response.header("content-length"), Some("8192"));
    let seen = a.seen.lock().unwrap();
    assert!(seen[0].0.starts_with("HEAD /head"));
    assert_eq!(field(&seen[0].0, "Content-Length"), None);
}
