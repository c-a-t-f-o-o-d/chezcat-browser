//! Browser-owned, local request rules. No list downloads, page-owned rules or
//! remote decisions. Allow rules only exempt this filter; they never grant an
//! origin, TLS, CSP or other capability denied elsewhere in the loader.

use crate::url::{self, HttpUrl};
use std::collections::HashSet;

pub const MAX_RULES: usize = 256;
pub const MAX_RULE_BYTES: usize = 1024;
pub const MAX_TOTAL_BYTES: usize = 64 * 1024;
// Small original starter list, not a copy of an external filtering database.
// Google's published tagging endpoints identify these advertising/analytics
// hosts: https://developers.google.com/tag-platform/learn/sst-fundamentals/6-testing
pub const BUILTIN_HOSTS: &[&str] = &[
    "doubleclick.net",
    "googleadservices.com",
    "googlesyndication.com",
    "google-analytics.com",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub builtins: bool,
    pub rules: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            builtins: true,
            rules: Vec::new(),
        }
    }
}

impl Config {
    pub fn normalize(&mut self) -> Result<(), String> {
        if self.rules.len() > MAX_RULES {
            return Err(format!(
                "At most {MAX_RULES} request-blocking rules are allowed."
            ));
        }
        let mut bytes = 0usize;
        let mut seen = HashSet::new();
        for text in &mut self.rules {
            let rule = Rule::parse(text)?;
            *text = rule.label;
            bytes += text.len();
            if bytes > MAX_TOTAL_BYTES {
                return Err("Request-blocking rules exceed 64 KiB.".into());
            }
            if !seen.insert(text.clone()) {
                return Err(format!("Duplicate request-blocking rule: {text}"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Matcher {
    Host(String),
    Path(String),
    Url(HttpUrl),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    allow: bool,
    label: String,
    matcher: Matcher,
}

pub fn normalize_rule(text: &str) -> Result<String, String> {
    Rule::parse(text).map(|rule| rule.label)
}

impl Rule {
    fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_RULE_BYTES {
            return Err(format!(
                "A request rule is limited to {MAX_RULE_BYTES} bytes."
            ));
        }
        let text = text.trim();
        let (allow, text) = text.strip_prefix("@@").map_or((false, text), |s| (true, s));
        if text.is_empty()
            || !text.is_ascii()
            || text
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(
                "Use a host, path:/prefix or url:https://host/prefix; prefix @@ to allow.".into(),
            );
        }
        let (matcher, spelling) = if let Some(path) = text.strip_prefix("path:") {
            if !path.starts_with('/') || path.contains(['?', '#', '*', '\\']) {
                return Err(
                    "A path rule must start with / and contain no query, fragment or wildcard."
                        .into(),
                );
            }
            let (parsed, _) = url::parse(&format!("https://rule.invalid{path}"))?;
            let path = match_path(&parsed.path);
            (Matcher::Path(path.clone()), format!("path:{path}"))
        } else if let Some(address) = text.strip_prefix("url:") {
            if address.contains(['#', '*']) {
                return Err("A URL-prefix rule cannot contain a fragment or wildcard.".into());
            }
            let (mut url, _) = url::parse(address)?;
            url.path = match_path(&url.path);
            let spelling = format!("url:{}", url.display());
            (Matcher::Url(url), spelling)
        } else {
            let text = text.strip_prefix("host:").unwrap_or(text);
            let text = if let Some(text) = text.strip_prefix("||") {
                text.strip_suffix('^')
                    .ok_or("A ||host rule must end with ^.")?
            } else {
                text
            };
            if text.contains(['/', '?', '#', '*', '@']) {
                return Err(
                    "A host rule must contain only a hostname. Use path: or url: for prefixes."
                        .into(),
                );
            }
            let (url, _) = url::parse(&format!("https://{text}/"))?;
            if url.host_header().trim_end_matches('.')
                != text.to_ascii_lowercase().trim_end_matches('.')
                || url.port != 443
            {
                return Err(
                    "Host rules cannot include ports; use a URL-prefix rule instead.".into(),
                );
            }
            let host = url.host.trim_end_matches('.').to_owned();
            let label = if host.contains(':') {
                format!("host:[{host}]")
            } else {
                format!("host:{host}")
            };
            (Matcher::Host(host), label)
        };
        let label = if allow {
            format!("@@{spelling}")
        } else {
            spelling
        };
        if label.len() > MAX_RULE_BYTES {
            return Err(format!(
                "The normalized rule exceeds {MAX_RULE_BYTES} bytes."
            ));
        }
        Ok(Self {
            allow,
            label,
            matcher,
        })
    }

    fn matches(&self, url: &HttpUrl, path: &str) -> bool {
        match &self.matcher {
            Matcher::Host(host) => host_matches(host, &url.host),
            Matcher::Path(prefix) => path.starts_with(prefix),
            Matcher::Url(prefix) => {
                if !prefix.same_origin(url) || !path.starts_with(&prefix.path) {
                    return false;
                }
                match &prefix.query {
                    None => true,
                    Some(query) => {
                        path == prefix.path
                            && url
                                .query
                                .as_ref()
                                .is_some_and(|actual| actual.starts_with(query))
                    }
                }
            }
        }
    }
}

fn host_matches(rule: &str, host: &str) -> bool {
    // A terminal DNS root dot must not bypass a host-family rule. This is
    // filter matching only: URL origin identity and the actual request stay as
    // parsed, and URL-prefix rules retain their exact-origin semantics.
    let host = host.trim_end_matches('.');
    host == rule
        || host
            .strip_suffix(rule)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

// Decode only unreserved ASCII for matching. Encoded separators stay encoded;
// this does not change the actual address sent to the server.
fn match_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut output = String::with_capacity(path.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let a = (bytes[i + 1] as char).to_digit(16);
            let b = (bytes[i + 2] as char).to_digit(16);
            if let (Some(a), Some(b)) = (a, b) {
                let value = (a * 16 + b) as u8;
                if value.is_ascii_alphanumeric() || matches!(value, b'-' | b'.' | b'_' | b'~') {
                    output.push(value as char);
                } else {
                    const HEX: &[u8] = b"0123456789ABCDEF";
                    output.push('%');
                    output.push(HEX[(value >> 4) as usize] as char);
                    output.push(HEX[(value & 15) as usize] as char);
                }
                i += 3;
                continue;
            }
        }
        output.push(bytes[i] as char);
        i += 1;
    }
    output
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    enabled: bool,
    builtins: bool,
    rules: Vec<Rule>,
    invalid: Option<String>,
}

impl Default for Policy {
    fn default() -> Self {
        Self::new(&Config::default())
    }
}

impl Policy {
    pub fn new(config: &Config) -> Self {
        let mut normalized = config.clone();
        let invalid = normalized.normalize().err();
        let rules = if invalid.is_none() {
            normalized
                .rules
                .iter()
                .map(|rule| Rule::parse(rule).expect("validated rule"))
                .collect()
        } else {
            Vec::new()
        };
        Self {
            enabled: config.enabled,
            builtins: config.builtins,
            rules,
            invalid,
        }
    }

    pub fn description(&self) -> String {
        if !self.enabled {
            return "Request blocking is disabled.".into();
        }
        format!("Request blocking: {} starter host rules; {} custom rule(s), including allow exceptions. Rules are local; no automatic list downloads.",
            if self.builtins { BUILTIN_HOSTS.len() } else { 0 }, self.rules.len())
    }

    pub fn check(&self, url: &HttpUrl) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(error) = &self.invalid {
            return Err(format!("Invalid request-blocking policy: {error}"));
        }
        let path = match_path(&url.path);
        if self
            .rules
            .iter()
            .any(|rule| rule.allow && rule.matches(url, &path))
        {
            return Ok(());
        }
        if let Some(rule) = self
            .rules
            .iter()
            .find(|rule| !rule.allow && rule.matches(url, &path))
        {
            return Err(format!("Blocked by request rule: {}", rule.label));
        }
        if self.builtins {
            if let Some(host) = BUILTIN_HOSTS
                .iter()
                .find(|host| host_matches(host, &url.host))
            {
                return Err(format!("Blocked by starter request rule: host:{host}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn web(value: &str) -> HttpUrl {
        url::parse(value).unwrap().0
    }
    fn policy(rules: &[&str]) -> Policy {
        Policy::new(&Config {
            enabled: true,
            builtins: true,
            rules: rules.iter().map(|s| (*s).into()).collect(),
        })
    }

    #[test]
    fn host_rules_are_boundary_aware_and_allow_rules_win() {
        let rules = policy(&["Ads.Example.com", "@@cdn.ads.example.com"]);
        assert!(rules.check(&web("https://ads.example.com/a")).is_err());
        assert!(rules.check(&web("https://ads.example.com./a")).is_err());
        assert!(rules.check(&web("https://x.ads.example.com/a")).is_err());
        assert!(rules.check(&web("https://x.ads.example.com./a")).is_err());
        assert!(rules.check(&web("https://notads.example.com/a")).is_ok());
        assert!(rules
            .check(&web("https://ads.example.com.evil.test/a"))
            .is_ok());
        assert!(rules.check(&web("https://cdn.ads.example.com/a")).is_ok());
        assert!(rules.check(&web("https://cdn.ads.example.com./a")).is_ok());
        assert!(rules
            .check(&web("https://googleads.g.doubleclick.net/a"))
            .is_err());
        assert!(rules
            .check(&web("https://googleads.g.doubleclick.net./a"))
            .is_err());
        assert!(Policy::default()
            .check(&web("https://googleads.g.doubleclick.net/a"))
            .is_ok());
        assert!(policy(&["@@doubleclick.net"])
            .check(&web("https://googleads.g.doubleclick.net/a"))
            .is_ok());
    }

    #[test]
    fn path_url_prefixes_and_encoded_unreserved_characters() {
        let rules = policy(&[
            "path:/ads/",
            "@@path:/ads/allowed/",
            "url:https://example.com/pixel?event=ad",
        ]);
        assert!(rules
            .check(&web("https://one.example/%61ds/banner.png"))
            .is_err());
        assert!(rules
            .check(&web("https://one.example/ads/allowed/photo.png"))
            .is_ok());
        assert!(rules
            .check(&web("https://example.com/pixel?event=ads&x=1"))
            .is_err());
        assert!(rules
            .check(&web("https://other.example/pixel?event=ad"))
            .is_ok());
        assert!(rules
            .check(&web("http://example.com/pixel?event=ad"))
            .is_ok());
        assert!(rules
            .check(&web("https://example.com/pixel2?event=ad"))
            .is_ok());
        assert!(rules
            .check(&web("https://example.com/pixel?event=ok"))
            .is_ok());
    }

    #[test]
    fn invalid_duplicate_and_oversized_rules_are_rejected() {
        for value in [
            "",
            "ads.example:8080",
            "*.example.com",
            "path:ads",
            "path:/x*",
            "url:https://a.test/#x",
            "example.com##div",
            "@@",
            "host:a.test/a",
            "path:/a b",
        ] {
            assert!(normalize_rule(value).is_err(), "{value}");
        }
        assert_eq!(
            normalize_rule("||ADS.example^ ").unwrap(),
            "host:ads.example"
        );
        assert_eq!(
            normalize_rule("||ADS.example.^ ").unwrap(),
            "host:ads.example"
        );
        assert_eq!(normalize_rule("host:[::1]").unwrap(), "host:[::1]");
        let mut config = Config {
            rules: vec!["a.test".into(), "host:a.test".into()],
            ..Config::default()
        };
        assert!(config.normalize().unwrap_err().contains("Duplicate"));
        config.rules = vec!["a.test".into(); MAX_RULES + 1];
        assert!(config.normalize().is_err());
        assert!(normalize_rule(&format!("path:/{}", "x".repeat(MAX_RULE_BYTES))).is_err());
        let invalid = policy(&["path:/x*"]);
        assert!(invalid.check(&web("https://a.test/")).is_err());
    }
}
