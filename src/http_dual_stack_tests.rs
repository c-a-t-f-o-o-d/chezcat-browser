//! Original offline controls. Documentation-range family failures below are
//! simulated; they are not observations of internet connectivity.
use super::*;
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};

fn ipv6(index: u16) -> SocketAddr {
    SocketAddr::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, index).into(), 443)
}
fn ipv4(index: u8) -> SocketAddr {
    SocketAddr::new(Ipv4Addr::new(192, 0, 2, index).into(), 443)
}

#[test]
fn dual_stack_ipv4_listener_survives_eight_simulated_failing_ipv6_candidates() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let reachable = listener.local_addr().unwrap();
    let resolved = (1..=8).map(ipv6).chain([reachable]);
    let candidates = select_addresses(resolved, || Ok(())).unwrap().addresses;
    let mut attempted = Vec::new();
    let stream = candidates.into_iter().find_map(|address| {
        attempted.push(address);
        if address.is_ipv6() {
            // Deliberately simulated family failure: never contact these IPs.
            None
        } else {
            Some(TcpStream::connect_timeout(&address, Duration::from_secs(1)).unwrap())
        }
    });
    assert!(stream.is_some(), "Usable IPv4 was discarded behind the IPv6 prefix");
    assert_eq!(attempted, [ipv6(1), reachable]);
    drop(stream);
    let (mut accepted, _) = listener.accept().unwrap();
    accepted.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(accepted.read(&mut [0u8; 1]).unwrap(), 0);
}

#[test]
fn dual_stack_simulated_ipv6_success_survives_eight_failing_ipv4_candidates() {
    let reachable = ipv6(1);
    let candidates = select_addresses((1..=8).map(ipv4).chain([reachable]), || Ok(())).unwrap().addresses;
    let mut attempted = Vec::new();
    // Both family outcomes are simulated: this needs no working native IPv6.
    let connected = candidates.into_iter().find(|address| {
        attempted.push(*address);
        address.is_ipv6()
    });
    assert_eq!(connected, Some(reachable), "Usable IPv6 was discarded behind the IPv4 prefix");
    assert_eq!(attempted, [ipv4(1), reachable]);
}

// Additional original offline controls. Append after the original two controls.
// Documentation-range addresses and synthetic clock/error outcomes are simulated.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Clone, Copy)]
enum DsSpend { None, Budget, Elapsed(Duration) }
struct DsStep { result: io::Result<()>, spend: DsSpend, cancel: bool }
impl DsStep {
    fn success(spend: DsSpend) -> Self { Self { result: Ok(()), spend, cancel: false } }
    fn failure(code: i32, spend: DsSpend) -> Self {
        Self { result: Err(io::Error::from_raw_os_error(code)), spend, cancel: false }
    }
}
#[derive(Debug)]
struct DsStream { address: SocketAddr, drops: Rc<RefCell<Vec<SocketAddr>>> }
impl Drop for DsStream {
    fn drop(&mut self) { self.drops.borrow_mut().push(self.address); }
}
struct DsConnector<'a> {
    current: Rc<Cell<Instant>>,
    steps: VecDeque<DsStep>,
    attempted: Vec<(SocketAddr, Duration)>,
    configured: Vec<SocketAddr>,
    drops: Rc<RefCell<Vec<SocketAddr>>>,
    cancel: Option<&'a AtomicBool>,
    fail_configure: bool,
}
impl<'a> DsConnector<'a> {
    fn new(steps: impl IntoIterator<Item = DsStep>) -> Self {
        Self { current: Rc::new(Cell::new(Instant::now())), steps: steps.into_iter().collect(),
            attempted: Vec::new(), configured: Vec::new(), drops: Rc::default(),
            cancel: None, fail_configure: false }
    }
}
impl Connector for DsConnector<'_> {
    type Stream = DsStream;
    fn now(&self) -> Instant { self.current.get() }
    fn attempt(&mut self, address: &SocketAddr, timeout: Duration) -> io::Result<DsStream> {
        assert!(!timeout.is_zero(), "No zero-budget connection may be started");
        self.attempted.push((*address, timeout));
        let step = self.steps.pop_front().expect("Unexpected additional connection attempt");
        self.current.set(self.current.get() + match step.spend {
            DsSpend::None => Duration::ZERO,
            DsSpend::Budget => timeout,
            DsSpend::Elapsed(elapsed) => elapsed,
        });
        if step.cancel { self.cancel.expect("Cancellation owner").store(true, Ordering::Release); }
        step.result.map(|()| DsStream { address: *address, drops: self.drops.clone() })
    }
    fn configure(&mut self, stream: &DsStream) -> Result<(), String> {
        self.configured.push(stream.address);
        if self.fail_configure { Err("SIMULATED socket-option refusal".into()) } else { Ok(()) }
    }
}
fn ds_plan(addresses: impl IntoIterator<Item = SocketAddr>) -> Candidates {
    select_addresses(addresses.into_iter(), || Ok(())).unwrap()
}
fn ds_ticket() -> network_trace::Ticket {
    network_trace::Trace::default().begin("GET", "http://fallback.invalid/redacted", "document")
}
fn ds_url(text: &str) -> HttpUrl { crate::url::parse(text).unwrap().0 }

#[test]
fn dual_stack_retains_stable_family_order_and_deduplicates_long_prefixes() {
    let plan = ds_plan((1..=64).map(ipv6).chain((1..=3).map(ipv4)));
    assert_eq!(plan.addresses, [ipv6(1), ipv4(1), ipv6(2), ipv4(2), ipv6(3), ipv4(3), ipv6(4), ipv6(5)]);
    assert_eq!((plan.scanned, plan.observed_v4, plan.observed_v6), (67, 3, 64));
    assert!(!plan.scan_limited && plan.selection_limited);
    assert_eq!((plan.retained_v4, plan.retained_v6), (3, 8));
    assert!(plan.family_limited && plan.attempt_limited);
    let reverse = ds_plan((1..=64).map(ipv4).chain((1..=3).map(ipv6)));
    assert_eq!(reverse.addresses, [ipv4(1), ipv6(1), ipv4(2), ipv6(2), ipv4(3), ipv6(3), ipv4(4), ipv4(5)]);
    let duplicates = ds_plan(std::iter::repeat(ipv6(1)).take(200)
        .chain([ipv6(2), ipv4(1), ipv4(1), ipv6(3)]));
    assert_eq!(duplicates.addresses, [ipv6(1), ipv4(1), ipv6(2), ipv6(3)]);
    assert_eq!(duplicates.scanned, 204);
    assert!(!duplicates.scan_limited && !duplicates.selection_limited);
    assert_eq!((duplicates.retained_v4, duplicates.retained_v6), (1, 3));
    assert!(!duplicates.family_limited && !duplicates.attempt_limited);
    // Equality includes IPv6 scope, so distinct eligible socket destinations
    // must not be collapsed merely because their IP octets match.
    let scoped = SocketAddr::V6(std::net::SocketAddrV6::new(Ipv6Addr::LOCALHOST, 443, 0, 2));
    let other_scope = SocketAddr::V6(std::net::SocketAddrV6::new(Ipv6Addr::LOCALHOST, 443, 0, 3));
    assert_eq!(ds_plan([scoped, scoped, other_scope]).addresses, [scoped, other_scope]);
}

#[test]
fn dual_stack_scan_cap_is_honest_and_cancellation_stops_enumeration() {
    let consumed = std::cell::Cell::new(0usize);
    let answers = std::iter::repeat(ipv6(1)).take(MAX_RESOLVER_SCAN)
        .chain([ipv4(1)]).inspect(|_| consumed.set(consumed.get() + 1));
    let plan = select_addresses(answers, || Ok(())).unwrap();
    assert_eq!(consumed.get(), MAX_RESOLVER_SCAN, "No hidden extra resolver probe");
    assert_eq!(plan.addresses, [ipv6(1)]);
    assert!(plan.scan_limited);
    assert!(plan.scan_status().contains("remaining unknown"));
    let last = ds_plan(std::iter::repeat(ipv6(1)).take(MAX_RESOLVER_SCAN - 1).chain([ipv4(1)]));
    assert_eq!(last.addresses, [ipv6(1), ipv4(1)], "An alternative at the final eligible entry must survive");
    assert!(last.scan_limited, "Exactly-at-cap cannot be represented as an exhaustive scan");
    let mut guards = 0;
    let consumed = std::cell::Cell::new(0usize);
    let failure = select_addresses(std::iter::repeat(ipv6(1)).inspect(|_| consumed.set(consumed.get() + 1)), || {
        guards += 1;
        if guards == 17 { Err("SIMULATED enumeration cancelled".into()) } else { Ok(()) }
    });
    assert!(matches!(failure, Err(ref message) if message.contains("cancelled")));
    assert!(consumed.get() <= 17, "Cancellation must stop a nonterminating source promptly");
}

#[test]
fn dual_stack_literals_bypass_dns_and_empty_or_failed_resolution_never_connects() {
    let cancel = AtomicBool::new(false);
    for (text, expected) in [("http://127.0.0.1:8123/check", "127.0.0.1:8123"),
        ("http://[::1]:8123/check", "[::1]:8123")] {
        let url = ds_url(text);
        let before = url.clone();
        let mut connector = DsConnector::new([DsStep::success(DsSpend::None)]);
        let deadline = connector.now() + Duration::from_secs(5);
        let mut events = Vec::new();
        let stream = connect_using(&url, &cancel, deadline, &mut events, &ds_ticket(),
            |_, _| -> io::Result<std::vec::IntoIter<SocketAddr>> { panic!("A literal must never enter the resolver") },
            &mut connector).unwrap();
        assert_eq!(stream.address, expected.parse::<SocketAddr>().unwrap());
        assert_eq!(url, before);
        assert_eq!(connector.attempted.len(), 1);
        assert!(events.iter().any(|event| event.contains("skipped (literal address)")));
        drop(stream);
        assert_eq!(connector.drops.borrow().len(), 1);
    }
    let url = ds_url("http://fallback.invalid/check?private_value=hidden");
    for resolved in [Ok(Vec::<SocketAddr>::new()), Err(io::Error::new(io::ErrorKind::Other,
        "SIMULATED resolver text with 192.0.2.9 and private_value=hidden"))] {
        let failed_dns = resolved.is_err();
        let mut connector = DsConnector::new([]);
        let deadline = connector.now() + Duration::from_secs(5);
        let mut events = Vec::new();
        let error = connect_using(&url, &cancel, deadline, &mut events, &ds_ticket(),
            |host, port| { assert_eq!(host, "fallback.invalid"); assert_eq!(port, 80); resolved.map(Vec::into_iter) },
            &mut connector).unwrap_err();
        assert!(connector.attempted.is_empty());
        assert!(if failed_dns { error.contains("DNS lookup failed") } else { error.contains("no usable network address") }, "{error}");
        let diagnostic = format!("{error}\n{}", events.join("\n"));
        assert!(!diagnostic.contains("192.0.2.9") && !diagnostic.contains("private_value=hidden"));
    }
    // DNS itself remains synchronous, but late completion must not launch a
    // connection after cancellation or the existing operation deadline.
    for late_cancel in [false, true] {
        let cancel = AtomicBool::new(false);
        let mut connector = DsConnector::new([]);
        let clock = connector.current.clone();
        let deadline = connector.now() + Duration::from_secs(5);
        let error = connect_using(&url, &cancel, deadline, &mut Vec::new(), &ds_ticket(),
            |_, _| {
                if late_cancel { cancel.store(true, Ordering::Release); } else { clock.set(deadline); }
                Ok([ipv4(1)].into_iter())
            }, &mut connector).unwrap_err();
        assert!(if late_cancel { error.contains("cancelled") } else { error.contains("timed out") }, "{error}");
        assert!(connector.attempted.is_empty());
    }
}

#[test]
fn dual_stack_all_fail_reports_distinct_bounded_attempt_outcomes_without_addresses() {
    let plan = ds_plan((1..=16).flat_map(|index| [ipv6(index), ipv4(index as u8)]));
    let steps = (0..MAX_CONNECT_CANDIDATES).map(|index| match index {
        0 => DsStep::failure(10013, DsSpend::Elapsed(Duration::from_millis(3))),
        1 => DsStep::failure(10061, DsSpend::Elapsed(Duration::from_millis(7))),
        _ => DsStep { result: Err(io::Error::new(io::ErrorKind::ConnectionRefused,
            "SIMULATED private 192.0.2.240 cookie=secret /private/path?q=hidden")), spend: DsSpend::None, cancel: false },
    });
    let mut connector = DsConnector::new(steps);
    let deadline = connector.now() + Duration::from_secs(60);
    let mut events = Vec::new();
    let error = connect_candidates(&plan, &AtomicBool::new(false), deadline, &mut events, &mut connector).unwrap_err();
    assert_eq!(connector.attempted.len(), MAX_CONNECT_CANDIDATES);
    assert!(connector.configured.is_empty() && connector.drops.borrow().is_empty());
    let attempts: Vec<_> = events.iter().filter(|event| event.starts_with("TCP attempt:")).collect();
    assert_eq!(attempts.len(), MAX_CONNECT_CANDIDATES);
    assert!(attempts[0].contains("family=IPv6") && attempts[0].contains("os_code=10013") && attempts[0].contains("elapsed=3 ms"));
    assert!(attempts[1].contains("family=IPv4") && attempts[1].contains("os_code=10061") && attempts[1].contains("elapsed=7 ms"));
    assert!(error.contains("8 bounded attempts") && error.contains("last attempt:"), "{error}");
    assert!(error.contains("IPv4 attempts=4") && error.contains("IPv6 attempts=4"), "{error}");
    let diagnostic = format!("{error}\n{}", events.join("\n"));
    for private in ["192.0.2.", "2001:db8", "cookie=secret", "/private/path", "q=hidden"] {
        assert!(!diagnostic.contains(private), "Diagnostic leaked {private}: {diagnostic}");
    }
}

#[test]
fn dual_stack_slow_attempt_leaves_alternative_time_and_single_family_keeps_realistic_budget() {
    let cancel = AtomicBool::new(false);
    let plan = ds_plan([ipv6(1), ipv6(2), ipv4(1)]);
    let mut connector = DsConnector::new([DsStep::failure(10060, DsSpend::Budget), DsStep::success(DsSpend::None)]);
    let deadline = connector.now() + Duration::from_millis(500);
    let mut events = Vec::new();
    let stream = connect_candidates(&plan, &cancel, deadline, &mut events, &mut connector).unwrap();
    assert_eq!(stream.address, ipv4(1));
    assert_eq!(connector.attempted.iter().map(|(address, _)| *address).collect::<Vec<_>>(), [ipv6(1), ipv4(1)]);
    assert!(connector.attempted[0].1 < Duration::from_millis(500));
    assert!(!connector.attempted[1].1.is_zero());
    drop(stream);
    for addresses in [vec![ipv4(1), ipv4(2)], vec![ipv6(1), ipv6(2)]] {
        let plan = ds_plan(addresses.clone());
        let mut connector = DsConnector::new([DsStep::failure(10060, DsSpend::Budget), DsStep::success(DsSpend::None)]);
        let deadline = connector.now() + Duration::from_secs(5);
        let stream = connect_candidates(&plan, &cancel, deadline, &mut Vec::new(), &mut connector).unwrap();
        assert_eq!(stream.address, addresses[1]);
        assert!(connector.attempted.iter().all(|(_, budget)| *budget == Duration::from_secs(2)),
            "Single-family attempts must retain the existing realistic ceiling");
    }
    let mut connector = DsConnector::new([DsStep::failure(10060, DsSpend::Budget)]);
    let deadline = connector.now() + Duration::from_millis(125);
    let error = connect_candidates(&ds_plan([ipv4(1), ipv4(2)]), &cancel, deadline, &mut Vec::new(), &mut connector).unwrap_err();
    assert!(error.contains("timed out"));
    assert_eq!(connector.attempted.len(), 1, "No later candidate after operation expiry");
    assert_eq!(connector.attempted[0].1, Duration::from_millis(125));
}

#[test]
fn dual_stack_cancelled_expired_and_unconfigurable_streams_are_dropped_without_later_attempts() {
    let url = ds_url("http://fallback.invalid/check");
    let mut connector = DsConnector::new([]);
    let deadline = connector.now() + Duration::from_secs(5);
    let error = connect_using(&url, &AtomicBool::new(true), deadline, &mut Vec::new(), &ds_ticket(),
        |_, _| -> io::Result<std::vec::IntoIter<SocketAddr>> { panic!("Pre-cancelled operation entered DNS") },
        &mut connector).unwrap_err();
    assert!(error.contains("cancelled") && connector.attempted.is_empty());
    let plan = ds_plan([ipv6(1), ipv4(1)]);
    for success in [false, true] {
        let cancel = AtomicBool::new(false);
        let step = DsStep { result: if success { Ok(()) } else { Err(io::Error::from_raw_os_error(10013)) }, spend: DsSpend::None, cancel: true };
        let mut connector = DsConnector::new([step]);
        connector.cancel = Some(&cancel);
        let deadline = connector.now() + Duration::from_secs(5);
        let error = connect_candidates(&plan, &cancel, deadline, &mut Vec::new(), &mut connector).unwrap_err();
        assert!(error.contains("cancelled"));
        assert_eq!(connector.attempted.len(), 1);
        assert_eq!(connector.drops.borrow().len(), if success { 1 } else { 0 });
    }
    let mut connector = DsConnector::new([]);
    let deadline = connector.now();
    let error = connect_candidates(&plan, &AtomicBool::new(false), deadline, &mut Vec::new(), &mut connector).unwrap_err();
    assert!(error.contains("timed out") && connector.attempted.is_empty());
    // Simulated success at the exact deadline must never escape to its caller.
    let mut connector = DsConnector::new([DsStep::success(DsSpend::Budget)]);
    let deadline = connector.now() + Duration::from_secs(2);
    let error = connect_candidates(&ds_plan([ipv4(1)]), &AtomicBool::new(false), deadline, &mut Vec::new(), &mut connector).unwrap_err();
    assert!(error.contains("timed out"));
    assert_eq!(connector.drops.borrow().as_slice(), [ipv4(1)]);
    let mut connector = DsConnector::new([DsStep::success(DsSpend::None)]);
    connector.fail_configure = true;
    let deadline = connector.now() + Duration::from_secs(5);
    let error = connect_candidates(&plan, &AtomicBool::new(false), deadline, &mut Vec::new(), &mut connector).unwrap_err();
    assert!(error.contains("socket-option refusal"));
    assert_eq!(connector.attempted.len(), 1);
    assert_eq!(connector.drops.borrow().len(), 1);
}

struct DsNative<'a> { inner: NativeConnector, attempted: Vec<SocketAddr>, cancel_after_connect: Option<&'a AtomicBool> }
impl Connector for DsNative<'_> {
    type Stream = TcpStream;
    fn now(&self) -> Instant { self.inner.now() }
    fn attempt(&mut self, address: &SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        self.attempted.push(*address);
        if address.is_ipv6() {
            return Err(io::Error::new(io::ErrorKind::NetworkUnreachable, "SIMULATED unavailable family; no packet sent"));
        }
        let stream = self.inner.attempt(address, timeout)?;
        if let Some(cancel) = self.cancel_after_connect { cancel.store(true, Ordering::Release); }
        Ok(stream)
    }
    fn configure(&mut self, stream: &TcpStream) -> Result<(), String> { self.inner.configure(stream) }
}
fn ds_peer_eof(listener: &TcpListener) {
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_nonblocking(false).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(peer.read(&mut [0u8; 1]).unwrap(), 0, "The owned connection was not closed");
}

#[test]
fn dual_stack_native_ipv4_path_preserves_options_hostname_and_cancelled_socket_cleanup() {
    for cancel_after in [false, true] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let reachable = listener.local_addr().unwrap();
        let url = ds_url(&format!("http://fallback.invalid:{}/check?private_value=hidden", reachable.port()));
        let before = url.clone();
        let mut unavailable = ipv6(1); unavailable.set_port(reachable.port());
        let cancel = AtomicBool::new(false);
        let mut connector = DsNative { inner: NativeConnector, attempted: Vec::new(), cancel_after_connect: cancel_after.then_some(&cancel) };
        let mut events = Vec::new();
        let result = connect_using(&url, &cancel, Instant::now() + Duration::from_secs(5), &mut events, &ds_ticket(),
            |host, port| {
                assert_eq!(host, "fallback.invalid"); assert_eq!(port, reachable.port());
                Ok((1..=8).map(move |index| { let mut address = ipv6(index); address.set_port(port); address }).chain([reachable]))
            },
            &mut connector);
        assert_eq!(connector.attempted, [unavailable, reachable]);
        assert_eq!(url, before, "Address selection must never replace the URL hostname");
        if cancel_after {
            assert!(result.unwrap_err().contains("cancelled"));
        } else {
            let stream = result.unwrap();
            assert!(stream.nodelay().unwrap());
            assert_eq!(stream.read_timeout().unwrap(), Some(POLL_TIMEOUT));
            assert_eq!(stream.write_timeout().unwrap(), Some(POLL_TIMEOUT));
            let wire = crate::http::request(&url, &crate::http::Request::Get, crate::http::FetchMode::Normal).unwrap();
            assert!(wire.contains(&format!("\r\nHost: fallback.invalid:{}\r\n", reachable.port())));
            drop(stream);
        }
        ds_peer_eof(&listener);
        assert!(!events.join("\n").contains("private_value=hidden"));
        assert!(!events.join("\n").contains("127.0.0.1"));
    }
}

#[test]
fn dual_stack_native_ipv6_literal_is_conditional_on_the_local_os_control() {
    let listener = match TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("CONDITIONAL native IPv6 coverage: SKIPPED (local bind unavailable; kind={:?}; code={:?})", error.kind(), error.raw_os_error());
            return;
        }
    };
    let address = listener.local_addr().unwrap();
    // Establish native OS support independently before requiring our path.
    let control = match TcpStream::connect_timeout(&address, Duration::from_secs(1)) {
        Ok(stream) => stream,
        Err(error) => {
            eprintln!("CONDITIONAL native IPv6 coverage: SKIPPED (local std control unavailable; kind={:?}; code={:?})", error.kind(), error.raw_os_error());
            return;
        }
    };
    drop(control);
    ds_peer_eof(&listener);
    let url = ds_url(&format!("http://[::1]:{}/check", address.port()));
    let mut connector = NativeConnector;
    let stream = connect_using(&url, &AtomicBool::new(false), Instant::now() + Duration::from_secs(3),
        &mut Vec::new(), &ds_ticket(),
        |_, _| -> io::Result<std::vec::IntoIter<SocketAddr>> { panic!("Literal IPv6 entered DNS") },
        &mut connector).unwrap();
    assert_eq!(stream.peer_addr().unwrap(), address);
    drop(stream);
    ds_peer_eof(&listener);
    eprintln!("CONDITIONAL native IPv6 coverage: EXECUTED (local std control and production connector)");
}

#[test]
fn dual_stack_cancelled_enumeration_does_not_report_false_zero_counts() {
    let cancel = AtomicBool::new(false);
    let consumed = Cell::new(0);
    let mut connector = DsConnector::new([]);
    let deadline = connector.now() + Duration::from_secs(5);
    let mut events = Vec::new();
    let error = connect_using(&ds_url("http://fallback.invalid/check"), &cancel, deadline,
        &mut events, &ds_ticket(), |_, _| {
            Ok((1..=64).map(ipv6).inspect(|_| {
                consumed.set(consumed.get() + 1);
                if consumed.get() == 3 { cancel.store(true, Ordering::Release); }
            }))
        }, &mut connector).unwrap_err();
    assert!(error.contains("cancelled"));
    assert_eq!(consumed.get(), 3);
    assert!(connector.attempted.is_empty());
    let summary = events.iter().find(|event| event.starts_with("Connection timing: DNS")).unwrap();
    for field in ["scanned=unavailable", "observed IPv4=unavailable", "observed IPv6=unavailable",
        "family cap reached=unavailable", "attempt cap reached=unavailable"] {
        assert!(summary.contains(field), "Incomplete statistics must not become false zeros: {summary}");
    }
}
