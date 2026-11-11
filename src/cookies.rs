//! Bounded, memory-only session cookies written for ChezCat.
//!
//! RFC 6265 and the HTTP working group's 6265bis draft define the cookie,
//! expiry, path, prefix, and SameSite rules used here:
//! https://www.rfc-editor.org/rfc/rfc6265
//! https://httpwg.org/http-extensions/draft-ietf-httpbis-rfc6265bis.html
//!
//! This is an explicit conservative subset: ASCII cookie syntax, no nameless
//! cookies, no persistence, and exact-host storage even when a matching Domain
//! attribute is supplied. Domain cookies never reach a parent or child host;
//! this does not require a public-suffix list. The caller supplies a scheme and
//! exact-host SameSite context. Unspecified/invalid SameSite means Lax, without
//! a temporary unsafe-method exception. Only HTTPS is considered secure.

use crate::url::HttpUrl;
use std::cmp::Reverse;
use std::fmt;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SET_BYTES: usize = 4096;
const MAX_COOKIES: usize = 256;
const MAX_HOST_COOKIES: usize = 50;
const MAX_STORED_BYTES: usize = 64 * 1024;
// Leave room for "Cookie: " and CRLF inside an 8 KiB HTTP header line.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 - 10;
const MAX_AGE_SECONDS: i64 = 400 * 24 * 60 * 60;

/// Share one session with `Arc<Session>` across document and HTTP workers.
/// Debug output contains counts only; it never includes cookie names or values.
#[derive(Default)]
pub struct Session {
    store: Mutex<Store>,
}

#[derive(Default)]
struct Store {
    cookies: Vec<Cookie>,
    next_creation: u64,
}

struct Cookie {
    host: String,
    path: String,
    name: String,
    value: String,
    secure: bool,
    http_only: bool,
    same_site: SameSite,
    expires: Option<i64>,
    creation: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SameSite {
    Lax,
    Strict,
    None,
}

impl Cookie {
    fn bytes(&self) -> usize {
        self.host.len() + self.path.len() + self.name.len() + self.value.len() + 4
    }

    fn expired(&self, now: i64) -> bool {
        self.expires.is_some_and(|expiry| expiry <= now)
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Ok(store) = self.store.lock() else {
            return f.write_str("Session { unavailable }");
        };
        f.debug_struct("Session")
            .field("cookies", &store.cookies.len())
            .field(
                "stored_bytes",
                &store.cookies.iter().map(Cookie::bytes).sum::<usize>(),
            )
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Process one Set-Cookie field value, not a comma-joined header list.
    /// Rejected input leaves existing live cookies intact; errors contain no
    /// input data. Expired cookies are discarded before capacity checks.
    /// The caller must restrict this to top-level responses or same-origin
    /// resources, including redirect hops; this API has no cross-site resource
    /// context with which to validate SameSite acceptance.
    pub fn set_http(&self, url: &HttpUrl, header: &str) -> Result<(), String> {
        self.set_at(url, header, true, now_seconds())
    }

    /// Process a document.cookie assignment. Invalid/protected writes return
    /// a generic error so the DOM setter can ignore them without exposing data.
    pub fn set_document(&self, url: &HttpUrl, text: &str) -> Result<(), String> {
        self.set_at(url, text, false, now_seconds())
    }

    /// Return a Cookie field value. `same_site` must use the initiating
    /// document's scheme and exact host; `top_level_safe` is only for safe
    /// top-level navigation, never an arbitrary subresource or POST.
    pub fn request_header(&self, url: &HttpUrl, same_site: bool, top_level_safe: bool) -> String {
        self.get_at(url, false, same_site, top_level_safe, now_seconds())
    }

    /// Return the visible document.cookie string for a top-level document.
    /// HttpOnly cookies are excluded, including from HTTPS documents.
    pub fn document(&self, url: &HttpUrl) -> String {
        self.get_at(url, true, true, false, now_seconds())
    }

    fn set_at(&self, url: &HttpUrl, text: &str, http: bool, now: i64) -> Result<(), String> {
        let mut cookie = parse_cookie(url, text, http, now)?;
        let mut store = self
            .store
            .lock()
            .map_err(|_| "Session cookie store is unavailable.".to_owned())?;
        store.cookies.retain(|old| !old.expired(now));

        // A non-secure response cannot overlay a Secure cookie at its path or
        // below it. The asymmetric path rule also prevents a longer-path cookie
        // from taking precedence over the protected cookie on the wire.
        if !url.secure
            && store.cookies.iter().any(|old| {
                old.secure
                    && old.host == cookie.host
                    && old.name == cookie.name
                    && path_matches(&cookie.path, &old.path)
            })
        {
            return Err("An insecure origin cannot replace a Secure cookie.".into());
        }
        let previous = store.cookies.iter().position(|old| {
            old.host == cookie.host && old.path == cookie.path && old.name == cookie.name
        });
        if !http && previous.is_some_and(|index| store.cookies[index].http_only) {
            return Err("Document scripts cannot replace an HttpOnly cookie.".into());
        }
        // Protection checks also apply to deletion attempts.
        if cookie.expired(now) {
            if let Some(index) = previous {
                store.cookies.remove(index);
            }
            return Ok(());
        }
        if previous.is_none()
            && (store.cookies.len() >= MAX_COOKIES
                || store
                    .cookies
                    .iter()
                    .filter(|old| old.host == cookie.host)
                    .count()
                    >= MAX_HOST_COOKIES)
        {
            return Err("Session cookie count limit reached.".into());
        }
        let bytes: usize = store.cookies.iter().map(Cookie::bytes).sum();
        let replaced_bytes = previous.map_or(0, |index| store.cookies[index].bytes());
        if bytes - replaced_bytes + cookie.bytes() > MAX_STORED_BYTES {
            return Err("Session cookie storage limit reached.".into());
        }
        if let Some(index) = previous {
            cookie.creation = store.cookies[index].creation;
            store.cookies[index] = cookie;
        } else {
            cookie.creation = store.next_creation;
            store.next_creation = store
                .next_creation
                .checked_add(1)
                .ok_or("Session cookie sequence limit reached.")?;
            store.cookies.push(cookie);
        }
        Ok(())
    }

    fn get_at(
        &self,
        url: &HttpUrl,
        document: bool,
        same_site: bool,
        top_level_safe: bool,
        now: i64,
    ) -> String {
        let Ok(mut store) = self.store.lock() else {
            return String::new();
        };
        store.cookies.retain(|cookie| !cookie.expired(now));
        let mut selected: Vec<_> = store
            .cookies
            .iter()
            .filter(|cookie| {
                cookie.host.eq_ignore_ascii_case(&url.host)
                    && path_matches(&url.path, &cookie.path)
                    && (!cookie.secure || url.secure)
                    && (!document || !cookie.http_only)
                    && (same_site
                        || cookie.same_site == SameSite::None
                        || cookie.same_site == SameSite::Lax && top_level_safe)
            })
            .collect();
        selected.sort_unstable_by_key(|cookie| (Reverse(cookie.path.len()), cookie.creation));
        let mut output = String::new();
        for cookie in selected {
            let separator = usize::from(!output.is_empty()) * 2;
            let added = separator + cookie.name.len() + 1 + cookie.value.len();
            if added > MAX_OUTPUT_BYTES - output.len() {
                continue;
            }
            if separator != 0 {
                output.push_str("; ");
            }
            output.push_str(&cookie.name);
            output.push('=');
            output.push_str(&cookie.value);
        }
        output
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn trim(value: &str) -> &str {
    value.trim_matches([' ', '\t'])
}

fn parse_cookie(url: &HttpUrl, text: &str, http: bool, now: i64) -> Result<Cookie, String> {
    if text.len() > MAX_SET_BYTES {
        return Err("A cookie assignment is limited to 4096 bytes.".into());
    }
    if !text.is_ascii()
        || text
            .bytes()
            .any(|byte| byte == 127 || byte < 32 && byte != b'\t')
    {
        return Err("Cookie input must use ASCII without control characters.".into());
    }
    if !valid_host(&url.host) || url.path.len() > 16 * 1024 {
        return Err("Cookie origin is outside the supported URL subset.".into());
    }
    let mut fields = text.split(';');
    let (name, value) = fields
        .next()
        .and_then(|pair| pair.split_once('='))
        .ok_or("A cookie needs a name and value separated by '='.")?;
    let (name, value) = (trim(name), trim(value));
    if name.is_empty() || !name.bytes().all(cookie_name_byte) || !valid_value(value) {
        return Err("Cookie name or value uses unsupported syntax.".into());
    }
    let mut domain = None;
    let default = default_path(&url.path);
    let mut path = default;
    let mut explicit_root_path = false;
    let mut secure = false;
    let mut http_only = false;
    let mut same_site = SameSite::Lax;
    let mut expires = None;
    let mut max_age = None;
    for field in fields {
        let (attribute, value) = field.split_once('=').unwrap_or((field, ""));
        let (attribute, value) = (trim(attribute), trim(value));
        if attribute.eq_ignore_ascii_case("domain") {
            domain = Some(value);
        } else if attribute.eq_ignore_ascii_case("path") && value.len() <= 1024 {
            path = if value.starts_with('/') {
                value
            } else {
                default
            };
            explicit_root_path = value == "/";
        } else if attribute.eq_ignore_ascii_case("secure") {
            secure = true;
        } else if attribute.eq_ignore_ascii_case("httponly") {
            http_only = true;
        } else if attribute.eq_ignore_ascii_case("samesite") {
            same_site = if value.eq_ignore_ascii_case("strict") {
                SameSite::Strict
            } else if value.eq_ignore_ascii_case("none") {
                SameSite::None
            } else {
                SameSite::Lax
            };
        } else if attribute.eq_ignore_ascii_case("expires") {
            if let Some(parsed) = cookie_date(value) {
                expires = Some(parsed.min(now.saturating_add(MAX_AGE_SECONDS)));
            }
        } else if attribute.eq_ignore_ascii_case("max-age") {
            if let Some(parsed) = age_seconds(value) {
                max_age = Some(now.saturating_add(parsed));
            }
        }
    }
    let host = url.host.to_ascii_lowercase();
    if let Some(domain) = domain {
        let domain = domain
            .strip_prefix('.')
            .unwrap_or(domain)
            .to_ascii_lowercase();
        if !valid_host(&domain)
            || !(host == domain
                || host.parse::<IpAddr>().is_err()
                    && host
                        .strip_suffix(&domain)
                        .is_some_and(|prefix| prefix.ends_with('.')))
        {
            return Err("Cookie Domain does not match its originating host.".into());
        }
    }
    if secure && !url.secure {
        return Err("Secure cookies require an HTTPS origin.".into());
    }
    if http_only && !http {
        return Err("Document scripts cannot set HttpOnly cookies.".into());
    }
    if same_site == SameSite::None && !secure {
        return Err("SameSite=None cookies require Secure.".into());
    }
    // Cookie identities remain case-sensitive. Prefix checks intentionally use
    // the current draft's stricter case-insensitive user-agent protection.
    let secure_prefix = name
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("__Secure-"));
    let host_prefix = name
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("__Host-"));
    if (secure_prefix || host_prefix) && !secure {
        return Err("Protected cookie prefixes require Secure.".into());
    }
    if host_prefix && (domain.is_some() || !explicit_root_path) {
        return Err("Host-prefixed cookies require explicit Path=/ and no Domain.".into());
    }
    Ok(Cookie {
        host,
        path: path.to_owned(),
        name: name.to_owned(),
        value: value.to_owned(),
        secure,
        http_only,
        same_site,
        expires: max_age.or(expires),
        creation: 0,
    })
}

fn cookie_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn valid_value(value: &str) -> bool {
    let value = if value.starts_with('"') {
        let Some(value) = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
        else {
            return false;
        };
        value
    } else {
        value
    };
    value
        .bytes()
        .all(|byte| matches!(byte, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && (host.parse::<IpAddr>().is_ok()
            || host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            }))
}

fn default_path(path: &str) -> &str {
    if !path.starts_with('/') {
        return "/";
    }
    match path.rfind('/') {
        Some(index) if index > 0 => &path[..index],
        _ => "/",
    }
}

fn path_matches(request: &str, cookie: &str) -> bool {
    request == cookie
        || request.starts_with(cookie)
            && (cookie.ends_with('/') || request.as_bytes().get(cookie.len()) == Some(&b'/'))
}

fn age_seconds(value: &str) -> Option<i64> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if value.starts_with('-') {
        return Some(0);
    }
    Some(digits.bytes().fold(0, |total: i64, byte| {
        (total * 10 + i64::from(byte - b'0')).min(MAX_AGE_SECONDS)
    }))
}

fn number_prefix(token: &str, min: usize, max: usize) -> Option<(u32, &str)> {
    let length = token.bytes().take_while(u8::is_ascii_digit).count();
    if length < min || length > max {
        return None;
    }
    let number = token.as_bytes()[..length]
        .iter()
        .fold(0, |number, byte| number * 10 + u32::from(byte - b'0'));
    Some((number, &token[length..]))
}

fn time_token(token: &str) -> Option<(u32, u32, u32)> {
    let (hour, rest) = number_prefix(token, 1, 2)?;
    let (minute, rest) = number_prefix(rest.strip_prefix(':')?, 1, 2)?;
    let (second, _) = number_prefix(rest.strip_prefix(':')?, 1, 2)?;
    Some((hour, minute, second))
}

fn month_token(token: &str) -> Option<u32> {
    let month = token.get(..3)?;
    if token.as_bytes().get(3).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ]
    .iter()
    .position(|name| name.eq_ignore_ascii_case(month))
    .map(|index| index as u32 + 1)
}

/// Scan the date's bounded tokens independently of weekday and formatting.
/// UTC conversion uses Gregorian year/month counts, not an OS date parser.
fn cookie_date(value: &str) -> Option<i64> {
    if !value.is_ascii() {
        return None;
    }
    let (mut time, mut day, mut month, mut year) = (None, None, None, None);
    for token in value.split(|ch: char| {
        matches!(ch as u32, 0x09 | 0x20..=0x2f | 0x3b..=0x40 | 0x5b..=0x60 | 0x7b..=0x7e)
    }) {
        if time.is_none() {
            if let Some(found) = time_token(token) {
                time = Some(found);
                continue;
            }
        }
        if day.is_none() {
            if let Some((found, _)) = number_prefix(token, 1, 2) {
                day = Some(found);
                continue;
            }
        }
        if month.is_none() {
            if let Some(found) = month_token(token) {
                month = Some(found);
                continue;
            }
        }
        if year.is_none() {
            if let Some((found, _)) = number_prefix(token, 2, 4) {
                year = Some(found);
            }
        }
    }
    let (hour, minute, second) = time?;
    let (day, month, mut year) = (day?, month?, year?);
    year += match year {
        0..=69 => 2000,
        70..=99 => 1900,
        _ => 0,
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = [
        31,
        28 + u32::from(leap),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if year < 1601
        || day == 0
        || day > days_in_month[month as usize - 1]
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let leap_days = |year: u32| i64::from(year / 4 - year / 100 + year / 400);
    let before_month = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let days = 365 * (i64::from(year) - 1970) + leap_days(year - 1) - leap_days(1969)
        + before_month[month as usize - 1]
        + i64::from(leap && month > 2)
        + i64::from(day - 1);
    Some(days * 86400 + i64::from(hour * 3600 + minute * 60 + second))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const NOW: i64 = 1_700_000_000;

    fn url(text: &str) -> HttpUrl {
        crate::url::parse(text).unwrap().0
    }

    fn request(session: &Session, target: &HttpUrl) -> String {
        session.get_at(target, false, true, false, NOW)
    }

    #[test]
    fn http_and_document_views_preserve_path_and_creation_order() {
        let session = Session::default();
        let target = url("https://example.test/a/page?query=1");
        session
            .set_at(&target, "root=private; Path=/; HttpOnly; Secure", true, NOW)
            .unwrap();
        session.set_at(&target, "first=one", false, NOW).unwrap();
        session.set_at(&target, "second=two", true, NOW).unwrap();
        session
            .set_at(&target, "first=changed", false, NOW)
            .unwrap();
        assert_eq!(
            request(&session, &target),
            "first=changed; second=two; root=private"
        );
        assert_eq!(
            session.get_at(&target, true, true, false, NOW),
            "first=changed; second=two"
        );
        assert_eq!(
            request(&session, &url("https://example.test/a")),
            "first=changed; second=two; root=private"
        );
        assert_eq!(
            request(&session, &url("https://example.test/ab")),
            "root=private"
        );
        assert_eq!(request(&session, &url("http://example.test/")), "");
    }

    #[test]
    fn matching_domain_never_broadens_the_exact_host_scope() {
        let session = Session::default();
        let target = url("https://www.example.test/path");
        session
            .set_at(
                &target,
                "SID=upper; Domain=.EXAMPLE.test; Path=/",
                true,
                NOW,
            )
            .unwrap();
        session
            .set_at(&target, "sid=lower; Domain=www.example.test", true, NOW)
            .unwrap();
        assert_eq!(
            request(&session, &url("https://www.example.test:8443/")),
            "SID=upper; sid=lower"
        );
        for other in [
            "https://example.test/",
            "https://child.www.example.test/",
            "https://badexample.test/",
        ] {
            assert_eq!(request(&session, &url(other)), "");
        }
        for domain in [
            "other.test",
            "badexample.test",
            "example.test.",
            "",
            "..example.test",
        ] {
            assert!(session
                .set_at(&target, &format!("x=secret; Domain={domain}"), true, NOW)
                .is_err());
        }
        let ip = url("http://127.0.0.1/");
        assert!(session
            .set_at(&ip, "ip=yes; Domain=0.0.1", true, NOW)
            .is_err());
        session
            .set_at(&ip, "ip=yes; Domain=127.0.0.1", true, NOW)
            .unwrap();
        assert_eq!(request(&session, &ip), "ip=yes");
    }

    #[test]
    fn scripts_and_insecure_origins_cannot_overwrite_protected_cookies() {
        let session = Session::default();
        let https = url("https://example.test/login/page");
        let http = url("http://example.test/login/page");
        session
            .set_at(
                &https,
                "auth=secret; Path=/login; Secure; HttpOnly",
                true,
                NOW,
            )
            .unwrap();
        for assignment in [
            "auth=changed; Path=/login",
            "auth=; Path=/login; Max-Age=0",
            "other=x; HttpOnly",
        ] {
            assert!(session.set_at(&https, assignment, false, NOW).is_err());
        }
        for assignment in [
            "auth=changed; Path=/login",
            "auth=; Path=/login; Max-Age=0",
            "auth=shadow; Path=/login/deep",
            "new=secret; Secure",
        ] {
            assert!(session.set_at(&http, assignment, true, NOW).is_err());
        }
        assert_eq!(request(&session, &https), "auth=secret");
        assert_eq!(session.get_at(&https, true, true, false, NOW), "");
        session
            .set_at(&http, "auth=separate; Path=/other", true, NOW)
            .unwrap();
        session
            .set_at(
                &https,
                "auth=replaced; Path=/login; Secure; HttpOnly",
                true,
                NOW,
            )
            .unwrap();
        assert_eq!(request(&session, &https), "auth=replaced");
    }

    #[test]
    fn mixed_case_prefixes_keep_their_security_rules_and_distinct_names() {
        let session = Session::default();
        let target = url("https://example.test/");
        for assignment in [
            "__Secure-id=x",
            "__sEcUrE-id=x",
            "__Host-id=x; Secure",
            "__hOsT-id=x; Secure; Path=/else",
            "__Host-id=x; Secure; Path=/; Domain=example.test",
        ] {
            assert!(session.set_at(&target, assignment, true, NOW).is_err());
        }
        assert!(session
            .set_at(
                &url("http://example.test/"),
                "__Secure-id=x; Secure",
                true,
                NOW
            )
            .is_err());
        session
            .set_at(&target, "__Host-id=upper; Secure; Path=/", false, NOW)
            .unwrap();
        session
            .set_at(&target, "__host-id=lower; Secure; Path=/", false, NOW)
            .unwrap();
        assert_eq!(
            request(&session, &target),
            "__Host-id=upper; __host-id=lower"
        );
    }

    #[test]
    fn samesite_lax_defaults_and_safe_top_level_exception_are_explicit() {
        let session = Session::default();
        let target = url("https://example.test/");
        for assignment in [
            "default=a",
            "strict=b; SameSite=Strict",
            "lax=c; SameSite=Lax",
            "none=d; Secure; SameSite=None",
            "unknown=e; SameSite=unknown",
        ] {
            session.set_at(&target, assignment, true, NOW).unwrap();
        }
        assert!(session
            .set_at(&target, "bad=x; SameSite=None", true, NOW)
            .is_err());
        assert_eq!(session.get_at(&target, false, false, false, NOW), "none=d");
        assert_eq!(
            session.get_at(&target, false, false, true, NOW),
            "default=a; lax=c; none=d; unknown=e"
        );
        assert_eq!(
            request(&session, &target),
            "default=a; strict=b; lax=c; none=d; unknown=e"
        );
        assert_eq!(
            session.get_at(&url("http://example.test/"), false, false, false, NOW),
            ""
        );
    }

    #[test]
    fn expiry_max_age_precedence_deletion_and_lifetime_caps() {
        let session = Session::default();
        let target = url("https://example.test/");
        session
            .set_at(
                &target,
                "a=one; Max-Age=2; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
                true,
                NOW,
            )
            .unwrap();
        assert_eq!(
            session.get_at(&target, false, true, false, NOW + 1),
            "a=one"
        );
        assert_eq!(session.get_at(&target, false, true, false, NOW + 2), "");
        session
            .set_at(
                &target,
                "a=two; Expires=Tue, 14 Nov 2023 22:13:21 GMT; Max-Age=+2",
                true,
                NOW,
            )
            .unwrap();
        assert_eq!(request(&session, &target), "a=two");
        assert_eq!(session.get_at(&target, false, true, false, NOW + 1), "");
        session
            .set_at(
                &target,
                "a=three; Max-Age=9999999999999999999999999999999",
                true,
                NOW,
            )
            .unwrap();
        assert_eq!(
            session.get_at(&target, false, true, false, NOW + MAX_AGE_SECONDS - 1),
            "a=three"
        );
        assert_eq!(
            session.get_at(&target, false, true, false, NOW + MAX_AGE_SECONDS),
            ""
        );
        session
            .set_at(
                &target,
                "a=four; Expires=Tue, 01 Jan 9999 00:00:00 GMT",
                true,
                NOW,
            )
            .unwrap();
        assert_eq!(
            session.get_at(&target, false, true, false, NOW + MAX_AGE_SECONDS),
            ""
        );
        session.set_at(&target, "a=five", true, NOW).unwrap();
        session
            .set_at(
                &target,
                "a=gone; Max-Age=-999999999999999999999999999",
                false,
                NOW,
            )
            .unwrap();
        assert_eq!(request(&session, &target), "");
    }

    #[test]
    fn cookie_dates_accept_common_forms_and_reject_invalid_calendar_values() {
        for text in [
            "Tue, 14 Nov 2023 22:13:20 GMT",
            "Tuesday, 14-Nov-23 22:13:20 GMT",
            "Tue Nov 14 22:13:20 2023",
        ] {
            assert_eq!(cookie_date(text), Some(NOW));
        }
        assert_eq!(cookie_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(
            cookie_date("Tue, 29 Feb 2000 00:00:00 GMT"),
            Some(951_782_400)
        );
        assert_eq!(
            cookie_date("Mon, 01 Jan 1601 00:00:00 GMT"),
            Some(-11_644_473_600)
        );
        for text in [
            "31 Feb 2024 00:00:00 GMT",
            "29 Feb 2100 00:00:00 GMT",
            "01 Jan 1599 00:00:00 GMT",
            "01 Jan 2024 24:00:00 GMT",
            "01 Jan 2024 00:00:60 GMT",
            "Jan 2024 00:00:00 GMT",
            "not a date",
        ] {
            assert_eq!(cookie_date(text), None, "{text}");
        }
    }

    #[test]
    fn syntax_rejections_do_not_leak_input_and_valid_quotes_are_retained() {
        let session = Session::default();
        let target = url("https://example.test/");
        for assignment in [
            "=sensitive",
            "bad name=sensitive",
            "bad:name=sensitive",
            "n=sensitive\r\nInjected: yes",
            "n=sensitive\0",
            "n=sensitive,other",
            "n=\"sensitive",
            "n=sensitive\\x",
            "n=sensitiveé",
        ] {
            let error = session.set_at(&target, assignment, true, NOW).unwrap_err();
            assert!(!error.contains("sensitive"));
        }
        assert!(session
            .set_at(&target, &format!("n={}", "x".repeat(4095)), true, NOW)
            .is_err());
        session
            .set_at(&target, "quoted=\"a=b\"; Unknown=ignored", true, NOW)
            .unwrap();
        session.set_at(&target, "empty=", true, NOW).unwrap();
        assert_eq!(request(&session, &target), "quoted=\"a=b\"; empty=");
        assert!(!format!("{session:?}").contains("quoted"));
        assert!(!format!("{session:?}").contains("a=b"));
    }

    #[test]
    fn capacity_rejects_atomically_and_expiry_releases_space() {
        let session = Session::default();
        let target = url("https://example.test/");
        for index in 0..MAX_HOST_COOKIES {
            session
                .set_at(&target, &format!("c{index}=keep; Max-Age=2"), true, NOW)
                .unwrap();
        }
        assert!(session
            .set_at(&target, "extra=rejected", true, NOW)
            .is_err());
        session.set_at(&target, "c0=changed", true, NOW).unwrap();
        assert!(request(&session, &target).starts_with("c0=changed; c1=keep"));
        session
            .set_at(&target, "extra=accepted", true, NOW + 2)
            .unwrap();
        assert_eq!(
            session.get_at(&target, false, true, false, NOW + 2),
            "c0=changed; extra=accepted"
        );

        let total = Session::default();
        for index in 0..MAX_COOKIES {
            total
                .set_at(&url(&format!("https://h{index}.test/")), "n=v", true, NOW)
                .unwrap();
        }
        assert!(total
            .set_at(&url("https://another.test/"), "n=v", true, NOW)
            .is_err());
        let bytes = Session::default();
        for index in 0..15 {
            bytes
                .set_at(
                    &target,
                    &format!("c{index}={}", "v".repeat(4000)),
                    true,
                    NOW,
                )
                .unwrap();
        }
        bytes.set_at(&target, "last=keep", true, NOW).unwrap();
        assert!(bytes
            .set_at(&target, &format!("huge={}", "w".repeat(4090)), true, NOW)
            .is_ok());
        assert!(bytes
            .set_at(&target, &format!("last={}", "x".repeat(4090)), true, NOW)
            .is_err());
        assert!(request(&bytes, &target).ends_with("last=keep"));
    }

    #[test]
    fn output_limit_skips_whole_cookies_without_losing_shorter_later_entries() {
        let session = Session::default();
        let target = url("https://example.test/a/page");
        let first = format!("a={}", "x".repeat(4090));
        let second = format!("b={}", "y".repeat(4090));
        session.set_at(&target, &first, false, NOW).unwrap();
        session.set_at(&target, &second, false, NOW).unwrap();
        session.set_at(&target, "c=last", false, NOW).unwrap();
        let expected = format!("{first}; c=last");
        assert_eq!(request(&session, &target), expected);
        assert_eq!(session.get_at(&target, true, true, false, NOW), expected);
        assert!(expected.len() <= MAX_OUTPUT_BYTES);
    }

    #[test]
    fn public_session_api_is_shared_in_memory_and_never_debugs_values() {
        let session = Arc::new(Session::default());
        let target = url("https://example.test/");
        let workers: Vec<_> = (0..4)
            .map(|index| {
                let session = Arc::clone(&session);
                let target = target.clone();
                std::thread::spawn(move || {
                    session
                        .set_http(&target, &format!("worker{index}=secret; Secure; HttpOnly"))
                        .unwrap();
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        session
            .set_document(&target, "visible=yes; Secure")
            .unwrap();
        assert_eq!(session.document(&target), "visible=yes");
        let sent = session.request_header(&target, true, false);
        assert_eq!(sent.split("; ").count(), 5);
        assert!(!format!("{session:?}").contains("secret"));
        assert!(Session::default()
            .request_header(&target, true, false)
            .is_empty());
    }
}
