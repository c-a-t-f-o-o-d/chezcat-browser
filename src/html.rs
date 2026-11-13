//! A small HTML subset parser. It builds an arena of nodes without recursion.
//!
//! This is not a complete HTML tree builder: table recovery, foreign content,
//! and the full named-character-reference table are not implemented.
//! Common Latin-1 and punctuation references use a hand-selected name table.
//! Numeric references, legacy name matching, and attribute ambiguity follow
//! https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state
//! An unfinished tag consumes the rest of the input. Reaching either tree limit
//! returns the prefix already parsed, rather than flattening deeper content.

use std::borrow::Cow;
use std::collections::HashSet;

const MAX_NODES: usize = 20_000;
const MAX_DEPTH: usize = 128; // Open-element stack, including the synthetic root.

#[derive(Debug)]
pub struct Document {
    pub title: String,
    /// The committed resource's MIME essence. Synthetic text/image viewers
    /// retain the source type; plain parser callers create HTML documents.
    pub content_type: String,
    pub nodes: Vec<Node>,
    /// Set at commit after resource policy is known. The current tokenizer still
    /// parses noscript markup; consumers hide/inactivate that subtree when true.
    /// Full HTML scripting-flag tokenization is not implemented yet.
    pub scripting_enabled: bool,
    /// CSP nonceability needs duplicate-attribute evidence even though normal
    /// HTML attribute lookup keeps only the first occurrence.
    pub nonce_unsafe_nodes: HashSet<usize>,
}

impl Document {
    pub fn attr(&self, node: usize, name: &str) -> Option<&str> {
        // HTML consumers accept a case-insensitive query, but only the exact
        // lower-case null-namespace attribute drives native HTML behavior.
        let name = if name.bytes().any(|c| c.is_ascii_uppercase()) {
            Cow::Owned(name.to_ascii_lowercase())
        } else { Cow::Borrowed(name) };
        match &self.nodes.get(node)?.kind {
            NodeKind::Element { attrs, .. } => attrs
                .iter()
                .find(|attr| attr.name.html_is(name.as_ref()))
                .map(|attr| attr.value.as_str()),
            NodeKind::Text(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<usize>,
    /// Persistent provenance; foreign-content tree building is unsupported.
    pub namespace: Namespace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace {
    None,
    Html,
    UnknownForeign,
}

#[derive(Debug)]
pub struct AttributeName {
    pub qualified: String,
    pub namespace: Option<AttributeNamespace>,
}

#[derive(Debug)]
pub struct AttributeNamespace {
    pub uri: String,
    pub prefix: Option<String>,
    pub local: String,
}

impl AttributeName {
    pub fn unqualified(qualified: String) -> Self { Self { qualified, namespace: None } }
    pub fn namespace_uri(&self) -> Option<&str> { self.namespace.as_ref().map(|ns| ns.uri.as_str()) }
    pub fn prefix(&self) -> Option<&str> { self.namespace.as_ref().and_then(|ns| ns.prefix.as_deref()) }
    pub fn local(&self) -> &str { self.namespace.as_ref().map_or(&self.qualified, |ns| &ns.local) }
    pub fn html_is(&self, name: &str) -> bool { self.namespace.is_none() && self.qualified == name }
    pub fn matches(&self, namespace: Option<&str>, local: &str) -> bool {
        self.namespace_uri() == namespace && self.local() == local
    }
    pub fn same_expanded_name(&self, other: &Self) -> bool { self.matches(other.namespace_uri(), other.local()) }
    pub fn bytes(&self) -> usize {
        self.qualified.len() + self.namespace.as_ref().map_or(0, |ns| ns.uri.len() + ns.local.len() + ns.prefix.as_ref().map_or(0, String::len))
    }
    // Callers charge bytes before cloning retained native metadata.
    pub fn copy(&self) -> Self {
        Self { qualified: self.qualified.clone(), namespace: self.namespace.as_ref().map(|ns|
            AttributeNamespace { uri: ns.uri.clone(), prefix: ns.prefix.clone(), local: ns.local.clone() }) }
    }
}

#[derive(Debug)]
pub struct Attribute {
    pub name: AttributeName,
    pub value: String,
}

impl Attribute {
    pub fn unqualified(name: String, value: String) -> Self { Self { name: AttributeName::unqualified(name), value } }
    pub fn bytes(&self) -> usize { self.name.bytes() + self.value.len() }
}

#[derive(Debug)]
pub enum NodeKind {
    Element {
        tag: String,
        id: Option<String>,
        href: Option<String>,
        attrs: Vec<Attribute>,
    },
    Text(String),
}

pub fn parse(input: &str) -> Document {
    // HTML input preprocessing happens before tokenization, so an LF produced
    // by a numeric reference remains distinct from a literal source CR.
    let normalized = if input.contains('\r') {
        Cow::Owned(input.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(input)
    };
    let input = normalized.as_ref();
    let mut document = Document {
        title: String::new(),
        content_type: "text/html".into(),
        scripting_enabled: false,
        nonce_unsafe_nodes: HashSet::new(),
        nodes: vec![Node {
            kind: NodeKind::Element {
                tag: "#document".into(),
                id: None,
                href: None,
                attrs: Vec::new(),
            },
            children: Vec::new(),
            namespace: Namespace::None,
        }],
    };
    let mut stack = vec![0];
    let mut position = 0;
    let mut title_seen = false;
    // If unsupported foreign recovery escapes through an unrelated ancestor,
    // later elements cannot safely be classified from this subset's stack.
    let mut uncertain_foreign_recovery = false;

    while position < input.len() {
        let parent = *stack.last().unwrap();
        let rest = &input[position..];
        if !rest.starts_with('<') {
            let end = position + rest.find('<').unwrap_or(rest.len());
            if !append_text(&mut document, parent, &input[position..end]) {
                break;
            }
            position = end;
            continue;
        }

        if let Some(comment) = rest.strip_prefix("<!--") {
            position = match comment.find("-->") {
                Some(end) => position + 4 + end + 3,
                None => input.len(),
            };
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            position += rest.find('>').map_or(rest.len(), |end| end + 1);
            continue;
        }

        let tag = match read_tag(input, position) {
            TagToken::Complete(tag) => tag,
            TagToken::Incomplete => break,
            TagToken::NotATag => {
                if !append_text(&mut document, parent, "<") {
                    break;
                }
                position += 1;
                continue;
            }
        };
        position = tag.end;

        if tag.closing {
            if let Some(index) = stack.iter().rposition(|&node| {
                matches!(&document.nodes[node].kind,
                    NodeKind::Element { tag: name, .. } if name == &tag.name)
            }) {
                if index > 0 {
                    if document.nodes[*stack.last().unwrap()].namespace == Namespace::UnknownForeign
                        && document.nodes[stack[index]].namespace != Namespace::UnknownForeign
                    {
                        uncertain_foreign_recovery = true;
                    }
                    stack.truncate(index);
                }
            }
            continue;
        }

        let was_foreign = document.nodes[*stack.last().unwrap()].namespace == Namespace::UnknownForeign;
        close_implied(&mut stack, &document, &tag.name);
        let parent = *stack.last().unwrap();
        if was_foreign && document.nodes[parent].namespace != Namespace::UnknownForeign {
            uncertain_foreign_recovery = true;
        }
        let raw = matches!(tag.name.as_str(), "script" | "style" | "title" | "textarea");
        let void = is_void(&tag.name);
        if document.nodes.len() == MAX_NODES || (!raw && !void && stack.len() == MAX_DEPTH) {
            break;
        }
        let node = document.nodes.len();
        if matches!(tag.name.as_str(), "script" | "style" | "link") && tag.duplicate_attributes {
            document.nonce_unsafe_nodes.insert(node);
        }
        document.nodes[parent].children.push(node);
        document.nodes.push(Node {
            namespace: if uncertain_foreign_recovery
                || matches!(tag.name.as_str(), "svg" | "math")
                || document.nodes[parent].namespace == Namespace::UnknownForeign
            { Namespace::UnknownForeign } else { Namespace::Html },
            kind: NodeKind::Element {
                tag: tag.name.clone(),
                id: tag.id,
                href: tag.href,
                attrs: tag.attrs.into_iter().map(|(name, value)| Attribute::unqualified(name, value)).collect(),
            },
            children: Vec::new(),
        });

        if raw {
            let (content_end, after_end_tag) = raw_text_end(input, position, &tag.name);
            if matches!(tag.name.as_str(), "title" | "textarea") {
                // RCDATA recognizes only its own end tag as markup. Decode
                // entities after finding that tag so '&lt;/textarea&gt;' stays
                // text rather than prematurely closing the element.
                let mut content =
                    decode_entities(&input[position..content_end]).replace('\0', "\u{fffd}");
                if tag.name == "textarea" && content.starts_with('\n') {
                    content.remove(0);
                }
                if tag.name == "title" && !title_seen {
                    title_seen = true;
                    // Keep a useful window title even when the source spans lines.
                    document.title = content
                        .split_ascii_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                }
                if !append_text_value(&mut document, node, content) {
                    break;
                }
            } else if matches!(tag.name.as_str(), "style" | "script")
                && !append_text_value(&mut document, node, input[position..content_end].to_owned())
            {
                break;
            }
            // CSS and classic JavaScript remain raw text. Their consumers apply
            // the resource/execution policy; neither is rendered as page text.
            position = after_end_tag;
        } else if !void {
            // A trailing slash does not close a non-void HTML element.
            stack.push(node);
        }
    }

    document
}

fn append_text(document: &mut Document, parent: usize, source: &str) -> bool {
    append_text_value(document, parent, decode_entities(source))
}

fn append_text_value(document: &mut Document, parent: usize, text: String) -> bool {
    if text.is_empty() {
        return true;
    }
    if let Some(&last) = document.nodes[parent].children.last() {
        if let NodeKind::Text(previous) = &mut document.nodes[last].kind {
            previous.push_str(&text);
            return true;
        }
    }
    if document.nodes.len() == MAX_NODES {
        return false;
    }
    let node = document.nodes.len();
    document.nodes[parent].children.push(node);
    document.nodes.push(Node {
        kind: NodeKind::Text(text),
        children: Vec::new(),
        namespace: Namespace::None,
    });
    true
}

struct Tag {
    name: String,
    id: Option<String>,
    href: Option<String>,
    attrs: Vec<(String, String)>,
    closing: bool,
    end: usize,
    duplicate_attributes: bool,
}

enum TagToken {
    Complete(Tag),
    Incomplete,
    NotATag,
}

fn read_tag(input: &str, start: usize) -> TagToken {
    let bytes = input.as_bytes();
    let mut position = start + 1;
    let closing = bytes.get(position) == Some(&b'/');
    if closing {
        position += 1;
    }
    if !bytes.get(position).is_some_and(u8::is_ascii_alphabetic) {
        return TagToken::NotATag;
    }
    let name_start = position;
    while position < bytes.len()
        && (bytes[position].is_ascii_alphanumeric()
            || matches!(bytes[position], b'-' | b'_' | b':'))
    {
        position += 1;
    }
    let name = input[name_start..position].to_ascii_lowercase();
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    let mut duplicate_attributes = false;

    while position < bytes.len() {
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        if position == bytes.len() {
            break;
        }
        if bytes[position] == b'>' {
            return TagToken::Complete(Tag {
                name,
                id: attrs
                    .iter()
                    .find(|(name, _)| name == "id")
                    .map(|(_, value)| value.clone()),
                href: attrs
                    .iter()
                    .find(|(name, _)| name == "href")
                    .map(|(_, value)| value.clone()),
                attrs,
                closing,
                end: position + 1,
                duplicate_attributes,
            });
        }
        if bytes[position] == b'/' {
            position += 1;
            continue;
        }

        let attribute_start = position;
        while position < bytes.len()
            && !bytes[position].is_ascii_whitespace()
            && !matches!(bytes[position], b'=' | b'/' | b'>')
        {
            position += 1;
        }
        if position == attribute_start {
            // A stray '=' must still advance the parser.
            position += 1;
            continue;
        }
        let attribute = &input[attribute_start..position];
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }

        let mut value = "";
        if bytes.get(position) == Some(&b'=') {
            position += 1;
            while position < bytes.len() && bytes[position].is_ascii_whitespace() {
                position += 1;
            }
            if matches!(bytes.get(position), Some(b'\'' | b'"')) {
                let quote = bytes[position];
                position += 1;
                let value_start = position;
                while position < bytes.len() && bytes[position] != quote {
                    position += 1;
                }
                value = &input[value_start..position];
                if position < bytes.len() {
                    position += 1;
                }
            } else {
                let value_start = position;
                while position < bytes.len()
                    && !bytes[position].is_ascii_whitespace()
                    && bytes[position] != b'>'
                {
                    position += 1;
                }
                value = &input[value_start..position];
            }
        }

        // Hashing avoids quadratic scans when a tag contains many attributes.
        let attribute = attribute.to_ascii_lowercase();
        if seen.insert(attribute.clone()) {
            attrs.push((attribute, decode_entities_in(value, true)));
        } else {
            duplicate_attributes = true;
        }
    }

    TagToken::Incomplete
}

fn close_implied(stack: &mut Vec<usize>, document: &Document, incoming: &str) {
    if matches!(
        incoming,
        "p" | "div"
            | "center"
            | "table"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "li"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "main"
            | "blockquote"
            | "pre"
            | "hr"
    ) {
        close_in_scope(
            stack,
            document,
            "p",
            &["html", "table", "td", "th", "button"],
        );
    }
    if incoming == "li" {
        close_in_scope(stack, document, "li", &["ul", "ol", "html"]);
    }
    if matches!(incoming, "option" | "optgroup") {
        close_in_scope(
            stack,
            document,
            "option",
            &["select", "datalist", "optgroup", "html"],
        );
    }
    if incoming == "optgroup" {
        close_in_scope(stack, document, "optgroup", &["select", "datalist", "html"]);
    }
    if matches!(incoming, "td" | "th" | "tr" | "thead" | "tbody" | "tfoot") {
        close_in_scope(stack, document, "td", &["tr", "table", "html"]);
        close_in_scope(stack, document, "th", &["tr", "table", "html"]);
    }
    if matches!(incoming, "tr" | "thead" | "tbody" | "tfoot") {
        close_in_scope(stack, document, "tr", &["table", "html"]);
    }
    if matches!(incoming, "thead" | "tbody" | "tfoot") {
        for tag in ["thead", "tbody", "tfoot"] {
            close_in_scope(stack, document, tag, &["table", "html"]);
        }
    }
}

fn close_in_scope(stack: &mut Vec<usize>, document: &Document, target: &str, barriers: &[&str]) {
    for position in (1..stack.len()).rev() {
        if let NodeKind::Element { tag, .. } = &document.nodes[stack[position]].kind {
            if tag == target {
                stack.truncate(position);
                return;
            }
            if barriers.contains(&tag.as_str()) {
                return;
            }
        }
    }
}

fn raw_text_end(input: &str, start: usize, name: &str) -> (usize, usize) {
    let bytes = input.as_bytes();
    let mut position = start;
    while let Some(offset) = input[position..].find('<') {
        let opening = position + offset;
        let name_start = opening + 2;
        let name_end = name_start + name.len();
        if bytes.get(opening + 1) == Some(&b'/')
            && bytes
                .get(name_start..name_end)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name.as_bytes()))
            && bytes
                .get(name_end)
                .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>'))
        {
            return match read_tag(input, opening) {
                TagToken::Complete(tag) => (opening, tag.end),
                _ => (input.len(), input.len()),
            };
        }
        position = opening + 1;
    }
    (input.len(), input.len())
}

fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn decode_entities(input: &str) -> String {
    decode_entities_in(input, false)
}

fn decode_entities_in(input: &str, attribute: bool) -> String {
    let mut output = String::with_capacity(input.len());
    let mut position = 0;
    while let Some(offset) = input[position..].find('&') {
        let opening = position + offset;
        output.push_str(&input[position..opening]);
        let rest = &input[opening + 1..];
        if let Some((character, consumed)) = character_reference(rest, attribute) {
            output.push(character);
            position = opening + 1 + consumed;
            continue;
        }
        output.push('&');
        position = opening + 1;
    }
    output.push_str(&input[position..]);
    output
}

fn character_reference(input: &str, attribute: bool) -> Option<(char, usize)> {
    let bytes = input.as_bytes();
    if bytes.first() == Some(&b'#') {
        let mut position = 1;
        let radix = if matches!(bytes.get(position), Some(b'x' | b'X')) {
            position += 1;
            16
        } else {
            10
        };
        let first_digit = position;
        let mut number = 0u32;
        while let Some(digit) = bytes
            .get(position)
            .and_then(|byte| char::from(*byte).to_digit(radix))
        {
            // Continue consuming a long numeric reference once, even after it
            // overflows; saturation guarantees the replacement character.
            number = number.saturating_mul(radix).saturating_add(digit);
            position += 1;
        }
        if position == first_digit {
            return None;
        }
        if bytes.get(position) == Some(&b';') {
            position += 1;
        }
        return Some((numeric_character(number), position));
    }

    // A bounded longest match among our supported names. Names are case
    // sensitive. Only the legacy names below can omit their semicolon.
    let mut matched = None;
    for (index, byte) in bytes.iter().take(32).enumerate() {
        if !byte.is_ascii_alphanumeric() {
            break;
        }
        let end = index + 1;
        if let Some((character, legacy)) = named_character(&input[..end]) {
            if bytes.get(end) == Some(&b';') {
                return Some((character, end + 1));
            }
            if legacy {
                matched = Some((character, end));
            }
        }
    }
    let (character, consumed) = matched?;
    if attribute
        && bytes
            .get(consumed)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'=')
    {
        return None;
    }
    Some((character, consumed))
}

fn numeric_character(number: u32) -> char {
    // These are HTML's numeric-reference corrections, not byte decoding. The
    // undefined Windows-1252 positions (81, 8D, 8F, 90, 9D) remain controls.
    let corrected = match number {
        0 => 0xfffd,
        0x80 => 0x20ac,
        0x82 => 0x201a,
        0x83 => 0x0192,
        0x84 => 0x201e,
        0x85 => 0x2026,
        0x86 => 0x2020,
        0x87 => 0x2021,
        0x88 => 0x02c6,
        0x89 => 0x2030,
        0x8a => 0x0160,
        0x8b => 0x2039,
        0x8c => 0x0152,
        0x8e => 0x017d,
        0x91 => 0x2018,
        0x92 => 0x2019,
        0x93 => 0x201c,
        0x94 => 0x201d,
        0x95 => 0x2022,
        0x96 => 0x2013,
        0x97 => 0x2014,
        0x98 => 0x02dc,
        0x99 => 0x2122,
        0x9a => 0x0161,
        0x9b => 0x203a,
        0x9c => 0x0153,
        0x9e => 0x017e,
        0x9f => 0x0178,
        _ => number,
    };
    char::from_u32(corrected).unwrap_or('\u{fffd}')
}

fn named_character(name: &str) -> Option<(char, bool)> {
    // Hand-selected from the standard's name/code-point facts, not an imported
    // entity database. All printable Latin-1 names and common web punctuation
    // are included. Unlisted names are unsupported; a legacy prefix can still
    // match, as it does in HTML's longest-match algorithm.
    let legacy = match name {
        "amp" | "AMP" => '&',
        "lt" | "LT" => '<',
        "gt" | "GT" => '>',
        "quot" | "QUOT" => '"',
        "nbsp" => '\u{00a0}',
        "iexcl" => '¡',
        "cent" => '¢',
        "pound" => '£',
        "curren" => '¤',
        "yen" => '¥',
        "brvbar" => '¦',
        "sect" => '§',
        "uml" => '¨',
        "copy" | "COPY" => '©',
        "ordf" => 'ª',
        "laquo" => '«',
        "not" => '¬',
        "shy" => '\u{00ad}',
        "reg" | "REG" => '®',
        "macr" => '¯',
        "deg" => '°',
        "plusmn" => '±',
        "sup2" => '²',
        "sup3" => '³',
        "acute" => '´',
        "micro" => 'µ',
        "para" => '¶',
        "middot" => '·',
        "cedil" => '¸',
        "sup1" => '¹',
        "ordm" => 'º',
        "raquo" => '»',
        "frac14" => '¼',
        "frac12" => '½',
        "frac34" => '¾',
        "iquest" => '¿',
        "Agrave" => 'À',
        "Aacute" => 'Á',
        "Acirc" => 'Â',
        "Atilde" => 'Ã',
        "Auml" => 'Ä',
        "Aring" => 'Å',
        "AElig" => 'Æ',
        "Ccedil" => 'Ç',
        "Egrave" => 'È',
        "Eacute" => 'É',
        "Ecirc" => 'Ê',
        "Euml" => 'Ë',
        "Igrave" => 'Ì',
        "Iacute" => 'Í',
        "Icirc" => 'Î',
        "Iuml" => 'Ï',
        "ETH" => 'Ð',
        "Ntilde" => 'Ñ',
        "Ograve" => 'Ò',
        "Oacute" => 'Ó',
        "Ocirc" => 'Ô',
        "Otilde" => 'Õ',
        "Ouml" => 'Ö',
        "times" => '×',
        "Oslash" => 'Ø',
        "Ugrave" => 'Ù',
        "Uacute" => 'Ú',
        "Ucirc" => 'Û',
        "Uuml" => 'Ü',
        "Yacute" => 'Ý',
        "THORN" => 'Þ',
        "szlig" => 'ß',
        "agrave" => 'à',
        "aacute" => 'á',
        "acirc" => 'â',
        "atilde" => 'ã',
        "auml" => 'ä',
        "aring" => 'å',
        "aelig" => 'æ',
        "ccedil" => 'ç',
        "egrave" => 'è',
        "eacute" => 'é',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "igrave" => 'ì',
        "iacute" => 'í',
        "icirc" => 'î',
        "iuml" => 'ï',
        "eth" => 'ð',
        "ntilde" => 'ñ',
        "ograve" => 'ò',
        "oacute" => 'ó',
        "ocirc" => 'ô',
        "otilde" => 'õ',
        "ouml" => 'ö',
        "divide" => '÷',
        "oslash" => 'ø',
        "ugrave" => 'ù',
        "uacute" => 'ú',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "yacute" => 'ý',
        "thorn" => 'þ',
        "yuml" => 'ÿ',
        _ => {
            let character = match name {
                "apos" => '\'',
                "euro" => '€',
                "trade" | "TRADE" => '™',
                "ndash" => '–',
                "mdash" => '—',
                "lsquo" | "OpenCurlyQuote" => '‘',
                "rsquo" | "CloseCurlyQuote" => '’',
                "sbquo" => '‚',
                "ldquo" | "OpenCurlyDoubleQuote" => '“',
                "rdquo" | "CloseCurlyDoubleQuote" => '”',
                "bdquo" => '„',
                "hellip" => '…',
                "bull" | "bullet" => '•',
                "dagger" => '†',
                "Dagger" => '‡',
                "permil" => '‰',
                "lsaquo" => '‹',
                "rsaquo" => '›',
                "OElig" => 'Œ',
                "oelig" => 'œ',
                "Scaron" => 'Š',
                "scaron" => 'š',
                "Zcaron" => 'Ž',
                "zcaron" => 'ž',
                "Yuml" => 'Ÿ',
                "circ" => 'ˆ',
                "tilde" => '˜',
                "fnof" => 'ƒ',
                "ensp" => '\u{2002}',
                "emsp" => '\u{2003}',
                "thinsp" => '\u{2009}',
                "NonBreakingSpace" => '\u{00a0}',
                "NewLine" => '\n',
                "Tab" => '\t',
                "minus" => '−',
                "ne" => '≠',
                "le" => '≤',
                "ge" => '≥',
                "notin" | "notinva" => '∉',
                "larr" => '←',
                "rarr" => '→',
                "uarr" => '↑',
                "darr" => '↓',
                "harr" => '↔',
                _ => return None,
            };
            return Some((character, false));
        }
    };
    Some((legacy, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_namespace_parser_provenance_is_conservative_across_foreign_recovery() {
        for markup in [
            "<html><svg><g id=x></html><p id=after>unknown</p>",
            "<p><svg><g id=x><div id=after>implied escape</div>",
        ] {
            let doc = parse(markup);
            for id in ["x", "after"] {
                let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
                assert_eq!(doc.nodes[node].namespace, Namespace::UnknownForeign, "{markup}");
            }
        }
        let doc = parse("<div id=html xmlns='urn:other'></div><math><mi id=m>x</mi></math><p id=after>known</p>");
        for (id, namespace) in [("html", Namespace::Html), ("m", Namespace::UnknownForeign), ("after", Namespace::Html)] {
            let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
            assert_eq!(doc.nodes[node].namespace, namespace);
        }
        assert!(doc.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Text(_))).all(|n| n.namespace == Namespace::None));
    }

    fn text(document: &Document) -> String {
        document
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                NodeKind::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn duplicate_attributes_keep_script_and_stylesheet_nonce_evidence() {
        let doc = parse("<link id=l rel=stylesheet nonce=first NONCE=second href=x.css><style id=s nonce=first nonce=second>p{color:red}</style><script id=j nonce=first nonce=second></script><link id=good rel=stylesheet nonce=only href=y.css>");
        for id in ["l", "s", "j", "good"] {
            let node = doc
                .nodes
                .iter()
                .enumerate()
                .find_map(|(node, _)| (doc.attr(node, "id") == Some(id)).then_some(node))
                .unwrap();
            assert_eq!(
                doc.attr(node, "nonce"),
                Some(if id == "good" { "only" } else { "first" })
            );
            assert_eq!(doc.nonce_unsafe_nodes.contains(&node), id != "good");
        }
    }

    #[test]
    fn omitted_table_cell_and_row_end_tags_keep_sibling_cells() {
        let doc = parse("<table><tr><td>A<th>B<tr><td>C<td><table><tr><td>N<td>Z</table><tbody><tr><td>D<td>E</table>");
        let rows: Vec<_> = doc
            .nodes
            .iter()
            .filter(|node| matches!(&node.kind, NodeKind::Element {tag,..} if tag == "tr"))
            .collect();
        assert_eq!(rows.len(), 4);
        for row in rows {
            let cells = row.children.iter().filter(|&&child| matches!(&doc.nodes[child].kind, NodeKind::Element {tag,..} if tag == "td" || tag == "th")).count();
            assert_eq!(cells, 2);
        }
        assert_eq!(text(&doc), "ABCNZDE");
    }

    #[test]
    fn keeps_structure_attributes_title_and_unicode() {
        let document = parse(
            "<html><head><title>  Fish &amp; Chips\n</title></head><body>\
             <H1 ID='hello' id='ignored'>Hello</H1>\
             <p><a HREF='next.html?a=1&amp;b=2'>café &#x1F642;</a><br>tail</p>\
             </body></html>",
        );
        assert_eq!(document.title, "Fish & Chips");
        let link = document
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    &node.kind, NodeKind::Element { tag, .. } if tag == "a"
                )
            })
            .unwrap();
        assert!(
            matches!(&link.kind, NodeKind::Element { href: Some(href), .. }
            if href == "next.html?a=1&b=2")
        );
        assert!(matches!(&document.nodes[link.children[0]].kind,
            NodeKind::Text(value) if value == "café 🙂"));
        assert!(document.nodes.iter().any(|node| matches!(
            &node.kind, NodeKind::Element { tag, id: Some(id), .. }
                if tag == "h1" && id == "hello"
        )));
    }

    #[test]
    fn retains_raw_css_and_scripts_without_parsing_markup_or_entities_inside_them() {
        let document = parse(
            "before<!-- hidden --><script>hidden </scripted><b>also hidden</b>\
             </ScRiPt><style>p::before { content: '&amp;'; }</style>after\
             <script>unterminated hidden",
        );
        assert_eq!(
            text(&document),
            "beforehidden </scripted><b>also hidden</b>p::before { content: '&amp;'; }afterunterminated hidden"
        );
    }

    #[test]
    fn retains_attributes_and_closes_paragraphs_and_list_items() {
        let document =
            parse("<p CLASS='one&#32;two' class='ignored' data-X='ok'>a<p>b<ul><li>c<li>d</ul>");
        let first = document.nodes[0].children[0];
        assert_eq!(document.attr(first, "class"), Some("one two"));
        assert_eq!(document.attr(first, "DATA-X"), Some("ok"));
        assert_eq!(document.attr(usize::MAX, "class"), None);
        assert_eq!(document.nodes[0].children.len(), 3);
        let list = document.nodes[0].children[2];
        assert_eq!(document.nodes[list].children.len(), 2);
    }

    #[test]
    fn omitted_option_and_group_end_tags_keep_select_choices_as_siblings() {
        let doc = parse("<select><option value=a>Alpha<option value=b>Beta<optgroup label=First><option>One<option>Two<optgroup label=Second disabled><option>Three</select><p>After</p>");
        let select = doc.nodes[0].children[0];
        let children = &doc.nodes[select].children;
        assert_eq!(children.len(), 4);
        assert_eq!(doc.attr(children[0], "value"), Some("a"));
        assert_eq!(doc.attr(children[1], "value"), Some("b"));
        assert_eq!(doc.attr(children[2], "label"), Some("First"));
        assert_eq!(doc.attr(children[3], "label"), Some("Second"));
        assert_eq!(doc.nodes[children[2]].children.len(), 2);
        assert_eq!(doc.nodes[children[3]].children.len(), 1);
        assert_eq!(doc.nodes[0].children.len(), 2);
        assert_eq!(text(&doc), "AlphaBetaOneTwoThreeAfter");

        let doc = parse("<select id=outer><option>Outer<select id=inner><option>Inner<option>Next</select></option></select>");
        let outer = doc.nodes[0].children[0];
        let option = doc.nodes[outer].children[0];
        let inner = doc.nodes[option].children[1];
        assert_eq!(doc.attr(inner, "id"), Some("inner"));
        assert_eq!(doc.nodes[inner].children.len(), 2);
    }

    #[test]
    fn recovers_predictably_and_decodes_only_supported_entities() {
        let document =
            parse("<p>2 < 3 &bogus; &#65; &#0; &#xD800; &lt;/p&gt;</wrong></p><div id='");
        assert_eq!(text(&document), "2 < 3 &bogus; A � � </p>");
        assert!(document.nodes.iter().all(|node| !matches!(
            &node.kind, NodeKind::Element { tag, .. } if tag == "div"
        )));
    }

    #[test]
    fn common_named_entities_keep_accents_punctuation_and_nonbreaking_spaces() {
        assert_eq!(
            decode_entities(
                "Im&aacute;genes &bull; B&uacute;squeda &copy; 2026 &reg; &trade; &euro;"
            ),
            "Imágenes • Búsqueda © 2026 ® ™ €"
        );
        assert_eq!(
            decode_entities(
                "&Aacute;&aacute; &Ccedil;&ccedil; &Ntilde;&ntilde; &OElig;&oelig; &Yuml;&yuml;"
            ),
            "Áá Çç Ññ Œœ Ÿÿ"
        );
        assert_eq!(
            decode_entities(
                "&ldquo;Caf&eacute;&rdquo;&nbsp;&ndash;&nbsp;&lsquo;ni&ntilde;o&rsquo;&hellip;"
            ),
            "“Café”\u{00a0}–\u{00a0}‘niño’…"
        );
        assert_eq!(
            decode_entities("&amp;lt; &unknown; &eAcute;"),
            "&lt; &unknown; &eAcute;"
        );
    }

    #[test]
    fn legacy_names_use_longest_matching_and_attribute_ambiguity_rules() {
        assert_eq!(
            decode_entities("&copycat &notit; &notin; &amp=1 &apos &trade"),
            "©cat ¬it; ∉ &=1 &apos &trade"
        );
        let document =
            parse("<a href='?a=1&copy=2&reg3&copy;=4&copy!' title='&AMP &quot;'>Link</a>");
        let link = document.nodes[0].children[0];
        assert_eq!(document.attr(link, "href"), Some("?a=1&copy=2&reg3©=4©!"));
        assert_eq!(document.attr(link, "title"), Some("& \""));
    }

    #[test]
    fn numeric_references_replace_invalid_scalars_and_apply_html_c1_corrections() {
        assert_eq!(
            decode_entities("&#0; &#xD800; &#x110000; &#128; &#x91; &#x92; &#x9f;"),
            "� � � € ‘ ’ Ÿ"
        );
        assert_eq!(
            decode_entities("&#x81; &#xFFFF; &#x1F642;"),
            "\u{81} \u{ffff} 🙂"
        );
        assert_eq!(
            decode_entities("&#65tail &#x41; &#x; &#; &#-1;"),
            "Atail A &#x; &#; &#-1;"
        );
        assert_eq!(
            decode_entities(&format!("&#{};end", "9".repeat(8_000))),
            "�end"
        );
        assert_eq!(decode_entities("&#13;&#10;"), "\r\n");
    }

    #[test]
    fn textarea_is_rcdata_and_ignores_only_its_first_line_feed() {
        let document = parse("<title> T &copy;\r\n one </title><textarea name='q'>\r\nIm&aacute;genes\r\n<b>literal</b> &lt;/textarea&gt; </textareaX>\0</TEXTAREA><p>after</p>");
        assert_eq!(document.title, "T © one");
        let textarea = document
            .nodes
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "textarea"))
            .unwrap();
        assert_eq!(textarea.children.len(), 1);
        assert!(
            matches!(&document.nodes[textarea.children[0]].kind, NodeKind::Text(value) if value == "Imágenes\n<b>literal</b> </textarea> </textareaX>�")
        );
        assert!(!document
            .nodes
            .iter()
            .any(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "b")));
        assert!(text(&document).ends_with("after"));
        let leading_reference = parse("<textarea>&#10;\nsecond line</textarea>");
        assert_eq!(text(&leading_reference), "\nsecond line");
    }

    #[test]
    fn limits_tree_size_and_nesting_without_recursion() {
        let deep = parse(&"<div>".repeat(MAX_DEPTH * 2));
        assert_eq!(deep.nodes.len(), MAX_DEPTH);
        let wide = parse(&"<br>".repeat(MAX_NODES * 2));
        assert_eq!(wide.nodes.len(), MAX_NODES);
        assert_eq!(wide.nodes[0].children.len(), MAX_NODES - 1);
    }
}
