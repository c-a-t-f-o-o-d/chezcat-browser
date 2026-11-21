//! Shared original HTTP transport for script APIs. JavaScript adapters own
//! request lifecycle; HTTP owns real sockets, TLS, cookies and cancellation.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScriptCredentials {
    Omit,
    SameOrigin,
    Include,
}
pub(super) fn copy_url(url: &HttpUrl) -> Result<HttpUrl, String> {
    Ok(HttpUrl {
        secure: url.secure,
        host: copy_text(&url.host)?,
        port: url.port,
        path: copy_text(&url.path)?,
        query: url.query.as_deref().map(copy_text).transpose()?,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScriptMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}
impl ScriptMethod {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        }
    }
    pub(crate) fn parse(name: &str) -> Result<Self, (&'static str, &'static str)> {
        if !token(name) {
            return Err(("SyntaxError", "HTTP method must be a nonempty token."));
        }
        if ["CONNECT", "TRACE", "TRACK"]
            .iter()
            .any(|value| name.eq_ignore_ascii_case(value))
        {
            return Err((
                "SecurityError",
                "This HTTP method is forbidden to script requests.",
            ));
        }
        match name.to_ascii_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "HEAD" => Ok(Self::Head),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "OPTIONS" => Ok(Self::Options),
            _ => Err((
                "NotSupportedError",
                "This script HTTP method is not implemented.",
            )),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScriptRequest {
    pub method: ScriptMethod,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
impl ScriptRequest {
    pub(crate) fn try_copy(&self) -> Result<Self, String> {
        let mut headers = Vec::new();
        headers
            .try_reserve_exact(self.headers.len())
            .map_err(|_| "Could not reserve script request headers.")?;
        for (name, value) in &self.headers {
            headers.push((copy_text(name)?, copy_text(value)?));
        }
        let mut body = Vec::new();
        body.try_reserve_exact(self.body.len())
            .map_err(|_| "Could not reserve script request body.")?;
        body.extend_from_slice(&self.body);
        Ok(Self {
            method: self.method,
            headers,
            body,
        })
    }
    pub(crate) fn payload_bytes(&self) -> usize {
        self.body.capacity()
            + self.headers.capacity() * std::mem::size_of::<(String, String)>()
            + self
                .headers
                .iter()
                .map(|(name, value)| name.capacity() + value.capacity())
                .sum::<usize>()
    }
    pub(super) fn wire_headers(&self) -> Result<String, String> {
        if matches!(self.method, ScriptMethod::Get | ScriptMethod::Head) && !self.body.is_empty() {
            return Err("GET/HEAD script requests cannot carry a body.".into());
        }
        if self.body.len() > MAX_BODY_BYTES {
            return Err("Script request body exceeds the HTTP body policy.".into());
        }
        let mut wire = String::new();
        if !matches!(self.method, ScriptMethod::Get | ScriptMethod::Head) {
            wire.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }
        for (name, value) in &self.headers {
            validate_header(name, value)?;
            let size = wire
                .len()
                .checked_add(name.len())
                .and_then(|size| size.checked_add(value.len() + 4))
                .filter(|size| *size < MAX_HEADER_BYTES)
                .ok_or("Script request headers exceed the HTTP header policy.")?;
            wire.try_reserve(size - wire.len())
                .map_err(|_| "Could not reserve script request header wire.")?;
            wire.push_str(name);
            wire.push_str(": ");
            wire.push_str(value);
            wire.push_str("\r\n");
        }
        Ok(wire)
    }
    pub(super) fn redirect(&mut self, status: u16, from: &HttpUrl, to: &HttpUrl) {
        if status == 303 && !matches!(self.method, ScriptMethod::Get | ScriptMethod::Head)
            || matches!(status, 301 | 302) && self.method == ScriptMethod::Post
        {
            self.method = ScriptMethod::Get;
            self.body = Vec::new();
            self.headers.retain(|(name, _)| {
                !matches!(
                    name.to_ascii_lowercase().as_str(),
                    "content-encoding" | "content-language" | "content-location" | "content-type"
                )
            });
        }
        if !from.same_origin(to) {
            self.headers
                .retain(|(name, _)| !name.eq_ignore_ascii_case("authorization"));
        }
    }
    /// Until native preflight is implemented, refuse a request which needs it
    /// before contacting a foreign origin. No privileged same-origin headers
    /// or body are forwarded under a no-CORS shortcut.
    fn check_cors_request(&self, origin: &HttpUrl, target: &HttpUrl) -> Result<(), String> {
        if origin.same_origin(target) {
            return Ok(());
        }
        let mut total = 0usize;
        if !matches!(
            self.method,
            ScriptMethod::Get | ScriptMethod::Head | ScriptMethod::Post
        ) {
            return Err("Script request requires an unimplemented CORS preflight.".into());
        }
        for (name, value) in &self.headers {
            total = total.saturating_add(value.len());
            if !cors_safe_header(name, value) || total > 1024 {
                return Err(
                    "Script request headers require an unimplemented CORS preflight.".into(),
                );
            }
        }
        Ok(())
    }
}
fn copy_text(text: &str) -> Result<String, String> {
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| "Could not reserve script request text.")?;
    value.push_str(text);
    Ok(value)
}
pub(crate) fn token(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}
pub(crate) fn forbidden_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("proxy-")
        || name.starts_with("sec-")
        || matches!(
            name.as_str(),
            "accept-charset"
                | "accept-encoding"
                | "access-control-request-headers"
                | "access-control-request-method"
                | "connection"
                | "content-length"
                | "cookie"
                | "cookie2"
                | "date"
                | "dnt"
                | "expect"
                | "host"
                | "keep-alive"
                | "origin"
                | "permissions-policy"
                | "referer"
                | "set-cookie"
                | "te"
                | "trailer"
                | "transfer-encoding"
                | "upgrade"
                | "user-agent"
                | "via"
                | "x-http-method"
                | "x-http-method-override"
                | "x-method-override"
        )
}
pub(crate) fn validate_header(name: &str, value: &str) -> Result<(), String> {
    if !token(name) || forbidden_header(name) {
        return Err("Invalid or browser-controlled script request header.".into());
    }
    if !value
        .bytes()
        .all(|byte| byte == b'\t' || (32..=126).contains(&byte))
    {
        return Err(
            "Script request header values require supported ASCII field text without CR/LF/NUL."
                .into(),
        );
    }
    if name.len() + value.len() + 2 > MAX_LINE_BYTES {
        return Err("Script request header exceeds the HTTP field policy.".into());
    }
    Ok(())
}
fn cors_safe_header(name: &str, value: &str) -> bool {
    if value.len() > 128 {
        return false;
    }
    let unsafe_byte =
        |byte: u8| byte < 32 && byte != b'\t' || byte == 127 || b"\"():<>?@[\\]{}".contains(&byte);
    match name.to_ascii_lowercase().as_str() {
        "accept" => !value.bytes().any(unsafe_byte),
        "accept-language" | "content-language" => value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b" *,-.;=".contains(&byte)),
        "content-type" => {
            !value.bytes().any(unsafe_byte)
                && matches!(
                    value
                        .split(';')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_ascii_lowercase()
                        .as_str(),
                    "application/x-www-form-urlencoded" | "multipart/form-data" | "text/plain"
                )
        }
        _ => false,
    }
}
fn filter_response_headers(
    headers: &mut Vec<(String, String)>,
    cors: bool,
    credentials: ScriptCredentials,
) {
    let mut exposed = Vec::new();
    if cors {
        for (_, value) in headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("access-control-expose-headers"))
        {
            exposed.extend(
                value
                    .split(',')
                    .map(|name| name.trim().to_ascii_lowercase()),
            );
        }
    }
    let wildcard =
        credentials != ScriptCredentials::Include && exposed.iter().any(|name| name == "*");
    headers.retain(|(name, _)| {
        let name = name.to_ascii_lowercase();
        !matches!(name.as_str(), "set-cookie" | "set-cookie2")
            && (!cors
                || wildcard
                || matches!(
                    name.as_str(),
                    "cache-control"
                        | "content-language"
                        | "content-length"
                        | "content-type"
                        | "expires"
                        | "last-modified"
                        | "pragma"
                )
                || exposed.contains(&name))
    });
}
impl Pool<'_> {
    pub(crate) fn fetch_script_request(
        &self,
        url: &HttpUrl,
        request: &ScriptRequest,
        limit: usize,
        origin: &HttpUrl,
        credentials: ScriptCredentials,
        policy: &crate::csp::PolicySet,
        guard: &ResourceFilter<'_>,
    ) -> Result<Response, String> {
        // Admission validates even fields not needed by the selected CORS path.
        request.wire_headers()?;
        let crossed = std::sync::atomic::AtomicBool::new(false);
        let filter = |target: &HttpUrl| {
            guard(target)?;
            policy.allows_connect(&crate::navigation::Location {
                url: Some(target.clone()),
                path: None,
                fragment: String::new(),
                root: None,
            })?;
            request.check_cors_request(origin, target)?;
            if !origin.same_origin(target) {
                crossed.store(true, Ordering::Release);
            }
            Ok(())
        };
        let outgoing = Request::Script {
            request: request.try_copy()?,
            origin: copy_url(origin)?,
        };
        let mut response = fetch_in_pool(
            self,
            url,
            &outgoing,
            BodyLimits::uniform(limit),
            None,
            FetchMode::Normal,
            true,
            "script-request",
            false,
            None,
            Some(&filter),
            Some(ResourceFetch::Media(media_cors::Config {
                origin,
                credentials: if credentials == ScriptCredentials::Include {
                    MediaCredentials::Include
                } else {
                    MediaCredentials::SameOrigin
                },
                omit: credentials == ScriptCredentials::Omit,
            })),
        )?;
        filter_response_headers(
            &mut response.headers,
            crossed.load(Ordering::Acquire),
            credentials,
        );
        Ok(response)
    }
}
#[cfg(test)]
#[path = "http_script_request_tests.rs"]
mod tests;
