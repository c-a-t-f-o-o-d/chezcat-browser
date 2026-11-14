//! Bounded HTTP/1.1 GET/form-POST transport, using standard-library sockets.
//! Message framing follows RFC 9110 / RFC 9112. HTTPS uses our TLS implementation.
//! Gzip/zlib content decoding follows framing and precedes document/image parsing.
//! No proxy, HTTP credentials or response cache. Browser loads can attach a
//! bounded memory-only cookie session. A document load can
//! share at most three authenticated connections, each with one exchange at a
//! time. Only complete, unambiguous HTTP/1.1 messages permit another request.

use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

use crate::url::HttpUrl;
use crate::{compression, network_trace, random, tls, trust};
use network_trace::PhaseKind;

#[cfg(test)]
#[path = "http_network_tests.rs"]
mod network_tests;

const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_FORM_BYTES: usize = 128 * 1024;
pub const USER_AGENT: &str = "ChezCat/0.63";
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_LINE_BYTES: usize = 8 * 1024;
const MAX_CHUNK_METADATA: usize = 512 * 1024;
const MAX_HEADERS: usize = 200;
const MAX_CHUNKS: usize = 65_536;
const MAX_INFORMATIONAL: usize = 8;
const MAX_REDIRECTS: usize = 5;
const MAX_CONTENT_CODINGS: usize = 4;
const MAX_EMPTY_CODINGS: usize = 32;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_TIMEOUT: Duration = Duration::from_millis(200);
const MAX_CONNECTIONS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchMode {
    Normal,
    /// Skip browser caches when one is added, omit conditional validators and
    /// ask intermediary caches to validate with the origin (RFC 9111 5.2.1.4).
    FullReload,
}

/// Navigation payloads are explicit; resource requests continue to use GET.
/// No submitted values are written to the activity report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Get,
    FormPost {
        body: Vec<u8>,
        /// A local document has an opaque origin, serialized as `null`.
        origin: Option<HttpUrl>,
    },
}

impl Request {
    pub fn is_post(&self) -> bool {
        matches!(self, Self::FormPost { .. })
    }

    fn method(&self) -> &'static str {
        if self.is_post() {
            "POST"
        } else {
            "GET"
        }
    }

    fn body(&self) -> &[u8] {
        match self {
            Self::Get => &[],
            Self::FormPost { body, .. } => body,
        }
    }

    fn redirect(&mut self, status: u16, from: &HttpUrl, to: &HttpUrl) -> Result<(), String> {
        if from.secure && !to.secure {
            return Err("HTTPS redirect to unencrypted HTTP was blocked.".into());
        }
        if self.is_post() {
            if matches!(status, 301..=303) {
                *self = Self::Get;
            } else if !from.same_origin(to) {
                return Err("Cross-origin redirect retaining a POST body was blocked; form values were not forwarded.".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Response {
    pub url: HttpUrl,
    /// Actual followed redirect provenance, including chains returning to the
    /// original URL. Never infer document identity from a final URL comparison.
    pub redirected: bool,
    /// None preserves the caller's fragment; Some("") explicitly clears it.
    pub fragment: Option<String>,
    pub status: u16,
    /// Authenticated protocol of the final exchange, not an assumed version.
    pub protocol: Option<&'static str>,
    /// The final exchange can be GET after a POST redirect. Callers use this
    /// to avoid an automatic POST replay during refresh/history navigation.
    pub request_is_post: bool,
    /// Final cookie request context, retained even when no Cookie field was
    /// sent. UI reloads must not upgrade an originally cross-site navigation.
    pub cookie_same_site: bool,
    /// Original headers; Content-Encoding and Content-Length describe the
    /// encoded representation. `body` contains the fully decoded bytes.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub events: Vec<String>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }
}

pub fn fetch(
    url: &HttpUrl,
    limit: usize,
    same_origin: Option<&HttpUrl>,
    cancel: &AtomicBool,
    trust: &trust::Store,
    mode: FetchMode,
) -> Result<Response, String> {
    fetch_request(url, &Request::Get, limit, same_origin, cancel, trust, mode)
}

pub fn fetch_request(
    url: &HttpUrl,
    initial: &Request,
    limit: usize,
    same_origin: Option<&HttpUrl>,
    cancel: &AtomicBool,
    trust: &trust::Store,
    mode: FetchMode,
) -> Result<Response, String> {
    // Standalone callers keep their original one-exchange lifetime. The loader
    // explicitly owns a Pool for its document and supported subresources.
    fetch_in_pool(
        &Pool::new(cancel, trust),
        url,
        initial,
        limit,
        same_origin,
        mode,
        false,
        "document",
        false,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn fetch_in_pool(
    pool: &Pool<'_>,
    url: &HttpUrl,
    initial: &Request,
    limit: usize,
    same_origin: Option<&HttpUrl>,
    mode: FetchMode,
    retain: bool,
    kind: &'static str,
    document: bool,
    classic_origin: Option<&HttpUrl>,
    resource_guard: Option<&ResourceFilter<'_>>,
) -> Result<Response, String> {
    let cancel = pool.cancel;
    if limit > MAX_BODY_BYTES {
        return Err("HTTP body limit cannot exceed 32 MiB.".into());
    }
    if initial.body().len() > MAX_FORM_BYTES {
        return Err("HTTP form request body exceeds 128 KiB.".into());
    }
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut current = url.clone();
    let mut outgoing = initial.clone();
    let mut fragment = None;
    let mut visited = HashSet::new();
    let mut events = Vec::new();
    let mut document_cookie_context = crate::cookies::NavigationContext::new(
        pool.cookie_origin.as_ref(),
        pool.cookie_context,
    );
    // Once a classic-script redirect leaves its document origin, credentials
    // remain omitted for the rest of the chain, even when it returns.
    let mut resource_credentials = true;
    let script_origin = if document {
        pool.document_origin.as_ref()
    } else {
        None
    };

    let result = (|| {
        for redirects in 0..=MAX_REDIRECTS {
            check(cancel, deadline)?;
            if same_origin.is_some() || classic_origin.is_some() || script_origin.is_some() {
                // Check every automatic destination before DNS, connecting
                // or sending bytes, including a permitted URL's redirect.
                // User-initiated document navigation remains unrestricted.
                if let Err(error) = pool.check_resource(&current) {
                    pool.network.blocked(&current.display(), kind, &error);
                    return Err(error);
                }
            }
            if document {
                if let Some(guard) = pool.document_filter {
                    if let Err(error) = guard(&current) {
                        pool.network.blocked(&current.display(), kind, &error);
                        return Err(error);
                    }
                }
            }
            if let Some(origin) = classic_origin {
                if let Err(error) = check_classic_script_origin(origin, &current) {
                    pool.network.blocked(&current.display(), kind, &error);
                    return Err(error);
                }
                resource_credentials &= current.same_origin(origin);
            }
            if let Some(guard) = resource_guard {
                if let Err(error) = guard(&current) {
                    pool.network.blocked(&current.display(), kind, &error);
                    return Err(error);
                }
            }
            if same_origin.is_some_and(|origin| !current.same_origin(origin)) {
                pool.network.blocked(
                    &current.display(),
                    kind,
                    "Cross-origin resource redirect blocked.",
                );
                return Err("Cross-origin resource navigation or redirect was blocked.".into());
            }
            if script_origin.is_some_and(|origin| !current.same_origin(origin)) {
                pool.network.blocked(
                    &current.display(),
                    kind,
                    "Cross-origin script navigation redirect blocked.",
                );
                return Err("Cross-origin script navigation or redirect was blocked.".into());
            }
            let mut request = request(&current, &outgoing, mode)?;
            let address = current.display();
            if !visited.insert((outgoing.method(), address.clone())) {
                return Err("HTTP redirect loop detected.".into());
            }
            events.push(format!("{} {address}", outgoing.method()));
            let cookie_same_site = if document {
                document_cookie_context.is_same_site(&current)
            } else {
                // Resource requests have their own already-checked document
                // origin. A top-level reload override must not affect them.
                resource_credentials
                    && same_origin.or(classic_origin).is_some_and(|origin| {
                        crate::cookies::same_site(origin, &current)
                    })
            };
            if let Some(cookies) = pool.cookies.as_ref().filter(|_| resource_credentials) {
                let value = cookies.request_header(
                    &current,
                    cookie_same_site,
                    document && !outgoing.is_post(),
                );
                if !value.is_empty() {
                    request.truncate(request.len() - 2);
                    request.push_str("Cookie: ");
                    request.push_str(&value);
                    request.push_str("\r\n\r\n");
                    events.push(
                        "Session Cookie header sent; values omitted from this report.".into(),
                    );
                }
            }
            if outgoing.is_post() {
                events.push(format!(
                    "FORM BODY: {} bytes; values omitted from this report.",
                    outgoing.body().len()
                ));
            }
            let (message, protocol) = pool.exchange(
                &current,
                &outgoing,
                &request,
                limit,
                deadline,
                retain,
                &mut events,
                kind,
                document,
            )?;
            events.push(
                if is_redirect(message.status) && header(&message.headers, "location").is_some() {
                    format!(
                        "HTTP framing: {}; redirect body intentionally skipped.",
                        message.framing.description()
                    )
                } else {
                    format!(
                        "HTTP framing: {}; {} encoded body bytes received.",
                        message.framing.description(),
                        message.encoded_bytes
                    )
                },
            );
            if let Some(origin) = classic_origin {
                check_classic_script_corp(origin, &current, &message.headers)?;
            }
            let cookie_headers = message
                .headers
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
                .map(|(_, value)| value);
            let mut accepted = 0;
            let mut declined = 0;
            let can_store_cookies = document
                || resource_credentials
                    && same_origin
                        .or(classic_origin)
                        .is_some_and(|origin| current.same_origin(origin));
            for value in cookie_headers {
                if can_store_cookies
                    && pool
                        .cookies
                        .as_ref()
                        .is_some_and(|cookies| cookies.set_http(&current, value).is_ok())
                {
                    accepted += 1;
                } else {
                    declined += 1;
                }
            }
            if accepted + declined > 0 {
                events.push(format!("Set-Cookie headers: {accepted} processed by the bounded session store; {declined} declined. Cookie values are omitted from this report."));
            }
            if is_redirect(message.status) {
                if let Some(location) = unique_header(&message.headers, "location")? {
                    if redirects == MAX_REDIRECTS {
                        return Err("HTTP redirect limit reached (5).".into());
                    }
                    let (next, next_fragment) = current.resolve(location)?;
                    let was_post = outgoing.is_post();
                    outgoing.redirect(message.status, &current, &next)?;
                    if location.contains('#') {
                        fragment = Some(next_fragment);
                    }
                    events.push(format!(
                        "HTTP {} redirect to {}",
                        message.status,
                        next.display()
                    ));
                    if was_post && !outgoing.is_post() {
                        events.push(
                        "Redirect changes POST to GET; form body and content headers discarded."
                            .into(),
                    );
                    }
                    // Once a redirect leaves the schemeful site, returning
                    // cannot restore Strict cookies. Sibling subdomains inside
                    // one registrable domain are not a cross-site hop.
                    document_cookie_context.redirect(&current, &next);
                    current = next;
                    continue;
                }
            }
            if !message.codings.is_empty() {
                let names = message
                    .codings
                    .iter()
                    .map(|coding| coding.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                events.push(format!(
                    "HTTP Content-Encoding: {names}; {} encoded bytes -> {} decoded bytes",
                    message.encoded_bytes,
                    message.body.len()
                ));
            }
            events.push(format!(
                "HTTP {}: {} body bytes",
                message.status,
                message.body.len()
            ));
            return Ok(Response {
                redirected: redirects != 0,
                url: current,
                fragment,
                status: message.status,
                protocol,
                request_is_post: outgoing.is_post(),
                cookie_same_site,
                headers: message.headers,
                body: message.body,
                events: std::mem::take(&mut events),
            });
        }
        Err("HTTP redirect limit reached.".into())
    })();
    result.map_err(|error| failure_with_activity(error, &events))
}

/// Deliberately narrower than general no-CORS fetching: only secure pages
/// may include another secure origin. No scheme downgrade or local-file path
/// can enter this API, and callers still enforce CSP and JavaScript MIME.
/// A crossorigin attribute changes fetch mode, not the origin itself. Until
/// cross-origin response validation is implemented, accept only paths that
/// remain same-origin. Anonymous and use-credentials are equivalent on such a
/// path; the existing cookie/CSP/TLS policies still apply.
pub(crate) fn check_classic_script_mode(
    origin: &HttpUrl,
    target: &HttpUrl,
    same_origin_only: bool,
) -> Result<(), String> {
    if same_origin_only && !origin.same_origin(target) {
        return Err("Cross-origin CORS-mode scripts are not implemented; only same-origin scripts are supported.".into());
    }
    check_classic_script_origin(origin, target)
}

pub(crate) fn check_classic_script_origin(
    origin: &HttpUrl,
    target: &HttpUrl,
) -> Result<(), String> {
    if origin.same_origin(target) || origin.secure && target.secure {
        Ok(())
    } else {
        Err("Cross-origin classic scripts require an HTTPS document and HTTPS destination; HTTP origin changes and mixed content are blocked.".into())
    }
}

/// Check no-CORS response restrictions on every classic-script exchange,
/// including redirects, before following another URL or accepting cookies.
/// same-site is conservative: exact scheme/host only; no guessed registrable
/// domains or public-suffix data are used. Ordinary resource origins are unchanged.
fn check_classic_script_corp(
    origin: &HttpUrl,
    target: &HttpUrl,
    headers: &[(String, String)],
) -> Result<(), String> {
    if origin.same_origin(target) {
        return Ok(());
    }
    let mut fields = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("cross-origin-resource-policy"));
    let Some((_, value)) = fields.next() else {
        return Ok(());
    };
    if fields.next().is_some() {
        return Err(
            "Ambiguous Cross-Origin-Resource-Policy headers; cross-origin script declined.".into(),
        );
    }
    match value.trim() {
        "same-origin" => Err("Cross-Origin-Resource-Policy same-origin blocked this script response.".into()),
        "same-site" if origin.secure != target.secure || origin.host != target.host => {
            Err("Cross-Origin-Resource-Policy same-site requires the exact scheme and host in this build.".into())
        }
        // The defined header values are case-sensitive. Invalid single values
        // are ignored, while ambiguous repeated fields fail conservatively.
        _ => Ok(()),
    }
}

fn failure_with_activity(error: String, events: &[String]) -> String {
    // Keep the authenticated protocol/peer details even when a later response
    // fails. Raw headers, response bodies and form bodies never enter this
    // trace. Redact URL queries as they may contain GET form values.
    const LIMIT: usize = 16 * 1024;
    const OMITTED: &str = "\n[additional activity omitted]";
    let mut output = error;
    output.push_str("\nConnection activity:");
    for event in events {
        output.push_str("\n  ");
        let url_event = event.starts_with("GET ")
            || event.starts_with("POST ")
            || (event.starts_with("HTTP ") && event.contains(" redirect to "));
        if let Some((prefix, _)) = event.split_once('?').filter(|_| url_event) {
            output.push_str(prefix);
            output.push_str("?[query omitted]");
        } else {
            output.push_str(event);
        }
        if output.len() > LIMIT {
            break;
        }
    }
    if output.len() > LIMIT {
        let mut end = LIMIT - OMITTED.len();
        while !output.is_char_boundary(end) {
            end -= 1;
        }
        output.truncate(end);
        output.push_str(OMITTED);
    }
    output
}

fn checked_host(url: &HttpUrl) -> Result<String, String> {
    if url.host.is_empty() || url.host.len() > 253 || !url.host.is_ascii() {
        return Err("Invalid HTTP host.".into());
    }
    if url.host.contains(':') {
        if !matches!(url.host.parse::<IpAddr>(), Ok(IpAddr::V6(_))) {
            return Err("Invalid IPv6 HTTP host.".into());
        }
    } else if !url
        .host
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err("Invalid HTTP host characters.".into());
    }
    Ok(url.host_header())
}

fn request(url: &HttpUrl, outgoing: &Request, mode: FetchMode) -> Result<String, String> {
    let host = checked_host(url)?;
    if url.path.len() > MAX_LINE_BYTES
        || url
            .query
            .as_ref()
            .is_some_and(|query| query.len() > MAX_LINE_BYTES)
    {
        return Err("HTTP request target exceeds the line-size limit.".into());
    }
    let target = url.request_target();
    if target.len() > MAX_LINE_BYTES - 16
        || !target.starts_with('/')
        || !target
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte) && !matches!(byte, b'#' | b'\\'))
    {
        return Err("Invalid or oversized HTTP request target.".into());
    }
    let cache_headers = match mode {
        FetchMode::Normal => "",
        FetchMode::FullReload => "Cache-Control: no-cache\r\nPragma: no-cache\r\n",
    };
    let form_headers = match outgoing {
        Request::Get => String::new(),
        Request::FormPost { body, origin } => {
            if body.len() > MAX_FORM_BYTES {
                return Err("HTTP form request body exceeds 128 KiB.".into());
            }
            let origin = match origin {
                Some(origin) => format!(
                    "{}://{}",
                    if origin.secure { "https" } else { "http" },
                    checked_host(origin)?
                ),
                None => "null".to_owned(),
            };
            format!("Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nOrigin: {origin}\r\n", body.len())
        }
    };
    let method = outgoing.method();
    Ok(format!(
        "{method} {target} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: {USER_AGENT}\r\nAccept: */*\r\nAccept-Encoding: gzip, deflate\r\n{cache_headers}{form_headers}\r\n"
    ))
}

/// Connections belong to one document load and one immutable trust store.
/// Cancellation applies to that entire load; request deadlines are renewed on
/// checkout. No connection or response survives navigation/reload.
pub struct Pool<'a> {
    cancel: &'a AtomicBool,
    trust: &'a trust::Store,
    state: Mutex<PoolState<'a>>,
    available: Condvar,
    resource_filter: Option<&'a ResourceFilter<'a>>,
    network: network_trace::Trace,
    cookies: Option<Arc<crate::cookies::Session>>,
    cookie_origin: Option<HttpUrl>,
    cookie_context: Option<bool>,
    document_origin: Option<HttpUrl>,
    document_filter: Option<&'a ResourceFilter<'a>>,
}

type ResourceFilter<'a> = dyn Fn(&HttpUrl) -> Result<(), String> + Sync + 'a;

struct PoolState<'a> {
    idle: Vec<Connection<'a>>,
    // Includes idle, leased, and currently connecting streams.
    total: usize,
}

struct Connection<'a> {
    origin: HttpUrl,
    stream: NetworkStream<'a>,
    deadline: Arc<Mutex<Instant>>,
    io: Arc<ExchangeIo>,
}

struct Lease<'p, 'a> {
    pool: &'p Pool<'a>,
    connection: Option<Connection<'a>>,
    reused: bool,
    counted: bool,
}

impl<'a> Pool<'a> {
    pub fn new(cancel: &'a AtomicBool, trust: &'a trust::Store) -> Self {
        Self {
            cancel,
            trust,
            state: Mutex::new(PoolState {
                idle: Vec::new(),
                total: 0,
            }),
            available: Condvar::new(),
            resource_filter: None,
            network: network_trace::Trace::default(),
            cookies: None,
            cookie_origin: None,
            cookie_context: None,
            document_origin: None,
            document_filter: None,
        }
    }

    pub fn with_resource_filter(mut self, filter: &'a ResourceFilter<'a>) -> Self {
        self.resource_filter = Some(filter);
        self
    }

    pub fn with_cookie_session(
        mut self,
        cookies: Arc<crate::cookies::Session>,
        origin: Option<HttpUrl>,
    ) -> Self {
        self.cookies = Some(cookies);
        self.cookie_origin = origin;
        self
    }

    /// Preserve a committed document's context for a user-initiated reload.
    /// A true override never grants same-site status without a matching
    /// initiator or after a schemeful-site-changing redirect.
    pub fn with_cookie_context(mut self, same_site: Option<bool>) -> Self {
        self.cookie_context = same_site;
        self
    }

    /// Restrict script-triggered document requests and every redirected target
    /// to the initiating document's origin and the configured request filter.
    /// Ordinary user document navigation leaves this unset.
    pub fn with_document_origin(mut self, origin: Option<HttpUrl>) -> Self {
        self.document_origin = origin;
        self
    }

    /// A form-action snapshot guards only navigation exchanges, not the
    /// destination document's later subresources. It runs before every hop.
    pub fn with_document_filter(mut self, filter: &'a ResourceFilter<'a>) -> Self {
        self.document_filter = Some(filter);
        self
    }

    pub fn with_network_trace(mut self, trace: network_trace::Trace) -> Self {
        self.network = trace;
        self
    }

    pub fn network_trace(&self) -> &network_trace::Trace {
        &self.network
    }

    pub fn check_resource(&self, url: &HttpUrl) -> Result<(), String> {
        match self.resource_filter {
            Some(filter) => filter(url),
            None => Ok(()),
        }
    }

    #[allow(dead_code)] // Retained pooled GET convenience API for transport callers.
    pub fn fetch(
        &self,
        url: &HttpUrl,
        limit: usize,
        same_origin: Option<&HttpUrl>,
        mode: FetchMode,
    ) -> Result<Response, String> {
        self.fetch_kind(url, limit, same_origin, mode, "resource")
    }

    #[allow(dead_code)] // Retained pooled form API; navigation uses fetch_document.
    pub fn fetch_request(
        &self,
        url: &HttpUrl,
        initial: &Request,
        limit: usize,
        same_origin: Option<&HttpUrl>,
        mode: FetchMode,
    ) -> Result<Response, String> {
        fetch_in_pool(
            self,
            url,
            initial,
            limit,
            same_origin,
            mode,
            true,
            "document",
            false,
            None,
            None,
        )
    }

    pub fn fetch_kind(
        &self,
        url: &HttpUrl,
        limit: usize,
        same_origin: Option<&HttpUrl>,
        mode: FetchMode,
        kind: &'static str,
    ) -> Result<Response, String> {
        fetch_in_pool(
            self,
            url,
            &Request::Get,
            limit,
            same_origin,
            mode,
            true,
            kind,
            false,
            None,
            None,
        )
    }

    /// Same-origin resource fetching with an additional per-element policy.
    /// Both filters run before every destination, including redirect hops.
    #[allow(clippy::too_many_arguments)]
    pub fn fetch_kind_checked(
        &self,
        url: &HttpUrl,
        limit: usize,
        origin: &HttpUrl,
        mode: FetchMode,
        kind: &'static str,
        guard: &ResourceFilter<'_>,
    ) -> Result<Response, String> {
        fetch_in_pool(
            self,
            url,
            &Request::Get,
            limit,
            Some(origin),
            mode,
            true,
            kind,
            false,
            None,
            Some(guard),
        )
    }

    /// Classic script inclusion only: same-origin HTTP(S), or HTTPS from an
    /// HTTPS document. This does not grant fetch/XHR or other resource access.
    /// Third-party sends and Set-Cookie are omitted permanently after origin
    /// exit. TLS, request blocking and the supplied CSP check remain mandatory.
    /// CORS-mode includes set same_origin_only, checked on every redirect hop.
    pub fn fetch_classic_script(
        &self,
        url: &HttpUrl,
        limit: usize,
        origin: &HttpUrl,
        mode: FetchMode,
        same_origin_only: bool,
        guard: &ResourceFilter<'_>,
    ) -> Result<Response, String> {
        check_classic_script_mode(origin, url, same_origin_only)?;
        fetch_in_pool(
            self,
            url,
            &Request::Get,
            limit,
            same_origin_only.then_some(origin),
            mode,
            true,
            "SCRIPT",
            false,
            Some(origin),
            Some(guard),
        )
    }

    /// Apply the document's small body limit after headers, while allowing an
    /// explicit attachment to use the download limit without replaying a POST.
    pub fn fetch_document(
        &self,
        url: &HttpUrl,
        request: &Request,
        mode: FetchMode,
    ) -> Result<Response, String> {
        fetch_in_pool(
            self,
            url,
            request,
            MAX_BODY_BYTES,
            None,
            mode,
            true,
            "document",
            true,
            None,
            None,
        )
    }

    fn acquire(
        &self,
        url: &HttpUrl,
        deadline: Instant,
        allow_reuse: bool,
        events: &mut Vec<String>,
        trace: &network_trace::Ticket,
    ) -> Result<Lease<'_, 'a>, String> {
        trace.phase(PhaseKind::Queue);
        loop {
            check(self.cancel, deadline)?;
            let mut state = self
                .state
                .lock()
                .map_err(|_| "HTTP connection pool failed.")?;
            if allow_reuse {
                if let Some(index) = state
                    .idle
                    .iter()
                    .position(|entry| entry.origin.same_origin(url))
                {
                    let connection = state.idle.swap_remove(index);
                    drop(state);
                    let lease = Lease {
                        pool: self,
                        connection: Some(connection),
                        reused: true,
                        counted: true,
                    };
                    let connection = lease.connection.as_ref().unwrap();
                    // Bytes already waiting cannot belong to a request that has
                    // not been sent. Also reject closed sockets before sending.
                    // TLS control records may conservatively cost a reconnect.
                    if !connection.stream.idle_usable() {
                        events.push("HTTP pool discarded an idle connection with pending data or a closed transport.".into());
                        drop(lease);
                        continue;
                    }
                    *connection
                        .deadline
                        .lock()
                        .map_err(|_| "HTTP deadline state failed.")? = deadline;
                    connection.io.reset();
                    events.push(format!(
                        "HTTP pool reused {} connection.",
                        connection.stream.protocol().unwrap_or("TCP")
                    ));
                    trace.end_phase();
                    return Ok(lease);
                }
            }
            if state.total < MAX_CONNECTIONS || !state.idle.is_empty() {
                // Evict a different origin (or an explicitly bypassed stale
                // entry) rather than exceeding the total connection bound.
                let evicted = if state.total == MAX_CONNECTIONS {
                    state.idle.pop()
                } else {
                    None
                };
                if evicted.is_none() {
                    state.total += 1;
                }
                drop(state);
                drop(evicted);
                let mut lease = Lease {
                    pool: self,
                    connection: None,
                    reused: false,
                    counted: true,
                };
                let clock = Arc::new(Mutex::new(deadline));
                let io = Arc::new(ExchangeIo::default());
                trace.end_phase();
                let stream = open(
                    url,
                    self.trust,
                    self.cancel,
                    deadline,
                    clock.clone(),
                    io.clone(),
                    events,
                    trace,
                )?;
                io.reset(); // Handshake bytes are not HTTP response bytes.
                lease.connection = Some(Connection {
                    origin: url.clone(),
                    stream,
                    deadline: clock,
                    io,
                });
                return Ok(lease);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (state, _) = self
                .available
                .wait_timeout(state, POLL_TIMEOUT.min(remaining))
                .map_err(|_| "HTTP connection pool failed.")?;
            drop(state);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn exchange(
        &self,
        url: &HttpUrl,
        outgoing: &Request,
        request: &str,
        limit: usize,
        deadline: Instant,
        retain: bool,
        events: &mut Vec<String>,
        kind: &'static str,
        document: bool,
    ) -> Result<(Message, Option<&'static str>), String> {
        for attempt in 0..2 {
            let trace = self.network.begin(outgoing.method(), &url.display(), kind);
            let mut lease =
                match self.acquire(url, deadline, attempt == 0 && retain, events, &trace) {
                    Ok(lease) => lease,
                    Err(error) => {
                        trace.fail(&error);
                        return Err(error);
                    }
                };
            let reused = lease.reused;
            let connection = lease.connection.as_mut().unwrap();
            let protocol = connection.stream.protocol();
            trace.connection(reused, protocol);
            let result = (|| {
                trace.phase(PhaseKind::Send);
                send(
                    &mut connection.stream,
                    request.as_bytes(),
                    self.cancel,
                    deadline,
                )
                .map_err(ResponseFailure::before_response)?;
                if outgoing.is_post() {
                    send(
                        &mut connection.stream,
                        outgoing.body(),
                        self.cancel,
                        deadline,
                    )
                    .map_err(ResponseFailure::before_response)?;
                }
                events.push("HTTP request sent; awaiting response.".into());
                trace.phase(PhaseKind::Wait);
                // Redirect bodies are not downloaded; their connections are
                // never retained, including redirects with an empty body.
                read_response_traced(
                    &mut connection.stream,
                    limit,
                    self.cancel,
                    deadline,
                    true,
                    Some(&trace),
                    document,
                )
            })();
            match result {
                Ok(message) => {
                    let redirect = is_redirect(message.status)
                        && header(&message.headers, "location").is_some();
                    trace.complete(
                        message.status,
                        (!redirect).then_some(message.encoded_bytes),
                        (!redirect).then_some(message.body.len()),
                        if redirect {
                            "Redirect response; body intentionally skipped."
                        } else {
                            ""
                        },
                    );
                    if retain && message.reusable && connection.stream.reusable() {
                        check(self.cancel, deadline)?;
                        lease.retain()?;
                    } else {
                        // Completed framed data remains valid if the peer has
                        // already closed before our closing alert is delivered.
                        if let Err(error) = connection.stream.close() {
                            events.push(format!("TLS closing alert could not be sent: {error}"));
                        }
                    }
                    return Ok((message, protocol));
                }
                Err(error) => {
                    let stale = error.http_bytes == 0
                        && (connection.io.closed_without_bytes()
                            || connection.stream.peer_closed());
                    if attempt == 0 && reused && !outgoing.is_post() && stale {
                        trace.fail(&format!(
                            "{}\nStale connection: retrying GET once on a fresh connection.",
                            error.text
                        ));
                        check(self.cancel, deadline)?;
                        events.push("HTTP pool stale GET: no HTTP response bytes; retrying once on a fresh connection.".into());
                        // Drop the failed lease before acquiring its replacement.
                        drop(lease);
                        continue;
                    }
                    trace.fail(&error.text);
                    return Err(error.text);
                }
            }
        }
        Err("HTTP connection retry limit reached.".into())
    }
}

impl Lease<'_, '_> {
    fn retain(mut self) -> Result<(), String> {
        let mut state = self
            .pool
            .state
            .lock()
            .map_err(|_| "HTTP connection pool failed.")?;
        // Cancellation may race the successful response; do not publish it for
        // a subsequent request once the document load has been replaced.
        if !self.pool.cancel.load(Ordering::Acquire) {
            state.idle.push(self.connection.take().unwrap());
            self.counted = false;
        }
        drop(state);
        self.pool.available.notify_one();
        Ok(())
    }
}

impl Drop for Lease<'_, '_> {
    fn drop(&mut self) {
        if self.counted {
            // Drop performs no network I/O. Failed/cancelled leases are never
            // recovered by another request, even if the pool mutex was poisoned.
            self.connection.take();
            if let Ok(mut state) = self.pool.state.lock() {
                state.total -= 1;
            }
            self.pool.available.notify_one();
        }
    }
}

#[derive(Default)]
struct ExchangeIo {
    received: AtomicUsize,
    closed: AtomicBool,
}

impl ExchangeIo {
    fn reset(&self) {
        self.received.store(0, Ordering::Relaxed);
        self.closed.store(false, Ordering::Relaxed);
    }
    fn closed_without_bytes(&self) -> bool {
        self.closed.load(Ordering::Relaxed) && self.received.load(Ordering::Relaxed) == 0
    }
    fn error(&self, error: &io::Error) {
        if matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::NotConnected
                | io::ErrorKind::UnexpectedEof
        ) {
            self.closed.store(true, Ordering::Relaxed);
        }
    }
}

struct Socket {
    stream: TcpStream,
    io: Arc<ExchangeIo>,
}

impl Read for Socket {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let result = self.stream.read(bytes);
        match &result {
            Ok(0) if !bytes.is_empty() => self.io.closed.store(true, Ordering::Relaxed),
            Ok(count) => {
                self.io.received.fetch_add(*count, Ordering::Relaxed);
            }
            Err(error) => self.io.error(error),
        }
        result
    }
}

impl Write for Socket {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = self.stream.write(bytes);
        if let Err(error) = &result {
            self.io.error(error);
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

type DeadlineCheck<'a> = Box<dyn Fn() -> Result<(), String> + Send + 'a>;

enum NetworkStream<'a> {
    Plain(Socket),
    Encrypted(tls::TlsStream<Socket, DeadlineCheck<'a>>),
}

impl Read for NetworkStream<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(bytes),
            Self::Encrypted(stream) => stream.read(bytes),
        }
    }
}

impl Write for NetworkStream<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(bytes),
            Self::Encrypted(stream) => stream.write(bytes),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Encrypted(stream) => stream.flush(),
        }
    }
}

impl NetworkStream<'_> {
    fn reusable(&self) -> bool {
        match self {
            Self::Plain(_) => true,
            Self::Encrypted(stream) => stream.reusable(),
        }
    }

    fn peer_closed(&self) -> bool {
        match self {
            Self::Plain(_) => false,
            Self::Encrypted(stream) => stream.peer_closed(),
        }
    }

    fn idle_usable(&self) -> bool {
        if !self.reusable() {
            return false;
        }
        let socket = match self {
            Self::Plain(socket) => socket,
            Self::Encrypted(stream) => stream.get_ref(),
        };
        if socket.stream.set_nonblocking(true).is_err() {
            return false;
        }
        let mut byte = [0];
        let ready = socket.stream.peek(&mut byte);
        let restored = socket.stream.set_nonblocking(false).is_ok();
        restored && ready.is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock)
    }

    fn protocol(&self) -> Option<&'static str> {
        match self {
            Self::Plain(_) => None,
            Self::Encrypted(stream) => Some(stream.protocol()),
        }
    }

    fn close(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(_) => Ok(()),
            Self::Encrypted(stream) => stream.close(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn open<'a>(
    url: &HttpUrl,
    trust: &trust::Store,
    cancel: &'a AtomicBool,
    deadline: Instant,
    clock: Arc<Mutex<Instant>>,
    io: Arc<ExchangeIo>,
    events: &mut Vec<String>,
    trace: &network_trace::Ticket,
) -> Result<NetworkStream<'a>, String> {
    if !url.secure {
        return Ok(NetworkStream::Plain(Socket {
            stream: connect(url, cancel, deadline, events, trace)?,
            io,
        }));
    }
    // Trust and fresh OS entropy must be available before connecting. The
    // HTTP request is sent only after TLS authenticates the peer completely.
    let roots = trust.roots()?;
    let mut entropy = [0u8; 64];
    random::fill(&mut entropy)?;
    let stream = Socket {
        stream: connect(url, cancel, deadline, events, trace)?,
        io,
    };
    let deadline_check: DeadlineCheck<'a> = Box::new(move || {
        let deadline = *clock.lock().map_err(|_| "HTTP deadline state failed.")?;
        check(cancel, deadline)
    });
    let started = Instant::now();
    trace.phase(PhaseKind::Tls);
    let secure = tls::TlsStream::connect(
        stream,
        &url.host,
        roots,
        SystemTime::now(),
        entropy,
        deadline_check,
    );
    events.push(format!(
        "Connection timing: TLS handshake={} ms; outcome={}",
        started.elapsed().as_millis(),
        if secure.is_ok() { "verified" } else { "failed" }
    ));
    trace.end_phase();
    let secure = secure?;
    events.push(format!("HTTPS trust: {}", trust.description()));
    events.push(secure.certificate_summary().to_owned());
    Ok(NetworkStream::Encrypted(secure))
}

fn check(cancel: &AtomicBool, deadline: Instant) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("HTTP request cancelled.".into())
    } else if Instant::now() >= deadline {
        Err("HTTP request timed out.".into())
    } else {
        Ok(())
    }
}

fn retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

fn connect(
    url: &HttpUrl,
    cancel: &AtomicBool,
    deadline: Instant,
    events: &mut Vec<String>,
    trace: &network_trace::Ticket,
) -> Result<TcpStream, String> {
    check(cancel, deadline)?;
    // At most two summaries per attempt sequence. Resolved IPs and request
    // query values are deliberately absent; redirects already have a fixed cap.
    // OS DNS resolution can block inside ToSocketAddrs. It is deliberately not
    // put on an unbounded series of detached resolver threads.
    let started = Instant::now();
    let literal = url.host.parse::<IpAddr>();
    if literal.is_err() {
        trace.phase(PhaseKind::Dns);
    }
    let addresses: Result<Vec<SocketAddr>, String> = match literal {
        Ok(ip) => Ok(vec![SocketAddr::new(ip, url.port)]),
        Err(_) => (url.host.as_str(), url.port)
            .to_socket_addrs()
            .map(|addresses| addresses.take(8).collect())
            .map_err(|error| format!("DNS lookup failed: {error}")),
    };
    events.push(format!(
        "Connection timing: DNS lookup={} ms; outcome={}; address candidates={}",
        started.elapsed().as_millis(),
        if literal.is_ok() {
            "skipped (literal address)"
        } else if addresses.is_ok() {
            "resolved"
        } else {
            "failed"
        },
        addresses.as_ref().map_or(0, Vec::len)
    ));
    trace.end_phase();
    let addresses = addresses?;
    let started = Instant::now();
    trace.phase(PhaseKind::Connect);
    let mut attempts = 0;
    let result = (|| {
        check(cancel, deadline)?;
        let mut failure = "The host has no usable network address.".to_owned();
        for address in addresses {
            check(cancel, deadline)?;
            let timeout = CONNECT_TIMEOUT.min(deadline.saturating_duration_since(Instant::now()));
            attempts += 1;
            match TcpStream::connect_timeout(&address, timeout) {
                Ok(stream) => {
                    // TLS handshake records and form headers/body are separate
                    // writes. Send them promptly instead of waiting for a delayed
                    // ACK of an earlier small record before the peer can proceed.
                    stream
                        .set_nodelay(true)
                        .map_err(|error| format!("Cannot disable TCP send delay: {error}"))?;
                    stream
                        .set_read_timeout(Some(POLL_TIMEOUT))
                        .map_err(|error| format!("Cannot set HTTP read timeout: {error}"))?;
                    stream
                        .set_write_timeout(Some(POLL_TIMEOUT))
                        .map_err(|error| format!("Cannot set HTTP write timeout: {error}"))?;
                    check(cancel, deadline)?;
                    return Ok(stream);
                }
                Err(error) => failure = format!("HTTP connection failed: {error}"),
            }
        }
        check(cancel, deadline)?;
        Err(failure)
    })();
    events.push(format!(
        "Connection timing: TCP connect={} ms; attempts={attempts}; outcome={}",
        started.elapsed().as_millis(),
        if result.is_ok() {
            "connected"
        } else {
            "failed"
        }
    ));
    trace.end_phase();
    result
}

fn send<W: Write>(
    stream: &mut W,
    mut bytes: &[u8],
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<(), String> {
    while !bytes.is_empty() {
        check(cancel, deadline)?;
        match stream.write(bytes) {
            Ok(0) => return Err("HTTP connection closed while sending the request.".into()),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if retryable(&error) => {}
            Err(error) => return Err(format!("HTTP request write failed: {error}")),
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Budget {
    Headers,
    Chunks,
}

struct Input<'a, R> {
    source: R,
    buffer: [u8; 8192],
    position: usize,
    filled: usize,
    wire_bytes: usize,
    wire_limit: usize,
    header_bytes: usize,
    chunk_bytes: usize,
    stage: &'static str,
    response_status: Option<u16>,
    framing: Option<Framing>,
    body_bytes: usize,
    codings: Vec<ContentCoding>,
    cancel: &'a AtomicBool,
    deadline: Instant,
}

impl<'a, R: Read> Input<'a, R> {
    fn new(source: R, limit: usize, cancel: &'a AtomicBool, deadline: Instant) -> Self {
        Self {
            source,
            buffer: [0; 8192],
            position: 0,
            filled: 0,
            wire_bytes: 0,
            wire_limit: limit + MAX_HEADER_BYTES + MAX_CHUNK_METADATA + 1,
            header_bytes: 0,
            chunk_bytes: 0,
            stage: "status line",
            response_status: None,
            framing: None,
            body_bytes: 0,
            codings: Vec::new(),
            cancel,
            deadline,
        }
    }

    fn explain_failure(&self, error: String) -> String {
        let status = self
            .response_status
            .map_or_else(|| "not received".into(), |status| status.to_string());
        let framing = self
            .framing
            .map_or_else(|| "not determined".into(), Framing::description);
        let coding = if self.codings.is_empty() {
            "none".into()
        } else {
            self.codings
                .iter()
                .map(|coding| coding.name())
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!("{error}\nHTTP response progress: stage={}; status={status}; framing={framing}; encoded body received={} bytes; HTTP bytes received={}; content coding={coding}.", self.stage, self.body_bytes, self.wire_bytes)
    }

    fn available(&mut self) -> Result<bool, String> {
        check(self.cancel, self.deadline)?;
        if self.position < self.filled {
            return Ok(true);
        }
        if self.wire_bytes == self.wire_limit {
            return Err("HTTP encoded response exceeds its size limit.".into());
        }
        let capacity = self.buffer.len().min(self.wire_limit - self.wire_bytes);
        loop {
            check(self.cancel, self.deadline)?;
            match self.source.read(&mut self.buffer[..capacity]) {
                Ok(count) => {
                    self.position = 0;
                    self.filled = count;
                    self.wire_bytes += count;
                    return Ok(count != 0);
                }
                Err(error) if retryable(&error) => {}
                Err(error) => return Err(format!("HTTP response read failed: {error}")),
            }
        }
    }

    fn byte(&mut self) -> Result<u8, String> {
        if !self.available()? {
            return Err("HTTP response ended before the message was complete.".into());
        }
        let value = self.buffer[self.position];
        self.position += 1;
        Ok(value)
    }

    fn account(&mut self, budget: Budget, count: usize) -> Result<(), String> {
        let (used, maximum, message) = match budget {
            Budget::Headers => (
                &mut self.header_bytes,
                MAX_HEADER_BYTES,
                "HTTP headers exceed 32 KiB.",
            ),
            Budget::Chunks => (
                &mut self.chunk_bytes,
                MAX_CHUNK_METADATA,
                "HTTP chunk metadata exceeds its size limit.",
            ),
        };
        if count > maximum - *used {
            return Err(message.into());
        }
        *used += count;
        Ok(())
    }

    fn line(&mut self, budget: Budget) -> Result<Vec<u8>, String> {
        let mut line = Vec::new();
        loop {
            if line.len() == MAX_LINE_BYTES {
                return Err("HTTP line exceeds 8 KiB.".into());
            }
            self.account(budget, 1)?;
            let byte = self.byte()?;
            line.push(byte);
            if byte == b'\n' {
                if line.len() < 2 || line[line.len() - 2] != b'\r' {
                    return Err("HTTP lines must end with CRLF.".into());
                }
                line.truncate(line.len() - 2);
                return Ok(line);
            }
        }
    }

    fn body_bytes(
        &mut self,
        output: &mut Vec<u8>,
        mut count: usize,
        limit: usize,
    ) -> Result<(), String> {
        reserve_body(output, count, limit)?;
        while count > 0 {
            if !self.available()? {
                return Err("HTTP body is truncated.".into());
            }
            let take = count.min(self.filled - self.position);
            output.extend_from_slice(&self.buffer[self.position..self.position + take]);
            self.body_bytes += take;
            self.position += take;
            count -= take;
        }
        Ok(())
    }

    fn body_to_eof(&mut self, output: &mut Vec<u8>, limit: usize) -> Result<(), String> {
        while self.available()? {
            let count = self.filled - self.position;
            reserve_body(output, count, limit)?;
            output.extend_from_slice(&self.buffer[self.position..self.filled]);
            self.body_bytes += count;
            self.position = self.filled;
        }
        Ok(())
    }
}

fn reserve_body(output: &mut Vec<u8>, count: usize, limit: usize) -> Result<(), String> {
    if count > limit.saturating_sub(output.len()) {
        return Err(format!("HTTP body exceeds the {limit}-byte limit."));
    }
    let needed = output.len() + count;
    if needed > output.capacity() {
        let capacity = needed
            .max(output.capacity().saturating_mul(2))
            .max(8192)
            .min(limit);
        output
            .try_reserve_exact(capacity - output.len())
            .map_err(|_| "Not enough memory for the HTTP body.")?;
    }
    Ok(())
}

#[derive(Debug)]
struct Message {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    encoded_bytes: usize,
    codings: Vec<ContentCoding>,
    framing: Framing,
    reusable: bool,
}

#[derive(Debug)]
struct ResponseFailure {
    text: String,
    http_bytes: usize,
}

impl ResponseFailure {
    fn before_response(text: String) -> Self {
        Self {
            text,
            http_bytes: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Framing {
    Empty,
    Length(usize),
    Chunked,
    Close,
}

impl Framing {
    fn description(self) -> String {
        match self {
            Self::Empty => "no body".into(),
            Self::Length(length) => format!("Content-Length {length}"),
            Self::Chunked => "chunked".into(),
            Self::Close => "connection close (no Content-Length or chunked encoding)".into(),
        }
    }
}

#[cfg(test)]
fn read_response<R: Read>(
    source: R,
    limit: usize,
    cancel: &AtomicBool,
    deadline: Instant,
    skip_redirect_body: bool,
) -> Result<Message, String> {
    read_response_detailed(source, limit, cancel, deadline, skip_redirect_body)
        .map_err(|error| error.text)
}

#[cfg(test)]
fn read_response_detailed<R: Read>(
    source: R,
    limit: usize,
    cancel: &AtomicBool,
    deadline: Instant,
    skip_redirect_body: bool,
) -> Result<Message, ResponseFailure> {
    read_response_traced(
        source,
        limit,
        cancel,
        deadline,
        skip_redirect_body,
        None,
        false,
    )
}

struct TracedRead<'a, R> {
    source: R,
    trace: Option<&'a network_trace::Ticket>,
}
impl<R: Read> Read for TracedRead<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.source.read(bytes)?;
        if let Some(trace) = self.trace.filter(|_| count != 0) {
            trace.received(count);
        }
        Ok(count)
    }
}

fn read_response_traced<R: Read>(
    source: R,
    limit: usize,
    cancel: &AtomicBool,
    deadline: Instant,
    skip_redirect_body: bool,
    trace: Option<&network_trace::Ticket>,
    document: bool,
) -> Result<Message, ResponseFailure> {
    if limit > MAX_BODY_BYTES {
        return Err(ResponseFailure::before_response(
            "HTTP body limit cannot exceed 32 MiB.".into(),
        ));
    }
    let mut input = Input::new(TracedRead { source, trace }, limit, cancel, deadline);
    let result = (|| {
        let mut connection_persistent = true;
        for information in 0..=MAX_INFORMATIONAL {
            input.stage = "status line";
            let (status, version_10) = status_line(&input.line(Budget::Headers)?)?;
            input.response_status = Some(status);
            input.stage = "headers";
            let headers = fields(&mut input, false)?;
            connection_persistent &= !version_10 && persistent_headers(&headers);
            if (100..200).contains(&status) {
                if status == 101 {
                    return Err("HTTP protocol upgrades are not supported.".into());
                }
                if information == MAX_INFORMATIONAL {
                    return Err("Too many informational HTTP responses.".into());
                }
                if header(&headers, "content-length").is_some()
                    || header(&headers, "transfer-encoding").is_some()
                {
                    return Err("Informational HTTP responses cannot declare body framing.".into());
                }
                continue;
            }
            input.stage = "framing headers";
            if let Some(trace) = trace {
                trace.response(status);
            }
            let framing = framing(status, version_10, &headers)?;
            input.framing = Some(framing);
            let redirect = is_redirect(status) && unique_header(&headers, "location")?.is_some();
            let mut body = Vec::new();
            let mut encoded_bytes = 0;
            let mut codings = Vec::new();
            if !matches!(framing, Framing::Empty) && !(skip_redirect_body && redirect) {
                // 205 must have zero content even if representation metadata is
                // present. As with 204/304, there is no encoded body to decompress.
                if status != 205 {
                    input.stage = "content coding";
                    codings = content_codings(&headers)?;
                }
                input.codings = codings.clone();
                let attachment = header(&headers, "content-disposition")
                    .and_then(|value| value.split(';').next())
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case("attachment"));
                let body_limit = if status == 205 {
                    0
                } else if document && !attachment {
                    limit.min(2 * 1024 * 1024)
                } else {
                    limit
                };
                input.stage = "body";
                match framing {
                    Framing::Length(count) => input.body_bytes(&mut body, count, body_limit)?,
                    Framing::Chunked => chunked(&mut input, &mut body, body_limit)?,
                    Framing::Close => input.body_to_eof(&mut body, body_limit)?,
                    Framing::Empty => {}
                }
                encoded_bytes = body.len();
                input.stage = "content decoding";
                if let Some(trace) = trace.filter(|_| !codings.is_empty()) {
                    trace.phase(PhaseKind::Decode);
                }
                // Each intermediate result obeys the same resource limit. Keep the
                // original headers for inspection; framing was already completed
                // using the encoded representation's length, never the decoded one.
                for coding in codings.iter().rev() {
                    check(cancel, deadline)?;
                    let allowed = || check(cancel, deadline);
                    body = match coding {
                        ContentCoding::Gzip => compression::gzip(&body, body_limit, &allowed),
                        ContentCoding::Deflate => compression::zlib(&body, body_limit, &allowed),
                        ContentCoding::Identity => Ok(body),
                    }
                    .map_err(|error| {
                        format!("HTTP {} content decoding failed: {error}", coding.name())
                    })?;
                }
            }
            check(cancel, deadline)?;
            // Every byte of this response must have a known boundary. Buffered
            // trailing bytes cannot be silently thrown away before another
            // request. Redirect bodies are deliberately skipped and discarded.
            let empty_has_framing = matches!(framing, Framing::Empty)
                && (header(&headers, "content-length").is_some()
                    || header(&headers, "transfer-encoding").is_some());
            let reusable = connection_persistent
                && !matches!(framing, Framing::Close)
                && !(skip_redirect_body && redirect)
                && input.position == input.filled
                && !empty_has_framing;
            return Ok(Message {
                status,
                headers,
                body,
                encoded_bytes,
                codings,
                framing,
                reusable,
            });
        }
        Err("The HTTP response has no final status.".into())
    })();
    result.map_err(|error| ResponseFailure {
        text: input.explain_failure(error),
        http_bytes: input.wire_bytes,
    })
}

fn persistent_headers(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .filter(|(name, _)| name == "connection")
        .all(|(_, value)| {
            value.split(',').all(|item| {
                let item = item.trim_matches([' ', '\t']);
                !item.is_empty()
                    && item.bytes().all(token)
                    && !item.eq_ignore_ascii_case("close")
                    && !item.eq_ignore_ascii_case("upgrade")
            })
        })
}

fn status_line(line: &[u8]) -> Result<(u16, bool), String> {
    if line.len() < 13
        || !matches!(&line[..8], b"HTTP/1.1" | b"HTTP/1.0")
        || line[8] != b' '
        || line[12] != b' '
        || !line[9..12].iter().all(u8::is_ascii_digit)
        || !line[13..]
            .iter()
            .all(|&byte| byte == b'\t' || byte >= 0x20 && byte != 0x7f)
    {
        return Err("Malformed or unsupported HTTP status line.".into());
    }
    let status = u16::from(line[9] - b'0') * 100
        + u16::from(line[10] - b'0') * 10
        + u16::from(line[11] - b'0');
    if !(100..600).contains(&status) {
        return Err("Invalid HTTP status code.".into());
    }
    Ok((status, &line[..8] == b"HTTP/1.0"))
}

fn token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

fn field(line: &[u8]) -> Result<(String, String), String> {
    let colon = line
        .iter()
        .position(|&byte| byte == b':')
        .ok_or("HTTP header is missing its colon.")?;
    if colon == 0 || !line[..colon].iter().copied().all(token) {
        return Err("Invalid HTTP field name or obsolete folded header.".into());
    }
    let value = trim_ows(&line[colon + 1..]);
    if !value
        .iter()
        .all(|&byte| byte == b'\t' || byte >= 0x20 && byte != 0x7f)
    {
        return Err("HTTP field contains a forbidden control character.".into());
    }
    let name: String = line[..colon]
        .iter()
        .map(|byte| char::from(byte.to_ascii_lowercase()))
        .collect();
    // Field bytes above ASCII remain opaque Latin-1 values, not lossy UTF-8.
    let value = value.iter().copied().map(char::from).collect();
    Ok((name, value))
}

fn fields<R: Read>(
    input: &mut Input<'_, R>,
    trailers: bool,
) -> Result<Vec<(String, String)>, String> {
    let mut headers = Vec::new();
    loop {
        let line = input.line(Budget::Headers)?;
        if line.is_empty() {
            return Ok(headers);
        }
        if headers.len() == MAX_HEADERS {
            return Err("HTTP field count exceeds 200.".into());
        }
        let entry = field(&line)?;
        if trailers
            && matches!(
                entry.0.as_str(),
                "content-length"
                    | "transfer-encoding"
                    | "host"
                    | "connection"
                    | "trailer"
                    | "te"
                    | "upgrade"
                    | "content-encoding"
                    | "content-type"
                    | "content-range"
                    | "location"
                    | "authorization"
                    | "proxy-authorization"
                    | "cookie"
                    | "set-cookie"
            )
        {
            return Err("HTTP trailer attempts to supply a prohibited field.".into());
        }
        headers.push(entry);
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn unique_header<'a>(
    headers: &'a [(String, String)],
    name: &str,
) -> Result<Option<&'a str>, String> {
    let mut matches = headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let value = matches.next().map(|(_, value)| value.as_str());
    if matches.next().is_some() {
        return Err(format!("Duplicate HTTP {name} fields are ambiguous."));
    }
    Ok(value)
}

fn framing(status: u16, version_10: bool, headers: &[(String, String)]) -> Result<Framing, String> {
    // These statuses end at the header terminator, even if representation
    // metadata such as a 304 Content-Length describes a nonempty resource.
    if matches!(status, 204 | 304) {
        return Ok(Framing::Empty);
    }
    let transfer = header(headers, "transfer-encoding").is_some();
    let length = header(headers, "content-length").is_some();
    if transfer && length {
        return Err("HTTP response has both Transfer-Encoding and Content-Length.".into());
    }
    if transfer {
        if version_10 {
            return Err("HTTP/1.0 cannot use Transfer-Encoding.".into());
        }
        let codings: Vec<&str> = headers
            .iter()
            .filter(|(name, _)| name == "transfer-encoding")
            .flat_map(|(_, value)| value.split(','))
            .map(|value| value.trim_matches([' ', '\t']))
            .collect();
        if codings.len() != 1 || !codings[0].eq_ignore_ascii_case("chunked") {
            return Err("Only a single chunked HTTP transfer coding is supported.".into());
        }
        return Ok(Framing::Chunked);
    }
    let mut size = None;
    for (_, value) in headers.iter().filter(|(name, _)| name == "content-length") {
        for item in value.split(',') {
            let item = item.trim_matches([' ', '\t']);
            if item.is_empty() || !item.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("Invalid HTTP Content-Length.".into());
            }
            let count = item
                .bytes()
                .try_fold(0usize, |value, byte| {
                    value
                        .checked_mul(10)
                        .and_then(|value| value.checked_add((byte - b'0') as usize))
                })
                .ok_or("HTTP Content-Length is too large.")?;
            if size.is_some_and(|previous| previous != count) {
                return Err("Conflicting HTTP Content-Length values.".into());
            }
            size = Some(count);
        }
    }
    Ok(size.map_or(Framing::Close, Framing::Length))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContentCoding {
    Identity,
    Gzip,
    Deflate,
}

impl ContentCoding {
    fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
        }
    }
}

fn content_codings(headers: &[(String, String)]) -> Result<Vec<ContentCoding>, String> {
    let mut codings = Vec::new();
    let mut empty = 0;
    for (_, value) in headers
        .iter()
        .filter(|(name, _)| name == "content-encoding")
    {
        for coding in value
            .split(',')
            .map(|value| value.trim_matches([' ', '\t']))
        {
            if coding.is_empty() {
                empty += 1;
                if empty > MAX_EMPTY_CODINGS {
                    return Err("HTTP Content-Encoding has too many empty list elements.".into());
                }
                continue;
            }
            if !coding.bytes().all(token) {
                return Err("Malformed HTTP Content-Encoding token.".into());
            }
            if codings.len() == MAX_CONTENT_CODINGS {
                return Err("HTTP Content-Encoding exceeds the four-layer limit.".into());
            }
            codings.push(if coding.eq_ignore_ascii_case("identity") {
                ContentCoding::Identity
            } else if coding.eq_ignore_ascii_case("gzip") || coding.eq_ignore_ascii_case("x-gzip") {
                ContentCoding::Gzip
            } else if coding.eq_ignore_ascii_case("deflate") {
                ContentCoding::Deflate
            } else {
                return Err(format!("HTTP Content-Encoding '{coding}' is not implemented; supported encodings are gzip and deflate."));
            });
        }
    }
    Ok(codings)
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn chunked<R: Read>(
    input: &mut Input<'_, R>,
    output: &mut Vec<u8>,
    limit: usize,
) -> Result<(), String> {
    for _ in 0..MAX_CHUNKS {
        input.stage = "chunk size";
        let size = chunk_size(&input.line(Budget::Chunks)?)?;
        if size == 0 {
            // Trailer fields are validated and discarded, never merged into the
            // response headers used for MIME, routing, or framing decisions.
            input.stage = "chunk trailers";
            fields(input, true)?;
            return Ok(());
        }
        input.stage = "chunk data";
        input.body_bytes(output, size, limit)?;
        input.stage = "chunk terminator";
        input.account(Budget::Chunks, 2)?;
        if input.byte()? != b'\r' || input.byte()? != b'\n' {
            return Err("HTTP chunk data is missing its terminating CRLF.".into());
        }
    }
    Err("HTTP chunk count exceeds 65536.".into())
}

fn chunk_size(line: &[u8]) -> Result<usize, String> {
    let mut position = 0usize;
    let mut size = 0usize;
    while let Some(&byte) = line.get(position) {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => break,
        };
        size = size
            .checked_mul(16)
            .and_then(|value| value.checked_add(digit as usize))
            .ok_or("HTTP chunk size is too large.")?;
        position += 1;
    }
    if position == 0 {
        return Err("Invalid HTTP chunk size.".into());
    }
    extensions(&line[position..])?;
    Ok(size)
}

fn extensions(mut rest: &[u8]) -> Result<(), String> {
    while !rest.is_empty() {
        rest = trim_ows(rest);
        if rest.is_empty() {
            return Ok(());
        }
        if rest[0] != b';' {
            return Err("Invalid HTTP chunk extension.".into());
        }
        rest = trim_ows(&rest[1..]);
        let count = rest.iter().copied().take_while(|&byte| token(byte)).count();
        if count == 0 {
            return Err("HTTP chunk extension has no name.".into());
        }
        rest = trim_ows(&rest[count..]);
        if rest.first() == Some(&b'=') {
            rest = trim_ows(&rest[1..]);
            if rest.first() == Some(&b'"') {
                rest = &rest[1..];
                let mut closed = false;
                while let Some((&byte, tail)) = rest.split_first() {
                    rest = tail;
                    if byte == b'"' {
                        closed = true;
                        break;
                    }
                    if byte == b'\\' {
                        let (&escaped, tail) = rest
                            .split_first()
                            .ok_or("Truncated quoted HTTP chunk extension.")?;
                        if !(escaped == b'\t' || escaped >= 0x20 && escaped != 0x7f) {
                            return Err("Invalid escaped HTTP chunk-extension byte.".into());
                        }
                        rest = tail;
                    } else if !(byte == b'\t' || byte >= 0x20 && byte != 0x7f) {
                        return Err("Invalid HTTP chunk-extension byte.".into());
                    }
                }
                if !closed {
                    return Err("Unclosed quoted HTTP chunk extension.".into());
                }
            } else {
                let count = rest.iter().copied().take_while(|&byte| token(byte)).count();
                if count == 0 {
                    return Err("HTTP chunk extension has no value.".into());
                }
                rest = &rest[count..];
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn classic_script_origin_policy_is_secure_and_does_not_change_general_resources() {
        let url = |text| crate::url::parse(text).unwrap().0;
        let secure = url("https://example.test/page");
        let plain = url("http://example.test/page");
        for target in [
            "https://example.test/a.js",
            "https://cdn.test/a.js",
            "https://cdn.test:8443/a.js",
        ] {
            assert!(check_classic_script_origin(&secure, &url(target)).is_ok());
        }
        assert!(check_classic_script_origin(&plain, &url("http://example.test/a.js")).is_ok());
        for (origin, target) in [
            (&secure, "http://example.test/a.js"),
            (&secure, "http://cdn.test/a.js"),
            (&plain, "http://cdn.test/a.js"),
            (&plain, "https://cdn.test/a.js"),
        ] {
            assert!(check_classic_script_origin(origin, &url(target)).is_err());
        }
        let cancel = AtomicBool::new(false);
        let trust = trust::Store::unconfigured();
        let pool = Pool::new(&cancel, &trust);
        // This is rejected before DNS/TLS; generic resources keep exact origin.
        let error = pool
            .fetch_kind(
                &url("https://cdn.test/a.css"),
                1024,
                Some(&secure),
                FetchMode::Normal,
                "CSS",
            )
            .unwrap_err();
        assert!(error.contains("Cross-origin resource"), "{error}");
    }

    #[test]
    fn classic_script_corp_checks_exact_origin_and_conservative_site_scope() {
        let url = |text| crate::url::parse(text).unwrap().0;
        let origin = url("https://example.test/page");
        let same = url("https://example.test/app.js");
        let port = url("https://example.test:8443/app.js");
        let other = url("https://cdn.example.test/app.js");
        let headers = |value: &str| vec![("cross-origin-resource-policy".into(), value.into())];
        assert!(check_classic_script_corp(&origin, &same, &headers("same-origin")).is_ok());
        assert!(check_classic_script_corp(&origin, &port, &headers("same-origin")).is_err());
        assert!(check_classic_script_corp(&origin, &port, &headers("same-site")).is_ok());
        assert!(check_classic_script_corp(&origin, &other, &headers("same-site")).is_err());
        assert!(check_classic_script_corp(&origin, &other, &headers("cross-origin")).is_ok());
        assert!(check_classic_script_corp(&origin, &other, &[]).is_ok());
        assert!(check_classic_script_corp(&origin, &other, &headers("SAME-ORIGIN")).is_ok());
        let mut duplicate = headers("cross-origin");
        duplicate.extend(headers("same-origin"));
        assert!(check_classic_script_corp(&origin, &other, &duplicate).is_err());
    }

    // Exercise parser boundaries independently of the socket's read sizes.
    struct Pieces<'a> {
        input: Cursor<&'a [u8]>,
        size: usize,
    }

    impl Read for Pieces<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let count = output.len().min(self.size);
            self.input.read(&mut output[..count])
        }
    }

    fn parse(input: &[u8], limit: usize) -> Result<Message, String> {
        read_response(
            Pieces {
                input: Cursor::new(input),
                size: 3,
            },
            limit,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(2),
            false,
        )
    }

    #[test]
    fn persistence_requires_complete_unambiguous_http11() {
        let parse = |wire: &[u8], skip_redirect| {
            read_response(
                Cursor::new(wire),
                1024,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(2),
                skip_redirect,
            )
            .unwrap()
        };
        for wire in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody".as_slice(),
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nbody\r\n0\r\nX-Note: ok\r\n\r\n",
            b"HTTP/1.1 204 No Content\r\n\r\n",
            b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n",
        ] { assert!(parse(wire, true).reusable, "{wire:?}"); }
        for wire in [
            b"HTTP/1.0 200 OK\r\nContent-Length: 4\r\nConnection: keep-alive\r\n\r\nbody".as_slice(),
            b"HTTP/1.1 200 OK\r\n\r\nbody",
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: Keep-Alive, CLOSE\r\n\r\nbody",
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: keep-alive\r\nConnection: close\r\n\r\nbody",
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: bad value\r\n\r\nbody",
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbodyEXTRA",
            b"HTTP/1.1 204 No Content\r\nContent-Length: 4\r\n\r\n",
            b"HTTP/1.1 304 Not Modified\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 100 Continue\r\nConnection: close\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
        ] { assert!(!parse(wire, true).reusable, "{wire:?}"); }
    }

    #[test]
    fn retry_metadata_counts_all_received_http_bytes() {
        for wire in [
            b"".as_slice(),
            b"H",
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nbody",
        ] {
            let error = read_response_detailed(
                Cursor::new(wire),
                1024,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(2),
                true,
            )
            .err()
            .unwrap();
            assert_eq!(error.http_bytes, wire.len());
        }
    }

    #[test]
    fn fixed_and_close_delimited_bodies() {
        let message = parse(b"HTTP/1.1 200 OK\r\nContent-Length: 7, 7\r\ncontent-length: 7\r\nX-Test: \tvalue \t\r\n\r\nabc\0defEXTRA", 7).unwrap();
        assert_eq!(message.status, 200);
        assert_eq!(message.body, b"abc\0def");
        assert_eq!(header(&message.headers, "X-Test"), Some("value"));
        let message = parse(b"HTTP/1.0 404 Not Found\r\n\r\nnot here", 8).unwrap();
        assert_eq!(message.status, 404);
        assert_eq!(message.body, b"not here");
        assert!(
            parse(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nfour", 20)
                .unwrap_err()
                .contains("truncated")
        );
        assert!(parse(b"HTTP/1.1 200 OK\r\n\r\nfive!", 4)
            .unwrap_err()
            .contains("limit"));
    }

    #[test]
    fn transport_failure_requires_a_complete_http_boundary() {
        // A TLS transport reports missing close_notify as an error. Content-
        // Length and chunked responses must finish without asking for another
        // byte; a close-delimited response cannot infer a complete body.
        struct Abrupt<'a>(Cursor<&'a [u8]>);
        impl Read for Abrupt<'_> {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                let count = output.len().min(7);
                match self.0.read(&mut output[..count])? {
                    0 => Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "TLS transport ended without close_notify (truncated connection).",
                    )),
                    count => Ok(count),
                }
            }
        }
        let parse = |wire: &[u8]| {
            read_response(
                Abrupt(Cursor::new(wire)),
                1024,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(2),
                false,
            )
        };
        for wire in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody".as_slice(),
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nbody\r\n0\r\n\r\n",
        ] {
            assert_eq!(parse(wire).unwrap().body, b"body");
        }
        assert_eq!(
            parse(b"HTTP/1.1 204 No Content\r\n\r\n").unwrap().status,
            204
        );
        for (wire, stage, framing, received) in [
            (b"".as_slice(), "status line", "not determined", 0),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n",
                "headers",
                "not determined",
                0,
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nbody",
                "body",
                "Content-Length 100",
                4,
            ),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nbody\r\n",
                "chunk size",
                "chunked",
                4,
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\n\r\nbody",
                "body",
                "connection close (no Content-Length or chunked encoding)",
                4,
            ),
        ] {
            let error = parse(wire).unwrap_err();
            assert!(error.contains("without close_notify"), "{error}");
            assert!(error.contains(&format!("stage={stage};")), "{error}");
            assert!(error.contains(&format!("framing={framing};")), "{error}");
            assert!(
                error.contains(&format!("encoded body received={received} bytes;")),
                "{error}"
            );
            if wire.is_empty() {
                assert!(error.contains("status=not received;"), "{error}");
            } else {
                assert!(error.contains("status=200;"), "{error}");
            }
            if wire.windows(4).any(|bytes| bytes == b"gzip") {
                assert!(error.contains("content coding=gzip."), "{error}");
            }
        }
    }

    #[test]
    fn failure_activity_is_bounded_and_omits_query_values() {
        let events = [
            "POST https://example.test/start?secret=form".into(),
            "FORM BODY: 12 bytes; values omitted from this report.".into(),
            "TLS 1.2: certificate chain verified".into(),
            "HTTP 303 redirect to https://example.test/result?token=private".into(),
            "GET https://example.test/result?token=private".into(),
        ];
        let output = failure_with_activity("HTTP body is truncated.".into(), &events);
        assert!(output.starts_with("HTTP body is truncated."));
        assert!(output.contains("TLS 1.2: certificate chain verified"));
        assert_eq!(output.matches("?[query omitted]").count(), 3);
        assert!(!output.contains("secret") && !output.contains("private"));
        let output = failure_with_activity("failure".into(), &["€".repeat(20_000)]);
        assert!(output.len() <= 16 * 1024);
        assert!(output.ends_with("[additional activity omitted]"));
    }

    #[test]
    fn ambiguous_and_unsupported_framing_is_rejected() {
        for fields in [
            "Content-Length: 1\r\nContent-Length: 2\r\n",
            "Content-Length: 1, 2\r\n",
            "Content-Length: -1\r\n",
            "Content-Length: +1\r\n",
            "Content-Length: 0x1\r\n",
            "Content-Length: 1,\r\n",
            "Content-Length: 99999999999999999999999999999\r\n",
            "Content-Length: 1\r\nTransfer-Encoding: chunked\r\n",
            "Transfer-Encoding: gzip, chunked\r\n",
            "Transfer-Encoding: chunked, chunked\r\n",
            "Transfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n",
            "Transfer-Encoding: chunked; extension=1\r\n",
            "Transfer-Encoding:\r\n",
        ] {
            let wire = format!("HTTP/1.1 200 OK\r\n{fields}\r\n1\r\nx\r\n0\r\n\r\n");
            assert!(parse(wire.as_bytes(), 20).is_err(), "accepted {fields:?}");
        }
        assert!(parse(
            b"HTTP/1.0 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
            20
        )
        .is_err());
        assert!(parse(
            b"HTTP/1.1 200 OK\r\nContent-Encoding: br\r\nContent-Length: 0\r\n\r\n",
            20
        )
        .unwrap_err()
        .contains("not implemented"));
    }

    #[test]
    fn chunk_extensions_trailers_and_truncation() {
        let prefix = "HTTP/1.1 200 OK\r\nTransfer-Encoding: Chunked\r\nTrailer: X-End\r\n\r\n";
        let wire = format!("{prefix}2;name=token;flag\r\nhe\r\n3 ; quoted = \"a\\\";b\"\r\nllo\r\n0\r\nX-End: ignored\r\n\r\n");
        let message = parse(wire.as_bytes(), 5).unwrap();
        assert_eq!(message.body, b"hello");
        assert!(header(&message.headers, "x-end").is_none());
        for body in [
            "2\r\nx",
            "1\r\nx\n0\r\n\r\n",
            "1\r\nx\r\n",
            "0\r\n",
            "0x1\r\nx\r\n0\r\n\r\n",
            "-1\r\nx\r\n0\r\n\r\n",
            "ffffffffffffffffffffffffffff\r\n",
            "1;=bad\r\nx\r\n0\r\n\r\n",
            "1;name=\r\nx\r\n0\r\n\r\n",
            "1;name=\"open\r\nx\r\n0\r\n\r\n",
            "0\r\nContent-Length: 1\r\n\r\n",
            "0\r\nLocation: /elsewhere\r\n\r\n",
        ] {
            assert!(
                parse(format!("{prefix}{body}").as_bytes(), 20).is_err(),
                "accepted {body:?}"
            );
        }
        assert!(
            parse(format!("{prefix}6\r\n123456\r\n0\r\n\r\n").as_bytes(), 5)
                .unwrap_err()
                .contains("limit")
        );
    }

    #[test]
    fn informational_empty_and_redirect_responses() {
        let message = parse(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 103 Early Hints\r\nLink: </x.css>\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx", 1).unwrap();
        assert_eq!(message.body, b"x");
        assert!(header(&message.headers, "link").is_none());
        for input in [
            b"HTTP/1.1 204 No Content\r\n\r\n".as_slice(),
            b"HTTP/1.1 304 Not Modified\r\nContent-Length: 9999\r\nContent-Encoding: gzip\r\n\r\n",
            b"HTTP/1.1 205 Reset Content\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 205 Reset Content\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
        ] {
            assert!(parse(input, 0).unwrap().body.is_empty());
        }
        for input in [
            b"HTTP/1.1 101 Switching Protocols\r\n\r\n".as_slice(),
            b"HTTP/1.1 103 Early Hints\r\nContent-Length: 0\r\n\r\nHTTP/1.1 204 No Content\r\n\r\n",
            b"HTTP/1.1 205 Reset Content\r\nContent-Length: 1\r\n\r\nx",
        ] {
            assert!(parse(input, 10).is_err());
        }
        let repeated = "HTTP/1.1 103 Hints\r\n\r\n".repeat(MAX_INFORMATIONAL + 1);
        assert!(parse(repeated.as_bytes(), 0)
            .unwrap_err()
            .contains("informational"));
        let redirect = read_response(
            Cursor::new(b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 99999999\r\nContent-Encoding: gzip\r\n\r\n"),
            1, &AtomicBool::new(false), Instant::now() + Duration::from_secs(2), true,
        ).unwrap();
        assert_eq!(redirect.status, 302);
        assert!(redirect.body.is_empty());
        assert!(parse(
            b"HTTP/1.1 302 Found\r\nLocation: /a\r\nLocation: /b\r\nContent-Length: 0\r\n\r\n",
            0
        )
        .is_err());
    }

    #[test]
    fn malformed_headers_and_resource_limits() {
        for wire in [
            "HTTP/1.1 200 OK\nContent-Length: 0\n\n",
            "HTTP/2.0 200 OK\r\n\r\n",
            "HTTP/1.1 099 Invalid\r\n\r\n",
            "HTTP/1.1 600 Invalid\r\n\r\n",
            "HTTP/1.1 200\r\n\r\n",
            "HTTP/1.1 200 OK\r\nFolded: first\r\n second\r\n\r\n",
            "HTTP/1.1 200 OK\r\nName : value\r\n\r\n",
            "HTTP/1.1 200 OK\r\n: value\r\n\r\n",
            "HTTP/1.1 200 OK\r\nName: val\0ue\r\n\r\n",
            "HTTP/1.1 200 OK\r\nName: val\rue\r\n\r\n",
        ] {
            assert!(parse(wire.as_bytes(), 20).is_err(), "accepted {wire:?}");
        }
        let line = format!(
            "HTTP/1.1 200 OK\r\nLarge: {}\r\n\r\n",
            "x".repeat(MAX_LINE_BYTES)
        );
        assert!(parse(line.as_bytes(), 0).unwrap_err().contains("line"));
        let many = format!(
            "HTTP/1.1 200 OK\r\n{}\r\n",
            "X: x\r\n".repeat(MAX_HEADERS + 1)
        );
        assert!(parse(many.as_bytes(), 0).unwrap_err().contains("count"));
        let large = format!(
            "HTTP/1.1 200 OK\r\n{}\r\n",
            format!("X: {}\r\n", "x".repeat(7000)).repeat(5)
        );
        assert!(parse(large.as_bytes(), 0).unwrap_err().contains("32 KiB"));
        let tiny_chunks = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{}0\r\n\r\n",
            "1\r\nx\r\n".repeat(MAX_CHUNKS)
        );
        assert!(parse(tiny_chunks.as_bytes(), MAX_CHUNKS)
            .unwrap_err()
            .contains("chunk count"));
        let extended = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{}0\r\n\r\n",
            format!("1;{}\r\nx\r\n", "a".repeat(8000)).repeat(66)
        );
        assert!(parse(extended.as_bytes(), 100)
            .unwrap_err()
            .contains("chunk metadata"));
    }

    #[test]
    fn content_codings_follow_encoded_framing_and_reverse_header_order() {
        // Independent Python gzip/zlib fixtures for the five bytes "hello".
        let gzip = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\x03\xcbH\xcd\xc9\xc9\x07\x00\x86\xa6\x10\x36\x05\x00\x00\x00";
        let deflate = b"\x78\x9c\xcbH\xcd\xc9\xc9\x07\x00\x06\x2c\x02\x15";
        let stacked = b"\x78\x9c\x93\xef\xe6\x60\x00\x01\x26\xe6\xd3\x1e\x67\x4f\x9e\x64\x67\x68\x5b\x26\x60\xc6\x0a\x14\x00\x00\x48\x01\x05\xa8";
        for (fields, encoded) in [
            ("Content-Encoding: gzip\r\n", gzip.as_slice()),
            ("Content-Encoding: X-GZip\r\n", gzip.as_slice()),
            ("Content-Encoding: deflate\r\n", deflate.as_slice()),
            ("Content-Encoding: gzip, deflate\r\n", stacked.as_slice()),
            (
                "Content-Encoding: , GZIP, identity,\r\nContent-Encoding: DEFLATE\r\n",
                stacked.as_slice(),
            ),
        ] {
            let mut wire = format!(
                "HTTP/1.1 200 OK\r\n{fields}Content-Length: {}\r\n\r\n",
                encoded.len()
            )
            .into_bytes();
            wire.extend_from_slice(encoded);
            let message = parse(&wire, 128).unwrap();
            assert_eq!(message.body, b"hello");
            assert_eq!(message.encoded_bytes, encoded.len());
            assert_eq!(
                header(&message.headers, "content-length").unwrap(),
                encoded.len().to_string()
            );
            // Chunk boundaries have no relationship to compression boundaries.
            let mut wire = format!("HTTP/1.1 200 OK\r\n{fields}Transfer-Encoding: chunked\r\n\r\n")
                .into_bytes();
            for part in encoded.chunks(3) {
                wire.extend_from_slice(format!("{:x}\r\n", part.len()).as_bytes());
                wire.extend_from_slice(part);
                wire.extend_from_slice(b"\r\n");
            }
            wire.extend_from_slice(b"0\r\n\r\n");
            assert_eq!(parse(&wire, 128).unwrap().body, b"hello");
        }
        for fields in [
            "Content-Encoding: deflate, gzip\r\n",
            "Content-Encoding: gzip\r\nContent-Encoding: br\r\n",
        ] {
            let mut wire = format!(
                "HTTP/1.1 200 OK\r\n{fields}Content-Length: {}\r\n\r\n",
                stacked.len()
            )
            .into_bytes();
            wire.extend_from_slice(stacked);
            assert!(parse(&wire, 128).is_err());
        }
        assert!(parse(
            b"HTTP/1.1 205 Reset Content\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n",
            0
        )
        .unwrap()
        .body
        .is_empty());
    }

    #[test]
    fn content_coding_metadata_and_expansion_have_independent_bounds() {
        for coding in [
            "br",
            "gzip;q=1",
            "\"gzip\"",
            "g zip",
            "identity,identity,identity,identity,gzip",
        ] {
            let headers = vec![("content-encoding".into(), coding.into())];
            assert!(content_codings(&headers).is_err(), "accepted {coding}");
        }
        let headers = vec![("content-encoding".into(), ",".repeat(MAX_EMPTY_CODINGS + 1))];
        assert!(content_codings(&headers).unwrap_err().contains("empty"));
        let headers = vec![("content-encoding".into(), " , identity, ,".into())];
        assert_eq!(
            content_codings(&headers).unwrap(),
            [ContentCoding::Identity]
        );
        // This 24-byte valid gzip expands to 256 bytes, exceeding a 64-byte
        // resource limit despite its small encoded Content-Length.
        let compressed = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\x03\xab\xa8\x18\xd9\x00\x00\x10\x1a\xee\x73\x00\x01\x00\x00";
        let mut wire = format!(
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
            compressed.len()
        )
        .into_bytes();
        wire.extend_from_slice(compressed);
        assert!(parse(&wire, 64).unwrap_err().contains("limit"));
        assert_eq!(parse(&wire, 256).unwrap().body, vec![b'x'; 256]);
        // The encoded limit remains independent of the decoded output limit.
        assert!(parse(&wire, compressed.len() - 1)
            .unwrap_err()
            .contains("body exceeds"));
    }

    #[test]
    fn cancellation_timeouts_and_request_injection() {
        let cancelled = AtomicBool::new(true);
        assert!(read_response(
            Cursor::new(b""),
            0,
            &cancelled,
            Instant::now() + Duration::from_secs(1),
            false
        )
        .unwrap_err()
        .contains("cancelled"));
        assert!(read_response(
            Cursor::new(b""),
            0,
            &AtomicBool::new(false),
            Instant::now(),
            false
        )
        .unwrap_err()
        .contains("timed out"));
        let (normal, _) = crate::url::parse("http://example.test:8080/a?q=1#fragment").unwrap();
        let wire = request(&normal, &Request::Get, FetchMode::Normal).unwrap();
        assert!(wire.starts_with("GET /a?q=1 HTTP/1.1\r\nHost: example.test:8080\r\n"));
        assert!(wire.contains("Accept-Encoding: gzip, deflate\r\n\r\n"));
        assert!(!wire.to_ascii_lowercase().contains("\r\nconnection:"));
        assert!(!wire.contains("fragment"));
        let mut invalid = normal.clone();
        invalid.host = "example.test\r\nInjected: yes".into();
        assert!(request(&invalid, &Request::Get, FetchMode::Normal).is_err());
        invalid = normal.clone();
        invalid.path = "/\r\nInjected: yes".into();
        assert!(request(&invalid, &Request::Get, FetchMode::Normal).is_err());
        invalid = normal.clone();
        invalid.query = Some("x\nInjected: yes".into());
        assert!(request(&invalid, &Request::Get, FetchMode::Normal).is_err());
        invalid = normal.clone();
        invalid.secure = true;
        let trust = trust::Store::unconfigured();
        assert!(fetch(
            &invalid,
            0,
            None,
            &AtomicBool::new(false),
            &trust,
            FetchMode::Normal
        )
        .unwrap_err()
        .contains("HTTPS"));
        invalid = normal.clone();
        invalid.port += 1;
        assert!(fetch(
            &normal,
            0,
            Some(&invalid),
            &AtomicBool::new(false),
            &trust,
            FetchMode::Normal
        )
        .unwrap_err()
        .contains("Cross-origin"));
        assert!(fetch(
            &normal,
            MAX_BODY_BYTES + 1,
            None,
            &AtomicBool::new(false),
            &trust,
            FetchMode::Normal
        )
        .unwrap_err()
        .contains("32 MiB"));
    }

    struct PostSite {
        url: HttpUrl,
        requests: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
        stop: std::sync::Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl PostSite {
        fn new() -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let recorded = requests.clone();
            let stop = std::sync::Arc::new(AtomicBool::new(false));
            let stopping = stop.clone();
            let thread = std::thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            // Accepted Winsock sockets inherit the polling listener mode.
                            stream.set_nonblocking(false).unwrap();
                            stream
                                .set_read_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            let mut wire = Vec::new();
                            let mut byte = [0u8; 1];
                            while !wire.ends_with(b"\r\n\r\n") && wire.len() < 32 * 1024 {
                                if stream.read(&mut byte).unwrap_or(0) == 0 {
                                    break;
                                }
                                wire.push(byte[0]);
                            }
                            if !wire.ends_with(b"\r\n\r\n") {
                                continue;
                            }
                            let headers = String::from_utf8(wire.clone()).unwrap();
                            let method = headers.split_whitespace().next().unwrap();
                            let path = headers.split_whitespace().nth(1).unwrap();
                            let length: usize = headers
                                .lines()
                                .find_map(|line| line.strip_prefix("Content-Length: "))
                                .map(|text| text.parse().unwrap())
                                .unwrap_or(0);
                            assert!(length <= MAX_FORM_BYTES);
                            let mut body = vec![0u8; length];
                            stream.read_exact(&mut body).unwrap();
                            wire.extend_from_slice(&body);
                            recorded.lock().unwrap().push(wire);
                            let (status, location) =
                                if let Some(status) = path.strip_prefix("/redirect/") {
                                    (
                                        status.parse::<u16>().unwrap(),
                                        Some("/done#result".to_owned()),
                                    )
                                } else if let Some(status) = path.strip_prefix("/cross/") {
                                    (
                                        status.parse::<u16>().unwrap(),
                                        Some(format!("http://localhost:{port}/done")),
                                    )
                                } else if path == "/cookie-away" {
                                    (302, Some(format!("http://localhost:{port}/cookie-return")))
                                } else if path == "/cookie-return" {
                                    (302, Some(format!("http://127.0.0.1:{port}/done")))
                                } else if path == "/same" && method == "POST" {
                                    (303, Some("/same".to_owned()))
                                } else if path == "/loop" {
                                    (307, Some("/loop".to_owned()))
                                } else if path == "/drop" {
                                    continue;
                                } else if path == "/invalid-redirect" {
                                    (307, Some("file:///local-file".to_owned()))
                                } else {
                                    (200, None)
                                };
                            let location = location
                                .map(|value| format!("Location: {value}\r\n"))
                                .unwrap_or_default();
                            let response = format!("HTTP/1.1 {status} Test\r\n{location}Content-Type: text/plain\r\nContent-Length: 8\r\nConnection: close\r\n\r\nreceived");
                            let _ = stream.write_all(response.as_bytes());
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2))
                        }
                        Err(error) => panic!("Loopback accept: {error}"),
                    }
                }
            });
            Self {
                url: crate::url::parse(&format!("http://127.0.0.1:{port}/"))
                    .unwrap()
                    .0,
                requests,
                stop,
                thread: Some(thread),
            }
        }

        fn post(&self, path: &str, mode: FetchMode) -> Result<Response, String> {
            let (url, _) = self.url.resolve(path).unwrap();
            fetch_request(
                &url,
                &Request::FormPost {
                    body: b"message=private%0D%0Avalue&choice=%E2%82%AC&choice=two".to_vec(),
                    origin: Some(self.url.clone()),
                },
                1024,
                None,
                &AtomicBool::new(false),
                &trust::Store::unconfigured(),
                mode,
            )
        }

        fn take(&self) -> Vec<Vec<u8>> {
            std::mem::take(&mut *self.requests.lock().unwrap())
        }
    }

    impl Drop for PostSite {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
            if let Err(failure) = thread.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(failure);
                }
            }
        }
        }
    }

    #[test]
    fn document_policy_guards_redirects_without_restricting_later_resources() {
        let site = PostSite::new();
        let cancel = AtomicBool::new(false);
        let trust = trust::Store::unconfigured();
        let filter = |target: &HttpUrl| {
            if target.path == "/done" {
                Err("form-action fixture refusal".into())
            } else {
                Ok(())
            }
        };
        let pool = Pool::new(&cancel, &trust).with_document_filter(&filter);
        let target = site.url.resolve("/redirect/303").unwrap().0;
        let error = pool
            .fetch_document(
                &target,
                &Request::FormPost {
                    body: b"q=kept".to_vec(),
                    origin: Some(site.url.clone()),
                },
                FetchMode::Normal,
            )
            .unwrap_err();
        assert!(error.contains("form-action fixture"), "{error}");
        assert_eq!(site.requests.lock().unwrap().len(), 1);
        let done = site.url.resolve("/done").unwrap().0;
        assert_eq!(
            pool.fetch_kind(&done, 1024, Some(&site.url), FetchMode::Normal, "CSS")
                .unwrap()
                .status,
            200
        );
        assert_eq!(site.requests.lock().unwrap().len(), 2);
    }

    #[test]
    fn cookie_site_equivalence_does_not_grant_cross_origin_post_authority() {
        let source = crate::url::parse("https://www.example.com/form").unwrap().0;
        let sibling = crate::url::parse("https://consent.example.com/submit").unwrap().0;
        assert!(crate::cookies::same_site(&source, &sibling));
        assert!(!source.same_origin(&sibling));
        let mut post = Request::FormPost {
            body: b"private=value".to_vec(),
            origin: Some(source.clone()),
        };
        assert!(post.redirect(307, &source, &sibling).is_err());
        assert!(post.is_post());
        assert_eq!(post.body(), b"private=value");
        // A 303 still discards the body; site equivalence does not retain it.
        post.redirect(303, &source, &sibling).unwrap();
        assert!(!post.is_post());
        assert!(post.body().is_empty());
    }

    #[test]
    fn document_cookie_context_survives_redirects_and_ui_reload() {
        let site = PostSite::new();
        let cancelled = AtomicBool::new(false);
        let trust = trust::Store::unconfigured();
        let cookies = Arc::new(crate::cookies::Session::default());
        cookies
            .set_http(&site.url, "strict=one; Path=/; SameSite=Strict")
            .unwrap();
        cookies
            .set_http(&site.url, "lax=two; Path=/; SameSite=Lax")
            .unwrap();
        let cookie_fields = || {
            site.take()
                .into_iter()
                .map(|wire| {
                    String::from_utf8(wire)
                        .unwrap()
                        .lines()
                        .find_map(|line| line.strip_prefix("Cookie: "))
                        .unwrap_or("")
                        .to_owned()
                })
                .collect::<Vec<_>>()
        };
        let pool = Pool::new(&cancelled, &trust)
            .with_cookie_session(cookies.clone(), Some(site.url.clone()));
        let response = pool
            .fetch_document(&site.url, &Request::Get, FetchMode::Normal)
            .unwrap();
        assert!(response.cookie_same_site);
        assert_eq!(cookie_fields(), ["strict=one; lax=two"]);

        // The redirect leaves the initiating host, then returns. Its final
        // request remains cross-site even though its destination matches.
        let (away, _) = site.url.resolve("/cookie-away").unwrap();
        let response = pool
            .fetch_document(&away, &Request::Get, FetchMode::Normal)
            .unwrap();
        assert_eq!(response.url.host, site.url.host);
        assert!(!response.cookie_same_site);
        assert_eq!(cookie_fields(), ["strict=one; lax=two", "", "lax=two"]);

        // A user reload preserves that context: safe navigation can send
        // Lax cookies, while a reissued unsafe POST cannot.
        let pool = Pool::new(&cancelled, &trust)
            .with_cookie_session(cookies.clone(), Some(site.url.clone()))
            .with_cookie_context(Some(response.cookie_same_site));
        assert!(
            !pool
                .fetch_document(&site.url, &Request::Get, FetchMode::FullReload)
                .unwrap()
                .cookie_same_site
        );
        assert_eq!(cookie_fields(), ["lax=two"]);
        assert!(
            !pool
                .fetch_document(
                    &site.url,
                    &Request::FormPost {
                        body: b"x=one".to_vec(),
                        origin: Some(site.url.clone()),
                    },
                    FetchMode::Normal,
                )
                .unwrap()
                .cookie_same_site
        );
        assert_eq!(cookie_fields(), [""]);

        // Resources use the checked document origin independently of the
        // context of the document's original top-level request.
        assert!(
            pool.fetch_kind(
                &site.url,
                1024,
                Some(&site.url),
                FetchMode::Normal,
                "SCRIPT",
            )
            .unwrap()
            .cookie_same_site
        );
        assert_eq!(cookie_fields(), ["strict=one; lax=two"]);

        // A true override grants nothing to a mismatching initiator. Scheme
        // participates in this comparison; a differing port does not.
        let mut origin = site.url.clone();
        origin.secure = true;
        let pool = Pool::new(&cancelled, &trust)
            .with_cookie_session(cookies.clone(), Some(origin))
            .with_cookie_context(Some(true));
        assert!(
            !pool
                .fetch_document(&site.url, &Request::Get, FetchMode::Normal)
                .unwrap()
                .cookie_same_site
        );
        assert_eq!(cookie_fields(), ["lax=two"]);
        let mut origin = site.url.clone();
        origin.port = if origin.port == 80 { 81 } else { 80 };
        let pool = Pool::new(&cancelled, &trust).with_cookie_session(cookies, Some(origin));
        assert!(
            pool.fetch_document(&site.url, &Request::Get, FetchMode::Normal)
                .unwrap()
                .cookie_same_site
        );
        assert_eq!(cookie_fields(), ["strict=one; lax=two"]);

        // The returned context describes the request, independently of
        // whether the current jar has any cookie to serialize.
        let pool = Pool::new(&cancelled, &trust).with_cookie_session(
            Arc::new(crate::cookies::Session::default()),
            Some(site.url.clone()),
        );
        assert!(
            pool.fetch_document(&site.url, &Request::Get, FetchMode::Normal)
                .unwrap()
                .cookie_same_site
        );
        assert_eq!(cookie_fields(), [""]);
        assert!(
            !Pool::new(&cancelled, &trust)
                .with_cookie_context(Some(true))
                .fetch_document(&site.url, &Request::Get, FetchMode::Normal)
                .unwrap()
                .cookie_same_site
        );
        assert_eq!(cookie_fields(), [""]);
    }

    #[test]
    fn failed_tls_handshake_retains_phase_timings_and_redacts_query_values() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut header = [0; 5];
            stream.read_exact(&mut header).unwrap();
            let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
            assert!(length <= 16384);
            let mut hello = vec![0; length];
            stream.read_exact(&mut hello).unwrap();
            // Independent TLS fatal handshake_failure alert after ClientHello.
            stream.write_all(&[21, 3, 3, 0, 2, 2, 40]).unwrap();
        });
        let (url, _) = crate::url::parse(&format!(
            "https://localhost:{port}/timing?secret=not-to-log"
        ))
        .unwrap();
        let trust = trust::Store::from_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/https-demo/demo-roots.pem"),
        )
        .unwrap();
        let error = fetch(
            &url,
            1024,
            None,
            &AtomicBool::new(false),
            &trust,
            FetchMode::Normal,
        )
        .unwrap_err();
        peer.join().unwrap();
        assert!(error.contains("Connection timing: DNS lookup="));
        assert!(error.contains("outcome=resolved"));
        assert!(error.contains("Connection timing: TCP connect="));
        assert!(error.contains("outcome=connected"));
        assert!(error.contains("Connection timing: TLS handshake="));
        assert!(error.contains("outcome=failed"));
        assert!(error.contains("[query omitted]"));
        assert!(!error.contains("not-to-log") && !error.contains("127.0.0.1"));
    }

    #[test]
    fn post_headers_are_bounded_and_origins_never_include_source_paths() {
        let url = crate::url::parse("https://destination.test/submit?keep=yes")
            .unwrap()
            .0;
        let origin = crate::url::parse("https://source.test:8443/private/path?secret=yes")
            .unwrap()
            .0;
        let mut post = Request::FormPost {
            body: b"message=one+two".to_vec(),
            origin: Some(origin.clone()),
        };
        let wire = request(&url, &post, FetchMode::FullReload).unwrap();
        assert!(wire.starts_with("POST /submit?keep=yes HTTP/1.1\r\n"));
        assert!(wire.contains("Content-Length: 15\r\n"));
        assert!(wire.contains("Content-Type: application/x-www-form-urlencoded\r\n"));
        assert!(wire.contains("Origin: https://source.test:8443\r\n"));
        assert!(wire.contains("Cache-Control: no-cache\r\nPragma: no-cache\r\n"));
        assert!(
            !wire.contains("private/path")
                && !wire.contains("secret=yes")
                && !wire.contains("message=")
        );
        if let Request::FormPost { origin: source, .. } = &mut post {
            *source = None;
        }
        assert!(request(&url, &post, FetchMode::Normal)
            .unwrap()
            .contains("Origin: null\r\n"));
        let mut invalid_origin = origin;
        invalid_origin.host = "valid.test\r\nInjected: value".into();
        if let Request::FormPost { origin: source, .. } = &mut post {
            *source = Some(invalid_origin);
        }
        assert!(request(&url, &post, FetchMode::Normal).is_err());
        let full = Request::FormPost {
            body: vec![b'x'; MAX_FORM_BYTES],
            origin: None,
        };
        assert!(request(&url, &full, FetchMode::Normal)
            .unwrap()
            .contains("Content-Length: 131072\r\n"));
        let too_large = Request::FormPost {
            body: vec![b'x'; MAX_FORM_BYTES + 1],
            origin: None,
        };
        assert!(request(&url, &too_large, FetchMode::Normal)
            .unwrap_err()
            .contains("128 KiB"));
        assert!(fetch_request(
            &url,
            &too_large,
            1,
            None,
            &AtomicBool::new(false),
            &trust::Store::unconfigured(),
            FetchMode::Normal
        )
        .unwrap_err()
        .contains("128 KiB"));
        let insecure = crate::url::parse("http://destination.test/submit")
            .unwrap()
            .0;
        for status in [301, 302, 303, 307, 308] {
            let mut request = full.clone();
            assert!(request
                .redirect(status, &url, &insecure)
                .unwrap_err()
                .contains("unencrypted"));
            assert!(request.is_post());
        }
    }

    #[test]
    fn live_post_redirects_keep_or_discard_payload_and_content_headers() {
        let site = PostSite::new();
        for status in [301, 302, 303, 307, 308] {
            let response = site
                .post(&format!("/redirect/{status}"), FetchMode::FullReload)
                .unwrap();
            assert_eq!(response.body, b"received");
            assert_eq!(response.fragment.as_deref(), Some("result"));
            assert_eq!(response.request_is_post, matches!(status, 307 | 308));
            let wires = site.take();
            assert_eq!(wires.len(), 2);
            let first = String::from_utf8(wires[0].clone()).unwrap();
            let second = String::from_utf8(wires[1].clone()).unwrap();
            let body = first.split_once("\r\n\r\n").unwrap().1;
            assert_eq!(
                body,
                "message=private%0D%0Avalue&choice=%E2%82%AC&choice=two"
            );
            assert!(first.starts_with(&format!("POST /redirect/{status} HTTP/1.1\r\n")));
            assert!(first.contains(&format!("Content-Length: {}\r\n", body.len())));
            for wire in [&first, &second] {
                assert!(wire.contains("Cache-Control: no-cache\r\nPragma: no-cache\r\n"));
                assert!(!wire.contains("Transfer-Encoding:"));
            }
            if matches!(status, 307 | 308) {
                assert!(second.starts_with("POST /done HTTP/1.1\r\n"));
                assert_eq!(second.split_once("\r\n\r\n").unwrap().1, body);
                assert!(second.contains("Origin: http://127.0.0.1:"));
            } else {
                assert!(second.starts_with("GET /done HTTP/1.1\r\n"));
                assert!(second.ends_with("\r\n\r\n"));
                for omitted in ["Content-Length:", "Content-Type:", "Origin:", "message="] {
                    assert!(!second.contains(omitted));
                }
            }
            let events = response.events.join("\n");
            assert!(events.contains("FORM BODY:"));
            assert!(!events.contains("private") && !events.contains("choice="));
            let timings: Vec<_> = response
                .events
                .iter()
                .filter(|event| event.starts_with("Connection timing:"))
                .collect();
            // Both HTTP exchanges retain their own bounded phase summaries.
            assert_eq!(timings.len(), 4);
            assert!(timings
                .iter()
                .all(|event| { !event.contains("127.0.0.1") && !event.contains("message=") }));
        }
        let same = site.post("/same", FetchMode::Normal).unwrap();
        assert!(!same.request_is_post);
        let wires = site.take();
        assert_eq!(wires.len(), 2);
        assert!(wires[0].starts_with(b"POST /same ") && wires[1].starts_with(b"GET /same "));
        for wire in wires {
            assert!(!String::from_utf8(wire).unwrap().contains("Cache-Control:"));
        }
        let direct = site.post("/done?keep=yes", FetchMode::Normal).unwrap();
        assert!(direct.request_is_post);
        assert!(site.take()[0].starts_with(b"POST /done?keep=yes HTTP/1.1\r\n"));
    }

    #[test]
    fn live_post_blocks_cross_origin_forwarding_loops_retries_and_cancelled_requests() {
        let site = PostSite::new();
        for status in [307, 308] {
            let error = site
                .post(&format!("/cross/{status}"), FetchMode::Normal)
                .unwrap_err();
            assert!(error.contains("Cross-origin") && error.contains("not forwarded"));
            assert_eq!(site.take().len(), 1);
        }
        let response = site.post("/cross/303", FetchMode::Normal).unwrap();
        assert!(!response.request_is_post);
        let wires = site.take();
        assert_eq!(wires.len(), 2);
        assert!(wires[1].starts_with(b"GET /done "));
        assert!(!String::from_utf8(wires[1].clone())
            .unwrap()
            .contains("message="));
        assert!(site
            .post("/loop", FetchMode::Normal)
            .unwrap_err()
            .contains("loop"));
        assert_eq!(site.take().len(), 1);
        let failed_read = site.post("/drop", FetchMode::Normal).unwrap_err();
        assert!(failed_read.contains("Connection timing: DNS lookup="));
        assert!(failed_read.contains("Connection timing: TCP connect="));
        assert!(failed_read.contains("attempts=1; outcome=connected"));
        assert_eq!(site.take().len(), 1);
        assert!(site
            .post("/invalid-redirect", FetchMode::Normal)
            .unwrap_err()
            .contains("http://"));
        assert_eq!(site.take().len(), 1);
        let request = Request::FormPost {
            body: b"x=secret".to_vec(),
            origin: None,
        };
        assert!(fetch_request(
            &site.url,
            &request,
            1,
            None,
            &AtomicBool::new(true),
            &trust::Store::unconfigured(),
            FetchMode::Normal
        )
        .unwrap_err()
        .contains("cancelled"));
        let mut secure = site.url.clone();
        secure.secure = true;
        assert!(fetch_request(
            &secure,
            &request,
            1,
            None,
            &AtomicBool::new(false),
            &trust::Store::unconfigured(),
            FetchMode::Normal
        )
        .unwrap_err()
        .contains("HTTPS"));
        assert!(site.take().is_empty());
    }

    #[test]
    fn crossorigin_classic_mode_does_not_confuse_same_site_with_same_origin() {
        let url = |s: &str| crate::url::parse(s).unwrap().0;
        let origin = url("https://www.example.test/");
        assert!(check_classic_script_mode(&origin, &url("https://www.example.test/a.js"), true).is_ok());
        for target in ["https://cdn.example.test/a.js", "https://www.example.test:444/a.js",
                       "http://www.example.test/a.js", "https://other.test/a.js"] {
            assert!(check_classic_script_mode(&origin, &url(target), true).is_err());
        }
        assert!(check_classic_script_mode(&origin, &url("https://cdn.example.test/a.js"), false).is_ok());
    }

}
