//! Stable, bounded Attr identities. Expanded names identify entries; qualified
//! names select the first entry for the ordinary HTML attribute APIs.
use super::*;
use crate::html::{AttributeName, AttributeNamespace};

const ATTR_BASE: u32 = 768;
const MAX_ATTRS: usize = 256;

pub(super) struct Attr {
    pub(super) owner: Option<usize>,
    pub(super) name: AttributeName,
    // Attached values stay in the element. Removal moves the actual String here.
    pub(super) value: String,
}

pub(super) fn index(handle: u32) -> Option<usize> {
    (ATTR_BASE..NODE_BASE).contains(&handle).then(|| (handle - ATTR_BASE) as usize)
}

fn property(name: &str) -> bool {
    dom_tree::node_constant(name).is_some() || matches!(name,
        "name" | "localName" | "nodeName" | "namespaceURI" | "prefix" | "value"
        | "nodeValue" | "textContent" | "ownerElement" | "ownerDocument" | "specified"
        | "nodeType" | "parentNode" | "parentElement" | "firstChild" | "lastChild"
        | "nextSibling" | "previousSibling" | "isConnected" | "isDefaultNamespace" | "compareDocumentPosition")
}

fn namespace(args: &[Value]) -> Result<Option<&str>, String> {
    match args.first() {
        Some(Value::Null | Value::Undefined) => Ok(None),
        Some(Value::String(text)) => Ok((!text.is_empty()).then_some(text.as_str())),
        _ => Err("Attribute operation requires a converted nullable namespace.".into()),
    }
}

fn name_text<'a>(name: &'a AttributeName, property: &str) -> Option<&'a str> {
    match property {
        "localName" => Some(name.local()),
        "namespaceURI" => name.namespace_uri(),
        "prefix" => name.prefix(),
        _ => Some(name.qualified.as_str()),
    }
}

impl Bridge<'_> {
    pub(super) fn attr_native_member(&self, name: &str) -> bool {
        property(name) || dom_tree::native_node_member(name)
    }
    pub(super) fn attr_receiver(&self, handle: u32) -> bool {
        index(handle).is_some_and(|id| self.state.attributes.get(id).is_some())
    }

    pub(super) fn attr_readonly(&self, handle: u32, name: &str) -> bool {
        self.attr_receiver(handle) && property(name)
            && !matches!(name, "value" | "nodeValue" | "textContent" | "isDefaultNamespace" | "compareDocumentPosition")
    }

    pub(super) fn attr_has(&self, handle: u32, name: &str) -> Result<bool, String> {
        if !self.attr_receiver(handle) { return Err("Invalid Attr handle.".into()); }
        Ok(property(name) || self.method(handle, name).is_some())
    }

    fn attribute_slot(&mut self, node: usize, namespace: Option<&str>, local: &str) -> Result<Option<usize>, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { return Err("Not an Element.".into()); };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|attr| attr.name.bytes()).sum())?;
        Ok(attrs.iter().position(|attr| attr.name.matches(namespace, local)))
    }

    pub(super) fn raw_attribute_slot(&mut self, node: usize, qualified: &str) -> Result<Option<usize>, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { return Err("Not an Element.".into()); };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|attr| attr.name.qualified.len()).sum())?;
        Ok(attrs.iter().position(|attr| attr.name.qualified == qualified))
    }

    pub(super) fn copy_attribute_name(&mut self, node: usize, slot: usize) -> Result<AttributeName, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { return Err("Not an Element.".into()); };
        let name = &attrs.get(slot).ok_or("Attribute slot is unavailable.")?.name;
        self.state.copies(name.bytes())?;
        Ok(name.copy())
    }

    pub(super) fn attr_get(&mut self, handle: u32, name: &str) -> Result<Value, String> {
        let id = index(handle).ok_or("Invalid Attr handle.")?;
        let attr = self.state.attributes.get(id).ok_or("Invalid Attr handle.")?;
        if let Some(value) = dom_tree::node_constant(name) { return Ok(Value::Number(f64::from(value))); }
        match name {
            "name" | "localName" | "nodeName" | "namespaceURI" | "prefix" => {
                let Some(text) = name_text(&attr.name, name) else { return Ok(Value::Null); };
                if text.len() > self.state.limits.host.string_bytes { return Err("Attr name exceeds the host string bound.".into()); }
                self.state.copies(text.len())?;
                Ok(Value::String(name_text(&self.state.attributes[id].name, name).unwrap().to_owned()))
            }
            "value" | "nodeValue" | "textContent" => {
                let location = if let Some(owner) = attr.owner {
                    let NodeKind::Element { attrs, .. } = &self.doc.nodes[owner].kind else { return Err("Invalid Attr owner.".into()); };
                    self.state.work(attrs.len())?;
                    self.state.copies(attrs.iter().map(|attr| attr.name.bytes()).sum())?;
                    let slot = attrs.iter().position(|entry| entry.name.same_expanded_name(&self.state.attributes[id].name))
                        .ok_or("Attached Attr is missing from its owner.")?;
                    Some((owner, slot))
                } else { None };
                let len = if let Some((owner, slot)) = location {
                    let NodeKind::Element { attrs, .. } = &self.doc.nodes[owner].kind else { unreachable!() };
                    attrs[slot].value.len()
                } else { self.state.attributes[id].value.len() };
                if len > self.state.limits.host.string_bytes { return Err("Attr value exceeds the host string bound.".into()); }
                self.state.copies(len)?;
                let text = if let Some((owner, slot)) = location {
                    let NodeKind::Element { attrs, .. } = &self.doc.nodes[owner].kind else { unreachable!() };
                    &attrs[slot].value
                } else { &self.state.attributes[id].value };
                Ok(Value::String(text.to_owned()))
            }
            "ownerElement" => Ok(attr.owner.map_or(Value::Null, |node| Value::Host(node_handle(node)))),
            "ownerDocument" => Ok(Value::Host(DOCUMENT)),
            "nodeType" => Ok(Value::Number(2.0)),
            "specified" => Ok(Value::Bool(true)),
            "isConnected" => Ok(Value::Bool(false)),
            "parentNode" | "parentElement" | "firstChild" | "lastChild"
            | "previousSibling" | "nextSibling" => Ok(Value::Null),
            "then" | "" => Ok(Value::Undefined),
            _ => Err(format!("Attr property {name:?} is not implemented.")),
        }
    }

    pub(super) fn attr_set(&mut self, handle: u32, property: &str, value: Value) -> Result<(), String> {
        let id = index(handle).filter(|&id| self.state.attributes.get(id).is_some()).ok_or("Invalid Attr handle.")?;
        if !matches!(property, "value" | "nodeValue" | "textContent") {
            return Err("Attr property is read-only or unsupported.".into());
        }
        let value = self.text(&value)?;
        if let Some(owner) = self.state.attributes[id].owner {
            self.state.copies(self.state.attributes[id].name.bytes())?;
            let name = self.state.attributes[id].name.copy();
            self.public_attribute(owner, name, Some(value))?;
        } else {
            self.state.replace_bytes(self.state.attributes[id].value.len(), value.len())?;
            self.state.attributes[id].value = value;
        }
        Ok(())
    }

    fn attr_element_node(&mut self, object: u32) -> Result<usize, String> {
        let node = self.node(object)?;
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html {
            return Err("Attr namespace provenance for foreign parser contexts is unavailable.".into());
        }
        Ok(node)
    }

    fn lookup_attribute_node_at(&mut self, node: usize, slot: Option<usize>) -> Result<Value, String> {
        let Some(slot) = slot else { return Ok(Value::Null); };
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        let name = &attrs[slot].name;
        self.state.work(self.state.attributes.len())?;
        self.state.copies(self.state.attributes.iter().filter(|attr| attr.owner == Some(node)).map(|attr| attr.name.bytes()).sum())?;
        if let Some(id) = self.state.attributes.iter().position(|attr| attr.owner == Some(node) && attr.name.same_expanded_name(name)) {
            return Ok(Value::Host(ATTR_BASE + id as u32));
        }
        if self.state.attributes.len() >= MAX_ATTRS { return Err("Attr identity limit reached (256 per document).".into()); }
        self.state.attributes.try_reserve(1).map_err(|_| "Attr storage reservation failed.")?;
        self.state.copies(name.bytes())?;
        self.state.replace_bytes(0, name.bytes())?;
        let name = name.copy();
        let handle = ATTR_BASE + self.state.attributes.len() as u32;
        self.state.attributes.push(Attr { owner: Some(node), name, value: String::new() });
        Ok(Value::Host(handle))
    }

    pub(super) fn get_attribute_node(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let text = self.text(args.first().ok_or("getAttributeNode requires a name.")?)?;
        self.state.copies(text.len())?;
        let slot = self.raw_attribute_slot(node, &text.to_ascii_lowercase())?;
        self.lookup_attribute_node_at(node, slot)
    }

    pub(super) fn get_attribute_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let ns = namespace(args)?;
        let local = self.text(args.get(1).ok_or("getAttributeNS requires two arguments.")?)?;
        let slot = self.attribute_slot(node, ns, &local)?;
        let Some(slot) = slot else { return Ok(Value::Null); };
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        let text = &attrs[slot].value;
        if text.len() > self.state.limits.host.string_bytes { return Err("Attribute value exceeds the host string bound.".into()); }
        self.state.copies(text.len())?;
        Ok(Value::String(text.to_owned()))
    }

    pub(super) fn get_attribute_node_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let local = self.text(args.get(1).ok_or("getAttributeNodeNS requires two arguments.")?)?;
        let slot = self.attribute_slot(node, namespace(args)?, &local)?;
        self.lookup_attribute_node_at(node, slot)
    }

    pub(super) fn set_attribute_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let ns = namespace(args)?;
        let qualified = self.text(args.get(1).ok_or("setAttributeNS requires three arguments.")?)?;
        let value = self.text(args.get(2).ok_or("setAttributeNS requires three arguments.")?)?;
        super::script_attribute_names::validate_namespace(ns, &qualified).map_err(|(_, message)| message)?;
        let name = if let Some(uri) = ns {
            let (prefix, local) = qualified.split_once(':').map_or((None, qualified.as_str()), |(p, l)| (Some(p), l));
            self.state.copies(uri.len() + local.len() + prefix.map_or(0, str::len))?;
            let namespace = AttributeNamespace { uri: uri.into(), prefix: prefix.map(str::to_owned), local: local.into() };
            AttributeName { qualified, namespace: Some(namespace) }
        } else { AttributeName::unqualified(qualified) };
        self.public_attribute(node, name, Some(value))?;
        Ok(Value::Undefined)
    }

    pub(super) fn remove_attribute_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let local = self.text(args.get(1).ok_or("removeAttributeNS requires two arguments.")?)?;
        if let Some(slot) = self.attribute_slot(node, namespace(args)?, &local)? {
            let name = self.copy_attribute_name(node, slot)?;
            self.public_attribute(node, name, None)?;
        }
        Ok(Value::Undefined)
    }

    pub(super) fn validate_attr_removal(&self, object: u32, args: &[Value]) -> Result<(), (&'static str, String)> {
        let Some(Value::Host(handle)) = args.first() else { return Err(("TypeError", "removeAttributeNode requires an Attr.".into())); };
        let Some(attr) = index(*handle).and_then(|id| self.state.attributes.get(id)) else {
            return Err(("TypeError", "removeAttributeNode requires an Attr.".into()));
        };
        if node_index(object) != attr.owner || attr.owner.is_none() {
            return Err(("NotFoundError", "Attr does not belong to this Element.".into()));
        }
        Ok(())
    }

    pub(super) fn remove_attribute_node(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        self.validate_attr_removal(object, args).map_err(|(_, message)| message)?;
        let Value::Host(handle) = args[0] else { unreachable!() };
        let id = index(handle).unwrap();
        self.state.copies(self.state.attributes[id].name.bytes())?;
        let attr = &self.state.attributes[id];
        let name = attr.name.copy();
        let owner = attr.owner.unwrap();
        self.public_attribute(owner, name, None)?;
        Ok(Value::Host(handle))
    }

    // Resolve a raw QName or expanded name before entering this policy boundary.
    // Only real lower-case null-namespace HTML attributes have native effects.
    pub(super) fn public_attribute(&mut self, node: usize, name: AttributeName, value: Option<String>) -> Result<(), String> {
        if name.namespace.is_some() || name.qualified.contains(':') || name.qualified.bytes().any(|c| c.is_ascii_uppercase()) {
            return self.attribute_named(node, name, value);
        }
        let key = name.qualified.as_str();
        if self.stylesheet_attribute_change(node, key, value.as_deref())? { return Ok(()); }
        if self.dom_script(node) && matches!(key, "src" | "type" | "charset" | "nonce" | "integrity" | "crossorigin" | "referrerpolicy" | "fetchpriority" | "async" | "defer" | "nomodule") {
            let queue = key == "src" && self.state.dom.connected[node];
            self.attribute_named(node, name, value)?;
            if queue { self.queue_dynamic_script(node)?; }
            return Ok(());
        }
        let image_attribute = tag(self.doc, node) == "img" && matches!(key, "src" | "alt" | "width" | "height")
            || self.html_spacing_property(node, key)
            || key == "border" && self.html_border_property(node)
            || key == "ismap" && self.html_image_map_property(node)
            || key == "srcdoc" && self.iframe_srcdoc_property(node);
        if !matches!(key, "id" | "class" | "style" | "title" | "hidden" | "disabled" | "role" | "lang" | "dir")
            && !inert_attribute_name(key) && !image_attribute {
            return Err("Supported attributes are id, class, style, title, hidden, disabled, role, lang, dir, data-* and aria-*; use .value/.checked for live controls.".into());
        }
        if key == "src" && image_attribute {
            if let Some(value) = value { self.image_set(node, "src", &Value::String(value))?; return Ok(()); }
            if !self.state.detached_images.contains(&node) { return Err("Changing a connected image source is not implemented.".into()); }
        }
        if key == "style" {
            if let Some(value) = value.as_ref() { validate_style(value, self.state.limits.host.string_bytes)?; }
        }
        self.attribute_named(node, name, value)
    }

    pub(super) fn attr_default_namespace(&mut self, object: u32) -> Option<Result<Option<&'static str>, String>> {
        let id = index(object)?;
        Some((|| {
            let owner = self.state.attributes.get(id).ok_or("Invalid Attr handle.")?.owner;
            if let Some(owner) = owner { dom_tree::default_namespace(self.doc, owner, None, self.state) }
            else { self.state.work(1)?; Ok(None) }
        })())
    }
}
