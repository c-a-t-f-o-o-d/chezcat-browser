//! Bounded DOM creation, tree mutation and named form lookup. Node identities
//! remain in the document arena until realm teardown. Loaded resources and the
//! native form model are protected from unsupported structural mutations.
//! Semantics: https://dom.spec.whatwg.org/#concept-node-pre-insert
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-nameditem
use super::*;

pub(super) fn element_node_name(doc: &Document, node: usize) -> String {
    let local = tag(doc, node);
    if doc.nodes[node].namespace == crate::html::Namespace::Svg {
        local.to_owned()
    } else {
        local.to_ascii_uppercase()
    }
}

pub(super) fn node_mutation_method(name: &str) -> bool {
    crate::javascript::NODE_MUTATION_METHODS.iter().any(|&(method, _)| method == name)
}

pub(super) fn native_node_member(name:&str)->bool {
    node_constant(name).is_some() || "constructor __proto__ nodeType nodeName baseURI isConnected ownerDocument parentNode parentElement childNodes firstChild lastChild previousSibling nextSibling nodeValue textContent getRootNode hasChildNodes normalize cloneNode isEqualNode isSameNode compareDocumentPosition contains lookupPrefix lookupNamespaceURI isDefaultNamespace insertBefore appendChild replaceChild removeChild addEventListener removeEventListener dispatchEvent".split_ascii_whitespace().any(|candidate|candidate==name)
}

// Finite real main-Document Node facts, not a new Document interface family.
pub(super) fn document_node_property(name:&str)->bool {
    matches!(name,"nodeType"|"nodeName"|"isConnected"|"ownerDocument"|"parentNode"|"parentElement"|"firstChild"|"lastChild"|"previousSibling"|"nextSibling")
}

pub(super) const MAX_DOM_DEPTH: usize = 128;

#[derive(Default)]
pub(super) struct DomState {
    pub(super) created: node_sets::NodeSet,
    pub(super) connected: Vec<bool>,
    pub(super) pending_focus: Option<usize>,
    pub(super) collections: Vec<dom_collections::TagCollection>,
    pub(super) child_lists: identity_map::IdentityMap<usize>,
    pub(super) inert_child_lists: identity_map::IdentityMap<u32>,
    pub(super) fragment_scripts: node_sets::NodeSet,
    pub(super) force_async: node_sets::NodeSet,
}

pub(super) fn inert_dom_tag(name: &str, scripting: bool) -> bool {
    matches!(name, "script" | "style" | "template" | "iframe" | "object")
        || name == "noscript" && scripting
}

// Keep the host element visible to queries while excluding its inert children.
pub(super) fn exposes_inert_host(name: &str, namespace: crate::html::Namespace) -> bool {
    name == "script" || matches!(name, "iframe" | "template" | "style") && namespace == crate::html::Namespace::Html
}

fn common_node_property(name: &str) -> bool {
    matches!(
        name,
        "nodeName"
            | "nodeType"
            | "parentNode"
            | "parentElement"
            | "ownerDocument"
            | "isConnected"
            | "firstChild"
            | "lastChild"
            | "nextSibling"
            | "previousSibling"
    )
}

fn script_attribute(property: &str) -> Option<&'static str> {
    Some(match property {
        "src" => "src",
        "type" => "type",
        "charset" => "charset",
        "nonce" => "nonce",
        "integrity" => "integrity",
        "crossOrigin" => "crossorigin",
        "referrerPolicy" => "referrerpolicy",
        "fetchPriority" => "fetchpriority",
        "async" => "async",
        "defer" => "defer",
        "noModule" => "nomodule",
        _ => return None,
    })
}

fn boolean_script_attribute(name: &str) -> bool {
    matches!(name, "async" | "defer" | "nomodule")
}

impl State {
    fn id_storage_available(&self, states:usize) -> Result<usize, String> {
        self.tree_replacement_available()?.checked_sub(states)
            .ok_or("DOM retained ID index replacement allowance exhausted.".into())
    }

    fn push_document_id(&mut self, ids: &mut Vec<(usize, usize)>, node: usize, order: usize, states:usize) -> Result<(), String> {
        let old = ids.capacity().checked_mul(size_of::<(usize, usize)>()).ok_or("DOM ID index backing overflow.")?;
        let available = self.id_storage_available(states)?.checked_sub(old).ok_or("DOM retained ID index allowance exhausted.")?;
        let (growth, _) = retained_storage::grow(ids, 1, available)?;
        self.work(1)?;
        let records = 1usize.checked_add(if growth.is_some(){ids.len()}else{0}).ok_or("DOM ID index movement overflow.")?;
        self.copies(records.checked_mul(size_of::<(usize, usize)>()).ok_or("DOM ID index copy overflow.")?)?;
        if let Some(mut next) = growth {next.append(ids);*ids=next;}
        ids.push((node,order));
        Ok(())
    }

    fn index_id_text<'a>(&mut self, doc: &'a Document, node: usize) -> Result<Option<&'a str>,String> {
        let fields=match &doc.nodes.get(node).ok_or("DOM ID index owner disappeared.")?.kind {
            NodeKind::Element{attrs,..}=>attrs.len(), _=>0,
        };
        self.work(fields.saturating_add(1))?;
        Ok(doc.attr(node,"id"))
    }

    fn compare_document_ids(&mut self, doc: &Document, a: (usize,usize), b: (usize,usize)) -> Result<std::cmp::Ordering,String> {
        let left=self.index_id_text(doc,a.0)?.ok_or("DOM ID index owner lost its ID.")?;
        let right=self.index_id_text(doc,b.0)?.ok_or("DOM ID index owner lost its ID.")?;
        self.work(left.len().min(right.len()).saturating_add(1))?;
        Ok(left.cmp(right).then(a.1.cmp(&b.1)))
    }

    fn sift_document_ids(&mut self, doc: &Document, ids: &mut [(usize,usize)], mut root: usize) -> Result<(),String> {
        while let Some(left)=root.checked_mul(2).and_then(|value|value.checked_add(1)).filter(|&child|child<ids.len()) {
            let mut child=left;
            if left+1<ids.len() && self.compare_document_ids(doc,ids[left],ids[left+1])?.is_lt() {child+=1;}
            if !self.compare_document_ids(doc,ids[root],ids[child])?.is_lt() {break;}
            self.copies(3*size_of::<(usize,usize)>())?;
            ids.swap(root,child);root=child;
        }
        Ok(())
    }

    fn sort_document_ids(&mut self, doc: &Document, ids: &mut Vec<(usize,usize)>) -> Result<(),String> {
        // Fallible in-place heap sort: every comparison and move is admitted,
        // and failure only changes this unpublished replacement index.
        for root in (0..ids.len()/2).rev() {self.sift_document_ids(doc,ids,root)?;}
        for end in (1..ids.len()).rev() {
            self.copies(3*size_of::<(usize,usize)>())?;
            ids.swap(0,end);self.sift_document_ids(doc,&mut ids[..end],0)?;
        }
        let mut kept=0;
        for read in 0..ids.len() {
            self.work(1)?;
            if kept!=0 {
                let a=self.index_id_text(doc,ids[kept-1].0)?.ok_or("DOM ID index owner lost its ID.")?;
                let b=self.index_id_text(doc,ids[read].0)?.ok_or("DOM ID index owner lost its ID.")?;
                self.work(a.len().min(b.len()).saturating_add(1))?;
                if a==b {continue;}
            }
            if kept!=read {self.copies(size_of::<(usize,usize)>())?;ids[kept]=ids[read];}
            kept+=1;
        }
        ids.truncate(kept);
        Ok(())
    }

    pub(super) fn document_id(&mut self, doc: &Document, id: &str) -> Result<Option<usize>,String> {
        let(mut low,mut high)=(0,self.tree.ids.len());
        while low<high {
            let middle=low+(high-low)/2;
            let node=self.tree.ids[middle].0;
            let candidate=self.index_id_text(doc,node)?.ok_or("DOM ID index owner lost its ID.")?;
            self.work(candidate.len().min(id.len()).saturating_add(1))?;
            match candidate.cmp(id) {
                std::cmp::Ordering::Less=>low=middle+1,
                std::cmp::Ordering::Greater=>high=middle,
                std::cmp::Ordering::Equal=>return Ok(Some(node)),
            }
        }
        Ok(None)
    }

    pub(super) fn rebuild_dom_tree(&mut self, doc: &Document) -> Result<(), String> {
        self.ensure_node_sets(doc.nodes.len())?;
        if !self.dirty {
            return Ok(());
        }
        let retained_on_entry=self.retained;
        let mut stack=std::mem::take(&mut self.tree_stack);
        stack.clear();
        let result=self.rebuild_dom_tree_with_stack(doc,&mut stack);
        stack.clear();
        if result.is_err() && self.retained>retained_on_entry && self.retained>self.retained_limit {
            // No author callback runs during a rebuild. Speculative replacement
            // credit cannot survive a failed publication as an overdrawn cache.
            let bytes=stack.capacity()*size_of::<tree_order::Visit>();
            let Some(retained)=self.retained.checked_sub(bytes) else {
                self.tree_stack=stack;
                return Err("DOM traversal retained accounting underflow.".into());
            };
            self.retained=retained;stack=Vec::new();
        }
        self.tree_stack=stack;
        result
    }

    fn rebuild_dom_tree_with_stack(&mut self,doc:&Document,stack:&mut Vec<tree_order::Visit>)->Result<(),String> {
        // Active lookup deliberately excludes inert subtrees; physical parent
        // links and connection state must still describe created script nodes.
        self.work(doc.nodes.len())?;
        let states=self.prepare_rebuilt_states(doc.nodes.len())?;
        let state_bytes=states.bytes();
        let mut tree = Tree {
            active: states.active,
            parents: states.parents,
            sibling_slots: states.siblings,
            ..Tree::default()
        };
        let mut connected = states.connected;
        if !doc.nodes.is_empty() {
            // Leave one current root order slot before optional scratch growth.
            self.reserve_tree_stack(stack,1,state_bytes.checked_add(size_of::<usize>()).ok_or("DOM root traversal backing overflow.")?)?;
            stack.push((0,None,false,usize::MAX));
        }
        while let Some((node, parent, inert, slot)) = stack.pop() {
            self.work(1)?;
            if node >= doc.nodes.len() || connected[node] {
                return Err("DOM tree has an invalid child or repeated parent.".into());
            }
            connected[node] = true;
            tree.parents[node] = parent;
            tree.sibling_slots[node] = slot;
            let name = tag(doc, node);
            let inert_children = inert_dom_tag(name, doc.scripting_enabled);
            // The embedding element belongs to this document. Indexing it
            // leaves its fallback subtree inert and never creates a child window.
            if !inert && (!inert_children || exposes_inert_host(name, doc.nodes[node].namespace)) {
                tree.active[node] = true;
                let has_id=self.index_id_text(doc,node)?.is_some_and(|id| !id.is_empty() && id.len() <= self.limits.host.string_bytes);
                let id_min=if has_id {(tree.ids.len()+1).saturating_sub(tree.ids.capacity())*size_of::<(usize,usize)>()}else{0};
                let other=state_bytes.checked_add(tree.ids.capacity()*size_of::<(usize,usize)>()).and_then(|bytes|bytes.checked_add(id_min))
                    .ok_or("DOM order/ID joint backing overflow.")?;
                self.push_tree_order(&mut tree.order,node,other)?;
                if name == "body" && tree.body.is_none() {
                    tree.body = Some(node);
                }
                if name == "head" && tree.head.is_none() {
                    tree.head = Some(node);
                }
                if parent == Some(0)
                    && tree.document_element.is_none()
                    && matches!(doc.nodes[node].kind, NodeKind::Element { .. })
                {
                    tree.document_element = Some(node);
                }
                if has_id {
                    // The index reads document-owned text; it retains no ID copy.
                    let other=state_bytes.checked_add(tree.order.capacity()*size_of::<usize>()).ok_or("DOM order/ID backing overflow.")?;
                    self.push_document_id(&mut tree.ids, node, tree.order.len() - 1, other)?;
                }
            }
            let other=state_bytes.checked_add(tree.order.capacity()*size_of::<usize>())
                .and_then(|bytes|bytes.checked_add(tree.ids.capacity()*size_of::<(usize,usize)>())).ok_or("DOM traversal metadata overflow.")?;
            self.reserve_tree_stack(stack,doc.nodes[node].children.len(),other)?;
            stack.extend(
                doc.nodes[node]
                    .children
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(slot, &child)| (child, Some(node), inert || inert_children, slot)),
            );
        }
        // Saved handles and static NodeLists retain detached ordinary subtrees,
        // whose internal topology is separate from connected document lookup.
        self.work(self.detached_nodes.scan_work())?;
        self.work(
            self.detached_nodes
                .iter()
                .map(|node| doc.nodes[node].children.len())
                .sum(),
        )?;
        self.work(self.detached_nodes.scan_work())?;
        for parent in self.detached_nodes.iter() {
            for (slot, &child) in doc.nodes[parent].children.iter().enumerate() {
                if self.detached_nodes.contains(&child) {
                    tree.parents[child] = Some(parent);
                    tree.sibling_slots[child] = slot;
                }
            }
        }
        self.sort_document_ids(doc, &mut tree.ids)?;
        let old_ids = self.tree.ids.capacity() * size_of::<(usize, usize)>();
        let new_ids = tree.ids.capacity() * size_of::<(usize, usize)>();
        self.retained = self.retained.checked_sub(old_ids).and_then(|bytes|bytes.checked_sub(self.node_state_bytes()))
            .and_then(|bytes|bytes.checked_sub(self.order_bytes()))
            .and_then(|bytes|bytes.checked_add(new_ids)).and_then(|bytes|bytes.checked_add(state_bytes))
            .and_then(|bytes|bytes.checked_add(tree.order.capacity()*size_of::<usize>()))
            .ok_or("DOM ID index retained accounting overflow.")?;
        self.tree = tree;
        self.dom.connected = connected;
        self.dirty = false;
        Ok(())
    }
}

impl Page {
    pub fn take_focus_request(&mut self) -> Option<usize> {
        self.state.dom.pending_focus.take()
    }
}

impl Bridge<'_> {
    // Document parsing can retain markup-shaped iframe fallback instead of
    // the single raw Text node required by HTML. Those arena entries have no
    // public child identity. Empty iframes have a known, ordinary child list.
    pub(super) fn check_iframe_child_topology(&self, node: usize) -> Result<(), String> {
        let item = self.doc.nodes.get(node).ok_or("Invalid child topology owner.")?;
        if !item.children.is_empty() && item.namespace == crate::html::Namespace::Html
            && matches!(&item.kind, NodeKind::Element { tag, .. } if tag == "iframe")
        {
            return Err("Iframe fallback child topology requires raw-text document parsing.".into());
        }
        Ok(())
    }

    pub(super) fn comment_receiver(&self, object:u32)->bool {
        node_index(object).is_some_and(|node|self.dom_accessible(node)
            && matches!(self.doc.nodes[node].kind,NodeKind::Comment(_)))
    }

    pub(super) fn processing_instruction_receiver(&self, object: u32) -> bool {
        self.inert_node_type(object) == Some(7)
            || node_index(object).is_some_and(|node| self.dom_accessible(node)
                && matches!(self.doc.nodes[node].kind, NodeKind::ProcessingInstruction { .. }))
    }

    pub(super) fn processing_instruction_target(&mut self, object: u32) -> Result<Value, String> {
        if self.inert_node_type(object) == Some(7) { return self.inert_get(object, "target"); }
        let node = self.dom_node(object)?;
        let NodeKind::ProcessingInstruction { target, .. } = &self.doc.nodes[node].kind else {
            return Err("ProcessingInstruction target requires a genuine node.".into());
        };
        self.state.copies(target.len())?;
        Ok(Value::String(target.clone()))
    }

    pub(super) fn create_processing_instruction(&mut self, owner: u32, args: &[Value]) -> Result<Value, String> {
        let target = self.text(args.first().ok_or("ProcessingInstruction requires a target.")?)?;
        let data = match args.get(1) {
            None | Some(Value::Undefined) => String::new(),
            Some(data) => self.text(data)?,
        };
        if self.inert_document(owner) {
            return self.create_inert_processing_instruction(owner, target, data);
        }
        self.create_dom_node(NodeKind::ProcessingInstruction { target, data })
    }

    pub(super) fn node_identity_receiver(&self,object:u32)->bool {
        !self.is_frame_handle(object) && (self.namespace_receiver(object) || self.inert_node_type(object).is_some())
    }

    pub(super) fn text_content_receiver(&self, object: u32) -> bool {
        self.node_identity_receiver(object) || self.frame_contains_node(object).is_some()
    }

    pub(super) fn document_node_get(&mut self,property:&str)->Result<Option<Value>,String> {
        if !document_node_property(property) {return Ok(None);}
        Ok(Some(match property {
            "nodeType"=>Value::Number(9.0),
            "nodeName"=>return self.string("#document").map(Some),
            "isConnected"=>Value::Bool(true),
            "firstChild"|"lastChild"=>{
                self.state.rebuild(self.doc)?;
                let root=self.doc.nodes.first().ok_or("Main Document root is unavailable.")?;
                let child=if property=="firstChild" {root.children.first()}else{root.children.last()};
                child.map_or(Value::Null,|&node|Value::Host(node_handle(node)))
            }
            _=>Value::Null,
        }))
    }

    pub(super) fn attr_element_receiver(&self, object: u32) -> bool {
        node_index(object).is_some_and(|node| self.dom_accessible(node)
            && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }))
    }

    fn element_naming_error(&self, node: usize) -> String {
        let parent = self.state.tree.parents[node];
        let grandparent = parent.and_then(|index| self.state.tree.parents[index]);
        let name = |index: Option<usize>| index.filter(|&index| index != 0)
            .map_or("none", |index| prefix(tag(self.doc, index), 48));
        format!(
            "Element naming requires known namespace provenance. Receiver tag={}, namespace=unknown parser context, owner=main document, parent={}, grandparent={}.",
            name(Some(node)), name(parent), name(grandparent)
        )
    }

    fn html_tab_index(&mut self, node: usize) -> Result<i32, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { return Err("tabIndex requires an HTML Element.".into()); };
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html {
            return Err("tabIndex for unsupported foreign parser contexts is unavailable.".into());
        }
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|attr| attr.name.bytes() + if attr.name.html_is("tabindex") { attr.value.len() } else { 0 }).sum())?;
        if let Some(value) = self.doc.attr(node, "tabindex").and_then(forms::parse_tab_index) { return Ok(value); }
        if matches!(tag(self.doc, node), "a" | "area" | "button" | "frame" | "iframe" | "input" | "object" | "select" | "textarea") { return Ok(0); }
        if tag(self.doc, node) == "summary" {
            if let Some(parent) = self.state.tree.parents[node].filter(|&parent| tag(self.doc, parent) == "details") {
                if self.doc.nodes[parent].namespace != crate::html::Namespace::Html {
                    return Err("tabIndex summary default has an unknown parent namespace.".into());
                }
                for &child in &self.doc.nodes[parent].children {
                    self.state.work(1)?;
                    if tag(self.doc, child) == "summary" {
                        if self.doc.nodes[child].namespace != crate::html::Namespace::Html {
                            return Err("tabIndex summary default has an unknown sibling namespace.".into());
                        }
                        return Ok(if child == node { 0 } else { -1 });
                    }
                }
            }
        }
        Ok(-1)
    }

    #[cold]
    #[inline(never)]
    pub(super) fn unavailable_offset(&self, node: usize, property: &str) -> String {
        let class = match tag(self.doc, node) {
            "html" => "html", "head" => "head", "div" => "div", "span" => "span",
            "input" => "input", "textarea" => "textarea", "button" => "button", "a" => "anchor",
            _ => "other",
        };
        format!("Element property {property:?} requires measured offset geometry. Receiver node={node}; class={class}; connected={}; created={}; geometry-attached={}. No coordinate was fabricated.",
            self.state.dom.connected[node], self.state.dom.created.contains(&node), self.state.geometry.is_some())
    }

    #[cold]
    #[inline(never)]
    fn unavailable_scroll(&self, node:usize, property:&str) -> String {
        format!("Element property {property:?} requires measured scrolling-area geometry. Receiver node={node}; tag={}; connected={}; created={}; geometry-attached={}. No extent was fabricated.",
            tag(self.doc,node),self.state.dom.connected[node],self.state.dom.created.contains(&node),self.state.geometry.is_some())
    }

    pub(super) fn namespace_receiver(&self, object: u32) -> bool {
        object == DOCUMENT
            || self.attr_receiver(object)
            || node_index(object).is_some_and(|node| self.dom_accessible(node))
            || self.frame_node_constant_readonly(object)
    }

    pub(super) fn is_default_namespace(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let query = match args.first() {
            Some(Value::Null | Value::Undefined) => None,
            Some(Value::String(text)) => if text.is_empty() { None } else { Some(text.as_str()) },
            _ => return Err("isDefaultNamespace requires a converted nullable string.".into()),
        };
        if let Some(result) = self.attr_default_namespace(object) {
            return result.map(|namespace| Value::Bool(query == namespace));
        }
        if let Some(result) = self.frame_default_namespace(object) {
            return result.map(|namespace| Value::Bool(query == namespace));
        }
        let node = if object == DOCUMENT {
            self.state.rebuild(self.doc)?;
            0
        } else { self.dom_node(object)? };
        let parent = self.state.tree.parents.get(node).copied().flatten();
        default_namespace(self.doc, node, parent, self.state)
            .map(|namespace| Value::Bool(query == namespace))
    }

    pub(super) fn dom_accessible(&self, node: usize) -> bool {
        if node == 0 || node >= self.doc.nodes.len() || self.inert_adopted_node(node_handle(node)) {
            return false;
        }
        if self.state.tree.active.get(node).copied().unwrap_or(false)
                || self.state.detached_nodes.contains(&node)
                || self.state.detached_images.contains(&node)
                || self.state.dom.created.contains(&node) { return true; }
        // A raw-text child retains its Text identity when the sheet or script
        // connects. Its containing resource still owns all content mutations.
        if matches!(self.doc.nodes[node].kind, NodeKind::Text(_))
            && self.state.dom.connected.get(node)==Some(&true)
            && self.state.tree.parents.get(node).copied().flatten().is_some_and(|parent|
                tag(self.doc,parent)=="script" || self.created_html_style(parent)) {
            return true;
        }
        if !self.state.dirty || self.state.dom.connected.get(node) != Some(&true) { return false; }
        // Mutation commits current parents/connection state before rebuilding
        // lookup caches. A reinserted parsed subtree remains a genuine Node in
        // that interval; prove its actual non-inert ancestry without changing
        // state or accepting an arbitrary in-range handle.
        let mut current = node;
        for _ in 0..self.doc.nodes.len() {
            if current == 0 { return true; }
            let name = tag(self.doc, current);
            if inert_dom_tag(name, self.doc.scripting_enabled)
                && (current != node || !exposes_inert_host(name, self.doc.nodes[current].namespace)) {
                return false;
            }
            let Some(parent) = self.state.tree.parents.get(current).copied().flatten() else { return false; };
            let Some(parent_node) = self.doc.nodes.get(parent) else { return false; };
            if !parent_node.children.contains(&current) { return false; }
            current = parent;
        }
        false
    }

    pub(super) fn dom_node(&mut self, handle: u32) -> Result<usize, String> {
        self.state.rebuild(self.doc)?;
        node_index(handle)
            .filter(|&node| self.dom_accessible(node))
            .ok_or_else(|| "DOM node is inert or unavailable in this subset.".into())
    }

    pub(super) fn dom_stylesheet(&self, node: usize) -> bool {
        self.doc.nodes.get(node).is_some_and(|entry|
            entry.namespace == crate::html::Namespace::Html
                && matches!(&entry.kind, NodeKind::Element { tag, .. } if tag == "link"))
    }

    pub(super) fn movable_template_host(&self,node:usize)->bool{self.state.dom.created.contains(&node)&&self.doc.nodes.get(node).is_some_and(|entry|entry.namespace==crate::html::Namespace::Html&&entry.children.is_empty()&&matches!(&entry.kind,NodeKind::Element{tag,..} if tag=="template"))}

    pub(super) fn created_html_style(&self, node: usize) -> bool {
        self.state.dom.created.contains(&node) && self.doc.nodes.get(node).is_some_and(|entry|
            entry.namespace == crate::html::Namespace::Html
                && matches!(&entry.kind, NodeKind::Element { tag, .. } if tag == "style"))
    }

    pub(super) fn style_text_attribute(&self, object: u32, property: &str) -> bool {
        matches!(property, "type" | "media") && node_index(object).is_some_and(|node|self.created_html_style(node))
    }

    pub(super) fn dom_script(&self, node: usize) -> bool {
        tag(self.doc, node) == "script" && self.state.dom.created.contains(&node)
    }

    pub(super) fn dom_method(&self, object: u32, property: &str) -> Option<&'static str> {
        if self.fragment_receiver(object) {return match property {
            "appendChild"=>Some("appendChild"),"insertBefore"=>Some("insertBefore"),
            "replaceChild"=>Some("replaceChild"),"removeChild"=>Some("removeChild"),_=>None,
        };}
        if object==DOCUMENT {match property {"createAttribute"=>return Some("createAttribute"),"createAttributeNS"=>return Some("createAttributeNS"),_=>{}}}
        if self.namespace_receiver(object) {
            match property {
                "isDefaultNamespace" => return Some("isDefaultNamespace"),
                "compareDocumentPosition" => return Some("compareDocumentPosition"),
                _ => {}
            }
        }
        let node = node_index(object).filter(|&node| node < self.doc.nodes.len())?;
        match property {
            "getBoundingClientRect" if self.class_list_element_receiver(object) => Some("getBoundingClientRect"),
            "getAttributeNode" if self.attr_element_receiver(object) => Some("getAttributeNode"),
            "hasAttributes" if self.attr_element_receiver(object) => Some("hasAttributes"),
            "getAttributeNodeNS" if self.attr_element_receiver(object) => Some("getAttributeNodeNS"),
            "getAttributeNS" if self.attr_element_receiver(object) => Some("getAttributeNS"),
            "setAttributeNS" if self.attr_element_receiver(object) => Some("setAttributeNS"),
            "removeAttributeNS" if self.attr_element_receiver(object) => Some("removeAttributeNS"),
            "removeAttributeNode" if self.attr_element_receiver(object) => Some("removeAttributeNode"),
            "setAttributeNode" if self.attr_element_receiver(object) => Some("setAttributeNode"),
            "setAttributeNodeNS" if self.attr_element_receiver(object) => Some("setAttributeNodeNS"),
            "appendChild" => Some("appendChild"),
            "insertBefore" => Some("insertBefore"),
            "replaceChild" => Some("replaceChild"),
            "removeChild" => Some("removeChild"),
            "focus" if matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) => {
                Some("focus")
            }
            _ => None,
        }
    }

    pub(super) fn dom_readonly(&self, object: u32, property: &str) -> bool {
        if object==DOCUMENT && document_node_property(property) {return true;}
        if property=="length" && self.comment_receiver(object) {return true;}
        if property=="childNodes" {return object==DOCUMENT || node_index(object).is_some_and(|node|self.dom_accessible(node));}
        if property == "attributes" { return self.class_list_element_receiver(object); }
        if property == "origin" && node_index(object).is_some_and(|node|
            self.dom_accessible(node) && self.anchor_url_property(node, property)) { return true; }
        if matches!(property,"localName"|"prefix"|"namespaceURI") {
            return self.attr_element_receiver(object) && node_index(object).is_some_and(|node|
                self.doc.nodes[node].namespace.element_uri().is_some());
        }
        if matches!(property, "offsetLeft" | "offsetTop" | "offsetWidth" | "offsetHeight" | "scrollWidth" | "scrollHeight") {
            return node_index(object).is_some_and(|node| self.dom_accessible(node)
                && self.doc.nodes[node].namespace == crate::html::Namespace::Html
                && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }));
        }
        if node_constant(property).is_some() {
            return object == DOCUMENT
                || node_index(object).is_some_and(|node| self.dom_accessible(node))
                || self.frame_node_constant_readonly(object);
        }
        node_index(object).is_some() && common_node_property(property)
    }

    pub(super) fn dom_string_setter(&self, object: u32, property: &str) -> bool {
        if property == "innerHTML" { return self.attr_element_receiver(object); }
        let Some(node) = node_index(object).filter(|&node| node < self.doc.nodes.len()) else {
            return false;
        };
        if self.html_text_reflection(node,property).is_some() {return true;}
        if property != "origin" && self.anchor_url_property(node, property) { return true; }
        self.dom_script(node)
            && script_attribute(property).is_some_and(|name| !boolean_script_attribute(name))
            || self.dom_stylesheet(node)
                && script_stylesheets::link_attribute(property)
                    .is_some_and(|name| name != "disabled")
            || matches!(self.doc.nodes[node].kind, NodeKind::Text(_)) && property == "data"
            || self.comment_receiver(object) && matches!(property,"data"|"nodeValue"|"textContent")
    }

    pub(super) fn dom_validate_call(
        &self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if object==DOCUMENT && method=="createAttribute" {
            if let Some(Value::String(name))=args.first() {if !super::script_attribute_names::valid_attribute_name(name) {return Err(("InvalidCharacterError","Invalid attribute local name.".into()));}}
        }
        if object==DOCUMENT && method=="createAttributeNS" {
            if let (Some(namespace),Some(Value::String(name)))=(args.first(),args.get(1)) {
                let namespace=match namespace{Value::String(value) if !value.is_empty()=>Some(value.as_str()),_=>None};
                super::script_attribute_names::validate_namespace(namespace,name)?;
            }
        }
        if (object == DOCUMENT || self.inert_document(object)) && method=="createElementNS" {
            if let (Some(namespace),Some(Value::String(name)))=(args.first(),args.get(1)) {
                let namespace=match namespace{Value::String(value) if !value.is_empty()=>Some(value.as_str()),_=>None};
                super::script_attribute_names::validate_element_namespace(namespace,name)?;
            }
        }
        if matches!(method,"setAttributeNode"|"setAttributeNodeNS") && node_index(object).is_some() {return self.validate_attr_attachment(object,args);}
        if method == "replaceChild" && node_index(object).is_some() {
            return self.validate_replace_child(object, args);
        }
        if method=="setAttribute" && node_index(object).is_some() {
            if let Some(Value::String(name))=args.first() {
                if !super::script_attribute_names::valid_attribute_name(name) {
                    return Err(("InvalidCharacterError","Invalid attribute local name.".into()));
                }
            }
        }
        if method == "setAttributeNS" && self.attr_element_receiver(object) {
            if let (Some(namespace), Some(Value::String(name))) = (args.first(), args.get(1)) {
                let ns = match namespace { Value::String(text) if !text.is_empty() => Some(text.as_str()), _ => None };
                super::script_attribute_names::validate_namespace(ns, name)?;
            }
        }
        if method == "compareDocumentPosition" {
            if !matches!(args.first(), Some(Value::Host(other)) if self.namespace_receiver(*other)) {
                return Err(("TypeError", "compareDocumentPosition requires a Node argument.".into()));
            }
        }
        if method == "removeAttributeNode" && node_index(object).is_some() {
            return self.validate_attr_removal(object, args);
        }
        if (object == DOCUMENT || self.inert_document(object)) && method == "createElement" {
            if let Some(Value::String(name)) = args.first() {
                // This HTML subset accepts bounded ASCII element names. Other
                // namespaces and customized built-in element options are absent.
                if name.is_empty()
                    || name.len() > 128
                    || !name.as_bytes()[0].is_ascii_alphabetic()
                    || !name.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
                    })
                {
                    return Err((
                        "InvalidCharacterError",
                        "Element name is outside the supported ASCII name syntax (128 bytes)."
                            .into(),
                    ));
                }
            }
        }
        if node_index(object).is_some()
            && matches!(method, "appendChild" | "insertBefore" | "removeChild")
        {
            if let (true, Some(Value::Host(handle))) = (method != "removeChild", args.first()) {
                if let Some(template) = self.checked_inert_fragment(*handle) {
                    return self.validate_inert_fragment_insertion(object, template, method, args);
                }
            }
            if !matches!(args.first(), Some(Value::Host(handle)) if node_index(*handle).is_some()) {
                return Err(("TypeError", format!("{method} requires a DOM node.")));
            }
            if method == "insertBefore"
                && !matches!(
                    args.get(1),
                    Some(Value::Null | Value::Undefined) | Some(Value::Host(_))
                )
            {
                return Err((
                    "TypeError",
                    "insertBefore requires a node or null reference.".into(),
                ));
            }
            // Removal has its own simple precondition. Pre-insertion hierarchy
            // validation has different precedence and is not inferred here.
            if method=="removeChild" {
                if let (Some(parent),Some(Value::Host(handle)))=(node_index(object).and_then(|id|self.doc.nodes.get(id)),args.first()) {
                    if node_index(*handle).is_some_and(|child|child<self.doc.nodes.len() && !parent.children.contains(&child)) {
                        return Err(("NotFoundError","The node is not a child of this parent.".into()));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_inert_fragment_insertion(
        &self,
        object: u32,
        template: Option<u32>,
        method: &str,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        let parent = node_index(object).filter(|&node| self.dom_accessible(node))
            .ok_or_else(|| ("TypeError", "Fragment destination is unavailable.".into()))?;
        if !matches!(self.doc.nodes[parent].kind, NodeKind::Element { .. } | NodeKind::DocumentFragment) {
            return Err(("HierarchyRequestError", "This Node cannot contain children.".into()));
        }
        if let Some(host) = template.and_then(node_index) {
            let mut ancestor = Some(parent);
            for _ in 0..=MAX_DOM_DEPTH {
                let Some(node) = ancestor else { break; };
                if node == host {
                    return Err(("HierarchyRequestError", "Fragment insertion would create a DOM cycle.".into()));
                }
                ancestor = self.state.tree.parents[node];
            }
        }
        if method == "insertBefore" {
            match args.get(1) {
                Some(Value::Null | Value::Undefined) => {}
                Some(Value::Host(reference)) if *reference == DOCUMENT || self.inert_node_type(*reference).is_some()
                    || node_index(*reference).is_some() => {
                    if !node_index(*reference).is_some_and(|node|
                        self.dom_accessible(node) && self.state.tree.parents[node] == Some(parent))
                    {
                        return Err(("NotFoundError", "The reference node is not a child of this parent.".into()));
                    }
                }
                _ => return Err(("TypeError", "insertBefore requires a node or null reference.".into())),
            }
        }
        Ok(())
    }

    pub(super) fn dom_has(&mut self, object: u32, property: &str) -> Result<Option<bool>, String> {
        if self.fragment_receiver(object) && matches!(property,"nodeValue"|"textContent") {self.state.work(1)?;return Ok(Some(true));}
        if self.fragment_receiver(object) {self.state.work(1)?;return Ok(Some(common_node_property(property)||property=="childNodes"||self.dom_method(object,property).is_some()));}
        if property=="childNodes" && node_index(object).is_some() {self.dom_node(object)?;return Ok(Some(true));}
        if property=="controls"&&self.media_interface_receiver(object) {self.node(object)?;return Ok(Some(true));}
        if self.media_element_receiver(object) {if let Some(value)=self.media_element_has(object,property)? {return Ok(Some(value));}}
        let Some(node) = node_index(object) else {
            return Ok(None);
        };
        let node = self.dom_node(node_handle(node))?;
        if self.anchor_url_property(node, property) { return Ok(Some(true)); }
        if self.style_text_attribute(object, property) { return Ok(Some(true)); }
        if property=="src"&&tag(self.doc,node)=="script"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return Ok(Some(true));}
        if property=="href"&&tag(self.doc,node)=="link"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return Ok(Some(true));}
        if matches!(property,"dir"|"lang"|"align"|"useMap") {
            if self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign {return Err("HTML reflection requires a known namespace context.".into());}
            return Ok(Some(self.html_text_reflection(node,property).is_some()));
        }
        if property == "getBoundingClientRect" { return Ok(Some(self.class_list_element_receiver(object))); }
        if property == "attributes" {
            if !matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) { return Ok(Some(false)); }
            if !matches!(self.doc.nodes[node].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg) { return Err("Attribute map requires known namespace provenance.".into()); }
            return Ok(Some(true));
        }
        if matches!(property,"localName"|"prefix"|"namespaceURI") {
            if matches!(self.doc.nodes[node].kind,NodeKind::Element {..}) && self.doc.nodes[node].namespace.element_uri().is_none() {
                return Err(self.element_naming_error(node));
            }
            return Ok(Some(matches!(self.doc.nodes[node].kind,NodeKind::Element {..})));
        }
        if property == "classList" { return Ok(Some(matches!(self.doc.nodes[node].kind,NodeKind::Element {..}))); }
        if property=="srcdoc" {
            if self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign {return Err("srcdoc requires known namespace provenance.".into());}
            return Ok(Some(self.iframe_srcdoc_property(node)));
        }
        if node_constant(property).is_some() { return Ok(Some(true)); }
        if matches!(property, "tabIndex" | "offsetLeft" | "offsetTop" | "offsetWidth" | "offsetHeight" | "scrollWidth" | "scrollHeight") && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) {
            if self.doc.nodes[node].namespace != crate::html::Namespace::Html {
                return Err("tabIndex for unsupported foreign parser contexts is unavailable.".into());
            }
            return Ok(Some(true));
        }
        if self.frame_content(node, property).is_some() { return Ok(Some(true)); }
        if self.dom_stylesheet(node)
            && (script_stylesheets::link_attribute(property).is_some()
                || matches!(property, "onload" | "onerror"))
        {
            return Ok(Some(true));
        }
        if matches!(property, "vspace" | "hspace") {
            if self.doc.nodes[node].namespace == crate::html::Namespace::UnknownForeign {
                return Err("Spacing lookup requires a known namespace context.".into());
            }
            return Ok(Some(self.html_spacing_property(node, property)));
        }
        if property == "border" {
            if self.doc.nodes[node].namespace == crate::html::Namespace::UnknownForeign {
                return Err("Border lookup requires a known namespace context.".into());
            }
            return Ok(Some(self.html_border_property(node)));
        }
        if property == "isMap" {
            if self.doc.nodes[node].namespace == crate::html::Namespace::UnknownForeign {
                return Err("Image-map reflection requires a known namespace context.".into());
            }
            return Ok(Some(self.html_image_map_property(node)));
        }
        if matches!(property, "innerHTML" | "outerHTML") && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) { return Ok(Some(true)); }
        if common_node_property(property) || self.dom_method(object, property).is_some() {
            return Ok(Some(true));
        }
        if self.doc.nodes[node].kind.character_data().is_some() {
            return Ok(Some(matches!(
                property,
                "data" | "nodeValue" | "textContent" | "length"
            )));
        }
        if tag(self.doc, node) == "script" && matches!(property, "nonce" | "text" | "textContent") {
            return Ok(Some(true));
        }
        if self.dom_script(node)
            && (script_attribute(property).is_some()
                || matches!(property, "text" | "textContent" | "onload" | "onerror"))
        {
            return Ok(Some(true));
        }
        Ok(None)
    }

    pub(super) fn dom_get(&mut self, object: u32, property: &str) -> Result<Option<Value>, String> {
        if self.fragment_receiver(object) {
            if property=="nodeValue" {self.state.work(1)?;return Ok(Some(Value::Null));}
            if property=="textContent" {let node=self.dom_node(object)?;return self.text_content(node).map(|value|Some(Value::String(value)));}
            if !common_node_property(property) && property!="childNodes" {self.state.work(1)?;return Ok(Some(Value::Undefined));}
        }
        if property=="childNodes" && node_index(object).is_some() {return self.create_child_list(object).map(Some);}
        if property=="controls"&&self.media_interface_receiver(object) {return self.media_element_get(object,property);}
        if self.media_element_receiver(object) {if let Some(value)=self.media_element_get(object,property)? {return Ok(Some(value));}}
        if node_index(object).is_none() {
            return Ok(None);
        }
        let node = self.dom_node(object)?;
        if self.anchor_url_property(node, property) { return self.anchor_url_get(node, property).map(Some); }
        if self.style_text_attribute(object, property) {
            let raw=self.doc.attr(node,property).unwrap_or("");
            if raw.len()>self.state.limits.host.string_bytes{return Err("Style attribute exceeds the configured string allowance.".into());}
            self.state.copies(raw.len())?;return Ok(Some(Value::String(raw.to_owned())));
        }
        if property=="src"&&tag(self.doc,node)=="script"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return self.image_source(node).map(Some);}
        if property=="href"&&tag(self.doc,node)=="link"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return self.reflected_url(node,"href").map(Some);}
        if matches!(property,"dir"|"lang"|"align"|"useMap") {return self.html_text_reflection_get(node,property).map(Some);}
        if matches!(property, "innerHTML" | "outerHTML") && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) {
            return self.serialize_html(node, property == "outerHTML").map(Some);
        }
        if property == "attributes" {
            if !matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) { return Ok(Some(Value::Undefined)); }
            if !matches!(self.doc.nodes[node].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg) { return Err("Attribute map requires known namespace provenance.".into()); }
            return Ok(Some(Value::Host(script_named_map::handle(object))));
        }
        if matches!(property,"localName"|"prefix"|"namespaceURI") {
            if !matches!(self.doc.nodes[node].kind,NodeKind::Element {..}) {return Ok(Some(Value::Undefined));}
            let uri = self.doc.nodes[node].namespace.element_uri().ok_or_else(|| self.element_naming_error(node))?;
            return match property {
                "prefix"=>Ok(Some(Value::Null)),
                "namespaceURI"=>self.string(uri).map(Some),
                _=>{
                    let name=tag(self.doc,node);
                    if name.len()>self.state.limits.host.string_bytes {return Err("Element local name exceeds the host string bound.".into());}
                    self.state.copies(name.len())?;
                    Ok(Some(Value::String(name.to_owned())))
                },
            };
        }
        if property=="srcdoc" {return self.iframe_srcdoc_get(node).map(Some);}
        if matches!(property,"scrollWidth"|"scrollHeight") {
            if !matches!(self.doc.nodes[node].kind,NodeKind::Element {..})
                || self.doc.nodes[node].namespace != crate::html::Namespace::Html {
                return Err("Scroll dimensions require a known HTML Element.".into());
            }
            // A detached Element has no associated CSS layout box. Connected
            // extents require renderer measurements, never a client-size alias.
            if !self.state.dom.connected[node] { return Ok(Some(Value::Number(0.0))); }
            if property=="scrollHeight" {
                if let Some(geometry)=&mut self.state.geometry {
                    let root=self.state.tree.document_element==Some(node);
                    return geometry.read(self.doc,self.forms,node,root,property,&mut self.state.host_work,&mut self.state.copy_work).map(|value|Some(Value::Number(value)));
                }
            }
            return Err(self.unavailable_scroll(node,property));
        }
        if property == "classList" {
            if !matches!(self.doc.nodes[node].kind,NodeKind::Element {..}) { return Ok(Some(Value::Undefined)); }
            if !matches!(self.doc.nodes[node].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg) { return Err("Class token list requires known namespace provenance.".into()); }
            return Ok(Some(Value::Host(script_class_list::handle(object))));
        }
        if matches!(property, "vspace" | "hspace") {
            return self.html_spacing_get(node, property).map(Some);
        }
        if property == "border" { return self.html_border_get(node).map(Some); }
        if property == "isMap" { return self.html_image_map_get(node).map(Some); }
        if let Some(number) = node_constant(property) {
            return Ok(Some(Value::Number(f64::from(number))));
        }
        if property == "tabIndex" { return self.html_tab_index(node).map(|value| Some(Value::Number(f64::from(value)))); }
        if matches!(property, "offsetLeft" | "offsetTop") && !self.form_named_control(object, property) {
            let axis = if property == "offsetLeft" {crate::script_geometry::Axis::X} else {crate::script_geometry::Axis::Y};
            return self.html_offset(object, Offset::Coordinate(axis)).map(Some);
        }
        let text = matches!(self.doc.nodes[node].kind, NodeKind::Text(_));
        let comment = matches!(self.doc.nodes[node].kind, NodeKind::Comment(_));
        let target = match &self.doc.nodes[node].kind {
            NodeKind::ProcessingInstruction { target, .. } => Some(target.as_str()),
            _ => None,
        };
        if let Some(value) = self.frame_content(node, property) { return Ok(Some(value)); }
        if common_node_property(property) {
            return Ok(Some(match property {
                "nodeType" => Value::Number(if self.fragment_receiver(object) {11.0} else if text { 3.0 } else if comment { 8.0 } else if target.is_some() { 7.0 } else { 1.0 }),
                "nodeName" => {
                    if let Some(target) = target {
                        self.state.copies(target.len())?;
                        return Ok(Some(Value::String(target.into())));
                    }
                    let name = if self.fragment_receiver(object) { "#document-fragment".into() } else if text {
                        "#text".into()
                    } else if comment {
                        "#comment".into()
                    } else {
                        element_node_name(self.doc, node)
                    };
                    return self.string(&name).map(Some);
                }
                "ownerDocument" => Value::Host(DOCUMENT),
                "isConnected" => Value::Bool(self.state.dom.connected[node]),
                "parentNode" => return self.element_parent(node, true).map(Some),
                "parentElement" => return self.element_parent(node, false).map(Some),
                "firstChild" | "lastChild" => {
                    self.check_iframe_child_topology(node)?;
                    self.state.work(self.doc.nodes[node].children.len())?;
                    let children = &self.doc.nodes[node].children;
                    let child = if property == "firstChild" {
                        children
                            .iter()
                            .copied()
                            .find(|&child| self.dom_accessible(child))
                    } else {
                        children
                            .iter()
                            .rev()
                            .copied()
                            .find(|&child| self.dom_accessible(child))
                    };
                    child.map_or(Value::Null, |child| Value::Host(node_handle(child)))
                }
                _ => {
                    let mut found = None;
                    if let Some(parent) = self.state.tree.parents[node] {
                        let siblings = &self.doc.nodes[parent].children;
                        self.state.work(1)?;
                        let slot = self.state.tree.sibling_slots.get(node).copied();
                        let index = if slot.is_some_and(|slot| siblings.get(slot) == Some(&node)) {
                            slot
                        } else {
                            // Checked detached-tree updates can publish a
                            // parent before the complete index is rebuilt.
                            self.state.work(siblings.len())?;
                            siblings.iter().position(|&child| child == node)
                        };
                        if let Some(index) = index {
                            let forward = property == "nextSibling";
                            let count = if forward { siblings.len() - index - 1 } else { index };
                            for distance in 1..=count {
                                self.state.work(1)?;
                                let sibling = siblings[if forward { index + distance } else { index - distance }];
                                if self.dom_accessible(sibling) {
                                    found = Some(sibling);
                                    break;
                                }
                            }
                        }
                    }
                    found.map_or(Value::Null, |child| Value::Host(node_handle(child)))
                }
            }));
        }
        if let Some(value) = self.doc.nodes[node].kind.character_data() {
            return Ok(Some(match property {
                "data" | "nodeValue" | "textContent" => {
                    if value.len() > self.state.limits.host.string_bytes {
                        return Err("DOM result exceeds 32 KiB.".into());
                    }
                    self.state.copies(value.len())?;
                    Value::String(value.into())
                }
                "length" => {
                    self.state.copies(value.len())?;
                    Value::Number(value.encode_utf16().count() as f64)
                }
                _ => Value::Undefined,
            }));
        }
        if let Some(value) = self.stylesheet_get(node, property)? {
            return Ok(Some(value));
        }
        if tag(self.doc, node) == "script" && property == "nonce" {
            let nonce = self.doc.attr(node, "nonce").unwrap_or("").to_owned();
            return self.string(&nonce).map(Some);
        }
        if tag(self.doc, node) == "script" && matches!(property, "text" | "textContent")
            || self.created_html_style(node) && property == "textContent" {
            return self
                .raw_child_text(node)
                .map(|text| Some(Value::String(text)));
        }
        if self.dom_script(node) {
            if let Some(name) = script_attribute(property) {
                return Ok(Some(if boolean_script_attribute(name) {
                    Value::Bool(
                        self.doc.attr(node, name).is_some()
                            || name == "async" && self.state.dom.force_async.contains(&node),
                    )
                } else if name == "src" {
                    self.image_source(node)?
                } else {
                    let value = self.doc.attr(node, name).unwrap_or("").to_owned();
                    self.string(&value)?
                }));
            }
        }
        Ok(None)
    }

    pub(super) fn create_dom_node(&mut self,kind:NodeKind)->Result<Value,String>{let namespace=if matches!(kind,NodeKind::Element{..}){crate::html::Namespace::Html}else{crate::html::Namespace::None};self.create_dom_node_ns(kind,namespace)}
    pub(super) fn create_dom_node_ns(&mut self, kind: NodeKind,namespace:crate::html::Namespace) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        if self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes) >= self.state.node_limit {
            return Err("DOM appended-node limit reached (256 per document).".into());
        }
        let bytes = match &kind {
            NodeKind::Element { tag, .. } => tag.len(),
            NodeKind::Text(text) | NodeKind::Comment(text) => text.capacity(),
            NodeKind::ProcessingInstruction { target, data } => target.capacity().saturating_add(data.capacity()),
            NodeKind::DocumentFragment => 0,
        };
        let node = self.doc.nodes.len();
        let handle = u32::try_from(node).ok().and_then(|index|NODE_BASE.checked_add(index))
            .filter(|handle|*handle<STYLE_BASE).ok_or("DOM node identity space exhausted.")?;
        // Reserve every owner container before bytes or a native identity are published.
        let remaining = self.state.retained_limit.saturating_sub(self.state.retained);
        let available = remaining.checked_sub(bytes).ok_or_else(||format!(
            "DOM retained node/payload backing allowance exhausted ({remaining} bytes available; {bytes} payload bytes requested)."))?;
        let (node_growth, node_bytes) = self.prepare_main_nodes(1, available)?;
        let work = match &kind {
            NodeKind::Text(text) | NodeKind::Comment(text) => text.len(),
            NodeKind::ProcessingInstruction { target, data } => target.len().saturating_add(data.len()),
            _ => bytes,
        };
        self.state.replace_storage(0, bytes.checked_add(node_bytes).ok_or("DOM node/payload storage overflow.")?, work)?;
        self.commit_main_nodes(node_growth);
        if matches!(&kind, NodeKind::Element { tag, .. } if tag == "script") {
            self.state.dom.force_async.insert(node);
        }
        self.doc.nodes.push(Node {
            namespace,
            kind,
            children: Vec::new(),
        });
        self.state.dom.created.insert(node);
        self.state.detached_nodes.insert(node);
        self.state.tree.active.push(false);
        self.state.tree.parents.push(None);
        self.state.dom.connected.push(false);
        Ok(Value::Host(handle))
    }

    // No mutation may invalidate a loaded resource, native form value, form
    // ownership, button caption or disabled fieldset's first-legend relation.
    pub(super) fn mutation_ancestors(&mut self, node: usize) -> Result<(), String> {
        let mut current = Some(node);
        while let Some(index) = current {
            self.state.work(1)?;
            if self.forms.controls.contains_key(&index)
                || tag(self.doc, index) == "input"
                || tag(self.doc, index) == "script" && !self.dom_script(index)
                || matches!(
                    tag(self.doc, index),
                    "title"
                        | "style"
                        | "template"
                        | "iframe"
                        | "object"
                        | "select"
                        | "option"
                        | "optgroup"
                        | "textarea"
                        | "video" | "audio" | "source"
                )
                || tag(self.doc, index) == "noscript" && self.doc.scripting_enabled
            {
                return Err("Changing native control contents or inert resource subtrees is not implemented.".into());
            }
            current = self.state.tree.parents[index];
        }
        Ok(())
    }

    /// Only innerHTML opts in to traversal of an old bare LINK. Its separate
    /// precommit stylesheet-state check must pass before children are replaced.
    pub(super) fn mutation_subtree_for_fragment(&mut self, root: usize, bare_links: bool) -> Result<(Vec<usize>, usize), String> {
        self.checked_mutation_subtree(root, bare_links, false)
    }

    pub(super) fn mutation_subtree_with_styles(&mut self,root:usize)->Result<(Vec<usize>,usize),String>{self.checked_mutation_subtree(root,false,true)}

    fn checked_mutation_subtree(&mut self, root: usize, bare_links: bool, direct_style: bool) -> Result<(Vec<usize>, usize), String> {
        let mut nodes = Vec::new();
        let mut stack = vec![(root, 1usize)];
        let mut depth = 0;
        while let Some((node, level)) = stack.pop() {
            self.state.work(1)?;
            depth = depth.max(level);
            self.validate_svg_node(node)?;
            let name = tag(self.doc, node);
            if self.native_button(node) || self.fresh_button(node) {
                self.state.work(self.doc.nodes.len())?;
                let (work,copy) = forms::button_caption_work(self.doc,node)?;
                self.state.work(work)?;self.state.copies(copy)?;
                forms::validate_button_caption(self.doc,node)?;
            }
            let select_branch = self.owned_select_member(root, node);
            let owned_select = matches!(name, "select" | "option" | "optgroup") && select_branch;
            if select_branch && matches!(&self.doc.nodes[node].kind, NodeKind::Element { .. })
                && !matches!(name, "select" | "option" | "optgroup") {
                return Err("Moving nonconventional select descendants or resources is not implemented.".into());
            }
            if forms::kind(self.doc, node).is_some() && !owned_select && !self.hidden_input(node) && !self.owned_fragment_text_input(node) && !self.owned_native_checkbox(node) && !self.owned_native_radio(node) && !self.owned_native_textarea(node) && !self.fresh_text_input(node) && !self.native_button(node) && !self.fresh_button(node)
                || matches!(
                    name,
                    "html"
                        | "head"
                        | "body"
                        | "form"
                        | "fieldset"
                        | "legend"
                        | "title"
                        | "object"
                        | "embed"
                        | "base"
                        | "meta"
                        | "video" | "audio" | "source"
                )
                || name=="template"&&!self.movable_template_host(node)
                || name == "style" && !(direct_style && self.created_html_style(node))
                || matches!(name, "option" | "optgroup") && !owned_select
                || name == "noscript" && self.doc.scripting_enabled
                || name == "script" && !self.dom_script(node)
                || name == "link" && !self.dom_stylesheet(node)
                    && !(bare_links && script_stylesheets::bare_fragment_link(self.doc, node))
            {
                let frame = if name == "iframe" {
                    let source = match self.doc.attr(node, "src").map(str::trim) {
                        None => "absent",
                        Some("") => "empty",
                        Some("about:blank") => "blank",
                        Some(_) => "other",
                    };
                    format!(" Frame source={source}; srcdoc={}; sandbox={}; hidden={}; inline-style={}.",
                        self.doc.attr(node, "srcdoc").is_some(), self.doc.attr(node, "sandbox").is_some(),
                        self.doc.attr(node, "hidden").is_some(), self.doc.attr(node, "style").is_some())
                } else { String::new() };
                return Err(format!("DOM tree mutation of controls, loaded resources or structural roots is not implemented. Blocked node={node}; tag={:?}; connected={}; created={}.{frame}",
                    prefix(name, 64), self.state.dom.connected.get(node).copied().unwrap_or(false), self.state.dom.created.contains(&node)));
            }
            let frame = name == "iframe";
            let image = name == "img";
            if frame { self.validate_blank_frame(node)?; }
            if image { self.validate_plain_image(node)?; }
            if let Some(id) = self.doc.attr(node, "id").filter(|id| !id.is_empty()) {
                self.state.work(self.forms.controls.len())?;
                self.state.copies(
                    self.forms
                        .controls
                        .keys()
                        .map(|&control| self.doc.attr(control, "form").map_or(0, str::len))
                        .sum(),
                )?;
                if self
                    .forms
                    .controls
                    .keys()
                    .any(|&control| self.doc.attr(control, "form") == Some(id))
                {
                    return Err(
                        "Moving an id used by an explicit form association is not implemented."
                            .into(),
                    );
                }
            }
            nodes.push(node);
            stack.extend(
                self.doc.nodes[node]
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, level + 1)),
            );
        }
        Ok((nodes, depth))
    }

    fn validate_replace_child(&self, object: u32, args: &[Value]) -> Result<(), (&'static str, String)> {
        // Web IDL converts BOTH Node arguments before the tree algorithm.
        if args.iter().take(2).count() != 2 || !args.iter().take(2).all(|value|
            matches!(value, Value::Host(handle) if self.namespace_receiver(*handle))) {
            return Err(("TypeError", "replaceChild requires two Node arguments.".into()));
        }
        let parent = node_index(object).unwrap();
        if !matches!(self.doc.nodes[parent].kind, NodeKind::Element { .. } | NodeKind::DocumentFragment) {
            return Err(("HierarchyRequestError", "Text nodes cannot contain children.".into()));
        }
        let Value::Host(incoming) = args[0] else { unreachable!() };
        // An inclusive ancestor is invalid before old-child membership is tested.
        let mut ancestor = Some(parent);
        while let Some(node) = ancestor {
            if node_index(incoming) == Some(node) || incoming == DOCUMENT && node == 0 {
                return Err(("HierarchyRequestError", "Replacement would create a DOM cycle.".into()));
            }
            ancestor = self.state.tree.parents.get(node).copied().flatten();
        }
        let Value::Host(outgoing) = args[1] else { unreachable!() };
        if node_index(outgoing).is_none_or(|child| !self.doc.nodes[parent].children.contains(&child)) {
            return Err(("NotFoundError", "The replaced node is not a child of this parent.".into()));
        }
        if incoming == DOCUMENT || self.attr_receiver(incoming) {
            return Err(("HierarchyRequestError", "A Document or Attr cannot be inserted as a child.".into()));
        }
        Ok(())
    }

    fn replace_dom_child(&mut self, parent: usize, args: &[Value]) -> Result<Value, String> {
        // The named hierarchy/membership errors have already passed validate_call.
        let Value::Host(incoming) = args[0] else { unreachable!() };
        let Value::Host(outgoing) = args[1] else { unreachable!() };
        let child = self.dom_node(incoming)?;
        if self.fragment_receiver(incoming) {return self.splice_document_fragment(parent,"replaceChild",args);}
        let old = self.dom_node(outgoing)?;
        if child == old { return Ok(Value::Host(outgoing)); }
        self.mutation_ancestors(parent)?;
        let source_parent = self.state.tree.parents[child];
        if let Some(source) = source_parent { self.mutation_ancestors(source)?; }
        self.validate_svg_edge(node_handle(parent),node_handle(child))?;
        let (incoming_nodes, depth) = self.mutation_subtree_with_styles(child)?;
        self.validate_button_children(parent,&incoming_nodes)?;
        let (outgoing_nodes, _) = self.mutation_subtree_with_styles(old)?;
        if !matches!(self.doc.nodes[parent].namespace,crate::html::Namespace::Html|crate::html::Namespace::Svg) && !self.fragment_receiver(node_handle(parent))
            || matches!(tag(self.doc, parent), "script" | "link" | "iframe")
            || incoming_nodes.iter().chain(&outgoing_nodes).any(|&node|
                self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign
                    || matches!(tag(self.doc, node), "script" | "link" | "iframe")) {
            return Err("replaceChild in foreign, script, stylesheet or frame contexts is not implemented; no tree was changed.".into());
        }
        if tag(self.doc, parent) == "picture" && tag(self.doc, child) == "img" {
            return Err("Dynamic picture image selection is not implemented.".into());
        }
        let mut parent_depth = 0usize;
        let mut ancestor = Some(parent);
        while let Some(node) = ancestor {
            self.state.work(1)?;
            parent_depth += 1;
            ancestor = self.state.tree.parents[node];
        }
        if parent_depth.saturating_add(depth) > MAX_DOM_DEPTH { return Err("DOM tree depth limit reached during replacement.".into()); }
        let work = self.doc.nodes[parent].children.len()
            .checked_add(source_parent.map_or(0, |node| self.doc.nodes[node].children.len()))
            .and_then(|count| count.checked_add(incoming_nodes.len()))
            .and_then(|count| count.checked_add(outgoing_nodes.len()))
            .ok_or("DOM replacement work size overflow.")?;
        self.state.work(work)?;

        // Stage the final vectors. Removing an existing sibling before locating
        // old handles either sibling order; incoming may also descend from old.
        let mut children = Vec::new();
        let destination_count = self.doc.nodes[parent].children.len().checked_sub(usize::from(source_parent==Some(parent)))
            .ok_or("DOM replacement child count underflow.")?;
        self.state.copies(destination_count.checked_mul(size_of::<usize>()).ok_or("DOM replacement child copy overflow.")?)?;
        children.try_reserve_exact(destination_count).map_err(|_| "DOM replacement children reservation failed.")?;
        for &node in &self.doc.nodes[parent].children {
            if node == child { continue; }
            children.push(if node == old { child } else { node });
        }
        let mut source_children = Vec::new();
        let mut old_links = child_link_bytes(&self.doc.nodes[parent].children);
        if let Some(source) = source_parent.filter(|&source| source != parent) {
            let count = self.doc.nodes[source].children.len().checked_sub(1).ok_or("DOM replacement source child count underflow.")?;
            self.state.copies(count.checked_mul(size_of::<usize>()).ok_or("DOM replacement source copy overflow.")?)?;
            source_children.try_reserve_exact(count).map_err(|_| "DOM replacement source reservation failed.")?;
            old_links = old_links.checked_add(child_link_bytes(&self.doc.nodes[source].children)).ok_or("DOM replacement old link storage overflow.")?;
            source_children.extend(self.doc.nodes[source].children.iter().copied().filter(|&node| node != child));
        }
        let mut incoming_set = HashSet::new();
        incoming_set.try_reserve(incoming_nodes.len()).map_err(|_| "DOM replacement topology reservation failed.")?;
        incoming_set.extend(incoming_nodes.iter().copied());
        let connected = self.state.dom.connected[parent];
        let changed_layout = connected || self.state.dom.connected[child];
        if incoming_nodes.iter().any(|&node|tag(self.doc,node)=="style"){self.validate_style_destination(parent)?;}
        let sheets=self.prepare_style_transfers(&incoming_nodes,&outgoing_nodes,connected)?;
        let images = self.prepare_image_connections(&incoming_nodes, connected)?;

        let form_plan = self.prepare_select_form_change(None, &[(old, None), (child, Some(parent))])?;
        let bytes = child_link_bytes(&children).checked_add(child_link_bytes(&source_children))
            .and_then(|bytes|bytes.checked_add(form_plan.retained_bytes)).ok_or("DOM replacement child/native storage overflow.")?;
        self.state.prepare_range_changes(3,incoming_nodes.len().saturating_add(outgoing_nodes.len()))?;
        let (source_position,replacement_position) = if self.state.ranges.is_empty() {(None,0)} else {
            self.state.work(children.len().saturating_add(source_parent.map_or(0,|source|self.doc.nodes[source].children.len())))?;
            (source_parent.and_then(|source|self.doc.nodes[source].children.iter().position(|&node|node==child)),
             children.iter().position(|&node|node==child).ok_or("Range replacement position is unavailable.")?)
        };
        self.state.replace_storage(old_links, bytes, form_plan.retained_bytes)?;
        if let (Some(source),Some(index))=(source_parent,source_position) {self.state.ranges_remove(source,index,1,&incoming_nodes);}
        self.state.ranges_remove(parent,replacement_position,1,&outgoing_nodes);
        self.state.ranges_insert(parent,replacement_position,1);
        // No fallible work remains after publishing either parent's new list.
        if let Some(source) = source_parent.filter(|&source| source != parent) { self.doc.nodes[source].children = source_children; }
        self.doc.nodes[parent].children = children;
        self.state.tree.parents[old] = None;
        self.state.tree.parents[child] = Some(parent);
        for node in outgoing_nodes {
            if !incoming_set.contains(&node) {
                self.state.dom.connected[node] = false;
                self.state.detached_nodes.insert(node);
            }
        }
        for node in incoming_nodes {
            self.state.dom.connected[node] = connected;
            if connected { self.state.detached_nodes.remove(&node); }
            else { self.state.detached_nodes.insert(node); }
        }
        form_plan.commit(self.forms);
        self.commit_image_connections(images);self.commit_inline_sheet_batch(sheets);
        self.state.dom_committed();
        self.state.dirty = true;
        if changed_layout { self.state.layout_changed(); }
        Ok(Value::Host(outgoing))
    }

    fn mutate_dom_tree(
        &mut self,
        parent: usize,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if !matches!(self.doc.nodes[parent].kind, NodeKind::Element { .. } | NodeKind::DocumentFragment) {
            return Err("HierarchyRequestError: Text nodes cannot contain children.".into());
        }
        let Some(Value::Host(handle)) = args.first() else {
            return Err("A DOM node is required.".into());
        };
        if method != "removeChild" {
            if let Some(template) = self.checked_inert_fragment(*handle) {
                self.validate_inert_fragment_insertion(node_handle(parent), template, method, args)
                    .map_err(|(name, error)| format!("{name}: {error}"))?;
                let reference = match (method, args.get(1)) {
                    ("insertBefore", Some(Value::Host(reference))) => node_index(*reference),
                    _ => None,
                };
                return self.splice_checked_inert_fragment(parent, *handle, reference);
            }
        }
        let child = self.dom_node(*handle)?;
        if self.fragment_receiver(*handle) && method!="removeChild" {return self.splice_document_fragment(parent,method,args);}
        let reference = if method == "insertBefore" {
            match args.get(1) {
                Some(Value::Host(handle)) => Some(self.dom_node(*handle)?),
                Some(Value::Null | Value::Undefined) => None,
                _ => return Err("insertBefore requires a node or null reference.".into()),
            }
        } else {
            None
        };
        if reference.is_some_and(|reference| self.state.tree.parents[reference] != Some(parent))
            || method == "removeChild" && self.state.tree.parents[child] != Some(parent)
        {
            return Err("NotFoundError: The reference node is not a child of this parent.".into());
        }
        let mut ancestor = Some(parent);
        let mut parent_depth = 0usize;
        while let Some(index) = ancestor {
            self.state.work(1)?;
            if index == child && method != "removeChild" {
                return Err("HierarchyRequestError: Insertion would create a DOM cycle.".into());
            }
            parent_depth += 1;
            ancestor = self.state.tree.parents[index];
        }
        if reference == Some(child) {
            return Ok(Value::Host(node_handle(child)));
        }
        if method!="removeChild"{self.validate_svg_edge(node_handle(parent),node_handle(child))?;}
        self.mutation_ancestors(parent)?;
        let old_parent = self.state.tree.parents[child];
        if let Some(old_parent) = old_parent {
            self.mutation_ancestors(old_parent)?;
        }
        let (descendants, depth) = self.checked_mutation_subtree(child, false, true).map_err(|error| format!(
            "{error} Operation={method}; parent={parent}; child={child}; old_parent={old_parent:?}."))?;
        if method!="removeChild"&&descendants.iter().any(|&node|tag(self.doc,node)=="style"){self.validate_style_destination(parent)?;}
        self.validate_button_children(parent,&descendants)?;
        self.state.work(descendants.len().saturating_mul(2))?;
        if descendants.iter().any(|&node| self.fresh_text_input(node) || self.owned_native_checkbox(node) || self.owned_native_radio(node) || self.owned_native_textarea(node) || self.fresh_button(node) || self.native_button(node))
            && descendants.iter().any(|&node| matches!(tag(self.doc,node),"script"|"link"|"iframe")) {
            return Err("New native INPUT admission with script, stylesheet or frame subtrees is not implemented; no tree was changed.".into());
        }
        if method != "removeChild" && tag(self.doc, parent) == "picture" && tag(self.doc, child) == "img" {
            return Err("Dynamic picture image selection is not implemented.".into());
        }
        if method != "removeChild" && parent_depth.saturating_add(depth) > MAX_DOM_DEPTH {
            return Err("DOM tree depth limit reached (128).".into());
        }
        if self.dom_script(parent) && !matches!(self.doc.nodes[child].kind, NodeKind::Text(_)) {
            return Err("Created scripts accept text children only in this subset.".into());
        }
        self.state.work(
            self.doc.nodes[parent]
                .children
                .len()
                .saturating_add(old_parent.map_or(0, |node| self.doc.nodes[node].children.len()))
                .saturating_add(descendants.len()),
        )?;
        let was_connected = self.state.dom.connected[child];
        let connected = method != "removeChild" && self.state.dom.connected[parent];
        let sheets=self.prepare_style_transfers(&descendants,&[],connected)?;
        let image_plans = self.prepare_image_connections(&descendants, connected)?;
        let frames = self.prepare_blank_frames(&descendants, connected, parent)?;
        let form_plan = self.prepare_select_form_change(None, &[(child, if method == "removeChild" { None } else { Some(parent) })])?;
        let (child_growth, child_bytes) = if method=="removeChild" || old_parent==Some(parent) { (None, 0) } else {
            let available = self.state.retained_limit.saturating_sub(self.state.retained)
                .checked_sub(form_plan.retained_bytes).ok_or("DOM child/native storage allowance exhausted.")?;
            retained_storage::grow(&self.doc.nodes[parent].children, 1, available)?
        };
        let copied = old_parent.map_or(0, |node| self.doc.nodes[node].children.len())
            .checked_add(if method=="removeChild" {0}else{self.doc.nodes[parent].children.len()+1})
            .and_then(|slots|slots.checked_add(if child_growth.is_some(){self.doc.nodes[parent].children.len()}else{0}))
            .and_then(|slots|slots.checked_mul(size_of::<usize>()))
            .and_then(|bytes|bytes.checked_add(form_plan.retained_bytes)).ok_or("DOM child/native copy work overflow.")?;
        let bytes = child_bytes.checked_add(form_plan.retained_bytes).ok_or("DOM child/native storage overflow.")?;
        self.state.prepare_range_changes(2,descendants.len())?;
        let source_position = if self.state.ranges.is_empty() {None} else {
            self.state.work(old_parent.map_or(0,|source|self.doc.nodes[source].children.len()))?;
            old_parent.and_then(|source|self.doc.nodes[source].children.iter().position(|&node|node==child))
        };
        self.state.replace_storage(0,bytes,copied)?;
        if let (Some(source),Some(index))=(old_parent,source_position) {self.state.ranges_remove(source,index,1,&descendants);}
        if let Some(mut children) = child_growth {
            children.append(&mut self.doc.nodes[parent].children);
            self.doc.nodes[parent].children=children;
        }
        if let Some(old_parent) = old_parent {
            self.doc.nodes[old_parent]
                .children
                .retain(|&node| node != child);
        }
        if method != "removeChild" {
            let position = reference
                .and_then(|reference| {
                    self.doc.nodes[parent]
                        .children
                        .iter()
                        .position(|&node| node == reference)
                })
                .unwrap_or(self.doc.nodes[parent].children.len());
            self.state.ranges_insert(parent,position,1);
            self.doc.nodes[parent].children.insert(position, child);
        }
        self.state.tree.parents[child] = if method == "removeChild" {
            None
        } else {
            Some(parent)
        };
        for &node in &descendants {
            self.state.dom.connected[node] = connected;
            if connected {
                self.state.detached_nodes.remove(&node);
            } else {
                self.state.detached_nodes.insert(node);
            }
        }
        form_plan.commit(self.forms);
        self.commit_inline_sheet_batch(sheets);
        self.commit_blank_frames(&descendants, frames);
        self.commit_image_connections(image_plans);
        self.state.dom_committed();
        self.state.dirty = true;
        if was_connected || connected {
            self.state.layout_changed();
        }
        for &node in &descendants {
            if self.dom_stylesheet(node) {
                self.queue_dynamic_stylesheet(node)?;
            }
        }
        if connected {
            for node in descendants {
                if self.dom_script(node) {
                    self.queue_dynamic_script(node)?;
                }
            }
            if self.dom_script(parent) {
                self.queue_dynamic_script(parent)?;
            }
        }
        Ok(Value::Host(node_handle(child)))
    }

    fn raw_child_text(&mut self, node: usize) -> Result<String, String> {
        let mut text = String::new();
        for &child in &self.doc.nodes[node].children {
            self.state.work(1)?;
            if let NodeKind::Text(value) = &self.doc.nodes[child].kind {
                if text.len().saturating_add(value.len()) > self.state.limits.host.string_bytes {
                    return Err("Raw element text exceeds the configured string allowance.".into());
                }
                self.state.copies(value.len())?;
                text.try_reserve(value.len()).map_err(|_| "Raw element text reservation failed.")?;
                text.push_str(value);
            }
        }
        Ok(text)
    }

    fn set_script_text(&mut self, node: usize, value: &Value) -> Result<(), String> {
        let text = if matches!(value, Value::Null) {
            String::new()
        } else {
            self.text(value)?
        };
        self.state.work(self.doc.nodes[node].children.len())?;
        let (child_growth, added) = if text.is_empty() { (None, 0) } else {
            let minimum = self.main_node_minimum(1)?;
            let available = self.state.retained_limit.saturating_sub(self.state.retained)
                .checked_sub(text.capacity()).and_then(|bytes|bytes.checked_sub(minimum))
                .ok_or("Script text/child/node storage allowance exhausted.")?;
            retained_storage::capacity(&self.doc.nodes[node].children, 1, available)?
        };
        // Keep the private child buffer reserved while node construction admits
        // the Text. No callback can observe this reservation; failure refunds it.
        self.state.prepare_range_changes(1,self.doc.nodes[node].children.len())?;
        self.state.replace_storage(0, added, if text.is_empty(){0}else{size_of::<usize>()})?;
        let new_child = if text.is_empty() {
            None
        } else {
            let created = match self.create_dom_node(NodeKind::Text(text)) {
                Ok(value) => value,
                Err(error) => { self.state.retained -= added; return Err(error); }
            };
            let Value::Host(handle) = created else {
                unreachable!();
            };
            node_index(handle)
        };
        self.state.ranges_remove(node,0,self.doc.nodes[node].children.len(),&self.doc.nodes[node].children);
        if let Some(children) = child_growth { self.doc.nodes[node].children = children; }
        for child in self.doc.nodes[node].children.drain(..) {
            self.state.detached_nodes.insert(child);
            self.state.dom.connected[child] = false;
            self.state.tree.parents[child] = None;
        }
        if let Some(child) = new_child {
            self.doc.nodes[node].children.push(child);
            self.state.tree.parents[child] = Some(node);
            self.state.dom.connected[child] = self.state.dom.connected[node];
            if self.state.dom.connected[node] {
                self.state.detached_nodes.remove(&child);
            }
        }
        self.state.dom_committed();
        self.state.dirty = true;
        if self.state.dom.connected[node] {
            self.queue_dynamic_script(node)?;
        }
        Ok(())
    }

    pub(super) fn dom_set(
        &mut self,
        object: u32,
        property: &str,
        value: &Value,
    ) -> Result<bool, String> {
        if self.fragment_receiver(object) {
            if property=="nodeValue" {self.state.work(1)?;return Ok(true);}
            if property=="textContent" {let text=if matches!(value,Value::Null|Value::Undefined){String::new()}else{self.text(value)?};let node=self.dom_node(object)?;self.set_text_content(node,text)?;return Ok(true);}
        }
        if node_index(object).is_none() {
            return Ok(false);
        }
        let node = self.dom_node(object)?;
        if property != "origin" && self.anchor_url_property(node, property) {
            self.anchor_url_set(node, property, value)?;
            return Ok(true);
        }
        if let Some(attribute)=self.html_text_reflection(node,property) {
            let value=self.text(value)?;self.attribute(node,attribute,Some(value))?;return Ok(true);
        }
        if let Some(old_bytes) = self.doc.nodes[node].kind.character_data_capacity() {
            if !matches!(property, "data" | "nodeValue" | "textContent") {
                return Ok(false);
            }
            let renders_text = matches!(self.doc.nodes[node].kind, NodeKind::Text(_));
            self.mutation_ancestors(node)?;
            let text = if matches!(value, Value::Null) && property != "data" {
                String::new()
            } else {
                self.text(value)?
            };
            self.state.prepare_range_changes(1,0)?;
            self.state.replace_storage(old_bytes, text.capacity(), text.len())?;
            self.state.ranges_replace_data(node);
            match &mut self.doc.nodes[node].kind {
                NodeKind::Text(data) | NodeKind::Comment(data) | NodeKind::ProcessingInstruction { data, .. } => {
                    *data = text;
                }
                _ => unreachable!("character data was checked above"),
            }
            self.state.dom_committed();
            if self.state.dom.connected[node] && renders_text {
                self.state.layout_changed();
                if let Some(parent) =
                    self.state.tree.parents[node].filter(|&parent| self.dom_script(parent))
                {
                    self.queue_dynamic_script(parent)?;
                }
            }
            return Ok(true);
        }
        if property == "innerHTML" {
            let text = self.text(value)?;
            self.set_inner_html(node, &text)?;
            return Ok(true);
        }
        if property == "outerHTML" && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) {
            return Err("outerHTML replacement is not implemented.".into());
        }
        if self.stylesheet_set(node, property, value)? {
            return Ok(true);
        }
        if self.dom_script(node) {
            if matches!(property, "text" | "textContent") {
                self.set_script_text(node, value)?;
                return Ok(true);
            }
            if let Some(name) = script_attribute(property) {
                let text = if boolean_script_attribute(name) {
                    value.truthy().then(String::new)
                } else {
                    Some(self.text(value)?)
                };
                self.attribute(node, name, text)?;
                if name == "async" {
                    self.state.dom.force_async.remove(&node);
                }
                if name == "src" && self.state.dom.connected[node] {
                    self.queue_dynamic_script(node)?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn dom_call(
        &mut self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Option<Value>, String> {
        if object==DOCUMENT && method=="createDocumentFragment" {
            return self.create_dom_node(NodeKind::DocumentFragment).map(Some);
        }
        if node_index(object).is_some() {
            match method {
                "hasAttributes" => {
                    let node = self.node(object)?;
                    let NodeKind::Element {attrs,..} = &self.doc.nodes[node].kind else {unreachable!()};
                    return Ok(Some(Value::Bool(!attrs.is_empty())));
                }
                "getAttributeNode" => return self.get_attribute_node(object, args).map(Some),
                "getAttributeNodeNS" => return self.get_attribute_node_ns(object, args).map(Some),
                "getAttributeNS" => return self.get_attribute_ns(object, args).map(Some),
                "setAttributeNS" => return self.set_attribute_ns(object, args).map(Some),
                "removeAttributeNS" => return self.remove_attribute_ns(object, args).map(Some),
                "removeAttributeNode" => return self.remove_attribute_node(object, args).map(Some),
                "setAttributeNode"|"setAttributeNodeNS"=>return self.set_attribute_node(object,args).map(Some),
                _ => (),
            }
        }
        if object==DOCUMENT && matches!(method,"createAttribute"|"createAttributeNS") {return self.create_attribute(args,method=="createAttributeNS").map(Some);}
        if object==DOCUMENT && method=="createElementNS" {
            let namespace=match args.first(){Some(Value::String(s)) if !s.is_empty()=>Some(s.as_str()),_=>None};
            let name=self.text(args.get(1).ok_or("createElementNS requires two arguments.")?)?;
            super::script_attribute_names::validate_element_namespace(namespace,&name).map_err(|(_,message)|message)?;
            if namespace==Some("http://www.w3.org/2000/svg")&&!name.contains(':'){
                if args.get(2).is_some_and(|v|!matches!(v,Value::Null|Value::Undefined)){return Err("SVG creation options are not implemented; no node was created.".into());}
                if !crate::html::svg_dom_name(&name){return Err("SVG element is outside the supported passive shape/container subset; no node was created.".into());}
                return self.create_dom_node_ns(NodeKind::Element{tag:name,id:None,href:None,attrs:Vec::new()},crate::html::Namespace::Svg).map(Some);
            }
            if namespace!=Some("http://www.w3.org/1999/xhtml")||name.contains(':') {return Err("createElementNS currently implements the HTML namespace with a null prefix; no node was created.".into());}
            if args.get(2).is_some_and(|v|!matches!(v,Value::Null|Value::Undefined)) {return Err("Element creation options/custom registries are not implemented; no node was created.".into());}
            if !name.bytes().all(|c|c.is_ascii_alphanumeric()||matches!(c,b'-'|b'_'|b'.')) {return Err("Valid element name is outside the supported factory spelling subset; no node was created.".into());}
            // Unlike createElement, preserve local-name case. Uppercase IMG is
            // an ordinary unknown HTML element, never an Image/resource alias.
            if name=="img" {return self.create_image(&[]).map(Some);}
            return self.create_dom_node(NodeKind::Element {tag:name,id:None,href:None,attrs:Vec::new()}).map(Some);
        }
        if object == DOCUMENT && matches!(method, "createElement" | "createTextNode" | "createComment") {
            let text = self.text(args.first().ok_or("DOM creation requires one argument.")?)?;
            if method == "createComment" {
                return self.create_dom_node(NodeKind::Comment(text)).map(Some);
            }
            if method == "createTextNode" {
                return self.create_dom_node(NodeKind::Text(text)).map(Some);
            }
            if text.eq_ignore_ascii_case("img") {
                return self.create_image(&[]).map(Some);
            }
            return self
                .create_dom_node(NodeKind::Element {
                    tag: text.to_ascii_lowercase(),
                    id: None,
                    href: None,
                    attrs: Vec::new(),
                })
                .map(Some);
        }
        if node_index(object).is_none() {
            return Ok(None);
        }
        if method == "replaceChild" {
            let node = self.dom_node(object)?;
            return self.replace_dom_child(node, args).map(Some);
        }
        if matches!(method, "appendChild" | "insertBefore" | "removeChild") {
            let node = self.dom_node(object)?;
            return self.mutate_dom_tree(node, method, args).map(Some);
        }
        if method == "focus" {
            let node = self.node(object)?;
            if self.state.tree.active[node]
                && self
                    .forms
                    .controls
                    .get(&node)
                    .is_some_and(|control| control.focusable())
            {
                self.state.dom.pending_focus = Some(node);
            }
            return Ok(Some(Value::Undefined));
        }
        Ok(None)
    }

    pub(super) fn named_document_property(&mut self, name: &str) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        let mut found = None;
        if name.is_empty() {
            return Ok(Value::Undefined);
        }
        for position in 0..self.state.tree.order.len() {
            let node = self.state.tree.order[position];
            self.state.work(1)?;
            if tag(self.doc, node) != "form" {
                continue;
            }
            let candidate = self.doc.attr(node, "name");
            self.state.copies(candidate.map_or(0, str::len))?;
            if candidate == Some(name) {
                if found.is_some() {
                    return Err("Multiple named forms require a live HTMLCollection, which is not implemented.".into());
                }
                found = Some(node);
            }
        }
        Ok(found.map_or(Value::Undefined, |node| Value::Host(node_handle(node))))
    }

    pub(super) fn named_form_property(&mut self, form: usize, name: &str) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        if name.is_empty() {
            return Ok(Value::Undefined);
        }
        let mut found = None;
        for position in 0..self.state.tree.order.len() {
            let node = self.state.tree.order[position];
            self.state.work(1)?;
            let Some(control) = self.forms.controls.get(&node) else {
                continue;
            };
            if control.owner != Some(form) || control.kind == Kind::Image {
                continue;
            }
            let id = self.doc.attr(node, "id");
            let candidate = self.doc.attr(node, "name");
            self.state.copies(
                id.map_or(0, str::len)
                    .saturating_add(candidate.map_or(0, str::len)),
            )?;
            if id == Some(name) || candidate == Some(name) {
                if found.is_some() {
                    return Err(
                        "Multiple named controls require RadioNodeList, which is not implemented."
                            .into(),
                    );
                }
                found = Some(node);
            }
        }
        Ok(found.map_or(Value::Undefined, |node| Value::Host(node_handle(node))))
    }
}

// https://dom.spec.whatwg.org/#locate-a-namespace (null prefix only).
// Supported HTML and SVG elements have a null prefix and keep their parser
// namespace. Unsupported foreign parser contexts have no proven namespace.
pub(super) fn default_namespace(
    doc: &Document, node: usize, parent: Option<usize>, state: &mut State,
) -> Result<Option<&'static str>, String> {
    state.work(1)?;
    let item = doc.nodes.get(node).ok_or("Invalid namespace node.")?;
    let element = if matches!(item.kind,NodeKind::DocumentFragment) { None } else if node == 0 {
        let mut found = None;
        for &child in &item.children {
            state.work(1)?;
            let item = doc.nodes.get(child).ok_or("Invalid document namespace child.")?;
            if matches!(item.kind, NodeKind::Element { .. }) { found = Some(item); break; }
        }
        found
    } else if matches!(item.kind, NodeKind::Element { .. }) { Some(item) }
    else if let Some(parent) = parent.filter(|&parent| parent != 0) {
        state.work(1)?;
        let item = doc.nodes.get(parent).ok_or("Invalid namespace parent.")?;
        if matches!(item.kind,NodeKind::DocumentFragment) {return Ok(None);}
        if !matches!(item.kind, NodeKind::Element { .. }) {
            return Err("Namespace parent is not an Element.".into());
        }
        Some(item)
    } else { None };
    match element.map(|item| item.namespace) {
        None => Ok(None),
        Some(namespace) => namespace.element_uri().map(Some)
            .ok_or_else(|| "Namespace lookup for unsupported foreign parser context is unavailable.".into()),
    }
}

// Inherited Node constants describe the interface, not implemented node factories.
pub(super) const NODE_CONSTANTS: &[(&str,u16)] = &[
    ("ELEMENT_NODE",1),
    ("ATTRIBUTE_NODE",2),
    ("TEXT_NODE",3),
    ("CDATA_SECTION_NODE",4),
    ("ENTITY_REFERENCE_NODE",5),
    ("ENTITY_NODE",6),
    ("PROCESSING_INSTRUCTION_NODE",7),
    ("COMMENT_NODE",8),
    ("DOCUMENT_NODE",9),
    ("DOCUMENT_TYPE_NODE",10),
    ("DOCUMENT_FRAGMENT_NODE",11),
    ("NOTATION_NODE",12),
    ("DOCUMENT_POSITION_DISCONNECTED",1),
    ("DOCUMENT_POSITION_PRECEDING",2),
    ("DOCUMENT_POSITION_FOLLOWING",4),
    ("DOCUMENT_POSITION_CONTAINS",8),
    ("DOCUMENT_POSITION_CONTAINED_BY",16),
    ("DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC",32),
];
pub(super) fn node_constant(property: &str) -> Option<u16> {
    NODE_CONSTANTS.iter().find(|(key,_)|*key==property).map(|(_,value)|*value)
}

#[cfg(test)]
mod namespaced_creation_tests {
    use super::*;
    #[test]
    fn svg_element_names_reflect_proven_namespace_and_preserve_fragment_replacement_guards() {
        let mut doc = crate::html::parse(concat!(
            "<html><body><circle id=htmlShape></circle><svg id=drawing>",
            "<g><circle id=shape></circle></g></svg><div id=neighbor></div><math id=guard><mi>x</mi></math></body></html>"
        ));
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
        let read = page.console_eval(&mut doc, &mut forms, r#"
            var shape=document.getElementById('shape'), ordinary=document.getElementById('htmlShape');
            var drawing=document.getElementById('drawing'), svgNS='http://www.w3.org/2000/svg';
            if (shape.namespaceURI!==svgNS || shape.localName!=='circle' || shape.prefix!==null ||
                shape.nodeName!=='circle' || shape.tagName!=='circle' || !shape.isDefaultNamespace(svgNS))
                throw 'SVG naming';
            if (ordinary.namespaceURI!=='http://www.w3.org/1999/xhtml' || ordinary.tagName!=='CIRCLE' ||
                ordinary.localName!=='circle' || ordinary.isDefaultNamespace(svgNS))
                throw 'HTML naming';
            shape.namespaceURI='changed';
            if (shape.namespaceURI!==svgNS || !('namespaceURI' in shape)) throw 'immutable namespace';
            var fragment=document.createDocumentFragment();
            fragment.appendChild(document.createElement('div'));
        "#);
        assert_eq!(read.errors, 0, "{}", page.report());
        let replacement = page.console_eval(&mut doc, &mut forms,
            "var inserted=fragment.firstChild;document.body.replaceChild(fragment,drawing);if(drawing.isConnected||shape.isConnected||inserted.parentNode!==document.body||fragment.childNodes.length!==0||drawing.namespaceURI!==svgNS)throw 'real SVG retirement';var blocked=document.createDocumentFragment();blocked.appendChild(document.createElement('div'));");
        assert_eq!(replacement.errors, 0, "{}", page.report());
        let nodes = format!("{:?}", doc.nodes);
        let commits = page.state.dom_commits;
        let refused = page.console_eval(&mut doc, &mut forms,
            "document.body.replaceChild(blocked,document.getElementById('guard'));");
        assert_eq!(refused.errors, 1, "unknown foreign subtree replacement must fail atomically");
        assert_eq!(format!("{:?}", doc.nodes), nodes);
        assert_eq!(page.state.dom_commits, commits);
    }

    fn start(source:&str)->(Document,Forms,Page,Outcome) {
        let mut doc=crate::html::parse("<html><head></head><body><p id='target'>Original</p></body></html>");doc.scripting_enabled=true;
        let mut forms=Forms::new(&doc);let mut page=Page::for_test();
        let outcome=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original namespaced creation".into(),source:source.into()}]);(doc,forms,page,outcome)
    }
    #[test]
    fn namespaced_html_creation_preserves_case_owner_identity_and_real_tree() {
        let source=r#"
var ns='http://www.w3.org/1999/xhtml',factory=document.createElementNS;
if(factory!==document.createElementNS||factory.length!==2||!('createElementNS' in document))throw 'method identity';
var lower=factory.call(document,ns,'div'),mixed=factory.bind(document)(ns,'DiV'),upper=factory.apply(document,[ns,'IMG']),image=factory.call(document,ns,'img');
if(lower.localName!=='div'||mixed.localName!=='DiV'||upper.localName!=='IMG'||mixed.tagName!=='DIV'||mixed.nodeName!=='DIV')throw 'case identity';
if(lower.nodeType!==1||lower.ownerDocument!==document||lower.parentNode!==null||lower.prefix!==null||lower.namespaceURI!==ns||!lower.isDefaultNamespace(ns))throw 'owner namespace';
if(upper instanceof Image||!(image instanceof Image))throw 'uppercase resource alias';
lower.id='created';lower.textContent='Namespaced original';document.body.appendChild(lower);
if(document.getElementById('created')!==lower||!lower.isConnected||lower.parentNode!==document.body||lower.textContent!=='Namespaced original')throw 'real tree';
var failed=0;try{new factory(ns,'div');}catch(e){if(e.name==='TypeError')failed++;}if(failed!==1)throw 'constructible method';
document.title='namespaced-ok';
"#;
        let(doc,_,page,result)=start(source);assert_eq!(result.errors,0,"{}",page.report());assert_eq!(doc.title,"namespaced-ok");assert!(!page.has_host_tasks());
    }
    #[test]
    fn namespaced_creation_conversion_reentry_branding_arity_and_typed_validation_order() {
        let source=r#"
var ns='http://www.w3.org/1999/xhtml',order='',factory=document.createElementNS;
var namespace={toString:function(){order+='namespace';var nested=factory.call(document,ns,'span');if(nested.ownerDocument!==document)throw 'nested owner';return ns;}};
var name={toString:function(){order+='name';return 'div';}};
var created=factory.call(document,namespace,name);if(order!=='namespacename'||created.localName!=='div')throw 'conversion order';
order='';var caught=0;
try{factory.call({},namespace,name);}catch(e){if(e.name==='TypeError')caught++;}
try{factory.call(document,namespace);}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2||order!=='')throw 'preconversion validation';
try{factory.call(document,Symbol('namespace'),name);}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3||order!=='')throw 'Symbol conversion order';
try{factory.call(document,namespace,Symbol('name'));}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==4||order!=='namespace')throw 'name Symbol order';
for(var pair of [[null,'p:bad name'],[ns,':div'],[ns,'p:'],[ns,'1invalid']]){try{factory.call(document,pair[0],pair[1]);throw 'accepted invalid name';}catch(e){if(e.name!=='InvalidCharacterError')throw e;}}
for(var pair of [[null,'p:div'],['','p:div'],[ns,'xml:div'],[ns,'xmlns'],['http://www.w3.org/2000/xmlns/','div']]){try{factory.call(document,pair[0],pair[1]);throw 'accepted namespace mismatch';}catch(e){if(e.name!=='NamespaceError')throw e;}}
document.title='conversions-ok';
"#;
        let(doc,_,page,result)=start(source);assert_eq!(result.errors,0,"{}",page.report());assert_eq!(doc.title,"conversions-ok");assert!(!page.has_host_tasks());
    }
    #[test]
    fn namespaced_creation_unsupported_slices_do_not_allocate_or_queue_resources() {
        let(mut doc,mut forms,mut page,result)=start("var ns='http://www.w3.org/1999/xhtml';");assert_eq!(result.errors,0);
        let nodes=doc.nodes.len();let created=page.state.dom.created.clone();
        for operation in ["document.createElementNS(null,'div');","document.createElementNS('','div');","document.createElementNS('http://www.w3.org/2000/svg','script');","document.createElementNS('urn:original','img');","document.createElementNS(ns,'p:div');","document.createElementNS(ns,'div',{});","document.createElementNS(ns,'div',{is:'original-custom'});","document.createElementNS(ns,'div=valid');","document.createElementNS(ns,'élement');"] {
            let result=page.console_eval(&mut doc,&mut forms,operation);assert_eq!(result.errors,1,"{operation}: {}",page.report());assert_eq!(doc.nodes.len(),nodes);assert_eq!(page.state.dom.created,created);assert!(!page.has_host_tasks());
        }
        // '=' is a valid element local-name character after ASCII alpha, even
        // though this factory's spelling slice declines it. Attribute rules
        // cannot replace the actual element-context name/namespace validation.
        assert!(super::super::script_attribute_names::validate_element_namespace(Some("urn:original"),"div=valid").is_ok());
        let err=super::super::script_attribute_names::validate_element_namespace(None,"xml:div=valid").unwrap_err();assert_eq!(err.0,"NamespaceError");
    }
    #[test]
    fn namespaced_creation_small_profiles_rollback_and_names_exceed_old_128_bytes() {
        let(mut doc,mut forms,mut page,result)=start("var ns='http://www.w3.org/1999/xhtml';");assert_eq!(result.errors,0);
        let before=doc.nodes.len();let retained=page.state.retained;let old_nodes=page.state.node_limit;page.state.node_limit=before;
        let result=page.console_eval(&mut doc,&mut forms,"document.createElementNS(ns,'div');");assert_eq!(result.errors,1);assert_eq!(doc.nodes.len(),before);assert_eq!(page.state.retained,retained);
        page.state.node_limit=old_nodes;let old_retained=page.state.retained_limit;page.state.retained_limit=retained+2;
        let result=page.console_eval(&mut doc,&mut forms,"document.createElementNS(ns,'div');");assert_eq!(result.errors,1);assert_eq!(doc.nodes.len(),before);assert_eq!(page.state.retained,retained);
        page.state.retained_limit=old_retained;
        let old_states=page.state.node_state_bytes();let old_core=doc.nodes.capacity();
        let source=format!("var name='{}';var element=document.createElementNS(ns,name);if(element.localName!==name)throw 'name truncated';", "n".repeat(512));
        let result=page.console_eval(&mut doc,&mut forms,&source);assert_eq!(result.errors,0,"{}",page.report());assert_eq!(doc.nodes.len(),before+1);assert_eq!(page.state.retained,retained+512+page.state.node_state_bytes()-old_states+(doc.nodes.capacity()-old_core)*size_of::<Node>());assert!(!page.has_host_tasks());
    }
}
