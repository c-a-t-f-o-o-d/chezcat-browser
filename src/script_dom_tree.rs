//! Bounded DOM creation, tree mutation and named form lookup. Node identities
//! remain in the document arena until realm teardown. Loaded resources and the
//! native form model are protected from unsupported structural mutations.
//! Semantics: https://dom.spec.whatwg.org/#concept-node-pre-insert
//! https://html.spec.whatwg.org/multipage/forms.html#dom-form-nameditem
use super::*;

pub(super) fn native_node_member(name:&str)->bool {
    node_constant(name).is_some() || "constructor __proto__ nodeType nodeName baseURI isConnected ownerDocument parentNode parentElement childNodes firstChild lastChild previousSibling nextSibling nodeValue textContent getRootNode hasChildNodes normalize cloneNode isEqualNode isSameNode compareDocumentPosition contains lookupPrefix lookupNamespaceURI isDefaultNamespace insertBefore appendChild replaceChild removeChild addEventListener removeEventListener dispatchEvent".split_ascii_whitespace().any(|candidate|candidate==name)
}

// Finite real main-Document Node facts, not a new Document interface family.
pub(super) fn document_node_property(name:&str)->bool {
    matches!(name,"nodeType"|"nodeName"|"isConnected"|"ownerDocument"|"parentNode"|"parentElement"|"firstChild"|"lastChild"|"previousSibling"|"nextSibling")
}

const MAX_DOM_DEPTH: usize = 128;

#[derive(Default)]
pub(super) struct DomState {
    pub(super) created: HashSet<usize>,
    pub(super) connected: Vec<bool>,
    pub(super) pending_focus: Option<usize>,
    pub(super) collections: Vec<dom_collections::TagCollection>,
    pub(super) child_lists: HashMap<usize, u32>,
    pub(super) fragment_scripts: HashSet<usize>,
    force_async: HashSet<usize>,
}

pub(super) fn inert_dom_tag(name: &str, scripting: bool) -> bool {
    matches!(name, "script" | "style" | "template" | "iframe" | "object")
        || name == "noscript" && scripting
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
    pub(super) fn rebuild_dom_tree(&mut self, doc: &Document) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        // Active lookup deliberately excludes inert subtrees; physical parent
        // links and connection state must still describe created script nodes.
        self.work(doc.nodes.len())?;
        let mut tree = Tree {
            active: vec![false; doc.nodes.len()],
            parents: vec![None; doc.nodes.len()],
            ..Tree::default()
        };
        let mut connected = vec![false; doc.nodes.len()];
        let mut stack = if doc.nodes.is_empty() {
            Vec::new()
        } else {
            vec![(0, None, false)]
        };
        while let Some((node, parent, inert)) = stack.pop() {
            self.work(1)?;
            if node >= doc.nodes.len() || connected[node] {
                return Err("DOM tree has an invalid child or repeated parent.".into());
            }
            connected[node] = true;
            tree.parents[node] = parent;
            let name = tag(doc, node);
            let inert = inert || name != "script" && inert_dom_tag(name, doc.scripting_enabled);
            if !inert {
                tree.active[node] = true;
                tree.order.push(node);
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
                if let Some(id) = doc
                    .attr(node, "id")
                    .filter(|id| !id.is_empty() && id.len() <= self.limits.host.string_bytes)
                {
                    self.copies(id.len())?;
                    tree.ids.entry(id.to_owned()).or_insert(node);
                }
            }
            stack.extend(
                doc.nodes[node]
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, Some(node), inert || name == "script")),
            );
        }
        // Saved handles and static NodeLists retain detached ordinary subtrees,
        // whose internal topology is separate from connected document lookup.
        self.work(self.detached_nodes.len())?;
        self.work(
            self.detached_nodes
                .iter()
                .map(|&node| doc.nodes[node].children.len())
                .sum(),
        )?;
        for &parent in &self.detached_nodes {
            for &child in &doc.nodes[parent].children {
                if self.detached_nodes.contains(&child) {
                    tree.parents[child] = Some(parent);
                }
            }
        }
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
    pub(super) fn comment_receiver(&self, object:u32)->bool {
        node_index(object).is_some_and(|node|self.dom_accessible(node)
            && matches!(self.doc.nodes[node].kind,NodeKind::Comment(_)))
    }

    pub(super) fn node_identity_receiver(&self,object:u32)->bool {
        !self.is_frame_handle(object) && self.namespace_receiver(object)
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
    fn unavailable_offset(&self, node: usize, property: &str) -> String {
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

    fn dom_accessible(&self, node: usize) -> bool {
        if node == 0 || node >= self.doc.nodes.len() { return false; }
        if self.state.tree.active.get(node).copied().unwrap_or(false)
                || self.state.detached_nodes.contains(&node)
                || self.state.detached_images.contains(&node)
                || self.state.dom.created.contains(&node) { return true; }
        // A script raw-text child is a retained Text node, not executable
        // markup. Exposing that identity never marks its subtree query-active
        // or changes the guarded script insertion/content mutation policies.
        if matches!(self.doc.nodes[node].kind, NodeKind::Text(_))
            && self.state.dom.connected.get(node)==Some(&true)
            && self.state.tree.parents.get(node).copied().flatten().is_some_and(|parent| tag(self.doc,parent)=="script") {
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
            if name == "script" && current != node
                || name != "script" && inert_dom_tag(name, self.doc.scripting_enabled) { return false; }
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
        tag(self.doc, node) == "link" && self.state.dom.created.contains(&node)
    }

    /// Created, detached STYLE metadata only. Parsed/admitted styles and
    /// stylesheet insertion/content retain their existing separate guards.
    pub(super) fn style_type_receiver(&self,object:u32)->bool {
        node_index(object).is_some_and(|node|self.state.dom.created.contains(&node)
            && self.state.dom.connected.get(node)==Some(&false)
            && self.doc.nodes.get(node).is_some_and(|n|n.namespace==crate::html::Namespace::Html
                && matches!(&n.kind,NodeKind::Element {tag,..} if tag=="style")))
    }

    pub(super) fn dom_script(&self, node: usize) -> bool {
        tag(self.doc, node) == "script" && self.state.dom.created.contains(&node)
    }

    pub(super) fn dom_method(&self, object: u32, property: &str) -> Option<&'static str> {
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
        if matches!(property,"localName"|"prefix"|"namespaceURI") {
            return self.attr_element_receiver(object) && node_index(object).is_some_and(|node|
                self.doc.nodes[node].namespace==crate::html::Namespace::Html);
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
        if object==DOCUMENT && method=="createElementNS" {
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
        if object == DOCUMENT && method == "createElement" {
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

    pub(super) fn dom_has(&mut self, object: u32, property: &str) -> Result<Option<bool>, String> {
        if property=="childNodes" && node_index(object).is_some() {self.dom_node(object)?;return Ok(Some(true));}
        if property=="controls"&&self.media_interface_receiver(object) {self.node(object)?;return Ok(Some(true));}
        if self.media_element_receiver(object) {if let Some(value)=self.media_element_has(object,property)? {return Ok(Some(value));}}
        let Some(node) = node_index(object) else {
            return Ok(None);
        };
        let node = self.dom_node(node_handle(node))?;
        if property=="type"&&self.style_type_receiver(object){return Ok(Some(true));}
        if property=="src"&&tag(self.doc,node)=="script"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return Ok(Some(true));}
        if property=="href"&&tag(self.doc,node)=="link"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return Ok(Some(true));}
        if matches!(property,"dir"|"lang"|"align"|"useMap") {
            if self.doc.nodes[node].namespace==crate::html::Namespace::UnknownForeign {return Err("HTML reflection requires a known namespace context.".into());}
            return Ok(Some(self.html_text_reflection(node,property).is_some()));
        }
        if property == "getBoundingClientRect" { return Ok(Some(self.class_list_element_receiver(object))); }
        if property == "attributes" {
            if !matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) { return Ok(Some(false)); }
            if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("Attribute map requires known namespace provenance.".into()); }
            return Ok(Some(true));
        }
        if matches!(property,"localName"|"prefix"|"namespaceURI") {
            if matches!(self.doc.nodes[node].kind,NodeKind::Element {..}) && self.doc.nodes[node].namespace!=crate::html::Namespace::Html {
                return Err("Element naming requires known namespace provenance.".into());
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
        if property == "innerHTML" && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) { return Ok(Some(true)); }
        if common_node_property(property) || self.dom_method(object, property).is_some() {
            return Ok(Some(true));
        }
        if matches!(self.doc.nodes[node].kind, NodeKind::Text(_) | NodeKind::Comment(_)) {
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
        if property=="childNodes" && node_index(object).is_some() {return self.create_child_list(object).map(Some);}
        if property=="controls"&&self.media_interface_receiver(object) {return self.media_element_get(object,property);}
        if self.media_element_receiver(object) {if let Some(value)=self.media_element_get(object,property)? {return Ok(Some(value));}}
        if node_index(object).is_none() {
            return Ok(None);
        }
        let node = self.dom_node(object)?;
        if property=="type"&&self.style_type_receiver(object){
            let raw=self.doc.attr(node,"type").unwrap_or("");
            if raw.len()>self.state.limits.host.string_bytes{return Err("Style type result exceeds the configured string allowance.".into());}
            self.state.copies(raw.len())?;return Ok(Some(Value::String(raw.to_owned())));
        }
        if property=="src"&&tag(self.doc,node)=="script"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return self.image_source(node).map(Some);}
        if property=="href"&&tag(self.doc,node)=="link"&&self.doc.nodes[node].namespace==crate::html::Namespace::Html {return self.reflected_url(node,"href").map(Some);}
        if matches!(property,"dir"|"lang"|"align"|"useMap") {return self.html_text_reflection_get(node,property).map(Some);}
        if property == "innerHTML" && matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) { return self.inner_html(node).map(Some); }
        if property == "attributes" {
            if !matches!(self.doc.nodes[node].kind, NodeKind::Element { .. }) { return Ok(Some(Value::Undefined)); }
            if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("Attribute map requires known namespace provenance.".into()); }
            return Ok(Some(Value::Host(script_named_map::handle(object))));
        }
        if matches!(property,"localName"|"prefix"|"namespaceURI") {
            if !matches!(self.doc.nodes[node].kind,NodeKind::Element {..}) {return Ok(Some(Value::Undefined));}
            if self.doc.nodes[node].namespace!=crate::html::Namespace::Html {return Err("Element naming requires known namespace provenance.".into());}
            return match property {
                "prefix"=>Ok(Some(Value::Null)),
                "namespaceURI"=>self.string("http://www.w3.org/1999/xhtml").map(Some),
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
                    return geometry.read(self.doc,self.forms,node,root,property,&mut self.state.host_work).map(|value|Some(Value::Number(value)));
                }
            }
            return Err(self.unavailable_scroll(node,property));
        }
        if property == "classList" {
            if !matches!(self.doc.nodes[node].kind,NodeKind::Element {..}) { return Ok(Some(Value::Undefined)); }
            if self.doc.nodes[node].namespace != crate::html::Namespace::Html { return Err("Class token list requires known namespace provenance.".into()); }
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
        if matches!(property, "offsetLeft" | "offsetTop") {
            if !matches!(self.doc.nodes[node].kind, NodeKind::Element { .. })
                || self.doc.nodes[node].namespace != crate::html::Namespace::Html
            { return Err("Offset lookup requires a known HTML Element.".into()); }
            if tag(self.doc, node) == "body" || !self.state.dom.connected[node] {
                return Ok(Some(Value::Number(0.0)));
            }
            return Err(self.unavailable_offset(node, property));
        }
        let text = matches!(self.doc.nodes[node].kind, NodeKind::Text(_));
        let comment = matches!(self.doc.nodes[node].kind, NodeKind::Comment(_));
        if let Some(value) = self.frame_content(node, property) { return Ok(Some(value)); }
        if common_node_property(property) {
            return Ok(Some(match property {
                "nodeType" => Value::Number(if text { 3.0 } else if comment { 8.0 } else { 1.0 }),
                "nodeName" => {
                    let name = if text {
                        "#text".into()
                    } else if comment {
                        "#comment".into()
                    } else {
                        tag(self.doc, node).to_ascii_uppercase()
                    };
                    return self.string(&name).map(Some);
                }
                "ownerDocument" => Value::Host(DOCUMENT),
                "isConnected" => Value::Bool(self.state.dom.connected[node]),
                "parentNode" => return self.element_parent(node, true).map(Some),
                "parentElement" => return self.element_parent(node, false).map(Some),
                "firstChild" | "lastChild" => {
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
                        self.state.work(self.doc.nodes[parent].children.len())?;
                        let siblings = &self.doc.nodes[parent].children;
                        if let Some(index) = siblings.iter().position(|&child| child == node) {
                            found = if property == "nextSibling" {
                                siblings[index + 1..]
                                    .iter()
                                    .copied()
                                    .find(|&child| self.dom_accessible(child))
                            } else {
                                siblings[..index]
                                    .iter()
                                    .rev()
                                    .copied()
                                    .find(|&child| self.dom_accessible(child))
                            };
                        }
                    }
                    found.map_or(Value::Null, |child| Value::Host(node_handle(child)))
                }
            }));
        }
        if let NodeKind::Text(value) | NodeKind::Comment(value) = &self.doc.nodes[node].kind {
            return Ok(Some(match property {
                "data" | "nodeValue" | "textContent" => {
                    if value.len() > self.state.limits.host.string_bytes {
                        return Err("DOM result exceeds 32 KiB.".into());
                    }
                    self.state.copies(value.len())?;
                    Value::String(value.clone())
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
        if tag(self.doc, node) == "script" && matches!(property, "text" | "textContent") {
            return self
                .dom_script_text(node)
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

    fn create_dom_node(&mut self, kind: NodeKind) -> Result<Value, String> {
        self.state.rebuild(self.doc)?;
        if self.doc.nodes.len().saturating_add(self.state.blank_frame_nodes) >= self.state.node_limit {
            return Err("DOM appended-node limit reached (256 per document).".into());
        }
        let bytes = match &kind {
            NodeKind::Element { tag, .. } => tag.len(),
            NodeKind::Text(text) | NodeKind::Comment(text) => text.len(),
        };
        let node = self.doc.nodes.len();
        let handle = u32::try_from(node).ok().and_then(|index|NODE_BASE.checked_add(index))
            .filter(|handle|*handle<STYLE_BASE).ok_or("DOM node identity space exhausted.")?;
        // Reserve every owner container before bytes or a native identity are published.
        self.doc.nodes.try_reserve(1).map_err(|_|"DOM node storage reservation failed.")?;
        self.state.dom.created.try_reserve(1).map_err(|_|"DOM created identity reservation failed.")?;
        self.state.detached_nodes.try_reserve(1).map_err(|_|"DOM detached identity reservation failed.")?;
        self.state.tree.active.try_reserve(1).map_err(|_|"DOM active-state reservation failed.")?;
        self.state.tree.parents.try_reserve(1).map_err(|_|"DOM parent-state reservation failed.")?;
        self.state.dom.connected.try_reserve(1).map_err(|_|"DOM connection-state reservation failed.")?;
        if matches!(&kind,NodeKind::Element{tag,..} if tag=="script") {
            self.state.dom.force_async.try_reserve(1).map_err(|_|"DOM script-state reservation failed.")?;
        }
        self.state.replace_bytes(0, bytes)?;
        if matches!(&kind, NodeKind::Element { tag, .. } if tag == "script") {
            self.state.dom.force_async.insert(node);
        }
        self.doc.nodes.push(Node {
            namespace: if matches!(kind, NodeKind::Element { .. }) {
                crate::html::Namespace::Html
            } else { crate::html::Namespace::None },
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

    pub(super) fn mutation_subtree(&mut self, root: usize) -> Result<(Vec<usize>, usize), String> {
        let mut nodes = Vec::new();
        let mut stack = vec![(root, 1usize)];
        let mut depth = 0;
        while let Some((node, level)) = stack.pop() {
            self.state.work(1)?;
            depth = depth.max(level);
            let name = tag(self.doc, node);
            let select_branch = self.owned_select_member(root, node);
            let owned_select = matches!(name, "select" | "option" | "optgroup") && select_branch;
            if select_branch && matches!(&self.doc.nodes[node].kind, NodeKind::Element { .. })
                && !matches!(name, "select" | "option" | "optgroup") {
                return Err("Moving nonconventional select descendants or resources is not implemented.".into());
            }
            if forms::kind(self.doc, node).is_some() && !owned_select
                || matches!(
                    name,
                    "html"
                        | "head"
                        | "body"
                        | "form"
                        | "fieldset"
                        | "legend"
                        | "style"
                        | "title"
                        | "object"
                        | "embed"
                        | "base"
                        | "meta"
                        | "template"
                        | "video" | "audio" | "source"
                )
                || matches!(name, "option" | "optgroup") && !owned_select
                || name == "noscript" && self.doc.scripting_enabled
                || name == "script" && !self.dom_script(node)
                || name == "link" && !self.dom_stylesheet(node)
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
        if !matches!(self.doc.nodes[parent].kind, NodeKind::Element { .. }) {
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
        let old = self.dom_node(outgoing)?;
        if child == old { return Ok(Value::Host(outgoing)); }
        self.mutation_ancestors(parent)?;
        let source_parent = self.state.tree.parents[child];
        if let Some(source) = source_parent { self.mutation_ancestors(source)?; }
        let (incoming_nodes, depth) = self.mutation_subtree(child)?;
        let (outgoing_nodes, _) = self.mutation_subtree(old)?;
        if self.doc.nodes[parent].namespace != crate::html::Namespace::Html
            || matches!(tag(self.doc, parent), "script" | "link" | "iframe")
            || incoming_nodes.iter().chain(&outgoing_nodes).any(|&node|
                self.doc.nodes[node].namespace == crate::html::Namespace::UnknownForeign
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
        children.try_reserve(self.doc.nodes[parent].children.len()).map_err(|_| "DOM replacement children reservation failed.")?;
        for &node in &self.doc.nodes[parent].children {
            if node == child { continue; }
            children.push(if node == old { child } else { node });
        }
        let mut source_children = Vec::new();
        if let Some(source) = source_parent.filter(|&source| source != parent) {
            source_children.try_reserve(self.doc.nodes[source].children.len()).map_err(|_| "DOM replacement source reservation failed.")?;
            source_children.extend(self.doc.nodes[source].children.iter().copied().filter(|&node| node != child));
        }
        let mut incoming_set = HashSet::new();
        incoming_set.try_reserve(incoming_nodes.len()).map_err(|_| "DOM replacement topology reservation failed.")?;
        incoming_set.extend(incoming_nodes.iter().copied());
        let detach_count = incoming_nodes.len().checked_add(outgoing_nodes.len()).ok_or("DOM replacement detached count overflow.")?;
        self.state.detached_nodes.try_reserve(detach_count).map_err(|_| "DOM replacement detached identity reservation failed.")?;
        let connected = self.state.dom.connected[parent];
        let changed_layout = connected || self.state.dom.connected[child];
        let images = self.prepare_image_connections(&incoming_nodes, connected)?;

        let form_plan = self.prepare_select_form_change(None, &[(old, None), (child, Some(parent))])?;
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
        self.commit_image_connections(images);
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
        if !matches!(self.doc.nodes[parent].kind, NodeKind::Element { .. }) {
            return Err("HierarchyRequestError: Text nodes cannot contain children.".into());
        }
        let Some(Value::Host(handle)) = args.first() else {
            return Err("A DOM node is required.".into());
        };
        let child = self.dom_node(*handle)?;
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
        self.mutation_ancestors(parent)?;
        let old_parent = self.state.tree.parents[child];
        if let Some(old_parent) = old_parent {
            self.mutation_ancestors(old_parent)?;
        }
        let (descendants, depth) = self.mutation_subtree(child).map_err(|error| format!(
            "{error} Operation={method}; parent={parent}<{:?}>; child={child}<{:?}>; old_parent={old_parent:?}; reference={reference:?}.",
            prefix(tag(self.doc, parent), 64), prefix(tag(self.doc, child), 64)))?;
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
        let image_plans = self.prepare_image_connections(&descendants, connected)?;
        let frames = self.prepare_blank_frames(&descendants, connected, parent)?;
        let form_plan = self.prepare_select_form_change(None, &[(child, if method == "removeChild" { None } else { Some(parent) })])?;
        if method != "removeChild" { self.doc.nodes[parent].children.try_reserve(1).map_err(|_| "DOM destination children reservation failed.")?; }
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

    fn dom_script_text(&mut self, node: usize) -> Result<String, String> {
        let mut text = String::new();
        for &child in &self.doc.nodes[node].children {
            self.state.work(1)?;
            if let NodeKind::Text(value) = &self.doc.nodes[child].kind {
                if text.len().saturating_add(value.len()) > self.state.limits.host.string_bytes {
                    return Err("Script text exceeds 32 KiB.".into());
                }
                self.state.copies(value.len())?;
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
        let new_child = if text.is_empty() {
            None
        } else {
            let Value::Host(handle) = self.create_dom_node(NodeKind::Text(text))? else {
                unreachable!();
            };
            node_index(handle)
        };
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
        if node_index(object).is_none() {
            return Ok(false);
        }
        let node = self.dom_node(object)?;
        if let Some(attribute)=self.html_text_reflection(node,property) {
            let value=self.text(value)?;self.attribute(node,attribute,Some(value))?;return Ok(true);
        }
        if let NodeKind::Text(old) | NodeKind::Comment(old) = &self.doc.nodes[node].kind {
            if !matches!(property, "data" | "nodeValue" | "textContent") {
                return Ok(false);
            }
            let old_len = old.len();
            let comment = matches!(self.doc.nodes[node].kind,NodeKind::Comment(_));
            self.mutation_ancestors(node)?;
            let text = if matches!(value, Value::Null) && property != "data" {
                String::new()
            } else {
                self.text(value)?
            };
            self.state.replace_bytes(old_len, text.len())?;
            self.doc.nodes[node].kind = if comment {NodeKind::Comment(text)} else {NodeKind::Text(text)};
            self.state.dom_committed();
            if self.state.dom.connected[node] && !comment {
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
// HTML elements have a null prefix and an immutable HTML namespace, regardless
// of raw xmlns attributes. Unknown parser contexts must fail explicitly.
pub(super) fn default_namespace(
    doc: &Document, node: usize, parent: Option<usize>, state: &mut State,
) -> Result<Option<&'static str>, String> {
    state.work(1)?;
    let item = doc.nodes.get(node).ok_or("Invalid namespace node.")?;
    let element = if node == 0 {
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
        if !matches!(item.kind, NodeKind::Element { .. }) {
            return Err("Namespace parent is not an Element.".into());
        }
        Some(item)
    } else { None };
    match element.map(|item| item.namespace) {
        None => Ok(None),
        Some(crate::html::Namespace::Html) => Ok(Some("http://www.w3.org/1999/xhtml")),
        Some(_) => Err("Namespace lookup for unsupported foreign parser context is unavailable.".into()),
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
        let source=format!("var name='{}';var element=document.createElementNS(ns,name);if(element.localName!==name)throw 'name truncated';", "n".repeat(512));
        let result=page.console_eval(&mut doc,&mut forms,&source);assert_eq!(result.errors,0,"{}",page.report());assert_eq!(doc.nodes.len(),before+1);assert_eq!(page.state.retained,retained+512);assert!(!page.has_host_tasks());
    }
}
