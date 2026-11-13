//! Original, bounded Content Security Policy checks for classic scripts, linked stylesheets and fonts.
//!
//! CSP3 source-list, nonce/hash, fallback and multiple-policy semantics:
//! https://www.w3.org/TR/CSP3/
//! This is not full CSP: script sources, linked style sources, font-src and base-uri are implemented;
//! other resource/navigation directives are diagnosed but not implemented.
//! Both parser-loaded and inserted external classic scripts use the same bounded
//! source checks. strict-dynamic never propagates trust in this subset;
//! SRI-bearing external scripts remain unsupported.

use crate::crypto_hash;
use crate::crypto_sha512;
use crate::html::{Document, NodeKind};
use crate::navigation::Location;
use crate::url::{self, HttpUrl};

const MAX_POLICY_BYTES: usize = 64 * 1024;
const MAX_POLICIES: usize = 16;
const MAX_DIRECTIVES: usize = 64;
const MAX_SOURCES: usize = 128;
const MAX_TOKEN_BYTES: usize = 2048;
const MAX_DIAGNOSTICS: usize = 24;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct PolicySet {
    origin: Option<Location>,
    policies: Vec<Policy>,
    bytes: usize,
    blocked_from: Option<usize>,
    diagnostics: Vec<String>,
    // Full cross-origin embedder isolation is outside this subset. An enforced
    // restrictive policy conservatively retains exact-origin script loading.
    restrict_cross_origin_scripts: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Policy {
    start: usize,
    default: Option<Vec<Source>>,
    scripts: Option<Vec<Source>>,
    elements: Option<Vec<Source>>,
    attributes: Option<Vec<Source>>,
    styles: Option<Vec<Source>>,
    style_elements: Option<Vec<Source>>,
    fonts: Option<Vec<Source>>,
    base: Option<Vec<Source>>,
    forms: Option<Vec<Source>>,
    sandbox: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    SelfOrigin,
    AnyHttp,
    Scheme(bool),
    Origin {
        secure: bool,
        host: String,
        port: Option<u16>,
    },
    Nonce(String),
    Hash(Hash, String),
    UnsafeInline,
    UnsafeEval,
    UnsafeHashes,
    StrictDynamic,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hash {
    Sha256,
    Sha384,
    Sha512,
}

#[derive(Default)]
struct Digests {
    sha256: Option<String>,
    sha384: Option<String>,
    sha512: Option<String>,
}

impl Digests {
    fn matches(&mut self, algorithm: Hash, expected: &str, source: &str) -> bool {
        let actual = match algorithm {
            Hash::Sha256 => self
                .sha256
                .get_or_insert_with(|| base64(&crypto_hash::sha256(source.as_bytes()))),
            Hash::Sha384 => self
                .sha384
                .get_or_insert_with(|| base64(&crypto_sha512::sha384(source.as_bytes()))),
            Hash::Sha512 => self
                .sha512
                .get_or_insert_with(|| base64(&crypto_sha512::sha512(source.as_bytes()))),
        };
        actual == expected
    }
}

impl PolicySet {
    /// Work/storage represented by an independent policy snapshot. Child
    /// contexts charge this before cloning rather than sharing mutable policy.
    pub(crate) fn clone_work_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>() + self.origin.as_ref().map_or(0, |origin| origin.display().len());
        bytes += self.diagnostics.iter().map(|line| std::mem::size_of::<String>() + line.len()).sum::<usize>();
        for policy in &self.policies {
            bytes += std::mem::size_of::<Policy>();
            for sources in [&policy.default, &policy.scripts, &policy.elements, &policy.attributes,
                &policy.styles, &policy.style_elements, &policy.fonts, &policy.base, &policy.forms].into_iter().flatten() {
                bytes += sources.iter().map(|source| std::mem::size_of::<Source>() + match source {
                    Source::Origin { host, .. } => host.len(),
                    Source::Nonce(text) | Source::Hash(_, text) => text.len(),
                    _ => 0,
                }).sum::<usize>();
            }
        }
        bytes
    }
    pub fn new(origin: &Location) -> Self {
        Self {
            origin: Some(origin.clone()),
            ..Self::default()
        }
    }

    pub fn add_header(&mut self, value: &str) {
        // The HTTP field is a comma-separated list of independently enforced
        // policies. Commas are not part of CSP directive values.
        self.add(value, 0, false);
    }

    /// Preserve restrictive document embedder policies without pretending
    /// to implement cross-origin isolation or a CORS mode. Report-only headers
    /// do not call this method. Multiple enforced headers only add restrictions.
    pub fn add_embedder_header(&mut self, value: &str) {
        let token = value.split(';').next().unwrap_or("").trim();
        if matches!(token, "require-corp" | "credentialless") {
            self.restrict_cross_origin_scripts = true;
            self.note("Cross-Origin-Embedder-Policy: restrictive policy present; cross-origin scripts are conservatively declined because full embedder-policy isolation is not implemented.".into());
        }
    }

    pub fn add_meta(&mut self, value: &str, node: usize) {
        self.add(value, node.saturating_add(1), true);
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    fn note(&mut self, text: String) {
        if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(text);
        }
    }

    fn block_from(&mut self, start: usize, reason: &str) {
        self.blocked_from = Some(self.blocked_from.map_or(start, |old| old.min(start)));
        self.note(format!(
            "CSP: {reason}; subsequent scripts, linked stylesheets, fonts and inline handlers are blocked."
        ));
    }

    fn add(&mut self, value: &str, start: usize, meta: bool) {
        if value.len() > MAX_POLICY_BYTES.saturating_sub(self.bytes) {
            self.block_from(start, "policy byte limit reached (64 KiB)");
            return;
        }
        self.bytes += value.len();
        // HTTP fields contain a policy list; a meta content value is one
        // serialized policy. Treating a comma inside meta as a separator could
        // accidentally recognize a malformed unsafe-inline token as valid.
        for serialized in value.split(|character| !meta && character == ',') {
            if serialized.trim().is_empty() {
                continue;
            }
            if self.policies.len() == MAX_POLICIES {
                self.block_from(start, "policy count limit reached (16)");
                return;
            }
            match parse_policy(serialized, start, meta) {
                Ok((policy, unknown_sources, other_directives)) => {
                    self.policies.push(policy);
                    let delivery = if meta { "meta" } else { "header" };
                    self.note(format!("CSP {delivery}: script/linked-style/font/base-uri/form-action policy enforced; {unknown_sources} unsupported source expression(s) grant no permission; {other_directives} other directive(s) are outside this subset. No violation reports are sent."));
                }
                Err(reason) => self.block_from(start, reason),
            }
        }
    }

    fn active(&self, node: usize) -> Result<impl Iterator<Item = &Policy>, String> {
        if self.blocked_from.is_some_and(|start| node >= start) {
            return Err("Content Security Policy blocked this resource because its policy exceeded an implementation limit.".into());
        }
        Ok(self
            .policies
            .iter()
            .filter(move |policy| node >= policy.start))
    }

    pub fn allows_inline(
        &self,
        node: usize,
        nonce: Option<&str>,
        source: &str,
    ) -> Result<(), String> {
        let mut digests = Digests::default();
        for policy in self.active(node)? {
            policy.allow_sandbox()?;
            if let Some(list) = policy.script_list(false) {
                if !inline_matches(list, nonce, source, false, &mut digests) {
                    return Err("Content Security Policy blocked an inline script (nonce/hash/inline source did not match).".into());
                }
            }
        }
        Ok(())
    }

    pub fn allows_external(
        &self,
        node: usize,
        nonce: Option<&str>,
        target: &Location,
    ) -> Result<(), String> {
        if self.restrict_cross_origin_scripts
            && self
                .origin
                .as_ref()
                .is_none_or(|origin| match (&origin.url, &target.url) {
                    (Some(origin), Some(target)) => !origin.same_origin(target),
                    _ => origin != target,
                })
        {
            return Err("Cross-Origin-Embedder-Policy requires the document's exact origin for scripts in this build.".into());
        }
        for policy in self.active(node)? {
            policy.allow_sandbox()?;
            if let Some(list) = policy.script_list(false) {
                // Nonces authorize parser scripts even with strict-dynamic.
                // Hash sources cannot authorize a fetch without enforced SRI.
                if nonce_matches(list, nonce) {
                    continue;
                }
                if list.iter().any(|s| matches!(s, Source::StrictDynamic))
                    || !list
                        .iter()
                        .any(|s| url_matches(s, self.origin.as_ref(), target))
                {
                    return Err("Content Security Policy blocked an external script source.".into());
                }
            }
        }
        Ok(())
    }

    pub fn allows_attribute(&self, source: &str) -> Result<(), String> {
        // Handlers are compiled after parsing and can execute much later, so
        // every installed policy applies even to attributes before a meta tag.
        let mut digests = Digests::default();
        for policy in self.active(usize::MAX)? {
            policy.allow_sandbox()?;
            if let Some(list) = policy.script_list(true) {
                if !inline_matches(list, None, source, true, &mut digests) {
                    return Err("Content Security Policy blocked an inline event handler.".into());
                }
            }
        }
        Ok(())
    }

    /// String compilation is governed by script-src/default-src, never by
    /// script-src-elem or script-src-attr. Scripts run after parsing, so all
    /// installed header/meta policies apply to eval and later callbacks.
    pub fn allows_eval(&self) -> Result<(), String> {
        for policy in self.active(usize::MAX)? {
            policy.allow_sandbox()?;
            if let Some(list) = policy.scripts.as_ref().or(policy.default.as_ref()) {
                if !list
                    .iter()
                    .any(|source| matches!(source, Source::UnsafeEval))
                {
                    return Err("Content Security Policy blocked string compilation: script-src/default-src does not allow 'unsafe-eval'.".into());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn allows_initial_blank_frame(&self) -> Result<(), String> {
        for policy in self.active(usize::MAX)? { policy.allow_sandbox()?; }
        Ok(())
    }

    pub fn allows_base(&self, node: usize, target: &Location) -> Result<(), String> {
        for policy in self.active(node)? {
            if let Some(list) = &policy.base {
                if !list
                    .iter()
                    .any(|s| url_matches(s, self.origin.as_ref(), target))
                {
                    return Err("Content Security Policy base-uri blocked this base URL.".into());
                }
            }
        }
        Ok(())
    }

    /// Form navigation uses every installed policy and has no default-src
    /// fallback. Script form submissions carry this snapshot through redirects.
    pub fn allows_form_action(&self, target: &Location) -> Result<(), String> {
        for policy in self.active(usize::MAX)? {
            if let Some(list) = &policy.forms {
                if !list
                    .iter()
                    .any(|source| url_matches(source, self.origin.as_ref(), target))
                {
                    return Err(
                        "Content Security Policy form-action blocked this form destination.".into(),
                    );
                }
            }
        }
        Ok(())
    }

    /// External LINK elements use style-src-elem, then style-src/default-src.
    /// Script nonces never authorize styles unless a style directive matches.
    /// Inline style elements/attributes remain outside this external-fetch API.
    pub fn allows_external_style(
        &self,
        node: usize,
        nonce: Option<&str>,
        target: &Location,
    ) -> Result<(), String> {
        for policy in self.active(node)? {
            if let Some(list) = policy
                .style_elements
                .as_ref()
                .or(policy.styles.as_ref())
                .or(policy.default.as_ref())
            {
                if !nonce_matches(list, nonce)
                    && !list
                        .iter()
                        .any(|source| url_matches(source, self.origin.as_ref(), target))
                {
                    return Err("Content Security Policy style-src-elem/style-src/default-src blocked this stylesheet source.".into());
                }
            }
        }
        Ok(())
    }

    pub fn allows_font(&self, node: usize, target: &Location) -> Result<(), String> {
        for policy in self.active(node)? {
            if let Some(list) = policy.fonts.as_ref().or(policy.default.as_ref()) {
                if !list
                    .iter()
                    .any(|source| url_matches(source, self.origin.as_ref(), target))
                {
                    return Err(
                        "Content Security Policy font-src/default-src blocked this font source."
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }
}

impl Policy {
    fn script_list(&self, attribute: bool) -> Option<&[Source]> {
        (if attribute {
            &self.attributes
        } else {
            &self.elements
        })
        .as_ref()
        .or(self.scripts.as_ref())
        .or(self.default.as_ref())
        .map(Vec::as_slice)
    }

    fn allow_sandbox(&self) -> Result<(), String> {
        if self.sandbox {
            Err("Content Security Policy sandbox is not implemented; scripting is blocked for this policy.".into())
        } else {
            Ok(())
        }
    }
}

fn parse_policy(
    value: &str,
    start: usize,
    meta: bool,
) -> Result<(Policy, usize, usize), &'static str> {
    let mut policy = Policy {
        start,
        default: None,
        scripts: None,
        elements: None,
        attributes: None,
        styles: None,
        style_elements: None,
        fonts: None,
        base: None,
        forms: None,
        sandbox: false,
    };
    let mut seen = std::collections::HashSet::new();
    let mut unknown_sources = 0;
    let mut other_directives = 0;
    for (count, directive) in value.split(';').enumerate() {
        if count >= MAX_DIRECTIVES {
            return Err("directive count limit reached (64)");
        }
        let mut words = directive.split_ascii_whitespace();
        let Some(name) = words.next() else {
            continue;
        };
        if name.len() > MAX_TOKEN_BYTES {
            return Err("directive name exceeds the parser limit");
        }
        let name = name.to_ascii_lowercase();
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            // An invalid directive does not become a recognized permissive one.
            other_directives += 1;
            continue;
        }
        if !seen.insert(name.clone()) {
            continue;
        } // First occurrence wins.
        let destination = match name.as_str() {
            "default-src" => &mut policy.default,
            "script-src" => &mut policy.scripts,
            "script-src-elem" => &mut policy.elements,
            "script-src-attr" => &mut policy.attributes,
            "style-src" => &mut policy.styles,
            "style-src-elem" => &mut policy.style_elements,
            "font-src" => &mut policy.fonts,
            "base-uri" => &mut policy.base,
            "form-action" => &mut policy.forms,
            "sandbox" => {
                policy.sandbox = !meta;
                continue;
            }
            _ => {
                other_directives += 1;
                continue;
            }
        };
        let mut sources = Vec::new();
        for word in words {
            if sources.len() == MAX_SOURCES {
                return Err("source-list limit reached (128)");
            }
            if word.len() > MAX_TOKEN_BYTES {
                return Err("source expression exceeds the parser limit (2048 bytes)");
            }
            if let Some(source) = parse_source(word) {
                sources.push(source);
            } else {
                unknown_sources += 1;
                sources.push(Source::Deny);
            }
        }
        *destination = Some(sources);
    }
    Ok((policy, unknown_sources, other_directives))
}

fn parse_source(value: &str) -> Option<Source> {
    let lower = value.to_ascii_lowercase();
    match lower.as_str() {
        "'self'" => return Some(Source::SelfOrigin),
        "*" => return Some(Source::AnyHttp),
        "http:" => return Some(Source::Scheme(false)),
        "https:" => return Some(Source::Scheme(true)),
        "'unsafe-inline'" => return Some(Source::UnsafeInline),
        "'unsafe-eval'" => return Some(Source::UnsafeEval),
        "'unsafe-hashes'" => return Some(Source::UnsafeHashes),
        "'strict-dynamic'" => return Some(Source::StrictDynamic),
        "'none'" | "'wasm-unsafe-eval'" | "'report-sample'" => return Some(Source::Deny),
        _ => {}
    }
    if lower.starts_with("'nonce-") && value.ends_with('\'') {
        let nonce = &value[7..value.len() - 1];
        return valid_base64(nonce).then(|| Source::Nonce(nonce.into()));
    }
    for (prefix, algorithm) in [
        ("'sha256-", Hash::Sha256),
        ("'sha384-", Hash::Sha384),
        ("'sha512-", Hash::Sha512),
    ] {
        if lower.starts_with(prefix) && value.ends_with('\'') {
            let digest = &value[prefix.len()..value.len() - 1];
            return valid_base64(digest)
                .then(|| Source::Hash(algorithm, digest.replace('-', "+").replace('_', "/")));
        }
    }
    let secure = if lower.starts_with("https://") {
        true
    } else if lower.starts_with("http://") {
        false
    } else {
        return None;
    };
    let rest = &value[if secure { 8 } else { 7 }..];
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    // Unsupported path/host wildcard/schemeless expressions grant nothing;
    // never turn a path-limited source into permission for an entire host.
    if authority.is_empty() || authority.contains(['/', '?', '#', '*', '@', '[', ']']) {
        return None;
    }
    let (parsed, fragment) = url::parse(value).ok()?;
    if parsed.path != "/"
        || parsed.query.is_some()
        || !fragment.is_empty()
        || parsed.host.parse::<std::net::IpAddr>().is_ok()
    {
        return None;
    }
    let port = authority.rsplit_once(':').map(|_| parsed.port);
    Some(Source::Origin {
        secure,
        host: parsed.host,
        port,
    })
}

fn nonce_matches(list: &[Source], nonce: Option<&str>) -> bool {
    nonce.filter(|n| !n.is_empty()).is_some_and(|nonce| {
        list.iter()
            .any(|source| matches!(source, Source::Nonce(expected) if nonce == expected))
    })
}

fn inline_matches(
    list: &[Source],
    nonce: Option<&str>,
    source: &str,
    attribute: bool,
    digests: &mut Digests,
) -> bool {
    if !attribute && nonce_matches(list, nonce) {
        return true;
    }
    let overrides_inline = list.iter().any(|s| {
        matches!(
            s,
            Source::Nonce(_) | Source::Hash(_, _) | Source::StrictDynamic
        )
    });
    if !overrides_inline && list.iter().any(|s| matches!(s, Source::UnsafeInline)) {
        return true;
    }
    if !attribute || list.iter().any(|s| matches!(s, Source::UnsafeHashes)) {
        return list.iter().any(|s| match s {
            Source::Hash(algorithm, expected) => digests.matches(*algorithm, expected, source),
            _ => false,
        });
    }
    false
}

fn url_matches(source: &Source, origin: Option<&Location>, target: &Location) -> bool {
    match source {
        Source::SelfOrigin => match (origin, &target.url) {
            (Some(origin), Some(target)) => origin
                .url
                .as_ref()
                .is_some_and(|own| self_matches(own, target)),
            (Some(origin), None) => {
                origin.url.is_none()
                    && origin.root.is_some()
                    && origin.root == target.root
                    && target.path.is_some()
            }
            _ => false,
        },
        Source::AnyHttp => target.url.is_some(),
        Source::Scheme(secure) => target.url.as_ref().is_some_and(|url| !secure || url.secure),
        Source::Origin { secure, host, port } => target.url.as_ref().is_some_and(|url| {
            (!secure || url.secure)
                && host == &url.host
                && port.map_or(url.port == if url.secure { 443 } else { 80 }, |port| {
                    port == url.port
                })
        }),
        _ => false,
    }
}

fn self_matches(origin: &HttpUrl, target: &HttpUrl) -> bool {
    origin.same_origin(target)
        || origin.host == target.host
            && (origin.port == target.port
                || origin.port == if origin.secure { 443 } else { 80 }
                    && target.port == if target.secure { 443 } else { 80 })
            && (target.secure || !origin.secure && !target.secure)
}

pub fn nonce(doc: &Document, node: usize) -> Option<&str> {
    if doc.nonce_unsafe_nodes.contains(&node) {
        return None;
    }
    let NodeKind::Element { attrs, .. } = &doc.nodes.get(node)?.kind else {
        return None;
    };
    let forbidden = |value: &str| {
        [b"<link".as_slice(), b"<script", b"<style"]
            .iter()
            .any(|needle| {
                value
                    .as_bytes()
                    .windows(needle.len())
                    .any(|part| part.eq_ignore_ascii_case(needle))
            })
    };
    if attrs
        .iter()
        .any(|(name, value)| forbidden(name) || forbidden(value))
    {
        return None;
    }
    doc.attr(node, "nonce")
}

fn valid_base64(value: &str) -> bool {
    let main = value.trim_end_matches('=');
    !main.is_empty()
        && value.len() - main.len() <= 2
        && main
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_'))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let value = (u32::from(group[0]) << 16)
            | (u32::from(*group.get(1).unwrap_or(&0)) << 8)
            | u32::from(*group.get(2).unwrap_or(&0));
        for shift in [18, 12] {
            result.push(ALPHABET[((value >> shift) & 63) as usize] as char);
        }
        result.push(if group.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        result.push(if group.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn location(value: &str) -> Location {
        Location {
            url: Some(url::parse(value).unwrap().0),
            path: None,
            root: None,
            fragment: String::new(),
        }
    }
    fn policy(text: &str) -> PolicySet {
        let mut policy = PolicySet::new(&location("https://example.test/page"));
        policy.add_header(text);
        policy
    }

    #[test]
    fn eval_checks_intersect_script_default_policies_without_element_nonce_permission() {
        for source in [
            "",
            "script-src 'unsafe-eval'",
            "default-src 'unsafe-eval'; script-src-elem 'none'; script-src-attr 'none'",
            "default-src 'none'; script-src 'nonce-known' 'strict-dynamic' 'unsafe-eval'",
            "script-src-elem 'none'",
        ] {
            assert!(policy(source).allows_eval().is_ok(), "{source}");
        }
        for source in [
            "default-src *",
            "script-src 'self' 'unsafe-inline'",
            "script-src 'nonce-known'; script-src-elem 'unsafe-eval'",
            "default-src 'unsafe-eval'; script-src 'none'",
            "script-src 'unsafe-eval', script-src 'none'",
            "script-src 'none'; script-src 'unsafe-eval'",
            "script-src 'wasm-unsafe-eval'",
            "sandbox allow-scripts; script-src 'unsafe-eval'",
        ] {
            assert!(policy(source).allows_eval().is_err(), "{source}");
        }
        let mut policies = policy("script-src 'unsafe-eval'");
        policies.add_meta("script-src 'self'", 25);
        assert!(policies.allows_eval().is_err());
        let mut policies = policy("script-src 'unsafe-eval'");
        policies.add_meta(&"x".repeat(MAX_POLICY_BYTES + 1), 25);
        assert!(policies.allows_eval().is_err());
    }

    #[test]
    fn directive_fallback_and_first_occurrence_are_independent() {
        let same = location("https://example.test/app.js");
        let other = location("https://other.test/app.js");
        let p = policy("default-src 'none'; script-src 'self'; script-src-elem 'unsafe-inline'; script-src-attr 'none'");
        assert!(p.allows_inline(1, None, "1;").is_ok());
        assert!(p.allows_external(1, None, &same).is_err());
        assert!(p.allows_attribute("1;").is_err());
        let p = policy("default-src 'none'; SCRIPT-SRC 'self'; script-src * 'unsafe-inline'");
        assert!(p.allows_external(1, None, &same).is_ok());
        assert!(p.allows_external(1, None, &other).is_err());
        assert!(p.allows_inline(1, None, "1;").is_err());
        let p = policy("img-src 'none'; made-up anything");
        assert!(p.allows_inline(1, None, "1;").is_ok());
        assert!(p.diagnostics()[0].contains("2 other directive"));
    }

    #[test]
    fn form_action_has_no_default_fallback_and_uses_all_installed_policies() {
        let same = location("https://example.test/search?q=one");
        let other = location("https://other.test/search?q=one");
        assert!(policy("default-src 'none'")
            .allows_form_action(&other)
            .is_ok());
        assert!(policy("form-action 'self'")
            .allows_form_action(&same)
            .is_ok());
        assert!(policy("form-action 'self'")
            .allows_form_action(&other)
            .is_err());
        assert!(policy("form-action 'none'; form-action *")
            .allows_form_action(&same)
            .is_err());
        assert!(policy("form-action *, form-action 'none'")
            .allows_form_action(&same)
            .is_err());
        assert!(policy("form-action 'nonce-One' 'unsafe-inline'")
            .allows_form_action(&same)
            .is_err());
        let mut set = policy("form-action 'self'");
        set.add_meta("form-action 'none'", 10);
        assert!(set.allows_form_action(&same).is_err());
    }

    #[test]
    fn restrictive_embedder_headers_keep_classic_scripts_at_exact_origin() {
        let same = location("https://example.test/a.js");
        let other = location("https://cdn.test/a.js");
        for value in [
            "require-corp",
            "require-corp; report-to=endpoint",
            "credentialless",
        ] {
            let mut p = policy("script-src https:");
            p.add_embedder_header(value);
            assert!(p.allows_external(1, None, &same).is_ok());
            assert!(p
                .allows_external(1, None, &other)
                .unwrap_err()
                .contains("Embedder"));
        }
        let mut p = policy("script-src https:");
        p.add_embedder_header("unsafe-none");
        assert!(p.allows_external(1, None, &other).is_ok());
        p.add_embedder_header("require-corp");
        p.add_embedder_header("unsafe-none");
        assert!(p.allows_external(1, None, &other).is_err());
    }

    #[test]
    fn linked_styles_use_style_fallback_nonces_and_intersecting_policies() {
        let same = location("https://example.test/screen.css");
        let other = location("https://other.test/screen.css");
        assert!(policy("default-src 'none'; style-src 'self'")
            .allows_external_style(1, None, &same)
            .is_ok());
        assert!(policy("default-src 'none'; script-src *")
            .allows_external_style(1, None, &same)
            .is_err());
        assert!(policy("style-src 'none'; style-src-elem 'self'")
            .allows_external_style(1, None, &same)
            .is_ok());
        assert!(policy("style-src *; style-src-elem 'none'")
            .allows_external_style(1, None, &same)
            .is_err());
        assert!(policy("style-src 'self'")
            .allows_external_style(1, None, &other)
            .is_err());
        assert!(policy("style-src 'nonce-One'; script-src 'nonce-Two'")
            .allows_external_style(1, Some("One"), &other)
            .is_ok());
        assert!(policy("style-src 'nonce-One'; script-src 'nonce-Two'")
            .allows_external_style(1, Some("Two"), &other)
            .is_err());
        assert!(policy("style-src 'self', style-src 'none'")
            .allows_external_style(1, None, &same)
            .is_err());
        assert!(policy("style-src 'none'; style-src *")
            .allows_external_style(1, None, &same)
            .is_err());
        let mut set = policy("style-src 'self'");
        set.add_meta("style-src 'none'", 10);
        assert!(set.allows_external_style(9, None, &same).is_ok());
        assert!(set.allows_external_style(usize::MAX, None, &same).is_err());
    }

    #[test]
    fn fonts_use_font_src_then_default_and_all_active_policies() {
        let same = location("https://example.test/font.woff");
        let other = location("https://other.test/font.woff");
        assert!(policy("default-src 'none'; font-src 'self'")
            .allows_font(1, &same)
            .is_ok());
        assert!(policy("default-src 'none'; script-src *")
            .allows_font(1, &same)
            .is_err());
        assert!(policy("font-src 'self'").allows_font(1, &other).is_err());
        assert!(policy("font-src 'none'; font-src *")
            .allows_font(1, &same)
            .is_err());
        assert!(policy("font-src *, font-src 'none'")
            .allows_font(1, &same)
            .is_err());
        assert!(policy("font-src 'nonce-known' 'unsafe-inline'")
            .allows_font(1, &same)
            .is_err());
        let mut set = policy("font-src 'self'");
        set.add_meta("font-src 'none'", 10);
        assert!(set.allows_font(9, &same).is_ok());
        assert!(set.allows_font(usize::MAX, &same).is_err());
    }

    #[test]
    fn nonce_strict_dynamic_and_inline_override_do_not_widen_permission() {
        let same = location("https://example.test/app.js");
        let p =
            policy("script-src 'unsafe-inline' 'self' https: 'nonce-Abc123==' 'strict-dynamic'");
        assert!(p.allows_inline(1, Some("Abc123=="), "1;").is_ok());
        assert!(p.allows_external(1, Some("Abc123=="), &same).is_ok());
        for nonce in [None, Some("abc123=="), Some(""), Some("Abc123")] {
            assert!(p.allows_inline(1, nonce, "1;").is_err());
            assert!(p.allows_external(1, nonce, &same).is_err());
        }
        assert!(p.allows_attribute("1;").is_err());
        let p = policy("script-src 'unsafe-inline' 'nonce-known'");
        assert!(p.allows_inline(1, None, "1;").is_err());
        assert!(p.allows_attribute("1;").is_err());
        let p = policy("script-src 'unsafe-inline'");
        assert!(p.allows_inline(1, None, "1;").is_ok());
        assert!(p.allows_attribute("1;").is_ok());
    }

    #[test]
    fn hashes_use_exact_utf8_and_attribute_hashes_require_explicit_opt_in() {
        // Generated independently with Python hashlib/base64.
        for token in [
            "sha256-9j7udBAIvztHvUOahWCwcmpWR+AEUK32hbWe3nEzLM0=",
            "sha384-73usZ3PI2NYBpqPFgXNKzwU6+X/fdEn+QklxZilttwtDlSffo3mHBjABBvo9w00/",
            "sha512-FcTSBIJidk5b2OOfuZtOliPnEQCJQwC9UNs7q7d0vZxYytLeCnIVDebFqvuxUBDB4ISpnkrBaq4mtCg7hAYq5Q==",
        ] {
            let p = policy(&format!("script-src '{token}' 'unsafe-inline'"));
            assert!(p.allows_inline(1, None, "var approved = 1;").is_ok());
            assert!(p.allows_inline(1, None, " var approved = 1;").is_err());
            assert!(p.allows_attribute("var approved = 1;").is_err());
            assert!(p.allows_external(1, None, &location("https://example.test/app.js")).is_err());
        }
        let p = policy("script-src 'sha256--s12hfqmKX2zUdJXooBaYfNEejxBtIw3UEUlxcya87A='");
        assert!(p.allows_inline(1, None, "var café = 1;").is_ok());
        let p = policy(
            "script-src-attr 'unsafe-hashes' 'sha256-2rvfFrggTCtyF5WOiTri1gDS8Boibj4Njn0e+VCBmDI='",
        );
        assert!(p.allows_attribute("return false;").is_ok());
        assert!(p.allows_attribute("return true;").is_err());
        let p = policy("script-src 'unsafe-inline' 'sha256-abc'");
        assert!(p.allows_inline(1, None, "1;").is_err());
    }

    #[test]
    fn multiple_headers_and_comma_policies_intersect() {
        let p = policy("script-src 'nonce-One', script-src 'nonce-Two'");
        assert!(p.allows_inline(1, Some("One"), "1;").is_err());
        assert!(p.allows_inline(1, Some("Two"), "1;").is_err());
        let mut p = policy("script-src 'nonce-One' 'unsafe-hashes' 'sha256-2rvfFrggTCtyF5WOiTri1gDS8Boibj4Njn0e+VCBmDI='");
        p.add_header("script-src 'none'");
        assert!(p.allows_inline(1, Some("One"), "1;").is_err());
        assert!(p.allows_attribute("return false;").is_err());
        let p = policy("script-src 'none' 'self'");
        assert!(p
            .allows_external(1, None, &location("https://example.test/app.js"))
            .is_ok());
    }

    #[test]
    fn meta_is_forward_only_but_later_handlers_use_all_installed_policies() {
        let mut p = policy("script-src 'unsafe-inline'");
        p.add_meta("script-src 'none'", 12);
        assert!(p.allows_inline(11, None, "1;").is_ok());
        assert!(p.allows_inline(13, None, "1;").is_err());
        assert!(p.allows_attribute("1;").is_err());
        let mut p = policy("script-src 'unsafe-inline'");
        p.add_meta("sandbox; script-src 'unsafe-inline'", 2);
        assert!(p.allows_inline(3, None, "1;").is_ok());
        let mut p = PolicySet::default();
        p.add_meta("script-src 'unsafe-inline', unknown-directive", 2);
        assert!(p.allows_inline(1, None, "1;").is_ok());
        assert!(p.allows_inline(3, None, "1;").is_err());
        let p = policy("sandbox allow-scripts; script-src 'unsafe-inline'");
        assert!(p
            .allows_inline(1, None, "1;")
            .unwrap_err()
            .contains("sandbox"));
        assert!(p.allows_attribute("1;").is_err());
    }

    #[test]
    fn url_sources_keep_scheme_host_and_port_boundaries() {
        let p = policy("script-src https://other.test:8443/");
        for value in [
            "https://other.test:8443/a.js",
            "https://OTHER.test:8443/a.js",
        ] {
            assert!(p.allows_external(1, None, &location(value)).is_ok());
        }
        for value in [
            "http://other.test:8443/a.js",
            "https://other.test/a.js",
            "https://other.test.evil:8443/a.js",
        ] {
            assert!(p.allows_external(1, None, &location(value)).is_err());
        }
        let p = policy("script-src http://other.test");
        assert!(p
            .allows_external(1, None, &location("https://other.test/a.js"))
            .is_ok());
        assert!(p
            .allows_external(1, None, &location("https://other.test:444/a.js"))
            .is_err());
        let p = policy("script-src https:");
        assert!(p
            .allows_external(1, None, &location("https://other.test/a.js"))
            .is_ok());
        assert!(p
            .allows_external(1, None, &location("http://other.test/a.js"))
            .is_err());
        let p = policy("script-src 'self'");
        assert!(p
            .allows_external(1, None, &location("https://example.test:444/a.js"))
            .is_err());
    }

    #[test]
    fn base_uri_uses_its_own_directive_without_default_fallback() {
        let p = policy("default-src 'none'");
        assert!(p.allows_base(1, &location("https://other.test/")).is_ok());
        let p = policy("script-src 'nonce-One'; base-uri 'none'");
        assert!(p
            .allows_base(1, &location("https://example.test/"))
            .is_err());
        let p = policy("base-uri 'self'");
        assert!(p
            .allows_base(1, &location("https://example.test/sub/"))
            .is_ok());
        assert!(p.allows_base(1, &location("https://other.test/")).is_err());
    }

    #[test]
    fn malformed_and_unsupported_sources_never_become_broader_permissions() {
        for source in [
            "",
            "'none'",
            "'nonce-'",
            "'nonce-invalid==='",
            "'nonce-invalid value'",
            "'sha256-'",
            "'sha256-a=b'",
            "'unknown'",
            "https://example.test/private/",
            "https://example.test/path",
            "https://example.test?query",
            "https://*.example.test",
            "example.test",
            "data:",
            "https://user@example.test",
            "https://example.test/#fragment",
        ] {
            let p = policy(&format!("script-src {source}"));
            assert!(
                p.allows_inline(1, Some("anything"), "1;").is_err(),
                "{source}"
            );
            assert!(
                p.allows_external(1, None, &location("https://example.test/app.js"))
                    .is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn parsing_limits_fail_closed_and_diagnostics_are_bounded() {
        for value in [
            format!("script-src {}", "'self' ".repeat(MAX_SOURCES + 1)),
            format!(
                "script-src 'nonce-{}' 'unsafe-inline'",
                "A".repeat(MAX_TOKEN_BYTES)
            ),
            "img-src 'self';".repeat(MAX_DIRECTIVES + 1),
            "script-src 'unsafe-inline',".repeat(MAX_POLICIES + 1),
            " ".repeat(MAX_POLICY_BYTES + 1),
        ] {
            let p = policy(&value);
            assert!(p.allows_inline(1, None, "1;").is_err());
            assert!(p.allows_attribute("1;").is_err());
            assert!(p.diagnostics.len() <= MAX_DIAGNOSTICS);
        }
        let mut p = policy("script-src 'unsafe-inline'");
        p.add_meta(&" ".repeat(MAX_POLICY_BYTES + 1), 10);
        assert!(p.allows_inline(9, None, "1;").is_ok());
        assert!(p.allows_inline(11, None, "1;").is_err());
        for _ in 0..100 {
            p.add_header(&" ".repeat(MAX_POLICY_BYTES + 1));
        }
        assert_eq!(p.diagnostics.len(), MAX_DIAGNOSTICS);
    }

    #[test]
    fn nonceability_retains_duplicate_attributes_and_rejects_dangling_markup() {
        let doc = crate::html::parse("<script nonce=One>1;</script><script nonce=One nonce=Two>2;</script><script nonce=One src=good.js SRC=bad.js>3;</script><script nonce=One x='&lt;SCRIPT evil'>4;</script><script nonce=One x='&lt;style'>5;</script><script nonce=One x='&lt;link'>6;</script>");
        let nodes: Vec<_> = doc
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                matches!(&n.kind, NodeKind::Element{tag,..} if tag=="script").then_some(i)
            })
            .collect();
        assert_eq!(nonce(&doc, nodes[0]), Some("One"));
        for &node in &nodes[1..] {
            assert_eq!(nonce(&doc, node), None);
        }
        assert_eq!(doc.attr(nodes[1], "nonce"), Some("One"));
        assert_eq!(doc.attr(nodes[2], "src"), Some("good.js"));
        assert_eq!(doc.nonce_unsafe_nodes.len(), 2);
    }
}
