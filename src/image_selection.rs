//! Original, bounded HTML responsive-image selection. This module performs no
//! I/O. The loader fetches only the selected URL and applies its normal policy.
//! References: WHATWG HTML 4.8.4.3.7–12, CSS Values 4 and Media Queries 4.
//! Selection uses the captured document viewport; resize refetch is separate.

use crate::html::{Document, NodeKind};
use std::collections::HashMap;

const MAX_ATTRIBUTE: usize = 64 * 1024;
const MAX_CANDIDATES: usize = 128;
const MAX_SOURCES: usize = 64;
const MAX_EXPRESSION: usize = 4096;
const MAX_TOKENS: usize = 256;
const MAX_DEPTH: usize = 16;
const MAX_DIAGNOSTICS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Environment {
    pub viewport_width: f64,
    pub viewport_height: f64,
    /// Device pixels per CSS pixel, including the browser's page zoom.
    pub density: f64,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            viewport_width: 1024.0,
            viewport_height: 768.0,
            density: 1.0,
        }
    }
}

impl Environment {
    fn bounded(self) -> Self {
        let finite = |value: f64, default: f64| {
            if value.is_finite() && value > 0.0 {
                value.min(1_000_000.0)
            } else {
                default
            }
        };
        Self {
            viewport_width: finite(self.viewport_width, 1024.0),
            viewport_height: finite(self.viewport_height, 768.0),
            density: finite(self.density, 1.0).min(64.0),
        }
    }
}

#[derive(Debug)]
pub struct SelectedImage {
    pub url: String,
    /// w / source-size, an x descriptor, or 1 for an ordinary src. Valid 0x
    /// yields zero; a zero source-size with a w descriptor yields infinity.
    /// The caller must bound resulting natural dimensions before layout.
    pub density: f64,
    pub source_node: usize,
    pub responsive: bool,
}

#[derive(Debug, Default)]
pub struct Selection {
    pub image: Option<SelectedImage>,
    /// Short bounded explanations; never source URLs or data-URL payloads.
    pub diagnostics: Vec<String>,
}

fn note(notes: &mut Vec<String>, text: &str) {
    if notes.len() < MAX_DIAGNOSTICS && !notes.iter().any(|old| old == text) {
        notes.push(text.to_owned());
    }
}

pub struct Selector<'a> {
    document: &'a Document,
    parents: Vec<Option<usize>>,
    positions: Vec<usize>,
    picture_sources: HashMap<usize, Vec<(usize, usize)>>,
    environment: Environment,
}

impl<'a> Selector<'a> {
    pub fn new(document: &'a Document, environment: Environment) -> Self {
        let mut parents = vec![None; document.nodes.len()];
        let mut positions = vec![0; document.nodes.len()];
        let mut picture_sources = HashMap::new();
        for (parent, item) in document.nodes.iter().enumerate() {
            let picture = matches!(&item.kind, NodeKind::Element{tag,..} if tag=="picture");
            let mut sources = Vec::new();
            for (position, &child) in item.children.iter().enumerate() {
                if let Some(slot) = parents.get_mut(child) {
                    *slot = Some(parent);
                    positions[child] = position;
                    if picture
                        && sources.len() <= MAX_SOURCES
                        && document.nodes.get(child).is_some_and(
                            |n| matches!(&n.kind,NodeKind::Element{tag,..}if tag=="source"),
                        )
                    {
                        sources.push((child, position));
                    }
                }
            }
            if !sources.is_empty() {
                picture_sources.insert(parent, sources);
            }
        }
        Self {
            document,
            parents,
            positions,
            picture_sources,
            environment: environment.bounded(),
        }
    }

    pub fn select(&self, node: usize) -> Selection {
        let mut result = Selection::default();
        if !self.tag(node, "img") {
            result.image = self.default_source(node);
            return result;
        }
        if let Some(sources) = self
            .parents
            .get(node)
            .copied()
            .flatten()
            .and_then(|p| self.picture_sources.get(&p))
        {
            for (examined, &(source, position)) in sources.iter().enumerate() {
                if position >= self.positions[node] {
                    break;
                }
                if examined == MAX_SOURCES {
                    note(
                        &mut result.diagnostics,
                        "Picture source limit reached (64); using the img source set.",
                    );
                    break;
                }
                let Some(value) = self.document.attr(source, "srcset") else {
                    continue;
                };
                if let Some(mime) = self.document.attr(source, "type") {
                    if !supported_mime(mime) {
                        note(
                            &mut result.diagnostics,
                            "A picture source has an unsupported image MIME type and was skipped.",
                        );
                        continue;
                    }
                }
                if let Some(media) = self.document.attr(source, "media") {
                    match media_matches(media, self.environment) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(()) => {
                            note(
                                &mut result.diagnostics,
                                "Unsupported or invalid picture media condition skipped.",
                            );
                            continue;
                        }
                    }
                }
                let candidates = parse_srcset(value, &mut result.diagnostics);
                if candidates.is_empty() {
                    continue;
                }
                let size = source_size(
                    self.document.attr(source, "sizes").unwrap_or(""),
                    self.environment,
                    &mut result.diagnostics,
                );
                result.image = choose(candidates, size, self.environment.density, source, true);
                if result.image.is_some() {
                    return result;
                }
            }
        }
        let value = self.document.attr(node, "srcset").unwrap_or("");
        let mut candidates = parse_srcset(value, &mut result.diagnostics);
        let responsive = self.document.attr(node, "srcset").is_some();
        let has_width = candidates
            .iter()
            .any(|c| matches!(c.descriptor, Descriptor::Width(_)));
        let has_one = candidates.iter().any(|c| {
            matches!(c.descriptor, Descriptor::Density(d) if d == 1.0)
                || matches!(c.descriptor, Descriptor::None)
        });
        if !has_width && !has_one {
            if let Some(default) = self.default_source(node) {
                candidates.push(Candidate {
                    url: default.url,
                    descriptor: Descriptor::None,
                });
            }
        }
        let size = source_size(
            self.document.attr(node, "sizes").unwrap_or(""),
            self.environment,
            &mut result.diagnostics,
        );
        result.image = choose(candidates, size, self.environment.density, node, responsive);
        result
    }

    fn default_source(&self, node: usize) -> Option<SelectedImage> {
        let url = trim(self.document.attr(node, "src")?);
        if url.is_empty() {
            return None;
        }
        Some(SelectedImage {
            url: url.to_owned(),
            density: 1.0,
            source_node: node,
            responsive: false,
        })
    }

    fn tag(&self, node: usize, wanted: &str) -> bool {
        self.document
            .nodes
            .get(node)
            .is_some_and(|n| matches!(&n.kind,NodeKind::Element{tag,..} if tag==wanted))
    }
}

/// MIME capabilities of the actual decoder families; recognition of an
/// unsupported container in the file chooser is not advertised support here.
pub fn supported_mime(value: &str) -> bool {
    if value.len() > 1024 {
        return false;
    }
    let essence = trim(value.split(';').next().unwrap_or("")).to_ascii_lowercase();
    matches!(
        essence.as_str(),
        "image/png"
            | "image/apng"
            | "image/jpeg"
            | "image/jpg"
            | "image/pjpeg"
            | "image/gif"
            | "image/bmp"
            | "image/x-ms-bmp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
            | "image/svg+xml"
            | "image/webp"
            | "image/tiff"
            | "image/qoi"
            | "image/x-qoi"
            | "image/x-portable-bitmap"
            | "image/x-portable-graymap"
            | "image/x-portable-pixmap"
            | "image/x-portable-anymap"
            | "image/x-portable-arbitrarymap"
            | "image/x-tga"
            | "image/tga"
            | "image/x-pcx"
            | "image/pcx"
    )
}

fn whitespace(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}
fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_ascii() && whitespace(c as u8))
}

#[derive(Clone, Copy, Debug)]
enum Descriptor {
    None,
    Width(f64),
    Density(f64),
}
#[derive(Debug)]
struct Candidate {
    url: String,
    descriptor: Descriptor,
}

fn choose(
    candidates: Vec<Candidate>,
    size: f64,
    target: f64,
    source_node: usize,
    responsive: bool,
) -> Option<SelectedImage> {
    let mut normalized: Vec<(Candidate, f64)> = Vec::new();
    for candidate in candidates {
        let density = match candidate.descriptor {
            Descriptor::None => 1.0,
            Descriptor::Width(w) => w / size,
            Descriptor::Density(d) => d,
        };
        if !normalized.iter().any(|(_, d)| *d == density) {
            normalized.push((candidate, density));
        }
    }
    normalized.sort_by(|a, b| a.1.total_cmp(&b.1));
    let selected = normalized
        .iter()
        .position(|(_, d)| *d >= target)
        .unwrap_or(normalized.len().saturating_sub(1));
    if normalized.is_empty() {
        return None;
    }
    let (candidate, density) = normalized.swap_remove(selected);
    Some(SelectedImage {
        url: candidate.url,
        density,
        source_node,
        responsive,
    })
}

fn parse_srcset(input: &str, notes: &mut Vec<String>) -> Vec<Candidate> {
    if input.len() > MAX_ATTRIBUTE {
        note(
            notes,
            "Srcset exceeds 64 KiB; the candidate list was ignored.",
        );
        return Vec::new();
    }
    let bytes = input.as_bytes();
    let mut at = 0;
    let mut result = Vec::new();
    let mut attempted = 0;
    while at < bytes.len() {
        while bytes.get(at).is_some_and(|b| whitespace(*b) || *b == b',') {
            at += 1;
        }
        if at == bytes.len() {
            break;
        }
        attempted += 1;
        if attempted > MAX_CANDIDATES {
            note(
                notes,
                "Srcset candidate limit reached (128); remaining candidates were ignored.",
            );
            break;
        }
        let start = at;
        while bytes.get(at).is_some_and(|b| !whitespace(*b)) {
            at += 1;
        }
        let mut url = &input[start..at];
        let mut descriptors = Vec::new();
        let mut too_many = false;
        if url.ends_with(',') {
            url = url.trim_end_matches(',');
        } else {
            while bytes.get(at).is_some_and(|b| whitespace(*b)) {
                at += 1;
            }
            let mut begin = at;
            let mut parentheses = false;
            while at < bytes.len() {
                let b = bytes[at];
                if parentheses {
                    if b == b')' {
                        parentheses = false;
                    }
                    at += 1;
                } else if b == b'(' {
                    parentheses = true;
                    at += 1;
                } else if b == b',' || whitespace(b) {
                    if begin < at {
                        if descriptors.len() < 4 {
                            descriptors.push(&input[begin..at]);
                        } else {
                            too_many = true;
                        }
                    }
                    at += 1;
                    if b == b',' {
                        begin = at;
                        break;
                    }
                    while bytes.get(at).is_some_and(|b| whitespace(*b)) {
                        at += 1;
                    }
                    begin = at;
                } else {
                    at += 1;
                }
            }
            if begin < at && at == bytes.len() {
                if descriptors.len() < 4 {
                    descriptors.push(&input[begin..at]);
                } else {
                    too_many = true;
                }
            }
        }
        if !too_many && !url.is_empty() {
            if let Some(descriptor) = parse_descriptors(&descriptors) {
                result.push(Candidate {
                    url: url.to_owned(),
                    descriptor,
                });
                continue;
            }
        }
        note(notes, "One or more invalid srcset candidates were ignored.");
    }
    result
}

fn integer(value: &str) -> Option<f64> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let result = value.parse::<u64>().ok()?;
    (result > 0).then_some(result as f64)
}

fn html_float(value: &str) -> Option<f64> {
    let bytes = value.as_bytes();
    let mut at = usize::from(bytes.first() == Some(&b'-'));
    let first = at;
    while bytes.get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    let digits = at - first;
    if bytes.get(at) == Some(&b'.') {
        at += 1;
        let start = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        if at == start {
            return None;
        }
    } else if digits == 0 {
        return None;
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let start = at;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        if at == start {
            return None;
        }
    }
    if at != bytes.len() || value.len() > 128 {
        return None;
    }
    let value = value.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

fn parse_descriptors(values: &[&str]) -> Option<Descriptor> {
    let mut width = None;
    let mut density = None;
    let mut height = None;
    for value in values {
        if let Some(number) = value.strip_suffix('w') {
            if width.is_some() || density.is_some() {
                return None;
            }
            width = Some(integer(number)?);
        } else if let Some(number) = value.strip_suffix('x') {
            if width.is_some() || density.is_some() || height.is_some() {
                return None;
            }
            let n = html_float(number)?;
            if n < 0.0 {
                return None;
            }
            density = Some(n);
        } else {
            let number = value.strip_suffix('h')?;
            if height.is_some() || density.is_some() {
                return None;
            }
            height = Some(integer(number)?);
        }
    }
    if height.is_some() && width.is_none() {
        return None;
    }
    Some(width.map_or_else(
        || density.map_or(Descriptor::None, Descriptor::Density),
        Descriptor::Width,
    ))
}

/// Parenthesis-aware top-level split; no CSS strings are meaningful in the
/// supported media/size expressions. Escapes/quotes deliberately fail parsing.
fn split_top(text: &str, delimiter: u8) -> Result<Vec<&str>, ()> {
    let mut depth = 0usize;
    let mut start = 0;
    let mut out = Vec::new();
    for (at, b) in text.bytes().enumerate() {
        match b {
            b'(' => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(());
                }
            }
            b')' => {
                depth = depth.checked_sub(1).ok_or(())?;
            }
            b if b == delimiter && depth == 0 => {
                out.push(&text[start..at]);
                start = at + 1;
                if out.len() > MAX_CANDIDATES {
                    return Err(());
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(());
    }
    out.push(&text[start..]);
    Ok(out)
}

fn source_size(input: &str, env: Environment, notes: &mut Vec<String>) -> f64 {
    if input.is_empty() {
        return env.viewport_width;
    }
    if input.len() > MAX_EXPRESSION {
        note(
            notes,
            "Sizes exceeds 4096 bytes; using the default 100vw source size.",
        );
        return env.viewport_width;
    }
    let Ok(clean) = without_comments(input) else {
        note(
            notes,
            "Invalid sizes syntax; using the default 100vw source size.",
        );
        return env.viewport_width;
    };
    let Ok(parts) = split_top(&clean, b',') else {
        note(
            notes,
            "Invalid sizes syntax; using the default 100vw source size.",
        );
        return env.viewport_width;
    };
    for part in parts {
        let part = trim_css(part);
        if part.is_empty() {
            continue;
        }
        // The last CSS component is the source-size value. Functions form one
        // component even when their argument list contains spaces or commas.
        let mut depth = 0usize;
        let mut start = 0;
        for (at, b) in part.bytes().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 && at + 1 < part.len() {
                        start = at + 1;
                    }
                }
                _ if depth == 0 && (whitespace(b) || b == 1) => start = at + 1,
                _ => {}
            }
        }
        let size = trim_css(&part[start..]);
        let condition = trim_css(&part[..start]);
        if size.eq_ignore_ascii_case("auto") {
            note(
                notes,
                "Sizes auto requires lazy layout sizing; using the next size or 100vw.",
            );
            continue;
        }
        let Some(value) = length(size, env) else {
            note(notes, "Unsupported or invalid sizes value skipped.");
            continue;
        };
        if value < 0.0 {
            continue;
        }
        if condition.is_empty() {
            return value;
        }
        // sizes uses a media-condition, not a media type such as screen.
        let condition_tokens = tokens(condition);
        if !matches!(
            condition_tokens.as_ref().ok().and_then(|t| t.first()),
            Some(Token::Open)
        ) && !matches!(condition_tokens.as_ref().ok().and_then(|t|t.first()),Some(Token::Word(w)) if w=="not")
        {
            note(
                notes,
                "Unsupported or invalid sizes media condition skipped.",
            );
            continue;
        }
        match media_matches(condition, env) {
            Ok(true) => return value,
            Ok(false) => {}
            Err(()) => note(
                notes,
                "Unsupported or invalid sizes media condition skipped.",
            ),
        }
    }
    env.viewport_width
}

fn without_comments(input: &str) -> Result<String, ()> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("/*") {
        output.push_str(&rest[..start]);
        let end = rest[start + 2..].find("*/").ok_or(())?;
        // A comment separates tokens, but is not a whitespace token. Preserve
        // that distinction for calc's mandatory spacing around + and -.
        output.push('\x01');
        rest = &rest[start + 2 + end + 2..];
    }
    output.push_str(rest);
    Ok(output)
}

fn trim_css(input: &str) -> &str {
    input.trim_matches(|c: char| c == '\x01' || (c.is_ascii() && whitespace(c as u8)))
}

fn operator_space(bytes: &[u8], at: usize, before: bool) -> bool {
    if before {
        bytes[..at]
            .iter()
            .rev()
            .find(|&&b| b != 1)
            .is_some_and(|b| whitespace(*b))
    } else {
        bytes[at + 1..]
            .iter()
            .find(|&&b| b != 1)
            .is_some_and(|b| whitespace(*b))
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64, String),
    Word(String),
    Open,
    Close,
    Comma,
    Plus,
    Minus,
    Star,
    Slash,
    Colon,
    Compare(u8),
}

fn tokens(input: &str) -> Result<Vec<Token>, ()> {
    if input.len() > MAX_EXPRESSION || !input.is_ascii() {
        return Err(());
    }
    let input = without_comments(input)?;
    let b = input.as_bytes();
    let mut at = 0;
    let mut out = Vec::new();
    while at < b.len() {
        if whitespace(b[at]) || b[at] == 1 {
            at += 1;
            continue;
        }
        let signed_number = matches!(b[at], b'+' | b'-')
            && (b.get(at + 1).is_some_and(u8::is_ascii_digit)
                || (b.get(at + 1) == Some(&b'.') && b.get(at + 2).is_some_and(u8::is_ascii_digit)));
        let token = match b[at] {
            b'(' => {
                at += 1;
                Token::Open
            }
            b')' => {
                at += 1;
                Token::Close
            }
            b',' => {
                at += 1;
                Token::Comma
            }
            b'+' | b'-'
                if !signed_number && b.get(at + 1).is_none_or(|n| !n.is_ascii_alphabetic()) =>
            {
                // CSS arithmetic requires whitespace around binary + and -.
                // A sign adjoining a number instead belongs to that number.
                if !operator_space(b, at, true) || !operator_space(b, at, false) {
                    return Err(());
                }
                let plus = b[at] == b'+';
                at += 1;
                if plus {
                    Token::Plus
                } else {
                    Token::Minus
                }
            }
            b'*' => {
                at += 1;
                Token::Star
            }
            b'/' => {
                at += 1;
                Token::Slash
            }
            b':' => {
                at += 1;
                Token::Colon
            }
            b'<' | b'>' | b'=' => {
                let mut operator = b[at];
                at += 1;
                if operator != b'=' && b.get(at) == Some(&b'=') {
                    operator = if operator == b'<' { b'l' } else { b'g' };
                    at += 1;
                }
                Token::Compare(operator)
            }
            b'0'..=b'9' | b'.' | b'+' | b'-'
                if b[at].is_ascii_digit() || b[at] == b'.' || signed_number =>
            {
                let start = at;
                let mut digits = 0;
                if signed_number {
                    at += 1;
                }
                while b.get(at).is_some_and(u8::is_ascii_digit) {
                    at += 1;
                    digits += 1;
                }
                if b.get(at) == Some(&b'.') && b.get(at + 1).is_some_and(u8::is_ascii_digit) {
                    at += 1;
                    while b.get(at).is_some_and(u8::is_ascii_digit) {
                        at += 1;
                        digits += 1;
                    }
                }
                if digits == 0 {
                    return Err(());
                }
                if matches!(b.get(at), Some(b'e' | b'E')) {
                    let exponent = at;
                    let mut end = at + 1;
                    if matches!(b.get(end), Some(b'+' | b'-')) {
                        end += 1;
                    }
                    let first = end;
                    while b.get(end).is_some_and(u8::is_ascii_digit) {
                        end += 1;
                    }
                    if end > first {
                        at = end;
                    } else {
                        at = exponent;
                    }
                }
                let number = input[start..at].parse::<f64>().map_err(|_| ())?;
                if !number.is_finite() {
                    return Err(());
                }
                let start = at;
                while b.get(at).is_some_and(u8::is_ascii_alphabetic) {
                    at += 1;
                }
                Token::Number(number, input[start..at].to_ascii_lowercase())
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'-' => {
                let start = at;
                at += 1;
                while b
                    .get(at)
                    .is_some_and(|x| x.is_ascii_alphanumeric() || *x == b'-')
                {
                    at += 1;
                }
                let word = input[start..at].to_ascii_lowercase();
                // Function names require immediate '('; media keywords require
                // separation so that and(...) cannot masquerade as and (...).
                if (matches!(word.as_str(), "calc" | "min" | "max" | "clamp")
                    && b.get(at) != Some(&b'('))
                    || (matches!(
                        word.as_str(),
                        "not" | "and" | "or" | "only" | "screen" | "all" | "print"
                    ) && b.get(at) == Some(&b'('))
                {
                    return Err(());
                }
                Token::Word(word)
            }
            _ => return Err(()),
        };
        out.push(token);
        if out.len() > MAX_TOKENS {
            return Err(());
        }
    }
    Ok(out)
}

#[derive(Clone, Copy)]
struct Quantity {
    value: f64,
    length: bool,
}

struct Math<'a> {
    tokens: &'a [Token],
    at: usize,
    env: Environment,
}
impl Math<'_> {
    fn expression(&mut self, depth: usize) -> Option<Quantity> {
        if depth > MAX_DEPTH {
            return None;
        }
        let mut left = self.product(depth + 1)?;
        while let Some(token @ (Token::Plus | Token::Minus)) = self.tokens.get(self.at) {
            let add = *token == Token::Plus;
            self.at += 1;
            let right = self.product(depth + 1)?;
            if left.length != right.length {
                return None;
            }
            left.value = if add {
                left.value + right.value
            } else {
                left.value - right.value
            };
            if !left.value.is_finite() {
                return None;
            }
        }
        Some(left)
    }
    fn product(&mut self, depth: usize) -> Option<Quantity> {
        let mut left = self.atom(depth + 1)?;
        while let Some(token @ (Token::Star | Token::Slash)) = self.tokens.get(self.at) {
            let multiply = *token == Token::Star;
            self.at += 1;
            let right = self.atom(depth + 1)?;
            if multiply {
                if left.length && right.length {
                    return None;
                }
                left = Quantity {
                    value: left.value * right.value,
                    length: left.length || right.length,
                };
            } else {
                if right.length || right.value == 0.0 {
                    return None;
                }
                left.value /= right.value;
            }
            if !left.value.is_finite() {
                return None;
            }
        }
        Some(left)
    }
    fn atom(&mut self, depth: usize) -> Option<Quantity> {
        if depth > MAX_DEPTH {
            return None;
        }
        let token = self.tokens.get(self.at)?;
        self.at += 1;
        match token {
            Token::Number(value, unit) => {
                let factor = match unit.as_str() {
                    "" => 1.0,
                    "px" => 1.0,
                    "em" | "rem" => 16.0,
                    "vw" => self.env.viewport_width / 100.0,
                    "vh" => self.env.viewport_height / 100.0,
                    "vmin" => self.env.viewport_width.min(self.env.viewport_height) / 100.0,
                    "vmax" => self.env.viewport_width.max(self.env.viewport_height) / 100.0,
                    "in" => 96.0,
                    "cm" => 96.0 / 2.54,
                    "mm" => 96.0 / 25.4,
                    "q" => 96.0 / 101.6,
                    "pt" => 96.0 / 72.0,
                    "pc" => 16.0,
                    _ => return None,
                };
                let n = value * factor;
                n.is_finite().then_some(Quantity {
                    value: n,
                    length: !unit.is_empty(),
                })
            }
            Token::Open => {
                let n = self.expression(depth + 1)?;
                if self.tokens.get(self.at) != Some(&Token::Close) {
                    return None;
                }
                self.at += 1;
                Some(n)
            }
            Token::Word(name) if matches!(name.as_str(), "calc" | "min" | "max" | "clamp") => {
                if self.tokens.get(self.at) != Some(&Token::Open) {
                    return None;
                }
                self.at += 1;
                let mut values = vec![self.expression(depth + 1)?];
                while self.tokens.get(self.at) == Some(&Token::Comma) {
                    self.at += 1;
                    values.push(self.expression(depth + 1)?);
                    if values.len() > 32 {
                        return None;
                    }
                }
                if self.tokens.get(self.at) != Some(&Token::Close) {
                    return None;
                }
                self.at += 1;
                if values.iter().any(|v| v.length != values[0].length) {
                    return None;
                }
                let value = match name.as_str() {
                    "calc" if values.len() == 1 => values[0].value,
                    "min" => values.iter().map(|v| v.value).fold(f64::INFINITY, f64::min),
                    "max" => values
                        .iter()
                        .map(|v| v.value)
                        .fold(f64::NEG_INFINITY, f64::max),
                    "clamp" if values.len() == 3 => {
                        values[0].value.max(values[1].value.min(values[2].value))
                    }
                    _ => return None,
                };
                Some(Quantity {
                    value,
                    length: values[0].length,
                })
            }
            _ => None,
        }
    }
}

fn length(text: &str, env: Environment) -> Option<f64> {
    let values = tokens(text).ok()?;
    if values.is_empty() {
        return None;
    }
    length_tokens(&values, env, true)
}

fn length_tokens(values: &[Token], env: Environment, nonnegative: bool) -> Option<f64> {
    // Outside a math function a length is one dimension (or bare zero), not
    // an arbitrary expression or parenthesized sum.
    if !matches!(values.first(), Some(Token::Number(..) | Token::Word(_))) {
        return None;
    }
    let function = matches!(values.first(), Some(Token::Word(_)));
    let mut parser = Math {
        tokens: values,
        at: 0,
        env,
    };
    let mut value = parser.atom(0)?;
    if parser.at != values.len() || !value.value.is_finite() || value.value.abs() > 1_000_000.0 {
        return None;
    }
    if nonnegative && value.value < 0.0 {
        if !function {
            return None;
        }
        // Range-checking is deferred inside CSS math and clamps the final
        // result, while a negative literal is an invalid source size.
        value.value = 0.0;
    }
    // A bare unitless zero is a valid length. calc(0) remains a number.
    (value.length || values == [Token::Number(0.0, String::new())]).then_some(value.value)
}

fn media_matches(input: &str, env: Environment) -> Result<bool, ()> {
    if trim(input).is_empty() {
        return Ok(true);
    }
    let tokens = tokens(input)?;
    if tokens.is_empty() {
        return Ok(true);
    }
    let mut start = 0;
    let mut depth = 0;
    let mut matched = false;
    let mut unsupported = false;
    for at in 0..=tokens.len() {
        match tokens.get(at) {
            Some(Token::Open) => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(());
                }
            }
            Some(Token::Close) => {
                depth = depth.checked_sub(1).ok_or(())?;
            }
            Some(Token::Comma) if depth == 0 => {}
            None if depth == 0 => {}
            None => return Err(()),
            _ => continue,
        }
        if at < tokens.len() && tokens[at] != Token::Comma {
            continue;
        }
        match query(&tokens[start..at], env) {
            Ok(value) => matched |= value,
            Err(()) => unsupported = true,
        }
        start = at + 1;
    }
    if matched {
        Ok(true)
    } else if unsupported {
        Err(())
    } else {
        Ok(false)
    }
}

fn query(tokens: &[Token], env: Environment) -> Result<bool, ()> {
    if tokens.is_empty() {
        return Err(());
    }
    let mut at = 0;
    let mut negate = false;
    let is_word =
        |at: usize, wanted: &str| matches!(tokens.get(at),Some(Token::Word(w)) if w==wanted);
    if is_word(at, "not")
        && matches!(tokens.get(at+1),Some(Token::Word(w)) if !matches!(w.as_str(),"not"|"only"|"and"|"or"))
    {
        negate = true;
        at += 1;
    }
    let only = is_word(at, "only");
    if only {
        at += 1;
    }
    let kind = match tokens.get(at) {
        Some(Token::Word(word)) if word != "not" => Some(word.as_str()),
        _ => None,
    };
    let mut value = true;
    if let Some(kind) = kind {
        value = match kind {
            "screen" | "all" => true,
            "print" => false,
            _ => return Err(()),
        };
        at += 1;
        if at < tokens.len() {
            if !is_word(at, "and") {
                return Err(());
            }
            at += 1;
        } else {
            return Ok(if negate { !value } else { value });
        }
    } else if only {
        return Err(());
    }
    let mut condition = Condition { tokens, at, env };
    value &= condition.expression(0, kind.is_none())?;
    if condition.at != tokens.len() {
        return Err(());
    }
    Ok(if negate { !value } else { value })
}

struct Condition<'a> {
    tokens: &'a [Token],
    at: usize,
    env: Environment,
}
impl Condition<'_> {
    fn expression(&mut self, depth: usize, allow_or: bool) -> Result<bool, ()> {
        if depth > MAX_DEPTH {
            return Err(());
        }
        if matches!(self.tokens.get(self.at),Some(Token::Word(w))if w=="not") {
            self.at += 1;
            // A media-not is one complete condition; additional terms require
            // grouping, e.g. (not (width:1px)) and (height:600px).
            return Ok(!self.atom(depth + 1)?);
        }
        let mut value = self.atom(depth + 1)?;
        let mut operator = None;
        loop {
            let op = match self.tokens.get(self.at) {
                Some(Token::Word(w)) if w == "and" => true,
                Some(Token::Word(w)) if w == "or" => false,
                _ => break,
            };
            if !op && !allow_or {
                return Err(());
            }
            if operator.is_some_and(|old| old != op) {
                return Err(());
            }
            operator = Some(op);
            self.at += 1;
            let next = self.atom(depth + 1)?;
            value = if op { value && next } else { value || next };
        }
        Ok(value)
    }
    fn atom(&mut self, depth: usize) -> Result<bool, ()> {
        if depth > MAX_DEPTH {
            return Err(());
        }
        if self.tokens.get(self.at) != Some(&Token::Open) {
            return Err(());
        }
        self.at += 1;
        let start = self.at;
        let mut level = 1;
        while level > 0 {
            match self.tokens.get(self.at) {
                Some(Token::Open) => level += 1,
                Some(Token::Close) => level -= 1,
                None => return Err(()),
                _ => {}
            }
            if level > 0 {
                self.at += 1;
            }
        }
        let end = self.at;
        self.at += 1;
        if matches!(self.tokens.get(start), Some(Token::Open))
            || matches!(self.tokens.get(start),Some(Token::Word(w)) if w=="not")
        {
            let mut inner = Condition {
                tokens: &self.tokens[start..end],
                at: 0,
                env: self.env,
            };
            let result = inner.expression(depth + 1, true)?;
            if inner.at != inner.tokens.len() {
                return Err(());
            }
            Ok(result)
        } else {
            feature(&self.tokens[start..end], self.env)
        }
    }
}

fn compare(left: f64, op: u8, right: f64) -> bool {
    match op {
        b'<' => left < right,
        b'>' => left > right,
        b'l' => left <= right,
        b'g' => left >= right,
        b'=' => left == right,
        _ => false,
    }
}

fn feature(tokens: &[Token], env: Environment) -> Result<bool, ()> {
    let Some(first) = tokens.first() else {
        return Err(());
    };
    if let Token::Word(name) = first {
        let (name, operator) = if let Some(s) = name.strip_prefix("min-") {
            (s, b'g')
        } else if let Some(s) = name.strip_prefix("max-") {
            (s, b'l')
        } else {
            (name.as_str(), b'=')
        };
        if tokens.len() == 1 {
            if operator != b'=' {
                return Err(());
            }
            return match name {
                "width" => Ok(env.viewport_width > 0.0),
                "height" => Ok(env.viewport_height > 0.0),
                "resolution" => Ok(env.density > 0.0),
                "aspect-ratio" | "orientation" => Ok(true),
                _ => Err(()),
            };
        }
        if tokens.get(1) == Some(&Token::Colon) {
            if name == "orientation" && operator == b'=' {
                return match &tokens[2..] {
                    [Token::Word(w)] if w == "landscape" => {
                        Ok(env.viewport_width > env.viewport_height)
                    }
                    [Token::Word(w)] if w == "portrait" => {
                        Ok(env.viewport_width <= env.viewport_height)
                    }
                    _ => Err(()),
                };
            }
            return Ok(compare(
                feature_actual(name, env)?,
                operator,
                feature_value(name, &tokens[2..], env)?,
            ));
        }
        if operator != b'=' {
            return Err(());
        }
    }
    // Range contexts: width >= 500px, 500px < width, and 20em < width <= 70em.
    let comparisons: Vec<_> = tokens
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            if let Token::Compare(op) = t {
                Some((i, *op))
            } else {
                None
            }
        })
        .collect();
    if comparisons.is_empty() || comparisons.len() > 2 {
        return Err(());
    }
    let at = match comparisons.as_slice() {
        [(separator, _)] if *separator == 1 && matches!(tokens.first(), Some(Token::Word(_))) => 0,
        [(separator, _)] if *separator + 2 == tokens.len() => tokens.len() - 1,
        [(left, _), (right, _)] if *right == left + 2 => left + 1,
        _ => return Err(()),
    };
    let Token::Word(name) = &tokens[at] else {
        return Err(());
    };
    let actual = feature_actual(name, env)?;
    match comparisons.as_slice() {
        [(separator, op)] if at == 0 && *separator == 1 => Ok(compare(
            actual,
            *op,
            feature_value(name, &tokens[2..], env)?,
        )),
        [(separator, op)] if at == tokens.len() - 1 && *separator == at - 1 => Ok(compare(
            feature_value(name, &tokens[..*separator], env)?,
            *op,
            actual,
        )),
        [(left, lop), (right, rop)] if at == left + 1 && *right == at + 1 => {
            if !matches!(
                (*lop, *rop),
                (b'<' | b'l', b'<' | b'l') | (b'>' | b'g', b'>' | b'g')
            ) {
                return Err(());
            }
            Ok(
                compare(feature_value(name, &tokens[..*left], env)?, *lop, actual)
                    && compare(
                        actual,
                        *rop,
                        feature_value(name, &tokens[right + 1..], env)?,
                    ),
            )
        }
        _ => Err(()),
    }
}

fn feature_actual(name: &str, env: Environment) -> Result<f64, ()> {
    match name {
        "width" => Ok(env.viewport_width),
        "height" => Ok(env.viewport_height),
        "aspect-ratio" => Ok(env.viewport_width / env.viewport_height),
        "resolution" => Ok(env.density),
        _ => Err(()),
    }
}

fn feature_value(name: &str, tokens: &[Token], env: Environment) -> Result<f64, ()> {
    if name == "resolution" {
        let [Token::Number(value, unit)] = tokens else {
            return Err(());
        };
        if *value <= 0.0 {
            return Err(());
        }
        return match unit.as_str() {
            "dppx" | "x" => Ok(*value),
            "dpi" => Ok(*value / 96.0),
            "dpcm" => Ok(*value * 2.54 / 96.0),
            _ => Err(()),
        };
    }
    if name == "aspect-ratio" {
        let ratio = match tokens {
            [Token::Number(value, unit)] if unit.is_empty() => *value,
            [Token::Number(a, au), Token::Slash, Token::Number(b, bu)]
                if au.is_empty() && bu.is_empty() && *b > 0.0 =>
            {
                a / b
            }
            _ => return Err(()),
        };
        return (ratio >= 0.0 && ratio.is_finite())
            .then_some(ratio)
            .ok_or(());
    }
    length_tokens(tokens, env, false).ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn selected(markup: &str, width: f64, density: f64) -> Selection {
        let doc = crate::html::parse(markup);
        let node = doc
            .nodes
            .iter()
            .position(|n| matches!(&n.kind,NodeKind::Element{tag,..}if tag=="img"))
            .unwrap();
        Selector::new(
            &doc,
            Environment {
                viewport_width: width,
                viewport_height: 600.0,
                density,
            },
        )
        .select(node)
    }
    fn result(markup: &str, width: f64, density: f64) -> (String, f64) {
        let image = selected(markup, width, density).image.unwrap();
        (image.url, image.density)
    }

    #[test]
    fn picture_skips_unsupported_types_and_conditions_then_uses_first_matching_source() {
        let source="<picture><source type=image/avif srcset=unsupported.avif><source media='(max-width:500px)' srcset='small.webp 1x, small2.webp 2x' type=image/webp><source media='(prefers-color-scheme:dark)' srcset=unknown.webp><source type=image/png srcset=wide.png><img src=fallback.jpg><source srcset=too-late.png></picture>";
        assert_eq!(result(source, 480.0, 2.0), ("small2.webp".into(), 2.0));
        assert_eq!(result(source, 900.0, 1.0), ("wide.png".into(), 1.0));
        assert!(!selected(source, 900.0, 1.0).diagnostics.is_empty());
        assert_eq!(result("<picture><source srcset='bad.png -2x'><span><img src=fallback.png></span></picture>",400.0,1.0).0,"fallback.png");
    }

    #[test]
    fn srcset_density_width_default_and_duplicate_rules() {
        assert_eq!(
            result(
                "<img src=one.png srcset='two.png 2x, three.png 3x'>",
                500.0,
                1.0
            ),
            ("one.png".into(), 1.0)
        );
        assert_eq!(
            result(
                "<img src=unused.png srcset='one.png, duplicate.png 1x, two.png 2x'>",
                500.0,
                1.0
            )
            .0,
            "one.png"
        );
        assert_eq!(result("<img src=unused.png srcset='small.png 400w, medium.png 800w, large.png 1600w' sizes='50vw'>",800.0,1.5),("medium.png".into(),2.0));
        assert_eq!(
            result(
                "<img src=unused.png srcset='small.png 400w, large.png 800w'>",
                1000.0,
                2.0
            ),
            ("large.png".into(), 0.8)
        );
        assert_eq!(
            result(
                "<img src=fallback.png srcset='broken.png 100w 2x, okay.png 2x'>",
                500.0,
                1.0
            )
            .0,
            "fallback.png"
        );
        assert_eq!(
            result(
                "<img src=fallback.png srcset='candidate.png 1.'>",
                500.0,
                1.0
            )
            .0,
            "fallback.png"
        );
    }

    #[test]
    fn comma_urls_and_future_height_descriptors_follow_the_tokenizer() {
        assert_eq!(
            result(
                "<img srcset='data:image/png;base64,AAAA 1x, two.png 2x'>",
                500.0,
                1.0
            )
            .0,
            "data:image/png;base64,AAAA"
        );
        assert_eq!(
            result(
                "<img srcset='one.png,two.png 1x, three.png 2x'>",
                500.0,
                1.0
            )
            .0,
            "one.png,two.png"
        );
        assert_eq!(
            result("<img srcset='a.png,,, b.png 2x'>", 500.0, 1.0).0,
            "a.png"
        );
        assert_eq!(
            result(
                "<img srcset='a.png nonsense(with,comma), b.png 2x'>",
                500.0,
                1.0
            )
            .0,
            "b.png"
        );
        assert_eq!(
            result(
                "<img srcset='a.png 400w 300h, b.png 2x' sizes=400px>",
                500.0,
                1.0
            )
            .0,
            "a.png"
        );
        assert_eq!(
            result("<img src=fallback srcset='a.png 300h'>", 500.0, 1.0).0,
            "fallback"
        );
    }

    #[test]
    fn sizes_media_math_and_fallbacks() {
        let image="<img srcset='small.png 400w, middle.png 800w, large.png 1600w' sizes='(max-width:30em) 100vw, (width <= 50em) calc(50vw - 20px), min(800px, 100vw)'>";
        assert_eq!(result(image, 400.0, 1.0).0, "small.png");
        assert_eq!(result(image, 800.0, 2.0).0, "middle.png");
        assert_eq!(result(image, 1200.0, 1.0).0, "middle.png");
        let mut notes = Vec::new();
        let env = Environment::default();
        assert_eq!(
            source_size(
                "auto, (min-width:1px) clamp(100px, 50vw, 600px)",
                env,
                &mut notes
            ),
            512.0
        );
        assert_eq!(
            source_size("(unknown:1) 10px, bad(20px), 75vw", env, &mut notes),
            768.0
        );
        assert_eq!(
            source_size("50%, var(--size), -20px", env, &mut notes),
            1024.0
        );
        assert_eq!(
            source_size("calc(100vw / 2 - 20px)", env, &mut notes),
            492.0
        );
    }

    #[test]
    fn media_boolean_ranges_resolution_and_unknowns() {
        let env = Environment {
            viewport_width: 800.0,
            viewport_height: 600.0,
            density: 2.0,
        };
        for input in [
            "screen and (min-width:40em)",
            "(400px < width <= 800px)",
            "(width >= 800px)",
            "(aspect-ratio:4/3)",
            "(resolution:192dpi)",
            "(orientation:landscape)",
            "not print",
            "print, (min-resolution:2dppx)",
            "((width:800px) and (height:600px))",
        ] {
            assert_eq!(media_matches(input, env), Ok(true), "{input}");
        }
        for input in [
            "print",
            "(width > 800px)",
            "(orientation:portrait)",
            "not screen and (width:800px)",
        ] {
            assert_eq!(media_matches(input, env), Ok(false), "{input}");
        }
        for input in [
            "not (unsupported:1)",
            "(width:800px) garbage",
            "screen or (width:1px)",
            "(width:800px) and (height:600px) or (width:1px)",
        ] {
            assert!(media_matches(input, env).is_err(), "{input}");
        }
    }

    #[test]
    fn zero_density_and_zero_sizes_remain_explicit_and_limits_are_bounded() {
        assert_eq!(result("<img srcset='zero.png 0x'>", 500.0, 1.0).1, 0.0);
        assert!(result("<img srcset='zero.png 400w' sizes=0px>", 500.0, 1.0)
            .1
            .is_infinite());
        let huge = "<img src=fallback srcset='".to_owned() + &"x".repeat(MAX_ATTRIBUTE + 1) + "'>";
        assert_eq!(result(&huge, 500.0, 1.0).0, "fallback");
        let input = (0..200)
            .map(|n| format!("{n}.png {}x", n + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let mut notes = Vec::new();
        assert_eq!(parse_srcset(&input, &mut notes).len(), 128);
        assert!(!notes.is_empty());
        assert!(!supported_mime("image/avif"));
        assert!(supported_mime(" IMAGE/WEBP; test=value "));
    }
}
