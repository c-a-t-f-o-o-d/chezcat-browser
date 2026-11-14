//! Stable, bounded Attr identities. Expanded names identify entries; qualified
//! names select the first entry for the ordinary HTML attribute APIs.
use super::*;
use crate::html::{AttributeName, AttributeNamespace};

pub(super) const ATTR_BASE: u32 = 0x8200_0000;
const MAX_ATTRS: usize = 65_536;

pub(super) struct Attr {
    pub(super) owner: Option<usize>,
    pub(super) name: AttributeName,
    // Attached values stay in the element. Removal moves the actual String here.
    pub(super) value: String,
}

pub(super) fn index(handle: u32) -> Option<usize> {
    (ATTR_BASE..ATTR_BASE + MAX_ATTRS as u32).contains(&handle).then(|| (handle - ATTR_BASE) as usize)
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
    pub(super) fn create_attribute(&mut self,args:&[Value],namespaced:bool)->Result<Value,String> {
        let qualified=self.text(args.get(usize::from(namespaced)).ok_or("Attribute creation requires a name.")?)?;
        let name=if namespaced {
            let ns=namespace(args)?;
            super::script_attribute_names::validate_namespace(ns,&qualified).map_err(|(_,message)|message)?;
            if let Some(uri)=ns {
                let (prefix,local)=qualified.split_once(':').map_or((None,qualified.as_str()),|(prefix,local)|(Some(prefix),local));
                self.state.copies(uri.len().checked_add(qualified.len()).ok_or("Attribute name accounting overflow.")?)?;
                let namespace=AttributeNamespace{uri:uri.into(),prefix:prefix.map(str::to_owned),local:local.into()};
                AttributeName{qualified,namespace:Some(namespace)}
            } else {AttributeName::unqualified(qualified)}
        } else {AttributeName::unqualified(qualified.to_ascii_lowercase())};
        let limit=self.state.limits.host.attribute_identities.min(MAX_ATTRS);
        if self.state.attributes.len()>=limit {return Err(format!("Attr identity limit reached ({limit} per document)."));}
        self.state.attributes.try_reserve(1).map_err(|_|"Attr storage reservation failed.")?;
        self.state.replace_bytes(0,name.bytes())?;
        let handle=ATTR_BASE+self.state.attributes.len() as u32;
        self.state.attributes.push(Attr{owner:None,name,value:String::new()});self.state.dom_committed();
        Ok(Value::Host(handle))
    }

    pub(super) fn validate_attr_attachment(&self,object:u32,args:&[Value])->Result<(),(&'static str,String)> {
        let Some(Value::Host(handle))=args.first() else {return Err(("TypeError","setAttributeNode requires a genuine Attr.".into()));};
        let Some(attr)=index(*handle).and_then(|id|self.state.attributes.get(id)) else {return Err(("TypeError","setAttributeNode requires a genuine Attr.".into()));};
        if attr.owner.is_some() && attr.owner!=node_index(object) {return Err(("InUseAttributeError","Attr already belongs to another Element.".into()));}
        Ok(())
    }

    /// The element value/effects commit once through the existing checked path.
    /// All identity/name/value staging precedes it; nothing fallible follows it.
    pub(super) fn set_attribute_node(&mut self,object:u32,args:&[Value])->Result<Value,String> {
        self.validate_attr_attachment(object,args).map_err(|(_,message)|message)?;
        let node=self.attr_element_node(object)?;
        let Value::Host(handle)=args[0] else {unreachable!()};let incoming=index(handle).unwrap();
        if self.state.attributes[incoming].owner==Some(node) {return Ok(Value::Host(handle));}
        let attr=&self.state.attributes[incoming];
        let active_name=attr.name.namespace.is_none() && !attr.name.qualified.contains(':');
        if active_name && (self.dom_script(node) && matches!(attr.name.qualified.as_str(),"src"|"type"|"charset"|"nonce"|"integrity"|"crossorigin"|"referrerpolicy"|"fetchpriority"|"async"|"defer"|"nomodule")
            || self.dom_stylesheet(node) && matches!(attr.name.qualified.as_str(),"href"|"rel"|"type"|"media"|"as"|"nonce"|"integrity"|"crossorigin"|"referrerpolicy"|"fetchpriority"|"disabled")) {
            return Err("Attribute-node adoption for active script/stylesheet settings is not implemented; no attribute was changed.".into());
        }
        let name_bytes=attr.name.bytes();let value_bytes=attr.value.len();
        self.state.copies(name_bytes.checked_mul(2).and_then(|n|n.checked_add(value_bytes)).ok_or("Attribute copy accounting overflow.")?)?;
        let element_name=self.state.attributes[incoming].name.copy();let write_name=element_name.copy();let value=self.state.attributes[incoming].value.clone();
        let slot=self.attribute_slot(node,element_name.namespace_uri(),element_name.local())?;
        self.state.work(self.state.attributes.len())?;
        let old_id=self.state.attributes.iter().position(|attr|attr.owner==Some(node)&&attr.name.same_expanded_name(&element_name));
        let (old_name,old_value)=if let Some(slot)=slot {
            let NodeKind::Element{attrs,..}=&self.doc.nodes[node].kind else {unreachable!()};let old=&attrs[slot];
            self.state.copies(old.name.bytes().checked_add(old.value.len()).ok_or("Attribute copy accounting overflow.")?)?;
            (Some(old.name.copy()),old.value.clone())
        } else {(None,String::new())};
        let new_old_identity=slot.is_some()&&old_id.is_none();
        if new_old_identity {
            let limit=self.state.limits.host.attribute_identities.min(MAX_ATTRS);
            if self.state.attributes.len()>=limit {return Err(format!("Attr identity limit reached ({limit} per document)."));}
            self.state.attributes.try_reserve(1).map_err(|_|"Attr storage reservation failed.")?;
        }
        let old_name_bytes=old_name.as_ref().map_or(0,AttributeName::bytes);
        let removed=value_bytes.checked_add(old_name_bytes).ok_or("Attribute accounting overflow.")?;
        let added=old_value.len().checked_add(if slot.is_some(){name_bytes}else{0}).and_then(|n|n.checked_add(if new_old_identity{old_name_bytes}else{0})).ok_or("Attribute accounting overflow.")?;
        let retained_before=self.state.retained;
        let factor=if element_name.html_is("id"){2}else{1};
        let public_old=if slot.is_some(){old_value.len().checked_mul(factor).and_then(|n|n.checked_add(old_name_bytes)).ok_or("Attribute accounting overflow.")?}else{0};
        let public_new=value_bytes.checked_mul(factor).and_then(|n|n.checked_add(if slot.is_some(){old_name_bytes}else{name_bytes})).ok_or("Attribute accounting overflow.")?;
        let total_removed=removed.checked_add(public_old).ok_or("Attribute accounting overflow.")?;
        let total_added=added.checked_add(public_new).ok_or("Attribute accounting overflow.")?;
        let retained_after=if total_added>=total_removed {retained_before.checked_add(total_added-total_removed)}else{retained_before.checked_sub(total_removed-total_added)}.ok_or("Attribute retained accounting overflow/underflow.")?;
        if retained_after>self.state.retained_limit {return Err("DOM retained attribute growth allowance reached.".into());}
        self.state.copies(added)?;
        // Credit transferred payload before a growing write; for a shrinking
        // write, defer positive identity accounting until its successful commit.
        if removed>added {self.state.retained=retained_before.checked_sub(removed-added).ok_or("Attribute retained accounting underflow.")?;}
        if let Err(error)=self.public_attribute(node,write_name,Some(value)) {self.state.retained=retained_before;return Err(error);}
        self.state.retained=retained_after;
        if let Some(slot)=slot {let NodeKind::Element{attrs,..}=&mut self.doc.nodes[node].kind else {unreachable!()};attrs[slot].name=element_name;}
        let result=if let Some(old_id)=old_id {
            self.state.attributes[old_id].owner=None;self.state.attributes[old_id].value=old_value;
            Value::Host(ATTR_BASE+old_id as u32)
        } else if let Some(old_name)=old_name {
            let handle=ATTR_BASE+self.state.attributes.len() as u32;
            self.state.attributes.push(Attr{owner:None,name:old_name,value:old_value});Value::Host(handle)
        } else {Value::Null};
        self.state.attributes[incoming].owner=Some(node);self.state.attributes[incoming].value.clear();
        Ok(result)
    }
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
            self.state.dom_committed();
        }
        Ok(())
    }

    pub(super) fn attr_element_node(&mut self, object: u32) -> Result<usize, String> {
        let node = self.node(object)?;
        if self.doc.nodes[node].namespace != crate::html::Namespace::Html {
            return Err("Attr namespace provenance for foreign parser contexts is unavailable.".into());
        }
        Ok(node)
    }

    pub(super) fn lookup_attribute_node_at(&mut self, node: usize, slot: Option<usize>) -> Result<Value, String> {
        let Some(slot) = slot else { return Ok(Value::Null); };
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        let name = &attrs[slot].name;
        self.state.work(self.state.attributes.len())?;
        self.state.copies(self.state.attributes.iter().filter(|attr| attr.owner == Some(node)).map(|attr| attr.name.bytes()).sum())?;
        if let Some(id) = self.state.attributes.iter().position(|attr| attr.owner == Some(node) && attr.name.same_expanded_name(name)) {
            return Ok(Value::Host(ATTR_BASE + id as u32));
        }
        let limit = self.state.limits.host.attribute_identities.min(MAX_ATTRS);
        if self.state.attributes.len() >= limit { return Err(format!("Attr identity limit reached ({limit} per document).")); }
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
        let media_source=key=="src"&&self.media_element_receiver(node_handle(node));
        let style_type=key=="type"&&self.style_type_receiver(node_handle(node));
        if self.stylesheet_attribute_change(node, key, value.as_deref())? { return Ok(()); }
        if self.dom_script(node) && matches!(key, "src" | "type" | "charset" | "nonce" | "integrity" | "crossorigin" | "referrerpolicy" | "fetchpriority" | "async" | "defer" | "nomodule") {
            let queue = key == "src" && self.state.dom.connected[node];
            self.attribute_named(node, name, value)?;
            if queue { self.queue_dynamic_script(node)?; }
            return Ok(());
        }
        let image_attribute = tag(self.doc, node) == "img" && matches!(key, "src" | "alt" | "width" | "height")
            || key=="align" && self.html_text_reflection(node,"align").is_some()
            || key=="usemap" && self.html_text_reflection(node,"useMap").is_some()
            || self.html_spacing_property(node, key)
            || key == "border" && self.html_border_property(node)
            || key == "ismap" && self.html_image_map_property(node)
            || key == "srcdoc" && self.iframe_srcdoc_property(node);
        // Nonce text follows existing CSP snapshots/queues; changing metadata
        // does not rerun a parsed script. Attribution reporting is unavailable:
        // preserve its content attribute without advertising an IDL API or
        // generating an attribution/report request.
        if !matches!(key, "id" | "class" | "style" | "title" | "hidden" | "disabled" | "role" | "lang" | "dir" | "nonce" | "attributionsrc")
            && !inert_attribute_name(key) && !image_attribute && !media_source && !style_type {
            return Err(format!("Supported attributes exclude {key:?} on {} in this native/metadata subset; no attribute was changed.",tag(self.doc,node)));
        }
        if key == "src" && image_attribute {
            return self.image_source_change(node, value);
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
