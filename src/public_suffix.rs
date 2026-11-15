//! Original bounded PSL lookup over checked-in ICANN and PRIVATE rule data.
//!
//! Data provenance/license/freshness: data/public_suffix/README.md. No network,
//! runtime data-file lookup, IDNA service, or third-party implementation is used.
//! Inputs are canonical lowercase ASCII DNS names from our URL/cookie parser.
//! Rules follow https://github.com/publicsuffix/list/wiki/Format: longest match,
//! exception precedence, a one-label wildcard, and the implicit default '*'.
//! Wildcard bases themselves are also treated as public (a conservative
//! convention used by libpsl); this cannot grant a wider cookie scope.

include!("../data/public_suffix/rules.rs");

fn contains(table: &[&str], key: &str) -> bool {
    table.binary_search(&key).is_ok()
}

// Every step drops a label. DNS hosts are already bounded to 253 ASCII bytes;
// no allocation, recursion, or input-dependent unbounded table walk is needed.
fn suffix_start(host: &str) -> usize {
    let mut best = host.rfind('.').map_or(0, |at| at + 1);
    let mut start = 0;
    let mut previous = None;
    loop {
        let suffix = &host[start..];
        if contains(EXCEPTION, suffix) {
            // Exceptions remove their leftmost label from the public suffix.
            return start + suffix.find('.').map_or(0, |at| at + 1);
        }
        if contains(EXACT, suffix) {
            best = best.min(start);
        }
        if contains(WILDCARD, suffix) {
            // A bare wildcard base is not a registrable cookie parent. This
            // is deliberately stricter than the formal PSL label-only rule.
            best = best.min(previous.unwrap_or(start));
        }
        let Some(dot) = suffix.find('.') else {
            return best;
        };
        previous = Some(start);
        start += dot + 1;
    }
}

pub(crate) fn is_public_suffix(host: &str) -> bool {
    suffix_start(host) == 0
}

/// Return eTLD+1, or None when the host is itself a public suffix. Callers must
/// handle IP addresses separately; an IP is never treated as a DNS suffix.
pub(crate) fn registrable_domain(host: &str) -> Option<&str> {
    let suffix = suffix_start(host);
    if suffix == 0 {
        return None;
    }
    let start = host[..suffix - 1].rfind('.').map_or(0, |at| at + 1);
    Some(&host[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_tables_are_sorted_complete_and_unambiguous() {
        for table in [EXACT, WILDCARD, EXCEPTION] {
            assert!(table.windows(2).all(|w| w[0] < w[1]));
            assert!(table.iter().all(|s| !s.is_empty() && s.is_ascii()));
        }
        assert!(EXACT.len() > 9000);
        assert!(contains(EXACT, "com"));
        assert!(contains(EXACT, "co.uk"));
        assert!(contains(EXACT, "github.io"));
        assert!(contains(EXACT, "blogspot.com"));
        assert!(contains(WILDCARD, "ck"));
        assert!(contains(EXCEPTION, "www.ck"));
    }

    #[test]
    fn suffix_exact_private_and_unknown_rules_keep_site_boundaries() {
        for (host, expected) in [
            ("www.example.com", Some("example.com")),
            ("a.b.example.co.uk", Some("example.co.uk")),
            ("a.user.github.io", Some("user.github.io")),
            ("a.user.blogspot.com", Some("user.blogspot.com")),
            ("a.b.unknown-suffix", Some("b.unknown-suffix")),
            ("localhost", None),
            ("com", None),
            ("co.uk", None),
            ("github.io", None),
        ] {
            assert_eq!(registrable_domain(host), expected, "{host}");
            assert_eq!(is_public_suffix(host), expected.is_none(), "{host}");
        }
    }

    #[test]
    fn suffix_wildcards_and_exceptions_obey_longest_matching_rule() {
        for (host, expected) in [
            ("ck", None),
            ("a.ck", None),
            ("b.a.ck", Some("b.a.ck")),
            ("www.ck", Some("www.ck")),
            ("a.www.ck", Some("www.ck")),
            ("kawasaki.jp", None),
            ("a.kawasaki.jp", None),
            ("city.kawasaki.jp", Some("city.kawasaki.jp")),
            ("a.city.kawasaki.jp", Some("city.kawasaki.jp")),
        ] {
            assert_eq!(registrable_domain(host), expected, "{host}");
        }
    }

    #[test]
    fn suffix_uses_punycode_rules_and_handles_maximum_label_depth() {
        assert!(is_public_suffix("xn--p1ai"));
        assert_eq!(registrable_domain("a.xn--p1ai"), Some("a.xn--p1ai"));
        let host = format!("{}example.com", "a.".repeat(120));
        assert!(host.len() <= 253);
        assert_eq!(registrable_domain(&host), Some("example.com"));
    }
}
