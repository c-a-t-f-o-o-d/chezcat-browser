//! Separate HTML node trees with no Window, renderer, or resource loader.
//! https://dom.spec.whatwg.org/#dom-domimplementation-createhtmldocument
use super::*;

pub(super) const MAIN_IMPLEMENTATION: u32 = 0x8600_0000;
const FIRST_ENTRY: u32 = MAIN_IMPLEMENTATION + 1;
const MAX_ENTRIES: usize = 65_536;
const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

pub(super) const GET: &str = "\0Document.get.implementation";
pub(super) const CREATE: &str = "\0DOMImplementation.createHTMLDocument";
pub(super) const HAS_FEATURE: &str = "\0DOMImplementation.hasFeature";

#[derive(Default)]
pub(super) struct Arena {
    entries: Vec<Entry>,
}

struct DocumentNodes {
    implementation: u32,
    doctype: u32,
    html: u32,
    head: u32,
    body: u32,
    title: String,
}

enum Kind {
    Document(DocumentNodes),
    Doctype,
    Element(String),
    Text(String),
    ProcessingInstruction { target: String, data: String },
}

struct InertNode {
    kind: Kind,
    owner: u32,
    parent: Option<u32>,
    children: Vec<u32>,
}

enum Entry {
    Implementation,
    Node(InertNode),
}

impl Arena {
    fn entry(&self, handle: u32) -> Option<&Entry> {
        let index = handle.checked_sub(FIRST_ENTRY)? as usize;
        self.entries.get(index)
    }

    fn node(&self, handle: u32) -> Option<&InertNode> {
        match self.entry(handle)? {
            Entry::Node(node) => Some(node),
            Entry::Implementation => None,
        }
    }
}

impl Bridge<'_> {
    pub(super) fn inert_document(&self, object: u32) -> bool {
        self.state.inert_documents.node(object)
            .is_some_and(|node| matches!(node.kind, Kind::Document(_)))
    }

    pub(super) fn inert_implementation(&self, object: u32) -> bool {
        object == MAIN_IMPLEMENTATION
            || matches!(self.state.inert_documents.entry(object), Some(Entry::Implementation))
    }

    pub(super) fn inert_node_type(&self, object: u32) -> Option<u32> {
        Some(match self.state.inert_documents.node(object)?.kind {
            Kind::Document(_) => 9,
            Kind::Doctype => 10,
            Kind::Element(_) => 1,
            Kind::Text(_) => 3,
            Kind::ProcessingInstruction { .. } => 7,
        })
    }

    pub(super) fn inert_interface(&self, object: u32) -> Option<&'static str> {
        if self.inert_implementation(object) {
            return Some("DOMImplementation");
        }
        Some(match &self.state.inert_documents.node(object)?.kind {
            Kind::Document(_) => "Document",
            Kind::Doctype => "DocumentType",
            Kind::Element(tag) => crate::javascript::ordinary_html_interface(tag).unwrap_or("HTMLElement"),
            Kind::Text(_) => "Text",
            Kind::ProcessingInstruction { .. } => "ProcessingInstruction",
        })
    }

    pub(super) fn inert_factory(&self, object: u32, name: &str) -> Option<&'static str> {
        if !self.inert_document(object) { return None; }
        match name {
            "createElement" => Some("createElement"),
            "createElementNS" => Some("createElementNS"),
            "createTextNode" => Some("createTextNode"),
            _ => None,
        }
    }

    pub(super) fn inert_owns_property(&self, object: u32, name: &str) -> bool {
        if self.inert_factory(object, name).is_some() { return true; }
        let Some(node) = self.state.inert_documents.node(object) else { return false; };
        if matches!(name, "ownerDocument" | "parentNode" | "parentElement" | "firstChild" | "lastChild"
            | "previousSibling" | "nextSibling" | "isConnected" | "nodeValue" | "baseURI")
        {
            return true;
        }
        match node.kind {
            Kind::Document(_) => matches!(name, "URL" | "documentURI" | "defaultView" | "contentType"
                | "compatMode" | "characterSet" | "namespaceURI" | "doctype" | "body" | "head" | "title"),
            Kind::Doctype => matches!(name, "name" | "publicId" | "systemId"),
            Kind::Element(_) => matches!(name, "localName" | "tagName" | "namespaceURI" | "prefix"),
            Kind::Text(_) | Kind::ProcessingInstruction { .. } => matches!(name, "data" | "length"),
        }
    }

    fn reserve_inert_entries(&mut self, count: usize, payload: usize) -> Result<u32, String> {
        let current = self.state.inert_documents.entries.len();
        let limit = self.state.limits.host.extra_nodes.min(MAX_ENTRIES);
        if current.saturating_add(count) > limit {
            return Err(format!("Inert DOM identity limit reached ({limit} entries)."));
        }
        self.state.work(count)?;
        self.state.inert_documents.entries.try_reserve(count)
            .map_err(|_| "Cannot reserve inert DOM identities.")?;
        self.state.replace_bytes(0, count.saturating_mul(std::mem::size_of::<Entry>())
            .saturating_add(payload))?;
        Ok(FIRST_ENTRY + current as u32)
    }

    pub(super) fn create_inert_document(&mut self, args: &[Value]) -> Result<Value, String> {
        let title = match args.first() {
            None | Some(Value::Undefined) => None,
            Some(value) => Some(self.text(value)?),
        };
        let title_capacity = title.as_ref().map_or(0, String::capacity);
        self.state.work(title_capacity)?;
        self.state.copies(title_capacity)?;
        let mut normalized = String::new();
        for word in title.as_deref().unwrap_or("").split_ascii_whitespace() {
            normalized.try_reserve(word.len() + usize::from(!normalized.is_empty()))
                .map_err(|_| "Cannot reserve the inert document title.")?;
            if !normalized.is_empty() { normalized.push(' '); }
            normalized.push_str(word);
        }
        let count = if title.is_some() { 8 } else { 6 };
        // The fixed element names and child links share the document budget.
        let payload = title_capacity.saturating_add(normalized.capacity()).saturating_add(64);
        let document = self.reserve_inert_entries(count, payload)?;
        let implementation = document + 1;
        let doctype = document + 2;
        let html = document + 3;
        let head = document + 4;
        let body = document + 5;
        let title_element = document + 6;
        let text = document + 7;
        let has_title = title.is_some();
        let node = |kind, parent, children| Entry::Node(InertNode {
            kind, parent, children, owner: document,
        });
        self.state.inert_documents.entries.extend([
            node(Kind::Document(DocumentNodes {
                implementation, doctype, html, head, body, title: normalized,
            }), None, vec![doctype, html]),
            Entry::Implementation,
            node(Kind::Doctype, Some(document), Vec::new()),
            node(Kind::Element("html".into()), Some(document), vec![head, body]),
            node(Kind::Element("head".into()), Some(html), if has_title { vec![title_element] } else { Vec::new() }),
            node(Kind::Element("body".into()), Some(html), Vec::new()),
        ]);
        if let Some(title) = title {
            self.state.inert_documents.entries.extend([
                node(Kind::Element("title".into()), Some(head), vec![text]),
                node(Kind::Text(title), Some(title_element), Vec::new()),
            ]);
        }
        Ok(Value::Host(document))
    }

    pub(super) fn inert_create_node(&mut self, owner: u32, method: &str, args: &[Value]) -> Result<Value, String> {
        let kind = if method == "createTextNode" {
            Kind::Text(self.text(args.first().ok_or("createTextNode requires a value.")?)?)
        } else {
            let namespace_aware = method == "createElementNS";
            let name = self.text(args.get(usize::from(namespace_aware)).ok_or("Element creation requires a name.")?)?;
            if namespace_aware {
                let namespace = match args.first() {
                    Some(Value::String(value)) if !value.is_empty() => Some(value.as_str()),
                    _ => None,
                };
                if namespace != Some(HTML_NAMESPACE) || name.contains(':') {
                    return Err("Inert createElementNS requires the HTML namespace and a null prefix.".into());
                }
            }
            if args.get(1 + usize::from(namespace_aware)).is_some_and(|v| !matches!(v, Value::Null | Value::Undefined)) {
                return Err("Inert element creation options are unavailable.".into());
            }
            if namespace_aware && !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')) {
                return Err("Inert element name is outside the supported spelling subset.".into());
            }
            Kind::Element(if namespace_aware { name } else { name.to_ascii_lowercase() })
        };
        self.append_inert_leaf(owner, kind)
    }

    pub(super) fn create_inert_processing_instruction(&mut self, owner: u32, target: String, data: String) -> Result<Value, String> {
        self.append_inert_leaf(owner, Kind::ProcessingInstruction { target, data })
    }

    fn append_inert_leaf(&mut self, owner: u32, kind: Kind) -> Result<Value, String> {
        let bytes = match &kind {
            Kind::Element(name) | Kind::Text(name) => name.capacity(),
            Kind::ProcessingInstruction { target, data } => target.capacity().saturating_add(data.capacity()),
            _ => 0,
        };
        let handle = self.reserve_inert_entries(1, bytes)?;
        self.state.inert_documents.entries.push(Entry::Node(InertNode {
            kind, owner, parent: None, children: Vec::new(),
        }));
        Ok(Value::Host(handle))
    }

    fn inert_text_content(&mut self, object: u32) -> Result<Value, String> {
        let mut pending = vec![object];
        let mut text = String::new();
        while let Some(id) = pending.pop() {
            self.state.work(1)?;
            let node = self.state.inert_documents.node(id).ok_or("Unknown inert Node.")?;
            if let Kind::Text(value) = &node.kind {
                let bytes = value.len();
                self.state.copies(bytes)?;
                let Kind::Text(value) = &self.state.inert_documents.node(id).unwrap().kind else { unreachable!() };
                text.push_str(value);
            } else {
                pending.extend(node.children.iter().rev().copied());
            }
        }
        Ok(Value::String(text))
    }

    pub(super) fn inert_get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        self.state.work(1)?;
        if self.inert_implementation(object) { return Ok(Value::Undefined); }
        let node = self.state.inert_documents.node(object).ok_or("Unknown inert Node.")?;
        if let Kind::ProcessingInstruction { target, data } = &node.kind {
            match property {
                "target" | "nodeName" | "data" | "nodeValue" | "textContent" => {
                    let read_target = matches!(property, "target" | "nodeName");
                    self.state.copies(if read_target { target.len() } else { data.len() })?;
                    let Kind::ProcessingInstruction { target, data } =
                        &self.state.inert_documents.node(object).unwrap().kind else { unreachable!() };
                    return Ok(Value::String(if read_target { target.clone() } else { data.clone() }));
                }
                "length" => {
                    self.state.work(data.len())?;
                    let Kind::ProcessingInstruction { data, .. } =
                        &self.state.inert_documents.node(object).unwrap().kind else { unreachable!() };
                    return Ok(Value::Number(data.encode_utf16().count() as f64));
                }
                _ => {}
            }
        }
        let host = |id: Option<u32>| id.map_or(Value::Null, Value::Host);
        let string = |value: &str| Value::String(value.into());
        let document = match &node.kind { Kind::Document(nodes) => Some(nodes), _ => None };
        Ok(match property {
            "nodeType" => Value::Number(f64::from(self.inert_node_type(object).unwrap())),
            "ownerDocument" => host((node.owner != object).then_some(node.owner)),
            "parentNode" => host(node.parent),
            "parentElement" => host(node.parent.filter(|&id| self.inert_node_type(id) == Some(1))),
            "firstChild" => host(node.children.first().copied()),
            "lastChild" => host(node.children.last().copied()),
            "previousSibling" | "nextSibling" => {
                let parent = node.parent.and_then(|id| self.state.inert_documents.node(id));
                host(parent.and_then(|parent| {
                    let at = parent.children.iter().position(|&id| id == object)?;
                    let sibling = if property == "nextSibling" { Some(at + 1) } else { at.checked_sub(1) }?;
                    parent.children.get(sibling).copied()
                }))
            }
            "baseURI" => string("about:blank"),
            "isConnected" => {
                let mut id = object;
                while let Some(parent) = self.state.inert_documents.node(id).and_then(|node| node.parent) { id = parent; }
                Value::Bool(id == node.owner)
            }
            "nodeName" => match &node.kind {
                Kind::Document(_) => string("#document"),
                Kind::Doctype => string("html"),
                Kind::Element(name) => Value::String(name.to_ascii_uppercase()),
                Kind::Text(_) => string("#text"),
                Kind::ProcessingInstruction { .. } => unreachable!("processing instruction name was handled above"),
            },
            "textContent" if matches!(node.kind, Kind::Element(_) | Kind::Text(_)) => return self.inert_text_content(object),
            "data" | "nodeValue" if matches!(node.kind, Kind::Text(_)) => return self.inert_text_content(object),
            "textContent" | "nodeValue" => Value::Null,
            "length" if matches!(node.kind, Kind::Text(_)) => {
                let Kind::Text(value) = &node.kind else { unreachable!() };
                let bytes = value.len();
                self.state.work(bytes)?;
                let Kind::Text(value) = &self.state.inert_documents.node(object).unwrap().kind else { unreachable!() };
                Value::Number(value.encode_utf16().count() as f64)
            }
            "documentElement" => host(document.map(|doc| doc.html)),
            "head" => host(document.map(|doc| doc.head)),
            "body" => host(document.map(|doc| doc.body)),
            "doctype" => host(document.map(|doc| doc.doctype)),
            "implementation" => host(document.map(|doc| doc.implementation)),
            "title" if document.is_some() => {
                self.state.copies(document.unwrap().title.len())?;
                let Kind::Document(document) = &self.state.inert_documents.node(object).unwrap().kind else { unreachable!() };
                string(&document.title)
            }
            "URL" | "documentURI" if document.is_some() => string("about:blank"),
            "contentType" if document.is_some() => string("text/html"),
            "compatMode" if document.is_some() => string("CSS1Compat"),
            "characterSet" if document.is_some() => string("UTF-8"),
            "defaultView" if document.is_some() => Value::Null,
            "name" if matches!(node.kind, Kind::Doctype) => string("html"),
            "publicId" | "systemId" if matches!(node.kind, Kind::Doctype) => string(""),
            "localName" if matches!(node.kind, Kind::Element(_)) => {
                let Kind::Element(name) = &node.kind else { unreachable!() };
                string(name)
            }
            "tagName" if matches!(node.kind, Kind::Element(_)) => {
                let Kind::Element(name) = &node.kind else { unreachable!() };
                Value::String(name.to_ascii_uppercase())
            }
            "namespaceURI" if matches!(node.kind, Kind::Element(_)) => string(HTML_NAMESPACE),
            "namespaceURI" | "prefix" => Value::Null,
            _ => Value::Undefined,
        })
    }
}
