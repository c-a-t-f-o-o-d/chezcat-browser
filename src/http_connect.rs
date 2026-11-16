//! Bounded serial dual-stack fallback. The OS supplies destination preference;
//! RFC 8305 sections 4-5 inform interleaving and fallback opportunity. This is
//! not concurrent Happy Eyeballs or an asynchronous DNS implementation.
use super::*;

#[cfg(test)]
#[path = "http_dual_stack_tests.rs"]
mod tests;

const MAX_RESOLVER_SCAN: usize = 256;
const MAX_CONNECT_CANDIDATES: usize = 8;

struct Candidates {
    addresses: Vec<SocketAddr>,
    scanned: usize,
    observed_v4: usize,
    observed_v6: usize,
    retained_v4: usize,
    retained_v6: usize,
    scan_limited: bool,
    selection_limited: bool,
    family_limited: bool,
    attempt_limited: bool,
}

impl Candidates {
    fn scan_status(&self) -> &'static str {
        if self.scan_limited { "limit reached (remaining unknown)" } else { "complete" }
    }

    fn selection_status(&self) -> &'static str {
        if self.selection_limited { "limited (candidates omitted)" } else { "complete" }
    }
}

fn select_addresses(
    mut addresses: impl Iterator<Item = SocketAddr>,
    mut guard: impl FnMut() -> Result<(), String>,
) -> Result<Candidates, String> {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    let mut preferred_v6 = None;
    let mut result = Candidates {
        addresses: Vec::new(), scanned: 0, observed_v4: 0, observed_v6: 0,
        retained_v4: 0, retained_v6: 0, scan_limited: true,
        selection_limited: false, family_limited: false, attempt_limited: false,
    };
    // Scan work is bounded independently of storage. In particular, filling one
    // family never stops the scan before a later alternative can be retained.
    for _ in 0..MAX_RESOLVER_SCAN {
        guard()?;
        let Some(address) = addresses.next() else {
            result.scan_limited = false;
            break;
        };
        result.scanned += 1;
        let v6 = address.is_ipv6();
        preferred_v6.get_or_insert(v6);
        let family = if v6 {
            result.observed_v6 += 1;
            &mut ipv6
        } else {
            result.observed_v4 += 1;
            &mut ipv4
        };
        if !family.contains(&address) {
            if family.len() < MAX_CONNECT_CANDIDATES {
                family.push(address);
            } else {
                result.family_limited = true;
            }
        }
    }
    guard()?;
    // No extra iterator probe at the scan limit: even exactly 256 answers are
    // conservatively reported as remaining-unknown, never exhaustive resolution.
    let (first, second) = if preferred_v6 == Some(true) { (&ipv6, &ipv4) } else { (&ipv4, &ipv6) };
    for index in 0..MAX_CONNECT_CANDIDATES {
        for family in [first, second] {
            if let Some(address) = family.get(index) {
                if result.addresses.len() < MAX_CONNECT_CANDIDATES {
                    result.addresses.push(*address);
                }
            }
        }
    }
    result.retained_v4 = ipv4.len();
    result.retained_v6 = ipv6.len();
    result.attempt_limited = ipv4.len() + ipv6.len() > result.addresses.len();
    result.selection_limited = result.family_limited || result.attempt_limited;
    Ok(result)
}

fn error_fields(error: &io::Error) -> String {
    // Error Display can contain caller-supplied strings. Keep diagnostics bounded
    // and independent of addresses, URL values, cookies and local paths.
    format!("kind={:?}; os_code={}", error.kind(), error.raw_os_error().map_or("none".into(), |code| code.to_string()))
}

trait Connector {
    type Stream;
    fn now(&self) -> Instant;
    fn attempt(&mut self, address: &SocketAddr, timeout: Duration) -> io::Result<Self::Stream>;
    fn configure(&mut self, stream: &Self::Stream) -> Result<(), String>;
}

struct NativeConnector;
impl Connector for NativeConnector {
    type Stream = TcpStream;
    fn now(&self) -> Instant { Instant::now() }
    fn attempt(&mut self, address: &SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        // This owns at most one blocking connect. std closes its socket on a
        // failed timeout; returned streams remain owned and drop on later error.
        TcpStream::connect_timeout(address, timeout)
    }
    fn configure(&mut self, stream: &TcpStream) -> Result<(), String> {
        // Preserve prompt TLS/form writes and bounded reads/writes.
        stream.set_nodelay(true).map_err(|error| format!("Cannot disable TCP send delay: {error}"))?;
        stream.set_read_timeout(Some(POLL_TIMEOUT)).map_err(|error| format!("Cannot set HTTP read timeout: {error}"))?;
        stream.set_write_timeout(Some(POLL_TIMEOUT)).map_err(|error| format!("Cannot set HTTP write timeout: {error}"))?;
        Ok(())
    }
}

fn connect_candidates<C: Connector>(
    candidates: &Candidates,
    cancel: &AtomicBool,
    deadline: Instant,
    events: &mut Vec<String>,
    connector: &mut C,
) -> Result<C::Stream, String> {
    let started = connector.now();
    let mut attempts = 0;
    let mut attempted_v4 = 0;
    let mut attempted_v6 = 0;
    let result = (|| {
        check_at(cancel, deadline, connector.now())?;
        let mut last = None;
        for (index, address) in candidates.addresses.iter().take(MAX_CONNECT_CANDIDATES).enumerate() {
            let now = connector.now();
            check_at(cancel, deadline, now)?;
            let remaining = deadline.saturating_duration_since(now);
            let alternative_pending = candidates.addresses[index + 1..].iter().any(|next| next.is_ipv6() != address.is_ipv6());
            // A slow preferred attempt must leave an alternative family time.
            // Keep the existing realistic 2s ceiling; RFC's 250ms racing delay
            // is not a reason to abort serial handshakes after only 250ms.
            let available = if alternative_pending { remaining / 2 } else { remaining };
            let timeout = CONNECT_TIMEOUT.min(available);
            if timeout.is_zero() { return Err("HTTP request timed out.".into()); }
            attempts += 1;
            let family = if address.is_ipv6() { attempted_v6 += 1; "IPv6" } else { attempted_v4 += 1; "IPv4" };
            let attempt_started = connector.now();
            let connected = connector.attempt(address, timeout);
            let outcome = match &connected {
                Ok(_) => "connected".to_owned(),
                Err(error) => format!("failed; {}", error_fields(error)),
            };
            events.push(format!("TCP attempt: index={attempts}; family={family}; elapsed={} ms; budget={} ms; outcome={outcome}",
                connector.now().saturating_duration_since(attempt_started).as_millis(), timeout.as_millis()));
            // Covers cancellation/expiry during either a failed or successful
            // blocking call. A successful but cancelled socket is dropped here.
            check_at(cancel, deadline, connector.now())?;
            match connected {
                Ok(stream) => {
                    connector.configure(&stream)?;
                    check_at(cancel, deadline, connector.now())?;
                    return Ok(stream);
                }
                Err(error) => last = Some(format!("family={family}; {}", error_fields(&error))),
            }
        }
        check_at(cancel, deadline, connector.now())?;
        match last {
            Some(last) => Err(format!("HTTP connection failed after {attempts} bounded attempts; IPv4 attempts={attempted_v4}; IPv6 attempts={attempted_v6}; last attempt: {last}; resolver scan={}; selection={}", candidates.scan_status(), candidates.selection_status())),
            None => Err("The host has no usable network address.".into()),
        }
    })();
    events.push(format!("Connection timing: TCP connect={} ms; attempts={attempts}; outcome={}; IPv4 attempts={attempted_v4}; IPv6 attempts={attempted_v6}",
        connector.now().saturating_duration_since(started).as_millis(), if result.is_ok() { "connected" } else { "failed" }));
    result
}

#[allow(clippy::too_many_arguments)]
fn connect_using<I, R, C>(
    url: &HttpUrl,
    cancel: &AtomicBool,
    deadline: Instant,
    events: &mut Vec<String>,
    trace: &network_trace::Ticket,
    resolve: R,
    connector: &mut C,
) -> Result<C::Stream, String>
where
    I: Iterator<Item = SocketAddr>,
    R: FnOnce(&str, u16) -> io::Result<I>,
    C: Connector,
{
    check_at(cancel, deadline, connector.now())?;
    let started = connector.now();
    let literal = url.host.parse::<IpAddr>();
    if literal.is_err() { trace.phase(PhaseKind::Dns); }
    // Synchronous OS resolution can still block. No detached resolver threads
    // are created. Recheck cancellation/deadline before enumerating or connecting.
    let candidates = match literal {
        Ok(ip) => select_addresses(std::iter::once(SocketAddr::new(ip, url.port)), || check_at(cancel, deadline, connector.now())),
        Err(_) => {
            let resolved = resolve(url.host.as_str(), url.port);
            check_at(cancel, deadline, connector.now()).and_then(|_| {
                resolved.map_err(|error| format!("DNS lookup failed: {}", error_fields(&error)))
                    .and_then(|addresses| select_addresses(addresses, || check_at(cancel, deadline, connector.now())))
            })
        }
    };
    // Mid-enumeration cancellation loses the partial plan. Unknown statistics
    // must stay unknown, rather than falsely reporting zero or unhit caps.
    let plan = candidates.as_ref().ok();
    let number = |value: Option<usize>| value.map_or("unavailable".into(), |value| value.to_string());
    let flag = |value: Option<bool>| value.map_or("unavailable".into(), |value| value.to_string());
    let count = |v6| number(plan.map(|plan| plan.addresses.iter().filter(|address| address.is_ipv6() == v6).count()));
    events.push(format!("Connection timing: DNS lookup={} ms; outcome={}; address candidates={}; IPv4 candidates={}; IPv6 candidates={}; observed IPv4={}; observed IPv6={}; retained unique IPv4={}; retained unique IPv6={}; scanned={}; scan={}; selection={}; family cap reached={}; attempt cap reached={}",
        connector.now().saturating_duration_since(started).as_millis(),
        if candidates.is_err() { "failed" } else if literal.is_ok() { "skipped (literal address)" } else { "resolved" },
        number(plan.map(|plan| plan.addresses.len())), count(false), count(true),
        number(plan.map(|plan| plan.observed_v4)), number(plan.map(|plan| plan.observed_v6)),
        number(plan.map(|plan| plan.retained_v4)), number(plan.map(|plan| plan.retained_v6)),
        number(plan.map(|plan| plan.scanned)), candidates.as_ref().map_or("unavailable", Candidates::scan_status),
        candidates.as_ref().map_or("unavailable", Candidates::selection_status),
        flag(plan.map(|plan| plan.family_limited)), flag(plan.map(|plan| plan.attempt_limited))));
    trace.end_phase();
    let candidates = candidates?;
    trace.phase(PhaseKind::Connect);
    let result = connect_candidates(&candidates, cancel, deadline, events, connector);
    trace.end_phase();
    result
}

pub(super) fn connect(
    url: &HttpUrl,
    cancel: &AtomicBool,
    deadline: Instant,
    events: &mut Vec<String>,
    trace: &network_trace::Ticket,
) -> Result<TcpStream, String> {
    // Only transport addresses vary. The caller retains the original URL and
    // hostname for TLS/SNI/certificates, Host, origins, cookies and redirects.
    connect_using(url, cancel, deadline, events, trace,
        |host, port| (host, port).to_socket_addrs(), &mut NativeConnector)
}
