//! Original, narrow stylesheet GET policy over the existing HTTP transport.
//! Ordinary unmarked LINK requests use no-CORS/include, not classic-script
//! credential omission. Explicit CORS mode remains exact-origin in this slice.
use super::*;

#[derive(Debug)]
pub(crate) struct StylesheetResponse {
    pub response: Response,
    /// Actual initial and redirected destinations, in order, for cache policy replay.
    pub hops: Vec<HttpUrl>,
    /// Sticky even when a later redirect returns to the document origin.
    pub cross_origin: bool,
    /// Complete decoded body bytes observed, including any later rejected response.
    pub received_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct StylesheetFailure {
    pub message: String,
    /// Known complete decoded bodies, not inferred partial wire/HTTP byte counts.
    pub received_bytes: usize,
}

impl std::fmt::Display for StylesheetFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

pub(crate) fn check_request(
    origin: &HttpUrl,
    target: &HttpUrl,
    same_origin_only: bool,
) -> Result<(), String> {
    if origin.secure && !target.secure {
        return Err("HTTPS stylesheets cannot use an unencrypted HTTP source.".into());
    }
    if same_origin_only && !origin.same_origin(target) {
        return Err("Cross-origin CORS-mode stylesheets are not implemented; only same-origin stylesheet URLs are supported.".into());
    }
    Ok(())
}

pub(super) struct State<'a> {
    origin: &'a HttpUrl,
    same_origin_only: bool,
    cookies: crate::cookies::NavigationContext<'a>,
    hops: Vec<HttpUrl>,
    cross_origin: bool,
    limit: usize,
    received_bytes: usize,
}

impl<'a> State<'a> {
    pub(super) fn new(origin: &'a HttpUrl, same_origin_only: bool, limit: usize) -> Self {
        Self { origin, same_origin_only,
            cookies: crate::cookies::NavigationContext::new(Some(origin), None),
            hops: Vec::new(), cross_origin: false, limit, received_bytes: 0 }
    }

    pub(super) fn prepare(&mut self, target: &HttpUrl) -> Result<(), String> {
        check_request(self.origin, target, self.same_origin_only)?;
        if self.hops.len() > MAX_REDIRECTS {
            return Err("Stylesheet redirect provenance exceeds the HTTP hop limit.".into());
        }
        let target = copy_url(target)?;
        self.hops.try_reserve(1).map_err(|_| "Cannot retain stylesheet redirect provenance.")?;
        self.cross_origin |= !self.origin.same_origin(&target);
        self.hops.push(target);
        Ok(())
    }

    pub(super) fn remaining_bytes(&self) -> usize {
        self.limit.saturating_sub(self.received_bytes)
    }

    /// Called immediately after a complete decoded exchange, before CORP,
    /// status, MIME or redirect decisions can reject or discard its body.
    pub(super) fn received(&mut self, bytes: usize) -> Result<(), String> {
        self.received_bytes = self.received_bytes.checked_add(bytes)
            .ok_or("Stylesheet received-byte accounting overflow.")?;
        if self.received_bytes > self.limit {
            return Err("Stylesheet responses exceed their aggregate body allowance.".into());
        }
        Ok(())
    }

    pub(super) fn failure(&self, message: String) -> StylesheetFailure {
        StylesheetFailure { message, received_bytes: self.received_bytes }
    }

    pub(super) fn same_site(&self, target: &HttpUrl) -> bool {
        self.cookies.is_same_site(target)
    }

    pub(super) fn redirect(&mut self, from: &HttpUrl, to: &HttpUrl) {
        self.cookies.redirect(from, to);
    }

    /// Run after eligible Set-Cookie effects, before another redirected request.
    pub(super) fn check_response(&self, target: &HttpUrl, headers: &[(String, String)]) -> Result<(), String> {
        if self.origin.same_origin(target) { return Ok(()); }
        let mut values = headers.iter().filter(|(name, _)| name.eq_ignore_ascii_case("cross-origin-resource-policy"));
        let Some((_, value)) = values.next() else { return Ok(()); };
        // Fetch combines repeated fields; a combined/unknown token is not a
        // recognized CORP policy under unsafe-none. COEP is enforced by the
        // caller's per-hop document policy and never inferred from this header.
        if values.next().is_some() { return Ok(()); }
        match value.trim() {
            "same-origin" => Err("Cross-Origin-Resource-Policy same-origin blocked this stylesheet response.".into()),
            "same-site" if !crate::cookies::same_site(self.origin, target) =>
                Err("Cross-Origin-Resource-Policy same-site blocked this stylesheet response.".into()),
            _ => Ok(()),
        }
    }

    fn validate(&self, response: &Response) -> Result<(), String> {
        if !(200..300).contains(&response.status) {
            return Err(format!("HTTP {} while fetching a stylesheet.", response.status));
        }
        if response.status == 206 || response.headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("content-range")) {
            return Err("Stylesheets require a complete response; unsolicited partial content is not supported.".into());
        }
        let content_type = unique_header(&response.headers, "content-type")?.unwrap_or("");
        let mime = content_type.split(';').next().unwrap_or("").trim();
        if !mime.eq_ignore_ascii_case("text/css") && (self.cross_origin || !mime.is_empty()) {
            return Err(if mime.is_empty() {
                "Cross-origin stylesheets require an unambiguous text/css Content-Type.".into()
            } else {
                "Stylesheet response has an unsupported Content-Type; text/css is required.".into()
            });
        }
        // The same-origin initial loader's existing missing-type subset stays
        // available. Dynamic callers retain their stricter text/css check.
        Ok(())
    }

    pub(super) fn finish(self, response: Response) -> Result<StylesheetResponse, StylesheetFailure> {
        if let Err(error) = self.validate(&response) {
            return Err(self.failure(failure_with_activity(error, &response.events)));
        }
        Ok(StylesheetResponse { response, hops: self.hops,
            cross_origin: self.cross_origin, received_bytes: self.received_bytes })
    }
}

fn copy_url(url: &HttpUrl) -> Result<HttpUrl, String> {
    fn copy(value: &str) -> Result<String, String> {
        let mut result = String::new();
        result.try_reserve(value.len()).map_err(|_| "Cannot retain a stylesheet destination.")?;
        result.push_str(value);
        Ok(result)
    }
    Ok(HttpUrl { secure: url.secure, host: copy(&url.host)?, port: url.port,
        path: copy(&url.path)?, query: url.query.as_deref().map(copy).transpose()? })
}

#[cfg(test)]
#[path = "http_stylesheet_tests.rs"]
mod tests;
