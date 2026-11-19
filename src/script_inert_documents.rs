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
    template_documents: HashMap<u32, u32>,
    template_contents: HashMap<u32, u32>,
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
    TemplateDocument { implementation: u32 },
    Fragment,
    Parsed { source: usize, copy_template_contents: bool },
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
            .is_some_and(|node| matches!(node.kind, Kind::Document(_) | Kind::TemplateDocument { .. }))
    }

    pub(super) fn inert_implementation(&self, object: u32) -> bool {
        object == MAIN_IMPLEMENTATION
            || matches!(self.state.inert_documents.entry(object), Some(Entry::Implementation))
    }

    pub(super) fn inert_node_type(&self, object: u32) -> Option<u32> {
        Some(match self.state.inert_documents.node(object)?.kind {
            Kind::Document(_) | Kind::TemplateDocument { .. } => 9,
            Kind::Fragment => 11,
            Kind::Parsed { source, .. } => match self.doc.nodes.get(source)?.kind {
                NodeKind::Element { .. } => 1,
                NodeKind::Text(_) => 3,
                NodeKind::Comment(_) => 8,
                NodeKind::ProcessingInstruction { .. } => 7,
                NodeKind::DocumentFragment => 11,
            },
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
            Kind::Document(_) | Kind::TemplateDocument { .. } => "Document",
            Kind::Fragment => "DocumentFragment",
            Kind::Parsed { source, .. } => {
                let node = self.doc.nodes.get(*source)?;
                match &node.kind {
                    NodeKind::Element { tag, .. } => match node.namespace {
                        crate::html::Namespace::Html => crate::javascript::ordinary_html_interface(tag).unwrap_or("HTMLElement"),
                        crate::html::Namespace::Svg => "SVGElement",
                        _ => "Element",
                    },
                    NodeKind::Text(_) => "Text",
                    NodeKind::Comment(_) => "Comment",
                    NodeKind::ProcessingInstruction { .. } => "ProcessingInstruction",
                    NodeKind::DocumentFragment => "DocumentFragment",
                }
            }
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
            | "previousSibling" | "nextSibling" | "isConnected" | "nodeValue" | "baseURI" | "childNodes")
        {
            return true;
        }
        match node.kind {
            Kind::Document(_) | Kind::TemplateDocument { .. } => matches!(name, "URL" | "documentURI" | "defaultView" | "contentType"
                | "compatMode" | "characterSet" | "namespaceURI" | "doctype" | "body" | "head" | "title"),
            Kind::Doctype => matches!(name, "name" | "publicId" | "systemId"),
            Kind::Element(_) => matches!(name, "localName" | "tagName" | "namespaceURI" | "prefix"),
            Kind::Fragment => false,
            Kind::Parsed { source, .. } => match self.doc.nodes[source].kind {
                NodeKind::Element { .. } => matches!(name, "id" | "localName" | "tagName" | "namespaceURI" | "prefix"),
                NodeKind::Text(_) | NodeKind::Comment(_) => matches!(name, "data" | "length"),
                NodeKind::ProcessingInstruction { .. } => matches!(name, "data" | "length"),
                NodeKind::DocumentFragment => false,
            },
            Kind::Text(_) | Kind::ProcessingInstruction { .. } => matches!(name, "data" | "length"),
        }
    }

    pub(super) fn inert_children(&self, object: u32) -> Option<&[u32]> {
        self.state.inert_documents.node(object).map(|node| node.children.as_slice())
    }

    fn parsed_template(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|entry|
            entry.namespace == crate::html::Namespace::Html
                && matches!(&entry.kind, NodeKind::Element { tag, .. } if tag == "template"))
    }

    fn template_source(&self, object: u32) -> Option<(u32, Option<usize>)> {
        if let Some(index) = node_index(object) {
            return (self.dom_accessible(index) && self.parsed_template(index))
                .then_some((DOCUMENT, Some(index)));
        }
        let node = self.state.inert_documents.node(object)?;
        match &node.kind {
            Kind::Element(tag) if tag == "template" => Some((node.owner, None)),
            Kind::Parsed { source, copy_template_contents } if self.parsed_template(*source) => {
                Some((node.owner, copy_template_contents.then_some(*source)))
            }
            _ => None,
        }
    }

    pub(super) fn template_receiver(&self, object: u32) -> bool {
        self.template_source(object).is_some()
    }

    pub(super) fn template_content(&mut self, object: u32) -> Result<Value, String> {
        let (document, source) = self.template_source(object)
            .ok_or("Template content requires a genuine HTMLTemplateElement.")?;
        if let Some(&handle) = self.state.inert_documents.template_contents.get(&object) {
            self.state.work(1)?;
            return Ok(Value::Host(handle));
        }
        let arena = &self.state.inert_documents;
        let existing_owner = if matches!(arena.node(document).map(|node| &node.kind), Some(Kind::TemplateDocument { .. })) {
            Some(document)
        } else {
            arena.template_documents.get(&document).copied()
        };
        let limit = self.state.limits.host.extra_nodes.min(MAX_ENTRIES).saturating_sub(arena.entries.len());
        let first = FIRST_ENTRY + arena.entries.len() as u32;
        let owner_overhead = if existing_owner.is_some() { 0 } else { 2 };
        let owner = existing_owner.unwrap_or(first);
        let fragment = first + owner_overhead as u32;
        if limit <= owner_overhead { return Err("Inert DOM identity limit reached during template content read.".into()); }

        // Source nodes stay in the immutable parsed input. The exposed
        // fragment gets new inert handles and never returns active-tree IDs.
        let mut pending = vec![(source, None)];
        let mut staged = Vec::new();
        let mut payload = 2 * std::mem::size_of::<(u32, u32)>();
        if existing_owner.is_none() {
            staged.push(Entry::Node(InertNode {
                kind: Kind::TemplateDocument { implementation: first + 1 },
                owner, parent: None, children: Vec::new(),
            }));
            staged.push(Entry::Implementation);
        }
        let mut index = 0;
        while let Some(&(source, parent)) = pending.get(index) {
            let handle = fragment + index as u32;
            let children = source.filter(|&child| index == 0 || !self.parsed_template(child))
                .map_or(&[][..], |child| self.doc.nodes[child].children.as_slice());
            self.state.work(1 + children.len())?;
            if children.len() > limit - owner_overhead - pending.len() {
                return Err("Inert DOM identity limit reached during template content read.".into());
            }
            pending.try_reserve(children.len()).map_err(|_| "Cannot reserve template traversal.")?;
            let child_start = fragment + pending.len() as u32;
            pending.extend(children.iter().map(|&child| (Some(child), Some(handle))));
            let mut child_handles = Vec::new();
            child_handles.try_reserve_exact(children.len()).map_err(|_| "Cannot reserve template children.")?;
            child_handles.extend(child_start..child_start + children.len() as u32);
            payload = payload.saturating_add(child_handles.capacity().saturating_mul(std::mem::size_of::<u32>()));
            staged.try_reserve(1).map_err(|_| "Cannot reserve parsed template nodes.")?;
            staged.push(Entry::Node(InertNode {
                kind: if index == 0 { Kind::Fragment } else {
                    Kind::Parsed { source: source.unwrap(), copy_template_contents: true }
                },
                owner, parent, children: child_handles,
            }));
            index += 1;
        }
        self.state.inert_documents.template_contents.try_reserve(1).map_err(|_| "Cannot reserve template content identity.")?;
        if existing_owner.is_none() {
            self.state.inert_documents.template_documents.try_reserve(1).map_err(|_| "Cannot reserve template document identity.")?;
        }
        let committed = self.reserve_inert_entries(staged.len(), payload)?;
        debug_assert_eq!(committed, first);
        let arena = &mut self.state.inert_documents;
        arena.entries.extend(staged);
        if existing_owner.is_none() { arena.template_documents.insert(document, owner); }
        arena.template_contents.insert(object, fragment);
        Ok(Value::Host(fragment))
    }

    pub(super) fn inert_clone_receiver(&self, object: u32) -> bool {
        self.state.inert_documents.node(object).is_some_and(|node| matches!(node.kind,
            Kind::Fragment | Kind::Parsed { .. } | Kind::Element(_) | Kind::Text(_)
                | Kind::ProcessingInstruction { .. }))
    }

    pub(super) fn clone_inert_tree(&mut self, object: u32, deep: bool) -> Result<Value, String> {
        let arena = &self.state.inert_documents;
        let owner = arena.node(object).ok_or("Inert clone source is unavailable.")?.owner;
        let remaining = self.state.limits.host.extra_nodes.min(MAX_ENTRIES).saturating_sub(arena.entries.len());
        if remaining == 0 { return Err("Inert DOM identity limit reached during cloning.".into()); }
        let first = FIRST_ENTRY + arena.entries.len() as u32;
        let mut pending = vec![(object, None)];
        let mut staged = Vec::new();
        let mut payload = 0usize;
        while let Some(&(source, parent)) = pending.get(staged.len()) {
            let node = self.state.inert_documents.node(source)
                .filter(|node| node.owner == owner).ok_or("Inert clone source ownership changed.")?;
            let child_count = if deep { node.children.len() } else { 0 };
            if child_count > remaining - pending.len() {
                return Err("Inert DOM identity limit reached during cloning.".into());
            }
            let string_bytes = match &node.kind {
                Kind::Element(name) | Kind::Text(name) => name.len(),
                Kind::ProcessingInstruction { target, data } => target.len().saturating_add(data.len()),
                _ => 0,
            };
            self.state.work(child_count.saturating_add(1))?;
            self.state.copies(string_bytes)?;
            pending.try_reserve(child_count).map_err(|_| "Cannot reserve inert clone traversal.")?;
            staged.try_reserve(1).map_err(|_| "Cannot reserve inert clone nodes.")?;
            let node = self.state.inert_documents.node(source).ok_or("Inert clone source is unavailable.")?;
            let handle = first + staged.len() as u32;
            let child_start = first + pending.len() as u32;
            pending.extend(node.children.iter().take(child_count).map(|&child| (child, Some(handle))));
            let mut children = Vec::new();
            children.try_reserve_exact(child_count).map_err(|_| "Cannot reserve inert clone children.")?;
            children.extend(child_start..child_start + child_count as u32);
            let kind = match &node.kind {
                Kind::Fragment => Kind::Fragment,
                Kind::Parsed { source, copy_template_contents } =>
                    Kind::Parsed { source: *source, copy_template_contents: deep && *copy_template_contents },
                Kind::Element(name) => Kind::Element(name.clone()),
                Kind::Text(data) => Kind::Text(data.clone()),
                Kind::ProcessingInstruction { target, data } =>
                    Kind::ProcessingInstruction { target: target.clone(), data: data.clone() },
                Kind::Document(_) | Kind::TemplateDocument { .. } | Kind::Doctype =>
                    return Err("Cloning this inert document node is unavailable.".into()),
            };
            payload = payload.saturating_add(string_bytes)
                .saturating_add(children.capacity().saturating_mul(std::mem::size_of::<u32>()));
            staged.push(Entry::Node(InertNode { kind, owner, parent, children }));
        }
        let committed = self.reserve_inert_entries(staged.len(), payload)?;
        debug_assert_eq!(committed, first);
        self.state.inert_documents.entries.extend(staged);
        Ok(Value::Host(first))
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

    fn inert_text_value(&self, object: u32) -> Option<&str> {
        match &self.state.inert_documents.node(object)?.kind {
            Kind::Text(value) => Some(value),
            Kind::Parsed { source, .. } => match &self.doc.nodes.get(*source)?.kind {
                NodeKind::Text(value) => Some(value),
                _ => None,
            },
            _ => None,
        }
    }

    fn inert_text_content(&mut self, object: u32) -> Result<Value, String> {
        let mut pending = vec![object];
        let mut text = String::new();
        while let Some(id) = pending.pop() {
            self.state.work(1)?;
            let node = self.state.inert_documents.node(id).ok_or("Unknown inert Node.")?;
            if let Some(value) = self.inert_text_value(id) {
                let bytes = value.len();
                if text.len().saturating_add(bytes) > self.state.limits.host.string_bytes {
                    return Err("Inert DOM textContent exceeds the host string limit.".into());
                }
                self.state.copies(bytes)?;
                text.push_str(self.inert_text_value(id).unwrap());
            } else {
                pending.try_reserve(node.children.len()).map_err(|_| "Cannot reserve inert text traversal.")?;
                pending.extend(node.children.iter().rev().copied());
            }
        }
        Ok(Value::String(text))
    }

    pub(super) fn inert_get(&mut self, object: u32, property: &str) -> Result<Value, String> {
        self.state.work(1)?;
        if self.inert_implementation(object) { return Ok(Value::Undefined); }
        if property == "childNodes" { return self.create_inert_child_list(object); }
        let node = self.state.inert_documents.node(object).ok_or("Unknown inert Node.")?;
        if let Kind::Parsed { source, .. } = node.kind {
            let parsed = &self.doc.nodes[source];
            if let Some(data) = parsed.kind.character_data() {
                match property {
                    "data" | "nodeValue" | "textContent" => {
                        self.state.copies(data.len())?;
                        return Ok(Value::String(data.into()));
                    }
                    "length" => {
                        self.state.work(data.len())?;
                        return Ok(Value::Number(data.encode_utf16().count() as f64));
                    }
                    _ => {}
                }
            }
            if let NodeKind::Element { tag, id, .. } = &parsed.kind {
                match property {
                    "localName" | "nodeName" | "tagName" => {
                        self.state.copies(tag.len())?;
                        return Ok(Value::String(if property != "localName" && parsed.namespace == crate::html::Namespace::Html {
                            tag.to_ascii_uppercase()
                        } else { tag.clone() }));
                    }
                    "namespaceURI" => return Ok(parsed.namespace.element_uri()
                        .map_or(Value::Null, |ns| Value::String(ns.into()))),
                    "id" => {
                        let id = id.as_deref().unwrap_or_default();
                        self.state.copies(id.len())?;
                        return Ok(Value::String(id.into()));
                    }
                    _ => {}
                }
            }
        }
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
        let template_implementation = match node.kind {
            Kind::TemplateDocument { implementation } => Some(implementation),
            _ => None,
        };
        let html_document = document.is_some() || template_implementation.is_some();
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
                Kind::Document(_) | Kind::TemplateDocument { .. } => string("#document"),
                Kind::Fragment => string("#document-fragment"),
                Kind::Parsed { source, .. } => match &self.doc.nodes[*source].kind {
                    NodeKind::Element { tag, .. } => string(tag),
                    NodeKind::Text(_) => string("#text"),
                    NodeKind::Comment(_) => string("#comment"),
                    NodeKind::ProcessingInstruction { target, .. } => string(target),
                    NodeKind::DocumentFragment => string("#document-fragment"),
                },
                Kind::Doctype => string("html"),
                Kind::Element(name) => Value::String(name.to_ascii_uppercase()),
                Kind::Text(_) => string("#text"),
                Kind::ProcessingInstruction { .. } => unreachable!("processing instruction name was handled above"),
            },
            "textContent" if matches!(node.kind, Kind::Fragment | Kind::Parsed { .. } | Kind::Element(_) | Kind::Text(_)) => return self.inert_text_content(object),
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
            "implementation" => host(document.map(|doc| doc.implementation).or(template_implementation)),
            "title" if template_implementation.is_some() => string(""),
            "title" if document.is_some() => {
                self.state.copies(document.unwrap().title.len())?;
                let Kind::Document(document) = &self.state.inert_documents.node(object).unwrap().kind else { unreachable!() };
                string(&document.title)
            }
            "URL" | "documentURI" if html_document => string("about:blank"),
            "contentType" if html_document => string("text/html"),
            "compatMode" if html_document => string("CSS1Compat"),
            "characterSet" if html_document => string("UTF-8"),
            "defaultView" if html_document => Value::Null,
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
