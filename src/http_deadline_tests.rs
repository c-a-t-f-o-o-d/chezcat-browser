//! Original owned loopback deadline profiles; no public hosts or TLS policy changes.
use super::*;
use std::net::TcpListener;
use std::thread::JoinHandle;

#[derive(Clone)]
struct Step {
    delay: Duration,
    next: Option<String>,
}

struct Site {
    url: HttpUrl,
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    worker: Option<JoinHandle<()>>,
}

impl Site {
    fn new(steps: Vec<Step>, cancel_second: Option<Arc<AtomicBool>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = crate::url::parse(&format!("http://{}/0", listener.local_addr().unwrap()))
            .unwrap()
            .0;
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let done = stop.clone();
        let recorded = requests.clone();
        let worker = std::thread::spawn(move || {
            let server_limit = Instant::now() + Duration::from_secs(10);
            loop {
                if Instant::now() >= server_limit {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_millis(100)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_millis(100)))
                            .unwrap();
                        let until = Instant::now() + Duration::from_secs(1);
                        let mut head = Vec::new();
                        while !head.ends_with(b"\r\n\r\n")
                            && head.len() < 8192
                            && Instant::now() < until
                            && !done.load(Ordering::Acquire)
                        {
                            let mut byte = [0];
                            match stream.read(&mut byte) {
                                Ok(0) => break,
                                Ok(_) => head.push(byte[0]),
                                Err(error) if retryable(&error) => {}
                                Err(_) => break,
                            }
                        }
                        let path = std::str::from_utf8(&head)
                            .unwrap()
                            .split_whitespace()
                            .nth(1)
                            .unwrap_or("<incomplete>")
                            .to_owned();
                        let index = {
                            let mut requests = recorded.lock().unwrap();
                            let index = requests.len();
                            requests.push(path);
                            index
                        };
                        if index == 1 {
                            if let Some(cancel) = &cancel_second {
                                cancel.store(true, Ordering::Release);
                            }
                        }
                        let Some(step) = steps.get(index) else {
                            continue;
                        };
                        let until = Instant::now() + step.delay;
                        while Instant::now() < until && !done.load(Ordering::Acquire) {
                            std::thread::sleep(
                                Duration::from_millis(5)
                                    .min(until.saturating_duration_since(Instant::now())),
                            );
                        }
                        if done.load(Ordering::Acquire) {
                            continue;
                        }
                        let response = match &step.next {
                            Some(next) => format!("HTTP/1.1 302 Original\r\nLocation: {next}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                            None => "HTTP/1.1 200 Original\r\nContent-Length: 8\r\nConnection: close\r\n\r\nreceived".into(),
                        };
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        // Drain queued accepts before stopping, so forbidden
                        // extra requests cannot hide behind a count race.
                        if done.load(Ordering::Acquire) || Instant::now() >= server_limit {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("owned deadline fixture accept: {error}"),
                }
            }
        });
        Self {
            url,
            stop,
            requests,
            worker: Some(worker),
        }
    }

    fn finish(mut self) -> Vec<String> {
        self.join();
        let requests = self.requests.lock().unwrap().clone();
        requests
    }

    fn join(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            if let Err(error) = worker.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(error);
                }
            }
        }
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.join();
    }
}

fn chain(hops: usize, delay: Duration) -> Vec<Step> {
    (0..hops)
        .map(|index| Step {
            delay,
            next: (index + 1 < hops).then(|| format!("/{}", index + 1)),
        })
        .collect()
}

fn profiled<'a>(
    cancel: &'a AtomicBool,
    trust: &'a trust::Store,
    hop: Duration,
    overall: Duration,
    trace: network_trace::Trace,
) -> Pool<'a> {
    assert!(!hop.is_zero() && hop <= REQUEST_TIMEOUT);
    assert!(!overall.is_zero() && overall <= OVERALL_TIMEOUT);
    let mut pool = Pool::new(cancel, trust).with_network_trace(trace);
    pool.hop_timeout = hop;
    pool.overall_timeout = overall;
    pool
}

#[test]
fn redirect_deadline_renewal_allows_two_and_three_bounded_slow_hops() {
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let defaults = Pool::new(&cancel, &trust);
    assert_eq!(defaults.hop_timeout, Duration::from_secs(20));
    assert_eq!(defaults.overall_timeout, Duration::from_secs(120));
    assert_eq!(MAX_REDIRECTS, 5);
    let hop = Duration::from_millis(900);
    let delay = Duration::from_millis(550);
    for hops in [2, 3] {
        // One whole-fetch window recreates the original policy with short
        // clocks. Every individual server delay is within the hop allowance.
        let old = Site::new(chain(hops, delay), None);
        let result = profiled(&cancel, &trust, hop, hop, network_trace::Trace::default())
            .fetch_document(&old.url, &Request::Get, FetchMode::Normal);
        assert!(result.unwrap_err().contains("timed out"));
        assert!(
            old.finish().len() >= 2,
            "must reproduce exhaustion across hops, not first-hop failure"
        );
        let renewed = Site::new(chain(hops, delay), None);
        let trace = network_trace::Trace::default();
        let response = profiled(&cancel, &trust, hop, hop * 6, trace.clone())
            .fetch_document(&renewed.url, &Request::Get, FetchMode::Normal)
            .unwrap();
        assert_eq!(response.body, b"received");
        assert!(response.redirected && response.url.path == format!("/{}", hops - 1));
        assert_eq!(
            renewed.finish(),
            (0..hops).map(|i| format!("/{i}")).collect::<Vec<_>>()
        );
        let rows = trace.snapshot().rows;
        assert_eq!(rows.len(), hops);
        assert!(rows
            .iter()
            .all(|row| row.outcome == network_trace::Outcome::Complete));
    }
}

#[test]
fn redirect_deadline_single_hop_is_still_bounded() {
    let site = Site::new(chain(1, Duration::from_millis(800)), None);
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let result = profiled(
        &cancel,
        &trust,
        Duration::from_millis(200),
        Duration::from_millis(1200),
        network_trace::Trace::default(),
    )
    .fetch_document(&site.url, &Request::Get, FetchMode::Normal);
    assert!(result.unwrap_err().contains("timed out"));
    assert_eq!(site.finish(), ["/0"]);
}

#[test]
fn redirect_deadline_overall_budget_is_not_renewed() {
    let site = Site::new(
        vec![
            Step {
                delay: Duration::from_millis(100),
                next: Some("/1".into()),
            },
            Step {
                delay: Duration::from_millis(100),
                next: Some("/2".into()),
            },
            Step {
                delay: Duration::from_millis(600),
                next: None,
            },
        ],
        None,
    );
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let trace = network_trace::Trace::default();
    let result = profiled(
        &cancel,
        &trust,
        Duration::from_millis(900),
        Duration::from_millis(500),
        trace.clone(),
    )
    .fetch_document(&site.url, &Request::Get, FetchMode::Normal);
    assert!(result.unwrap_err().contains("timed out"));
    assert_eq!(site.finish(), ["/0", "/1", "/2"]);
    let rows = trace.snapshot().rows;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].status, Some(302));
    assert_eq!(rows[1].status, Some(302));
    assert_eq!(rows[2].outcome, network_trace::Outcome::Failed);
}

#[test]
fn redirect_deadline_cancelled_later_hop_does_not_renew_or_contact_a_third() {
    let cancel = Arc::new(AtomicBool::new(false));
    let site = Site::new(
        vec![
            Step {
                delay: Duration::ZERO,
                next: Some("/1".into()),
            },
            Step {
                delay: Duration::from_secs(2),
                next: Some("/2".into()),
            },
        ],
        Some(cancel.clone()),
    );
    let trust = trust::Store::unconfigured();
    let result = profiled(
        &cancel,
        &trust,
        Duration::from_millis(900),
        Duration::from_millis(5400),
        network_trace::Trace::default(),
    )
    .fetch_document(&site.url, &Request::Get, FetchMode::Normal);
    assert!(result.unwrap_err().contains("cancelled"));
    assert_eq!(site.finish(), ["/0", "/1"]);
    let untouched = Site::new(chain(1, Duration::ZERO), None);
    assert!(profiled(
        &cancel,
        &trust,
        Duration::from_millis(900),
        Duration::from_millis(5400),
        network_trace::Trace::default()
    )
    .fetch_document(&untouched.url, &Request::Get, FetchMode::Normal)
    .unwrap_err()
    .contains("cancelled"));
    assert!(untouched.finish().is_empty());
}

#[test]
fn redirect_deadline_loops_and_five_redirect_cap_remain_bounded() {
    let cancel = AtomicBool::new(false);
    let trust = trust::Store::unconfigured();
    let looped = Site::new(
        vec![Step {
            delay: Duration::ZERO,
            next: Some("/0".into()),
        }],
        None,
    );
    assert!(Pool::new(&cancel, &trust)
        .fetch_document(&looped.url, &Request::Get, FetchMode::Normal)
        .unwrap_err()
        .contains("redirect loop"));
    assert_eq!(looped.finish(), ["/0"]);
    let forward = Site::new(
        (0..8)
            .map(|i| Step {
                delay: Duration::ZERO,
                next: Some(format!("/{}", i + 1)),
            })
            .collect(),
        None,
    );
    assert!(Pool::new(&cancel, &trust)
        .fetch_document(&forward.url, &Request::Get, FetchMode::Normal)
        .unwrap_err()
        .contains("redirect limit"));
    assert_eq!(forward.finish().len(), MAX_REDIRECTS + 1);
}
