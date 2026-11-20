//! Separate HTML node trees with no Window, renderer, or resource loader.
//! https://dom.spec.whatwg.org/#dom-domimplementation-createhtmldocument
use super::*;
use std::rc::Rc;

#[path = "script_dom_parser.rs"]
pub(super) mod parser;

pub(super) const MAIN_IMPLEMENTATION: u32 = 0x8600_0000;
const FIRST_ENTRY: u32 = MAIN_IMPLEMENTATION + 1;
const ENTRY_END: u32 = 0x8700_0000; // AbortController owns the next typed domain.
const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

#[cfg(test)]
#[path = "script_inert_storage_tests.rs"]
mod storage_tests;
#[cfg(test)]
#[path = "script_template_identity_storage_tests.rs"]
mod template_identity_storage_tests;
#[cfg(test)]
#[path = "script_adopted_storage_tests.rs"]
mod adopted_storage_tests;
#[cfg(test)]
#[path = "script_promoted_child_list_tests.rs"]
mod promoted_child_list_tests;

pub(super) const GET: &str = "\0Document.get.implementation";
pub(super) const CREATE: &str = "\0DOMImplementation.createHTMLDocument";
pub(super) const HAS_FEATURE: &str = "\0DOMImplementation.hasFeature";

#[derive(Default)]
pub(super) struct Arena {
    entries: Vec<Entry>,
    adopted: identity_map::IdentityMap<u32, InertNode>,
    generated: Vec<GeneratedNode>,
    template_documents: identity_map::IdentityMap<u32>,
    template_contents: identity_map::IdentityMap<u32>,
}

struct TemplateIndexGrowth {
    documents: Option<Vec<(u32, u32)>>,
    contents: Option<Vec<(u32, u32)>>,
}

struct DocumentNodes {
    implementation: u32,
    doctype: Option<u32>,
    url: String,
    html: u32,
    head: u32,
    body: u32,
    title: String,
}

#[derive(Clone, Copy)]
enum ParsedSource {
    Loaded(usize),
    Generated(usize),
}

struct GeneratedNode {
    node: Rc<Node>,
    unsafe_nonce: bool,
}

enum Kind {
    Document(DocumentNodes),
    TemplateDocument { implementation: u32 },
    Fragment { template: Option<u32> },
    Parsed { source: ParsedSource, initial_template: Option<usize> },
    Doctype,
    Element(String),
    Text(String),
    ProcessingInstruction { target: String, data: String },
}

pub(super) enum ImportSource<'a> {
    Parsed { node: &'a Node, unsafe_nonce: bool },
    Element(&'a str),
    Text(&'a str),
    Fragment,
    ProcessingInstruction { target: &'a str, data: &'a str },
}

struct InertNode {
    kind: Kind,
    owner: u32,
    parent: Option<u32>,
    children: Vec<u32>,
}

struct Promotion {
    index: usize,
    parent: usize,
    node: Node,
    unsafe_nonce: bool,
    child_list: Option<u32>,
}

enum Entry {
    Implementation,
    Parser,
    Node(InertNode),
    ParkedNode,
}

impl Arena {
    pub(super) fn backing_bytes(&self) -> usize {
        self.entries.capacity() * size_of::<Entry>()
            + self.generated.capacity() * size_of::<GeneratedNode>()
            + self.template_documents.storage_bytes() + self.template_contents.storage_bytes()
            + self.adopted.storage_bytes()
    }

    fn entry(&self, handle: u32) -> Option<&Entry> {
        let index = handle.checked_sub(FIRST_ENTRY)? as usize;
        self.entries.get(index)
    }

    fn node(&self, handle: u32) -> Option<&InertNode> {
        if handle < FIRST_ENTRY { return self.adopted.get(&handle); }
        match self.entry(handle)? {
            Entry::Node(node) => Some(node),
            Entry::Implementation | Entry::Parser | Entry::ParkedNode => None,
        }
    }

    fn node_mut(&mut self, handle: u32) -> Option<&mut InertNode> {
        if handle < FIRST_ENTRY { return self.adopted.get_mut(&handle); }
        match self.entries.get_mut((handle - FIRST_ENTRY) as usize)? {
            Entry::Node(node) => Some(node),
            Entry::Implementation | Entry::Parser | Entry::ParkedNode => None,
        }
    }
}

impl Bridge<'_> {
    pub(super) fn inert_document(&self, object: u32) -> bool {
        self.state.inert_documents.node(object)
            .is_some_and(|node| matches!(node.kind, Kind::Document(_) | Kind::TemplateDocument { .. }))
    }

    pub(super) fn inert_parser(&self, object: u32) -> bool {
        matches!(self.state.inert_documents.entry(object), Some(Entry::Parser))
    }

    pub(super) fn inert_implementation(&self, object: u32) -> bool {
        object == MAIN_IMPLEMENTATION
            || matches!(self.state.inert_documents.entry(object), Some(Entry::Implementation))
    }

    pub(super) fn inert_node_type(&self, object: u32) -> Option<u32> {
        Some(match self.state.inert_documents.node(object)?.kind {
            Kind::Document(_) | Kind::TemplateDocument { .. } => 9,
            Kind::Fragment { .. } => 11,
            Kind::Parsed { source, .. } => match self.parsed_source(source)?.0.kind {
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
        if self.inert_parser(object) { return Some("DOMParser"); }
        Some(match &self.state.inert_documents.node(object)?.kind {
            Kind::Document(_) | Kind::TemplateDocument { .. } => "Document",
            Kind::Fragment { .. } => "DocumentFragment",
            Kind::Parsed { source, .. } => {
                let node = self.parsed_source(*source)?.0;
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
        if matches!(name, "ownerDocument" | "isConnected" | "nodeValue" | "baseURI") {
            return true;
        }
        match node.kind {
            Kind::Document(_) | Kind::TemplateDocument { .. } => matches!(name, "URL" | "documentURI" | "defaultView" | "contentType"
                | "compatMode" | "characterSet" | "namespaceURI" | "doctype" | "body" | "head" | "title"),
            Kind::Doctype => matches!(name, "name" | "publicId" | "systemId"),
            Kind::Element(_) => matches!(name, "localName" | "tagName" | "namespaceURI" | "prefix"),
            Kind::Fragment { .. } => false,
            Kind::Parsed { source, .. } => match self.parsed_source(source).map(|(node, _)| &node.kind) {
                Some(NodeKind::Element { .. }) => matches!(name, "id" | "localName" | "tagName" | "namespaceURI" | "prefix"),
                Some(NodeKind::Text(_) | NodeKind::Comment(_) | NodeKind::ProcessingInstruction { .. }) =>
                    matches!(name, "data" | "length"),
                _ => false,
            },
            Kind::Text(_) | Kind::ProcessingInstruction { .. } => matches!(name, "data" | "length"),
        }
    }

    pub(super) fn inert_children(&self, object: u32) -> Option<&[u32]> {
        self.state.inert_documents.node(object).map(|node| node.children.as_slice())
    }

    pub(super) fn inert_element_attribute(&mut self, object: u32, argument: &Value) -> Result<Value, String> {
        let name = self.text(argument)?;
        let node = self.state.inert_documents.node(object).ok_or("Inert Element is unavailable.")?;
        let source = match node.kind {
            Kind::Element(_) => return Ok(Value::Null),
            Kind::Parsed { source, .. } => source,
            _ => return Err("getAttribute requires an Element.".into()),
        };
        let (node, _) = self.parsed_source(source).ok_or("Inert Element source is unavailable.")?;
        let NodeKind::Element { attrs, .. } = &node.kind else {
            return Err("getAttribute requires an Element.".into());
        };
        let html = node.namespace == crate::html::Namespace::Html;
        let count = attrs.len();
        let work = attrs.iter().map(|attr| attr.name.qualified.len()).sum::<usize>();
        self.state.work(count)?;
        self.state.copies(work)?;
        let name = if html {
            self.state.copies(name.len())?;
            name.to_ascii_lowercase()
        } else {
            name
        };
        let (node, _) = self.parsed_source(source).ok_or("Inert Element source is unavailable.")?;
        let NodeKind::Element { attrs, .. } = &node.kind else { unreachable!() };
        let Some(index) = attrs.iter().position(|attr| attr.name.qualified == name) else {
            return Ok(Value::Null);
        };
        let size = attrs[index].value.len();
        if size > self.state.limits.host.string_bytes {
            return Err("Attribute value exceeds the host string bound.".into());
        }
        self.state.copies(size)?;
        let (node, _) = self.parsed_source(source).ok_or("Inert Element source is unavailable.")?;
        let NodeKind::Element { attrs, .. } = &node.kind else { unreachable!() };
        Ok(Value::String(attrs[index].value.clone()))
    }

    pub(super) fn checked_inert_fragment(&self, object: u32) -> Option<Option<u32>> {
        let node = self.state.inert_documents.node(object)?;
        match node.kind {
            Kind::Fragment { template } if node.parent.is_none()
                && node.children.iter().all(|&child| self.parked_template_source(child).is_some()) => Some(template),
            _ => None,
        }
    }

    fn parked_template_source(&self, handle: u32) -> Option<(&InertNode, &GeneratedNode, usize)> {
        let index = node_index(handle)?;
        let arena = &self.state.inert_documents;
        let node = arena.adopted.get(&handle)?;
        let Kind::Parsed { source: ParsedSource::Generated(source), .. } = node.kind else { return None; };
        Some((node, arena.generated.get(source)?, index))
    }

    /// Promote the measured safe HTML subset from template markup. The
    /// destination tree only sees the existing identities after all checks.
    pub(super) fn splice_checked_inert_fragment(
        &mut self, parent: usize, fragment: u32, reference: Option<usize>,
    ) -> Result<Value, String> {
        let children = self.inert_children(fragment).ok_or("Inert fragment is unavailable.")?;
        let count = children.len();
        if count == 0 { return Ok(Value::Host(fragment)); }
        self.state.work(count)?;
        self.mutation_ancestors(parent)?;
        if self.doc.nodes[parent].namespace != crate::html::Namespace::Html
            && !self.fragment_receiver(node_handle(parent))
            || matches!(tag(self.doc, parent), "script" | "link")
        {
            return Err("Inert fragment insertion requires an ordinary HTML or fragment destination.".into());
        }
        let mut depth = 0;
        let mut ancestor = Some(parent);
        while let Some(node) = ancestor {
            self.state.work(1)?;
            depth += 1;
            ancestor = self.state.tree.parents[node];
        }
        let children = self.inert_children(fragment).ok_or("Inert fragment is unavailable.")?;
        let mut roots = Vec::new();
        let mut pending = Vec::new();
        roots.try_reserve_exact(count).map_err(|_| "Cannot reserve promoted fragment roots.")?;
        pending.try_reserve(count).map_err(|_| "Cannot reserve promoted fragment traversal.")?;
        for &handle in children {
            roots.push(node_index(handle).ok_or("Inert fragment root has no reserved document slot.")?);
            pending.push((handle, parent, depth + 1));
        }
        let mut plan = Vec::new();
        let mut indices = Vec::new();
        let mut stylesheet = None;
        let mut retained = 0usize;
        let mut released = 0usize;
        while let Some((handle, parent, depth)) = pending.pop() {
            if depth > dom_tree::MAX_DOM_DEPTH { return Err("Inert fragment insertion exceeds DOM depth.".into()); }
            self.state.work(1)?;
            let (inert, source, index) = self.parked_template_source(handle)
                .ok_or("Inert fragment contains unsupported active markup.")?;
            released = inert.children.capacity().checked_mul(size_of::<u32>())
                .and_then(|bytes| released.checked_add(bytes)).ok_or("Promoted inert child-link refund overflow.")?;
            let raw = Rc::clone(&source.node);
            let unsafe_nonce = source.unsafe_nonce;
            let mut bytes = copied_node_bytes(&raw);
            if let NodeKind::Element { tag, href, attrs, id } = &raw.kind {
                if let Some(id) = id.as_deref().filter(|id| !id.is_empty()) {
                    self.state.work(self.forms.controls.len())?;
                    if self.forms.controls.keys().any(|&control| self.doc.attr(control, "form") == Some(id)) {
                        return Err("Inert fragment cannot change explicit native form associations.".into());
                    }
                }
                self.state.work(attrs.len())?;
                if let Some(attr) = attrs.iter().find(|attr| attr.name.html_is("style")) {
                    validate_style(&attr.value, self.state.limits.host.string_bytes)?;
                }
                if tag == "style" && stylesheet.replace(index).is_some() {
                    return Err("Inert fragment insertion admits one inline stylesheet per splice.".into());
                }
                bytes = bytes.saturating_add(tag.len()).saturating_add(href.as_ref().map_or(0, String::len));
            }
            let (inert, _, _) = self.parked_template_source(handle).unwrap();
            let children = inert.children.len();
            bytes = bytes.saturating_add(children.saturating_mul(size_of::<usize>()));
            self.state.work(children)?;
            self.state.copies(bytes)?;
            retained = retained.saturating_add(bytes);
            let (inert, _, _) = self.parked_template_source(handle).unwrap();
            let mut node_children = Vec::new();
            node_children.try_reserve_exact(children).map_err(|_| "Cannot reserve promoted subtree children.")?;
            pending.try_reserve(children).map_err(|_| "Cannot reserve promoted subtree traversal.")?;
            for &child in &inert.children {
                let slot = node_index(child).ok_or("Inert fragment contains unsupported active markup.")?;
                node_children.push(slot);
                pending.push((child, index, depth + 1));
            }
            let node = Node {
                namespace: raw.namespace,
                kind: script_clone_node::copy_node_kind(&raw.kind)?,
                children: node_children,
            };
            let extra=copied_payload_extra(&node.kind);self.state.copies(extra)?;retained=retained.saturating_add(extra);
            plan.try_reserve(1).map_err(|_| "Cannot reserve inert promotion plan.")?;
            indices.try_reserve(1).map_err(|_| "Cannot reserve inert promotion identities.")?;
            plan.push(Promotion { index, parent, node, unsafe_nonce, child_list: None });
            indices.push(index);
        }
        self.state.work(self.doc.nodes[parent].children.len())?;
        let position = reference.and_then(|reference| self.doc.nodes[parent].children.iter().position(|&node| node == reference))
            .unwrap_or(self.doc.nodes[parent].children.len());
        self.doc.nodes[parent].children.try_reserve(count).map_err(|_| "Cannot reserve fragment destination children.")?;
        self.doc.nonce_unsafe_nodes.try_reserve(plan.len()).map_err(|_| "Cannot reserve fragment nonce provenance.")?;
        self.state.dom.created.try_reserve(plan.len()).map_err(|_| "Cannot reserve promoted node identities.")?;
        self.state.detached_nodes.try_reserve(plan.len()).map_err(|_| "Cannot reserve detached promotion identities.")?;
        self.state.work(plan.len().saturating_mul(self.state.dom.inert_child_lists.lookup_work()))?;
        let mut child_lists = 0usize;
        for item in &mut plan {
            item.child_list = self.state.dom.inert_child_lists.get(&node_handle(item.index)).copied();
            if let Some(list) = item.child_list {
                self.state.work(self.state.dom.child_lists.lookup_work())?;
                if self.state.dom.child_lists.contains_key(&item.index) {
                    return Err("Promoted child-list destination identity already exists.".into());
                }
                if !queries::node_list_index(list).and_then(|index| self.state.node_lists.get(index))
                    .is_some_and(|list| matches!(list, queries::NodeList::InertChildren(handle) if *handle==node_handle(item.index)))
                {
                    return Err("Promoted child-list source identity is inconsistent.".into());
                }
                child_lists += 1;
            }
        }
        let available=self.state.retained.checked_sub(released)
            .and_then(|bytes|self.state.retained_limit.checked_sub(bytes))
            .and_then(|bytes|bytes.checked_sub(retained)).ok_or("DOM retained promotion allowance exhausted.")?;
        let (child_map,added)=self.state.dom.child_lists.prepare(child_lists,available)?;
        // Append then sort the destination once. The source index compacts in
        // one membership scan after all failure points; its backing stays paid.
        let destination_length = self.state.dom.child_lists.len().checked_add(child_lists)
            .ok_or("Promoted child-list destination size overflow.")?;
        let destination_depth = (usize::BITS - destination_length.leading_zeros()) as usize + 1;
        let destination_sort = if child_lists==0 { 0 } else {
            destination_length.checked_mul(destination_depth).ok_or("Promoted child-list sort work overflow.")?
        };
        self.state.work(destination_sort)?;
        let grow_copy = if child_map.is_some(){self.state.dom.child_lists.len()}else{0};
        let copied = destination_sort.checked_mul(3).and_then(|work|work.checked_add(grow_copy))
            .and_then(|work|work.checked_add(child_lists)).ok_or("Promoted child-list copy work overflow.")?;
        self.state.copies(self.state.dom.child_lists.movement_bytes(copied))?;
        retained=retained.checked_add(added).ok_or("Promoted child-list storage overflow.")?;
        // One sorted membership set permits one compacting removal scan. It
        // avoids repeatedly shifting the adopted store for each promoted node.
        let search = (usize::BITS - indices.len().leading_zeros()) as usize + 1;
        let sort_work = indices.len().checked_mul(search).ok_or("Promotion index sort work overflow.")?;
        let scan_work = self.state.inert_documents.adopted.len().checked_add(
                if child_lists==0 {0}else{self.state.dom.inert_child_lists.len()})
            .and_then(|length|length.checked_mul(search + 3))
            .and_then(|work| work.checked_mul(2)).and_then(|work| work.checked_add(sort_work))
            .ok_or("Promotion index scan work overflow.")?;
        self.state.work(scan_work)?;
        self.state.copies(sort_work.checked_mul(3 * size_of::<usize>()).ok_or("Promotion index sort copy work overflow.")?)?;
        let connected = self.state.dom.connected[parent];
        // Populate checked slots for native CSS and caption preflight while the
        // inert routing still owns them. No renderer or script observes these
        // slots until the destination's children are committed.
        for item in &mut plan {
            std::mem::swap(&mut self.doc.nodes[item.index], &mut item.node);
            if item.unsafe_nonce { self.doc.nonce_unsafe_nodes.insert(item.index); }
        }
        let checked = (|| {
            self.validate_button_children(parent, &indices)?;
            let inline = stylesheet.map(|node| self.prepare_inline_sheet(node, connected)).transpose()?;
            indices.sort_unstable();
            let shifted = self.state.inert_documents.adopted.retention_movement(|handle|
                !node_index(*handle).is_some_and(|node| indices.binary_search(&node).is_ok()));
            self.state.work(shifted)?;
            self.state.copies(self.state.inert_documents.adopted.movement_bytes(shifted))?;
            if child_lists!=0 {
                let shifted = self.state.dom.inert_child_lists.retention_movement(|handle|
                    !node_index(*handle).is_some_and(|node| indices.binary_search(&node).is_ok()));
                self.state.work(shifted)?;
                self.state.copies(self.state.dom.inert_child_lists.movement_bytes(shifted))?;
            }
            self.state.replace_storage(released, retained, retained)?;
            Ok::<_, String>(inline)
        })();
        let inline = match checked {
            Ok(inline) => inline,
            Err(error) => {
                for item in &mut plan {
                    std::mem::swap(&mut self.doc.nodes[item.index], &mut item.node);
                    if item.unsafe_nonce { self.doc.nonce_unsafe_nodes.remove(&item.index); }
                }
                return Err(error);
            }
        };
        self.state.dom.child_lists.commit(child_map);
        self.state.dom.child_lists.append_and_sort(child_lists,
            plan.iter().filter_map(|item|item.child_list.map(|list|(item.index,list))));
        if child_lists!=0 {
            self.state.dom.inert_child_lists.retain(|handle|
                !node_index(*handle).is_some_and(|node| indices.binary_search(&node).is_ok()));
        }
        self.state.inert_documents.node_mut(fragment).unwrap().children.clear();
        self.doc.nodes[parent].children.splice(position..position, roots);
        self.state.inert_documents.adopted.retain(|handle|
            !node_index(*handle).is_some_and(|node| indices.binary_search(&node).is_ok()));
        for item in plan {
            if let Some(list) = item.child_list {
                self.state.node_lists[queries::node_list_index(list).unwrap()] = queries::NodeList::Children(item.index);
            }
            self.state.dom.created.insert(item.index);
            self.state.tree.parents[item.index] = Some(item.parent);
            self.state.dom.connected[item.index] = connected;
            if !connected { self.state.detached_nodes.insert(item.index); }
        }
        if let (Some(node), Some(inline)) = (stylesheet, inline) { self.commit_inline_sheet(node, inline); }
        self.state.dom_committed();
        self.state.dirty = true;
        if connected { self.state.layout_changed(); }
        Ok(Value::Host(fragment))
    }

    fn parsed_source(&self, source: ParsedSource) -> Option<(&Node, bool)> {
        match source {
            ParsedSource::Loaded(index) => Some((self.doc.nodes.get(index)?, self.doc.nonce_unsafe_nodes.contains(&index))),
            ParsedSource::Generated(index) => {
                let source = self.state.inert_documents.generated.get(index)?;
                Some((source.node.as_ref(), source.unsafe_nonce))
            }
        }
    }

    fn parsed_template(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|entry|
            entry.namespace == crate::html::Namespace::Html
                && matches!(&entry.kind, NodeKind::Element { tag, .. } if tag == "template"))
    }

    fn template_source(&self, object: u32) -> Option<(u32, Option<usize>)> {
        if let Some(index) = node_index(object).filter(|_| !self.inert_adopted_node(object)) {
            return (self.dom_accessible(index) && self.parsed_template(index)).then_some((DOCUMENT, Some(index)));
        }
        let node = self.state.inert_documents.node(object)?;
        match &node.kind {
            Kind::Element(tag) if tag == "template" => Some((node.owner, None)),
            Kind::Parsed { source, initial_template } if self.parsed_source(*source).is_some_and(|(source, _)| {
                source.namespace == crate::html::Namespace::Html
                    && matches!(&source.kind, NodeKind::Element { tag, .. } if tag == "template")
            }) => Some((node.owner, *initial_template)),
            _ => None,
        }
    }

    pub(super) fn template_receiver(&self, object: u32) -> bool {
        self.template_source(object).is_some()
    }

    pub(super) fn template_fragment_receiver(&self, object: u32) -> bool {
        matches!(self.state.inert_documents.node(object).map(|node| &node.kind),
            Some(Kind::Fragment { template: Some(_) }))
    }

    pub(super) fn inert_adopted_node(&self, object: u32) -> bool {
        self.state.inert_documents.adopted.contains_key(&object)
    }

    pub(super) fn validate_template_append(&self, fragment: u32, args: &[Value]) -> Result<(), (&'static str, String)> {
        let Some(Value::Host(child)) = args.first() else {
            return Err(("TypeError", "Template appendChild requires a genuine Node.".into()));
        };
        let Some(index) = node_index(*child) else {
            return Err(("NotSupportedError", "Template append supports detached main-document elements.".into()));
        };
        if !self.dom_accessible(index) || !self.state.dom.created.contains(&index)
            || self.state.dom.connected.get(index) == Some(&true)
            || self.state.tree.parents.get(index).copied().flatten().is_some()
        {
            return Err(("NotSupportedError", "Template append requires a detached author-created node.".into()));
        }
        let node = &self.doc.nodes[index];
        let NodeKind::Element { tag, .. } = &node.kind else {
            return Err(("NotSupportedError", "Template append supports ordinary HTML elements.".into()));
        };
        if node.namespace != crate::html::Namespace::Html || !node.children.is_empty()
            || !(tag == "template" || script_clone_node::ordinary_clone_element(tag))
        {
            return Err(("NotSupportedError", "Template append supports ordinary HTML leaves and inert nested templates.".into()));
        }
        if tag == "template" {
            let arena = &self.state.inert_documents;
            let owner = arena.node(fragment).map(|node| node.owner);
            if let Some(content) = arena.template_contents.get(child).and_then(|&handle| arena.node(handle)) {
                if Some(content.owner) != owner {
                    return Err(("NotSupportedError", "Template append cannot transfer materialized contents to another inert owner.".into()));
                }
            }
        }
        Ok(())
    }

    pub(super) fn append_template_child(&mut self, fragment: u32, args: &[Value]) -> Result<Value, String> {
        self.validate_template_append(fragment, args).map_err(|(_, message)| message)?;
        let Value::Host(child) = args[0] else { unreachable!() };
        let index = node_index(child).unwrap();
        let owner = self.state.inert_documents.node(fragment).ok_or("Missing template fragment.")?.owner;
        let mut ancestor = Some(fragment);
        while let Some(id) = ancestor {
            self.state.work(1)?;
            if id == child { return Err("Template append would create an ancestor cycle.".into()); }
            ancestor = self.state.inert_documents.node(id).and_then(|node| {
                node.parent.or_else(|| match node.kind {
                    Kind::Fragment { template } => template,
                    _ => None,
                })
            });
        }
        let available = self.state.retained_limit.saturating_sub(self.state.retained);
        let minimum = self.state.inert_documents.adopted.minimum_growth(1)?;
        let children = &self.state.inert_documents.node(fragment).unwrap().children;
        let (replacement, added) = retained_storage::grow(children, 1,
            available.checked_sub(minimum).ok_or("Template adoption index allowance exhausted.")?)?;
        let moved = if replacement.is_some() { children.len() } else { 0 };
        self.state.work(moved.saturating_add(1))?;
        let copies = (moved + 1).checked_mul(size_of::<u32>()).ok_or("Template child-link copy work overflow.")?;
        let (index_growth, index_bytes) = self.prepare_adopted_index(std::iter::once(child), available - added)?;
        self.state.replace_storage(0, added + index_bytes, copies)?;
        let arena = &mut self.state.inert_documents;
        arena.adopted.commit(index_growth);
        if let Some(mut next) = replacement {
            let children = &mut arena.node_mut(fragment).unwrap().children;
            next.append(children);
            *children = next;
        }
        arena.adopted.insert(child, InertNode {
            kind: Kind::Parsed { source: ParsedSource::Loaded(index), initial_template: None },
            owner, parent: Some(fragment), children: Vec::new(),
        });
        arena.node_mut(fragment).unwrap().children.push(child);
        Ok(Value::Host(child))
    }

    pub(super) fn inert_root(&mut self, object: u32) -> Result<Value, String> {
        let mut current = object;
        loop {
            self.state.work(1)?;
            let node = self.state.inert_documents.node(current).ok_or("Inert root is unavailable.")?;
            let Some(parent) = node.parent else { return Ok(Value::Host(current)); };
            current = parent;
        }
    }

    pub(super) fn template_content(&mut self, object: u32) -> Result<Value, String> {
        let (document, source) = self.template_source(object)
            .ok_or("Template content requires a genuine HTMLTemplateElement.")?;
        self.state.work(self.state.inert_documents.template_contents.lookup_work()
            + self.state.inert_documents.template_documents.lookup_work())?;
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
        let limit = self.inert_remaining_entries();
        let first = FIRST_ENTRY + arena.entries.len() as u32;
        let owner_overhead = if existing_owner.is_some() { 0 } else { 2 };
        let owner = existing_owner.unwrap_or(first);
        let fragment = first + owner_overhead as u32;
        if limit <= owner_overhead { return Err("Inert DOM identity limit reached during template content read.".into()); }

        // Source nodes stay in the immutable parsed input. The exposed
        // fragment gets new inert handles and never returns active-tree IDs.
        let mut pending = vec![(source, None)];
        let mut staged = Vec::new();
        let mut payload = 0usize;
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
                kind: if index == 0 { Kind::Fragment { template: Some(object) } } else {
                    let source = source.unwrap();
                    Kind::Parsed {
                        source: ParsedSource::Loaded(source),
                        initial_template: self.parsed_template(source).then_some(source),
                    }
                },
                owner, parent, children: child_handles,
            }));
            index += 1;
        }
        let document_link = existing_owner.is_none().then_some((document, owner));
        let content_link = [(object, fragment)];
        let (committed, indexes) = self.prepare_template_indexes(staged.len(), payload, 0, document_link, &content_link)?;
        debug_assert_eq!(committed, first);
        let arena = &mut self.state.inert_documents;
        arena.entries.extend(staged);
        arena.template_documents.commit(indexes.documents);
        arena.template_contents.commit(indexes.contents);
        if existing_owner.is_none() { arena.template_documents.insert(document, owner); }
        arena.template_contents.insert(object, fragment);
        Ok(Value::Host(fragment))
    }

    pub(super) fn template_html(&mut self, object: u32) -> Result<Value, String> {
        use script_inner_html::{append, escaped, raw_text, write_open_tag};

        let Value::Host(root) = self.template_content(object)? else { unreachable!() };
        let mut output = String::new();
        let mut pending = Vec::new();
        let children = self.inert_children(root).ok_or("Template fragment is unavailable.")?;
        pending.try_reserve(children.len()).map_err(|_| "Cannot reserve template serialization.")?;
        pending.extend(children.iter().rev().map(|&child| (child, false, false)));
        while let Some((handle, end, raw)) = pending.pop() {
            self.state.work(1)?;
            let Kind::Parsed { source, .. } = self.state.inert_documents.node(handle)
                .ok_or("Template serialization node is unavailable.")?.kind else {
                return Err("Unsupported inert template serialization node.".into());
            };
            let generated = match source {
                ParsedSource::Generated(index) => Some(Rc::clone(&self.state.inert_documents.generated[index].node)),
                _ => None,
            };
            let node = match source {
                ParsedSource::Loaded(index) => &self.doc.nodes[index],
                ParsedSource::Generated(_) => generated.as_deref().unwrap(),
            };
            match &node.kind {
                NodeKind::Text(text) => escaped(self.state, &mut output, text, false, raw)?,
                NodeKind::Comment(data) => {
                    for piece in ["<!--", data, "-->"] { append(self.state, &mut output, piece)?; }
                }
                NodeKind::ProcessingInstruction { target, data } => {
                    for piece in ["<?", target, " ", data, "?>"] { append(self.state, &mut output, piece)?; }
                }
                NodeKind::DocumentFragment => return Err("Nested fragment serialization is unavailable.".into()),
                NodeKind::Element { tag, attrs, .. } => {
                    if !matches!(node.namespace, crate::html::Namespace::Html | crate::html::Namespace::Svg) {
                        return Err("Template serialization of foreign elements is unavailable.".into());
                    }
                    if end {
                        for piece in ["</", tag, ">"] { append(self.state, &mut output, piece)?; }
                        continue;
                    }
                    write_open_tag(self.state, &mut output, tag, attrs)?;
                    if crate::html::is_void(tag) { continue; }
                    let raw = raw_text(tag, self.doc.scripting_enabled);
                    let children_owner = if tag == "template" {
                        let Value::Host(fragment) = self.template_content(handle)? else { unreachable!() };
                        fragment
                    } else { handle };
                    let children = self.inert_children(children_owner).ok_or("Template serialization children are unavailable.")?;
                    pending.try_reserve(children.len().saturating_add(1)).map_err(|_| "Cannot reserve template serialization.")?;
                    pending.push((handle, true, false));
                    pending.extend(children.iter().rev().map(|&child| (child, false, raw)));
                }
            }
        }
        Ok(Value::String(output))
    }

    pub(super) fn set_template_html(&mut self, object: u32, markup: &str) -> Result<(), String> {
        let (document, _) = self.template_source(object).ok_or("Invalid HTML template receiver.")?;
        if markup.len() > self.state.limits.host.string_bytes {
            return Err("HTML fragment source exceeds the host string limit.".into());
        }
        self.state.work(markup.len())?;
        self.state.copies(markup.len().saturating_mul(2))?;
        let parsed = crate::html::parse_fragment(markup, "template", self.doc.scripting_enabled)?;

        self.state.work(self.state.inert_documents.template_contents.lookup_work()
            + self.state.inert_documents.template_documents.lookup_work())?;
        let had_fragment = self.state.inert_documents.template_contents.contains_key(&object);
        let had_template_owner = self.state.inert_documents.template_documents.contains_key(&document);
        let creates_owner_index = !had_template_owner && !matches!(
            self.state.inert_documents.node(document).map(|node| &node.kind),
            Some(Kind::TemplateDocument { .. }));
        // Cleanup cannot fail once the unobservable content cache is staged.
        // Prepay the exact old-entry shifts and future lookup depth now.
        if !had_fragment {
            let arena = &self.state.inert_documents;
            let content_work = arena.template_contents.insert_work(&object);
            let document_work = if creates_owner_index { arena.template_documents.insert_work(&document) } else { 0 };
            let shifted = content_work - arena.template_contents.lookup_work()
                + if creates_owner_index { document_work - arena.template_documents.lookup_work() } else { 0 };
            self.state.work(content_work.saturating_add(document_work).saturating_add(2))?;
            self.state.copies(shifted.saturating_mul(size_of::<(u32, u32)>()))?;
        }
        let starting_entries = self.state.inert_documents.entries.len();
        let starting_backing = self.state.inert_documents.backing_bytes();
        let starting_bytes = self.state.retained;
        let Value::Host(fragment) = self.template_content(object)? else { unreachable!() };
        let result = self.replace_template_children(fragment, parsed);
        if result.is_err() && !had_fragment {
            let arena = &mut self.state.inert_documents;
            arena.entries.truncate(starting_entries);
            arena.template_contents.remove(&object);
            if creates_owner_index { arena.template_documents.remove(&document); }
            // Rolled-back payload is gone; reusable backing remains allocated.
            let backing = arena.backing_bytes();
            self.state.retained = starting_bytes + (backing - starting_backing);
        }
        result
    }

    fn replace_template_children(&mut self, fragment: u32, mut parsed: Document) -> Result<(), String> {
        let arena = &self.state.inert_documents;
        let owner = arena.node(fragment).ok_or("Template fragment is unavailable.")?.owner;
        let fragment_index = (fragment - FIRST_ENTRY) as usize;
        let root = arena.node(fragment).ok_or("Template fragment is unavailable.")?;
        let released = root.children.capacity().checked_mul(size_of::<u32>())
            .ok_or("Template child-link storage overflow.")?;
        let detach_work = root.children.len();
        let count = parsed.nodes.len().checked_sub(1).ok_or("Missing checked fragment root.")?;
        if count > self.inert_remaining_entries_with_release(released) {
            return Err("Inert DOM identity limit reached during template replacement.".into());
        }
        self.state.work(detach_work)?;
        let arena = &self.state.inert_documents;
        let first = FIRST_ENTRY + arena.entries.len() as u32;
        let generated_base = arena.generated.len();
        // Leave immutable raw sources in the inert arena. A separate reserved
        // document slot keeps the Node identity stable across owner changes.
        let base = self.doc.nodes.len();
        let can_park = count <= self.state.node_limit
            .saturating_sub(base.saturating_add(self.state.blank_frame_nodes))
            .min(((STYLE_BASE - NODE_BASE) as usize).saturating_sub(base));
        let mut handles = Vec::new();
        handles.try_reserve_exact(count + 1).map_err(|_| "Cannot reserve template node links.")?;
        handles.push(fragment);
        let mut parked = 0;
        for (offset, node) in parsed.nodes.iter().skip(1).enumerate() {
            let handle = if can_park && template_node_can_park(node) {
                let handle = node_handle(base + parked);
                parked += 1;
                handle
            } else {
                first + offset as u32
            };
            handles.push(handle);
        }
        let mut entries = Vec::new();
        let mut generated = Vec::new();
        let mut parked_nodes = Vec::new();
        entries.try_reserve_exact(count).map_err(|_| "Cannot reserve template replacement identities.")?;
        generated.try_reserve_exact(count).map_err(|_| "Cannot reserve parsed template replacement nodes.")?;
        parked_nodes.try_reserve_exact(parked).map_err(|_| "Cannot reserve parked template identities.")?;
        let mut parents = Vec::new();
        parents.try_reserve_exact(parsed.nodes.len()).map_err(|_| "Cannot reserve template parent links.")?;
        parents.resize(parsed.nodes.len(), None);
        let mut payload = count.saturating_mul(std::mem::size_of::<Node>() + 2 * size_of::<usize>());
        let children_for = |items: &[usize]| -> Result<Vec<u32>, String> {
            let mut children = Vec::new();
            children.try_reserve_exact(items.len()).map_err(|_| "Cannot reserve template child links.")?;
            children.extend(items.iter().map(|&child| handles[child]));
            Ok(children)
        };
        for (local, node) in parsed.nodes.iter().enumerate() {
            self.state.work(node.children.len().saturating_add(1))?;
            let parent = Some(handles[local]);
            for &child in &node.children { parents[child] = parent; }
            if local == 0 { continue; }
            payload = payload.saturating_add(node_bytes(node));
            if let NodeKind::Element { tag, href, attrs, .. } = &node.kind {
                if !matches!(node.namespace, crate::html::Namespace::Html | crate::html::Namespace::Svg) || tag == "template" {
                    return Err("Template fragment parsing of foreign or nested template markup is unavailable.".into());
                }
                self.state.work(attrs.len())?;
                payload = payload.saturating_add(tag.len()).saturating_add(href.as_ref().map_or(0, String::len));
            }
        }
        let root_children = children_for(&parsed.nodes[0].children)?;
        payload = payload.saturating_add(root_children.capacity().saturating_mul(std::mem::size_of::<u32>()));
        for (offset, mut node) in parsed.nodes.drain(1..).enumerate() {
            let local = offset + 1;
            let children = children_for(&node.children)?;
            payload = payload.saturating_add(children.capacity().saturating_mul(std::mem::size_of::<u32>()));
            node.children = Vec::new();
            let inert = InertNode {
                kind: Kind::Parsed {
                    source: ParsedSource::Generated(generated_base + offset),
                    initial_template: None,
                },
                owner, parent: parents[local], children,
            };
            let handle = handles[local];
            if handle < FIRST_ENTRY {
                parked_nodes.push((handle, inert));
                entries.push(Entry::ParkedNode);
            } else {
                entries.push(Entry::Node(inert));
            }
            generated.push(GeneratedNode { node: Rc::new(node), unsafe_nonce: parsed.nonce_unsafe_nodes.contains(&local) });
        }
        payload = payload.saturating_add(parked.saturating_mul(size_of::<Node>()));
        self.doc.nodes.try_reserve(parked).map_err(|_| "Cannot reserve future template document slots.")?;
        self.state.tree.parents.try_reserve(parked).map_err(|_| "Cannot reserve parked template parent slots.")?;
        self.state.tree.active.try_reserve(parked).map_err(|_| "Cannot reserve parked template active slots.")?;
        self.state.dom.connected.try_reserve(parked).map_err(|_| "Cannot reserve parked template connection slots.")?;
        let minimum = self.state.inert_documents.entries.len().checked_add(entries.len())
            .and_then(|required| required.saturating_sub(self.state.inert_documents.entries.capacity()).checked_mul(size_of::<Entry>()))
            .and_then(|bytes| self.state.inert_documents.generated.len().checked_add(generated.len())
                .and_then(|required| required.saturating_sub(self.state.inert_documents.generated.capacity()).checked_mul(size_of::<GeneratedNode>()))
                .and_then(|sources| bytes.checked_add(sources))).ok_or("Parked template registry storage overflow.")?;
        let available = self.state.retained.checked_sub(released)
            .and_then(|retained| self.state.retained_limit.checked_sub(retained))
            .and_then(|bytes| bytes.checked_sub(payload)).and_then(|bytes| bytes.checked_sub(minimum))
            .ok_or("Parked template identity bundle allowance exhausted.")?;
        let (index_growth, index_bytes) = self.prepare_adopted_index(parked_nodes.iter().map(|entry| entry.0), available)?;
        let committed = self.reserve_inert_storage(entries.len(), payload, generated.len(), released, index_bytes)?;
        debug_assert_eq!(committed, first);
        let arena = &mut self.state.inert_documents;
        let Entry::Node(root) = &mut arena.entries[fragment_index] else { unreachable!() };
        let old = std::mem::replace(&mut root.children, root_children);
        for child in old {
            arena.node_mut(child).expect("published inert child").parent = None;
        }
        arena.generated.extend(generated);
        arena.entries.extend(entries);
        arena.adopted.commit(index_growth);
        for (key, value) in parked_nodes { arena.adopted.insert(key, value); }
        for _ in 0..parked {
            self.doc.nodes.push(Node {
                namespace: crate::html::Namespace::None,
                kind: NodeKind::DocumentFragment,
                children: Vec::new(),
            });
            self.state.tree.parents.push(None);
            self.state.tree.active.push(false);
            self.state.dom.connected.push(false);
        }
        Ok(())
    }

    pub(super) fn inert_import_source(&self, object: u32) -> Option<ImportSource<'_>> {
        Some(match &self.state.inert_documents.node(object)?.kind {
            Kind::Parsed { source, .. } => {
                let (node, unsafe_nonce) = self.parsed_source(*source)?;
                ImportSource::Parsed { node, unsafe_nonce }
            }
            Kind::Element(tag) => ImportSource::Element(tag),
            Kind::Text(data) => ImportSource::Text(data),
            Kind::Fragment { .. } => ImportSource::Fragment,
            Kind::ProcessingInstruction { target, data } => ImportSource::ProcessingInstruction { target, data },
            Kind::Document(_) | Kind::TemplateDocument { .. } | Kind::Doctype => return None,
        })
    }

    pub(super) fn inert_clone_receiver(&self, object: u32) -> bool {
        self.state.inert_documents.node(object).is_some_and(|node| matches!(node.kind,
            Kind::Fragment { .. } | Kind::Parsed { .. } | Kind::Element(_) | Kind::Text(_)
                | Kind::ProcessingInstruction { .. }))
    }

    pub(super) fn clone_inert_tree(&mut self, object: u32, deep: bool) -> Result<Value, String> {
        #[derive(Clone, Copy)]
        struct Step {
            source: u32,
            owner: u32,
            parent: Option<u32>,
            template: Option<u32>,
        }

        let arena = &self.state.inert_documents;
        let remaining = self.inert_remaining_entries();
        if remaining == 0 { return Err("Inert DOM identity limit reached during cloning.".into()); }
        let first = FIRST_ENTRY + arena.entries.len() as u32;
        let mut generated = None;
        let mut payload = 0usize;
        // Main-document templates carry an ordinary element record, but their
        // content is a distinct host-linked tree. Snapshot the outer element
        // and retain the original owner's identity in the staged clone.
        let (owner, outer) = if let Some(node) = arena.node(object) {
            (node.owner, None)
        } else {
            let (owner, initial) = self.template_source(object).ok_or("Inert clone source is unavailable.")?;
            let index = node_index(object).ok_or("Main template has no native node.")?;
            let source = &self.doc.nodes[index];
            let NodeKind::Element { tag, href, attrs, .. } = &source.kind else { unreachable!() };
            if attrs.iter().any(|attr| attr.name.namespace.is_none()
                && (attr.name.qualified.starts_with("on") || attr.name.html_is("is"))) {
                return Err("Template cloning cannot transfer active handlers or customized built-ins.".into());
            }
            let bytes = copied_node_bytes(source).saturating_add(tag.len()).saturating_add(href.as_ref().map_or(0, String::len));
            self.state.work(attrs.len().saturating_add(1))?;
            self.state.copies(bytes)?;
            payload = bytes.saturating_add(size_of::<Node>() + 2 * size_of::<usize>());
            let generated_index = self.state.inert_documents.generated.len();
            generated = Some(GeneratedNode {
                node: Rc::new(Node {
                    kind: script_clone_node::copy_node_kind(&source.kind)?,
                    namespace: source.namespace, children: Vec::new(),
                }),
                unsafe_nonce: self.doc.nonce_unsafe_nodes.contains(&index),
            });
            let extra=copied_payload_extra(&generated.as_ref().unwrap().node.kind);self.state.copies(extra)?;payload=payload.saturating_add(extra);
            (owner, Some(Kind::Parsed {
                source: ParsedSource::Generated(generated_index),
                initial_template: if deep { initial } else { None },
            }))
        };
        let mut pending = vec![Step { source: object, owner, parent: None, template: None }];
        let mut staged = Vec::new();
        let mut template_links = Vec::new();
        let mut outer = outer;
        while let Some(step) = pending.get(staged.len()) {
            let Step { source, owner, parent, template } = *step;
            let handle = first + staged.len() as u32;
            let arena = &self.state.inert_documents;
            let (children, kind) = if let Some(kind) = outer.as_ref() {
                (&[][..], kind)
            } else {
                let source = arena.node(source).filter(|node| node.owner == owner)
                    .ok_or("Inert clone source ownership changed.")?;
                (if deep { source.children.as_slice() } else { &[][..] }, &source.kind)
            };
            let content = if deep {
                arena.template_contents.get(&source).map(|&fragment| {
                    let node = arena.node(fragment).filter(|node| node.parent.is_none()
                        && matches!(node.kind, Kind::Fragment { template: Some(host) } if host == source))
                        .ok_or("Missing template content identity during cloning.")?;
                    Ok::<_, String>((fragment, node.owner))
                }).transpose()?
            } else { None };
            let extra = children.len().saturating_add(usize::from(content.is_some()));
            if extra > remaining - pending.len() {
                return Err("Inert DOM identity limit reached during cloning.".into());
            }
            let string_bytes = match kind {
                Kind::Element(name) | Kind::Text(name) => name.len(),
                Kind::ProcessingInstruction { target, data } => target.len().saturating_add(data.len()),
                _ => 0,
            };
            self.state.work(extra.saturating_add(1))?;
            self.state.copies(string_bytes)?;
            pending.try_reserve(extra).map_err(|_| "Cannot reserve inert clone traversal.")?;
            staged.try_reserve(1).map_err(|_| "Cannot reserve inert clone nodes.")?;
            let child_start = first + pending.len() as u32;
            let child_count = if deep && outer.is_none() {
                let source = self.state.inert_documents.node(source).ok_or("Inert clone source is unavailable.")?;
                pending.extend(source.children.iter().map(|&source| Step {
                    source, owner, parent: Some(handle), template: None,
                }));
                source.children.len()
            } else { 0 };
            if let Some((source, owner)) = content {
                let new_fragment = first + pending.len() as u32;
                template_links.try_reserve(1).map_err(|_| "Cannot reserve cloned template identities.")?;
                template_links.push((handle, new_fragment));
                pending.push(Step { source, owner, parent: None, template: Some(handle) });
            }
            let mut children = Vec::new();
            children.try_reserve_exact(child_count).map_err(|_| "Cannot reserve inert clone children.")?;
            children.extend(child_start..child_start + child_count as u32);
            let kind = if let Some(kind) = outer.take() { kind } else {
                let node = self.state.inert_documents.node(source).ok_or("Inert clone source is unavailable.")?;
                match &node.kind {
                    Kind::Fragment { .. } => Kind::Fragment { template },
                    Kind::Parsed { source, initial_template } =>
                        Kind::Parsed { source: *source, initial_template: if deep { *initial_template } else { None } },
                    Kind::Element(name) => Kind::Element(name.clone()),
                    Kind::Text(data) => Kind::Text(data.clone()),
                    Kind::ProcessingInstruction { target, data } =>
                        Kind::ProcessingInstruction { target: target.clone(), data: data.clone() },
                    Kind::Document(_) | Kind::TemplateDocument { .. } | Kind::Doctype =>
                        return Err("Cloning this inert document node is unavailable.".into()),
                }
            };
            payload = payload.saturating_add(string_bytes)
                .saturating_add(children.capacity().saturating_mul(size_of::<u32>()));
            staged.push(Entry::Node(InertNode { kind, owner, parent, children }));
        }
        let (committed, indexes) = self.prepare_template_indexes(staged.len(), payload,
            usize::from(generated.is_some()), None, &template_links)?;
        debug_assert_eq!(committed, first);
        let arena = &mut self.state.inert_documents;
        if let Some(source) = generated { arena.generated.push(source); }
        arena.entries.extend(staged);
        arena.template_documents.commit(indexes.documents);
        arena.template_contents.commit(indexes.contents);
        for (key, value) in template_links { arena.template_contents.insert(key, value); }
        Ok(Value::Host(first))
    }

    fn inert_remaining_entries(&self) -> usize {
        self.inert_remaining_entries_with_release(0)
    }

    fn prepare_template_indexes(&mut self, count: usize, payload: usize, generated: usize,
        document: Option<(u32, u32)>, contents: &[(u32, u32)]) -> Result<(u32, TemplateIndexGrowth), String>
    {
        let mut checks = 0usize;
        for (map, added) in [
            (&self.state.inert_documents.template_documents, usize::from(document.is_some())),
            (&self.state.inert_documents.template_contents, contents.len()),
        ] {
            let length = map.len().checked_add(added).ok_or("Template index count overflow.")?;
            let lookup = (usize::BITS - length.leading_zeros()) as usize + 1;
            checks = added.checked_mul(3 * lookup + 1).and_then(|work| checks.checked_add(work))
                .ok_or("Template index work overflow.")?;
        }
        self.state.work(checks)?;
        let arena = &self.state.inert_documents;
        let entry_minimum = arena.entries.len().checked_add(count)
            .and_then(|required| required.saturating_sub(arena.entries.capacity()).checked_mul(size_of::<Entry>()))
            .ok_or("Template entry storage overflow.")?;
        let source_minimum = arena.generated.len().checked_add(generated)
            .and_then(|required| required.saturating_sub(arena.generated.capacity()).checked_mul(size_of::<GeneratedNode>()))
            .ok_or("Template source storage overflow.")?;
        let available = self.state.retained_limit.saturating_sub(self.state.retained)
            .checked_sub(payload).and_then(|bytes| bytes.checked_sub(entry_minimum))
            .and_then(|bytes| bytes.checked_sub(source_minimum)).ok_or("Template identity bundle allowance exhausted.")?;
        let content_minimum = arena.template_contents.minimum_growth(contents.len())?;
        let (documents, document_bytes) = arena.template_documents.prepare(usize::from(document.is_some()),
            available.checked_sub(content_minimum).ok_or("Template content index allowance exhausted.")?)?;
        let (content_growth, content_bytes) = arena.template_contents.prepare(contents.len(), available - document_bytes)?;

        let mut movement = 0usize;
        let mut copies = 0usize;
        for (map, links, grew) in [
            (&arena.template_documents, document.as_slice(), documents.is_some()),
            (&arena.template_contents, contents, content_growth.is_some()),
        ] {
            // Preparation searches, strict ordering validation and the later
            // publication searches all spend work before any live mutation.
            if links.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                return Err("Template index publication requires distinct ordered identities.".into());
            }
            let mut shifted = 0usize;
            for &(key, _) in links {
                if map.contains_key(&key) { return Err("Template content identity already exists.".into()); }
                shifted = shifted.checked_add(map.insert_work(&key) - map.lookup_work())
                    .ok_or("Template index movement overflow.")?;
            }
            // Earlier keys in this ordered batch never shift for a later key.
            let moved = shifted.checked_add(if grew { map.len() } else { 0 })
                .ok_or("Template index movement overflow.")?;
            movement = movement.checked_add(moved).ok_or("Template index work overflow.")?;
            copies = moved.checked_add(links.len()).and_then(|slots| slots.checked_mul(size_of::<(u32, u32)>()))
                .and_then(|bytes| copies.checked_add(bytes)).ok_or("Template index copy work overflow.")?;
        }
        self.state.work(movement)?;
        self.state.copies(copies)?;
        let metadata = document_bytes.checked_add(content_bytes).ok_or("Template index storage overflow.")?;
        let handle = self.reserve_inert_storage(count, payload, generated, 0, metadata)?;
        Ok((handle, TemplateIndexGrowth { documents, contents: content_growth }))
    }

    fn prepare_adopted_index(&mut self, keys: impl ExactSizeIterator<Item=u32>, available: usize)
        -> Result<(Option<Vec<(u32, InertNode)>>, usize), String>
    {
        let count = keys.len();
        let map = &self.state.inert_documents.adopted;
        let length = map.len().checked_add(count).ok_or("Adopted node index count overflow.")?;
        let lookup = (usize::BITS - length.leading_zeros()) as usize + 1;
        self.state.work(count.checked_mul(3 * lookup + 1).ok_or("Adopted node index work overflow.")?)?;
        let map = &self.state.inert_documents.adopted;
        let (growth, added) = map.prepare(count, available)?;
        let mut moved = if growth.is_some() { map.len() } else { 0 };
        let mut previous = None;
        for key in keys {
            if previous.is_some_and(|before| before >= key) || map.contains_key(&key) {
                return Err("Adoption requires distinct ordered unpublished identities.".into());
            }
            previous = Some(key);
            moved = moved.checked_add(map.insert_work(&key) - map.lookup_work())
                .ok_or("Adopted node index movement overflow.")?;
        }
        let copies = moved.checked_add(count).and_then(|slots| slots.checked_mul(size_of::<(u32, InertNode)>()))
            .ok_or("Adopted node index copy work overflow.")?;
        self.state.work(moved)?;
        self.state.copies(copies)?;
        Ok((growth, added))
    }

    fn inert_remaining_entries_with_release(&self, released: usize) -> usize {
        let arena = &self.state.inert_documents;
        let representation = (ENTRY_END - FIRST_ENTRY) as usize;
        let spare = arena.entries.capacity().saturating_sub(arena.entries.len());
        let available = self.state.retained_limit.saturating_sub(self.state.retained.saturating_sub(released));
        let remaining = representation.saturating_sub(arena.entries.len())
            .min(spare.saturating_add(available / size_of::<Entry>()));
        #[cfg(test)]
        if self.state.limits.test_entry_arenas {
            return remaining.min(self.state.limits.host.extra_nodes.saturating_sub(arena.entries.len()));
        }
        remaining
    }

    // Entry and generated-source backing grow jointly with the admitted payload.
    // Prepare both replacements before publishing either capacity or any handle.
    fn reserve_inert_entries(&mut self, count: usize, payload: usize, generated: usize) -> Result<u32, String> {
        self.reserve_inert_storage(count, payload, generated, 0, 0)
    }

    fn reserve_inert_storage(&mut self, count: usize, payload: usize, generated: usize, released: usize, metadata: usize) -> Result<u32, String> {
        let arena = &self.state.inert_documents;
        let current = arena.entries.len();
        #[cfg(test)]
        if self.state.limits.test_entry_arenas
            && count > self.state.limits.host.extra_nodes.saturating_sub(current)
        {
            return Err(format!("Inert DOM identity limit reached ({} entries).", self.state.limits.host.extra_nodes));
        }
        if count > ((ENTRY_END - FIRST_ENTRY) as usize).saturating_sub(current) {
            return Err("Inert DOM typed identity space exhausted.".into());
        }
        if count > self.inert_remaining_entries_with_release(released) {
            return Err(format!("Inert DOM entry backing allowance exhausted ({} bytes available).",
                self.state.retained_limit.saturating_sub(self.state.retained)));
        }
        let handle = FIRST_ENTRY.checked_add(u32::try_from(current).map_err(|_| "Inert identity index overflow.")?)
            .ok_or("Inert identity overflow.")?;
        self.state.work(count.saturating_add(generated))?;
        let retained = self.state.retained.checked_sub(released).ok_or("Inert retained storage refund underflow.")?;
        let storage = payload.checked_add(metadata).ok_or("Inert payload/index storage overflow.")?;
        let available = self.state.retained_limit.saturating_sub(retained)
            .checked_sub(storage).ok_or("Inert DOM retained payload allowance exhausted.")?;
        let arena = &self.state.inert_documents;
        let minimum = arena.generated.len().checked_add(generated)
            .and_then(|required| required.saturating_sub(arena.generated.capacity()).checked_mul(size_of::<GeneratedNode>()))
            .ok_or("Inert source capacity overflow.")?;
        let (entries, entry_bytes) = retained_storage::grow(&arena.entries, count,
            available.checked_sub(minimum).ok_or("Inert source backing allowance exhausted.")?)?;
        let (sources, source_bytes) = retained_storage::grow(&arena.generated, generated, available - entry_bytes)?;
        let moving = if entries.is_some() { current * size_of::<Entry>() } else { 0 };
        let moving = moving.checked_add(if sources.is_some() { arena.generated.len() * size_of::<GeneratedNode>() } else { 0 })
            .ok_or("Inert source movement overflow.")?;
        let work = count.checked_mul(size_of::<Entry>())
            .and_then(|bytes| generated.checked_mul(size_of::<GeneratedNode>()).and_then(|sources| bytes.checked_add(sources)))
            .and_then(|bytes| bytes.checked_add(moving)).and_then(|bytes| bytes.checked_add(payload))
            .ok_or("Inert identity copy work overflow.")?;
        let added = storage.checked_add(entry_bytes).and_then(|bytes| bytes.checked_add(source_bytes))
            .ok_or("Inert retained storage overflow.")?;
        self.state.replace_storage(released, added, work)?;
        let arena = &mut self.state.inert_documents;
        if let Some(mut next) = entries { next.append(&mut arena.entries); arena.entries = next; }
        if let Some(mut next) = sources { next.append(&mut arena.generated); arena.generated = next; }
        Ok(handle)
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
        let document = FIRST_ENTRY + self.state.inert_documents.entries.len() as u32;
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
        let links = |items: &[u32]| -> Result<Vec<u32>, String> {
            let mut values = Vec::new();
            values.try_reserve_exact(items.len()).map_err(|_| "Cannot reserve inert document children.")?;
            values.extend_from_slice(items);
            Ok(values)
        };
        let text_copy = script_clone_node::copy_text;
        let mut entries = Vec::new();
        entries.try_reserve_exact(count).map_err(|_| "Cannot stage inert document entries.")?;
        entries.extend([
            node(Kind::Document(DocumentNodes {
                implementation, doctype: Some(doctype), url: text_copy("about:blank")?, html, head, body, title: normalized,
            }), None, links(&[doctype, html])?),
            Entry::Implementation,
            node(Kind::Doctype, Some(document), Vec::new()),
            node(Kind::Element(text_copy("html")?), Some(document), links(&[head, body])?),
            node(Kind::Element(text_copy("head")?), Some(html), if has_title { links(&[title_element])? } else { Vec::new() }),
            node(Kind::Element(text_copy("body")?), Some(html), Vec::new()),
        ]);
        if let Some(title) = title {
            entries.extend([
                node(Kind::Element(text_copy("title")?), Some(head), links(&[text])?),
                node(Kind::Text(title), Some(title_element), Vec::new()),
            ]);
        }
        let payload = entries.iter().try_fold(0usize, |bytes, entry| {
            let Entry::Node(node) = entry else { return Ok::<_, String>(bytes); };
            let text = match &node.kind {
                Kind::Document(doc) => doc.url.capacity().checked_add(doc.title.capacity()),
                Kind::Element(text) | Kind::Text(text) => Some(text.capacity()),
                _ => Some(0),
            }.ok_or("Inert document payload overflow.")?;
            node.children.capacity().checked_mul(size_of::<u32>())
                .and_then(|links| bytes.checked_add(links)).and_then(|bytes| bytes.checked_add(text))
                .ok_or("Inert document payload overflow.".into())
        })?;
        let admitted = self.reserve_inert_entries(count, payload, 0)?;
        debug_assert_eq!(admitted, document);
        self.state.inert_documents.entries.extend(entries);
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
        let handle = self.reserve_inert_entries(1, bytes, 0)?;
        self.state.inert_documents.entries.push(Entry::Node(InertNode {
            kind, owner, parent: None, children: Vec::new(),
        }));
        Ok(Value::Host(handle))
    }

    fn inert_text_value(&self, object: u32) -> Option<&str> {
        match &self.state.inert_documents.node(object)?.kind {
            Kind::Text(value) => Some(value),
            Kind::Parsed { source, .. } => match &self.parsed_source(*source)?.0.kind {
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
        if self.inert_implementation(object) || self.inert_parser(object) { return Ok(Value::Undefined); }
        if property == "childNodes" { return self.create_inert_child_list(object); }
        let node = self.state.inert_documents.node(object).ok_or("Unknown inert Node.")?;
        if let Kind::Parsed { source, .. } = node.kind {
            let parsed = self.parsed_source(source).ok_or("Inert parsed node is unavailable.")?.0;
            if let Some(data) = parsed.kind.character_data() {
                match property {
                    "data" | "nodeValue" | "textContent" => {
                        self.state.copies(data.len())?;
                        let data = self.parsed_source(source).unwrap().0.kind.character_data().unwrap();
                        return Ok(Value::String(data.into()));
                    }
                    "length" => {
                        self.state.work(data.len())?;
                        let data = self.parsed_source(source).unwrap().0.kind.character_data().unwrap();
                        return Ok(Value::Number(data.encode_utf16().count() as f64));
                    }
                    _ => {}
                }
            }
            if let NodeKind::Element { tag, id, .. } = &parsed.kind {
                match property {
                    "localName" | "nodeName" | "tagName" => {
                        self.state.copies(tag.len())?;
                        let parsed = self.parsed_source(source).unwrap().0;
                        let NodeKind::Element { tag, .. } = &parsed.kind else { unreachable!() };
                        return Ok(Value::String(if property != "localName" && parsed.namespace == crate::html::Namespace::Html {
                            tag.to_ascii_uppercase()
                        } else { tag.clone() }));
                    }
                    "namespaceURI" => return Ok(parsed.namespace.element_uri()
                        .map_or(Value::Null, |ns| Value::String(ns.into()))),
                    "id" => {
                        let id = id.as_deref().unwrap_or_default();
                        self.state.copies(id.len())?;
                        let NodeKind::Element { id, .. } = &self.parsed_source(source).unwrap().0.kind else { unreachable!() };
                        return Ok(Value::String(id.as_deref().unwrap_or_default().into()));
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
            "baseURI" => {
                let owner = self.state.inert_documents.node(node.owner);
                match owner.map(|owner| &owner.kind) {
                    Some(Kind::Document(document)) => string(&document.url),
                    _ => string("about:blank"),
                }
            }
            "isConnected" => {
                let mut id = object;
                while let Some(parent) = self.state.inert_documents.node(id).and_then(|node| node.parent) { id = parent; }
                Value::Bool(id == node.owner)
            }
            "nodeName" => match &node.kind {
                Kind::Document(_) | Kind::TemplateDocument { .. } => string("#document"),
                Kind::Fragment { .. } => string("#document-fragment"),
                Kind::Parsed { source, .. } => match &self.parsed_source(*source).ok_or("Inert parsed node is unavailable.")?.0.kind {
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
            "textContent" if matches!(node.kind, Kind::Fragment { .. } | Kind::Parsed { .. } | Kind::Element(_) | Kind::Text(_)) => return self.inert_text_content(object),
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
            "doctype" => host(document.and_then(|doc| doc.doctype)),
            "implementation" => host(document.map(|doc| doc.implementation).or(template_implementation)),
            "title" if template_implementation.is_some() => string(""),
            "title" if document.is_some() => {
                self.state.copies(document.unwrap().title.len())?;
                let Kind::Document(document) = &self.state.inert_documents.node(object).unwrap().kind else { unreachable!() };
                string(&document.title)
            }
            "URL" | "documentURI" if html_document => string(document.map_or("about:blank", |doc| &doc.url)),
            "contentType" if html_document => string("text/html"),
            "compatMode" if html_document => string(if document.is_some_and(|doc| doc.doctype.is_none()) {
                "BackCompat"
            } else { "CSS1Compat" }),
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

fn template_node_can_park(node: &Node) -> bool {
    match &node.kind {
        NodeKind::Text(_) | NodeKind::Comment(_) => true,
        // Only the checked template parser feeds this reservation path.
        // It publishes SVG provenance after validating the entire island.
        NodeKind::Element { .. } if node.namespace == crate::html::Namespace::Svg => true,
        NodeKind::Element { tag, attrs, .. } => {
            node.namespace == crate::html::Namespace::Html
                && (tag == "style" || script_clone_node::ordinary_clone_element(tag)
                    || crate::html::valid_custom_element_name(tag))
                && !attrs.iter().any(|attr| attr.name.namespace.is_some()
                    || attr.name.qualified.starts_with("on")
                    // STYLE uses the same native stylesheet admission even if
                    // its parsed markup carries an ordinary `is` attribute.
                    || tag != "style" && attr.name.html_is("is"))
        }
        _ => false,
    }
}
