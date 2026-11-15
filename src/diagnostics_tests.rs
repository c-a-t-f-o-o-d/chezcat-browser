//! Original loopback checks; no page JavaScript or public-service traffic.
use super::*;
use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::Duration;

static NEXT_REPORT: AtomicU64 = AtomicU64::new(0);

struct Report(PathBuf);
impl Report {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "chezcat-diagnostic-{}-{}.txt",
            std::process::id(),
            NEXT_REPORT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn read(&self) -> String {
        std::fs::read_to_string(&self.0).unwrap()
    }
}
impl Drop for Report {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct Site {
    url: String,
    connections: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Site {
    fn new(host: &str, response: Vec<u8>) -> Self {
        let listener = TcpListener::bind((host, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/original", listener.local_addr().unwrap());
        let connections = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let count = connections.clone();
        let done = stop.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let stopping = done.load(Ordering::Acquire) || Instant::now() >= deadline;
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Count TCP acceptance, not merely a parsed request.
                        count.fetch_add(1, Ordering::Relaxed);
                        // Winsock accepted sockets can inherit nonblocking.
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_millis(250)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_millis(250)))
                            .unwrap();
                        let mut request = Vec::new();
                        let request_deadline = Instant::now() + Duration::from_secs(1);
                        while !request.ends_with(b"\r\n\r\n")
                            && request.len() < 8192
                            && Instant::now() < request_deadline
                        {
                            let mut byte = [0];
                            match stream.read(&mut byte) {
                                Ok(0) => break,
                                Ok(_) => request.push(byte[0]),
                                Err(error)
                                    if matches!(
                                        error.kind(),
                                        std::io::ErrorKind::WouldBlock
                                            | std::io::ErrorKind::TimedOut
                                            | std::io::ErrorKind::Interrupted
                                    ) => {}
                                Err(_) => break,
                            }
                        }
                        // A faulty guard may close after connecting. Counting
                        // that contact is still enough to fail the sentinel.
                        let _ = stream.write_all(&response);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if stopping {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("owned loopback listener failed: {error}"),
                }
            }
        });
        Self {
            url,
            connections,
            stop,
            worker: Some(worker),
        }
    }
    fn count(&self) -> usize {
        self.connections.load(Ordering::Relaxed)
    }
    fn finish(&mut self) -> usize {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
        self.count()
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            if let Err(failure) = worker.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(failure);
                }
            }
        }
    }
}

fn response(status: u16, extra: &str, body: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status} Original\r\nContent-Length: {}\r\nContent-Type: text/html\r\nConnection: close\r\n{extra}\r\n{body}", body.len()).into_bytes()
}
fn hosts(values: &[&str]) -> AllowedHosts {
    let mut result = AllowedHosts::default();
    for value in values {
        result.push(value).unwrap();
    }
    result
}
fn report(site: &Site, allowed: &AllowedHosts) -> String {
    let output = Report::new();
    write_report(
        &output.0,
        &[site.url.clone()],
        &trust::Store::unconfigured(),
        allowed,
    )
    .unwrap();
    output.read()
}

#[test]
fn diagnostic_hosts_are_exact_ascii_hosts_with_bounded_atomic_admission() {
    let mut allowed = hosts(&["ExAmPlE.test", "127.0.0.1", "[2001:0db8::1]"]);
    assert_eq!(allowed.0, ["example.test", "127.0.0.1", "2001:db8::1"]);
    assert!(allowed
        .check(&url::parse("https://example.test:444/path").unwrap().0)
        .is_ok());
    assert!(allowed
        .check(&url::parse("https://www.example.test/").unwrap().0)
        .is_err());
    assert!(allowed
        .check(
            &url::parse("https://example.test.attacker.invalid/")
                .unwrap()
                .0
        )
        .is_err());
    for value in [
        "",
        "https://example.test",
        "example.test/path",
        "example.test:443",
        "name@host",
        "*.example.test",
        "example..test",
        "-example.test",
        "example.test?x",
        "a\r\nb",
        " host",
        "host ",
        "é.test",
        "127.1",
        "0x7f000001",
        "[::1]:80",
        "::1",
        "[::1]",
    ] {
        let before = allowed.0.len();
        let result = allowed.push(value);
        if value == "[::1]" {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err(), "accepted {value:?}");
            assert_eq!(allowed.0.len(), before);
        }
    }
    while allowed.0.len() < MAX_HOSTS {
        allowed.push("example.test").unwrap();
    }
    assert!(allowed.push("extra.test").unwrap_err().contains("128"));
    assert_eq!(allowed.0.len(), MAX_HOSTS);
    assert!(allowed.require_diagnose(false).is_err());
    assert!(allowed.require_diagnose(true).is_ok());
    assert!(AllowedHosts::default().require_diagnose(false).is_ok());
}

#[test]
fn diagnostic_allowed_loopback_records_real_phases_without_claiming_usability() {
    let site = Site::new("127.0.0.1", response(200, "", "<h1>Original document</h1>"));
    let output = report(&site, &hosts(&["127.0.0.1"]));
    assert_eq!(site.count(), 1);
    assert!(
        output.contains("RESULT: document received\nHTTP status: 200"),
        "{output}"
    );
    assert!(output.contains(&format!("Final URL: {}", site.url)));
    assert!(output.contains("Page usability: not tested"));
    assert!(output.contains("Measured phase: TCP connect;"));
    assert!(
        !output.contains("Measured phase: DNS lookup;"),
        "literal IP must not invent a DNS phase"
    );
    assert!(output.contains("outcome=complete; status=200"));
    assert!(output
        .contains("no configured text marker found; this does not establish access or usability"));
}

#[test]
fn diagnostic_redirect_host_guard_refuses_before_destination_connection() {
    let mut destination = Site::new("127.0.0.1", response(200, "", "uncontacted sentinel"));
    destination.url = destination.url.replacen("127.0.0.1", "localhost", 1);
    let source = Site::new(
        "127.0.0.1",
        response(302, &format!("Location: {}\r\n", destination.url), ""),
    );
    let output = report(&source, &hosts(&["127.0.0.1"]));
    assert_eq!(source.count(), 1);
    assert_eq!(
        destination.count(),
        0,
        "forbidden destination must not receive even a TCP connection"
    );
    assert!(output.contains("RESULT: failed"), "{output}");
    assert!(
        output.contains(&format!("Failed URL: {}", destination.url)),
        "{output}"
    );
    assert!(output.contains("Diagnostic host allowlist refused host: localhost"));
    assert!(output.contains("Failure boundary: document guard before DNS"));
    assert!(output.contains("Last measured phase: not recorded"));
    assert!(output.contains("outcome=complete; status=302"));
    assert!(output.contains("outcome=blocked; status=not received"));
    assert_eq!(output.matches("Measured phase: TCP connect;").count(), 1);
    let initially_denied = report(&destination, &hosts(&["127.0.0.1"]));
    assert!(initially_denied.contains("document guard before DNS"));
    assert!(!initially_denied.contains("Measured phase:"));
    assert_eq!(destination.count(), 0);
    // Join after draining already queued accepts: a connection cannot hide
    // behind a scheduling race in the no-contact assertion.
    assert_eq!(destination.finish(), 0);
}

#[test]
fn diagnostic_default_policy_still_follows_ordinary_loopback_redirects() {
    let mut destination = Site::new("127.0.0.1", response(200, "", "ordinary final document"));
    destination.url = destination.url.replacen("127.0.0.1", "localhost", 1);
    let source = Site::new(
        "127.0.0.1",
        response(302, &format!("Location: {}\r\n", destination.url), ""),
    );
    let output = report(&source, &AllowedHosts::default());
    assert_eq!((source.count(), destination.count()), (1, 1));
    assert!(output.contains("Host policy: default unrestricted redirects"));
    assert!(output.contains(&format!("Final URL: {}", destination.url)));
    assert!(output.contains("RESULT: document received\nHTTP status: 200"));
}

#[test]
fn diagnostic_failed_redirect_body_reports_actual_destination_and_receive_phase() {
    let mut destination = Site::new(
        "127.0.0.1",
        b"HTTP/1.1 200 Original\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort".to_vec(),
    );
    destination.url = destination.url.replacen("127.0.0.1", "localhost", 1);
    let source = Site::new(
        "127.0.0.1",
        response(302, &format!("Location: {}\r\n", destination.url), ""),
    );
    let output = report(&source, &hosts(&["127.0.0.1", "localhost"]));
    assert_eq!((source.count(), destination.count()), (1, 1));
    assert!(output.contains("RESULT: failed"), "{output}");
    assert!(output.contains(&format!("Failed URL: {}", destination.url)));
    assert!(output.contains("Last measured phase: Receive headers and body"));
    assert!(output.contains("outcome=failed; status=200"));
    assert!(output.contains("outcome=complete; status=302"));
    assert!(!output.contains("RESULT: document received"));
    assert!(!output.contains("Failure boundary: document guard before DNS"));
}

#[test]
fn diagnostic_status_and_body_markers_are_observations_not_challenge_verdicts() {
    let site = Site::new(
        "127.0.0.1",
        response(
            403,
            "",
            "CAPTCHA documentation mentions Unusual Traffic and ACCESS DENIED.",
        ),
    );
    let output = report(&site, &hosts(&["127.0.0.1"]));
    assert!(output.contains("HTTP status: 403"));
    for marker in ["captcha", "unusual traffic", "access denied"] {
        assert!(output.contains(&format!("Possible body challenge marker: {marker}")));
    }
    assert!(output
        .contains("text markers only; not proof of a challenge, denial, or successful access"));
    assert!(output.contains("Page usability: not tested"));
    assert_eq!(site.count(), 1);
}

#[test]
fn diagnostic_attachment_body_limit_and_all_target_prevalidation_are_retained() {
    let site = Site::new("127.0.0.1", format!("HTTP/1.1 200 Original\r\nContent-Disposition: attachment\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", BODY_LIMIT + 1).into_bytes());
    let output = report(&site, &hosts(&["127.0.0.1"]));
    assert!(output.contains("RESULT: failed"), "{output}");
    assert!(
        output.contains(&format!("{BODY_LIMIT}-byte limit")),
        "{output}"
    );
    assert!(output.contains("Last measured phase: Receive headers and body"));
    let before = site.count();
    let path = Report::new();
    assert!(write_report(
        &path.0,
        &[site.url.clone(), "not a URL".into()],
        &trust::Store::unconfigured(),
        &AllowedHosts::default()
    )
    .is_err());
    assert!(
        !path.0.exists(),
        "invalid batch must fail before report creation"
    );
    assert_eq!(site.count(), before);
}

#[test]
fn diagnostic_metadata_is_single_line_escaped_utf8_and_bounded() {
    let raw = "url\\part\r\nRESULT: forged\t\0\u{85}\u{2028}\u{202e}é";
    let escaped = escaped(raw).unwrap();
    assert!(escaped.contains("\\r\\nRESULT: forged\\t\\u{0}"));
    assert!(escaped.contains("\\\\part"));
    assert!(escaped.ends_with('é'));
    assert!(!escaped.chars().any(char::is_control));
    assert!(!escaped.contains('\u{2028}'));
    assert!(!escaped.contains('\u{202e}'));
    for raw in [
        "é".repeat(FIELD_LIMIT),
        "\n".repeat(FIELD_LIMIT),
        "x".repeat(FIELD_LIMIT + 1),
    ] {
        let value = super::escaped(&raw).unwrap();
        assert!(value.len() <= FIELD_LIMIT);
        assert!(value.ends_with("...[truncated]"));
    }
    let trace = network_trace::Trace::default();
    trace.blocked(raw, "document", raw);
    let mut output = Vec::new();
    write_trace(&mut output, &trace.snapshot()).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("RESULT:"))
            .count(),
        0
    );
    assert!(output.contains("outcome=blocked; status=not received"));
    assert!(!output.contains("Measured phase:"));
}
