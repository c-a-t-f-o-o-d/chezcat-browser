//! Original bounded, local URL and URLSearchParams values.
//! This module implements the HTTP(S) part of the WHATWG URL API. It neither
//! fetches resources nor changes the browser's stricter network URL parser.
//! Non-HTTP(S) schemes and IDNA conversion remain explicit capability failures.
//! Source: https://url.spec.whatwg.org/#api (algorithms, not engine source).

use super::{
    Accessors, Builtin, Declaration, Fault, Host, Ordinary, Reference, Runtime, Value,
    MAX_CALL_DEPTH,
};
use std::net::Ipv6Addr;

const MAX_URL_BYTES: usize = 16 * 1024;
const MAX_PARAMS_BYTES: usize = 64 * 1024;
const MAX_PAIRS: usize = 1024;
const HEX: &[u8; 16] = b"0123456789ABCDEF";
type Pair = (String, String);

#[derive(Clone)]
pub(super) struct Url {
    secure: bool,
    username: String,
    password: String,
    hostname: String,
    port: Option<u16>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}
impl Url {
    fn bytes(&self) -> usize {
        self.username.len()
            + self.password.len()
            + self.hostname.len()
            + self.path.len()
            + self.query.as_ref().map_or(0, String::len)
            + self.fragment.as_ref().map_or(0, String::len)
    }
    fn protocol(&self) -> &'static str {
        if self.secure {
            "https:"
        } else {
            "http:"
        }
    }
    fn default_port(&self) -> u16 {
        if self.secure {
            443
        } else {
            80
        }
    }
    fn host(&self) -> String {
        match self.port {
            Some(p) => format!("{}:{p}", self.hostname),
            None => self.hostname.clone(),
        }
    }
    fn origin(&self) -> String {
        format!("{}//{}", self.protocol(), self.host())
    }
    fn serialized_len(&self) -> usize {
        self.protocol().len()
            + 2
            + self.hostname.len()
            + self.port.map_or(0, |p| p.to_string().len() + 1)
            + if self.username.is_empty() && self.password.is_empty() {
                0
            } else {
                self.username.len()
                    + 1
                    + if self.password.is_empty() {
                        0
                    } else {
                        self.password.len() + 1
                    }
            }
            + self.path.len()
            + self.query.as_ref().map_or(0, |s| s.len() + 1)
            + self.fragment.as_ref().map_or(0, |s| s.len() + 1)
    }
    fn href(&self) -> String {
        let mut text = String::with_capacity(self.serialized_len());
        text.push_str(self.protocol());
        text.push_str("//");
        if !self.username.is_empty() || !self.password.is_empty() {
            text.push_str(&self.username);
            if !self.password.is_empty() {
                text.push(':');
                text.push_str(&self.password);
            }
            text.push('@');
        }
        text.push_str(&self.host());
        text.push_str(&self.path);
        if let Some(query) = &self.query {
            text.push('?');
            text.push_str(query);
        }
        if let Some(fragment) = &self.fragment {
            text.push('#');
            text.push_str(fragment);
        }
        text
    }
    fn check(&self) -> Result<(), ParseError> {
        if self.serialized_len() > MAX_URL_BYTES {
            Err(ParseError::Limit("URL length limit reached (16 KiB)."))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Url,
    Params,
}
#[derive(Clone, Copy)]
pub(super) enum IterKind {
    Keys,
    Values,
    Entries,
}
pub(super) enum Data {
    Url {
        value: Url,
        params: usize,
    },
    Params {
        pairs: Vec<Pair>,
        owner: Option<usize>,
    },
    Iterator {
        source: Option<usize>,
        next: usize,
        kind: IterKind,
    },
}
impl Data {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        use super::function_reachability::Edge;
        match self {
            Self::Url { params, .. } => add(Edge::Object(*params)),
            Self::Params { owner, .. } | Self::Iterator { source: owner, .. } => { if let Some(id) = owner { add(Edge::Object(*id)); } }
        }
    }

    pub(super) fn tag(&self) -> &'static str {
        match self {
            Self::Url { .. } => "URL",
            Self::Params { .. } => "URLSearchParams",
            Self::Iterator { .. } => "URLSearchParams Iterator",
        }
    }
    fn bytes(&self) -> usize {
        match self {
            Self::Url { value, .. } => value.bytes(),
            Self::Params { pairs, .. } => pair_storage(pairs),
            Self::Iterator { .. } => 0,
        }
    }
}
#[derive(Default)]
pub(super) struct Roots {
    url: Option<usize>,
    params: Option<usize>,
    iterator: Option<usize>,
}

impl Roots {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        for id in [self.url, self.params, self.iterator].into_iter().flatten() { add(super::function_reachability::Edge::Object(id)); }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor(Kind),
    Static(bool),
    Get(&'static str),
    Set(&'static str),
    UrlString,
    Params(&'static str),
    Size,
    Next,
}
impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Constructor(Kind::Url) | Self::Static(_) | Self::Set(_) => 1,
            Self::Params("append" | "set") => 2,
            Self::Params("get" | "getAll" | "has" | "delete" | "forEach") => 1,
            _ => 0,
        }
    }
}
#[derive(Debug)]
enum ParseError {
    Invalid,
    Unsupported(&'static str),
    Limit(&'static str),
}
impl ParseError {
    fn fault(self) -> Fault {
        match self {
            Self::Invalid => Fault::exception("TypeError", "Invalid URL."),
            Self::Unsupported(message) | Self::Limit(message) => Fault::new(message),
        }
    }
}
#[derive(Clone, Copy)]
enum Encode {
    User,
    Path,
    Query,
    Fragment,
    Form,
}
fn encode_byte(b: u8, set: Encode) -> bool {
    if matches!(set, Encode::Form) {
        return !(b.is_ascii_alphanumeric() || matches!(b, b'*' | b'-' | b'.' | b'_'));
    }
    if !(0x21..=0x7e).contains(&b) || matches!(b, b'"' | b'<' | b'>') {
        return true;
    }
    match set {
        Encode::Fragment => b == b'`',
        Encode::Query => matches!(b, b'#' | b'\''),
        Encode::Path => matches!(b, b'#' | b'?' | b'^' | b'`' | b'{' | b'}'),
        Encode::User => matches!(
            b,
            b'#' | b'?'
                | b'^'
                | b'`'
                | b'{'
                | b'}'
                | b'/'
                | b':'
                | b';'
                | b'='
                | b'@'
                | b'['
                | b'\\'
                | b']'
                | b'|'
        ),
        Encode::Form => unreachable!(),
    }
}
fn encode(input: &str, set: Encode, max: usize) -> Result<String, ParseError> {
    let len = input
        .bytes()
        .map(|b| {
            if matches!(set, Encode::Form) && b == b' ' {
                1
            } else if encode_byte(b, set) {
                3
            } else {
                1
            }
        })
        .sum::<usize>();
    if len > max {
        return Err(ParseError::Limit(if matches!(set, Encode::Form) {
            "URLSearchParams byte limit reached (64 KiB)."
        } else {
            "URL length limit reached (16 KiB)."
        }));
    }
    let mut out = String::with_capacity(len);
    for b in input.bytes() {
        if matches!(set, Encode::Form) && b == b' ' {
            out.push('+');
        } else if encode_byte(b, set) {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 15) as usize] as char);
        } else {
            out.push(b as char);
        }
    }
    Ok(out)
}
fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
fn percent_decode(input: &str, plus: bool) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(a * 16 + b);
                i += 3;
                continue;
            }
        }
        out.push(if plus && bytes[i] == b'+' {
            b' '
        } else {
            bytes[i]
        });
        i += 1;
    }
    out
}
fn clean(input: &str, trim: bool) -> String {
    let input = if trim {
        input.trim_matches(|c: char| c <= '\u{20}')
    } else {
        input
    };
    input
        .chars()
        .filter(|c| !matches!(*c, '\t' | '\n' | '\r'))
        .collect()
}
fn scheme(input: &str) -> Option<(&str, &str)> {
    let at = input.find(':')?;
    let name = &input[..at];
    if name
        .as_bytes()
        .first()
        .is_none_or(|b| !b.is_ascii_alphabetic())
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return None;
    }
    Some((name, &input[at + 1..]))
}
fn ipv4_number(input: &str) -> Option<u64> {
    if input.is_empty() {
        return None;
    }
    let (base, digits) = if input.len() >= 2 && input[..2].eq_ignore_ascii_case("0x") {
        (16, &input[2..])
    } else if input.len() >= 2 && input.starts_with('0') {
        (8, &input[1..])
    } else {
        (10, input)
    };
    if digits.is_empty() {
        return Some(0);
    }
    let mut n = 0u64;
    for b in digits.bytes() {
        let digit = hex(b)? as u64;
        if digit >= base {
            return None;
        }
        n = n.saturating_mul(base).saturating_add(digit);
    }
    Some(n)
}
fn ipv4(host: &str) -> Result<Option<String>, ParseError> {
    let text = host.strip_suffix('.').unwrap_or(host);
    let last = text.rsplit('.').next().unwrap_or("");
    if ipv4_number(last).is_none()
        && !(!last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()))
    {
        return Ok(None);
    }
    let parts = text.split('.').collect::<Vec<_>>();
    if parts.len() > 4 || parts.iter().any(|s| s.is_empty()) {
        return Err(ParseError::Invalid);
    }
    let mut numbers = Vec::with_capacity(parts.len());
    for part in &parts {
        numbers.push(ipv4_number(part).ok_or(ParseError::Invalid)?);
    }
    if numbers[..numbers.len() - 1].iter().any(|n| *n > 255) {
        return Err(ParseError::Invalid);
    }
    let last = *numbers.last().unwrap();
    if last >= (1u64 << (8 * (5 - numbers.len()))) {
        return Err(ParseError::Invalid);
    }
    let mut n = last;
    for (i, v) in numbers[..numbers.len() - 1].iter().enumerate() {
        n += *v << (8 * (3 - i));
    }
    Ok(Some(format!(
        "{}.{}.{}.{}",
        n >> 24,
        (n >> 16) & 255,
        (n >> 8) & 255,
        n & 255
    )))
}
fn ipv6_text(address: Ipv6Addr) -> String {
    let parts = address.segments();
    let mut best = (0, 0);
    let mut i = 0;
    while i < 8 {
        if parts[i] != 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < 8 && parts[i] == 0 {
            i += 1;
        }
        if i - start > best.1 {
            best = (start, i - start);
        }
    }
    let mut out = String::from("[");
    let mut i = 0;
    while i < 8 {
        if best.1 > 1 && i == best.0 {
            out.push_str("::");
            i += best.1;
            continue;
        }
        if i > 0 && !(best.1 > 1 && i == best.0 + best.1) {
            out.push(':');
        }
        out.push_str(&format!("{:x}", parts[i]));
        i += 1;
    }
    out.push(']');
    out
}
fn hostname(input: &str) -> Result<String, ParseError> {
    if input.starts_with('[') {
        let inner = input
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or(ParseError::Invalid)?;
        return inner
            .parse::<Ipv6Addr>()
            .map(ipv6_text)
            .map_err(|_| ParseError::Invalid);
    }
    if input.is_empty() {
        return Err(ParseError::Invalid);
    }
    let bytes = percent_decode(input, false);
    let text = String::from_utf8(bytes).map_err(|_| ParseError::Invalid)?;
    // ACE labels also require IDNA validation. Do not report them as parsed
    // successfully merely because their external representation is ASCII.
    if !text.is_ascii()
        || text
            .split('.')
            .any(|label| label.len() >= 4 && label[..4].eq_ignore_ascii_case("xn--"))
    {
        return Err(ParseError::Unsupported(
            "URL IDNA host conversion/validation is not implemented.",
        ));
    }
    if text.bytes().any(|b| {
        b <= 0x20
            || b == 0x7f
            || matches!(
                b,
                b'#' | b'%'
                    | b'/'
                    | b':'
                    | b'<'
                    | b'>'
                    | b'?'
                    | b'@'
                    | b'['
                    | b'\\'
                    | b']'
                    | b'^'
                    | b'|'
            )
    }) {
        return Err(ParseError::Invalid);
    }
    let text = text.to_ascii_lowercase();
    Ok(ipv4(&text)?.unwrap_or(text))
}
fn host_port(input: &str) -> Result<(&str, Option<&str>), ParseError> {
    if input.starts_with('[') {
        let end = input.find(']').ok_or(ParseError::Invalid)?;
        let rest = &input[end + 1..];
        if rest.is_empty() {
            Ok((&input[..end + 1], None))
        } else {
            Ok((
                &input[..end + 1],
                Some(rest.strip_prefix(':').ok_or(ParseError::Invalid)?),
            ))
        }
    } else if let Some((host, port)) = input.split_once(':') {
        Ok((host, Some(port)))
    } else {
        Ok((input, None))
    }
}
fn port(input: &str, default: u16, partial: bool) -> Result<Option<u16>, ParseError> {
    let end = input
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(input.len());
    if (!partial && end != input.len()) || (partial && end == 0 && !input.is_empty()) {
        return Err(ParseError::Invalid);
    }
    let text = &input[..end];
    if text.is_empty() {
        return Ok(None);
    }
    let mut number = 0u32;
    for b in text.bytes() {
        number = number * 10 + (b - b'0') as u32;
        if number > 65535 {
            return Err(ParseError::Invalid);
        }
    }
    Ok(if number == default as u32 {
        None
    } else {
        Some(number as u16)
    })
}
fn path(input: &str) -> Result<String, ParseError> {
    let converted = input.replace('\\', "/");
    let input = converted.strip_prefix('/').unwrap_or(&converted);
    let mut stack = Vec::new();
    let segments = input.split('/').collect::<Vec<_>>();
    for (i, segment) in segments.iter().enumerate() {
        let lower = segment.to_ascii_lowercase();
        let single = lower == "." || lower == "%2e";
        let double = lower == ".." || lower == ".%2e" || lower == "%2e." || lower == "%2e%2e";
        if double {
            stack.pop();
        }
        if single || double {
            if i + 1 == segments.len() {
                stack.push(String::new());
            }
        } else {
            stack.push(encode(segment, Encode::Path, MAX_URL_BYTES)?);
        }
    }
    let out = format!("/{}", stack.join("/"));
    if out.len() > MAX_URL_BYTES {
        Err(ParseError::Limit("URL length limit reached (16 KiB)."))
    } else {
        Ok(out)
    }
}
fn split_tail(input: &str) -> (&str, Option<&str>, Option<&str>) {
    let (head, fragment) = input
        .split_once('#')
        .map_or((input, None), |(a, b)| (a, Some(b)));
    let (head, query) = head
        .split_once('?')
        .map_or((head, None), |(a, b)| (a, Some(b)));
    (head, query, fragment)
}
fn parse_url(input: &str, base: Option<&Url>) -> Result<Url, ParseError> {
    if input.len() > MAX_URL_BYTES {
        return Err(ParseError::Limit("URL length limit reached (16 KiB)."));
    }
    let normalized = clean(input, true);
    let input = normalized.as_str();
    let specified = scheme(input);
    let (secure, rest, absolute) = if let Some((name, rest)) = specified {
        let secure = if name.eq_ignore_ascii_case("https") {
            true
        } else if name.eq_ignore_ascii_case("http") {
            false
        } else {
            return Err(ParseError::Unsupported(
                "URL values support only HTTP(S) schemes.",
            ));
        };
        let absolute = base.is_none_or(|b| b.secure != secure)
            || rest.starts_with("//")
            || rest.starts_with("/\\")
            || rest.starts_with("\\/")
            || rest.starts_with("\\\\");
        (secure, rest, absolute)
    } else {
        (base.ok_or(ParseError::Invalid)?.secure, input, false)
    };
    let (head, query, fragment) = split_tail(rest);
    let normalized_head = head.replace('\\', "/");
    let head = normalized_head.as_str();
    let authority = absolute || head.starts_with("//");
    let mut url = if authority {
        let head = head.trim_start_matches('/');
        let (authority, pathtext) = head.split_once('/').map_or((head, ""), |(a, b)| (a, b));
        let (user, authority) = authority
            .rsplit_once('@')
            .map_or((None, authority), |(a, b)| (Some(a), b));
        let (host, porttext) = host_port(authority)?;
        let hostname = hostname(host)?;
        let default = if secure { 443 } else { 80 };
        let port = porttext
            .map(|p| port(p, default, false))
            .transpose()?
            .flatten();
        let (username, password) = if let Some(user) = user {
            let (a, b) = user.split_once(':').unwrap_or((user, ""));
            (
                encode(a, Encode::User, MAX_URL_BYTES)?,
                encode(b, Encode::User, MAX_URL_BYTES)?,
            )
        } else {
            (String::new(), String::new())
        };
        Url {
            secure,
            username,
            password,
            hostname,
            port,
            path: path(&format!("/{pathtext}"))?,
            query: None,
            fragment: None,
        }
    } else {
        let mut url = base.ok_or(ParseError::Invalid)?.clone();
        url.fragment = None;
        if !head.is_empty() {
            let merged = if head.starts_with('/') {
                head.to_owned()
            } else {
                format!(
                    "{}/{}",
                    url.path.rsplit_once('/').map_or("", |(a, _)| a),
                    head
                )
            };
            url.path = path(&merged)?;
            url.query = None;
        }
        url
    };
    if let Some(q) = query {
        url.query = Some(encode(q, Encode::Query, MAX_URL_BYTES)?);
    }
    if let Some(f) = fragment {
        url.fragment = Some(encode(f, Encode::Fragment, MAX_URL_BYTES)?);
    }
    url.check()?;
    Ok(url)
}
fn pair_text_bytes(pairs: &[Pair]) -> usize {
    pairs.iter().map(|(a, b)| a.len() + b.len()).sum()
}
fn pair_storage(pairs: &Vec<Pair>) -> usize {
    pair_text_bytes(pairs) + pairs.capacity() * std::mem::size_of::<Pair>()
}
fn check_pairs(pairs: &[Pair]) -> Result<(), Fault> {
    if pairs.len() > MAX_PAIRS {
        return Err(Fault::new("URLSearchParams entry limit reached (1024)."));
    }
    if pair_text_bytes(pairs) > MAX_PARAMS_BYTES {
        return Err(Fault::new("URLSearchParams byte limit reached (64 KiB)."));
    }
    Ok(())
}
fn parse_pairs(input: &str) -> Result<Vec<Pair>, Fault> {
    if input.len() > MAX_PARAMS_BYTES {
        return Err(Fault::new("URLSearchParams byte limit reached (64 KiB)."));
    }
    let mut pairs = Vec::new();
    let mut total = 0usize;
    for part in input.split('&').filter(|s| !s.is_empty()) {
        if pairs.len() >= MAX_PAIRS {
            return Err(Fault::new("URLSearchParams entry limit reached (1024)."));
        }
        let (a, b) = part.split_once('=').unwrap_or((part, ""));
        let a = String::from_utf8_lossy(&percent_decode(a, true)).into_owned();
        let b = String::from_utf8_lossy(&percent_decode(b, true)).into_owned();
        total += a.len() + b.len();
        if total > MAX_PARAMS_BYTES {
            return Err(Fault::new("URLSearchParams byte limit reached (64 KiB)."));
        }
        pairs.push((a, b));
    }
    Ok(pairs)
}
fn serialize_pairs(pairs: &[Pair]) -> Result<String, Fault> {
    let mut out = String::new();
    for (i, (a, b)) in pairs.iter().enumerate() {
        let a = encode(a, Encode::Form, MAX_PARAMS_BYTES).map_err(ParseError::fault)?;
        let b = encode(b, Encode::Form, MAX_PARAMS_BYTES).map_err(ParseError::fault)?;
        if out.len() + a.len() + b.len() + 1 + usize::from(i > 0) > MAX_PARAMS_BYTES {
            return Err(Fault::new("URLSearchParams byte limit reached (64 KiB)."));
        }
        if i > 0 {
            out.push('&');
        }
        out.push_str(&a);
        out.push('=');
        out.push_str(&b);
    }
    Ok(out)
}

impl Runtime {
    pub(super) fn initialize_url_api(&mut self) -> Result<(), Fault> {
        for (name, kind) in [("URL", Kind::Url), ("URLSearchParams", Kind::Params)] {
            let prototype = self.new_object(None)?;
            match kind {
                Kind::Url => self.url_roots.url = Some(prototype),
                Kind::Params => self.url_roots.params = Some(prototype),
            }
            let constructor = self.native_function(Builtin::Url(Op::Constructor(kind)), name)?;
            self.put_binding(self.global_environment, name.into(), Some(constructor), Declaration::Var, false)?;
            self.constructor_prototype(name, Value::Object(prototype))?;
        }
        let url = self.url_roots.url.unwrap();
        for name in [
            "href",
            "origin",
            "protocol",
            "username",
            "password",
            "host",
            "hostname",
            "port",
            "pathname",
            "search",
            "hash",
            "searchParams",
        ] {
            self.url_accessor(
                url,
                name,
                Op::Get(name),
                if matches!(name, "origin" | "searchParams") {
                    None
                } else {
                    Some(Op::Set(name))
                },
            )?;
        }
        for name in ["toString", "toJSON"] {
            self.url_method(url, name, Op::UrlString)?;
        }
        let constructor = self.binding_value(self.global_environment, "URL")?;
        let ctor = self.ordinary_id(Ordinary::from_value(&constructor).unwrap())?;
        for (name, parse) in [("canParse", false), ("parse", true)] {
            self.url_method(ctor, name, Op::Static(parse))?;
        }
        let params = self.url_roots.params.unwrap();
        self.url_accessor(params, "size", Op::Size, None)?;
        for name in [
            "append", "delete", "get", "getAll", "has", "set", "sort", "toString", "keys",
            "values", "entries", "forEach",
        ] {
            self.url_method(params, name, Op::Params(name))?;
        }
        let entries = self.get_property(params, "entries")?;
        self.store_symbol_property(
            params,
            self.symbol_iterator.unwrap(),
            entries,
            Some((true, true, true)),
        )?;
        let iterator = self.new_object(None)?;
        self.url_roots.iterator = Some(iterator);
        self.url_method(iterator, "next", Op::Next)?;
        let identity = self.native_function(
            Builtin::Iterator(super::iterator_protocol::Op::Self_),
            "[Symbol.iterator]",
        )?;
        self.store_symbol_property(
            iterator,
            self.symbol_iterator.unwrap(),
            identity,
            Some((false, true, true)),
        )?;
        Ok(())
    }
    fn url_method(&mut self, owner: usize, name: &str, op: Op) -> Result<(), Fault> {
        let f = self.native_function(Builtin::Url(op), name)?;
        self.store_property(owner, name.into(), f, Some((true, true, true)))
    }
    fn url_accessor(
        &mut self,
        owner: usize,
        name: &str,
        get: Op,
        set: Option<Op>,
    ) -> Result<(), Fault> {
        let Value::Function(get) =
            self.native_function(Builtin::Url(get), &format!("get {name}"))?
        else {
            unreachable!()
        };
        let set = if let Some(set) = set {
            let Value::Function(set) =
                self.native_function(Builtin::Url(set), &format!("set {name}"))?
            else {
                unreachable!()
            };
            Some(set)
        } else {
            None
        };
        self.store_accessor_property(
            owner,
            name.into(),
            Accessors {
                get: Some(get),
                set,
            },
            (true, true),
        )
    }
    fn url_string(&mut self, value: &Value, host: &mut dyn Host) -> Result<String, Fault> {
        let input = self.copy_value(value)?;
        let primitive = self.primitive_with_hint(input, true, host)?;
        let text = primitive.script_text()?;
        self.work(text.len())?;
        Ok(text)
    }
    fn url_data(&self, id: usize) -> Result<&Data, Fault> {
        self.object(id)?.url.as_ref().ok_or_else(|| {
            Fault::exception("TypeError", "URL method has an incompatible receiver.")
        })
    }
    fn url_record(&self, id: usize) -> Result<(&Url, usize), Fault> {
        match self.url_data(id)? {
            Data::Url { value, params } => Ok((value, *params)),
            _ => Err(Fault::exception(
                "TypeError",
                "URL method has an incompatible receiver.",
            )),
        }
    }
    fn params_data(&self, id: usize) -> Result<(&Vec<Pair>, Option<usize>), Fault> {
        match self.url_data(id)? {
            Data::Params { pairs, owner } => Ok((pairs, *owner)),
            _ => Err(Fault::exception(
                "TypeError",
                "URLSearchParams method has an incompatible receiver.",
            )),
        }
    }
    fn url_receiver(&self, this: &Value, params: bool) -> Result<usize, Fault> {
        if let Value::Object(id) = this {
            if params {
                self.params_data(*id)?;
            } else {
                self.url_record(*id)?;
            }
            return Ok(*id);
        }
        Err(Fault::exception(
            "TypeError",
            "URL method has an incompatible receiver.",
        ))
    }
    fn url_reserve(&mut self, old: usize, new: usize) -> Result<usize, Fault> {
        self.work(new)?;
        let total = self.value_bytes.saturating_sub(old).saturating_add(new);
        if total > self.limits.heap.retained_values {
            return Err(Fault::new(format!("Retained value limit reached ({} bytes).", self.limits.heap.retained_values)));
        }
        Ok(total)
    }
    fn new_url_value(&mut self, value: Url) -> Result<Value, Fault> {
        self.ensure_global_intrinsics("URL")?;
        self.work(value.bytes().saturating_mul(4))?;
        let pairs = parse_pairs(value.query.as_deref().unwrap_or(""))?;
        let bytes = value.bytes() + pair_storage(&pairs);
        let retained = self.url_reserve(0, bytes)?;
        let object = self.new_object(None)?;
        let params = self.new_object(None)?;
        self.objects[object].url = Some(Data::Url { value, params });
        self.objects[object].prototype = self.url_roots.url.map(Ordinary::Object);
        self.objects[params].url = Some(Data::Params {
            pairs,
            owner: Some(object),
        });
        self.objects[params].prototype = self.url_roots.params.map(Ordinary::Object);
        self.value_bytes = retained;
        Ok(Value::Object(object))
    }
    fn new_params_value(&mut self, pairs: Vec<Pair>) -> Result<Value, Fault> {
        self.ensure_global_intrinsics("URLSearchParams")?;
        check_pairs(&pairs)?;
        let retained = self.url_reserve(0, pair_storage(&pairs))?;
        let object = self.new_object(None)?;
        self.objects[object].url = Some(Data::Params { pairs, owner: None });
        self.objects[object].prototype = self.url_roots.params.map(Ordinary::Object);
        self.value_bytes = retained;
        Ok(Value::Object(object))
    }
    fn url_parse_arguments(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Result<Url, ParseError>, Fault> {
        if args.is_empty() {
            return Err(Fault::exception(
                "TypeError",
                "URL requires a URL argument.",
            ));
        }
        let input = self.url_string(&args[0], host)?;
        let base = if let Some(value) = args.get(1).filter(|v| !matches!(v, Value::Undefined)) {
            Some(self.url_string(value, host)?)
        } else {
            None
        };
        self.work((input.len() + base.as_ref().map_or(0, String::len)).saturating_mul(8))?;
        let base = match base {
            Some(base) => match parse_url(&base, None) {
                Ok(base) => Some(base),
                Err(error) => return Ok(Err(error)),
            },
            None => None,
        };
        Ok(parse_url(&input, base.as_ref()))
    }
    pub(super) fn url_construct(
        &mut self,
        kind: Kind,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("URL callback call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = (|| match kind {
            Kind::Url => {
                let value = self
                    .url_parse_arguments(args, host)?
                    .map_err(ParseError::fault)?;
                self.new_url_value(value)
            }
            Kind::Params => {
                let pairs = self.params_input(args.first().unwrap_or(&Value::Undefined), host)?;
                self.new_params_value(pairs)
            }
        })();
        self.call_depth -= 1;
        result
    }
    fn params_input(&mut self, value: &Value, host: &mut dyn Host) -> Result<Vec<Pair>, Fault> {
        if matches!(value, Value::Undefined) {
            return Ok(Vec::new());
        }
        if matches!(value, Value::Host(_)) {
            return Err(Fault::new(
                "Native-host URLSearchParams construction is not implemented.",
            ));
        }
        if let Some(ordinary) = Ordinary::from_value(value) {
            if let Some(method) = self.iterator_method(value, host)? {
                let input = self.copy_value(value)?;
                let mut iterator = self.iterator_from_method(input, method, host)?;
                let mut pairs = Vec::new();
                let mut bytes = 0usize;
                loop {
                    let Some(item) = self.iterator_next(&mut iterator, host)? else {
                        return Ok(pairs);
                    };
                    let next = (|| {
                        if pairs.len() >= MAX_PAIRS {
                            return Err(Fault::new("URLSearchParams entry limit reached (1024)."));
                        }
                        let pair = self.params_sequence_pair(item, host)?;
                        bytes += pair.0.len() + pair.1.len();
                        if bytes > MAX_PARAMS_BYTES {
                            return Err(Fault::new("URLSearchParams byte limit reached (64 KiB)."));
                        }
                        Ok(pair)
                    })();
                    match next {
                        Ok(pair) => pairs.push(pair),
                        Err(error) => return self.iterator_close(&mut iterator, Err(error), host),
                    }
                }
            }
            let object = self.ordinary_id(ordinary)?;
            let keys = self.own_string_keys(object, false)?;
            let symbols = self.own_symbol_keys(object, false)?;
            let mut pairs = Vec::new();
            let mut bytes = 0usize;
            for key in keys {
                self.step()?;
                if !self.own_enumerable(object, &key)? {
                    continue;
                }
                if pairs.len() >= MAX_PAIRS {
                    return Err(Fault::new("URLSearchParams entry limit reached (1024)."));
                }
                let receiver = self.copy_value(value)?;
                let next =
                    self.read_reference(&Reference::Property(receiver, key.clone()), host)?;
                let next = self.url_string(&next, host)?;
                bytes += key.len() + next.len();
                if bytes > MAX_PARAMS_BYTES {
                    return Err(Fault::new("URLSearchParams byte limit reached (64 KiB)."));
                }
                pairs.push((key, next));
            }
            for key in symbols {
                self.step()?;
                if self.objects[object]
                    .symbol_properties
                    .get(&key)
                    .is_some_and(|p| p.enumerable)
                {
                    return Err(Fault::exception(
                        "TypeError",
                        "Cannot convert a Symbol record key to a string.",
                    ));
                }
            }
            return Ok(pairs);
        }
        let text = self.url_string(value, host)?;
        self.work(text.len().saturating_mul(4))?;
        parse_pairs(text.strip_prefix('?').unwrap_or(&text))
    }
    fn params_sequence_pair(&mut self, item: Value, host: &mut dyn Host) -> Result<Pair, Fault> {
        if Ordinary::from_value(&item).is_none() {
            return Err(Fault::exception(
                "TypeError",
                "URLSearchParams pair must be an iterable object.",
            ));
        }
        let method = self.iterator_method(&item, host)?.ok_or_else(|| {
            Fault::exception("TypeError", "URLSearchParams pair must be iterable.")
        })?;
        let mut iterator = self.iterator_from_method(item, method, host)?;
        let mut values = Vec::new();
        let mut bytes = 0usize;
        loop {
            let Some(value) = self.iterator_next(&mut iterator, host)? else {
                break;
            };
            let result = (|| {
                if values.len() >= MAX_PAIRS {
                    return Err(Fault::new(
                        "URLSearchParams pair conversion limit reached (1024).",
                    ));
                }
                let text = self.url_string(&value, host)?;
                bytes += text.len();
                if bytes > MAX_PARAMS_BYTES {
                    return Err(Fault::new("URLSearchParams byte limit reached (64 KiB)."));
                }
                Ok(text)
            })();
            match result {
                Ok(value) => values.push(value),
                Err(error) => return self.iterator_close(&mut iterator, Err(error), host),
            }
        }
        if values.len() != 2 {
            return Err(Fault::exception(
                "TypeError",
                "URLSearchParams pairs must contain exactly two values.",
            ));
        }
        let value = values.pop().unwrap();
        Ok((values.pop().unwrap(), value))
    }
    fn url_replace(&mut self, id: usize, value: Url, update_query: bool) -> Result<(), Fault> {
        value.check().map_err(ParseError::fault)?;
        let (_, params) = self.url_record(id)?;
        let pairs = if update_query {
            self.work(
                value
                    .query
                    .as_ref()
                    .map_or(0, String::len)
                    .saturating_mul(4),
            )?;
            Some(parse_pairs(value.query.as_deref().unwrap_or(""))?)
        } else {
            None
        };
        let old = self.url_data(id)?.bytes()
            + if pairs.is_some() {
                self.url_data(params)?.bytes()
            } else {
                0
            };
        let new = value.bytes() + pairs.as_ref().map_or(0, pair_storage);
        let retained = self.url_reserve(old, new)?;
        self.objects[id].url = Some(Data::Url { value, params });
        if let Some(pairs) = pairs {
            self.objects[params].url = Some(Data::Params {
                pairs,
                owner: Some(id),
            });
        }
        self.value_bytes = retained;
        Ok(())
    }
    fn params_replace(&mut self, id: usize, pairs: Vec<Pair>) -> Result<(), Fault> {
        check_pairs(&pairs)?;
        self.work(pair_storage(&pairs).saturating_mul(4))?;
        let (_, owner) = self.params_data(id)?;
        let changed_url = if let Some(owner) = owner {
            let text = serialize_pairs(&pairs)?;
            self.work(self.url_data(owner)?.bytes())?;
            let mut value = self.url_record(owner)?.0.clone();
            value.query = if text.is_empty() { None } else { Some(text) };
            value.check().map_err(ParseError::fault)?;
            Some(value)
        } else {
            None
        };
        let old = self.url_data(id)?.bytes()
            + owner.map_or(0, |owner| self.objects[owner].url.as_ref().unwrap().bytes());
        let new = pair_storage(&pairs) + changed_url.as_ref().map_or(0, Url::bytes);
        let retained = self.url_reserve(old, new)?;
        self.objects[id].url = Some(Data::Params { pairs, owner });
        if let (Some(owner), Some(value)) = (owner, changed_url) {
            self.objects[owner].url = Some(Data::Url { value, params: id });
        }
        self.value_bytes = retained;
        Ok(())
    }
    fn url_get(&mut self, id: usize, name: &str) -> Result<Value, Fault> {
        if name == "searchParams" {
            return Ok(Value::Object(self.url_record(id)?.1));
        }
        self.work(self.url_record(id)?.0.bytes())?;
        let value = self.url_record(id)?.0;
        let text = match name {
            "href" => value.href(),
            "origin" => value.origin(),
            "protocol" => value.protocol().into(),
            "username" => value.username.clone(),
            "password" => value.password.clone(),
            "host" => value.host(),
            "hostname" => value.hostname.clone(),
            "port" => value.port.map_or(String::new(), |p| p.to_string()),
            "pathname" => value.path.clone(),
            "search" => value
                .query
                .as_ref()
                .filter(|s| !s.is_empty())
                .map_or(String::new(), |s| format!("?{s}")),
            "hash" => value
                .fragment
                .as_ref()
                .filter(|s| !s.is_empty())
                .map_or(String::new(), |s| format!("#{s}")),
            _ => unreachable!(),
        };
        self.checked_value(Value::String(text))
    }
    fn url_set(&mut self, id: usize, name: &str, text: String) -> Result<Value, Fault> {
        if text.len() > MAX_URL_BYTES {
            return Err(Fault::new("URL length limit reached (16 KiB)."));
        }
        self.work(text.len().saturating_mul(8) + self.url_record(id)?.0.bytes())?;
        let mut value = self.url_record(id)?.0.clone();
        if name == "href" {
            value = parse_url(&text, None).map_err(ParseError::fault)?;
            self.url_replace(id, value, true)?;
            return Ok(Value::Undefined);
        }
        let text = if matches!(name, "username" | "password") {
            text
        } else {
            clean(&text, false)
        };
        let mut update_query = false;
        match name {
            "username" => {
                value.username =
                    encode(&text, Encode::User, MAX_URL_BYTES).map_err(ParseError::fault)?
            }
            "password" => {
                value.password =
                    encode(&text, Encode::User, MAX_URL_BYTES).map_err(ParseError::fault)?
            }
            "pathname" => value.path = path(&text).map_err(ParseError::fault)?,
            "search" => {
                value.query = if text.is_empty() {
                    None
                } else {
                    Some(
                        encode(
                            text.strip_prefix('?').unwrap_or(&text),
                            Encode::Query,
                            MAX_URL_BYTES,
                        )
                        .map_err(ParseError::fault)?,
                    )
                };
                update_query = true;
            }
            "hash" => {
                value.fragment = if text.is_empty() {
                    None
                } else {
                    Some(
                        encode(
                            text.strip_prefix('#').unwrap_or(&text),
                            Encode::Fragment,
                            MAX_URL_BYTES,
                        )
                        .map_err(ParseError::fault)?,
                    )
                }
            }
            "protocol" => {
                let name = text.split(':').next().unwrap_or("");
                if name.eq_ignore_ascii_case("http") {
                    value.secure = false;
                } else if name.eq_ignore_ascii_case("https") {
                    value.secure = true;
                } else if scheme(&format!("{name}:")).is_some() {
                    return Err(Fault::new("URL values support only HTTP(S) schemes."));
                } else {
                    return Ok(Value::Undefined);
                }
                if value.port == Some(value.default_port()) {
                    value.port = None;
                }
            }
            "port" => match port(&text, value.default_port(), true) {
                Ok(port) => value.port = port,
                Err(ParseError::Invalid) => return Ok(Value::Undefined),
                Err(e) => return Err(e.fault()),
            },
            "host" | "hostname" => {
                let text = text.split(['/', '?', '#', '\\']).next().unwrap_or("");
                let (host, p) = match host_port(text) {
                    Ok(parts) => parts,
                    Err(ParseError::Invalid) => return Ok(Value::Undefined),
                    Err(e) => return Err(e.fault()),
                };
                if name == "hostname" && p.is_some() {
                    return Ok(Value::Undefined);
                }
                match hostname(host) {
                    Ok(host) => value.hostname = host,
                    Err(ParseError::Invalid) => return Ok(Value::Undefined),
                    Err(e) => return Err(e.fault()),
                }
                if name == "host" {
                    if let Some(p) = p.filter(|p| !p.is_empty()) {
                        if let Ok(port) = port(p, value.default_port(), true) {
                            value.port = port;
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
        self.url_replace(id, value, update_query)?;
        Ok(Value::Undefined)
    }
    fn params_next(&mut self, id: usize) -> Result<Option<Value>, Fault> {
        self.step()?;
        let (source, next, kind) = match self.url_data(id)? {
            Data::Iterator { source, next, kind } => (*source, *next, *kind),
            _ => {
                return Err(Fault::exception(
                    "TypeError",
                    "URLSearchParams iterator has an incompatible receiver.",
                ))
            }
        };
        let Some(source) = source else {
            return Ok(None);
        };
        let pairs = self.params_data(source)?.0;
        // Web IDL pair iterators consult the live list on every next call.
        // A direct next() after done can observe subsequently appended pairs.
        if next >= pairs.len() {
            return Ok(None);
        }
        let bytes = pairs[next].0.len() + pairs[next].1.len();
        self.work(bytes)?;
        let pair = self.params_data(source)?.0[next].clone();
        if let Some(Data::Iterator { next, .. }) = &mut self.objects[id].url {
            *next += 1;
        }
        match kind {
            IterKind::Keys => Ok(Some(Value::String(pair.0))),
            IterKind::Values => Ok(Some(Value::String(pair.1))),
            IterKind::Entries => {
                let array = self.new_object(Some(2))?;
                self.set_property(array, "0".into(), Value::String(pair.0))?;
                self.set_property(array, "1".into(), Value::String(pair.1))?;
                Ok(Some(Value::Object(array)))
            }
        }
    }
    fn params_method(
        &mut self,
        id: usize,
        name: &str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if name == "forEach" {
            let callback = args.first().unwrap_or(&Value::Undefined);
            if !matches!(callback, Value::Function(_)) {
                return Err(Fault::exception(
                    "TypeError",
                    "URLSearchParams forEach callback must be callable.",
                ));
            }
            let mut index = 0;
            loop {
                self.step()?;
                let pairs = self.params_data(id)?.0;
                if index >= pairs.len() {
                    break;
                }
                let bytes = pairs[index].0.len() + pairs[index].1.len();
                self.work(bytes)?;
                let (name, value) = self.params_data(id)?.0[index].clone();
                let this = self.copy_value(args.get(1).unwrap_or(&Value::Undefined))?;
                self.call_value(
                    callback.clone(),
                    &[Value::String(value), Value::String(name), Value::Object(id)],
                    this,
                    host,
                )?;
                index += 1;
            }
            return Ok(Value::Undefined);
        }
        if matches!(name, "keys" | "values" | "entries") {
            let iterator = self.new_object(None)?;
            let kind = match name {
                "keys" => IterKind::Keys,
                "values" => IterKind::Values,
                _ => IterKind::Entries,
            };
            self.objects[iterator].url = Some(Data::Iterator {
                source: Some(id),
                next: 0,
                kind,
            });
            self.objects[iterator].prototype = self.url_roots.iterator.map(Ordinary::Object);
            return Ok(Value::Object(iterator));
        }
        if name == "toString" {
            self.work(self.url_data(id)?.bytes().saturating_mul(4))?;
            let text = serialize_pairs(self.params_data(id)?.0)?;
            return self.checked_value(Value::String(text));
        }
        let required = if matches!(name, "append" | "set") {
            2
        } else if name == "sort" {
            0
        } else {
            1
        };
        if args.len() < required {
            return Err(Fault::exception(
                "TypeError",
                "Missing required URLSearchParams argument.",
            ));
        }
        let key = if required > 0 {
            Some(self.url_string(&args[0], host)?)
        } else {
            None
        };
        let item = if required == 2 {
            Some(self.url_string(&args[1], host)?)
        } else if matches!(name, "has" | "delete")
            && args.get(1).is_some_and(|v| !matches!(v, Value::Undefined))
        {
            Some(self.url_string(&args[1], host)?)
        } else {
            None
        };
        self.work(self.url_data(id)?.bytes())?;
        match name {
            "get" => {
                let found = self
                    .params_data(id)?
                    .0
                    .iter()
                    .find(|(k, _)| Some(k) == key.as_ref())
                    .map(|(_, v)| v.clone());
                return match found {
                    Some(text) => self.checked_value(Value::String(text)),
                    None => Ok(Value::Null),
                };
            }
            "getAll" => {
                let found = self
                    .params_data(id)?
                    .0
                    .iter()
                    .filter(|(k, _)| Some(k) == key.as_ref())
                    .map(|(_, v)| v.clone())
                    .collect::<Vec<_>>();
                let array = self.new_object(Some(found.len()))?;
                for (i, text) in found.into_iter().enumerate() {
                    self.step()?;
                    self.set_property(array, i.to_string(), Value::String(text))?;
                }
                return Ok(Value::Object(array));
            }
            "has" => {
                return Ok(Value::Bool(self.params_data(id)?.0.iter().any(|(k, v)| {
                    Some(k) == key.as_ref() && item.as_ref().is_none_or(|item| v == item)
                })))
            }
            _ => {}
        }
        let mut pairs = self.params_data(id)?.0.clone();
        match name {
            "append" => {
                if pairs.len() >= MAX_PAIRS {
                    return Err(Fault::new("URLSearchParams entry limit reached (1024)."));
                }
                pairs.push((key.unwrap(), item.unwrap()));
            }
            "set" => {
                let key = key.unwrap();
                let item = item.unwrap();
                let mut first = true;
                pairs.retain_mut(|(k, v)| {
                    if *k != key {
                        return true;
                    }
                    if !first {
                        return false;
                    }
                    *v = item.clone();
                    first = false;
                    true
                });
                if first {
                    pairs.push((key, item));
                }
            }
            "delete" => pairs.retain(|(k, v)| {
                !(Some(k) == key.as_ref() && item.as_ref().is_none_or(|item| v == item))
            }),
            "sort" => {
                let longest = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
                let rounds = if pairs.len() > 1 {
                    usize::BITS - pairs.len().leading_zeros()
                } else {
                    0
                };
                self.work(
                    longest
                        .saturating_mul(pairs.len())
                        .saturating_mul(rounds as usize)
                        .saturating_mul(2),
                )?;
                pairs.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            }
            _ => unreachable!(),
        }
        self.params_replace(id, pairs)?;
        Ok(Value::Undefined)
    }
    pub(super) fn url_builtin(
        &mut self,
        op: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fault::new("URL callback call limit reached (32)."));
        }
        self.call_depth += 1;
        let result = (|| match op {
            Op::Constructor(_) => Err(Fault::exception(
                "TypeError",
                "URL constructors require new.",
            )),
            Op::Static(parse) => match self.url_parse_arguments(args, host)? {
                Ok(value) => {
                    if parse {
                        self.new_url_value(value)
                    } else {
                        Ok(Value::Bool(true))
                    }
                }
                Err(ParseError::Invalid) => Ok(if parse {
                    Value::Null
                } else {
                    Value::Bool(false)
                }),
                Err(error) => Err(error.fault()),
            },
            Op::Get(name) => {
                let id = self.url_receiver(&this, false)?;
                self.url_get(id, name)
            }
            Op::Set(name) => {
                let id = self.url_receiver(&this, false)?;
                let text = self.url_string(args.first().unwrap_or(&Value::Undefined), host)?;
                self.url_set(id, name, text)
            }
            Op::UrlString => {
                let id = self.url_receiver(&this, false)?;
                self.url_get(id, "href")
            }
            Op::Size => {
                let id = self.url_receiver(&this, true)?;
                Ok(Value::Number(self.params_data(id)?.0.len() as f64))
            }
            Op::Params(name) => {
                let id = self.url_receiver(&this, true)?;
                self.params_method(id, name, args, host)
            }
            Op::Next => {
                let Value::Object(id) = this else {
                    return Err(Fault::exception(
                        "TypeError",
                        "URLSearchParams iterator has an incompatible receiver.",
                    ));
                };
                let next = self.params_next(id)?;
                let done = next.is_none();
                let object = self.new_object(None)?;
                self.set_property(object, "value".into(), next.unwrap_or(Value::Undefined))?;
                self.set_property(object, "done".into(), Value::Bool(done))?;
                Ok(Value::Object(object))
            }
        })();
        self.call_depth -= 1;
        result
    }
}

#[cfg(test)]
#[path = "url_api_tests.rs"]
mod tests;
