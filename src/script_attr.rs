//! Stable, bounded Attr identities for real HTML attributes. No synthetic names
//! or duck-typed attribute objects can acquire mutation authority.
use super::*;

const ATTR_BASE: u32 = 768;
const MAX_ATTRS: usize = 256;

pub(super) struct Attr {
    pub(super) owner: Option<usize>,
    pub(super) name: String,
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

impl Bridge<'_> {
    pub(super) fn attr_receiver(&self, handle: u32) -> bool {
        index(handle).is_some_and(|id| self.state.attributes.get(id).is_some())
    }

    pub(super) fn attr_readonly(&self, handle: u32, name: &str) -> bool {
        self.attr_receiver(handle) && property(name)
            && !matches!(name, "value" | "nodeValue" | "textContent" | "isDefaultNamespace" | "compareDocumentPosition")
    }

    pub(super) fn attr_has(&self, handle: u32, name: &str) -> Result<bool, String> {
        if !self.attr_receiver(handle) { return Err("Invalid Attr handle.".into()); }
        Ok(property(name))
    }

    pub(super) fn attr_get(&mut self, handle: u32, name: &str) -> Result<Value, String> {
        let id = index(handle).ok_or("Invalid Attr handle.")?;
        let attr = self.state.attributes.get(id)
            .ok_or("Invalid Attr handle.")?;
        if let Some(value) = dom_tree::node_constant(name) { return Ok(Value::Number(f64::from(value))); }
        match name {
            "name" | "localName" | "nodeName" => {
                let len = attr.name.len();
                if len > self.state.limits.host.string_bytes { return Err("Attr name exceeds 32 KiB.".into()); }
                self.state.copies(len)?;
                Ok(Value::String(self.state.attributes[id].name.clone()))
            }
            "value" | "nodeValue" | "textContent" => {
                let location = if let Some(owner) = attr.owner {
                    let NodeKind::Element { attrs, .. } = &self.doc.nodes[owner].kind else { return Err("Invalid Attr owner.".into()); };
                    self.state.work(attrs.len())?;
                    self.state.copies(attrs.iter().map(|(name, _)| name.len()).sum())?;
                    let slot = attrs.iter().position(|(name, _)| name == &self.state.attributes[id].name)
                        .ok_or("Attached Attr is missing from its owner.")?;
                    Some((owner, slot))
                } else { None };
                let len = if let Some((owner, slot)) = location {
                    let NodeKind::Element { attrs, .. } = &self.doc.nodes[owner].kind else { unreachable!() };
                    attrs[slot].1.len()
                } else { self.state.attributes[id].value.len() };
                if len > self.state.limits.host.string_bytes { return Err("Attr value exceeds 32 KiB.".into()); }
                self.state.copies(len)?;
                let value = if let Some((owner, slot)) = location {
                    let NodeKind::Element { attrs, .. } = &self.doc.nodes[owner].kind else { unreachable!() };
                    &attrs[slot].1
                } else { &self.state.attributes[id].value };
                Ok(Value::String(value.to_owned()))
            }
            "ownerElement" => Ok(attr.owner.map_or(Value::Null, |node| Value::Host(node_handle(node)))),
            "ownerDocument" => Ok(Value::Host(DOCUMENT)),
            "nodeType" => Ok(Value::Number(2.0)),
            "specified" => Ok(Value::Bool(true)),
            "isConnected" => Ok(Value::Bool(false)),
            "namespaceURI" | "prefix" | "parentNode" | "parentElement" | "firstChild"
            | "lastChild" | "previousSibling" | "nextSibling" => Ok(Value::Null),
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
        let owner = self.state.attributes[id].owner;
        if let Some(owner) = owner {
            self.state.copies(self.state.attributes[id].name.len())?;
            let name = self.state.attributes[id].name.clone();
            // All existing public mutation policy, including resource, control,
            // script and stylesheet restrictions, remains on this same path.
            self.call(node_handle(owner), "setAttribute", &[Value::String(name), Value::String(value)])?;
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

    fn lookup_attribute_node(&mut self, node: usize, name: String) -> Result<Value, String> {
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|(name, _)| name.len()).sum())?;
        if !attrs.iter().any(|(key, _)| key == &name) { return Ok(Value::Null); }
        self.state.work(self.state.attributes.len())?;
        self.state.copies(self.state.attributes.iter().filter(|attr| attr.owner == Some(node)).map(|attr| attr.name.len()).sum())?;
        if let Some(id) = self.state.attributes.iter().position(|attr| attr.owner == Some(node) && attr.name == name) {
            return Ok(Value::Host(ATTR_BASE + id as u32));
        }
        if self.state.attributes.len() >= MAX_ATTRS { return Err("Attr identity limit reached (256 per document).".into()); }
        self.state.attributes.try_reserve(1).map_err(|_| "Attr storage reservation failed.")?;
        self.state.replace_bytes(0, name.len())?;
        let handle = ATTR_BASE + self.state.attributes.len() as u32;
        self.state.attributes.push(Attr { owner: Some(node), name, value: String::new() });
        Ok(Value::Host(handle))
    }

    pub(super) fn get_attribute_node(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let text = self.text(args.first().ok_or("getAttributeNode requires a name.")?)?;
        self.state.copies(text.len())?;
        self.lookup_attribute_node(node, text.to_ascii_lowercase())
    }

    pub(super) fn get_attribute_node_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let namespace = match args.first() {
            Some(Value::Null | Value::Undefined) => None,
            Some(Value::String(text)) => if text.is_empty() { None } else { Some(text.as_str()) },
            _ => return Err("getAttributeNodeNS requires a converted nullable namespace.".into()),
        };
        let name = self.text(args.get(1).ok_or("getAttributeNodeNS requires two arguments.")?)?;
        // Every attribute produced by this supported HTML parser/mutation model
        // has null namespace and no prefix. Foreign parser contexts were rejected
        // above; namespace-aware factories/attachment remain unavailable.
        if namespace.is_some() { return Ok(Value::Null); }
        self.lookup_attribute_node(node, name)
    }

    pub(super) fn remove_attribute_ns(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let node = self.attr_element_node(object)?;
        let namespace = match args.first() {
            Some(Value::Null | Value::Undefined) => None,
            Some(Value::String(text)) => if text.is_empty() { None } else { Some(text.as_str()) },
            _ => return Err("removeAttributeNS requires a converted nullable namespace.".into()),
        };
        let name = self.text(args.get(1).ok_or("removeAttributeNS requires two arguments.")?)?;
        // Exact local-name matching: unlike removeAttribute this never folds
        // ASCII case, interprets colons, or creates an Attr identity on a miss.
        // The supported HTML model has only unqualified attributes.
        if namespace.is_some() { return Ok(Value::Undefined); }
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        self.state.work(attrs.len())?;
        self.state.copies(attrs.iter().map(|(key, _)|key.len()).sum())?;
        if !attrs.iter().any(|(key,_)|key == &name) { return Ok(Value::Undefined); }
        // Reuse every existing mutation/security check and Attr detachment path.
        self.call(object,"removeAttribute",&[Value::String(name)])
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
        self.state.copies(self.state.attributes[index(handle).unwrap()].name.len())?;
        let name = self.state.attributes[index(handle).unwrap()].name.clone();
        self.call(object, "removeAttribute", &[Value::String(name)])?;
        Ok(Value::Host(handle))
    }

    pub(super) fn attr_default_namespace(&mut self, object: u32) -> Option<Result<Option<&'static str>, String>> {
        let id = index(object)?;
        Some((|| {
            let owner = self.state.attributes.get(id).ok_or("Invalid Attr handle.")?.owner;
            if let Some(owner) = owner {
                dom_tree::default_namespace(self.doc, owner, None, self.state)
            } else { self.state.work(1)?; Ok(None) }
        })())
    }
}
