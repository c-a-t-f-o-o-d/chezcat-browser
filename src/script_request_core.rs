//! Original asynchronous document-owned script requests, shared by adapters.
//! Owned snapshots cross the worker boundary; JS callbacks stay on the App thread.
use crate::{http, script_storage::Lease, url::HttpUrl};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Id {
    pub receiver: u32,
    pub generation: u64,
}
pub(crate) struct Request {
    pub id: Id,
    pub target: HttpUrl,
    pub origin: HttpUrl,
    pub outgoing: http::ScriptRequest,
    pub credentials: http::ScriptCredentials,
    pub policy: crate::csp::PolicySet,
    pub cancel: Arc<AtomicBool>,
    pub timeout_ms: u32,
    pub started: Instant,
    pub response_limit: usize,
    pub force_preflight: bool,
    _storage: Lease,
}
impl Request {
    pub(crate) fn prepare(
        id: Id,
        target: HttpUrl,
        origin: HttpUrl,
        outgoing: http::ScriptRequest,
        credentials: http::ScriptCredentials,
        policy: crate::csp::PolicySet,
        timeout_ms: u32,
        response_limit: usize,
        used: Arc<AtomicUsize>,
        limit: usize,
    ) -> Result<Arc<Self>, String> {
        let bytes = std::mem::size_of::<Self>()
            .checked_add(5 * std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(url_bytes(&target)))
            .and_then(|bytes| bytes.checked_add(url_bytes(&origin)))
            .and_then(|bytes| bytes.checked_add(outgoing.payload_bytes()))
            .and_then(|bytes| {
                bytes.checked_add(
                    policy
                        .owned_bytes()
                        .saturating_sub(std::mem::size_of::<crate::csp::PolicySet>()),
                )
            })
            .ok_or("Script request snapshot storage overflow.")?;
        let storage = Lease::new(used, bytes, limit)?;
        Ok(Arc::new(Self {
            id,
            target,
            origin,
            outgoing,
            credentials,
            policy,
            cancel: Arc::new(AtomicBool::new(false)),
            timeout_ms,
            started: Instant::now(),
            response_limit,
            force_preflight: false,
            _storage: storage,
        }))
    }
    pub(crate) fn abort(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}
fn url_bytes(url: &HttpUrl) -> usize {
    url.host.capacity() + url.path.capacity() + url.query.as_ref().map_or(0, String::capacity)
}
#[derive(Debug)]
pub(crate) enum Failure {
    Network(String),
    Timeout,
    Storage,
}
pub(crate) struct Completion {
    pub epoch: u64,
    pub request: Arc<Request>,
    pub result: Result<http::Response, Failure>,
    pub(crate) _storage: Option<Lease>,
}
struct Job {
    epoch: u64,
    request: Arc<Request>,
    blocking: Arc<crate::blocking::Policy>,
    network: crate::network_trace::Trace,
    _storage: Lease,
}
struct Shared {
    queue: VecDeque<Job>,
    completed: VecDeque<Completion>,
    active: Option<Arc<AtomicBool>>,
    stopping: bool,
    used: Arc<AtomicUsize>,
    limit: usize,
    wake: crate::loader::Wake,
    _storage: Lease,
}
pub(crate) struct Worker {
    state: Arc<(Mutex<Shared>, Condvar)>,
}
fn response_bytes(response: &http::Response) -> usize {
    response.status_text.capacity()
        + response.body.capacity()
        + response.headers.capacity() * std::mem::size_of::<(String, String)>()
        + response
            .headers
            .iter()
            .map(|(name, value)| name.capacity() + value.capacity())
            .sum::<usize>()
        + response.events.capacity() * std::mem::size_of::<String>()
        + response.events.iter().map(String::capacity).sum::<usize>()
        + url_bytes(&response.url)
        + response.fragment.as_ref().map_or(0, String::capacity)
}
fn prepare_queue<T>(
    queue: &VecDeque<T>,
    needed: usize,
    used: Arc<AtomicUsize>,
    limit: usize,
) -> Result<Option<(VecDeque<T>, Lease)>, String> {
    let before = queue.capacity();
    if needed <= before {
        return Ok(None);
    }
    let available = limit.saturating_sub(used.load(Ordering::Acquire));
    let slot = std::mem::size_of::<T>();
    let growth = before.max(4).max(needed - before).min(available / slot);
    let capacity = before
        .checked_add(growth)
        .filter(|capacity| *capacity >= needed)
        .ok_or("Script request queue backing exhausted.")?;
    let mut next = VecDeque::new();
    next.try_reserve_exact(capacity)
        .map_err(|_| "Could not reserve script request queue.")?;
    let bytes = next
        .capacity()
        .checked_sub(before)
        .and_then(|slots| slots.checked_mul(slot))
        .ok_or("Script request queue storage overflow.")?;
    Ok(Some((next, Lease::new(used, bytes, limit)?)))
}
impl Worker {
    pub(crate) fn new(
        wake: crate::loader::Wake,
        trust: Arc<crate::trust::Store>,
        cookies: Arc<crate::cookies::Session>,
        used: Arc<AtomicUsize>,
        limit: usize,
    ) -> Result<Self, String> {
        let storage = Lease::new(
            used.clone(),
            std::mem::size_of::<(Mutex<Shared>, Condvar)>() + 2 * std::mem::size_of::<usize>(),
            limit,
        )?;
        let state = Arc::new((
            Mutex::new(Shared {
                queue: VecDeque::new(),
                completed: VecDeque::new(),
                active: None,
                stopping: false,
                used,
                limit,
                wake,
                _storage: storage,
            }),
            Condvar::new(),
        ));
        let shared = state.clone();
        std::thread::Builder::new()
            .name("script-requests".into())
            .spawn(move || loop {
                let (job, available) = {
                    let mut state = shared.0.lock().unwrap();
                    while state.queue.is_empty() && !state.stopping {
                        state = shared.1.wait(state).unwrap();
                    }
                    if state.stopping {
                        return;
                    }
                    let job = state.queue.pop_front().unwrap();
                    state.active = Some(job.request.cancel.clone());
                    let available = state
                        .limit
                        .saturating_sub(state.used.load(Ordering::Acquire));
                    (job, available)
                };
                if job.request.cancel.load(Ordering::Acquire) {
                    shared.0.lock().unwrap().active = None;
                    continue;
                }
                let request = &job.request;
                let remaining = if request.timeout_ms == 0 {
                    None
                } else {
                    Duration::from_millis(u64::from(request.timeout_ms))
                        .checked_sub(request.started.elapsed())
                };
                let mut result = if request.timeout_ms != 0 && remaining.is_none() {
                    Err(Failure::Timeout)
                } else {
                    let filter = |url: &HttpUrl| {
                        job.blocking.check(url)?;
                        if request.force_preflight && !request.origin.same_origin(url) {
                            return Err(
                                "XHR upload listeners require an unimplemented CORS preflight."
                                    .into(),
                            );
                        }
                        Ok(())
                    };
                    let mut pool = http::Pool::new(&request.cancel, &trust)
                        .with_cookie_session(cookies.clone(), Some(request.origin.clone()))
                        .with_network_trace(job.network)
                        .with_resource_filter(&filter);
                    if let Some(remaining) = remaining {
                        pool = pool.with_script_timeout(remaining);
                    }
                    pool.fetch_script_request(
                        &request.target,
                        &request.outgoing,
                        request.response_limit.min(available),
                        &request.origin,
                        request.credentials,
                        &request.policy,
                        &filter,
                    )
                    .map_err(Failure::Network)
                };
                if request.timeout_ms != 0
                    && request.started.elapsed()
                        >= Duration::from_millis(u64::from(request.timeout_ms))
                {
                    result = Err(Failure::Timeout);
                }
                let wake = {
                    let mut state = shared.0.lock().unwrap();
                    state.active = None;
                    if state.stopping {
                        return;
                    }
                    if request.cancel.load(Ordering::Acquire) {
                        continue;
                    }
                    let bytes = match &result {
                        Ok(response) => response_bytes(response),
                        Err(Failure::Network(error)) => error.capacity(),
                        Err(_) => 0,
                    };
                    let storage = match Lease::new(state.used.clone(), bytes, state.limit) {
                        Ok(lease) => Some(lease),
                        Err(_) => {
                            result = Err(Failure::Storage);
                            None
                        }
                    };
                    assert!(
                        state.completed.len() < state.completed.capacity(),
                        "Admitted request requires a reserved completion slot"
                    );
                    state.completed.push_back(Completion {
                        epoch: job.epoch,
                        request: job.request,
                        result,
                        _storage: storage,
                    });
                    state.wake.clone()
                };
                wake();
            })
            .map_err(|error| format!("Cannot start native script request worker: {error}"))?;
        Ok(Self { state })
    }
    pub(crate) fn submit(
        &self,
        epoch: u64,
        request: Arc<Request>,
        blocking: Arc<crate::blocking::Policy>,
        network: crate::network_trace::Trace,
    ) -> Result<(), String> {
        let mut state = self.state.0.lock().unwrap();
        if state.stopping {
            return Err("Script request worker is stopping.".into());
        }
        let storage = Lease::new(
            state.used.clone(),
            blocking.owned_bytes() + 2 * std::mem::size_of::<usize>(),
            state.limit,
        )?;
        let count = state
            .completed
            .len()
            .checked_add(state.queue.len())
            .and_then(|count| count.checked_add(usize::from(state.active.is_some())))
            .and_then(|count| count.checked_add(1))
            .ok_or("Script completion queue overflow.")?;
        let queued = state
            .queue
            .len()
            .checked_add(1)
            .ok_or("Script request queue overflow.")?;
        let next = prepare_queue(&state.queue, queued, state.used.clone(), state.limit)?;
        let complete = prepare_queue(&state.completed, count, state.used.clone(), state.limit)?;
        if let Some((mut next, storage)) = next {
            next.append(&mut state.queue);
            state.queue = next;
            state._storage.merge(storage);
        }
        if let Some((mut next, storage)) = complete {
            next.append(&mut state.completed);
            state.completed = next;
            state._storage.merge(storage);
        }
        state.queue.push_back(Job {
            epoch,
            request,
            blocking,
            network,
            _storage: storage,
        });
        self.state.1.notify_one();
        Ok(())
    }
    pub(crate) fn poll(&self) -> Option<Completion> {
        let mut state = self.state.0.lock().unwrap();
        let completion = state.completed.pop_front();
        let wake = (!state.completed.is_empty()).then(|| state.wake.clone());
        drop(state);
        if let Some(wake) = wake {
            wake();
        }
        completion
    }
    pub(crate) fn set_wake(&self, wake: crate::loader::Wake) {
        self.state.0.lock().unwrap().wake = wake;
    }
    pub(crate) fn cancel(&self) {
        let mut state = self.state.0.lock().unwrap();
        if let Some(cancel) = &state.active {
            cancel.store(true, Ordering::Release);
        }
        for job in &state.queue {
            job.request.abort();
        }
        state.queue.clear();
        state.completed.clear();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        self.state.0.lock().unwrap().stopping = true;
        self.state.1.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    fn spec(used: Arc<AtomicUsize>, url: &HttpUrl, id: u64, timeout: u32) -> Arc<Request> {
        let mut policy = crate::csp::PolicySet::new(&crate::navigation::Location {
            url: Some(url.clone()),
            path: None,
            fragment: String::new(),
            root: None,
        });
        policy.add_header("connect-src 'self'");
        Request::prepare(
            Id {
                receiver: 0x8e000000,
                generation: id,
            },
            url.clone(),
            url.clone(),
            http::ScriptRequest {
                method: http::ScriptMethod::Get,
                headers: Vec::new(),
                body: Vec::new(),
            },
            http::ScriptCredentials::SameOrigin,
            policy,
            timeout,
            4096,
            used,
            1024 * 1024,
        )
        .unwrap()
    }
    fn worker(used: Arc<AtomicUsize>) -> Worker {
        Worker::new(
            Arc::new(|| {}),
            Arc::new(crate::trust::Store::unconfigured()),
            Arc::new(crate::cookies::Session::default()),
            used,
            1024 * 1024,
        )
        .unwrap()
    }
    fn completion(worker: &Worker) -> Completion {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = worker.poll() {
                return result;
            }
            assert!(Instant::now() < deadline, "owned async completion deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn retire(worker: Worker, used: &Arc<AtomicUsize>) {
        let weak = Arc::downgrade(&worker.state);
        drop(worker);
        let deadline = Instant::now() + Duration::from_secs(2);
        while weak.upgrade().is_some() {
            assert!(Instant::now() < deadline, "native worker retirement");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(used.load(Ordering::Acquire), 0);
    }
    #[test]
    fn script_request_async_completion_is_real_and_canceled_generation_never_resurrects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = crate::url::parse(&format!(
            "http://127.0.0.1:{}/owned",
            listener.local_addr().unwrap().port()
        ))
        .unwrap()
        .0;
        let (arrived, seen) = std::sync::mpsc::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut head = Vec::new();
                let mut byte = [0];
                while !head.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    head.push(byte[0]);
                    assert!(head.len() < 65536);
                }
                assert!(head.starts_with(b"GET /owned HTTP/1.1"));
                if index == 0 {
                    arrived.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                let _=stream.write_all(b"HTTP/1.1 200 Actual\r\nContent-Type: text/plain\r\nContent-Length: 11\r\nConnection: close\r\n\r\nactual body");
            }
        });
        let used = Arc::new(AtomicUsize::new(0));
        let worker = worker(used.clone());
        let first = spec(used.clone(), &url, 1, 0);
        let second = spec(used.clone(), &url, 2, 0);
        let blocking = Arc::new(crate::blocking::Policy::default());
        worker
            .submit(
                7,
                first.clone(),
                blocking.clone(),
                crate::network_trace::Trace::default(),
            )
            .unwrap();
        seen.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            worker.poll().is_none(),
            "no completion before actual response"
        );
        first.abort();
        worker
            .submit(
                7,
                second.clone(),
                blocking,
                crate::network_trace::Trace::default(),
            )
            .unwrap();
        release.send(()).unwrap();
        let done = completion(&worker);
        assert_eq!(done.epoch, 7);
        assert_eq!(done.request.id, second.id);
        assert_eq!(done.result.as_ref().unwrap().body, b"actual body");
        assert!(worker.poll().is_none());
        server.join().unwrap();
        drop(done);
        drop(first);
        drop(second);
        retire(worker, &used);
    }
    #[test]
    fn script_request_queued_timeout_has_no_contact_and_releases_owned_snapshot() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = crate::url::parse(&format!(
            "http://127.0.0.1:{}/never",
            listener.local_addr().unwrap().port()
        ))
        .unwrap()
        .0;
        let used = Arc::new(AtomicUsize::new(0));
        let worker = worker(used.clone());
        let request = spec(used.clone(), &url, 1, 1);
        std::thread::sleep(Duration::from_millis(4));
        worker
            .submit(
                9,
                request.clone(),
                Arc::new(crate::blocking::Policy::default()),
                crate::network_trace::Trace::default(),
            )
            .unwrap();
        let done = completion(&worker);
        assert!(matches!(done.result, Err(Failure::Timeout)));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        drop(done);
        drop(request);
        retire(worker, &used);
    }
    #[test]
    fn script_request_worker_backing_refusal_is_unpublished_and_next_submit_recovers() {
        let used = Arc::new(AtomicUsize::new(0));
        let url = crate::url::parse("http://127.0.0.1:9/refuse").unwrap().0;
        let worker = worker(used.clone());
        let request = spec(used.clone(), &url, 1, 0);
        let baseline = used.load(Ordering::Acquire);
        worker.state.0.lock().unwrap().limit = baseline;
        assert!(worker
            .submit(
                1,
                request.clone(),
                Arc::new(crate::blocking::Policy::default()),
                crate::network_trace::Trace::default()
            )
            .is_err());
        assert_eq!(used.load(Ordering::Acquire), baseline);
        assert!(worker.poll().is_none());
        assert!(worker.state.0.lock().unwrap().queue.is_empty());
        worker.state.0.lock().unwrap().limit = 1024 * 1024;
        let mut policy = crate::csp::PolicySet::new(&crate::navigation::Location {
            url: Some(url.clone()),
            path: None,
            fragment: String::new(),
            root: None,
        });
        policy.add_header("connect-src 'none'");
        let denied = Request::prepare(
            Id {
                receiver: 0x8e000000,
                generation: 2,
            },
            url.clone(),
            url.clone(),
            http::ScriptRequest {
                method: http::ScriptMethod::Get,
                headers: Vec::new(),
                body: Vec::new(),
            },
            http::ScriptCredentials::SameOrigin,
            policy,
            0,
            4096,
            used.clone(),
            1024 * 1024,
        )
        .unwrap();
        worker
            .submit(
                1,
                denied.clone(),
                Arc::new(crate::blocking::Policy::default()),
                crate::network_trace::Trace::default(),
            )
            .unwrap();
        let done = completion(&worker);
        assert!(
            matches!(&done.result,Err(Failure::Network(error)) if error.contains("connect-src"))
        );
        drop(done);
        drop(request);
        drop(denied);
        retire(worker, &used);
    }
}
