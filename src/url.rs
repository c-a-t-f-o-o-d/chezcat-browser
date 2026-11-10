//! A bounded HTTP(S) URL subset, written for this browser.
//!
//! The WHATWG URL Standard and RFC 3986 define the syntax and resolution rules
//! used here. This intentionally rejects credentials, IDNA names, legacy numeric
//! IPv4 forms, raw controls/backslashes, and malformed escapes. Encoded controls
//! and backslashes are also rejected in paths and fragments; query escapes stay
//! encoded data so form values can round-trip without entering request headers.
//! It does not attempt the error recovery of a general-purpose browser parser.

use std::net::{Ipv4Addr, Ipv6Addr};

const MAX_URL_BYTES: usize = 16 * 1024;
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Parser-produced URLs have an ASCII canonical host, a leading-slash ASCII
/// path, and an optional ASCII query. Escapes remain encoded for the HTTP wire.
/// IPv6 hosts have no brackets here; serialization restores those brackets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HttpUrl {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub query: Option<String>,
}

/// Parse an explicit HTTP(S) URL. The fragment is UTF-8 decoded separately and
/// never appears in an HTTP request target. A literal '+' is not a space.
pub fn parse(input: &str) -> Result<(HttpUrl, String), String> {
    let input = checked_input(input)?;
    let (scheme, rest) = input
        .split_once("://")
        .ok_or_else(|| "Use an explicit http:// or https:// address.".to_owned())?;
    let secure = if scheme.eq_ignore_ascii_case("https") {
        true
    } else if scheme.eq_ignore_ascii_case("http") {
        false
    } else {
        return Err("Only http:// and https:// web addresses are supported.".to_owned());
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (host, port) = authority(&rest[..authority_end], secure)?;
    let (path, query, fragment) = reference_parts(&rest[authority_end..])?;
    let path = if path.is_empty() {
        "/".to_owned()
    } else {
        normalize_path(&encode_component(path, false)?)
    };
    let url = HttpUrl {
        secure,
        host,
        port,
        path,
        query: query
            .map(|query| encode_component(query, true))
            .transpose()?,
    };
    check_serialized_size(&url, &fragment)?;
    Ok((url, fragment))
}

impl HttpUrl {
    pub fn display(&self) -> String {
        format!(
            "{}://{}{}",
            if self.secure { "https" } else { "http" },
            self.host_header(),
            self.request_target()
        )
    }

    /// Resolve a web reference without ever interpreting it as a local path.
    /// An omitted fragment returns an empty fragment; a redirect caller may
    /// separately implement HTTP fragment inheritance when Location lacks '#'.
    pub fn resolve(&self, input: &str) -> Result<(HttpUrl, String), String> {
        let input = checked_input(input)?;
        if input.starts_with("//") {
            return parse(&format!(
                "{}:{input}",
                if self.secure { "https" } else { "http" }
            ));
        }
        // A colon in the first segment identifies a scheme, even if that
        // scheme is unsupported. A file name with ':' can use './name:part'.
        let first_segment = input.split(['/', '?', '#']).next().unwrap_or("");
        if first_segment.contains(':') {
            return parse(input);
        }
        let (path, query, fragment) = reference_parts(input)?;
        let mut url = self.clone();
        if path.is_empty() {
            if let Some(query) = query {
                url.query = Some(encode_component(query, true)?);
            }
        } else {
            let encoded = encode_component(path, false)?;
            let combined = if encoded.starts_with('/') {
                encoded
            } else {
                let directory_end = self.path.rfind('/').map_or(0, |index| index + 1);
                format!("{}{encoded}", &self.path[..directory_end])
            };
            url.path = normalize_path(&combined);
            url.query = query
                .map(|query| encode_component(query, true))
                .transpose()?;
        }
        check_serialized_size(&url, &fragment)?;
        Ok((url, fragment))
    }

    pub fn request_target(&self) -> String {
        match &self.query {
            Some(query) => format!("{}?{query}", self.path),
            None => self.path.clone(),
        }
    }

    pub fn host_header(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == default_port(self.secure) {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }

    pub fn same_origin(&self, other: &Self) -> bool {
        self.secure == other.secure
            && self.port == other.port
            && self.host.eq_ignore_ascii_case(&other.host)
    }
}

fn checked_input(input: &str) -> Result<&str, String> {
    if input.len() > MAX_URL_BYTES {
        return Err(format!(
            "Web addresses are limited to {MAX_URL_BYTES} bytes."
        ));
    }
    if input.chars().any(char::is_control) || input.contains('\\') {
        return Err("Web addresses cannot contain controls or backslashes.".to_owned());
    }
    Ok(input.trim_matches(' '))
}

fn check_serialized_size(url: &HttpUrl, fragment: &str) -> Result<(), String> {
    // A fragment can expand to at most three ASCII bytes per UTF-8 byte.
    // Bound the actual encoded size without allocating an extra full string.
    let fragment_size = fragment.bytes().fold(0usize, |total, byte| {
        total + if unreserved(byte) { 1 } else { 3 }
    });
    if url.display().len() + usize::from(!fragment.is_empty()) + fragment_size > MAX_URL_BYTES {
        return Err(format!(
            "Web addresses are limited to {MAX_URL_BYTES} encoded bytes."
        ));
    }
    Ok(())
}

fn default_port(secure: bool) -> u16 {
    if secure {
        443
    } else {
        80
    }
}

fn authority(input: &str, secure: bool) -> Result<(String, u16), String> {
    if input.is_empty() || input.contains('@') {
        return Err("A web address needs a host and cannot contain credentials.".to_owned());
    }
    let (host, port) = if let Some(rest) = input.strip_prefix('[') {
        let close = rest
            .find(']')
            .ok_or_else(|| "An IPv6 host needs closing brackets.".to_owned())?;
        let host = rest[..close]
            .parse::<Ipv6Addr>()
            .map_err(|_| {
                "Use a valid bracketed IPv6 address without a zone identifier.".to_owned()
            })?
            .to_string();
        let suffix = &rest[close + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            Some(
                suffix
                    .strip_prefix(':')
                    .ok_or_else(|| "Unexpected text after the IPv6 host.".to_owned())?,
            )
        };
        (host, port)
    } else {
        let (host, port) = match input.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (input, None),
        };
        (dns_or_ipv4(host)?, port)
    };
    let port = match port {
        None => default_port(secure),
        Some(port) if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) => {
            let port = port
                .parse::<u16>()
                .map_err(|_| "The port must be between 1 and 65535.".to_owned())?;
            if port == 0 {
                return Err("The port must be between 1 and 65535.".to_owned());
            }
            port
        }
        Some(_) => return Err("A port must contain decimal digits.".to_owned()),
    };
    Ok((host, port))
}

fn dns_or_ipv4(input: &str) -> Result<String, String> {
    if input.is_empty() || !input.is_ascii() {
        return Err("Use an ASCII DNS name, IPv4 address, or bracketed IPv6 address.".to_owned());
    }
    let name = input.strip_suffix('.').unwrap_or(input);
    if name.is_empty() || name.len() > 253 {
        return Err("The DNS host name is empty or too long.".to_owned());
    }
    for label in name.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("The host contains an invalid DNS label.".to_owned());
        }
    }
    let last = name.rsplit('.').next().unwrap_or("");
    let numeric_last = last.bytes().all(|byte| byte.is_ascii_digit())
        || last
            .strip_prefix("0x")
            .or_else(|| last.strip_prefix("0X"))
            .is_some_and(|digits| digits.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if numeric_last {
        // Avoid resolver-dependent shorthand, octal and hexadecimal IPv4 forms.
        return name
            .parse::<Ipv4Addr>()
            .map(|address| address.to_string())
            .map_err(|_| {
                "Use a four-part decimal IPv4 address; legacy numeric forms are unsupported."
                    .to_owned()
            });
    }
    Ok(input.to_ascii_lowercase())
}

fn reference_parts(input: &str) -> Result<(&str, Option<&str>, String), String> {
    let (before_fragment, fragment) = input.split_once('#').unwrap_or((input, ""));
    let (path, query) = match before_fragment.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (before_fragment, None),
    };
    Ok((path, query, decode_fragment(fragment)?))
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn escape_byte(bytes: &[u8], index: usize, query_data: bool) -> Result<u8, String> {
    let high = bytes.get(index + 1).and_then(|byte| hex(*byte));
    let low = bytes.get(index + 2).and_then(|byte| hex(*byte));
    let (Some(high), Some(low)) = (high, low) else {
        return Err("A percent escape must contain two hexadecimal digits.".to_owned());
    };
    let byte = high * 16 + low;
    if !query_data && (byte.is_ascii_control() || byte == b'\\') {
        return Err(
            "Encoded controls and backslashes are unsupported in URL paths and fragments."
                .to_owned(),
        );
    }
    Ok(byte)
}

fn unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn encode_component(input: &str, query: bool) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut encoded = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            // Query escapes are data, including encoded line breaks from HTML
            // forms. Preserve the escape; never decode it onto the HTTP wire.
            let byte = escape_byte(bytes, index, query)?;
            append_escape(&mut encoded, byte);
            index += 3;
            continue;
        }
        if unreserved(byte)
            || matches!(
                byte,
                b'!' | b'$'
                    | b'&'
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
                    | b'/'
            )
            || byte == b'\'' && !query
            || byte == b'?' && query
        {
            encoded.push(char::from(byte));
        } else {
            append_escape(&mut encoded, byte);
        }
        index += 1;
    }
    Ok(encoded)
}

fn append_escape(output: &mut String, byte: u8) {
    output.push('%');
    output.push(char::from(HEX[usize::from(byte >> 4)]));
    output.push(char::from(HEX[usize::from(byte & 15)]));
}

fn decode_fragment(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            decoded.push(escape_byte(bytes, index, false)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let fragment = String::from_utf8(decoded)
        .map_err(|_| "A URL fragment must contain valid UTF-8 text.".to_owned())?;
    if fragment.chars().any(char::is_control) {
        return Err("A URL fragment cannot contain control characters.".to_owned());
    }
    Ok(fragment)
}

fn normalize_path(input: &str) -> String {
    let mut segments = Vec::new();
    let mut remaining = input
        .strip_prefix('/')
        .unwrap_or(input)
        .split('/')
        .peekable();
    while let Some(segment) = remaining.next() {
        match segment {
            "." | "%2E" => {
                if remaining.peek().is_none() {
                    segments.push("");
                }
            }
            ".." | ".%2E" | "%2E." | "%2E%2E" => {
                segments.pop();
                if remaining.peek().is_none() {
                    segments.push("");
                }
            }
            _ => segments.push(segment),
        }
    }
    format!("/{}", segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(input: &str) -> HttpUrl {
        parse(input).unwrap().0
    }

    #[test]
    fn explicit_addresses_canonicalize_authority_and_keep_wire_components_separate() {
        let (value, fragment) =
            parse("HTTP://Example.COM:80/café a?q=雪+x&v=%2f#chap%20two+三").unwrap();
        assert_eq!(value.host, "example.com");
        assert_eq!(value.port, 80);
        assert_eq!(value.path, "/caf%C3%A9%20a");
        assert_eq!(value.query.as_deref(), Some("q=%E9%9B%AA+x&v=%2F"));
        assert_eq!(value.host_header(), "example.com");
        assert_eq!(value.request_target(), "/caf%C3%A9%20a?q=%E9%9B%AA+x&v=%2F");
        assert_eq!(fragment, "chap two+三");
        assert_eq!(parse(&value.display()).unwrap().0, value);
        assert_eq!(url("https://example.com").display(), "https://example.com/");
        assert_eq!(url("http://example.com?").request_target(), "/?");
    }

    #[test]
    fn literal_addresses_and_ports_have_unambiguous_origin_forms() {
        assert_eq!(url("http://127.0.0.1:8765").host_header(), "127.0.0.1:8765");
        let ipv6 = url("https://[2001:0DB8:0:0:0:0:0:1]:443/x");
        assert_eq!(ipv6.host, "2001:db8::1");
        assert_eq!(ipv6.host_header(), "[2001:db8::1]");
        assert_eq!(url("http://[::1]:8765").host_header(), "[::1]:8765");
        assert!(url("http://EXAMPLE.com:80/a").same_origin(&url("http://example.com/b?q=1")));
        for other in [
            "https://example.com/",
            "http://example.com:81/",
            "http://other.example.com/",
            "http://example.com./",
        ] {
            assert!(
                !url("http://example.com/").same_origin(&url(other)),
                "{other}"
            );
        }
    }

    #[test]
    fn relative_resolution_merges_queries_fragments_and_directories() {
        let base = url("http://example.com/a/b/page?old=1");
        for (reference, expected, fragment) in [
            ("", "http://example.com/a/b/page?old=1", ""),
            ("#one%20two", "http://example.com/a/b/page?old=1", "one two"),
            ("?new=2#x", "http://example.com/a/b/page?new=2", "x"),
            ("?", "http://example.com/a/b/page?", ""),
            ("child", "http://example.com/a/b/child", ""),
            ("../other?x=1", "http://example.com/a/other?x=1", ""),
            ("/root", "http://example.com/root", ""),
            ("//OTHER.test:8080/path", "http://other.test:8080/path", ""),
            ("https://other.test/a", "https://other.test/a", ""),
            ("./item:part", "http://example.com/a/b/item:part", ""),
        ] {
            let (resolved, actual_fragment) = base.resolve(reference).unwrap();
            assert_eq!(resolved.display(), expected, "{reference}");
            assert_eq!(actual_fragment, fragment, "{reference}");
        }
    }

    #[test]
    fn dot_segments_include_encoded_dots_but_do_not_decode_slashes_twice() {
        let base = url("http://example.com/a/b/page");
        for (reference, path) in [
            ("../../../../x", "/x"),
            ("%2e%2e/x", "/a/x"),
            (".%2e/x", "/a/x"),
            ("%2E./x", "/a/x"),
            ("./", "/a/b/"),
            ("..", "/a/"),
            ("/a//.", "/a//"),
            ("/a//../b", "/a/b"),
            ("/a/%252e%252e/x", "/a/%252e%252e/x"),
            ("/a%2fb/../x", "/x"),
            ("/a/%2E%2E/", "/"),
        ] {
            assert_eq!(base.resolve(reference).unwrap().0.path, path, "{reference}");
        }
        assert_eq!(url("http://example.com/a/../b/.").path, "/b/");
    }

    #[test]
    fn malformed_or_ambiguous_hosts_and_unsupported_schemes_are_rejected() {
        for input in [
            "example.com",
            "http:example.com",
            "http:///x",
            "http://",
            "https://user:pass@example.com/",
            "http://user@example.com/",
            "http://éxample.com/",
            "http://%65xample.com/",
            "http://a..b/",
            "http://-a.test/",
            "http://a-.test/",
            "http://a_b.test/",
            "http://a:0/",
            "http://a:65536/",
            "http://a:/",
            "http://a:+80/",
            "http://a:80:90/",
            "http://::1/",
            "http://[::1]extra/",
            "http://[::1",
            "http://[fe80::1%25eth0]/",
            "http://127.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://0177.0.0.1/",
            "http://127.0.0.999/",
            "http://a.123/",
            "file:///tmp/a.html",
            "javascript:alert(1)",
            "data:text/html,test",
            "about:home",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
        let base = url("http://example.com/");
        for input in [
            "file:///tmp/a",
            "about:home",
            "javascript:alert(1)",
            "C:/private/a",
            "mailto:user@example.com",
            "///tmp/a",
        ] {
            assert!(base.resolve(input).is_err(), "{input}");
        }
    }

    #[test]
    fn controls_escaped_controls_and_backslashes_cannot_reach_request_headers() {
        for tail in [
            "/a\r\nX-Test:1",
            "/a\t",
            "/%0d%0aX-Test:1",
            "/%00",
            "/%7f",
            "/%5csecret",
            "/a\\b",
            "/%",
            "/%2",
            "/%xz",
            "/?q=a\r\nX-Test:1",
            "/?q=a\\b",
            "/#%0D",
            "/#%FF",
        ] {
            let input = format!("http://example.com{tail}");
            assert!(parse(&input).is_err(), "{input:?}");
            assert!(
                url("http://example.com/").resolve(tail).is_err(),
                "{tail:?}"
            );
        }
        assert!(parse("\nhttp://example.com/").is_err());
        assert!(parse("http://exam\tple.com/").is_err());
        assert_eq!(
            url("http://example.com/%FF?q=%23%3F").request_target(),
            "/%FF?q=%23%3F"
        );
    }

    #[test]
    fn escaped_query_data_round_trips_without_creating_wire_controls() {
        let input = "http://example.test/search?q=C%3a%5cfolder&lines=a%0d%0ab&tab=x%09y&zero=%00";
        let value = url(input);
        let target = "/search?q=C%3A%5Cfolder&lines=a%0D%0Ab&tab=x%09y&zero=%00";
        assert_eq!(value.request_target(), target);
        assert_eq!(url(&value.display()), value);
        assert!(value
            .request_target()
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte)));
        let inherited = value.resolve("#chapter").unwrap().0;
        assert_eq!(inherited.request_target(), target);
        let changed = value.resolve("next?value=%5c%0d%0a%09").unwrap().0;
        assert_eq!(changed.request_target(), "/next?value=%5C%0D%0A%09");
        let injection = url("http://example.test/?q=%0D%0AX-Injected%3A+yes");
        assert!(!injection.request_target().contains(['\r', '\n', '\\']));
        for invalid in [
            "http://example.test/%0D?q=ok",
            "http://example.test/%5C?q=ok",
            "http://example.test/?q=ok#%0A",
            "http://example.test/?q=ok#%5C",
            "http://example.test/?q=raw\r\nX-Injected:yes",
            "http://example.test/?q=raw\\value",
        ] {
            assert!(parse(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn bounds_apply_to_input_and_to_expanded_serialization() {
        assert!(parse(&format!("http://example.com/{}", "a".repeat(MAX_URL_BYTES))).is_err());
        assert!(parse(&format!("http://example.com/{}", "雪".repeat(2_000))).is_err());
        let base = url(&format!("http://example.com/{}/page", "a".repeat(9_000)));
        assert!(base.resolve(&"b".repeat(9_000)).is_err());
        assert!(parse(&format!("http://{}.test", "a".repeat(64))).is_err());
    }
}
