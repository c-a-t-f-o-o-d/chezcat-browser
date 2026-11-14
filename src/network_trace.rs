//! Bounded, per-navigation measurements from our own HTTP stack.
//! Times share one monotonic clock. Missing phases stay missing; no timing is
//! inferred from activity strings, response sizes, or site-supplied headers.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const MAX_REQUESTS: usize = 384;
const MAX_PHASES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseKind {
    Queue,
    Dns,
    Connect,
    Tls,
    Send,
    Wait,
    Receive,
    Decode,
}
impl PhaseKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queue => "Connection queue",
            Self::Dns => "DNS lookup",
            Self::Connect => "TCP connect",
            Self::Tls => "TLS handshake",
            Self::Send => "Send request",
            Self::Wait => "Wait for first response byte",
            Self::Receive => "Receive headers and body",
            Self::Decode => "Decode content encoding",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Phase {
    pub kind: PhaseKind,
    pub start: Duration,
    pub end: Option<Duration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Complete,
    Failed,
    Blocked,
    Reused,
}
impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Reused => "reused",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Exchange {
    pub id: u64,
    pub url: String,
    pub method: &'static str,
    pub kind: &'static str,
    pub start: Duration,
    pub end: Option<Duration>,
    pub phases: Vec<Phase>,
    pub outcome: Outcome,
    pub status: Option<u16>,
    pub protocol: Option<&'static str>,
    pub reused_connection: bool,
    /// Decrypted HTTP bytes actually read, including headers/framing; excludes
    /// TLS records, handshake traffic and TCP/IP overhead.
    pub http_bytes: usize,
    pub encoded_bytes: Option<usize>,
    pub decoded_bytes: Option<usize>,
    pub note: String,
}
impl Exchange {
    pub fn duration(&self, now: Duration) -> Duration {
        self.end.unwrap_or(now).saturating_sub(self.start)
    }
}

#[derive(Debug)]
struct State {
    rows: Vec<Exchange>,
    revision: u64,
    omitted: usize,
}
#[derive(Debug)]
struct Inner {
    start: Instant,
    epoch: Option<Duration>,
    state: Mutex<State>,
}
#[derive(Clone, Debug)]
pub struct Trace(Arc<Inner>);
impl Default for Trace {
    fn default() -> Self {
        Self(Arc::new(Inner {
            start: Instant::now(),
            epoch: SystemTime::now().duration_since(UNIX_EPOCH).ok(),
            state: Mutex::new(State {
                rows: Vec::new(),
                revision: 0,
                omitted: 0,
            }),
        }))
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub rows: Vec<Exchange>,
    pub elapsed: Duration,
    pub revision: u64,
    pub omitted: usize,
}
impl Trace {
    pub fn elapsed(&self) -> Duration {
        self.0.start.elapsed()
    }
    pub fn revision(&self) -> u64 {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .revision
    }
    pub fn snapshot(&self) -> Snapshot {
        let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        Snapshot {
            rows: state.rows.clone(),
            elapsed: self.0.start.elapsed(),
            revision: state.revision,
            omitted: state.omitted,
        }
    }
    pub fn begin(&self, method: &'static str, url: &str, kind: &'static str) -> Ticket {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        let id = if state.rows.len() < MAX_REQUESTS {
            let id = state.rows.len() as u64;
            state.rows.push(Exchange {
                id,
                url: bounded(url, 4096),
                method,
                kind,
                start: self.0.start.elapsed(),
                end: None,
                phases: Vec::new(),
                outcome: Outcome::Pending,
                status: None,
                protocol: None,
                reused_connection: false,
                http_bytes: 0,
                encoded_bytes: None,
                decoded_bytes: None,
                note: String::new(),
            });
            Some(id)
        } else {
            state.omitted = state.omitted.saturating_add(1);
            None
        };
        state.revision = state.revision.wrapping_add(1);
        Ticket {
            trace: self.clone(),
            id,
            finished: false,
            document: (kind=="document").then(||Mutex::new(DocumentAttempt::default())),
        }
    }
    pub fn blocked(&self, url: &str, kind: &'static str, reason: &str) {
        self.begin("GET", url, kind).end(Outcome::Blocked, reason);
    }
    pub fn reused(&self, url: &str, kind: &'static str, failed: bool) {
        self.begin("GET", url, kind).end(
            Outcome::Reused,
            if failed {
                "Earlier failure reused within this document load; no request sent."
            } else {
                "Resource reused within this document load; no request sent."
            },
        );
    }
}

pub struct Ticket {
    trace: Trace,
    id: Option<u64>,
    finished: bool,
    // Native document measurements survive diagnostic row truncation. They
    // travel on the actual returned response, never a "last row" heuristic.
    document: Option<Mutex<DocumentAttempt>>,
}

#[derive(Default,Clone,Copy)]
struct DocumentAttempt {request:Option<Duration>,first:Option<Duration>,last:Option<Duration>,headers:Option<Duration>}

#[derive(Clone,Copy,Debug)]
pub struct DocumentTiming {
    origin:Instant,
    navigation_start:u64,
    request_start:u64,
    response_start:u64,
    response_end:u64,
    final_headers:u64,
}
impl DocumentTiming {
    pub fn origin(&self)->Instant {self.origin}
    pub fn value(&self,name:&str)->Option<u64> {Some(match name {"navigationStart"=>self.navigation_start,"requestStart"=>self.request_start,"responseStart"=>self.response_start,"responseEnd"=>self.response_end,_=>return None})}
    pub fn report(&self)->String {format!("Measured document timing (integer epoch ms): navigationStart={} requestStart={} responseStart={} responseEnd={} finalHeadersParsed={}. Redirect/DNS/DOM event timing is not exposed.\n",self.navigation_start,self.request_start,self.response_start,self.response_end,self.final_headers)}
}
impl Ticket {
    pub fn document_timing(&self)->Option<DocumentTiming> {
        let a=*self.document.as_ref()?.lock().unwrap_or_else(|e|e.into_inner());
        let epoch=self.trace.0.epoch?;
        let stamp=|offset:Duration| {let n=u64::try_from(epoch.checked_add(offset)?.as_millis()).ok()?;(n<=9_007_199_254_740_991).then_some(n)};
        let(request,first,last,headers)=(a.request?,a.first?,a.last?,a.headers?);
        if first<request||last<first||headers<first {return None;}
        Some(DocumentTiming {origin:self.trace.0.start,navigation_start:stamp(Duration::ZERO)?,request_start:stamp(request)?,response_start:stamp(first)?,response_end:stamp(last)?,final_headers:stamp(headers)?})
    }
    fn update(&self, change: impl FnOnce(&mut Exchange, Duration)) {
        let Some(id) = self.id else {
            return;
        };
        let mut state = self.trace.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(row) = state.rows.get_mut(id as usize) {
            change(row, self.trace.0.start.elapsed());
            state.revision = state.revision.wrapping_add(1);
        }
    }
    pub fn phase(&self, kind: PhaseKind) {
        if kind==PhaseKind::Send {if let Some(a)=&self.document {a.lock().unwrap_or_else(|e|e.into_inner()).request=Some(self.trace.elapsed());}}
        self.update(|row, now| {
            if let Some(phase) = row.phases.last_mut() {
                if phase.kind == kind && phase.end.is_none() {
                    return;
                }
                if phase.end.is_none() {
                    phase.end = Some(now);
                }
            }
            if row.phases.len() < MAX_PHASES {
                row.phases.push(Phase {
                    kind,
                    start: now,
                    end: None,
                });
            }
        });
    }
    pub fn end_phase(&self) {
        self.update(|row, now| {
            if let Some(phase) = row.phases.last_mut().filter(|p| p.end.is_none()) {
                phase.end = Some(now);
            }
        });
    }
    pub fn connection(&self, reused: bool, protocol: Option<&'static str>) {
        self.update(|row, _| {
            row.reused_connection = reused;
            row.protocol = protocol;
        });
    }
    pub fn received(&self, bytes: usize) {
        if bytes!=0 {let now=self.trace.elapsed();if let Some(a)=&self.document {let mut a=a.lock().unwrap_or_else(|e|e.into_inner());a.first.get_or_insert(now);a.last=Some(now);}}
        self.update(|row, now| {
            if row.http_bytes == 0 && bytes != 0 {
                if let Some(phase) = row.phases.last_mut().filter(|p| p.end.is_none()) {
                    phase.end = Some(now);
                }
                row.phases.push(Phase {
                    kind: PhaseKind::Receive,
                    start: now,
                    end: None,
                });
            }
            row.http_bytes = row.http_bytes.saturating_add(bytes);
        });
    }
    pub fn response(&self, status: u16) {
        if let Some(a)=&self.document {a.lock().unwrap_or_else(|e|e.into_inner()).headers=Some(self.trace.elapsed());}
        self.update(|row, _| row.status = Some(status));
    }
    pub fn complete(
        mut self,
        status: u16,
        encoded: Option<usize>,
        decoded: Option<usize>,
        note: &str,
    ) {
        self.update(|row, _| {
            row.status = Some(status);
            row.encoded_bytes = encoded;
            row.decoded_bytes = decoded;
        });
        self.end(Outcome::Complete, note);
    }
    pub fn fail(mut self, reason: &str) {
        self.end(Outcome::Failed, reason);
    }
    fn end(&mut self, outcome: Outcome, note: &str) {
        self.update(|row, now| {
            if let Some(phase) = row.phases.last_mut().filter(|p| p.end.is_none()) {
                phase.end = Some(now);
            }
            row.end = Some(now);
            row.outcome = outcome;
            row.note = bounded(note, 2048);
        });
        self.finished = true;
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        if !self.finished {
            self.end(Outcome::Failed, "Request stopped before completion.");
        }
    }
}
fn bounded(value: &str, maximum: usize) -> String {
    let mut end = value.len().min(maximum);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_timing_document_observations_survive_trace_cap_and_require_all_measurements() {
        let trace=Trace::default();for _ in 0..MAX_REQUESTS {trace.begin("GET","https://fixture.test/metadata","image").fail("original fixture");}
        let ticket=trace.begin("GET","https://fixture.test/document","document");assert!(ticket.id.is_none());assert!(ticket.document_timing().is_none());
        ticket.phase(PhaseKind::Send);ticket.received(0);assert!(ticket.document_timing().is_none());
        ticket.received(1);assert!(ticket.document_timing().is_none());ticket.response(200);let initial=ticket.document_timing().unwrap();std::thread::sleep(Duration::from_millis(3));ticket.received(2);
        let t=ticket.document_timing().unwrap();assert_eq!(t.response_start,initial.response_start);assert!(t.response_end>=initial.response_end);assert!(t.navigation_start<=t.request_start&&t.request_start<=t.response_start);assert!(t.navigation_start>0);assert_eq!(t.value("redirectStart"),None);
        ticket.complete(200,Some(3),Some(3),"");assert_eq!(trace.snapshot().rows.len(),MAX_REQUESTS);
        let resource=trace.begin("GET","https://fixture.test/subresource","script");resource.phase(PhaseKind::Send);resource.received(10);resource.response(200);assert!(resource.document_timing().is_none());
    }
    #[test]
    fn navigation_timing_missing_epoch_or_unsafe_integer_never_becomes_zero_success() {
        for epoch in [None,Some(Duration::from_secs(u64::MAX))] {
            let trace=Trace(Arc::new(Inner {start:Instant::now(),epoch,state:Mutex::new(State {rows:Vec::new(),revision:0,omitted:0})}));
            let ticket=trace.begin("GET","https://fixture.test/document","document");ticket.phase(PhaseKind::Send);ticket.received(1);ticket.response(200);assert!(ticket.document_timing().is_none());
        }
    }
    #[test]
    fn bounded_trace_records_no_fabricated_phases() {
        let trace = Trace::default();
        trace.blocked("https://example.test/a", "image", "policy");
        trace.reused("https://example.test/a", "image", false);
        for _ in 0..MAX_REQUESTS {
            trace
                .begin("GET", "https://example.test/b", "image")
                .fail("test");
        }
        let shot = trace.snapshot();
        assert_eq!(shot.rows.len(), MAX_REQUESTS);
        assert_eq!(shot.omitted, 2);
        assert!(shot.rows[0].phases.is_empty());
        assert_eq!(shot.rows[1].http_bytes, 0);
        assert_eq!(shot.rows[1].outcome, Outcome::Reused);
    }
}
