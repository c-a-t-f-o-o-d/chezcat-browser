//! Live, bounded tag/class/direct-element collections. Each handle retains a root
//! and filter, never a snapshot of node references. Queries share DOM work
//! limits and preserve the document's inert-subtree policy.
//! https://dom.spec.whatwg.org/#concept-getelementsbytagname
use super::*;
use crate::dom_string::DomStringRef;

// Independent of adjacent Window/frame/node identities.
const COLLECTION_BASE:u32=0x8c00_0000;
const COLLECTION_END:u32=0x8d00_0000;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Query { Tag, Namespace, Class }

impl Query {
    pub(super) fn arity(self) -> usize { if self == Self::Namespace { 2 } else { 1 } }
}

pub(super) fn operation(method: &str) -> Option<(Query, bool)> {
    use crate::javascript::*;
    match method {
        DOCUMENT_GET_ELEMENTS_TAG => Some((Query::Tag, true)),
        DOCUMENT_GET_ELEMENTS_TAG_NS => Some((Query::Namespace, true)),
        DOCUMENT_GET_ELEMENTS_CLASS => Some((Query::Class, true)),
        ELEMENT_GET_ELEMENTS_TAG => Some((Query::Tag, false)),
        ELEMENT_GET_ELEMENTS_TAG_NS => Some((Query::Namespace, false)),
        ELEMENT_GET_ELEMENTS_CLASS => Some((Query::Class, false)),
        _ => None,
    }
}
#[cfg(test)]
const MAX_COLLECTIONS:usize=128;

pub(super) struct TagCollection {
    root: u32,
    filter: Filter,
}

enum Filter {
    Children(u32),
    Tag(DomString,bool),
    Namespace(ExpandedNameFilter),
    Classes(Vec<Vec<u16>>),
}

// The local name and namespace are DOMStrings. A prefix is not part of either
// comparison. Store the exact units without retaining a display projection.
struct ExpandedNameFilter {
    namespace: Option<Vec<u16>>,
    local: Vec<u16>,
}

impl ExpandedNameFilter {
    fn owned_bytes(&self) -> usize {
        (self.local.capacity() + self.namespace.as_ref().map_or(0, Vec::capacity)) * size_of::<u16>()
    }

    fn all(&self) -> bool {
        self.namespace.as_deref() == Some(&[0x2a]) && self.local == [0x2a]
    }

    fn matches(&self, namespace: crate::html::element_names::NamespaceRef<'_>, local: DomStringRef<'_>) -> bool {
        use crate::html::element_names::NamespaceRef;
        let actual = match namespace {
            NamespaceRef::Null => None,
            NamespaceRef::Static(uri) => Some(DomStringRef::Scalar(uri)),
            NamespaceRef::Owned(uri) => Some(uri.view()),
        };
        let same_namespace = match (self.namespace.as_deref(), actual) {
            (Some([0x2a]), _) | (None, None) => true,
            (Some(expected), Some(value)) => value.same_units(DomStringRef::Raw(expected)),
            _ => false,
        };
        same_namespace && (self.local == [0x2a] || local.same_units(DomStringRef::Raw(&self.local)))
    }

    fn matches_node(&self, node: &Node) -> Result<bool, String> {
        let NodeKind::Element { tag, expanded_name, .. } = &node.kind else { return Ok(false); };
        if self.all() { return Ok(true); }
        let name = crate::html::element_names::local(node.namespace, tag, expanded_name)?;
        let namespace = crate::html::element_names::namespace(node.namespace, expanded_name)?;
        Ok(self.matches(namespace, name))
    }

    fn comparison_bytes(&self, node: &Node) -> Result<usize, String> {
        use crate::html::element_names::NamespaceRef;
        let NodeKind::Element { tag, expanded_name, .. } = &node.kind else { return Ok(0); };
        if self.all() { return Ok(0); }
        let local = crate::html::element_names::local(node.namespace, tag, expanded_name)?;
        let namespace = crate::html::element_names::namespace(node.namespace, expanded_name)?;
        let uri_bytes = match namespace {
            NamespaceRef::Null => 0,
            NamespaceRef::Static(uri) => uri.len(),
            NamespaceRef::Owned(uri) => uri.logical_bytes(),
        };
        Ok(self.field_work(uri_bytes, local.logical_bytes()))
    }

    fn field_work(&self, uri_bytes: usize, local_bytes: usize) -> usize {
        // Comparing an exact unit filter stops at the shorter string. A
        // scalar view can consume up to four source bytes for its next unit.
        let compare = |actual: usize, expected: &[u16]| {
            if expected == [0x2a] { return 0; }
            actual.min(expected.len().saturating_add(1).saturating_mul(4))
                .saturating_add(expected.len().saturating_mul(2).min(actual.saturating_add(1).saturating_mul(2)))
        };
        compare(local_bytes, &self.local).saturating_add(self.namespace.as_deref().map_or(0, |ns| compare(uri_bytes, ns)))
    }
}

fn converted_string(value: &Value) -> Result<DomStringRef<'_>, String> {
    match value {
        Value::String(s) => Ok(DomStringRef::Scalar(s)),
        Value::StringUnits(units) => Ok(DomStringRef::Raw(units)),
        _ => Err("Expected the converted DOMString collection argument.".into()),
    }
}

fn copy_filter_units(text: DomStringRef<'_>, length: usize, available: &mut usize) -> Result<Vec<u16>, String> {
    if length > *available / size_of::<u16>() { return Err("DOM retained expanded-name filter allowance exhausted.".into()); }
    let mut units = Vec::new();
    units.try_reserve_exact(length).map_err(|_| "Cannot reserve expanded-name collection filter.")?;
    let bytes = units.capacity().checked_mul(size_of::<u16>()).ok_or("Expanded-name filter size overflow.")?;
    *available = available.checked_sub(bytes).ok_or("DOM retained expanded-name filter allowance exhausted.")?;
    units.extend(text.code_units());
    Ok(units)
}
fn qualified_matches(local:crate::dom_string::DomStringRef<'_>,prefix:Option<&DomString>,name:&DomString,lowercase:bool)->bool{
    let actual=prefix.into_iter().flat_map(DomString::code_units).chain(prefix.map(|_|0x3a)).chain(local.code_units());actual.eq(name.code_units().map(|u|if lowercase&&(0x41..=0x5a).contains(&u){u+0x20}else{u}))
}
fn tag_matches(node:&Node,name:&DomString,html:bool)->Result<bool,String>{let NodeKind::Element{tag,expanded_name,..}=&node.kind else{return Ok(false);};if name=="*"{return Ok(true);}let prefix=crate::html::element_names::prefix(node.namespace,expanded_name)?;let local=crate::html::element_names::local(node.namespace,tag,expanded_name)?;Ok(qualified_matches(local,prefix,name,html&&node.namespace==crate::html::Namespace::Html))}
impl Filter {
    fn owned_bytes(&self) -> usize {
        match self {
            Self::Children(_) => 0,
            Self::Tag(name,_) => name.capacity(),
            Self::Namespace(filter) => filter.owned_bytes(),
            Self::Classes(names) => names.capacity()*size_of::<Vec<u16>>()+names.iter().map(|name| name.capacity()*size_of::<u16>()).sum::<usize>(),
        }
    }
}

pub(super) fn class_tokens(value: DomStringRef<'_>) -> impl Iterator<Item = DomStringRef<'_>> {
    let (text, units) = match value {
        DomStringRef::Scalar(text) => (text, &[][..]),
        DomStringRef::Raw(units) => ("", units),
    };
    let scalar = text.split(class_whitespace).map(DomStringRef::Scalar);
    let raw = units.split(|unit| matches!(unit, 0x09 | 0x0a | 0x0c | 0x0d | 0x20)).map(DomStringRef::Raw);
    scalar.chain(raw).filter(|token| !token.is_empty())
}

fn class_token_eq(left: DomStringRef<'_>, right: &[u16], quirks: bool) -> bool {
    if !quirks { return left.code_units().eq(right.iter().copied()); }
    let fold = |unit| if (0x41..=0x5a).contains(&unit) { unit + 0x20 } else { unit };
    left.code_units().map(fold).eq(right.iter().copied().map(fold))
}

fn class_filter_matches(attribute: Option<&DomString>, names: &[Vec<u16>], quirks: bool) -> bool {
    attribute.is_some_and(|class| names.iter().all(|name|
        class_tokens(class.view()).any(|token| class_token_eq(token, name, quirks))))
}

pub(super) fn collection_handle_index(handle: u32) -> Option<usize> {
    (COLLECTION_BASE..COLLECTION_END)
        .contains(&handle)
        .then(|| (handle - COLLECTION_BASE) as usize)
}

impl Bridge<'_> {
    pub(super) fn create_namespace_collection(&mut self, root: u32, args: &[Value]) -> Result<Value, String> {
        let [ns, local, ..] = args else { return Err("getElementsByTagNameNS requires two arguments.".into()); };
        let namespace = if matches!(ns, Value::Null | Value::Undefined) { None }
            else { Some(converted_string(ns)?).filter(|s| !s.is_empty()) };
        let local = converted_string(local)?;
        let string_limit = self.state.limits.host.string_bytes;
        if local.logical_bytes() > string_limit || namespace.is_some_and(|s| s.logical_bytes() > string_limit) {
            return Err("Expanded-name filter input exceeds the DOM string allowance.".into());
        }
        let input_bytes = local.logical_bytes().checked_add(namespace.map_or(0, DomStringRef::logical_bytes))
            .ok_or("Expanded-name collection comparison overflow.")?;
        self.state.work(input_bytes.saturating_add(1))?;
        let n = namespace.map_or(0, |s| s.code_units().count());
        let l = local.code_units().count();
        let required = n.checked_add(l).and_then(|units| units.checked_mul(size_of::<u16>()))
            .ok_or("Expanded-name collection filter size overflow.")?;
        let mut available = self.state.retained_limit.saturating_sub(self.state.retained);
        if required > available { return Err("DOM retained expanded-name filter allowance exhausted.".into()); }
        self.state.copies(required)?;
        let namespace = namespace.map(|s| copy_filter_units(s, n, &mut available)).transpose()?;
        let local = copy_filter_units(local, l, &mut available)?;
        self.store_live_collection(root, Filter::Namespace(ExpandedNameFilter { namespace, local }))
    }
    pub(super) fn collection_interface_receiver(&self, object: u32) -> Option<bool> {
        if let Some((_, node)) = self.frame_contains_node(object) { return Some(node == 0); }
        if object == DOCUMENT || self.inert_node_type(object) == Some(9) { return Some(true); }
        if self.inert_node_type(object) == Some(1) || node_index(object).is_some_and(|n|
            self.dom_accessible(n) && matches!(self.doc.nodes[n].kind, NodeKind::Element { .. })) {
            return Some(false);
        }
        None
    }
    pub(super) fn canonical_collection_member(&self, object: u32, property: &str) -> bool {
        matches!(property, "getElementsByTagName" | "getElementsByTagNameNS" | "getElementsByClassName")
            && self.collection_interface_receiver(object).is_some() && !self.form_named_control(object, property)
    }
    fn verify_collection_root(&mut self, object: u32) -> Result<(), String> {
        if let Some((owner, _)) = self.frame_contains_node(object) { self.contains_child_document(owner)?; }
        else if object != DOCUMENT && !matches!(self.inert_node_type(object), Some(1 | 9)) { self.node(object)?; }
        Ok(())
    }
    fn inert_class_attribute(&self, node: u32) -> Option<&DomString> {
        self.inert_element_attributes(node)?.iter().find(|attr| attr.name.html_is("class"))
            .map(|attr| &attr.value)
    }

    fn frame_collection_handles(&mut self, owner: u32, root: usize, index: usize) -> Result<Vec<u32>, String> {
        let doc = self.contains_child_document(owner)?;
        let count = doc.nodes.len();
        if doc.nodes[root].children.len() > count { return Err("Child collection root has invalid topology.".into()); }
        if doc.nodes[root].children.is_empty()
            || matches!(&self.state.dom.collections[index].filter, Filter::Classes(names) if names.is_empty()) {
            return Ok(Vec::new());
        }
        let available = self.state.retained_limit.saturating_sub(self.state.retained);
        let required = count.checked_mul(size_of::<usize>() + size_of::<u32>()).ok_or("Child collection storage overflow.")?;
        if required > available { return Err("Child collection traversal storage exhausted.".into()); }
        let mut stack = Vec::new();
        let mut result = Vec::new();
        stack.try_reserve_exact(count).map_err(|_| "Cannot reserve child collection traversal.")?;
        result.try_reserve_exact(count).map_err(|_| "Cannot reserve child collection handles.")?;
        let storage = stack.capacity().checked_mul(size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(result.capacity().checked_mul(size_of::<u32>())?))
            .filter(|&bytes| bytes <= available).ok_or("Child collection traversal storage exhausted.")?;
        self.state.copies(storage)?;
        stack.extend(self.contains_child_document(owner)?.nodes[root].children.iter().rev().copied());
        let mut visited = 0;
        while let Some(node) = stack.pop() {
            visited += 1;
            if visited > count { return Err("Child collection topology repeats a node.".into()); }
            let doc = self.contains_child_document(owner)?;
            let record = doc.nodes.get(node).ok_or("Unknown child collection node.")?;
            let NodeKind::Element { tag, expanded_name, attrs, .. } = &record.kind else { continue; };
            let inert = dom_tree::inert_dom_tag(tag, doc.scripting_enabled);
            if !inert {
                if record.children.len() > count - stack.len() { return Err("Child collection topology exceeds its document.".into()); }
                stack.extend(record.children.iter().rev().copied());
            } else if !dom_tree::exposes_inert_host(tag, record.namespace) { continue; }
            let filter = &self.state.dom.collections[index].filter;
            let work = match filter {
                Filter::Tag(_, _) => crate::html::element_names::qualified_work(tag, expanded_name),
                Filter::Namespace(query) => query.comparison_bytes(record)?,
                Filter::Classes(names) => attrs.len().saturating_add(
                    doc.dom_attr(node, "class").map_or(0, DomString::logical_bytes).saturating_mul(names.len())),
                Filter::Children(_) => return Err("Child collections use the direct children view.".into()),
            };
            self.state.work(work.saturating_add(1))?;
            let doc = self.contains_child_document(owner)?;
            let matches = match &self.state.dom.collections[index].filter {
                Filter::Tag(name, html) => tag_matches(&doc.nodes[node], name, *html)?,
                Filter::Namespace(query) => query.matches_node(&doc.nodes[node])?,
                Filter::Classes(names) => class_filter_matches(doc.dom_attr(node, "class"), names, doc.mode == crate::html::DocumentMode::Quirks),
                Filter::Children(_) => unreachable!(),
            };
            if matches { result.push(self.frame_child_node_handle(owner, node)?); }
        }
        Ok(result)
    }

    fn tag_collection_handles(&mut self,handle:u32)->Result<Vec<u32>,String>{
        let index=collection_handle_index(handle).ok_or("Invalid HTMLCollection handle.")?;let root=self.state.dom.collections.get(index).ok_or("Missing live collection.")?.root;let owner=root;
        if let Some((window, node)) = self.frame_contains_node(root) {
            return self.frame_collection_handles(window, node, index);
        }
        if self.inert_node_type(owner).is_none(){let nodes=self.tag_collection_nodes(handle)?;self.state.copies(nodes.len().checked_mul(size_of::<u32>()).ok_or("Collection handle conversion overflow.")?)?;let mut result=Vec::new();result.try_reserve_exact(nodes.len()).map_err(|_|"Cannot reserve live collection handles.")?;result.extend(nodes.into_iter().map(node_handle));return Ok(result);}
        if matches!(&self.state.dom.collections[index].filter,Filter::Classes(names) if names.is_empty()){return Ok(Vec::new());}
        let mut pending=Vec::new();let mut result=Vec::new();let count=self.inert_children(owner).ok_or("Adopted collection root is unavailable.")?.len();self.state.work(count)?;self.state.copies(count.checked_mul(size_of::<u32>()).ok_or("Collection root traversal overflow.")?)?;pending.try_reserve_exact(count).map_err(|_|"Cannot reserve adopted collection traversal.")?;pending.extend(self.inert_children(owner).unwrap().iter().rev().copied());
        let quirks = self.inert_document_mode_for_node(owner) == Some(crate::html::DocumentMode::Quirks);
        let maximum=self.inert_node_count();let mut visited=0usize;
        while let Some(node)=pending.pop(){
            visited=visited.checked_add(1).filter(|&count|count<=maximum).ok_or("Cyclic adopted collection topology.")?;self.state.work(1)?;
            let source=self.inert_import_source(node);let element=matches!(source,Some(script_inert_documents::ImportSource::Element(_)|script_inert_documents::ImportSource::Parsed{node:Node{kind:NodeKind::Element{..},..},..}));
            let work=match &source{Some(script_inert_documents::ImportSource::Element(tag))=>tag.len(),Some(script_inert_documents::ImportSource::Parsed{node:Node{kind:NodeKind::Element{tag,attrs,expanded_name,..},..},..})=>attrs.iter().fold(attrs.len()+crate::html::element_names::qualified_work(tag,expanded_name),|n,attr|n.saturating_add(attr.name.bytes())),_=>0};self.state.work(work)?;
            let classes=match &self.state.dom.collections[index].filter{Filter::Classes(names)=>names.len(),_=>0};
            if classes != 0 {
                let length = self.inert_class_attribute(node).map_or(0, DomString::logical_bytes);
                let scans = length.checked_mul(classes).ok_or("Adopted class scan overflow.")?;
                self.state.work(scans)?;
                self.state.copies(scans)?;
            }
            if let Filter::Namespace(query) = &self.state.dom.collections[index].filter {
                let bytes = match self.inert_import_source(node) {
                    Some(script_inert_documents::ImportSource::Parsed { node, .. }) => query.comparison_bytes(node)?,
                    Some(script_inert_documents::ImportSource::Element(local)) =>
                        query.field_work("http://www.w3.org/1999/xhtml".len(), local.len()),
                    _ => 0,
                };
                self.state.work(bytes.saturating_add(1))?;
            }
            let filter=&self.state.dom.collections[index].filter;let source=self.inert_import_source(node);
            if matches!(source,Some(script_inert_documents::ImportSource::Parsed{node:Node{namespace:crate::html::Namespace::UnknownForeign,..},..})){return Err("Adopted collection requires known namespace provenance.".into());}
            let matched=if !element{false}else{match (filter,source){
                (Filter::Tag(name,html),Some(script_inert_documents::ImportSource::Element(tag)))=>name=="*"||qualified_matches(crate::dom_string::DomStringRef::Scalar(tag),None,name,*html),
                (Filter::Tag(name,html),Some(script_inert_documents::ImportSource::Parsed{node,..}))=>tag_matches(node,name,*html)?,
                (Filter::Namespace(query), Some(script_inert_documents::ImportSource::Parsed { node, .. })) => query.matches_node(node)?,
                (Filter::Namespace(query), Some(script_inert_documents::ImportSource::Element(local))) =>
                    query.matches(crate::html::element_names::NamespaceRef::Static("http://www.w3.org/1999/xhtml"), DomStringRef::Scalar(local)),
                (Filter::Classes(names), _) => class_filter_matches(self.inert_class_attribute(node), names, quirks),
                _=>return Err("Unsupported adopted collection filter.".into()),
            }};
            if matched{self.state.copies(size_of::<u32>())?;result.try_reserve(1).map_err(|_|"Cannot reserve adopted collection result.")?;result.push(node);}
            if self.template_receiver(node){continue;}
            let count=self.inert_children(node).ok_or("Adopted collection child is unavailable.")?.len();self.state.work(count)?;self.state.copies(count.checked_mul(size_of::<u32>()).ok_or("Adopted collection traversal overflow.")?)?;pending.try_reserve(count).map_err(|_|"Cannot reserve adopted collection descendants.")?;pending.extend(self.inert_children(node).unwrap().iter().rev().copied());
        }
        Ok(result)
    }
    pub(super) fn create_tag_collection(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<Value, String> {
        let name = self.dom_text(value)?;
        self.verify_collection_root(object)?;
        let root=object;
        let kind = if let Some((owner, _)) = self.frame_contains_node(root) { self.contains_child_document(owner)?.kind }
            else { self.inert_document_kind_for_node(root).unwrap_or(self.doc.kind) };
        let html = kind == crate::html::DocumentKind::Html;
        self.store_live_collection(root, Filter::Tag(name,html))
    }

    pub(super) fn create_class_collection(
        &mut self,
        object: u32,
        value: &Value,
    ) -> Result<Value, String> {
        self.verify_collection_root(object)?;
        let root=object;
        let text = self.dom_text(value)?;
        let bytes = text.logical_bytes();
        self.state.work(bytes)?;
        self.state.copies(bytes)?;
        let available = self.state.retained_limit.saturating_sub(self.state.retained);
        let count = class_tokens(text.view()).count();
        self.state.work(bytes)?;
        let scratch_cost = |capacity: usize| capacity.checked_mul(size_of::<DomStringRef>())
            .and_then(|amount| amount.checked_add(text.capacity()))
            .filter(|amount| *amount <= available)
            .ok_or("DOM retained class-filter scratch allowance exhausted.");
        scratch_cost(count)?;
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(count).map_err(|_| "Could not reserve class-filter tokens.")?;
        let scratch = scratch_cost(tokens.capacity())?;
        tokens.extend(class_tokens(text.view()));
        let levels = (usize::BITS - count.leading_zeros()) as usize + 1;
        self.state.work(bytes.saturating_mul(levels).saturating_mul(4))?;
        tokens.sort_unstable_by(|a, b| a.code_units().cmp(b.code_units()));
        tokens.dedup_by(|a, b| a.code_units().eq(b.code_units()));

        let name_cost = |capacity: usize| capacity.checked_mul(size_of::<Vec<u16>>())
            .and_then(|amount| amount.checked_add(scratch))
            .filter(|amount| *amount <= available)
            .ok_or("DOM retained class-filter name allowance exhausted.");
        name_cost(tokens.len())?;
        let mut names = Vec::new();
        names.try_reserve_exact(tokens.len()).map_err(|_| "Could not reserve class-filter names.")?;
        let mut occupied = name_cost(names.capacity())?;
        for token in tokens {
            let units = token.code_units().count();
            let needed = units.checked_mul(size_of::<u16>()).ok_or("Class filter length overflow.")?;
            if needed > available - occupied { return Err("DOM retained class-filter text allowance exhausted.".into()); }
            self.state.copies(needed)?;
            let mut name = Vec::new();
            name.try_reserve_exact(units).map_err(|_| "Could not reserve class-filter text.")?;
            occupied = name.capacity().checked_mul(size_of::<u16>()).and_then(|size| occupied.checked_add(size))
                .filter(|amount| *amount <= available).ok_or("DOM retained class-filter text allocation exceeds allowance.")?;
            name.extend(token.code_units());
            names.push(name);
        }
        drop(text);
        self.store_live_collection(root, Filter::Classes(names))
    }

    fn store_live_collection(&mut self, root: u32, filter: Filter) -> Result<Value, String> {
        let count=self.state.dom.collections.len();
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&count>=MAX_COLLECTIONS{return Err("DOM live tag-collection limit reached (128 per document).".into());}
        if count>=(COLLECTION_END-COLLECTION_BASE) as usize{return Err("HTMLCollection native identity space exhausted.".into());}
        let payload=filter.owned_bytes();
        let available=self.state.retained_limit.saturating_sub(self.state.retained).checked_sub(payload).ok_or("DOM retained collection filter allowance exhausted.")?;
        let (replacement,added)=retained_storage::grow(&self.state.dom.collections,1,available)?;
        if let Some(mut next)=replacement {
            self.state.copies(count*size_of::<TagCollection>())?;
            next.append(&mut self.state.dom.collections);self.state.dom.collections=next;
        }
        let handle=COLLECTION_BASE+count as u32;
        self.state.dom.collections.push(TagCollection{root,filter});self.state.retained+=payload+added;
        Ok(Value::Host(handle))
    }

    fn tag_collection_nodes(&mut self, handle: u32) -> Result<Vec<usize>, String> {
        let index = collection_handle_index(handle).ok_or("Invalid HTMLCollection handle.")?;
        let collection = self
            .state
            .dom
            .collections
            .get(index)
            .ok_or("Invalid HTMLCollection handle.")?;
        let root=if collection.root==DOCUMENT{0}else{node_index(collection.root).ok_or("Native collection root is unavailable.")?};
        if matches!(collection.filter,Filter::Children(_)){return Err("Children views use stable native handles.".into());}
        if matches!(&collection.filter,Filter::Classes(names) if names.is_empty()){return Ok(Vec::new());}
        let mut candidates=self.query_candidates(root)?;
        if matches!(&self.state.dom.collections[index].filter,Filter::Tag(name,_) if name=="*"){return Ok(candidates);}
        let mut kept=0;
        for position in 0..candidates.len() {
            let node=candidates[position];self.state.work(1)?;
            match &self.state.dom.collections[index].filter {
                Filter::Children(_)=>unreachable!(),
                Filter::Namespace(query) => {
                    self.state.work(query.comparison_bytes(&self.doc.nodes[node])?.saturating_add(1))?;
                }
                Filter::Tag(_,_)=>{let NodeKind::Element{tag,expanded_name,..}=&self.doc.nodes[node].kind else{unreachable!()};self.state.copies(crate::html::element_names::qualified_work(tag,expanded_name))?;},
                Filter::Classes(names)=>{
                    let count=names.len();
                    let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else{unreachable!()};
                    self.state.work(attrs.len())?;
                    self.state.copies(attrs.iter().map(|attr|attr.name.bytes()).sum())?;
                    let scans=self.doc.dom_attr(node,"class").map_or(0, DomString::logical_bytes).checked_mul(count).ok_or("Class collection scan accounting overflow.")?;
                    self.state.work(scans)?;self.state.copies(scans)?;
                }
            }
            let matches=match &self.state.dom.collections[index].filter {
                Filter::Children(_)=>unreachable!(),
                Filter::Tag(name,html)=>tag_matches(&self.doc.nodes[node],name,*html)?,
                Filter::Namespace(query) => query.matches_node(&self.doc.nodes[node])?,
                Filter::Classes(names)=>{
                    class_filter_matches(self.doc.dom_attr(node, "class"), names, self.doc.mode == crate::html::DocumentMode::Quirks)
                }
            };
            if matches {
                if kept!=position{self.state.copies(size_of::<usize>())?;candidates[kept]=node;}
                kept+=1;
            }
        }
        candidates.truncate(kept);Ok(candidates)
    }

    fn tag_collection_named(&mut self, handle: u32, name: &str) -> Result<Value, String> {
        if let Some(owner)=self.children_collection_owner(handle){
            if name.is_empty(){return Ok(Value::Undefined);}
            for node in self.parent_element_nodes(owner)? {
                if self.collection_element_named_match(node,name)?{return Ok(Value::Host(node));}
            }
            return Ok(Value::Undefined);
        }
        if name.is_empty() {
            return Ok(Value::Undefined);
        }
        for node in self.tag_collection_handles(handle)? {
            if self.collection_element_named_match(node,name)?{return Ok(Value::Host(node));}
        }
        Ok(Value::Undefined)
    }

    pub(super) fn tag_collection_get(
        &mut self,
        handle: u32,
        property: &str,
    ) -> Result<Value, String> {
        if handle==DOCUMENT_IMAGES{return self.image_collection_get(property);}
        if self.collection_prototype_member(property)&&property!="length" {return Ok(Value::Undefined);}
        if let Some(owner)=self.children_collection_owner(handle){
            if property=="length"{return self.parent_node_get(owner,"childElementCount");}
            if let Some(index)=collection_index(property){return Ok(self.parent_element_nodes(owner)?.get(index).copied().map_or(Value::Undefined,Value::Host));}
            return self.tag_collection_named(handle,property);
        }
        if property == "length" {
            return Ok(Value::Number(
                self.tag_collection_handles(handle)?.len() as f64
            ));
        }
        if let Some(index) = collection_index(property) {
            return Ok(self
                .tag_collection_handles(handle)?
                .get(index)
                .map_or(Value::Undefined, |&node| Value::Host(node)));
        }
        self.tag_collection_named(handle, property)
    }

    pub(super) fn tag_collection_call(
        &mut self,
        handle: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if handle==DOCUMENT_IMAGES{return self.call(handle,method,args);}
        let argument = args
            .first()
            .ok_or("HTMLCollection methods require one argument.")?;
        let value = match method {
            "item" => {
                let index = timer_integer(argument)? as u32 as usize;
                if let Some(owner)=self.children_collection_owner(handle){self.parent_element_nodes(owner)?.get(index).copied().map_or(Value::Undefined,Value::Host)}
                else {self.tag_collection_handles(handle)?.get(index).map_or(Value::Undefined, |&node| Value::Host(node))}
            }
            "namedItem" => {
                let name = self.text(argument)?;
                self.tag_collection_named(handle, &name)?
            }
            _ => return Err("HTMLCollection method is not implemented.".into()),
        };
        Ok(if matches!(value, Value::Undefined) {
            Value::Null
        } else {
            value
        })
    }

    pub(super) fn tag_collection_keys(&mut self, handle: u32) -> Result<Vec<String>, String> {
        let length = if let Some(owner)=self.children_collection_owner(handle){self.parent_element_nodes(owner)?.len()}else{self.tag_collection_handles(handle)?.len()};
        #[cfg(test)]
        if self.state.limits.test_entry_arenas&&length>4096{return Err("HTMLCollection enumeration limit reached (4096 indices).".into());}
        self.state.index_keys(length)
    }

    pub(super) fn tag_collection_own_data(
        &mut self,
        handle: u32,
        property: &str,
    ) -> Result<Option<Value>, String> {
        if collection_index(property).is_some() {
            return self
                .tag_collection_get(handle, property)
                .map(|value| (!matches!(value, Value::Undefined)).then_some(value));
        }
        if matches!(property, "length" | "item" | "namedItem") {
            return Ok(None);
        }
        if !matches!(
            self.tag_collection_named(handle, property)?,
            Value::Undefined
        ) {
            // Named properties are nonenumerable in HTMLCollection. The host
            // descriptor API currently represents indexed data only; never
            // return a false enumerable descriptor for a named member.
            return Err("Named HTMLCollection property descriptors are not implemented.".into());
        }
        Ok(None)
    }
}

fn class_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

impl State {
    // No script runs while keys are staged. Admit the complete vector and each
    // actual string capacity against the current shared host allowance.
    pub(super) fn index_keys(&mut self,length:usize)->Result<Vec<String>,String> {
        use std::fmt::Write;
        self.work(length)?;self.copies(length.saturating_mul(20))?;
        let available=self.retained_limit.saturating_sub(self.retained);
        let minimum=length.checked_mul(size_of::<String>()).filter(|bytes|*bytes<=available).ok_or("DOM retained collection key allowance exhausted.")?;
        let mut keys=Vec::new();keys.try_reserve_exact(length).map_err(|_|"Could not reserve collection keys.")?;
        let mut bytes=keys.capacity().checked_mul(size_of::<String>()).filter(|bytes|*bytes<=available).ok_or("DOM retained collection key allocation exceeds allowance.")?;
        debug_assert!(bytes>=minimum);
        for index in 0..length {
            let digits=if index==0{1}else{index.ilog10() as usize+1};
            if digits>available.saturating_sub(bytes){return Err("DOM retained collection key text allowance exhausted.".into());}
            let mut text=String::new();text.try_reserve_exact(digits).map_err(|_|"Could not reserve collection index text.")?;
            bytes=bytes.checked_add(text.capacity()).filter(|bytes|*bytes<=available).ok_or("DOM retained collection index allocation exceeds allowance.")?;
            write!(&mut text,"{index}").map_err(|_|"Could not format collection index.")?;keys.push(text);
        }
        Ok(keys)
    }
}

#[cfg(test)]
#[path = "script_live_collection_storage_tests.rs"]
mod storage_tests;

impl Bridge<'_> {
    fn collection_element_named_match(&mut self,object:u32,name:&str)->Result<bool,String>{
        let node=if let Some((owner,node))=self.frame_contains_node(object){self.contains_child_document(owner)?.nodes.get(node)}
            else if self.inert_node_type(object).is_some(){self.inert_collection_element_source(object)}
            else {node_index(object).and_then(|index|self.doc.nodes.get(index))};
        let (work,found)=if let Some(Node{namespace,kind:NodeKind::Element{attrs,..},..})=node {
            let html=*namespace==crate::html::Namespace::Html;
            let work=attrs.iter().fold(attrs.len(),|sum,attr|sum.saturating_add(attr.name.qualified.len()).saturating_add(attr.value.len()));
            (work,attrs.iter().any(|attr|(attr.name.html_is("id")||html&&attr.name.html_is("name"))&&attr.value==name))
        }else{(1,false)};
        self.state.work(work)?;Ok(found)
    }
    pub(super) fn collection_receiver(&self,handle:u32)->bool {handle==DOCUMENT_IMAGES||collection_handle_index(handle).is_some_and(|index|index<self.state.dom.collections.len())}
    pub(super) fn collection_prototype_member(&self,property:&str)->bool {matches!(property,"constructor"|"length"|"item"|"namedItem")}
    fn children_collection_owner(&self,handle:u32)->Option<u32>{
        let record=self.state.dom.collections.get(collection_handle_index(handle)?)?;
        if let Filter::Children(owner)=record.filter {Some(owner)}else{None}
    }
    pub(super) fn collection_owner_window(&self,handle:u32)->Option<u32>{
        if !self.collection_receiver(handle){return None;}
        let root = self.children_collection_owner(handle).or_else(||
            self.state.dom.collections.get(collection_handle_index(handle)?).map(|record| record.root));
        Some(root.and_then(|owner| self.frame_instance_window(owner)).unwrap_or(WINDOW))
    }
    pub(super) fn create_children_collection(&mut self,owner:u32)->Result<Value,String>{
        // SameObject lookup needs no second owner registry or promotion update.
        // Pay the existing registry scan before either returning or publishing.
        self.state.work(self.state.dom.collections.len().saturating_add(1))?;
        if let Some(index)=self.state.dom.collections.iter().position(|record|matches!(record.filter,Filter::Children(parent) if parent==owner)){
            return Ok(Value::Host(COLLECTION_BASE+index as u32));
        }
        self.store_live_collection(0,Filter::Children(owner))
    }
}
